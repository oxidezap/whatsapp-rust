//! Full-text message search via SQLite FTS5 (feature `search`).
//!
//! External-content index over `messages.text_content`, kept in sync by
//! triggers so the writer never has to think about it. Created lazily and
//! idempotently at open instead of in a migration, so builds without the
//! feature leave no FTS objects behind.
//!
//! Caveat: the index maps by implicit rowid; after a manual `VACUUM`, run
//! `INSERT INTO messages_fts(messages_fts) VALUES('rebuild')`.

use diesel::prelude::*;
use wacore_binary::Jid;

use crate::error::{ChatStoreError, Result, db_err};
use crate::queries::MessageRow;
use crate::schema;
use crate::store::ChatStore;
use crate::types::StoredMessage;

pub(crate) fn ensure_fts(conn: &mut SqliteConnection) -> QueryResult<()> {
    // One transaction for existence-check + DDL + backfill: a partially
    // created index (table committed, rebuild lost) would pass the existence
    // gate forever after, leaving pre-existing rows permanently unindexed.
    conn.transaction(ensure_fts_inner)
}

fn ensure_fts_inner(conn: &mut SqliteConnection) -> QueryResult<()> {
    #[derive(QueryableByName)]
    struct CountRow {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        n: i32,
    }
    let already_exists: bool = diesel::sql_query(
        "SELECT COUNT(*) AS n FROM sqlite_master WHERE type = 'table' AND name = 'messages_fts'",
    )
    .get_result::<CountRow>(conn)?
    .n > 0;

    // Canonical FTS5 external-content recipe: EVERY content row gets exactly
    // one index entry (NULL indexes as empty), and the update trigger pairs
    // delete-then-insert in one body. Anything cuter (WHEN guards, split
    // triggers) breaks the symmetry FTS5's shadow bookkeeping relies on and
    // corrupts rank queries.
    for statement in [
        "CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
             text_content, content='messages', content_rowid='rowid')",
        "CREATE TRIGGER IF NOT EXISTS messages_fts_ai AFTER INSERT ON messages BEGIN
             INSERT INTO messages_fts(rowid, text_content) VALUES (new.rowid, new.text_content);
             END",
        "CREATE TRIGGER IF NOT EXISTS messages_fts_ad AFTER DELETE ON messages BEGIN
             INSERT INTO messages_fts(messages_fts, rowid, text_content)
             VALUES ('delete', old.rowid, old.text_content);
             END",
        "CREATE TRIGGER IF NOT EXISTS messages_fts_au AFTER UPDATE OF text_content ON messages BEGIN
             INSERT INTO messages_fts(messages_fts, rowid, text_content)
             VALUES ('delete', old.rowid, old.text_content);
             INSERT INTO messages_fts(rowid, text_content) VALUES (new.rowid, new.text_content);
             END",
    ] {
        diesel::sql_query(statement).execute(conn)?;
    }
    // First enablement on a database that already has messages: the triggers
    // only cover future writes, so index the existing rows once.
    if !already_exists {
        diesel::sql_query("INSERT INTO messages_fts(messages_fts) VALUES('rebuild')")
            .execute(conn)?;
    }
    Ok(())
}

/// Turn free text into an FTS5 query: each whitespace token becomes a quoted
/// prefix term (`"tok"*`), AND-combined. Sidesteps FTS5 operator syntax so
/// user input can't produce a syntax error.
fn build_match_query(input: &str) -> Option<String> {
    let mut query = String::with_capacity(input.len() + 8);
    for token in input.split_whitespace() {
        if !query.is_empty() {
            query.push(' ');
        }
        query.push('"');
        for ch in token.chars() {
            if ch == '"' {
                query.push('"');
            }
            query.push(ch);
        }
        query.push_str("\"*");
    }
    (!query.is_empty()).then_some(query)
}

/// Shortest token that still earns relevance ranking.
///
/// `ORDER BY rank` has to score every row a term matches before `LIMIT` can
/// discard any, so its cost tracks the size of the match set rather than the
/// page the caller asked for. A one- or two-character prefix matches a large
/// fraction of a real store, which is how a single keystroke turned into a
/// multi-second query. Below this length the search orders by arrival instead:
/// FTS5 walks its index in rowid order and stops at `LIMIT`, and "the newest
/// things that start with this" is a defensible answer for a term that short.
const MIN_RANKED_TERM_LEN: usize = 3;

/// Max rowids per hydration statement, under SQLite's default 999
/// host-parameter limit.
const ID_PARAM_CHUNK: usize = 900;

#[derive(QueryableByName)]
struct FtsHit {
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    rowid: i64,
}

impl ChatStore {
    /// Full-text search over message text/captions, best match first. The
    /// query is plain words (prefix-matched); FTS5 operators are neutralized.
    ///
    /// A term shorter than three characters is ordered newest-first instead of
    /// by relevance: ranking has to score every row such a prefix matches
    /// before `limit` can discard any, which on a real store means most of it.
    pub async fn search_messages(&self, query: &str, limit: i64) -> Result<Vec<StoredMessage>> {
        self.search(query, None, limit).await
    }

    /// The same search restricted to one chat.
    ///
    /// Scoping happens inside the FTS join, so a chat that ranks sparsely still
    /// yields its hits — over-fetching globally and filtering afterwards both
    /// costs more and silently drops them.
    ///
    /// A 1:1 chat may be addressed by either of the peer's identities; both
    /// find the thread.
    pub async fn search_messages_in_chat(
        &self,
        chat: &Jid,
        query: &str,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        self.search(query, Some(chat.to_string()), limit).await
    }

    async fn search(
        &self,
        query: &str,
        chat: Option<String>,
        limit: i64,
    ) -> Result<Vec<StoredMessage>> {
        let Some(match_query) = build_match_query(query) else {
            return Err(ChatStoreError::InvalidSearchQuery);
        };
        let ranked = query
            .split_whitespace()
            .all(|token| token.chars().count() >= MIN_RANKED_TERM_LEN);
        // A negative LIMIT means "unbounded" to SQLite; never let that happen.
        let limit = limit.max(0);
        if limit == 0 {
            return Ok(Vec::new());
        }
        let device_id = self.device_id();
        let rows: Vec<MessageRow> =
            self.db()
                .read(move |conn| {
                    let keys = match &chat {
                        Some(chat) => crate::lid::chat_key_candidates(conn, device_id, chat)
                            .map_err(db_err)?,
                        None => Vec::new(),
                    };
                    let hits = fts_hits(conn, device_id, &match_query, &keys, ranked, limit)?;
                    if hits.is_empty() {
                        return Ok(Vec::new());
                    }
                    // One statement per chunk of hits, instead of a point query
                    // per hit (each of which used to re-resolve the chat's
                    // identity keys first, so N hits cost ~2N serialized round
                    // trips). `limit` is the caller's, so the id list is chunked
                    // rather than trusted to stay under SQLite's host-parameter
                    // ceiling.
                    let ids: Vec<i64> = hits.iter().map(|hit| hit.rowid).collect();
                    let mut rows: Vec<MessageRow> = Vec::with_capacity(ids.len());
                    for chunk in ids.chunks(ID_PARAM_CHUNK) {
                        rows.extend(
                            schema::messages::dsl::messages
                                .filter(schema::messages::dsl::rowid.eq_any(chunk))
                                .load::<MessageRow>(conn)
                                .map_err(db_err)?,
                        );
                    }
                    // `eq_any` returns table order, and chunking splits it
                    // further; restore the order the ranking (or the recency
                    // scan) put them in.
                    let rank_of: std::collections::HashMap<i64, usize> =
                        ids.iter().enumerate().map(|(at, id)| (*id, at)).collect();
                    rows.sort_by_key(|row| rank_of.get(&row.rowid).copied().unwrap_or(usize::MAX));
                    Ok(rows)
                })
                .await?;
        Ok(rows.into_iter().map(Into::into).collect())
    }
}

/// Matching rowids, best first. `keys` scopes the search to one chat's storage
/// identities; empty searches every chat.
fn fts_hits(
    conn: &mut SqliteConnection,
    device_id: i32,
    match_query: &str,
    keys: &[String],
    ranked: bool,
    limit: i64,
) -> wacore::store::error::Result<Vec<FtsHit>> {
    use diesel::sql_types::{BigInt, Integer, Text};

    // Constant fragments, never caller input. `rank` is qualified because it is
    // only unambiguous today by accident — `messages` has no such column, and
    // an unqualified reference would silently start resolving to it if one were
    // ever added.
    let order = if ranked { "f.rank" } else { "f.rowid DESC" };
    let Some(first_key) = keys.first() else {
        return diesel::sql_query(format!(
            "SELECT f.rowid AS rowid
             FROM messages_fts f JOIN messages m ON m.rowid = f.rowid
             WHERE messages_fts MATCH ? AND m.device_id = ?
             ORDER BY {order} LIMIT ?"
        ))
        .bind::<Text, _>(match_query)
        .bind::<Integer, _>(device_id)
        .bind::<BigInt, _>(limit)
        .load(conn)
        .map_err(db_err);
    };
    // A chat has at most two storage identities (PN and LID). Padding the
    // single-key case to two placeholders keeps one statically bound statement
    // instead of a variadic one; `IN (x, x)` is `IN (x)`.
    let second_key = keys.get(1).unwrap_or(first_key);
    diesel::sql_query(format!(
        "SELECT f.rowid AS rowid
         FROM messages_fts f JOIN messages m ON m.rowid = f.rowid
         WHERE messages_fts MATCH ? AND m.device_id = ? AND m.chat_jid IN (?, ?)
         ORDER BY {order} LIMIT ?"
    ))
    .bind::<Text, _>(match_query)
    .bind::<Integer, _>(device_id)
    .bind::<Text, _>(first_key)
    .bind::<Text, _>(second_key)
    .bind::<BigInt, _>(limit)
    .load(conn)
    .map_err(db_err)
}

#[cfg(test)]
mod tests {
    use super::build_match_query;

    #[test]
    fn match_query_neutralizes_operators_and_quotes() {
        assert_eq!(build_match_query("hello"), Some("\"hello\"*".into()));
        assert_eq!(
            build_match_query("hello world"),
            Some("\"hello\"* \"world\"*".into())
        );
        assert_eq!(build_match_query("a\"b"), Some("\"a\"\"b\"*".into()));
        assert_eq!(
            build_match_query("NOT OR AND"),
            Some("\"NOT\"* \"OR\"* \"AND\"*".into())
        );
        assert_eq!(build_match_query("   "), None);
    }
}
