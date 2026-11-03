// SPDX-License-Identifier: MIT
use super::{CiphertextMessageType, ProtocolAddress};
pub type Result<T> = core::result::Result<T, SignalProtocolError>;
#[derive(Debug, thiserror::Error)]
pub enum SignalProtocolError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    #[error("invalid state in {0}: {1}")]
    InvalidState(&'static str, String),
    #[error("backend error in {0}: {1}")]
    BackendError(
        &'static str,
        #[source] Box<dyn std::error::Error + Send + Sync>,
    ),
    #[error("invalid protobuf encoding")]
    InvalidProtobufEncoding,
    #[error("invalid persisted session: {0}")]
    InvalidSessionStructure(String),
    #[error("unknown prekey")]
    InvalidPreKeyId,
    #[error("unknown signed prekey")]
    InvalidSignedPreKeyId,
    #[error("no session for {0}")]
    SessionNotFound(ProtocolAddress),
    #[error("untrusted identity {0}")]
    UntrustedIdentity(ProtocolAddress),
    #[error("authentication failed for {0:?}")]
    BadMac(CiphertextMessageType),
    #[error("invalid {0:?}: {1}")]
    InvalidMessage(CiphertextMessageType, &'static str),
    #[error("duplicate message, chain {0}, counter {1}")]
    DuplicatedMessage(u32, u32),
    #[error("sender key state missing: {0}")]
    NoSenderKeyState(String),
    #[error("invalid sender key session")]
    InvalidSenderKeySession,
    #[error("signature validation failed")]
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
    Engine(#[from] wacore_signal_engine::Error),
    #[error("external binding: {0}")]
    FfiBindingError(String),
    #[error("sealed sender: {0}")]
    InvalidSealedSenderMessage(String),
    #[error("callback {0}: {1}")]
    ApplicationCallbackError(&'static str, Box<dyn std::error::Error + Send + Sync>),
}
pub(crate) fn message_error(
    error: wacore_signal_engine::Error,
    kind: CiphertextMessageType,
    chain: u32,
    counter: u32,
) -> SignalProtocolError {
    use wacore_signal_engine::Error as E;
    match error {
        E::Authentication => {
            log::warn!("MAC verification failed");
            SignalProtocolError::BadMac(kind)
        }
        E::Duplicate => SignalProtocolError::DuplicatedMessage(chain, counter),
        E::FutureMessage => {
            SignalProtocolError::InvalidMessage(kind, "message counter exceeds future limit")
        }
        E::Encoding(_) => SignalProtocolError::InvalidProtobufEncoding,
        E::Version => SignalProtocolError::UnrecognizedMessageVersion(3),
        error => error.into(),
    }
}

impl SignalProtocolError {
    pub fn is_stored_session_corruption(&self) -> bool {
        matches!(self, Self::InvalidSessionStructure(_))
    }
}
