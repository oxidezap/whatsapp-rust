//! Inbound commit batcher: accumulates decrypted messages during the offline
//! drain and commits them per batch — pending-inbound buffer, Signal-cache
//! flush, durability hook, event dispatch and acks all amortize over the
//! batch. Mirrors WA Web's `MessageProcessorCache` (`createSnapshot`: bulk
//! message-table write → bulk signal-store commit → aggregate receipts), with
//! the same flush triggers: batch size, timeout, and end-of-drain.
//!
//! Live messages bypass accumulation and commit as a batch of one, which is
//! also WA Web behavior (the same pipeline with an immediate flush).

use super::*;
use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;
use wacore::store::traits::{PendingInboundKey, PendingInboundRow};
use wacore::types::events::{BatchOrigin, InboundMessage, MessageBatch};

/// WA Web pulls the offline backlog in server batches of 200
/// (`DEFAULT_MAX_BATCH_SIZE`); one commit per server batch is the natural
/// granularity.
const MAX_BATCH_MESSAGES: usize = 200;
/// Byte cap so a media-heavy backlog cannot hold multi-MB protos in memory;
/// WA Web caps by count only, we are stricter.
const MAX_BATCH_BYTES: usize = 4 * 1024 * 1024;
/// WA Web's offline pre-ack batcher uses `delayMs: 3000`; the message cache
/// timeout is an AB prop of the same magnitude.
const FLUSH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Default)]
struct BatchState {
    entries: Vec<InboundMessage>,
    /// Sum of `message_encoded_len` of the entries, for the byte cap.
    bytes: usize,
    timer_armed: bool,
}

pub(crate) struct InboundCommitBatcher {
    state: std::sync::Mutex<BatchState>,
    /// Bumped on every take; a timer that observes a stale epoch stands down.
    epoch: AtomicU64,
    /// Serializes commit sequences so batches reach the hook in accumulation
    /// order. The guard doubles as the reusable encode arena.
    arena: async_lock::Mutex<Vec<u8>>,
}

impl Default for InboundCommitBatcher {
    fn default() -> Self {
        Self {
            state: std::sync::Mutex::new(BatchState::default()),
            epoch: AtomicU64::new(0),
            arena: async_lock::Mutex::new(Vec::new()),
        }
    }
}

impl InboundCommitBatcher {
    fn lock(&self) -> std::sync::MutexGuard<'_, BatchState> {
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poison) => poison.into_inner(),
        }
    }

    /// Take the accumulated batch, invalidating any armed timer.
    fn take(&self) -> Vec<InboundMessage> {
        let mut state = self.lock();
        self.epoch.fetch_add(1, Ordering::AcqRel);
        state.bytes = 0;
        state.timer_armed = false;
        std::mem::take(&mut state.entries)
    }

    /// Drop accumulated entries without committing (connection teardown).
    /// Uncommitted messages were never acked, so the server redelivers them.
    pub(crate) fn clear(&self) {
        let dropped = self.take();
        if !dropped.is_empty() {
            log::debug!(
                "Dropping {} uncommitted inbound messages; the server will redeliver them",
                dropped.len()
            );
        }
    }
}

impl Client {
    /// Queue a decrypted message for the next batch commit. Returns the armed
    /// timer epoch when this push started a fresh batch (the caller spawns the
    /// timeout flush), `None` otherwise.
    fn enqueue_inbound_commit(&self, item: InboundMessage) -> Option<u64> {
        let batcher = &self.inbound_commit_batch;
        let mut state = batcher.lock();
        state.bytes += waproto::codec::message_encoded_len(&item.message);
        state.entries.push(item);
        if !state.timer_armed {
            state.timer_armed = true;
            Some(batcher.epoch.load(Ordering::Acquire))
        } else {
            None
        }
    }

    /// Batch a message decrypted while the offline drain is active, or commit
    /// immediately (batch of one) on the live path.
    pub(crate) async fn commit_or_batch_inbound(self: &Arc<Self>, item: InboundMessage) {
        if self.offline_sync_completed.load(Ordering::Relaxed) {
            self.commit_inbound_batch(vec![item], BatchOrigin::Live, false)
                .await;
            return;
        }
        if let Some(epoch) = self.enqueue_inbound_commit(item) {
            let client = self.clone();
            self.runtime
                .spawn(Box::pin(async move {
                    client.runtime.sleep(FLUSH_TIMEOUT).await;
                    if client.inbound_commit_batch.epoch.load(Ordering::Acquire) == epoch {
                        client.flush_inbound_commits_acquiring_permit().await;
                    }
                }))
                .detach();
        }
    }

    /// Size/byte-cap check, run at the end of stanza processing while the
    /// global processing permit is still held (so the Signal flush inside the
    /// commit cannot interleave with a half-processed stanza).
    pub(crate) async fn maybe_flush_inbound_commits(self: &Arc<Self>) {
        let over = {
            let state = self.inbound_commit_batch.lock();
            state.entries.len() >= MAX_BATCH_MESSAGES || state.bytes >= MAX_BATCH_BYTES
        };
        if over {
            let batch = self.inbound_commit_batch.take();
            self.commit_inbound_batch(batch, BatchOrigin::OfflineDrain, true)
                .await;
        }
    }

    /// Flush after acquiring a global processing permit, so no stanza is
    /// mid-decrypt when the Signal cache is flushed: a crash could otherwise
    /// persist a ratchet advance for a message no batch has committed, turning
    /// its redelivery into an unrecoverable duplicate. During the drain the
    /// semaphore holds a single permit, so this fully serializes with stanza
    /// processing; after the drain the batcher is empty and this no-ops.
    pub(crate) async fn flush_inbound_commits_acquiring_permit(self: &Arc<Self>) {
        let _permit = loop {
            let (generation, semaphore) = self.read_message_semaphore();
            let permit = semaphore.acquire_arc().await;
            if generation
                == self
                    .message_semaphore_generation
                    .load(std::sync::atomic::Ordering::SeqCst)
            {
                break permit;
            }
            drop(permit);
        };
        let batch = self.inbound_commit_batch.take();
        if batch.is_empty() {
            return;
        }
        self.commit_inbound_batch(batch, BatchOrigin::OfflineDrain, true)
            .await;
    }

    /// Commit one batch: durable buffer → Signal flush → hook → clear buffer →
    /// event → acks. WA Web ordering (`createSnapshot`), so nothing is acked or
    /// observable before it is durable. On any commit failure everything stays
    /// unacked and the server redelivers the whole batch.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.commit_batch", level = "debug", skip_all, fields(count = items.len())))]
    pub(crate) async fn commit_inbound_batch(
        self: &Arc<Self>,
        items: Vec<InboundMessage>,
        origin: BatchOrigin,
        flush_signal: bool,
    ) {
        if items.is_empty() {
            return;
        }

        if let Some(hook) = self.inbound_durability_hook() {
            // Key strings live for the whole commit; rows borrow them and the
            // encode arena, so the batch write allocates nothing per row
            // beyond these.
            let keys: Vec<(String, String)> = items
                .iter()
                .map(|m| {
                    (
                        m.info.source.chat.to_string(),
                        m.info.source.sender.to_string(),
                    )
                })
                .collect();

            let mut arena = self.inbound_commit_batch.arena.lock().await;
            arena.clear();
            let mut ranges = Vec::with_capacity(items.len());
            for item in &items {
                let start = arena.len();
                waproto::codec::message_encode_into(&item.message, &mut arena);
                ranges.push(start..arena.len());
            }
            let rows: Vec<PendingInboundRow<'_>> = items
                .iter()
                .zip(&keys)
                .zip(&ranges)
                .map(|((item, (chat, sender)), range)| PendingInboundRow {
                    chat,
                    sender,
                    id: &item.info.id,
                    message: &arena[range.clone()],
                })
                .collect();

            let backend = self.persistence_manager.backend();
            // Fail closed: without a durable buffered copy, do not run the hook
            // and do not ack — the server redelivers once storage recovers.
            if let Err(e) = backend.store_pending_inbound_batch(&rows).await {
                log::error!(
                    "Failed to buffer inbound batch of {}; suppressing acks for redelivery: {e:?}",
                    items.len()
                );
                return;
            }
            drop(rows);

            if flush_signal {
                self.flush_signal_cache_logged("commit_batch", None).await;
            }

            if let Err(e) = hook.on_messages(self.clone(), &items).await {
                log::warn!(
                    "Inbound durability hook failed for batch of {}; suppressing acks for redelivery: {e:?}",
                    items.len()
                );
                return;
            }

            let delete_keys: Vec<PendingInboundKey<'_>> = items
                .iter()
                .zip(&keys)
                .map(|(item, (chat, sender))| PendingInboundKey {
                    chat,
                    sender,
                    id: &item.info.id,
                })
                .collect();
            if let Err(e) = backend.delete_pending_inbound_batch(&delete_keys).await {
                // Leftover rows replay as duplicates; the idempotent hook
                // re-commits and the replay path clears them.
                log::debug!(
                    "Failed to clear {} buffered inbound messages: {e:?}",
                    delete_keys.len()
                );
            }
        } else if flush_signal {
            self.flush_signal_cache_logged("commit_batch", None).await;
        }

        let batch = MessageBatch {
            messages: items.into(),
            origin,
        };
        self.core.event_bus.dispatch(Event::Messages(batch.clone()));
        for item in batch.messages.iter() {
            self.ack_received_message(&item.info);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_client_with_failing_http;
    use crate::types::durability_hook::InboundDurabilityHook;
    use crate::types::message::{MessageInfo, MessageSource};
    use std::sync::Mutex;
    use wacore::types::events::ChannelEventHandler;

    struct RecordingHook {
        batches: Mutex<Vec<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl InboundDurabilityHook for RecordingHook {
        async fn on_messages(
            &self,
            _client: Arc<Client>,
            batch: &[InboundMessage],
        ) -> anyhow::Result<()> {
            self.batches
                .lock()
                .expect("hook lock")
                .push(batch.iter().map(|m| m.info.id.clone()).collect());
            Ok(())
        }
    }

    fn item(id: &str) -> InboundMessage {
        InboundMessage {
            message: Arc::new(wa::Message {
                conversation: Some(format!("text {id}")),
                ..Default::default()
            }),
            info: Arc::new(MessageInfo {
                id: id.to_string(),
                source: MessageSource {
                    chat: "100@g.us".parse().unwrap(),
                    sender: "200@s.whatsapp.net".parse().unwrap(),
                    ..Default::default()
                },
                ..Default::default()
            }),
        }
    }

    // During the drain, messages accumulate and one flush commits them all in
    // arrival order as a single hook call and a single OfflineDrain event.
    #[tokio::test]
    async fn drain_accumulates_then_commits_in_order() {
        let client = create_test_client_with_failing_http("batch_drain").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);

        client
            .offline_sync_completed
            .store(false, std::sync::atomic::Ordering::Relaxed);
        for id in ["B1", "B2", "B3"] {
            client.commit_or_batch_inbound(item(id)).await;
        }
        assert!(
            hook.batches.lock().expect("hook lock").is_empty(),
            "sub-threshold entries must accumulate, not commit"
        );

        client.flush_inbound_commits_acquiring_permit().await;

        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(batches, vec![vec!["B1", "B2", "B3"]]);

        let event = rx.try_recv().expect("one batch event");
        let batch = event.message_batch().expect("Messages event");
        assert_eq!(batch.origin, BatchOrigin::OfflineDrain);
        let ids: Vec<&str> = batch.messages.iter().map(|m| m.info.id.as_str()).collect();
        assert_eq!(ids, ["B1", "B2", "B3"]);
        assert!(rx.try_recv().is_err(), "exactly one event for the batch");

        // A committed batch leaves no buffered copies behind.
        let backend = client.persistence_manager.backend();
        for id in ["B1", "B2", "B3"] {
            assert!(
                backend
                    .get_pending_inbound("100@g.us", "200@s.whatsapp.net", id)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }

    // Live traffic commits immediately as a batch of one.
    #[tokio::test]
    async fn live_commits_as_batch_of_one() {
        let client = create_test_client_with_failing_http("batch_live").await;
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);
        client
            .offline_sync_completed
            .store(true, std::sync::atomic::Ordering::Relaxed);

        client.commit_or_batch_inbound(item("L1")).await;

        assert_eq!(
            hook.batches.lock().expect("hook lock").clone(),
            vec![vec!["L1"]]
        );
        let event = rx.try_recv().expect("live event");
        let batch = event.message_batch().expect("Messages event");
        assert_eq!(batch.origin, BatchOrigin::Live);
        assert_eq!(batch.messages.len(), 1);
    }

    // The size trigger commits a full batch from the stanza-end check.
    #[tokio::test]
    async fn size_trigger_flushes_full_batch() {
        let client = create_test_client_with_failing_http("batch_size").await;
        client
            .offline_sync_completed
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());

        for i in 0..MAX_BATCH_MESSAGES {
            client.commit_or_batch_inbound(item(&format!("S{i}"))).await;
        }
        client.maybe_flush_inbound_commits().await;

        let batches = hook.batches.lock().expect("hook lock").clone();
        assert_eq!(batches.len(), 1, "one commit for the full batch");
        assert_eq!(batches[0].len(), MAX_BATCH_MESSAGES);
        assert_eq!(batches[0][0], "S0");
        assert_eq!(batches[0][MAX_BATCH_MESSAGES - 1], "S199");
    }

    // Without a hook, the drain still batches the event dispatch.
    #[tokio::test]
    async fn drain_without_hook_batches_events() {
        let client = create_test_client_with_failing_http("batch_no_hook").await;
        client
            .offline_sync_completed
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);

        client.commit_or_batch_inbound(item("N1")).await;
        client.commit_or_batch_inbound(item("N2")).await;
        client.flush_inbound_commits_acquiring_permit().await;

        let event = rx.try_recv().expect("one batch event");
        assert_eq!(
            event
                .messages()
                .map(|m| m.info.id.as_str())
                .collect::<Vec<_>>(),
            ["N1", "N2"]
        );
    }

    // clear() drops uncommitted entries: no hook call, no event, and the
    // pending buffer was never written (the server redelivers instead).
    #[tokio::test]
    async fn clear_drops_uncommitted_entries() {
        let client = create_test_client_with_failing_http("batch_clear").await;
        client
            .offline_sync_completed
            .store(false, std::sync::atomic::Ordering::Relaxed);
        let hook = Arc::new(RecordingHook {
            batches: Mutex::new(Vec::new()),
        });
        let _ = client.inbound_durability_hook.set(hook.clone());
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);

        client.commit_or_batch_inbound(item("C1")).await;
        client.inbound_commit_batch.clear();
        client.flush_inbound_commits_acquiring_permit().await;

        assert!(hook.batches.lock().expect("hook lock").is_empty());
        assert!(rx.try_recv().is_err());
    }
}
