//! Per-session ALLOCATOR attribution through the `TaskInstrument` hook.
//!
//! Run with:
//!     cargo run --example alloc_tracking
//!
//! [`Client::memory_report`] answers "how many bytes do this session's
//! structures retain right now". This example answers the complementary
//! question — "how many bytes does this session's work allocate, including
//! transients" — by combining:
//!
//! 1. a hand-rolled counting global allocator (no external crates), and
//! 2. a [`TaskInstrument`] that marks, per thread, which session's task is
//!    currently being polled, so the allocator knows whom to charge.
//!
//! The library knows nothing about allocators: the hook is just enter/exit
//! around every poll of a client's internal tasks. The same pattern plugs in
//! `tracking-allocator`, dhat, or ESP-IDF `heap_caps` sampling instead of
//! this toy allocator. Expect a measurable overhead while enabled (Vector
//! reports ~10-20% for the equivalent design); it is a diagnostics tool, not
//! an always-on meter. Allocations outside instrumented tasks (your own
//! caller-side code, other libraries) land in the "untracked" bucket.

use portable_atomic::{AtomicI64, Ordering};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::Arc;
use std::time::Duration;

use log::{error, info};
use wacore::stats::TaskInstrument;
use whatsapp_rust::prelude::*;

/// Live allocation counters for one attribution bucket.
#[derive(Default)]
struct Bucket {
    current_bytes: AtomicI64,
    total_allocs: AtomicI64,
}

static SESSION_A: Bucket = Bucket {
    current_bytes: AtomicI64::new(0),
    total_allocs: AtomicI64::new(0),
};
static UNTRACKED: Bucket = Bucket {
    current_bytes: AtomicI64::new(0),
    total_allocs: AtomicI64::new(0),
};

thread_local! {
    /// Which bucket the currently-polled task belongs to. `None` = untracked.
    static CURRENT: Cell<Option<&'static Bucket>> = const { Cell::new(None) };
}

struct AttributingAllocator;

impl AttributingAllocator {
    fn bucket() -> &'static Bucket {
        CURRENT
            .try_with(|c| c.get())
            .ok()
            .flatten()
            .unwrap_or(&UNTRACKED)
    }
}

// SAFETY-adjacent caveat: deallocations are charged to the CURRENT bucket,
// not the allocating one, so a buffer allocated inside session A but freed
// elsewhere skews both buckets. Fine for a demo; real deployments use a
// tracker that stores the group per allocation (e.g. tracking-allocator).
unsafe impl GlobalAlloc for AttributingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let bucket = Self::bucket();
        bucket
            .current_bytes
            .fetch_add(layout.size() as i64, Ordering::Relaxed);
        bucket.total_allocs.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        Self::bucket()
            .current_bytes
            .fetch_sub(layout.size() as i64, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: AttributingAllocator = AttributingAllocator;

/// TaskInstrument that scopes the thread to a bucket for the poll's duration.
struct SessionAllocScope(&'static Bucket);

impl TaskInstrument for SessionAllocScope {
    fn on_poll_start(&self) {
        CURRENT.with(|c| c.set(Some(self.0)));
    }
    fn on_poll_end(&self) {
        CURRENT.with(|c| c.set(None));
    }
}

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

        let bot = Bot::builder()
            .with_backend(store)
            .with_task_instrument(Arc::new(SessionAllocScope(&SESSION_A)))
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
                    info!(
                        "session A: {}B live across {} allocs | untracked: {}B live",
                        SESSION_A.current_bytes.load(Ordering::Relaxed),
                        SESSION_A.total_allocs.load(Ordering::Relaxed),
                        UNTRACKED.current_bytes.load(Ordering::Relaxed),
                    );
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
