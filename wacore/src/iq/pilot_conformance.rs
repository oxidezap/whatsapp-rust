//! Auto-generated private IQ bundle conformance cases (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Reviewed whatspec c581c20a9d0605df49abf297f2e7cfc04ccf1ff7.
//! Bundle set 05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b.
//! Archive SHA-256 22243b9c2ff18a66cea8fd66551821fe0f9f09786d107e2be8284637db86c349.
//! Source asset: https://github.com/oxidezap/whatspec/releases/download/bundle-store/bundles-2.3000.1047483476-05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b.tar.xz
//! Correlation/error dispatch are outside this payload-only fixture.
//! WASmaxParseUtils: 09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8 bytes 2256336..2263058, module SHA-256 a2bbd76df3740e48f0f5c979c5ecc94d30217e726c7b42f2b23490161bcc42e6.
//! WASmaxInGroupsAcceptGroupAddResponseGroupJoinRequestSuccess: 34859849724a114869fd78e4479a6c412b4ab3ec36c3da2985d8ec4ecb75d5d2 bytes 4229..4728, module SHA-256 3e137254ccda2e5b4b5d97fd512fe1ff74449cb360886195ecd46e098956718a.
//! WASmaxInGroupsAcceptGroupAddResponseSuccess: 34859849724a114869fd78e4479a6c412b4ab3ec36c3da2985d8ec4ecb75d5d2 bytes 5098..5459, module SHA-256 2881a4c961499317969ca0c9a4ddbbd41cc60f4c0ac6567467e4ffc9cce62354.
//! WASmaxGroupsAcceptGroupAddRPC: 34859849724a114869fd78e4479a6c412b4ab3ec36c3da2985d8ec4ecb75d5d2 bytes 5922..7533, module SHA-256 2167e222e0875e85276bd519e5bd86839ba1e92808af5a1320b424d84ca3d7ae.

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
