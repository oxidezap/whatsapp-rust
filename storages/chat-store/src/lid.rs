//! LID/PN peer-identity resolution for chat keys.
//!
//! A 1:1 peer has two interchangeable wire identities — phone number
//! (`@s.whatsapp.net`) and LID (`@lid`) — and traffic for one thread can
//! arrive under either, independent of which key its rows were stored under.
//! WA Web reconciles the two at lookup time
//! (`WAWebDBBulkGetRootMsgs.fixMsgKeysWithPnMapping`,
//! `WAWebLidMigrationUtils.getAlternateMsgKey`) and routes inbound 1:1
//! traffic to the existing thread whichever identity addressed it
//! (`WAWebMessageProcessUtils.selectChatForOneOnOneMessage`): legacy chat ids
//! stay stable, only brand-new chats are keyed by LID.
//!
//! The device store's `lid_pn_mapping` table lives in the same database file
//! and is bidirectional, so both candidate keys of a peer are always
//! derivable — it already is the alias index WA Web keeps as the chat table's
//! `accountLid` column, and the chat-store needs no schema of its own.

use diesel::prelude::*;
use diesel::sql_types::{Bool, Integer, Text};
use wacore_binary::{Jid, Server};

use crate::schema;
use crate::store::ChangeSet;

/// Bare 1:1 user chat key — the only namespace with a PN/LID alias. Hosted
/// and interop namespaces alias differently and are left alone.
fn user_chat(chat: &str) -> Option<Jid> {
    let jid: Jid = chat.parse().ok()?;
    (jid.device == 0 && jid.integrator == 0 && matches!(jid.server, Server::Pn | Server::Lid))
        .then_some(jid)
}

#[derive(QueryableByName)]
struct UserRow {
    #[diesel(sql_type = Text)]
    user: String,
}

/// The peer's other identity, from the device store's mapping table. PN
/// resolves to its most recently updated LID (the same rule as
/// `SqliteStore::get_pn_mapping`); LID resolves straight to its PN.
pub(crate) fn counterpart_chat_key(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
) -> QueryResult<Option<String>> {
    let Some(jid) = user_chat(chat) else {
        return Ok(None);
    };
    let (sql, server) = if jid.is_lid() {
        (
            "SELECT phone_number AS user FROM lid_pn_mapping \
             WHERE lid = ? AND device_id = ? LIMIT 1",
            Server::Pn,
        )
    } else {
        (
            "SELECT lid AS user FROM lid_pn_mapping \
             WHERE phone_number = ? AND device_id = ? ORDER BY updated_at DESC LIMIT 1",
            Server::Lid,
        )
    };
    let row: Option<UserRow> = diesel::sql_query(sql)
        .bind::<Text, _>(jid.user.as_str())
        .bind::<Integer, _>(device_id)
        .get_result(conn)
        .optional()?;
    Ok(row.map(|r| Jid::new(r.user, server).to_string()))
}

/// Every key the peer's rows may live under: the given key plus its mapped
/// counterpart. Read queries filter with these so either identity finds the
/// thread (and a not-yet-merged split reads as one thread).
pub(crate) fn chat_key_candidates(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
) -> QueryResult<Vec<String>> {
    let mut keys = vec![chat.to_string()];
    if let Some(alt) = counterpart_chat_key(conn, device_id, chat)? {
        keys.push(alt);
    }
    Ok(keys)
}

/// Storage key for a chat addressed as `wire_chat`, WA Web
/// `selectChatForOneOnOneMessage` parity: an existing thread keeps its key
/// whichever identity addressed it; a brand-new chat with a known LID is
/// keyed by the LID. Rows split across both keys (the state receipts dropped
/// under the wrong identity leave behind) are merged before routing.
pub(crate) fn route_chat_key(
    conn: &mut SqliteConnection,
    device_id: i32,
    wire_chat: &str,
    cs: &mut ChangeSet,
) -> QueryResult<String> {
    let Some(alt) = counterpart_chat_key(conn, device_id, wire_chat)? else {
        return Ok(wire_chat.to_string());
    };
    let existing: Vec<String> = {
        use schema::chats::dsl;
        dsl::chats
            .filter(
                dsl::device_id
                    .eq(device_id)
                    .and(dsl::jid.eq_any([wire_chat, alt.as_str()])),
            )
            .select(dsl::jid)
            .load(conn)?
    };
    match (
        existing.iter().any(|j| j == wire_chat),
        existing.contains(&alt),
    ) {
        (true, true) => merge_split_chat(conn, device_id, wire_chat, &alt, cs),
        (true, false) => Ok(wire_chat.to_string()),
        (false, true) => Ok(alt),
        (false, false) => Ok(lid_side(wire_chat, &alt).to_string()),
    }
}

fn lid_side<'a>(a: &'a str, b: &'a str) -> &'a str {
    if a.ends_with("@lid") { a } else { b }
}

fn newest_message_ts(
    conn: &mut SqliteConnection,
    device_id: i32,
    chat: &str,
) -> QueryResult<Option<i64>> {
    use schema::messages::dsl;
    dsl::messages
        .filter(dsl::device_id.eq(device_id).and(dsl::chat_jid.eq(chat)))
        .order((dsl::timestamp_ms.desc(), dsl::msg_id.desc()))
        .select(dsl::timestamp_ms)
        .first(conn)
        .optional()
}

#[derive(QueryableByName)]
struct DupMessage {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Integer)]
    status: i32,
    #[diesel(sql_type = Bool)]
    starred: bool,
}

/// Fold a peer's split PN/LID pair into one thread and return the surviving
/// key. Destination is the side with the newer message activity — that is the
/// thread the peer is living in — with ties (and the empty/empty case) going
/// to the LID side, the canonical identity going forward. Idempotent: with
/// nothing under the source key this is a no-op.
pub(crate) fn merge_split_chat(
    conn: &mut SqliteConnection,
    device_id: i32,
    a: &str,
    b: &str,
    cs: &mut ChangeSet,
) -> QueryResult<String> {
    if a == b {
        return Ok(a.to_string());
    }
    let ts_a = newest_message_ts(conn, device_id, a)?;
    let ts_b = newest_message_ts(conn, device_id, b)?;
    let (src, dest) = match (ts_a, ts_b) {
        (Some(ta), Some(tb)) if ta > tb => (b, a),
        (Some(ta), Some(tb)) if ta < tb => (a, b),
        (Some(_), None) => (b, a),
        (None, Some(_)) => (a, b),
        _ => {
            let dest = lid_side(a, b);
            if dest == a { (b, a) } else { (a, b) }
        }
    };
    let src_has_chat_row = {
        use schema::chats::dsl;
        dsl::chats
            .filter(dsl::device_id.eq(device_id).and(dsl::jid.eq(src)))
            .select(dsl::jid)
            .first::<String>(conn)
            .optional()?
            .is_some()
    };
    let src_ts = if src == a { ts_a } else { ts_b };
    // Nothing lives under the source key: already reconciled (or never split).
    if !src_has_chat_row && src_ts.is_none() {
        return Ok(dest.to_string());
    }

    // Same message stored under both keys (fanout echo vs. history overlap):
    // fold the source copy's advance-only columns into the surviving row —
    // the destination row keeps its content (it may carry a newer edit or a
    // tombstone the source copy predates).
    let dups: Vec<DupMessage> = diesel::sql_query(
        "SELECT m.msg_id AS id, m.status AS status, m.starred AS starred FROM messages m \
         WHERE m.device_id = ? AND m.chat_jid = ? AND EXISTS \
         (SELECT 1 FROM messages d WHERE d.device_id = m.device_id \
          AND d.chat_jid = ? AND d.msg_id = m.msg_id)",
    )
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(src)
    .bind::<Text, _>(dest)
    .load(conn)?;
    for dup in &dups {
        use schema::messages::dsl;
        diesel::update(
            crate::store::message_row(device_id, dest, &dup.id).filter(dsl::status.lt(dup.status)),
        )
        .set(dsl::status.eq(dup.status))
        .execute(conn)?;
        if dup.starred {
            diesel::update(crate::store::message_row(device_id, dest, &dup.id))
                .set(dsl::starred.eq(true))
                .execute(conn)?;
        }
    }
    // UPDATE OR IGNORE: PK collisions (the dups above) stay behind and are
    // dropped after. rowids survive the UPDATE, so the FTS external-content
    // index stays consistent; the leftover DELETE fires its cleanup trigger.
    diesel::sql_query(
        "UPDATE OR IGNORE messages SET chat_jid = ? WHERE device_id = ? AND chat_jid = ?",
    )
    .bind::<Text, _>(dest)
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(src)
    .execute(conn)?;
    diesel::sql_query("DELETE FROM messages WHERE device_id = ? AND chat_jid = ?")
        .bind::<Integer, _>(device_id)
        .bind::<Text, _>(src)
        .execute(conn)?;

    // Satellites: the newest reaction per (msg, sender) and the highest
    // receipt per (msg, user) win across the pair, matching their live-path
    // monotonic rules — drop the losing destination rows, then move.
    diesel::sql_query(
        "DELETE FROM reactions WHERE device_id = ?1 AND chat_jid = ?3 AND EXISTS \
         (SELECT 1 FROM reactions s WHERE s.device_id = ?1 AND s.chat_jid = ?2 \
          AND s.msg_id = reactions.msg_id AND s.sender_jid = reactions.sender_jid \
          AND s.ts_ms > reactions.ts_ms)",
    )
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(src)
    .bind::<Text, _>(dest)
    .execute(conn)?;
    diesel::sql_query(
        "UPDATE OR IGNORE reactions SET chat_jid = ? WHERE device_id = ? AND chat_jid = ?",
    )
    .bind::<Text, _>(dest)
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(src)
    .execute(conn)?;
    diesel::sql_query("DELETE FROM reactions WHERE device_id = ? AND chat_jid = ?")
        .bind::<Integer, _>(device_id)
        .bind::<Text, _>(src)
        .execute(conn)?;

    diesel::sql_query(
        "DELETE FROM message_receipts WHERE device_id = ?1 AND chat_jid = ?3 AND EXISTS \
         (SELECT 1 FROM message_receipts s WHERE s.device_id = ?1 AND s.chat_jid = ?2 \
          AND s.msg_id = message_receipts.msg_id AND s.user_jid = message_receipts.user_jid \
          AND s.receipt_type > message_receipts.receipt_type)",
    )
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(src)
    .bind::<Text, _>(dest)
    .execute(conn)?;
    diesel::sql_query(
        "UPDATE OR IGNORE message_receipts SET chat_jid = ? WHERE device_id = ? AND chat_jid = ?",
    )
    .bind::<Text, _>(dest)
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(src)
    .execute(conn)?;
    diesel::sql_query("DELETE FROM message_receipts WHERE device_id = ? AND chat_jid = ?")
        .bind::<Integer, _>(device_id)
        .bind::<Text, _>(src)
        .execute(conn)?;

    crate::store::merge_chat_metadata(conn, device_id, src, dest)?;

    cs.chats = true;
    cs.message_chats.insert(src.to_string());
    cs.message_chats.insert(dest.to_string());
    Ok(dest.to_string())
}
