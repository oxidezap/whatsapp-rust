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
#[cfg(test)]
use wacore::types::events::InboundMessage;

impl Client {
    /// The registered inbound durability hook, if any. `None` (default) keeps
    /// the at-most-once ack path without live-retention locking or backing.
    pub(crate) fn inbound_durability_hook(&self) -> Option<Arc<dyn InboundDurabilityHook>> {
        self.inbound_durability_hook.get().cloned()
    }

    // Recovery owns the retained entries in a cold frame; normal session
    // processing does not need their iterator and plaintext buffers inline.
    async fn replay_retained_raw(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        raw: Vec<live_retention::RawRetained>,
    ) -> bool {
        for entry in raw {
            // Do not redo Signal decryption: its ratchet already advanced.
            // Raw staging has not yet prepared a consumer projection.
            let _charge = Arc::clone(&entry.charge);
            if entry.enc_type == "msmsg" {
                if !Box::pin(self.replay_retained_bot(info, entry)).await {
                    return false;
                }
                continue;
            }
            let outcome = Box::pin(self.handle_decrypted_plaintext_in_scope(
                entry.enc_type,
                entry.plaintext.to_vec(),
                entry.padding_version,
                receive::PlaintextSlot {
                    index: entry.origin.position,
                    annotations: EncNodeAnnotations {
                        state: None,
                        session_type: None,
                    },
                    producer: entry.origin.stanza,
                },
                info,
            ))
            .await;
            if !outcome.is_ok_and(|outcome| outcome.delivered || !outcome.dispatched) {
                return false;
            }
        }
        true
    }

    // The decoded bot graph belongs to this cold replay frame, rather than
    // enlarging the future retained by every chat lane.
    async fn replay_retained_bot(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        entry: live_retention::RawRetained,
    ) -> bool {
        let Ok(message) = waproto::codec::message_decode(&entry.plaintext) else {
            return false;
        };
        let Ok(index) = u32::try_from(entry.origin.position) else {
            return false;
        };
        matches!(
            self.dispatch_shared_message_with_carrier(
                Arc::new(message),
                info,
                false,
                None,
                Some((Some(index), entry.plaintext)),
                Some(entry.origin),
            )
            .await,
            InboundCommitState::Durable
        )
    }

    /// Redelivery path: when the server replays an already-decrypted message
    /// (`DuplicatedMessage`), re-commit it from the buffered copy instead of
    /// acking. The replay routes through the commit batcher: during a drain it
    /// joins the accumulating batch, so its hook commit, ack and event keep
    /// arrival order with the fresh stanzas around it; live it commits
    /// immediately as a batch of one. Either way the batch commit rewrites and
    /// then clears its pending row, and consumers observe the message there.
    /// Usually its original batch never dispatched because the hook failed.
    /// A completed hook publication suppresses cleanup replay within this
    /// Client's dispatch cache. Hooks and consumers still need idempotence
    /// across cache expiry or process restart. A plain ack is sent only for
    /// a genuine duplicate (no buffered copy). A read failure fails closed
    /// (no ack) so a transient storage error cannot drop a message that still
    /// needs its hook to commit.
    ///
    /// Returns `true` when a buffered copy was replayed *and* its commit will
    /// dispatch it, which hands the message to consumers again: a caller
    /// counting suppressions must not count that one. A replay whose commit
    /// failed returns `false`, because nothing reached a consumer.
    pub(crate) async fn ack_or_replay_to_hook(self: &Arc<Self>, info: &Arc<MessageInfo>) -> bool {
        if self.inbound_durability_hook().is_some() {
            let raw = self
                .inbound_commit_batch
                .live_retention
                .raw_for_message(info);
            if !raw.is_empty() {
                return Box::pin(self.replay_retained_raw(info, raw)).await;
            }
            let retained = match self.inbound_commit_batch.live_retention.for_message(info) {
                Ok(retained) => retained,
                Err(error) => {
                    log::warn!("Retained projection decode failed: {error:?}");
                    return false;
                }
            };
            if !retained.is_empty() {
                let retained = match self.live_pending_retry_batch(&retained, true).await {
                    Ok(Some(hydrated)) => hydrated,
                    Ok(None) => retained,
                    Err(error) => {
                        log::warn!("Retained producer recovery failed: {error:?}");
                        return false;
                    }
                };
                let delivered = self.replay_pending_batch(retained.into(), false).await;
                if delivered {
                    self.ack_received_message(info);
                }
                return delivered;
            }
            match self.read_pending_for_replay(info).await {
                Ok(Some((sender, record))) => {
                    // Decode before releasing the read owner, then release it
                    // before the batch's conditional write takes that owner.
                    let (bytes, read_lease) = record.into_bytes();
                    let decoded = raw_pending::PendingJournal::decode(bytes);
                    let storage_sender: Arc<str> = sender.into();
                    drop(read_lease);
                    let replay = match decoded.and_then(|journal| {
                        journal
                            .occurrences
                            .into_iter()
                            .enumerate()
                            .map(|(position, entry)| {
                                // Preserve the replay position and producer absence of legacy rows.
                                let mut delivery = raw_pending::PendingDelivery::from_occurrence(
                                    entry, position, info, None, None,
                                )?;
                                delivery.storage_sender = Some(Arc::clone(&storage_sender));
                                delivery.receipt =
                                    self.inbound_commit_batch.live_retention.receipt_for(info);
                                Ok(delivery)
                            })
                            .collect::<anyhow::Result<Vec<_>>>()
                    }) {
                        Ok(replay) => replay,
                        Err(error) => {
                            log::error!(
                                "[msg:{}] invalid pending journal; preserving row and suppressing ACK: {error:?}",
                                info.id
                            );
                            return false;
                        }
                    };
                    if !self.inbound_commit_batch.is_active() {
                        return self.replay_pending_batch(replay.into(), true).await;
                    }
                    for item in replay {
                        if matches!(
                            self.commit_or_batch_pending(item, false).await,
                            InboundCommitState::Failed
                        ) {
                            return false;
                        }
                    }
                    return true;
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
        false
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
            id: id.into(),
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
            .message(Arc::new({
                let mut proto_fixture = wa::Message::default();
                proto_fixture.conversation = Some("hello".to_string());
                proto_fixture
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
                    .message(Arc::new({
                        let mut proto_fixture = wa::Message::default();
                        proto_fixture.conversation = Some("hello".to_string());
                        proto_fixture
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
        client.core.event_bus.subscribe_handler(handler).detach();
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

    #[tokio::test]
    async fn replay_malformed_row_keeps_original_bytes_and_does_not_deliver() {
        let client = create_test_client_with_failing_http("durability_malformed").await;
        let hook = counting_hook(true);
        let _ = client.inbound_durability_hook.set(hook.clone());
        let info = test_info("MSG_MALFORMED");
        let backend = client.persistence_manager.backend();
        let chat = info.source.chat.to_string();
        let sender = info.source.sender.to_string();
        // Zero is not a valid protobuf tag. It also discriminates a private
        // envelope; unsupported versions must never be deleted as corruption.
        let original = b"\0WRPI\xfforiginal";
        backend
            .store_pending_inbound(&chat, &sender, &info.id, original)
            .await
            .expect("fixture row");
        let (handler, rx) = wacore::types::events::ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        assert!(!client.ack_or_replay_to_hook(&info).await);
        assert_eq!(hook.calls.load(Ordering::SeqCst), 0);
        assert!(rx.try_recv().is_err());
        assert_eq!(
            backend
                .get_pending_inbound(&chat, &sender, &info.id)
                .await
                .expect("read row")
                .as_deref(),
            Some(original.as_slice())
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

    // These exercise the existing receive entry point, not a decode/re-encode
    // oracle. The original synthetic unknown field is also a noncanonical
    // varint and precedes the known fields, so normalized storage cannot pass.
    async fn raw_origin_fixture(name: &str, message: wa::Message, from_me: bool) {
        let client = create_test_client_with_failing_http(name).await;
        let hook = counting_hook(false);
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        let mut info = (*test_info("RAW_ORIGIN")).clone();
        info.source.is_from_me = from_me;
        let info = Arc::new(info);
        let mut raw = vec![0xa0, 0x1f, 0x81, 0x00]; // field500 varint1, exact bytes
        raw.extend(waproto::codec::message_to_vec(&message));
        assert!(waproto::codec::message_decode(&raw).is_ok());
        client
            .handle_decrypted_plaintext(
                "msg",
                wacore::messages::MessageUtils::pad_message_v2(raw.clone()),
                2,
                0,
                Default::default(),
                &info,
            )
            .await
            .unwrap();
        assert_eq!(
            hook.calls.load(Ordering::SeqCst),
            1,
            "the hook failed after receive normalization"
        );
        let stored = client
            .persistence_manager
            .backend()
            .get_pending_inbound(
                &info.source.chat.to_string(),
                &info.source.sender.to_string(),
                &info.id,
            )
            .await
            .unwrap()
            .expect("failed hook must leave a durable record");
        assert!(
            stored.windows(raw.len()).any(|bytes| bytes == raw),
            "pending replay must retain original unknown fields, order and noncanonical bytes before normalization"
        );
    }

    #[tokio::test]
    async fn raw_pending_dsm_preserves_original_noncanonical_carrier() {
        let mut message = wa::Message::default();
        let mut inner = wa::Message::default();
        inner.conversation = Some("synthetic DSM delivery".into());
        let mut dsm = wa::message::DeviceSentMessage::default();
        dsm.message = buffa::MessageField::some(inner);
        message.device_sent_message = buffa::MessageField::some(dsm);
        raw_origin_fixture("raw_dsm", message, true).await;
    }

    #[tokio::test]
    async fn raw_pending_history_preserves_detached_original_carrier() {
        let mut message = wa::Message::default();
        message.conversation = Some("synthetic accompanying delivery".into());
        let mut protocol = wa::message::ProtocolMessage::default();
        protocol.r#type = Some(wa::message::protocol_message::Type::HISTORY_SYNC_NOTIFICATION);
        let mut history = wa::message::HistorySyncNotification::default();
        history.initial_hist_bootstrap_inline_payload = Some(vec![7; 128]);
        protocol.history_sync_notification = buffa::MessageField::some(history);
        message.protocol_message = buffa::MessageField::some(protocol);
        // A peer history notification is detached but rejected by the self-only
        // gate, so this fixture cannot download or acknowledge real history.
        raw_origin_fixture("raw_history", message, false).await;
    }

    struct OrderedReplayHook {
        succeed: AtomicBool,
        attempted: std::sync::Mutex<Vec<String>>,
        observed: std::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl InboundDurabilityHook for OrderedReplayHook {
        async fn on_messages(
            &self,
            _: Arc<Client>,
            batch: &[InboundMessage],
        ) -> anyhow::Result<()> {
            self.attempted
                .lock()
                .unwrap()
                .extend(batch.iter().map(|item| {
                    item.message
                        .conversation
                        .as_deref()
                        .unwrap_or_default()
                        .to_owned()
                }));
            if !self.succeed.load(Ordering::SeqCst) {
                return Err(anyhow::anyhow!("synthetic consumer unavailable"));
            }
            self.observed
                .lock()
                .unwrap()
                .extend(batch.iter().map(|item| {
                    item.message
                        .conversation
                        .as_deref()
                        .unwrap_or_default()
                        .to_owned()
                }));
            Ok(())
        }
    }

    #[tokio::test]
    async fn raw_pending_failed_replay_does_not_report_consumer_delivery() {
        let client = create_test_client_with_failing_http("raw_replay_failed").await;
        let hook = counting_hook(false);
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        client
            .commit_or_batch_inbound(test_item("RAW_REPLAY_FAILED"), false)
            .await;
        assert!(
            !client
                .ack_or_replay_to_hook(&test_info("RAW_REPLAY_FAILED"))
                .await
        );
        assert_eq!(hook.calls.load(Ordering::SeqCst), 2);
        assert!(
            client
                .persistence_manager
                .backend()
                .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "RAW_REPLAY_FAILED")
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn raw_pending_subset_cannot_clear_uncommitted_occurrence_group() {
        let client = create_test_client_with_failing_http("raw_subset").await;
        let hook = counting_hook(false);
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        let info = test_info("RAW_SUBSET");
        for (index, text) in ["A", "B"].into_iter().enumerate() {
            let mut message = wa::Message::default();
            message.conversation = Some(text.into());
            if index == 1 {
                hook.succeed.store(true, Ordering::SeqCst);
            }
            client
                .handle_decrypted_plaintext(
                    "msg",
                    wacore::messages::MessageUtils::encode_and_pad(&message),
                    2,
                    index,
                    Default::default(),
                    &info,
                )
                .await
                .unwrap();
        }
        assert!(client.delivery_receipt_queue.get().is_none());
        assert!(
            client.transport_ack_queue.get().is_none(),
            "a successful subset hook must not acknowledge the retained group"
        );
        let backend = client.persistence_manager.backend();
        let bytes = backend
            .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "RAW_SUBSET")
            .await
            .unwrap()
            .expect("subset commit cannot delete predecessor");
        let journal = raw_pending::PendingJournal::decode(bytes.into()).unwrap();
        assert_eq!(journal.occurrences.len(), 2);
        assert!(client.ack_or_replay_to_hook(&info).await);
        assert!(
            client.delivery_receipt_queue.get().is_some()
                || client.transport_ack_queue.get().is_some(),
            "only the complete replay may schedule the stanza acknowledgement"
        );
        assert!(
            backend
                .get_pending_inbound("100@g.us", "200@s.whatsapp.net", "RAW_SUBSET")
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn raw_pending_replay_preserves_equal_occurrences_and_later_part() {
        let client = create_test_client_with_failing_http("raw_occurrences").await;
        let hook = Arc::new(OrderedReplayHook {
            succeed: AtomicBool::new(false),
            attempted: std::sync::Mutex::new(Vec::new()),
            observed: std::sync::Mutex::new(Vec::new()),
        });
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        let info = test_info("RAW_MULTIPART");
        let origin = raw_pending::StanzaOrigin::new();
        for (index, text) in ["A", "A", "B"].into_iter().enumerate() {
            let mut message = wa::Message::default();
            message.conversation = Some(text.into());
            client
                .handle_decrypted_plaintext_in_scope(
                    "msg",
                    wacore::messages::MessageUtils::encode_and_pad(&message),
                    2,
                    receive::PlaintextSlot {
                        index,
                        annotations: Default::default(),
                        producer: origin,
                    },
                    &info,
                )
                .await
                .unwrap();
        }
        assert_eq!(
            *hook.attempted.lock().unwrap(),
            ["A", "A", "B"],
            "all three original occurrences reached the failing hook"
        );
        hook.succeed.store(true, Ordering::SeqCst);
        assert!(client.ack_or_replay_to_hook(&info).await);
        assert_eq!(
            *hook.observed.lock().unwrap(),
            ["A", "A", "B"],
            "one pending row must preserve the whole occurrence group, not only its last normalized payload"
        );
    }
}
