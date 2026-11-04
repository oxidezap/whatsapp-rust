// SPDX-License-Identifier: MIT
use super::{CiphertextMessageType, ProtocolAddress};
pub type Result<T> = core::result::Result<T, SignalProtocolError>;
#[derive(thiserror::Error)]
pub enum SignalProtocolError {
    #[error("invalid argument")]
    InvalidArgument(String),
    #[error("invalid state in {0}")]
    InvalidState(&'static str, String),
    #[error("backend error in {0}")]
    BackendError(
        &'static str,
        #[source] Box<dyn std::error::Error + Send + Sync>,
    ),
    #[error("invalid protobuf encoding")]
    InvalidProtobufEncoding,
    #[error("invalid persisted session")]
    InvalidSessionStructure(String),
    #[error("unknown prekey")]
    InvalidPreKeyId,
    #[error("unknown signed prekey")]
    InvalidSignedPreKeyId,
    #[error("no session for {}", .0.observe())]
    SessionNotFound(Box<ProtocolAddress>),
    #[error("untrusted identity {}", .0.observe())]
    UntrustedIdentity(Box<ProtocolAddress>),
    #[error("authentication failed for {0:?}")]
    BadMac(CiphertextMessageType),
    #[error("invalid {0:?}: {1}")]
    InvalidMessage(CiphertextMessageType, &'static str),
    #[error("duplicate message, chain {0}, counter {1}")]
    DuplicatedMessage(u32, u32),
    #[error("sender key state missing")]
    NoSenderKeyState(String),
    #[error("invalid sender key session")]
    InvalidSenderKeySession,
    #[error("invalid signature")]
    SignatureValidationFailed,
    #[error("key agreement failed: {0}")]
    KeyAgreementFailed(crate::crypto::CryptoProviderError),
    #[error("ciphertext too short: {0}")]
    CiphertextMessageTooShort(usize),
    #[error("legacy ciphertext version {0}")]
    LegacyCiphertextVersion(u8),
    #[error("unrecognized ciphertext version {0}")]
    UnrecognizedCiphertextVersion(u8),
    #[error("unrecognized message version {0}")]
    UnrecognizedMessageVersion(u32),
    #[error(transparent)]
    Curve(#[from] crate::core::CurveError),
    #[error(transparent)]
    Crypto(#[from] crate::crypto::CryptoProviderError),
    #[error(transparent)]
    Engine(#[from] crate::engine::Error),
    #[error("ffi callback error")]
    FfiBindingError(String),
    #[error("invalid sealed sender message")]
    InvalidSealedSenderMessage(String),
    #[error("callback {0} failed")]
    ApplicationCallbackError(&'static str, Box<dyn std::error::Error + Send + Sync>),
}
pub(crate) fn message_error(
    error: crate::engine::Error,
    kind: CiphertextMessageType,
    chain: u32,
    counter: u32,
) -> SignalProtocolError {
    use crate::engine::Error as E;
    match error {
        E::Authentication => {
            log::warn!("MAC verification failed");
            SignalProtocolError::BadMac(kind)
        }
        E::Duplicate => SignalProtocolError::DuplicatedMessage(chain, counter),
        E::ClosedChain => {
            SignalProtocolError::InvalidSessionStructure("receiver chain is closed".into())
        }
        E::FutureMessage => {
            SignalProtocolError::InvalidMessage(kind, "message counter exceeds future limit")
        }
        E::InvalidCiphertext => {
            SignalProtocolError::InvalidMessage(kind, "invalid ciphertext padding")
        }
        E::Crypto => SignalProtocolError::Crypto(crate::crypto::CryptoProviderError::BackendFailed),
        E::Encoding(_) => SignalProtocolError::InvalidProtobufEncoding,
        E::Version => SignalProtocolError::UnrecognizedMessageVersion(3),
        error => error.into(),
    }
}

impl SignalProtocolError {
    pub fn is_stored_session_corruption(&self) -> bool {
        matches!(self, Self::InvalidSessionStructure(reason) if reason != "receiver chain is closed")
    }
}

impl core::fmt::Debug for SignalProtocolError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn protocol_errors_do_not_inflate_every_result_and_async_frame() {
        assert!(
            size_of::<SignalProtocolError>() <= 48,
            "protocol errors occupy {} bytes",
            size_of::<SignalProtocolError>()
        );
        assert!(size_of::<Result<()>>() <= 48);
    }
}
