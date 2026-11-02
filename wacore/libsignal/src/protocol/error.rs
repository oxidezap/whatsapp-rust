//
// Copyright 2020-2021 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::panic::UnwindSafe;

use crate::{
    core::{
        ProtocolAddress,
        curve::{CurveError, KeyType},
    },
    crypto::CryptoProviderError,
    protocol::CiphertextMessageType,
};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SignalProtocolError>;

#[derive(Error)]
pub enum SignalProtocolError {
    #[error("invalid argument (detail bytes={})", .0.len())]
    InvalidArgument(String),
    #[error("invalid state for call to {method} to succeed (detail bytes={bytes})", method = .0, bytes = .1.len())]
    InvalidState(&'static str, String),

    #[error("backend store error in {0}")]
    BackendError(
        &'static str,
        #[source] Box<dyn std::error::Error + Send + Sync>,
    ),

    #[error("protobuf encoding was invalid")]
    InvalidProtobufEncoding,

    #[error("ciphertext serialized bytes were too short <{0}>")]
    CiphertextMessageTooShort(usize),
    #[error("ciphertext version was too old <{0}>")]
    LegacyCiphertextVersion(u8),
    #[error("ciphertext version was unrecognized <{0}>")]
    UnrecognizedCiphertextVersion(u8),
    #[error("unrecognized message version <{0}>")]
    UnrecognizedMessageVersion(u32),

    #[error("fingerprint version number mismatch them {0} us {1}")]
    FingerprintVersionMismatch(u32, u32),
    #[error("fingerprint parsing error")]
    FingerprintParsingError,

    #[error("no key type identifier")]
    NoKeyTypeIdentifier,
    #[error("bad key type <{0:#04x}>")]
    BadKeyType(u8),
    #[error("bad key length <{1}> for key with type <{0}>")]
    BadKeyLength(KeyType, usize),

    #[error("invalid signature detected")]
    SignatureValidationFailed,

    #[error("the active crypto provider failed the key agreement")]
    KeyAgreementFailed(#[source] CryptoProviderError),

    #[error("untrusted identity for address {}", .0.observe())]
    UntrustedIdentity(ProtocolAddress),

    #[error("invalid prekey identifier")]
    InvalidPreKeyId,
    #[error("invalid signed prekey identifier")]
    InvalidSignedPreKeyId,

    #[error("invalid MAC key length <{0}>")]
    InvalidMacKeyLength(usize),

    #[error("no sender key state (detail bytes={})", .0.len())]
    NoSenderKeyState(String),

    #[error("session with {} not found", .0.observe())]
    SessionNotFound(ProtocolAddress),
    #[error("invalid session: {0}")]
    InvalidSessionStructure(&'static str),
    #[error("invalid sender key session")]
    InvalidSenderKeySession,
    #[error("session for {peer} has invalid registration ID {1:X}", peer = .0.observe())]
    InvalidRegistrationId(ProtocolAddress, u32),

    #[error("message with old counter {0} / {1}")]
    DuplicatedMessage(u32, u32),
    #[error("invalid {0:?} message: {1}")]
    InvalidMessage(CiphertextMessageType, &'static str),
    #[error("MAC verification failed for {0:?} message")]
    BadMac(CiphertextMessageType),

    #[error("error while invoking an ffi callback (detail bytes={})", .0.len())]
    FfiBindingError(String),
    #[error("error in application callback method '{0}'")]
    ApplicationCallbackError(
        &'static str,
        #[source] Box<dyn std::error::Error + Send + Sync + UnwindSafe + 'static>,
    ),

    #[error("invalid sealed sender message (detail bytes={})", .0.len())]
    InvalidSealedSenderMessage(String),
    #[error("unknown sealed sender message version {0}")]
    UnknownSealedSenderVersion(u8),
    #[error("self send of a sealed sender message")]
    SealedSenderSelfSend,
}

// Match Display's deliberate fields: an arbitrary backend/callback Debug may
// contain stored keys or plaintext. Typed sources remain available via source().
impl std::fmt::Debug for SignalProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl From<CurveError> for SignalProtocolError {
    fn from(e: CurveError) -> Self {
        match e {
            CurveError::NoKeyTypeIdentifier => Self::NoKeyTypeIdentifier,
            CurveError::BadKeyType(raw) => Self::BadKeyType(raw),
            CurveError::BadKeyLength(key_type, len) => Self::BadKeyLength(key_type, len),
            CurveError::AgreementFailed(source) => Self::KeyAgreementFailed(source),
        }
    }
}

/// The one [`SignalProtocolError::InvalidSessionStructure`] that is not this
/// device's stored state failing.
///
/// `session_cipher` raises it when a message arrives on a receiver chain we
/// closed — a fact about the message, not about the row it was decrypted
/// against. Both raise sites use this constant so the string cannot drift away
/// from the predicate that reads it.
pub(crate) const CLOSED_RECEIVER_CHAIN: &str = "receiver chain is closed";

impl SignalProtocolError {
    /// Whether this error is a stored session record that decoded and then would
    /// not yield usable state, rather than a verdict on the message.
    ///
    /// Every producer of [`Self::InvalidSessionStructure`] is reading persisted
    /// session state — most through `InvalidSessionError`, which exists, in its
    /// own words, "to keep from accidentally propagating deserialization
    /// errors" — with the single exception of a closed receiver chain.
    ///
    /// Callers use this to avoid reporting our own corrupt state against the
    /// peer who sent the message. It answers only that question: it says nothing
    /// about whether the state is recoverable, and it is not a check for every
    /// kind of local storage trouble (a store that cannot be read at all
    /// surfaces as [`Self::BackendError`] instead).
    pub fn is_stored_session_corruption(&self) -> bool {
        matches!(self, Self::InvalidSessionStructure(what) if *what != CLOSED_RECEIVER_CHAIN)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_receiver_chain_is_not_stored_corruption() {
        assert!(
            !SignalProtocolError::InvalidSessionStructure(CLOSED_RECEIVER_CHAIN)
                .is_stored_session_corruption(),
            "a chain we closed is a fact about the message, not a corrupt row",
        );
        assert!(
            SignalProtocolError::InvalidSessionStructure(
                "cannot decrypt without remote identity key"
            )
            .is_stored_session_corruption(),
            "session state missing its remote identity key is ours",
        );
        assert!(
            SignalProtocolError::InvalidSessionStructure("invalid receiver chain message keys")
                .is_stored_session_corruption(),
            "message keys the cipher rejects are ours — libsignal logs the state as corrupt",
        );
        assert!(
            !SignalProtocolError::InvalidSenderKeySession.is_stored_session_corruption(),
            "the sender-key variant is classified on its own, not through this one",
        );
    }

    #[derive(Debug, thiserror::Error)]
    #[error("synthetic backend failure: {code}")]
    struct DummyBackendError {
        code: u32,
    }

    #[test]
    fn backend_error_preserves_typed_source_via_downcast() {
        let dummy = DummyBackendError { code: 42 };
        let spe = SignalProtocolError::BackendError("test_context", Box::new(dummy));
        let src = std::error::Error::source(&spe).expect("source preserved");
        let inner = src
            .downcast_ref::<DummyBackendError>()
            .expect("downcasts to DummyBackendError");
        assert_eq!(inner.code, 42);
    }
    #[test]
    fn error_formatting_keeps_peer_and_failure_context_without_backend_payloads() {
        let address = ProtocolAddress::new("19990000001@s.whatsapp.net", 7.into());
        for error in [
            SignalProtocolError::UntrustedIdentity(address.clone()),
            SignalProtocolError::SessionNotFound(address.clone()),
            SignalProtocolError::InvalidRegistrationId(address, 123),
        ] {
            for output in [error.to_string(), format!("{error:#?}")] {
                assert!(output.contains("peer#"));
                assert!(output.contains(".7"));
                assert!(!output.contains("19990000001"));
            }
        }
        for error in [
            SignalProtocolError::InvalidArgument("synthetic-private-key-bytes".into()),
            SignalProtocolError::InvalidState(
                "message_encrypt",
                "synthetic-private-key-bytes".into(),
            ),
            SignalProtocolError::NoSenderKeyState("synthetic-private-key-bytes".into()),
            SignalProtocolError::FfiBindingError("synthetic-private-key-bytes".into()),
            SignalProtocolError::InvalidSealedSenderMessage("synthetic-private-key-bytes".into()),
        ] {
            for output in [error.to_string(), format!("{error:#?}")] {
                assert!(!output.contains("synthetic-private-key-bytes"));
                assert!(output.contains("detail bytes="));
            }
            match error {
                SignalProtocolError::InvalidArgument(text)
                | SignalProtocolError::InvalidState(_, text)
                | SignalProtocolError::NoSenderKeyState(text)
                | SignalProtocolError::FfiBindingError(text)
                | SignalProtocolError::InvalidSealedSenderMessage(text) => {
                    assert_eq!(text, "synthetic-private-key-bytes");
                }
                _ => unreachable!("fixture uses string-bearing external errors"),
            }
        }
        let error = SignalProtocolError::BackendError(
            "load_session",
            Box::new(std::io::Error::other("synthetic-private-key-bytes")),
        );
        assert!(format!("{error:#?}").contains("load_session"));
        assert!(!format!("{error:#?}").contains("synthetic-private-key-bytes"));
        assert!(
            std::error::Error::source(&error)
                .expect("backend source remains available")
                .to_string()
                .contains("synthetic-private-key-bytes")
        );
    }
}
