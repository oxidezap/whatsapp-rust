//! VoIP calls media plane (Tokio runtime side): the DTLS/SCTP DataChannel transport
//! over the WhatsApp relay, encoded audio, the call state machine, and the media pipeline.
//! Pure protocol/crypto lives in `wacore::voip`.
//!
//! This module pulls webrtc-rs, which does not build on wasm32 or espidf. The wasm/esp32-safe
//! subset is `wacore`'s `voip` feature (pure-Rust crypto and encoded transport); add
//! `wacore/voip-mlow` for its pure-Rust MLOW codec.

// Fail fast with an actionable message instead of a confusing webrtc link error.
#[cfg(all(
    feature = "voip-runtime",
    any(target_arch = "wasm32", target_os = "espidf")
))]
compile_error!(
    "the native VoIP features of `whatsapp-rust` pull webrtc-rs and do not build on \
     wasm32/espidf. For those targets use `wacore/voip` for crypto/encoded transport and \
     optionally `wacore/voip-mlow` for the pure-Rust MLOW codec."
);

pub mod audio;
pub mod driver;
pub mod facade;
pub mod registry;
pub mod session;
pub mod transport;
pub mod video;

pub use audio::{AudioSink, AudioSource, EncodedAudioSink, EncodedAudioSource};
pub use facade::{AcceptCall, CallHandle, OutgoingCall};
pub use video::{VideoFrame, VideoSink, VideoSource};
// CallHandle::events() yields these, so surface them next to CallHandle (they live in wacore).
pub use wacore::voip::{
    AudioCodec, AudioConfig, AudioFormat, AudioIo, AudioRtpProfile, EncodedAudioFrame,
    OpusMlowPacketError, depacketize_opus_from_mlow, packetize_opus_for_mlow,
};
pub use wacore::voip::{CallEvent, VideoUpgradeToken};
// `CallEvent::VideoStateChanged` carries this; surface it next to CallEvent (it lives in wacore).
pub use wacore::types::call::VideoState;
