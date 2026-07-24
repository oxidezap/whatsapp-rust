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
use std::borrow::Borrow;
use std::collections::{BTreeMap, HashMap};
use std::hash::{BuildHasher, Hash, RandomState};
use std::sync::Arc;
use std::time::Duration;
use wacore::runtime::BoxFuture;
use wacore::sync_marker::MaybeSend;
use wacore::time::Instant;

struct CacheEntry<V> {
    value: V,
    // Monotonic instants (not wall-clock) so TTL/TTI are immune to clock jumps,
    // matching moka's timer semantics.
    inserted_at: Instant,
    last_accessed_at: Instant,
    /// FIFO sequence number; the key for this entry in `CacheInner::order`.
    seq: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CapacityStats {
    pub entries: u64,
    pub evictions: u64,
    pub eviction_blocks: u64,
}

/// Portable, runtime-agnostic in-process cache.
///
/// - Max capacity with FIFO eviction
/// - TTL (time-to-live) and TTI (time-to-idle)
/// - Single-flight `get_with` / `get_with_by_ref`
pub struct PortableCache<K, V> {
    inner: Arc<RwLock<CacheInner<K, V>>>,
    /// Per-key init locks for single-flight `get_with`.
    init_locks: Arc<InitLocks>,
    max_capacity: Option<u64>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
    /// Optional predicate gating capacity eviction: returns `true` if a value may
    /// be evicted. Used by coordination-lock caches to protect an entry a live task
    /// still holds (e.g. an `Arc<Mutex>` mid-lock), which would otherwise be
    /// FIFO-evicted and re-minted, letting two writers race the guarded resource.
    evict_guard: Option<fn(&V) -> bool>,
}

struct CacheInner<K, V> {
    map: HashMap<K, CacheEntry<V>>,
    /// FIFO eviction order keyed by monotonic sequence. `seq -> key`, so eviction
    /// is `pop_first()` (O(log n)) and a targeted `remove_key` is O(log n) via the
    /// entry's stored `seq` — instead of an O(n) scan over an insertion list.
    order: BTreeMap<u64, K>,
    /// Next FIFO sequence to assign.
    next_seq: u64,
    capacity_evictions: u64,
    capacity_eviction_blocks: u64,
}

impl<K, V> CacheInner<K, V>
where
    K: Hash + Eq + Clone,
{
    fn new() -> Self {
        Self {
            map: HashMap::new(),
            order: BTreeMap::new(),
            next_seq: 0,
            capacity_evictions: 0,
            capacity_eviction_blocks: 0,
        }
    }

    fn remove_key(&mut self, key: &K) -> Option<CacheEntry<V>> {
        let entry = self.map.remove(key)?;
        self.order.remove(&entry.seq);
        Some(entry)
    }

    /// Evict entries until under `cap`. Outlined and never-inlined so the guarded
    /// scan is compiled once per `<K, V>` (not duplicated into every `insert_new`
    /// inline site), keeping the binary-size cost of the guard path small.
    #[inline(never)]
    fn evict_to_capacity(&mut self, cap: u64, evict_guard: Option<fn(&V) -> bool>) {
        while self.map.len() as u64 >= cap {
            match evict_guard {
                // Unguarded caches keep the single-pass pop_first() fast path.
                None => match self.order.pop_first() {
                    Some((_, oldest_key)) => {
                        if self.map.remove(&oldest_key).is_some() {
                            self.capacity_evictions = self.capacity_evictions.saturating_add(1);
                        }
                    }
                    None => break,
                },
                // Guarded: skip entries a live task still holds so a later lookup
                // can't mint a duplicate; if every entry is held, allow temporary
                // over-capacity rather than dropping a live entry. Remove by seq so
                // the key isn't cloned.
                Some(is_evictable) => {
                    let mut victim_seq = None;
                    for (seq, k) in self.order.iter() {
                        if self.map.get(k).is_some_and(|e| is_evictable(&e.value)) {
                            victim_seq = Some(*seq);
                            break;
                        }
                    }
                    match victim_seq {
                        Some(seq) => {
                            if let Some(oldest_key) = self.order.remove(&seq)
                                && self.map.remove(&oldest_key).is_some()
                            {
                                self.capacity_evictions = self.capacity_evictions.saturating_add(1);
                            }
                        }
                        None => {
                            self.capacity_eviction_blocks =
                                self.capacity_eviction_blocks.saturating_add(1);
                            break;
                        }
                    }
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
        self.order.insert(seq, key.clone());
        self.map.insert(
            key,
            CacheEntry {
                value,
                inserted_at: now,
                last_accessed_at: now,
                seq,
            },
        );
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

    pub fn time_to_idle(mut self, tti: Duration) -> Self {
        self.tti = Some(tti);
        self
    }

    pub fn build(self) -> PortableCache<K, V> {
        PortableCache {
            inner: Arc::new(RwLock::new(CacheInner::new())),
            init_locks: Arc::new(InitLocks::new()),
            max_capacity: self.max_capacity,
            ttl: self.ttl,
            tti: self.tti,
            evict_guard: self.evict_guard,
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

    fn find_key<Q>(inner: &CacheInner<K, V>, key: &Q) -> Option<K>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        inner.map.get_key_value(key).map(|(k, _)| k.clone())
    }

    pub async fn get<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let now = self.entry_time();

        // Fast path (no TTI): read lock only, no write needed.
        if self.tti.is_none() {
            let guard = self.inner.read().await;
            let entry = guard.map.get(key)?;
            if self.is_expired(entry, now) {
                let owned_key = Self::find_key(&guard, key)?;
                drop(guard);
                let mut wguard = self.inner.write().await;
                if let Some(e) = wguard.map.get(key)
                    && self.is_expired(e, now)
                {
                    wguard.remove_key(&owned_key);
                }
                return None;
            }
            return Some(entry.value.clone());
        }

        // TTI path: write lock to update last_accessed_at.
        let mut guard = self.inner.write().await;
        let entry = guard.map.get_mut(key)?;
        if self.is_expired(entry, now) {
            let owned_key = Self::find_key(&guard, key)?;
            guard.remove_key(&owned_key);
            return None;
        }
        entry.last_accessed_at = now;
        Some(entry.value.clone())
    }

    pub async fn insert(&self, key: K, value: V) {
        let now = self.entry_time();
        let mut guard = self.inner.write().await;

        if let Some(entry) = guard.map.get_mut(&key) {
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
            .map
            .get(key)
            .is_some_and(|entry| self.is_expired(entry, now))
            && let Some(owned_key) = Self::find_key(&guard, key)
        {
            guard.remove_key(&owned_key);
        }

        let (next, result) = update(guard.map.get(key).map(|entry| &entry.value));
        let Some(next) = next else {
            return result;
        };

        if let Some(entry) = guard.map.get_mut(key) {
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

        if let Some(entry) = guard.map.get_mut(&key) {
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
        let now = self.entry_time();
        let mut guard = self.inner.write().await;
        let owned_key = Self::find_key(&guard, key)?;
        let entry = guard.remove_key(&owned_key)?;
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
        if let Some(owned_key) = Self::find_key(&guard, key) {
            guard.remove_key(&owned_key);
        }
    }

    /// Reliably remove all entries, awaiting the write lock. Prefer this in
    /// async contexts over [`invalidate_all`](Self::invalidate_all), whose
    /// best-effort sync spin can skip the clear under sustained write
    /// contention.
    pub async fn clear(&self) {
        let mut guard = self.inner.write().await;
        guard.map.clear();
        guard.order.clear();
    }

    /// Sync invalidate. Spins briefly if the lock is held; kept for moka API
    /// parity. In async contexts prefer [`clear`](Self::clear), which can't
    /// silently skip the clear.
    pub fn invalidate_all(&self) {
        for _ in 0..64 {
            if let Some(mut guard) = self.inner.try_write() {
                guard.map.clear();
                guard.order.clear();
                return;
            }
            std::hint::spin_loop();
        }
        log::warn!("PortableCache::invalidate_all: could not acquire write lock after retries");
    }

    pub fn entry_count(&self) -> u64 {
        self.inner
            .try_read()
            .map(|g| g.map.len() as u64)
            .unwrap_or(0)
    }

    pub(crate) async fn capacity_stats(&self) -> CapacityStats {
        let guard = self.inner.read().await;
        CapacityStats {
            entries: guard.map.len() as u64,
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
        guard
            .map
            .iter()
            .fold(init, |acc, (k, e)| f(acc, k, &e.value))
    }

    /// Entry count plus estimated retained bytes, summing `per_entry` under a
    /// single awaited read guard so the pair is mutually consistent (and never
    /// the empty best-effort snapshot [`iter`](Self::iter) can degrade to).
    pub async fn memory_stats(
        &self,
        mut per_entry: impl FnMut(&K, &V) -> usize,
    ) -> wacore::stats::CollectionStats {
        let guard = self.inner.read().await;
        let bytes: usize = guard.map.iter().map(|(k, e)| per_entry(k, &e.value)).sum();
        wacore::stats::CollectionStats::new(guard.map.len() as u64, bytes as u64)
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
            .map
            .iter()
            .map(|(k, e)| (Arc::new(k.clone()), e.value.clone()))
            .collect()
    }

    /// Get or insert (single-flight). Takes key by value.
    ///
    /// The initializer is boxed only on cache miss — a hit returns without
    /// allocating. The boxing keeps the slow path monomorphic per `<K, V>`
    /// instead of per call-site future type. A racer that loses the
    /// double-check inside [`get_with_slow`](Self::get_with_slow) pays one
    /// spare box; deferring the box past the double-check would drag the
    /// future type parameter back into the slow path, re-stamping it per
    /// call site.
    #[inline]
    pub async fn get_with<F>(&self, key: K, init: F) -> V
    where
        F: std::future::Future<Output = V> + MaybeSend,
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
        F: std::future::Future<Output = V> + MaybeSend,
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

    /// Evict expired entries and clean up unused init locks.
    pub async fn run_pending_tasks(&self) {
        let now = self.entry_time();
        let mut guard = self.inner.write().await;

        guard.map.retain(|_, entry| !self.is_expired(entry, now));

        // Drop order entries whose keys were just expired out of the map.
        // Borrow fields separately to satisfy the borrow checker.
        let CacheInner { map, order, .. } = &mut *guard;
        order.retain(|_, k| map.contains_key(k));

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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        let entry = guard.map.get("key").expect("inserted cache entry");
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
