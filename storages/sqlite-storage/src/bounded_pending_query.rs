use diesel::query_builder::{AstPass, Query, QueryFragment, QueryId};
use diesel::query_dsl::RunQueryDsl;
use diesel::result::QueryResult;
use diesel::sql_types::{BigInt, Integer, Text, Untyped};
use diesel::sqlite::Sqlite;

#[derive(Debug)]
pub(crate) struct BoundedPendingQueryId;

// The SQL and five bind types are invariant. A static identity lets each
// connection reuse the statement without reusing any key or admission limit.
#[derive(Debug)]
pub(crate) struct BoundedPendingQuery<'a> {
    pub max_bytes: i64,
    pub device_id: i32,
    pub chat: &'a str,
    pub sender: &'a str,
    pub id: &'a str,
}

impl QueryId for BoundedPendingQuery<'_> {
    type QueryId = BoundedPendingQueryId;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl Query for BoundedPendingQuery<'_> {
    type SqlType = Untyped;
}

impl<Conn> RunQueryDsl<Conn> for BoundedPendingQuery<'_> {}

impl QueryFragment<Sqlite> for BoundedPendingQuery<'_> {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "SELECT typeof(message) AS storage_type, \
             CASE WHEN typeof(message) = 'blob' THEN length(message) ELSE NULL END AS byte_len, \
             CASE WHEN typeof(message) = 'blob' THEN \
               CASE WHEN length(message) <= ",
        );
        out.push_bind_param::<BigInt, _>(&self.max_bytes)?;
        out.push_sql(
            " THEN message ELSE NULL END \
             ELSE NULL END AS payload \
             FROM pending_inbound_messages WHERE device_id = ",
        );
        out.push_bind_param::<Integer, _>(&self.device_id)?;
        out.push_sql(" AND chat = ");
        out.push_bind_param::<Text, _>(&self.chat)?;
        out.push_sql(" AND sender = ");
        out.push_bind_param::<Text, _>(&self.sender)?;
        out.push_sql(" AND id = ");
        out.push_bind_param::<Text, _>(&self.id)?;
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct BoundedPendingSendersId;

pub(crate) struct BoundedPendingSenders<'a> {
    pub device_id: i32,
    pub chat: &'a str,
    pub id: &'a str,
}
impl QueryId for BoundedPendingSenders<'_> {
    type QueryId = BoundedPendingSendersId;
    const HAS_STATIC_QUERY_ID: bool = true;
}
impl Query for BoundedPendingSenders<'_> {
    type SqlType = Untyped;
}
impl<Conn> RunQueryDsl<Conn> for BoundedPendingSenders<'_> {}
impl QueryFragment<Sqlite> for BoundedPendingSenders<'_> {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        // Never materialize an oversized TEXT key, even when the BLOB is tiny.
        out.push_sql("SELECT CASE WHEN length(CAST(sender AS BLOB)) <= 128 THEN sender ELSE NULL END AS sender FROM pending_inbound_messages WHERE device_id = ");
        out.push_bind_param::<Integer, _>(&self.device_id)?;
        out.push_sql(" AND chat = ");
        out.push_bind_param::<Text, _>(&self.chat)?;
        out.push_sql(" AND id = ");
        out.push_bind_param::<Text, _>(&self.id)?;
        out.push_sql(" LIMIT 401");
        Ok(())
    }
}
