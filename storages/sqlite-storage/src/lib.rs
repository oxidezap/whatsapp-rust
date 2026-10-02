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
