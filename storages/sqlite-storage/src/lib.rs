//! SQLite storage backend for whatsapp-rust
//!
//! This crate provides a SQLite-based storage implementation for the whatsapp-rust library.
//! It implements all the required storage traits from wacore::store::traits.

mod pool;
mod schema;
mod shared;
mod sqlite_store;
pub(crate) mod upsert_queries;
mod wire;

pub use shared::SharedSqlite;
pub use sqlite_store::{
    CommitBarrierError, CommitBarrierFuture, CommitBarrierHook, ConnectionInitHook, SqliteStore,
    SqliteStoreConfig, Synchronous,
};

#[cfg(feature = "test-util")]
#[doc(hidden)]
pub async fn test_retry_backoff(delay_ms: u64) {
    sqlite_store::retry_backoff(delay_ms).await;
}
