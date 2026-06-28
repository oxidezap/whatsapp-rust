//! VoIP calls media plane (Tokio runtime side): the DTLS/SCTP DataChannel transport
//! over the WhatsApp relay, Opus audio, the call state machine, and the media pipeline.
//! Pure protocol/crypto lives in `wacore::voip`.
//!
//! This module pulls webrtc-rs (DTLS/SCTP) and the libopus FFI, neither of which builds on
//! wasm32 or espidf. The wasm/esp32-safe subset is `wacore`'s `voip` feature (pure-Rust crypto
//! + the MLow codec), which has no FFI and stays buildable on those targets.

// Fail fast with an actionable message instead of a confusing webrtc/opus FFI link error.
#[cfg(all(feature = "voip", any(target_arch = "wasm32", target_os = "espidf")))]
compile_error!(
    "the `voip` feature of `whatsapp-rust` pulls webrtc-rs + libopus FFI and does not build on \
     wasm32/espidf. For those targets enable only `wacore`'s `voip` feature (pure-Rust crypto + \
     the MLow codec)."
);

pub mod audio;
pub mod driver;
pub mod facade;
pub mod registry;
pub mod session;
pub mod transport;

pub use audio::{AudioSink, AudioSource};
pub use facade::{AcceptCall, CallHandle, OutgoingCall};
// CallHandle::events() yields these, so surface them next to CallHandle (they live in wacore).
pub use wacore::voip::CallEvent;
