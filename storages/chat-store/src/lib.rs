//! SQLite-backed chat/message history for whatsapp-rust.
//!
//! This crate materializes the client's event stream (messages, receipts,
//! edits, revokes, reactions, history sync, app-state chat updates) into
//! queryable tables in the SAME database file as the device store, so a UI or
//! stateful bot survives a restart without re-syncing. It has no UI
//! dependencies — it is a data layer any consumer can opt into.
//!
//! Design:
//! - **Event-sourced**: register [`ChatStore::handler`] on the client; a
//!   single writer task applies batched events transactionally, in order.
//! - **Proto as source of truth**: each message row stores the encoded
//!   `wa::Message` plus denormalized columns for listing/search — new proto
//!   fields never require a migration.
//! - **Query + invalidation**: read via the async query API (keyset
//!   pagination), subscribe to [`types::StoreChange`] to know when to re-query.
//!   No second in-memory cache of rows.
//! - **Shared file, shared pool**: writes go through
//!   [`SqliteStore::shared`](whatsapp_rust_sqlite_storage::SqliteStore),
//!   so there is exactly one WAL writer per database file.
//!
//! ```ignore
//! let chat_store = ChatStore::new(&sqlite_store).await?;
//! client.register_handler(chat_store.handler());
//!
//! let chats = chat_store.chats(false, 50).await?;
//! let page = chat_store.messages(&chats[0].jid, None, 40).await?;
//! let mut changes = chat_store.subscribe();
//! ```

mod error;
#[cfg(feature = "search")]
mod fts;
mod materialize;
mod queries;
mod schema;
mod store;
pub mod types;

pub use error::{ChatStoreError, Result};
pub use store::ChatStore;
pub use types::{
    ChatEntry, ContactEntry, MediaRef, MessageCursor, MessageKind, MessageStatus, ReactionEntry,
    ReceiptEntry, StoreChange, StoredMessage,
};
