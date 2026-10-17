use std::alloc::{GlobalAlloc, Layout, System};
use std::future::{Future, pending};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;

use whatsapp_rust::async_trait;
use whatsapp_rust::download::{DownloadLimits, DownloadParams, MediaDownloader, MediaType};
use whatsapp_rust::wacore::net::{HttpClient, HttpRequest, HttpResponse, StreamingHttpResponse};
use whatsapp_rust::wacore::runtime::{AbortHandle, Runtime};

const BLOB_BYTES: usize = 64 * 1024;
const DOWNLOADS: usize = 4;
static LIVE_BLOB_BYTES: AtomicUsize = AtomicUsize::new(0);
static REFUSE_BLOB_ALLOCATION: AtomicBool = AtomicBool::new(false);

struct CountingAllocator;

// SAFETY: accepted pointers and layouts pass unchanged to System. Rejected
// allocations return null, and accounting uses only atomics.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == BLOB_BYTES && REFUSE_BLOB_ALLOCATION.load(Ordering::SeqCst) {
            return std::ptr::null_mut();
        }
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() && layout.size() == BLOB_BYTES {
            LIVE_BLOB_BYTES.fetch_add(BLOB_BYTES, Ordering::SeqCst);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == BLOB_BYTES {
            LIVE_BLOB_BYTES.fetch_sub(BLOB_BYTES, Ordering::SeqCst);
        }
        unsafe { System.dealloc(ptr, layout) };
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        if size == BLOB_BYTES && REFUSE_BLOB_ALLOCATION.load(Ordering::SeqCst) {
            return std::ptr::null_mut();
        }
        let next = unsafe { System.realloc(ptr, layout, size) };
        if !next.is_null() {
            if layout.size() == BLOB_BYTES {
                LIVE_BLOB_BYTES.fetch_sub(BLOB_BYTES, Ordering::SeqCst);
            }
            if size == BLOB_BYTES {
                LIVE_BLOB_BYTES.fetch_add(BLOB_BYTES, Ordering::SeqCst);
            }
        }
        next
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Default)]
struct ParkedWorker {
    submitted: AtomicUsize,
    jobs: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
}

#[async_trait]
impl Runtime for ParkedWorker {
    fn spawn(&self, _: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
        panic!("unexpected background task");
    }

    fn sleep(&self, _: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(pending())
    }

    fn spawn_blocking(
        &self,
        job: Box<dyn FnOnce() + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.submitted.fetch_add(1, Ordering::SeqCst);
        self.jobs.lock().unwrap().push(job);
        Box::pin(pending())
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }
}

#[derive(Default)]
struct NoNetworkHttp {
    entered: AtomicUsize,
}

#[async_trait]
impl HttpClient for NoNetworkHttp {
    async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
        panic!("unexpected buffered HTTP");
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn execute_streaming(&self, _: HttpRequest) -> anyhow::Result<StreamingHttpResponse> {
        self.entered.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("network must not run while the worker is parked");
    }
}

#[test]
fn queued_streaming_downloads_do_not_reserve_bodies_before_worker_admission() {
    for limits in [DownloadLimits::default(), DownloadLimits::new(65536, 65536)] {
        queued_downloads(limits);
    }
    allocation_failure_is_a_terminal_error();
}

fn queued_downloads(limits: DownloadLimits) {
    let worker = Arc::new(ParkedWorker::default());
    let http = Arc::new(NoNetworkHttp::default());
    let downloader = MediaDownloader::with_default_hosts(http.clone(), worker.clone());
    let params: Vec<_> = (0..DOWNLOADS)
        .map(|i| {
            DownloadParams::encrypted(
                format!("/synthetic-app-state-{i}"),
                &[1; 32],
                &[2; 32],
                &[3; 32],
                BLOB_BYTES as u64,
                MediaType::AppState,
            )
            .unwrap()
        })
        .collect();
    let baseline = LIVE_BLOB_BYTES.load(Ordering::SeqCst);
    let mut downloads = Box::pin(futures::future::join_all(
        params
            .iter()
            .map(|param| downloader.download(param, limits)),
    ));
    let mut context = Context::from_waker(futures::task::noop_waker_ref());
    assert!(matches!(
        downloads.as_mut().poll(&mut context),
        Poll::Pending
    ));
    let retained = LIVE_BLOB_BYTES.load(Ordering::SeqCst) - baseline;
    let submitted = worker.submitted.load(Ordering::SeqCst);
    let http_calls = http.entered.load(Ordering::SeqCst);
    drop(downloads);
    assert_eq!(LIVE_BLOB_BYTES.load(Ordering::SeqCst), baseline);
    for job in worker.jobs.lock().unwrap().drain(..) {
        job();
    }
    assert_eq!(
        http.entered.load(Ordering::SeqCst),
        0,
        "cancelled queued work must never start HTTP"
    );
    assert_eq!(LIVE_BLOB_BYTES.load(Ordering::SeqCst), baseline);
    assert_eq!(http_calls, 0);
    assert_eq!(submitted, DOWNLOADS);
    assert_eq!(
        retained, 0,
        "queued downloads reserved {retained} bytes before worker admission; no HTTP call started"
    );
}

fn allocation_failure_is_a_terminal_error() {
    struct ImmediateWorker;
    #[async_trait]
    impl Runtime for ImmediateWorker {
        fn spawn(&self, _: Pin<Box<dyn Future<Output = ()> + Send>>) -> AbortHandle {
            unreachable!()
        }
        fn sleep(&self, _: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(pending())
        }
        fn spawn_blocking(
            &self,
            job: Box<dyn FnOnce() + Send>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            job();
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }
    struct BodyHttp {
        body: Vec<u8>,
        calls: AtomicUsize,
    }
    #[async_trait]
    impl HttpClient for BodyHttp {
        async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
            unreachable!()
        }
        fn supports_streaming(&self) -> bool {
            true
        }
        fn execute_streaming(&self, _: HttpRequest) -> anyhow::Result<StreamingHttpResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(StreamingHttpResponse::new(
                200,
                Box::new(std::io::Cursor::new(self.body.clone())),
            ))
        }
    }
    let encrypted =
        wacore::upload::encrypt_media(&vec![7; BLOB_BYTES], MediaType::AppState).unwrap();
    for declared in [BLOB_BYTES as u64, 16 * 1024] {
        let params = DownloadParams::encrypted(
            "/synthetic-allocation-failure",
            &encrypted.media_key,
            &encrypted.file_sha256,
            &encrypted.file_enc_sha256,
            declared,
            MediaType::AppState,
        )
        .unwrap();
        let http = Arc::new(BodyHttp {
            body: encrypted.data_to_upload.clone(),
            calls: AtomicUsize::new(0),
        });
        let downloader =
            MediaDownloader::with_default_hosts(http.clone(), Arc::new(ImmediateWorker));
        let mut download = Box::pin(downloader.download(&params, DownloadLimits::default()));
        let mut context = Context::from_waker(futures::task::noop_waker_ref());
        REFUSE_BLOB_ALLOCATION.store(true, Ordering::SeqCst);
        let result = download.as_mut().poll(&mut context);
        REFUSE_BLOB_ALLOCATION.store(false, Ordering::SeqCst);
        assert!(matches!(
            result,
            Poll::Ready(Err(
                whatsapp_rust::download::MediaDownloadError::ResourcePolicy(
                    whatsapp_rust::download::DownloadPolicyError::AllocationFailed
                )
            ))
        ));
        assert_eq!(
            http.calls.load(Ordering::SeqCst),
            1,
            "allocation failure must stop host failover"
        );
    }
}
