//! Fast connected-workload CPU/allocation-peak surfaces, NOT idle RSS.
//! Real elapsed checkpoints live in `examples/connected_idle.rs`; see `connected_idle.md`.

use whatsapp_rust::bench_support::connected_idle::{Activity, BackendFixture, Session};

struct Fixture<'a> {
    rt: &'a tokio::runtime::Runtime,
    session: Option<Session>,
    activity: Option<Activity>,
    backend: Option<BackendFixture>,
}

impl Drop for Fixture<'_> {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            self.rt
                .block_on(session.shutdown())
                .expect("fixture shutdown");
        }
        if let Some(backend) = self.backend.take() {
            backend.cleanup().expect("fixture database cleanup");
        }
    }
}

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("fixture runtime")
}

#[divan::bench(args = [false, true], sample_count = 5, sample_size = 1)]
fn activity_peak(bencher: divan::Bencher, sqlite: bool) {
    let rt = runtime();
    bencher
        .with_inputs(|| {
            rt.block_on(async {
                let backend = if sqlite {
                    BackendFixture::sqlite().await.expect("real SQLite backend")
                } else {
                    BackendFixture::memory()
                };
                let session = Session::connect(backend.backend()).await.expect("connect");
                let activity = session
                    .prepare_activity()
                    .await
                    .expect("prepare ciphertext/history");
                Fixture {
                    rt: &rt,
                    session: Some(session),
                    activity: Some(activity),
                    backend: Some(backend),
                }
            })
        })
        .bench_local_refs(|fixture| {
            // Input destruction (including graceful shutdown) is outside the timed region.
            rt.block_on(
                fixture
                    .session
                    .as_ref()
                    .expect("session")
                    .receive_activity(fixture.activity.take().expect("one activity per input")),
            )
            .expect("all messages committed and dispatched");
        });
}

#[divan::bench(args = [false, true], sample_count = 5, sample_size = 1)]
fn cache_maintenance_after_activity(bencher: divan::Bencher, sqlite: bool) {
    let rt = runtime();
    bencher
        .with_inputs(|| {
            rt.block_on(async {
                let backend = if sqlite {
                    BackendFixture::sqlite().await.expect("real SQLite backend")
                } else {
                    BackendFixture::memory()
                };
                let session = Session::connect(backend.backend()).await.expect("connect");
                let activity = session.prepare_activity().await.expect("prepare activity");
                session.receive_activity(activity).await.expect("activity");
                Fixture {
                    rt: &rt,
                    session: Some(session),
                    activity: None,
                    backend: Some(backend),
                }
            })
        })
        .bench_local_refs(|fixture| {
            rt.block_on(fixture.session.as_ref().expect("session").maintenance())
                .expect("cache maintenance");
        });
}
