use crate::stanza::BusinessSubscription;
use crate::types::call::IncomingCall;
use crate::types::message::MessageInfo;
use crate::types::presence::{ChatPresence, ChatPresenceMedia, ReceiptType};
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use wacore_binary::Node;
use wacore_binary::OwnedNodeRef;
use wacore_binary::{Jid, MessageId};
use waproto::whatsapp as wa;

/// A lazily-parsed history sync blob.
///
/// Wraps the decompressed protobuf bytes and only decodes on first access.
/// With `Arc<Event>` dispatch, all handlers share the same `LazyHistorySync`
/// so `OnceLock` gives parse-once semantics for free.
///
/// Cheap metadata (`sync_type`, `chunk_order`, `progress`) is available
/// without decoding — useful for filtering events.
///
/// Call [`get()`](Self::get) for full access to conversations, pushnames,
/// global settings, past participants, call logs, and everything else in
/// the `wa::HistorySync` proto.
pub struct LazyHistorySync {
    /// Decompressed protobuf bytes. Taken (freed) once [`get()`](Self::get)
    /// materializes the owned proto, so the two halves don't coexist (~2x the
    /// decompressed size) for the event's lifetime.
    raw_bytes: Mutex<Option<Bytes>>,
    /// Original decompressed size, kept after `raw_bytes` is freed so Debug and
    /// [`raw_size()`](Self::raw_size) stay meaningful.
    raw_size: usize,
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
        // Common case (not yet decoded): carry the raw bytes for a cheap, lazy
        // clone. Once `get()` has freed the raw bytes, carry the decoded proto
        // instead (a deep copy, only when cloning an already-inspected blob) so
        // the clone stays usable rather than decoding to `None`.
        let raw = self.locked_raw().clone();
        let parsed = OnceLock::new();
        if raw.is_none()
            && let Some(decoded) = self.parsed.get()
        {
            let _ = parsed.set(decoded.clone());
        }
        Self {
            raw_bytes: Mutex::new(raw),
            raw_size: self.raw_size,
            sync_type: self.sync_type,
            chunk_order: self.chunk_order,
            progress: self.progress,
            peer_data_request_session_id: self.peer_data_request_session_id.clone(),
            parsed,
        }
    }
}

impl LazyHistorySync {
    pub fn new(
        raw_bytes: Bytes,
        sync_type: i32,
        chunk_order: Option<u32>,
        progress: Option<u32>,
    ) -> Self {
        Self {
            raw_size: raw_bytes.len(),
            raw_bytes: Mutex::new(Some(raw_bytes)),
            sync_type,
            chunk_order,
            progress,
            peer_data_request_session_id: None,
            parsed: OnceLock::new(),
        }
    }

    /// Lock the raw-bytes slot, recovering from a poisoned mutex (a poison only
    /// means a prior holder panicked; the `Option<Bytes>` is still valid).
    fn locked_raw(&self) -> std::sync::MutexGuard<'_, Option<Bytes>> {
        self.raw_bytes.lock().unwrap_or_else(|e| e.into_inner())
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

    /// `None` for server-pushed syncs (e.g. `INITIAL_BOOTSTRAP`).
    pub fn peer_data_request_session_id(&self) -> Option<&str> {
        self.peer_data_request_session_id.as_deref()
    }

    /// Full decode of the history sync proto, cached via OnceLock.
    /// Returns `None` if decoding fails.
    ///
    /// On the first successful decode the decompressed `raw_bytes` are freed, so
    /// only the owned proto is retained afterwards (not ~2x). A consumer that
    /// needs the raw bytes for partial decoding must read [`raw_bytes()`] before
    /// calling this; afterwards it returns `None`.
    ///
    /// [`raw_bytes()`]: Self::raw_bytes
    pub fn get(&self) -> Option<&wa::HistorySync> {
        let parsed = self.parsed.get_or_init(|| {
            // Cheap refcount bump; the lock is released before decoding so a
            // concurrent reader isn't blocked by the parse.
            let raw = self.locked_raw().clone()?;
            waproto::codec::history_sync_decode(&raw[..])
                .ok()
                .map(Box::new)
        });
        // Free the raw bytes only AFTER the owned proto is committed, so a
        // concurrent clone never sees both gone (raw == None implies parsed set).
        if parsed.is_some() {
            *self.locked_raw() = None;
        }
        parsed.as_deref()
    }

    /// The raw decompressed protobuf bytes for custom/partial decoding, or
    /// `None` once [`get()`](Self::get) has consumed them on a successful decode.
    pub fn raw_bytes(&self) -> Option<Bytes> {
        self.locked_raw().clone()
    }

    /// Size of the decompressed blob in bytes, available even after the raw
    /// bytes have been freed by [`get()`](Self::get).
    pub fn raw_size(&self) -> usize {
        self.raw_size
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
            .field("raw_size", &self.raw_size)
            .field("raw_freed", &self.locked_raw().is_none())
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
/// be at most 64 kinds.
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
    QrScannedWithoutMultidevice,
    ClientOutdated,
    Message,
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
    // When adding a variant, mind the 64-kind ceiling below (EventInterest packs
    // each discriminant as a bit in a u64) and keep the guard pointing at the
    // last variant.
}

impl EventKind {
    /// Bit-index ceiling: [`EventInterest`] packs each kind's discriminant into a
    /// `u64`, so there can be at most 64 kinds.
    pub const CAPACITY: u8 = 64;
}

// Build-time tripwire: a new variant that would overflow EventInterest's bitmask
// fails compilation instead of silently corrupting the mask at runtime.
const _: () = assert!((EventKind::MexNotification as u8) < EventKind::CAPACITY);

/// A set of [`EventKind`]s a handler wants delivered. The event bus skips
/// materializing and dispatching events whose kind no handler wants, so a
/// handler that subscribes to a few kinds never pays for boxing the others.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventInterest(u64);

impl EventInterest {
    /// Every kind. Default for handlers that don't narrow their interest.
    pub const ALL: EventInterest = EventInterest(u64::MAX);

    /// No kinds.
    pub const fn none() -> Self {
        EventInterest(0)
    }

    /// Interest in exactly the given kinds.
    pub fn of(kinds: &[EventKind]) -> Self {
        let mut bits = 0u64;
        let mut i = 0;
        while i < kinds.len() {
            bits |= 1u64 << (kinds[i] as u8);
            i += 1;
        }
        EventInterest(bits)
    }

    /// Add a kind to the set.
    pub const fn with(self, kind: EventKind) -> Self {
        EventInterest(self.0 | (1u64 << (kind as u8)))
    }

    /// Whether `kind` is in the set.
    #[inline]
    pub const fn wants(self, kind: EventKind) -> bool {
        self.0 & (1u64 << (kind as u8)) != 0
    }

    /// Set union, for aggregating the interests of several handlers behind one
    /// bus registration.
    pub const fn union(self, other: Self) -> Self {
        EventInterest(self.0 | other.0)
    }
}

pub trait EventHandler: crate::sync_marker::MaybeSendSync {
    fn handle_event(&self, event: Arc<Event>);

    /// Which event kinds this handler wants. Defaults to all kinds, so the bus
    /// keeps delivering everything to handlers that don't opt into a narrower
    /// set. Override to let the bus skip materializing unwanted events.
    fn interest(&self) -> EventInterest {
        EventInterest::ALL
    }
}

/// Event handler that forwards events to an async channel.
///
/// # Example
/// ```ignore
/// let (handler, rx) = ChannelEventHandler::new();
/// client.register_handler(handler);
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

/// Immutable snapshot of the registered handlers. `dispatch` clones only the
/// outer `Arc` (one refcount bump, no `Vec` allocation), then drops the lock and
/// iterates the snapshot. Handler interest is re-evaluated per dispatch so a
/// handler whose `interest()` widens at runtime still receives the new kinds.
#[derive(Default)]
struct HandlerSnapshot {
    handlers: Vec<Arc<dyn EventHandler>>,
}

#[derive(Default, Clone)]
pub struct CoreEventBus {
    // Copy-on-write: the snapshot is only swapped (under the lock) when a
    // handler is added, which happens at startup. `dispatch` takes a cheap
    // outer-Arc clone and then drops the lock, so a concurrent `add_handler`
    // can never invalidate a snapshot a dispatch is iterating.
    handlers: Arc<RwLock<Arc<HandlerSnapshot>>>,
}

impl CoreEventBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn snapshot(&self) -> Arc<HandlerSnapshot> {
        self.handlers
            .read()
            .expect("RwLock should not be poisoned")
            .clone()
    }

    pub fn add_handler(&self, handler: Arc<dyn EventHandler>) {
        let mut guard = self
            .handlers
            .write()
            .expect("RwLock should not be poisoned");
        let current = &**guard;
        let mut handlers = Vec::with_capacity(current.handlers.len() + 1);
        handlers.extend(current.handlers.iter().cloned());
        handlers.push(handler);
        *guard = Arc::new(HandlerSnapshot { handlers });
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
        self.snapshot()
            .handlers
            .iter()
            .any(|h| h.interest().wants(kind))
    }

    pub fn dispatch(&self, event: Event) {
        let snapshot = self.snapshot();
        // Skip materializing the event (Arc) when no handler wants this kind. The
        // interest is re-evaluated here (not read from a cached aggregate) so a
        // handler whose interest() widens at runtime is never short-circuited out.
        let kind = event.kind();
        if !snapshot.handlers.iter().any(|h| h.interest().wants(kind)) {
            return;
        }
        let event = Arc::new(event);
        for handler in &snapshot.handlers {
            if handler.interest().wants(kind) {
                handler.handle_event(Arc::clone(&event));
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct DeviceNotificationInfo {
    /// Device ID (extracted from JID)
    pub device_id: u32,
    /// Optional key index
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_index: Option<u32>,
}

/// Device list update notification.
/// Emitted when a user's device list changes (device added/removed/updated).
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub enum Event {
    Connected(Connected),
    Disconnected(Disconnected),
    PairSuccess(PairSuccess),
    PairError(PairError),
    LoggedOut(LoggedOut),
    PairingQrCode {
        code: String,
        timeout: std::time::Duration,
    },
    /// Generated pair code for phone number linking.
    /// User should enter this code on their phone in WhatsApp > Linked Devices.
    PairingCode {
        /// The 8-character pairing code to display.
        code: String,
        /// Approximate validity duration (~180 seconds).
        timeout: std::time::Duration,
    },
    QrScannedWithoutMultidevice(QrScannedWithoutMultidevice),
    ClientOutdated(ClientOutdated),

    Message(Arc<wa::Message>, Arc<MessageInfo>),
    Receipt(Receipt),
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
    /// Gated by `Client::set_raw_node_forwarding(true)` to avoid overhead when unused.
    #[serde(skip)]
    RawNode(Arc<OwnedNodeRef>),

    /// Server-pushed MEX (GraphQL) update. Routed by the textual `op_name`,
    /// which is stable across WA Web bundle releases.
    MexNotification(MexNotification),
}

/// `payload` shape depends on `op_name`. `offline` mirrors the raw string
/// the server sets when replaying backlog (often a timestamp); presence
/// alone signals backlog vs live.
#[derive(Debug, Clone, Serialize)]
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
            Event::PairingQrCode { .. } => EventKind::PairingQrCode,
            Event::PairingCode { .. } => EventKind::PairingCode,
            Event::QrScannedWithoutMultidevice(_) => EventKind::QrScannedWithoutMultidevice,
            Event::ClientOutdated(_) => EventKind::ClientOutdated,
            Event::Message(_, _) => EventKind::Message,
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
        }
    }

    pub fn as_message(&self) -> Option<(&Arc<wa::Message>, &MessageInfo)> {
        if let Event::Message(msg, info) = self {
            Some((msg, &**info))
        } else {
            None
        }
    }

    pub fn message_text(&self) -> Option<&str> {
        let (msg, _) = self.as_message()?;
        msg.conversation.as_deref()
    }
}

/// A newsletter live update notification, typically containing updated
/// reaction counts for one or more messages.
#[derive(Debug, Clone, Serialize)]
pub struct NewsletterLiveUpdate {
    /// The newsletter channel this update belongs to.
    pub newsletter_jid: Jid,
    pub messages: Vec<NewsletterLiveUpdateMessage>,
}

/// A single message entry in a newsletter live update.
#[derive(Debug, Clone, Serialize)]
pub struct NewsletterLiveUpdateMessage {
    pub server_id: u64,
    pub reactions: Vec<NewsletterLiveUpdateReaction>,
}

/// A reaction count in a newsletter live update.
#[derive(Debug, Clone, Serialize)]
pub struct NewsletterLiveUpdateReaction {
    pub code: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairSuccess {
    pub id: Jid,
    pub lid: Jid,
    pub business_name: String,
    pub platform: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PairError {
    pub id: Jid,
    pub lid: Jid,
    pub business_name: String,
    pub platform: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct QrScannedWithoutMultidevice;

#[derive(Debug, Clone, Serialize)]
pub struct ClientOutdated;

#[derive(Debug, Clone, Serialize)]
pub struct Connected;

#[derive(Debug, Clone, Serialize)]
pub struct LoggedOut {
    pub on_connect: bool,
    pub reason: ConnectFailureReason,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamReplaced;

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

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct ConnectFailure {
    pub reason: ConnectFailureReason,
    pub message: String,
    pub raw: Option<Node>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StreamError {
    pub code: String,
    pub raw: Option<Node>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Disconnected;

#[derive(Debug, Clone, Serialize)]
pub struct OfflineSyncPreview {
    pub total: i32,
    pub app_data_changes: i32,
    pub messages: i32,
    pub notifications: i32,
    pub receipts: i32,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, crate::WireEnum)]
pub enum UnavailableType {
    #[wire_default]
    #[wire = "unknown"]
    Unknown,
    #[wire = "view_once"]
    ViewOnce,
}

#[derive(Debug, Clone, Serialize)]
pub struct UndecryptableMessage {
    pub info: Arc<MessageInfo>,
    pub is_unavailable: bool,
    pub unavailable_type: UnavailableType,
    pub decrypt_fail_mode: DecryptFailMode,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct ChatPresenceUpdate {
    pub source: crate::types::message::MessageSource,
    pub state: ChatPresence,
    pub media: ChatPresenceMedia,
}

#[derive(Debug, Clone, Serialize)]
pub struct PresenceUpdate {
    /// The contact whose presence changed.
    pub from: Jid,
    pub unavailable: bool,
    pub last_seen: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct ContactSyncRequested {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<DateTime<Utc>>,
    pub timestamp: DateTime<Utc>,
}

/// Group update notification.
///
/// Emitted for each action in a `<notification type="w:gp2">` stanza.
/// A single notification may produce multiple `GroupUpdate` events (one per action).
#[derive(Debug, Clone, Serialize)]
pub struct GroupUpdate {
    /// The group this update applies to
    pub group_jid: Jid,
    /// The admin/user who triggered the change (`participant` attribute)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant: Option<Jid>,
    /// Phone number JID of the participant (for LID-addressed groups)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_pn: Option<Jid>,
    /// When the change occurred
    pub timestamp: DateTime<Utc>,
    /// Whether the group uses LID addressing mode
    pub is_lid_addressing_mode: bool,
    /// The specific action
    pub action: crate::stanza::groups::GroupNotificationAction,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContactUpdate {
    /// The chat/contact this sync action applies to.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::ContactAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PushNameUpdate {
    /// The contact who changed their push name.
    pub jid: Jid,
    pub message: Box<MessageInfo>,
    pub old_push_name: String,
    pub new_push_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PinUpdate {
    /// The chat being pinned or unpinned.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::PinAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MuteUpdate {
    /// The chat being muted or unmuted.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::MuteAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchiveUpdate {
    /// The chat being archived or unarchived.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::ArchiveChatAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
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

#[derive(Debug, Clone, Serialize)]
pub struct MarkChatAsReadUpdate {
    /// The chat being marked as read or unread.
    pub jid: Jid,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::MarkChatAsReadAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct UserStatusMuteUpdate {
    /// The entity whose status was (un)muted.
    pub jid: Jid,
    /// `true` = status muted, `false` = unmuted.
    pub muted: bool,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::UserStatusMuteAction>,
    pub from_full_sync: bool,
}

#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct LabelEditUpdate {
    /// The label identifier (the index key, not a JID).
    pub label_id: String,
    pub timestamp: DateTime<Utc>,
    pub action: Box<wa::sync_action_value::LabelEditAction>,
    pub from_full_sync: bool,
}

/// A label was associated with or removed from a chat on a linked device.
/// `action.labeled == Some(true)` means the label was added to the chat.
#[derive(Debug, Clone, Serialize)]
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
mod tests {
    use super::*;
    use prost::Message;
    use waproto::whatsapp as wa;

    /// Build a HistorySync proto with conversations and encode it.
    fn make_history_sync_bytes(conversations: Vec<wa::Conversation>) -> Vec<u8> {
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations,
            ..Default::default()
        };
        hs.encode_to_vec()
    }

    #[test]
    fn lazy_history_sync_get_decodes() {
        let bytes = make_history_sync_bytes(vec![wa::Conversation {
            id: "chat@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 0, None, None);

        let hs = lazy.get().expect("should decode");
        assert_eq!(hs.conversations.len(), 1);
        assert_eq!(hs.conversations[0].id, "chat@s.whatsapp.net");
    }

    #[test]
    fn lazy_history_sync_caches_decode() {
        let bytes = make_history_sync_bytes(vec![wa::Conversation {
            id: "test@g.us".to_string(),
            ..Default::default()
        }]);
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 0, None, None);

        let first = lazy.get().expect("first decode");
        let second = lazy.get().expect("second decode");
        // Same reference — OnceLock cached it
        assert!(std::ptr::eq(first, second));
    }

    #[test]
    fn lazy_history_sync_cheap_metadata() {
        let bytes = make_history_sync_bytes(vec![]);
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 3, Some(2), Some(50));

        assert_eq!(lazy.sync_type(), 3);
        assert_eq!(lazy.chunk_order(), Some(2));
        assert_eq!(lazy.progress(), Some(50));
    }

    #[test]
    fn lazy_history_sync_peer_data_request_session_id() {
        let bytes = make_history_sync_bytes(vec![]);

        let unset = LazyHistorySync::new(Bytes::from(bytes.clone()), 0, None, None);
        assert_eq!(unset.peer_data_request_session_id(), None);

        let set = LazyHistorySync::new(Bytes::from(bytes), 0, None, None)
            .with_peer_data_request_session_id(Some("session-123".to_string()));
        assert_eq!(set.peer_data_request_session_id(), Some("session-123"));

        // Round-trip through Clone
        let cloned = set.clone();
        assert_eq!(cloned.peer_data_request_session_id(), Some("session-123"));
    }

    #[test]
    fn lazy_history_sync_raw_bytes() {
        let bytes = make_history_sync_bytes(vec![wa::Conversation {
            id: "raw@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);
        let raw = bytes.clone();
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 0, None, None);

        assert_eq!(lazy.raw_bytes().as_deref(), Some(&raw[..]));

        // Consumer can partial-decode from raw_bytes
        let raw_bytes = lazy.raw_bytes().expect("raw still present before get()");
        let decoded = wa::HistorySync::decode(&raw_bytes[..]).expect("should decode");
        assert_eq!(decoded.conversations[0].id, "raw@s.whatsapp.net");
    }

    #[test]
    fn lazy_history_sync_get_frees_raw_bytes() {
        let bytes = make_history_sync_bytes(vec![wa::Conversation {
            id: "freed@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);
        let raw_len = bytes.len();
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 7, Some(1), Some(42));

        assert!(lazy.raw_bytes().is_some(), "raw present before get()");
        assert_eq!(
            lazy.get().expect("decodes").conversations[0].id,
            "freed@s.whatsapp.net"
        );

        // The decompressed bytes are released once the owned proto exists.
        assert!(
            lazy.raw_bytes().is_none(),
            "raw freed after a successful get()"
        );
        // Metadata survives the free.
        assert_eq!(lazy.raw_size(), raw_len);
        assert_eq!(lazy.sync_type(), 7);
        assert_eq!(lazy.chunk_order(), Some(1));
        assert_eq!(lazy.progress(), Some(42));
        // get() still returns the cached proto after raw is gone.
        assert_eq!(
            lazy.get().expect("cached").conversations[0].id,
            "freed@s.whatsapp.net"
        );
    }

    #[test]
    fn lazy_history_sync_keeps_raw_when_decode_fails() {
        // A corrupt blob fails to decode; raw is kept so partial decode / retry
        // remains possible (only a successful decode frees it).
        let lazy = LazyHistorySync::new(Bytes::from_static(&[0xFF, 0xFF, 0xFF]), 0, None, None);
        assert!(lazy.get().is_none());
        assert!(
            lazy.raw_bytes().is_some(),
            "raw must survive a failed decode"
        );
    }

    #[test]
    fn lazy_history_sync_clone_after_get_stays_decodable() {
        let bytes = make_history_sync_bytes(vec![wa::Conversation {
            id: "cloned@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 0, None, None);

        // Decode on the original, which frees its raw bytes.
        assert_eq!(
            lazy.get().expect("decodes").conversations[0].id,
            "cloned@s.whatsapp.net"
        );
        assert!(lazy.raw_bytes().is_none());

        // A clone taken AFTER the original decoded must still yield the history
        // (the decoded proto is carried over since the raw bytes are gone).
        let cloned = lazy.clone();
        assert_eq!(
            cloned.get().expect("clone still decodes").conversations[0].id,
            "cloned@s.whatsapp.net"
        );
    }

    #[test]
    fn lazy_history_sync_clone_before_get_is_lazy() {
        let bytes = make_history_sync_bytes(vec![wa::Conversation {
            id: "lazyclone@s.whatsapp.net".to_string(),
            ..Default::default()
        }]);
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 0, None, None);

        // Cloning before any decode carries the raw bytes (cheap, still lazy).
        let cloned = lazy.clone();
        assert!(cloned.raw_bytes().is_some(), "lazy clone carries raw");
        assert_eq!(
            cloned.get().expect("decodes").conversations[0].id,
            "lazyclone@s.whatsapp.net"
        );
    }

    #[test]
    fn lazy_history_sync_empty_bytes_decodes_default() {
        // Empty protobuf bytes are valid — decode to default HistorySync
        let lazy = LazyHistorySync::new(Bytes::new(), 0, None, None);
        let hs = lazy.get().expect("empty bytes decode to default");
        assert!(hs.conversations.is_empty());
    }

    #[test]
    fn lazy_history_sync_corrupt_bytes_returns_none() {
        let lazy = LazyHistorySync::new(Bytes::from_static(&[0xFF, 0xFF, 0xFF]), 0, None, None);
        assert!(lazy.get().is_none());
    }

    #[test]
    fn lazy_history_sync_preserves_messages() {
        let conv = wa::Conversation {
            id: "chat@s.whatsapp.net".to_string(),
            messages: vec![wa::HistorySyncMsg {
                message: Some(wa::WebMessageInfo {
                    key: wa::MessageKey {
                        id: Some("msg-0".to_string()),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                msg_order_id: Some(0),
            }],
            ..Default::default()
        };
        let bytes = make_history_sync_bytes(vec![conv]);
        let lazy = LazyHistorySync::new(Bytes::from(bytes), 0, None, None);

        let hs = lazy.get().expect("should decode");
        assert_eq!(hs.conversations[0].messages.len(), 1);
        assert_eq!(
            hs.conversations[0].messages[0]
                .message
                .as_ref()
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
            interest: EventInterest::of(&[EventKind::Message]),
        });
        let all = Arc::new(Recorder {
            kinds: Mutex::new(Vec::new()),
            interest: EventInterest::ALL,
        });
        bus.add_handler(only_msg.clone());
        bus.add_handler(all.clone());

        bus.dispatch(Event::Connected(Connected));

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
                EventInterest::of(&[EventKind::Message])
            }
        }
        let bus2 = CoreEventBus::new();
        bus2.add_handler(Arc::new(Counter));
        bus2.dispatch(Event::Connected(Connected));
        assert_eq!(CALLS.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn dispatch_respects_dynamically_widened_interest() {
        use std::sync::Mutex;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // A handler whose interest() widens after registration. dispatch must
        // re-read interest each time (never a stale cached aggregate), so the
        // newly-wanted kind is delivered.
        struct Dynamic {
            interest: Mutex<EventInterest>,
            hits: AtomicUsize,
        }
        impl EventHandler for Dynamic {
            fn handle_event(&self, _: Arc<Event>) {
                self.hits.fetch_add(1, Ordering::SeqCst);
            }
            fn interest(&self) -> EventInterest {
                *self.interest.lock().unwrap()
            }
        }

        let bus = CoreEventBus::new();
        let h = Arc::new(Dynamic {
            interest: Mutex::new(EventInterest::of(&[EventKind::Message])),
            hits: AtomicUsize::new(0),
        });
        bus.add_handler(h.clone());

        // Not yet interested in Connected: dropped before materialization.
        bus.dispatch(Event::Connected(Connected));
        assert_eq!(h.hits.load(Ordering::SeqCst), 0);
        assert!(!bus.has_handler_for(EventKind::Connected));

        // Widen interest at runtime.
        *h.interest.lock().unwrap() = EventInterest::ALL;
        assert!(bus.has_handler_for(EventKind::Connected));
        bus.dispatch(Event::Connected(Connected));
        assert_eq!(
            h.hits.load(Ordering::SeqCst),
            1,
            "a handler whose interest widened at runtime must receive the newly-wanted kind"
        );
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
        assert!(!bus.has_handler_for(EventKind::Message));
        assert!(!bus.has_handler_for(EventKind::Receipt));

        bus.add_handler(Arc::new(Narrow(EventInterest::of(&[EventKind::Message]))));
        assert!(bus.has_handlers());
        assert!(bus.has_handler_for(EventKind::Message));
        assert!(!bus.has_handler_for(EventKind::Receipt));

        // has_handler_for is true once any registered handler wants the kind.
        bus.add_handler(Arc::new(Narrow(EventInterest::of(&[EventKind::Receipt]))));
        assert!(bus.has_handler_for(EventKind::Message));
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
        for id in 0..5u32 {
            bus.add_handler(Arc::new(Tagged {
                id,
                log: log.clone(),
            }));
        }
        bus.dispatch(Event::Connected(Connected));
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
                        .add_handler(Arc::new(Late(self.invocations.clone())));
                }
            }
        }

        let bus = CoreEventBus::new();
        let invocations = Arc::new(AtomicUsize::new(0));
        bus.add_handler(Arc::new(AddsDuringDispatch {
            bus: bus.clone(),
            invocations: invocations.clone(),
            added: Mutex::new(false),
        }));

        // First dispatch: only the original handler runs, even though it adds a
        // second handler mid-flight.
        bus.dispatch(Event::Connected(Connected));
        assert_eq!(invocations.load(Ordering::SeqCst), 1);
        assert_eq!(bus.snapshot().handlers.len(), 2);

        // Second dispatch sees both handlers (original adds nothing new now).
        bus.dispatch(Event::Connected(Connected));
        assert_eq!(invocations.load(Ordering::SeqCst), 3);
    }
}
