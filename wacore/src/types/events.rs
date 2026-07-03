use crate::stanza::BusinessSubscription;
use crate::types::call::{CallEndedElsewhere, IncomingCall, MissedCall};
use crate::types::message::MessageInfo;
use crate::types::presence::{ChatPresence, ChatPresenceMedia, ReceiptType};
use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
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
const _: () = assert!((EventKind::PairPasskeyError as u8) < EventKind::CAPACITY);

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
    /// Gated by `Client::set_raw_node_forwarding(true)` to avoid overhead when unused.
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
#[derive(Debug, Clone, Serialize)]
pub struct PairPasskeyRequest {
    /// Verbatim `PublicKeyCredentialRequestOptions` JSON from the server. Pass it
    /// straight to a WebAuthn `get` (e.g. Android Credential Manager), or parse it
    /// with `whatsapp_rust::passkey::parse_request_options`.
    pub request_options_json: String,
}

/// Payload for [`Event::PairPasskeyConfirmation`].
#[derive(Debug, Clone, Serialize)]
pub struct PairPasskeyConfirmation {
    pub code: String,
    pub skip_handoff_ux: bool,
}

/// Payload for [`Event::PairPasskeyError`].
#[derive(Debug, Clone, Serialize)]
pub struct PairPasskeyError {
    pub error: String,
    pub continuation: bool,
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
pub struct Disconnected {
    /// Why the transport ended — lets consumers tell a routine server stream
    /// recycle (`reason.is_clean_shutdown()`) from a genuine transport failure
    /// without parsing logs.
    pub reason: crate::net::DisconnectReason,
}

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
    use buffa::Message;
    use waproto::whatsapp as wa;

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
