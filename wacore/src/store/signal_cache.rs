use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex as SyncMutex, MutexGuard as SyncMutexGuard};

use anyhow::Result;
use async_lock::Mutex;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};
use rand::RngExt;

use crate::libsignal::protocol::{ProtocolAddress, SenderKeyRecord, SessionRecord};
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
    cache: &mut HashMap<Arc<str>, Option<V>>,
    dirty: &HashSet<Arc<str>>,
    deleted: Option<&HashSet<Arc<str>>>,
    max_entries: usize,
) {
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

/// Default max entries per store before clean entry eviction triggers.
const DEFAULT_MAX_CACHE_ENTRIES: usize = 2_000;

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

/// In-memory write-back cache for Signal protocol state.
/// Keys use `Arc<str>` for O(1) clone. Sessions cached as objects (serialized on flush).
/// Capacity-bounded: every path that grows a store (writes and read-populate
/// misses) evicts non-dirty entries once the high watermark is crossed, trimming
/// back to `max_entries` (amortized O(1) thanks to the slack early-out).
pub struct SignalStoreCache {
    sessions: Mutex<SessionStoreState>,
    session_recovery_generation: AtomicU64,
    has_pending_session_restores: AtomicBool,
    pending_session_restores: SyncMutex<Vec<PendingSessionRestore>>,
    identities: Mutex<ByteStoreState>,
    sender_keys: Mutex<SenderKeyStoreState>,
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
    /// Taken by load_session; has_session treats as present, flush/eviction skip.
    CheckedOut,
}

struct SessionStoreState {
    incarnation: StoreIncarnation,
    checkout_generation: u64,
    cache: HashMap<Arc<str>, SessionEntry>,
    dirty: HashSet<Arc<str>>,
    deleted: HashSet<Arc<str>>,
    /// Sessions whose raised counter reservation has not reached the backend
    /// yet. While any address is here, an outbound ciphertext may be relying
    /// on a lease that only exists in memory, so the send path must flush
    /// before the wire. Entries leave only when a flush actually persists
    /// them or their tombstone. Always a subset of `dirty` + `deleted`, so
    /// eviction can never drop a pending entry.
    reservation_pending: HashSet<Arc<str>>,
}

impl SessionStoreState {
    fn new(incarnation: StoreIncarnation) -> Self {
        Self {
            incarnation,
            checkout_generation: 0,
            cache: HashMap::new(),
            dirty: HashSet::new(),
            deleted: HashSet::new(),
            reservation_pending: HashSet::new(),
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

    fn discard(&mut self, incarnation: StoreIncarnation, generation: u64) {
        self.clear();
        self.incarnation = incarnation;
        self.checkout_generation = generation;
    }

    fn evict_if_needed(&mut self, max_entries: usize) {
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
                SessionEntry::CheckedOut => continue, // never evict checked-out
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
    record: SessionRecord,
    generation: u64,
}

// === Sender key object cache (same pattern as sessions) ===

struct SenderKeyStoreState {
    incarnation: StoreIncarnation,
    // `Arc`-wrapped so a warm `get_sender_key` (the per-send peek reads and the
    // per-decrypt load) bumps a refcount instead of deep-cloning the record's
    // `VecDeque<SenderKeyState>` with up to `MAX_MESSAGE_KEYS` message keys each.
    cache: HashMap<Arc<str>, Option<Arc<SenderKeyRecord>>>,
    dirty: HashSet<Arc<str>>,
    /// Chains advanced by an outbound encrypt and not yet persisted; the send
    /// path must flush before the wire while any entry is here. Decrypt-side
    /// dirtiness deliberately does NOT enter this set (it re-derives forward),
    /// so unrelated group receives never force a sync flush onto a DM send.
    wire_gate_pending: HashSet<Arc<str>>,
}

impl SenderKeyStoreState {
    fn new(incarnation: StoreIncarnation) -> Self {
        Self {
            incarnation,
            cache: HashMap::new(),
            dirty: HashSet::new(),
            wire_gate_pending: HashSet::new(),
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
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.dirty.clear();
        self.wire_gate_pending.clear();
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
    cache: HashMap<Arc<str>, Option<Arc<[u8]>>>,
    dirty: HashSet<Arc<str>>,
    deleted: HashSet<Arc<str>>,
}

impl ByteStoreState {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
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
        Self {
            sessions: Mutex::new(SessionStoreState::new(incarnation)),
            session_recovery_generation: AtomicU64::new(0),
            has_pending_session_restores: AtomicBool::new(false),
            pending_session_restores: SyncMutex::new(Vec::new()),
            identities: Mutex::new(ByteStoreState::new()),
            sender_keys: Mutex::new(SenderKeyStoreState::new(incarnation)),
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
            generation,
        } in pending.drain(..)
        {
            if generation != state.checkout_generation {
                continue;
            }
            let address = state
                .cache
                .get_key_value(address.as_ref())
                .map_or(address, |(cached, _)| cached.clone());
            state.put_with_key(address, record);
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
        generation: u64,
    ) -> bool {
        if let Some(mut state) = self.try_lock_sessions() {
            if generation != state.checkout_generation {
                return false;
            }
            state.put(address.as_str(), record);
            state.evict_if_needed(self.max_entries);
            return true;
        }

        let mut pending = self.pending_session_restores();
        if generation != self.session_recovery_generation.load(Ordering::Acquire) {
            return false;
        }
        pending.push(PendingSessionRestore {
            address: Arc::from(address.as_str()),
            record,
            generation,
        });
        self.has_pending_session_restores
            .store(true, Ordering::Release);
        true
    }

    /// Whether any session or identity is known for `user` (across device ids),
    /// checking the in-memory cache first, then the durable backend. Lets a
    /// caller skip a per-device migration scan for a user we've never had Signal
    /// state with. Conservative on the cache side: any matching key counts
    /// (even a stale/checked-out marker), so it never reports "none" when state
    /// might exist.
    pub async fn has_state_for_user(&self, user: &str, backend: &dyn SignalStore) -> Result<bool> {
        fn matches(addr: &str, user: &str) -> bool {
            addr.strip_prefix(user)
                .is_some_and(|rest| rest.starts_with('@') || rest.starts_with(':'))
        }
        {
            let state = self.lock_sessions().await;
            if state.cache.keys().any(|k| matches(k, user)) {
                return Ok(true);
            }
        }
        {
            let state = self.identities.lock().await;
            if state.cache.keys().any(|k| matches(k, user)) {
                return Ok(true);
            }
        }
        Ok(backend.has_signal_state_for_user(user).await?)
    }

    // === Sessions (object cache — serialize only during flush) ===

    /// Takes ownership of the cached session, leaving a `CheckedOut` marker.
    /// Callers must return the record with [`put_session`] after use.
    pub async fn get_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<Option<SessionRecord>> {
        self.checkout_session(address, backend)
            .await
            .map(|(record, _)| record)
    }

    /// The generation prevents a cancelled checkout from surviving a lossy reset.
    #[doc(hidden)]
    pub async fn checkout_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<(Option<SessionRecord>, u64)> {
        let key = address.as_str();
        {
            let mut state = self.lock_sessions().await;
            let generation = state.checkout_generation;
            if let Some(entry) = state.cache.get_mut(key) {
                if matches!(entry, SessionEntry::Present(_)) {
                    let SessionEntry::Present(record) =
                        std::mem::replace(entry, SessionEntry::CheckedOut)
                    else {
                        unreachable!()
                    };
                    // Unique unless a peek's Arc is still alive (short-lived
                    // inspection paths), so this is a move, not a clone.
                    return Ok((
                        Some(Arc::try_unwrap(record).unwrap_or_else(|arc| (*arc).clone())),
                        generation,
                    ));
                }
                return Ok((None, generation));
            }
        }
        // Backend I/O outside the lock
        let backend_result = backend.get_session(key).await?;
        let mut state = self.lock_sessions().await;
        let generation = state.checkout_generation;
        match backend_result {
            Some(bytes) => {
                if state.cache.contains_key(key) {
                    // Another task populated this slot while we were loading;
                    // defer to whatever they wrote (Present, CheckedOut, etc).
                    // Deserialize and return without caching to avoid conflict.
                    return Ok((
                        Some(SessionRecord::deserialize_for_store(
                            &bytes,
                            &state.incarnation,
                        )?),
                        generation,
                    ));
                }
                let record = SessionRecord::deserialize_for_store(&bytes, &state.incarnation)?;
                state.cache.insert(Arc::from(key), SessionEntry::CheckedOut);
                state.evict_if_needed(self.max_entries);
                Ok((Some(record), generation))
            }
            None => {
                if !state.cache.contains_key(key) {
                    state.cache.insert(Arc::from(key), SessionEntry::Absent);
                    state.evict_if_needed(self.max_entries);
                }
                Ok((None, generation))
            }
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
        match backend_result {
            Some(bytes) => {
                let record = Arc::new(SessionRecord::deserialize_for_store(
                    &bytes,
                    &state.incarnation,
                )?);
                if !state.cache.contains_key(key) {
                    state
                        .cache
                        .insert(Arc::from(key), SessionEntry::Present(record.clone()));
                    state.evict_if_needed(self.max_entries);
                }
                Ok(Some(record))
            }
            None => {
                if !state.cache.contains_key(key) {
                    state.cache.insert(Arc::from(key), SessionEntry::Absent);
                    state.evict_if_needed(self.max_entries);
                }
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
        state
            .cache
            .get(address.as_str())
            .map(|entry| !matches!(entry, SessionEntry::Absent))
    }

    pub async fn delete_session(&self, address: &ProtocolAddress) {
        let mut state = self.lock_sessions().await;
        state.delete(address.as_str());
    }

    /// Non-destructive existence check (`CheckedOut` counts as present).
    /// Backend misses are negative-cached; hits are not cached to skip
    /// deserialization (the subsequent `get_session` will cache on demand).
    pub async fn has_session(
        &self,
        address: &ProtocolAddress,
        backend: &dyn SignalStore,
    ) -> Result<bool> {
        let key = address.as_str();
        {
            let state = self.lock_sessions().await;
            if let Some(entry) = state.cache.get(key) {
                return Ok(!matches!(entry, SessionEntry::Absent));
            }
        }
        // Backend I/O outside the lock
        let exists = backend.has_session(key).await?;
        if !exists {
            let mut state = self.lock_sessions().await;
            // Re-check: another task may have populated the cache
            if !state.cache.contains_key(key) {
                state.cache.insert(Arc::from(key), SessionEntry::Absent);
                state.evict_if_needed(self.max_entries);
            }
        }
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
                    Some(SessionEntry::CheckedOut)
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
                            Some(SessionEntry::CheckedOut) => Some(false),
                            Some(SessionEntry::Absent) | None => None,
                        };
                        let durable = match durable {
                            Some(d) => d,
                            None => backend.has_session(addr.as_ref()).await?,
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
    /// gated on durability: a session counter lease was raised, or a
    /// sender-key chain advanced via encrypt, and neither has reached the
    /// backend. The send path flushes synchronously only while this holds;
    /// everything else (decrypt advances, identities) safely rides the
    /// coalesced write-behind.
    pub async fn needs_pre_wire_flush(&self) -> bool {
        if !self.lock_sessions().await.reservation_pending.is_empty() {
            return true;
        }
        !self.sender_keys.lock().await.wire_gate_pending.is_empty()
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
                        SessionEntry::Absent | SessionEntry::CheckedOut => None,
                    }
                })
                .collect();
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
                .sum();
            CollectionStats::new(i.cache.len() as u64, bytes as u64)
        };

        let (sk_count, sk_keys_len, sk_recs): (u64, usize, Vec<_>) = {
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
            (sk.cache.len() as u64, keys_len, recs)
        };
        let sk_bytes: usize =
            sk_keys_len + sk_recs.iter().map(|r| r.estimated_size()).sum::<usize>();
        let sender_keys = CollectionStats::new(sk_count, sk_bytes as u64);

        (sessions, identities, sender_keys)
    }

    /// A lossy discard must invalidate exact-reload trust.
    pub async fn clear(&self) {
        self.clear_with_incarnation(new_store_incarnation()).await;
    }

    async fn clear_with_incarnation(&self, incarnation: StoreIncarnation) {
        {
            let mut sessions = self.sessions.lock().await;
            let mut pending = self.pending_session_restores();
            let generation = self
                .session_recovery_generation
                .fetch_add(1, Ordering::AcqRel)
                .wrapping_add(1);
            pending.clear();
            self.has_pending_session_restores
                .store(false, Ordering::Release);
            sessions.discard(incarnation, generation);
        }
        self.identities.lock().await.clear();
        self.sender_keys.lock().await.discard(incarnation);
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
            sessions.clear();
            self.removed_prekeys.lock().await.clear();
        }
        drop(sessions);

        let mut identities = self.identities.lock().await;
        if identities.dirty.is_empty() && identities.deleted.is_empty() {
            identities.clear();
        }
        drop(identities);

        let mut sender_keys = self.sender_keys.lock().await;
        if sender_keys.dirty.is_empty() && sender_keys.wire_gate_pending.is_empty() {
            sender_keys.clear();
        }
    }
}

#[cfg(test)]
mod sender_key_lock_tests {
    use super::*;
    use crate::libsignal::store::sender_key_name::SenderKeyName;

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
        let addr = ProtocolAddress::new("15550009999".to_string(), 1.into());

        assert!(
            cache
                .try_put_session(&addr, SessionRecord::new_fresh())
                .is_ok(),
            "uncontended try_put_session must succeed"
        );

        assert_eq!(cache.try_has_session(&addr), Some(true));
        cache.flush(&backend).await.unwrap();
        assert!(
            crate::store::traits::SignalStore::get_session(&backend, addr.as_str())
                .await
                .unwrap()
                .is_some(),
            "flush must persist a session stored via the fast path"
        );
    }

    #[tokio::test]
    async fn try_session_paths_fall_back_under_contention() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550009999".to_string(), 1.into());

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
        drop(guard);

        assert_eq!(
            cache.try_has_session(&addr),
            None,
            "unknown entry must defer to the async path"
        );
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
        let addr = ProtocolAddress::new("15550008888".to_string(), 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;

        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();
        let sessions = cache.sessions.lock().await;
        assert!(cache.restore_session_from_checkout(
            &addr,
            record.expect("checked-out record"),
            generation,
        ));
        assert_eq!(cache.pending_session_restores().len(), 1);
        drop(sessions);

        cache.flush(&backend).await.unwrap();
        assert!(
            crate::store::traits::SignalStore::get_session(&backend, addr.as_str())
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
        let addr = ProtocolAddress::new("15550007777".to_string(), 1.into());
        cache.put_session(&addr, SessionRecord::new_fresh()).await;
        let (record, generation) = cache.checkout_session(&addr, &backend).await.unwrap();

        cache.clear().await;
        assert!(!cache.restore_session_from_checkout(
            &addr,
            record.expect("checked-out record"),
            generation,
        ));
        assert!(cache.peek_session(&addr, &backend).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn try_has_session_reports_known_absent() {
        let cache = SignalStoreCache::new();
        let addr = ProtocolAddress::new("15550009999".to_string(), 1.into());

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
        let addr = ProtocolAddress::new("15550009999".to_string(), 1.into());
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
        ProtocolAddress::new("bob".to_string(), 1.into())
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

        let addr_a = ProtocolAddress::new("alice".to_string(), 1.into());
        let addr_b = ProtocolAddress::new("bob".to_string(), 1.into());

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

        let addr_a = ProtocolAddress::new("alice".to_string(), 1.into());
        let addr_b = ProtocolAddress::new("bob".to_string(), 1.into());

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
        ProtocolAddress::new(format!("user{i}@s.whatsapp.net"), DeviceId::new(0))
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
                matches!(entry, Some(SessionEntry::CheckedOut)),
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

    fn leased_session() -> SessionRecord {
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
    async fn dm_clean_reload_is_exact_but_new_cache_burns_the_lease() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = ProtocolAddress::new("15550001001".to_string(), 1.into());
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

    #[tokio::test]
    async fn incomplete_session_flush_retains_newer_state_and_fails_closed_on_recovery() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries_and_incarnation(
            DEFAULT_MAX_CACHE_ENTRIES,
            [0xA1; 16],
        );
        let address = ProtocolAddress::new("15550001002".to_string(), 1.into());
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
                Some(SessionEntry::CheckedOut)
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
        ProtocolAddress::new(user.to_string(), 1.into())
    }

    fn leased_record() -> SessionRecord {
        let mut record = SessionRecord::new_fresh();
        record.reserve_sender_chain_counters(0);
        record
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
}
