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
}

impl SharedSqlite {
    /// Run `f` on a pooled connection from a blocking thread, holding one of the
    /// store's serialization permits for the duration. The closure owns error
    /// mapping into [`StoreError`] so callers can also run non-query work
    /// (e.g. their own embedded migrations) through the same choke point.
    pub async fn run<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut SqliteConnection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let permit = self
            .semaphore
            .clone()
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

impl SqliteStore {
    /// Handle for sibling crates to run their own queries and migrations against
    /// this store's database file through the same pool and semaphore.
    pub fn shared(&self) -> SharedSqlite {
        SharedSqlite {
            pool: self.pool.clone(),
            semaphore: self.db_semaphore.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::prelude::*;

    use crate::sqlite_store::SqliteStore;
    use wacore::store::error::StoreError;

    async fn create_test_store(tag: &str) -> SqliteStore {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_name = format!(
            "file:memdb_shared_{tag}_{}_{}?mode=memory&cache=shared",
            std::process::id(),
            id
        );
        SqliteStore::new(&db_name)
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
