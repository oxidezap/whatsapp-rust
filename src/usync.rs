//! User device list synchronization.
//!
//! Device list IQ specification is defined in `wacore::iq::usync`.

use crate::client::Client;
use crate::request::IqError;
use log::{debug, warn};
use wacore::iq::usync::{DeviceListResponse, DeviceListSpec};
use wacore_binary::Jid;

/// An authoritative refresh retries when a newer registry mutation wins while
/// its IQ is in flight. Bound retries so a continuously changing account never
/// turns one send into an unbounded request loop.
const DEVICE_REFRESH_MAX_ATTEMPTS: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceListApplyMode {
    CachePreferred,
    StrictRefresh,
    GroupRefresh,
}

impl From<crate::cache::Freshness> for DeviceListApplyMode {
    fn from(freshness: crate::cache::Freshness) -> Self {
        match freshness {
            crate::cache::Freshness::CachePreferred => Self::CachePreferred,
            crate::cache::Freshness::Refresh => Self::StrictRefresh,
        }
    }
}

#[inline]
/// Every user a device-list response speaks for, in both namespaces, as the
/// member set a topology change is checked against.
fn device_response_users(
    response: &DeviceListResponse,
) -> crate::client::member_index::MemberIndex {
    crate::client::member_index::MemberIndex::from_users(
        response
            .device_lists
            .iter()
            .map(|device_list| device_list.user.user.as_str())
            .chain(
                response
                    .lid_mappings
                    .iter()
                    .flat_map(|mapping| [mapping.phone_number.as_str(), mapping.lid.as_str()]),
            ),
    )
}

pub use wacore::iq::usync::{
    UsyncAddressingMode, UsyncBotCommand, UsyncBotProfessionalType, UsyncBotProfileResult,
    UsyncBotPrompt, UsyncBusinessResult, UsyncContactResult, UsyncContext, UsyncDeviceListResult,
    UsyncDeviceResult, UsyncDeviceSyncHint, UsyncDevicesResult, UsyncDisappearingModeResult,
    UsyncFeature, UsyncFeatureResult, UsyncKeyIndexResult, UsyncMode, UsyncOutcome, UsyncProtocol,
    UsyncProtocolKind, UsyncProtocolResult, UsyncProtocolState, UsyncQuery, UsyncResponse,
    UsyncStatusResult, UsyncSubprotocolError, UsyncTextStatusResult, UsyncUser, UsyncUserResult,
    UsyncValidationError,
};

impl Client {
    /// Executes a typed USync query.
    ///
    /// The client generates the protocol `sid` independently from the IQ ID,
    /// matching WhatsApp Web. This neutral operation only returns decoded wire
    /// data; cache and persistence effects remain in specialized client APIs.
    pub async fn query_usync(&self, query: UsyncQuery) -> Result<UsyncResponse, IqError> {
        let sid = self.generate_request_id();
        let spec = wacore::iq::usync::UsyncQuerySpec::new(query, sid)
            .map_err(|error| IqError::EncodeError(error.into()))?;
        self.execute(spec).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.usync.get_user_devices", level = "debug", skip_all, fields(users = jids.len()), err(Debug)))]
    pub(crate) async fn get_user_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        let mut owned = Vec::with_capacity(jids.len());
        owned.extend(jids.iter().map(Jid::to_non_ad));
        self.get_user_devices_owned(owned).await
    }

    pub(crate) async fn get_user_devices_owned(
        &self,
        jids: Vec<Jid>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        // One batched registry pass: the cache answers per user, and every
        // user it cannot answer goes to the backend in ONE read. The
        // previous per-user fan-out (16 concurrent `get_devices`) bought
        // nothing, because the backend read is on the single write permit
        // and the permit re-serialized them. Order is irrelevant (phash
        // sorts, encrypt fan-out is order-agnostic). An unresolved user is
        // one with no usable record anywhere (WA Web always keeps device 0,
        // so an empty record counts as none) and falls through to the
        // network below.
        let jids: Vec<Jid> = jids.into_iter().map(Jid::into_non_ad).collect();
        let (mut all_devices, mut jids_to_fetch) = self.get_devices_from_registry_batch(jids).await;

        if !jids_to_fetch.is_empty() {
            wacore::types::jid::sort_dedup_by_user(&mut jids_to_fetch);
            debug!(
                "get_user_devices: Cache miss, fetching from network for {} unique users",
                jids_to_fetch.len()
            );
            all_devices.extend(self.fetch_user_devices(jids_to_fetch).await?);
        }

        Ok(all_devices)
    }

    pub(crate) async fn refresh_user_devices(
        &self,
        mut jids: Vec<Jid>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        for jid in &mut jids {
            jid.agent = 0;
            jid.device = 0;
        }
        wacore::types::jid::sort_dedup_by_user(&mut jids);
        self.fetch_user_devices_with_freshness(jids, crate::cache::Freshness::Refresh)
            .await
    }

    /// Publish successful group sync results, then read the complete local fanout.
    /// An omitted or invalid user keeps its previous registry record.
    pub(crate) async fn refresh_group_user_devices(
        &self,
        mut jids: Vec<Jid>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        for jid in &mut jids {
            jid.agent = 0;
            jid.device = 0;
        }
        wacore::types::jid::sort_dedup_by_user(&mut jids);
        if jids.is_empty() {
            return Ok(Vec::new());
        }
        for _ in 0..DEVICE_REFRESH_MAX_ATTEMPTS {
            let generation = self.device_topology.current();
            let response = self
                .execute(DeviceListSpec::new(
                    jids.clone(),
                    self.generate_request_id(),
                ))
                .await?;
            if let Some(devices) = self
                .try_process_group_device_list_response(&response, &jids, generation)
                .await?
            {
                return Ok(devices);
            }
        }
        anyhow::bail!("device registry kept changing while a group refresh was in flight")
    }

    async fn try_process_group_device_list_response(
        &self,
        response: &DeviceListResponse,
        users: &[Jid],
        topology_generation: u64,
    ) -> Result<Option<Vec<Jid>>, anyhow::Error> {
        let mapping_guard = self.lid_pn_cache.lock_mutation().await;
        let registry_guard = self.device_topology.lock_registry().await;
        if !self
            .device_topology
            .unchanged_for(topology_generation, &device_response_users(response))
        {
            return Ok(None);
        }
        self.learn_device_list_mappings_guarded(response, &mapping_guard)
            .await?;
        self.process_device_list_response_guarded(
            response,
            DeviceListApplyMode::GroupRefresh,
            &registry_guard,
        )
        .await?;
        // Read under the publication guards, including users omitted by the
        // response. Retrying a partial response must retain the original query.
        let (devices, _) = self.get_devices_from_registry_batch(users.to_vec()).await;
        Ok(Some(devices))
    }

    async fn fetch_user_devices(&self, jids: Vec<Jid>) -> Result<Vec<Jid>, anyhow::Error> {
        self.fetch_user_devices_with_freshness(jids, crate::cache::Freshness::CachePreferred)
            .await
    }

    async fn fetch_user_devices_with_freshness(
        &self,
        mut jids: Vec<Jid>,
        freshness: crate::cache::Freshness,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        if jids.is_empty() {
            return Ok(Vec::new());
        }
        if freshness == crate::cache::Freshness::CachePreferred {
            let sid = self.generate_request_id();
            let response = self.execute(DeviceListSpec::new(jids, sid)).await?;
            return self
                .process_device_list_response(&response, freshness)
                .await;
        }

        for attempt in 0..DEVICE_REFRESH_MAX_ATTEMPTS {
            let topology_generation = self.device_topology.current();
            let sid = self.generate_request_id();
            let response = self
                .execute(DeviceListSpec::new(jids, sid).require_complete_response())
                .await?;

            if let Some(devices) = self
                .try_process_refreshed_device_list_response(
                    &response,
                    freshness,
                    topology_generation,
                )
                .await?
            {
                return Ok(devices);
            }

            if attempt + 1 == DEVICE_REFRESH_MAX_ATTEMPTS {
                anyhow::bail!(
                    "device registry kept changing while an authoritative refresh was in flight"
                );
            }

            // The complete response contains every requested identity. Move its
            // canonical JIDs into the next attempt only on this rare race path;
            // the ordinary refresh performs no duplicate query-vector clone.
            jids = response
                .device_lists
                .into_iter()
                .map(|user| user.user)
                .collect();
        }

        unreachable!("bounded device refresh loop always returns")
    }

    async fn learn_device_list_mappings_guarded(
        &self,
        response: &DeviceListResponse,
        guard: &crate::lid_pn_cache::LidPnMutationGuard<'_>,
    ) -> Result<(), anyhow::Error> {
        // Learn LID↔PN mappings via the same batched, guarded learner routing_info
        // uses (one detached transaction, skipping already-durable pairs), so
        // per-mapping DB writes stay off the send's critical path. Client
        // construction always installs `self_weak`; failing the impossible
        // upgrade keeps mapping and registry publication atomic.
        //
        // Ordering: the old per-mapping path AWAITED migrate_signal_sessions_on_lid_discovery.
        // Detaching it can let a standalone usync (sync_own_device_list /
        // flush_pending_device_sync) that is the FIRST learner of a LID for a
        // contact with prior PN Signal state encrypt before the PN-wins migration
        // runs — but the per-address session_lock_for both take is the real
        // barrier (they can't interleave). The group-send path is unchanged:
        // routing_info already learns these same pairs detached upstream.
        if response.lid_mappings.is_empty() {
            return Ok(());
        }
        let client = self
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
            .ok_or_else(|| anyhow::anyhow!("client ownership unavailable during device sync"))?;
        let mappings: Vec<(String, String)> = response
            .lid_mappings
            .iter()
            .map(|mapping| (mapping.lid.to_string(), mapping.phone_number.to_string()))
            .collect();
        client
            .learn_lid_pn_mappings_batch_guarded(
                mappings,
                crate::lid_pn_cache::LearningSource::Usync,
                false,
                guard,
            )
            .await;
        Ok(())
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
    async fn process_device_list_response(
        &self,
        response: &DeviceListResponse,
        freshness: crate::cache::Freshness,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        // Keep this order stable: mapping writers never acquire the registry
        // guard while holding their lock, so mapping -> registry cannot cycle.
        let mapping_guard = self.lid_pn_cache.lock_mutation().await;
        let registry_guard = self.device_topology.lock_registry().await;
        self.learn_device_list_mappings_guarded(response, &mapping_guard)
            .await?;
        self.process_device_list_response_guarded(response, freshness.into(), &registry_guard)
            .await
    }

    async fn try_process_refreshed_device_list_response(
        &self,
        response: &DeviceListResponse,
        freshness: crate::cache::Freshness,
        topology_generation: u64,
    ) -> Result<Option<Vec<Jid>>, anyhow::Error> {
        // Serialize both mutation classes across the CAS and publication. The
        // response's own mappings are deliberately learned only after the CAS.
        let mapping_guard = self.lid_pn_cache.lock_mutation().await;
        let registry_guard = self.device_topology.lock_registry().await;
        if !self
            .device_topology
            .unchanged_for(topology_generation, &device_response_users(response))
        {
            return Ok(None);
        }
        self.learn_device_list_mappings_guarded(response, &mapping_guard)
            .await?;
        self.process_device_list_response_guarded(response, freshness.into(), &registry_guard)
            .await
            .map(Some)
    }

    async fn process_device_list_response_guarded(
        &self,
        response: &DeviceListResponse,
        mode: DeviceListApplyMode,
        guard: &crate::client::device_topology::DeviceRegistryMutationGuard<'_>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        let mut fetched_devices = Vec::new();
        if mode != DeviceListApplyMode::GroupRefresh {
            fetched_devices.reserve(response.device_lists.len());
        }
        let mut device_records: Vec<wacore::store::traits::DeviceListRecord> =
            Vec::with_capacity(response.device_lists.len());
        struct PendingIdentityReset<'a> {
            user: &'a Jid,
            previous: wacore::store::traits::DeviceListRecord,
            invalidate_registry: bool,
        }
        // Identity changes are rare, so this stays allocation-free for the
        // ordinary response. More importantly, deferring the destructive work
        // lets an authoritative refresh validate every user before any prior
        // Signal sessions or registry snapshot are discarded.
        let mut pending_identity_resets = Vec::new();
        let mut pending_removals = Vec::new();
        let mut retained_ids = std::collections::HashSet::new();

        // The existing records for the whole response in one alias-aware
        // (LID <-> PN) batch, so a response covering a large group does not
        // pay a serialized backend read per member before it can be applied.
        let users: Vec<&Jid> = response
            .device_lists
            .iter()
            .map(|user_list| &user_list.user)
            .collect();
        let mut existing_records = self.load_device_records_batch(&users).await.into_iter();

        for user_list in &response.device_lists {
            // Update device registry (single source of truth for device lists).
            // Preserve key_index values from existing records (set via account_sync)
            let mut existing_record = existing_records.next().flatten();

            // Decode key-index-list if present (WA Web: handleKeyIndexResult)
            let decoded_key_index = user_list
                .key_index_bytes
                .as_deref()
                .and_then(wacore::adv::decode_key_index_list);

            if mode == DeviceListApplyMode::GroupRefresh
                && user_list.key_index_bytes.is_some()
                && decoded_key_index.is_none()
            {
                continue;
            }

            // Check raw_id mismatch for identity change detection
            // TODO: also check advAccountType mismatch (see patch_device_add TODO)
            let mut raw_id = decoded_key_index.as_ref().map(|d| d.raw_id);
            let pending_identity_reset = if let Some(ref decoded) = decoded_key_index
                && let Some(ref existing) = existing_record
                && let Some(stored_raw_id) = existing.raw_id
                && stored_raw_id != decoded.raw_id
            {
                log::info!(
                    "raw_id mismatch for user {} in usync: stored={stored_raw_id}, received={}. Scheduling record reset.",
                    user_list.user.user,
                    decoded.raw_id
                );
                existing_record.take()
            } else {
                None
            };

            // Preserve raw_id from existing when usync didn't provide one
            // (no key-index-list). An identity change takes the old record out
            // above, so its raw_id and key indices cannot leak into the new one.
            if raw_id.is_none() {
                raw_id = existing_record
                    .as_ref()
                    .filter(|record| !record.devices.is_empty())
                    .and_then(|record| record.raw_id);
            }

            let mut devices: Vec<wacore::store::traits::DeviceInfo> = user_list
                .devices
                .iter()
                .map(|d| {
                    // Server-returned key_index takes priority over cached
                    let key_index = d.key_index.or_else(|| {
                        // Accounts ordinarily have only a handful of companion
                        // devices; a short scan avoids allocating a HashMap for
                        // every user in a large fanout response.
                        existing_record.as_ref().and_then(|record| {
                            record
                                .devices
                                .iter()
                                .find(|cached| cached.device_id() == d.device)
                                .and_then(|cached| cached.key_index())
                        })
                    });
                    wacore::store::traits::DeviceInfo::new(d.device, key_index)
                        .with_hosting(d.is_hosted)
                })
                .collect();

            // Apply valid_indexes filtering if key-index-list was decoded
            if let Some(ref decoded) = decoded_key_index {
                wacore::adv::retain_devices_by_key_index(&mut devices, decoded);
            }

            // An empty device list is never valid — WA Web always keeps the primary
            // (device 0), so a usync returning no devices for a user is transient or
            // corrupt. Persisting it would clobber a good cached record, or store an
            // empty one that get_user_devices then re-fetches on every send.
            if devices.is_empty() {
                if mode == DeviceListApplyMode::StrictRefresh {
                    anyhow::bail!(
                        "device-list refresh left no valid devices for {}",
                        user_list.user
                    );
                }
                if mode == DeviceListApplyMode::GroupRefresh {
                    continue;
                }
                if let Some(previous) = pending_identity_reset {
                    pending_identity_resets.push(PendingIdentityReset {
                        user: &user_list.user,
                        previous,
                        // No replacement record will be written. Keeping the old
                        // snapshot would pair a new identity with stale devices
                        // and suppress the next authoritative network fetch.
                        invalidate_registry: true,
                    });
                }
                continue;
            }

            if let Some(previous) = existing_record.as_ref() {
                retained_ids.clear();
                retained_ids.extend(devices.iter().map(|device| device.device_id()));
                for device in &previous.devices {
                    // The same response can reuse an ID whose old key index
                    // was revoked. Its replacement must not inherit a warm mark.
                    if !retained_ids.contains(&device.device_id())
                        || (device.device_id() != 0
                            && decoded_key_index.as_ref().is_some_and(|decoded| {
                                !wacore::adv::is_key_index_valid(device.key_index(), decoded)
                            }))
                    {
                        pending_removals.push((&user_list.user, device.device_id()));
                    }
                }
            }

            if let Some(previous) = pending_identity_reset {
                pending_identity_resets.push(PendingIdentityReset {
                    user: &user_list.user,
                    previous,
                    invalidate_registry: false,
                });
            }

            // Convert filtered DeviceInfo list back to JIDs for return
            if mode != DeviceListApplyMode::GroupRefresh {
                let user_jid = &user_list.user;
                for d in &devices {
                    fetched_devices
                        .push(user_jid.with_device_hosting(d.device_id(), d.is_hosted()));
                }
            }

            device_records.push(wacore::store::traits::DeviceListRecord {
                user: std::sync::Arc::from(user_list.user.user.as_str()),
                devices: devices.into_boxed_slice(),
                timestamp: wacore::time::now_secs(),
                phash: user_list.phash.clone().map(Box::<str>::from),
                raw_id,
            });
        }

        // Publish replacements before destructive cleanup: if the backend
        // write fails, sender-key rows and sessions are still intact and a
        // retry recomputes the same removals. Everything here runs under the
        // mapping and registry guards, so no send observes the intermediate
        // state.
        //
        // One batched backend write for the whole usync response — for
        // large groups this collapses N spawn_blocking SQLite hops into
        // a single transaction, which dominated the per-send wall-clock.
        self.update_device_lists_guarded(device_records, guard)
            .await?;
        for (user, device_id) in pending_removals {
            if let Err(e) = self
                .delete_sender_key_rows_for_device(&user.user, device_id)
                .await
            {
                // The replacement records are already durable; bias the live
                // cache toward redistribution so a removed-then-readded device
                // cannot ride a stale warm mark past the SKDM it needs.
                self.sender_key_device_cache
                    .invalidate_entries_for_device(&user.user, device_id)
                    .await;
                return Err(e.into());
            }
        }
        for reset in pending_identity_resets {
            for device in &reset.previous.devices {
                if device.device_id() != 0
                    && let Err(e) = self
                        .delete_sender_key_rows_for_device(&reset.user.user, device.device_id())
                        .await
                {
                    self.sender_key_device_cache
                        .invalidate_entries_for_device(&reset.user.user, device.device_id())
                        .await;
                    return Err(e.into());
                }
            }
            self.clear_device_record(
                &reset.user.user,
                reset.user.server.as_str(),
                &reset.previous,
            )
            .await;
            if reset.invalidate_registry {
                self.invalidate_device_cache_guarded(&reset.user.user, guard)
                    .await;
            }
        }

        Ok(fetched_devices)
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
            if let Some(record) = self.load_device_record_for_jid(&bare).await
                && let Some(phash) = record.phash
            {
                hashes.insert(bare.clone(), (String::from(phash), record.timestamp));
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
        let devices = self
            .process_device_list_response(&response, crate::cache::Freshness::CachePreferred)
            .await?;
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
        let pending = self.pending_device_sync.take_all();
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
                    self.pending_device_sync.add(jid);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Freshness;
    use crate::test_utils::create_test_client;
    use wacore::libsignal::protocol::{ProtocolAddress, SessionRecord};
    use wacore::store::traits::{DeviceInfo, DeviceListRecord};
    use wacore::types::jid::JidExt;

    fn signed_key_index_bytes(valid_indexes: Vec<u32>, current_index: u32) -> Vec<u8> {
        let key_index = waproto::whatsapp::ADVKeyIndexList {
            raw_id: Some(1),
            timestamp: Some(1_700_000_000),
            current_index: Some(current_index),
            valid_indexes,
            ..Default::default()
        };
        let signed = waproto::whatsapp::ADVSignedKeyIndexList {
            details: Some(waproto::codec::adv_key_index_list_to_vec(&key_index)),
            ..Default::default()
        };
        waproto::codec::adv_signed_key_index_list_to_vec(&signed)
    }

    async fn seed_fresh_session(client: &Client, jid: &Jid) -> ProtocolAddress {
        let address = jid.to_protocol_address();
        client
            .signal_cache
            .put_session(&address, SessionRecord::new_fresh())
            .await;
        address
    }

    async fn has_session(client: &Client, address: &ProtocolAddress) -> bool {
        let snapshot = client.persistence_manager.get_device_snapshot();
        client
            .signal_cache
            .has_session(address, &*snapshot.backend)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn group_refresh_partial_success_rebuilds_cached_unresolved_users() {
        use wacore::iq::spec::IqSpec;
        use wacore_binary::builder::NodeBuilder;

        let client = create_test_client().await;
        let users = vec![
            Jid::pn("12025550121"),
            Jid::pn("12025550122"),
            Jid::pn("12025550127"),
        ];
        for user in &users {
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: 1,
                    phash: None,
                    raw_id: Some(2),
                })
                .await
                .unwrap();
        }
        let wire = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", users[0].to_string())
                            .children([NodeBuilder::new("devices")
                                .children([NodeBuilder::new("device-list")
                                    .children([NodeBuilder::new("device").attr("id", "0").build()])
                                    .build()])
                                .build()])
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", users[1].to_string())
                            .children([NodeBuilder::new("devices")
                                .children([NodeBuilder::new("error").attr("code", "500").build()])
                                .build()])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();
        let response = DeviceListSpec::new(users.clone(), "group-partial")
            .parse_response(&wire.as_node_ref())
            .unwrap();
        assert!(
            DeviceListSpec::new(users.clone(), "dm-strict")
                .require_complete_response()
                .parse_response(&wire.as_node_ref())
                .is_err()
        );
        let devices = client
            .try_process_group_device_list_response(
                &response,
                &users,
                client.device_topology.current(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(devices.len(), 5);
        assert!(!devices.contains(&users[0].with_device(7)));
        assert!(devices.contains(&users[1].with_device(7)));
        assert!(devices.contains(&users[2].with_device(7)));
    }

    #[tokio::test]
    async fn group_refresh_preserves_malformed_and_filtered_identity_records() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        for (bytes, devices) in [
            (Some(vec![0xff]), vec![UsyncDevice::new(7, Some(3))]),
            (
                Some(signed_key_index_bytes(Vec::new(), 10)),
                vec![UsyncDevice::new(7, Some(3))],
            ),
            (None, Vec::new()),
        ] {
            let client = create_test_client().await;
            let user = Jid::pn("12025550123");
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: 1,
                    phash: Some("old".into()),
                    raw_id: Some(2),
                })
                .await
                .unwrap();
            let session = seed_fresh_session(&client, &user.with_device(7)).await;
            let response = DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices,
                    phash: None,
                    key_index_bytes: bytes,
                }],
                lid_mappings: Vec::new(),
            };
            let devices = client
                .try_process_group_device_list_response(
                    &response,
                    std::slice::from_ref(&user),
                    client.device_topology.current(),
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(devices.len(), 2);
            let record = client.load_device_record_for_jid(&user).await.unwrap();
            assert_eq!(record.raw_id, Some(2));
            assert_eq!(record.phash.as_deref(), Some("old"));
            assert!(has_session(&client, &session).await);
        }
    }

    #[tokio::test]
    async fn group_refresh_rejects_response_after_concurrent_notification() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550124");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: 1,
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        let generation = client.device_topology.current();
        client
            .patch_device_add(
                &user.user,
                &wacore::stanza::devices::DeviceElement {
                    jid: user.with_device(7),
                    key_index: None,
                    lid: None,
                },
                None,
            )
            .await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        assert!(
            client
                .try_process_group_device_list_response(
                    &response,
                    std::slice::from_ref(&user),
                    generation,
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            client
                .get_devices_from_registry(&user)
                .await
                .unwrap()
                .contains(&user.with_device(7))
        );
        let omitted = DeviceListResponse {
            device_lists: Vec::new(),
            lid_mappings: Vec::new(),
        };
        let devices = client
            .try_process_group_device_list_response(
                &omitted,
                std::slice::from_ref(&user),
                generation,
            )
            .await
            .unwrap()
            .unwrap();
        assert!(devices.contains(&user.with_device(7)));
    }

    #[tokio::test]
    async fn group_refresh_removed_then_reused_device_id_is_cold() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550125");
        let group = "120363000000000125@g.us";
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: 1,
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        let rows = vec![
            (user.to_string(), true),
            (user.with_device(7).to_string(), true),
        ];
        client
            .persistence_manager
            .set_sender_key_status(
                group,
                &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
            )
            .await
            .unwrap();
        client
            .sender_key_device_cache
            .get_or_init(group, async {
                std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
            })
            .await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        client
            .try_process_group_device_list_response(
                &response,
                std::slice::from_ref(&user),
                client.device_topology.current(),
            )
            .await
            .unwrap()
            .unwrap();
        client
            .patch_device_add(
                &user.user,
                &wacore::stanza::devices::DeviceElement {
                    jid: user.with_device(7),
                    key_index: Some(4),
                    lid: None,
                },
                None,
            )
            .await;
        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert_eq!(rows, vec![(user.to_string(), true)]);
        let map = client
            .sender_key_device_cache
            .get_or_init(group, async {
                std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
            })
            .await;
        assert_eq!(map.device_has_key(&user.user, 7), None);
        assert_eq!(map.device_has_key(&user.user, 0), Some(true));
        assert_eq!(
            client
                .load_device_record_for_jid(&user)
                .await
                .unwrap()
                .devices
                .iter()
                .find(|device| device.device_id() == 7)
                .unwrap()
                .key_index(),
            Some(4)
        );
    }

    #[tokio::test]
    async fn group_refresh_same_response_reused_id_checks_previous_key_index() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        for previous_index_valid in [false, true] {
            let client = create_test_client().await;
            let user = Jid::pn("12025550128");
            let group = "120363000000000128@g.us";
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: 1,
                    phash: None,
                    raw_id: Some(1),
                })
                .await
                .unwrap();
            let rows = vec![
                (user.to_string(), true),
                (user.with_device(7).to_string(), true),
            ];
            client
                .persistence_manager
                .set_sender_key_status(
                    group,
                    &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
                )
                .await
                .unwrap();
            let warm = client
                .sender_key_device_cache
                .get_or_init(group, async {
                    std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
                })
                .await;
            assert!(warm.device_and_primary_warm(&user.user, 7));

            let valid_indexes = if previous_index_valid {
                vec![3, 4]
            } else {
                vec![4]
            };
            let response = DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(4))],
                    phash: None,
                    key_index_bytes: Some(signed_key_index_bytes(valid_indexes, 4)),
                }],
                lid_mappings: Vec::new(),
            };
            let devices = client
                .try_process_group_device_list_response(
                    &response,
                    std::slice::from_ref(&user),
                    client.device_topology.current(),
                )
                .await
                .unwrap()
                .unwrap();
            assert!(devices.contains(&user.with_device(7)));
            let record = client.load_device_record_for_jid(&user).await.unwrap();
            assert_eq!(record.raw_id, Some(1));
            assert_eq!(
                record
                    .devices
                    .iter()
                    .find(|device| device.device_id() == 7)
                    .unwrap()
                    .key_index(),
                Some(4)
            );

            let rows = client
                .persistence_manager
                .get_sender_key_devices(group)
                .await
                .unwrap();
            assert_eq!(
                rows.iter()
                    .any(|(jid, has_key)| jid == &user.with_device(7).to_string() && *has_key),
                previous_index_valid
            );
            let map = client
                .sender_key_device_cache
                .get_or_init(group, async {
                    std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
                })
                .await;
            assert_eq!(
                map.device_has_key(&user.user, 7),
                previous_index_valid.then_some(true)
            );
            assert_eq!(map.device_has_key(&user.user, 0), Some(true));
        }
    }

    /// A failed registry write must not leave sender-key rows and sessions
    /// deleted for a device list that never landed: the cleanup runs after
    /// the write, so a write that never happened leaves everything intact.
    #[tokio::test]
    async fn group_refresh_write_failure_keeps_tracking_and_sessions() {
        use std::sync::atomic::Ordering;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550129");
        let group = "120363000000000129@g.us";
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: 1,
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        let rows = [
            (user.to_string(), true),
            (user.with_device(7).to_string(), true),
        ];
        client
            .persistence_manager
            .set_sender_key_status(
                group,
                &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
            )
            .await
            .unwrap();
        let session = seed_fresh_session(&client, &user.with_device(7)).await;
        // The replacement drops device 7, so the old order would delete its
        // sender-key rows and session before attempting the write.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        client
            .fail_next_device_list_write
            .store(true, Ordering::SeqCst);
        client
            .try_process_group_device_list_response(
                &response,
                std::slice::from_ref(&user),
                client.device_topology.current(),
            )
            .await
            .expect_err("the injected write failure must surface");
        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert!(
            rows.iter()
                .any(|(jid, has_key)| jid == &user.with_device(7).to_string() && *has_key),
            "a write that never happened must not delete sender-key tracking: {rows:?}"
        );
        assert!(
            has_session(&client, &session).await,
            "a write that never happened must not delete sessions"
        );
        assert!(
            client
                .load_device_record_for_jid(&user)
                .await
                .unwrap()
                .devices
                .iter()
                .any(|device| device.device_id() == 7),
            "the previous device list must survive a failed write"
        );
    }

    #[tokio::test]
    async fn test_device_registry_hit_resolves_devices() {
        let client = create_test_client().await;

        let user_jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();

        // Insert a device record into the registry (simulates prior usync/notification)
        let record = DeviceListRecord {
            user: "1234567890".into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(10))].into(),
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
    async fn refresh_bypasses_a_warm_registry_without_clearing_it_first() {
        let client = create_test_client().await;
        let user: Jid = "12025550102@s.whatsapp.net".parse().unwrap();
        client
            .update_device_list(DeviceListRecord {
                user: "12025550102".into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(8, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        let cached = client
            .get_user_devices(std::slice::from_ref(&user))
            .await
            .unwrap();
        assert_eq!(
            cached.len(),
            2,
            "cache-preferred must use the warm snapshot"
        );

        let refresh = client.refresh_user_devices(vec![user.clone()]).await;
        assert!(
            refresh.is_err(),
            "the offline fixture proves refresh consulted the source"
        );

        let preserved = client
            .get_devices_from_registry(&user)
            .await
            .expect("a failed refresh must leave the previous snapshot readable");
        assert_eq!(preserved.len(), 2);
        assert!(preserved.iter().any(|device| device.device == 8));
    }

    #[tokio::test]
    async fn test_device_registry_hit_for_lid_jid() {
        let client = create_test_client().await;

        let lid_jid: Jid = "100000012345678@lid".parse().unwrap();

        let record = DeviceListRecord {
            user: "100000012345678".into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(39, Some(25))].into(),
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
            devices: [DeviceInfo::new(5, None)].into(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        // Evict from registry cache to force DB path
        client
            .device_registry_cache
            .raw_invalidate_for_tests("9876543210")
            .await;
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
            devices: Box::default(),
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
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("2:oldB".into()),
                raw_id: None,
            })
            .await
            .unwrap();

        // Response only contains user A — B is omitted (unchanged).
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: "1111111111@s.whatsapp.net".parse().unwrap(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: Some("2:a".into()),
                key_index_bytes: None,
            }],
            lid_mappings: vec![],
        };

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn process_response_preserves_hosted_device_addressing() {
        use wacore::iq::usync::DeviceListResponse;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("1111111111");
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![
                    UsyncDevice::new(0, None),
                    UsyncDevice::new(7, Some(3)).with_hosting(true),
                ],
                phash: Some("2:hosted".into()),
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
        assert!(
            fetched
                .iter()
                .any(|jid| jid.device == 7 && jid.server == wacore_binary::Server::Hosted)
        );

        let cached = client
            .get_devices_from_registry(&user)
            .await
            .expect("processed devices should be cached");
        assert!(
            cached
                .iter()
                .any(|jid| jid.device == 7 && jid.server == wacore_binary::Server::Hosted)
        );
    }

    #[tokio::test]
    async fn stale_refresh_does_not_overwrite_a_newer_device_notification() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550102");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("1:before".into()),
                raw_id: None,
            })
            .await
            .unwrap();

        let refresh_started_at = client.device_topology.current();
        let stale_response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: Some("1:stale".into()),
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };

        client
            .patch_device_add(
                &user.user,
                &wacore::stanza::devices::DeviceElement {
                    jid: user.with_device(7),
                    key_index: None,
                    lid: None,
                },
                None,
            )
            .await;

        let published = client
            .try_process_refreshed_device_list_response(
                &stale_response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();
        assert!(published.is_none(), "the stale response must be retried");

        let retained = client
            .get_devices_from_registry(&user)
            .await
            .expect("notification snapshot must remain available");
        assert!(retained.iter().any(|device| device.device == 7));
    }

    #[tokio::test]
    async fn stale_refresh_does_not_overwrite_a_newer_lid_mapping() {
        use wacore::usync::UsyncLidMapping;

        let client = create_test_client().await;
        let phone = "12025550110";
        let current_lid = "100000000000110";
        let stale_lid = "100000000000111";
        let refresh_started_at = client.device_topology.current();

        client
            .add_lid_pn_mapping(
                current_lid,
                phone,
                crate::lid_pn_cache::LearningSource::PeerPnMessage,
            )
            .await
            .unwrap();

        let stale_response = DeviceListResponse {
            device_lists: Vec::new(),
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.into(),
                lid: stale_lid.into(),
            }],
        };
        let published = client
            .try_process_refreshed_device_list_response(
                &stale_response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();

        assert!(published.is_none(), "the stale response must be retried");
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(current_lid),
            "the mapping learned after the refresh started must survive"
        );
    }

    #[tokio::test]
    async fn refresh_commits_its_own_lid_mapping_after_the_cas() {
        use wacore::usync::UsyncLidMapping;

        let client = create_test_client().await;
        let phone = "12025550112";
        let lid = "100000000000112";
        let refresh_started_at = client.device_topology.current();
        let response = DeviceListResponse {
            device_lists: Vec::new(),
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.into(),
                lid: lid.into(),
            }],
        };

        let published = client
            .try_process_refreshed_device_list_response(
                &response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();

        assert!(
            published.is_some(),
            "the response must not conflict with itself"
        );
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid)
        );
    }

    #[tokio::test]
    async fn unrelated_registry_change_does_not_restart_a_refresh() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let refreshed = Jid::pn("12025550104");
        let refresh_started_at = client.device_topology.current();
        client
            .update_device_list(DeviceListRecord {
                user: "12025550105".into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: refreshed,
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let published = client
            .try_process_refreshed_device_list_response(
                &response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();
        assert!(published.is_some());
    }

    #[tokio::test]
    async fn unrelated_mapping_change_does_not_restart_a_refresh() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let refreshed = Jid::pn("12025550113");
        let refresh_started_at = client.device_topology.current();
        client
            .add_lid_pn_mapping(
                "100000000000114",
                "12025550114",
                crate::lid_pn_cache::LearningSource::PeerPnMessage,
            )
            .await
            .unwrap();

        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: refreshed,
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let published = client
            .try_process_refreshed_device_list_response(
                &response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();

        assert!(published.is_some());
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
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(4, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("3:old".into()),
                raw_id: None,
            })
            .await
            .unwrap();

        // The same user comes back from usync with no devices.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: "3333333333@s.whatsapp.net".parse().unwrap(),
                devices: vec![],
                phash: Some("3:empty".into()),
                key_index_bytes: None,
            }],
            lid_mappings: vec![],
        };

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
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

    #[tokio::test]
    async fn refresh_rejects_a_device_list_emptied_by_key_index_filtering() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("4444444444");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("2:previous".into()),
                raw_id: Some(1),
            })
            .await
            .unwrap();

        // The wire response is non-empty, but its only companion is outside the
        // signed key-index set. This exercises the post-projection completeness
        // check rather than the raw USync response check.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(7, Some(3))],
                phash: Some("2:incomplete".into()),
                key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
            }],
            lid_mappings: Vec::new(),
        };

        let error = client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .expect_err("an authoritative refresh must not return an empty projection");
        assert!(error.to_string().contains("no valid devices"));

        let preserved = client
            .get_devices_from_registry(&user)
            .await
            .expect("a rejected refresh must preserve the previous snapshot");
        assert_eq!(preserved.len(), 2);
        assert!(preserved.iter().any(|device| device.device == 7));
    }

    #[tokio::test]
    async fn rejected_refresh_defers_identity_cleanup_for_every_user() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let identity_changed = Jid::pn("4444444451");
        let invalid = Jid::pn("4444444452");

        for (user, raw_id) in [(&identity_changed, 2), (&invalid, 1)] {
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: wacore::time::now_secs(),
                    phash: Some("2:previous".into()),
                    raw_id: Some(raw_id),
                })
                .await
                .unwrap();
        }

        let previous_session = seed_fresh_session(&client, &identity_changed.with_device(7)).await;
        let response = DeviceListResponse {
            device_lists: vec![
                UserDeviceList {
                    user: identity_changed.clone(),
                    // A primary device survives key-index filtering, so this
                    // first user schedules a valid identity replacement.
                    devices: vec![UsyncDevice::new(0, None)],
                    phash: Some("1:changed".into()),
                    key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
                },
                UserDeviceList {
                    user: invalid,
                    // The second user makes the authoritative response invalid
                    // only after key-index projection.
                    devices: vec![UsyncDevice::new(7, Some(3))],
                    phash: Some("1:invalid".into()),
                    key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
                },
            ],
            lid_mappings: Vec::new(),
        };

        client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .expect_err("the second user must reject the whole authoritative refresh");

        assert!(
            has_session(&client, &previous_session).await,
            "validation failure must not partially clear an earlier user's sessions"
        );
        let preserved = client
            .get_devices_from_registry(&identity_changed)
            .await
            .expect("validation failure must preserve the earlier registry snapshot");
        assert!(preserved.iter().any(|device| device.device == 7));
    }

    #[tokio::test]
    async fn filtered_identity_change_invalidates_the_stale_registry() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("4444444453");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("2:previous".into()),
                raw_id: Some(2),
            })
            .await
            .unwrap();
        let previous_session = seed_fresh_session(&client, &user.with_device(7)).await;

        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(7, Some(3))],
                phash: Some("1:changed".into()),
                key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
            }],
            lid_mappings: Vec::new(),
        };

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
        assert!(fetched.is_empty());
        assert!(
            !has_session(&client, &previous_session).await,
            "an accepted identity change must clear sessions from the old identity"
        );
        assert!(
            client.get_devices_from_registry(&user).await.is_none(),
            "without a replacement snapshot, the stale registry must be invalidated"
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

        client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();

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
