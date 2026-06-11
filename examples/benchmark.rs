use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use wacore::net::{HttpClient, HttpRequest};
use wacore::proto_helpers::MessageExt;
use wacore::store::InMemoryBackend;
use wacore::types::events::{Event, EventKind};
use whatsapp_rust::TokioRuntime;
use whatsapp_rust::bot::{Bot, MessageContext};
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

/// Derive the mock-server admin scan-qr endpoint from a `ws[s]://host:port/...`
/// WebSocket URL. Same host/port, scheme `ws`→`http` / `wss`→`https`, path
/// `/admin/mock-phone/scan-qr`. Mirrors `tests/e2e/src/lib.rs`. Returns `None`
/// for URLs that don't match the ws scheme — the autoresponder would
/// no-op on real WhatsApp anyway, but skipping the POST keeps logs clean.
fn mock_admin_scan_qr_url(ws_url: &str) -> Option<String> {
    let http_scheme = if ws_url.starts_with("wss://") {
        "https://"
    } else if ws_url.starts_with("ws://") {
        "http://"
    } else {
        return None;
    };
    let after_scheme = ws_url.split("://").nth(1)?;
    let host_port = after_scheme.split('/').next()?;
    Some(format!("{http_scheme}{host_port}/admin/mock-phone/scan-qr"))
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
        // Accept either WHATSAPP_WS_URL or MOCK_SERVER_URL — the latter
        // matches the convention the e2e suite uses.
        let configured_ws_url = std::env::var("WHATSAPP_WS_URL")
            .ok()
            .or_else(|| std::env::var("MOCK_SERVER_URL").ok());
        let mut transport_factory = TokioWebSocketTransportFactory::new();
        if let Some(url) = configured_ws_url.as_ref() {
            transport_factory = transport_factory.with_url(url.clone());
        }
        // Pre-derive the admin scan-qr URL so the on_event closure can
        // auto-pair against a mock server. None for real WhatsApp (or any
        // non-ws URL) — the closure simply skips the POST in that case.
        let admin_scan_url = configured_ws_url
            .as_deref()
            .and_then(mock_admin_scan_qr_url);
        let http_client = UreqHttpClient::new();

        let builder = Bot::builder()
            .with_backend(InMemoryBackend::new())
            .with_transport_factory(transport_factory)
            .with_http_client(http_client)
            .with_runtime(TokioRuntime);

        let bot = builder
            .on_event_for(
                &[
                    EventKind::Message,
                    EventKind::PairingQrCode,
                    EventKind::Connected,
                    EventKind::LoggedOut,
                ],
                move |event, client| {
                    let admin_scan_url = admin_scan_url.clone();
                    async move {
                        match &*event {
                            Event::Message(msg, info) => {
                                if let Some(text) = msg.text_content()
                                    && text == "ping"
                                {
                                    let ctx =
                                        MessageContext::from_arc(Arc::clone(msg), info, client);
                                    info!("Received text ping, sending pong...");

                                    let pong_text = format!("pong {}", ctx.info.id);
                                    if let Err(e) = ctx.reply(pong_text).await {
                                        error!("Failed to send pong reply: {}", e);
                                    }
                                }
                            }
                            Event::PairingQrCode { code, .. } => {
                                // Mirrors tests/e2e/src/lib.rs::spawn_qr_autoresponder_http.
                                // Auto-pair against the mock server's admin endpoint
                                // when the configured WS URL looks like a mock
                                // server; real WhatsApp connections fall back to
                                // manual scan via the printed code below.
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
