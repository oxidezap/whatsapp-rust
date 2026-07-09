use chrono::{DateTime, Utc};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

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
    /// `-1` means "manually marked unread" (WA Web convention).
    pub unread_count: i32,
    pub pinned_at: Option<DateTime<Utc>>,
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
    pub kind: String,
    pub text: Option<String>,
    pub message: Option<Box<wa::Message>>,
    pub status: MessageStatus,
    pub starred: bool,
    pub edited_at: Option<DateTime<Utc>>,
    pub revoked: bool,
}

/// Keyset-pagination cursor: pass the values of the oldest message you have to
/// fetch the page before it. Never an OFFSET — stable under concurrent inserts.
#[derive(Debug, Clone)]
pub struct MessageCursor {
    pub timestamp_ms: i64,
    pub msg_id: String,
}

impl From<&StoredMessage> for MessageCursor {
    fn from(m: &StoredMessage) -> Self {
        Self {
            timestamp_ms: m.timestamp.timestamp_millis(),
            msg_id: m.id.clone(),
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
