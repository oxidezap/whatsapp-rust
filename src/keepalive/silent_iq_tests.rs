use super::*;
use crate::test_utils::{create_iq_test_client, decode_sent_iq, log_capture, node_to_owned_ref};
use wacore::time::{MonotonicProvider, set_monotonic_provider};
use wacore_binary::builder::NodeBuilder;

/// Tokio's paused clock must drive BOTH the IQ timers and SessionStats. Merely
/// pausing Tokio leaves the default wacore monotonic clock on real time and
/// silently prevents this test from reaching the 20-second watchdog.
struct PausedClock(tokio::time::Instant);

impl MonotonicProvider for PausedClock {
    fn now_nanos(&self) -> u64 {
        tokio::time::Instant::now()
            .saturating_duration_since(self.0)
            .as_nanos()
            .try_into()
            .expect("test duration fits u64 nanoseconds")
    }
}

fn isolate(test_name: &str) -> bool {
    // The logger AND the clock are process globals. Reuse the existing
    // single-test child harness; nextest already provides that isolation.
    let test_name = test_name.strip_prefix("whatsapp_rust::").unwrap();
    if log_capture::delegated_to_child(test_name) {
        return true;
    }
    set_monotonic_provider(PausedClock(tokio::time::Instant::now()))
        .expect("isolated test must install its clock before constructing a client");
    false
}

fn request(
    client: &Arc<Client>,
    id: &'static str,
    timeout: Option<Duration>,
) -> tokio::task::JoinHandle<Result<Arc<wacore_binary::OwnedNodeRef>, IqError>> {
    let client = client.clone();
    tokio::spawn(async move {
        client
            .send_iq_node(
                NodeBuilder::new("iq")
                    .attr("id", id)
                    .attr("to", wacore_binary::Jid::new("", wacore_binary::Server::Pn))
                    .attr("type", "get")
                    .attr("xmlns", "test:ignored")
                    .build(),
                timeout,
            )
            .await
    })
}

async fn receive_result(client: &Arc<Client>, id: &str) {
    // Mirror data-arrival accounting, then use the actual node router and IQ
    // waiter resolver. No direct completion of a response_waiters entry.
    client.stats.mark_recv_activity();
    client
        .process_node(node_to_owned_ref(
            &NodeBuilder::new("iq")
                .attr("id", id)
                .attr(
                    "from",
                    wacore_binary::Jid::new("", wacore_binary::Server::Pn),
                )
                .attr("type", "result")
                .attr("t", "1760000000")
                .build(),
        ))
        .await;
}

fn start_keepalive(client: &Arc<Client>) -> tokio::task::JoinHandle<()> {
    let shutdown = client.connection_shutdown_signal();
    let generation = client.connection_generation.load(Ordering::Acquire);
    let client = client.clone();
    tokio::spawn(async move { client.keepalive_loop(shutdown, generation).await })
}

#[tokio::test(start_paused = true)]
async fn silent_iq_times_out_without_aborting_another_request_when_ping_responds() {
    if isolate(concat!(
        module_path!(),
        "::silent_iq_times_out_without_aborting_another_request_when_ping_responds"
    )) {
        return;
    }
    let logs = log_capture::session();
    let (client, transport) = create_iq_test_client().await;
    let silent = request(&client, "SILENT", None);
    decode_sent_iq(&transport, 0).await;
    let started = wacore::time::Instant::now();
    let other = request(&client, "OTHER", Some(Duration::from_secs(150)));
    decode_sent_iq(&transport, 1).await;
    let keepalive = start_keepalive(&client);

    let peer = {
        let client = client.clone();
        let transport = transport.clone();
        tokio::spawn(async move {
            let mut index = 2;
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                while index < transport.sent_count() {
                    let node = decode_sent_iq(&transport, index).await;
                    assert_eq!(
                        node.get().attrs().optional_string("xmlns").as_deref(),
                        Some("w:p")
                    );
                    let id = node
                        .get()
                        .attrs()
                        .optional_string("id")
                        .unwrap()
                        .into_owned();
                    assert_ne!(id, "SILENT");
                    assert_ne!(id, "OTHER");
                    receive_result(&client, &id).await;
                    index += 1;
                }
            }
        })
    };

    let result = tokio::time::timeout(Duration::from_secs(90), silent)
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(result, Err(IqError::Timeout)),
        "an ignored application IQ must fail locally when the connection answers probes, got {result:?}"
    );
    assert!(started.elapsed() >= crate::request::DEFAULT_IQ_TIMEOUT);
    assert!(
        !other.is_finished(),
        "the unrelated request must remain pending"
    );
    assert!(
        !keepalive.is_finished(),
        "the keepalive must remain on the original connection"
    );
    assert!(
        transport.sent_count() >= 3,
        "the peer must have answered a real encoded probe"
    );
    assert!(!client.response_waiters_guard().contains_key("SILENT"));
    assert!(client.response_waiters_guard().contains_key("OTHER"));
    receive_result(&client, "OTHER").await;
    assert!(other.await.unwrap().is_ok());
    assert!(
        logs.records_for("whatsapp_rust::client::lifecycle")
            .iter()
            .all(|(_, message)| !message.contains("Reconnecting immediately"))
    );

    peer.abort();
    client.notify_connection_shutdown();
    keepalive.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn silent_socket_still_reconnects_after_a_bounded_probe() {
    if isolate(concat!(
        module_path!(),
        "::silent_socket_still_reconnects_after_a_bounded_probe"
    )) {
        return;
    }
    let logs = log_capture::session();
    let (client, transport) = create_iq_test_client().await;
    let silent = request(&client, "SILENT", None);
    decode_sent_iq(&transport, 0).await;
    let started = wacore::time::Instant::now();
    let first_send = client.stats.first_send_since_recv();
    let keepalive = start_keepalive(&client);

    tokio::time::timeout(Duration::from_secs(74), keepalive)
        .await
        .expect("a black-holed socket must not wait indefinitely for the application IQ")
        .unwrap();
    assert!(matches!(silent.await.unwrap(), Err(IqError::NotConnected)));
    assert!(started.elapsed() >= wacore::protocol::keepalive::DEAD_SOCKET_TIME);
    assert!(started.elapsed() < crate::request::DEFAULT_IQ_TIMEOUT);
    assert_eq!(
        transport.sent_count(),
        2,
        "one ignored IQ plus one unanswered liveness probe"
    );
    assert_eq!(
        client.stats.first_send_since_recv(),
        first_send,
        "probing must not re-arm the first unanswered-send anchor"
    );
    assert!(client.response_waiters_guard().is_empty());
    let warnings = logs.records_for("Client/Keepalive");
    assert!(
        warnings
            .iter()
            .any(|(level, message)| *level == log::Level::Warn
                && message.contains("watchdog")
                && message.contains("pending response waiters: 1"))
    );
    assert!(
        warnings
            .iter()
            .all(|(_, message)| !message.contains("(dead socket)"))
    );
}

#[tokio::test(start_paused = true)]
async fn recent_inbound_traffic_keeps_pending_iqs_from_causing_extra_pings() {
    if isolate(concat!(
        module_path!(),
        "::recent_inbound_traffic_keeps_pending_iqs_from_causing_extra_pings"
    )) {
        return;
    }
    let _logs = log_capture::session();
    let (client, transport) = create_iq_test_client().await;
    let silent = request(&client, "SILENT", Some(Duration::from_secs(120)));
    decode_sent_iq(&transport, 0).await;
    client.stats.mark_recv_activity();
    let keepalive = start_keepalive(&client);
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_secs(10)).await;
        client.stats.mark_recv_activity();
    }
    assert_eq!(
        transport.sent_count(),
        1,
        "recent receives already prove liveness"
    );
    assert!(!keepalive.is_finished());
    assert!(!silent.is_finished());
    receive_result(&client, "SILENT").await;
    assert!(silent.await.unwrap().is_ok());
    client.notify_connection_shutdown();
    keepalive.await.unwrap();
}

#[tokio::test(start_paused = true)]
async fn short_request_timeout_does_not_wait_for_the_watchdog_or_a_probe() {
    if isolate(concat!(
        module_path!(),
        "::short_request_timeout_does_not_wait_for_the_watchdog_or_a_probe"
    )) {
        return;
    }
    let _logs = log_capture::session();
    let (client, transport) = create_iq_test_client().await;
    let silent = request(&client, "SHORT", Some(Duration::from_secs(8)));
    decode_sent_iq(&transport, 0).await;
    let keepalive = start_keepalive(&client);
    assert!(matches!(silent.await.unwrap(), Err(IqError::Timeout)));
    assert_eq!(transport.sent_count(), 1);
    assert!(!keepalive.is_finished());
    assert!(client.response_waiters_guard().is_empty());
    client.notify_connection_shutdown();
    keepalive.await.unwrap();
}

/// A client whose socket takes writes and never finishes them: established,
/// but no longer draining, as a TCP connection is after the laptop under it
/// resumed from suspend with its route gone.
async fn wedged_client() -> (
    Arc<Client>,
    Arc<crate::transport::mock::StallingMockTransport>,
) {
    use wacore::handshake::NoiseCipher;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(crate::transport::mock::StallingMockTransport::new());
    let noise_socket = crate::socket::NoiseSocket::with_observers(
        client.runtime.clone(),
        transport.clone() as Arc<dyn crate::transport::Transport>,
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        crate::socket::noise_socket::SendObservers::with_stats(client.stats.clone()),
    );
    *client.transport.lock().await =
        Some(transport.clone() as Arc<dyn crate::transport::Transport>);
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.set_connected_for_test(true);
    client.is_running.store(true, Ordering::Release);
    // The link carried traffic until the write wedged.
    client.stats.mark_recv_activity();
    (client, transport)
}

#[tokio::test(start_paused = true)]
async fn an_iq_whose_write_never_completes_times_out() {
    if isolate(concat!(
        module_path!(),
        "::an_iq_whose_write_never_completes_times_out"
    )) {
        return;
    }
    let (client, transport) = wedged_client().await;
    let started = tokio::time::Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(300),
        request(&client, "WEDGED", Some(Duration::from_secs(20))),
    )
    .await
    .expect("the IQ deadline must cover a write that never completes")
    .unwrap();
    assert!(transport.sends_started() >= 1);
    assert!(
        matches!(result, Err(IqError::Timeout)),
        "a wedged write must surface as the IQ's timeout, got {result:?}"
    );
    assert!(started.elapsed() <= Duration::from_secs(21));
}

#[tokio::test(start_paused = true)]
async fn keepalive_reconnects_when_its_ping_write_never_completes() {
    if isolate(concat!(
        module_path!(),
        "::keepalive_reconnects_when_its_ping_write_never_completes"
    )) {
        return;
    }
    let (client, transport) = wedged_client().await;
    let started = tokio::time::Instant::now();
    tokio::time::timeout(Duration::from_secs(600), start_keepalive(&client))
        .await
        .expect("the keepalive must give up on a ping it cannot even write")
        .unwrap();
    assert!(
        transport.disconnects_started() >= 1,
        "the keepalive must have torn the wedged connection down"
    );
    // Armed when the first ping entered the transport, the dead-socket
    // watchdog fires on the tick after that ping's deadline: at most two
    // intervals and two answer deadlines, not three unanswered pings.
    assert!(
        started.elapsed() <= 2 * (KEEP_ALIVE_INTERVAL_MAX + KEEP_ALIVE_RESPONSE_DEADLINE),
        "took {:?}",
        started.elapsed()
    );
}
