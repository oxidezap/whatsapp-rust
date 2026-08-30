//! The relay media transport, and the paths that name it.
//!
//! Two things live here and only one of them owns a socket. The native relay stack -- a UDP
//! endpoint per call with DTLS, SCTP and the pre-negotiated DataChannel over it -- is the
//! `native` submodule, gated on `voip-relay-native`, because a UDP socket is exactly what wasm32
//! and espidf do not have.
//!
//! The module itself is not gated, and that is deliberate rather than tidy: `RandTxIds` and the
//! packet demux have always been reachable at `whatsapp_rust::voip::transport::*`, neither one
//! needs a socket, and gating the module around them would break every codec-only consumer that
//! imports one -- a build with `voip-mlow` and no relay is exactly the build a browser makes.

#[cfg(feature = "voip-relay-native")]
mod native;

#[cfg(feature = "voip-relay-native")]
pub use native::*;

// `RandTxIds` used to be defined here and had no business being: an OS-RNG transaction id needs no
// socket. It moved to the portable driver when the native relay became its own feature, and is
// re-exported at its old path -- ungated, because the path was never the socket's.
pub use crate::voip::driver::RandTxIds;

// First-byte relay-packet demux, in the portable core; re-exported so the existing
// `whatsapp_rust::voip::transport::{classify_relay_packet, RelayPacketKind}` paths stay stable on
// every build that has ever had them.
pub use wacore::voip::demux::{RelayPacketKind, classify_relay_packet};
