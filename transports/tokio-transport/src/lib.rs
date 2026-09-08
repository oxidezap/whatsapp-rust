//! Tokio WebSocket transport for whatsapp-rust.
//!
//! For custom connections, use [`from_websocket`].

use async_trait::async_trait;
use bytes::Bytes;
use futures_util::stream::{SplitSink, SplitStream};
use futures_util::{SinkExt, StreamExt};
use log::{debug, warn};
use std::sync::{Arc, Once};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::Mutex;
use tokio_websockets::{ClientBuilder, Message, WebSocketStream};
use wacore::net::{
    DisconnectReason, Transport, TransportEvent, TransportFactory, WHATSAPP_WEB_ORIGIN,
    WHATSAPP_WEB_WS_URL,
};

pub use tokio_websockets::Connector;

const EVENT_CHANNEL_CAPACITY: usize = 64;

// Best-effort per-session footprint estimates for `Transport::resource_report`.
// tokio-websockets and rustls don't expose their live buffer sizes, so these
// are documented static estimates of steady-state cost, not measurements: a
// WebSocket read + write framing buffer, plus rustls record buffers and key
// schedule for one TLS session. They exist to give a consumer a realistic
// order-of-magnitude for the transport's ~tens-of-KiB-per-session contribution.
const EST_READ_BUFFER_BYTES: u64 = 16 * 1024;
const EST_WRITE_BUFFER_BYTES: u64 = 16 * 1024;
const EST_TLS_STATE_BYTES: u64 = 32 * 1024;

/// The static per-session footprint estimate reported by every WebSocket
/// transport. Factored out so its numbers are unit-testable without a live
/// socket.
fn transport_resource_estimate() -> wacore::stats::TransportResourceReport {
    wacore::stats::TransportResourceReport {
        read_buffer_bytes: Some(EST_READ_BUFFER_BYTES),
        write_buffer_bytes: Some(EST_WRITE_BUFFER_BYTES),
        tls_state_bytes: Some(EST_TLS_STATE_BYTES),
    }
}

static CRYPTO_PROVIDER_INIT: Once = Once::new();

/// rustls 0.23.43 divides this budget by eight to size its server-name queue,
/// then evicts when the queue reaches capacity. Two slots retain one name;
/// one slot immediately evicts it. The per-server TLS 1.3 ticket limit stays eight.
const RESUMPTION_TICKETS: usize = 16;

/// Applies the single-host resumption sizing to a freshly built config.
fn size_for_one_host(mut config: rustls::ClientConfig) -> rustls::ClientConfig {
    config.resumption = rustls::client::Resumption::in_memory_sessions(RESUMPTION_TICKETS);
    config
}

/// Returns the default TLS connector used by [`TokioWebSocketTransportFactory`].
///
/// Useful as a starting point when users need to inspect or replicate the
/// default TLS configuration before customizing it via [`TokioWebSocketTransportFactory::with_connector`].
///
/// Its session-resumption store is sized for the one host a factory dials,
/// rather than the many rustls provisions for by default. Reused across several
/// hosts it still works, but only the most recent ones keep their tickets; size
/// it back up if that is the shape you need.
///
/// On first call, installs `ring` as the global rustls crypto provider
/// (no-op if one is already installed).
pub fn default_tls_connector() -> Connector {
    CRYPTO_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    #[cfg(feature = "danger-skip-tls-verify")]
    {
        use std::sync::Arc as StdArc;
        use tokio_rustls::TlsConnector;

        warn!("TLS certificate verification is DISABLED");

        #[derive(Debug)]
        struct NoVerifier;

        impl rustls::client::danger::ServerCertVerifier for NoVerifier {
            fn verify_server_cert(
                &self,
                _end_entity: &rustls::pki_types::CertificateDer<'_>,
                _intermediates: &[rustls::pki_types::CertificateDer<'_>],
                _server_name: &rustls::pki_types::ServerName<'_>,
                _ocsp_response: &[u8],
                _now: rustls::pki_types::UnixTime,
            ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
                Ok(rustls::client::danger::ServerCertVerified::assertion())
            }

            fn verify_tls12_signature(
                &self,
                _message: &[u8],
                _cert: &rustls::pki_types::CertificateDer<'_>,
                _dss: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
            {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }

            fn verify_tls13_signature(
                &self,
                _message: &[u8],
                _cert: &rustls::pki_types::CertificateDer<'_>,
                _dss: &rustls::DigitallySignedStruct,
            ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error>
            {
                Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
            }

            fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
                vec![
                    rustls::SignatureScheme::RSA_PKCS1_SHA256,
                    rustls::SignatureScheme::RSA_PKCS1_SHA384,
                    rustls::SignatureScheme::RSA_PKCS1_SHA512,
                    rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
                    rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
                    rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
                    rustls::SignatureScheme::RSA_PSS_SHA256,
                    rustls::SignatureScheme::RSA_PSS_SHA384,
                    rustls::SignatureScheme::RSA_PSS_SHA512,
                    rustls::SignatureScheme::ED25519,
                ]
            }
        }

        let config = size_for_one_host(
            rustls::ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(StdArc::new(NoVerifier))
                .with_no_client_auth(),
        );

        Connector::Rustls(TlsConnector::from(StdArc::new(config)))
    }

    #[cfg(not(feature = "danger-skip-tls-verify"))]
    {
        use std::sync::Arc as StdArc;
        use tokio_rustls::TlsConnector;

        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = size_for_one_host(
            rustls::ClientConfig::builder()
                .with_root_certificates(root_store)
                .with_no_client_auth(),
        );

        Connector::Rustls(TlsConnector::from(StdArc::new(config)))
    }
}

type Sink<S> = SplitSink<WebSocketStream<S>, Message>;

struct WsTransport<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> {
    sink: Arc<Mutex<Option<Sink<S>>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> WsTransport<S> {
    fn new(sink: Sink<S>, shutdown_tx: tokio::sync::watch::Sender<bool>) -> Self {
        Self {
            sink: Arc::new(Mutex::new(Some(sink))),
            shutdown_tx,
        }
    }
}

#[async_trait]
impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> Transport for WsTransport<S> {
    async fn send(&self, data: Bytes) -> Result<(), anyhow::Error> {
        let mut guard = self.sink.lock().await;
        let sink = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Socket is closed"))?;
        debug!("--> Sending {} bytes", data.len());
        sink.send(Message::binary(data))
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket send error: {e}"))?;
        Ok(())
    }

    async fn disconnect(&self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(mut sink) = self.sink.lock().await.take() {
            let _ = sink
                .send(Message::close(
                    Some(tokio_websockets::CloseCode::NORMAL_CLOSURE),
                    "",
                ))
                .await;
        }
    }

    fn resource_report(&self) -> Option<wacore::stats::TransportResourceReport> {
        // Static best-effort estimates (see the constants): tokio-websockets and
        // rustls don't surface their live buffer sizes.
        Some(transport_resource_estimate())
    }
}

/// Reads at or above this size are moved out of the WebSocket codec's buffer
/// into their own allocation before handoff.
///
/// tokio-websockets cuts each message out of its long-lived read buffer
/// (`BytesMut::split_to`, which always promotes the buffer to shared), so the
/// `Bytes` inside a message is a shared view into it: `FrameDecoder::feed_owned`
/// could never adopt such a read (`Bytes::try_into_mut` fails while the codec
/// holds the other reference) and would copy every large frame anyway, while
/// the view kept the codec's whole read buffer — and any frames coalesced into
/// it — alive until the read loop got around to that copy. Moving a large read
/// into its own `Vec` hands the decoder a uniquely-owned buffer it adopts with
/// no further copy, and drops the shared view here so the codec reuses its
/// buffer for the next read.
///
/// Reads below this size keep the zero-copy shared handoff: the decoder copies
/// those into its amortized chunk buffer regardless, so a `Vec` here would add
/// an allocation and a copy per small stanza.
///
/// Must match `CHUNK_SIZE` in `wacore-noise`'s framing module, the size at and
/// above which `feed_owned` adopts. Drift in either direction stays correct (a
/// fallback copy), only less optimal.
const ADOPT_THRESHOLD: usize = 1024;

/// Moves a received message's payload into the `Bytes` handed to the read loop.
///
/// Large reads get their own allocation (adoptable by `feed_owned`); small
/// reads keep the zero-copy shared view (copied into the decoder's chunk
/// buffer regardless). Rationale on [`ADOPT_THRESHOLD`].
fn handoff_bytes(payload: tokio_websockets::Payload) -> Bytes {
    if payload.len() >= ADOPT_THRESHOLD {
        Bytes::from(payload.to_vec())
    } else {
        Bytes::from(payload)
    }
}

async fn read_pump<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    mut stream: SplitStream<WebSocketStream<S>>,
    tx: async_channel::Sender<TransportEvent>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    // Default covers the shutdown-initiated breaks (our own disconnect, where
    // the client already knows the cause); the receive arms overwrite it with
    // the real reason so a clean server recycle is distinguishable from an
    // abrupt EOF or a read error in the logs.
    let mut reason = DisconnectReason::Unknown;
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            next = stream.next() => match next {
                Some(Ok(msg)) if msg.is_binary() => {
                    let payload = msg.into_payload();
                    debug!("<-- Received WebSocket data: {} bytes", payload.len());
                    let bytes = handoff_bytes(payload);
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        r = tx.send(TransportEvent::DataReceived(bytes)) => {
                            if r.is_err() {
                                warn!("Event receiver dropped");
                                break;
                            }
                        }
                    }
                }
                Some(Ok(msg)) if msg.is_close() => {
                    reason = match msg.as_close() {
                        Some((code, text)) => DisconnectReason::ServerClose {
                            code: Some(u16::from(code)),
                            reason: text.to_owned(),
                        },
                        None => DisconnectReason::ServerClose {
                            code: None,
                            reason: String::new(),
                        },
                    };
                    debug!("Received close frame: {reason}");
                    break;
                }
                Some(Ok(_)) => {} // ping/pong/text handled by tokio-websockets
                Some(Err(e)) => {
                    reason = DisconnectReason::ReadError(e.to_string());
                    warn!("WebSocket read error: {e}");
                    break;
                }
                None => {
                    reason = DisconnectReason::StreamEnded;
                    debug!("WebSocket stream ended");
                    break;
                }
            },
        }
    }

    let _ = tx.send(TransportEvent::Disconnected(reason)).await;
}

/// Wraps an already-upgraded [`WebSocketStream`] into a [`Transport`] + event channel.
///
/// Useful for custom connection strategies (e.g. IPv4 preference, TCP keepalive).
pub fn from_websocket<S>(
    ws: WebSocketStream<S>,
) -> (Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (sink, stream) = ws.split();
    let (event_tx, event_rx) = async_channel::bounded(EVENT_CHANNEL_CAPACITY);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let transport = Arc::new(WsTransport::new(sink, shutdown_tx));

    // Enqueue Connected before spawning so it precedes any DataReceived.
    let _ = event_tx.try_send(TransportEvent::Connected);

    tokio::task::spawn(read_pump(stream, event_tx, shutdown_rx));

    (transport, event_rx)
}

/// Default [`TransportFactory`] using system DNS, TCP, and TLS.
///
/// For custom connection logic, use [`from_websocket`] directly.
pub struct TokioWebSocketTransportFactory {
    url: String,
    connector: Option<Connector>,
    /// Built on the first dial and kept. Rebuilding it per connection cost a
    /// fresh TLS config every reconnect and left the resumption store inside
    /// it permanently empty, so resumption could never fire.
    default_connector: std::sync::OnceLock<Connector>,
    origin: Option<String>,
}

impl TokioWebSocketTransportFactory {
    pub fn new() -> Self {
        Self {
            url: WHATSAPP_WEB_WS_URL.to_string(),
            connector: None,
            default_connector: std::sync::OnceLock::new(),
            origin: Some(WHATSAPP_WEB_ORIGIN.to_string()),
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Send a different `Origin` on the upgrade request.
    ///
    /// The default is [`WHATSAPP_WEB_ORIGIN`], and it stays correct when
    /// [`with_url`](Self::with_url) points at a relay or a mock — the origin
    /// names the endpoint the peer is standing in for, not the host dialled.
    /// Reach for this only when something in front of WhatsApp demands its own.
    pub fn with_origin(mut self, origin: impl Into<String>) -> Self {
        self.origin = Some(origin.into());
        self
    }

    /// Open the socket with no `Origin` at all, as this crate did before the
    /// header was added. For a peer that rejects the upgrade over it; no known
    /// WhatsApp endpoint does.
    pub fn without_origin(mut self) -> Self {
        self.origin = None;
        self
    }

    /// Use a custom TLS [`Connector`] instead of the built-in default.
    ///
    /// This is the primary extension point for custom TLS configuration
    /// (e.g. custom CA certificates, client certs). For full proxy support,
    /// implement [`TransportFactory`] directly and use [`from_websocket`].
    ///
    /// Repeated dials on one factory retain its default connector. Separate
    /// factories build separate defaults. To share TLS configuration and session
    /// storage across factories, clone the rustls payload, not `Connector`:
    ///
    /// ```
    /// use whatsapp_rust_tokio_transport::{
    ///     Connector, TokioWebSocketTransportFactory, default_tls_connector,
    /// };
    ///
    /// # fn main() -> anyhow::Result<()> {
    /// let Connector::Rustls(tls) = default_tls_connector() else {
    ///     anyhow::bail!("expected the default rustls connector");
    /// };
    /// let primary = TokioWebSocketTransportFactory::new()
    ///     .with_connector(Connector::Rustls(tls.clone()));
    /// let secondary = TokioWebSocketTransportFactory::new()
    ///     .with_url("wss://web.whatsapp.com:5222/ws/chat")
    ///     .with_connector(Connector::Rustls(tls.clone()));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// The clones share an `Arc<ClientConfig>`, not an open socket. Share only
    /// within the intended trust, client-identity, and tenant policy. The default
    /// store holds tickets for one server name; supply your own configuration
    /// for a larger multi-host cache. Changing a port or ED query does not change
    /// the server-name key, but resumption still requires processed tickets and
    /// server acceptance. Certificate validation, SNI, and Origin are unchanged.
    pub fn with_connector(mut self, connector: Connector) -> Self {
        self.connector = Some(connector);
        self
    }
}

impl Default for TokioWebSocketTransportFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TransportFactory for TokioWebSocketTransportFactory {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error> {
        let uri: http::Uri = self
            .url
            .parse()
            .map_err(|e| anyhow::anyhow!("Failed to parse URL: {e}"))?;

        let connector = match &self.connector {
            Some(c) => c,
            None => self.default_connector.get_or_init(default_tls_connector),
        };

        let mut builder = ClientBuilder::from_uri(uri).connector(connector);
        if let Some(origin) = &self.origin {
            let value = http::HeaderValue::from_str(origin)
                .map_err(|e| anyhow::anyhow!("Invalid Origin {origin:?}: {e}"))?;
            builder = builder
                .add_header(http::header::ORIGIN, value)
                .map_err(|e| anyhow::anyhow!("Failed to set Origin header: {e}"))?;
        }

        debug!("Dialing WebSocket");
        let (ws, _) = builder
            .connect()
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket connect failed: {e}"))?;

        Ok(from_websocket(ws))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A large read must reach the frame decoder as a uniquely-owned `Bytes`
    /// or `feed_owned` cannot adopt it. The production input is always shared
    /// (the codec cuts messages out of its read buffer via `split_to`), so
    /// this feeds the handoff a clone-held payload — the shape that must still
    /// come out unique — and asserts adoption-ability by pointer semantics:
    /// `try_into_mut` succeeds only for a single owner.
    #[test]
    fn large_reads_hand_over_uniquely_owned_bytes() {
        let backing = Bytes::from(vec![0xA5u8; ADOPT_THRESHOLD * 4]);
        // `clone` keeps a second owner alive, as the codec's read buffer does.
        let shared = tokio_websockets::Payload::from(backing.clone());
        assert!(
            backing.clone().try_into_mut().is_err(),
            "test setup must be shared to mean anything"
        );

        let bytes = handoff_bytes(shared);
        assert_eq!(&bytes[..], &backing[..]);
        assert!(
            bytes.try_into_mut().is_ok(),
            "a large read must be adoptable by feed_owned"
        );
    }

    /// Small reads keep the shared handoff: the decoder copies them into its
    /// chunk buffer either way. Pinned both ways: intact bytes, and still
    /// shared — a regression to a per-read copy would pass the first assert
    /// while breaking the zero-copy handoff this documents.
    #[test]
    fn small_reads_hand_over_shared_bytes_intact() {
        let backing = Bytes::from(vec![0x5Au8; ADOPT_THRESHOLD / 4]);
        // `clone` keeps a second owner alive, as the codec's read buffer does.
        let bytes = handoff_bytes(tokio_websockets::Payload::from(backing.clone()));
        assert_eq!(&bytes[..], &backing[..]);
        assert!(
            bytes.try_into_mut().is_err(),
            "small reads must preserve the shared handoff"
        );
    }

    /// Workstream C: the transport's reported footprint is the sum of its
    /// read/write framing buffers and the TLS state estimate.
    #[test]
    fn resource_estimate_totals_present_fields() {
        let report = transport_resource_estimate();
        assert_eq!(report.read_buffer_bytes, Some(EST_READ_BUFFER_BYTES));
        assert_eq!(report.write_buffer_bytes, Some(EST_WRITE_BUFFER_BYTES));
        assert_eq!(report.tls_state_bytes, Some(EST_TLS_STATE_BYTES));
        assert_eq!(
            report.total_bytes(),
            EST_READ_BUFFER_BYTES + EST_WRITE_BUFFER_BYTES + EST_TLS_STATE_BYTES
        );
    }

    async fn attempt_dial(factory: &mut TokioWebSocketTransportFactory) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            factory.url = format!("ws://{}/ws/chat", listener.local_addr().unwrap());
            let server = async { drop(listener.accept().await.unwrap()) };
            let client = async { assert!(factory.create_transport().await.is_err()) };
            tokio::join!(server, client);
        })
        .await
        .expect("local dial timed out");
    }

    #[tokio::test]
    async fn the_default_connector_is_retained_across_dials() {
        let mut factory = TokioWebSocketTransportFactory::new();
        assert!(factory.default_connector.get().is_none());

        attempt_dial(&mut factory).await;
        let config = rustls_config(factory.default_connector.get().unwrap()).clone();

        attempt_dial(&mut factory).await;
        assert!(Arc::ptr_eq(
            &config,
            rustls_config(factory.default_connector.get().unwrap()),
        ));

        let mut other = TokioWebSocketTransportFactory::new();
        attempt_dial(&mut other).await;
        assert!(!Arc::ptr_eq(
            &config,
            rustls_config(other.default_connector.get().unwrap()),
        ));
    }

    fn rustls_config(connector: &Connector) -> &Arc<rustls::ClientConfig> {
        match connector {
            Connector::Rustls(tls) => tls.config(),
            _ => panic!("expected rustls"),
        }
    }

    #[test]
    fn single_host_session_cache_retains_state() {
        use rustls::client::ClientSessionStore;
        let cache = rustls::client::ClientSessionMemoryCache::new(RESUMPTION_TICKETS);
        let name = rustls::pki_types::ServerName::try_from("localhost").unwrap();
        cache.set_kx_hint(name.clone(), rustls::NamedGroup::X25519);
        assert_eq!(cache.kx_hint(&name), Some(rustls::NamedGroup::X25519));
    }

    #[derive(Debug)]
    struct TicketStore {
        cache: rustls::client::ClientSessionMemoryCache,
        received: tokio::sync::Semaphore,
    }

    impl TicketStore {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                cache: rustls::client::ClientSessionMemoryCache::new(RESUMPTION_TICKETS),
                received: tokio::sync::Semaphore::new(0),
            })
        }
    }

    impl rustls::client::ClientSessionStore for TicketStore {
        fn set_kx_hint(
            &self,
            name: rustls::pki_types::ServerName<'static>,
            group: rustls::NamedGroup,
        ) {
            self.cache.set_kx_hint(name, group);
        }

        fn kx_hint(&self, name: &rustls::pki_types::ServerName<'_>) -> Option<rustls::NamedGroup> {
            self.cache.kx_hint(name)
        }

        fn set_tls12_session(
            &self,
            name: rustls::pki_types::ServerName<'static>,
            value: rustls::client::Tls12ClientSessionValue,
        ) {
            self.cache.set_tls12_session(name, value);
        }

        fn tls12_session(
            &self,
            name: &rustls::pki_types::ServerName<'_>,
        ) -> Option<rustls::client::Tls12ClientSessionValue> {
            self.cache.tls12_session(name)
        }

        fn remove_tls12_session(&self, name: &rustls::pki_types::ServerName<'static>) {
            self.cache.remove_tls12_session(name);
        }

        fn insert_tls13_ticket(
            &self,
            name: rustls::pki_types::ServerName<'static>,
            value: rustls::client::Tls13ClientSessionValue,
        ) {
            self.cache.insert_tls13_ticket(name, value);
            self.received.add_permits(1);
        }

        fn take_tls13_ticket(
            &self,
            name: &rustls::pki_types::ServerName<'static>,
        ) -> Option<rustls::client::Tls13ClientSessionValue> {
            self.cache.take_tls13_ticket(name)
        }
    }

    fn tls_server_config(
        name: &str,
    ) -> (
        Arc<rustls::ServerConfig>,
        rustls::pki_types::CertificateDer<'static>,
    ) {
        let rcgen::CertifiedKey { cert, signing_key } =
            rcgen::generate_simple_self_signed(vec![name.into()]).unwrap();
        let mut config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            rustls::pki_types::PrivatePkcs8KeyDer::from(signing_key.serialize_der()).into(),
        )
        .unwrap();
        config.send_tls13_tickets = 1;
        (Arc::new(config), cert.der().clone())
    }

    fn trusted_connector(
        cert: rustls::pki_types::CertificateDer<'static>,
        store: &Arc<TicketStore>,
    ) -> tokio_rustls::TlsConnector {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert).unwrap();
        let mut config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        config.resumption = rustls::client::Resumption::store(store.clone());
        tokio_rustls::TlsConnector::from(Arc::new(config))
    }

    async fn tls_dial(
        listener: &tokio::net::TcpListener,
        server_config: &Arc<rustls::ServerConfig>,
        factory: &TokioWebSocketTransportFactory,
        store: &TicketStore,
        expected_target: &str,
    ) -> rustls::HandshakeKind {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let server = async {
                let (tcp, _) = listener.accept().await.unwrap();
                let tls = tokio_rustls::TlsAcceptor::from(server_config.clone())
                    .accept(tcp)
                    .await
                    .unwrap();
                assert_eq!(tls.get_ref().1.server_name(), None);
                let kind = tls.get_ref().1.handshake_kind().unwrap();
                let (request, mut ws) = tokio_websockets::ServerBuilder::new()
                    .accept(tls)
                    .await
                    .unwrap();
                assert_eq!(
                    request.uri().path_and_query().unwrap().as_str(),
                    expected_target
                );
                assert_eq!(request.headers()[http::header::ORIGIN], WHATSAPP_WEB_ORIGIN);
                assert!(ws.next().await.unwrap().unwrap().is_close());
                kind
            };
            let client = async {
                let (transport, events) = factory.create_transport().await.unwrap();
                assert!(matches!(
                    events.recv().await.unwrap(),
                    TransportEvent::Connected
                ));
                // A completed handshake alone does not mean the client has read its ticket.
                store.received.acquire().await.unwrap().forget();
                transport.disconnect().await;
                assert!(matches!(
                    events.recv().await.unwrap(),
                    TransportEvent::Disconnected(_)
                ));
            };
            let (kind, ()) = tokio::join!(server, client);
            kind
        })
        .await
        .expect("local TLS dial timed out")
    }

    #[tokio::test]
    async fn shared_factories_retain_tls_sessions_across_ed_and_ports() {
        tokio::time::timeout(std::time::Duration::from_secs(10), async {
            let (server_config, cert) = tls_server_config("127.0.0.1");
            let first = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let second = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let store = TicketStore::new();
            let tls = trusted_connector(cert.clone(), &store);
            let make_factory = |port, target: &str, tls: tokio_rustls::TlsConnector| {
                TokioWebSocketTransportFactory::new()
                    .with_url(format!("wss://127.0.0.1:{port}{target}"))
                    .with_connector(Connector::Rustls(tls))
            };
            let a = make_factory(
                first.local_addr().unwrap().port(),
                "/ws/chat?ED=AQ==",
                tls.clone(),
            );
            let b = make_factory(
                second.local_addr().unwrap().port(),
                "/ws/chat?ED=Ag==",
                tls.clone(),
            );
            assert!(Arc::ptr_eq(
                rustls_config(a.connector.as_ref().unwrap()),
                rustls_config(b.connector.as_ref().unwrap())
            ));
            assert_eq!(
                tls_dial(&first, &server_config, &a, &store, "/ws/chat?ED=AQ==").await,
                rustls::HandshakeKind::Full
            );
            assert_eq!(
                tls_dial(&first, &server_config, &a, &store, "/ws/chat?ED=AQ==").await,
                rustls::HandshakeKind::Resumed
            );
            drop(a);
            assert_eq!(
                tls_dial(&second, &server_config, &b, &store, "/ws/chat?ED=Ag==").await,
                rustls::HandshakeKind::Resumed
            );
            assert!(b.default_connector.get().is_none());

            let isolated_store = TicketStore::new();
            let isolated_tls = trusted_connector(cert, &isolated_store);
            assert!(!Arc::ptr_eq(tls.config(), isolated_tls.config()));
            let isolated = make_factory(
                second.local_addr().unwrap().port(),
                "/ws/chat?ED=Aw==",
                isolated_tls,
            );
            assert_eq!(
                tls_dial(
                    &second,
                    &server_config,
                    &isolated,
                    &isolated_store,
                    "/ws/chat?ED=Aw=="
                )
                .await,
                rustls::HandshakeKind::Full
            );
            assert_eq!(
                tls_dial(&second, &server_config, &b, &store, "/ws/chat?ED=Ag==").await,
                rustls::HandshakeKind::Resumed
            );
        })
        .await
        .expect("shared TLS factory test timed out");
    }

    #[tokio::test]
    async fn tls_rejects_untrusted_certificates_and_wrong_server_names() {
        let (server_config, cert) = tls_server_config("localhost");
        let unrelated_key = rcgen::KeyPair::generate().unwrap();
        let mut params = rcgen::CertificateParams::new(vec!["unrelated.invalid".into()]).unwrap();
        params
            .distinguished_name
            .push(rcgen::DnType::CommonName, "Unrelated test root");
        let unrelated_cert = params.self_signed(&unrelated_key).unwrap().der().clone();
        for (root, expected_error) in [
            (unrelated_cert, "UnknownIssuer"),
            (cert, "certificate not valid for name"),
        ] {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let factory = TokioWebSocketTransportFactory::new()
                    .with_url(format!(
                        "wss://127.0.0.1:{}/ws/chat",
                        listener.local_addr().unwrap().port()
                    ))
                    .with_connector(Connector::Rustls(trusted_connector(
                        root,
                        &TicketStore::new(),
                    )));
                let server = async {
                    let (tcp, _) = listener.accept().await.unwrap();
                    assert!(
                        tokio_rustls::TlsAcceptor::from(server_config.clone())
                            .accept(tcp)
                            .await
                            .is_err()
                    );
                };
                let client = async {
                    let error = match factory.create_transport().await {
                        Ok(_) => panic!("invalid TLS peer was accepted"),
                        Err(error) => error,
                    };
                    assert!(error.to_string().contains(expected_error), "{error}");
                };
                tokio::join!(server, client);
            })
            .await
            .expect("local TLS rejection timed out");
        }
    }

    #[tokio::test]
    async fn direct_rustls_connector_sends_dns_sni_and_checks_hostname() {
        let (server_config, cert) = tls_server_config("localhost");
        let connector = trusted_connector(cert, &TicketStore::new());
        assert!(connector.config().enable_sni);
        for name in ["localhost", "wrong.invalid"] {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                // DNS names go only to rustls, never to the socket resolver or factory.
                let server = async {
                    let (tcp, _) = listener.accept().await.unwrap();
                    tokio_rustls::TlsAcceptor::from(server_config.clone())
                        .accept(tcp)
                        .await
                };
                let client = async {
                    let tcp = tokio::net::TcpStream::connect(addr).await.unwrap();
                    connector
                        .connect(rustls::pki_types::ServerName::try_from(name).unwrap(), tcp)
                        .await
                };
                let (server, client) = tokio::join!(server, client);
                if name == "localhost" {
                    assert_eq!(server.unwrap().get_ref().1.server_name(), Some(name));
                    assert!(client.is_ok());
                } else {
                    assert!(server.is_err());
                    let error = match client {
                        Ok(_) => panic!("wrong DNS name was accepted"),
                        Err(error) => error,
                    };
                    assert!(
                        error.to_string().contains("certificate not valid for name"),
                        "{error}"
                    );
                }
            })
            .await
            .expect("direct rustls DNS-name test timed out");
        }
    }

    /// The retained default must stay unbuilt when the caller supplied one:
    /// building it anyway would pay for a TLS config nothing ever dials with.
    #[tokio::test]
    async fn a_custom_connector_leaves_the_default_unbuilt() {
        let mut factory =
            TokioWebSocketTransportFactory::new().with_connector(default_tls_connector());

        attempt_dial(&mut factory).await;

        assert!(
            factory.default_connector.get().is_none(),
            "a caller-supplied connector was bypassed"
        );
    }

    /// Runs one upgrade attempt against a throwaway listener and returns the
    /// request bytes the peer read.
    ///
    /// tokio-websockets exposes no view of the request it builds, so the socket
    /// is the only place the header is observable — which is also the only
    /// place it matters. `ws://` keeps the listener plain: `connect()` routes
    /// that scheme through `Connector::Plain` and ignores our TLS connector.
    /// The connect itself always fails, because the listener answers nothing.
    async fn captured_upgrade_request(
        factory: TokioWebSocketTransportFactory,
    ) -> Result<String, anyhow::Error> {
        use tokio::io::AsyncReadExt;

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        let server = tokio::task::spawn(async move {
            let (mut stream, _) = listener.accept().await?;
            let mut request = Vec::new();
            let mut buf = [0u8; 512];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut buf).await? {
                    0 => break,
                    n => request.extend_from_slice(&buf[..n]),
                }
            }
            Ok::<_, std::io::Error>(String::from_utf8_lossy(&request).into_owned())
        });

        let _ = factory
            .with_url(format!("ws://{addr}/ws/chat"))
            .create_transport()
            .await;

        Ok(server.await??)
    }

    #[tokio::test]
    async fn upgrade_carries_the_web_origin_by_default() -> Result<(), anyhow::Error> {
        let request = captured_upgrade_request(TokioWebSocketTransportFactory::new()).await?;

        assert!(
            request.contains(&format!("origin: {WHATSAPP_WEB_ORIGIN}\r\n")),
            "upgrade must carry the WA Web origin, got:\n{request}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn with_origin_replaces_the_default() -> Result<(), anyhow::Error> {
        let request = captured_upgrade_request(
            TokioWebSocketTransportFactory::new().with_origin("https://relay.example"),
        )
        .await?;

        assert!(
            request.contains("origin: https://relay.example\r\n"),
            "the override must reach the wire, got:\n{request}"
        );
        assert!(
            !request.contains(WHATSAPP_WEB_ORIGIN),
            "the default must not be sent alongside it, got:\n{request}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn without_origin_omits_the_header() -> Result<(), anyhow::Error> {
        let request =
            captured_upgrade_request(TokioWebSocketTransportFactory::new().without_origin())
                .await?;

        assert!(
            !request.to_ascii_lowercase().contains("origin:"),
            "opting out must send no origin at all, got:\n{request}"
        );
        Ok(())
    }
}
