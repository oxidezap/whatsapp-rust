//! Post-decrypt dispatch: event emission, acks and delivery receipts.

use super::*;

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
                while let Ok((info, guard)) = rx.recv().await {
                    let Some(client) = client.upgrade() else {
                        break;
                    };
                    client.send_delivery_receipt(&info).await;
                    drop(guard);
                }
            }))
            .detach();
        tx
    }
}
