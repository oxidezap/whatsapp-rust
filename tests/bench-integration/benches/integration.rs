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

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use e2e_tests::{TestClient, text_msg};
use tokio::sync::Mutex;
use whatsapp_rust::Jid;

fn main() {
    divan::main();
}

// A single multi-thread runtime shared by every bench. Building one per
// iteration would charge thread-pool startup syscalls to the measured region,
// and the real client expects a multi-thread scheduler.
static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        // Best-effort logger init, mirroring the old binary; ignore errors so a
        // second bench module call is harmless.
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
            .try_init()
            .ok();
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("build multi-thread bench runtime")
    })
}

/// A connected, session-warmed client pair reused across all send/receive
/// iterations. Connecting + warming a pair is far more expensive than the
/// measured send, so it is done once (unmeasured) and shared. `wait_for_text`
/// needs `&mut`, hence the `Mutex`.
struct Pair {
    a: TestClient,
    b: TestClient,
    jid_b: Jid,
}

static PAIR: OnceLock<Mutex<Pair>> = OnceLock::new();

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
fn pair() -> &'static Mutex<Pair> {
    PAIR.get_or_init(|| {
        rt().block_on(async {
            let a = TestClient::connect("bench_pair_a")
                .await
                .expect("connect client a");
            let mut b = TestClient::connect("bench_pair_b")
                .await
                .expect("connect client b");
            let jid_b = b.jid().await;

            let warm = unique_body("warmup");
            a.client
                .send_message(jid_b.clone(), text_msg(&warm))
                .await
                .expect("send warmup message");
            b.wait_for_text(&warm, 30).await.expect("receive warmup");

            Mutex::new(Pair { a, b, jid_b })
        })
    })
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
    let pair = pair();
    bencher.bench_local(|| {
        rt.block_on(async {
            let guard = pair.lock().await;
            for _ in 0..n {
                let body = unique_body("send");
                guard
                    .a
                    .client
                    .send_message(guard.jid_b.clone(), text_msg(&body))
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
    let pair = pair();
    bencher.bench_local(|| {
        rt.block_on(async {
            let mut guard = pair.lock().await;
            for _ in 0..n {
                let body = unique_body("recv");
                guard
                    .a
                    .client
                    .send_message(guard.jid_b.clone(), text_msg(&body))
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
    let client: &'static Mutex<TestClient> = {
        static RECONNECT: OnceLock<Mutex<TestClient>> = OnceLock::new();
        RECONNECT.get_or_init(|| {
            rt().block_on(async {
                Mutex::new(
                    TestClient::connect("bench_reconn")
                        .await
                        .expect("connect reconnect client"),
                )
            })
        })
    };

    bencher.bench_local(|| {
        rt().block_on(async {
            client
                .lock()
                .await
                .reconnect_and_wait()
                .await
                .expect("reconnect and wait");
        });
    });
}
