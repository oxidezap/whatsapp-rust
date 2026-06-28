//! Property test: `unmarshal(marshal(node)) == node` for randomly generated
//! `Node` trees.
//!
//! The encoder canonicalizes losslessly (token compression, nibble/hex packing,
//! JID packing), so not every syntactically-valid `Node` round-trips byte-for-byte
//! into the same `Node`. Two encoder behaviors are genuinely lossy at the *type*
//! level and the generator is constrained to avoid them rather than weakening the
//! assertion:
//!
//!  * `@`-bearing strings ≤48 bytes are parsed as JIDs (legacy dot-device, agent
//!    suffixes, unknown servers, ...), which is ambiguous/lossy — so no generated
//!    string contains `@`.
//!  * A `String` *content* that is not a protocol token and not nibble/hex-packable
//!    is emitted as raw bytes and decodes back as `NodeContent::Bytes`, not
//!    `String`. String content is therefore restricted to the nibble alphabet
//!    (`[0-9.-]`), which stays `String` on decode; arbitrary payloads are exercised
//!    through `NodeContent::Bytes`, which is always lossless.
//!
//! Attribute keys/values take the decoder's string path, which maps raw bytes back
//! to `String`, so they may be arbitrary (`@`-free) text and still round-trip.

use proptest::collection::{btree_map, vec};
use proptest::prelude::*;
use wacore_binary::marshal::{marshal, unmarshal_ref};
use wacore_binary::node::{Attrs, Node, NodeContent, NodeValue};

/// Bound recursion and collection sizes so the test is fast and the encoder's
/// recursive descent can't blow the stack.
const MAX_DEPTH: u32 = 4;
const MAX_CHILDREN: usize = 4;
const MAX_ATTRS: usize = 4;
const MAX_BYTES: usize = 64;

/// `@`-free string, kept short so we cover the token/packed/raw classification
/// paths without generating megabytes.
fn safe_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("[^@]{0,16}").expect("valid regex")
}

/// Non-empty nibble-packable string: stays `NodeContent::String` after a
/// round-trip. The empty string is excluded because the encoder emits it as a
/// zero-length binary blob (`BINARY_8 + 0`), which decodes back as
/// `NodeContent::Bytes([])` rather than `String("")`.
fn nibble_string() -> impl Strategy<Value = String> {
    prop::string::string_regex("[0-9.-]{1,16}").expect("valid regex")
}

fn attrs_strategy() -> impl Strategy<Value = Attrs> {
    // A BTreeMap keeps keys unique (duplicate keys also round-trip, but uniqueness
    // makes shrinking output easier to read) and yields a deterministic order.
    btree_map(safe_string(), safe_string(), 0..=MAX_ATTRS).prop_map(|map| {
        let mut attrs = Attrs::new();
        for (k, v) in map {
            attrs.push(k, NodeValue::String(v.into()));
        }
        attrs
    })
}

fn node_strategy() -> impl Strategy<Value = Node> {
    let leaf_content = prop_oneof![
        Just(None),
        nibble_string().prop_map(|s| Some(NodeContent::String(s.into()))),
        vec(any::<u8>(), 0..=MAX_BYTES).prop_map(|b| Some(NodeContent::Bytes(b))),
    ];

    let leaf = (safe_string(), attrs_strategy(), leaf_content)
        .prop_map(|(tag, attrs, content)| Node::new(tag, attrs, content));

    leaf.prop_recursive(MAX_DEPTH, 64, MAX_CHILDREN as u32, move |inner| {
        (
            safe_string(),
            attrs_strategy(),
            vec(inner, 1..=MAX_CHILDREN),
        )
            .prop_map(|(tag, attrs, children)| {
                Node::new(tag, attrs, Some(NodeContent::Nodes(children)))
            })
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn marshal_unmarshal_roundtrip(node in node_strategy()) {
        let bytes = marshal(&node).expect("marshal must succeed for a valid node");
        // `marshal` writes a leading format byte that `unmarshal_ref` does not expect.
        let decoded = unmarshal_ref(&bytes[1..])
            .expect("decoding our own encoder output must succeed")
            .to_owned();
        prop_assert_eq!(decoded, node);
    }
}
