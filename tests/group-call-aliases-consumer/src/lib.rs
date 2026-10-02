//! External compilation of hierarchy, lookup envelopes, and call-action wire tags.
//!
//! Deliberate alias removals (the positive tests below cover the replacements):
//!
//! ```compile_fail
//! use whatsapp_rust::{GroupType, group_type};
//! ```
//!
//! ```compile_fail
//! use whatsapp_rust::features::community::{GroupType, group_type};
//! ```
//!
//! ```compile_fail
//! use whatsapp_rust::wacore::types::call::CallAction;
//! fn legacy(action: &CallAction) { let _ = action.action_kind(); }
//! ```

#[cfg(test)]
#[path = "../../group_lookup_contract.rs"]
mod group_lookup_contract;

#[cfg(test)]
mod call_action {
    use whatsapp_rust::wacore::stanza::call::parse_call_stanza;
    use whatsapp_rust::wacore::types::call::{CallAction, CallActionTag};
    use whatsapp_rust::{NodeBuilder, serde_json};

    #[test]
    fn parsed_action_exposes_the_same_wire_string_and_json_shape() {
        let node = NodeBuilder::new("call")
            .attr("from", "111111111111111@lid")
            .attr("id", "STANZA-ID")
            .attr("t", "1704067200")
            .children([NodeBuilder::new("offer_notice")
                .attr("call-id", "CALL-ID")
                .attr("call-creator", "111111111111111@lid")
                .attr("media", "video")
                .attr("type", "group")
                .build()])
            .build();
        let call = parse_call_stanza(&node.as_node_ref()).unwrap().unwrap();
        assert!(matches!(
            call.action,
            CallAction::OfferNotice {
                is_video: true,
                is_group: true,
                ..
            }
        ));
        assert_eq!(call.action.wire_tag(), "offer_notice");
        assert_eq!(
            CallActionTag::try_from(call.action.wire_tag()).unwrap(),
            CallActionTag::OfferNotice
        );
        assert_eq!(call.action.call_id(), "CALL-ID");
        assert_eq!(call.stanza_id, "STANZA-ID");
        assert_eq!(
            serde_json::to_value(&call.action).unwrap(),
            serde_json::json!({
                "type": "offer_notice",
                "call_id": "CALL-ID",
                "call_creator": {
                    "user": "111111111111111", "server": "lid", "agent": 0,
                    "device": 0, "integrator": 0
                },
                "is_video": true,
                "is_group": true
            })
        );
    }

    #[test]
    fn unknown_action_is_ignored_but_malformed_known_action_is_not() {
        let unknown = NodeBuilder::new("call")
            .children([NodeBuilder::new("future_call_action").build()])
            .build();
        assert!(parse_call_stanza(&unknown.as_node_ref()).unwrap().is_none());
        for missing in ["call-id", "call-creator"] {
            let mut action = NodeBuilder::new("offer_notice");
            if missing != "call-id" {
                action = action.attr("call-id", "CALL-ID");
            }
            let malformed = NodeBuilder::new("call")
                .attr("from", "111111111111111@lid")
                .attr("t", "1704067200")
                .children([action.build()])
                .build();
            let error = parse_call_stanza(&malformed.as_node_ref()).unwrap_err();
            assert!(error.to_string().contains(&format!("missing '{missing}'")));
        }
    }
}
