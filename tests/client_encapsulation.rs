//! External-consumer contracts; all dependencies are public, with no real network.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::store::commands::DeviceCommand;
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
use whatsapp_rust::types::enc_handler::EncHandler;
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::wacore::store::traits::DeviceStore;
use whatsapp_rust::{Client, ConnectError, RunCompletionReason, TokioRuntime};

struct OfflineHttp;
#[whatsapp_rust::async_trait]
impl HttpClient for OfflineHttp {
    async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
        whatsapp_rust::anyhow::bail!("offline encapsulation HTTP")
    }
}
struct OfflineTransport(Arc<AtomicUsize>);
#[whatsapp_rust::async_trait]
impl TransportFactory for OfflineTransport {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        self.0.fetch_add(1, Ordering::SeqCst);
        whatsapp_rust::anyhow::bail!("offline encapsulation transport")
    }
}
struct HostHandler;
#[whatsapp_rust::async_trait]
impl EncHandler for HostHandler {
    async fn handle(
        &self,
        _: Arc<Client>,
        _: &wacore_binary::Node,
        _: &whatsapp_rust::types::message::MessageInfo,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

async fn client() -> (Arc<Client>, Arc<AtomicUsize>, Arc<dyn HttpClient>) {
    let attempts = Arc::new(AtomicUsize::new(0));
    let http: Arc<dyn HttpClient> = Arc::new(OfflineHttp);
    let pm = Arc::new(
        PersistenceManager::new(Arc::new(InMemoryBackend::new()))
            .await
            .unwrap(),
    );
    let client = Client::builder()
        .with_persistence_manager(pm)
        .with_runtime(TokioRuntime)
        .with_transport_factory(OfflineTransport(attempts.clone()))
        .with_http_client_arc(http.clone())
        .with_version_override((2, 3000, 1))
        .with_enc_handler("custom", HostHandler)
        .with_enc_handler("custom", HostHandler)
        .build()
        .await
        .unwrap()
        .into_client();
    (client, attempts, http)
}

#[tokio::test(start_paused = true)]
async fn runtime_disable_interrupts_backoff_without_another_attempt() {
    let (client, attempts, _) = client().await;
    assert!(client.auto_reconnect_enabled());
    let mut run = Box::pin(client.run());
    assert!(whatsapp_rust::futures::poll!(run.as_mut()).is_pending());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    client.set_auto_reconnect(false);
    // No clock advance or transport teardown: the policy wake itself ends run.
    let reason = run.await;
    assert!(
        matches!(reason, RunCompletionReason::AutoReconnectDisabled {
        connect_error: Some(ConnectError::Transport(error)), ..
    } if error.to_string().contains("offline encapsulation transport"))
    );
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    client.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn reenable_does_not_shorten_backoff_or_resurrect_shutdown() {
    let (client, attempts, _) = client().await;
    let mut run = Box::pin(client.run());
    assert!(whatsapp_rust::futures::poll!(run.as_mut()).is_pending());
    client.set_auto_reconnect(false);
    client.set_auto_reconnect(true);
    assert!(whatsapp_rust::futures::poll!(run.as_mut()).is_pending());
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
    client.set_auto_reconnect(true);
    assert!(whatsapp_rust::futures::poll!(run.as_mut()).is_pending());
    client.shutdown().await;
    assert!(matches!(run.await, RunCompletionReason::ShutdownRequested));
    client.set_auto_reconnect(true);
    assert!(matches!(
        client.run().await,
        RunCompletionReason::ShutdownRequested
    ));
    assert_eq!(attempts.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn injected_http_identity_and_builder_registry_remain_available() {
    let (client, _, http) = client().await;
    assert!(Arc::ptr_eq(client.http_client(), &http));
    assert!(client.has_enc_handler("custom"));
    assert!(!client.has_enc_handler("missing"));
    assert_eq!(client.memory_report().await.custom_enc_handlers, 1);
    let retained_http = client.http_client().clone();
    let weak = Arc::downgrade(&client);
    client.shutdown().await;
    drop(client);
    // Startup maintenance is owned separately; wait for it without changing
    // lifecycle deadlines or making the HTTP handle own the Client.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while weak.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(Arc::ptr_eq(&retained_http, &http));
}

#[tokio::test]
async fn scoped_device_mutation_publishes_success_error_and_cancellation() {
    let backend = Arc::new(InMemoryBackend::new());
    let pm = PersistenceManager::new(backend.clone()).await.unwrap();
    let old = pm.get_device_snapshot();
    let value = pm
        .modify_device(|device| {
            device.push_name = "sync".into();
            42
        })
        .await;
    assert_eq!(value, 42);
    assert_eq!(old.push_name, "");
    assert_eq!(pm.get_device_snapshot().push_name, "sync");
    assert!(!Arc::ptr_eq(&old, &pm.get_device_snapshot()));
    assert!(Arc::ptr_eq(&old.backend, &pm.backend()));
    pm.flush().await.unwrap();
    assert_eq!(backend.load().await.unwrap().unwrap().push_name, "sync");

    let result: Result<(), &str> = pm
        .modify_device_async(|device| {
            Box::pin(async move {
                device.push_name = "error".into();
                Err("adapter error")
            })
        })
        .await;
    assert_eq!(result, Err("adapter error"));
    assert_eq!(pm.get_device_snapshot().push_name, "error");
    pm.flush().await.unwrap();
    assert_eq!(backend.load().await.unwrap().unwrap().push_name, "error");

    let mut mutation = Box::pin(pm.modify_device_async(|device| {
        Box::pin(async move {
            device.push_name = "cancelled".into();
            std::future::pending::<()>().await;
        })
    }));
    assert!(whatsapp_rust::futures::poll!(mutation.as_mut()).is_pending());
    // Readers retain a committed point-in-time snapshot until the guard drops.
    assert_eq!(pm.get_device_snapshot().push_name, "error");
    drop(mutation);
    assert_eq!(pm.get_device_snapshot().push_name, "cancelled");
    pm.flush().await.unwrap();
    assert_eq!(
        backend.load().await.unwrap().unwrap().push_name,
        "cancelled"
    );
    pm.process_command(DeviceCommand::SetPushName("command".into()))
        .await;
    pm.flush().await.unwrap();
    assert_eq!(backend.load().await.unwrap().unwrap().push_name, "command");
}

#[tokio::test]
async fn clean_flush_waits_for_an_active_modifier_before_reporting_success() {
    let backend = Arc::new(InMemoryBackend::new());
    let pm = PersistenceManager::new(backend.clone()).await.unwrap();
    let mut mutation = Box::pin(pm.modify_device_async(|device| {
        Box::pin(async move {
            device.push_name = "during-flush".into();
            std::future::pending::<()>().await;
        })
    }));
    assert!(whatsapp_rust::futures::poll!(mutation.as_mut()).is_pending());
    let mut flush = Box::pin(pm.flush());
    assert!(
        whatsapp_rust::futures::poll!(flush.as_mut()).is_pending(),
        "clean/final flush must wait for the active modifier's publication"
    );
    drop(mutation);
    flush.await.unwrap();
    assert_eq!(
        backend.load().await.unwrap().unwrap().push_name,
        "during-flush"
    );
}

#[tokio::test]
async fn panicking_modifier_publishes_partial_state_and_releases_lock() {
    use whatsapp_rust::futures::FutureExt;
    let backend = Arc::new(InMemoryBackend::new());
    let pm = PersistenceManager::new(backend.clone()).await.unwrap();
    let result = std::panic::AssertUnwindSafe(pm.modify_device_async(|device| {
        Box::pin(async move {
            device.push_name = "partial".into();
            panic!("synthetic adapter panic");
        })
    }))
    .catch_unwind()
    .await;
    assert!(result.is_err());
    assert_eq!(pm.get_device_snapshot().push_name, "partial");
    pm.flush().await.unwrap();
    assert_eq!(backend.load().await.unwrap().unwrap().push_name, "partial");
}
