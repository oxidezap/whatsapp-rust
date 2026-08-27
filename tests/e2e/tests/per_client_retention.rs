//! Per-client retained heap, measured at the allocator.
//!
//! `process_footprint.rs` answers the same question in `RssAnon`, which is
//! page-quantised: a 2 KiB structure and a 3 KiB one both read as "one page, or
//! none". This file counts bytes as the global allocator hands them out, so a
//! single preallocated queue is separable from the client that owns it. That is
//! the resolution an audit of *which* per-client structure costs what needs;
//! `RssAnon` remains the right instrument for what a consumer actually pays.
//!
//! Both tests are `#[ignore]`d: they are measuring tools, not guards. A number
//! worth guarding gets a deterministic observable instead (see
//! `agent_docs/observability.md`, "Should a residency probe be permanent?").
//!
//! Run: `cargo nextest run --profile e2e -p e2e-tests --run-ignored all -E
//! 'binary(per_client_retention)' --no-capture`

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, Ordering};

use log::info;
use wacore::net::TransportEvent;
use wacore::store::InMemoryBackend;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::portable_cache::PortableCache;
use whatsapp_rust::sync_task::MajorSyncTask;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

/// Live bytes handed out by the global allocator and not yet returned.
///
/// Signed: a measurement window can free more than it allocates (a builder that
/// drops its inputs), and clamping that to zero would silently turn a net
/// release into "no change".
static LIVE: AtomicIsize = AtomicIsize::new(0);

struct Counting;

// SAFETY: every method forwards to `System` with the same layout it received
// and only adds a relaxed counter update, so the allocator contract is exactly
// `System`'s.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            LIVE.fetch_add(layout.size() as isize, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            LIVE.fetch_add(
                new_size as isize - layout.size() as isize,
                Ordering::Relaxed,
            );
        }
        new_ptr
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn live() -> isize {
    LIVE.load(Ordering::Relaxed)
}

/// Live-heap growth across `build`, with the built value kept alive.
///
/// Returns the value so the caller decides when it dies; measuring a value that
/// has already been dropped would report zero for every structure.
fn retained<T>(build: impl FnOnce() -> T) -> (T, isize) {
    let before = live();
    let value = build();
    (value, live() - before)
}

fn median(mut v: Vec<isize>) -> isize {
    v.sort_unstable();
    v[v.len() / 2]
}

/// The counter itself, since every figure below is only as good as it is.
///
/// Not `#[ignore]`d, unlike the measurements: this one is fast, deterministic
/// and is the thing a reader has to trust. Bounds rather than equalities,
/// because `LIVE` is process-wide and the harness threads allocate too — one
/// large allocation makes that noise irrelevant to the assertions.
#[test]
fn the_allocator_counts_what_it_hands_out() {
    const SIZE: usize = 4 * 1024 * 1024;

    let (buffer, alloc_bytes) = retained(|| vec![0u8; SIZE]);
    assert!(
        alloc_bytes >= SIZE as isize,
        "an allocation must raise the live count by at least its size, got {alloc_bytes}"
    );

    // realloc: growing in place or by copy must be counted as the delta, not as
    // a second whole allocation.
    let (grown, realloc_bytes) = retained(|| {
        let mut buffer = buffer;
        buffer.resize(SIZE * 2, 0);
        buffer
    });
    assert!(
        realloc_bytes >= SIZE as isize,
        "growing by SIZE must raise the count by about SIZE, got {realloc_bytes}"
    );
    assert!(
        realloc_bytes < (2 * SIZE) as isize,
        "growing by SIZE must not be counted as a fresh 2*SIZE allocation, got {realloc_bytes}"
    );

    let before_drop = live();
    drop(grown);
    let freed = before_drop - live();
    assert!(
        freed >= (2 * SIZE) as isize,
        "dropping must return the whole allocation to the count, got {freed}"
    );
}

/// What the two per-client bounded queues cost, and what fraction of a client
/// that is.
///
/// The capacities are the constants the client and the Tokio transport build
/// with (`Client::new` and `EVENT_CHANNEL_CAPACITY`); they are restated here
/// because a channel's preallocation is not reachable from outside it, so the
/// only way to price one is to build an identical channel.
#[test]
#[ignore]
fn preallocated_queue_slots() {
    let _ = env_logger::builder().is_test(true).try_init();

    let (major_sync, major_sync_bytes) = retained(|| async_channel::bounded::<MajorSyncTask>(32));
    let (transport, transport_bytes) = retained(|| async_channel::bounded::<TransportEvent>(64));

    info!(
        "major_sync_task_sender   capacity=32 payload={:>4} B   retained={:>6} B",
        size_of::<MajorSyncTask>(),
        major_sync_bytes
    );
    info!(
        "transport events         capacity=64 payload={:>4} B   retained={:>6} B",
        size_of::<TransportEvent>(),
        transport_bytes
    );

    drop(major_sync);
    drop(transport);

    // Both channels are preallocated in full at construction: a slot array, not
    // a growing queue. If either ever reported ~0 the measurement would be
    // reading a lazily-allocated buffer instead of the reservation.
    assert!(
        major_sync_bytes > 0 && transport_bytes > 0,
        "a bounded channel must retain its slot array on construction"
    );
}

/// A configured capacity is a bound, not a reservation.
///
/// Worth measuring rather than assuming, because the sibling test above shows
/// the opposite case: a bounded channel *does* size its allocation by capacity.
/// `PortableCache` starts on an empty `HashMap`, so a client carrying
/// `session_locks_capacity = 10_000` pays for none of it until entries arrive —
/// raising a cap costs nothing at construction.
#[test]
#[ignore]
fn cache_capacity_is_not_preallocated() {
    let _ = env_logger::builder().is_test(true).try_init();

    let (small, small_bytes) = retained(|| {
        PortableCache::<String, u64>::builder()
            .max_capacity(1)
            .build()
    });
    let (large, large_bytes) = retained(|| {
        PortableCache::<String, u64>::builder()
            .max_capacity(10_000)
            .build()
    });

    info!("cache capacity=1      retained={small_bytes:>6} B");
    info!("cache capacity=10000  retained={large_bytes:>6} B");

    drop(small);
    drop(large);

    assert_eq!(
        small_bytes, large_bytes,
        "a capacity cap must not size any allocation, or every cap becomes a per-client cost"
    );
}

/// Retained heap per constructed `Client`, at allocator resolution, with the
/// pieces a consumer supplies priced separately.
///
/// Needs no mock server: `build()` performs no I/O, so the transport factory
/// points at a port nothing listens on (same recipe as
/// `process_footprint.rs::client_construction_footprint`).
///
/// Report the median of the tail, not the mean over all: the first clients pay
/// for process-wide lazily-built state (codec tables, TLS roots) that no later
/// client repeats, which is exactly the asymmetry a per-client figure must not
/// absorb.
///
/// The counter is process-wide, so in principle a build window can absorb
/// allocation from an earlier client's background tasks — the clients stay alive
/// and the runtime is multi-threaded. The median over the tail is the guard
/// against that, and the check that it worked is in the output: the
/// `http+client` column comes out flat to within a few bytes across clients
/// 2..N, and the same under `dev` and `release`. Read that column and read the
/// table, not only the summary — a run whose tail is not flat is a run where
/// something else was allocating, and its median means less.
#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn constructed_client_retained_heap() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let n: usize = std::env::var("RETENTION_CLIENTS")
        .ok()
        .map(|raw| raw.parse().expect("RETENTION_CLIENTS is not a number"))
        .unwrap_or(16);
    assert!(n >= 3, "need a steady-state tail to take a marginal from");

    let mut bots = Vec::with_capacity(n);
    let mut backend_deltas = Vec::with_capacity(n);
    let mut http_deltas = Vec::with_capacity(n);
    let mut client_deltas = Vec::with_capacity(n);

    for i in 0..n {
        // Built outside the client window and priced on their own: both are
        // consumer-supplied and shareable (`with_http_client` takes a clone of
        // one agent), so folding them into the client figure would report a
        // cost a multi-session process may not pay per session.
        let (backend, backend_bytes) = retained(|| Arc::new(InMemoryBackend::new()));
        let (http, http_bytes) = retained(UreqHttpClient::new);

        let before = live();
        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(
                whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory::new()
                    .with_url("wss://127.0.0.1:1/ws/chat"),
            )
            .with_http_client(http)
            .with_runtime(whatsapp_rust::TokioRuntime)
            .with_version((2, 3000, 0))
            .with_push_name(format!("retention_{i}"))
            .build()
            .await?;
        let client_bytes = live() - before;

        bots.push(bot);
        backend_deltas.push(backend_bytes);
        http_deltas.push(http_bytes);
        client_deltas.push(client_bytes);
        info!(
            "client {:>3}   backend={:>7} B  http={:>6} B  client={:>8} B  http+client={:>8} B",
            i + 1,
            backend_bytes,
            http_bytes,
            client_bytes,
            http_bytes + client_bytes
        );
    }

    let tail = |v: &[isize]| median(v.iter().copied().skip(2).collect());
    // `http` and `client` are reported together as well as apart: ureq
    // materializes part of its agent lazily, so which of the two windows a
    // given allocation lands in drifts by a few hundred bytes between runs
    // while their sum is stable to the byte. Quote the sum.
    info!(
        "marginal (median 3..N)   backend={:>7} B  http={:>6} B  client={:>8} B  http+client={:>8} B",
        tail(&backend_deltas),
        tail(&http_deltas),
        tail(&client_deltas),
        tail(
            &http_deltas
                .iter()
                .zip(&client_deltas)
                .map(|(h, c)| h + c)
                .collect::<Vec<_>>()
        )
    );
    info!(
        "first client             backend={:>7} B  http={:>6} B  client={:>8} B  http+client={:>8} B",
        backend_deltas[0],
        http_deltas[0],
        client_deltas[0],
        http_deltas[0] + client_deltas[0]
    );

    assert_eq!(bots.len(), n);
    Ok(())
}
