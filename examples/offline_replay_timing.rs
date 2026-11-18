#![allow(clippy::print_stdout)]

use anyhow::{Context, Result, bail};
use whatsapp_rust::bench_support::connected_idle::{
    BackendFixture,
    offline_replay::{REPLAY_MESSAGES, REPLAY_PAGES, Replay},
};

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
            _ => bail!("backend must be memory or sqlite"),
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
