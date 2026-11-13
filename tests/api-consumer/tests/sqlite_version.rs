#![cfg(target_arch = "wasm32")]

use diesel::prelude::*;
use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[derive(QueryableByName)]
struct SqliteBuild {
    #[diesel(sql_type = diesel::sql_types::Text)]
    version: String,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    no_load_extensions: i32,
    #[diesel(sql_type = diesel::sql_types::Integer)]
    single_threaded: i32,
}

#[wasm_bindgen_test]
fn browser_links_sqlite_with_the_wal_page_size_fix() {
    let mut connection = SqliteConnection::establish(":memory:").expect("open browser SQLite");
    let row: SqliteBuild = diesel::sql_query(
        "SELECT sqlite_version() AS version, \
         sqlite_compileoption_used('OMIT_LOAD_EXTENSION') AS no_load_extensions, \
         sqlite_compileoption_used('THREADSAFE=0') AS single_threaded",
    )
    .get_result(&mut connection)
    .expect("query the linked browser SQLite");
    wasm_bindgen_test::console_log!("linked browser SQLite {}", row.version);
    let version: [u32; 3] = row
        .version
        .split('.')
        .map(|part| part.parse().expect("numeric SQLite version"))
        .collect::<Vec<_>>()
        .try_into()
        .expect("SQLite version has three components");
    assert!(
        version >= [3, 54, 0],
        "browser SQLite {} predates the WAL page-size fix in 3.54.0",
        row.version
    );
    assert_eq!(row.no_load_extensions, 1, "browser SQLite must omit dlopen");
    assert_eq!(row.single_threaded, 1, "browser calls stay single-threaded");
}
