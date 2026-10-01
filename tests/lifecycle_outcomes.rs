//! Public lifecycle contracts; no network, private state, or SQLite required.
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use whatsapp_rust::bot::{Bot, BotRunOutcome};
use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::{
    Client, ConnectError, DrainOutcome, Reachability, RunCompletionReason, TokioRuntime,
};

struct OfflineHttp;
#[async_trait::async_trait]
impl HttpClient for OfflineHttp {
    async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
        anyhow::bail!("offline lifecycle HTTP")
    }
}
struct OfflineTransport;
#[async_trait::async_trait]
impl TransportFactory for OfflineTransport {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        anyhow::bail!("offline lifecycle transport")
    }
}
async fn bot() -> Bot {
    Bot::builder()
        .with_backend(InMemoryBackend::new())
        .with_runtime(TokioRuntime)
        .with_http_client(OfflineHttp)
        .with_transport_factory(OfflineTransport)
        .with_version((2, 3000, 1))
        .build()
        .await
        .unwrap()
}
async fn client() -> Arc<Client> {
    let pm = Arc::new(
        PersistenceManager::new(Arc::new(InMemoryBackend::new()))
            .await
            .unwrap(),
    );
    let (client, receiver) = Client::builder()
        .with_persistence_manager(pm)
        .with_runtime(TokioRuntime)
        .with_http_client(OfflineHttp)
        .with_transport_factory(OfflineTransport)
        .with_version_override((2, 3000, 1))
        .build()
        .await
        .unwrap()
        .into_parts();
    assert_eq!(receiver.receiver_count(), 1);
    client
}

#[tokio::test]
async fn foreground_and_background_preserve_shutdown_reason() {
    let foreground = bot().await;
    let report = foreground.client().shutdown().await;
    assert_eq!(report.inbound, DrainOutcome::Completed);
    assert_eq!(report.outbound, DrainOutcome::Completed);
    assert_eq!(report.signal_settle, DrainOutcome::Completed);
    assert!(report.device.is_ok());
    assert_eq!(report.message_secrets.failed_batches, 0);
    assert!(matches!(
        foreground.run().await,
        RunCompletionReason::ShutdownRequested
    ));

    let background = bot().await;
    background.client().shutdown().await;
    assert!(matches!(
        background.spawn().await,
        BotRunOutcome::Completed(RunCompletionReason::ShutdownRequested)
    ));

    let handle = bot().await.spawn();
    let report = handle.shutdown().await;
    assert!(report.shutdown.device.is_ok());
    assert!(matches!(
        report.run,
        BotRunOutcome::Completed(RunCompletionReason::ShutdownRequested)
    ));
}

#[tokio::test]
async fn background_preserves_auto_reconnect_failure() {
    let foreground = bot().await;
    foreground
        .client()
        .enable_auto_reconnect
        .store(false, Ordering::Relaxed);
    let background = bot().await;
    background
        .client()
        .enable_auto_reconnect
        .store(false, Ordering::Relaxed);
    let fore = foreground.run().await;
    let back = match background.spawn().await {
        BotRunOutcome::Completed(reason) => reason,
        other => panic!("unexpected task outcome: {other:?}"),
    };
    for reason in [fore, back] {
        match reason {
            RunCompletionReason::AutoReconnectDisabled {
                connection: None,
                connect_error: Some(error),
                protocol_error: None,
                ..
            } => {
                let source = std::error::Error::source(&error).expect("connect source retained");
                assert!(
                    source.to_string().contains("offline lifecycle transport"),
                    "unexpected connect cause: {error:?}"
                );
            }
            other => panic!("lost connect cause: {other:?}"),
        }
    }
}

#[tokio::test]
async fn already_running_pause_and_sticky_shutdown_are_public() {
    let client = client().await;
    client.pause().await;
    let running = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !matches!(client.reachability(), Reachability::Paused) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        client.run().await,
        RunCompletionReason::AlreadyRunning
    ));
    client.shutdown().await;
    assert!(matches!(
        running.await.unwrap(),
        RunCompletionReason::ShutdownRequested
    ));
    client.resume();
    assert!(matches!(
        client.run().await,
        RunCompletionReason::ShutdownRequested
    ));
    assert!(matches!(
        client.connect().await,
        Err(ConnectError::Shutdown)
    ));
    let fresh = self::client().await;
    assert!(!fresh.shutdown_signal().is_fired());
    fresh.pause().await;
    fresh.resume();
    assert!(!fresh.is_paused());
    fresh.shutdown().await;
}

#[tokio::test]
async fn background_already_running_is_observable() {
    let bot = bot().await;
    let client = bot.client();
    client.pause().await;
    let first = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        while !matches!(client.reachability(), Reachability::Paused) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(matches!(
        bot.spawn().await,
        BotRunOutcome::Completed(RunCompletionReason::AlreadyRunning)
    ));
    client.shutdown().await;
    assert!(matches!(
        first.await.unwrap(),
        RunCompletionReason::ShutdownRequested
    ));
}

#[tokio::test]
async fn aborted_handle_returns_without_waiting_for_run_sender() {
    let handle = bot().await.spawn();
    handle.abort();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .unwrap(),
        BotRunOutcome::AbortRequested
    ));
}

// This signature checks the public output types/futures without duplicating a
// driver, and is compiled by the standalone MSRV host with no workspace flags.
#[allow(dead_code)]
async fn readiness(client: &Client) -> Result<(), ConnectError> {
    client.wait_for_socket_ready(Duration::from_secs(1)).await?;
    client.wait_for_session_ready(Duration::from_secs(1)).await
}
