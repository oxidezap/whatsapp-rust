//! The store itself: a write-behind materializer over the client's event
//! stream plus the public write API. All writes funnel through one writer task
//! (one transaction per drained batch), so event order is preserved and fan-in
//! bursts don't pay per-event commit costs.

use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

use buffa::Message as _;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};
use log::warn;
use tokio::sync::{broadcast, mpsc, oneshot};
use wacore::store::error::StoreError;
use wacore::types::events::{Event, EventHandler, EventInterest, EventKind, InboundMessage};
use wacore::types::presence::ReceiptType;
use wacore_binary::{Jid, JidExt as _};
use waproto::whatsapp as wa;
use whatsapp_rust_sqlite_storage::{SharedSqlite, SqliteStore};

use crate::error::{ChatStoreError, Result, db_err};
use crate::materialize::{KIND_UNDECRYPTABLE, MessageOp, classify, extract_text, message_kind};
use crate::schema;
use crate::types::StoreChange;

const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

/// Max events applied per transaction. Bounds transaction size during
/// offline-drain bursts; the writer loops immediately for the remainder.
const BATCH_MAX: usize = 128;

/// Capacity of the invalidation broadcast. Lagging receivers see
/// `RecvError::Lagged` and should re-query everything they display.
const CHANGE_CHANNEL_CAPACITY: usize = 256;

/// Manually-marked-unread sentinel for `chats.unread_count` (WA Web convention).
const UNREAD_MARKER: i32 = -1;

pub(crate) enum WriterMsg {
    Event(Arc<Event>),
    Outgoing {
        chat: Jid,
        msg_id: String,
        proto: Vec<u8>,
        kind: &'static str,
        text: Option<String>,
        timestamp_ms: i64,
    },
    // String, not StoreError: one batch outcome fans out to many waiters and
    // StoreError is not Clone.
    Flush(oneshot::Sender<std::result::Result<(), String>>),
}

/// SQLite-backed chat/message/contact history, materialized from the client's
/// event stream into the same database file as the device store.
///
/// Wire-up:
/// ```ignore
/// let chat_store = ChatStore::new(&sqlite_store).await?;
/// client.register_handler(chat_store.handler());
/// let mut changes = chat_store.subscribe();
/// ```
pub struct ChatStore {
    db: SharedSqlite,
    device_id: i32,
    tx: mpsc::UnboundedSender<WriterMsg>,
    changes: broadcast::Sender<StoreChange>,
}

struct ChatStoreHandler {
    tx: mpsc::UnboundedSender<WriterMsg>,
}

impl EventHandler for ChatStoreHandler {
    fn handle_event(&self, event: Arc<Event>) {
        // Writer gone (store dropped): nothing to record into, drop silently.
        let _ = self.tx.send(WriterMsg::Event(event));
    }

    fn interest(&self) -> EventInterest {
        EventInterest::of(&[
            EventKind::Messages,
            EventKind::Receipt,
            EventKind::ServerAck,
            EventKind::UndecryptableMessage,
            EventKind::HistorySync,
            EventKind::PushNameUpdate,
            EventKind::ContactUpdate,
            EventKind::PinUpdate,
            EventKind::MuteUpdate,
            EventKind::ArchiveUpdate,
            EventKind::StarUpdate,
            EventKind::MarkChatAsReadUpdate,
            EventKind::DeleteChatUpdate,
            EventKind::ClearChatUpdate,
            EventKind::DeleteMessageForMeUpdate,
        ])
    }
}

impl ChatStore {
    /// Open (running migrations if needed) on the same database file as
    /// `store`, bound to its device id, and start the writer task.
    pub async fn new(store: &SqliteStore) -> Result<Arc<Self>> {
        let db = store.shared();
        let device_id = store.device_id();

        db.run(|conn| {
            conn.run_pending_migrations(MIGRATIONS)
                .map(|_| ())
                .map_err(StoreError::Migration)?;
            #[cfg(feature = "search")]
            crate::fts::ensure_fts(conn).map_err(db_err)?;
            Ok(())
        })
        .await?;

        let (tx, rx) = mpsc::unbounded_channel();
        let (changes, _) = broadcast::channel(CHANGE_CHANNEL_CAPACITY);

        let this = Arc::new(Self {
            db: db.clone(),
            device_id,
            tx,
            changes: changes.clone(),
        });
        tokio::spawn(writer_loop(db, device_id, rx, changes));
        Ok(this)
    }

    /// Event handler to register on the client. The store keeps working if the
    /// handler outlives it (events are then dropped), and vice versa.
    pub fn handler(&self) -> Arc<dyn EventHandler> {
        Arc::new(ChatStoreHandler {
            tx: self.tx.clone(),
        })
    }

    /// Subscribe to invalidation signals. Emitted once per committed write
    /// batch, deduplicated. On `Lagged`, re-query all visible state.
    pub fn subscribe(&self) -> broadcast::Receiver<StoreChange> {
        self.changes.subscribe()
    }

    /// Record a message this client just sent. Goes through the writer queue so
    /// it cannot race the server ack / receipts that follow it in event order.
    /// Status starts at [`MessageStatus::Pending`](crate::types::MessageStatus::Pending)
    /// and is lifted by acks/receipts.
    pub fn record_outgoing(
        &self,
        chat: &Jid,
        msg_id: impl Into<String>,
        message: &wa::Message,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let base = wacore::proto_helpers::MessageExt::get_base_message(message);
        self.tx
            .send(WriterMsg::Outgoing {
                chat: chat.clone(),
                msg_id: msg_id.into(),
                proto: message.encode_to_vec(),
                kind: message_kind(base),
                text: extract_text(base),
                timestamp_ms: timestamp.timestamp_millis(),
            })
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))
    }

    /// Wait until every write enqueued before this call is committed. Errors
    /// with [`ChatStoreError::WriteBatchFailed`] when any batch since the
    /// previous flush answer rolled back. The contract is TEMPORAL, not
    /// per-caller: writes enqueued by anyone before this call share its fate,
    /// so a failure that dropped someone else's earlier writes still reports
    /// here (conservative: a false failure is possible, a false success is
    /// not).
    pub async fn flush(&self) -> Result<()> {
        let (tx, rx) = oneshot::channel();
        self.tx
            .send(WriterMsg::Flush(tx))
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))?;
        rx.await
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))?
            .map_err(ChatStoreError::WriteBatchFailed)
    }

    pub fn device_id(&self) -> i32 {
        self.device_id
    }

    pub(crate) fn db(&self) -> &SharedSqlite {
        &self.db
    }
}

/// A sync action's message range. The wire boundary is unix SECONDS while
/// rows store milliseconds, so the boundary covers its WHOLE second; when the
/// action lists explicit boundary messages (WA Web fills `messages` exactly to
/// disambiguate same-second siblings), only the listed ids inside the boundary
/// second count as covered.
struct RangeBound {
    /// First ms of the boundary second.
    second_start_ms: i64,
    /// Last ms of the boundary second.
    second_end_ms: i64,
    /// Ids the action explicitly covers at the boundary; `None` = the whole
    /// boundary second is covered (sender did not enumerate).
    keys: Option<Vec<String>>,
}

fn range_bound(
    range: &buffa::MessageField<wa::sync_action_value::SyncActionMessageRange>,
) -> Option<RangeBound> {
    let range = range.as_option()?;
    let ts_secs = range.last_message_timestamp.filter(|&ts| ts > 0)?;
    let second_start_ms = ts_secs.saturating_mul(1000);
    let keys: Vec<String> = range
        .messages
        .iter()
        .filter_map(|m| m.key.as_option().and_then(|k| k.id.clone()))
        .collect();
    Some(RangeBound {
        second_start_ms,
        second_end_ms: second_start_ms.saturating_add(999),
        keys: (!keys.is_empty()).then_some(keys),
    })
}

/// Extra read-boundary ids kept per chat; overflow drops the oldest entries.
const READ_EXTRA_IDS_CAP: usize = 256;

/// The chat's materialized self-read state: everything at or below the
/// watermark is read, plus the explicitly-named ids — boundary-instant/keyed
/// coverage that a scalar watermark cannot express (both directions of the
/// same-second ambiguity are lossy without them).
struct ReadState {
    watermark_ms: i64,
    extra_ids: Vec<String>,
}

impl ReadState {
    fn covers(&self, ts_ms: i64, msg_id: &str) -> bool {
        ts_ms <= self.watermark_ms || self.extra_ids.iter().any(|id| id == msg_id)
    }
}

fn read_state(conn: &mut SqliteConnection, device_id: i32, chat: &str) -> QueryResult<ReadState> {
    let row: Option<(i64, Option<String>)> = chat_row(device_id, chat)
        .select((
            schema::chats::read_boundary_ms,
            schema::chats::read_boundary_ids,
        ))
        .first(conn)
        .optional()?;
    let (watermark_ms, ids_json) = row.unwrap_or((0, None));
    let extra_ids = ids_json
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    Ok(ReadState {
        watermark_ms,
        extra_ids,
    })
}

/// Fold a read event (watermark + explicitly covered ids) into the chat's
/// monotonic read state. Ids already implied by the watermark are pruned.
/// Returns the post-advance state, or `None` when the event brought nothing
/// new (a stale replay, which must not touch the unread badge).
fn advance_read_state(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    watermark_ms: i64,
    covered_ids: &[String],
) -> QueryResult<Option<ReadState>> {
    use schema::messages::dsl;
    let mut state = read_state(conn, device_id, chat)?;
    let before = (state.watermark_ms, state.extra_ids.clone());
    if watermark_ms > state.watermark_ms {
        state.watermark_ms = watermark_ms;
    }
    for id in covered_ids {
        if !state.extra_ids.iter().any(|existing| existing == id) {
            state.extra_ids.push(id.clone());
        }
    }
    if !state.extra_ids.is_empty() {
        let implied: Vec<String> = dsl::messages
            .filter(
                dsl::device_id
                    .eq(device_id)
                    .and(dsl::chat_jid.eq(chat))
                    .and(dsl::msg_id.eq_any(&state.extra_ids))
                    .and(dsl::timestamp_ms.le(state.watermark_ms)),
            )
            .select(dsl::msg_id)
            .load(conn)?;
        if !implied.is_empty() {
            state.extra_ids.retain(|id| !implied.contains(id));
        }
    }
    if state.extra_ids.len() > READ_EXTRA_IDS_CAP {
        let overflow = state.extra_ids.len() - READ_EXTRA_IDS_CAP;
        state.extra_ids.drain(..overflow);
    }
    if (state.watermark_ms, &state.extra_ids) == (before.0, &before.1) {
        return Ok(None);
    }
    let ids_json = (!state.extra_ids.is_empty())
        .then(|| serde_json::to_string(&state.extra_ids).ok())
        .flatten();
    diesel::update(chat_row(device_id, chat))
        .set((
            schema::chats::read_boundary_ms.eq(state.watermark_ms),
            schema::chats::read_boundary_ids.eq(ids_json),
        ))
        .execute(conn)?;
    Ok(Some(state))
}

/// Incoming rows not covered by the read state.
fn count_unread(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    state: &ReadState,
) -> QueryResult<i32> {
    use schema::messages::dsl;
    let mut query = dsl::messages
        .filter(
            dsl::device_id
                .eq(device_id)
                .and(dsl::chat_jid.eq(chat))
                .and(dsl::from_me.eq(false))
                .and(dsl::timestamp_ms.gt(state.watermark_ms)),
        )
        .into_boxed();
    if !state.extra_ids.is_empty() {
        query = query.filter(dsl::msg_id.ne_all(&state.extra_ids));
    }
    let unread: i64 = query.count().get_result(conn)?;
    Ok(unread.min(i32::MAX as i64) as i32)
}

/// Incoming rows NOT covered by `bound`: strictly newer than the boundary
/// second, plus same-second rows the action's keyed list does not name.
/// Rows the read state already covers don't count — a stale ranged action
/// replaying after a newer self-read must not resurrect their badge.
fn count_uncovered_incoming(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    bound: &RangeBound,
) -> QueryResult<i32> {
    use schema::messages::dsl;
    let state = read_state(conn, device_id, chat)?;
    let mut base = dsl::messages
        .filter(
            dsl::device_id
                .eq(device_id)
                .and(dsl::chat_jid.eq(chat))
                .and(dsl::from_me.eq(false))
                .and(dsl::timestamp_ms.gt(state.watermark_ms)),
        )
        .into_boxed();
    if !state.extra_ids.is_empty() {
        base = base.filter(dsl::msg_id.ne_all(state.extra_ids.clone()));
    }
    let uncovered: i64 = match &bound.keys {
        None => base
            .filter(dsl::timestamp_ms.gt(bound.second_end_ms))
            .count()
            .get_result(conn)?,
        Some(keys) => base
            .filter(dsl::timestamp_ms.gt(bound.second_start_ms - 1))
            .filter(
                dsl::timestamp_ms
                    .gt(bound.second_end_ms)
                    .or(dsl::msg_id.ne_all(keys.clone())),
            )
            .count()
            .get_result(conn)?,
    };
    Ok(uncovered.min(i32::MAX as i64) as i32)
}

/// Chats/contacts touched by a batch, accumulated for post-commit invalidation.
#[derive(Default)]
struct ChangeSet {
    chats: bool,
    contacts: bool,
    message_chats: BTreeSet<String>,
}

async fn writer_loop(
    db: SharedSqlite,
    device_id: i32,
    mut rx: mpsc::UnboundedReceiver<WriterMsg>,
    changes: broadcast::Sender<StoreChange>,
) {
    // Sticky across iterations: a failed batch with no flush waiter of its
    // own must still be reported to the NEXT flush (a >BATCH_MAX backlog spans
    // several transactions). Consumed when delivered.
    let mut pending_error: Option<String> = None;
    while let Some(first) = rx.recv().await {
        let mut batch = Vec::with_capacity(8);
        let mut flushes = Vec::new();
        // A Flush is a batch BARRIER: stop draining there, so writes enqueued
        // after a caller's flush() can neither commit ahead of that call's
        // answer nor drag the awaited writes down with a later failure.
        let mut queue_msg = |msg: WriterMsg, batch: &mut Vec<WriterMsg>| match msg {
            WriterMsg::Flush(done) => {
                flushes.push(done);
                true
            }
            other => {
                batch.push(other);
                false
            }
        };
        let mut at_barrier = queue_msg(first, &mut batch);
        while !at_barrier && batch.len() < BATCH_MAX {
            match rx.try_recv() {
                Ok(msg) => at_barrier = queue_msg(msg, &mut batch),
                Err(_) => break,
            }
        }

        if !batch.is_empty() {
            let result = db
                .run(move |conn| {
                    conn.transaction(|conn| {
                        let mut cs = ChangeSet::default();
                        for msg in &batch {
                            apply_writer_msg(conn, device_id, msg, &mut cs)?;
                        }
                        Ok(cs)
                    })
                    .map_err(db_err)
                })
                .await;
            match result {
                Ok(cs) => emit_changes(&changes, cs),
                // The batch rolled back; state stays consistent (previous
                // commit) but this slice of history is lost. Surfacing beats
                // crashing the writer.
                Err(e) => {
                    warn!("chat-store: dropping write batch: {e}");
                    pending_error = Some(e.to_string());
                }
            }
        }
        if flushes.is_empty() {
            continue;
        }
        let outcome = match pending_error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        };
        for done in flushes {
            let _ = done.send(outcome.clone());
        }
    }
}

fn emit_changes(changes: &broadcast::Sender<StoreChange>, cs: ChangeSet) {
    if cs.chats {
        let _ = changes.send(StoreChange::Chats);
    }
    if cs.contacts {
        let _ = changes.send(StoreChange::Contacts);
    }
    for chat in cs.message_chats {
        if let Ok(jid) = Jid::from_str(&chat) {
            let _ = changes.send(StoreChange::Messages { chat: jid });
        }
    }
}

fn apply_writer_msg(
    conn: &mut SqliteConnection,
    device_id: i32,
    msg: &WriterMsg,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    match msg {
        WriterMsg::Event(event) => apply_event(conn, device_id, event, cs),
        WriterMsg::Outgoing {
            chat,
            msg_id,
            proto,
            kind,
            text,
            timestamp_ms,
        } => {
            let chat_str = chat.to_string();
            let stored = insert_message(
                conn,
                device_id,
                NewMessage {
                    chat_jid: &chat_str,
                    msg_id,
                    sender_jid: "",
                    from_me: true,
                    timestamp_ms: *timestamp_ms,
                    kind,
                    text: text.as_deref(),
                    proto: Some(proto),
                    status: wa::web_message_info::Status::PENDING as i32,
                    starred: false,
                    overwrite: true,
                },
            )?;
            if stored != StoredRow::Skipped {
                bump_chat(
                    conn,
                    device_id,
                    &chat_str,
                    ChatBump {
                        msg_id,
                        ts_ms: *timestamp_ms,
                        preview: text.as_deref(),
                        kind: Some(kind),
                        unread_delta: 0,
                    },
                )?;
                cs.chats = true;
            }
            cs.message_chats.insert(chat_str);
            Ok(())
        }
        WriterMsg::Flush(_) => Ok(()),
    }
}

fn apply_event(
    conn: &mut SqliteConnection,
    device_id: i32,
    event: &Event,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    match event {
        Event::Messages(batch) => {
            for inbound in batch.iter() {
                apply_inbound(conn, device_id, inbound, cs)?;
            }
            Ok(())
        }
        Event::Receipt(receipt) => apply_receipt(conn, device_id, receipt, cs),
        Event::ServerAck(ack) => apply_server_ack(conn, device_id, ack, cs),
        Event::UndecryptableMessage(undec) => {
            let chat = undec.info.source.chat.to_string();
            let sender = undec.info.source.sender.to_string();
            let inserted = insert_message(
                conn,
                device_id,
                NewMessage {
                    chat_jid: &chat,
                    msg_id: &undec.info.id,
                    sender_jid: &sender,
                    from_me: undec.info.source.is_from_me,
                    timestamp_ms: undec.info.timestamp.timestamp_millis(),
                    kind: KIND_UNDECRYPTABLE,
                    text: None,
                    proto: None,
                    status: wa::web_message_info::Status::DELIVERY_ACK as i32,
                    starred: false,
                    overwrite: false,
                },
            )?;
            // A duplicate placeholder (or one for an id that was already
            // recovered/revoked) must neither recount nor blank the preview.
            if inserted == StoredRow::Inserted {
                bump_chat(
                    conn,
                    device_id,
                    &chat,
                    ChatBump {
                        msg_id: &undec.info.id,
                        ts_ms: undec.info.timestamp.timestamp_millis(),
                        preview: None,
                        kind: Some(KIND_UNDECRYPTABLE),
                        unread_delta: i32::from(!undec.info.source.is_from_me),
                    },
                )?;
                cs.chats = true;
            }
            cs.message_chats.insert(chat);
            Ok(())
        }
        Event::HistorySync(lazy) => apply_history_sync(conn, device_id, lazy, cs),
        Event::PushNameUpdate(update) => {
            upsert_contact_push_name(
                conn,
                device_id,
                &update.jid.to_string(),
                &update.new_push_name,
            )?;
            cs.contacts = true;
            Ok(())
        }
        Event::ContactUpdate(update) => {
            upsert_contact_names(
                conn,
                device_id,
                &update.jid.to_string(),
                update.action.full_name.as_deref(),
                update.action.first_name.as_deref(),
            )?;
            cs.contacts = true;
            Ok(())
        }
        Event::PinUpdate(update) => {
            let pinned_at = update
                .action
                .pinned
                .unwrap_or(false)
                .then(|| update.timestamp.timestamp_millis());
            let chat = update.jid.to_string();
            ensure_chat(conn, device_id, &chat)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::pinned_at.eq(pinned_at))
                .execute(conn)?;
            cs.chats = true;
            Ok(())
        }
        Event::MuteUpdate(update) => {
            let muted_until = if update.action.muted.unwrap_or(false) {
                // No end timestamp = muted forever.
                Some(update.action.mute_end_timestamp.unwrap_or(i64::MAX))
            } else {
                None
            };
            let chat = update.jid.to_string();
            ensure_chat(conn, device_id, &chat)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::muted_until.eq(muted_until))
                .execute(conn)?;
            cs.chats = true;
            Ok(())
        }
        Event::ArchiveUpdate(update) => {
            let chat = update.jid.to_string();
            ensure_chat(conn, device_id, &chat)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::archived.eq(update.action.archived.unwrap_or(false)))
                .execute(conn)?;
            cs.chats = true;
            Ok(())
        }
        Event::MarkChatAsReadUpdate(update) => {
            let chat = update.jid.to_string();
            ensure_chat(conn, device_id, &chat)?;
            if update.action.read.unwrap_or(false) {
                // A delayed replay only covers messages up to its range;
                // anything we materialized past it is still unread. Reads
                // fold into the monotonic read state (watermark + keyed
                // boundary ids), so later stale actions/receipts can't
                // resurrect the badge — and a stale replay itself changes
                // nothing.
                let advanced = match range_bound(&update.action.message_range) {
                    Some(bound) => {
                        // A keyed boundary second can't be expressed by the
                        // watermark alone: it stops short and the named ids
                        // ride along in the state.
                        let (watermark, ids): (i64, &[String]) = match &bound.keys {
                            Some(keys) => (bound.second_start_ms - 1, keys.as_slice()),
                            None => (bound.second_end_ms, &[]),
                        };
                        advance_read_state(conn, device_id, &chat, watermark, ids)?
                    }
                    None => {
                        use schema::messages::dsl;
                        let newest: Option<Option<i64>> = dsl::messages
                            .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq(&chat)))
                            .select(diesel::dsl::max(dsl::timestamp_ms))
                            .first(conn)
                            .optional()?;
                        // Empty chat: the action's own timestamp is the read
                        // moment — the state must still advance, or a later
                        // stale replay resurrects a badge this read cleared.
                        let watermark = newest
                            .flatten()
                            .unwrap_or_else(|| update.timestamp.timestamp_millis());
                        advance_read_state(conn, device_id, &chat, watermark, &[])?
                    }
                };
                if let Some(state) = advanced {
                    let unread = count_unread(conn, device_id, &chat, &state)?;
                    diesel::update(chat_row(device_id, &chat))
                        .set(schema::chats::unread_count.eq(unread))
                        .execute(conn)?;
                }
            } else {
                diesel::update(chat_row(device_id, &chat))
                    .set(schema::chats::unread_count.eq(UNREAD_MARKER))
                    .execute(conn)?;
            }
            cs.chats = true;
            Ok(())
        }
        Event::StarUpdate(update) => {
            let chat = update.chat_jid.to_string();
            diesel::update(message_row(device_id, &chat, &update.message_id))
                .set(schema::messages::starred.eq(update.action.starred.unwrap_or(false)))
                .execute(conn)?;
            cs.message_chats.insert(chat);
            Ok(())
        }
        Event::DeleteChatUpdate(update) => {
            let chat = update.jid.to_string();
            let bound = range_bound(&update.action.message_range);
            delete_chat_rows(conn, device_id, &chat, true, bound.as_ref())?;
            // A delayed delete only covers up to its range: when newer
            // messages were already materialized locally, the chat survives
            // with them instead of vanishing.
            let survivors = remaining_messages(conn, device_id, &chat)?;
            match &bound {
                Some(bound) if survivors > 0 => {
                    recompute_chat_preview(conn, device_id, &chat)?;
                    let unread = count_uncovered_incoming(conn, device_id, &chat, bound)?;
                    diesel::update(chat_row(device_id, &chat))
                        .set(schema::chats::unread_count.eq(unread))
                        .execute(conn)?;
                }
                _ => {
                    diesel::delete(chat_row(device_id, &chat)).execute(conn)?;
                }
            }
            cs.chats = true;
            cs.message_chats.insert(chat);
            Ok(())
        }
        Event::ClearChatUpdate(update) => {
            let chat = update.jid.to_string();
            let bound = range_bound(&update.action.message_range);
            delete_chat_rows(
                conn,
                device_id,
                &chat,
                update.delete_starred,
                bound.as_ref(),
            )?;
            // Starred rows (and messages newer than the range) may survive the
            // clear: the preview/kind must reflect the newest survivor, not go
            // blank (or keep stale kind).
            recompute_chat_preview(conn, device_id, &chat)?;
            // Unread survivors past a ranged clear keep their badge; an
            // unranged clear empties the chat, so zero is exact there.
            let unread = match &bound {
                Some(bound) => count_uncovered_incoming(conn, device_id, &chat, bound)?,
                None => 0,
            };
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::unread_count.eq(unread))
                .execute(conn)?;
            cs.chats = true;
            cs.message_chats.insert(chat);
            Ok(())
        }
        Event::DeleteMessageForMeUpdate(update) => {
            let chat = update.chat_jid.to_string();
            // Capture the victim's read state before it goes: deleting an
            // unread inbound row must also drop its badge (sentinel -1 and
            // already-read rows are untouched).
            let victim: Option<(bool, i64)> = message_row(device_id, &chat, &update.message_id)
                .select((schema::messages::from_me, schema::messages::timestamp_ms))
                .first(conn)
                .optional()?;
            diesel::delete(message_row(device_id, &chat, &update.message_id)).execute(conn)?;
            if let Some((false, ts_ms)) = victim
                && !read_state(conn, device_id, &chat)?.covers(ts_ms, &update.message_id)
            {
                diesel::update(
                    chat_row(device_id, &chat).filter(schema::chats::unread_count.gt(0)),
                )
                .set(schema::chats::unread_count.eq(schema::chats::unread_count - 1))
                .execute(conn)?;
            }
            diesel::delete(
                schema::reactions::table.filter(
                    schema::reactions::device_id
                        .eq(device_id)
                        .and(schema::reactions::chat_jid.eq(&chat))
                        .and(schema::reactions::msg_id.eq(&update.message_id)),
                ),
            )
            .execute(conn)?;
            diesel::delete(
                schema::message_receipts::table.filter(
                    schema::message_receipts::device_id
                        .eq(device_id)
                        .and(schema::message_receipts::chat_jid.eq(&chat))
                        .and(schema::message_receipts::msg_id.eq(&update.message_id)),
                ),
            )
            .execute(conn)?;
            // The deleted row may have been the chat's preview.
            recompute_chat_preview(conn, device_id, &chat)?;
            cs.chats = true;
            cs.message_chats.insert(chat);
            Ok(())
        }
        _ => Ok(()),
    }
}

fn apply_inbound(
    conn: &mut SqliteConnection,
    device_id: i32,
    inbound: &InboundMessage,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    let info = &inbound.info;
    let chat = info.source.chat.to_string();
    let sender = info.source.sender.to_string();
    let ts_ms = info.timestamp.timestamp_millis();

    // Live push names ride on every message; keep contacts warm from them.
    if !info.push_name.is_empty() && !info.source.is_from_me {
        upsert_contact_push_name(conn, device_id, &sender, &info.push_name)?;
        cs.contacts = true;
    }

    match classify(&inbound.message) {
        MessageOp::Store { kind, text } => {
            let inserted = insert_message(
                conn,
                device_id,
                NewMessage {
                    chat_jid: &chat,
                    msg_id: &info.id,
                    sender_jid: &sender,
                    from_me: info.source.is_from_me,
                    timestamp_ms: ts_ms,
                    kind,
                    text: text.as_deref(),
                    proto: Some(&inbound.message.encode_to_vec()),
                    status: if info.source.is_from_me {
                        wa::web_message_info::Status::SERVER_ACK as i32
                    } else {
                        wa::web_message_info::Status::DELIVERY_ACK as i32
                    },
                    starred: false,
                    overwrite: true,
                },
            )?;
            // A refreshed row (redelivery, PDO recovery of a placeholder that
            // already counted) must not inflate the unread badge again — and a
            // skipped one (revoked tombstone) must not surface its content in
            // the chat preview at all.
            let unread_delta =
                i32::from(inserted == StoredRow::Inserted && !info.source.is_from_me);
            if inserted != StoredRow::Skipped {
                bump_chat(
                    conn,
                    device_id,
                    &chat,
                    ChatBump {
                        msg_id: &info.id,
                        ts_ms,
                        preview: text.as_deref(),
                        kind: Some(kind),
                        unread_delta,
                    },
                )?;
                cs.chats = true;
            }
            cs.message_chats.insert(chat);
        }
        MessageOp::Reaction { target_id, emoji } => {
            apply_reaction(conn, device_id, &chat, &target_id, &sender, &emoji, ts_ms)?;
            cs.message_chats.insert(chat);
        }
        MessageOp::Edit {
            target_id,
            new_text,
            new_kind,
            new_proto,
        } => {
            if apply_edit(
                conn,
                device_id,
                &chat,
                &target_id,
                new_text.as_deref(),
                new_kind,
                &new_proto,
                ts_ms,
            )? {
                cs.chats = true;
            }
            cs.message_chats.insert(chat);
        }
        MessageOp::Revoke {
            target_id,
            target_from_me,
            target_participant,
        } => {
            if apply_revoke(
                conn,
                device_id,
                &chat,
                &target_id,
                target_participant.as_deref().unwrap_or(&sender),
                target_from_me,
                ts_ms,
            )? {
                cs.chats = true;
            }
            cs.message_chats.insert(chat);
        }
        MessageOp::Ignore => {}
    }
    Ok(())
}

/// Apply an edit to its target row. Monotonic on `edited_at_ms` so a replayed
/// or stale (e.g. history-sync) edit can't roll back a newer one. Returns
/// whether the chat-list preview changed.
#[allow(clippy::too_many_arguments)]
fn apply_edit(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    target_id: &str,
    new_text: Option<&str>,
    new_kind: &str,
    new_proto: &[u8],
    ts_ms: i64,
) -> QueryResult<bool> {
    use schema::messages::dsl;
    let updated = diesel::update(
        message_row(device_id, chat, target_id)
            // A tombstone absorbs edits too: revoked content must not resurface.
            .filter(dsl::revoked.eq(false))
            .filter(dsl::edited_at_ms.is_null().or(dsl::edited_at_ms.le(ts_ms))),
    )
    .set((
        dsl::text_content.eq(new_text),
        dsl::kind.eq(new_kind),
        dsl::proto.eq(Some(new_proto)),
        dsl::edited_at_ms.eq(Some(ts_ms)),
    ))
    .execute(conn)?;
    if updated == 0 {
        return Ok(false);
    }
    refresh_preview_if_latest(conn, device_id, chat, target_id, new_text, Some(new_kind))
}

/// Tombstone the target row. A revoke arriving before its content (offline
/// drain reordering) inserts the tombstone up front, so the content's later
/// arrival can't resurrect it. Returns whether the chat-list preview changed.
fn apply_revoke(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    target_id: &str,
    sender: &str,
    target_from_me: bool,
    ts_ms: i64,
) -> QueryResult<bool> {
    use schema::messages::dsl;
    let updated = diesel::update(message_row(device_id, chat, target_id))
        .set((
            dsl::revoked.eq(true),
            dsl::text_content.eq(None::<String>),
            dsl::proto.eq(None::<Vec<u8>>),
        ))
        .execute(conn)?;
    if updated == 0 {
        diesel::insert_into(dsl::messages)
            .values((
                dsl::device_id.eq(device_id),
                dsl::chat_jid.eq(chat),
                dsl::msg_id.eq(target_id),
                dsl::sender_jid.eq(sender),
                dsl::from_me.eq(target_from_me),
                dsl::timestamp_ms.eq(ts_ms),
                dsl::kind.eq("unknown"),
                dsl::revoked.eq(true),
            ))
            .on_conflict_do_nothing()
            .execute(conn)?;
        return Ok(false);
    }
    refresh_preview_if_latest(conn, device_id, chat, target_id, None, None)
}

/// When `msg_id` is the chat's most recent message, replace the denormalized
/// chat-list preview (an edit/revoke of an older message leaves it alone).
/// "Most recent" uses the same total order as `messages()` — `(timestamp_ms,
/// msg_id)` — so a same-millisecond sibling can't hijack the preview.
fn refresh_preview_if_latest(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    msg_id: &str,
    preview: Option<&str>,
    kind: Option<&str>,
) -> QueryResult<bool> {
    use schema::messages::dsl;
    let newest: Option<String> = dsl::messages
        .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq(chat)))
        .order((dsl::timestamp_ms.desc(), dsl::msg_id.desc()))
        .select(dsl::msg_id)
        .first(conn)
        .optional()?;
    if newest.as_deref() != Some(msg_id) {
        return Ok(false);
    }
    diesel::update(chat_row(device_id, chat))
        .set((
            schema::chats::last_message_preview.eq(preview),
            schema::chats::last_message_kind.eq(kind),
        ))
        .execute(conn)?;
    Ok(true)
}

/// Re-derive the chat-list preview from the newest remaining message (used
/// after deletions, where the previewed row may be gone).
///
/// `last_message_ts` is deliberately NOT recomputed: it models the chat's
/// activity (list position), which WhatsApp keeps in place when the latest
/// message is deleted-for-me. Newest-row time is derivable via
/// `messages(chat, None, 1)` if a consumer ever needs it.
fn recompute_chat_preview(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
) -> QueryResult<()> {
    use schema::messages::dsl;
    let newest: Option<(Option<String>, String, bool)> = dsl::messages
        .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq(chat)))
        .order((dsl::timestamp_ms.desc(), dsl::msg_id.desc()))
        .select((dsl::text_content, dsl::kind, dsl::revoked))
        .first(conn)
        .optional()?;
    let (preview, kind) = match newest {
        // A tombstone previews as nothing at all — its pre-revoke kind must
        // not leak back (mirrors the revoke path's (None, None)).
        Some((_, _, true)) | None => (None, None),
        Some((text, kind, false)) => (text, Some(kind)),
    };
    diesel::update(chat_row(device_id, chat))
        .set((
            schema::chats::last_message_preview.eq(preview),
            schema::chats::last_message_kind.eq(kind),
        ))
        .execute(conn)?;
    Ok(())
}

fn apply_reaction(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    target_id: &str,
    sender: &str,
    emoji: &str,
    ts_ms: i64,
) -> QueryResult<()> {
    use schema::reactions::dsl;
    if emoji.is_empty() {
        // Same monotonic rule as the add path: a stale remove (older than the
        // stored reaction) must not delete a newer live one.
        diesel::delete(
            dsl::reactions.filter(
                dsl::device_id
                    .eq(device_id)
                    .and(dsl::chat_jid.eq(chat))
                    .and(dsl::msg_id.eq(target_id))
                    .and(dsl::sender_jid.eq(sender))
                    .and(dsl::ts_ms.le(ts_ms)),
            ),
        )
        .execute(conn)?;
    } else {
        diesel::insert_into(dsl::reactions)
            .values((
                dsl::device_id.eq(device_id),
                dsl::chat_jid.eq(chat),
                dsl::msg_id.eq(target_id),
                dsl::sender_jid.eq(sender),
                dsl::emoji.eq(emoji),
                dsl::ts_ms.eq(ts_ms),
            ))
            .on_conflict_do_nothing()
            .execute(conn)?;
        // Latest reaction per sender wins; a stale copy (e.g. from a history
        // chunk) must not replace a newer live one.
        diesel::update(
            dsl::reactions.filter(
                dsl::device_id
                    .eq(device_id)
                    .and(dsl::chat_jid.eq(chat))
                    .and(dsl::msg_id.eq(target_id))
                    .and(dsl::sender_jid.eq(sender))
                    .and(dsl::ts_ms.le(ts_ms)),
            ),
        )
        .set((dsl::emoji.eq(emoji), dsl::ts_ms.eq(ts_ms)))
        .execute(conn)?;
    }
    Ok(())
}

fn apply_receipt(
    conn: &mut SqliteConnection,
    device_id: i32,
    receipt: &wacore::types::events::Receipt,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    let chat = receipt.source.chat.to_string();
    let ts_ms = receipt.timestamp.timestamp_millis();

    let status = match receipt.r#type {
        ReceiptType::Delivered => wa::web_message_info::Status::DELIVERY_ACK as i32,
        ReceiptType::Read => wa::web_message_info::Status::READ as i32,
        ReceiptType::Played => wa::web_message_info::Status::PLAYED as i32,
        ReceiptType::ReadSelf | ReceiptType::PlayedSelf => {
            // Read on another of our devices — up to the covered messages.
            // WA read state is "read up to X": the boundary is the newest
            // covered row (falling back to the receipt's own timestamp).
            use schema::messages::dsl;
            let covered_max: Option<Option<i64>> = dsl::messages
                .filter(
                    dsl::device_id
                        .eq(device_id)
                        .and(dsl::chat_jid.eq(&chat))
                        .and(dsl::msg_id.eq_any(&receipt.message_ids)),
                )
                .select(diesel::dsl::max(dsl::timestamp_ms))
                .first(conn)
                .optional()?;
            let boundary_ms = covered_max.flatten().unwrap_or(ts_ms);
            ensure_chat(conn, device_id, &chat)?;
            // Fold into the monotonic read state: the watermark stops SHORT
            // of the boundary instant (coverage there is keyed by the
            // receipt's ids — timestamps collide at wire granularity), and
            // the named ids ride along so a covered row materialized later
            // stays read while an unlisted same-instant sibling still badges.
            // A stale replay changes nothing and is skipped outright.
            let Some(state) = advance_read_state(
                conn,
                device_id,
                &chat,
                boundary_ms - 1,
                &receipt.message_ids,
            )?
            else {
                return Ok(());
            };
            let unread = count_unread(conn, device_id, &chat, &state)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::unread_count.eq(unread))
                .execute(conn)?;
            cs.chats = true;
            return Ok(());
        }
        _ => return Ok(()),
    };

    let user = receipt.source.sender.to_string();
    for msg_id in &receipt.message_ids {
        // Peer receipts only ever advance the delivery state of our own
        // messages, and never backwards.
        diesel::update(
            message_row(device_id, &chat, msg_id).filter(
                schema::messages::from_me
                    .eq(true)
                    .and(schema::messages::status.lt(status)),
            ),
        )
        .set(schema::messages::status.eq(status))
        .execute(conn)?;

        // Derived from the JID: the library's receipt parser leaves
        // `source.is_group` defaulted, so the flag can't be trusted here.
        if receipt.source.chat.is_group() {
            use schema::message_receipts::dsl;
            diesel::insert_into(dsl::message_receipts)
                .values((
                    dsl::device_id.eq(device_id),
                    dsl::chat_jid.eq(&chat),
                    dsl::msg_id.eq(msg_id),
                    dsl::user_jid.eq(&user),
                    dsl::receipt_type.eq(status),
                    dsl::ts_ms.eq(ts_ms),
                ))
                .on_conflict_do_nothing()
                .execute(conn)?;
            // Existing row: advance only (a late "delivered" must not undo "read").
            diesel::update(
                dsl::message_receipts.filter(
                    dsl::device_id
                        .eq(device_id)
                        .and(dsl::chat_jid.eq(&chat))
                        .and(dsl::msg_id.eq(msg_id))
                        .and(dsl::user_jid.eq(&user))
                        .and(dsl::receipt_type.lt(status)),
                ),
            )
            .set((dsl::receipt_type.eq(status), dsl::ts_ms.eq(ts_ms)))
            .execute(conn)?;
        }
    }
    cs.message_chats.insert(chat);
    Ok(())
}

fn apply_server_ack(
    conn: &mut SqliteConnection,
    device_id: i32,
    ack: &wacore::types::events::ServerAck,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    // Acks cover every stanza class; only message acks map to a stored row.
    if ack.class.as_deref() != Some("message") || ack.error.is_some() {
        return Ok(());
    }
    use schema::messages::dsl;
    let updated = diesel::update(
        dsl::messages.filter(
            dsl::device_id
                .eq(device_id)
                .and(dsl::msg_id.eq(&ack.id))
                .and(dsl::from_me.eq(true))
                .and(dsl::status.lt(wa::web_message_info::Status::SERVER_ACK as i32)),
        ),
    )
    .set(dsl::status.eq(wa::web_message_info::Status::SERVER_ACK as i32))
    .execute(conn)?;
    if updated > 0 {
        // Acks usually carry the chat; when they don't, resolve it from the
        // row we just updated so consumers still get invalidated.
        let chat = match &ack.from {
            Some(from) => Some(from.to_string()),
            None => dsl::messages
                .filter(dsl::device_id.eq(device_id).and(dsl::msg_id.eq(&ack.id)))
                .select(dsl::chat_jid)
                .first(conn)
                .optional()?,
        };
        if let Some(chat) = chat {
            cs.message_chats.insert(chat);
        }
    }
    Ok(())
}

fn apply_history_sync(
    conn: &mut SqliteConnection,
    device_id: i32,
    lazy: &wacore::types::events::LazyHistorySync,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    let mut stream = lazy.stream();
    loop {
        let conv = match stream.next_conversation() {
            Ok(Some(conv)) => conv,
            Ok(None) => break,
            Err(e) => {
                // Framing/zlib failure: the stream position is gone, the rest
                // of this chunk is unreadable (per-conversation decode errors
                // are skipped inside the stream, not surfaced here).
                warn!("chat-store: history sync chunk framing broken, aborting chunk: {e}");
                return Ok(());
            }
        };
        apply_history_conversation(conn, device_id, &conv, cs)?;
    }
    if stream.skipped_conversations() > 0 {
        warn!(
            "chat-store: history sync skipped {} undecodable conversation(s)",
            stream.skipped_conversations()
        );
    }
    match stream.remainder() {
        Ok(rest) => {
            for pushname in &rest.pushnames {
                if let (Some(jid), Some(name)) = (&pushname.id, &pushname.pushname) {
                    upsert_contact_push_name(conn, device_id, jid, name)?;
                    cs.contacts = true;
                }
            }
        }
        Err(e) => warn!("chat-store: history sync remainder unreadable: {e}"),
    }
    Ok(())
}

fn apply_history_conversation(
    conn: &mut SqliteConnection,
    device_id: i32,
    conv: &wa::Conversation,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    let chat = conv.id.as_str();
    let last_ts_ms = conv
        .conversation_timestamp
        .map(|s| (s as i64).saturating_mul(1000))
        .unwrap_or(0);

    {
        use schema::chats::dsl;
        let name = conv.name.as_deref().or(conv.display_name.as_deref());
        diesel::insert_into(dsl::chats)
            .values((
                dsl::device_id.eq(device_id),
                dsl::jid.eq(chat),
                dsl::name.eq(name),
                dsl::last_message_ts.eq(last_ts_ms),
                dsl::unread_count.eq(conv.unread_count.unwrap_or(0) as i32),
                // Wire values are unix SECONDS; the columns (and the live
                // app-state paths) are milliseconds.
                dsl::pinned_at.eq(conv
                    .pinned
                    .map(|p| (p as i64).saturating_mul(1000))
                    .filter(|&p| p > 0)),
                dsl::muted_until.eq(conv
                    .mute_end_time
                    .map(|m| (m as i64).saturating_mul(1000))
                    .filter(|&m| m > 0)),
                dsl::archived.eq(conv.archived.unwrap_or(false)),
                dsl::ephemeral_expiration.eq(conv.ephemeral_expiration.map(|e| e as i32)),
            ))
            .on_conflict((dsl::device_id, dsl::jid))
            .do_update()
            // Live rows already track unread/mute/pin; history only refreshes
            // identity + activity floor.
            .set((
                dsl::name.eq(name),
                dsl::last_message_ts.eq(diesel::dsl::sql::<diesel::sql_types::BigInt>(
                    "MAX(last_message_ts, excluded.last_message_ts)",
                )),
            ))
            .execute(conn)?;
    }

    for hist_msg in &conv.messages {
        let Some(wmi) = hist_msg.message.as_option() else {
            continue;
        };
        apply_history_message(conn, device_id, chat, wmi, cs)?;
    }
    // Backfill the denormalized preview from the newest materialized row, so a
    // freshly-paired client's chat list isn't blank until live traffic.
    recompute_chat_preview(conn, device_id, chat)?;
    cs.chats = true;
    cs.message_chats.insert(chat.to_string());
    Ok(())
}

fn apply_history_message(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    wmi: &wa::WebMessageInfo,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    let Some(key) = wmi.key.as_option() else {
        return Ok(());
    };
    let Some(msg_id) = key.id.as_deref() else {
        return Ok(());
    };
    let from_me = key.from_me.unwrap_or(false);
    let sender = wmi
        .participant
        .as_deref()
        .or(key.participant.as_deref())
        .unwrap_or(if from_me { "" } else { chat });
    let ts_ms = wmi
        .message_timestamp
        .map(|s| (s as i64).saturating_mul(1000))
        .unwrap_or(0);

    if let Some(name) = wmi.push_name.as_deref()
        && !name.is_empty()
        && !from_me
        && !sender.is_empty()
    {
        upsert_contact_push_name(conn, device_id, sender, name)?;
        cs.contacts = true;
    }

    if let Some(message) = wmi.message.as_option() {
        match classify(message) {
            MessageOp::Store { kind, text } => {
                let _ = insert_message(
                    conn,
                    device_id,
                    NewMessage {
                        chat_jid: chat,
                        msg_id,
                        sender_jid: sender,
                        from_me,
                        timestamp_ms: ts_ms,
                        kind,
                        text: text.as_deref(),
                        proto: Some(&message.encode_to_vec()),
                        status: wmi
                            .status
                            .map(|s| s as i32)
                            .unwrap_or(wa::web_message_info::Status::PENDING as i32),
                        starred: wmi.starred.unwrap_or(false),
                        // History is the stale copy: live rows win.
                        overwrite: false,
                    },
                )?;
            }
            MessageOp::Reaction { target_id, emoji } => {
                apply_reaction(conn, device_id, chat, &target_id, sender, &emoji, ts_ms)?;
            }
            MessageOp::Edit {
                target_id,
                new_text,
                new_kind,
                new_proto,
            } => {
                if apply_edit(
                    conn,
                    device_id,
                    chat,
                    &target_id,
                    new_text.as_deref(),
                    new_kind,
                    &new_proto,
                    ts_ms,
                )? {
                    cs.chats = true;
                }
            }
            MessageOp::Revoke {
                target_id,
                target_from_me,
                target_participant,
            } => {
                if apply_revoke(
                    conn,
                    device_id,
                    chat,
                    &target_id,
                    target_participant.as_deref().unwrap_or(sender),
                    target_from_me,
                    ts_ms,
                )? {
                    cs.chats = true;
                }
            }
            MessageOp::Ignore => {}
        }
    }

    // Reactions the server aggregated onto the target message.
    for reaction in &wmi.reactions {
        let Some(text) = reaction.text.as_deref() else {
            continue;
        };
        let reactor = reaction
            .key
            .as_option()
            .and_then(|k| {
                if k.from_me.unwrap_or(false) {
                    Some("")
                } else {
                    k.participant.as_deref().or(k.remote_jid.as_deref())
                }
            })
            .unwrap_or("");
        let reaction_ts = reaction.sender_timestamp_ms.unwrap_or(ts_ms);
        apply_reaction(conn, device_id, chat, msg_id, reactor, text, reaction_ts)?;
    }
    Ok(())
}

struct NewMessage<'a> {
    chat_jid: &'a str,
    msg_id: &'a str,
    sender_jid: &'a str,
    from_me: bool,
    timestamp_ms: i64,
    kind: &'a str,
    text: Option<&'a str>,
    proto: Option<&'a [u8]>,
    status: i32,
    starred: bool,
    /// Live redeliveries refresh content in place (PDO recovery replaces an
    /// `undecryptable` placeholder); history-sync copies never clobber live rows.
    overwrite: bool,
}

/// What actually happened to the row, so callers can gate side effects
/// (unread counting, chat-preview bumps) on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoredRow {
    /// A new row was inserted.
    Inserted,
    /// The id existed; its content was refreshed in place (`overwrite`).
    Refreshed,
    /// The id existed and was left untouched (history duplicate, or a revoked
    /// tombstone that a redelivery must not resurrect or re-surface).
    Skipped,
}

/// A refresh never touches `revoked` (a tombstone outranks any stale
/// redelivery) and never crosses senders: message ids are SENDER-chosen, so a
/// same-id row from a different sender must not rewrite the original's content
/// (adversarial id reuse would otherwise alter someone else's message in the
/// local history). Both cases report [`StoredRow::Skipped`].
fn insert_message(
    conn: &mut SqliteConnection,
    device_id: i32,
    new: NewMessage<'_>,
) -> QueryResult<StoredRow> {
    use schema::messages::dsl;
    let values = (
        dsl::device_id.eq(device_id),
        dsl::chat_jid.eq(new.chat_jid),
        dsl::msg_id.eq(new.msg_id),
        dsl::sender_jid.eq(new.sender_jid),
        dsl::from_me.eq(new.from_me),
        dsl::timestamp_ms.eq(new.timestamp_ms),
        dsl::kind.eq(new.kind),
        dsl::text_content.eq(new.text),
        dsl::proto.eq(new.proto),
        dsl::status.eq(new.status),
        dsl::starred.eq(new.starred),
    );
    let inserted = diesel::insert_into(dsl::messages)
        .values(values)
        .on_conflict_do_nothing()
        .execute(conn)?
        > 0;
    if inserted {
        return Ok(StoredRow::Inserted);
    }
    if new.overwrite {
        let refreshed = diesel::update(
            message_row(device_id, new.chat_jid, new.msg_id)
                .filter(dsl::revoked.eq(false))
                .filter(dsl::sender_jid.eq(new.sender_jid))
                // A redelivery carries the PRE-edit original; an edited row
                // must keep its newer content.
                .filter(dsl::edited_at_ms.is_null()),
        )
        .set((
            dsl::kind.eq(new.kind),
            dsl::text_content.eq(new.text),
            dsl::proto.eq(new.proto),
        ))
        .execute(conn)?;
        if refreshed > 0 {
            return Ok(StoredRow::Refreshed);
        }
    }
    Ok(StoredRow::Skipped)
}

/// Refresh a chat's activity row for a message at `ts_ms`: creates the row if
/// missing, advances ordering/preview only for newer messages, and bumps the
/// unread counter by `unread_delta` (unless manually marked unread).
/// One message's contribution to its chat's denormalized row.
struct ChatBump<'a> {
    msg_id: &'a str,
    ts_ms: i64,
    preview: Option<&'a str>,
    kind: Option<&'a str>,
    unread_delta: i32,
}

fn bump_chat(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    bump: ChatBump<'_>,
) -> QueryResult<()> {
    use schema::chats::dsl;
    ensure_chat(conn, device_id, chat)?;
    // Ordering timestamp is monotonic on its own...
    diesel::update(chat_row(device_id, chat).filter(dsl::last_message_ts.le(bump.ts_ms)))
        .set(dsl::last_message_ts.eq(bump.ts_ms))
        .execute(conn)?;
    // ...but the preview belongs to the newest row by the FULL (timestamp_ms,
    // msg_id) order — a same-millisecond sibling applied later must not win.
    refresh_preview_if_latest(conn, device_id, chat, bump.msg_id, bump.preview, bump.kind)?;
    if bump.unread_delta != 0 {
        // An old row materialized late (offline drain) that a read already
        // covered must not badge.
        let state = read_state(conn, device_id, chat)?;
        if !state.covers(bump.ts_ms, bump.msg_id) {
            diesel::update(chat_row(device_id, chat).filter(dsl::unread_count.ge(0)))
                .set(dsl::unread_count.eq(dsl::unread_count + bump.unread_delta))
                .execute(conn)?;
        }
    }
    Ok(())
}

fn ensure_chat(conn: &mut SqliteConnection, device_id: i32, chat: &str) -> QueryResult<()> {
    use schema::chats::dsl;
    diesel::insert_into(dsl::chats)
        .values((dsl::device_id.eq(device_id), dsl::jid.eq(chat)))
        .on_conflict_do_nothing()
        .execute(conn)?;
    Ok(())
}

fn upsert_contact_push_name(
    conn: &mut SqliteConnection,
    device_id: i32,
    jid: &str,
    push_name: &str,
) -> QueryResult<()> {
    use schema::contacts::dsl;
    diesel::insert_into(dsl::contacts)
        .values((
            dsl::device_id.eq(device_id),
            dsl::jid.eq(jid),
            dsl::push_name.eq(push_name),
        ))
        .on_conflict((dsl::device_id, dsl::jid))
        .do_update()
        .set(dsl::push_name.eq(push_name))
        .execute(conn)?;
    Ok(())
}

fn upsert_contact_names(
    conn: &mut SqliteConnection,
    device_id: i32,
    jid: &str,
    full_name: Option<&str>,
    first_name: Option<&str>,
) -> QueryResult<()> {
    use schema::contacts::dsl;
    diesel::insert_into(dsl::contacts)
        .values((
            dsl::device_id.eq(device_id),
            dsl::jid.eq(jid),
            dsl::full_name.eq(full_name),
            dsl::first_name.eq(first_name),
        ))
        .on_conflict((dsl::device_id, dsl::jid))
        .do_update()
        .set((dsl::full_name.eq(full_name), dsl::first_name.eq(first_name)))
        .execute(conn)?;
    Ok(())
}

/// Delete a chat's message rows (and their reactions/receipts). With
/// `delete_starred = false`, starred messages and their satellites survive.
fn delete_chat_rows(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    delete_starred: bool,
    bound: Option<&RangeBound>,
) -> QueryResult<()> {
    use schema::messages::dsl as m;
    // A ranged action only covers messages up to its boundary; rows we
    // materialized after it (live/offline traffic) survive. With a keyed
    // boundary, same-second siblings the action does not name survive too.
    match bound {
        None => {
            let mut query = diesel::delete(
                m::messages.filter(m::device_id.eq(device_id).and(m::chat_jid.eq(chat))),
            )
            .into_boxed();
            if !delete_starred {
                query = query.filter(m::starred.eq(false));
            }
            query.execute(conn)?;
        }
        Some(bound) => {
            let mut query = diesel::delete(
                m::messages.filter(m::device_id.eq(device_id).and(m::chat_jid.eq(chat))),
            )
            .into_boxed();
            if !delete_starred {
                query = query.filter(m::starred.eq(false));
            }
            match &bound.keys {
                None => {
                    query = query.filter(m::timestamp_ms.le(bound.second_end_ms));
                    query.execute(conn)?;
                }
                Some(keys) => {
                    // Everything strictly before the boundary second...
                    query = query.filter(m::timestamp_ms.lt(bound.second_start_ms));
                    query.execute(conn)?;
                    // ...plus the boundary rows the action names explicitly.
                    let mut keyed = diesel::delete(
                        m::messages.filter(m::device_id.eq(device_id).and(m::chat_jid.eq(chat))),
                    )
                    .into_boxed();
                    if !delete_starred {
                        keyed = keyed.filter(m::starred.eq(false));
                    }
                    keyed
                        .filter(m::timestamp_ms.le(bound.second_end_ms))
                        .filter(m::msg_id.eq_any(keys))
                        .execute(conn)?;
                }
            }
        }
    }
    // Satellites of messages that no longer exist.
    diesel::sql_query(
        "DELETE FROM reactions WHERE device_id = ? AND chat_jid = ? AND msg_id NOT IN \
         (SELECT msg_id FROM messages WHERE device_id = ? AND chat_jid = ?)",
    )
    .bind::<diesel::sql_types::Integer, _>(device_id)
    .bind::<diesel::sql_types::Text, _>(chat)
    .bind::<diesel::sql_types::Integer, _>(device_id)
    .bind::<diesel::sql_types::Text, _>(chat)
    .execute(conn)?;
    diesel::sql_query(
        "DELETE FROM message_receipts WHERE device_id = ? AND chat_jid = ? AND msg_id NOT IN \
         (SELECT msg_id FROM messages WHERE device_id = ? AND chat_jid = ?)",
    )
    .bind::<diesel::sql_types::Integer, _>(device_id)
    .bind::<diesel::sql_types::Text, _>(chat)
    .bind::<diesel::sql_types::Integer, _>(device_id)
    .bind::<diesel::sql_types::Text, _>(chat)
    .execute(conn)?;
    Ok(())
}

fn remaining_messages(conn: &mut SqliteConnection, device_id: i32, chat: &str) -> QueryResult<i64> {
    use schema::messages::dsl;
    dsl::messages
        .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq(chat)))
        .count()
        .get_result(conn)
}

type ChatRowFilter<'a> = diesel::dsl::Filter<
    schema::chats::table,
    diesel::dsl::And<
        diesel::dsl::Eq<schema::chats::device_id, i32>,
        diesel::dsl::Eq<schema::chats::jid, &'a str>,
    >,
>;

fn chat_row(device_id: i32, chat: &str) -> ChatRowFilter<'_> {
    schema::chats::table.filter(
        schema::chats::device_id
            .eq(device_id)
            .and(schema::chats::jid.eq(chat)),
    )
}

type MessageRowFilter<'a> = diesel::dsl::Filter<
    schema::messages::table,
    diesel::dsl::And<
        diesel::dsl::And<
            diesel::dsl::Eq<schema::messages::device_id, i32>,
            diesel::dsl::Eq<schema::messages::chat_jid, &'a str>,
        >,
        diesel::dsl::Eq<schema::messages::msg_id, &'a str>,
    >,
>;

fn message_row<'a>(device_id: i32, chat: &'a str, msg_id: &'a str) -> MessageRowFilter<'a> {
    schema::messages::table.filter(
        schema::messages::device_id
            .eq(device_id)
            .and(schema::messages::chat_jid.eq(chat))
            .and(schema::messages::msg_id.eq(msg_id)),
    )
}
