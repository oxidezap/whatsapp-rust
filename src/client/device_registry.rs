//! Device Registry methods for Client.
//!
//! Manages the device registry cache for tracking known devices per user.
//! Uses LID-first storage with bidirectional lookup support.

use anyhow::Result;
use log::{debug, info, warn};
use std::sync::Arc;
use wacore_binary::Jid;

use super::Client;

/// Per-group device-list snapshot for `resolve_group_devices_memoized`.
/// Valid while the producing `GroupInfo` Arc is still the cached one AND the
/// device-topology generation is unchanged.
pub(crate) struct GroupDevicesMemo {
    /// Weak identity of the producing GroupInfo: pointer equality is ABA-safe
    /// because the Weak keeps the allocation alive, while the heavy data
    /// (participants, maps) is freed as soon as the metadata cache drops its
    /// Arc — the memo retains a struct-sized header, not the whole GroupInfo.
    pub(crate) group_info: std::sync::Weak<wacore::client::context::GroupInfo>,
    pub(crate) generation: u64,
    /// Member identifiers in BOTH namespaces (participant users, their mapped
    /// counterparts, resolved device users): the scoped-invalidation check
    /// tests the topology log's touched users against this set.
    pub(crate) members: Arc<std::collections::HashSet<wacore_binary::CompactString>>,
    pub(crate) devices: Arc<wacore::send::ResolvedGroupDevices>,
}

impl wacore::stats::HeapSize for GroupDevicesMemo {
    fn heap_bytes(&self) -> usize {
        // The Weak keeps only the GroupInfo allocation header alive; the memo
        // does not retain its payload.
        self.members.iter().map(|m| m.heap_bytes()).sum::<usize>()
            + self.members.capacity() * std::mem::size_of::<wacore_binary::CompactString>()
            + self.devices.heap_bytes()
    }
}

/// Result of resolving a user identifier to lookup keys.
/// This makes the LID/PN relationship explicit instead of using magic indices.
#[derive(Debug, Clone)]
enum UserLookupKeys {
    /// User is a LID with known phone number mapping.
    /// Keys: [LID, PN]
    LidWithPn {
        lid: wacore_binary::CompactString,
        pn: wacore_binary::CompactString,
    },
    /// User is a phone number with known LID mapping.
    /// Keys: [LID, PN]
    PnWithLid {
        lid: wacore_binary::CompactString,
        pn: wacore_binary::CompactString,
    },
    /// Unknown user - no LID-PN mapping exists.
    /// Could be either a LID or PN, we don't know.
    Unknown { user: wacore_binary::CompactString },
}

impl UserLookupKeys {
    /// Returns all keys to try for lookups, in preference order.
    fn all_keys(&self) -> Vec<&str> {
        match self {
            Self::LidWithPn { lid, pn } | Self::PnWithLid { lid, pn } => vec![lid, pn],
            Self::Unknown { user } => vec![user],
        }
    }

    /// Returns the canonical (preferred) key for storage.
    fn canonical_key(&self) -> &str {
        match self {
            Self::LidWithPn { lid, .. } | Self::PnWithLid { lid, .. } => lid,
            Self::Unknown { user } => user,
        }
    }
}

impl Client {
    /// Resolve a user identifier to its canonical storage key (LID preferred).
    ///
    /// This is a convenience wrapper around `resolve_lookup_keys().canonical_key()`.
    #[cfg(test)]
    pub(crate) async fn resolve_to_canonical_key(&self, user: &str) -> String {
        self.resolve_lookup_keys(user)
            .await
            .canonical_key()
            .to_string()
    }

    /// Resolve a group's full (LID-converted) device list, memoized per group.
    ///
    /// The input set is a pure function of `group_info` (participants + LID
    /// normalization), so the memo is valid exactly while BOTH hold:
    /// the same `GroupInfo` snapshot (`Arc` identity — any metadata refresh or
    /// membership change produces a new `Arc`) and an unchanged
    /// `device_topology_generation` (any registry/mapping write bumps it).
    /// On a warm repeat send this turns the per-member cache fan-out
    /// (2 lookups per participant) into one memo hit.
    pub(crate) async fn resolve_group_devices_memoized(
        &self,
        group: &Jid,
        group_info: &Arc<wacore::client::context::GroupInfo>,
        own_sending_jid: &Jid,
    ) -> Result<Arc<wacore::send::ResolvedGroupDevices>, anyhow::Error> {
        // Store-backed registry or mapping caches can be written by OTHER
        // processes (e.g. shared Redis across pods), which this process's
        // topology tracker cannot observe; the memo's freshness contract
        // doesn't hold there, so it is disabled and every send resolves.
        if !self.group_devices_memo_enabled {
            return Ok(Arc::new(wacore::send::ResolvedGroupDevices::new(
                self.resolve_group_devices_uncached(group_info, own_sending_jid)
                    .await?,
            )));
        }
        // Load the generation BEFORE resolving (do NOT move this after
        // get_user_devices): a write racing the resolve bumps it afterwards,
        // so the memo we store is already stale by its own stamp and the next
        // read revalidates. Loading after would stamp racing writes as seen
        // and serve their effects stale.
        let generation = self.device_topology.current();

        if let Some(memo) = self.group_devices_memo.get(group).await
            && std::ptr::eq(memo.group_info.as_ptr(), Arc::as_ptr(group_info))
        {
            if memo.generation == generation {
                // Refcount bump: the snapshot is immutable, so a hit shares
                // it instead of cloning the device Vec.
                return Ok(Arc::clone(&memo.devices));
            }
            // Stale stamp: when every change since it touched only users
            // outside this group, re-stamp instead of recomputing, so write
            // storms on unrelated groups don't tank the hit rate. Any doubt
            // (log overflow, member touched) falls through to the recompute.
            if self
                .device_topology
                .unchanged_for(memo.generation, |user| memo.members.contains(user))
            {
                self.group_devices_memo
                    .insert(
                        group.clone(),
                        Arc::new(GroupDevicesMemo {
                            group_info: memo.group_info.clone(),
                            generation,
                            members: Arc::clone(&memo.members),
                            devices: Arc::clone(&memo.devices),
                        }),
                    )
                    .await;
                return Ok(Arc::clone(&memo.devices));
            }
        }

        let devices = self
            .resolve_group_devices_uncached(group_info, own_sending_jid)
            .await?;

        // Member identifiers in both namespaces, so the scoped-invalidation
        // check can match however a write was keyed: writes record every
        // resolved lookup alias (see DeviceRegistryCache::insert callers), and
        // this set carries each member's group-facing identity (participant
        // user + mapped counterpart) plus the namespace the resolved device
        // JIDs ended up in.
        let mut members = std::collections::HashSet::with_capacity(
            group_info.participants.len() * 2 + devices.len() + 2,
        );
        for participant in &group_info.participants {
            members.insert(participant.user.clone());
            if participant.is_lid()
                && let Some(pn) = group_info.phone_jid_for_lid_user(&participant.user)
            {
                members.insert(pn.user.clone());
            } else if let Some(lid) = group_info.lid_user_for_phone_user(&participant.user) {
                members.insert(lid.clone());
            }
        }
        members.insert(own_sending_jid.user.clone());
        for device in &devices {
            members.insert(device.user.clone());
        }

        let devices = Arc::new(wacore::send::ResolvedGroupDevices::new(devices));
        self.group_devices_memo
            .insert(
                group.clone(),
                Arc::new(GroupDevicesMemo {
                    group_info: Arc::downgrade(group_info),
                    generation,
                    members: Arc::new(members),
                    devices: Arc::clone(&devices),
                }),
            )
            .await;
        Ok(devices)
    }

    /// The memo's recompute body: derive the resolve set from `group_info`
    /// (participants + LID normalization, appending self when the server
    /// snapshot omitted it — mirroring `ensure_self_in_group`, so keying the
    /// memo off the pre-ensure Arc stays equivalent) and resolve it.
    async fn resolve_group_devices_uncached(
        &self,
        group_info: &Arc<wacore::client::context::GroupInfo>,
        own_sending_jid: &Jid,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        let is_lid_mode = group_info.addressing_mode == wacore::types::message::AddressingMode::Lid;
        let mut jids_to_resolve: Vec<Jid> = group_info
            .participants
            .iter()
            .map(|jid| {
                if is_lid_mode
                    && jid.is_lid()
                    && let Some(pn) = group_info.phone_jid_for_lid_user(&jid.user)
                {
                    return pn.to_non_ad();
                }
                jid.to_non_ad()
            })
            .collect();
        if !group_info
            .participants
            .iter()
            .any(|participant| wacore_binary::JidExt::is_same_user_as(participant, own_sending_jid))
        {
            let own = if is_lid_mode
                && own_sending_jid.is_lid()
                && let Some(pn) = group_info.phone_jid_for_lid_user(&own_sending_jid.user)
            {
                pn.to_non_ad()
            } else {
                own_sending_jid.to_non_ad()
            };
            jids_to_resolve.push(own);
        }

        let mut devices = self.get_user_devices(&jids_to_resolve).await?;
        if is_lid_mode {
            // WA Web expects LID addressing in SKDM <to> nodes for LID groups.
            devices = devices
                .into_iter()
                .map(|d| group_info.phone_device_jid_into_lid(d))
                .collect();
        }
        Ok(devices)
    }

    /// Resolve a user identifier to its lookup keys with type information.
    ///
    /// Returns a `UserLookupKeys` enum that explicitly represents:
    /// - `LidWithPn`: User is a LID with known phone number mapping
    /// - `PnWithLid`: User is a phone number with known LID mapping
    /// - `Unknown`: No LID-PN mapping exists (could be either type)
    async fn resolve_lookup_keys(&self, user: &str) -> UserLookupKeys {
        // Check if user is a LID (has a phone number mapping). The `user`-derived
        // key is built inline via CompactString (LID/PN are short), avoiding a
        // heap String per member on every group send.
        if let Some(pn) = self.lid_pn_cache.get_phone_number(user).await {
            return UserLookupKeys::LidWithPn {
                lid: user.into(),
                pn: pn.into(),
            };
        }

        // Check if user is a PN (has a LID mapping)
        if let Some(lid) = self.lid_pn_cache.get_current_lid(user).await {
            return UserLookupKeys::PnWithLid {
                lid,
                pn: user.into(),
            };
        }

        // Unknown user - no mapping exists
        UserLookupKeys::Unknown { user: user.into() }
    }

    /// Server-aware variant of `resolve_lookup_keys` for callers holding a
    /// full `Jid`: a LID user can only key the lid->pn direction and a PN
    /// user only pn->lid, so the known namespace removes the blind second
    /// probe (one `lid_pn_cache` lookup per member instead of two, on every
    /// group send). Other namespaces keep the two-probe fallback.
    async fn resolve_lookup_keys_for_jid(&self, jid: &Jid) -> UserLookupKeys {
        if jid.server == wacore_binary::Server::Lid {
            if let Some(pn) = self.lid_pn_cache.get_phone_number(&jid.user).await {
                return UserLookupKeys::LidWithPn {
                    lid: jid.user.as_str().into(),
                    pn: pn.into(),
                };
            }
            return UserLookupKeys::Unknown {
                user: jid.user.as_str().into(),
            };
        }
        if jid.server == wacore_binary::Server::Pn {
            if let Some(lid) = self.lid_pn_cache.get_current_lid(&jid.user).await {
                return UserLookupKeys::PnWithLid {
                    lid,
                    pn: jid.user.as_str().into(),
                };
            }
            return UserLookupKeys::Unknown {
                user: jid.user.as_str().into(),
            };
        }
        self.resolve_lookup_keys(&jid.user).await
    }

    /// Owned-key variant of `resolve_lookup_keys`. Test-only: production callers
    /// use the borrowed `resolve_lookup_keys(..).all_keys()` to avoid the churn.
    #[cfg(test)]
    pub(crate) async fn get_lookup_keys(&self, user: &str) -> Vec<String> {
        self.resolve_lookup_keys(user)
            .await
            .all_keys()
            .into_iter()
            .map(String::from)
            .collect()
    }

    /// WA Web: `isFromKnownDevice(author)` — local check only, no network.
    pub(crate) async fn is_from_known_device(&self, sender: &wacore_binary::Jid) -> bool {
        let device_id = sender.device as u32;
        self.has_device(&sender.user, device_id).await
    }

    /// Check if a device exists for a user.
    /// Returns true for device_id 0 (primary device always exists).
    pub(crate) async fn has_device(&self, user: &str, device_id: u32) -> bool {
        if device_id == 0 {
            return true;
        }

        // Borrowed `&str` keys (like get_devices_from_registry), bound once so both
        // loops share one Vec<&str>: avoids the per-message get_lookup_keys churn.
        let lookup = self.resolve_lookup_keys(user).await;
        let keys = lookup.all_keys();

        for &key in &keys {
            if let Some(record) = self.device_registry_cache.get(key).await {
                return record.devices.iter().any(|d| d.device_id == device_id);
            }
        }

        let backend = self.persistence_manager.backend();
        for &key in &keys {
            match backend.get_devices(key).await {
                Ok(Some(record)) => {
                    let has_device = record.devices.iter().any(|d| d.device_id == device_id);
                    // Cache under the record's actual stored key, not our guessed one,
                    // to keep the cache and backend consistent.
                    self.device_registry_cache
                        .promote(record.user.clone(), Arc::new(record))
                        .await;
                    return has_device;
                }
                Ok(None) => continue,
                Err(e) => {
                    warn!("Failed to check device registry for {}: {e}", key);
                }
            }
        }

        false
    }

    /// Update the device list for a user.
    /// Stores under LID when mapping is known, otherwise under PN.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.update_device_list",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn update_device_list(
        &self,
        mut record: wacore::store::traits::DeviceListRecord,
    ) -> Result<()> {
        use anyhow::Context;

        let original_user = record.user.clone();
        let lookup = self.resolve_lookup_keys(&original_user).await;
        let canonical_key = lookup.canonical_key().to_string();
        record.user.clone_from(&canonical_key); // More efficient: reuses allocation

        // Clone record for cache before moving to backend
        let record_for_cache = record.clone();

        // Use canonical_key directly as cache key (no extra clone)
        // Record every lookup alias, not just canonical+original: a LID-keyed
        // update must also touch the mapped PN, or a PN-addressed group's memo
        // (whose member set only knows the PN side) would re-stamp stale.
        self.device_registry_cache
            .insert(
                canonical_key.clone(),
                Arc::new(record_for_cache),
                lookup
                    .all_keys()
                    .into_iter()
                    .chain(std::iter::once(original_user.as_str())),
            )
            .await;

        let backend = self.persistence_manager.backend();
        backend
            .update_device_list(record)
            .await
            .context("Failed to update device list in backend")?;

        if canonical_key != original_user {
            // Invalidate before + after delete so a concurrent reader that
            // resurrects the cache from the about-to-be-deleted DB row still
            // gets cleared. Run the second invalidate unconditionally: even
            // if delete fails, the cache may have been repopulated with data
            // that no longer reflects our intent.
            self.device_registry_cache.invalidate(&original_user).await;
            if let Err(e) = backend.delete_devices(&original_user).await {
                warn!(
                    "Failed to delete stale device row under {} after canonical flip: {e}",
                    original_user
                );
            }
            self.device_registry_cache.invalidate(&original_user).await;
            debug!(
                "Device registry: stored under LID {} (resolved from {})",
                canonical_key, original_user
            );
        }

        Ok(())
    }

    /// Batched variant of [`update_device_list`]. Cache is populated
    /// synchronously per record (cheap in-process inserts); the backend write
    /// collapses into a single transaction. Used by usync after fetching
    /// device lists for many users at once, where the per-row commit
    /// dominated wall-clock time on large groups.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.update_device_lists", level = "debug", skip_all, fields(count = records.len()), err(Debug)))]
    pub(crate) async fn update_device_lists(
        &self,
        records: Vec<wacore::store::traits::DeviceListRecord>,
    ) -> Result<()> {
        use anyhow::Context;

        if records.is_empty() {
            return Ok(());
        }

        let mut prepared = Vec::with_capacity(records.len());
        let mut to_delete: Vec<String> = Vec::new();

        for mut record in records {
            let original_user = record.user.clone();
            let lookup = self.resolve_lookup_keys(&original_user).await;
            let canonical_key = lookup.canonical_key().to_string();
            record.user.clone_from(&canonical_key);

            let record_for_cache = record.clone();
            // Same alias rule as update_device_list: record every lookup key.
            self.device_registry_cache
                .insert(
                    canonical_key.clone(),
                    Arc::new(record_for_cache),
                    lookup
                        .all_keys()
                        .into_iter()
                        .chain(std::iter::once(original_user.as_str())),
                )
                .await;

            if canonical_key != original_user {
                to_delete.push(original_user);
            }
            prepared.push(record);
        }

        let backend = self.persistence_manager.backend();
        backend
            .update_device_lists(prepared)
            .await
            .context("Failed to update device lists in backend")?;

        // Canonical-flip cleanup is rare and per-row; keep the original
        // pattern (invalidate cache + best-effort delete + re-invalidate)
        // rather than batching deletes. On error we log and continue so a
        // single bad row doesn't drop the rest of the batch.
        for original_user in to_delete {
            self.device_registry_cache.invalidate(&original_user).await;
            if let Err(e) = backend.delete_devices(&original_user).await {
                warn!(
                    "Failed to delete stale device row under {} after canonical flip: {e}",
                    original_user
                );
            }
            self.device_registry_cache.invalidate(&original_user).await;
        }

        Ok(())
    }

    /// Spawn the local identity-change reaction off the current path so it runs
    /// after any held session lock is released (the reaction acquires its own
    /// locks and must not deadlock against an in-flight decrypt/encrypt batch).
    ///
    /// Triggered from both the inbound decrypt path and the outbound
    /// session-establishment paths when `save_identity` reports
    /// [`IdentityChange::ReplacedExisting`](wacore::libsignal::protocol::IdentityChange),
    /// mirroring WA Web `saveIdentity` -> `handleNewIdentity`. Gating
    /// (primary-device, skip-self) lives in [`handle_local_identity_change`].
    ///
    /// [`handle_local_identity_change`]: crate::handlers::notification::handle_local_identity_change
    pub(crate) fn react_to_local_identity_change(&self, sender: &Jid) {
        let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) else {
            return;
        };
        let sender = sender.clone();
        self.runtime
            .spawn(Box::pin(async move {
                crate::handlers::notification::handle_local_identity_change(&client, sender).await;
            }))
            .detach();
    }

    /// Invalidate cached device data for a specific user.
    ///
    /// Removes all device registry cache entries (all LID/PN aliases) so the
    /// next lookup falls through to the database or network.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.invalidate_device_cache",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn invalidate_device_cache(&self, user: &str) {
        let lookup = self.resolve_lookup_keys(user).await;

        for key in lookup.all_keys() {
            self.device_registry_cache.invalidate(key).await;
            // Also delete from DB so get_devices_from_registry doesn't
            // fall back to stale persisted data — forces a network re-fetch
            if let Err(e) = self.persistence_manager.backend().delete_devices(key).await {
                warn!("Failed to delete device registry from DB for {key}: {e}");
            }
            // Invalidate again after the delete: a concurrent reader that read
            // the doomed DB row can promote() it back between the first
            // invalidate and the delete commit (same guard as the canonical
            // flip path in update_device_list).
            self.device_registry_cache.invalidate(key).await;
        }

        debug!("Invalidated device cache for user: {} ({:?})", user, lookup);
    }

    /// Patch device registry after a device add notification.
    ///
    /// Matches WA Web's `handleDeviceAddNotification()` in `AdvDeviceNotificationApi`:
    /// 1. Decode `key-index-list` signed bytes → `ADVKeyIndexList`
    /// 2. Filter existing devices by `valid_indexes` (prune stale devices)
    /// 3. Add the new device
    /// 4. Replace the full device record
    ///
    /// If `signed_bytes` is absent, falls back to simple append (lenient).
    ///
    /// New devices need no explicit cache invalidation: `resolve_skdm_targets`
    /// queries the registry on each send and `device_has_key()` returns `None`
    /// for unseen device IDs, dropping them into `needs_skdm` automatically.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.session.patch_device_add", level = "debug", skip_all)
    )]
    pub(crate) async fn patch_device_add(
        &self,
        user: &str,
        device: &wacore::stanza::devices::DeviceElement,
        key_index_info: Option<&wacore::stanza::devices::KeyIndexInfo>,
    ) {
        let device_id = device.device_id();

        let Some(mut record) = self.load_device_record(user).await else {
            return;
        };

        let signed_bytes = key_index_info.and_then(|ki| ki.signed_bytes.as_deref());

        if let Some(bytes) = signed_bytes {
            if let Some(decoded) = wacore::adv::decode_key_index_list(bytes) {
                // Check raw_id mismatch (identity change)
                // TODO: WA Web also triggers clearRecord on advAccountType change
                // (HOSTED ↔ E2EE), gated behind bizCoexGatingUtils.bizHostedDevicesEnabled().
                // Add when we implement hosted device coexistence support.
                if let Some(stored_raw_id) = record.raw_id
                    && stored_raw_id != decoded.raw_id
                {
                    info!(
                        "raw_id mismatch for user {user}: stored={stored_raw_id}, received={}. Clearing record.",
                        decoded.raw_id
                    );
                    self.clear_device_record(user, device.jid.server.as_str(), &record)
                        .await;
                    record.devices.clear();
                }
                record.raw_id = Some(decoded.raw_id);

                // Filter stale devices by valid_indexes
                record.devices =
                    wacore::adv::filter_devices_by_key_index(&record.devices, &decoded);

                // Only add the new device if its key_index is accepted by the ADV list
                if !record.devices.iter().any(|d| d.device_id == device_id)
                    && wacore::adv::is_key_index_valid(device.key_index, &decoded)
                {
                    record.devices.push(wacore::store::traits::DeviceInfo {
                        device_id,
                        key_index: device.key_index,
                    });
                }
            } else {
                warn!("patch_device_add: failed to decode key-index-list for user {user}");
                self.append_device_if_new(&mut record, device_id, device.key_index);
            }
        } else {
            // No signed bytes — fall back to simple append
            self.append_device_if_new(&mut record, device_id, device.key_index);
        }

        // WA Web `AdvDeviceNotificationApi.handleDeviceAddNotification` re-adds the
        // primary (device 0) to the rebuilt list unconditionally, so a raw_id
        // mismatch (which clears the list) never drops it. `filter_devices_by_key_index`
        // only keeps device 0 when it is already in the input, so after a clear the
        // primary would otherwise be lost, leaving a record with no device 0 (or an
        // empty list) that suppresses the usync re-fetch forever.
        //
        // The primary's key_index is never read (`filter_devices_by_key_index` keeps
        // device 0 regardless and `is_key_index_valid` is not applied to it), so store
        // `None` to match how device 0 is recorded everywhere else.
        if !record.devices.iter().any(|d| d.device_id == 0) {
            record.devices.push(wacore::store::traits::DeviceInfo {
                device_id: 0,
                key_index: None,
            });
        }

        // New devices are picked up automatically by `resolve_skdm_targets`:
        // unknown device → `device_has_key()` returns `None` → falls into
        // `needs_skdm`. No global cache invalidation needed.

        if let Err(e) = self.update_device_list(record).await {
            warn!("patch_device_add: failed to persist: {e}");
        }
    }

    /// Append a device if it doesn't already exist in the record.
    fn append_device_if_new(
        &self,
        record: &mut wacore::store::traits::DeviceListRecord,
        device_id: u32,
        key_index: Option<u32>,
    ) {
        if !record.devices.iter().any(|d| d.device_id == device_id) {
            record.devices.push(wacore::store::traits::DeviceInfo {
                device_id,
                key_index,
            });
        }
    }

    /// Delete Signal sessions for specific device IDs under both LID and PN
    /// addresses, then flush. Shared by `clear_device_record` and
    /// `patch_device_remove`.
    async fn delete_sessions_for_devices(&self, user: &str, device_ids: &[u16]) {
        let lookup = self.resolve_lookup_keys(user).await;
        let servers = [wacore_binary::Server::Lid, wacore_binary::Server::Pn];
        for server in servers {
            for key in lookup.all_keys() {
                for &device_id in device_ids {
                    let mut jid = Jid::new(key, server);
                    jid.device = device_id;
                    let addr = wacore::types::jid::JidExt::to_protocol_address(&jid);
                    self.signal_cache.delete_session(&addr).await;
                }
            }
        }
        self.flush_signal_cache_logged("delete_sessions_for_devices", None)
            .await;
    }

    /// Clear device record on raw_id mismatch (identity change).
    ///
    /// Matches WA Web's `clearDeviceRecord()` in `IdentityUpdateDeviceTableApi`:
    /// - Deletes Signal sessions for non-primary devices (stale identity)
    /// - Invalidates sender key device cache so SKDM will be redistributed
    ///
    /// The companion-device session wipe is intentionally not per-device locked
    /// (matches WA Web's single-threaded model). A concurrent encrypt to one of
    /// those companions can re-store a session right after the wipe, but that is
    /// self-healing: the next send re-establishes it via `process_prekey_bundle`.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.session.clear_device_record", level = "debug", skip_all)
    )]
    pub(crate) async fn clear_device_record(
        &self,
        user: &str,
        _server: &str,
        record: &wacore::store::traits::DeviceListRecord,
    ) {
        let non_primary_ids: Vec<u16> = record
            .devices
            .iter()
            .filter(|d| d.device_id != 0)
            .map(|d| d.device_id as u16)
            .collect();
        info!(
            "Clearing device record for user {user}: removing {} non-primary device(s) due to raw_id change",
            non_primary_ids.len()
        );

        self.delete_sessions_for_devices(user, &non_primary_ids)
            .await;

        // WA Web's `WAWebUpdateLocalSignalSession` only calls `markForgetSenderKey`
        // on retry receipts, per-group/per-device. A global SKDM wipe here would
        // empty the tracker often enough to feed the no-distribution path.
    }

    /// Remove a device from the registry after a device remove notification.
    ///
    /// Matches WA Web's `bulkApplyDeviceUpdate` cleanup for removed devices
    /// (`UpdateDeviceTableApi`): deletes Signal sessions for the device,
    /// then invalidates the sender key device cache so SKDM will be
    /// redistributed on the next group send.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.patch_device_remove", level = "debug", skip_all, fields(device_id = device_id)))]
    pub(crate) async fn patch_device_remove(&self, user: &str, device_id: u32) {
        // WA Web's remove path re-adds the primary unconditionally, mirroring its
        // add path: device 0 is never dropped. Without this guard a remove for the
        // primary would both delete its sender-key rows and persist a record with no
        // device 0, which then suppresses the usync re-fetch forever (the symmetric
        // failure to the add path fixed above).
        if device_id == 0 {
            return;
        }
        if let Some(mut record) = self.load_device_record(user).await {
            let before = record.devices.len();
            record.devices.retain(|d| d.device_id != device_id);
            if record.devices.len() != before {
                // JID-keyed structures (Signal sessions, sender_key_devices)
                // store device as u16. A blind cast for ids > u16::MAX would
                // truncate to a different value and cleanup the wrong device.
                let Ok(device_id_u16) = u16::try_from(device_id) else {
                    warn!(
                        "patch_device_remove: device_id {device_id} > u16::MAX — skipping \
                         session/SKDM cleanup but still persisting registry removal"
                    );
                    if let Err(e) = self.update_device_list(record).await {
                        warn!("patch_device_remove: failed to persist: {e}");
                    }
                    return;
                };

                if device_id_u16 != 0 {
                    self.delete_sessions_for_devices(user, &[device_id_u16])
                        .await;
                }
                // WA Web's `updateGroupParticipantsInTransaction` deletes the
                // device JID from each affected group's senderKey Map. Skip
                // the registry update on failure: a half-applied state where
                // `resolve_devices` says "gone" but the tracker still vouches
                // `has_key=true` would silently skip SKDM redistribution.
                if let Err(e) = self
                    .delete_sender_key_rows_for_device(user, device_id_u16)
                    .await
                {
                    warn!(
                        "patch_device_remove: sender-key cleanup failed for {user}:{device_id}: {e} \
                         — aborting registry update"
                    );
                    return;
                }
                if let Err(e) = self.update_device_list(record).await {
                    warn!("patch_device_remove: failed to persist: {e}");
                }
            }
        }
    }

    /// Delete `sender_key_devices` rows whose `device_jid` matches the given
    /// (user, device_id) under either LID or PN addressing. Both alias keys
    /// for the user are tried via `resolve_lookup_keys`. The in-memory cache
    /// is also evicted for groups that indexed the removed JID — necessary
    /// because a future re-add of the same device_id would otherwise hit
    /// a stale `has_key=true` entry and skip SKDM.
    ///
    /// Cache eviction runs only after the DB delete succeeds; on failure the
    /// error is propagated so the caller can leave both DB and cache in their
    /// pre-call state rather than half-applying the cleanup.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.delete_sender_key_rows", level = "debug", skip_all, fields(device_id = device_id), err(Debug)))]
    async fn delete_sender_key_rows_for_device(
        &self,
        user: &str,
        device_id: u16,
    ) -> Result<(), wacore::store::error::StoreError> {
        let lookup = self.resolve_lookup_keys(user).await;
        let servers = [wacore_binary::Server::Lid, wacore_binary::Server::Pn];
        let mut candidates: Vec<String> = Vec::with_capacity(4);
        for server in servers {
            for key in lookup.all_keys() {
                let mut jid = Jid::new(key, server);
                jid.device = device_id;
                candidates.push(jid.to_string());
            }
        }
        let refs: Vec<&str> = candidates.iter().map(|s| s.as_str()).collect();
        self.persistence_manager
            .delete_sender_key_device_rows(&refs)
            .await?;

        for key in lookup.all_keys() {
            self.sender_key_device_cache
                .invalidate_entries_for_device(key, device_id)
                .await;
        }
        Ok(())
    }

    /// Update key_index for a device in the registry.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.session.patch_device_update", level = "debug", skip_all)
    )]
    pub(crate) async fn patch_device_update(
        &self,
        user: &str,
        device: &wacore::stanza::devices::DeviceElement,
    ) {
        let device_id = device.device_id();

        if let Some(mut record) = self.load_device_record(user).await
            && let Some(d) = record.devices.iter_mut().find(|d| d.device_id == device_id)
        {
            d.key_index = device.key_index;
            if let Err(e) = self.update_device_list(record).await {
                warn!("patch_device_update: failed to persist: {e}");
            }
        }
    }

    /// Load a `DeviceListRecord` from cache or DB for patching.
    pub(crate) async fn load_device_record(
        &self,
        user: &str,
    ) -> Option<wacore::store::traits::DeviceListRecord> {
        let lookup = self.resolve_lookup_keys(user).await;

        for key in lookup.all_keys() {
            if let Some(record) = self.device_registry_cache.get(key).await {
                // Cold load-modify-persist path: callers mutate the owned record.
                return Some((*record).clone());
            }
        }

        let backend = self.persistence_manager.backend();
        for key in lookup.all_keys() {
            match backend.get_devices(key).await {
                Ok(Some(record)) => {
                    self.device_registry_cache
                        .promote(record.user.clone(), Arc::new(record.clone()))
                        .await;
                    return Some(record);
                }
                Ok(None) => continue,
                Err(e) => {
                    warn!("load_device_record: DB lookup failed for {key}: {e}");
                }
            }
        }

        None
    }

    /// Look up device JIDs from the device registry (cache + DB) for a single user.
    ///
    /// Returns `None` if no record exists. On DB hit, re-populates the
    /// `device_registry_cache` for subsequent `has_device()` calls.
    ///
    /// This follows the same 2-tier pattern as [`has_device`]: registry cache first,
    /// then the backend database.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.get_devices_from_registry", level = "trace", skip_all, fields(peer = %jid.observe())))]
    pub(crate) async fn get_devices_from_registry(&self, jid: &Jid) -> Option<Vec<Jid>> {
        // Use the borrowed `&str` keys directly: both the in-process cache and the
        // backend take `&str`, so going through `get_lookup_keys` (which re-owns
        // the already-cloned keys into a `Vec<String>`) just churns per member on
        // every group send. `lookup` owns the key Strings for the duration here.
        let lookup = self.resolve_lookup_keys_for_jid(jid).await;

        // L1: device_registry_cache (in-process, fast)
        for key in lookup.all_keys() {
            if let Some(record) = self.device_registry_cache.get(key).await {
                let devices = Self::reconstruct_device_jids(jid, &record);
                // An empty record is never a valid device set — WA Web always keeps
                // the primary (device 0) — so read it as a miss instead of `Some([])`.
                // The 1:1 send path reads this directly and only warms from the network
                // on `None`; returning `Some([])` would shadow that warmup and the
                // bare-JID fallback, leaving a corrupted empty row unhealed.
                if !devices.is_empty() {
                    return Some(devices);
                }
            }
        }

        // L2: backend DB
        let backend = self.persistence_manager.backend();
        for key in lookup.all_keys() {
            match backend.get_devices(key).await {
                Ok(Some(record)) => {
                    let devices = Self::reconstruct_device_jids(jid, &record);
                    // Same invariant as L1: an empty row is corruption, treat as a miss.
                    if devices.is_empty() {
                        continue;
                    }
                    self.device_registry_cache
                        .promote(record.user.clone(), Arc::new(record))
                        .await;
                    return Some(devices);
                }
                Ok(None) => continue,
                Err(e) => {
                    warn!("get_devices_from_registry: DB lookup failed for {key}: {e}");
                }
            }
        }

        None
    }

    /// Reconstruct `Vec<Jid>` from a `DeviceListRecord`, using the query JID's
    /// user part and server type. This ensures that a PN-typed query always
    /// returns PN-typed device JIDs even if the record is stored under a LID key
    /// (and vice versa), which matters after PN-to-LID migration.
    fn reconstruct_device_jids(
        query_jid: &Jid,
        record: &wacore::store::traits::DeviceListRecord,
    ) -> Vec<Jid> {
        let user = &query_jid.user;
        record
            .devices
            .iter()
            .map(|d| {
                debug_assert!(
                    d.device_id <= u16::MAX as u32,
                    "device_id {} overflows u16",
                    d.device_id
                );
                let device = d.device_id as u16;
                if query_jid.is_lid() {
                    Jid::lid_device(user.clone(), device)
                } else {
                    Jid::pn_device(user.clone(), device)
                }
            })
            .collect()
    }

    /// Background loop placeholder for device registry cleanup.
    /// Note: Cleanup functionality was removed as part of trait simplification.
    /// Device registry entries are managed through normal update/get operations.
    pub(super) async fn device_registry_cleanup_loop(&self) {
        // Simply wait for shutdown signal
        self.shutdown_notifier.listen().await;
        debug!(
            target: "Client/DeviceRegistry",
            "Shutdown signaled, exiting cleanup loop"
        );
    }

    /// Migrate device registry entries from PN key to LID key.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.migrate_device_registry",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn migrate_device_registry_on_lid_discovery(&self, pn: &str, lid: &str) {
        let backend = self.persistence_manager.backend();

        match backend.get_devices(pn).await {
            Ok(Some(mut record)) => {
                info!(
                    "Migrating device registry entry from PN {} to LID {} ({} devices)",
                    pn,
                    lid,
                    record.devices.len()
                );

                record.user = lid.to_string();

                if let Err(e) = backend.update_device_list(record.clone()).await {
                    // The backend row may have changed even on error, so the
                    // change is recorded before the early return; the success
                    // path records once via the fused cache insert below.
                    self.device_topology.record([pn, lid]);
                    warn!("Failed to migrate device registry to LID: {}", e);
                    return;
                }

                self.device_registry_cache
                    .insert(lid.to_string(), Arc::new(record), [lid, pn])
                    .await;

                // Drop the PN-keyed row in both cache and DB. Invalidate
                // twice (before + after delete) so a concurrent reader can't
                // resurrect the cache from the DB row between the two calls.
                // Always run the second invalidate; even if delete fails, the
                // cache may carry resurrected data that shouldn't stick.
                self.device_registry_cache.invalidate(pn).await;
                if let Err(e) = backend.delete_devices(pn).await {
                    warn!("Failed to delete PN-keyed device row during LID migration: {e}");
                }
                self.device_registry_cache.invalidate(pn).await;
            }
            Ok(None) => {}
            Err(e) => {
                warn!("Failed to check for PN device registry entry: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid_pn_cache::LearningSource;
    use crate::test_utils::create_test_client_with_failing_http;
    use std::sync::Arc;

    async fn create_test_client() -> Arc<Client> {
        create_test_client_with_failing_http("device_registry").await
    }

    async fn setup_lid_pn(client: &Arc<Client>, lid: &str, pn: &str) {
        use crate::lid_pn_cache::LidPnEntry;
        let entry = LidPnEntry::new(lid.to_string(), pn.to_string(), LearningSource::Usync);
        client.lid_pn_cache.add(&entry).await;
    }

    async fn setup_device_record(client: &Arc<Client>, user: &str, device_ids: &[u32]) {
        let record = wacore::store::traits::DeviceListRecord {
            user: user.into(),
            devices: device_ids
                .iter()
                .map(|&id| wacore::store::traits::DeviceInfo {
                    device_id: id,
                    key_index: None,
                })
                .collect(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client
            .device_registry_cache
            .raw_insert_for_tests(user.into(), Arc::new(record))
            .await;
    }

    /// The server-aware probe must resolve the same canonical record as the
    /// blind two-probe for both namespaces: a LID jid via its lid->pn mapping
    /// and a PN jid via pn->lid, plus the unmapped-PN fallback.
    #[tokio::test]
    async fn server_aware_probe_resolves_both_namespaces() {
        let client = create_test_client().await;
        let pn = "5511999990000";
        let lid = "100000000000001";
        client
            .add_lid_pn_mapping(lid, pn, crate::lid_pn_cache::LearningSource::Usync)
            .await
            .expect("mapping should persist");
        setup_device_record(&client, pn, &[0, 7]).await;

        let via_pn = client
            .get_devices_from_registry(&Jid::pn(pn))
            .await
            .expect("PN jid must resolve via pn->lid probe");
        assert_eq!(via_pn.len(), 2);
        let via_lid = client
            .get_devices_from_registry(&Jid::lid(lid))
            .await
            .expect("LID jid must resolve via lid->pn probe");
        assert_eq!(via_lid.len(), 2);

        // Unmapped PN still resolves through its own key.
        let bare = "5511888880000";
        setup_device_record(&client, bare, &[0]).await;
        assert!(
            client
                .get_devices_from_registry(&Jid::pn(bare))
                .await
                .is_some(),
            "unmapped PN must resolve via its own record"
        );
    }

    /// Locks the three validity gates of the group-devices memo: a repeat
    /// resolve with the same GroupInfo Arc + generation is a memo hit (proved
    /// by serving a raw cache change STALE), any topology bump recomputes,
    /// and a refreshed GroupInfo (new Arc, same content) recomputes.
    #[tokio::test]
    async fn group_devices_memo_hits_and_invalidates() {
        use wacore::client::context::GroupInfo;
        use wacore::types::message::AddressingMode;

        let client = create_test_client().await;
        let group: Jid = "120363000000000042@g.us".parse().expect("group jid");
        let user_a = "5511999990001";
        let user_b = "5511999990002";
        setup_device_record(&client, user_a, &[0, 5]).await;
        setup_device_record(&client, user_b, &[0]).await;

        let group_info = Arc::new(GroupInfo::new(
            vec![Jid::pn(user_a), Jid::pn(user_b)],
            AddressingMode::Pn,
        ));

        let first = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve should succeed");
        assert_eq!(first.devices().len(), 3, "0+5 for A, 0 for B");

        // Raw cache write WITHOUT a topology bump: the memo must keep serving
        // the snapshot (this is what proves the repeat call was a hit and not
        // a silent recompute).
        setup_device_record(&client, user_a, &[0]).await;
        let stale = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve should succeed");
        assert!(
            std::sync::Arc::ptr_eq(&stale, &first),
            "same Arc + same generation must be a memo hit"
        );

        // A topology change touching a MEMBER invalidates and the recompute
        // sees the new record.
        client.device_topology.record([user_a]);
        let fresh = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve should succeed");
        assert_eq!(
            fresh.devices().len(),
            2,
            "post-bump resolve must see the raw change"
        );

        // A refreshed GroupInfo (new Arc, identical content) must recompute
        // even with an unchanged generation.
        setup_device_record(&client, user_b, &[0, 9]).await;
        let refreshed_info = Arc::new(GroupInfo::new(
            vec![Jid::pn(user_a), Jid::pn(user_b)],
            AddressingMode::Pn,
        ));
        let after_refresh = client
            .resolve_group_devices_memoized(
                &group,
                &refreshed_info,
                &refreshed_info.participants[0],
            )
            .await
            .expect("resolve should succeed");
        assert_eq!(
            after_refresh.devices().len(),
            3,
            "a new GroupInfo Arc must invalidate the memo by identity"
        );
    }

    /// Locks the scoped invalidation: changes touching only OTHER groups'
    /// users re-stamp the memo (still a hit), a member's change recomputes,
    /// and the doubt fallbacks (global event, log overflow) recompute.
    #[tokio::test]
    async fn group_devices_memo_scoped_invalidation() {
        use wacore::client::context::GroupInfo;
        use wacore::types::message::AddressingMode;

        let client = create_test_client().await;
        let group: Jid = "120363000000000077@g.us".parse().expect("group jid");
        let user_a = "5511999990011";
        setup_device_record(&client, user_a, &[0, 5]).await;
        let group_info = Arc::new(GroupInfo::new(vec![Jid::pn(user_a)], AddressingMode::Pn));

        let first = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(first.devices().len(), 2);

        // Raw change (not recorded) + changes touching only a NON-member:
        // the memo must re-stamp and keep serving the snapshot.
        setup_device_record(&client, user_a, &[0]).await;
        client.device_topology.record(["5511000000001"]);
        client.device_topology.record(["5511000000002"]);
        let stale = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert!(
            std::sync::Arc::ptr_eq(&stale, &first),
            "non-member changes must re-stamp, not recompute"
        );

        // A member's change recomputes and sees the raw change.
        client.device_topology.record([user_a]);
        let fresh = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(fresh.devices().len(), 1, "member change must recompute");

        // Global events (mapping cache clear, warm-up) poison the fast path.
        setup_device_record(&client, user_a, &[0, 5, 9]).await;
        client.device_topology.record_global();
        let after_global = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(
            after_global.devices().len(),
            3,
            "global event must recompute"
        );

        // Log overflow past the memo's stamp: cannot prove cleanliness,
        // must recompute.
        setup_device_record(&client, user_a, &[0]).await;
        for _ in 0..300 {
            client.device_topology.record(["5511000000003"]);
        }
        let after_overflow = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(
            after_overflow.devices().len(),
            1,
            "log overflow must recompute"
        );
    }

    /// A mapping add for a member (logged under BOTH its LID and PN keys)
    /// must invalidate even when the group only knows one namespace.
    #[tokio::test]
    async fn group_devices_memo_invalidated_by_member_mapping_change() {
        use wacore::client::context::GroupInfo;
        use wacore::types::message::AddressingMode;

        let client = create_test_client().await;
        let group: Jid = "120363000000000078@g.us".parse().expect("group jid");
        let pn = "5511999990012";
        setup_device_record(&client, pn, &[0]).await;
        let group_info = Arc::new(GroupInfo::new(vec![Jid::pn(pn)], AddressingMode::Pn));

        let first = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(first.devices().len(), 1);

        // Raw change, then learn a LID mapping for the member: the add logs
        // (lid, pn) and the memo's member set carries the PN, so it must
        // recompute even though the group never saw the LID.
        setup_device_record(&client, pn, &[0, 7]).await;
        client
            .add_lid_pn_mapping(
                "100000000000077",
                pn,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .expect("mapping");
        let fresh = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(
            fresh.devices().len(),
            2,
            "a member's mapping change must invalidate the memo"
        );
    }

    /// Review fix: a server group snapshot that omits self used to be
    /// rebuilt by ensure_self_in_group on every send (fresh Arc), making the
    /// memo permanently miss. Keying off the pre-ensure Arc and appending
    /// self inside the derivation keeps the identity stable.
    #[tokio::test]
    async fn memo_hits_when_self_missing_from_group_snapshot() {
        use wacore::client::context::GroupInfo;
        use wacore::types::message::AddressingMode;

        let client = create_test_client().await;
        let group: Jid = "120363000000000080@g.us".parse().expect("group jid");
        let member = "5511999990014";
        let own = Jid::pn("5511999990015");
        setup_device_record(&client, member, &[0]).await;
        setup_device_record(&client, "5511999990015", &[0, 3]).await;

        // Self deliberately absent from the snapshot.
        let group_info = Arc::new(GroupInfo::new(vec![Jid::pn(member)], AddressingMode::Pn));

        let first = client
            .resolve_group_devices_memoized(&group, &group_info, &own)
            .await
            .expect("resolve");
        assert_eq!(
            first.devices().len(),
            3,
            "member device + own's two devices"
        );

        let second = client
            .resolve_group_devices_memoized(&group, &group_info, &own)
            .await
            .expect("resolve");
        assert!(
            Arc::ptr_eq(&first, &second),
            "a self-missing group snapshot must still produce memo hits"
        );
    }

    /// Codex P2 regression: a PN-addressed group's memo only knows the PN
    /// side of a member when the cached GroupInfo carries no LID map, but a
    /// later usync update can arrive keyed by the LID (canonical == original).
    /// The write must record every lookup alias so the memo recomputes
    /// instead of re-stamping stale.
    #[tokio::test]
    async fn lid_keyed_update_invalidates_pn_group_memo() {
        use wacore::client::context::GroupInfo;
        use wacore::types::message::AddressingMode;

        let client = create_test_client().await;
        let group: Jid = "120363000000000079@g.us".parse().expect("group jid");
        let pn = "5511999990013";
        let lid = "100000000000079";

        // Mapping known BEFORE the memo: the canonical record lives under the
        // LID, while the group only references the member by PN.
        client
            .add_lid_pn_mapping(lid, pn, crate::lid_pn_cache::LearningSource::Usync)
            .await
            .expect("mapping");
        client
            .update_device_list(wacore::store::traits::DeviceListRecord {
                user: pn.into(),
                devices: vec![wacore::store::traits::DeviceInfo {
                    device_id: 0,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .expect("seed record");

        let group_info = Arc::new(GroupInfo::new(vec![Jid::pn(pn)], AddressingMode::Pn));
        let first = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(first.devices().len(), 1);

        // The update arrives keyed by the LID: canonical == original == LID,
        // so without the alias rule only the LID would be recorded and the
        // PN-only member set would re-stamp the stale snapshot.
        client
            .update_device_list(wacore::store::traits::DeviceListRecord {
                user: lid.into(),
                devices: vec![
                    wacore::store::traits::DeviceInfo {
                        device_id: 0,
                        key_index: None,
                    },
                    wacore::store::traits::DeviceInfo {
                        device_id: 11,
                        key_index: None,
                    },
                ],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .expect("LID-keyed update");

        let fresh = client
            .resolve_group_devices_memoized(&group, &group_info, &group_info.participants[0])
            .await
            .expect("resolve");
        assert_eq!(
            fresh.devices().len(),
            2,
            "a LID-keyed update for a member must invalidate the PN group's memo"
        );
    }

    /// Locks the invariant that every device-topology write path bumps the
    /// generation. patch_device_add/patch_device_remove funnel their writes
    /// through update_device_list, so the funnel is what is asserted.
    #[tokio::test]
    async fn topology_mutators_bump_the_generation() {
        let client = create_test_client().await;
        let current_gen = |c: &Arc<Client>| c.device_topology.current();

        let before = current_gen(&client);
        client
            .update_device_list(wacore::store::traits::DeviceListRecord {
                user: "5511999990003".into(),
                devices: vec![wacore::store::traits::DeviceInfo {
                    device_id: 0,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .expect("update_device_list");
        assert!(
            current_gen(&client) > before,
            "update_device_list must bump"
        );

        let before = current_gen(&client);
        client
            .update_device_lists(vec![wacore::store::traits::DeviceListRecord {
                user: "5511999990004".into(),
                devices: vec![wacore::store::traits::DeviceInfo {
                    device_id: 0,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            }])
            .await
            .expect("update_device_lists");
        assert!(
            current_gen(&client) > before,
            "update_device_lists must bump"
        );

        let before = current_gen(&client);
        client.invalidate_device_cache("5511999990003").await;
        assert!(
            current_gen(&client) > before,
            "invalidate_device_cache must bump"
        );

        let before = current_gen(&client);
        client
            .add_lid_pn_mapping(
                "100000000000042",
                "5511999990004",
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .expect("mapping should persist");
        assert!(
            current_gen(&client) > before,
            "add_lid_pn_mapping must bump"
        );

        // Fresh pair: the record must live under its PN key (no mapping yet)
        // for the migration to find and move it.
        client
            .update_device_list(wacore::store::traits::DeviceListRecord {
                user: "5511999990005".into(),
                devices: vec![wacore::store::traits::DeviceInfo {
                    device_id: 0,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .expect("seed PN-keyed record");
        let before = current_gen(&client);
        client
            .migrate_device_registry_on_lid_discovery("5511999990005", "100000000000043")
            .await;
        assert!(
            current_gen(&client) > before,
            "migrate_device_registry_on_lid_discovery must bump"
        );
    }

    #[tokio::test]
    async fn warm_registry_hit_shares_arc_not_deep_clone() {
        let client = create_test_client().await;
        setup_device_record(&client, "15551112222", &[1, 2]).await;

        let a = client
            .device_registry_cache
            .get("15551112222")
            .await
            .expect("warm hit");
        let b = client
            .device_registry_cache
            .get("15551112222")
            .await
            .expect("warm hit");

        // A warm registry hit returns a refcount bump of the same allocation, not a deep copy.
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(a.devices.len(), 2);
    }

    #[tokio::test]
    async fn test_resolve_to_canonical_key_unknown_user() {
        let client = create_test_client().await;
        let result = client.resolve_to_canonical_key("15551234567").await;
        assert_eq!(result, "15551234567");
    }

    #[tokio::test]
    async fn test_resolve_to_canonical_key_with_lid_mapping() {
        let client = create_test_client().await;
        let lid = "100000000000001";
        let pn = "15551234567";

        setup_lid_pn(&client, lid, pn).await;

        // PN should resolve to LID
        let result = client.resolve_to_canonical_key(pn).await;
        assert_eq!(result, lid);

        // LID should stay as LID
        let result = client.resolve_to_canonical_key(lid).await;
        assert_eq!(result, lid);
    }

    #[tokio::test]
    async fn test_get_lookup_keys_unknown_user() {
        let client = create_test_client().await;
        let keys = client.get_lookup_keys("15551234567").await;
        assert_eq!(keys, vec!["15551234567"]);
    }

    #[tokio::test]
    async fn test_get_lookup_keys_with_lid_mapping() {
        let client = create_test_client().await;
        let lid = "100000000000001";
        let pn = "15551234567";

        setup_lid_pn(&client, lid, pn).await;

        // Looking up by PN should return [LID, PN]
        let keys = client.get_lookup_keys(pn).await;
        assert_eq!(keys, vec![lid.to_string(), pn.to_string()]);

        // Looking up by LID should return [LID, PN]
        let keys = client.get_lookup_keys(lid).await;
        assert_eq!(keys, vec![lid.to_string(), pn.to_string()]);
    }

    #[tokio::test]
    async fn test_15_digit_lid_handling() {
        let client = create_test_client().await;
        // Real example: 15-digit LID
        let lid = "100000000000001";
        let pn = "15551234567";

        assert_eq!(lid.len(), 15, "LID should be 15 digits");

        setup_lid_pn(&client, lid, pn).await;

        // 15-digit LID should be properly recognized via cache lookup
        let canonical = client.resolve_to_canonical_key(lid).await;
        assert_eq!(canonical, lid);

        let keys = client.get_lookup_keys(lid).await;
        assert_eq!(keys.len(), 2);
        assert_eq!(keys[0], lid);
        assert_eq!(keys[1], pn);
    }

    #[tokio::test]
    async fn test_has_device_primary_always_exists() {
        let client = create_test_client().await;
        assert!(client.has_device("anyuser", 0).await);
    }

    #[tokio::test]
    async fn test_has_device_unknown_device() {
        let client = create_test_client().await;
        assert!(!client.has_device("15551234567", 5).await);
    }

    #[tokio::test]
    async fn test_has_device_with_cached_record() {
        let client = create_test_client().await;
        let lid = "100000000000001";
        let pn = "15551234567";

        setup_lid_pn(&client, lid, pn).await;
        setup_device_record(&client, lid, &[1]).await;

        // Device should be findable via both PN and LID (bidirectional lookup)
        assert!(client.has_device(pn, 1).await);
        assert!(client.has_device(lid, 1).await);
        // Non-existent device should return false
        assert!(!client.has_device(lid, 99).await);
    }

    /// has_device must iterate every lookup key: a record keyed under PN is found
    /// when queried by LID (the fallback key), and vice versa. Guards the
    /// borrowed-`all_keys()` iteration the churn fix preserves.
    #[tokio::test]
    async fn test_has_device_found_via_fallback_lookup_key() {
        let client = create_test_client().await;
        let lid = "100000000000009";
        let pn = "15559998888";

        setup_lid_pn(&client, lid, pn).await;
        setup_device_record(&client, pn, &[2]).await;

        assert!(
            client.has_device(lid, 2).await,
            "device keyed under PN must be found when queried by LID"
        );
        assert!(client.has_device(pn, 2).await);
        assert!(!client.has_device(lid, 77).await);
    }

    /// Test that invalidate_device_cache clears registry cache entries for
    /// all LID/PN aliases when called with either identifier.
    #[tokio::test]
    async fn test_invalidate_device_cache_uses_correct_jid_types() {
        let client = create_test_client().await;
        let lid = "100000000000001";
        let pn = "15551234567";

        setup_lid_pn(&client, lid, pn).await;
        setup_device_record(&client, lid, &[1]).await;

        assert!(client.device_registry_cache.get(lid).await.is_some());

        // Invalidate via PN — should clear LID entry too (bidirectional resolution)
        client.invalidate_device_cache(pn).await;
        assert!(
            client.device_registry_cache.get(lid).await.is_none(),
            "LID entry should be invalidated when called with PN"
        );

        // Re-insert and invalidate via LID
        setup_device_record(&client, lid, &[2]).await;

        client.invalidate_device_cache(lid).await;
        assert!(
            client.device_registry_cache.get(lid).await.is_none(),
            "LID entry should be invalidated when called with LID"
        );
    }

    /// Test that invalidate_device_cache handles unknown users (no LID-PN mapping).
    #[tokio::test]
    async fn test_invalidate_device_cache_unknown_user_invalidates_both_types() {
        let client = create_test_client().await;
        let unknown_user = "100000000000999";

        setup_device_record(&client, unknown_user, &[1]).await;

        assert!(
            client
                .device_registry_cache
                .get(unknown_user)
                .await
                .is_some()
        );

        client.invalidate_device_cache(unknown_user).await;
        assert!(
            client
                .device_registry_cache
                .get(unknown_user)
                .await
                .is_none(),
            "Unknown user entry should be invalidated"
        );
    }

    // ── Granular patch tests ──────────────────────────────────────────────

    fn make_device_element(
        device_id: u16,
        key_index: Option<u32>,
    ) -> wacore::stanza::devices::DeviceElement {
        wacore::stanza::devices::DeviceElement {
            jid: Jid {
                user: "15551234567".into(),
                server: wacore_binary::Server::Pn,
                device: device_id,
                ..Default::default()
            },
            key_index,
            lid: None,
        }
    }

    #[tokio::test]
    async fn test_patch_device_add_to_existing_cache() {
        let client = create_test_client().await;

        // Pre-populate registry cache with device 0
        setup_device_record(&client, "15551234567", &[0]).await;

        // Patch: add device 3
        let elem = make_device_element(3, Some(5));
        client.patch_device_add("15551234567", &elem, None).await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        assert_eq!(updated.devices.len(), 2);
        assert!(updated.devices.iter().any(|d| d.device_id == 3));
        let dev3 = updated.devices.iter().find(|d| d.device_id == 3).unwrap();
        assert_eq!(dev3.key_index, Some(5));
    }

    #[tokio::test]
    async fn test_patch_device_add_deduplicates() {
        let client = create_test_client().await;

        setup_device_record(&client, "15551234567", &[0, 3]).await;

        // Patch: add device 3 again — should not duplicate
        let elem = make_device_element(3, None);
        client.patch_device_add("15551234567", &elem, None).await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        assert_eq!(
            updated.devices.iter().filter(|d| d.device_id == 3).count(),
            1
        );
        assert!(updated.devices.iter().any(|d| d.device_id == 0));
        assert_eq!(updated.devices.len(), 2);
    }

    #[tokio::test]
    async fn test_patch_device_add_noop_on_miss() {
        let client = create_test_client().await;

        // No pre-populated cache — patch should be a no-op
        let elem = make_device_element(3, None);
        client.patch_device_add("15551234567", &elem, None).await;

        assert!(
            client
                .device_registry_cache
                .get("15551234567")
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_patch_device_remove() {
        let client = create_test_client().await;

        setup_device_record(&client, "15551234567", &[0, 3]).await;

        client.patch_device_remove("15551234567", 3).await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        assert_eq!(updated.devices.len(), 1);
        assert_eq!(updated.devices[0].device_id, 0);
    }

    #[tokio::test]
    async fn test_patch_device_update_key_index() {
        let client = create_test_client().await;

        // Pre-populate registry cache
        let record = wacore::store::traits::DeviceListRecord {
            user: "15551234567".to_string(),
            devices: vec![
                wacore::store::traits::DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                wacore::store::traits::DeviceInfo {
                    device_id: 3,
                    key_index: Some(1),
                },
            ],
            timestamp: 1000,
            phash: None,
            raw_id: None,
        };
        client
            .device_registry_cache
            .raw_insert_for_tests("15551234567".to_string(), Arc::new(record))
            .await;

        // Patch: update device 3 key_index to 5
        let elem = make_device_element(3, Some(5));
        client.patch_device_update("15551234567", &elem).await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        let dev3 = updated.devices.iter().find(|d| d.device_id == 3).unwrap();
        assert_eq!(dev3.key_index, Some(5));
    }

    #[tokio::test]
    async fn test_patch_device_add_updates_registry() {
        let client = create_test_client().await;

        // Pre-populate registry cache
        setup_device_record(&client, "15551234567", &[0]).await;

        // Patch: add device 3
        let elem = make_device_element(3, Some(2));
        client.patch_device_add("15551234567", &elem, None).await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        assert_eq!(updated.devices.len(), 2);
        let dev3 = updated.devices.iter().find(|d| d.device_id == 3).unwrap();
        assert_eq!(dev3.key_index, Some(2));
    }

    /// Encode an `ADVSignedKeyIndexList` whose decoded `raw_id`/`valid_indexes`
    /// drive `patch_device_add` (the signature is not verified locally; the
    /// notification arrives over the authenticated Noise channel).
    fn make_signed_key_index_bytes(
        raw_id: u32,
        current_index: u32,
        valid_indexes: Vec<u32>,
    ) -> Vec<u8> {
        use buffa::Message;
        let details = waproto::whatsapp::ADVKeyIndexList {
            raw_id: Some(raw_id),
            timestamp: Some(100),
            current_index: Some(current_index),
            valid_indexes,
            account_type: None,
        }
        .encode_to_vec();
        waproto::whatsapp::ADVSignedKeyIndexList {
            details: Some(details),
            account_signature: None,
            account_signature_key: None,
        }
        .encode_to_vec()
    }

    fn record_with_raw_id(
        user: &str,
        device_ids: &[u32],
        raw_id: u32,
    ) -> wacore::store::traits::DeviceListRecord {
        wacore::store::traits::DeviceListRecord {
            user: user.into(),
            devices: device_ids
                .iter()
                .map(|&id| wacore::store::traits::DeviceInfo {
                    device_id: id,
                    key_index: if id == 0 { None } else { Some(7) },
                })
                .collect(),
            timestamp: 1000,
            phash: None,
            raw_id: Some(raw_id),
        }
    }

    // raw_id mismatch clears the list and rebuilds from the notification; the
    // primary (device 0) must survive. Regression guard for a record that would
    // otherwise persist without device 0 (and then suppress usync forever).
    #[tokio::test]
    async fn test_patch_device_add_raw_id_mismatch_preserves_primary() {
        let client = create_test_client().await;

        client
            .device_registry_cache
            .raw_insert_for_tests(
                "15551234567".to_string(),
                Arc::new(record_with_raw_id("15551234567", &[0, 5], 1)),
            )
            .await;

        // New raw_id (2) != stored (1) → clear + rebuild. Notified device 19 has a
        // valid key index, so the rebuilt list is the companion plus the primary.
        let signed = make_signed_key_index_bytes(2, 0, vec![7]);
        let key_index_info = wacore::stanza::devices::KeyIndexInfo {
            timestamp: 100,
            signed_bytes: Some(signed),
        };
        let elem = make_device_element(19, Some(7));
        client
            .patch_device_add("15551234567", &elem, Some(&key_index_info))
            .await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        let dev0 = updated
            .devices
            .iter()
            .find(|d| d.device_id == 0)
            .unwrap_or_else(|| {
                panic!(
                    "primary (device 0) must survive a raw_id mismatch clear, got {:?}",
                    updated.devices
                )
            });
        // The re-seeded primary carries no key index (it is never read for device 0).
        assert_eq!(dev0.key_index, None);
        assert!(updated.devices.iter().any(|d| d.device_id == 19));
        // Stale companion from the old identity is dropped by the clear.
        assert!(!updated.devices.iter().any(|d| d.device_id == 5));
    }

    // Same mismatch but the notified device's key index is rejected, so the
    // rebuilt list would be empty without the primary re-seed. Guards the `[]`
    // record that otherwise leaves the user with zero devices.
    #[tokio::test]
    async fn test_patch_device_add_raw_id_mismatch_rejected_device_keeps_primary() {
        let client = create_test_client().await;

        client
            .device_registry_cache
            .raw_insert_for_tests(
                "15551234567".to_string(),
                Arc::new(record_with_raw_id("15551234567", &[0, 5], 1)),
            )
            .await;

        // current_index 10, empty valid set → notified key index 3 is invalid
        // (not in valid set, not > current_index), so no companion is added.
        let signed = make_signed_key_index_bytes(2, 10, vec![]);
        let key_index_info = wacore::stanza::devices::KeyIndexInfo {
            timestamp: 100,
            signed_bytes: Some(signed),
        };
        let elem = make_device_element(19, Some(3));
        client
            .patch_device_add("15551234567", &elem, Some(&key_index_info))
            .await;

        let updated = client
            .device_registry_cache
            .get("15551234567")
            .await
            .unwrap();
        assert_eq!(
            updated.devices.len(),
            1,
            "expected only the primary, got {:?}",
            updated.devices
        );
        assert_eq!(updated.devices[0].device_id, 0);
        assert_eq!(updated.devices[0].key_index, None);
    }

    #[tokio::test]
    async fn test_lid_migration_preserves_registry_cache() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;
        let pn = "15550000099";
        let lid = "100000000000099";

        // Store device list under PN in backend
        let record = DeviceListRecord {
            user: pn.to_string(),
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
        client
            .persistence_manager
            .backend()
            .update_device_list(record)
            .await
            .unwrap();

        setup_lid_pn(&client, lid, pn).await;

        // Migrate
        client
            .migrate_device_registry_on_lid_discovery(pn, lid)
            .await;

        // LID entry should exist in registry cache
        let cached = client.device_registry_cache.get(lid).await;
        assert!(
            cached.is_some(),
            "LID key should be in registry cache after migration"
        );
        assert_eq!(cached.unwrap().devices.len(), 2);

        // PN entry should be gone
        let pn_cached = client.device_registry_cache.get(pn).await;
        assert!(
            pn_cached.is_none(),
            "PN key should be invalidated after migration"
        );

        // get_devices_from_registry should find devices via LID lookup
        let lid_jid = Jid::lid(lid);
        let devices = client.get_devices_from_registry(&lid_jid).await;
        assert!(devices.is_some(), "should resolve devices via LID");
        assert_eq!(devices.unwrap().len(), 2);
    }

    /// Regression: querying a LID-stored record by PN (and vice versa) must
    /// return device JIDs whose user part matches the *query* alias, not the
    /// storage key.
    #[tokio::test]
    async fn test_reconstruct_device_jids_uses_query_alias() {
        let client = create_test_client().await;
        let pn = "15550000088";
        let lid = "100000000000088";

        setup_device_record(&client, lid, &[5]).await;
        setup_lid_pn(&client, lid, pn).await;

        // Query by PN — should find the LID-stored record but return PN-typed JIDs
        let pn_jid = Jid::pn(pn);
        let devices = client
            .get_devices_from_registry(&pn_jid)
            .await
            .expect("should resolve LID record via PN alias");
        assert_eq!(devices.len(), 1);
        assert!(devices[0].is_pn(), "device JID should be PN-typed");
        assert_eq!(
            devices[0].user, pn,
            "device JID user should be the PN, not the LID"
        );
        assert_eq!(devices[0].device, 5);

        // Query by LID — should return LID-typed JIDs
        let lid_jid = Jid::lid(lid);
        let devices = client
            .get_devices_from_registry(&lid_jid)
            .await
            .expect("should resolve LID record via LID");
        assert_eq!(devices.len(), 1);
        assert!(devices[0].is_lid(), "device JID should be LID-typed");
        assert_eq!(devices[0].user, lid, "device JID user should be the LID");
    }

    // A present-but-empty record must read as a miss (None), not Some([]). The
    // 1:1 send path reads get_devices_from_registry directly and only warms from
    // the network on None, so an empty Some would shadow that warmup and the
    // bare-JID fallback, leaving the corrupted row unhealed on the send path.
    #[tokio::test]
    async fn get_devices_from_registry_reads_empty_record_as_miss() {
        let client = create_test_client().await;
        let user = "15551234567";
        setup_device_record(&client, user, &[]).await;

        assert!(
            client
                .get_devices_from_registry(&Jid::pn(user))
                .await
                .is_none(),
            "an empty record must read as a miss, not Some([])"
        );
    }

    // ── DB-fallback tests for patch helpers ──────────────────────────────

    #[tokio::test]
    async fn test_patch_device_add_falls_back_to_db() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;

        // Seed backend DB directly (bypassing the in-process cache)
        let record = DeviceListRecord {
            user: "15551234567".into(),
            devices: vec![DeviceInfo {
                device_id: 0,
                key_index: None,
            }],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client
            .persistence_manager
            .backend()
            .update_device_list(record)
            .await
            .unwrap();

        // Moka cache is empty — old code would no-op here
        assert!(
            client
                .device_registry_cache
                .get("15551234567")
                .await
                .is_none()
        );

        let elem = make_device_element(3, Some(7));
        client.patch_device_add("15551234567", &elem, None).await;

        // Verify patch was applied to DB (not silently dropped)
        let updated = client
            .persistence_manager
            .backend()
            .get_devices("15551234567")
            .await
            .unwrap()
            .expect("record should still exist in DB");
        assert_eq!(updated.devices.len(), 2);
        assert!(updated.devices.iter().any(|d| d.device_id == 3));

        // Cache should be warm now too
        assert!(
            client
                .device_registry_cache
                .get("15551234567")
                .await
                .is_some()
        );
    }

    #[tokio::test]
    async fn test_patch_device_remove_falls_back_to_db() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;

        let record = DeviceListRecord {
            user: "15551234567".into(),
            devices: vec![
                DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                DeviceInfo {
                    device_id: 3,
                    key_index: Some(5),
                },
            ],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client
            .persistence_manager
            .backend()
            .update_device_list(record)
            .await
            .unwrap();

        assert!(
            client
                .device_registry_cache
                .get("15551234567")
                .await
                .is_none()
        );

        client.patch_device_remove("15551234567", 3).await;

        let updated = client
            .persistence_manager
            .backend()
            .get_devices("15551234567")
            .await
            .unwrap()
            .expect("record should still exist");
        assert_eq!(updated.devices.len(), 1);
        assert_eq!(updated.devices[0].device_id, 0);
    }

    // ── Sender key device cache: post-fix behavior ──────────────────────

    /// `device_has_key` returns `None` for unknown devices, so an added device
    /// naturally falls into `needs_skdm` on the next send without any cache wipe.
    #[tokio::test]
    async fn test_patch_device_add_keeps_cache_warm_new_device_seen_as_unknown() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let client = create_test_client().await;
        setup_device_record(&client, "15551234567", &[0]).await;

        let group = "120363000000000001@g.us";
        let map =
            SenderKeyDeviceMap::from_db_rows(&[("15551234567:0@s.whatsapp.net".into(), true)]);
        client
            .sender_key_device_cache
            .get_or_init(group, async { std::sync::Arc::new(map) })
            .await;

        let elem = make_device_element(3, Some(5));
        client.patch_device_add("15551234567", &elem, None).await;

        let warm = client
            .sender_key_device_cache
            .get_or_init(group, async {
                panic!("cache should still be warm — no global invalidation")
            })
            .await;
        assert_eq!(warm.device_has_key("15551234567", 0), Some(true));
        assert_eq!(warm.device_has_key("15551234567", 3), None);
    }

    #[tokio::test]
    async fn test_patch_device_add_no_invalidation_when_device_exists() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;

        // Pre-populate device registry with device 0 AND device 3
        let record = DeviceListRecord {
            user: "15551234567".into(),
            devices: vec![
                DeviceInfo {
                    device_id: 0,
                    key_index: None,
                },
                DeviceInfo {
                    device_id: 3,
                    key_index: Some(5),
                },
            ],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client
            .device_registry_cache
            .raw_insert_for_tests("15551234567".into(), Arc::new(record))
            .await;

        // Warm the sender key device cache
        let group = "120363000000000001@g.us";
        let map = SenderKeyDeviceMap::from_db_rows(&[
            ("15551234567:0@s.whatsapp.net".into(), true),
            ("15551234567:3@s.whatsapp.net".into(), true),
        ]);
        client
            .sender_key_device_cache
            .get_or_init(group, async { std::sync::Arc::new(map) })
            .await;

        // Re-add device 3 (already exists) — should NOT invalidate cache
        let elem = make_device_element(3, Some(5));
        client.patch_device_add("15551234567", &elem, None).await;

        // Cache should still have the old entry
        let cached = client
            .sender_key_device_cache
            .get_or_init(group, async {
                panic!("init should not be called — cache should still be warm")
            })
            .await;
        assert!(!cached.is_empty(), "cache should still be warm");
    }

    /// On remove, the sender_key_devices DB row for the device is dropped
    /// (mirrors WA Web's `senderKey.delete(deviceJid)`). The next resolve sees
    /// the device gone from the registry and skips it, so no SKDM redistribution
    /// is needed for surviving devices.
    #[tokio::test]
    async fn test_patch_device_remove_clears_row_and_keeps_others_warm() {
        let client = create_test_client().await;
        setup_device_record(&client, "15551234567", &[0, 3]).await;

        let group = "120363000000000001@g.us";
        client
            .persistence_manager
            .set_sender_key_status(
                group,
                &[
                    ("15551234567:0@s.whatsapp.net", true),
                    ("15551234567:3@s.whatsapp.net", true),
                ],
            )
            .await
            .unwrap();

        client.patch_device_remove("15551234567", 3).await;

        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert!(
            rows.iter()
                .any(|(j, _)| j == "15551234567:0@s.whatsapp.net")
        );
        assert!(
            !rows
                .iter()
                .any(|(j, _)| j == "15551234567:3@s.whatsapp.net")
        );
    }

    // ── LID↔PN zombie-path regression tests (PR #579) ───────────────────

    /// U1 — `update_device_list` deletes the stale DB row when the canonical
    /// key flips (e.g. the LID↔PN mapping is learned between two writes).
    /// Without this, the old PN-keyed row lingers and re-surfaces as a zombie
    /// through alias lookup, causing 406s on group sends.
    #[tokio::test]
    async fn test_update_device_list_canonical_flip_deletes_old_db_row() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;
        let pn = "15550000011";
        let lid = "100000000000011";
        let backend = client.persistence_manager.backend();

        // Legacy state: DB row stored under PN (mapping wasn't known yet).
        backend
            .update_device_list(DeviceListRecord {
                user: pn.to_string(),
                devices: vec![DeviceInfo {
                    device_id: 5,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        setup_lid_pn(&client, lid, pn).await;

        // New write: `update_device_list` with original_user = PN, canonical
        // now resolves to LID because the mapping is known.
        client
            .update_device_list(DeviceListRecord {
                user: pn.to_string(),
                devices: vec![DeviceInfo {
                    device_id: 7,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        assert!(
            backend.get_devices(pn).await.unwrap().is_none(),
            "old PN-keyed DB row must be deleted after canonical flip"
        );
        let lid_row = backend.get_devices(lid).await.unwrap();
        assert!(lid_row.is_some(), "new LID-keyed DB row must exist");
        assert_eq!(lid_row.unwrap().devices[0].device_id, 7);
    }

    /// U2 — `migrate_device_registry_on_lid_discovery` deletes the PN-keyed DB
    /// row, not just the cache entry. Without this the PN row stayed around
    /// as a zombie that surfaced via alias lookup on future sends.
    #[tokio::test]
    async fn test_migrate_device_registry_deletes_pn_db_row() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;
        let pn = "15550000022";
        let lid = "100000000000022";
        let backend = client.persistence_manager.backend();

        backend
            .update_device_list(DeviceListRecord {
                user: pn.to_string(),
                devices: vec![DeviceInfo {
                    device_id: 0,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        setup_lid_pn(&client, lid, pn).await;

        client
            .migrate_device_registry_on_lid_discovery(pn, lid)
            .await;

        assert!(
            backend.get_devices(pn).await.unwrap().is_none(),
            "PN-keyed DB row must be gone after migration"
        );
        assert!(
            backend.get_devices(lid).await.unwrap().is_some(),
            "LID-keyed DB row must exist after migration"
        );
    }

    /// U3 — `invalidate_device_cache` with a known LID↔PN mapping clears both
    /// aliases from the DB (not only the cache). This is the primary fix for
    /// the 23-batches-in-3h45m zombie loop from the field report.
    #[tokio::test]
    async fn test_invalidate_device_cache_clears_both_aliases_from_db() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;
        let pn = "15550000033";
        let lid = "100000000000033";
        let backend = client.persistence_manager.backend();

        // Seed DB under BOTH aliases (simulating split-brain legacy state).
        for user in [pn, lid] {
            backend
                .update_device_list(DeviceListRecord {
                    user: user.to_string(),
                    devices: vec![DeviceInfo {
                        device_id: 1,
                        key_index: None,
                    }],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: None,
                })
                .await
                .unwrap();
        }
        setup_lid_pn(&client, lid, pn).await;

        client.invalidate_device_cache(lid).await;

        assert!(
            backend.get_devices(pn).await.unwrap().is_none(),
            "PN DB row must be deleted via alias resolution"
        );
        assert!(
            backend.get_devices(lid).await.unwrap().is_none(),
            "LID DB row must be deleted"
        );
        assert!(
            client.device_registry_cache.get(pn).await.is_none(),
            "PN cache entry must be gone"
        );
        assert!(
            client.device_registry_cache.get(lid).await.is_none(),
            "LID cache entry must be gone"
        );
    }

    /// U4 — canonical-flip path with a warm cache: no zombie entry survives.
    ///
    /// This does *not* deterministically exercise the TOCTOU window between
    /// invalidate1 and delete — the first invalidate clears the pre-seeded
    /// cache, so the test would pass even without the post-delete second
    /// invalidate. Reaching that window requires interleaving a concurrent
    /// reader between those two calls, which would need a backend-level
    /// latch (i.e., wrapping `Backend` to run a hook before `delete_devices`).
    /// The full trait has ~50 methods via blanket impl, so that machinery is
    /// out of scope for this PR; the double-invalidate lives on as
    /// defense-in-depth validated by code review rather than this test.
    ///
    /// What this still guards: the first invalidate + DB delete end-to-end
    /// (removing either one would fail this test).
    #[tokio::test]
    async fn test_update_device_list_canonical_flip_clears_warm_cache() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let client = create_test_client().await;
        let pn = "15550000044";
        let lid = "100000000000044";
        let backend = client.persistence_manager.backend();

        let legacy = DeviceListRecord {
            user: pn.to_string(),
            devices: vec![DeviceInfo {
                device_id: 9,
                key_index: None,
            }],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        backend.update_device_list(legacy.clone()).await.unwrap();
        // Warm cache under PN to simulate a reader that populated it before
        // the mapping was learned.
        client
            .device_registry_cache
            .raw_insert_for_tests(pn.into(), Arc::new(legacy))
            .await;

        setup_lid_pn(&client, lid, pn).await;

        client
            .update_device_list(DeviceListRecord {
                user: pn.to_string(),
                devices: vec![DeviceInfo {
                    device_id: 10,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        assert!(
            client.device_registry_cache.get(pn).await.is_none(),
            "cache[pn] must be cleared after canonical flip"
        );
        assert!(
            backend.get_devices(pn).await.unwrap().is_none(),
            "DB[pn] must be deleted after canonical flip"
        );
    }

    // ── SKDM flow regression tests ─────────────────────────────────────

    /// After remove, the in-memory cache must not return `has_key=true` for
    /// the removed JID. A future re-add of the same device_id would otherwise
    /// hit the stale entry and skip SKDM redistribution.
    #[tokio::test]
    async fn patch_device_remove_evicts_cached_has_key_for_removed_device() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let client = create_test_client().await;
        let user = "15551234567";
        setup_device_record(&client, user, &[0, 5]).await;

        let group = "120363000000000001@g.us";
        let map = SenderKeyDeviceMap::from_db_rows(&[(format!("{user}:5@s.whatsapp.net"), true)]);
        client
            .sender_key_device_cache
            .get_or_init(group, async { std::sync::Arc::new(map) })
            .await;

        client.patch_device_remove(user, 5).await;

        let reloaded = client
            .sender_key_device_cache
            .get_or_init(group, async {
                std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(
                    &client
                        .persistence_manager
                        .get_sender_key_devices(group)
                        .await
                        .unwrap(),
                ))
            })
            .await;
        assert_eq!(reloaded.device_has_key(user, 5), None);
    }

    #[tokio::test]
    async fn patch_device_remove_clears_sender_key_device_rows() {
        let client = create_test_client().await;
        let user = "15551234567";
        setup_device_record(&client, user, &[0, 5]).await;

        let group = "120363000000000001@g.us";
        let device_jid = format!("{user}:5@s.whatsapp.net");
        client
            .persistence_manager
            .set_sender_key_status(group, &[(device_jid.as_str(), true)])
            .await
            .unwrap();

        client.patch_device_remove(user, 5).await;

        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert!(rows.iter().all(|(jid, _)| jid != &device_jid));
    }

    // A remove targeting the primary (device 0) must be a no-op: WA Web never
    // drops device 0. Regression guard for the symmetric failure to the add path
    // — dropping the primary persists a record that suppresses usync forever.
    #[tokio::test]
    async fn patch_device_remove_keeps_primary() {
        let client = create_test_client().await;
        let user = "15551234567";
        setup_device_record(&client, user, &[0, 5]).await;

        client.patch_device_remove(user, 0).await;

        let record = client.device_registry_cache.get(user).await.unwrap();
        assert!(
            record.devices.iter().any(|d| d.device_id == 0),
            "remove for the primary must be ignored, got {:?}",
            record.devices
        );
        // The companion is untouched too — the remove is a full no-op.
        assert!(record.devices.iter().any(|d| d.device_id == 5));
    }

    #[tokio::test]
    async fn patch_device_add_preserves_unrelated_group_caches() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let client = create_test_client().await;
        setup_device_record(&client, "15551234567", &[0]).await;

        let group = "120363000000000002@g.us";
        let map =
            SenderKeyDeviceMap::from_db_rows(&[("99999999999:0@s.whatsapp.net".into(), true)]);
        client
            .sender_key_device_cache
            .get_or_init(group, async { std::sync::Arc::new(map) })
            .await;

        let elem = make_device_element(3, Some(5));
        client.patch_device_add("15551234567", &elem, None).await;

        let warm = client
            .sender_key_device_cache
            .get_or_init(group, async {
                panic!("cache should still be warm — no global invalidation")
            })
            .await;
        assert_eq!(warm.device_has_key("99999999999", 0), Some(true));
    }

    #[tokio::test]
    async fn patch_device_remove_preserves_unrelated_group_caches() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let client = create_test_client().await;
        setup_device_record(&client, "15551234567", &[0, 5]).await;

        let group = "120363000000000002@g.us";
        let map =
            SenderKeyDeviceMap::from_db_rows(&[("99999999999:0@s.whatsapp.net".into(), true)]);
        client
            .sender_key_device_cache
            .get_or_init(group, async { std::sync::Arc::new(map) })
            .await;

        client.patch_device_remove("15551234567", 5).await;

        let warm = client
            .sender_key_device_cache
            .get_or_init(group, async {
                panic!("cache should still be warm — no global invalidation")
            })
            .await;
        assert_eq!(warm.device_has_key("99999999999", 0), Some(true));
    }

    /// Forward secrecy: removing a participant who had `has_key=true` must
    /// drop the bot's own sender key and clear the group's tracker so the
    /// next send forces full SKDM redistribution.
    #[tokio::test]
    async fn participant_remove_rotates_sender_key_when_any_had_key() {
        use std::str::FromStr;
        use wacore::libsignal::protocol::SenderKeyRecord;
        use wacore::libsignal::store::sender_key_name::SenderKeyName;
        use wacore::types::jid::JidExt;

        let client = create_test_client().await;
        let group = "120363000000000001@g.us";
        let own_lid = Jid::from_str("193832511623409:13@lid").unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;

        let sk_name = SenderKeyName::from_parts(group, own_lid.to_protocol_address().as_str());
        client
            .signal_cache
            .put_sender_key(&sk_name, SenderKeyRecord::new_empty())
            .await;

        client
            .persistence_manager
            .set_sender_key_status(
                group,
                &[
                    ("271060335329480:0@lid", true),
                    ("77610646245392:0@lid", true),
                ],
            )
            .await
            .unwrap();

        client
            .rotate_sender_key_on_participant_remove(group, &["271060335329480"])
            .await;

        let device_snapshot = client.persistence_manager.get_device_snapshot();
        let key = client
            .signal_cache
            .get_sender_key(&sk_name, &*device_snapshot.backend)
            .await
            .unwrap();
        assert!(
            key.is_none(),
            "sender key must be deleted on remove rotation"
        );

        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert!(rows.is_empty(), "sender_key_devices must be cleared");
    }

    /// No rotation when removed participants never received an SKDM — there
    /// is nothing for them to decrypt forward, so don't pay the redistribute cost.
    #[tokio::test]
    async fn participant_remove_skips_rotation_when_none_had_key() {
        use std::str::FromStr;
        use wacore::libsignal::protocol::SenderKeyRecord;
        use wacore::libsignal::store::sender_key_name::SenderKeyName;
        use wacore::types::jid::JidExt;

        let client = create_test_client().await;
        let group = "120363000000000001@g.us";
        let own_lid = Jid::from_str("193832511623409:13@lid").unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;

        let sk_name = SenderKeyName::from_parts(group, own_lid.to_protocol_address().as_str());
        client
            .signal_cache
            .put_sender_key(&sk_name, SenderKeyRecord::new_empty())
            .await;

        client
            .persistence_manager
            .set_sender_key_status(group, &[("271060335329480:0@lid", false)])
            .await
            .unwrap();

        client
            .rotate_sender_key_on_participant_remove(group, &["271060335329480"])
            .await;

        let device_snapshot = client.persistence_manager.get_device_snapshot();
        let key = client
            .signal_cache
            .get_sender_key(&sk_name, &*device_snapshot.backend)
            .await
            .unwrap();
        assert!(
            key.is_some(),
            "sender key must survive when removed had no key"
        );
    }
}
