//! Inbound durability hook: pending-record replay and the consumer commit barrier.
//! See
//! [`crate::types::durability_hook::InboundDurabilityHook`] for the contract.
//!
//! The first-receipt path lives in [`super::commit_batch`]: messages commit
//! per batch (buffer → hook → ack). This module keeps the redelivery replay,
//! where each replayed stanza resolves all of its stored payload parts.

use super::*;
use crate::types::durability_hook::InboundDurabilityHook;
use wacore::types::events::InboundMessage;

// Zero is not a protobuf field tag, so the envelope cannot alias an existing
// serialized Message. Single-message rows keep their original representation.
const PARTS_HEADER: &[u8] = b"\0WAPI\x01";

pub(super) fn encode_pending_parts(parts: &[&[u8]]) -> Vec<u8> {
    let mut record = Vec::new();
    record.extend_from_slice(PARTS_HEADER);
    for part in parts {
        // A slice cannot exceed u64 on supported targets. Lengths are decoded
        // against the remaining input before any payload allocation.
        record.extend_from_slice(&(part.len() as u64).to_be_bytes());
        record.extend_from_slice(part);
    }
    record
}

fn pending_parts(bytes: &[u8]) -> anyhow::Result<Vec<&[u8]>> {
    if bytes.first() != Some(&0) {
        return Ok(vec![bytes]);
    }
    let mut remaining = bytes
        .strip_prefix(PARTS_HEADER)
        .ok_or_else(|| anyhow::anyhow!("unknown pending-inbound record header"))?;
    let mut parts = Vec::new();
    while !remaining.is_empty() {
        anyhow::ensure!(remaining.len() >= 8, "truncated pending-inbound length");
        let mut length = [0; 8];
        length.copy_from_slice(&remaining[..8]);
        remaining = &remaining[8..];
        let length = usize::try_from(u64::from_be_bytes(length))?;
        anyhow::ensure!(
            length <= remaining.len(),
            "truncated pending-inbound payload"
        );
        parts.push(&remaining[..length]);
        remaining = &remaining[length..];
    }
    anyhow::ensure!(!parts.is_empty(), "empty pending-inbound record");
    Ok(parts)
}
pub(super) fn decode_pending_parts(bytes: &[u8]) -> anyhow::Result<Vec<wa::Message>> {
    pending_parts(bytes)?
        .into_iter()
        .map(|part| waproto::codec::message_decode(part).map_err(Into::into))
        .collect()
}

pub(super) fn extend_pending_record(
    existing: &[u8],
    proposed: &[u8],
) -> anyhow::Result<Option<Vec<u8>>> {
    let original_parts = pending_parts(existing)?;
    let mut scratch = Vec::new();
    // Replay selection already treats SKDM as a carrier, not a new user
    // payload. Extension must use that same occurrence identity while keeping
    // each original row's bytes, including its carrier and unknown fields.
    let existing_fingerprints: Vec<_> = decode_pending_parts(existing)?
        .iter()
        .map(|message| MessageDispatch::fingerprint_into(message, &mut scratch))
        .collect();
    let proposed_parts = pending_parts(proposed)?;
    let mut next = 0;
    let mut merged = Vec::with_capacity(proposed_parts.len());
    for part in proposed_parts {
        let matches = if let Some(expected) = existing_fingerprints.get(next) {
            let message = waproto::codec::message_decode(part)?;
            *expected == MessageDispatch::fingerprint_into(&message, &mut scratch)
        } else {
            false
        };
        if matches {
            merged.push(original_parts[next]);
            next += 1;
        } else {
            merged.push(part);
        }
    }
    anyhow::ensure!(
        next == original_parts.len(),
        "pending identity conflicts with retained payloads"
    );
    if merged.len() == original_parts.len() {
        return Ok(None);
    }
    // Every existing part stays byte-for-byte intact; only additional parts
    // extend the row. No parallel DTO or table migration is involved.
    Ok(Some(encode_pending_parts(&merged)))
}

fn is_subsequence(sequence: &[DispatchFingerprint], candidate: &[DispatchFingerprint]) -> bool {
    let mut remaining = sequence;
    for fingerprint in candidate {
        if remaining.first() == Some(fingerprint) {
            remaining = &remaining[1..];
        }
    }
    remaining.is_empty()
}

pub(super) struct PendingReplay {
    pub(super) items: Vec<InboundMessage>,
    pub(super) keys: Vec<(String, String, String)>,
}

// Keep the string comparisons out of the stable sort's generated inner loops.
#[inline(never)]
fn compare_pending_senders(a: &str, b: &str, preferred: &str) -> std::cmp::Ordering {
    (a != preferred, a).cmp(&(b != preferred, b))
}

impl Client {
    /// Load all matching original keys before decrypting another delivery.
    /// Other namespaces/participants are filtered before decoding; their rows
    /// cannot block or be removed by this consumer's successful commit.
    pub(super) async fn load_pending_inbound(
        &self,
        info: &Arc<MessageInfo>,
    ) -> anyhow::Result<PendingReplay> {
        let backend = self.persistence_manager.backend();
        let chat = info.source.chat.to_string();
        let sender = info.source.sender.to_string();
        let group = info.source.chat.is_group()
            || info.source.chat.is_broadcast_list()
            || info.source.chat.is_status_broadcast();
        let mut rows = if group {
            backend
                .get_pending_inbound_for_message(&chat, &info.id)
                .await?
        } else {
            backend
                .get_pending_inbound(&chat, &sender, &info.id)
                .await?
                .map(|bytes| (sender.clone(), bytes))
                .into_iter()
                .collect()
        };
        rows.retain(|(stored_sender, _)| {
            stored_sender == &sender
                || group
                    && stored_sender
                        .parse::<Jid>()
                        .is_ok_and(|stored| stored.to_non_ad() == info.source.sender.to_non_ad())
        });
        // Prefer the exact spelling when recorded sequences are equivalent.
        rows.sort_by(|a, b| compare_pending_senders(&a.0, &b.0, &sender));
        let mut replay = PendingReplay {
            items: Vec::new(),
            keys: Vec::new(),
        };
        let mut sequences = Vec::new();
        let mut candidates = Vec::new();
        let mut scratch = Vec::new();
        for (stored_sender, bytes) in rows {
            let items: Vec<_> = decode_pending_parts(&bytes)?
                .into_iter()
                .map(|message| {
                    InboundMessage::builder()
                        .message(Arc::new(message))
                        .info(Arc::clone(info))
                        .build()
                })
                .collect();
            sequences.push(
                items
                    .iter()
                    .map(|item| MessageDispatch::fingerprint_into(&item.message, &mut scratch))
                    .collect::<Vec<_>>(),
            );
            candidates.push(items);
            replay
                .keys
                .push((chat.clone(), stored_sender, info.id.to_string()));
        }
        // Prefer an already-recorded sequence containing every row. An exact
        // alias can be only a suffix: [B] must not make stored [A,B] become [B,A].
        // Compare occurrences so [A,A,B] still contains two copies of A.
        if let Some(complete) = sequences.iter().position(|candidate| {
            sequences
                .iter()
                .all(|sequence| is_subsequence(sequence, candidate))
        }) {
            replay.items = candidates.swap_remove(complete);
        } else {
            for items in candidates {
                retention::merge_parts(&mut replay.items, items);
            }
            let canonical: Vec<_> = replay
                .items
                .iter()
                .map(|item| MessageDispatch::fingerprint_into(&item.message, &mut scratch))
                .collect();
            anyhow::ensure!(
                sequences
                    .iter()
                    .all(|sequence| is_subsequence(sequence, &canonical)),
                "conflicting pending-inbound part order across sender keys"
            );
        }
        Ok(replay)
    }

    /// The registered inbound durability hook, if any. `None` (default) keeps
    /// the existing at-most-once acknowledgement path.
    pub(crate) fn inbound_durability_hook(&self) -> Option<Arc<dyn InboundDurabilityHook>> {
        self.inbound_durability_hook.get().cloned()
    }

    /// Redelivery path: when the server replays an already-decrypted message
    /// (`DuplicatedMessage`), re-commit it from the buffered copy instead of
    /// acking. The replay routes through the commit batcher: during a drain it
    /// joins the accumulating batch, so its hook commit, ack and event keep
    /// arrival order with the fresh stanzas around it; live it commits
    /// immediately with all stored parts. A successful batch commit clears its
    /// pending row, and consumers observe the message there.
    /// Usually its original batch never dispatched (the hook failed then); if
    /// it did (post-commit row cleanup failed AND the ack was lost), event
    /// consumers can see it twice, so commits must tolerate duplicate
    /// `Event::Messages` with a hook registered. A plain ack is sent only for
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
            if self.inbound_commit_batch.retention.has_plaintext(info) {
                if let Some(items) = self.inbound_commit_batch.retention.replay_items(info) {
                    return !matches!(
                        self.commit_or_batch_inbound_items(items, false).await,
                        InboundCommitState::Failed
                    );
                }
                // Its complete in-memory stanza will commit at the end of this
                // receive, even when a storage failure left no pending row.
                return false;
            }
            match self.load_pending_inbound(info).await {
                Ok(replay) if !replay.items.is_empty() => {
                    if self.inbound_commit_batch.retention.is_collecting(info) {
                        self.inbound_commit_batch.retention.seed_replay(replay);
                        return false; // The outer producer seals every part together.
                    }
                    self.inbound_commit_batch.retention.begin(info, None).await;
                    let _collection = self.inbound_commit_batch.retention.collection_guard(info);
                    self.inbound_commit_batch.retention.seed_replay(replay);
                    let (items, _) = self
                        .inbound_commit_batch
                        .retention
                        .seal(info, self.inbound_commit_batch.is_active());
                    return !matches!(
                        self.commit_or_batch_inbound_items(items, false).await,
                        InboundCommitState::Failed
                    );
                }
                Ok(_) => {
                    if !self.inbound_commit_batch.retention.has_plaintext(info) {
                        self.ack_received_message(info);
                    }
                }
                Err(error) => {
                    log::warn!(
                        "Pending inbound lookup failed; preserving all records and withholding receipt: {error:?}"
                    );
                }
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

    #[test]
    fn pending_record_reads_legacy_and_preserves_opaque_parts() {
        // Framing never interprets protobuf tags or enum values.
        let unknown = [0xc0, 0x3e, 7];
        assert_eq!(pending_parts(&unknown).unwrap(), [&unknown[..]]);
        let opaque = encode_pending_parts(&[&unknown, &[], &unknown]);
        assert_eq!(
            pending_parts(&opaque).unwrap(),
            [&unknown[..], &[], &unknown[..]]
        );
        let wire = [10, 1, b'x'];
        let legacy = decode_pending_parts(&wire).unwrap();
        let record = encode_pending_parts(&[&wire, &[], &wire]);
        let decoded = decode_pending_parts(&record).unwrap();
        assert_eq!(decoded.len(), 3);
        for message in [&legacy[0], &decoded[0], &decoded[2]] {
            let mut encoded = Vec::new();
            waproto::codec::message_encode_into(message, &mut encoded);
            assert_eq!(encoded, wire);
        }
        assert!(decode_pending_parts(PARTS_HEADER).is_err());
        assert!(decode_pending_parts(b"\0WAPI\x02").is_err());
        assert!(decode_pending_parts(&record[..record.len() - 1]).is_err());
        let mut oversized = PARTS_HEADER.to_vec();
        oversized.extend_from_slice(&u64::MAX.to_be_bytes());
        assert!(decode_pending_parts(&oversized).is_err());
    }

    #[test]
    fn pending_extension_uses_replay_carrier_equivalence_and_keeps_bytes() {
        let original = [10, 1, b'b', 0xc0, 0x3e, 7];
        let with_carrier = [&original[..], &[0x12, 0]].concat();
        let first = [10, 1, b'a'];
        let proposed = encode_pending_parts(&[&first, &with_carrier]);
        let extended = extend_pending_record(&original, &proposed)
            .unwrap()
            .unwrap();
        assert_eq!(
            pending_parts(&extended).unwrap(),
            [&first[..], &original[..]]
        );
        assert!(
            extend_pending_record(&original, &with_carrier)
                .unwrap()
                .is_none()
        );
        let duplicate = encode_pending_parts(&[&original, &original]);
        assert!(extend_pending_record(&duplicate, &proposed).is_err());
        assert!(extend_pending_record(&original, &first).is_err());
    }

    #[tokio::test]
    async fn restarted_alias_replay_with_carrier_difference_reaches_hook() {
        let client = create_test_client_with_failing_http("durability_carrier_alias").await;
        let mut info = (*test_info("CARRIER_ALIAS")).clone();
        info.source.is_group = true;
        let info = Arc::new(info);
        let backend = client.persistence_manager.backend();
        let original = [10, 1, b'b', 0xc0, 0x3e, 7];
        let with_carrier = [&original[..], &[0x12, 0]].concat();
        let combined = encode_pending_parts(&[&[10, 1, b'a'], &with_carrier]);
        for (sender, bytes) in [
            ("200@s.whatsapp.net", &original[..]),
            ("200:1@s.whatsapp.net", &combined[..]),
        ] {
            backend
                .store_pending_inbound("100@g.us", sender, &info.id, bytes)
                .await
                .unwrap();
        }
        let hook = counting_hook(true);
        assert!(client.inbound_durability_hook.set(hook.clone()).is_ok());
        assert!(client.ack_or_replay_to_hook(&info).await);
        assert_eq!(hook.calls.load(Ordering::SeqCst), 1);
        assert_eq!(hook.messages.load(Ordering::SeqCst), 2);
        for sender in ["200@s.whatsapp.net", "200:1@s.whatsapp.net"] {
            assert!(
                backend
                    .get_pending_inbound("100@g.us", sender, &info.id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    #[test]
    fn extending_a_pending_record_keeps_legacy_bytes_and_part_multiplicity() {
        let original = [10, 1, b'x', 0xc0, 0x3e, 7];
        let mut canonical = Vec::new();
        waproto::codec::message_encode_into(
            &decode_pending_parts(&original).unwrap()[0],
            &mut canonical,
        );
        let proposed = encode_pending_parts(&[&canonical, &[10, 1, b'y'], &[10, 1, b'y']]);
        let extended = extend_pending_record(&original, &proposed)
            .unwrap()
            .unwrap();
        assert_eq!(
            pending_parts(&extended).unwrap(),
            [&original[..], &[10, 1, b'y'], &[10, 1, b'y']]
        );
        assert!(extend_pending_record(&original, &[10, 1, b'y']).is_err());
        assert!(
            extend_pending_record(&original, &canonical)
                .unwrap()
                .is_none()
        );
    }

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
                let mut proto = wa::Message::default();
                proto.conversation = Some("hello".to_string());
                proto
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
                        let mut proto = wa::Message::default();
                        proto.conversation = Some("hello".to_string());
                        proto
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
    async fn pending_replay_preserves_distinct_parts_with_the_same_identity() {
        let client = create_test_client_with_failing_http("durability_parts").await;
        let hook = counting_hook(false);
        let _ = client.inbound_durability_hook.set(hook.clone());
        let first = test_item("MULTIPART");
        let info = Arc::clone(&first.info);
        let mut message = wa::Message::default();
        message.conversation = Some("second distinct part".to_owned());
        let second = InboundMessage::builder()
            .message(Arc::new(message))
            .info(Arc::clone(&info))
            .build();
        assert!(
            client
                .commit_inbound_batch(Arc::from([first, second]), BatchOrigin::OfflineDrain, None,)
                .await
        );
        assert_eq!(hook.messages.swap(0, Ordering::SeqCst), 2);
        hook.succeed.store(true, Ordering::SeqCst);
        assert!(client.ack_or_replay_to_hook(&info).await);
        assert_eq!(
            hook.messages.load(Ordering::SeqCst),
            2,
            "replay must retain every distinct part of one message identity"
        );
    }

    #[tokio::test]
    async fn failed_replay_preserves_original_legacy_wire_bytes() {
        let client = create_test_client_with_failing_http("durability_legacy_wire").await;
        let _ = client.inbound_durability_hook.set(counting_hook(false));
        let info = test_info("LEGACY_WIRE");
        // A known conversation and an opaque unknown field in an existing row.
        let original = [10, 1, b'x', 0xc0, 0x3e, 7];
        let backend = client.persistence_manager.backend();
        backend
            .store_pending_inbound(
                &info.source.chat.to_string(),
                &info.source.sender.to_string(),
                &info.id,
                &original,
            )
            .await
            .unwrap();
        client.ack_or_replay_to_hook(&info).await;
        assert_eq!(
            backend
                .get_pending_inbound(
                    &info.source.chat.to_string(),
                    &info.source.sender.to_string(),
                    &info.id
                )
                .await
                .unwrap()
                .unwrap(),
            original
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
