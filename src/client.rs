mod accessors;
mod adapters;
mod app_state;
pub(crate) use app_state::{
    BatchedSyncOutcome, BatchedSyncRequest, CriticalSyncPlan, SyncScope, SyncSettles,
};
#[cfg(test)]
pub(crate) use app_state::{SyncHolder, batched_sync_outcome_tests::batch_result};
mod builder;
mod context_impl;
mod device_memo_stats;
mod device_registry;
pub(crate) mod device_topology;
#[cfg(feature = "client-lifecycle")]
mod extension_lifecycle;
pub mod interceptor;
mod iq_ops;
mod lid_pn;
mod lifecycle;
pub(crate) mod member_index;
mod messaging;
mod node_io;
pub(crate) mod offline_resume;
mod sender_keys;
mod sessions;
pub(crate) mod subsystem;
pub(crate) mod voip;
use builder::{ClientAssembly, ClientExtensions};
pub use builder::{ClientBuild, ClientBuilder, ClientBuilderError};
pub(crate) use device_memo_stats::{
    DeviceMemoCounters, GroupDevicesMemoOutcome, SkdmTargetsMemoOutcome,
};
pub use device_memo_stats::{DeviceMemoStats, GroupDevicesMemoStats, SkdmTargetsMemoStats};
#[cfg(feature = "client-lifecycle")]
use extension_lifecycle::LifecycleRegistration;
#[cfg(feature = "client-lifecycle")]
#[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
pub use extension_lifecycle::{ClientLifecycle, ConnectionScope, ConnectionScopeState};
pub use lifecycle::{Connection, Reachability};
pub use voip::{CallError, Voip};

use crate::cache::Cache;
use crate::cache_store::TypedCache;
use crate::handshake;
use crate::lid_pn_cache::LidPnCache;
use crate::pair;
use anyhow::Result;
use futures::FutureExt;
#[cfg(test)]
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;

use wacore::xml::{DisplayableNode, DisplayableNodeRef};
use wacore_binary::JidExt;
use wacore_binary::Node;
use wacore_binary::builder::NodeBuilder;
#[cfg(test)]
use wacore_binary::{Attrs, NodeValue};

use crate::appstate_sync::AppStateProcessor;
use crate::handlers::chatstate::ChatStateEvent;
use crate::jid_utils::server_jid;
use crate::store::{commands::DeviceCommand, persistence_manager::PersistenceManager};
use crate::types::enc_handler::EncHandler;
use crate::types::events::{ConnectFailureReason, Event};

use log::{debug, error, info, trace, warn};

use rand::{Rng, RngExt};
use scopeguard;
use wacore_binary::Jid;

use portable_atomic::AtomicU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};
use wacore::stanza::wire_tags::{NotificationType, StanzaTag};

/// Lease that keeps decrypted-payload events enabled for one consumer.
///
/// Dropping the final lease disables forwarding. The lease holds only a weak
/// client reference, so it cannot keep the client alive.
#[must_use = "dropping the lease immediately releases decrypted-payload forwarding"]
pub struct DecryptedPayloadLease {
    client: std::sync::Weak<Client>,
}

impl Drop for DecryptedPayloadLease {
    fn drop(&mut self) {
        let Some(client) = self.client.upgrade() else {
            return;
        };
        let previous = client
            .decrypted_payload_forwarding
            .fetch_sub(1, Ordering::Relaxed);
        debug_assert!(previous > 0, "decrypted-payload forwarding lease underflow");
    }
}

/// Lease that keeps per-`<enc>` decrypt-failure events enabled for one consumer.
///
/// Dropping the final lease disables forwarding. The lease holds only a weak
/// client reference, so it cannot keep the client alive.
#[must_use = "dropping the lease immediately releases enc-decrypt-failure forwarding"]
pub struct EncDecryptFailedLease {
    client: std::sync::Weak<Client>,
}

impl Drop for EncDecryptFailedLease {
    fn drop(&mut self) {
        let Some(client) = self.client.upgrade() else {
            return;
        };
        let previous = client
            .enc_decrypt_failed_forwarding
            .fetch_sub(1, Ordering::Relaxed);
        debug_assert!(
            previous > 0,
            "enc-decrypt-failure forwarding lease underflow"
        );
    }
}

/// Lease that keeps raw decoded stanza events enabled for one consumer.
///
/// Dropping the final lease disables forwarding. The lease holds only a weak
/// client reference, so it cannot keep the client alive.
#[must_use = "dropping the lease immediately releases raw-node forwarding"]
pub struct RawNodeLease {
    client: std::sync::Weak<Client>,
}

impl Drop for RawNodeLease {
    fn drop(&mut self) {
        let Some(client) = self.client.upgrade() else {
            return;
        };
        let previous = client.raw_node_forwarding.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(previous > 0, "raw-node forwarding lease underflow");
    }
}

/// Lease that keeps sent-frame events enabled for one consumer.
///
/// Dropping the final lease disables forwarding. The lease holds only a weak
/// client reference, so it cannot keep the client alive.
///
/// It gates, it does not fence, and one aggregate count gates them all rather
/// than one per lease: a frame captured while any lease was alive can still
/// arrive just after this one drops, and which handlers receive it is a matter of
/// subscription, not of who holds a lease. Every gated kind works this way.
/// Making the drop wait for in-flight dispatches to drain would instead deadlock
/// an observer that drops its lease from inside its own handler, which is the
/// natural way to record one frame and stop.
#[must_use = "dropping the lease immediately releases sent-frame forwarding"]
pub struct SentFrameLease {
    client: std::sync::Weak<Client>,
}

impl Drop for SentFrameLease {
    fn drop(&mut self) {
        let Some(client) = self.client.upgrade() else {
            return;
        };
        client.sent_frame_tap.release();
    }
}

/// Publishes the plaintext frames that reached the transport as
/// [`Event::SentFrame`](wacore::types::events::Event::SentFrame).
///
/// The client owns it and hands the noise sender a clone of the `Arc`, the same
/// way it hands over [`SessionStats`](wacore::stats::SessionStats): the gate has
/// to be readable from the one point every send crosses, and that task cannot
/// hold the client without keeping it alive.
pub(crate) struct SentFrameTap {
    /// Number of consumers currently requesting the event.
    forwarding: AtomicUsize,
    bus: wacore::types::events::CoreEventBus,
    /// Proves the no-lease path builds nothing, rather than only that it
    /// dispatches nothing.
    #[cfg(test)]
    published: AtomicUsize,
}

impl SentFrameTap {
    pub(crate) fn new(bus: wacore::types::events::CoreEventBus) -> Self {
        Self {
            forwarding: AtomicUsize::new(0),
            bus,
            #[cfg(test)]
            published: AtomicUsize::new(0),
        }
    }

    /// Enable forwarding for one consumer. The public door is
    /// [`Client::acquire_sent_frame_forwarding`], which pairs this with a lease
    /// that releases it on drop; a caller here owns that pairing itself.
    pub(crate) fn acquire(&self) {
        let incremented = self
            .forwarding
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                count.checked_add(1)
            })
            .is_ok();
        assert!(incremented, "sent-frame forwarding lease counter overflow");
    }

    pub(crate) fn release(&self) {
        let previous = self.forwarding.fetch_sub(1, Ordering::Relaxed);
        debug_assert!(previous > 0, "sent-frame forwarding lease underflow");
    }

    #[inline]
    pub(crate) fn enabled(&self) -> bool {
        self.forwarding.load(Ordering::Relaxed) != 0
    }

    /// Hand one frame to the observers.
    ///
    /// The dispatch is caught: a consumer that only watches must not be able to
    /// take the send pipeline down with it, and this runs on the noise sender
    /// task, whose death would end every send on the connection. Containment is
    /// per dispatch, not per handler, so a panicking observer costs this frame
    /// for the observers behind it — the bus offers no per-handler isolation for
    /// any kind, and plugins already wrap their own handlers. A handler that
    /// *blocks* still stalls sends, the contract every handler has on the read
    /// loop.
    pub(crate) fn publish(&self, plaintext: bytes::Bytes) {
        #[cfg(test)]
        self.published.fetch_add(1, Ordering::Relaxed);
        let dispatch = std::panic::AssertUnwindSafe(|| {
            self.bus.dispatch(Event::SentFrame(
                wacore::types::events::SentFrame::builder()
                    .plaintext(plaintext)
                    .build(),
            ));
        });
        if std::panic::catch_unwind(dispatch).is_err() {
            warn!("A sent-frame observer panicked; the send pipeline is unaffected.");
        }
    }

    #[cfg(test)]
    pub(crate) fn published(&self) -> usize {
        self.published.load(Ordering::Relaxed)
    }
}

/// Filter for matching incoming stanzas (nodes) by tag and attributes.
///
/// Used with [`Client::wait_for_node`] to wait for specific stanzas.
/// Zero-cost when no waiters are active (single atomic load per node).
///
/// # Example
/// ```ignore
/// // Wait for a w:gp2 notification from a specific group
/// let waiter = client.wait_for_node(
///     NodeFilter::tag("notification")
///         .attr("type", "w:gp2")
///         .attr("from", "group@g.us"),
/// );
/// // ... trigger the action ...
/// let node = waiter.await?;
/// ```
#[derive(Debug, Clone)]
pub struct NodeFilter {
    tag: String,
    attrs: Vec<(String, String)>,
}

impl NodeFilter {
    /// Create a filter matching nodes with the given tag.
    pub fn tag(tag: impl Into<String>) -> Self {
        Self {
            tag: tag.into(),
            attrs: Vec::new(),
        }
    }

    /// Add an attribute constraint. All attributes must match.
    pub fn attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push((key.into(), value.into()));
        self
    }

    /// Shorthand for `.attr("from", jid.to_string())`.
    pub fn from_jid(self, jid: &Jid) -> Self {
        self.attr("from", jid.to_string())
    }

    fn matches(&self, node: &wacore_binary::NodeRef<'_>) -> bool {
        node.tag == self.tag.as_str()
            && self.attrs.iter().all(|(k, v)| {
                node.get_attr(k.as_str())
                    .is_some_and(|attr| attr == v.as_str())
            })
    }
}

struct NodeWaiter {
    filter: NodeFilter,
    tx: futures::channel::oneshot::Sender<Arc<wacore_binary::OwnedNodeRef>>,
}

struct SentNodeWaiter {
    filter: NodeFilter,
    tx: futures::channel::oneshot::Sender<Arc<Node>>,
}

fn resolve_waiters(
    waiters_mutex: &std::sync::Mutex<Vec<NodeWaiter>>,
    counter: &AtomicUsize,
    node: &Arc<wacore_binary::OwnedNodeRef>,
) {
    let nr = node.get();
    let mut waiters = waiters_mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut i = 0;
    while i < waiters.len() {
        if waiters[i].tx.is_canceled() {
            waiters.swap_remove(i);
            counter.fetch_sub(1, Ordering::Release);
        } else if waiters[i].filter.matches(nr) {
            let w = waiters.swap_remove(i);
            counter.fetch_sub(1, Ordering::Release);
            let _ = w.tx.send(Arc::clone(node));
        } else {
            i += 1;
        }
    }
}

use async_lock::Mutex;
use async_lock::RwLock;
use std::time::Duration;
use thiserror::Error;

use wacore::appstate::patch_decode::WAPatchName;
use wacore::client::context::GroupInfo;

/// Group metadata cache. Values are `Arc`-wrapped so a warm `query_info` hit
/// shares the metadata (refcount bump) instead of deep-cloning the participant
/// list and LID/PN maps on every group send.
type GroupCache = TypedCache<Jid, Arc<GroupInfo>>;

/// Memoized SKDM warm state per group: the `(devices, sender-key map)` Weak
/// pair + map generation it was computed against, the exact sending identity
/// the filter ran as (it excludes that device, and own-device classification
/// depends on it — a mid-session identity change must miss), and the memoized
/// `needs_skdm` targets (empty or own-devices-only). See `skdm_warm_memo`.
pub(crate) type SkdmWarmMemoEntry = (
    std::sync::Weak<wacore::send::ResolvedGroupDevices>,
    std::sync::Weak<crate::sender_key_device_cache::SenderKeyDeviceMap>,
    u64,
    Jid,
    // Frozen: the memoized targets never change once stored (a change of the
    // inputs produces a new entry), and the steady state is empty or the own
    // devices, so the filter's growth capacity has nothing to park here for.
    Box<[Jid]>,
);
use wacore::runtime::timeout as rt_timeout;
use waproto::whatsapp as wa;

use crate::cache_config::CacheConfig;
use crate::socket::{NoiseSocket, SocketError, error::EncryptSendError};
use crate::sync_task::MajorSyncTask;
use wacore::runtime::Runtime;

/// Type alias for chatstate event handler functions.
type ChatStateHandler = Arc<dyn Fn(ChatStateEvent) + Send + Sync>;

/// Per-chat lane for sequential message processing. Combines the enqueue lock
/// and queue sender into a single cached entry (one lookup instead of two).
/// Keyed by `Jid` to avoid per-message `to_string()` allocation.
#[derive(Clone)]
pub(crate) struct ChatLane {
    pub enqueue_lock: Arc<Mutex<()>>,
    pub queue_tx: async_channel::Sender<QueuedChatMessage>,
    /// Held by the lane's worker for as long as it runs; a replacement
    /// worker takes it before its first message. Why it is shared across
    /// lane generations is explained at `create_chat_lane`.
    pub worker_running: Arc<Mutex<()>>,
}

impl ChatLane {
    pub(crate) fn try_enqueue(
        &self,
        node: Arc<wacore_binary::OwnedNodeRef>,
    ) -> Result<(), async_channel::TrySendError<QueuedChatMessage>> {
        self.queue_tx.try_send(QueuedChatMessage {
            node,
            lane_liveness: Arc::clone(&self.enqueue_lock),
        })
    }
}

pub(crate) struct QueuedChatMessage {
    pub node: Arc<wacore_binary::OwnedNodeRef>,
    pub lane_liveness: Arc<Mutex<()>>,
}

const APP_STATE_RETRY_MAX_ATTEMPTS: u32 = 6;

/// WA Web: MQTT `MqttProtocolClient.connect()` uses `CONNECT_TIMEOUT = 20s`,
/// DGW `connectTimeoutMs` defaults to `20000ms`.
const TRANSPORT_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

pub use wacore::stats::{
    AllocSnapshot, CollectionStats, HttpResourceReport, StatsSnapshot, StorageResourceReport,
    TransportResourceReport,
};

/// On-demand report of the client's internal collections: entry counts plus
/// estimated retained heap bytes for the memory-dominant caches.
///
/// Counts are approximate (caches may have pending evictions); byte figures
/// are honest estimates (encoded-size proxies for Signal records, payload
/// sums elsewhere — see [`wacore::stats::HeapSize`]), suitable for
/// per-session attribution and leak detection, not byte-exact accounting.
/// Store-backed caches report `bytes: 0` — their entries live outside this
/// process.
///
/// Call [`Client::memory_report`] to obtain one. Nothing is computed unless
/// called.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct MemoryReport {
    // -- TTL/capacity-bounded caches --
    pub group_cache: CollectionStats,
    pub device_registry_cache: CollectionStats,
    pub lid_pn_lid_entries: CollectionStats,
    /// Entry count of the PN-direction map. Both maps share the same
    /// `Arc<LidPnEntry>` payloads, attributed to
    /// [`Self::lid_pn_lid_entries`]; bytes here cover only entries the LID
    /// map no longer holds (normally 0), so the total counts each once.
    pub lid_pn_pn_entries: CollectionStats,
    /// Contact-hash → LID index of the LID/PN cache, one entry per identifier
    /// (both sides of every pair). Bytes are the table alone: the LID it
    /// points at is the entry's, already counted above. Unbounded with the
    /// cache it indexes, and rebuilt from warm-up each process.
    pub lid_pn_contact_hash_entries: CollectionStats,
    /// PN → LID pairs this process has durably persisted, the dedup that lets
    /// the learn path skip a re-persist. Bytes are the table alone (both
    /// strings are the entry's). One entry per persisted contact for the
    /// process lifetime.
    pub lid_pn_persisted_entries: CollectionStats,
    pub recent_messages: CollectionStats,
    pub sender_key_device_cache: CollectionStats,
    pub group_devices_memo: CollectionStats,
    pub dm_devices_memo: CollectionStats,
    pub message_retry_counts: u64,
    pub undecryptable_dispatched: u64,
    /// Entries in the dispatch-once gate for decrypted messages.
    pub dispatched_messages: u64,
    pub pdo_pending_requests: u64,
    pub pdo_requested: u64,
    /// Queued/running history-sync tasks and their logical compressed-payload
    /// byte sum. A shared `Bytes` slice may retain a larger backing allocation,
    /// whose capacity is not exposed by the type.
    pub history_sync_tasks: CollectionStats,
    /// Lifetime high-water mark of queued/running history-sync tasks.
    pub history_sync_tasks_peak: u64,
    /// Lifetime high-water mark of logical compressed-payload bytes.
    pub history_sync_payload_bytes_peak: u64,
    // -- Transient retention (accumulated, not yet handed on) --
    /// Inbound messages accumulated for the next per-batch commit, and the
    /// encoded-byte sum compared against the 4 MiB flush threshold. The decoded
    /// protos this holds are the largest per-client allocation in this report by
    /// two orders of magnitude.
    ///
    /// "Accumulated", not "resident": a batch already handed to its commit is
    /// still in memory but no longer counted here (see
    /// `InboundCommitBatcher::pending_stats`). Live traffic commits
    /// immediately, so outside an offline drain this is normally zero.
    pub inbound_commit_batch: CollectionStats,
    /// Delivery receipts held back during an offline drain, to be flushed as
    /// aggregate `<receipt>` stanzas (WA Web `sendAggregateOfflineReceipts`).
    ///
    /// Bounded by the same commit batch that fills it — the buffer is flushed
    /// per batch snapshot, so it tops out at the batch's 400 messages — and
    /// empty outside the drain. Reported because it is the largest transient
    /// the drain retains after that batch itself.
    pub offline_receipt_buffer: CollectionStats,
    /// `messageSecret` captures buffered for write-behind persistence — from
    /// live receives and sends as well as an offline drain, so a slow backend
    /// can saturate this with no drain in progress.
    ///
    /// A producer that would exceed the 4096-entry limit waits for an in-flight
    /// write rather than the buffer growing. That limit is not a hard ceiling:
    /// a queueing future cancelled while backpressured force-buffers what it
    /// still holds rather than losing it, so this can read above the limit
    /// during teardown.
    pub msg_secret_buffer: usize,
    /// Users awaiting a device-list refresh, and the dedup that suppresses a
    /// second refresh for the same user while one is outstanding.
    ///
    /// Offline entries are drained by `doPendingDeviceSync` at the end of the
    /// backlog; an entry the *online* path adds leaves when its refresh
    /// finishes. A value that stays high outside a drain means refreshes are
    /// not completing, not that many users were seen.
    pub pending_device_sync: usize,
    // -- Capacity-only caches (coordination, counts only) --
    pub session_locks: u64,
    /// Addresses with a session establishment in flight; normally zero.
    pub ensure_inflight: u64,
    /// Groups with a metadata query in flight; normally zero.
    pub group_metadata_inflight: u64,
    pub chat_lanes: u64,
    /// Inbound messages queued behind their chat's lane worker, summed over
    /// every lane. The lanes are capacity-bounded; their queues are not, and
    /// each queued message retains its whole frame, so a worker that is stuck
    /// (a slow durability hook, a hung decrypt) shows up here as a backlog
    /// that grows with the chat's inbound rate.
    pub chat_lane_backlog: u64,
    pub group_distribution_locks: u64,
    /// Cumulative capacity evictions; poll successive reports to derive a rate.
    pub group_distribution_lock_evictions: u64,
    /// Cumulative attempts that kept a live lane and temporarily exceeded capacity.
    pub group_distribution_lock_eviction_blocks: u64,
    pub resend_rate_limiter_chats: u64,
    /// Peers whose session was recently recreated, keyed to rate-limit the next
    /// recreate. Counts only: the entries are a JID and an instant.
    pub session_recreate_history: u64,
    /// Groups whose sender-key distribution is memoised as already warm.
    pub skdm_warm_memo: u64,
    // -- Unbounded collections --
    /// Deferred acks queued for the transport-ack worker. Unbounded, and each
    /// entry retains the full inbound node plus a flush guard, so a stalled
    /// transport shows up here as a growing backlog.
    pub transport_ack_queue: usize,
    /// Delivery receipts queued for their worker, same shape as above.
    pub delivery_receipt_queue: usize,
    pub response_waiters: usize,
    pub node_waiters: usize,
    /// Waiters parked on outgoing nodes, the pre-encryption counterpart of
    /// [`Self::node_waiters`]. Each retains a filter and a oneshot sender.
    pub sent_node_waiters: usize,
    pub pending_retries: usize,
    /// Numbers with a `refresh_lid` re-resolve in flight. Bounded by the
    /// number of distinct peers acked at once; a value that stays high
    /// means refreshes are not completing, not that many were requested.
    pub pending_lid_refreshes: usize,
    pub presence_subscriptions: usize,
    pub app_state_key_requests: usize,
    /// Expanded app-state keys the processor holds in memory. No capacity cap
    /// and no TTL — one entry per distinct key id the server's patches
    /// reference, emptied only on reconnect. Zero until the first app-state
    /// sync builds the processor.
    pub app_state_key_cache: usize,
    /// Collections with an outstanding snapshot-recovery request.
    ///
    /// Bounded by the number of collections in practice, but retained until a
    /// reply consumes the entry, so it is reported rather than assumed small.
    pub app_state_recovery_requests: usize,
    pub app_state_syncing: usize,
    pub signal_sessions: CollectionStats,
    pub signal_identities: CollectionStats,
    pub signal_sender_keys: CollectionStats,
    /// What the optional subsystems attached to this build retain. Empty when
    /// none is attached. One field rather than a `cfg`'d field per subsystem,
    /// so the report has one shape whatever was compiled; see
    /// `agent_docs/subsystem_boundary.md`.
    pub subsystems: Vec<SubsystemMemory>,
    #[cfg(feature = "plugins")]
    pub plugins: u64,
    #[cfg(feature = "plugins")]
    pub plugin_install_tasks: u64,
    #[cfg(feature = "plugins")]
    pub plugin_connection_tasks: u64,
    #[cfg(feature = "plugins")]
    pub plugin_connection_generations: u64,
    #[cfg(feature = "plugins")]
    pub plugin_core_event_subscriptions: u64,
    #[cfg(feature = "plugins")]
    pub plugin_stanza_interceptors: u64,
    #[cfg(feature = "plugins")]
    pub plugin_event_endpoints: u64,
    #[cfg(feature = "plugins")]
    pub plugin_event_endpoint_capacity: u64,
    /// Unique custom-event envelopes and payload bytes still retained in endpoint queues.
    #[cfg(feature = "plugins")]
    pub plugin_event_queue: CollectionStats,
    // -- Misc --
    pub chatstate_handlers: usize,
    pub custom_enc_handlers: usize,
    /// Interceptors currently registered.
    ///
    /// A handle that outlives its interest leaves one registered, and a leak
    /// here costs a walk on every stanza — which the count is what makes
    /// visible.
    pub stanza_interceptors: usize,
}

/// Names one collection an attached subsystem reports.
///
/// A subsystem exports these as constants (see `voip::collections`), so looking
/// a figure up in [`MemoryReport`] is a name the compiler checks rather than two
/// string literals a caller has to spell the way the report happens to print
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubsystemCollection {
    pub subsystem: &'static str,
    pub collection: &'static str,
}

impl SubsystemCollection {
    pub const fn new(subsystem: &'static str, collection: &'static str) -> Self {
        Self {
            subsystem,
            collection,
        }
    }
}

/// One collection an attached subsystem retains, as `MemoryReport` carries it.
///
/// The subsystem and the collection stay separate fields rather than one fused
/// display string, so a caller looks a figure up by what it is instead of by
/// how the report happens to print it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct SubsystemMemory {
    /// The subsystem that reported it, e.g. `"voip"`.
    pub subsystem: &'static str,
    /// The collection within that subsystem, e.g. `"active_calls"`.
    pub collection: &'static str,
    pub stats: CollectionStats,
}

impl MemoryReport {
    /// Common byte-carrying collections used by both totals and `Display`.
    /// Feature-specific collections stay beside their gated report section.
    fn collections(&self) -> [(&'static str, &CollectionStats); 16] {
        [
            ("group_cache:", &self.group_cache),
            ("device_registry_cache:", &self.device_registry_cache),
            ("recent_messages:", &self.recent_messages),
            ("sk_device_cache:", &self.sender_key_device_cache),
            ("group_devices_memo:", &self.group_devices_memo),
            ("dm_devices_memo:", &self.dm_devices_memo),
            ("lid_pn (lid):", &self.lid_pn_lid_entries),
            ("lid_pn (pn):", &self.lid_pn_pn_entries),
            ("lid_pn (hash):", &self.lid_pn_contact_hash_entries),
            ("lid_pn (persisted):", &self.lid_pn_persisted_entries),
            ("signal_sessions:", &self.signal_sessions),
            ("signal_identities:", &self.signal_identities),
            ("signal_sender_keys:", &self.signal_sender_keys),
            ("history_sync_tasks:", &self.history_sync_tasks),
            ("inbound_commit_batch:", &self.inbound_commit_batch),
            ("offline_receipts:", &self.offline_receipt_buffer),
        ]
    }

    /// One collection of one attached subsystem. `None` when that subsystem is
    /// not attached to this build, or does not report that collection.
    pub fn subsystem(&self, which: SubsystemCollection) -> Option<CollectionStats> {
        self.subsystems
            .iter()
            .find(|retained| {
                retained.subsystem == which.subsystem && retained.collection == which.collection
            })
            .map(|retained| retained.stats)
    }

    /// Sum of every estimated byte figure in the report.
    pub fn total_estimated_bytes(&self) -> u64 {
        let total: u64 = self.collections().iter().map(|(_, c)| c.bytes).sum();
        let total = self.subsystems.iter().fold(total, |sum, retained| {
            sum.saturating_add(retained.stats.bytes)
        });
        #[cfg(feature = "plugins")]
        let total = total.saturating_add(self.plugin_event_queue.bytes);
        total
    }

    /// The collections whose only bound is a drain or a lifecycle, by name
    /// and entry count — the set a long-running soak compares between
    /// snapshots. Kept here, beside the fields, so a collection added to the
    /// report is added to the growth check in the same place, rather than to
    /// a hand-copied list in a test that nothing keeps in step.
    ///
    /// Capacity-bounded caches are left out: they can only ever read their
    /// cap. `lid_pn_*` maps are in, since their bound is the contact list.
    pub fn unbounded_counts(&self) -> Vec<(&'static str, u64)> {
        let n = |v: usize| u64::try_from(v).unwrap_or(u64::MAX);
        vec![
            ("lid_pn_lid_entries", self.lid_pn_lid_entries.entries),
            ("lid_pn_pn_entries", self.lid_pn_pn_entries.entries),
            (
                "lid_pn_contact_hash_entries",
                self.lid_pn_contact_hash_entries.entries,
            ),
            (
                "lid_pn_persisted_entries",
                self.lid_pn_persisted_entries.entries,
            ),
            ("history_sync_tasks", self.history_sync_tasks.entries),
            ("inbound_commit_batch", self.inbound_commit_batch.entries),
            ("msg_secret_buffer", n(self.msg_secret_buffer)),
            ("pending_device_sync", n(self.pending_device_sync)),
            ("ensure_inflight", self.ensure_inflight),
            ("group_metadata_inflight", self.group_metadata_inflight),
            ("chat_lane_backlog", self.chat_lane_backlog),
            ("transport_ack_queue", n(self.transport_ack_queue)),
            ("delivery_receipt_queue", n(self.delivery_receipt_queue)),
            ("response_waiters", n(self.response_waiters)),
            ("node_waiters", n(self.node_waiters)),
            ("sent_node_waiters", n(self.sent_node_waiters)),
            ("pending_retries", n(self.pending_retries)),
            ("pending_lid_refreshes", n(self.pending_lid_refreshes)),
            ("presence_subscriptions", n(self.presence_subscriptions)),
            ("app_state_key_requests", n(self.app_state_key_requests)),
            ("app_state_key_cache", n(self.app_state_key_cache)),
            (
                "app_state_recovery_requests",
                n(self.app_state_recovery_requests),
            ),
            ("app_state_syncing", n(self.app_state_syncing)),
            ("signal_sessions", self.signal_sessions.entries),
            ("signal_identities", self.signal_identities.entries),
            ("signal_sender_keys", self.signal_sender_keys.entries),
            ("chatstate_handlers", n(self.chatstate_handlers)),
            ("custom_enc_handlers", n(self.custom_enc_handlers)),
            ("stanza_interceptors", n(self.stanza_interceptors)),
        ]
    }
}

impl std::fmt::Display for MemoryReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn line(
            f: &mut std::fmt::Formatter<'_>,
            name: &str,
            c: &CollectionStats,
        ) -> std::fmt::Result {
            writeln!(f, "  {name:<22} {:>7} entries {:>10} B", c.entries, c.bytes)
        }
        // First TTL_BOUNDED entries of collections() are the TTL-bounded
        // caches; the next LID_PN_MAPS are the LID/PN maps, which have no
        // TTL and are bounded by the contact list, so they get their own
        // heading rather than passing as bounded-cache activity; the next
        // SIGNAL_CACHES are Signal store caches. The rest are transient
        // retention: history sync, the inbound commit batch, then the offline
        // receipt buffer. Adding a cache to collections() means moving this
        // boundary, or the sections shift.
        const TTL_BOUNDED: usize = 6;
        const LID_PN_MAPS: usize = 4;
        const LID_PN_END: usize = TTL_BOUNDED + LID_PN_MAPS;
        const SIGNAL_CACHES: usize = 3;
        const HISTORY_SYNC: usize = LID_PN_END + SIGNAL_CACHES;
        const COMMIT_BATCH: usize = HISTORY_SYNC + 1;
        const OFFLINE_RECEIPTS: usize = COMMIT_BATCH + 1;
        let collections = self.collections();
        writeln!(f, "=== Memory Report ===")?;
        writeln!(f, "--- TTL-bounded caches ---")?;
        for (name, c) in &collections[..TTL_BOUNDED] {
            line(f, name, c)?;
        }
        writeln!(f, "  message_retry_counts:   {}", self.message_retry_counts)?;
        writeln!(
            f,
            "  undec_dispatched:       {}",
            self.undecryptable_dispatched
        )?;
        writeln!(f, "  dispatched_messages:    {}", self.dispatched_messages)?;
        writeln!(f, "  pdo_pending_requests:   {}", self.pdo_pending_requests)?;
        writeln!(f, "  pdo_requested:          {}", self.pdo_requested)?;
        writeln!(f, "--- LID/PN maps (bounded by the contact list) ---")?;
        for (name, c) in &collections[TTL_BOUNDED..LID_PN_END] {
            line(f, name, c)?;
        }
        writeln!(f, "--- Capacity-only caches ---")?;
        writeln!(f, "  session_locks:          {}", self.session_locks)?;
        writeln!(f, "  ensure_inflight:        {}", self.ensure_inflight)?;
        writeln!(
            f,
            "  group_metadata_inflight:{}",
            self.group_metadata_inflight
        )?;
        writeln!(f, "  chat_lanes:             {}", self.chat_lanes)?;
        writeln!(
            f,
            "  group_dist_locks:       {} (evicted: {}, blocked: {})",
            self.group_distribution_locks,
            self.group_distribution_lock_evictions,
            self.group_distribution_lock_eviction_blocks
        )?;
        writeln!(
            f,
            "  resend_rl_chats:        {}",
            self.resend_rate_limiter_chats
        )?;
        writeln!(
            f,
            "  session_recreate_hist:  {}",
            self.session_recreate_history
        )?;
        writeln!(f, "  skdm_warm_memo:         {}", self.skdm_warm_memo)?;
        writeln!(f, "--- Unbounded collections ---")?;
        writeln!(f, "  chat_lane_backlog:      {}", self.chat_lane_backlog)?;
        writeln!(f, "  transport_ack_queue:    {}", self.transport_ack_queue)?;
        writeln!(
            f,
            "  delivery_receipt_queue: {}",
            self.delivery_receipt_queue
        )?;
        writeln!(f, "  response_waiters:       {}", self.response_waiters)?;
        writeln!(f, "  node_waiters:           {}", self.node_waiters)?;
        writeln!(f, "  sent_node_waiters:      {}", self.sent_node_waiters)?;
        writeln!(f, "  pending_retries:        {}", self.pending_retries)?;
        writeln!(
            f,
            "  pending_lid_refreshes:  {}",
            self.pending_lid_refreshes
        )?;
        writeln!(
            f,
            "  presence_subscriptions: {}",
            self.presence_subscriptions
        )?;
        writeln!(
            f,
            "  app_state_key_requests: {}",
            self.app_state_key_requests
        )?;
        writeln!(f, "  app_state_key_cache:    {}", self.app_state_key_cache)?;
        writeln!(
            f,
            "  app_state_recovery:     {}",
            self.app_state_recovery_requests
        )?;
        writeln!(f, "  app_state_syncing:      {}", self.app_state_syncing)?;
        writeln!(f, "--- Signal store caches ---")?;
        for (name, c) in &collections[LID_PN_END..LID_PN_END + SIGNAL_CACHES] {
            line(f, name, c)?;
        }
        if !self.subsystems.is_empty() {
            writeln!(f, "--- Optional subsystems ---")?;
            for retained in &self.subsystems {
                let name = format!("{} {}:", retained.subsystem, retained.collection);
                line(f, &name, &retained.stats)?;
            }
        }
        writeln!(f, "--- In-flight history sync ---")?;
        line(f, collections[HISTORY_SYNC].0, &self.history_sync_tasks)?;
        writeln!(
            f,
            "  peak tasks:             {}",
            self.history_sync_tasks_peak
        )?;
        writeln!(
            f,
            "  peak payload storage:   {} B",
            self.history_sync_payload_bytes_peak
        )?;
        writeln!(f, "--- Transient retention ---")?;
        line(f, collections[COMMIT_BATCH].0, &self.inbound_commit_batch)?;
        line(
            f,
            collections[OFFLINE_RECEIPTS].0,
            &self.offline_receipt_buffer,
        )?;
        writeln!(f, "  msg_secret_buffer:      {}", self.msg_secret_buffer)?;
        writeln!(f, "  pending_device_sync:    {}", self.pending_device_sync)?;
        #[cfg(feature = "plugins")]
        {
            writeln!(f, "--- Plugins ---")?;
            writeln!(f, "  installed:              {}", self.plugins)?;
            writeln!(f, "  install tasks:          {}", self.plugin_install_tasks)?;
            writeln!(
                f,
                "  connection tasks:       {} (generations: {})",
                self.plugin_connection_tasks, self.plugin_connection_generations
            )?;
            writeln!(
                f,
                "  core subscriptions:     {}",
                self.plugin_core_event_subscriptions
            )?;
            writeln!(
                f,
                "  stanza interceptors:    {}",
                self.plugin_stanza_interceptors
            )?;
            writeln!(
                f,
                "  event endpoints:        {} (capacity: {})",
                self.plugin_event_endpoints, self.plugin_event_endpoint_capacity
            )?;
            line(f, "event_queue:", &self.plugin_event_queue)?;
        }
        writeln!(f, "--- Misc ---")?;
        writeln!(f, "  chatstate_handlers:     {}", self.chatstate_handlers)?;
        writeln!(f, "  custom_enc_handlers:    {}", self.custom_enc_handlers)?;
        writeln!(f, "  stanza_interceptors:    {}", self.stanza_interceptors)?;
        writeln!(
            f,
            "  total estimated:        {} B",
            self.total_estimated_bytes()
        )?;
        Ok(())
    }
}

/// Unified per-session resource estimate: the client's own collections plus the
/// components that live *outside* the `Client` and dominate real per-session
/// RAM — the storage backend, transport, and HTTP client — and an optional
/// allocation-churn snapshot.
///
/// Obtain one from [`Client::resource_report`]. Each out-of-client component
/// fills only what it can introspect (see the per-field types), so absent
/// figures mean "not reported", not "zero".
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ResourceReport {
    /// The client's own in-process collections — identical to
    /// [`Client::memory_report`].
    pub client: MemoryReport,
    /// Storage-backend footprint (SQLite page cache, etc.). All-`None` for
    /// backends that don't report.
    pub storage: StorageResourceReport,
    /// Transport buffers + TLS/noise state, if the transport reports them.
    pub transport: Option<TransportResourceReport>,
    /// HTTP connection-pool + in-flight footprint, if the client reports it.
    pub http: Option<HttpResourceReport>,
    /// Allocation churn attributed to this client's instrumented work, present
    /// only when an [`AllocMeter`](wacore::stats::AllocMeter) was installed via
    /// `BotBuilder::with_alloc_meter`. It is a churn/attribution signal, not a
    /// retained figure, so it is deliberately excluded from
    /// [`Self::total_estimated_bytes`].
    pub alloc: Option<AllocSnapshot>,
}

impl ResourceReport {
    /// Best-effort sum of **retained** bytes across the present point-in-time
    /// components (client collections + storage + transport + HTTP).
    ///
    /// Exactness varies by component and this is a **lower bound** overall:
    /// - client collections and transport/HTTP buffers are honest estimates;
    /// - storage `memory_bytes` is an upper bound on the SQLite page cache
    ///   (`min(cache cap, db size)`), 0 for remote backends;
    /// - components reporting `None` contribute 0 (absent, not zero);
    /// - `alloc` (churn, not residency) is excluded.
    pub fn total_estimated_bytes(&self) -> u64 {
        // Saturating: a caller-built or backend-supplied component could carry a
        // large value; the total must stay conservative, never wrap.
        self.client
            .total_estimated_bytes()
            .saturating_add(self.storage.total_bytes())
            .saturating_add(self.transport.map_or(0, |t| t.total_bytes()))
            .saturating_add(self.http.map_or(0, |h| h.total_bytes()))
    }
}

impl std::fmt::Display for ResourceReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Resource Report ===")?;
        writeln!(
            f,
            "  client collections:     {:>10} B",
            self.client.total_estimated_bytes()
        )?;
        writeln!(
            f,
            "  storage backend:        {:>10} B (pages: {:?})",
            self.storage.total_bytes(),
            self.storage.pages
        )?;
        writeln!(
            f,
            "  transport:              {:>10} B",
            self.transport.map_or(0, |t| t.total_bytes())
        )?;
        writeln!(
            f,
            "  http client:            {:>10} B",
            self.http.map_or(0, |h| h.total_bytes())
        )?;
        if let Some(alloc) = self.alloc {
            writeln!(
                f,
                "  alloc churn:            {:>10} B allocated / {:>10} B freed ({} allocs)",
                alloc.allocated_bytes, alloc.freed_bytes, alloc.allocations
            )?;
        }
        writeln!(
            f,
            "  total retained (lower bound): {} B",
            self.total_estimated_bytes()
        )?;
        Ok(())
    }
}

/// Shared base error for transport/connection concerns.
///
/// The DRY foundation every per-domain error builds on (each domain embeds it
/// via `#[from]`): it carries the cases common to every network operation —
/// `NotConnected`, `NotLoggedIn`, IQ failures, socket / encrypt-send errors. It
/// is NOT an umbrella over the whole API; the per-domain typed errors remain
/// the public return types.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientError {
    #[error("client is not connected")]
    NotConnected,
    #[error("socket error: {0}")]
    Socket(#[from] SocketError),
    #[error("encrypt/send error: {0}")]
    EncryptSend(#[from] EncryptSendError),
    #[error("client is not logged in")]
    NotLoggedIn,
    #[error("IQ request failed: {0}")]
    Iq(#[from] crate::request::IqError),
    /// Last-resort catch-all for internal failures threaded through `?` that do
    /// not (yet) have a dedicated variant. `Display` forwards to the inner
    /// error while `source()` still exposes it for downcast.
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

/// The step of the connect flow a [`ConnectError::Timeout`] refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConnectStage {
    /// Resolving the app version advertised to the server.
    VersionFetch,
    /// Opening the underlying transport.
    Transport,
    /// Waiting for the noise socket, which is ready before login.
    Socket,
    /// Waiting for login plus the critical app state sync to finish.
    Ready,
}

impl std::fmt::Display for ConnectStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stage = match self {
            ConnectStage::VersionFetch => "version fetch",
            ConnectStage::Transport => "transport connect",
            ConnectStage::Socket => "socket wait",
            ConnectStage::Ready => "connection wait",
        };
        f.write_str(stage)
    }
}

/// Failure modes of [`Client::connect`] and of the readiness waiters
/// ([`Client::wait_for_socket`], [`Client::wait_for_connected`]).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConnectError {
    /// A connection is already up, or another attempt is already in flight.
    #[error("client is already connected")]
    AlreadyConnected,
    /// Construction never completed, so the attempt was rejected before any I/O.
    #[error("client construction did not activate")]
    NotActivated,
    /// The client was shut down. Shutdown is final, so build a new client
    /// rather than reconnecting this one.
    #[error("client has been shut down")]
    Shutdown,
    /// [`Client::pause`] is in effect. Unlike [`Self::Shutdown`] this is not
    /// final: [`Client::resume`] lifts it and connecting works again.
    #[error("client is paused")]
    Paused,
    /// A step of the connect flow ran out of time.
    #[error("{stage} timed out after {timeout:?}")]
    Timeout {
        stage: ConnectStage,
        timeout: Duration,
    },
    /// The app version could not be resolved.
    #[error("failed to resolve app version")]
    Version(#[source] anyhow::Error),
    /// The transport factory could not open a connection.
    #[error("failed to open transport")]
    Transport(#[source] anyhow::Error),
    /// The noise handshake failed after the transport was up.
    #[error("{0}")]
    Handshake(#[from] handshake::HandshakeError),
}

/// Failures of the background Signal maintenance surface: signed pre-key
/// rotation ([`Client::rotate_signed_pre_key`]) and cache durability
/// ([`Client::flush_pending_signal_state`]).
///
/// The split that matters to a caller is corruption versus everything else:
/// [`Self::CorruptKey`] will keep failing until the stored material is
/// replaced, while storage, IQ and drain failures are worth retrying.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SignalMaintenanceError {
    /// Key material is unusable: bad encoding, or a missing/wrong-sized field.
    /// Almost always a staged record that a retry would read back identically.
    #[error("corrupt signed pre-key material: {0}")]
    CorruptKey(String),
    /// The storage backend failed a read, write or flush.
    #[error("signal storage failure: {0}")]
    Storage(#[source] anyhow::Error),
    /// The rotation IQ was rejected by the server or never reached it.
    #[error("IQ request failed: {0}")]
    Iq(#[from] crate::request::IqError),
    /// A Signal primitive failed (e.g. signing the new signed pre-key).
    #[error("{0}")]
    Signal(#[from] wacore::libsignal::protocol::SignalProtocolError),
    /// The inbound drain batch could not be committed, so the Signal cache was
    /// left unflushed on purpose and the server redelivers those messages.
    #[error(
        "inbound drain batch commit failed; Signal cache left unflushed so the server redelivers"
    )]
    DrainCommitFailed,
    /// The client is going away while an inbound drain is active; flushing
    /// there would persist ratchet advances whose messages have no durable row.
    #[error("client dropping while inbound drain is active; skipping Signal flush")]
    DrainShuttingDown,
}

impl ConnectError {
    /// A step of the connect flow ran out of time.
    ///
    /// Matched exhaustively so a new variant has to be classified here rather
    /// than defaulting to "not a timeout" unnoticed.
    pub fn is_timeout(&self) -> bool {
        match self {
            ConnectError::Timeout { .. } => true,
            ConnectError::Handshake(handshake) => handshake.is_timeout(),
            ConnectError::AlreadyConnected
            | ConnectError::NotActivated
            | ConnectError::Shutdown
            | ConnectError::Paused
            | ConnectError::Version(_)
            | ConnectError::Transport(_) => false,
        }
    }
}

impl ClientError {
    pub fn is_transport_unavailable(&self) -> bool {
        match self {
            ClientError::NotConnected => true,
            ClientError::EncryptSend(e) => e.is_transport_unavailable(),
            ClientError::Iq(e) => e.is_transport_unavailable(),
            _ => false,
        }
    }
}

use wacore::types::message::ChatMessageId;

/// Metrics for tracking offline sync progress
#[derive(Debug)]
pub(crate) struct OfflineSyncMetrics {
    pub active: AtomicBool,
    pub total_messages: AtomicUsize,
    pub processed_messages: AtomicUsize,
    // Using simple std Mutex for timestamp as it's rarely contended and non-async
    pub start_time: std::sync::Mutex<Option<wacore::time::Instant>>,
}

type ResponseWaiterSender = futures::channel::oneshot::Sender<Arc<wacore_binary::OwnedNodeRef>>;

/// What a streaming waiter is handed when its response arrives.
pub(crate) enum StreamedResponse<'s, 'a> {
    /// An `<iq type="result">` decoded on demand: the stream stands inside the
    /// root element, before its first child.
    Stream(&'s mut wacore_binary::NodeStream<'a>),
    /// The response held whole: an error stanza, or a session with an observer
    /// that needs every node as a tree.
    Node(&'s Arc<wacore_binary::OwnedNodeRef>),
}

/// The consumer of a streamed IQ response. Runs on the read loop, once.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type StreamSink = Box<dyn FnOnce(StreamedResponse<'_, '_>) + Send>;
#[cfg(target_arch = "wasm32")]
pub(crate) type StreamSink = Box<dyn FnOnce(StreamedResponse<'_, '_>)>;

/// What a pending ack/IQ entry is waiting to do once the response arrives.
///
/// A phash check used to be an `Iq` waiter plus a spawned task holding the
/// receiver and a ten second timer, which is a task, a channel and a timer per
/// outgoing message for a comparison that almost always succeeds. Carrying the
/// expected value in the map instead lets the read loop compare it inline and
/// spawn only on the rare mismatch.
pub(crate) enum ResponseWaiter {
    /// Classic request/response: hand the node to whoever is awaiting it.
    Iq(ResponseWaiterSender),
    /// Compare the server's `phash` against ours; act only if they differ.
    Phash(PhashWaiter),
    /// Consume the response on the read loop as it is decoded, so a response
    /// larger than the heap can afford as a tree never becomes one. See
    /// [`Client::execute_streaming`].
    Stream(StreamSink),
}

pub(crate) struct PhashWaiter {
    pub(crate) expected: wacore_binary::CompactString,
    pub(crate) jid: Jid,
    pub(crate) invalidate_group_cache: bool,
    /// DM sends only: the device set this stanza actually covered, shared with
    /// the send's own memo entry rather than copied. A mismatch resends to the
    /// devices a refreshed list holds and this one does not, which is what the
    /// `excludeList` in WA Web's `resendUserMsg` job amounts to.
    pub(crate) dm_devices: Option<Arc<wacore::send::ResolvedDmDevices>>,
    /// Of those, the ones that produced no `<enc>`. Empty on a complete
    /// fan-out, so the common send stores nothing: a device the stanza named
    /// but could not encrypt for holds no copy, and the repair has to resend
    /// to it rather than count it as already covered.
    pub(crate) dm_unreached: Vec<Jid>,
    /// Sweep epoch this waiter was registered in. Expiry is counted in sweeps
    /// rather than seconds: a wall deadline is subject to clock jumps (see
    /// wacore::time) and would have to be derived from an instant sampled well
    /// before registration, while reading a fresh clock here is what the send
    /// clock budget forbids. Surviving one full sweep is the trigger, so the
    /// window is one keepalive tick (15 to 30 s) instead of the old fixed 10 s.
    pub(crate) registered_epoch: u64,
}

struct ResponseWaiterEntry {
    generation: NonZeroU64,
    waiter: ResponseWaiter,
}

/// Map of pending IQ/ack response waiters, keyed by request id.
///
/// Every registration carries a unique generation so guarded IQ cleanup cannot
/// remove a newer waiter that reused the same explicit ID.
#[derive(Default)]
pub(crate) struct ResponseWaiterMap {
    entries: HashMap<String, ResponseWaiterEntry>,
    last_generation: u64,
    /// Advanced once per sweep. Registration reads it under the lock it already
    /// takes, so a waiter records its age without touching a clock.
    sweep_epoch: u64,
    /// How many `Stream` entries are in the map, mirrored outside the lock so
    /// the read loop can tell whether a frame might be one to stream without
    /// taking it per frame. Maintained here, by every path that adds or
    /// removes an entry, so no caller keeps its own count.
    stream_waiters: Arc<AtomicUsize>,
}

impl ResponseWaiterMap {
    /// A map whose `Stream` count is published through `counter`.
    pub(crate) fn with_stream_counter(counter: Arc<AtomicUsize>) -> Self {
        Self {
            stream_waiters: counter,
            ..Self::default()
        }
    }

    fn note_added(&self, waiter: &ResponseWaiter) {
        if matches!(waiter, ResponseWaiter::Stream(_)) {
            self.stream_waiters.fetch_add(1, Ordering::AcqRel);
        }
    }

    fn note_removed(&self, waiter: &ResponseWaiter) {
        if matches!(waiter, ResponseWaiter::Stream(_)) {
            self.stream_waiters.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Take the `Stream` waiter registered under `request_id`, leaving any
    /// other kind of waiter where it is.
    pub(crate) fn take_stream(&mut self, request_id: &str) -> Option<StreamSink> {
        if !matches!(
            self.entries.get(request_id).map(|entry| &entry.waiter),
            Some(ResponseWaiter::Stream(_))
        ) {
            return None;
        }
        match self.remove(request_id) {
            Some(ResponseWaiter::Stream(sink)) => Some(sink),
            _ => None,
        }
    }
    fn next_generation(&mut self) -> NonZeroU64 {
        loop {
            self.last_generation = self.last_generation.wrapping_add(1);
            if let Some(generation) = NonZeroU64::new(self.last_generation) {
                return generation;
            }
        }
    }

    pub(crate) fn try_insert_guarded(
        &mut self,
        request_id: String,
        waiter: ResponseWaiter,
    ) -> Option<NonZeroU64> {
        if self.entries.contains_key(&request_id) {
            return None;
        }
        let generation = self.next_generation();
        self.note_added(&waiter);
        self.entries
            .insert(request_id, ResponseWaiterEntry { generation, waiter });
        Some(generation)
    }

    pub(crate) fn insert(
        &mut self,
        request_id: String,
        waiter: ResponseWaiter,
    ) -> Option<ResponseWaiter> {
        let generation = self.next_generation();
        self.note_added(&waiter);
        let replaced = self
            .entries
            .insert(request_id, ResponseWaiterEntry { generation, waiter })
            .map(|entry| entry.waiter);
        if let Some(replaced) = &replaced {
            self.note_removed(replaced);
        }
        replaced
    }

    pub(crate) fn remove(&mut self, request_id: &str) -> Option<ResponseWaiter> {
        let removed = self.entries.remove(request_id).map(|entry| entry.waiter);
        if let Some(removed) = &removed {
            self.note_removed(removed);
        }
        removed
    }

    /// The epoch a waiter registered now belongs to.
    pub(crate) fn current_epoch(&self) -> u64 {
        self.sweep_epoch
    }

    /// Drop phash waiters that lived through a whole sweep without their ack.
    ///
    /// Runs on the keepalive tick, before the recent-activity early return: a
    /// connection with steady inbound traffic skips the ping entirely, and
    /// sweeping only inside the ping would let lost acks accumulate for as long
    /// as traffic keeps flowing. The map is also what makes keepalive treat the
    /// connection as "IQs pending", so a stranded waiter silences pings.
    pub(crate) fn drop_expired_phash(&mut self) {
        let epoch = self.sweep_epoch;
        self.entries.retain(|_, entry| match &entry.waiter {
            ResponseWaiter::Phash(waiter) => waiter.registered_epoch >= epoch,
            ResponseWaiter::Iq(_) | ResponseWaiter::Stream(_) => true,
        });
        self.sweep_epoch = self.sweep_epoch.wrapping_add(1);
    }

    pub(crate) fn remove_guarded(&mut self, request_id: &str, cleanup_generation: NonZeroU64) {
        if self
            .entries
            .get(request_id)
            .is_some_and(|entry| entry.generation == cleanup_generation)
        {
            self.remove(request_id);
        }
    }

    /// Drop every pending sender and release the map allocation without
    /// resetting the generation sequence. Guards owned by the drained requests
    /// may outlive a disconnect and must never match a later registration.
    pub(crate) fn clear(&mut self) {
        self.entries = HashMap::new();
        self.stream_waiters.store(0, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn contains_key(&self, request_id: &str) -> bool {
        self.entries.contains_key(request_id)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}

/// A single WhatsApp session: the connection, the Signal state, and every
/// protocol operation built on top of them.
///
/// This is the low-level entry point. Build one with
/// [`ClientBuilder`], which
/// takes the four platform dependencies (storage backend, transport factory,
/// HTTP client, async runtime) and validates them at runtime. Most applications
/// should use [`Bot`](crate::bot::Bot) instead and reach the client through
/// [`Bot::client`](crate::bot::Bot::client); `Client` is what remains when you
/// need to drive the lifecycle yourself, from an FFI host, or from a wrapper
/// that cannot express typestate generics.
///
/// The client is always used behind an `Arc` (most methods take `self: &Arc<Self>`)
/// and is cheap to clone and share across tasks.
///
/// # Lifecycle
///
/// [`Client::run`] owns the session: it connects, keeps the socket alive, and
/// reconnects with backoff until [`Client::disconnect`] is called or the device
/// is logged out. [`Client::connect`] performs a single connection attempt
/// without the supervision loop, for hosts that manage retries themselves.
///
/// # Events
///
/// Everything the server reports (messages, receipts, pairing progress,
/// connection state) is delivered as an [`Event`]
/// on the event bus. Register a handler with [`Client::subscribe`] (explicit
/// [`EventInterest`](wacore::types::events::EventInterest) filter) or
/// [`Client::subscribe_handler`].
///
/// # Sending
///
/// [`Client::send_message`] covers the common path;
/// [`Client::send_message_with_options`] takes a [`SendOptions`](crate::send::SendOptions)
/// for message-id pinning, ephemeral expiration, and cache freshness. Domain
/// operations hang off accessors such as [`Client::groups`], [`Client::contacts`],
/// and [`Client::presence`].
pub struct Client {
    pub(crate) runtime: Arc<dyn Runtime>,
    pub(crate) core: wacore::client::CoreClient,

    pub(crate) persistence_manager: Arc<PersistenceManager>,
    /// Write-behind buffer for inbound messageSecret captures; readers check
    /// it before the backend so the durable write can leave the receive lane.
    pub(crate) msg_secret_buffer: Arc<crate::msg_secret_buffer::MsgSecretWriteBuffer>,
    /// Accumulates decrypted messages during the offline drain for per-batch
    /// commit (WA Web MessageProcessorCache parity).
    pub(crate) inbound_commit_batch: crate::message::commit_batch::InboundCommitBatcher,
    pub(crate) media_conn: Arc<RwLock<Option<crate::mediaconn::MediaConn>>>,
    /// The one in-flight `refresh_media_conn` fetch, shared by concurrent
    /// callers; `None` when no fetch is running. See `crate::mediaconn`.
    pub(crate) media_conn_flight:
        Arc<std::sync::Mutex<Option<Arc<crate::mediaconn::MediaConnFlight>>>>,
    /// Fetch sequence for `media_conn`: tagged at fetch start so a response
    /// that lost its race only publishes when nothing newer began after it.
    pub(crate) media_conn_seq: AtomicU64,
    /// Test gate: parks a fetch after it takes the `media_conn` publication
    /// lock but before it re-checks the sequence, so a test can drive a newer
    /// fetch past it and prove the older answer cannot publish.
    #[cfg(test)]
    pub(crate) media_conn_test_block_store: AtomicBool,
    /// Counts entries into the parked publication above.
    #[cfg(test)]
    pub(crate) media_conn_test_in_store: AtomicU32,

    pub(crate) is_logged_in: AtomicBool,
    #[cfg(feature = "client-lifecycle")]
    pub(crate) login_transition: std::sync::Mutex<()>,
    pub(crate) is_connecting: AtomicBool,
    pub(crate) is_running: AtomicBool,
    /// Whether the noise socket is established (connected to WhatsApp servers).
    /// Uses an AtomicBool instead of probing the noise_socket mutex to avoid
    /// TOCTOU races where `try_lock()` fails due to contention, not disconnection.
    is_connected: AtomicBool,

    /// whatsmeow's `sendActiveReceipts`: 0 = inactive (default), 1 = active
    /// (presence available), 2 = forced. When 0, delivery receipts use `type="inactive"`.
    send_active_receipts: AtomicU32,

    /// Per-process counter of consecutive Noise IK handshake failures, scoped
    /// to the lifetime of this `Client`. Mirrors `K` in WA Web's
    /// `WAWebOpenChatSocket` (`ChatSocket.js`): on the first failure within a
    /// process, the next connect skips IK and falls back to XX so a stale
    /// cached `serverStaticPublic` doesn't trap us in a loop. Reset to 0 on
    /// any successful handshake (XX, IK, or XXfallback).
    pub(crate) ik_handshake_failures: AtomicU32,
    /// Terminal shutdown (process-wide). Fired ONLY by `disconnect()`.
    /// Long-lived subscribers that must outlive reconnect cycles (saver,
    /// device registry cleanup) subscribe here.
    pub(crate) shutdown_notifier: wacore::runtime::ShutdownNotifier,

    /// Per-connection shutdown. Replaced with a fresh notifier on every new
    /// connection; fired on cleanup_connection_state / stream end / stream
    /// error / connect_failure / disconnect. Per-connection subscribers
    /// (keepalive, request waiters, read loop, offline flush) observe this.
    pub(crate) connection_shutdown: std::sync::Mutex<wacore::runtime::ShutdownNotifier>,
    /// Allocated only when an extension host installs lifecycle callbacks.
    #[cfg(feature = "client-lifecycle")]
    lifecycle: Option<Arc<LifecycleRegistration>>,
    /// Allocated only when at least one build-time plugin is registered.
    #[cfg(feature = "plugins")]
    pub(crate) plugin_host: Option<Arc<crate::plugins::PluginHost>>,
    /// Per-session wire I/O and activity counters. Written at the transport
    /// chokepoints (noise sender task, read loop); the keepalive dead-socket
    /// watchdog reads its activity timestamps. Snapshot via [`Client::stats`].
    pub(crate) stats: Arc<wacore::stats::SessionStats>,

    pub(crate) transport: Arc<Mutex<Option<Arc<dyn crate::transport::Transport>>>>,
    pub(crate) transport_events:
        Arc<Mutex<Option<async_channel::Receiver<crate::transport::TransportEvent>>>>,
    pub(crate) transport_factory: Arc<dyn crate::transport::TransportFactory>,
    /// Replaced per connection, so not a `OnceLock` — but every critical section
    /// is a clone or a store, so a sync lock makes holding it across an `.await`
    /// a compile error on the send path rather than a review question.
    pub(crate) noise_socket: Arc<std::sync::Mutex<Option<Arc<NoiseSocket>>>>,

    /// Pending IQ/ack response waiters keyed by request id.
    ///
    /// A `std::sync::Mutex` (like the `node_waiters` sibling below): the critical
    /// section is a trivial map op never held across an `.await`, and a sync lock
    /// is what lets `ResponseWaiterGuard` remove a cancelled waiter from `Drop`
    /// (an async lock couldn't). See `send_and_wait_iq`.
    pub(crate) response_waiters: Arc<std::sync::Mutex<ResponseWaiterMap>>,
    /// How many `response_waiters` entries want their response streamed, kept
    /// by the map and read by the read loop per frame: while it is zero no
    /// frame is peeked at before it is decoded whole, so the path every other
    /// frame takes is untouched by the streaming one.
    pub(crate) stream_waiter_count: Arc<AtomicUsize>,

    /// Generic node waiters for waiting on specific stanzas by tag/attributes.
    /// Uses std::sync::Mutex (not tokio) since the critical section is trivial.
    /// Guarded by `node_waiter_count` for zero-cost when no waiters are active.
    node_waiters: std::sync::Mutex<Vec<NodeWaiter>>,
    node_waiter_count: AtomicUsize,
    /// Waiters for raw outgoing nodes before encryption.
    sent_node_waiters: std::sync::Mutex<Vec<SentNodeWaiter>>,
    sent_node_waiter_count: AtomicUsize,

    pub(crate) unique_id: String,
    pub(crate) id_counter: Arc<AtomicU64>,

    pub(crate) unified_session: crate::unified_session::UnifiedSessionManager,

    /// In-memory cache for Signal protocol state (sessions, identities, sender keys).
    /// Matches WhatsApp Web's SignalStoreCache pattern: crypto ops read/write this
    /// cache, and DB writes are flushed out of it — synchronously on the send path
    /// and coalesced on the receive path (see `signal_flush.rs`).
    pub(crate) signal_cache: Arc<crate::store::signal_cache::SignalStoreCache>,

    /// Limits message processing concurrency (1 permit during offline sync, N after).
    /// Wrapped in Mutex to allow replacing on reconnect.
    pub(crate) message_processing_semaphore: std::sync::Mutex<Arc<async_lock::Semaphore>>,
    /// Bumped on every semaphore swap so stale Arc clones are rejected.
    pub(crate) message_semaphore_generation: AtomicU64,

    /// Per-device session locks for Signal protocol operations.
    /// Prevents race conditions when multiple messages from the same sender
    /// are processed concurrently across different chats.
    /// Keys are Signal protocol address strings (e.g., "user@s.whatsapp.net:0")
    /// to match the SignalProtocolStoreAdapter's internal locking.
    pub(crate) session_locks: Cache<String, Arc<Mutex<()>>>,

    /// Addresses whose session establishment is already in flight, so a
    /// concurrent caller waits on it instead of fetching the same bundle again.
    ///
    /// An existence probe before the fetch cannot do this on its own: it answers
    /// before the IQ goes out, so every caller in a burst reads the same "no
    /// session" and every one of them fetches. Each answered bundle is then
    /// installed over the last, retiring a session the peer may still be
    /// encrypting under, and each fetch burns one of the peer's one-time
    /// prekeys. WA Web keeps the same registration in
    /// `WAWebManageE2ESessionsJob` (a module-level wid -> promise map, cleared
    /// in a `finally`).
    ///
    /// Holds only what is in flight — normally empty — so it is a plain map
    /// rather than a capacity-bounded cache.
    pub(crate) ensure_inflight: Arc<sessions::EnsureRegistry>,

    /// Group-metadata queries in flight, so a burst of callers for one group
    /// shares a single round trip. See [`GroupMetadataRegistry`] for why only
    /// `get_metadata` needs it.
    ///
    /// [`GroupMetadataRegistry`]: crate::features::GroupMetadataRegistry
    pub(crate) group_metadata_inflight: Arc<crate::features::GroupMetadataRegistry>,

    /// Per-chat lane combining enqueue lock + message queue into a single cached entry.
    /// One cache lookup instead of two per incoming message.
    pub(crate) chat_lanes: Cache<Jid, ChatLane>,

    /// Cache for LID to Phone Number mappings (bidirectional).
    /// When we receive a message with sender_lid/sender_pn attributes, we store the mapping here.
    /// This allows us to reuse existing LID-based sessions when sending replies.
    /// The cache is backed by persistent storage and warmed up on client initialization.
    pub(crate) lid_pn_cache: Arc<LidPnCache>,
    pub(crate) ab_props: Arc<wacore::store::ab_props::AbPropsCache>,

    /// Lazily built on the first group send and never replaced afterwards, so a
    /// `OnceLock` keeps the read on that path down to an atomic load.
    pub group_cache: std::sync::OnceLock<Arc<GroupCache>>,

    pub(crate) expected_disconnect: AtomicBool,
    /// Set by `reconnect()` to suppress the "Message loop exited with an error" warning.
    /// Unlike `expected_disconnect`, this does NOT skip the reconnect backoff.
    pub(crate) intentional_reconnect: AtomicBool,

    /// Connection generation counter - incremented on each new connection.
    /// Used to detect stale post-login tasks from previous connections.
    pub(crate) connection_generation: Arc<AtomicU64>,

    /// Cache for recent messages (serialized bytes) for retry functionality.
    /// Uses an in-process cache with TTL and max capacity for automatic eviction.
    pub(crate) recent_messages: Cache<ChatMessageId, Arc<Vec<u8>>>,

    pub(crate) sender_key_device_cache: crate::sender_key_device_cache::SenderKeyDeviceCache,

    pub(crate) pending_device_sync: crate::pending_device_sync::PendingDeviceSync,

    pub(crate) pending_retries: Arc<std::sync::Mutex<HashSet<String>>>,

    /// Identities with a `refresh_lid` re-resolve in flight, keyed by
    /// `(connection_generation, PN-side JID)`. A burst of sends to one stale
    /// peer is acked one message at a time, and every one of those acks carries
    /// the flag, so without this the same query would go out once per ack while
    /// the first is still pending. The generation scopes a reservation to the
    /// connection that took it, so a refresh left parked on a dead socket
    /// cannot suppress the next connection's.
    pub(crate) pending_lid_refreshes: Arc<std::sync::Mutex<HashSet<(u64, String)>>>,

    /// Track retry attempts per message to prevent infinite retry loops.
    /// Key: "{chat}:{msg_id}:{sender}", Value: retry count plus the most
    /// recent `RetryReason` we attached, fused so the decrypt-failure path
    /// does one cache write and the binary carries one cache instantiation
    /// instead of two. The reason is `None` when the count was learned from
    /// the sender's echoed stanza `count` attribute rather than a local
    /// decrypt failure; diagnostics and regression tests read it to tell
    /// which failure arm ran (the count alone can't separate NoSession from
    /// BadMac etc.). Matches WhatsApp Web's MAX_RETRY = 5 behavior.
    pub(crate) message_retry_counts:
        Cache<String, (u8, Option<wacore::protocol::retry::RetryReason>)>,

    /// Per-peer timestamp of the last forced session recreate via the
    /// "no keys + retry≥2 + >1h since last" path (whatsmeow parity).
    /// WA Web's updateLocalSignalSession only deletes on regId mismatch /
    /// base-key collision — sessions that diverged without either trigger
    /// stay stuck. This map throttles the fallback so a noisy peer can't
    /// loop us through prekey fetches.
    pub(crate) session_recreate_history: Cache<Jid, wacore::time::Instant>,

    /// Per-chat outbound resend rate limiter: bounds the aggregate resend rate
    /// to a chat (the anti-abuse signal) so a PN to LID fan-out cannot storm into
    /// AccountLocked. Throttled devices still recover via the fresh-SKDM mark.
    pub(crate) resend_rate_limiter: crate::resend_rate_limiter::ResendRateLimiter,

    /// Dispatch-once gate for `UndecryptableMessage`: a server resend of a
    /// failed id re-enters the failure path and would otherwise fire a
    /// duplicate event. Mirrors WA Web's DB-level placeholder uniqueness
    /// in `WAWebMessageProcessPlaceholder`.
    pub(crate) undecryptable_dispatched: Cache<wacore::types::message::SenderMessageId, ()>,

    /// Dispatch-once gate for a decrypted message. A sender retrying its own
    /// outbox resends one id as fresh ciphertext on a new ratchet iteration,
    /// which decrypts as new traffic, so only message identity can collapse it.
    pub(crate) dispatched_messages: Cache<wacore::types::message::SenderMessageId, ()>,

    /// Lifetime count of resent messages this gate kept from reaching
    /// consumers. Client-level, so it survives reconnects: the sender's retry
    /// window does not end because our socket did.
    pub(crate) duplicate_dispatch_suppressed: AtomicU64,

    pub enable_auto_reconnect: Arc<AtomicBool>,
    /// Set by [`Client::pause`] and cleared by [`Client::resume`]: the run loop
    /// parks instead of connecting for as long as it holds.
    ///
    /// Deliberately not part of `is_terminal`: a paused client is between
    /// connections, not finished, and the application means to come back.
    pub(crate) paused: AtomicBool,
    /// Fired by [`Client::pause`] and [`Client::resume`], and by nothing else,
    /// so the run loop's reconnect backoff can watch it without the spurious
    /// wakes that would collapse the delay it exists to serve.
    pub(crate) pause_state_notifier: event_listener::Event,
    /// Set by [`Client::pause`] for the connection it tears down, consumed by
    /// the run loop's post-connection branch. A one-shot fact rather than a
    /// re-read of `paused`, because a [`Client::resume`] can land between the
    /// two and the backoff a pause does not owe must not turn on that timing.
    pub(crate) pause_teardown_pending: AtomicBool,
    /// Bumped by every [`Client::pause`]. A connection attempt reads it once at
    /// the start and is refused if it has moved, so an attempt that spanned a
    /// pause is never published — even when a [`Client::resume`] landed while it
    /// was still handshaking and left the flag reading `false` throughout.
    pub(crate) pause_generation: AtomicU64,
    /// Held across the connect graph's final refusal-check-and-publish and
    /// across [`Client::pause`]'s capture of what it is tearing down, so a pause
    /// cannot read "no connection" from an attempt one statement short of
    /// publishing one. Deliberately covers flag and slot writes only — never
    /// network I/O — so it cannot become the kind of wait that parks a caller
    /// behind an unresponsive socket.
    pub(crate) connection_publish: Mutex<()>,
    /// Consecutive reconnect failures, drives the Fibonacci backoff. Exposed
    /// read-only via [`StatsSnapshot::reconnect_errors`](wacore::stats::StatsSnapshot).
    pub(crate) auto_reconnect_errors: AtomicU32,
    /// When the last successful authentication (`<success>`) landed, or unset.
    /// Gates the WA Web `resetDelay` backoff reset (see [`should_reset_backoff`]).
    /// Monotonic: it only ever answers how long the connection has been up, and
    /// a wall clock would let a resumed laptop declare a seconds-old connection
    /// stable, or a backwards adjustment withhold the reset indefinitely.
    pub(crate) connected_at: wacore::time::AtomicInstant,
    /// Set when an explicit backoff penalty was applied this connection (429
    /// rate-limit, manual `reconnect()`); cleared on the next `<success>`. Keeps
    /// the stability reset from erasing a deliberate penalty (WA Web `cancelReset`).
    pub(crate) backoff_reset_suppressed: AtomicBool,

    pub(crate) needs_initial_full_sync: Arc<app_state::BootstrapGate>,

    /// Built on first app-state use and never replaced: reconnect clears the
    /// processor's key cache in place rather than swapping the processor.
    pub(crate) app_state_processor: std::sync::OnceLock<Arc<AppStateProcessor>>,
    pub(crate) app_state_key_requests: Arc<Mutex<HashMap<Vec<u8>, wacore::time::Instant>>>,
    /// Tracks collections currently being synced to prevent duplicate sync tasks.
    /// Matches WA Web's in-flight tracking set in WAWebSyncdCollectionsStateMachine.
    pub(crate) app_state_syncing: Arc<app_state::SyncInFlight>,
    /// Serializes outgoing app-state patch sends.
    ///
    /// `w:sync:app:state` is optimistic-concurrency: a patch names the base
    /// version it was built on, and only one patch can win per version. Two
    /// unserialized verbs (two quick `markChatAsRead`s) build on the same base
    /// and at most one lands. One lock for every collection, rather than one
    /// per collection, matches whatsmeow's single `appStateSyncLock` and WA Web
    /// funnelling all collections through one `CollectionsStateMachine`; sends
    /// are user-paced, so there is nothing to gain from finer granularity.
    pub(crate) app_state_send_lock: Arc<Mutex<()>>,
    pub(crate) initial_keys_synced_notifier: Arc<event_listener::Event>,
    pub(crate) initial_app_state_keys_received: AtomicBool,

    /// Prevents concurrent prekey upload operations (matches WA Web's dedup set in `handlePreKeyLow`).
    pub(crate) prekey_upload_lock: Arc<Mutex<()>>,
    /// Single-flights signed pre-key rotation so overlapping post-login tasks
    /// (from reconnect churn) can't run the rotate/upload/prune flow concurrently.
    pub(crate) signed_pre_key_rotation_lock: Arc<Mutex<()>>,
    /// Notifier for when offline sync (ib offline stanza) is received.
    /// WhatsApp Web waits for this before sending passive tasks (prekey upload, active IQ, presence).
    pub(crate) offline_sync_notifier: event_listener::Event,
    /// Flag indicating offline sync has completed (received ib offline stanza).
    /// Flips only AFTER the drain-tail commit, so the tail's acks still join
    /// the aggregate offline-receipt drain.
    pub(crate) offline_sync_completed: AtomicBool,
    /// Highest connection generation whose drain finisher has started, held as
    /// `generation + 1` so zero reads as "none yet". Separate from
    /// `offline_sync_completed` because the finish runs off the read loop and
    /// that flag must flip only after its commit.
    ///
    /// A once-guard because the semaphore swap is not idempotent, and stamped
    /// with the generation for the same reason the terminal report is: a
    /// completion descheduled past its own connection would otherwise claim
    /// the boolean after a teardown cleared it, and the next connection's
    /// completion would find the guard taken and never start a finisher.
    pub(crate) offline_sync_finish_started: AtomicU64,
    /// Highest connection generation whose resume already published a terminal
    /// event, either `OfflineSyncCompleted` or `OfflineSyncInterrupted`, held
    /// as `generation + 1` so zero reads as "none yet".
    ///
    /// Monotonic rather than a boolean that something clears, because the
    /// publications race in two directions. The finisher runs detached and
    /// checks the generation before publishing, so it can pass that check and
    /// then be descheduled past a teardown, past a reconnect, and past the
    /// next drain's preview. A claim therefore fails both for a drain already
    /// reported and for one a *newer* drain has overtaken, and nothing has to
    /// reopen the guard for the next drain: its own higher stamp does that.
    pub(crate) offline_terminal_reported: AtomicU64,
    /// Delivery receipts buffered during offline sync, flushed as aggregate
    /// `<receipt>` stanzas at completion (WA Web `sendAggregateOfflineReceipts`).
    /// Empty (zero capacity) outside the offline window.
    pub(crate) offline_receipt_buffer:
        std::sync::Mutex<Vec<Arc<crate::types::message::MessageInfo>>>,
    /// Task count, retained payload storage, peaks, and idle notification for
    /// history sync work.
    pub(crate) history_sync_activity: Arc<crate::sync_task::HistorySyncActivity>,
    /// Flushed by `disconnect()`/`reconnect()` before tearing down the transport
    /// so in-flight delivery receipts aren't dropped with `NotConnected`
    /// (issue #571).
    pub(crate) outbound_flush: Arc<crate::flush_scope::FlushScope>,
    /// Feed of the persistent delivery-receipt worker (spawned on first use).
    /// Queued items carry a [`crate::flush_scope::FlushGuard`] so `flush()`
    /// still waits for receipts that are queued but not yet sent.
    pub(crate) delivery_receipt_queue: std::sync::OnceLock<
        async_channel::Sender<(
            Arc<crate::types::message::MessageInfo>,
            crate::flush_scope::FlushGuard,
        )>,
    >,
    /// Feed of the persistent transport-ack worker, mirroring
    /// [`Self::delivery_receipt_queue`]. Deferred acks used to be one spawned
    /// task each; the queue also gives them FIFO order, which the spawns did
    /// not guarantee.
    pub(crate) transport_ack_queue: std::sync::OnceLock<
        async_channel::Sender<(
            Arc<wacore_binary::OwnedNodeRef>,
            crate::flush_scope::FlushGuard,
        )>,
    >,
    /// Contacts with active presence subscriptions that must be re-subscribed on reconnect.
    pub(crate) presence_subscriptions: Arc<std::sync::Mutex<HashSet<Jid>>>,
    /// Metrics for granular offline sync logging
    pub(crate) offline_sync_metrics: Arc<OfflineSyncMetrics>,
    /// Drives the WA Web pull-batch loop for offline backlog delivery.
    pub(crate) offline_batch: Arc<offline_resume::OfflineBatchCoordinator>,
    /// Notifier for when the noise socket is established (before login).
    /// Use this to wait for the socket to be ready for sending messages.
    pub(crate) socket_ready_notifier: event_listener::Event,
    /// Set to `true` only when `dispatch_connected()` fires (once the critical
    /// sync has an answer, clean or not). Reset on each new connection attempt.
    /// Used by `wait_for_connected()` to avoid a false-positive fast path when
    /// the client is logged in but critical app state hasn't been asked for yet.
    pub(crate) is_ready: AtomicBool,
    /// Notifier for when the client is fully connected and logged in.
    /// Triggered after Event::Connected is dispatched.
    pub(crate) connected_notifier: event_listener::Event,
    /// The `connection_generation` that `<success>` finished publishing.
    ///
    /// `is_logged_in` is set by the dedup swap that has to come *before* the
    /// generation is incremented, so between those two stores a reader sees an
    /// authenticated client whose generation is about to change underneath it.
    /// Work that bound a scope in that window had every attempt rejected as
    /// retired. This lags `connection_generation` by exactly that window, so
    /// equality means the generation a caller is about to bind is the final one.
    pub(crate) authenticated_generation: AtomicU64,
    /// Fired whenever the answer to *can work reach the server, and is it still
    /// worth waiting* may have changed: the session authenticated, or the client
    /// became terminal.
    ///
    /// Neither of the other two notifiers answers that. `socket_ready_notifier`
    /// fires before login, so a waiter released by it can send an IQ the server
    /// will not answer and whose generation `<success>` then retires;
    /// `connected_notifier` fires only after the critical sync, which app-state
    /// work must not sit through because it may *be* that sync. And nothing at
    /// all announces a client that stops without a replacement socket ever
    /// arriving — the case that leaves a detached retry parked forever, holding
    /// the `Arc<Client>` whose drop would have been the only other way out.
    ///
    /// Every terminal transition must fire this. See [`Client::is_terminal`].
    pub(crate) session_state_notifier: event_listener::Event,
    pub(crate) major_sync_task_sender: async_channel::Sender<MajorSyncTask>,
    pub(crate) pairing_cancellation_tx: Arc<Mutex<Option<async_channel::Sender<()>>>>,
    /// Asks the QR rotation task to re-render the ref it is already showing.
    /// The payload embeds the adv secret, so a rotation has to reach the code
    /// on screen and not just the next one.
    pub(crate) pairing_qr_refresh_tx: Arc<Mutex<Option<async_channel::Sender<()>>>>,

    /// State machine for pair code authentication flow.
    /// Tracks the pending pair code request and ephemeral keys.
    pub(crate) pair_code_state: Arc<Mutex<wacore::pair_code::PairCodeState>>,

    /// Per-client state of every optional subsystem attached to this build,
    /// each under its own type, in one field rather than one field per
    /// subsystem. Empty, and zero-sized, in a build with none attached; see
    /// `agent_docs/subsystem_boundary.md`.
    pub(crate) subsystems: subsystem::Subsystems,

    /// Custom handlers for encrypted message types. Set once at `Bot::build` and
    /// immutable afterward, so the receive hot path reads it with a plain
    /// `OnceLock::get` (no lock) and no per-node guard acquisition.
    pub custom_enc_handlers: std::sync::OnceLock<HashMap<String, Arc<dyn EncHandler>>>,

    /// Optional inbound durability hook. When set, the transport ack for a
    /// decrypted user message is deferred until the hook commits it, converting
    /// the consumer to at-least-once delivery. Set once at `Bot::build` and read
    /// lock-free on the receive path. `None` (default) keeps the current
    /// at-most-once behavior with zero overhead.
    pub(crate) inbound_durability_hook:
        std::sync::OnceLock<Arc<dyn crate::types::durability_hook::InboundDurabilityHook>>,

    /// Optional retry-receipt admission policy (see
    /// [`crate::types::retry_admission::RetryAdmission`]): an operator opt-in to
    /// drop some group/status retries. `None` (default) keeps WA Web behavior
    /// with a single lock-free `OnceLock::get` on the receive path.
    pub(crate) retry_admission:
        std::sync::OnceLock<Arc<dyn crate::types::retry_admission::RetryAdmission>>,

    /// Chat state (typing indicator) handlers registered by external consumers.
    /// Each handler receives a `ChatStateEvent` describing the chat, optional participant and state.
    ///
    /// Copy-on-write behind a sync lock, guarded by `chatstate_handler_count` so
    /// the default (no handler registered) never takes the lock nor builds the
    /// event that only a handler would read.
    pub(crate) chatstate_handlers: Arc<std::sync::RwLock<Arc<[ChatStateHandler]>>>,
    pub(crate) chatstate_handler_count: AtomicUsize,

    pub(crate) pdo_pending_requests: Cache<ChatMessageId, crate::pdo::PendingPdoRequest>,

    /// Messages already covered by a placeholder-resend PDO request. Mirrors
    /// the session-lifetime set in
    /// `WAWebNonMessageDataRequestPlaceholderMessageResendUtils`: at most one
    /// request per message, no matter how many times the server redelivers
    /// the undecryptable original. Entries are dropped on send failure so a
    /// transient error does not block the next attempt.
    ///
    /// Keyed with the sender, unlike [`Self::pdo_pending_requests`]. This one
    /// is a purely local gate that never has to agree with anything the phone
    /// sends back, so it can name the message precisely; the pending map has
    /// to match a response and keeps the key the phone answers with.
    pub(crate) pdo_requested: Cache<wacore::types::message::SenderMessageId, ()>,

    /// LRU cache for device registry (matches WhatsApp Web's 5000 entry limit).
    /// Maps user ID to DeviceListRecord for fast device existence checks.
    /// Backed by persistent storage.
    /// Device registry fused with its topology tracker: every write records
    /// the change by construction, so the device-list memos below can never
    /// be left stale by a forgotten bump.
    pub(crate) device_registry_cache: device_topology::DeviceRegistryCache,
    /// Shared topology tracker (generation + changed-users log). LidPnCache
    /// records mapping changes into it; the memos validate against it.
    pub(crate) device_topology: Arc<device_topology::DeviceTopology>,
    /// Whether the device-list memos (group and DM) may be used: false when
    /// the registry or LID-PN caches are store-backed (a shared external
    /// store can be written by other processes, which the in-process
    /// topology tracker cannot observe).
    pub(crate) device_memos_enabled: bool,
    /// Per-group memo of the fully resolved (LID-converted) device list,
    /// validated by GroupInfo identity + the device topology. Serves the
    /// per-send full-set resolution in `resolve_skdm_targets` so a warm
    /// repeat send skips the per-member cache fan-out.
    pub(crate) group_devices_memo: Cache<Jid, Arc<device_registry::GroupDevicesMemo>>,
    /// Per-recipient memo of the resolved DM fan-out (recipient devices +
    /// own companions, partitioned, with its phash), keyed by the resolved
    /// wire jid and validated by the sending identity + the device topology.
    /// A warm repeat DM skips both registry lookups, the list rebuild and
    /// the phash.
    pub(crate) dm_devices_memo: Cache<Jid, Arc<device_registry::DmDevicesMemo>>,
    /// Full DM fan-out recomputes (memo miss or bypass), so tests can prove a
    /// repeat send really served the memo instead of redoing the resolution.
    #[cfg(test)]
    pub(crate) dm_devices_memo_recomputes: AtomicU64,

    /// Per-term hit/miss counts for the two group-path device memos above,
    /// read through [`Client::device_memo_stats`]. Which term invalidated is
    /// the only thing that distinguishes "this memo is doing its job" from
    /// "this memo has never hit", and no benchmark can observe it: a fixture
    /// that forces the outcome measures the cost of the outcome it forced.
    pub(crate) device_memo_counters: DeviceMemoCounters,

    /// Single-flight for cold SKDM distribution, keyed per group. Concurrent
    /// cold sends each re-ran the full per-member fan-out before any of them
    /// marked the devices warm; the loser now waits here and re-resolves,
    /// finding everything warm. Warm sends never touch it.
    pub(crate) group_distribution_locks: Cache<Jid, Arc<Mutex<()>>>,

    /// Last `(devices, sender-key-device map)` Arc pair whose `needs_skdm`
    /// was warm — empty, or only our own devices (which are never memoized
    /// warm and re-receive their SKDM every send; WA Web `!isMeDevice`) —
    /// plus the map's generation and that needs set, so a warm repeat send
    /// skips `filter_skdm_targets` and reuses the memoized targets. `Weak`
    /// keeps the pointer comparison ABA-safe (matching `GroupDevicesMemo`);
    /// the generation catches an in-place cold flip that leaves the `Arc`
    /// pointer unchanged.
    pub(crate) skdm_warm_memo: Cache<Jid, SkdmWarmMemoEntry>,

    /// Router for dispatching stanzas to their appropriate handlers
    pub(crate) stanza_router: crate::handlers::router::StanzaRouter,

    /// Whether to send ACKs synchronously or in a background task
    pub(crate) synchronous_ack: bool,

    /// HTTP client for making HTTP requests (media upload/download, version fetching)
    pub http_client: Arc<dyn crate::http::HttpClient>,

    /// Version override for testing or manual specification
    pub(crate) override_version: Option<(u32, u32, u32)>,

    /// The fallback the latest connect attempt settled for, reported on
    /// [`Event::Connected`]. Rewritten by every attempt, so a reconnect that
    /// reaches the source clears what a blocked one recorded.
    pub(crate) app_version_fallback:
        std::sync::Mutex<Option<wacore::types::events::AppVersionFallback>>,

    /// When true, history sync notifications are acknowledged but not downloaded
    /// or processed. Set via `BotBuilder::skip_history_sync()`.
    pub(crate) skip_history_sync: AtomicBool,

    /// Whether the A/B props catalog is fetched on connect. Set via
    /// `ClientBuilder::with_ab_props_fetch`; see there for what turning it
    /// off costs.
    pub(crate) ab_props_fetch: AtomicBool,

    /// Whether the connection lifecycle may announce `available` on its own.
    /// Set via [`BotBuilder::with_presence_policy`] or
    /// [`Client::set_presence_policy`]; explicit presence calls are unaffected.
    pub(crate) automatic_presence: AtomicBool,

    /// Number of one-time pre-keys generated per upload batch. Defaults to
    /// [`crate::prekeys::DEFAULT_WANTED_PRE_KEY_COUNT`]; set via
    /// [`BotBuilder::with_wanted_pre_key_count`] or [`Client::set_wanted_pre_key_count`].
    /// Clamped to the protocol-safe range at upload time.
    pub(crate) wanted_pre_key_count: AtomicUsize,

    /// Cache configuration for TTL and capacity of all caches.
    /// Stored for use by lazily-initialized caches (group_cache).
    pub(crate) cache_config: CacheConfig,

    /// Weak self-reference for spawning background tasks from `&self` methods.
    /// Initialized after `Arc::new(this)` in the constructor.
    pub(crate) self_weak: std::sync::OnceLock<std::sync::Weak<Client>>,

    /// Single-flight state for the coalesced Signal-cache flush worker:
    /// `(connection_generation << 2) | RUNNING/DIRTY bits` (see `signal_flush.rs`).
    pub(crate) signal_flush_state: AtomicU64,
    /// Barrier between a coalesced-flush worker's backend write and teardown's
    /// Signal-cache settle. The generation-scoped atomic only orders
    /// `signal_flush_state`, not the writes themselves: a worker that passed its
    /// pre-flush generation check could still be mid-flush when teardown settles
    /// the cache and the next connection's drain dirties it, persisting rowless
    /// advances out of band. The worker holds this only across the flush (never
    /// across sleep/backoff) and re-checks the generation under it; teardown
    /// holds it around the settle. Lock order is always this-gate → processing
    /// permit / sessions lock, so no inversion.
    pub(crate) signal_flush_lifecycle: Mutex<()>,
    /// Serializes a drain's end against the teardown that retires it.
    ///
    /// The generation stamp decides *who* reports, but the finisher runs
    /// detached, so without this its check and its publication interleave with
    /// the teardown's own resets: the winner's writes could land on either
    /// side of them, and a semaphore widened after the reset would follow the
    /// next connection into its drain. Held across the claim and everything it
    /// publishes on one side, and across the teardown's offline resets on the
    /// other.
    pub(crate) offline_terminal_lock: Mutex<()>,
    /// Injected failures for the coalesced flush (consumed one per attempt),
    /// so tests can exercise the retry/backoff path deterministically.
    #[cfg(test)]
    pub(crate) signal_flush_test_failures: AtomicU32,
    /// Blocks each coalesced flush attempt while set, so a test can hold a
    /// worker inside the flush and drive a concurrent generation change.
    #[cfg(test)]
    pub(crate) signal_flush_test_block: AtomicBool,
    /// Set by `cleanup_connection_state` immediately before it takes
    /// `offline_terminal_lock`. Lets a test prove the teardown reached the
    /// lock rather than inferring it from elapsed scheduler turns: everything
    /// the transition writes is on the far side of this point, so a test
    /// holding the lock knows nothing has been reset yet when it fires.
    #[cfg(test)]
    pub(crate) offline_terminal_gate_reached: AtomicBool,
    /// Counts entries into the coalesced flush attempt, so a test can wait
    /// until a worker is actually inside the (blocked) flush.
    #[cfg(test)]
    pub(crate) signal_flush_test_in_attempt: AtomicU32,
    /// Keeps retry regressions deterministic without corrupting the test database.
    #[cfg(test)]
    pub(crate) app_state_key_share_prepare_test_failures: AtomicU32,
    /// Counts `ChatStateEvent` constructions, so a test can prove the
    /// no-handler fast path skips the build rather than just the invoke.
    #[cfg(test)]
    pub(crate) chatstate_events_built: AtomicU32,

    /// Holds the background saver's AbortHandle so the task lifetime follows
    /// `Arc<Client>` ref count instead of the Bot wrapper's. Set once by
    /// `Bot::build`; on Client drop (last Arc), the handle drops and the saver
    /// is aborted.
    pub(crate) saver_handle: std::sync::OnceLock<wacore::runtime::AbortHandle>,

    /// Typed handle to an [`AllocMeter`](wacore::stats::AllocMeter) installed via
    /// `BotBuilder::with_alloc_meter`, so [`Client::resource_report`] can fold in
    /// its allocation-churn snapshot. Unset unless that builder method was used.
    pub(crate) alloc_meter: std::sync::OnceLock<Arc<wacore::stats::AllocMeter>>,

    /// Number of consumers currently requesting `Event::RawNode` forwarding.
    raw_node_forwarding: AtomicUsize,

    /// Number of consumers currently requesting `Event::DecryptedPayload`
    /// forwarding.
    decrypted_payload_forwarding: AtomicUsize,

    /// Number of consumers currently requesting `Event::EncDecryptFailed`
    /// forwarding. Counted apart from `decrypted_payload_forwarding` so a
    /// consumer that only watches failures does not turn on payload cloning,
    /// and one that only watches successes pays nothing on the failure paths.
    enc_decrypt_failed_forwarding: AtomicUsize,

    /// Gate and publisher for `Event::SentFrame`. Behind an `Arc` because the
    /// noise sender task reads it; see [`SentFrameTap`].
    pub(crate) sent_frame_tap: Arc<SentFrameTap>,

    /// Stanza interceptors, behind the same copy-on-write snapshot the event
    /// bus uses: reading one costs a refcount bump, so the read loop allocates
    /// nothing per stanza. Registering is the rare side, and pays the copy.
    stanza_interceptors: std::sync::RwLock<Arc<Vec<interceptor::Registration>>>,
    /// Kept alongside so the read loop can skip the lock entirely while none
    /// are registered.
    stanza_interceptor_count: AtomicUsize,
    next_interceptor_id: AtomicU64,
}

/// Builds a pong response node for a server-initiated ping.
///
/// Matches WhatsApp Web (`WAWebCommsHandleStanza`): only includes `id`
/// when the server ping carried one.
fn build_pong(to: String, id: Option<&str>) -> Node {
    let mut builder = NodeBuilder::new("iq").attr("to", to).attr("type", "result");
    if let Some(id) = id {
        builder = builder.attr("id", id);
    }
    builder.build()
}

/// Compare decoded attribute values by their wire display without allocating.
#[inline]
fn value_refs_display_equal(
    left: &wacore_binary::node::ValueRef<'_>,
    right: &wacore_binary::node::ValueRef<'_>,
) -> bool {
    use wacore_binary::node::ValueRef;

    match (left, right) {
        (ValueRef::String(left), ValueRef::String(right)) => left == right,
        (ValueRef::Jid(left), ValueRef::Jid(right)) => left.display_eq_jid(right),
        (ValueRef::String(left), ValueRef::Jid(right)) => right.display_eq(left),
        (ValueRef::Jid(left), ValueRef::String(right)) => left.display_eq(right),
    }
}

#[derive(Clone, Copy)]
enum AckParticipantPolicy {
    Preserve,
    OmitReceiptDestinationDuplicate,
}

#[inline]
fn ack_participant<'node, 'data>(
    node: &'node wacore_binary::NodeRef<'data>,
    from: &wacore_binary::node::ValueRef<'data>,
    policy: AckParticipantPolicy,
) -> Option<&'node wacore_binary::node::ValueRef<'data>> {
    node.get_attr("participant")
        .filter(|participant| match policy {
            AckParticipantPolicy::Preserve => true,
            AckParticipantPolicy::OmitReceiptDestinationDuplicate => {
                node.tag != StanzaTag::Receipt.as_str()
                    || !value_refs_display_equal(participant, from)
            }
        })
}

/// Build an `<ack/>` for the given stanza, matching WA Web / whatsmeow behavior:
///
/// - `class` = original stanza tag
/// - `id`, `to` (flipped from `from`) copied from original
/// - `participant` follows the generic or receipt-specialized policy
/// - `from` = own device PN, only for message acks
/// - `type` echoed when present, except `notification type="encrypt"`, whose
///   ack WA Web builds without one
///
/// For receipt acks, WA Web uses `MAYBE_CUSTOM_STRING(ackString)` where
/// `ackString = maybeAttrString("type")` — so `type` is only included when
/// explicitly present on the incoming receipt (delivery receipts normally
/// have no type attribute, meaning the ack also has no type).
///
/// Encode an ack stanza directly to bytes, bypassing Node + marshal_auto.
/// Acks are the most frequent outbound stanza (~1 per inbound message).
fn encode_ack_bytes(
    node: &wacore_binary::NodeRef<'_>,
    own_device_pn: Option<&Jid>,
    participant_policy: AckParticipantPolicy,
) -> Result<Vec<u8>, crate::features::StanzaResponseError> {
    use wacore_binary::encoder::{ByteWriter, EncodeNode, Encoder};

    let id_val = crate::features::required_stanza_attr(node, "id")?;
    let from_val = crate::features::required_stanza_attr(node, "from")?;
    let tag = node.tag.as_ref();
    let participant_val = ack_participant(node, from_val, participant_policy);
    // Server expects `recipient` echoed back so it can route the ack to the
    // origin companion/device (hosted-companion, peer, LID-routed stanzas).
    // Dropping it makes the server close the stream with `<stream:error><ack/>`.
    let recipient_val = node.get_attr("recipient");

    let typ_val = ack_type(node);

    // WA Web stamps the own device JID for both classes.
    let own_device_pn = if tag == StanzaTag::Message.as_str() || tag == StanzaTag::Status.as_str() {
        Some(own_device_pn.ok_or(crate::features::StanzaResponseError::MissingLocalIdentity)?)
    } else {
        None
    };

    // Count attrs: class + id + to + optional(from, participant, recipient, type)
    let attr_count = 3
        + usize::from(own_device_pn.is_some())
        + usize::from(participant_val.is_some())
        + usize::from(recipient_val.is_some())
        + usize::from(typ_val.is_some());

    struct AckNode<'a> {
        id: &'a wacore_binary::node::ValueRef<'a>,
        from: &'a wacore_binary::node::ValueRef<'a>,
        participant: Option<&'a wacore_binary::node::ValueRef<'a>>,
        recipient: Option<&'a wacore_binary::node::ValueRef<'a>>,
        typ: Option<&'a wacore_binary::node::ValueRef<'a>>,
        own_pn: Option<&'a Jid>,
        tag_str: &'a str,
        attr_count: usize,
    }

    impl EncodeNode for AckNode<'_> {
        fn tag(&self) -> &str {
            "ack"
        }
        fn attrs_len(&self) -> usize {
            self.attr_count
        }
        fn has_content(&self) -> bool {
            false
        }
        fn encode_attrs<'a, W: ByteWriter>(
            &self,
            enc: &mut Encoder<'a, W>,
        ) -> wacore_binary::Result<()> {
            enc.write_string("class")?;
            enc.write_string(self.tag_str)?;
            enc.write_string("id")?;
            self.id.encode_value(enc)?;
            enc.write_string("to")?;
            self.from.encode_value(enc)?;
            if let Some(pn) = self.own_pn {
                enc.write_string("from")?;
                enc.write_jid_owned(pn)?;
            }
            if let Some(p) = self.participant {
                enc.write_string("participant")?;
                p.encode_value(enc)?;
            }
            if let Some(r) = self.recipient {
                enc.write_string("recipient")?;
                r.encode_value(enc)?;
            }
            if let Some(t) = self.typ {
                enc.write_string("type")?;
                t.encode_value(enc)?;
            }
            Ok(())
        }
        fn encode_content<'a, W: ByteWriter>(
            &self,
            _enc: &mut Encoder<'a, W>,
        ) -> wacore_binary::Result<()> {
            Ok(())
        }
    }

    let ack = AckNode {
        id: id_val,
        from: from_val,
        participant: participant_val,
        recipient: recipient_val,
        typ: typ_val,
        own_pn: own_device_pn,
        tag_str: tag,
        attr_count,
    };

    let mut buf = Vec::with_capacity(64);
    let mut encoder = Encoder::new_vec(&mut buf)?;
    encoder.write_node(&ack)?;
    Ok(buf)
}

/// Minimal `<message>` stanza carrying the attrs `encode_ack_bytes` needs,
/// reconstructed after the node tree has been dropped. The original `from`
/// is the group for group/broadcast stanzas and the sender otherwise (sender
/// keeps the device qualifier; `chat` is device-stripped for DMs). Mirrors
/// whatsmeow's `sendAck` (`to`=from, copy recipient/participant).
fn message_ack_source_node(info: &crate::types::message::MessageInfo) -> Node {
    let from = if info.source.is_group {
        &info.source.chat
    } else {
        &info.source.sender
    };
    let mut builder = NodeBuilder::new("message")
        .attr("id", &info.id)
        .attr("from", from);
    if let Some(recipient) = &info.source.recipient {
        builder = builder.attr("recipient", recipient);
    }
    if info.source.is_group {
        builder = builder.attr("participant", &info.source.sender);
    }
    builder.build()
}

/// Build an automatic ack Node (used in tests for structure verification).
#[cfg(test)]
fn build_ack_node(node: &wacore_binary::NodeRef<'_>, own_device_pn: Option<&Jid>) -> Option<Node> {
    let id = node.get_attr("id")?.to_node_value();
    let from_ref = node.get_attr("from")?;
    let from = from_ref.to_node_value();
    let tag = node.tag.as_ref();
    let participant = ack_participant(
        node,
        from_ref,
        AckParticipantPolicy::OmitReceiptDestinationDuplicate,
    )
    .map(|value| value.to_node_value());
    let recipient = node.get_attr("recipient").map(|v| v.to_node_value());
    let typ = ack_type(node).map(|v| v.to_node_value());
    let mut attrs = Attrs::with_capacity(7);
    attrs.insert("class", NodeValue::from(tag));
    attrs.insert("id", id);
    attrs.insert("to", from);
    if tag == StanzaTag::Message.as_str()
        && let Some(own_device_pn) = own_device_pn
    {
        attrs.insert("from", NodeValue::Jid(own_device_pn.clone()));
    }
    if let Some(p) = participant {
        attrs.insert("participant", p);
    }
    if let Some(r) = recipient {
        attrs.insert("recipient", r);
    }
    if let Some(t) = typ {
        attrs.insert("type", t);
    }
    Some(Node {
        tag: Cow::Borrowed("ack"),
        attrs,
        content: None,
    })
}

/// The `type` an ack echoes back, or `None` when it must carry none.
///
/// Every WA Web handler for `<notification type="encrypt">` builds its ack as
/// `wap("ack", {to, id, class: "notification"})` with no `type` at all:
/// `WAWebHandlePreKeyLow` (`<count>`/`<pq_count>`), `WAWebHandleDigestKey`
/// (`<digest>`) and `WAWebHandleIdentityChange` (`<identity>`) all agree, and
/// the IR mirrors the same three ack shapes. Echoing `type="encrypt"` on the
/// other two was a stanza this client sends and the official one never does.
fn ack_type<'n, 'a>(
    node: &'n wacore_binary::NodeRef<'a>,
) -> Option<&'n wacore_binary::node::ValueRef<'a>> {
    if is_encrypt_notification(node) {
        None
    } else {
        node.get_attr("type")
    }
}

/// Whether `node` is a `<notification type="encrypt">`, whatever child it
/// carries.
fn is_encrypt_notification(node: &wacore_binary::NodeRef<'_>) -> bool {
    node.tag == StanzaTag::Notification.as_str()
        && node
            .get_attr("type")
            .is_some_and(|value| value == NotificationType::Encrypt.as_str())
}

/// Whether the reconnect backoff counter should snap back to its 1s base after
/// a disconnect — WA Web's `resetDelay` (30s) semantics. `penalty_pending`
/// mirrors WA Web's `cancelReset()`: an explicit penalty applied this cycle
/// (429 rate-limit, or a manual `reconnect()` step) must survive, so a
/// long-lived-then-rate-limited connection keeps its deliberate backoff instead
/// of snapping to 1s.
pub(crate) fn should_reset_backoff(
    connected_at: Option<wacore::time::Instant>,
    now: wacore::time::Instant,
    penalty_pending: bool,
) -> bool {
    !penalty_pending
        && connected_at.is_some_and(|connected_at| {
            now.saturating_duration_since(connected_at) >= Client::STABLE_CONNECTION_RESET
        })
}

/// Ceiling on the consecutive-failure count that drives the reconnect backoff.
///
/// The delay is already pinned at the 900 s cap from attempt 17 on, so every
/// value past that names the same wait — what would keep growing is only the
/// number itself, which `stats().reconnect_errors` reports and which a 429 adds
/// five to at a time. A link that flaps for weeks would run it up without
/// bound, so it saturates here instead: far enough beyond the cap that the
/// schedule is untouched, close enough that the counter stays a number a
/// consumer can read.
pub(crate) const MAX_BACKOFF_ATTEMPTS: u32 = 64;

/// The consecutive-failure count that follows `previous`, saturating at
/// [`MAX_BACKOFF_ATTEMPTS`].
pub(crate) fn next_backoff_attempt(previous: u32) -> u32 {
    previous.saturating_add(1).min(MAX_BACKOFF_ATTEMPTS)
}

/// Computes a reconnect delay matching WhatsApp Web's Fibonacci backoff:
/// `{ algo: { type: "fibonacci", first: 1000, second: 1000 }, jitter: 0.1, max: 9e5 }`
///
/// Sequence: 1s, 1s, 2s, 3s, 5s, 8s, 13s, 21s, 34s, 55s, 89s, 144s, ... capped at 900s.
/// Each value gets ±10% random jitter.
fn fibonacci_backoff(attempt: u32) -> Duration {
    const MAX_MS: u64 = 900_000; // WA Web: 9e5

    let mut a: u64 = 1000;
    let mut b: u64 = 1000;
    for _ in 0..attempt {
        // Every step past the cap yields the cap again, so the loop stops at
        // the first one instead of counting out an attempt number nothing else
        // bounds. Same clamp `prekeys.rs` applies to its own retry exponent,
        // and the same reason.
        if a >= MAX_MS {
            break;
        }
        let next = a.saturating_add(b).min(MAX_MS);
        a = b;
        b = next;
    }
    let base = a.min(MAX_MS);

    // ±10% jitter (WA Web: jitter: 0.1)
    let jitter_range = base / 10;
    let jitter = if jitter_range > 0 {
        // The thread-local generator, not a fresh `StdRng`: seeding one runs a
        // full ChaCha key schedule off OS entropy to draw a single number
        // (measured at 14.8 us against 808 ns for the draw, which is why
        // `keepalive_loop` hoists its own out of the loop).
        rand::rng().random_range(0..=(jitter_range * 2)) as i64 - jitter_range as i64
    } else {
        0
    };
    let ms = (base as i64 + jitter).max(0) as u64;
    Duration::from_millis(ms)
}

/// Release the table a reservation set grew during a burst.
///
/// `pending_retries` and `pending_lid_refreshes` hold one entry per in-flight
/// operation and are empty almost all the time, but a reconnect can push
/// hundreds of retries through at once and a `HashSet` never gives that table
/// back on its own. The `len * 4` threshold keeps a set that is still draining
/// from oscillating between shrink and regrow; `shrink_to` rather than
/// `shrink_to_fit` leaves room for the tail of the burst.
pub(crate) fn release_after_burst<T: Eq + std::hash::Hash>(set: &mut HashSet<T>) {
    if set.capacity() > 32 && set.len() * 4 < set.capacity() {
        set.shrink_to(set.len() * 2);
    }
}

#[cfg(test)]
mod tests;
