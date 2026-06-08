//! Experimental sockudo-ws WebSocket transport for whatsapp-rust.
//!
//! A drop-in alternative to the tokio-websockets transport, backed by the
//! [`sockudo-ws`](https://github.com/sockudo/sockudo-ws) crate, so the two can
//! be benchmarked head-to-head through the same [`Transport`]/[`TransportEvent`]
//! abstraction. This crate establishes TCP + TLS itself (tokio-rustls) and hands
//! the established stream to sockudo's HTTP/1.1 upgrade via
//! [`WebSocketClient::connect_raw`], then splits the stream into a write half
//! (driving [`Transport::send`]) and a read half (the event pump).
//!
//! It mirrors the tokio-transport structure deliberately so the only variable in
//! a benchmark is the WebSocket library.

use async_trait::async_trait;
use bytes::Bytes;
use log::{debug, warn};
use sockudo_ws::client::WebSocketClient;
use sockudo_ws::{Config, Http1, Message, SplitReader, SplitWriter, WebSocketStream};
use std::sync::{Arc, Once};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_rustls::TlsConnector;
use wacore::net::{
    DisconnectReason, Transport, TransportEvent, TransportFactory, WHATSAPP_WEB_WS_URL,
};

const EVENT_CHANNEL_CAPACITY: usize = 64;

static CRYPTO_PROVIDER_INIT: Once = Once::new();

/// Builds the tokio-rustls connector used to dial `wss://` endpoints. Matches the
/// tokio-transport TLS setup (rustls + ring, webpki roots) so TLS is not a
/// variable in the benchmark. Under `danger-skip-tls-verify` it installs a
/// no-op certificate verifier for the self-signed mock server.
fn tls_connector() -> TlsConnector {
    CRYPTO_PROVIDER_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });

    #[cfg(feature = "danger-skip-tls-verify")]
    let config = {
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

        rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth()
    };

    #[cfg(not(feature = "danger-skip-tls-verify"))]
    let config = {
        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };

    TlsConnector::from(Arc::new(config))
}

/// A parsed `ws://` / `wss://` endpoint. IPv6 literal authorities are not
/// supported (the benchmark and WhatsApp endpoints are host names or IPv4).
struct WsTarget {
    tls: bool,
    host: String,
    port: u16,
    path: String,
}

fn parse_ws_url(url: &str) -> anyhow::Result<WsTarget> {
    let (tls, rest) = if let Some(r) = url.strip_prefix("wss://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("ws://") {
        (false, r)
    } else {
        anyhow::bail!("URL must start with ws:// or wss://: {url}");
    };

    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    let path = if path.is_empty() { "/" } else { path };

    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|_| anyhow::anyhow!("invalid port in URL: {url}"))?,
        ),
        None => (authority.to_string(), if tls { 443 } else { 80 }),
    };
    if host.is_empty() {
        anyhow::bail!("empty host in URL: {url}");
    }

    Ok(WsTarget {
        tls,
        host,
        port,
        path: path.to_string(),
    })
}

struct SockudoTransport<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> {
    writer: Arc<Mutex<Option<SplitWriter<S>>>>,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

#[async_trait]
impl<S: AsyncRead + AsyncWrite + Unpin + Send + 'static> Transport for SockudoTransport<S> {
    async fn send(&self, data: Bytes) -> Result<(), anyhow::Error> {
        let mut guard = self.writer.lock().await;
        let writer = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Socket is closed"))?;
        debug!("--> Sending {} bytes", data.len());
        writer
            .send_binary(data)
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket send error: {e}"))?;
        Ok(())
    }

    async fn disconnect(&self) {
        let _ = self.shutdown_tx.send(true);
        if let Some(mut writer) = self.writer.lock().await.take() {
            let _ = writer.close(1000, "").await;
        }
    }
}

async fn read_pump<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    mut reader: SplitReader<S>,
    tx: async_channel::Sender<TransportEvent>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    // Default covers shutdown-initiated breaks (our own disconnect); the receive
    // arms overwrite it with the real reason so a clean server recycle is told
    // apart from an abrupt EOF or a read error in the logs. Matches tokio-transport.
    let mut reason = DisconnectReason::Unknown;
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            next = reader.next() => match next {
                Some(Ok(Message::Binary(payload))) => {
                    debug!("<-- Received WebSocket data: {} bytes", payload.len());
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        r = tx.send(TransportEvent::DataReceived(payload)) => {
                            if r.is_err() {
                                warn!("Event receiver dropped");
                                break;
                            }
                        }
                    }
                }
                Some(Ok(Message::Close(close))) => {
                    reason = match close {
                        Some(cr) => DisconnectReason::ServerClose {
                            code: Some(cr.code),
                            reason: cr.reason,
                        },
                        None => DisconnectReason::ServerClose {
                            code: None,
                            reason: String::new(),
                        },
                    };
                    debug!("Received close frame: {reason}");
                    break;
                }
                // text/ping/pong: sockudo answers pings via the reader->writer
                // control channel internally, so nothing to do here.
                Some(Ok(_)) => {}
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

/// How often the writer is flushed to drain queued control responses (pongs,
/// close acks) while no application data is being sent. Well under the seconds-
/// to-tens-of-seconds a peer waits for a pong before closing an idle connection.
const CONTROL_FLUSH_INTERVAL: Duration = Duration::from_secs(5);

/// Periodically flushes the split writer so control responses the reader queued
/// (pongs in particular) are actually written on otherwise-idle connections. The
/// hot send path already drains the queue on every `send`; this only matters when
/// there is no application traffic. Exits on shutdown, when the writer has been
/// taken by `disconnect`, or when a flush fails (the read pump reports the cause).
async fn control_flush_pump<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(
    writer: Arc<Mutex<Option<SplitWriter<S>>>>,
    mut shutdown: tokio::sync::watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            _ = tokio::time::sleep(CONTROL_FLUSH_INTERVAL) => {
                let mut guard = writer.lock().await;
                match guard.as_mut() {
                    Some(w) => {
                        if w.flush().await.is_err() {
                            break;
                        }
                    }
                    None => break,
                }
            }
        }
    }
}

/// Wraps an already-upgraded sockudo [`WebSocketStream`] into a [`Transport`] +
/// event channel. Splits into independent read/write halves (sockudo uses
/// `tokio::io::split` under the hood, no shared mutex on the socket).
fn from_websocket<S>(
    ws: WebSocketStream<S>,
) -> (Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)
where
    S: AsyncRead + AsyncWrite + Send + Unpin + 'static,
{
    let (reader, writer) = ws.split();
    let (event_tx, event_rx) = async_channel::bounded(EVENT_CHANNEL_CAPACITY);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let writer = Arc::new(Mutex::new(Some(writer)));
    let flush_shutdown = shutdown_tx.subscribe();

    let transport = Arc::new(SockudoTransport {
        writer: Arc::clone(&writer),
        shutdown_tx,
    });

    // Enqueue Connected before spawning so it precedes any DataReceived.
    let _ = event_tx.try_send(TransportEvent::Connected);

    // Drive the split writer's control queue. sockudo's SplitReader only QUEUES
    // pong/close responses (it never writes); the SplitWriter drains them on
    // send/flush. With no application traffic an idle connection would never flush
    // a queued pong, so the server would drop it. This task flushes the writer
    // periodically to answer pings even when nothing is being sent.
    tokio::task::spawn(control_flush_pump(Arc::clone(&writer), flush_shutdown));

    tokio::task::spawn(read_pump(reader, event_tx, shutdown_rx));

    (transport, event_rx)
}

/// [`TransportFactory`] backed by sockudo-ws. Dials with system DNS + TCP, then
/// TLS (tokio-rustls) for `wss://`, and performs the WebSocket upgrade through
/// sockudo's HTTP/1.1 client.
pub struct SockudoWebSocketTransportFactory {
    url: String,
}

impl SockudoWebSocketTransportFactory {
    pub fn new() -> Self {
        Self {
            url: WHATSAPP_WEB_WS_URL.to_string(),
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }
}

impl Default for SockudoWebSocketTransportFactory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TransportFactory for SockudoWebSocketTransportFactory {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error> {
        let target = parse_ws_url(&self.url)?;
        debug!("Dialing {} (sockudo-ws)", self.url);

        let tcp = TcpStream::connect((target.host.as_str(), target.port))
            .await
            .map_err(|e| anyhow::anyhow!("TCP connect failed: {e}"))?;
        // Low-latency default for an interactive WS client (matches the typical
        // tokio-websockets setup); avoids Nagle batching of small WA frames.
        let _ = tcp.set_nodelay(true);

        let client = WebSocketClient::<Http1>::new(Config::default());

        if target.tls {
            let connector = tls_connector();
            let server_name = rustls::pki_types::ServerName::try_from(target.host.clone())
                .map_err(|e| anyhow::anyhow!("invalid TLS server name {:?}: {e}", target.host))?;
            let tls = connector
                .connect(server_name, tcp)
                .await
                .map_err(|e| anyhow::anyhow!("TLS handshake failed: {e}"))?;
            let (ws, _handshake) = client
                .connect_raw(tls, &target.host, &target.path, None)
                .await
                .map_err(|e| anyhow::anyhow!("WebSocket upgrade failed: {e}"))?;
            Ok(from_websocket(ws))
        } else {
            let (ws, _handshake) = client
                .connect_raw(tcp, &target.host, &target.path, None)
                .await
                .map_err(|e| anyhow::anyhow!("WebSocket upgrade failed: {e}"))?;
            Ok(from_websocket(ws))
        }
    }
}
