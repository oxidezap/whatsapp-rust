//! The VoIP relay-transport seam: a dumb packet pipe to one WhatsApp relay endpoint, mirroring
//! `wacore::net::Transport` for the media plane. The relay carries STUN/RTP/RTCP as opaque binary
//! messages; this trait knows nothing about that framing. Reads are push-based (an
//! `async_channel::Receiver` of events), exactly like the main connection's transport.
//!
//! The platform implements it: native wraps the webrtc-rs DataChannel, the WASM bridge wraps a
//! Node `dgram` socket (JS owns the socket; drop on overflow since VoIP is loss tolerant), and an
//! embedded consumer could wrap a UDP `Conn`. The sans-IO `CallEngine` never touches this trait; the
//! shell pumps `PacketReceived` events into `handle_input` and runs `Output::Transmit` via `send`.

use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;

/// Why a relay media channel ended. The relay is a datagram pipe with no close handshake, so the
/// set is smaller than the WebSocket `wacore::net::DisconnectReason`.
#[derive(Debug, Clone)]
pub enum RelayDisconnectReason {
    /// The channel was closed cleanly (local disconnect, or the peer/relay closed it).
    Closed,
    /// A transport-level read/IO error ended the channel.
    ReadError(String),
    /// The reason was not reported by this transport.
    Unknown,
}

impl std::fmt::Display for RelayDisconnectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Closed => write!(f, "channel closed"),
            Self::ReadError(e) => write!(f, "read error: {e}"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// An event pushed from a relay media channel. Mirrors `wacore::net::TransportEvent` for the VoIP
/// media plane.
#[derive(Debug, Clone)]
pub enum RelayTransportEvent {
    /// The channel is open and ready to carry packets.
    Connected,
    /// One packet (STUN/RTP/RTCP) arrived from the relay.
    PacketReceived(Bytes),
    /// The channel was lost, with the reason if one was reported.
    Disconnected(RelayDisconnectReason),
}

/// A dumb packet pipe to one WhatsApp relay endpoint. Like `wacore::net::Transport` it has no
/// knowledge of STUN/RTP framing; it ships and receives opaque datagrams. VoIP is loss tolerant, so
/// an implementation MAY drop a packet under backpressure rather than block or error.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait RelayTransport: crate::sync_marker::MaybeSendSync {
    /// Send one packet to the relay.
    async fn send(&self, data: Bytes) -> Result<()>;

    /// Close the channel.
    async fn disconnect(&self);
}

/// Creates a [`RelayTransport`] connected to a relay endpoint, returning it alongside a push stream
/// of inbound packets. Mirrors `wacore::net::TransportFactory`.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait RelayTransportFactory: crate::sync_marker::MaybeSendSync {
    /// Connect to the relay and return the channel plus its event stream.
    async fn connect(
        &self,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )>;
}
