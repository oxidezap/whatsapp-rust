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
use log::warn;

use crate::error::{ChatStoreError, Result, db_err};
use crate::store::ChatStore;
use crate::types::StoredMessage;

pub(crate) fn ensure_fts(conn: &mut SqliteConnection) -> QueryResult<()> {
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

#[derive(QueryableByName)]
struct FtsRow {
    #[diesel(sql_type = diesel::sql_types::Text)]
    chat_jid: String,
    #[diesel(sql_type = diesel::sql_types::Text)]
    msg_id: String,
}

impl ChatStore {
    /// Full-text search over message text/captions, best match first. The
    /// query is plain words (prefix-matched); FTS5 operators are neutralized.
    pub async fn search_messages(&self, query: &str, limit: i64) -> Result<Vec<StoredMessage>> {
        let Some(match_query) = build_match_query(query) else {
            return Err(ChatStoreError::InvalidSearchQuery);
        };
        let device_id = self.device_id();
        let hits: Vec<FtsRow> = self
            .db()
            .run(move |conn| {
                diesel::sql_query(
                    "SELECT m.chat_jid AS chat_jid, m.msg_id AS msg_id
                     FROM messages_fts f JOIN messages m ON m.rowid = f.rowid
                     WHERE messages_fts MATCH ? AND m.device_id = ?
                     ORDER BY rank LIMIT ?",
                )
                .bind::<diesel::sql_types::Text, _>(&match_query)
                .bind::<diesel::sql_types::Integer, _>(device_id)
                .bind::<diesel::sql_types::BigInt, _>(limit)
                .load(conn)
                .map_err(db_err)
            })
            .await?;

        // Hydrate full rows through the regular path (decodes protos, parses
        // JIDs). One extra point query per hit; hit counts are UI-page sized.
        let mut results = Vec::with_capacity(hits.len());
        for hit in hits {
            match hit.chat_jid.parse() {
                Ok(chat) => {
                    if let Some(message) = self.message(&chat, &hit.msg_id).await? {
                        results.push(message);
                    }
                }
                Err(_) => warn!("chat-store: unparseable chat JID in FTS hit"),
            }
        }
        Ok(results)
    }
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
