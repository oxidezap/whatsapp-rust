use crate::client::{Client, ClientBuilder};
use crate::request::IqError;
use crate::runtime_impl::TokioRuntime;
use crate::store::persistence_manager::PersistenceManager;
use crate::test_utils::{MockHttpClient, create_test_backend, log_capture};
use crate::transport::{DisconnectReason, Transport, TransportEvent, TransportFactory};
use crate::waproto::whatsapp as wa;
use anyhow::Context;
use async_trait::async_trait;
use bytes::Bytes;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use wacore::handshake::NoiseHandshake;
use wacore::libsignal::protocol::KeyPair;
use wacore::protocol::keepalive::DEAD_SOCKET_TIME;
use wacore::time::{MonotonicProvider, set_monotonic_provider};
use wacore_binary::OwnedNodeRef;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::consts::{NOISE_PATTERN_XX, WA_CONN_HEADER};
use wacore_noise::test_util::build_cert_chain_bytes;

#[derive(Clone)]
struct LoopbackTcpFactory {
    address: SocketAddr,
    writes: Arc<AtomicUsize>,
    write_notify: Arc<tokio::sync::Notify>,
}

struct LoopbackTcpTransport {
    writer: tokio::sync::Mutex<tokio::net::tcp::OwnedWriteHalf>,
    writes: Arc<AtomicUsize>,
    write_notify: Arc<tokio::sync::Notify>,
}

#[async_trait]
impl Transport for LoopbackTcpTransport {
    async fn send(&self, data: Bytes) -> anyhow::Result<()> {
        self.writer.lock().await.write_all(&data).await?;
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.write_notify.notify_one();
        Ok(())
    }

    async fn disconnect(&self) {
        let _ = self.writer.lock().await.shutdown().await;
    }
}

#[async_trait]
impl TransportFactory for LoopbackTcpFactory {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        let stream = TcpStream::connect(self.address).await?;
        let (mut reader, writer) = stream.into_split();
        let (events_tx, events_rx) = async_channel::unbounded();
        events_tx.send(TransportEvent::Connected).await?;
        tokio::spawn(async move {
            let mut buf = vec![0; 8192];
            loop {
                match reader.read(&mut buf).await {
                    Ok(0) => {
                        let _ = events_tx
                            .send(TransportEvent::Disconnected(DisconnectReason::StreamEnded))
                            .await;
                        return;
                    }
                    Ok(n) => {
                        if events_tx
                            .send(TransportEvent::DataReceived(Bytes::copy_from_slice(
                                &buf[..n],
                            )))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                    Err(error) => {
                        let _ = events_tx
                            .send(TransportEvent::Disconnected(DisconnectReason::ReadError(
                                error.to_string(),
                            )))
                            .await;
                        return;
                    }
                }
            }
        });
        Ok((
            Arc::new(LoopbackTcpTransport {
                writer: tokio::sync::Mutex::new(writer),
                writes: self.writes.clone(),
                write_notify: self.write_notify.clone(),
            }),
            events_rx,
        ))
    }
}

const TIME_SCALE: u32 = 10;

struct TestClock(tokio::time::Instant);

impl MonotonicProvider for TestClock {
    fn now_nanos(&self) -> u64 {
        tokio::time::Instant::now()
            .saturating_duration_since(self.0)
            .as_nanos()
            .saturating_mul(TIME_SCALE.into())
            .try_into()
            .expect("scaled test duration fits u64 nanoseconds")
    }
}

struct AcceleratedRuntime;

#[async_trait]
impl wacore::runtime::Runtime for AcceleratedRuntime {
    fn spawn(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> wacore::runtime::AbortHandle {
        TokioRuntime.spawn(future)
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        TokioRuntime.sleep(duration.div_f64(f64::from(TIME_SCALE)))
    }

    fn spawn_blocking(
        &self,
        task: Box<dyn FnOnce() + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        TokioRuntime.spawn_blocking(task)
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        TokioRuntime.yield_now()
    }
}

fn isolate(test_name: &str) -> bool {
    let test_name = test_name.strip_prefix("whatsapp_rust::").unwrap();
    if log_capture::delegated_to_child(test_name) {
        return true;
    }
    set_monotonic_provider(TestClock(tokio::time::Instant::now()))
        .expect("the isolated test installs its clock before building a client");
    false
}

async fn connected_client(
    address: SocketAddr,
) -> (Arc<Client>, Arc<AtomicUsize>, Arc<tokio::sync::Notify>) {
    let persistence_manager = Arc::new(
        PersistenceManager::new(create_test_backend().await)
            .await
            .expect("in-memory backend initializes"),
    );
    let writes = Arc::new(AtomicUsize::new(0));
    let write_notify = Arc::new(tokio::sync::Notify::new());
    let build = ClientBuilder::new()
        .with_runtime(AcceleratedRuntime)
        .with_persistence_manager(persistence_manager)
        .with_transport_factory(LoopbackTcpFactory {
            address,
            writes: writes.clone(),
            write_notify: write_notify.clone(),
        })
        .with_http_client(MockHttpClient)
        .with_version_override((2, 3000, 0))
        .with_noise_cert_policy(crate::handshake::NoiseCertPolicy::DangerSkipCertChainVerify)
        .with_ab_props_fetch(false)
        .build()
        .await
        .expect("test client builds");
    let (client, _sync_rx) = build.into_parts();
    client.enter_live_mode_for_tests();
    (client, writes, write_notify)
}

async fn read_frame(
    stream: &mut TcpStream,
    with_connection_header: bool,
) -> anyhow::Result<Vec<u8>> {
    if with_connection_header {
        let mut header = vec![0; WA_CONN_HEADER.len()];
        stream.read_exact(&mut header).await?;
        anyhow::ensure!(
            header == WA_CONN_HEADER,
            "unexpected Noise connection header"
        );
    }
    let mut prefix = [0; 3];
    stream.read_exact(&mut prefix).await?;
    let len =
        (usize::from(prefix[0]) << 16) | (usize::from(prefix[1]) << 8) | usize::from(prefix[2]);
    let mut payload = vec![0; len];
    stream.read_exact(&mut payload).await?;
    Ok(payload)
}

async fn write_frame(stream: &mut TcpStream, payload: &[u8]) -> anyhow::Result<()> {
    stream
        .write_all(&wacore::framing::encode_frame(payload, None)?)
        .await?;
    Ok(())
}

async fn serve_noise_handshake(
    stream: &mut TcpStream,
) -> anyhow::Result<(
    wacore::handshake::NoiseCipher,
    wacore::handshake::NoiseCipher,
)> {
    let client_hello = waproto::codec::handshake_message_decode(&read_frame(stream, true).await?)?;
    let client_eph: [u8; 32] = client_hello
        .client_hello
        .into_option()
        .expect("client sends XX hello")
        .ephemeral
        .expect("client hello ephemeral")
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid client ephemeral length"))?;

    let server_static = KeyPair::generate(&mut rand::rng());
    let server_static_pub: [u8; 32] = server_static
        .public_key
        .public_key_bytes()
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid server static key length"))?;
    let cert_chain = build_cert_chain_bytes(&server_static_pub);
    let server_eph = KeyPair::generate(&mut rand::rng());
    let server_eph_pub: [u8; 32] = server_eph
        .public_key
        .public_key_bytes()
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid server ephemeral length"))?;

    let mut noise = NoiseHandshake::new(NOISE_PATTERN_XX, &WA_CONN_HEADER)?;
    noise.authenticate(&client_eph);
    noise.authenticate(&server_eph_pub);
    noise.mix_shared_secret(server_eph.private_key.serialize(), &client_eph)?;
    let encrypted_static = noise.encrypt(&server_static_pub)?;
    noise.mix_shared_secret(server_static.private_key.serialize(), &client_eph)?;
    let encrypted_payload = noise.encrypt(&cert_chain)?;
    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(server_eph_pub.to_vec()),
            r#static: Some(encrypted_static),
            payload: Some(encrypted_payload),
            ..Default::default()
        }),
        ..Default::default()
    };
    write_frame(
        stream,
        &waproto::codec::handshake_message_to_vec(&server_hello),
    )
    .await?;

    let client_finish =
        waproto::codec::handshake_message_decode(&read_frame(stream, false).await?)?;
    let finish = client_finish
        .client_finish
        .into_option()
        .expect("client completes XX handshake");
    let client_static = noise.decrypt(finish.r#static.as_deref().expect("client static"))?;
    noise.mix_shared_secret(server_eph.private_key.serialize(), &client_static)?;
    let _login_payload = noise.decrypt(finish.payload.as_deref().expect("client payload"))?;
    let (write, read) = noise.finish()?;
    // `NoiseHandshake::finish` yields initiator write/read keys; from the peer
    // these directions are reversed.
    Ok((write, read))
}

#[derive(Clone, Copy)]
enum PeerBehavior {
    AnswerProbes,
    IgnoreEverything,
    /// Sends `<success>` right after the handshake, as the server does, and
    /// answers every request the login sets off.
    LogInAndAnswerRequests,
}

fn decode_peer_frame(
    cipher: &wacore::handshake::NoiseCipher,
    counter: &mut u32,
    mut packed: Vec<u8>,
) -> anyhow::Result<OwnedNodeRef> {
    cipher.decrypt_in_place_with_counter(*counter, &mut packed)?;
    *counter += 1;
    let decoded = wacore_binary::util::unpack(&packed)?;
    Ok(OwnedNodeRef::new(decoded.into_owned())?)
}

#[derive(Debug)]
struct PeerObservation {
    id: String,
    xmlns: String,
    is_iq_get: bool,
}

fn peer_observation(node: &OwnedNodeRef) -> PeerObservation {
    let mut attrs = node.get().attrs();
    PeerObservation {
        id: attrs.optional_string("id").unwrap_or_default().into_owned(),
        xmlns: attrs
            .optional_string("xmlns")
            .unwrap_or_default()
            .into_owned(),
        is_iq_get: node.get().tag.as_ref() == "iq"
            && attrs.optional_string("type").as_deref() == Some("get"),
    }
}

async fn run_peer(
    listener: TcpListener,
    behavior: PeerBehavior,
    observed: async_channel::Sender<PeerObservation>,
    handshake_complete: async_channel::Sender<()>,
) -> anyhow::Result<()> {
    let (mut stream, _) = listener.accept().await?;
    let (client_to_server, server_to_client) = serve_noise_handshake(&mut stream).await?;
    handshake_complete.send(()).await?;
    let mut incoming_counter = 0;
    let mut outgoing_counter = 0;
    if matches!(behavior, PeerBehavior::LogInAndAnswerRequests) {
        let success = wacore_binary::marshal::marshal(&NodeBuilder::new("success").build())?;
        let encrypted = server_to_client.encrypt_with_counter(outgoing_counter, &success)?;
        outgoing_counter += 1;
        write_frame(&mut stream, &encrypted).await?;
    }
    loop {
        let ciphertext =
            match tokio::time::timeout(Duration::from_secs(15), read_frame(&mut stream, false))
                .await
            {
                Ok(Ok(frame)) => frame,
                Ok(Err(error)) if error.downcast_ref::<std::io::Error>().is_some() => return Ok(()),
                Ok(Err(error)) => return Err(error),
                Err(_) => {
                    observed
                        .send(PeerObservation {
                            id: "peer_timeout".into(),
                            xmlns: "no TCP frame after handshake".into(),
                            is_iq_get: false,
                        })
                        .await?;
                    return Ok(());
                }
            };
        let node = decode_peer_frame(&client_to_server, &mut incoming_counter, ciphertext)?;
        let observation = peer_observation(&node);
        let id = observation.id.clone();
        let xmlns = observation.xmlns.clone();
        observed.send(observation).await?;

        let should_answer = match behavior {
            PeerBehavior::AnswerProbes => xmlns == "w:p" || xmlns == "test:other",
            PeerBehavior::IgnoreEverything => false,
            PeerBehavior::LogInAndAnswerRequests => {
                node.get().tag.as_ref() == "iq"
                    && matches!(
                        node.get().attrs().optional_string("type").as_deref(),
                        Some("get" | "set")
                    )
            }
        };
        if should_answer {
            let result = NodeBuilder::new("iq")
                .attr("id", id)
                .attr(
                    "from",
                    wacore_binary::Jid::new("", wacore_binary::Server::Pn),
                )
                .attr("type", "result")
                .attr("t", "1760000000")
                .build();
            let marshalled = wacore_binary::marshal::marshal(&result)?;
            let encrypted = server_to_client.encrypt_with_counter(outgoing_counter, &marshalled)?;
            outgoing_counter += 1;
            write_frame(&mut stream, &encrypted).await?;
        }
    }
}

fn drain_observed(rx: &async_channel::Receiver<PeerObservation>) -> Vec<PeerObservation> {
    let mut observed = Vec::new();
    while let Ok(event) = rx.try_recv() {
        observed.push(event);
    }
    observed
}

async fn send_request(
    client: &Arc<Client>,
    id: &'static str,
    namespace: &'static str,
    timeout: Option<Duration>,
) -> tokio::task::JoinHandle<Result<Arc<OwnedNodeRef>, IqError>> {
    let client = client.clone();
    tokio::spawn(async move {
        client
            .send_iq_node(
                NodeBuilder::new("iq")
                    .attr("id", id)
                    .attr("to", wacore_binary::Jid::new("", wacore_binary::Server::Pn))
                    .attr("type", "get")
                    .attr("xmlns", namespace)
                    .build(),
                timeout,
            )
            .await
    })
}

async fn dial_and_read(client: Arc<Client>) -> Option<DisconnectReason> {
    client
        .connect()
        .await
        .expect("the loopback Noise handshake should succeed")
        .read_until_disconnected()
        .await
}

async fn wait_for_application_iqs(
    rx: &async_channel::Receiver<PeerObservation>,
    seen: &mut Vec<PeerObservation>,
) -> anyhow::Result<()> {
    // Transport.send can carry several Noise frames; only the peer's decoded
    // application get-IQs prove progress: matching attributes on a different
    // stanza are not the requests we sent. Keep every observation for the later
    // real-ping assertion, including any ping arriving during this wait.
    tokio::time::timeout(Duration::from_secs(5), async {
        while ![("ignored", "test:ignored"), ("other", "test:other")]
            .iter()
            .all(|(id, xmlns)| {
                seen.iter()
                    .any(|event| event.is_iq_get && event.id == *id && event.xmlns == *xmlns)
            })
        {
            let event = rx.recv().await.with_context(|| {
                format!("peer observations closed before both application IQs; seen={seen:?}")
            })?;
            seen.push(event);
        }
        Ok::<_, anyhow::Error>(())
    })
    .await
    .map_err(|_| anyhow::anyhow!("authenticated application IQ progress stalled; seen={seen:?}"))?
}

fn control_frame(counter: u32, id: &str, namespace: &str) -> Bytes {
    control_node_frame(
        counter,
        &NodeBuilder::new("iq")
            .attr("id", id)
            .attr("type", "get")
            .attr("xmlns", namespace)
            .build(),
    )
}

fn control_node_frame(counter: u32, node: &wacore_binary::Node) -> Bytes {
    let packed = wacore_binary::marshal::marshal(node).unwrap();
    let cipher = wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap();
    let ciphertext = cipher.encrypt_with_counter(counter, &packed).unwrap();
    Bytes::from(wacore::framing::encode_frame(&ciphertext, None).unwrap())
}

/// The controls share the real peer's framing, authentication, nonce sequence
/// and observation path. Fixed keys avoid repeating XX setup; the victim below
/// still exercises the complete handshake and production IQ/keepalive loops.
async fn control_progress(
    outgoing: &[Bytes],
) -> (anyhow::Result<()>, anyhow::Result<()>, Vec<PeerObservation>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let writes = Arc::new(AtomicUsize::new(0));
    let factory = LoopbackTcpFactory {
        address: listener.local_addr().unwrap(),
        writes: writes.clone(),
        write_notify: Arc::new(tokio::sync::Notify::new()),
    };
    let (tx, rx) = async_channel::unbounded();
    let peer = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await?;
        let cipher = wacore::handshake::NoiseCipher::new(&[0; 32])?;
        let mut counter = 0;
        loop {
            let frame = match read_frame(&mut stream, false).await {
                Ok(frame) => frame,
                Err(error) if error.downcast_ref::<std::io::Error>().is_some() => return Ok(()),
                Err(error) => return Err(error),
            };
            let node = decode_peer_frame(&cipher, &mut counter, frame)?;
            tx.send(peer_observation(&node)).await?;
        }
    });
    let (transport, _events) = factory.create_transport().await.unwrap();
    for write in outgoing {
        transport.send(write.clone()).await.unwrap();
    }
    assert_eq!(writes.load(Ordering::Relaxed), outgoing.len());
    transport.disconnect().await;
    let mut seen = Vec::new();
    let progress = wait_for_application_iqs(&rx, &mut seen).await;
    let peer_result = peer.await.unwrap();
    (progress, peer_result, seen)
}

#[tokio::test]
async fn application_progress_accepts_two_noise_frames_in_one_transport_send() {
    let mut coalesced = control_frame(0, "ignored", "test:ignored").to_vec();
    coalesced.extend_from_slice(&control_frame(1, "other", "test:other"));
    let (progress, peer, seen) = control_progress(&[Bytes::from(coalesced)]).await;
    progress.unwrap();
    peer.unwrap();
    assert_eq!(seen.len(), 2);
}

#[tokio::test]
async fn application_progress_rejects_missing_second_frame() {
    let (progress, peer, seen) =
        control_progress(&[control_frame(0, "ignored", "test:ignored")]).await;
    let failure = progress.unwrap_err();
    assert!(failure.downcast_ref::<async_channel::RecvError>().is_some());
    let message = failure.to_string();
    assert!(
        message.contains("seen=") && message.contains("ignored"),
        "{message}"
    );
    peer.unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].id, "ignored");
    assert_eq!(seen[0].xmlns, "test:ignored");
}

#[tokio::test]
async fn application_progress_rejects_malformed_second_frame() {
    let mut coalesced = control_frame(0, "ignored", "test:ignored").to_vec();
    let mut malformed = control_frame(1, "other", "test:other").to_vec();
    *malformed.last_mut().unwrap() ^= 1;
    coalesced.extend_from_slice(&malformed);
    let (progress, peer, seen) = control_progress(&[Bytes::from(coalesced)]).await;
    assert!(progress.is_err());
    assert!(peer.is_err(), "the second frame must fail authentication");
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].id, "ignored");
    assert_eq!(seen[0].xmlns, "test:ignored");
}

#[tokio::test]
async fn application_progress_does_not_credit_a_ping_as_the_other_iq() {
    let (progress, peer, seen) = control_progress(&[
        control_frame(0, "ignored", "test:ignored"),
        control_frame(1, "other", "w:p"),
    ])
    .await;
    assert!(
        progress.is_err(),
        "two transport sends are not two application IQs"
    );
    peer.unwrap();
    assert_eq!(seen.len(), 2);
}

#[tokio::test]
async fn application_progress_rejects_non_iq_with_matching_id_and_namespace() {
    let wrong_tag = NodeBuilder::new("presence")
        .attr("id", "other")
        .attr("xmlns", "test:other")
        .attr("type", "get")
        .build();
    let (progress, peer, seen) = control_progress(&[
        control_frame(0, "ignored", "test:ignored"),
        control_node_frame(1, &wrong_tag),
    ])
    .await;
    assert!(progress.is_err());
    peer.unwrap();
    assert_eq!(seen.len(), 2);
    assert!(!seen[1].is_iq_get);
}

#[tokio::test]
async fn application_progress_rejects_wrong_iq_type_with_matching_id_and_namespace() {
    let wrong_type = NodeBuilder::new("iq")
        .attr("id", "other")
        .attr("xmlns", "test:other")
        .attr("type", "result")
        .build();
    let (progress, peer, seen) = control_progress(&[
        control_frame(0, "ignored", "test:ignored"),
        control_node_frame(1, &wrong_type),
    ])
    .await;
    assert!(progress.is_err());
    peer.unwrap();
    assert_eq!(seen.len(), 2);
    assert!(!seen[1].is_iq_get);
}

async fn wait_for_writes(writes: &AtomicUsize, write_notify: &tokio::sync::Notify, count: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while writes.load(Ordering::Relaxed) < count {
            let notified = write_notify.notified();
            if writes.load(Ordering::Relaxed) < count {
                notified.await;
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "outgoing WA writes stalled at {} before target {count}",
            writes.load(Ordering::Relaxed)
        )
    });
}

async fn await_connected(client: &Client) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.is_socket_connected() || !client.is_running.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("client published the handshaken TCP connection");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_tcp_ignored_iq_survives_until_timeout_when_noise_ping_is_answered() {
    if isolate(concat!(
        module_path!(),
        "::loopback_tcp_ignored_iq_survives_until_timeout_when_noise_ping_is_answered"
    )) {
        return;
    }
    let logs = log_capture::session();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (observed_tx, observed_rx) = async_channel::unbounded();
    let (handshake_tx, handshake_rx) = async_channel::bounded(1);
    let peer = tokio::spawn(run_peer(
        listener,
        PeerBehavior::AnswerProbes,
        observed_tx,
        handshake_tx,
    ));
    let (client, writes, _write_notify) = connected_client(address).await;
    let reader = tokio::spawn(dial_and_read(client.clone()));
    await_connected(&client).await;
    tokio::time::timeout(Duration::from_secs(5), handshake_rx.recv())
        .await
        .expect("the TCP peer completes the real Noise handshake")
        .unwrap();

    let ignored = send_request(&client, "ignored", "test:ignored", None).await;
    let unrelated = send_request(
        &client,
        "other",
        "test:other",
        Some(Duration::from_secs(120)),
    )
    .await;
    let mut seen = Vec::new();
    wait_for_application_iqs(&observed_rx, &mut seen)
        .await
        .expect("the peer authenticates both concurrent application IQs within 5s");
    let ignored_result = tokio::time::timeout(Duration::from_secs(15), ignored)
        .await
        .expect("the default IQ timeout should expire before any reconnect")
        .unwrap();
    assert!(
        matches!(ignored_result, Err(IqError::Timeout)),
        "answered-probe connection should remain through IQ timeout, got {ignored_result:?}; connected={}, peer_finished={}, writes={}, lifecycle_logs={:?}",
        client.is_socket_connected(),
        peer.is_finished(),
        writes.load(Ordering::Relaxed),
        (
            logs.records_for("whatsapp_rust::client::lifecycle"),
            logs.records_for("Client/Keepalive"),
            drain_observed(&observed_rx)
        )
    );
    assert!(
        client.is_socket_connected(),
        "answered Noise probes keep the TCP connection alive"
    );
    seen.extend(drain_observed(&observed_rx));
    assert!(
        seen.iter().any(|event| event.xmlns == "w:p"),
        "the peer must receive a real w:p over TCP"
    );
    assert!(
        unrelated.await.unwrap().is_ok(),
        "the controlled peer answers the unrelated IQ"
    );
    client.shutdown().await;
    assert!(reader.await.unwrap().is_none());
    peer.await.unwrap().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_tcp_silent_peer_reconnects_after_one_noise_ping() {
    if isolate(concat!(
        module_path!(),
        "::loopback_tcp_silent_peer_reconnects_after_one_noise_ping"
    )) {
        return;
    }
    let logs = log_capture::session();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (observed_tx, observed_rx) = async_channel::unbounded();
    let (handshake_tx, handshake_rx) = async_channel::bounded(1);
    let peer = tokio::spawn(run_peer(
        listener,
        PeerBehavior::IgnoreEverything,
        observed_tx,
        handshake_tx,
    ));
    let (client, writes, write_notify) = connected_client(address).await;
    let reader = tokio::spawn(dial_and_read(client.clone()));
    await_connected(&client).await;
    tokio::time::timeout(Duration::from_secs(5), handshake_rx.recv())
        .await
        .expect("the TCP peer completes the real Noise handshake")
        .unwrap();
    let handshake_writes = writes.load(Ordering::Relaxed);

    let started = wacore::time::Instant::now();
    let ignored = send_request(&client, "ignored", "test:ignored", None).await;
    wait_for_writes(&writes, &write_notify, handshake_writes + 1).await;
    let disconnected = tokio::time::timeout(Duration::from_secs(15), reader)
        .await
        .expect("an unanswered peer must be disconnected after one bounded probe")
        .unwrap();
    assert!(
        disconnected.is_none(),
        "watchdog reconnect is an expected teardown"
    );
    assert!(!client.is_socket_connected());
    assert!(
        started.elapsed() >= DEAD_SOCKET_TIME,
        "disconnected after {:?}; keepalive logs: {:?}",
        started.elapsed(),
        logs.records_for("Client/Keepalive")
    );
    assert!(started.elapsed() < crate::request::DEFAULT_IQ_TIMEOUT);
    let ignored_result = ignored.await.unwrap();
    assert!(
        matches!(
            ignored_result,
            Err(IqError::NotConnected | IqError::InternalChannelClosed)
        ),
        "connection teardown must abort the pending application IQ, got {ignored_result:?}"
    );
    let seen = drain_observed(&observed_rx);
    assert_eq!(
        seen.len(),
        2,
        "one ignored application IQ and one liveness probe"
    );
    assert_eq!(seen[1].xmlns, "w:p");
    peer.await.unwrap().unwrap();
}

/// The order production runs in, which a keepalive started by hand skips: the
/// loop is spawned when the connection starts, and `<success>` arrives after
/// it, through the read loop. Login moves the connection generation on, so a
/// loop that only knows the pre-login one retires itself at its first tick and
/// the logged-in connection is left with no idle ping.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_tcp_keepalive_pings_after_login() {
    if isolate(concat!(
        module_path!(),
        "::loopback_tcp_keepalive_pings_after_login"
    )) {
        return;
    }
    let logs = log_capture::session();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let (observed_tx, observed_rx) = async_channel::unbounded();
    let (handshake_tx, handshake_rx) = async_channel::bounded(1);
    let peer = tokio::spawn(run_peer(
        listener,
        PeerBehavior::LogInAndAnswerRequests,
        observed_tx,
        handshake_tx,
    ));
    let (client, _writes, _write_notify) = connected_client(address).await;
    let reader = tokio::spawn(dial_and_read(client.clone()));
    tokio::time::timeout(Duration::from_secs(5), handshake_rx.recv())
        .await
        .expect("the TCP peer completes the real Noise handshake")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.is_logged_in() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the client processes the peer's <success>");

    // Ten real seconds are 100 on the client's clock: several keepalive ticks,
    // enough for the login's own requests to settle and the link to go idle.
    let pinged = tokio::time::timeout(Duration::from_secs(10), async {
        while let Ok(event) = observed_rx.recv().await {
            if event.xmlns == "w:p" {
                return true;
            }
        }
        false
    })
    .await;
    assert!(
        matches!(pinged, Ok(true)),
        "a logged-in idle connection must be pinged; keepalive logs: {:?}",
        logs.records_for("Client/Keepalive")
    );
    client.shutdown().await;
    assert!(reader.await.unwrap().is_none());
    peer.await.unwrap().unwrap();
}
