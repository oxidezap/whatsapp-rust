mod accessors;
mod adapters;
mod app_state;
mod context_impl;
mod device_registry;
pub(crate) mod device_topology;
mod iq_ops;
mod lid_pn;
mod lifecycle;
mod messaging;
mod node_io;
pub(crate) mod offline_resume;
mod sender_keys;
mod sessions;
mod voip;
pub use voip::{CallError, Voip};

use crate::cache::Cache;
use crate::cache_store::TypedCache;
use crate::handshake;
use crate::lid_pn_cache::LidPnCache;
use crate::pair;
use anyhow::{Result, anyhow};
use futures::FutureExt;
#[cfg(test)]
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

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
                    .is_some_and(|attr| attr.as_str() == v.as_str())
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
    pub enqueue_lock: Arc<async_lock::Mutex<()>>,
    pub queue_tx: async_channel::Sender<Arc<wacore_binary::OwnedNodeRef>>,
}

const APP_STATE_RETRY_MAX_ATTEMPTS: u32 = 6;

/// WA Web: MQTT `MqttProtocolClient.connect()` uses `CONNECT_TIMEOUT = 20s`,
/// DGW `connectTimeoutMs` defaults to `20000ms`.
const TRANSPORT_CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

pub use wacore::stats::{CollectionStats, StatsSnapshot};

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
    pub recent_messages: CollectionStats,
    pub sender_key_device_cache: CollectionStats,
    pub group_devices_memo: CollectionStats,
    pub message_retry_counts: u64,
    pub undecryptable_dispatched: u64,
    pub pdo_pending_requests: u64,
    pub pdo_requested: u64,
    // -- Capacity-only caches (coordination, counts only) --
    pub session_locks: u64,
    pub chat_lanes: u64,
    pub resend_rate_limiter_chats: u64,
    // -- Unbounded collections --
    pub response_waiters: usize,
    pub node_waiters: usize,
    pub pending_retries: usize,
    pub presence_subscriptions: usize,
    pub app_state_key_requests: usize,
    pub app_state_syncing: usize,
    pub signal_sessions: CollectionStats,
    pub signal_identities: CollectionStats,
    pub signal_sender_keys: CollectionStats,
    // -- Misc --
    pub chatstate_handlers: usize,
    pub custom_enc_handlers: usize,
}

impl MemoryReport {
    /// Every byte-carrying collection with its display name — the single list
    /// [`Self::total_estimated_bytes`] and `Display` derive from, so a new
    /// collection cannot be summed but not shown (or vice versa).
    fn collections(&self) -> [(&'static str, &CollectionStats); 10] {
        [
            ("group_cache:", &self.group_cache),
            ("device_registry_cache:", &self.device_registry_cache),
            ("lid_pn (lid):", &self.lid_pn_lid_entries),
            ("lid_pn (pn):", &self.lid_pn_pn_entries),
            ("recent_messages:", &self.recent_messages),
            ("sk_device_cache:", &self.sender_key_device_cache),
            ("group_devices_memo:", &self.group_devices_memo),
            ("signal_sessions:", &self.signal_sessions),
            ("signal_identities:", &self.signal_identities),
            ("signal_sender_keys:", &self.signal_sender_keys),
        ]
    }

    /// Sum of every estimated byte figure in the report.
    pub fn total_estimated_bytes(&self) -> u64 {
        self.collections().iter().map(|(_, c)| c.bytes).sum()
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
        // caches; the rest are the Signal store caches.
        const TTL_BOUNDED: usize = 7;
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
        writeln!(f, "  pdo_pending_requests:   {}", self.pdo_pending_requests)?;
        writeln!(f, "  pdo_requested:          {}", self.pdo_requested)?;
        writeln!(f, "--- Capacity-only caches ---")?;
        writeln!(f, "  session_locks:          {}", self.session_locks)?;
        writeln!(f, "  chat_lanes:             {}", self.chat_lanes)?;
        writeln!(
            f,
            "  resend_rl_chats:        {}",
            self.resend_rate_limiter_chats
        )?;
        writeln!(f, "--- Unbounded collections ---")?;
        writeln!(f, "  response_waiters:       {}", self.response_waiters)?;
        writeln!(f, "  node_waiters:           {}", self.node_waiters)?;
        writeln!(f, "  pending_retries:        {}", self.pending_retries)?;
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
        writeln!(f, "  app_state_syncing:      {}", self.app_state_syncing)?;
        writeln!(f, "--- Signal store caches ---")?;
        for (name, c) in &collections[TTL_BOUNDED..] {
            line(f, name, c)?;
        }
        writeln!(f, "--- Misc ---")?;
        writeln!(f, "  chatstate_handlers:     {}", self.chatstate_handlers)?;
        writeln!(f, "  custom_enc_handlers:    {}", self.custom_enc_handlers)?;
        writeln!(
            f,
            "  total estimated:        {} B",
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
    #[error("client is already connected")]
    AlreadyConnected,
    #[error("client is not logged in")]
    NotLoggedIn,
    #[error("IQ request failed: {0}")]
    Iq(#[from] crate::request::IqError),
    /// Last-resort catch-all for internal failures threaded through `?` that do
    /// not (yet) have a dedicated variant. Transparent so the underlying
    /// error's `Display`/source chain is preserved.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl ClientError {
    pub fn is_transport_unavailable(&self) -> bool {
        match self {
            ClientError::NotConnected => true,
            ClientError::EncryptSend(e) => e.is_transport_unavailable(),
            // Transport loss can now arrive wrapped in an IQ failure (the base
            // error gained `Iq`); unwrap it so retry/reconnect still triggers.
            ClientError::Iq(e) => match e {
                crate::request::IqError::NotConnected => true,
                crate::request::IqError::EncryptSend(e) => e.is_transport_unavailable(),
                crate::request::IqError::ClientState(client) => client.is_transport_unavailable(),
                _ => false,
            },
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

pub struct Client {
    pub(crate) runtime: Arc<dyn Runtime>,
    pub(crate) core: wacore::client::CoreClient,

    pub(crate) persistence_manager: Arc<PersistenceManager>,
    /// Write-behind buffer for inbound messageSecret captures; readers check
    /// it before the backend so the durable write can leave the receive lane.
    pub(crate) msg_secret_buffer: Arc<crate::msg_secret_buffer::MsgSecretWriteBuffer>,
    pub(crate) media_conn: Arc<RwLock<Option<crate::mediaconn::MediaConn>>>,

    pub(crate) is_logged_in: Arc<AtomicBool>,
    pub(crate) is_connecting: Arc<AtomicBool>,
    pub(crate) is_running: Arc<AtomicBool>,
    /// Whether the noise socket is established (connected to WhatsApp servers).
    /// Uses an AtomicBool instead of probing the noise_socket mutex to avoid
    /// TOCTOU races where `try_lock()` fails due to contention, not disconnection.
    is_connected: Arc<AtomicBool>,

    /// whatsmeow's `sendActiveReceipts`: 0 = inactive (default), 1 = active
    /// (presence available), 2 = forced. When 0, delivery receipts use `type="inactive"`.
    send_active_receipts: AtomicU32,

    /// Per-process counter of consecutive Noise IK handshake failures, scoped
    /// to the lifetime of this `Client`. Mirrors `K` in WA Web's
    /// `WAWebOpenChatSocket` (`ChatSocket.js`): on the first failure within a
    /// process, the next connect skips IK and falls back to XX so a stale
    /// cached `serverStaticPublic` doesn't trap us in a loop. Reset to 0 on
    /// any successful handshake (XX, IK, or XXfallback).
    pub(crate) ik_handshake_failures: Arc<AtomicU32>,
    /// Terminal shutdown (process-wide). Fired ONLY by `disconnect()`.
    /// Long-lived subscribers that must outlive reconnect cycles (saver,
    /// device registry cleanup) subscribe here.
    pub(crate) shutdown_notifier: wacore::runtime::ShutdownNotifier,

    /// Per-connection shutdown. Replaced with a fresh notifier on every new
    /// connection; fired on cleanup_connection_state / stream end / stream
    /// error / connect_failure / disconnect. Per-connection subscribers
    /// (keepalive, request waiters, read loop, offline flush) observe this.
    pub(crate) connection_shutdown: std::sync::Mutex<wacore::runtime::ShutdownNotifier>,
    /// Per-session wire I/O and activity counters. Written at the transport
    /// chokepoints (noise sender task, read loop); the keepalive dead-socket
    /// watchdog reads its activity timestamps. Snapshot via [`Client::stats`].
    pub(crate) stats: Arc<wacore::stats::SessionStats>,

    pub(crate) transport: Arc<Mutex<Option<Arc<dyn crate::transport::Transport>>>>,
    pub(crate) transport_events:
        Arc<Mutex<Option<async_channel::Receiver<crate::transport::TransportEvent>>>>,
    pub(crate) transport_factory: Arc<dyn crate::transport::TransportFactory>,
    pub(crate) noise_socket: Arc<Mutex<Option<Arc<NoiseSocket>>>>,

    pub(crate) response_waiters: Arc<
        Mutex<HashMap<String, futures::channel::oneshot::Sender<Arc<wacore_binary::OwnedNodeRef>>>>,
    >,

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
    /// Matches WhatsApp Web's SignalStoreCache pattern: crypto ops read/write this cache,
    /// and DB writes are deferred to flush() after each message is processed.
    pub(crate) signal_cache: Arc<crate::store::signal_cache::SignalStoreCache>,

    /// Limits message processing concurrency (1 permit during offline sync, N after).
    /// Wrapped in Mutex to allow replacing on reconnect.
    pub(crate) message_processing_semaphore: std::sync::Mutex<Arc<async_lock::Semaphore>>,
    /// Bumped on every semaphore swap so stale Arc clones are rejected.
    pub(crate) message_semaphore_generation: Arc<AtomicU64>,

    /// Per-device session locks for Signal protocol operations.
    /// Prevents race conditions when multiple messages from the same sender
    /// are processed concurrently across different chats.
    /// Keys are Signal protocol address strings (e.g., "user@s.whatsapp.net:0")
    /// to match the SignalProtocolStoreAdapter's internal locking.
    pub(crate) session_locks: Cache<String, Arc<async_lock::Mutex<()>>>,

    /// Per-chat lane combining enqueue lock + message queue into a single cached entry.
    /// One cache lookup instead of two per incoming message.
    pub(crate) chat_lanes: Cache<Jid, ChatLane>,

    /// Cache for LID to Phone Number mappings (bidirectional).
    /// When we receive a message with sender_lid/sender_pn attributes, we store the mapping here.
    /// This allows us to reuse existing LID-based sessions when sending replies.
    /// The cache is backed by persistent storage and warmed up on client initialization.
    pub(crate) lid_pn_cache: Arc<LidPnCache>,
    pub(crate) ab_props: Arc<wacore::store::ab_props::AbPropsCache>,

    pub group_cache: async_lock::Mutex<Option<Arc<GroupCache>>>,

    pub(crate) expected_disconnect: Arc<AtomicBool>,
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
    pub(crate) session_recreate_history: Cache<wacore_binary::jid::Jid, wacore::time::Instant>,

    /// Per-chat outbound resend rate limiter: bounds the aggregate resend rate
    /// to a chat (the anti-abuse signal) so a PN to LID fan-out cannot storm into
    /// AccountLocked. Throttled devices still recover via the fresh-SKDM mark.
    pub(crate) resend_rate_limiter: crate::resend_rate_limiter::ResendRateLimiter,

    /// Dispatch-once gate for `UndecryptableMessage`: a server resend of a
    /// failed id re-enters the failure path and would otherwise fire a
    /// duplicate event. Mirrors WA Web's DB-level placeholder uniqueness
    /// in `WAWebMessageProcessPlaceholder`.
    pub(crate) undecryptable_dispatched: Cache<ChatMessageId, ()>,

    pub enable_auto_reconnect: Arc<AtomicBool>,
    /// Consecutive reconnect failures, drives the Fibonacci backoff. Exposed
    /// read-only via [`StatsSnapshot::reconnect_errors`](wacore::stats::StatsSnapshot).
    pub(crate) auto_reconnect_errors: Arc<AtomicU32>,

    pub(crate) needs_initial_full_sync: Arc<AtomicBool>,

    pub(crate) app_state_processor: async_lock::Mutex<Option<Arc<AppStateProcessor>>>,
    pub(crate) app_state_key_requests: Arc<Mutex<HashMap<String, wacore::time::Instant>>>,
    /// Tracks collections currently being synced to prevent duplicate sync tasks.
    /// Matches WA Web's in-flight tracking set in WAWebSyncdCollectionsStateMachine.
    pub(crate) app_state_syncing: Arc<Mutex<HashSet<WAPatchName>>>,
    pub(crate) initial_keys_synced_notifier: Arc<event_listener::Event>,
    pub(crate) initial_app_state_keys_received: Arc<AtomicBool>,

    /// Prevents concurrent prekey upload operations (matches WA Web's dedup set in `handlePreKeyLow`).
    pub(crate) prekey_upload_lock: Arc<async_lock::Mutex<()>>,
    /// Notifier for when offline sync (ib offline stanza) is received.
    /// WhatsApp Web waits for this before sending passive tasks (prekey upload, active IQ, presence).
    pub(crate) offline_sync_notifier: Arc<event_listener::Event>,
    /// Flag indicating offline sync has completed (received ib offline stanza).
    pub(crate) offline_sync_completed: Arc<AtomicBool>,
    /// Delivery receipts buffered during offline sync, flushed as aggregate
    /// `<receipt>` stanzas at completion (WA Web `sendAggregateOfflineReceipts`).
    /// Empty (zero capacity) outside the offline window.
    pub(crate) offline_receipt_buffer:
        std::sync::Mutex<Vec<Arc<crate::types::message::MessageInfo>>>,
    /// Number of history sync tasks currently queued or running.
    pub(crate) history_sync_tasks_in_flight: Arc<AtomicUsize>,
    /// Notifier triggered when history sync work becomes idle.
    pub(crate) history_sync_idle_notifier: Arc<event_listener::Event>,
    /// Flushed by `disconnect()`/`reconnect()` before tearing down the transport
    /// so in-flight delivery receipts aren't dropped with `NotConnected`
    /// (issue #571).
    pub(crate) outbound_flush: Arc<crate::flush_scope::FlushScope>,
    /// Contacts with active presence subscriptions that must be re-subscribed on reconnect.
    pub(crate) presence_subscriptions: Arc<async_lock::Mutex<HashSet<Jid>>>,
    /// Metrics for granular offline sync logging
    pub(crate) offline_sync_metrics: Arc<OfflineSyncMetrics>,
    /// Drives the WA Web pull-batch loop for offline backlog delivery.
    pub(crate) offline_batch: Arc<crate::client::offline_resume::OfflineBatchCoordinator>,
    /// Notifier for when the noise socket is established (before login).
    /// Use this to wait for the socket to be ready for sending messages.
    pub(crate) socket_ready_notifier: Arc<event_listener::Event>,
    /// Set to `true` only when `dispatch_connected()` fires (after critical sync
    /// completes). Reset on each new connection attempt. Used by
    /// `wait_for_connected()` to avoid a false-positive fast path when the
    /// client is logged in but critical app state hasn't synced yet.
    pub(crate) is_ready: Arc<AtomicBool>,
    /// Notifier for when the client is fully connected and logged in.
    /// Triggered after Event::Connected is dispatched.
    pub(crate) connected_notifier: Arc<event_listener::Event>,
    pub(crate) major_sync_task_sender: async_channel::Sender<MajorSyncTask>,
    pub(crate) pairing_cancellation_tx: Arc<Mutex<Option<async_channel::Sender<()>>>>,

    /// State machine for pair code authentication flow.
    /// Tracks the pending pair code request and ephemeral keys.
    pub(crate) pair_code_state: Arc<Mutex<wacore::pair_code::PairCodeState>>,

    /// SHORTCAKE_PASSKEY linking flow state: the pending handoff key, the
    /// per-attempt ephemeral linking cache, and the optional host authenticator.
    pub(crate) passkey_state: Arc<Mutex<crate::passkey::flow::PasskeyFlowState>>,

    /// Wait-free "an open is in flight" reservation for the passkey flow. Kept
    /// outside `passkey_state` so it can be released synchronously on drop (a
    /// cancelled open can't leave it stuck), unlike a flag behind the async lock.
    pub(crate) passkey_opening: AtomicBool,

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

    /// Chat state (typing indicator) handlers registered by external consumers.
    /// Each handler receives a `ChatStateEvent` describing the chat, optional participant and state.
    pub(crate) chatstate_handlers: Arc<RwLock<Vec<ChatStateHandler>>>,

    pub(crate) pdo_pending_requests:
        Cache<wacore::types::message::ChatMessageId, crate::pdo::PendingPdoRequest>,

    /// Messages already covered by a placeholder-resend PDO request. Mirrors
    /// the session-lifetime set in
    /// `WAWebNonMessageDataRequestPlaceholderMessageResendUtils`: at most one
    /// request per message, no matter how many times the server redelivers
    /// the undecryptable original. Entries are dropped on send failure so a
    /// transient error does not block the next attempt.
    pub(crate) pdo_requested: Cache<wacore::types::message::ChatMessageId, ()>,

    /// LRU cache for device registry (matches WhatsApp Web's 5000 entry limit).
    /// Maps user ID to DeviceListRecord for fast device existence checks.
    /// Backed by persistent storage.
    /// Device registry fused with its topology tracker: every write records
    /// the change by construction, so the group-devices memo below can never
    /// be left stale by a forgotten bump.
    pub(crate) device_registry_cache: crate::client::device_topology::DeviceRegistryCache,
    /// Shared topology tracker (generation + changed-users log). LidPnCache
    /// records mapping changes into it; the memo validates against it.
    pub(crate) device_topology: Arc<crate::client::device_topology::DeviceTopology>,
    /// Whether the group-devices memo may be used: false when the registry
    /// or LID-PN caches are store-backed (a shared external store can be
    /// written by other processes, which the in-process topology tracker
    /// cannot observe).
    pub(crate) group_devices_memo_enabled: bool,
    /// Per-group memo of the fully resolved (LID-converted) device list,
    /// validated by GroupInfo identity + the device topology. Serves the
    /// per-send full-set resolution in `resolve_skdm_targets` so a warm
    /// repeat send skips the per-member cache fan-out.
    pub(crate) group_devices_memo:
        Cache<Jid, Arc<crate::client::device_registry::GroupDevicesMemo>>,

    /// Single-flight for cold SKDM distribution, keyed per group. Concurrent
    /// cold sends each re-ran the full per-member fan-out before any of them
    /// marked the devices warm; the loser now waits here and re-resolves,
    /// finding everything warm. Warm sends never touch it.
    pub(crate) group_distribution_locks: Cache<Jid, Arc<async_lock::Mutex<()>>>,

    /// Last `(devices, sender-key-device map)` Arc pair with an empty `needs_skdm`,
    /// so a warm repeat send skips `filter_skdm_targets`. `Weak` keeps the pointer
    /// comparison ABA-safe, matching `GroupDevicesMemo`.
    pub(crate) skdm_warm_memo: Cache<
        Jid,
        (
            std::sync::Weak<wacore::send::ResolvedGroupDevices>,
            std::sync::Weak<crate::sender_key_device_cache::SenderKeyDeviceMap>,
        ),
    >,

    /// Router for dispatching stanzas to their appropriate handlers
    pub(crate) stanza_router: crate::handlers::router::StanzaRouter,

    /// Whether to send ACKs synchronously or in a background task
    pub(crate) synchronous_ack: bool,

    /// HTTP client for making HTTP requests (media upload/download, version fetching)
    pub http_client: Arc<dyn crate::http::HttpClient>,

    /// Version override for testing or manual specification
    pub(crate) override_version: Option<(u32, u32, u32)>,

    /// When true, history sync notifications are acknowledged but not downloaded
    /// or processed. Set via `BotBuilder::skip_history_sync()`.
    pub(crate) skip_history_sync: AtomicBool,

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

    /// Holds the background saver's AbortHandle so the task lifetime follows
    /// `Arc<Client>` ref count instead of the Bot wrapper's. Set once by
    /// `Bot::build`; on Client drop (last Arc), the handle drops and the saver
    /// is aborted.
    pub(crate) saver_handle: std::sync::OnceLock<wacore::runtime::AbortHandle>,

    /// When true, emit `Event::RawNode` for every decoded stanza before router dispatch.
    /// Default false — only enable when external consumers need raw protocol access.
    raw_node_forwarding: AtomicBool,

    /// Active VoIP calls and their media-task abort handles. `abort_all` runs from the
    /// connection-cleanup path so a disconnect/reconnect tears down every in-flight call. Behind the
    /// `voip` feature: it is populated only by the `voip` media facade.
    #[cfg(feature = "voip")]
    pub(crate) call_registry: Arc<wacore::voip::CallRegistry>,

    /// Outgoing calls awaiting their relay. The initiator's relay is not in the offer; it arrives
    /// from the server AFTER the offer (live-only), so each `voip().call()` parks the material needed
    /// to spawn the engine here, keyed by call-id, until a `<call>` carrying a `<relay>` for that id
    /// arrives. Behind the `voip` feature; populated only by the media facade.
    #[cfg(feature = "voip")]
    pub(crate) pending_outgoing_calls: Arc<
        std::sync::Mutex<std::collections::HashMap<String, crate::voip::facade::PendingOutgoing>>,
    >,
}

/// Builds a pong response node for a server-initiated ping.
///
/// Matches WhatsApp Web (`WAWebCommsHandleStanza`): only includes `id`
/// when the server ping carried one.
fn build_pong(to: String, id: Option<&str>) -> wacore_binary::Node {
    let mut builder = NodeBuilder::new("iq").attr("to", to).attr("type", "result");
    if let Some(id) = id {
        builder = builder.attr("id", id);
    }
    builder.build()
}

/// Build an `<ack/>` for the given stanza, matching WA Web / whatsmeow behavior:
///
/// - `class` = original stanza tag
/// - `id`, `to` (flipped from `from`), `participant` copied from original
/// - `from` = own device PN, only for message acks
/// - `type` echoed for non-message stanzas (whatsmeow: `node.Tag != "message"`),
///   except `notification type="encrypt"` with `<identity/>` child (WA Web drops type there).
///
/// For receipt acks, WA Web uses `MAYBE_CUSTOM_STRING(ackString)` where
/// `ackString = maybeAttrString("type")` — so `type` is only included when
/// explicitly present on the incoming receipt (delivery receipts normally
/// have no type attribute, meaning the ack also has no type).
/// Encode an ack stanza directly to bytes, bypassing Node + marshal_auto.
/// Acks are the most frequent outbound stanza (~1 per inbound message).
fn encode_ack_bytes(
    node: &wacore_binary::NodeRef<'_>,
    own_device_pn: Option<&Jid>,
) -> Result<Option<Vec<u8>>, wacore_binary::error::BinaryError> {
    use wacore_binary::encoder::{ByteWriter, EncodeNode, Encoder};

    let Some(id_val) = node.get_attr("id") else {
        return Ok(None);
    };
    let Some(from_val) = node.get_attr("from") else {
        return Ok(None);
    };
    // WAWebReceiptAck: `participant: r && r !== e ? DEVICE_JID(r) : DROP_ATTR`.
    // Drop the attribute when it would duplicate `to` (which is the flipped `from`).
    let participant_val = node.get_attr("participant").filter(|p| {
        let p_str = p.as_str();
        let from_str = from_val.as_str();
        p_str.as_ref() != from_str.as_ref()
    });
    // Server expects `recipient` echoed back so it can route the ack to the
    // origin companion/device (hosted-companion, peer, LID-routed stanzas).
    // Dropping it makes the server close the stream with `<stream:error><ack/>`.
    let recipient_val = node.get_attr("recipient");
    let tag = node.tag.as_ref();

    let typ_val = if tag != "message" && !is_encrypt_identity_notification(node) {
        node.get_attr("type")
    } else {
        None
    };

    let include_from = tag == "message" && own_device_pn.is_some();

    // Count attrs: class + id + to + optional(from, participant, recipient, type)
    let attr_count = 3
        + usize::from(include_from)
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
        own_pn: if include_from { own_device_pn } else { None },
        tag_str: tag,
        attr_count,
    };

    let mut buf = Vec::with_capacity(64);
    let mut encoder = Encoder::new_vec(&mut buf)?;
    encoder.write_node(&ack)?;
    Ok(Some(buf))
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

/// Build an ack Node (used in tests for structure verification).
#[cfg(test)]
fn build_ack_node(node: &wacore_binary::NodeRef<'_>, own_device_pn: Option<&Jid>) -> Option<Node> {
    let id = node.get_attr("id")?.to_node_value();
    let from_ref = node.get_attr("from")?;
    let from = from_ref.to_node_value();
    // Drop participant when it duplicates `to` (the flipped `from`).
    let participant = node
        .get_attr("participant")
        .filter(|p| p.as_str().as_ref() != from_ref.as_str().as_ref())
        .map(|v| v.to_node_value());
    let recipient = node.get_attr("recipient").map(|v| v.to_node_value());
    let tag = node.tag.as_ref();
    let typ = if tag != "message" && !is_encrypt_identity_notification(node) {
        node.get_attr("type").map(|v| v.to_node_value())
    } else {
        None
    };
    let mut attrs = Attrs::with_capacity(7);
    attrs.insert("class", NodeValue::from(tag));
    attrs.insert("id", id);
    attrs.insert("to", from);
    if tag == "message"
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

/// WA Web omits `type` when ACKing `<notification type="encrypt"><identity/></notification>`.
fn is_encrypt_identity_notification(node: &wacore_binary::NodeRef<'_>) -> bool {
    node.tag == "notification"
        && node
            .get_attr("type")
            .is_some_and(|v| v.as_str() == "encrypt")
        && node.get_optional_child("identity").is_some()
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
        let next = a.saturating_add(b).min(MAX_MS);
        a = b;
        b = next;
    }
    let base = a.min(MAX_MS);

    // ±10% jitter (WA Web: jitter: 0.1)
    let jitter_range = base / 10;
    let jitter = if jitter_range > 0 {
        rand::make_rng::<rand::rngs::StdRng>().random_range(0..=(jitter_range * 2)) as i64
            - jitter_range as i64
    } else {
        0
    };
    let ms = (base as i64 + jitter).max(0) as u64;
    Duration::from_millis(ms)
}

#[cfg(test)]
mod tests;
