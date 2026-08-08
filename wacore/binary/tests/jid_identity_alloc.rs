//! What the per-message identity path costs in heap allocations, and where the
//! threshold that decides it actually sits.
//!
//! `CompactString` keeps a string inline while it fits in `size_of::<String>()`
//! bytes. That is 24 on a 64-bit host but 12 on the wasm32 and 32-bit builds
//! this crate also targets, and every identity the wire carries lands between
//! the two: a 13-digit phone user, a 15-digit LID, an 18-character group id, a
//! 20-character stanza id. The same code therefore allocates nothing per
//! message on a desktop and once per identity on wasm, so an allocation profile
//! taken on one says nothing about the other.
//!
//! Both halves are pinned here. The identities stay structured (a `Jid` is
//! never rendered to text just to be compared or used as a key, and
//! `rendering_costs_an_allocation_each` measures what doing so would cost), and
//! every expected count is derived from `size_of::<String>()` rather than
//! hardcoded, so the assertions stay true on whichever target runs them.
//!
//! Single test fn on purpose, as in `jid_non_ad_arc_alloc`: the counting
//! allocator is process-global, so a concurrently running sibling test would
//! bleed its allocations into the measurement.

// Host-only allocation-count harness; std's 64-bit atomic is fine (never built
// for embedded targets).
#![allow(clippy::disallowed_types)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Jid;
use wacore_binary::marshal::marshal;
use wacore_binary::node::OwnedNodeRef;

struct CountingAlloc;
static ALLOCS: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCS.fetch_add(1, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

/// Smallest allocation delta over many windows: the counter is process-global,
/// so a harness thread can bleed into any single window, but a genuine extra
/// allocation is paid on every iteration and its minimum can never drop.
fn min_allocs<T>(iterations: u32, mut op: impl FnMut() -> T) -> u64 {
    let mut min = u64::MAX;
    for _ in 0..iterations {
        let before = ALLOCS.load(Ordering::Relaxed);
        let value = std::hint::black_box(op());
        let after = ALLOCS.load(Ordering::Relaxed);
        drop(value);
        min = min.min(after - before);
    }
    min
}

/// The inline budget a `CompactString` has on the target running this test.
fn inline_capacity() -> usize {
    size_of::<String>()
}

/// The mix a live stream carries, since it is the *length* of each identity
/// that decides inline against heap: user JIDs with and without a device, both
/// namespaces, a modern group id, the legacy `<creator>-<timestamp>` form that
/// lands exactly on the 64-bit budget, a newsletter and the status address.
/// Numbers are fictitious.
const TRAFFIC: &[&str] = &[
    "5511987650001@s.whatsapp.net",
    "5511987650002:12@s.whatsapp.net",
    "5511987650003:99@s.whatsapp.net",
    "100000000000001@lid",
    "100000000000002:7@lid",
    "120363000000000001@g.us",
    "5511987650001-1700000000@g.us",
    "120000000000000001@newsletter",
    "status@broadcast",
];

/// Values the decoder cannot borrow out of the frame, because the wire packs
/// two characters per byte: a group id, a phone user, a stanza id and a
/// timestamp, which is the set every `<message>` carries.
const MATERIALIZED: &[&str] = &[
    "120363000000000001",
    "5511987650002",
    "3EB0A1B2C3D4E5F60718",
    "1770000000",
];

/// The user part of a rendered JID, which is the only piece a `Jid` owns.
fn user_of(rendered: &str) -> &str {
    let user = rendered.split('@').next().unwrap_or("");
    user.split(':').next().unwrap_or(user)
}

/// One allocation per identity whose user does not fit inline, none otherwise.
fn expected_per_user(rendered: &str) -> u64 {
    u64::from(user_of(rendered).len() > inline_capacity())
}

/// Two stanzas of the same shape: one whose values the decoder has to
/// materialise, one whose values it can borrow straight out of the frame. The
/// container allocations are identical, so the difference between them is
/// exactly the materialised strings that did not fit inline.
fn stanza(materialized: bool) -> bytes::Bytes {
    let node = if materialized {
        NodeBuilder::new("message")
            .attr("from", Jid::from_str("120363000000000001@g.us").unwrap())
            .attr(
                "participant",
                Jid::from_str("5511987650002:12@s.whatsapp.net").unwrap(),
            )
            .attr("id", MATERIALIZED[2])
            .attr("t", MATERIALIZED[3])
            .build()
    } else {
        NodeBuilder::new("message")
            .attr("from", "ab")
            .attr("participant", "cd")
            .attr("id", "ef")
            .attr("t", "gh")
            .build()
    };
    let mut raw = marshal(&node).expect("marshal");
    // `unmarshal_ref` does not expect the leading format byte `marshal` writes.
    raw.remove(0);
    bytes::Bytes::from(raw)
}

/// One test fn covering four properties, because the counting allocator is
/// process-global and sibling tests would run against it concurrently.
#[test]
fn per_message_identities_stay_off_the_heap() {
    identities_parse_derive_and_render_unchanged();
    rendering_costs_an_allocation_each();
    decoding_a_stanza_allocates_only_what_it_cannot_borrow();
    the_inline_boundary_sits_exactly_at_size_of_string();
}

fn identities_parse_derive_and_render_unchanged() {
    for &raw in TRAFFIC {
        // Rendering has to be byte-identical to what the wire and the logs
        // already carry: a cheaper representation that spelled a JID
        // differently would be a protocol change wearing an optimization's
        // clothes.
        let jid = Jid::from_str(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
        assert_eq!(jid.to_string(), raw, "{raw} did not render back unchanged");
        assert_eq!(
            Jid::from_str(&jid.to_string()).unwrap(),
            jid,
            "{raw} did not survive parse -> render -> parse"
        );

        let expected = expected_per_user(raw);
        assert_eq!(
            min_allocs(200, || Jid::from_str(raw).unwrap()),
            expected,
            "parsing {raw}"
        );

        // The three shapes the send path derives per recipient. Each carries
        // the user forward, so each costs what the user costs and nothing more.
        assert_eq!(min_allocs(200, || jid.clone()), expected, "clone {raw}");
        assert_eq!(
            min_allocs(200, || jid.with_device(21)),
            expected,
            "with_device {raw}"
        );
        assert_eq!(
            min_allocs(200, || jid.to_non_ad()),
            expected,
            "to_non_ad {raw}"
        );
    }
}

/// The price of the alternative the structured representation exists to avoid:
/// an identity turned into text to be compared or keyed allocates whatever its
/// length, because a rendered JID is past the inline budget on every target.
fn rendering_costs_an_allocation_each() {
    for &raw in TRAFFIC {
        let jid = Jid::from_str(raw).unwrap();
        assert_eq!(
            min_allocs(200, || jid.to_string()),
            1,
            "rendering {raw} to text"
        );
        assert_eq!(
            min_allocs(200, || jid.to_non_ad_string()),
            1,
            "rendering the non-AD form of {raw}"
        );
    }
}

fn decoding_a_stanza_allocates_only_what_it_cannot_borrow() {
    let materialized = stanza(true);
    let borrowed = stanza(false);

    let decoded = OwnedNodeRef::new(materialized.clone()).expect("decode");
    assert!(decoded.get_attr("from").unwrap() == "120363000000000001@g.us");
    assert!(decoded.get_attr("id").unwrap() == MATERIALIZED[2]);

    let with = min_allocs(200, || OwnedNodeRef::new(materialized.clone()).unwrap());
    let without = min_allocs(200, || OwnedNodeRef::new(borrowed.clone()).unwrap());
    let expected = MATERIALIZED
        .iter()
        .filter(|value| value.len() > inline_capacity())
        .count() as u64;

    assert_eq!(
        with - without,
        expected,
        "a decoded stanza should allocate only for materialised values past the \
         {}-byte inline budget",
        inline_capacity()
    );
}

/// The boundary itself, which is where an off-by-one would hide: a user of
/// exactly the inline budget stays off the heap, one byte more does not, and
/// both still render and round-trip identically.
fn the_inline_boundary_sits_exactly_at_size_of_string() {
    let cap = inline_capacity();
    let at = format!("{}@lid", "9".repeat(cap));
    let over = format!("{}@lid", "9".repeat(cap + 1));

    for raw in [at.as_str(), over.as_str()] {
        let jid = Jid::from_str(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
        assert_eq!(jid.to_string(), raw);
        assert_eq!(Jid::from_str(&jid.to_string()).unwrap(), jid);
    }

    assert_eq!(
        min_allocs(200, || Jid::from_str(&at).unwrap()),
        0,
        "a {cap}-byte user is the longest that still fits inline"
    );
    assert_eq!(
        min_allocs(200, || Jid::from_str(&over).unwrap()),
        1,
        "one byte past the budget has to reach the heap"
    );
}
