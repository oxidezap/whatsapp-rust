//! SQLite storage backend for whatsapp-rust
//!
//! This crate provides a SQLite-based storage implementation for the whatsapp-rust library.
//! It implements all the required storage traits from wacore::store::traits.
//!
//! Open a single account with [`SqliteStore::open`], or tune/share a database
//! with [`SqliteDatabase::open`] and select scopes with [`SqliteDatabase::store`].
//! Administration belongs to the database; [`SharedSqlite`] remains available
//! for advanced adapters with their own tables and account predicates.
//!
//! # SQLite version
//!
//! SQLite before 3.54.0 can return stale page-cache bytes after it opens a
//! WAL database whose `-wal` file has a different page size
//! (RUSTSEC-2026-0459). The default `bundled-sqlite` feature compiles the
//! SQLite that `libsqlite3-sys` 0.38 bundles, which is affected; Diesel 2.3
//! does not yet accept `libsqlite3-sys` 0.39, which bundles 3.54.1. To link a
//! fixed SQLite you built yourself, set `LIBSQLITE3_SYS_USE_PKG_CONFIG=1`
//! (this overrides `bundled`) and point `libsqlite3-sys` at the library with
//! `SQLITE3_LIB_DIR` and `SQLITE3_STATIC=1`, or with pkg-config. In this
//! repository, `cargo xt fixed-sqlite` builds 3.54.1 from the official release
//! and prints those variables. The wasm backend links `sqlite-wasm-rs` 0.6.3,
//! which bundles 3.54.1.
//!
//! The pre-1.0 constructor/configuration aliases are deliberately removed:
//! ```compile_fail
//! use whatsapp_rust_sqlite_storage::SqliteStore;
//! let _ = SqliteStore::new("whatsapp.db");
//! ```
//! ```compile_fail
//! use whatsapp_rust_sqlite_storage::SqliteStoreConfig;
//! ```
//! Scope changes and administration are not operations on a backend handle:
//! ```compile_fail
//! use whatsapp_rust_sqlite_storage::SqliteStore;
//! fn old_scope(store: &SqliteStore) {
//!     let _ = store.share_for_device(2);
//! }
//! ```
//! ```compile_fail
//! use whatsapp_rust_sqlite_storage::SqliteStore;
//! fn old_admin(store: &SqliteStore) {
//!     let _ = store.list_devices();
//! }
//! ```

mod bounded_mobile_binding;
mod bounded_pending_query;
mod database;
mod pool;
mod schema;
mod shared;
mod sqlite_store;
pub(crate) mod upsert_queries;
mod wire;

pub use database::SqliteDatabase;
pub use shared::SharedSqlite;
pub use sqlite_store::{
    CommitBarrierError, CommitBarrierFuture, CommitBarrierHook, ConnectionInitHook,
    SqliteDatabaseConfig, SqliteStore, StoredDeviceSummary, Synchronous,
};

#[cfg(feature = "test-util")]
#[doc(hidden)]
pub async fn test_retry_backoff(delay_ms: u64) {
    sqlite_store::retry_backoff(delay_ms).await;
}
