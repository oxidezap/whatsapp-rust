//! Newsletter (Channel) IQ specifications.
//!
//! Newsletters use two protocol layers:
//! - Mex (GraphQL) for metadata/management operations — see the
//!   `*_newsletter*` modules in `crate::iq::mex_operations` for document IDs
//!   and typed variables
//! - Standard IQ (xmlns="newsletter") for message operations

/// IQ namespace for newsletter operations (message history, reactions, live updates).
pub const NEWSLETTER_XMLNS: &str = "newsletter";
