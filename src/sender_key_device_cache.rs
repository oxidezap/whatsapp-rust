//! In-memory cache for per-group sender key device tracking.
//! Avoids DB round-trips on group sends after the first.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use portable_atomic::AtomicU64;

use crate::cache::Cache;
use crate::cache_config::CacheEntryConfig;
use wacore_binary::Jid;

/// Pre-parsed, pre-indexed sender key device map for one group.
///
/// `has_key` is an [`AtomicBool`] so `markForgetSenderKey` can flip one device
/// cold in place, matching WA Web's per-device participant-record update,
/// instead of invalidating the whole group and forcing the next send to re-read
/// and re-parse every row from the DB. An in-place flip keeps the same `Arc`, so
/// `generation` is the version stamp the `skdm_warm_memo` compares to notice the
/// change (pointer identity alone cannot).
#[derive(Debug)]
pub(crate) struct SenderKeyDeviceMap {
    /// user → (device_id → has_key)
    devices: HashMap<Arc<str>, HashMap<u16, AtomicBool>>,
    /// Bumped on every in-place warm-state change. Same freshness contract the
    /// device-registry generation gives membership.
    generation: AtomicU64,
}

impl SenderKeyDeviceMap {
    pub fn from_db_rows(rows: &[(String, bool)]) -> Self {
        let mut devices: HashMap<Arc<str>, HashMap<u16, AtomicBool>> =
            HashMap::with_capacity(rows.len());

        for (jid_str, has_key) in rows {
            match jid_str.parse::<Jid>() {
                Ok(jid) => {
                    let user: Arc<str> = Arc::from(jid.user.as_str());
                    devices
                        .entry(user)
                        .or_default()
                        .insert(jid.device, AtomicBool::new(*has_key));
                }
                Err(e) => {
                    log::warn!("Skipping malformed device JID '{}': {}", jid_str, e);
                }
            }
        }

        Self {
            devices,
            generation: AtomicU64::new(0),
        }
    }

    /// Monotonic version stamp for the warm state. The `skdm_warm_memo` records
    /// this value and rejects its skip once it advances, so an in-place cold
    /// flip (which does not swap the `Arc`) is still detected. Load this BEFORE
    /// reading device state, so a racing flip stamps the memo as already stale.
    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Single (user, device) lookup. Retained for tests that cross-check the
    /// warm gate; production resolves both lookups via `device_and_primary_warm`.
    #[cfg(test)]
    pub fn device_has_key(&self, user: &str, device: u16) -> Option<bool> {
        Some(
            self.devices
                .get(user)?
                .get(&device)?
                .load(Ordering::Relaxed),
        )
    }

    /// WA Web warm gate (ParticipantStore.js): a device is warm only when it AND
    /// its primary (device 0) hold the key. Resolves the per-user inner map once
    /// so the two device lookups share a single outer (user-string) hash instead
    /// of re-hashing the user per call. A missing entry counts as cold.
    pub fn device_and_primary_warm(&self, user: &str, device: u16) -> bool {
        let Some(by_device) = self.devices.get(user) else {
            return false;
        };
        by_device
            .get(&device)
            .is_some_and(|k| k.load(Ordering::Relaxed))
            && by_device.get(&0).is_some_and(|k| k.load(Ordering::Relaxed))
    }
}

pub(crate) struct SenderKeyDeviceCache {
    inner: Cache<String, Arc<SenderKeyDeviceMap>>,
}

impl SenderKeyDeviceCache {
    pub(crate) fn new(config: &CacheEntryConfig) -> Self {
        Self {
            inner: config.build_with_tti(),
        }
    }

    /// Atomically get-or-init: returns cached value or runs `init` once per key.
    /// Concurrent callers for the same key share the single init result.
    pub(crate) async fn get_or_init<F>(&self, group_jid: &str, init: F) -> Arc<SenderKeyDeviceMap>
    where
        F: std::future::Future<Output = Arc<SenderKeyDeviceMap>>,
    {
        self.inner.get_with_by_ref(group_jid, init).await
    }

    pub(crate) async fn invalidate(&self, group_jid: &str) {
        self.inner.invalidate(group_jid).await;
    }

    /// Flip the given devices to `has_key=false` in place, if this group's map
    /// is cached, and bump the map's generation so the `skdm_warm_memo` re-runs
    /// its target filter. Matches WA Web's per-device `markForgetSenderKey`: no
    /// whole-group invalidation, so a storm of retry receipts never forces the
    /// next send to re-read every row. A device absent from the map is already
    /// cold, so it is skipped. The DB write is the source of truth; this only
    /// keeps a live cache entry consistent with it. On a cache miss the next
    /// send rebuilds from the DB, which already carries the write.
    pub(crate) async fn mark_forgotten<'a>(
        &self,
        group_jid: &str,
        devices: impl Iterator<Item = &'a Jid> + Send,
    ) {
        let Some(map) = self.inner.get(group_jid).await else {
            return;
        };
        let mut changed = false;
        for jid in devices {
            if let Some(by_device) = map.devices.get(jid.user.as_str())
                && let Some(flag) = by_device.get(&jid.device)
                && flag.swap(false, Ordering::Relaxed)
            {
                // Only a real high→low transition is a warm-state change; a
                // device already cold must not advance the generation, or a
                // retry storm would churn the warm memo with no-op misses.
                changed = true;
            }
        }
        if changed {
            // Release publishes the flip(s); the memo's Acquire load of the
            // generation then also observes the cold state.
            map.generation.fetch_add(1, Ordering::Release);
        }
    }

    /// Drop cache entries whose map indexes the given (user, device_id). Needed
    /// after a device is removed: a future re-add of the same device_id would
    /// otherwise hit a stale `has_key=true` entry and skip SKDM redistribution.
    pub(crate) async fn invalidate_entries_for_device(&self, user: &str, device_id: u16) {
        // Reliable awaited snapshot, not the best-effort `iter()`: a skipped
        // entry here would leave a stale `has_key=true` and drop a later SKDM
        // fanout for a re-added device.
        let to_drop: Vec<String> = self
            .inner
            .snapshot_entries()
            .await
            .into_iter()
            .filter_map(|(group_jid, map)| {
                map.devices
                    .get(user)
                    .and_then(|devmap| devmap.get(&device_id))
                    .map(|_| group_jid.as_ref().clone())
            })
            .collect();
        for g in to_drop {
            self.inner.invalidate(&g).await;
        }
    }

    /// Approximate entry count plus estimated retained bytes.
    pub(crate) async fn memory_stats(&self) -> wacore::stats::CollectionStats {
        // Slot allocations use capacity() (outer and inner maps alike);
        // per-entry heap is summed by iteration.
        self.inner
            .memory_stats(|k, v| {
                k.capacity()
                    + v.devices.capacity()
                        * std::mem::size_of::<(Arc<str>, HashMap<u16, AtomicBool>)>()
                    + v.devices
                        .iter()
                        .map(|(user, by_device)| {
                            user.len()
                                + by_device.capacity() * std::mem::size_of::<(u16, AtomicBool)>()
                        })
                        .sum::<usize>()
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache_config::CacheEntryConfig;

    fn cache() -> SenderKeyDeviceCache {
        SenderKeyDeviceCache::new(&CacheEntryConfig::new(None, 100))
    }

    #[tokio::test]
    async fn mark_forgotten_flips_in_place_without_invalidating() {
        let c = cache();
        let group = "120363000000000001@g.us";
        let map0 = c
            .get_or_init(group, async {
                Arc::new(SenderKeyDeviceMap::from_db_rows(&[
                    ("111:0@lid".to_string(), true),
                    ("111:5@lid".to_string(), true),
                    ("222:0@lid".to_string(), true),
                ]))
            })
            .await;
        let gen_before = map0.generation();

        let dev5: Jid = "111:5@lid".parse().unwrap();
        c.mark_forgotten(group, std::iter::once(&dev5)).await;

        // Still cached (no whole-group invalidation): reading it must not run
        // the init closure.
        let map = c
            .get_or_init(group, async { panic!("group was invalidated") })
            .await;
        // The forgotten device is cold; its sibling and the other user are
        // untouched, which the blunt whole-group invalidate could not preserve.
        assert_eq!(map.device_has_key("111", 5), Some(false));
        assert_eq!(map.device_has_key("111", 0), Some(true));
        assert_eq!(map.device_has_key("222", 0), Some(true));
        assert!(!map.device_and_primary_warm("111", 5));
        assert!(map.device_and_primary_warm("222", 0));
        // Same Arc, advanced generation: the warm memo detects the change.
        assert!(Arc::ptr_eq(&map0, &map));
        assert_ne!(map.generation(), gen_before);

        // A mark for a device the map doesn't hold changes nothing, so the
        // generation must not advance (no spurious memo miss).
        let gen_after = map.generation();
        let absent: Jid = "999:0@lid".parse().unwrap();
        c.mark_forgotten(group, std::iter::once(&absent)).await;
        assert_eq!(
            map.generation(),
            gen_after,
            "no-op mark must not bump generation"
        );

        // Re-marking an already-cold device it DOES hold is also a no-op: the
        // flag is already false, so a retry storm must not churn the generation.
        c.mark_forgotten(group, std::iter::once(&dev5)).await;
        assert_eq!(
            map.generation(),
            gen_after,
            "duplicate cold mark must not bump generation"
        );
    }

    #[tokio::test]
    async fn mark_forgotten_flips_a_mixed_batch_and_bumps_once() {
        let c = cache();
        let group = "120363000000000001@g.us";
        let map = c
            .get_or_init(group, async {
                Arc::new(SenderKeyDeviceMap::from_db_rows(&[
                    ("111:0@lid".to_string(), true),
                    ("111:5@lid".to_string(), true),
                    ("222:0@lid".to_string(), true),
                ]))
            })
            .await;
        let gen_before = map.generation();

        // One receipt naming two present devices and one the map doesn't hold:
        // both present flip cold, the absent one is skipped, and the whole batch
        // advances the generation exactly once (not once per device).
        let d0: Jid = "111:0@lid".parse().unwrap();
        let d5: Jid = "111:5@lid".parse().unwrap();
        let absent: Jid = "333:0@lid".parse().unwrap();
        c.mark_forgotten(group, [&d0, &d5, &absent].into_iter())
            .await;

        assert_eq!(map.device_has_key("111", 0), Some(false));
        assert_eq!(map.device_has_key("111", 5), Some(false));
        assert_eq!(map.device_has_key("222", 0), Some(true));
        assert_eq!(
            map.generation(),
            gen_before + 1,
            "a batch of flips bumps the generation exactly once"
        );
    }

    #[test]
    fn warm_gate_requires_both_device_and_primary() {
        // WA Web's ParticipantStore gate: a device is warm only when it AND its
        // primary (device 0) both hold the key. This is the per-device SKDM
        // targeting decision, so pin every branch.
        let m = SenderKeyDeviceMap::from_db_rows(&[
            ("111:0@lid".to_string(), true),  // primary warm
            ("111:5@lid".to_string(), true),  // secondary warm
            ("222:0@lid".to_string(), false), // primary cold
            ("222:7@lid".to_string(), true),  // secondary warm, primary cold
            ("333:9@lid".to_string(), true),  // secondary warm, no primary row
        ]);

        // Device and its primary both warm.
        assert!(m.device_and_primary_warm("111", 5));
        assert!(m.device_and_primary_warm("111", 0));
        // Secondary is warm but the primary is cold: the whole user is cold.
        assert!(!m.device_and_primary_warm("222", 7));
        // The primary itself when cold.
        assert!(!m.device_and_primary_warm("222", 0));
        // Secondary present but the primary row is absent: absent counts as cold.
        assert!(!m.device_and_primary_warm("333", 9));
        // A user the map never saw is cold.
        assert!(!m.device_and_primary_warm("999", 0));
    }

    #[test]
    fn from_db_rows_skips_malformed_and_keeps_valid() {
        // A corrupt or partially-migrated row must not poison the whole map: bad
        // JIDs are skipped (logged) and the valid devices still index correctly.
        let m = SenderKeyDeviceMap::from_db_rows(&[
            ("111:0@lid".to_string(), true),
            ("not-a-jid".to_string(), true), // unknown server → skipped
            ("111:xx@lid".to_string(), true), // non-numeric device → skipped
            ("222:0@lid".to_string(), false),
        ]);

        assert_eq!(m.device_has_key("111", 0), Some(true));
        assert_eq!(m.device_has_key("222", 0), Some(false));
        // The malformed rows produced no entries at all.
        assert_eq!(m.device_has_key("not-a-jid", 0), None);
        assert_eq!(m.device_has_key("111", 1), None);
    }

    #[tokio::test]
    async fn mark_forgotten_is_noop_on_cache_miss() {
        let c = cache();
        let dev: Jid = "111:0@lid".parse().unwrap();
        // No entry for this group: must not panic or create one.
        c.mark_forgotten("120363000000000009@g.us", std::iter::once(&dev))
            .await;
        let map = c
            .get_or_init("120363000000000009@g.us", async {
                Arc::new(SenderKeyDeviceMap::from_db_rows(&[]))
            })
            .await;
        assert!(map.is_empty());
    }
}
