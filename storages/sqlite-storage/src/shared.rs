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
    read_semaphore: Option<Arc<tokio::sync::Semaphore>>,
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
        self.run_with(Arc::clone(&self.semaphore), f).await
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
        let semaphore = self
            .read_semaphore
            .clone()
            .unwrap_or_else(|| Arc::clone(&self.semaphore));
        self.run_with(semaphore, move |conn| read_snapshot(conn, f))
            .await
    }

    async fn run_with<F, T>(&self, semaphore: Arc<tokio::sync::Semaphore>, f: F) -> Result<T>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = semaphore
            .acquire_owned()
            .await
            .map_err(|e| StoreError::Database(Box::new(e)))?;
        let pool = self.pool.clone();
        tokio::task::spawn_blocking(move || {
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
            read_semaphore: self.read_semaphore.clone(),
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

    /// With reader connections configured, a slow read must not hold up another
    /// read. Before the split there was one permit for everything, so a long
    /// query stalled every other read on the session for its whole duration.
    #[tokio::test]
    async fn reads_run_concurrently_when_reader_connections_are_configured() {
        use crate::sqlite_store::SqliteStoreConfig;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let store = SqliteStore::with_config(
            &unique_db_name("read_concurrency"),
            SqliteStoreConfig {
                read_pool_size: 4,
                ..Default::default()
            },
        )
        .await
        .expect("store with reader connections");
        let shared = store.shared();

        // Each reader parks until all four have arrived. With a single shared
        // permit this deadlocks until the test times out; with reader permits
        // they overlap and the barrier releases.
        let barrier = Arc::new(std::sync::Barrier::new(4));
        let observed = Arc::new(AtomicUsize::new(0));
        let mut readers = Vec::new();
        for _ in 0..4 {
            let shared = shared.clone();
            let barrier = Arc::clone(&barrier);
            let observed = Arc::clone(&observed);
            readers.push(tokio::spawn(async move {
                shared
                    .read(move |conn| {
                        diesel::sql_query("SELECT 1")
                            .execute(conn)
                            .map_err(db_err)?;
                        observed.fetch_add(1, Ordering::SeqCst);
                        // Blocking wait: these are on spawn_blocking threads.
                        barrier.wait();
                        Ok(())
                    })
                    .await
            }));
        }
        for reader in readers {
            tokio::time::timeout(std::time::Duration::from_secs(10), reader)
                .await
                .expect("readers must overlap, not serialize")
                .expect("join")
                .expect("read");
        }
        assert_eq!(observed.load(Ordering::SeqCst), 4);
    }

    /// A write keeps its own connection, so readers saturating their permits
    /// can never leave the writer waiting on the pool.
    #[tokio::test]
    async fn a_write_proceeds_while_every_reader_permit_is_held() {
        use crate::sqlite_store::SqliteStoreConfig;
        use std::sync::Arc;

        let store = SqliteStore::with_config(
            &unique_db_name("write_not_starved"),
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

        // Both reader permits held for the duration.
        let release = Arc::new(std::sync::Barrier::new(3));
        let mut readers = Vec::new();
        for _ in 0..2 {
            let shared = shared.clone();
            let release = Arc::clone(&release);
            readers.push(tokio::spawn(async move {
                shared
                    .read(move |conn| {
                        diesel::sql_query("SELECT 1")
                            .execute(conn)
                            .map_err(db_err)?;
                        release.wait();
                        Ok(())
                    })
                    .await
            }));
        }

        let writer = shared.clone();
        let wrote = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            writer.run(|conn| {
                diesel::sql_query("INSERT INTO probe (k) VALUES (1)")
                    .execute(conn)
                    .map_err(db_err)?;
                Ok(())
            }),
        )
        .await
        .expect("the writer must not queue behind readers");
        wrote.expect("insert");

        release.wait();
        for reader in readers {
            reader.await.expect("join").expect("read");
        }
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
