use std::sync::Arc;
use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};

pub struct OfflineHttp;
#[async_trait::async_trait]
impl HttpClient for OfflineHttp {
    async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
        anyhow::bail!("offline fixture HTTP")
    }
}
pub struct OfflineTransport;
#[async_trait::async_trait]
impl TransportFactory for OfflineTransport {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        anyhow::bail!("offline fixture transport")
    }
}
