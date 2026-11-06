//! Experimental, runtime-independent foundations for mobile clients.
//!
//! Hosts supply time, transport, HTTP, randomness, persistence and platform
//! services. Reference-derived wire components remain experimental until
//! independently qualified against the pinned official client artifacts.

mod environment;
pub mod primary;
mod registration;
pub mod registration_wire;
mod session;

pub use environment::{
    EnvironmentBinding, EnvironmentBound, MobileConfigurationError, MobileEnvironment, MobileTrust,
    RegistrationEndpoint, TcpEndpoint,
};
pub use registration::{
    AttemptToken, RegistrationCheckpoint, RegistrationCommitToken, RegistrationError,
    RegistrationIdentity, RegistrationOperation, RegistrationOutcome, RegistrationPolicy,
    RegistrationRunId, RegistrationStage, RegistrationWorkflow,
};
pub use session::{
    MobileAppVersion, MobileCertificateExpiryPolicy, MobileDevice, MobileLoginOptions,
    MobileLoginProfile, MobileNoisePrologue, MobileNoiseTrust, MobilePlatform, MobileSessionError,
    MobileXxHandshake, MobileXxHandshakeOutcome, VerifiedMobileCertificate,
};

mod token;
pub use token::{AndroidTokenError, AndroidTokenMaterial, ios_registration_token};
