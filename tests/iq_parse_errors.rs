//! Public consumer checks for decoded IQ structure errors.

use whatsapp_rust::anyhow::Context;
use whatsapp_rust::features::GroupError;
use whatsapp_rust::request::IqError;
use whatsapp_rust::wacore::iq::{
    newsletter::MyAddOnsSpec, node::IqParseError, passive::PassiveModeSpec, spec::IqSpec,
};
use whatsapp_rust::{ErrorChainExt, NodeBuilder};

fn assert_parse_only(error: &impl ErrorChainExt) {
    assert_eq!(error.server_rejection(), None);
    assert_eq!(error.http_status(), None);
    assert!(!error.is_timeout());
    assert!(!error.is_transport_unavailable());
    assert!(error.store_failure().is_none());
}

#[test]
fn passive_response_requires_the_requested_child_and_keeps_its_name() {
    for (passive, expected, other) in [(true, "passive", "active"), (false, "active", "passive")] {
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new(other).build()])
            .build();
        let error = PassiveModeSpec::new(passive)
            .parse_response(&response.as_node_ref())
            .unwrap_err();
        assert!(matches!(error.downcast_ref::<IqParseError>(),
            Some(IqParseError::MissingChild { tag }) if tag == expected));
        assert_eq!(error.to_string(), format!("<{expected}> child not found"));

        let error = GroupError::Iq(IqError::ParseError(error.context("consumer operation")));
        let recovered = error
            .sources()
            .find_map(|source| source.downcast_ref::<IqParseError>())
            .unwrap();
        assert!(matches!(recovered, IqParseError::MissingChild { tag } if tag == expected));
        assert_parse_only(&error);
    }
}

#[test]
fn newsletter_missing_child_and_attribute_remain_distinct() {
    let jid = "120363000000000001@newsletter".parse().unwrap();
    let spec = MyAddOnsSpec::new(&jid, 20);
    let missing_child = NodeBuilder::new("iq").attr("type", "result").build();
    let error = spec
        .parse_response(&missing_child.as_node_ref())
        .unwrap_err();
    assert!(matches!(error.downcast_ref::<IqParseError>(),
        Some(IqParseError::MissingChild { tag }) if tag == "my_addons"));

    let missing_attribute = NodeBuilder::new("iq")
        .children([NodeBuilder::new("my_addons")
            .children([NodeBuilder::new("messages")
                .attr("unrelated", "fixture-secret-must-not-appear")
                .build()])
            .build()])
        .build();
    let error = spec
        .parse_response(&missing_attribute.as_node_ref())
        .context("reading newsletter add-ons")
        .unwrap_err();
    assert!(matches!(error.downcast_ref::<IqParseError>(),
        Some(IqParseError::MissingAttribute { name }) if name == "jid"));
    assert_eq!(
        error.root_cause().to_string(),
        "missing required attribute jid"
    );
    assert!(!format!("{error:?}").contains("fixture-secret-must-not-appear"));
    let error = IqError::ParseError(error);
    assert!(
        error
            .sources()
            .any(|source| matches!(source.downcast_ref::<IqParseError>(),
        Some(IqParseError::MissingAttribute { name }) if name == "jid"))
    );
    assert_parse_only(&error);
}

#[test]
fn empty_present_attribute_is_still_accepted_by_the_helper() {
    use whatsapp_rust::wacore::iq::groups::GetGroupInviteLinkIq;
    let jid = "120363000000000001@g.us".parse().unwrap();
    let response = NodeBuilder::new("iq")
        .children([NodeBuilder::new("invite").attr("code", "").build()])
        .build();
    assert_eq!(
        GetGroupInviteLinkIq::new(&jid, false)
            .parse_response(&response.as_node_ref())
            .unwrap(),
        "https://chat.whatsapp.com/"
    );
}
