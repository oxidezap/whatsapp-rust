// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! RUSTSEC-2026-0459: SQLite before 3.54.0 can serve stale page-cache bytes
//! after a WAL database is reopened with a different page size.
//!
//! Diesel 2.3 caps `libsqlite3-sys` below 0.39, whose bundled SQLite is still
//! affected. A build that links an external SQLite, either without the
//! `bundled-sqlite` feature or with `LIBSQLITE3_SYS_USE_PKG_CONFIG` (as
//! `cargo xt fixed-sqlite` sets up), must link 3.54 or newer; this test
//! enforces that. With the bundled copy it reports the version and passes.
//! The wasm backend links sqlite-wasm-rs 0.6.3, which bundles 3.54.1.
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

/// Whether this build links an external SQLite rather than the copy bundled
/// with `libsqlite3-sys`: bundling is off, or the environment overrides it.
fn links_own_sqlite() -> bool {
    !cfg!(feature = "bundled-sqlite")
        || option_env!("LIBSQLITE3_SYS_USE_PKG_CONFIG").is_some_and(|value| value != "0")
}

#[test]
fn a_build_that_links_its_own_sqlite_links_a_fixed_one() {
    let version = linked_version();
    println!("linked SQLite {version}");
    if !links_own_sqlite() {
        println!(
            "this build links the SQLite bundled with libsqlite3-sys, which is affected by \
             RUSTSEC-2026-0459; run `cargo xt fixed-sqlite` to link 3.54.1 instead"
        );
        return;
    }
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
