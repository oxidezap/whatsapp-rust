//! Decrypt-failure handling, retry receipts and undecryptable events.

use super::*;

impl Client {
    /// Dispatch an `UndecryptableMessage` event at most once per `(chat, id)`
    /// via the single-flight `get_with` semantic on `undecryptable_dispatched`.
    /// The atomic arm avoids the get-then-insert race where two concurrent
    /// callers would both dispatch. Mirrors WA Web's DB-level placeholder
    /// uniqueness in `WAWebMessageProcessPlaceholder`.
    ///
    /// Returns `true` if this call dispatched the event, `false` if a
    /// previous call already did.
    pub(crate) async fn dispatch_undecryptable_event(
        &self,
        info: Arc<MessageInfo>,
        is_unavailable: bool,
        unavailable_type: crate::types::events::UnavailableType,
        decrypt_fail_mode: crate::types::events::DecryptFailMode,
    ) -> bool {
        let dedup_key =
            wacore::types::message::ChatMessageId::new(info.source.chat.clone(), info.id.clone());
        // The init future only runs for the winning caller. Others receive
        // the cached `()` and leave the flag as false.
        let fresh = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fresh_clone = fresh.clone();
        self.undecryptable_dispatched
            .get_with(dedup_key, async move {
                fresh_clone.store(true, std::sync::atomic::Ordering::Release);
            })
            .await;
        let was_fresh = fresh.load(std::sync::atomic::Ordering::Acquire);
        if was_fresh {
            self.core.event_bus.dispatch(Event::UndecryptableMessage(
                crate::types::events::UndecryptableMessage {
                    info,
                    is_unavailable,
                    unavailable_type,
                    decrypt_fail_mode,
                },
            ));
        } else {
            log::debug!(
                "[msg:{}] UndecryptableMessage already dispatched for this id; skipping duplicate event",
                info.id,
            );
        }
        was_fresh
    }

    /// Dispatch an undecryptable event, then send the retry receipt and the
    /// transport ack in one ordered, flushed task.
    ///
    /// The retry asks the sender to re-encrypt; the ack clears the stanza from
    /// the server's offline queue (the retry alone does not). Both run in a
    /// single `outbound_flush` task so `disconnect()` flushes them together and
    /// the retry always goes out before the ack: if only one makes it, it is the
    /// retry, so the message is never cleared without a resend request. status is
    /// also acked here (flushed) rather than relying on the detached `should_ack`
    /// gate, which can be dropped mid-flush on disconnect; the server dedups the
    /// resulting duplicate ack.
    ///
    /// Returns `true` to be assigned to `dispatched_undecryptable` flag.
    pub(crate) async fn handle_decrypt_failure(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        reason: RetryReason,
        decrypt_fail_mode: crate::types::events::DecryptFailMode,
    ) -> bool {
        self.dispatch_undecryptable_event(
            Arc::clone(info),
            false,
            crate::types::events::UnavailableType::Unknown,
            decrypt_fail_mode,
        )
        .await;
        let client = Arc::clone(self);
        let info = Arc::clone(info);
        self.outbound_flush.spawn(&*self.runtime, async move {
            // A self-fanout is our own message; retrying it to ourselves is
            // futile and the server's offline queue ignores a bare transport
            // ack, so it would replay forever. Clear it with the sender receipt
            // instead (same stanza the success/duplicate paths now emit). Mirror
            // ack_received_message: a bot-authored message in a non-bot chat
            // takes the bot-invoke-response bare ack (the retry path below), not
            // the sender receipt. Gate on the same eligibility as the ack path.
            if info.source.is_self_fanout()
                && !info.source.is_bot_authored_non_bot_chat()
                && Self::should_send_delivery_receipt(&info)
            {
                client.send_delivery_receipt(&info).await;
                return;
            }
            // Only ack once the resend request is actually out; otherwise leave
            // the stanza queued so the server redelivers and we retry.
            let resend_sent = client.run_retry_receipt(&info, reason).await;
            if resend_sent {
                client.send_transport_ack(&info).await;
            }
        });
        true
    }

    pub(crate) async fn handle_plaintext_failure(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        decrypt_fail_mode: crate::types::events::DecryptFailMode,
    ) -> bool {
        let dispatched = self
            .dispatch_undecryptable_event(
                Arc::clone(info),
                false,
                crate::types::events::UnavailableType::Unknown,
                decrypt_fail_mode,
            )
            .await;
        self.spawn_nack(info, NackReason::InvalidProtobuf, None);
        dispatched
    }

    /// Increments the retry count for a message and returns the new count.
    /// Returns `None` if max retries have been reached.
    ///
    /// Note: get-then-insert has a theoretical TOCTOU window since
    /// `spawn_retry_receipt` detaches. In practice, retries for the same
    /// message are rare and a double-send is benign (recipients deduplicate
    /// by message ID).
    pub(crate) async fn increment_retry_count(
        &self,
        cache_key: &str,
        reason: RetryReason,
    ) -> Option<u8> {
        let cache_key = cache_key.to_owned();
        let current = self.message_retry_counts.get(&cache_key).await;
        let new_count = match current {
            Some(count) if count >= MAX_DECRYPT_RETRIES => return None,
            Some(count) => count + 1,
            None => 1,
        };
        self.message_retry_counts
            .insert(cache_key.clone(), new_count)
            .await;
        self.recent_retry_reasons.insert(cache_key, reason).await;
        Some(new_count)
    }

    /// Generate consistent cache key for retry logic.
    pub(crate) async fn make_retry_cache_key(
        &self,
        chat: &Jid,
        msg_id: &str,
        sender: &Jid,
    ) -> String {
        let chat = self.resolve_encryption_jid(chat).await;
        let sender = self.resolve_encryption_jid(sender).await;
        // +40 covers @server suffixes, :device, separators for two JIDs
        let mut key =
            String::with_capacity(chat.user.len() + msg_id.len() + sender.user.len() + 40);
        chat.push_to(&mut key);
        key.push(':');
        key.push_str(msg_id);
        key.push(':');
        sender.push_to(&mut key);
        key
    }

    /// Spawns a task that sends a retry receipt for a failed decryption.
    ///
    /// This is used when sessions are not found or invalid to request the sender to resend
    /// the message with a PreKeySignalMessage to re-establish the session.
    ///
    /// # Retry Count Tracking
    ///
    /// This method tracks retry counts per message (keyed by `{chat}:{msg_id}:{sender}`)
    /// and stops sending retry receipts after `MAX_DECRYPT_RETRIES` (5) attempts to prevent
    /// infinite retry loops. This matches WhatsApp Web's behavior.
    ///
    /// # PDO Backup
    ///
    /// A PDO (Peer Data Operation) request is spawned only on the FIRST retry attempt.
    /// This asks our primary phone to share the already-decrypted message content.
    /// PDO is NOT spawned on subsequent retries to avoid duplicate requests.
    ///
    /// When max retries is reached, an immediate PDO request is sent as a last resort.
    ///
    /// # Arguments
    /// * `info` - The message info for the failed message
    /// * `reason` - The retry reason code (matches WhatsApp Web's RetryReason enum)
    #[cfg(test)]
    pub(crate) fn spawn_retry_receipt(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        reason: RetryReason,
    ) {
        let client = Arc::clone(self);
        let info = Arc::clone(info);
        self.outbound_flush.spawn(&*self.runtime, async move {
            client.run_retry_receipt(&info, reason).await;
        });
    }

    /// Increment the retry count and send the retry receipt (or, at the cap, a
    /// last-resort PDO). Awaitable so it can be ordered before the transport ack.
    ///
    /// Returns whether the caller should send the ack: `false` when we intended
    /// to retry but the send failed (so the stanza stays queued for another try),
    /// `true` when the resend went out or we deliberately gave up at the cap.
    async fn run_retry_receipt(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        reason: RetryReason,
    ) -> bool {
        let cache_key = self
            .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
            .await;

        let Some(retry_count) = self.increment_retry_count(&cache_key, reason).await else {
            log::info!(
                "Max retries ({}) reached for message {} from {} [{:?}]. Sending immediate PDO request.",
                MAX_DECRYPT_RETRIES,
                info.id,
                info.source.sender,
                reason
            );
            // Capped: give up and clear the backlog regardless of PDO outcome.
            self.run_pdo_request(info).await;
            return true;
        };

        if retry_count > HIGH_RETRY_COUNT_THRESHOLD {
            log::warn!(
                "High retry count ({}) for message {} in chat {} from {} [{:?}]",
                retry_count,
                info.id,
                info.source.chat,
                info.source.sender,
                reason
            );
        }

        let retry_sent = match self.send_retry_receipt(info, retry_count, reason).await {
            Ok(()) => {
                debug!(
                    "Sent retry receipt #{} for message {} in chat {} from {} [{:?}]",
                    retry_count, info.id, info.source.chat, info.source.sender, reason
                );
                true
            }
            Err(e) => {
                log::error!(
                    "Failed to send retry receipt #{} for message {} [{:?}]: {:?}",
                    retry_count,
                    info.id,
                    reason,
                    e
                );
                false
            }
        };

        // First retry only, to avoid duplicate PDO requests. Awaited so it runs
        // before the caller's ack; the retry receipt already landed first.
        if retry_count == 1 {
            self.run_pdo_request(info).await;
        }
        retry_sent
    }
}
