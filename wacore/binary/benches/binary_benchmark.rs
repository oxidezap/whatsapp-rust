use divan::black_box;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use std::io::Write;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal::{
    marshal, marshal_auto, marshal_exact, marshal_ref, marshal_ref_auto, marshal_ref_exact,
    marshal_to, unmarshal_ref,
};
use wacore_binary::node::Node;
use wacore_binary::util::unpack;

fn main() {
    divan::main();
}

fn create_small_node() -> Node {
    NodeBuilder::new("message")
        .attr("to", "user@s.whatsapp.net")
        .attr("id", "12345")
        .attr("type", "text")
        .build()
}

fn create_large_node() -> Node {
    NodeBuilder::new("iq")
        .attr("to", "server@s.whatsapp.net")
        .attr("id", "abcdef")
        .attr("type", "get")
        .attr("xmlns", "usync")
        .children(vec![
            NodeBuilder::new("usync")
                .attr("sid", "message:1")
                .attr("mode", "query")
                .attr("last", "true")
                .children(vec![
                    NodeBuilder::new("query")
                        .children(vec![NodeBuilder::new("business").build()])
                        .build(),
                ])
                .build(),
            NodeBuilder::new("list")
                .children((0..20).map(|i| {
                    NodeBuilder::new("item")
                        .attr("index", i.to_string())
                        .bytes(vec![i as u8; 32])
                        .build()
                }))
                .build(),
        ])
        .build()
}

fn create_attr_node() -> Node {
    NodeBuilder::new("iq")
        .attr("xmlns", "test:ns")
        .attr("type", "result")
        .attr("id", "abc123")
        .attr("from", "server@s.whatsapp.net")
        .attr("has_flag", "true")
        .attr("timestamp", "1700000000")
        .build()
}

// Creates a node with long string content to test the JID parsing optimization.
// Long strings (> 48 chars) should skip JID parsing for better performance.
fn create_long_string_node() -> Node {
    // Generate a 500+ character string that contains '@' but is NOT a valid JID.
    // Without the optimization, parse_jid would scan the entire string.
    let base_pattern = "Lorem ipsum with email user@example.com in text. ";
    let long_text = base_pattern.repeat(11); // ~550 characters

    NodeBuilder::new("message")
        .attr("to", "1234567890@s.whatsapp.net")
        .attr("id", "ABC123DEF456")
        .attr("type", "text")
        .string_content(long_text)
        .build()
}

/// Creates a node structure that simulates a usync response with many children
/// for testing child iteration performance.
fn create_usync_like_node() -> Node {
    NodeBuilder::new("iq")
        .attr("type", "result")
        .children(vec![
            NodeBuilder::new("usync")
                .children(vec![
                    NodeBuilder::new("list")
                        .children((0..50).map(|i| {
                            NodeBuilder::new("user")
                                .attr("jid", format!("{}@s.whatsapp.net", i))
                                .children(vec![
                                    NodeBuilder::new("devices")
                                        .children(vec![
                                            NodeBuilder::new("device-list")
                                                .children((0..3).map(|d| {
                                                    NodeBuilder::new("device")
                                                        .attr("id", d.to_string())
                                                        .build()
                                                }))
                                                .build(),
                                        ])
                                        .build(),
                                    NodeBuilder::new("contact").attr("type", "in").build(),
                                ])
                                .build()
                        }))
                        .build(),
                ])
                .build(),
        ])
        .build()
}
// Creates a node with multiple JID attributes to benchmark JID handling.
// When marshaled, these strings are encoded as JID tokens (JID_PAIR or AD_JID).
// The optimization avoids stringify→parse roundtrip when converting NodeRef→Node.
fn create_jid_heavy_node() -> Node {
    NodeBuilder::new("message")
        .attr("from", "15551234567@s.whatsapp.net")
        .attr("to", "15559876543@s.whatsapp.net")
        .attr("participant", "15555555555@s.whatsapp.net")
        .attr("recipient", "15556666666@s.whatsapp.net")
        .attr("notify", "15557777777@s.whatsapp.net")
        .attr("id", "ABCDEF123456")
        .attr("type", "text")
        .build()
}

fn create_huge_bytes_node() -> Node {
    NodeBuilder::new("message")
        .attr("to", "server@s.whatsapp.net")
        .attr("id", "huge-binary")
        .bytes(vec![0x5A; 256 * 1024])
        .build()
}

fn create_many_children_node() -> Node {
    NodeBuilder::new("iq")
        .attr("to", "server@s.whatsapp.net")
        .attr("id", "many-children")
        .children((0..2048).map(|i| {
            NodeBuilder::new("item")
                .attr("index", i.to_string())
                .attr("type", "entry")
                .string_content("ok")
                .build()
        }))
        .build()
}

// Marshal benchmarks. Inputs are pre-built via with_inputs so the reported
// cost is the encoder alone, not node construction. `marshal_auto` is the
// production send-path strategy, so it gets one bench per payload shape; the
// plain/exact strategies are tracked on a single shape for comparison.
#[divan::bench]
fn bench_marshal_auto_small(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_small_node)
        .bench_refs(|node| black_box(marshal_auto(black_box(node)).unwrap()));
}

#[divan::bench]
fn bench_marshal_auto_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_large_node)
        .bench_refs(|node| black_box(marshal_auto(black_box(node)).unwrap()));
}

#[divan::bench]
fn bench_marshal_auto_long_string(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_long_string_node)
        .bench_refs(|node| black_box(marshal_auto(black_box(node)).unwrap()));
}

#[divan::bench]
fn bench_marshal_auto_huge_bytes(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_huge_bytes_node)
        .bench_refs(|node| black_box(marshal_auto(black_box(node)).unwrap()));
}

#[divan::bench]
fn bench_marshal_auto_many_children(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_many_children_node)
        .bench_refs(|node| black_box(marshal_auto(black_box(node)).unwrap()));
}

// The exact (plan + hint replay) strategy is the production send path for
// message plaintext, so its worst case — many children, JID-heavy — gets its
// own pin.
#[divan::bench]
fn bench_marshal_exact_many_children(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_many_children_node)
        .bench_refs(|node| black_box(marshal_exact(black_box(node)).unwrap()));
}

// Strategy comparison on one shape.
#[divan::bench]
fn bench_marshal_plain_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_large_node)
        .bench_refs(|node| black_box(marshal(black_box(node)).unwrap()));
}

#[divan::bench]
fn bench_marshal_exact_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_large_node)
        .bench_refs(|node| black_box(marshal_exact(black_box(node)).unwrap()));
}

#[divan::bench]
fn bench_marshal_to_reused_buffer_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| (create_large_node(), Vec::with_capacity(4096)))
        .bench_refs(|(node, buffer)| {
            marshal_to(black_box(node), buffer).unwrap();
            // Contents, not length: keeps the marshal writes observable.
            black_box(buffer.as_slice());
        });
}

// Setup functions for unmarshal benchmarks - pre-compute marshaled data
// Note: marshal() adds a flag byte at position 0, unmarshal_ref expects data without it
fn setup_small_marshaled() -> Vec<u8> {
    marshal(&create_small_node()).unwrap()
}

fn setup_large_marshaled() -> Vec<u8> {
    marshal(&create_large_node()).unwrap()
}

#[divan::bench]
fn bench_unmarshal_small(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_small_marshaled)
        .bench_refs(|marshaled| {
            black_box(unmarshal_ref(black_box(&marshaled[1..])).unwrap());
        });
}

#[divan::bench]
fn bench_unmarshal_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_large_marshaled)
        .bench_refs(|marshaled| {
            black_box(unmarshal_ref(black_box(&marshaled[1..])).unwrap());
        });
}

// Unpack benchmarks: payloads are pre-built in setup so the compressed case
// measures the inflate path, not the deflate used to build the fixture. The
// compressed body is a realistic multi-KB frame (the marshaled usync-like
// node), matching what the server actually compresses.
fn setup_uncompressed_payload() -> Vec<u8> {
    let mut payload = vec![0u8];
    payload.extend_from_slice(&marshal(&create_large_node()).unwrap()[1..]);
    payload
}

fn setup_compressed_payload() -> Vec<u8> {
    let body = marshal(&create_usync_like_node()).unwrap();
    let mut payload = vec![2u8];
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&body[1..]).unwrap();
    payload.extend_from_slice(&encoder.finish().unwrap());
    payload
}

#[divan::bench]
fn bench_unpack_uncompressed(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_uncompressed_payload)
        .bench_refs(|payload| {
            black_box(unpack(black_box(payload)).unwrap());
        });
}

#[divan::bench]
fn bench_unpack_compressed(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_compressed_payload)
        .bench_refs(|payload| {
            black_box(unpack(black_box(payload)).unwrap());
        });
}

// Setup function for attr_parser benchmark - pre-compute marshaled data
fn setup_attr_marshaled() -> Vec<u8> {
    marshal(&create_attr_node()).unwrap()
}

// Measures decode + attr access together: a NodeRef borrows its wire buffer,
// so the parse cannot be moved into setup without owning the node.
#[divan::bench]
fn bench_attr_parser(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_attr_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(&marshaled[1..]).unwrap();

            let mut parser = node_ref.attrs();
            black_box(parser.optional_string("xmlns"));
            black_box(parser.optional_string("type"));
            black_box(parser.optional_jid("from"));
            black_box(parser.optional_bool("has_flag"));
            black_box(parser.optional_u64("timestamp"));
            black_box(parser.finish().is_ok());
        });
}

// Round-trip benchmark: unmarshal to NodeRef and re-marshal using the borrowed path.
// This tests the zero-copy encoding path with EncodeNode trait.
#[divan::bench]
fn bench_roundtrip_small(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_small_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(black_box(&marshaled[1..])).unwrap();
            black_box(marshal_ref(&node_ref).unwrap())
        });
}

#[divan::bench]
fn bench_roundtrip_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_large_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(black_box(&marshaled[1..])).unwrap();
            black_box(marshal_ref(&node_ref).unwrap())
        });
}

#[divan::bench]
fn bench_roundtrip_auto_small(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_small_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(black_box(&marshaled[1..])).unwrap();
            black_box(marshal_ref_auto(&node_ref).unwrap())
        });
}

#[divan::bench]
fn bench_roundtrip_auto_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_large_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(black_box(&marshaled[1..])).unwrap();
            black_box(marshal_ref_auto(&node_ref).unwrap())
        });
}

#[divan::bench]
fn bench_roundtrip_exact_small(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_small_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(black_box(&marshaled[1..])).unwrap();
            black_box(marshal_ref_exact(&node_ref).unwrap())
        });
}

#[divan::bench]
fn bench_roundtrip_exact_large(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_large_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(black_box(&marshaled[1..])).unwrap();
            black_box(marshal_ref_exact(&node_ref).unwrap())
        });
}

// Child iteration benchmark: tests get_children_by_tag performance over the
// recursive traversal pattern used in usync parsing. The tree is pre-built.
#[divan::bench]
fn bench_get_children_by_tag(bencher: divan::Bencher) {
    bencher
        .with_inputs(create_usync_like_node)
        .bench_refs(|node| {
            let usync = node.get_optional_child("usync").unwrap();
            let list = usync.get_optional_child("list").unwrap();

            let mut count = 0;
            for user in black_box(list.get_children_by_tag("user")) {
                if let Some(devices) = user.get_optional_child("devices")
                    && let Some(device_list) = devices.get_optional_child("device-list")
                {
                    for _device in black_box(device_list.get_children_by_tag("device")) {
                        count += 1;
                    }
                }
            }
            black_box(count);
        });
}

// Setup function for JID optimization benchmark - pre-compute marshaled JID-heavy data
fn setup_jid_heavy_marshaled() -> Vec<u8> {
    marshal(&create_jid_heavy_node()).unwrap()
}

// Benchmark that measures the JID attribute optimization.
// This tests the flow: unmarshal → to_owned() → access JIDs via AttrParser
//
// The optimization benefit:
// - Before: JID decoded → stringified in to_owned() → re-parsed in optional_jid()
// - After: JID decoded → preserved as Jid in to_owned() → cloned in optional_jid()
#[divan::bench]
fn bench_jid_to_owned_access(bencher: divan::Bencher) {
    bencher
        .with_inputs(setup_jid_heavy_marshaled)
        .bench_refs(|marshaled| {
            // Skip the flag byte at position 0
            let node_ref = unmarshal_ref(&marshaled[1..]).unwrap();

            // Convert to owned Node - this is where JIDs are preserved (optimization)
            let node = node_ref.to_owned();

            // Access JID attributes via AttrParser - if JIDs are preserved, no parsing needed
            let mut parser = node.attrs();
            black_box(parser.optional_jid("from"));
            black_box(parser.optional_jid("to"));
            black_box(parser.optional_jid("participant"));
            black_box(parser.optional_jid("recipient"));
            black_box(parser.optional_jid("notify"));
            black_box(parser.optional_string("id"));
            black_box(parser.optional_string("type"));
            node
        });
}
