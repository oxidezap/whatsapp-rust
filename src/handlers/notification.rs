use super::traits::StanzaHandler;
use crate::client::Client;
use crate::lid_pn_cache::LearningSource;
use crate::types::events::Event;
use async_trait::async_trait;
use log::{debug, info, warn};
use std::sync::Arc;
use wacore::stanza::business::BusinessNotification;
use wacore::stanza::devices::DeviceNotification;
use wacore::stanza::groups::{GroupNotification, GroupNotificationAction};
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore::types::events::{
    BusinessStatusUpdate, BusinessUpdateType, ContactNumberChanged, ContactSyncRequested,
    ContactUpdated, DeviceListUpdate, DeviceNotificationInfo, GroupUpdate, MexNotification,
    PictureUpdate, UserAboutUpdate,
};
use wacore_binary::NodeContentRef;
use wacore_binary::{Jid, JidExt};
use wacore_binary::{NodeRef, OwnedNodeRef};

/// Handler for `<notification>` stanzas.
///
/// Processes various notification types including:
/// - Encrypt notifications (key upload requests)
/// - Server sync notifications
/// - Account sync notifications (push name updates)
/// - Device notifications (device add/remove/update)
#[derive(Default)]
pub struct NotificationHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for NotificationHandler {
    fn tag(&self) -> &'static str {
        "notification"
    }

    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        handle_notification_impl(&client, node).await;
        true
    }
}

/// Dispatch notification by type. Each arm calls a separate async fn so the
/// compiler doesn't size this future for all arms simultaneously.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.dispatch", level = "debug", skip_all)
)]
async fn handle_notification_impl(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    let nr = node.get();
    let notification_type = nr.attrs().optional_string("type");

    match notification_type.as_deref().unwrap_or_default() {
        "encrypt" => handle_encrypt_notification(client, nr).await,
        "server_sync" => handle_server_sync_notification(client, nr),
        "account_sync" => handle_account_sync_notification(client, nr).await,
        "devices" => handle_devices_notification(client, nr).await,
        "link_code_companion_reg" => {
            crate::pair_code::handle_pair_code_notification(client, nr).await;
        }
        "business" => handle_business_notification(client, nr).await,
        "picture" => handle_picture_notification(client, nr),
        "privacy_token" => handle_privacy_token_notification(client, nr).await,
        "status" => handle_status_notification(client, nr),
        "contacts" => handle_contacts_notification(client, nr).await,
        "w:gp2" => handle_group_notification(client, Arc::clone(&node)).await,
        "disappearing_mode" => handle_disappearing_mode_notification(client, nr),
        "newsletter" => handle_newsletter_notification(client, Arc::clone(&node)),
        "mex" => handle_mex_notification(client, nr),
        "mediaretry" => {
            debug!(
                "Received mediaretry notification for msg {}",
                nr.attrs().optional_string("id").unwrap_or_default()
            );
        }
        other => {
            debug!("Unhandled notification type '{other}', dispatching raw event");
            client
                .core
                .event_bus
                .dispatch(Event::Notification(Arc::clone(&node)));
        }
    }
}

async fn handle_encrypt_notification(client: &Arc<Client>, nr: &wacore_binary::NodeRef<'_>) {
    if nr.get_optional_child("identity").is_some() {
        handle_identity_change(client, nr).await;
    } else if nr
        .get_attr("from")
        .is_some_and(|v| v.as_str() == wacore_binary::SERVER_JID)
    {
        let first_child_tag = nr
            .children()
            .and_then(|c| c.first().map(|n| n.tag.as_ref()));
        match first_child_tag {
            Some("count") => handle_prekey_low(client).await,
            Some("digest") => handle_digest_key(client),
            other => warn!("Unhandled encrypt notification child: {:?}", other),
        }
    }
}

/// Sync is fire-and-forget (spawned), so this is not async -- it parses
/// collection nodes synchronously and spawns the async sync task.
fn handle_server_sync_notification(client: &Arc<Client>, nr: &wacore_binary::NodeRef<'_>) {
    use std::str::FromStr;
    use wacore::appstate::patch_decode::WAPatchName;

    let mut collections = Vec::new();
    if let Some(children) = nr.children() {
        for collection_node in children.iter().filter(|c| c.tag == "collection") {
            let name_cow = collection_node.attrs().optional_string("name");
            let name_str = name_cow.as_deref().unwrap_or("<unknown>");
            let server_version = collection_node.attrs().optional_u64("version").unwrap_or(0);
            debug!(
                target: "Client/AppState",
                "Received server_sync for collection '{}' version {}",
                name_str, server_version
            );
            if let Ok(patch_name) = WAPatchName::from_str(name_str)
                && !matches!(patch_name, WAPatchName::Unknown)
            {
                collections.push((patch_name, server_version));
            }
        }
    }

    if !collections.is_empty() {
        let client_clone = client.clone();
        let generation = client
            .connection_generation
            .load(std::sync::atomic::Ordering::Acquire);
        client
            .runtime
            .spawn(Box::pin(async move {
                if client_clone
                    .connection_generation
                    .load(std::sync::atomic::Ordering::Acquire)
                    != generation
                {
                    log::debug!(target: "Client/AppState", "server_sync task cancelled: connection generation changed");
                    return;
                }

                let backend = client_clone.persistence_manager.backend();
                let mut to_sync = Vec::new();
                for (name, server_version) in collections {
                    if server_version > 0 {
                        match backend.get_version(name.as_str()).await {
                            Ok(state) if state.version >= server_version => {
                                debug!(
                                    target: "Client/AppState",
                                    "Skipping server_sync for {:?}: local version {} >= server version {}",
                                    name, state.version, server_version
                                );
                                continue;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                warn!(
                                    target: "Client/AppState",
                                    "Failed to get local version for {:?}: {e}, syncing anyway",
                                    name
                                );
                            }
                        }
                    }
                    to_sync.push(name);
                }

                if !to_sync.is_empty() {
                    if client_clone.is_shutting_down() {
                        log::debug!(target: "Client/AppState", "Skipping server_sync: client is shutting down");
                        return;
                    }
                    if client_clone
                        .connection_generation
                        .load(std::sync::atomic::Ordering::Acquire)
                        != generation
                    {
                        log::debug!(target: "Client/AppState", "server_sync task cancelled: connection generation changed during version check");
                        return;
                    }
                    if let Err(e) = client_clone.sync_collections_batched(to_sync).await
                        && !client_clone.is_shutting_down()
                    {
                        warn!(
                            target: "Client/AppState",
                            "Failed to batch sync app state from server_sync: {e}"
                        );
                    }
                }
            }))
            .detach();
    }
}

async fn handle_account_sync_notification(client: &Arc<Client>, nr: &wacore_binary::NodeRef<'_>) {
    if let Some(new_push_name) = nr.attrs().optional_string("pushname") {
        client
            .clone()
            .update_push_name_and_notify(new_push_name.to_string())
            .await;
    }
    if let Some(devices_node) = nr.get_optional_child_by_tag(&["devices"]) {
        handle_account_sync_devices(client, nr, devices_node).await;
    }
}

/// Handle encrypt/count notification (PreKey Low).
///
/// Matches WA Web's `WAWebHandlePreKeyLow`:
/// 1. Mark `server_has_prekeys = false`
/// 2. Wait for offline delivery to complete
/// 3. Acquire dedup lock (prevents concurrent uploads)
/// 4. Upload prekeys with Fibonacci retry
async fn handle_prekey_low(client: &Arc<Client>) {
    // Persist flag matching WA Web's setServerHasPreKeys(false) (PreKeyLow.js:43)
    client
        .persistence_manager
        .modify_device(|d| d.server_has_prekeys = false)
        .await;

    let client_clone = client.clone();
    client
        .runtime
        .spawn(Box::pin(async move {
            // Wait for offline delivery first (matches WA Web's waitForOfflineDeliveryEnd)
            client_clone.wait_for_offline_delivery_end().await;

            if !client_clone
                .is_logged_in
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                debug!("Pre-key upload skipped: disconnected during offline delivery wait");
                return;
            }

            let _guard = client_clone.prekey_upload_lock.lock().await;

            // Dedup: check persisted flag in case another task already uploaded
            if client_clone
                .persistence_manager
                .get_device_snapshot()
                .await
                .server_has_prekeys
            {
                debug!("Pre-key upload already completed by another task, skipping");
                return;
            }

            // WA Web's handlePreKeyLow uploads unconditionally (no server-count query).
            // Force past the count guard: the server only emits prekey-low after crossing
            // its own (higher) threshold, so re-querying and skipping when count >= 5 lets
            // the pool keep draining.
            if let Err(e) = client_clone.upload_pre_keys_with_retry(true).await {
                warn!(
                    "Failed to upload pre-keys after prekey_low notification: {:?}",
                    e
                );
            }
        }))
        .detach();
}

/// Handle encrypt/digest notification (Digest Key validation).
///
/// Matches WA Web's `WAWebHandleDigestKey`:
/// Queries server for key bundle digest, validates SHA-1 hash locally,
/// re-uploads only when the server has no record (404).
///
/// `validate_digest_key` owns `prekey_upload_lock` acquisition internally, so
/// any upload it triggers stays serialized with `upload_pre_keys_at_login`,
/// `handle_prekey_low`, and `refresh_pre_keys` without this caller needing to
/// (and indeed, holding it here would deadlock — `async_lock::Mutex` is not
/// reentrant).
fn handle_digest_key(client: &Arc<Client>) {
    let client_clone = client.clone();
    client
        .runtime
        .spawn(Box::pin(async move {
            if let Err(e) = client_clone.validate_digest_key().await {
                warn!("Digest key validation failed: {:?}", e);
            }
        }))
        .detach();
}

/// Handle identity change notification (user reinstalled WhatsApp).
///
/// Matches WA Web's `WAWebHandleIdentityChange`:
/// ```xml
/// <notification type="encrypt" from="user@s.whatsapp.net">
///   <identity/>
/// </notification>
/// ```
///
/// WA Web defers this when offline. We process immediately because all cleanup
/// is local-only, and `ensure_e2e_sessions` self-defers via `wait_for_offline_delivery_end`.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.identity_change", level = "debug", skip_all)
)]
async fn handle_identity_change(client: &Arc<Client>, node: &NodeRef<'_>) {
    let from_jid = crate::require_from_jid!(node, "Identity change notification");

    // Only primary device identity changes matter
    if from_jid.device != 0 {
        debug!(
            "Ignoring identity change from companion device {}",
            from_jid.observe()
        );
        return;
    }

    // Self-identity changes use a different flow; clearing our own record would break sessions
    let device_snapshot = client.persistence_manager.get_device_snapshot().await;
    let is_me = device_snapshot
        .pn
        .as_ref()
        .is_some_and(|pn| pn.user == from_jid.user)
        || device_snapshot
            .lid
            .as_ref()
            .is_some_and(|lid| lid.user == from_jid.user);
    if is_me {
        debug!("Ignoring self-primary identity change");
        return;
    }

    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    use wacore::types::jid::JidExt;

    // Always run the device-list cleanup, matching WA Web's
    // clearDeviceRecordForIdentityChange (which runs BEFORE the had-prior-identity
    // gate): drop companion device sessions + force a fresh usync of the peer's
    // device list on the next send.
    if let Some(record) = client.load_device_record(&from_jid.user).await {
        client
            .clear_device_record(&from_jid.user, from_jid.server.as_str(), &record)
            .await;
    }
    client.invalidate_device_cache(&from_jid.user).await;

    // WA Web gates the heavy reset behind loadIdentityKey(addr) != null
    // (WAWebHandleIdentityChange: `if (!isStringNullOrEmpty(t))`). Read the stored
    // identity non-destructively BEFORE deleting it. With no prior identity (e.g. a
    // group-only peer we never had a session with), skip the session delete/rebuild,
    // status sender-key rotation, tcToken reissue and the change notification — that
    // path would otherwise eagerly fetch prekeys + X3DH to build a session we may
    // never use.
    //
    // Check every address the identity could be stored under, because PN/LID
    // resolution can diverge from where the state actually lives:
    //   - the resolved (preferred LID-or-PN) address from resolve_encryption_jid;
    //   - the original PN address (state can still be under PN when a PN->LID
    //     mapping was learned from offline replay but the migration hasn't run yet);
    //   - the LID carried by the stanza itself (the local cache may be cold/evicted
    //     so resolve falls back to PN, yet the state lives under the stanza LID).
    // Reading only the resolved address would false-negative and skip a real reset.
    let resolved = client.resolve_encryption_jid(&from_jid).await;
    let stanza_lid = node.attrs().optional_jid("lid");
    let backend = client.persistence_manager.backend();

    let mut reset_addrs = vec![resolved.to_protocol_address()];
    for candidate in [Some(from_jid.clone()), stanza_lid.clone()]
        .into_iter()
        .flatten()
    {
        let cand_addr = candidate.to_protocol_address();
        if !reset_addrs.contains(&cand_addr) {
            reset_addrs.push(cand_addr);
        }
    }

    // Treat a backend read error as had-prior (fail-safe): run the reset rather
    // than silently skip it, matching the old always-reset behavior. Collapsing
    // an Err into "no prior identity" would be a fail-open regression on a
    // session-deletion path (see the same explicit-match rule in lid_pn.rs).
    let mut had_prior_identity = false;
    for cand in &reset_addrs {
        match client
            .signal_cache
            .get_identity(cand, backend.as_ref())
            .await
        {
            Ok(Some(_)) => {
                had_prior_identity = true;
                break;
            }
            Ok(None) => {}
            Err(e) => {
                warn!(
                    "Identity change: failed reading stored identity for {}: {e}; proceeding with reset",
                    wacore::types::jid::observe_protocol_address(cand)
                );
                had_prior_identity = true;
                break;
            }
        }
    }

    if !had_prior_identity {
        info!(
            "Identity change for {} (had_prior_identity=false): device record cleared, skipping session reset",
            from_jid.user
        );
        return;
    }

    // Counted here, past the companion/self/no-prior gates, so it reflects actual
    // session resets rather than every identity-change push received.
    wacore::telemetry::identity_change();
    info!(
        "Identity change for {} (had_prior_identity=true): resetting session",
        from_jid.user
    );

    // Delete the session + identity at every candidate address (resolved + the
    // pre-migration PN one) so a fresh session can be established, and rotate the
    // status sender key for forward secrecy. Single flush covers all of it.
    {
        for cand in &reset_addrs {
            // Hold the per-address session lock while deleting to prevent concurrent
            // encrypt/decrypt from recreating the stale session (mirrors
            // Signal::delete_sessions). One lock at a time, so no lock-ordering risk.
            let lock = client.session_lock_for(cand.as_str()).await;
            let _guard = lock.lock().await;
            client.signal_cache.delete_session(cand).await;
            client.signal_cache.delete_identity(cand).await;
        }

        let status_group = "status@broadcast";
        for own_jid in device_snapshot.pn.iter().chain(device_snapshot.lid.iter()) {
            let sk_name =
                SenderKeyName::from_parts(status_group, own_jid.to_protocol_address().as_str());
            client
                .signal_cache
                .delete_sender_key(sk_name.cache_key())
                .await;
        }

        client
            .flush_signal_cache_logged("identity change", None)
            .await;
    }

    // Re-issue an active trusted-contact token, matching WA Web
    // handleE2eIdentityChange -> sendTcTokenWhenDeviceIdentityChange. Spawned so
    // the notification handler doesn't block on an IQ; it no-ops unless a
    // non-expired sender token already exists.
    if !from_jid.is_bot() && !from_jid.is_status_broadcast() {
        let tc_client = client.clone();
        let tc_jid = from_jid.clone();
        client
            .runtime
            .spawn(Box::pin(async move {
                tc_client
                    .reissue_tc_token_after_identity_change(&tc_jid)
                    .await;
            }))
            .detach();
    }

    // = addSecurityCodeChangedNotifications, which WA Web fires inside the gate.
    client.core.event_bus.dispatch(Event::IdentityChange(
        crate::types::events::IdentityChange {
            user: from_jid.clone(),
            lid_user: stanza_lid,
            implicit: false,
        },
    ));

    // Re-establish the session eagerly so the next send is fast (WA Web does this
    // inside the gate too). Skip only while the offline backlog is still draining,
    // matching WA Web's `C = !isEmpty(offline) && !isResumeFromRestartComplete()`:
    // deferring every offline-tagged push would otherwise pile up a prekey-fetch
    // burst when the resume completes. Deferral is safe because every send path
    // re-establishes before encrypting (ensure_e2e_sessions in the DM/group send
    // paths, plus encrypt_for_devices' own has_session->prekey-fetch fallback).
    let arrived_during_resume = node.attrs().optional_string("offline").is_some()
        && !client
            .offline_sync_completed
            .load(std::sync::atomic::Ordering::Relaxed);
    if arrived_during_resume {
        debug!(
            "Identity change for {} arrived during offline resume; deferring session re-establishment to next send",
            from_jid.user
        );
    } else {
        let client_clone = client.clone();
        let session_jid = from_jid;
        client
            .runtime
            .spawn(Box::pin(async move {
                if let Err(e) = client_clone.ensure_e2e_sessions(&[session_jid]).await {
                    warn!("Identity change: failed to re-establish session: {e}");
                }
            }))
            .detach();
    }
}

/// React to a locally-detected identity change.
///
/// Fires when decrypting a peer's message saved a new identity key that replaced
/// a different one (`IdentityChange::ReplacedExisting`). Mirrors WA Web
/// `ProtocolStoreUnifiedApi.saveIdentity` -> `handleNewIdentity`: clear the
/// device-list/sender-key tracking, force a fresh usync, re-issue an active tc
/// token, and emit `Event::IdentityChange { implicit: true }`.
///
/// Deliberately lighter than the server `<identity/>` push handler
/// ([`handle_identity_change`]): it does NOT delete the primary session, rotate
/// the status sender key, or re-establish sessions. The message that triggered
/// this is establishing the new session right now, and the heavier reset is the
/// server push's job (which reliably follows). This matches WA Web, where the
/// local `handleNewIdentity` omits those steps that only the server-push
/// `handleE2eIdentityChange` performs.
#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.notif.local_identity_change", level = "debug", skip_all, fields(sender = %sender.observe())))]
pub(crate) async fn handle_local_identity_change(client: &Arc<Client>, sender: Jid) {
    // Only a peer's primary-device identity change matters; companion devices
    // carry their own identities (WA Web ignores them on this path).
    if sender.device != 0 {
        return;
    }

    // Self-identity changes use a separate flow; clearing our own record would
    // break our sessions.
    let device_snapshot = client.persistence_manager.get_device_snapshot().await;
    let is_me = device_snapshot
        .pn
        .as_ref()
        .is_some_and(|pn| pn.user == sender.user)
        || device_snapshot
            .lid
            .as_ref()
            .is_some_and(|lid| lid.user == sender.user);
    if is_me {
        return;
    }

    info!(
        "Local identity change detected for {}: clearing device record",
        sender.user
    );

    // Deletes non-primary sessions + all sender key device tracking.
    if let Some(record) = client.load_device_record(&sender.user).await {
        client
            .clear_device_record(&sender.user, sender.server.as_str(), &record)
            .await;
    }

    // Force a fresh usync on next send so we re-learn the peer's device list.
    client.invalidate_device_cache(&sender.user).await;

    // Re-issue an active trusted-contact token (no-op unless one is live).
    if !sender.is_bot() && !sender.is_status_broadcast() {
        client.reissue_tc_token_after_identity_change(&sender).await;
    }

    client.core.event_bus.dispatch(Event::IdentityChange(
        crate::types::events::IdentityChange {
            user: sender,
            lid_user: None,
            implicit: true,
        },
    ));
}

/// Handle device list change notifications.
/// Matches WhatsApp Web's WAWebHandleDeviceNotification.handleDevicesNotification().
///
/// Device notifications have the structure:
/// ```xml
/// <notification type="devices" from="user@s.whatsapp.net">
///   <add device_hash="..."> or <remove device_hash="..."> or <update hash="...">
///     <device jid="user:device@server"/>
///     <key-index-list ts="..."/>
///   </add/remove/update>
/// </notification>
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.devices", level = "debug", skip_all)
)]
async fn handle_devices_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let notification = match DeviceNotification::try_parse(node) {
        Ok(n) => n,
        Err(e) => {
            warn!("Failed to parse device notification: {e}");
            return;
        }
    };

    // Learn LID-PN mapping if present
    if let Some((lid, pn)) = notification.lid_pn_mapping()
        && let Err(e) = client
            .add_lid_pn_mapping(lid, pn, LearningSource::DeviceNotification)
            .await
    {
        warn!("Failed to add LID-PN mapping from device notification: {e}");
    }

    // Process the single operation (per WhatsApp Web: one operation per notification).
    // Granularly patch caches instead of invalidating — matches WA Web's
    // bulkCreateOrReplace pattern and avoids a usync IQ round-trip.
    let op = &notification.operation;
    debug!(
        "Device notification: user={}, type={:?}, devices={:?}",
        notification.user(),
        op.operation_type,
        op.device_ids()
    );

    match op.operation_type {
        wacore::stanza::devices::DeviceNotificationType::Add => {
            for device in &op.devices {
                client
                    .patch_device_add(notification.user(), device, op.key_index.as_ref())
                    .await;
            }
        }
        wacore::stanza::devices::DeviceNotificationType::Remove => {
            for device in &op.devices {
                client
                    .patch_device_remove(notification.user(), device.device_id())
                    .await;
            }
        }
        wacore::stanza::devices::DeviceNotificationType::Update => {
            if op.devices.is_empty() {
                // Hash-only update without device list — fall back to
                // invalidation so the next read rehydrates from the server.
                client.invalidate_device_cache(notification.user()).await;
            } else {
                for device in &op.devices {
                    client
                        .patch_device_update(notification.user(), device)
                        .await;
                }
            }
        }
    }

    // Dispatch event to notify application layer
    let event = Event::DeviceListUpdate(DeviceListUpdate {
        user: notification.from.clone(),
        lid_user: notification.lid_user.clone(),
        update_type: op.operation_type.into(),
        devices: op
            .devices
            .iter()
            .map(|d| DeviceNotificationInfo {
                device_id: d.device_id(),
                key_index: d.key_index,
            })
            .collect(),
        key_index: op.key_index.clone(),
        contact_hash: op.contact_hash.clone(),
    });
    client.core.event_bus.dispatch(event);
}

/// Parsed device info from account_sync notification
struct AccountSyncDevice {
    jid: Jid,
    key_index: Option<u32>,
}

/// Parse devices from account_sync notification's <devices> child.
///
/// Example structure:
/// ```xml
/// <devices dhash="2:FnEWjS13">
///   <device jid="15551234567@s.whatsapp.net"/>
///   <device jid="15551234567:64@s.whatsapp.net" key-index="2"/>
///   <key-index-list ts="1766612162"><!-- bytes --></key-index-list>
/// </devices>
/// ```
fn parse_account_sync_device_list(devices_node: &NodeRef<'_>) -> Vec<AccountSyncDevice> {
    let Some(children) = devices_node.children() else {
        return Vec::new();
    };

    children
        .iter()
        .filter(|n| n.tag == "device")
        .filter_map(|n| {
            let jid = n.attrs().optional_jid("jid")?;
            let key_index = n.attrs().optional_u64("key-index").map(|v| v as u32);
            Some(AccountSyncDevice { jid, key_index })
        })
        .collect()
}

/// Handle account_sync notification with <devices> child.
///
/// This is sent when devices are added/removed from OUR account (e.g., pairing a new WhatsApp Web).
/// Matches WhatsApp Web's `handleAccountSyncNotification` for `AccountSyncType.DEVICES`.
///
/// Key behaviors:
/// 1. Check if notification is for our own account (isSameAccountAndAddressingMode)
/// 2. Parse device list from notification
/// 3. Update device registry with new device list
/// 4. Does NOT trigger app state sync (that's handled by server_sync)
async fn handle_account_sync_devices(
    client: &Arc<Client>,
    node: &NodeRef<'_>,
    devices_node: &NodeRef<'_>,
) {
    // Extract the "from" JID - this is the account the notification is about
    let from_jid = crate::require_from_jid!(
        node,
        target: "Client/AccountSync",
        "account_sync devices"
    );

    // Get our own JIDs (PN and LID) to verify this is about our account
    let device_snapshot = client.persistence_manager.get_device_snapshot().await;
    let own_pn = device_snapshot.pn.as_ref();
    let own_lid = device_snapshot.lid.as_ref();

    // Check if notification is about our own account
    // Matches WhatsApp Web's isSameAccountAndAddressingMode check
    let is_own_account = own_pn.is_some_and(|pn| pn.is_same_user_as(&from_jid))
        || own_lid.is_some_and(|lid| lid.is_same_user_as(&from_jid));

    if !is_own_account {
        // WhatsApp Web logs "wid-is-not-self" error in this case
        warn!(
            target: "Client/AccountSync",
            "Received account_sync devices for non-self user: {} (our PN: {:?}, LID: {:?})",
            from_jid.observe(),
            own_pn.map(|j| j.user.as_str()),
            own_lid.map(|j| j.user.as_str())
        );
        return;
    }

    // Parse device list from notification
    let devices = parse_account_sync_device_list(devices_node);
    if devices.is_empty() {
        debug!(target: "Client/AccountSync", "account_sync devices list is empty");
        return;
    }

    // Extract dhash (device hash) for cache validation
    let dhash = devices_node
        .attrs()
        .optional_string("dhash")
        .map(|s| s.into_owned());

    // Get timestamp from notification
    let timestamp = node
        .attrs()
        .optional_u64("t")
        .map(|v| v as i64)
        .unwrap_or_else(wacore::time::now_secs);

    // Preserve existing raw_id so account_sync doesn't erase it
    let existing_raw_id = client
        .load_device_record(&from_jid.user)
        .await
        .and_then(|r| r.raw_id);

    // Build DeviceListRecord for storage
    // Note: update_device_list() will automatically store under LID if mapping is known
    let device_list = DeviceListRecord {
        user: from_jid.user.to_string(),
        devices: devices
            .iter()
            .map(|d| DeviceInfo {
                device_id: d.jid.device as u32,
                key_index: d.key_index,
            })
            .collect(),
        timestamp,
        phash: dhash,
        raw_id: existing_raw_id,
    };

    if let Err(e) = client.update_device_list(device_list).await {
        warn!(
            target: "Client/AccountSync",
            "Failed to update device list from account_sync: {}",
            e
        );
        return;
    }

    info!(
        target: "Client/AccountSync",
        "Updated own device list from account_sync: {} devices (user: {})",
        devices.len(),
        from_jid.user
    );

    // Log individual devices at debug level
    for device in &devices {
        debug!(
            target: "Client/AccountSync",
            "  Device: {} (key-index: {:?})",
            device.jid.observe(),
            device.key_index
        );
    }
}

/// Handle incoming privacy_token notification.
///
/// Stores trusted contact tokens from contacts. Matches WhatsApp Web's
/// `WAWebHandlePrivacyTokenNotification`.
///
/// Structure:
/// ```xml
/// <notification type="privacy_token" from="user@s.whatsapp.net" sender_lid="user@lid">
///   <tokens>
///     <token type="trusted_contact" t="1707000000"><!-- bytes --></token>
///   </tokens>
/// </notification>
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.privacy_token", level = "debug", skip_all)
)]
async fn handle_privacy_token_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    use wacore::iq::tctoken::parse_privacy_token_notification;
    use wacore::store::traits::TcTokenEntry;

    let from_jid = node.attrs().optional_jid("from");

    // Resolve the sender to a LID key for storage.
    // WA Web uses `sender_lid` attr if present, otherwise resolves from `from`.
    let sender_lid_jid = node
        .attrs()
        .optional_jid("sender_lid")
        .filter(|j| !j.user.is_empty());

    // Resolve to a LID key. We borrow from Jid.user (CompactString) or from
    // get_current_lid (CompactString), then pass as &str to the storage layer.
    let resolved_lid: Option<wacore_binary::CompactString>;
    let sender_lid: &str = if let Some(ref lid_jid) = sender_lid_jid {
        &lid_jid.user
    } else {
        let from = match &from_jid {
            Some(jid) => jid,
            None => {
                warn!(target: "Client/TcToken", "privacy_token notification missing 'from' attribute");
                return;
            }
        };

        if from.is_lid() {
            &from.user
        } else {
            resolved_lid = client.lid_pn_cache.get_current_lid(&from.user).await;
            match &resolved_lid {
                Some(lid) => lid.as_str(),
                None => {
                    debug!(
                        target: "Client/TcToken",
                        "Cannot resolve LID for privacy_token sender {}, storing under PN",
                        from.observe()
                    );
                    &from.user
                }
            }
        }
    };

    // Parse the token data from the notification
    let received_tokens = match parse_privacy_token_notification(node) {
        Ok(tokens) => tokens,
        Err(e) => {
            warn!(target: "Client/TcToken", "Failed to parse privacy_token notification: {e}");
            return;
        }
    };

    if received_tokens.is_empty() {
        debug!(target: "Client/TcToken", "privacy_token notification had no trusted_contact tokens");
        return;
    }

    let backend = client.persistence_manager.backend();
    let mut token_stored = false;

    for received in &received_tokens {
        match backend.get_tc_token(sender_lid).await {
            Ok(Some(existing)) => {
                // Skip if token bytes are identical and timestamp hasn't advanced
                if existing.token == received.token {
                    if received.timestamp > existing.token_timestamp {
                        // Same bytes but newer timestamp — refresh to prevent premature pruning
                        let refreshed = TcTokenEntry {
                            token_timestamp: received.timestamp,
                            ..existing
                        };
                        if let Err(e) = backend.put_tc_token(sender_lid, &refreshed).await {
                            warn!(target: "Client/TcToken", "Failed to refresh tc_token timestamp for {}: {e}", sender_lid);
                        }
                    }
                    continue;
                }

                // Timestamp monotonicity guard: only store if incoming >= existing
                if received.timestamp < existing.token_timestamp {
                    debug!(
                        target: "Client/TcToken",
                        "Skipping older token for {} (incoming={}, existing={})",
                        sender_lid, received.timestamp, existing.token_timestamp
                    );
                    continue;
                }

                // Preserve existing sender_timestamp when updating token
                let entry = TcTokenEntry {
                    token: received.token.clone(),
                    token_timestamp: received.timestamp,
                    sender_timestamp: existing.sender_timestamp,
                };

                if let Err(e) = backend.put_tc_token(sender_lid, &entry).await {
                    warn!(target: "Client/TcToken", "Failed to update tc_token for {}: {e}", sender_lid);
                } else {
                    debug!(target: "Client/TcToken", "Updated tc_token for {} (t={})", sender_lid, received.timestamp);
                    token_stored = true;
                }
            }
            Ok(None) => {
                // New token — no existing entry
                let entry = TcTokenEntry {
                    token: received.token.clone(),
                    token_timestamp: received.timestamp,
                    sender_timestamp: None,
                };

                if let Err(e) = backend.put_tc_token(sender_lid, &entry).await {
                    warn!(target: "Client/TcToken", "Failed to store tc_token for {}: {e}", sender_lid);
                } else {
                    debug!(target: "Client/TcToken", "Stored new tc_token for {} (t={})", sender_lid, received.timestamp);
                    token_stored = true;
                }
            }
            Err(e) => {
                warn!(target: "Client/TcToken", "Failed to read tc_token for {}: {e}, skipping", sender_lid);
            }
        }
    }

    // Re-subscribe presence with the updated token.
    if token_stored
        && let Some(from) = &from_jid
        && let Err(e) = client.presence().re_subscribe_when_active(from).await
    {
        debug!(target: "Client/TcToken", "Failed to re-subscribe presence for {}: {e}", from.observe());
    }
}

/// Handle business notification (WhatsApp Web: `WAWebHandleBusinessNotification`).
async fn handle_business_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let notification = match BusinessNotification::try_parse(node) {
        Ok(n) => n,
        Err(e) => {
            warn!(target: "Client/Business", "Failed to parse business notification: {e}");
            return;
        }
    };

    debug!(
        target: "Client/Business",
        "Business notification: from={}, type={}, jid={:?}",
        notification.from.observe(),
        notification.notification_type,
        notification.jid
    );

    let update_type = BusinessUpdateType::from(notification.notification_type.clone());
    let verified_name = notification
        .verified_name
        .as_ref()
        .and_then(|vn| vn.name.clone());

    let event = Event::BusinessStatusUpdate(BusinessStatusUpdate {
        jid: notification.from.clone(),
        update_type,
        timestamp: wacore::time::from_secs_or_now(notification.timestamp),
        target_jid: notification.jid.clone(),
        hash: notification.hash.clone(),
        verified_name,
        product_ids: notification.product_ids.clone(),
        collection_ids: notification.collection_ids.clone(),
        subscriptions: notification.subscriptions.clone(),
    });

    match notification.notification_type {
        wacore::stanza::business::BusinessNotificationType::RemoveJid
        | wacore::stanza::business::BusinessNotificationType::RemoveHash => {
            info!(
                target: "Client/Business",
                "Contact {} is no longer a business account",
                notification.from.observe()
            );
        }
        wacore::stanza::business::BusinessNotificationType::VerifiedNameJid
        | wacore::stanza::business::BusinessNotificationType::VerifiedNameHash => {
            if let Some(name) = &notification
                .verified_name
                .as_ref()
                .and_then(|vn| vn.name.as_ref())
            {
                info!(
                    target: "Client/Business",
                    "Contact {} verified business name: {}",
                    notification.from.observe(),
                    name
                );
            }
        }
        wacore::stanza::business::BusinessNotificationType::Profile
        | wacore::stanza::business::BusinessNotificationType::ProfileHash => {
            debug!(
                target: "Client/Business",
                "Contact {} business profile updated (hash: {:?})",
                notification.from.observe(),
                notification.hash
            );
        }
        _ => {}
    }

    client.core.event_bus.dispatch(event);
}

/// Handle profile picture change notifications.
///
/// Matches WhatsApp Web's `WAWebHandleProfilePicNotification`.
///
/// Structure:
/// ```xml
/// <notification type="picture" from="user@s.whatsapp.net" t="1234567890" id="...">
///   <set jid="user@s.whatsapp.net" id="pic_id" author="author@s.whatsapp.net"/>
/// </notification>
/// ```
///
/// Or for removal (no child or `<delete>` child):
/// ```xml
/// <notification type="picture" from="user@s.whatsapp.net" t="1234567890" id="...">
///   <delete jid="user@s.whatsapp.net"/>
/// </notification>
/// ```
fn handle_picture_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let from = crate::require_from_jid!(
        node,
        target: "Client/Picture",
        "picture notification"
    );

    let timestamp = notification_timestamp(node);

    // Look for <set>, <delete>, or <request> child to determine the action.
    // WhatsApp Web has two formats:
    // - With `jid` attr: direct update for that JID
    // - With `hash` attr (no `jid`): side contact, resolved via contact hash lookup
    let (jid, author, removed, picture_id) = if let Some(set_node) = node.get_optional_child("set")
    {
        let jid = set_node.attrs().optional_jid("jid").unwrap_or_else(|| {
            if set_node.attrs().optional_string("hash").is_some() {
                debug!(
                    target: "Client/Picture",
                    "Hash-based picture notification (no jid), using from={}", from.observe()
                );
            }
            from.clone()
        });
        let author = set_node.attrs().optional_jid("author");
        let pic_id = set_node
            .attrs()
            .optional_string("id")
            .map(|s| s.to_string());
        (jid, author, false, pic_id)
    } else if let Some(delete_node) = node.get_optional_child("delete") {
        let jid = delete_node
            .attrs()
            .optional_jid("jid")
            .unwrap_or_else(|| from.clone());
        let author = delete_node.attrs().optional_jid("author");
        (jid, author, true, None)
    } else {
        // No <set> or <delete> child. Check if notification has no children at all,
        // which WhatsApp uses as a deletion signal (bare notification).
        let children = node.children().map(|c| c.len()).unwrap_or(0);
        if children == 0 {
            let jid = node
                .attrs()
                .optional_jid("jid")
                .unwrap_or_else(|| from.clone());
            let author = node.attrs().optional_jid("author");
            (jid, author, true, None)
        } else {
            // Unknown child type (e.g., "request", "set_avatar") — log and skip
            let child_tag = node
                .children()
                .and_then(|c| c.first().map(|n| n.tag.as_ref()));
            debug!(
                target: "Client/Picture",
                "Ignoring picture notification with child {:?} from {}", child_tag, from.observe()
            );
            return;
        }
    };

    debug!(
        target: "Client/Picture",
        "Picture {}: jid={}, author={:?}, pic_id={:?}",
        if removed { "removed" } else { "updated" },
        jid.observe(), author, picture_id
    );

    let event = Event::PictureUpdate(PictureUpdate {
        jid,
        author,
        timestamp,
        removed,
        picture_id,
    });
    client.core.event_bus.dispatch(event);
}

/// Handle status/about text change notifications.
///
/// Matches WhatsApp Web's `WAWebHandleAboutNotification`.
///
/// Structure:
/// ```xml
/// <notification type="status" from="user@s.whatsapp.net" t="1234567890" notify="PushName">
///   <set>new status text</set>
/// </notification>
/// ```
fn handle_status_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let from = crate::require_from_jid!(
        node,
        target: "Client/Status",
        "status notification"
    );

    let timestamp = notification_timestamp(node);

    if let Some(set_node) = node.get_optional_child("set") {
        let status_text = match set_node.content.as_deref() {
            Some(NodeContentRef::String(s)) => s.to_string(),
            Some(NodeContentRef::Bytes(b)) => String::from_utf8_lossy(b.as_ref()).into_owned(),
            _ => String::new(),
        };

        debug!(
            target: "Client/Status",
            "Status update from {} (length={})", from.observe(), status_text.len()
        );

        let event = Event::UserAboutUpdate(UserAboutUpdate {
            jid: from,
            status: status_text,
            timestamp,
        });
        client.core.event_bus.dispatch(event);
    } else {
        debug!(
            target: "Client/Status",
            "Status notification from {} without <set> child, ignoring", from.observe()
        );
    }
}

fn notification_timestamp(node: &NodeRef<'_>) -> chrono::DateTime<chrono::Utc> {
    node.attrs()
        .optional_u64("t")
        .and_then(|t| i64::try_from(t).ok())
        .and_then(wacore::time::from_secs)
        .unwrap_or_else(wacore::time::now_utc)
}

/// Learn LID-PN mappings from a contacts modify notification.
///
/// WA Web (`WAWebHandleContactNotification` → `WAWebDBCreateLidPnMappings`):
/// The `<modify>` child carries four attributes:
/// - `old` / `new` — old and new PN (phone number) JIDs
/// - `old_lid` / `new_lid` — old and new LID JIDs (optional)
///
/// When both `old_lid` and `new_lid` are present, WA Web creates two mappings:
/// `{ lid: old_lid, pn: old }` and `{ lid: new_lid, pn: new }`.
async fn learn_contact_modify_mappings(
    client: &Arc<Client>,
    old_pn: &Jid,
    new_pn: &Jid,
    old_lid: Option<&Jid>,
    new_lid: Option<&Jid>,
) {
    // WA Web: createLidPnMappings({mappings:[{lid:oldLid,pn:oldJid},{lid:newLid,pn:newJid}]})
    if let (Some(old_lid), Some(new_lid)) = (old_lid, new_lid) {
        for (lid, pn) in [(old_lid, old_pn), (new_lid, new_pn)] {
            if let Err(e) = client
                .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::DeviceNotification)
                .await
            {
                warn!(
                    target: "Client/Contacts",
                    "Failed to add LID-PN mapping lid={} pn={}: {e}",
                    lid.observe(), pn.observe()
                );
            }
        }
    } else {
        debug!(
            target: "Client/Contacts",
            "Contacts modify without old_lid/new_lid, skipping LID-PN mapping (old={}, new={})",
            old_pn.observe(), new_pn.observe()
        );
    }
}

/// Handle contact change notifications.
///
/// WA Web: `WAWebHandleContactNotification`
///
/// These stanzas are sent as `<notification type="contacts">` with a single child action:
/// - `<update jid="..."/>` — contact profile changed. Consumers should
///   invalidate cached presence/profile picture (WA Web resets PresenceCollection
///   and refreshes profile pic thumb).
/// - `<modify old="..." new="..." old_lid="..." new_lid="..."/>` — contact
///   changed phone number. Creates LID-PN mappings when LID attrs present.
/// - `<sync after="..."/>` — server requests full contact re-sync.
/// - `<add .../>` or `<remove .../>` — lightweight roster changes (ACK only).
async fn handle_contacts_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let timestamp = notification_timestamp(node);

    let Some(child) = node.children().and_then(|children| children.first()) else {
        debug!(
            target: "Client/Contacts",
            "Ignoring contacts notification without child action"
        );
        return;
    };

    match child.tag.as_ref() {
        "update" => {
            let Some(jid) = child.attrs().optional_jid("jid") else {
                // WA Web: when no jid, tries hash-based lookup against local contacts
                // (first 4 chars of contact userhash). If no match, it's a no-op.
                // We don't maintain a userhash index, so just ack and move on.
                debug!(target: "Client/Contacts", "contacts update with hash but no jid, ignoring (hash={:?})",
                    child.attrs().optional_string("hash"));
                return;
            };

            debug!(target: "Client/Contacts", "Contact updated for {}", jid.observe());
            client
                .core
                .event_bus
                .dispatch(Event::ContactUpdated(ContactUpdated { jid, timestamp }));
        }
        "modify" => {
            // WA Web: old/new are PN JIDs, old_lid/new_lid are optional LID JIDs.
            let mut child_attrs = child.attrs();
            let Some(old_jid) = child_attrs.optional_jid("old") else {
                warn!(target: "Client/Contacts", "contacts modify missing 'old' attribute");
                return;
            };
            let Some(new_jid) = child_attrs.optional_jid("new") else {
                warn!(target: "Client/Contacts", "contacts modify missing 'new' attribute");
                return;
            };
            let old_lid = child_attrs.optional_jid("old_lid");
            let new_lid = child_attrs.optional_jid("new_lid");

            learn_contact_modify_mappings(
                client,
                &old_jid,
                &new_jid,
                old_lid.as_ref(),
                new_lid.as_ref(),
            )
            .await;

            debug!(
                target: "Client/Contacts",
                "Contact number changed: {} -> {} (old_lid={:?}, new_lid={:?})",
                old_jid.observe(), new_jid.observe(), old_lid, new_lid
            );
            client
                .core
                .event_bus
                .dispatch(Event::ContactNumberChanged(ContactNumberChanged {
                    old_jid,
                    new_jid,
                    old_lid,
                    new_lid,
                    timestamp,
                }));
        }
        "sync" => {
            let after = child
                .attrs()
                .optional_u64("after")
                .and_then(|after| wacore::time::from_secs(after as i64));

            debug!(
                target: "Client/Contacts",
                "Contact sync requested after {:?}",
                after
            );
            client
                .core
                .event_bus
                .dispatch(Event::ContactSyncRequested(ContactSyncRequested {
                    after,
                    timestamp,
                }));
        }
        "add" | "remove" => {
            debug!(
                target: "Client/Contacts",
                "Contact {} notification handled without extra work",
                child.tag
            );
        }
        other => {
            debug!(
                target: "Client/Contacts",
                "Ignoring unknown contacts notification child {:?}",
                other
            );
        }
    }
}

/// Handle w:gp2 group notifications.
///
/// Parses all child actions (participant changes, setting changes, metadata updates)
/// and dispatches typed `Event::GroupUpdate` events for each.
///
/// Reference: WhatsApp Web `WAWebHandleGroupNotification` (Ri7Gf1BxhsX.js:12556-12962)
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.group", level = "debug", skip_all)
)]
async fn handle_group_notification(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    let notification = match GroupNotification::try_from_node_ref(node.get()) {
        Some(n) => n,
        None => {
            warn!(target: "Client/Group", "w:gp2 notification missing 'from' attribute");
            return;
        }
    };

    let timestamp = i64::try_from(notification.timestamp)
        .ok()
        .and_then(wacore::time::from_secs)
        .unwrap_or_else(wacore::time::now_utc);

    for action in notification.actions {
        // Granularly patch group cache instead of invalidating — matches WA Web's
        // addParticipantInfo / removeParticipantInfo pattern and avoids a
        // group metadata IQ round-trip.
        match &action {
            GroupNotificationAction::Add { participants, .. } => {
                let group_cache = client.get_group_cache().await;
                if let Some(info) = group_cache.get(&notification.group_jid).await {
                    let mut info = Arc::unwrap_or_clone(info);
                    info.add_participants(
                        participants
                            .iter()
                            .map(|p| (&p.jid, p.phone_number.as_ref())),
                    );
                    client
                        .persist_group_metadata(&notification.group_jid, &info)
                        .await;
                    group_cache
                        .insert(notification.group_jid.clone(), Arc::new(info))
                        .await;
                    debug!(
                        target: "Client/Group",
                        "Patched group cache for {}: added {} participants",
                        notification.group_jid.observe(), participants.len()
                    );
                } else {
                    // Cache expired: can't patch in place, so drop the now-stale blob.
                    debug!(
                        target: "Client/Group",
                        "Group cache expired for {}: invalidating persisted metadata (add)",
                        notification.group_jid.observe()
                    );
                    client
                        .invalidate_persisted_group_metadata(&notification.group_jid)
                        .await;
                }
            }
            GroupNotificationAction::Remove { participants, .. } => {
                let users: Vec<&str> = participants.iter().map(|p| p.jid.user.as_str()).collect();
                let group_cache = client.get_group_cache().await;
                if let Some(info) = group_cache.get(&notification.group_jid).await {
                    let mut info = Arc::unwrap_or_clone(info);
                    info.remove_participants(&users);
                    client
                        .persist_group_metadata(&notification.group_jid, &info)
                        .await;
                    group_cache
                        .insert(notification.group_jid.clone(), Arc::new(info))
                        .await;
                    debug!(
                        target: "Client/Group",
                        "Patched group cache for {}: removed {} participants",
                        notification.group_jid.observe(), participants.len()
                    );
                } else {
                    // Cache expired: can't patch in place, so drop the now-stale blob.
                    debug!(
                        target: "Client/Group",
                        "Group cache expired for {}: invalidating persisted metadata (remove)",
                        notification.group_jid.observe()
                    );
                    client
                        .invalidate_persisted_group_metadata(&notification.group_jid)
                        .await;
                }
                client
                    .rotate_sender_key_on_participant_remove(
                        &notification.group_jid.to_string(),
                        &users,
                    )
                    .await;
            }
            _ => {}
        }

        debug!(
            target: "Client/Group",
            "Group notification: group={}, action={}",
            notification.group_jid.observe(), action.tag_name()
        );

        client
            .core
            .event_bus
            .dispatch(Event::GroupUpdate(GroupUpdate {
                group_jid: notification.group_jid.clone(),
                participant: notification.participant.clone(),
                participant_pn: notification.participant_pn.clone(),
                timestamp,
                is_lid_addressing_mode: notification.is_lid_addressing_mode,
                action,
            }));
    }

    // Also dispatch legacy generic notification for backward compatibility
    client
        .core
        .event_bus
        .dispatch(Event::Notification(Arc::clone(&node)));
}

/// Handle `<notification type="newsletter">` — live updates with reaction counts.
///
/// Format:
/// ```xml
/// <notification from="NL_JID" type="newsletter" id="..." t="...">
///   <live_updates>
///     <messages jid="NL_JID" t="...">
///       <message server_id="123" ...>
///         <reactions><reaction code="👍" count="3"/></reactions>
///       </message>
///     </messages>
///   </live_updates>
/// </notification>
/// ```
fn handle_newsletter_notification(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    use crate::features::newsletter::parse_reaction_counts;
    use wacore::types::events::{
        NewsletterLiveUpdate, NewsletterLiveUpdateMessage, NewsletterLiveUpdateReaction,
    };

    let nr = node.get();

    let Some(newsletter_jid) = nr.attrs().optional_jid("from") else {
        return;
    };

    if let Some(live_updates) = nr.get_optional_child("live_updates")
        && let Some(messages_node) = live_updates.get_optional_child("messages")
        && let Some(children) = messages_node.children()
    {
        let messages: Vec<_> = children
            .iter()
            .filter(|n| n.tag.as_ref() == "message")
            .filter_map(|msg_node| {
                let server_id = msg_node
                    .get_attr("server_id")
                    .map(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok())?;

                let reactions = parse_reaction_counts(msg_node)
                    .into_iter()
                    .map(|r| NewsletterLiveUpdateReaction {
                        code: r.code,
                        count: r.count,
                    })
                    .collect();

                Some(NewsletterLiveUpdateMessage {
                    server_id,
                    reactions,
                })
            })
            .collect();

        if !messages.is_empty() {
            client
                .core
                .event_bus
                .dispatch(Event::NewsletterLiveUpdate(NewsletterLiveUpdate {
                    newsletter_jid,
                    messages,
                }));
        }
    }

    // Also dispatch raw notification for backward compatibility
    client
        .core
        .event_bus
        .dispatch(Event::Notification(Arc::clone(&node)));
}

/// `<notification type="mex"><update op_name="…">{json}</update></notification>`
/// Routed by `op_name` so the dispatcher survives bundle rebuilds.
fn handle_mex_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let Some(update_node) = node.get_optional_child("update") else {
        warn!(
            target: "Client/Mex",
            "mex notification missing <update> child: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    let Some(op_name) = update_node.attrs().optional_string("op_name") else {
        warn!(
            target: "Client/Mex",
            "mex notification <update> missing op_name attribute: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    // `from_str` skips the redundant UTF-8 validation `from_slice` would
    // do on a `&str`.
    let parsed = match update_node.content.as_deref() {
        Some(NodeContentRef::String(s)) => serde_json::from_str(s),
        Some(NodeContentRef::Bytes(b)) => serde_json::from_slice(b.as_ref()),
        _ => {
            warn!(target: "Client/Mex", "mex notification op={op_name} has no JSON body");
            return;
        }
    };
    let payload: serde_json::Value = match parsed {
        Ok(v) => v,
        Err(e) => {
            warn!(target: "Client/Mex", "mex notification op={op_name} JSON parse failed: {e}");
            return;
        }
    };

    let mut attrs = node.attrs();
    let from = attrs.optional_jid("from");
    let stanza_id = attrs.optional_string("id").map(|s| s.into_owned());
    let offline = attrs.optional_string("offline").map(|s| s.into_owned());
    let op_name = op_name.into_owned();

    debug!(
        target: "Client/Mex",
        "mex notification received: op_name={op_name} offline={}",
        offline.is_some()
    );
    client
        .core
        .event_bus
        .dispatch(Event::MexNotification(MexNotification {
            op_name,
            from,
            stanza_id,
            offline,
            payload,
        }));
}

/// Handle `<notification type="disappearing_mode">` — a contact changed
/// their default disappearing messages setting.
///
/// WA Web: `WAWebHandleDisappearingModeNotification` parses the
/// `<disappearing_mode duration="..." t="..."/>` child and calls
/// `WAWebUpdateDisappearingModeForContact` which applies the update only
/// if the new timestamp is newer than the stored one.
///
/// We dispatch `Event::DisappearingModeChanged` and let consumers decide
/// how to persist/apply it.
fn handle_disappearing_mode_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let mut attrs = node.attrs();
    let from = attrs.jid("from").to_non_ad();

    let Some(dm_node) = node.get_optional_child("disappearing_mode") else {
        warn!(
            "disappearing_mode notification missing <disappearing_mode> child: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    let mut dm_attrs = dm_node.attrs();

    // WA Web: `t.attrInt("duration", 0)` — defaults to 0 (disabled).
    let duration = dm_attrs
        .optional_string("duration")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    // WA Web: `t.attrTime("t")` — required, no default.
    let Some(setting_timestamp) = dm_attrs
        .optional_string("t")
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(wacore::time::from_secs)
    else {
        warn!(
            "disappearing_mode notification missing or invalid 't' attribute: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    debug!(
        "Disappearing mode changed for {}: duration={}s, t={}",
        from.observe(),
        duration,
        setting_timestamp
    );

    client
        .core
        .event_bus
        .dispatch(Event::DisappearingModeChanged(
            wacore::types::events::DisappearingModeChanged {
                from,
                duration,
                setting_timestamp,
            },
        ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestEventCollector, create_test_client};
    use std::sync::Arc;
    use wacore::stanza::devices::DeviceNotificationType;
    use wacore::types::events::DeviceListUpdateType;
    use wacore_binary::Node;
    use wacore_binary::builder::NodeBuilder;

    fn node_to_arc(node: Node) -> Arc<OwnedNodeRef> {
        crate::test_utils::node_to_owned_ref(&node)
    }

    #[test]
    fn test_parse_device_add_notification() {
        // Per WhatsApp Web: add operation has single device + key-index-list
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("add")
                .children([
                    NodeBuilder::new("device")
                        .attr("jid", "1234567890:1@s.whatsapp.net")
                        .build(),
                    NodeBuilder::new("key-index-list")
                        .attr("ts", "1000")
                        .bytes(vec![0x01, 0x02, 0x03])
                        .build(),
                ])
                .build()])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        assert_eq!(parsed.operation.operation_type, DeviceNotificationType::Add);
        assert_eq!(parsed.operation.device_ids(), vec![1]);
        // Verify key index info
        assert!(parsed.operation.key_index.is_some());
        assert_eq!(parsed.operation.key_index.as_ref().unwrap().timestamp, 1000);
    }

    #[test]
    fn test_parse_device_remove_notification() {
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("remove")
                .children([
                    NodeBuilder::new("device")
                        .attr("jid", "1234567890:3@s.whatsapp.net")
                        .build(),
                    NodeBuilder::new("key-index-list")
                        .attr("ts", "2000")
                        .build(),
                ])
                .build()])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        assert_eq!(
            parsed.operation.operation_type,
            DeviceNotificationType::Remove
        );
        assert_eq!(parsed.operation.device_ids(), vec![3]);
    }

    #[test]
    fn test_parse_device_update_notification_with_hash() {
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("update")
                .attr("hash", "2:abcdef123456")
                .build()])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        assert_eq!(
            parsed.operation.operation_type,
            DeviceNotificationType::Update
        );
        assert_eq!(
            parsed.operation.contact_hash,
            Some("2:abcdef123456".to_string())
        );
        // Update operations don't have devices (just hash for lookup)
        assert!(parsed.operation.devices.is_empty());
    }

    #[test]
    fn test_parse_empty_device_notification_fails() {
        // Per WhatsApp Web: at least one operation (add/remove/update) is required
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .build();

        let result = DeviceNotification::try_parse(&node.as_node_ref());
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("missing required operation")
        );
    }

    #[test]
    fn test_parse_multiple_operations_uses_priority() {
        // Per WhatsApp Web: only ONE operation is processed with priority remove > add > update
        // If both remove and add are present, remove should be processed
        let node = NodeBuilder::new("notification")
            .attr("type", "devices")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([
                NodeBuilder::new("add")
                    .children([
                        NodeBuilder::new("device")
                            .attr("jid", "1234567890:5@s.whatsapp.net")
                            .build(),
                        NodeBuilder::new("key-index-list")
                            .attr("ts", "3000")
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new("remove")
                    .children([
                        NodeBuilder::new("device")
                            .attr("jid", "1234567890:2@s.whatsapp.net")
                            .build(),
                        NodeBuilder::new("key-index-list")
                            .attr("ts", "3001")
                            .build(),
                    ])
                    .build(),
            ])
            .build();

        let parsed = DeviceNotification::try_parse(&node.as_node_ref()).unwrap();
        // Should process remove, not add (priority: remove > add > update)
        assert_eq!(
            parsed.operation.operation_type,
            DeviceNotificationType::Remove
        );
        assert_eq!(parsed.operation.device_ids(), vec![2]);
    }

    #[test]
    fn test_device_list_update_type_from_notification_type() {
        assert_eq!(
            DeviceListUpdateType::from(DeviceNotificationType::Add),
            DeviceListUpdateType::Add
        );
        assert_eq!(
            DeviceListUpdateType::from(DeviceNotificationType::Remove),
            DeviceListUpdateType::Remove
        );
        assert_eq!(
            DeviceListUpdateType::from(DeviceNotificationType::Update),
            DeviceListUpdateType::Update
        );
    }

    // Tests for account_sync device parsing

    #[test]
    fn test_parse_account_sync_device_list_basic() {
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:FnEWjS13")
            .children([
                NodeBuilder::new("device")
                    .attr("jid", "15551234567@s.whatsapp.net")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "15551234567:64@s.whatsapp.net")
                    .attr("key-index", "2")
                    .build(),
            ])
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        assert_eq!(devices.len(), 2);

        // Primary device (device 0)
        assert_eq!(devices[0].jid.user, "15551234567");
        assert_eq!(devices[0].jid.device, 0);
        assert_eq!(devices[0].key_index, None);

        // Companion device (device 64)
        assert_eq!(devices[1].jid.user, "15551234567");
        assert_eq!(devices[1].jid.device, 64);
        assert_eq!(devices[1].key_index, Some(2));
    }

    #[test]
    fn test_parse_account_sync_device_list_with_key_index_list() {
        // Real-world structure includes <key-index-list> which should be ignored
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:FnEWjS13")
            .children([
                NodeBuilder::new("device")
                    .attr("jid", "15551234567@s.whatsapp.net")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "15551234567:77@s.whatsapp.net")
                    .attr("key-index", "15")
                    .build(),
                NodeBuilder::new("key-index-list")
                    .attr("ts", "1766612162")
                    .bytes(vec![0x01, 0x02, 0x03]) // Simulated signed bytes
                    .build(),
            ])
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        // Should only parse <device> tags, not <key-index-list>
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].jid.device, 0);
        assert_eq!(devices[1].jid.device, 77);
        assert_eq!(devices[1].key_index, Some(15));
    }

    #[test]
    fn test_parse_account_sync_device_list_empty() {
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:FnEWjS13")
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        assert!(devices.is_empty());
    }

    #[test]
    fn test_parse_account_sync_device_list_multiple_devices() {
        let devices_node = NodeBuilder::new("devices")
            .attr("dhash", "2:XYZ123")
            .children([
                NodeBuilder::new("device")
                    .attr("jid", "1234567890@s.whatsapp.net")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "1234567890:1@s.whatsapp.net")
                    .attr("key-index", "1")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "1234567890:2@s.whatsapp.net")
                    .attr("key-index", "5")
                    .build(),
                NodeBuilder::new("device")
                    .attr("jid", "1234567890:3@s.whatsapp.net")
                    .attr("key-index", "10")
                    .build(),
            ])
            .build();

        let devices = parse_account_sync_device_list(&devices_node.as_node_ref());
        assert_eq!(devices.len(), 4);

        // Verify device IDs are correctly parsed
        assert_eq!(devices[0].jid.device, 0);
        assert_eq!(devices[1].jid.device, 1);
        assert_eq!(devices[2].jid.device, 2);
        assert_eq!(devices[3].jid.device, 3);

        // Verify key indexes
        assert_eq!(devices[0].key_index, None);
        assert_eq!(devices[1].key_index, Some(1));
        assert_eq!(devices[2].key_index, Some(5));
        assert_eq!(devices[3].key_index, Some(10));
    }

    // ── disappearing_mode notification parsing tests ─────────────────────

    /// Helper: parse a disappearing_mode notification node the same way
    /// the handler does, returning `(duration, setting_timestamp)` or `None`
    /// on validation failure.
    fn parse_disappearing_mode(node: &Node) -> Option<(u32, i64)> {
        let dm_node = node.get_optional_child("disappearing_mode")?;
        let mut dm_attrs = dm_node.attrs();
        let duration = dm_attrs
            .optional_string("duration")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let setting_timestamp = dm_attrs
            .optional_string("t")
            .and_then(|s| s.parse::<i64>().ok())
            .filter(|&t| wacore::time::from_secs(t).is_some())?;
        Some((duration, setting_timestamp))
    }

    #[test]
    fn test_parse_disappearing_mode_valid() {
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("type", "disappearing_mode")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("duration", "86400")
                .attr("t", "1773519041")
                .build()])
            .build();

        let (duration, ts) = parse_disappearing_mode(&node).expect("should parse");
        assert_eq!(duration, 86400);
        assert_eq!(ts, 1773519041);
    }

    #[test]
    fn test_parse_disappearing_mode_disabled() {
        // duration=0 means disappearing messages disabled
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("duration", "0")
                .attr("t", "1773519041")
                .build()])
            .build();

        let (duration, ts) = parse_disappearing_mode(&node).expect("should parse");
        assert_eq!(duration, 0, "duration=0 means disabled");
        assert_eq!(ts, 1773519041);
    }

    #[test]
    fn test_parse_disappearing_mode_missing_child() {
        // No <disappearing_mode> child → returns None
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("type", "disappearing_mode")
            .build();

        assert!(
            parse_disappearing_mode(&node).is_none(),
            "should return None when child element is missing"
        );
    }

    #[test]
    fn test_parse_disappearing_mode_missing_timestamp() {
        // Missing 't' attribute → returns None (required field)
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("duration", "86400")
                .build()])
            .build();

        assert!(
            parse_disappearing_mode(&node).is_none(),
            "should return None when 't' attribute is missing"
        );
    }

    #[test]
    fn test_parse_disappearing_mode_missing_duration_defaults_to_zero() {
        // Missing duration defaults to 0 (WA Web: attrInt("duration", 0))
        let node = NodeBuilder::new("notification")
            .attr("from", "5511999999999@s.whatsapp.net")
            .children([NodeBuilder::new("disappearing_mode")
                .attr("t", "1773519041")
                .build()])
            .build();

        let (duration, _) = parse_disappearing_mode(&node).expect("should parse");
        assert_eq!(duration, 0, "missing duration should default to 0");
    }

    #[tokio::test]
    async fn test_contacts_update_dispatches_contact_updated_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-update-1")
            .attr("t", "1773519041")
            .children([NodeBuilder::new("update")
                .attr("jid", "5511999999999@s.whatsapp.net")
                .build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        assert!(
            events.len() == 1
                && matches!(
                    &*events[0],
                    Event::ContactUpdated(ContactUpdated { jid, .. })
                    if jid == &Jid::pn("5511999999999")
                )
        );
    }

    #[tokio::test]
    async fn test_contacts_modify_with_lid_creates_mappings() {
        // WA Web: old/new are PN JIDs, old_lid/new_lid are LID JIDs.
        // Creates two mappings: old_lid→old_pn AND new_lid→new_pn.
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-1")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511888888888@s.whatsapp.net")
                .attr("old_lid", "100000011111111@lid")
                .attr("new_lid", "100000022222222@lid")
                .build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        // Both LID-PN mappings should be created
        assert_eq!(
            client
                .lid_pn_cache
                .get_phone_number("100000011111111")
                .await,
            Some("5511999999999".to_string()),
            "old_lid should map to old PN"
        );
        assert_eq!(
            client
                .lid_pn_cache
                .get_phone_number("100000022222222")
                .await,
            Some("5511888888888".to_string()),
            "new_lid should map to new PN"
        );

        let events = collector.events();
        assert!(
            events.len() == 1
                && matches!(
                    &*events[0],
                    Event::ContactNumberChanged(ContactNumberChanged {
                        old_jid, new_jid, old_lid, new_lid, ..
                    })
                    if old_jid == &Jid::pn("5511999999999")
                        && new_jid == &Jid::pn("5511888888888")
                        && old_lid.is_some()
                        && new_lid.is_some()
                )
        );
    }

    #[tokio::test]
    async fn test_contacts_modify_without_lid_skips_mapping() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-2")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511888888888@s.whatsapp.net")
                .build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        // Event should still be dispatched, just without LID info
        assert_eq!(collector.events().len(), 1);
    }

    #[tokio::test]
    async fn test_contacts_sync_dispatches_contact_sync_requested_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-sync-1")
            .children([NodeBuilder::new("sync").attr("after", "1773519041").build()])
            .build();

        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        assert!(
            events.len() == 1
                && matches!(
                    &*events[0],
                    Event::ContactSyncRequested(ContactSyncRequested { after, .. })
                    if after.is_some()
                )
        );
    }

    #[tokio::test]
    async fn test_contacts_add_remove_do_not_dispatch_events() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        for tag in ["add", "remove"] {
            let node = NodeBuilder::new("notification")
                .attr("type", "contacts")
                .attr("from", "s.whatsapp.net")
                .attr("id", format!("contacts-{tag}-1"))
                .children([NodeBuilder::new(tag).build()])
                .build();
            handle_notification_impl(&client, node_to_arc(node)).await;
        }

        assert!(
            collector.events().is_empty(),
            "add/remove should not dispatch events"
        );
    }

    #[tokio::test]
    async fn test_contacts_empty_notification_ignored() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // No child element
        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-empty-1")
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "empty contacts notification should not dispatch events"
        );
    }

    /// Same PN on both sides is still dispatched as a ContactNumberChanged
    /// event (with `old_jid == new_jid`). WA Web JS has no special guard for
    /// this case either; the LID mapping update is a no-op when LIDs are
    /// also equal. Consumers can filter if they care.
    #[tokio::test]
    async fn test_contacts_modify_same_jid_still_dispatches() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-same")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511999999999@s.whatsapp.net")
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &*events[0],
            Event::ContactNumberChanged(ContactNumberChanged { old_jid, new_jid, .. })
                if old_jid == new_jid
        ));
    }

    /// Partial LID (only `new_lid`, missing `old_lid`) must NOT create any
    /// LID-PN mapping, since WA Web requires BOTH for createLidPnMappings.
    #[tokio::test]
    async fn test_contacts_modify_partial_lid_skips_mappings() {
        let client = create_test_client().await;

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-partial")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .attr("new", "5511888888888@s.whatsapp.net")
                .attr("new_lid", "100000022222222@lid")
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            client
                .lid_pn_cache
                .get_phone_number("100000022222222")
                .await
                .is_none(),
            "no mapping should be created when old_lid is missing"
        );
    }

    /// Missing `new` attribute: the parser should warn and not dispatch
    /// anything, mirroring WA Web's parser error path.
    #[tokio::test]
    async fn test_contacts_modify_missing_new_attr_drops_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "s.whatsapp.net")
            .attr("id", "contacts-modify-bad")
            .children([NodeBuilder::new("modify")
                .attr("old", "5511999999999@s.whatsapp.net")
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(collector.events().is_empty());
    }

    /// Group `w:gp2` change_number: the parsed action must carry the new
    /// owner from the child's `jid` attr and the sub_group_suggestions from
    /// `<sub_group_suggestion jid=.../>` children. The old owner is the
    /// notification's top-level `participant` attribute, surfaced on
    /// `GroupUpdate.participant`.
    #[tokio::test]
    async fn test_group_change_number_dispatches_with_new_owner_and_suggestions() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "w:gp2")
            .attr("from", "120363000000000000@g.us")
            .attr("participant", "5511999999999@s.whatsapp.net")
            .attr("id", "gp2-change-1")
            .attr("t", "1773519041")
            .children([NodeBuilder::new("change_number")
                .attr("jid", "5511888888888@s.whatsapp.net")
                .children([
                    NodeBuilder::new("sub_group_suggestion")
                        .attr("jid", "120363111111111111@g.us")
                        .build(),
                    NodeBuilder::new("sub_group_suggestion")
                        .attr("jid", "120363222222222222@g.us")
                        .build(),
                ])
                .build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let events = collector.events();
        let group_update = events
            .iter()
            .find_map(|e| match &**e {
                Event::GroupUpdate(u) => Some(u),
                _ => None,
            })
            .expect("expected GroupUpdate");

        assert_eq!(
            group_update.participant.as_ref().map(|j| j.user.as_str()),
            Some("5511999999999"),
            "old owner comes from notification.participant"
        );
        match &group_update.action {
            GroupNotificationAction::ChangeNumber {
                new_owner,
                sub_group_suggestions,
            } => {
                assert_eq!(
                    new_owner.as_ref().map(|j| j.user.as_str()),
                    Some("5511888888888")
                );
                assert_eq!(sub_group_suggestions.len(), 2);
            }
            other => panic!("expected ChangeNumber, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_contacts_update_hash_only_ignored() {
        // WA Web sends <update hash="Quvc"/> without jid when using hash-based lookup.
        // We don't maintain a userhash index, so this should be a no-op.
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "contacts")
            .attr("from", "551199887766@s.whatsapp.net")
            .attr("id", "3251801952")
            .attr("t", "1773668072")
            .children([NodeBuilder::new("update").attr("hash", "Quvc").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "hash-only update without jid should not dispatch events"
        );
    }

    #[tokio::test]
    async fn test_identity_change_dispatches_event_and_invalidates_cache() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Pre-populate device registry so clear_device_record has something to clear
        let record = wacore::store::traits::DeviceListRecord {
            user: "5511999999999".into(),
            devices: vec![wacore::store::traits::DeviceInfo {
                device_id: 1,
                key_index: None,
            }],
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: Some(42),
        };
        client
            .device_registry_cache
            .insert("5511999999999".into(), Arc::new(record))
            .await;

        // Seed a stored identity so the had-prior-identity gate runs the full reset
        // (delete + notify), matching WA Web's `if (!isEmpty(loadIdentityKey(addr)))`.
        {
            use wacore::types::jid::JidExt;
            let target: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
            client
                .signal_cache
                .put_identity(&target.to_protocol_address(), &[7u8; 32])
                .await;
        }

        // Simulate identity change notification: type="encrypt" with <identity/> child
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("id", "identity-change-1")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        // Should have dispatched IdentityChange event
        let events = collector.events();
        assert!(
            events
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "should dispatch IdentityChange event, got: {:?}",
            events
        );

        // Device registry cache should be invalidated
        assert!(
            client
                .device_registry_cache
                .get("5511999999999")
                .await
                .is_none(),
            "device registry cache should be invalidated after identity change"
        );
    }

    #[tokio::test]
    async fn test_identity_change_ignores_self_primary() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Set our own JID so the self-check works
        client
            .persistence_manager
            .modify_device(|d| {
                d.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
            })
            .await;

        // Identity change FROM our own JID — should be ignored per WA Web's isMePrimary
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511999999999@s.whatsapp.net")
            .attr("id", "identity-change-self")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "self identity change should be ignored"
        );
    }

    #[tokio::test]
    async fn test_identity_change_ignores_companion_device() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511999999999:5@s.whatsapp.net")
            .attr("id", "identity-change-2")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector.events().is_empty(),
            "companion device identity change should be ignored"
        );
    }

    #[tokio::test]
    async fn test_local_identity_change_dispatches_implicit_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let sender: Jid = "5511777777777@s.whatsapp.net".parse().unwrap();
        handle_local_identity_change(&client, sender).await;

        let events = collector.events();
        // The event is dispatched last (after clear_device_record +
        // invalidate_device_cache), so observing it proves the handler ran to
        // completion. invalidate_device_cache itself is covered by
        // test_invalidate_device_cache_uses_correct_jid_types.
        let ic = events
            .iter()
            .find_map(|e| match &**e {
                Event::IdentityChange(ic) => Some(ic.clone()),
                _ => None,
            })
            .expect("local detection should dispatch IdentityChange");
        assert!(
            ic.implicit,
            "locally-detected identity change must set implicit=true"
        );
    }

    #[tokio::test]
    async fn test_local_identity_change_skips_self() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        client
            .persistence_manager
            .modify_device(|d| {
                d.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
            })
            .await;

        let sender: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        handle_local_identity_change(&client, sender).await;

        assert!(
            collector.events().is_empty(),
            "self identity change must never clear our own record"
        );
    }

    #[tokio::test]
    async fn test_local_identity_change_skips_companion_device() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let sender: Jid = "5511777777777:5@s.whatsapp.net".parse().unwrap();
        handle_local_identity_change(&client, sender).await;

        assert!(
            collector.events().is_empty(),
            "companion device identity change should be ignored"
        );
    }

    #[tokio::test]
    async fn test_identity_change_deletes_primary_session() {
        use wacore::libsignal::protocol::SessionRecord;
        use wacore::types::jid::JidExt;

        let client = create_test_client().await;

        let target_jid: Jid = "5511888888888@s.whatsapp.net".parse().unwrap();
        let addr = target_jid.to_protocol_address();

        // Pre-populate a session for the primary device
        client
            .signal_cache
            .put_session(&addr, SessionRecord::new_fresh())
            .await;
        client.signal_cache.put_identity(&addr, &[0u8; 32]).await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511888888888@s.whatsapp.net")
            .attr("id", "identity-change-3")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let backend = client.persistence_manager.backend();
        let has_session = client
            .signal_cache
            .has_session(&addr, &*backend)
            .await
            .unwrap();
        assert!(!has_session, "primary session should be deleted");

        let has_identity = client
            .signal_cache
            .get_identity(&addr, &*backend)
            .await
            .unwrap();
        assert!(has_identity.is_none(), "identity key should be deleted");
    }

    #[tokio::test]
    async fn test_identity_change_rotates_status_sender_key() {
        use wacore::libsignal::store::sender_key_name::SenderKeyName;
        use wacore::types::jid::JidExt;

        let client = create_test_client().await;

        // Set our own JID so sender key deletion knows which namespaces to check
        let own_jid: Jid = "5511777777777@s.whatsapp.net".parse().unwrap();
        client
            .persistence_manager
            .modify_device(|d| {
                d.pn = Some(own_jid.clone());
            })
            .await;

        // Pre-populate a sender key for status@broadcast
        let sk_name =
            SenderKeyName::from_parts("status@broadcast", own_jid.to_protocol_address().as_str());
        let sk_record = wacore::libsignal::protocol::SenderKeyRecord::new_empty();
        client
            .signal_cache
            .put_sender_key(&sk_name, sk_record)
            .await;

        // Seed a stored identity for the changed user so the had-prior-identity gate
        // runs the reset (which rotates the status sender key).
        let changed: Jid = "5511888888888@s.whatsapp.net".parse().unwrap();
        client
            .signal_cache
            .put_identity(&changed.to_protocol_address(), &[7u8; 32])
            .await;

        // Fire identity change for a different user
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511888888888@s.whatsapp.net")
            .attr("id", "identity-change-4")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        let backend = client.persistence_manager.backend();
        let sk = client
            .signal_cache
            .get_sender_key(&sk_name, &*backend)
            .await
            .unwrap();
        assert!(
            sk.is_none(),
            "status@broadcast sender key should be deleted for forward secrecy"
        );
    }

    #[tokio::test]
    async fn test_identity_change_with_offline_attribute() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Prior identity present so the gate runs (the offline attr only defers the
        // eager session re-establishment, not the change notification).
        let changed: Jid = "5511888888888@s.whatsapp.net".parse().unwrap();
        client
            .signal_cache
            .put_identity(&changed.to_protocol_address(), &[7u8; 32])
            .await;

        // Notification with offline attribute should still be processed
        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511888888888@s.whatsapp.net")
            .attr("id", "identity-change-5")
            .attr("offline", "1")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "offline identity change should still dispatch event"
        );
    }

    /// With no prior identity for the peer (e.g. a group-only member we never had a
    /// session with), the had-prior-identity gate skips the heavy reset: no change
    /// notification and no session/identity deletion. Only the device-list cleanup
    /// runs. Mirrors WA Web `if (!isEmpty(loadIdentityKey(addr)))`.
    #[tokio::test]
    async fn test_identity_change_no_prior_identity_skips_reset() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let target: Jid = "5511666666666@s.whatsapp.net".parse().unwrap();
        let addr = target.to_protocol_address();
        // Seed a device-registry entry (with a companion device) so the always-on
        // cleanup has something to do, but deliberately do NOT seed an identity.
        client
            .device_registry_cache
            .insert(
                "5511666666666".into(),
                Arc::new(wacore::store::traits::DeviceListRecord {
                    user: "5511666666666".into(),
                    devices: vec![wacore::store::traits::DeviceInfo {
                        device_id: 1,
                        key_index: None,
                    }],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: Some(1),
                }),
            )
            .await;

        // Seed a companion-device (device 1) Signal session: clear_device_record
        // runs even on the no-prior path, so this must be deleted afterward. Keyed
        // the same way delete_sessions_for_devices builds the address.
        let mut companion = wacore_binary::Jid::new("5511666666666", wacore_binary::Server::Pn);
        companion.device = 1;
        let companion_addr = companion.to_protocol_address();
        client
            .signal_cache
            .put_session(
                &companion_addr,
                wacore::libsignal::protocol::SessionRecord::new_fresh(),
            )
            .await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511666666666@s.whatsapp.net")
            .attr("id", "identity-change-noprior")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        // No change notification for a peer we had no prior identity with.
        assert!(
            collector.events().is_empty(),
            "no-prior-identity push must not dispatch IdentityChange, got: {:?}",
            collector.events()
        );
        // But the always-on device-list cleanup still ran.
        assert!(
            client
                .device_registry_cache
                .get("5511666666666")
                .await
                .is_none(),
            "device registry cache should still be invalidated on the no-prior path"
        );
        // And no identity was created by an (skipped) eager re-establishment.
        let backend = client.persistence_manager.backend();
        assert!(
            client
                .signal_cache
                .get_identity(&addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "no-prior path must not establish an identity"
        );
        // The always-on clear_device_record must still delete companion sessions.
        assert!(
            !client
                .signal_cache
                .has_session(&companion_addr, backend.as_ref())
                .await
                .unwrap(),
            "companion-device session must be cleared even on the no-prior path"
        );
    }

    /// Regression: when a PN->LID mapping was learned offline (migration deferred),
    /// the identity is still under the PN address while resolve_encryption_jid points
    /// at the LID. The gate must check the original PN address too and still run the
    /// reset (delete the stale PN identity + dispatch the event), not false-negative.
    #[tokio::test]
    async fn test_identity_change_resets_unmigrated_pn_identity_under_lid_resolve() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        let pn = "5511555555555";
        let lid = "100000000000055";
        // Offline learn: records the PN->LID mapping in cache but skips the Signal
        // migration, so resolve points at the LID while state stays under the PN.
        client
            .learn_lid_pn_mapping_fast(lid, pn, LearningSource::Other, true)
            .await;

        let pn_jid: Jid = "5511555555555@s.whatsapp.net".parse().unwrap();
        // Confirm the setup actually diverges (resolve -> LID), else the test is moot.
        let resolved = client.resolve_encryption_jid(&pn_jid).await;
        assert!(
            resolved.is_lid(),
            "test setup: resolve_encryption_jid should return the LID, got {resolved}"
        );

        // Seed the identity under the PN address (not the LID).
        let pn_addr = pn_jid.to_protocol_address();
        client.signal_cache.put_identity(&pn_addr, &[7u8; 32]).await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511555555555@s.whatsapp.net")
            .attr("id", "identity-change-pnlid")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "must dispatch IdentityChange when the identity is under the unmigrated PN address"
        );
        let backend = client.persistence_manager.backend();
        assert!(
            client
                .signal_cache
                .get_identity(&pn_addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "the stale PN identity must be deleted by the reset"
        );
    }

    /// Regression: a stanza can carry a `lid` attr while the local PN->LID cache is
    /// cold, so resolve_encryption_jid falls back to PN. If the identity lives under
    /// the stanza LID, the gate must still find it (via the stanza-LID candidate) and
    /// run the reset rather than skip it.
    #[tokio::test]
    async fn test_identity_change_resets_identity_under_stanza_lid_with_cold_cache() {
        use wacore::types::jid::JidExt;
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone());

        // Cold cache: no PN->LID mapping, so resolve_encryption_jid(PN) returns PN.
        let pn_jid: Jid = "5511444444444@s.whatsapp.net".parse().unwrap();
        let resolved = client.resolve_encryption_jid(&pn_jid).await;
        assert!(
            !resolved.is_lid(),
            "test setup: cache must be cold (resolve -> PN), got {resolved}"
        );

        // The identity lives under the LID carried by the stanza, not the PN.
        let lid_jid: Jid = "100000000000066@lid".parse().unwrap();
        let lid_addr = lid_jid.to_protocol_address();
        client
            .signal_cache
            .put_identity(&lid_addr, &[7u8; 32])
            .await;

        let node = NodeBuilder::new("notification")
            .attr("type", "encrypt")
            .attr("from", "5511444444444@s.whatsapp.net")
            .attr("lid", "100000000000066@lid")
            .attr("id", "identity-change-stanzalid")
            .children([NodeBuilder::new("identity").build()])
            .build();
        handle_notification_impl(&client, node_to_arc(node)).await;

        assert!(
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::IdentityChange(_))),
            "must dispatch IdentityChange when the identity is under the stanza LID"
        );
        let backend = client.persistence_manager.backend();
        assert!(
            client
                .signal_cache
                .get_identity(&lid_addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "the stale stanza-LID identity must be deleted by the reset"
        );
    }
}
