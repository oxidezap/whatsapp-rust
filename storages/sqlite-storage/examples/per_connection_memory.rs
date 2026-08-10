//! What one extra SQLite connection costs, measured rather than estimated.
//!
//! A process that holds N WhatsApp sessions against the *same* database file
//! opens N stores, and every constructor builds its own r2d2 pool — so N
//! connections. This harness prices that: it seeds one database, then opens N
//! sessions two ways and reports the resident-set delta per session.
//!
//! ```text
//! cargo run -p whatsapp-rust-sqlite-storage --release \
//!     --example per_connection_memory -- <pools|handles> <sessions> <cache_kib> [warm]
//! cargo run -p whatsapp-rust-sqlite-storage --release \
//!     --example per_connection_memory -- writes <sessions> <read_pool_size>
//! cargo run -p whatsapp-rust-sqlite-storage --release \
//!     --example per_connection_memory -- compile-options
//! ```
//!
//! * `pools` — one `SqliteStore::new_for_device` per session (today's shape).
//! * `handles` — one store, then `share_for_device` per session (one pool).
//! * `warm` — every session scans the seeded table first, filling its page
//!   cache to the `cache_kib` cap. Without it each session only does the small
//!   reads an idle session does, which is the realistic steady state.
//! * `writes` — the other side of the trade: every session writes at once,
//!   both ways, reporting total time and the spread between the fastest and
//!   slowest session (i.e. whether anyone starves).
//!
//! Resident set, not a Rust allocator counter: SQLite's page cache and
//! lookaside are `sqlite3_malloc` allocations from the bundled C library, which
//! a `GlobalAlloc` wrapper never sees. RSS is coarse (page-granular, and
//! includes whatever the allocator declines to return), so it is read after a
//! settle and divided across enough sessions for the per-session figure to
//! outweigh the noise. Linux only.

// The numbers *are* this binary's output; there is no logger to route them
// through, and a measurement harness whose result lands in a log filter would
// be worse than useless.
#![allow(clippy::print_stdout)]

use std::time::Duration;

use diesel::prelude::*;
use wacore::store::traits::SignalStore as _;
use whatsapp_rust_sqlite_storage::{SqliteStore, SqliteStoreConfig};

/// Rows of ~1 KiB each: enough database that a full scan can fill a 512 KiB
/// page cache several times over, so the cap is what bounds a warm connection.
const SEED_ROWS: usize = 4_000;
const ROW_BYTES: usize = 1_024;

fn rss_bytes() -> u64 {
    // Field 2 of /proc/self/statm is resident pages. 4 KiB is the page size on
    // every platform this harness is meant to run on; a wrong constant would
    // scale every number here equally, so comparisons stay valid regardless.
    let statm = std::fs::read_to_string("/proc/self/statm").expect("/proc/self/statm");
    let pages: u64 = statm
        .split_whitespace()
        .nth(1)
        .and_then(|f| f.parse().ok())
        .expect("resident pages");
    pages * 4096
}

fn db_err(e: diesel::result::Error) -> wacore::store::error::StoreError {
    wacore::store::error::StoreError::Database(Box::new(e))
}

/// Seed a database large enough that page caches have something to hold.
async fn seed(url: &str) {
    let store = SqliteStore::new_for_device(url, 1).await.expect("open");
    let record = vec![0x5au8; ROW_BYTES];
    for chunk in (0..SEED_ROWS).collect::<Vec<_>>().chunks(200) {
        let batch: Vec<_> = chunk
            .iter()
            .map(|i| {
                (
                    format!("seed.{i}:0").into(),
                    bytes::Bytes::from(record.clone()),
                )
            })
            .collect();
        store.put_sessions_batch(&batch).await.expect("seed write");
    }
}

/// The reads an idle session actually does on connect: a couple of point
/// lookups. Also forces r2d2 to open the connection, which is the allocation
/// this harness is pricing.
async fn touch(store: &SqliteStore) {
    store.get_session("seed.0:0").await.expect("point read");
    store.get_session("seed.1:0").await.expect("point read");
}

/// A full table scan, which pulls pages in until the cache cap stops it.
async fn scan(store: &SqliteStore) {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        n: i64,
    }
    store
        .shared()
        .read(|conn| {
            diesel::sql_query("SELECT count(record) AS n FROM sessions")
                .get_result::<Count>(conn)
                .map(|c| c.n)
                .map_err(db_err)
        })
        .await
        .expect("scan");
}

/// Every session issues `WRITES` writes at once. Returns the wall clock for
/// the whole burst and each session's own duration, so a mode that finishes
/// quickly by letting one session hog the lock is visible as a wide spread.
async fn write_burst(stores: Vec<SqliteStore>) -> (Duration, Duration, Duration) {
    const WRITES: usize = 200;
    let started = wacore::time::Instant::now();
    let mut tasks = Vec::new();
    for (n, store) in stores.into_iter().enumerate() {
        tasks.push(tokio::spawn(async move {
            let session_started = wacore::time::Instant::now();
            for i in 0..WRITES {
                store
                    .put_session(&format!("peer.{n}.{i}:0"), &[n as u8; 256])
                    .await
                    .expect("write");
            }
            session_started.elapsed()
        }));
    }
    let mut per_session = Vec::new();
    for task in tasks {
        per_session.push(task.await.expect("join"));
    }
    (
        started.elapsed(),
        per_session.iter().copied().min().unwrap_or_default(),
        per_session.iter().copied().max().unwrap_or_default(),
    )
}

async fn writes(url: &str, sessions: usize, read_pool_size: u32) {
    let config = || SqliteStoreConfig {
        read_pool_size,
        ..Default::default()
    };

    let base = SqliteStore::with_config_for_device(url, 1, config())
        .await
        .expect("open");
    let mut fleet = vec![base.clone()];
    for device_id in 2..=sessions {
        fleet.push(base.share_for_device(device_id as i32));
    }
    let (total, fastest, slowest) = write_burst(fleet).await;
    println!(
        "handles sessions={sessions} read_pool_size={read_pool_size} \
         total={total:?} fastest={fastest:?} slowest={slowest:?}"
    );

    let mut separate = Vec::new();
    for device_id in 1..=sessions {
        separate.push(
            SqliteStore::with_config_for_device(url, device_id as i32, config())
                .await
                .expect("open"),
        );
    }
    let (total, fastest, slowest) = write_burst(separate).await;
    println!(
        "pools   sessions={sessions} read_pool_size={read_pool_size} \
         total={total:?} fastest={fastest:?} slowest={slowest:?}"
    );
}

async fn compile_options(url: &str) {
    let store = SqliteStore::new(url).await.expect("open");
    #[derive(QueryableByName)]
    struct Opt {
        #[diesel(sql_type = diesel::sql_types::Text)]
        compile_options: String,
    }
    let opts: Vec<Opt> = store
        .shared()
        .run(|conn| {
            diesel::sql_query("PRAGMA compile_options")
                .load(conn)
                .map_err(db_err)
        })
        .await
        .expect("compile_options");
    for opt in opts {
        println!("{}", opt.compile_options);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mode = args.first().map(String::as_str).unwrap_or("pools");
    let sessions: usize = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(50);
    let cache_kib: u32 = args.get(2).and_then(|a| a.parse().ok()).unwrap_or(512);
    let warm = args.iter().any(|a| a == "warm");

    // A directory of our own, created exclusively: `create_dir` fails outright
    // if anything already sits at the path (a symlink included), so nobody who
    // can write to the shared temp directory can pre-place one and redirect the
    // database, WAL and shm files this then writes.
    let dir = std::env::temp_dir().join(format!("wa_percon_{}", std::process::id()));
    std::fs::create_dir(&dir).expect("exclusive scratch directory");
    // A guard, not a tail cleanup: two of the modes below return early.
    struct ScratchDir(std::path::PathBuf);
    impl Drop for ScratchDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let scratch = ScratchDir(dir.clone());
    let url = dir.join("bench.db").to_string_lossy().into_owned();

    if mode == "compile-options" {
        compile_options(&url).await;
        return;
    }
    if mode == "writes" {
        // args[2] is the reader-pool size here, not a cache size.
        writes(&url, sessions, cache_kib).await;
        return;
    }

    seed(&url).await;
    let config = || SqliteStoreConfig {
        cache_size_kib: cache_kib,
        ..Default::default()
    };

    // Settle: the seeding store is dropped, and its connection with it, so the
    // baseline is the process without any session on this database.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let before = rss_bytes();

    let mut stores = Vec::with_capacity(sessions);
    match mode {
        "pools" => {
            for device_id in 0..sessions {
                let store =
                    SqliteStore::with_config_for_device(&url, device_id as i32 + 1, config())
                        .await
                        .expect("open");
                touch(&store).await;
                stores.push(store);
            }
        }
        "handles" => {
            let base = SqliteStore::with_config_for_device(&url, 1, config())
                .await
                .expect("open");
            touch(&base).await;
            for device_id in 1..sessions {
                stores.push(base.share_for_device(device_id as i32 + 1));
            }
            stores.push(base);
        }
        other => panic!("unknown mode {other}"),
    }
    if warm {
        for store in &stores {
            scan(store).await;
        }
    }

    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = rss_bytes();
    let delta = after.saturating_sub(before);
    println!(
        "mode={mode} sessions={sessions} cache_kib={cache_kib} warm={warm} \
         rss_delta={delta}B per_session={:.1}KiB",
        delta as f64 / sessions as f64 / 1024.0
    );

    drop(stores);
    drop(scratch);
}
