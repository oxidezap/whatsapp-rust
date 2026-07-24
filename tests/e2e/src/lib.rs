// `--all-features` feature-unifies extra deps onto `whatsapp-rust`, enlarging the
// `send_and_expect_text()` future past the default layout-query depth. Matches the
// `512` already used in `src/lib.rs` / `examples/demo.rs`.
#![recursion_limit = "512"]
// Harness diagnostics. This lives in a lib target rather than under
// `#[cfg(test)]`, so clippy's allow-print-in-tests does not reach it.
#![allow(clippy::print_stderr)]

use std::collections::HashMap;
use std::sync::Arc;

use wacore::net::{HttpClient, HttpRequest};
use wacore::store::InMemoryBackend;
use wacore::store::traits::{AppSyncStore, TcTokenEntry};
use wacore::types::events::{ChannelEventHandler, Event};
use wacore_binary::node::Node;
use whatsapp_rust::Jid;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

/// Returns the mock server WebSocket URL from env, or the default.
pub fn mock_server_url() -> String {
    std::env::var("MOCK_SERVER_URL").unwrap_or_else(|_| "wss://127.0.0.1:8080/ws/chat".to_string())
}

/// Translate the `MOCK_SERVER_URL` (a `ws[s]://host:port/ws/chat` WebSocket
/// URL) to the matching admin HTTP URL for the QR-scan endpoint exposed by
/// bartender. Same host/port, scheme `ws`→`http` / `wss`→`https`, path
/// `/admin/mock-phone/scan-qr`.
fn mock_admin_scan_qr_url() -> String {
    let ws = mock_server_url();
    let http_scheme = if ws.starts_with("wss://") {
        "https://"
    } else {
        "http://"
    };
    let after_scheme = ws.split("://").nth(1).unwrap_or(&ws);
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);
    format!("{http_scheme}{host_port}/admin/mock-phone/scan-qr")
}

/// Spawn an in-test "phone" that watches `event_rx` for the first
/// `Event::PairingQrCode` and POSTs the QR string to the mock-server's
/// admin endpoint. This is the out-of-process equivalent of bartender's
/// in-process `spawn_qr_autoresponder`. Idempotent: stops after one scan.
fn spawn_qr_autoresponder_http(
    event_rx: async_channel::Receiver<Arc<Event>>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let url = mock_admin_scan_qr_url();
        let http = UreqHttpClient::new();
        while let Ok(event) = event_rx.recv().await {
            if let Event::PairingQrCode(qr) = &*event {
                let req = HttpRequest {
                    url: url.clone(),
                    method: "POST".into(),
                    headers: HashMap::new(),
                    body: Some(qr.code.as_bytes().to_vec().into()),
                };
                match http.execute(req).await {
                    Ok(resp) if (200..300).contains(&resp.status_code) => return,
                    Ok(resp) => {
                        eprintln!(
                            "qr-autoresponder: admin POST returned status {}: {}",
                            resp.status_code,
                            String::from_utf8_lossy(&resp.body)
                        );
                        return;
                    }
                    Err(e) => {
                        eprintln!("qr-autoresponder: admin POST transport error: {e}");
                        return;
                    }
                }
            }
        }
    })
}

pub fn unique_push_name(prefix: &str) -> String {
    format!("{}_{}", prefix, uuid::Uuid::new_v4())
}

pub fn restricted_push_name(prefix: &str) -> String {
    format!("restricted:{}", unique_push_name(prefix))
}

pub fn scenario_push_name(prefix: &str, flags: &[&str]) -> String {
    assert!(
        !flags.is_empty(),
        "scenario_push_name requires at least one flag"
    );
    format!("scenario:{}:{}", flags.join(","), unique_push_name(prefix))
}

/// A connected client ready for testing, with its event receiver and run handle.
pub struct TestClient {
    pub client: Arc<whatsapp_rust::client::Client>,
    pub event_rx: async_channel::Receiver<Arc<Event>>,
    pub run_handle: whatsapp_rust::bot::BotHandle,
    /// The concrete backend, retained for its test hooks
    /// (`session_batch_write_count`, `set_fail_session_writes`).
    pub backend: Arc<InMemoryBackend>,
}

async fn connect_diagnostics(
    client: &whatsapp_rust::client::Client,
    backend: &InMemoryBackend,
) -> String {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

    match tokio::time::timeout(TIMEOUT, collect_connect_diagnostics(client, backend)).await {
        Ok(diagnostics) => diagnostics,
        Err(_) => format!(
            "diagnostics_timed_out_after={}s, socket_connected={}, logged_in={}",
            TIMEOUT.as_secs(),
            client.is_connected(),
            client.is_logged_in(),
        ),
    }
}

async fn collect_connect_diagnostics(
    client: &whatsapp_rust::client::Client,
    backend: &InMemoryBackend,
) -> String {
    async fn session_status(
        client: &whatsapp_rust::client::Client,
        jid: Option<Jid>,
    ) -> &'static str {
        match jid {
            Some(jid) => match client.signal().validate_session(&jid).await {
                Ok(true) => "present",
                Ok(false) => "absent",
                Err(_) => "unknown",
            },
            None => "unavailable",
        }
    }

    let snapshot = client.persistence_manager().get_device_snapshot();
    let primary_pn = snapshot.pn.as_ref().map(|jid| jid.to_non_ad());
    let primary_lid = snapshot.lid.as_ref().map(|jid| jid.to_non_ad());
    let has_pn = snapshot.pn.is_some();
    let has_lid = snapshot.lid.is_some();
    drop(snapshot);

    let (primary_pn_session, primary_lid_session) = futures::join!(
        session_status(client, primary_pn),
        session_status(client, primary_lid),
    );
    let sync_keys = backend.sync_key_count_for_test().await;
    let report = client.memory_report().await;

    format!(
        "socket_connected={}, logged_in={}, has_pn={}, has_lid={}, sync_keys={}, \
         primary_pn_session={}, primary_lid_session={}, app_state_requests={}, app_state_syncs={}",
        client.is_connected(),
        client.is_logged_in(),
        has_pn,
        has_lid,
        sync_keys,
        primary_pn_session,
        primary_lid_session,
        report.app_state_key_requests,
        report.app_state_syncing,
    )
}

impl TestClient {
    /// Create a client, connect to the mock server, and wait for PairSuccess + Connected.
    /// Returns the connected TestClient with its JID available via `client.pn()`.
    pub async fn connect(prefix: &str) -> anyhow::Result<Self> {
        Self::connect_inner(prefix, Some(unique_push_name(prefix))).await
    }

    /// Connect without pre-seeding a push name.
    ///
    /// Use only for tests that explicitly cover the fresh-pairing app-state
    /// path where push name arrives from server sync.
    pub async fn connect_without_push_name(prefix: &str) -> anyhow::Result<Self> {
        Self::connect_inner(prefix, None).await
    }

    /// Connect with a specific push_name for deterministic phone assignment.
    ///
    /// Two clients with the same `push_name` will be paired to the same phone number
    /// with different device IDs, enabling multi-device testing.
    pub async fn connect_as(prefix: &str, push_name: &str) -> anyhow::Result<Self> {
        Self::connect_inner(prefix, Some(push_name.to_string())).await
    }

    async fn connect_inner(prefix: &str, push_name: Option<String>) -> anyhow::Result<Self> {
        let transport_factory = TokioWebSocketTransportFactory::new().with_url(mock_server_url());
        let (event_handler, event_rx) = ChannelEventHandler::new();

        let backend = Arc::new(InMemoryBackend::new());
        let mut builder = Bot::builder()
            .with_backend_arc(backend.clone())
            .with_transport_factory(transport_factory)
            .with_http_client(UreqHttpClient::new())
            .with_runtime(whatsapp_rust::TokioRuntime)
            .with_version((2, 3000, 0));

        let push_name_pre_seeded = push_name.is_some();
        if let Some(name) = push_name {
            builder = builder.with_push_name(name);
        }

        let bot = builder.build().await?;

        let client = bot.client();
        // with_push_name pre-seeds the name so the setting_pushName mutation has old==new (skipping auto set_available), so force active to keep delivery receipts from being type="inactive".
        if push_name_pre_seeded {
            client.set_force_active_delivery_receipts(true);
        }
        client.subscribe_handler(event_handler).detach();

        // The mock server no longer auto-pairs (legacy timer is off by
        // default). Spawn an out-of-process "phone" that POSTs the first
        // QR this client emits to the admin endpoint, mirroring the
        // in-process autoresponder used by bartender's own e2e suite.
        // Uses its own ChannelEventHandler because async_channel is MPMC:
        // sharing event_rx would steal events from wait_for_event below.
        let (qr_handler, qr_rx) = ChannelEventHandler::new();
        client.subscribe_handler(qr_handler).detach();
        let _qr_responder = spawn_qr_autoresponder_http(qr_rx);

        let run_handle = bot.spawn();

        // Readiness gate: `wait_for_connected` resolves on the canonical
        // `is_ready` signal (`dispatch_connected`, after the critical sync) via
        // a notifier, so it does not race event arrival order or fall back to an
        // orthogonal signal — the earlier flake, where a fixed 30s wait for
        // `Connected` timed out under CI load. PairSuccess/Connected still land
        // in the unbounded `event_rx`; the predicate-filtered `wait_for_event`
        // discards them.
        if let Err(e) = client
            .wait_for_connected(tokio::time::Duration::from_secs(60))
            .await
        {
            let diagnostics = connect_diagnostics(&client, &backend).await;
            client.disconnect().await;
            drop(run_handle);
            return Err(anyhow::anyhow!(
                "{prefix}: client never became ready after pairing ({diagnostics}): {e}"
            ));
        }

        // Drain the initial startup sync (offline messages + history) so tests
        // start from a quiescent state. This is a hard requirement, not
        // best-effort: a timeout here means a real startup hang or a mid-sync
        // client that would make assertions race changing state, so it fails
        // the connect.
        if let Err(e) = client
            .wait_for_startup_sync(tokio::time::Duration::from_secs(30))
            .await
        {
            let diagnostics = connect_diagnostics(&client, &backend).await;
            client.disconnect().await;
            drop(run_handle);
            return Err(anyhow::anyhow!(
                "{prefix}: startup sync did not settle ({diagnostics}): {e}"
            ));
        }

        Ok(Self {
            client,
            event_rx,
            run_handle,
            backend,
        })
    }

    // ── JID helpers ─────────────────────────────────────────────────────────

    /// Get this client's phone number JID (non-AD format).
    pub async fn jid(&self) -> Jid {
        self.client
            .pn()
            .expect("Client should have a JID after connect")
            .to_non_ad()
    }

    /// Get the storage key used for this client's tcToken entries.
    ///
    /// Notification handling stores tcTokens under the sender's LID when it is
    /// available, otherwise it falls back to the phone-number user part.
    pub async fn tc_token_key(&self) -> anyhow::Result<String> {
        if let Some(lid) = self.client.lid() {
            return Ok(lid.user.to_string());
        }

        self.client
            .pn()
            .map(|jid| jid.user.to_string())
            .ok_or_else(|| anyhow::anyhow!("Client should have a JID after connect"))
    }

    /// Wait until a tcToken entry exists for the given storage key.
    pub async fn wait_for_tc_token(
        &self,
        jid_key: &str,
        timeout_secs: u64,
    ) -> anyhow::Result<TcTokenEntry> {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

        loop {
            if let Some(entry) = self.client.tc_token().get(jid_key).await? {
                return Ok(entry);
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timed out waiting for tc_token entry for {}",
                    jid_key
                ));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    pub fn sent_message_waiter(
        &self,
        msg_id: &str,
    ) -> futures::channel::oneshot::Receiver<Arc<Node>> {
        self.client
            .wait_for_sent_node(whatsapp_rust::NodeFilter::tag("message").attr("id", msg_id))
    }

    pub fn next_sent_message_waiter(&self) -> futures::channel::oneshot::Receiver<Arc<Node>> {
        self.client
            .wait_for_sent_node(whatsapp_rust::NodeFilter::tag("message"))
    }

    pub async fn nct_salt(&self) -> Option<Vec<u8>> {
        self.client
            .persistence_manager()
            .get_device_snapshot()
            .nct_salt
            .clone()
    }

    pub async fn wait_for_nct_salt(&self, timeout_secs: u64) -> anyhow::Result<Vec<u8>> {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

        loop {
            if let Some(salt) = self.nct_salt().await {
                return Ok(salt);
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!("Timed out waiting for NCT salt"));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    }

    // ── Event waiting ───────────────────────────────────────────────────────

    /// Wait for an event matching the predicate, with a timeout in seconds.
    pub async fn wait_for_event<F>(
        &mut self,
        timeout_secs: u64,
        mut predicate: F,
    ) -> anyhow::Result<Arc<Event>>
    where
        F: FnMut(&Event) -> bool,
    {
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        tokio::time::timeout(timeout, async {
            loop {
                match self.event_rx.recv().await {
                    Ok(event) if predicate(&event) => return Ok(event),
                    Ok(_) => continue,
                    Err(e) => return Err(anyhow::anyhow!("Event channel closed: {e}")),
                }
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for event"))?
    }

    /// Wait for a text message with specific content.
    pub async fn wait_for_text(
        &mut self,
        text: &str,
        timeout_secs: u64,
    ) -> anyhow::Result<Arc<Event>> {
        let text = text.to_string();
        // Match anywhere in the batch: a drain event can carry the target
        // text behind other messages.
        self.wait_for_event(timeout_secs, move |e| {
            e.messages()
                .any(|m| m.message.conversation.as_deref() == Some(text.as_str()))
        })
        .await
    }

    /// Wait for a text message on a specific group.
    pub async fn wait_for_group_text(
        &mut self,
        group_jid: &Jid,
        text: &str,
        timeout_secs: u64,
    ) -> anyhow::Result<Arc<Event>> {
        let gid = group_jid.clone();
        let text = text.to_string();
        self.wait_for_event(timeout_secs, move |e| {
            e.messages().any(|m| {
                m.info.source.chat == gid
                    && m.message.conversation.as_deref() == Some(text.as_str())
            })
        })
        .await
    }

    /// Wait for a w:gp2 group notification.
    pub async fn wait_for_group_notification(
        &mut self,
        timeout_secs: u64,
    ) -> anyhow::Result<Arc<Event>> {
        self.wait_for_event(timeout_secs, |e| {
            matches!(e, Event::Notification(node) if node.get().get_attr("type").is_some_and(|v| v.as_str() == "w:gp2"))
        })
        .await
    }

    /// Assert that NO event matching the predicate arrives within the timeout.
    /// Returns Ok(()) if the wait times out (expected), panics if an event arrives.
    pub async fn assert_no_event<F>(
        &mut self,
        timeout_secs: u64,
        predicate: F,
        context: &str,
    ) -> anyhow::Result<()>
    where
        F: FnMut(&Event) -> bool,
    {
        let result = self.wait_for_event(timeout_secs, predicate).await;
        match result {
            Ok(event) => panic!("{context}: expected no event but got: {event:?}"),
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("Timed out"),
                    "{context}: expected timeout error, got: {msg}"
                );
                Ok(())
            }
        }
    }

    /// Wait for initial app state sync to complete (keys become available).
    pub async fn wait_for_app_state_sync(&mut self) -> anyhow::Result<()> {
        let push_name = self.client.push_name();
        if !push_name.is_empty() {
            return Ok(());
        }
        self.wait_for_event(10, |e| matches!(e, Event::SelfPushNameUpdated(_)))
            .await?;
        Ok(())
    }

    pub async fn wait_for_sync_key(&self, key_id: &[u8], timeout_secs: u64) -> anyhow::Result<()> {
        tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), async {
            loop {
                if self.backend.get_sync_key(key_id).await?.is_some() {
                    return Ok::<_, anyhow::Error>(());
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Timed out after {}s waiting for app-state sync key {key_id:02x?}",
                timeout_secs,
            )
        })?
    }

    // ── Connection lifecycle ────────────────────────────────────────────────

    /// Wait until the current connection is torn down.
    ///
    /// `reconnect()` / `reconnect_immediately()` only ask the transport to
    /// close; the client is still bookkeeping-connected until the run loop
    /// finishes `cleanup_connection_state`. Offline tests must observe that
    /// transition before acting, and polling it is bounded and self-reporting
    /// where a fixed sleep is neither.
    pub async fn wait_for_disconnected(&self, timeout_secs: u64) -> anyhow::Result<()> {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);

        loop {
            if !self.client.is_connected() {
                return Ok(());
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow::anyhow!(
                    "Timed out after {timeout_secs}s waiting for the client to go offline"
                ));
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
        }
    }

    /// Reconnect and wait for the Connected event.
    pub async fn reconnect_and_wait(&mut self) -> anyhow::Result<()> {
        // Drain any buffered Connected events from prior connections
        while let Ok(event) = self.event_rx.try_recv() {
            if matches!(&*event, Event::Connected(_)) {
                continue;
            }
        }
        // This helper is for tests that only need a fresh online connection.
        // Offline-window tests call `reconnect()` directly.
        self.client.reconnect_immediately().await;
        self.wait_for_event(10, |e| matches!(e, Event::Connected(_)))
            .await?;
        Ok(())
    }

    /// Disconnect and wait for the run task to complete cleanly.
    pub async fn disconnect(self) {
        self.client.disconnect().await;
        let run_handle = self.run_handle;

        match tokio::time::timeout(tokio::time::Duration::from_secs(5), run_handle).await {
            Ok(_) => {}
            Err(_) => {
                eprintln!("WARN: timed out waiting for client run task shutdown");
                // BotHandle's Drop aborts the task automatically
            }
        }
    }
}

// ── Free-standing test helpers ──────────────────────────────────────────────

/// True when `node` has a direct child with the given tag.
pub fn has_child(node: &Node, tag: &str) -> bool {
    node.children()
        .map(|children| children.iter().any(|child| child.tag == tag))
        .unwrap_or(false)
}

/// Backend session-store key for a peer's `(user, server, device)`, matching the
/// `<user>[:dev]@<server>.0` protocol-address form the Signal store keys on.
pub fn peer_session_addr(user: &str, server: &str, device: u16) -> String {
    if device == 0 {
        format!("{user}@{server}.0")
    } else {
        format!("{user}:{device}@{server}.0")
    }
}

/// Scan the backend for sessions matching `user` across device IDs 0..=99,
/// returning `(address, has_pending_pre_key)` for each one found.
///
/// The range must cover the peer's real companion device (a paired client is a
/// non-zero device, e.g. 33), not just low ids — otherwise the only session
/// found is the phantom device-0 one, whose pending_pre_key never clears
/// (device 0 never completes the X3DH handshake).
///
/// A record that deserializes without a current state is still reported (with
/// `has_pending_pre_key = false`), so absence in the result means "no session
/// stored", which is what the LID-only assertions rely on.
pub async fn scan_sessions(
    backend: &dyn wacore::store::traits::SignalStore,
    user: &str,
    server: &str,
) -> anyhow::Result<Vec<(String, bool)>> {
    let mut results = Vec::new();
    for device_id in 0..=99u16 {
        let addr = peer_session_addr(user, server, device_id);
        if let Some(data) = backend.get_session(&addr).await? {
            let record = wacore::libsignal::protocol::SessionRecord::deserialize(&data)?;
            let has_pending = match record.session_state() {
                Some(state) => state
                    .unacknowledged_pre_key_message_items()
                    .map_err(|e| anyhow::anyhow!("invalid session state: {e}"))?
                    .is_some(),
                None => false,
            };
            results.push((addr, has_pending));
        }
    }
    Ok(results)
}

/// Build a simple text message.
pub fn text_msg(text: &str) -> wa::Message {
    wa::Message {
        conversation: Some(text.to_string()),
        ..Default::default()
    }
}

/// Send a text message and wait for the receiver to get it. Returns the message ID.
pub async fn send_and_expect_text(
    sender: &whatsapp_rust::client::Client,
    receiver: &mut TestClient,
    to: &Jid,
    text: &str,
    timeout_secs: u64,
) -> anyhow::Result<String> {
    let result = sender.send_message(to.clone(), text_msg(text)).await?;
    receiver.wait_for_text(text, timeout_secs).await?;
    Ok(result.message_id)
}
