//! Auto-generated group unlink decisions (WhatsApp 2.3000.1052559453). DO NOT EDIT.
//!
//! Qualified source: <https://github.com/oxidezap/whatspec/blob/afa3e343f37b0b64f03cffcacf2448ef883a2d8b/generated/notif/group-unlink.json>
//! Module-relative source bytes 5939..7296. Null results preserve ordered warnings.
use super::super::normalization_warning::{Decision, WarningSpec};
use super::super::normalized::GroupParseError;
pub(super) fn decide_unlink(kind: &str, reason: Option<&str>) -> Result<Decision, GroupParseError> {
    let kind = match kind {
        "__defineGetter__"
        | "__defineSetter__"
        | "__lookupGetter__"
        | "__lookupSetter__"
        | "__proto__"
        | "constructor"
        | "hasOwnProperty"
        | "isPrototypeOf"
        | "parent_group"
        | "propertyIsEnumerable"
        | "sibling_group"
        | "sub_group"
        | "toLocaleString"
        | "toString"
        | "valueOf" => Some(kind),
        _ => None,
    };
    let reason = match reason {
        Some("delete_parent" | "integrity_delete_parent") => reason,
        _ => None,
    };
    match (kind, reason) {
        (None, None) => Ok(Decision::Null(WarningSpec {
            template: &["Unrecognized unlink_type: ", ""],
            event: "group-unrecognized-unlink-type",
        })),
        (None, Some("delete_parent")) => Ok(Decision::Null(WarningSpec {
            template: &["Unrecognized unlink_type: ", ""],
            event: "group-unrecognized-unlink-type",
        })),
        (None, Some("integrity_delete_parent")) => Ok(Decision::Null(WarningSpec {
            template: &["Unrecognized unlink_type: ", ""],
            event: "group-unrecognized-unlink-type",
        })),
        (Some("__defineGetter__"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__defineGetter__"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__defineGetter__"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__defineSetter__"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__defineSetter__"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__defineSetter__"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__lookupGetter__"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__lookupGetter__"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__lookupGetter__"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__lookupSetter__"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__lookupSetter__"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__lookupSetter__"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__proto__"), None) => Err(GroupParseError::UnsupportedActionType("unlink_type")),
        (Some("__proto__"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("__proto__"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("constructor"), None) => Err(GroupParseError::UnsupportedActionType("unlink_type")),
        (Some("constructor"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("constructor"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("hasOwnProperty"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("hasOwnProperty"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("hasOwnProperty"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("isPrototypeOf"), None) => Err(GroupParseError::UnsupportedActionType("unlink_type")),
        (Some("isPrototypeOf"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("isPrototypeOf"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("parent_group"), None) => Ok(Decision::Action("parent_group_unlink")),
        (Some("parent_group"), Some("delete_parent")) => {
            Ok(Decision::Action("delete_parent_group_unlink"))
        }
        (Some("parent_group"), Some("integrity_delete_parent")) => {
            Ok(Decision::Action("integrity_parent_group_unlink"))
        }
        (Some("propertyIsEnumerable"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("propertyIsEnumerable"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("propertyIsEnumerable"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("sibling_group"), None) => Ok(Decision::Action("sibling_group_unlink")),
        (Some("sibling_group"), Some("delete_parent")) => {
            Ok(Decision::Action("sibling_group_unlink"))
        }
        (Some("sibling_group"), Some("integrity_delete_parent")) => {
            Ok(Decision::Action("sibling_group_unlink"))
        }
        (Some("sub_group"), None) => Ok(Decision::Action("sub_group_unlink")),
        (Some("sub_group"), Some("delete_parent")) => {
            Ok(Decision::Action("delete_parent_group_unlink"))
        }
        (Some("sub_group"), Some("integrity_delete_parent")) => {
            Ok(Decision::Action("integrity_sub_group_unlink"))
        }
        (Some("toLocaleString"), None) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("toLocaleString"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("toLocaleString"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("toString"), None) => Err(GroupParseError::UnsupportedActionType("unlink_type")),
        (Some("toString"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("toString"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("valueOf"), None) => Err(GroupParseError::UnsupportedActionType("unlink_type")),
        (Some("valueOf"), Some("delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        (Some("valueOf"), Some("integrity_delete_parent")) => {
            Err(GroupParseError::UnsupportedActionType("unlink_type"))
        }
        _ => Err(GroupParseError::Field("unqualified unlink decision")),
    }
}
