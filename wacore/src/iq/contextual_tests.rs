use super::{contextual, groups::*, passive::PassiveModeSpec, spec::IqSpec};
use crate::request::RequestUtils;
use wacore_binary::{Jid, Node, builder::NodeBuilder};

fn encoded(spec: &impl IqSpec) -> Vec<u8> {
    let request =
        RequestUtils::new(String::new()).build_iq_node(spec.build_iq(), Some("same-id".into()));
    wacore_binary::marshal::marshal(&request).unwrap()
}

#[test]
fn generated_builders_preserve_public_adapter_request_bytes() {
    use contextual::w_g2::*;
    let group = Jid::group("123");
    let admin = Jid::pn("15550000001");
    assert_eq!(
        encoded(&SetGroupSubjectIq::new(
            &group,
            GroupSubject::new("Olá & <group>").unwrap()
        )),
        encoded(&MakeSetSubjectRequestSpec::new(
            group.clone(),
            "Olá & <group>".as_bytes().to_vec()
        ))
    );
    assert_eq!(
        encoded(&AcceptGroupInviteV4Iq::new(
            &group,
            "synthetic-code",
            12345,
            &admin
        )),
        encoded(&MakeAcceptGroupAddRequestSpec::new(
            "synthetic-code",
            12345,
            &admin,
            group.clone()
        ))
    );
    let subgroup = Jid::group("456");
    assert_eq!(
        encoded(&JoinLinkedGroupIq::new(&group, &subgroup)),
        encoded(&MakeJoinLinkedGroupRequestSpec::new(None, &subgroup, group))
    );
    assert_eq!(
        encoded(&PassiveModeSpec::active()),
        encoded(&contextual::passive::MakeActiveIQRequestSpec)
    );
    assert_eq!(
        encoded(&PassiveModeSpec::passive()),
        encoded(&contextual::passive::MakePassiveIQRequestSpec)
    );
}

fn country_response(kind: &str, child: Node) -> Node {
    NodeBuilder::new("iq")
        .attr("id", "sent-id")
        .attr("from", "s.whatsapp.net")
        .attr("type", kind)
        .children([child])
        .build()
}

#[test]
fn country_code_requires_payload_and_exact_paired_error_alternative() {
    use contextual::md::*;
    let request = NodeBuilder::new("iq")
        .attr("id", "sent-id")
        .attr("to", "s.whatsapp.net")
        .build();
    let response = country_response(
        "result",
        NodeBuilder::new("country_code").attr("iso", "BR").build(),
    );
    let MakeGetCountryCodeRequestResponse::GetCountryCodeResponse(value) =
        MakeGetCountryCodeRequestSpec::parse_sent_response(
            &response.as_node_ref(),
            &request.as_node_ref(),
        )
        .unwrap()
    else {
        panic!("country-code response")
    };
    assert_eq!(value.country_code_iso, "BR");
    for (code, text, valid) in [
        (400, "bad-request", true),
        (500, "internal-server-error", true),
        (400, "internal-server-error", false),
        (500, "bad-request", false),
    ] {
        let response = country_response(
            "error",
            NodeBuilder::new("error")
                .attr("code", code.to_string())
                .attr("text", text)
                .build(),
        );
        assert_eq!(
            MakeGetCountryCodeRequestSpec::parse_sent_response(
                &response.as_node_ref(),
                &request.as_node_ref()
            )
            .is_ok(),
            valid
        );
    }
}

#[test]
fn v4_approval_precedes_compatibility_metadata_but_bare_success_keeps_it() {
    let group = Jid::group("123");
    let spec = AcceptGroupInviteV4Iq::new(&group, "synthetic", 12345, &Jid::pn("15550000001"));
    let request =
        RequestUtils::new(String::new()).build_iq_node(spec.build_iq(), Some("sent-id".into()));
    for tag in ["group", "community"] {
        for approval in [false, true] {
            let mut children = vec![NodeBuilder::new(tag).attr("jid", Jid::group("456")).build()];
            if approval {
                children.push(NodeBuilder::new("membership_approval_request").build());
            }
            let response = NodeBuilder::new("iq")
                .attr("id", "sent-id")
                .attr("from", &group)
                .attr("type", "result")
                .children(children)
                .build();
            let result = spec
                .parse_response_with_context(&response.as_node_ref(), &request.as_node_ref())
                .unwrap();
            if approval {
                assert!(matches!(result, JoinGroupResult::PendingApproval(jid) if jid == group));
            } else {
                assert!(matches!(result, JoinGroupResult::Joined(jid) if jid == Jid::group("456")));
            }
        }
    }
}

#[test]
fn both_mode_contracts_validate_sent_context_and_unique_mode_child() {
    for (spec, tag, other) in [
        (PassiveModeSpec::active(), "active", "passive"),
        (PassiveModeSpec::passive(), "passive", "active"),
    ] {
        let request =
            RequestUtils::new(String::new()).build_iq_node(spec.build_iq(), Some("sent-id".into()));
        for (id, from, children, valid) in [
            ("sent-id", "s.whatsapp.net", vec![tag], true),
            ("wrong-id", "s.whatsapp.net", vec![tag], false),
            ("sent-id", "g.us", vec![tag], false),
            ("sent-id", "s.whatsapp.net", vec![], false),
            ("sent-id", "s.whatsapp.net", vec![other], false),
            ("sent-id", "s.whatsapp.net", vec![tag, tag], false),
        ] {
            let response = NodeBuilder::new("iq")
                .attr("id", id)
                .attr("from", from)
                .attr("type", "result")
                .children(
                    children
                        .into_iter()
                        .map(|tag| NodeBuilder::new(tag).build()),
                )
                .build();
            assert_eq!(
                spec.parse_response_with_context(&response.as_node_ref(), &request.as_node_ref())
                    .is_ok(),
                valid,
                "{tag} {id} {from}"
            );
        }
    }
}
