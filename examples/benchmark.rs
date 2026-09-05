// See the matching note in lib.rs: instrumented large async fns need a deeper
// recursion limit when the `--all-features` paths (tracing + tracing-pii) combine.
#![recursion_limit = "512"]

use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use wacore::net::{HttpClient, HttpRequest};
use wacore::proto_helpers::MessageExt;
use wacore::store::InMemoryBackend;
use wacore::types::events::{Event, EventKind};
use whatsapp_rust::TokioRuntime;
use whatsapp_rust::bot::{Bot, MessageContext};
use whatsapp_rust::handshake::NoiseCertPolicy;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

/// Derive the mock-server admin scan-qr endpoint from a `ws[s]://host:port/...`
/// WebSocket URL. Changes only the scheme (`ws`→`http` / `wss`→`https`) and the
/// path (`/admin/mock-phone/scan-qr`); host, port and query carry over, and any
/// fragment is dropped (fragments are never sent over HTTP). Mirrors
/// `tests/e2e/src/lib.rs`. Returns `None` for URLs without a ws scheme or
/// without an authority.
fn mock_admin_scan_qr_url(ws_url: &str) -> Option<String> {
    let ws_url = ws_url.split('#').next()?;
    let uri: http::Uri = ws_url.parse().ok()?;
    let http_scheme = match uri.scheme_str() {
        Some("ws") => "http",
        Some("wss") => "https",
        _ => return None,
    };
    let authority = uri.authority()?;
    let mut admin = format!("{http_scheme}://{authority}/admin/mock-phone/scan-qr");
    if let Some(query) = uri.query() {
        admin.push('?');
        admin.push_str(query);
    }
    Some(admin)
}

/// Endpoint + Noise policy selection for this example.
///
/// - Default (neither var set): production default URL, `Strict`, no admin POST.
/// - `WHATSAPP_WS_URL` set: custom/production URL, always `Strict`, never an
///   admin POST. Wins over `MOCK_SERVER_URL` so a production override cannot
///   inherit the mock bypass.
/// - Only `MOCK_SERVER_URL` set: mock URL, `DangerSkipCertChainVerify`, admin
///   POST derived from that URL. The mock cannot produce a WhatsApp-rooted
///   chain, so the bypass is scoped to this explicitly selected mode.
///
/// No hostname guessing and no core-global policy: the bypass travels as a
/// per-client builder value, exactly like `tests/e2e`.
struct BenchmarkEndpoint {
    ws_url: Option<String>,
    cert_policy: NoiseCertPolicy,
    admin_scan_url: Option<String>,
    is_mock: bool,
}

fn resolve_benchmark_endpoint(
    whatsapp_ws_url: Option<String>,
    mock_server_url: Option<String>,
) -> BenchmarkEndpoint {
    let normalize = |v: Option<String>| v.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let whatsapp_ws_url = normalize(whatsapp_ws_url);
    let mock_server_url = normalize(mock_server_url);
    if let Some(url) = whatsapp_ws_url {
        return BenchmarkEndpoint {
            ws_url: Some(url),
            cert_policy: NoiseCertPolicy::Strict,
            admin_scan_url: None,
            is_mock: false,
        };
    }
    if let Some(url) = mock_server_url {
        let admin_scan_url = mock_admin_scan_qr_url(&url);
        return BenchmarkEndpoint {
            ws_url: Some(url),
            cert_policy: NoiseCertPolicy::DangerSkipCertChainVerify,
            admin_scan_url,
            is_mock: true,
        };
    }
    BenchmarkEndpoint {
        ws_url: None,
        cert_policy: NoiseCertPolicy::Strict,
        admin_scan_url: None,
        is_mock: false,
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn resolve_benchmark_endpoint_from_env() -> BenchmarkEndpoint {
    resolve_benchmark_endpoint(
        non_empty_env("WHATSAPP_WS_URL"),
        non_empty_env("MOCK_SERVER_URL"),
    )
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{:<5}] [{}] - {}",
                wacore::time::now_utc().format("%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(async {
        // `--features danger-skip-tls-verify` is needed only to talk to the
        // mock's own-root TLS chain.
        let endpoint = resolve_benchmark_endpoint_from_env();
        if endpoint.is_mock {
            info!("Mock mode: Noise cert bypass scoped to this client");
        }
        let mut transport_factory = TokioWebSocketTransportFactory::new();
        if let Some(url) = endpoint.ws_url.as_ref() {
            transport_factory = transport_factory.with_url(url.clone());
        }
        let admin_scan_url = endpoint.admin_scan_url.clone();
        let http_client = UreqHttpClient::new();

        let builder = Bot::builder()
            .with_backend(InMemoryBackend::new())
            .with_transport_factory(transport_factory)
            .with_http_client(http_client)
            .with_runtime(TokioRuntime)
            .with_noise_cert_policy(endpoint.cert_policy);

        let bot = builder
            .on_event_for(
                &[
                    EventKind::Messages,
                    EventKind::PairingQrCode,
                    EventKind::Connected,
                    EventKind::LoggedOut,
                ],
                move |event, client| {
                    let admin_scan_url = admin_scan_url.clone();
                    async move {
                        match &*event {
                            Event::Messages(batch) => {
                                for m in batch {
                                    if m.message.text_content() != Some("ping") {
                                        continue;
                                    }
                                    let ctx = MessageContext::from_inbound(m, Arc::clone(&client));
                                    info!("Received text ping, sending pong...");

                                    let pong_text = format!("pong {}", ctx.info.id);
                                    if let Err(e) = ctx.reply(pong_text).await {
                                        error!("Failed to send pong reply: {}", e);
                                    }
                                }
                            }
                            Event::PairingQrCode(qr) => {
                                let code = &qr.code;
                                // Mirrors tests/e2e/src/lib.rs::spawn_qr_autoresponder_http.
                                // Set only in mock mode; otherwise manual scan below.
                                if let Some(url) = admin_scan_url.as_ref() {
                                    let http = UreqHttpClient::new();
                                    let req = HttpRequest {
                                        url: url.clone(),
                                        method: "POST".into(),
                                        headers: HashMap::new(),
                                        body: Some(code.as_bytes().to_vec().into()),
                                    };
                                    match http.execute(req).await {
                                        Ok(resp) if (200..300).contains(&resp.status_code) => {
                                            info!("Auto-paired with mock server via {url}");
                                        }
                                        Ok(resp) => {
                                            warn!(
                                                "mock admin POST returned status {}: {}",
                                                resp.status_code,
                                                String::from_utf8_lossy(&resp.body)
                                            );
                                        }
                                        Err(e) => {
                                            warn!("mock admin POST transport error: {e}");
                                        }
                                    }
                                } else {
                                    info!("Scan this QR code with WhatsApp:\n{code}");
                                }
                            }
                            Event::Connected(_) => {
                                info!("✅ Bot connected successfully!");
                            }
                            Event::LoggedOut(_) => {
                                error!("❌ Bot was logged out!");
                            }
                            _ => {}
                        }
                    }
                },
            )
            .build()
            .await
            .expect("Failed to build bot");

        bot.run().await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_strict_with_no_admin_post() {
        let endpoint = resolve_benchmark_endpoint(None, None);
        assert!(!endpoint.is_mock);
        assert_eq!(endpoint.cert_policy, NoiseCertPolicy::Strict);
        assert!(endpoint.ws_url.is_none());
        assert!(endpoint.admin_scan_url.is_none());
    }

    #[test]
    fn production_override_stays_strict_without_admin_post() {
        let endpoint =
            resolve_benchmark_endpoint(Some("wss://example.invalid/ws/chat".to_string()), None);
        assert!(!endpoint.is_mock);
        assert_eq!(endpoint.cert_policy, NoiseCertPolicy::Strict);
        assert_eq!(
            endpoint.ws_url.as_deref(),
            Some("wss://example.invalid/ws/chat")
        );
        assert!(endpoint.admin_scan_url.is_none());
    }

    #[test]
    fn mock_selection_gets_bypass_and_admin_post() {
        let endpoint =
            resolve_benchmark_endpoint(None, Some("wss://127.0.0.1:8080/ws/chat".to_string()));
        assert!(endpoint.is_mock);
        assert_eq!(
            endpoint.cert_policy,
            NoiseCertPolicy::DangerSkipCertChainVerify
        );
        assert_eq!(
            endpoint.ws_url.as_deref(),
            Some("wss://127.0.0.1:8080/ws/chat")
        );
        assert_eq!(
            endpoint.admin_scan_url.as_deref(),
            Some("https://127.0.0.1:8080/admin/mock-phone/scan-qr")
        );
    }

    #[test]
    fn production_override_wins_over_mock() {
        let endpoint = resolve_benchmark_endpoint(
            Some("wss://example.invalid/ws/chat".to_string()),
            Some("wss://127.0.0.1:8080/ws/chat".to_string()),
        );
        assert!(!endpoint.is_mock);
        assert_eq!(endpoint.cert_policy, NoiseCertPolicy::Strict);
        assert_eq!(
            endpoint.ws_url.as_deref(),
            Some("wss://example.invalid/ws/chat")
        );
        assert!(
            endpoint.admin_scan_url.is_none(),
            "a production override must never inherit the mock admin POST"
        );
    }

    #[test]
    fn blank_values_count_as_unset() {
        let endpoint = resolve_benchmark_endpoint(Some("  ".to_string()), Some("".to_string()));
        assert!(!endpoint.is_mock);
        assert_eq!(endpoint.cert_policy, NoiseCertPolicy::Strict);
        assert!(endpoint.ws_url.is_none());
    }

    #[test]
    fn admin_url_scheme_mapping() {
        assert_eq!(
            mock_admin_scan_qr_url("wss://127.0.0.1:8080/ws/chat").as_deref(),
            Some("https://127.0.0.1:8080/admin/mock-phone/scan-qr")
        );
        assert_eq!(
            mock_admin_scan_qr_url("ws://127.0.0.1:8080/ws/chat").as_deref(),
            Some("http://127.0.0.1:8080/admin/mock-phone/scan-qr")
        );
        for non_ws in [
            "https://127.0.0.1:8080/ws/chat",
            "http://127.0.0.1:8080/ws/chat",
            "ftp://127.0.0.1:8080/ws/chat",
            "not a url",
            "wss://",
        ] {
            assert!(
                mock_admin_scan_qr_url(non_ws).is_none(),
                "{non_ws:?} must not yield an admin URL"
            );
        }
    }

    #[test]
    fn admin_url_preserves_query_and_drops_fragment() {
        assert_eq!(
            mock_admin_scan_qr_url("wss://127.0.0.1:8080?session=1").as_deref(),
            Some("https://127.0.0.1:8080/admin/mock-phone/scan-qr?session=1")
        );
        assert_eq!(
            mock_admin_scan_qr_url("wss://127.0.0.1:8080/ws/chat#frag").as_deref(),
            Some("https://127.0.0.1:8080/admin/mock-phone/scan-qr")
        );
    }

    #[test]
    fn admin_url_handles_ipv6_authority() {
        assert_eq!(
            mock_admin_scan_qr_url("wss://[::1]:8080/ws/chat").as_deref(),
            Some("https://[::1]:8080/admin/mock-phone/scan-qr")
        );
    }
}
