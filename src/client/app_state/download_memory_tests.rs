use super::*;
use crate::download::{ClientDownloadError, DownloadPolicyError, MediaDecryptionError, MediaType};
use crate::http::{HttpClient, HttpRequest, HttpResponse};
use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use wacore::appstate::patch_decode::PatchList;
use wacore::net::StreamingHttpResponse;

struct BlobHttp {
    body: Vec<u8>,
    calls: AtomicUsize,
    streaming: bool,
    corrupt_first: AtomicBool,
}

#[async_trait::async_trait]
impl HttpClient for BlobHttp {
    async fn execute(&self, _: HttpRequest) -> Result<HttpResponse> {
        assert!(!self.streaming, "streaming adapters must not buffer HTTP");
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(HttpResponse::new(200, self.body.clone()))
    }

    fn supports_streaming(&self) -> bool {
        self.streaming
    }

    fn execute_streaming(&self, _: HttpRequest) -> Result<StreamingHttpResponse> {
        let first = self.calls.fetch_add(1, Ordering::SeqCst) == 0;
        let mut body = self.body.clone();
        if first && self.corrupt_first.load(Ordering::SeqCst) {
            body[0] ^= 1;
        }
        Ok(StreamingHttpResponse::new(200, Box::new(Cursor::new(body))))
    }
}

fn fixture(size: usize) -> (wa::ExternalBlobReference, Vec<u8>) {
    let encrypted = wacore::upload::encrypt_media(&vec![7; size], MediaType::AppState).unwrap();
    let mut reference = wa::ExternalBlobReference::default();
    reference.direct_path = Some("/synthetic-snapshot".into());
    reference.media_key = Some(encrypted.media_key.to_vec());
    reference.file_sha256 = Some(encrypted.file_sha256.to_vec());
    reference.file_enc_sha256 = Some(encrypted.file_enc_sha256.to_vec());
    reference.file_size_bytes = Some(size as u64);
    (reference, encrypted.data_to_upload)
}

fn list(reference: wa::ExternalBlobReference) -> PatchList {
    PatchList {
        name: WAPatchName::Regular,
        has_more_patches: false,
        patches: Vec::new(),
        snapshot: None,
        snapshot_ref: Some(reference),
        error: None,
    }
}

async fn client(
    limit: usize,
    concurrency: usize,
    body: Vec<u8>,
    streaming: bool,
) -> (Arc<Client>, Arc<BlobHttp>) {
    let http = Arc::new(BlobHttp {
        body,
        calls: AtomicUsize::new(0),
        streaming,
        corrupt_first: AtomicBool::new(false),
    });
    let config = CacheConfig {
        app_state_downloads: crate::AppStateDownloadConfig::new(
            limit,
            std::num::NonZeroUsize::new(concurrency).unwrap(),
        ),
        ..Default::default()
    };
    let client =
        crate::test_utils::create_test_client_with_config("blob-memory", http.clone(), config)
            .await;
    *client.media_conn.write().await = Some(crate::mediaconn::MediaConn {
        auth: "synthetic".into(),
        ttl: 3600,
        auth_ttl: None,
        hosts: vec![crate::mediaconn::MediaConnHost::new(
            "cdn.example.com".into(),
        )],
        fetched_at: wacore::time::Instant::now(),
    });
    (client, http)
}

fn is_memory_error(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(
            cause.downcast_ref::<ClientDownloadError>(),
            Some(ClientDownloadError::ResourcePolicy(
                DownloadPolicyError::MemoryBudgetExceeded { .. }
            ))
        )
    })
}

#[tokio::test]
async fn prefetch_admits_only_configured_jobs_and_refills_after_completion() {
    use std::task::{Context, Poll};

    for concurrency in [1, 2, 4] {
        let (reference, body) = fixture(32);
        let (client, http) = client(192, concurrency, body, true).await;
        let lists: Vec<_> = (0..6)
            .map(|index| {
                let mut reference = reference.clone();
                reference.direct_path = Some(format!("/synthetic-{index}"));
                list(reference)
            })
            .collect();
        let route_gate = client.media_conn.write().await;
        let mut prefetch = Box::pin(client.pre_download_external_blobs(&lists));
        let mut context = Context::from_waker(futures::task::noop_waker_ref());
        assert!(matches!(
            prefetch.as_mut().poll(&mut context),
            Poll::Pending
        ));
        let budget = client.cache_config.app_state_downloads.budget();
        assert_eq!(budget.used(), concurrency * 32);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        drop(prefetch);
        assert_eq!(budget.used(), 0);
        drop(route_gate);

        let blobs = client.pre_download_external_blobs(&lists).await;
        assert_eq!(blobs.len(), lists.len());
        for blob in blobs.values() {
            assert_eq!(blob.as_ref().unwrap().as_ref(), &[7; 32]);
        }
        assert_eq!(budget.used(), 192);
        assert_eq!(http.calls.load(Ordering::SeqCst), 6);
        drop(blobs);
        assert_eq!(budget.used(), 0);
    }
}

#[tokio::test]
async fn batch_sum_is_bounded_without_waiting_for_its_own_results() {
    for concurrency in [1, 4] {
        let (reference, body) = fixture(32);
        let (client, _) = client(64, concurrency, body, true).await;
        let lists: Vec<_> = (0..4)
            .map(|i| {
                let mut reference = reference.clone();
                reference.direct_path = Some(format!("/synthetic-{i}"));
                list(reference)
            })
            .collect();
        let blobs = tokio::time::timeout(
            Duration::from_secs(5),
            client.pre_download_external_blobs(&lists),
        )
        .await
        .unwrap();
        let resolve = external_blob_resolver(&blobs);
        let mut successes = 0;
        for pl in &lists {
            match resolve(pl.snapshot_ref.as_ref().unwrap()) {
                Ok(bytes) => {
                    assert_eq!(bytes.as_ref(), &[7; 32]);
                    successes += 1;
                }
                Err(error) => assert!(is_memory_error(&error), "{error:?}"),
            }
        }
        assert_eq!(successes, 2);
        drop(resolve);
        drop(blobs);
        let retry = client.pre_download_external_blobs(&lists[..1]).await;
        assert!(external_blob_resolver(&retry)(lists[0].snapshot_ref.as_ref().unwrap()).is_ok());
    }
}

#[tokio::test]
async fn completed_blobs_remain_charged_across_prefetch_batches() {
    let (reference, body) = fixture(32);
    let (client, _) = client(32, 4, body, true).await;
    let lists = [list(reference.clone())];
    let first = client.pre_download_external_blobs(&lists).await;
    assert!(external_blob_resolver(&first)(&reference).is_ok());
    let second = client.pre_download_external_blobs(&lists).await;
    assert!(is_memory_error(
        &external_blob_resolver(&second)(&reference).unwrap_err()
    ));
    drop(first);
    let third = client.pre_download_external_blobs(&lists).await;
    assert!(external_blob_resolver(&third)(&reference).is_ok());
}

#[tokio::test]
async fn snapshot_and_mutation_prefetch_support_buffered_and_streaming_http() {
    for streaming in [false, true] {
        let (snapshot, body) = fixture(32);
        let mut mutation = snapshot.clone();
        mutation.direct_path = Some("/synthetic-mutations".into());
        let mut patch_list = list(snapshot.clone());
        let mut patch = wa::SyncdPatch::default();
        patch.external_mutations = Some(mutation.clone()).into();
        patch_list.patches.push(patch);
        let (client, http) = client(64, 2, body, streaming).await;
        let blobs = client.pre_download_external_blobs(&[patch_list]).await;
        assert_eq!(blobs.len(), 2);
        {
            let resolve = external_blob_resolver(&blobs);
            assert_eq!(resolve(&snapshot).unwrap().as_ref(), &[7; 32]);
            assert_eq!(resolve(&mutation).unwrap().as_ref(), &[7; 32]);
        }
        assert_eq!(http.calls.load(Ordering::SeqCst), 2);
        assert_eq!(client.cache_config.app_state_downloads.budget().used(), 64);
        drop(blobs);
        assert_eq!(client.cache_config.app_state_downloads.budget().used(), 0);
    }
}

#[tokio::test]
async fn oversized_declared_blob_fails_before_http_with_either_adapter() {
    for streaming in [true, false] {
        let (reference, body) = fixture(32);
        let (client, http) = client(31, 1, body, streaming).await;
        let blobs = client
            .pre_download_external_blobs(&[list(reference.clone())])
            .await;
        let error = external_blob_resolver(&blobs)(&reference).unwrap_err();
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<ClientDownloadError>(),
            Some(ClientDownloadError::ResourcePolicy(_))
        )));
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        let processor = client.get_app_state_processor();
        let resolve = external_blob_resolver(&blobs);
        let error = processor
            .process_parsed_patch_list(list(reference), &resolve, true)
            .await
            .expect_err("resource failure must reach the processor");
        assert!(error.chain().any(|cause| matches!(
            cause.downcast_ref::<ClientDownloadError>(),
            Some(ClientDownloadError::ResourcePolicy(_))
        )));
        assert!(
            client
                .persistence_manager
                .backend()
                .get_version(WAPatchName::Regular.as_str())
                .await
                .unwrap()
                .is_none()
        );
    }
}

#[tokio::test]
async fn unknown_and_underreported_lengths_cannot_bypass_memory_budget() {
    for declared in [None, Some(1)] {
        let (mut reference, body) = fixture(32);
        reference.file_size_bytes = declared;
        let (client, _) = client(16, 1, body, true).await;
        let blobs = client
            .pre_download_external_blobs(&[list(reference.clone())])
            .await;
        assert!(is_memory_error(
            &external_blob_resolver(&blobs)(&reference).unwrap_err()
        ));
    }
}

#[tokio::test]
async fn cryptographic_failure_is_preserved_and_releases_capacity() {
    let (mut reference, body) = fixture(32);
    let (client, _) = client(32, 1, body, true).await;
    let valid = reference.clone();
    reference.file_sha256 = Some(vec![0; 32]);
    let blobs = client
        .pre_download_external_blobs(&[list(reference.clone())])
        .await;
    let error = external_blob_resolver(&blobs)(&reference).unwrap_err();
    assert!(
        error
            .chain()
            .any(|cause| cause.is::<MediaDecryptionError>()),
        "{error:?}"
    );
    let retry = client
        .pre_download_external_blobs(&[list(valid.clone())])
        .await;
    assert!(external_blob_resolver(&retry)(&valid).is_ok());
}

#[tokio::test]
async fn failover_reuses_capacity_without_spending_a_second_body_budget() {
    let (reference, body) = fixture(32);
    let (client, http) = client(32, 1, body, true).await;
    http.corrupt_first.store(true, Ordering::SeqCst);
    client
        .media_conn
        .write()
        .await
        .as_mut()
        .unwrap()
        .hosts
        .push(crate::mediaconn::MediaConnHost::new(
            "fallback.example.com".into(),
        ));
    let blobs = client
        .pre_download_external_blobs(&[list(reference.clone())])
        .await;
    assert_eq!(
        external_blob_resolver(&blobs)(&reference).unwrap().as_ref(),
        &[7; 32]
    );
    assert_eq!(http.calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn zero_budget_allows_empty_verified_blob_and_deduplicates_paths() {
    let (reference, body) = fixture(0);
    let (client, http) = client(0, 4, body, true).await;
    let blobs = client
        .pre_download_external_blobs(&[list(reference.clone()), list(reference.clone())])
        .await;
    assert!(
        external_blob_resolver(&blobs)(&reference)
            .unwrap()
            .is_empty()
    );
    assert_eq!(blobs.len(), 1);
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn cancelling_active_prefetch_stops_io_and_releases_its_reservation() {
    use std::future::Future;
    use std::io::Read;
    use std::pin::Pin;
    use std::sync::mpsc;
    use wacore::runtime::{AbortHandle, Runtime};

    struct ObservedRuntime(mpsc::Sender<()>);
    #[async_trait::async_trait]
    impl Runtime for ObservedRuntime {
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            crate::TokioRuntime.yield_now()
        }
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send>>) -> AbortHandle {
            crate::TokioRuntime.spawn(future)
        }
        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            crate::TokioRuntime.sleep(duration)
        }
        fn spawn_blocking(
            &self,
            job: Box<dyn FnOnce() + Send>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let finished = self.0.clone();
            crate::TokioRuntime.spawn_blocking(Box::new(move || {
                job();
                let _ = finished.send(());
            }))
        }
    }
    struct GatedBody {
        body: Cursor<Vec<u8>>,
        reads: Arc<AtomicUsize>,
        started: mpsc::Sender<()>,
        resume: mpsc::Receiver<()>,
    }
    impl Read for GatedBody {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            if self.reads.fetch_add(1, Ordering::SeqCst) == 1 {
                self.started.send(()).unwrap();
                self.resume.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            self.body.read(output)
        }
    }
    struct GatedHttp(std::sync::Mutex<Option<GatedBody>>);
    #[async_trait::async_trait]
    impl HttpClient for GatedHttp {
        async fn execute(&self, _: HttpRequest) -> Result<HttpResponse> {
            unreachable!()
        }
        fn supports_streaming(&self) -> bool {
            true
        }
        fn execute_streaming(&self, _: HttpRequest) -> Result<StreamingHttpResponse> {
            Ok(StreamingHttpResponse::new(
                200,
                Box::new(self.0.lock().unwrap().take().unwrap()),
            ))
        }
    }

    let (reference, body) = fixture(32 * 1024);
    let (started_tx, started_rx) = mpsc::channel();
    let (resume_tx, resume_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let reads = Arc::new(AtomicUsize::new(0));
    let http = Arc::new(GatedHttp(std::sync::Mutex::new(Some(GatedBody {
        body: Cursor::new(body),
        reads: reads.clone(),
        started: started_tx,
        resume: resume_rx,
    }))));
    let config = CacheConfig {
        app_state_downloads: crate::AppStateDownloadConfig::new(
            32 * 1024,
            std::num::NonZeroUsize::MIN,
        ),
        ..Default::default()
    };
    let client = crate::test_utils::create_test_client_with_config_and_runtime(
        "cancel-blob-memory",
        http,
        config,
        Arc::new(ObservedRuntime(finished_tx)),
    )
    .await;
    *client.media_conn.write().await = Some(crate::mediaconn::MediaConn {
        auth: "synthetic".into(),
        ttl: 3600,
        auth_ttl: None,
        hosts: vec![crate::mediaconn::MediaConnHost::new(
            "cdn.example.com".into(),
        )],
        fetched_at: wacore::time::Instant::now(),
    });
    // Initialization may use the worker before this download is submitted.
    while finished_rx.try_recv().is_ok() {}
    let task_client = client.clone();
    let task = tokio::spawn(async move {
        task_client
            .pre_download_external_blobs(&[list(reference)])
            .await
    });
    tokio::task::spawn_blocking(move || started_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    assert_eq!(
        client.cache_config.app_state_downloads.budget().used(),
        32 * 1024
    );
    task.abort();
    assert!(matches!(task.await, Err(error) if error.is_cancelled()));
    resume_tx.send(()).unwrap();
    tokio::task::spawn_blocking(move || finished_rx.recv_timeout(Duration::from_secs(5)).unwrap())
        .await
        .unwrap();
    assert_eq!(reads.load(Ordering::SeqCst), 2);
    assert_eq!(client.cache_config.app_state_downloads.budget().used(), 0);
}

#[test]
fn blob_budget_is_lazy_and_shared_between_callers() {
    let policy = AppStateDownloads::new(crate::AppStateDownloadConfig::default());
    assert!(policy.budget.get().is_none());
    let first = policy.budget();
    assert!(std::ptr::eq(first, policy.budget()));
    assert_eq!(first.used(), 0);
}
