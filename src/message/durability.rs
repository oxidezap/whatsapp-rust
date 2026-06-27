//! Inbound durability hook: opt-in at-least-once delivery by gating the
//! transport ack on a consumer-provided durable commit. See
//! [`crate::types::durability_hook::InboundDurabilityHook`] for the contract.

use super::*;
use crate::types::durability_hook::InboundDurabilityHook;

impl Client {
    /// The registered inbound durability hook, if any. `None` (default) keeps
    /// the at-most-once ack path with zero overhead.
    pub(crate) fn inbound_durability_hook(&self) -> Option<Arc<dyn InboundDurabilityHook>> {
        self.inbound_durability_hook.get().cloned()
    }

    /// First-receipt path: buffer the decrypted message durably, run the hook,
    /// and ack only on success. On failure the buffered row is left for the
    /// server's redelivery to retry. Replaces the plain ack for a freshly
    /// dispatched user message when a hook is configured.
    pub(crate) async fn run_inbound_durability_hook(
        self: &Arc<Self>,
        hook: Arc<dyn InboundDurabilityHook>,
        info: &Arc<MessageInfo>,
        msg: &Arc<wa::Message>,
    ) {
        let backend = self.persistence_manager.backend();
        let chat = info.source.chat.to_string();
        let sender = info.source.sender.to_string();
        // Persist before the Signal ratchet is flushed (which happens after this
        // returns) so a crash mid-commit replays the message instead of losing it.
        // `message_to_vec` is the shared non-generic encoder so this call does not
        // monomorphize the whole `wa::Message` proto tree into this crate.
        let bytes = waproto::codec::message_to_vec(msg);
        // Fail closed: if we cannot durably buffer the message, do not run the
        // hook and do not ack. The server keeps it queued and redelivers it once
        // storage recovers, rather than us acking a message we cannot replay.
        if let Err(e) = backend
            .store_pending_inbound(&chat, &sender, &info.id, &bytes)
            .await
        {
            log::error!(
                "[msg:{}] failed to buffer inbound message; suppressing ack for redelivery: {e:?}",
                info.id
            );
            return;
        }

        match hook.on_message(self.clone(), info, msg).await {
            Ok(()) => {
                if let Err(e) = backend
                    .delete_pending_inbound(&chat, &sender, &info.id)
                    .await
                {
                    log::debug!(
                        "[msg:{}] failed to clear buffered inbound message: {e:?}",
                        info.id
                    );
                }
                self.ack_received_message(info);
            }
            Err(e) => {
                log::warn!(
                    "[msg:{}] inbound durability hook failed; suppressing ack for redelivery: {e:?}",
                    info.id
                );
            }
        }
    }

    /// Redelivery path: when the server replays an already-decrypted message
    /// (`DuplicatedMessage`), re-run the hook from the buffered copy instead of
    /// acking. A plain ack is sent only for a genuine duplicate (no buffered
    /// copy). A read failure fails closed (no ack) so a transient storage error
    /// cannot drop a message that still needs its hook to commit.
    pub(crate) async fn ack_or_replay_to_hook(self: &Arc<Self>, info: &Arc<MessageInfo>) {
        if let Some(hook) = self.inbound_durability_hook() {
            let backend = self.persistence_manager.backend();
            let chat = info.source.chat.to_string();
            let sender = info.source.sender.to_string();
            match backend.get_pending_inbound(&chat, &sender, &info.id).await {
                Ok(Some(bytes)) => {
                    match waproto::codec::message_decode(&bytes) {
                        Ok(msg) => {
                            let msg = Arc::new(msg);
                            match hook.on_message(self.clone(), info, &msg).await {
                                Ok(()) => {
                                    if let Err(e) = backend
                                        .delete_pending_inbound(&chat, &sender, &info.id)
                                        .await
                                    {
                                        log::debug!(
                                            "[msg:{}] failed to clear buffered inbound message: {e:?}",
                                            info.id
                                        );
                                    }
                                    self.ack_received_message(info);
                                }
                                Err(e) => {
                                    log::warn!(
                                        "[msg:{}] inbound durability hook still failing on redelivery; keeping for retry: {e:?}",
                                        info.id
                                    );
                                }
                            }
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
                    }
                }
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

    struct CountingHook {
        calls: AtomicUsize,
        succeed: AtomicBool,
    }

    #[async_trait::async_trait]
    impl InboundDurabilityHook for CountingHook {
        async fn on_message(
            &self,
            _client: Arc<Client>,
            _info: &MessageInfo,
            _message: &wa::Message,
        ) -> anyhow::Result<()> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.succeed.load(Ordering::SeqCst) {
                Ok(())
            } else {
                Err(anyhow::anyhow!("commit failed"))
            }
        }
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

    fn test_msg() -> Arc<wa::Message> {
        Arc::new(wa::Message {
            conversation: Some("hello".to_string()),
            ..Default::default()
        })
    }

    // A successful hook acks the message and clears the buffered copy.
    #[tokio::test]
    async fn hook_ok_clears_buffer() {
        let client = create_test_client_with_failing_http("durability_ok").await;
        let hook = Arc::new(CountingHook {
            calls: AtomicUsize::new(0),
            succeed: AtomicBool::new(true),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        let info = test_info("MSG_OK");
        client
            .run_inbound_durability_hook(
                client.inbound_durability_hook().unwrap(),
                &info,
                &test_msg(),
            )
            .await;

        assert_eq!(hook.calls.load(Ordering::SeqCst), 1);
        let backend = client.persistence_manager.backend();
        assert!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    "MSG_OK",
                )
                .await
                .unwrap()
                .is_none(),
            "a committed message must not stay buffered"
        );
    }

    // A failing hook suppresses the ack and keeps the buffered copy; a later
    // redelivery re-runs the hook and, once it succeeds, clears the buffer.
    #[tokio::test]
    async fn hook_err_keeps_buffer_then_replays() {
        let client = create_test_client_with_failing_http("durability_err").await;
        let hook = Arc::new(CountingHook {
            calls: AtomicUsize::new(0),
            succeed: AtomicBool::new(false),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let backend = client.persistence_manager.backend();

        let info = test_info("MSG_ERR");
        client
            .run_inbound_durability_hook(
                client.inbound_durability_hook().unwrap(),
                &info,
                &test_msg(),
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

        // Redelivery once the commit succeeds clears the buffer.
        hook.succeed.store(true, Ordering::SeqCst);
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(hook.calls.load(Ordering::SeqCst), 3);
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
        let hook = Arc::new(CountingHook {
            calls: AtomicUsize::new(0),
            succeed: AtomicBool::new(true),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        client.ack_or_replay_to_hook(&test_info("MSG_NONE")).await;
        assert_eq!(
            hook.calls.load(Ordering::SeqCst),
            0,
            "no buffered copy means the hook must not run"
        );
    }
}
