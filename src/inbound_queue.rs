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

type BudgetRegistry = std::sync::Mutex<
    std::collections::HashMap<wacore_binary::jid::Jid, std::sync::Weak<ByteBudget>>,
>;

struct BudgetRegistration {
    registry: std::sync::Weak<BudgetRegistry>,
    chat: wacore_binary::jid::Jid,
}

/// A non-waiting byte counter. One lease is the only way to return a charge.
/// Unlike a runtime semaphore this also works when tokio-runtime is disabled.
pub(crate) struct ByteBudget {
    available: AtomicUsize,
    registration: Option<BudgetRegistration>,
}
impl ByteBudget {
    pub(crate) fn new(bytes: usize) -> Self {
        Self {
            available: AtomicUsize::new(bytes),
            registration: None,
        }
    }
    pub(crate) fn available(&self) -> usize {
        self.available.load(Ordering::Relaxed)
    }
}
impl Drop for ByteBudget {
    fn drop(&mut self) {
        let address = self as *const Self;
        let Some(registration) = &self.registration else {
            return;
        };
        let Some(registry) = registration.registry.upgrade() else {
            return;
        };
        let mut entries = registry.lock().unwrap_or_else(|p| p.into_inner());
        // A failed Weak::upgrade can install a replacement while this destructor
        // waits for the lock. Never remove that new budget's registration.
        if entries
            .get(&registration.chat)
            .is_some_and(|entry| entry.as_ptr() == address)
        {
            entries.remove(&registration.chat);
        }
    }
}

pub(crate) struct ByteLease {
    budget: Arc<ByteBudget>,
    bytes: usize,
}
impl Drop for ByteLease {
    fn drop(&mut self) {
        self.budget
            .available
            .fetch_add(self.bytes, Ordering::Relaxed);
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
    per_chat: Arc<BudgetRegistry>,
    pub draining: std::sync::atomic::AtomicBool,
}
impl Default for QueueBudgets {
    fn default() -> Self {
        Self {
            chat: Arc::new(ByteBudget::new(GLOBAL_CHAT_BYTES)),
            ack: Arc::new(ByteBudget::new(ACK_BYTES)),
            work: Arc::default(),
            per_chat: Arc::default(),
            draining: false.into(),
        }
    }
}
impl QueueBudgets {
    pub(crate) fn memory_stats(&self) -> (wacore::stats::CollectionStats, u64, u64) {
        use wacore::stats::HeapSize;
        let entries = self.per_chat.lock().unwrap_or_else(|p| p.into_inner());
        let registry_bytes = wacore::stats::hash_table_bytes(
            entries.capacity(),
            size_of::<(wacore_binary::jid::Jid, std::sync::Weak<ByteBudget>)>(),
        ) + entries
            .keys()
            .map(|chat| {
                // The map and live budget each own the chat key. Include the budget
                // allocation and Arc header without upgrading/dropping under this lock.
                2 * chat.heap_bytes() + size_of::<ByteBudget>() + 2 * size_of::<usize>()
            })
            .sum::<usize>()
            + size_of::<BudgetRegistry>()
            + 2 * size_of::<usize>();
        (
            wacore::stats::CollectionStats::new(entries.len() as u64, registry_bytes as u64),
            GLOBAL_CHAT_BYTES.saturating_sub(self.chat.available()) as u64,
            ACK_BYTES.saturating_sub(self.ack.available()) as u64,
        )
    }

    pub(crate) fn chat_budget(&self, chat: &wacore_binary::jid::Jid) -> Arc<ByteBudget> {
        self.chat_budget_with_limit(chat, CHAT_BYTES)
    }

    fn chat_budget_with_limit(
        &self,
        chat: &wacore_binary::jid::Jid,
        bytes: usize,
    ) -> Arc<ByteBudget> {
        let mut entries = self.per_chat.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(budget) = entries.get(chat).and_then(std::sync::Weak::upgrade) {
            return budget;
        }
        // Only live lanes/byte leases own a strong reference. Their final drop
        // removes this weak entry, so sequential chat churn leaves no tombstones.
        let budget = Arc::new(ByteBudget {
            available: AtomicUsize::new(bytes),
            registration: Some(BudgetRegistration {
                registry: Arc::downgrade(&self.per_chat),
                chat: chat.clone(),
            }),
        });
        entries.insert(chat.clone(), Arc::downgrade(&budget));
        budget
    }
}

pub(crate) fn reserve(budget: &Arc<ByteBudget>, bytes: usize) -> Option<ByteLease> {
    // A minimum charge also bounds the number of tiny queued or in-flight nodes.
    let charge = bytes.max(1024);
    budget
        .available
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

impl crate::client::Client {
    /// Freeze admissions, but keep reading IQ replies while accepted work drains.
    /// Disconnecting immediately could replay the same burst forever without
    /// letting even its admitted prefix become durable.
    pub(crate) fn drain_overloaded_inbound(
        self: &Arc<Self>,
        shutdown: &wacore::runtime::ShutdownSignal,
    ) {
        // Serialize the scope check and admission close with notifier replacement.
        // reset_connection_shutdown fires the old signal under this same lock.
        {
            let _connection = self
                .connection_shutdown
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if shutdown.is_fired()
                || self
                    .inbound_queue_budgets
                    .draining
                    .swap(true, Ordering::AcqRel)
            {
                return;
            }
            self.inbound_queue_budgets.work.close();
        }
        log::warn!(
            "Inbound queue overloaded; draining before reconnect (chat bytes available: {}, ACK bytes available: {})",
            self.inbound_queue_budgets.chat.available(),
            self.inbound_queue_budgets.ack.available()
        );
        let client = Arc::clone(self);
        let shutdown = shutdown.clone();
        self.runtime.spawn_detached(Box::pin(async move {
            let grace = std::time::Duration::from_secs(5);
            client
                .inbound_queue_budgets
                .work
                .flush(&*client.runtime, grace)
                .await;
            if shutdown.is_fired() {
                return;
            }
            client.flush_inbound_commits_bounded(grace).await;
            {
                let _connection = client
                    .connection_shutdown
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                if shutdown.is_fired() {
                    return;
                }
                // New receipts/ACKs cannot escape a flush that just observed zero.
                client.outbound_flush.close();
            }
            client.outbound_flush.flush(&*client.runtime, grace).await;
            {
                let connection = client
                    .connection_shutdown
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                if shutdown.is_fired() {
                    return;
                }
                connection.notify();
            }
            client.notify_session_state();
        }));
    }
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
                generation: 0,
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
    fn chat_budget_registry_follows_live_owners_without_weak_tombstones() {
        let budgets = QueueBudgets::default();
        let chat: wacore_binary::jid::Jid = "120363000000042@g.us".parse().unwrap();
        let budget = budgets.chat_budget_with_limit(&chat, 2048);
        let same = budgets.chat_budget(&chat);
        assert!(Arc::ptr_eq(&budget, &same));
        let retained = retain_node(&node(1, 4), &budgets.chat, &budget, Arc::default()).unwrap();
        drop(budget);
        drop(same);
        assert_eq!(budgets.per_chat.lock().unwrap().len(), 1);
        drop(retained);
        assert!(budgets.per_chat.lock().unwrap().is_empty());
        for id in 0..128 {
            let chat = format!("12036300000{id:04}@g.us").parse().unwrap();
            drop(budgets.chat_budget(&chat));
            assert!(budgets.per_chat.lock().unwrap().is_empty());
        }
    }

    #[test]
    fn an_old_budget_destructor_cannot_remove_its_concurrent_replacement() {
        let budgets = QueueBudgets::default();
        let chat: wacore_binary::jid::Jid = "120363000000042@g.us".parse().unwrap();
        let old = budgets.chat_budget(&chat);
        let weak = Arc::downgrade(&old);
        let registry = Arc::clone(&budgets.per_chat);
        let mut entries = registry.lock().unwrap();
        let dropping = std::thread::spawn(move || drop(old));
        while weak.strong_count() != 0 {
            std::thread::yield_now();
        }
        let replacement = Arc::new(ByteBudget {
            available: AtomicUsize::new(CHAT_BYTES),
            registration: Some(BudgetRegistration {
                registry: Arc::downgrade(&registry),
                chat: chat.clone(),
            }),
        });
        entries.insert(chat.clone(), Arc::downgrade(&replacement));
        drop(entries);
        dropping.join().unwrap();
        let found = budgets.chat_budget(&chat);
        assert!(Arc::ptr_eq(&found, &replacement));
        drop(found);
        drop(replacement);
        assert!(registry.lock().unwrap().is_empty());
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
        assert!(
            Arc::strong_count(&lane.enqueue_lock) > 1,
            "escaped slices must pin the per-chat budget against cache eviction"
        );
        drop(slice);
        assert_eq!(Arc::strong_count(&lane.enqueue_lock), 1);
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

#[cfg(test)]
mod overload_tests {
    use super::*;
    use crate::client::{ChatLane, ResponseWaiter};
    use wacore_binary::builder::NodeBuilder;

    #[tokio::test]
    async fn stale_overload_cannot_close_a_reopened_connection() {
        let client = crate::test_utils::create_test_client().await;
        let old = client.connection_shutdown_signal();
        client.reset_connection_shutdown();
        assert!(old.is_fired());
        client.drain_overloaded_inbound(&old);
        assert!(
            !client
                .inbound_queue_budgets
                .draining
                .load(Ordering::Acquire)
        );
        assert!(client.inbound_queue_budgets.work.try_track().is_some());
        assert!(client.outbound_flush.try_track().is_some());
        assert!(!client.connection_shutdown_signal().is_fired());
    }

    #[tokio::test]
    async fn retired_overload_drain_cannot_close_fresh_outbound_admission() {
        let client = crate::test_utils::create_test_client().await;
        let work = client.inbound_queue_budgets.work.try_track().unwrap();
        let old = client.connection_shutdown_signal();
        client.drain_overloaded_inbound(&old);
        while client.inbound_queue_budgets.work.flush_waiters() == 0 {
            tokio::task::yield_now().await;
        }
        client.reset_connection_shutdown();
        client.outbound_flush.reopen();
        drop(work);
        while client.inbound_queue_budgets.work.flush_waiters() != 0 {
            tokio::task::yield_now().await;
        }
        assert!(client.outbound_flush.try_track().is_some());
        assert!(client.inbound_queue_budgets.work.try_track().is_some());
        assert!(!client.connection_shutdown_signal().is_fired());
    }

    #[tokio::test]
    async fn overload_closes_outbound_admission_before_its_final_flush() {
        let client = crate::test_utils::create_test_client().await;
        let outbound = client.outbound_flush.try_track().unwrap();
        let shutdown = client.connection_shutdown_signal();
        client.drain_overloaded_inbound(&shutdown);
        while client.outbound_flush.flush_waiters() == 0 {
            tokio::task::yield_now().await;
        }
        assert!(client.outbound_flush.try_track().is_none());
        assert!(!shutdown.is_fired());
        drop(outbound);
        wacore::runtime::wait_for_shutdown(&shutdown).await;
    }

    #[tokio::test]
    async fn escaped_chat_bytes_keep_the_same_budget_across_connection_cache_clear() {
        let client = crate::test_utils::create_test_client().await;
        let chat: wacore_binary::jid::Jid = "120363000000042@g.us".parse().unwrap();
        let budget = client
            .inbound_queue_budgets
            .chat_budget_with_limit(&chat, 1024);
        let weak = Arc::downgrade(&budget);
        let (queue_tx, rx) = async_channel::bounded(1);
        let rx = DrainingReceiver(rx);
        let lane = ChatLane {
            generation: 0,
            queue_tx,
            enqueue_lock: Arc::default(),
            worker_running: Arc::default(),
            byte_budget: budget,
            global_budget: Arc::clone(&client.inbound_queue_budgets.chat),
            work: Arc::clone(&client.inbound_queue_budgets.work),
        };
        let message = || {
            crate::test_utils::node_to_owned_ref(
                &NodeBuilder::new("message")
                    .attr("from", "120363000000042@g.us")
                    .attr("id", "budget-reconnect")
                    .bytes(vec![1; 4])
                    .build(),
            )
        };
        lane.try_enqueue(message()).unwrap();
        client.chat_lanes.insert(chat.clone(), lane).await;
        let queued = rx.try_recv().unwrap();
        let slice = queued
            .node
            .slice_bytes(queued.node.content_bytes().unwrap());
        drop(queued);
        drop(rx);
        client.notify_connection_shutdown();
        client.chat_lanes.clear().await; // the exact cleanup operation bypassing evict_guard
        let ack_lease = reserve(&client.inbound_queue_budgets.ack, 4).unwrap();
        let report = client.memory_report().await;
        assert_eq!(report.chat_lane_backlog, 0);
        assert_eq!(report.inbound_chat_budgets.entries, 1);
        assert!(report.inbound_chat_budgets.bytes > 0);
        assert_eq!(report.inbound_chat_charged_bytes, 1024);
        assert_eq!(report.inbound_ack_charged_bytes, 1024);
        let total = report.total_estimated_bytes();
        let mut without_queues = report;
        without_queues.inbound_chat_budgets.bytes = 0;
        without_queues.inbound_chat_charged_bytes = 0;
        without_queues.inbound_ack_charged_bytes = 0;
        assert!(total >= without_queues.total_estimated_bytes() + 2048);
        drop(ack_lease);
        client.connection_generation.fetch_add(1, Ordering::AcqRel);
        client.reset_connection_shutdown();
        let mut cancelled = false;
        crate::handlers::message::MessageHandler::handle_inline(
            Arc::clone(&client),
            message(),
            &mut cancelled,
        )
        .await;
        assert!(
            cancelled,
            "a reconnect must not reset bytes retained by this chat"
        );
        let replacement = client.chat_lanes.get(&chat).await.unwrap();
        assert!(Arc::ptr_eq(
            &replacement.byte_budget,
            &weak.upgrade().unwrap()
        ));
        assert_eq!(replacement.byte_budget.available(), 0);
        drop(slice);
        assert_eq!(replacement.byte_budget.available(), 1024);
        let report = client.memory_report().await;
        assert_eq!(report.inbound_chat_charged_bytes, 0);
        assert_eq!(report.inbound_ack_charged_bytes, 0);
    }

    #[tokio::test(start_paused = true)]
    async fn overload_keeps_iq_routing_alive_until_accepted_work_finishes() {
        let client = crate::test_utils::create_test_client().await;
        let work = client.inbound_queue_budgets.work.try_track().unwrap();
        client.drain_overloaded_inbound(&client.connection_shutdown_signal());
        client.drain_overloaded_inbound(&client.connection_shutdown_signal()); // one drain per connection
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
                .load(Ordering::Acquire)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_work_cannot_hold_the_overload_drain_open_forever() {
        let client = crate::test_utils::create_test_client().await;
        let work = client.inbound_queue_budgets.work.try_track().unwrap();
        client.drain_overloaded_inbound(&client.connection_shutdown_signal());
        wacore::runtime::wait_for_shutdown(&client.connection_shutdown_signal()).await;
        assert_eq!(client.inbound_queue_budgets.work.pending(), 1);
        drop(work);
        assert_eq!(client.inbound_queue_budgets.work.pending(), 0);
    }

    #[tokio::test]
    async fn a_waiting_admission_cannot_cross_a_reconnect() {
        let client = crate::test_utils::create_test_client().await;
        let (queue_tx, rx) = async_channel::bounded(1);
        let lane = ChatLane {
            generation: 0,
            queue_tx,
            enqueue_lock: Arc::default(),
            worker_running: Arc::default(),
            byte_budget: Arc::new(ByteBudget::new(CHAT_BYTES)),
            global_budget: Arc::clone(&client.inbound_queue_budgets.chat),
            work: Arc::clone(&client.inbound_queue_budgets.work),
        };
        let lock = Arc::clone(&lane.enqueue_lock);
        let guard = lock.lock().await;
        client
            .chat_lanes
            .insert("120363000000042@g.us".parse().unwrap(), lane)
            .await;
        let node = crate::test_utils::node_to_owned_ref(
            &NodeBuilder::new("message")
                .attr("from", "120363000000042@g.us")
                .attr("id", "old")
                .build(),
        );
        let mut cancelled = false;
        let future = crate::handlers::message::MessageHandler::handle_inline(
            Arc::clone(&client),
            node,
            &mut cancelled,
        );
        let mut future = Box::pin(future);
        assert!(futures::poll!(&mut future).is_pending());
        client.notify_connection_shutdown();
        client.reset_connection_shutdown();
        client.connection_generation.fetch_add(1, Ordering::AcqRel);
        drop(guard);
        assert!(future.await);
        assert!(cancelled);
        assert!(rx.is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn saturation_cancels_admission_without_creating_more_workers() {
        let client = crate::test_utils::create_test_client().await;
        let chat = "120363000000042@g.us".parse().unwrap();
        let (queue_tx, rx) = async_channel::bounded(1);
        let rx = DrainingReceiver(rx);
        let lane = ChatLane {
            generation: 0,
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
                .load(Ordering::Acquire)
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
