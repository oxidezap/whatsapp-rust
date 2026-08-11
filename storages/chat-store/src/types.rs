use chrono::{DateTime, Utc};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Content class of a stored message: one label per renderable bubble type,
/// shared by every frontend (desktop, TUI, mobile) so none of them hard-codes
/// label strings. Stored as text in the database; [`Other`](Self::Other)
/// round-trips labels written by a newer crate version.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MessageKind {
    Text,
    Image,
    Video,
    /// Round video note ("ptv").
    VideoNote,
    Audio,
    /// Push-to-talk voice note ("ptt").
    VoiceNote,
    Sticker,
    Document,
    Contact,
    Location,
    Poll,
    Event,
    GroupInvite,
    /// Hydrated business template (WABA notification).
    Template,
    /// Reply to a template button.
    TemplateReply,
    Buttons,
    ButtonsResponse,
    List,
    ListResponse,
    Interactive,
    InteractiveResponse,
    /// Placeholder for a message that could not be decrypted **yet** — a
    /// retry or a PDO placeholder-resend may still fill it in.
    Undecryptable,
    /// A view-once photo, video or voice note the server fanned out as
    /// `<unavailable>`. The phone never shares that content with a companion,
    /// so unlike [`Undecryptable`](Self::Undecryptable) this will not resolve —
    /// it is the one-time chip WA Web renders ("open on your phone"), not a
    /// "waiting for this message" placeholder.
    ViewOnce,
    /// A hosted-content fanout. Permanently unavailable to a companion, like
    /// [`ViewOnce`](Self::ViewOnce).
    Hosted,
    /// A bot-message fanout. Permanently unavailable to a companion, like
    /// [`ViewOnce`](Self::ViewOnce).
    Bot,
    /// Real content this crate version doesn't classify.
    Unknown,
    /// A database label written by a newer crate version.
    Other(String),
}

impl MessageKind {
    /// The database label. Stable: these are on-disk values.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Image => "image",
            Self::Video => "video",
            Self::VideoNote => "ptv",
            Self::Audio => "audio",
            Self::VoiceNote => "ptt",
            Self::Sticker => "sticker",
            Self::Document => "document",
            Self::Contact => "contact",
            Self::Location => "location",
            Self::Poll => "poll",
            Self::Event => "event",
            Self::GroupInvite => "group_invite",
            Self::Template => "template",
            Self::TemplateReply => "template_reply",
            Self::Buttons => "buttons",
            Self::ButtonsResponse => "buttons_response",
            Self::List => "list",
            Self::ListResponse => "list_response",
            Self::Interactive => "interactive",
            Self::InteractiveResponse => "interactive_response",
            Self::Undecryptable => "undecryptable",
            Self::ViewOnce => "view_once",
            Self::Hosted => "hosted",
            Self::Bot => "bot",
            Self::Unknown => "unknown",
            Self::Other(label) => label,
        }
    }

    pub(crate) fn from_db(label: String) -> Self {
        match label.as_str() {
            "text" => Self::Text,
            "image" => Self::Image,
            "video" => Self::Video,
            "ptv" => Self::VideoNote,
            "audio" => Self::Audio,
            "ptt" => Self::VoiceNote,
            "sticker" => Self::Sticker,
            "document" => Self::Document,
            "contact" => Self::Contact,
            "location" => Self::Location,
            "poll" => Self::Poll,
            "event" => Self::Event,
            "group_invite" => Self::GroupInvite,
            "template" => Self::Template,
            "template_reply" => Self::TemplateReply,
            "buttons" => Self::Buttons,
            "buttons_response" => Self::ButtonsResponse,
            "list" => Self::List,
            "list_response" => Self::ListResponse,
            "interactive" => Self::Interactive,
            "interactive_response" => Self::InteractiveResponse,
            "undecryptable" => Self::Undecryptable,
            "view_once" => Self::ViewOnce,
            "hosted" => Self::Hosted,
            "bot" => Self::Bot,
            "unknown" => Self::Unknown,
            _ => Self::Other(label),
        }
    }
}

/// Delivery state of a stored message, on the same scale WhatsApp itself uses
/// (`WebMessageInfo.Status`), so history-sync statuses map through unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(i32)]
pub enum MessageStatus {
    Error = 0,
    Pending = 1,
    ServerAck = 2,
    Delivered = 3,
    Read = 4,
    Played = 5,
}

impl MessageStatus {
    pub fn from_raw(raw: i32) -> Self {
        match raw {
            0 => Self::Error,
            2 => Self::ServerAck,
            3 => Self::Delivered,
            4 => Self::Read,
            5 => Self::Played,
            _ => Self::Pending,
        }
    }
}

/// One row of the chat list, ordered for display (pinned first, then most
/// recent activity).
#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub jid: Jid,
    pub name: Option<String>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub last_message_preview: Option<String>,
    /// Content class of the latest message, so a media preview can render as
    /// "\[photo\]"/"\[voice note\]" in whatever way (and language) the frontend
    /// chooses — the store never bakes in presentation strings.
    pub last_message_kind: Option<MessageKind>,
    /// `-1` means "manually marked unread" (WA Web convention).
    pub unread_count: i32,
    pub pinned_at: Option<DateTime<Utc>>,
    /// `Some(DateTime::MAX_UTC)` = muted forever (no expiry).
    pub muted_until: Option<DateTime<Utc>>,
    pub archived: bool,
    pub ephemeral_expiration: Option<u32>,
}

/// A stored message. `message` is the decoded proto when the row has one and
/// it decodes cleanly; the denormalized columns (`kind`, `text`) always work
/// even when it doesn't.
#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub chat_jid: Jid,
    pub id: String,
    pub sender_jid: Jid,
    pub from_me: bool,
    pub timestamp: DateTime<Utc>,
    pub kind: MessageKind,
    pub text: Option<String>,
    pub message: Option<Box<wa::Message>>,
    pub status: MessageStatus,
    pub starred: bool,
    pub edited_at: Option<DateTime<Utc>>,
    pub revoked: bool,
    /// Arrival order within this store, ascending. Opaque: compare it, don't
    /// interpret it. It exists because the server's `t` is whole seconds, so
    /// two messages exchanged in the same second carry the same `timestamp`
    /// and something has to break the tie — this is the order the socket
    /// delivered them in, which is the order both ends display.
    ///
    /// Comparable, not durable: a `VACUUM` preserves the relative order these
    /// values encode but may renumber the values themselves, and SQLite hands
    /// out the implicit rowid as `max(rowid) + 1`, so deleting the newest
    /// message gives its number to the next arrival and clearing a chat
    /// entirely restarts at 1. A `seq` (or a [`MessageCursor`]/[`ArrivalCursor`]
    /// built from one) is good for a live paging session and must not be
    /// persisted across restarts or compared against a remembered value — a new
    /// message can legitimately land below one.
    ///
    /// It is *store* arrival, not wire arrival. Inbound rows are inserted as
    /// the socket delivers them, but an outgoing row is inserted when the host
    /// calls `record_outgoing`, which happens after its send resolves — so a
    /// peer reply that is decrypted and materialized in that gap, and that
    /// lands on the same whole second, takes the lower `seq`. That needs a full
    /// round trip to complete inside one second while a local enqueue is still
    /// pending, and it is bounded to same-second pairs; the ordering it
    /// replaced was wrong for roughly three of every four such pairs, in a
    /// fixed direction.
    pub seq: i64,
}

/// Keyset-pagination cursor: pass the values of the oldest message you have to
/// fetch the page before it. Never an OFFSET — stable under concurrent inserts.
#[derive(Debug, Clone)]
pub struct MessageCursor {
    pub timestamp_ms: i64,
    /// [`StoredMessage::seq`] of the same message. Must match the sort's
    /// tiebreak exactly, or a page boundary that lands inside a same-second
    /// run would skip or repeat rows.
    pub seq: i64,
}

impl From<&StoredMessage> for MessageCursor {
    fn from(m: &StoredMessage) -> Self {
        Self {
            timestamp_ms: m.timestamp.timestamp_millis(),
            seq: m.seq,
        }
    }
}

/// Keyset-pagination cursor for the session-wide arrival feed: pass the
/// [`seq`](StoredMessage::seq) of the last message on the page you have to
/// fetch the page after it, which is the next batch of older arrivals.
///
/// Separate from [`MessageCursor`] because the two order by different keys — a
/// per-chat page sorts by `(timestamp_ms, seq)` and the arrival feed sorts by
/// `seq` alone, so a cursor from one cannot page the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrivalCursor {
    /// [`StoredMessage::seq`] of the last message on the previous page.
    pub seq: i64,
}

impl From<&StoredMessage> for ArrivalCursor {
    fn from(m: &StoredMessage) -> Self {
        Self { seq: m.seq }
    }
}

/// Keyset-pagination cursor for the chat list: pass the values of the last
/// chat you have to fetch the page after it.
///
/// The list is two ordered runs — pinned chats by pin time, then the rest by
/// activity — so the cursor records which run it sits in (`pinned_at`) as well
/// as where.
#[derive(Debug, Clone)]
pub struct ChatCursor {
    /// `Some` for a cursor inside the pinned run, `None` for the activity run.
    pub pinned_at_ms: Option<i64>,
    pub last_message_ts: i64,
    pub jid: String,
}

impl From<&ChatEntry> for ChatCursor {
    fn from(c: &ChatEntry) -> Self {
        Self {
            pinned_at_ms: c.pinned_at.map(|t| t.timestamp_millis()),
            last_message_ts: c.last_message_at.map_or(0, |t| t.timestamp_millis()),
            jid: c.jid.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReactionEntry {
    pub sender_jid: Jid,
    pub emoji: String,
    pub timestamp: DateTime<Utc>,
}

/// Per-user delivery/read state of one message (group "read by" lists).
#[derive(Debug, Clone)]
pub struct ReceiptEntry {
    pub user_jid: Jid,
    pub status: MessageStatus,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ContactEntry {
    pub jid: Jid,
    pub push_name: Option<String>,
    pub full_name: Option<String>,
    pub first_name: Option<String>,
    pub business_name: Option<String>,
}

impl ContactEntry {
    /// Best display name available, WA Web precedence: address book (full,
    /// then first name), then push name, then business name.
    pub fn display_name(&self) -> Option<&str> {
        self.full_name
            .as_deref()
            .or(self.first_name.as_deref())
            .or(self.push_name.as_deref())
            .or(self.business_name.as_deref())
    }
}

#[derive(Debug, Clone)]
pub struct MediaRef {
    pub file_sha256: Vec<u8>,
    pub file_path: String,
    pub mime_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub downloaded_at: DateTime<Utc>,
}

/// Invalidation signal emitted after each committed write batch. Consumers
/// re-run the queries backing their visible state; the store never pushes row
/// data (query + invalidation, not cache duplication).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreChange {
    /// Chat-list-level change: ordering, previews, unread counts, membership.
    Chats,
    /// The message set of one chat changed (insert/edit/revoke/reaction/status).
    Messages { chat: Jid },
    /// Contact naming changed.
    Contacts,
}
