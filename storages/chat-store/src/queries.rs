//! Read API. Every query runs on the shared pool's blocking thread; results
//! come back as plain owned values (the SQLite page cache is the cache — no
//! row caching on this side).

use std::str::FromStr;

use chrono::{DateTime, Utc};
use diesel::prelude::*;
use log::warn;
use wacore_binary::Jid;

use crate::error::{Result, db_err};
use crate::schema;
use crate::store::ChatStore;
use crate::types::{
    ChatEntry, ContactEntry, MediaRef, MessageCursor, MessageKind, MessageStatus, ReactionEntry,
    ReceiptEntry, StoredMessage,
};

fn ms_to_utc(ms: i64) -> Option<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp_millis(ms)
}

type ContactRow = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

type MediaRefRow = (Vec<u8>, String, Option<String>, Option<i64>, i64);

/// Parse a stored JID column; empty (own history messages with no participant)
/// maps to the default JID rather than an error.
fn parse_jid(raw: &str) -> Jid {
    if raw.is_empty() {
        return Jid::default();
    }
    Jid::from_str(raw).unwrap_or_else(|_| {
        warn!("chat-store: unparseable JID in database: {raw}");
        Jid::default()
    })
}

#[derive(Queryable)]
struct ChatRow {
    #[allow(dead_code)]
    device_id: i32,
    jid: String,
    name: Option<String>,
    last_message_ts: i64,
    last_message_preview: Option<String>,
    last_message_kind: Option<String>,
    unread_count: i32,
    pinned_at: Option<i64>,
    muted_until: Option<i64>,
    archived: bool,
    ephemeral_expiration: Option<i32>,
    #[allow(dead_code)]
    read_boundary_ms: i64,
    #[allow(dead_code)]
    read_boundary_ids: Option<String>,
}

impl From<ChatRow> for ChatEntry {
    fn from(row: ChatRow) -> Self {
        ChatEntry {
            jid: parse_jid(&row.jid),
            name: row.name,
            last_message_at: (row.last_message_ts > 0)
                .then(|| ms_to_utc(row.last_message_ts))
                .flatten(),
            last_message_preview: row.last_message_preview,
            last_message_kind: row.last_message_kind.map(MessageKind::from_db),
            unread_count: row.unread_count,
            pinned_at: row.pinned_at.and_then(ms_to_utc),
            // The writer stores i64::MAX for "muted forever"; that value is
            // outside DateTime's range, and silently mapping it to None would
            // make a forever-muted chat read as unmuted.
            muted_until: row.muted_until.and_then(|ms| {
                if ms == i64::MAX {
                    Some(DateTime::<Utc>::MAX_UTC)
                } else {
                    ms_to_utc(ms)
                }
            }),
            archived: row.archived,
            ephemeral_expiration: row.ephemeral_expiration.map(|e| e as u32),
        }
    }
}

#[derive(Queryable)]
struct MessageRow {
    #[allow(dead_code)]
    device_id: i32,
    chat_jid: String,
    msg_id: String,
    sender_jid: String,
    from_me: bool,
    timestamp_ms: i64,
    kind: String,
    text_content: Option<String>,
    proto: Option<Vec<u8>>,
    status: i32,
    starred: bool,
    edited_at_ms: Option<i64>,
    revoked: bool,
}

impl From<MessageRow> for StoredMessage {
    fn from(row: MessageRow) -> Self {
        let message = row.proto.as_deref().and_then(|bytes| {
            match waproto::codec::message_decode(bytes) {
                Ok(msg) => Some(Box::new(msg)),
                Err(e) => {
                    // Denormalized columns still render; only the proto is lost.
                    warn!(
                        "chat-store: stored proto for {} undecodable: {e}",
                        row.msg_id
                    );
                    None
                }
            }
        });
        StoredMessage {
            chat_jid: parse_jid(&row.chat_jid),
            id: row.msg_id,
            sender_jid: parse_jid(&row.sender_jid),
            from_me: row.from_me,
            timestamp: ms_to_utc(row.timestamp_ms).unwrap_or_default(),
            kind: MessageKind::from_db(row.kind),
            text: row.text_content,
            message,
            status: MessageStatus::from_raw(row.status),
            starred: row.starred,
            edited_at: row.edited_at_ms.and_then(ms_to_utc),
            revoked: row.revoked,
        }
    }
}

impl ChatStore {
    /// Chat list in a sensible default order (pinned first, then latest
    /// activity). Purely a default: every ordering input (`pinned_at`,
    /// `last_message_at`, `archived`, ...) is on [`ChatEntry`], so a frontend
    /// with different needs re-sorts freely.
    pub async fn chats(&self, include_archived: bool, limit: i64) -> Result<Vec<ChatEntry>> {
        use schema::chats::dsl;
        // A negative LIMIT means "unbounded" to SQLite; never let that happen.
        let limit = limit.max(0);
        let device_id = self.device_id();
        let rows: Vec<ChatRow> = self
            .db()
            .run(move |conn| {
                let mut query = dsl::chats.filter(dsl::device_id.eq(device_id)).into_boxed();
                if !include_archived {
                    query = query.filter(dsl::archived.eq(false));
                }
                query
                    .order((
                        diesel::dsl::sql::<diesel::sql_types::Bool>("pinned_at IS NULL"),
                        dsl::pinned_at.desc(),
                        dsl::last_message_ts.desc(),
                    ))
                    .limit(limit)
                    .load(conn)
                    .map_err(db_err)
            })
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// One page of a chat's messages, newest first. Pass the cursor of the
    /// oldest message you already have to get the page before it.
    ///
    /// A 1:1 chat may be addressed by either of the peer's identities (phone
    /// number or LID); the query resolves the alias, so both find the thread.
    pub async fn messages(
        &self,
        chat: &Jid,
        before: Option<MessageCursor>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        use schema::messages::dsl;
        let limit = limit.max(0);
        let device_id = self.device_id();
        let chat = chat.to_string();
        let rows: Vec<MessageRow> = self
            .db()
            .run(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &chat).map_err(db_err)?;
                let mut query = dsl::messages
                    .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq_any(keys)))
                    .into_boxed();
                if let Some(cursor) = &before {
                    query = query.filter(
                        dsl::timestamp_ms
                            .lt(cursor.timestamp_ms)
                            .or(dsl::timestamp_ms
                                .eq(cursor.timestamp_ms)
                                .and(dsl::msg_id.lt(&cursor.msg_id))),
                    );
                }
                query
                    .order((dsl::timestamp_ms.desc(), dsl::msg_id.desc()))
                    .limit(limit)
                    .load(conn)
                    .map_err(db_err)
            })
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    pub async fn message(&self, chat: &Jid, msg_id: &str) -> Result<Option<StoredMessage>> {
        use schema::messages::dsl;
        let device_id = self.device_id();
        let chat = chat.to_string();
        let msg_id = msg_id.to_owned();
        let row: Option<MessageRow> = self
            .db()
            .run(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &chat).map_err(db_err)?;
                dsl::messages
                    .filter(
                        dsl::device_id
                            .eq(device_id)
                            .and(dsl::chat_jid.eq_any(keys))
                            .and(dsl::msg_id.eq(&msg_id)),
                    )
                    .first(conn)
                    .optional()
                    .map_err(db_err)
            })
            .await?;
        Ok(row.map(Into::into))
    }

    pub async fn reactions(&self, chat: &Jid, msg_id: &str) -> Result<Vec<ReactionEntry>> {
        use schema::reactions::dsl;
        let device_id = self.device_id();
        let chat = chat.to_string();
        let msg_id = msg_id.to_owned();
        let rows: Vec<(String, String, i64)> = self
            .db()
            .run(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &chat).map_err(db_err)?;
                dsl::reactions
                    .filter(
                        dsl::device_id
                            .eq(device_id)
                            .and(dsl::chat_jid.eq_any(keys))
                            .and(dsl::msg_id.eq(&msg_id)),
                    )
                    .select((dsl::sender_jid, dsl::emoji, dsl::ts_ms))
                    .order(dsl::ts_ms.asc())
                    .load(conn)
                    .map_err(db_err)
            })
            .await?;
        Ok(rows
            .into_iter()
            .map(|(sender, emoji, ts)| ReactionEntry {
                sender_jid: parse_jid(&sender),
                emoji,
                timestamp: ms_to_utc(ts).unwrap_or_default(),
            })
            .collect())
    }

    /// Per-user receipts of one message (group "delivered to"/"read by").
    pub async fn receipts(&self, chat: &Jid, msg_id: &str) -> Result<Vec<ReceiptEntry>> {
        use schema::message_receipts::dsl;
        let device_id = self.device_id();
        let chat = chat.to_string();
        let msg_id = msg_id.to_owned();
        let rows: Vec<(String, i32, i64)> = self
            .db()
            .run(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &chat).map_err(db_err)?;
                dsl::message_receipts
                    .filter(
                        dsl::device_id
                            .eq(device_id)
                            .and(dsl::chat_jid.eq_any(keys))
                            .and(dsl::msg_id.eq(&msg_id)),
                    )
                    .select((dsl::user_jid, dsl::receipt_type, dsl::ts_ms))
                    .order(dsl::ts_ms.asc())
                    .load(conn)
                    .map_err(db_err)
            })
            .await?;
        Ok(rows
            .into_iter()
            .map(|(user, status, ts)| ReceiptEntry {
                user_jid: parse_jid(&user),
                status: MessageStatus::from_raw(status),
                timestamp: ms_to_utc(ts).unwrap_or_default(),
            })
            .collect())
    }

    pub async fn contact(&self, jid: &Jid) -> Result<Option<ContactEntry>> {
        use schema::contacts::dsl;
        let device_id = self.device_id();
        // Bare key, matching how the writers file contacts: a caller holding a
        // message's `sender` has the device on it.
        let jid_str = jid.to_non_ad_string();
        let row: Option<ContactRow> = self
            .db()
            .run(move |conn| {
                dsl::contacts
                    .filter(dsl::device_id.eq(device_id).and(dsl::jid.eq(&jid_str)))
                    .select((
                        dsl::jid,
                        dsl::push_name,
                        dsl::full_name,
                        dsl::first_name,
                        dsl::business_name,
                    ))
                    .first(conn)
                    .optional()
                    .map_err(db_err)
            })
            .await?;
        Ok(row.map(
            |(jid, push_name, full_name, first_name, business_name)| ContactEntry {
                jid: parse_jid(&jid),
                push_name,
                full_name,
                first_name,
                business_name,
            },
        ))
    }

    /// Sum of positive unread counters (ignores "marked unread" sentinels).
    pub async fn unread_total(&self) -> Result<i64> {
        use schema::chats::dsl;
        let device_id = self.device_id();
        let total: Option<i64> = self
            .db()
            .run(move |conn| {
                dsl::chats
                    .filter(dsl::device_id.eq(device_id).and(dsl::unread_count.gt(0)))
                    .select(diesel::dsl::sum(dsl::unread_count))
                    .first(conn)
                    .map_err(db_err)
            })
            .await?;
        Ok(total.unwrap_or(0))
    }

    /// Record where a downloaded media blob lives locally, keyed by content
    /// hash so identical files are stored once.
    pub async fn put_media_ref(
        &self,
        file_sha256: Vec<u8>,
        file_path: String,
        mime_type: Option<String>,
        size_bytes: Option<i64>,
    ) -> Result<()> {
        use schema::media_refs::dsl;
        let device_id = self.device_id();
        let now_ms = wacore::time::now_utc().timestamp_millis();
        self.db()
            .run(move |conn| {
                diesel::insert_into(dsl::media_refs)
                    .values((
                        dsl::device_id.eq(device_id),
                        dsl::file_sha256.eq(&file_sha256),
                        dsl::file_path.eq(&file_path),
                        dsl::mime_type.eq(&mime_type),
                        dsl::size_bytes.eq(size_bytes),
                        dsl::downloaded_at_ms.eq(now_ms),
                    ))
                    .on_conflict((dsl::device_id, dsl::file_sha256))
                    .do_update()
                    .set((
                        dsl::file_path.eq(&file_path),
                        dsl::mime_type.eq(&mime_type),
                        dsl::size_bytes.eq(size_bytes),
                        dsl::downloaded_at_ms.eq(now_ms),
                    ))
                    .execute(conn)
                    .map(|_| ())
                    .map_err(db_err)
            })
            .await?;
        Ok(())
    }

    pub async fn media_ref(&self, file_sha256: &[u8]) -> Result<Option<MediaRef>> {
        use schema::media_refs::dsl;
        let device_id = self.device_id();
        let sha = file_sha256.to_vec();
        let row: Option<MediaRefRow> = self
            .db()
            .run(move |conn| {
                dsl::media_refs
                    .filter(dsl::device_id.eq(device_id).and(dsl::file_sha256.eq(&sha)))
                    .select((
                        dsl::file_sha256,
                        dsl::file_path,
                        dsl::mime_type,
                        dsl::size_bytes,
                        dsl::downloaded_at_ms,
                    ))
                    .first(conn)
                    .optional()
                    .map_err(db_err)
            })
            .await?;
        Ok(row.map(
            |(file_sha256, file_path, mime_type, size_bytes, downloaded_at_ms)| MediaRef {
                file_sha256,
                file_path,
                mime_type,
                size_bytes,
                downloaded_at: ms_to_utc(downloaded_at_ms).unwrap_or_default(),
            },
        ))
    }
}
