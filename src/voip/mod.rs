//! VoIP calls media plane: the call state machine, the media pipeline, encoded audio, and the
//! relay transport that carries them. Pure protocol/crypto lives in `wacore::voip`.
//!
//! # Two halves, and only one of them owns a socket
//!
//! Everything here except [`transport`] is portable: it drives `wacore`'s sans-IO engine over an
//! injected [`Runtime`](wacore::runtime::Runtime) and an injected
//! [`RelayTransportFactory`](wacore::voip::transport::RelayTransportFactory), so it names no clock
//! and opens no descriptor. That half is `voip-runtime`, and it builds wherever `wacore` does.
//!
//! [`transport`] is the other half -- a UDP socket per call with DTLS, SCTP and the pre-negotiated
//! DataChannel over it -- and it is `voip-relay-native`, because a UDP socket is exactly what
//! wasm32 and espidf do not have. A page reaches the same relay through an `RTCPeerConnection`,
//! which is a factory it supplies with [`Client::set_relay_transport_factory`]; it needs no part of
//! this module's native half, and asking for one used to be a `compile_error!` across the whole
//! feature rather than across the socket.
//!
//! [`Client::set_relay_transport_factory`]: crate::client::Client::set_relay_transport_factory

// Fail fast with an actionable message instead of a confusing link error further down. Narrowed to
// the transport: `voip-runtime` alone is portable, and the message now names the way forward
// rather than only the wall.
#[cfg(all(
    feature = "voip-relay-native",
    any(target_arch = "wasm32", target_os = "espidf")
))]
compile_error!(
    "`voip-relay-native` drives the relay media stack over a Tokio UDP socket and does not build \
     on wasm32/espidf. Enable `voip-mlow` (or `voip-encoded`) without it and supply your own \
     `RelayTransportFactory` through `Client::set_relay_transport_factory` -- in a browser an \
     `RTCPeerConnection` with a pre-negotiated DataChannel reaches the same relay."
);

pub mod audio;
pub mod driver;
pub mod facade;
pub mod registry;
pub mod session;
mod state;
#[cfg(feature = "voip-relay-native")]
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
// The platform transport seam, beside the facade that consults it: a consumer installing one
// through `Client::set_relay_transport_provider` reaches for all three, and having to name `wacore`
// for them while naming `whatsapp_rust` for the call is a paper cut on the one path this crate now
// asks a platform to implement.
pub use wacore::voip::{
    RelayEndpointParams, RelayTransport, RelayTransportEvent, RelayTransportFactory,
    RelayTransportProvider,
};
// `CallEvent::VideoStateChanged` carries this; surface it next to CallEvent (it lives in wacore).
pub use wacore::types::call::VideoState;
pub use wacore::types::group_call::{
    CallLink, CallLinkJoin, CallLinkMedia, CallLinkPreview, GROUP_CALL_MAX_PARTICIPANTS,
    GroupCallDevice, GroupCallEncRekey, GroupCallParticipant, GroupCallRelay,
    GroupCallRelayEndpoint, GroupCallUpdate, ScreenShare, ScreenShareState, WaitingRoom,
    WaitingRoomUser,
};
