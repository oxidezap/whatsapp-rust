// ureq is a blocking HTTP client that depends on std::net and OS threads.
// It cannot work on wasm32 targets — users must provide their own HttpClient.
#![cfg(not(target_arch = "wasm32"))]

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore::net::{HttpClient, HttpRequest, HttpResponse, StreamingHttpResponse, UploadBody};
use wacore::stats::HttpResourceReport;

/// Matches `MAX_FILE_SIZE_BYTES` in `WAWebServerPropConstants` (2 GiB).
/// Overrides ureq's 10 MiB default on `read_to_vec()`.
pub const DEFAULT_MAX_BODY_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Per-buffer size for the default agent (16 KiB vs ureq's 128 KiB default):
/// WA API payloads are small JSON; media uses streaming I/O.
const INPUT_BUFFER_BYTES: u64 = 16 * 1024;
const OUTPUT_BUFFER_BYTES: u64 = 16 * 1024;
/// Idle connections the default agent's pool may retain.
const MAX_IDLE_CONNECTIONS: u64 = 3;

/// HTTP client implementation using `ureq` for synchronous HTTP requests.
/// Since `ureq` is blocking, all requests are wrapped in `tokio::task::spawn_blocking`.
#[derive(Debug, Clone)]
pub struct UreqHttpClient {
    agent: ureq::Agent,
    /// Total-bytes cap for both [`UreqHttpClient::execute`] and the reader from
    /// [`UreqHttpClient::execute_streaming`]. Bounds an in-memory sink so a
    /// hostile CDN can't drive it to OOM; defaults to WA's 2 GiB max file size.
    max_body_bytes: u64,
    /// Best-effort pool footprint for `resource_report`. `None` when a custom
    /// agent is supplied (its buffer/pool config is opaque to us).
    pool_report: Option<HttpResourceReport>,
    /// Set by the first request. Shared, because cloning shares the agent and
    /// therefore the pool. Read by [`UreqHttpClient::resource_report`].
    requested: Arc<AtomicBool>,
}

/// Pool footprint of the default agent once it has connected: each idle
/// connection keeps an input and an output buffer. ureq exposes neither the live
/// pool size nor in-flight buffering, so this is an upper-bound estimate, not a
/// measurement.
fn default_pool_report() -> HttpResourceReport {
    HttpResourceReport {
        pool_connections: Some(MAX_IDLE_CONNECTIONS),
        pool_buffer_bytes: Some(MAX_IDLE_CONNECTIONS * (INPUT_BUFFER_BYTES + OUTPUT_BUFFER_BYTES)),
        inflight_bytes: None,
    }
}

/// What the pool holds before the first request: nothing.
const EMPTY_POOL_REPORT: HttpResourceReport = HttpResourceReport {
    pool_connections: Some(0),
    pool_buffer_bytes: Some(0),
    inflight_bytes: None,
};

/// Latches that ureq is about to be handed a request it can actually send.
///
/// Decided before dispatch rather than from the error afterwards, because the
/// two are not the same question: a redirect to a malformed `Location` fails
/// with the same `BadUri` as a malformed request, long after the first one
/// reached the wire. Anything ureq refuses to build never opens a socket, so
/// the pool stays provably empty; everything past this point is its business.
///
/// The three refusals: `headers_ref` reports a builder carrying a bad header
/// name or value, ureq rejects a URI with no host or no known scheme, and a
/// request carrying `Connection: close` returns its socket to nobody. Erring
/// towards "not dispatchable" if those rules ever widen keeps the estimate a
/// lower bound, which is the direction that stays honest.
///
/// Relaxed: the flag feeds an on-demand estimate, not a happens-before.
fn mark_if_dispatchable<Any>(requested: &AtomicBool, req: &ureq::RequestBuilder<Any>, url: &str) {
    let Some(headers) = req.headers_ref() else {
        return;
    };
    // Mirrors ureq's own rule byte for byte (`ureq_proto`'s `Call::new` compares
    // the whole header value to `close`), because the question here is what ureq
    // will do with the socket, not what the RFC lets a caller write.
    let pools = !headers
        .get_all(ureq::http::header::CONNECTION)
        .iter()
        .any(|value| value.as_bytes() == b"close");
    let dispatchable = pools
        && ureq::http::Uri::try_from(url).is_ok_and(|uri| {
            uri.authority().is_some() && matches!(uri.scheme_str(), Some("http" | "https"))
        });
    if dispatchable {
        requested.store(true, Ordering::Relaxed);
    }
}

impl UreqHttpClient {
    pub fn new() -> Self {
        Self {
            agent: build_agent(),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            pool_report: Some(default_pool_report()),
            requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a client with a pre-configured [`ureq::Agent`].
    ///
    /// This lets you configure proxy support, custom TLS, timeouts,
    /// or any other agent-level settings externally.
    pub fn with_agent(agent: ureq::Agent) -> Self {
        Self {
            agent,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
            // A custom agent's buffer/pool sizes are opaque — don't guess.
            pool_report: None,
            requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Override the per-response cap for [`UreqHttpClient::execute`] and
    /// [`UreqHttpClient::execute_streaming`]. Set to `u64::MAX` to disable; a
    /// hostile server can then exhaust memory.
    pub fn with_max_body_bytes(mut self, max_body_bytes: u64) -> Self {
        self.max_body_bytes = max_body_bytes;
        self
    }
}

impl Default for UreqHttpClient {
    fn default() -> Self {
        Self::new()
    }
}

fn build_agent() -> ureq::Agent {
    use ureq::config::Config;

    #[allow(unused_mut)]
    let mut builder = Config::builder()
        // 16 KB per buffer instead of the 128 KB default.
        // WA API payloads are small JSON; media uses streaming I/O.
        .input_buffer_size(INPUT_BUFFER_BYTES as usize)
        .output_buffer_size(OUTPUT_BUFFER_BYTES as usize)
        .max_idle_connections(MAX_IDLE_CONNECTIONS as usize)
        .max_idle_connections_per_host(2);

    #[cfg(feature = "danger-skip-tls-verify")]
    {
        use ureq::tls::TlsConfig;
        builder = builder.tls_config(TlsConfig::builder().disable_verification(true).build());
    }

    builder.build().into()
}

/// Deliver 4xx/5xx as a response instead of `ureq::Error::StatusCode`.
///
/// [`HttpClient`] reserves `Err` for transport failures: the media paths read
/// `status_code` to decide whether a failure is retryable on the same host
/// (5xx), needs a refreshed media-auth token (401/403), or a re-derived URL
/// (404/410). ureq's default would collapse all of those into one opaque error
/// and take the media-conn refresh with it.
///
/// Set per request rather than on the agent, so a caller-supplied agent
/// ([`UreqHttpClient::with_agent`]) — which carries ureq's defaults, not ours —
/// still honors the contract.
fn status_as_response<Any>(req: ureq::RequestBuilder<Any>) -> ureq::RequestBuilder<Any> {
    req.config().http_status_as_error(false).build()
}

/// Ceiling on a non-2xx body, on top of [`UreqHttpClient::max_body_bytes`]
/// rather than instead of it — that knob is the caller's memory bound, and an
/// error page is not a reason to overrun it.
///
/// A CDN error page is diagnostic text, not payload: `upload.rs` puts it in the
/// error message, and WhatsApp Web goes further, reclassifying a 403 whose body
/// says `URL signature expired` as an expired URL rather than a refusal. Worth
/// a few KiB, never worth the megabytes a hostile host could send.
const ERROR_BODY_CAP: u64 = 64 * 1024;

/// Read the response body, keeping the status readable no matter what.
///
/// A 2xx body IS the payload, so an over-cap read there stays an error — the
/// caller must not mistake a truncated media file for a complete one. A non-2xx
/// body is diagnostic, so it is truncated instead: losing the tail of an error
/// page costs nothing, while losing the status costs the media-conn refresh
/// (see [`status_as_response`]).
///
/// Truncating leaves bytes unread, so ureq drops the connection instead of
/// pooling it. That is the intended trade: draining an unbounded error body to
/// save a socket hands a broken or hostile host a way to spend our time, and
/// the host that just answered 401/403 is the one this attempt is about to
/// rotate away from anyway.
fn read_body(response: ureq::http::Response<ureq::Body>, max_body_bytes: u64) -> Result<Vec<u8>> {
    if response.status().is_success() {
        // ureq's `read_to_vec()` default cap is 10 MiB.
        return Ok(response
            .into_body()
            .into_with_config()
            .limit(max_body_bytes)
            .read_to_vec()?);
    }

    let mut body = Vec::new();
    let mut reader = std::io::Read::take(
        response.into_body().into_reader(),
        max_body_bytes.min(ERROR_BODY_CAP),
    );
    // A read that fails partway still leaves the status worth returning.
    let _ = std::io::Read::read_to_end(&mut reader, &mut body);
    Ok(body)
}

#[async_trait]
impl HttpClient for UreqHttpClient {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse> {
        let agent = self.agent.clone();
        let max_body_bytes = self.max_body_bytes;
        let requested = self.requested.clone();
        // Since ureq is blocking, we must use spawn_blocking
        tokio::task::spawn_blocking(move || {
            let response = match request.method.as_str() {
                "GET" => {
                    let mut req = status_as_response(agent.get(&request.url));
                    for (key, value) in &request.headers {
                        req = req.header(key, value);
                    }
                    mark_if_dispatchable(&requested, &req, &request.url);
                    req.call()?
                }
                "POST" => {
                    let mut req = status_as_response(agent.post(&request.url));
                    for (key, value) in &request.headers {
                        req = req.header(key, value);
                    }
                    mark_if_dispatchable(&requested, &req, &request.url);
                    if let Some(body) = request.body {
                        req.send(&body[..])?
                    } else {
                        req.send(&[])?
                    }
                }
                method => {
                    return Err(anyhow::anyhow!("Unsupported HTTP method: {}", method));
                }
            };

            let status_code = response.status().as_u16();
            let body = read_body(response, max_body_bytes)?;

            Ok(HttpResponse { status_code, body })
        })
        .await?
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn execute_streaming(&self, request: HttpRequest) -> Result<StreamingHttpResponse> {
        // Note: no spawn_blocking here — this is called FROM within spawn_blocking
        // by the streaming download code. The entire HTTP fetch + decrypt happens
        // in one blocking thread.
        let response = match request.method.as_str() {
            "GET" => {
                let mut req = status_as_response(self.agent.get(&request.url));
                for (key, value) in &request.headers {
                    req = req.header(key, value);
                }
                mark_if_dispatchable(&self.requested, &req, &request.url);
                req.call()?
            }
            method => {
                return Err(anyhow::anyhow!(
                    "Streaming only supports GET, got: {}",
                    method
                ));
            }
        };

        let status_code = response.status().as_u16();
        // Bound the streaming reader to the same cap `execute` enforces: an
        // in-memory sink (`Client::download` buffers into a `Vec`) must not be
        // driveable to OOM by a CDN that streams past the declared length. Over
        // the cap the reader hits EOF and the downstream MAC/SHA check fails,
        // rather than growing the sink unbounded. `DOWNLOAD_PREALLOC_CAP` only
        // sizes the initial allocation, not the total read.
        let reader = std::io::Read::take(response.into_body().into_reader(), self.max_body_bytes);

        Ok(StreamingHttpResponse {
            status_code,
            body: Box::new(reader),
        })
    }

    fn supports_upload_streaming(&self) -> bool {
        true
    }

    fn execute_upload(
        &self,
        request: HttpRequest,
        body: UploadBody,
        content_length: u64,
    ) -> Result<HttpResponse> {
        // No spawn_blocking — like execute_streaming, this is driven from within
        // a blocking context, and the reader is read on this thread.
        if request.method != "POST" {
            return Err(anyhow::anyhow!(
                "Upload streaming only supports POST, got: {}",
                request.method
            ));
        }

        let mut req = status_as_response(self.agent.post(&request.url));
        for (key, value) in &request.headers {
            req = req.header(key, value);
        }
        // Explicit Content-Length keeps ureq length-delimited instead of chunked
        // (which WhatsApp's CDN rejects) for an arbitrary reader body.
        let content_length = content_length.to_string();
        req = req.header("content-length", content_length.as_str());

        mark_if_dispatchable(&self.requested, &req, &request.url);
        let response = req.send(ureq::SendBody::from_owned_reader(body))?;

        let status_code = response.status().as_u16();
        let body = read_body(response, self.max_body_bytes)?;

        Ok(HttpResponse { status_code, body })
    }

    /// An empty pool before the first request, the configured cap after it.
    ///
    /// ureq allocates per connection, not per agent, so an agent that has never
    /// connected holds no buffers at all: 2.8 KiB of measured RSS against the
    /// 96 KiB the cap advertises. Reporting the cap there put a quarter of a
    /// session's estimate on memory that was not resident, and the client's
    /// `resource_report()` total promises a lower bound.
    ///
    /// Both answers need an agent we built. A caller-supplied one
    /// ([`UreqHttpClient::with_agent`]) may already have connected before it
    /// reached us, since agents share their pool with every clone, so its pool
    /// is as opaque as its buffer sizes and stays unreported.
    fn resource_report(&self) -> Option<HttpResourceReport> {
        let pool_report = self.pool_report?;
        if !self.requested.load(Ordering::Relaxed) {
            return Some(EMPTY_POOL_REPORT);
        }
        Some(pool_report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;
    use std::time::Duration;

    fn spawn_fixed_size_server(body_size: usize) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = [0u8; 4096];
            let mut total = Vec::new();
            loop {
                let n = stream.read(&mut buf).unwrap_or(0);
                if n == 0 {
                    return;
                }
                total.extend_from_slice(&buf[..n]);
                if total.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body_size
            );
            stream.write_all(header.as_bytes()).unwrap();
            let chunk = vec![0xABu8; 64 * 1024];
            let mut sent = 0usize;
            while sent < body_size {
                let take = chunk.len().min(body_size - sent);
                stream.write_all(&chunk[..take]).unwrap();
                sent += take;
            }
        });
        format!("http://{}", addr)
    }

    /// Regression: ureq 3.x caps `read_to_vec()` at 10 MiB by default.
    #[tokio::test(flavor = "current_thread")]
    async fn execute_accepts_body_larger_than_ureq_default_limit() {
        const SIZE: usize = 12 * 1024 * 1024;
        let url = spawn_fixed_size_server(SIZE);
        let resp = UreqHttpClient::new()
            .execute(HttpRequest {
                method: "GET".into(),
                url,
                headers: std::collections::HashMap::new(),
                body: None,
            })
            .await
            .expect("body must fit under the configured cap");
        assert_eq!(resp.status_code, 200);
        assert_eq!(resp.body.len(), SIZE);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn with_max_body_bytes_enforces_tighter_cap() {
        const SIZE: usize = 4 * 1024 * 1024;
        let url = spawn_fixed_size_server(SIZE);
        UreqHttpClient::new()
            .with_max_body_bytes(1024)
            .execute(HttpRequest {
                method: "GET".into(),
                url,
                headers: std::collections::HashMap::new(),
                body: None,
            })
            .await
            .expect_err("1 KiB cap must reject a 4 MiB body");
    }

    // The streaming reader must honor the same cap: an over-cap body is
    // truncated at EOF (the caller's decrypt/MAC check then rejects it) instead
    // of growing an in-memory sink to OOM.
    #[tokio::test(flavor = "current_thread")]
    async fn execute_streaming_bounds_body_at_cap() {
        const SIZE: usize = 4 * 1024 * 1024;
        const CAP: u64 = 1024;
        let url = spawn_fixed_size_server(SIZE);
        let read = tokio::task::spawn_blocking(move || {
            let mut resp = UreqHttpClient::new()
                .with_max_body_bytes(CAP)
                .execute_streaming(HttpRequest {
                    method: "GET".into(),
                    url,
                    headers: std::collections::HashMap::new(),
                    body: None,
                })
                .expect("streaming GET should start");
            let mut sink = std::io::sink();
            std::io::copy(&mut resp.body, &mut sink).expect("draining the reader should not error")
        })
        .await
        .unwrap();
        assert_eq!(read, CAP, "streaming body must stop at the cap");
    }

    /// Captures the raw request headers and body of a single POST, then replies 200.
    fn spawn_capture_server() -> (String, std::sync::mpsc::Receiver<(String, Vec<u8>)>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let header_end = loop {
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
                let n = stream.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    return;
                }
                buf.extend_from_slice(&tmp[..n]);
            };
            let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let content_length = headers.lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                if k.trim().eq_ignore_ascii_case("content-length") {
                    v.trim().parse::<usize>().ok()
                } else {
                    None
                }
            });
            let mut body = buf[header_end..].to_vec();
            if let Some(cl) = content_length {
                while body.len() < cl {
                    let n = stream.read(&mut tmp).unwrap_or(0);
                    if n == 0 {
                        break;
                    }
                    body.extend_from_slice(&tmp[..n]);
                }
            }
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
            let _ = tx.send((headers, body));
        });
        (format!("http://{addr}"), rx)
    }

    fn parsed_content_length(headers: &str) -> Option<usize> {
        headers.lines().find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
    }

    /// The key invariant: an arbitrary (non-`File`) reader body must be sent with
    /// an explicit Content-Length and never chunked — matching WhatsApp Web.
    #[test]
    fn upload_streaming_sets_content_length_not_chunked() {
        let (url, rx) = spawn_capture_server();
        let payload: Vec<u8> = (0..5000u32).map(|i| i as u8).collect();
        let client = UreqHttpClient::new();

        let resp = client
            .execute_upload(
                HttpRequest {
                    method: "POST".into(),
                    url,
                    headers: std::collections::HashMap::new(),
                    body: None,
                },
                Box::new(std::io::Cursor::new(payload.clone())),
                payload.len() as u64,
            )
            .expect("upload should succeed");
        assert_eq!(resp.status_code, 200);

        let (headers, body) = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("server should capture the request");
        assert_eq!(
            parsed_content_length(&headers),
            Some(payload.len()),
            "exact Content-Length expected, headers:\n{headers}"
        );
        assert!(
            !headers.to_ascii_lowercase().contains("transfer-encoding"),
            "body must not be chunked, headers:\n{headers}"
        );
        assert_eq!(body, payload, "server must receive the exact bytes");
    }

    /// A body larger than the 16 KiB output buffer exercises real chunked reads
    /// from the reader while still arriving intact and length-delimited.
    #[test]
    fn upload_streaming_large_body_integrity() {
        let (url, rx) = spawn_capture_server();
        let payload: Vec<u8> = (0..200_000usize).map(|i| (i % 251) as u8).collect();
        let client = UreqHttpClient::new();

        let resp = client
            .execute_upload(
                HttpRequest {
                    method: "POST".into(),
                    url,
                    headers: std::collections::HashMap::new(),
                    body: None,
                },
                Box::new(std::io::Cursor::new(payload.clone())),
                payload.len() as u64,
            )
            .expect("upload should succeed");
        assert_eq!(resp.status_code, 200);

        let (headers, body) = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("server should capture the request");
        assert_eq!(parsed_content_length(&headers), Some(payload.len()));
        assert_eq!(body, payload);
    }

    /// Workstream D: once it has connected, the default agent reports its
    /// idle-pool buffer estimate; a custom agent (opaque config) reports nothing.
    #[tokio::test(flavor = "current_thread")]
    async fn resource_report_estimates_default_pool_after_a_request() {
        let url = spawn_status_server(200, "OK");
        let client = UreqHttpClient::new();
        client.execute(get(url)).await.expect("request should run");

        let report = client
            .resource_report()
            .expect("default agent reports a pool estimate");
        assert_eq!(report.pool_connections, Some(MAX_IDLE_CONNECTIONS));
        assert_eq!(
            report.pool_buffer_bytes,
            Some(MAX_IDLE_CONNECTIONS * (INPUT_BUFFER_BYTES + OUTPUT_BUFFER_BYTES))
        );
        assert_eq!(report.inflight_bytes, None);
        assert!(report.total_bytes() > 0);

        // A custom agent's buffer/pool config is opaque — don't guess.
        let custom = UreqHttpClient::with_agent(build_agent());
        custom
            .execute(get(spawn_status_server(200, "OK")))
            .await
            .expect("request should run");
        assert!(
            custom.resource_report().is_none(),
            "custom-agent client reports no estimate"
        );

        // with_max_body_bytes preserves the pool estimate.
        let capped = UreqHttpClient::new().with_max_body_bytes(1024);
        capped
            .execute(get(spawn_status_server(200, "OK")))
            .await
            .expect("request should run");
        assert!(capped.resource_report().is_some());
    }

    /// An agent we built and never used holds no pool buffers, so it reports
    /// the measured `Some(0)` rather than the cap. `Some(0)` and not `None`
    /// because an empty pool is knowable here, unlike a component that cannot
    /// introspect itself at all.
    #[test]
    fn resource_report_is_empty_before_the_first_request() {
        for client in [
            UreqHttpClient::new(),
            UreqHttpClient::new().with_max_body_bytes(1024),
        ] {
            let report = client
                .resource_report()
                .expect("an empty pool is a fact, not an absence");
            assert_eq!(report.pool_connections, Some(0));
            assert_eq!(report.pool_buffer_bytes, Some(0));
            assert_eq!(report.total_bytes(), 0);
        }
    }

    /// A caller-supplied agent is opaque in both directions: its buffer sizes
    /// are unknown, and it may already have connected before it reached us,
    /// because every clone of an agent shares one pool. Answering `Some(0)`
    /// there would understate a pool we cannot see.
    #[test]
    fn a_custom_agent_reports_nothing_even_before_the_first_request() {
        let shared = build_agent();
        assert!(
            UreqHttpClient::with_agent(shared.clone())
                .resource_report()
                .is_none(),
            "a pool this client did not create is not knowably empty"
        );

        // The case that makes it unknowable: the agent connected before we
        // wrapped it. Two requests answered over one accepted connection is the
        // proof that the first one is sitting in the pool, and draining each
        // body is what makes it poolable at all.
        let (url, accepted) = spawn_keep_alive_server();
        for _ in 0..2 {
            let response = shared
                .get(&url)
                .call()
                .expect("the warm-up request must reach the fixture");
            assert_eq!(
                response
                    .into_body()
                    .read_to_vec()
                    .expect("draining leaves a poolable connection"),
                b"ok"
            );
        }
        assert_eq!(
            accepted.load(Ordering::Relaxed),
            1,
            "the second request must have reused a pooled connection"
        );

        assert!(
            UreqHttpClient::with_agent(shared)
                .resource_report()
                .is_none()
        );
    }

    /// Cloning shares the agent and therefore the pool, so it has to share the
    /// latch too. Otherwise the original keeps reporting an empty pool while a
    /// clone fills it.
    #[tokio::test(flavor = "current_thread")]
    async fn a_clone_that_requests_marks_the_original() {
        let client = UreqHttpClient::new();
        let clone = client.clone();
        clone
            .execute(get(spawn_status_server(200, "OK")))
            .await
            .expect("request should run");

        assert_eq!(
            client.resource_report().and_then(|r| r.pool_connections),
            Some(MAX_IDLE_CONNECTIONS),
            "the original shares the clone's pool, so it must share its estimate"
        );
    }

    /// A request that never reaches the wire must not move the report: an
    /// unsupported method is rejected before any connection is attempted.
    #[tokio::test(flavor = "current_thread")]
    async fn a_rejected_request_leaves_the_pool_reported_empty() {
        let client = UreqHttpClient::new();
        client
            .execute(HttpRequest {
                method: "PATCH".into(),
                url: "http://127.0.0.1:0/never".into(),
                headers: std::collections::HashMap::new(),
                body: None,
            })
            .await
            .expect_err("PATCH is not supported");
        client
            .execute_upload(
                HttpRequest {
                    method: "GET".into(),
                    url: "http://127.0.0.1:0/never".into(),
                    headers: std::collections::HashMap::new(),
                    body: None,
                },
                Box::new(std::io::Cursor::new(vec![1u8])),
                1,
            )
            .expect_err("upload streaming is POST-only");

        assert_eq!(
            client.resource_report().and_then(|r| r.pool_buffer_bytes),
            Some(0),
            "nothing connected, so nothing is pooled"
        );
    }

    /// A supported method is not enough: a request ureq rejects while building
    /// it never reaches a socket either, so the pool stays reported empty.
    #[tokio::test(flavor = "current_thread")]
    async fn a_request_rejected_before_the_wire_leaves_the_pool_reported_empty() {
        let client = UreqHttpClient::new();
        client
            .execute(get("not-a-uri".into()))
            .await
            .expect_err("a URI with no scheme or host cannot be sent");
        client
            .execute(
                HttpRequest::post("http://127.0.0.1:1/never")
                    .with_header("bad header name", "v")
                    .with_body(b"x".to_vec()),
            )
            .await
            .expect_err("an invalid header name cannot be sent");

        assert_eq!(
            client.resource_report().and_then(|r| r.pool_buffer_bytes),
            Some(0),
            "nothing was built, so nothing connected"
        );
    }

    /// An ordinary request keeps landing on the connection the last one left.
    #[tokio::test(flavor = "current_thread")]
    async fn an_ordinary_request_reuses_the_pooled_connection() {
        let (url, accepted) = spawn_keep_alive_server();
        let client = UreqHttpClient::new();
        for _ in 0..2 {
            client
                .execute(get(url.clone()))
                .await
                .expect("the request to answer");
        }

        assert_eq!(
            accepted.load(Ordering::Relaxed),
            1,
            "the second request opened its own connection instead of reusing the pooled one"
        );
        assert_eq!(
            client.resource_report().and_then(|r| r.pool_connections),
            Some(MAX_IDLE_CONNECTIONS)
        );
    }

    /// A closing request opens its own connection, and the estimate must not
    /// then claim a pool the agent does not hold.
    #[tokio::test(flavor = "current_thread")]
    async fn a_connection_close_request_pools_nothing_and_reports_nothing() {
        let (url, accepted) = spawn_keep_alive_server();
        let client = UreqHttpClient::new();
        for _ in 0..2 {
            client
                .execute(get(url.clone()).with_header("connection", "close"))
                .await
                .expect("the request to answer");
        }

        assert_eq!(
            accepted.load(Ordering::Relaxed),
            2,
            "a connection the caller asked to close was pooled and reused"
        );
        assert_eq!(
            client.resource_report().and_then(|r| r.pool_buffer_bytes),
            Some(0),
            "nothing was pooled, so the estimate must not claim the cap"
        );
    }

    /// RFC 9110 lets `Connection` carry a token list, and ureq does not read one
    /// — it compares the whole value to `close`. The estimate deliberately
    /// follows ureq rather than the RFC, so this pins the pair together: widen
    /// one and this fails until the other widens too.
    #[tokio::test(flavor = "current_thread")]
    async fn a_token_list_close_is_pooled_by_ureq_and_reported_as_pooled() {
        let (url, accepted) = spawn_keep_alive_server();
        let client = UreqHttpClient::new();
        for _ in 0..2 {
            client
                .execute(get(url.clone()).with_header("connection", "keep-alive, close"))
                .await
                .expect("the request to answer");
        }

        assert_eq!(
            accepted.load(Ordering::Relaxed),
            1,
            "ureq pooled a token-list close; the estimate below assumes it did"
        );
        assert_eq!(
            client.resource_report().and_then(|r| r.pool_connections),
            Some(MAX_IDLE_CONNECTIONS),
            "a pooled connection must not be reported as an empty pool"
        );
    }

    /// A redirect to a malformed `Location` fails with the same error a
    /// malformed request does, but the first hop already reached the wire. The
    /// latch is decided before dispatch precisely so this case is not read as a
    /// request that never left.
    #[tokio::test(flavor = "current_thread")]
    async fn a_redirect_to_a_bad_location_still_counts_as_dispatched() {
        let url = spawn_status_server_with_headers(
            302,
            "Found",
            &[("location", "://not-a-uri")],
            Vec::new(),
        );
        let client = UreqHttpClient::new();
        client
            .execute(get(url))
            .await
            .expect_err("a redirect to a malformed location cannot be followed");

        assert_eq!(
            client.resource_report().and_then(|r| r.pool_connections),
            Some(MAX_IDLE_CONNECTIONS),
            "the first hop connected, so the pool is no longer provably empty"
        );
    }

    /// A transport failure still attempts a connection, so the estimate must
    /// switch on: what the pool holds after a failed connect is ureq's business,
    /// not something this client can rule out.
    #[tokio::test(flavor = "current_thread")]
    async fn a_failed_request_still_switches_the_estimate_on() {
        let client = UreqHttpClient::new();
        client
            .execute(get(spawn_disconnecting_server()))
            .await
            .expect_err("the peer hangs up before answering");

        assert_eq!(
            client.resource_report().and_then(|r| r.pool_connections),
            Some(MAX_IDLE_CONNECTIONS)
        );
    }

    fn spawn_status_server(status: u16, reason: &str) -> String {
        spawn_status_server_with_body(status, reason, b"denied".to_vec())
    }

    /// Answers every request with a small 200 and leaves the connection open,
    /// so a drained response goes back into the agent's pool. The counter is
    /// how a test tells a reused connection from a fresh one.
    fn spawn_keep_alive_server() -> (String, Arc<std::sync::atomic::AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        let accepted = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = accepted.clone();
        thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                counter.fetch_add(1, Ordering::Relaxed);
                thread::spawn(move || {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 1024];
                    loop {
                        match stream.read(&mut tmp) {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&tmp[..n]),
                        }
                        while let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                            buf.drain(..pos + 4);
                            if stream
                                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                });
            }
        });
        (format!("http://{addr}"), accepted)
    }

    /// Accepts one connection and hangs up without answering. An owned port
    /// rather than a well-known one nothing is expected to use, so the failure
    /// is the fixture's doing and not the machine's.
    fn spawn_disconnecting_server() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                drop(stream);
            }
        });
        format!("http://{addr}")
    }

    /// Answers one request with `status` and `body`. The request body is drained
    /// first so a rejected upload never races a broken pipe against the response.
    fn spawn_status_server_with_body(status: u16, reason: &str, body: Vec<u8>) -> String {
        spawn_status_server_with_headers(status, reason, &[], body)
    }

    fn spawn_status_server_with_headers(
        status: u16,
        reason: &str,
        extra_headers: &[(&str, &str)],
        body: Vec<u8>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        let reason = reason.to_string();
        let extra: String = extra_headers
            .iter()
            .map(|(k, v)| format!("{k}: {v}\r\n"))
            .collect();
        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            let header_end = loop {
                if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    break pos + 4;
                }
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            };
            let headers = String::from_utf8_lossy(&buf[..header_end]).to_string();
            if let Some(cl) = parsed_content_length(&headers) {
                let mut body_len = buf.len() - header_end;
                while body_len < cl {
                    match stream.read(&mut tmp) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => body_len += n,
                    }
                }
            }
            let header = format!(
                "HTTP/1.1 {status} {reason}\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            // Either write can fail: the client is free to hang up once it has
            // all of the body it intends to keep.
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{addr}")
    }

    fn get(url: String) -> HttpRequest {
        HttpRequest {
            method: "GET".into(),
            url,
            headers: std::collections::HashMap::new(),
            body: None,
        }
    }

    /// Regression (#1185): a CDN 403/404 is a *response*, not a transport error.
    /// `download.rs` classifies the status itself — 401/403 into a media-auth
    /// refresh, 404/410 into a URL re-derivation — so swallowing the status into
    /// an opaque `Err` makes both paths unreachable and every host retry carries
    /// the same stale auth token.
    #[tokio::test(flavor = "current_thread")]
    async fn execute_surfaces_non_2xx_status_instead_of_erroring() {
        for (status, reason) in [
            (401u16, "Unauthorized"),
            (403, "Forbidden"),
            (404, "Not Found"),
        ] {
            let url = spawn_status_server(status, reason);
            let resp = UreqHttpClient::new()
                .execute(get(url))
                .await
                .unwrap_or_else(|e| panic!("{status} must arrive as a response, got error: {e}"));
            assert_eq!(resp.status_code, status);
            assert_eq!(resp.body, b"denied");
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn execute_post_surfaces_non_2xx_status_instead_of_erroring() {
        let url = spawn_status_server(403, "Forbidden");
        let resp = UreqHttpClient::new()
            .execute(HttpRequest::post(url).with_body(b"payload".to_vec()))
            .await
            .expect("403 must arrive as a response, not an error");
        assert_eq!(resp.status_code, 403);
    }

    /// The streaming path is what media downloads actually use.
    #[tokio::test(flavor = "current_thread")]
    async fn execute_streaming_surfaces_non_2xx_status_instead_of_erroring() {
        let url = spawn_status_server(403, "Forbidden");
        let status = tokio::task::spawn_blocking(move || {
            UreqHttpClient::new()
                .execute_streaming(get(url))
                .expect("403 must arrive as a response, not an error")
                .status_code
        })
        .await
        .unwrap();
        assert_eq!(status, 403);
    }

    /// Uploads classify `is_media_auth_error(status)` off the response too.
    #[test]
    fn execute_upload_surfaces_non_2xx_status_instead_of_erroring() {
        let url = spawn_status_server(403, "Forbidden");
        let payload = vec![7u8; 128];
        let resp = UreqHttpClient::new()
            .execute_upload(
                HttpRequest {
                    method: "POST".into(),
                    url,
                    headers: std::collections::HashMap::new(),
                    body: None,
                },
                Box::new(std::io::Cursor::new(payload.clone())),
                payload.len() as u64,
            )
            .expect("403 must arrive as a response, not an error");
        assert_eq!(resp.status_code, 403);
    }

    /// Knowing the status is not enough if reading the body then throws it away.
    /// A 403 whose error page overruns a tightened `max_body_bytes` must still
    /// arrive as a 403 — otherwise the media-conn refresh is unreachable again,
    /// by a different route.
    #[tokio::test(flavor = "current_thread")]
    async fn over_cap_error_body_does_not_cost_the_status() {
        const CAP: u64 = 1024;
        let url = spawn_status_server_with_body(403, "Forbidden", vec![b'x'; 4 * 1024 * 1024]);
        let resp = UreqHttpClient::new()
            .with_max_body_bytes(CAP)
            .execute(get(url))
            .await
            .expect("an over-cap error page must not erase the status it came with");
        assert_eq!(resp.status_code, 403);
        assert!(
            resp.body.len() as u64 <= CAP,
            "the diagnostic body must stay bounded, got {} bytes",
            resp.body.len()
        );
    }

    /// The mirror case, and the reason the truncation is not unconditional: a
    /// 2xx body IS the payload, so an over-cap read there must stay an error
    /// rather than hand back a silently truncated media file.
    #[tokio::test(flavor = "current_thread")]
    async fn over_cap_success_body_is_still_an_error() {
        let url = spawn_status_server_with_body(200, "OK", vec![b'x'; 4 * 1024 * 1024]);
        UreqHttpClient::new()
            .with_max_body_bytes(1024)
            .execute(get(url))
            .await
            .expect_err("a truncated 2xx payload must never look like a complete one");
    }

    /// A caller-supplied agent carries ureq's own defaults, so the status
    /// contract has to be enforced per request rather than on our agent.
    #[tokio::test(flavor = "current_thread")]
    async fn custom_agent_also_surfaces_non_2xx_status() {
        let url = spawn_status_server(403, "Forbidden");
        let agent: ureq::Agent = ureq::config::Config::builder().build().into();
        let resp = UreqHttpClient::with_agent(agent)
            .execute(get(url))
            .await
            .expect("403 must arrive as a response even with a custom agent");
        assert_eq!(resp.status_code, 403);
    }

    /// How long the gate below waits for its peers before giving up. Long
    /// enough that a loaded runner never trips it, short enough that the whole
    /// failure still lands inside nextest's 60s slow warning.
    const GATE_DEADLINE: Duration = Duration::from_secs(20);

    /// A [`std::sync::Barrier`] that gives up instead of waiting forever.
    ///
    /// `Barrier::wait` is the natural fit — release only once `n` callers have
    /// arrived — but a barrier that is one caller short blocks every thread on
    /// it for good. That turns the regression this fixture exists to catch into
    /// a hung CI job rather than a verdict: nextest's default profile
    /// deliberately warns on a slow test without killing it
    /// (`.config/nextest.toml`), so nothing upstream would cut the wait short.
    ///
    /// So the release is sticky and has a deadline. Sticky because a serialized
    /// client arrives one request at a time: without it, each of the `n`
    /// stragglers would serve its own full deadline and the failure would take
    /// `n` times longer than the diagnosis needs. Whoever trips the deadline
    /// records it, and the test asserts on that — a named failure instead of a
    /// wait with no end.
    #[derive(Default)]
    struct Gate {
        state: std::sync::Mutex<GateState>,
        released: std::sync::Condvar,
    }

    #[derive(Default)]
    struct GateState {
        arrived: usize,
        open: bool,
        starved: bool,
    }

    impl Gate {
        fn wait(&self, n: usize, deadline: Duration) {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.arrived += 1;
            if state.arrived >= n || state.open {
                state.open = true;
                self.released.notify_all();
                return;
            }
            let (mut state, timeout) = self
                .released
                .wait_timeout_while(state, deadline, |state| !state.open)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if timeout.timed_out() {
                state.starved = true;
                state.open = true;
                self.released.notify_all();
            }
        }

        fn starved(&self) -> bool {
            self.state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .starved
        }
    }

    /// The escape hatch itself: a peer that never arrives ends the wait at the
    /// deadline and records it, which is what turns the regression below into a
    /// failure instead of a hang. Worth its own test because nothing else
    /// exercises it — the concurrency test only ever takes the happy path.
    #[test]
    fn the_gate_gives_up_instead_of_waiting_forever() {
        let gate = Gate::default();
        gate.wait(2, Duration::from_millis(50));
        assert!(
            gate.starved(),
            "a peer that never arrives must end the wait"
        );
    }

    /// And the other side of it: real peers release the gate without tripping
    /// the deadline, so the assertion above cannot pass vacuously.
    #[test]
    fn the_gate_releases_once_its_peers_arrive() {
        let gate = Arc::new(Gate::default());
        let peer = Arc::clone(&gate);
        let joined = thread::spawn(move || peer.wait(2, GATE_DEADLINE));
        gate.wait(2, GATE_DEADLINE);
        joined.join().expect("peer thread");

        assert!(
            !gate.starved(),
            "both arrived, so nothing should have starved"
        );
    }

    /// Answers nothing until `n` requests have arrived, so the test can only
    /// pass if all `n` were in flight at the same time.
    fn spawn_barrier_server(n: usize) -> (String, Arc<Gate>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().unwrap();
        let gate = Arc::new(Gate::default());
        let server_gate = Arc::clone(&gate);
        thread::spawn(move || {
            while let Ok((mut stream, _)) = listener.accept() {
                let gate = Arc::clone(&server_gate);
                thread::spawn(move || {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 1024];
                    loop {
                        match stream.read(&mut tmp) {
                            Ok(0) | Err(_) => return,
                            Ok(k) => buf.extend_from_slice(&tmp[..k]),
                        }
                        if buf.windows(4).any(|w| w == b"\r\n\r\n") {
                            break;
                        }
                    }
                    gate.wait(n, GATE_DEADLINE);
                    // Answered even when the gate timed out, so the client side
                    // finishes and the assertion — not the wait — is what fails.
                    let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok");
                });
            }
        });
        (format!("http://{addr}"), gate)
    }

    /// One client shared by a fleet must not become a fleet-wide lock.
    ///
    /// `BotBuilder::with_http_client_arc` invites exactly that sharing, so what
    /// `ureq::Agent` does with concurrent callers is part of this crate's
    /// contract rather than an implementation detail: it holds the pool lock
    /// across checkout only, never across the request. The barrier server is
    /// what makes a regression fail rather than merely run slower — if the
    /// agent ever serialized, request 1 would block forever waiting for a
    /// response the server only sends once request N has arrived.
    ///
    /// `N` is deliberately far above the agent's 3-connection idle pool: that
    /// cap bounds what is *retained* between requests, and reading it as a
    /// concurrency limit is the mistake this pins down. It is also the figure
    /// `BotBuilder::with_http_client_arc` quotes, so the doc there is only ever
    /// claiming what this test actually holds — raise one and raise the other.
    #[tokio::test(flavor = "current_thread")]
    async fn a_shared_client_runs_concurrent_requests_concurrently() {
        const N: usize = 64;
        let (url, gate) = spawn_barrier_server(N);
        let client = Arc::new(UreqHttpClient::new());

        let mut handles = Vec::with_capacity(N);
        for _ in 0..N {
            let client = Arc::clone(&client);
            let url = url.clone();
            handles.push(tokio::spawn(async move { client.execute(get(url)).await }));
        }
        for handle in handles {
            let response = handle
                .await
                .expect("request task")
                .expect("every request should reach the fixture");
            assert_eq!(response.status_code, 200);
        }

        assert!(
            !gate.starved(),
            "the gate timed out: fewer than {N} requests were ever in flight at once, \
             so the shared agent serialized them"
        );
    }

    #[test]
    fn upload_streaming_rejects_non_post() {
        let client = UreqHttpClient::new();
        let err = client.execute_upload(
            HttpRequest {
                method: "GET".into(),
                url: "http://127.0.0.1:0/never".into(),
                headers: std::collections::HashMap::new(),
                body: None,
            },
            Box::new(std::io::Cursor::new(vec![1u8, 2, 3])),
            3,
        );
        assert!(err.is_err(), "only POST is allowed for upload streaming");
    }
}
