//! In-memory cache for per-group sender key device tracking.
//! Avoids DB round-trips on group sends after the first.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use portable_atomic::AtomicU64;

use crate::cache::Cache;
use crate::cache_config::CacheEntryConfig;
use wacore::stats::hash_table_bytes;
use wacore_binary::Jid;

/// One tracked device of a user: which device it is, and whether it holds the
/// group's sender key.
///
/// `has_key` is an [`AtomicBool`] so `markForgetSenderKey` can flip one device
/// cold in place, matching WA Web's per-device participant-record update,
/// instead of invalidating the whole group and forcing the next send to re-read
/// and re-parse every row from the DB.
#[derive(Debug)]
struct DeviceWarmState {
    device_id: u16,
    has_key: AtomicBool,
}

/// Pre-parsed, pre-indexed sender key device map for one group.
///
/// A user's devices are a compact slice, not a nested `HashMap`. WA gives a
/// user a handful of devices, so a per-user table spends more on buckets than
/// it holds and a scan over three or four 4-byte entries beats hashing a `u16`;
/// the slice also stays sorted by device id, which puts the primary — the one
/// device every warm check has to consult — first.
///
/// An in-place flip keeps the same `Arc`, so `generation` is the version stamp
/// the `skdm_warm_memo` compares to notice the change (pointer identity alone
/// cannot).
#[derive(Debug)]
pub(crate) struct SenderKeyDeviceMap {
    /// user → its devices, sorted by device id.
    devices: HashMap<Arc<str>, Box<[DeviceWarmState]>>,
    /// Bumped on every in-place warm-state change. Same freshness contract the
    /// device-registry generation gives membership.
    generation: AtomicU64,
}

/// The two reference counts an `Arc` allocation carries ahead of its payload.
/// A user key is one `Arc<str>` per user, so leaving this out understated a
/// 1024-user map by 16 KiB — and a report that understates what grows is the
/// one thing `HeapSize` says these figures must not do.
const ARC_HEADER: usize = 2 * size_of::<usize>();

/// The state of `device_id` within one user's devices.
///
/// A linear scan, not a binary search: a user has a handful of devices, and at
/// that size the branch-free walk over 4-byte entries wins outright — the sort
/// is there for the primary shortcut below, not for this.
fn device_state(states: &[DeviceWarmState], device_id: u16) -> Option<&DeviceWarmState> {
    states.iter().find(|state| state.device_id == device_id)
}

/// Order a user's devices by id. Insertion sort because the slice is a handful
/// of entries built once per group load; a generic `sort_unstable_by` here
/// would instantiate pdqsort for a new element type, which #1353 measured at
/// 15.6 KiB of `.text`.
fn sort_by_device_id(states: &mut [DeviceWarmState]) {
    for i in 1..states.len() {
        let mut j = i;
        while j > 0 && states[j - 1].device_id > states[j].device_id {
            states.swap(j - 1, j);
            j -= 1;
        }
    }
}

impl SenderKeyDeviceMap {
    pub fn from_db_rows(rows: &[(String, bool)]) -> Self {
        // Deliberately unsized: `rows` counts devices, not users, so reserving
        // by it over-allocates the outer table by however many devices a user
        // averages — three, in a group whose members carry companions.
        let mut by_user: HashMap<Arc<str>, Vec<DeviceWarmState>> = HashMap::new();

        for (jid_str, has_key) in rows {
            match jid_str.parse::<Jid>() {
                Ok(jid) => {
                    // `Arc<str>: Borrow<str>`, so the repeat rows of a user
                    // that already has an entry cost a lookup instead of a
                    // fresh `Arc` allocation per device.
                    match by_user.get_mut(jid.user.as_str()) {
                        Some(states) => match device_state(states, jid.device) {
                            // Two rows can collapse onto one (user, device):
                            // this indexes the PARSED jid's user and device,
                            // so rows differing only in server — the same
                            // member left behind under both `@s.whatsapp.net`
                            // and `@lid` by an addressing-mode migration —
                            // land on the same slot. Conflicting
                            // values resolve to cold, never to warm — the
                            // whole map already reads a missing entry as cold
                            // because redistributing a sender key nobody
                            // needed is free, while skipping one a device did
                            // need leaves that device unable to read the
                            // message. Order-independent, so it cannot matter
                            // which row the backend happened to return first.
                            Some(existing) => {
                                if !*has_key {
                                    existing.has_key.store(false, Ordering::Relaxed);
                                }
                            }
                            None => states.push(DeviceWarmState {
                                device_id: jid.device,
                                has_key: AtomicBool::new(*has_key),
                            }),
                        },
                        None => {
                            by_user.insert(
                                Arc::from(jid.user.as_str()),
                                vec![DeviceWarmState {
                                    device_id: jid.device,
                                    has_key: AtomicBool::new(*has_key),
                                }],
                            );
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Skipping malformed device JID '{}': {}", jid_str, e);
                }
            }
        }

        let devices = by_user
            .into_iter()
            .map(|(user, mut states)| {
                sort_by_device_id(&mut states);
                (user, states.into_boxed_slice())
            })
            .collect();

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
            device_state(self.devices.get(user)?, device)?
                .has_key
                .load(Ordering::Relaxed),
        )
    }

    /// WA Web warm gate (ParticipantStore.js): a device is warm only when it AND
    /// its primary (device 0) hold the key. Resolves the per-user inner map once
    /// so the two device lookups share a single outer (user-string) hash instead
    /// of re-hashing the user per call. A missing entry counts as cold.
    pub fn device_and_primary_warm(&self, user: &str, device: u16) -> bool {
        let Some(states) = self.devices.get(user) else {
            return false;
        };
        // Sorted by device id, so the primary is the first entry when it is
        // present at all — the check every warm device pays is a single load.
        let primary_warm = states
            .first()
            .is_some_and(|state| state.device_id == 0 && state.has_key.load(Ordering::Relaxed));
        primary_warm
            && (device == 0
                || device_state(states, device)
                    .is_some_and(|state| state.has_key.load(Ordering::Relaxed)))
    }

    /// Bytes this map retains beyond its own struct: the tables it owns plus
    /// the user strings it keys on.
    ///
    /// One definition, shared by the cache's `memory_stats` and by the test
    /// that pins the per-device bound — a second copy would let the report and
    /// the bound drift apart, and the bound is the only thing standing between
    /// a layout change and a silent regression.
    pub(crate) fn retained_bytes(&self) -> usize {
        // The one table left goes through `hash_table_bytes`, which accounts
        // for the buckets hashbrown really owns rather than the entries that
        // fit in them; the per-user slices are exact, so they are summed by
        // iteration.
        hash_table_bytes(
            self.devices.capacity(),
            size_of::<(Arc<str>, Box<[DeviceWarmState]>)>(),
        ) + self
            .devices
            .iter()
            .map(|(user, states)| {
                ARC_HEADER + user.len() + states.len() * size_of::<DeviceWarmState>()
            })
            .sum::<usize>()
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
        F: Future<Output = Arc<SenderKeyDeviceMap>> + wacore::sync_marker::MaybeSend,
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
            if let Some(states) = map.devices.get(jid.user.as_str())
                && let Some(state) = device_state(states, jid.device)
                && state.has_key.swap(false, Ordering::Relaxed)
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
                    .and_then(|states| device_state(states, device_id))
                    .map(|_| group_jid.as_ref().clone())
            })
            .collect();
        for g in to_drop {
            self.inner.invalidate(&g).await;
        }
    }

    /// Approximate entry count plus estimated retained bytes.
    pub(crate) async fn memory_stats(&self) -> wacore::stats::CollectionStats {
        self.inner
            .memory_stats(|k, v| k.capacity() + v.retained_bytes())
            .await
    }

    /// Sweep maps idle past their TTI. Driven by
    /// [`Client::run_cache_maintenance`].
    ///
    /// [`Client::run_cache_maintenance`]: crate::client::Client::run_cache_maintenance
    pub(crate) async fn run_pending_tasks(&self) {
        self.inner.run_pending_tasks().await;
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

    /// The warm gate reads the primary off the front of the slice, so the sort
    /// is load-bearing: rows arrive in whatever order the DB hands them, and an
    /// unsorted slice would report a user with a warm primary as cold and
    /// redistribute their sender key on every single send.
    /// The index keys on the parsed jid's user and device, so rows differing
    /// only in server collapse onto one slot — the shape an addressing-mode
    /// migration leaves behind, with the same member stored under both
    /// `@s.whatsapp.net` and `@lid`. Whichever order they arrive in, the
    /// collapse must land cold: a spurious redistribution costs one SKDM,
    /// while a spurious warm reading skips a distribution the device needed
    /// and leaves it unable to read the message.
    #[test]
    fn conflicting_duplicate_rows_collapse_to_cold() {
        for rows in [
            [
                ("111:0@lid".to_string(), true),
                ("111:0@s.whatsapp.net".to_string(), false),
            ],
            [
                ("111:0@lid".to_string(), false),
                ("111:0@s.whatsapp.net".to_string(), true),
            ],
        ] {
            let m = SenderKeyDeviceMap::from_db_rows(&rows);
            assert_eq!(
                m.device_has_key("111", 0),
                Some(false),
                "conflicting rows {rows:?} must read cold"
            );
            assert!(!m.device_and_primary_warm("111", 0));
        }

        // Agreeing duplicates keep their value — the collapse must not turn a
        // genuinely warm device cold and redistribute on every send.
        let m = SenderKeyDeviceMap::from_db_rows(&[
            ("111:0@lid".to_string(), true),
            ("111:0@s.whatsapp.net".to_string(), true),
        ]);
        assert_eq!(m.device_has_key("111", 0), Some(true));
        assert!(m.device_and_primary_warm("111", 0));
    }

    #[test]
    fn devices_are_ordered_whatever_order_the_rows_arrive_in() {
        let m = SenderKeyDeviceMap::from_db_rows(&[
            ("111:9@lid".to_string(), true),
            ("111:5@lid".to_string(), true),
            ("111:0@lid".to_string(), true),
            ("222:3@lid".to_string(), true),
            ("222:0@lid".to_string(), false),
        ]);

        assert!(m.device_and_primary_warm("111", 9));
        assert!(m.device_and_primary_warm("111", 5));
        assert!(m.device_and_primary_warm("111", 0));
        // Primary cold, and it is not the first row of its user either.
        assert!(!m.device_and_primary_warm("222", 3));
        assert!(!m.device_and_primary_warm("222", 0));
        // Every device is still individually addressable after the reorder.
        assert_eq!(m.device_has_key("111", 9), Some(true));
        assert_eq!(m.device_has_key("222", 3), Some(true));
        assert_eq!(m.device_has_key("222", 0), Some(false));
    }

    /// A user whose primary row is missing entirely: the shortcut must read
    /// that as cold rather than mistaking the lowest device it does have for
    /// the primary.
    #[test]
    fn a_user_without_a_primary_row_is_cold() {
        let m = SenderKeyDeviceMap::from_db_rows(&[
            ("111:5@lid".to_string(), true),
            ("111:9@lid".to_string(), true),
        ]);

        assert!(!m.device_and_primary_warm("111", 5));
        assert!(!m.device_and_primary_warm("111", 9));
        assert_eq!(m.device_has_key("111", 5), Some(true));
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

    /// A group's sender-key map stays resident for as long as the group is
    /// warm, and there is one per group, so its cost per tracked device is a
    /// bound rather than a comment. Uses the same 1024x3 shape as the device
    /// memo test in `device_registry`, since the two sit side by side behind
    /// every group send.
    #[test]
    fn retained_bytes_per_device_stay_bounded() {
        const USERS: usize = 1024;
        const DEVICES_PER_USER: usize = 3;

        let mut rows = Vec::with_capacity(USERS * DEVICES_PER_USER);
        for i in 0..USERS {
            for device in 0..DEVICES_PER_USER {
                rows.push((format!("1000000{i:08}:{device}@lid"), true));
            }
        }
        let map = SenderKeyDeviceMap::from_db_rows(&rows);

        let per_device =
            (size_of::<SenderKeyDeviceMap>() + map.retained_bytes()) / (USERS * DEVICES_PER_USER);
        assert!(
            per_device <= 36,
            "a warm 1024-user sender-key map must stay within 36 B per device, got {per_device}"
        );
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
