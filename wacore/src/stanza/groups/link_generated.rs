//! Auto-generated group link decisions (WhatsApp 2.3000.1052559453). DO NOT EDIT.
//!
//! Qualified source: https://github.com/oxidezap/whatspec/blob/afa3e343f37b0b64f03cffcacf2448ef883a2d8b/generated/notif/group-link.json
//! Module-relative source bytes 17403..18068. Null results preserve ordered warnings.
use super::super::normalization_warning::{Decision, WarningSpec};
use super::super::normalized::GroupParseError;
pub(super) const LINK_TYPE_ATTRIBUTE: &str = "link_type";
pub(super) const GROUP_CHILD_TAG: &str = "group";
pub(super) const GROUP_JID_ATTRIBUTE: &str = "jid";
pub(super) const GROUP_SUBJECT_ATTRIBUTE: &str = "subject";
pub(super) const GROUP_SUBJECT_TIME_ATTRIBUTE: &str = "s_t";
pub(super) fn decide_link(kind: &str) -> Result<Decision, GroupParseError> {
    match kind {
        "__defineGetter__" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "__defineSetter__" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "__lookupGetter__" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "__lookupSetter__" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "__proto__" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "constructor" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "hasOwnProperty" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "isPrototypeOf" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "parent_group" => Ok(Decision::Action("parent_group_link")),
        "propertyIsEnumerable" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "sibling_group" => Ok(Decision::Action("sibling_group_link")),
        "sub_group" => Ok(Decision::Action("sub_group_link")),
        "toLocaleString" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "toString" => Err(GroupParseError::UnsupportedActionType("link_type")),
        "valueOf" => Err(GroupParseError::UnsupportedActionType("link_type")),
        _ => Ok(Decision::Null(WarningSpec {
            template: &["Unrecognized link_type: ", ""],
            event: "group-unrecognized-link-type",
        })),
    }
}
pub(super) fn hidden_subgroup_child(kind: &str) -> Option<&'static str> {
    match kind {
        "__defineGetter__" => Some("hidden_group"),
        "__defineSetter__" => Some("hidden_group"),
        "__lookupGetter__" => Some("hidden_group"),
        "__lookupSetter__" => Some("hidden_group"),
        "__proto__" => Some("hidden_group"),
        "constructor" => Some("hidden_group"),
        "hasOwnProperty" => Some("hidden_group"),
        "isPrototypeOf" => Some("hidden_group"),
        "parent_group" => None,
        "propertyIsEnumerable" => Some("hidden_group"),
        "sibling_group" => Some("hidden_group"),
        "sub_group" => Some("hidden_group"),
        "toLocaleString" => Some("hidden_group"),
        "toString" => Some("hidden_group"),
        "valueOf" => Some("hidden_group"),
        _ => None,
    }
}
