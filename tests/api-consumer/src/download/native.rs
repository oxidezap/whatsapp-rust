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
        async_trait, wacore, waproto as proto,
    };

    #[test]
    fn owned_metadata_constructors_reject_invalid_inputs() {
        use whatsapp_rust::download::{DownloadPreparationError as Invalid, Downloadable};
        let encrypted = |key: &[u8], plain: &[u8], cipher: &[u8], kind| {
            DownloadParams::encrypted("/file", key, plain, cipher, 16, kind)
        };
        for length in [0, 31, 33] {
            assert!(matches!(
                encrypted(&vec![1; length], &[2; 32], &[3; 32], MediaType::Image),
                Err(Invalid::InvalidMediaKeyLength)
            ));
            assert!(matches!(
                encrypted(&[1; 32], &vec![2; length], &[3; 32], MediaType::Image),
                Err(Invalid::InvalidPlaintextHashLength)
            ));
            assert!(matches!(
                encrypted(&[1; 32], &[2; 32], &vec![3; length], MediaType::Image),
                Err(Invalid::InvalidEncryptedHashLength)
            ));
            assert!(matches!(
                DownloadParams::plaintext("/file", &vec![2; length], 16, MediaType::Image),
                Err(Invalid::InvalidPlaintextHashLength)
            ));
        }
        for kind in [
            MediaType::NewsletterMusicArtwork,
            MediaType::ProductCatalogImage,
        ] {
            assert!(matches!(
                encrypted(&[1; 32], &[2; 32], &[3; 32], kind),
                Err(Invalid::RequiresPlaintext)
            ));
            let params = DownloadParams::plaintext("/file", &[2; 32], 16, kind).unwrap();
            assert!(!params.is_encrypted());
            assert!(params.media_key().is_none());
        }
        assert!(matches!(
            DownloadParams::plaintext("/file", &[2; 32], 16, MediaType::MusicArtwork),
            Err(Invalid::RequiresEncryption)
        ));
        assert!(
            encrypted(&[1; 32], &[2; 32], &[3; 32], MediaType::MusicArtwork)
                .unwrap()
                .is_encrypted()
        );
        assert!(
            !DownloadParams::plaintext("/file", &[2; 32], 16, MediaType::Image)
                .unwrap()
                .is_encrypted()
        );
    }

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
        client_with_http(Rejected).await
    }

    async fn client_with_http(http: impl HttpClient + 'static) -> Arc<Client> {
        let pm = PersistenceManager::new(Arc::new(InMemoryBackend::new()))
            .await
            .unwrap();
        ClientBuilder::new()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(Arc::new(pm))
            .with_transport_factory(Offline)
            .with_http_client(http)
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
            DownloadParams::encrypted("/d", &[1; 32], &[2; 32], &[3; 32], 16, MediaType::Image)
                .unwrap();
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
            MediaRoute::new(Vec::new()),
        );
        let independent_error: MediaDownloadError =
            independent.download(&params).await.unwrap_err();
        assert!(matches!(independent_error, MediaDownloadError::NoHosts));
    }

    struct AcceptedBody(bool);
    #[async_trait]
    impl HttpClient for AcceptedBody {
        async fn execute(&self, _: HttpRequest) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status_code: 200,
                body: b"external destination".to_vec(),
            })
        }
        fn supports_streaming(&self) -> bool {
            self.0
        }
        fn execute_streaming(&self, _: HttpRequest) -> Result<wacore::net::StreamingHttpResponse> {
            Ok(wacore::net::StreamingHttpResponse {
                status_code: 200,
                body: Box::new(Cursor::new(b"external destination".to_vec())),
            })
        }
    }

    #[tokio::test]
    async fn plaintext_params_download_verified_bytes_and_rewind_the_writer() {
        let data = b"external destination";
        let hash = wacore::upload::encrypt_media(data, MediaType::Image)
            .unwrap()
            .file_sha256;
        for streaming in [false, true] {
            let downloader = MediaDownloader::new(
                Arc::new(AcceptedBody(streaming)),
                Arc::new(TokioRuntime),
                MediaRoute::new(vec![whatsapp_rust::download::MediaHost::new(
                    "cdn.example.com",
                )]),
            );
            for kind in [
                MediaType::Image,
                MediaType::NewsletterMusicArtwork,
                MediaType::ProductCatalogImage,
            ] {
                let params =
                    DownloadParams::plaintext("/file", &hash, data.len() as u64, kind).unwrap();
                assert_eq!(downloader.download(&params).await.unwrap(), data);
                let writer = downloader
                    .download_to_writer(&params, Cursor::new(vec![0xff; 128]))
                    .await
                    .unwrap();
                assert_eq!(writer.position(), 0);
                assert_eq!(writer.into_inner(), data);
            }
        }
    }

    #[derive(Debug)]
    struct DestinationFault {
        sink: HostWriter,
        failed: bool,
        fail_write: bool,
    }
    impl std::io::Write for DestinationFault {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.fail_write {
                return Err(std::io::Error::from_raw_os_error(28));
            }
            self.sink.write(bytes)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.sink.flush()
        }
    }
    impl std::io::Seek for DestinationFault {
        fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
            self.sink.seek(pos)
        }
    }
    impl DownloadWriter for DestinationFault {
        fn truncate(&mut self, len: u64) -> std::io::Result<()> {
            if !self.fail_write && !self.failed {
                self.failed = true;
                return Err(std::io::Error::from_raw_os_error(28));
            }
            self.sink.truncate(len)
        }
    }

    #[tokio::test]
    async fn local_writer_failures_have_public_domain_types_and_original_io_sources() {
        let encrypted =
            wacore::upload::encrypt_media(b"external destination", MediaType::Image).unwrap();
        let message = proto::whatsapp::message::ImageMessage {
            static_url: Some("https://cdn.example.com/static".into()),
            file_sha256: Some(encrypted.file_sha256.to_vec()),
            ..Default::default()
        };
        for streaming in [false, true] {
            for fail_write in [false, true] {
                let writer = || DestinationFault {
                    sink: HostWriter(Cursor::new(Vec::new())),
                    failed: false,
                    fail_write,
                };
                let client = client_with_http(AcceptedBody(streaming)).await;
                let error = client
                    .download_to_writer(&message, writer())
                    .await
                    .unwrap_err();
                let ClientDownloadError::WriterIo(cause) = error else {
                    panic!("wrong destination classification: {error:?}");
                };
                let io = cause.downcast_ref::<std::io::Error>().unwrap();
                assert_eq!(io.raw_os_error(), Some(28));
                assert_eq!(
                    io.to_string(),
                    std::io::Error::from_raw_os_error(28).to_string()
                );
                let independent = MediaDownloader::new(
                    Arc::new(AcceptedBody(streaming)),
                    Arc::new(TokioRuntime),
                    MediaRoute::new(Vec::new()),
                );
                let error = independent
                    .download_to_writer(&message, writer())
                    .await
                    .unwrap_err();
                let MediaDownloadError::WriterIo(cause) = error else {
                    panic!("wrong destination classification: {error:?}");
                };
                let io = cause.downcast_ref::<std::io::Error>().unwrap();
                assert_eq!(io.raw_os_error(), Some(28));
                assert_eq!(
                    io.to_string(),
                    std::io::Error::from_raw_os_error(28).to_string()
                );
            }
        }
    }

    #[tokio::test]
    async fn download_params_use_canonical_entries_with_final_session_errors() {
        let client = client().await;
        let params =
            DownloadParams::encrypted("/d", &[1; 32], &[2; 32], &[3; 32], 16, MediaType::Image)
                .unwrap();
        assert!(matches!(
            client.download(&params).await.unwrap_err(),
            ClientDownloadError::MediaSession { .. }
        ));
        assert!(matches!(
            client
                .download_to_writer(&params, Cursor::new(Vec::new()))
                .await
                .unwrap_err(),
            ClientDownloadError::MediaSession { .. }
        ));
    }

    // Host traits remain implementable, not sealed by the API refactor.
    #[derive(Debug)]
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
