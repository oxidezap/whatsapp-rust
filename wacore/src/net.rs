use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::sync::Arc;

/// Default WhatsApp Web websocket endpoint.
pub const WHATSAPP_WEB_WS_URL: &str = "wss://web.whatsapp.com/ws/chat";

/// Why the transport connection ended. Lets a benign server-initiated stream
/// recycle (a clean Close frame) be told apart from an abrupt EOF or a real
/// read error when diagnosing reconnect behavior.
#[derive(Debug, Clone)]
pub enum DisconnectReason {
    /// The peer sent a WebSocket Close frame. `code` is the RFC 6455 close
    /// code (1000 = normal closure); `reason` is the optional UTF-8 text.
    ServerClose { code: Option<u16>, reason: String },
    /// The stream ended (EOF) without a Close frame.
    StreamEnded,
    /// A transport-level read/IO error ended the connection.
    ReadError(String),
    /// The reason was not reported by this transport (e.g. local shutdown).
    Unknown,
}

impl std::fmt::Display for DisconnectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerClose { code, reason } => match (code, reason.is_empty()) {
                (Some(c), false) => write!(f, "server close frame (code {c}: {reason})"),
                (Some(c), true) => write!(f, "server close frame (code {c})"),
                (None, false) => write!(f, "server close frame ({reason})"),
                (None, true) => write!(f, "server close frame (no code)"),
            },
            Self::StreamEnded => write!(f, "stream ended (EOF)"),
            Self::ReadError(e) => write!(f, "read error: {e}"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl DisconnectReason {
    /// Whether this is a benign, server-initiated stream recycle (the normal
    /// WhatsApp reconnect path) rather than a transport-level error.
    ///
    /// Used only to pick a log level: a clean shutdown is logged quietly (the
    /// reconnect is routine), while everything else stays loud so a genuine
    /// transport failure is never hidden behind reconnect noise. Deliberately
    /// conservative — anything ambiguous returns `false` (stays loud): a read/IO
    /// error, an abnormal close code, or an unreported reason.
    pub fn is_clean_shutdown(&self) -> bool {
        match self {
            // EOF with no Close frame is how the WA server recycles a connection.
            Self::StreamEnded => true,
            // A Close frame with a normal / going-away / no code is graceful; any
            // other code (protocol/server error, restart, etc.) stays loud.
            Self::ServerClose { code, .. } => matches!(code, None | Some(1000) | Some(1001)),
            // A transport read/IO error is a real failure — never quiet.
            Self::ReadError(_) => false,
            // Unknown reason: stay loud, don't assume it was benign.
            Self::Unknown => false,
        }
    }
}

/// An event produced by the transport layer.
#[derive(Debug, Clone)]
pub enum TransportEvent {
    /// The transport has successfully connected.
    Connected,
    /// Raw data has been received from the server.
    DataReceived(Bytes),
    /// The connection was lost, with the reason if the transport reported one.
    Disconnected(DisconnectReason),
}

/// Represents an active network connection.
/// The transport is a dumb pipe for bytes with no knowledge of WhatsApp framing.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Transport: Send + Sync {
    /// Sends raw data to the server.
    async fn send(&self, data: Bytes) -> Result<(), anyhow::Error>;

    /// Closes the connection.
    async fn disconnect(&self);
}

/// A factory responsible for creating new transport instances.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait TransportFactory: Send + Sync {
    /// Creates a new transport and returns it, along with a stream of events.
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error>;
}

/// A simple structure to represent an HTTP request
#[derive(Debug, Clone)]
pub struct HttpRequest {
    pub url: String,
    pub method: String, // "GET" or "POST"
    pub headers: HashMap<String, String>,
    pub body: Option<Bytes>,
}

impl HttpRequest {
    pub fn get(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "GET".to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            method: "POST".to_string(),
            headers: HashMap::new(),
            body: None,
        }
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    pub fn with_body(mut self, body: impl Into<Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }
}

/// A simple structure for the HTTP response
#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status_code: u16,
    pub body: Vec<u8>,
}

impl HttpResponse {
    pub fn body_string(&self) -> Result<String> {
        Ok(String::from_utf8(self.body.clone())?)
    }
}

/// An HTTP response with a streaming body reader instead of a buffered `Vec<u8>`.
/// Used for large downloads where buffering the entire response would be wasteful.
pub struct StreamingHttpResponse {
    pub status_code: u16,
    pub body: Box<dyn std::io::Read + Send>,
}

/// A streaming request body: a reader whose total length is known up front, so
/// the client can send an exact `Content-Length` (WhatsApp's CDN rejects chunked
/// transfer-encoding on upload).
pub type UploadBody = Box<dyn std::io::Read + Send>;

/// Trait for executing HTTP requests in a runtime-agnostic way
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait HttpClient: Send + Sync {
    /// Executes a given HTTP request and returns the response.
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse>;

    /// Whether this client supports synchronous streaming downloads.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Synchronous streaming variant — returns a reader over the response body.
    /// Must be called from a blocking context.
    fn execute_streaming(&self, _request: HttpRequest) -> Result<StreamingHttpResponse> {
        Err(anyhow::anyhow!(
            "Streaming not supported by this HTTP client"
        ))
    }

    /// Whether this client can stream a request body from a reader (upload).
    fn supports_upload_streaming(&self) -> bool {
        false
    }

    /// Synchronous streaming upload: send `body` (exactly `content_length` bytes)
    /// as the request body. Implementations MUST set an explicit `Content-Length`
    /// rather than chunked transfer-encoding. Any body set on `request` is
    /// ignored. Must be called from a blocking context.
    fn execute_upload(
        &self,
        _request: HttpRequest,
        _body: UploadBody,
        _content_length: u64,
    ) -> Result<HttpResponse> {
        Err(anyhow::anyhow!(
            "Upload streaming not supported by this HTTP client"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::DisconnectReason;

    // Happy paths: benign server-initiated recycles must classify as clean so
    // their reconnect is logged quietly.
    #[test]
    fn clean_shutdowns_are_classified_clean() {
        assert!(DisconnectReason::StreamEnded.is_clean_shutdown());
        assert!(
            DisconnectReason::ServerClose {
                code: Some(1000),
                reason: String::new()
            }
            .is_clean_shutdown()
        );
        assert!(
            DisconnectReason::ServerClose {
                code: Some(1001),
                reason: "going away".to_string()
            }
            .is_clean_shutdown()
        );
        assert!(
            DisconnectReason::ServerClose {
                code: None,
                reason: String::new()
            }
            .is_clean_shutdown()
        );
    }

    // Bad paths: a real transport error, an abnormal close code, or an unreported
    // reason must NOT be classified clean — they have to stay loud so genuine
    // failures are never hidden behind reconnect noise.
    #[test]
    fn real_errors_are_never_classified_clean() {
        assert!(!DisconnectReason::ReadError("connection reset".to_string()).is_clean_shutdown());
        assert!(!DisconnectReason::Unknown.is_clean_shutdown());
        for code in [1002u16, 1006, 1011, 1012, 1013, 3000, 4000] {
            assert!(
                !DisconnectReason::ServerClose {
                    code: Some(code),
                    reason: String::new()
                }
                .is_clean_shutdown(),
                "close code {code} must not be treated as a clean shutdown"
            );
        }
    }
}
