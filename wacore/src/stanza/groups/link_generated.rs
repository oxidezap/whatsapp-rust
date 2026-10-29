//! Auto-generated qualified group LINK normalization (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Source contract: https://github.com/oxidezap/whatspec/blob/ed8da49c24a0e943854e1c7077c60129e169aeb4/generated/notif/group-link.json
//! Module-relative LINK case bytes 17137..17639.
//! Plain Object.prototype results remain explicitly unsupported.
//! "WAWebHandleGroupNotification": bundle "09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8", module "a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1", bytes 1693402..1716842.
//! "WAWebGroupType": bundle "082e0ef2faaa6247e6e17a225bb1685b7e583e946f7b0dc17ee58c431c1a07a8", module "d63035154901e9677bc08acd8ab3994845087875ca5ce84636ec765b72553519", bytes 959207..963377.
//! "WAWebHandleGroupNotificationConst": bundle "09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8", module "176daa1a4d66aee3b907bc26ba2e40da1fde706e142f44bf1bdb8cfb9fd3581e", bytes 268451..276687.

use super::super::normalized::GroupParseError;
pub(super) const LINK_TYPE_ATTRIBUTE: &str = "link_type";
pub(super) const GROUP_CHILD_TAG: &str = "group";
pub(super) const GROUP_JID_ATTRIBUTE: &str = "jid";
pub(super) const GROUP_SUBJECT_ATTRIBUTE: &str = "subject";
pub(super) const GROUP_SUBJECT_TIME_ATTRIBUTE: &str = "s_t";

pub(super) fn normalize_link(kind: &str) -> Result<Option<&'static str>, GroupParseError> {
    match kind {
        "__defineGetter__" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "__defineSetter__" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "__lookupGetter__" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "__lookupSetter__" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "__proto__" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "constructor" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "hasOwnProperty" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "isPrototypeOf" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "parent_group" => Ok(Some("parent_group_link")),
        "propertyIsEnumerable" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "sibling_group" => Ok(Some("sibling_group_link")),
        "sub_group" => Ok(Some("sub_group_link")),
        "toLocaleString" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "toString" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        "valueOf" => Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)),
        _ => Ok(None),
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
        _ => Some("hidden_group"),
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse_link;
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    #[test]
    fn official_link_cases_cover_normalization_and_field_presence() {
        for (
            kind,
            hidden,
            unsupported,
            expected_action,
            jid,
            subject,
            subject_time,
            expected_hidden,
        ) in [
            (
                "parent_group",
                false,
                false,
                Some("parent_group_link"),
                "123456@g.us",
                "synthetic group",
                123i64,
                None,
            ),
            (
                "parent_group",
                true,
                false,
                Some("parent_group_link"),
                "123456@g.us",
                "synthetic group",
                123i64,
                None,
            ),
            (
                "sibling_group",
                false,
                false,
                Some("sibling_group_link"),
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "sibling_group",
                true,
                false,
                Some("sibling_group_link"),
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "sub_group",
                false,
                false,
                Some("sub_group_link"),
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "sub_group",
                true,
                false,
                Some("sub_group_link"),
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "",
                false,
                false,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "",
                true,
                false,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "future_kind",
                false,
                false,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "future_kind",
                true,
                false,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "__defineGetter__",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "__defineGetter__",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "__defineSetter__",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "__defineSetter__",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "__lookupGetter__",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "__lookupGetter__",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "__lookupSetter__",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "__lookupSetter__",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "__proto__",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "__proto__",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "constructor",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "constructor",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "hasOwnProperty",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "hasOwnProperty",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "isPrototypeOf",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "isPrototypeOf",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "propertyIsEnumerable",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "propertyIsEnumerable",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "toLocaleString",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "toLocaleString",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "toString",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "toString",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
            (
                "valueOf",
                false,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(false),
            ),
            (
                "valueOf",
                true,
                true,
                None,
                "123456@g.us",
                "synthetic group",
                123i64,
                Some(true),
            ),
        ] {
            let group = NodeBuilder::new("group")
                .attr("jid", jid)
                .attr("subject", subject)
                .attr("s_t", subject_time.to_string())
                .children(if hidden {
                    vec![NodeBuilder::new("hidden_group").build()]
                } else {
                    vec![]
                })
                .build();
            let node = NodeBuilder::new("link")
                .attr("link_type", kind)
                .children([group])
                .build();
            let actual = parse_link(&node.as_node_ref());
            if unsupported {
                assert_eq!(
                    actual.unwrap_err(),
                    GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE)
                );
                continue;
            }
            let actual = actual.unwrap();
            assert_eq!(
                actual.kind.map(|v| v.as_str()),
                expected_action,
                "{kind}/{hidden}"
            );
            assert_eq!(actual.groups.len(), 1);
            assert_eq!(actual.groups[0].jid.to_string(), jid);
            assert_eq!(actual.groups[0].subject, subject);
            assert_eq!(actual.groups[0].subject_time, subject_time);
            assert_eq!(
                actual.groups[0].hidden_subgroup, expected_hidden,
                "{kind}/{hidden}"
            );
        }
    }
}
