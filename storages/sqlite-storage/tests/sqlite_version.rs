// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! RUSTSEC-2026-0459: SQLite before 3.54.0 can serve stale page-cache bytes
//! after a WAL database is reopened with a different page size.
//!
//! Every native storage profile must link a fixed SQLite, including bundled
//! builds and builds that select a system library.
#![cfg(not(target_family = "wasm"))]

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Text;

#[derive(QueryableByName)]
struct Version {
    #[diesel(sql_type = Text)]
    version: String,
}

fn parse(version: &str) -> (u32, u32, u32) {
    let mut parts = version.split('.').map(|p| p.parse::<u32>());
    let mut next = || {
        parts
            .next()
            .and_then(Result::ok)
            .unwrap_or_else(|| panic!("unparseable SQLite version {version:?}"))
    };
    (next(), next(), next())
}

fn linked_version() -> String {
    let mut conn = SqliteConnection::establish(":memory:").expect("open in-memory database");
    let row: Version = sql_query("SELECT sqlite_version() AS version")
        .get_result(&mut conn)
        .expect("query sqlite_version()");
    row.version
}

#[test]
fn linked_sqlite_is_fixed() {
    let version = linked_version();
    assert!(
        parse(&version) >= (3, 54, 0),
        "linked SQLite {version} predates 3.54.0 and is affected by RUSTSEC-2026-0459"
    );
}

#[test]
fn versions_compare_by_number() {
    assert!(parse("3.54.1") >= (3, 54, 0));
    assert!(parse("3.100.0") >= (3, 54, 0));
    assert!(parse("3.50.4") < (3, 54, 0));
}
