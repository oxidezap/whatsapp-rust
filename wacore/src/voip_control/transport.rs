//! The VoIP relay-transport seam: a dumb packet pipe to one WhatsApp relay endpoint, mirroring
//! `wacore::net::Transport` for the media plane. The relay carries STUN/RTP/RTCP as opaque binary
//! messages; this trait knows nothing about that framing. Reads are push-based (an
//! `async_channel::Receiver` of events), exactly like the main connection's transport.
//!
//! The platform implements it: native wraps the webrtc-rs DataChannel, the WASM bridge wraps a
//! Node `dgram` socket (JS owns the socket; drop on overflow since VoIP is loss tolerant), and an
//! embedded consumer could wrap a UDP `Conn`. The sans-IO `CallEngine` never touches this trait; the
//! shell pumps `PacketReceived` events into `handle_input` and runs `Output::Transmit` via `send`.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Result, bail};
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
///
/// `#[non_exhaustive]`: this is the platform transport seam, and it has now grown a variant once.
/// An out-of-tree implementation matching on it exhaustively would break every time the media plane
/// learns to report something new, which is a poor trade for a stream a consumer only forwards.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RelayTransportEvent {
    /// The channel is open and ready to carry packets.
    Connected,
    /// One packet (STUN/RTP/RTCP) arrived from the relay.
    PacketReceived(Bytes),
    /// Inbound media the transport discarded under backpressure before the call ever saw it.
    ///
    /// A silent drop here is indistinguishable from a peer who stopped sending, which is the class
    /// of ambiguity that kept issue #1105 open. Carrying the count lets the engine fold it into
    /// [`crate::voip::CallMediaStats`] instead of losing it at the crate boundary.
    InboundDropped(u32),
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

    /// Replace this channel with one connected to a newly selected relay endpoint.
    ///
    /// Platforms that cannot redial return an error, causing the driver to end the call instead of
    /// silently sending allocation traffic to the retired relay.
    ///
    /// # What an address does not carry, and what that costs
    ///
    /// This seam carries the new endpoint and nothing else, and a group call can rotate the
    /// credentials that [`RelayEndpointParams`] calls ICE ones. A `<relay>` in a group update
    /// may carry a new `<key>` (the `ice_pwd`) and new `tokens`/`auth_tokens` (the `ice_ufrag`),
    /// and `group_relay_credential_refresh_rearms_allocation_on_the_same_endpoint` in
    /// `wacore::voip::engine` is a test that rotates all three.
    ///
    /// The engine answers such an update by rebuilding its STUN allocate with the new key and
    /// token, which is the whole of what a *native* transport needs: it ships opaque datagrams,
    /// so the credentials only ever appear inside the payloads the engine builds. A transport
    /// that instead bakes them into the channel itself -- a browser's, whose synthetic SDP answer
    /// names `ice-ufrag` and `ice-pwd` -- is not so served, and there are two distinct gaps:
    ///
    /// - **Credentials rotate, endpoint unchanged.** No `Output::ReconnectRelay` is emitted at
    ///   all (that same test asserts it: "unchanged relay coordinates must not reconnect the
    ///   socket"), so this method is never called and such a transport is never told.
    /// - **Endpoint and credentials both change.** This method is called with the address alone,
    ///   so an implementation can only redial carrying whatever it was constructed with.
    ///
    /// So an implementation should reuse its existing credentials against the new address rather
    /// than refusing -- refusing ends a call over a routine migration -- while knowing they may
    /// be the retired relay's. Closing either gap means the engine saying more than a
    /// `SocketAddr`: a whole [`RelayEndpointParams`] on this output, and a second output for the
    /// refresh that emits none. Both are changes to `Output`'s contract, which is why this is
    /// written down rather than guessed at here.
    async fn reconnect(
        &self,
        endpoint: SocketAddr,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )> {
        bail!("relay transport does not support reconnecting to {endpoint}")
    }
}

/// Everything a platform needs to reach one relay endpoint.
///
/// The address alone is enough for a stack that dials UDP and handshakes DTLS itself, which is what
/// the native transport does. It is not enough for a browser: there the media channel is an
/// `RTCPeerConnection`, ICE is not optional, and a synthetic SDP answer describing the relay has to
/// carry credentials the relay will actually validate -- so the two ICE fields travel with the
/// address rather than being looked up from a `RelayData` no transport is handed.
///
/// Both are derived rather than invented, and *which* credential is which matters: the relay block
/// carries two separately indexed token sets and swapping them is a call that will not connect.
///
/// - `ice_ufrag` is [`token_to_ice_ufrag`](crate::voip::relay_parse::token_to_ice_ufrag) of the
///   selected endpoint's `<auth_token>`, indexed by its `auth_token_id`.
/// - `ice_pwd` is the relay `<key>` in the ASCII base64 form it arrived in -- the same bytes the
///   engine keys STUN MESSAGE-INTEGRITY with.
///
/// The `<token>` beside them, indexed by `token_id`, is the STUN *allocation* credential and goes
/// on the wire as `RELAY-TOKEN`. It is never a ufrag. A ufrag built from it is refused at the
/// browser's first connectivity check, which surfaces as a call that will not connect rather than
/// as anything resembling a bad credential -- so it is worth naming here rather than leaving to be
/// inferred from a neighbouring field name.
#[derive(Clone)]
pub struct RelayEndpointParams {
    /// Where the relay is.
    pub addr: SocketAddr,
    /// `a=ice-ufrag` for a synthetic SDP answer.
    pub ice_ufrag: String,
    /// `a=ice-pwd` for a synthetic SDP answer. Live credential material.
    pub ice_pwd: String,
}

// Manual Debug: `ice_pwd` is the relay key, and this struct is the kind of thing a `{:?}` in a
// transport implementation reaches for. Matches the redaction `RelayData` and `CallConfig` apply.
impl core::fmt::Debug for RelayEndpointParams {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RelayEndpointParams")
            .field("addr", &self.addr)
            .field("ice_ufrag", &self.ice_ufrag)
            .field("ice_pwd", &"[redacted]")
            .finish()
    }
}

/// Chooses which [`RelayTransportFactory`] dials a given relay endpoint.
///
/// A factory is bound to one address, and the address is not known until the server names the
/// relay for a call -- so what a platform actually supplies is this: a way to make a factory once
/// the address arrives. Native builds default to the UDP/DTLS/SCTP dialer behind
/// `voip-relay-native`; a browser has no UDP socket and hands in an `RTCPeerConnection` instead,
/// which reaches the same relay over the same pre-negotiated DataChannel the native stack builds
/// by hand.
///
/// It is a trait rather than a boxed closure because an implementation carries state -- a page's
/// carries the JS handles its peer connections are built from -- and a closure returning an `Arc`
/// would have to own that anyway.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait RelayTransportProvider: crate::sync_marker::MaybeSendSync {
    /// The factory that dials `relay`.
    ///
    /// Async because a platform may have to ask for something before it can build one -- a browser
    /// creating an `RTCPeerConnection` is a call into JS. Failing here fails the call with the
    /// reason, which is the honest answer for a page whose browser has no WebRTC at all.
    async fn factory(&self, relay: &RelayEndpointParams) -> Result<Arc<dyn RelayTransportFactory>>;
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
