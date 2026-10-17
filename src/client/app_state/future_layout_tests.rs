use super::*;
use std::mem::size_of_val;

#[tokio::test]
async fn app_state_async_frames_fit_small_heap_blocks() {
    let client =
        crate::test_utils::create_test_client_with_http("frame-sizes", Arc::new(StreamingOnly))
            .await;
    let scope = client.sync_scope(None);
    let mut outcome = BatchedSyncOutcome::default();
    let reference = wa::ExternalBlobReference::default();
    let processor = client.get_app_state_processor();
    let lists = [];
    let blobs = PrefetchedBlobs::new();
    let resolve = external_blob_resolver(&blobs);
    let list = wacore::appstate::patch_decode::PatchList {
        name: WAPatchName::Regular,
        has_more_patches: false,
        patches: Vec::new(),
        snapshot: None,
        snapshot_ref: None,
        error: None,
    };
    let message = wa::Message::default();
    let error = anyhow::anyhow!("synthetic layout probe");
    let frames = [
        (
            "sync",
            size_of_val(&client.sync_collections_batched(vec![WAPatchName::Regular], scope)),
        ),
        (
            "sync_with",
            size_of_val(&client.sync_collections_batched_with(
                vec![WAPatchName::Regular],
                scope,
                BatchedSyncRequest::default(),
            )),
        ),
        (
            "sync_inner",
            size_of_val(&client.sync_collections_batched_inner(
                vec![WAPatchName::Regular],
                scope,
                BatchedSyncRequest::default(),
                &mut outcome,
            )),
        ),
        (
            "prefetch",
            size_of_val(&client.pre_download_external_blobs(&lists)),
        ),
        (
            "download",
            size_of_val(&client.download(&reference, Default::default())),
        ),
        (
            "bounded_download",
            size_of_val(
                &client.download_prepared_memory(
                    &reference,
                    client
                        .prepare_memory_download(
                            &reference,
                            crate::download::memory::MemoryBudget::new(0),
                        )
                        .unwrap(),
                ),
            ),
        ),
        (
            "apply",
            size_of_val(&processor.process_one_patch_list(list, &resolve, true)),
        ),
        (
            "ensure_sessions",
            size_of_val(&client.ensure_e2e_sessions(&[])),
        ),
        (
            "snapshot_recovery",
            size_of_val(&client.request_syncd_snapshot_recovery(WAPatchName::Regular.as_str())),
        ),
        (
            "blob_job",
            size_of_val(
                &client.external_blob_job(&reference, BlobKind::Snapshot(WAPatchName::Regular)),
            ),
        ),
        (
            "missing_keys",
            size_of_val(&client.request_missing_keys_with_dedup(&[], Duration::from_secs(1))),
        ),
        (
            "recovery",
            size_of_val(&client.escalate_to_snapshot_recovery(WAPatchName::Regular, &error)),
        ),
        (
            "dispatch",
            size_of_val(&client.dispatch_collection_mutations(
                Vec::new(),
                WAPatchName::Regular,
                1,
                true,
                true,
            )),
        ),
        (
            "send",
            size_of_val(&client.send_message_impl(
                "559900000001@s.whatsapp.net".parse().unwrap(),
                &message,
                crate::send::SendPipelineOptions::default(),
            )),
        ),
        (
            "send_iq",
            size_of_val(&client.send_iq(crate::request::InfoQuery {
                namespace: "w:sync:app:state",
                query_type: crate::request::InfoQueryType::Set,
                to: server_jid().clone(),
                target: None,
                id: None,
                content: None,
                timeout: None,
            })),
        ),
    ];
    assert!(
        frames.iter().all(|(_, size)| *size <= 2048),
        "app-state async frames exceed 2048 B: {frames:?}"
    );
}

#[tokio::test]
async fn refused_prefetch_jobs_do_not_allocate_download_sized_nodes() {
    use std::task::{Context, Poll};
    let config = CacheConfig {
        app_state_downloads: crate::AppStateDownloadConfig::new(
            0,
            std::num::NonZeroUsize::new(4).unwrap(),
        ),
        ..Default::default()
    };
    let client = crate::test_utils::create_test_client_with_config(
        "prefetch-frames",
        Arc::new(StreamingOnly),
        config,
    )
    .await;
    let lists: Vec<_> = (0..4)
        .map(|index| {
            let mut reference = wa::ExternalBlobReference::default();
            reference.direct_path = Some(format!("/synthetic-{index}"));
            reference.file_size_bytes = Some(65536);
            reference.handle = Some("synthetic-unused-handle".repeat(256));
            wacore::appstate::patch_decode::PatchList {
                name: WAPatchName::Regular,
                has_more_patches: false,
                patches: Vec::new(),
                snapshot: None,
                snapshot_ref: Some(reference),
                error: None,
            }
        })
        .collect();
    let mut future = Box::pin(client.pre_download_external_blobs(&lists));
    let mut context = Context::from_waker(futures::task::noop_waker_ref());
    let mut ready = false;
    let largest = crate::test_alloc::min_max_block(usize::MAX, || {
        let result = future.as_mut().poll(&mut context);
        ready = matches!(result, Poll::Ready(_));
        result
    });
    assert!(
        ready,
        "resource admission must finish without queuing HTTP work"
    );
    assert!(
        largest <= 1024,
        "refused prefetch allocated a {largest} B block"
    );
}

struct StreamingOnly;

#[async_trait::async_trait]
impl crate::http::HttpClient for StreamingOnly {
    async fn execute(&self, _: crate::http::HttpRequest) -> Result<crate::http::HttpResponse> {
        unreachable!()
    }
    fn supports_streaming(&self) -> bool {
        true
    }
    fn execute_streaming(
        &self,
        _: crate::http::HttpRequest,
    ) -> Result<wacore::net::StreamingHttpResponse> {
        unreachable!()
    }
}
