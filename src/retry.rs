use crate::client::Client;
use crate::message::RetryReason;
use crate::types::events::Receipt;
use buffa::Message;
use log::{debug, info, warn};
use wacore::types::message::MessageCategory;

use scopeguard;
use std::sync::Arc;
use wacore::iq::prekeys::{OneTimePreKeyNode, SignedPreKeyNode};
use wacore::libsignal::protocol::{PreKeyBundle, PublicKey, UsePQRatchet, process_prekey_bundle};
use wacore::protocol::ProtocolNode;
use wacore::types::jid::JidExt;
use wacore_binary::JidExt as _;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, OwnedNodeRef};
#[cfg(test)]
use wacore_binary::{Node, NodeContent};
use wacore_binary::{NodeContentRef, NodeRef};

/// Helper to extract bytes content from a Node (used in tests).
#[cfg(test)]
fn get_bytes_content(node: &Node) -> Option<&[u8]> {
    match &node.content {
        Some(NodeContent::Bytes(b)) => Some(b.as_slice()),
        _ => None,
    }
}

/// Helper to extract bytes content from a NodeRef.
fn get_bytes_content_ref<'a>(node: &'a NodeRef<'_>) -> Option<&'a [u8]> {
    match node.content.as_deref() {
        Some(NodeContentRef::Bytes(b)) => Some(b.as_ref()),
        _ => None,
    }
}

/// Maximum retry attempts we'll honor (matches WhatsApp Web's MAX_RETRY = 5).
/// We refuse to resend if the requester has already retried this many times.
const MAX_RETRY_COUNT: u8 = 5;

/// Minimum retry count before we start tracking base keys.
/// WhatsApp Web saves base key on retry 2, checks on retry > 2.
const MIN_RETRY_FOR_BASE_KEY_CHECK: u8 = 2;

/// Throttle for the "no-keys + retry≥2" forced-recreate fallback. Mirrors
/// whatsmeow's `recreateSessionTimeout` (`retry.go:156`).
const RECREATE_SESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3600);

/// Separated chat and requester JIDs for retry receipt handling.
/// Mirrors WAWebHandleRetryRequest `getActualChatInfo` + `getTargetChat`.
struct RetryChatInfo {
    /// Bare chat JID (no device suffix) for message lookup.
    chat: Jid,
    /// Device-specific JID of the requesting device, for session management.
    requester: Jid,
    /// Raw `from` JID from the receipt, for stanza `to` attribute.
    /// WA Web preserves the original `from` (variable `m`) for the retry stanza.
    original_from: Jid,
    /// Receipt's `recipient` attribute, if present. WA Web's
    /// `handleRetryRequest` propagates this verbatim into the retry resend
    /// (only self-DM and bot receipts carry it).
    recipient: Option<Jid>,
    /// True if the requester is a bot JID (skip namespace normalization).
    is_bot: bool,
    /// WA Web's `bot_retry` parser path: only primary `@bot` JIDs, not legacy PN bots.
    is_fbid_bot_retry: bool,
}

fn is_fbid_bot_retry_jid(jid: &Jid) -> bool {
    jid.server == wacore_binary::Server::Bot && jid.device() == 0
}

/// Resolve the chat and requester JIDs from a retry receipt, separating
/// message-lookup concerns from session-management concerns.
/// Mirrors WAWebHandleRetryRequest `getActualChatInfo` + `getTargetChat`.
fn resolve_retry_chat_info(
    receipt: &Receipt,
    node: &NodeRef<'_>,
    own_pn: Option<&Jid>,
    own_lid: Option<&Jid>,
) -> Option<RetryChatInfo> {
    let from = &receipt.source.chat;

    if from.is_group() || from.is_status_broadcast() {
        // Groups/status: chat is already the group/broadcast JID.
        // Requester is the participant attr (the actual retrying device).
        let participant = node.attrs().optional_jid("participant");
        let is_fbid_bot_retry =
            from.is_group() && participant.as_ref().is_some_and(is_fbid_bot_retry_jid);
        let requester = participant.unwrap_or_else(|| receipt.source.sender.clone());
        let is_bot = requester.is_bot();
        Some(RetryChatInfo {
            chat: from.clone(),
            requester,
            original_from: from.clone(),
            recipient: node.attrs().optional_jid("recipient"),
            is_bot,
            is_fbid_bot_retry,
        })
    } else {
        // DM: resolve chat target via getTargetChat logic.
        let recipient = node.attrs().optional_jid("recipient");
        let is_bot = from.is_bot();

        // WA Web getTargetChat (RetryRequest.js:339-371):
        // 1. Bot + recipient → chat = recipient
        // 2. Peer device + recipient → chat = recipient
        // 3. Peer device without recipient → WA Web aborts (returns null).
        // 4. Normal user → chat = asUserWidOrThrow(from) = from.to_non_ad()
        let is_peer = own_pn.is_some_and(|pn| from.is_same_user_as(pn))
            || own_lid.is_some_and(|lid| from.is_same_user_as(lid));

        let chat = if is_bot && let Some(r) = recipient.as_ref() {
            r.to_non_ad()
        } else if is_peer {
            match recipient.as_ref() {
                Some(r) => r.to_non_ad(),
                None => {
                    log::warn!("Ignoring peer device retry without recipient attr");
                    return None;
                }
            }
        } else {
            from.to_non_ad()
        };

        let requester = if from.device() == 0 && from.agent == 0 {
            chat.clone()
        } else {
            from.clone()
        };

        Some(RetryChatInfo {
            chat,
            requester,
            original_from: from.clone(),
            recipient,
            is_bot,
            is_fbid_bot_retry: is_fbid_bot_retry_jid(from),
        })
    }
}

fn validate_retry_prekey_presence(
    keys_node: &NodeRef<'_>,
    is_fbid_bot_retry: bool,
) -> Result<(), anyhow::Error> {
    if !is_fbid_bot_retry && keys_node.get_optional_child("key").is_none() {
        anyhow::bail!("regular retry key bundle missing one-time prekey");
    }
    Ok(())
}

// No retry_count in the key: concurrent receipts for the same participant must
// serialize, otherwise two update_local_signal_session calls race on session state.
fn build_retry_processing_key(chat: &Jid, message_id: &str, participant_jid: &Jid) -> String {
    let mut key = String::with_capacity(message_id.len() + 64);
    chat.push_to(&mut key);
    key.push(':');
    key.push_str(message_id);
    key.push(':');
    participant_jid.push_to(&mut key);
    key
}

impl Client {
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.retry.handle_receipt", level = "debug", skip_all, fields(chat = %receipt.source.chat.observe(), sender = %receipt.source.sender.observe(), count = tracing::field::Empty), err(Debug)))]
    pub(crate) async fn handle_retry_receipt(
        self: &Arc<Self>,
        receipt: &Receipt,
        node: &Arc<OwnedNodeRef>,
    ) -> Result<(), anyhow::Error> {
        let nr = node.get();
        let retry_child = nr
            .get_optional_child("retry")
            .ok_or_else(|| anyhow::anyhow!("<retry> child missing from receipt"))?;

        let message_id = retry_child
            .get_attr("id")
            .map(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("<retry> missing 'id' attribute"))?
            .into_owned();
        let retry_count: u8 = retry_child
            .get_attr("count")
            .map(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);
        // Record the count on the span so retry-storm depth is aggregable per
        // sender even when the cap refuses early below.
        #[cfg(feature = "tracing")]
        tracing::Span::current().record("count", retry_count);

        // Refuse to handle retries that have exceeded the maximum attempts.
        // This prevents infinite retry loops and matches WhatsApp Web's behavior.
        // Logged at debug: remote-driven, expected and fully handled — WA Web
        // emits this refusal via WALogger.LOG (informational), not WARN.
        if retry_count >= MAX_RETRY_COUNT {
            debug!(
                "Refusing retry #{} for message {} from {}: exceeds max attempts ({})",
                retry_count,
                message_id,
                receipt.source.sender.observe(),
                MAX_RETRY_COUNT
            );
            wacore::telemetry::retry_refused();
            return Ok(());
        }

        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let Some(mut info) = resolve_retry_chat_info(
            receipt,
            nr,
            device_snapshot.pn.as_ref(),
            device_snapshot.lid.as_ref(),
        ) else {
            return Ok(());
        };
        let is_group_or_status = info.chat.is_group() || info.chat.is_status_broadcast();

        // WA Web doesn't dedupe receipts (Message/Queue.js just serializes per-chat);
        // MAX_RETRY_COUNT covers loop prevention. This lock only guards against
        // two concurrent receipts racing on session state.
        let processing_key = build_retry_processing_key(&info.chat, &message_id, &info.requester);

        if !self
            .pending_retries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(processing_key.clone())
        {
            log::debug!("Ignoring retry for {processing_key}: a retry is already in progress.");
            return Ok(());
        }
        // processing_key isn't needed by name after this point — move it into
        // the scopeguard instead of cloning again.
        let pending = Arc::clone(&self.pending_retries);
        let _guard = scopeguard::guard((), move |()| {
            pending
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&processing_key);
        });

        // A retry from a device missing from our registry signals a stale device
        // list for this user, so refresh it (rate-limited, dedup'd) to learn the
        // device for the next send. Done before the message-cache lookup so an
        // evicted retry still triggers it.
        let sender_device_id = info.requester.device() as u32;
        let device_known = self
            .has_device(&info.requester.user, sender_device_id)
            .await;
        if !device_known {
            // Parity with WA Web's MdRetryFromUnknownDevice WAM (id 2178), which
            // commits here only — not from the shared inbound device sync, which
            // schedule_unknown_device_sync is also called from elsewhere.
            wacore::telemetry::retry_unknown_device(if sender_device_id == 0 {
                "primary"
            } else {
                "companion"
            });
            self.schedule_unknown_device_sync(info.requester.to_non_ad(), receipt.offline)
                .await;
        }

        // Peek keeps the message in the cache, so we avoid the decode + re-encode
        // and the background DB delete + re-store that take + re-add did on every
        // retry (pure churn during retry storms). Fall back to the consuming take +
        // re-add only on an L1 miss (DB-only mode, or after eviction), where peek
        // can't serve it; that path still re-adds so other devices can retry.
        let (original_msg, alt_chat) = match self.peek_recent_message(&info.chat, &message_id).await
        {
            Some(result) => result,
            None => match self.take_recent_message(&info.chat, &message_id).await {
                Some(result) => {
                    self.add_recent_message(&info.chat, &message_id, &result.0, None)
                        .await;
                    result
                }
                None => {
                    log::debug!(
                        "Ignoring retry for message {message_id}: already handled or not found in cache."
                    );
                    return Ok(());
                }
            },
        };

        // When message was found via alternate PN<->LID key, the Signal session
        // lives in the stored message's namespace (not the receipt's). Build the
        // encryption JID from that namespace + requester's device, skipping
        // resolve_encryption_jid (which would map back to the primary namespace).
        // WA Web: `e.from.isBot() ? (p = e.from) : (p = d.isLid() ? toLid(e.from) : toPn(e.from))`
        // Bots skip namespace normalization (WAWebHandleRetryRequest:311-312).
        let resolved_jid = if let Some(alt_chat) = alt_chat
            && !is_group_or_status
            && !info.is_bot
        {
            let requester = &info.requester;
            info.requester = Jid {
                user: alt_chat.user,
                server: alt_chat.server,
                device: requester.device,
                agent: requester.agent,
                integrator: requester.integrator,
            };
            info.requester.clone()
        } else {
            self.resolve_encryption_jid(&info.requester).await
        };

        let keys_node_present = nr.get_optional_child("keys").is_some();
        if wacore::protocol::retry::should_drop_unknown_device_retry(
            keys_node_present,
            device_known,
        ) {
            warn!(
                "handle_retry_receipt: device not found for device={}, user={}",
                sender_device_id, info.requester.user
            );
            return Ok(());
        }

        // Check if this is a retry from our own device (peer).
        let is_peer = device_snapshot
            .pn
            .as_ref()
            .is_some_and(|our_pn| info.requester.is_same_user_as(our_pn))
            || device_snapshot
                .lid
                .as_ref()
                .is_some_and(|our_lid| info.requester.is_same_user_as(our_lid));

        // Fetch group info (cache-first, server on miss) — used for SKDM rotation + addressing_mode.
        // Without this, a cold cache would silently default to PN semantics for LID groups.
        let cached_group_info = if info.chat.is_group() {
            match self.groups().query_info(&info.chat).await {
                Ok(gi) => Some(gi),
                Err(e) => {
                    log::warn!(
                        "Failed to fetch group info for retry of msg {} in {}: {e}",
                        message_id,
                        info.chat.observe()
                    );
                    None
                }
            }
        } else {
            None
        };

        // WA Web rotateKey: unknown device (not in participant list, not LID) →
        // force full sender key rotation by clearing all sender key device tracking.
        // This is separate from updateLocalSignalSession and specific to group retries.
        if is_group_or_status && !info.requester.is_lid() && !info.chat.is_status_broadcast() {
            let group_jid = info.chat.to_string();
            let is_known_participant = cached_group_info
                .as_ref()
                .is_some_and(|g| g.participants.iter().any(|p| p.user == info.requester.user));

            if !is_known_participant {
                log::warn!(
                    "Unknown device {} in group {} — forcing full sender key rotation \
                     (matches WA Web's rotateKey behavior)",
                    info.requester.observe(),
                    group_jid
                );

                // WA Web: deleteGroupSenderKeyInfo(groupWid, ownWid) — delete our own
                // sender key for forward secrecy. When addressing mode is known,
                // delete only that namespace; otherwise both.
                let addressing_mode = cached_group_info.as_ref().map(|g| g.addressing_mode);
                let jids_to_delete: Vec<_> = match addressing_mode {
                    Some(wacore::types::message::AddressingMode::Lid) => {
                        device_snapshot.lid.as_ref().into_iter().collect()
                    }
                    Some(wacore::types::message::AddressingMode::Pn) => {
                        device_snapshot.pn.as_ref().into_iter().collect()
                    }
                    None => device_snapshot
                        .lid
                        .as_ref()
                        .into_iter()
                        .chain(device_snapshot.pn.as_ref())
                        .collect(),
                };

                for own_jid in jids_to_delete {
                    use wacore::libsignal::store::sender_key_name::SenderKeyName;
                    let sk_name = SenderKeyName::from_parts(
                        &group_jid,
                        own_jid.to_protocol_address().as_str(),
                    );
                    self.signal_cache
                        .delete_sender_key(sk_name.cache_key())
                        .await;
                }

                // DB first, then cache invalidate — prevents a concurrent
                // resolve_skdm_targets from reviving stale cache entries.
                if let Err(e) = self
                    .persistence_manager
                    .clear_sender_key_devices(&group_jid)
                    .await
                {
                    log::warn!("Failed to clear sender key devices for rotation: {}", e);
                }
                self.sender_key_device_cache.invalidate(&group_jid).await;
            }
        }

        // Mirror WAWebUpdateLocalSignalSession for all chat types: markForgetSenderKey
        // (group/status) + processKeyBundle + regId-mismatch delete + base-key logic.
        // Must run before ensureE2ESessions so any session deletion here is rebuilt there.
        if !self
            .update_local_signal_session(
                &info,
                &resolved_jid,
                &message_id,
                retry_count,
                nr,
                is_peer,
            )
            .await
        {
            return Ok(());
        }

        // Whatsmeow parity (`retry.go:284`). WA Web's regId/base-key check
        // doesn't catch silently-diverged sessions; this fallback does.
        if nr.get_optional_child("keys").is_none() {
            // Hold the per-peer session lock across the throttle check+stamp AND
            // the delete so the recreate decision is atomic per peer. The
            // `session_recreate_history` get+insert is not atomic on its own,
            // and retry receipts for different message_ids from the same peer
            // dispatch concurrently (detached spawn in `handle_receipt`), so
            // without this lock two of them could both pass the throttle and
            // recreate. Mirrors whatsmeow holding `sessionRecreateHistoryLock`
            // across its check+stamp (`retry.go:160`). This is the same per-peer
            // lock the delete already used, so it adds no new lock.
            let signal_address = resolved_jid.to_protocol_address();
            let lock = self.session_lock_for(signal_address.as_str()).await;
            let guard = lock.lock().await;
            if let Some(reason) = self
                .should_recreate_session(retry_count, &resolved_jid)
                .await
            {
                info!(
                    "Recreating session with {} for retry of {message_id}: {reason}",
                    resolved_jid.observe()
                );
                self.signal_cache.delete_session(&signal_address).await;
                drop(guard);
                self.flush_signal_cache_logged("should_recreate_session", Some(&message_id))
                    .await;
            }
        }

        // Status broadcasts can't resend (requires explicit recipient list).
        // Participant already marked for fresh SKDM above; next status send includes them.
        if info.chat.is_status_broadcast() {
            info!(
                "Status broadcast retry for {} — participant marked for fresh SKDM, \
                 will be included in next status send",
                message_id
            );
            return Ok(());
        }

        // Bound the aggregate resend rate per group (the anti-abuse signal): a
        // PN to LID fan-out has many distinct devices retry the same messages,
        // which per-device/per-message caps miss. Group-only: the requester was
        // marked for fresh SKDM above so future messages recover, and it
        // re-requests this one on its own timer once the bucket refills. DMs have
        // no SKDM fallback, so they keep the unconditional resend (bounded by
        // MAX_RETRY_COUNT) rather than risk dropping a delivery.
        if info.chat.is_group() && !self.resend_rate_limiter.try_acquire(&info.chat).await {
            debug!(
                "Throttling resend of {} to {}: per-chat resend rate cap reached",
                message_id,
                info.chat.observe()
            );
            return Ok(());
        }

        info!(
            "Resending message {} to {} (retry #{})",
            message_id,
            info.chat.observe(),
            retry_count
        );

        if info.chat.is_group() {
            // Group retry: pairwise encrypt to failing device only (RetryMsgJob.js:71).
            // Using sender-key broadcast would resend to ALL participants → duplicates.
            //
            // WA Web calls ensureE2ESessions for all chat types, not just DMs
            // (RetryRequest.js:200). Without this, a reg-ID mismatch or unknown
            // device whose session was deleted above would fail `prepare_group_retry_stanza`
            // with "session not found", silencing subsequent retries via the duplicate filter.
            self.ensure_e2e_sessions_resolved(std::slice::from_ref(&resolved_jid))
                .await?;

            let device_snapshot = self.persistence_manager.get_device_snapshot();

            let addressing_mode = cached_group_info
                .as_ref()
                .map(|g| g.addressing_mode)
                .unwrap_or_default();

            let signal_address = resolved_jid.to_protocol_address();
            let session_mutex = self.session_lock_for(signal_address.as_str()).await;
            let _session_guard = session_mutex.lock().await;
            let mut store_adapter = self.signal_adapter().await;

            let edit_attr =
                wacore::types::message::EditAttribute::infer_from_message(&original_msg);
            let stanza = wacore::send::prepare_group_retry_stanza(
                &mut store_adapter.session_store,
                &mut store_adapter.identity_store,
                info.chat,
                info.requester,
                resolved_jid.clone(),
                &original_msg,
                message_id,
                retry_count,
                device_snapshot.account.as_deref(),
                addressing_mode,
                edit_attr,
            )
            .await?;

            self.send_node(stanza).await?;
            self.flush_signal_cache().await?;
        } else {
            // DM retry: pairwise resend to the requesting device only.
            // Use _resolved variant: resolved_jid is already in the correct
            // namespace (including alternate PN/LID normalization).
            // WA Web's ensureE2ESessions also uses already-normalized JIDs.
            self.ensure_e2e_sessions_resolved(std::slice::from_ref(&resolved_jid))
                .await?;

            let device_snapshot = self.persistence_manager.get_device_snapshot();
            let signal_address = resolved_jid.to_protocol_address();
            let session_mutex = self.session_lock_for(signal_address.as_str()).await;
            let _session_guard = session_mutex.lock().await;
            let mut store_adapter = self.signal_adapter().await;

            let edit_attr =
                wacore::types::message::EditAttribute::infer_from_message(&original_msg);
            // WA Web forwards the receipt's `recipient` verbatim
            // (`f && (k.recipient = f)` in handleRetryRequest); for non-self
            // DM receipts the attribute is absent and the resend drops it.
            let stanza = wacore::send::prepare_dm_retry_stanza(
                &mut store_adapter.session_store,
                &mut store_adapter.identity_store,
                info.original_from,
                info.recipient.clone(),
                resolved_jid.clone(),
                &original_msg,
                message_id,
                retry_count,
                device_snapshot.account.as_deref(),
                edit_attr,
            )
            .await?;

            self.send_node(stanza).await?;
            self.flush_signal_cache().await?;
        }

        Ok(())
    }

    /// Mirrors WAWebUpdateLocalSignalSession (`WAWeb/Update/LocalSignalSession.js`).
    /// Runs before ensureE2ESessions + sendRetry for all chat types (DM, group,
    /// status). Order and semantics match the WA Web implementation:
    ///   1. markForgetSenderKey for group/status (participant needs fresh SKDM)
    ///   2. processKeyBundle if `<keys>` present
    ///   3. If no bundle AND stored regId differs → delete session
    ///   4. retry == 2 → save current base key, return (no delete)
    ///   5. retry > 2 AND same base key → delete session (force re-establish)
    ///
    /// Unlike the previous DM-only path, this does NOT unconditionally delete
    /// the session on every retry — WA Web preserves it on retry==1 and on
    /// retry>2 when the base key already changed (session was regenerated
    /// legitimately). The subsequent `ensure_e2e_sessions_resolved` call in
    /// `handle_retry_receipt` rebuilds any session this function deleted.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.retry.update_local_session", level = "debug", skip_all, fields(chat = %info.chat.observe(), peer = %resolved_jid.observe(), retry = retry_count)))]
    async fn update_local_signal_session(
        &self,
        info: &RetryChatInfo,
        resolved_jid: &Jid,
        message_id: &str,
        retry_count: u8,
        node: &NodeRef<'_>,
        is_peer: bool,
    ) -> bool {
        // 1. markForgetSenderKey (WA Web L33-38). Rust unifies group and status
        //    under a single storage (chat JID as the key) — markForgetSenderKey
        //    handles both `@g.us` and `status@broadcast` as opaque group_jid.
        if info.chat.is_group() || info.chat.is_status_broadcast() {
            let group_jid = info.chat.to_string();
            match self
                .mark_forget_sender_key(&group_jid, std::slice::from_ref(&info.requester))
                .await
            {
                Ok(()) => {
                    let chat_type = if info.chat.is_status_broadcast() {
                        "status broadcast"
                    } else {
                        "group"
                    };
                    info!(
                        "Marked {} for fresh SKDM in {} {} due to retry receipt",
                        info.requester.observe(),
                        chat_type,
                        group_jid
                    );
                }
                Err(e) => log::warn!(
                    "Failed to mark sender key forget for {} in {}: {}",
                    info.requester.observe(),
                    group_jid,
                    e
                ),
            }
        }

        // 2. processKeyBundle (WA Web L51). Previously gated behind
        //    `!is_status_broadcast()`; WA Web runs it unconditionally.
        let keys_node_present = node.get_optional_child("keys").is_some();
        let key_bundle_result = self
            .process_retry_key_bundle(node, resolved_jid, is_peer, info.is_fbid_bot_retry)
            .await;
        let key_bundle_processed = key_bundle_result.is_ok();

        // 3. No bundle + regId mismatch → delete session (WA Web L52-65).
        //    Gate on `!keys_node_present` so a rejected bundle (security
        //    refusal for peer reg-ID change, parse errors, invalid reg ID)
        //    doesn't trigger destructive session deletion as a side effect.
        if !key_bundle_processed && keys_node_present {
            log::warn!(
                "Key bundle present but rejected for {}: {:?} — aborting retry resend",
                resolved_jid.observe(),
                key_bundle_result.as_ref().err()
            );
            return false;
        }
        if !key_bundle_processed && !keys_node_present {
            if let Err(ref e) = key_bundle_result {
                // Demoted to debug on the happy path (peer retry without re-key):
                // only warn when a regId mismatch triggers a delete below.
                log::debug!(
                    "No key bundle in retry receipt for {}: {}. Checking for reg ID mismatch.",
                    resolved_jid.observe(),
                    e
                );
            }

            if let Some(received_reg_id) =
                wacore::protocol::retry::extract_registration_id_from_node_ref(node)
            {
                let signal_address = resolved_jid.to_protocol_address();
                let device_snapshot = self.persistence_manager.get_device_snapshot();
                let session = self
                    .signal_cache
                    .peek_session(&signal_address, &*device_snapshot.backend)
                    .await
                    .ok()
                    .flatten();

                if let Some(session) = session
                    && let Ok(stored_reg_id) = session.remote_registration_id()
                    && stored_reg_id != 0
                    && stored_reg_id != received_reg_id
                {
                    info!(
                        "Registration ID mismatch for {} (stored: {}, received: {}). \
                         Deleting session since no key bundle provided.",
                        wacore::types::jid::observe_protocol_address(&signal_address),
                        stored_reg_id,
                        received_reg_id
                    );
                    let lock = self.session_lock_for(signal_address.as_str()).await;
                    let _guard = lock.lock().await;
                    self.signal_cache.delete_session(&signal_address).await;
                    drop(_guard);
                    self.flush_signal_cache_logged("reg ID mismatch session deletion", None)
                        .await;
                }
            }
        }

        // 4-5. Base-key collision logic (WA Web L66-80). Applied to ALL chat
        //      types now — previously only ran in the DM branch.
        let signal_address = resolved_jid.to_protocol_address();
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let session = self
            .signal_cache
            .peek_session(&signal_address, &*device_snapshot.backend)
            .await
            .ok()
            .flatten();

        let Some(session) = session else {
            return true;
        };
        let Ok(current_base_key) = session.alice_base_key() else {
            return true;
        };

        let addr_str = signal_address.as_str();
        if retry_count == MIN_RETRY_FOR_BASE_KEY_CHECK {
            // retry == 2: save base key, do NOT delete (WA Web L66-67).
            match device_snapshot
                .backend
                .save_base_key(addr_str, message_id, current_base_key)
                .await
            {
                Ok(()) => info!(
                    "Saved base key for {} at retry #{} for collision detection",
                    wacore::types::jid::observe_protocol_address(&signal_address),
                    retry_count
                ),
                Err(e) => warn!(
                    "Failed to save base key for {}: {}",
                    wacore::types::jid::observe_protocol_address(&signal_address),
                    e
                ),
            }
            return true;
        }

        if retry_count > MIN_RETRY_FOR_BASE_KEY_CHECK {
            match device_snapshot
                .backend
                .has_same_base_key(addr_str, message_id, current_base_key)
                .await
            {
                Ok(true) => {
                    // Informational, not WARN: this is the corrective action WA
                    // Web takes here too (WAWebUpdateLocalSignalSession logs the
                    // same-base-key delete via WALogger.LOG), and the three
                    // sibling branches of this routine already log at info.
                    info!(
                        "Base key collision detected for {} (msg {}) at retry #{}. \
                         Session hasn't been regenerated. Forcing fresh session.",
                        wacore::types::jid::observe_protocol_address(&signal_address),
                        message_id,
                        retry_count
                    );
                    wacore::telemetry::base_key_collision();
                    let _ = device_snapshot
                        .backend
                        .delete_base_key(addr_str, message_id)
                        .await;
                    let lock = self.session_lock_for(signal_address.as_str()).await;
                    let _guard = lock.lock().await;
                    self.signal_cache.delete_session(&signal_address).await;
                    drop(_guard);
                    self.flush_signal_cache_logged(
                        "base key collision — forcing fresh session",
                        None,
                    )
                    .await;
                }
                Ok(false) => {
                    info!(
                        "Base key changed for {} (msg {}) at retry #{} - session regenerated",
                        wacore::types::jid::observe_protocol_address(&signal_address),
                        message_id,
                        retry_count
                    );
                    let _ = device_snapshot
                        .backend
                        .delete_base_key(addr_str, message_id)
                        .await;
                }
                Err(e) => {
                    warn!(
                        "Failed to check base key for {}: {}",
                        wacore::types::jid::observe_protocol_address(&signal_address),
                        e
                    );
                }
            }
        }
        true
    }

    /// Mirrors whatsmeow's `shouldRecreateSession`. Returns `Some(reason)`
    /// and bumps the history clock if we should drop the local session for
    /// `jid`; `None` otherwise. Two conditions trigger:
    ///   1. No session present locally.
    ///   2. `retry_count >= 2` and >`RECREATE_SESSION_TIMEOUT` since the
    ///      last recreate for this JID.
    ///
    /// Callers pair this with `signal_cache.delete_session` so the next
    /// `ensure_e2e_sessions_resolved` does the prekey fetch + rebuild.
    async fn should_recreate_session(&self, retry_count: u8, jid: &Jid) -> Option<&'static str> {
        self.should_recreate_session_at(retry_count, jid, wacore::time::Instant::now())
            .await
    }

    /// Injectable-clock variant for testing the throttle expiry path.
    /// wacore::time::Instant is std::time::Instant-backed so subtracting a
    /// Duration to fabricate a "past" stamp saturates to 0 in young test
    /// runtimes; passing a future `now` instead exercises the same branch.
    async fn should_recreate_session_at(
        &self,
        retry_count: u8,
        jid: &Jid,
        now: wacore::time::Instant,
    ) -> Option<&'static str> {
        let signal_address = jid.to_protocol_address();
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        // Whatsmeow returns `false` on `ContainsSession` errors so a transient
        // backend read failure doesn't masquerade as "no session" and trigger
        // an unnecessary delete + prekey fetch (`retry.go:161-163`).
        let has_session = match self
            .signal_cache
            .has_session(&signal_address, &*device_snapshot.backend)
            .await
        {
            Ok(present) => present,
            Err(e) => {
                warn!(
                    "should_recreate_session: has_session failed for {}: {} — skipping recreate",
                    signal_address, e
                );
                return None;
            }
        };

        let history = &self.session_recreate_history;

        if !has_session {
            history.insert(jid.clone(), now).await;
            return Some("we don't have a Signal session with them");
        }

        if retry_count < MIN_RETRY_FOR_BASE_KEY_CHECK {
            return None;
        }

        // Throttle: skip if this peer was recreated within the timeout. This
        // explicit age check against the injectable `now` is the authoritative,
        // deterministic gate. The cache's 1h TTL on `session_recreate_history`
        // is only a memory backstop (lazy eviction independent of the stored
        // `now`, so it can't drive the throttle decision).
        // Do NOT drop this check as "redundant with the TTL".
        if let Some(prev) = history.get(jid).await
            && now.saturating_duration_since(prev) < RECREATE_SESSION_TIMEOUT
        {
            return None;
        }

        history.insert(jid.clone(), now).await;
        Some("retry count > 1 and over an hour since last recreation")
    }

    /// Extracts and processes the key bundle from a retry receipt.
    /// This allows us to establish a new session with the requester using their fresh prekeys.
    ///
    /// # Arguments
    /// * `node` - The retry receipt node containing the key bundle
    /// * `requester_jid` - The JID of the device requesting the retry
    /// * `is_peer` - Whether this is a peer device (our own device)
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.retry.process_key_bundle", level = "debug", skip_all, fields(peer = %requester_jid.observe(), is_peer, is_fbid_bot_retry), err(Debug)))]
    async fn process_retry_key_bundle(
        &self,
        node: &NodeRef<'_>,
        requester_jid: &wacore_binary::Jid,
        is_peer: bool,
        is_fbid_bot_retry: bool,
    ) -> Result<(), anyhow::Error> {
        let keys_node = node
            .get_optional_child("keys")
            .ok_or_else(|| anyhow::anyhow!("<keys> child missing from retry receipt"))?;
        validate_retry_prekey_presence(keys_node, is_fbid_bot_retry)?;

        // Use the centralized extractor so the >4-byte rejection rule applies
        // here too, not just on the no-keys retry path.
        let registration_id =
            wacore::protocol::retry::extract_registration_id_from_node_ref(node).unwrap_or(0);

        if registration_id == 0 {
            return Err(anyhow::anyhow!("Invalid registration ID in retry receipt"));
        }

        // Use requester_jid directly — the caller already resolved the correct
        // namespace (including alternate PN/LID normalization). Re-resolving
        // here would undo that normalization.
        let signal_address = requester_jid.to_protocol_address();

        // Check if the registration ID changed (indicates device reinstall).
        // Read session through cache for consistent state.
        {
            let device_snapshot = self.persistence_manager.get_device_snapshot();
            let session = self
                .signal_cache
                .peek_session(&signal_address, &*device_snapshot.backend)
                .await
                .ok()
                .flatten();

            if let Some(session) = session {
                let existing_reg_id = session.remote_registration_id()?;
                if existing_reg_id != 0 && existing_reg_id != registration_id {
                    // WhatsApp Web throws an error for peer device registration ID changes.
                    // This is a security measure - peer devices should maintain consistent identity.
                    if is_peer {
                        return Err(anyhow::anyhow!(
                            "Registration ID changed for peer device {} (was {}, now {}). \
                             This may indicate the device was reinstalled.",
                            signal_address,
                            existing_reg_id,
                            registration_id
                        ));
                    }
                    info!(
                        "Registration ID changed for {} (was {}, now {}). Session will be replaced.",
                        signal_address, existing_reg_id, registration_id
                    );
                }
            }
        }

        // Extract identity key.
        let identity_bytes = keys_node
            .get_optional_child("identity")
            .and_then(get_bytes_content_ref)
            .ok_or_else(|| anyhow::anyhow!("Missing identity key in retry receipt"))?;
        let identity_key = PublicKey::from_djb_public_key_bytes(identity_bytes)?;

        // Companion devices ADV-bind the fetched identity via <device-identity>;
        // reject a present-but-invalid one so a relay can't swap in a forged key.
        // Mirrors the prekey-fetch path. The account key is the in-blob
        // `account_signature_key` or, when the server omits it, the contact's
        // primary (device 0) identity from the store. An unverifiable-for-lack-of-key
        // chain or a missing device-identity is logged, not fatal.
        if requester_jid.device != 0
            && let Some(device_identity) = keys_node
                .get_optional_child("device-identity")
                .and_then(get_bytes_content_ref)
        {
            let fetched_identity: [u8; 32] = identity_bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("identity key in retry receipt is not 32 bytes"))?;
            let account_identity = self.load_account_identity(requester_jid).await;
            match wacore::adv::validate_adv_with_identity_key(
                device_identity,
                &fetched_identity,
                account_identity.as_ref(),
            ) {
                wacore::adv::AdvValidation::Valid => {}
                wacore::adv::AdvValidation::Invalid => {
                    return Err(anyhow::anyhow!(
                        "device-identity ADV validation failed for companion {requester_jid}"
                    ));
                }
                wacore::adv::AdvValidation::NoAccountKey => log::debug!(
                    "retry key bundle for companion {requester_jid} omits account_signature_key and no stored account identity; proceeding without ADV validation"
                ),
            }
        } else if requester_jid.device != 0 {
            log::warn!(
                "retry key bundle for companion {requester_jid} omits <device-identity>; proceeding without ADV validation"
            );
        }

        // Extract prekey (optional in some cases).
        let prekey_data = if let Some(key_ref) = keys_node.get_optional_child("key") {
            let prekey_node = OneTimePreKeyNode::try_from_node_ref(key_ref)?;
            let prekey_public = PublicKey::from_djb_public_key_bytes(&prekey_node.public_bytes)?;
            Some((prekey_node.id.into(), prekey_public))
        } else {
            None
        };

        // Extract signed prekey.
        let skey_ref = keys_node
            .get_optional_child("skey")
            .ok_or_else(|| anyhow::anyhow!("Missing signed prekey in retry receipt"))?;

        let signed_prekey = SignedPreKeyNode::try_from_node_ref(skey_ref)?;
        let skey_public = PublicKey::from_djb_public_key_bytes(&signed_prekey.public_bytes)?;
        let skey_signature: [u8; 64] = signed_prekey
            .signature
            .as_slice()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid signature length"))?;

        // Build and process the prekey bundle.
        let bundle = PreKeyBundle::new(
            registration_id,
            u32::from(requester_jid.device).into(),
            prekey_data,
            signed_prekey.id.into(),
            skey_public,
            skey_signature.into(),
            identity_key.into(),
        )?;

        // Acquire per-sender session lock to prevent race with concurrent message decryption.
        // This matches the session_locks pattern used in process_session_enc_batch.
        let session_mutex = self.session_lock_for(signal_address.as_str()).await;
        let _session_guard = session_mutex.lock().await;

        let mut adapter = self.signal_adapter().await;

        let identity_change = process_prekey_bundle(
            &signal_address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
            &bundle,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
            UsePQRatchet::No,
        )
        .await?;

        // Flush after session establishment
        self.flush_signal_cache().await?;

        if identity_change == wacore::libsignal::protocol::IdentityChange::ReplacedExisting {
            self.react_to_local_identity_change(requester_jid);
        }

        info!(
            "Processed key bundle from retry receipt for {}",
            signal_address
        );

        Ok(())
    }

    /// Sends a retry receipt to request the sender to resend a message.
    ///
    /// # Arguments
    /// * `info` - The message info for the failed message
    /// * `retry_count` - The retry attempt number (1-5). This is sent to the sender so they
    ///   know which attempt this is. The sender may use this to decide whether to resend.
    /// * `reason` - The retry reason code (matches WhatsApp Web's RetryReason enum). This helps
    ///   the sender understand why the message couldn't be decrypted.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.retry.send_receipt", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), retry = retry_count), err(Debug)))]
    pub(crate) async fn send_retry_receipt(
        &self,
        info: &crate::types::message::MessageInfo,
        retry_count: u8,
        reason: RetryReason,
    ) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();

        // WA Web's sendRetryReceipt aborts only when `!to.isBot() && participant.isBot()`,
        // with participant null for DMs. A bot DM is chat == sender == bot, so it is NOT
        // suppressed and the retry is sent; only a bot reply in a non-bot group is dropped.
        // Same helper the ack and self-fanout paths already use.
        if info.source.is_bot_authored_non_bot_chat() {
            log::debug!(
                "Skipping retry receipt for message {} from bot {} in non-bot chat {}",
                info.id,
                info.source.sender.observe(),
                info.source.chat.observe()
            );
            return Ok(());
        }

        debug!(
            "Sending retry receipt #{} for message {} in chat {} from {} (reason: {:?})",
            retry_count,
            info.id,
            info.source.chat.observe(),
            info.source.sender.observe(),
            reason
        );

        // Build the retry element with the error code (matches WhatsApp Web's format)
        let mut retry_builder = NodeBuilder::new("retry")
            .attr("v", "1")
            .attr("id", info.id.clone())
            .attr("t", info.timestamp.timestamp())
            .attr("count", retry_count);

        // Include the error code if it's not UnknownError (matches WhatsApp Web's behavior
        // where error is only included when there's a specific reason)
        if reason != RetryReason::UnknownError {
            retry_builder = retry_builder.attr("error", reason as u8);
        }

        let retry_node = retry_builder.build();

        let registration_id_bytes = device_snapshot.registration_id.to_be_bytes().to_vec();
        let registration_node = NodeBuilder::new("registration")
            .bytes(registration_id_bytes)
            .build();

        let keys_node = if wacore::protocol::retry::should_include_keys(retry_count, reason) {
            // WA Web's getOrGenSinglePreKey = getOrGenPreKeys(1): the retry
            // prekey is the first unuploaded window key (reused when one is
            // left over, freshly generated and stored otherwise) and the next
            // batch upload re-offers it to the server pool. Hold
            // prekey_upload_lock to serialize the watermark math with uploads.
            let prekey_guard = self.prekey_upload_lock.lock().await;
            let (new_prekey_id, new_prekey_public) = self.get_or_gen_single_pre_key().await?;
            drop(prekey_guard);
            let device_snapshot = self.persistence_manager.get_device_snapshot();

            let device_identity_bytes = device_snapshot
                .account
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Missing device account info for retry receipt"))?
                .encode_to_vec();

            Some(wacore::protocol::retry::build_retry_keys_node(
                &device_snapshot.identity_key.public_key,
                new_prekey_id,
                &new_prekey_public,
                device_snapshot.signed_pre_key_id,
                &device_snapshot.signed_pre_key.public_key,
                device_snapshot.signed_pre_key_signature.to_vec(),
                device_identity_bytes,
            ))
        } else {
            None
        };

        let receipt_to = if info.source.is_group {
            &info.source.chat
        } else {
            &info.source.sender
        };

        // Build the receipt node. For group messages, include the participant attribute
        // to identify which group member should resend. For DMs, omit it since the
        // "to" address already identifies the sender.
        let mut builder = NodeBuilder::new("receipt")
            .attr("to", receipt_to)
            .attr("id", info.id.clone())
            .attr("type", "retry");

        if info.source.is_group {
            builder = builder.attr("participant", &info.source.sender);
        }

        // Handle peer vs device sync messages (matches WhatsApp Web's sendRetryReceipt):
        // WhatsApp Web checks: if (to.isUser()) { if (isMeAccount(to)) { ... } }
        // This means the category/recipient logic ONLY applies to DMs (not groups).
        // For groups, only the participant attribute is set (handled above).
        if !info.source.is_group {
            let is_from_own_account = device_snapshot
                .pn
                .as_ref()
                .is_some_and(|pn| info.source.sender.is_same_user_as(pn))
                || device_snapshot
                    .lid
                    .as_ref()
                    .is_some_and(|lid| info.source.sender.is_same_user_as(lid));

            if is_from_own_account {
                if info.category == MessageCategory::Peer {
                    builder = builder.attr("category", MessageCategory::Peer.as_str());
                } else {
                    // Include recipient so the sender can look up the original message.
                    // Without this, the retry fails silently (getTargetChat returns null).
                    let recipient = info.source.recipient.as_ref().unwrap_or(&info.source.chat);
                    builder = builder.attr("recipient", recipient);
                }
            }
        }

        // Build children list - keys are only included when retryCount >= 2
        let receipt_node = if let Some(keys) = keys_node {
            builder
                .children([retry_node, registration_node, keys])
                .build()
        } else {
            builder.children([retry_node, registration_node]).build()
        };

        self.send_node(receipt_node).await?;
        Ok(())
    }

    /// Sends an `enc_rekey_retry` receipt for VoIP call encryption re-keying.
    ///
    /// WA Web: When a peer fails to decrypt VoIP call encryption data (e.g.,
    /// `<enc>` within a `<call>` stanza), the receiver sends this receipt asking
    /// the sender to re-key.  The receipt uses `<enc_rekey>` child instead of
    /// `<retry>`, carrying VoIP call context (`call-id`, `call-creator`).
    ///
    /// WA Web reference: `ENC_RETRY_RECEIPT_ATTRS.GROUP_CALL = "enc_rekey_retry"`,
    /// constructed in `WAWebVoipSignalingEnums` module.
    #[allow(dead_code)] // Will be used when call handling is implemented (#345)
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.retry.send_enc_rekey_receipt", level = "debug", skip_all, fields(peer = %peer_jid.observe(), retry = retry_count), err(Debug)))]
    pub(crate) async fn send_enc_rekey_retry_receipt(
        &self,
        stanza_id: &str,
        peer_jid: &wacore_binary::Jid,
        call_id: &str,
        call_creator: &wacore_binary::Jid,
        retry_count: u8,
    ) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();

        let registration_id_bytes = device_snapshot.registration_id.to_be_bytes().to_vec();

        // WA Web: <enc_rekey call-creator="JID" call-id="..." count="N"/>
        let enc_rekey_node = NodeBuilder::new("enc_rekey")
            .attr("call-creator", call_creator)
            .attr("call-id", call_id)
            .attr("count", retry_count)
            .build();

        let registration_node = NodeBuilder::new("registration")
            .bytes(registration_id_bytes)
            .build();

        let receipt_node = NodeBuilder::new("receipt")
            .attr("to", peer_jid)
            .attr("id", stanza_id)
            .attr("type", "enc_rekey_retry")
            .children([enc_rekey_node, registration_node])
            .build();

        info!(
            "Sending enc_rekey_retry receipt for call-id={} to {} (count={})",
            call_id,
            peer_jid.observe(),
            retry_count
        );

        self.send_node(receipt_node).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::persistence_manager::PersistenceManager;
    use crate::test_utils::MockHttpClient;
    use std::borrow::Cow;
    use std::sync::Arc;
    use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair};
    use wacore::types::jid::JidExt as _;
    use wacore_binary::{Jid, JidExt};
    use waproto::whatsapp as wa;

    fn resolve_retry_chat_info(
        receipt: &Receipt,
        node: &NodeRef<'_>,
        own_pn: Option<&Jid>,
        own_lid: Option<&Jid>,
    ) -> RetryChatInfo {
        super::resolve_retry_chat_info(receipt, node, own_pn, own_lid)
            .expect("retry should resolve a target chat")
    }

    fn maybe_resolve_retry_chat_info(
        receipt: &Receipt,
        node: &NodeRef<'_>,
        own_pn: Option<&Jid>,
        own_lid: Option<&Jid>,
    ) -> Option<RetryChatInfo> {
        super::resolve_retry_chat_info(receipt, node, own_pn, own_lid)
    }

    #[tokio::test]
    async fn recent_message_cache_insert_and_take() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        // Enable L1 cache so MockBackend (which doesn't persist) works for this test
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let chat: Jid = "120363021033254949@g.us"
            .parse()
            .expect("test JID should be valid");
        let msg_id = "ABC123".to_string();
        let msg = wa::Message {
            conversation: Some("hello".into()),
            ..Default::default()
        };

        // Insert via the new async API
        client.add_recent_message(&chat, &msg_id, &msg, None).await;

        // First take should return and remove it from cache
        let taken = client.take_recent_message(&chat, &msg_id).await;
        assert!(taken.is_some());
        let (msg, alt_chat) = taken.unwrap();
        assert!(alt_chat.is_none(), "primary key should match");
        assert_eq!(msg.conversation.as_deref(), Some("hello"));

        // Second take should return None
        let taken_again = client.take_recent_message(&chat, &msg_id).await;
        assert!(taken_again.is_none());
    }

    #[tokio::test]
    async fn peek_recent_message_does_not_consume() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let chat: Jid = "120363021033254949@g.us".parse().unwrap();
        let msg_id = "PEEK1".to_string();
        let msg = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        client.add_recent_message(&chat, &msg_id, &msg, None).await;

        // Peeking twice both return the message and leave it in the cache...
        for _ in 0..2 {
            let peeked = client.peek_recent_message(&chat, &msg_id).await;
            let (m, alt) = peeked.expect("peek should find the cached message");
            assert!(alt.is_none());
            assert_eq!(m.conversation.as_deref(), Some("hi"));
        }
        // ...so a subsequent take still finds it (peek didn't remove it).
        assert!(client.take_recent_message(&chat, &msg_id).await.is_some());
    }

    #[test]
    fn get_bytes_content_extracts_bytes() {
        use wacore_binary::{Attrs, Node};

        // Test with bytes content
        let node = Node {
            tag: Cow::Borrowed("test"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Bytes(vec![1, 2, 3, 4])),
        };
        assert_eq!(get_bytes_content(&node), Some(&[1, 2, 3, 4][..]));

        // Test with string content (should return None)
        let node_str = Node {
            tag: Cow::Borrowed("test"),
            attrs: Attrs::new(),
            content: Some(NodeContent::String("hello".into())),
        };
        assert_eq!(get_bytes_content(&node_str), None);

        // Test with no content
        let node_empty = Node {
            tag: Cow::Borrowed("test"),
            attrs: Attrs::new(),
            content: None,
        };
        assert_eq!(get_bytes_content(&node_empty), None);
    }

    #[test]
    fn peer_detection_logic() {
        let our_jid = Jid::pn("559911112222");
        let peer_jid = Jid::pn_device("559911112222", 1);
        let other_jid = Jid::pn("559933334444");

        assert_eq!(our_jid.user, peer_jid.user);
        assert_ne!(our_jid.user, other_jid.user);
    }

    /// Integration test for retry receipt attribute logic.
    /// Tests the fix for lost device sync messages (AC7B18EBD4445BFC55C0EA3CF9F913F8 case).
    /// Matches WhatsApp Web's sendRetryReceipt: if (to.isUser()) { if (isMeAccount(to)) { ... } }
    #[test]
    fn retry_receipt_attributes_for_device_sync_vs_peer_vs_group() {
        use wacore::types::message::{MessageCategory, MessageInfo, MessageSource};
        use wacore_binary::builder::NodeBuilder;

        let our_pn = Jid::pn("559999999999");
        let our_lid = Jid::lid("100000000000001");

        fn build_retry_receipt(
            info: &MessageInfo,
            our_pn: &Jid,
            our_lid: &Jid,
        ) -> wacore_binary::Node {
            // Mirror production routing: groups → chat JID, DMs → sender JID
            let receipt_to = if info.source.is_group {
                &info.source.chat
            } else {
                &info.source.sender
            };
            let mut builder = NodeBuilder::new("receipt")
                .attr("to", receipt_to)
                .attr("id", info.id.clone())
                .attr("type", "retry");

            if info.source.is_group {
                builder = builder.attr("participant", &info.source.sender);
            }

            if !info.source.is_group {
                let is_from_own_account = info.source.sender.is_same_user_as(our_pn)
                    || info.source.sender.is_same_user_as(our_lid);

                if is_from_own_account {
                    if info.category == MessageCategory::Peer {
                        builder = builder.attr("category", MessageCategory::Peer.as_str());
                    } else {
                        let recipient = info.source.recipient.as_ref().unwrap_or(&info.source.chat);
                        builder = builder.attr("recipient", recipient);
                    }
                }
            }

            builder.build()
        }

        // Case 1: Device sync DM
        let recipient_lid = Jid::lid("200000000000002");
        let device_sync_info = MessageInfo {
            id: "DEVICE_SYNC_MSG_001".to_string(),
            source: MessageSource {
                chat: recipient_lid.clone(),
                sender: our_lid.clone(),
                is_from_me: true,
                is_group: false,
                recipient: Some(recipient_lid.clone()),
                ..Default::default()
            },
            category: MessageCategory::default(),
            ..Default::default()
        };

        let node = build_retry_receipt(&device_sync_info, &our_pn, &our_lid);
        assert_eq!(
            node.attrs
                .get("recipient")
                .map(|v| v == "200000000000002@lid"),
            Some(true),
            "Device sync DM should include recipient"
        );
        assert!(
            node.attrs.get("category").is_none(),
            "Device sync DM should NOT have category=peer"
        );
        assert!(
            node.attrs.get("participant").is_none(),
            "DM should NOT have participant"
        );

        // Case 2: Peer DM with category="peer"
        let other_pn = Jid::pn("551188888888");
        let peer_info = MessageInfo {
            id: "PEER123".to_string(),
            source: MessageSource {
                chat: other_pn.clone(),
                sender: our_pn.clone(),
                is_from_me: true,
                is_group: false,
                recipient: None,
                ..Default::default()
            },
            category: MessageCategory::Peer,
            ..Default::default()
        };

        let node = build_retry_receipt(&peer_info, &our_pn, &our_lid);
        assert_eq!(
            node.attrs.get("category").map(|v| v == "peer"),
            Some(true),
            "Peer DM should have category=peer"
        );
        assert!(
            node.attrs.get("recipient").is_none(),
            "Peer DM should NOT have recipient"
        );

        // Case 3: Group message from our own account
        let group_info = MessageInfo {
            id: "GROUP123".to_string(),
            source: MessageSource {
                chat: "123456789@g.us".parse().unwrap(),
                sender: our_lid.clone(),
                is_from_me: true,
                is_group: true,
                recipient: None,
                ..Default::default()
            },
            category: MessageCategory::default(),
            ..Default::default()
        };

        let node = build_retry_receipt(&group_info, &our_pn, &our_lid);
        assert!(
            node.attrs.get("participant").is_some(),
            "Group should have participant"
        );
        assert!(
            node.attrs.get("category").is_none(),
            "Group should NOT have category"
        );
        assert!(
            node.attrs.get("recipient").is_none(),
            "Group should NOT have recipient"
        );

        // Case 4: DM from someone else
        let other_dm_info = MessageInfo {
            id: "OTHER123".to_string(),
            source: MessageSource {
                chat: other_pn.clone(),
                sender: other_pn.clone(),
                is_from_me: false,
                is_group: false,
                recipient: None,
                ..Default::default()
            },
            category: MessageCategory::default(),
            ..Default::default()
        };

        let node = build_retry_receipt(&other_dm_info, &our_pn, &our_lid);
        assert!(
            node.attrs.get("category").is_none(),
            "DM from other should NOT have category"
        );
        assert!(
            node.attrs.get("recipient").is_none(),
            "DM from other should NOT have recipient"
        );
    }

    /// Verify enc_rekey_retry receipt node structure matches WhatsApp Web:
    /// <receipt to="peer" id="stanza_id" type="enc_rekey_retry">
    ///   <enc_rekey call-creator="creator_jid" call-id="..." count="N"/>
    ///   <registration>{4-byte big-endian reg id}</registration>
    /// </receipt>
    #[test]
    fn enc_rekey_retry_receipt_node_structure() {
        use wacore_binary::builder::NodeBuilder;

        let peer_jid: Jid = "5511999999999@s.whatsapp.net".parse().expect("peer JID");
        let call_creator: Jid = "5511888888888@s.whatsapp.net".parse().expect("creator JID");
        let call_id = "CALL-ABC-123";
        let stanza_id = "3EB0AABBCCDD";
        let retry_count: u8 = 2;
        let registration_id: u32 = 12345;

        // Build the receipt exactly as send_enc_rekey_retry_receipt does
        let enc_rekey_node = NodeBuilder::new("enc_rekey")
            .attr("call-creator", call_creator)
            .attr("call-id", call_id)
            .attr("count", retry_count)
            .build();

        let registration_node = NodeBuilder::new("registration")
            .bytes(registration_id.to_be_bytes().to_vec())
            .build();

        let receipt_node = NodeBuilder::new("receipt")
            .attr("to", peer_jid)
            .attr("id", stanza_id)
            .attr("type", "enc_rekey_retry")
            .children([enc_rekey_node, registration_node])
            .build();

        // Verify top-level receipt attributes
        assert_eq!(
            receipt_node.attrs().optional_string("type").as_deref(),
            Some("enc_rekey_retry"),
            "receipt type must be enc_rekey_retry"
        );
        assert!(
            receipt_node
                .attrs
                .get("to")
                .is_some_and(|v| *v == "5511999999999@s.whatsapp.net"),
            "receipt 'to' must be peer JID"
        );
        assert_eq!(
            receipt_node.attrs().optional_string("id").as_deref(),
            Some("3EB0AABBCCDD")
        );

        // Verify <enc_rekey> child (NOT <retry>)
        assert!(
            receipt_node.get_optional_child("retry").is_none(),
            "enc_rekey_retry must NOT contain <retry> child"
        );
        let enc_rekey = receipt_node
            .get_optional_child("enc_rekey")
            .expect("<enc_rekey> child must exist");
        assert_eq!(
            enc_rekey.attrs().optional_string("call-id").as_deref(),
            Some("CALL-ABC-123")
        );
        assert!(
            enc_rekey
                .attrs
                .get("call-creator")
                .is_some_and(|v| *v == "5511888888888@s.whatsapp.net"),
            "enc_rekey 'call-creator' must be creator JID"
        );
        assert_eq!(
            enc_rekey.attrs().optional_string("count").as_deref(),
            Some("2")
        );

        // Verify <registration> child
        let registration = receipt_node
            .get_optional_child("registration")
            .expect("<registration> child must exist");
        let reg_bytes = match &registration.content {
            Some(wacore_binary::NodeContent::Bytes(b)) => b.clone(),
            _ => panic!("registration must contain bytes"),
        };
        assert_eq!(
            u32::from_be_bytes(reg_bytes.try_into().unwrap()),
            12345,
            "registration ID must be 4-byte big-endian"
        );
    }

    #[test]
    fn prekey_id_parsing() {
        // PreKey IDs are 3 bytes big-endian
        let id_bytes = [0x01, 0x02, 0x03];
        let prekey_id = u32::from_be_bytes([0, id_bytes[0], id_bytes[1], id_bytes[2]]);
        assert_eq!(prekey_id, 0x00010203);

        // Signed prekey IDs follow the same format
        let skey_id_bytes = [0xFF, 0xFE, 0xFD];
        let skey_id = u32::from_be_bytes([0, skey_id_bytes[0], skey_id_bytes[1], skey_id_bytes[2]]);
        assert_eq!(skey_id, 0x00FFFEFD);
    }

    #[tokio::test]
    async fn base_key_store_operations() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;

        let address = "12345.0:1";
        let msg_id = "ABC123";
        let base_key = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        // Initially, has_same_base_key should return false (no saved key)
        let result = backend.has_same_base_key(address, msg_id, &base_key).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Save the base key
        let save_result = backend.save_base_key(address, msg_id, &base_key).await;
        assert!(save_result.is_ok());

        // Same key should now match (collision detected)
        let result = backend.has_same_base_key(address, msg_id, &base_key).await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // Different key should NOT match (no collision)
        let different_key = vec![10, 9, 8, 7, 6, 5, 4, 3, 2, 1];
        let result = backend
            .has_same_base_key(address, msg_id, &different_key)
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap());

        // Delete the base key
        let delete_result = backend.delete_base_key(address, msg_id).await;
        assert!(delete_result.is_ok());

        // After deletion, has_same_base_key should return false
        let result = backend.has_same_base_key(address, msg_id, &base_key).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn base_key_store_upsert() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;

        let address = "12345.0:1";
        let msg_id = "MSG001";
        let first_key = vec![1, 2, 3];
        let second_key = vec![4, 5, 6];

        // Save first key
        backend
            .save_base_key(address, msg_id, &first_key)
            .await
            .unwrap();
        assert!(
            backend
                .has_same_base_key(address, msg_id, &first_key)
                .await
                .unwrap()
        );
        assert!(
            !backend
                .has_same_base_key(address, msg_id, &second_key)
                .await
                .unwrap()
        );

        // Save second key (upsert should replace)
        backend
            .save_base_key(address, msg_id, &second_key)
            .await
            .unwrap();
        assert!(
            !backend
                .has_same_base_key(address, msg_id, &first_key)
                .await
                .unwrap()
        );
        assert!(
            backend
                .has_same_base_key(address, msg_id, &second_key)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn base_key_store_multiple_messages() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;

        let address = "12345.0:1";
        let msg_id_1 = "MSG001";
        let msg_id_2 = "MSG002";
        let key_1 = vec![1, 2, 3];
        let key_2 = vec![4, 5, 6];

        // Save keys for different messages
        backend
            .save_base_key(address, msg_id_1, &key_1)
            .await
            .unwrap();
        backend
            .save_base_key(address, msg_id_2, &key_2)
            .await
            .unwrap();

        // Each message should have its own key
        assert!(
            backend
                .has_same_base_key(address, msg_id_1, &key_1)
                .await
                .unwrap()
        );
        assert!(
            !backend
                .has_same_base_key(address, msg_id_1, &key_2)
                .await
                .unwrap()
        );
        assert!(
            !backend
                .has_same_base_key(address, msg_id_2, &key_1)
                .await
                .unwrap()
        );
        assert!(
            backend
                .has_same_base_key(address, msg_id_2, &key_2)
                .await
                .unwrap()
        );

        // Delete one message's key, other should remain
        backend.delete_base_key(address, msg_id_1).await.unwrap();
        assert!(
            !backend
                .has_same_base_key(address, msg_id_1, &key_1)
                .await
                .unwrap()
        );
        assert!(
            backend
                .has_same_base_key(address, msg_id_2, &key_2)
                .await
                .unwrap()
        );
    }

    /// Build a minimal `<receipt>` Node representing an incoming retry receipt
    /// without `<keys>`. Used by tests that exercise the no-bundle path of
    /// `update_local_signal_session`.
    fn build_retry_receipt_without_keys() -> Node {
        use wacore_binary::builder::NodeBuilder;
        NodeBuilder::new("receipt").build()
    }

    /// Build a `<receipt>` with a `<registration>` child carrying `reg_id` (big
    /// endian). Used to exercise the reg-ID-mismatch branch without a full
    /// `<keys>` bundle.
    fn build_retry_receipt_with_registration(reg_id: u32) -> Node {
        use wacore_binary::builder::NodeBuilder;
        NodeBuilder::new("receipt")
            .children([NodeBuilder::new("registration")
                .bytes(reg_id.to_be_bytes().to_vec())
                .build()])
            .build()
    }

    fn dm_retry_info(resolved_jid: &Jid) -> RetryChatInfo {
        RetryChatInfo {
            chat: resolved_jid.to_non_ad(),
            requester: resolved_jid.clone(),
            original_from: resolved_jid.clone(),
            recipient: None,
            is_bot: false,
            is_fbid_bot_retry: false,
        }
    }

    // Produces a parseable SessionRecord so peek_session succeeds and
    // alice_base_key/remote_registration_id return meaningful values.
    fn valid_serialized_session(remote_regid: u32, base_key: Vec<u8>) -> Vec<u8> {
        use wacore::libsignal::protocol::{SessionRecord, SessionState};
        use waproto::whatsapp::SessionStructure;

        let state = SessionState::from_session_structure(SessionStructure {
            session_version: Some(3),
            local_identity_public: None,
            remote_identity_public: None,
            root_key: None,
            previous_counter: Some(0),
            sender_chain: buffa::MessageField::default(),
            receiver_chains: vec![],
            pending_pre_key: buffa::MessageField::default(),
            remote_registration_id: Some(remote_regid),
            local_registration_id: Some(0),
            alice_base_key: Some(base_key),
            needs_refresh: None,
            pending_key_exchange: buffa::MessageField::default(),
        });
        SessionRecord::new(state)
            .serialize()
            .expect("serialize session record")
    }

    /// WA Web compliance: at retry #1 with no `<keys>`, `updateLocalSignalSession`
    /// does NOT delete the session. Previously the Rust DM path unconditionally
    /// deleted on every retry — this regressed legitimate sessions and forced
    /// unnecessary prekey bundle fetches.
    /// Ref: `WAWeb/Update/LocalSignalSession.js` (no delete on retry==1)
    #[tokio::test]
    async fn update_local_signal_session_preserves_dm_session_at_retry_1() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("retry_preserve_retry_1").await;
        let user = "100000000000088".to_string();
        let resolved_jid = Jid::lid_device(user.clone(), 33);

        let backend = client.persistence_manager.backend();
        let device_0 = Jid::lid_device(user.clone(), 0).to_protocol_address();
        let device_33 = Jid::lid_device(user, 33).to_protocol_address();

        // Real serializable SessionRecords — peek_session must return Some(...)
        // so the function reaches the base-key branch at retry==1 and exercises
        // the "no delete" rule. Invalid bytes would short-circuit via .ok().flatten().
        let session_bytes_33 = valid_serialized_session(4242, vec![0xAA; 32]);
        let session_bytes_0 = valid_serialized_session(4243, vec![0xBB; 32]);
        backend
            .put_session(device_0.as_str(), &session_bytes_0)
            .await
            .unwrap();
        backend
            .put_session(device_33.as_str(), &session_bytes_33)
            .await
            .unwrap();

        let node = build_retry_receipt_without_keys();
        let node_ref = node.as_node_ref();
        client
            .update_local_signal_session(
                &dm_retry_info(&resolved_jid),
                &resolved_jid,
                "MSG-RETRY-1",
                1,
                &node_ref,
                false,
            )
            .await;
        client.flush_signal_cache().await.unwrap();

        assert!(
            backend
                .get_session(device_0.as_str())
                .await
                .unwrap()
                .is_some(),
            "non-requesting device session must be preserved"
        );
        assert!(
            backend
                .get_session(device_33.as_str())
                .await
                .unwrap()
                .is_some(),
            "requesting device session with valid record must be preserved at retry #1"
        );
    }

    /// Production scenario from debug-1776271138: peer sends retry receipt
    /// without `<keys>` but with `<registration>` whose reg_id differs from
    /// our stored session. WA Web deletes the session (LocalSignalSession.js
    /// L52-65) so the next ensureE2ESessions fetches a fresh bundle.
    #[tokio::test]
    async fn update_local_signal_session_deletes_on_regid_mismatch() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("retry_regid_mismatch").await;
        let resolved_jid = Jid::lid_device("100000000000099".to_string(), 17);
        let signal_address = resolved_jid.to_protocol_address();
        let backend = client.persistence_manager.backend();

        let stored_regid = 4242u32;
        let session_bytes = valid_serialized_session(stored_regid, vec![0xAA; 32]);
        backend
            .put_session(signal_address.as_str(), &session_bytes)
            .await
            .unwrap();

        let received_regid = 0xDEAD_BEEFu32;
        assert_ne!(stored_regid, received_regid);
        let node = build_retry_receipt_with_registration(received_regid);
        let node_ref = node.as_node_ref();
        client
            .update_local_signal_session(
                &dm_retry_info(&resolved_jid),
                &resolved_jid,
                "MSG-REGID",
                1,
                &node_ref,
                false,
            )
            .await;
        client.flush_signal_cache().await.unwrap();

        assert!(
            backend
                .get_session(signal_address.as_str())
                .await
                .unwrap()
                .is_none(),
            "session must be deleted when retry has no keys and reg IDs differ"
        );
    }

    /// Unparseable session bytes: peek_session returns None via .ok().flatten(),
    /// so every branch that dereferences a session is skipped. Verifies we
    /// don't panic or re-process stale bytes when the record can't decode.
    #[tokio::test]
    async fn update_local_signal_session_handles_unparseable_session_gracefully() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("retry_unparseable_session")
                .await;
        let resolved_jid = Jid::lid_device("100000000000099".to_string(), 17);
        let signal_address = resolved_jid.to_protocol_address();
        let backend = client.persistence_manager.backend();

        backend
            .put_session(signal_address.as_str(), b"invalid-session")
            .await
            .unwrap();

        let node = build_retry_receipt_with_registration(0xDEAD_BEEF);
        let node_ref = node.as_node_ref();
        client
            .update_local_signal_session(
                &dm_retry_info(&resolved_jid),
                &resolved_jid,
                "MSG-REGID",
                1,
                &node_ref,
                false,
            )
            .await;
        client.flush_signal_cache().await.unwrap();

        assert!(
            backend
                .get_session(signal_address.as_str())
                .await
                .unwrap()
                .is_some(),
            "unparseable bytes skip every branch; nothing should delete them"
        );
    }

    /// Verify the function is a safe no-op when there is no session at all.
    /// This is the common case for retries from devices we haven't messaged
    /// yet (e.g., a new companion device).
    #[tokio::test]
    async fn update_local_signal_session_no_session_is_noop() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("retry_no_session").await;
        let resolved_jid = Jid::lid_device("100000000000199".to_string(), 42);
        let node = build_retry_receipt_without_keys();
        let node_ref = node.as_node_ref();
        client
            .update_local_signal_session(
                &dm_retry_info(&resolved_jid),
                &resolved_jid,
                "MSG-NOSESS",
                1,
                &node_ref,
                false,
            )
            .await;
    }

    /// Group/status at retry #1 must not delete any session. Group/status
    /// previously skipped the base-key path entirely; now it runs but the
    /// retry==1 short-circuit still prevents deletion.
    #[tokio::test]
    async fn update_local_signal_session_preserves_group_session_at_retry_1() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("retry_group_preserve").await;
        let resolved_jid = Jid::lid_device("100000000000088".to_string(), 33);
        let signal_address = resolved_jid.to_protocol_address();
        let backend = client.persistence_manager.backend();

        let session_bytes = valid_serialized_session(9999, vec![0xCC; 32]);
        backend
            .put_session(signal_address.as_str(), &session_bytes)
            .await
            .unwrap();

        let group_chat: Jid = "120363042537531116@g.us".parse().unwrap();
        let info = RetryChatInfo {
            chat: group_chat.clone(),
            requester: resolved_jid.clone(),
            original_from: group_chat,
            recipient: None,
            is_bot: false,
            is_fbid_bot_retry: false,
        };

        let node = build_retry_receipt_without_keys();
        let node_ref = node.as_node_ref();
        client
            .update_local_signal_session(&info, &resolved_jid, "MSG-GRP-1", 1, &node_ref, false)
            .await;
        client.flush_signal_cache().await.unwrap();

        assert!(
            backend
                .get_session(signal_address.as_str())
                .await
                .unwrap()
                .is_some(),
            "group retry at #1 should not delete the session"
        );
    }

    /// `should_recreate_session` mirrors whatsmeow `shouldRecreateSession`:
    /// 1) no session → always recreate;
    /// 2) session exists + retry<2 → never recreate;
    /// 3) session exists + retry≥2 + first time (or >1h since last) → recreate.
    /// 4) session exists + retry≥2 + recreated <1h ago → throttled, do not recreate.
    #[tokio::test]
    async fn should_recreate_session_matrix() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("should_recreate_session")
                .await;

        // Use disjoint JIDs per scenario so the negative-cache populated by
        // `has_session` on the "no session" branch can't shadow the later
        // backend put for the "session present" branches.
        let jid_with = Jid::lid_device("999999999999991".to_string(), 3);
        let jid_without = Jid::lid_device("999999999999992".to_string(), 3);

        // Seed a session for jid_with BEFORE the first has_session lookup so
        // the cache caches the hit, not the miss.
        let session_bytes = valid_serialized_session(7777, vec![0xEE; 32]);
        client
            .persistence_manager
            .backend()
            .put_session(jid_with.to_protocol_address().as_str(), &session_bytes)
            .await
            .unwrap();

        // 1) session present + retry<2 → never recreate, no history stamp.
        assert!(
            client.should_recreate_session(1, &jid_with).await.is_none(),
            "retry<2 with session present should not recreate"
        );
        assert!(
            client
                .session_recreate_history
                .get(&jid_with)
                .await
                .is_none(),
            "no-op path must not stamp the history"
        );

        // 2) session present + retry≥2 + cold history → recreate, stamp history.
        assert!(
            client
                .should_recreate_session(2, &jid_with)
                .await
                .is_some_and(|r| r.contains("retry count > 1")),
            "retry≥2 with cold history should recreate"
        );
        let after_first = client.session_recreate_history.get(&jid_with).await;
        assert!(after_first.is_some(), "first recreate must stamp history");

        // 3) session present + retry≥2 + recent history → throttled.
        assert!(
            client.should_recreate_session(3, &jid_with).await.is_none(),
            "retry≥2 within {}s should be throttled",
            RECREATE_SESSION_TIMEOUT.as_secs()
        );
        let after_second = client.session_recreate_history.get(&jid_with).await;
        assert_eq!(
            after_first, after_second,
            "throttled path must not re-stamp the history"
        );

        // 4) Past the throttle window → fresh recreate. Use a future `now`
        // (subtracting from a young runtime's Instant would saturate to zero).
        let stamp_then = after_first.expect("first recreate stamped history");
        let well_past = stamp_then + RECREATE_SESSION_TIMEOUT + std::time::Duration::from_secs(1);
        assert!(
            client
                .should_recreate_session_at(3, &jid_with, well_past)
                .await
                .is_some_and(|r| r.contains("over an hour")),
            "entry past the throttle window must allow a fresh recreate"
        );

        // 5) no session → recreate regardless of retry count.
        assert!(
            client
                .should_recreate_session(0, &jid_without)
                .await
                .is_some_and(|r| r.contains("don't have a Signal session")),
            "missing session should recreate"
        );
    }

    /// The `session_recreate_history` is capacity-bounded (256), unlike the
    /// old age-only prune which never evicted a still-recent entry. Under more
    /// than that many distinct peers retrying within the window, the cache can evict
    /// a recent entry, costing at most one extra recreate for that peer
    /// (bounded and self-healing: re-stamped on the next receipt), never the
    /// unbounded prekey loop the throttle prevents. Documents that trade-off.
    #[tokio::test]
    async fn session_recreate_history_is_capacity_bounded() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("session_recreate_history_cap")
                .await;
        let now = wacore::time::Instant::now();
        let cap: u64 = 256;

        // Insert well over the cap of distinct, all-recent peers.
        for i in 0..(cap * 2) {
            let jid = Jid::lid_device(format!("{}", 900_000_000_000_000u64 + i), 3);
            client.session_recreate_history.insert(jid, now).await;
        }
        client.session_recreate_history.run_pending_tasks().await;

        let count = client.session_recreate_history.entry_count();
        assert!(
            count <= cap,
            "capacity must bound the throttle history (got {count}, cap {cap}); \
             a still-recent entry can be evicted under heavy peer load"
        );
    }

    /// The resend rate limiter is reachable and tunable through the public
    /// `Client` API, and its drops surface on `resends_throttled_total`. Covers
    /// the wiring the `handle_retry_receipt` hook relies on; the bucket logic
    /// itself is unit-tested in `resend_rate_limiter`.
    #[tokio::test]
    async fn client_resend_rate_limiter_is_wired_and_tunable() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("resend_rl_wired").await;
        let chat: Jid = "120363021033254949@g.us".parse().unwrap();

        // Tight ceiling, no refill: the bucket holds exactly `burst` tokens.
        client.set_resend_rate_limit(3, 0);
        let mut allowed = 0;
        for _ in 0..10 {
            if client.resend_rate_limiter.try_acquire(&chat).await {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 3, "client honors the configured per-chat burst");
        assert_eq!(
            client.resends_throttled_total(),
            7,
            "public counter tracks dropped resends"
        );

        // Disabling restores unthrottled behavior.
        client.set_resend_rate_limit(0, 0);
        let other: Jid = "120363000000000001@g.us".parse().unwrap();
        for _ in 0..50 {
            assert!(client.resend_rate_limiter.try_acquire(&other).await);
        }
    }

    /// End-to-end: a throttled group retry drops the resend (returns Ok, sends
    /// nothing) while the path up to the limiter still runs, and the cached
    /// message is retained for the device's later re-request. Exercises the hook
    /// placement and the no-resend-on-refusal semantics the unit tests cannot.
    #[tokio::test]
    async fn handle_retry_receipt_drops_throttled_group_resend() {
        use wacore_binary::builder::NodeBuilder;

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let group: Jid = "120363021033254949@g.us".parse().unwrap();
        let msg_id = "RLMSG001";
        client
            .add_recent_message(
                &group,
                msg_id,
                &wa::Message {
                    conversation: Some("hi".into()),
                    ..Default::default()
                },
                None,
            )
            .await;

        // Drain the single token so the incoming retry must be throttled; the
        // throttle returns before any network resend, keeping the test offline.
        client.set_resend_rate_limit(1, 0);
        assert!(client.resend_rate_limiter.try_acquire(&group).await);

        // Inbound group retry from a device-0 LID participant: has_device's
        // device-0 fast path makes it known, LID skips rotateKey, and no <keys>
        // leaves update_local_signal_session a noop on a missing session.
        let node = NodeBuilder::new("receipt")
            .attr("participant", "555000111@lid")
            .children([NodeBuilder::new("retry")
                .attr("id", msg_id)
                .attr("count", "1")
                .build()])
            .build();
        let node_ref = crate::test_utils::node_to_owned_ref(&node);
        let receipt = Receipt {
            source: crate::types::message::MessageSource {
                chat: group.clone(),
                sender: "555000111@lid".parse().unwrap(),
                is_group: true,
                ..Default::default()
            },
            message_ids: vec![msg_id.to_string()],
            timestamp: wacore::time::now_utc(),
            r#type: crate::types::presence::ReceiptType::Retry,
            offline: false,
        };

        let result = client.handle_retry_receipt(&receipt, &node_ref).await;
        assert!(
            result.is_ok(),
            "a throttled retry returns Ok(()), not an error"
        );
        assert_eq!(
            client.resends_throttled_total(),
            1,
            "the resend was dropped by the limiter"
        );
        assert!(
            client.peek_recent_message(&group, msg_id).await.is_some(),
            "throttling keeps the message cached for the device's re-request"
        );
        assert_eq!(
            client.pending_retries.lock().unwrap().len(),
            0,
            "the in-progress marker is cleared after the throttled return"
        );
    }

    /// Atomicity guard for the per-peer session lock the retry caller wraps
    /// around the recreate check+stamp. The cache's get+insert is not atomic, and
    /// same-peer retries for different message_ids dispatch concurrently, so
    /// without the lock both could observe a cold history and recreate. Holding
    /// `session_lock_for` serializes the decision: exactly one recreate fires.
    /// (Mirrors the caller's lock; the matrix test covers the sequential logic.)
    #[tokio::test]
    async fn concurrent_same_peer_recreate_check_is_serialized() {
        let client =
            crate::test_utils::create_test_client_with_failing_http("concurrent_recreate").await;
        let jid = Jid::lid_device("999999999999993".to_string(), 3);

        // Seed a session so the retry>=2 throttle branch is exercised (the
        // no-session branch always stamps and would not show serialization).
        let session_bytes = valid_serialized_session(8888, vec![0xCC; 32]);
        client
            .persistence_manager
            .backend()
            .put_session(jid.to_protocol_address().as_str(), &session_bytes)
            .await
            .unwrap();

        let c1 = client.clone();
        let j1 = jid.clone();
        let task1 = async move {
            let addr = j1.to_protocol_address();
            let lock = c1.session_lock_for(addr.as_str()).await;
            let _g = lock.lock().await;
            c1.should_recreate_session(2, &j1).await.is_some()
        };
        let c2 = client.clone();
        let j2 = jid.clone();
        let task2 = async move {
            let addr = j2.to_protocol_address();
            let lock = c2.session_lock_for(addr.as_str()).await;
            let _g = lock.lock().await;
            c2.should_recreate_session(2, &j2).await.is_some()
        };
        let (a, b) = tokio::join!(task1, task2);

        assert_eq!(
            usize::from(a) + usize::from(b),
            1,
            "exactly one of two concurrent same-peer recreate checks may fire; \
             the per-peer session lock serializes the non-atomic get+insert"
        );
    }

    /// WA Web calls `ensureE2ESessions([g])` before resending for all chat types
    /// (RetryRequest.js:200). When the session already exists, this MUST be a
    /// fast no-op — otherwise group/status retries would hit the network on
    /// every receipt, defeating the cache. Regression guard for the group-branch
    /// call added alongside this test.
    #[tokio::test]
    async fn ensure_e2e_sessions_resolved_is_noop_when_session_exists() {
        use std::sync::atomic::Ordering;

        let client = crate::test_utils::create_test_client_with_failing_http(
            "group_retry_ensure_sessions_noop",
        )
        .await;

        // Bypass the offline-delivery wait that ensureE2ESessions does first.
        client.offline_sync_completed.store(true, Ordering::Relaxed);

        let resolved_jid = Jid::lid_device("100000000000199".to_string(), 17);
        let signal_address = resolved_jid.to_protocol_address();

        let session_bytes = valid_serialized_session(5555, vec![0xDD; 32]);
        client
            .persistence_manager
            .backend()
            .put_session(signal_address.as_str(), &session_bytes)
            .await
            .unwrap();

        // With a session present, no prekey fetch should happen (the test
        // client has no wired IQ responder, so a fetch would hang/error).
        client
            .ensure_e2e_sessions_resolved(std::slice::from_ref(&resolved_jid))
            .await
            .expect("no-op when session exists");
    }

    #[tokio::test]
    async fn retry_key_bundle_requires_one_time_prekey_except_fbid_bot() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _sync_rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm,
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let remote_identity = IdentityKeyPair::generate(&mut rng);
        let signed_prekey = KeyPair::generate(&mut rng);
        let signed_prekey_signature = remote_identity
            .private_key()
            .calculate_signature(&signed_prekey.public_key.serialize(), &mut rng)
            .expect("signed prekey signature should be valid");

        let regular_requester = Jid::pn_device("559922223333", 1);
        let keys = NodeBuilder::new("keys")
            .children([
                NodeBuilder::new("type").bytes(vec![5]).build(),
                NodeBuilder::new("identity")
                    .bytes(
                        remote_identity
                            .identity_key()
                            .public_key()
                            .public_key_bytes()
                            .to_vec(),
                    )
                    .build(),
                SignedPreKeyNode::new(
                    100,
                    signed_prekey.public_key.public_key_bytes().to_vec(),
                    signed_prekey_signature.to_vec(),
                )
                .into_node(),
            ])
            .build();
        let receipt = NodeBuilder::new("receipt")
            .children([
                NodeBuilder::new("registration")
                    .bytes(12345u32.to_be_bytes().to_vec())
                    .build(),
                keys,
            ])
            .build();

        let err = client
            .process_retry_key_bundle(&receipt.as_node_ref(), &regular_requester, false, false)
            .await
            .expect_err("regular retry without one-time prekey must be rejected");
        assert!(
            err.to_string()
                .contains("regular retry key bundle missing one-time prekey")
        );

        let fbid_bot_requester = Jid::new("200000000000002", wacore_binary::Server::Bot);
        client
            .process_retry_key_bundle(&receipt.as_node_ref(), &fbid_bot_requester, false, true)
            .await
            .expect("fbid bot retry without one-time prekey should establish a session");

        let snapshot = client.persistence_manager.get_device_snapshot();
        let session = client
            .signal_cache
            .peek_session(
                &fbid_bot_requester.to_protocol_address(),
                &*snapshot.backend,
            )
            .await
            .expect("session lookup should succeed");
        assert!(session.is_some());
    }

    #[test]
    fn bot_jid_detection() {
        // Test bot JID detection for bot message filtering
        use wacore_binary::JidExt as _;

        // Regular user JID - not a bot
        let regular_user: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        assert!(!regular_user.is_bot());

        // Bot JID with bot server
        let bot_server: Jid = "somebot@bot".parse().unwrap();
        assert!(bot_server.is_bot());

        // Legacy bot JID pattern (1313555...)
        let legacy_bot: Jid = "1313555123456@s.whatsapp.net".parse().unwrap();
        assert!(legacy_bot.is_bot());

        // Legacy bot JID pattern (131655500...)
        let legacy_bot2: Jid = "131655500123456@s.whatsapp.net".parse().unwrap();
        assert!(legacy_bot2.is_bot());

        // Similar but not bot (doesn't start with exact prefix)
        let not_bot: Jid = "1313556123456@s.whatsapp.net".parse().unwrap();
        assert!(!not_bot.is_bot());
    }

    #[test]
    fn extract_registration_id_from_node_test() {
        use wacore::protocol::retry::{
            extract_registration_id_from_node, extract_registration_id_from_node_ref,
        };
        use wacore_binary::{Attrs, Node};

        let reg_receipt = |bytes: Vec<u8>| Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![Node {
                tag: Cow::Borrowed("registration"),
                attrs: Attrs::new(),
                content: Some(NodeContent::Bytes(bytes)),
            }])),
        };

        // 4-byte registration ID.
        let parent = reg_receipt(vec![0x00, 0x01, 0x02, 0x03]);
        assert_eq!(extract_registration_id_from_node(&parent), Some(0x00010203));
        assert_eq!(
            extract_registration_id_from_node_ref(&parent.as_node_ref()),
            Some(0x00010203)
        );

        // 3-byte registration ID (variable length, left zero-padded).
        let parent_short = reg_receipt(vec![0x01, 0x02, 0x03]);
        assert_eq!(
            extract_registration_id_from_node(&parent_short),
            Some(0x00010203)
        );
        assert_eq!(
            extract_registration_id_from_node_ref(&parent_short.as_node_ref()),
            Some(0x00010203)
        );

        // Oversized (>4 byte) payload: rejected, not truncated, on both paths.
        let parent_oversized = reg_receipt(vec![0x01, 0x02, 0x03, 0x04, 0x05]);
        assert_eq!(extract_registration_id_from_node(&parent_oversized), None);
        assert_eq!(
            extract_registration_id_from_node_ref(&parent_oversized.as_node_ref()),
            None
        );

        // No registration node.
        let parent_no_reg = Node {
            tag: Cow::Borrowed("receipt"),
            attrs: Attrs::new(),
            content: Some(NodeContent::Nodes(vec![])),
        };
        assert_eq!(extract_registration_id_from_node(&parent_no_reg), None);
        assert_eq!(
            extract_registration_id_from_node_ref(&parent_no_reg.as_node_ref()),
            None
        );

        // Empty bytes.
        let parent_empty = reg_receipt(vec![]);
        assert_eq!(extract_registration_id_from_node(&parent_empty), None);
        assert_eq!(
            extract_registration_id_from_node_ref(&parent_empty.as_node_ref()),
            None
        );
    }

    #[test]
    fn group_or_status_detection_for_sender_key_handling() {
        // Test that both groups and status broadcasts trigger sender key handling
        use wacore_binary::JidExt as _;

        let group: Jid = "120363021033254949@g.us".parse().unwrap();
        let status: Jid = "status@broadcast".parse().unwrap();
        let dm: Jid = "1234567890@s.whatsapp.net".parse().unwrap();

        // Both group and status should trigger sender key deletion
        assert!(group.is_group() || group.is_status_broadcast());
        assert!(status.is_group() || status.is_status_broadcast());

        // DM should NOT trigger sender key deletion
        assert!(!(dm.is_group() || dm.is_status_broadcast()));
    }

    /// Test that verifies the key inclusion optimization:
    /// - Keys should be included on retry#1 for NoSession errors (the optimization)
    /// - Keys should NOT be included on retry#1 for other error types
    /// - Keys should be included on retry#2+ for ALL error types
    #[test]
    fn keys_inclusion_optimization_for_no_session_errors() {
        use crate::message::RetryReason;

        // Test cases: (retry_count, reason, should_include_keys)
        let test_cases = [
            // NoSession errors - optimization kicks in at retry#1
            (
                1,
                RetryReason::NoSession,
                true,
                "NoSession at retry#1 should include keys (optimization)",
            ),
            (
                2,
                RetryReason::NoSession,
                true,
                "NoSession at retry#2 should include keys",
            ),
            (
                3,
                RetryReason::NoSession,
                true,
                "NoSession at retry#3 should include keys",
            ),
            // InvalidMessage errors - no keys at retry#1, keys at retry#2+
            (
                1,
                RetryReason::InvalidMessage,
                false,
                "InvalidMessage at retry#1 should NOT include keys",
            ),
            (
                2,
                RetryReason::InvalidMessage,
                true,
                "InvalidMessage at retry#2 should include keys",
            ),
            (
                3,
                RetryReason::InvalidMessage,
                true,
                "InvalidMessage at retry#3 should include keys",
            ),
            // BadMac errors - same as InvalidMessage
            (
                1,
                RetryReason::BadMac,
                false,
                "BadMac at retry#1 should NOT include keys",
            ),
            (
                2,
                RetryReason::BadMac,
                true,
                "BadMac at retry#2 should include keys",
            ),
            // UnknownError - no keys at retry#1
            (
                1,
                RetryReason::UnknownError,
                false,
                "UnknownError at retry#1 should NOT include keys",
            ),
            (
                2,
                RetryReason::UnknownError,
                true,
                "UnknownError at retry#2 should include keys",
            ),
        ];

        for (retry_count, reason, should_include_keys, description) in test_cases {
            // Replicate the logic from send_retry_receipt
            let would_include_keys =
                wacore::protocol::retry::should_include_keys(retry_count, reason);

            assert_eq!(
                would_include_keys, should_include_keys,
                "Failed: {description}. retry_count={retry_count}, reason={reason:?}"
            );
        }
    }

    /// Integration test simulating high concurrent offline message scenarios.
    /// This tests the scenario where many skmsg-only messages arrive before SKDM,
    /// causing NoSession errors that need retry with keys.
    #[tokio::test]
    async fn concurrent_offline_messages_retry_key_optimization() {
        use crate::message::RetryReason;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::sync::Barrier;

        let _ = env_logger::builder().is_test(true).try_init();

        // Simulate processing multiple concurrent skmsg failures
        // Each represents a skmsg-only message from the same sender that failed with NoSession
        let num_messages = 50;
        let barrier = Arc::new(Barrier::new(num_messages));

        // Track how many would include keys on retry#1
        let keys_included_count = Arc::new(AtomicUsize::new(0));
        let no_keys_count = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();

        for i in 0..num_messages {
            let barrier = barrier.clone();
            let keys_included = keys_included_count.clone();
            let no_keys = no_keys_count.clone();

            handles.push(tokio::spawn(async move {
                // Simulate concurrent message processing
                barrier.wait().await;

                // Each message is a skmsg-only message that fails with NoSession
                // (simulating burst of group messages before SKDM arrives)
                let retry_count = 1; // First retry
                let reason = if i % 5 == 0 {
                    // Some messages have MAC failure (pkmsg failed)
                    RetryReason::InvalidMessage
                } else {
                    // Most are skmsg-only NoSession failures
                    RetryReason::NoSession
                };

                let would_include_keys =
                    wacore::protocol::retry::should_include_keys(retry_count, reason);

                if would_include_keys {
                    keys_included.fetch_add(1, Ordering::SeqCst);
                } else {
                    no_keys.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        // Wait for all tasks to complete
        for handle in handles {
            handle.await.expect("task should complete");
        }

        let total_keys_included = keys_included_count.load(Ordering::SeqCst);
        let total_no_keys = no_keys_count.load(Ordering::SeqCst);

        // With our optimization:
        // - 80% (40/50) are NoSession → keys included on retry#1
        // - 20% (10/50) are InvalidMessage → no keys on retry#1
        assert_eq!(
            total_keys_included, 40,
            "Expected 40 messages to include keys (NoSession), got {total_keys_included}"
        );
        assert_eq!(
            total_no_keys, 10,
            "Expected 10 messages to NOT include keys (InvalidMessage), got {total_no_keys}"
        );

        // Verify the optimization reduces round-trips
        // Without optimization: ALL 50 would need retry#2 for keys
        // With optimization: Only 10 need retry#2 for keys (80% improvement for NoSession)
        let optimization_benefit = (total_keys_included as f64 / num_messages as f64) * 100.0;
        assert!(
            optimization_benefit >= 80.0,
            "Optimization should benefit at least 80% of NoSession messages, got {optimization_benefit:.1}%"
        );
    }

    /// Test that the retry optimization correctly handles the edge case where
    /// a sender device is removed mid-retry (cannot respond to retry receipts).
    /// This tests our ability to handle the root cause of permanent failures.
    #[test]
    fn retry_optimization_with_removed_device_scenario() {
        use crate::message::RetryReason;

        // Simulate the scenario from the log:
        // 1. skmsg arrives → NoSession error → retry#1 with keys (optimization)
        // 2. Device is removed → no response to retry
        // 3. Message is permanently lost (expected behavior)

        let retry_count = 1;
        let reason = RetryReason::NoSession;

        // With optimization, we include keys on retry#1
        let would_include_keys = wacore::protocol::retry::should_include_keys(retry_count, reason);

        assert!(
            would_include_keys,
            "NoSession should include keys on retry#1 to give sender best chance to respond"
        );

        // Even if sender device is removed, we tried our best by including keys early
        // This reduces the window for message loss from:
        // - Before: retry#1 (no keys) → sender can't establish session → retry#2 (keys) → device removed
        // - After: retry#1 (keys) → sender can establish session immediately → device removed before response
        // The optimization gives the sender one fewer round-trip to respond.
    }

    /// Helper to build a DM Receipt for testing resolve_retry_chat_info.
    fn make_test_receipt(from: &str) -> Receipt {
        Receipt {
            source: crate::types::message::MessageSource {
                chat: from.parse().unwrap(),
                sender: from.parse().unwrap(),
                ..Default::default()
            },
            message_ids: vec!["MSG001".to_string()],
            timestamp: wacore::time::now_utc(),
            r#type: crate::types::presence::ReceiptType::Retry,
            offline: false,
        }
    }

    #[test]
    fn resolve_retry_chat_info_dm_with_device() {
        use wacore_binary::builder::NodeBuilder;

        // Node attrs are unused in the DM branch (no participant lookup)
        let node = NodeBuilder::new("receipt").build();
        let receipt = make_test_receipt("5511999999999:33@s.whatsapp.net");
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        // chat should be bare (device stripped)
        assert_eq!(info.chat.device(), 0);
        assert_eq!(info.chat.user, "5511999999999");
        assert!(info.chat.is_pn());

        // requester should preserve device 33
        assert_eq!(info.requester.device(), 33);
        assert_eq!(info.requester.user, "5511999999999");
    }

    #[test]
    fn resolve_retry_chat_info_lid_dm_with_device() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt").build();
        let receipt = make_test_receipt("236395184570386:5@lid");
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        // chat should be bare LID (device stripped)
        assert_eq!(info.chat.device(), 0);
        assert_eq!(info.chat.user, "236395184570386");
        assert!(info.chat.is_lid());

        // requester should preserve device 5
        assert_eq!(info.requester.device(), 5);
        assert_eq!(info.requester.user, "236395184570386");
        assert!(info.requester.is_lid());
    }

    /// `info.recipient` must come from the receipt's `recipient` attribute,
    /// not derived from `info.chat`. Pre-fix, the DM resend used
    /// `info.chat.clone()` for the stanza's `recipient` — fine on the primary
    /// namespace but wrong whenever `take_recent_message` hit `alt_chat` (the
    /// original was sent under PN while the receipt arrived under LID, or
    /// vice-versa). WA Web's `WAWebHandleRetryRequest` forwards the receipt
    /// attr verbatim (`f && (k.recipient = f)`), so the resend's `recipient`
    /// matches the original outbound's namespace regardless of how the
    /// receipt's `from` was addressed.
    #[test]
    fn resolve_retry_chat_info_forwards_recipient_attribute_verbatim() {
        use wacore_binary::builder::NodeBuilder;

        // Cross-namespace shape: receipt `from` is LID, `recipient` is PN.
        let node = NodeBuilder::new("receipt")
            .attr("recipient", "5500000000123@s.whatsapp.net")
            .build();
        let receipt = make_test_receipt("100000000000456:5@lid");
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        let recipient = info
            .recipient
            .as_ref()
            .expect("recipient must be populated from the node attr");
        assert_eq!(recipient.user, "5500000000123");
        assert!(recipient.is_pn(), "recipient namespace must be PN");
        assert_ne!(
            recipient.user, info.chat.user,
            "recipient must come from the node attr, not info.chat"
        );

        // Inverse: absent attr → None (drops `recipient` from the resend
        // stanza, mirroring WA Web's `f && (k.recipient = f)`).
        let node_no_recipient = NodeBuilder::new("receipt").build();
        let info_no_recipient =
            resolve_retry_chat_info(&receipt, &node_no_recipient.as_node_ref(), None, None);
        assert!(
            info_no_recipient.recipient.is_none(),
            "missing `recipient` attr must propagate as None"
        );
    }

    #[test]
    fn resolve_retry_chat_info_dm_bare() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt").build();
        let receipt = make_test_receipt("5511999999999@s.whatsapp.net");
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert_eq!(info.chat.device(), 0);
        assert_eq!(info.requester.device(), 0);
        assert_eq!(info.chat, info.requester);
    }

    #[test]
    fn resolve_retry_chat_info_group() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt")
            .attr("from", "120363021033254949@g.us")
            .attr("id", "MSG001")
            .attr("participant", "236395184570386:33@lid")
            .attr("type", "retry")
            .build();
        let receipt = Receipt {
            source: crate::types::message::MessageSource {
                chat: "120363021033254949@g.us".parse().unwrap(),
                sender: "236395184570386:33@lid".parse().unwrap(),
                ..Default::default()
            },
            message_ids: vec!["MSG001".to_string()],
            timestamp: wacore::time::now_utc(),
            r#type: crate::types::presence::ReceiptType::Retry,
            offline: false,
        };
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.chat.is_group());
        assert_eq!(info.chat.user, "120363021033254949");
        assert!(info.requester.is_lid());
        assert_eq!(info.requester.device(), 33);
    }

    #[test]
    fn resolve_retry_chat_info_group_bot_device_marks_bot_namespace_only() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt")
            .attr("participant", "somebot:4@bot")
            .build();
        let receipt = Receipt {
            source: crate::types::message::MessageSource {
                chat: "120363021033254949@g.us".parse().unwrap(),
                sender: "somebot:4@bot".parse().unwrap(),
                ..Default::default()
            },
            message_ids: vec!["MSG001".to_string()],
            timestamp: wacore::time::now_utc(),
            r#type: crate::types::presence::ReceiptType::Retry,
            offline: false,
        };

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.chat.is_group());
        assert!(info.is_bot);
        assert!(!info.is_fbid_bot_retry);
    }

    #[test]
    fn resolve_retry_chat_info_group_primary_fbid_bot_marks_bot_retry() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt")
            .attr("participant", "somebot@bot")
            .build();
        let receipt = Receipt {
            source: crate::types::message::MessageSource {
                chat: "120363021033254949@g.us".parse().unwrap(),
                sender: "somebot@bot".parse().unwrap(),
                ..Default::default()
            },
            message_ids: vec!["MSG001".to_string()],
            timestamp: wacore::time::now_utc(),
            r#type: crate::types::presence::ReceiptType::Retry,
            offline: false,
        };

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.chat.is_group());
        assert!(info.is_bot);
        assert!(info.is_fbid_bot_retry);
    }

    #[test]
    fn resolve_retry_chat_info_status_broadcast() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt")
            .attr("from", "status@broadcast")
            .attr("id", "3EB06D00CAB92340790621")
            .attr("participant", "236395184570386@lid")
            .attr("type", "retry")
            .build();
        let receipt = make_test_receipt("status@broadcast");
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.chat.is_status_broadcast());
        // requester should be the participant, not status@broadcast
        assert!(info.requester.is_lid());
        assert_eq!(info.requester.user, "236395184570386");
    }

    #[test]
    fn resolve_retry_chat_info_status_broadcast_no_participant() {
        use wacore_binary::builder::NodeBuilder;

        // Missing participant attr (edge case) — falls back to sender
        let node = NodeBuilder::new("receipt")
            .attr("from", "status@broadcast")
            .attr("id", "MSG001")
            .attr("type", "retry")
            .build();
        let receipt = make_test_receipt("status@broadcast");
        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.chat.is_status_broadcast());
        assert!(info.requester.is_status_broadcast());
    }

    // Different participants get different keys; same participant keeps the same
    // key across retry counts so pending_retries serializes concurrent receipts.
    #[test]
    fn retry_processing_key_per_participant() {
        let msg_id = "3EB06D00CAB92340790621";

        let status_chat = Jid::status_broadcast();
        let status_participant_a: Jid = "236395184570386@lid".parse().unwrap();
        let status_participant_b: Jid = "559985213786@s.whatsapp.net".parse().unwrap();
        let status_key_a = build_retry_processing_key(&status_chat, msg_id, &status_participant_a);
        let status_key_b = build_retry_processing_key(&status_chat, msg_id, &status_participant_b);
        assert_ne!(
            status_key_a, status_key_b,
            "Different status participants must have different processing keys"
        );
        assert_eq!(
            status_key_a,
            build_retry_processing_key(&status_chat, msg_id, &status_participant_a),
            "Same participant must produce the same key — any retry count for that \
             participant serializes through pending_retries"
        );

        let dm_chat = Jid::pn("559911112222");
        let dm_device_a = Jid::pn_device("559922223333", 1);
        let dm_device_b = Jid::pn_device("559922223333", 2);
        let dm_key_a = build_retry_processing_key(&dm_chat, msg_id, &dm_device_a);
        let dm_key_b = build_retry_processing_key(&dm_chat, msg_id, &dm_device_b);
        assert_ne!(
            dm_key_a, dm_key_b,
            "Different DM requester devices must have different processing keys"
        );
        assert_eq!(
            dm_key_a,
            build_retry_processing_key(&dm_chat, msg_id, &dm_device_a),
            "Same DM requester device must produce the same processing key"
        );
    }

    /// Test that the recent message cache supports re-addition after take.
    /// This is critical for multi-device retries where another device can
    /// ask for the same message after the first retry already consumed it.
    #[tokio::test]
    async fn recent_message_cache_readd_after_take() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        // Enable L1 cache so MockBackend (which doesn't persist) works for this test
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let msg = wa::Message {
            extended_text_message: buffa::MessageField::some(wa::message::ExtendedTextMessage {
                text: Some("status text".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        for (chat, msg_id) in [
            (Jid::status_broadcast(), "STATUS_MSG_001".to_string()),
            (Jid::pn("559911112222"), "DM_MSG_001".to_string()),
        ] {
            client.add_recent_message(&chat, &msg_id, &msg, None).await;

            let taken = client.take_recent_message(&chat, &msg_id).await;
            assert!(taken.is_some(), "First take should succeed for {chat}");

            let (taken_msg, _) = taken.unwrap();
            client
                .add_recent_message(&chat, &msg_id, &taken_msg, None)
                .await;

            let taken2 = client.take_recent_message(&chat, &msg_id).await;
            assert!(
                taken2.is_some(),
                "Second take should succeed after re-add for {chat}"
            );
            assert_eq!(
                taken2
                    .unwrap()
                    .0
                    .extended_text_message
                    .as_option()
                    .unwrap()
                    .text
                    .as_deref(),
                Some("status text")
            );
        }
    }

    /// Message stored under bare JID should be found when looking up via bare
    /// JID (the path resolve_retry_chat_info now provides for DMs).
    #[tokio::test]
    async fn dm_retry_message_lookup_uses_bare_jid() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let bare_jid: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let msg_id = "RETRY_MSG_001";
        let msg = wa::Message {
            conversation: Some("test dm".into()),
            ..Default::default()
        };

        // Store under bare JID (how send_message stores it)
        client
            .add_recent_message(&bare_jid, msg_id, &msg, None)
            .await;

        // Lookup via bare JID should succeed (this is what info.chat provides)
        let taken = client.take_recent_message(&bare_jid, msg_id).await;
        assert!(taken.is_some(), "Lookup via bare JID should succeed");
        let (msg_out, alt_chat) = taken.unwrap();
        assert!(alt_chat.is_none(), "primary key should match for bare JID");

        // Re-add under bare JID
        client
            .add_recent_message(&bare_jid, msg_id, &msg_out, None)
            .await;

        // Second take should also work
        let taken2 = client.take_recent_message(&bare_jid, msg_id).await;
        assert!(
            taken2.is_some(),
            "Second lookup via bare JID should succeed after re-add"
        );
    }

    /// Alternate PN/LID key lookup: a message stored under PN should be found
    /// when the primary lookup resolves to LID (because a mapping was added
    /// between send time and retry time).
    #[tokio::test]
    async fn alternate_key_lookup_pn_to_lid() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let pn_jid: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let lid_jid: Jid = "236395184570386@lid".parse().unwrap();
        let msg_id = "RETRY_ALT_001";
        let msg = wa::Message {
            conversation: Some("alternate key test".into()),
            ..Default::default()
        };

        // Store under PN (no LID mapping existed at send time)
        client.add_recent_message(&pn_jid, msg_id, &msg, None).await;

        // Now add a LID mapping (simulates mapping arriving between send and retry)
        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: lid_jid.user.as_str().into(),
                phone_number: pn_jid.user.as_str().into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;

        // Lookup via LID: primary key resolves to LID (miss),
        // alternate key falls back to PN (hit)
        let taken = client.take_recent_message(&lid_jid, msg_id).await;
        assert!(
            taken.is_some(),
            "Alternate PN key lookup should find message stored under PN"
        );
        let (msg_out, alt_chat) = taken.unwrap();
        let alt_chat = alt_chat.expect("should be found via alternate key");
        assert!(alt_chat.is_pn(), "alternate chat should be PN");
        assert_eq!(alt_chat.user, pn_jid.user);
        assert_eq!(msg_out.conversation.as_deref(), Some("alternate key test"));
    }

    /// swap_pn_lid_namespace should swap between PN and LID while preserving
    /// device/agent — this is the shared helper used for both alternate key
    /// computation and requester normalization after an alternate hit.
    #[tokio::test]
    async fn swap_pn_lid_namespace_preserves_device() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let (client, _sync_rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let pn_jid: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let lid_jid: Jid = "236395184570386@lid".parse().unwrap();

        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: lid_jid.user.as_str().into(),
                phone_number: pn_jid.user.as_str().into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;

        // LID:5 → PN:5
        let lid_with_device: Jid = "236395184570386:5@lid".parse().unwrap();
        let swapped = client.swap_pn_lid_namespace(&lid_with_device).await;
        let swapped = swapped.expect("should resolve LID→PN");
        assert!(swapped.is_pn());
        assert_eq!(swapped.user, "5511999999999");
        assert_eq!(swapped.device(), 5);

        // PN:3 → LID:3
        let pn_with_device: Jid = "5511999999999:3@s.whatsapp.net".parse().unwrap();
        let swapped = client.swap_pn_lid_namespace(&pn_with_device).await;
        let swapped = swapped.expect("should resolve PN→LID");
        assert!(swapped.is_lid());
        assert_eq!(swapped.user, "236395184570386");
        assert_eq!(swapped.device(), 3);

        // Group JID → None
        let group: Jid = "120363021033254949@g.us".parse().unwrap();
        assert!(client.swap_pn_lid_namespace(&group).await.is_none());
    }

    /// Alternate key lookup via PN input: message stored under PN, LID mapping
    /// added later, lookup via PN. Exercises the `server != server` optimization
    /// where `to` is used directly as alternate (no cache round-trip).
    #[tokio::test]
    async fn alternate_key_lookup_pn_input_server_changed() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let pn_jid: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let lid_jid: Jid = "236395184570386@lid".parse().unwrap();
        let msg_id = "RETRY_ALT_PN";
        let msg = wa::Message {
            conversation: Some("pn input alternate".into()),
            ..Default::default()
        };

        // Store under PN (no mapping at send time)
        client.add_recent_message(&pn_jid, msg_id, &msg, None).await;

        // Add LID mapping
        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: lid_jid.user.as_str().into(),
                phone_number: pn_jid.user.as_str().into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;

        // Lookup via PN: resolve_encryption_jid maps to LID (primary),
        // primary misses, server changed (Lid != Pn) → uses `to` directly
        let taken = client.take_recent_message(&pn_jid, msg_id).await;
        assert!(
            taken.is_some(),
            "Should find message via server-changed path"
        );
        let (msg_out, alt_chat) = taken.unwrap();
        let alt_chat = alt_chat.expect("should be alternate hit");
        assert!(
            alt_chat.is_pn(),
            "alternate chat should be PN (the original input)"
        );
        assert_eq!(alt_chat.user, pn_jid.user);
        assert_eq!(msg_out.conversation.as_deref(), Some("pn input alternate"));
    }

    /// When no PN/LID mapping exists, no alternate is tried and take returns None.
    #[tokio::test]
    async fn no_alternate_without_mapping() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let lid_jid: Jid = "236395184570386@lid".parse().unwrap();
        let msg_id = "RETRY_NO_ALT";
        let msg = wa::Message {
            conversation: Some("no alternate".into()),
            ..Default::default()
        };

        // Store under LID, no PN mapping exists
        client
            .add_recent_message(&lid_jid, msg_id, &msg, None)
            .await;

        // Lookup via LID: primary hits directly (same namespace)
        let taken = client.take_recent_message(&lid_jid, msg_id).await;
        assert!(taken.is_some());
        let (_, alt_chat) = taken.unwrap();
        assert!(alt_chat.is_none(), "primary hit should have no alt_chat");

        // Now try looking up a message that doesn't exist at all
        let missing = client.take_recent_message(&lid_jid, "NONEXISTENT").await;
        assert!(missing.is_none(), "non-existent message should return None");
    }

    /// When both primary and alternate miss, take returns None.
    #[tokio::test]
    async fn alternate_key_both_miss() {
        let _ = env_logger::builder().is_test(true).try_init();

        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager should initialize"),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 1_000;
        let (client, _sync_rx) = Client::new_with_cache_config(
            Arc::new(crate::runtime_impl::TokioRuntime),
            pm.clone(),
            Arc::new(crate::transport::mock::MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            config,
        )
        .await;

        let pn_jid: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let lid_jid: Jid = "236395184570386@lid".parse().unwrap();

        // Add mapping but don't store any message
        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: lid_jid.user.as_str().into(),
                phone_number: pn_jid.user.as_str().into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;

        // Lookup via PN: primary (LID) misses, alternate (PN) also misses
        let taken = client.take_recent_message(&pn_jid, "MISSING").await;
        assert!(taken.is_none(), "both primary and alternate miss → None");
    }

    // --- Peer device / bot / original_from tests ---

    #[test]
    fn resolve_retry_chat_info_peer_device_with_recipient() {
        use wacore_binary::builder::NodeBuilder;

        // Peer retry: from=our own JID, recipient=the actual chat partner
        let our_pn: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let recipient: Jid = "5522888888888@s.whatsapp.net".parse().unwrap();

        let node = NodeBuilder::new("receipt")
            .attr("recipient", "5522888888888@s.whatsapp.net")
            .build();
        let receipt = make_test_receipt("5511999999999:2@s.whatsapp.net");

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), Some(&our_pn), None);

        // Chat should be the recipient (the actual conversation partner)
        assert_eq!(info.chat.user, recipient.user);
        assert_eq!(info.chat.device(), 0, "chat should be bare");
        // Requester is still our device
        assert_eq!(info.requester.user, our_pn.user);
        assert_eq!(info.requester.device(), 2);
    }

    #[test]
    fn resolve_retry_chat_info_peer_device_without_recipient() {
        use wacore_binary::builder::NodeBuilder;

        // Peer retry without recipient attr has no target chat in WA Web.
        let our_pn: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let node = NodeBuilder::new("receipt").build();
        let receipt = make_test_receipt("5511999999999:2@s.whatsapp.net");

        let info =
            maybe_resolve_retry_chat_info(&receipt, &node.as_node_ref(), Some(&our_pn), None);

        assert!(info.is_none());
    }

    #[test]
    fn resolve_retry_chat_info_bot_with_recipient() {
        use wacore_binary::builder::NodeBuilder;

        // Bot retry: from=bot JID, recipient=actual chat
        let node = NodeBuilder::new("receipt")
            .attr("recipient", "5522888888888@s.whatsapp.net")
            .build();
        let receipt = make_test_receipt("131355500001@s.whatsapp.net");

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.is_bot, "bot JID should be detected");
        assert!(
            !info.is_fbid_bot_retry,
            "legacy PN bots use the regular retry parser"
        );
        // Chat should be the recipient
        assert_eq!(info.chat.user, "5522888888888");
        assert_eq!(info.chat.device(), 0);
    }

    #[test]
    fn resolve_retry_chat_info_bot_without_recipient() {
        use wacore_binary::builder::NodeBuilder;

        // Bot retry without recipient — falls through to normal DM path
        let node = NodeBuilder::new("receipt").build();
        let receipt = make_test_receipt("131355500001@s.whatsapp.net");

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.is_bot);
        assert!(!info.is_fbid_bot_retry);
        // Without recipient, falls to from.to_non_ad()
        assert_eq!(info.chat.user, "131355500001");
    }

    #[test]
    fn resolve_retry_chat_info_fbid_bot_dm_marks_bot_retry() {
        use wacore_binary::builder::NodeBuilder;

        let node = NodeBuilder::new("receipt")
            .attr("recipient", "5522888888888@s.whatsapp.net")
            .build();
        let receipt = make_test_receipt("200000000000002@bot");

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        assert!(info.is_bot);
        assert!(info.is_fbid_bot_retry);
        assert_eq!(info.chat.user, "5522888888888");
    }

    #[test]
    fn resolve_retry_chat_info_preserves_original_from() {
        use wacore_binary::builder::NodeBuilder;

        // DM with device suffix — original_from preserves the raw receipt from
        // (WA Web: variable m = e.from, used as-is for stanza to)
        let node = NodeBuilder::new("receipt").build();
        let receipt = make_test_receipt("5511999999999:33@s.whatsapp.net");

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, None);

        // original_from keeps the full JID including device
        assert_eq!(info.original_from.device(), 33);
        assert_eq!(info.original_from.user, "5511999999999");

        // chat is bare
        assert_eq!(info.chat.device(), 0);
        assert_eq!(info.chat.user, "5511999999999");
    }

    #[test]
    fn resolve_retry_chat_info_peer_via_lid() {
        use wacore_binary::builder::NodeBuilder;

        // Peer retry detected via LID (not PN)
        let our_lid: Jid = "236395184570386@lid".parse().unwrap();
        let recipient: Jid = "5522888888888@s.whatsapp.net".parse().unwrap();

        let node = NodeBuilder::new("receipt")
            .attr("recipient", "5522888888888@s.whatsapp.net")
            .build();
        let receipt = make_test_receipt("236395184570386:5@lid");

        let info = resolve_retry_chat_info(&receipt, &node.as_node_ref(), None, Some(&our_lid));

        assert_eq!(info.chat.user, recipient.user);
        assert_eq!(info.chat.device(), 0);
        assert_eq!(info.requester.device(), 5);
    }
}
