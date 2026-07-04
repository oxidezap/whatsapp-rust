//! User device list synchronization.
//!
//! Device list IQ specification is defined in `wacore::iq::usync`.

use crate::client::Client;
use log::{debug, warn};
use std::collections::HashSet;
use wacore::iq::usync::{DeviceListResponse, DeviceListSpec};
use wacore_binary::Jid;

impl Client {
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.usync.get_user_devices", level = "debug", skip_all, fields(users = jids.len()), err(Debug)))]
    pub(crate) async fn get_user_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        let mut jids_to_fetch: HashSet<Jid> = HashSet::with_capacity(jids.len());
        let mut all_devices = Vec::with_capacity(jids.len() * 2);

        // Resolve the LOCAL registry scan concurrently (the network usync below is
        // already one batched IQ) — a cold-cache large group would otherwise
        // serialize 256+ per-user cache/DB reads. Order is irrelevant (phash sorts,
        // encrypt fan-out is order-agnostic). A None result means an empty/corrupt
        // record, which falls through to the network below (WA Web always keeps
        // device 0). Materialize to an owned Vec first so the stream doesn't borrow
        // `jids` through buffer_unordered (Send bound).
        use futures::StreamExt;
        // Bounded fan-out over the independent per-user registry reads.
        const DEVICE_LIST_RESOLVE_CONCURRENCY: usize = 16;
        let non_ad: Vec<Jid> = jids.iter().map(|j| j.to_non_ad()).collect();
        let resolved: Vec<(Jid, Option<Vec<Jid>>)> = futures::stream::iter(non_ad)
            .map(|jid| async move {
                let devices = self.get_devices_from_registry(&jid).await;
                (jid, devices)
            })
            .buffer_unordered(DEVICE_LIST_RESOLVE_CONCURRENCY)
            .collect()
            .await;

        for (jid, devices) in resolved {
            match devices {
                Some(devices) => all_devices.extend(devices),
                None => {
                    jids_to_fetch.insert(jid);
                }
            }
        }

        if !jids_to_fetch.is_empty() {
            debug!(
                "get_user_devices: Cache miss, fetching from network for {} unique users",
                jids_to_fetch.len()
            );

            let sid = self.generate_request_id();
            let jids_vec: Vec<Jid> = jids_to_fetch.into_iter().collect();
            let spec = DeviceListSpec::new(jids_vec, sid);

            let response = self.execute(spec).await?;

            let fetched_devices = self.process_device_list_response(&response).await;
            all_devices.extend(fetched_devices);
        }

        Ok(all_devices)
    }

    /// Apply a usync device-list response to the registry: persist LID mappings,
    /// rebuild each returned user's `DeviceListRecord` (preserving key indices and
    /// handling raw_id identity changes), and batch-write them. Returns the
    /// resolved device JIDs for the users present in the response.
    ///
    /// Users the server OMITS — unchanged ones, when we sent a `device_hash` — are
    /// simply absent here, so their cached records are left untouched (the
    /// merge-safe behavior the `device_hash` optimization depends on).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.usync.process_device_list", level = "debug", skip_all, fields(users = response.device_lists.len())))]
    async fn process_device_list_response(&self, response: &DeviceListResponse) -> Vec<Jid> {
        // Learn LID↔PN mappings via the same batched, guarded learner query_info
        // uses (one detached transaction, skipping already-durable pairs), so
        // per-mapping DB writes stay off the send's critical path. Falls back to
        // the per-mapping path if the owning Arc<Client> isn't available.
        //
        // Ordering: the old per-mapping path AWAITED migrate_signal_sessions_on_lid_discovery.
        // Detaching it can let a standalone usync (sync_own_device_list /
        // flush_pending_device_sync) that is the FIRST learner of a LID for a
        // contact with prior PN Signal state encrypt before the PN-wins migration
        // runs — but the per-address session_lock_for both take is the real
        // barrier (they can't interleave). The group-send path is unchanged:
        // query_info already learns these same pairs detached upstream.
        if !response.lid_mappings.is_empty() {
            if let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) {
                let mappings: Vec<(String, String)> = response
                    .lid_mappings
                    .iter()
                    .map(|m| (m.lid.to_string(), m.phone_number.to_string()))
                    .collect();
                client
                    .learn_lid_pn_mappings_batch(
                        mappings,
                        crate::lid_pn_cache::LearningSource::Usync,
                        false,
                    )
                    .await;
            } else {
                for mapping in &response.lid_mappings {
                    if let Err(err) = self
                        .add_lid_pn_mapping(
                            &mapping.lid,
                            &mapping.phone_number,
                            crate::lid_pn_cache::LearningSource::Usync,
                        )
                        .await
                    {
                        warn!(
                            "Failed to persist LID {} -> {} from usync: {err}",
                            mapping.lid, mapping.phone_number,
                        );
                    }
                }
            }
        }

        let mut fetched_devices = Vec::with_capacity(response.device_lists.len());
        let mut device_records: Vec<wacore::store::traits::DeviceListRecord> =
            Vec::with_capacity(response.device_lists.len());

        for user_list in &response.device_lists {
            // Update device registry (single source of truth for device lists).
            // Preserve key_index values from existing records (set via account_sync)
            // Use alias-aware lookup (resolves LID ↔ PN) to find
            // existing record regardless of which key it was stored under
            let existing_record = self.load_device_record(&user_list.user.user).await;

            let mut existing_key_indices: std::collections::HashMap<u32, Option<u32>> =
                existing_record
                    .as_ref()
                    .map(|r| {
                        r.devices
                            .iter()
                            .map(|d| (d.device_id, d.key_index))
                            .collect()
                    })
                    .unwrap_or_default();

            // Decode key-index-list if present (WA Web: handleKeyIndexResult)
            let decoded_key_index = user_list
                .key_index_bytes
                .as_deref()
                .and_then(wacore::adv::decode_key_index_list);

            // Check raw_id mismatch for identity change detection
            // TODO: also check advAccountType mismatch (see patch_device_add TODO)
            let mut raw_id = decoded_key_index.as_ref().map(|d| d.raw_id);
            if let Some(ref decoded) = decoded_key_index
                && let Some(ref existing) = existing_record
                && let Some(stored_raw_id) = existing.raw_id
                && stored_raw_id != decoded.raw_id
            {
                log::info!(
                    "raw_id mismatch for user {} in usync: stored={stored_raw_id}, received={}. Clearing record.",
                    user_list.user.user,
                    decoded.raw_id
                );
                self.clear_device_record(
                    &user_list.user.user,
                    user_list.user.server.as_str(),
                    existing,
                )
                .await;
                // Old key indices are from the previous identity — don't reuse
                existing_key_indices.clear();
            }

            // Preserve raw_id from existing when usync didn't provide one
            // (no key-index-list) and no mismatch cleared the indices.
            // existing_key_indices is empty after a mismatch clear, so this
            // correctly skips preservation after identity change.
            if raw_id.is_none() && !existing_key_indices.is_empty() {
                raw_id = existing_record.as_ref().and_then(|r| r.raw_id);
            }

            let mut devices: Vec<wacore::store::traits::DeviceInfo> = user_list
                .devices
                .iter()
                .map(|d| wacore::store::traits::DeviceInfo {
                    device_id: d.device as u32,
                    // Server-returned key_index takes priority over cached
                    key_index: d.key_index.or_else(|| {
                        existing_key_indices
                            .get(&(d.device as u32))
                            .copied()
                            .flatten()
                    }),
                })
                .collect();

            // Apply valid_indexes filtering if key-index-list was decoded
            if let Some(ref decoded) = decoded_key_index {
                devices = wacore::adv::filter_devices_by_key_index(&devices, decoded);
            }

            // Convert filtered DeviceInfo list back to JIDs for return
            let user_jid = &user_list.user;
            for d in &devices {
                let mut jid = user_jid.clone();
                jid.device = d.device_id as u16;
                fetched_devices.push(jid);
            }

            // An empty device list is never valid — WA Web always keeps the primary
            // (device 0), so a usync returning no devices for a user is transient or
            // corrupt. Persisting it would clobber a good cached record, or store an
            // empty one that get_user_devices then re-fetches on every send.
            if devices.is_empty() {
                continue;
            }

            device_records.push(wacore::store::traits::DeviceListRecord {
                user: user_list.user.user.to_string(),
                devices,
                timestamp: wacore::time::now_secs(),
                phash: user_list.phash.clone(),
                raw_id,
            });
        }

        // One batched backend write for the whole usync response — for
        // large groups this collapses N spawn_blocking SQLite hops into
        // a single transaction, which dominated the per-send wall-clock.
        if let Err(e) = self.update_device_lists(device_records).await {
            warn!("Failed to update device registry batch: {e}");
        }

        fetched_devices
    }

    /// Re-sync own device list from the server. Mirrors WA Web `syncMyDeviceList`:
    /// sends the cached per-user `device_hash` so the server answers "unchanged"
    /// (by omitting the user) instead of returning the full list on every reconnect.
    /// On a changed list the server returns it and we update; omitted users keep
    /// their cache.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.usync.sync_own_device_list",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn sync_own_device_list(&self) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();

        let mut jids = Vec::with_capacity(2);
        let mut hashes: std::collections::HashMap<Jid, (String, i64)> =
            std::collections::HashMap::new();
        for own in device_snapshot.pn.iter().chain(device_snapshot.lid.iter()) {
            let bare = own.to_non_ad();
            // Carry the cached device_hash so an unchanged list is skipped server-side.
            if let Some(record) = self.load_device_record(&bare.user).await
                && let Some(phash) = record.phash
            {
                hashes.insert(bare.clone(), (phash, record.timestamp));
            }
            jids.push(bare);
        }

        if jids.is_empty() {
            return Ok(());
        }

        let sid = self.generate_request_id();
        let spec = DeviceListSpec::with_hashes(jids, sid, hashes);
        let response = self.execute(spec).await?;
        // `process_device_list_response` only touches users the server actually
        // returned, so unchanged (omitted) own devices keep their cache.
        let devices = self.process_device_list_response(&response).await;
        log::info!(
            "Re-synced own device list: {} device(s) updated",
            devices.len()
        );
        Ok(())
    }

    /// WA Web: `doPendingDeviceSync()` — flush batched unknown-device users.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.usync.flush_pending_device_sync",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn flush_pending_device_sync(&self) {
        let pending = self.pending_device_sync.take_all().await;
        if pending.is_empty() {
            return;
        }

        debug!("Flushing pending device sync for {} users", pending.len());

        // Invalidate stale records so get_user_devices hits the network
        for jid in &pending {
            self.invalidate_device_cache(&jid.user).await;
        }

        match self.get_user_devices(&pending).await {
            Ok(devices) => {
                debug!(
                    "Pending device sync completed: {} devices across {} users",
                    devices.len(),
                    pending.len()
                );
            }
            Err(e) => {
                warn!(
                    "Pending device sync failed, re-enqueueing {} users: {e:?}",
                    pending.len()
                );
                for jid in &pending {
                    self.pending_device_sync.add(jid).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_client;
    use wacore::store::traits::{DeviceInfo, DeviceListRecord};

    #[tokio::test]
    async fn test_device_registry_hit_resolves_devices() {
        let client = create_test_client().await;

        let user_jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();

        // Insert a device record into the registry (simulates prior usync/notification)
        let record = DeviceListRecord {
            user: "1234567890".into(),
            devices: vec![
                DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                DeviceInfo {
                    device_id: 3,
                    key_index: Some(10),
                },
            ],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        // get_user_devices should resolve from registry without network
        let devices = client.get_user_devices(&[user_jid]).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|d| d.device == 0));
        assert!(devices.iter().any(|d| d.device == 3));
        assert!(devices.iter().all(|d| d.is_pn()));
    }

    #[tokio::test]
    async fn test_device_registry_hit_for_lid_jid() {
        let client = create_test_client().await;

        let lid_jid: Jid = "100000012345678@lid".parse().unwrap();

        let record = DeviceListRecord {
            user: "100000012345678".into(),
            devices: vec![
                DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                DeviceInfo {
                    device_id: 39,
                    key_index: Some(25),
                },
            ],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        let devices = client.get_user_devices(&[lid_jid]).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|d| d.device == 0));
        assert!(devices.iter().any(|d| d.device == 39));
        assert!(devices.iter().all(|d| d.is_lid()));
    }

    #[tokio::test]
    async fn test_device_registry_db_fallback() {
        let client = create_test_client().await;

        let user_jid: Jid = "9876543210@s.whatsapp.net".parse().unwrap();

        // Insert into backend DB via update_device_list
        let record = DeviceListRecord {
            user: "9876543210".into(),
            devices: vec![DeviceInfo {
                device_id: 5,
                key_index: None,
            }],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        // Evict from registry cache to force DB path
        client.device_registry_cache.invalidate("9876543210").await;
        client.device_registry_cache.run_pending_tasks().await;

        // Should still resolve from DB
        let devices = client.get_user_devices(&[user_jid]).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device, 5);
    }

    // A present-but-empty device record is local corruption and must not be
    // treated as authoritative: get_user_devices falls through to the network so
    // the list can self-heal. Offline, that surfaces as an error, which proves the
    // fetch was attempted (the old behavior returned Ok([]) with no fetch).
    #[tokio::test]
    async fn test_empty_device_record_falls_through_to_network() {
        let client = create_test_client().await;

        let user_jid: Jid = "5551230000@s.whatsapp.net".parse().unwrap();

        let record = DeviceListRecord {
            user: "5551230000".into(),
            devices: vec![],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        let result = client.get_user_devices(&[user_jid]).await;
        assert!(
            result.is_err(),
            "empty record must fall through to the network, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_cache_size_eviction() {
        use crate::cache::Cache;

        let cache: Cache<i32, String> = Cache::builder().max_capacity(2).build();

        cache.insert(1, "one".to_string()).await;
        cache.insert(2, "two".to_string()).await;
        cache.insert(3, "three".to_string()).await;

        cache.run_pending_tasks().await;

        let count = cache.entry_count();
        assert!(
            count <= 2,
            "Cache should have at most 2 items, has {}",
            count
        );
    }

    /// #3 merge-safety: when the server omits an unchanged user (the `device_hash`
    /// skip), `process_device_list_response` must update only the returned users
    /// and leave the omitted user's cached devices untouched.
    #[tokio::test]
    async fn process_response_preserves_omitted_users() {
        use wacore::iq::usync::DeviceListResponse;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;

        // Seed user B in the registry (will be the "unchanged/omitted" one).
        client
            .update_device_list(DeviceListRecord {
                user: "2222222222".into(),
                devices: vec![
                    DeviceInfo {
                        device_id: 0,
                        key_index: None,
                    },
                    DeviceInfo {
                        device_id: 7,
                        key_index: None,
                    },
                ],
                timestamp: wacore::time::now_secs(),
                phash: Some("2:oldB".to_string()),
                raw_id: None,
            })
            .await
            .unwrap();

        // Response only contains user A — B is omitted (unchanged).
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: "1111111111@s.whatsapp.net".parse().unwrap(),
                devices: vec![UsyncDevice {
                    device: 0,
                    key_index: None,
                }],
                phash: Some("2:a".to_string()),
                key_index_bytes: None,
            }],
            lid_mappings: vec![],
        };

        let fetched = client.process_device_list_response(&response).await;
        assert!(
            fetched.iter().any(|j| j.user == "1111111111"),
            "returned user A must be resolved"
        );

        // B's cache is preserved (still 2 devices) — not wiped by the omission.
        let b_jid: Jid = "2222222222@s.whatsapp.net".parse().unwrap();
        let b_devices = client
            .get_devices_from_registry(&b_jid)
            .await
            .expect("omitted user B must keep its cached record");
        assert_eq!(
            b_devices.len(),
            2,
            "omitted user's devices must be preserved"
        );
    }

    /// A usync that returns an empty device list for a user is transient or
    /// corrupt (WA Web always keeps device 0). `process_device_list_response`
    /// must not persist it: a good cached record stays intact instead of being
    /// clobbered with an empty list that `get_user_devices` then re-fetches on
    /// every send.
    #[tokio::test]
    async fn process_response_skips_empty_device_list() {
        use wacore::usync::UserDeviceList;

        let client = create_test_client().await;

        client
            .update_device_list(DeviceListRecord {
                user: "3333333333".into(),
                devices: vec![
                    DeviceInfo {
                        device_id: 0,
                        key_index: None,
                    },
                    DeviceInfo {
                        device_id: 4,
                        key_index: None,
                    },
                ],
                timestamp: wacore::time::now_secs(),
                phash: Some("3:old".to_string()),
                raw_id: None,
            })
            .await
            .unwrap();

        // The same user comes back from usync with no devices.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: "3333333333@s.whatsapp.net".parse().unwrap(),
                devices: vec![],
                phash: Some("3:empty".to_string()),
                key_index_bytes: None,
            }],
            lid_mappings: vec![],
        };

        let fetched = client.process_device_list_response(&response).await;
        assert!(
            !fetched.iter().any(|j| j.user == "3333333333"),
            "an empty returned list contributes no devices"
        );

        // The good cached record survives — not clobbered with an empty list.
        let jid: Jid = "3333333333@s.whatsapp.net".parse().unwrap();
        let devices = client
            .get_devices_from_registry(&jid)
            .await
            .expect("the good record must survive an empty usync response");
        assert_eq!(
            devices.len(),
            2,
            "empty response must not clobber the record"
        );
    }

    /// The batched LID-PN learn path warms the in-memory cache SYNCHRONOUSLY
    /// (the persist runs detached), so a mapping from the usync response is
    /// resolvable the moment `process_device_list_response` returns. Locks the
    /// contract that the new path doesn't defer the cache update.
    #[tokio::test]
    async fn process_response_warms_lid_pn_cache_synchronously() {
        use wacore::usync::UsyncLidMapping;

        let client = create_test_client().await;

        // Pin that we exercise the BATCHED branch, not the per-mapping fallback:
        // the branch is `if let Some(client) = self.self_weak...upgrade()`, so a
        // live self_weak upgrade means learn_lid_pn_mappings_batch is the path
        // taken. (Both paths warm the cache, so without this the test could pass
        // via the fallback.)
        assert!(
            client.self_weak.get().and_then(|w| w.upgrade()).is_some(),
            "fixture must populate self_weak so the batched learner is exercised"
        );

        let response = DeviceListResponse {
            device_lists: vec![],
            lid_mappings: vec![UsyncLidMapping {
                phone_number: "559980000123".into(),
                lid: "100000000000123".into(),
            }],
        };

        assert!(
            client
                .lid_pn_cache
                .get_current_lid("559980000123")
                .await
                .is_none()
        );

        let _ = client.process_device_list_response(&response).await;

        assert_eq!(
            client
                .lid_pn_cache
                .get_current_lid("559980000123")
                .await
                .as_deref(),
            Some("100000000000123"),
            "usync LID mapping must be in the cache synchronously after the call"
        );
    }
}
