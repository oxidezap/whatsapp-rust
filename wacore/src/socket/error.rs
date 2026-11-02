use crate::handshake::NoiseError;
use thiserror::Error;
use wacore_binary::error::BinaryError;

#[derive(Error)]
#[non_exhaustive]
pub enum SocketError {
    #[error("socket is closed")]
    SocketClosed,
    #[error("I/O error: {}", crate::diagnostics::ErrorSummary(.0))]
    Io(#[from] std::io::Error),
    #[error("noise cipher operation failed: {0}")]
    Cipher(#[from] NoiseError),
    #[error("binary protocol marshalling failed: {}", crate::diagnostics::ErrorSummary(.0))]
    Marshal(#[source] BinaryError),
}

impl std::fmt::Debug for SocketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SocketClosed => f.write_str("SocketClosed"),
            Self::Io(e) => f
                .debug_tuple("Io")
                .field(&crate::diagnostics::ErrorSummary(e))
                .finish(),
            Self::Cipher(e) => f.debug_tuple("Cipher").field(e).finish(),
            Self::Marshal(e) => f
                .debug_tuple("Marshal")
                .field(&crate::diagnostics::ErrorSummary(e))
                .finish(),
        }
    }
}

pub type Result<T> = std::result::Result<T, SocketError>;

/// Outcome of one frame's trip through the noise sender.
pub type EncryptSendResult = std::result::Result<(), EncryptSendError>;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EncryptSendErrorKind {
    #[error("cryptography error")]
    Crypto,
    #[error("framing error")]
    Framing,
    #[error("transport error")]
    Transport,
    #[error("task join error")]
    Join,
    #[error("sender channel closed")]
    ChannelClosed,
    /// A previous frame failed at the transport, so this connection's write
    /// keystream can no longer be extended safely. See
    /// [`EncryptSendError::poisoned`].
    #[error("sender poisoned by an earlier transport failure")]
    Poisoned,
}

#[derive(thiserror::Error)]
#[error("{kind}: {}", crate::diagnostics::ErrorSummary(.source.as_ref()))]
#[non_exhaustive]
pub struct EncryptSendError {
    pub kind: EncryptSendErrorKind,
    #[source]
    pub source: anyhow::Error,
}

impl std::fmt::Debug for EncryptSendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptSendError")
            .field("kind", &self.kind)
            .field(
                "source",
                &crate::diagnostics::ErrorSummary(self.source.as_ref()),
            )
            .finish()
    }
}

impl EncryptSendError {
    pub fn crypto(source: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: EncryptSendErrorKind::Crypto,
            source: source.into(),
        }
    }

    pub fn framing(source: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: EncryptSendErrorKind::Framing,
            source: source.into(),
        }
    }

    pub fn transport(source: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: EncryptSendErrorKind::Transport,
            source: source.into(),
        }
    }

    pub fn join(source: impl Into<anyhow::Error>) -> Self {
        Self {
            kind: EncryptSendErrorKind::Join,
            source: source.into(),
        }
    }

    pub fn channel_closed() -> Self {
        Self {
            kind: EncryptSendErrorKind::ChannelClosed,
            source: anyhow::anyhow!("sender task channel closed unexpectedly"),
        }
    }

    /// A transport write failed earlier on this connection, so the peer's view
    /// of the frame stream is unknown: it may have consumed the frame, seen a
    /// truncated prefix, or nothing at all. Encrypting anything else under the
    /// same write key would have to guess a counter, and guessing wrong reuses
    /// an AES-GCM nonce (two ciphertexts under one key/nonce leak both
    /// plaintexts). The only safe recovery is a new connection with fresh
    /// handshake keys, so the sender refuses every later frame instead.
    pub fn poisoned() -> Self {
        Self {
            kind: EncryptSendErrorKind::Poisoned,
            source: anyhow::anyhow!(
                "noise sender disabled after a transport failure; reconnect to rekey"
            ),
        }
    }

    /// The transport is gone (broken pipe, closed connection, channel dropped)
    /// or was declared unusable by [`Self::poisoned`]. Callers treat all three
    /// the same way: stop retrying on this connection and reconnect.
    pub fn is_transport_unavailable(&self) -> bool {
        matches!(
            self.kind,
            EncryptSendErrorKind::Transport
                | EncryptSendErrorKind::ChannelClosed
                | EncryptSendErrorKind::Poisoned
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libsignal::crypto::CryptoProviderError;

    #[test]
    fn cipher_preserves_noise_source_through_socket_error() {
        let noise = NoiseError::Decrypt(CryptoProviderError::AuthFailed);
        let se: SocketError = noise.into();
        // First hop: SocketError -> NoiseError
        let src = std::error::Error::source(&se).expect("source preserved");
        let ne = src
            .downcast_ref::<NoiseError>()
            .expect("downcasts to NoiseError");
        assert!(matches!(ne, NoiseError::Decrypt(_)));
        // Second hop: NoiseError -> CryptoProviderError
        let inner = std::error::Error::source(ne).expect("inner source preserved");
        let cpe = inner
            .downcast_ref::<CryptoProviderError>()
            .expect("downcasts to CryptoProviderError");
        assert!(matches!(cpe, CryptoProviderError::AuthFailed));
    }

    #[test]
    fn crypto_preserves_the_noise_error_type() {
        let err = EncryptSendError::crypto(NoiseError::Encrypt(CryptoProviderError::BackendFailed));
        assert!(matches!(err.kind, EncryptSendErrorKind::Crypto));
        let src = std::error::Error::source(&err).expect("source preserved");
        let ne = src
            .downcast_ref::<NoiseError>()
            .expect("downcasts to NoiseError");
        assert!(matches!(ne, NoiseError::Encrypt(_)));
    }

    #[test]
    fn crypto_from_an_untyped_source_still_carries_its_message() {
        let err = EncryptSendError::crypto(anyhow::anyhow!("some opaque failure"));
        let src = std::error::Error::source(&err).expect("source preserved");
        assert!(src.downcast_ref::<NoiseError>().is_none());
        assert_eq!(src.to_string(), "some opaque failure");
    }

    #[test]
    fn marshal_preserves_binary_error_source() {
        let be = BinaryError::InvalidNode;
        let se = SocketError::Marshal(be);
        let src = std::error::Error::source(&se).expect("source preserved");
        let inner = src
            .downcast_ref::<BinaryError>()
            .expect("downcasts to BinaryError");
        assert!(matches!(inner, BinaryError::InvalidNode));
    }
}
