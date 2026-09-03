//! `marshal_shallow` is what `Client::marshal_node_for_send` puts on the wire,
//! so two things have to hold for every shape a send produces: it must agree
//! with `marshal_exact` byte for byte, and its reserve must cover the encoding
//! on the first try — one allocation per call is the whole point of estimating
//! instead of planning.
//!
//! The allocation half shares this binary with the equality half because the
//! counting allocator is process-global; a min-delta over many windows is what
//! keeps a sibling test's allocations out of the measurement.

// Host-only allocation-count harness; std's 64-bit atomic is fine (never built
// for embedded targets).
#![allow(clippy::disallowed_types)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicU64, Ordering};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Jid;
use wacore_binary::marshal::{marshal_exact, marshal_shallow};
use wacore_binary::node::Node;

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

/// A device JID per index, spread over users the way a real fan-out resolves:
/// a device id past 255 would silently leave the `AD_JID` encoding and measure
/// a second wire shape. Numbers are reserved-for-fiction NANP.
fn device_jid(index: usize) -> Jid {
    const DEVICES_PER_USER: usize = 4;
    let user = 19045550180u64 + (index / DEVICES_PER_USER) as u64;
    Jid::pn_device(user.to_string(), (index % DEVICES_PER_USER) as u16)
}

fn dm_message() -> Node {
    NodeBuilder::new("message")
        .attr("to", "19045550181@s.whatsapp.net")
        .attr("id", "3EB0A1B2C3D4E5F60718")
        .attr("type", "text")
        .attr("t", "1760000000")
        .attr("edit", "1")
        .attr("category", "peer")
        .children(vec![
            NodeBuilder::new("enc")
                .attr("v", "2")
                .attr("type", "msg")
                .bytes(vec![0xAB; 256])
                .build(),
        ])
        .build()
}

fn group_message() -> Node {
    NodeBuilder::new("message")
        .attr("to", "120363000000000001@g.us")
        .attr("id", "3EB0A1B2C3D4E5F60719")
        .attr("type", "text")
        .attr("t", "1760000000")
        .children(vec![
            NodeBuilder::new("enc")
                .attr("v", "2")
                .attr("type", "skmsg")
                .bytes(vec![0xCD; 256])
                .build(),
        ])
        .build()
}

fn receipt() -> Node {
    NodeBuilder::new("receipt")
        .attr("to", "19045550182@s.whatsapp.net")
        .attr("id", "3EB0A9252A8F12B7E2")
        .attr("type", "read")
        .attr("t", "1760000000")
        .build()
}

fn ack_shaped() -> Node {
    NodeBuilder::new("ack")
        .attr("to", "100000000000002@lid")
        .attr("id", "3EB0A9252A8F12B7E3")
        .attr("class", "message")
        .build()
}

/// The per-device fan-out a first send emits: a `<to>` per recipient device,
/// each wrapping its own `<enc>`, with string-typed JIDs.
fn fanout(width: usize) -> Node {
    let recipients: Vec<Node> = (0..width)
        .map(|i| {
            NodeBuilder::new("to")
                .attr("jid", device_jid(i).to_string())
                .children(vec![
                    NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "pkmsg")
                        .bytes(vec![0xEF; 128])
                        .build(),
                ])
                .build()
        })
        .collect();
    NodeBuilder::new("message")
        .attr("to", "19045550183@s.whatsapp.net")
        .attr("id", "3EB0A1B2C3D4E5F6071A")
        .attr("type", "text")
        .children(recipients)
        .build()
}

/// The sender-key distribution shape: the same fan-out under a
/// `<participants>` wrapper, with typed [`Jid`] attributes as
/// `build_participant_node` passes them.
fn skdm_fanout(width: usize) -> Node {
    let recipients: Vec<Node> = (0..width)
        .map(|i| {
            NodeBuilder::new("to")
                .attr("jid", device_jid(i))
                .children(vec![
                    NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "msg")
                        .bytes(vec![0xAB; 128])
                        .build(),
                ])
                .build()
        })
        .collect();
    NodeBuilder::new("message")
        .attr("to", "120363000000000001@g.us")
        .attr("id", "3EB0A1B2C3D4E5F6071B")
        .attr("type", "text")
        .children(vec![
            NodeBuilder::new("participants")
                .children(recipients)
                .build(),
        ])
        .build()
}

fn send_shapes() -> Vec<(&'static str, Node)> {
    vec![
        ("dm", dm_message()),
        ("group", group_message()),
        ("receipt", receipt()),
        ("ack", ack_shaped()),
        ("fanout 8", fanout(8)),
        ("fanout 64", fanout(64)),
        ("skdm fanout 8", skdm_fanout(8)),
        ("skdm fanout 512", skdm_fanout(512)),
    ]
}

#[test]
fn shallow_encodes_every_send_shape_exactly_as_the_exact_path_did() {
    for (name, node) in send_shapes() {
        let exact = marshal_exact(&node).expect("marshal_exact");
        let shallow = marshal_shallow(&node).expect("marshal_shallow");
        assert_eq!(shallow, exact, "{name}: encoding must not change");
    }
}

#[test]
fn shallow_reserves_enough_to_allocate_once() {
    for (name, node) in send_shapes() {
        // Min-delta over many windows: the counter is process-global and
        // harness threads bleed sporadic allocations, but if the reserve
        // really covers the encoding, at least one window lands on exactly
        // the output buffer.
        let mut min_delta = u64::MAX;
        for _ in 0..64 {
            let before = ALLOCS.load(Ordering::Relaxed);
            let payload = marshal_shallow(&node).expect("marshal_shallow");
            let after = ALLOCS.load(Ordering::Relaxed);
            assert!(!payload.is_empty());
            min_delta = min_delta.min(after - before);
        }
        assert_eq!(
            min_delta, 1,
            "{name}: the shallow reserve must cover the encoding, so the output \
             buffer is the only allocation"
        );
    }
}
