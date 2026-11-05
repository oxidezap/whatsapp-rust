//! Experimental, runtime-independent foundations for mobile clients.
//!
//! This module defines configuration and workflow policy, not an implemented
//! registration wire protocol. Hosts supply time, transport, HTTP, randomness,
//! persistence and platform services. No mobile request is sent by these types.
//! Official APK qualification is still required before adding wire builders.

mod environment;
mod registration;

pub use environment::{
    EnvironmentBinding, EnvironmentBound, MobileConfigurationError, MobileEnvironment, MobileTrust,
    RegistrationEndpoint, TcpEndpoint,
};
pub use registration::{
    AttemptToken, RegistrationCheckpoint, RegistrationError, RegistrationIdentity,
    RegistrationOperation, RegistrationOutcome, RegistrationPolicy, RegistrationRunId,
    RegistrationStage, RegistrationWorkflow,
};
