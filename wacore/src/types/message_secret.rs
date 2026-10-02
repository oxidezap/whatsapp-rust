//! Protocol-sized message secrets with explicit byte access and redacted diagnostics.

use std::fmt;

use crate::reporting_token::MESSAGE_SECRET_SIZE;

/// A validated message secret. `Debug` never exposes the key material.
///
/// This is not a zeroizing container: explicit byte access is available for
/// protobuf, crypto and persistence interop, whose buffers may also hold copies.
///
/// ```
/// use wacore::types::message_secret::MessageSecret;
/// let secret = MessageSecret::try_from(vec![177; 32])?;
/// assert_eq!(format!("{secret:?}"), "MessageSecret([REDACTED])");
/// assert!(MessageSecret::try_from(vec![177; 31]).is_err());
/// // Byte access is explicit and sensitive.
/// assert_eq!(secret.into_bytes(), [177; 32]);
/// # Ok::<(), wacore::types::message_secret::InvalidMessageSecret>(())
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct MessageSecret([u8; MESSAGE_SECRET_SIZE]);

/// A secret with an invalid length. Contains only its length, never its bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("message secret must be {MESSAGE_SECRET_SIZE} bytes, got {actual}")]
pub struct InvalidMessageSecret {
    pub actual: usize,
}

impl MessageSecret {
    pub const fn from_bytes(bytes: [u8; MESSAGE_SECRET_SIZE]) -> Self {
        Self(bytes)
    }

    /// Explicit access to secret material; do not log these bytes.
    pub const fn as_bytes(&self) -> &[u8; MESSAGE_SECRET_SIZE] {
        &self.0
    }

    pub fn into_bytes(self) -> [u8; MESSAGE_SECRET_SIZE] {
        self.0
    }
}

impl From<[u8; MESSAGE_SECRET_SIZE]> for MessageSecret {
    fn from(bytes: [u8; MESSAGE_SECRET_SIZE]) -> Self {
        Self::from_bytes(bytes)
    }
}

impl TryFrom<&[u8]> for MessageSecret {
    type Error = InvalidMessageSecret;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        let bytes = bytes.try_into().map_err(|_| InvalidMessageSecret {
            actual: bytes.len(),
        })?;
        Ok(Self(bytes))
    }
}

impl TryFrom<Vec<u8>> for MessageSecret {
    type Error = InvalidMessageSecret;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(bytes.as_slice())
    }
}

impl fmt::Debug for MessageSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MessageSecret([REDACTED])")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_validation_and_explicit_interop() {
        for length in [0, MESSAGE_SECRET_SIZE - 1, MESSAGE_SECRET_SIZE + 1, 4096] {
            let error = MessageSecret::try_from(vec![177; length]).unwrap_err();
            assert_eq!(error.actual, length);
            assert!(!format!("{error:?}").contains("177"));
        }
        let bytes = [177; MESSAGE_SECRET_SIZE];
        let secret = MessageSecret::try_from(bytes.as_slice()).unwrap();
        assert_eq!(secret.as_bytes(), &bytes);
        assert_eq!(secret.clone().into_bytes(), bytes);
        assert!(format!("{bytes:?}").contains("177"), "negative control");
        assert_eq!(format!("{secret:?}"), "MessageSecret([REDACTED])");
        assert_eq!(format!("{secret:#?}"), "MessageSecret([REDACTED])");
    }
}
