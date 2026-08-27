//! Auto-generated stanza and notification vocabulary (WhatsApp 2.3000.1045368834). DO NOT EDIT.
//!
//! The tag a stanza arrives under, and the type of a `<notification>`.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand.

/// The tag a stanza arrives under.
///
/// The union of WA Web's stanza dispatcher, the requests the server
/// sends us, and the types this client sends: no one document lists
/// them all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, crate::WireEnum)]
pub enum StanzaTag {
    #[wire = "ack"]
    Ack,
    #[wire = "call"]
    Call,
    #[wire = "chatstate"]
    ChatState,
    #[wire = "error"]
    ErrorStanza,
    #[wire = "failure"]
    Failure,
    #[wire = "ib"]
    InfoBanner,
    #[wire = "iq"]
    Iq,
    #[wire = "message"]
    Message,
    #[wire = "notification"]
    Notification,
    #[wire = "presence"]
    Presence,
    #[wire = "receipt"]
    Receipt,
    #[wire = "status"]
    Status,
    #[wire = "stream:error"]
    StreamError,
    #[wire = "success"]
    Success,
    #[wire = "xmlstreamend"]
    XmlStreamEnd,
}

/// The `type` attribute of an incoming `<notification>`.
///
/// Every value WA Web routes. This client handles a subset and
/// forwards the rest as a raw event, so a variant here is a value the
/// protocol carries, not a promise that anything acts on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, crate::WireEnum)]
pub enum NotificationType {
    #[wire = "account_sync"]
    AccountSync,
    #[wire = "business"]
    Business,
    #[wire = "companion_reg_refresh"]
    CompanionRegRefresh,
    #[wire = "contacts"]
    Contacts,
    #[wire = "crsc_continuation"]
    CrscContinuation,
    #[wire = "devices"]
    Devices,
    #[wire = "digital_commerce_subscription"]
    DigitalCommerceSubscription,
    #[wire = "disappearing_mode"]
    DisappearingMode,
    #[wire = "encrypt"]
    Encrypt,
    #[wire = "fb:update"]
    FbUpdate,
    #[wire = "hosted"]
    Hosted,
    #[wire = "link_code_companion_reg"]
    LinkCodeCompanionReg,
    #[wire = "mediaretry"]
    MediaRetry,
    #[wire = "mex"]
    Mex,
    #[wire = "newsletter"]
    Newsletter,
    #[wire = "passkey_prologue_request"]
    PasskeyPrologueRequest,
    #[wire = "pay"]
    Pay,
    #[wire = "picture"]
    Picture,
    #[wire = "privacy_token"]
    PrivacyToken,
    #[wire = "psa"]
    Psa,
    #[wire = "registration"]
    Registration,
    #[wire = "server"]
    Server,
    #[wire = "server_sync"]
    ServerSync,
    #[wire = "status"]
    Status,
    #[wire = "w:gp2"]
    WGp2,
    #[wire = "w:growth"]
    WGrowth,
    #[wire = "waffle"]
    Waffle,
}
