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
    DisconnectReason, Transport, TransportEvent, TransportFactory, WHATSAPP_WEB_WS_URL,
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

/// Returns the default TLS connector used by [`TokioWebSocketTransportFactory`].
///
/// Useful as a starting point when users need to inspect or replicate the
/// default TLS configuration before customizing it via [`TokioWebSocketTransportFactory::with_connector`].
///
/// On first call, installs the pure-Rust RustCrypto provider as the global
/// rustls crypto provider (no-op if one is already installed).
pub fn default_tls_connector() -> Connector {
    CRYPTO_PROVIDER_INIT.call_once(|| {
        let _ = rustls_rustcrypto::provider().install_default();
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

        let config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(StdArc::new(NoVerifier))
            .with_no_client_auth();

        Connector::Rustls(TlsConnector::from(StdArc::new(config)))
    }

    #[cfg(not(feature = "danger-skip-tls-verify"))]
    {
        use std::sync::Arc as StdArc;
        use tokio_rustls::TlsConnector;

        let mut root_store = rustls::RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

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
    async fn send(&self, data: bytes::Bytes) -> Result<(), anyhow::Error> {
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
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        r = tx.send(TransportEvent::DataReceived(Bytes::from(payload))) => {
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
}

impl TokioWebSocketTransportFactory {
    pub fn new() -> Self {
        Self {
            url: WHATSAPP_WEB_WS_URL.to_string(),
            connector: None,
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.url = url.into();
        self
    }

    /// Use a custom TLS [`Connector`] instead of the built-in default.
    ///
    /// This is the primary extension point for custom TLS configuration
    /// (e.g. custom CA certificates, client certs). For full proxy support,
    /// implement [`TransportFactory`] directly and use [`from_websocket`].
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

        let default_connector;
        let connector = match &self.connector {
            Some(c) => c,
            None => {
                default_connector = default_tls_connector();
                &default_connector
            }
        };

        debug!("Dialing {}", self.url);
        let (ws, _) = ClientBuilder::from_uri(uri)
            .connector(connector)
            .connect()
            .await
            .map_err(|e| anyhow::anyhow!("WebSocket connect failed: {e}"))?;

        Ok(from_websocket(ws))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
