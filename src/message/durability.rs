//! Inbound durability hook: opt-in at-least-once delivery by gating the
//! transport ack on a consumer-provided durable commit. See
//! [`crate::types::durability_hook::InboundDurabilityHook`] for the contract.
//!
//! The first-receipt path lives in [`super::commit_batch`]: messages commit
//! per batch (buffer → hook → ack). This module keeps the redelivery replay,
//! which is inherently per-message: each replayed stanza resolves against its
//! own buffered copy.

use super::*;
use crate::types::durability_hook::InboundDurabilityHook;
use wacore::types::events::InboundMessage;

impl Client {
    /// The registered inbound durability hook, if any. `None` (default) keeps
    /// the at-most-once ack path with zero overhead.
    pub(crate) fn inbound_durability_hook(&self) -> Option<Arc<dyn InboundDurabilityHook>> {
        self.inbound_durability_hook.get().cloned()
    }

    /// Redelivery path: when the server replays an already-decrypted message
    /// (`DuplicatedMessage`), re-commit it from the buffered copy instead of
    /// acking. The replay routes through the commit batcher: during a drain it
    /// joins the accumulating batch, so its hook commit, ack and event keep
    /// arrival order with the fresh stanzas around it; live it commits
    /// immediately as a batch of one. Either way the batch commit rewrites and
    /// then clears its pending row, and consumers observe the message there.
    /// Usually its original batch never dispatched (the hook failed then); if
    /// it did (post-commit row cleanup failed AND the ack was lost), event
    /// consumers see it twice — the documented at-least-once shape of
    /// `Event::Messages` with a hook registered. A plain ack is sent only for
    /// a genuine duplicate (no buffered copy). A read failure fails closed
    /// (no ack) so a transient storage error cannot drop a message that still
    /// needs its hook to commit.
    pub(crate) async fn ack_or_replay_to_hook(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        if self.inbound_durability_hook().is_some() {
            let backend = self.persistence_manager.backend();
            let chat = info.source.chat.to_string();
            let sender = info.source.sender.to_string();
            match backend.get_pending_inbound(&chat, &sender, &info.id).await {
                Ok(Some(bytes)) => match waproto::codec::message_decode(&bytes) {
                    Ok(msg) => {
                        self.commit_or_batch_inbound(
                            InboundMessage::builder()
                                .message(Arc::new(msg))
                                .info(Arc::clone(info))
                                .build(),
                            false,
                        )
                        .await;
                    }
                    Err(e) => {
                        // Corrupt row (our own serialization): it can never be
                        // replayed, so drop it and ack to unstick the queue.
                        log::error!(
                            "[msg:{}] failed to decode buffered inbound message; acking to unstick queue: {e:?}",
                            info.id
                        );
                        let _ = backend
                            .delete_pending_inbound(&chat, &sender, &info.id)
                            .await;
                        self.ack_received_message(info);
                    }
                },
                // Genuine duplicate (never buffered, or already committed): ack it.
                Ok(None) => self.ack_received_message(info),
                // Fail closed: a transient read error must not ack a message whose
                // hook may not have committed. Leave it unacked for the next replay.
                Err(e) => log::warn!(
                    "[msg:{}] failed to read pending inbound buffer; suppressing ack for redelivery: {e:?}",
                    info.id
                ),
            }
        } else {
            self.ack_received_message(info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_client_with_failing_http;
    use crate::types::message::MessageInfo;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use wacore::types::events::BatchOrigin;

    struct CountingHook {
        calls: AtomicUsize,
        messages: AtomicUsize,
        succeed: AtomicBool,
    }

    #[async_trait::async_trait]
    impl InboundDurabilityHook for CountingHook {
        async fn on_messages(
            &self,
            _client: Arc<Client>,
            batch: &[InboundMessage],
        ) -> anyhow::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.messages.fetch_add(batch.len(), Ordering::SeqCst);
            if self.succeed.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("commit failed"))
            }
        }
    }

    fn counting_hook(succeed: bool) -> Arc<CountingHook> {
        Arc::new(CountingHook {
            calls: AtomicUsize::new(0),
            messages: AtomicUsize::new(0),
            succeed: AtomicBool::new(succeed),
        })
    }

    fn test_info(id: &str) -> Arc<MessageInfo> {
        use crate::types::message::MessageSource;
        Arc::new(MessageInfo {
            id: id.to_string(),
            source: MessageSource {
                chat: "100@g.us".parse().unwrap(),
                sender: "200@s.whatsapp.net".parse().unwrap(),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    fn test_item(id: &str) -> InboundMessage {
        InboundMessage::builder()
            .message(Arc::new(wa::Message {
                conversation: Some("hello".to_string()),
                ..Default::default()
            }))
            .info(test_info(id))
            .build()
    }

    // A successful batch commit acks the messages and clears every buffered copy.
    #[tokio::test]
    async fn commit_ok_clears_buffer() {
        let client = create_test_client_with_failing_http("durability_ok").await;
        let hook = counting_hook(true);
        let _ = client.inbound_durability_hook.set(hook.clone());

        let items: Arc<[InboundMessage]> =
            Arc::from([test_item("MSG_OK_1"), test_item("MSG_OK_2")]);
        let infos: Vec<_> = items.iter().map(|i| Arc::clone(&i.info)).collect();
        client
            .commit_inbound_batch(Arc::clone(&items), BatchOrigin::OfflineDrain, None)
            .await;

        assert_eq!(hook.calls.load(Ordering::SeqCst), 1, "one commit per batch");
        assert_eq!(hook.messages.load(Ordering::SeqCst), 2);
        let backend = client.persistence_manager.backend();
        for info in &infos {
            assert!(
                backend
                    .get_pending_inbound(
                        &info.source.chat.to_string(),
                        &info.source.sender.to_string(),
                        &info.id,
                    )
                    .await
                    .unwrap()
                    .is_none(),
                "a committed message must not stay buffered"
            );
        }
    }

    // A failing batch commit suppresses the acks and keeps every buffered copy;
    // later per-message redeliveries replay each one and, once the hook
    // succeeds, clear them.
    #[tokio::test]
    async fn commit_err_keeps_buffer_then_replays() {
        let client = create_test_client_with_failing_http("durability_err").await;
        let hook = counting_hook(false);
        let _ = client.inbound_durability_hook.set(hook.clone());
        let backend = client.persistence_manager.backend();

        let info = test_info("MSG_ERR");
        client
            .commit_inbound_batch(
                Arc::from([InboundMessage::builder()
                    .message(Arc::new(wa::Message {
                        conversation: Some("hello".to_string()),
                        ..Default::default()
                    }))
                    .info(Arc::clone(&info))
                    .build()]),
                BatchOrigin::OfflineDrain,
                None,
            )
            .await;

        assert_eq!(hook.calls.load(Ordering::SeqCst), 1);
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_ERR",
                )
                .await
                .unwrap()
                .is_some(),
            "a failed commit must keep the message buffered for redelivery"
        );

        // Redelivery while the hook still fails: re-runs but keeps the buffer.
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(
            hook.calls.load(Ordering::SeqCst),
            2,
            "redelivery must re-run the hook"
        );
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_ERR",
                )
                .await
                .unwrap()
                .is_some(),
            "a still-failing hook keeps the buffered copy"
        );

        // Redelivery once the commit succeeds clears the buffer AND finally
        // dispatches the event (the original batch never did).
        let (handler, rx) = wacore::types::events::ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);
        hook.succeed.store(true, Ordering::SeqCst);
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(hook.calls.load(Ordering::SeqCst), 3);
        let event = rx.try_recv().expect("successful replay must dispatch");
        assert_eq!(
            event
                .messages()
                .map(|m| m.info.id.as_str())
                .collect::<Vec<_>>(),
            ["MSG_ERR"],
            "consumers must observe a message whose hook only succeeded on replay"
        );
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_ERR",
                )
                .await
                .unwrap()
                .is_none(),
            "a successful replay must clear the buffered copy"
        );
    }

    // A genuine duplicate (no buffered copy) just acks without invoking the hook.
    #[tokio::test]
    async fn replay_without_buffer_just_acks() {
        let client = create_test_client_with_failing_http("durability_dup").await;
        let hook = counting_hook(true);
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.ack_or_replay_to_hook(&test_info("MSG_NONE")).await;
        assert_eq!(
            hook.calls.load(Ordering::SeqCst),
            0,
            "no buffered copy means the hook must not run"
        );
    }
}
