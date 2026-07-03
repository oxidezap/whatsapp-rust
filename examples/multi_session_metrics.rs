//! Two WhatsApp sessions in ONE process, each with its own precise metrics.
//!
//! Run with:
//!     cargo run --example multi_session_metrics
//!
//! Demonstrates the per-session observability surface:
//! - [`Client::stats`] — always-on wire I/O counters (bytes/frames/messages,
//!   reconnects, activity timestamps). One relaxed atomic add per frame.
//! - [`Client::memory_report`] — on-demand entry counts + estimated retained
//!   heap bytes of every internal collection. Costs nothing unless called.
//! - [`CpuMeter`] via `BotBuilder::with_task_instrument` — busy time (CPU
//!   proxy) and poll count of each client's internal tasks. Runtime-agnostic:
//!   the same hook works on Tokio, wasm or embedded runtimes.
//!
//! Each client gets its own SQLite store (multi-account pattern), its own
//! `CpuMeter`, and reports independently — no process spawning needed.

use std::sync::Arc;
use std::time::Duration;

use log::{error, info};
use wacore::stats::CpuMeter;
use whatsapp_rust::prelude::*;

async fn build_session(db_path: &str, label: &'static str) -> Option<(Bot, Arc<CpuMeter>)> {
    let store = match SqliteStore::new(db_path).await {
        Ok(store) => store,
        Err(e) => {
            error!("[{label}] failed to create SQLite backend: {e}");
            return None;
        }
    };

    // Keep a clone of the meter: the builder takes it as the task hook, we
    // read snapshots from ours.
    let cpu_meter = Arc::new(CpuMeter::new());

    let bot = Bot::builder()
        .with_backend(store)
        .with_task_instrument(cpu_meter.clone())
        .on_qr_code(move |code, timeout| async move {
            info!("[{label}] QR code (valid {}s):\n{code}", timeout.as_secs());
        })
        .build()
        .await;

    match bot {
        Ok(bot) => Some((bot, cpu_meter)),
        Err(e) => {
            error!("[{label}] failed to build bot: {e}");
            None
        }
    }
}

async fn print_session_metrics(label: &str, client: &Arc<Client>, cpu: &CpuMeter) {
    let stats = client.stats();
    let memory = client.memory_report().await;
    let cpu = cpu.snapshot();

    info!(
        "[{label}] wire: {}B out / {}B in ({} / {} frames) | msgs: {} sent / {} recv | reconnects: {}",
        stats.bytes_sent,
        stats.bytes_received,
        stats.frames_sent,
        stats.frames_received,
        stats.messages_sent,
        stats.messages_received,
        stats.reconnects,
    );
    info!(
        "[{label}] cpu: {:?} busy over {} polls | retained heap (est.): {}B (signal: {}B in {} sessions)",
        cpu.busy,
        cpu.polls,
        memory.total_estimated_bytes(),
        memory.signal_sessions.bytes,
        memory.signal_sessions.entries,
    );
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(async {
        let Some((bot_a, cpu_a)) = build_session("session_a.db", "A").await else {
            return;
        };
        let Some((bot_b, cpu_b)) = build_session("session_b.db", "B").await else {
            return;
        };

        let client_a = bot_a.client();
        let client_b = bot_b.client();

        let run_a = tokio::spawn(bot_a.run());
        let run_b = tokio::spawn(bot_b.run());

        // Report both sessions side by side every 10s.
        let mut ticker = tokio::time::interval(Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    print_session_metrics("A", &client_a, &cpu_a).await;
                    print_session_metrics("B", &client_b, &cpu_b).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down both sessions...");
                    client_a.disconnect().await;
                    client_b.disconnect().await;
                    break;
                }
            }
        }

        let _ = run_a.await;
        let _ = run_b.await;
    });
}
