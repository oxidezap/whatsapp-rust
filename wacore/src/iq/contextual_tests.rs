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
