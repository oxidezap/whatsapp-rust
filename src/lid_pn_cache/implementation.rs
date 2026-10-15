//! LID-PN (Linked ID to Phone Number) Cache
//!
//! This module implements a cache for mapping between WhatsApp's Linked IDs (LIDs)
//! and phone numbers. The cache is used for Signal address resolution - WhatsApp Web
//! uses LID-based addresses for Signal sessions when available.
//!
//! The cache maintains bidirectional mappings:
//! - LID -> Entry (for getting phone number from LID)
//! - Phone Number -> Entry (for getting LID from phone number)
//!
//! When multiple LIDs exist for the same phone number (rare), the most recent one
//! (by `created_at` timestamp) is considered "current".
//!
//! Both maps are unbounded by default and never expire. WA Web
//! (`WAWebLidPnCache`) uses plain `Map`s, so a mapping learned at startup or
//! via usync stays available for every subsequent Signal-address resolution.
//! A custom `CacheEntryConfig` can impose a capacity bound if memory pressure
//! requires it, accepting the trade-off that capacity-LRU eviction silently
//! downgrades Signal addresses to `@c.us`.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use wacore_binary::CompactString;

use crate::cache_config::{CacheConfig, CacheEntryConfig};
use crate::cache_store::TypedCache;
#[cfg(test)]
use wacore::types::LearningSource;
use wacore::types::LidPnEntry;

/// Namespaces used in the custom store.
const NS_LID: &str = "lid_pn_by_lid";
const NS_PN: &str = "lid_pn_by_pn";

/// An integer key allocates nothing and stays `Display` for the store path.
#[inline]
fn pack_contact_hash(hash: [u8; 3]) -> u32 {
    u32::from_be_bytes([0, hash[0], hash[1], hash[2]])
}

#[inline]
fn contact_hash_key(user: &str) -> u32 {
    pack_contact_hash(wacore::crypto::contact_notification_hash(user))
}

/// Cache for LID to Phone Number mappings.
///
/// This cache maintains bidirectional mappings between LIDs and phone numbers,
/// similar to WhatsApp Web's LidPnCache class. It provides fast lookups for
/// Signal address resolution.
///
/// The cache is thread-safe and can be shared across async tasks.
pub struct LidPnCache {
    /// LID -> Entry mapping. `Arc` so the hot `get_*` lookups clone a refcount,
    /// not the entry's strings; only the owned-`LidPnEntry` accessors deep-clone.
    /// Keys are `Arc<str>` sharing the entry's own allocations — each identifier
    /// is stored once per mapping, not once as key and again inside the entry
    /// (this cache is unbounded by design, so per-entry bytes compound).
    lid_to_entry: TypedCache<Arc<str>, Arc<LidPnEntry>>,
    /// Phone number -> Entry mapping (stores the most recent LID for that PN)
    pn_to_entry: TypedCache<Arc<str>, Arc<LidPnEntry>>,
    /// Serializes mapping writers. Reads remain lock-free; the guard exists so
    /// an authoritative device refresh can compare topology and publish its
    /// mappings plus registry snapshot without a concurrent writer slipping
    /// between those steps.
    mutation: async_lock::Mutex<()>,
    // This instance's startup writes break bootstrap ties. Retained external
    // cache rows carry no such provenance. Drop this state
    // after bootstrap; ordinary learning never grows a permanent shadow index.
    // Allocate the two map headers only on the first live startup write, so
    // clients without one do not retain their fixed cost for their lifetime.
    startup_writes: std::sync::Mutex<Option<Box<StartupWrites>>>,
    startup_finished: AtomicBool,
    pub(crate) identity_continuity: wacore::store::signal_cache::IdentityContinuity,
    /// Device-topology tracker (attached by Client construction): a mapping
    /// change alters which canonical record either key resolves to, so adds
    /// record both identifiers. Recording lives here, at the write
    /// chokepoint, so callers cannot forget it.
    topology: std::sync::OnceLock<Arc<crate::client::device_topology::DeviceTopology>>,
    /// Contact hash -> LID, for the `<devices>` `<update hash>` notification
    /// whose 3-byte hash is the only identifier it carries. Both sides of a pair
    /// are indexed because the hashed id is LID-namespaced only for migrated
    /// contacts, and both resolve to the LID. Always in-process: derived state
    /// each process rebuilds from its own warm-up. A 3-byte hash collides for
    /// ~1 pair in a few thousand contacts, where the newest write wins.
    contact_hash_to_lid: TypedCache<u32, Arc<str>>,
    /// PN -> the LID this process durably persisted for it. Lets the learn hot
    /// path skip a re-persist without swallowing the first live persist of a
    /// mapping an offline replay only warmed in memory. Keyed by the pair so a
    /// remap (or a stale detached write that marks late) only matches its own
    /// LID, never a newer un-persisted one. `Arc<str>` value: the hot-path check
    /// clones a refcount and compares in place, no payload copy. In-memory only;
    /// mappings loaded from persistent storage are marked during warm-up.
    persisted: TypedCache<Arc<str>, Arc<str>>,
}

/// What [`LidPnCache::memory_stats`] reports: one figure per internal map.
#[derive(Debug, Clone, Copy, Default)]
pub struct LidPnMemory {
    /// LID → entry, charged with the entry payloads.
    pub lid: wacore::stats::CollectionStats,
    /// PN → entry; bytes cover only entries the LID map no longer shares.
    pub pn: wacore::stats::CollectionStats,
    /// Contact hash → LID, structural bytes only (the LID is the entry's).
    pub contact_hash: wacore::stats::CollectionStats,
    /// PN → persisted LID, structural bytes only (both strings are the entry's).
    pub persisted: wacore::stats::CollectionStats,
}

/// Proof that LID/PN mapping mutations are serialized.
pub(crate) struct LidPnMutationGuard<'a> {
    _guard: async_lock::MutexGuard<'a, ()>,
}

#[derive(Default)]
struct StartupWrites {
    lids: HashMap<Arc<str>, Arc<LidPnEntry>>,
    phones: HashMap<Arc<str>, Arc<LidPnEntry>>,
}

pub(crate) struct LidPnWarmUpGuard<'a>(&'a LidPnCache);

impl Drop for LidPnWarmUpGuard<'_> {
    fn drop(&mut self) {
        drop(self.0.take_startup_writes());
    }
}

fn same_entry(left: &LidPnEntry, right: &LidPnEntry) -> bool {
    left.lid == right.lid
        && left.phone_number == right.phone_number
        && left.created_at == right.created_at
        && left.learning_source == right.learning_source
}

enum MappingWrite {
    Live,
    WarmUp {
        preserve_lid: bool,
        live_pn_created_at: Option<i64>,
    },
}

impl Default for LidPnCache {
    fn default() -> Self {
        Self::new()
    }
}

impl LidPnCache {
    /// Create a new empty cache with default settings (no time-based expiry,
    /// effectively unbounded — matches `WAWebLidPnCache`).
    pub fn new() -> Self {
        Self::with_config(&CacheConfig::default().lid_pn_cache, None)
    }

    /// Create a new cache with custom configuration (uses time_to_idle semantics
    /// when a timeout is set; default config has none).
    ///
    /// When `store` is `Some`, both internal maps use the custom backend.
    /// When `store` is `None`, both maps use in-process caches.
    pub fn with_config(
        config: &CacheEntryConfig,
        store: Option<Arc<dyn wacore::store::CacheStore>>,
    ) -> Self {
        // Eviction and external writers can change resolution without add().
        // Historical sends cannot prove alias continuity in those configurations.
        let identity_continuity =
            if store.is_none() && config.timeout.is_none() && config.capacity == u64::MAX {
                wacore::store::signal_cache::IdentityContinuity::default()
            } else {
                wacore::store::signal_cache::IdentityContinuity::unavailable()
            };
        match store {
            Some(s) => Self {
                lid_to_entry: TypedCache::from_store(s.clone(), NS_LID, config.timeout),
                pn_to_entry: TypedCache::from_store(s, NS_PN, config.timeout),
                mutation: async_lock::Mutex::new(()),
                startup_writes: std::sync::Mutex::new(None),
                startup_finished: AtomicBool::new(false),
                identity_continuity,
                // Always in-memory: tracks per-process persist state, never the
                // mapping itself, so it must not go through the shared store.
                persisted: TypedCache::from_local(config.build_with_tti()),
                contact_hash_to_lid: TypedCache::from_local(config.build_with_tti()),
                topology: std::sync::OnceLock::new(),
            },
            None => Self {
                lid_to_entry: TypedCache::from_local(config.build_with_tti()),
                pn_to_entry: TypedCache::from_local(config.build_with_tti()),
                mutation: async_lock::Mutex::new(()),
                startup_writes: std::sync::Mutex::new(None),
                startup_finished: AtomicBool::new(false),
                identity_continuity,
                persisted: TypedCache::from_local(config.build_with_tti()),
                contact_hash_to_lid: TypedCache::from_local(config.build_with_tti()),
                topology: std::sync::OnceLock::new(),
            },
        }
    }

    /// Releases transient startup provenance on success, read error, or cancellation.
    pub(crate) fn warm_up_guard(&self) -> LidPnWarmUpGuard<'_> {
        LidPnWarmUpGuard(self)
    }

    fn take_startup_writes(&self) -> Option<StartupWrites> {
        self.startup_finished.store(true, Ordering::Release);
        self.startup_writes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .take()
            .map(|writes| *writes)
    }

    fn track_startup_write(&self, record: impl FnOnce(&mut StartupWrites)) {
        let mut writes = self
            .startup_writes
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        // A caller can pass the fast-path check before warm-up takes the maps.
        // Check under the lock before lazily creating them, or that late writer
        // would recreate provenance after bootstrap has already released it.
        if !self.startup_finished.load(Ordering::Acquire) {
            record(writes.get_or_insert_default());
        }
    }

    /// Attach the device-topology tracker. Mapping writes before the attach
    /// (none in practice: Client construction attaches before warm-up) are
    /// simply not scoped.
    pub(crate) fn attach_topology(
        &self,
        topology: Arc<crate::client::device_topology::DeviceTopology>,
    ) {
        let _ = self.topology.set(topology);
    }

    pub(crate) async fn lock_mutation(&self) -> LidPnMutationGuard<'_> {
        LidPnMutationGuard {
            _guard: self.mutation.lock().await,
        }
    }

    /// Test-only clones of the backing custom stores, one per direction map.
    #[cfg(test)]
    pub(crate) fn custom_stores_for_tests(&self) -> Vec<Arc<dyn wacore::store::CacheStore>> {
        [
            self.lid_to_entry.custom_store_for_tests(),
            self.pn_to_entry.custom_store_for_tests(),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Approximate entry counts plus estimated retained bytes for the LID and
    /// PN maps. Bytes are `0` when backed by a custom store (entries live
    /// outside this process).
    ///
    /// `add()` stores the same `Arc<LidPnEntry>` under both directions, so the
    /// payload is attributed to the LID map; the PN side counts only entries
    /// the LID map no longer holds (transient eviction asymmetry), keeping
    /// every entry counted exactly once. `Arc<T>`'s `HeapSize` already
    /// includes `size_of::<LidPnEntry>()`.
    pub async fn memory_stats(&self) -> LidPnMemory {
        use wacore::stats::HeapSize;
        // Dedup by heap address as `usize`, not `*const LidPnEntry`: a raw-pointer
        // set is `!Send`, and held across the two `.await`s below it would make
        // `Client::memory_report()` `!Send` (unusable off a single thread). The cast
        // is a plain address on every target (`LidPnEntry: Sized` → thin pointer).
        let mut lid_ptrs: HashSet<usize> = HashSet::new();
        let mut lid = self
            .lid_to_entry
            .memory_stats(|_, v| {
                lid_ptrs.insert(Arc::as_ptr(v) as usize);
                v.heap_bytes()
            })
            .await;
        lid.bytes += self.identity_continuity.estimated_heap_bytes() as u64;
        let pn = self
            .pn_to_entry
            .memory_stats(|_, v| {
                if lid_ptrs.contains(&(Arc::as_ptr(v) as usize)) {
                    0
                } else {
                    v.heap_bytes()
                }
            })
            .await;
        // Both side maps hold only `Arc<str>`s the entries above already own
        // (`contact_hash_to_lid` clones the entry's LID; `persisted` takes the
        // entry's own pair), so their payload is charged once, at the entry.
        // What they add is table structure, which `memory_stats` charges
        // itself — and that is the part that grows with the contact count for
        // the whole process lifetime, which is why they are reported at all.
        let contact_hash = self.contact_hash_to_lid.structural_stats().await;
        let persisted = self.persisted.structural_stats().await;
        LidPnMemory {
            lid,
            pn,
            contact_hash,
            persisted,
        }
    }

    /// Sweep entries past the configured idle timeout (a no-op for the
    /// default, unbounded configuration, and for store-backed maps). Driven by
    /// [`Client::run_cache_maintenance`].
    ///
    /// [`Client::run_cache_maintenance`]: crate::client::Client::run_cache_maintenance
    pub async fn run_pending_tasks(&self) {
        self.lid_to_entry.run_pending_tasks().await;
        self.pn_to_entry.run_pending_tasks().await;
        self.contact_hash_to_lid.run_pending_tasks().await;
        self.persisted.run_pending_tasks().await;
    }

    /// Get the current LID for a phone number.
    ///
    /// Returns the LID user part if a mapping exists, None otherwise.
    /// The cache holds `Arc<LidPnEntry>`; return the LID user as an (inline for
    /// typical ~15-digit LIDs) `CompactString` instead of deep-cloning a `String`.
    pub async fn get_current_lid(&self, phone: &str) -> Option<CompactString> {
        self.pn_to_entry
            .get(phone)
            .await
            .map(|e| CompactString::from(&*e.lid))
    }

    /// Whether the learn fast path can skip re-recording `phone <-> lid`: the
    /// pair is durably persisted AND resolvable in BOTH cache directions.
    /// Requiring both directions means skipping never leaves the reverse
    /// (LID -> PN) lookup cold under a configured eviction; that lookup
    /// (`get_phone_number`, e.g. in PN->LID session migration) has no backend
    /// fallback. All checks compare in place over the Arc-shared values, no
    /// payload clone.
    pub(crate) async fn can_skip_relearn(&self, phone: &str, lid: &str) -> bool {
        self.is_persisted(phone, lid).await
            && self
                .pn_to_entry
                .get(phone)
                .await
                .is_some_and(|e| &*e.lid == lid)
            && self
                .lid_to_entry
                .get(lid)
                .await
                .is_some_and(|e| &*e.phone_number == phone)
    }

    /// Get the phone number for a LID.
    ///
    /// Returns the phone number user part if a mapping exists, None otherwise.
    pub async fn get_phone_number(&self, lid: &str) -> Option<String> {
        self.lid_to_entry
            .get(lid)
            .await
            .map(|e| e.phone_number.to_string())
    }

    /// Get the full entry for a LID.
    pub async fn get_entry_by_lid(&self, lid: &str) -> Option<LidPnEntry> {
        self.lid_to_entry.get(lid).await.map(|e| (*e).clone())
    }

    /// Get the full entry for a phone number.
    pub async fn get_entry_by_phone(&self, phone: &str) -> Option<LidPnEntry> {
        self.pn_to_entry.get(phone).await.map(|e| (*e).clone())
    }

    /// Add or update a mapping in the cache.
    ///
    /// For the LID -> Entry map, this always updates.
    /// For the PN -> Entry map, this only updates if the new entry has a
    /// newer or equal `created_at` timestamp (matching WhatsApp Web behavior).
    ///
    /// The writer guard keeps the get-then-insert sequence ordered within this
    /// client, including when the cache uses an external backend.
    pub async fn add(&self, entry: &LidPnEntry) {
        let guard = self.lock_mutation().await;
        self.add_guarded(entry, &guard).await;
    }

    pub(crate) async fn add_guarded(&self, entry: &LidPnEntry, guard: &LidPnMutationGuard<'_>) {
        self.add_guarded_impl(entry, guard, MappingWrite::Live)
            .await;
    }

    /// The write behind [`add_guarded`](Self::add_guarded). `record_topology`
    /// is `false` only for the startup warm-up, which records the whole batch
    /// as one scoped change instead of one lock round trip per entry. Scoped,
    /// not global: the warm-up task runs concurrently with the first live
    /// refreshes, and a global change would restart every one of them.
    async fn add_guarded_impl(
        &self,
        entry: &LidPnEntry,
        _guard: &LidPnMutationGuard<'_>,
        write: MappingWrite,
    ) {
        let (write_lid, live_pn_created_at, record_topology) = match write {
            MappingWrite::Live => (true, None, true),
            MappingWrite::WarmUp {
                preserve_lid,
                live_pn_created_at,
            } => (!preserve_lid, live_pn_created_at, false),
        };
        let previous_pn = self.pn_to_entry.get(&*entry.phone_number).await;
        let previous_lid = self.lid_to_entry.get(&*entry.lid).await;
        // Live writes break equal-timestamp ties, never defeat newer PN rows.
        // Once a newer snapshot row wins, later snapshot ties keep their usual
        // input ordering rather than inheriting the original live preference.
        let prefer_live_pn = previous_pn
            .as_ref()
            .is_some_and(|previous| live_pn_created_at == Some(previous.created_at));
        // A protected pair also supplies metadata when repairing its missing
        // counterpart. Unprotected external-cache rows follow durable timestamps.
        let entry = if !write_lid
            && let Some(existing) = &previous_lid
            && existing.phone_number == entry.phone_number
            && existing.created_at >= entry.created_at
        {
            existing.as_ref()
        } else if prefer_live_pn
            && let Some(existing) = &previous_pn
            && existing.lid == entry.lid
            && existing.created_at >= entry.created_at
        {
            existing.as_ref()
        } else {
            entry
        };
        let should_update_pn = match &previous_pn {
            Some(existing) => {
                existing.created_at < entry.created_at
                    || (existing.created_at == entry.created_at && !prefer_live_pn)
            }
            None => true,
        };
        let mapping_changed = (write_lid
            && previous_lid
                .as_ref()
                .is_none_or(|existing| existing.phone_number != entry.phone_number))
            || (should_update_pn
                && previous_pn
                    .as_ref()
                    .is_none_or(|existing| existing.lid != entry.lid));
        let _identity_change = mapping_changed.then(|| {
            self.identity_continuity.changing(
                [
                    Some(&*entry.lid),
                    Some(&*entry.phone_number),
                    previous_pn.as_ref().map(|previous| &*previous.lid),
                    previous_lid
                        .as_ref()
                        .map(|previous| &*previous.phone_number),
                ]
                .into_iter()
                .flatten(),
            )
        });

        let track_startup = record_topology && !self.startup_finished.load(Ordering::Acquire);
        // One shared copy of the entry; the keys clone the entry's own
        // Arc<str> allocations, so each identifier lives once per mapping.
        if write_lid || should_update_pn {
            let shared = Arc::new(entry.clone());
            if write_lid {
                if track_startup {
                    self.track_startup_write(|writes| {
                        writes
                            .lids
                            .insert(Arc::clone(&shared.lid), Arc::clone(&shared));
                    });
                }
                self.lid_to_entry
                    .insert(shared.lid.clone(), Arc::clone(&shared))
                    .await;
                self.contact_hash_to_lid
                    .insert(contact_hash_key(&shared.lid), Arc::clone(&shared.lid))
                    .await;
            }
            if should_update_pn {
                self.contact_hash_to_lid
                    .insert(
                        contact_hash_key(&shared.phone_number),
                        Arc::clone(&shared.lid),
                    )
                    .await;
                if track_startup {
                    self.track_startup_write(|writes| {
                        writes
                            .phones
                            .insert(Arc::clone(&shared.phone_number), Arc::clone(&shared));
                    });
                }
                self.pn_to_entry
                    .insert(shared.phone_number.clone(), shared)
                    .await;
            }
        }

        // Custom stores may survive this process; their winning entries still
        // need the always-local hash index rebuilt, without rewriting either map.
        if !record_topology {
            if !write_lid && let Some(existing) = &previous_lid {
                self.contact_hash_to_lid
                    .insert(contact_hash_key(&existing.lid), Arc::clone(&existing.lid))
                    .await;
                if existing.phone_number != entry.phone_number {
                    let owner = self.pn_to_entry.get(&*existing.phone_number).await;
                    self.contact_hash_to_lid
                        .insert(
                            contact_hash_key(&existing.phone_number),
                            owner.as_ref().map_or_else(
                                || Arc::clone(&existing.lid),
                                |owner| Arc::clone(&owner.lid),
                            ),
                        )
                        .await;
                }
            }
            if !should_update_pn && let Some(existing) = &previous_pn {
                self.contact_hash_to_lid
                    .insert(contact_hash_key(&existing.lid), Arc::clone(&existing.lid))
                    .await;
                self.contact_hash_to_lid
                    .insert(
                        contact_hash_key(&existing.phone_number),
                        Arc::clone(&existing.lid),
                    )
                    .await;
            }
        }

        if record_topology && let Some(topology) = self.topology.get() {
            topology.record([&*entry.lid, &*entry.phone_number]);
        }
    }

    /// Resolve the contact a `<devices>` `<update hash>` refers to.
    pub(crate) async fn lid_for_contact_hash(&self, hash: [u8; 3]) -> Option<Arc<str>> {
        self.contact_hash_to_lid.get(&pack_contact_hash(hash)).await
    }

    /// Whether this process has durably persisted exactly `phone -> lid`.
    /// Pair-keyed so a remap, or a stale detached write marking late, never
    /// reports a newer un-persisted LID as persisted. Clones no payload.
    pub(crate) async fn is_persisted(&self, phone: &str, lid: &str) -> bool {
        self.persisted
            .get(phone)
            .await
            .is_some_and(|stored| stored.as_ref() == lid)
    }

    /// Takes the entry's own identifiers: this map holds one pair per
    /// persisted contact for the process lifetime, so re-allocating both
    /// strings here would duplicate every mapping's payload a third time.
    pub(crate) async fn mark_persisted(&self, phone: &Arc<str>, lid: &Arc<str>) {
        self.persisted
            .insert(Arc::clone(phone), Arc::clone(lid))
            .await;
    }

    /// Warm up the cache with entries from persistent storage.
    ///
    /// This should be called during client initialization to populate
    /// the cache from the database. Initial bootstrap preserves locally written
    /// LID directions and prefers live PN entries on equal timestamps. Newer PN
    /// timestamps still win. Local hashes are rebuilt from the winning entries.
    pub async fn warm_up(&self, entries: impl IntoIterator<Item = LidPnEntry>) {
        let start = wacore::time::Instant::now();
        let mut count = 0;
        let guard = self.lock_mutation().await;
        let mut touched: Vec<Arc<str>> = Vec::new();

        // Validate only this instance's startup writes, not every stored row.
        // Best-effort failures can leave an occupied but stale predecessor.
        // Freeze protection before publication so snapshot timestamp ordering
        // cannot mistake a bootstrap insertion for a successful live write.
        let writes = self.take_startup_writes().unwrap_or_default();
        let mut live_lids = HashSet::with_capacity(writes.lids.len());
        for (lid, written) in writes.lids {
            if self
                .lid_to_entry
                .get(&*lid)
                .await
                .is_some_and(|cached| same_entry(&cached, &written))
            {
                live_lids.insert(lid);
            }
        }
        let mut live_phones = HashMap::with_capacity(writes.phones.len());
        for (phone, written) in writes.phones {
            if self
                .pn_to_entry
                .get(&*phone)
                .await
                .is_some_and(|cached| same_entry(&cached, &written))
            {
                live_phones.insert(phone, written.created_at);
            }
        }

        for entry in entries {
            let write = MappingWrite::WarmUp {
                preserve_lid: live_lids.contains(&entry.lid),
                live_pn_created_at: live_phones.get(&entry.phone_number).copied(),
            };
            self.add_guarded_impl(&entry, &guard, write).await;
            touched.push(Arc::clone(&entry.lid));
            touched.push(Arc::clone(&entry.phone_number));
            // `warm_up` only accepts durable rows. Mark the pair that won the
            // PN-side timestamp resolution so a live re-learn neither writes
            // it again nor repeats discovery migrations.
            if self
                .pn_to_entry
                .get(&*entry.phone_number)
                .await
                .is_some_and(|current| current.lid == entry.lid)
            {
                self.mark_persisted(&entry.phone_number, &entry.lid).await;
            }
            count += 1;
        }
        // One record for the batch; see `add_guarded_impl`. A batch wider
        // than the log's bound overflows it, which poisons every memo once,
        // exactly what per-entry records would have done. An empty batch
        // changed nothing and records nothing: a generation bump with no
        // users would still send every memo through a restamp.
        if !touched.is_empty()
            && let Some(topology) = self.topology.get()
        {
            topology.record(touched.iter().map(|id| &**id));
        }

        log::debug!(
            "LID-PN cache warmed up with {} entries in {:?}",
            count,
            start.elapsed()
        );
    }

    /// Clear all entries from the cache.
    ///
    /// Awaits the actual clear operation on custom backends (unlike
    /// `invalidate_all` which is fire-and-forget).
    #[cfg_attr(not(any(test, feature = "bench-harness")), allow(dead_code))]
    pub async fn clear(&self) {
        let _guard = self.lock_mutation().await;
        let _identity_change = self.identity_continuity.changing([]);
        self.lid_to_entry.clear().await;
        self.pn_to_entry.clear().await;
        self.persisted.clear().await;
        self.contact_hash_to_lid.clear().await;
        if let Some(topology) = self.topology.get() {
            topology.record_global();
        }
    }

    /// Get the number of LID entries in the cache.
    #[cfg_attr(not(any(test, feature = "bench-harness")), allow(dead_code))]
    pub async fn lid_count(&self) -> u64 {
        self.lid_to_entry.run_pending_tasks().await;
        self.lid_to_entry.entry_count()
    }

    /// Get the number of phone number entries in the cache.
    #[cfg_attr(not(any(test, feature = "bench-harness")), allow(dead_code))]
    pub async fn pn_count(&self) -> u64 {
        self.pn_to_entry.run_pending_tasks().await;
        self.pn_to_entry.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn evictable_alias_cache_cannot_authorize_historical_sends() {
        for config in [
            CacheEntryConfig::new(None, 1),
            CacheEntryConfig::new(Some(std::time::Duration::from_secs(60)), u64::MAX),
        ] {
            let cache = LidPnCache::with_config(&config, None);
            assert_eq!(cache.identity_continuity.snapshot(), None);
        }
        assert!(LidPnCache::new().identity_continuity.snapshot().is_some());
    }

    #[tokio::test]
    async fn alias_journal_records_both_ends_of_replaced_mappings() {
        let cache = LidPnCache::new();
        let original = LidPnEntry::new("100000000000001", "15550000001", LearningSource::Usync);
        cache.add(&original).await;
        let sent = cache.identity_continuity.snapshot();
        let mut replacement = original.clone();
        replacement.lid = "100000000000002".into();
        cache.add(&replacement).await;
        replacement.phone_number = "15550000002".into();
        cache.add(&replacement).await;
        for user in [
            "100000000000001",
            "100000000000002",
            "15550000001",
            "15550000002",
        ] {
            assert!(!cache.identity_continuity.unchanged_for(sent, [user]));
        }
        assert!(
            cache
                .identity_continuity
                .unchanged_for(sent, ["15550000003"])
        );
    }

    #[tokio::test]
    async fn test_basic_operations() {
        let cache = LidPnCache::new();

        // Initially empty
        assert!(cache.get_current_lid("559980000001").await.is_none());
        assert!(cache.get_phone_number("100000012345678").await.is_none());

        // Add a mapping
        let entry = LidPnEntry::new(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            LearningSource::Usync,
        );
        cache.add(&entry).await;

        // Should be retrievable both ways
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000012345678")
        );
        assert_eq!(
            cache.get_phone_number("100000012345678").await,
            Some("559980000001".to_string())
        );
    }

    #[tokio::test]
    async fn test_timestamp_conflict_resolution() {
        let cache = LidPnCache::new();

        // Add old mapping
        let old_entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Other,
        );
        cache.add(&old_entry).await;

        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000012345678")
        );

        // Add newer mapping for same phone (different LID)
        let new_entry = LidPnEntry::with_timestamp(
            "100000087654321".to_string(),
            "559980000001".to_string(),
            2000,
            LearningSource::Usync,
        );
        cache.add(&new_entry).await;

        // Should return the newer LID for PN lookup
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000087654321")
        );

        // Both LIDs should still be in the LID -> Entry map
        assert_eq!(
            cache.get_phone_number("100000012345678").await,
            Some("559980000001".to_string())
        );
        assert_eq!(
            cache.get_phone_number("100000087654321").await,
            Some("559980000001".to_string())
        );
    }

    #[tokio::test]
    async fn test_older_entry_does_not_override() {
        let cache = LidPnCache::new();

        // Add new mapping first
        let new_entry = LidPnEntry::with_timestamp(
            "100000087654321".to_string(),
            "559980000001".to_string(),
            2000,
            LearningSource::Usync,
        );
        cache.add(&new_entry).await;

        // Try to add older mapping
        let old_entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Other,
        );
        cache.add(&old_entry).await;

        // PN -> LID should still return the newer one
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000087654321")
        );

        // The number's contact hash follows the same winner, or a device
        // update naming that number would refresh the superseded LID.
        let phone_hash = wacore::crypto::contact_notification_hash("559980000001");
        assert_eq!(
            cache.lid_for_contact_hash(phone_hash).await.as_deref(),
            Some("100000087654321")
        );
        // Each LID still resolves to itself: the loser is a real contact too.
        for lid in ["100000087654321", "100000012345678"] {
            assert_eq!(
                cache
                    .lid_for_contact_hash(wacore::crypto::contact_notification_hash(lid))
                    .await
                    .as_deref(),
                Some(lid)
            );
        }
    }

    #[tokio::test]
    async fn test_warm_up() {
        let cache = LidPnCache::new();

        let entries = vec![
            LidPnEntry::with_timestamp(
                "lid1".to_string(),
                "pn1".to_string(),
                1,
                LearningSource::Other,
            ),
            LidPnEntry::with_timestamp(
                "lid2".to_string(),
                "pn2".to_string(),
                2,
                LearningSource::Usync,
            ),
            LidPnEntry::with_timestamp(
                "lid3".to_string(),
                "pn3".to_string(),
                3,
                LearningSource::PeerPnMessage,
            ),
        ];

        cache.warm_up(entries).await;

        assert_eq!(cache.lid_count().await, 3);
        assert_eq!(cache.pn_count().await, 3);

        assert_eq!(cache.get_current_lid("pn1").await.as_deref(), Some("lid1"));
        assert_eq!(cache.get_current_lid("pn2").await.as_deref(), Some("lid2"));
        assert_eq!(cache.get_current_lid("pn3").await.as_deref(), Some("lid3"));
        assert!(cache.can_skip_relearn("pn1", "lid1").await);
        assert!(cache.can_skip_relearn("pn2", "lid2").await);
        assert!(cache.can_skip_relearn("pn3", "lid3").await);
    }

    #[tokio::test]
    async fn warm_up_preserves_cached_mappings_and_metadata() {
        for (stored_lid, stored_pn, stored_at) in [
            ("lid1", "old_pn", 20),
            ("old_lid", "pn1", 10),
            ("lid1", "pn1", 10),
        ] {
            let cache = LidPnCache::new();
            let live = LidPnEntry::with_timestamp("lid1", "pn1", 10, LearningSource::Usync);
            cache.add(&live).await;
            // Reverse mappings keep the live write; PN mappings keep it on a
            // timestamp tie. A strictly newer PN row is covered separately.
            cache
                .warm_up([LidPnEntry::with_timestamp(
                    stored_lid,
                    stored_pn,
                    stored_at,
                    LearningSource::Other,
                )])
                .await;
            for entry in [
                cache.get_entry_by_lid("lid1").await.unwrap(),
                cache.get_entry_by_phone("pn1").await.unwrap(),
            ] {
                assert_eq!(entry.lid, live.lid);
                assert_eq!(entry.phone_number, live.phone_number);
                assert_eq!(entry.created_at, live.created_at);
                assert_eq!(entry.learning_source, live.learning_source);
            }
            assert_eq!(
                cache.can_skip_relearn("pn1", "lid1").await,
                stored_lid == "lid1" && stored_pn == "pn1"
            );
            if stored_lid != "lid1" {
                assert_eq!(
                    cache.get_phone_number(stored_lid).await.as_deref(),
                    Some(stored_pn)
                );
            }
        }
    }

    #[derive(Default)]
    struct SharedCacheStore {
        entries: std::sync::Mutex<HashMap<(String, String), Vec<u8>>>,
        fail_namespace: std::sync::Mutex<Option<&'static str>>,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl wacore::store::CacheStore for SharedCacheStore {
        async fn get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self
                .entries
                .lock()
                .unwrap()
                .get(&(namespace.into(), key.into()))
                .cloned())
        }

        async fn set(
            &self,
            namespace: &str,
            key: &str,
            value: &[u8],
            _: Option<std::time::Duration>,
        ) -> anyhow::Result<()> {
            if self
                .fail_namespace
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|blocked| *blocked == namespace)
            {
                anyhow::bail!("synthetic cache write failure");
            }
            self.entries
                .lock()
                .unwrap()
                .insert((namespace.into(), key.into()), value.into());
            Ok(())
        }

        async fn delete(&self, namespace: &str, key: &str) -> anyhow::Result<()> {
            self.entries
                .lock()
                .unwrap()
                .remove(&(namespace.into(), key.into()));
            Ok(())
        }

        async fn clear(&self, namespace: &str) -> anyhow::Result<()> {
            self.entries
                .lock()
                .unwrap()
                .retain(|(stored, _), _| stored != namespace);
            Ok(())
        }
    }

    #[tokio::test]
    async fn warm_up_restores_custom_store_hashes_and_historical_lids() {
        let store = Arc::new(SharedCacheStore::default());
        let config = CacheConfig::default().lid_pn_cache;
        let live = LidPnEntry::with_timestamp("new_lid", "pn1", 10, LearningSource::Usync);
        let first = LidPnCache::with_config(&config, Some(store.clone()));
        first.add(&live).await;
        drop(first);

        let restarted = LidPnCache::with_config(&config, Some(store));
        let hash = wacore::crypto::contact_notification_hash;
        assert!(restarted.lid_for_contact_hash(hash("pn1")).await.is_none());
        restarted
            .warm_up([LidPnEntry::with_timestamp(
                "old_lid",
                "pn1",
                1,
                LearningSource::Other,
            )])
            .await;
        assert_eq!(
            restarted.get_current_lid("pn1").await.as_deref(),
            Some("new_lid")
        );
        assert_eq!(
            restarted.get_phone_number("old_lid").await.as_deref(),
            Some("pn1")
        );
        for (user, expected_lid) in [
            ("pn1", "new_lid"),
            ("new_lid", "new_lid"),
            ("old_lid", "old_lid"),
        ] {
            assert_eq!(
                restarted.lid_for_contact_hash(hash(user)).await.as_deref(),
                Some(expected_lid)
            );
        }
        let cached = restarted.get_entry_by_lid("new_lid").await.unwrap();
        assert_eq!(cached.created_at, live.created_at);
        assert_eq!(cached.learning_source, live.learning_source);
        assert!(!restarted.can_skip_relearn("pn1", "new_lid").await);
    }

    #[tokio::test]
    async fn warm_up_heals_either_missing_custom_store_direction() {
        use wacore::store::CacheStore;

        for (namespace, key) in [(NS_LID, "lid1"), (NS_PN, "pn1")] {
            let store = Arc::new(SharedCacheStore::default());
            let config = CacheConfig::default().lid_pn_cache;
            let live = LidPnEntry::with_timestamp("lid1", "pn1", 10, LearningSource::Usync);
            let first = LidPnCache::with_config(&config, Some(store.clone()));
            first.add(&live).await;
            // Model a previous best-effort write reaching only one direction.
            store.delete(namespace, key).await.unwrap();
            drop(first);
            let restarted = LidPnCache::with_config(&config, Some(store));
            restarted
                .warm_up([LidPnEntry::with_timestamp(
                    "lid1",
                    "pn1",
                    1,
                    LearningSource::Other,
                )])
                .await;
            let by_lid = restarted.get_entry_by_lid("lid1").await.unwrap();
            let by_phone = restarted.get_entry_by_phone("pn1").await.unwrap();
            for entry in [&by_lid, &by_phone] {
                assert_eq!(entry.lid, live.lid);
                assert_eq!(entry.phone_number, live.phone_number);
            }
            // A fresh instance restores the durable reverse row. The PN side
            // retains a surviving newer timestamp, or loads storage if missing.
            assert_eq!(by_lid.created_at, 1);
            assert_eq!(by_lid.learning_source, LearningSource::Other);
            assert_eq!(
                by_phone.created_at,
                if namespace == NS_LID { 10 } else { 1 }
            );
            assert_eq!(
                by_phone.learning_source,
                if namespace == NS_LID {
                    LearningSource::Usync
                } else {
                    LearningSource::Other
                }
            );
            for user in ["lid1", "pn1"] {
                assert_eq!(
                    restarted
                        .lid_for_contact_hash(wacore::crypto::contact_notification_hash(user))
                        .await
                        .as_deref(),
                    Some("lid1")
                );
            }
            assert!(restarted.can_skip_relearn("pn1", "lid1").await);
        }
    }

    #[tokio::test]
    async fn startup_provenance_releases_entries_after_completion_or_early_exit() {
        for complete in [false, true] {
            let store = Arc::new(SharedCacheStore::default());
            let cache = LidPnCache::with_config(&CacheConfig::default().lid_pn_cache, Some(store));
            assert!(cache.startup_writes.lock().unwrap().is_none());
            let entry = LidPnEntry::with_timestamp("lid1", "pn1", 10, LearningSource::Usync);
            // The custom store keeps bytes, and the local hash index keeps LIDs.
            // Only transient startup provenance can retain this phone allocation.
            let phone = Arc::downgrade(&entry.phone_number);
            cache.add(&entry).await;
            assert!(cache.startup_writes.lock().unwrap().is_some());
            drop(entry);
            assert!(phone.upgrade().is_some());
            if complete {
                cache
                    .warm_up([LidPnEntry::with_timestamp(
                        "lid1",
                        "pn1",
                        1,
                        LearningSource::Other,
                    )])
                    .await;
            } else {
                let scope = cache.warm_up_guard();
                drop(scope);
            }
            assert!(phone.upgrade().is_none());

            // Model a writer that passed the outer fast-path check before the
            // warm-up completed, then acquired the provenance lock afterwards.
            cache.track_startup_write(|_| panic!("finished startup must reject late writers"));
            assert!(cache.startup_writes.lock().unwrap().is_none());

            let later = LidPnEntry::with_timestamp("lid2", "pn2", 20, LearningSource::Usync);
            let phone = Arc::downgrade(&later.phone_number);
            cache.add(&later).await;
            drop(later);
            assert!(phone.upgrade().is_none());
        }
    }

    #[tokio::test]
    async fn warm_up_repairs_occupied_stale_custom_store_directions() {
        for namespace in [NS_PN, NS_LID] {
            for restart in [true, false] {
                let store = Arc::new(SharedCacheStore::default());
                let config = CacheConfig::default().lid_pn_cache;
                let (old, durable) = if namespace == NS_PN {
                    (
                        LidPnEntry::with_timestamp("old_lid", "pn1", 1, LearningSource::Other),
                        LidPnEntry::with_timestamp("new_lid", "pn1", 2, LearningSource::Usync),
                    )
                } else {
                    (
                        LidPnEntry::with_timestamp("lid1", "old_pn", 1, LearningSource::Other),
                        LidPnEntry::with_timestamp("lid1", "pn1", 2, LearningSource::Usync),
                    )
                };
                let cache = LidPnCache::with_config(&config, Some(store.clone()));
                cache.add(&old).await;
                *store.fail_namespace.lock().unwrap() = Some(namespace);
                cache.add(&durable).await;
                *store.fail_namespace.lock().unwrap() = None;
                if namespace == NS_PN {
                    assert_eq!(
                        cache.get_current_lid("pn1").await.as_deref(),
                        Some("old_lid")
                    );
                } else {
                    assert_eq!(
                        cache.get_phone_number("lid1").await.as_deref(),
                        Some("old_pn")
                    );
                }
                let cache = if restart {
                    drop(cache);
                    LidPnCache::with_config(&config, Some(store))
                } else {
                    cache
                };
                cache.warm_up([durable.clone()]).await;
                for entry in [
                    cache.get_entry_by_lid(&durable.lid).await.unwrap(),
                    cache
                        .get_entry_by_phone(&durable.phone_number)
                        .await
                        .unwrap(),
                ] {
                    assert_eq!(entry.lid, durable.lid);
                    assert_eq!(entry.phone_number, durable.phone_number);
                    assert_eq!(entry.created_at, durable.created_at);
                    assert_eq!(entry.learning_source, durable.learning_source);
                }
                for user in [&*durable.lid, &*durable.phone_number] {
                    assert_eq!(
                        cache
                            .lid_for_contact_hash(wacore::crypto::contact_notification_hash(user))
                            .await
                            .as_deref(),
                        Some(&*durable.lid)
                    );
                }
                assert!(
                    cache
                        .can_skip_relearn(&durable.phone_number, &durable.lid)
                        .await
                );
            }
        }
    }

    #[tokio::test]
    async fn warm_up_pn_timestamps_are_independent_of_startup_order() {
        for (source, live_at, stored_at, winner) in [
            (LearningSource::MigrationSyncOld, 0, 20, "stored_lid"),
            (LearningSource::BlocklistInactive, 0, 20, "stored_lid"),
            (LearningSource::Usync, 10, 20, "stored_lid"),
            (LearningSource::Usync, 10, 10, "live_lid"),
            (LearningSource::Usync, 20, 10, "live_lid"),
            (LearningSource::BlocklistInactive, 0, 0, "live_lid"),
        ] {
            for delayed in [false, true] {
                let cache = LidPnCache::new();
                let live = LidPnEntry::with_timestamp("live_lid", "pn1", live_at, source);
                let stored = LidPnEntry::with_timestamp(
                    "stored_lid",
                    "pn1",
                    stored_at,
                    LearningSource::Other,
                );
                if delayed {
                    cache.add(&live).await;
                    cache.warm_up([stored.clone()]).await;
                } else {
                    cache.warm_up([stored.clone()]).await;
                    cache.add(&live).await;
                }
                let expected = if winner == "live_lid" { &live } else { &stored };
                let current = cache.get_entry_by_phone("pn1").await.unwrap();
                assert_eq!(
                    &*current.lid, winner,
                    "source={source:?}, delayed={delayed}"
                );
                assert_eq!(current.phone_number, expected.phone_number);
                assert_eq!(current.created_at, expected.created_at);
                assert_eq!(current.learning_source, expected.learning_source);
                for lid in ["live_lid", "stored_lid"] {
                    assert_eq!(cache.get_phone_number(lid).await.as_deref(), Some("pn1"));
                }
            }
        }
    }

    #[tokio::test]
    async fn warm_up_newer_pn_metadata_survives_live_reverse_protection() {
        for other_at in [15, 20] {
            let live = LidPnEntry::with_timestamp("lid1", "pn1", 10, LearningSource::Usync);
            let newer = LidPnEntry::with_timestamp("lid1", "pn1", 20, LearningSource::Other);
            let other = LidPnEntry::with_timestamp("lid2", "pn1", other_at, LearningSource::Other);
            for entries in [
                [newer.clone(), other.clone()],
                [other.clone(), newer.clone()],
            ] {
                let expected = if other_at == 20 {
                    entries.last().unwrap()
                } else {
                    &newer
                };
                let cache = LidPnCache::new();
                cache.add(&live).await;
                cache.warm_up(entries.clone()).await;
                let current = cache.get_entry_by_phone("pn1").await.unwrap();
                assert_eq!(current.lid, expected.lid);
                assert_eq!(current.phone_number, expected.phone_number);
                assert_eq!(current.created_at, expected.created_at);
                assert_eq!(current.learning_source, expected.learning_source);
                let reverse = cache.get_entry_by_lid("lid1").await.unwrap();
                assert_eq!(reverse.phone_number, live.phone_number);
                assert_eq!(reverse.created_at, live.created_at);
                assert_eq!(reverse.learning_source, live.learning_source);
            }
        }
    }

    #[tokio::test]
    async fn warm_up_keeps_snapshot_timestamp_ordering_in_both_input_orders() {
        let old = LidPnEntry::with_timestamp("old_lid", "pn1", 1, LearningSource::Other);
        let latest = LidPnEntry::with_timestamp("new_lid", "pn1", 2, LearningSource::Usync);
        for entries in [[old.clone(), latest.clone()], [latest.clone(), old.clone()]] {
            let cache = LidPnCache::new();
            cache.warm_up(entries).await;
            assert_eq!(
                cache.get_current_lid("pn1").await.as_deref(),
                Some("new_lid")
            );
            assert_eq!(
                cache.get_phone_number("old_lid").await.as_deref(),
                Some("pn1")
            );
            assert_eq!(
                cache.get_phone_number("new_lid").await.as_deref(),
                Some("pn1")
            );
            assert!(cache.can_skip_relearn("pn1", "new_lid").await);
        }
    }

    /// The startup warm-up races the first live refreshes, so it must record
    /// the batch as a scoped change: a refresh of a user it did not touch
    /// keeps its result, one of a user it did recomputes.
    #[tokio::test]
    async fn warm_up_records_a_scoped_change() {
        let cache = LidPnCache::new();
        let topology = crate::client::device_topology::DeviceTopology::new();
        cache.attach_topology(Arc::clone(&topology));
        let before = topology.current();

        cache
            .warm_up([LidPnEntry::with_timestamp(
                "lid1".to_string(),
                "pn1".to_string(),
                1,
                LearningSource::Other,
            )])
            .await;

        use crate::client::member_index::MemberIndex;
        let members = |user: &str| MemberIndex::from_users([user]);
        assert!(topology.unchanged_for(before, &members("unrelated")));
        assert!(!topology.unchanged_for(before, &members("pn1")));
        assert!(!topology.unchanged_for(before, &members("lid1")));
    }

    #[tokio::test]
    async fn test_clear() {
        let cache = LidPnCache::new();

        let entry = LidPnEntry::new(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            LearningSource::Usync,
        );
        cache.add(&entry).await;

        assert_eq!(cache.lid_count().await, 1);
        assert_eq!(cache.pn_count().await, 1);

        cache.clear().await;
        assert_eq!(cache.lid_count().await, 0);
        assert_eq!(cache.pn_count().await, 0);
        assert!(cache.get_current_lid("559980000001").await.is_none());
    }

    #[tokio::test]
    async fn persisted_marker_is_pair_specific() {
        let cache = LidPnCache::new();
        let pn = "559980000099";
        let (lid_a, lid_b) = ("100000000000001", "100000000000002");
        assert!(!cache.is_persisted(pn, lid_a).await);
        cache
            .mark_persisted(&Arc::from(pn), &Arc::from(lid_a))
            .await;
        assert!(cache.is_persisted(pn, lid_a).await);
        // A remap to a new LID is not persisted (even a stale mark of the old
        // LID never satisfies the new pair), so it re-persists.
        assert!(!cache.is_persisted(pn, lid_b).await);
    }

    #[tokio::test]
    async fn skip_requires_both_cache_directions() {
        let cache = LidPnCache::new();
        let (pn, lid) = ("559980000099", "100000000000001");
        cache
            .add(&LidPnEntry::new(
                lid.to_string(),
                pn.to_string(),
                LearningSource::PeerPnMessage,
            ))
            .await;
        cache.mark_persisted(&Arc::from(pn), &Arc::from(lid)).await;
        assert!(cache.can_skip_relearn(pn, lid).await);

        // Evict the reverse (LID -> PN) entry: the fast path must stop skipping
        // so the next live message re-warms it (get_phone_number has no fallback).
        cache.lid_to_entry.invalidate(lid).await;
        assert!(
            !cache.can_skip_relearn(pn, lid).await,
            "skip must require the reverse map, not just PN -> LID"
        );
    }

    #[tokio::test]
    async fn unbounded_table_resolves_all_four_indexes() {
        let cache = LidPnCache::new();
        let entry = LidPnEntry::with_timestamp(
            "100000012345678".to_string(),
            "559980000001".to_string(),
            1000,
            LearningSource::Usync,
        );
        cache.add(&entry).await;
        cache.mark_persisted(&entry.phone_number, &entry.lid).await;

        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000012345678")
        );
        assert_eq!(
            cache.get_phone_number("100000012345678").await.as_deref(),
            Some("559980000001")
        );
        for user in ["100000012345678", "559980000001"] {
            let hash = wacore::crypto::contact_notification_hash(user);
            assert_eq!(
                cache.lid_for_contact_hash(hash).await.as_deref(),
                Some("100000012345678")
            );
        }
        assert!(cache.is_persisted("559980000001", "100000012345678").await);
        assert!(
            cache
                .can_skip_relearn("559980000001", "100000012345678")
                .await
        );
        assert_eq!(cache.lid_count().await, 1);
        assert_eq!(cache.pn_count().await, 1);
        cache.run_pending_tasks().await;
        assert_eq!(cache.lid_count().await, 1);
        assert_eq!(
            cache.get_current_lid("559980000001").await.as_deref(),
            Some("100000012345678"),
            "maintenance must not drop unbounded entries"
        );
    }

    #[tokio::test]
    async fn bounded_table_still_evicts_and_expiring_table_still_expires() {
        let evicting = LidPnCache::with_config(&CacheEntryConfig::new(None, 1), None);
        evicting
            .add(&LidPnEntry::with_timestamp(
                "100000000000001".to_string(),
                "15550000001".to_string(),
                1,
                LearningSource::Usync,
            ))
            .await;
        evicting
            .add(&LidPnEntry::with_timestamp(
                "100000000000002".to_string(),
                "15550000002".to_string(),
                2,
                LearningSource::Usync,
            ))
            .await;
        assert_eq!(
            evicting.lid_count().await,
            1,
            "capacity-1 LID map keeps the managed eviction policy"
        );

        let expiring = LidPnCache::with_config(
            &CacheEntryConfig::new(Some(std::time::Duration::from_millis(50)), 1_000),
            None,
        );
        expiring
            .add(&LidPnEntry::new(
                "100000000000003".to_string(),
                "15550000003".to_string(),
                LearningSource::Usync,
            ))
            .await;
        assert!(expiring.get_current_lid("15550000003").await.is_some());
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
        assert!(
            expiring.get_current_lid("15550000003").await.is_none(),
            "idle expiry still applies on the managed path"
        );
    }

    #[tokio::test]
    async fn unbounded_replacement_keeps_pn_winner_rule() {
        let cache = LidPnCache::new();
        cache
            .add(&LidPnEntry::with_timestamp(
                "100000000000001".to_string(),
                "15550000004".to_string(),
                2000,
                LearningSource::Usync,
            ))
            .await;
        cache
            .add(&LidPnEntry::with_timestamp(
                "100000000000002".to_string(),
                "15550000004".to_string(),
                1000,
                LearningSource::Other,
            ))
            .await;
        assert_eq!(
            cache.get_current_lid("15550000004").await.as_deref(),
            Some("100000000000001"),
            "an older PN mapping must not displace the winner"
        );
        let phone_hash = wacore::crypto::contact_notification_hash("15550000004");
        assert_eq!(
            cache.lid_for_contact_hash(phone_hash).await.as_deref(),
            Some("100000000000001")
        );
        assert_eq!(
            cache.get_phone_number("100000000000002").await.as_deref(),
            Some("15550000004"),
            "the losing LID stays resolvable"
        );
    }

    #[test]
    fn test_learning_source_serialization() {
        let sources = [
            (LearningSource::Usync, "usync"),
            (LearningSource::PeerPnMessage, "peer_pn_message"),
            (LearningSource::PeerLidMessage, "peer_lid_message"),
            (LearningSource::RecipientLatestLid, "recipient_latest_lid"),
            (LearningSource::MigrationSyncLatest, "migration_sync_latest"),
            (LearningSource::MigrationSyncOld, "migration_sync_old"),
            (LearningSource::BlocklistActive, "blocklist_active"),
            (LearningSource::BlocklistInactive, "blocklist_inactive"),
            (LearningSource::Pairing, "pairing"),
            (LearningSource::DeviceNotification, "device_notification"),
            (LearningSource::Other, "other"),
        ];

        for (source, expected_str) in sources {
            assert_eq!(source.as_str(), expected_str);
            assert_eq!(LearningSource::parse(expected_str), source);
        }

        // Unknown string should map to Other
        assert_eq!(LearningSource::parse("unknown"), LearningSource::Other);
    }
}
