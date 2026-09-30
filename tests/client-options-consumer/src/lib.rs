//! Shared options do not weaken Bot's compile-time dependency requirements.
//!
//! ```compile_fail
//! use whatsapp_rust::bot::Bot;
//! use whatsapp_rust::ClientOptions;
//! let _ = Bot::builder().with_client_options(ClientOptions::default()).build();
//! ```

#[cfg(test)]
#[path = "../../client_options.rs"]
mod contract;
