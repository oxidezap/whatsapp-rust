//! Production app-state prefetch fixture for benchmarks.

use crate::Client;
use anyhow::{Result, ensure};
use bytes::Bytes;
use std::sync::Arc;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};
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
        Self::with_completion_gate(size, None).await
    }

    /// Align the synthetic streams at EOF while their production download
    /// writers are still live. No extra payload owner survives verification.
    pub async fn phase_aligned(size: usize, gate: Arc<PrefetchCompletionGate>) -> Result<Self> {
        Self::with_completion_gate(size, Some(gate)).await
    }

    async fn with_completion_gate(
        size: usize,
        gate: Option<Arc<PrefetchCompletionGate>>,
    ) -> Result<Self> {
        let plaintext = vec![7; size];
        let encrypted =
            wacore::upload::encrypt_media(&plaintext, wacore::download::MediaType::AppState)?;
        let client =
            crate::bench_support::build_offline_client_with_http(Arc::new(StreamingBodyHttp {
                body: encrypted.data_to_upload.into(),
                gate,
            }))
            .await;
        // Client construction schedules finite retention/cache initialization.
        // Join it during setup instead of charging whichever prefetch polls it.
        tokio::time::timeout(Duration::from_secs(30), client.bench_startup.wait()).await?;
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

/// One-shot rendezvous for a known number of synthetic HTTP streams. A missing
/// participant fails the sample instead of hanging the blocking thread pool.
pub struct PrefetchCompletionGate {
    streams: usize,
    arrived: Mutex<usize>,
    ready: Condvar,
}

impl PrefetchCompletionGate {
    pub fn new(streams: usize) -> Self {
        assert!(streams > 0);
        Self {
            streams,
            arrived: Mutex::new(0),
            ready: Condvar::new(),
        }
    }

    fn wait(&self, timeout: Duration) -> std::io::Result<()> {
        let deadline = Instant::now() + timeout;
        let mut arrived = self.arrived.lock().expect("completion gate");
        *arrived += 1;
        assert!(*arrived <= self.streams, "completion gate reused");
        self.ready.notify_all();
        while *arrived < self.streams {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "prefetch completion gate missing a stream",
                ));
            }
            (arrived, _) = self
                .ready
                .wait_timeout(arrived, remaining)
                .expect("completion gate");
        }
        Ok(())
    }
}

struct CompletionBody {
    body: std::io::Cursor<Bytes>,
    gate: Option<Arc<PrefetchCompletionGate>>,
}

impl std::io::Read for CompletionBody {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let count = self.body.read(out)?;
        if count == 0
            && !out.is_empty()
            && let Some(gate) = self.gate.take()
        {
            gate.wait(Duration::from_secs(120))?;
        }
        Ok(count)
    }
}

struct StreamingBodyHttp {
    body: Bytes,
    gate: Option<Arc<PrefetchCompletionGate>>,
}

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
            Box::new(CompletionBody {
                body: std::io::Cursor::new(self.body.clone()),
                gate: self.gate.clone(),
            }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn completion_gate_waits_for_peers_and_eof_only_once() {
        let gate = Arc::new(PrefetchCompletionGate::new(2));
        let mut body = CompletionBody {
            body: std::io::Cursor::new(Bytes::from_static(b"fixture")),
            gate: Some(Arc::clone(&gate)),
        };
        let mut bytes = [0; 16];
        assert_eq!(body.read(&mut []).unwrap(), 0);
        assert_eq!(body.read(&mut bytes).unwrap(), 7);
        assert_eq!(*gate.arrived.lock().unwrap(), 0);
        std::thread::scope(|scope| {
            let peer = scope.spawn(|| gate.wait(Duration::from_secs(5)).unwrap());
            assert_eq!(body.read(&mut bytes).unwrap(), 0);
            peer.join().unwrap();
        });
        assert_eq!(*gate.arrived.lock().unwrap(), 2);
        assert_eq!(body.read(&mut bytes).unwrap(), 0);
    }

    #[test]
    fn missing_stream_fails_instead_of_hanging() {
        let gate = PrefetchCompletionGate::new(2);
        assert_eq!(
            gate.wait(Duration::ZERO).unwrap_err().kind(),
            std::io::ErrorKind::TimedOut
        );
    }

    // Keep uncoordinated concurrent stress as well as the controlled peak
    // scenario. These assertions validate real download/verification behavior,
    // not timing or allocator-specific byte counts.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn eight_clients_verify_with_and_without_completion_alignment() {
        for aligned in [false, true] {
            let gate = Arc::new(PrefetchCompletionGate::new(32));
            let mut tasks = Vec::new();
            for _ in 0..8 {
                let fixture = if aligned {
                    PrefetchHarness::phase_aligned(65536, Arc::clone(&gate))
                        .await
                        .unwrap()
                } else {
                    PrefetchHarness::new(65536).await.unwrap()
                };
                tasks.push(tokio::spawn(async move {
                    fixture.prefetch().await.unwrap();
                    assert_eq!(
                        fixture
                            .client
                            .cache_config
                            .app_state_downloads
                            .budget()
                            .used(),
                        0
                    );
                }));
            }
            for task in tasks {
                task.await.unwrap();
            }
            assert_eq!(*gate.arrived.lock().unwrap(), if aligned { 32 } else { 0 });
        }
    }
}
