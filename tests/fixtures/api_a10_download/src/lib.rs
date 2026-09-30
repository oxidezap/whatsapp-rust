//! Also included by the workspace integration test, so CI exercises this API.
#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::Cursor;
    use std::sync::Arc;
    use whatsapp_rust::anyhow::{self, Result};
    use whatsapp_rust::download::{
        ClientDownloadError, DownloadParams, DownloadWriter, MediaDownloadError, MediaDownloader,
        MediaRoute, MediaType,
    };
    use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse, HttpStatusError};
    use whatsapp_rust::store::persistence_manager::PersistenceManager;
    use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
    use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
    use whatsapp_rust::{
        Client, ClientBuilder, ErrorChainExt, IqError, TokioRuntime, async_channel as channels,
        async_trait, waproto as proto,
    };

    struct Offline;
    #[async_trait]
    impl TransportFactory for Offline {
        async fn create_transport(
            &self,
        ) -> Result<(Arc<dyn Transport>, channels::Receiver<TransportEvent>)> {
            panic!("downloads must not dial a socket");
        }
    }

    struct Rejected;
    #[async_trait]
    impl HttpClient for Rejected {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse> {
            assert_eq!(request.url, "https://cdn.example.com/static");
            Ok(HttpResponse {
                status_code: 410,
                body: Vec::new(),
            })
        }
    }

    async fn client() -> Arc<Client> {
        let pm = PersistenceManager::new(Arc::new(InMemoryBackend::new()))
            .await
            .unwrap();
        ClientBuilder::new()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(Arc::new(pm))
            .with_transport_factory(Offline)
            .with_http_client(Rejected)
            .build()
            .await
            .unwrap()
            .into_client()
    }

    #[tokio::test]
    async fn final_client_errors_are_public_and_preserve_typed_sources() {
        let client = client().await;
        let message = proto::whatsapp::message::ImageMessage {
            static_url: Some("https://cdn.example.com/static".into()),
            file_sha256: Some(vec![0; 32]),
            ..Default::default()
        };
        let error: ClientDownloadError = client.download(&message).await.unwrap_err();
        assert!(matches!(error, ClientDownloadError::ReferenceRejected(_)));
        assert_eq!((&error as &(dyn Error + 'static)).http_status(), Some(410));
        assert_eq!(
            error
                .sources()
                .find_map(|cause| cause.downcast_ref::<HttpStatusError>())
                .unwrap()
                .status,
            410
        );
        // Existing anyhow-returning application functions can continue to use ?.
        let erased: anyhow::Error = error.into();
        assert!(erased.downcast_ref::<ClientDownloadError>().is_some());

        let params =
            DownloadParams::encrypted("/d", &[1; 32], &[2; 32], &[3; 32], 16, MediaType::Image);
        let error: ClientDownloadError = client.download(&params).await.unwrap_err();
        assert!(matches!(
            error,
            ClientDownloadError::MediaSession {
                force_refresh: false,
                ..
            }
        ));
        assert!(matches!(
            error.source().unwrap().downcast_ref::<IqError>(),
            Some(IqError::NotConnected)
        ));

        let error = client
            .download_to_writer(&params, Cursor::new(Vec::new()))
            .await
            .unwrap_err();
        assert!(matches!(error, ClientDownloadError::MediaSession { .. }));
        let independent = MediaDownloader::new(
            Arc::new(Rejected),
            Arc::new(TokioRuntime),
            MediaRoute::unauthenticated(Vec::new()),
        );
        let independent_error: MediaDownloadError =
            independent.download(&params).await.unwrap_err();
        assert!(matches!(independent_error, MediaDownloadError::NoHosts));
    }

    // Intentional compatibility fixture only; the library tree uses canonical APIs.
    #[allow(deprecated)]
    #[tokio::test]
    async fn legacy_aliases_delegate_to_the_same_final_session_error() {
        let client = client().await;
        let params =
            DownloadParams::encrypted("/d", &[1; 32], &[2; 32], &[3; 32], 16, MediaType::Image);
        assert!(matches!(
            client.download_from_params(&params).await.unwrap_err(),
            ClientDownloadError::MediaSession { .. }
        ));
        assert!(matches!(
            client
                .download_from_params_to_writer(&params, Cursor::new(Vec::new()))
                .await
                .unwrap_err(),
            ClientDownloadError::MediaSession { .. }
        ));
    }

    // Host traits remain implementable, not sealed by the API refactor.
    struct HostWriter(Cursor<Vec<u8>>);
    impl std::io::Write for HostWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.write(bytes)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.0.flush()
        }
    }
    impl std::io::Seek for HostWriter {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.0.seek(pos)
        }
    }
    impl DownloadWriter for HostWriter {
        fn truncate(&mut self, len: u64) -> std::io::Result<()> {
            self.0.truncate(len)
        }
    }
    #[test]
    fn external_writer_is_implementable() {
        let mut writer = HostWriter(Cursor::new(vec![1, 2, 3]));
        writer.truncate(0).unwrap();
        assert!(writer.0.into_inner().is_empty());
    }
}
