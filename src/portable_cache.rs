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
use std::hash::Hash;
use std::sync::Arc;
use std::time::Duration;
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

/// Portable, runtime-agnostic in-process cache.
///
/// - Max capacity with FIFO eviction
/// - TTL (time-to-live) and TTI (time-to-idle)
/// - Single-flight `get_with` / `get_with_by_ref`
pub struct PortableCache<K, V> {
    inner: Arc<RwLock<CacheInner<K, V>>>,
    /// Per-key init locks for single-flight `get_with`.
    init_locks: Arc<AsyncMutex<HashMap<K, Arc<AsyncMutex<()>>>>>,
    max_capacity: Option<u64>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
}

struct CacheInner<K, V> {
    map: HashMap<K, CacheEntry<V>>,
    /// FIFO eviction order keyed by monotonic sequence. `seq -> key`, so eviction
    /// is `pop_first()` (O(log n)) and a targeted `remove_key` is O(log n) via the
    /// entry's stored `seq` — instead of an O(n) scan over an insertion list.
    order: BTreeMap<u64, K>,
    /// Next FIFO sequence to assign.
    next_seq: u64,
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
        }
    }

    fn remove_key(&mut self, key: &K) -> Option<CacheEntry<V>> {
        let entry = self.map.remove(key)?;
        self.order.remove(&entry.seq);
        Some(entry)
    }

    /// Insert a brand-new entry (the caller has already confirmed the key is
    /// absent), evicting the oldest entries first if at capacity. Assigns and
    /// records the FIFO sequence.
    fn insert_new(&mut self, key: K, value: V, now: Instant, max_capacity: Option<u64>) {
        if let Some(cap) = max_capacity {
            while self.map.len() as u64 >= cap {
                match self.order.pop_first() {
                    Some((_, oldest_key)) => {
                        self.map.remove(&oldest_key);
                    }
                    None => break,
                }
            }
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

// -- Builder --

pub struct PortableCacheBuilder<K, V> {
    max_capacity: Option<u64>,
    ttl: Option<Duration>,
    tti: Option<Duration>,
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
            _marker: std::marker::PhantomData,
        }
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
            init_locks: Arc::new(AsyncMutex::new(HashMap::new())),
            max_capacity: self.max_capacity,
            ttl: self.ttl,
            tti: self.tti,
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
        let now = Instant::now();

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
        let now = Instant::now();
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

        guard.insert_new(key, value, now, self.max_capacity);
    }

    /// Insert and return a clone of the value in one write lock.
    async fn insert_and_return(&self, key: K, value: V) -> V {
        let now = Instant::now();
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
        guard.insert_new(key, value, now, self.max_capacity);
        ret
    }

    pub async fn remove<Q>(&self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let now = Instant::now();
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
    pub async fn get_with<F>(&self, key: K, init: F) -> V
    where
        F: std::future::Future<Output = V>,
    {
        if let Some(v) = self.get(&key).await {
            return v;
        }

        let init_mutex = {
            let mut locks = self.init_locks.lock().await;
            locks
                .entry(key.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };

        let value = {
            let _init_guard = init_mutex.lock().await;
            // Double-check after acquiring per-key lock.
            if let Some(v) = self.get(&key).await {
                v
            } else {
                self.insert_and_return(key.clone(), init.await).await
            }
        };

        self.reclaim_init_lock(&key, &init_mutex).await;
        value
    }

    /// Get or insert (single-flight). Takes key by reference — only allocates
    /// the owned key on cache miss.
    pub async fn get_with_by_ref<Q, F>(&self, key: &Q, init: F) -> V
    where
        K: Borrow<Q>,
        Q: ToOwned<Owned = K> + Hash + Eq + ?Sized,
        F: std::future::Future<Output = V>,
    {
        if let Some(v) = self.get(key).await {
            return v;
        }

        let owned_key = key.to_owned();
        let init_mutex = {
            let mut locks = self.init_locks.lock().await;
            locks
                .entry(owned_key.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(())))
                .clone()
        };

        let value = {
            let _init_guard = init_mutex.lock().await;
            if let Some(v) = self.get(key).await {
                v
            } else {
                self.insert_and_return(owned_key.clone(), init.await).await
            }
        };

        self.reclaim_init_lock(&owned_key, &init_mutex).await;
        value
    }

    /// Drop a single-flight init lock once no other caller is using it, so
    /// `init_locks` can't grow without bound across distinct keys (it is
    /// otherwise only reclaimed by [`run_pending_tasks`], which several hot
    /// `get_with` caches never call). `strong_count <= 2` means only this
    /// caller's clone and the map entry remain; the `ptr_eq` guard avoids
    /// dropping a newer lock a racing caller may have inserted.
    async fn reclaim_init_lock(&self, key: &K, init_mutex: &Arc<AsyncMutex<()>>) {
        let mut locks = self.init_locks.lock().await;
        if Arc::strong_count(init_mutex) <= 2
            && let Some(existing) = locks.get(key)
            && Arc::ptr_eq(existing, init_mutex)
        {
            locks.remove(key);
        }
    }

    /// Evict expired entries and clean up unused init locks.
    pub async fn run_pending_tasks(&self) {
        let now = Instant::now();
        let mut guard = self.inner.write().await;

        guard.map.retain(|_, entry| !self.is_expired(entry, now));

        // Drop order entries whose keys were just expired out of the map.
        // Borrow fields separately to satisfy the borrow checker.
        let CacheInner { map, order, .. } = &mut *guard;
        order.retain(|_, k| map.contains_key(k));

        drop(guard);

        // Clean up init locks not actively held.
        let mut locks = self.init_locks.lock().await;
        locks.retain(|_, v| Arc::strong_count(v) > 1);
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
    async fn test_update_existing_key() {
        let cache = build_cache::<String, String>();

        cache.insert("key1".to_string(), "v1".to_string()).await;
        cache.insert("key1".to_string(), "v2".to_string()).await;
        assert_eq!(cache.get("key1").await, Some("v2".to_string()));
        assert_eq!(cache.entry_count(), 1);
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

        let locks = cache.init_locks.lock().await;
        assert!(
            locks.is_empty(),
            "init locks must be reclaimed after get_with"
        );
    }
}
