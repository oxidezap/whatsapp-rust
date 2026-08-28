//! Cross-crate access to a store's SQLite database file.
//!
//! Sibling crates that keep their own tables in the same file (e.g. a chat/message
//! store) must not open a second connection pool: two pools mean two WAL writers
//! fighting over the file lock, two page caches, and two busy queues. [`SharedSqlite`]
//! hands them the store's own pool and write-serialization semaphore instead.

use diesel::sqlite::SqliteConnection;
use std::sync::Arc;
use wacore::store::error::{Result, StoreError};

use crate::sqlite_store::{SqlitePool, SqliteStore};

/// Clonable handle onto a [`SqliteStore`]'s connection pool and serialization
/// semaphore. Obtained via [`SqliteStore::shared`]. Holding one does not keep any
/// device row alive — it is purely connection plumbing.
#[derive(Clone)]
pub struct SharedSqlite {
    pool: SqlitePool,
    semaphore: Arc<tokio::sync::Semaphore>,
    reads: Option<crate::sqlite_store::ReadPool>,
}

impl SharedSqlite {
    /// Run `f` on a pooled connection from a blocking thread, holding one of the
    /// store's serialization permits for the duration. The closure owns error
    /// mapping into [`StoreError`] so callers can also run non-query work
    /// (e.g. their own embedded migrations) through the same choke point.
    ///
    /// This is the write path: everything that can modify the database belongs
    /// here, because the permit is what keeps two writers off SQLite at once.
    /// For work that only reads, [`read`](Self::read) skips that queue.
    pub async fn run<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        Self::run_on(self.pool.clone(), Arc::clone(&self.semaphore), f).await
    }

    /// Run a **read-only** `f` without queueing behind the write permit.
    ///
    /// WAL lets readers run alongside the single writer, which is the whole
    /// reason a slow query should not be able to stall the rest of a session.
    /// Reads still take a permit — one per reader connection — so the number of
    /// blocking threads stays bounded by the pool.
    ///
    /// `f` runs inside a deferred transaction, so every statement in it sees
    /// one snapshot. The write permit used to supply that for free — while a
    /// read held it the writer could not commit between its statements — and a
    /// reader that resolves a chat's identity keys and then queries by them, or
    /// collects search hits and then hydrates them, would otherwise straddle
    /// two committed states and come back short. A deferred transaction over
    /// read-only statements never asks for the write lock, so it pins the
    /// snapshot without contending with the writer.
    ///
    /// Only correct for statements that cannot write. A write sent through here
    /// escapes the serialization the store relies on and can deadlock against
    /// the real writer on the transaction upgrade, which `busy_timeout` cannot
    /// resolve. When a store has no reader connections configured
    /// ([`SqliteStoreConfig::read_pool_size`](crate::SqliteStoreConfig::read_pool_size)
    /// left at 0) this queues on the write permit exactly like
    /// [`run`](Self::run), so it is always safe to call — it simply buys no
    /// concurrency until the embedder opts in.
    pub async fn read<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let (pool, semaphore) = match &self.reads {
            Some(reads) => (reads.pool.clone(), Arc::clone(&reads.semaphore)),
            None => (self.pool.clone(), Arc::clone(&self.semaphore)),
        };
        Self::run_on(pool, semaphore, move |conn| read_snapshot(conn, f)).await
    }

    async fn run_on<F, T>(
        pool: SqlitePool,
        semaphore: Arc<tokio::sync::Semaphore>,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = semaphore
            .acquire_owned()
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))?;
        crate::pool::spawn_blocking(move || {
            let _permit = permit;
            let mut conn = pool
                .get()
                .map_err(|e| StoreError::Connection(Box::new(e)))?;
            f(&mut conn)
        })
        .await
        .map_err(|e| StoreError::Database(Box::new(e)))?
    }
}

/// Run `f` under one read snapshot.
///
/// Diesel's `transaction` needs an error type it can build from its own, and
/// [`StoreError`] deliberately has no such conversion (callers choose how a
/// database error is classified), so both travel in one enum and unwrap on the
/// way out.
fn read_snapshot<T>(
    conn: &mut SqliteConnection,
    f: impl FnOnce(&mut SqliteConnection) -> Result<T>,
) -> Result<T> {
    use diesel::connection::Connection as _;

    enum TxnError {
        Store(StoreError),
        Diesel(diesel::result::Error),
    }
    impl From<diesel::result::Error> for TxnError {
        fn from(e: diesel::result::Error) -> Self {
            Self::Diesel(e)
        }
    }
    conn.transaction::<T, TxnError, _>(|conn| f(conn).map_err(TxnError::Store))
        .map_err(|e| match e {
            TxnError::Store(e) => e,
            TxnError::Diesel(e) => StoreError::Database(Box::new(e)),
        })
}

impl SqliteStore {
    /// Handle for sibling crates to run their own queries and migrations against
    /// this store's database file through the same pool and semaphore.
    pub fn shared(&self) -> SharedSqlite {
        SharedSqlite {
            pool: self.pool.clone(),
            semaphore: self.db_semaphore.clone(),
            reads: self.reads.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::prelude::*;

    use crate::sqlite_store::SqliteStore;
    use wacore::store::error::StoreError;

    fn unique_db_name(tag: &str) -> String {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        format!(
            "file:memdb_shared_{tag}_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        )
    }

    async fn create_test_store(tag: &str) -> SqliteStore {
        SqliteStore::new(&unique_db_name(tag))
            .await
            .expect("Failed to create test store")
    }

    fn db_err(e: diesel::result::Error) -> StoreError {
        StoreError::Database(Box::new(e))
    }

    #[tokio::test]
    async fn shared_handle_sees_the_same_database() {
        let store = create_test_store("same_db").await;
        let shared = store.shared();

        shared
            .run(|conn| {
                diesel::sql_query("CREATE TABLE sibling_data (k TEXT PRIMARY KEY, v TEXT)")
                    .execute(conn)
                    .map_err(db_err)?;
                diesel::sql_query("INSERT INTO sibling_data (k, v) VALUES ('a', 'b')")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
            .expect("create + insert through shared handle");

        // A second (cloned) handle reads what the first wrote: one pool, one file.
        #[derive(QueryableByName)]
        struct Row {
            #[diesel(sql_type = diesel::sql_types::Text)]
            v: String,
        }
        let rows: Vec<Row> = shared
            .clone()
            .run(|conn| {
                diesel::sql_query("SELECT v FROM sibling_data WHERE k = 'a'")
                    .load(conn)
                    .map_err(db_err)
            })
            .await
            .expect("read through cloned handle");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].v, "b");
    }

    /// A file-backed store, since reader connections need real WAL and an
    /// in-memory database has none to switch to. Removed on drop.
    struct TempDb(std::path::PathBuf);

    impl TempDb {
        fn new(tag: &str) -> Self {
            use portable_atomic::AtomicU64;
            use std::sync::atomic::Ordering;
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let id = COUNTER.fetch_add(1, Ordering::Relaxed);
            let mut path = std::env::temp_dir();
            path.push(format!("wa_shared_{tag}_{}_{id}.db", std::process::id()));
            let _ = std::fs::remove_file(&path);
            Self(path)
        }

        fn url(&self) -> String {
            self.0.to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDb {
        fn drop(&mut self) {
            for suffix in ["", "-wal", "-shm"] {
                let mut p = self.0.clone().into_os_string();
                p.push(suffix);
                let _ = std::fs::remove_file(p);
            }
        }
    }

    /// A reader parks until released, but never forever: a blocking task cannot
    /// be aborted, and a test that panics while one is parked would hang the
    /// runtime on shutdown instead of reporting the failure.
    fn park_until_released(rx: &std::sync::mpsc::Receiver<()>) {
        let _ = rx.recv_timeout(std::time::Duration::from_secs(20));
    }

    /// With reader connections configured, a slow read must not hold up another
    /// read. Before the split there was one permit for everything, so a long
    /// query stalled every other read on the session for its whole duration.
    #[tokio::test]
    async fn reads_run_concurrently_when_reader_connections_are_configured() {
        use crate::sqlite_store::SqliteStoreConfig;
        use std::sync::Arc;

        let db = TempDb::new("read_concurrency");
        let store = SqliteStore::with_config(
            &db.url(),
            SqliteStoreConfig {
                read_pool_size: 4,
                ..Default::default()
            },
        )
        .await
        .expect("store with reader connections");
        assert!(store.reads.is_some(), "a file-backed store reaches WAL");
        let shared = store.shared();

        // Every reader announces itself and then parks. With a single shared
        // permit only the first would ever announce, so the collect below is
        // what actually proves they overlap.
        let (ready_tx, mut ready_rx) = tokio::sync::mpsc::unbounded_channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
        let mut readers = Vec::new();
        for _ in 0..4 {
            let shared = shared.clone();
            let ready_tx = ready_tx.clone();
            let release_rx = Arc::clone(&release_rx);
            readers.push(tokio::spawn(async move {
                shared
                    .read(move |conn| {
                        diesel::sql_query("SELECT 1")
                            .execute(conn)
                            .map_err(db_err)?;
                        let _ = ready_tx.send(());
                        // Each reader holds its own receiver turn; the guard is
                        // only contended while parked, which is the point.
                        park_until_released(&release_rx.lock().expect("release rx"));
                        Ok(())
                    })
                    .await
            }));
        }
        drop(ready_tx);

        // All four must be inside `read` at once.
        for _ in 0..4 {
            tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx.recv())
                .await
                .expect("readers must overlap, not serialize")
                .expect("reader alive");
        }
        for _ in 0..4 {
            let _ = release_tx.send(());
        }
        for reader in readers {
            reader.await.expect("join").expect("read");
        }
    }

    /// A write keeps its own pool, so readers saturating theirs can never leave
    /// the writer waiting for a connection.
    #[tokio::test]
    async fn a_write_proceeds_while_every_reader_permit_is_held() {
        use crate::sqlite_store::SqliteStoreConfig;
        use std::sync::Arc;

        let db = TempDb::new("write_not_starved");
        let store = SqliteStore::with_config(
            &db.url(),
            SqliteStoreConfig {
                read_pool_size: 2,
                ..Default::default()
            },
        )
        .await
        .expect("store with reader connections");
        let shared = store.shared();
        shared
            .run(|conn| {
                diesel::sql_query("CREATE TABLE probe (k INTEGER PRIMARY KEY)")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
            .expect("create");

        let (ready_tx, mut ready_rx) = tokio::sync::mpsc::unbounded_channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let release_rx = Arc::new(std::sync::Mutex::new(release_rx));
        let mut readers = Vec::new();
        for _ in 0..2 {
            let shared = shared.clone();
            let ready_tx = ready_tx.clone();
            let release_rx = Arc::clone(&release_rx);
            readers.push(tokio::spawn(async move {
                shared
                    .read(move |conn| {
                        diesel::sql_query("SELECT 1")
                            .execute(conn)
                            .map_err(db_err)?;
                        let _ = ready_tx.send(());
                        park_until_released(&release_rx.lock().expect("release rx"));
                        Ok(())
                    })
                    .await
            }));
        }
        drop(ready_tx);

        // Only meaningful once both readers actually hold their permits and
        // connections — otherwise the writer could simply win the race.
        for _ in 0..2 {
            tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx.recv())
                .await
                .expect("readers must check out")
                .expect("reader alive");
        }

        let wrote = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            shared.run(|conn| {
                diesel::sql_query("INSERT INTO probe (k) VALUES (1)")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            }),
        )
        .await;
        // Release before asserting: a parked blocking task cannot be aborted,
        // so panicking first would hang the runtime instead of failing.
        for _ in 0..2 {
            let _ = release_tx.send(());
        }
        wrote
            .expect("the writer must not queue behind readers")
            .expect("insert");
        for reader in readers {
            reader.await.expect("join").expect("read");
        }
    }

    /// Reader connections are `query_only`, so a write sent down the read path
    /// fails loudly instead of silently escaping the write serialization.
    #[tokio::test]
    async fn a_write_through_the_read_path_is_refused() {
        use crate::sqlite_store::SqliteStoreConfig;

        let db = TempDb::new("read_only");
        let store = SqliteStore::with_config(
            &db.url(),
            SqliteStoreConfig {
                read_pool_size: 1,
                ..Default::default()
            },
        )
        .await
        .expect("store with reader connections");
        let shared = store.shared();

        let result = shared
            .read(|conn| {
                diesel::sql_query("CREATE TABLE nope (k INTEGER)")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await;
        assert!(matches!(result, Err(StoreError::Database(_))));
    }

    /// An in-memory database never reaches WAL, so reader connections would buy
    /// contention instead of concurrency. The store declines them and keeps the
    /// single queue rather than pretending.
    #[tokio::test]
    async fn reader_connections_are_declined_without_wal() {
        use crate::sqlite_store::SqliteStoreConfig;

        let store = SqliteStore::with_config(
            &unique_db_name("no_wal"),
            SqliteStoreConfig {
                read_pool_size: 4,
                ..Default::default()
            },
        )
        .await
        .expect("in-memory store still opens");
        assert!(store.reads.is_none(), "no WAL, no reader pool");

        // And reads still work, on the write queue.
        store
            .shared()
            .read(|conn| {
                diesel::sql_query("SELECT 1")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
            .expect("reads fall back to the write permit");
    }

    /// Shared cache reaches WAL, so the journal check alone would let reader
    /// connections through — into table locks that block the writer outright.
    #[tokio::test]
    async fn reader_connections_are_declined_for_shared_cache() {
        use crate::sqlite_store::SqliteStoreConfig;

        let db = TempDb::new("shared_cache");
        let store = SqliteStore::with_config(
            &format!("file:{}?cache=shared", db.url()),
            SqliteStoreConfig {
                read_pool_size: 4,
                ..Default::default()
            },
        )
        .await
        .expect("shared-cache store still opens");
        assert!(
            store.reads.is_none(),
            "shared cache serializes readers against the writer anyway"
        );

        // The same file without the parameter does get reader connections, so
        // the decline is about the cache mode and not about the path.
        let store = SqliteStore::with_config(
            &db.url(),
            SqliteStoreConfig {
                read_pool_size: 4,
                ..Default::default()
            },
        )
        .await
        .expect("private-cache store opens");
        assert!(
            store.reads.is_some(),
            "private cache under WAL gets readers"
        );
    }

    /// Reader connections hold page caches of their own, so the memory bound
    /// has to count them or a read-enabled store reports the write pool's usage
    /// as if it were the whole store's.
    #[tokio::test]
    async fn resource_report_counts_reader_connections() {
        use crate::sqlite_store::SqliteStoreConfig;
        use wacore::store::traits::DeviceStore;

        let db = TempDb::new("report_readers");
        let config = || SqliteStoreConfig {
            read_pool_size: 3,
            ..Default::default()
        };

        let with_readers = SqliteStore::with_config(&db.url(), config())
            .await
            .expect("store opens");
        assert!(with_readers.reads.is_some(), "readers are configured");
        // r2d2 opens min_idle connections eagerly; touch the read path so the
        // reader pool has definitely opened one to account for.
        with_readers
            .shared()
            .read(|conn| {
                diesel::sql_query("SELECT 1")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
            .expect("read succeeds");

        let baseline = TempDb::new("report_no_readers");
        let without = SqliteStore::new(&baseline.url())
            .await
            .expect("store opens");

        let (a, b) = (
            with_readers.resource_report().await,
            without.resource_report().await,
        );
        let (Some(with_mem), Some(without_mem)) = (a.memory_bytes, b.memory_bytes) else {
            panic!("both stores report a cache estimate");
        };
        assert!(
            with_mem > without_mem,
            "reader caches must widen the bound: {with_mem} vs {without_mem}"
        );
    }

    /// Left at its default, `read` is the write path — same queue, same
    /// behaviour as before the knob existed.
    #[tokio::test]
    async fn read_falls_back_to_the_write_permit_by_default() {
        let store = create_test_store("read_default").await;
        let shared = store.shared();
        shared
            .read(|conn| {
                diesel::sql_query("SELECT 1")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            })
            .await
            .expect("reads work with no reader connections configured");
    }

    #[tokio::test]
    async fn shared_handle_propagates_closure_errors() {
        let store = create_test_store("err").await;
        let result = store
            .shared()
            .run(|conn| {
                diesel::sql_query("SELECT * FROM does_not_exist")
                    .execute(conn)
                    .map_err(db_err)
            })
            .await;
        assert!(matches!(result, Err(StoreError::Database(_))));
    }
}
