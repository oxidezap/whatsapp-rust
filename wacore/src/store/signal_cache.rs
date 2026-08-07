use std::collections::{HashMap, HashSet, VecDeque};
use std::num::NonZeroU64;
use std::sync::{Arc, Mutex as SyncMutex, MutexGuard as SyncMutexGuard};

use anyhow::Result;
use async_lock::Mutex;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};
use rand::RngExt;

use crate::libsignal::protocol::{
    ProtocolAddress, SenderKeyRecord, SessionCheckoutKey, SessionCheckoutStoreResult, SessionRecord,
};
use crate::libsignal::store::sender_key_name::SenderKeyName;
use crate::store::traits::SignalStore;

type StoreIncarnation = [u8; 16];

fn new_store_incarnation() -> StoreIncarnation {
    let mut incarnation = [0; 16];
    rand::make_rng::<rand::rngs::StdRng>().fill(&mut incarnation);
    incarnation
}

/// Evict clean (non-dirty, non-deleted) entries from a cache HashMap.
/// Negative entries (None values) are evicted first.
///
/// Amortized: the O(n) scan only runs once the map crosses the high watermark
/// (`max_entries + slack`), then it trims back down to `max_entries`. Steady
/// state over capacity therefore costs O(1) per call because a fresh scan needs
/// `slack` more growth inserts before it can fire again. Call it from every path
/// that grows the map, including read-populate (cache-miss) inserts, so the cache
/// stays bounded even under unique-key read floods; the early-out keeps it cheap.
fn evict_clean_entries<V>(
    cache: &mut UserIndexedCache<Option<V>>,
    dirty: &HashSet<Arc<str>>,
    deleted: Option<&HashSet<Arc<str>>>,
    max_entries: usize,
) {
    compact_users_if_needed(cache, max_entries);
    if cache.len() <= high_watermark(max_entries) {
        return;
    }
    let overflow = cache.len().saturating_sub(max_entries);
    let mut negative = Vec::with_capacity(overflow);
    let mut positive = Vec::with_capacity(overflow);
    for (k, v) in cache.iter() {
        if dirty.contains(k.as_ref()) {
            continue;
        }
        if let Some(del) = deleted
            && del.contains(k.as_ref())
        {
            continue;
        }
        if v.is_none() {
            negative.push(k.clone());
        } else {
            positive.push(k.clone());
        }
    }
    for key in negative.into_iter().chain(positive).take(overflow) {
        cache.remove(&key);
    }
}

/// Rebuild the user superset once eviction has let it drift a whole watermark
/// past the entries backing it. A rebuild leaves it no larger than the live key
/// count, so the next one is that many inserts away.
fn compact_users_if_needed<V>(cache: &mut UserIndexedCache<V>, max_entries: usize) {
    // Both conditions matter. The watermark keeps the rebuild rare; requiring
    // the index to exceed the live key count keeps it self-limiting, since a
    // rebuild always lands at or below that count. Without it, a store holding
    // more distinct users than the watermark — dirty entries that eviction
    // cannot trim, as a run of failing flushes produces — would rebuild on
    // every single update, under the global mutex.
    if cache.users_len() > high_watermark(max_entries) && cache.users_len() > cache.len() {
        cache.compact_users();
    }
}

/// Default max entries per store before clean entry eviction triggers.
const DEFAULT_MAX_CACHE_ENTRIES: usize = 2_000;

/// Removals retained for cold readers to consult. A reader that spans more
/// than this many removals is told its key went, which costs a re-read; the
/// window is what keeps the bookkeeping fixed-size and free of any per-reader
/// state that a cancelled read could strand.
const RECENT_REMOVALS: usize = 64;

/// Unlocked cold sender-key reads to try before falling back to reading under
/// the lock. Losing twice means a removal landed in both windows, which needs a
/// flush plus an eviction each time; the fallback keeps that bounded.
const SENDER_KEY_UNLOCKED_READ_ATTEMPTS: usize = 2;

/// Slack above `max_entries` the cache may grow to before an eviction scan
/// fires, expressed as a divisor of `max_entries` (1/8th here). Trimming back
/// to `max_entries` then amortizes the O(n) scan over this many inserts. A
/// floor keeps the amortization meaningful when `max_entries` is tiny (tests).
const EVICTION_SLACK_DIVISOR: usize = 8;
const EVICTION_SLACK_FLOOR: usize = 16;

/// The size the cache may reach before a scan is allowed to run. Eviction trims
/// back to `max_entries`, so the strict in-memory bound is this value.
fn high_watermark(max_entries: usize) -> usize {
    max_entries.saturating_add((max_entries / EVICTION_SLACK_DIVISOR).max(EVICTION_SLACK_FLOOR))
}

/// The set of addresses whose durability lease has not reached the backend,
/// paired with a lock-free view of "is it non-empty?" so the pre-wire query can
/// answer without taking the mutex that owns the set.
///
/// The two error directions are not symmetric. A flag reading `true` over an
/// empty set costs one redundant flush and is always correct. A flag reading
/// `false` over a non-empty set publishes ciphertext whose lease is only in
/// memory, which is counter reuse after a restart. So the set is private and
/// every mutator recomputes the flag from the set it just changed, under the
/// lock that owns it: lowering the flag is only reachable from an observed
/// empty set.
struct PendingWireGate {
    addresses: HashSet<Arc<str>>,
    non_empty: Arc<AtomicBool>,
}

impl PendingWireGate {
    fn new() -> Self {
        Self {
            addresses: HashSet::new(),
            non_empty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Handle for reading the gate without the owning lock.
    fn flag(&self) -> Arc<AtomicBool> {
        self.non_empty.clone()
    }

    fn insert(&mut self, address: Arc<str>) {
        self.addresses.insert(address);
        self.publish();
    }

    fn remove(&mut self, address: &str) {
        self.addresses.remove(address);
        self.publish();
    }

    fn clear(&mut self) {
        self.addresses.clear();
        self.publish();
    }

    fn is_empty(&self) -> bool {
        self.addresses.is_empty()
    }

    #[cfg(test)]
    fn contains(&self, address: &str) -> bool {
        self.addresses.contains(address)
    }

    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = &Arc<str>> {
        self.addresses.iter()
    }

    /// `Release` so a reader whose `Acquire` load sees the lowered flag also
    /// sees the removals that emptied the set.
    fn publish(&self) {
        self.non_empty
            .store(!self.addresses.is_empty(), Ordering::Release);
    }
}

fn protocol_address_matches_user(address: &str, user: &str) -> bool {
    address
        .strip_prefix(user)
        .is_some_and(|suffix| suffix.starts_with('@') || suffix.starts_with(':'))
}

/// The user half of a protocol address, matching the prefix
/// [`protocol_address_matches_user`] tests.
fn user_of_protocol_address(address: &str) -> &str {
    match address.find(['@', ':']) {
        Some(end) => &address[..end],
        None => address,
    }
}

/// A cache map that can answer "is any address here owned by this user?"
/// without scanning every key.
///
/// The user set is a deliberate superset: removals leave it untouched, so its
/// only error is a `true` for a user whose last entry is gone, costing one
/// migration pass that finds nothing. It can never answer "no state" for a
/// user that has some, which is the direction that would silently skip a
/// migration. `insert` is the only way in, so an entry cannot reach the map
/// without registering its user.
struct UserIndexedCache<V> {
    map: HashMap<Arc<str>, V>,
    users: HashSet<Arc<str>>,
    /// Counts removal events. A cold reader that saw a slot absent, released
    /// the lock, and finds it absent again cannot otherwise tell "never
    /// written" from "written, flushed, and evicted": a clean removal keeps the
    /// incarnation, so its pre-write bytes would be trusted as an exact reload.
    removal_seq: u64,
    /// The last `RECENT_REMOVALS` removals, newest last, so a reader can ask
    /// about its own key rather than about cache-wide churn. A fixed window
    /// needs no per-reader registration, so a cancelled read leaves nothing
    /// behind; a reader older than the window is told "removed" instead.
    recent_removals: VecDeque<(u64, Arc<str>)>,
    /// Sequence of the last removal that could not name its keys (`clear`,
    /// `retain`), which invalidates every reader older than it.
    opaque_removal_seq: u64,
}

impl<V> UserIndexedCache<V> {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            users: HashSet::new(),
            removal_seq: 0,
            recent_removals: VecDeque::new(),
            opaque_removal_seq: 0,
        }
    }

    fn insert(&mut self, key: Arc<str>, value: V) -> Option<V> {
        let user = user_of_protocol_address(&key);
        if !self.users.contains(user) {
            self.users.insert(Arc::from(user));
        }
        self.map.insert(key, value)
    }

    /// Stamp to take before a cold read releases the lock.
    fn removal_seq(&self) -> u64 {
        self.removal_seq
    }

    /// Whether `key` was removed after `since`. Conservative in two places: a
    /// removal that could not name its keys, and a reader older than the
    /// retained window. Both answer "removed", which costs a re-read rather
    /// than admitting bytes that predate a write.
    fn removed_since(&self, key: &str, since: u64) -> bool {
        if self.opaque_removal_seq > since {
            return true;
        }
        if self.removal_seq.saturating_sub(since) > RECENT_REMOVALS as u64 {
            return true;
        }
        self.recent_removals
            .iter()
            .any(|(seq, removed)| *seq > since && removed.as_ref() == key)
    }

    fn note_removal(&mut self, key: Arc<str>) {
        self.removal_seq += 1;
        if self.recent_removals.len() == RECENT_REMOVALS {
            self.recent_removals.pop_front();
        }
        self.recent_removals.push_back((self.removal_seq, key));
    }

    fn note_opaque_removal(&mut self) {
        self.removal_seq += 1;
        self.opaque_removal_seq = self.removal_seq;
    }

    fn has_user(&self, user: &str) -> bool {
        // Normalize the query the way the keys were derived. A matching
        // address begins with `user`, so a separator inside `user` is also the
        // address's first one and both collapse to the same key: an addressed
        // `19995551006:5` and a bare `19995551006` are one entry here.
        self.users.contains(user_of_protocol_address(user))
    }

    /// Drop users no longer backed by an entry. Bounds the superset's drift
    /// after eviction; callers gate it on a watermark so it stays amortized.
    fn compact_users(&mut self) {
        self.users.clear();
        let users: Vec<Arc<str>> = self
            .map
            .keys()
            .map(|key| Arc::from(user_of_protocol_address(key)))
            .collect();
        self.users.extend(users);
    }

    fn users_len(&self) -> usize {
        self.users.len()
    }

    /// Retained bytes of the bookkeeping beside the primary map: the user
    /// index, plus the removal window, whose keys outlive the entries they
    /// name and so are owned solely here. Keeps `memory_stats` from reporting
    /// only the map.
    fn overhead_bytes(&self) -> usize {
        self.users.iter().map(|user| user.len()).sum::<usize>()
            + self
                .recent_removals
                .iter()
                .map(|(_, key)| key.len() + size_of::<u64>())
                .sum::<usize>()
    }

    fn get(&self, key: &str) -> Option<&V> {
        self.map.get(key)
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut V> {
        self.map.get_mut(key)
    }

    fn get_key_value(&self, key: &str) -> Option<(&Arc<str>, &V)> {
        self.map.get_key_value(key)
    }

    fn contains_key(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    fn remove(&mut self, key: &str) -> Option<V> {
        let (key, value) = self.map.remove_entry(key)?;
        // Reuses the map's own `Arc<str>`, so recording a removal allocates
        // nothing.
        self.note_removal(key);
        Some(value)
    }

    fn retain(&mut self, keep: impl FnMut(&Arc<str>, &mut V) -> bool) {
        let before = self.map.len();
        self.map.retain(keep);
        if self.map.len() != before {
            // `retain` does not report which keys went.
            self.note_opaque_removal();
        }
    }

    fn clear(&mut self) {
        if !self.map.is_empty() {
            self.note_opaque_removal();
        }
        self.map.clear();
        self.users.clear();
    }

    fn len(&self) -> usize {
        self.map.len()
    }

    fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn iter(&self) -> impl Iterator<Item = (&Arc<str>, &V)> {
        self.map.iter()
    }

    #[cfg(test)]
    fn values(&self) -> impl Iterator<Item = &V> {
        self.map.values()
    }
}

/// In-memory write-back cache for Signal protocol state.
/// Keys use `Arc<str>` for O(1) clone. Sessions cached as objects (serialized on flush).
/// Capacity-bounded: every path that grows a store (writes and read-populate
/// misses) evicts non-dirty entries once the high watermark is crossed, trimming
/// back to `max_entries` (amortized O(1) thanks to the slack early-out).
pub struct SignalStoreCache {
    sessions: Mutex<SessionStoreState>,
    /// The gates' own flags (see `PendingWireGate`), so a send answers
    /// `needs_pre_wire_flush` without queueing behind a flush that holds either
    /// store lock across its backend I/O.
    session_wire_gate: Arc<AtomicBool>,
    session_recovery_generation: AtomicU64,
    has_pending_session_restores: AtomicBool,
    pending_session_restores: SyncMutex<Vec<PendingSessionRestore>>,
    identities: Mutex<ByteStoreState>,
    sender_keys: Mutex<SenderKeyStoreState>,
    sender_key_wire_gate: Arc<AtomicBool>,
    /// Fast-path guard for the normally-empty pending distribution map. Warm
    /// group encrypts avoid a second sender-key mutex acquisition.
    has_pending_sender_key_distributions: AtomicBool,
    /// Consumed one-time prekeys buffered for durable deletion, keyed by the
    /// address of the session whose pkmsg promotion consumed each one. The flush
    /// deletes a prekey only after that session is persisted, so a crash can never
    /// lose both and leave a redelivered pkmsg undecryptable. Per-address (not a
    /// global flag) so only the prekeys of still-volatile sessions are deferred.
    removed_prekeys: Mutex<HashMap<u32, Arc<str>>>,
    /// Per-(group, sender) locks serializing each sender-key chain advance.
    /// Coordination only (like the client session locks): never time-evicted.
    sender_key_locks: Mutex<HashMap<Arc<str>, Arc<Mutex<()>>>>,
    max_entries: usize,
}

// === Session object cache (no per-message serialize/deserialize) ===

/// Cache entry tracking whether a session is present, absent, or checked out
/// by an encrypt/decrypt operation.
enum SessionEntry {
    // `Arc` so `peek_session` (retry / LID-migration checks) bumps a refcount
    // instead of deep-cloning the record (KBs with archived states).
    Present(Arc<SessionRecord>),
    Absent,
    CheckedOut {
        had_session: bool,
        token: NonZeroU64,
    },
}

impl SessionEntry {
    fn exists(&self) -> bool {
        matches!(
            self,
            Self::Present(_)
                | Self::CheckedOut {
                    had_session: true,
                    ..
                }
        )
    }
}

enum CachedSessionCheckout {
    Missing(SessionCheckoutKey),
    Absent(SessionCheckoutKey),
    Busy,
    Present(SessionRecord, SessionCheckoutKey),
}

struct SessionStoreState {
    incarnation: StoreIncarnation,
    checkout_generation: u64,
    next_checkout_token: u64,
    cache: UserIndexedCache<SessionEntry>,
    dirty: HashSet<Arc<str>>,
    deleted: HashSet<Arc<str>>,
    /// Sessions whose raised counter reservation has not reached the backend
    /// yet. While any address is here, an outbound ciphertext may be relying
    /// on a lease that only exists in memory, so the send path must flush
    /// before the wire. Entries leave only when a flush actually persists
    /// them or their tombstone. Always a subset of `dirty` + `deleted`, so
    /// eviction can never drop a pending entry.
    reservation_pending: PendingWireGate,
}

impl SessionStoreState {
    fn new(incarnation: StoreIncarnation) -> Self {
        Self {
            incarnation,
            checkout_generation: 0,
            next_checkout_token: 1,
            cache: UserIndexedCache::new(),
            dirty: HashSet::new(),
            deleted: HashSet::new(),
            reservation_pending: PendingWireGate::new(),
        }
    }

    /// Reuse the existing Arc<str> key if the address is already in the cache,
    /// avoiding a heap allocation on every call (hot path: key always exists).
    fn key_for(&self, address: &str) -> Arc<str> {
        match self.cache.get_key_value(address) {
            Some((existing, _)) => existing.clone(),
            None => Arc::from(address),
        }
    }

    fn put(&mut self, address: &str, record: SessionRecord) {
        let addr = self.key_for(address);
        self.put_with_key(addr, record);
    }

    fn put_with_key(&mut self, addr: Arc<str>, mut record: SessionRecord) {
        // Take over the record's wire gate: the address stays pending until a
        // flush persists it, regardless of later checkout/put round trips.
        if record.has_pending_reservation() {
            record.clear_pending_reservation();
            self.reservation_pending.insert(addr.clone());
        }
        self.cache
            .insert(addr.clone(), SessionEntry::Present(Arc::new(record)));
        self.dirty.insert(addr.clone());
        self.deleted.remove(&addr);
    }

    fn checkout(&mut self, address: &str) -> CachedSessionCheckout {
        let token = NonZeroU64::new(self.next_checkout_token).unwrap_or(NonZeroU64::MIN);
        self.next_checkout_token = self.next_checkout_token.wrapping_add(1);
        if self.next_checkout_token == 0 {
            self.next_checkout_token = 1;
        }
        let checkout = SessionCheckoutKey::new(self.checkout_generation, token);
        let Some(entry) = self.cache.get_mut(address) else {
            return CachedSessionCheckout::Missing(checkout);
        };
        match entry {
            SessionEntry::Present(_) => {
                let SessionEntry::Present(record) = std::mem::replace(
                    entry,
                    SessionEntry::CheckedOut {
                        had_session: true,
                        token,
                    },
                ) else {
                    unreachable!()
                };
                CachedSessionCheckout::Present(
                    Arc::try_unwrap(record).unwrap_or_else(|arc| (*arc).clone()),
                    checkout,
                )
            }
            SessionEntry::Absent => {
                *entry = SessionEntry::CheckedOut {
                    had_session: false,
                    token,
                };
                CachedSessionCheckout::Absent(checkout)
            }
            SessionEntry::CheckedOut { .. } => CachedSessionCheckout::Busy,
        }
    }

    fn delete(&mut self, address: &str) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), SessionEntry::Absent);
        self.deleted.insert(addr.clone());
        self.dirty.remove(&addr);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
        self.deleted.clear();
        // Lossy callers have removed the transport; clean callers require no
        // pending gate before preserving exact-reload trust.
        self.reservation_pending.clear();
    }

    fn clear_clean_entries(&mut self) {
        self.cache
            .retain(|_, entry| matches!(entry, SessionEntry::CheckedOut { .. }));
        // Teardown drops nearly every entry at once and is not a hot path, so
        // settle the user superset here instead of letting it drift until the
        // watermark rebuild.
        self.cache.compact_users();
    }

    fn discard(&mut self, incarnation: StoreIncarnation, generation: u64) {
        self.clear();
        self.incarnation = incarnation;
        self.checkout_generation = generation;
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        compact_users_if_needed(&mut self.cache, max_entries);
        if self.cache.len() <= high_watermark(max_entries) {
            return;
        }
        let overflow = self.cache.len().saturating_sub(max_entries);
        let mut negative = Vec::with_capacity(overflow);
        let mut positive = Vec::with_capacity(overflow);
        for (k, v) in self.cache.iter() {
            if self.dirty.contains(k.as_ref()) || self.deleted.contains(k.as_ref()) {
                continue;
            }
            match v {
                SessionEntry::CheckedOut { .. } => continue, // never evict checked-out
                SessionEntry::Absent => negative.push(k.clone()),
                SessionEntry::Present(_) => positive.push(k.clone()),
            }
        }
        for key in negative.into_iter().chain(positive).take(overflow) {
            self.cache.remove(&key);
        }
    }
}

struct PendingSessionRestore {
    address: Arc<str>,
    record: Option<SessionRecord>,
    checkout: SessionCheckoutKey,
    had_session: bool,
    completion: Option<Arc<AtomicBool>>,
}

// === Sender key object cache (same pattern as sessions) ===

struct SenderKeyStoreState {
    incarnation: StoreIncarnation,
    // `Arc`-wrapped so a warm `get_sender_key` (the per-send peek reads and the
    // per-decrypt load) bumps a refcount instead of deep-cloning the record's
    // `VecDeque<SenderKeyState>` with up to `MAX_MESSAGE_KEYS` message keys each.
    cache: UserIndexedCache<Option<Arc<SenderKeyRecord>>>,
    dirty: HashSet<Arc<str>>,
    /// Chains whose outbound iteration lease was raised but not yet persisted;
    /// the send path must flush before the wire while any entry is here.
    /// Decrypt-side dirtiness deliberately does NOT enter this set (it
    /// re-derives forward),
    /// so unrelated group receives never force a sync flush onto a DM send.
    wire_gate_pending: PendingWireGate,
    /// Distributions created for a new outbound chain but not yet returned by
    /// a successful encryption call. A failed durability gate leaves the
    /// distribution here so a retry cannot emit ciphertext for an
    /// undistributed key.
    pending_distributions: HashMap<Arc<str>, Arc<[u8]>>,
}

impl SenderKeyStoreState {
    fn new(incarnation: StoreIncarnation) -> Self {
        Self {
            incarnation,
            cache: UserIndexedCache::new(),
            dirty: HashSet::new(),
            wire_gate_pending: PendingWireGate::new(),
            pending_distributions: HashMap::new(),
        }
    }

    fn key_for(&self, address: &str) -> Arc<str> {
        match self.cache.get_key_value(address) {
            Some((existing, _)) => existing.clone(),
            None => Arc::from(address),
        }
    }

    fn put(&mut self, address: &str, mut record: SenderKeyRecord) {
        let addr = self.key_for(address);
        if record.is_wire_gated() {
            record.clear_wire_gated();
            self.wire_gate_pending.insert(addr.clone());
        }
        self.cache.insert(addr.clone(), Some(Arc::new(record)));
        self.dirty.insert(addr.clone());
    }

    fn delete(&mut self, address: &str) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), None);
        self.dirty.insert(addr.clone());
        self.pending_distributions.remove(address);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
        self.wire_gate_pending.clear();
        self.pending_distributions.clear();
    }

    fn discard(&mut self, incarnation: StoreIncarnation) {
        self.clear();
        self.incarnation = incarnation;
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        evict_clean_entries(&mut self.cache, &self.dirty, None, max_entries);
    }
}

// === Byte cache for identities ===

struct ByteStoreState {
    /// Cached entries. `None` value = known-absent (negative cache).
    cache: UserIndexedCache<Option<Arc<[u8]>>>,
    dirty: HashSet<Arc<str>>,
    deleted: HashSet<Arc<str>>,
}

impl ByteStoreState {
    fn new() -> Self {
        Self {
            cache: UserIndexedCache::new(),
            dirty: HashSet::new(),
            deleted: HashSet::new(),
        }
    }

    /// Reuse the existing Arc<str> key if the address is already in the cache.
    fn key_for(&self, address: &str) -> Arc<str> {
        match self.cache.get_key_value(address) {
            Some((existing, _)) => existing.clone(),
            None => Arc::from(address),
        }
    }

    /// Insert data, skipping if bytes are identical (avoids redundant dirty marks).
    /// Use for stores where data rarely changes (identities).
    fn put_dedup(&mut self, address: &str, data: &[u8]) {
        if let Some(Some(existing)) = self.cache.get(address)
            && existing.as_ref() == data
        {
            return;
        }
        self.put(address, data);
    }

    /// Insert data unconditionally. Use for stores where data changes every
    /// message (sender keys) — the byte comparison would always fail.
    fn put(&mut self, address: &str, data: &[u8]) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), Some(Arc::from(data)));
        self.dirty.insert(addr.clone());
        self.deleted.remove(&addr);
    }

    /// Mark an entry as deleted (negative-cached).
    fn delete(&mut self, address: &str) {
        let addr = self.key_for(address);
        self.cache.insert(addr.clone(), None);
        self.deleted.insert(addr.clone());
        self.dirty.remove(&addr);
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
        self.deleted.clear();
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
        evict_clean_entries(
            &mut self.cache,
            &self.dirty,
            Some(&self.deleted),
            max_entries,
        );
    }
}

impl Default for SignalStoreCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalStoreCache {
    pub fn new() -> Self {
        Self::with_max_entries(DEFAULT_MAX_CACHE_ENTRIES)
    }

    pub fn with_max_entries(max_entries: usize) -> Self {
        Self::with_max_entries_and_incarnation(max_entries, new_store_incarnation())
    }

    fn with_max_entries_and_incarnation(max_entries: usize, incarnation: StoreIncarnation) -> Self {
        let sessions = SessionStoreState::new(incarnation);
        let sender_keys = SenderKeyStoreState::new(incarnation);
        Self {
            session_wire_gate: sessions.reservation_pending.flag(),
            sender_key_wire_gate: sender_keys.wire_gate_pending.flag(),
            sessions: Mutex::new(sessions),
            session_recovery_generation: AtomicU64::new(0),
            has_pending_session_restores: AtomicBool::new(false),
            pending_session_restores: SyncMutex::new(Vec::new()),
            identities: Mutex::new(ByteStoreState::new()),
            sender_keys: Mutex::new(sender_keys),
            has_pending_sender_key_distributions: AtomicBool::new(false),
            removed_prekeys: Mutex::new(HashMap::new()),
            sender_key_locks: Mutex::new(HashMap::new()),
            max_entries,
        }
    }

    fn pending_session_restores(&self) -> SyncMutexGuard<'_, Vec<PendingSessionRestore>> {
        self.pending_session_restores
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn drain_session_restores(&self, state: &mut SessionStoreState) {
        if !self.has_pending_session_restores.load(Ordering::Acquire) {
            return;
        }
        let mut pending = self.pending_session_restores();
        for PendingSessionRestore {
            address,
            record,
            checkout,
            had_session,
            completion,
        } in pending.drain(..)
        {
            let key = if checkout.generation() == state.checkout_generation
                && let Some((
                    key,
                    SessionEntry::CheckedOut {
                        had_session: was_present,
                        token,
                    },
                )) = state.cache.get_key_value(address.as_ref())
                && *was_present == had_session
                && *token == checkout.token()
            {
                Some(key.clone())
            } else {
                None
            };
            let restored = key.is_some();
            match (key, record) {
                (Some(key), Some(record)) => state.put_with_key(key, record),
                (Some(key), None) => {
                    state.cache.insert(key, SessionEntry::Absent);
                }
                (None, _) => {}
            }
            if let Some(completion) = completion {
                completion.store(restored, Ordering::Release);
            }
        }
        self.has_pending_session_restores
            .store(false, Ordering::Release);
        state.evict_if_needed(self.max_entries);
    }

    async fn lock_sessions(&self) -> async_lock::MutexGuard<'_, SessionStoreState> {
        let mut state = self.sessions.lock().await;
        self.drain_session_restores(&mut state);
        state
    }

    fn try_lock_sessions(&self) -> Option<async_lock::MutexGuard<'_, SessionStoreState>> {
        let mut state = self.sessions.try_lock()?;
        self.drain_session_restores(&mut state);
        Some(state)
    }

    /// A cancelled owner must return its record without awaiting the contested cache lock.
    #[doc(hidden)]
    pub fn restore_session_from_checkout(
        &self,
        address: &ProtocolAddress,
        record: SessionRecord,
        checkout: SessionCheckoutKey,
        had_session: bool,
    ) -> SessionCheckoutStoreResult {
        if checkout.generation() != self.session_recovery_generation.load(Ordering::Acquire) {
            return SessionCheckoutStoreResult::Rejected;
        }
        if let Some(mut state) = self.try_lock_sessions() {
            if checkout.generation() != self.session_recovery_generation.load(Ordering::Acquire)
                || checkout.generation() != state.checkout_generation
            {
                return SessionCheckoutStoreResult::Rejected;
            }
            let Some((
                key,
                SessionEntry::CheckedOut {
                    had_session: was_present,
                    token,
                },
            )) = state.cache.get_key_value(address.as_str())
            else {
                return SessionCheckoutStoreResult::Rejected;
            };
            if *was_present != had_session || *token != checkout.token() {
                return SessionCheckoutStoreResult::Rejected;
            }
            let key = key.clone();
            state.put_with_key(key, record);
            state.evict_if_needed(self.max_entries);
            return SessionCheckoutStoreResult::Stored;
        }

        let mut pending = self.pending_session_restores();
        if checkout.generation() != self.session_recovery_generation.load(Ordering::Acquire) {
            return SessionCheckoutStoreResult::Rejected;
        }
        let completion = Arc::new(AtomicBool::new(false));
        pending.push(PendingSessionRestore {
            address: Arc::from(address.as_str()),
            record: Some(record),
            checkout,
            had_session,
            completion: Some(completion.clone()),
        });
        self.has_pending_session_restores
            .store(true, Ordering::Release);
        SessionCheckoutStoreResult::Pending(completion)
    }

    /// An empty checkout must release its pinned cache slot even when dropped.
    #[doc(hidden)]
    pub fn cancel_session_checkout(&self, address: &ProtocolAddress, checkout: SessionCheckoutKey) {
        if checkout.generation() != self.session_recovery_generation.load(Ordering::Acquire) {
            return;
        }
        let Some(mut state) = self.try_lock_sessions() else {
            let mut pending = self.pending_session_restores();
            if checkout.generation() == self.session_recovery_generation.load(Ordering::Acquire) {
                pending.push(PendingSessionRestore {
                    address: Arc::from(address.as_str()),
                    record: None,
                    checkout,
                    had_session: false,
                    completion: None,
                });
                self.has_pending_session_restores
                    .store(true, Ordering::Release);
            }
            return;
        };
        let key = if checkout.generation()
            == self.session_recovery_generation.load(Ordering::Acquire)
            && checkout.generation() == state.checkout_generation
            && let Some((
                key,
                SessionEntry::CheckedOut {
                    had_session: false,
                    token,
                },
            )) = state.cache.get_key_value(address.as_str())
            && *token == checkout.token()
        {
            Some(key.clone())
        } else {
            None
        };
        if let Some(key) = key {
            state.cache.insert(key, SessionEntry::Absent);
            state.evict_if_needed(self.max_entries);
        }
    }

    /// A queued commit drives its own restore; cancellation may leave it for the next cache access.
    #[doc(hidden)]
    pub async fn complete_session_checkout(&self) {
        drop(self.lock_sessions().await);
    }

    /// Model a flush followed by a capacity eviction or `clear_after_flush`:
    /// the chain becomes clean and then leaves the cache outright.
    #[cfg(test)]
    async fn drop_clean_sender_key_for_test(&self, cache_key: &str) {
        let mut state = self.sender_keys.lock().await;
        state.dirty.remove(cache_key);
        state.wire_gate_pending.remove(cache_key);
        state.cache.remove(cache_key);
    }

    /// Whether any session or identity is known for `user` (across device ids),
    /// checking the in-memory cache first, then the durable backend. Lets a
    /// caller skip a per-device migration scan for a user we've never had Signal
    /// state with. Conservative on the cache side: any matching key counts
    /// (even a stale/checked-out marker), so it never reports "none" when state
    /// might exist.
    pub async fn has_state_for_user(&self, user: &str, backend: &dyn SignalStore) -> Result<bool> {
        if self.lock_sessions().await.cache.has_user(user) {
            return Ok(true);
        }
        if self.identities.lock().await.cache.has_user(user) {
            return Ok(true);
        }
        Ok(backend.has_signal_state_for_user(user).await?)
    }

    /// Whether this user's pairwise session or identity writes still need a
    /// durability retry. Migration uses this after a failed flush, when the
    /// cache already reflects the move and a second pass makes no new changes.
    pub async fn has_pending_pairwise_writes_for_user(&self, user: &str) -> bool {
        {
            let state = self.lock_sessions().await;
            if state
                .dirty
                .iter()
                .chain(&state.deleted)
                .any(|address| protocol_address_matches_user(address, user))
            {
                return true;
            }
        }
        let state = self.identities.lock().await;
        state
            .dirty
            .iter()
            .chain(&state.deleted)
            .any(|address| protocol_address_matches_user(address, user))
    }

    // === Sessions (object cache — serialize only during flush) ===

    /// Decode a stored session, quarantining a blob this build cannot read.
    ///
    /// Deserialization is a pure function of the bytes, so a row that fails
    /// once fails identically forever — and it fails on *every* path that must
    /// load the address, including the decrypt of the peer's next pre-key
    /// message and the retry repair, which are precisely the paths that would
    /// otherwise replace it. Propagating the error therefore strands the
    /// address until an operator deletes the row by hand. Reporting it as
    /// absent instead lets the ordinary no-session recovery fetch a pre-key
    /// bundle and overwrite it. Nothing is lost: a record we cannot decode can
    /// derive no key material, so it cannot repeat a counter either.
    fn decode_stored_session(
        key: &str,
        bytes: &[u8],
        incarnation: &StoreIncarnation,
    ) -> Option<SessionRecord> {
        match SessionRecord::deserialize_for_store(bytes, incarnation) {
            Ok(record) => Some(record),
            Err(error) => {
                log::error!(
                    "discarding unreadable session row for addr#{:016x}: {error} — recovering with a fresh session",
                    wacore_binary::jid::observe_token(key)
                );
                crate::telemetry::session_record_quarantined();
                None
            }
        }
    }

    /// Takes ownership of the cached session, leaving a `CheckedOut` marker.
    /// Callers must return the record with [`put_session`](Self::put_session) after use.
    pub async fn get_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<SessionRecord>> {
        let (record, checkout) = self.checkout_session(address, backend).await?;
        if record.is_none() {
            self.cancel_session_checkout(address, checkout);
        }
        Ok(record)
    }

    /// The checkout key rejects stale owners and owners from before a lossy reset.
    #[doc(hidden)]
    pub async fn checkout_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<(Option<SessionRecord>, SessionCheckoutKey)> {
        let key = address.as_str();
        {
            let mut state = self.lock_sessions().await;
            match state.checkout(key) {
                CachedSessionCheckout::Present(record, checkout) => {
                    return Ok((Some(record), checkout));
                }
                CachedSessionCheckout::Absent(checkout) => return Ok((None, checkout)),
                CachedSessionCheckout::Busy => {
                    anyhow::bail!("session is already checked out")
                }
                CachedSessionCheckout::Missing(_) => {}
            }
        }
        // Backend I/O outside the lock
        let backend_result = backend.get_session(key).await?;
        let mut state = self.lock_sessions().await;
        let checkout = match state.checkout(key) {
            CachedSessionCheckout::Present(record, checkout) => {
                return Ok((Some(record), checkout));
            }
            CachedSessionCheckout::Absent(checkout) => return Ok((None, checkout)),
            CachedSessionCheckout::Busy => anyhow::bail!("session is already checked out"),
            CachedSessionCheckout::Missing(checkout) => checkout,
        };
        match backend_result
            .as_deref()
            .and_then(|bytes| Self::decode_stored_session(key, bytes, &state.incarnation))
        {
            Some(record) => {
                state.cache.insert(
                    Arc::from(key),
                    SessionEntry::CheckedOut {
                        had_session: true,
                        token: checkout.token(),
                    },
                );
                state.evict_if_needed(self.max_entries);
                Ok((Some(record), checkout))
            }
            None => {
                state.cache.insert(
                    Arc::from(key),
                    SessionEntry::CheckedOut {
                        had_session: false,
                        token: checkout.token(),
                    },
                );
                state.evict_if_needed(self.max_entries);
                Ok((None, checkout))
            }
        }
    }

    /// A warm checkout avoids the device lock and boxed async store future.
    #[doc(hidden)]
    pub fn try_checkout_session(
        &self,
        address: &ProtocolAddress,
    ) -> Option<Result<(Option<SessionRecord>, SessionCheckoutKey)>> {
        let mut state = self.try_lock_sessions()?;
        match state.checkout(address.as_str()) {
            CachedSessionCheckout::Present(record, checkout) => Some(Ok((Some(record), checkout))),
            CachedSessionCheckout::Absent(checkout) => Some(Ok((None, checkout))),
            CachedSessionCheckout::Busy => {
                Some(Err(anyhow::anyhow!("session is already checked out")))
            }
            CachedSessionCheckout::Missing(_) => None,
        }
    }

    /// Non-destructive read. Clones the session without removing it from
    /// cache. Use for inspection-only paths (retry, LID migration checks).
    pub async fn peek_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<Arc<SessionRecord>>> {
        let key = address.as_str();
        {
            let state = self.lock_sessions().await;
            if let Some(entry) = state.cache.get(key) {
                return match entry {
                    SessionEntry::Present(record) => Ok(Some(record.clone())),
                    _ => Ok(None),
                };
            }
        }
        // Backend I/O outside the lock
        let backend_result = backend.get_session(key).await?;
        let mut state = self.lock_sessions().await;
        if let Some(entry) = state.cache.get(key) {
            return match entry {
                SessionEntry::Present(record) => Ok(Some(record.clone())),
                SessionEntry::Absent | SessionEntry::CheckedOut { .. } => Ok(None),
            };
        }
        match backend_result
            .as_deref()
            .and_then(|bytes| Self::decode_stored_session(key, bytes, &state.incarnation))
        {
            Some(record) => {
                let record = Arc::new(record);
                state
                    .cache
                    .insert(Arc::from(key), SessionEntry::Present(record.clone()));
                state.evict_if_needed(self.max_entries);
                Ok(Some(record))
            }
            None => {
                state.cache.insert(Arc::from(key), SessionEntry::Absent);
                state.evict_if_needed(self.max_entries);
                Ok(None)
            }
        }
    }

    pub async fn put_session(&self, address: &ProtocolAddress, record: SessionRecord) {
        let mut state = self.lock_sessions().await;
        state.put(address.as_str(), record);
        state.evict_if_needed(self.max_entries);
    }

    /// Non-blocking [`Self::put_session`]: completes synchronously when the
    /// sessions lock is free. Returns the record back on contention (e.g. a
    /// flush commit in progress) so the caller can take the async path
    /// without cloning.
    // Err carries the record by value on purpose: boxing it would add the
    // very allocation this fast path exists to avoid.
    #[allow(clippy::result_large_err)]
    pub fn try_put_session(
        &self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> core::result::Result<(), SessionRecord> {
        match self.try_lock_sessions() {
            Some(mut state) => {
                state.put(address.as_str(), record);
                state.evict_if_needed(self.max_entries);
                Ok(())
            }
            None => Err(record),
        }
    }

    /// Non-blocking [`Self::has_session`] restricted to what the cache already
    /// knows: `Some` only when the lock is free AND the entry is cached;
    /// `None` sends the caller to the async path (backend consult).
    pub fn try_has_session(&self, address: &ProtocolAddress) -> Option<bool> {
        let state = self.try_lock_sessions()?;
        state.cache.get(address.as_str()).map(SessionEntry::exists)
    }

    pub async fn delete_session(&self, address: &ProtocolAddress) {
        let mut state = self.lock_sessions().await;
        state.delete(address.as_str());
    }

    /// Non-destructive existence check; an empty checkout remains absent.
    ///
    /// A cold probe reads and decodes the row rather than asking the backend
    /// whether it exists. Row existence alone would report a quarantined
    /// session as present, and this is the probe that decides whether a send
    /// fetches a pre-key bundle: answering `true` for a row that
    /// [`Self::checkout_session`] will then discard skips the recovery, and the
    /// send fails or silently drops that recipient from the fan-out. The decode
    /// is not wasted work either, since the record it produces is cached for
    /// the checkout that follows.
    pub async fn has_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<bool> {
        let key = address.as_str();
        {
            let state = self.lock_sessions().await;
            if let Some(entry) = state.cache.get(key) {
                return Ok(entry.exists());
            }
        }
        // Backend I/O outside the lock
        let backend_result = backend.get_session(key).await?;
        let mut state = self.lock_sessions().await;
        if let Some(entry) = state.cache.get(key) {
            return Ok(entry.exists());
        }
        let entry = match backend_result
            .as_deref()
            .and_then(|bytes| Self::decode_stored_session(key, bytes, &state.incarnation))
        {
            Some(record) => SessionEntry::Present(Arc::new(record)),
            None => SessionEntry::Absent,
        };
        let exists = entry.exists();
        state.cache.insert(Arc::from(key), entry);
        state.evict_if_needed(self.max_entries);
        Ok(exists)
    }

    // === Identities ===

    pub async fn get_identity(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<Arc<[u8]>>> {
        let key = address.as_str();
        // Cache check inside scoped lock so concurrent callers don't queue on
        // the mutex during the backend roundtrip. Mirrors get_session/has_session.
        {
            let state = self.identities.lock().await;
            if let Some(cached) = state.cache.get(key) {
                return Ok(cached.clone());
            }
        }
        // Backend I/O outside the lock.
        let data = backend.load_identity(key).await?;
        let arc_data = data.map(Arc::from);
        let mut state = self.identities.lock().await;
        // Re-check: another task may have populated the cache while we awaited.
        if let Some(cached) = state.cache.get(key) {
            return Ok(cached.clone());
        }
        state.cache.insert(Arc::from(key), arc_data.clone());
        state.evict_if_needed(self.max_entries);
        Ok(arc_data)
    }

    pub async fn put_identity(&self, address: &ProtocolAddress, data: &[u8]) {
        let mut state = self.identities.lock().await;
        state.put_dedup(address.as_str(), data);
        state.evict_if_needed(self.max_entries);
    }

    /// Non-blocking cached identity read: `Some` only when the lock is free
    /// AND the entry is cached (`Some(None)` = known-absent); `None` sends
    /// the caller to the async path.
    pub fn try_get_identity(&self, address: &ProtocolAddress) -> Option<Option<Arc<[u8]>>> {
        let state = self.identities.try_lock()?;
        state.cache.get(address.as_str()).cloned()
    }

    /// Non-blocking [`Self::put_identity`]; `false` = contended, caller must
    /// take the async path.
    pub fn try_put_identity(&self, address: &ProtocolAddress, data: &[u8]) -> bool {
        match self.identities.try_lock() {
            Some(mut state) => {
                state.put_dedup(address.as_str(), data);
                state.evict_if_needed(self.max_entries);
                true
            }
            None => false,
        }
    }

    pub async fn delete_identity(&self, address: &ProtocolAddress) {
        let mut state = self.identities.lock().await;
        state.delete(address.as_str());
    }

    // === Sender Keys ===

    /// Returns a shared (`Arc`) handle to the cached sender-key record. A warm hit
    /// is a refcount bump, not a deep clone of the message-key backlog. Callers
    /// that need to mutate clone the inner record (e.g. via the trait
    /// `load_sender_key`), so the cache copy is never mutated through this handle.
    pub async fn get_sender_key(
        &self,
        name: &SenderKeyName,
        backend: &dyn SignalStore,
    ) -> Result<Option<Arc<SenderKeyRecord>>> {
        let key = name.cache_key();
        for _ in 0..SENDER_KEY_UNLOCKED_READ_ATTEMPTS {
            let (incarnation, since) = {
                let state = self.sender_keys.lock().await;
                if let Some(cached) = state.cache.get(key) {
                    return Ok(cached.clone());
                }
                (state.incarnation, state.cache.removal_seq())
            };

            // Decoding stays outside the lock too: a cold chain can carry
            // MAX_MESSAGE_KEYS skipped keys, and parsing them is the expensive
            // half of the miss. Errors are held rather than raised, so a
            // concurrent write still wins the re-check below: an unreadable row
            // must not fail an operation the cache can already answer.
            let decoded: Result<Option<Arc<SenderKeyRecord>>> =
                match backend.get_sender_key(key).await {
                    Ok(Some(bytes)) => SenderKeyRecord::deserialize_for_store(&bytes, &incarnation)
                        .map(|record| Some(Arc::new(record)))
                        .map_err(anyhow::Error::from),
                    Ok(None) => Ok(None),
                    Err(error) => Err(anyhow::Error::from(error)),
                };

            let mut state = self.sender_keys.lock().await;
            // A put or delete that landed while we awaited describes the chain
            // more recently than the bytes we read, so it wins; we would
            // otherwise resurrect a deleted chain or undo a fresher iteration.
            if let Some(cached) = state.cache.get(key) {
                return Ok(cached.clone());
            }
            // Still absent, but that is only trustworthy if this key was not
            // removed and no lossy clear reset the incarnation meanwhile.
            // Either could mean a newer record was written and then dropped,
            // leaving our older bytes to be adopted as an exact reload and let
            // the chain resume an iteration that has already been published.
            if state.incarnation == incarnation && !state.cache.removed_since(key, since) {
                let record = decoded?;
                state.cache.insert(Arc::from(key), record.clone());
                state.evict_if_needed(self.max_entries);
                return Ok(record);
            }
        }

        // Repeatedly raced. Read under the lock, which cannot be raced at all.
        let mut state = self.sender_keys.lock().await;
        if let Some(cached) = state.cache.get(key) {
            return Ok(cached.clone());
        }
        let record = match backend.get_sender_key(key).await? {
            Some(bytes) => Some(Arc::new(SenderKeyRecord::deserialize_for_store(
                &bytes,
                &state.incarnation,
            )?)),
            None => None,
        };
        state.cache.insert(Arc::from(key), record.clone());
        state.evict_if_needed(self.max_entries);
        Ok(record)
    }

    pub async fn put_sender_key(&self, name: &SenderKeyName, record: SenderKeyRecord) {
        let mut state = self.sender_keys.lock().await;
        state.put(name.cache_key(), record);
        state.evict_if_needed(self.max_entries);
    }

    /// Retain a newly created sender-key distribution until the encryption
    /// operation that owns it passes its durability gate.
    pub async fn cache_pending_sender_key_distribution(
        &self,
        name: &SenderKeyName,
        distribution: Arc<[u8]>,
    ) {
        let mut state = self.sender_keys.lock().await;
        let key = state.key_for(name.cache_key());
        state.pending_distributions.insert(key, distribution);
        self.has_pending_sender_key_distributions
            .store(true, Ordering::Release);
    }

    /// Return a retained distribution whose prior encryption attempt did not
    /// complete its durability gate.
    pub async fn pending_sender_key_distribution(&self, name: &SenderKeyName) -> Option<Arc<[u8]>> {
        if !self
            .has_pending_sender_key_distributions
            .load(Ordering::Acquire)
        {
            return None;
        }
        self.sender_keys
            .lock()
            .await
            .pending_distributions
            .get(name.cache_key())
            .cloned()
    }

    /// Clear a retained distribution after a successful encryption, but only
    /// if it is still the distribution observed by that call. This prevents a
    /// concurrent chain replacement from losing its newer distribution.
    pub async fn clear_pending_sender_key_distribution(
        &self,
        name: &SenderKeyName,
        expected: &[u8],
    ) {
        let mut state = self.sender_keys.lock().await;
        if state
            .pending_distributions
            .get(name.cache_key())
            .is_some_and(|distribution| distribution.as_ref() == expected)
        {
            state.pending_distributions.remove(name.cache_key());
            if state.pending_distributions.is_empty() {
                self.has_pending_sender_key_distributions
                    .store(false, Ordering::Release);
            }
        }
    }

    /// Shared lock for the `name` chain. Same name returns the same lock so a
    /// concurrent encrypt can't read a chain iteration another is advancing.
    pub async fn sender_key_lock(&self, name: &SenderKeyName) -> Arc<Mutex<()>> {
        self.shared_named_lock(name.cache_key()).await
    }

    /// Shared per-group session-setup lock (see
    /// `SenderKeyStore::session_setup_lock`). Lives in the chain-lock map
    /// under a suffixed key; chain cache_keys end in a numeric device id, so
    /// the key spaces are disjoint.
    pub async fn session_setup_lock(&self, name: &SenderKeyName) -> Arc<Mutex<()>> {
        let mut key = String::with_capacity(name.cache_key().len() + 8);
        key.push_str(name.cache_key());
        key.push_str("::setup");
        self.shared_named_lock(&key).await
    }

    async fn shared_named_lock(&self, key: &str) -> Arc<Mutex<()>> {
        let mut map = self.sender_key_locks.lock().await;
        if let Some(lock) = map.get(key) {
            return lock.clone();
        }
        // Drop idle locks (held only by the map) once the map grows large.
        if map.len() >= self.max_entries {
            map.retain(|_, lock| Arc::strong_count(lock) > 1);
        }
        let lock = Arc::new(Mutex::new(()));
        map.insert(Arc::from(key), lock.clone());
        lock
    }

    /// Prevent an in-flight mutation from storing the retired chain again.
    pub async fn delete_sender_key(&self, cache_key: &str) {
        let lock = self.shared_named_lock(cache_key).await;
        let _guard = lock.lock().await;
        let mut state = self.sender_keys.lock().await;
        state.delete(cache_key);
        if state.pending_distributions.is_empty() {
            self.has_pending_sender_key_distributions
                .store(false, Ordering::Release);
        }
    }

    /// Delete one sender-key chain from the cache and backend while holding its
    /// chain lock. Only this record is persisted, avoiding a global cache flush
    /// while preventing an in-flight mutation from resurrecting the old chain.
    pub async fn delete_sender_key_durable(
        &self,
        name: &SenderKeyName,
        backend: &dyn SignalStore,
    ) -> Result<()> {
        let lock = self.sender_key_lock(name).await;
        let _guard = lock.lock().await;
        let cache_key = name.cache_key();
        {
            let mut state = self.sender_keys.lock().await;
            state.delete(cache_key);
            if state.pending_distributions.is_empty() {
                self.has_pending_sender_key_distributions
                    .store(false, Ordering::Release);
            }
        }

        // The per-chain guard above keeps this record stable; unrelated chains
        // must not queue behind backend latency on the global cache mutex.
        backend.delete_sender_key(cache_key).await?;

        let mut state = self.sender_keys.lock().await;
        if matches!(state.cache.get(cache_key), Some(None)) {
            state.dirty.remove(cache_key);
            state.wire_gate_pending.remove(cache_key);
        } else if state.cache.contains_key(cache_key) {
            // Defensive against a direct cache writer that did not honor the
            // chain lock: the backend delete may have raced its write, so keep
            // the replacement dirty for the next flush.
            let key = state.key_for(cache_key);
            state.dirty.insert(key);
        }
        state.evict_if_needed(self.max_entries);
        Ok(())
    }

    // === Consumed pre-keys ===

    /// Buffer a consumed one-time pre-key for deletion on the next flush, keyed by
    /// the address of the session whose pkmsg promotion consumed it, rather than
    /// deleting it from the backend immediately. The decrypt path promotes that
    /// session into the (volatile) session cache, so deleting the prekey durably
    /// before the session is flushed would lose both on a crash. Flush removes a
    /// buffered prekey only once its own session is durable; a session still
    /// checked out defers just that prekey, not the others.
    pub async fn remove_prekey(&self, prekey_id: u32, session_address: &str) {
        self.removed_prekeys
            .lock()
            .await
            .insert(prekey_id, Arc::from(session_address));
    }

    // === Flush ===

    /// Flush all dirty state to the backend.
    ///
    /// Identities and sender keys are flushed independently under their own lock,
    /// so each is locked only during its own I/O while the others stay free for
    /// concurrent encrypt/decrypt. Sessions and consumed pre-keys are committed
    /// together under the single sessions lock: the prekey delete must be atomic
    /// with the session put against concurrent buffering, so they cannot use
    /// separate lock scopes. Within each scope the lock is held across snapshot,
    /// I/O, and clear, so there is no race between snapshot and clear and dirty
    /// sets are cleared only after successful writes.
    pub async fn flush(&self, backend: &dyn SignalStore) -> Result<()> {
        // Flush sessions: one batched write for all dirty puts instead of one
        // backend call (and one SQLite transaction) per session.
        {
            let mut state = self.lock_sessions().await;
            let incarnation = state.incarnation;
            let dirty_keys: Vec<_> = state.dirty.iter().cloned().collect();
            let deleted_keys: Vec<_> = state.deleted.iter().cloned().collect();

            let mut batch: Vec<(Arc<str>, bytes::Bytes)> = Vec::new();
            for address in &dirty_keys {
                // A dirty key is Present (promoted) or CheckedOut (taken by a
                // concurrent reader). Only the Present ones can be persisted now;
                // a CheckedOut one stays volatile and its consumed prekey is
                // deferred below until a later flush sees it durable.
                if let Some(SessionEntry::Present(record)) = state.cache.get(address.as_ref()) {
                    let mut buf = Vec::new();
                    record.serialize_into_for_store(&mut buf, &incarnation);
                    batch.push((address.clone(), bytes::Bytes::from(buf)));
                }
            }
            if !batch.is_empty() {
                backend.put_sessions_batch(&batch).await?;
                // These leases are durable now; only the written addresses
                // leave the pending set (a CheckedOut session stays gated).
                for (address, _) in &batch {
                    state.reservation_pending.remove(address);
                }
            }
            for address in &deleted_keys {
                backend.delete_session(address).await?;
                state.reservation_pending.remove(address);
            }

            for key in &dirty_keys {
                if !matches!(
                    state.cache.get(key.as_ref()),
                    Some(SessionEntry::CheckedOut { .. })
                ) {
                    state.dirty.remove(key);
                }
            }
            for key in &deleted_keys {
                state.deleted.remove(key);
            }
            state.evict_if_needed(self.max_entries);

            // Delete a consumed one-time prekey only once its session is durable.
            // Durability is decided per session, not from a single flush's batch:
            // a Present (clean at drain) entry is persisted (by this flush or an
            // earlier one); a CheckedOut entry is the still-volatile promoted copy,
            // so defer; an absent/deleted/evicted/cleared entry is ambiguous, so
            // ask the backend. This covers a prekey buffered just after a
            // concurrent flush already persisted its session (it would never
            // re-enter a batch) and never deletes a prekey whose session was
            // dropped before reaching the backend (which would make a redelivered
            // pkmsg permanently undecryptable). Staying under the sessions lock
            // keeps the session commit and the prekey delete atomic against a
            // decrypt buffering its own prekey (it must take this same lock to
            // store its session first), matching WAWebSignalProtocolStoreUnifiedApi
            // (bulkPutSession + bulkRemovePreKey under one lock). The buffer is
            // mutated only after each delete succeeds, so a failed flush leaves the
            // IDs for the next attempt.
            {
                let mut removed = self.removed_prekeys.lock().await;
                if !removed.is_empty() {
                    let mut deletable: Vec<u32> = Vec::new();
                    for (id, addr) in removed.iter() {
                        // Resolve to an owned decision before any await so no cache
                        // borrow is held across the backend roundtrip.
                        let durable = match state.cache.get(addr.as_ref()) {
                            Some(SessionEntry::Present(_)) => Some(true),
                            Some(SessionEntry::CheckedOut { .. }) => Some(false),
                            Some(SessionEntry::Absent) | None => None,
                        };
                        let durable = match durable {
                            Some(d) => d,
                            // Row existence is not enough: a row that does not
                            // decode is no session at all, and deleting the
                            // prekey against it is the very outcome this block
                            // exists to prevent -- a redelivered pkmsg would
                            // have neither a usable session nor the prekey to
                            // rebuild one. Decoded under the sessions lock we
                            // already hold, so the decision stays atomic
                            // against a decrypt storing its own session.
                            None => backend
                                .get_session(addr.as_ref())
                                .await?
                                .as_deref()
                                .and_then(|bytes| {
                                    Self::decode_stored_session(
                                        addr.as_ref(),
                                        bytes,
                                        &state.incarnation,
                                    )
                                })
                                .is_some(),
                        };
                        if durable {
                            deletable.push(*id);
                        }
                    }
                    for id in &deletable {
                        backend.remove_prekey(*id).await?;
                    }
                    for id in &deletable {
                        removed.remove(id);
                    }
                }
            }
        }

        // Flush identities
        {
            let mut state = self.identities.lock().await;
            let dirty_keys: Vec<_> = state.dirty.iter().cloned().collect();
            let deleted_keys: Vec<_> = state.deleted.iter().cloned().collect();

            let mut batch: Vec<(Arc<str>, [u8; 32])> = Vec::new();
            for address in &dirty_keys {
                if let Some(Some(data)) = state.cache.get(address.as_ref()) {
                    let key: [u8; 32] = data.as_ref().try_into().map_err(|_| {
                        anyhow::anyhow!(
                            "Corrupted identity key for {address}: expected 32 bytes, got {}",
                            data.len()
                        )
                    })?;
                    batch.push((address.clone(), key));
                }
            }
            if !batch.is_empty() {
                backend.put_identities_batch(&batch).await?;
            }
            for address in &deleted_keys {
                backend.delete_identity(address).await?;
            }

            for key in &dirty_keys {
                state.dirty.remove(key);
            }
            for key in &deleted_keys {
                state.deleted.remove(key);
            }
            state.evict_if_needed(self.max_entries);
        }

        // Flush sender keys
        {
            let mut state = self.sender_keys.lock().await;
            let incarnation = state.incarnation;
            let dirty_keys: Vec<_> = state.dirty.iter().cloned().collect();

            let mut batch: Vec<(Arc<str>, bytes::Bytes)> = Vec::new();
            for name in &dirty_keys {
                match state.cache.get(name.as_ref()) {
                    Some(Some(record)) => {
                        let bytes = record
                            .serialize_for_store(&incarnation)
                            .map_err(|e| anyhow::anyhow!("sender key serialize for {name}: {e}"))?;
                        batch.push((name.clone(), bytes::Bytes::from(bytes)));
                    }
                    Some(None) => {
                        backend.delete_sender_key(name).await?;
                        state.wire_gate_pending.remove(name);
                    }
                    None => {}
                }
            }
            if !batch.is_empty() {
                backend.put_sender_keys_batch(&batch).await?;
                for (name, _) in &batch {
                    state.wire_gate_pending.remove(name);
                }
            }

            for key in &dirty_keys {
                state.dirty.remove(key);
            }
            state.evict_if_needed(self.max_entries);
        }

        Ok(())
    }

    /// Whether an outbound ciphertext produced since the last flush is still
    /// gated on durability because a session or sender-key counter lease was
    /// raised and has not reached the backend. The send path flushes
    /// synchronously only while this holds;
    /// everything else (decrypt advances, identities) safely rides the
    /// coalesced write-behind.
    ///
    /// Answered from the gates' flags so the common (open) case does not take
    /// either store lock, which a flush holds across its backend I/O.
    pub async fn needs_pre_wire_flush(&self) -> bool {
        if self.wire_gates_raised() {
            return true;
        }
        // A restore that could not take the sessions lock is still holding its
        // record, and with it any lease that record raised. Only the drain in
        // `lock_sessions` moves it into the gate, so a queued restore is the
        // one state the flags cannot describe.
        if self.has_pending_session_restores.load(Ordering::Acquire) {
            drop(self.lock_sessions().await);
            return self.wire_gates_raised();
        }
        false
    }

    /// `Acquire` pairs with the gates' `Release` publish: observing a lowered
    /// flag also observes the set mutation that lowered it. A `Relaxed` load
    /// would let a send read "no lease pending" against a set another task has
    /// not finished emptying, which is the unsafe direction.
    fn wire_gates_raised(&self) -> bool {
        self.session_wire_gate.load(Ordering::Acquire)
            || self.sender_key_wire_gate.load(Ordering::Acquire)
    }

    /// Entry counts and estimated retained bytes for each store
    /// (sessions, identities, sender_keys). Sizes use the records' encoded-size
    /// proxy (see `SessionRecord::estimated_size`); on-demand only — walks the
    /// caches under their locks.
    ///
    /// Session entry counts include negative (`Absent`) and checked-out slots
    /// — they occupy the map. Byte totals include the key length for every
    /// slot, but the estimated record payload only for `Present` entries.
    pub async fn memory_stats(
        &self,
    ) -> (
        crate::stats::CollectionStats,
        crate::stats::CollectionStats,
        crate::stats::CollectionStats,
    ) {
        use crate::stats::CollectionStats;

        // Sizing a record walks its whole protobuf tree, and these mutexes
        // serialize the Signal encrypt/decrypt path — so only key lengths and
        // Arc refcount bumps happen under the locks; the estimated_size walks
        // run after each guard drops. Identities are raw bytes (len is free)
        // and stay fully under their lock.
        let (session_count, session_keys_len, session_recs): (u64, usize, Vec<_>) = {
            let s = self.lock_sessions().await;
            let mut keys_len = 0usize;
            let recs = s
                .cache
                .iter()
                .filter_map(|(k, v)| {
                    keys_len += k.len();
                    match v {
                        SessionEntry::Present(rec) => Some(rec.clone()),
                        SessionEntry::Absent | SessionEntry::CheckedOut { .. } => None,
                    }
                })
                .collect();
            keys_len += s.cache.overhead_bytes();
            (s.cache.len() as u64, keys_len, recs)
        };
        let session_bytes: usize = session_keys_len
            + session_recs
                .iter()
                .map(|r| r.estimated_size())
                .sum::<usize>();
        let sessions = CollectionStats::new(session_count, session_bytes as u64);

        let identities = {
            let i = self.identities.lock().await;
            let bytes: usize = i
                .cache
                .iter()
                .map(|(k, v)| k.len() + v.as_ref().map_or(0, |b| b.len()))
                .sum::<usize>()
                + i.cache.overhead_bytes();
            CollectionStats::new(i.cache.len() as u64, bytes as u64)
        };

        let (sk_count, sk_keys_len, sk_pending_bytes, sk_recs): (u64, usize, usize, Vec<_>) = {
            let sk = self.sender_keys.lock().await;
            let mut keys_len = 0usize;
            let recs = sk
                .cache
                .iter()
                .filter_map(|(k, v)| {
                    keys_len += k.len();
                    v.clone()
                })
                .collect();
            let pending_bytes = sk
                .pending_distributions
                .values()
                .map(|distribution| distribution.len())
                .sum();
            let (pending_only_count, pending_only_key_bytes) = sk
                .pending_distributions
                .keys()
                .filter(|key| !sk.cache.contains_key(key.as_ref()))
                .fold((0usize, 0usize), |(count, bytes), key| {
                    (count + 1, bytes + key.len())
                });
            keys_len += pending_only_key_bytes + sk.cache.overhead_bytes();
            (
                (sk.cache.len() + pending_only_count) as u64,
                keys_len,
                pending_bytes,
                recs,
            )
        };
        let sk_bytes: usize = sk_keys_len
            + sk_pending_bytes
            + sk_recs.iter().map(|r| r.estimated_size()).sum::<usize>();
        let sender_keys = CollectionStats::new(sk_count, sk_bytes as u64);

        (sessions, identities, sender_keys)
    }

    /// A lossy discard must invalidate exact-reload trust.
    pub async fn clear(&self) {
        self.clear_with_incarnation(new_store_incarnation()).await;
    }

    async fn clear_with_incarnation(&self, incarnation: StoreIncarnation) {
        self.session_recovery_generation
            .fetch_add(1, Ordering::AcqRel);
        {
            let mut sessions = self.sessions.lock().await;
            let mut pending = self.pending_session_restores();
            let generation = self.session_recovery_generation.load(Ordering::Acquire);
            pending.clear();
            self.has_pending_session_restores
                .store(false, Ordering::Release);
            sessions.discard(incarnation, generation);
        }
        self.identities.lock().await.clear();
        self.sender_keys.lock().await.discard(incarnation);
        self.has_pending_sender_key_distributions
            .store(false, Ordering::Release);
        // Drop buffered prekey removals together with the volatile sessions they
        // belong to: the promoted session is gone, so the still-durable prekey
        // must stay so a redelivered pkmsg can rebuild the session.
        self.removed_prekeys.lock().await.clear();
    }

    /// Only a discard can make a post-flush write's stale snapshot reloadable.
    #[doc(hidden)]
    pub async fn clear_after_flush(&self) {
        let mut sessions = self.lock_sessions().await;
        if sessions.dirty.is_empty()
            && sessions.deleted.is_empty()
            && sessions.reservation_pending.is_empty()
        {
            sessions.clear_clean_entries();
            if sessions.cache.is_empty() {
                self.removed_prekeys.lock().await.clear();
            }
        }
        drop(sessions);

        let mut identities = self.identities.lock().await;
        if identities.dirty.is_empty() && identities.deleted.is_empty() {
            identities.clear();
        }
        drop(identities);

        let mut sender_keys = self.sender_keys.lock().await;
        if sender_keys.dirty.is_empty()
            && sender_keys.wire_gate_pending.is_empty()
            && sender_keys.pending_distributions.is_empty()
        {
            sender_keys.clear();
            self.has_pending_sender_key_distributions
                .store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod sender_key_lock_tests {
    use super::*;
    use crate::libsignal::store::sender_key_name::SenderKeyName;
    use crate::store::error::Result as StoreResult;
    use bytes::Bytes;

    struct BlockingSessionLookup {
        started: async_lock::Barrier,
        release: async_lock::Barrier,
    }

    impl BlockingSessionLookup {
        fn new() -> Self {
            Self {
                started: async_lock::Barrier::new(2),
                release: async_lock::Barrier::new(2),
            }
        }
    }

    #[async_trait::async_trait]
    impl SignalStore for BlockingSessionLookup {
        async fn put_identity(&self, _: &str, _: [u8; 32]) -> StoreResult<()> {
            unreachable!()
        }

        async fn load_identity(&self, _: &str) -> StoreResult<Option<[u8; 32]>> {
            unreachable!()
        }

        async fn delete_identity(&self, _: &str) -> StoreResult<()> {
            unreachable!()
        }

        async fn get_session(&self, _: &str) -> StoreResult<Option<Bytes>> {
            self.started.wait().await;
            self.release.wait().await;
            Ok(None)
        }

        async fn has_session(&self, _: &str) -> StoreResult<bool> {
            self.started.wait().await;
            self.release.wait().await;
            Ok(false)
        }

        async fn put_session(&self, _: &str, _: &[u8]) -> StoreResult<()> {
            unreachable!()
        }

        async fn delete_session(&self, _: &str) -> StoreResult<()> {
            unreachable!()
        }

        async fn store_prekey(&self, _: u32, _: &[u8], _: bool) -> StoreResult<()> {
            unreachable!()
        }

        async fn load_prekey(&self, _: u32) -> StoreResult<Option<Bytes>> {
            unreachable!()
        }

        async fn mark_prekeys_uploaded(&self, _: &[u32]) -> StoreResult<()> {
            unreachable!()
        }

        async fn remove_prekey(&self, _: u32) -> StoreResult<()> {
            unreachable!()
        }

        async fn get_max_prekey_id(&self) -> StoreResult<u32> {
            unreachable!()
        }

        async fn store_signed_prekey(&self, _: u32, _: &[u8]) -> StoreResult<()> {
            unreachable!()
        }

        async fn load_signed_prekey(&self, _: u32) -> StoreResult<Option<Vec<u8>>> {
            unreachable!()
        }

        async fn load_all_signed_prekeys(&self) -> StoreResult<Vec<(u32, Vec<u8>)>> {
            unreachable!()
        }

        async fn remove_signed_prekey(&self, _: u32) -> StoreResult<()> {
            unreachable!()
        }

        async fn put_sender_key(&self, _: &str, _: &[u8]) -> StoreResult<()> {
            unreachable!()
        }

        async fn get_sender_key(&self, _: &str) -> StoreResult<Option<Vec<u8>>> {
            unreachable!()
        }

        async fn delete_sender_key(&self, _: &str) -> StoreResult<()> {
            unreachable!()
        }
    }

    /// Sender-key backend whose read parks until every expected reader has
    /// arrived, so a test can prove that N cold readers reach it concurrently
    /// rather than queueing on the global cache mutex.
    struct GatedSenderKeyLookup {
        arrived: async_lock::Barrier,
        release: async_lock::Barrier,
        hits: std::sync::atomic::AtomicUsize,
        gated_reads: usize,
        payload: SyncMutex<Option<Vec<u8>>>,
    }

    impl GatedSenderKeyLookup {
        fn new(readers: usize, payload: Option<Vec<u8>>) -> Self {
            Self::with_rounds(readers, readers, payload)
        }

        /// `readers` sizes the rendezvous (plus the driving test task);
        /// `gated_reads` is how many backend calls park on it, which is a
        /// different number when one reader is made to read repeatedly.
        fn with_rounds(readers: usize, gated_reads: usize, payload: Option<Vec<u8>>) -> Self {
            Self {
                arrived: async_lock::Barrier::new(readers + 1),
                release: async_lock::Barrier::new(readers + 1),
                hits: std::sync::atomic::AtomicUsize::new(0),
                gated_reads,
                payload: SyncMutex::new(payload),
            }
        }

        fn hits(&self) -> usize {
            self.hits.load(Ordering::Relaxed)
        }

        /// Model the flush that makes a concurrent write durable, so a reader
        /// forced to read again sees what a real backend would now hold.
        fn set_payload(&self, payload: Option<Vec<u8>>) {
            *self.payload.lock().unwrap_or_else(|p| p.into_inner()) = payload;
        }
    }

    #[async_trait::async_trait]
    impl SignalStore for GatedSenderKeyLookup {
        async fn get_sender_key(&self, _: &str) -> StoreResult<Option<Vec<u8>>> {
            // Only the first round is gated. A reader whose install loses the
            // epoch check reads again, and that retry must not wait on a
            // rendezvous the test has already passed through.
            let hit = self.hits.fetch_add(1, Ordering::Relaxed);
            // Sampled before parking: these bytes belong to the moment the read
            // started, so a write landing while we are gated is invisible here
            // and visible to the next call, as a real backend behaves.
            let sampled = self
                .payload
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            if hit < self.gated_reads {
                self.arrived.wait().await;
                self.release.wait().await;
            }
            Ok(sampled)
        }

        async fn put_identity(&self, _: &str, _: [u8; 32]) -> StoreResult<()> {
            unreachable!()
        }
        async fn load_identity(&self, _: &str) -> StoreResult<Option<[u8; 32]>> {
            unreachable!()
        }
        async fn delete_identity(&self, _: &str) -> StoreResult<()> {
            unreachable!()
        }
        async fn get_session(&self, _: &str) -> StoreResult<Option<Bytes>> {
            unreachable!()
        }
        async fn put_session(&self, _: &str, _: &[u8]) -> StoreResult<()> {
            unreachable!()
        }
        async fn delete_session(&self, _: &str) -> StoreResult<()> {
            unreachable!()
        }
        async fn store_prekey(&self, _: u32, _: &[u8], _: bool) -> StoreResult<()> {
            unreachable!()
        }
        async fn load_prekey(&self, _: u32) -> StoreResult<Option<Bytes>> {
            unreachable!()
        }
        async fn mark_prekeys_uploaded(&self, _: &[u32]) -> StoreResult<()> {
            unreachable!()
        }
        async fn remove_prekey(&self, _: u32) -> StoreResult<()> {
            unreachable!()
        }
        async fn get_max_prekey_id(&self) -> StoreResult<u32> {
            unreachable!()
        }
        async fn store_signed_prekey(&self, _: u32, _: &[u8]) -> StoreResult<()> {
            unreachable!()
        }
        async fn load_signed_prekey(&self, _: u32) -> StoreResult<Option<Vec<u8>>> {
            unreachable!()
        }
        async fn load_all_signed_prekeys(&self) -> StoreResult<Vec<(u32, Vec<u8>)>> {
            unreachable!()
        }
        async fn remove_signed_prekey(&self, _: u32) -> StoreResult<()> {
            unreachable!()
        }
        /// A flush writes through here, so the payload a later read samples is
        /// the one the flush persisted, as a real backend behaves.
        async fn put_sender_key(&self, _: &str, record: &[u8]) -> StoreResult<()> {
            self.set_payload(Some(record.to_vec()));
            Ok(())
        }
        async fn delete_sender_key(&self, _: &str) -> StoreResult<()> {
            self.set_payload(None);
            Ok(())
        }
    }

    fn sender_key_record_with_chain(chain_id: u32) -> SenderKeyRecord {
        use crate::libsignal::protocol::KeyPair;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let kp = KeyPair::generate(&mut rng);
        let mut record = SenderKeyRecord::new_empty();
        record
            .add_sender_key_state(
                3,
                chain_id,
                0,
                &[7u8; 32],
                kp.public_key,
                Some(kp.private_key),
            )
            .expect("valid sender key state");
        record
    }

    fn chain_id_of(record: &SenderKeyRecord) -> u32 {
        record
            .sender_key_state()
            .expect("record must carry a state")
            .chain_id()
    }

    #[tokio::test]
    async fn cold_sender_key_miss_loads_from_the_backend() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let name = SenderKeyName::from_parts("19995550001@g.us", "19995550002@s.whatsapp.net:0");
        let stored = sender_key_record_with_chain(7);
        backend
            .put_sender_key(
                name.cache_key(),
                &stored.serialize().expect("serialize record"),
            )
            .await
            .expect("seed backend");

        let loaded = cache
            .get_sender_key(&name, &backend)
            .await
            .expect("cold load")
            .expect("record present");
        assert_eq!(chain_id_of(&loaded), 7);

        // Second read is a cache hit and must agree with the first.
        let warm = cache
            .get_sender_key(&name, &backend)
            .await
            .expect("warm load")
            .expect("record present");
        assert_eq!(chain_id_of(&warm), 7);
    }

    #[tokio::test]
    async fn concurrent_cold_sender_key_readers_share_one_cached_value() {
        let cache = Arc::new(SignalStoreCache::new());
        let stored = sender_key_record_with_chain(11);
        let backend = Arc::new(GatedSenderKeyLookup::new(
            2,
            Some(stored.serialize().expect("serialize record")),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550003@g.us",
            "19995550004@s.whatsapp.net:0",
        ));

        let readers: Vec<_> = (0..2)
            .map(|_| {
                let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
                tokio::spawn(async move { cache.get_sender_key(&name, &*backend).await })
            })
            .collect();

        // Both readers must reach the backend: with the lock held across the
        // round-trip the second would still be queued and this would not
        // rendezvous.
        backend.arrived.wait().await;
        backend.release.wait().await;

        for reader in readers {
            let record = reader
                .await
                .expect("reader task")
                .expect("cold load")
                .expect("record present");
            assert_eq!(chain_id_of(&record), 11);
        }
        assert_eq!(backend.hits(), 2, "both readers should consult the backend");

        let cached = cache
            .get_sender_key(&name, &*backend)
            .await
            .expect("warm load")
            .expect("record present");
        assert_eq!(chain_id_of(&cached), 11, "cache must hold a single value");
    }

    #[tokio::test]
    async fn a_late_sender_key_reader_does_not_overwrite_a_concurrent_put() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::new(
            1,
            Some(
                sender_key_record_with_chain(1)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550005@g.us",
            "19995550006@s.whatsapp.net:0",
        ));

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });

        backend.arrived.wait().await;
        // The reader is parked inside the backend: a writer must be able to
        // take the cache mutex right now, and must win the re-check.
        cache
            .put_sender_key(&name, sender_key_record_with_chain(2))
            .await;
        backend.release.wait().await;

        let observed = reader
            .await
            .expect("reader task")
            .expect("cold load")
            .expect("record present");
        assert_eq!(chain_id_of(&observed), 2, "reader must yield to the writer");

        let cached = cache
            .get_sender_key(&name, &*backend)
            .await
            .expect("warm load")
            .expect("record present");
        assert_eq!(chain_id_of(&cached), 2, "stale backend read must not land");
    }

    #[tokio::test]
    async fn a_late_sender_key_reader_does_not_resurrect_a_concurrent_delete() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::new(
            1,
            Some(
                sender_key_record_with_chain(3)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550007@g.us",
            "19995550008@s.whatsapp.net:0",
        ));

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });

        backend.arrived.wait().await;
        cache.delete_sender_key(name.cache_key()).await;
        backend.release.wait().await;

        assert!(
            reader
                .await
                .expect("reader task")
                .expect("cold load")
                .is_none(),
            "reader must observe the delete, not the row it read before it"
        );
        assert!(
            cache
                .get_sender_key(&name, &*backend)
                .await
                .expect("warm load")
                .is_none(),
            "the tombstone must survive the late reader"
        );
    }

    /// The dangerous shape behind a re-check that only looks for an entry: a
    /// newer record is written, flushed and then dropped by a clean removal
    /// while a cold read is in flight. The slot is absent again at re-check,
    /// but the reader's bytes predate the write, and a clean removal keeps the
    /// incarnation, so installing them would be trusted as an exact reload and
    /// could resume an already-published iteration.
    #[tokio::test]
    async fn a_write_dropped_by_eviction_is_not_replaced_by_the_stale_read() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::new(
            1,
            Some(
                sender_key_record_with_chain(1)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550009@g.us",
            "19995550010@s.whatsapp.net:0",
        ));

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });

        backend.arrived.wait().await;
        // A newer chain lands, is flushed (so the backend now holds it), then
        // leaves the cache entirely as a clean entry.
        cache
            .put_sender_key(&name, sender_key_record_with_chain(2))
            .await;
        backend.set_payload(Some(
            sender_key_record_with_chain(2)
                .serialize()
                .expect("serialize record"),
        ));
        cache.drop_clean_sender_key_for_test(name.cache_key()).await;
        backend.release.wait().await;

        let observed = reader
            .await
            .expect("reader task")
            .expect("cold load")
            .expect("record present");
        assert_eq!(
            chain_id_of(&observed),
            2,
            "the pre-write bytes must not survive a removal that happened after them"
        );
        let cached = cache
            .get_sender_key(&name, &*backend)
            .await
            .expect("warm load")
            .expect("record present");
        assert_eq!(chain_id_of(&cached), 2, "a stale record must not be cached");
    }

    /// A cold read that is cancelled mid-backend must leave no bookkeeping
    /// behind: the removal window is fixed-size and reader-agnostic, so a
    /// dropped future cannot strand state that would pin its chain to the slow
    /// path or grow the cache.
    #[tokio::test]
    async fn a_cancelled_cold_read_leaves_no_bookkeeping() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::with_rounds(
            1,
            1,
            Some(
                sender_key_record_with_chain(4)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550017@g.us",
            "19995550018@s.whatsapp.net:0",
        ));

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });
        backend.arrived.wait().await;
        // Drop the future while it is parked inside the backend.
        reader.abort();
        let _ = reader.await;

        // A fresh read of the same chain must take the unlocked path and
        // install on its first attempt, exactly as if nothing had happened.
        let hits_before = backend.hits();
        let observed = cache
            .get_sender_key(&name, &*backend)
            .await
            .expect("cold load")
            .expect("record present");
        assert_eq!(chain_id_of(&observed), 4);
        assert_eq!(
            backend.hits() - hits_before,
            1,
            "a cancelled read must not push later reads onto the retry path"
        );
    }

    /// The keyed removal path is what the other race tests drive. `clear` and
    /// `retain` cannot name the keys they drop, so they take a separate branch
    /// that concedes every in-flight reader — and that is the branch where a
    /// missing bump would let pre-write bytes be adopted as an exact reload.
    /// Drives it through the real flush-then-`clear_after_flush` sequence.
    #[tokio::test]
    async fn a_write_dropped_by_clear_after_flush_is_not_replaced_by_the_stale_read() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::new(
            1,
            Some(
                sender_key_record_with_chain(1)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550019@g.us",
            "19995550020@s.whatsapp.net:0",
        ));

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });

        backend.arrived.wait().await;
        // A newer chain lands, is flushed (which writes it through to the
        // backend), and teardown then drops it from the cache — the opaque
        // removal path, not the keyed one.
        cache
            .put_sender_key(&name, sender_key_record_with_chain(2))
            .await;
        cache.flush(&*backend).await.expect("flush");
        cache.clear_after_flush().await;
        backend.release.wait().await;

        let observed = reader
            .await
            .expect("reader task")
            .expect("cold load")
            .expect("record present");
        assert_eq!(
            chain_id_of(&observed),
            2,
            "an unnamed removal must still reject bytes that predate the write"
        );
    }

    /// The removal signal is per key: churn on other chains, which is the
    /// normal state of a cache at its eviction watermark, must not cost an
    /// unrelated cold reader its unlocked install.
    #[tokio::test]
    async fn churn_on_other_chains_does_not_force_a_reread() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::new(
            1,
            Some(
                sender_key_record_with_chain(5)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550013@g.us",
            "19995550014@s.whatsapp.net:0",
        ));
        let other = SenderKeyName::from_parts("19995550015@g.us", "19995550016@s.whatsapp.net:0");

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });

        backend.arrived.wait().await;
        // A whole write-and-drop cycle on a different chain.
        cache
            .put_sender_key(&other, sender_key_record_with_chain(9))
            .await;
        cache
            .drop_clean_sender_key_for_test(other.cache_key())
            .await;
        backend.release.wait().await;

        let observed = reader
            .await
            .expect("reader task")
            .expect("cold load")
            .expect("record present");
        assert_eq!(chain_id_of(&observed), 5);
        assert_eq!(
            backend.hits(),
            1,
            "an unrelated chain's removal must not invalidate this read"
        );
    }

    /// Losing the epoch check on every unlocked attempt drops through to the
    /// read taken under the lock, which cannot be raced. That path installs
    /// without an epoch check precisely because nothing can intervene, so it
    /// needs its own coverage rather than inheriting the loop's.
    #[tokio::test]
    async fn a_read_that_loses_every_race_falls_back_to_the_locked_path() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(GatedSenderKeyLookup::with_rounds(
            1,
            // Gate exactly the unlocked attempts; the locked fallback then runs
            // ungated, as a real backend would.
            SENDER_KEY_UNLOCKED_READ_ATTEMPTS,
            Some(
                sender_key_record_with_chain(1)
                    .serialize()
                    .expect("serialize record"),
            ),
        ));
        let name = Arc::new(SenderKeyName::from_parts(
            "19995550011@g.us",
            "19995550012@s.whatsapp.net:0",
        ));

        let reader = tokio::spawn({
            let (cache, backend, name) = (cache.clone(), backend.clone(), name.clone());
            async move { cache.get_sender_key(&name, &*backend).await }
        });

        // Invalidate this key on every attempt, so no unlocked install survives.
        for chain in 2..=(SENDER_KEY_UNLOCKED_READ_ATTEMPTS as u32 + 1) {
            backend.arrived.wait().await;
            cache
                .put_sender_key(&name, sender_key_record_with_chain(chain))
                .await;
            backend.set_payload(Some(
                sender_key_record_with_chain(chain)
                    .serialize()
                    .expect("serialize record"),
            ));
            cache.drop_clean_sender_key_for_test(name.cache_key()).await;
            backend.release.wait().await;
        }

        let observed = reader
            .await
            .expect("reader task")
            .expect("cold load")
            .expect("record present");
        let latest = SENDER_KEY_UNLOCKED_READ_ATTEMPTS as u32 + 1;
        assert_eq!(
            chain_id_of(&observed),
            latest,
            "the locked fallback must return the current record"
        );
        assert!(
            backend.hits() > SENDER_KEY_UNLOCKED_READ_ATTEMPTS,
            "the locked fallback must have read the backend itself"
        );
    }

    #[tokio::test]
    async fn the_user_index_never_misses_state_across_the_mutation_paths() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();

        // Every public path that can put an address into either scanned store
        // must leave the user answerable without a backend probe.
        let session_user = "19995551001";
        let session_addr =
            ProtocolAddress::new(&format!("{session_user}@s.whatsapp.net"), 0.into());
        cache
            .put_session(&session_addr, SessionRecord::new_fresh())
            .await;
        assert!(
            cache
                .has_state_for_user(session_user, &backend)
                .await
                .unwrap()
        );

        let deleted_user = "19995551002";
        let deleted_addr =
            ProtocolAddress::new(&format!("{deleted_user}@s.whatsapp.net"), 0.into());
        cache.delete_session(&deleted_addr).await;
        assert!(
            cache
                .has_state_for_user(deleted_user, &backend)
                .await
                .unwrap()
        );

        let identity_user = "19995551003";
        let identity_addr =
            ProtocolAddress::new(&format!("{identity_user}@s.whatsapp.net"), 0.into());
        cache.put_identity(&identity_addr, &[3u8; 32]).await;
        assert!(
            cache
                .has_state_for_user(identity_user, &backend)
                .await
                .unwrap()
        );

        let probed_user = "19995551004";
        let probed_addr = ProtocolAddress::new(&format!("{probed_user}@s.whatsapp.net"), 0.into());
        cache.has_session(&probed_addr, &backend).await.unwrap();
        assert!(
            cache
                .has_state_for_user(probed_user, &backend)
                .await
                .unwrap()
        );

        let checked_out_user = "19995551005";
        let checked_out_addr =
            ProtocolAddress::new(&format!("{checked_out_user}@s.whatsapp.net"), 0.into());
        let (_, checkout) = cache
            .checkout_session(&checked_out_addr, &backend)
            .await
            .unwrap();
        assert!(
            cache
                .has_state_for_user(checked_out_user, &backend)
                .await
                .unwrap()
        );
        cache.cancel_session_checkout(&checked_out_addr, checkout);
        assert!(
            cache
                .has_state_for_user(checked_out_user, &backend)
                .await
                .unwrap()
        );

        // A user with no state anywhere still answers false.
        assert!(
            !cache
                .has_state_for_user("19995559999", &backend)
                .await
                .unwrap()
        );

        // An addressed-device JID renders as `user:device@server.N`, so both
        // `user` and `user:device` prefix-match it under the scan predicate.
        // Only the first is an index key; the second must be conceded rather
        // than denied, which is the one false negative available here.
        let device_addr = ProtocolAddress::new("19995551006:5@c.us", 0.into());
        cache
            .put_session(&device_addr, SessionRecord::new_fresh())
            .await;
        assert!(
            cache
                .has_state_for_user("19995551006", &backend)
                .await
                .unwrap()
        );

        let with_device = "19995551006:5";
        assert!(
            protocol_address_matches_user(device_addr.as_str(), with_device),
            "the scan predicate matches this user, so the index must not deny it"
        );
        assert!(
            cache
                .has_state_for_user(with_device, &backend)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn the_user_index_survives_eviction_and_defers_to_the_backend_when_cold() {
        let cache = SignalStoreCache::with_max_entries(4);
        let backend = crate::store::in_memory::InMemoryBackend::new();

        // A dirty entry is never evicted, so this user must stay answerable
        // from the index no matter how much churn follows.
        let pinned_user = "19995552000";
        let pinned_addr = ProtocolAddress::new(&format!("{pinned_user}@s.whatsapp.net"), 0.into());
        cache
            .put_session(&pinned_addr, SessionRecord::new_fresh())
            .await;

        // A user whose only entry is clean, so eviction can drop it. Give it
        // durable state so the probe has something truthful to report.
        let evicted_user = "19995552001";
        let evicted_addr =
            ProtocolAddress::new(&format!("{evicted_user}@s.whatsapp.net"), 0.into());
        backend
            .put_identity(evicted_addr.as_str(), [9u8; 32])
            .await
            .unwrap();
        cache.has_session(&evicted_addr, &backend).await.unwrap();

        // Churn well past the high watermark so eviction and index compaction
        // both run repeatedly.
        for i in 100..400 {
            let addr = ProtocolAddress::new(&format!("1999555{i:04}@s.whatsapp.net"), 0.into());
            cache.has_session(&addr, &backend).await.unwrap();
        }

        assert!(
            cache
                .has_state_for_user(pinned_user, &backend)
                .await
                .unwrap(),
            "a still-cached user must stay answerable from the index"
        );
        assert!(
            cache
                .has_state_for_user(evicted_user, &backend)
                .await
                .unwrap(),
            "an evicted user's durable state must still be found via the probe"
        );
        assert!(
            !cache
                .has_state_for_user("19995559998", &backend)
                .await
                .unwrap(),
            "a user with no state anywhere must answer false"
        );

        // A lossy reset drops the index with the cache; the probe is then the
        // only source of truth, and must still find durable state.
        cache.clear().await;
        assert!(
            cache
                .has_state_for_user(evicted_user, &backend)
                .await
                .unwrap(),
            "durable state must be found through the probe with a cold index"
        );
    }

    async fn wait_for_lock_waiter(lock: &Arc<Mutex<()>>, baseline: usize) {
        for _ in 0..10_000 {
            if Arc::strong_count(lock) > baseline {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("task did not reach the contested lock");
    }

    #[tokio::test]
    async fn same_name_shares_one_lock() {
        let cache = SignalStoreCache::new();
        let a = SenderKeyName::from_parts("g1@g.us", "u1@s.whatsapp.net:0");
        let b = SenderKeyName::from_parts("g2@g.us", "u1@s.whatsapp.net:0");

        let l1 = cache.sender_key_lock(&a).await;
        let l2 = cache.sender_key_lock(&a).await;
        let l3 = cache.sender_key_lock(&b).await;

        assert!(Arc::ptr_eq(&l1, &l2), "same name must share one lock");
        assert!(!Arc::ptr_eq(&l1, &l3), "different names must not share");
    }

    #[tokio::test]
    async fn same_name_lock_is_mutually_exclusive() {
        let cache = SignalStoreCache::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");
        let lock = cache.sender_key_lock(&name).await;

        let guard = lock.lock().await;
        assert!(
            lock.try_lock().is_none(),
            "held lock must block a second acquire"
        );
        drop(guard);
        assert!(lock.try_lock().is_some(), "released lock must reacquire");
    }

    #[tokio::test]
    async fn delete_waits_for_the_chain_lock() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");
        cache
            .put_sender_key(&name, SenderKeyRecord::new_empty())
            .await;

        let lock = cache.sender_key_lock(&name).await;
        let held = lock.lock().await;
        let lock_refs = Arc::strong_count(&lock);
        let started = Arc::new(async_lock::Barrier::new(2));
        let task = tokio::spawn({
            let cache = cache.clone();
            let started = started.clone();
            let cache_key = name.cache_key().to_string();
            async move {
                started.wait().await;
                cache.delete_sender_key(&cache_key).await;
            }
        });

        started.wait().await;
        wait_for_lock_waiter(&lock, lock_refs).await;
        assert!(
            cache
                .get_sender_key(&name, &backend)
                .await
                .unwrap()
                .is_some(),
            "delete must wait for the in-flight chain mutation"
        );

        drop(held);
        task.await.expect("delete task");
        assert!(
            cache
                .get_sender_key(&name, &backend)
                .await
                .unwrap()
                .is_none(),
            "delete must run after the mutation releases the chain"
        );
    }

    #[tokio::test]
    async fn warm_sender_key_hit_shares_arc_not_deep_clone() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        cache
            .put_sender_key(&name, SenderKeyRecord::new_empty())
            .await;

        let a = cache
            .get_sender_key(&name, &backend)
            .await
            .unwrap()
            .expect("warm hit");
        let b = cache
            .get_sender_key(&name, &backend)
            .await
            .unwrap()
            .expect("warm hit");

        // A warm sender-key hit returns a refcount bump of the same allocation,
        // not a deep copy of the message-key backlog.
        assert!(Arc::ptr_eq(&a, &b));
    }

    /// The sync fast path must be indistinguishable from `put_session`:
    /// visible to reads AND marked dirty so the flush persists it.
    #[tokio::test]
    async fn try_put_session_marks_dirty_and_flushes() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550009999", 1.into());

        assert!(
            cache
                .try_put_session(&addr, SessionRecord::new_fresh())
                .is_ok(),
            "uncontended try_put_session must succeed"
        );

        assert_eq!(cache.try_has_session(&addr), Some(true));
        cache.flush(&backend).await.unwrap();
        assert!(
            SignalStore::get_session(&backend, addr.as_str())
                .await
                .unwrap()
                .is_some(),
            "flush must persist a session stored via the fast path"
        );
    }

    #[tokio::test]
    async fn try_session_paths_fall_back_under_contention() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550009999", 1.into());

        let guard = cache.sessions.lock().await;
        assert!(
            cache
                .try_put_session(&addr, SessionRecord::new_fresh())
                .is_err(),
            "held sessions lock must reject try_put_session"
        );
        assert_eq!(
            cache.try_has_session(&addr),
            None,
            "held sessions lock must reject try_has_session"
        );
        assert!(
            cache.try_checkout_session(&addr).is_none(),
            "held sessions lock must defer checkout"
        );
        drop(guard);

        assert_eq!(
            cache.try_has_session(&addr),
            None,
            "unknown entry must defer to the async path"
        );
        assert!(cache.try_checkout_session(&addr).is_none());
        assert!(
            cache
                .try_put_session(&addr, SessionRecord::new_fresh())
                .is_ok(),
            "released lock must accept try_put_session"
        );
        assert_eq!(cache.try_has_session(&addr), Some(true));
    }

    #[tokio::test]
    async fn cancelled_checkout_queues_under_contention_and_remains_flushable() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550008888", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;

        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();
        let sessions = cache.sessions.lock().await;
        let SessionCheckoutStoreResult::Pending(completion) = cache.restore_session_from_checkout(
            &addr,
            record.expect("checked-out record"),
            generation,
            true,
        ) else {
            panic!("contended restore must be queued")
        };
        drop(completion);
        assert_eq!(cache.pending_session_restores().len(), 1);
        drop(sessions);

        cache.flush(&backend).await.unwrap();
        assert!(
            SignalStore::get_session(&backend, addr.as_str())
                .await
                .unwrap()
                .is_some(),
            "a queued cancellation restore must not strand dirty state"
        );
    }

    #[tokio::test]
    async fn lossy_clear_rejects_an_older_checkout_generation() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550007777", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();

        cache.clear().await;
        assert!(matches!(
            cache.restore_session_from_checkout(
                &addr,
                record.expect("checked-out record"),
                generation,
                true,
            ),
            SessionCheckoutStoreResult::Rejected
        ));
        assert!(cache.peek_session(&addr, &backend).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn lossy_clear_invalidates_checkouts_before_waiting_for_the_cache() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550007776", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        let (record, checkout) = cache.checkout_session(&addr, &backend).await.unwrap();

        let sessions = cache.sessions.lock().await;
        let clear = tokio::spawn({
            let cache = cache.clone();
            async move { cache.clear().await }
        });
        for _ in 0..10_000 {
            if cache.session_recovery_generation.load(Ordering::Acquire) != checkout.generation() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_ne!(
            cache.session_recovery_generation.load(Ordering::Acquire),
            checkout.generation(),
            "clear must invalidate owners before waiting"
        );
        assert!(matches!(
            cache.restore_session_from_checkout(
                &addr,
                record.expect("checked-out record"),
                checkout,
                true,
            ),
            SessionCheckoutStoreResult::Rejected
        ));

        drop(sessions);
        clear.await.unwrap();
    }

    #[tokio::test]
    async fn stale_checkout_cannot_overwrite_a_new_owner() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550007775", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;

        let (old_record, old_checkout) = cache
            .try_checkout_session(&addr)
            .expect("warm checkout")
            .expect("old owner");
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        let (new_record, new_checkout) = cache
            .try_checkout_session(&addr)
            .expect("warm checkout")
            .expect("new owner");
        assert_ne!(old_checkout, new_checkout);

        assert!(matches!(
            cache.restore_session_from_checkout(
                &addr,
                old_record.expect("old record"),
                old_checkout,
                true,
            ),
            SessionCheckoutStoreResult::Rejected
        ));
        assert!(matches!(
            cache.restore_session_from_checkout(
                &addr,
                new_record.expect("new record"),
                new_checkout,
                true,
            ),
            SessionCheckoutStoreResult::Stored
        ));
    }

    #[tokio::test]
    async fn checkout_rejects_a_competing_owner() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550007770", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;

        let (record, generation) = cache
            .try_checkout_session(&addr)
            .expect("warm checkout")
            .expect("first owner");
        let error = match cache
            .try_checkout_session(&addr)
            .expect("checked-out slots are known")
        {
            Ok(_) => panic!("a second owner must be rejected"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("already checked out"));
        assert!(matches!(
            cache.restore_session_from_checkout(
                &addr,
                record.expect("first owner"),
                generation,
                true,
            ),
            SessionCheckoutStoreResult::Stored
        ));
    }

    #[tokio::test]
    async fn restore_does_not_resurrect_a_deleted_slot() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550007771", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;

        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();
        cache.delete_session(&addr).await;
        assert!(matches!(
            cache.restore_session_from_checkout(
                &addr,
                record.expect("checked-out record"),
                generation,
                true,
            ),
            SessionCheckoutStoreResult::Rejected
        ));
        assert!(cache.peek_session(&addr, &backend).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn queued_restore_does_not_overwrite_a_delete() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550007772", 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();

        let mut sessions = cache.sessions.lock().await;
        sessions.delete(addr.as_str());
        let SessionCheckoutStoreResult::Pending(completion) = cache.restore_session_from_checkout(
            &addr,
            record.expect("checked-out record"),
            generation,
            true,
        ) else {
            panic!("contended restore must be queued")
        };
        drop(sessions);

        cache.complete_session_checkout().await;
        assert!(!completion.load(Ordering::Acquire));
        assert!(cache.peek_session(&addr, &backend).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn empty_checkout_reserves_and_releases_its_slot() {
        let cache = SignalStoreCache::new();
        let backend = crate::store::in_memory::InMemoryBackend::new();
        let addr = ProtocolAddress::new("15550007773", 1.into());

        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();
        assert!(record.is_none());
        assert_eq!(cache.try_has_session(&addr), Some(false));
        assert!(!cache.has_session(&addr, &backend).await.unwrap());
        assert!(cache.checkout_session(&addr, &backend).await.is_err());
        cache.cancel_session_checkout(&addr, generation);

        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();
        assert!(record.is_none());
        let sessions = cache.sessions.lock().await;
        cache.cancel_session_checkout(&addr, generation);
        assert_eq!(cache.pending_session_restores().len(), 1);
        drop(sessions);
        cache.complete_session_checkout().await;
        assert_eq!(cache.try_has_session(&addr), Some(false));
    }

    #[tokio::test]
    async fn peek_prefers_a_cache_write_that_wins_the_backend_race() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(BlockingSessionLookup::new());
        let addr = ProtocolAddress::new("15550007774", 1.into());

        let peek = tokio::spawn({
            let cache = cache.clone();
            let backend = backend.clone();
            let addr = addr.clone();
            async move { cache.peek_session(&addr, backend.as_ref()).await }
        });
        backend.started.wait().await;
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        backend.release.wait().await;

        assert!(peek.await.unwrap().unwrap().is_some());
    }

    #[tokio::test]
    async fn existence_prefers_a_cache_write_that_wins_the_backend_race() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(BlockingSessionLookup::new());
        let addr = ProtocolAddress::new("15550007772", 2.into());

        let exists = tokio::spawn({
            let cache = cache.clone();
            let backend = backend.clone();
            let addr = addr.clone();
            async move { cache.has_session(&addr, backend.as_ref()).await }
        });
        backend.started.wait().await;
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        backend.release.wait().await;

        assert!(exists.await.unwrap().unwrap());
    }

    #[tokio::test]
    async fn try_has_session_reports_known_absent() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550009999", 1.into());

        cache.delete_session(&addr).await;
        assert_eq!(
            cache.try_has_session(&addr),
            Some(false),
            "negative-cached entry must answer synchronously"
        );
    }

    #[tokio::test]
    async fn try_identity_paths_cover_hit_miss_and_contention() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550009999", 1.into());
        let key_bytes = [7u8; 32];

        assert_eq!(
            cache.try_get_identity(&addr),
            None,
            "unknown entry must defer to the async path"
        );

        assert!(cache.try_put_identity(&addr, &key_bytes));
        match cache.try_get_identity(&addr) {
            Some(Some(bytes)) => assert_eq!(bytes.as_ref(), &key_bytes),
            other => panic!("expected cached identity, got {other:?}"),
        }

        let guard = cache.identities.lock().await;
        assert_eq!(cache.try_get_identity(&addr), None);
        assert!(!cache.try_put_identity(&addr, &key_bytes));
        drop(guard);

        cache.delete_identity(&addr).await;
        assert_eq!(
            cache.try_get_identity(&addr),
            Some(None),
            "known-absent identity must answer synchronously"
        );
    }
}

#[cfg(test)]
mod consumed_prekey_atomicity_tests {
    use super::*;
    use crate::store::in_memory::InMemoryBackend;
    use crate::store::traits::SignalStore;

    const PREKEY_ID: u32 = 4242;

    /// Seed a durable prekey in the backend and return the address the inbound
    /// pkmsg promotes a session for.
    async fn seed(backend: &InMemoryBackend) -> ProtocolAddress {
        backend
            .store_prekey(PREKEY_ID, b"durable-prekey", false)
            .await
            .unwrap();
        ProtocolAddress::new("bob", 1.into())
    }

    /// The inbound pkmsg decrypt promotes the session into the volatile cache and
    /// then "removes" the consumed prekey. The removal must NOT touch the backend
    /// until the session-bearing flush runs, so a crash in the window between
    /// decrypt and flush can never leave the prekey durably deleted while its new
    /// session is still only in memory.
    #[tokio::test]
    async fn consumed_prekey_stays_durable_until_session_flush() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        // Decrypt path: session into cache (volatile), prekey buffered for removal.
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID, addr.as_str()).await;

        // Pre-flush invariant: the prekey is still durable in the backend, so even
        // if everything volatile is lost the redelivered pkmsg can rebuild.
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "consumed prekey must remain in the backend until the session flush"
        );
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_none(),
            "session is only volatile before flush"
        );

        // Flush commits the session AND the prekey deletion together.
        cache.flush(&backend).await.unwrap();

        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_some(),
            "session must be durable after flush"
        );
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_none(),
            "prekey must be deleted once the session it produced is durable"
        );
    }

    /// If a dirty (promoted-but-not-yet-durable) session is checked out by a
    /// concurrent reader at flush time, the flush cannot persist it, so the consumed
    /// prekey must be DEFERRED rather than deleted. Deleting it here would recreate
    /// the crash-orphan window. A later flush, once the session is back and durable,
    /// commits both.
    #[tokio::test]
    async fn checked_out_session_defers_prekey_delete_until_durable() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        // Decrypt path: session promoted (dirty, volatile) + prekey buffered.
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID, addr.as_str()).await;

        // A concurrent reader checks the session out after the per-address lock was
        // released (get_session leaves a CheckedOut marker; the dirty bit stays).
        let taken = cache.get_session(&addr, &backend).await.unwrap();
        assert!(taken.is_some(), "the promoted session should be readable");

        // Flush while the session is checked out: it cannot be persisted, so the
        // prekey must NOT be deleted.
        cache.flush(&backend).await.unwrap();
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_none(),
            "a checked-out session is not persisted by this flush"
        );
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "prekey must not be deleted while its session is checked out (still volatile)"
        );

        // The reader returns the session; a later flush persists it and now commits
        // the deferred prekey deletion.
        cache.put_session(&addr, taken.unwrap()).await;
        cache.flush(&backend).await.unwrap();
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_some(),
            "session is durable after the reader returned it"
        );
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_none(),
            "the deferred prekey deletion commits once the session is durable"
        );
    }

    /// One flush carrying two consumed prekeys must delete each one on its OWN
    /// session's durability, not gate them together. Session A is persisted by
    /// this flush, so A's prekey is deleted now; session B is checked out (still
    /// volatile), so only B's prekey is deferred. A coarse "defer all if any
    /// session is checked out" gate would leave A's prekey buffered, and a later
    /// clear() would then drop it while A's session stays live, leaking the
    /// one-time prekey forever. This is the per-address guarantee.
    #[tokio::test]
    async fn one_flush_drains_persisted_session_prekey_and_defers_checked_out_one() {
        const PREKEY_A: u32 = 5101;
        const PREKEY_B: u32 = 5102;

        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        backend.store_prekey(PREKEY_A, b"a", false).await.unwrap();
        backend.store_prekey(PREKEY_B, b"b", false).await.unwrap();

        let addr_a = ProtocolAddress::new("alice", 1.into());
        let addr_b = ProtocolAddress::new("bob", 1.into());

        // Both decrypts promote their session (dirty) and buffer their prekey.
        cache.put_session(&addr_a, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_A, addr_a.as_str()).await;
        cache.put_session(&addr_b, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_B, addr_b.as_str()).await;

        // A reader checks B's session out; A stays Present. The dirty bit on B
        // stays set, so this flush skips persisting B but persists A.
        let taken_b = cache.get_session(&addr_b, &backend).await.unwrap();
        assert!(taken_b.is_some(), "B's promoted session should be readable");

        cache.flush(&backend).await.unwrap();

        // A's session is durable, so A's prekey is deleted in this same flush.
        assert!(
            backend
                .get_session(addr_a.as_str())
                .await
                .unwrap()
                .is_some(),
            "A's session must be durable after the flush"
        );
        assert!(
            backend.load_prekey(PREKEY_A).await.unwrap().is_none(),
            "A's prekey must be deleted: its session was persisted this flush"
        );

        // B's session is still volatile (checked out), so B's prekey is deferred
        // and stays buffered, NOT held back by A's commit.
        assert!(
            backend.load_prekey(PREKEY_B).await.unwrap().is_some(),
            "B's prekey must be deferred while B's session is checked out"
        );
        assert!(
            cache.removed_prekeys.lock().await.contains_key(&PREKEY_B),
            "B's prekey stays buffered for a later flush"
        );
        assert!(
            !cache.removed_prekeys.lock().await.contains_key(&PREKEY_A),
            "A's prekey must be drained from the buffer, not left to leak"
        );

        // Once B's reader returns the session, the next flush commits both.
        cache.put_session(&addr_b, taken_b.unwrap()).await;
        cache.flush(&backend).await.unwrap();
        assert!(
            backend.load_prekey(PREKEY_B).await.unwrap().is_none(),
            "B's prekey is deleted once B's session is durable"
        );
    }

    /// A disconnect (cache clear) before the flush drops the volatile session, so
    /// the still-durable prekey must be kept (its buffered removal dropped) to let
    /// a redelivered pkmsg rebuild the session.
    #[tokio::test]
    async fn clear_before_flush_keeps_prekey_so_pkmsg_can_rebuild() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID, addr.as_str()).await;

        cache.clear().await;

        // The session never reached the backend, so the prekey must survive.
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_none(),
            "volatile session is dropped on clear"
        );
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "prekey must survive a clear that discarded its unflushed session"
        );

        // A subsequent flush of the now-empty buffer is a no-op for the prekey.
        cache.flush(&backend).await.unwrap();
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "cleared buffer must not delete the prekey on a later flush"
        );
    }

    /// The same, for a row that is present but does not decode. Row existence
    /// alone would call it durable and delete the prekey, leaving a redelivered
    /// pkmsg with neither a usable session nor the prekey to rebuild one --
    /// which is the exact outcome the deferral rule exists to prevent.
    #[tokio::test]
    async fn prekey_behind_an_unreadable_session_row_survives_flush() {
        use super::lease_reload_tests::leased_session;
        use crate::libsignal::protocol::consts::MAX_RESERVATION_FAST_FORWARD;

        let backend = InMemoryBackend::new();
        let addr = seed(&backend).await;

        // Persist a row that only fails to decode after a restart, so the
        // backend genuinely holds bytes for this address.
        let writer = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let mut stranded = leased_session();
        stranded.reserve_sender_chain_counters(MAX_RESERVATION_FAST_FORWARD);
        writer.put_session(&addr, stranded).await;
        writer.flush(&backend).await.unwrap();
        assert!(
            backend.get_session(addr.as_str()).await.unwrap().is_some(),
            "the row is there; what follows is about whether it decodes"
        );

        // A different incarnation: the reload has to fast-forward, and refuses.
        let restarted = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xB2; 16],
        );
        restarted.remove_prekey(PREKEY_ID, addr.as_str()).await;
        restarted.flush(&backend).await.unwrap();

        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "a prekey behind a row that does not decode must survive the flush"
        );
    }

    /// A prekey buffered for a session that is not durable (its volatile session
    /// was dropped before the buffer insert landed, e.g. a disconnect clear()
    /// racing the consume path) must NOT be deleted: removing the durable prekey
    /// with no session behind it makes a redelivered pkmsg permanently
    /// undecryptable. The drain falls back to the backend, which has no session
    /// here, so the prekey is deferred.
    #[tokio::test]
    async fn prekey_without_a_persisted_session_survives_flush() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        // Buffer a prekey whose session is absent from the cache and the backend,
        // so the flush has no durable session to tie it to.
        cache.remove_prekey(PREKEY_ID, addr.as_str()).await;

        cache.flush(&backend).await.unwrap();

        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "a prekey with no durable session must survive the flush"
        );
        assert!(
            cache.removed_prekeys.lock().await.contains_key(&PREKEY_ID),
            "it stays buffered; a later clear() drops it, keeping the prekey durable"
        );
    }

    /// A prekey buffered AFTER its session was already persisted (a concurrent
    /// flush ran between the decrypt's session store and the receive path's buffer
    /// insert) must still be deleted: the session is durable, so the one-time
    /// prekey must not linger forever. The drain recognizes already-durable
    /// sessions, not only those this flush persisted.
    #[tokio::test]
    async fn prekey_buffered_after_session_already_durable_is_deleted() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let addr = seed(&backend).await;

        // A prior flush already persisted and cleaned the session, exactly as a
        // concurrent flush would leave it before the prekey gets buffered.
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.flush(&backend).await.unwrap();
        assert!(backend.get_session(addr.as_str()).await.unwrap().is_some());

        // Only now does the receive path buffer the consumed prekey.
        cache.remove_prekey(PREKEY_ID, addr.as_str()).await;

        cache.flush(&backend).await.unwrap();
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_none(),
            "prekey of an already-durable session must be deleted on the next flush"
        );
    }

    /// A failed session write must abort the flush before the prekey deletion, and
    /// the buffered ID must remain so the next flush retries it. This guards the
    /// exact regression: the prekey lane running before/independently of a durable
    /// session.
    #[tokio::test]
    async fn failed_session_flush_does_not_delete_prekey() {
        struct FailingSessions(InMemoryBackend);

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl SignalStore for FailingSessions {
            async fn put_sessions_batch(
                &self,
                _sessions: &[(Arc<str>, bytes::Bytes)],
            ) -> crate::store::error::Result<()> {
                Err(crate::store::error::StoreError::Validation(
                    "simulated session write failure".to_string(),
                ))
            }

            async fn put_identity(
                &self,
                address: &str,
                key: [u8; 32],
            ) -> crate::store::error::Result<()> {
                self.0.put_identity(address, key).await
            }
            async fn load_identity(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<[u8; 32]>> {
                self.0.load_identity(address).await
            }
            async fn delete_identity(&self, address: &str) -> crate::store::error::Result<()> {
                self.0.delete_identity(address).await
            }
            async fn get_session(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.0.get_session(address).await
            }
            async fn put_session(
                &self,
                address: &str,
                session: &[u8],
            ) -> crate::store::error::Result<()> {
                self.0.put_session(address, session).await
            }
            async fn delete_session(&self, address: &str) -> crate::store::error::Result<()> {
                self.0.delete_session(address).await
            }
            async fn store_prekey(
                &self,
                id: u32,
                record: &[u8],
                uploaded: bool,
            ) -> crate::store::error::Result<()> {
                self.0.store_prekey(id, record, uploaded).await
            }
            async fn load_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.0.load_prekey(id).await
            }
            async fn remove_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                self.0.remove_prekey(id).await
            }
            async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> crate::store::error::Result<()> {
                self.0.mark_prekeys_uploaded(ids).await
            }
            async fn get_max_prekey_id(&self) -> crate::store::error::Result<u32> {
                self.0.get_max_prekey_id().await
            }
            async fn store_signed_prekey(
                &self,
                id: u32,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.0.store_signed_prekey(id, record).await
            }
            async fn load_signed_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.0.load_signed_prekey(id).await
            }
            async fn load_all_signed_prekeys(
                &self,
            ) -> crate::store::error::Result<Vec<(u32, Vec<u8>)>> {
                self.0.load_all_signed_prekeys().await
            }
            async fn remove_signed_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                self.0.remove_signed_prekey(id).await
            }
            async fn put_sender_key(
                &self,
                address: &str,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.0.put_sender_key(address, record).await
            }
            async fn get_sender_key(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.0.get_sender_key(address).await
            }
            async fn delete_sender_key(&self, address: &str) -> crate::store::error::Result<()> {
                self.0.delete_sender_key(address).await
            }
        }

        let inner = InMemoryBackend::new();
        let addr = seed(&inner).await;
        let backend = FailingSessions(inner);
        let cache = SignalStoreCache::new();

        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_ID, addr.as_str()).await;

        // The session write fails, so flush errors out before the prekey lane.
        assert!(cache.flush(&backend).await.is_err());

        // The prekey must still be durable: it must never be deleted while its
        // session is not committed.
        assert!(
            backend.load_prekey(PREKEY_ID).await.unwrap().is_some(),
            "prekey must not be deleted when the session write fails"
        );

        // The buffered removal must remain so a later successful flush retries it.
        assert!(
            cache.removed_prekeys.lock().await.contains_key(&PREKEY_ID),
            "buffered prekey removal must persist across a failed flush"
        );
    }

    /// A decrypt racing a flush must never lose the session<->prekey atomicity.
    ///
    /// Sender A's flush holds the sessions lock across both the session commit AND
    /// the consumed-prekey drain. While it is mid-flush, sender B's decrypt tries to
    /// promote B's session and buffer B's consumed prekey. Because the prekey buffer
    /// is drained under that same sessions lock, B cannot reach the buffer until A's
    /// flush has fully committed and released the lock, so A's flush can never delete
    /// B's prekey while B's session is still volatile. The buggy form (prekey drain
    /// in a separate lock scope) releases the sessions lock first, leaving a window
    /// where B buffers its prekey and A then durably deletes it with B's session
    /// unflushed. The backend asserts the sessions lock is held at the moment the
    /// prekey is deleted, which directly distinguishes the fixed and buggy forms.
    #[tokio::test]
    async fn concurrent_decrypt_does_not_lose_prekey_during_flush() {
        use std::sync::Arc as StdArc;
        use std::sync::atomic::{AtomicBool, Ordering};

        const PREKEY_A: u32 = 1001;
        const PREKEY_B: u32 = 1002;

        /// Wraps an InMemoryBackend. `put_sessions_batch` yields the executor many
        /// times before doing the real write, so a concurrently spawned decrypt has
        /// every chance to reach (and block on) the sessions lock while A's flush
        /// holds it. `remove_prekey` records whether the sessions lock was actually
        /// held (the core invariant the fix establishes) and flags any prekey delete
        /// whose owning session is not yet durable.
        struct GatedBackend {
            inner: InMemoryBackend,
            // The cache under flush, so the backend can probe the sessions lock.
            cache: StdArc<SignalStoreCache>,
            // Set if a prekey was deleted while the sessions lock was NOT held: that
            // is the regression (prekey drain outside the sessions lock scope).
            drained_without_sessions_lock: StdArc<AtomicBool>,
            // Set if a prekey delete ever ran while its session was still volatile.
            violation: StdArc<AtomicBool>,
            addr_b: String,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl SignalStore for GatedBackend {
            async fn put_sessions_batch(
                &self,
                sessions: &[(Arc<str>, bytes::Bytes)],
            ) -> crate::store::error::Result<()> {
                // A's flush holds the sessions lock here; yield repeatedly so B's
                // spawned decrypt gets scheduled and blocks on that lock before the
                // session commit (and the prekey drain) completes.
                for _ in 0..64 {
                    tokio::task::yield_now().await;
                }
                self.inner.put_sessions_batch(sessions).await
            }
            async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> crate::store::error::Result<()> {
                self.inner.mark_prekeys_uploaded(ids).await
            }
            async fn remove_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                // The fix drains prekeys under the sessions lock, so a try_lock here
                // must fail while a flush is deleting. If it succeeds, the drain ran
                // outside the sessions lock: the exact regression.
                if self.cache.sessions.try_lock().is_some() {
                    self.drained_without_sessions_lock
                        .store(true, Ordering::SeqCst);
                }
                // B's prekey may only be deleted once B's session is durable.
                if id == PREKEY_B
                    && self
                        .inner
                        .get_session(&self.addr_b)
                        .await
                        .unwrap()
                        .is_none()
                {
                    self.violation.store(true, Ordering::SeqCst);
                }
                self.inner.remove_prekey(id).await
            }

            async fn put_identity(
                &self,
                address: &str,
                key: [u8; 32],
            ) -> crate::store::error::Result<()> {
                self.inner.put_identity(address, key).await
            }
            async fn load_identity(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<[u8; 32]>> {
                self.inner.load_identity(address).await
            }
            async fn delete_identity(&self, address: &str) -> crate::store::error::Result<()> {
                self.inner.delete_identity(address).await
            }
            async fn get_session(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.inner.get_session(address).await
            }
            async fn put_session(
                &self,
                address: &str,
                session: &[u8],
            ) -> crate::store::error::Result<()> {
                self.inner.put_session(address, session).await
            }
            async fn delete_session(&self, address: &str) -> crate::store::error::Result<()> {
                self.inner.delete_session(address).await
            }
            async fn store_prekey(
                &self,
                id: u32,
                record: &[u8],
                uploaded: bool,
            ) -> crate::store::error::Result<()> {
                self.inner.store_prekey(id, record, uploaded).await
            }
            async fn load_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<bytes::Bytes>> {
                self.inner.load_prekey(id).await
            }
            async fn get_max_prekey_id(&self) -> crate::store::error::Result<u32> {
                self.inner.get_max_prekey_id().await
            }
            async fn store_signed_prekey(
                &self,
                id: u32,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.inner.store_signed_prekey(id, record).await
            }
            async fn load_signed_prekey(
                &self,
                id: u32,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.inner.load_signed_prekey(id).await
            }
            async fn load_all_signed_prekeys(
                &self,
            ) -> crate::store::error::Result<Vec<(u32, Vec<u8>)>> {
                self.inner.load_all_signed_prekeys().await
            }
            async fn remove_signed_prekey(&self, id: u32) -> crate::store::error::Result<()> {
                self.inner.remove_signed_prekey(id).await
            }
            async fn put_sender_key(
                &self,
                address: &str,
                record: &[u8],
            ) -> crate::store::error::Result<()> {
                self.inner.put_sender_key(address, record).await
            }
            async fn get_sender_key(
                &self,
                address: &str,
            ) -> crate::store::error::Result<Option<Vec<u8>>> {
                self.inner.get_sender_key(address).await
            }
            async fn delete_sender_key(&self, address: &str) -> crate::store::error::Result<()> {
                self.inner.delete_sender_key(address).await
            }
        }

        let inner = InMemoryBackend::new();
        inner
            .store_prekey(PREKEY_A, b"prekey-a", false)
            .await
            .unwrap();
        inner
            .store_prekey(PREKEY_B, b"prekey-b", false)
            .await
            .unwrap();

        let addr_a = ProtocolAddress::new("alice", 1.into());
        let addr_b = ProtocolAddress::new("bob", 1.into());

        let cache = StdArc::new(SignalStoreCache::new());
        let violation = StdArc::new(AtomicBool::new(false));
        let drained_without_sessions_lock = StdArc::new(AtomicBool::new(false));

        let backend = StdArc::new(GatedBackend {
            inner,
            cache: cache.clone(),
            drained_without_sessions_lock: drained_without_sessions_lock.clone(),
            violation: violation.clone(),
            addr_b: addr_b.as_str().to_string(),
        });

        // Sender A's decrypt: promote A's session, buffer A's consumed prekey.
        cache.put_session(&addr_a, SessionRecord::new_fresh()).await;
        cache.remove_prekey(PREKEY_A, addr_a.as_str()).await;

        // Sender B's decrypt races A's flush: it promotes B's session and buffers
        // B's consumed prekey. put_session must take the sessions lock, so while A's
        // flush holds it (yielding inside put_sessions_batch) B blocks here and can
        // only buffer once A's flush has committed and released the lock.
        let b_cache = cache.clone();
        let addr_b_task = addr_b.clone();
        let b_task = tokio::spawn(async move {
            b_cache
                .put_session(&addr_b_task, SessionRecord::new_fresh())
                .await;
            b_cache.remove_prekey(PREKEY_B, addr_b_task.as_str()).await;
        });

        // A's flush runs concurrently with B's spawned decrypt. It holds the
        // sessions lock across its yielding I/O and the prekey drain, so B cannot
        // insert into removed_prekeys until A is done: A can never delete B's prekey.
        cache.flush(backend.as_ref()).await.unwrap();
        b_task.await.unwrap();

        // The core invariant: every prekey delete during the flush ran while the
        // sessions lock was held, so no concurrent decrypt could have buffered a
        // prekey into the same drain. This is what makes session+prekey atomic.
        assert!(
            !drained_without_sessions_lock.load(Ordering::SeqCst),
            "prekey was drained without holding the sessions lock (regression)"
        );

        // The flush must never have deleted B's prekey while B's session was
        // volatile.
        assert!(
            !violation.load(Ordering::SeqCst),
            "flush deleted B's prekey while B's session was still volatile"
        );

        // A's commit is durable: its session is persisted and its prekey gone.
        assert!(
            backend
                .get_session(addr_a.as_str())
                .await
                .unwrap()
                .is_some(),
            "sender A's session must be durable after its flush"
        );
        assert!(
            backend.load_prekey(PREKEY_A).await.unwrap().is_none(),
            "sender A's consumed prekey must be deleted with its session"
        );

        // B buffered its prekey only after A's flush completed, so B's prekey is
        // still durable and still buffered for B's own next flush.
        assert!(
            backend.load_prekey(PREKEY_B).await.unwrap().is_some(),
            "B's prekey must survive a concurrent flush that did not persist B's session"
        );
        assert!(
            cache.removed_prekeys.lock().await.contains_key(&PREKEY_B),
            "B's prekey removal stays buffered for B's own flush"
        );

        // B's own flush then commits B's session and B's prekey atomically.
        cache.flush(backend.as_ref()).await.unwrap();
        assert!(
            backend
                .get_session(addr_b.as_str())
                .await
                .unwrap()
                .is_some(),
            "B's session must be durable after B's flush"
        );
        assert!(
            backend.load_prekey(PREKEY_B).await.unwrap().is_none(),
            "B's prekey is deleted only once B's session is durable"
        );
        assert!(
            !violation.load(Ordering::SeqCst),
            "B's prekey delete must coincide with B's durable session"
        );
    }
}

#[cfg(test)]
mod eviction_tests {
    use super::*;
    use crate::libsignal::protocol::{DeviceId, ProtocolAddress};
    use crate::store::in_memory::InMemoryBackend;

    fn addr(i: usize) -> ProtocolAddress {
        ProtocolAddress::new(&format!("user{i}@s.whatsapp.net"), DeviceId::new(0))
    }

    #[test]
    fn high_watermark_is_above_max_and_amortizes() {
        // The watermark must sit strictly above max_entries so a scan can fire
        // only after `slack` extra inserts, otherwise the amortization is lost.
        assert!(high_watermark(2_000) > 2_000);
        assert_eq!(
            high_watermark(2_000),
            2_000 + 2_000 / EVICTION_SLACK_DIVISOR
        );
        // Tiny caps still get a meaningful slack via the floor.
        assert_eq!(high_watermark(4), 4 + EVICTION_SLACK_FLOOR);
    }

    #[tokio::test]
    async fn eviction_bounds_cache_over_many_inserts() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // Flush after each put so the prior entry becomes clean (non-dirty) and
        // therefore evictable on the next put; otherwise every entry is pinned.
        for i in 0..(max * 4) {
            cache.put_identity(&addr(i), &[0u8; 32]).await;
            cache.flush(&backend).await.unwrap();
        }

        let len = cache.identities.lock().await.cache.len();
        assert!(
            len <= high_watermark(max),
            "cache grew past the high watermark: len={len} watermark={}",
            high_watermark(max)
        );
        // It must still be doing real work, not collapsing to empty.
        assert!(
            len >= max,
            "eviction was too aggressive: len={len} max={max}"
        );
    }

    #[tokio::test]
    async fn read_over_capacity_stays_bounded() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // Push the identity store right up to the watermark with clean entries.
        let watermark = high_watermark(max);
        for i in 0..watermark {
            cache.put_identity(&addr(i), &[0u8; 32]).await;
            cache.flush(&backend).await.unwrap();
        }
        let before = cache.identities.lock().await.cache.len();
        assert_eq!(before, watermark, "setup should fill exactly to watermark");

        // A read-populate (cache-miss) that crosses the watermark must trigger the
        // amortized eviction too: read traffic populates the cache, so it cannot be
        // allowed to grow it unbounded.
        let missing = addr(watermark + 1);
        let got = cache.get_identity(&missing, &backend).await.unwrap();
        assert!(got.is_none());

        let after = cache.identities.lock().await.cache.len();
        assert!(
            after <= watermark,
            "a read over capacity must stay bounded: after={after} watermark={watermark}"
        );
    }

    #[tokio::test]
    async fn read_flood_of_unique_keys_stays_bounded() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // A flood of unique cache-miss reads each negative-cache a clean entry.
        // Without read-path eviction this grew without bound; it must stay bounded.
        for i in 0..(max * 8) {
            assert!(
                cache
                    .get_identity(&addr(i), &backend)
                    .await
                    .unwrap()
                    .is_none()
            );
        }

        let len = cache.identities.lock().await.cache.len();
        assert!(
            len <= high_watermark(max),
            "unique-read flood must stay bounded: len={len} watermark={}",
            high_watermark(max)
        );
    }

    #[tokio::test]
    async fn dirty_entries_are_never_evicted() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);

        // Every put marks the key dirty and we never flush, so all entries are
        // pinned. Even far past the watermark, none may be dropped.
        let total = high_watermark(max) * 2;
        for i in 0..total {
            cache.put_identity(&addr(i), &[0u8; 32]).await;
        }

        let len = cache.identities.lock().await.cache.len();
        assert_eq!(
            len, total,
            "dirty (unflushed) entries must never be evicted"
        );
    }

    #[tokio::test]
    async fn checked_out_sessions_are_never_evicted() {
        let max = 64usize;
        let cache = SignalStoreCache::with_max_entries(max);
        let backend = InMemoryBackend::new();

        // Persist one session, then check it out (get_session leaves a CheckedOut
        // marker) so eviction must skip it.
        let pinned = addr(0);
        cache.put_session(&pinned, SessionRecord::new_fresh()).await;
        cache.flush(&backend).await.unwrap();
        let taken = cache.get_session(&pinned, &backend).await.unwrap();
        assert!(taken.is_some(), "session should be present before checkout");

        // Flood the session store with clean Absent markers (has_session misses)
        // so the watermark is crossed, then trigger eviction via a put.
        let watermark = high_watermark(max);
        for i in 1..(watermark + 8) {
            // has_session miss negative-caches an Absent entry (a read, no evict).
            assert!(!cache.has_session(&addr(i), &backend).await.unwrap());
        }
        // A put fires the eviction scan; it must drop clean Absent markers but
        // keep the CheckedOut session pinned.
        cache
            .put_session(&addr(99_999), SessionRecord::new_fresh())
            .await;

        {
            let state = cache.sessions.lock().await;
            let entry = state.cache.get(pinned.as_str());
            assert!(
                matches!(entry, Some(SessionEntry::CheckedOut { .. })),
                "checked-out session must survive eviction"
            );
            assert!(
                state.cache.len() <= high_watermark(max) + 1,
                "eviction must bound the session cache: len={}",
                state.cache.len()
            );
        }
    }
}

#[cfg(test)]
mod lease_reload_tests {
    use super::*;
    use crate::libsignal::protocol::{
        ChainKey, IdentityKey, KeyPair, RootKey, SenderKeyStore, SessionState,
        create_sender_key_distribution_message, group_decrypt, group_encrypt,
        process_sender_key_distribution_message,
    };
    use crate::store::in_memory::InMemoryBackend;

    struct CachedSenderKeyStore<'a> {
        cache: &'a SignalStoreCache,
        backend: &'a InMemoryBackend,
    }

    #[async_trait::async_trait]
    impl SenderKeyStore for CachedSenderKeyStore<'_> {
        async fn store_sender_key(
            &mut self,
            name: &SenderKeyName,
            record: SenderKeyRecord,
        ) -> crate::libsignal::protocol::error::Result<()> {
            self.cache.put_sender_key(name, record).await;
            Ok(())
        }

        async fn load_sender_key(
            &self,
            name: &SenderKeyName,
        ) -> crate::libsignal::protocol::error::Result<Option<SenderKeyRecord>> {
            Ok(self
                .cache
                .get_sender_key(name, self.backend)
                .await
                .expect("test backend")
                .map(|record| (*record).clone()))
        }
    }

    fn sender_key_name() -> SenderKeyName {
        SenderKeyName::from_parts("group@g.us", "15550001000@s.whatsapp.net:0")
    }

    pub(super) fn leased_session() -> SessionRecord {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let local = IdentityKey::new(KeyPair::generate(&mut rng).public_key);
        let remote = IdentityKey::new(KeyPair::generate(&mut rng).public_key);
        let base_key = KeyPair::generate(&mut rng).public_key;
        let mut state = SessionState::new(3, &local, &remote, &RootKey::new([0; 32]), &base_key);
        state.set_sender_chain(&KeyPair::generate(&mut rng), &ChainKey::new([1; 32], 0));
        let mut record = SessionRecord::new(state);
        record.reserve_sender_chain_counters(0);
        record
    }

    fn session_chain_index(record: &SessionRecord) -> u32 {
        record
            .session_state()
            .expect("session")
            .get_sender_chain_key()
            .expect("sender chain")
            .index()
    }

    #[tokio::test]
    async fn post_flush_clear_preserves_only_live_checkouts() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let active = ProtocolAddress::new("15550001007", 1.into());
        let idle = ProtocolAddress::new("15550001008", 1.into());
        cache.put_session(&active, leased_session()).await;
        cache.put_session(&idle, leased_session()).await;
        cache.flush(&backend).await.expect("flush");

        let (record, checkout) = cache.checkout_session(&active, &backend).await.unwrap();
        cache.remove_prekey(7, active.as_str()).await;
        cache.clear_after_flush().await;

        {
            let state = cache.sessions.lock().await;
            assert!(matches!(
                state.cache.get(active.as_str()),
                Some(SessionEntry::CheckedOut { .. })
            ));
            assert!(!state.cache.contains_key(idle.as_str()));
        }
        assert!(cache.removed_prekeys.lock().await.contains_key(&7));
        assert!(matches!(
            cache.restore_session_from_checkout(
                &active,
                record.expect("checked-out record"),
                checkout,
                true,
            ),
            SessionCheckoutStoreResult::Stored
        ));
    }

    #[tokio::test]
    async fn dm_clean_reload_is_exact_but_new_cache_burns_the_lease() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = ProtocolAddress::new("15550001001", 1.into());
        cache.put_session(&address, leased_session()).await;
        cache.flush(&backend).await.expect("flush");
        cache.clear_after_flush().await;

        let clean = cache
            .get_session(&address, &backend)
            .await
            .expect("cache load")
            .expect("session");
        assert_eq!(session_chain_index(&clean), 0);

        let replacement = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xB2; 16],
        );
        let recovered = replacement
            .get_session(&address, &backend)
            .await
            .expect("recovery load")
            .expect("session");
        assert_eq!(
            session_chain_index(&recovered),
            crate::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH
        );
    }

    /// A row whose lease is stranded above its chain (issue #1146: written by
    /// a build that let a DH ratchet retire the chain without rebasing the
    /// ceiling) cannot be fast-forwarded on recovery. It must not become a
    /// hard error on every load: that strands the address, because the very
    /// paths that would replace the session — the peer's next pre-key message
    /// and the retry repair — have to load it first. Report it absent so the
    /// no-session recovery replaces it.
    #[tokio::test]
    async fn an_unreadable_session_row_is_reported_absent_so_recovery_can_replace_it() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = ProtocolAddress::new("15550001009", 1.into());

        let mut stranded = leased_session();
        stranded.reserve_sender_chain_counters(
            crate::libsignal::protocol::consts::MAX_RESERVATION_FAST_FORWARD,
        );
        assert_eq!(session_chain_index(&stranded), 0);
        cache.put_session(&address, stranded).await;
        cache.flush(&backend).await.expect("flush");

        // A live reload never fast-forwards, so the row still looks fine here.
        cache.clear_after_flush().await;
        assert!(
            cache
                .get_session(&address, &backend)
                .await
                .expect("live reload")
                .is_some()
        );

        // A restart (or lossy reset) is where recovery has to fast-forward.
        let restarted = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xB2; 16],
        );
        assert!(
            restarted
                .get_session(&address, &backend)
                .await
                .expect("an unreadable row must not fail the load")
                .is_none()
        );
        assert!(
            !restarted
                .has_session(&address, &backend)
                .await
                .expect("has_session"),
            "the quarantined address must look session-less so ensure_e2e_sessions rebuilds it"
        );
    }

    /// The existence probe on a cold cache is what decides whether a send
    /// fetches a pre-key bundle, and it runs before anything loads the record.
    /// Asking the backend whether the row exists answers `true` for a row the
    /// very next checkout will discard, so the recovery is skipped and the send
    /// either fails or drops that recipient from the fan-out.
    ///
    /// Distinct from the test above, which reaches `has_session` only after a
    /// `get_session` has already negative-cached the address: that one passes
    /// against the backend-existence probe too.
    #[tokio::test]
    async fn a_cold_existence_probe_does_not_report_a_quarantined_row_as_present() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = ProtocolAddress::new("15550001010", 1.into());

        let mut stranded = leased_session();
        stranded.reserve_sender_chain_counters(
            crate::libsignal::protocol::consts::MAX_RESERVATION_FAST_FORWARD,
        );
        cache.put_session(&address, stranded).await;
        cache.flush(&backend).await.expect("flush");

        // Nothing has touched this address in this incarnation: the probe is
        // the first thing to reach the row, exactly as it is on a real restart.
        let restarted = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xB2; 16],
        );
        assert!(
            !restarted
                .has_session(&address, &backend)
                .await
                .expect("a quarantined row must not fail the probe"),
            "a row the next checkout would discard must not be reported present"
        );

        // And the negative answer is cached, so the send that follows keeps
        // seeing it session-less rather than re-reading the same row.
        assert!(
            restarted
                .get_session(&address, &backend)
                .await
                .expect("checkout")
                .is_none()
        );
    }

    #[tokio::test]
    async fn incomplete_session_flush_retains_newer_state_and_fails_closed_on_recovery() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = ProtocolAddress::new("15550001002", 1.into());
        cache.put_session(&address, leased_session()).await;
        cache.flush(&backend).await.expect("initial flush");

        let mut advanced = cache
            .get_session(&address, &backend)
            .await
            .expect("cache load")
            .expect("session");
        let next = advanced
            .session_state()
            .expect("session")
            .get_sender_chain_key()
            .expect("sender chain")
            .next_chain_key()
            .expect("chain advance");
        advanced
            .session_state_mut()
            .expect("session")
            .set_sender_chain_key(&next)
            .expect("chain update");
        cache.put_session(&address, advanced).await;

        let checked_out = cache
            .get_session(&address, &backend)
            .await
            .expect("cache checkout")
            .expect("session");
        cache.flush(&backend).await.expect("skipped flush");
        cache.clear_after_flush().await;

        {
            let state = cache.sessions.lock().await;
            assert_eq!(state.incarnation, [0xA1; 16]);
            assert!(state.dirty.contains(address.as_str()));
            assert!(matches!(
                state.cache.get(address.as_str()),
                Some(SessionEntry::CheckedOut { .. })
            ));
        }

        let replacement = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xB2; 16],
        );
        let recovered = replacement
            .get_session(&address, &backend)
            .await
            .expect("recovery load")
            .expect("session");
        assert_eq!(
            session_chain_index(&recovered),
            crate::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH
        );

        cache.put_session(&address, checked_out).await;
        cache.flush(&backend).await.expect("retry flush");
        cache.clear_after_flush().await;
        let exact = cache
            .get_session(&address, &backend)
            .await
            .expect("exact reload")
            .expect("session");
        assert_eq!(session_chain_index(&exact), 1);
    }

    #[tokio::test]
    async fn repeated_clean_reloads_keep_group_messages_within_forward_jump_limit() {
        let sender_backend = InMemoryBackend::new();
        let sender_cache = SignalStoreCache::new();
        let mut sender = CachedSenderKeyStore {
            cache: &sender_cache,
            backend: &sender_backend,
        };
        let receiver_backend = InMemoryBackend::new();
        let receiver_cache = SignalStoreCache::new();
        let mut receiver = CachedSenderKeyStore {
            cache: &receiver_cache,
            backend: &receiver_backend,
        };
        let name = sender_key_name();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let skdm = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .expect("sender setup");
        process_sender_key_distribution_message(&name, &skdm, &mut receiver)
            .await
            .expect("receiver setup");

        let mut last = None;
        for expected_iteration in 0..=32 {
            let message = group_encrypt(&mut sender, &name, b"payload", &mut rng)
                .await
                .expect("group encrypt");
            assert_eq!(message.iteration(), expected_iteration);
            last = Some(message);
            sender_cache.flush(&sender_backend).await.expect("flush");
            sender_cache.clear_after_flush().await;
        }

        let plaintext = group_decrypt(last.expect("message").serialized(), &mut receiver, &name)
            .await
            .expect("a peer may miss every preceding message");
        assert_eq!(plaintext, b"payload");
    }

    #[tokio::test]
    async fn clean_sender_key_eviction_does_not_burn_a_lease() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let mut store = CachedSenderKeyStore {
            cache: &cache,
            backend: &backend,
        };
        let name = sender_key_name();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        create_sender_key_distribution_message(&name, &mut store, &mut rng)
            .await
            .expect("sender setup");

        let first = group_encrypt(&mut store, &name, b"first", &mut rng)
            .await
            .expect("first send");
        assert_eq!(first.iteration(), 0);
        cache.flush(&backend).await.expect("flush");
        assert!(
            cache
                .sender_keys
                .lock()
                .await
                .cache
                .remove(name.cache_key())
                .is_some()
        );

        let second = group_encrypt(&mut store, &name, b"second", &mut rng)
            .await
            .expect("send after eviction");
        assert_eq!(second.iteration(), 1);
    }

    #[tokio::test]
    async fn dirty_sender_key_stays_resident_while_recovery_fails_closed() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let mut store = CachedSenderKeyStore {
            cache: &cache,
            backend: &backend,
        };
        let name = sender_key_name();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        create_sender_key_distribution_message(&name, &mut store, &mut rng)
            .await
            .expect("sender setup");

        let first = group_encrypt(&mut store, &name, b"first", &mut rng)
            .await
            .expect("first send");
        assert_eq!(first.iteration(), 0);
        cache.flush(&backend).await.expect("flush");

        let unflushed = group_encrypt(&mut store, &name, b"unflushed", &mut rng)
            .await
            .expect("unflushed send");
        assert_eq!(unflushed.iteration(), 1);
        cache.clear_after_flush().await;

        {
            let state = cache.sender_keys.lock().await;
            assert_eq!(state.incarnation, [0xA1; 16]);
            assert!(state.dirty.contains(name.cache_key()));
            assert!(state.cache.contains_key(name.cache_key()));
        }

        let resumed = group_encrypt(&mut store, &name, b"resumed", &mut rng)
            .await
            .expect("resident send");
        assert_eq!(resumed.iteration(), 2);

        let replacement = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xB2; 16],
        );
        let mut recovered_store = CachedSenderKeyStore {
            cache: &replacement,
            backend: &backend,
        };
        let recovered = group_encrypt(&mut recovered_store, &name, b"recovered", &mut rng)
            .await
            .expect("recovery send");
        assert_eq!(
            recovered.iteration(),
            crate::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH
        );

        cache.flush(&backend).await.expect("retry flush");
        cache.clear_after_flush().await;
        let exact = group_encrypt(&mut store, &name, b"exact", &mut rng)
            .await
            .expect("exact reload");
        assert_eq!(exact.iteration(), 3);
    }
}

#[cfg(test)]
mod pre_wire_gate_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::libsignal::store::sender_key_name::SenderKeyName;
    use crate::store::in_memory::InMemoryBackend;
    use async_lock::Barrier;

    fn addr(user: &str) -> ProtocolAddress {
        ProtocolAddress::new(user, 1.into())
    }

    fn leased_record() -> SessionRecord {
        let mut record = SessionRecord::new_fresh();
        record.reserve_sender_chain_counters(0);
        record
    }

    fn gated_sender_key() -> SenderKeyRecord {
        let mut record = SenderKeyRecord::new_empty();
        record.mark_wire_gated();
        record
    }

    /// The lock-free flags only exist to answer `needs_pre_wire_flush`, so
    /// every mutation of either pending set has to leave them agreeing with it.
    async fn assert_gate_agrees(cache: &SignalStoreCache, after: &str) {
        let session_set = !cache.lock_sessions().await.reservation_pending.is_empty();
        let sender_set = !cache.sender_keys.lock().await.wire_gate_pending.is_empty();
        assert_eq!(
            cache.session_wire_gate.load(Ordering::Acquire),
            session_set,
            "session flag disagrees with its set after {after}"
        );
        assert_eq!(
            cache.sender_key_wire_gate.load(Ordering::Acquire),
            sender_set,
            "sender-key flag disagrees with its set after {after}"
        );
        assert_eq!(
            cache.needs_pre_wire_flush().await,
            session_set || sender_set,
            "wire-gate query disagrees with cache state after {after}"
        );
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum DeleteTarget {
        Session,
        SenderKey,
    }

    struct DeleteBarrierBackend {
        inner: InMemoryBackend,
        target: DeleteTarget,
        entered: Barrier,
        release: Barrier,
        fail_delete: AtomicBool,
    }

    impl DeleteBarrierBackend {
        fn new(target: DeleteTarget) -> Self {
            Self {
                inner: InMemoryBackend::new(),
                target,
                entered: Barrier::new(2),
                release: Barrier::new(2),
                fail_delete: AtomicBool::new(true),
            }
        }

        async fn gate_delete(&self, target: DeleteTarget) -> crate::store::error::Result<()> {
            if self.target != target {
                return Ok(());
            }
            self.entered.wait().await;
            self.release.wait().await;
            if self.fail_delete.load(Ordering::Acquire) {
                return Err(crate::store::error::StoreError::Validation(
                    "simulated delete failure".to_string(),
                ));
            }
            Ok(())
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl SignalStore for DeleteBarrierBackend {
        async fn put_identity(
            &self,
            address: &str,
            key: [u8; 32],
        ) -> crate::store::error::Result<()> {
            self.inner.put_identity(address, key).await
        }

        async fn load_identity(
            &self,
            address: &str,
        ) -> crate::store::error::Result<Option<[u8; 32]>> {
            self.inner.load_identity(address).await
        }

        async fn delete_identity(&self, address: &str) -> crate::store::error::Result<()> {
            self.inner.delete_identity(address).await
        }

        async fn get_session(
            &self,
            address: &str,
        ) -> crate::store::error::Result<Option<bytes::Bytes>> {
            self.inner.get_session(address).await
        }

        async fn put_session(
            &self,
            address: &str,
            session: &[u8],
        ) -> crate::store::error::Result<()> {
            self.inner.put_session(address, session).await
        }

        async fn delete_session(&self, address: &str) -> crate::store::error::Result<()> {
            self.gate_delete(DeleteTarget::Session).await?;
            self.inner.delete_session(address).await
        }

        async fn store_prekey(
            &self,
            id: u32,
            record: &[u8],
            uploaded: bool,
        ) -> crate::store::error::Result<()> {
            self.inner.store_prekey(id, record, uploaded).await
        }

        async fn load_prekey(&self, id: u32) -> crate::store::error::Result<Option<bytes::Bytes>> {
            self.inner.load_prekey(id).await
        }

        async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> crate::store::error::Result<()> {
            self.inner.mark_prekeys_uploaded(ids).await
        }

        async fn remove_prekey(&self, id: u32) -> crate::store::error::Result<()> {
            self.inner.remove_prekey(id).await
        }

        async fn get_max_prekey_id(&self) -> crate::store::error::Result<u32> {
            self.inner.get_max_prekey_id().await
        }

        async fn store_signed_prekey(
            &self,
            id: u32,
            record: &[u8],
        ) -> crate::store::error::Result<()> {
            self.inner.store_signed_prekey(id, record).await
        }

        async fn load_signed_prekey(
            &self,
            id: u32,
        ) -> crate::store::error::Result<Option<Vec<u8>>> {
            self.inner.load_signed_prekey(id).await
        }

        async fn load_all_signed_prekeys(
            &self,
        ) -> crate::store::error::Result<Vec<(u32, Vec<u8>)>> {
            self.inner.load_all_signed_prekeys().await
        }

        async fn remove_signed_prekey(&self, id: u32) -> crate::store::error::Result<()> {
            self.inner.remove_signed_prekey(id).await
        }

        async fn put_sender_key(
            &self,
            address: &str,
            record: &[u8],
        ) -> crate::store::error::Result<()> {
            self.inner.put_sender_key(address, record).await
        }

        async fn get_sender_key(
            &self,
            address: &str,
        ) -> crate::store::error::Result<Option<Vec<u8>>> {
            self.inner.get_sender_key(address).await
        }

        async fn delete_sender_key(&self, address: &str) -> crate::store::error::Result<()> {
            self.gate_delete(DeleteTarget::SenderKey).await?;
            self.inner.delete_sender_key(address).await
        }
    }

    async fn run_gated_flush(
        cache: Arc<SignalStoreCache>,
        backend: Arc<DeleteBarrierBackend>,
    ) -> Result<()> {
        let flush_cache = cache.clone();
        let flush_backend = backend.clone();
        let task = tokio::spawn(async move { flush_cache.flush(flush_backend.as_ref()).await });

        backend.entered.wait().await;
        backend.release.wait().await;
        task.await.expect("flush task")
    }

    /// A raised lease gates the wire until a flush actually persists it; a
    /// plain (decrypt-style) session write never does.
    #[tokio::test]
    async fn session_lease_gates_until_a_successful_flush() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();

        cache
            .put_session(&addr("15550000001"), SessionRecord::new_fresh())
            .await;
        assert!(
            !cache.needs_pre_wire_flush().await,
            "a dirty session without a raised lease must not gate the wire"
        );

        cache
            .put_session(&addr("15550000002"), leased_record())
            .await;
        assert!(cache.needs_pre_wire_flush().await);

        cache.flush(&backend).await.unwrap();
        assert!(
            !cache.needs_pre_wire_flush().await,
            "a persisted lease releases the gate"
        );
    }

    /// A failed flush must keep the gate closed — the lease never reached
    /// storage, so the ciphertext must keep waiting.
    #[tokio::test]
    async fn failed_flush_keeps_the_gate_closed() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();

        cache
            .put_session(&addr("15550000003"), leased_record())
            .await;
        backend.set_fail_session_writes(true);
        assert!(cache.flush(&backend).await.is_err());
        assert!(
            cache.needs_pre_wire_flush().await,
            "an unpersisted lease must keep gating the wire"
        );

        backend.set_fail_session_writes(false);
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
    }

    /// A checked-out session cannot be persisted by a flush, so its pending
    /// lease must survive that flush and release only once the returned
    /// record is actually written.
    #[tokio::test]
    async fn checked_out_session_keeps_its_lease_pending_across_a_flush() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let a = addr("15550000004");

        cache.put_session(&a, leased_record()).await;
        let taken = cache.get_session(&a, &backend).await.unwrap().unwrap();

        cache.flush(&backend).await.unwrap();
        assert!(
            cache.needs_pre_wire_flush().await,
            "a checked-out lease was not persisted and must keep the gate closed"
        );

        cache.put_session(&a, taken).await;
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
    }

    /// Outbound sender-key advances gate the wire; decrypt-side dirtiness
    /// (no wire gate mark) must not, so group receives never force a sync
    /// flush onto an unrelated DM send.
    #[tokio::test]
    async fn only_encrypt_marked_sender_keys_gate_the_wire() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        cache
            .put_sender_key(&name, SenderKeyRecord::new_empty())
            .await;
        assert!(
            !cache.needs_pre_wire_flush().await,
            "a decrypt-side sender-key write must not gate the wire"
        );

        let mut outbound = SenderKeyRecord::new_empty();
        outbound.mark_wire_gated();
        cache.put_sender_key(&name, outbound).await;
        assert!(cache.needs_pre_wire_flush().await);

        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
    }

    /// The sender-key counterpart of `failed_flush_keeps_the_gate_closed`: a
    /// flush that fails writing the chain advance must keep the wire gated.
    #[tokio::test]
    async fn failed_flush_keeps_the_sender_key_gate_closed() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        let mut outbound = SenderKeyRecord::new_empty();
        outbound.mark_wire_gated();
        cache.put_sender_key(&name, outbound).await;

        backend.set_fail_sender_key_writes(true);
        assert!(cache.flush(&backend).await.is_err());
        assert!(
            cache.needs_pre_wire_flush().await,
            "an unpersisted sender-key advance must keep gating the wire"
        );

        backend.set_fail_sender_key_writes(false);
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
    }

    #[tokio::test]
    async fn session_tombstone_keeps_gate_until_delete_is_durable() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(DeleteBarrierBackend::new(DeleteTarget::Session));
        let address = addr("15550000007");

        backend
            .inner
            .put_session(address.as_str(), b"durable session")
            .await
            .unwrap();
        cache.put_session(&address, leased_record()).await;
        cache.delete_session(&address).await;
        assert!(cache.needs_pre_wire_flush().await);

        assert!(
            run_gated_flush(cache.clone(), backend.clone())
                .await
                .is_err()
        );
        assert!(cache.needs_pre_wire_flush().await);
        assert!(
            backend
                .inner
                .get_session(address.as_str())
                .await
                .unwrap()
                .is_some()
        );

        backend.fail_delete.store(false, Ordering::Release);
        run_gated_flush(cache.clone(), backend.clone())
            .await
            .unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert!(
            backend
                .inner
                .get_session(address.as_str())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn sender_key_tombstone_keeps_gate_until_delete_is_durable() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(DeleteBarrierBackend::new(DeleteTarget::SenderKey));
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        backend
            .inner
            .put_sender_key(name.cache_key(), b"durable sender key")
            .await
            .unwrap();
        let mut outbound = SenderKeyRecord::new_empty();
        outbound.mark_wire_gated();
        cache.put_sender_key(&name, outbound).await;
        cache.delete_sender_key(name.cache_key()).await;
        assert!(cache.needs_pre_wire_flush().await);

        assert!(
            run_gated_flush(cache.clone(), backend.clone())
                .await
                .is_err()
        );
        assert!(cache.needs_pre_wire_flush().await);
        assert!(
            backend
                .inner
                .get_sender_key(name.cache_key())
                .await
                .unwrap()
                .is_some()
        );

        backend.fail_delete.store(false, Ordering::Release);
        run_gated_flush(cache.clone(), backend.clone())
            .await
            .unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert!(
            backend
                .inner
                .get_sender_key(name.cache_key())
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn durable_sender_key_delete_does_not_block_unrelated_chains() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(DeleteBarrierBackend::new(DeleteTarget::SenderKey));
        backend.fail_delete.store(false, Ordering::Release);
        let target = SenderKeyName::from_parts("g1@g.us", "u@s.whatsapp.net:0");
        let unrelated = SenderKeyName::from_parts("g2@g.us", "u@s.whatsapp.net:0");
        cache
            .put_sender_key(&target, SenderKeyRecord::new_empty())
            .await;
        let target_lock = cache.sender_key_lock(&target).await;

        let deletion = tokio::spawn({
            let cache = cache.clone();
            let backend = backend.clone();
            async move {
                cache
                    .delete_sender_key_durable(&target, backend.as_ref())
                    .await
            }
        });
        backend.entered.wait().await;

        assert!(
            target_lock.try_lock().is_none(),
            "the target chain must remain serialized during backend deletion"
        );
        tokio::time::timeout(
            std::time::Duration::from_secs(1),
            cache.put_sender_key(&unrelated, SenderKeyRecord::new_empty()),
        )
        .await
        .expect("backend latency for one chain must not hold the global cache lock");

        backend.release.wait().await;
        deletion
            .await
            .expect("delete task")
            .expect("durable delete");
        assert!(
            cache
                .get_sender_key(&unrelated, backend.as_ref())
                .await
                .unwrap()
                .is_some(),
            "unrelated state must remain available"
        );
    }

    /// Cleanup racing a post-flush write must not release its durability gate.
    #[tokio::test]
    async fn clear_after_flush_retains_every_post_flush_write_and_wire_gate() {
        const PREKEY_ID: u32 = 7001;

        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = addr("15550000005");
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        cache.flush(&backend).await.unwrap();
        cache.put_session(&address, leased_record()).await;
        cache.put_identity(&address, &[7; 32]).await;
        backend
            .store_prekey(PREKEY_ID, b"prekey", false)
            .await
            .unwrap();
        cache.remove_prekey(PREKEY_ID, address.as_str()).await;
        let mut outbound = SenderKeyRecord::new_empty();
        outbound.mark_wire_gated();
        cache.put_sender_key(&name, outbound).await;

        cache.clear_after_flush().await;

        assert!(cache.needs_pre_wire_flush().await);
        {
            let sessions = cache.sessions.lock().await;
            assert_eq!(sessions.incarnation, [0xA1; 16]);
            assert!(sessions.dirty.contains(address.as_str()));
            assert!(sessions.reservation_pending.contains(address.as_str()));
        }
        {
            let identities = cache.identities.lock().await;
            assert!(identities.dirty.contains(address.as_str()));
        }
        {
            let sender_keys = cache.sender_keys.lock().await;
            assert_eq!(sender_keys.incarnation, [0xA1; 16]);
            assert!(sender_keys.dirty.contains(name.cache_key()));
            assert!(sender_keys.wire_gate_pending.contains(name.cache_key()));
        }
        assert!(cache.removed_prekeys.lock().await.contains_key(&PREKEY_ID));

        cache.flush(&backend).await.unwrap();

        assert!(!cache.needs_pre_wire_flush().await);
        assert!(
            backend
                .get_session(address.as_str())
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(
            backend.load_identity(address.as_str()).await.unwrap(),
            Some([7; 32])
        );
        assert!(
            backend
                .get_sender_key(name.cache_key())
                .await
                .unwrap()
                .is_some()
        );
        assert!(backend.load_prekey(PREKEY_ID).await.unwrap().is_none());
    }

    /// A lossy clear can drop the gate because the transport is already gone.
    #[tokio::test]
    async fn clear_drops_a_pending_tombstone_gate() {
        let cache = SignalStoreCache::new();
        let a = addr("15550000006");

        cache.put_session(&a, leased_record()).await;
        cache.delete_session(&a).await;
        assert!(cache.needs_pre_wire_flush().await);

        cache.clear().await;
        assert!(!cache.needs_pre_wire_flush().await);
    }

    /// One case per mutation site of either pending set. A site that changes a
    /// set without republishing its flag lets a send publish ciphertext whose
    /// lease is only in memory, so the agreement is checked after each.
    #[tokio::test]
    async fn every_pending_set_mutation_keeps_its_flag_in_agreement() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let a = addr("15550000010");
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        // SessionStoreState::put_with_key
        cache.put_session(&a, leased_record()).await;
        assert!(cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a session lease insert").await;

        // flush: the written batch leaves the set
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a session flush").await;

        // flush: a persisted tombstone leaves the set
        cache.put_session(&a, leased_record()).await;
        cache.delete_session(&a).await;
        assert!(cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a session tombstone").await;
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a session tombstone flush").await;

        // SessionStoreState::clear
        cache.put_session(&a, leased_record()).await;
        cache.clear().await;
        assert_gate_agrees(&cache, "a session clear").await;

        // SenderKeyStoreState::put
        cache.put_sender_key(&name, gated_sender_key()).await;
        assert!(cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a sender-key lease insert").await;

        // flush: the written batch leaves the set
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a sender-key flush").await;

        // flush: a persisted sender-key tombstone leaves the set
        cache.put_sender_key(&name, gated_sender_key()).await;
        cache.delete_sender_key(name.cache_key()).await;
        assert!(cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a sender-key tombstone").await;
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a sender-key tombstone flush").await;

        // delete_sender_key_durable
        cache.put_sender_key(&name, gated_sender_key()).await;
        assert!(cache.needs_pre_wire_flush().await);
        cache
            .delete_sender_key_durable(&name, &backend)
            .await
            .unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a durable sender-key delete").await;

        // SenderKeyStoreState::clear
        cache.put_sender_key(&name, gated_sender_key()).await;
        cache.clear().await;
        assert_gate_agrees(&cache, "a sender-key clear").await;
    }

    /// A durable delete whose backend call failed never persisted the
    /// tombstone, so the gate it inherited must survive the failure.
    #[tokio::test]
    async fn a_failed_durable_delete_keeps_the_gate_closed() {
        let cache = Arc::new(SignalStoreCache::new());
        let backend = Arc::new(DeleteBarrierBackend::new(DeleteTarget::SenderKey));
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        cache.put_sender_key(&name, gated_sender_key()).await;
        assert!(cache.needs_pre_wire_flush().await);

        let deletion = tokio::spawn({
            let cache = cache.clone();
            let backend = backend.clone();
            let name = name.clone();
            async move {
                cache
                    .delete_sender_key_durable(&name, backend.as_ref())
                    .await
            }
        });
        backend.entered.wait().await;
        backend.release.wait().await;
        assert!(deletion.await.expect("delete task").is_err());

        assert!(
            cache.needs_pre_wire_flush().await,
            "an unpersisted tombstone must keep gating the wire"
        );
        assert_gate_agrees(&cache, "a failed durable delete").await;
    }

    /// A failed flush must leave the flag raised, not just the set: the flag is
    /// what the send path actually reads.
    #[tokio::test]
    async fn a_failed_flush_leaves_both_flags_raised() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let name = SenderKeyName::from_parts("g@g.us", "u@s.whatsapp.net:0");

        cache
            .put_session(&addr("15550000011"), leased_record())
            .await;
        cache.put_sender_key(&name, gated_sender_key()).await;

        backend.set_fail_session_writes(true);
        assert!(cache.flush(&backend).await.is_err());
        assert!(cache.session_wire_gate.load(Ordering::Acquire));
        assert_gate_agrees(&cache, "a failed session flush").await;

        backend.set_fail_session_writes(false);
        backend.set_fail_sender_key_writes(true);
        assert!(cache.flush(&backend).await.is_err());
        assert!(cache.sender_key_wire_gate.load(Ordering::Acquire));
        assert_gate_agrees(&cache, "a failed sender-key flush").await;

        backend.set_fail_sender_key_writes(false);
        cache.flush(&backend).await.unwrap();
        assert!(!cache.needs_pre_wire_flush().await);
        assert_gate_agrees(&cache, "a successful flush").await;
    }

    /// A restore that lost the race for the sessions lock is still holding its
    /// record, and with it any lease that record raised. No flag has seen it,
    /// so the query has to fall back to the drain.
    #[tokio::test]
    async fn a_queued_restore_holding_a_lease_keeps_the_gate_closed() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::new();
        let a = addr("15550000012");

        let (record, checkout) = cache.checkout_session(&a, &backend).await.unwrap();
        assert!(record.is_none(), "no session was stored for this address");

        let guard = cache.sessions.lock().await;
        let SessionCheckoutStoreResult::Pending(completion) =
            cache.restore_session_from_checkout(&a, leased_record(), checkout, false)
        else {
            panic!("a contended sessions lock must queue the restore");
        };
        drop(guard);

        assert!(
            !cache.session_wire_gate.load(Ordering::Acquire),
            "the queued lease has not reached the set yet"
        );
        assert!(
            cache.needs_pre_wire_flush().await,
            "a queued restore's lease must keep gating the wire"
        );
        assert!(
            completion.load(Ordering::Acquire),
            "the restore was applied"
        );
        assert_gate_agrees(&cache, "a drained restore").await;
    }

    /// A lease insert that has completed can never be missed by a later query.
    /// The reverse skew is fine: an early `true` only costs a flush.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_completed_lease_insert_is_never_missed_by_a_concurrent_query() {
        const ROUNDS: usize = 64;

        for round in 0..ROUNDS {
            let cache = Arc::new(SignalStoreCache::new());
            let raised = Arc::new(AtomicBool::new(false));
            let writer = tokio::spawn({
                let cache = cache.clone();
                let raised = raised.clone();
                async move {
                    cache
                        .put_session(&addr(&format!("1555001{round:04}")), leased_record())
                        .await;
                    raised.store(true, Ordering::Release);
                }
            });

            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    // Load first: if this reads `true`, the insert is ordered
                    // before the query below, which therefore may not miss it.
                    let announced = raised.load(Ordering::Acquire);
                    let gate = cache.needs_pre_wire_flush().await;
                    if announced {
                        assert!(gate, "a completed lease insert left the gate open");
                        return;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("the writer must finish");
            writer.await.expect("writer task");
        }
    }
}

#[cfg(test)]
#[path = "signal_cache_durability_chaos.rs"]
mod durability_chaos_tests;
