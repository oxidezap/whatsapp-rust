//! Before/after of the `msg_secrets` storage changes, on a representative
//! dataset, for a reviewer to reproduce.
//!
//! It builds two databases with the same rows: the **legacy** shape (a
//! `created_at` column and a non-partial `(device_id, expires_at)` index) and
//! the **current** shape (`created_at` gone, `(device_id, expires_at) WHERE
//! expires_at <> 0`). Both are populated with a busy-bot mix — a share of
//! never-expire rows and the rest with deadlines spread across devices — then
//! it reports, per shape:
//!
//! * `dbstat` bytes for the table, its primary-key autoindex, and the expiry
//!   index;
//! * the `EXPLAIN QUERY PLAN` for the prune and the point lookup;
//! * wall time for a prune that deletes the expired rows, and for N point
//!   lookups.
//!
//! ```text
//! cargo run -p whatsapp-rust-sqlite-storage --release \
//!     --example msg_secret_storage -- [rows] [never_percent] [devices]
//! ```
//!
//! Defaults: 400_000 rows, 15% never-expire, 2 devices.

// A measurement harness: prints are its output. Same allow as the sibling
// `per_connection_memory` example.
#![allow(clippy::print_stdout, clippy::print_stderr)]

use wacore::time::Instant;

use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

const SHARED_COLUMNS: &str = "chat TEXT NOT NULL, sender TEXT NOT NULL, msg_id TEXT NOT NULL, \
     secret BLOB NOT NULL, device_id INTEGER NOT NULL, expires_at INTEGER NOT NULL, \
     message_ts INTEGER NOT NULL";

fn main() {
    let rows: usize = arg(1, 400_000);
    if rows < 8 {
        eprintln!("rows must be at least 8, got {rows}");
        std::process::exit(2);
    }
    let never_percent: usize = arg(2, 15).min(100);
    let devices: i64 = arg(3, 2).max(1) as i64;
    let db_dir = std::env::temp_dir();
    let now: i64 = 1_800_000_000;

    println!("rows={rows} never={never_percent}% devices={devices} page_size=4096\n");

    for (label, partial) in [("legacy", false), ("current", true)] {
        let path = db_dir.join(format!("wa_msg_secret_storage_{label}.db"));
        let _ = std::fs::remove_file(&path);
        let url = path.to_string_lossy().into_owned();
        let mut conn = SqliteConnection::establish(&url).expect("open");

        conn.batch_execute(
            "PRAGMA page_size = 4096;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;",
        )
        .expect("pragmas");

        let created_column = if partial {
            ""
        } else {
            "created_at INTEGER NOT NULL, "
        };
        let index_predicate = if partial {
            " WHERE expires_at <> 0"
        } else {
            ""
        };
        conn.batch_execute(&format!(
            "CREATE TABLE msg_secrets ({created_column}{SHARED_COLUMNS}, \
             PRIMARY KEY (chat, sender, msg_id, device_id));\
             CREATE INDEX idx_msg_secrets_expires ON msg_secrets (device_id, expires_at){index_predicate};"
        ))
        .expect("schema");

        // Row-at-a-time inserts inside one transaction is the honest shape of
        // the live seed path, minus the per-row commit the write buffer
        // already coalesces.
        conn.batch_execute("BEGIN;").expect("begin");
        seed(&mut conn, rows, never_percent, devices, now, !partial);
        conn.batch_execute("COMMIT;").expect("commit");
        let _ = diesel::sql_query("PRAGMA wal_checkpoint(TRUNCATE);").execute(&mut conn);

        let table = dbstat(&mut conn, "msg_secrets");
        let pk = dbstat(&mut conn, "sqlite_autoindex_msg_secrets_1");
        let idx = dbstat(&mut conn, "idx_msg_secrets_expires");
        println!("== {label} ==");
        if table + pk + idx == 0 {
            println!(
                "  (this SQLite build has no dbstat vtab; read table/index bytes with the\n   \
                 sqlite3 CLI: sqlite3 <file> \"SELECT name, SUM(pgsize) FROM dbstat GROUP BY name\")"
            );
        }
        println!(
            "  table={table:>10}  pk_autoindex={pk:>10}  expires_index={idx:>9}  \
             (schema+index total {})",
            table + pk + idx
        );
        println!(
            "  page_count={}  freelist={}",
            pragma_i64(&mut conn, "page_count"),
            pragma_i64(&mut conn, "freelist_count")
        );
        let file_bytes = std::fs::metadata(&url).map(|m| m.len()).unwrap_or(0);
        println!("  main file bytes: {file_bytes}");

        for (name, sql) in [
            (
                "prune",
                "DELETE FROM msg_secrets WHERE device_id = 1 AND expires_at <> 0 AND expires_at <= 1800000000",
            ),
            (
                "lookup",
                "SELECT secret, message_ts FROM msg_secrets WHERE chat = 'c' AND sender = 's' AND msg_id = 'M' AND device_id = 1",
            ),
        ] {
            let plan = explain(&mut conn, sql);
            println!("  {name} plan: {plan}");
        }

        let deleted = diesel::sql_query(
            "DELETE FROM msg_secrets WHERE device_id = 1 AND expires_at <> 0 AND expires_at <= 1800000000",
        )
        .execute(&mut conn)
        .expect("prune");
        let _ = diesel::sql_query("PRAGMA wal_checkpoint(TRUNCATE);").execute(&mut conn);
        println!("  prune deleted {deleted} rows");

        // Point lookups on rows that exist on device 1.
        let started = Instant::now();
        let mut hits = 0usize;
        for i in 0..50_000usize {
            let chat = format!("5511{:09}@s.whatsapp.net", i % (rows / 8));
            let msg = format!("3EB0{i:016X}");
            #[derive(QueryableByName)]
            struct Hit {
                #[diesel(sql_type = diesel::sql_types::Integer)]
                n: i32,
            }
            let got: Hit = diesel::sql_query(
                "SELECT count(*) AS n FROM msg_secrets WHERE chat = ?1 AND sender = ?1 \
                 AND msg_id = ?2 AND device_id = 1",
            )
            .bind::<diesel::sql_types::Text, _>(&chat)
            .bind::<diesel::sql_types::Text, _>(&msg)
            .get_result(&mut conn)
            .expect("lookup");
            hits += got.n as usize;
        }
        println!(
            "  50k point lookups in {:?} (hits {hits})\n",
            started.elapsed()
        );
    }
}

fn seed(
    conn: &mut SqliteConnection,
    rows: usize,
    never_percent: usize,
    devices: i64,
    now: i64,
    with_created_at: bool,
) {
    for i in 0..rows {
        let chat = format!("5511{:09}@s.whatsapp.net", i % (rows / 8));
        let sender = if i % 3 == 0 {
            format!("1203630{:06}@g.us", i % 20_000)
        } else {
            chat.clone()
        };
        let msg = format!("3EB0{i:016X}");
        let device = (i as i64 % devices) + 1;
        let expires_at = if i % 100 < never_percent {
            0
        } else {
            now - 2_000_000 + (i as i64 % 4_000_000)
        };
        let secret = vec![0x5Au8; 32];
        if with_created_at {
            diesel::sql_query(
                "INSERT INTO msg_secrets (chat, sender, msg_id, secret, device_id, created_at, expires_at, message_ts) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0)",
            )
            .bind::<diesel::sql_types::Text, _>(&chat)
            .bind::<diesel::sql_types::Text, _>(&sender)
            .bind::<diesel::sql_types::Text, _>(&msg)
            .bind::<diesel::sql_types::Binary, _>(&secret)
            .bind::<diesel::sql_types::BigInt, _>(device)
            .bind::<diesel::sql_types::BigInt, _>(now)
            .bind::<diesel::sql_types::BigInt, _>(expires_at)
            .execute(conn)
            .expect("insert");
        } else {
            diesel::sql_query(
                "INSERT INTO msg_secrets (chat, sender, msg_id, secret, device_id, expires_at, message_ts) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            )
            .bind::<diesel::sql_types::Text, _>(&chat)
            .bind::<diesel::sql_types::Text, _>(&sender)
            .bind::<diesel::sql_types::Text, _>(&msg)
            .bind::<diesel::sql_types::Binary, _>(&secret)
            .bind::<diesel::sql_types::BigInt, _>(device)
            .bind::<diesel::sql_types::BigInt, _>(expires_at)
            .execute(conn)
            .expect("insert");
        }
    }
}

fn dbstat(conn: &mut SqliteConnection, name: &str) -> i64 {
    #[derive(QueryableByName)]
    struct Bytes {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        bytes: i64,
    }
    diesel::sql_query(format!(
        "SELECT COALESCE(SUM(pgsize), 0) AS bytes FROM dbstat WHERE name = '{}'",
        name.replace('\'', "''")
    ))
    .get_result(conn)
    .map(|b: Bytes| b.bytes)
    .unwrap_or(0)
}

fn pragma_i64(conn: &mut SqliteConnection, pragma: &str) -> i64 {
    // Every integer PRAGMA returns its result under a column named after the
    // pragma itself, so the name is built the same way for both.
    #[derive(QueryableByName)]
    struct Value {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        #[diesel(column_name = val)]
        val: i64,
    }
    let name = pragma.split('(').next().unwrap_or(pragma);
    diesel::sql_query(format!("SELECT {name} AS val FROM pragma_{name};"))
        .get_result::<Value>(conn)
        .map(|v| v.val)
        .unwrap_or(-1)
}

fn explain(conn: &mut SqliteConnection, sql: &str) -> String {
    #[derive(QueryableByName)]
    struct Detail {
        #[diesel(sql_type = diesel::sql_types::Text)]
        detail: String,
    }
    diesel::sql_query(format!("EXPLAIN QUERY PLAN {sql}"))
        .load::<Detail>(conn)
        .map(|rows| {
            rows.into_iter()
                .map(|r| r.detail)
                .collect::<Vec<_>>()
                .join(" | ")
        })
        .unwrap_or_else(|e| format!("<{e}>"))
}

fn arg(index: usize, default: usize) -> usize {
    std::env::args()
        .nth(index)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}
