use crate::cache::Freshness;
use crate::client::Client;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use wacore::types::message::AddressingMode;
use wacore_binary::Jid;

type Refresh = Arc<async_lock::Mutex<Option<bool>>>;
type MessageRepairs = HashMap<(u64, Jid, String), Arc<futures::future::AbortHandle>>;

// Web's filterDeviceWithChangedIdentity checks per-primary identity row ranges
// after ensureE2ESessions. Two journal tokens keep original sends constant-sized;
// only the cold repair path resolves aliases and filters changed accounts.
#[derive(Clone, Copy, Default)]
pub(crate) struct GroupIdentitySnapshot {
    signal: Option<u64>,
    aliases: Option<u64>,
}

impl GroupIdentitySnapshot {
    pub(crate) fn capture(client: &Client) -> Self {
        Self {
            signal: client.signal_cache.identity_continuity.snapshot(),
            aliases: client.lid_pn_cache.identity_continuity.snapshot(),
        }
    }

    fn has_history(&self, client: &Client) -> bool {
        client
            .signal_cache
            .identity_continuity
            .unchanged_for(self.signal, [])
            && client
                .lid_pn_cache
                .identity_continuity
                .unchanged_for(self.aliases, [])
    }

    async fn unchanged_for(&self, client: &Client, user: &str) -> bool {
        let cache = &client.lid_pn_cache;
        let pn = cache.get_phone_number(user).await;
        let lid = cache.get_current_lid(user).await;
        // A stale reverse alias is not evidence that its old and current
        // accounts are interchangeable. Reject just this ambiguous account.
        if let Some(pn) = &pn
            && cache.get_current_lid(pn).await.as_deref() != Some(user)
        {
            return false;
        }
        if let Some(lid) = &lid
            && cache.get_phone_number(lid).await.as_deref() != Some(user)
        {
            return false;
        }
        let users = [Some(user), pn.as_deref(), lid.as_deref()];
        client
            .signal_cache
            .identity_continuity
            .unchanged_for(self.signal, users.into_iter().flatten())
            && cache
                .identity_continuity
                .unchanged_for(self.aliases, users.into_iter().flatten())
    }
}

#[derive(Clone)]
pub(crate) struct GroupSendSnapshot {
    pub(crate) connection_generation: u64,
    pub(crate) identity: GroupIdentitySnapshot,
    pub(crate) devices: Arc<wacore::send::ResolvedGroupDevices>,
    pub(crate) message_secret: Option<[u8; 32]>,
    pub(crate) addressing_mode: AddressingMode,
}

#[derive(Default)]
pub(crate) struct GroupRepair {
    entries: Mutex<HashMap<(u64, Jid), Refresh>>,
    messages: Arc<Mutex<MessageRepairs>>,
}

impl GroupRepair {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(super) fn refresh(&self, generation: u64, group: &Jid) -> Refresh {
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        Arc::clone(entries.entry((generation, group.clone())).or_default())
    }

    fn remove(&self, generation: u64, group: &Jid, refresh: &Refresh) {
        let mut entries = self.entries.lock().unwrap_or_else(|p| p.into_inner());
        let key = (generation, group.clone());
        if entries
            .get(&key)
            .is_some_and(|entry| Arc::ptr_eq(entry, refresh))
        {
            entries.remove(&key);
            if entries.is_empty() {
                *entries = HashMap::new();
            }
        }
    }

    pub(crate) fn clear(&self) {
        *self.entries.lock().unwrap_or_else(|p| p.into_inner()) = HashMap::new();
        let messages =
            std::mem::take(&mut *self.messages.lock().unwrap_or_else(|p| p.into_inner()));
        for (_, handle) in messages {
            handle.abort();
        }
    }

    pub(crate) fn start_message(
        &self,
        generation: u64,
        group: &Jid,
        id: &str,
    ) -> (
        futures::future::AbortRegistration,
        Arc<futures::future::AbortHandle>,
    ) {
        let (handle, registration) = futures::future::AbortHandle::new_pair();
        let handle = Arc::new(handle);
        let mut messages = self.messages.lock().unwrap_or_else(|p| p.into_inner());
        let key = (generation, group.clone(), id.to_owned());
        if let Some(previous) = messages.get(&key) {
            if previous.is_aborted() {
                handle.abort();
                return (registration, handle);
            }
            previous.abort();
        }
        messages.insert(key, Arc::clone(&handle));
        (registration, handle)
    }

    pub(crate) fn finish_message(
        &self,
        generation: u64,
        group: &Jid,
        id: &str,
        reservation: &Arc<futures::future::AbortHandle>,
    ) {
        let mut messages = self.messages.lock().unwrap_or_else(|p| p.into_inner());
        let key = (generation, group.clone(), id.to_owned());
        if messages
            .get(&key)
            .is_some_and(|handle| Arc::ptr_eq(handle, reservation) && !handle.is_aborted())
        {
            messages.remove(&key);
        }
        if messages.is_empty() {
            *messages = HashMap::new();
        }
    }

    pub(crate) fn revoke_message(
        &self,
        generation: u64,
        group: &Jid,
        id: &str,
        runtime: &Arc<dyn wacore::runtime::Runtime>,
        shutdown: wacore::runtime::ShutdownSignal,
    ) {
        let key = (generation, group.clone(), id.to_owned());
        let handle = {
            let mut messages = self.messages.lock().unwrap_or_else(|p| p.into_inner());
            let handle = messages
                .entry(key.clone())
                .or_insert_with(|| Arc::new(futures::future::AbortHandle::new_pair().0));
            if handle.is_aborted() {
                return;
            }
            handle.abort();
            Arc::clone(handle)
        };
        // Cover an ack already removed from its waiter but not yet scheduled.
        // The tombstone outlives the phash waiter's full sweep window.
        let messages = Arc::clone(&self.messages);
        let clock = Arc::clone(runtime);
        runtime.spawn_detached(Box::pin(async move {
            let sleep = clock.sleep(super::GROUP_DEVICE_RESYNC_COOLDOWN);
            let cancelled = wacore::runtime::wait_for_shutdown(&shutdown);
            futures::pin_mut!(sleep, cancelled);
            let _ = futures::future::select(cancelled, sleep).await;
            let mut messages = messages.lock().unwrap_or_else(|p| p.into_inner());
            if messages
                .get(&key)
                .is_some_and(|current| Arc::ptr_eq(current, &handle))
            {
                messages.remove(&key);
                if messages.is_empty() {
                    *messages = HashMap::new();
                }
            }
        }));
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    pub(crate) fn retained_message_count(&self) -> usize {
        self.messages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len()
    }

    #[cfg(test)]
    pub(crate) fn active_message_count(&self) -> usize {
        self.messages
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .values()
            .filter(|handle| !handle.is_aborted())
            .count()
    }
}

fn group_delta(old: &[Jid], fresh: &[Jid]) -> Vec<Jid> {
    let users: std::collections::HashSet<_> = old
        .iter()
        .map(|jid| (jid.user.as_str(), jid.server, jid.integrator))
        .collect();
    let devices: std::collections::HashSet<_> = old.iter().collect();
    fresh
        .iter()
        .filter(|jid| {
            users.contains(&(jid.user.as_str(), jid.server, jid.integrator))
                && !devices.contains(jid)
        })
        .cloned()
        .collect()
}

impl Client {
    pub(crate) fn cancel_group_message_repair(&self, group: &Jid, message_id: &str) {
        self.response_waiters_guard().remove(message_id);
        self.pending_group_device_resync.revoke_message(
            self.connection_generation.load(Ordering::Acquire),
            group,
            message_id,
            &self.runtime,
            self.connection_shutdown_signal(),
        );
    }

    pub(super) async fn refresh_group_for_repair(
        self: &Arc<Self>,
        group: &Jid,
        generation: u64,
    ) -> bool {
        if self.connection_generation.load(Ordering::Acquire) != generation {
            return false;
        }
        let refresh = self.pending_group_device_resync.refresh(generation, group);
        let mut result = refresh.lock_arc().await;
        if self.connection_generation.load(Ordering::Acquire) != generation {
            self.pending_group_device_resync
                .remove(generation, group, &refresh);
            return false;
        }
        if let Some(success) = *result {
            return success;
        }
        let client = Arc::clone(self);
        let group = group.clone();
        let shutdown = self.connection_shutdown_signal();
        let task_refresh = Arc::clone(&refresh);
        // A message expiring or being revoked must not cancel another message's refresh.
        self.runtime.spawn_detached(Box::pin(async move {
            let work = async {
                if client.connection_generation.load(Ordering::Acquire) != generation {
                    anyhow::bail!("group refresh connection retired");
                }
                let info = client
                    .groups()
                    .query_info_with_freshness(&group, Freshness::Refresh)
                    .await?;
                let snapshot = client.persistence_manager.get_device_snapshot();
                let own = match info.addressing_mode {
                    AddressingMode::Lid => snapshot.lid.as_ref(),
                    AddressingMode::Pn => snapshot.pn.as_ref(),
                }
                .ok_or(crate::client::ClientError::NotLoggedIn)?;
                client
                    .resolve_group_devices_uncached(&info, own, Freshness::Refresh)
                    .await?;
                anyhow::Ok(())
            };
            let cancelled = wacore::runtime::wait_for_shutdown(&shutdown);
            futures::pin_mut!(work, cancelled);
            let success = match futures::future::select(cancelled, work).await {
                futures::future::Either::Right((outcome, _)) => {
                    if let Err(error) = &outcome {
                        log::warn!("Group device refresh failed: {error}");
                    }
                    outcome.is_ok()
                        && client.connection_generation.load(Ordering::Acquire) == generation
                }
                futures::future::Either::Left(_) => false,
            };
            *result = Some(success);
            drop(result);
            let cooldown = if success {
                super::GROUP_DEVICE_RESYNC_COOLDOWN
            } else {
                super::GROUP_DEVICE_RESYNC_RETRY_COOLDOWN
            };
            let sleep = client.runtime.sleep(cooldown);
            let cancelled = wacore::runtime::wait_for_shutdown(&shutdown);
            futures::pin_mut!(sleep, cancelled);
            let _ = futures::future::select(cancelled, sleep).await;
            client
                .pending_group_device_resync
                .remove(generation, &group, &task_refresh);
        }));
        refresh.lock().await.unwrap_or(false)
    }

    pub(crate) async fn repair_group_message(
        self: &Arc<Self>,
        group: &Jid,
        message_id: &str,
        sent: GroupSendSnapshot,
        generation: u64,
    ) -> anyhow::Result<()> {
        let socket = self.get_noise_socket()?;
        if !self.refresh_group_for_repair(group, generation).await {
            return Ok(());
        }
        if !sent.identity.has_history(self) {
            return Ok(());
        }
        let snapshot = self.persistence_manager.get_device_snapshot();
        let own_pn = snapshot
            .pn
            .as_ref()
            .ok_or(crate::client::ClientError::NotLoggedIn)?;
        let addressed = sent.devices;
        // Membership refresh must not add recipients to a message already
        // sent, but the resolve must keep the refreshed LID→PN maps: a bare
        // GroupInfo has an empty map, so LID members would be queried by raw
        // LID and PN-keyed answers could not convert back, emptying the delta.
        let users: std::collections::HashSet<_> =
            addressed.devices().iter().map(Jid::to_non_ad).collect();
        let mode = sent.addressing_mode;
        let own = match mode {
            AddressingMode::Lid => snapshot
                .lid
                .as_ref()
                .ok_or(crate::client::ClientError::NotLoggedIn)?,
            AddressingMode::Pn => own_pn,
        };
        // The refresh above just published, so this is a cache hit for the
        // membership the server returned, maps included.
        let fresh_info = self.groups().query_info(group).await?;
        let mut old_group = (*fresh_info).clone();
        let departed: Vec<wacore_binary::CompactString> = old_group
            .participants
            .iter()
            .filter(|participant| !users.iter().any(|user| user.user == participant.user))
            .map(|participant| participant.user.clone())
            .collect();
        let departed: Vec<&str> = departed.iter().map(|user| user.as_str()).collect();
        old_group.remove_participants(&departed);
        let fresh = self
            .resolve_group_devices_uncached(&old_group, own, Freshness::CachePreferred)
            .await?;
        let delta = group_delta(addressed.devices(), &fresh);
        if delta.is_empty() {
            return Ok(());
        }
        let Some(bytes) = self.recent_message_bytes(group, message_id).await else {
            return Ok(());
        };
        let message = waproto::codec::message_decode(bytes.as_slice())?;
        let devices = wacore::send::ResolvedDmDevices::new(delta, own_pn, snapshot.lid.as_ref());
        let addresses = self.resolve_encryption_jids(devices.devices()).await;
        self.ensure_e2e_sessions_resolved(&addresses).await?;
        let mut keys = addresses.clone();
        super::sort_session_lock_keys(&mut keys);
        let guards = self.session_guards_for(&keys).await;
        if !sent.identity.has_history(self) {
            return Ok(());
        }
        let context = sent
            .message_secret
            .map(|secret| waproto::whatsapp::MessageContextInfo {
                message_secret: Some(secret.to_vec()),
                reporting_token_version: Some(wacore::reporting_token::REPORTING_TOKEN_VERSION),
                ..Default::default()
            });
        let plaintexts = if message.message_context_info.is_unset() {
            wacore::messages::MessageUtils::dm_plaintexts_from_encoded(
                &bytes,
                context.as_ref(),
                group,
            )
        } else {
            wacore::messages::MessageUtils::encode_dm_plaintexts(&message, context.as_ref(), group)
        };
        let mut adapter = self.signal_adapter();
        let mut stores = adapter.as_signal_stores();
        let mut participants = Vec::with_capacity(devices.devices().len());
        let mut prekey = false;
        let mut attempted_devices = 0;
        let split = devices.recipient_devices().len();
        let edit = wacore::types::message::EditAttribute::infer_from_message(&message);
        for (targets, plaintext, addresses) in [
            (
                devices.recipient_devices(),
                plaintexts.recipient.as_slice(),
                &addresses[..split],
            ),
            (
                devices.own_other_devices(),
                plaintexts.own_devices.as_slice(),
                &addresses[split..],
            ),
        ] {
            let mut eligible_targets = Vec::with_capacity(targets.len());
            let mut eligible_addresses = Vec::with_capacity(targets.len());
            for (target, address) in targets.iter().zip(addresses) {
                if sent.identity.unchanged_for(self, &target.user).await
                    && (target.user == address.user
                        || sent.identity.unchanged_for(self, &address.user).await)
                {
                    eligible_targets.push(target.clone());
                    eligible_addresses.push(address.clone());
                }
            }
            if eligible_targets.is_empty() {
                continue;
            }
            attempted_devices += eligible_targets.len();
            let summary = wacore::send::encrypt_for_devices_into(
                &*self.runtime,
                &mut stores,
                self.as_ref(),
                &eligible_targets,
                plaintext,
                wacore::send::should_hide_decrypt_fail_for_send(edit.as_ref(), &message),
                wacore::send::media_type_from_message(&message),
                &mut participants,
                Some(&eligible_addresses),
            )
            .await?;
            prekey |= summary.includes_prekey_message;
        }
        drop(guards);
        if attempted_devices == 0 {
            return Ok(());
        }
        if participants.is_empty() {
            anyhow::bail!("group repair could not encrypt for any missed device");
        }
        self.persist_signal_state_pre_wire().await?;
        // Discard ciphertext for accounts changed during encryption or flush.
        // Later changes cannot re-encrypt these bytes under a replacement key.
        let mut permitted = Vec::with_capacity(participants.len());
        for participant in participants {
            let jid = participant.attrs().optional_jid("jid");
            if let Some(jid) = jid
                && sent.identity.unchanged_for(self, &jid.user).await
            {
                permitted.push(participant);
            }
        }
        if permitted.is_empty() {
            return Ok(());
        }
        let mut children = vec![
            wacore_binary::builder::NodeBuilder::new("participants")
                .children(permitted)
                .build(),
        ];
        let mut marker = wacore_binary::builder::NodeBuilder::new("enc")
            .attr("v", "2")
            .attr("type", "skmsg");
        if let Some(media) = wacore::send::media_type_from_message(&message) {
            marker = marker.attr("mediatype", media);
        }
        children.push(marker.build());
        if let Some(identity) =
            wacore::send::needs_device_identity(prekey, snapshot.account.as_deref())?
        {
            children.push(
                wacore_binary::builder::NodeBuilder::new("device-identity")
                    .bytes(identity)
                    .build(),
            );
        }
        let mut node = wacore_binary::builder::NodeBuilder::new("message")
            .attr("id", message_id)
            .attr("to", group.clone())
            .attr("type", wacore::send::stanza_type_from_message(&message))
            .attr("addressing_mode", mode.as_str())
            .attr("device_fanout", "false");
        if let Some(edit) = edit {
            node = node.attr("edit", edit.to_string_val());
        }
        if self.connection_generation.load(Ordering::Acquire) != generation {
            return Ok(());
        }
        // Pin publication to the socket that owned the ack, even if reconnect races this poll.
        let plaintext = self.marshal_node_for_send(node.children(children).build())?;
        socket
            .encrypt_and_send(bytes::Bytes::from(plaintext))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::proto_helpers::MessageBuilderExt;

    #[tokio::test]
    async fn missing_identity_history_still_waits_for_device_refresh() {
        use std::future::{Future, poll_fn};
        use std::task::Poll;

        let (client, _) = crate::test_utils::create_iq_test_client().await;
        let group = "120363000000000001@g.us".parse().unwrap();
        let generation = client.connection_generation.load(Ordering::Acquire);
        let refresh = client
            .pending_group_device_resync
            .refresh(generation, &group);
        let mut result = refresh.lock().await;
        let sent = GroupSendSnapshot {
            connection_generation: generation,
            identity: GroupIdentitySnapshot::default(),
            devices: Arc::new(wacore::send::ResolvedGroupDevices::new(Vec::new())),
            message_secret: None,
            addressing_mode: AddressingMode::Pn,
        };
        let repair = client.repair_group_message(&group, "NOHISTORY", sent, generation);
        futures::pin_mut!(repair);
        assert!(poll_fn(|cx| Poll::Ready(repair.as_mut().poll(cx).is_pending())).await);
        *result = Some(true);
        drop(result);
        repair.await.unwrap();
    }

    #[tokio::test]
    async fn reregistered_primary_excludes_historical_delta_on_hot_and_cold_saves() {
        use wacore::libsignal::protocol::{IdentityKeyPair, IdentityKeyStore, ProtocolAddress};

        for cold in [false, true] {
            let client = crate::test_utils::create_test_client().await;
            let mut adapter = client.signal_adapter();
            let primary = ProtocolAddress::new("15550000001", 0.into());
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            let first = *IdentityKeyPair::generate(&mut rng).identity_key();
            let replacement = *IdentityKeyPair::generate(&mut rng).identity_key();
            adapter
                .identity_store
                .save_identity(&primary, &first)
                .await
                .unwrap();
            let sent = GroupIdentitySnapshot::capture(&client);
            if cold {
                client
                    .signal_cache
                    .flush(client.persistence_manager.backend().as_ref())
                    .await
                    .unwrap();
                client.signal_cache.clear_after_flush().await;
            }
            adapter
                .identity_store
                .save_identity(&primary, &replacement)
                .await
                .unwrap();
            assert!(!sent.unchanged_for(&client, "15550000001").await);
            adapter
                .identity_store
                .save_identity(&primary, &first)
                .await
                .unwrap();
            assert!(
                !sent.unchanged_for(&client, "15550000001").await,
                "changing back must not restore old-message access"
            );
        }
    }

    #[tokio::test]
    async fn same_primary_and_new_companion_keep_historical_delta_eligible() {
        use wacore::libsignal::protocol::{IdentityKeyPair, IdentityKeyStore};
        use wacore::types::jid::JidExt;

        let client = crate::test_utils::create_test_client().await;
        let mut adapter = client.signal_adapter();
        let primary: Jid = "15550000001@s.whatsapp.net".parse().unwrap();
        let companion = primary.with_device(2);
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let key = *IdentityKeyPair::generate(&mut rng).identity_key();
        adapter
            .identity_store
            .save_identity(&primary.to_protocol_address(), &key)
            .await
            .unwrap();
        let sent = GroupIdentitySnapshot::capture(&client);
        adapter
            .identity_store
            .save_identity(&primary.to_protocol_address(), &key)
            .await
            .unwrap();
        adapter
            .identity_store
            .save_identity(&companion.to_protocol_address(), &key)
            .await
            .unwrap();
        assert!(sent.unchanged_for(&client, "15550000001").await);
        assert_eq!(
            group_delta(
                std::slice::from_ref(&primary),
                &[primary.clone(), companion.clone()]
            ),
            vec![companion]
        );
    }

    #[tokio::test]
    async fn unknown_primary_first_observation_does_not_block_a_companion() {
        use wacore::libsignal::protocol::{IdentityKeyPair, IdentityKeyStore, ProtocolAddress};

        let client = crate::test_utils::create_test_client().await;
        let sent = GroupIdentitySnapshot::capture(&client);
        let mut adapter = client.signal_adapter();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let key = *IdentityKeyPair::generate(&mut rng).identity_key();
        for device in [0, 2] {
            adapter
                .identity_store
                .save_identity(&ProtocolAddress::new("15550000001", device.into()), &key)
                .await
                .unwrap();
        }
        assert!(sent.unchanged_for(&client, "15550000001").await);
    }

    #[tokio::test]
    async fn alias_remap_excludes_history_but_relearning_the_same_pair_does_not() {
        use crate::lid_pn_cache::{LearningSource, LidPnEntry};

        let client = crate::test_utils::create_test_client().await;
        let original = LidPnEntry::new("100000000000001", "15550000001", LearningSource::Usync);
        client.lid_pn_cache.add(&original).await;
        let sent = GroupIdentitySnapshot::capture(&client);
        client.lid_pn_cache.add(&original).await;
        assert!(sent.unchanged_for(&client, "15550000001").await);
        let mut replacement = original.clone();
        replacement.lid = "100000000000002".into();
        client.lid_pn_cache.add(&replacement).await;
        assert!(!sent.unchanged_for(&client, "15550000001").await);
        let current = GroupIdentitySnapshot::capture(&client);
        assert!(!current.unchanged_for(&client, "100000000000001").await);
        assert!(current.unchanged_for(&client, "100000000000002").await);
        assert!(current.unchanged_for(&client, "15550000002").await);
        client.lid_pn_cache.add(&original).await;
        assert!(!sent.unchanged_for(&client, "15550000001").await);
    }

    #[tokio::test]
    async fn raw_id_reset_excludes_history_even_without_companion_sessions() {
        use crate::lid_pn_cache::{LearningSource, LidPnEntry};

        let client = crate::test_utils::create_test_client().await;
        client
            .lid_pn_cache
            .add(&LidPnEntry::new(
                "100000000000001",
                "15550000001",
                LearningSource::Usync,
            ))
            .await;
        let sent = GroupIdentitySnapshot::capture(&client);
        let record = wacore::store::traits::DeviceListRecord {
            user: "15550000001".into(),
            devices: vec![wacore::store::traits::DeviceInfo::new(0, None)].into(),
            timestamp: 0,
            phash: None,
            raw_id: Some(1),
        };
        client
            .clear_device_record("15550000001", "s.whatsapp.net", &record)
            .await;
        assert!(!sent.unchanged_for(&client, "15550000001").await);
        assert!(!sent.unchanged_for(&client, "100000000000001").await);
        assert!(sent.unchanged_for(&client, "15550000002").await);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn primary_identity_change_is_shared_by_pn_and_lid_aliases() {
        use crate::lid_pn_cache::{LearningSource, LidPnEntry};
        use crate::test_utils::seed_peer_session;

        let client = crate::test_utils::create_test_client().await;
        client
            .lid_pn_cache
            .add(&LidPnEntry::new(
                "100000000000001",
                "15550000001",
                LearningSource::Usync,
            ))
            .await;
        let primary: Jid = "100000000000001@lid".parse().unwrap();
        seed_peer_session(&client, &primary).await;
        let sent = GroupIdentitySnapshot::capture(&client);
        seed_peer_session(&client, &primary).await;
        assert!(!sent.unchanged_for(&client, "15550000001").await);
        assert!(!sent.unchanged_for(&client, "100000000000001").await);
        assert!(sent.unchanged_for(&client, "15550000002").await);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[tokio::test]
    async fn historical_repair_emits_only_for_a_continuous_account() {
        use crate::store::commands::DeviceCommand;
        use crate::test_utils::{create_iq_test_client, decode_sent_iq, seed_peer_session};
        use wacore::client::context::GroupInfo;
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};
        use wacore::types::jid::JidExt;

        for (replaced, during_repair) in [(false, false), (true, false), (true, true)] {
            let (client, transport) = create_iq_test_client().await;
            client
                .persistence_manager
                .process_command(DeviceCommand::SetAccount(Some(
                    super::super::tests::peer_test_account_proto(),
                )))
                .await;
            let own: Jid = "15550000009@s.whatsapp.net".parse().unwrap();
            let primary: Jid = "15550000001@s.whatsapp.net".parse().unwrap();
            let companion = primary.with_device(2);
            let stable: Jid = "15550000002@s.whatsapp.net".parse().unwrap();
            let stable_companion = stable.with_device(2);
            let group: Jid = "120363000000000001@g.us".parse().unwrap();
            client
                .persistence_manager
                .process_command(DeviceCommand::SetId(Some(own.clone())))
                .await;
            client
                .persistence_manager
                .process_command(DeviceCommand::SetLid(Some(
                    "100000000000009@lid".parse().unwrap(),
                )))
                .await;
            for jid in [&own, &primary, &stable] {
                client
                    .device_registry_cache
                    .raw_insert_for_tests(
                        Arc::from(jid.user.as_str()),
                        Arc::new(DeviceListRecord {
                            user: jid.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None)].into(),
                            timestamp: wacore::time::now_secs(),
                            phash: None,
                            raw_id: None,
                        }),
                    )
                    .await;
            }
            seed_peer_session(&client, &primary).await;
            seed_peer_session(&client, &stable).await;
            client
                .get_group_cache()
                .insert(
                    group.clone(),
                    Arc::new(GroupInfo::new(
                        vec![primary.clone(), stable.clone()],
                        AddressingMode::Pn,
                    )),
                )
                .await;
            client
                .send_message_with_options(
                    group.clone(),
                    waproto::whatsapp::Message::text("original message"),
                    super::super::SendOptions::default().with_message_id("CONTINUITY"),
                )
                .await
                .unwrap();
            let Some(crate::client::ResponseWaiter::GroupPhash(_, sent)) =
                client.response_waiters_guard().remove("CONTINUITY")
            else {
                panic!("original group snapshot missing");
            };
            seed_peer_session(&client, &companion).await;
            seed_peer_session(&client, &stable_companion).await;
            let unrelated: Jid = "15550000003@s.whatsapp.net".parse().unwrap();
            seed_peer_session(&client, &unrelated).await;
            seed_peer_session(&client, &unrelated).await;
            client
                .lid_pn_cache
                .add(&crate::lid_pn_cache::LidPnEntry::new(
                    "100000000000003",
                    "15550000003",
                    crate::lid_pn_cache::LearningSource::Usync,
                ))
                .await;
            if replaced && !during_repair {
                seed_peer_session(&client, &primary).await;
            }
            for member in [&primary, &stable] {
                client
                    .device_registry_cache
                    .raw_insert_for_tests(
                        Arc::from(member.user.as_str()),
                        Arc::new(DeviceListRecord {
                            user: member.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(2, None)].into(),
                            timestamp: wacore::time::now_secs(),
                            phash: None,
                            raw_id: None,
                        }),
                    )
                    .await;
            }
            let generation = sent.connection_generation;
            let refresh = client
                .pending_group_device_resync
                .refresh(generation, &group);
            *refresh.lock().await = Some(true);
            let before = transport.sent_count();
            let guard = if during_repair {
                Some(
                    client
                        .session_lock_for(companion.to_protocol_address().as_str())
                        .await
                        .lock_arc()
                        .await,
                )
            } else {
                None
            };
            let repair = client.repair_group_message(&group, "CONTINUITY", sent, generation);
            futures::pin_mut!(repair);
            if during_repair {
                assert!(futures::poll!(repair.as_mut()).is_pending());
                seed_peer_session(&client, &primary).await;
            }
            drop(guard);
            repair.await.unwrap();
            assert_eq!(transport.sent_count(), before + 1);
            let owned = decode_sent_iq(&transport, before).await;
            let node = owned.get();
            assert_eq!(
                node.attrs().optional_string("id").as_deref(),
                Some("CONTINUITY")
            );
            let participants = node.get_optional_child("participants").unwrap();
            let targets: Vec<_> = participants
                .children()
                .unwrap()
                .iter()
                .map(|child| child.attrs().optional_jid("jid").unwrap())
                .collect();
            assert!(targets.contains(&stable_companion));
            assert_eq!(targets.contains(&companion), !replaced);
            assert_eq!(targets.len(), 1 + usize::from(!replaced));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn duplicate_revokes_share_one_expiry_and_release_map_capacity() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let runtime: Arc<dyn wacore::runtime::Runtime> =
            Arc::new(crate::runtime_impl::TokioRuntime);
        let shutdown = wacore::runtime::ShutdownNotifier::new();
        repairs.revoke_message(1, &group, "ORIGINAL", &runtime, shutdown.subscribe());
        let handle = Arc::clone(
            repairs
                .messages
                .lock()
                .unwrap()
                .get(&(1, group.clone(), "ORIGINAL".into()))
                .unwrap(),
        );
        for _ in 0..32 {
            repairs.revoke_message(1, &group, "ORIGINAL", &runtime, shutdown.subscribe());
        }
        assert_eq!(
            Arc::strong_count(&handle),
            3,
            "map, one expiry task, and this test"
        );
        repairs.finish_message(1, &group, "ORIGINAL", &handle);
        assert_eq!(
            repairs.retained_message_count(),
            1,
            "completion must retain revoke suppression until expiry"
        );
        tokio::task::yield_now().await;
        tokio::time::advance(super::super::GROUP_DEVICE_RESYNC_COOLDOWN).await;
        tokio::task::yield_now().await;
        assert_eq!(repairs.retained_message_count(), 0);
        assert_eq!(repairs.messages.lock().unwrap().capacity(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn revoke_expiry_stops_on_shutdown_without_touching_new_generation() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let runtime: Arc<dyn wacore::runtime::Runtime> =
            Arc::new(crate::runtime_impl::TokioRuntime);
        let old_shutdown = wacore::runtime::ShutdownNotifier::new();
        repairs.revoke_message(1, &group, "ORIGINAL", &runtime, old_shutdown.subscribe());
        repairs.clear();
        let new_shutdown = wacore::runtime::ShutdownNotifier::new();
        repairs.revoke_message(2, &group, "ORIGINAL", &runtime, new_shutdown.subscribe());
        old_shutdown.notify();
        tokio::task::yield_now().await;
        assert_eq!(repairs.retained_message_count(), 1);
        new_shutdown.notify();
        tokio::task::yield_now().await;
        assert_eq!(repairs.retained_message_count(), 0);
        assert_eq!(repairs.messages.lock().unwrap().capacity(), 0);
    }

    #[test]
    fn disconnect_resets_cooldown_and_old_completion_cannot_release_new_refresh() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let old = repairs.refresh(1, &group);
        assert!(Arc::ptr_eq(&old, &repairs.refresh(1, &group)));
        repairs.clear();
        let new = repairs.refresh(2, &group);
        repairs.remove(1, &group, &old);
        assert!(Arc::ptr_eq(&new, &repairs.refresh(2, &group)));
        repairs.remove(2, &group, &new);
        assert_eq!(repairs.len(), 0);
    }

    #[tokio::test]
    async fn successful_refresh_is_shared_without_consuming_message_repairs() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let first = repairs.refresh(1, &group);
        let second = repairs.refresh(1, &group);
        *first.lock().await = Some(true);
        assert_eq!(*second.lock().await, Some(true));
        assert_eq!(*first.lock().await, Some(true));
    }

    /// A failed refresh must not hold the full divergence cooldown: the next
    /// mismatch after a network blip is likely legitimate. Advancing past
    /// the retry backoff releases the mark while the full cooldown still has
    /// almost its whole duration left, which is what distinguishes the two.
    #[tokio::test(start_paused = true)]
    async fn failed_refresh_uses_the_short_backoff() {
        let client = crate::test_utils::create_test_client().await;
        let group = "120363000000000001@g.us".parse().unwrap();
        assert!(!client.refresh_group_for_repair(&group, 0).await);
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(
            super::super::GROUP_DEVICE_RESYNC_RETRY_COOLDOWN + std::time::Duration::from_secs(1),
        )
        .await;
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
        assert_eq!(client.pending_group_device_resync.len(), 0);
    }

    #[tokio::test]
    async fn revoke_cancels_only_the_matching_message_and_generation() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let (old, old_handle) = repairs.start_message(1, &group, "ORIGINAL");
        repairs.clear();
        let (current, _current_handle) = repairs.start_message(2, &group, "ORIGINAL");
        repairs.finish_message(1, &group, "ORIGINAL", &old_handle);
        let runtime: Arc<dyn wacore::runtime::Runtime> =
            Arc::new(crate::runtime_impl::TokioRuntime);
        repairs.revoke_message(
            2,
            &group,
            "ORIGINAL",
            &runtime,
            wacore::runtime::ShutdownSignal::never(),
        );
        assert!(
            futures::future::Abortable::new(futures::future::pending::<()>(), old)
                .await
                .is_err()
        );
        assert!(
            futures::future::Abortable::new(futures::future::pending::<()>(), current)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn revoke_between_ack_removal_and_repair_registration_still_cancels() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let runtime: Arc<dyn wacore::runtime::Runtime> =
            Arc::new(crate::runtime_impl::TokioRuntime);
        repairs.revoke_message(
            1,
            &group,
            "ORIGINAL",
            &runtime,
            wacore::runtime::ShutdownSignal::never(),
        );
        let (registration, handle) = repairs.start_message(1, &group, "ORIGINAL");
        assert!(
            futures::future::Abortable::new(futures::future::pending::<()>(), registration)
                .await
                .is_err()
        );
        repairs.finish_message(1, &group, "ORIGINAL", &handle);
        assert_eq!(repairs.active_message_count(), 0);
    }

    #[tokio::test]
    async fn old_completion_cannot_release_reused_message_id_reservation() {
        let repairs = GroupRepair::new();
        let group = "120363000000000001@g.us".parse().unwrap();
        let (old, old_handle) = repairs.start_message(1, &group, "EXPLICITID");
        let (current, current_handle) = repairs.start_message(1, &group, "EXPLICITID");
        repairs.finish_message(1, &group, "EXPLICITID", &old_handle);
        assert_eq!(repairs.active_message_count(), 1);
        let runtime: Arc<dyn wacore::runtime::Runtime> =
            Arc::new(crate::runtime_impl::TokioRuntime);
        repairs.revoke_message(
            1,
            &group,
            "EXPLICITID",
            &runtime,
            wacore::runtime::ShutdownSignal::never(),
        );
        assert!(
            futures::future::Abortable::new(futures::future::pending::<()>(), old)
                .await
                .is_err()
        );
        assert!(
            futures::future::Abortable::new(futures::future::pending::<()>(), current)
                .await
                .is_err()
        );
        repairs.finish_message(1, &group, "EXPLICITID", &current_handle);
        assert_eq!(
            repairs.retained_message_count(),
            1,
            "revoked tombstone survives completion"
        );
    }

    #[test]
    fn group_delta_excludes_new_members_and_already_addressed_devices() {
        let old: Vec<Jid> = vec!["111111111111:1@s.whatsapp.net".parse().unwrap()];
        let fresh = vec![
            old[0].clone(),
            "111111111111:2@s.whatsapp.net".parse().unwrap(),
            "222222222222:1@s.whatsapp.net".parse().unwrap(),
        ];
        assert_eq!(group_delta(&old, &fresh), vec![fresh[1].clone()]);
    }
}
