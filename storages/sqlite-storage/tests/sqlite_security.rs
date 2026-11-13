#![cfg(all(
    feature = "bundled-sqlite",
    not(all(target_family = "wasm", target_os = "unknown"))
))]

use diesel::{Connection, RunQueryDsl, SqliteConnection};

#[derive(diesel::QueryableByName)]
struct SqliteVersion {
    #[diesel(sql_type = diesel::sql_types::Text)]
    version: String,
}

#[test]
fn default_bundled_sqlite_contains_the_wal_page_size_fix() {
    let mut connection = SqliteConnection::establish(":memory:").unwrap();
    let row: SqliteVersion = diesel::sql_query("SELECT sqlite_version() AS version")
        .get_result(&mut connection)
        .unwrap();
    eprintln!("linked SQLite {}", row.version);
    let mut parts = row
        .version
        .split('.')
        .map(|part| part.parse::<u32>().expect("numeric SQLite version"));
    let version = (
        parts.next().expect("major version"),
        parts.next().expect("minor version"),
        parts.next().expect("patch version"),
    );
    assert!(parts.next().is_none());
    assert!(
        version >= (3, 54, 0),
        "linked SQLite {} predates the WAL page-size fix in 3.54.0",
        row.version
    );
}
