//! External compilation of hierarchy, lookup envelopes, and call-action wire tags.
//!
//! Deliberate alias removals through the formerly public reexports. Check each
//! alias separately so restoring just one cannot hide behind the other's error.
//! The positive counterparts use the same accessible paths and setup. Nightly
//! also checks the error-code annotations; stable accepts any compilation error,
//! so its results still need the positive controls and diagnostic inspection.
//!
//! ```
//! use whatsapp_rust::{GroupHierarchy, GroupMetadata};
//! let metadata = GroupMetadata::new("120363000000000021@g.us".parse().unwrap());
//! assert_eq!(metadata.hierarchy(), GroupHierarchy::Standalone);
//! ```
//!
//! ```compile_fail,E0432
//! use whatsapp_rust::GroupType;
//! ```
//!
//! ```compile_fail,E0432
//! use whatsapp_rust::group_type;
//! ```
//!
//! ```
//! use whatsapp_rust::features::{Community, GroupHierarchy, GroupMetadata};
//! let _: Option<Community<'_>> = None;
//! let metadata = GroupMetadata::new("120363000000000021@g.us".parse().unwrap());
//! assert_eq!(metadata.hierarchy(), GroupHierarchy::Standalone);
//! ```
//!
//! ```compile_fail,E0432
//! use whatsapp_rust::features::GroupType;
//! ```
//!
//! ```compile_fail,E0432
//! use whatsapp_rust::features::group_type;
//! ```
//!
//! Module privacy is a separate contract, not evidence of alias removal: the
//! original legacy-path check would fail even if both aliases were restored.
//!
//! ```compile_fail
//! use whatsapp_rust::features::community::{GroupType, group_type};
//! ```
//!
//! Even a retained public type cannot be imported through that private module:
//!
//! ```compile_fail
//! use whatsapp_rust::features::community::Community;
//! ```
//!
//! ```
//! use whatsapp_rust::wacore::types::call::CallAction;
//! fn current(action: &CallAction) { let _: &str = action.wire_tag(); }
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
