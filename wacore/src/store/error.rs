use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    #[error("I/O error")]
    Io(#[from] std::io::Error),

    #[error("serialization/deserialization error")]
    Serialization(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// Validation failure with a descriptive message and no underlying typed
    /// source — e.g. "Invalid foo length: 17". Prefer `Serialization` or a
    /// dedicated typed variant if a real source exists.
    #[error("data validation failed: {0}")]
    Validation(String),

    /// Invalid persisted message-secret data. The source contains only the
    /// observed length, never the key material.
    #[error("invalid stored message secret: {0}")]
    InvalidMessageSecret(#[source] crate::types::message_secret::InvalidMessageSecret),

    #[error("database connection error")]
    Connection(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("database operation error")]
    Database(#[source] Box<dyn std::error::Error + Send + Sync>),

    /// The backend exhausted its busy/locked contention retry budget.
    /// This remains classified as contention by [`Self::is_database_busy_or_locked`].
    #[error("database operation '{op}' exhausted retries after {attempts} attempts")]
    RetriesExhausted {
        op: String,
        /// Total attempts, including the initial operation.
        attempts: u32,
        /// The last contention failure, preserved for typed inspection.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("migration error")]
    Migration(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("store configuration is invalid: {0}")]
    InvalidConfig(String),

    #[error("device with ID {0} not found")]
    DeviceNotFound(i32),
}

impl StoreError {
    /// Recognizes exhausted contention retries and rejects known non-database
    /// error variants without inspecting their messages.
    ///
    /// `Database` and `Connection` carry opaque backend causes. Unless their
    /// source chain contains another typed `StoreError`, this compatibility
    /// fallback looks for "busy" or "locked" in source messages, ignoring case.
    /// It can misclassify unrelated backend messages and cannot establish retry
    /// safety. Backends should use the specific error variants when available.
    /// SQLite's Diesel adapter loses BUSY/LOCKED codes, so opaque source-text
    /// detection remains necessary there.
    pub fn is_database_busy_or_locked(&self) -> bool {
        let mut layer: &(dyn std::error::Error + 'static) = match self {
            Self::RetriesExhausted { .. } => return true,
            Self::Database(source) | Self::Connection(source) => source.as_ref(),
            _ => return false,
        };
        let mut text_indicates_contention = false;
        loop {
            if let Some(typed) = layer.downcast_ref::<Self>() {
                return typed.is_database_busy_or_locked();
            }
            let message = layer.to_string().to_lowercase();
            text_indicates_contention |= message.contains("locked") || message.contains("busy");
            match layer.source() {
                Some(inner) => layer = inner,
                None => return text_indicates_contention,
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, StoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("synthetic backend error: {0}")]
    struct DummyBackendError(&'static str);

    #[test]
    fn database_preserves_typed_source_via_downcast() {
        let inner = DummyBackendError("bang");
        let se = StoreError::Database(Box::new(inner));
        let src = std::error::Error::source(&se).expect("source preserved");
        let downcast = src
            .downcast_ref::<DummyBackendError>()
            .expect("downcasts to DummyBackendError");
        assert_eq!(downcast.0, "bang");
    }

    #[test]
    fn is_busy_or_locked_walks_chain() {
        let inner = DummyBackendError("database is locked");
        let se = StoreError::Database(Box::new(inner));
        assert!(se.is_database_busy_or_locked());
    }

    #[test]
    fn is_busy_or_locked_negative() {
        let inner = DummyBackendError("permission denied");
        let se = StoreError::Database(Box::new(inner));
        assert!(!se.is_database_busy_or_locked());
    }

    #[test]
    fn is_busy_or_locked_is_case_insensitive() {
        // SQLite drivers in different ecosystems vary on casing for these
        // diagnostic strings ("database is LOCKED", "Busy", etc.). The check
        // must not depend on the exact casing the underlying driver chose.
        for msg in [
            "database is LOCKED",
            "SQLITE_BUSY: write contention",
            "Busy",
            "Locked",
        ] {
            let se = StoreError::Database(Box::new(DummyBackendError(msg)));
            assert!(
                se.is_database_busy_or_locked(),
                "expected {msg:?} to be detected"
            );
        }
    }
}
