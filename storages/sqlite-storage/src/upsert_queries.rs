use diesel::query_builder::{AstPass, QueryFragment, QueryId};
use diesel::query_dsl::RunQueryDsl;
use diesel::result::QueryResult;
use diesel::sql_types::{Binary, Integer, Nullable, Text};
use diesel::sqlite::Sqlite;

/// Static query identifier for `UpsertSession`.
#[derive(Debug, Clone, Copy)]
pub struct UpsertSessionQuery;

impl QueryId for UpsertSessionQuery {
    type QueryId = Self;
    const HAS_STATIC_QUERY_ID: bool = true;
}

/// Typed UPSERT statement for the `sessions` table.
///
/// Invariant SQL and bind order ensure safe caching in Diesel's per-connection
/// statement cache. Uses SQLite `excluded.record` to avoid double-binding the
/// payload blob between `VALUES` and `DO UPDATE SET`.
#[derive(Debug)]
pub struct UpsertSession<'a> {
    pub address: &'a str,
    pub record: &'a [u8],
    pub device_id: i32,
}

impl<'a> QueryId for UpsertSession<'a> {
    type QueryId = UpsertSessionQuery;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl<Conn> RunQueryDsl<Conn> for UpsertSession<'_> {}

impl<'a> QueryFragment<Sqlite> for UpsertSession<'a> {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("INSERT INTO \"sessions\" (\"address\", \"record\", \"device_id\") VALUES (");
        out.push_bind_param::<Text, _>(&self.address)?;
        out.push_sql(", ");
        out.push_bind_param::<Binary, _>(&self.record)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.device_id)?;
        out.push_sql(
            ") ON CONFLICT (\"address\", \"device_id\") DO UPDATE SET \"record\" = excluded.\"record\"",
        );
        Ok(())
    }
}

/// Static query identifier for `UpsertIdentity`.
#[derive(Debug, Clone, Copy)]
pub struct UpsertIdentityQuery;

impl QueryId for UpsertIdentityQuery {
    type QueryId = Self;
    const HAS_STATIC_QUERY_ID: bool = true;
}

/// Typed UPSERT statement for the `identities` table.
#[derive(Debug)]
pub struct UpsertIdentity<'a> {
    pub address: &'a str,
    pub key: &'a [u8],
    pub device_id: i32,
}

impl<'a> QueryId for UpsertIdentity<'a> {
    type QueryId = UpsertIdentityQuery;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl<Conn> RunQueryDsl<Conn> for UpsertIdentity<'_> {}

impl<'a> QueryFragment<Sqlite> for UpsertIdentity<'a> {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql("INSERT INTO \"identities\" (\"address\", \"key\", \"device_id\") VALUES (");
        out.push_bind_param::<Text, _>(&self.address)?;
        out.push_sql(", ");
        out.push_bind_param::<Binary, _>(&self.key)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.device_id)?;
        out.push_sql(
            ") ON CONFLICT (\"address\", \"device_id\") DO UPDATE SET \"key\" = excluded.\"key\"",
        );
        Ok(())
    }
}

/// Static query identifier for `UpsertSenderKey`.
#[derive(Debug, Clone, Copy)]
pub struct UpsertSenderKeyQuery;

impl QueryId for UpsertSenderKeyQuery {
    type QueryId = Self;
    const HAS_STATIC_QUERY_ID: bool = true;
}

/// Typed UPSERT statement for the `sender_keys` table.
#[derive(Debug)]
pub struct UpsertSenderKey<'a> {
    pub address: &'a str,
    pub record: &'a [u8],
    pub device_id: i32,
}

impl<'a> QueryId for UpsertSenderKey<'a> {
    type QueryId = UpsertSenderKeyQuery;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl<Conn> RunQueryDsl<Conn> for UpsertSenderKey<'_> {}

impl<'a> QueryFragment<Sqlite> for UpsertSenderKey<'a> {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "INSERT INTO \"sender_keys\" (\"address\", \"record\", \"device_id\") VALUES (",
        );
        out.push_bind_param::<Text, _>(&self.address)?;
        out.push_sql(", ");
        out.push_bind_param::<Binary, _>(&self.record)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.device_id)?;
        out.push_sql(
            ") ON CONFLICT (\"address\", \"device_id\") DO UPDATE SET \"record\" = excluded.\"record\"",
        );
        Ok(())
    }
}

/// Static query identifier for `UpsertDeviceRegistry`.
#[derive(Debug, Clone, Copy)]
pub struct UpsertDeviceRegistryQuery;

impl QueryId for UpsertDeviceRegistryQuery {
    type QueryId = Self;
    const HAS_STATIC_QUERY_ID: bool = true;
}

/// Typed UPSERT statement for the `device_registry` table.
///
/// Invariant parameter count and types ensure that optional fields (`phash`, `raw_id`)
/// generate the exact same SQL placeholder sequence whether `Some` or `None`.
#[derive(Debug)]
pub struct UpsertDeviceRegistry<'a> {
    pub user_id: &'a str,
    pub devices_json: &'a str,
    pub timestamp: i32,
    pub phash: Option<&'a str>,
    pub device_id: i32,
    pub updated_at: i32,
    pub raw_id: Option<i32>,
}

impl<'a> QueryId for UpsertDeviceRegistry<'a> {
    type QueryId = UpsertDeviceRegistryQuery;
    const HAS_STATIC_QUERY_ID: bool = true;
}

impl<Conn> RunQueryDsl<Conn> for UpsertDeviceRegistry<'_> {}

impl<'a> QueryFragment<Sqlite> for UpsertDeviceRegistry<'a> {
    fn walk_ast<'b>(&'b self, mut out: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        out.push_sql(
            "INSERT INTO \"device_registry\" (\
             \"user_id\", \"devices_json\", \"timestamp\", \"phash\", \"device_id\", \"updated_at\", \"raw_id\"\
             ) VALUES (",
        );
        out.push_bind_param::<Text, _>(&self.user_id)?;
        out.push_sql(", ");
        out.push_bind_param::<Text, _>(&self.devices_json)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.timestamp)?;
        out.push_sql(", ");
        out.push_bind_param::<Nullable<Text>, _>(&self.phash)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.device_id)?;
        out.push_sql(", ");
        out.push_bind_param::<Integer, _>(&self.updated_at)?;
        out.push_sql(", ");
        out.push_bind_param::<Nullable<Integer>, _>(&self.raw_id)?;
        out.push_sql(
            ") ON CONFLICT (\"user_id\", \"device_id\") DO UPDATE SET \
             \"devices_json\" = excluded.\"devices_json\", \
             \"timestamp\" = excluded.\"timestamp\", \
             \"phash\" = excluded.\"phash\", \
             \"updated_at\" = excluded.\"updated_at\", \
             \"raw_id\" = excluded.\"raw_id\"",
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::connection::SimpleConnection;
    use diesel::prelude::*;
    use diesel::sqlite::SqliteConnection;

    #[test]
    fn test_upsert_session_sql_invariance() {
        let q1 = UpsertSession {
            address: "1234@s.whatsapp.net",
            record: b"record1",
            device_id: 0,
        };
        let q2 = UpsertSession {
            address: "5678@s.whatsapp.net",
            record: b"record2_longer_than_before",
            device_id: 1,
        };
        let s1 = diesel::debug_query::<Sqlite, _>(&q1).to_string();
        let s2 = diesel::debug_query::<Sqlite, _>(&q2).to_string();
        let sql1 = s1.split(" -- binds:").next().unwrap();
        let sql2 = s2.split(" -- binds:").next().unwrap();
        assert_eq!(sql1, sql2);
        assert_eq!(
            sql1,
            "INSERT INTO \"sessions\" (\"address\", \"record\", \"device_id\") VALUES (?, ?, ?) \
             ON CONFLICT (\"address\", \"device_id\") DO UPDATE SET \"record\" = excluded.\"record\""
        );
    }

    #[test]
    fn test_upsert_identity_sql_invariance() {
        let q1 = UpsertIdentity {
            address: "user1",
            key: b"key1",
            device_id: 0,
        };
        let q2 = UpsertIdentity {
            address: "user2",
            key: b"key2",
            device_id: 2,
        };
        let s1 = diesel::debug_query::<Sqlite, _>(&q1).to_string();
        let s2 = diesel::debug_query::<Sqlite, _>(&q2).to_string();
        let sql1 = s1.split(" -- binds:").next().unwrap();
        let sql2 = s2.split(" -- binds:").next().unwrap();
        assert_eq!(sql1, sql2);
        assert_eq!(
            sql1,
            "INSERT INTO \"identities\" (\"address\", \"key\", \"device_id\") VALUES (?, ?, ?) \
             ON CONFLICT (\"address\", \"device_id\") DO UPDATE SET \"key\" = excluded.\"key\""
        );
    }

    #[test]
    fn test_upsert_sender_key_sql_invariance() {
        let q1 = UpsertSenderKey {
            address: "group1",
            record: b"sk1",
            device_id: 0,
        };
        let q2 = UpsertSenderKey {
            address: "group2",
            record: b"sk2",
            device_id: 3,
        };
        let s1 = diesel::debug_query::<Sqlite, _>(&q1).to_string();
        let s2 = diesel::debug_query::<Sqlite, _>(&q2).to_string();
        let sql1 = s1.split(" -- binds:").next().unwrap();
        let sql2 = s2.split(" -- binds:").next().unwrap();
        assert_eq!(sql1, sql2);
        assert_eq!(
            sql1,
            "INSERT INTO \"sender_keys\" (\"address\", \"record\", \"device_id\") VALUES (?, ?, ?) \
             ON CONFLICT (\"address\", \"device_id\") DO UPDATE SET \"record\" = excluded.\"record\""
        );
    }

    #[test]
    fn test_upsert_device_registry_sql_invariance_some_and_none() {
        let q_some = UpsertDeviceRegistry {
            user_id: "user1",
            devices_json: "{}",
            timestamp: 100,
            phash: Some("phash1"),
            device_id: 0,
            updated_at: 200,
            raw_id: Some(42),
        };
        let q_none = UpsertDeviceRegistry {
            user_id: "user2",
            devices_json: "[]",
            timestamp: 101,
            phash: None,
            device_id: 1,
            updated_at: 201,
            raw_id: None,
        };

        let s_some = diesel::debug_query::<Sqlite, _>(&q_some).to_string();
        let s_none = diesel::debug_query::<Sqlite, _>(&q_none).to_string();
        let sql_template_some = s_some.split(" -- binds:").next().unwrap();
        let sql_template_none = s_none.split(" -- binds:").next().unwrap();
        assert_eq!(sql_template_some, sql_template_none);
        assert_eq!(
            sql_template_some,
            "INSERT INTO \"device_registry\" (\
             \"user_id\", \"devices_json\", \"timestamp\", \"phash\", \"device_id\", \"updated_at\", \"raw_id\"\
             ) VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT (\"user_id\", \"device_id\") DO UPDATE SET \
             \"devices_json\" = excluded.\"devices_json\", \
             \"timestamp\" = excluded.\"timestamp\", \
             \"phash\" = excluded.\"phash\", \
             \"updated_at\" = excluded.\"updated_at\", \
             \"raw_id\" = excluded.\"raw_id\""
        );
    }

    #[test]
    fn test_static_query_ids_unique() {
        use std::any::TypeId;
        let id_session = TypeId::of::<<UpsertSession as QueryId>::QueryId>();
        let id_identity = TypeId::of::<<UpsertIdentity as QueryId>::QueryId>();
        let id_sender_key = TypeId::of::<<UpsertSenderKey as QueryId>::QueryId>();
        let id_registry = TypeId::of::<<UpsertDeviceRegistry as QueryId>::QueryId>();

        assert_ne!(id_session, id_identity);
        assert_ne!(id_session, id_sender_key);
        assert_ne!(id_session, id_registry);
        assert_ne!(id_identity, id_sender_key);
        assert_ne!(id_identity, id_registry);
        assert_ne!(id_sender_key, id_registry);

        const { assert!(<UpsertSession as QueryId>::HAS_STATIC_QUERY_ID) };
        const { assert!(<UpsertIdentity as QueryId>::HAS_STATIC_QUERY_ID) };
        const { assert!(<UpsertSenderKey as QueryId>::HAS_STATIC_QUERY_ID) };
        const { assert!(<UpsertDeviceRegistry as QueryId>::HAS_STATIC_QUERY_ID) };
    }

    #[test]
    fn test_diesel_upsert_execution_and_cache_reuse() {
        let mut conn = SqliteConnection::establish(":memory:").expect("in-memory db");
        conn.batch_execute(
            "CREATE TABLE sessions (
                address TEXT NOT NULL,
                record BLOB NOT NULL,
                device_id INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (address, device_id)
            );",
        )
        .expect("create table");

        // First execution - prepares and caches
        let rows1 = UpsertSession {
            address: "user@s.whatsapp.net",
            record: b"initial_session",
            device_id: 0,
        }
        .execute(&mut conn)
        .expect("execute 1");
        assert_eq!(rows1, 1);

        // Second execution - reuses cached prepared statement, updates row
        let rows2 = UpsertSession {
            address: "user@s.whatsapp.net",
            record: b"updated_session",
            device_id: 0,
        }
        .execute(&mut conn)
        .expect("execute 2");
        assert_eq!(rows2, 1);

        // Third execution - different device_id, inserts new row
        let rows3 = UpsertSession {
            address: "user@s.whatsapp.net",
            record: b"device_1_session",
            device_id: 1,
        }
        .execute(&mut conn)
        .expect("execute 3");
        assert_eq!(rows3, 1);

        // Verify data
        use diesel::dsl::sql;
        let count: i64 = diesel::select(sql::<diesel::sql_types::BigInt>("count(*) FROM sessions"))
            .get_result(&mut conn)
            .expect("count");
        assert_eq!(count, 2);
    }

    #[test]
    fn test_device_registry_null_transitions() {
        let mut conn = SqliteConnection::establish(":memory:").expect("in-memory db");
        conn.batch_execute(
            "CREATE TABLE device_registry (
                user_id TEXT NOT NULL,
                devices_json TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                phash TEXT,
                device_id INTEGER NOT NULL DEFAULT 0,
                updated_at INTEGER NOT NULL,
                raw_id INTEGER,
                PRIMARY KEY (user_id, device_id)
            );",
        )
        .expect("create table");

        // 1. Insert with Some(phash) and Some(raw_id)
        UpsertDeviceRegistry {
            user_id: "user1",
            devices_json: "[1,2]",
            timestamp: 100,
            phash: Some("phash_v1"),
            device_id: 0,
            updated_at: 1000,
            raw_id: Some(777),
        }
        .execute(&mut conn)
        .expect("insert");

        #[derive(QueryableByName, Debug, PartialEq)]
        struct RegistryRow {
            #[diesel(sql_type = Text)]
            devices_json: String,
            #[diesel(sql_type = Nullable<Text>)]
            phash: Option<String>,
            #[diesel(sql_type = Nullable<Integer>)]
            raw_id: Option<i32>,
        }

        let row: RegistryRow = diesel::sql_query(
            "SELECT devices_json, phash, raw_id FROM device_registry WHERE user_id = 'user1'",
        )
        .get_result(&mut conn)
        .expect("read 1");
        assert_eq!(row.phash.as_deref(), Some("phash_v1"));
        assert_eq!(row.raw_id, Some(777));

        // 2. Update with None for both phash and raw_id - verify excluded.* writes NULL
        UpsertDeviceRegistry {
            user_id: "user1",
            devices_json: "[1,2,3]",
            timestamp: 200,
            phash: None,
            device_id: 0,
            updated_at: 2000,
            raw_id: None,
        }
        .execute(&mut conn)
        .expect("update to None");

        let row2: RegistryRow = diesel::sql_query(
            "SELECT devices_json, phash, raw_id FROM device_registry WHERE user_id = 'user1'",
        )
        .get_result(&mut conn)
        .expect("read 2");
        assert_eq!(row2.devices_json, "[1,2,3]");
        assert_eq!(row2.phash, None);
        assert_eq!(row2.raw_id, None);

        // 3. Update back to Some(phash) and None raw_id
        UpsertDeviceRegistry {
            user_id: "user1",
            devices_json: "[1]",
            timestamp: 300,
            phash: Some("phash_v2"),
            device_id: 0,
            updated_at: 3000,
            raw_id: None,
        }
        .execute(&mut conn)
        .expect("update to Some/None");

        let row3: RegistryRow = diesel::sql_query(
            "SELECT devices_json, phash, raw_id FROM device_registry WHERE user_id = 'user1'",
        )
        .get_result(&mut conn)
        .expect("read 3");
        assert_eq!(row3.phash.as_deref(), Some("phash_v2"));
        assert_eq!(row3.raw_id, None);
    }

    #[test]
    fn test_batch_rollback_leaves_connection_clean_for_reuse() {
        let mut conn = SqliteConnection::establish(":memory:").expect("in-memory db");
        conn.batch_execute(
            "CREATE TABLE sessions (
                address TEXT NOT NULL,
                record BLOB NOT NULL,
                device_id INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (address, device_id)
            );",
        )
        .expect("create table");

        // Seed 1 row
        UpsertSession {
            address: "user1",
            record: b"initial",
            device_id: 0,
        }
        .execute(&mut conn)
        .expect("initial seed");

        // Execute a transaction that aborts
        let result: Result<(), diesel::result::Error> = conn.transaction(|conn| {
            UpsertSession {
                address: "user2",
                record: b"rec2",
                device_id: 0,
            }
            .execute(conn)?;

            // Simulating mid-batch abort
            Err(diesel::result::Error::RollbackTransaction)
        });
        assert!(result.is_err());

        // Verify user2 was NOT committed
        let count: i64 = diesel::select(diesel::dsl::sql::<diesel::sql_types::BigInt>(
            "count(*) FROM sessions",
        ))
        .get_result(&mut conn)
        .expect("count");
        assert_eq!(count, 1);

        // Connection statement cache is still healthy and prepared statement can be reused immediately
        UpsertSession {
            address: "user1",
            record: b"updated_after_rollback",
            device_id: 0,
        }
        .execute(&mut conn)
        .expect("reuse after rollback");

        #[derive(QueryableByName)]
        struct RecordRow {
            #[diesel(sql_type = Binary)]
            record: Vec<u8>,
        }
        let row: RecordRow =
            diesel::sql_query("SELECT record FROM sessions WHERE address = 'user1'")
                .get_result(&mut conn)
                .expect("query");
        assert_eq!(row.record, b"updated_after_rollback");
    }

    #[test]
    fn every_upsert_is_cached_once_per_connection_and_survives_statement_error() {
        use diesel::connection::InstrumentationEvent;
        use std::sync::{Arc, Mutex};

        for _ in 0..2 {
            let mut conn = SqliteConnection::establish(":memory:").unwrap();
            conn.batch_execute(
                "CREATE TABLE sessions (address TEXT, record BLOB CHECK(length(record) > 0), device_id INTEGER, PRIMARY KEY(address, device_id));
                 CREATE TABLE identities (address TEXT, key BLOB, device_id INTEGER, PRIMARY KEY(address, device_id));
                 CREATE TABLE sender_keys (address TEXT, record BLOB, device_id INTEGER, PRIMARY KEY(address, device_id));
                 CREATE TABLE device_registry (user_id TEXT, devices_json TEXT, timestamp INTEGER, phash TEXT, device_id INTEGER, updated_at INTEGER, raw_id INTEGER, PRIMARY KEY(user_id, device_id));"
            ).unwrap();
            let cached = Arc::new(Mutex::new(Vec::<String>::new()));
            let events = Arc::clone(&cached);
            conn.set_instrumentation(move |event: InstrumentationEvent<'_>| {
                if let InstrumentationEvent::CacheQuery { sql, .. } = event {
                    events.lock().unwrap().push(sql.to_owned());
                }
            });

            for turn in 0..4 {
                for device_id in 0..2 {
                    UpsertSession {
                        address: "fictional",
                        record: &[turn + 1],
                        device_id,
                    }
                    .execute(&mut conn)
                    .unwrap();
                    UpsertIdentity {
                        address: "fictional",
                        key: &[turn + 2; 32],
                        device_id,
                    }
                    .execute(&mut conn)
                    .unwrap();
                    UpsertSenderKey {
                        address: "fictional",
                        record: &[turn + 3],
                        device_id,
                    }
                    .execute(&mut conn)
                    .unwrap();
                    UpsertDeviceRegistry {
                        user_id: "fictional",
                        devices_json: "[]",
                        timestamp: i32::from(turn),
                        phash: (turn % 2 == 0).then_some("hash"),
                        device_id,
                        updated_at: i32::from(turn),
                        raw_id: (turn % 2 != 0).then_some(7),
                    }
                    .execute(&mut conn)
                    .unwrap();
                }
            }
            let queries = cached.lock().unwrap().clone();
            assert_eq!(
                queries.len(),
                4,
                "exactly one cache insertion per query shape"
            );
            for table in ["sessions", "identities", "sender_keys", "device_registry"] {
                assert_eq!(
                    queries
                        .iter()
                        .filter(|sql| sql.starts_with(&format!("INSERT INTO \"{table}\"")))
                        .count(),
                    1
                );
            }

            let failed: QueryResult<()> = conn.transaction(|conn| {
                UpsertSession {
                    address: "rolled-back",
                    record: b"valid",
                    device_id: 0,
                }
                .execute(conn)?;
                UpsertSession {
                    address: "invalid",
                    record: b"",
                    device_id: 0,
                }
                .execute(conn)?;
                Ok(())
            });
            assert!(matches!(
                failed,
                Err(diesel::result::Error::DatabaseError(..))
            ));
            UpsertSession {
                address: "fictional",
                record: b"after-error",
                device_id: 0,
            }
            .execute(&mut conn)
            .unwrap();
            assert_eq!(
                cached.lock().unwrap().len(),
                4,
                "failed execution must not evict/reprepare the statement"
            );

            #[derive(QueryableByName)]
            struct Row {
                #[diesel(sql_type = Text)]
                address: String,
                #[diesel(sql_type = Binary)]
                record: Vec<u8>,
                #[diesel(sql_type = Integer)]
                device_id: i32,
            }
            let rows = diesel::sql_query(
                "SELECT address, record, device_id FROM sessions ORDER BY device_id",
            )
            .load::<Row>(&mut conn)
            .unwrap();
            assert_eq!(
                rows.len(),
                2,
                "the failed transaction must leave no partial rows"
            );
            assert!(rows.iter().all(|r| r.address == "fictional"));
            assert_eq!(
                (rows[0].device_id, rows[0].record.as_slice()),
                (0, b"after-error".as_slice())
            );
            assert_eq!(
                (rows[1].device_id, rows[1].record.as_slice()),
                (1, [4].as_slice())
            );
        }
    }
}
