//! Lifecycle proof only: virtual Tokio time is not a native memory measurement.

#[path = "../src/bench_support/connected_idle/util.rs"]
mod fixture_util;

use anyhow::{Result, ensure};
use whatsapp_rust::bench_support::connected_idle::{BackendFixture, LANES, Session};

#[tokio::test(start_paused = true)]
async fn connected_activity_and_idle_lifecycle() -> Result<()> {
    let backend = BackendFixture::memory();
    let session = Session::connect(backend.backend()).await?;
    let activity = session.prepare_activity().await?;
    session.receive_activity(activity).await?;
    let busy = session.checkpoint().await;
    ensure!(
        busy.connected && busy.open_lanes == LANES,
        "activity did not create live workers: {busy:?}"
    );
    session.idle().await?;
    let idle = session.checkpoint().await;
    ensure!(
        idle.connected && idle.open_lanes == 0 && idle.running_workers == 0,
        "{idle:?}"
    );
    session.maintenance().await?;
    ensure!(
        session.checkpoint().await.connected,
        "maintenance disconnected client"
    );
    session.shutdown().await?;
    backend.cleanup()
}

#[cfg(feature = "sqlite-storage")]
#[tokio::test]
async fn sqlite_connected_activity_and_virtual_idle() -> Result<()> {
    let backend = BackendFixture::sqlite().await?;
    let session = Session::connect(backend.backend()).await?;
    let activity = session.prepare_activity().await?;
    session.receive_activity(activity).await?;
    ensure!(
        session.checkpoint().await.open_lanes == LANES,
        "SQLite activity missed workers"
    );
    tokio::time::pause();
    session.idle().await?;
    tokio::time::resume();
    let idle = session.checkpoint().await;
    ensure!(idle.connected && idle.running_workers == 0, "{idle:?}");
    session.maintenance().await?;
    session.shutdown().await?;
    backend.cleanup()
}

#[tokio::test]
async fn shutdown_releases_active_workers_before_backend_cleanup() -> Result<()> {
    #[cfg(feature = "sqlite-storage")]
    let backends = [BackendFixture::memory(), BackendFixture::sqlite().await?];
    #[cfg(not(feature = "sqlite-storage"))]
    let backends = [BackendFixture::memory()];
    for backend in backends {
        let session = match Session::connect(backend.backend()).await {
            Ok(session) => session,
            Err(error) => {
                let cleanup = backend.cleanup();
                return Err::<(), _>(error).and(cleanup);
            }
        };
        let activity_result = async {
            let activity = session.prepare_activity().await?;
            session.receive_activity(activity).await?;
            ensure!(
                session.checkpoint().await.running_workers > 0,
                "test must disconnect before idle retirement"
            );
            Ok::<_, anyhow::Error>(())
        }
        .await;
        let shutdown = session.shutdown().await;
        let cleanup = backend.cleanup();
        activity_result.and(shutdown).and(cleanup)?;
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn connected_control_stays_alive_without_activity() -> Result<()> {
    let backend = BackendFixture::memory();
    let session = Session::connect(backend.backend()).await?;
    session.finish_control().await?;
    session.idle().await?;
    // Recent-activity decisions use wacore's real monotonic clock, not Tokio
    // time. The native measurement verifies periodic keepalive instead.
    session.probe().await?;
    let control = session.checkpoint().await;
    ensure!(
        control.connected && control.lanes == 0 && control.messages == 0,
        "{control:?}"
    );
    ensure!(
        session.pongs() > 0,
        "keepalive never reached the synthetic server"
    );
    session.shutdown().await?;
    backend.cleanup()
}
