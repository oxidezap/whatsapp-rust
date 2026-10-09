//! Expectations read independently from the verified WA Web 2.3000.1047483476
//! bundle, also exercised by whatspec's independent_iq_cases.rs at 587a514.
//! Set hash 05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b.
//! Request/result source: bundles 65b89bd6, 34859849, 09d91fdc. Exact module
//! spans are recorded in whatspec/docs/iq-response-admission.md at that pin.
//! Tests use public specs and the existing error-dispatch boundary, not emitted
//! constants or snapshots of the generator's output.

use wacore::iq::groups::{AcceptGroupInviteV4Iq, GroupSubject, JoinGroupResult, SetGroupSubjectIq};
use wacore::iq::spec::IqSpec;
use wacore::request::{IqError, RequestUtils};
use wacore_binary::{Jid, Node, NodeContent, builder::NodeBuilder};

fn specs() -> (SetGroupSubjectIq, AcceptGroupInviteV4Iq) {
    let group: Jid = "123@g.us".parse().unwrap();
    let admin: Jid = "456@s.whatsapp.net".parse().unwrap();
    (
        SetGroupSubjectIq::new(&group, GroupSubject::new("Olá & <grupo>").unwrap()),
        AcceptGroupInviteV4Iq::new(&group, "invite-42", 98765, &admin),
    )
}

fn result(content: Option<NodeContent>) -> Node {
    NodeBuilder::new("iq")
        .attr("id", "req-1")
        .attr("from", "123@g.us")
        .attr("type", "result")
        .apply_content(content)
        .build()
}

#[test]
fn requests_preserve_wire_and_node_representation() {
    let (subject, accept) = specs();
    let utils = RequestUtils::new("test".into());
    for (request, expected) in [
        (
            subject.build_iq(),
            NodeBuilder::new("subject")
                .string_content("Olá & <grupo>")
                .build(),
        ),
        (
            accept.build_iq(),
            NodeBuilder::new("accept")
                .attr("code", "invite-42")
                .attr("expiration", 98765i64)
                .attr("admin", &accept.admin_jid)
                .build(),
        ),
    ] {
        let actual = utils.build_iq_node(request, Some("req-1".into()));
        let expected = NodeBuilder::new("iq")
            .attr("id", "req-1")
            .attr("xmlns", "w:g2")
            .attr("type", "set")
            .attr("to", &accept.group_jid)
            .children([expected])
            .build();
        assert_eq!(actual, expected);
        assert_eq!(
            wacore_binary::marshal(&actual).unwrap(),
            wacore_binary::marshal(&expected).unwrap()
        );
    }
}

#[test]
fn integer_extremes_and_jid_flavors_are_not_normalized() {
    let group: Jid = "123@g.us".parse().unwrap();
    for admin in ["456@s.whatsapp.net", "456@lid", "456:7@s.whatsapp.net"] {
        let admin: Jid = admin.parse().unwrap();
        for expiration in [i64::MIN, -1, 0, i64::MAX] {
            let req = AcceptGroupInviteV4Iq::new(&group, "", expiration, &admin).build_iq();
            assert_eq!(req.to, group);
            let Some(NodeContent::Nodes(nodes)) = req.content else {
                panic!("nodes")
            };
            assert_eq!(
                nodes[0].attrs().optional_string("expiration").as_deref(),
                Some(expiration.to_string().as_str())
            );
            assert_eq!(nodes[0].attrs().optional_jid("admin").unwrap(), admin);
        }
    }
}

#[test]
fn approval_is_a_unique_presence_gate_not_a_jid_payload() {
    let (subject, accept) = specs();
    for approval in [
        NodeBuilder::new("membership_approval_request").build(),
        NodeBuilder::new("membership_approval_request")
            .attr("future", "value")
            .build(),
        NodeBuilder::new("membership_approval_request")
            .attr("jid", "not-a-jid")
            .build(),
    ] {
        let response = result(Some(NodeContent::Nodes(vec![approval])));
        assert_eq!(
            accept.parse_response(&response.as_node_ref()).unwrap(),
            JoinGroupResult::PendingApproval(accept.group_jid.clone())
        );
        subject.parse_response(&response.as_node_ref()).unwrap();
    }
}

#[test]
fn failed_approval_gate_falls_through_to_bare_success() {
    let (subject, accept) = specs();
    let approval = || NodeBuilder::new("membership_approval_request").build();
    for content in [
        None,
        Some(NodeContent::Nodes(vec![])),
        Some(NodeContent::Nodes(vec![approval(), approval()])),
        Some(NodeContent::Nodes(vec![NodeBuilder::new("future").build()])),
        Some(NodeContent::Bytes(vec![0xff])),
        Some(NodeContent::Bytes(vec![])),
    ] {
        let response = result(content);
        assert_eq!(
            accept.parse_response(&response.as_node_ref()).unwrap(),
            JoinGroupResult::Joined(accept.group_jid.clone())
        );
        subject.parse_response(&response.as_node_ref()).unwrap();
    }
}

#[test]
fn unknown_siblings_do_not_disable_a_valid_approval_gate() {
    let (_, accept) = specs();
    let response = result(Some(NodeContent::Nodes(vec![
        NodeBuilder::new("future").build(),
        NodeBuilder::new("membership_approval_request").build(),
    ])));
    assert_eq!(
        accept.parse_response(&response.as_node_ref()).unwrap(),
        JoinGroupResult::PendingApproval(accept.group_jid.clone())
    );
}

#[test]
fn legacy_explicit_group_results_keep_their_jid_and_errors() {
    let (_, accept) = specs();
    for tag in ["group", "community"] {
        let response = result(Some(NodeContent::Nodes(vec![
            NodeBuilder::new(tag).attr("jid", "789@g.us").build(),
        ])));
        assert_eq!(
            accept.parse_response(&response.as_node_ref()).unwrap(),
            JoinGroupResult::Joined("789@g.us".parse().unwrap())
        );
        let response = result(Some(NodeContent::Nodes(vec![
            NodeBuilder::new(tag).build(),
        ])));
        assert!(accept.parse_response(&response.as_node_ref()).is_err());
    }
}

#[test]
fn runtime_error_conversion_remains_the_owner_of_errors() {
    let utils = RequestUtils::new("test".into());
    for (code, text) in [
        (304, "already-exists"),
        (400, "bad-request"),
        (406, "not-acceptable"),
        (500, "resource-constraint"),
        (500, "internal-server-error"),
        (599, "future-error"),
    ] {
        let response = NodeBuilder::new("iq")
            .attr("type", "error")
            .children([NodeBuilder::new("error")
                .attr("code", code)
                .attr("text", text)
                .attr("type", "wait")
                .attr("backoff", 7u32)
                .build()])
            .build();
        match utils
            .parse_iq_response(&response.as_node_ref())
            .unwrap_err()
        {
            IqError::ServerError {
                code: actual,
                text: actual_text,
                error_type,
                backoff,
            } => {
                assert_eq!(actual, code as u16);
                assert_eq!(actual_text, text);
                assert_eq!(error_type.as_deref(), Some("wait"));
                assert_eq!(backoff, Some(7));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    for ty in [None, Some("set"), Some("future")] {
        let mut node = NodeBuilder::new("iq");
        if let Some(ty) = ty {
            node = node.attr("type", ty);
        }
        assert!(matches!(
            utils.parse_iq_response(&node.build().as_node_ref()),
            Err(IqError::UnexpectedResponseType { .. })
        ));
    }
}
