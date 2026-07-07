use crate::client::Client;
use crate::lid_pn_cache::LearningSource;
use crate::types::events::Event;
use log::{debug, info, warn};
use std::sync::Arc;
use wacore::stanza::devices::DeviceNotification;
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore::types::events::{DeviceListUpdate, DeviceNotificationInfo};
use wacore_binary::NodeRef;
use wacore_binary::{Jid, JidExt};

pub(crate) async fn handle_encrypt_notification(
    client: &Arc<Client>,
    nr: &wacore_binary::NodeRef<'_>,
) {
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

pub(crate) async fn handle_account_sync_notification(
    client: &Arc<Client>,
    nr: &wacore_binary::NodeRef<'_>,
) {
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
pub(crate) async fn handle_prekey_low(client: &Arc<Client>) {
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
pub(crate) fn handle_digest_key(client: &Arc<Client>) {
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
pub(crate) async fn handle_identity_change(client: &Arc<Client>, node: &NodeRef<'_>) {
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
    let device_snapshot = client.persistence_manager.get_device_snapshot();
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
            .flush_signal_cache_batch_safe_logged("identity change", None)
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
        crate::types::events::IdentityChange::builder()
            .user(from_jid.clone())
            .maybe_lid_user(stanza_lid)
            .implicit(false)
            .build(),
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
    let device_snapshot = client.persistence_manager.get_device_snapshot();
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
        crate::types::events::IdentityChange::builder()
            .user(sender)
            .maybe_lid_user(None)
            .implicit(true)
            .build(),
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
pub(crate) async fn handle_devices_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
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
    let event = Event::DeviceListUpdate(
        DeviceListUpdate::builder()
            .user(notification.from.clone())
            .maybe_lid_user(notification.lid_user.clone())
            .update_type(op.operation_type.into())
            .devices(
                op.devices
                    .iter()
                    .map(|d| {
                        DeviceNotificationInfo::builder()
                            .device_id(d.device_id())
                            .maybe_key_index(d.key_index)
                            .build()
                    })
                    .collect(),
            )
            .maybe_key_index(op.key_index.clone())
            .maybe_contact_hash(op.contact_hash.clone())
            .build(),
    );
    client.core.event_bus.dispatch(event);
}

/// Parsed device info from account_sync notification
pub(crate) struct AccountSyncDevice {
    pub(crate) jid: Jid,
    pub(crate) key_index: Option<u32>,
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
pub(crate) fn parse_account_sync_device_list(devices_node: &NodeRef<'_>) -> Vec<AccountSyncDevice> {
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
pub(crate) async fn handle_account_sync_devices(
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
    let device_snapshot = client.persistence_manager.get_device_snapshot();
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
