//! Relay media transport: a pre-negotiated WebRTC DataChannel over SCTP-over-DTLS-over-UDP
//! to a single WhatsApp relay endpoint. The synthetic-SDP / wrtc dance reduces, at this layer,
//! to: connect a UDP socket to the relay, DTLS-handshake as the client (self-signed cert,
//! server-cert verification skipped, since SRTP keys come from callKey/hbh_key, not DTLS),
//! run an SCTP association over it, and open the pre-negotiated id=0 DataChannel that carries
//! STUN/RTP/RTCP as binary messages.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use bytes::Bytes;
use tokio::net::UdpSocket;
use webrtc_data::data_channel::{Config as DcConfig, DataChannel};
use webrtc_data::message::message_channel_open::ChannelType;
use webrtc_dtls::config::Config as DtlsConfig;
use webrtc_dtls::conn::DTLSConn;
use webrtc_dtls::crypto::Certificate;
use webrtc_sctp::association::{Association, Config as SctpConfig};
use webrtc_sctp::chunk::chunk_payload_data::PayloadProtocolIdentifier;
use webrtc_sctp::stream::ReliabilityType;
use webrtc_util_011::Conn as Conn011;

use wacore::runtime::{AbortHandle, Runtime};
use wacore::voip::engine::TxIdSource;
use wacore::voip::transport::{
    RelayDisconnectReason, RelayTransport, RelayTransportEvent, RelayTransportFactory,
};

/// DataChannel label WA Web uses (pre-negotiated, id=0).
const DATA_CHANNEL_LABEL: &str = "pre-negotiated";
/// SCTP-over-DTLS WebRTC port.
const SCTP_PORT: u16 = 5000;

// First-byte relay-packet demux moved to the portable core; re-exported so the existing
// `whatsapp_rust::voip::transport::{classify_relay_packet, RelayPacketKind}` paths stay stable.
pub use wacore::voip::demux::{RelayPacketKind, classify_relay_packet};

/// Bridges the util-0.11 `Conn` produced by `webrtc-dtls` to the util-0.17 `Conn` consumed
/// by `webrtc-sctp`. The two traits are identical across the version gap, so this is pure
/// delegation with error remapping.
struct DtlsToSctpConn(Arc<DTLSConn>);

fn remap(e: webrtc_util_011::Error) -> webrtc_util::Error {
    webrtc_util::Error::Other(e.to_string())
}

#[async_trait]
impl webrtc_util::Conn for DtlsToSctpConn {
    async fn connect(&self, addr: SocketAddr) -> Result<(), webrtc_util::Error> {
        Conn011::connect(&*self.0, addr).await.map_err(remap)
    }
    async fn recv(&self, buf: &mut [u8]) -> Result<usize, webrtc_util::Error> {
        Conn011::recv(&*self.0, buf).await.map_err(remap)
    }
    async fn recv_from(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), webrtc_util::Error> {
        Conn011::recv_from(&*self.0, buf).await.map_err(remap)
    }
    async fn send(&self, buf: &[u8]) -> Result<usize, webrtc_util::Error> {
        Conn011::send(&*self.0, buf).await.map_err(remap)
    }
    async fn send_to(&self, buf: &[u8], target: SocketAddr) -> Result<usize, webrtc_util::Error> {
        Conn011::send_to(&*self.0, buf, target).await.map_err(remap)
    }
    fn local_addr(&self) -> Result<SocketAddr, webrtc_util::Error> {
        Conn011::local_addr(&*self.0).map_err(remap)
    }
    fn remote_addr(&self) -> Option<SocketAddr> {
        Conn011::remote_addr(&*self.0)
    }
    async fn close(&self) -> Result<(), webrtc_util::Error> {
        Conn011::close(&*self.0).await.map_err(remap)
    }
    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }
}

/// An open relay media channel: STUN/RTP/RTCP travel as binary DataChannel messages.
pub struct RelayMediaChannel {
    dc: Arc<DataChannel>,
    /// Aborts the inbound read pump when this channel is dropped or disconnected. An aborted driver
    /// never runs `disconnect`, and a parked DataChannel read won't observe the dropped event
    /// receiver, so this edge-triggers the pump's teardown instead of polling. The pump holds only
    /// `Arc<DataChannel>`, not this channel, so there is no reference cycle keeping it alive. (The
    /// relay socket itself lives in the association below, freed by closing it.)
    pump: std::sync::Mutex<Option<AbortHandle>>,
    /// The SCTP association behind `dc`. `dc.close()` only resets the stream; the association's two
    /// background loops (`read_loop`/`write_loop`) hold their own strong refs and keep the `net_conn`
    /// (DTLS-over-UDP socket) alive until `Association::close()` runs. Held so teardown can close it:
    /// taken+closed by `disconnect`, or by `Drop` (off-task, since `Drop` can't await) when the driver
    /// was aborted and `disconnect` never ran -- otherwise the loops + socket leak until the relay
    /// happens to drop the flow.
    assoc: std::sync::Mutex<Option<Arc<Association>>>,
    /// Portable runtime handle (not `tokio::spawn`) so `Drop` can spawn the off-task association
    /// close without binding this transport to a specific runtime, matching the sans-IO seam.
    runtime: Arc<dyn Runtime>,
}

impl RelayMediaChannel {
    /// Lock the read-pump slot, recovering the guard on a poisoned mutex rather than panicking.
    #[inline]
    fn lock_pump(&self) -> std::sync::MutexGuard<'_, Option<AbortHandle>> {
        self.pump.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Lock the association slot, recovering the guard on a poisoned mutex rather than panicking.
    #[inline]
    fn lock_assoc(&self) -> std::sync::MutexGuard<'_, Option<Arc<Association>>> {
        self.assoc.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl Drop for RelayMediaChannel {
    fn drop(&mut self) {
        // The pump's own `AbortHandle` Drop already aborts the read pump. On an aborted driver
        // `disconnect` never ran, so close the association here to release its loops + UDP socket.
        // `Drop` can't await; spawn the close on the portable runtime and `detach` so the returned
        // handle dropping at end of scope doesn't abort the close. This runs on the call's runtime
        // (the task owning this transport is being torn down on it), so the spawn always lands.
        let assoc = self.lock_assoc().take();
        if let Some(assoc) = assoc {
            self.runtime
                .spawn(Box::pin(async move {
                    let _ = assoc.close().await;
                }))
                .detach();
        }
    }
}

/// Connect the full media stack to one relay endpoint. Deferred for live validation.
/// webrtc-dtls selects rustls' process-default `CryptoProvider`. The dependency tree carries both
/// `ring` and `aws-lc-rs`, so rustls can't auto-pick one and the first handshake would panic.
/// Install `ring` once; if the app already installed a provider the error is ignored and theirs
/// stands, so this never overrides a caller's choice.
fn install_default_crypto_provider() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

/// The multi-step setup is kept on `anyhow` for per-step `.context()`; the factory wraps the
/// aggregate (a connect failure is fatal for the call).
pub async fn connect_relay_media(
    relay_addr: SocketAddr,
    runtime: Arc<dyn Runtime>,
) -> Result<RelayMediaChannel> {
    // 1. UDP socket connected to the relay. Bind in the relay's address family so an IPv6 relay is
    //    reachable (WA relays are IPv4 today, but the unspecified bind must match to connect).
    let bind_addr = if relay_addr.is_ipv6() {
        "[::]:0"
    } else {
        "0.0.0.0:0"
    };
    let udp = UdpSocket::bind(bind_addr).await.context("bind udp")?;
    udp.connect(relay_addr)
        .await
        .context("connect udp to relay")?;
    let udp: Arc<dyn Conn011 + Send + Sync> = Arc::new(udp);

    // 2. DTLS client. Self-signed cert (relay does not validate the client cert); skip
    //    server-cert verification (the SDP fingerprint is fixed/cosmetic; media auth is HBH SRTP).
    install_default_crypto_provider();
    let cert = Certificate::generate_self_signed(vec!["wa-voip".to_owned()])
        .map_err(|e| anyhow!("dtls self-signed cert: {e}"))?;
    let dtls_config = DtlsConfig {
        certificates: vec![cert],
        insecure_skip_verify: true,
        // Only the SNI/cert-hostname, irrelevant under insecure_skip_verify; set to silence the
        // library's empty-remote-addr warn.
        server_name: "localhost".to_owned(),
        ..Default::default()
    };
    let dtls = DTLSConn::new(udp, dtls_config, true, None)
        .await
        .map_err(|e| anyhow!("dtls handshake: {e}"))?;
    let net_conn: Arc<dyn webrtc_util::Conn + Send + Sync> =
        Arc::new(DtlsToSctpConn(Arc::new(dtls)));

    // 3. SCTP association (client) over the DTLS connection.
    let assoc = Association::client(SctpConfig {
        net_conn,
        max_receive_buffer_size: 0,
        max_message_size: 0,
        name: "wa-voip".to_owned(),
        remote_port: SCTP_PORT,
        local_port: SCTP_PORT,
    })
    .await
    .map_err(|e| anyhow!("sctp client: {e}"))?;

    // 4. Pre-negotiated id=0 media DataChannel, opened UNRELIABLE + UNORDERED (see
    //    `open_media_datachannel`). Keep the `Arc<Association>` (the channel holds it for teardown).
    let assoc = Arc::new(assoc);
    let dc = open_media_datachannel(&assoc).await?;

    Ok(RelayMediaChannel {
        dc: Arc::new(dc),
        pump: std::sync::Mutex::new(None),
        assoc: std::sync::Mutex::new(Some(assoc)),
        runtime,
    })
}

/// Open the pre-negotiated id=0 media DataChannel as UNRELIABLE + UNORDERED, the shape WA Web opens
/// this channel with (`ordered=false, maxRetransmits=0`). Real-time RTP must NOT ride a reliable+
/// ordered SCTP stream: a single reordered/lost packet head-of-line-blocks every later one (the
/// receiver holds them until the gap is retransmitted, then delivers a burst), which the peer hears as
/// choppy/garbled audio -- worst on links that reorder at all (Wi-Fi). Reliability is per-sender, so
/// this is also why outbound was choppy while inbound stayed clean (the peer already sends unreliable).
/// `webrtc-data::DataChannel::client` does NOT commit the config's reliability for a `negotiated`
/// channel (only the DCEP-accept path does), so we set it on the SCTP stream directly -- the same
/// mapping `commit_reliability_params` applies for `PartialReliableRexmitUnordered` with 0 retransmits.
async fn open_media_datachannel(assoc: &Arc<Association>) -> anyhow::Result<DataChannel> {
    let stream = assoc
        .open_stream(0, PayloadProtocolIdentifier::Binary)
        .await
        .map_err(|e| anyhow!("open sctp media stream: {e}"))?;
    // unordered = true, partial-reliability by retransmit count = 0 (never retransmit).
    stream.set_reliability_params(true, ReliabilityType::Rexmit, 0);
    DataChannel::client(
        stream,
        DcConfig {
            channel_type: ChannelType::PartialReliableRexmitUnordered,
            reliability_parameter: 0,
            negotiated: true,
            label: DATA_CHANNEL_LABEL.to_owned(),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| anyhow!("datachannel client: {e}"))
}

// RelayMediaChannel is the concrete native RelayTransport: the platform half of the seam the
// sans-IO CallEngine drives through. The engine never names this type; the driver holds it as
// Arc<dyn RelayTransport>.

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
        self.dc
            .write(&data)
            .await
            .map(|_| ())
            .map_err(|e| anyhow!("relay datachannel write: {e}"))
    }

    async fn disconnect(&self) {
        // Stop the read pump (Drop would also abort it) before closing the channel.
        if let Some(h) = self.lock_pump().take() {
            h.abort();
        }
        let _ = self.dc.close().await;
        // Close the association so `net_conn` (the DTLS/UDP socket) and the read/write loops are
        // released; `dc.close()` above only reset the stream. Drop covers the aborted-driver path.
        // Take out of the lock first: a `std::sync` guard held across the await would make the
        // `disconnect` future `!Send`.
        let assoc = self.lock_assoc().take();
        if let Some(assoc) = assoc {
            let _ = assoc.close().await;
        }
    }
}

/// Inbound event-channel depth. VoIP is loss tolerant, so the read pump drops packets rather than
/// block when the driver falls behind.
const RELAY_EVENT_CAP: usize = 256;
/// One DataChannel message fits in a UDP MTU; 1500 covers any STUN/RTP/RTCP packet WA sends. Used by
/// the in-test loopback relay, whose messages are single small packets.
#[cfg(test)]
const RELAY_READ_BUF: usize = 1500;
/// SCTP read buffer for the inbound pump. webrtc-sctp reassembles inbound messages up to its default
/// `max_message_size` (65536) regardless of MTU, and a buffer smaller than the delivered message
/// yields a fatal `ErrShortBuffer` that drops the call. Size to the reassembly cap so no relay-sent
/// message can truncate. Heap, allocated once per call and reused (not a stack array).
const RELAY_SCTP_READ_BUF: usize = 65536;
/// Upper bound on the relay handshake (UDP+DTLS+SCTP+DataChannel); a black-holed or wedged endpoint
/// fails here instead of parking the caller forever.
const RELAY_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);

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

#[async_trait]
impl RelayTransportFactory for RelayMediaChannelFactory {
    async fn connect(
        &self,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )> {
        // Bound the UDP+DTLS+SCTP+DataChannel handshake: a relay whose UDP is reachable but whose
        // DTLS/SCTP wedges (black-holed endpoint) would otherwise park the caller forever with no
        // failure surfaced. Matches the old connect_and_allocate's 12s timeout.
        let chan = Arc::new(
            wacore::runtime::timeout(
                &*self.runtime,
                RELAY_CONNECT_TIMEOUT,
                connect_relay_media(self.addr, self.runtime.clone()),
            )
                .await
                .map_err(|_| {
                    anyhow!("relay connect timed out after {RELAY_CONNECT_TIMEOUT:?} (DTLS/SCTP didn't complete)")
                })?
                .map_err(|e| anyhow!("relay connect: {e}"))?,
        );
        let (tx, rx) = async_channel::bounded(RELAY_EVENT_CAP);
        let _ = tx.try_send(RelayTransportEvent::Connected);
        // Pump over a clone of just the DataChannel, not the channel that owns the handle: the task
        // must hold no path back to its own AbortHandle, or that Arc cycle would keep it alive. Store
        // the handle so disconnect() (and Drop) edge-trigger the pump's teardown instead of polling.
        let pump = self
            .runtime
            .spawn(Box::pin(relay_read_pump(chan.dc.clone(), tx)));
        *chan.lock_pump() = Some(pump);
        Ok((chan as Arc<dyn RelayTransport>, rx))
    }
}

/// The inbound half [`relay_read_pump`] consumes: a readable relay channel. Implemented by the
/// webrtc-rs [`DataChannel`]; abstracting it lets the pump's event mapping be unit-tested without a
/// live relay. `Ok(0)` means the stream reset (EOF); the concrete read error is stringified at this
/// boundary so the pump (and its tests) stay independent of the webrtc error type.
#[async_trait]
trait RelayChannelRead: Send + Sync {
    async fn read_message(&self, buf: &mut [u8]) -> Result<usize, String>;
}

#[async_trait]
impl RelayChannelRead for DataChannel {
    async fn read_message(&self, buf: &mut [u8]) -> Result<usize, String> {
        self.read(buf).await.map_err(|e| e.to_string())
    }
}

/// Pump channel reads into the event channel until the stream resets, a read errors, or the driver
/// drops the receiver. It parks in `read_message` between packets; teardown is edge-triggered by the
/// [`AbortHandle`] the factory stored on the channel (aborted by `disconnect`/Drop), so an aborted
/// driver that never runs `disconnect` still releases this task and the relay socket. The pump holds
/// only the channel reader, never the `RelayMediaChannel`, so it forms no cycle with its own handle.
async fn relay_read_pump<R: RelayChannelRead>(
    dc: Arc<R>,
    tx: async_channel::Sender<RelayTransportEvent>,
) {
    let mut buf = vec![0u8; RELAY_SCTP_READ_BUF];
    loop {
        match dc.read_message(&mut buf).await {
            // After the call ends the SCTP stream resets and read returns Ok(0) forever (EOF).
            Ok(0) => {
                let _ = tx
                    .send(RelayTransportEvent::Disconnected(
                        RelayDisconnectReason::Closed,
                    ))
                    .await;
                break;
            }
            Ok(n) => {
                match tx.try_send(RelayTransportEvent::PacketReceived(Bytes::copy_from_slice(
                    &buf[..n],
                ))) {
                    Ok(()) => {}
                    // Media is loss tolerant, but STUN control is NOT: dropping a Binding Request
                    // means the engine never replies Binding Success, so the relay's
                    // consent-freshness check fails and it tears the call down (~4s) -- even though
                    // only media should be lossy. Under backpressure drop media, but preserve STUN
                    // with a blocking send (the driver is draining, so it unblocks promptly; a closed
                    // channel means the driver is gone).
                    Err(async_channel::TrySendError::Full(ev)) => {
                        if classify_relay_packet(&buf[..n]) == RelayPacketKind::Stun
                            && tx.send(ev).await.is_err()
                        {
                            break;
                        }
                    }
                    // Driver gone (the call ended): stop pumping.
                    Err(async_channel::TrySendError::Closed(_)) => break,
                }
            }
            Err(e) => {
                let _ = tx
                    .send(RelayTransportEvent::Disconnected(
                        RelayDisconnectReason::ReadError(e),
                    ))
                    .await;
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Scripted channel reader: each queued entry is one `read_message` result -- `Ok(bytes)` a
    /// message, `Err` a read error. An empty queue returns `Ok(0)` (EOF) so the pump terminates.
    struct ScriptedReader {
        msgs: Mutex<VecDeque<Result<Vec<u8>, String>>>,
    }
    impl ScriptedReader {
        fn new(msgs: impl IntoIterator<Item = Result<Vec<u8>, String>>) -> Arc<Self> {
            Arc::new(Self {
                msgs: Mutex::new(msgs.into_iter().collect()),
            })
        }
    }
    #[async_trait]
    impl RelayChannelRead for ScriptedReader {
        async fn read_message(&self, buf: &mut [u8]) -> Result<usize, String> {
            match self.msgs.lock().unwrap().pop_front() {
                Some(Ok(data)) => {
                    buf[..data.len()].copy_from_slice(&data);
                    Ok(data.len())
                }
                Some(Err(e)) => Err(e),
                None => Ok(0), // EOF
            }
        }
    }

    // Reads become PacketReceived events; a drained stream (Ok(0)) becomes Disconnected(Closed) and
    // ends the pump. Unbounded channel so the EOF send never blocks.
    #[tokio::test]
    async fn pump_maps_reads_then_eof_to_disconnect() {
        let reader = ScriptedReader::new([Ok(vec![1, 2, 3]), Ok(vec![4, 5])]);
        let (tx, rx) = async_channel::unbounded();
        relay_read_pump(reader, tx).await;
        match rx.try_recv() {
            Ok(RelayTransportEvent::PacketReceived(b)) => assert_eq!(b.as_ref(), &[1u8, 2, 3][..]),
            other => panic!("expected first packet, got {other:?}"),
        }
        match rx.try_recv() {
            Ok(RelayTransportEvent::PacketReceived(b)) => assert_eq!(b.as_ref(), &[4u8, 5][..]),
            other => panic!("expected second packet, got {other:?}"),
        }
        assert!(matches!(
            rx.try_recv(),
            Ok(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed
            ))
        ));
        assert!(rx.try_recv().is_err(), "no events after Disconnected");
    }

    // A read error becomes Disconnected(ReadError(..)) carrying the message, and ends the pump.
    #[tokio::test]
    async fn pump_maps_read_error_to_disconnect() {
        let reader = ScriptedReader::new([Ok(vec![9]), Err("relay read failed".to_string())]);
        let (tx, rx) = async_channel::unbounded();
        relay_read_pump(reader, tx).await;
        assert!(matches!(
            rx.try_recv(),
            Ok(RelayTransportEvent::PacketReceived(_))
        ));
        match rx.try_recv() {
            Ok(RelayTransportEvent::Disconnected(RelayDisconnectReason::ReadError(e))) => {
                assert_eq!(e, "relay read failed");
            }
            other => panic!("expected Disconnected(ReadError), got {other:?}"),
        }
    }

    // A closed receiver (the driver dropped it) stops the pump promptly via the try_send Closed arm,
    // without hanging or emitting more.
    #[tokio::test]
    async fn pump_stops_when_receiver_closed() {
        let reader = ScriptedReader::new([Ok(vec![1]), Ok(vec![2]), Ok(vec![3])]);
        let (tx, rx) = async_channel::unbounded();
        rx.close(); // driver gone before the pump runs
        relay_read_pump(reader, tx).await;
        assert!(rx.try_recv().is_err());
    }

    // Under backpressure (a full event channel because the driver is behind on media), the pump must
    // drop media but PRESERVE STUN control: a dropped Binding Request never gets a Binding Success, so
    // the relay's consent-freshness check fails and tears the call down. A cap-1 channel is filled by
    // the first media packet; the next media is dropped while the STUN behind it is held by a blocking
    // send -- so the second event the driver sees is the STUN, proving the media in between was
    // dropped AND the STUN survived. (The old drop-everything pump delivered Disconnected here.)
    #[tokio::test]
    async fn pump_preserves_stun_but_drops_media_under_backpressure() {
        let reader = ScriptedReader::new([
            Ok(vec![0x90, 0x78, 1, 2]), // RTP media: fills the cap-1 channel
            Ok(vec![0x90, 0x78, 3, 4]), // RTP media: dropped while the channel is full
            Ok(vec![0x00, 0x01, 5, 6]), // STUN binding request: must survive the backpressure
        ]);
        let (tx, rx) = async_channel::bounded(1);
        let pump = tokio::spawn(relay_read_pump(reader, tx));

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
        assert!(matches!(
            rx.recv().await.unwrap(),
            RelayTransportEvent::Disconnected(RelayDisconnectReason::Closed)
        ));
        pump.await.unwrap();
    }
}

/// End-to-end over a REAL localhost UDP socket: the production `RelayMediaChannelFactory` dials the
/// full DTLS+SCTP+DataChannel stack to an in-test relay server, and the sans-IO `CallEngine` driven
/// by `run_call_tokio` exchanges packets with it. This is the one place CI exercises the native
/// transport's I/O (the rest of the suite mocks the socket), closing the `connect_relay_media is not
/// exercised in CI` gap in the module header.
#[cfg(test)]
mod udp_relay_e2e {
    use super::*;
    use std::time::Duration;

    use wacore::voip::engine::{CallConfig, CallEvent, SequentialTxIds};
    use wacore::voip::session::{CallDirection, MediaPipeline, MediaPipelineParams};
    use wacore::voip::{CallChannels, CallEngine};
    use webrtc_dtls::config::Config as DtlsConfig;
    use webrtc_sctp::association::{Association, Config as SctpConfig};

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
            samples_per_packet: SAMPLES,
            relay_token: vec![0xAB; 16],
            relay_ip: relay_addr.ip().to_string(),
            relay_port: relay_addr.port(),
            integrity_key: b"relay-key".to_vec(),
            warp_mi_tag_len: 4,
            enable_media: true,
            enable_sframe: false,
        }
    }

    /// The relay server half of the DataChannel: the DTLS server handshake (mirroring the client's
    /// skip-verify self-signed setup), the SCTP server association, and the same pre-negotiated id=0
    /// stream the client dials. Returns once the channel is open. Bridges DTLS's util-0.11 `Conn` to
    /// SCTP's util-0.17 `Conn` via the same `DtlsToSctpConn` the client uses.
    async fn accept_relay(udp: UdpSocket) -> Arc<DataChannel> {
        let udp: Arc<dyn Conn011 + Send + Sync> = Arc::new(udp);
        let cert = Certificate::generate_self_signed(vec!["wa-relay".to_owned()]).unwrap();
        let dtls = DTLSConn::new(
            udp,
            DtlsConfig {
                certificates: vec![cert],
                insecure_skip_verify: true,
                ..Default::default()
            },
            false, // server
            None,
        )
        .await
        .expect("relay dtls server handshake");
        let net_conn: Arc<dyn webrtc_util::Conn + Send + Sync> =
            Arc::new(DtlsToSctpConn(Arc::new(dtls)));
        let assoc = Association::server(SctpConfig {
            net_conn,
            max_receive_buffer_size: 0,
            max_message_size: 0,
            name: "wa-relay".to_owned(),
            remote_port: SCTP_PORT,
            local_port: SCTP_PORT,
        })
        .await
        .expect("relay sctp server");
        // Pre-negotiated channels carry no DCEP handshake, so both ends open stream id=0 directly.
        // The simulated relay uses the same unreliable+unordered media channel the real peer does.
        let dc = open_media_datachannel(&Arc::new(assoc))
            .await
            .expect("relay datachannel");
        Arc::new(dc)
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
        // Both ring and aws-lc-rs are in the tree, so install ring before the server-side handshake
        // (the client side installs it too, but the in-test relay may handshake first).
        install_default_crypto_provider();
        let body = async {
            // The relay's UDP socket. Learn the client addr from its first datagram (the DTLS
            // ClientHello), then connect so the server socket is a point-to-point pipe for the stack.
            let server_udp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            let server_addr = server_udp.local_addr().unwrap();

            // The relay sets this once it sees the engine's outbound RTP (the mic frame on the wire).
            let saw_outbound_rtp = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let saw_for_relay = saw_outbound_rtp.clone();

            let relay_task = tokio::spawn(async move {
                let mut peek = [0u8; RELAY_READ_BUF];
                let (_, client_addr) = server_udp.peek_from(&mut peek).await.unwrap();
                server_udp.connect(client_addr).await.unwrap();
                let dc = accept_relay(server_udp).await;

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

                let mut buf = vec![0u8; RELAY_READ_BUF];
                let mut sent_peer_rtp = 0u32;
                // React to the engine's traffic until the driver tears the channel down (read EOF/err):
                // ack every allocate (initial + keepalive), and on the first outbound RTP stream two
                // peer RTP frames back (two so the engine's playout prebuffer reaches its target). The
                // channel must stay open so the driver keeps running its 20ms playout ticks and drains
                // the buffered peer audio; closing early would stop playout before it became audible.
                loop {
                    let n = match dc.read(&mut buf).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => n,
                    };
                    match classify_relay_packet(&buf[..n]) {
                        RelayPacketKind::Stun
                            if wacore::voip::stun::stun_message_type(&buf[..n])
                                == Some(wacore::voip::stun::MSG_ALLOCATE_REQUEST) =>
                        {
                            // A bare allocate-success header (magic cookie set, no MI/FP) is all the
                            // engine's is_allocate_or_binding_success requires.
                            let ok = wacore::voip::stun::encode_stun_request(
                                wacore::voip::stun::MSG_ALLOCATE_SUCCESS,
                                &[1u8; 12],
                                &[],
                                None,
                                false,
                            );
                            dc.write(&Bytes::from(ok)).await.unwrap();
                        }
                        RelayPacketKind::Rtp => {
                            saw_for_relay.store(true, std::sync::atomic::Ordering::SeqCst);
                            while sent_peer_rtp < 2 {
                                let pkt = peer_rtp(&mut peer, &mut enc, sent_peer_rtp);
                                dc.write(&Bytes::from(pkt)).await.unwrap();
                                sent_peer_rtp += 1;
                            }
                        }
                        _ => {}
                    }
                }
                let _ = dc.close().await;
            });

            // The REAL native factory + transport dialing the relay over loopback UDP.
            let factory = RelayMediaChannelFactory::new(
                server_addr,
                Arc::new(crate::runtime_impl::TokioRuntime),
            );
            let (transport, relay_events) = factory.connect().await.expect("native relay connect");

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
                    events: ev_tx,
                    rekey: None,
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

            // Tear the call down: aborting the driver drops the transport, the relay reads EOF and its
            // task ends. Join it so a stuck relay surfaces here rather than leaking past the test.
            drop(mic_tx);
            driver.abort();
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
