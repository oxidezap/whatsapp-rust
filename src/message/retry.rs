//! Decrypt-failure handling, retry receipts and undecryptable events.

use super::*;

impl Client {
    /// Request retransmission of an inbound message stanza.
    ///
    /// The stanza is parsed once into the canonical message metadata model.
    /// This operation sends only the retry receipt; transport acknowledgement
    /// remains the caller's responsibility.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.request_retry", level = "debug", skip_all, err(Debug))
    )]
    pub async fn request_message_retry(
        self: &Arc<Self>,
        stanza: &NodeRef<'_>,
        options: crate::features::RetryRequestOptions,
    ) -> Result<crate::features::RetryRequestOutcome, crate::features::RetryRequestError> {
        if stanza.tag.as_ref() != "message" {
            return Err(crate::features::RetryRequestError::UnsupportedStanzaClass);
        }
        if stanza.get_attr("id").is_none() {
            return Err(crate::features::RetryRequestError::MissingAttribute("id"));
        }
        if stanza.get_attr("from").is_none() {
            return Err(crate::features::RetryRequestError::MissingAttribute("from"));
        }
        if !self.is_connected() {
            return Err(crate::client::ClientError::NotConnected.into());
        }

        let device = self.persistence_manager.get_device_snapshot();
        let own_pn = device
            .pn
            .as_ref()
            .ok_or(crate::features::RetryRequestError::MissingLocalIdentity)?;
        let info = wacore::messages::parse_message_info(stanza, own_pn, device.lid.as_ref())
            .map_err(crate::features::RetryRequestError::InvalidStanza)?;
        let max_sender_retry_count = message_enc_nodes_for_device(stanza, Some(own_pn))
            .map(sender_retry_count)
            .max()
            .unwrap_or(0);
        let info = Arc::new(info);
        drop(device);

        self.request_retry_for_info(
            &info,
            options,
            (max_sender_retry_count > 0).then_some(max_sender_retry_count),
        )
        .await
    }

    /// Dispatch an `UndecryptableMessage` event at most once per `(chat, id)`
    /// via the single-flight `get_with` semantic on `undecryptable_dispatched`.
    /// The atomic arm avoids the get-then-insert race where two concurrent
    /// callers would both dispatch. Mirrors WA Web's DB-level placeholder
    /// uniqueness in `WAWebMessageProcessPlaceholder`.
    ///
    /// Returns `true` if this call dispatched the event, `false` if a
    /// previous call already did.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.undecryptable", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), msg_id = %info.id)))]
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
        let fresh = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let fresh_clone = fresh.clone();
        self.undecryptable_dispatched
            .get_with(dedup_key, async move {
                fresh_clone.store(true, Ordering::Release);
            })
            .await;
        let was_fresh = fresh.load(Ordering::Acquire);
        if was_fresh {
            wacore::telemetry::recv("undecryptable");
            self.core.event_bus.dispatch(Event::UndecryptableMessage(
                crate::types::events::UndecryptableMessage::builder()
                    .info(info)
                    .is_unavailable(is_unavailable)
                    .unavailable_type(unavailable_type)
                    .decrypt_fail_mode(decrypt_fail_mode)
                    .build(),
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
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.decrypt_failure", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id, reason = ?reason)))]
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

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.plaintext_failure", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), msg_id = %info.id)))]
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
    pub(crate) async fn increment_retry_count(
        &self,
        cache_key: &str,
        reason: RetryReason,
    ) -> Option<u8> {
        self.message_retry_counts
            .upsert_with_by_ref(cache_key, |current| {
                let count = match current {
                    Some((count, _)) if *count >= MAX_DECRYPT_RETRIES => return (None, None),
                    Some((count, _)) => *count + 1,
                    None => 1,
                };
                (Some((count, Some(reason))), Some(count))
            })
            .await
    }

    /// Raise the local retry count to a sender-echoed count without allowing a
    /// concurrent local increment to be overwritten.
    pub(crate) async fn preseed_retry_count(&self, cache_key: &str, sender_count: u8) {
        self.message_retry_counts
            .upsert_with_by_ref(cache_key, |current| match current {
                Some((count, _)) if *count >= sender_count => (None, ()),
                Some((_, reason)) => (Some((sender_count, *reason)), ()),
                None => (Some((sender_count, None)), ()),
            })
            .await;
    }

    /// Generate consistent cache key for retry logic.
    pub(crate) async fn make_retry_cache_key(
        &self,
        chat: &Jid,
        msg_id: &str,
        sender: &Jid,
    ) -> String {
        // Two independent LID/PN resolves for different JIDs — run concurrently.
        let (chat, sender) = futures::join!(
            self.resolve_encryption_jid(chat),
            self.resolve_encryption_jid(sender),
        );
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
    /// When max retries is reached, a PDO request is attempted as a last resort;
    /// the `pdo_requested` memo makes it a no-op if one already went out for
    /// this message, so capped redeliveries cannot re-ask the phone.
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
    /// last-resort PDO). This is the shared operation used by explicit requests
    /// and the automatic decrypt-failure pipeline.
    async fn request_retry_for_info(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        options: crate::features::RetryRequestOptions,
        sender_retry_count: Option<u8>,
    ) -> Result<crate::features::RetryRequestOutcome, crate::features::RetryRequestError> {
        let reason = options.reason();
        let cache_key = self
            .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
            .await;

        if let Some(sender_retry_count) = sender_retry_count {
            self.preseed_retry_count(&cache_key, sender_retry_count)
                .await;
        }

        let Some(retry_count) = self.increment_retry_count(&cache_key, reason).await else {
            log::debug!(
                "Max retries ({}) reached for message {} from {} [{:?}]. Requesting PDO fallback.",
                MAX_DECRYPT_RETRIES,
                info.id,
                info.source.sender.observe(),
                reason
            );
            self.run_pdo_request(info).await;
            return Ok(crate::features::RetryRequestOutcome::LimitReached);
        };

        if retry_count > HIGH_RETRY_COUNT_THRESHOLD {
            log::warn!(
                "High retry count ({}) for message {} in chat {} from {} [{:?}]",
                retry_count,
                info.id,
                info.source.chat.observe(),
                info.source.sender.observe(),
                reason
            );
        }

        let send_result = self
            .send_retry_receipt(info, retry_count, reason, options.force_include_keys())
            .await;

        // PDO is an independent first-attempt recovery path. Preserve it even
        // when building or sending the retry receipt fails; the caller still
        // receives that failure and the automatic pipeline still withholds its
        // transport acknowledgement.
        if retry_count == 1 {
            self.run_pdo_request(info).await;
        }

        let send_outcome = send_result?;

        let outcome = match send_outcome {
            crate::retry::RetryReceiptSendOutcome::Sent { included_keys } => {
                wacore::telemetry::retry_receipt(reason.as_str());
                if retry_count >= MAX_DECRYPT_RETRIES {
                    wacore::telemetry::high_retry(reason.as_str());
                }
                debug!(
                    "Sent retry receipt #{} for message {} in chat {} from {} [{:?}]",
                    retry_count,
                    info.id,
                    info.source.chat.observe(),
                    info.source.sender.observe(),
                    reason
                );
                crate::features::RetryRequestOutcome::Sent {
                    retry_count,
                    included_keys,
                }
            }
            crate::retry::RetryReceiptSendOutcome::Suppressed => {
                crate::features::RetryRequestOutcome::Suppressed { retry_count }
            }
        };

        Ok(outcome)
    }

    /// Awaitable automatic wrapper so retry can be ordered before transport ack.
    ///
    /// Returns whether the caller should send the ack: `false` when we intended
    /// to retry but the send failed (so the stanza stays queued for another try),
    /// `true` when the resend went out or we deliberately gave up at the cap.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.retry_receipt", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id, reason = ?reason)))]
    async fn run_retry_receipt(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        reason: RetryReason,
    ) -> bool {
        match self
            .request_retry_for_info(
                info,
                crate::features::RetryRequestOptions::new().with_reason(reason),
                None,
            )
            .await
        {
            Ok(_) => true,
            Err(error) => {
                log::error!(
                    "Failed to send retry receipt for message {} [{:?}]: {error:?}",
                    info.id,
                    reason
                );
                false
            }
        }
    }
}
