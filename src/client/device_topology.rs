//! Device-topology change tracking for the per-group device-list memo.
//!
//! "Topology" here means anything that can change a device-list answer:
//! registry record writes/invalidations and LID-PN mapping changes. Instead of
//! trusting every write path to remember a manual generation bump, the bump
//! lives INSIDE the write chokepoints ([`DeviceRegistryCache`] and
//! `LidPnCache::add`), so a writer cannot forget it by construction.
//!
//! Each change also logs WHICH canonical users it touched (both namespaces),
//! so a memo whose generation went stale can prove "none of the changed users
//! are in my group" and re-stamp itself instead of recomputing. Every doubtful
//! case (log overflow, global events) degrades to a recompute, never to
//! serving stale data.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use portable_atomic::AtomicU64;
use wacore_binary::CompactString;

/// Bounded log length. Sized so a burst (e.g. a usync response for a large
/// group) still fits; overflow just disables the scoped-revalidation fast
/// path until affected memos recompute once.
///
/// A length bound, not a preallocation: the deque grows into it, because a
/// session that never touches a group never writes an entry and would
/// otherwise carry the full 8 KiB for nothing.
const TOPOLOGY_LOG_CAPACITY: usize = 256;

struct TopologyLog {
    /// (generation, canonical user touched).
    entries: VecDeque<(u64, CompactString)>,
    /// Highest generation evicted from `entries` (0 = nothing evicted).
    /// A memo older than this cannot be proven clean and must recompute.
    floor: u64,
}

/// Shared tracker: a monotonic generation plus the bounded changed-users log.
pub(crate) struct DeviceTopology {
    generation: AtomicU64,
    log: std::sync::Mutex<TopologyLog>,
    registry_mutation: async_lock::Mutex<()>,
}

/// Proof that a device-registry mutation is serialized with authoritative
/// refresh publication. The cache write API requires this token so new write
/// paths cannot accidentally bypass the ordering invariant.
pub(crate) struct DeviceRegistryMutationGuard<'a> {
    _guard: async_lock::MutexGuard<'a, ()>,
}

impl DeviceTopology {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            generation: AtomicU64::new(0),
            log: std::sync::Mutex::new(TopologyLog {
                entries: VecDeque::new(),
                floor: 0,
            }),
            registry_mutation: async_lock::Mutex::new(()),
        })
    }

    pub(crate) fn current(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    pub(crate) async fn lock_registry(&self) -> DeviceRegistryMutationGuard<'_> {
        DeviceRegistryMutationGuard {
            _guard: self.registry_mutation.lock().await,
        }
    }

    /// Record one topology change touching the given users (pass BOTH
    /// namespaces of an identity when known: a mapping change alters which
    /// canonical record either key resolves to).
    pub(crate) fn record<'a>(&self, users: impl IntoIterator<Item = &'a str>) {
        self.record_change(users);
    }

    fn record_change<'a>(&self, users: impl IntoIterator<Item = &'a str>) {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        let generation = self.generation.load(Ordering::Acquire) + 1;
        for user in users {
            if log.entries.len() == TOPOLOGY_LOG_CAPACITY
                && let Some((evicted_gen, _)) = log.entries.pop_front()
            {
                log.floor = evicted_gen;
            }
            log.entries
                .push_back((generation, CompactString::from(user)));
        }
        // Publish the generation only after the log holds the users, so a
        // reader that observes the new generation can always find (or rule
        // out) the corresponding entries.
        self.generation.store(generation, Ordering::Release);
    }

    /// Record a registry mutation while holding its serialization guard, so a
    /// refresh can compare-and-publish without a check/write race.
    pub(crate) fn record_registry<'a>(
        &self,
        _guard: &DeviceRegistryMutationGuard<'_>,
        users: impl IntoIterator<Item = &'a str>,
    ) {
        self.record_change(users);
    }

    /// Record a change whose blast radius is unknown (bulk warm-up, cache
    /// clear): bumps and poisons the scoped fast path so every memo
    /// recomputes once.
    pub(crate) fn record_global(&self) {
        let mut log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        let generation = self.generation.load(Ordering::Acquire) + 1;
        log.entries.clear();
        log.floor = generation;
        self.generation.store(generation, Ordering::Release);
    }

    /// Whether every change after `since` only touched users for which
    /// `is_member` returns false. `false` on any doubt (log overflow past
    /// `since`), so callers recompute.
    pub(crate) fn unchanged_for(&self, since: u64, is_member: impl Fn(&str) -> bool) -> bool {
        let log = self.log.lock().unwrap_or_else(|p| p.into_inner());
        if log.floor > since {
            return false;
        }
        log.entries
            .iter()
            .filter(|(generation, _)| *generation > since)
            .all(|(_, user)| !is_member(user))
    }
}

/// The device registry cache plus its topology tracker, fused so every write
/// records the change. Reads are pass-through; the only write entry points
/// are [`insert`](Self::insert), [`invalidate`](Self::invalidate) and the
/// non-recording [`promote`](Self::promote) (whose data is by definition what
/// the DB fallback already answered).
pub(crate) struct DeviceRegistryCache {
    cache: crate::cache_store::TypedCache<String, Arc<wacore::store::traits::DeviceListRecord>>,
    topology: Arc<DeviceTopology>,
}

impl DeviceRegistryCache {
    pub(crate) fn new(
        cache: crate::cache_store::TypedCache<String, Arc<wacore::store::traits::DeviceListRecord>>,
        topology: Arc<DeviceTopology>,
    ) -> Self {
        Self { cache, topology }
    }

    pub(crate) async fn get(
        &self,
        key: &str,
    ) -> Option<Arc<wacore::store::traits::DeviceListRecord>> {
        self.cache.get(key).await
    }

    /// Write a record and log the touched users. `touched` carries the keys
    /// whose answers change (canonical key, plus the original alias when the
    /// canonical flipped).
    pub(crate) async fn insert<'a>(
        &self,
        guard: &DeviceRegistryMutationGuard<'_>,
        key: String,
        record: Arc<wacore::store::traits::DeviceListRecord>,
        touched: impl IntoIterator<Item = &'a str>,
    ) {
        self.cache.insert(key, record).await;
        self.topology.record_registry(guard, touched);
    }

    pub(crate) async fn invalidate(&self, guard: &DeviceRegistryMutationGuard<'_>, key: &str) {
        self.cache.invalidate(key).await;
        self.topology.record_registry(guard, [key]);
    }

    /// Cache-fill from the DB row the fallback path would have returned: the
    /// answer is unchanged, so no topology change is recorded.
    pub(crate) async fn promote(
        &self,
        key: String,
        record: Arc<wacore::store::traits::DeviceListRecord>,
    ) {
        self.cache.insert(key, record).await;
    }

    /// Approximate entry count plus estimated retained bytes. Bytes are `0`
    /// when backed by a custom store (entries live outside this process).
    pub(crate) async fn memory_stats(&self) -> wacore::stats::CollectionStats {
        use wacore::stats::HeapSize;
        self.cache
            .memory_stats(|k, v| k.capacity() + v.heap_bytes())
            .await
    }

    /// Test-only passthrough for cache maintenance flushes.
    #[cfg(test)]
    pub(crate) async fn run_pending_tasks(&self) {
        self.cache.run_pending_tasks().await;
    }

    /// Test-only raw write that bypasses topology recording, for fixture
    /// seeding and for proving that memo hits really are hits (a raw change
    /// must be served stale).
    #[cfg(test)]
    pub(crate) async fn raw_insert_for_tests(
        &self,
        key: String,
        record: Arc<wacore::store::traits::DeviceListRecord>,
    ) {
        self.cache.insert(key, record).await;
    }

    #[cfg(test)]
    pub(crate) async fn raw_invalidate_for_tests(&self, key: &str) {
        self.cache.invalidate(key).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{DeviceTopology, TOPOLOGY_LOG_CAPACITY};

    /// The whole point of the log: a memo whose generation went stale proves
    /// none of the changed users are in its group and re-stamps itself.
    #[test]
    fn an_unrelated_change_leaves_a_memo_provably_clean() {
        let topology = DeviceTopology::new();
        let stamped = topology.current();
        topology.record(["someone-else"]);

        assert!(topology.current() > stamped, "the generation must advance");
        assert!(topology.unchanged_for(stamped, |user| user == "member"));
        assert!(
            !topology.unchanged_for(stamped, |user| user == "someone-else"),
            "a change touching a member cannot be proven clean"
        );
    }

    /// Growing into the bound must not raise it: past the cap the log evicts,
    /// and an evicted generation lifts the floor so anything older recomputes.
    #[test]
    fn overflowing_the_bound_evicts_and_poisons_older_generations() {
        let topology = DeviceTopology::new();
        let stamped = topology.current();
        for i in 0..TOPOLOGY_LOG_CAPACITY + 10 {
            topology.record([format!("user-{i}").as_str()]);
        }

        assert_eq!(
            topology.log.lock().unwrap().entries.len(),
            TOPOLOGY_LOG_CAPACITY,
            "the log grew past its bound"
        );
        assert!(
            !topology.unchanged_for(stamped, |_| false),
            "a generation behind the floor cannot be proven clean"
        );
        // Everything still in the log stays answerable, which is what the
        // eviction is trading the old entries for.
        let floor = topology.log.lock().unwrap().floor;
        assert!(topology.unchanged_for(floor, |user| user == "absent"));
    }

    /// A global change clears the log, so the generations it published must
    /// still be readable as "recompute" rather than as an empty (clean) log.
    #[test]
    fn a_global_change_poisons_every_earlier_generation() {
        let topology = DeviceTopology::new();
        topology.record(["a"]);
        let stamped = topology.current();
        topology.record_global();

        assert!(topology.log.lock().unwrap().entries.is_empty());
        assert!(
            !topology.unchanged_for(stamped, |_| false),
            "an emptied log must not read as nothing having changed"
        );
        assert!(topology.unchanged_for(topology.current(), |_| true));
    }
}
