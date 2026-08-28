//! Where a connection comes from, and where blocking work runs.
//!
//! Both are one answer on a server and a different one in a browser, and the
//! difference is the same fact twice: **a page has no threads to give**.
//! `wasm32-unknown-unknown` has no `thread::spawn`, so r2d2's management pool
//! fails to start (`scheduled-thread-pool` unwraps that spawn), and tokio's
//! blocking pool fails the first time anything is handed to it.
//!
//! Neither is a loss there, because neither is buying anything. A pool exists
//! to hand out connections to concurrent callers; a page has one agent, so
//! there is one connection and nothing to hand it to. `spawn_blocking` exists
//! to keep a slow query off the async worker; a page's query already runs on
//! the only agent there is, and moving it is not something the platform can
//! do. So the browser shapes are a connection behind a lock and a call that
//! simply runs.
//!
//! Everything above this module is written once. That is the whole point of
//! the module: the shim keeps r2d2's own spelling — `Pool::builder()`,
//! `.get()`, `.max_size()`, `.state()` — so the six thousand lines of store
//! that use them do not learn which platform they are on.

#[cfg(not(target_family = "wasm"))]
pub(crate) use native::*;
#[cfg(target_family = "wasm")]
pub(crate) use web::*;

#[cfg(not(target_family = "wasm"))]
mod native {
    use diesel::r2d2::ConnectionManager;
    use diesel::sqlite::SqliteConnection;
    use std::sync::Arc;

    pub(crate) type Pool = diesel::r2d2::Pool<ConnectionManager<SqliteConnection>>;
    pub(crate) type Builder = diesel::r2d2::Builder<ConnectionManager<SqliteConnection>>;

    /// r2d2's own builder, with the management pool resolved.
    ///
    /// `None` means "share the process-wide one", which is the default an
    /// embedder gets for not choosing: one `SqliteStore` per session times
    /// r2d2's own three management threads is hundreds of idle threads on a
    /// busy worker.
    pub(crate) fn builder(
        thread_pool: Option<Arc<scheduled_thread_pool::ScheduledThreadPool>>,
    ) -> Builder {
        Pool::builder().thread_pool(thread_pool.unwrap_or_else(shared_thread_pool))
    }

    /// One `ScheduledThreadPool` shared by every store's r2d2 pool.
    ///
    /// Those threads only do infrequent connection reaping and creation, so
    /// two of them for the whole process is plenty.
    fn shared_thread_pool() -> Arc<scheduled_thread_pool::ScheduledThreadPool> {
        static POOL: std::sync::OnceLock<Arc<scheduled_thread_pool::ScheduledThreadPool>> =
            std::sync::OnceLock::new();
        POOL.get_or_init(|| {
            Arc::new(
                scheduled_thread_pool::ScheduledThreadPool::builder()
                    .num_threads(2)
                    .thread_name_pattern("r2d2-shared-{}")
                    .build(),
            )
        })
        .clone()
    }

    pub(crate) use tokio::task::spawn_blocking;
}

#[cfg(target_family = "wasm")]
mod web {
    use diesel::r2d2::{ConnectionManager, CustomizeConnection, ManageConnection};
    use diesel::sqlite::SqliteConnection;
    use std::sync::Arc;
    use tokio::sync::{Mutex, OwnedMutexGuard};

    /// The failure a "pool" of one can actually have: opening the database, or
    /// finding the connection already in use.
    ///
    /// r2d2's own error is not constructible from outside r2d2, and there is
    /// nothing to gain from imitating it — every call site boxes this as a
    /// `StoreError::Connection` and prints it.
    #[derive(Debug)]
    pub(crate) struct PoolError(String);

    impl std::fmt::Display for PoolError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str(&self.0)
        }
    }
    impl std::error::Error for PoolError {}

    /// A single connection, opened once and lent out.
    ///
    /// Not a pool and not pretending to be one: `max_size` is 1 because the
    /// database is single-threaded here — `sqlite-wasm-rs` is compiled with
    /// `SQLITE_THREADSAFE=0` and its handles are JavaScript values, which do
    /// not cross agents at all. A second connection would be a second VFS
    /// handle on the same origin-private file, which is the one thing OPFS
    /// and IndexedDB backends both refuse.
    #[derive(Clone)]
    pub(crate) struct Pool {
        conn: Arc<Mutex<SqliteConnection>>,
    }

    /// A connection borrowed from [`Pool`], returned when it drops.
    ///
    /// Shaped as r2d2's `PooledConnection` is — a `DerefMut` to the
    /// connection — so the store's query code is the same code on both sides.
    pub(crate) struct PooledConnection {
        guard: OwnedMutexGuard<SqliteConnection>,
    }

    impl std::ops::Deref for PooledConnection {
        type Target = SqliteConnection;
        fn deref(&self) -> &Self::Target {
            &self.guard
        }
    }
    impl std::ops::DerefMut for PooledConnection {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.guard
        }
    }

    /// What r2d2 reports about a pool, for the one caller that asks.
    pub(crate) struct State {
        pub(crate) connections: u32,
    }

    impl Pool {
        pub(crate) fn get(&self) -> Result<PooledConnection, PoolError> {
            // `try_lock` rather than a wait: there is one agent, so a
            // connection already lent out was lent to *this* call stack, and
            // waiting for it would wait forever. Saying so is the only useful
            // answer, and it is the answer immediately.
            self.try_get().ok_or_else(|| {
                PoolError("the database connection is already in use on this call stack".into())
            })
        }

        /// The same checkout, for a caller that would rather have nothing than
        /// wait — the storage report, which is best-effort by construction.
        pub(crate) fn try_get(&self) -> Option<PooledConnection> {
            Arc::clone(&self.conn)
                .try_lock_owned()
                .ok()
                .map(|guard| PooledConnection { guard })
        }

        pub(crate) fn max_size(&self) -> u32 {
            1
        }

        pub(crate) fn state(&self) -> State {
            State { connections: 1 }
        }
    }

    /// The half of r2d2's builder this crate actually uses.
    ///
    /// `test_on_check_out` and the management thread pool have no meaning for
    /// a connection that is never checked out to another thread and never
    /// reaped, so they are accepted and dropped — keeping the call sites
    /// identical is worth more than making them ask which platform they are
    /// on.
    pub(crate) struct Builder {
        customizer: Option<Box<dyn CustomizeConnection<SqliteConnection, diesel::r2d2::Error>>>,
    }

    pub(crate) fn builder(
        _thread_pool: Option<Arc<scheduled_thread_pool::ScheduledThreadPool>>,
    ) -> Builder {
        Builder { customizer: None }
    }

    impl Builder {
        pub(crate) fn max_size(self, _size: u32) -> Self {
            self
        }
        pub(crate) fn test_on_check_out(self, _test: bool) -> Self {
            self
        }
        pub(crate) fn connection_customizer(
            mut self,
            customizer: Box<dyn CustomizeConnection<SqliteConnection, diesel::r2d2::Error>>,
        ) -> Self {
            self.customizer = Some(customizer);
            self
        }

        /// Open the connection and run the customizer over it.
        ///
        /// Eagerly, where r2d2 would open lazily: the pragmas the customizer
        /// applies (`journal_mode`, `busy_timeout`, the page cache) are the
        /// database's whole configuration, and deferring them would let the
        /// first query decide them instead.
        pub(crate) fn build(
            self,
            manager: ConnectionManager<SqliteConnection>,
        ) -> Result<Pool, PoolError> {
            let mut conn = manager
                .connect()
                .map_err(|e| PoolError(format!("opening the database: {e}")))?;
            if let Some(customizer) = &self.customizer {
                customizer
                    .on_acquire(&mut conn)
                    .map_err(|e| PoolError(format!("configuring the connection: {e}")))?;
            }
            Ok(Pool {
                conn: Arc::new(Mutex::new(conn)),
            })
        }
    }

    /// The error a task that cannot be cancelled cannot have.
    ///
    /// [`spawn_blocking`] here does not spawn, so nothing can panic in another
    /// thread and nothing can be aborted. The type exists so the call sites'
    /// `map_err` is the same line on both platforms.
    #[derive(Debug)]
    pub(crate) enum JoinError {}

    impl std::fmt::Display for JoinError {
        fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match *self {}
        }
    }
    impl std::error::Error for JoinError {}

    /// Run it here, because there is nowhere else to run it.
    ///
    /// The signature is `spawn_blocking`'s, bounds included, so the call sites
    /// do not change and the native build keeps every guarantee it had. What
    /// is lost is real and worth naming: a long query blocks the page's only
    /// agent, which is why the session belongs in a worker rather than in the
    /// window.
    pub(crate) async fn spawn_blocking<F, T>(f: F) -> Result<T, JoinError>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        Ok(f())
    }
}
