use super::*;
use crate::store::traits::Backend;
use crate::types::message::{MessageInfo, MessageSource};
use crate::{HistorySyncDurabilityHook, HistorySyncKey, HistorySyncMetadata};
use bytes::Bytes;
use flate2::{Compression, write::ZlibEncoder};
use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use waproto::whatsapp as wa;

#[derive(Default)]
struct RecordingHistory {
    records: Mutex<Vec<(HistorySyncKey, Vec<u8>)>>,
    notifications: AtomicUsize,
    chunks: AtomicUsize,
    fail: AtomicU8,
    parked: AtomicBool,
    entered: tokio::sync::Notify,
    gate: tokio::sync::Notify,
    chunk_parked: AtomicBool,
    chunk_entered: tokio::sync::Notify,
    chunk_gate: tokio::sync::Notify,
    backend: Option<Arc<dyn Backend>>,
    require_journal: AtomicBool,
}

#[async_trait::async_trait]
impl HistorySyncDurabilityHook for RecordingHistory {
    async fn on_notification(
        &self,
        key: &HistorySyncKey,
        metadata: &HistorySyncMetadata<'_>,
        notification: &[u8],
    ) -> anyhow::Result<()> {
        self.notifications.fetch_add(1, Ordering::SeqCst);
        if self.require_journal.load(Ordering::SeqCst) {
            assert!(
                self.backend
                    .as_ref()
                    .unwrap()
                    .get_pending_inbound(&key.chat, &key.sender, &key.id)
                    .await?
                    .is_some(),
                "the full carrier must be durable before the host callback"
            );
        }
        let decoded = waproto::codec::history_sync_notification_decode(notification)?;
        assert_eq!(
            metadata.sync_type,
            decoded.sync_type.map(|kind| kind as i32)
        );
        assert_eq!(metadata.chunk_order, decoded.chunk_order);
        assert_eq!(metadata.progress, decoded.progress);
        assert_eq!(metadata.file_length, decoded.file_length);
        assert_eq!(
            metadata.peer_data_request_session_id,
            decoded.peer_data_request_session_id.as_deref()
        );
        assert_eq!(
            metadata.inline_payload_len,
            decoded
                .initial_hist_bootstrap_inline_payload
                .as_ref()
                .map(Vec::len)
        );
        anyhow::ensure!(
            self.fail.load(Ordering::SeqCst) != 1,
            "notification failure"
        );
        {
            let mut records = self.records.lock().unwrap();
            if let Some((_, previous)) = records.iter().find(|(previous, _)| previous == key) {
                anyhow::ensure!(previous == notification, "conflicting notification");
            } else {
                records.push((key.clone(), notification.to_vec()));
            }
        }
        if self.parked.load(Ordering::SeqCst) {
            self.entered.notify_one();
            self.gate.notified().await;
        }
        Ok(())
    }

    async fn on_compressed_chunk(
        &self,
        key: &HistorySyncKey,
        metadata: &HistorySyncMetadata<'_>,
        compressed: &[u8],
    ) -> anyhow::Result<()> {
        self.chunks.fetch_add(1, Ordering::SeqCst);
        {
            let records = self.records.lock().unwrap();
            let (_, notification) = records
                .iter()
                .find(|(previous, _)| previous == key)
                .unwrap();
            let decoded = waproto::codec::history_sync_notification_decode(notification)?;
            assert_eq!(metadata.chunk_order, decoded.chunk_order);
            if let Some(inline) = decoded.initial_hist_bootstrap_inline_payload {
                assert_eq!(compressed, inline);
            }
        }
        if self.chunk_parked.load(Ordering::SeqCst) {
            self.chunk_entered.notify_one();
            self.chunk_gate.notified().await;
        }
        anyhow::ensure!(self.fail.load(Ordering::SeqCst) != 2, "chunk failure");
        Ok(())
    }
}

fn notification() -> wa::message::HistorySyncNotification {
    let mut history = wa::HistorySync::default();
    history.sync_type = wa::history_sync::HistorySyncType::RECENT;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&waproto::codec::history_sync_to_vec(&history))
        .unwrap();
    let compressed = encoder.finish().unwrap();
    let mut notification = wa::message::HistorySyncNotification::default();
    notification.sync_type = Some(wa::message::HistorySyncType::RECENT);
    notification.file_length = Some(compressed.len() as u64);
    notification.chunk_order = Some(7);
    notification.progress = Some(55);
    notification.peer_data_request_session_id = Some("synthetic-recovery-session".into());
    notification.initial_hist_bootstrap_inline_payload = Some(compressed);
    notification
}

fn info(id: &str) -> Arc<MessageInfo> {
    Arc::new(MessageInfo {
        id: id.into(),
        source: MessageSource {
            chat: "15550000001@s.whatsapp.net".parse().unwrap(),
            sender: "15550000001:7@s.whatsapp.net".parse().unwrap(),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    })
}

fn key(info: &MessageInfo) -> HistorySyncKey {
    HistorySyncKey {
        chat: info.source.chat.to_string(),
        sender: info.source.sender.to_string(),
        id: info.id.to_string(),
    }
}

async fn client(
    backend: Arc<dyn Backend>,
) -> (
    Arc<Client>,
    Arc<RecordingHistory>,
    Arc<crate::transport::mock::CapturingMockTransport>,
) {
    let (client, hook, transport, receiver) = client_with_receiver(backend).await;
    client.start_sync_task_worker(receiver);
    (client, hook, transport)
}

async fn client_with_receiver(
    backend: Arc<dyn Backend>,
) -> (
    Arc<Client>,
    Arc<RecordingHistory>,
    Arc<crate::transport::mock::CapturingMockTransport>,
    async_channel::Receiver<crate::sync_task::MajorSyncTask>,
) {
    client_with_runtime(backend, Arc::new(crate::runtime_impl::TokioRuntime)).await
}

async fn client_with_runtime(
    backend: Arc<dyn Backend>,
    runtime: Arc<dyn wacore::runtime::Runtime>,
) -> (
    Arc<Client>,
    Arc<RecordingHistory>,
    Arc<crate::transport::mock::CapturingMockTransport>,
    async_channel::Receiver<crate::sync_task::MajorSyncTask>,
) {
    client_with_http_and_runtime(
        backend,
        runtime,
        Arc::new(crate::test_utils::MockHttpClient),
    )
    .await
}

async fn client_with_http_and_runtime(
    backend: Arc<dyn Backend>,
    runtime: Arc<dyn wacore::runtime::Runtime>,
    http: Arc<dyn crate::http::HttpClient>,
) -> (
    Arc<Client>,
    Arc<RecordingHistory>,
    Arc<crate::transport::mock::CapturingMockTransport>,
    async_channel::Receiver<crate::sync_task::MajorSyncTask>,
) {
    let persistence = Arc::new(
        crate::store::persistence_manager::PersistenceManager::new(backend.clone())
            .await
            .unwrap(),
    );
    let (client, receiver) = Client::builder()
        .with_runtime_arc(runtime)
        .with_persistence_manager(persistence)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_http_client_arc(http)
        .build()
        .await
        .unwrap()
        .into_parts();
    let transport = Arc::new(crate::transport::mock::CapturingMockTransport::new());
    *client.noise_socket.lock().unwrap() = Some(Arc::new(crate::socket::NoiseSocket::new(
        client.runtime.clone(),
        transport.clone(),
        wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
        wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
    )));
    client.set_connected_for_test(true);
    client.is_running.store(true, Ordering::Release);
    client.enter_live_mode_for_tests();
    client
        .persistence_manager
        .process_command(wacore::store::commands::DeviceCommand::SetId(Some(
            "15550000001:1@s.whatsapp.net".parse().unwrap(),
        )))
        .await;
    let hook = Arc::new(RecordingHistory {
        backend: Some(backend),
        require_journal: AtomicBool::new(true),
        ..Default::default()
    });
    client
        .history_sync_durability_hook
        .set(hook.clone())
        .ok()
        .unwrap();
    (client, hook, transport, receiver)
}

async fn memory_client() -> (
    Arc<Client>,
    Arc<RecordingHistory>,
    Arc<crate::transport::mock::CapturingMockTransport>,
) {
    client(Arc::new(wacore::store::in_memory::InMemoryBackend::new())).await
}

async fn receive(
    client: &Arc<Client>,
    info: &Arc<MessageInfo>,
    notification: wa::message::HistorySyncNotification,
) -> bool {
    let mut protocol = wa::message::ProtocolMessage::default();
    protocol.r#type = Some(wa::message::protocol_message::Type::HistorySyncNotification);
    protocol.history_sync_notification = buffa::MessageField::some(notification);
    let mut message = wa::Message::default();
    message.protocol_message = buffa::MessageField::some(protocol);
    client
        .handle_decrypted_plaintext(
            "msg",
            wacore::messages::MessageUtils::encode_and_pad(&message),
            2,
            0,
            Default::default(),
            info,
        )
        .await
        .unwrap()
        .delivered
}

async fn pending(client: &Client, info: &MessageInfo) -> Option<Vec<u8>> {
    let key = key(info);
    client
        .persistence_manager
        .backend()
        .get_pending_inbound(&key.chat, &key.sender, &key.id)
        .await
        .unwrap()
}

async fn history_idle(client: &Client) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let idle = client.history_sync_activity.listen();
            if client.history_sync_activity.tasks() == 0 {
                return;
            }
            idle.await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn durable_history_notification_gates_ack_and_chunk_gates_history_receipt() {
    let (client, hook, transport) = memory_client().await;
    let info = info("DURABLE_ORDER");
    hook.parked.store(true, Ordering::SeqCst);
    hook.chunk_parked.store(true, Ordering::SeqCst);
    let mut history_receipt = client
        .wait_for_sent_node(crate::client::NodeFilter::tag("receipt").attr("type", "hist_sync"));
    let operation = tokio::spawn({
        let client = client.clone();
        let info = info.clone();
        async move { receive(&client, &info, notification()).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    assert!(pending(&client, &info).await.is_some());
    assert_eq!(client.history_sync_activity.tasks(), 1);
    assert_eq!(
        client.history_sync_activity.snapshot().payload_bytes,
        notification()
            .initial_hist_bootstrap_inline_payload
            .unwrap()
            .len()
    );
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 0);
    assert!(transport.sent().is_empty());
    hook.gate.notify_one();
    assert!(operation.await.unwrap());
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        hook.chunk_entered.notified(),
    )
    .await
    .unwrap();
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 1);
    assert_eq!(hook.records.lock().unwrap()[0].0, key(&info));
    assert!(pending(&client, &info).await.is_none());
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert!(!transport.sent().is_empty());
    assert!(history_receipt.try_recv().unwrap().is_none());
    hook.chunk_gate.notify_one();
    history_idle(&client).await;
    assert!(history_receipt.try_recv().unwrap().is_some());
}

#[tokio::test]
async fn durable_history_failures_keep_the_carrier_and_retry_on_redelivery() {
    let (client, hook, transport) = memory_client().await;
    let info = info("DURABLE_REDELIVERY");
    hook.fail.store(1, Ordering::SeqCst);
    assert!(!receive(&client, &info, notification()).await);
    assert!(pending(&client, &info).await.is_some());
    assert!(transport.sent().is_empty());
    hook.fail.store(0, Ordering::SeqCst);
    assert!(client.ack_or_replay_to_hook(&info).await);
    history_idle(&client).await;
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 2);
    assert!(pending(&client, &info).await.is_none());
}

#[tokio::test]
async fn durable_history_cancellation_keeps_journal_for_a_new_client() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let path = std::env::temp_dir().join(format!(
        "wa_history_recovery_{}_{}.db",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::SeqCst),
    ));
    let path = path.to_str().unwrap();
    let backend = Arc::new(crate::store::SqliteStore::open(path).await.unwrap());
    let (first, hook, transport) = client(backend).await;
    let info = info("DURABLE_CANCEL");
    hook.parked.store(true, Ordering::SeqCst);
    let operation = tokio::spawn({
        let client = first.clone();
        let info = info.clone();
        async move { receive(&client, &info, notification()).await }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    operation.abort();
    assert!(operation.await.unwrap_err().is_cancelled());
    assert!(pending(&first, &info).await.is_some());
    assert!(transport.sent().is_empty());
    first.signal_shutdown_sync();
    drop(first);
    drop(hook);
    let backend = Arc::new(crate::store::SqliteStore::open(path).await.unwrap());
    let (second, hook, _) = client(backend).await;
    assert!(second.ack_or_replay_to_hook(&info).await);
    history_idle(&second).await;
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 1);
    assert!(pending(&second, &info).await.is_none());
    second.signal_shutdown_sync();
    drop(second);
    drop(hook);
    std::fs::remove_file(path).unwrap();
}

#[tokio::test]
async fn durable_history_host_replay_checks_identity_and_retains_metadata() {
    let (first, hook, _) = memory_client().await;
    let info = info("DURABLE_HOST");
    hook.fail.store(2, Ordering::SeqCst);
    assert!(receive(&first, &info, notification()).await);
    history_idle(&first).await;
    let (key, bytes) = hook.records.lock().unwrap()[0].clone();
    let (second, recovered, _) = memory_client().await;
    recovered.require_journal.store(false, Ordering::SeqCst);
    second
        .replay_history_sync_notification(&key, &bytes)
        .await
        .unwrap();
    assert_eq!(recovered.chunks.load(Ordering::SeqCst), 1);
    assert_eq!(
        recovered.records.lock().unwrap()[0],
        (key.clone(), bytes.clone())
    );
    for foreign in [
        HistorySyncKey {
            sender: "15550000002:7@s.whatsapp.net".into(),
            ..key.clone()
        },
        HistorySyncKey {
            chat: "15550000002@s.whatsapp.net".into(),
            ..key.clone()
        },
        HistorySyncKey {
            id: String::new(),
            ..key.clone()
        },
    ] {
        assert!(
            second
                .replay_history_sync_notification(&foreign, &bytes)
                .await
                .is_err()
        );
    }
    assert_eq!(recovered.notifications.load(Ordering::SeqCst), 1);
    assert!(
        second
            .replay_history_sync_notification(&key, &[255])
            .await
            .is_err()
    );
}

#[tokio::test]
async fn durable_history_nonself_never_reaches_hooks() {
    let (client, hook, _) = memory_client().await;
    let mut info = (*info("FOREIGN_HISTORY")).clone();
    info.source.is_from_me = false;
    info.source.sender = "15550000002@s.whatsapp.net".parse().unwrap();
    let info = Arc::new(info);
    assert!(receive(&client, &info, notification()).await);
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 0);
    assert!(pending(&client, &info).await.is_none());
}

#[tokio::test]
async fn durable_history_malformed_chunk_and_download_failure_keep_host_copy_without_history_receipt()
 {
    for inline in [Some(vec![1, 2, 3]), None] {
        let (client, hook, transport) = memory_client().await;
        let info = info("INVALID_HISTORY");
        let mut notification = notification();
        notification.initial_hist_bootstrap_inline_payload = inline;
        let mut receipt = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("receipt").attr("type", "hist_sync"),
        );
        assert!(receive(&client, &info, notification).await);
        history_idle(&client).await;
        assert_eq!(hook.notifications.load(Ordering::SeqCst), 1);
        assert!(pending(&client, &info).await.is_none());
        assert_eq!(hook.records.lock().unwrap().len(), 1);
        assert!(receipt.try_recv().unwrap().is_none());
        crate::test_utils::wait_for_outbound_tasks(&client).await;
        assert!(!transport.sent().is_empty());
    }
}

#[tokio::test]
async fn durable_history_same_id_different_sender_has_distinct_identity() {
    let (client, hook, _) = memory_client().await;
    for device in [7, 8] {
        let mut info = (*info("SAME_ID")).clone();
        info.source.sender = format!("15550000001:{device}@s.whatsapp.net")
            .parse()
            .unwrap();
        assert!(receive(&client, &Arc::new(info), notification()).await);
    }
    history_idle(&client).await;
    let records = hook.records.lock().unwrap();
    assert_eq!(records.len(), 2);
    assert_ne!(records[0].0.sender, records[1].0.sender);
}

#[tokio::test]
async fn durable_history_rejects_conflicting_notification_even_when_projection_is_duplicate() {
    let (client, hook, transport) = memory_client().await;
    let info = info("DIVERGENT_HISTORY");
    assert!(receive(&client, &info, notification()).await);
    history_idle(&client).await;
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    let before = transport.sent().len();
    let mut changed = notification();
    changed.progress = Some(56);
    assert!(!receive(&client, &info, changed).await);
    assert!(pending(&client, &info).await.is_some());
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(transport.sent().len(), before);
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 2);
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 1);
    assert_eq!(hook.records.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn durable_history_skip_acknowledges_without_capture() {
    let (client, hook, transport) = memory_client().await;
    client.set_skip_history_sync(true);
    assert!(receive(&client, &info("SKIP_HISTORY"), notification()).await);
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    assert!(!transport.sent().is_empty());
}

#[tokio::test]
async fn durable_history_journal_failure_retains_retry_without_calling_host() {
    let (mut backend, _) = crate::store::release::tests::ProbeBackend::new();
    let failing = Arc::new(AtomicBool::new(false));
    backend.fail_before_method = Some(("replace_pending_inbound_if", failing.clone()));
    let (client, hook, transport) = client(Arc::new(backend)).await;
    let info = info("JOURNAL_FAILURE");
    failing.store(true, Ordering::SeqCst);
    assert!(!receive(&client, &info, notification()).await);
    assert!(pending(&client, &info).await.is_none());
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    assert!(transport.sent().is_empty());
    assert!(
        !client
            .inbound_commit_batch
            .live_retention
            .for_message(&info)
            .unwrap()
            .is_empty()
    );
    failing.store(false, Ordering::SeqCst);
    assert!(client.ack_or_replay_to_hook(&info).await);
    history_idle(&client).await;
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 1);
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 1);
    assert!(pending(&client, &info).await.is_none());
}

#[tokio::test]
async fn durable_history_ambiguous_journal_write_replays_the_committed_carrier() {
    let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
    let (client, hook, transport) = client(backend.clone()).await;
    let info = info("AMBIGUOUS_HISTORY");
    backend.fail_next_pending_replace_after_commit();
    assert!(!receive(&client, &info, notification()).await);
    assert!(pending(&client, &info).await.is_some());
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    assert!(transport.sent().is_empty());
    assert!(client.ack_or_replay_to_hook(&info).await);
    history_idle(&client).await;
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 1);
    assert!(pending(&client, &info).await.is_none());
}

#[tokio::test]
async fn durable_history_full_or_closed_queue_preserves_journal_without_waiting_for_downloads() {
    for closed in [false, true] {
        let (client, hook, transport, receiver) =
            client_with_receiver(Arc::new(wacore::store::in_memory::InMemoryBackend::new())).await;
        if closed {
            receiver.close();
        } else {
            for index in 0..8 {
                assert!(receive(&client, &info(&format!("QUEUED_{index}")), notification()).await);
            }
        }
        crate::test_utils::wait_for_outbound_tasks(&client).await;
        let before = transport.sent().len();
        let info = info("QUEUE_REFUSED");
        let delivered = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            receive(&client, &info, notification()),
        )
        .await
        .expect("receive must not wait for a history worker slot");
        assert!(!delivered);
        assert!(pending(&client, &info).await.is_some());
        assert_eq!(hook.chunks.load(Ordering::SeqCst), 0);
        assert!(
            hook.records
                .lock()
                .unwrap()
                .iter()
                .any(|(key, _)| key.id == "QUEUE_REFUSED")
        );
        crate::test_utils::wait_for_outbound_tasks(&client).await;
        assert_eq!(transport.sent().len(), before);
        if !closed {
            drop(receiver.recv().await.unwrap());
            assert!(client.ack_or_replay_to_hook(&info).await);
            assert!(pending(&client, &info).await.is_none());
        }
    }
}

#[tokio::test]
async fn durable_history_inline_replay_before_run_succeeds_but_terminal_replay_is_rejected() {
    let (client, hook, _) = memory_client().await;
    client.is_running.store(false, Ordering::Release);
    client.set_connected_for_test(false);
    *client.noise_socket.lock().unwrap() = None;
    client.set_auto_reconnect(false);
    assert!(
        client.is_terminal(),
        "the connection policy has no future socket"
    );
    hook.require_journal.store(false, Ordering::SeqCst);
    let key = key(&info("BEFORE_RUN"));
    let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
    client
        .replay_history_sync_notification(&key, &bytes)
        .await
        .unwrap();
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 1);
    assert!(!client.is_running.load(Ordering::Acquire));
    client.signal_shutdown_sync();
    assert!(
        client
            .replay_history_sync_notification(&key, &bytes)
            .await
            .is_err()
    );
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn durable_history_worker_and_replay_share_two_permits_and_cancellation_releases_one() {
    let (client, hook, _) = memory_client().await;
    hook.chunk_parked.store(true, Ordering::SeqCst);
    assert!(receive(&client, &info("WORKER_SLOT"), notification()).await);
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        hook.chunk_entered.notified(),
    )
    .await
    .unwrap();
    hook.require_journal.store(false, Ordering::SeqCst);
    let replay = |id: &'static str| {
        let client = client.clone();
        tokio::spawn(async move {
            let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
            client
                .replay_history_sync_notification(&key(&info(id)), &bytes)
                .await
        })
    };
    let second = replay("REPLAY_SLOT");
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        hook.chunk_entered.notified(),
    )
    .await
    .unwrap();
    let third = replay("WAITING_REPLAY");
    crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 2);
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 2);
    second.abort();
    assert!(second.await.unwrap_err().is_cancelled());
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        hook.chunk_entered.notified(),
    )
    .await
    .unwrap();
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 3);
    third.abort();
    assert!(third.await.unwrap_err().is_cancelled());
    hook.chunk_gate.notify_one();
    history_idle(&client).await;
    let first_permit = client.acquire_history_sync_permit().await.unwrap();
    let second_permit = client.acquire_history_sync_permit().await.unwrap();
    drop((first_permit, second_permit));
}

#[tokio::test]
async fn durable_history_session_wake_preserves_semaphore_waiter_registration() {
    use std::future::Future as _;
    use std::task::{Context, Wake, Waker};

    struct CountWake(AtomicUsize);
    impl Wake for CountWake {
        fn wake(self: Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    let (client, _, _) = memory_client().await;
    let held = client.acquire_history_sync_permit().await.unwrap();
    let other = client.acquire_history_sync_permit().await.unwrap();
    let first_wakes = Arc::new(CountWake(AtomicUsize::new(0)));
    let second_wakes = Arc::new(CountWake(AtomicUsize::new(0)));
    let first_waker = Waker::from(first_wakes.clone());
    let second_waker = Waker::from(second_wakes.clone());
    let mut first_cx = Context::from_waker(&first_waker);
    let mut second_cx = Context::from_waker(&second_waker);
    let mut first = Box::pin(client.acquire_history_sync_permit());
    let mut second = Box::pin(client.acquire_history_sync_permit());
    assert!(first.as_mut().poll(&mut first_cx).is_pending());
    assert!(second.as_mut().poll(&mut second_cx).is_pending());

    client.session_state_notifier.notify(usize::MAX);
    // A later waiter can be scheduled first after the shared session wake.
    assert!(second.as_mut().poll(&mut second_cx).is_pending());
    assert!(first.as_mut().poll(&mut first_cx).is_pending());
    first_wakes.0.store(0, Ordering::SeqCst);
    second_wakes.0.store(0, Ordering::SeqCst);
    drop(held);
    let first_notified = first_wakes.0.load(Ordering::SeqCst);
    let second_notified = second_wakes.0.load(Ordering::SeqCst);
    drop((first, second, other));
    assert!(
        first_notified > 0,
        "older semaphore waiter lost its listener on session wake"
    );
    assert_eq!(second_notified, 0);
}

#[tokio::test]
async fn durable_history_two_replays_leave_capacity_for_normal_sync_intake() {
    let (client, hook, _) = memory_client().await;
    hook.require_journal.store(false, Ordering::SeqCst);
    hook.chunk_parked.store(true, Ordering::SeqCst);
    let first = tokio::spawn({
        let client = client.clone();
        async move {
            let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
            client
                .replay_history_sync_notification(&key(&info("FIRST_REPLAY")), &bytes)
                .await
        }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        hook.chunk_entered.notified(),
    )
    .await
    .unwrap();

    let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
    let second_key = key(&info("SECOND_REPLAY"));
    let mut second = Box::pin(client.replay_history_sync_notification(&second_key, &bytes));
    assert!(futures::poll!(second.as_mut()).is_pending());
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 1);

    // The first replay remains blocked in its hook while normal intake drains.
    hook.chunk_parked.store(false, Ordering::SeqCst);
    let normal = info("NORMAL_WITH_REPLAY_BACKLOG");
    let receipt = client.wait_for_sent_node(
        crate::client::NodeFilter::tag("receipt")
            .attr("type", "hist_sync")
            .attr("id", normal.id.to_string()),
    );
    assert!(receive(&client, &normal, notification()).await);
    tokio::time::timeout(std::time::Duration::from_secs(5), receipt)
        .await
        .expect("normal history must finish while both replay futures remain pending")
        .unwrap();
    assert!(pending(&client, &normal).await.is_none());
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 2);
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 2);
    assert!(!first.is_finished());
    assert!(futures::poll!(second.as_mut()).is_pending());

    hook.chunk_gate.notify_one();
    first.await.unwrap().unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), second)
        .await
        .unwrap()
        .unwrap();
    history_idle(&client).await;
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn durable_history_terminal_transition_cancels_a_replay_waiting_for_a_permit() {
    let (client, hook, _) = memory_client().await;
    let first = client.acquire_history_sync_permit().await.unwrap();
    let second = client.acquire_history_sync_permit().await.unwrap();
    let replay = tokio::spawn({
        let client = client.clone();
        async move {
            let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
            client
                .replay_history_sync_notification(&key(&info("TERMINAL_WAIT")), &bytes)
                .await
        }
    });
    crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;
    client.signal_shutdown_sync();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), replay)
            .await
            .unwrap()
            .unwrap()
            .is_err()
    );
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    drop((first, second));
}

#[tokio::test]
async fn durable_history_terminal_transition_cancels_a_replay_waiting_for_replay_capacity() {
    let (client, hook, _) = memory_client().await;
    let permit = client
        .history_sync_activity
        .replay_permits
        .acquire_arc()
        .await;
    let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
    let replay_key = key(&info("TERMINAL_REPLAY_CAPACITY"));
    let mut replay = Box::pin(client.replay_history_sync_notification(&replay_key, &bytes));
    assert!(futures::poll!(replay.as_mut()).is_pending());
    client.signal_shutdown_sync();
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(1), replay)
            .await
            .unwrap()
            .is_err()
    );
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    drop(permit);
}

struct ParkedEncodingRuntime {
    entered: Arc<tokio::sync::Notify>,
    release: Mutex<Option<std::sync::mpsc::Receiver<()>>>,
    skip: AtomicUsize,
}

struct BufferedHistoryHttp(Vec<u8>);

#[async_trait::async_trait]
impl crate::http::HttpClient for BufferedHistoryHttp {
    async fn execute(
        &self,
        request: crate::http::HttpRequest,
    ) -> anyhow::Result<crate::http::HttpResponse> {
        assert!(request.url.contains("cdn.example.invalid"));
        Ok(crate::http::HttpResponse::new(200, self.0.clone()))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn durable_history_cancelled_download_keeps_cpu_and_replay_leases() {
    let (release, gate) = std::sync::mpsc::channel();
    let runtime = Arc::new(ParkedEncodingRuntime {
        entered: Arc::new(tokio::sync::Notify::new()),
        release: Mutex::new(Some(gate)),
        skip: AtomicUsize::new(0),
    });
    let mut notification = notification();
    let compressed = notification
        .initial_hist_bootstrap_inline_payload
        .take()
        .unwrap();
    let encrypted =
        wacore::upload::encrypt_media(&compressed, wacore::download::MediaType::History).unwrap();
    notification.direct_path = Some("/synthetic-history".into());
    notification.media_key = Some(encrypted.media_key.to_vec());
    notification.file_sha256 = Some(encrypted.file_sha256.to_vec());
    notification.file_enc_sha256 = Some(encrypted.file_enc_sha256.to_vec());
    let (client, hook, transport, _) = client_with_http_and_runtime(
        Arc::new(wacore::store::in_memory::InMemoryBackend::new()),
        runtime.clone(),
        Arc::new(BufferedHistoryHttp(encrypted.data_to_upload)),
    )
    .await;
    *client.media_conn.write().await = Some(crate::mediaconn::MediaConn {
        auth: "synthetic-history-auth".into(),
        ttl: 300,
        auth_ttl: Some(300),
        hosts: vec![crate::mediaconn::MediaConnHost::new(
            "cdn.example.invalid".into(),
        )],
        fetched_at: wacore::time::Instant::now(),
    });
    hook.require_journal.store(false, Ordering::SeqCst);
    let replay = tokio::spawn({
        let client = client.clone();
        async move {
            let bytes = waproto::codec::history_sync_notification_to_vec(&notification);
            client
                .replay_history_sync_notification(&key(&info("DOWNLOAD_CPU_CANCEL")), &bytes)
                .await
        }
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        runtime.entered.notified(),
    )
    .await
    .unwrap();
    replay.abort();
    assert!(replay.await.unwrap_err().is_cancelled());
    let spare = client
        .history_sync_activity
        .processing_permits
        .try_acquire_arc()
        .unwrap();
    let cpu_retained = client
        .history_sync_activity
        .processing_permits
        .try_acquire_arc()
        .is_none();
    let replay_retained = client
        .history_sync_activity
        .replay_permits
        .try_acquire_arc()
        .is_none();
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if client
                .history_sync_activity
                .processing_permits
                .try_acquire_arc()
                .is_some()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 1);
    assert_eq!(hook.chunks.load(Ordering::SeqCst), 0);
    assert!(transport.sent().is_empty());
    assert!(
        cpu_retained,
        "download CPU released its processing lease before blocking work finished"
    );
    assert!(
        replay_retained,
        "download CPU released its replay reservation before blocking work finished"
    );
    drop(spare);
}

#[async_trait::async_trait]
impl wacore::runtime::Runtime for ParkedEncodingRuntime {
    fn spawn(
        &self,
        future: wacore::runtime::BoxFuture<'static, ()>,
    ) -> wacore::runtime::AbortHandle {
        crate::runtime_impl::TokioRuntime.spawn(future)
    }

    fn sleep(&self, duration: std::time::Duration) -> wacore::runtime::BoxFuture<'static, ()> {
        crate::runtime_impl::TokioRuntime.sleep(duration)
    }

    fn yield_now(&self) -> Option<wacore::runtime::BoxFuture<'static, ()>> {
        None
    }

    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> wacore::runtime::BoxFuture<'static, ()> {
        if self
            .skip
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |skip| {
                skip.checked_sub(1)
            })
            .is_ok()
        {
            return crate::runtime_impl::TokioRuntime.spawn_blocking(f);
        }
        let release = self.release.lock().unwrap().take().unwrap();
        let entered = self.entered.clone();
        let executor_thread = std::thread::current().id();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                assert_ne!(std::thread::current().id(), executor_thread);
                entered.notify_one();
                release
                    .recv_timeout(std::time::Duration::from_secs(5))
                    .unwrap();
                f();
            })
            .await
            .unwrap();
        })
    }
}

#[tokio::test(flavor = "current_thread")]
async fn durable_history_large_intake_does_not_encode_outside_processing_capacity() {
    let (client, hook, transport) = memory_client().await;
    let first = client.acquire_history_sync_permit().await.unwrap();
    let second = client.acquire_history_sync_permit().await.unwrap();
    let info = info("SATURATED_ENCODING");
    let mut notification = notification();
    notification.initial_hist_bootstrap_inline_payload = Some(vec![0xa5; 1024 * 1024]);

    assert!(!receive(&client, &info, notification).await);
    assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
    assert!(transport.sent().is_empty());
    assert!(pending(&client, &info).await.is_some());
    assert_eq!(client.history_sync_activity.tasks(), 0);
    drop((first, second));
}

#[tokio::test(flavor = "current_thread")]
async fn durable_history_large_inline_encoding_leaves_executor_and_stays_tracked_on_cancel() {
    for cancel in [false, true] {
        let (release, gate) = std::sync::mpsc::channel();
        let runtime = Arc::new(ParkedEncodingRuntime {
            entered: Arc::new(tokio::sync::Notify::new()),
            release: Mutex::new(Some(gate)),
            skip: AtomicUsize::new(0),
        });
        let (client, hook, transport, receiver) = client_with_runtime(
            Arc::new(wacore::store::in_memory::InMemoryBackend::new()),
            runtime.clone(),
        )
        .await;
        let info = info("LARGE_ENCODING");
        let mut notification = notification();
        let inline = vec![0xa5; 1024 * 1024];
        notification.initial_hist_bootstrap_inline_payload = Some(inline.clone());
        notification.file_length = Some(inline.len() as u64);
        let operation = tokio::spawn({
            let client = client.clone();
            let info = info.clone();
            async move { receive(&client, &info, notification).await }
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            runtime.entered.notified(),
        )
        .await
        .unwrap();
        assert!(pending(&client, &info).await.is_some());
        assert_eq!(client.history_sync_activity.tasks(), 1);
        assert_eq!(
            client.history_sync_activity.snapshot().payload_bytes,
            inline.len()
        );
        assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
        assert!(transport.sent().is_empty());
        let spare = client.acquire_history_sync_permit().await.unwrap();
        assert!(
            client
                .history_sync_activity
                .processing_permits
                .try_acquire_arc()
                .is_none()
        );
        if cancel {
            operation.abort();
            assert!(operation.await.unwrap_err().is_cancelled());
            assert_eq!(client.history_sync_activity.tasks(), 1);
            assert!(
                client
                    .history_sync_activity
                    .processing_permits
                    .try_acquire_arc()
                    .is_none()
            );
            release.send(()).unwrap();
            history_idle(&client).await;
            assert!(
                client
                    .history_sync_activity
                    .processing_permits
                    .try_acquire_arc()
                    .is_some()
            );
            assert!(pending(&client, &info).await.is_some());
            assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
        } else {
            release.send(()).unwrap();
            assert!(operation.await.unwrap());
            {
                let records = hook.records.lock().unwrap();
                let decoded =
                    waproto::codec::history_sync_notification_decode(&records[0].1).unwrap();
                assert_eq!(
                    decoded.initial_hist_bootstrap_inline_payload.as_deref(),
                    Some(inline.as_slice())
                );
            }
            assert!(pending(&client, &info).await.is_none());
            drop(receiver.recv().await.unwrap());
            history_idle(&client).await;
        }
        drop(spare);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn durable_history_cancelled_replay_keeps_permit_until_blocking_decode_or_encoding_finishes()
{
    for skipped in [0, 1] {
        let (release, gate) = std::sync::mpsc::channel();
        let runtime = Arc::new(ParkedEncodingRuntime {
            entered: Arc::new(tokio::sync::Notify::new()),
            release: Mutex::new(Some(gate)),
            skip: AtomicUsize::new(skipped),
        });
        let (client, hook, _, _) = client_with_runtime(
            Arc::new(wacore::store::in_memory::InMemoryBackend::new()),
            runtime.clone(),
        )
        .await;
        hook.require_journal.store(false, Ordering::SeqCst);
        let mut notification = notification();
        notification.initial_hist_bootstrap_inline_payload = Some(vec![0xa5; 1024 * 1024]);
        let replay = tokio::spawn({
            let client = client.clone();
            async move {
                let bytes = waproto::codec::history_sync_notification_to_vec(&notification);
                client
                    .replay_history_sync_notification(&key(&info("CANCEL_ENCODING")), &bytes)
                    .await
            }
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            runtime.entered.notified(),
        )
        .await
        .unwrap();
        assert_eq!(
            client.history_sync_activity.snapshot().payload_bytes,
            if skipped == 0 { 0 } else { 1024 * 1024 }
        );
        replay.abort();
        assert!(replay.await.unwrap_err().is_cancelled());
        let spare = client
            .history_sync_activity
            .processing_permits
            .try_acquire_arc()
            .unwrap();
        assert!(
            client
                .history_sync_activity
                .processing_permits
                .try_acquire_arc()
                .is_none()
        );
        assert_eq!(client.history_sync_activity.tasks(), 1);
        release.send(()).unwrap();
        history_idle(&client).await;
        assert!(
            client
                .history_sync_activity
                .processing_permits
                .try_acquire_arc()
                .is_some()
        );
        assert_eq!(hook.notifications.load(Ordering::SeqCst), 0);
        drop(spare);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn durable_history_cancelled_blocking_replay_reserves_normal_capacity_until_cpu_finishes() {
    for parsing in [false, true] {
        let (release, gate) = std::sync::mpsc::channel();
        let runtime = Arc::new(ParkedEncodingRuntime {
            entered: Arc::new(tokio::sync::Notify::new()),
            release: Mutex::new(Some(gate)),
            skip: AtomicUsize::new(1 + usize::from(parsing)),
        });
        let (client, hook, _, receiver) = client_with_runtime(
            Arc::new(wacore::store::in_memory::InMemoryBackend::new()),
            runtime.clone(),
        )
        .await;
        client.start_sync_task_worker(receiver);
        hook.require_journal.store(false, Ordering::SeqCst);
        let first = tokio::spawn({
            let client = client.clone();
            async move {
                let mut notification = notification();
                notification.initial_hist_bootstrap_inline_payload = Some(vec![0xa5; 1024 * 1024]);
                let bytes = waproto::codec::history_sync_notification_to_vec(&notification);
                client
                    .replay_history_sync_notification(&key(&info("CANCEL_CPU")), &bytes)
                    .await
            }
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            runtime.entered.notified(),
        )
        .await
        .unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());

        assert_eq!(client.history_sync_activity.tasks(), 1);
        assert_eq!(
            client.history_sync_activity.snapshot().payload_bytes,
            1024 * 1024
        );
        client.offline_sync_completed.store(true, Ordering::Relaxed);
        let mut startup = Box::pin(client.wait_for_startup_sync(std::time::Duration::from_secs(5)));
        assert!(futures::poll!(startup.as_mut()).is_pending());
        let before = hook.notifications.load(Ordering::SeqCst);
        assert_eq!(before, usize::from(parsing));
        let bytes = waproto::codec::history_sync_notification_to_vec(&notification());
        let second_key = key(&info("WAITING_FOR_CPU"));
        let mut second = Box::pin(client.replay_history_sync_notification(&second_key, &bytes));
        assert!(futures::poll!(second.as_mut()).is_pending());
        assert_eq!(hook.notifications.load(Ordering::SeqCst), before);

        let normal = info("NORMAL_WITH_CANCELLED_REPLAY");
        let receipt = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("receipt")
                .attr("type", "hist_sync")
                .attr("id", normal.id.to_string()),
        );
        assert!(receive(&client, &normal, notification()).await);
        tokio::time::timeout(std::time::Duration::from_secs(5), receipt)
            .await
            .expect("detached replay CPU must leave a slot for the normal worker")
            .unwrap();
        assert!(pending(&client, &normal).await.is_none());
        assert_eq!(hook.notifications.load(Ordering::SeqCst), before + 1);
        assert!(futures::poll!(second.as_mut()).is_pending());

        release.send(()).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), second)
            .await
            .unwrap()
            .unwrap();
        history_idle(&client).await;
        assert_eq!(hook.notifications.load(Ordering::SeqCst), before + 2);
        let first = client
            .history_sync_activity
            .processing_permits
            .try_acquire_arc()
            .unwrap();
        let second = client
            .history_sync_activity
            .processing_permits
            .try_acquire_arc()
            .unwrap();
        let replay = client
            .history_sync_activity
            .replay_permits
            .try_acquire_arc()
            .unwrap();
        drop((first, second, replay));
    }
}

#[tokio::test]
async fn durable_history_replay_retains_unknown_notification_fields() {
    let (client, hook, _) = memory_client().await;
    hook.require_journal.store(false, Ordering::SeqCst);
    let mut wire = waproto::codec::history_sync_notification_to_vec(&notification());
    // Synthetic future field 1000, varint 123. Generated owned models drop it.
    wire.extend_from_slice(&[0xc0, 0x3e, 0x7b]);
    let decoded = waproto::codec::history_sync_notification_decode(&wire).unwrap();
    assert_ne!(
        waproto::codec::history_sync_notification_to_vec(&decoded),
        wire
    );
    client
        .replay_history_sync_notification(&key(&info("UNKNOWN_WIRE")), &wire)
        .await
        .unwrap();
    assert_eq!(hook.records.lock().unwrap()[0].1, wire);
}

#[tokio::test]
async fn durable_history_reconnect_keeps_parked_replay_accounted() {
    let (client, hook, _) = memory_client().await;
    hook.require_journal.store(false, Ordering::SeqCst);
    hook.parked.store(true, Ordering::SeqCst);
    let replay = tokio::spawn({
        let client = client.clone();
        async move {
            let wire = waproto::codec::history_sync_notification_to_vec(&notification());
            client
                .replay_history_sync_notification(&key(&info("RECONNECT_ACCOUNTING")), &wire)
                .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    let before = client.history_sync_activity.snapshot();
    assert_eq!(before.tasks, 1);
    assert!(before.payload_bytes > 0);
    client.cleanup_connection_state().await;
    let after = client.history_sync_activity.snapshot();
    assert_eq!(after.tasks, before.tasks);
    assert_eq!(after.payload_bytes, before.payload_bytes);
    replay.abort();
    assert!(replay.await.unwrap_err().is_cancelled());
    history_idle(&client).await;
    assert_eq!(client.history_sync_activity.snapshot().payload_bytes, 0);
}

#[tokio::test]
async fn durable_history_intake_retains_unknown_notification_fields() {
    fn nested(field: u32, body: &[u8]) -> Bytes {
        let mut wire = Vec::new();
        buffa::encoding::encode_varint(u64::from(field) << 3 | 2, &mut wire);
        buffa::encoding::encode_varint(body.len() as u64, &mut wire);
        wire.extend_from_slice(body);
        Bytes::from(wire)
    }
    let (client, hook, _, receiver) =
        client_with_receiver(Arc::new(wacore::store::in_memory::InMemoryBackend::new())).await;
    hook.require_journal.store(false, Ordering::SeqCst);
    let mut wire = waproto::codec::history_sync_notification_to_vec(&notification());
    wire.extend_from_slice(&[0xc0, 0x3e, 0x7b]);
    let protocol = nested(
        waproto::tags::message::protocol_message::HISTORY_SYNC_NOTIFICATION,
        &wire,
    );
    let carrier = nested(waproto::tags::message::PROTOCOL_MESSAGE, &protocol);
    client
        .commit_history_sync_carrier(&info("UNKNOWN_INTAKE"), carrier)
        .await
        .unwrap();
    assert_eq!(hook.records.lock().unwrap()[0].1, wire);
    drop(receiver.recv().await.unwrap());
    history_idle(&client).await;
}
