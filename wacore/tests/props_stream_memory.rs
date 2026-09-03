//! Peak heap of receiving a full A/B props response, measured at the
//! allocator.
//!
//! The props catalog is the largest frame of an ordinary login: a few thousand
//! `<prop>` children, ~28 KB compressed as the server sends it. On an ESP32-C3
//! (no PSRAM, ~90 KB of heap free once the connection is up, 58 KB at the
//! moment the frame arrives) decoding it as a tree aborted the process every
//! time. This test pins the receive path's cost for that frame so a regression
//! shows up here, on a host, rather than on a serial console.
//!
//! What is measured is exactly what the read loop does after decryption:
//! [`NodeStream::from_packed`] over the compressed payload, the root peek the
//! read loop makes to find the waiter, and [`PropsSpec::consume_response`]
//! filtering to the watched codes. The compressed frame itself stays alive
//! throughout, as it does on the read loop.
//!
//! The budget is derived from the numbers the C3 reported, not from what the
//! code happens to allocate today:
//!
//! - free heap with the frame already in it: 58,580 B;
//! - the zlib-rs inflate state: one allocation of ~47.5 KB, the largest thing
//!   the path allocates and the one thing that cannot shrink (state plus a
//!   32 KB LZ77 window the format requires);
//! - the inflate window of the stream: 4 KB;
//! - the child being decoded, and the retained props: well under 1 KB.
//!
//! So the path must fit in the 58 KB with the inflate state built from
//! nothing, and in a few KB when the state is already parked in the thread's
//! pool (see `zlib_pool::warm_pool`), which is what the firmware does on the
//! executor thread before connecting. Both are asserted, with the state's
//! size measured rather than assumed.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use wacore::iq::abprops::{ALL, AbDefault};
use wacore::iq::props::PropsSpec;
use wacore::iq::spec::{IqSpec, IqStreamSpec};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal::marshal;
use wacore_binary::util::FORMAT_COMPRESSED;
use wacore_binary::{NodeStream, OwnedNodeRef, zlib_pool};

/// Live heap, its high-water mark, and the largest single request seen since
/// the last reset. Relaxed: the tests are single-threaded.
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static LARGEST: AtomicUsize = AtomicUsize::new(0);

struct Counting;

// SAFETY: every method forwards to `System` with the layout it received and
// only adds counter updates, so the allocator contract is exactly `System`'s.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            grow(layout.size());
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            // A realloc is a new block of `new_size` for the purpose of the
            // largest-request check: an allocator without in-place growth has
            // to find a hole that big.
            LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
            grow(new_size);
        }
        new_ptr
    }
}

fn grow(size: usize) {
    let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(live, Ordering::Relaxed);
    LARGEST.fetch_max(size, Ordering::Relaxed);
}

#[global_allocator]
static ALLOC: Counting = Counting;

/// The counters are process-wide and `cargo test` runs tests on threads, so a
/// measurement holds this for its whole test.
static MEASURING: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Peak live bytes above the baseline, and the largest single request, over
/// `run`.
fn measure(run: impl FnOnce()) -> (usize, usize) {
    let base = LIVE.load(Ordering::Relaxed);
    PEAK.store(base, Ordering::Relaxed);
    LARGEST.store(0, Ordering::Relaxed);
    run();
    (
        PEAK.load(Ordering::Relaxed) - base,
        LARGEST.load(Ordering::Relaxed),
    )
}

/// Free heap the C3 reported with the frame already allocated.
const C3_FREE_WITH_FRAME: usize = 58_580;
/// What the stream may cost once the inflate state is parked: its 4 KB window,
/// one child, the retained props, and slack for allocator rounding.
const STREAM_BUDGET_WARM: usize = 8 * 1024;
/// The compressed frame the C3 saw was 28,204 bytes on the wire.
const TARGET_COMPRESSED: usize = 28_000;

/// A full props response with the real catalog's codes and values shaped like
/// the server's: booleans, integers, and strings long enough that the frame
/// compresses to the size the C3 saw rather than to the few KB a catalog of
/// defaults would.
fn props_frame() -> (Vec<u8>, usize, usize) {
    let mut seed: u64 = 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut children = Vec::new();
    for group in ALL {
        for prop in group.iter() {
            let value = match prop.default {
                AbDefault::Bool(b) => (if b { "true" } else { "false" }).to_string(),
                AbDefault::Int(_) => (next() % 1_000_000).to_string(),
                AbDefault::Float(_) => format!("{:.3}", (next() % 10_000) as f64 / 100.0),
                // Server-side strings are ids, JSON and lists. The catalog
                // has 177 of them; an opaque 160-character token each is what
                // brings the frame to the size the C3 saw.
                AbDefault::Str(_) => (0..10).map(|_| format!("{:016x}", next())).collect(),
            };
            children.push(
                NodeBuilder::new("prop")
                    .attr("config_code", prop.code)
                    .attr("config_value", value)
                    .build(),
            );
        }
    }
    let count = children.len();
    let props = NodeBuilder::new("props")
        .attr("protocol", "1")
        .attr("ab_key", "0123456789abcdef0123456789abcdef")
        .attr("hash", "fedcba9876543210fedcba9876543210")
        .attr("refresh", 86_400u32)
        .attr("refresh_id", 1u32)
        .attr("delta_update", "false")
        .children(children)
        .build();
    let iq = NodeBuilder::new("iq")
        .attr("from", "s.whatsapp.net")
        .attr("type", "result")
        .attr("id", "1234.5678-9")
        .children(vec![props])
        .build();
    let packed = marshal(&iq).expect("marshal");

    use std::io::Write;
    let mut encoder =
        flate2::write::ZlibEncoder::new(vec![FORMAT_COMPRESSED], flate2::Compression::default());
    encoder.write_all(&packed[1..]).expect("compress");
    let frame = encoder.finish().expect("compress");
    (frame, packed.len() - 1, count)
}

fn watched() -> Vec<u32> {
    wacore::iq::props::WATCHED.iter().map(|p| p.code).collect()
}

/// The watched codes the fixture can carry: `WATCHED` also lists flags the
/// current bundle no longer ships (`props::stale`), which the catalog the
/// fixture is built from does not have.
fn watched_in_catalog() -> usize {
    let catalog: std::collections::HashSet<u32> =
        ALL.iter().flat_map(|g| g.iter()).map(|p| p.code).collect();
    watched().iter().filter(|c| catalog.contains(c)).count()
}

/// What the read loop does with the frame, from the decrypted payload on.
fn receive_streamed(frame: &[u8], spec: &PropsSpec) -> wacore::iq::props::PropsResponse {
    let mut stream = NodeStream::from_packed(frame).expect("packed");
    let root = stream.open().expect("root").expect("root");
    assert_eq!(root.tag, "iq");
    let response = spec.consume_response(&mut stream).expect("consume");
    stream.finish().expect("finish");
    response
}

#[test]
fn a_full_props_response_streams_within_the_c3_budget() {
    let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let (frame, decompressed, count) = props_frame();
    assert!(
        (TARGET_COMPRESSED..TARGET_COMPRESSED + 6_000).contains(&frame.len()),
        "fixture drifted: {} compressed bytes for {count} props ({decompressed} decompressed); \
         the budget below is only meaningful for a frame the size the C3 saw",
        frame.len()
    );
    let spec = PropsSpec::new().retaining(watched());
    let expected_kept = watched_in_catalog();

    // The inflate state on its own, as the pool builds it.
    zlib_pool::drain_pool();
    let (inflate_state, inflate_block) = measure(zlib_pool::warm_pool);

    // Warm: the state is parked (the firmware warms it on the executor thread
    // before connecting), so the frame costs the stream's window and the
    // children it decodes. This is the case the C3 has to fit: with the frame
    // in memory it had 58,580 B, and the state alone is 47.5 KB of that, so a
    // state built at that moment never fits next to the frame whatever the
    // rest of the path costs.
    let (warm_peak, warm_largest) = measure(|| {
        let response = receive_streamed(&frame, &spec);
        assert_eq!(response.experiment_props.len(), expected_kept);
    });
    eprintln!(
        "props frame: {} B compressed, {decompressed} B decompressed, {count} props; \
         inflate state {inflate_state} B (one block of {inflate_block} B); \
         warm peak {warm_peak} B (largest request {warm_largest} B)",
        frame.len()
    );
    assert!(
        warm_peak + frame.len() < C3_FREE_WITH_FRAME,
        "warm receive needs {warm_peak} B on top of the {} B frame; the C3 had {C3_FREE_WITH_FRAME} B",
        frame.len()
    );
    assert!(
        warm_peak <= STREAM_BUDGET_WARM,
        "with the inflate state parked the stream cost {warm_peak} B, budget {STREAM_BUDGET_WARM} B"
    );
    assert!(
        warm_largest <= 4 * 1024 + 64,
        "with the inflate state parked no request may exceed the 4 KB inflate window, saw {warm_largest} B"
    );

    // Cold: nothing parked, so the stream builds the state. It may cost the
    // state and nothing else beyond the warm path, and the state must remain
    // the largest single request: a second block that size would be a second
    // window.
    zlib_pool::drain_pool();
    let (cold_peak, cold_largest) = measure(|| {
        let response = receive_streamed(&frame, &spec);
        assert_eq!(response.experiment_props.len(), expected_kept);
    });
    eprintln!("cold peak {cold_peak} B (largest request {cold_largest} B)");
    assert!(
        cold_peak <= inflate_state + STREAM_BUDGET_WARM,
        "cold receive cost {cold_peak} B: more than the inflate state ({inflate_state} B) plus the warm budget"
    );
    assert_eq!(
        cold_largest, inflate_block,
        "the largest single request must be the inflate state itself, nothing the stream adds"
    );
}

/// The streamed reading and the tree reading agree on this fixture, and the
/// tree reading is the one the C3 could not afford: recorded here so the gap
/// is a number in the test log, not a claim in a commit message.
#[test]
fn the_tree_decode_of_the_same_frame_costs_what_the_c3_cannot_pay() {
    let _measuring = MEASURING.lock().unwrap_or_else(|e| e.into_inner());
    let (frame, decompressed, _) = props_frame();
    let spec = PropsSpec::new().retaining(watched());
    let streamed = receive_streamed(&frame, &spec);

    zlib_pool::warm_pool();
    let mut parsed = None;
    let (tree_peak, tree_largest) = measure(|| {
        let node_bytes = wacore_binary::util::unpack(&frame)
            .expect("unpack")
            .into_owned();
        let owned = OwnedNodeRef::new(node_bytes).expect("decode");
        parsed = Some(spec.parse_response(owned.get()).expect("parse"));
    });
    eprintln!(
        "tree decode of the same frame: peak {tree_peak} B, largest request {tree_largest} B \
         ({decompressed} B decompressed)"
    );
    let parsed = parsed.expect("parsed");
    assert_eq!(parsed.experiment_props, streamed.experiment_props);
    assert_eq!(parsed.hash, streamed.hash);
    assert!(
        tree_peak > C3_FREE_WITH_FRAME,
        "the tree decode fits the C3 after all ({tree_peak} B); the streaming path may be redundant"
    );
}
