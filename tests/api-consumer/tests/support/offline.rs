use std::sync::Arc;
use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};

pub struct OfflineHttp;
#[async_trait::async_trait]
impl HttpClient for OfflineHttp {
    async fn execute(&self, _: HttpRequest) -> Result<HttpResponse, whatsapp_rust::http::HttpError> {
        Err(whatsapp_rust::http::HttpError::adapter(std::io::Error::other("offline fixture HTTP")))
    }
}
pub struct OfflineTransport;
#[async_trait::async_trait]
impl TransportFactory for OfflineTransport {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), whatsapp_rust::transport::TransportError> {
        Err("offline fixture transport".into())
    }
}
