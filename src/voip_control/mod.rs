//! The neutral control-plane media contract, re-exported from `wacore::voip_control`.
//!
//! This module is the public spelling of the seam. Its API names no `wacore::voip` type, so it
//! compiles under the `voip-control` feature with the engine off, and a media backend implements
//! [`VoipMediaBackend`]/[`VoipMediaSession`] against it without linking the engine.
//!
//! This is the byte cut, not only the seam: `src/voip` (the facade and call registry) and
//! `src/client/voip.rs` are gated on `voip-control` and compile against these neutral types with
//! the engine off. `voip-engine-wacore` adds the resident `WacoreVoipMediaBackend` on top.

pub use wacore::voip_control::{
    CallDirection, MediaAudioCodec, MediaAudioFormat, MediaAudioIo, MediaAudioRtpProfile,
    MediaAudioSpec, MediaCloseReason, MediaCodecDecisionSource, MediaCommand, MediaDirectPeer,
    MediaEncodedFrame, MediaEvent, MediaGroupControlKind, MediaGroupSpec, MediaGroupTransition,
    MediaKeyframeUrgency, MediaRtcpFeedback, MediaRtcpReportBlock, MediaSessionKey,
    MediaSessionSpec, MediaSetupError, MediaSilenceReason, MediaStats, MediaVideoUpgradeToken,
    VoipMediaBackend, VoipMediaSession,
};

#[cfg(feature = "voip-engine-wacore")]
pub mod wacore_backend;
