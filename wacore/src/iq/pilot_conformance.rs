//! Auto-generated private IQ bundle conformance cases (WhatsApp 2.3000.1052559453). DO NOT EDIT.
//!
//! Reviewed whatspec afa3e343f37b0b64f03cffcacf2448ef883a2d8b.
//! Bundle set d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf.
//! Archive SHA-256 2958d99a3d84141fa8479f1f991e06246e68c063ca8f618a8d5d361287387f62.
//! Source asset: https://github.com/oxidezap/whatspec/releases/download/bundle-store/bundles-2.3000.1052559453-d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf.tar.xz
//! Correlation/error dispatch are outside this payload-only fixture.
//! WASmaxParseUtils: a018f64e2c88a44300ee70b669e4fbfdb09d2491aa0552b48bbad42d39614d42 bytes 27431..34172, module SHA-256 972c3f403bc0588e2060897999b16426511d058165bd7b602dc6869eb6718889.
//! WASmaxGroupsAcceptGroupAddRPC: 29bc9cdd1e375455ed50fd0505d66eae6d7635bbab9aeee94851139d0ac38b82 bytes 19..6075, module SHA-256 de8a93c8f1a2460f967ff5ef44fff52dbd4f907d80cc56ee6b3c931ea12bdda2.

use super::pilots::{AcceptGroupAddSuccess, parse_accept_group_add_payload};
use wacore_binary::{NodeContent, builder::NodeBuilder};

#[test]
fn accept_group_add_matches_reviewed_bundle_cases() {
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(None)
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::Success,
        "absent"
    );
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(Some(NodeContent::Nodes(vec![
            NodeBuilder::new("membership_approval_request").build(),
        ])))
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::GroupJoinRequestSuccess,
        "unique"
    );
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(Some(NodeContent::Nodes(vec![
            NodeBuilder::new("membership_approval_request").build(),
            NodeBuilder::new("membership_approval_request").build(),
        ])))
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::Success,
        "duplicate"
    );
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(Some(NodeContent::Nodes(vec![
            NodeBuilder::new("future").build(),
        ])))
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::Success,
        "unknown"
    );
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(Some(NodeContent::Nodes(vec![
            NodeBuilder::new("future").build(),
            NodeBuilder::new("membership_approval_request").build(),
        ])))
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::GroupJoinRequestSuccess,
        "unique_with_unknown"
    );
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(Some(NodeContent::Nodes(vec![
            NodeBuilder::new("membership_approval_request").build(),
            NodeBuilder::new("future").build(),
            NodeBuilder::new("membership_approval_request").build(),
        ])))
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::Success,
        "unknown_between_duplicates"
    );
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .apply_content(Some(NodeContent::Bytes(vec![
            0x66, 0x75, 0x74, 0x75, 0x72, 0x65,
        ])))
        .build();
    assert_eq!(
        parse_accept_group_add_payload(&response.as_node_ref()),
        AcceptGroupAddSuccess::Success,
        "scalar"
    );
}
