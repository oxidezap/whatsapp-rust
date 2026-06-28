//! Session establishment constants.

/// Maximum number of JIDs to include in a single prekey fetch request.
/// Matches WhatsApp Web's SESSION_CHECK_BATCH constant.
pub const SESSION_CHECK_BATCH_SIZE: usize = 50;
