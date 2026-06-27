//! Post-decrypt dispatch: event emission, acks and delivery receipts.

use super::*;

impl Client {
    /// Dispatches a successfully parsed message to the event bus and sends a delivery receipt.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.dispatch", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id)))]
    pub(crate) async fn dispatch_parsed_message(
        self: &Arc<Self>,
        msg: wa::Message,
        info: &Arc<MessageInfo>,
    ) {
        use wacore::proto_helpers::MessageExt;
        wacore::telemetry::recv("decrypted");

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
                .as_ref()
                .and_then(|c| c.target_message_key.clone())
        {
            Arc::make_mut(&mut info).comment_target = Some(target);
        }
        let dispatch_msg = Arc::new(decrypted.unwrap_or(msg));

        if let Some(hook) = self.inbound_durability_hook() {
            // At-least-once: in-process handlers still fire, but the transport
            // ack is deferred until the hook durably commits the message.
            self.core
                .event_bus
                .dispatch(Event::Message(Arc::clone(&dispatch_msg), Arc::clone(&info)));
            self.run_inbound_durability_hook(hook, &info, &dispatch_msg)
                .await;
        } else {
            // Default at-most-once path (unchanged): ack, then dispatch.
            self.ack_received_message(&info);
            self.core
                .event_bus
                .dispatch(Event::Message(dispatch_msg, info));
        }
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

    /// Spawn a delivery receipt, tracked so `disconnect()` can flush it (issue #571).
    ///
    /// Offline-drained messages are buffered instead and flushed as aggregate
    /// `<receipt>` stanzas when the offline sync completes, collapsing a
    /// reconnect backlog of N receipts into ~1 stanza per (chat, author)
    /// (WA Web `sendAggregateOfflineReceipts`). Live messages stay 1:1.
    fn spawn_delivery_receipt(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        if info.is_offline && self.try_buffer_offline_receipt(info) {
            return;
        }
        let client = self.clone();
        let info = Arc::clone(info);
        self.outbound_flush.spawn(&*self.runtime, async move {
            client.send_delivery_receipt(&info).await;
        });
    }
}
