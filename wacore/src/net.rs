use anyhow::Result;
use async_trait::async_trait;
use bytes::Bytes;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

/// Default WhatsApp Web websocket endpoint.
pub const WHATSAPP_WEB_WS_URL: &str = "wss://web.whatsapp.com/ws/chat";

/// Alternate-port WhatsApp Web websocket endpoint.
///
/// WA Web dials this concurrently with [`WHATSAPP_WEB_WS_URL`] (its
/// `openWebSocketsConcurrently` races `wss://web.whatsapp.com/ws/chat` against
/// `wss://web.whatsapp.com:5222/ws/chat` and keeps the first `onopen`). A
/// network that blocks 443 but not 5222, or vice versa, connects through the
/// survivor, where a single-URL dial fails outright.
pub const WHATSAPP_WEB_WS_URL_FALLBACK: &str = "wss://web.whatsapp.com:5222/ws/chat";

/// Both chat endpoints WA Web dials, primary first.
pub const WHATSAPP_WEB_WS_URLS: [&str; 2] = [WHATSAPP_WEB_WS_URL, WHATSAPP_WEB_WS_URL_FALLBACK];

/// Appends WA Web's `?ED=` edge-routing query parameter to a chat URL.
///
/// The value is the same `edge_routing_info` the handshake already sends as
/// the binary `ED` pre-intro, here base64url-encoded exactly like WA Web's
/// `encodeB64UrlSafe` (URL-safe alphabet, padding kept). Both dial URLs carry
/// it. Returns `url` unchanged when routing is absent, empty, or oversize, the
/// same omit rules as `build_handshake_header`: a missing query never fails a
/// handshake, the pre-intro still carries the bytes.
pub fn with_edge_routing_param(url: &str, edge_routing_info: Option<&[u8]>) -> String {
    let Some(info) = edge_routing_info else {
        return url.to_string();
    };
    if info.is_empty() || info.len() > wacore_noise::MAX_EDGE_ROUTING_LEN {
        return url.to_string();
    }
    use base64::Engine as _;
    let encoded = base64::engine::general_purpose::URL_SAFE.encode(info);
    // The query goes before any `#fragment`: fragment bytes never reach the
    // server, so an `ED` appended after `#` would lose the routing hint.
    let (base, fragment) = match url.split_once('#') {
        Some((base, fragment)) => (base, Some(fragment)),
        None => (url, None),
    };
    let sep = if base.contains('?') { '&' } else { '?' };
    match fragment {
        Some(fragment) => format!("{base}{sep}ED={encoded}#{fragment}"),
        None => format!("{base}{sep}ED={encoded}"),
    }
}

/// `Origin` sent to every WhatsApp Web endpoint — the chat socket and the media
/// hosts alike.
///
/// Constant rather than derived from [`ClientProfile`], because the origin
/// describes the endpoint and not the client: [`WHATSAPP_WEB_WS_URL`] is the web
/// companion endpoint whatever platform the `ClientPayload` claims, and the
/// native apps do not speak to it at all. Omitting it for a non-web profile
/// would make the connection more anomalous, not less. whatsmeow and Baileys
/// both send this value unconditionally.
///
/// [`ClientProfile`]: crate::client_profile::ClientProfile
pub const WHATSAPP_WEB_ORIGIN: &str = "https://web.whatsapp.com";

/// Why the transport connection ended. Lets a benign server-initiated stream
/// recycle (a clean Close frame) be told apart from an abrupt EOF or a real
/// read error when diagnosing reconnect behavior.
///
/// Serialize: carried by `events::Disconnected`, whose payload consumers forward
/// as JSON (webhooks, dashboards) — snake_case so the wire shape doesn't leak
/// Rust variant naming.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
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
pub trait Transport: crate::sync_marker::MaybeSendSync {
    /// Sends raw data to the server.
    async fn send(&self, data: Bytes) -> Result<(), anyhow::Error>;

    /// Closes the connection.
    async fn disconnect(&self);

    /// Best-effort per-session footprint of this transport: read/write framing
    /// buffers plus a TLS/noise session-state estimate. `None` by default;
    /// concrete transports (e.g. the Tokio WebSocket transport) fill in what
    /// they can. Not blanket-impl'd, so a defaulted method here is cleanly
    /// overridable — no `Backend`-style wrinkle.
    fn resource_report(&self) -> Option<crate::stats::TransportResourceReport> {
        None
    }
}

/// A factory responsible for creating new transport instances.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait TransportFactory: crate::sync_marker::MaybeSendSync {
    /// Creates a new transport and returns it, along with a stream of events.
    ///
    /// Dropping the returned future must abort the dial without leaking an
    /// open transport: [`RacingTransportFactory`] drops a still-dialling loser
    /// (aborting it) and closes an already-open one in background, so a
    /// half-open dial must leave no socket behind when dropped.
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error>;
}

/// Dials two endpoints concurrently and keeps the first success, mirroring WA
/// Web's `openWebSocketsConcurrently`: the winner is returned, a loser that
/// already opened is closed cleanly via [`Transport::disconnect`], a loser
/// still dialling is aborted by dropping it, and an individual failure is
/// suppressed while the other dial is still in flight. Only when both fail
/// does this return an error (the one that completed last, as WA Web rejects
/// with the failure that completes the set).
///
/// The winner never waits for the loser: [`Transport::disconnect`] carries no
/// completion bound (a valid transport may stall it forever), so an
/// already-open loser is closed on `runtime` via [`Runtime::spawn_detached`]
/// while the winner is returned immediately. The background close is bounded,
/// so a stalled loser is dropped and its socket released instead of lingering
/// detached. Its event channel is dropped with it, so the loser's close never
/// surfaces as a `DisconnectReason`.
///
/// No handshake runs here, so at most one socket ever reaches Noise: this
/// returns a single transport and the caller handshakes exactly it. The race
/// itself uses only `futures` combinators, so this stays portable
/// (wasm32/ESP32); dropping the race future aborts both dials with no socket
/// to close.
///
/// Inner factories keep their own contracts: TLS session resumption stays
/// inside each factory's connector, `Origin` stays each factory's, and custom
/// strategies built on `from_websocket` compose by wrapping the factories.
///
/// [`Runtime::spawn_detached`]: crate::runtime::Runtime::spawn_detached
pub struct RacingTransportFactory {
    primary: Arc<dyn TransportFactory>,
    secondary: Arc<dyn TransportFactory>,
    runtime: Arc<dyn crate::runtime::Runtime>,
}

impl RacingTransportFactory {
    /// Races `primary` against `secondary`; the first success wins regardless
    /// of order, so pass the preferred endpoint first only as a tiebreak hint.
    /// For chat parity this is one factory per [`WHATSAPP_WEB_WS_URLS`] entry.
    pub fn new(
        primary: Arc<dyn TransportFactory>,
        secondary: Arc<dyn TransportFactory>,
        runtime: Arc<dyn crate::runtime::Runtime>,
    ) -> Self {
        Self {
            primary,
            secondary,
            runtime,
        }
    }
}

type TransportDial =
    Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error>;

/// Bound for the loser's background close.
///
/// A stalled close must still release its socket eventually, or repeated races
/// would accumulate detached tasks retaining dead transports. Well under the
/// 20s transport timeout, so the cleanup never overlaps the client's own
/// connect accounting.
const LOSER_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Settles a dial that lost the race: an already-open loser is closed in
/// background and its late failure suppressed; a still-pending loser is left
/// for the caller to drop, which aborts the dial per [`TransportFactory`]'s
/// contract. Never awaits the loser, so a stalled close cannot delay a usable
/// winner; the background close itself is bounded, so a stalled loser is
/// dropped and its socket released instead of lingering detached forever.
fn settle_loser<F>(
    winner: TransportDial,
    loser: F,
    runtime: &Arc<dyn crate::runtime::Runtime>,
) -> TransportDial
where
    F: Future<Output = TransportDial>,
{
    use futures::future::FutureExt as _;

    if winner.is_ok()
        && let Some(Ok((loser_transport, _))) = loser.now_or_never()
    {
        let spawner = Arc::clone(runtime);
        let timer = Arc::clone(runtime);
        spawner.spawn_detached(Box::pin(async move {
            let _ =
                crate::runtime::timeout(&*timer, LOSER_CLOSE_TIMEOUT, loser_transport.disconnect())
                    .await;
        }));
    }
    winner
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl TransportFactory for RacingTransportFactory {
    async fn create_transport(&self) -> TransportDial {
        use futures::future::{Either, select};
        use futures::pin_mut;

        let primary_fut = self.primary.create_transport();
        let secondary_fut = self.secondary.create_transport();
        pin_mut!(primary_fut, secondary_fut);

        match select(primary_fut, secondary_fut).await {
            Either::Left((first, second_fut)) => {
                if first.is_ok() {
                    settle_loser(first, second_fut, &self.runtime)
                } else {
                    second_fut.await
                }
            }
            Either::Right((second, first_fut)) => {
                if second.is_ok() {
                    settle_loser(second, first_fut, &self.runtime)
                } else {
                    first_fut.await
                }
            }
        }
    }
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

/// Trait for executing HTTP requests in a runtime-agnostic way.
///
/// **A completed exchange is `Ok`, whatever the status.** `Err` means the
/// exchange never happened — DNS, connect, TLS, timeout, a body that broke the
/// declared cap. Implementations MUST NOT map 4xx/5xx to an error: media
/// download and upload read `status_code` to tell a stale media-auth token
/// (401/403) and an expired URL (404/410) — both of which need a refreshed
/// media connection before the retry — apart from a host-level failure that
/// should simply move to the next CDN host. An implementation that hides the
/// status behind an opaque error makes every one of those retries repeat the
/// same dead auth token. Some HTTP crates default the other way: `ureq`'s
/// `http_status_as_error` is the known case.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait HttpClient: crate::sync_marker::MaybeSendSync {
    /// Executes a given HTTP request and returns the response, non-2xx included.
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

    /// Best-effort per-session footprint of this client: idle connection-pool
    /// buffers plus any in-flight download/media buffering the impl can see.
    /// `None` by default; `ureq`/`reqwest`-backed clients report what their
    /// (limited) introspection allows. Media downloads are a real transient-RAM
    /// source, so a coarse estimate is still worth reporting.
    fn resource_report(&self) -> Option<crate::stats::HttpResourceReport> {
        None
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

/// Race and `?ED=` tests. The race itself uses only `futures` combinators plus
/// a `Runtime` for the loser's background close, so it stays portable; the
/// tests below use Tokio timers purely as controllable latency/failure scripts
/// for the mock dials.
#[cfg(all(test, not(target_arch = "wasm32")))]
mod racing_tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    struct TestRuntime;

    #[async_trait::async_trait]
    impl crate::runtime::Runtime for TestRuntime {
        fn spawn(
            &self,
            future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> crate::runtime::AbortHandle {
            let handle = tokio::spawn(future);
            crate::runtime::AbortHandle::new(move || handle.abort())
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(tokio::time::sleep(duration))
        }

        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let _ = tokio::task::spawn_blocking(f).await;
            })
        }

        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    fn test_runtime() -> Arc<dyn crate::runtime::Runtime> {
        Arc::new(TestRuntime)
    }

    struct DialCounters {
        started: AtomicUsize,
        live: AtomicUsize,
        opened: AtomicUsize,
        disconnects: AtomicUsize,
        transports_live: AtomicUsize,
    }

    impl DialCounters {
        fn zero() -> Self {
            Self {
                started: AtomicUsize::new(0),
                live: AtomicUsize::new(0),
                opened: AtomicUsize::new(0),
                disconnects: AtomicUsize::new(0),
                transports_live: AtomicUsize::new(0),
            }
        }

        fn get(&self, f: impl Fn(&DialCounters) -> &AtomicUsize) -> usize {
            f(self).load(Ordering::Acquire)
        }
    }

    struct LiveGuard(Arc<DialCounters>);

    impl Drop for LiveGuard {
        fn drop(&mut self) {
            self.0.live.fetch_sub(1, Ordering::AcqRel);
        }
    }

    struct MockDialTransport {
        counters: Arc<DialCounters>,
        stall_disconnect: bool,
    }

    impl Drop for MockDialTransport {
        fn drop(&mut self) {
            self.counters.transports_live.fetch_sub(1, Ordering::AcqRel);
        }
    }

    #[async_trait::async_trait]
    impl Transport for MockDialTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn disconnect(&self) {
            // Mirrors `StallingMockTransport`: a valid transport may never
            // finish closing, which is exactly what must not block the winner.
            if self.stall_disconnect {
                std::future::pending::<()>().await;
            }
            self.counters.disconnects.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// A factory whose dial sleeps, then succeeds or fails on script. `None`
    /// delay answers immediately, which is what makes the simultaneous-open
    /// path deterministic.
    #[derive(Clone)]
    struct ScriptedDialFactory {
        delay: Option<Duration>,
        fail_with: Option<&'static str>,
        stall_disconnect: bool,
        counters: Arc<DialCounters>,
    }

    #[async_trait::async_trait]
    impl TransportFactory for ScriptedDialFactory {
        async fn create_transport(
            &self,
        ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error>
        {
            self.counters.started.fetch_add(1, Ordering::AcqRel);
            self.counters.live.fetch_add(1, Ordering::AcqRel);
            let _live = LiveGuard(self.counters.clone());
            if let Some(delay) = self.delay {
                tokio::time::sleep(delay).await;
            }
            if let Some(msg) = self.fail_with {
                return Err(anyhow::anyhow!("{msg}"));
            }
            self.counters.opened.fetch_add(1, Ordering::AcqRel);
            self.counters.transports_live.fetch_add(1, Ordering::AcqRel);
            let (_tx, rx) = async_channel::bounded(1);
            Ok((
                Arc::new(MockDialTransport {
                    counters: self.counters.clone(),
                    stall_disconnect: self.stall_disconnect,
                }),
                rx,
            ))
        }
    }

    fn scripted_full(
        delay_ms: Option<u64>,
        fail_with: Option<&'static str>,
        stall_disconnect: bool,
    ) -> (ScriptedDialFactory, Arc<DialCounters>) {
        let counters = Arc::new(DialCounters::zero());
        (
            ScriptedDialFactory {
                delay: delay_ms.map(Duration::from_millis),
                fail_with,
                stall_disconnect,
                counters: counters.clone(),
            },
            counters,
        )
    }

    fn scripted(
        delay_ms: Option<u64>,
        fail_with: Option<&'static str>,
    ) -> (ScriptedDialFactory, Arc<DialCounters>) {
        scripted_full(delay_ms, fail_with, false)
    }

    fn race(
        primary: ScriptedDialFactory,
        secondary: ScriptedDialFactory,
    ) -> RacingTransportFactory {
        RacingTransportFactory::new(Arc::new(primary), Arc::new(secondary), test_runtime())
    }

    /// Background loser closes resolve promptly, but not synchronously: wait
    /// up to the bound for `disconnects` to reach `want`.
    async fn await_disconnects(
        primary_c: &Arc<DialCounters>,
        secondary_c: &Arc<DialCounters>,
        want: usize,
    ) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while primary_c.get(|c| &c.disconnects) + secondary_c.get(|c| &c.disconnects) < want {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .expect("the background loser close runs to completion");
    }

    #[tokio::test]
    async fn fastest_success_wins_and_loser_dial_is_aborted() {
        let (primary, primary_c) = scripted(Some(100), None);
        let (secondary, secondary_c) = scripted(Some(5), None);

        let (_transport, _rx) = race(primary, secondary)
            .create_transport()
            .await
            .expect("the fast dial succeeds");

        assert_eq!(secondary_c.get(|c| &c.opened), 1);
        assert_eq!(
            primary_c.get(|c| &c.opened),
            0,
            "the slow dial never opened"
        );
        assert_eq!(
            primary_c.get(|c| &c.disconnects),
            0,
            "a dial that never opened has no socket to close"
        );
        assert_eq!(secondary_c.get(|c| &c.disconnects), 0);
        assert_eq!(primary_c.get(|c| &c.live), 0);
        assert_eq!(secondary_c.get(|c| &c.live), 0);
    }

    #[tokio::test]
    async fn simultaneous_success_closes_loser_cleanly() {
        let (primary, primary_c) = scripted(None, None);
        let (secondary, secondary_c) = scripted(None, None);

        let (_transport, _rx) = race(primary, secondary)
            .create_transport()
            .await
            .expect("one of the instant dials wins");

        assert_eq!(
            primary_c.get(|c| &c.opened) + secondary_c.get(|c| &c.opened),
            2,
            "both dials opened before either could be aborted"
        );
        // The loser close runs detached: the winner is already returned, so
        // wait for the background close instead of asserting synchronously.
        await_disconnects(&primary_c, &secondary_c, 1).await;
        let (p_disc, s_disc) = (
            primary_c.get(|c| &c.disconnects),
            secondary_c.get(|c| &c.disconnects),
        );
        assert!(
            (p_disc, s_disc) == (0, 1) || (p_disc, s_disc) == (1, 0),
            "exactly the loser is closed, the winner is untouched (got {p_disc}/{s_disc})"
        );
    }

    #[tokio::test]
    async fn stalled_loser_close_does_not_block_winner() {
        // Both dials open instantly and both stall forever on close: whoever
        // loses, the winner must still be delivered promptly and stay usable.
        let (primary, _) = scripted_full(None, None, true);
        let (secondary, _) = scripted_full(None, None, true);

        let (transport, _rx) = tokio::time::timeout(
            Duration::from_secs(5),
            race(primary, secondary).create_transport(),
        )
        .await
        .expect("the winner resolves despite the stalled loser close")
        .expect("one of the instant dials wins");

        transport
            .send(Bytes::from_static(b"ping"))
            .await
            .expect("the delivered winner is usable");
    }

    #[tokio::test]
    async fn stalled_loser_close_is_bounded_and_releases_its_socket() {
        // Both dials open instantly and both stall forever on close. The
        // loser's background close must give up and drop the transport
        // instead of retaining it detached forever: only the delivered
        // winner stays alive.
        let (primary, primary_c) = scripted_full(None, None, true);
        let (secondary, secondary_c) = scripted_full(None, None, true);

        let (_transport, _rx) = race(primary, secondary)
            .create_transport()
            .await
            .expect("one of the instant dials wins");
        assert_eq!(
            primary_c.get(|c| &c.transports_live) + secondary_c.get(|c| &c.transports_live),
            2,
            "both dials opened before either could be aborted"
        );

        tokio::time::timeout(Duration::from_secs(10), async {
            while primary_c.get(|c| &c.transports_live) + secondary_c.get(|c| &c.transports_live)
                > 1
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the stalled loser is dropped after the close bound");

        // Neither close ever completed: the loser's was aborted by the bound,
        // the winner's was never asked for.
        assert_eq!(primary_c.get(|c| &c.disconnects), 0);
        assert_eq!(secondary_c.get(|c| &c.disconnects), 0);
    }

    #[tokio::test]
    async fn first_failure_waits_for_second_success() {
        let (primary, _) = scripted(None, Some("primary refused"));
        let (secondary, secondary_c) = scripted(Some(10), None);

        race(primary, secondary)
            .create_transport()
            .await
            .expect("the surviving dial wins despite the refusal");

        assert_eq!(secondary_c.get(|c| &c.opened), 1);
    }

    #[tokio::test]
    async fn double_failure_returns_the_last_error() {
        // WA Web rejects with the failure that completes the set.
        let (primary, _) = scripted(Some(20), Some("boom-primary"));
        let (secondary, _) = scripted(Some(5), Some("boom-secondary"));
        let err = race(primary, secondary)
            .create_transport()
            .await
            .err()
            .expect("both dials fail");
        assert!(
            err.to_string().contains("boom-primary"),
            "the last failure wins, got: {err}"
        );

        let (primary, _) = scripted(Some(5), Some("boom-primary"));
        let (secondary, _) = scripted(Some(20), Some("boom-secondary"));
        let err = race(primary, secondary)
            .create_transport()
            .await
            .err()
            .expect("both dials fail");
        assert!(
            err.to_string().contains("boom-secondary"),
            "the last failure wins, got: {err}"
        );
    }

    #[tokio::test]
    async fn cancelled_race_leaks_no_socket() {
        use futures::FutureExt as _;

        let (primary, primary_c) = scripted(Some(100), None);
        let (secondary, secondary_c) = scripted(Some(100), None);
        let racing = race(primary, secondary);

        assert!(
            racing.create_transport().now_or_never().is_none(),
            "both dials are still in flight after one poll"
        );
        assert_eq!(primary_c.get(|c| &c.started), 1);
        assert_eq!(secondary_c.get(|c| &c.started), 1);

        // The race future (and both dials) dropped here.
        assert_eq!(primary_c.get(|c| &c.live), 0);
        assert_eq!(secondary_c.get(|c| &c.live), 0);
        assert_eq!(primary_c.get(|c| &c.opened), 0);
        assert_eq!(secondary_c.get(|c| &c.opened), 0);
        assert_eq!(primary_c.get(|c| &c.disconnects), 0);
        assert_eq!(secondary_c.get(|c| &c.disconnects), 0);
    }

    #[tokio::test]
    async fn winner_closed_immediately_after_win_leaves_no_loser() {
        let (primary, primary_c) = scripted(Some(50), None);
        let (secondary, secondary_c) = scripted(Some(5), None);

        let (transport, _rx) = race(primary, secondary)
            .create_transport()
            .await
            .expect("the fast dial succeeds");
        transport.disconnect().await;

        assert_eq!(secondary_c.get(|c| &c.disconnects), 1);
        assert_eq!(primary_c.get(|c| &c.opened), 0);
        assert_eq!(primary_c.get(|c| &c.disconnects), 0);
        assert_eq!(primary_c.get(|c| &c.live), 0);
        assert_eq!(secondary_c.get(|c| &c.live), 0);
    }

    #[test]
    fn edge_routing_param_absent_or_empty_leaves_url_untouched() {
        assert_eq!(
            with_edge_routing_param(WHATSAPP_WEB_WS_URL, None),
            WHATSAPP_WEB_WS_URL
        );
        assert_eq!(
            with_edge_routing_param(WHATSAPP_WEB_WS_URL, Some(&[])),
            WHATSAPP_WEB_WS_URL
        );
    }

    #[test]
    fn edge_routing_param_encodes_like_wa_web() {
        assert_eq!(
            with_edge_routing_param(WHATSAPP_WEB_WS_URL, Some(&[0x01, 0x02, 0x03])),
            format!("{WHATSAPP_WEB_WS_URL}?ED=AQID")
        );
        // WA Web's urlSafeBase64 swaps the alphabet but never strips `=`.
        assert_eq!(
            with_edge_routing_param(WHATSAPP_WEB_WS_URL, Some(&[0x01, 0x02])),
            format!("{WHATSAPP_WEB_WS_URL}?ED=AQI=")
        );
        // `+`/`/` become `-`/`_`.
        assert_eq!(
            with_edge_routing_param(WHATSAPP_WEB_WS_URL, Some(&[0xFB, 0xFF])),
            format!("{WHATSAPP_WEB_WS_URL}?ED=-_8=")
        );
        let url = with_edge_routing_param(WHATSAPP_WEB_WS_URL, Some(&[0xDE, 0xAD, 0xBE, 0xEF]));
        let encoded = url.split("?ED=").nth(1).expect("query is present");
        use base64::Engine as _;
        let decoded = base64::engine::general_purpose::URL_SAFE
            .decode(encoded)
            .expect("valid base64url");
        assert_eq!(decoded, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn edge_routing_param_goes_before_any_fragment() {
        assert_eq!(
            with_edge_routing_param("wss://example.invalid/ws/chat#frag", Some(&[0x01])),
            "wss://example.invalid/ws/chat?ED=AQ==#frag"
        );
        assert_eq!(
            with_edge_routing_param("wss://example.invalid/ws/chat?x=1#frag", Some(&[0x01])),
            "wss://example.invalid/ws/chat?x=1&ED=AQ==#frag"
        );
    }

    #[test]
    fn edge_routing_param_oversize_keeps_previous_behavior() {
        let oversize = vec![0x00u8; wacore_noise::MAX_EDGE_ROUTING_LEN + 1];
        assert_eq!(
            with_edge_routing_param(WHATSAPP_WEB_WS_URL, Some(&oversize)),
            WHATSAPP_WEB_WS_URL
        );
    }

    #[test]
    fn chat_urls_cover_both_wa_web_endpoints() {
        assert_eq!(
            WHATSAPP_WEB_WS_URL_FALLBACK,
            "wss://web.whatsapp.com:5222/ws/chat"
        );
        assert_eq!(
            WHATSAPP_WEB_WS_URLS,
            [WHATSAPP_WEB_WS_URL, WHATSAPP_WEB_WS_URL_FALLBACK]
        );
    }
}
