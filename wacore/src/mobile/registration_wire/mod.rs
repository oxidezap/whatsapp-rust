//! Registration request/response codecs, independent of transport and providers.
//!
//! Wire shapes are traced to the pinned Cobalt and whalibmob registration drivers.
//! Their differences are explicit profile inputs, not guessed current defaults.
//! Encoding a request does not establish official APK or server qualification.
//!
//! Fixed reference provenance:
//! - Cobalt common fields, encrypted envelope and operation branches:
//!   <https://github.com/Auties00/Cobalt/blob/cb9fbf172507626948fbc3611d0a6fb00bb3a0f1/modules/lib/src/main/java/com/github/auties00/cobalt/registration/MobileClientRegistration.java>
//! - whalibmob ordering, padding and query differences:
//!   <https://github.com/Kunboruto20/whalibmob/blob/8f50db4c586534dc0af4d13b26bdd09ef42660c0/lib/Registration.js>
//!
//! These references are implementation evidence, not current APK qualification.
//! No production destination, current version, token or server key is assumed.

mod envelope;
mod form;
mod request;
mod response;

pub use envelope::{EphemeralRegistrationKey, RegistrationEnvelope};
pub use form::{Base64Padding, RegistrationForm};
pub use request::{
    AttestationPolicy, BodyAttestation, CodeRequestPlan, RegistrationAppVersion,
    RegistrationCommonFields, RegistrationEncoding, RegistrationHeaderProfile,
    RegistrationHttpRequest, RegistrationIdentityFields, RegistrationLoginMetadata,
    RegistrationMaterial, RegistrationMaterialPersistence, RegistrationPlatform,
    RegistrationRequest, RegistrationRequestBuilder, SealedRegistrationRequest, VerificationMethod,
    flash_code_from_caller_id,
};
pub use response::{
    RegistrationChallenge, RegistrationDeliveryHints, RegistrationResponse,
    RegistrationResponseDecision, RegistrationResponseStatus, RegistrationWaitHints,
};

/// A relative registration operation, never a production destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum RegistrationEndpointPath {
    #[wire = "/exist"]
    Exist,
    #[wire = "/code"]
    Code,
    #[wire = "/register"]
    Register,
    #[wire = "/challenge"]
    Challenge,
    #[wire = "/security"]
    Security,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum RegistrationWireError {
    #[error("registration data exceeds its byte or field budget")]
    SizeLimit,
    #[error("invalid registration field {0}")]
    InvalidField(&'static str),
    #[error("duplicate registration field")]
    DuplicateField,
    #[error("missing required registration material {0}")]
    MissingMaterial(&'static str),
    #[error("registration cryptographic provider failed")]
    Crypto,
    #[error("registration peer key produces an all-zero agreement")]
    InvalidServerKey,
    #[error("malformed registration JSON response")]
    InvalidJson,
    #[error("invalid registration response field type {0}")]
    InvalidResponseField(String),
    #[error("malformed registration challenge encoding")]
    InvalidChallenge,
}

pub(crate) const FORM_LIMIT: usize = 128 * 1024;
pub(crate) const FIELD_LIMIT: usize = 256;
pub(crate) const RESPONSE_LIMIT: usize = 256 * 1024;

pub(crate) fn header_value(value: &str) -> Result<(), RegistrationWireError> {
    if value.is_empty()
        || value.len() > 16 * 1024
        || !value.is_ascii()
        || value.chars().any(char::is_control)
    {
        return Err(RegistrationWireError::InvalidField("HTTP header"));
    }
    Ok(())
}
