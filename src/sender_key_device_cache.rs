//! In-memory cache for per-group sender key device tracking.
//! Avoids DB round-trips on group sends after the first.

use std::collections::HashMap;
use std::sync::Arc;

use crate::cache::Cache;
use crate::cache_config::CacheEntryConfig;
use wacore_binary::Jid;

/// Pre-parsed, pre-indexed sender key device map for one group.
#[derive(Clone, Debug)]
pub(crate) struct SenderKeyDeviceMap {
    /// user → (device_id → has_key)
    devices: HashMap<Arc<str>, HashMap<u16, bool>>,
}

impl SenderKeyDeviceMap {
    pub fn from_db_rows(rows: &[(String, bool)]) -> Self {
        let mut devices: HashMap<Arc<str>, HashMap<u16, bool>> = HashMap::with_capacity(rows.len());

        for (jid_str, has_key) in rows {
            match jid_str.parse::<Jid>() {
                Ok(jid) => {
                    let user: Arc<str> = Arc::from(jid.user.as_str());
                    devices
                        .entry(user)
                        .or_default()
                        .insert(jid.device, *has_key);
                }
                Err(e) => {
                    log::warn!("Skipping malformed device JID '{}': {}", jid_str, e);
                }
            }
        }

        Self { devices }
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.devices.is_empty()
    }

    /// Single (user, device) lookup. Retained for tests that cross-check the
    /// warm gate; production resolves both lookups via `device_and_primary_warm`.
    #[cfg(test)]
    pub fn device_has_key(&self, user: &str, device: u16) -> Option<bool> {
        self.devices.get(user)?.get(&device).copied()
    }

    /// WA Web warm gate (ParticipantStore.js): a device is warm only when it AND
    /// its primary (device 0) hold the key. Resolves the per-user inner map once
    /// so the two device lookups share a single outer (user-string) hash instead
    /// of re-hashing the user per call. A missing entry counts as cold (`?? false`).
    pub fn device_and_primary_warm(&self, user: &str, device: u16) -> bool {
        let Some(by_device) = self.devices.get(user) else {
            return false;
        };
        by_device.get(&device).copied().unwrap_or(false)
            && by_device.get(&0).copied().unwrap_or(false)
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
                    + v.devices.capacity() * std::mem::size_of::<(Arc<str>, HashMap<u16, bool>)>()
                    + v.devices
                        .iter()
                        .map(|(user, by_device)| {
                            user.len() + by_device.capacity() * std::mem::size_of::<(u16, bool)>()
                        })
                        .sum::<usize>()
            })
            .await
    }
}
