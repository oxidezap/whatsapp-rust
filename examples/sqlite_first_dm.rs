//! File-backed first DM and explicit Signal flush with SQLite synchronous=FULL.

#![allow(clippy::print_stdout)]

use whatsapp_rust::bench_support::connected_idle::sqlite_first_dm;

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .start_paused(true)
        .build()?;
    let sample = runtime.block_on(sqlite_first_dm())?;
    println!("{}", serde_json::to_string(&sample)?);
    Ok(())
}
