//! Stanza types for WhatsApp protocol notifications.
//!
//! This module contains type-safe parsers for incoming notification stanzas.

pub mod business;
pub mod call;
pub mod connect_failure;
pub mod devices;
pub mod group_call;
pub mod groups;
pub mod message;
pub mod notification;
pub mod receipt;
pub mod wire_tags;

pub use business::*;
pub use devices::*;
pub use groups::*;
pub use message::*;
