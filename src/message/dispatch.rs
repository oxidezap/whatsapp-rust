//! Post-decrypt dispatch: event emission, acks and delivery receipts.

use super::*;

#[inline]
fn delivery_receipt_burst_warning(
    result: &crate::socket::error::EncryptSendResult,
) -> Option<&crate::socket::error::EncryptSendError> {
    match result {
        Err(error) if !error.is_transport_unavailable() => Some(error),
        Ok(()) | Err(_) => None,
    }
}

impl Client {
    /// Dispatches a successfully parsed message to the event bus and sends a delivery receipt.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.dispatch", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id)))]
    pub(crate) async fn dispatch_parsed_message(
        self: &Arc<Self>,
        msg: wa::Message,
        info: &Arc<MessageInfo>,
        track_commit: bool,
    ) -> InboundCommitState {
        use wacore::proto_helpers::MessageExt;
        wacore::telemetry::recv("decrypted");
        self.stats.record_message_received();

        let mut info = Arc::clone(info);
        if info.ephemeral_expiration.is_none()
            && let Some(exp) = msg.get_base_message().get_ephemeral_expiration()
        {
            Arc::make_mut(&mut info).ephemeral_expiration = Some(exp);
        }

        // Keep this ordered with dispatch; add-on messages can immediately
        // reference the secret from the stanza just processed.
        self.maybe_capture_inbound_msg_secret(&msg, &info).await;
        let decrypted = self
            .maybe_decrypt_secret_encrypted_message(&msg, &info)
            .await;
        // A decrypted comment surfaces as its inner body Message, which has no
        // slot for the parent post key; carry the threading link on the info.
        if decrypted.is_some()
            && let Some(target) = msg
                .enc_comment_message
                .as_option()
                .and_then(|c| c.target_message_key.as_option().cloned())
        {
            Arc::make_mut(&mut info).comment_target = Some(target);
        }
        let dispatch_msg = Arc::new(decrypted.unwrap_or(msg));

        // Newsletters never enter the commit pipeline: the plaintext stanza
        // was already transport-acked at enqueue and the server never
        // redelivers it, so a hook failure (or a batcher reset) would lose
        // the message for good instead of trading on redelivery. They also
        // reach here without the processing permit, so enqueueing could
        // straddle the drain→live transition.
        if info.source.chat.is_newsletter() {
            self.core.event_bus.dispatch(Event::Messages(
                wacore::types::events::MessageBatch::builder()
                    .messages(Arc::from([wacore::types::events::InboundMessage::builder(
                    )
                    .message(dispatch_msg)
                    .info(info)
                    .build()]))
                    .origin(wacore::types::events::BatchOrigin::Live)
                    .build(),
            ));
            return InboundCommitState::Durable;
        }

        // Live traffic commits (and acks) as a batch of one; during the
        // offline drain the message joins the accumulating commit batch and
        // the event/ack fire only after its batch commits. Either way the
        // hook (when registered) gates everything observable.
        self.commit_or_batch_inbound(
            wacore::types::events::InboundMessage::builder()
                .message(dispatch_msg)
                .info(info)
                .build(),
            track_commit,
        )
        .await
    }

    /// Acknowledge a received message so the server drops it from the offline
    /// queue: a delivery receipt when applicable (incl. the `type="sender"`
    /// receipt for own-account self-fanouts), else a transport ack. status is
    /// acked by the `should_ack` gate, newsletters/empty ids need nothing here.
    pub(crate) fn ack_received_message(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        if info.id.is_empty() || info.source.chat.is_newsletter() {
            return;
        }
        // WA Web `sendAggregateReceipts`: for a DELIVERY where the chat is NOT
        // a bot but the author IS a bot (a bot reply inside a group), it emits
        // a bare `<ack class="message">` via `sendBotInvokeResponseAcks`, not a
        // `<receipt>`. A 1:1 bot chat keeps the normal receipt (chat.isBot() →
        // the branch's `v` is false). Our transport ack is that bare
        // `<ack class="message">` (group form carries `participant`).
        if info.source.is_bot_authored_non_bot_chat() {
            self.spawn_message_ack(info);
            return;
        }
        if Self::should_send_delivery_receipt(info) {
            self.spawn_delivery_receipt(info);
        } else if !info.source.chat.is_status_broadcast() {
            self.spawn_message_ack(info);
        }
    }

    /// Queue a delivery receipt, tracked so `disconnect()` can flush it (issue #571).
    ///
    /// Live receipts feed a single persistent worker instead of one spawned
    /// task each: the per-message cost drops to a channel slot plus a
    /// [`crate::flush_scope::FlushGuard`], and `flush()` still waits because
    /// the guard rides the queue until the send completes.
    ///
    /// Offline-drained messages are buffered instead and flushed as aggregate
    /// `<receipt>` stanzas when the offline sync completes, collapsing a
    /// reconnect backlog of N receipts into ~1 stanza per (chat, author)
    /// (WA Web `sendAggregateOfflineReceipts`). Live messages stay 1:1.
    fn spawn_delivery_receipt(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        if info.is_offline && self.try_buffer_offline_receipt(info) {
            return;
        }
        // A closed scope (disconnect in progress) drops the receipt, exactly
        // like the previous spawn-per-receipt path.
        let Some(guard) = self.outbound_flush.try_track() else {
            return;
        };
        let tx = self
            .delivery_receipt_queue
            .get_or_init(|| self.start_delivery_receipt_worker());
        // Only fails if the worker exited (client teardown); dropping the
        // guard here keeps `flush()` honest.
        let _ = tx.try_send((Arc::clone(info), guard));
    }

    /// How many queued receipts one burst may take; mirrors the ack worker's
    /// [`MAX_ACK_BURST`](Client::MAX_ACK_BURST), where the tradeoff is measured.
    const MAX_RECEIPT_BURST: usize = 4;

    /// Worker task shared by every live delivery receipt. Holds only a `Weak`
    /// so a dropped `Client` closes the channel and ends the task instead of
    /// keeping the client alive.
    fn start_delivery_receipt_worker(
        self: &Arc<Self>,
    ) -> async_channel::Sender<(Arc<MessageInfo>, crate::flush_scope::FlushGuard)> {
        let (tx, rx) =
            async_channel::unbounded::<(Arc<MessageInfo>, crate::flush_scope::FlushGuard)>();
        let client = Arc::downgrade(self);
        self.runtime
            .spawn(Box::pin(async move {
                // Reuse the bounded control buffers for the worker's lifetime.
                // Encoded payload allocations still move into `Bytes`; only
                // the outer storage stays here.
                let mut batch = Vec::with_capacity(Self::MAX_RECEIPT_BURST);
                let mut frames = Vec::with_capacity(Self::MAX_RECEIPT_BURST);
                let mut guards = Vec::with_capacity(Self::MAX_RECEIPT_BURST);
                while let Ok(first) = rx.recv().await {
                    let Some(client) = client.upgrade() else {
                        break;
                    };

                    // Same reasoning as the ack worker: awaiting each receipt
                    // before reading the next means the noise sender never has
                    // two frames to coalesce. `try_recv` only, so nothing waits
                    // on work that has not arrived.
                    batch.push(first);
                    while batch.len() < Self::MAX_RECEIPT_BURST
                        && let Ok(next) = rx.try_recv()
                    {
                        batch.push(next);
                    }

                    // No teardown gate here, unlike the ack worker: the
                    // single-receipt path never had one either (it relies on
                    // the socket reporting NotConnected), and adding one for
                    // symmetry would silently start dropping receipts that
                    // today still go out.
                    for (info, guard) in batch.drain(..) {
                        // Building the node is synchronous, so the burst is
                        // fully prepared before anything reaches the socket.
                        if let Some(frame) = client.prepare_delivery_receipt(&info) {
                            frames.push(frame);
                            guards.push(guard);
                        }
                    }
                    if frames.is_empty() {
                        continue;
                    }

                    // Spans the await *and* the result inspection: a receipt
                    // that stalls or fails in the transport has to show up
                    // inside the span, not after it closed. The per-receipt
                    // `wa.receipt.send_delivery` span stays on the
                    // single-receipt path, which this one does not use.
                    let frame_count = frames.len();
                    let send_and_report = async {
                        match client.send_raw_bytes_burst(&mut frames).await {
                            Ok(results) => {
                                for result in results {
                                    if let Some(error) = delivery_receipt_burst_warning(&result) {
                                        log::warn!(target: "Client/Receipt", "Failed to send delivery receipt: {error:?}");
                                    }
                                }
                            }
                            Err(e) => {
                                if !matches!(e, crate::client::ClientError::NotConnected) {
                                    log::warn!(target: "Client/Receipt", "Failed to send delivery receipt burst: {e:?}");
                                }
                            }
                        }
                    };
                    #[cfg(feature = "tracing")]
                    {
                        use tracing::Instrument;
                        send_and_report
                            .instrument(tracing::debug_span!(
                                "wa.receipt.delivery_burst",
                                frames = frame_count
                            ))
                            .await;
                    }
                    #[cfg(not(feature = "tracing"))]
                    {
                        let _ = frame_count;
                        send_and_report.await;
                    }
                    debug_assert!(
                        frames.is_empty(),
                        "send_raw_bytes_burst must always drain its input"
                    );
                    guards.clear();
                }
            }))
            .detach();
        tx
    }
}

#[cfg(test)]
mod tests {
    use super::delivery_receipt_burst_warning;
    use crate::socket::error::EncryptSendError;

    #[test]
    fn receipt_burst_logging_is_quiet_for_success_and_reconnect_failures() {
        let reconnect_failures = [
            Ok(()),
            Err(EncryptSendError::transport(anyhow::anyhow!(
                "transport unavailable"
            ))),
            Err(EncryptSendError::channel_closed()),
            Err(EncryptSendError::poisoned()),
        ];

        for result in &reconnect_failures {
            assert!(
                delivery_receipt_burst_warning(result).is_none(),
                "success and reconnect-related failures must stay quiet: {result:?}"
            );
        }
    }

    #[test]
    fn receipt_burst_logging_keeps_actionable_local_failures_visible() {
        let actionable_failures = [
            Err(EncryptSendError::crypto(anyhow::anyhow!(
                "encryption failed"
            ))),
            Err(EncryptSendError::framing(anyhow::anyhow!("framing failed"))),
            Err(EncryptSendError::join(anyhow::anyhow!(
                "sender join failed"
            ))),
        ];

        for result in &actionable_failures {
            assert!(
                delivery_receipt_burst_warning(result).is_some(),
                "local send failures must remain visible: {result:?}"
            );
        }
    }
}
