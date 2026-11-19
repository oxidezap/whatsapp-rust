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
use std::hash::{BuildHasher, RandomState};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

type BudgetRegistry = std::sync::Mutex<BudgetEntries>;

#[derive(Default)]
struct BudgetEntries {
    hasher: RandomState,
    table: hashbrown::HashTable<BudgetEntry>,
}

struct BudgetEntry {
    chat: wacore_binary::jid::Jid,
    budget: std::sync::Weak<ByteBudget>,
}

struct BudgetRegistration {
    registry: std::sync::Weak<BudgetRegistry>,
    hash: u64,
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
        if let Ok(entry) = entries
            .table
            .find_entry(registration.hash, |entry| entry.budget.as_ptr() == address)
        {
            entry.remove();
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

type ChatSender = async_channel::Sender<crate::client::QueuedChatMessage>;
type SharedChatSender = std::sync::Mutex<ChatSenderState>;

struct ChatSenderState {
    sender: Option<ChatSender>,
    capacity: Option<std::num::NonZeroUsize>,
    remaining: usize,
}

pub(crate) enum ChatQueueAdmitError {
    Closed,
    Full,
}

/// The cached lane keeps its ordering locks after the worker exits. The
/// worker retires this sender separately so those locks cannot pin the queue.
#[derive(Clone)]
pub(crate) struct ChatQueueSender(Arc<SharedChatSender>);

impl From<ChatSender> for ChatQueueSender {
    fn from(sender: ChatSender) -> Self {
        Self(Arc::new(std::sync::Mutex::new(ChatSenderState {
            sender: Some(sender),
            capacity: None,
            remaining: 0,
        })))
    }
}

impl ChatQueueSender {
    pub(crate) fn bounded(capacity: usize) -> (Self, ChatQueueReceiver) {
        let capacity = std::num::NonZeroUsize::new(capacity).expect("chat queue capacity");
        let (sender, receiver) = async_channel::unbounded();
        let sender = Self(Arc::new(std::sync::Mutex::new(ChatSenderState {
            sender: Some(sender),
            capacity: Some(capacity),
            remaining: capacity.get(),
        })));
        let receiver = ChatQueueReceiver {
            receiver: DrainingReceiver(receiver),
            sender: Arc::downgrade(&sender.0),
        };
        (sender, receiver)
    }

    fn active_sender(&self) -> Option<ChatSender> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sender
            .clone()
    }

    pub(crate) fn admit_sender(&self) -> Result<ChatSender, ChatQueueAdmitError> {
        let mut state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let ChatSenderState {
            sender,
            capacity,
            remaining,
        } = &mut *state;
        let sender = sender.as_ref().ok_or(ChatQueueAdmitError::Closed)?;
        if sender.is_closed() {
            return Err(ChatQueueAdmitError::Closed);
        }
        if let Some(limit) = capacity {
            // The producer holds the lane's enqueue lock. A receiver can only
            // increase spare capacity, and a failed admission spends at most
            // one credit until the next length check.
            if *remaining == 0 {
                *remaining = limit.get().saturating_sub(sender.len());
            }
            if *remaining == 0 {
                return Err(ChatQueueAdmitError::Full);
            }
            *remaining -= 1;
        } else if sender.is_full() {
            return Err(ChatQueueAdmitError::Full);
        }
        Ok(sender.clone())
    }

    pub(crate) fn close(&self) -> bool {
        self.active_sender().is_some_and(|sender| sender.close())
    }

    pub(crate) fn same_channel(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(crate) fn len(&self) -> usize {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sender
            .as_ref()
            .map_or(0, ChatSender::len)
    }

    #[cfg(any(test, feature = "bench-harness"))]
    pub(crate) fn is_closed(&self) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .sender
            .as_ref()
            .is_none_or(ChatSender::is_closed)
    }

    #[cfg(test)]
    pub(crate) fn capacity(&self) -> Option<usize> {
        let state = self.0.lock().unwrap_or_else(|p| p.into_inner());
        let sender = state.sender.as_ref()?;
        state
            .capacity
            .map(std::num::NonZeroUsize::get)
            .or_else(|| sender.capacity())
    }
}

pub(crate) struct ChatQueueReceiver {
    receiver: DrainingReceiver<crate::client::QueuedChatMessage>,
    sender: std::sync::Weak<SharedChatSender>,
}

impl std::ops::Deref for ChatQueueReceiver {
    type Target = async_channel::Receiver<crate::client::QueuedChatMessage>;

    fn deref(&self) -> &Self::Target {
        &self.receiver
    }
}

impl Drop for ChatQueueReceiver {
    fn drop(&mut self) {
        self.receiver.close();
        if let Some(shared) = self.sender.upgrade() {
            let sender = shared
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .sender
                .take();
            drop(sender);
        }
    }
}

// Offline delivery is pulled in batches of `offline_resume::BATCH_SIZE`, and a
// continuation is only reserved while the stanzas requested and not yet
// finished fit one batch. That gate makes two batches the most one busy chat
// can ever be handed; a lane smaller than that overflows on every resume: the
// burst is refused, the connection recycles, the server redelivers the same
// backlog, and the resume never gets past it. Memory stays bounded by the byte
// budgets below.
pub(crate) const CHAT_QUEUE_ITEMS: usize = 512;
pub(crate) const CHAT_BYTES: usize = 16 * 1024 * 1024;
const GLOBAL_CHAT_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const ACK_QUEUE_ITEMS: usize = 256;
const ACK_BYTES: usize = 256 * 1024;
// Allow the worker, socket sender and completion wakeups to run. This is a
// fixed scheduling allowance, never a wait for capacity or a processing result.
pub(crate) const SATURATION_RETRY_TURNS: usize = 4;

/// Some runtimes omit periodic reader yields. Saturation still needs an actual
/// scheduling point before retrying, without waiting for the worker to finish.
pub(crate) async fn yield_for_capacity(runtime: &dyn wacore::runtime::Runtime) {
    if let Some(turn) = runtime.yield_now() {
        turn.await;
    } else {
        let mut yielded = false;
        futures::future::poll_fn(|cx| {
            if std::mem::replace(&mut yielded, true) {
                std::task::Poll::Ready(())
            } else {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
        })
        .await;
    }
}

/// ACK delivery and cold ADV discovery are optional on a connected client.
/// Share their lazy attachment so unused services do not enlarge its controller.
#[derive(Default)]
struct QueueTasks {
    ack_queue: std::sync::OnceLock<async_channel::Sender<Box<QueuedAck>>>,
    adv_bootstrap: std::sync::OnceLock<Arc<crate::prekeys::AdvBootstrapRegistry>>,
}

pub(crate) struct QueueBudgets {
    tasks: std::sync::OnceLock<Box<QueueTasks>>,
    pub chat: Arc<ByteBudget>,
    pub ack: Arc<ByteBudget>,
    pub work: Arc<crate::flush_scope::FlushScope>,
    per_chat: Arc<BudgetRegistry>,
    pub draining: std::sync::atomic::AtomicBool,
}
impl Default for QueueBudgets {
    fn default() -> Self {
        Self {
            tasks: std::sync::OnceLock::new(),
            chat: Arc::new(ByteBudget::new(GLOBAL_CHAT_BYTES)),
            ack: Arc::new(ByteBudget::new(ACK_BYTES)),
            work: Arc::default(),
            per_chat: Arc::default(),
            draining: false.into(),
        }
    }
}
impl QueueBudgets {
    /// Attach the ACK queue only when a stanza actually needs deferred delivery.
    pub(crate) fn ack_queue(&self) -> &std::sync::OnceLock<async_channel::Sender<Box<QueuedAck>>> {
        &self.tasks.get_or_init(Box::default).ack_queue
    }

    /// Inspect an existing ACK worker without allocating optional task state.
    pub(crate) fn ack_sender(&self) -> Option<&async_channel::Sender<Box<QueuedAck>>> {
        self.tasks.get().and_then(|tasks| tasks.ack_queue.get())
    }

    /// Attach account-flight coordination only when ADV discovery needs it.
    pub(crate) fn adv_bootstrap(&self) -> &Arc<crate::prekeys::AdvBootstrapRegistry> {
        self.tasks
            .get_or_init(Box::default)
            .adv_bootstrap
            .get_or_init(Arc::default)
    }

    /// Count queue controllers, live chat budgets and retained ADV flights, including empty map capacity.
    pub(crate) fn memory_stats(&self) -> (wacore::stats::CollectionStats, u64, u64) {
        use wacore::stats::HeapSize;
        let entries = self.per_chat.lock().unwrap_or_else(|p| p.into_inner());
        let registry_bytes = wacore::stats::hash_table_bytes(
            entries.table.capacity(),
            size_of::<BudgetEntry>(),
        ) + entries
            .table
            .iter()
            .map(|entry| {
                // Count the budget allocation without upgrading its weak reference.
                entry.chat.heap_bytes() + size_of::<ByteBudget>() + 2 * size_of::<usize>()
            })
            .sum::<usize>()
            + size_of::<BudgetRegistry>()
            + 2 * size_of::<usize>()
            // Controller, global counters and work scope are counted once.
            + size_of::<Self>()
            + 2 * (size_of::<ByteBudget>() + 2 * size_of::<usize>())
            + size_of::<crate::flush_scope::FlushScope>() + 2 * size_of::<usize>()
            + self.tasks.get().map_or(0, |tasks| {
                size_of::<QueueTasks>()
                    + tasks
                        .adv_bootstrap
                        .get()
                        .map_or(0, |registry| registry.retained_bytes())
            });
        (
            wacore::stats::CollectionStats::new(entries.table.len() as u64, registry_bytes as u64),
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
        let BudgetEntries { hasher, table } = &mut *entries;
        let hash = hasher.hash_one(chat);
        let slot = table.find_mut(hash, |entry| &entry.chat == chat);
        if let Some(budget) = slot.as_ref().and_then(|entry| entry.budget.upgrade()) {
            return budget;
        }
        // Only live lanes/byte leases own a strong reference. Their final drop
        // removes this weak entry, so sequential chat churn leaves no tombstones.
        let budget = Arc::new(ByteBudget {
            available: AtomicUsize::new(bytes),
            registration: Some(BudgetRegistration {
                registry: Arc::downgrade(&self.per_chat),
                hash,
            }),
        });
        let weak = Arc::downgrade(&budget);
        if let Some(slot) = slot {
            slot.budget = weak;
        } else {
            table.insert_unique(
                hash,
                BudgetEntry {
                    chat: chat.clone(),
                    budget: weak,
                },
                |entry| hasher.hash_one(&entry.chat),
            );
        }
        budget
    }
}

pub(crate) fn reserve(budget: &Arc<ByteBudget>, bytes: usize) -> Option<ByteLease> {
    // A minimum charge also bounds the number of tiny queued or in-flight nodes.
    let charge = bytes.max(1024);
    budget
        .available
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |available| {
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

/// Owns an encoded ACK and its budget lease. Keeping the `Vec` itself (rather
/// than converting it to `Bytes` first) avoids the `Bytes` shared-header
/// allocation for a `Vec` whose length differs from its capacity, and keeps
/// the `from_owner` allocation small. This offsets the boxed queue entry.
struct RetainedAck {
    bytes: Vec<u8>,
    _lease: ByteLease,
}
impl AsRef<[u8]> for RetainedAck {
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

pub(crate) fn retain_ack(bytes: Vec<u8>, lease: ByteLease) -> bytes::Bytes {
    bytes::Bytes::from_owner(RetainedAck {
        bytes,
        _lease: lease,
    })
}

impl crate::client::Client {
    /// Reuse the original ACK queue's lazy cell for all receive-queue state.
    /// Once created, budgets survive every connection/lane reset.
    // Keep one copy of this access/initialization path under LTO: chat and ACK
    // call sites need the same controller, not duplicated initialization code.
    #[inline(never)]
    pub(crate) fn inbound_queues(&self) -> &QueueBudgets {
        self.inbound_queues.get_or_init(Box::default)
    }

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
            if shutdown.is_fired() || self.inbound_queues().draining.swap(true, Ordering::AcqRel) {
                return;
            }
            self.inbound_queues().work.close();
        }
        log::warn!(
            "Inbound queue overloaded; draining before reconnect (chat bytes available: {}, ACK bytes available: {})",
            self.inbound_queues().chat.available(),
            self.inbound_queues().ack.available()
        );
        let client = Arc::clone(self);
        let shutdown = shutdown.clone();
        self.runtime.spawn_detached(Box::pin(async move {
            let grace = std::time::Duration::from_secs(5);
            client
                .inbound_queues()
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
    /// Optional workers must not add inline words or allocate during stats reads.
    #[test]
    fn cold_queue_controller_keeps_its_pre_adv_footprint() {
        // The pre-ADV controller had one ACK slot, four Arc pointers and a
        // draining flag padded to pointer alignment. Keep that representation.
        let legacy = size_of::<std::sync::OnceLock<async_channel::Sender<Box<QueuedAck>>>>()
            + 5 * size_of::<usize>();
        assert_eq!(size_of::<QueueBudgets>(), legacy);
        let budgets = QueueBudgets::default();
        let before = budgets.memory_stats().0.bytes;
        assert!(budgets.ack_sender().is_none());
        assert_eq!(budgets.memory_stats().0.bytes, before);
        assert!(budgets.tasks.get().is_none());
    }

    /// Count the shared sidecar once, then count ADV ownership when it is used.
    #[test]
    fn optional_queue_task_allocations_remain_visible() {
        let budgets = QueueBudgets::default();
        let cold = budgets.memory_stats().0.bytes;
        let (tx, _rx) = async_channel::bounded(1);
        assert!(budgets.ack_queue().set(tx).is_ok());
        let ack = budgets.memory_stats().0.bytes;
        assert_eq!(ack - cold, size_of::<QueueTasks>() as u64);
        let registry = budgets.adv_bootstrap();
        assert_eq!(
            budgets.memory_stats().0.bytes - ack,
            registry.retained_bytes() as u64
        );
        assert!(budgets.ack_sender().is_some());
    }

    use super::*;
    use crate::client::{ChatLane, QueuedChatMessage};
    use async_lock::Mutex;
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn a_dropped_chat_receiver_releases_the_cached_sender() {
        let (cached, worker) = ChatQueueSender::bounded(8);
        let outstanding_producer = cached.active_sender().unwrap();
        drop(worker);
        assert!(outstanding_producer.is_closed());
        assert!(cached.active_sender().is_none());
    }

    fn lane(
        global: &Arc<ByteBudget>,
        bytes: usize,
        items: usize,
    ) -> (ChatLane, DrainingReceiver<QueuedChatMessage>, QueueBudgets) {
        let (queue_tx, rx) = async_channel::bounded(items);
        (
            ChatLane {
                generation: 0,
                queue_tx: queue_tx.into(),
                enqueue_lock: Arc::new(Mutex::new(())),
                worker_running: Arc::new(Mutex::new(())),
                byte_budget: Arc::new(ByteBudget::new(bytes)),
            },
            DrainingReceiver(rx),
            QueueBudgets {
                chat: Arc::clone(global),
                ..QueueBudgets::default()
            },
        )
    }
    #[test]
    fn byte_budget_refusals_do_not_exhaust_lazy_queue_capacity() {
        let global = Arc::new(ByteBudget::new(4096));
        let (mut lane, _unused_receiver, budgets) = lane(&global, 4096, 1);
        let (sender, receiver) = ChatQueueSender::bounded(2);
        lane.queue_tx = sender;

        for id in 0..2 {
            assert!(matches!(
                lane.try_enqueue(&budgets, node(id, 4096)),
                Err(async_channel::TrySendError::Full(_))
            ));
        }
        for id in 0..2 {
            lane.try_enqueue(&budgets, node(id, 0)).unwrap();
        }
        assert!(matches!(
            lane.try_enqueue(&budgets, node(2, 0)),
            Err(async_channel::TrySendError::Full(_))
        ));
        drop(receiver.try_recv().unwrap());
        lane.try_enqueue(&budgets, node(2, 0)).unwrap();
        assert_eq!(receiver.len(), 2);
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

    #[tokio::test]
    async fn an_offline_batch_can_handoff_a_full_lane_to_its_ready_worker() {
        ready_chat_backlog(false).await;
    }

    #[tokio::test]
    async fn reader_scheduled_offline_batches_progress_without_reconnecting() {
        ready_chat_backlog(true).await;
    }

    async fn ready_chat_backlog(reader_yields: bool) {
        let client = crate::test_utils::create_test_client().await;
        let (lane, rx, _lane_queues) =
            lane(&client.inbound_queues().chat, CHAT_BYTES, CHAT_QUEUE_ITEMS);
        let budget = Arc::clone(&lane.byte_budget);
        client
            .chat_lanes
            .insert("120363000000042@g.us".parse().unwrap(), lane)
            .await;
        let worker = tokio::spawn(async move {
            for id in 0..600 {
                let queued = rx.recv().await.unwrap();
                assert_eq!(
                    queued.node.attrs().optional_string("id").as_deref(),
                    Some(id.to_string().as_str())
                );
            }
        });
        // Three 200-stanza batches. The adversarial case omits the real reader's
        // periodic yields; neither case may recycle a connection with ready work.
        for id in 0..600 {
            let mut cancelled = false;
            crate::handlers::message::MessageHandler::handle_inline(
                Arc::clone(&client),
                node(id, 0),
                &mut cancelled,
            )
            .await;
            assert!(!cancelled, "healthy batch cancelled at {id}");
            if reader_yields
                && ((id + 1) as u32).is_multiple_of(client.runtime.yield_frequency())
                && let Some(turn) = client.runtime.yield_now()
            {
                turn.await;
            }
        }
        worker.await.unwrap();
        assert!(!client.inbound_queues().draining.load(Ordering::Acquire));
        assert!(!client.connection_shutdown_signal().is_fired());
        assert_eq!(budget.available(), CHAT_BYTES);
    }

    #[tokio::test]
    async fn a_lane_capacity_retry_cannot_admit_work_after_reconnect() {
        let client = crate::test_utils::create_test_client().await;
        let (lane, rx, lane_queues) = lane(&client.inbound_queues().chat, CHAT_BYTES, 1);
        lane.try_enqueue(&lane_queues, node(0, 0)).unwrap();
        let shutdown = client.connection_shutdown_signal();
        let enqueue = lane.enqueue_lock.lock().await;
        let mut retry = Box::pin(lane.enqueue_scoped(
            &lane_queues,
            &enqueue,
            node(1, 0),
            shutdown,
            &*client.runtime,
            None,
        ));
        assert!(futures::poll!(&mut retry).is_pending());
        client.reset_connection_shutdown();
        drop(rx.try_recv().unwrap());
        assert!(retry.await.is_err());
        assert!(rx.is_empty());
        assert_eq!(lane.byte_budget.available(), CHAT_BYTES);
        assert!(!client.inbound_queues().draining.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn closed_work_admission_stops_capacity_retries_immediately() {
        let client = crate::test_utils::create_test_client().await;
        for close_before in [true, false] {
            let (lane, rx, lane_queues) = lane(&client.inbound_queues().chat, CHAT_BYTES, 1);
            if close_before {
                lane_queues.work.close();
            } else {
                lane.try_enqueue(&lane_queues, node(0, 0)).unwrap();
            }
            let enqueue = lane.enqueue_lock.lock().await;
            let mut retry = Box::pin(lane.enqueue_scoped(
                &lane_queues,
                &enqueue,
                node(1, 64 * 1024),
                client.connection_shutdown_signal(),
                &*client.runtime,
                None,
            ));
            if !close_before {
                assert!(futures::poll!(&mut retry).is_pending());
                lane_queues.work.close();
                drop(rx.try_recv().unwrap());
            }
            assert!(matches!(
                futures::poll!(&mut retry),
                std::task::Poll::Ready(Err(_))
            ));
            assert_eq!(
                lane_queues.work.track_rejections(),
                usize::from(close_before)
            );
            assert_eq!(lane_queues.work.pending(), 0);
            assert_eq!(lane.byte_budget.available(), CHAT_BYTES);
            assert!(rx.is_empty());
        }
    }

    #[tokio::test]
    async fn reporting_and_cleanup_do_not_allocate_unused_queue_state() {
        let client = crate::test_utils::create_test_client().await;
        assert!(client.inbound_queues.get().is_none());
        let report = client.memory_report().await;
        assert_eq!(report.inbound_chat_budgets.bytes, 0);
        assert_eq!(report.transport_ack_queue, 0);
        client.chat_lanes.clear().await;
        assert!(client.inbound_queues.get().is_none());
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
        assert_eq!(budgets.per_chat.lock().unwrap().table.len(), 1);
        drop(retained);
        assert!(budgets.per_chat.lock().unwrap().table.is_empty());
        for id in 0..128 {
            let chat = format!("12036300000{id:04}@g.us").parse().unwrap();
            drop(budgets.chat_budget(&chat));
            assert!(budgets.per_chat.lock().unwrap().table.is_empty());
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
        let hash = entries.hasher.hash_one(&chat);
        let replacement = Arc::new(ByteBudget {
            available: AtomicUsize::new(CHAT_BYTES),
            registration: Some(BudgetRegistration {
                registry: Arc::downgrade(&registry),
                hash,
            }),
        });
        entries
            .table
            .find_mut(hash, |entry| entry.chat == chat)
            .unwrap()
            .budget = Arc::downgrade(&replacement);
        drop(entries);
        dropping.join().unwrap();
        let found = budgets.chat_budget(&chat);
        assert!(Arc::ptr_eq(&found, &replacement));
        drop(found);
        drop(replacement);
        assert!(registry.lock().unwrap().table.is_empty());
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
        let (lane, rx, lane_queues) = lane(&global, 256 * 1024, CHAT_QUEUE_ITEMS);
        let mut accepted = 0;
        for id in 0..128 {
            if lane.try_enqueue(&lane_queues, node(id, 64 * 1024)).is_ok() {
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
        let (lane, rx, lane_queues) = lane(&global, 2048, 1);
        lane.try_enqueue(&lane_queues, node(1, 4)).unwrap();
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
        let (busy, busy_rx, busy_queues) = lane(&global, 2048, 2);
        let (other, other_rx, other_queues) = lane(&global, 2048, 2);
        busy.try_enqueue(&busy_queues, node(1, 4)).unwrap();
        busy.try_enqueue(&busy_queues, node(2, 4)).unwrap();
        assert!(matches!(
            busy.try_enqueue(&busy_queues, node(3, 4)),
            Err(async_channel::TrySendError::Full(_))
        ));
        other.try_enqueue(&other_queues, node(4, 4)).unwrap();
        let first = busy_rx.try_recv().unwrap();
        assert_eq!(
            first.node.attrs().optional_string("id").as_deref(),
            Some("1")
        );
        assert!(
            busy.try_enqueue(&busy_queues, node(3, 4)).is_err(),
            "in-flight ownership still counts"
        );
        drop(first);
        busy.try_enqueue(&busy_queues, node(3, 4)).unwrap();
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
        let (lane, rx, lane_queues) = lane(&global, 4096, 1);
        lane.try_enqueue(&lane_queues, node(1, 4)).unwrap();
        assert!(lane.try_enqueue(&lane_queues, node(2, 4)).is_err());
        assert_eq!(global.available(), 3072);
        rx.close();
        assert!(matches!(
            lane.try_enqueue(&lane_queues, node(3, 4)),
            Err(async_channel::TrySendError::Closed(_))
        ));
        drop(rx);
        assert_eq!(global.available(), 4096);
    }

    #[test]
    fn global_budget_applies_across_lanes_and_reservation_failure_rolls_back() {
        let global = Arc::new(ByteBudget::new(2048));
        let (a, a_rx, a_queues) = lane(&global, 4096, 8);
        let (b, b_rx, b_queues) = lane(&global, 4096, 8);
        a.try_enqueue(&a_queues, node(1, 4)).unwrap();
        b.try_enqueue(&b_queues, node(2, 4)).unwrap();
        assert!(a.try_enqueue(&a_queues, node(3, 4)).is_err());
        drop(a_rx);
        b.try_enqueue(&b_queues, node(3, 4)).unwrap();
        drop(b_rx);
        assert_eq!(global.available(), 2048);
        let empty = Arc::new(ByteBudget::new(0));
        assert!(retain_node(&node(1, 4), &global, &empty, Arc::default()).is_none());
        assert_eq!(global.available(), 2048);
    }

    #[tokio::test]
    async fn worker_cancelled_before_first_poll_drains_its_cached_sender() {
        let global = Arc::new(ByteBudget::new(2048));
        let (lane, rx, lane_queues) = lane(&global, 2048, 2);
        lane.try_enqueue(&lane_queues, node(1, 4)).unwrap();
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
        let (lane, rx, lane_queues) = lane(&global, 2048, 2);
        lane.try_enqueue(&lane_queues, node(1, 4)).unwrap();
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
        assert!(!client.inbound_queues().draining.load(Ordering::Acquire));
        assert!(client.inbound_queues().work.try_track().is_some());
        assert!(client.outbound_flush.try_track().is_some());
        assert!(!client.connection_shutdown_signal().is_fired());
    }

    #[tokio::test]
    async fn retired_overload_drain_cannot_close_fresh_outbound_admission() {
        let client = crate::test_utils::create_test_client().await;
        let work = client.inbound_queues().work.try_track().unwrap();
        let old = client.connection_shutdown_signal();
        client.drain_overloaded_inbound(&old);
        while client.inbound_queues().work.flush_waiters() == 0 {
            tokio::task::yield_now().await;
        }
        client.reset_connection_shutdown();
        client.outbound_flush.reopen();
        drop(work);
        while client.inbound_queues().work.flush_waiters() != 0 {
            tokio::task::yield_now().await;
        }
        assert!(client.outbound_flush.try_track().is_some());
        assert!(client.inbound_queues().work.try_track().is_some());
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
        let budget = client.inbound_queues().chat_budget_with_limit(&chat, 1024);
        let weak = Arc::downgrade(&budget);
        let (queue_tx, rx) = async_channel::bounded(1);
        let rx = DrainingReceiver(rx);
        let lane = ChatLane {
            generation: 0,
            queue_tx: queue_tx.into(),
            enqueue_lock: Arc::default(),
            worker_running: Arc::default(),
            byte_budget: budget,
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
        lane.try_enqueue(client.inbound_queues(), message())
            .unwrap();
        client.chat_lanes.insert(chat.clone(), lane).await;
        let queued = rx.try_recv().unwrap();
        let slice = queued
            .node
            .slice_bytes(queued.node.content_bytes().unwrap());
        drop(queued);
        drop(rx);
        client.notify_connection_shutdown();
        client.chat_lanes.clear().await; // the exact cleanup operation bypassing evict_guard
        let ack_lease = reserve(&client.inbound_queues().ack, 4).unwrap();
        let report = client.memory_report().await;
        assert_eq!(report.chat_lane_backlog, 0);
        assert_eq!(report.inbound_chat_budgets.entries, 1);
        assert!(report.inbound_chat_budgets.bytes > 0);
        assert_eq!(report.inbound_chat_charged_bytes, 1024);
        assert_eq!(report.inbound_ack_charged_bytes, 1024);
        let registry_line = report
            .to_string()
            .lines()
            .find(|line| line.contains("inbound_chat_budgets:"))
            .expect("the retained registry must be attributable in logged reports")
            .to_owned();
        assert!(registry_line.contains("1 entries"));
        assert!(registry_line.contains(&format!("{} B", report.inbound_chat_budgets.bytes)));
        let total = report.total_estimated_bytes();
        let mut without_queues = report;
        without_queues.inbound_chat_budgets.bytes = 0;
        without_queues.inbound_chat_charged_bytes = 0;
        without_queues.inbound_ack_charged_bytes = 0;
        assert!(total >= without_queues.total_estimated_bytes() + 2048);
        drop(ack_lease);
        client.advance_generation_for_test(1, Ordering::AcqRel);
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
        let work = client.inbound_queues().work.try_track().unwrap();
        client.drain_overloaded_inbound(&client.connection_shutdown_signal());
        client.drain_overloaded_inbound(&client.connection_shutdown_signal()); // one drain per connection
        assert!(client.inbound_queues().work.try_track().is_none());
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
        assert!(client.inbound_queues().work.try_track().is_some());
        assert!(!client.inbound_queues().draining.load(Ordering::Acquire));
    }

    #[tokio::test(start_paused = true)]
    async fn stalled_work_cannot_hold_the_overload_drain_open_forever() {
        let client = crate::test_utils::create_test_client().await;
        let work = client.inbound_queues().work.try_track().unwrap();
        client.drain_overloaded_inbound(&client.connection_shutdown_signal());
        wacore::runtime::wait_for_shutdown(&client.connection_shutdown_signal()).await;
        assert_eq!(client.inbound_queues().work.pending(), 1);
        drop(work);
        assert_eq!(client.inbound_queues().work.pending(), 0);
    }

    #[tokio::test]
    async fn a_waiting_admission_cannot_cross_a_reconnect() {
        let client = crate::test_utils::create_test_client().await;
        let (queue_tx, rx) = async_channel::bounded(1);
        let lane = ChatLane {
            generation: 0,
            queue_tx: queue_tx.into(),
            enqueue_lock: Arc::default(),
            worker_running: Arc::default(),
            byte_budget: Arc::new(ByteBudget::new(CHAT_BYTES)),
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
        client.advance_generation_for_test(1, Ordering::AcqRel);
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
            queue_tx: queue_tx.into(),
            enqueue_lock: Arc::default(),
            worker_running: Arc::default(),
            byte_budget: Arc::new(ByteBudget::new(CHAT_BYTES)),
        };
        let node = || {
            crate::test_utils::node_to_owned_ref(
                &NodeBuilder::new("message")
                    .attr("from", "120363000000042@g.us")
                    .attr("id", "pending")
                    .build(),
            )
        };
        lane.try_enqueue(client.inbound_queues(), node()).unwrap();
        client.chat_lanes.insert(chat, lane).await;
        let mut cancelled = false;
        crate::handlers::message::MessageHandler::handle_inline(
            Arc::clone(&client),
            node(),
            &mut cancelled,
        )
        .await;
        assert!(cancelled);
        assert!(client.inbound_queues().draining.load(Ordering::Acquire));
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
