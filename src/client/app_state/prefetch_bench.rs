//! Production app-state prefetch fixture for benchmarks.

use crate::Client;
use anyhow::{Result, ensure};
use bytes::Bytes;
use std::sync::Arc;
use wacore::appstate::patch_decode::{PatchList, WAPatchName};
use waproto::whatsapp as wa;

/// Four retained, authenticated blobs using the production app-state prefetch.
pub struct PrefetchHarness {
    client: Arc<Client>,
    lists: Vec<PatchList>,
    plaintext: Vec<u8>,
}

impl PrefetchHarness {
    pub async fn new(size: usize) -> Result<Self> {
        let plaintext = vec![7; size];
        let encrypted =
            wacore::upload::encrypt_media(&plaintext, wacore::download::MediaType::AppState)?;
        let client = crate::bench_support::build_offline_client_with_http(Arc::new(
            StreamingBodyHttp(encrypted.data_to_upload.into()),
        ))
        .await;
        // Client construction schedules finite retention/cache initialization.
        // Join it during setup instead of charging whichever prefetch polls it.
        tokio::time::timeout(
            std::time::Duration::from_secs(30),
            client.bench_startup.wait(),
        )
        .await?;
        *client.media_conn.write().await = Some(crate::mediaconn::MediaConn {
            auth: "synthetic-probe".into(),
            ttl: 3600,
            auth_ttl: None,
            hosts: vec![crate::mediaconn::MediaConnHost::new(
                "synthetic.invalid".into(),
            )],
            fetched_at: wacore::time::Instant::now(),
        });
        let lists = (0..4)
            .map(|index| {
                let mut reference = wa::ExternalBlobReference::default();
                reference.direct_path = Some(format!("/synthetic-probe-{index}"));
                reference.media_key = Some(encrypted.media_key.to_vec());
                reference.file_sha256 = Some(encrypted.file_sha256.to_vec());
                reference.file_enc_sha256 = Some(encrypted.file_enc_sha256.to_vec());
                reference.file_size_bytes = Some(size as u64);
                PatchList {
                    name: WAPatchName::Regular,
                    has_more_patches: false,
                    patches: vec![],
                    snapshot: None,
                    snapshot_ref: Some(reference),
                    error: None,
                }
            })
            .collect();
        Ok(Self {
            client,
            lists,
            plaintext,
        })
    }

    pub async fn prefetch(&self) -> Result<()> {
        ensure!(
            self.client.bench_startup.pending() == 0,
            "startup entered prefetch measurement"
        );
        let blobs = self.client.pre_download_external_blobs(&self.lists).await;
        ensure!(blobs.len() == 4, "prefetch lost a blob");
        for blob in blobs.values() {
            ensure!(
                blob.as_ref()
                    .map_err(|error| anyhow::Error::new(Arc::clone(error)))?
                    .as_ref()
                    == self.plaintext,
                "prefetch plaintext mismatch"
            );
        }
        Ok(())
    }
}

struct StreamingBodyHttp(Bytes);

#[async_trait::async_trait]
impl crate::http::HttpClient for StreamingBodyHttp {
    async fn execute(&self, _: crate::http::HttpRequest) -> Result<crate::http::HttpResponse> {
        anyhow::bail!("benchmark expects streaming HTTP")
    }

    fn supports_streaming(&self) -> bool {
        true
    }

    fn execute_streaming(
        &self,
        _: crate::http::HttpRequest,
    ) -> Result<wacore::net::StreamingHttpResponse> {
        Ok(wacore::net::StreamingHttpResponse::new(
            200,
            Box::new(std::io::Cursor::new(self.0.clone())),
        ))
    }
}
