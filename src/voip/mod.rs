//! VoIP calls media plane (Tokio runtime side): the DTLS/SCTP DataChannel transport
//! over the WhatsApp relay, encoded audio, the call state machine, and the media pipeline.
//! Pure protocol/crypto lives in `wacore::voip`.
//!
//! This module drives the media stack over a Tokio UDP socket, which does not exist on wasm32 or
//! espidf. The wasm/esp32-safe subset is `wacore`'s `voip` feature (pure-Rust crypto and encoded
//! transport); add `wacore/voip-mlow` for its pure-Rust MLOW codec.

// Fail fast with an actionable message instead of a confusing link error further down.
#[cfg(all(
    feature = "voip-runtime",
    any(target_arch = "wasm32", target_os = "espidf")
))]
compile_error!(
    "the native VoIP features of `whatsapp-rust` drive the relay media stack over a Tokio UDP \
     socket and do not build on wasm32/espidf. For those targets use `wacore/voip` for \
     crypto/encoded transport and optionally `wacore/voip-mlow` for the pure-Rust MLOW codec."
);

pub mod audio;
pub mod driver;
pub mod facade;
pub mod registry;
pub mod session;
mod state;
pub mod transport;
pub mod video;

pub use state::collections;

pub(crate) use state::Voip;

pub use audio::{AudioSink, AudioSource, EncodedAudioSink, EncodedAudioSource};
pub use facade::{
    AcceptCall, CallHandle, CallLinkCall, CallTermination, GroupBoundCall, OutgoingCall,
    OutgoingGroupCall,
};
pub use video::{VideoFrame, VideoSink, VideoSource};
// Surface core types carried by the facade next to the builders and handle that expose them.
pub use wacore::voip::{
    AudioCodec, AudioConfig, AudioFormat, AudioIo, AudioRtpProfile, EncodedAudioFrame,
    OpusMlowPacketError, depacketize_opus_from_mlow, packetize_opus_for_mlow,
};
pub use wacore::voip::{CallEvent, GroupCallState, GroupStateApply, VideoUpgradeToken};
// `CallEvent::VideoStateChanged` carries this; surface it next to CallEvent (it lives in wacore).
pub use wacore::types::call::VideoState;
pub use wacore::types::group_call::{
    CallLink, CallLinkJoin, CallLinkMedia, CallLinkPreview, GROUP_CALL_MAX_PARTICIPANTS,
    GroupCallDevice, GroupCallEncRekey, GroupCallParticipant, GroupCallRelay,
    GroupCallRelayEndpoint, GroupCallUpdate, ScreenShare, ScreenShareState, WaitingRoom,
    WaitingRoomUser,
};
