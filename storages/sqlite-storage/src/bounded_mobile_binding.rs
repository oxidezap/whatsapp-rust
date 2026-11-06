//! SQL-side admission for the persisted mobile binding, including feature-off reads.
use diesel::deserialize::{self, FromSql};
use diesel::expression::{
    AppearsOnTable, Expression, SelectableExpression, ValidGrouping, is_aggregate,
};
use diesel::query_builder::{AstPass, QueryFragment, QueryId};
use diesel::result::QueryResult;
use diesel::sql_types::{BigInt, Binary, Nullable};
use diesel::sqlite::{Sqlite, SqliteType, SqliteValue};
use wacore::store::error::{Result, StoreError};

use crate::schema::device;

const MAX_BINDING_BYTES: usize = 1_048_576;
const MAX_BINDING_BYTES_SQL: i64 = MAX_BINDING_BYTES as i64;

/// The SQL and bind type are invariant, so each connection can reuse the statement.
#[derive(Clone, Copy)]
pub(crate) struct GuardedMobileBindingExpression;

impl Expression for GuardedMobileBindingExpression {
    type SqlType = Nullable<Binary>;
}

impl AppearsOnTable<device::table> for GuardedMobileBindingExpression {}
impl SelectableExpression<device::table> for GuardedMobileBindingExpression {}
impl ValidGrouping<()> for GuardedMobileBindingExpression {
    type IsAggregate = is_aggregate::No;
}
impl QueryId for GuardedMobileBindingExpression {
    type QueryId = Self;
    const HAS_STATIC_QUERY_ID: bool = true;
}
impl QueryFragment<Sqlite> for GuardedMobileBindingExpression {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        // Nested CASE avoids reading TEXT length or returning a rejected BLOB.
        // The integer sentinel stays distinct from a genuine SQL NULL.
        out.push_sql(
            "CASE WHEN mobile_session_binding IS NULL THEN NULL \
             WHEN typeof(mobile_session_binding) = 'blob' THEN \
               CASE WHEN length(mobile_session_binding) <= ",
        );
        out.push_bind_param::<BigInt, _>(&MAX_BINDING_BYTES_SQL)?;
        out.push_sql(" THEN mobile_session_binding ELSE 0 END ELSE 0 END");
        Ok(())
    }
}

enum BindingValue {
    Absent,
    Bytes(Vec<u8>),
    Rejected,
}

/// Only SQL-admitted bytes can be copied into an owned binding buffer.
#[derive(diesel::deserialize::FromSqlRow)]
pub(crate) struct GuardedMobileBinding(BindingValue);

impl FromSql<Nullable<Binary>, Sqlite> for GuardedMobileBinding {
    fn from_sql(mut value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
        // Inspect the actual storage class before read_blob can coerce it.
        if value.value_type() != Some(SqliteType::Binary) {
            return Ok(Self(BindingValue::Rejected));
        }
        let bytes = value.read_blob();
        Ok(Self(if bytes.len() <= MAX_BINDING_BYTES {
            BindingValue::Bytes(bytes.to_vec())
        } else {
            BindingValue::Rejected
        }))
    }

    fn from_nullable_sql(value: Option<SqliteValue<'_, '_, '_>>) -> deserialize::Result<Self> {
        match value {
            Some(value) => Self::from_sql(value),
            None => Ok(Self(BindingValue::Absent)),
        }
    }
}

impl GuardedMobileBinding {
    pub(crate) fn into_bytes(self) -> Result<Option<Vec<u8>>> {
        match self.0 {
            BindingValue::Absent => Ok(None),
            BindingValue::Bytes(bytes) => Ok(Some(bytes)),
            BindingValue::Rejected => Err(StoreError::Validation(
                "oversized mobile session binding".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::{InstrumentationEvent, SimpleConnection};
    use diesel::prelude::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    #[derive(diesel::deserialize::FromSqlRow)]
    struct StorageClass(Option<SqliteType>);

    impl FromSql<Nullable<Binary>, Sqlite> for StorageClass {
        fn from_sql(value: SqliteValue<'_, '_, '_>) -> deserialize::Result<Self> {
            // Observing the SQL result type never reads or copies its payload.
            Ok(Self(value.value_type()))
        }

        fn from_nullable_sql(value: Option<SqliteValue<'_, '_, '_>>) -> deserialize::Result<Self> {
            Ok(Self(value.and_then(|value| value.value_type())))
        }
    }

    #[test]
    fn projection_bounds_payloads_preserves_null_and_reuses_the_account_query() {
        let mut conn = SqliteConnection::establish(":memory:").unwrap();
        conn.batch_execute(
            "CREATE TABLE device (id INTEGER PRIMARY KEY, mobile_session_binding BLOB); \
             INSERT INTO device VALUES \
             (1, NULL), (2, X''), (3, zeroblob(1048576)), \
             (4, zeroblob(1048577)), (5, 'synthetic text'), \
             (6, CAST(zeroblob(1048577) AS TEXT)), (7, 0), (8, 1.5);",
        )
        .unwrap();
        let cached = Arc::new(AtomicUsize::new(0));
        let observed = cached.clone();
        conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
            if let InstrumentationEvent::CacheQuery { .. } = event {
                observed.fetch_add(1, Ordering::SeqCst);
            }
        });

        for (id, expected_type) in [
            (1, None),
            (2, Some(SqliteType::Binary)),
            (3, Some(SqliteType::Binary)),
            (4, Some(SqliteType::Long)),
            (5, Some(SqliteType::Long)),
            (6, Some(SqliteType::Long)),
            (7, Some(SqliteType::Long)),
            (8, Some(SqliteType::Long)),
        ] {
            let result: StorageClass = device::table
                .filter(device::id.eq(id))
                .select(GuardedMobileBindingExpression)
                .first(&mut conn)
                .unwrap();
            assert_eq!(result.0, expected_type);
        }
        for (id, expected_len) in [(1, None), (2, Some(0)), (3, Some(MAX_BINDING_BYTES))] {
            let column: GuardedMobileBinding = device::table
                .filter(device::id.eq(id))
                .select(GuardedMobileBindingExpression)
                .first(&mut conn)
                .unwrap();
            assert_eq!(
                column.into_bytes().unwrap().map(|bytes| bytes.len()),
                expected_len
            );
        }
        for id in 4..=8 {
            let column: GuardedMobileBinding = device::table
                .filter(device::id.eq(id))
                .select(GuardedMobileBindingExpression)
                .first(&mut conn)
                .unwrap();
            assert!(matches!(
                column.into_bytes(),
                Err(StoreError::Validation(_))
            ));
        }
        let missing: Option<GuardedMobileBinding> = device::table
            .filter(device::id.eq(99))
            .select(GuardedMobileBindingExpression)
            .first(&mut conn)
            .optional()
            .unwrap();
        assert!(missing.is_none());
        assert_eq!(cached.load(Ordering::SeqCst), 1);
    }
}
