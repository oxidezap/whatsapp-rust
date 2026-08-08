//! What the per-message identity path costs in heap allocations, and where the
//! threshold that decides it actually sits.
//!
//! `CompactString` keeps a string inline while it fits in `size_of::<String>()`
//! bytes. That is 24 on a 64-bit host but 12 on the wasm32 and 32-bit builds
//! this crate also targets, and most of what the wire carries lands between the
//! two: a 13-digit phone user, a 15-digit LID, an 18-character group id, a
//! 20-character stanza id. The same code therefore allocates for those on wasm
//! and not on a desktop, so an allocation profile taken on one says nothing
//! about the other. A legacy `<creator>-<timestamp>` group id clears neither
//! budget, and is here so both sides of the wide one are covered.
//!
//! Both halves are pinned here. The identities stay structured (a `Jid` is
//! never rendered to text just to be compared or used as a key, and
//! `rendering_costs_an_allocation_each` measures what doing so would cost), and
//! every expected count is derived from `size_of::<String>()` rather than
//! hardcoded, so the assertions stay true on whichever target runs them.
//!
//! CI only runs this on the 64-bit host: the wasm job builds the library and
//! does not execute integration tests, and there is no 32-bit runner. So the
//! 12-byte budget is never measured here, only derived. What is checked on
//! every run is that the fixtures still straddle it, since a mix that drifted
//! to all-short values would keep passing while measuring nothing that matters
//! to the target the question came from.
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
use wacore_binary::node::{NodeStr, OwnedNodeRef, ValueRef};

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

/// The budget on a 32-bit target, which no runner here has. Used to keep the
/// fixtures honest rather than to measure anything.
const SMALL_TARGET_BUDGET: usize = 12;

/// The mix a live stream carries, since it is the *length* of each identity
/// that decides inline against heap: user JIDs with and without a device, both
/// namespaces, a modern group id, both legacy `<creator>-<timestamp>` forms
/// that bracket the 64-bit budget, a newsletter and the status address.
/// Numbers are fictitious.
///
/// The 25-byte legacy id is what keeps the derived-address assertions honest:
/// without an identity past the wide budget they would all expect zero on the
/// only host CI runs, and the one-allocation branch would go unexercised.
const TRAFFIC: &[&str] = &[
    "5511987650001@s.whatsapp.net",
    "5511987650002:12@s.whatsapp.net",
    "5511987650003:99@s.whatsapp.net",
    "100000000000001@lid",
    "100000000000002:7@lid",
    "120363000000000001@g.us",
    "5511987650001-1700000000@g.us",
    "55119876500011-1700000000@g.us",
    "120000000000000001@newsletter",
    "status@broadcast",
];

/// The attributes `stanza` carries, in the order `value_shapes` reports them.
const ATTRS: &[&str] = &["from", "participant", "recipient", "id", "t"];

/// The values in a `<message>` the decoder cannot borrow out of the frame,
/// because the wire packs two characters per byte: a group id, a phone user, a
/// legacy group id, a stanza id and a timestamp. Each is paired with a short
/// spelling of itself, which packs the same way and fits inline everywhere, so
/// the two stanzas below differ in length and in nothing else.
///
/// The legacy group id is 25 bytes, one past the 64-bit budget. Without a value
/// on that side of it the subtraction below would expect zero on the only host
/// CI has, and would pass whether or not the allocating branch still works.
///
/// None of the short spellings is a bare one or two digit run, because the
/// encoder checks the token dictionary before it packs and those are tokens:
/// they would come back borrowed and put the control on a different decoder
/// route. `value_shapes` is what holds them to that.
const MATERIALIZED: &[(&str, &str)] = &[
    ("120363000000000001", "9876"),
    ("5511987650002", "5511"),
    ("55119876500011-1700000000", "1-2"),
    ("3EB0A1B2C3D4E5F60718", "3EB0"),
    ("1770000000", "97531"),
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

/// A `<message>` built from either the long or the short spelling of each
/// value. Both go through the same decoder branches, so subtracting one from
/// the other leaves exactly the materialised strings that did not fit inline.
fn stanza(long: bool) -> bytes::Bytes {
    let pick = |i: usize| {
        if long {
            MATERIALIZED[i].0
        } else {
            MATERIALIZED[i].1
        }
    };
    let node = NodeBuilder::new("message")
        .attr("from", Jid::group(pick(0)))
        .attr("participant", Jid::pn_device(pick(1), 12))
        .attr("recipient", Jid::group(pick(2)))
        .attr("id", pick(3))
        .attr("t", pick(4))
        .build();
    let mut raw = marshal(&node).expect("marshal");
    // `unmarshal_ref` does not expect the leading format byte `marshal` writes.
    raw.remove(0);
    bytes::Bytes::from(raw)
}

/// How each attribute value came back: which `ValueRef` branch, and whether the
/// decoder had to materialise it or could borrow it out of the frame. Both are
/// wire-encoding facts the subtraction in
/// `decoding_a_stanza_allocates_only_what_it_cannot_borrow` rests on, so they
/// are read back and asserted rather than assumed.
fn value_shapes(bytes: &bytes::Bytes) -> Vec<(&'static str, &'static str)> {
    let decoded = OwnedNodeRef::new(bytes.clone()).expect("decode");
    ATTRS
        .iter()
        .map(|key| {
            let (branch, text) = match decoded.get_attr(key).expect("attr") {
                ValueRef::Jid(jid) => ("jid", &jid.user),
                ValueRef::String(value) => ("string", value),
            };
            let storage = match text {
                NodeStr::Borrowed(_) => "borrowed",
                NodeStr::Owned(_) => "materialised",
            };
            (branch, storage)
        })
        .collect()
}

/// One test fn covering all of them, because the counting allocator is
/// process-global and sibling tests would run against it concurrently.
#[test]
fn per_message_identities_stay_off_the_heap() {
    the_fixtures_straddle_both_budgets();
    identities_parse_derive_and_render_unchanged();
    rendering_costs_an_allocation_each();
    decoding_a_stanza_allocates_only_what_it_cannot_borrow();
    the_inline_boundary_sits_exactly_at_size_of_string();
}

/// The fixtures have to contain values on both sides of the 12-byte budget, or
/// the counts this file derives would be trivially zero on the target that
/// motivated it and nobody would notice.
fn the_fixtures_straddle_both_budgets() {
    if inline_capacity() <= SMALL_TARGET_BUDGET {
        // Running on the narrow target itself. Everything below describes what
        // a wide host should see of it; the measurements do the rest, and they
        // derive their own counts, so there is nothing here to check.
        return;
    }

    let crossing = TRAFFIC
        .iter()
        .filter(|raw| {
            let len = user_of(raw).len();
            len > SMALL_TARGET_BUDGET && len <= inline_capacity()
        })
        .count();
    assert!(
        crossing >= 5,
        "the traffic mix must keep identities that are inline at \
         {} bytes and heap at {SMALL_TARGET_BUDGET}, found {crossing}",
        inline_capacity()
    );

    assert!(
        TRAFFIC
            .iter()
            .any(|raw| user_of(raw).len() > inline_capacity()),
        "one identity must be past this host's budget too, or every derived \
         address would expect zero and never allocate"
    );

    let materialized_crossing = MATERIALIZED
        .iter()
        .filter(|(value, _)| value.len() > SMALL_TARGET_BUDGET)
        .count();
    assert_eq!(
        materialized_crossing, 4,
        "the stanza must keep four values a 32-bit decode would allocate"
    );
    assert!(
        MATERIALIZED
            .iter()
            .any(|(value, _)| value.len() > inline_capacity()),
        "one stanza value must be past this host's budget too, or the decode \
         subtraction would expect zero and never exercise the allocating branch"
    );
    assert!(
        MATERIALIZED
            .iter()
            .all(|(_, short)| short.len() <= SMALL_TARGET_BUDGET),
        "the short spelling of every value must fit inline on every target"
    );
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
    let long = stanza(true);
    let short = stanza(false);

    let decoded = OwnedNodeRef::new(long.clone()).expect("decode");
    assert!(decoded.get_attr("from").unwrap() == "120363000000000001@g.us");
    assert!(decoded.get_attr("id").unwrap() == MATERIALIZED[3].0);
    // Attribute values go through `read_value`, so a JID token stays a
    // `ValueRef::Jid` and only its packed user is materialised. Every value has
    // to be materialised on both stanzas, or the subtraction is measuring the
    // difference between two decoder routes instead of two lengths.
    let expected = [
        ("jid", "materialised"),
        ("jid", "materialised"),
        ("jid", "materialised"),
        ("string", "materialised"),
        ("string", "materialised"),
    ];
    assert_eq!(value_shapes(&long), expected);
    assert_eq!(value_shapes(&short), expected);

    let with = min_allocs(200, || OwnedNodeRef::new(long.clone()).unwrap());
    let without = min_allocs(200, || OwnedNodeRef::new(short.clone()).unwrap());
    let expected = MATERIALIZED
        .iter()
        .filter(|(value, _)| value.len() > inline_capacity())
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
