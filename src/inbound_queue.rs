//! Receive queue budgets cover queued and in-flight backing bytes, including
//! escaped zero-copy slices. Chat payload storage is compacted on admission.
//! Charges are max(1024, encoded node length): 32 MiB across chats, 16 MiB per
//! chat, and 256 KiB separately for encoded ACKs. Channels also bound item count.
//! These are payload budgets, not bounds on total RSS: decoded tree containers,
//! transient frame/decompression buffers, processing outputs, observers and host
//! storage have separate lifetimes. The per-chat byte limit accommodates the
//! existing 16 MiB decompression ceiling.
//! Saturation freezes chat admission and drains accepted work with the reader
//! still serving IQs, then recycles the connection. Drain deadlines are finite;
//! permanently stalled processing cannot be promised progress or delivery.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A non-waiting byte counter. One lease is the only way to return a charge.
/// Unlike a runtime semaphore this also works when tokio-runtime is disabled.
pub(crate) struct ByteBudget(AtomicUsize);
impl ByteBudget {
    pub(crate) fn new(bytes: usize) -> Self {
        Self(AtomicUsize::new(bytes))
    }
    pub(crate) fn available(&self) -> usize {
        self.0.load(Ordering::Relaxed)
    }
}
pub(crate) struct ByteLease {
    budget: Arc<ByteBudget>,
    bytes: usize,
}
impl Drop for ByteLease {
    fn drop(&mut self) {
        self.budget.0.fetch_add(self.bytes, Ordering::Relaxed);
    }
}

/// Channel storage outlives its worker while a cached sender exists. Close and
/// drain it even if the worker future is dropped before its first poll.
pub(crate) struct DrainingReceiver<T>(pub async_channel::Receiver<T>);
impl<T> std::ops::Deref for DrainingReceiver<T> {
    type Target = async_channel::Receiver<T>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> Drop for DrainingReceiver<T> {
    fn drop(&mut self) {
        self.0.close();
        while self.0.try_recv().is_ok() {}
    }
}

pub(crate) const CHAT_QUEUE_ITEMS: usize = 64;
pub(crate) const CHAT_BYTES: usize = 16 * 1024 * 1024;
const GLOBAL_CHAT_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const ACK_QUEUE_ITEMS: usize = 256;
const ACK_BYTES: usize = 256 * 1024;

pub(crate) struct QueueBudgets {
    pub chat: Arc<ByteBudget>,
    pub ack: Arc<ByteBudget>,
    pub work: Arc<crate::flush_scope::FlushScope>,
    pub draining: std::sync::atomic::AtomicBool,
}
impl Default for QueueBudgets {
    fn default() -> Self {
        Self {
            chat: Arc::new(ByteBudget::new(GLOBAL_CHAT_BYTES)),
            ack: Arc::new(ByteBudget::new(ACK_BYTES)),
            work: Arc::default(),
            draining: false.into(),
        }
    }
}

pub(crate) fn reserve(budget: &Arc<ByteBudget>, bytes: usize) -> Option<ByteLease> {
    // A minimum charge also bounds the number of tiny queued or in-flight nodes.
    let charge = bytes.max(1024);
    budget
        .0
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |available| {
            available.checked_sub(charge)
        })
        .ok()?;
    Some(ByteLease {
        budget: Arc::clone(budget),
        bytes: charge,
    })
}

/// The permits belong to the backing bytes, including slices handed to consumers.
/// Releasing a worker or cancelling its future cannot release them prematurely.
struct RetainedBytes {
    bytes: bytes::Bytes,
    _global: ByteLease,
    _chat: Option<ByteLease>,
    _liveness: Option<Arc<async_lock::Mutex<()>>>,
}
impl AsRef<[u8]> for RetainedBytes {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

pub(crate) fn retain_node(
    node: &wacore_binary::OwnedNodeRef,
    global: &Arc<ByteBudget>,
    chat: &Arc<ByteBudget>,
    liveness: Arc<async_lock::Mutex<()>>,
) -> Option<wacore_binary::OwnedNodeRef> {
    let bytes = node.backing_bytes();
    let owner = RetainedBytes {
        _global: reserve(global, bytes.len())?,
        _chat: Some(reserve(chat, bytes.len())?),
        _liveness: Some(liveness),
        // A decoded frame may be a small view into a larger transport slab.
        // Compact only after admission so the charged length bounds retained
        // payload storage rather than just the visible part of that slab.
        bytes: bytes::Bytes::copy_from_slice(&bytes),
    };
    // Reparse the borrowed containers without unsafe cart replacement. Every
    // descendant Bytes view retains both leases.
    wacore_binary::OwnedNodeRef::new(bytes::Bytes::from_owner(owner)).ok()
}

pub(crate) struct QueuedAck {
    pub bytes: bytes::Bytes,
    pub flush: crate::flush_scope::FlushGuard,
    pub shutdown: wacore::runtime::ShutdownSignal,
}

pub(crate) fn retain_ack(bytes: Vec<u8>, lease: ByteLease) -> bytes::Bytes {
    bytes::Bytes::from_owner(RetainedBytes {
        bytes: bytes.into(),
        _global: lease,
        _chat: None,
        _liveness: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{ChatLane, QueuedChatMessage};
    use async_lock::Mutex;
    use wacore_binary::builder::NodeBuilder;

    fn lane(
        global: &Arc<ByteBudget>,
        bytes: usize,
        items: usize,
    ) -> (ChatLane, DrainingReceiver<QueuedChatMessage>) {
        let (queue_tx, rx) = async_channel::bounded(items);
        (
            ChatLane {
                queue_tx,
                enqueue_lock: Arc::new(Mutex::new(())),
                worker_running: Arc::new(Mutex::new(())),
                byte_budget: Arc::new(ByteBudget::new(bytes)),
                global_budget: Arc::clone(global),
                work: Arc::default(),
            },
            DrainingReceiver(rx),
        )
    }
    fn node(id: usize, bytes: usize) -> Arc<wacore_binary::OwnedNodeRef> {
        crate::test_utils::node_to_owned_ref(
            &NodeBuilder::new("message")
                .attr("from", "120363000000042@g.us")
                .attr("id", id.to_string())
                .bytes(vec![id as u8; bytes])
                .build(),
        )
    }

    #[test]
    fn blocked_workers_cannot_accumulate_unbounded_payloads() {
        let (old_tx, old_rx) = async_channel::unbounded();
        for id in 0..128 {
            old_tx.try_send(node(id, 64 * 1024)).unwrap();
        }
        assert_eq!(old_tx.capacity(), None);
        let baseline_payload: usize = (0..128)
            .map(|_| old_rx.try_recv().unwrap().content_bytes().unwrap().len())
            .sum();
        assert_eq!(baseline_payload, 8 * 1024 * 1024);
        eprintln!(
            "baseline: 128 distinct payloads, {baseline_payload} payload bytes, capacity None"
        );
        let global = Arc::new(ByteBudget::new(1024 * 1024));
        let (lane, rx) = lane(&global, 256 * 1024, CHAT_QUEUE_ITEMS);
        let mut accepted = 0;
        for id in 0..128 {
            if lane.try_enqueue(node(id, 64 * 1024)).is_ok() {
                accepted += 1;
            }
        }
        assert_eq!(accepted, 3); // encoded attributes count against the byte limit too
        assert_eq!(rx.len(), 3);
        eprintln!(
            "bounded: {accepted} distinct payloads, {} charged bytes",
            1024 * 1024 - global.available()
        );
        assert!(global.available() >= 768 * 1024);
        drop(rx);
        assert_eq!(global.available(), 1024 * 1024);
    }

    #[test]
    fn in_flight_nodes_and_escaped_slices_keep_their_charge() {
        let global = Arc::new(ByteBudget::new(2048));
        let (lane, rx) = lane(&global, 2048, 1);
        lane.try_enqueue(node(1, 4)).unwrap();
        let queued = rx.try_recv().unwrap();
        let slice = queued
            .node
            .slice_bytes(queued.node.content_bytes().unwrap());
        let clone = Arc::clone(&queued.node);
        drop(queued);
        assert_eq!(global.available(), 1024);
        drop(clone);
        assert_eq!(global.available(), 1024);
        drop(slice);
        assert_eq!(global.available(), 2048);
    }

    #[test]
    fn busy_chat_leaves_capacity_for_another_chat_and_recovers_in_order() {
        let global = Arc::new(ByteBudget::new(4096));
        let (busy, busy_rx) = lane(&global, 2048, 2);
        let (other, other_rx) = lane(&global, 2048, 2);
        busy.try_enqueue(node(1, 4)).unwrap();
        busy.try_enqueue(node(2, 4)).unwrap();
        assert!(matches!(
            busy.try_enqueue(node(3, 4)),
            Err(async_channel::TrySendError::Full(_))
        ));
        other.try_enqueue(node(4, 4)).unwrap();
        let first = busy_rx.try_recv().unwrap();
        assert_eq!(
            first.node.attrs().optional_string("id").as_deref(),
            Some("1")
        );
        assert!(
            busy.try_enqueue(node(3, 4)).is_err(),
            "in-flight ownership still counts"
        );
        drop(first);
        busy.try_enqueue(node(3, 4)).unwrap();
        for id in ["2", "3"] {
            let queued = busy_rx.try_recv().unwrap();
            assert_eq!(
                queued.node.attrs().optional_string("id").as_deref(),
                Some(id)
            );
        }
        drop(other_rx);
        assert_eq!(global.available(), 4096);
    }

    #[test]
    fn item_limit_and_closed_lane_fail_without_leaking_or_double_charging() {
        let global = Arc::new(ByteBudget::new(4096));
        let (lane, rx) = lane(&global, 4096, 1);
        lane.try_enqueue(node(1, 4)).unwrap();
        assert!(lane.try_enqueue(node(2, 4)).is_err());
        assert_eq!(global.available(), 3072);
        rx.close();
        assert!(matches!(
            lane.try_enqueue(node(3, 4)),
            Err(async_channel::TrySendError::Closed(_))
        ));
        drop(rx);
        assert_eq!(global.available(), 4096);
    }

    #[test]
    fn global_budget_applies_across_lanes_and_reservation_failure_rolls_back() {
        let global = Arc::new(ByteBudget::new(2048));
        let (a, a_rx) = lane(&global, 4096, 8);
        let (b, b_rx) = lane(&global, 4096, 8);
        a.try_enqueue(node(1, 4)).unwrap();
        b.try_enqueue(node(2, 4)).unwrap();
        assert!(a.try_enqueue(node(3, 4)).is_err());
        drop(a_rx);
        b.try_enqueue(node(3, 4)).unwrap();
        drop(b_rx);
        assert_eq!(global.available(), 2048);
        let empty = Arc::new(ByteBudget::new(0));
        assert!(retain_node(&node(1, 4), &global, &empty, Arc::default()).is_none());
        assert_eq!(global.available(), 2048);
    }

    #[tokio::test]
    async fn worker_cancelled_before_first_poll_drains_its_cached_sender() {
        let global = Arc::new(ByteBudget::new(2048));
        let (lane, rx) = lane(&global, 2048, 2);
        lane.try_enqueue(node(1, 4)).unwrap();
        let task = tokio::spawn(async move {
            let _rx = rx;
            std::future::pending::<()>().await;
        });
        task.abort();
        let _ = task.await;
        assert!(lane.queue_tx.is_closed());
        assert_eq!(lane.queue_tx.len(), 0);
        assert_eq!(global.available(), 2048);
    }

    #[tokio::test]
    async fn cancellation_releases_owned_nodes_but_ack_bytes_follow_the_sender() {
        let global = Arc::new(ByteBudget::new(2048));
        let (lane, rx) = lane(&global, 2048, 2);
        lane.try_enqueue(node(1, 4)).unwrap();
        let queued = rx.try_recv().unwrap();
        let task = tokio::spawn(async move {
            let _queued = queued;
            std::future::pending::<()>().await;
        });
        task.abort();
        let _ = task.await;
        assert_eq!(global.available(), 2048);
        let ack = retain_ack(vec![1; 64], reserve(&global, 64).unwrap());
        let sender = ack.clone();
        drop(ack);
        assert_eq!(global.available(), 1024);
        drop(sender);
        assert_eq!(global.available(), 2048);
    }
}

impl crate::client::Client {
    /// Freeze admissions, but keep reading IQ replies while accepted work drains.
    /// Disconnecting immediately could replay the same burst forever without
    /// letting even its admitted prefix become durable.
    pub(crate) fn drain_overloaded_inbound(self: &Arc<Self>) {
        use std::sync::atomic::Ordering;
        if self
            .inbound_queue_budgets
            .draining
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        self.inbound_queue_budgets.work.close();
        log::warn!(
            "Inbound queue overloaded; draining before reconnect (chat bytes available: {}, ACK bytes available: {})",
            self.inbound_queue_budgets.chat.available(),
            self.inbound_queue_budgets.ack.available()
        );
        let client = Arc::clone(self);
        let generation = self.connection_generation.load(Ordering::Acquire);
        self.runtime.spawn_detached(Box::pin(async move {
            let grace = std::time::Duration::from_secs(5);
            client
                .inbound_queue_budgets
                .work
                .flush(&*client.runtime, grace)
                .await;
            if client.connection_generation.load(Ordering::Acquire) != generation {
                return;
            }
            client.flush_inbound_commits_bounded(grace).await;
            client.outbound_flush.flush(&*client.runtime, grace).await;
            if client.connection_generation.load(Ordering::Acquire) == generation {
                client.notify_connection_shutdown();
            }
        }));
    }
}

#[cfg(test)]
mod overload_tests {
    use super::*;
    use crate::client::{ChatLane, ResponseWaiter};
    use wacore_binary::builder::NodeBuilder;

    #[tokio::test(start_paused = true)]
    async fn overload_keeps_iq_routing_alive_until_accepted_work_finishes() {
        let client = crate::test_utils::create_test_client().await;
        let work = client.inbound_queue_budgets.work.try_track().unwrap();
        client.drain_overloaded_inbound();
        client.drain_overloaded_inbound(); // one drain per connection
        assert!(client.inbound_queue_budgets.work.try_track().is_none());
        assert!(!client.connection_shutdown_signal().is_fired());
        let (tx, rx) = futures::channel::oneshot::channel();
        client
            .response_waiters_guard()
            .insert("worker-iq".to_owned(), ResponseWaiter::Iq(tx));
        let iq = crate::test_utils::node_to_owned_ref(
            &NodeBuilder::new("iq")
                .attr("type", "result")
                .attr("id", "worker-iq")
                .attr("from", "s.whatsapp.net")
                .build(),
        );
        client
            .process_decrypted_node(Arc::try_unwrap(iq).ok().unwrap())
            .await;
        assert_eq!(rx.await.unwrap().tag(), "iq");
        assert!(!client.connection_shutdown_signal().is_fired());
        drop(work);
        // Wait on the real signal rather than assume a number of scheduler yields.
        wacore::runtime::wait_for_shutdown(&client.connection_shutdown_signal()).await;
        client.reset_connection_shutdown();
        assert!(client.inbound_queue_budgets.work.try_track().is_some());
        assert!(
            !client
                .inbound_queue_budgets
                .draining
                .load(std::sync::atomic::Ordering::Acquire)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn saturation_cancels_admission_without_creating_more_workers() {
        let client = crate::test_utils::create_test_client().await;
        let chat = "120363000000042@g.us".parse().unwrap();
        let (queue_tx, rx) = async_channel::bounded(1);
        let rx = DrainingReceiver(rx);
        let lane = ChatLane {
            queue_tx,
            enqueue_lock: Arc::default(),
            worker_running: Arc::default(),
            byte_budget: Arc::new(ByteBudget::new(CHAT_BYTES)),
            global_budget: Arc::clone(&client.inbound_queue_budgets.chat),
            work: Arc::clone(&client.inbound_queue_budgets.work),
        };
        let node = || {
            crate::test_utils::node_to_owned_ref(
                &NodeBuilder::new("message")
                    .attr("from", "120363000000042@g.us")
                    .attr("id", "pending")
                    .build(),
            )
        };
        lane.try_enqueue(node()).unwrap();
        client.chat_lanes.insert(chat, lane).await;
        let mut cancelled = false;
        crate::handlers::message::MessageHandler::handle_inline(
            Arc::clone(&client),
            node(),
            &mut cancelled,
        )
        .await;
        assert!(cancelled);
        assert!(
            client
                .inbound_queue_budgets
                .draining
                .load(std::sync::atomic::Ordering::Acquire)
        );
        assert!(!client.connection_shutdown_signal().is_fired());
        let other = crate::test_utils::node_to_owned_ref(
            &NodeBuilder::new("message")
                .attr("from", "120363000000043@g.us")
                .attr("id", "later")
                .build(),
        );
        cancelled = false;
        crate::handlers::message::MessageHandler::handle_inline(
            Arc::clone(&client),
            other,
            &mut cancelled,
        )
        .await;
        assert!(cancelled);
        assert!(
            client
                .chat_lanes
                .get(&"120363000000043@g.us".parse().unwrap())
                .await
                .is_none()
        );
        drop(rx);
        wacore::runtime::wait_for_shutdown(&client.connection_shutdown_signal()).await;
    }
}
