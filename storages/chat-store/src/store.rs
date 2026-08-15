//! The store itself: a write-behind materializer over the client's event
//! stream plus the public write API. All writes funnel through one writer task
//! (one transaction per drained batch), so event order is preserved and fan-in
//! bursts don't pay per-event commit costs.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::str::FromStr;
use std::sync::Arc;

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
use crate::materialize::{
    KIND_UNDECRYPTABLE, MessageOp, classify, extract_text, message_kind, unavailable_kind,
};
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
    Edit {
        chat: Jid,
        target_id: String,
        proto: Vec<u8>,
        kind: &'static str,
        text: Option<String>,
        timestamp_ms: i64,
    },
    Revoke {
        chat: Jid,
        target_id: String,
        timestamp_ms: i64,
    },
    Reaction {
        chat: Jid,
        target_id: String,
        target_from_me: bool,
        target_participant: Option<String>,
        emoji: String,
        timestamp_ms: i64,
    },
    Reconcile(Jid),
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
/// let _chat_subscription = client.subscribe_handler(chat_store.handler());
/// let mut changes = chat_store.subscribe();
/// ```
pub struct ChatStore {
    db: SharedSqlite,
    device_id: i32,
    tx: mpsc::UnboundedSender<WriterMsg>,
    changes: broadcast::Sender<StoreChange>,
    skip_hook_committed: Arc<std::sync::atomic::AtomicBool>,
}

struct ChatStoreHandler {
    tx: mpsc::UnboundedSender<WriterMsg>,
    skip_hook_committed: Arc<std::sync::atomic::AtomicBool>,
}

impl EventHandler for ChatStoreHandler {
    fn handle_event(&self, event: Arc<Event>) {
        // `hook_committed` says a durability hook committed the batch — NOT
        // that it committed it *here*. A hook that persists somewhere else
        // entirely is just as common, and for that host this store is the only
        // materializer; skipping would silently lose acknowledged messages.
        // Only the host knows which it runs, so the skip is opt-in and this
        // load is the answer it gave (see `skip_hook_committed_batches`).
        if self
            .skip_hook_committed
            .load(std::sync::atomic::Ordering::Relaxed)
            && event
                .as_messages()
                .is_some_and(|batch| batch.hook_committed)
        {
            return;
        }
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
            skip_hook_committed: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        });
        tokio::spawn(writer_loop(db, device_id, rx, changes));
        Ok(this)
    }

    /// Declare that this client's inbound durability hook already materializes
    /// into THIS store, so batches it committed can be skipped here.
    ///
    /// Off by default, and deliberately not inferred: a batch's
    /// `hook_committed` marker says a hook committed it, not that the hook
    /// wrote it *here*. A host whose hook persists elsewhere — its own
    /// database, a queue, an audit log — still needs this store to materialize
    /// every batch, and skipping on the marker alone would silently drop
    /// acknowledged messages out of its history, previews and subscriptions.
    /// Only the host knows which arrangement it runs.
    ///
    /// Turn it on when the hook feeds this store and you would otherwise pay
    /// for every message twice: the inbound path overwrites, so the second
    /// pass is a full UPDATE of the proto blob plus an FTS delete+insert plus
    /// another chat bump, and it doubles the `StoreChange` fan-out, so every
    /// subscriber re-queries every surface twice per message.
    ///
    /// Takes effect on the next event; handlers already handed out observe it.
    pub fn skip_hook_committed_batches(&self, skip: bool) {
        self.skip_hook_committed
            .store(skip, std::sync::atomic::Ordering::Relaxed);
    }

    /// Event handler to register on the client. The store keeps working if the
    /// handler outlives it (events are then dropped), and vice versa.
    pub fn handler(&self) -> Arc<dyn EventHandler> {
        Arc::new(ChatStoreHandler {
            tx: self.tx.clone(),
            skip_hook_committed: Arc::clone(&self.skip_hook_committed),
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
    /// and is lifted by acks/receipts. `timestamp` is the optimistic display
    /// time; a positive message ack replaces it with the server's `t` when
    /// available and refreshes the conversation order.
    ///
    /// `chat` may be either of a 1:1 peer's identities (phone number or LID):
    /// the row is stored on the peer's one thread regardless — an existing
    /// thread keeps its key, a brand-new chat with a known LID mapping is
    /// keyed by the LID (WA Web behavior) — and every query resolves the
    /// alias, so reads by either identity keep working.
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
                proto: waproto::codec::message_to_vec(message),
                kind: message_kind(base),
                text: extract_text(base),
                timestamp_ms: timestamp.timestamp_millis(),
            })
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))
    }

    /// Record an edit this client just sent for one of its own messages.
    ///
    /// This is the local counterpart of an inbound `MESSAGE_EDIT`: it updates
    /// the existing row in place (or creates the same out-of-order placeholder
    /// as the event path), preserving the edit's timestamp ordering and
    /// tombstone rules. Goes through the writer queue; use
    /// [`flush`](Self::flush) to await completion.
    pub fn record_edit(
        &self,
        chat: &Jid,
        target_id: &str,
        new_content: &wa::Message,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let base = wacore::proto_helpers::MessageExt::get_base_message(new_content);
        self.tx
            .send(WriterMsg::Edit {
                chat: chat.clone(),
                target_id: target_id.to_owned(),
                proto: waproto::codec::message_to_vec(new_content),
                kind: message_kind(base),
                text: extract_text(base),
                timestamp_ms: timestamp.timestamp_millis(),
            })
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))
    }

    /// Record a sender revoke this client just sent for one of its own
    /// messages.
    ///
    /// The target becomes a tombstone and cannot be resurrected by a delayed
    /// content delivery or edit. Goes through the writer queue; use
    /// [`flush`](Self::flush) to await completion.
    pub fn record_revoke(
        &self,
        chat: &Jid,
        target_id: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        self.tx
            .send(WriterMsg::Revoke {
                chat: chat.clone(),
                target_id: target_id.to_owned(),
                timestamp_ms: timestamp.timestamp_millis(),
            })
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))
    }

    /// Record a reaction this client just sent. An empty `emoji` removes this
    /// client's existing reaction, matching the inbound event semantics.
    ///
    /// `target` is the same message key passed to `Client::send_reaction` and
    /// must contain an id. If no stored message matches its authorship, the
    /// queued reaction is a no-op. Goes through the writer queue; use
    /// [`flush`](Self::flush) to await completion.
    pub fn record_reaction(
        &self,
        chat: &Jid,
        target: &wa::MessageKey,
        emoji: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        let target_id = target.id.clone().ok_or_else(|| {
            ChatStoreError::Store(StoreError::Validation(
                "reaction target key missing id".into(),
            ))
        })?;
        self.tx
            .send(WriterMsg::Reaction {
                chat: chat.clone(),
                target_id,
                target_from_me: target.from_me.unwrap_or(false),
                target_participant: target.participant.clone(),
                emoji: emoji.to_owned(),
                timestamp_ms: timestamp.timestamp_millis(),
            })
            .map_err(|_| ChatStoreError::Store(StoreError::Validation("writer stopped".into())))
    }

    /// Reconcile a 1:1 peer's PN- and LID-keyed rows into a single thread.
    ///
    /// Receipts dropped under the wrong identity (before this crate resolved
    /// PN/LID aliases) left some stores with a split pair: a populated chat
    /// under the phone-number key plus a stray `@lid` twin. Live traffic for
    /// the peer now heals such a pair on its own; this makes the repair
    /// on-demand for embedders that want it eagerly. Idempotent — a peer with
    /// one thread (or no LID mapping yet) is a no-op. Goes through the writer
    /// queue; use [`flush`](Self::flush) to await completion.
    pub fn reconcile_chat(&self, chat: &Jid) -> Result<()> {
        self.tx
            .send(WriterMsg::Reconcile(chat.clone()))
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
pub(crate) struct ChangeSet {
    pub(crate) chats: bool,
    pub(crate) contacts: bool,
    pub(crate) message_chats: BTreeSet<String>,
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
    // Outlives every batch: the insert that answers a deferred ack is by
    // definition in a later one. Shared with the blocking closure rather than
    // moved into it, so a panic inside the transaction cannot carry the queue
    // off with it — the acks a dying batch deferred are exactly the ones with
    // no other record left.
    let deferred_acks = Arc::new(std::sync::Mutex::new(DeferredAcks::default()));
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
            // Snapshot what a failure has to fold back onto. Deferred acks are
            // rare, so the usual clone is of an empty queue.
            let pre_batch = {
                let mut acks = lock_deferred_acks(&deferred_acks);
                acks.begin_batch();
                acks.clone()
            };
            let shared = Arc::clone(&deferred_acks);
            let result = db
                .run(move |conn| {
                    let mut deferred = lock_deferred_acks(&shared);
                    conn.transaction(|conn| {
                        let mut cs = ChangeSet::default();
                        for msg in &batch {
                            apply_writer_msg(conn, device_id, msg, &mut cs, &mut deferred)?;
                        }
                        Ok(cs)
                    })
                    .map_err(db_err)
                })
                .await;
            match result {
                Ok(cs) => emit_changes(&changes, cs),
                // Nothing committed, by any route: the transaction rolled back,
                // or the pool/task failed before or during it. The queue is
                // reachable either way, so fold it back the same way — undoing
                // what the batch consumed, keeping what it deferred.
                Err(e) => {
                    let mut acks = lock_deferred_acks(&deferred_acks);
                    *acks = std::mem::take(&mut *acks).rolled_back(pre_batch);
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

/// Take the deferred-ack queue, poisoned or not.
///
/// Poisoning here means the writer's transaction panicked mid-batch, and the
/// contents are precisely what has to be recovered in that case — the acks it
/// had deferred have no other record. Refusing to read them would turn the
/// panic into the silent loss the queue exists to prevent.
fn lock_deferred_acks(
    acks: &std::sync::Mutex<DeferredAcks>,
) -> std::sync::MutexGuard<'_, DeferredAcks> {
    acks.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
    deferred: &mut DeferredAcks,
) -> QueryResult<()> {
    match msg {
        WriterMsg::Event(event) => apply_event(conn, device_id, event, cs, deferred),
        WriterMsg::Reconcile(chat) => {
            let wire = chat.to_string();
            if let Some(alt) = crate::lid::counterpart_chat_key(conn, device_id, &wire)? {
                crate::lid::merge_split_chat(conn, device_id, &wire, &alt, cs)?;
            }
            Ok(())
        }
        WriterMsg::Outgoing {
            chat,
            msg_id,
            proto,
            kind,
            text,
            timestamp_ms,
        } => {
            let chat_str = route_writer_chat(conn, device_id, chat, cs)?;
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
                // The row this send's ack was waiting for now exists. Applying
                // it here also corrects the optimistic timestamp we just wrote
                // to the server's, before anything renders the row.
                if let Some(ack) = deferred.take_matching(
                    msg_id,
                    &chat_str,
                    wacore::time::now_utc().timestamp_millis(),
                ) && let AckApplied::Deferrable(_) = apply_server_ack(conn, device_id, &ack, cs)?
                {
                    // The row exists, so this should not happen; say so rather
                    // than let the ack vanish the way it used to.
                    warn!(
                        target: "ChatStore/Ack",
                        "Held ack for {msg_id} matched no row even after its insert"
                    );
                }
            }
            cs.message_chats.insert(chat_str);
            Ok(())
        }
        WriterMsg::Edit {
            chat,
            target_id,
            proto,
            kind,
            text,
            timestamp_ms,
        } => {
            let chat_str = route_writer_chat(conn, device_id, chat, cs)?;
            if !local_target_collides_with_peer(conn, device_id, &chat_str, target_id)?
                && apply_edit(
                    conn,
                    device_id,
                    &chat_str,
                    target_id,
                    "",
                    true,
                    text.as_deref(),
                    kind,
                    proto,
                    *timestamp_ms,
                )?
            {
                cs.chats = true;
            }
            cs.message_chats.insert(chat_str);
            Ok(())
        }
        WriterMsg::Revoke {
            chat,
            target_id,
            timestamp_ms,
        } => {
            let chat_str = route_writer_chat(conn, device_id, chat, cs)?;
            if !local_target_collides_with_peer(conn, device_id, &chat_str, target_id)?
                && apply_revoke(
                    conn,
                    device_id,
                    &chat_str,
                    target_id,
                    "",
                    true,
                    *timestamp_ms,
                )?
            {
                cs.chats = true;
            }
            cs.message_chats.insert(chat_str);
            Ok(())
        }
        WriterMsg::Reaction {
            chat,
            target_id,
            target_from_me,
            target_participant,
            emoji,
            timestamp_ms,
        } => {
            let chat_str = route_writer_chat(conn, device_id, chat, cs)?;
            if local_reaction_target_matches(
                conn,
                device_id,
                &chat_str,
                target_id,
                *target_from_me,
                target_participant.as_deref(),
            )? {
                // Own reactors are stored as the empty JID, the same sentinel
                // used by history sync for key.from_me reactions.
                apply_reaction(
                    conn,
                    device_id,
                    &chat_str,
                    target_id,
                    "",
                    emoji,
                    *timestamp_ms,
                )?;
            }
            cs.message_chats.insert(chat_str);
            Ok(())
        }
        WriterMsg::Flush(_) => Ok(()),
    }
}

fn route_writer_chat(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &Jid,
    cs: &mut ChangeSet,
) -> QueryResult<String> {
    let wire = chat.to_string();
    let routed = crate::lid::route_chat_key(conn, device_id, &wire, cs)?;
    if routed != wire {
        cs.message_chats.insert(wire);
    }
    Ok(routed)
}

/// A local amendment may create an own-message placeholder when its target is
/// absent, but an existing peer row with the same sender-chosen id belongs to
/// a different message and must remain untouched.
fn local_target_collides_with_peer(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    target_id: &str,
) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        message_row(device_id, chat, target_id).filter(schema::messages::from_me.eq(false)),
    ))
    .get_result(conn)
}

/// Match the full target identity, not just its sender-chosen id. Device
/// suffixes and known PN/LID aliases normalize before participant comparison.
fn local_reaction_target_matches(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    target_id: &str,
    target_from_me: bool,
    target_participant: Option<&str>,
) -> QueryResult<bool> {
    let target: Option<(bool, String)> = message_row(device_id, chat, target_id)
        .select((schema::messages::from_me, schema::messages::sender_jid))
        .first(conn)
        .optional()?;
    let Some((stored_from_me, stored_sender)) = target else {
        return Ok(false);
    };
    if stored_from_me != target_from_me {
        return Ok(false);
    }
    if target_from_me {
        return Ok(true);
    }
    let Some(participant) = target_participant else {
        let needs_participant = Jid::from_str(chat).is_ok_and(|jid| {
            jid.is_group() || jid.is_status_broadcast() || jid.is_broadcast_list()
        });
        return Ok(!needs_participant);
    };
    let (Ok(stored), Ok(target)) = (Jid::from_str(&stored_sender), Jid::from_str(participant))
    else {
        return Ok(stored_sender == participant);
    };
    let stored = stored.to_non_ad_string();
    let target = target.to_non_ad_string();
    if stored == target {
        return Ok(true);
    }
    Ok(
        crate::lid::counterpart_chat_key(conn, device_id, &stored)?.as_deref()
            == Some(target.as_str()),
    )
}

fn apply_event(
    conn: &mut SqliteConnection,
    device_id: i32,
    event: &Event,
    cs: &mut ChangeSet,
    deferred: &mut DeferredAcks,
) -> QueryResult<()> {
    match event {
        Event::Messages(batch) => {
            for inbound in batch.iter() {
                apply_inbound(conn, device_id, inbound, cs)?;
            }
            Ok(())
        }
        Event::Receipt(receipt) => apply_receipt(conn, device_id, receipt, cs),
        Event::ServerAck(ack) => {
            if let AckApplied::Deferrable(chat) = apply_server_ack(conn, device_id, ack, cs)? {
                deferred.defer(ack, chat, wacore::time::now_utc().timestamp_millis());
            }
            Ok(())
        }
        Event::UndecryptableMessage(undec) => {
            let kind = unavailable_kind(undec.unavailable_type).unwrap_or(KIND_UNDECRYPTABLE);
            let wire = undec.info.source.chat.to_string();
            let chat = crate::lid::route_chat_key(conn, device_id, &wire, cs)?;
            if chat != wire {
                cs.message_chats.insert(wire);
            }
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
                    kind,
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
                        kind: Some(kind),
                        unread_delta: i32::from(!undec.info.source.is_from_me),
                    },
                )?;
                cs.chats = true;
            }
            cs.message_chats.insert(chat);
            Ok(())
        }
        Event::HistorySync(lazy) => apply_history_sync(conn, device_id, lazy, cs),
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
            let chat = crate::lid::route_chat_key(conn, device_id, &update.jid.to_string(), cs)?;
            ensure_chat(conn, device_id, &chat)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::pinned_at.eq(pinned_at))
                .execute(conn)?;
            cs.chats = true;
            Ok(())
        }
        Event::MuteUpdate(update) => {
            let muted_until = if update.action.muted.unwrap_or(false) {
                // Absent or non-positive (WA Web sends -1 for indefinite,
                // this crate's own mute_chat() included) = muted forever.
                Some(
                    update
                        .action
                        .mute_end_timestamp
                        .filter(|&ts| ts > 0)
                        .unwrap_or(i64::MAX),
                )
            } else {
                None
            };
            let chat = crate::lid::route_chat_key(conn, device_id, &update.jid.to_string(), cs)?;
            ensure_chat(conn, device_id, &chat)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::muted_until.eq(muted_until))
                .execute(conn)?;
            cs.chats = true;
            Ok(())
        }
        Event::ArchiveUpdate(update) => {
            let chat = crate::lid::route_chat_key(conn, device_id, &update.jid.to_string(), cs)?;
            ensure_chat(conn, device_id, &chat)?;
            diesel::update(chat_row(device_id, &chat))
                .set(schema::chats::archived.eq(update.action.archived.unwrap_or(false)))
                .execute(conn)?;
            cs.chats = true;
            Ok(())
        }
        Event::MarkChatAsReadUpdate(update) => {
            let chat = crate::lid::route_chat_key(conn, device_id, &update.jid.to_string(), cs)?;
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
                match advanced {
                    Some(state) => {
                        let unread = count_unread(conn, device_id, &chat, &state)?;
                        diesel::update(chat_row(device_id, &chat))
                            .set(schema::chats::unread_count.eq(unread))
                            .execute(conn)?;
                    }
                    // Cursor didn't move (re-reading an already-read chat),
                    // but a read still clears a manual-unread marker.
                    None => {
                        let state = read_state(conn, device_id, &chat)?;
                        let unread = count_unread(conn, device_id, &chat, &state)?;
                        diesel::update(
                            chat_row(device_id, &chat)
                                .filter(schema::chats::unread_count.eq(UNREAD_MARKER)),
                        )
                        .set(schema::chats::unread_count.eq(unread))
                        .execute(conn)?;
                    }
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
            let chat =
                crate::lid::route_chat_key(conn, device_id, &update.chat_jid.to_string(), cs)?;
            diesel::update(message_row(device_id, &chat, &update.message_id))
                .set(schema::messages::starred.eq(update.action.starred.unwrap_or(false)))
                .execute(conn)?;
            cs.message_chats.insert(chat);
            Ok(())
        }
        Event::DeleteChatUpdate(update) => {
            let chat = crate::lid::route_chat_key(conn, device_id, &update.jid.to_string(), cs)?;
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
            let chat = crate::lid::route_chat_key(conn, device_id, &update.jid.to_string(), cs)?;
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
            let chat =
                crate::lid::route_chat_key(conn, device_id, &update.chat_jid.to_string(), cs)?;
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
    let wire = info.source.chat.to_string();
    let chat = crate::lid::route_chat_key(conn, device_id, &wire, cs)?;
    if chat != wire {
        cs.message_chats.insert(wire);
    }
    let sender = info.source.sender.to_string();
    let ts_ms = info.timestamp.timestamp_millis();

    // Live push names ride on every message; keep contacts warm from them.
    if !info.push_name.is_empty() && !info.source.is_from_me {
        upsert_contact_push_name(conn, device_id, &sender, &info.push_name)?;
        cs.contacts = true;
    }

    // Same for business verified names, so display_name() can fall back to them.
    if !info.source.is_from_me
        && let Some(name) = info
            .verified_name
            .as_ref()
            .and_then(|vn| vn.name.as_deref())
        && !name.is_empty()
    {
        upsert_contact_business_name(conn, device_id, &sender, name)?;
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
                    proto: Some(&waproto::codec::message_to_vec(&inbound.message)),
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
                &sender,
                info.source.is_from_me,
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
/// or stale (e.g. history-sync) edit can't roll back a newer one. An edit
/// arriving before its target (offline drain reordering) materializes the
/// edited content up front — `insert_message` skips edited rows, so the
/// original's later arrival can't show pre-edit text. Returns whether the
/// chat-list preview changed.
#[allow(clippy::too_many_arguments)]
fn apply_edit(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    target_id: &str,
    sender: &str,
    from_me: bool,
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
        let inserted = diesel::insert_into(dsl::messages)
            .values((
                dsl::device_id.eq(device_id),
                dsl::chat_jid.eq(chat),
                dsl::msg_id.eq(target_id),
                dsl::sender_jid.eq(sender),
                dsl::from_me.eq(from_me),
                dsl::timestamp_ms.eq(ts_ms),
                dsl::kind.eq(new_kind),
                dsl::text_content.eq(new_text),
                dsl::proto.eq(Some(new_proto)),
                dsl::status.eq(if from_me {
                    wa::web_message_info::Status::SERVER_ACK as i32
                } else {
                    wa::web_message_info::Status::DELIVERY_ACK as i32
                }),
                dsl::edited_at_ms.eq(Some(ts_ms)),
            ))
            // Conflict = the row exists but rejected the edit (revoked, or a
            // newer edit already applied): stale, nothing to preserve.
            .on_conflict_do_nothing()
            .execute(conn)?
            > 0;
        if inserted {
            // The message DID happen — the chat must exist, order by it and
            // badge it exactly as if the (never-seen) original had landed.
            bump_chat(
                conn,
                device_id,
                chat,
                ChatBump {
                    msg_id: target_id,
                    ts_ms,
                    preview: new_text,
                    kind: Some(new_kind),
                    unread_delta: i32::from(!from_me),
                },
            )?;
            return Ok(true);
        }
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
        let inserted = diesel::insert_into(dsl::messages)
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
            .execute(conn)?
            > 0;
        // The tombstone may be the chat's first/newest row: the chat must
        // exist and order by it (the deleted message DID happen), and an
        // unseen deletion still counts as unread like WA's own badge does.
        if inserted {
            bump_chat(
                conn,
                device_id,
                chat,
                ChatBump {
                    msg_id: target_id,
                    ts_ms,
                    preview: None,
                    kind: None,
                    unread_delta: i32::from(!target_from_me),
                },
            )?;
            return Ok(true);
        }
        return Ok(false);
    }
    refresh_preview_if_latest(conn, device_id, chat, target_id, None, None)
}

/// When `msg_id` is the chat's most recent message, replace the denormalized
/// chat-list preview (an edit/revoke of an older message leaves it alone).
/// "Most recent" uses the same total order as `messages()` — `(timestamp_ms,
/// rowid)` — so a same-second sibling can't hijack the preview.
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
        .order((dsl::timestamp_ms.desc(), dsl::rowid.desc()))
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

struct ChatHead {
    timestamp_ms: i64,
    preview: Option<String>,
    kind: Option<String>,
}

fn newest_chat_head(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
) -> QueryResult<Option<ChatHead>> {
    use schema::messages::dsl;
    let newest: Option<(i64, Option<String>, String, bool)> = dsl::messages
        .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq(chat)))
        .order((dsl::timestamp_ms.desc(), dsl::rowid.desc()))
        .select((
            dsl::timestamp_ms,
            dsl::text_content,
            dsl::kind,
            dsl::revoked,
        ))
        .first(conn)
        .optional()?;
    Ok(newest.map(|(timestamp_ms, text, kind, revoked)| {
        // A tombstone previews as nothing at all — its pre-revoke kind must
        // not leak back into the chat head.
        let (preview, kind) = if revoked {
            (None, None)
        } else {
            (text, Some(kind))
        };
        ChatHead {
            timestamp_ms,
            preview,
            kind,
        }
    }))
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
    let (preview, kind) = match newest_chat_head(conn, device_id, chat)? {
        Some(head) => (head.preview, head.kind),
        None => (None, None),
    };
    diesel::update(chat_row(device_id, chat))
        .set((
            schema::chats::last_message_preview.eq(preview),
            schema::chats::last_message_kind.eq(kind),
        ))
        .execute(conn)?;
    Ok(())
}

/// Re-derive the chat head when the server replaces an optimistic outgoing
/// timestamp. A deleted newer message deliberately keeps its activity time,
/// while the preview always follows the newest surviving row.
fn reconcile_chat_head_after_timestamp_change(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    old_timestamp_ms: i64,
    new_timestamp_ms: i64,
) -> QueryResult<bool> {
    use schema::chats::dsl as chats;
    let current_head: Option<i64> = chat_row(device_id, chat)
        .select(chats::last_message_ts)
        .first(conn)
        .optional()?;
    let Some(current_head) = current_head else {
        return Ok(false);
    };
    let Some(head) = newest_chat_head(conn, device_id, chat)? else {
        return Ok(false);
    };
    let updated = if current_head != old_timestamp_ms && new_timestamp_ms < current_head {
        diesel::update(chat_row(device_id, chat))
            .set((
                chats::last_message_preview.eq(head.preview),
                chats::last_message_kind.eq(head.kind),
            ))
            .execute(conn)?
    } else {
        diesel::update(chat_row(device_id, chat))
            .set((
                chats::last_message_ts.eq(head.timestamp_ms),
                chats::last_message_preview.eq(head.preview),
                chats::last_message_kind.eq(head.kind),
            ))
            .execute(conn)?
    };
    Ok(updated > 0)
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
    // Empty emoji is a removal tombstone, not a deletion: retaining its
    // timestamp prevents an older history chunk from resurrecting the prior
    // reaction. The read API hides these rows.
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
    // chunk) must not replace either a newer live reaction or its tombstone.
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
    Ok(())
}

fn apply_receipt(
    conn: &mut SqliteConnection,
    device_id: i32,
    receipt: &wacore::types::events::Receipt,
    cs: &mut ChangeSet,
) -> QueryResult<()> {
    // Receipts are the one event that carries the peer's wire identity
    // verbatim: the parser keeps the device on `chat` because the retry
    // pipeline and the receipt echo need the full JID, so a companion device
    // acking a DM arrives as `user:48@lid`. Rows are keyed bare.
    let chat = receipt.source.chat.to_non_ad_string();
    let ts_ms = receipt.timestamp.timestamp_millis();

    let status = match receipt.r#type {
        ReceiptType::Delivered => wa::web_message_info::Status::DELIVERY_ACK as i32,
        ReceiptType::Read => wa::web_message_info::Status::READ as i32,
        ReceiptType::Played => wa::web_message_info::Status::PLAYED as i32,
        ReceiptType::ReadSelf | ReceiptType::PlayedSelf => {
            // Self receipts are LID-addressed once the peer is; the thread may
            // be keyed by either identity (or split) — route to where it lives
            // so the read state lands on the real rows, not a stray twin.
            let wire = chat;
            let chat = crate::lid::route_chat_key(conn, device_id, &wire, cs)?;
            if chat != wire {
                cs.message_chats.insert(wire);
            }
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
                // Cursor didn't move (chat re-read on another device), but a
                // self-read still clears a manual-unread marker.
                let state = read_state(conn, device_id, &chat)?;
                let unread = count_unread(conn, device_id, &chat, &state)?;
                let cleared = diesel::update(
                    chat_row(device_id, &chat)
                        .filter(schema::chats::unread_count.eq(UNREAD_MARKER)),
                )
                .set(schema::chats::unread_count.eq(unread))
                .execute(conn)?;
                if cleared > 0 {
                    cs.chats = true;
                }
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

    // One read-by row per participant, not per device: a member reading on
    // their phone and on Web emits one receipt each.
    let user = receipt.source.sender.to_non_ad_string();
    let mut missed: Vec<&String> = Vec::new();
    for msg_id in &receipt.message_ids {
        // Zero rows covers both the real PN/LID miss and a replay against a
        // row already at/past the target; the alt retry stays harmless for
        // the latter (advance-only) and still heals a lagging split copy.
        if !advance_status(conn, device_id, &chat, msg_id, status)? {
            missed.push(msg_id);
        }
    }
    // A modern peer addresses the receipt by whichever identity it has for
    // the thread — LID receipts for PN-keyed rows or vice versa. Retry the
    // misses under the mapped counterpart key (WA Web's alternate-key
    // fallback, `fixMsgKeysWithPnMapping`); costs one indexed lookup and only
    // on the miss path, so the already-consistent case stays free.
    //
    // Where a message answers under the counterpart key, its receipt belongs
    // there too: the satellite prune is per chat and drops receipt rows whose
    // `msg_id` is absent from *that* chat, so a row left under the wire key
    // would be collected as an orphan.
    let mut relocated: std::collections::HashMap<&String, String> =
        std::collections::HashMap::new();
    // Named by the receipt but held by no chat: the wire key is only a guess
    // for these, resolved once below.
    let mut unowned: Vec<&String> = Vec::new();
    // Resolved only when something actually missed, so a receipt whose messages
    // all answered under the key they were addressed by pays nothing extra —
    // which is the overwhelmingly common case and the one worth keeping free.
    let counterpart = if missed.is_empty() || receipt.source.chat.is_group() {
        None
    } else {
        crate::lid::counterpart_chat_key(conn, device_id, &chat)?
    };
    for msg_id in missed {
        if let Some(alt) = &counterpart
            && advance_status(conn, device_id, alt, msg_id, status)?
        {
            relocated.insert(msg_id, alt.clone());
            continue;
        }
        // The status not advancing does not mean the row is missing: a replayed
        // receipt, or one arriving behind the state already recorded, moves
        // nothing under either key. Whether a message is here at all is a
        // separate question from whether this receipt changed it, and only the
        // first decides where — or whether — the receipt is filed.
        //
        // The addressed key is asked first, and separately from whether it
        // still has a `chats` row: a delete can retire the chat while its
        // messages await cleanup, and a receipt for one of those belongs where
        // the message is, not where the thread went.
        if message_exists(conn, device_id, &chat, msg_id)? {
            continue;
        }
        if let Some(alt) = &counterpart
            && message_exists(conn, device_id, alt, msg_id)?
        {
            relocated.insert(msg_id, alt.clone());
        } else {
            unowned.push(msg_id);
        }
    }
    if !relocated.is_empty()
        && let Some(alt) = counterpart
    {
        cs.message_chats.insert(alt);
    }

    // Both chat kinds record the per-state rows. A group needs them to say who
    // has read; a 1:1 needs them because the message's own `status` keeps only
    // the state it reached, not the instant it got there — which is the half
    // WA Web's "Delivered hh:mm / Read hh:mm" is made of.
    //
    // A receipt for a message no chat holds is dropped rather than parked. The
    // id is the server's, not ours, and nothing here can tell "our send has not
    // been recorded yet" from "this message was deleted and its receipts swept
    // with it" — and the second reading is the common one, because a peer
    // receipt costs a round trip to that peer and back, so it arrives well
    // after the send it answers. Parking it re-created metadata for messages a
    // user had deleted, which is a worse answer than a blank time on a race
    // that resolves itself: the message's own status is only ever advanced by a
    // receipt that finds it, and a later one for the same message will.
    for msg_id in &receipt.message_ids {
        let key = match relocated.get(msg_id) {
            Some(alt) => alt,
            None if unowned.contains(&msg_id) => continue,
            None => &chat,
        };
        record_receipt(conn, device_id, key, msg_id, &user, status, ts_ms)?;
    }

    cs.message_chats.insert(chat);
    Ok(())
}

/// Move one of our messages forward to `status`, reporting whether it moved.
///
/// Peer receipts only ever advance the delivery state of our own messages, and
/// never backwards — so a replay, or one arriving behind the state already
/// recorded, moves nothing and says so.
fn advance_status(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    msg_id: &str,
    status: i32,
) -> QueryResult<bool> {
    let updated = diesel::update(
        message_row(device_id, chat, msg_id).filter(
            schema::messages::from_me
                .eq(true)
                .and(schema::messages::status.lt(status)),
        ),
    )
    .set(schema::messages::status.eq(status))
    .execute(conn)?;
    Ok(updated > 0)
}

/// Whether this device stores an outgoing message with this id in this chat.
fn message_exists(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    msg_id: &str,
) -> QueryResult<bool> {
    diesel::select(diesel::dsl::exists(
        message_row(device_id, chat, msg_id).filter(schema::messages::from_me.eq(true)),
    ))
    .get_result(conn)
}

/// Record that `user` reached `status` on one message, at `ts_ms`.
///
/// Keeps the earliest instant for a state rather than the first one processed.
/// A replay is a duplicate rather than a new event, and receipts do not arrive
/// in time order: an offline queue drains after the live socket, so a peer
/// device's delayed report can land behind a later one for the same state.
/// Arrival order would then decide what message info shows, which is the same
/// reason the alias merge resolves its collisions by `MIN(ts_ms)`.
fn record_receipt(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
    msg_id: &str,
    user: &str,
    status: i32,
    ts_ms: i64,
) -> QueryResult<()> {
    use schema::message_receipts::dsl;
    let row = || {
        dsl::message_receipts.filter(
            dsl::device_id
                .eq(device_id)
                .and(dsl::chat_jid.eq(chat))
                .and(dsl::msg_id.eq(msg_id))
                .and(dsl::user_jid.eq(user))
                .and(dsl::receipt_type.eq(status)),
        )
    };
    let inserted = diesel::insert_into(dsl::message_receipts)
        .values((
            dsl::device_id.eq(device_id),
            dsl::chat_jid.eq(chat),
            dsl::msg_id.eq(msg_id),
            dsl::user_jid.eq(user),
            dsl::receipt_type.eq(status),
            dsl::ts_ms.eq(ts_ms),
        ))
        .on_conflict_do_nothing()
        .execute(conn)?;
    // Only a conflict leaves an instant to reconsider: a row this call created
    // already holds `ts_ms`, and the first report of a state is the common
    // case on a path that runs for every receipt.
    if inserted == 0 {
        diesel::update(row().filter(dsl::ts_ms.gt(ts_ms)))
            .set(dsl::ts_ms.eq(ts_ms))
            .execute(conn)?;
    }
    Ok(())
}

/// Which outgoing row a server ack belongs to, if one can be named.
///
/// [`NotYet`](Self::NotYet) and [`Ambiguous`](Self::Ambiguous) are both "no row
/// applied", but they must not be treated alike: only the first is answerable
/// by waiting. Deferring an ambiguous ack would hand it to whichever row next
/// claims that id — turning a deliberate refusal into a delayed mis-apply.
enum AckTarget {
    Resolved {
        chat: String,
        timestamp_ms: i64,
    },
    /// No outgoing row with this id yet. Carries the storage key the ack named,
    /// when it named one, so a deferral can be held against that chat instead
    /// of against the id alone.
    NotYet {
        chat: Option<String>,
    },
    Ambiguous,
}

fn resolve_server_ack_message(
    conn: &mut SqliteConnection,
    device_id: i32,
    ack: &wacore::types::events::ServerAck,
    cs: &mut ChangeSet,
) -> QueryResult<AckTarget> {
    use schema::messages::dsl;
    if let Some(from) = &ack.from {
        let wire = from.to_string();
        let chat = crate::lid::route_chat_key(conn, device_id, &wire, cs)?;
        let timestamp_ms: Option<i64> = message_row(device_id, &chat, &ack.id)
            .filter(dsl::from_me.eq(true))
            .select(dsl::timestamp_ms)
            .first(conn)
            .optional()?;
        if let Some(timestamp_ms) = timestamp_ms {
            return Ok(AckTarget::Resolved { chat, timestamp_ms });
        }
        // The row may sit under the peer's other identity, so retry across the
        // PN/LID pair — but ONLY that pair. Message ids are sender-chosen and
        // unique within a chat, so widening this to every chat on the device
        // would let a named ack land on an unrelated thread that happens to
        // reuse the id.
        let keys = crate::lid::chat_key_candidates(conn, device_id, &wire)?;
        let aliased: Option<(String, i64)> = dsl::messages
            .filter(
                dsl::device_id
                    .eq(device_id)
                    .and(dsl::chat_jid.eq_any(keys))
                    .and(dsl::msg_id.eq(&ack.id))
                    .and(dsl::from_me.eq(true)),
            )
            .select((dsl::chat_jid, dsl::timestamp_ms))
            .first(conn)
            .optional()?;
        return Ok(match aliased {
            Some((chat, timestamp_ms)) => AckTarget::Resolved { chat, timestamp_ms },
            None => AckTarget::NotYet { chat: Some(chat) },
        });
    }

    // Only a chatless ack falls back to the whole device, and then the id is
    // safe only when it names exactly one outgoing row.
    let matches: Vec<(String, i64)> = dsl::messages
        .filter(
            dsl::device_id
                .eq(device_id)
                .and(dsl::msg_id.eq(&ack.id))
                .and(dsl::from_me.eq(true)),
        )
        .select((dsl::chat_jid, dsl::timestamp_ms))
        .limit(2)
        .load(conn)?;
    match <[(String, i64); 1]>::try_from(matches) {
        Ok([(chat, timestamp_ms)]) => Ok(AckTarget::Resolved { chat, timestamp_ms }),
        Err(matches) if matches.is_empty() => Ok(AckTarget::NotYet { chat: None }),
        Err(_) => {
            warn!(
                target: "ChatStore/Ack",
                "Ignoring ambiguous message ack for reused id {}",
                ack.id
            );
            Ok(AckTarget::Ambiguous)
        }
    }
}

/// How long an unmatched message ack waits for its outgoing row, and how many
/// may wait at once. Both are generous relative to the window they cover (a
/// local enqueue losing to a network round trip) and small enough that a
/// pathological stream of unmatchable ids cannot grow the writer's footprint.
const DEFERRED_ACK_TTL_MS: i64 = 60_000;
const DEFERRED_ACK_CAP: usize = 64;

/// Message-class acks that arrived before their outgoing row existed.
///
/// `Event::ServerAck` is dispatched synchronously on the socket-read path,
/// while `send_message` returns at the stanza write. A host that records its
/// outgoing message *after* the send resolves — the safe order, since
/// recording first leaves a forever-pending ghost row when the send fails and
/// the store has no row delete — therefore races the ack. The window is narrow,
/// needing the local enqueue to lose to a full round trip, but the loss used to
/// be silent and permanent: the row kept its `pending` clock until some
/// delivery receipt happened to lift it (never, for an offline recipient) and
/// never picked up the server's authoritative send timestamp.
///
/// This is the same materialize-later shape the store already uses for
/// out-of-order edits and revokes, minus the placeholder row: an ack carries no
/// content, so there is nothing to show until the real insert arrives.
#[derive(Default, Clone)]
pub(crate) struct DeferredAcks {
    /// Oldest first — pushes append, so the queue is sorted by age and expiry
    /// is a prefix drain.
    entries: std::collections::VecDeque<DeferredAck>,
    /// Everything [`defer`](Self::defer) added since [`begin_batch`], kept even
    /// after `take_matching` consumes it.
    ///
    /// A batch's two kinds of mutation roll back in opposite directions. A
    /// consumption must be undone — the insert that took the ack did not
    /// survive, so the ack is still owed a row. An addition must NOT be undone:
    /// its `ServerAck` event is already off the writer channel and there is no
    /// redelivery for it, so this queue is the only remaining record. Losing it
    /// is precisely the silent, permanent drop the queue exists to prevent.
    ///
    /// [`begin_batch`]: Self::begin_batch
    added_this_batch: Vec<DeferredAck>,
}

#[derive(Clone)]
struct DeferredAck {
    deferred_at_ms: i64,
    /// Storage key the ack named, when it named one. Message ids are
    /// sender-chosen and only unique within a chat, so an ack that names its
    /// chat must only be handed to an insert into that same chat — otherwise a
    /// host reusing one id across two threads could see chat A's ack land on
    /// chat B's row. `None` (the server omitted the chat) matches on the id
    /// alone, which is the same basis its own resolution falls back to.
    ///
    /// That `None` case stays order-dependent, as the undeferred chatless path
    /// always has been: it resolves against the rows that exist when it runs,
    /// so a host that reuses one id across two chats can have the first insert
    /// take an ack the second would have made ambiguous. Closing that would
    /// mean holding every ack to the end of the batch, trading the writer's
    /// in-order application for a case that needs the host to break id
    /// uniqueness in the first place.
    chat: Option<String>,
    ack: wacore::types::events::ServerAck,
}

impl DeferredAcks {
    fn expire(&mut self, now_ms: i64) {
        while let Some(entry) = self.entries.front() {
            if now_ms.saturating_sub(entry.deferred_at_ms) < DEFERRED_ACK_TTL_MS {
                break;
            }
            warn!(
                target: "ChatStore/Ack",
                "Dropping unmatched message ack for {}: no outgoing row appeared within {}s",
                entry.ack.id,
                DEFERRED_ACK_TTL_MS / 1000
            );
            self.entries.pop_front();
        }
    }

    /// Append within the cap, evicting the oldest waiter to make room.
    fn push_bounded(&mut self, entry: DeferredAck) {
        if self.entries.len() >= DEFERRED_ACK_CAP
            && let Some(evicted) = self.entries.pop_front()
        {
            warn!(
                target: "ChatStore/Ack",
                "Dropping unmatched message ack for {}: {DEFERRED_ACK_CAP} acks already waiting",
                evicted.ack.id
            );
        }
        self.entries.push_back(entry);
    }

    /// Open a writer batch: the previous batch's additions are settled and no
    /// longer need replaying.
    fn begin_batch(&mut self) {
        self.added_this_batch.clear();
    }

    /// Fold a batch that did not commit back onto the state it started from.
    ///
    /// The pre-batch queue is the truth for consumptions — the inserts that
    /// took those acks rolled back, so they are still owed rows. The batch's
    /// additions ride along on top, because nothing will deliver them again.
    fn rolled_back(self, mut pre_batch: Self) -> Self {
        for entry in self.added_this_batch {
            pre_batch.push_bounded(entry);
        }
        pre_batch
    }

    fn defer(&mut self, ack: &wacore::types::events::ServerAck, chat: Option<String>, now_ms: i64) {
        self.expire(now_ms);
        let entry = DeferredAck {
            deferred_at_ms: now_ms,
            chat,
            ack: ack.clone(),
        };
        self.added_this_batch.push(entry.clone());
        self.push_bounded(entry);
    }

    fn take_matching(
        &mut self,
        msg_id: &str,
        chat: &str,
        now_ms: i64,
    ) -> Option<wacore::types::events::ServerAck> {
        self.expire(now_ms);
        let at = self.entries.iter().position(|entry| {
            entry.ack.id == msg_id && entry.chat.as_deref().is_none_or(|named| named == chat)
        })?;
        self.entries.remove(at).map(|entry| entry.ack)
    }
}

/// What became of a server ack, so the caller knows whether anything is left to
/// hold on to.
enum AckApplied {
    /// Applied to a row, or deliberately dropped — nothing left to hold.
    Settled,
    /// The send is not recorded yet. Carries the storage key the ack named, to
    /// hold the deferral against.
    Deferrable(Option<String>),
}

fn apply_server_ack(
    conn: &mut SqliteConnection,
    device_id: i32,
    ack: &wacore::types::events::ServerAck,
    cs: &mut ChangeSet,
) -> QueryResult<AckApplied> {
    // Acks cover every stanza class; only message acks map to a stored row.
    if ack.class.as_deref() != Some("message") {
        return Ok(AckApplied::Settled);
    }
    use schema::messages::dsl;
    let (chat, old_timestamp_ms) = match resolve_server_ack_message(conn, device_id, ack, cs)? {
        AckTarget::Resolved { chat, timestamp_ms } => (chat, timestamp_ms),
        // Answerable by waiting: the send may just not be recorded yet.
        AckTarget::NotYet { chat } => return Ok(AckApplied::Deferrable(chat)),
        // Not answerable by waiting, and dangerous to hold — report it settled
        // so the caller drops it instead of arming it for the next row that
        // reuses the id.
        AckTarget::Ambiguous => return Ok(AckApplied::Settled),
    };
    let target = message_row(device_id, &chat, &ack.id).filter(dsl::from_me.eq(true));
    let status_updated = if ack.error.is_some() {
        // Nack: the server rejected the send. Only a still-pending row fails —
        // the server emits one ack per stanza, so a row past PENDING already
        // got its positive answer and a stray nack must not regress it.
        diesel::update(target.filter(dsl::status.eq(wa::web_message_info::Status::PENDING as i32)))
            .set(dsl::status.eq(wa::web_message_info::Status::ERROR as i32))
            .execute(conn)?
            > 0
    } else {
        diesel::update(
            target.filter(dsl::status.lt(wa::web_message_info::Status::SERVER_ACK as i32)),
        )
        .set(dsl::status.eq(wa::web_message_info::Status::SERVER_ACK as i32))
        .execute(conn)?
            > 0
    };
    // A positive message ack's `t` is the server's authoritative send clock.
    // Apply it independently of the status transition: a delivery/read receipt
    // may have advanced the row before the ack event reaches this writer.
    let server_timestamp_ms = ack
        .timestamp
        .filter(|_| ack.error.is_none())
        .map(|timestamp| timestamp.timestamp_millis());
    let timestamp_updated = if let Some(timestamp_ms) = server_timestamp_ms {
        diesel::update(
            message_row(device_id, &chat, &ack.id)
                .filter(dsl::from_me.eq(true))
                .filter(dsl::timestamp_ms.ne(timestamp_ms)),
        )
        .set(dsl::timestamp_ms.eq(timestamp_ms))
        .execute(conn)?
            > 0
    } else {
        false
    };
    if timestamp_updated
        && let Some(timestamp_ms) = server_timestamp_ms
        && reconcile_chat_head_after_timestamp_change(
            conn,
            device_id,
            &chat,
            old_timestamp_ms,
            timestamp_ms,
        )?
    {
        cs.chats = true;
    }
    if status_updated || timestamp_updated {
        // Resolve the chat from the row itself: the ack's `from` is the wire
        // identity, which may not be the key the row is stored under (PN/LID
        // aliasing). Emit both so consumers keyed by either get invalidated.
        cs.message_chats.insert(chat);
        if let Some(from) = &ack.from {
            cs.message_chats.insert(from.to_string());
        }
    }
    Ok(AckApplied::Settled)
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
    let chat = &crate::lid::route_chat_key(conn, device_id, conv.id.as_str(), cs)?;
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
                        proto: Some(&waproto::codec::message_to_vec(message)),
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
                    sender,
                    from_me,
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

/// Union a split pair's chat rows into `dest` and drop `src` (the message
/// rows have already moved). Activity and preview re-derive from the merged
/// messages; the self-read state is the union of both sides so neither
/// side's covered messages re-badge; sticky user prefs (pin/mute/archive,
/// name, ephemeral) keep dest's value and fall back to src's. A manual-unread
/// marker on either side survives; otherwise the badge is recounted.
pub(crate) fn merge_chat_metadata(
    conn: &mut SqliteConnection,
    device_id: i32,
    src: &str,
    dest: &str,
) -> QueryResult<()> {
    use schema::chats::dsl;
    type PrefRow = (
        i64,
        i32,
        Option<i64>,
        Option<i64>,
        bool,
        Option<i32>,
        Option<String>,
    );
    let prefs = |conn: &mut SqliteConnection, key: &str| -> QueryResult<Option<PrefRow>> {
        chat_row(device_id, key)
            .select((
                dsl::last_message_ts,
                dsl::unread_count,
                dsl::pinned_at,
                dsl::muted_until,
                dsl::archived,
                dsl::ephemeral_expiration,
                dsl::name,
            ))
            .first(conn)
            .optional()
    };
    let Some(src_row) = prefs(conn, src)? else {
        return Ok(());
    };
    let src_state = read_state(conn, device_id, src)?;
    ensure_chat(conn, device_id, dest)?;
    let dest_row = prefs(conn, dest)?.unwrap_or((0, 0, None, None, false, None, None));
    let dest_state = read_state(conn, device_id, dest)?;

    let mut merged = ReadState {
        watermark_ms: src_state.watermark_ms.max(dest_state.watermark_ms),
        extra_ids: dest_state.extra_ids,
    };
    for id in src_state.extra_ids {
        if !merged.extra_ids.contains(&id) {
            merged.extra_ids.push(id);
        }
    }
    if merged.extra_ids.len() > READ_EXTRA_IDS_CAP {
        let overflow = merged.extra_ids.len() - READ_EXTRA_IDS_CAP;
        merged.extra_ids.drain(..overflow);
    }
    let ids_json = (!merged.extra_ids.is_empty())
        .then(|| serde_json::to_string(&merged.extra_ids).ok())
        .flatten();

    let unread = if src_row.1 == UNREAD_MARKER || dest_row.1 == UNREAD_MARKER {
        UNREAD_MARKER
    } else {
        count_unread(conn, device_id, dest, &merged)?
    };
    diesel::update(chat_row(device_id, dest))
        .set((
            dsl::last_message_ts.eq(src_row.0.max(dest_row.0)),
            dsl::unread_count.eq(unread),
            dsl::pinned_at.eq(dest_row.2.or(src_row.2)),
            dsl::muted_until.eq(dest_row.3.or(src_row.3)),
            dsl::archived.eq(dest_row.4 || src_row.4),
            dsl::ephemeral_expiration.eq(dest_row.5.or(src_row.5)),
            dsl::name.eq(dest_row.6.or(src_row.6)),
            dsl::read_boundary_ms.eq(merged.watermark_ms),
            dsl::read_boundary_ids.eq(ids_json),
        ))
        .execute(conn)?;
    recompute_chat_preview(conn, device_id, dest)?;
    diesel::delete(chat_row(device_id, src)).execute(conn)?;
    Ok(())
}

/// Contacts are keyed by the peer's bare identity, the canonical form
/// [`ChatStore::contact`] looks up. Message senders keep their device by
/// design (a peer texting from WhatsApp Web is `user:48@lid`), so writing the
/// sender verbatim would file the name under a key nothing ever reads.
fn contact_key(jid: &str) -> Cow<'_, str> {
    match jid.parse::<Jid>() {
        // Bare already renders identically; only pay the allocation otherwise.
        Ok(parsed) if parsed.device != 0 || parsed.agent != 0 => {
            Cow::Owned(parsed.to_non_ad_string())
        }
        _ => Cow::Borrowed(jid),
    }
}

fn upsert_contact_push_name(
    conn: &mut SqliteConnection,
    device_id: i32,
    jid: &str,
    push_name: &str,
) -> QueryResult<()> {
    use schema::contacts::dsl;
    let jid = contact_key(jid);
    diesel::insert_into(dsl::contacts)
        .values((
            dsl::device_id.eq(device_id),
            dsl::jid.eq(&jid),
            dsl::push_name.eq(push_name),
        ))
        .on_conflict((dsl::device_id, dsl::jid))
        .do_update()
        .set(dsl::push_name.eq(push_name))
        .execute(conn)?;
    Ok(())
}

fn upsert_contact_business_name(
    conn: &mut SqliteConnection,
    device_id: i32,
    jid: &str,
    business_name: &str,
) -> QueryResult<()> {
    use schema::contacts::dsl;
    let jid = contact_key(jid);
    diesel::insert_into(dsl::contacts)
        .values((
            dsl::device_id.eq(device_id),
            dsl::jid.eq(&jid),
            dsl::business_name.eq(business_name),
        ))
        .on_conflict((dsl::device_id, dsl::jid))
        .do_update()
        .set(dsl::business_name.eq(business_name))
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
    let jid = contact_key(jid);
    diesel::insert_into(dsl::contacts)
        .values((
            dsl::device_id.eq(device_id),
            dsl::jid.eq(&jid),
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

pub(crate) type MessageRowFilter<'a> = diesel::dsl::Filter<
    schema::messages::table,
    diesel::dsl::And<
        diesel::dsl::And<
            diesel::dsl::Eq<schema::messages::device_id, i32>,
            diesel::dsl::Eq<schema::messages::chat_jid, &'a str>,
        >,
        diesel::dsl::Eq<schema::messages::msg_id, &'a str>,
    >,
>;

pub(crate) fn message_row<'a>(
    device_id: i32,
    chat: &'a str,
    msg_id: &'a str,
) -> MessageRowFilter<'a> {
    schema::messages::table.filter(
        schema::messages::device_id
            .eq(device_id)
            .and(schema::messages::chat_jid.eq(chat))
            .and(schema::messages::msg_id.eq(msg_id)),
    )
}

#[cfg(test)]
mod deferred_ack_tests {
    use super::*;

    fn ack(id: &str) -> wacore::types::events::ServerAck {
        wacore::types::events::ServerAck::builder()
            .id(id.to_string())
            .class("message".to_string())
            .build()
    }

    const CHAT: &str = "559900000001@s.whatsapp.net";
    const OTHER: &str = "559900000002@s.whatsapp.net";

    #[test]
    fn takes_only_its_own_id() {
        let mut acks = DeferredAcks::default();
        acks.defer(&ack("A"), None, 0);
        acks.defer(&ack("B"), None, 0);

        assert!(acks.take_matching("C", CHAT, 0).is_none());
        assert_eq!(acks.take_matching("B", CHAT, 0).unwrap().id, "B");
        // Consumed, not merely read.
        assert!(acks.take_matching("B", CHAT, 0).is_none());
        assert_eq!(acks.take_matching("A", CHAT, 0).unwrap().id, "A");
    }

    /// Message ids are sender-chosen and unique only within a chat, so an ack
    /// that named its chat must not be handed to an insert into another one.
    #[test]
    fn a_chat_scoped_ack_ignores_the_same_id_elsewhere() {
        let mut acks = DeferredAcks::default();
        acks.defer(&ack("OUT-DUP"), Some(CHAT.to_string()), 0);

        assert!(
            acks.take_matching("OUT-DUP", OTHER, 0).is_none(),
            "another chat's insert must not consume it"
        );
        assert!(acks.take_matching("OUT-DUP", CHAT, 0).is_some());
    }

    /// An ack the server sent without a chat resolves on the id alone, so it
    /// matches whichever chat records that id.
    #[test]
    fn a_chatless_ack_matches_any_chat() {
        let mut acks = DeferredAcks::default();
        acks.defer(&ack("OUT-ANY"), None, 0);
        assert!(acks.take_matching("OUT-ANY", OTHER, 0).is_some());
    }

    #[test]
    fn drops_entries_past_the_ttl() {
        let mut acks = DeferredAcks::default();
        acks.defer(&ack("STALE"), None, 0);

        assert!(
            acks.take_matching("STALE", CHAT, DEFERRED_ACK_TTL_MS)
                .is_none()
        );
        // One millisecond inside the window still matches.
        acks.defer(&ack("FRESH"), None, 0);
        assert!(
            acks.take_matching("FRESH", CHAT, DEFERRED_ACK_TTL_MS - 1)
                .is_some()
        );
    }

    #[test]
    fn evicts_the_oldest_at_capacity() {
        let mut acks = DeferredAcks::default();
        for i in 0..DEFERRED_ACK_CAP + 1 {
            acks.defer(&ack(&format!("ACK-{i}")), None, 0);
        }
        assert!(
            acks.take_matching("ACK-0", CHAT, 0).is_none(),
            "the oldest makes room"
        );
        assert!(
            acks.take_matching(&format!("ACK-{DEFERRED_ACK_CAP}"), CHAT, 0)
                .is_some()
        );
    }

    /// A rolled-back batch undoes what it consumed: the insert that took the
    /// ack did not survive, so the ack is still owed a row.
    #[test]
    fn rollback_gives_back_a_consumed_ack() {
        let mut acks = DeferredAcks::default();
        acks.defer(&ack("OUT-1"), None, 0);

        acks.begin_batch();
        let pre_batch = acks.clone();
        assert_eq!(acks.take_matching("OUT-1", CHAT, 0).unwrap().id, "OUT-1");
        assert!(acks.take_matching("OUT-1", CHAT, 0).is_none());

        acks = acks.rolled_back(pre_batch);
        assert_eq!(acks.take_matching("OUT-1", CHAT, 0).unwrap().id, "OUT-1");
    }

    /// ...but it must NOT undo what it added. A `ServerAck` event is off the
    /// writer channel by then and never redelivered, so dropping the deferral
    /// is the silent permanent loss this whole queue exists to prevent.
    #[test]
    fn rollback_keeps_an_ack_the_batch_deferred() {
        let mut acks = DeferredAcks::default();

        acks.begin_batch();
        let pre_batch = acks.clone();
        acks.defer(&ack("OUT-NEW"), None, 0);

        acks = acks.rolled_back(pre_batch);
        assert_eq!(
            acks.take_matching("OUT-NEW", CHAT, 0).unwrap().id,
            "OUT-NEW"
        );
    }

    /// An ack deferred AND consumed inside the same failed batch loses both
    /// mutations, so it is owed a row again.
    #[test]
    fn rollback_keeps_an_ack_the_batch_deferred_then_consumed() {
        let mut acks = DeferredAcks::default();

        acks.begin_batch();
        let pre_batch = acks.clone();
        acks.defer(&ack("OUT-BOTH"), None, 0);
        assert_eq!(
            acks.take_matching("OUT-BOTH", CHAT, 0).unwrap().id,
            "OUT-BOTH"
        );

        acks = acks.rolled_back(pre_batch);
        assert_eq!(
            acks.take_matching("OUT-BOTH", CHAT, 0).unwrap().id,
            "OUT-BOTH"
        );
    }

    /// A transaction that panics poisons the queue's lock while holding acks
    /// that have no other record. Reading through the poison is the whole
    /// point: refusing would turn the panic into the silent loss.
    #[test]
    fn a_poisoned_queue_still_yields_its_acks() {
        let acks = Arc::new(std::sync::Mutex::new(DeferredAcks::default()));
        lock_deferred_acks(&acks).defer(&ack("OUT-PANIC"), None, 0);

        let poisoner = Arc::clone(&acks);
        let panicked = std::thread::spawn(move || {
            let _guard = lock_deferred_acks(&poisoner);
            panic!("writer transaction blew up mid-batch");
        })
        .join();
        assert!(panicked.is_err(), "the thread must actually panic");
        assert!(acks.is_poisoned());

        assert_eq!(
            lock_deferred_acks(&acks)
                .take_matching("OUT-PANIC", CHAT, 0)
                .unwrap()
                .id,
            "OUT-PANIC"
        );
    }

    /// A committed batch settles its additions; the next rollback must not
    /// resurrect them.
    #[test]
    fn a_new_batch_forgets_the_previous_batch_additions() {
        let mut acks = DeferredAcks::default();
        acks.begin_batch();
        acks.defer(&ack("OUT-OLD"), None, 0);
        assert_eq!(
            acks.take_matching("OUT-OLD", CHAT, 0).unwrap().id,
            "OUT-OLD"
        );

        // Next batch commits nothing of its own and rolls back.
        acks.begin_batch();
        let pre_batch = acks.clone();
        acks = acks.rolled_back(pre_batch);

        assert!(
            acks.take_matching("OUT-OLD", CHAT, 0).is_none(),
            "the previous batch committed that consumption"
        );
    }
}
