//! Portable in-process cache: the client's sole cache backend, on every target
//! including wasm32.
//!
//! TTL/TTI use the monotonic [`wacore::time::Instant`] (not the wall clock),
//! so expiry is immune to system-clock jumps. Provides capacity + TTL/TTI
//! eviction and an async, single-flight `get_with`.
//!
//! `get_with` / `get_with_by_ref` are single-flight: concurrent inits for the
//! same missing key run the initializer once.

use async_lock::{Mutex as AsyncMutex, RwLock};
use hashbrown::HashTable;
use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash, RandomState};
use std::sync::Arc;
use std::time::Duration;
use wacore::runtime::BoxFuture;
use wacore::sync_marker::MaybeSend;
use wacore::time::Instant;

/// How far an entry's recorded access time may lag the real access before
/// [`PortableCache::get`] renews it under the write lock, as a divisor of the
/// TTI.
///
/// Renewal only exists to keep an in-use entry from idling out, so it does not
/// have to be exact. Skipping it leaves the stamp at most `tti / this` behind,
/// which can only expire an entry that much *early*, never keep an expired one
/// alive. In exchange a hot key leaves the read path only at a window boundary
/// rather than on every lookup, so a read-saturated cache no longer serialises
/// behind a writer.
const TTI_RENEWAL_DIVISOR: u32 = 16;

struct CacheEntry<V> {
    value: V,
    // Monotonic instants (not wall-clock) so TTL/TTI are immune to clock jumps,
    // matching moka's timer semantics.
    inserted_at: Instant,
    last_accessed_at: Instant,
    /// Eviction sequence number; the key for this entry in `CacheInner::order`.
    seq: u64,
    /// Set by every hit, cleared by the eviction scan: the second-chance bit
    /// of CLOCK. A hit costs one relaxed store under the read lock; the
    /// reordering it earns is paid by the eviction that would have dropped
    /// the entry, which re-queues it at the back instead. Moving the entry
    /// on the hit itself needed the write lock, and once every entry was
    /// being moved on every pass a warm read cost three times what it had.
    referenced: portable_atomic::AtomicBool,
}

/// One table slot: the key, its hash, and the entry.
///
/// The hash is kept so the table can grow without re-hashing every key and so
/// the FIFO side can address a slot by `(hash, seq)` instead of holding a
/// second copy of the key. Before this, `order` was a `BTreeMap<u64, K>`: every
/// entry carried its key twice, and for a `String`, `Jid` or `SenderMessageId`
/// key that second copy was a second heap allocation held for the entry's
/// whole lifetime, across every cache the client keeps.
struct Slot<K, V> {
    key: K,
    hash: u64,
    entry: CacheEntry<V>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CapacityStats {
    pub entries: u64,
    pub evictions: u64,
    pub eviction_blocks: u64,
}

/// Portable, runtime-agnostic in-process cache.
///
/// - Max capacity with second-chance (CLOCK) eviction: least recently used
///   leaves first, and a hit costs one atomic store
/// - TTL (time-to-live) and TTI (time-to-idle)
/// - Single-flight `get_with`
///
/// Uses `async_lock::RwLock` (runtime-agnostic, works on wasm32), so this
/// compiles and runs on every target.
///
/// Both lazy and best-effort: expired entries are only removed lazily on
/// access or in `run_pending_tasks`. `entry_count` may include
/// expired-but-not-yet-evicted entries.
pub struct PortableCache<K, V> {
    inner: Arc<RwLock<CacheInner<K, V>>>,
    /// Shared single-flight init-lock registry (see `InitLocks`).
    init_locks: Arc<InitLocks>,
    max_capacity: Option<u64>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
    /// See [`PortableCacheBuilder::evict_guard`].
    evict_guard: Option<fn(&V) -> bool>,
    #[cfg(test)]
    tti_renewals: Arc<portable_atomic::AtomicU64>,
}

/// What the eviction walk asks the slot table, through one `dyn` hook so the
/// walk itself is compiled once. The walk is the bulk of the eviction code
/// and every `K`/`V` pair would otherwise carry its own copy; the table
/// probes it wraps are small.
enum ClockOp {
    /// Is the entry held, hit since the last pass, or a victim?
    Classify { seq: u64, hash: u64 },
    /// Re-key the entry to `fresh`. Answers `Victim` when found, `Skip` when
    /// the slot is gone.
    Reseq { seq: u64, hash: u64, fresh: u64 },
}

enum ClockVerdict {
    Skip,
    SecondChance,
    Victim,
}

enum ClockWalk {
    Victim {
        seq: u64,
        hash: u64,
    },
    /// No victim, but at least one entry spent its second chance; the next
    /// pass over the same entries finds them unreferenced.
    Requeued,
    /// Nothing evictable at all.
    Blocked,
}

/// One pass of the second-chance walk over `order`, oldest first: the first
/// unreferenced, unheld entry is the victim; entries hit since the last pass
/// have their bit cleared (by `probe`) and re-queue behind everything
/// inserted so far, so each can earn at most one more pass per hit and the
/// scan cannot cycle.
fn clock_walk(
    order: &mut BTreeMap<u64, u64>,
    next_seq: &mut u64,
    probe: &mut dyn FnMut(ClockOp) -> ClockVerdict,
) -> ClockWalk {
    let mut victim = None;
    let mut second_chance = Vec::new();
    for (&seq, &hash) in order.iter() {
        match probe(ClockOp::Classify { seq, hash }) {
            ClockVerdict::Skip => continue,
            ClockVerdict::SecondChance => second_chance.push((seq, hash)),
            ClockVerdict::Victim => {
                victim = Some((seq, hash));
                break;
            }
        }
    }
    let requeued = !second_chance.is_empty();
    for (seq, hash) in second_chance {
        let fresh = *next_seq;
        *next_seq += 1;
        order.remove(&seq);
        if matches!(
            probe(ClockOp::Reseq { seq, hash, fresh }),
            ClockVerdict::Victim
        ) {
            order.insert(fresh, hash);
        }
    }
    match victim {
        Some((seq, hash)) => ClockWalk::Victim { seq, hash },
        None if requeued => ClockWalk::Requeued,
        None => ClockWalk::Blocked,
    }
}

struct CacheInner<K, V> {
    /// Hashes keys once at insert; lookups with a borrowed `Q` hash through
    /// the same state, which the `Borrow` contract keeps consistent with `K`.
    hasher: RandomState,
    table: HashTable<Slot<K, V>>,
    /// Eviction order, `seq -> hash`, oldest first; an entry given a second
    /// chance is re-keyed to the back. Eviction walks from the front (O(log n)
    /// per step) and a targeted `remove_key` is O(log n) via the entry's
    /// stored `seq`. The hash plus the seq find the slot in `table`, so no key
    /// is stored here.
    ///
    /// Left empty for a cache that has no capacity bound: nothing could ever
    /// pop it, so maintaining it would spend a `BTreeMap` node per entry on a
    /// structure that is never read. The unbounded LID↔PN caches hold tens
    /// of thousands of entries for the life of the process.
    order: BTreeMap<u64, u64>,
    track_order: bool,
    /// Next FIFO sequence to assign.
    next_seq: u64,
    capacity_evictions: u64,
    capacity_eviction_blocks: u64,
}

impl<K, V> CacheInner<K, V>
where
    K: Hash + Eq + Clone,
{
    fn new(track_order: bool) -> Self {
        Self {
            hasher: RandomState::new(),
            table: HashTable::new(),
            order: BTreeMap::new(),
            track_order,
            next_seq: 0,
            capacity_evictions: 0,
            capacity_eviction_blocks: 0,
        }
    }

    #[inline]
    fn hash_of<Q: Hash + ?Sized>(&self, key: &Q) -> u64 {
        self.hasher.hash_one(key)
    }

    #[inline]
    fn len(&self) -> usize {
        self.table.len()
    }

    fn get<Q>(&self, key: &Q) -> Option<&CacheEntry<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_of(key);
        self.table
            .find(hash, |slot| slot.key.borrow() == key)
            .map(|slot| &slot.entry)
    }

    fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut CacheEntry<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_of(key);
        self.table
            .find_mut(hash, |slot| slot.key.borrow() == key)
            .map(|slot| &mut slot.entry)
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &CacheEntry<V>)> {
        self.table.iter().map(|slot| (&slot.key, &slot.entry))
    }

    /// Bytes the table and the eviction order themselves hold, on top of the
    /// entries: hashbrown's buckets (a slot plus a control byte each, at its
    /// power-of-two capacity) and one B-tree node share per ordered entry.
    fn structural_bytes(&self) -> usize {
        // A `BTreeMap<u64, u64>` leaf holds up to 11 pairs and averages
        // roughly two thirds full, so the per-entry share is a little over
        // the pair itself; 8 bytes of overhead is the conservative round-up.
        const ORDER_NODE_SHARE: usize = 2 * size_of::<u64>() + 8;
        let order = if self.track_order {
            self.order.len() * ORDER_NODE_SHARE
        } else {
            0
        };
        wacore::stats::hash_table_bytes(self.table.capacity(), size_of::<Slot<K, V>>()) + order
    }

    fn clear(&mut self) {
        self.table.clear();
        self.order.clear();
    }

    /// Borrowed removal: the `order` side is keyed by the entry's own `seq`,
    /// so nothing here ever needs an owned `K`, and callers with a `&str` or
    /// `&Jid` need not clone the key just to delete it.
    fn remove_key<Q>(&mut self, key: &Q) -> Option<CacheEntry<V>>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let hash = self.hash_of(key);
        let (slot, _) = self
            .table
            .find_entry(hash, |slot| slot.key.borrow() == key)
            .ok()?
            .remove();
        self.order.remove(&slot.entry.seq);
        Some(slot.entry)
    }

    /// Remove the slot the FIFO side named by `(hash, seq)`.
    fn remove_by_seq(&mut self, hash: u64, seq: u64) -> bool {
        self.table
            .find_entry(hash, |slot| slot.entry.seq == seq)
            .ok()
            .map(|occupied| occupied.remove())
            .is_some()
    }

    /// Evict until below `cap`: oldest first, except that an entry hit since
    /// it was last considered gets a second chance (its bit is cleared and it
    /// re-queues at the back), so what leaves is the least recently *used*
    /// entry rather than the least recently inserted. With an `evict_guard`,
    /// entries the guard reports as held are skipped so a later lookup cannot
    /// mint a duplicate; if every entry is held, the cache runs over capacity
    /// for a while rather than dropping a live one (see
    /// [`PortableCacheBuilder::evict_guard`]).
    fn evict_to_capacity(&mut self, cap: u64, evict_guard: Option<fn(&V) -> bool>) {
        while self.table.len() as u64 >= cap {
            let table = &mut self.table;
            let walk = clock_walk(&mut self.order, &mut self.next_seq, &mut |op| match op {
                ClockOp::Classify { seq, hash } => {
                    match table.find(hash, |slot| slot.entry.seq == seq) {
                        None => ClockVerdict::Skip,
                        Some(slot)
                            if evict_guard
                                .is_some_and(|is_evictable| !is_evictable(&slot.entry.value)) =>
                        {
                            ClockVerdict::Skip
                        }
                        Some(slot)
                            if slot
                                .entry
                                .referenced
                                .swap(false, std::sync::atomic::Ordering::Relaxed) =>
                        {
                            ClockVerdict::SecondChance
                        }
                        Some(_) => ClockVerdict::Victim,
                    }
                }
                ClockOp::Reseq { seq, hash, fresh } => {
                    match table.find_mut(hash, |slot| slot.entry.seq == seq) {
                        Some(slot) => {
                            slot.entry.seq = fresh;
                            ClockVerdict::Victim
                        }
                        None => ClockVerdict::Skip,
                    }
                }
            });
            match walk {
                ClockWalk::Victim { seq, hash } => {
                    self.order.remove(&seq);
                    if self.remove_by_seq(hash, seq) {
                        self.capacity_evictions = self.capacity_evictions.saturating_add(1);
                    }
                }
                // Every candidate had a chance to spend; the next pass over
                // the same entries finds them unreferenced.
                ClockWalk::Requeued => continue,
                ClockWalk::Blocked => {
                    self.capacity_eviction_blocks = self.capacity_eviction_blocks.saturating_add(1);
                    break;
                }
            }
        }
    }

    /// Insert a brand-new entry (the caller has already confirmed the key is
    /// absent), evicting the oldest entries first if at capacity. Assigns and
    /// records the FIFO sequence.
    fn insert_new(
        &mut self,
        key: K,
        value: V,
        now: Instant,
        max_capacity: Option<u64>,
        evict_guard: Option<fn(&V) -> bool>,
    ) {
        if let Some(cap) = max_capacity {
            self.evict_to_capacity(cap, evict_guard);
        }

        let seq = self.next_seq;
        self.next_seq += 1;
        let hash = self.hash_of(&key);
        if self.track_order {
            self.order.insert(seq, hash);
        }
        self.table.insert_unique(
            hash,
            Slot {
                key,
                hash,
                entry: CacheEntry {
                    value,
                    inserted_at: now,
                    last_accessed_at: now,
                    seq,
                    referenced: portable_atomic::AtomicBool::new(false),
                },
            },
            |slot| slot.hash,
        );
    }

    /// Drop expired entries and the FIFO records that named them.
    fn retain_unexpired(&mut self, mut is_expired: impl FnMut(&CacheEntry<V>) -> bool) {
        let table = &mut self.table;
        let order = &mut self.order;
        table.retain(|slot| !is_expired(&slot.entry));
        order.retain(|seq, hash| table.find(*hash, |slot| slot.entry.seq == *seq).is_some());
    }
}

/// Single-flight init-lock registry, keyed by key hash instead of the key
/// itself so it is compiled once for every `<K, V>` cache in the binary. A
/// hash collision only makes two distinct keys share one init lock — they
/// serialize their initializers, and the double-checked `get` inside
/// `get_with_slow` keeps the result correct — so the key never needs to be
/// stored or cloned here.
struct InitLocks {
    /// Shared across cache clones so a key hashes identically everywhere.
    hasher: RandomState,
    map: AsyncMutex<HashMap<u64, Arc<AsyncMutex<()>>>>,
}

impl InitLocks {
    fn new() -> Self {
        Self {
            hasher: RandomState::new(),
            map: AsyncMutex::new(HashMap::new()),
        }
    }

    fn hash_of<Q: Hash + ?Sized>(&self, key: &Q) -> u64 {
        self.hasher.hash_one(key)
    }

    async fn acquire(&self, hash: u64) -> Arc<AsyncMutex<()>> {
        let mut locks = self.map.lock().await;
        locks
            .entry(hash)
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone()
    }

    /// Drop a single-flight init lock once no other caller is using it, so the
    /// registry can't grow without bound across distinct keys (it is otherwise
    /// only reclaimed by `run_pending_tasks`, which several hot `get_with`
    /// caches never call). `strong_count <= 2` means only this caller's clone
    /// and the map entry remain; the `ptr_eq` guard avoids dropping a newer
    /// lock a racing caller may have inserted.
    async fn reclaim(&self, hash: u64, init_mutex: &Arc<AsyncMutex<()>>) {
        let mut locks = self.map.lock().await;
        if Arc::strong_count(init_mutex) <= 2
            && let Some(existing) = locks.get(&hash)
            && Arc::ptr_eq(existing, init_mutex)
        {
            locks.remove(&hash);
        }
    }

    /// Best-effort synchronous reclaim for cancellation paths: `try_lock` so it
    /// can run inside `Drop`. Contention here only defers cleanup to the next
    /// reclaim on this hash or to `run_pending_tasks`.
    fn reclaim_now(&self, hash: u64, init_mutex: &Arc<AsyncMutex<()>>) {
        if let Some(mut locks) = self.map.try_lock()
            && Arc::strong_count(init_mutex) <= 2
            && let Some(existing) = locks.get(&hash)
            && Arc::ptr_eq(existing, init_mutex)
        {
            locks.remove(&hash);
        }
    }

    async fn retain_active(&self) {
        let mut locks = self.map.lock().await;
        locks.retain(|_, v| Arc::strong_count(v) > 1);
    }
}

/// Reclaims a single-flight init lock if `get_with_slow` is cancelled mid-init
/// (caller timeout/abort), so cancelled fills can't grow the registry until
/// `run_pending_tasks`. The success path disarms it and runs the awaited
/// (guaranteed) reclaim instead.
struct InitLockCleanup<'a> {
    registry: &'a InitLocks,
    hash: u64,
    lock: Option<Arc<AsyncMutex<()>>>,
}

impl InitLockCleanup<'_> {
    fn disarm(&mut self) -> Arc<AsyncMutex<()>> {
        self.lock.take().expect("init-lock cleanup disarmed twice")
    }
}

impl Drop for InitLockCleanup<'_> {
    fn drop(&mut self) {
        if let Some(lock) = self.lock.take() {
            self.registry.reclaim_now(self.hash, &lock);
        }
    }
}

// -- Builder --

pub struct PortableCacheBuilder<K, V> {
    max_capacity: Option<u64>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
    evict_guard: Option<fn(&V) -> bool>,
    _marker: std::marker::PhantomData<fn(K, V)>,
}

impl<K, V> PortableCacheBuilder<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    fn new() -> Self {
        Self {
            max_capacity: None,
            ttl: None,
            tti: None,
            evict_guard: None,
            _marker: std::marker::PhantomData,
        }
    }

    /// Protect entries a live task still holds from capacity eviction: `guard`
    /// returns `true` when a value is safe to evict. For an `Arc<Mutex>` lock cache,
    /// pass `|v| Arc::strong_count(v) <= 1`, so an entry held elsewhere is never
    /// FIFO-evicted and re-minted (which would let two writers race the resource).
    pub fn evict_guard(mut self, guard: fn(&V) -> bool) -> Self {
        self.evict_guard = Some(guard);
        self
    }

    pub fn max_capacity(mut self, cap: u64) -> Self {
        self.max_capacity = Some(cap);
        self
    }

    pub fn time_to_live(mut self, ttl: Duration) -> Self {
        self.ttl = Some(ttl);
        self
    }

    /// Expire an entry once it has gone this long without a lookup.
    ///
    /// Approximate: a lookup refreshes the idle deadline lazily, so an entry can
    /// expire up to a sixteenth of `tti` early. It is never served late.
    pub fn time_to_idle(mut self, tti: Duration) -> Self {
        self.tti = Some(tti);
        self
    }

    /// # Panics
    ///
    /// If an [`evict_guard`](Self::evict_guard) is combined with a TTL or TTI.
    pub fn build(self) -> PortableCache<K, V> {
        // An evict_guard marks a cache of live coordination objects. Expiry
        // does not consult the guard, so a timeout would drop an entry a task
        // still holds and let the next lookup mint a duplicate of it.
        assert!(
            self.evict_guard.is_none() || (self.ttl.is_none() && self.tti.is_none()),
            "a cache with an evict_guard holds live coordination objects and must not expire by time"
        );

        PortableCache {
            inner: Arc::new(RwLock::new(CacheInner::new(
                self.max_capacity.is_some_and(|cap| cap != u64::MAX),
            ))),
            init_locks: Arc::new(InitLocks::new()),
            max_capacity: self.max_capacity,
            ttl: self.ttl,
            tti: self.tti,
            evict_guard: self.evict_guard,
            #[cfg(test)]
            tti_renewals: Arc::new(portable_atomic::AtomicU64::new(0)),
        }
    }
}

// -- PortableCache impl --

impl<K, V> PortableCache<K, V>
where
    K: Hash + Eq + Clone + Send + Sync + 'static,
    V: Clone + Send + Sync + 'static,
{
    pub fn builder() -> PortableCacheBuilder<K, V> {
        PortableCacheBuilder::new()
    }

    /// Read the monotonic clock only for caches that can expire entries.
    /// Non-expiring caches use a stable sentinel because their timestamps are
    /// never observed, avoiding unnecessary clock reads on every operation.
    #[inline]
    fn entry_time(&self) -> Instant {
        if self.ttl.is_some() || self.tti.is_some() {
            Instant::now()
        } else {
            Instant::ZERO
        }
    }

    fn is_expired(&self, entry: &CacheEntry<V>, now: Instant) -> bool {
        if let Some(ttl) = self.ttl
            && now.saturating_duration_since(entry.inserted_at) >= ttl
        {
            return true;
        }
        if let Some(tti) = self.tti
            && now.saturating_duration_since(entry.last_accessed_at) >= tti
        {
            return true;
        }
        false
    }

    /// Whether `entry`'s access stamp has aged past [`TTI_RENEWAL_DIVISOR`]'s
    /// tolerance and is worth pushing forward under the write lock. A cache
    /// without TTI never renews.
    fn needs_tti_renewal(&self, entry: &CacheEntry<V>, now: Instant) -> bool {
        self.tti.is_some_and(|tti| {
            now.saturating_duration_since(entry.last_accessed_at) >= tti / TTI_RENEWAL_DIVISOR
        })
    }

    pub async fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let (value, renew_at) = {
            let guard = self.inner.read().await;
            let entry = guard.get(key)?;
            // Read the clock after the lookup: a miss has no timestamp to
            // compare, and lookups that miss are a large share of the calls
            // (every negative registry probe, every warm-up).
            let now = self.entry_time();
            if self.is_expired(entry, now) {
                // Identity of the entry judged expired: `seq` moves if the slot
                // was re-inserted, `inserted_at` if the value was rewritten in
                // place. Removing without it could drop a replacement written
                // while the guard was down and judge it by a stale `now`.
                let observed = (entry.seq, entry.inserted_at);
                drop(guard);
                let mut wguard = self.inner.write().await;
                if let Some(e) = wguard.get(key)
                    && (e.seq, e.inserted_at) == observed
                    && self.is_expired(e, now)
                {
                    wguard.remove_key(key);
                }
                return None;
            }
            if guard.track_order {
                entry
                    .referenced
                    .store(true, std::sync::atomic::Ordering::Relaxed);
            }
            (
                entry.value.clone(),
                self.needs_tti_renewal(entry, now).then_some(now),
            )
        };

        if let Some(now) = renew_at {
            let mut guard = self.inner.write().await;
            // Re-decide under the lock: the key may have been invalidated (the
            // miss leaves it that way) or already refreshed by a racing renewal
            // or insert, whose newer stamp this lookup must leave alone.
            if let Some(entry) = guard.get_mut(key)
                && self.needs_tti_renewal(entry, now)
            {
                entry.last_accessed_at = now;
                #[cfg(test)]
                self.tti_renewals
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }

        Some(value)
    }

    pub async fn insert(&self, key: K, value: V) {
        let now = self.entry_time();
        let mut guard = self.inner.write().await;

        if let Some(entry) = guard.get_mut(&key) {
            entry.value = value;
            entry.inserted_at = now;
            entry.last_accessed_at = now;
            return;
        }

        if self.max_capacity == Some(0) {
            return;
        }

        guard.insert_new(key, value, now, self.max_capacity, self.evict_guard);
    }

    /// Atomically derive and optionally store a value from the current entry.
    ///
    /// The closure runs synchronously under the cache's existing write lock.
    /// Returning `None` leaves an existing value unchanged and keeps a missing
    /// key absent. The key is cloned only when the operation inserts a new
    /// entry.
    pub async fn upsert_with_by_ref<Q, R>(
        &self,
        key: &Q,
        update: impl FnOnce(Option<&V>) -> (Option<V>, R),
    ) -> R
    where
        K: Borrow<Q>,
        Q: ToOwned<Owned = K> + Hash + Eq + ?Sized,
    {
        let now = self.entry_time();
        let mut guard = self.inner.write().await;

        if guard
            .get(key)
            .is_some_and(|entry| self.is_expired(entry, now))
        {
            guard.remove_key(key);
        }

        let (next, result) = update(guard.get(key).map(|entry| &entry.value));
        let Some(next) = next else {
            return result;
        };

        if let Some(entry) = guard.get_mut(key) {
            entry.value = next;
            entry.inserted_at = now;
            entry.last_accessed_at = now;
        } else if self.max_capacity != Some(0) {
            guard.insert_new(
                key.to_owned(),
                next,
                now,
                self.max_capacity,
                self.evict_guard,
            );
        }

        result
    }

    /// Insert and return a clone of the value in one write lock.
    async fn insert_and_return(&self, key: K, value: V) -> V {
        let now = self.entry_time();
        let mut guard = self.inner.write().await;

        if let Some(entry) = guard.get_mut(&key) {
            let ret = value.clone();
            entry.value = value;
            entry.inserted_at = now;
            entry.last_accessed_at = now;
            return ret;
        }

        if self.max_capacity == Some(0) {
            return value;
        }

        let ret = value.clone();
        guard.insert_new(key, value, now, self.max_capacity, self.evict_guard);
        ret
    }

    pub async fn remove<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut guard = self.inner.write().await;
        let entry = guard.remove_key(key)?;
        // Nothing to date until an entry is actually in hand.
        let now = self.entry_time();
        if self.is_expired(&entry, now) {
            None
        } else {
            Some(entry.value)
        }
    }

    pub async fn invalidate<Q>(&self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mut guard = self.inner.write().await;
        guard.remove_key(key);
    }

    /// Reliably remove all entries, awaiting the write lock. Prefer this in
    /// async contexts over [`invalidate_all`](Self::invalidate_all), whose
    /// best-effort sync spin can skip the clear under sustained write
    /// contention.
    pub async fn clear(&self) {
        let mut guard = self.inner.write().await;
        guard.clear();
    }

    /// Sync invalidate. Spins briefly if the lock is held; kept for moka API
    /// parity. In async contexts prefer [`clear`](Self::clear), which can't
    /// silently skip the clear.
    pub fn invalidate_all(&self) {
        for _ in 0..64 {
            if let Some(mut guard) = self.inner.try_write() {
                guard.clear();
                return;
            }
            std::hint::spin_loop();
        }
        log::warn!("PortableCache::invalidate_all: could not acquire write lock after retries");
    }

    /// The configured capacity, or `None` when unbounded. `Some(0)` is the
    /// off switch: `insert` short-circuits, so the cache never holds anything
    /// and a caller can skip work that only makes sense when it does.
    pub fn configured_capacity(&self) -> Option<u64> {
        self.max_capacity
    }

    pub fn entry_count(&self) -> u64 {
        self.inner.try_read().map(|g| g.len() as u64).unwrap_or(0)
    }

    pub(crate) async fn capacity_stats(&self) -> CapacityStats {
        let guard = self.inner.read().await;
        CapacityStats {
            entries: guard.len() as u64,
            evictions: guard.capacity_evictions,
            eviction_blocks: guard.capacity_eviction_blocks,
        }
    }

    /// Reliable awaited snapshot of `(Arc<K>, V)` pairs. Prefer this over
    /// [`iter`](Self::iter) in async contexts: `iter` is best-effort (a
    /// `try_read` spin that yields an empty snapshot under write contention),
    /// which would silently skip entries an invalidation pass must see.
    pub async fn snapshot_entries(&self) -> Vec<(Arc<K>, V)> {
        let guard = self.inner.read().await;
        Self::snapshot(&guard)
    }

    /// Reliable awaited fold over `(&K, &V)`. Unlike the snapshot walks this
    /// clones nothing — memory reports must not themselves allocate in
    /// proportion to the cache — and unlike [`iter`](Self::iter) it cannot
    /// degrade to an empty walk under write contention.
    pub async fn fold_entries<A>(&self, init: A, mut f: impl FnMut(A, &K, &V) -> A) -> A {
        let guard = self.inner.read().await;
        guard.iter().fold(init, |acc, (k, e)| f(acc, k, &e.value))
    }

    /// Entry count plus estimated retained bytes: the table and eviction
    /// order the cache itself holds, plus `per_entry` summed over the entries,
    /// under a single awaited read guard so the pair is mutually consistent
    /// (and never the empty best-effort snapshot [`iter`](Self::iter) can
    /// degrade to). Callers charge only what their entries point at; the
    /// slots and order nodes are charged here, once, for every cache.
    pub async fn memory_stats(
        &self,
        mut per_entry: impl FnMut(&K, &V) -> usize,
    ) -> wacore::stats::CollectionStats {
        let guard = self.inner.read().await;
        let bytes: usize = guard.iter().map(|(k, e)| per_entry(k, &e.value)).sum();
        wacore::stats::CollectionStats::new(
            guard.len() as u64,
            (bytes + guard.structural_bytes()) as u64,
        )
    }

    /// Eager snapshot iterator over `(Arc<K>, V)`: snapshot, not lazy. Includes
    /// expired-but-not-yet-evicted entries (consistent with `entry_count`).
    /// Best-effort (`try_read` spin); use [`snapshot_entries`](Self::snapshot_entries)
    /// when missing an entry would be a correctness bug. Caller must not `.await`
    /// with the writer guard held from the same task — would deadlock on
    /// single-threaded runtimes.
    pub fn iter(&self) -> std::vec::IntoIter<(Arc<K>, V)> {
        for _ in 0..1024 {
            if let Some(guard) = self.inner.try_read() {
                return Self::snapshot(&guard).into_iter();
            }
            std::hint::spin_loop();
        }
        log::warn!(
            "PortableCache::iter: could not acquire read lock after retries; \
             returning empty snapshot"
        );
        Vec::new().into_iter()
    }

    fn snapshot(guard: &CacheInner<K, V>) -> Vec<(Arc<K>, V)> {
        guard
            .iter()
            .map(|(k, e)| (Arc::new(k.clone()), e.value.clone()))
            .collect()
    }

    /// Get or insert (single-flight). Takes key by value.
    ///
    /// The initializer is boxed only on cache miss — a hit returns without
    /// allocating. The boxing keeps the slow path monomorphic per `<K, V>`
    /// instead of per call-site future type. A racer that loses the
    /// double-check inside `get_with_slow` pays one
    /// spare box; deferring the box past the double-check would drag the
    /// future type parameter back into the slow path, re-stamping it per
    /// call site.
    #[inline]
    pub async fn get_with<F>(&self, key: K, init: F) -> V
    where
        F: Future<Output = V> + MaybeSend,
    {
        if let Some(v) = self.get(&key).await {
            return v;
        }
        self.get_with_slow(key, Box::pin(init)).await
    }

    /// Get or insert (single-flight). Takes key by reference — only allocates
    /// the owned key (and the boxed initializer) on cache miss.
    #[inline]
    pub async fn get_with_by_ref<Q, F>(&self, key: &Q, init: F) -> V
    where
        K: Borrow<Q>,
        Q: ToOwned<Owned = K> + Hash + Eq + ?Sized,
        F: Future<Output = V> + MaybeSend,
    {
        if let Some(v) = self.get(key).await {
            return v;
        }
        self.get_with_slow(key.to_owned(), Box::pin(init)).await
    }

    /// Miss path shared by [`get_with`](Self::get_with) and
    /// [`get_with_by_ref`](Self::get_with_by_ref): single-flight init under the
    /// per-key lock, with a double-checked `get` so a collided or racing key
    /// still resolves to the first inserted value.
    async fn get_with_slow(&self, key: K, init: BoxFuture<'_, V>) -> V {
        let hash = self.init_locks.hash_of(&key);
        // The cleanup guard holds the sole long-lived Arc so its Drop sees an
        // exact strong count if this future is cancelled at any await below.
        let mut cleanup = InitLockCleanup {
            registry: &self.init_locks,
            hash,
            lock: Some(self.init_locks.acquire(hash).await),
        };

        let value = {
            let _init_guard = cleanup
                .lock
                .as_ref()
                .expect("init-lock cleanup still armed")
                .lock()
                .await;
            // Double-check after acquiring the per-key lock.
            if let Some(v) = self.get(&key).await {
                v
            } else {
                let value = init.await;
                self.insert_and_return(key, value).await
            }
        };

        let init_mutex = cleanup.disarm();
        drop(cleanup);
        self.init_locks.reclaim(hash, &init_mutex).await;
        value
    }

    /// Stores made by the TTI renewal path since construction.
    #[cfg(test)]
    fn tti_renewals(&self) -> u64 {
        self.tti_renewals.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Evict expired entries and clean up unused init locks.
    /// Test-only read that leaves the second-chance bit alone, so a test can
    /// observe eviction order without feeding it.
    #[cfg(test)]
    async fn get_no_touch<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.inner.read().await.get(key).map(|e| e.value.clone())
    }

    pub async fn run_pending_tasks(&self) {
        let now = self.entry_time();
        let mut guard = self.inner.write().await;

        guard.retain_unexpired(|entry| self.is_expired(entry, now));

        drop(guard);

        // Clean up init locks not actively held.
        self.init_locks.retain_active().await;
    }
}

impl<K, V> Clone for PortableCache<K, V> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            init_locks: Arc::clone(&self.init_locks),
            max_capacity: self.max_capacity,
            ttl: self.ttl,
            tti: self.tti,
            evict_guard: self.evict_guard,
            #[cfg(test)]
            tti_renewals: Arc::clone(&self.tti_renewals),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    fn build_cache<K, V>() -> PortableCache<K, V>
    where
        K: Hash + Eq + Clone + Send + Sync + 'static,
        V: Clone + Send + Sync + 'static,
    {
        PortableCache::builder().max_capacity(100).build()
    }

    #[tokio::test]
    async fn test_basic_insert_and_get() {
        let cache = build_cache::<String, String>();

        assert!(cache.get("key1").await.is_none());

        cache.insert("key1".to_string(), "value1".to_string()).await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));
    }

    #[tokio::test]
    async fn capacity_only_cache_uses_clock_free_timestamps() {
        let cache = build_cache::<String, String>();
        assert_eq!(cache.entry_time(), Instant::ZERO);

        cache.insert("key".into(), "value".into()).await;
        assert_eq!(cache.get("key").await.as_deref(), Some("value"));

        let guard = cache.inner.read().await;
        let entry = guard.get("key").expect("inserted cache entry");
        assert_eq!(entry.inserted_at, Instant::ZERO);
        assert_eq!(entry.last_accessed_at, Instant::ZERO);
    }

    #[tokio::test]
    async fn test_update_existing_key() {
        let cache = build_cache::<String, String>();

        cache.insert("key1".to_string(), "v1".to_string()).await;
        cache.insert("key1".to_string(), "v2".to_string()).await;
        assert_eq!(cache.get("key1").await, Some("v2".to_string()));
        assert_eq!(cache.entry_count(), 1);
    }

    #[tokio::test]
    async fn upsert_with_by_ref_serializes_read_modify_write() {
        let cache = Arc::new(build_cache::<String, u32>());
        let mut tasks = Vec::new();
        for _ in 0..32 {
            let cache = Arc::clone(&cache);
            tasks.push(tokio::spawn(async move {
                cache
                    .upsert_with_by_ref("counter", |current| {
                        let next = current.copied().unwrap_or_default() + 1;
                        (Some(next), next)
                    })
                    .await
            }));
        }

        let mut results = Vec::with_capacity(tasks.len());
        for task in tasks {
            results.push(task.await.unwrap());
        }
        results.sort_unstable();

        assert_eq!(results, (1..=32).collect::<Vec<_>>());
        assert_eq!(cache.get("counter").await, Some(32));

        let unchanged = cache
            .upsert_with_by_ref("counter", |current| (None, current.copied()))
            .await;
        assert_eq!(unchanged, Some(32));
        assert_eq!(cache.get("counter").await, Some(32));
    }

    #[tokio::test]
    async fn test_capacity_eviction() {
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(3).build();

        cache.insert("a".into(), 1).await;
        cache.insert("b".into(), 2).await;
        cache.insert("c".into(), 3).await;
        assert_eq!(cache.entry_count(), 3);

        cache.insert("d".into(), 4).await;
        assert_eq!(cache.entry_count(), 3);
        assert!(cache.get("a").await.is_none());
        assert_eq!(cache.get("b").await, Some(2));
        assert_eq!(cache.get("d").await, Some(4));
        assert_eq!(
            cache.capacity_stats().await,
            CapacityStats {
                entries: 3,
                evictions: 1,
                eviction_blocks: 0,
            }
        );
    }

    #[tokio::test]
    async fn evict_guard_protects_held_entries() {
        type Lock = Arc<AsyncMutex<()>>;
        let guarded: PortableCache<String, Lock> = PortableCache::builder()
            .max_capacity(2)
            .evict_guard(|m| Arc::strong_count(m) <= 1)
            .build();

        // Insert an entry and keep an external clone — a live task holding the lock.
        let held: Lock = Arc::new(AsyncMutex::new(()));
        guarded.insert("held".into(), held.clone()).await;

        // Churn far past capacity with fresh, unheld entries.
        for i in 0..5 {
            guarded
                .insert(format!("k{i}"), Arc::new(AsyncMutex::new(())))
                .await;
        }

        // The held entry survives (protected) and is the SAME mutex instance, so a
        // later lookup can't mint a duplicate that two writers would race.
        let again = guarded
            .get("held")
            .await
            .expect("held entry must not be FIFO-evicted");
        assert!(Arc::ptr_eq(&held, &again), "same mutex instance preserved");

        // Contrast: with no guard the identical churn FIFO-evicts the held entry.
        let unguarded: PortableCache<String, Lock> =
            PortableCache::builder().max_capacity(2).build();
        unguarded.insert("held".into(), held.clone()).await;
        for i in 0..5 {
            unguarded
                .insert(format!("k{i}"), Arc::new(AsyncMutex::new(())))
                .await;
        }
        assert!(
            unguarded.get("held").await.is_none(),
            "an unguarded cache FIFO-evicts the held entry (the bug this guards)"
        );
    }

    #[tokio::test]
    async fn evict_guard_allows_temporary_over_capacity_when_all_held() {
        type Lock = Arc<AsyncMutex<()>>;
        let cache: PortableCache<String, Lock> = PortableCache::builder()
            .max_capacity(2)
            .evict_guard(|m| Arc::strong_count(m) <= 1)
            .build();

        // Hold every entry, then insert one more: with nothing evictable the cache
        // grows past capacity rather than dropping a live lock.
        let mut held = Vec::new();
        for i in 0..3 {
            let lock: Lock = Arc::new(AsyncMutex::new(()));
            held.push(lock.clone());
            cache.insert(format!("k{i}"), lock).await;
        }
        assert_eq!(
            cache.entry_count(),
            3,
            "all entries held -> cache exceeds capacity instead of evicting a live lock"
        );
        assert_eq!(
            cache.capacity_stats().await,
            CapacityStats {
                entries: 3,
                evictions: 0,
                eviction_blocks: 1,
            }
        );

        // Drop the external refs; the next insert now evicts back down to capacity.
        drop(held);
        cache
            .insert("fresh".into(), Arc::new(AsyncMutex::new(())))
            .await;
        assert_eq!(
            cache.entry_count(),
            2,
            "once entries are released, eviction resumes down to capacity"
        );
        assert_eq!(
            cache.capacity_stats().await,
            CapacityStats {
                entries: 2,
                evictions: 2,
                eviction_blocks: 1,
            }
        );
    }

    /// A bounded cache is least-recently-used, not first-in-first-out: an
    /// entry that keeps being read outlives everything inserted after it,
    /// while an entry never read again is evicted in insertion order.
    #[tokio::test]
    async fn a_read_entry_outlives_entries_inserted_after_it() {
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(8).build();
        cache.insert("hot".to_string(), 1).await;
        cache.insert("cold".to_string(), 2).await;
        for i in 0..64u32 {
            cache.insert(format!("filler-{i}"), i).await;
            assert_eq!(
                cache.get("hot").await,
                Some(1),
                "hot entry evicted after {i} inserts"
            );
        }
        assert_eq!(
            cache.get("cold").await,
            None,
            "an unread entry must age out"
        );
        assert_eq!(cache.entry_count(), 8);
    }

    /// A hit never takes the write lock or moves the entry: the second
    /// chance is spent by the eviction that reaches it, which re-queues it
    /// once and drops it the next time round if nothing read it again.
    #[tokio::test]
    async fn a_hit_is_spent_by_one_eviction_pass() {
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(4).build();
        cache.insert("a".to_string(), 1).await;
        let seq_before = cache.inner.read().await.get("a").map(|e| e.seq);
        cache.get("a").await;
        assert_eq!(
            cache.inner.read().await.get("a").map(|e| e.seq),
            seq_before,
            "a hit must not reorder under the read lock"
        );
        // Filling past capacity reaches `a` first; the hit re-queues it
        // behind the fillers instead of evicting it.
        for i in 0..4u32 {
            cache.insert(format!("b{i}"), i).await;
        }
        assert_eq!(cache.get_no_touch("a").await, Some(1));
        assert!(cache.inner.read().await.get("a").map(|e| e.seq) > seq_before);
        // Not read since: the next pass over it evicts.
        for i in 4..8u32 {
            cache.insert(format!("b{i}"), i).await;
        }
        assert_eq!(cache.get_no_touch("a").await, None);
    }

    /// When every entry has been hit since the last pass, the pass that spends
    /// their chances must not end the eviction: the cache stays at capacity.
    #[tokio::test]
    async fn an_all_referenced_pass_still_evicts() {
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(1).build();
        cache.insert("a".to_string(), 1).await;
        cache.get("a").await;
        cache.insert("b".to_string(), 2).await;
        let stats = cache.capacity_stats().await;
        assert_eq!(stats.entries, 1);
        assert_eq!(stats.eviction_blocks, 0);
        assert_eq!(cache.get_no_touch("a").await, None);
        assert_eq!(cache.get_no_touch("b").await, Some(2));
    }

    #[tokio::test]
    async fn test_remove_then_eviction_preserves_fifo_order() {
        // A removed key must leave the FIFO `order` consistent: eviction must skip
        // it (no stale order entry) and still evict the genuinely-oldest survivor.
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(3).build();
        cache.insert("a".into(), 1).await;
        cache.insert("b".into(), 2).await;
        cache.insert("c".into(), 3).await;

        // Remove the oldest, then fill back to capacity.
        assert_eq!(cache.remove("a").await, Some(1));
        cache.insert("d".into(), 4).await; // count = 3 (b, c, d), no eviction
        assert_eq!(cache.entry_count(), 3);

        // Next insert evicts the now-oldest survivor (b), not the removed "a".
        cache.insert("e".into(), 5).await;
        assert_eq!(cache.entry_count(), 3);
        assert!(cache.get("b").await.is_none(), "b was the oldest survivor");
        assert_eq!(cache.get("c").await, Some(3));
        assert_eq!(cache.get("d").await, Some(4));
        assert_eq!(cache.get("e").await, Some(5));
    }

    #[tokio::test]
    async fn test_zero_capacity_disables_caching() {
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(0).build();

        cache.insert("a".into(), 1).await;
        assert!(cache.get("a").await.is_none());
        assert_eq!(cache.entry_count(), 0);
    }

    /// Expiry decided against a supplied instant, so the boundary (exactly at
    /// the deadline, which counts as expired) is pinned without depending on
    /// wall-clock timing.
    #[test]
    fn expiry_boundary_is_exact_under_a_controlled_clock() {
        let ttl = Duration::from_secs(60);
        let tti = Duration::from_secs(10);
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(ttl)
            .time_to_idle(tti)
            .build();

        let inserted = Instant::ZERO + Duration::from_secs(1_000);
        let entry = CacheEntry {
            value: 1,
            inserted_at: inserted,
            last_accessed_at: inserted,
            seq: 0,
            referenced: portable_atomic::AtomicBool::new(false),
        };

        assert!(!cache.is_expired(&entry, inserted + tti - Duration::from_nanos(1)));
        assert!(cache.is_expired(&entry, inserted + tti), "TTI is inclusive");

        let idle_free: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(ttl)
            .build();
        assert!(!idle_free.is_expired(&entry, inserted + ttl - Duration::from_nanos(1)));
        assert!(
            idle_free.is_expired(&entry, inserted + ttl),
            "TTL is inclusive"
        );
    }

    /// The renewal tolerance is a hard boundary, pinned against a supplied
    /// instant so it does not depend on wall-clock timing.
    #[test]
    fn tti_renewal_tolerance_is_exact_under_a_controlled_clock() {
        let tti = Duration::from_secs(3600);
        let tolerance = tti / TTI_RENEWAL_DIVISOR;
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(tti)
            .build();

        let stamped = Instant::ZERO + Duration::from_secs(1_000);
        let entry = CacheEntry {
            value: 1,
            inserted_at: stamped,
            last_accessed_at: stamped,
            seq: 0,
            referenced: portable_atomic::AtomicBool::new(false),
        };

        assert!(!cache.needs_tti_renewal(&entry, stamped));
        assert!(!cache.needs_tti_renewal(&entry, stamped + tolerance - Duration::from_nanos(1)));
        assert!(
            cache.needs_tti_renewal(&entry, stamped + tolerance),
            "the renewal tolerance is inclusive, like the expiry boundary"
        );

        // Without TTI there is nothing to renew, so `get` never leaves the read
        // path however stale the stamp is.
        let ttl_only: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(tti)
            .build();
        assert!(!ttl_only.needs_tti_renewal(&entry, stamped + tti * 100));
    }

    /// The declared contract of the tolerance: a skipped renewal leaves the
    /// stamp behind the real access, so an entry can expire up to one tolerance
    /// window EARLY, and never a nanosecond late. Serving a long-expired entry
    /// is the error direction this design must not have.
    #[test]
    fn a_skipped_renewal_expires_early_never_late() {
        let tti = Duration::from_secs(3600);
        let tolerance = tti / TTI_RENEWAL_DIVISOR;
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(tti)
            .build();

        // Worst case: the real access landed a hair before the tolerance
        // elapsed, so the recorded stamp lags it by (almost) a full window.
        let stamped = Instant::ZERO + Duration::from_secs(1_000);
        let real_access = stamped + tolerance - Duration::from_nanos(1);
        let entry = CacheEntry {
            value: 1,
            inserted_at: stamped,
            last_accessed_at: stamped,
            seq: 0,
            referenced: portable_atomic::AtomicBool::new(false),
        };

        // Never late: gone by `real_access + tti` at the very latest.
        assert!(cache.is_expired(&entry, real_access + tti));
        // At most one tolerance window early, never more.
        assert!(cache.is_expired(&entry, stamped + tti));
        assert!(!cache.is_expired(&entry, real_access + tti - tolerance));

        // A key used at least once per window is restamped well inside the
        // TTI, so continuous use still keeps it alive indefinitely.
        assert!(tolerance < tti);
    }

    /// The point of the design: repeated lookups of a hot key stay on the read
    /// lock. Observed through the stamp, which only a write-lock renewal moves.
    #[tokio::test]
    async fn a_hot_key_is_not_restamped_on_every_get() {
        // TTI far longer than this test runs, so no lookup comes near the
        // renewal tolerance.
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(Duration::from_secs(3600))
            .build();
        cache.insert("k".into(), 1).await;

        let stamp = |cache: PortableCache<String, u32>| async move {
            cache.inner.read().await.get("k").unwrap().last_accessed_at
        };
        let before = stamp(cache.clone()).await;

        for _ in 0..1_000 {
            assert_eq!(cache.get("k").await, Some(1));
        }

        assert_eq!(
            stamp(cache.clone()).await,
            before,
            "a lookup inside the renewal tolerance must not take the write lock"
        );
        assert_eq!(cache.tti_renewals(), 0, "no lookup reached the write path");
    }

    /// The other half of the contract: once the stamp ages past the tolerance,
    /// a lookup does promote to the write lock and renew it, so a continuously
    /// used entry never idles out.
    #[tokio::test]
    async fn a_get_past_the_tolerance_renews_the_stamp() {
        // 4s TTI => 250ms tolerance. Polling every 25ms renews long before the
        // entry could idle out.
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(Duration::from_secs(4))
            .build();
        cache.insert("k".into(), 1).await;

        let stamp = |cache: PortableCache<String, u32>| async move {
            cache.inner.read().await.get("k").unwrap().last_accessed_at
        };
        let before = stamp(cache.clone()).await;

        let mut renewed = false;
        for _ in 0..400 {
            assert_eq!(cache.get("k").await, Some(1), "polling must keep it alive");
            if stamp(cache.clone()).await > before {
                renewed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        assert!(renewed, "a lookup past the tolerance must renew the stamp");
    }

    /// The expiry-removal path drops the guard before taking the write lock, so
    /// it removes only the entry it judged, identified by `(seq, inserted_at)`.
    /// That is sound only while every write path moves one of the two, so pin
    /// which one each moves.
    #[tokio::test]
    async fn a_rewritten_entry_gets_a_new_identity() {
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_secs(3600))
            .build();

        let identity = |cache: PortableCache<String, u32>| async move {
            let guard = cache.inner.read().await;
            let e = guard.get("k").unwrap();
            (e.seq, e.inserted_at)
        };

        cache.insert("k".into(), 1).await;
        let first = identity(cache.clone()).await;

        // Rewrite in place until the clock has visibly advanced, so this does
        // not depend on the monotonic provider's granularity.
        let mut rewritten = first;
        for i in 0..400 {
            cache.insert("k".into(), i).await;
            rewritten = identity(cache.clone()).await;
            if rewritten.1 != first.1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        assert_eq!(rewritten.0, first.0, "an in-place rewrite keeps the slot");
        assert_ne!(
            rewritten.1, first.1,
            "an in-place rewrite must move inserted_at, or an expiring lookup \
             could remove the value it wrote"
        );

        // A removal plus a fresh insert takes a new slot instead.
        cache.invalidate("k").await;
        cache.insert("k".into(), 3).await;
        assert_ne!(
            identity(cache.clone()).await.0,
            rewritten.0,
            "a reinsert must take a new seq"
        );
    }

    /// A burst of lookups crossing the tolerance together all decide to renew
    /// under their read guards. Each re-decides under the write lock, so the
    /// ones queued behind the first find the entry already fresh and leave its
    /// stamp alone instead of each pushing it further forward.
    ///
    /// The pileup itself cannot be forced: the lock is writer-preferring, so
    /// the window between the read-side decision and the first store closes as
    /// soon as one writer registers. The count assertion is exact for correct
    /// code and catches a regression only when the race does occur; what makes
    /// the property hold is the write-lock re-check plus the exact tolerance
    /// boundary pinned in `tti_renewal_tolerance_is_exact_under_a_controlled_clock`.
    #[tokio::test]
    async fn a_concurrent_burst_past_the_tolerance_renews_once() {
        // 4s TTI => 250ms tolerance.
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(Duration::from_secs(4))
            .build();
        cache.insert("k".into(), 1).await;

        let stamp = |cache: PortableCache<String, u32>| async move {
            cache.inner.read().await.get("k").unwrap().last_accessed_at
        };
        let before = stamp(cache.clone()).await;
        assert_eq!(cache.tti_renewals(), 0);

        // Idle past the tolerance without touching the cache, so the whole
        // burst observes the same stale stamp.
        let deadline = Instant::now() + Duration::from_millis(300);
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let barrier = Arc::new(tokio::sync::Barrier::new(32));
        let mut handles = Vec::new();
        for _ in 0..32 {
            let cache = cache.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                cache.get("k").await
            }));
        }
        for h in handles {
            assert_eq!(h.await.unwrap(), Some(1), "every lookup is still served");
        }

        assert!(stamp(cache.clone()).await > before, "the stamp was renewed");
        // The whole burst lands far inside the 250ms tolerance, so whichever
        // writer stores first leaves the rest nothing to do. Comparing raw
        // stamps instead would let every queued writer store in turn.
        assert_eq!(
            cache.tti_renewals(),
            1,
            "a burst must renew once, not once per queued writer"
        );

        // And the next lookup is back on the read path entirely.
        assert_eq!(cache.get("k").await, Some(1));
        assert_eq!(
            cache.tti_renewals(),
            1,
            "a lookup inside the tolerance must not renew again"
        );
    }

    /// Failure case: an entry that idled past its TTI must be removed and never
    /// served, including when the burst of lookups that would have renewed it
    /// arrives concurrently right at the boundary.
    #[tokio::test]
    async fn an_expired_entry_is_never_served_to_a_concurrent_burst() {
        let tti = Duration::from_millis(120);
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(tti)
            .build();
        cache.insert("k".into(), 1).await;

        // Idle it out without touching the cache, so nothing renews the stamp.
        let deadline = Instant::now() + tti + Duration::from_millis(20);
        while Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let barrier = Arc::new(tokio::sync::Barrier::new(32));
        let mut handles = Vec::new();
        for _ in 0..32 {
            let cache = cache.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                cache.get("k").await
            }));
        }
        for h in handles {
            assert_eq!(
                h.await.unwrap(),
                None,
                "an entry past its TTI must never be served"
            );
        }
        assert!(
            cache.get("k").await.is_none(),
            "the expired entry must stay removed, not be resurrected by a renewal"
        );
    }

    /// Failure case: `get` racing `insert` and `invalidate` on one key must
    /// never observe a torn value, and its renewal write must never undo an
    /// invalidation.
    #[tokio::test]
    async fn concurrent_get_insert_and_invalidate_never_tear_or_resurrect() {
        // A short TTI makes the tolerance ~3ms, so nearly every lookup takes
        // the renewal write path: maximum pressure on the race.
        let cache: PortableCache<String, (u64, u64)> = PortableCache::builder()
            .max_capacity(100)
            .time_to_idle(Duration::from_millis(50))
            .build();
        cache.insert("k".into(), (0, 0)).await;

        // Both halves of the value carry the same generation, so a torn read
        // would show a mismatched pair.
        let stop = Arc::new(AtomicBool::new(false));
        let mut readers = Vec::new();
        for _ in 0..8 {
            let cache = cache.clone();
            let stop = stop.clone();
            readers.push(tokio::spawn(async move {
                while !stop.load(Ordering::Relaxed) {
                    if let Some((a, b)) = cache.get("k").await {
                        assert_eq!(a, b, "get returned a torn value");
                    }
                    tokio::task::yield_now().await;
                }
            }));
        }

        let writer = tokio::spawn({
            let cache = cache.clone();
            async move {
                for i in 1..=2_000u64 {
                    cache.insert("k".into(), (i, i)).await;
                    tokio::task::yield_now().await;
                }
            }
        });
        let invalidator = tokio::spawn({
            let cache = cache.clone();
            async move {
                for _ in 0..2_000 {
                    cache.invalidate("k").await;
                    tokio::task::yield_now().await;
                }
            }
        });
        writer.await.unwrap();
        invalidator.await.unwrap();

        // Nothing inserts any more, so the key must stay gone while the readers
        // keep hammering it; a renewal that re-inserted would bring it back.
        cache.invalidate("k").await;
        for _ in 0..200 {
            assert!(
                cache.get("k").await.is_none(),
                "a get's renewal resurrected an invalidated entry"
            );
            tokio::task::yield_now().await;
        }

        stop.store(true, Ordering::Relaxed);
        for r in readers {
            r.await.unwrap();
        }
    }

    /// Named regression for the coordination caches (`session_locks`,
    /// `chat_lanes`, `group_distribution_locks`): they are marked by their
    /// `evict_guard`, and expiry does not consult it, so a timeout would drop a
    /// lock a task still holds and let the next lookup mint a second one.
    #[test]
    #[should_panic(expected = "must not expire by time")]
    fn a_coordination_cache_cannot_be_given_a_tti() {
        let _: PortableCache<String, Arc<AsyncMutex<()>>> = PortableCache::builder()
            .max_capacity(16)
            .evict_guard(|m| Arc::strong_count(m) <= 1)
            .time_to_idle(Duration::from_secs(60))
            .build();
    }

    #[test]
    #[should_panic(expected = "must not expire by time")]
    fn a_coordination_cache_cannot_be_given_a_ttl() {
        let _: PortableCache<String, Arc<AsyncMutex<()>>> = PortableCache::builder()
            .max_capacity(16)
            .evict_guard(|m| Arc::strong_count(m) <= 1)
            .time_to_live(Duration::from_secs(60))
            .build();
    }

    /// A lookup that finds nothing has no timestamp to compare, so it must not
    /// pay for one. Every negative registry probe goes through here.
    #[tokio::test]
    async fn a_miss_does_not_read_the_clock() {
        use wacore::time::clock_reads;

        for cache in [
            PortableCache::<String, u32>::builder()
                .max_capacity(100)
                .time_to_live(Duration::from_secs(60))
                .build(),
            PortableCache::<String, u32>::builder()
                .max_capacity(100)
                .time_to_idle(Duration::from_secs(60))
                .build(),
        ] {
            cache.insert("present".into(), 1).await;

            let base = clock_reads::snapshot();
            assert!(cache.get("absent").await.is_none());
            assert!(cache.remove("absent").await.is_none());
            assert_eq!(
                clock_reads::since(base).monotonic,
                0,
                "a miss must not read the monotonic clock"
            );

            let hit = clock_reads::snapshot();
            assert_eq!(cache.get("present").await, Some(1));
            assert_eq!(
                clock_reads::since(hit).monotonic,
                1,
                "a hit reads once, to decide expiry"
            );
        }
    }

    /// A key wrapper that counts its clones. The cache used to clone every
    /// key into its FIFO index, a second heap allocation per entry for the
    /// life of the entry; the FIFO side now addresses slots by `(hash, seq)`.
    #[derive(Debug)]
    struct CountingKey(String, Arc<AtomicUsize>);

    impl PartialEq for CountingKey {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Eq for CountingKey {}
    impl Hash for CountingKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            self.0.hash(state);
        }
    }

    impl Clone for CountingKey {
        fn clone(&self) -> Self {
            self.1.fetch_add(1, Ordering::Relaxed);
            Self(self.0.clone(), Arc::clone(&self.1))
        }
    }

    #[tokio::test]
    async fn an_insert_stores_the_key_once_and_never_clones_it() {
        let clones = Arc::new(AtomicUsize::new(0));
        let cache: PortableCache<CountingKey, u32> =
            PortableCache::builder().max_capacity(8).build();
        for i in 0..16u32 {
            cache
                .insert(CountingKey(format!("k{i}"), Arc::clone(&clones)), i)
                .await;
        }
        assert_eq!(
            cache.entry_count(),
            8,
            "FIFO eviction still bounds the cache"
        );
        assert_eq!(
            clones.load(Ordering::Relaxed),
            0,
            "insert, eviction and the FIFO index must not clone the key"
        );
        cache
            .remove(&CountingKey("k12".into(), Arc::clone(&clones)))
            .await;
        cache
            .invalidate(&CountingKey("k13".into(), Arc::clone(&clones)))
            .await;
        assert_eq!(clones.load(Ordering::Relaxed), 0);
        assert_eq!(cache.entry_count(), 6);
    }

    #[tokio::test]
    async fn an_unbounded_cache_keeps_no_fifo_index() {
        let unbounded: PortableCache<String, u32> = PortableCache::builder().build();
        let effectively_unbounded: PortableCache<String, u32> =
            PortableCache::builder().max_capacity(u64::MAX).build();
        let bounded: PortableCache<String, u32> = PortableCache::builder().max_capacity(8).build();
        for cache in [&unbounded, &effectively_unbounded, &bounded] {
            for i in 0..4u32 {
                cache.insert(format!("k{i}"), i).await;
            }
            assert_eq!(cache.get("k2").await, Some(2));
        }
        assert!(unbounded.inner.read().await.order.is_empty());
        assert!(effectively_unbounded.inner.read().await.order.is_empty());
        assert_eq!(bounded.inner.read().await.order.len(), 4);
        // Removal on the unbounded caches still works without the index.
        assert_eq!(unbounded.remove("k1").await, Some(1));
        assert_eq!(unbounded.entry_count(), 3);
    }

    #[tokio::test]
    async fn test_ttl_expiry() {
        let cache: PortableCache<String, String> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_millis(50))
            .build();

        cache.insert("key1".to_string(), "value1".to_string()).await;
        assert_eq!(cache.get("key1").await, Some("value1".to_string()));

        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate() {
        let cache = build_cache::<String, String>();

        cache.insert("key1".to_string(), "value1".to_string()).await;
        cache.invalidate("key1").await;
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_invalidate_all() {
        let cache = build_cache::<String, u32>();

        cache.insert("a".into(), 1).await;
        cache.insert("b".into(), 2).await;
        cache.invalidate_all();
        assert_eq!(cache.entry_count(), 0);
        assert!(cache.get("a").await.is_none());
    }

    #[tokio::test]
    async fn test_remove() {
        let cache = build_cache::<String, String>();

        cache.insert("key1".to_string(), "v1".to_string()).await;
        let removed = cache.remove("key1").await;
        assert_eq!(removed, Some("v1".to_string()));
        assert!(cache.get("key1").await.is_none());
    }

    #[tokio::test]
    async fn test_iter_snapshot_includes_expired() {
        // Snapshot semantics: iter returns all map entries, including ones
        // past TTL that haven't been evicted yet. Pin this so the call site
        // (invalidate_entries_for_device) keeps idempotent invalidation.
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_millis(10))
            .build();
        cache.insert("a".to_string(), 1).await;
        cache.insert("b".to_string(), 2).await;
        tokio::time::sleep(Duration::from_millis(20)).await;

        let mut keys: Vec<String> = cache.iter().map(|(k, _)| k.as_ref().clone()).collect();
        keys.sort();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn test_get_with_basic() {
        let cache = build_cache::<String, u32>();

        let v = cache.get_with("key1".to_string(), async { 42 }).await;
        assert_eq!(v, 42);

        let v = cache.get_with("key1".to_string(), async { 99 }).await;
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn test_get_with_by_ref_basic() {
        let cache = build_cache::<String, u32>();
        let key = "key1".to_string();

        let v = cache.get_with_by_ref(&key, async { 42 }).await;
        assert_eq!(v, 42);

        let v = cache.get_with_by_ref(&key, async { 99 }).await;
        assert_eq!(v, 42);
    }

    #[tokio::test]
    async fn test_get_with_single_flight() {
        let cache: PortableCache<String, Arc<AtomicUsize>> =
            PortableCache::builder().max_capacity(100).build();

        let init_count = Arc::new(AtomicUsize::new(0));
        let num_tasks = 20;
        let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));

        let mut handles = Vec::new();
        for _ in 0..num_tasks {
            let cache = cache.clone();
            let init_count = init_count.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                cache
                    .get_with("shared_key".to_string(), async {
                        init_count.fetch_add(1, Ordering::SeqCst);
                        tokio::task::yield_now().await;
                        Arc::new(AtomicUsize::new(0))
                    })
                    .await
            }));
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }

        assert_eq!(init_count.load(Ordering::SeqCst), 1);
        let first = &results[0];
        for r in &results[1..] {
            assert!(Arc::ptr_eq(first, r));
        }
    }

    #[tokio::test]
    async fn test_get_with_by_ref_single_flight() {
        let cache: PortableCache<String, Arc<AtomicUsize>> =
            PortableCache::builder().max_capacity(100).build();

        let init_count = Arc::new(AtomicUsize::new(0));
        let num_tasks = 20;
        let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));

        let mut handles = Vec::new();
        for _ in 0..num_tasks {
            let cache = cache.clone();
            let init_count = init_count.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                let key = "shared_key".to_string();
                cache
                    .get_with_by_ref(&key, async {
                        init_count.fetch_add(1, Ordering::SeqCst);
                        tokio::task::yield_now().await;
                        Arc::new(AtomicUsize::new(0))
                    })
                    .await
            }));
        }

        let mut results = Vec::new();
        for h in handles {
            results.push(h.await.unwrap());
        }

        assert_eq!(init_count.load(Ordering::SeqCst), 1);
        let first = &results[0];
        for r in &results[1..] {
            assert!(Arc::ptr_eq(first, r));
        }
    }

    #[tokio::test]
    async fn test_get_with_different_keys_parallel() {
        let cache = build_cache::<String, u32>();

        let init_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for i in 0..10 {
            let cache = cache.clone();
            let init_count = init_count.clone();
            handles.push(tokio::spawn(async move {
                cache
                    .get_with(format!("key_{i}"), async {
                        init_count.fetch_add(1, Ordering::SeqCst);
                        i as u32
                    })
                    .await
            }));
        }

        for (i, h) in handles.into_iter().enumerate() {
            assert_eq!(h.await.unwrap(), i as u32);
        }
        assert_eq!(init_count.load(Ordering::SeqCst), 10);
    }

    #[tokio::test]
    async fn test_session_lock_pattern() {
        let cache: PortableCache<String, Arc<async_lock::Mutex<()>>> =
            PortableCache::builder().max_capacity(100).build();

        let counter = Arc::new(AtomicUsize::new(0));
        let num_tasks = 50;
        let barrier = Arc::new(tokio::sync::Barrier::new(num_tasks));

        let mut handles = Vec::new();
        for _ in 0..num_tasks {
            let cache = cache.clone();
            let counter = counter.clone();
            let barrier = barrier.clone();
            handles.push(tokio::spawn(async move {
                barrier.wait().await;
                let mutex = cache
                    .get_with("sender_123".to_string(), async {
                        Arc::new(async_lock::Mutex::new(()))
                    })
                    .await;
                let _guard = mutex.lock().await;
                let val = counter.load(Ordering::SeqCst);
                tokio::task::yield_now().await;
                counter.store(val + 1, Ordering::SeqCst);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), num_tasks);
    }

    #[tokio::test]
    async fn test_run_pending_tasks_cleans_expired() {
        let cache: PortableCache<String, u32> = PortableCache::builder()
            .max_capacity(100)
            .time_to_live(Duration::from_millis(50))
            .build();

        cache.insert("a".into(), 1).await;
        cache.insert("b".into(), 2).await;
        assert_eq!(cache.entry_count(), 2);

        tokio::time::sleep(Duration::from_millis(60)).await;
        cache.run_pending_tasks().await;
        assert_eq!(cache.entry_count(), 0);
    }

    #[tokio::test]
    async fn test_get_with_reclaims_init_lock_eagerly() {
        // A completed single-flight `get_with` must not leave its per-key init
        // lock behind — otherwise high-cardinality caches (session locks, chat
        // lanes, dedup) that never call run_pending_tasks leak one lock per key.
        let cache: PortableCache<String, u32> = PortableCache::builder().max_capacity(100).build();

        let _ = cache.get_with("key1".to_string(), async { 1 }).await;
        let _ = cache.get_with_by_ref("key2", async { 2 }).await;

        let locks = cache.init_locks.map.lock().await;
        assert!(
            locks.is_empty(),
            "init locks must be reclaimed after get_with"
        );
    }

    #[tokio::test]
    async fn cancelled_get_with_reclaims_init_lock() {
        // A get_with whose caller is aborted mid-init must not leave its
        // per-key init lock behind: hot caches never call run_pending_tasks.
        let cache = build_cache::<String, u32>();
        let task = tokio::spawn({
            let cache = cache.clone();
            async move {
                cache
                    .get_with("stuck".to_string(), std::future::pending::<u32>())
                    .await
            }
        });

        // Poll (bounded) until the in-flight init registers its lock.
        let mut registered = false;
        for _ in 0..400 {
            if !cache.init_locks.map.lock().await.is_empty() {
                registered = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(registered, "in-flight get_with never registered its lock");

        task.abort();
        let _ = task.await;

        // Poll (bounded): the cleanup guard reclaims on cancellation, without
        // any run_pending_tasks call.
        let mut reclaimed = false;
        for _ in 0..400 {
            if cache.init_locks.map.lock().await.is_empty() {
                reclaimed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(reclaimed, "cancelled get_with leaked its init lock");
    }

    #[tokio::test]
    async fn init_locks_collision_shares_one_lock() {
        // Two keys that hash to the same slot must share the lock (they
        // serialize) and both resolve correctly through the double-checked get.
        let registry = InitLocks::new();
        let first = registry.acquire(42).await;
        let second = registry.acquire(42).await;
        assert!(
            Arc::ptr_eq(&first, &second),
            "same hash must yield the same init lock"
        );

        // While another caller still holds a clone, reclaim must keep the entry.
        registry.reclaim(42, &first).await;
        assert!(
            registry.map.lock().await.contains_key(&42),
            "reclaim must not drop a lock another caller still holds"
        );

        // Once the other caller is done, the entry is removed.
        drop(second);
        registry.reclaim(42, &first).await;
        assert!(
            registry.map.lock().await.is_empty(),
            "last reclaim must drop the registry entry"
        );
    }

    /// Key whose hash is a constant, so any two instances collide in the
    /// hash-keyed init-lock registry while remaining distinct map keys.
    #[derive(Clone, PartialEq, Eq, Debug)]
    struct CollidingKey(&'static str);

    impl Hash for CollidingKey {
        fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
            state.write_u64(0);
        }
    }

    #[tokio::test]
    async fn colliding_keys_keep_distinct_values() {
        let cache: PortableCache<CollidingKey, u32> =
            PortableCache::builder().max_capacity(16).build();
        let (a, b) = (CollidingKey("a"), CollidingKey("b"));
        assert_eq!(
            cache.init_locks.hash_of(&a),
            cache.init_locks.hash_of(&b),
            "test premise: both keys must share one init-lock slot"
        );

        // Rendezvous BEFORE get_with: colliding keys share one init lock, so
        // their initializers serialize and must never wait on each other.
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let mut tasks = Vec::new();
        for (key, value) in [(a.clone(), 1u32), (b.clone(), 2u32)] {
            let cache = cache.clone();
            let barrier = barrier.clone();
            tasks.push(tokio::spawn(async move {
                barrier.wait().await;
                cache
                    .get_with(key, async {
                        tokio::task::yield_now().await;
                        value
                    })
                    .await
            }));
        }
        let mut results = Vec::new();
        for task in tasks {
            results.push(task.await.unwrap());
        }
        assert_eq!(results, vec![1, 2], "each key must get its own init value");
        assert_eq!(cache.get(&a).await, Some(1));
        assert_eq!(cache.get(&b).await, Some(2));
    }

    #[tokio::test]
    async fn get_with_distinct_keys_share_registry_correctly() {
        // Same value type, distinct keys: each key keeps its own value even
        // though the init-lock registry is keyed by hash rather than by key.
        let cache = build_cache::<String, u32>();
        let a = cache.get_with("a".to_string(), async { 1 }).await;
        let b = cache.get_with("b".to_string(), async { 2 }).await;
        assert_eq!((a, b), (1, 2));
        assert_eq!(cache.get("a").await, Some(1));
        assert_eq!(cache.get("b").await, Some(2));
    }
}
