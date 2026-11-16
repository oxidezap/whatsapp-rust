//! Auto-generated private IQ bundle conformance cases (WhatsApp 2.3000.1054751086). DO NOT EDIT.
//!
//! Reviewed whatspec 2ff44531865451b99cf2840cbe9670d4eef3f8ef.
//! Bundle set a2fcddddfcfa3fd73d880c9d6e625923c91ab2da89c9b39f5f060d8fffd955d1.
//! Archive SHA-256 4b690aa5aef47e269d72c6e32ffba2045c7b0f97b5eac2393e84ef85fd05609c.
//! Source asset: https://github.com/oxidezap/whatspec/releases/download/bundle-store/bundles-2.3000.1054751086-a2fcddddfcfa3fd73d880c9d6e625923c91ab2da89c9b39f5f060d8fffd955d1.tar.xz
//! Correlation/error dispatch are outside this payload-only fixture.
//! WASmaxGroupsAcceptGroupAddRPC: 5022cb88fc6b1cc260c97bcb81d0f4a8b577aaed1aa763556fc3831375934e61 bytes 19..6039, module SHA-256 c77a0cebff45cb0caa69d48bafc71942911370b3d8dc35ce3b228d489bd7c4ac.
//! WASmaxParseUtils: 581d9f49e50e896f381cc221f85eeb9655ab09b5ec2135ef2bc0762f24fde13a bytes 79568..86309, module SHA-256 972c3f403bc0588e2060897999b16426511d058165bd7b602dc6869eb6718889.

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
