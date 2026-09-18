//! The neutral control-plane media contract, re-exported from `wacore::voip_control`.
//!
//! This module is the public spelling of the seam. Its API names no `wacore::voip` type, so it
//! compiles under the `voip-control` feature with the engine off, and a media backend implements
//! [`VoipMediaBackend`]/[`VoipMediaSession`] against it without linking the engine.
//!
//! What this module is not: the byte cut. `src/voip` (the facade and call registry) is gated on
//! `voip-runtime` and still names signaling types that live in `wacore::voip`, so a build with only
//! `voip-control` does not compile the call flow at all. Splitting `wacore::voip` into a signaling
//! half and an engine half is the next phase; `agent_docs/subsystem_boundary.md` records which
//! types have to move.

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
