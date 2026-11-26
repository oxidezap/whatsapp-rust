//! Primary-side companion approval primitives.
//!
//! ADV messages use the existing companion verifiers and generated protobufs.
//! Mobile request composition additionally follows the pinned reference clients;
//! it is not evidence of official server acceptance. Hosts own entropy, time,
//! request delivery and durable publication of the resulting device state.

mod exchange;
mod identity;
mod iq;
mod pair_code;
mod qr;

pub use exchange::{AcknowledgedCompanion, PendingCompanion, VerifiedCompanion};
pub use identity::{
    AdvDefaultEncoding, IdentityParameters, PrimarySelfIdentityProfile, SignedCompanionIdentity,
    build_signed_companion_identity, build_signed_key_index_list, next_companion_key_index,
    validate_primary_account_identity, validate_primary_account_identity_with_profile,
    verify_companion_proof,
};
pub use iq::{
    PairDeviceAcknowledgement, PairDevicePem, PairDeviceSpec, PrimaryHelloSpec,
    PublishKeyIndexListSpec, RemoveAllCompanionsSpec, RemovePrimaryCompanionSpec,
    derive_companion_pem_key,
};
pub use pair_code::{PrimaryHello, prepare_primary_hello};
pub use qr::CompanionQr;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PrimaryLinkError {
    #[error("invalid companion QR: {0}")]
    InvalidQr(&'static str),
    #[error("invalid primary linking input: {0}")]
    InvalidInput(&'static str),
    #[error("primary linking cryptographic authentication failed")]
    Authentication,
    #[error("invalid primary linking response: {0}")]
    InvalidResponse(&'static str),
    #[error("invalid Curve25519 key or signature")]
    Curve(#[from] crate::libsignal::protocol::CurveError),
    #[error("invalid primary linking protobuf")]
    Protobuf(#[from] waproto::protobuf::DecodeError),
    #[error("primary linking AEAD failed")]
    Aead(#[from] crate::libsignal::crypto::CryptoProviderError),
    #[error("primary linking key derivation failed")]
    KeyDerivation,
}

pub(super) fn bounded_text(value: &str, limit: usize) -> bool {
    !value.is_empty() && value.len() <= limit && !value.chars().any(char::is_control)
}

pub(super) fn validate_accounts(accounts: &[wacore_binary::Jid]) -> Result<(), PrimaryLinkError> {
    use wacore_binary::Server;
    if accounts.is_empty()
        || accounts.len() > 2
        || accounts.iter().any(|jid| {
            jid.user.is_empty()
                || jid.device != 0
                || jid.agent != 0
                || jid.integrator != 0
                || !matches!(jid.server, Server::Pn | Server::Lid)
        })
        || accounts.len() == 2 && accounts[0].server == accounts[1].server
    {
        return Err(PrimaryLinkError::InvalidInput("primary account JIDs"));
    }
    Ok(())
}

pub(super) fn validate_companion(
    jid: &wacore_binary::Jid,
    accounts: &[wacore_binary::Jid],
) -> Result<(), PrimaryLinkError> {
    use wacore_binary::{JidExt as _, Server};
    validate_accounts(accounts)?;
    if jid.device == 0
        || jid.agent != 0
        || jid.integrator != 0
        || jid.is_hosted()
        || !matches!(jid.server, Server::Pn | Server::Lid)
        || !accounts
            .iter()
            .any(|account| account.user == jid.user && account.server == jid.server)
    {
        return Err(PrimaryLinkError::Authentication);
    }
    Ok(())
}
