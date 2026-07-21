use crate::stanza::BusinessSubscription;
use crate::types::call::{CallEndedElsewhere, IncomingCall, MissedCall};
use crate::types::message::MessageInfo;
use crate::types::presence::{ChatPresence, ChatPresenceMedia, ReceiptType};
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use portable_atomic::{AtomicU64, Ordering};
use serde::Serialize;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};
use wacore_binary::Node;
use wacore_binary::OwnedNodeRef;
use wacore_binary::{Jid, MessageId};
use waproto::whatsapp as wa;

/// A lazily-parsed history sync blob.
///
/// Carries the original **compressed** payload (one immutable `Bytes`,
/// typically ~10x smaller than the inflated form), so holding or queueing the
/// event costs O(compressed) memory. Cheap metadata (`sync_type`,
/// `chunk_order`, `progress`) is available without touching the payload, and
/// with `Arc<Event>` dispatch all handlers share the same instance.
///
/// Three ways at the payload, by increasing cost:
/// - [`stream()`](Self::stream) — conversations one at a time with bounded
///   memory (peak ≈ the largest single conversation), plus a decoded
///   remainder for everything else.
/// - [`decompress()`](Self::decompress) — the raw decompressed protobuf
///   bytes, inflated per call, for custom partial decoding.
/// - [`get()`](Self::get) — full decode, cached; later calls are free.
///
/// A multi-MB chunk takes tens of milliseconds to inflate (plus decode for
/// `get()`). Inside an async handler, prefer doing that work in
/// `spawn_blocking` — clone [`compressed_bytes()`](Self::compressed_bytes)
/// into the closure — when [`decompressed_size()`](Self::decompressed_size)
/// is large.
pub struct LazyHistorySync {
    /// Original zlib-compressed payload. Immutable for the event's lifetime:
    /// clones are refcount bumps, and every accessor keeps working after
    /// [`get()`](Self::get) (no take-dance).
    compressed: Bytes,
    /// Exact inflated size, counted by the producer's extraction pass; doubles
    /// as the inflate cap (a tighter anti-bomb bound than the global ceiling).
    decompressed_size: usize,
    sync_type: i32,
    chunk_order: Option<u32>,
    progress: Option<u32>,
    /// Set on ON_DEMAND syncs so consumers can correlate the answer with their
    /// outstanding `fetchMessageHistory` / `requestPlaceholderResend` request.
    peer_data_request_session_id: Option<String>,
    parsed: OnceLock<Option<Box<wa::HistorySync>>>,
}

impl Clone for LazyHistorySync {
    fn clone(&self) -> Self {
        // The decode cache is intentionally not carried over: it would deep-copy
        // a multi-MB proto. A clone re-inflates on demand from the shared
        // compressed bytes.
        Self {
            compressed: self.compressed.clone(),
            decompressed_size: self.decompressed_size,
            sync_type: self.sync_type,
            chunk_order: self.chunk_order,
            progress: self.progress,
            peer_data_request_session_id: self.peer_data_request_session_id.clone(),
            parsed: OnceLock::new(),
        }
    }
}

impl LazyHistorySync {
    pub fn new(
        compressed: Bytes,
        decompressed_size: usize,
        sync_type: i32,
        chunk_order: Option<u32>,
        progress: Option<u32>,
    ) -> Self {
        Self {
            compressed,
            decompressed_size,
            sync_type,
            chunk_order,
            progress,
            peer_data_request_session_id: None,
            parsed: OnceLock::new(),
        }
    }

    pub fn with_peer_data_request_session_id(mut self, id: Option<String>) -> Self {
        self.peer_data_request_session_id = id;
        self
    }

    /// History sync type (e.g. InitialBootstrap, Recent, PushName).
    /// Available without decoding the proto.
    pub fn sync_type(&self) -> i32 {
        self.sync_type
    }

    /// Chunk ordering for multi-chunk transfers.
    pub fn chunk_order(&self) -> Option<u32> {
        self.chunk_order
    }

    /// Sync progress (0-100).
    pub fn progress(&self) -> Option<u32> {
        self.progress
    }

    /// `None` for server-pushed syncs (e.g. `InitialBootstrap`).
    pub fn peer_data_request_session_id(&self) -> Option<&str> {
        self.peer_data_request_session_id.as_deref()
    }

    /// The original zlib-compressed payload. Zero-cost access; `Bytes` clones
    /// share the buffer (hand one to `spawn_blocking` for off-runtime
    /// consumption).
    pub fn compressed_bytes(&self) -> &Bytes {
        &self.compressed
    }

    /// Exact size of the decompressed blob in bytes, known without inflating.
    pub fn decompressed_size(&self) -> usize {
        self.decompressed_size
    }

    /// Inflate the payload, returning the raw decompressed protobuf bytes for
    /// custom partial decoding. Inflates on EVERY call (no caching) — hold on
    /// to the result if it is needed more than once. The exact
    /// [`decompressed_size`](Self::decompressed_size) caps the inflate, so a
    /// tampered blob fails instead of over-allocating.
    pub fn decompress(&self) -> std::io::Result<Bytes> {
        wacore_binary::zlib_pool::decompress_zlib_pooled(
            &self.compressed,
            self.decompressed_size as u64,
        )
        .map(Bytes::from)
    }

    /// Incremental reader over the payload: conversations one at a time, then
    /// everything else as a decoded remainder, without materializing the whole
    /// decompressed blob. See [`HistorySyncStream`].
    ///
    /// [`HistorySyncStream`]: crate::history_sync::HistorySyncStream
    pub fn stream(&self) -> crate::history_sync::HistorySyncStream<'_> {
        crate::history_sync::HistorySyncStream::new(&self.compressed, self.decompressed_size as u64)
    }

    /// Full decode of the history sync proto, cached via OnceLock: the first
    /// call inflates + decodes, later calls are free. Returns `None` if
    /// inflating or decoding fails. The compressed payload is kept, so
    /// [`decompress()`](Self::decompress) and [`stream()`](Self::stream) keep
    /// working afterwards.
    pub fn get(&self) -> Option<&wa::HistorySync> {
        self.parsed
            .get_or_init(|| {
                let raw = self.decompress().ok()?;
                waproto::codec::history_sync_decode(&raw[..])
                    .ok()
                    .map(Box::new)
            })
            .as_deref()
    }
}

impl fmt::Debug for LazyHistorySync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LazyHistorySync")
            .field("sync_type", &self.sync_type)
            .field("chunk_order", &self.chunk_order)
            .field("progress", &self.progress)
            .field(
                "peer_data_request_session_id",
                &self.peer_data_request_session_id,
            )
            .field("compressed_size", &self.compressed.len())
            .field("decompressed_size", &self.decompressed_size)
            .field(
                "parsed",
                &self.parsed.get().and_then(|o| o.as_ref()).is_some(),
            )
            .finish()
    }
}

impl Serialize for LazyHistorySync {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("LazyHistorySync", 4)?;
        s.serialize_field("sync_type", &self.sync_type)?;
        s.serialize_field("chunk_order", &self.chunk_order)?;
        s.serialize_field("progress", &self.progress)?;
        s.serialize_field(
            "peer_data_request_session_id",
            &self.peer_data_request_session_id,
        )?;
        s.end()
    }
}

/// Discriminant for each [`Event`] variant, used to express handler interest
/// without materializing the event. One per `Event` variant, in declaration
/// order; the value doubles as a bit index in [`EventInterest`], so there can
/// be at most 128 kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum EventKind {
    Connected,
    Disconnected,
    PairSuccess,
    PairError,
    LoggedOut,
    PairingQrCode,
    PairingCode,
    PairingCodeRefresh,
    QrScannedWithoutMultidevice,
    ClientOutdated,
    Messages,
    Receipt,
    UndecryptableMessage,
    Notification,
    ChatPresence,
    Presence,
    PictureUpdate,
    UserAboutUpdate,
    ContactUpdated,
    ContactNumberChanged,
    ContactSyncRequested,
    GroupUpdate,
    ContactUpdate,
    IncomingCall,
    MissedCall,
    CallEndedElsewhere,
    PushNameUpdate,
    SelfPushNameUpdated,
    PinUpdate,
    MuteUpdate,
    ArchiveUpdate,
    StarUpdate,
    MarkChatAsReadUpdate,
    DeleteChatUpdate,
    ClearChatUpdate,
    UserStatusMuteUpdate,
    DeleteMessageForMeUpdate,
    LabelEditUpdate,
    LabelAssociationUpdate,
    HistorySync,
    OfflineSyncPreview,
    OfflineSyncCompleted,
    DeviceListUpdate,
    IdentityChange,
    BusinessStatusUpdate,
    StreamReplaced,
    TemporaryBan,
    ConnectFailure,
    StreamError,
    DisappearingModeChanged,
    NewsletterLiveUpdate,
    RawNode,
    MexNotification,
    PairPasskeyRequest,
    PairPasskeyConfirmation,
    PairPasskeyError,
    ServerAck,
    // When adding a variant, mind the 128-kind ceiling below (EventInterest packs
    // each discriminant as a bit in a u128) and keep the guard pointing at the
    // last variant.
}

impl EventKind {
    /// Bit-index ceiling: [`EventInterest`] packs each kind's discriminant into a
    /// `u128`, so there can be at most 128 kinds.
    pub const CAPACITY: u8 = 128;
}

// Build-time tripwire: a new variant that would overflow EventInterest's bitmask
// fails compilation instead of silently corrupting the mask at runtime.
const _: () = assert!((EventKind::ServerAck as u8) < EventKind::CAPACITY);

/// A set of [`EventKind`]s a handler wants delivered. Producers can query the
/// aggregate interest before building expensive payloads, and dispatch avoids
/// allocating an `Arc<Event>` when no handler wants the kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventInterest(u128);

impl EventInterest {
    /// Every kind. Default for handlers that don't narrow their interest.
    pub const ALL: EventInterest = EventInterest(u128::MAX);

    /// No kinds.
    pub const fn none() -> Self {
        EventInterest(0)
    }

    /// Interest in exactly the given kinds.
    pub fn of(kinds: &[EventKind]) -> Self {
        let mut bits = 0u128;
        let mut i = 0;
        while i < kinds.len() {
            bits |= 1u128 << (kinds[i] as u8);
            i += 1;
        }
        EventInterest(bits)
    }

    /// Add a kind to the set.
    pub const fn with(self, kind: EventKind) -> Self {
        EventInterest(self.0 | (1u128 << (kind as u8)))
    }

    /// Whether `kind` is in the set.
    #[inline]
    pub const fn wants(self, kind: EventKind) -> bool {
        self.0 & (1u128 << (kind as u8)) != 0
    }

    /// Set union, for aggregating the interests of several handlers behind one
    /// bus registration.
    pub const fn union(self, other: Self) -> Self {
        EventInterest(self.0 | other.0)
    }

    const fn words(self) -> (u64, u64) {
        (self.0 as u64, (self.0 >> 64) as u64)
    }
}

pub trait EventHandler: crate::sync_marker::MaybeSendSync {
    fn handle_event(&self, event: Arc<Event>);

    /// Registration-time interest hint used by
    /// [`CoreEventBus::subscribe_handler`]. The bus captures it once; use
    /// [`Subscription::update_interest`] for later changes.
    fn interest(&self) -> EventInterest {
        EventInterest::ALL
    }
}

/// Event handler that forwards events to an async channel.
///
/// # Example
/// ```ignore
/// let (handler, rx) = ChannelEventHandler::new();
/// let _subscription = client.subscribe_handler(handler);
/// while let Ok(event) = rx.recv().await {
///     if matches!(&*event, Event::Connected(_)) { break; }
/// }
/// ```
pub struct ChannelEventHandler {
    tx: async_channel::Sender<Arc<Event>>,
}

impl ChannelEventHandler {
    pub fn new() -> (Arc<Self>, async_channel::Receiver<Arc<Event>>) {
        let (tx, rx) = async_channel::unbounded();
        (Arc::new(Self { tx }), rx)
    }
}

impl EventHandler for ChannelEventHandler {
    fn handle_event(&self, event: Arc<Event>) {
        let _ = self.tx.try_send(event);
    }
}

#[derive(Clone)]
struct HandlerEntry {
    id: u64,
    interest: EventInterest,
    handler: Arc<dyn EventHandler>,
}

/// Immutable snapshot of the registered handlers. `dispatch` clones only the
/// outer `Arc` (one refcount bump, no `Vec` allocation), then drops the lock and
/// iterates it. Interests change only through the subscription that owns an
/// entry, so one snapshot always contains a coherent handler/filter pair.
#[derive(Default)]
struct HandlerSnapshot {
    handlers: Vec<HandlerEntry>,
}

struct CoreEventBusInner {
    handlers: RwLock<Arc<HandlerSnapshot>>,
    next_id: AtomicU64,
    interest_low: AtomicU64,
    interest_high: AtomicU64,
}

impl Default for CoreEventBusInner {
    fn default() -> Self {
        Self {
            handlers: RwLock::new(Arc::new(HandlerSnapshot::default())),
            next_id: AtomicU64::new(1),
            interest_low: AtomicU64::new(0),
            interest_high: AtomicU64::new(0),
        }
    }
}

impl CoreEventBusInner {
    fn snapshot(&self) -> Arc<HandlerSnapshot> {
        self.handlers
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn publish_interest(&self, interest: EventInterest) {
        let (low, high) = interest.words();
        if low != 0 {
            self.interest_low.fetch_or(low, Ordering::Release);
        }
        if high != 0 {
            self.interest_high.fetch_or(high, Ordering::Release);
        }
    }

    fn store_aggregate(&self, snapshot: &HandlerSnapshot) {
        let aggregate = snapshot
            .handlers
            .iter()
            .fold(EventInterest::none(), |all, entry| {
                all.union(entry.interest)
            });
        let (low, high) = aggregate.words();
        self.interest_low.store(low, Ordering::Release);
        self.interest_high.store(high, Ordering::Release);
    }

    fn remove(&self, id: u64) -> bool {
        let mut guard = self
            .handlers
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = &**guard;
        let Some(position) = current.handlers.iter().position(|entry| entry.id == id) else {
            return false;
        };
        let mut handlers = Vec::with_capacity(current.handlers.len() - 1);
        handlers.extend(current.handlers[..position].iter().cloned());
        handlers.extend(current.handlers[position + 1..].iter().cloned());
        let snapshot = Arc::new(HandlerSnapshot { handlers });
        // Retire the entry before clearing bits; an early read may only be a
        // harmless false positive.
        *guard = Arc::clone(&snapshot);
        self.store_aggregate(&snapshot);
        true
    }

    fn update_interest(&self, id: u64, interest: EventInterest) -> bool {
        let mut guard = self
            .handlers
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = &**guard;
        let Some(position) = current.handlers.iter().position(|entry| entry.id == id) else {
            return false;
        };
        if current.handlers[position].interest == interest {
            return true;
        }

        // Publish additions first so a completed snapshot update can never be
        // hidden by the lock-free producer filter.
        self.publish_interest(interest);
        let mut handlers = current.handlers.clone();
        handlers[position].interest = interest;
        let snapshot = Arc::new(HandlerSnapshot { handlers });
        *guard = Arc::clone(&snapshot);
        self.store_aggregate(&snapshot);
        true
    }

    fn has_handler_for(&self, kind: EventKind) -> bool {
        let bit = kind as u8;
        if bit < 64 {
            self.interest_low.load(Ordering::Acquire) & (1u64 << bit) != 0
        } else {
            self.interest_high.load(Ordering::Acquire) & (1u64 << (bit - 64)) != 0
        }
    }
}

/// Removal token for one event-handler registration.
///
/// Dropping it removes the handler. A dispatch that already cloned the old
/// snapshot may still complete once, while later dispatches cannot see it.
#[must_use = "dropping the subscription immediately unregisters the event handler"]
pub struct Subscription {
    bus: std::sync::Weak<CoreEventBusInner>,
    id: u64,
    active: bool,
}

impl Subscription {
    /// Replace this registration's filter without re-registering its handler.
    /// Returns `false` if the bus no longer exists or the entry was removed.
    pub fn update_interest(&self, interest: EventInterest) -> bool {
        self.active
            && self
                .bus
                .upgrade()
                .is_some_and(|bus| bus.update_interest(self.id, interest))
    }

    /// Remove the handler now instead of waiting for `Drop`.
    pub fn unsubscribe(mut self) -> bool {
        let removed = self.remove();
        self.active = false;
        removed
    }

    /// Keep this registration for the remaining lifetime of the event bus.
    pub fn detach(mut self) {
        self.active = false;
    }

    fn remove(&self) -> bool {
        self.bus.upgrade().is_some_and(|bus| bus.remove(self.id))
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if self.active {
            self.remove();
        }
    }
}

#[derive(Default, Clone)]
pub struct CoreEventBus {
    inner: Arc<CoreEventBusInner>,
}

impl CoreEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> Arc<HandlerSnapshot> {
        self.inner.snapshot()
    }

    /// Register `handler` with an explicit, stable filter.
    pub fn subscribe(
        &self,
        interest: EventInterest,
        handler: Arc<dyn EventHandler>,
    ) -> Subscription {
        let mut guard = self
            .inner
            .handlers
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let current = &**guard;
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let mut handlers = Vec::with_capacity(current.handlers.len() + 1);
        handlers.extend(current.handlers.iter().cloned());
        handlers.push(HandlerEntry {
            id,
            interest,
            handler,
        });
        // An early bit only takes the slow path against the previous snapshot.
        self.inner.publish_interest(interest);
        *guard = Arc::new(HandlerSnapshot { handlers });
        Subscription {
            bus: Arc::downgrade(&self.inner),
            id,
            active: true,
        }
    }

    /// Register using the handler's current [`EventHandler::interest`] hint.
    pub fn subscribe_handler(&self, handler: Arc<dyn EventHandler>) -> Subscription {
        let interest = handler.interest();
        self.subscribe(interest, handler)
    }

    /// Returns true if there are any event handlers registered.
    /// Useful for skipping expensive work when no one is listening.
    pub fn has_handlers(&self) -> bool {
        !self.snapshot().handlers.is_empty()
    }

    /// Whether any registered handler is interested in `kind`. Lets callers
    /// skip producing an event nobody would receive (e.g. retaining a large
    /// `HistorySync` blob when only message-only handlers are registered).
    pub fn has_handler_for(&self, kind: EventKind) -> bool {
        self.inner.has_handler_for(kind)
    }

    pub fn dispatch(&self, event: Event) {
        let kind = event.kind();
        if !self.has_handler_for(kind) {
            return;
        }
        let snapshot = self.snapshot();
        let event = Arc::new(event);
        for entry in &snapshot.handlers {
            if entry.interest.wants(kind) {
                entry.handler.handle_event(Arc::clone(&event));
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct SelfPushNameUpdated {
    pub from_server: bool,
    pub old_name: String,
    pub new_name: String,
}

/// Type of device list update notification.
/// Matches WhatsApp Web's device notification types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum DeviceListUpdateType {
    /// A device was added to the user's account
    #[wire = "add"]
    Add,
    /// A device was removed from the user's account
    #[wire = "remove"]
    Remove,
    /// Device information was updated
    #[wire = "update"]
    Update,
}

impl From<crate::stanza::devices::DeviceNotificationType> for DeviceListUpdateType {
    fn from(t: crate::stanza::devices::DeviceNotificationType) -> Self {
        match t {
            crate::stanza::devices::DeviceNotificationType::Add => Self::Add,
            crate::stanza::devices::DeviceNotificationType::Remove => Self::Remove,
            crate::stanza::devices::DeviceNotificationType::Update => Self::Update,
        }
    }
}

/// Device information from notification.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct DeviceNotificationInfo {
    /// Device ID (extracted from JID)
    pub device_id: u32,
    /// Optional key index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_index: Option<u32>,
}

/// Device list update notification.
/// Emitted when a user's device list changes (device added/removed/updated).
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct DeviceListUpdate {
    /// The user whose device list changed (from attribute)
    pub user: Jid,
    /// Optional LID user (for LID-PN mapping)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lid_user: Option<Jid>,
    /// Type of update (add/remove/update)
    pub update_type: DeviceListUpdateType,
    /// Affected devices with detailed info
    pub devices: Vec<DeviceNotificationInfo>,
    /// Key index info (for add/remove)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_index: Option<crate::stanza::devices::KeyIndexInfo>,
    /// Contact hash (for update - used for contact lookup)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_hash: Option<String>,
}

/// Identity key changed for a user (e.g., user reinstalled WhatsApp).
/// Emitted after device record cleanup so sessions and sender keys are cleared.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct IdentityChange {
    /// The user whose identity changed
    pub user: Jid,
    /// Optional LID for the user
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lid_user: Option<Jid>,
    /// `true` when detected locally while saving a peer's new identity during
    /// decrypt (mirrors WA Web `saveIdentity` -> `handleNewIdentity`), `false`
    /// when triggered by the server's `<identity/>` notification.
    pub implicit: bool,
}

/// Type of business status update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum BusinessUpdateType {
    #[wire = "removed_as_business"]
    RemovedAsBusiness,
    #[wire = "verified_name_changed"]
    VerifiedNameChanged,
    #[wire = "profile_updated"]
    ProfileUpdated,
    #[wire = "products_updated"]
    ProductsUpdated,
    #[wire = "collections_updated"]
    CollectionsUpdated,
    #[wire = "subscriptions_updated"]
    SubscriptionsUpdated,
    #[wire_default]
    #[wire = "unknown"]
    Unknown,
}

impl From<crate::stanza::business::BusinessNotificationType> for BusinessUpdateType {
    fn from(t: crate::stanza::business::BusinessNotificationType) -> Self {
        match t {
            crate::stanza::business::BusinessNotificationType::RemoveJid
            | crate::stanza::business::BusinessNotificationType::RemoveHash => {
                Self::RemovedAsBusiness
            }
            crate::stanza::business::BusinessNotificationType::VerifiedNameJid
            | crate::stanza::business::BusinessNotificationType::VerifiedNameHash => {
                Self::VerifiedNameChanged
            }
            crate::stanza::business::BusinessNotificationType::Profile
            | crate::stanza::business::BusinessNotificationType::ProfileHash => {
                Self::ProfileUpdated
            }
            crate::stanza::business::BusinessNotificationType::Product => Self::ProductsUpdated,
            crate::stanza::business::BusinessNotificationType::Collection => {
                Self::CollectionsUpdated
            }
            crate::stanza::business::BusinessNotificationType::Subscriptions => {
                Self::SubscriptionsUpdated
            }
            crate::stanza::business::BusinessNotificationType::Unknown => Self::Unknown,
        }
    }
}

/// Business status update notification.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct BusinessStatusUpdate {
    /// The business account whose status changed.
    pub jid: Jid,
    pub update_type: BusinessUpdateType,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_jid: Option<Jid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_name: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub product_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub collection_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subscriptions: Vec<BusinessSubscription>,
}

/// A contact's default disappearing messages setting changed.
///
/// Sent by the server as `<notification type="disappearing_mode">`.
/// WA Web: `WAWebHandleDisappearingModeNotification` →
/// `WAWebUpdateDisappearingModeForContact`.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct DisappearingModeChanged {
    /// The contact whose setting changed.
    pub from: Jid,
    /// New duration in seconds (0 = disabled, 86400 = 24h, etc.).
    pub duration: u32,
    /// When the setting was changed.
    /// Consumers should only apply this if it's newer than their stored value.
    #[serde(with = "chrono::serde::ts_seconds")]
    pub setting_timestamp: DateTime<Utc>,
}

/// An event dispatched by the client to registered handlers.
///
/// # Stability
///
/// The enum is `#[non_exhaustive]`, so match arms must keep a `_` catch-all.
/// Every payload struct is sealed the same way — `#[non_exhaustive]` plus a
/// `bon` builder for construction — so a payload can gain fields without
/// breaking consumers. Read the fields you need (`ack.class`) or keep a `..`
/// rest when destructuring, rather than binding every field. Construct payloads
/// via their generated builder (`ServerAck::builder()…build()`), not a struct
/// literal; a maybe-absent field is always modeled as `Option<T>` (with a
/// `maybe_*` setter), never an empty-string / zero sentinel. Even the
/// unit-marker events are empty sealed structs built as
/// `Connected::builder().build()`, so a new payload must follow the pattern.
///
/// `Debug` output is intentionally variant-name-only (`{:?}` prints e.g.
/// `Messages`): a derived impl would drag the entire generated proto `Debug`
/// graph into the binary. `Serialize` the event when full contents are needed.
#[derive(Clone, Serialize)]
#[non_exhaustive]
pub enum Event {
    Connected(Connected),
    Disconnected(Disconnected),
    PairSuccess(PairSuccess),
    PairError(PairError),
    LoggedOut(LoggedOut),
    PairingQrCode(PairingQrCode),
    PairingCode(PairingCode),
    PairingCodeRefresh(PairingCodeRefresh),
    QrScannedWithoutMultidevice(QrScannedWithoutMultidevice),
    ClientOutdated(ClientOutdated),

    /// One or more decrypted inbound messages, in arrival order. Live traffic arrives as single-message
    /// batches; an offline drain delivers one batch per durable commit, so a
    /// consumer never sees a message that a registered durability hook has
    /// not committed. The `Arc` slice is shared with the hook call — same
    /// items, same order, no copies.
    ///
    /// With a hook registered this event is at-least-once, like the hook: a
    /// redelivery whose buffered copy survived (e.g. the post-commit cleanup
    /// failed and the ack was lost) replays through the same commit and
    /// dispatches again. Exceptions that bypass the hook: newsletter
    /// messages (plaintext, acked on their own path, never redelivered) and
    /// PDO placeholder recoveries (identified by
    /// `info.unavailable_request_id`) dispatch event-only.
    Messages(MessageBatch),
    Receipt(Receipt),
    /// The server `<ack>`-ed (or nack-ed) an outgoing stanza.
    ///
    /// Observe-only: dispatched for every server `<ack>` that carries an id,
    /// before and independently of the internal ack-waiter resolution, so it
    /// never interacts with the send/phash flow. Lets consumers measure
    /// send → server-accept latency and surface nack codes programmatically
    /// (today nacks are only visible as `warn!` logs).
    ServerAck(ServerAck),
    UndecryptableMessage(UndecryptableMessage),
    #[serde(skip)]
    Notification(Arc<OwnedNodeRef>),

    ChatPresence(ChatPresenceUpdate),
    Presence(PresenceUpdate),
    PictureUpdate(PictureUpdate),
    UserAboutUpdate(UserAboutUpdate),
    ContactUpdated(ContactUpdated),
    ContactNumberChanged(ContactNumberChanged),
    ContactSyncRequested(ContactSyncRequested),

    /// Group metadata/settings/participant change from w:gp2 notification.
    GroupUpdate(GroupUpdate),
    ContactUpdate(ContactUpdate),

    /// Incoming `<call>` stanza from the server (offer, preaccept, accept,
    /// reject, terminate). Mirror of WA Web's inbound call signaling.
    IncomingCall(IncomingCall),

    /// A call that must not ring (e.g. an offer replayed from the offline queue on reconnect).
    /// Surfaced separately from [`IncomingCall`] so a consumer cannot accidentally auto-accept a
    /// dead call. Mirror of WA Web's `cancel_call` + `missed_call`.
    MissedCall(MissedCall),

    /// An incoming call we were ringing for was answered/declined on another of our devices, so the
    /// caller dismissed this one. Distinct from [`MissedCall`] -- mirrors WA Web's AcceptedElsewhere /
    /// Rejected call-log outcomes (`<terminate reason="accepted_elsewhere"|"rejected_elsewhere">`).
    CallEndedElsewhere(CallEndedElsewhere),

    PushNameUpdate(PushNameUpdate),
    SelfPushNameUpdated(SelfPushNameUpdated),
    PinUpdate(PinUpdate),
    MuteUpdate(MuteUpdate),
    ArchiveUpdate(ArchiveUpdate),
    StarUpdate(StarUpdate),
    MarkChatAsReadUpdate(MarkChatAsReadUpdate),
    DeleteChatUpdate(DeleteChatUpdate),
    ClearChatUpdate(ClearChatUpdate),
    UserStatusMuteUpdate(UserStatusMuteUpdate),
    DeleteMessageForMeUpdate(DeleteMessageForMeUpdate),
    LabelEditUpdate(LabelEditUpdate),
    LabelAssociationUpdate(LabelAssociationUpdate),

    HistorySync(Box<LazyHistorySync>),
    OfflineSyncPreview(OfflineSyncPreview),
    OfflineSyncCompleted(OfflineSyncCompleted),

    /// Device list changed for a user (device added/removed/updated)
    DeviceListUpdate(DeviceListUpdate),

    /// Identity key changed (user reinstalled WhatsApp)
    IdentityChange(IdentityChange),

    /// Business account status changed (verified name, profile, conversion to personal)
    BusinessStatusUpdate(BusinessStatusUpdate),

    StreamReplaced(StreamReplaced),
    TemporaryBan(TemporaryBan),
    ConnectFailure(ConnectFailure),
    StreamError(StreamError),

    /// A contact changed their default disappearing messages setting.
    DisappearingModeChanged(DisappearingModeChanged),

    /// Newsletter live update (reaction counts changed, message updates, etc.).
    NewsletterLiveUpdate(NewsletterLiveUpdate),

    /// Raw decoded stanza, emitted before router dispatch.
    /// Library extension — no WA Web equivalent (WA Web has no raw stanza observer).
    /// Gated by `Client::acquire_raw_node_forwarding()` to avoid overhead when unused.
    #[serde(skip)]
    RawNode(Arc<OwnedNodeRef>),

    /// Server-pushed MEX (GraphQL) update. Routed by the textual `op_name`,
    /// which is stable across WA Web bundle releases.
    MexNotification(MexNotification),

    /// SHORTCAKE_PASSKEY: the server asked for a WebAuthn assertion to gate this
    /// companion link. Carries the verbatim `PublicKeyCredentialRequestOptions`
    /// JSON; the host obtains an assertion (via [`crate::sync_marker`]-agnostic
    /// authenticator) and the client sends it back. If a passkey authenticator is
    /// registered the client drives this automatically; this event is for hosts
    /// that drive the assertion manually.
    PairPasskeyRequest(PairPasskeyRequest),

    /// SHORTCAKE_PASSKEY: the link reached the verification stage. `code` is the
    /// 8-char (dashed) pairing code; when `skip_handoff_ux` is set, continuity was
    /// proven via the handoff proof and the code need not be shown to the user.
    PairPasskeyConfirmation(PairPasskeyConfirmation),

    /// SHORTCAKE_PASSKEY: the passkey link failed. `continuation` distinguishes a
    /// failure during the continuation/verification stage from the initial request.
    PairPasskeyError(PairPasskeyError),
}

/// Payload for [`Event::PairPasskeyRequest`].
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairPasskeyRequest {
    /// Verbatim `PublicKeyCredentialRequestOptions` JSON from the server. Pass it
    /// straight to a WebAuthn `get` (e.g. Android Credential Manager), or parse it
    /// with `whatsapp_rust::passkey::parse_request_options`.
    pub request_options_json: String,
}

/// Payload for [`Event::PairPasskeyConfirmation`].
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairPasskeyConfirmation {
    pub code: String,
    pub skip_handoff_ux: bool,
}

/// Payload for [`Event::PairPasskeyError`].
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairPasskeyError {
    pub error: String,
    pub continuation: bool,
}

/// `payload` shape depends on `op_name`. `offline` mirrors the raw string
/// the server sets when replaying backlog (often a timestamp); presence
/// alone signals backlog vs live.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct MexNotification {
    pub op_name: String,
    pub from: Option<Jid>,
    pub stanza_id: Option<String>,
    pub offline: Option<String>,
    pub payload: serde_json::Value,
}

impl Event {
    /// The [`EventKind`] discriminant for this event, used by the bus to test
    /// handler interest before materializing the event.
    pub fn kind(&self) -> EventKind {
        match self {
            Event::Connected(_) => EventKind::Connected,
            Event::Disconnected(_) => EventKind::Disconnected,
            Event::PairSuccess(_) => EventKind::PairSuccess,
            Event::PairError(_) => EventKind::PairError,
            Event::LoggedOut(_) => EventKind::LoggedOut,
            Event::PairingQrCode(_) => EventKind::PairingQrCode,
            Event::PairingCode(_) => EventKind::PairingCode,
            Event::PairingCodeRefresh(_) => EventKind::PairingCodeRefresh,
            Event::QrScannedWithoutMultidevice(_) => EventKind::QrScannedWithoutMultidevice,
            Event::ClientOutdated(_) => EventKind::ClientOutdated,
            Event::Messages(_) => EventKind::Messages,
            Event::Receipt(_) => EventKind::Receipt,
            Event::UndecryptableMessage(_) => EventKind::UndecryptableMessage,
            Event::Notification(_) => EventKind::Notification,
            Event::ChatPresence(_) => EventKind::ChatPresence,
            Event::Presence(_) => EventKind::Presence,
            Event::PictureUpdate(_) => EventKind::PictureUpdate,
            Event::UserAboutUpdate(_) => EventKind::UserAboutUpdate,
            Event::ContactUpdated(_) => EventKind::ContactUpdated,
            Event::ContactNumberChanged(_) => EventKind::ContactNumberChanged,
            Event::ContactSyncRequested(_) => EventKind::ContactSyncRequested,
            Event::GroupUpdate(_) => EventKind::GroupUpdate,
            Event::ContactUpdate(_) => EventKind::ContactUpdate,
            Event::IncomingCall(_) => EventKind::IncomingCall,
            Event::MissedCall(_) => EventKind::MissedCall,
            Event::CallEndedElsewhere(_) => EventKind::CallEndedElsewhere,
            Event::PushNameUpdate(_) => EventKind::PushNameUpdate,
            Event::SelfPushNameUpdated(_) => EventKind::SelfPushNameUpdated,
            Event::PinUpdate(_) => EventKind::PinUpdate,
            Event::MuteUpdate(_) => EventKind::MuteUpdate,
            Event::ArchiveUpdate(_) => EventKind::ArchiveUpdate,
            Event::StarUpdate(_) => EventKind::StarUpdate,
            Event::MarkChatAsReadUpdate(_) => EventKind::MarkChatAsReadUpdate,
            Event::DeleteChatUpdate(_) => EventKind::DeleteChatUpdate,
            Event::ClearChatUpdate(_) => EventKind::ClearChatUpdate,
            Event::UserStatusMuteUpdate(_) => EventKind::UserStatusMuteUpdate,
            Event::DeleteMessageForMeUpdate(_) => EventKind::DeleteMessageForMeUpdate,
            Event::LabelEditUpdate(_) => EventKind::LabelEditUpdate,
            Event::LabelAssociationUpdate(_) => EventKind::LabelAssociationUpdate,
            Event::HistorySync(_) => EventKind::HistorySync,
            Event::OfflineSyncPreview(_) => EventKind::OfflineSyncPreview,
            Event::OfflineSyncCompleted(_) => EventKind::OfflineSyncCompleted,
            Event::DeviceListUpdate(_) => EventKind::DeviceListUpdate,
            Event::IdentityChange(_) => EventKind::IdentityChange,
            Event::BusinessStatusUpdate(_) => EventKind::BusinessStatusUpdate,
            Event::StreamReplaced(_) => EventKind::StreamReplaced,
            Event::TemporaryBan(_) => EventKind::TemporaryBan,
            Event::ConnectFailure(_) => EventKind::ConnectFailure,
            Event::StreamError(_) => EventKind::StreamError,
            Event::DisappearingModeChanged(_) => EventKind::DisappearingModeChanged,
            Event::NewsletterLiveUpdate(_) => EventKind::NewsletterLiveUpdate,
            Event::RawNode(_) => EventKind::RawNode,
            Event::MexNotification(_) => EventKind::MexNotification,
            Event::PairPasskeyRequest(_) => EventKind::PairPasskeyRequest,
            Event::PairPasskeyConfirmation(_) => EventKind::PairPasskeyConfirmation,
            Event::PairPasskeyError(_) => EventKind::PairPasskeyError,
            Event::ServerAck(_) => EventKind::ServerAck,
        }
    }

    /// This event as its [`MessageBatch`], or `None` for any other event kind.
    /// Use this when you need the batch's [`origin`](MessageBatch::origin) or
    /// want to treat the messages as a whole; to just iterate the messages,
    /// prefer [`messages`](Self::messages).
    pub fn as_messages(&self) -> Option<&MessageBatch> {
        match self {
            Event::Messages(batch) => Some(batch),
            _ => None,
        }
    }

    /// The inbound messages carried by this event, in arrival order; an empty
    /// iterator for every other event kind (so it drops cleanly into a
    /// `for msg in event.messages()` scan over a mixed event stream).
    pub fn messages(&self) -> impl Iterator<Item = &InboundMessage> {
        self.as_messages().into_iter().flatten()
    }
}

// Variant name only, on purpose: Messages/HistorySync transitively contain
// `wa::Message`, so a derived impl would keep the entire generated proto Debug
// graph (hundreds of KiB) in the binary. Serialize the event for full contents.
impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.kind(), f)
    }
}

/// One decrypted inbound message. The same items (and order) back both
/// consumer surfaces: the durability hook's batch and [`Event::Messages`].
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct InboundMessage {
    pub message: Arc<wa::Message>,
    pub info: Arc<MessageInfo>,
}

/// How a [`MessageBatch`] was delivered. This describes the delivery shape,
/// not a message's provenance: whether a stanza came from the offline queue
/// is `info.is_offline` on each [`InboundMessage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum BatchOrigin {
    /// Delivered immediately as a batch of one: live traffic, and redelivery
    /// replays that commit outside an accumulated batch.
    Live,
    /// An accumulated batch from the offline drain, one per durable commit
    /// (WA Web's MessageProcessorCache snapshot granularity).
    OfflineDrain,
}

/// Payload of [`Event::Messages`]: the decrypted messages of one durable
/// commit, in arrival order. Behaves as a collection of its messages —
/// `for msg in &batch`, `batch.iter()`, `batch.len()` — with `origin`
/// carrying the delivery shape alongside.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct MessageBatch {
    pub messages: Arc<[InboundMessage]>,
    pub origin: BatchOrigin,
}

impl MessageBatch {
    pub fn iter(&self) -> std::slice::Iter<'_, InboundMessage> {
        self.messages.iter()
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    pub fn first(&self) -> Option<&InboundMessage> {
        self.messages.first()
    }
}

impl<'a> IntoIterator for &'a MessageBatch {
    type Item = &'a InboundMessage;
    type IntoIter = std::slice::Iter<'a, InboundMessage>;

    fn into_iter(self) -> Self::IntoIter {
        self.messages.iter()
    }
}

/// A newsletter live update notification, typically containing updated
/// reaction counts for one or more messages.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct NewsletterLiveUpdate {
    /// The newsletter channel this update belongs to.
    pub newsletter_jid: Jid,
    pub messages: Vec<NewsletterLiveUpdateMessage>,
}

/// A single message entry in a newsletter live update.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct NewsletterLiveUpdateMessage {
    pub server_id: u64,
    pub reactions: Vec<NewsletterLiveUpdateReaction>,
}

/// A reaction count in a newsletter live update.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct NewsletterLiveUpdateReaction {
    pub code: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairSuccess {
    pub id: Jid,
    pub lid: Jid,
    pub business_name: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairError {
    pub id: Jid,
    pub lid: Jid,
    pub business_name: String,
    pub platform: String,
    pub error: String,
}

/// A QR code the consumer renders during multi-device pairing.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairingQrCode {
    /// The QR payload to render.
    pub code: String,
    /// How long this code stays valid before the next one rotates in.
    pub timeout: std::time::Duration,
}

/// Generated pair code for phone number linking.
/// User should enter this code on their phone in WhatsApp > Linked Devices.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairingCode {
    /// The 8-character pairing code to display.
    pub code: String,
    /// Approximate validity duration (~180 seconds).
    pub timeout: std::time::Duration,
}

/// The server asked the companion to refresh an in-progress phone-number
/// pairing code (WA Web `refreshAltLinkingCode` / `forceManualRefresh`).
/// Only emitted while a pair-code flow is outstanding and the server's ref
/// matches it. The consumer should request a fresh code via
/// `pair_with_code`; the previous code is no longer guaranteed valid.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PairingCodeRefresh {
    /// `true` when the server set `force_manual_refresh` — the code must be
    /// re-requested explicitly rather than auto-rotated.
    pub force_manual: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct QrScannedWithoutMultidevice {}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ClientOutdated {}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct Connected {}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct LoggedOut {
    pub on_connect: bool,
    pub reason: ConnectFailureReason,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct StreamReplaced {}

#[derive(Debug, Clone, PartialEq, Eq, crate::WireEnum)]
#[wire(kind = "int")]
pub enum TempBanReason {
    #[wire = 101]
    SentToTooManyPeople,
    #[wire = 102]
    BlockedByUsers,
    #[wire = 103]
    CreatedTooManyGroups,
    #[wire = 104]
    SentTooManySameMessage,
    #[wire = 106]
    BroadcastList,
    #[wire_fallback]
    Unknown(i32),
}

impl fmt::Display for TempBanReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            Self::SentToTooManyPeople => {
                "you sent too many messages to people who don't have you in their address books"
            }
            Self::BlockedByUsers => "too many people blocked you",
            Self::CreatedTooManyGroups => {
                "you created too many groups with people who don't have you in their address books"
            }
            Self::SentTooManySameMessage => "you sent the same message to too many people",
            Self::BroadcastList => "you sent too many messages to a broadcast list",
            Self::Unknown(_) => "you may have violated the terms of service (unknown error)",
        };
        write!(f, "{}: {}", self.code(), msg)
    }
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct TemporaryBan {
    pub code: TempBanReason,
    pub expire: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
#[wire(kind = "int")]
pub enum ConnectFailureReason {
    #[wire = 400]
    Generic,
    #[wire = 401]
    LoggedOut,
    #[wire = 402]
    TempBanned,
    /// WA Web 403 = REASON_LOCKED: account/device locked server-side; the client
    /// logs out as LogoutReason.AccountLocked. (A manual unlink instead arrives
    /// as `<conflict type="device_removed">`.)
    #[wire = 403]
    AccountLocked,
    #[wire = 406]
    UnknownLogout,
    #[wire = 405]
    ClientOutdated,
    #[wire = 409]
    BadUserAgent,
    #[wire = 413]
    CatExpired,
    #[wire = 414]
    CatInvalid,
    #[wire = 415]
    NotFound,
    #[wire = 418]
    ClientUnknown,
    #[wire = 500]
    InternalServerError,
    #[wire = 501]
    Experimental,
    #[wire = 503]
    ServiceUnavailable,
    #[wire_fallback]
    Unknown(i32),
}

impl ConnectFailureReason {
    pub fn is_logged_out(&self) -> bool {
        matches!(
            self,
            Self::LoggedOut | Self::AccountLocked | Self::UnknownLogout
        )
    }

    pub fn should_reconnect(&self) -> bool {
        matches!(self, Self::ServiceUnavailable | Self::InternalServerError)
    }
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ConnectFailure {
    pub reason: ConnectFailureReason,
    /// The server's `message` attribute on the `<failure>` stanza, when present.
    pub message: Option<String>,
    pub raw: Option<Node>,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct StreamError {
    pub code: String,
    pub raw: Option<Node>,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct Disconnected {
    /// Why the transport ended — lets consumers tell a routine server stream
    /// recycle (`reason.is_clean_shutdown()`) from a genuine transport failure
    /// without parsing logs.
    pub reason: crate::net::DisconnectReason,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct OfflineSyncPreview {
    pub total: i32,
    pub app_data_changes: i32,
    pub messages: i32,
    pub notifications: i32,
    pub receipts: i32,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct OfflineSyncCompleted {
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum DecryptFailMode {
    #[wire = "show"]
    Show,
    #[wire = "hide"]
    Hide,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum UnavailableType {
    #[wire_default]
    #[wire = "unknown"]
    Unknown,
    #[wire = "view_once"]
    ViewOnce,
    #[wire = "hosted"]
    Hosted,
    #[wire = "bot"]
    Bot,
}

impl UnavailableType {
    /// Classify an `<unavailable>` fanout the way WA Web picks its
    /// placeholderType, honouring the same precedence (bot > hosted >
    /// view_once). Anything else is a plain fanout (`Unknown`).
    pub fn from_fanout_flags(is_bot: bool, is_hosted: bool, is_view_once: bool) -> Self {
        if is_bot {
            Self::Bot
        } else if is_hosted {
            Self::Hosted
        } else if is_view_once {
            Self::ViewOnce
        } else {
            Self::Unknown
        }
    }

    /// Bot, hosted and view-once fanouts are the three subtypes
    /// `WAWebNonMessageDataRequestPlaceholderMessageResendUtils` excludes from
    /// placeholder-resend: the phone won't share that content with a companion,
    /// so a resend to our own device only returns empty. A plain fanout
    /// (`Unknown`) stays recoverable.
    pub fn is_unrecoverable_fanout(&self) -> bool {
        matches!(self, Self::ViewOnce | Self::Hosted | Self::Bot)
    }
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct UndecryptableMessage {
    pub info: Arc<MessageInfo>,
    pub is_unavailable: bool,
    pub unavailable_type: UnavailableType,
    pub decrypt_fail_mode: DecryptFailMode,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct Receipt {
    pub source: crate::types::message::MessageSource,
    pub message_ids: Vec<MessageId>,
    pub timestamp: DateTime<Utc>,
    pub r#type: ReceiptType,
    /// True when the receipt carried the `offline` attribute, i.e. it was drained
    /// from the server's offline queue on reconnect rather than delivered live.
    /// Mirrors WA Web `incomingMsgReceiptParser` (`offline: maybeAttrString`).
    pub offline: bool,
}

/// Payload of [`Event::ServerAck`]: the server acknowledged (or nacked) an
/// outgoing stanza. Server acks cover every outgoing stanza class — message,
/// receipt, notification, call — so consumers should filter on [`class`](Self::class)
/// before correlating ids.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ServerAck {
    /// Id of the acked stanza (for a sent message, its message id).
    pub id: String,
    /// Stanza class the ack refers to (`"message"`, `"receipt"`,
    /// `"notification"`, `"call"`, …). `None` when the server omits it.
    pub class: Option<String>,
    /// Chat/entity the ack refers to, when present and parseable.
    pub from: Option<Jid>,
    /// Server timestamp from the ack's `t` attribute, when present. For a
    /// message ack this is the authoritative send timestamp (whatsmeow reads
    /// the same attribute into `SendResponse.Timestamp`).
    pub timestamp: Option<DateTime<Utc>>,
    /// Nack code (e.g. `"479"`) when the server rejected the stanza; `None`
    /// for a plain ack.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ChatPresenceUpdate {
    pub source: crate::types::message::MessageSource,
    pub state: ChatPresence,
    pub media: ChatPresenceMedia,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PresenceUpdate {
    /// The contact whose presence changed.
    pub from: Jid,
    pub unavailable: bool,
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PictureUpdate {
    /// The JID whose picture changed (user or group).
    pub jid: Jid,
    /// The user who made the change. Present for group picture changes
    /// (the admin who changed it). `None` for personal picture updates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<Jid>,
    pub timestamp: DateTime<Utc>,
    /// Whether the picture was removed (true) or set/updated (false).
    pub removed: bool,
    /// The server-assigned picture ID (from `<set id="..."/>`). `None` for deletions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct UserAboutUpdate {
    /// The contact whose about text changed.
    pub jid: Jid,
    pub status: String,
    pub timestamp: DateTime<Utc>,
}

/// A contact's profile changed (server notification).
///
/// Emitted from `<notification type="contacts"><update jid="..."/>`.
/// WA Web resets cached presence and refreshes the profile picture on this
/// event — consumers should invalidate any cached presence/profile data.
///
/// Not to be confused with [`ContactUpdate`] which comes from app-state
/// sync mutations (different source, different payload).
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ContactUpdated {
    /// The contact whose profile was updated.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
}

/// A contact changed their phone number.
///
/// Emitted from `<notification type="contacts"><modify old="..." new="..."
/// old_lid="..." new_lid="..."/>`.
///
/// The library updates the global LID-PN cache when both `old_lid` and
/// `new_lid` are present, mirroring `WAWebDBCreateLidPnMappings`. No Signal
/// session is wiped (WA Web `WAWebHandleContactNotification` also leaves
/// sessions intact). Group participant updates arrive via separate
/// `w:gp2` notifications, so per-group caches are not touched here.
/// Consumers can subscribe and refresh their own caches if needed.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ContactNumberChanged {
    /// Old phone number JID.
    pub old_jid: Jid,
    /// New phone number JID.
    pub new_jid: Jid,
    /// Old LID (if provided by server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_lid: Option<Jid>,
    /// New LID (if provided by server).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new_lid: Option<Jid>,
    pub timestamp: DateTime<Utc>,
}

/// Server requests a full contact re-sync.
///
/// Emitted from `<notification type="contacts"><sync after="..."/>`.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ContactSyncRequested {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
}

/// Group update notification.
///
/// Emitted for each action in a `<notification type="w:gp2">` stanza.
/// A single notification may produce multiple `GroupUpdate` events (one per action).
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupUpdate {
    /// The group this update applies to
    pub group_jid: Jid,
    /// Identifier of the source notification stanza.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_id: Option<String>,
    /// Display name supplied with the source notification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    /// Raw offline-delivery marker supplied with the source notification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline: Option<String>,
    /// Zero-based emitted-action index within the source notification.
    #[builder(default)]
    pub action_index: u32,
    /// The admin/user who triggered the change (`participant` attribute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant: Option<Jid>,
    /// Phone number JID of the participant (for LID-addressed groups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_pn: Option<Jid>,
    /// Username of the participant, when supplied by the group notification.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_username: Option<String>,
    /// Country code supplied for the participant by the server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_country_code: Option<String>,
    /// When the change occurred
    pub timestamp: DateTime<Utc>,
    /// Whether the group uses LID addressing mode
    pub is_lid_addressing_mode: bool,
    /// Whether participant identity information was incomplete in the source stanza.
    #[builder(default)]
    pub has_incomplete_participant_information: bool,
    /// The specific action
    pub action: crate::stanza::groups::GroupNotificationAction,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ContactUpdate {
    /// The chat/contact this sync action applies to.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::ContactAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PushNameUpdate {
    /// The contact who changed their push name.
    pub jid: Jid,
    pub message: Box<MessageInfo>,
    pub old_push_name: String,
    pub new_push_name: String,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct PinUpdate {
    /// The chat being pinned or unpinned.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::PinAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct MuteUpdate {
    /// The chat being muted or unmuted.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::MuteAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ArchiveUpdate {
    /// The chat being archived or unarchived.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::ArchiveChatAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct StarUpdate {
    /// The chat containing the starred or unstarred message.
    pub chat_jid: Jid,
    /// The participant who sent the message. `Some` for group messages from
    /// others, `None` for self-authored or 1-on-1 messages (wire value `"0"`).
    pub participant_jid: Option<Jid>,
    pub message_id: String,
    pub from_me: bool,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::StarAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct MarkChatAsReadUpdate {
    /// The chat being marked as read or unread.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::MarkChatAsReadAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct DeleteChatUpdate {
    /// The chat being deleted.
    pub jid: Jid,
    /// From the index, not the proto — DeleteChatAction only has messageRange.
    pub delete_media: bool,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::DeleteChatAction>,
    pub from_full_sync: bool,
}

/// A chat's messages were cleared (kept) on a linked device.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ClearChatUpdate {
    /// The chat being cleared.
    pub jid: Jid,
    /// From the index, not the proto — ClearChatAction only has messageRange.
    pub delete_starred: bool,
    /// From the index, not the proto.
    pub delete_media: bool,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::ClearChatAction>,
    pub from_full_sync: bool,
}

/// A contact/group/newsletter's status updates were muted/unmuted on a linked device.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct UserStatusMuteUpdate {
    /// The entity whose status was (un)muted.
    pub jid: Jid,
    /// `true` = status muted, `false` = unmuted.
    pub muted: bool,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::UserStatusMuteAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct DeleteMessageForMeUpdate {
    /// The chat containing the deleted message.
    pub chat_jid: Jid,
    pub participant_jid: Option<Jid>,
    pub message_id: String,
    pub from_me: bool,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::DeleteMessageForMeAction>,
    pub from_full_sync: bool,
}

/// A label was created, renamed/recolored, or deleted on a linked device.
/// `action.deleted == Some(true)` means the label was removed.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct LabelEditUpdate {
    /// The label identifier (the index key, not a JID).
    pub label_id: String,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::LabelEditAction>,
    pub from_full_sync: bool,
}

/// A label was associated with or removed from a chat on a linked device.
/// `action.labeled == Some(true)` means the label was added to the chat.
#[derive(Debug, Clone, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct LabelAssociationUpdate {
    /// The label identifier.
    pub label_id: String,
    /// The chat the label was associated with or removed from.
    pub chat_jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::LabelAssociationAction>,
    pub from_full_sync: bool,
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use buffa::Message;
    use waproto::whatsapp as wa;

    #[test]
    fn group_update_builder_defaults_additive_scalar_fields() {
        let update = GroupUpdate::builder()
            .group_jid("120363000000000001@g.us".parse().unwrap())
            .timestamp(DateTime::<Utc>::UNIX_EPOCH)
            .is_lid_addressing_mode(false)
            .action(crate::stanza::groups::GroupNotificationAction::Unlocked)
            .build();

        assert_eq!(update.action_index, 0);
        assert!(!update.has_incomplete_participant_information);
    }

    #[test]
    fn unavailable_fanout_flags_follow_wa_web_precedence() {
        use UnavailableType::*;
        // bot wins over hosted and view_once
        assert_eq!(UnavailableType::from_fanout_flags(true, true, true), Bot);
        assert_eq!(UnavailableType::from_fanout_flags(true, false, false), Bot);
        // hosted wins over view_once
        assert_eq!(
            UnavailableType::from_fanout_flags(false, true, true),
            Hosted
        );
        assert_eq!(
            UnavailableType::from_fanout_flags(false, false, true),
            ViewOnce
        );
        // nothing set is a plain fanout
        assert_eq!(
            UnavailableType::from_fanout_flags(false, false, false),
            Unknown
        );
    }

    #[test]
    fn only_plain_fanout_is_recoverable() {
        use UnavailableType::*;
        assert!(Bot.is_unrecoverable_fanout());
        assert!(Hosted.is_unrecoverable_fanout());
        assert!(ViewOnce.is_unrecoverable_fanout());
        assert!(!Unknown.is_unrecoverable_fanout());
    }

    /// Build a HistorySync proto with conversations, returning its
    /// zlib-compressed wire form plus the exact decompressed size.
    fn make_compressed_history_sync(conversations: Vec<wa::Conversation>) -> (Bytes, usize) {
        use flate2::{Compression, write::ZlibEncoder};
        use std::io::Write;
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap,
            conversations,
            ..Default::default()
        };
        let raw = hs.encode_to_vec();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).unwrap();
        (Bytes::from(encoder.finish().unwrap()), raw.len())
    }

    fn lazy_from(conversations: Vec<wa::Conversation>) -> LazyHistorySync {
        let (compressed, raw_len) = make_compressed_history_sync(conversations);
        LazyHistorySync::new(compressed, raw_len, 0, None, None)
    }

    #[test]
    fn lazy_history_sync_get_decodes() {
        let lazy = lazy_from(vec![wa::Conversation {
            id: "chat@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);

        let hs = lazy.get().expect("should decode");
        assert_eq!(hs.conversations.len(), 1);
        assert_eq!(hs.conversations[0].id, "chat@s.whatsapp.net");
    }

    #[test]
    fn lazy_history_sync_caches_decode() {
        let lazy = lazy_from(vec![wa::Conversation {
            id: "test@g.us".to_string(),
            ..Default::default()
        }]);

        let first = lazy.get().expect("first decode");
        let second = lazy.get().expect("second decode");
        // Same reference — OnceLock cached it
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn lazy_history_sync_cheap_metadata() {
        let (compressed, raw_len) = make_compressed_history_sync(vec![]);
        let lazy = LazyHistorySync::new(compressed.clone(), raw_len, 3, Some(2), Some(50));

        assert_eq!(lazy.sync_type(), 3);
        assert_eq!(lazy.chunk_order(), Some(2));
        assert_eq!(lazy.progress(), Some(50));
        assert_eq!(lazy.decompressed_size(), raw_len);
        assert_eq!(lazy.compressed_bytes(), &compressed);
    }

    #[test]
    fn lazy_history_sync_peer_data_request_session_id() {
        let (compressed, raw_len) = make_compressed_history_sync(vec![]);

        let unset = LazyHistorySync::new(compressed.clone(), raw_len, 0, None, None);
        assert_eq!(unset.peer_data_request_session_id(), None);

        let set = LazyHistorySync::new(compressed, raw_len, 0, None, None)
            .with_peer_data_request_session_id(Some("session-123".to_string()));
        assert_eq!(set.peer_data_request_session_id(), Some("session-123"));

        // Round-trip through Clone
        let cloned = set.clone();
        assert_eq!(cloned.peer_data_request_session_id(), Some("session-123"));
    }

    #[test]
    fn lazy_history_sync_decompress_yields_raw_proto() {
        let lazy = lazy_from(vec![wa::Conversation {
            id: "raw@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);

        // Consumer can partial-decode from the inflated bytes.
        let raw = lazy.decompress().expect("inflates");
        assert_eq!(raw.len(), lazy.decompressed_size());
        let decoded = wa::HistorySync::decode_from_slice(&raw[..]).expect("should decode");
        assert_eq!(decoded.conversations[0].id, "raw@s.whatsapp.net");

        // No caching: a second call inflates again and matches.
        assert_eq!(lazy.decompress().expect("inflates again"), raw);
    }

    #[test]
    fn lazy_history_sync_everything_keeps_working_after_get() {
        let lazy = lazy_from(vec![wa::Conversation {
            id: "kept@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);

        assert_eq!(
            lazy.get().expect("decodes").conversations[0].id,
            "kept@s.whatsapp.net"
        );

        // The compressed payload is kept: decompress() and stream() still work
        // after a successful get() (the old take-dance surprise is gone).
        let raw = lazy.decompress().expect("decompress after get()");
        assert_eq!(raw.len(), lazy.decompressed_size());
        let mut stream = lazy.stream();
        let conversation = stream
            .next_conversation()
            .expect("stream after get()")
            .expect("one conversation");
        assert_eq!(conversation.id, "kept@s.whatsapp.net");
    }

    #[test]
    fn lazy_history_sync_stream_iterates_conversations() {
        let lazy = lazy_from(vec![
            wa::Conversation {
                id: "first@s.whatsapp.net".to_string(),
                ..Default::default()
            },
            wa::Conversation {
                id: "second@s.whatsapp.net".to_string(),
                ..Default::default()
            },
        ]);

        let mut stream = lazy.stream();
        assert_eq!(
            stream.next_conversation().unwrap().unwrap().id,
            "first@s.whatsapp.net"
        );
        assert_eq!(
            stream.next_conversation().unwrap().unwrap().id,
            "second@s.whatsapp.net"
        );
        assert!(stream.next_conversation().unwrap().is_none());
        let remainder = stream.remainder().expect("remainder decodes");
        assert!(remainder.conversations.is_empty());
        assert_eq!(
            remainder.sync_type,
            wa::history_sync::HistorySyncType::InitialBootstrap
        );
    }

    #[test]
    fn lazy_history_sync_clone_is_cheap_and_redecodes() {
        let lazy = lazy_from(vec![wa::Conversation {
            id: "cloned@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);

        // Decode on the original; the clone shares the compressed buffer (no
        // deep copy) and re-decodes on demand (the cache isn't carried over).
        assert_eq!(
            lazy.get().expect("decodes").conversations[0].id,
            "cloned@s.whatsapp.net"
        );
        let cloned = lazy.clone();
        assert_eq!(
            cloned.compressed_bytes().as_ptr(),
            lazy.compressed_bytes().as_ptr(),
            "clone shares the compressed buffer"
        );
        assert_eq!(
            cloned.get().expect("clone still decodes").conversations[0].id,
            "cloned@s.whatsapp.net"
        );
    }

    #[test]
    fn lazy_history_sync_empty_proto_decodes_default() {
        // A zero-conversation HistorySync still inflates and decodes.
        let lazy = lazy_from(vec![]);
        let hs = lazy.get().expect("decodes");
        assert!(hs.conversations.is_empty());
    }

    #[test]
    fn lazy_history_sync_corrupt_bytes_returns_none() {
        // Not a zlib stream: inflating fails, get() yields None, and the
        // payload stays available for inspection.
        let lazy = LazyHistorySync::new(Bytes::from_static(&[0xFF, 0xFF, 0xFF]), 16, 0, None, None);
        assert!(lazy.get().is_none());
        assert!(lazy.decompress().is_err());
        assert_eq!(lazy.compressed_bytes().len(), 3);
    }

    #[test]
    fn lazy_history_sync_undersized_cap_fails_loud() {
        // A decompressed_size below the real inflated size trips the inflate
        // cap instead of silently over-allocating past the producer's count.
        let (compressed, raw_len) = make_compressed_history_sync(vec![wa::Conversation {
            id: "capped@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);
        let lazy = LazyHistorySync::new(compressed, raw_len - 1, 0, None, None);
        assert!(lazy.decompress().is_err());
        assert!(lazy.get().is_none());
    }

    #[test]
    fn lazy_history_sync_preserves_messages() {
        let conv = wa::Conversation {
            id: "chat@s.whatsapp.net".to_string(),
            messages: vec![wa::HistorySyncMsg {
                message: wa::WebMessageInfo {
                    key: wa::MessageKey {
                        id: Some("msg-0".to_string()),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                }
                .into(),
                msg_order_id: Some(0),
            }],
            ..Default::default()
        };
        let lazy = lazy_from(vec![conv]);

        let hs = lazy.get().expect("should decode");
        assert_eq!(hs.conversations[0].messages.len(), 1);
        assert_eq!(
            hs.conversations[0].messages[0]
                .message
                .as_option()
                .unwrap()
                .key
                .id
                .as_deref(),
            Some("msg-0")
        );
    }

    #[test]
    fn connect_failure_reason_403_is_account_locked() {
        // WA Web maps reason 403 to REASON_LOCKED (account/device locked),
        // a logout that must not auto-reconnect.
        assert_eq!(
            ConnectFailureReason::from(403),
            ConnectFailureReason::AccountLocked
        );
        assert!(ConnectFailureReason::AccountLocked.is_logged_out());
        assert!(!ConnectFailureReason::AccountLocked.should_reconnect());

        assert!(ConnectFailureReason::LoggedOut.is_logged_out());
        assert!(ConnectFailureReason::UnknownLogout.is_logged_out());

        // Transient server errors reconnect instead of logging out.
        assert!(ConnectFailureReason::ServiceUnavailable.should_reconnect());
        assert!(ConnectFailureReason::InternalServerError.should_reconnect());
        assert!(!ConnectFailureReason::ServiceUnavailable.is_logged_out());

        // A temp ban is neither a logout nor a reconnect on this path.
        assert!(!ConnectFailureReason::TempBanned.is_logged_out());
        assert!(!ConnectFailureReason::TempBanned.should_reconnect());

        // Unrecognized codes fall through to the catch-all, never a logout.
        assert_eq!(
            ConnectFailureReason::from(499),
            ConnectFailureReason::Unknown(499)
        );
        assert!(!ConnectFailureReason::from(499).is_logged_out());
    }

    #[test]
    fn interest_filters_dispatch() {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Recorder {
            kinds: Mutex<Vec<EventKind>>,
            interest: EventInterest,
        }
        impl EventHandler for Recorder {
            fn handle_event(&self, event: Arc<Event>) {
                self.kinds.lock().unwrap().push(event.kind());
            }
            fn interest(&self) -> EventInterest {
                self.interest
            }
        }

        let bus = CoreEventBus::new();
        let only_msg = Arc::new(Recorder {
            kinds: Mutex::new(Vec::new()),
            interest: EventInterest::of(&[EventKind::Messages]),
        });
        let all = Arc::new(Recorder {
            kinds: Mutex::new(Vec::new()),
            interest: EventInterest::ALL,
        });
        let _only_msg = bus.subscribe_handler(only_msg.clone());
        let _all = bus.subscribe_handler(all.clone());

        bus.dispatch(Event::Connected(Connected::builder().build()));

        // The narrow handler (Message-only) was skipped; the ALL handler got it.
        assert!(only_msg.kinds.lock().unwrap().is_empty());
        assert_eq!(*all.kinds.lock().unwrap(), vec![EventKind::Connected]);

        // A kind nobody wants is dropped before materialization: prove the bus
        // never invokes a handler for it.
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        struct Counter;
        impl EventHandler for Counter {
            fn handle_event(&self, _: Arc<Event>) {
                CALLS.fetch_add(1, Ordering::SeqCst);
            }
            fn interest(&self) -> EventInterest {
                EventInterest::of(&[EventKind::Messages])
            }
        }
        let bus2 = CoreEventBus::new();
        let _counter = bus2.subscribe_handler(Arc::new(Counter));
        bus2.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn subscription_updates_interest_explicitly() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Dynamic {
            hits: AtomicUsize,
        }
        impl EventHandler for Dynamic {
            fn handle_event(&self, _: Arc<Event>) {
                self.hits.fetch_add(1, Ordering::SeqCst);
            }
        }

        let bus = CoreEventBus::new();
        let h = Arc::new(Dynamic {
            hits: AtomicUsize::new(0),
        });
        let subscription = bus.subscribe(EventInterest::of(&[EventKind::Messages]), h.clone());

        // Not yet interested in Connected: dropped before materialization.
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(h.hits.load(Ordering::SeqCst), 0);
        assert!(!bus.has_handler_for(EventKind::Connected));

        assert!(subscription.update_interest(EventInterest::ALL));
        assert!(bus.has_handler_for(EventKind::Connected));
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(
            h.hits.load(Ordering::SeqCst),
            1,
            "the updated subscription must receive the newly-wanted kind"
        );

        assert!(subscription.update_interest(EventInterest::none()));
        assert!(!bus.has_handler_for(EventKind::Connected));
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(h.hits.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn aggregate_interest_and_has_handler_for() {
        struct Narrow(EventInterest);
        impl EventHandler for Narrow {
            fn handle_event(&self, _: Arc<Event>) {}
            fn interest(&self) -> EventInterest {
                self.0
            }
        }

        let bus = CoreEventBus::new();
        // Empty bus: nothing is wanted and there are no handlers.
        assert!(!bus.has_handlers());
        assert!(!bus.has_handler_for(EventKind::Messages));
        assert!(!bus.has_handler_for(EventKind::Receipt));

        let _messages =
            bus.subscribe_handler(Arc::new(Narrow(EventInterest::of(&[EventKind::Messages]))));
        assert!(bus.has_handlers());
        assert!(bus.has_handler_for(EventKind::Messages));
        assert!(!bus.has_handler_for(EventKind::Receipt));

        // has_handler_for is true once any registered handler wants the kind.
        let _receipt =
            bus.subscribe_handler(Arc::new(Narrow(EventInterest::of(&[EventKind::Receipt]))));
        assert!(bus.has_handler_for(EventKind::Messages));
        assert!(bus.has_handler_for(EventKind::Receipt));
        assert!(!bus.has_handler_for(EventKind::Connected));
    }

    #[test]
    fn dispatch_preserves_handler_ordering() {
        use std::sync::Mutex;

        struct Tagged {
            id: u32,
            log: Arc<Mutex<Vec<u32>>>,
        }
        impl EventHandler for Tagged {
            fn handle_event(&self, _: Arc<Event>) {
                self.log.lock().unwrap().push(self.id);
            }
        }

        let bus = CoreEventBus::new();
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut subscriptions = Vec::new();
        for id in 0..5u32 {
            subscriptions.push(bus.subscribe_handler(Arc::new(Tagged {
                id,
                log: log.clone(),
            })));
        }
        bus.dispatch(Event::Connected(Connected::builder().build()));
        // Copy-on-write rebuilds must keep registration order intact.
        assert_eq!(*log.lock().unwrap(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn dispatch_is_reentrancy_safe_against_concurrent_add() {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // A handler that registers another handler while it is being dispatched.
        // The snapshot taken by `dispatch` must outlive the swap, so the newly
        // added handler is NOT invoked for the in-flight event and the iteration
        // does not observe a mutated list.
        struct AddsDuringDispatch {
            bus: CoreEventBus,
            invocations: Arc<AtomicUsize>,
            added: Mutex<bool>,
        }
        impl EventHandler for AddsDuringDispatch {
            fn handle_event(&self, _: Arc<Event>) {
                self.invocations.fetch_add(1, Ordering::SeqCst);
                let mut added = self.added.lock().unwrap();
                if !*added {
                    *added = true;
                    struct Late(Arc<AtomicUsize>);
                    impl EventHandler for Late {
                        fn handle_event(&self, _: Arc<Event>) {
                            self.0.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    self.bus
                        .subscribe_handler(Arc::new(Late(self.invocations.clone())))
                        .detach();
                }
            }
        }

        let bus = CoreEventBus::new();
        let invocations = Arc::new(AtomicUsize::new(0));
        let _registration = bus.subscribe_handler(Arc::new(AddsDuringDispatch {
            bus: bus.clone(),
            invocations: invocations.clone(),
            added: Mutex::new(false),
        }));

        // First dispatch: only the original handler runs, even though it adds a
        // second handler mid-flight.
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(invocations.load(Ordering::SeqCst), 1);
        assert_eq!(bus.snapshot().handlers.len(), 2);

        // Second dispatch sees both handlers (original adds nothing new now).
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(invocations.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn dropping_subscription_unregisters_handler() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Counter(Arc<AtomicUsize>);
        impl EventHandler for Counter {
            fn handle_event(&self, _: Arc<Event>) {
                self.0.fetch_add(1, Ordering::SeqCst);
            }
        }

        let bus = CoreEventBus::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let subscription = bus.subscribe_handler(Arc::new(Counter(Arc::clone(&calls))));
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        drop(subscription);
        assert!(!bus.has_handlers());
        assert!(!bus.has_handler_for(EventKind::Connected));
        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn in_flight_dispatch_can_finish_after_unsubscribe() {
        use std::sync::Barrier;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct Blocking {
            started: Arc<Barrier>,
            release: Arc<Barrier>,
            calls: Arc<AtomicUsize>,
        }
        impl EventHandler for Blocking {
            fn handle_event(&self, _: Arc<Event>) {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.started.wait();
                self.release.wait();
            }
        }

        let bus = CoreEventBus::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let started = Arc::new(Barrier::new(2));
        let release = Arc::new(Barrier::new(2));
        let subscription = bus.subscribe_handler(Arc::new(Blocking {
            started: Arc::clone(&started),
            release: Arc::clone(&release),
            calls: Arc::clone(&calls),
        }));
        let dispatch_bus = bus.clone();
        let dispatch = std::thread::spawn(move || {
            dispatch_bus.dispatch(Event::Connected(Connected::builder().build()));
        });

        started.wait();
        drop(subscription);
        release.wait();
        dispatch.join().expect("dispatch thread");
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        bus.dispatch(Event::Connected(Connected::builder().build()));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
