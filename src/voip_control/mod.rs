//! The neutral control-plane media contract, re-exported from `wacore::voip_control`.
//!
//! This module is the public spelling of the seam. Its API names no `wacore::voip` type, so it
//! compiles under the `voip-control` feature with the engine off, and a media backend implements
//! [`VoipMediaBackend`]/[`VoipMediaSession`] against it without linking the engine.
//!
//! This is the byte cut, not only the seam: `src/voip` (the facade and call registry) and
//! `src/client/voip.rs` are gated on `voip-control` and compile against these neutral types with
//! the engine off. `voip-engine-wacore` adds the resident `WacoreVoipMediaBackend` on top.

/// The group roster type the [`VoipMediaSession`] contract names, so an external backend
/// implements the trait importing only this module: [`VoipMediaSession::group_update_fits`]
/// takes it by reference, and without this re-export that signature would force a `wacore`
/// import through the seam.
pub use wacore::types::group_call::GroupCallUpdate;
/// Test-only video ports, matching the `wacore::voip_control` gate: the only constructor is the
/// facade's test-only endpoint mapping. `cfg(test)` rather than `test-util` because this crate
/// declares no such feature, and the only in-tree user is the facade's own unit tests.
#[cfg(test)]
pub use wacore::voip_control::MediaVideoPorts;
/// The seam surface, spelled so an external crate implements [`VoipMediaBackend`] importing only
/// `whatsapp_rust::voip_control::*`: every neutral command, event, port, and channel the contract
/// names, with no `wacore` in the path.
pub use wacore::voip_control::{
    AudioSink, AudioSource, CallDirection, EncodedAudioSink, EncodedAudioSource, MediaAudioCodec,
    MediaAudioFormat, MediaAudioIo, MediaAudioPorts, MediaAudioRtpProfile, MediaAudioSpec,
    MediaCloseReason, MediaCodecDecisionSource, MediaCommand, MediaDirectPeer, MediaEncodedFrame,
    MediaEvent, MediaGroupControlKind, MediaGroupEpoch, MediaGroupSpec, MediaGroupTransition,
    MediaKeyframeUrgency, MediaOpenContext, MediaRtcpFeedback, MediaRtcpReportBlock,
    MediaSessionKey, MediaSessionSpec, MediaSetupError, MediaSilenceReason, MediaStats,
    MediaVideoChannels, MediaVideoUpgradeToken, TimedVideoFrame, VideoControl,
    VideoControlReceiver, VideoFrame, VideoInput, VideoSink, VideoSource, VoipMediaBackend,
    VoipMediaSession,
};

#[cfg(feature = "voip-engine-wacore")]
pub mod wacore_backend;
