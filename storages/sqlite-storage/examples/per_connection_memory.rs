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
    // smaps_rollup rather than statm: it reports `Rss:` already in kB, so the
    // reading does not depend on knowing the kernel's page size (which is not
    // always 4 KiB, and which std cannot report without libc).
    let rollup =
        std::fs::read_to_string("/proc/self/smaps_rollup").expect("/proc/self/smaps_rollup");
    let kib: u64 = rollup
        .lines()
        .find_map(|line| line.strip_prefix("Rss:"))
        .and_then(|value| value.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .expect("Rss: line");
    kib * 1024
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

/// `dir` rather than one database URL: each arm gets a database of its own, so
/// the second is not measured against the pages, WAL and rows the first left
/// behind — which would confound the comparison with run order.
async fn writes(dir: &std::path::Path, sessions: usize, read_pool_size: u32) {
    let config = || SqliteStoreConfig {
        read_pool_size,
        ..Default::default()
    };
    let url = |name: &str| dir.join(name).to_string_lossy().into_owned();

    let shared_url = url("writes_handles.db");
    let base = SqliteStore::with_config_for_device(&shared_url, 1, config())
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

    let separate_url = url("writes_pools.db");
    let mut separate = Vec::new();
    for device_id in 1..=sessions {
        separate.push(
            SqliteStore::with_config_for_device(&separate_url, device_id as i32, config())
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
    // Zero would divide the per-session figure by nothing, and `handles` would
    // still open its base store — a count nobody asked for.
    assert!(sessions >= 1, "sessions must be at least 1");

    // A directory of our own, created exclusively and owner-only: the create
    // fails outright if anything already sits at the path (a symlink included),
    // and mode 0o700 keeps it that way regardless of umask. Between them,
    // nobody else on the machine can pre-place or swap the database, WAL and
    // shm files this then writes.
    use std::os::unix::fs::DirBuilderExt as _;
    let dir = std::env::temp_dir().join(format!("wa_percon_{}", std::process::id()));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&dir)
        .expect("exclusive scratch directory");
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
        writes(&dir, sessions, cache_kib).await;
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

    let mut stores: Vec<SqliteStore> = Vec::with_capacity(sessions);
    let mut after_first = before;
    for n in 0..sessions {
        let device_id = n as i32 + 1;
        let store = match (mode, stores.first()) {
            ("pools", _) => SqliteStore::with_config_for_device(&url, device_id, config())
                .await
                .expect("open"),
            // The first handle is a real store; the rest hang off it.
            ("handles", None) => SqliteStore::with_config_for_device(&url, device_id, config())
                .await
                .expect("open"),
            ("handles", Some(base)) => base.share_for_device(device_id),
            (other, _) => panic!("unknown mode {other}"),
        };
        touch(&store).await;
        if warm {
            scan(&store).await;
        }
        stores.push(store);
        if n == 0 {
            // The *marginal* cost is the number that describes the batch, and
            // it is the one that survives a warmed allocator: whatever the
            // seeding connection left behind for the first session to reuse is
            // in this reading, so subtracting it takes the discount out of the
            // per-session figure instead of hiding in it.
            tokio::time::sleep(Duration::from_millis(200)).await;
            after_first = rss_bytes();
        }
    }

    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = rss_bytes();
    let delta = after.saturating_sub(before);
    let marginal = after.saturating_sub(after_first);
    let others = sessions.saturating_sub(1);
    println!(
        "mode={mode} sessions={sessions} cache_kib={cache_kib} warm={warm} \
         rss_delta={delta}B per_session={:.1}KiB marginal_per_session={}",
        delta as f64 / sessions as f64 / 1024.0,
        if others == 0 {
            // One session is all baseline and no margin; saying "0.0KiB" would
            // read as a measurement rather than the absence of one.
            "n/a".to_string()
        } else {
            format!("{:.1}KiB", marginal as f64 / others as f64 / 1024.0)
        }
    );

    drop(stores);
    drop(scratch);
}
