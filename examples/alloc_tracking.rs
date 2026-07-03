//! Per-session ALLOCATOR attribution with the built-in [`AllocMeter`].
//!
//! Run with:
//!     cargo run --example alloc_tracking
//!
//! [`Client::memory_report`] answers "how many bytes do this session's
//! structures retain right now". This example answers the complementary
//! question — "how many bytes does this session's work allocate, including
//! transients" — using [`AllocMeter`], the first-class helper that attributes
//! allocations to a client through the `TaskInstrument` hook.
//!
//! The library still knows nothing about allocators. The host provides:
//!
//! 1. a global allocator that calls `AllocMeter::on_alloc` / `on_dealloc` on
//!    every (de)allocation (below — no external crates), and
//! 2. the meter itself, installed with `with_alloc_meter`, which marks per
//!    thread *which* client's task is being polled so the charge lands on it.
//!
//! Everything else — the thread-local routing, the nesting-safe stack — lives
//! in `AllocMeter`. Compare the ~20 lines here to hand-rolling the bucket
//! logic. The same allocator pattern plugs in `tracking-allocator`, dhat, or
//! ESP-IDF `heap_caps` sampling. Expect a measurable overhead while enabled; it
//! is a diagnostics tool, not an always-on meter. Allocations outside
//! instrumented tasks (your caller-side code, other libraries) are uncounted.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};
use wacore::stats::AllocMeter;
use whatsapp_rust::prelude::*;

/// A global allocator that forwards every (de)allocation size to whichever
/// `AllocMeter` is active on the current thread. `AllocMeter::on_alloc` is
/// allocation-free, so this does not recurse.
struct AttributingAllocator;

unsafe impl GlobalAlloc for AttributingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // Count only a successful allocation, so an OOM null return doesn't
        // inflate the counter with bytes that were never allocated.
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            AllocMeter::on_alloc(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        AllocMeter::on_dealloc(layout.size());
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: AttributingAllocator = AttributingAllocator;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(async {
        let store = match SqliteStore::new("session_a.db").await {
            Ok(store) => store,
            Err(e) => {
                error!("failed to create SQLite backend: {e}");
                return;
            }
        };

        // One meter per session. `with_alloc_meter` installs it as the task
        // instrument AND keeps a handle so `client.resource_report()` folds in
        // its snapshot; we keep our own clone to read it directly here.
        let meter = Arc::new(AllocMeter::new());

        let bot = Bot::builder()
            .with_backend(store)
            .with_alloc_meter(meter.clone())
            .on_qr_code(|code, timeout| async move {
                info!("QR code (valid {}s):\n{code}", timeout.as_secs());
            })
            .build()
            .await;

        let bot = match bot {
            Ok(bot) => bot,
            Err(e) => {
                error!("failed to build bot: {e}");
                return;
            }
        };

        let client = bot.client();
        let run = tokio::spawn(bot.run());

        let mut ticker = tokio::time::interval(Duration::from_secs(10));
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let snap = meter.snapshot();
                    info!(
                        "session A alloc churn: {}B allocated / {}B freed across {} allocs (net {}B)",
                        snap.allocated_bytes, snap.freed_bytes, snap.allocations, snap.net_bytes(),
                    );
                    // The unified view: client collections + storage + transport
                    // + http + this alloc snapshot, in one report.
                    info!("\n{}", client.resource_report().await);
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Shutting down...");
                    client.disconnect().await;
                    break;
                }
            }
        }
        let _ = run.await;
    });
}
