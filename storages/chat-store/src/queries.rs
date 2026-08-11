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
    ArrivalCursor, ChatCursor, ChatEntry, ContactEntry, MediaRef, MessageCursor, MessageKind,
    MessageStatus, ReactionEntry, ReceiptEntry, StoredMessage,
};

fn ms_to_utc(ms: i64) -> Option<DateTime<Utc>> {
    DateTime::<Utc>::from_timestamp_millis(ms)
}

/// A wall-clock instant as the first whole millisecond at or after it.
///
/// `timestamp_millis` truncates, and stored timestamps are whole milliseconds,
/// so a bound landing inside a millisecond has to move to the next one for both
/// ends of a half-open window: a row at `.500` is neither `>= .500_5` nor
/// excluded by `< .500_5`, and truncation gets both backwards. `Utc::now()`
/// carries nanoseconds, so this is the common case for a caller passing "an
/// hour ago", not an exotic one.
fn ceil_to_ms(t: DateTime<Utc>) -> i64 {
    let ms = t.timestamp_millis();
    if t.timestamp_subsec_nanos().is_multiple_of(1_000_000) {
        ms
    } else {
        ms.saturating_add(1)
    }
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
pub(crate) struct MessageRow {
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
    pub(crate) rowid: i64,
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
            seq: row.rowid,
        }
    }
}

/// The session-wide arrival page, as a query. Split out so a test can pin its
/// plan: this read is only cheap while SQLite answers `ORDER BY rowid DESC` by
/// walking the table's own B-tree backwards, and nothing in the SQL says so.
fn arrival_page_query(
    device_id: i32,
    after: Option<ArrivalCursor>,
    since_ms: Option<i64>,
    until_ms: Option<i64>,
    limit: i64,
) -> schema::messages::BoxedQuery<'static, diesel::sqlite::Sqlite> {
    use diesel::sql_types::{Bool, Integer};
    use schema::messages::dsl;
    // The unary `+` keeps `device_id` off the index the planner would otherwise
    // reach for. `idx_messages_by_id` leads with `device_id`, so SQLite scores
    // it as the better entry point and then pays a temp B-tree to put the whole
    // device's messages back in rowid order — a full sort of the table on every
    // page, to return one page. Denied that index, it reads the table backwards
    // and stops at LIMIT, which is the plan this feed is designed around.
    let mut query = dsl::messages
        .filter(diesel::dsl::sql::<Bool>("+device_id = ").bind::<Integer, _>(device_id))
        .into_boxed();
    if let Some(cursor) = after {
        query = query.filter(dsl::rowid.lt(cursor.seq));
    }
    // Wall-clock bounds are predicates over the arrival scan, never the
    // ordering key: see `messages_by_arrival_in_range`.
    if let Some(since_ms) = since_ms {
        query = query.filter(dsl::timestamp_ms.ge(since_ms));
    }
    if let Some(until_ms) = until_ms {
        query = query.filter(dsl::timestamp_ms.lt(until_ms));
    }
    query.order(dsl::rowid.desc()).limit(limit)
}

impl ChatStore {
    /// Chat list in a sensible default order (pinned first, then latest
    /// activity). Purely a default: every ordering input (`pinned_at`,
    /// `last_message_at`, `archived`, ...) is on [`ChatEntry`], so a frontend
    /// with different needs re-sorts freely.
    ///
    /// Equivalent to [`chats_page`](Self::chats_page) with no cursor.
    pub async fn chats(&self, include_archived: bool, limit: i64) -> Result<Vec<ChatEntry>> {
        self.chats_page(include_archived, None, limit).await
    }

    /// One page of the chat list. Pass the cursor of the last chat you already
    /// have to get the page after it.
    ///
    /// The list is two ordered runs concatenated — pinned chats by pin time,
    /// then everything else by activity — because SQLite cannot serve the
    /// combined `(pinned_at IS NULL, pinned_at DESC, last_message_ts DESC)`
    /// sort from any column index, and paying a full scan plus a temp B-tree
    /// per call is what this shape avoids. Each run is a plain ordered range
    /// scan that stops at `limit`.
    pub async fn chats_page(
        &self,
        include_archived: bool,
        after: Option<ChatCursor>,
        limit: i64,
    ) -> Result<Vec<ChatEntry>> {
        use schema::chats::dsl;
        // A negative LIMIT means "unbounded" to SQLite; never let that happen.
        let limit = limit.max(0);
        let device_id = self.device_id();
        let rows: Vec<ChatRow> = self
            .db()
            .read(move |conn| {
                // A cursor in the activity run has already passed every pinned
                // chat, so that run is skipped entirely rather than re-read.
                let resume_pinned = match &after {
                    Some(cursor) => cursor.pinned_at_ms,
                    None => None,
                };
                let start_in_activity_run =
                    matches!(&after, Some(cursor) if cursor.pinned_at_ms.is_none());

                let mut rows: Vec<ChatRow> = Vec::new();
                if !start_in_activity_run {
                    let mut query = dsl::chats
                        .filter(dsl::device_id.eq(device_id))
                        .filter(dsl::pinned_at.is_not_null())
                        .into_boxed();
                    if !include_archived {
                        query = query.filter(dsl::archived.eq(false));
                    }
                    if let (Some(pinned_at), Some(cursor)) = (resume_pinned, &after) {
                        query = query.filter(
                            dsl::pinned_at
                                .lt(pinned_at)
                                .or(dsl::pinned_at.eq(pinned_at).and(
                                    dsl::last_message_ts.lt(cursor.last_message_ts).or(
                                        dsl::last_message_ts
                                            .eq(cursor.last_message_ts)
                                            .and(dsl::jid.lt(cursor.jid.clone())),
                                    ),
                                )),
                        );
                    }
                    // Activity still decides between equally-pinned chats —
                    // history-sync pin times are second-precision and collide,
                    // and the old combined sort ranked them this way too.
                    rows = query
                        .order((
                            dsl::pinned_at.desc(),
                            dsl::last_message_ts.desc(),
                            dsl::jid.desc(),
                        ))
                        .limit(limit)
                        .load(conn)
                        .map_err(db_err)?;
                }

                let remaining = limit - rows.len() as i64;
                if remaining > 0 {
                    let mut query = dsl::chats
                        .filter(dsl::device_id.eq(device_id))
                        .filter(dsl::pinned_at.is_null())
                        .into_boxed();
                    if !include_archived {
                        query = query.filter(dsl::archived.eq(false));
                    }
                    if start_in_activity_run && let Some(cursor) = &after {
                        query = query.filter(
                            dsl::last_message_ts.lt(cursor.last_message_ts).or(
                                dsl::last_message_ts
                                    .eq(cursor.last_message_ts)
                                    .and(dsl::jid.lt(cursor.jid.clone())),
                            ),
                        );
                    }
                    let tail: Vec<ChatRow> = query
                        .order((dsl::last_message_ts.desc(), dsl::jid.desc()))
                        .limit(remaining)
                        .load(conn)
                        .map_err(db_err)?;
                    rows.extend(tail);
                }
                Ok(rows)
            })
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// One chat by key, or `None` if the store has never seen it.
    ///
    /// A 1:1 chat may be addressed by either of the peer's identities (phone
    /// number or LID); both resolve to the row the thread is actually stored
    /// under. This is the point lookup the primary key always supported —
    /// mapping an addressed JID back to a store key, or folding one chat's
    /// unread count, does not need the whole list.
    ///
    /// Returns one stored row, never a synthesized merge of two. While a
    /// PN/LID pair is still split, sticky metadata (pin, mute, archive, name)
    /// can sit on the side this does not return, exactly as it can in
    /// [`chats`](Self::chats), which lists such a pair as two entries. Unioning
    /// the two is [`merge_chat_metadata`]'s job and it happens on
    /// reconciliation; doing it again here would put write-path precedence
    /// rules in a query and make this disagree with the list.
    ///
    /// [`merge_chat_metadata`]: ChatStore::reconcile_chat
    pub async fn chat(&self, jid: &Jid) -> Result<Option<ChatEntry>> {
        use schema::chats::dsl;
        let device_id = self.device_id();
        let jid = jid.to_string();
        let row: Option<ChatRow> = self
            .db()
            .read(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &jid).map_err(db_err)?;
                dsl::chats
                    .filter(dsl::device_id.eq(device_id).and(dsl::jid.eq_any(keys)))
                    // A split pair (rows under both identities, not yet merged)
                    // would match twice; the active thread is the one with
                    // activity on it. Same tiebreak as the list, so the two
                    // surfaces cannot disagree about which row is the thread
                    // when both sides carry the same activity time (common
                    // right after a reconcile, and whenever both are 0).
                    .order((dsl::last_message_ts.desc(), dsl::jid.desc()))
                    .first(conn)
                    .optional()
                    .map_err(db_err)
            })
            .await?;
        Ok(row.map(Into::into))
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
            .read(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &chat).map_err(db_err)?;
                let mut query = dsl::messages
                    .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq_any(keys)))
                    .into_boxed();
                if let Some(cursor) = &before {
                    // Mirrors the sort exactly; anything looser skips or
                    // repeats rows at a page boundary inside a same-second run.
                    query = query.filter(
                        dsl::timestamp_ms
                            .lt(cursor.timestamp_ms)
                            .or(dsl::timestamp_ms
                                .eq(cursor.timestamp_ms)
                                .and(dsl::rowid.lt(cursor.seq))),
                    );
                }
                query
                    .order((dsl::timestamp_ms.desc(), dsl::rowid.desc()))
                    .limit(limit)
                    .load(conn)
                    .map_err(db_err)
            })
            .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// One page of the whole session's messages, every chat interleaved, newest
    /// arrival first. `after` is the cursor of the last row of the page you
    /// have; it yields the page after that one, which is the next batch of
    /// *older* arrivals.
    ///
    /// This is the read a reconciliation consumer wants — "everything that
    /// landed since I last looked, across all chats" — which the chat list plus
    /// [`messages`](Self::messages) can only answer by paging every thread.
    /// Each pass re-enters at the head and walks down until it recognizes what
    /// it already has; the cursor pages *within* a pass and is not carried
    /// across passes:
    ///
    /// ```ignore
    /// let mut after = None;
    /// loop {
    ///     let page = store.messages_by_arrival(after, 100).await?;
    ///     let Some(oldest) = page.last() else { break };
    ///     after = Some(oldest.into());
    ///     // Stop on content, never on a remembered `seq` — see below.
    ///     if page.iter().all(|m| already_stored(&m.chat_jid, &m.id)) { break }
    ///     // ... take the ones that are new ...
    /// }
    /// ```
    ///
    /// Two ways to get this wrong, both silent:
    ///
    /// Passing a remembered cursor as `after` does the opposite of what it
    /// reads like — it asks for rows *older* than that point, so the consumer
    /// walks back into its own history and never sees a new message.
    ///
    /// Stopping at a remembered `seq` skips messages. `seq` is the implicit
    /// rowid, which SQLite assigns as `max(rowid) + 1`: deleting the newest
    /// message hands its number to the next arrival, clearing a chat entirely
    /// restarts at 1, and a `VACUUM` renumbers independently of all that. Each
    /// of those puts a genuinely new message at or below a remembered value,
    /// where a watermark comparison reads it as already seen. Deleting and
    /// clearing are ordinary app-state events this store applies, so it is
    /// routine rather than a corner case. Compare content across passes —
    /// `(chat_jid, id)` is the stable identity.
    ///
    /// Equivalent to [`messages_by_arrival_in_range`](Self::messages_by_arrival_in_range)
    /// with no bounds.
    pub async fn messages_by_arrival(
        &self,
        after: Option<ArrivalCursor>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        self.messages_by_arrival_in_range(after, None, None, limit)
            .await
    }

    /// The arrival feed restricted to a half-open wall-clock window,
    /// `since <= timestamp < until`. Either end may be `None` for unbounded.
    /// Sub-millisecond bounds are honored exactly; stored timestamps are whole
    /// milliseconds, so each end resolves to the first one at or after it.
    ///
    /// The window is a filter over the scan, not a seek: cost tracks the rows
    /// walked, not the rows returned, so a narrow window over an old part of a
    /// large store reads everything newer than it before yielding anything.
    /// Narrowing that would take a `(device_id, timestamp_ms)` index, which
    /// costs every message write; the feed itself does not need one.
    ///
    /// # Ordering
    ///
    /// Arrival, not timestamp — [`StoredMessage::seq`] descending. History-sync
    /// backfill inserts old conversations at new `seq`, so a poller keyed on
    /// `timestamp` would skip those rows forever while an arrival-keyed one
    /// sees them on its next pull. Paging runs newest-first because that is the
    /// direction a volatile cursor survives: every pass re-enters at the head,
    /// so nothing depends on a `seq` still meaning what it did last time.
    ///
    /// # Arrival, not change
    ///
    /// A tombstone or an undecryptable placeholder is a row like any other and
    /// appears here. A *mutation* of a row does not: an edit, a revoke, a star
    /// or a status change rewrites the row in place, and `seq` is assigned by
    /// the INSERT and survives every UPDATE, so a message the consumer has
    /// already walked past never resurfaces at the head no matter what happens
    /// to it afterwards. A consumer that has to track those subscribes to
    /// [`StoreChange::Messages`](crate::types::StoreChange::Messages) via
    /// [`ChatStore::subscribe`] and re-reads the chat it names; this feed
    /// answers "what has arrived", not "what has changed".
    ///
    /// # Cost
    ///
    /// A reverse walk of the `messages` B-tree, which is why the session-wide
    /// read needs no index of its own. The session's `device_id` rides along as
    /// a predicate, so a database file holding several devices walks past its
    /// siblings' rows to fill a page.
    pub async fn messages_by_arrival_in_range(
        &self,
        after: Option<ArrivalCursor>,
        since: Option<DateTime<Utc>>,
        until: Option<DateTime<Utc>>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        // A negative LIMIT means "unbounded" to SQLite; never let that happen.
        let limit = limit.max(0);
        let device_id = self.device_id();
        let since_ms = since.map(ceil_to_ms);
        let until_ms = until.map(ceil_to_ms);
        let rows: Vec<MessageRow> = self
            .db()
            .read(move |conn| {
                arrival_page_query(device_id, after, since_ms, until_ms, limit)
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
            .read(move |conn| {
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
            .read(move |conn| {
                let keys =
                    crate::lid::chat_key_candidates(conn, device_id, &chat).map_err(db_err)?;
                dsl::reactions
                    .filter(
                        dsl::device_id
                            .eq(device_id)
                            .and(dsl::chat_jid.eq_any(keys))
                            .and(dsl::msg_id.eq(&msg_id))
                            .and(dsl::emoji.ne("")),
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
            .read(move |conn| {
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
            .read(move |conn| {
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
            .read(move |conn| {
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
            .read(move |conn| {
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

#[cfg(test)]
mod tests {
    use super::{ArrivalCursor, arrival_page_query};
    use diesel::prelude::*;
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

    const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

    #[derive(diesel::QueryableByName)]
    struct PlanRow {
        #[diesel(sql_type = diesel::sql_types::Text)]
        detail: String,
    }

    /// `EXPLAIN QUERY PLAN` for the query as diesel actually renders it. Binds
    /// stay unbound: the planner does not need their values, and asking it
    /// about hand-written SQL would pin a string this crate never runs.
    fn plan(sql: &str) -> String {
        let mut conn = SqliteConnection::establish(":memory:").expect("in-memory sqlite");
        conn.run_pending_migrations(MIGRATIONS).expect("migrate");
        let rows: Vec<PlanRow> = diesel::sql_query(format!("EXPLAIN QUERY PLAN {sql}"))
            .load(&mut conn)
            .expect("explain");
        rows.into_iter()
            .map(|row| row.detail)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rendered_sql(after: Option<ArrivalCursor>, since_ms: Option<i64>) -> String {
        let query = arrival_page_query(1, after, since_ms, None, 50);
        let debug = diesel::debug_query::<diesel::sqlite::Sqlite, _>(&query).to_string();
        // `debug_query` appends the bind list after the statement.
        match debug.split_once(" -- binds") {
            Some((sql, _)) => sql.to_string(),
            None => debug,
        }
    }

    /// The whole point of ordering the feed by arrival: SQLite answers it by
    /// walking the `messages` B-tree backwards — a plain reverse `SCAN`, or a
    /// `SEARCH ... USING INTEGER PRIMARY KEY` that seeks to the cursor first —
    /// so a page costs no index and no sort.
    ///
    /// Left to itself the planner does the opposite: `idx_messages_by_id` leads
    /// with `device_id`, so it enters there and pays a temp B-tree to recover
    /// rowid order, turning every page into a full sort of the device's
    /// messages. That is what the `+device_id` in the query prevents, and this
    /// is the test that notices if it stops working.
    #[test]
    fn arrival_page_reads_the_table_in_arrival_order_without_sorting() {
        for (label, sql) in [
            ("first page", rendered_sql(None, None)),
            (
                "resumed page",
                rendered_sql(Some(ArrivalCursor { seq: 4_096 }), None),
            ),
            ("windowed page", rendered_sql(None, Some(1_700_000_000_000))),
        ] {
            let plan = plan(&sql);
            assert!(
                // Any `INDEX`, not just `USING INDEX`: SQLite also spells the
                // regressed plans `USING COVERING INDEX` and `USING AUTOMATIC
                // COVERING INDEX`, and the plans this test wants name neither
                // (`INTEGER PRIMARY KEY` is the table).
                plan.contains("messages") && !plan.contains("INDEX"),
                "{label}: expected the table itself, got:\n{plan}"
            );
            assert!(
                !plan.contains("TEMP B-TREE"),
                "{label}: ordering must stream from the table, got:\n{plan}"
            );
        }
    }
}
