#![allow(clippy::print_stdout)]

use whatsapp_rust::bench_support::connected_idle::{
    BackendFixture,
    offline_replay::{REPLAY_MESSAGES, REPLAY_PAGES, Replay},
};

type Result<T, E = Box<dyn std::error::Error + Send + Sync>> = std::result::Result<T, E>;

trait ContextExt<T> {
    fn context<C: std::fmt::Display>(self, msg: C) -> Result<T>;
}
impl<T> ContextExt<T> for Option<T> {
    fn context<C: std::fmt::Display>(self, msg: C) -> Result<T> {
        self.ok_or_else(|| msg.to_string().into())
    }
}
impl<T, E: std::fmt::Display> ContextExt<T> for std::result::Result<T, E> {
    fn context<C: std::fmt::Display>(self, msg: C) -> Result<T> {
        self.map_err(|e| format!("{msg}: {e}").into())
    }
}

fn main() -> Result<()> {
    let backend = std::env::args()
        .nth(1)
        .context("usage: offline_replay_timing <memory|sqlite>")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let fixture = match backend.as_str() {
            "memory" => BackendFixture::memory(),
            "sqlite" => BackendFixture::sqlite().await?,
            _ => return Err("backend must be memory or sqlite".into()),
        };
        let mut replay = Replay::prepare(fixture.backend(), true).await?;
        let timings = replay.receive().await?.context("wall timings were not collected")?;
        replay.shutdown().await?;
        fixture.cleanup()?;
        println!(
            "{backend} messages={REPLAY_MESSAGES} pages={REPLAY_PAGES} request_span_us={} first_publication_us={} last_publication_us={} all_receipts_us={}",
            timings.request_span.as_micros(),
            timings.first_publication.as_micros(),
            timings.last_publication.as_micros(),
            timings.all_receipts.as_micros(),
        );
        Ok(())
    })
}
