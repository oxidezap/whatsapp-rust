//! Integration benchmarks driven through the real async client against the
//! bartender mock server. Ported from a custom timing/allocation binary to
//! divan so CodSpeed records deterministic memory metrics (replacing the old
//! counting allocator), simulation instruction counts, and flamegraphs.
//!
//! These cannot run without the mock server (`MOCK_SERVER_URL`), so they only
//! execute in CI under `cargo codspeed run`.

// Large `--all-features` async fns (tracing + tracing-pii) need a deeper
// recursion limit; matches `src/lib.rs` and the e2e crate.
#![recursion_limit = "512"]
// Host-only bench harness (counting allocator + unique-body counter); std's
// 64-bit atomic is fine since this never builds for embedded targets.
#![allow(clippy::disallowed_types)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use e2e_tests::{TestClient, text_msg};
use tokio::sync::Mutex;
use whatsapp_rust::Jid;

// Deterministic allocator for CodSpeed's memory instrument: `realloc` always
// allocates a fresh block and copies, never growing in place. Whether the system
// allocator can grow in place depends on the live heap layout, which differs
// run-to-run and would otherwise be charged to the benchmark as memory noise (the
// CodSpeed `reducing-variance` recipe). Bench-only; production keeps the default.
struct DeterministicAlloc;

unsafe impl GlobalAlloc for DeterministicAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe {
            let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());
            let new_ptr = System.alloc(new_layout);
            if !new_ptr.is_null() {
                std::ptr::copy_nonoverlapping(ptr, new_ptr, layout.size().min(new_size));
                System.dealloc(ptr, layout);
            }
            new_ptr
        }
    }
}

#[global_allocator]
static GLOBAL: DeterministicAlloc = DeterministicAlloc;

fn main() {
    divan::main();
}

// A single multi-thread runtime pinned to a fixed worker count, shared by every
// bench. The default worker count tracks the runner's CPU count, so thread stacks and
// scheduling varied per runner; pinning it makes the benches runner-independent. A
// fixed count rather than current_thread because the background event drainers
// (connect_warmed_pair) must keep draining between divan samples to preserve
// cross-sample isolation, and current_thread freezes spawned tasks outside block_on.
// Built once so runtime startup isn't charged to the measured region.
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        // Best-effort logger init, mirroring the old binary; ignore errors so a
        // second bench module call is harmless.
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
            .try_init()
            .ok();
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("build bench runtime")
    })
}

/// A connected, session-warmed client pair. Connecting + warming a pair is far
/// more expensive than the measured send, so it is done once (unmeasured) and
/// reused across iterations. `wait_for_text` needs `&mut`, hence the `Mutex`.
struct Pair {
    a: TestClient,
    b: TestClient,
    jid_b: Jid,
}

// Separate pairs for the sender-only and round-trip benches. `send_message`
// never reads client `b`, so sharing one pair would force `send_and_receive`'s
// measured `wait_for_text` to discard the send backlog first, leaking the send
// bench's iteration count into the round-trip result. Dedicated pairs keep each
// bench independent of the other (and of divan's run order); the send pair also
// drains `b` in the background so its queue stays bounded (see below).
static PAIR_SEND: OnceLock<Mutex<Pair>> = OnceLock::new();
static PAIR_RECV: OnceLock<Mutex<Pair>> = OnceLock::new();

/// Monotonic counter producing a unique body per measured iteration. The mock
/// server dedupes identical message bodies, so reusing one would let later
/// iterations short-circuit and stop measuring the real send path.
static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_body(tag: &str) -> String {
    format!("bench-{tag}-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

/// Connect two clients and warm their Signal session with one throwaway
/// round-trip, so measured sends exercise steady state (plain `msg`), not the
/// first-message pre-key path.
///
/// No bench reads the clients' event channels, so left alone they grow without
/// bound across divan samples — delivery receipts pile up on `a`, delivered
/// messages on `b` — making CodSpeed memory/simulation numbers depend on how
/// many sends ran earlier. A background task drains each unread channel: `a`
/// always (only receipts land there), `b` only when `drain_b` is set. The
/// round-trip pair leaves `drain_b` off, since `send_and_receive` consumes `b`
/// itself via `wait_for_text`.
fn connect_warmed_pair(prefix_a: &str, prefix_b: &str, drain_b: bool) -> Mutex<Pair> {
    rt().block_on(async {
        let a = TestClient::connect(prefix_a)
            .await
            .expect("connect client a");
        let mut b = TestClient::connect(prefix_b)
            .await
            .expect("connect client b");
        let jid_b = b.jid().await;

        let warm = unique_body("warmup");
        a.client
            .send_message(jid_b.clone(), text_msg(&warm))
            .await
            .expect("send warmup message");
        b.wait_for_text(&warm, 30).await.expect("receive warmup");

        // Started after warmup so the drainers can't steal the warmup
        // message/receipt above.
        {
            let rx = a.event_rx.clone();
            tokio::spawn(async move { while rx.recv().await.is_ok() {} });
        }
        if drain_b {
            let rx = b.event_rx.clone();
            tokio::spawn(async move { while rx.recv().await.is_ok() {} });
        }

        Mutex::new(Pair { a, b, jid_b })
    })
}

fn pair_send() -> &'static Mutex<Pair> {
    PAIR_SEND.get_or_init(|| connect_warmed_pair("bench_send_a", "bench_send_b", true))
}

fn pair_recv() -> &'static Mutex<Pair> {
    PAIR_RECV.get_or_init(|| connect_warmed_pair("bench_recv_a", "bench_recv_b", false))
}

// x20 coverage: divan's `args` runs the bench once per value and reports each
// separately, so one fn yields both the single-op result (n=1) and the 20-op
// batch (n=20) — the loop runs inside the measured region. This preserves the
// original single + x20 signal without a duplicate fn; the batch total stands
// in for the old hand-divided amortized number.
const BATCH_SIZES: [u64; 2] = [1, 20];

/// Client creation through Connected (ready). The runtime is initialized
/// outside the measured closure so first-call thread-pool/logger startup is not
/// charged to the result. The cheap disconnect stays inside on purpose: the
/// connect handshake dominates its cost, and tearing the client down each
/// iteration stops sessions leaking across iterations on the mock server.
#[divan::bench]
fn connect_to_ready(bencher: divan::Bencher) {
    let rt = rt();
    bencher.bench_local(|| {
        rt.block_on(async {
            let c = TestClient::connect("bench_connect")
                .await
                .expect("connect client");
            c.disconnect().await;
        });
    });
}

/// Sending a single DM (sender side only, matching the original — no
/// `wait_for_text` here). Covers protobuf encode, Signal encrypt, node marshal
/// and the WebSocket write. The runtime and warmed pair are initialized outside
/// the measured closure so connect + Signal warmup are not charged to the send.
#[divan::bench(args = BATCH_SIZES)]
fn send_message(bencher: divan::Bencher, n: u64) {
    let rt = rt();
    let pair = pair_send();
    bencher
        // Build the unique messages in unmeasured setup so the body `format!`
        // and protobuf `String` allocations aren't charged to the send path.
        .with_inputs(|| {
            (0..n)
                .map(|_| text_msg(&unique_body("send")))
                .collect::<Vec<_>>()
        })
        .bench_local_values(|msgs| {
            rt.block_on(async {
                let guard = pair.lock().await;
                for msg in msgs {
                    guard
                        .a
                        .client
                        .send_message(guard.jid_b.clone(), msg)
                        .await
                        .expect("send message");
                }
            });
        });
}

/// Full send + receive round-trip on the warmed pair. Runtime and pair are
/// initialized outside the measured closure (see `send_message`).
#[divan::bench(args = BATCH_SIZES)]
fn send_and_receive(bencher: divan::Bencher, n: u64) {
    let rt = rt();
    let pair = pair_recv();
    bencher
        // Messages are built in unmeasured setup (see `send_message`); the body
        // is kept alongside to match the delivered text in `wait_for_text`.
        .with_inputs(|| {
            (0..n)
                .map(|_| {
                    let body = unique_body("recv");
                    (text_msg(&body), body)
                })
                .collect::<Vec<_>>()
        })
        .bench_local_values(|items| {
            rt.block_on(async {
                let mut guard = pair.lock().await;
                for (msg, body) in items {
                    guard
                        .a
                        .client
                        .send_message(guard.jid_b.clone(), msg)
                        .await
                        .expect("send message");
                    guard
                        .b
                        .wait_for_text(&body, 30)
                        .await
                        .expect("receive text");
                }
            });
        });
}

/// A reconnect cycle (disconnect -> reconnect -> ready). The client is created
/// once outside the measured region; only `reconnect_and_wait` is measured.
#[divan::bench]
fn reconnect(bencher: divan::Bencher) {
    let rt = rt();
    let client: &'static Mutex<TestClient> = {
        static RECONNECT: OnceLock<Mutex<TestClient>> = OnceLock::new();
        RECONNECT.get_or_init(|| {
            rt.block_on(async {
                Mutex::new(
                    TestClient::connect("bench_reconn")
                        .await
                        .expect("connect reconnect client"),
                )
            })
        })
    };

    bencher
        // Drain events buffered since the previous reconnect (late init-IQ
        // responses) in unmeasured setup, so `reconnect_and_wait`'s own
        // start-of-call `try_recv` drain doesn't charge the prior sample's
        // leftovers to this one.
        .with_inputs(|| {
            rt.block_on(async {
                let guard = client.lock().await;
                while guard.event_rx.try_recv().is_ok() {}
            });
        })
        .bench_local_values(|()| {
            rt.block_on(async {
                client
                    .lock()
                    .await
                    .reconnect_and_wait()
                    .await
                    .expect("reconnect and wait");
            });
        });
}
