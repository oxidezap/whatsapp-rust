use diesel::ExpressionMethods;
use diesel::connection::{Connection, SimpleConnection};
use diesel::migration::{MigrationConnection, MigrationVersion};
use diesel::query_dsl::RunQueryDsl;
use diesel::sql_types::VarChar;
use diesel::sqlite::SqliteConnection;
use std::collections::HashSet;
use std::error::Error;
use std::fmt;
use wacore::store::error::{Result, StoreError};

diesel::table! {
    __diesel_schema_migrations (version) {
        version -> VarChar,
        run_on -> Timestamp,
    }
}

#[derive(Clone, Copy)]
struct SqliteMigration {
    version: &'static str,
    name: &'static str,
    up_sql: &'static str,
    run_in_transaction: bool,
}

macro_rules! sqlite_migration {
    ($version:literal, $name:literal) => {
        SqliteMigration {
            version: $version,
            name: $name,
            up_sql: include_str!(concat!("../migrations/", $name, "/up.sql")),
            run_in_transaction: true,
        }
    };
}

// Diesel's embedded migrations retain down.sql and TOML metadata at runtime.
// This store only applies upgrades; the test-only Diesel harness remains the
// reference for migration fixtures and historical database setup.
const SQLITE_MIGRATIONS: &[SqliteMigration] = &[
    sqlite_migration!("20250814035031", "2025-08-14-035031_initial"),
    sqlite_migration!(
        "202509230322320000",
        "2025-09-23-032232-0000_add_multi_account_support"
    ),
    sqlite_migration!("20251204000000", "2025-12-04-000000_add_skdm_recipients"),
    sqlite_migration!("20251211000000", "2025-12-11-000000_add_lid_pn_mapping"),
    sqlite_migration!(
        "20251224000000",
        "2025-12-24-000000_add_whatsapp_web_alignment"
    ),
    sqlite_migration!("20260205000000", "2026-02-05-000000_add_props_hash"),
    sqlite_migration!("20260212000000", "2026-02-12-000000_add_tc_tokens"),
    sqlite_migration!("20260312000000", "2026-03-12-000000_add_next_pre_key_id"),
    sqlite_migration!("20260315000000", "2026-03-15-000000_add_sent_messages"),
    sqlite_migration!("20260323000000", "2026-03-23-000000_add_nct_salt"),
    sqlite_migration!(
        "20260326000000",
        "2026-03-26-000000_unified_sender_key_devices"
    ),
    sqlite_migration!(
        "20260331000000",
        "2026-03-31-000000_add_device_registry_raw_id"
    ),
    sqlite_migration!("20260406171140", "2026-04-06-171140_add_server_has_prekeys"),
    sqlite_migration!("20260426000000", "2026-04-26-000000_add_server_cert_chain"),
    sqlite_migration!("20260513000000", "2026-05-13-000000_add_login_counter"),
    sqlite_migration!("20260528000000", "2026-05-28-000000_add_msg_secrets"),
    sqlite_migration!("20260531000000", "2026-05-31-000000_msg_secret_expires_at"),
    sqlite_migration!("20260601000000", "2026-06-01-000000_add_group_metadata"),
    sqlite_migration!(
        "20260610000000",
        "2026-06-10-000000_add_first_unupload_pk_id"
    ),
    sqlite_migration!(
        "20260626000000",
        "2026-06-26-000000_add_pending_inbound_messages"
    ),
    sqlite_migration!("20260702000000", "2026-07-02-000000_add_lid_migrated"),
    sqlite_migration!(
        "20260703000000",
        "2026-07-03-000000_add_signed_prekey_rotation"
    ),
    sqlite_migration!(
        "20260703000001",
        "2026-07-03-000001_add_read_receipts_privacy"
    ),
    sqlite_migration!("20260816000000", "2026-08-16-000000_add_client_expiration"),
    sqlite_migration!(
        "20260831000000",
        "2026-08-31-000000_normalize_legacy_user_server"
    ),
    sqlite_migration!(
        "20260903000000",
        "2026-09-03-000000_drop_redundant_device_id_indexes"
    ),
    sqlite_migration!(
        "20260903000001",
        "2026-09-03-000001_drop_dead_device_registry_indexes"
    ),
    sqlite_migration!(
        "20260904000000",
        "2026-09-04-000000_index_base_keys_created_at"
    ),
    sqlite_migration!(
        "20260904000001",
        "2026-09-04-000001_drop_remaining_device_id_indexes"
    ),
    sqlite_migration!(
        "20260904000002",
        "2026-09-04-000002_index_sender_key_devices_by_device_jid"
    ),
    sqlite_migration!(
        "20260916000000",
        "2026-09-16-000000_drop_msg_secrets_created_at"
    ),
    sqlite_migration!(
        "20260916000001",
        "2026-09-16-000001_msg_secrets_partial_expiry_index"
    ),
    sqlite_migration!("20260929000000", "2026-09-29-000000_add_status_privacy"),
    sqlite_migration!(
        "20261105000000",
        "2026-11-05-000000_add_mobile_session_binding"
    ),
];

#[derive(diesel::QueryableByName)]
struct AppliedMigration {
    #[diesel(sql_type = VarChar)]
    version: String,
}

#[derive(Debug)]
struct MigrationSqlError {
    name: &'static str,
    source: diesel::result::Error,
}

impl fmt::Display for MigrationSqlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Failed to run {} with: {}",
            self.name, self.source
        )
    }
}

impl Error for MigrationSqlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

pub(crate) fn run_pending(connection: &mut SqliteConnection) -> Result<()> {
    <SqliteConnection as MigrationConnection>::setup(connection)
        .map_err(|error| StoreError::Migration(Box::new(error)))?;

    let applied =
        diesel::sql_query("SELECT version FROM __diesel_schema_migrations ORDER BY version DESC")
            .load::<AppliedMigration>(connection)
            .map_err(|error| StoreError::Migration(Box::new(error)))?
            .into_iter()
            .map(|row| row.version)
            .collect::<HashSet<_>>();

    for migration in SQLITE_MIGRATIONS
        .iter()
        .filter(|migration| !applied.contains(migration.version))
    {
        apply_migration(connection, migration).map_err(StoreError::Migration)?;
    }

    Ok(())
}

fn apply_migration(
    connection: &mut SqliteConnection,
    migration: &SqliteMigration,
) -> std::result::Result<(), Box<dyn Error + Send + Sync>> {
    let run = |connection: &mut SqliteConnection| {
        connection
            .batch_execute(migration.up_sql)
            .map_err(|source| {
                Box::new(MigrationSqlError {
                    name: migration.name,
                    source,
                }) as Box<dyn Error + Send + Sync>
            })?;
        diesel::insert_into(__diesel_schema_migrations::table)
            .values(
                __diesel_schema_migrations::version.eq(MigrationVersion::from(migration.version)),
            )
            .execute(connection)
            .map_err(|error| Box::new(error) as Box<dyn Error + Send + Sync>)?;
        Ok(())
    };

    if migration.run_in_transaction {
        connection.transaction(run)
    } else {
        run(connection)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::migration::MigrationSource;
    use diesel::sql_types::Text;
    use diesel_migrations::{EmbeddedMigrations, MigrationHarness, embed_migrations};

    const REFERENCE_MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

    #[test]
    fn migration_versions_match_diesel_and_stay_sorted() {
        let migrations =
            <EmbeddedMigrations as MigrationSource<diesel::sqlite::Sqlite>>::migrations(
                &REFERENCE_MIGRATIONS,
            )
            .unwrap();
        let mut reference = migrations
            .iter()
            .map(|migration| {
                (
                    migration.name().version().to_string(),
                    migration.name().to_string(),
                    migration.metadata().run_in_transaction(),
                )
            })
            .collect::<Vec<_>>();
        reference.sort_by(|a, b| a.0.cmp(&b.0));

        let candidate = SQLITE_MIGRATIONS
            .iter()
            .map(|migration| {
                (
                    migration.version.to_owned(),
                    migration.name.to_owned(),
                    migration.run_in_transaction,
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(candidate, reference);
        assert!(candidate.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn fresh_database_ledger_and_schema_match_diesel_and_reopen_is_idempotent() {
        let mut reference = SqliteConnection::establish(":memory:").unwrap();
        let mut candidate = SqliteConnection::establish(":memory:").unwrap();
        reference
            .run_pending_migrations(REFERENCE_MIGRATIONS)
            .unwrap();
        run_pending(&mut candidate).unwrap();

        assert_eq!(
            schema_snapshot(&mut candidate),
            schema_snapshot(&mut reference)
        );
        let expected = ledger_versions(&mut reference);
        assert_eq!(ledger_versions(&mut candidate), expected);

        run_pending(&mut candidate).unwrap();
        assert_eq!(ledger_versions(&mut candidate), expected);
    }

    #[test]
    fn migration_insert_has_a_static_statement_cache_key() {
        fn has_static_query_id<Q: diesel::query_builder::QueryId>(_: &Q) -> bool {
            Q::HAS_STATIC_QUERY_ID
        }

        let query = diesel::insert_into(__diesel_schema_migrations::table).values(
            __diesel_schema_migrations::version.eq(MigrationVersion::from("20250814035031")),
        );
        assert!(has_static_query_id(&query));
    }

    #[test]
    fn failed_migration_rolls_back_ddl_and_ledger_insert() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        <SqliteConnection as MigrationConnection>::setup(&mut connection).unwrap();
        let migration = SqliteMigration {
            version: "20991231000000",
            name: "2099-12-31-000000_fail_after_ddl",
            up_sql: "CREATE TABLE rollback_probe (id INTEGER PRIMARY KEY); SELECT FROM;",
            run_in_transaction: true,
        };

        let error =
            StoreError::Migration(apply_migration(&mut connection, &migration).unwrap_err());
        assert!(matches!(&error, StoreError::Migration(_)));
        let migration_error = Error::source(&error).unwrap();
        assert!(migration_error.to_string().contains(migration.name));
        assert!(migration_error.to_string().contains("syntax error"));
        let source = Error::source(migration_error).unwrap();
        assert!(source.downcast_ref::<diesel::result::Error>().is_some());

        let table_count: i64 = diesel::sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'rollback_probe'",
        )
        .get_result::<Count>(&mut connection)
        .unwrap()
        .count;
        assert_eq!(table_count, 0);
        let version_count: i64 = diesel::sql_query(
            "SELECT COUNT(*) AS count FROM __diesel_schema_migrations WHERE version = ?",
        )
        .bind::<VarChar, _>(migration.version)
        .get_result::<Count>(&mut connection)
        .unwrap()
        .count;
        assert_eq!(version_count, 0);
    }

    #[test]
    fn failed_ledger_insert_rolls_back_successful_migration_sql() {
        let mut connection = SqliteConnection::establish(":memory:").unwrap();
        <SqliteConnection as MigrationConnection>::setup(&mut connection).unwrap();
        let migration = SqliteMigration {
            version: "20991231000000",
            name: "2099-12-31-000000_duplicate_ledger_version",
            up_sql: "CREATE TABLE rollback_probe (id INTEGER PRIMARY KEY);",
            run_in_transaction: true,
        };
        diesel::insert_into(__diesel_schema_migrations::table)
            .values(
                __diesel_schema_migrations::version.eq(MigrationVersion::from(migration.version)),
            )
            .execute(&mut connection)
            .unwrap();

        let error =
            StoreError::Migration(apply_migration(&mut connection, &migration).unwrap_err());
        let source = Error::source(&error).unwrap();
        assert!(source.to_string().contains("UNIQUE constraint failed"));
        assert!(source.downcast_ref::<diesel::result::Error>().is_some());

        let table_count: i64 = diesel::sql_query(
            "SELECT COUNT(*) AS count FROM sqlite_master WHERE type = 'table' AND name = 'rollback_probe'",
        )
        .get_result::<Count>(&mut connection)
        .unwrap()
        .count;
        assert_eq!(table_count, 0);
        assert_eq!(
            ledger_versions(&mut connection),
            vec![migration.version.to_owned()]
        );
    }

    #[derive(diesel::QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }

    #[derive(diesel::QueryableByName)]
    struct SchemaRow {
        #[diesel(sql_type = Text)]
        object_type: String,
        #[diesel(sql_type = Text)]
        name: String,
        #[diesel(sql_type = Text)]
        sql: String,
    }

    fn schema_snapshot(connection: &mut SqliteConnection) -> Vec<(String, String, String)> {
        diesel::sql_query(
            "SELECT type AS object_type, name, COALESCE(sql, '') AS sql \
             FROM sqlite_master ORDER BY type, name",
        )
        .load::<SchemaRow>(connection)
        .unwrap()
        .into_iter()
        .map(|row| (row.object_type, row.name, row.sql))
        .collect()
    }

    fn ledger_versions(connection: &mut SqliteConnection) -> Vec<String> {
        diesel::sql_query("SELECT version FROM __diesel_schema_migrations ORDER BY version")
            .load::<AppliedMigration>(connection)
            .unwrap()
            .into_iter()
            .map(|row| row.version)
            .collect()
    }
}
