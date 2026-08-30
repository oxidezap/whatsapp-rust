//! The relay media transport, and the paths that name it.
//!
//! The native relay stack -- a UDP endpoint per call with DTLS, SCTP and the pre-negotiated
//! DataChannel over it -- is the `native` submodule, gated on `voip-relay-native`. Why that half
//! is a feature of its own is in [`crate::voip`], and stays there.
//!
//! What is decided *here* is that the module around it is not gated: `RandTxIds` and the packet
//! demux have always been reachable at `whatsapp_rust::voip::transport::*`, neither needs a
//! socket, and gating the module for their sake would break every codec-only consumer that imports
//! one -- a build with `voip-mlow` and no relay is exactly the build a browser makes.

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
