//! A single-dependency downstream host. No root dev-feature unification.
use std::sync::Arc;
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::wacore::runtime::BoxFuture;
use whatsapp_rust::{Client, ClientBuilder};

pub struct HostHandler;
#[cfg_attr(target_arch = "wasm32", whatsapp_rust::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), whatsapp_rust::async_trait)]
impl whatsapp_rust::types::enc_handler::EncHandler for HostHandler {
    async fn handle(
        &self,
        _: Arc<Client>,
        _: &whatsapp_rust::wacore_binary::Node,
        _: &whatsapp_rust::types::message::MessageInfo,
    ) -> whatsapp_rust::anyhow::Result<()> {
        Ok(())
    }
}

pub fn configure(builder: ClientBuilder) -> ClientBuilder {
    builder.with_enc_handler("host.payload", HostHandler)
}

pub fn independent_downloader(
    client: &Client,
    runtime: Arc<dyn whatsapp_rust::Runtime>,
) -> whatsapp_rust::download::MediaDownloader {
    whatsapp_rust::download::MediaDownloader::with_default_hosts(
        client.http_client().clone(),
        runtime,
    )
}

// The published WASM bridge's buffered stream upload/resume check uses this
// injected HTTP capability without needing a socket or replacing the service.
pub fn bridge_http(
    client: &Client,
    request: whatsapp_rust::http::HttpRequest,
) -> BoxFuture<'_, whatsapp_rust::anyhow::Result<whatsapp_rust::http::HttpResponse>> {
    Box::pin(async move { client.http_client().execute(request).await })
}

pub fn policy(client: &Client) {
    client.set_auto_reconnect(false);
    assert!(!client.auto_reconnect_enabled());
    let _ = client.has_enc_handler("host.payload");
}

// Real bodies with Send/boxed async trait-style erasure on native. Compile-only:
// a caller must supply a managed session to drive any network/manual workers.
pub fn inspect(client: &Client) -> BoxFuture<'_, ()> {
    Box::pin(async move {
        let _ = client.memory_report().await.group_cache;
        let _ = client.resource_report().await.http;
        client.run_cache_maintenance().await;
    })
}

pub fn mutate_adapter(
    pm: &PersistenceManager,
) -> BoxFuture<'_, Result<u32, whatsapp_rust::wacore::libsignal::protocol::SignalProtocolError>> {
    Box::pin(async move {
        pm.modify_device_async(|device| {
            Box::pin(async move {
                use whatsapp_rust::wacore::libsignal::protocol::IdentityKeyStore;
                device.get_local_registration_id().await
            })
        })
        .await
    })
}

pub fn signal_stores(
    pm: Arc<PersistenceManager>,
    cache: Arc<whatsapp_rust::store::signal_cache::SignalStoreCache>,
) -> whatsapp_rust::store::signal_adapter::SignalProtocolStoreAdapter {
    whatsapp_rust::store::signal_adapter::SignalProtocolStoreAdapter::new(pm, cache)
}

// These names remain advanced/public rather than disappearing with workers.
pub fn manual_build(build: whatsapp_rust::ClientBuild) {
    let (_client, _sync): (
        Arc<Client>,
        whatsapp_rust::async_channel::Receiver<whatsapp_rust::sync_task::MajorSyncTask>,
    ) = build.into_parts();
}
pub fn core_reexports(_: whatsapp_rust::waproto::whatsapp::Message, _: whatsapp_rust::Jid) {}

#[cfg(feature = "plugins")]
pub struct HostPlugin;
#[cfg(feature = "plugins")]
impl whatsapp_rust::ClientPlugin for HostPlugin {
    type Api = ();
    fn manifest(&self) -> whatsapp_rust::PluginManifest {
        whatsapp_rust::PluginManifest::new("example.encapsulation", "0.1.0")
    }
    fn install(
        &self,
        _: whatsapp_rust::PluginContext,
    ) -> whatsapp_rust::PluginFuture<'_, whatsapp_rust::anyhow::Result<Arc<Self::Api>>> {
        Box::pin(async { Ok(Arc::new(())) })
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::future::Future;
    use std::task::{Context, Poll, Wake, Waker};
    use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
    use whatsapp_rust::wacore::store::traits::DeviceStore;

    // Only uncontended in-memory operations are driven here, not an executor.
    fn ready<T>(future: impl Future<Output = T>) -> T {
        struct NoWake;
        impl Wake for NoWake {
            fn wake(self: Arc<Self>) {}
        }
        let waker = Waker::from(Arc::new(NoWake));
        let mut context = Context::from_waker(&waker);
        match std::pin::pin!(future).as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("in-memory fixture unexpectedly suspended"),
        }
    }

    #[test]
    fn managed_adapter_preserves_snapshot_and_backend_identity() {
        let backend = Arc::new(InMemoryBackend::new());
        let pm = ready(PersistenceManager::new(backend.clone())).unwrap();
        let before = pm.get_device_snapshot();
        assert_eq!(ready(mutate_adapter(&pm)).unwrap(), before.registration_id);
        ready(pm.modify_device(|device| device.push_name = "host".into()));
        let after = pm.get_device_snapshot();
        assert_eq!(after.push_name, "host");
        assert_eq!(before.push_name, "");
        assert!(Arc::ptr_eq(&after.backend, &pm.backend()));
        ready(pm.flush()).unwrap();
        assert_eq!(ready(backend.load()).unwrap().unwrap().push_name, "host");
    }
}
