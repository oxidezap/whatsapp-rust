//! Native, real-SQLite evaluation of r2d2 reader retirement. No mock connections.

use super::read_routing_tests::TempDb;
use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

fn db_error(error: DieselError) -> StoreError {
    StoreError::Database(Box::new(error))
}

/// Same writer, options, semaphore and read API as production. Only the reader
/// idle timeout is accelerated: r2d2 still uses its real 30-second reaper.
async fn store_with_readers(
    url: &str,
    adaptive: bool,
    idle: Duration,
    readers: u32,
    hook: Option<ConnectionInitHook>,
) -> SqliteStore {
    let config = SqliteDatabaseConfig::default();
    let mut store = crate::SqliteDatabase::open(
        url,
        SqliteDatabaseConfig {
            read_pool_size: 0,
            ..config.clone()
        },
    )
    .await
    .expect("writer and migrations")
    .store(1);
    let options = ConnectionOptions {
        cache_size_kib: config.cache_size_kib,
        mmap_size: config.mmap_size,
        busy_timeout_ms: config.busy_timeout.as_millis() as u64,
        synchronous: config.synchronous,
        connection_init: hook,
        query_only: true,
    };
    let manager = ConnectionManager::<SqliteConnection>::new(url);
    let pool = crate::pool::spawn_blocking(move || {
        let mut builder = crate::pool::builder(None)
            .max_size(readers)
            .idle_timeout(Some(idle))
            .test_on_check_out(false)
            .connection_customizer(Box::new(options));
        if adaptive {
            builder = builder.min_idle(Some(0));
        }
        let pool = builder.build(manager).expect("reader pool");
        let mut initial = Vec::new();
        for _ in 0..readers {
            initial.push(pool.get().expect("validate reader"));
        }
        drop(initial);
        pool
    })
    .await
    .expect("blocking join");
    store.reads = Some(ReadPool {
        pool,
        semaphore: Arc::new(tokio::sync::Semaphore::new(readers as usize)),
    });
    store
}

async fn wait_for_connections(pool: &SqlitePool, wanted: u32) {
    tokio::time::timeout(Duration::from_secs(40), async {
        while pool.state().connections != wanted {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("r2d2 reaper reaches expected connection count");
}

#[tokio::test]
async fn production_readers_are_eagerly_validated_before_becoming_reclaimable() {
    let db = TempDb::new("reader_defaults");
    let initialized = Arc::new(AtomicUsize::new(0));
    let hook = initialized.clone();
    let store = crate::SqliteDatabase::open(
        &db.url(),
        SqliteDatabaseConfig::default()
            .with_read_pool_size(3)
            .with_connection_init(move |_| {
                hook.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }),
    )
    .await
    .unwrap()
    .store(1);
    let reads = store.reads.as_ref().unwrap();
    assert_eq!(reads.pool.min_idle(), Some(0));
    assert_eq!(reads.pool.idle_timeout(), Some(Duration::from_secs(600)));
    assert_eq!(reads.pool.state().connections, 3);
    assert_eq!(
        initialized.load(Ordering::SeqCst),
        4,
        "writer plus every reader initialized"
    );
    assert_eq!(reads.semaphore.available_permits(), 3);
    assert!(store.get_session("absent:0").await.unwrap().is_none());
}

#[tokio::test]
async fn retirement_protects_leases_and_reopens_beside_a_writer() {
    let db = TempDb::new("reader_retirement");
    let store = store_with_readers(&db.url(), true, Duration::from_millis(1), 2, None).await;
    store.put_session("fictitious:0", b"before").await.unwrap();
    let pool = store.reads.as_ref().unwrap().pool.clone();
    let mut leased = pool.get().unwrap();
    // Hold a real WAL snapshot across retirement of the other reader.
    leased.begin_test_transaction().unwrap();
    let read_snapshot_row = |conn: &mut SqliteConnection| {
        sessions::table
            .select(sessions::record)
            .filter(sessions::address.eq("fictitious:0"))
            .first::<Vec<u8>>(conn)
            .unwrap()
    };
    assert_eq!(read_snapshot_row(&mut leased), b"before");
    assert_eq!(pragma_i64(&mut leased, "query_only"), Some(1));
    #[derive(QueryableByName)]
    struct BusyTimeout {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        timeout: i32,
    }
    // SQLite names this pragma's result column `timeout`, not `busy_timeout`.
    let timeout = diesel::sql_query("PRAGMA busy_timeout")
        .get_result::<BusyTimeout>(&mut *leased)
        .unwrap();
    assert_eq!(timeout.timeout, 30_000);
    assert_eq!(pragma_i64(&mut leased, "foreign_keys"), Some(1));
    wait_for_connections(&pool, 1).await;
    store.put_session("fictitious:0", b"after").await.unwrap();
    assert_eq!(
        read_snapshot_row(&mut leased),
        b"before",
        "lease keeps its WAL snapshot"
    );
    assert_eq!(pool.state().connections, 1, "the leased reader survives");
    drop(leased);
    wait_for_connections(&pool, 0).await;

    let shared = store.shared();
    let sibling = store.share_for_device(2);
    // A cold read still has a separate queue, even when no reader is open.
    let writer = store.db_semaphore.clone().acquire_owned().await.unwrap();
    use diesel::connection::TransactionManager as _;
    let mut write_conn = store.pool.get().unwrap();
    <SqliteConnection as Connection>::TransactionManager::begin_transaction(&mut *write_conn)
        .unwrap();
    diesel::update(sessions::table.filter(sessions::address.eq("fictitious:0")))
        .set(sessions::record.eq(b"uncommitted".as_slice()))
        .execute(&mut *write_conn)
        .unwrap();
    // Concurrent cold readers must reopen beside an actual WAL write lock,
    // not just a held permit, and see the last committed value.
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let store = store.clone();
        tasks.push(tokio::spawn(async move {
            store.get_session("fictitious:0").await
        }));
    }
    for task in tasks {
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), task)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
                .as_deref(),
            Some(b"after".as_slice())
        );
    }
    assert!(pool.state().connections <= 2);
    let resumed = tokio::time::timeout(Duration::from_secs(5), store.get_session("fictitious:0"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.as_deref(), Some(b"after".as_slice()));
    assert!(sibling.reads.as_ref().unwrap().pool.state().connections > 0);
    assert!(
        shared
            .read(|conn| diesel::delete(sessions::table)
                .execute(conn)
                .map_err(db_error))
            .await
            .is_err()
    );
    <SqliteConnection as Connection>::TransactionManager::rollback_transaction(&mut *write_conn)
        .unwrap();
    drop(write_conn);
    drop(writer);
    drop(pool);
    drop(store);
    drop(sibling);
    // The SharedSqlite lease is sufficient to keep the pool usable after store drop.
    shared
        .read(|conn| {
            diesel::sql_query("SELECT 1")
                .execute(conn)
                .map_err(db_error)
        })
        .await
        .unwrap();
    drop(shared);
}

#[tokio::test]
async fn cold_reader_creation_failure_recovers_and_cancelled_reads_keep_their_lease() {
    let db = TempDb::new("reader_failure");
    let fail = Arc::new(AtomicBool::new(false));
    let acquisitions = Arc::new(AtomicUsize::new(0));
    let hook: ConnectionInitHook = {
        let fail = fail.clone();
        let acquisitions = acquisitions.clone();
        Arc::new(move |_| {
            acquisitions.fetch_add(1, Ordering::SeqCst);
            if fail.load(Ordering::SeqCst) {
                return Err(std::io::Error::other("injected reader init failure").into());
            }
            Ok(())
        })
    };
    let store = store_with_readers(&db.url(), true, Duration::from_millis(1), 1, Some(hook)).await;
    let pool = store.reads.as_ref().unwrap().pool.clone();
    wait_for_connections(&pool, 0).await;
    fail.store(true, Ordering::SeqCst);
    let failed_pool = pool.clone();
    let failure = crate::pool::spawn_blocking(move || {
        failed_pool.get_timeout(Duration::from_millis(100)).is_err()
    })
    .await
    .unwrap();
    assert!(failure);
    fail.store(false, Ordering::SeqCst);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), store.get_session("absent:0"))
            .await
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert!(acquisitions.load(Ordering::SeqCst) >= 3);

    let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let reader = tokio::spawn({
        let store = store.clone();
        async move {
            store
                .shared()
                .read(move |conn| {
                    diesel::sql_query("SELECT 1")
                        .execute(conn)
                        .map_err(db_error)?;
                    let _ = entered_tx.send(());
                    release_rx.recv_timeout(Duration::from_secs(40)).unwrap();
                    Ok(())
                })
                .await
        }
    });
    entered_rx.await.unwrap();
    reader.abort();
    assert!(reader.await.unwrap_err().is_cancelled());
    // Cancellation of the async waiter must not release a blocking job's permit.
    assert_eq!(
        store.reads.as_ref().unwrap().semaphore.available_permits(),
        0
    );
    tokio::time::sleep(Duration::from_secs(31)).await;
    assert_eq!(
        pool.state().connections,
        1,
        "cancelled-but-running read is leased"
    );
    release_tx.send(()).unwrap();
    wait_for_connections(&pool, 0).await;
    assert!(store.get_session("absent:0").await.unwrap().is_none());
}

/// Run each arm in a fresh, isolated test process, not under parallel tests.
/// SQLite's global counter includes its C allocations; RssAnon comes from procfs,
/// independently of page-cache bounds and file-backed resident pages.
#[cfg(all(target_os = "linux", feature = "bundled-sqlite"))]
#[tokio::test]
#[ignore = "native measurement: two isolated release runs, each waits for the real reaper"]
#[allow(clippy::print_stdout)]
async fn measure_reader_retention() {
    use wacore::time::Instant;

    fn checkpoint(label: &str, stores: &[SqliteStore]) {
        // SAFETY: SQLite's process-global allocation counter takes no pointers
        // and is threadsafe in the native bundled build.
        let sqlite_heap = unsafe { libsqlite3_sys::sqlite3_memory_used() };
        let status = std::fs::read_to_string("/proc/self/status").unwrap();
        let value = |key: &str| -> u64 {
            status
                .lines()
                .find_map(|line| line.strip_prefix(key))
                .and_then(|v| v.split_whitespace().next())
                .unwrap()
                .parse::<u64>()
                .unwrap()
                * 1024
        };
        let writers: u32 = stores.iter().map(|s| s.pool.state().connections).sum();
        let readers: u32 = stores
            .iter()
            .map(|s| s.reads.as_ref().unwrap().pool.state().connections)
            .sum();
        println!(
            "{label}: sqlite_heap={sqlite_heap} rss_anon={} rss_total={} writers={writers} readers={readers}",
            value("RssAnon:"),
            value("VmRSS:")
        );
    }
    fn percentiles(mut ns: Vec<u128>) -> (u128, u128) {
        ns.sort_unstable();
        (
            ns[ns.len() / 2],
            ns[(ns.len() * 99 / 100).min(ns.len() - 1)],
        )
    }
    let adaptive = std::env::var("SQLITE_READER_MODE").as_deref() == Ok("adaptive");
    let count = std::env::var("SQLITE_READER_COUNT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(50usize);
    let idle_secs = std::env::var("SQLITE_READER_IDLE_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1u64);
    assert!(count > 0 && idle_secs > 0, "positive count and timeout");
    println!(
        "mode={} stores={count} idle_timeout_secs={idle_secs}",
        if adaptive { "adaptive" } else { "baseline" }
    );
    let warm = std::env::var("SQLITE_READER_WARM").as_deref() == Ok("scan");
    let production = idle_secs == 600;
    println!("warm_scan={warm} production_builder={production}");
    let db = TempDb::new("retention_measurement");
    let seed = crate::SqliteDatabase::open(
        &db.url(),
        SqliteDatabaseConfig::default().with_read_pool_size(0),
    )
    .await
    .unwrap()
    .store(1);
    let rows: Vec<_> = (0..4000)
        .map(|i| {
            (
                format!("fictitious.{i}:0").into(),
                Bytes::from(vec![0x5a; 1024]),
            )
        })
        .collect();
    seed.put_sessions_batch(&rows).await.unwrap();
    drop(rows);
    drop(seed);
    checkpoint("baseline", &[]);
    let mut stores = Vec::new();
    for _ in 0..count {
        let store = if production {
            // Both real-interval arms use the actual constructor. The baseline
            // binary is built with the pre-change reader-builder body restored;
            // these assertions catch accidentally measuring the wrong policy.
            crate::SqliteDatabase::open(&db.url(), SqliteDatabaseConfig::default())
                .await
                .unwrap()
                .store(1)
        } else {
            store_with_readers(&db.url(), adaptive, Duration::from_secs(idle_secs), 1, None).await
        };
        let reads = store.reads.as_ref().unwrap();
        assert_eq!(reads.pool.min_idle(), if adaptive { Some(0) } else { None });
        assert_eq!(
            reads.pool.idle_timeout(),
            Some(Duration::from_secs(idle_secs))
        );
        assert_eq!(reads.pool.max_size(), 1);
        assert_eq!(reads.pool.state().connections, 1, "eager validation");
        assert_eq!(reads.semaphore.available_permits(), 1);
        store.get_session("fictitious.0:0").await.unwrap();
        stores.push(store);
    }
    checkpoint("point_reads", &stores);
    if warm {
        #[derive(QueryableByName)]
        struct Count {
            #[diesel(sql_type = diesel::sql_types::BigInt)]
            n: i64,
        }
        for store in &stores {
            let count = store
                .shared()
                .read(|conn| {
                    diesel::sql_query("SELECT count(record) AS n FROM sessions")
                        .get_result::<Count>(conn)
                        .map(|row| row.n)
                        .map_err(db_error)
                })
                .await
                .unwrap();
            assert_eq!(count, 4000);
        }
        checkpoint("warm_scan", &stores);
    }
    tokio::time::sleep(Duration::from_secs(idle_secs + 31)).await;
    checkpoint("idle", &stores);
    for store in &stores {
        assert_eq!(store.pool.state().connections, 1, "writer retained");
        assert_eq!(
            store.reads.as_ref().unwrap().pool.state().connections,
            if adaptive { 0 } else { 1 },
            "observed retirement, not inferred heap"
        );
    }

    let mut resume = Vec::new();
    for store in &stores {
        let start = Instant::now();
        assert!(store.get_session("fictitious.0:0").await.unwrap().is_some());
        resume.push(start.elapsed().as_nanos());
    }
    println!("cold_resume_ns_p50_p99={:?}", percentiles(resume));
    checkpoint("resumed", &stores);
    assert!(
        stores
            .iter()
            .all(|store| store.reads.as_ref().unwrap().pool.state().connections == 1)
    );
    let mut warm_latency = Vec::new();
    for store in &stores {
        for _ in 0..100 {
            let start = Instant::now();
            assert!(store.get_session("fictitious.0:0").await.unwrap().is_some());
            warm_latency.push(start.elapsed().as_nanos());
        }
    }
    println!("warm_read_ns_p50_p99={:?}", percentiles(warm_latency));

    let store = &stores[0];
    let burst: Vec<_> = (0..256)
        .map(|i| {
            (
                format!("updated.{i}:0").into(),
                Bytes::from(vec![0xa5; 1024]),
            )
        })
        .collect();
    // Upsert existing rows: each iteration does the same write work.
    store.put_sessions_batch(&burst).await.unwrap();
    let start = Instant::now();
    let mut latency = Vec::new();
    let mut writes = 0;
    let read_loop = async {
        for _ in 0..2000 {
            let start = Instant::now();
            store.get_session("fictitious.0:0").await.unwrap();
            latency.push(start.elapsed().as_nanos());
        }
    };
    let write_loop = async {
        for _ in 0..200 {
            store.put_sessions_batch(&burst).await.unwrap();
            writes += 1;
        }
    };
    tokio::join!(read_loop, write_loop);
    println!(
        "active: reads=2000 writes={writes} elapsed_ns={} read_ns_p50_p99={:?}",
        start.elapsed().as_nanos(),
        percentiles(latency)
    );
    drop(stores);
    tokio::time::sleep(Duration::from_millis(100)).await;
    checkpoint("shutdown", &[]);
}
