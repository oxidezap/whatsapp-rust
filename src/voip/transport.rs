//! Relay media transport: a pre-negotiated WebRTC DataChannel over SCTP-over-DTLS-over-UDP
//! to a single WhatsApp relay endpoint. The synthetic-SDP / wrtc dance reduces, at this layer,
//! to: connect a UDP socket to the relay, DTLS-handshake as the client (self-signed cert,
//! server-cert verification skipped, since SRTP keys come from callKey, not DTLS),
//! run an SCTP association over it, and open the pre-negotiated id=0 DataChannel that carries
//! STUN/RTP/RTCP as binary messages.
//!
//! The protocol stack itself is sans-IO. `RelayStack` owns no socket and reads no clock: it is
//! handed datagrams plus an `Instant` and polled for what it wants on the wire and for the
//! retransmission deadlines it needs waking at. One task, `relay_driver`, is the only place a
//! socket, a timer or a spawn appears, which puts the media plane on the same seam the portable
//! `CallEngine` already sits on.

use std::collections::VecDeque;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use log::debug;
use tokio::net::UdpSocket;

use rtc_datachannel::data_channel::DataChannel;
use rtc_datachannel::message::message_channel_open::ChannelType;
use rtc_dtls::config::ConfigBuilder as DtlsConfigBuilder;
use rtc_dtls::crypto::Certificate;
use rtc_dtls::endpoint::{Endpoint as DtlsEndpoint, EndpointEvent as DtlsEndpointEvent};
use rtc_shared::TransportProtocol;

use rtc_sctp::{
    Association, AssociationHandle, ClientConfig, DatagramEvent, Endpoint as SctpEndpoint,
    EndpointConfig, Event as SctpEvent, Payload, PayloadProtocolIdentifier, StreamEvent, StreamId,
    TransportConfig,
};

use wacore::runtime::{AbortHandle, Runtime};
use wacore::voip::engine::TxIdSource;
use wacore::voip::relay_parse::WEB_CLIENT_RELAY_PORT;
use wacore::voip::transport::{
    RelayDisconnectReason, RelayTransport, RelayTransportEvent, RelayTransportFactory,
};

/// DataChannel label WA Web uses (pre-negotiated, id=0).
const DATA_CHANNEL_LABEL: &str = "pre-negotiated";
/// SCTP-over-DTLS WebRTC port.
const SCTP_PORT: u16 = 5000;
/// The pre-negotiated DataChannel's stream id. Both ends open it directly, because a pre-negotiated
/// channel carries no DCEP handshake -- which is why WA Web uses one.
const MEDIA_STREAM_ID: StreamId = 0;

/// The shape WA Web opens the media channel with (`ordered=false, maxRetransmits=0`), and the shape
/// this transport has to keep. Real-time RTP must NOT ride a reliable, ordered SCTP stream: a single
/// reordered or lost packet head-of-line-blocks every later one (the receiver holds them until the
/// gap is retransmitted, then delivers a burst), which the peer hears as choppy or garbled audio,
/// worst on links that reorder at all. Reliability is per-sender, which is why an ordered outbound
/// stream sounded choppy to the peer while inbound stayed clean.
const MEDIA_CHANNEL_TYPE: ChannelType = ChannelType::PartialReliableRexmitUnordered;
/// The retransmit count [`MEDIA_CHANNEL_TYPE`] carries: never retransmit, abandon instead.
const MEDIA_MAX_RETRANSMITS: u32 = 0;

// First-byte relay-packet demux moved to the portable core; re-exported so the existing
// `whatsapp_rust::voip::transport::{classify_relay_packet, RelayPacketKind}` paths stay stable.
pub use wacore::voip::demux::{RelayPacketKind, classify_relay_packet};

/// Why the media stack stopped. The driver branches on the variant: a peer close ends the call the
/// way hanging up does, anything else is a transport failure the call surfaces as a read error.
#[derive(Debug, thiserror::Error)]
enum StackError {
    /// The relay sent `close_notify` or a fatal alert, or tore its association down.
    #[error("relay closed the media channel: {0}")]
    PeerClosed(String),
    /// The stack cannot continue: a handshake that failed or ran out of retransmissions, a socket
    /// error, or a protocol layer that refused what we asked of it.
    #[error("{0}")]
    Failed(String),
}

impl From<StackError> for RelayDisconnectReason {
    fn from(e: StackError) -> Self {
        match e {
            StackError::PeerClosed(_) => Self::Closed,
            StackError::Failed(msg) => Self::ReadError(msg),
        }
    }
}

/// The sans-IO relay media stack: DTLS over the relay's UDP flow, an SCTP association inside it,
/// and the pre-negotiated id=0 stream that carries STUN/RTP/RTCP.
///
/// Every entry point takes the current `Instant` and every output is polled, so this type never
/// touches a socket, a clock or a task. That is what makes the retransmission timers of both DTLS
/// and SCTP the driver's business rather than a library thread's, and what would let the whole
/// stack move to the runtime-agnostic core later.
struct RelayStack {
    remote: SocketAddr,
    dtls: DtlsEndpoint,
    sctp: SctpEndpoint,
    /// Set on the dialling end. The client drives both handshakes -- DTLS first, then the SCTP
    /// association inside it; the accepting end has both handed to it.
    initiator: bool,
    /// `None` until the association exists. SCTP only ever runs inside the secure channel.
    assoc: Option<(AssociationHandle, Association)>,
    /// Set once the media stream is open, which is what `Connected` means to the caller.
    open: bool,
    /// Reassembled inbound messages, drained by the driver into relay events.
    inbound: VecDeque<Bytes>,
    /// Set once the stack has stopped, so later input is ignored rather than re-reported.
    stopped: bool,
}

impl RelayStack {
    /// Build the dialling half and queue the DTLS ClientHello. Nothing reaches the wire until the
    /// driver polls [`Self::poll_transmit`].
    fn connect(local: SocketAddr, remote: SocketAddr, now: Instant) -> Result<Self> {
        let config = dtls_config(true, Some(remote))?;
        let mut dtls = DtlsEndpoint::new(local, TransportProtocol::UDP, None);
        dtls.connect(now, remote, Arc::new(config), None)
            .map_err(|e| anyhow!("dtls connect: {e}"))?;
        // The dialling end initiates the association too, so its SCTP endpoint needs no server
        // config: nothing will ever arrive on it that this side did not ask for.
        let sctp = SctpEndpoint::new(
            local,
            TransportProtocol::UDP,
            Arc::new(EndpointConfig::new()),
            None,
        );
        Ok(Self::new(remote, dtls, sctp, true))
    }

    /// The accepting half, which the loopback relay in this module's tests runs. Only the roles
    /// differ: from the first datagram on it is the same state machine, driven by the same loop.
    #[cfg(test)]
    fn accept(local: SocketAddr, remote: SocketAddr) -> Result<Self> {
        let config = dtls_config(false, None)?;
        let dtls = DtlsEndpoint::new(local, TransportProtocol::UDP, Some(Arc::new(config)));
        let sctp = SctpEndpoint::new(
            local,
            TransportProtocol::UDP,
            Arc::new(EndpointConfig::new()),
            Some(Arc::new(rtc_sctp::ServerConfig::new(
                sctp_transport_config(),
            ))),
        );
        Ok(Self::new(remote, dtls, sctp, false))
    }

    fn new(remote: SocketAddr, dtls: DtlsEndpoint, sctp: SctpEndpoint, initiator: bool) -> Self {
        Self {
            remote,
            dtls,
            sctp,
            initiator,
            assoc: None,
            open: false,
            inbound: VecDeque::new(),
            stopped: false,
        }
    }

    /// Whether the media channel is open for traffic.
    fn is_open(&self) -> bool {
        self.open
    }

    /// Feed one datagram off the relay socket.
    fn handle_datagram(&mut self, now: Instant, data: &[u8]) -> Result<(), StackError> {
        if self.stopped {
            return Ok(());
        }
        let events = match self.dtls.read(now, self.remote, None, BytesMut::from(data)) {
            Ok(events) => events,
            // A close_notify or fatal alert ends the call. Anything else is one unusable datagram:
            // the socket is connected to the relay, so a record that fails to parse or authenticate
            // means corruption on the path, and dropping it is what a datagram protocol is supposed
            // to do. The exception is the handshake, where the only datagrams that arrive are the
            // ones it needs, so a failure there is the handshake failing and there is nothing to
            // wait for.
            Err(rtc_shared::error::Error::ErrAlertFatalOrClose) => {
                self.stopped = true;
                return Err(StackError::PeerClosed("alert".to_owned()));
            }
            Err(e) if self.assoc.is_none() => {
                self.stopped = true;
                return Err(StackError::Failed(format!("dtls handshake: {e}")));
            }
            Err(e) => {
                debug!("voip: dropping unusable relay datagram: {e}");
                return Ok(());
            }
        };

        for event in events {
            match event {
                DtlsEndpointEvent::HandshakeComplete if self.initiator => {
                    self.start_association(now)?;
                }
                DtlsEndpointEvent::ApplicationData(sctp_packet) => {
                    self.feed_association(now, sctp_packet.freeze());
                }
                // The event set is non-exhaustive by design, and a variant this transport has never
                // heard of carries nothing it could act on.
                _ => {}
            }
        }
        self.pump(now)
    }

    /// Queue one media message. Fails only if the channel is not (or no longer) open.
    fn write_media(&mut self, now: Instant, data: &Bytes) -> Result<(), StackError> {
        if !self.open {
            return Err(StackError::Failed("media channel is not open".to_owned()));
        }
        let Some((_, assoc)) = self.assoc.as_mut() else {
            return Err(StackError::Failed("no sctp association".to_owned()));
        };
        // SCTP cannot carry a zero-length message, so an empty one travels under its own payload
        // protocol id. The engine never sends one, but the rule belongs with the framing.
        let ppi = if data.is_empty() {
            PayloadProtocolIdentifier::BinaryEmpty
        } else {
            PayloadProtocolIdentifier::Binary
        };
        assoc
            .stream(MEDIA_STREAM_ID)
            .and_then(|mut s| s.write_chunk_with_ppi(data, ppi))
            .map_err(|e| StackError::Failed(format!("media write: {e}")))?;
        self.pump(now)
    }

    /// Start the SCTP association. The dialling end does this once, when DTLS reports complete.
    fn start_association(&mut self, now: Instant) -> Result<(), StackError> {
        let (handle, assoc) = self
            .sctp
            .connect(now, ClientConfig::new(sctp_transport_config()), self.remote)
            .map_err(|e| StackError::Failed(format!("sctp connect: {e}")))?;
        self.assoc = Some((handle, assoc));
        Ok(())
    }

    /// Route one decrypted SCTP packet into the association, creating it on the accepting end.
    fn feed_association(&mut self, now: Instant, packet: Bytes) {
        let Self {
            sctp,
            assoc,
            remote,
            ..
        } = self;
        match sctp.handle(now, *remote, None, packet) {
            Some((ch, DatagramEvent::AssociationEvent(event))) => {
                // Application data before the association exists cannot be ours; nothing else
                // shares this DTLS channel.
                if let Some((handle, association)) = assoc.as_mut()
                    && ch == *handle
                {
                    association.handle_event(event);
                }
            }
            // Only an endpoint carrying a server config produces this, so on the dialling end the
            // arm never fires; it is what makes the accepting end the same type.
            Some((ch, DatagramEvent::NewAssociation(association))) if assoc.is_none() => {
                *assoc = Some((ch, association));
            }
            _ => {}
        }
    }

    /// Drain everything the association produced since the last call: state changes, endpoint
    /// bookkeeping, inbound messages, and the packets it wants encrypted and sent.
    fn pump(&mut self, now: Instant) -> Result<(), StackError> {
        let Self {
            remote,
            dtls,
            sctp,
            assoc,
            open,
            inbound,
            stopped,
            ..
        } = self;
        let Some((handle, association)) = assoc.as_mut() else {
            return Ok(());
        };

        let mut connected = false;
        let mut readable = false;
        let mut ended = None;
        while let Some(event) = association.poll() {
            match event {
                SctpEvent::Connected => connected = true,
                SctpEvent::Stream(StreamEvent::Readable { id }) if id == MEDIA_STREAM_ID => {
                    readable = true;
                }
                // The association going away after the channel opened is the relay hanging up; a
                // handshake that never finished is a failure to connect.
                SctpEvent::AssociationLost { reason, .. } => {
                    ended = Some(StackError::PeerClosed(reason.to_string()));
                }
                SctpEvent::HandshakeFailed { reason } => {
                    ended = Some(StackError::Failed(format!("sctp handshake: {reason}")));
                }
                _ => {}
            }
        }

        if connected {
            open_media_stream(association)?;
            *open = true;
        }
        if readable && *open {
            drain_media_stream(association, inbound);
        }

        while let Some(event) = association.poll_endpoint_event() {
            sctp.handle_event(*handle, event);
        }
        // Every SCTP packet becomes one DTLS application-data record, and the driver turns each of
        // those into one datagram.
        while let Some(transmit) = association.poll_transmit(now) {
            if let Payload::RawEncode(packets) = transmit.message {
                for packet in packets {
                    dtls.write(now, *remote, &packet)
                        .map_err(|e| StackError::Failed(format!("dtls write: {e}")))?;
                }
            }
        }

        if let Some(e) = ended {
            *stopped = true;
            *open = false;
            return Err(e);
        }
        Ok(())
    }

    /// The next datagram to put on the wire, if any.
    fn poll_transmit(&mut self) -> Option<BytesMut> {
        self.dtls.poll_transmit().map(|t| t.message)
    }

    /// The next reassembled inbound message, if any.
    fn poll_inbound(&mut self) -> Option<Bytes> {
        self.inbound.pop_front()
    }

    /// When [`Self::handle_timeout`] next needs calling. DTLS handshake retransmission and SCTP's
    /// retransmission, ack and heartbeat timers both live here, so the driver arms one timer for
    /// the pair instead of racing two.
    fn poll_timeout(&self) -> Option<Instant> {
        let dtls = self.dtls.poll_timeout(&self.remote);
        let sctp = self.assoc.as_ref().and_then(|(_, a)| a.poll_timeout());
        match (dtls, sctp) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, None) => a,
            (None, b) => b,
        }
    }

    /// Advance both state machines to `now`, firing whichever timers are due.
    fn handle_timeout(&mut self, now: Instant) -> Result<(), StackError> {
        if self.stopped {
            return Ok(());
        }
        // A DTLS timeout error means the handshake ran out of retransmissions, which nothing later
        // recovers from.
        if let Err(e) = self.dtls.handle_timeout(self.remote, now) {
            self.stopped = true;
            return Err(StackError::Failed(format!("dtls handshake timeout: {e}")));
        }
        if let Some((_, assoc)) = self.assoc.as_mut() {
            assoc.handle_timeout(now);
        }
        self.pump(now)
    }

    /// Reset the media stream the way a peer closing its DataChannel does, leaving the association
    /// itself up. Test-only, because production hangs up by tearing the whole channel down -- but
    /// the relay does exactly this at the end of a call, which is what the old stack saw as a read
    /// of `Ok(0)`.
    #[cfg(test)]
    fn close_media_stream(&mut self, now: Instant) -> Result<(), StackError> {
        if let Some((_, assoc)) = self.assoc.as_mut() {
            assoc
                .stream(MEDIA_STREAM_ID)
                .and_then(|mut s| s.close())
                .map_err(|e| StackError::Failed(format!("close media stream: {e}")))?;
        }
        self.pump(now)
    }

    /// Tear the channel down politely: SHUTDOWN the association, then `close_notify` the DTLS
    /// channel. One flush, without waiting for the relay's SHUTDOWN-ACK -- the relay drops the flow
    /// on its own consent-freshness timer, and a teardown that waits on a peer that may already be
    /// gone is a teardown that hangs.
    fn close(&mut self, now: Instant) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.open = false;
        if let Some((_, assoc)) = self.assoc.as_mut() {
            let _ = assoc.shutdown();
        }
        let _ = self.pump(now);
        let _ = self.dtls.close(now);
    }
}

/// The clock the media stack runs on.
///
/// `std::time::Instant` rather than the portable `wacore::time::Instant`, because that is the type
/// the protocol crates take and hand back for their retransmission deadlines, and the type
/// `tokio::time::sleep_until` needs to arm a timer on one. Native-only either way: this module
/// already refuses to build where a Tokio UDP socket does not exist.
#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

/// The DTLS configuration both ends of the relay flow use.
///
/// The relay does not validate our client certificate, and we do not validate its: the SDP
/// fingerprint is fixed and cosmetic at this layer, and media authentication is hop-by-hop SRTP
/// keyed from callKey, not from this handshake.
fn dtls_config(
    is_client: bool,
    remote: Option<SocketAddr>,
) -> Result<rtc_dtls::config::HandshakeConfig> {
    let provider = rtc_dtls::crypto_provider::default_provider()
        .map_err(|e| anyhow!("dtls crypto provider: {e}"))?;
    let cert = Certificate::generate_self_signed(vec!["wa-voip".to_owned()], provider.crypto())
        .map_err(|e| anyhow!("dtls self-signed cert: {e}"))?;
    DtlsConfigBuilder::default()
        .with_crypto_provider(provider)
        .with_certificates(vec![cert])
        .with_insecure_skip_verify(true)
        // Only the SNI/cert-hostname, irrelevant under insecure_skip_verify; set to silence the
        // library's empty-server-name warn.
        .with_server_name("localhost".to_owned())
        .build(is_client, remote)
        .map_err(|e| anyhow!("dtls config: {e}"))
}

/// The SCTP parameters of a relay association. Everything else is the crate's default, which is
/// already what the relay expects (MTU 1191, a 1 MiB advertised receive window).
fn sctp_transport_config() -> TransportConfig {
    TransportConfig::default()
        .with_sctp_port(SCTP_PORT)
        .with_max_message_size(RELAY_SCTP_MAX_MESSAGE)
}

/// Open the pre-negotiated id=0 media stream with the reliability [`MEDIA_CHANNEL_TYPE`] implies.
///
/// The reliability is applied here, to the SCTP stream, rather than by the DataChannel layer: for a
/// `negotiated` channel that layer only emits a `DATA_CHANNEL_OPEN` flagged as one the transport
/// must apply locally and must not send to the peer, and this driver is that transport. What the
/// library owns is the mapping from the W3C channel type to SCTP's ordered flag and reliability
/// type, which is where the RFC 8832 rule belongs -- so the pair cannot drift from the channel type
/// the way two hand-written arguments could.
fn open_media_stream(assoc: &mut Association) -> Result<(), StackError> {
    let (unordered, reliability) = DataChannel::get_reliability_params(MEDIA_CHANNEL_TYPE);
    let mut stream = assoc
        .open_stream(MEDIA_STREAM_ID, PayloadProtocolIdentifier::Binary)
        .map_err(|e| StackError::Failed(format!("open sctp media stream: {e}")))?;
    stream
        .set_reliability_params(unordered, reliability, MEDIA_MAX_RETRANSMITS)
        .map_err(|e| StackError::Failed(format!("media reliability: {e}")))?;
    debug!("voip: media channel '{DATA_CHANNEL_LABEL}' open on stream {MEDIA_STREAM_ID}");
    Ok(())
}

/// Move every reassembled message off the media stream into `inbound`.
fn drain_media_stream(assoc: &mut Association, inbound: &mut VecDeque<Bytes>) {
    let Ok(mut stream) = assoc.stream(MEDIA_STREAM_ID) else {
        return;
    };
    while let Ok(Some(chunks)) = stream.read() {
        // A pre-negotiated channel exchanges no DCEP, so anything but user data on this stream is
        // not addressed to us.
        if !matches!(
            chunks.ppi,
            PayloadProtocolIdentifier::Binary | PayloadProtocolIdentifier::String
        ) {
            continue;
        }
        // Cannot exceed the cap: it is the one the association reassembled under.
        if let Ok(payload) = chunks.to_payload(RELAY_SCTP_MAX_MESSAGE as usize) {
            inbound.push_back(payload.freeze());
        }
    }
}

/// An open relay media channel: STUN/RTP/RTCP travel as binary DataChannel messages.
///
/// A handle onto `relay_driver`, which owns the socket and the whole protocol stack; everything
/// here is a message to that task.
pub struct RelayMediaChannel {
    /// Outbound media queue. Closing it is how [`RelayTransport::disconnect`] asks the driver to
    /// tear the channel down gracefully: the driver reads a closed queue as a hang-up.
    outbound: async_channel::Sender<Bytes>,
    /// Resolves (with an error, since nothing is ever sent on it) when the driver task exits, so
    /// `disconnect` can wait out the polite close it just asked for.
    finished: async_channel::Receiver<()>,
    /// Aborts the driver when this channel is dropped. A dropped handle means the call is gone, and
    /// the driver owns the relay socket and both state machines, so aborting it releases all of
    /// them -- there are no library background loops left to close separately. `AbortHandle` aborts
    /// on its own `Drop`, so this type needs no `Drop` of its own; what an abort skips is the
    /// goodbye on the wire, which is why `disconnect` exists and why the call loop always runs it.
    driver: std::sync::Mutex<Option<AbortHandle>>,
    /// Kept so `reconnect` can dial the next relay on the runtime the call runs on.
    runtime: Arc<dyn Runtime>,
}

impl RelayMediaChannel {
    /// Lock the driver-handle slot, recovering the guard on a poisoned mutex rather than panicking.
    #[inline]
    fn lock_driver(&self) -> std::sync::MutexGuard<'_, Option<AbortHandle>> {
        self.driver.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Connect the full media stack to one relay endpoint.
///
/// Returns once the driver reports the media channel open, so a caller holding the `Ok` can send on
/// it. The multi-step setup is kept on `anyhow` for per-step `.context()`; the factory wraps the
/// aggregate, since a connect failure is fatal for the call.
pub async fn connect_relay_media(
    relay_addr: SocketAddr,
    runtime: Arc<dyn Runtime>,
) -> Result<(
    RelayMediaChannel,
    async_channel::Receiver<RelayTransportEvent>,
)> {
    // Bind in the relay's address family so an IPv6 relay is reachable (WA relays are IPv4 today,
    // but the unspecified bind must match to connect).
    let bind_addr = if relay_addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let socket = UdpSocket::bind(bind_addr).await.context("bind udp")?;
    socket
        .connect(relay_addr)
        .await
        .context("connect udp to relay")?;
    let local_addr = socket.local_addr().context("local udp addr")?;
    let stack = RelayStack::connect(local_addr, relay_addr, now())?;

    let (outbound_tx, outbound_rx) = async_channel::bounded(RELAY_SEND_CAP);
    let (events_tx, events_rx) = async_channel::bounded(RELAY_EVENT_CAP);
    let (finished_tx, finished_rx) = async_channel::bounded(1);
    let (open_tx, open_rx) = async_channel::bounded(1);

    let driver = runtime.spawn(Box::pin(relay_driver(
        socket,
        stack,
        outbound_rx,
        events_tx,
        finished_tx,
        open_tx,
    )));
    let channel = RelayMediaChannel {
        outbound: outbound_tx,
        finished: finished_rx,
        driver: std::sync::Mutex::new(Some(driver)),
        runtime,
    };

    // The driver reports the open channel here and, separately, as the `Connected` event the call
    // loop expects to read off the stream, so waiting here does not consume that event.
    match open_rx.recv().await {
        Ok(Ok(())) => Ok((channel, events_rx)),
        Ok(Err(e)) => Err(anyhow!("{e}")),
        Err(_) => Err(anyhow!(
            "relay media driver stopped before the channel opened"
        )),
    }
}

/// OS-RNG-backed STUN transaction ids for production calls. The core's `SequentialTxIds` is
/// deterministic (test-only); real calls need unpredictable ids for consent freshness.
#[derive(Default)]
pub struct RandTxIds;

impl TxIdSource for RandTxIds {
    fn next_tx_id(&mut self) -> [u8; 12] {
        rand::random()
    }
}

#[async_trait]
impl RelayTransport for RelayMediaChannel {
    async fn send(&self, data: Bytes) -> Result<()> {
        match self.outbound.try_send(data) {
            Ok(()) => Ok(()),
            // The same trade the inbound side makes, in the other direction: a full queue means the
            // driver is not draining, and media is what a call can afford to lose. STUN is not --
            // see `deliver` -- so it waits for a slot instead.
            Err(async_channel::TrySendError::Full(data)) => {
                if classify_relay_packet(&data) == RelayPacketKind::Stun {
                    self.outbound
                        .send(data)
                        .await
                        .map_err(|_| anyhow!("relay media channel closed"))?;
                }
                Ok(())
            }
            Err(async_channel::TrySendError::Closed(_)) => bail!("relay media channel closed"),
        }
    }

    async fn disconnect(&self) {
        // Closing the queue is the hang-up: the driver drains what is already queued, then sends
        // SCTP SHUTDOWN and DTLS close_notify. Wait for it to finish, bounded, so a relay that
        // stopped answering cannot hold teardown open, then abort whatever is left.
        self.outbound.close();
        let _ = wacore::runtime::timeout(
            &*self.runtime,
            RELAY_DISCONNECT_TIMEOUT,
            self.finished.recv(),
        )
        .await;
        if let Some(driver) = self.lock_driver().take() {
            driver.abort();
        }
    }

    async fn reconnect(
        &self,
        endpoint: SocketAddr,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )> {
        RelayMediaChannelFactory::new(endpoint, self.runtime.clone())
            .connect()
            .await
    }
}

/// Inbound event-channel depth. VoIP is loss tolerant, so the driver drops packets rather than block
/// when the call falls behind.
const RELAY_EVENT_CAP: usize = 256;
/// Outbound queue depth. Same depth and same rule as inbound: stalling the engine's 20ms tick loop
/// behind a wedged socket costs a call more than losing a media packet does.
const RELAY_SEND_CAP: usize = 256;
/// Receive buffer for the relay socket. A DTLS record carrying one SCTP packet stays inside the path
/// MTU, so this only has to be comfortably larger than one: a truncated datagram would fail its DTLS
/// MAC and be dropped, which is a silent way to lose a call.
const RELAY_DATAGRAM_BUF: usize = 8192;
/// The largest message the association will reassemble, and so the most a relay can make one call
/// hold at once. The number the previous stack ran with, kept because it is a local receive limit
/// that never appears on the wire.
const RELAY_SCTP_MAX_MESSAGE: u32 = 65536;
/// Upper bound on the relay handshake (UDP+DTLS+SCTP+media stream); a black-holed or wedged endpoint
/// fails here instead of parking the caller forever. It still bounds the whole sequence rather than
/// any one step: the driver reports the channel open only once every layer is up, so one deadline
/// covers what is now several round trips.
const RELAY_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);
/// How long one inbound STUN packet may hold the driver while the call is behind. Well under both
/// retransmission timers this same task drives (DTLS re-flights at 1s, SCTP's RTO floor is 1s), so a
/// consumer that stalls loses a STUN packet rather than the association.
const RELAY_STUN_HOLD: std::time::Duration = std::time::Duration::from_millis(100);
/// How long `disconnect` waits for the driver's polite close before abandoning it.
const RELAY_DISCONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// A [`RelayTransportFactory`] that dials one relay endpoint (UDP+DTLS+SCTP+DataChannel) and pumps
/// inbound DataChannel messages out as [`RelayTransportEvent`]s, mirroring how the main-connection
/// `TransportFactory` returns `(Arc<dyn Transport>, Receiver<TransportEvent>)`.
pub struct RelayMediaChannelFactory {
    addr: SocketAddr,
    runtime: Arc<dyn Runtime>,
}

impl RelayMediaChannelFactory {
    pub fn new(addr: SocketAddr, runtime: Arc<dyn Runtime>) -> Self {
        Self { addr, runtime }
    }
}

/// The relay addresses to dial concurrently for `advertised`.
///
/// Which of the two media ports a relay serves is NOT derivable from the offer, and it varies per
/// relay host AND per session: a host may advertise 3478 and answer only on
/// [`WEB_CLIENT_RELAY_PORT`], or advertise and honour the same port. So either choice alone
/// black-holes for some relays, costing a full `RELAY_CONNECT_TIMEOUT` and failing the call.
///
/// The advertised port comes first so a relay that honours it is dialled on the port it asked for.
/// Never yields a duplicate, which would double-dial the same endpoint for nothing.
fn dial_candidates(advertised: SocketAddr) -> Vec<SocketAddr> {
    let mut out = vec![advertised];
    if advertised.port() != WEB_CLIENT_RELAY_PORT {
        out.push(SocketAddr::new(advertised.ip(), WEB_CLIENT_RELAY_PORT));
    }
    out
}

/// One candidate dial's result: the address that answered, its channel, and its event stream.
type DialedRelay = (
    SocketAddr,
    RelayMediaChannel,
    async_channel::Receiver<RelayTransportEvent>,
);

#[async_trait]
impl RelayTransportFactory for RelayMediaChannelFactory {
    async fn connect(
        &self,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )> {
        // Race every candidate port (see `dial_candidates`) and keep whichever completes the
        // handshake. `select_ok` discards the losers' errors and drops their futures, aborting those
        // dials. Cost when the advertised port is the right one is a second UDP socket for the
        // duration of the handshake; trying serially would instead add a full RELAY_CONNECT_TIMEOUT
        // to every call on the other relay family.
        let dialed = dial_candidates(self.addr);
        // Whichever candidate wins is the only record of whether this relay honoured the port it
        // advertised, and `select_ok` yields just the winner's output - so carry the address through
        // rather than reconstruct it. Also tags each error with its address: `select_ok` propagates
        // only the LAST error, which without this would name no endpoint at all.
        let candidates: Vec<Pin<Box<dyn Future<Output = Result<DialedRelay>> + Send>>> = dialed
            .iter()
            .map(|&addr| {
                let runtime = self.runtime.clone();
                Box::pin(async move {
                    match connect_relay_media(addr, runtime).await {
                        Ok((chan, events)) => Ok((addr, chan, events)),
                        Err(e) => Err(anyhow!("{addr}: {e}")),
                    }
                }) as Pin<Box<dyn Future<Output = _> + Send>>
            })
            .collect();
        let tried = dialed
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        // Bound the UDP+DTLS+SCTP+DataChannel handshake: a relay whose UDP is reachable but whose
        // DTLS/SCTP wedges (black-holed endpoint) would otherwise park the caller forever with no
        // failure surfaced. Matches the old connect_and_allocate's 12s timeout.
        let (winner, chan, events) = wacore::runtime::timeout(
            &*self.runtime,
            RELAY_CONNECT_TIMEOUT,
            futures::future::select_ok(candidates),
        )
        .await
        .map_err(|_| {
            anyhow!(
                "relay connect timed out after {RELAY_CONNECT_TIMEOUT:?} \
                 (DTLS/SCTP didn't complete); tried {tried}"
            )
        })?
        .map_err(|e| anyhow!("relay connect: {e}; tried {tried}"))?
        .0;
        // The advertised port and the port that answered diverge per host and per session, so this
        // is the only place that fact is observable in production.
        debug!(
            "voip: relay media connected on {winner} (advertised {}){}",
            self.addr,
            if winner == self.addr {
                ""
            } else {
                " - relay did NOT serve its advertised port"
            }
        );
        Ok((Arc::new(chan) as Arc<dyn RelayTransport>, events))
    }
}

/// The one task that owns I/O for a relay media channel: the socket, the timers, and the sans-IO
/// [`RelayStack`] they feed.
///
/// One task rather than several because every input has to reach the same state machine; splitting
/// reads, writes and timers apart would only put a lock in front of it, and the work per datagram is
/// a copy and one AES record, not something worth parallelising. It runs from the first ClientHello
/// through teardown, so the handshake needs no second code path and no second set of timers.
///
/// It ends when the relay closes, the socket errors, the outbound queue closes (a local hang-up), or
/// the call drops its event receiver.
async fn relay_driver(
    socket: UdpSocket,
    mut stack: RelayStack,
    outbound: async_channel::Receiver<Bytes>,
    events: async_channel::Sender<RelayTransportEvent>,
    _finished: async_channel::Sender<()>,
    open: async_channel::Sender<Result<(), String>>,
) {
    let mut buf = vec![0u8; RELAY_DATAGRAM_BUF];
    let mut announced = false;
    let mut hung_up = false;
    loop {
        // Flush, deliver, announce: everything the stack produced since the last wakeup, before
        // parking on the next one.
        while let Some(datagram) = stack.poll_transmit() {
            if let Err(e) = socket.send(&datagram).await {
                let reason = RelayDisconnectReason::ReadError(format!("relay socket send: {e}"));
                finish(&events, &open, announced, reason).await;
                return;
            }
        }
        while let Some(packet) = stack.poll_inbound() {
            if !deliver(&events, packet).await {
                return; // the call dropped its receiver
            }
        }
        if !announced && stack.is_open() {
            announced = true;
            let _ = open.send(Ok(())).await;
            // The call loop reads `Connected` off the event stream; `open` only unblocks the caller
            // of `connect_relay_media`.
            let _ = events.try_send(RelayTransportEvent::Connected);
        }
        if hung_up {
            // `try_send`, unlike the terminal send in `finish`, because this close was asked for
            // locally: the call already knows, and by the time `disconnect` runs it has left its
            // receive loop, so nothing is draining this channel. Awaiting a slot that only the
            // departed reader could free would hang the driver until `disconnect` gave up on it --
            // a two-second teardown stall, and only when the call was already behind enough to fill
            // the queue.
            let _ = events.try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ));
            return;
        }

        let deadline = stack.poll_timeout();
        let step = tokio::select! {
            read = socket.recv(&mut buf) => match read {
                Ok(n) => stack.handle_datagram(now(), &buf[..n]),
                // A connected UDP socket surfaces ICMP unreachable here, which is how a relay that
                // went away is noticed rather than waited out.
                Err(e) => Err(StackError::Failed(format!("relay socket recv: {e}"))),
            },
            queued = outbound.recv() => match queued {
                Ok(packet) => stack.write_media(now(), &packet),
                // `disconnect` closed the queue: say goodbye on the wire, then let the top of the
                // loop flush it before ending.
                Err(_) => {
                    stack.close(now());
                    hung_up = true;
                    Ok(())
                }
            },
            () = sleep_until(deadline) => stack.handle_timeout(now()),
        };

        if let Err(e) = step {
            debug!("voip: relay media channel ended: {e}");
            finish(&events, &open, announced, e.into()).await;
            return;
        }
    }
}

/// Park until `deadline`, or forever when there is no timer to wait for.
async fn sleep_until(deadline: Option<Instant>) {
    match deadline {
        Some(at) => tokio::time::sleep_until(tokio::time::Instant::from_std(at)).await,
        None => std::future::pending().await,
    }
}

/// Push one inbound packet to the call, dropping media (but never STUN) under backpressure. Returns
/// `false` once the call has dropped its receiver and the driver should stop.
async fn deliver(events: &async_channel::Sender<RelayTransportEvent>, packet: Bytes) -> bool {
    let kind = classify_relay_packet(&packet);
    match events.try_send(RelayTransportEvent::PacketReceived(packet)) {
        Ok(()) => true,
        // Media is loss tolerant, but STUN control is NOT: dropping a Binding Request means the
        // engine never replies Binding Success, so the relay's consent-freshness check fails and it
        // tears the call down (~4s) -- even though only media should be lossy. Under backpressure
        // drop media, but wait for a slot for STUN.
        //
        // Bounded, and that bound is the whole reason this is not a plain `send().await`: the same
        // task owns the DTLS and SCTP retransmission timers, so a call that stops draining would
        // otherwise stall them here and take the association down -- turning a slow consumer into a
        // dead one. Past the bound the STUN goes the way of the media.
        Err(async_channel::TrySendError::Full(event)) => {
            if kind != RelayPacketKind::Stun {
                return true;
            }
            match tokio::time::timeout(RELAY_STUN_HOLD, events.send(event)).await {
                Ok(sent) => sent.is_ok(),
                Err(_) => true,
            }
        }
        Err(async_channel::TrySendError::Closed(_)) => false,
    }
}

/// Report the end of the channel to whoever is still listening: the caller of `connect_relay_media`
/// if it never opened, the call loop if it did.
async fn finish(
    events: &async_channel::Sender<RelayTransportEvent>,
    open: &async_channel::Sender<Result<(), String>>,
    announced: bool,
    reason: RelayDisconnectReason,
) {
    if announced {
        let _ = events.send(RelayTransportEvent::Disconnected(reason)).await;
    } else {
        let _ = open.send(Err(reason.to_string())).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A relay that advertises 3478 must still be tried on the web client port, since which of the
    /// two a host actually serves is not visible in the offer.
    #[test]
    fn dial_candidates_adds_the_web_client_port() {
        let advertised: SocketAddr = "57.144.43.57:3478".parse().unwrap();
        assert_eq!(
            dial_candidates(advertised),
            vec![
                advertised,
                "57.144.43.57:3480".parse::<SocketAddr>().unwrap()
            ],
            "the advertised port is dialled first, with 3480 raced alongside it"
        );
    }

    /// The failure case: an endpoint already on the web client port must not be dialled twice.
    #[test]
    fn dial_candidates_does_not_duplicate_the_web_client_port() {
        let advertised: SocketAddr = "170.78.54.98:3480".parse().unwrap();
        assert_eq!(dial_candidates(advertised), vec![advertised]);
    }

    /// The channel type WA Web opens the media channel with has to decompose into the SCTP send
    /// parameters the stream is configured with. This is the pair `open_media_stream` applies, and
    /// it comes from the library rather than from two literals, so this pins the type against what
    /// it means rather than against itself.
    #[test]
    fn media_channel_type_means_unordered_and_never_retransmit() {
        let (unordered, reliability) = DataChannel::get_reliability_params(MEDIA_CHANNEL_TYPE);
        assert!(unordered, "real-time RTP must not ride an ordered stream");
        assert_eq!(reliability, rtc_sctp::ReliabilityType::Rexmit);
        assert_eq!(
            MEDIA_MAX_RETRANSMITS, 0,
            "a retransmitted media packet arrives too late to play and blocks the ones behind it"
        );
    }

    // Reads become PacketReceived events; the delivery rule is loss tolerant for media and lossless
    // for STUN, because the relay's consent-freshness check hangs off the STUN exchange.
    #[tokio::test]
    async fn deliver_maps_packets_to_events() {
        let (tx, rx) = async_channel::unbounded();
        assert!(deliver(&tx, Bytes::from_static(&[1, 2, 3])).await);
        assert!(deliver(&tx, Bytes::from_static(&[4, 5])).await);
        match rx.try_recv() {
            Ok(RelayTransportEvent::PacketReceived(b)) => assert_eq!(b.as_ref(), &[1u8, 2, 3][..]),
            other => panic!("expected first packet, got {other:?}"),
        }
        match rx.try_recv() {
            Ok(RelayTransportEvent::PacketReceived(b)) => assert_eq!(b.as_ref(), &[4u8, 5][..]),
            other => panic!("expected second packet, got {other:?}"),
        }
    }

    // A closed receiver (the call dropped it) stops the driver promptly via the try_send Closed arm.
    #[tokio::test]
    async fn deliver_reports_a_closed_receiver() {
        let (tx, rx) = async_channel::unbounded();
        rx.close();
        assert!(
            !deliver(&tx, Bytes::from_static(&[1])).await,
            "a closed event receiver must stop the driver"
        );
    }

    // The STUN hold is bounded, and that bound is what keeps a stalled call from taking the
    // association with it: the driver owns the DTLS and SCTP retransmission timers, so an unbounded
    // wait here would stop them. Nobody ever drains this channel, so the send can only end by
    // timing out -- and `deliver` must come back saying the driver should keep running.
    #[tokio::test(start_paused = true)]
    async fn deliver_bounds_how_long_stun_can_hold_the_driver() {
        let (tx, _rx) = async_channel::bounded(1);
        assert!(deliver(&tx, Bytes::from_static(&[0x90, 0x78, 1, 2])).await);
        let held = tokio::time::Instant::now();
        // The outer bound is what makes an unbounded hold fail here rather than hang the suite.
        let kept_going = tokio::time::timeout(
            RELAY_STUN_HOLD * 50,
            deliver(&tx, Bytes::from_static(&[0x00, 0x01, 5, 6])),
        )
        .await
        .expect("the STUN hold must give up, or the driver's timers never run again");
        assert!(
            kept_going,
            "a STUN packet that cannot be delivered is dropped, not a reason to stop the driver"
        );
        assert!(
            held.elapsed() >= RELAY_STUN_HOLD,
            "the STUN must be held for the bound before being given up on"
        );
    }

    // Under backpressure (a full event channel because the call is behind on media), delivery must
    // drop media but PRESERVE STUN control: a dropped Binding Request never gets a Binding Success,
    // so the relay's consent-freshness check fails and tears the call down. A cap-1 channel is
    // filled by the first media packet; the next media is dropped while the STUN behind it is held
    // by a blocking send -- so the second event the call sees is the STUN, proving the media in
    // between was dropped AND the STUN survived.
    #[tokio::test]
    async fn deliver_preserves_stun_but_drops_media_under_backpressure() {
        let (tx, rx) = async_channel::bounded(1);
        let feed = tokio::spawn(async move {
            assert!(deliver(&tx, Bytes::from_static(&[0x90, 0x78, 1, 2])).await);
            assert!(deliver(&tx, Bytes::from_static(&[0x90, 0x78, 3, 4])).await);
            assert!(deliver(&tx, Bytes::from_static(&[0x00, 0x01, 5, 6])).await);
        });

        // Receiving the first media frees the slot the held STUN send is waiting on.
        let first = rx.recv().await.unwrap();
        assert!(
            matches!(&first, RelayTransportEvent::PacketReceived(d) if d[0] == 0x90),
            "first delivered event is the media that filled the channel, got {first:?}"
        );
        let second = rx.recv().await.unwrap();
        assert!(
            matches!(&second, RelayTransportEvent::PacketReceived(d)
                if classify_relay_packet(d) == RelayPacketKind::Stun),
            "the STUN must be preserved while the media behind the first was dropped, got {second:?}"
        );
        feed.await.unwrap();
    }
}

/// The stack on its own, with no socket and no clock: two [`RelayStack`]s wired to each other in
/// memory, datagrams handed across by the test. This is what sans-IO buys -- the handshakes, the
/// media stream and its teardown are all reachable without a port, a timer or a task, so the paths
/// a live relay only takes at the end of a call can be tested directly.
#[cfg(test)]
mod stack {
    use super::*;

    const CLIENT: &str = "127.0.0.1:5001";
    const RELAY: &str = "127.0.0.1:5002";

    /// What each half made of the datagrams it was handed. A test that only cares about one side
    /// can read that field and drop the rest.
    struct Exchanged {
        at_relay: Result<(), StackError>,
        at_client: Result<(), StackError>,
    }

    /// A pair mid-handshake, plus the instant they were built at.
    struct Pair {
        client: RelayStack,
        relay: RelayStack,
        now: Instant,
    }

    impl Pair {
        fn new() -> Self {
            let client_addr: SocketAddr = CLIENT.parse().unwrap();
            let relay_addr: SocketAddr = RELAY.parse().unwrap();
            let now = now();
            Pair {
                client: RelayStack::connect(client_addr, relay_addr, now).expect("client stack"),
                relay: RelayStack::accept(relay_addr, client_addr).expect("relay stack"),
                now,
            }
        }

        /// Carry every queued datagram across, both ways, until neither side has anything to send.
        /// The link is lossless and in order, so no timer ever has to fire for this to converge.
        fn exchange(&mut self) -> Exchanged {
            let mut out = Exchanged {
                at_relay: Ok(()),
                at_client: Ok(()),
            };
            for _ in 0..64 {
                let mut moved = false;
                while let Some(d) = self.client.poll_transmit() {
                    moved = true;
                    if out.at_relay.is_ok() {
                        out.at_relay = self.relay.handle_datagram(self.now, &d);
                    }
                }
                while let Some(d) = self.relay.poll_transmit() {
                    moved = true;
                    if out.at_client.is_ok() {
                        out.at_client = self.client.handle_datagram(self.now, &d);
                    }
                }
                if !moved {
                    break;
                }
            }
            out
        }

        /// Both halves open, which is the state a call starts from.
        fn connected() -> Self {
            let mut pair = Pair::new();
            let exchanged = pair.exchange();
            exchanged.at_relay.expect("relay accepted the handshake");
            exchanged.at_client.expect("client completed the handshake");
            assert!(pair.client.is_open() && pair.relay.is_open());
            pair
        }
    }

    /// DTLS, the SCTP association and the media stream all come up over a plain in-memory link, with
    /// the dialling and accepting halves of the same type on either end.
    #[test]
    fn both_halves_reach_an_open_media_channel() {
        let pair = Pair::connected();
        assert!(pair.client.is_open(), "the dialling half must open");
        assert!(pair.relay.is_open(), "the accepting half must open");
    }

    /// A message written on one side comes out the other, which is the whole job.
    #[test]
    fn media_crosses_the_open_channel() {
        let mut pair = Pair::connected();
        pair.client
            .write_media(pair.now, &Bytes::from_static(b"hello relay"))
            .expect("client write");
        pair.exchange();
        assert_eq!(
            pair.relay.poll_inbound().as_deref(),
            Some(&b"hello relay"[..])
        );
    }

    /// The relay ends a call by resetting the media stream while the association stays up. The old
    /// DataChannel read pump saw that as `Ok(0)` and reported `Closed`; this asserts the sans-IO
    /// stack still ends the channel rather than parking on a live association nothing arrives on.
    #[test]
    fn a_peer_stream_reset_closes_the_channel() {
        let mut pair = Pair::connected();
        pair.relay
            .close_media_stream(pair.now)
            .expect("relay resets its media stream");
        let Err(e) = pair.exchange().at_client else {
            panic!("the client must not keep the channel open after the peer reset the stream");
        };
        assert!(
            matches!(
                RelayDisconnectReason::from(e),
                RelayDisconnectReason::Closed
            ),
            "a peer reset is the peer hanging up, not a transport error"
        );
        assert!(!pair.client.is_open());
    }

    /// A relay that hangs up properly (SCTP SHUTDOWN, then DTLS close_notify) is reported as a clean
    /// close, not as a read error the call would surface as a failure.
    #[test]
    fn a_peer_close_is_reported_as_closed() {
        let mut pair = Pair::connected();
        pair.relay.close(pair.now);
        let Err(e) = pair.exchange().at_client else {
            panic!("the client must notice the relay closing the channel");
        };
        assert!(matches!(
            RelayDisconnectReason::from(e),
            RelayDisconnectReason::Closed
        ));
    }

    /// Garbage on the wire is one lost datagram, not a lost call: the socket is connected to the
    /// relay, so a record that fails to authenticate means corruption on the path. Once the
    /// association exists the stack drops it and keeps going.
    #[test]
    fn a_corrupt_datagram_after_the_handshake_is_dropped_not_fatal() {
        let mut pair = Pair::connected();
        pair.client
            .handle_datagram(pair.now, &[0xFFu8; 48])
            .expect("a corrupt record must not end the channel");
        assert!(pair.client.is_open());

        // ... and the channel still carries media afterwards.
        pair.relay
            .write_media(pair.now, &Bytes::from_static(b"still here"))
            .expect("relay write");
        pair.exchange();
        assert_eq!(
            pair.client.poll_inbound().as_deref(),
            Some(&b"still here"[..])
        );
    }

    /// The same garbage before the handshake finishes is the handshake failing. There is nothing
    /// else those datagrams could have been, so the caller finds out now instead of waiting out
    /// `RELAY_CONNECT_TIMEOUT`.
    #[test]
    fn a_corrupt_datagram_during_the_handshake_is_fatal() {
        let mut pair = Pair::new();
        assert!(
            pair.client
                .handle_datagram(pair.now, &[0xFFu8; 48])
                .is_err(),
            "a handshake that cannot proceed must fail rather than wait"
        );
    }
}

/// The relay half of the loopback tests: the accepting end of the same stack the production factory
/// dials, driven by the same [`relay_driver`], plus the socket plumbing to put a test in the middle
/// of the wire.
#[cfg(test)]
mod loopback_relay {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// An accepted relay media channel, in the same two halves a connected client gets: a queue to
    /// send on and the stream of what arrived. The relay side of a test is then a few lines rather
    /// than a second event loop.
    pub(super) struct AcceptedRelay {
        pub(super) outbound: async_channel::Sender<Bytes>,
        pub(super) events: async_channel::Receiver<RelayTransportEvent>,
        pub(super) driver: tokio::task::JoinHandle<()>,
    }

    /// Accept the client's media channel on `udp`, which must already be connected to it. Returns
    /// once the DTLS handshake, the SCTP association and the media stream are all up.
    pub(super) async fn accept_relay(udp: UdpSocket) -> AcceptedRelay {
        let local = udp.local_addr().expect("relay local addr");
        let remote = udp.peer_addr().expect("relay peer addr");
        let stack = RelayStack::accept(local, remote).expect("relay stack");

        let (outbound_tx, outbound_rx) = async_channel::bounded(RELAY_SEND_CAP);
        let (events_tx, events_rx) = async_channel::bounded(RELAY_EVENT_CAP);
        let (finished_tx, _finished_rx) = async_channel::bounded(1);
        let (open_tx, open_rx) = async_channel::bounded(1);
        let driver = tokio::spawn(relay_driver(
            udp,
            stack,
            outbound_rx,
            events_tx,
            finished_tx,
            open_tx,
        ));
        open_rx
            .recv()
            .await
            .expect("relay driver reports the channel")
            .expect("relay media channel open");
        AcceptedRelay {
            outbound: outbound_tx,
            events: events_rx,
            driver,
        }
    }

    /// The next packet the relay received, or `None` once the channel ended.
    pub(super) async fn next_packet(relay: &AcceptedRelay) -> Option<Bytes> {
        while let Ok(event) = relay.events.recv().await {
            match event {
                RelayTransportEvent::PacketReceived(p) => return Some(p),
                RelayTransportEvent::Disconnected(_) => return None,
                RelayTransportEvent::Connected => {}
            }
        }
        None
    }

    /// Control surface of [`run_reordering_proxy`], shared with the test that arms it.
    #[derive(Default)]
    pub(super) struct ReorderControl {
        /// How many client-to-relay datagrams to capture before releasing them in reverse arrival
        /// order. Zero means pass everything straight through. A test arms this only after the
        /// handshake has completed, so nothing but application data is ever reordered.
        pub(super) hold_target: AtomicUsize,
        /// How many datagrams are captured so far, so a test can send one message, wait for its
        /// datagram to land here, and thereby know each message got a datagram of its own.
        pub(super) held: AtomicUsize,
    }

    /// Forward datagrams between the client and the relay, optionally reversing a run of them.
    ///
    /// Whether the media channel is ordered is not observable from either endpoint's API: it shows
    /// up only in what SCTP does to out-of-order arrivals. Putting the reordering on the wire is
    /// what makes it observable -- an ordered stream restores send order before delivering, an
    /// unordered one delivers each message the moment it arrives.
    pub(super) async fn run_reordering_proxy(
        front: UdpSocket,
        relay_addr: SocketAddr,
        ctl: Arc<ReorderControl>,
    ) {
        let back = UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("proxy back socket");
        back.connect(relay_addr).await.expect("proxy dial relay");
        let mut client_addr: Option<SocketAddr> = None;
        let mut captured: Vec<Vec<u8>> = Vec::new();
        let mut from_client = vec![0u8; RELAY_DATAGRAM_BUF];
        let mut from_relay = vec![0u8; RELAY_DATAGRAM_BUF];
        loop {
            tokio::select! {
                r = front.recv_from(&mut from_client) => {
                    let Ok((n, peer)) = r else { break };
                    client_addr = Some(peer);
                    if ctl.hold_target.load(Ordering::SeqCst) == 0 {
                        let _ = back.send(&from_client[..n]).await;
                        continue;
                    }
                    captured.push(from_client[..n].to_vec());
                    ctl.held.store(captured.len(), Ordering::SeqCst);
                    if captured.len() >= ctl.hold_target.load(Ordering::SeqCst) {
                        ctl.hold_target.store(0, Ordering::SeqCst);
                        for datagram in captured.drain(..).rev() {
                            let _ = back.send(&datagram).await;
                        }
                    }
                }
                r = back.recv(&mut from_relay) => {
                    let Ok(n) = r else { break };
                    if let Some(peer) = client_addr {
                        let _ = front.send_to(&from_relay[..n], peer).await;
                    }
                }
            }
        }
    }

    /// Poll `cond` until it holds, failing at the deadline. Tests carry a bound, never a fixed wait:
    /// a condition already met returns on the first poll, and a condition that never comes names
    /// itself in the panic instead of hanging the suite.
    pub(super) async fn wait_for(mut cond: impl FnMut() -> bool, what: &str) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while !cond() {
            assert!(
                tokio::time::Instant::now() < deadline,
                "timed out waiting for {what}"
            );
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }
}

/// The media channel must stay UNORDERED, the shape WA Web opens it with. A reliable, ordered stream
/// head-of-line-blocks real-time RTP, which the peer hears as choppy audio -- the regression this
/// pins down, and the one the loopback e2e below cannot see because a loopback socket never reorders
/// on its own.
///
/// This pins the ordered flag, not the retransmit count: `maxRetransmits = 0` changes nothing about
/// delivery on a path that loses nothing, and neither SCTP stack exposes a stream's reliability to
/// read back. What guards the retransmit count instead is that both send parameters are derived from
/// one channel type (see `media_channel_type_means_unordered_and_never_retransmit`), so the ordered
/// flag this test does pin cannot drift away from it.
#[cfg(test)]
mod media_channel_reliability {
    use super::loopback_relay::{
        ReorderControl, accept_relay, next_packet, run_reordering_proxy, wait_for,
    };
    use super::*;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    /// How many messages to write, hold on the wire, and release reversed. Four is enough for the
    /// delivered order to be unambiguous while keeping the hold far below SCTP's retransmit timer,
    /// which would otherwise abandon a held chunk, since this channel never retransmits.
    const MESSAGES: usize = 4;

    /// Reversing the client's datagrams on the wire must reverse what the relay reads. That only
    /// happens on an unordered stream: were the channel ordered, SCTP would hold each arrival until
    /// its predecessor came and hand the relay 0,1,2,3 no matter what the network did.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn media_channel_delivers_out_of_order_arrivals_immediately() {
        let body = async {
            // Relay, proxy, client: the client dials the proxy, which forwards to the relay.
            let relay_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let relay_addr = relay_udp.local_addr().unwrap();
            let front = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let proxy_addr = front.local_addr().unwrap();

            let (order_tx, order_rx) = async_channel::unbounded::<u8>();
            let relay_task = tokio::spawn(async move {
                let mut peek = [0u8; RELAY_DATAGRAM_BUF];
                let (_, proxy_back) = relay_udp.peek_from(&mut peek).await.unwrap();
                relay_udp.connect(proxy_back).await.unwrap();
                let relay = accept_relay(relay_udp).await;
                while let Some(packet) = next_packet(&relay).await {
                    if order_tx.send(packet[0]).await.is_err() {
                        break;
                    }
                }
                relay.driver.abort();
            });

            let ctl = Arc::new(ReorderControl::default());
            let proxy_task = tokio::spawn(run_reordering_proxy(front, relay_addr, ctl.clone()));

            // The production media channel, dialled through the proxy. `connect_relay_media` rather
            // than the factory: the factory also races the web-client port, and a second dial would
            // put datagrams the proxy never sees on the wire.
            let (chan, _events) =
                connect_relay_media(proxy_addr, Arc::new(crate::runtime_impl::TokioRuntime))
                    .await
                    .expect("relay media connect through the proxy");

            // Arm only now: everything before this is handshake traffic, which has to arrive in
            // order for the stack to come up at all.
            ctl.hold_target.store(MESSAGES, Ordering::SeqCst);
            for i in 0..MESSAGES {
                chan.send(Bytes::from(vec![i as u8; 8])).await.unwrap();
                // Step one datagram at a time so each message is provably its own datagram; a batch
                // SCTP bundled into one packet would survive reversal and prove nothing.
                wait_for(
                    || ctl.held.load(Ordering::SeqCst) > i,
                    "the proxy to capture the datagram just written",
                )
                .await;
            }

            let mut delivered = Vec::with_capacity(MESSAGES);
            for _ in 0..MESSAGES {
                let id = tokio::time::timeout(Duration::from_secs(5), order_rx.recv())
                    .await
                    .expect("the relay must receive every released message")
                    .expect("relay read channel stayed open");
                delivered.push(id);
            }

            let expected: Vec<u8> = (0..MESSAGES as u8).rev().collect();
            assert_eq!(
                delivered,
                expected,
                "the relay must read the messages in arrival order; an ordered stream would have \
                 re-sorted them back to {:?}",
                (0..MESSAGES as u8).collect::<Vec<_>>()
            );

            chan.disconnect().await;
            drop(chan);
            relay_task.abort();
            proxy_task.abort();
        };

        tokio::time::timeout(Duration::from_secs(30), body)
            .await
            .expect("the reordering round-trip must complete within the bound");
    }
}

/// The failure paths a relay can take a call down: a handshake that never completes, and a relay
/// that stops answering once it has. Both must end the channel inside a bound rather than park the
/// caller or leave a live-looking transport nothing will ever arrive on.
#[cfg(test)]
mod relay_failures {
    use super::loopback_relay::accept_relay;
    use super::*;
    use std::time::Duration;

    /// An endpoint that answers the ClientHello with something that is not DTLS must fail the
    /// connect, not sit on it. The old stack failed inside the library's own handshake; this stack
    /// owns the handshake, so this is the test that says the ownership did not lose the failure.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn connect_fails_when_the_handshake_never_completes() {
        let junk_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let junk_addr = junk_udp.local_addr().unwrap();
        let junk = tokio::spawn(async move {
            let mut buf = vec![0u8; RELAY_DATAGRAM_BUF];
            while let Ok((_, peer)) = junk_udp.recv_from(&mut buf).await {
                // Well-formed as a datagram, meaningless as a DTLS record.
                let _ = junk_udp.send_to(&[0xFFu8; 32], peer).await;
            }
        });

        let connect = connect_relay_media(junk_addr, Arc::new(crate::runtime_impl::TokioRuntime));
        let outcome = tokio::time::timeout(Duration::from_secs(5), connect)
            .await
            .expect("connect must fail inside the bound rather than park the caller");
        assert!(
            outcome.is_err(),
            "an endpoint that never completes the DTLS handshake must not yield a channel"
        );
        junk.abort();
    }

    /// Hanging up must not wait on a call that has stopped listening. `run_call` leaves its receive
    /// loop before it calls `disconnect`, so once the event channel is full nothing will ever drain
    /// it again -- and a driver that blocked delivering its goodbye there would sit until
    /// `RELAY_DISCONNECT_TIMEOUT` gave up on it. That stall would land exactly when the call was
    /// already behind, which is the worst time to add two seconds to teardown.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn hanging_up_does_not_wait_on_a_backed_up_event_queue() {
        let body = async {
            let relay_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let relay_addr = relay_udp.local_addr().unwrap();
            let relay_task = tokio::spawn(async move {
                let mut peek = [0u8; RELAY_DATAGRAM_BUF];
                let (_, client) = relay_udp.peek_from(&mut peek).await.unwrap();
                relay_udp.connect(client).await.unwrap();
                accept_relay(relay_udp).await
            });

            let (chan, events) =
                connect_relay_media(relay_addr, Arc::new(crate::runtime_impl::TokioRuntime))
                    .await
                    .expect("relay media connect");
            let relay = relay_task.await.unwrap();

            // Back the call up without reading a single event: media past the cap is dropped, so the
            // queue fills to exactly `RELAY_EVENT_CAP` and stays there. Non-STUN payloads, so
            // delivery never takes the STUN hold.
            while events.len() < RELAY_EVENT_CAP {
                relay
                    .outbound
                    .send(Bytes::from_static(&[0x90, 0x78, 7, 7]))
                    .await
                    .expect("relay writes into the open channel");
                tokio::task::yield_now().await;
            }

            let hang_up = tokio::time::Instant::now();
            chan.disconnect().await;
            let took = hang_up.elapsed();
            assert!(
                took < RELAY_DISCONNECT_TIMEOUT / 2,
                "hanging up took {took:?}, so the driver blocked on the full event queue instead of \
                 ending; the bound is {RELAY_DISCONNECT_TIMEOUT:?}"
            );

            relay.driver.abort();
        };

        tokio::time::timeout(Duration::from_secs(30), body)
            .await
            .expect("the hang-up round-trip must complete within the bound");
    }

    /// A relay that goes away after the channel opened must take the channel down with it. On a
    /// connected UDP socket the vanished peer surfaces as an ICMP-driven socket error, which the
    /// driver reports as `Disconnected` instead of leaving the call waiting on silence.
    ///
    /// This covers the socket-error path specifically, which is the only signal a relay that stops
    /// answering without saying so ever produces. It is gated to Linux because it is the platform
    /// whose loopback delivers ICMP port-unreachable to a connected UDP socket; elsewhere the test
    /// would run out its bound and fail on correct code. The two ways a relay ends a call politely
    /// are the portable paths and have their own tests over the in-memory link, so a platform
    /// without that ICMP behaviour still has the close paths covered.
    #[cfg(target_os = "linux")]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_relay_that_disappears_disconnects_the_channel() {
        let body = async {
            let relay_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let relay_addr = relay_udp.local_addr().unwrap();
            let relay_task = tokio::spawn(async move {
                let mut peek = [0u8; RELAY_DATAGRAM_BUF];
                let (_, client) = relay_udp.peek_from(&mut peek).await.unwrap();
                relay_udp.connect(client).await.unwrap();
                accept_relay(relay_udp).await
            });

            let (chan, events) =
                connect_relay_media(relay_addr, Arc::new(crate::runtime_impl::TokioRuntime))
                    .await
                    .expect("relay media connect");
            assert!(matches!(
                events.recv().await,
                Ok(RelayTransportEvent::Connected)
            ));

            // Take the relay away: aborting the driver drops the socket it owns, so the port stops
            // existing and the client's next write draws an ICMP unreachable.
            let relay = relay_task.await.unwrap();
            relay.driver.abort();
            drop(relay);

            // Keep writing until the driver notices. Nothing here waits a fixed span: each send is
            // followed by a short poll of the event stream, and the outer bound is the failure.
            let disconnect = async {
                loop {
                    if chan
                        .send(Bytes::from_static(&[0x90, 0x78, 0, 0]))
                        .await
                        .is_err()
                    {
                        return None;
                    }
                    if let Ok(event) =
                        tokio::time::timeout(Duration::from_millis(50), events.recv()).await
                    {
                        match event {
                            Ok(RelayTransportEvent::Disconnected(reason)) => return Some(reason),
                            Ok(_) => {}
                            Err(_) => return None,
                        }
                    }
                }
            };
            let reason = tokio::time::timeout(Duration::from_secs(10), disconnect)
                .await
                .expect("a vanished relay must disconnect the channel inside the bound");
            assert!(
                reason.is_some(),
                "the channel must end with a Disconnected event, not by the event stream closing"
            );
        };

        tokio::time::timeout(Duration::from_secs(30), body)
            .await
            .expect("the vanished-relay round-trip must complete within the bound");
    }
}

/// End-to-end over a REAL localhost UDP socket: the production `RelayMediaChannelFactory` dials the
/// full DTLS+SCTP+DataChannel stack to an in-test relay server, and the sans-IO `CallEngine` driven
/// by `run_call_tokio` exchanges packets with it. This is the one place CI exercises the driver's
/// I/O, its timers and both handshakes together; the rest of the suite mocks the socket.
#[cfg(all(test, feature = "voip-mlow"))]
mod udp_relay_e2e {
    use super::*;
    use std::time::Duration;

    use wacore::voip::engine::{CallConfig, CallEvent, SequentialTxIds};
    use wacore::voip::session::{CallDirection, MediaPipeline, MediaPipelineParams};
    use wacore::voip::{CallChannels, CallEngine, video_control_channel};

    use super::loopback_relay::{accept_relay, next_packet};
    use crate::voip::driver::run_call_tokio;

    const SELF_LID: &str = "111111111111111:0@lid";
    const PEER_LID: &str = "222222222222222:0@lid";
    const SSRC: u32 = 0x5741_0001;
    const SAMPLES: u32 = 960;

    fn config(relay_addr: SocketAddr) -> CallConfig {
        CallConfig {
            call_id: "CID".into(),
            direction: CallDirection::Incoming,
            self_lid: SELF_LID.into(),
            peer_lid: PEER_LID.into(),
            call_key: (0u8..32).collect(),
            ssrc: SSRC,
            audio: wacore::voip::AudioConfig::MLOW_PCM,
            relay_token: vec![0xAB; 16],
            relay_ip: relay_addr.ip().to_string(),
            relay_port: relay_addr.port(),
            integrity_key: b"relay-key".to_vec(),
            warp_mi_tag_len: 4,
            enable_media: true,
            enable_video: false,
            enable_sframe: false,
        }
    }

    /// One mirrored-peer MLow tone frame, SRTP-protected so the engine's `unprotect_audio` (its recv
    /// keys) accepts it and decodes it to audible playout. A blind echo of the engine's own RTP would
    /// fail unprotect (wrong direction keys), so the relay re-encrypts as the peer would.
    fn peer_rtp(
        peer: &mut MediaPipeline,
        enc: &mut wacore::voip::mlow::MlowEncoder,
        n: u32,
    ) -> Vec<u8> {
        let tone: Vec<f32> = (0..SAMPLES as usize)
            .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
            .collect();
        let frame = enc.encode(&tone).expect("mlow encode");
        peer.protect_audio(&frame)
    }

    // The native transport over a real loopback UDP socket end-to-end: the engine's STUN allocate
    // reaches the relay, the relay's allocate-success drives the engine to RelayAllocated, a mic frame
    // becomes outbound RTP on the wire, and the relay's mirrored-peer RTP decodes to non-silent
    // playout. Bounded by a timeout so a wedged handshake fails fast instead of hanging CI.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn native_transport_relays_packets_over_loopback_udp() {
        let body = async {
            // The relay's UDP socket. Learn the client addr from its first datagram (the DTLS
            // ClientHello), then connect so the server socket is a point-to-point pipe for the stack.
            let server_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let server_addr = server_udp.local_addr().unwrap();

            // The relay sets this once it sees the engine's outbound RTP (the mic frame on the wire).
            let saw_outbound_rtp = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let saw_for_relay = saw_outbound_rtp.clone();

            let relay_task = tokio::spawn(async move {
                let mut peek = [0u8; RELAY_DATAGRAM_BUF];
                let (_, client_addr) = server_udp.peek_from(&mut peek).await.unwrap();
                server_udp.connect(client_addr).await.unwrap();
                let relay = accept_relay(server_udp).await;

                // A mirrored peer: its self LID is the engine's peer LID, so its protect keys match the
                // engine's unprotect keys.
                let call_key: Vec<u8> = (0u8..32).collect();
                let mut peer = MediaPipeline::new(&MediaPipelineParams {
                    call_key: &call_key,
                    self_lid: PEER_LID,
                    peer_lid: SELF_LID,
                    ssrc: SSRC,
                    samples_per_packet: SAMPLES,
                    warp_mi_tag_len: 4,
                })
                .unwrap();
                let mut enc = wacore::voip::mlow::MlowEncoder::new();
                let mut sent_peer_rtp = 0u32;

                // React to the engine's traffic until the driver tears the channel down: ack every
                // allocate (initial + keepalive), and on the first outbound RTP stream two peer RTP
                // frames back (two so the engine's playout prebuffer reaches its target). The channel
                // must stay open so the driver keeps running its 20ms playout ticks and drains the
                // buffered peer audio; closing early would stop playout before it became audible.
                while let Some(packet) = next_packet(&relay).await {
                    match classify_relay_packet(&packet) {
                        RelayPacketKind::Stun
                            if wacore::voip::stun::stun_message_type(&packet)
                                == Some(wacore::voip::stun::MSG_ALLOCATE_REQUEST) =>
                        {
                            // A bare allocate-success header (magic cookie set, no MI/FP) is all the
                            // engine's is_allocate_or_binding_success requires.
                            let transaction_id: [u8; 12] =
                                wacore::voip::stun::stun_transaction_id(&packet)
                                    .expect("allocate transaction id")
                                    .try_into()
                                    .expect("STUN transaction IDs are 12 bytes");
                            let ok = wacore::voip::stun::encode_stun_request(
                                wacore::voip::stun::MSG_ALLOCATE_SUCCESS,
                                &transaction_id,
                                &[],
                                None,
                                false,
                            );
                            relay.outbound.send(Bytes::from(ok)).await.unwrap();
                        }
                        RelayPacketKind::Rtp => {
                            saw_for_relay.store(true, std::sync::atomic::Ordering::SeqCst);
                            while sent_peer_rtp < 2 {
                                let pkt = peer_rtp(&mut peer, &mut enc, sent_peer_rtp);
                                relay.outbound.send(Bytes::from(pkt)).await.unwrap();
                                sent_peer_rtp += 1;
                            }
                        }
                        _ => {}
                    }
                }
                relay.driver.abort();
            });

            // The REAL native factory + transport dialing the relay over loopback UDP.
            let factory = RelayMediaChannelFactory::new(
                server_addr,
                Arc::new(crate::runtime_impl::TokioRuntime),
            );
            let (transport, relay_events) = factory.connect().await.expect("native relay connect");
            // The call loop takes ownership of the transport; keep a handle for the teardown below.
            let teardown = transport.clone();
            let (mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
            let tone: Vec<i16> = (0..SAMPLES as usize)
                .map(|i| (8000.0 * (i as f32 * 0.1).sin()) as i16)
                .collect();
            mic_tx.try_send(tone).unwrap();
            let (spk_tx, spk_rx) = async_channel::unbounded::<Vec<i16>>();
            let (ev_tx, ev_rx) = async_channel::unbounded::<CallEvent>();

            let eng =
                CallEngine::new(config(server_addr), Box::new(SequentialTxIds::new())).unwrap();
            let driver = tokio::spawn(run_call_tokio(
                transport,
                relay_events,
                CallChannels {
                    mic: mic_rx,
                    speaker: spk_tx,
                    encoded_audio_in: async_channel::bounded(1).1,
                    encoded_audio_out: async_channel::bounded(1).0,
                    events: ev_tx,
                    rekey: None,
                    video_in: async_channel::bounded(1).1,
                    video_out: async_channel::bounded(1).0,
                    video_ctl: video_control_channel().1,
                    group_ctl: None,
                },
                eng,
            ));

            // RelayAllocated must come from the real allocate-success over the wire.
            let allocated = async {
                loop {
                    if matches!(ev_rx.recv().await, Ok(CallEvent::RelayAllocated)) {
                        break;
                    }
                }
            };
            tokio::time::timeout(Duration::from_secs(10), allocated)
                .await
                .expect("the relay's allocate-success must surface RelayAllocated");

            // Audible playout from the relay's mirrored-peer RTP, decoded end-to-end. The driver's real
            // 20ms playout ticks drain the buffered peer frames; the channel stays open meanwhile.
            let audible = async {
                loop {
                    if let Ok(frame) = spk_rx.recv().await
                        && frame.iter().any(|&s| s != 0)
                    {
                        break;
                    }
                }
            };
            tokio::time::timeout(Duration::from_secs(10), audible)
                .await
                .expect("peer RTP must decode to audible playout over the real transport");

            // The relay observed the engine's mic frame as outbound RTP on the wire.
            assert!(
                saw_outbound_rtp.load(std::sync::atomic::Ordering::SeqCst),
                "the engine's mic frame must reach the relay as outbound RTP"
            );

            // Tear the call down the way the call loop does, with an explicit disconnect: that is
            // what puts SCTP SHUTDOWN and DTLS close_notify on the wire, so the relay's channel ends
            // and its task returns. Aborting alone would only drop the socket, leaving the relay
            // parked on a read that nothing will ever answer.
            drop(mic_tx);
            driver.abort();
            teardown.disconnect().await;
            // Surface a relay-task panic instead of swallowing it (the outer 30s timeout bounds a
            // truly stuck join).
            if let Ok(joined) = tokio::time::timeout(Duration::from_secs(5), relay_task).await {
                joined.expect("relay task panicked after teardown");
            }
        };

        // Bound the whole test so a stuck DTLS/SCTP handshake fails fast instead of blocking CI.
        tokio::time::timeout(Duration::from_secs(30), body)
            .await
            .expect("the loopback relay round-trip must complete within the bound");
    }
}
