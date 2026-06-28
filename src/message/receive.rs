//! Core incoming-message pipeline: classify, decrypt and process.

use super::*;

impl Client {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.incoming", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_incoming_message(self: Arc<Self>, node: Arc<OwnedNodeRef>) {
        // Phase 1: classify borrows the node tree, extracts owned payloads, returns quickly.
        // Phase 2: process_classified_message holds no node borrows across heavy .await points,
        // keeping the async state machine small.
        let classified = match self.classify_incoming_message(&node).await {
            Some(c) => c,
            None => return,
        };
        // node is no longer borrowed here -- drop it before the heavy phase
        drop(node);
        self.process_classified_message(classified).await;
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.classify", level = "debug", skip_all)
    )]
    pub(crate) async fn classify_incoming_message(
        self: &Arc<Self>,
        node: &OwnedNodeRef,
    ) -> Option<ClassifiedMessage> {
        let nr = node.get();
        let info = match self.parse_message_info(nr).await {
            Ok(info) => Arc::new(info),
            Err(e) => {
                let id = nr.get_attr("id").map(|v| v.as_str());
                let from = nr.get_attr("from").map(|v| v.as_str());
                log::warn!("Failed to parse message info (id={id:?}, from={from:?}): {e:?}");
                return None;
            }
        };

        // Newsletters use <plaintext> instead of <enc> because they are not E2E encrypted.
        if info.source.chat.is_newsletter() {
            self.handle_newsletter_message(nr, &info).await;
            return None;
        }

        self.cache_lid_pn_from_message(
            &info.source.sender,
            info.source.sender_alt.as_ref(),
            info.is_offline,
        )
        .await;
        let sender_encryption_jid = self.resolve_encryption_jid(&info.source.sender).await;

        let unavailable_node = nr.get_optional_child("unavailable");

        let mut all_enc_nodes: Vec<&NodeRef<'_>> = Vec::with_capacity(4);

        let direct_enc_nodes = nr.get_children_by_tag("enc");
        all_enc_nodes.extend(direct_enc_nodes);

        let participants = nr.get_optional_child_by_tag(&["participants"]);
        if let Some(participants_node) = participants {
            let own_jid = self.get_pn();
            let to_nodes = participants_node.get_children_by_tag("to");
            for to_node in to_nodes {
                let to_jid = match to_node.attrs().optional_jid("jid") {
                    Some(jid) => jid,
                    None => continue,
                };
                if own_jid.as_ref().is_some_and(|ours| *ours == to_jid) {
                    let enc_children = to_node.get_children_by_tag("enc");
                    all_enc_nodes.extend(enc_children);
                }
            }
        }

        if all_enc_nodes.is_empty() && unavailable_node.is_none() {
            log::warn!(
                "[msg:{}] Received non-newsletter message without <enc> child: {}",
                info.id,
                nr.tag
            );
            return None;
        }

        if let Some(unavailable) = unavailable_node
            && all_enc_nodes.is_empty()
        {
            let unavailable_type = match unavailable.get_attr("type").map(|v| v.as_str()).as_deref()
            {
                Some("view_once") => crate::types::events::UnavailableType::ViewOnce,
                _ => crate::types::events::UnavailableType::Unknown,
            };
            log::info!(
                "[msg:{}] Message has <unavailable> child (type: {:?}), requesting from phone via PDO",
                info.id,
                unavailable_type
            );
            // PDO is the only recovery here (no retry receipt), so run it before
            // the transport ack in one flush task: the ack must not clear the
            // offline queue before the PDO request goes out. status is acked by
            // the should_ack gate. Mirrors whatsmeow's request-then-ack.
            self.dispatch_undecryptable_event(
                Arc::clone(&info),
                true,
                unavailable_type,
                crate::types::events::DecryptFailMode::Show,
            )
            .await;
            let client = Arc::clone(self);
            let info2 = Arc::clone(&info);
            let skip_ack = info.source.chat.is_status_broadcast();
            self.outbound_flush.spawn(&*self.runtime, async move {
                // Only ack once the PDO request is out (or skipped as ancient);
                // a transient send failure leaves it queued for redelivery.
                let pdo_sent = client.run_pdo_request(&info2).await;
                if !skip_ack && pdo_sent {
                    client.send_transport_ack(&info2).await;
                }
            });
            return None;
        }

        let mut session_payloads = Vec::with_capacity(all_enc_nodes.len());
        let mut group_payloads = Vec::with_capacity(all_enc_nodes.len());
        let mut bot_payloads = Vec::with_capacity(all_enc_nodes.len());
        let mut max_sender_retry_count: u8 = 0;
        let mut has_hide_fail = false;
        let mut had_unknown_enc = false;
        let mut had_custom_handler = false;

        // Custom enc handlers are set once at Bot::build and immutable after, so
        // read the map lock-free once instead of acquiring an async RwLock guard
        // per enc node. `None` (the common zero-handler bot) skips the lookup.
        let custom_enc_handlers = self.custom_enc_handlers.get();

        for enc_node in &all_enc_nodes {
            // Parse sender retry count (WA Web: e.maybeAttrInt("count") ?? 0)
            // Clamp to MAX_DECRYPT_RETRIES to prevent u64→u8 truncation on unexpected values.
            let sender_count = enc_node
                .attrs()
                .optional_u64("count")
                .map(|c| c.min(MAX_DECRYPT_RETRIES as u64) as u8)
                .unwrap_or(0);
            max_sender_retry_count = max_sender_retry_count.max(sender_count);

            // Parse decrypt-fail attribute (WA Web: e.maybeAttrString("decrypt-fail") === "hide")
            if enc_node
                .get_attr("decrypt-fail")
                .map(|v| v.as_str())
                .is_some_and(|s| s == "hide")
            {
                has_hide_fail = true;
            }

            let enc_type = match enc_node.attrs().optional_string("type") {
                Some(t) => t,
                None => {
                    log::warn!("Enc node missing 'type' attribute, skipping");
                    had_unknown_enc = true;
                    continue;
                }
            };

            if let Some(handler) =
                custom_enc_handlers.and_then(|m| m.get(enc_type.as_ref()).cloned())
            {
                let handler_clone = handler;
                let client_clone = self.clone();
                let info_arc = Arc::clone(&info);
                // Custom enc handlers take &Node (public API); convert from NodeRef here.
                let enc_node_owned = (*enc_node).to_owned();
                let enc_type_owned = enc_type.to_string();

                self.runtime
                    .spawn(Box::pin(async move {
                        if let Err(e) = handler_clone
                            .handle(client_clone, &enc_node_owned, &info_arc)
                            .await
                        {
                            log::warn!(
                                "Custom handler for enc type '{}' failed: {e:?}",
                                enc_type_owned
                            );
                        }
                    }))
                    .detach();
                had_custom_handler = true;
                continue;
            }

            // `had_unknown_enc` means "produced no usable payload": either the
            // type is unrecognized or it's known but the body is empty.
            // Either way the stanza needs the fallback ack or the server replays.
            if EncType::from_wire(enc_type.as_ref()).is_none() {
                log::warn!("Enc node has unknown type: {enc_type}");
                had_unknown_enc = true;
                continue;
            }

            let payload = match EncPayload::from_owned_node(node, enc_node) {
                Some(p) => p,
                None => {
                    log::warn!("Enc node {enc_type} has no content");
                    had_unknown_enc = true;
                    continue;
                }
            };

            if payload.enc_type.is_bot_secret() {
                bot_payloads.push(payload);
            } else if payload.enc_type.is_session() {
                session_payloads.push(payload);
            } else {
                group_payloads.push(payload);
            }
        }

        // WA Web diagnostic: validate skmsg is not first in multi-enc messages.
        if !session_payloads.is_empty()
            && !group_payloads.is_empty()
            && all_enc_nodes.first().is_some_and(|n| {
                n.get_attr("type")
                    .map(|v| v.as_str())
                    .is_some_and(|s| s == EncType::SenderKey.as_wire_str())
            })
        {
            log::error!(
                "[msg:{}] Protocol violation: skmsg is first in multi-enc message from {}. \
                 Expected pkmsg/msg first (containing SKDM).",
                info.id,
                info.source.sender.observe()
            );
        }

        // Unknown-only stanzas would loop in the offline queue until
        // <stream:error>. Custom handlers ack on their own; status is covered
        // by should_ack. Ack from `nr` so `recipient` survives. Skip when any
        // bucket has usable payloads (including msmsg) so the regular dispatch
        // path runs and the valid enc still decrypts.
        if session_payloads.is_empty()
            && group_payloads.is_empty()
            && bot_payloads.is_empty()
            && had_unknown_enc
            && !had_custom_handler
        {
            log::info!(
                "[msg:{}] All enc payloads unrecognized; transport-acking to drop from offline queue",
                info.id
            );
            if !info.source.chat.is_status_broadcast() {
                self.spawn_node_transport_ack(nr).await;
            }
            return None;
        }

        Some(ClassifiedMessage {
            info,
            sender_encryption_jid,
            session_payloads,
            group_payloads,
            bot_payloads,
            max_sender_retry_count,
            decrypt_fail_mode: if has_hide_fail {
                crate::types::events::DecryptFailMode::Hide
            } else {
                crate::types::events::DecryptFailMode::Show
            },
        })
    }

    /// Phase 2: acquire permit, decrypt payloads, flush. No node borrows.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.process", level = "debug", skip_all)
    )]
    pub(crate) async fn process_classified_message(self: Arc<Self>, msg: ClassifiedMessage) {
        let ClassifiedMessage {
            info,
            sender_encryption_jid,
            session_payloads,
            group_payloads,
            bot_payloads,
            max_sender_retry_count,
            decrypt_fail_mode,
        } = msg;

        if max_sender_retry_count > 0 {
            let cache_key = self
                .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
                .await;
            let existing = self.message_retry_counts.get(&cache_key).await;
            if max_sender_retry_count > existing.map_or(0, |(count, _)| count) {
                // Keep any locally recorded reason; the echoed count carries none.
                let reason = existing.and_then(|(_, reason)| reason);
                self.message_retry_counts
                    .insert(cache_key, (max_sender_retry_count, reason))
                    .await;
            }
            log::debug!(
                "[msg:{}] Sender retry count {} pre-seeded into cache",
                info.id,
                max_sender_retry_count
            );
        }

        // Acquire global processing permit (1 during offline sync, N after).
        // Read generation + clone Arc under the same mutex so the pair is consistent.
        //
        // When the semaphore transitions from 1→N (offline→online), tasks waiting on
        // the old 1-permit semaphore must re-acquire from the new N-permit semaphore.
        // Without this re-acquire loop, those tasks would be silently dropped, which
        // can lose pkmsg messages carrying SKDM (sender key distribution). If the
        // SKDM is lost, ALL subsequent skmsg messages from that sender will fail
        // with "No sender key state".
        let _global_permit = loop {
            let (generation, semaphore) = self.read_message_semaphore();
            let permit = semaphore.acquire_arc().await;
            if generation
                == self
                    .message_semaphore_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                break permit;
            }
            // Generation changed while waiting (e.g. offline→online transition).
            // Drop the stale permit and retry with the new semaphore, which has
            // more permits and will grant access quickly.
            log::debug!(
                "Semaphore generation changed during acquire, re-acquiring from new semaphore"
            );
            drop(permit);
        };

        log::debug!(
            "Starting PASS 1: Processing {} session establishment messages (pkmsg/msg)",
            session_payloads.len()
        );

        // Skip session processing for group/broadcast JIDs — they use sender keys, not 1:1 sessions.
        let is_group_sender = sender_encryption_jid.is_group()
            || sender_encryption_jid.is_broadcast_list()
            || sender_encryption_jid.is_status_broadcast();

        let session_outcome = if !is_group_sender && !session_payloads.is_empty() {
            self.clone()
                .process_session_enc_batch(
                    &session_payloads,
                    &info,
                    &sender_encryption_jid,
                    decrypt_fail_mode,
                )
                .await
        } else {
            if is_group_sender && !session_payloads.is_empty() {
                log::debug!(
                    "Skipping {} session messages from group sender {}",
                    session_payloads.len(),
                    sender_encryption_jid.observe()
                );
            }
            SessionBatchOutcome::default()
        };
        let session_decrypted_successfully = session_outcome.decrypted;
        let session_had_duplicates = session_outcome.duplicate;
        let session_dispatched_undecryptable = session_outcome.undecryptable;

        log::debug!(
            "Starting PASS 2: Processing {} group content messages (skmsg)",
            group_payloads.len()
        );

        // Only process group content if:
        // 1. There were no session messages (session already exists), OR
        // 2. Session messages were successfully decrypted, OR
        // 3. Session messages were duplicates (already processed, so session exists)
        // Skip only if session messages FAILED to decrypt (not duplicates, not absent).
        // Matches WA Web's `canDecryptNext` pattern: if pkmsg fails with a retriable error,
        // the SKDM it carried is lost, so skmsg will always fail with NoSenderKey — skip it
        // to avoid unnecessary retry receipts. The retry for the pkmsg will cause the sender
        // to resend the entire message including SKDM.
        if !group_payloads.is_empty() {
            let should_process_skmsg =
                should_process_skmsg_after_session(session_payloads.is_empty(), session_outcome);

            if should_process_skmsg {
                match self
                    .clone()
                    .process_group_enc_batch(
                        &group_payloads,
                        &info,
                        &sender_encryption_jid,
                        decrypt_fail_mode,
                    )
                    .await
                {
                    Ok(()) => {
                        // Processed successfully or handled errors (e.g. sent retry receipt)
                    }
                    Err(e) => {
                        log::warn!(
                            "[msg:{}] Batch group decrypt from {} in {} failed: {e:?}",
                            info.id,
                            info.source.sender.observe(),
                            info.source.chat.observe()
                        );
                    }
                }
            } else {
                // Only show warning if session messages actually FAILED (not duplicates)
                if !session_had_duplicates {
                    if info.is_expired_status() {
                        log::debug!(
                            "[msg:{}] Silently dropping expired status from {}",
                            info.id,
                            info.source.sender.observe()
                        );
                    } else {
                        // WA Web skips the skmsg silently after a retryable
                        // pkmsg failure (canDecryptNext in
                        // WAWebMsgProcessingDecryptionHandler); the pkmsg
                        // failure itself is already logged and retried.
                        log::debug!(
                            "Skipping skmsg decryption for message {} from {} because pkmsg failed to decrypt.",
                            info.id,
                            info.source.sender.observe()
                        );
                        if !session_dispatched_undecryptable {
                            self.dispatch_undecryptable_event(
                                Arc::clone(&info),
                                false,
                                crate::types::events::UnavailableType::Unknown,
                                decrypt_fail_mode,
                            )
                            .await;
                        }
                    }

                    // Do NOT send a delivery receipt for undecryptable messages.
                    // Per whatsmeow's implementation, delivery receipts are only sent for
                    // successfully decrypted/handled messages. Sending a receipt here would
                    // tell the server we processed it, incrementing the offline counter.
                    // The transport <ack> is sufficient for acknowledgment.
                }
                // If session_had_duplicates is true, we silently skip (no warning, no event)
                // because the message was already processed in a previous session
            }
        } else if !session_decrypted_successfully
            && !session_had_duplicates
            && !session_payloads.is_empty()
        {
            // Edge case: message with only msg/pkmsg that failed to decrypt, no skmsg
            log::log!(
                decrypt_fail_log_level(decrypt_fail_mode),
                "Message {} from {} failed to decrypt and has no group content. Dispatching UndecryptableMessage event.",
                info.id,
                info.source.sender.observe()
            );
            // Dispatch UndecryptableMessage event for messages that failed to decrypt
            // (This should not cause double-dispatching since process_session_enc_batch
            // already returned dispatched_undecryptable=false for this case)
            if !session_dispatched_undecryptable {
                self.dispatch_undecryptable_event(
                    Arc::clone(&info),
                    false,
                    crate::types::events::UnavailableType::Unknown,
                    decrypt_fail_mode,
                )
                .await;
            }
            // Do NOT send delivery receipt - transport ack is sufficient
        } else if session_had_duplicates
            && !session_decrypted_successfully
            && !session_dispatched_undecryptable
            && !info.source.chat.is_status_broadcast()
        {
            // Duplicate (already-processed) with no group content: ack it so the
            // server drops it from the offline queue (whatsmeow/WA Web treat
            // old-counter like success). status is acked by the should_ack gate
            // (a status SKDM pkmsg can reach here), so skip it to avoid a
            // redundant receipt. With a durability hook, a buffered copy here
            // means the original commit never acked, so replay it instead.
            self.ack_or_replay_to_hook(&info).await;
        } else if should_ack_skdm_only_session_fallback(session_outcome, bot_payloads.is_empty()) {
            // SKDM-only session decrypts skip dispatch, so this stanza would
            // otherwise stay queued. WA Web and whatsmeow ack every decrypted
            // message; the ack shape still comes from the message source.
            // Status is intentionally not filtered here, so its success receipt
            // still follows the normal WA Web path.
            self.ack_received_message(&info);
        }

        // Bot-secret (msmsg) payloads run inline here so they're serialised
        // with the session/group decrypt batches under the same global
        // permit + per-chat enqueue lock acquired upstream.
        for payload in bot_payloads {
            self.handle_msmsg_payload(&info, payload).await;
        }

        // Flush cached Signal state to DB (matches WA Web's flushBufferToDiskIfNotMemOnlyMode)
        self.flush_signal_cache_logged("message", Some(&info.id))
            .await;
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.session_decrypt", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %sender_encryption_jid.observe(), msg_id = %info.id)))]
    pub(crate) async fn process_session_enc_batch(
        self: Arc<Self>,
        payloads: &[EncPayload],
        info: &Arc<MessageInfo>,
        sender_encryption_jid: &Jid,
        decrypt_fail_mode: crate::types::events::DecryptFailMode,
    ) -> SessionBatchOutcome {
        use wacore::libsignal::protocol::CiphertextMessage;
        if payloads.is_empty() {
            return SessionBatchOutcome::default();
        }

        // Acquire a per-sender session lock to prevent race conditions when
        // multiple messages from the same sender are processed concurrently.
        // Use the full Signal protocol address string as the lock key so it matches
        // the SignalProtocolStoreAdapter's per-session locks (prevents ratchet counter races).
        let signal_address = sender_encryption_jid.to_protocol_address();

        // `session_guard` is held across the entire batch but dropped around
        // calls into `try_pn_to_lid_migration_decrypt` because that function's
        // migration loop re-enters this same mutex (non-reentrant).
        let session_mutex = self.session_lock_for(signal_address.as_str()).await;
        let mut session_guard: Option<async_lock::MutexGuardArc<()>> =
            Some(session_mutex.lock_arc().await);

        // Started after the lock so the histogram is crypto-only, not lock/queue wait.
        let _t = wacore::telemetry::timer(wacore::telemetry::DECRYPT_DURATION);

        let mut adapter = self.signal_adapter().await;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let mut outcome = SessionBatchOutcome::default();
        // Local identity-change detection fires once per batch: the first pkmsg
        // saves the new key (ReplacedExisting); the rest are NewOrUnchanged.
        let mut local_identity_reacted = false;

        for payload in payloads {
            let ciphertext = &payload.ciphertext[..];
            let enc_type = payload.enc_type;
            let enc_type_str = enc_type.as_wire_str();
            let padding_version = payload.padding_version;

            // WA Web `MsgSendReceipt.js` nacks PARSE_ERROR; without it the
            // server retransmits the malformed stanza forever. Mirrors the
            // `handle_decrypt_failure` shape (dispatch event + spawn wire I/O
            // so the session lock isn't held across the send).
            let parsed_message = if enc_type == EncType::PreKeyMessage {
                match PreKeySignalMessage::try_from(ciphertext) {
                    Ok(m) => CiphertextMessage::PreKeySignalMessage(m),
                    Err(e) => {
                        log::error!(
                            "[msg:{}] Failed to parse PreKeySignalMessage from {}: {e:?}. Sending nack.",
                            info.id,
                            info.source.sender.observe()
                        );
                        // |= so a later dedup'd return (false) can't clobber
                        // a true set by a prior iteration in this batch.
                        outcome.had_failure = true;
                        outcome.undecryptable |= self
                            .dispatch_undecryptable_event(
                                Arc::clone(info),
                                false,
                                crate::types::events::UnavailableType::Unknown,
                                decrypt_fail_mode,
                            )
                            .await;
                        self.spawn_nack(info, NackReason::ParsingError, None);
                        continue;
                    }
                }
            } else {
                match SignalMessage::try_from(ciphertext) {
                    Ok(m) => CiphertextMessage::SignalMessage(m),
                    Err(e) => {
                        log::error!(
                            "[msg:{}] Failed to parse SignalMessage from {}: {e:?}. Sending nack.",
                            info.id,
                            info.source.sender.observe()
                        );
                        outcome.had_failure = true;
                        outcome.undecryptable |= self
                            .dispatch_undecryptable_event(
                                Arc::clone(info),
                                false,
                                crate::types::events::UnavailableType::Unknown,
                                decrypt_fail_mode,
                            )
                            .await;
                        self.spawn_nack(info, NackReason::ParsingError, None);
                        continue;
                    }
                }
            };

            if enc_type == EncType::PreKeyMessage {
                // FLAGGED FOR DEBUGGING: "Bad Mac" Reproducibility
                #[cfg(feature = "debug-snapshots")]
                {
                    use base64::prelude::*;
                    let payload = serde_json::json!({
                        "id": info.id,
                        "sender_jid": sender_encryption_jid.to_string(),
                        "timestamp": info.timestamp,
                        "enc_type": enc_type_str,
                        "payload_base64": BASE64_STANDARD.encode(ciphertext),
                    });

                    let content_bytes = serde_json::to_vec_pretty(&payload).unwrap_or_default();

                    if let Err(e) = self
                        .persistence_manager
                        .create_snapshot(&format!("pre_pkmsg_{}", info.id), Some(&content_bytes))
                        .await
                    {
                        log::warn!("Failed to create snapshot for pkmsg: {}", e);
                    }
                }
                #[cfg(not(feature = "debug-snapshots"))]
                {
                    // No-op if disabled
                }
            }

            // Shadow with wire string for all downstream usage (logging, handlers)
            let enc_type = enc_type_str;

            let decrypt_res = message_decrypt(
                &parsed_message,
                &signal_address,
                &mut adapter.session_store,
                &mut adapter.identity_store,
                &mut adapter.pre_key_store,
                &adapter.signed_pre_key_store,
                &mut rng,
                UsePQRatchet::No,
            )
            .await;

            match decrypt_res {
                Ok(decrypted) => {
                    // Buffer the prekey this pkmsg consumed: message_decrypt promoted
                    // the session into the (volatile) cache but no longer deletes the
                    // prekey itself. The post-loop flush deletes it only once that
                    // session is durable, keeping a crash from orphaning the prekey.
                    if let Some(prekey_id) = decrypted.consumed_prekey_id {
                        adapter
                            .pre_key_store
                            .buffer_consumed_prekey(prekey_id, &signal_address)
                            .await;
                    }
                    if decrypted.identity_change == IdentityChange::ReplacedExisting
                        && !local_identity_reacted
                    {
                        local_identity_reacted = true;
                        self.react_to_local_identity_change(sender_encryption_jid);
                    }
                    let padded_plaintext = decrypted.plaintext;
                    match self
                        .clone()
                        .handle_decrypted_plaintext(
                            enc_type,
                            &padded_plaintext,
                            padding_version,
                            info,
                        )
                        .await
                    {
                        Ok(plaintext_outcome) => {
                            outcome.decrypted = true;
                            outcome.dispatched |= plaintext_outcome.dispatched;
                            outcome.skdm_only |= plaintext_outcome.skdm_only;
                        }
                        Err(e) => {
                            log::warn!(
                                "[msg:{}] Failed processing plaintext from {}: {e:?}",
                                info.id,
                                info.source.sender.observe()
                            );
                            outcome.decrypted = true;
                            outcome.plaintext_failed = true;
                            outcome.had_failure = true;
                            outcome.undecryptable |=
                                self.handle_plaintext_failure(info, decrypt_fail_mode).await;
                        }
                    }
                }
                Err(e) => {
                    // Handle DuplicatedMessage: This is expected when messages are redelivered during reconnection
                    if let SignalProtocolError::DuplicatedMessage(chain, counter) = e {
                        log::debug!(
                            "Skipping already-processed message from {} (chain {}, counter {}). This is normal during reconnection.",
                            info.source.sender.observe(),
                            chain,
                            counter
                        );
                        // Mark that we saw a duplicate so we can skip skmsg without showing error
                        outcome.duplicate = true;
                        continue;
                    }
                    // Handle UntrustedIdentity: This happens when a user re-installs WhatsApp or changes devices.
                    // The Signal Protocol's security policy rejects messages from new identity keys by default.
                    // We handle this by clearing the old identity (to trust the new one), then retrying decryption.
                    // IMPORTANT: We do NOT delete the session! When the PreKeySignalMessage is processed,
                    // libsignal's `promote_state` will archive the old session as a "previous state".
                    // This allows us to decrypt any in-flight messages that were encrypted with the old session.
                    if let SignalProtocolError::UntrustedIdentity(ref address) = e {
                        log::warn!(
                            "[msg:{}] Received message from untrusted identity: {}. This typically means the sender re-installed WhatsApp or changed their device. Clearing old identity to trust new key (keeping session for in-flight messages).",
                            info.id,
                            address
                        );

                        // Delete the old, untrusted identity through the signal cache.
                        // NOTE: We intentionally do NOT delete the session here. The session will be
                        // archived (not deleted) when the new PreKeySignalMessage is processed,
                        // allowing decryption of any in-flight messages encrypted with the old session.
                        self.signal_cache.delete_identity(address).await;
                        // Flush immediately so the backend is updated BEFORE the retry decrypt below.
                        // Device::is_trusted_identity reads from backend, not cache.
                        if let Err(e) = self.flush_signal_cache().await {
                            log::warn!(
                                "Failed to flush identity deletion for {}: {e:?}",
                                wacore::types::jid::observe_protocol_address(address)
                            );
                            outcome.had_failure = true;
                            continue;
                        }
                        log::info!(
                            "Cleared old identity for {} from cache and backend",
                            address
                        );

                        // Re-attempt decryption with the new identity
                        log::info!(
                            "[msg:{}] Retrying message decryption for {} after clearing untrusted identity",
                            info.id,
                            address
                        );

                        let retry_decrypt_res = message_decrypt(
                            &parsed_message,
                            &signal_address,
                            &mut adapter.session_store,
                            &mut adapter.identity_store,
                            &mut adapter.pre_key_store,
                            &adapter.signed_pre_key_store,
                            &mut rng,
                            UsePQRatchet::No,
                        )
                        .await;

                        match retry_decrypt_res {
                            Ok(decrypted) => {
                                log::debug!(
                                    "[msg:{}] Successfully decrypted message from {} after handling untrusted identity",
                                    info.id,
                                    address
                                );
                                if let Some(prekey_id) = decrypted.consumed_prekey_id {
                                    adapter
                                        .pre_key_store
                                        .buffer_consumed_prekey(prekey_id, &signal_address)
                                        .await;
                                }
                                // Normally NewOrUnchanged here (the untrusted
                                // identity was deleted+flushed before the retry),
                                // but mirror the main-decode gate so a concurrent
                                // re-save can't drop the signal.
                                if decrypted.identity_change == IdentityChange::ReplacedExisting
                                    && !local_identity_reacted
                                {
                                    local_identity_reacted = true;
                                    self.react_to_local_identity_change(sender_encryption_jid);
                                }
                                let padded_plaintext = decrypted.plaintext;
                                match self
                                    .clone()
                                    .handle_decrypted_plaintext(
                                        enc_type,
                                        &padded_plaintext,
                                        padding_version,
                                        info,
                                    )
                                    .await
                                {
                                    Ok(plaintext_outcome) => {
                                        outcome.decrypted = true;
                                        outcome.dispatched |= plaintext_outcome.dispatched;
                                        outcome.skdm_only |= plaintext_outcome.skdm_only;
                                    }
                                    Err(e) => {
                                        log::warn!(
                                            "Failed processing plaintext after identity retry: {e:?}"
                                        );
                                        outcome.decrypted = true;
                                        outcome.plaintext_failed = true;
                                        outcome.had_failure = true;
                                        outcome.undecryptable |= self
                                            .handle_plaintext_failure(info, decrypt_fail_mode)
                                            .await;
                                    }
                                }
                            }
                            Err(retry_err) => {
                                // Handle DuplicatedMessage in retry path: This commonly happens during reconnection
                                // when the same message is redelivered by the server after we already processed it.
                                // The first attempt triggered UntrustedIdentity, we cleared the session, but meanwhile
                                // another message from the same sender re-established the session and consumed the counter.
                                // This is benign - the message was already successfully processed.
                                if let SignalProtocolError::DuplicatedMessage(chain, counter) =
                                    retry_err
                                {
                                    log::debug!(
                                        "Message from {} was already processed (chain {}, counter {}) - detected during untrusted identity retry. This is normal during reconnection.",
                                        address,
                                        chain,
                                        counter
                                    );
                                    outcome.duplicate = true;
                                } else if matches!(retry_err, SignalProtocolError::InvalidPreKeyId)
                                {
                                    // Session may exist under PN address after identity change
                                    let migration_outcome = self
                                        .try_pn_to_lid_migration_decrypt(
                                            sender_encryption_jid,
                                            &signal_address,
                                            &parsed_message,
                                            &mut adapter,
                                            &mut rng,
                                            enc_type,
                                            padding_version,
                                            info,
                                            &session_mutex,
                                            &mut session_guard,
                                        )
                                        .await;
                                    if migration_outcome.decrypted
                                        || migration_outcome.duplicate
                                        || migration_outcome.plaintext_failed
                                    {
                                        outcome.decrypted |= migration_outcome.decrypted;
                                        outcome.duplicate |= migration_outcome.duplicate;
                                        outcome.dispatched |= migration_outcome.dispatched;
                                        outcome.skdm_only |= migration_outcome.skdm_only;
                                        outcome.plaintext_failed |=
                                            migration_outcome.plaintext_failed;
                                        outcome.had_failure |= migration_outcome.plaintext_failed;
                                        if migration_outcome.plaintext_failed {
                                            outcome.undecryptable |= self
                                                .handle_plaintext_failure(info, decrypt_fail_mode)
                                                .await;
                                        }
                                    } else {
                                        log::debug!(
                                            "[msg:{}] InvalidPreKeyId after identity change for {}. \
                                             Sending retry receipt with fresh keys.",
                                            info.id,
                                            address
                                        );
                                        outcome.had_failure = true;
                                        outcome.undecryptable |= self
                                            .handle_decrypt_failure(
                                                info,
                                                RetryReason::InvalidKeyId,
                                                decrypt_fail_mode,
                                            )
                                            .await;
                                    }
                                } else {
                                    log::error!(
                                        "[msg:{}] Decryption failed even after clearing untrusted identity for {}: {:?}",
                                        info.id,
                                        address,
                                        retry_err
                                    );
                                    // Send retry receipt so the sender resends with a PreKeySignalMessage
                                    // to establish a new session with the new identity
                                    outcome.had_failure = true;
                                    outcome.undecryptable |= self
                                        .handle_decrypt_failure(
                                            info,
                                            RetryReason::InvalidKey,
                                            decrypt_fail_mode,
                                        )
                                        .await;
                                }
                            }
                        }

                        // Re-issue tctoken so the contact still has a valid token for us
                        let sender_jid = info.source.sender.clone();
                        if !sender_jid.is_bot() && !sender_jid.is_status_broadcast() {
                            let client = self.clone();
                            self.runtime
                                .spawn(Box::pin(async move {
                                    client
                                        .reissue_tc_token_after_identity_change(&sender_jid)
                                        .await;
                                }))
                                .detach();
                        }

                        continue;
                    }
                    // Try PN→LID session migration before sending retry receipt
                    if let SignalProtocolError::SessionNotFound(_) = e {
                        let migration_outcome = self
                            .try_pn_to_lid_migration_decrypt(
                                sender_encryption_jid,
                                &signal_address,
                                &parsed_message,
                                &mut adapter,
                                &mut rng,
                                enc_type,
                                padding_version,
                                info,
                                &session_mutex,
                                &mut session_guard,
                            )
                            .await;
                        if migration_outcome.decrypted
                            || migration_outcome.duplicate
                            || migration_outcome.plaintext_failed
                        {
                            outcome.decrypted |= migration_outcome.decrypted;
                            outcome.duplicate |= migration_outcome.duplicate;
                            outcome.dispatched |= migration_outcome.dispatched;
                            outcome.skdm_only |= migration_outcome.skdm_only;
                            outcome.plaintext_failed |= migration_outcome.plaintext_failed;
                            outcome.had_failure |= migration_outcome.plaintext_failed;
                            if migration_outcome.plaintext_failed {
                                outcome.undecryptable |=
                                    self.handle_plaintext_failure(info, decrypt_fail_mode).await;
                            }
                            continue;
                        }

                        debug!(
                            "[msg:{}] No session found for {} message from {}. Sending retry receipt to request session establishment.",
                            info.id,
                            enc_type,
                            info.source.sender.observe()
                        );
                        outcome.had_failure = true;
                        outcome.undecryptable |= self
                            .handle_decrypt_failure(info, RetryReason::NoSession, decrypt_fail_mode)
                            .await;
                        continue;
                    } else if matches!(
                        e,
                        SignalProtocolError::BadMac(_) | SignalProtocolError::InvalidMessage(_, _)
                    ) {
                        // whatsmeow migrates PN sessions before decrypt; a fresh
                        // LID record can otherwise shadow the sender's PN ratchet.
                        let migration_outcome = self
                            .try_pn_to_lid_migration_decrypt(
                                sender_encryption_jid,
                                &signal_address,
                                &parsed_message,
                                &mut adapter,
                                &mut rng,
                                enc_type,
                                padding_version,
                                info,
                                &session_mutex,
                                &mut session_guard,
                            )
                            .await;
                        if migration_outcome.decrypted
                            || migration_outcome.duplicate
                            || migration_outcome.plaintext_failed
                        {
                            outcome.decrypted |= migration_outcome.decrypted;
                            outcome.duplicate |= migration_outcome.duplicate;
                            outcome.dispatched |= migration_outcome.dispatched;
                            outcome.skdm_only |= migration_outcome.skdm_only;
                            outcome.plaintext_failed |= migration_outcome.plaintext_failed;
                            outcome.had_failure |= migration_outcome.plaintext_failed;
                            if migration_outcome.plaintext_failed {
                                outcome.undecryptable |=
                                    self.handle_plaintext_failure(info, decrypt_fail_mode).await;
                            }
                            continue;
                        }

                        // WAWebMsgProcessingDecryptionHandler classifies both as
                        // SignalRetryable -> sendRetryReceipt only, with no delete.
                        let (reason, label) = if matches!(e, SignalProtocolError::BadMac(_)) {
                            (RetryReason::BadMac, "BadMac")
                        } else {
                            (RetryReason::InvalidMessage, "InvalidMessage")
                        };
                        log::log!(
                            decrypt_fail_log_level(decrypt_fail_mode),
                            "[msg:{}] Decryption failed for {} message from {} due to {label}. \
                             Sending retry receipt.",
                            info.id,
                            enc_type,
                            info.source.sender.observe()
                        );

                        outcome.had_failure = true;
                        outcome.undecryptable |= self
                            .handle_decrypt_failure(info, reason, decrypt_fail_mode)
                            .await;
                        continue;
                    } else if matches!(e, SignalProtocolError::InvalidPreKeyId) {
                        // InvalidPreKeyId on a PreKeyMessage can also mean the
                        // session exists under a PN address (legacy migration).
                        // Migrating lets Signal use the existing ratchet state
                        // instead of looking up the consumed one-time prekey.
                        let migration_outcome = self
                            .try_pn_to_lid_migration_decrypt(
                                sender_encryption_jid,
                                &signal_address,
                                &parsed_message,
                                &mut adapter,
                                &mut rng,
                                enc_type,
                                padding_version,
                                info,
                                &session_mutex,
                                &mut session_guard,
                            )
                            .await;
                        if migration_outcome.decrypted
                            || migration_outcome.duplicate
                            || migration_outcome.plaintext_failed
                        {
                            outcome.decrypted |= migration_outcome.decrypted;
                            outcome.duplicate |= migration_outcome.duplicate;
                            outcome.dispatched |= migration_outcome.dispatched;
                            outcome.skdm_only |= migration_outcome.skdm_only;
                            outcome.plaintext_failed |= migration_outcome.plaintext_failed;
                            outcome.had_failure |= migration_outcome.plaintext_failed;
                            if migration_outcome.plaintext_failed {
                                outcome.undecryptable |=
                                    self.handle_plaintext_failure(info, decrypt_fail_mode).await;
                            }
                            continue;
                        }

                        log::debug!(
                            "[msg:{}] Decryption failed for {} message from {} due to InvalidPreKeyId. \
                             Sender is using a prekey we don't have (likely session established while offline). \
                             Sending retry receipt with fresh prekeys.",
                            info.id,
                            enc_type,
                            info.source.sender.observe()
                        );

                        // Send retry receipt with fresh prekeys
                        outcome.had_failure = true;
                        outcome.undecryptable |= self
                            .handle_decrypt_failure(
                                info,
                                RetryReason::InvalidKeyId,
                                decrypt_fail_mode,
                            )
                            .await;
                        continue;
                    } else {
                        // Catch-all → WA Web's UnhandledError nack (500).
                        log::error!(
                            "[msg:{}] Batch session decrypt failed (type: {}) from {}: {:?}. Sending nack.",
                            info.id,
                            enc_type,
                            info.source.sender.observe(),
                            e
                        );
                        outcome.had_failure = true;
                        outcome.undecryptable |= self
                            .dispatch_undecryptable_event(
                                Arc::clone(info),
                                false,
                                crate::types::events::UnavailableType::Unknown,
                                decrypt_fail_mode,
                            )
                            .await;
                        self.spawn_nack(info, NackReason::UnhandledError, None);
                        continue;
                    }
                }
            }
        }
        outcome
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.group_decrypt", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id), err(Debug)))]
    async fn process_group_enc_batch(
        self: Arc<Self>,
        payloads: &[EncPayload],
        info: &Arc<MessageInfo>,
        _sender_encryption_jid: &Jid,
        decrypt_fail_mode: crate::types::events::DecryptFailMode,
    ) -> Result<(), DecryptionError> {
        if payloads.is_empty() {
            return Ok(());
        }
        let mut adapter = self.signal_adapter().await;

        // Always use bare sender for sender key operations. Real WA delivers
        // skmsg with bare participant but pkmsg (SKDM) with device-qualified
        // participant — normalizing to bare ensures consistent lookup.
        // Hoisted out of the payload loop: all three are loop-invariant.
        let sender_for_sk = info.source.sender.to_non_ad();
        let sender_address = sender_for_sk.to_protocol_address();
        let sender_key_name = make_sender_key_name(&info.source.chat, &sender_address);

        for payload in payloads {
            let ciphertext = &payload.ciphertext[..];
            let padding_version = payload.padding_version;

            log::debug!(
                "Looking up sender key for group {} with sender address {} (from sender JID: {})",
                info.source.chat.observe(),
                sender_address,
                info.source.sender.observe()
            );

            let decrypt_result =
                group_decrypt(ciphertext, &mut adapter.sender_key_store, &sender_key_name).await;

            match decrypt_result {
                Ok(padded_plaintext) => {
                    // Sync device list if sender is unknown, but still process
                    // the message. Signal decryption success already proves the
                    // sender holds the session key — discarding would only add
                    // latency via an unnecessary retry round-trip.
                    if !self.is_from_known_device(&info.source.sender).await {
                        debug!(
                            "[msg:{}] Unknown device {}, triggering device sync",
                            info.id,
                            info.source.sender.observe()
                        );
                        self.handle_unknown_device_sync(info).await;
                    }

                    if let Err(e) = self
                        .clone()
                        .handle_decrypted_plaintext(
                            "skmsg",
                            &padded_plaintext,
                            padding_version,
                            info,
                        )
                        .await
                    {
                        log::warn!("Failed processing group plaintext (batch): {e:?}");
                    }
                }
                Err(SignalProtocolError::DuplicatedMessage(iteration, counter)) => {
                    log::debug!(
                        "Skipping already-processed sender key message from {} in group {} (iteration {}, counter {}). This is normal during reconnection.",
                        info.source.sender.observe(),
                        info.source.chat.observe(),
                        iteration,
                        counter
                    );
                    // Redelivered duplicate: ack it so the server drops it from the
                    // offline queue. status is already acked by the should_ack gate,
                    // so skip it to avoid a redundant receipt. With a durability hook,
                    // a buffered copy means the original commit never acked: replay it.
                    if !info.source.chat.is_status_broadcast() {
                        self.ack_or_replay_to_hook(info).await;
                    }
                }
                Err(SignalProtocolError::NoSenderKeyState(msg)) => {
                    if info.is_expired_status() {
                        log::debug!(
                            "[msg:{}] Skipping retry for expired status from {}",
                            info.id,
                            info.source.sender.observe()
                        );
                        continue;
                    }

                    let is_unknown_device = !self.is_from_known_device(&info.source.sender).await;
                    let retry_reason = if is_unknown_device {
                        RetryReason::UnknownCompanionNoPrekey
                    } else {
                        RetryReason::NoSession
                    };

                    debug!(
                        "No sender key state for group message [msg:{}] from {}: {}. Sending retry receipt.",
                        info.id,
                        info.source.sender.observe(),
                        msg
                    );

                    if is_unknown_device {
                        self.handle_unknown_device_sync(info).await;
                    }

                    self.handle_decrypt_failure(info, retry_reason, decrypt_fail_mode)
                        .await;
                }
                Err(e) => {
                    if info.is_expired_status() {
                        log::debug!(
                            "[msg:{}] Ignoring decrypt error for expired status from {}: {:?}",
                            info.id,
                            info.source.sender.observe(),
                            e
                        );
                        continue;
                    }

                    log::log!(
                        decrypt_fail_log_level(decrypt_fail_mode),
                        "Group batch decrypt failed [msg:{}] for group {} sender {}: {:?}",
                        info.id,
                        sender_key_name.group_id(),
                        sender_key_name.sender_id(),
                        e
                    );
                    // Always surface the failure to consumers; nack only non-status
                    // (status is acked by the should_ack gate) so the server drops
                    // it from the offline queue.
                    self.dispatch_undecryptable_event(
                        Arc::clone(info),
                        false,
                        crate::types::events::UnavailableType::Unknown,
                        decrypt_fail_mode,
                    )
                    .await;
                    if !info.source.chat.is_status_broadcast() {
                        self.spawn_nack(info, NackReason::UnhandledError, None);
                    }
                }
            }
        }
        Ok(())
    }

    /// WA Web: online → `syncDeviceListJob`, offline → `OfflinePendingDeviceCache`.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.unknown_device_sync", level = "debug", skip_all, fields(sender = %info.source.sender.observe(), msg_id = %info.id)))]
    async fn handle_unknown_device_sync(self: &Arc<Self>, info: &MessageInfo) {
        self.schedule_unknown_device_sync(info.source.sender.to_non_ad(), info.is_offline)
            .await;
    }

    /// Refresh a user's device list after encountering one of their devices that
    /// is missing from our registry. Dedups per user (skips a sync already
    /// pending/in-flight), then offline → batch for `doPendingDeviceSync`, online
    /// → invalidate + usync immediately. WA Web: online `syncDeviceListJob`,
    /// offline `OfflinePendingDeviceCache`.
    pub(crate) async fn schedule_unknown_device_sync(
        self: &Arc<Self>,
        user_jid: Jid,
        is_offline: bool,
    ) {
        // Dedup: skip if we already have a sync pending/in-flight for this user
        if !self.pending_device_sync.add(&user_jid).await {
            return;
        }

        if is_offline {
            log::debug!(
                "Queueing {} for pending device sync (offline)",
                user_jid.observe()
            );
        } else {
            log::debug!(
                "Triggering immediate device sync for {}",
                user_jid.observe()
            );
            let client = Arc::clone(self);
            self.runtime
                .spawn(Box::pin(async move {
                    client.invalidate_device_cache(&user_jid.user).await;
                    if let Err(e) = client.get_user_devices(&[user_jid]).await {
                        log::warn!("Immediate device sync failed: {e:?}");
                    }
                }))
                .detach();
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.handle_plaintext", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id, enc_type = %enc_type), err(Debug)))]
    pub(crate) async fn handle_decrypted_plaintext(
        self: Arc<Self>,
        enc_type: &str,
        padded_plaintext: &[u8],
        padding_version: u8,
        info: &Arc<MessageInfo>,
    ) -> Result<PlaintextHandleOutcome, anyhow::Error> {
        let original_msg = wacore::messages::decode_plaintext(padded_plaintext, padding_version)?;
        log::debug!(
            "[msg:{}] Successfully decrypted message from {}: type={} [batch path]",
            info.id,
            info.source.sender.observe(),
            enc_type
        );

        // Validate DSM presence against sender identity
        // (WAWebHandleMsgError.DeviceSentMessageError)
        if original_msg.device_sent_message.is_some() && !info.source.is_from_me {
            warn!(
                "[msg:{}] DeviceSentMessage present but sender {} is not self",
                info.id,
                info.source.sender.observe(),
            );
        }

        // WA Web validateBclHash: a self-synced broadcast/status carries a
        // phashV2 of the broadcast recipients in deviceSentMessage.phash.
        // Recompute over our <participants> view and warn on divergence. We log
        // only (no drop) until the participant hash form is confirmed live.
        if let Some(dsm) = &original_msg.device_sent_message
            && let Some(expected) = dsm.phash.as_deref()
            && !info.bcl_participants.is_empty()
            && !wacore::messages::MessageUtils::validate_bcl_hash(&info.bcl_participants, expected)
        {
            warn!(
                "[msg:{}] bcl hash mismatch on device-sent broadcast (expected={expected}); \
                 keeping message (validate-only)",
                info.id,
            );
        }

        // Unwrap DeviceSentMessage wrapper (self-sent messages synced from
        // the primary device). The actual content (reactions, text, etc.)
        // is nested inside device_sent_message.message and must be
        // extracted before protocol checks or dispatch.
        let mut msg = wacore::messages::unwrap_device_sent(original_msg);

        // Post-decryption logic (SKDM, sync keys, etc.)
        if let Some(skdm) = &msg.sender_key_distribution_message
            && let Some(axolotl_bytes) = &skdm.axolotl_sender_key_distribution_message
        {
            self.handle_sender_key_distribution_message(
                &info.source.chat,
                &info.source.sender,
                axolotl_bytes,
            )
            .await;
        }

        // app_state_sync_key_share is a self-only protocol message (app-state
        // sync keys shared between our own devices). A peer could otherwise
        // inject keys and forge app-state mutations, so honour it only from
        // self. WA Web `WAWebKeyManagementHandleKeyShareApi` gates on
        // `isMeAccountNonLid(from)`; whatsmeow on `info.IsFromMe`.
        if let Some(protocol_msg) = &msg.protocol_message
            && let Some(keys) = &protocol_msg.app_state_sync_key_share
        {
            if info.source.is_from_me {
                self.handle_app_state_sync_key_share(keys).await;
            } else {
                warn!(
                    "[msg:{}] Dropping app_state_sync_key_share from non-self sender {}",
                    info.id,
                    info.source.sender.observe()
                );
            }
        }

        // PDO responses come from our own account (is_from_me) via device 0 (primary phone)
        if info.source.is_from_me
            && let Some(protocol_msg) = &msg.protocol_message
            && let Some(pdo_response) = &protocol_msg.peer_data_operation_request_response_message
        {
            self.handle_pdo_response(pdo_response, info).await;
        }

        // Note: msg might be modified by take() below
        let history_sync_taken = msg
            .protocol_message
            .as_mut()
            .and_then(|pm| pm.history_sync_notification.take());

        // history_sync_notification is self-only (our phone drives history sync).
        // A spoofed one from a peer would force a download of attacker-controlled
        // history, so honour it only from self. WA Web
        // `WAWebHandleHistorySyncNotification` gates on `isMePrimaryNonLid`.
        if let Some(history_sync) = history_sync_taken {
            if info.source.is_from_me {
                self.handle_history_sync(info.id.clone(), history_sync)
                    .await;
            } else {
                warn!(
                    "[msg:{}] Dropping history_sync_notification from non-self sender {}",
                    info.id,
                    info.source.sender.observe()
                );
            }
        }

        // Skip dispatch for messages that only carry sender key distribution
        // (protocol-level key exchange) with no user-visible content.
        // These arrive as a separate pkmsg enc node alongside the actual
        // group message (skmsg) and would otherwise surface as "unknown".
        if wacore::messages::is_sender_key_distribution_only(&mut msg) {
            log::debug!(
                "[msg:{}] Skipping event dispatch for sender key distribution message",
                info.id
            );
            Ok(PlaintextHandleOutcome {
                skdm_only: true,
                ..Default::default()
            })
        } else {
            self.dispatch_parsed_message(msg, info).await;
            Ok(PlaintextHandleOutcome {
                dispatched: true,
                ..Default::default()
            })
        }
    }

    /// Attempt PN→LID session migration and retry decryption.
    /// Returns whether decryption succeeded after migration and whether it
    /// reached user dispatch.
    ///
    /// Manages the per-address session lock around the migration loop:
    /// drops the caller's guard (migration re-enters that mutex and
    /// async_lock is non-reentrant), then reacquires it for the retry
    /// decrypt and replaces the caller's `session_guard` on the way out
    /// so the next payload in the batch stays serialized.
    #[allow(clippy::too_many_arguments)]
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.pn_lid_migration_decrypt", level = "debug", skip_all, fields(sender = %sender_jid.observe(), msg_id = %info.id, enc_type = %enc_type)))]
    async fn try_pn_to_lid_migration_decrypt(
        self: &Arc<Self>,
        sender_jid: &Jid,
        signal_address: &wacore::libsignal::protocol::ProtocolAddress,
        parsed_message: &wacore::libsignal::protocol::CiphertextMessage,
        adapter: &mut crate::store::signal_adapter::SignalProtocolStoreAdapter,
        rng: &mut rand::rngs::StdRng,
        enc_type: &str,
        padding_version: u8,
        info: &Arc<MessageInfo>,
        session_mutex: &Arc<async_lock::Mutex<()>>,
        session_guard: &mut Option<async_lock::MutexGuardArc<()>>,
    ) -> MigrationDecryptOutcome {
        use wacore::libsignal::protocol::{UsePQRatchet, message_decrypt};

        if !sender_jid.is_lid() {
            return MigrationDecryptOutcome::default();
        }

        let Some(pn) = self.lid_pn_cache.get_phone_number(&sender_jid.user).await else {
            return MigrationDecryptOutcome::default();
        };

        // Release the address lock so the migration loop can acquire it for
        // the matching device without re-entering.
        *session_guard = None;
        let migrated = self
            .migrate_signal_sessions_on_lid_discovery(&pn, &sender_jid.user)
            .await;
        // Re-acquire for the retry decrypt and hand the guard back to the
        // caller for subsequent payloads in the batch.
        *session_guard = Some(session_mutex.lock_arc().await);

        // Nothing moved namespaces, so the retry would hit the exact same
        // state, fail identically, and log a second decrypt failure for
        // every redelivered copy of an undecryptable message.
        if !migrated {
            log::debug!(
                "[msg:{}] No PN state to migrate for {}; skipping migration retry decrypt",
                info.id,
                info.source.sender.observe()
            );
            return MigrationDecryptOutcome::default();
        }

        match message_decrypt(
            parsed_message,
            signal_address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
            &mut adapter.pre_key_store,
            &adapter.signed_pre_key_store,
            rng,
            UsePQRatchet::No,
        )
        .await
        {
            // PN→LID migration re-addresses an existing peer; the LID address gets
            // the identity for the first time (NewOrUnchanged), so no local
            // identity-change reaction is warranted here.
            Ok(decrypted) => {
                log::info!(
                    "[msg:{}] Decrypted after PN→LID session migration for {}",
                    info.id,
                    info.source.sender.observe()
                );
                if let Some(prekey_id) = decrypted.consumed_prekey_id {
                    adapter
                        .pre_key_store
                        .buffer_consumed_prekey(prekey_id, signal_address)
                        .await;
                }
                let padded_plaintext = decrypted.plaintext;
                match self
                    .clone()
                    .handle_decrypted_plaintext(enc_type, &padded_plaintext, padding_version, info)
                    .await
                {
                    Ok(plaintext_outcome) => MigrationDecryptOutcome {
                        decrypted: true,
                        dispatched: plaintext_outcome.dispatched,
                        skdm_only: plaintext_outcome.skdm_only,
                        ..Default::default()
                    },
                    Err(e) => {
                        log::warn!(
                            "[msg:{}] Failed processing plaintext after migration: {e:?}",
                            info.id
                        );
                        MigrationDecryptOutcome {
                            decrypted: true,
                            plaintext_failed: true,
                            ..Default::default()
                        }
                    }
                }
            }
            Err(SignalProtocolError::DuplicatedMessage(chain, counter)) => {
                log::debug!(
                    "[msg:{}] Already processed (chain {chain}, counter {counter}) after migration",
                    info.id
                );
                MigrationDecryptOutcome {
                    duplicate: true,
                    ..Default::default()
                }
            }
            Err(retry_err) => {
                log::warn!(
                    "[msg:{}] Decryption still failed after PN→LID migration: {retry_err:?}",
                    info.id
                );
                MigrationDecryptOutcome::default()
            }
        }
    }

    pub(crate) async fn cache_lid_pn_from_message(
        self: &Arc<Self>,
        sender: &Jid,
        alt: Option<&Jid>,
        is_offline: bool,
    ) {
        let (lid_user, pn_user, source) = if sender.server.is_lid_family() {
            if let Some(alt_jid) = alt
                && alt_jid.server.is_pn_family()
            {
                (
                    &sender.user,
                    &alt_jid.user,
                    crate::lid_pn_cache::LearningSource::PeerLidMessage,
                )
            } else {
                return;
            }
        } else if sender.server.is_pn_family() {
            if let Some(alt_jid) = alt
                && alt_jid.server.is_lid_family()
            {
                (
                    &alt_jid.user,
                    &sender.user,
                    crate::lid_pn_cache::LearningSource::PeerPnMessage,
                )
            } else {
                return;
            }
        } else {
            return;
        };

        self.learn_lid_pn_mapping_fast(lid_user, pn_user, source, is_offline)
            .await;
    }

    pub(crate) async fn parse_message_info(
        &self,
        node: &wacore_binary::NodeRef<'_>,
    ) -> Result<MessageInfo, anyhow::Error> {
        // Per-message path: borrow pn/lid from the snapshot, no lock, no clones.
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let default_jid = Jid::default();
        let own_jid = device_snapshot.pn.as_ref().unwrap_or(&default_jid);
        wacore::messages::parse_message_info(node, own_jid, device_snapshot.lid.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_utils::create_test_client_with_failing_http;
    use wacore_binary::Jid;

    // The offline path batches the user for a deferred usync and dedups repeated
    // requests, so a retry storm from one unknown device cannot fan out into a
    // usync storm.
    #[tokio::test]
    async fn schedule_unknown_device_sync_batches_and_dedups() {
        let client = create_test_client_with_failing_http("schedule_resync").await;
        let user: Jid = "12345678901234@lid".parse().unwrap();

        client
            .schedule_unknown_device_sync(user.clone(), true)
            .await;
        // Same user again is deduped, not enqueued twice.
        client
            .schedule_unknown_device_sync(user.clone(), true)
            .await;

        let pending = client.pending_device_sync.take_all().await;
        assert_eq!(pending, vec![user]);
    }
}
