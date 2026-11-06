//! Publication boundary for the optional mobile SDK.

#[path = "../mobile/primary.rs"]
pub mod mobile_primary;
#[path = "../mobile/registration.rs"]
pub mod mobile_registration;
#[path = "../mobile_session.rs"]
pub mod mobile_session;

pub use mobile_session::{
    MobileAppStatePolicy, MobileEphemeralKeyProvider, MobileLoginAttempt,
    MobileLoginOptionsProvider, RegisteredMobileSession, RegistrationLoginPhoneIdMapping,
    validate_registration_login_profile,
};
pub use wacore::mobile;
pub use wacore::mobile::primary::PrimarySelfIdentityProfile;
pub use wacore::{download::MobileMediaHttpProfile, send::PrimaryDeviceIdentityPolicy};
