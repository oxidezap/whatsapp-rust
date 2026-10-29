//! Auto-generated experimental group unlink normalization (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Independent contract pin: https://github.com/oxidezap/whatspec/blob/57494ccd865fa6471743264fea036f00d26d89cb/generated/notif/group-unlink.json
//! Module-relative helper bytes 6032..7222. Payload parsing remains handwritten.

//! "WAWebHandleGroupNotification": bundle "09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8", module "a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1", bundle bytes 1693402..1716842.
//! "WAWebGroupType": bundle "082e0ef2faaa6247e6e17a225bb1685b7e583e946f7b0dc17ee58c431c1a07a8", module "d63035154901e9677bc08acd8ab3994845087875ca5ce84636ec765b72553519", bundle bytes 959207..963377.
//! "WAWebHandleGroupNotificationConst": bundle "09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8", module "176daa1a4d66aee3b907bc26ba2e40da1fde706e142f44bf1bdb8cfb9fd3581e", bundle bytes 268451..276687.
use super::GroupParseError;
pub(super) fn normalize_unlink(
    kind: &str,
    reason: Option<&str>,
) -> Result<Option<&'static str>, GroupParseError> {
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
        (None, None) => Ok(None),
        (None, Some("delete_parent")) => Ok(None),
        (None, Some("integrity_delete_parent")) => Ok(None),
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
        (Some("parent_group"), None) => Ok(Some("parent_group_unlink")),
        (Some("parent_group"), Some("delete_parent")) => Ok(Some("delete_parent_group_unlink")),
        (Some("parent_group"), Some("integrity_delete_parent")) => {
            Ok(Some("integrity_parent_group_unlink"))
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
        (Some("sibling_group"), None) => Ok(Some("sibling_group_unlink")),
        (Some("sibling_group"), Some("delete_parent")) => Ok(Some("sibling_group_unlink")),
        (Some("sibling_group"), Some("integrity_delete_parent")) => {
            Ok(Some("sibling_group_unlink"))
        }
        (Some("sub_group"), None) => Ok(Some("sub_group_unlink")),
        (Some("sub_group"), Some("delete_parent")) => Ok(Some("delete_parent_group_unlink")),
        (Some("sub_group"), Some("integrity_delete_parent")) => {
            Ok(Some("integrity_sub_group_unlink"))
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
        _ => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_unlink;
    #[test]
    fn isolated_official_js_cases() {
        assert_eq!(
            normalize_unlink("parent_group", None),
            Ok(Some("parent_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("delete_parent")),
            Ok(Some("delete_parent_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("integrity_delete_parent")),
            Ok(Some("integrity_parent_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("")),
            Ok(Some("parent_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("future_reason")),
            Ok(Some("parent_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sibling_group", None),
            Ok(Some("sibling_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("delete_parent")),
            Ok(Some("sibling_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("integrity_delete_parent")),
            Ok(Some("sibling_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("")),
            Ok(Some("sibling_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("future_reason")),
            Ok(Some("sibling_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sub_group", None),
            Ok(Some("sub_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("delete_parent")),
            Ok(Some("delete_parent_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("integrity_delete_parent")),
            Ok(Some("integrity_sub_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("")),
            Ok(Some("sub_group_unlink"))
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("future_reason")),
            Ok(Some("sub_group_unlink"))
        );
        assert_eq!(normalize_unlink("", None), Ok(None));
        assert_eq!(normalize_unlink("", Some("delete_parent")), Ok(None));
        assert_eq!(
            normalize_unlink("", Some("integrity_delete_parent")),
            Ok(None)
        );
        assert_eq!(normalize_unlink("", Some("")), Ok(None));
        assert_eq!(normalize_unlink("", Some("future_reason")), Ok(None));
        assert_eq!(normalize_unlink("future_kind", None), Ok(None));
        assert_eq!(
            normalize_unlink("future_kind", Some("delete_parent")),
            Ok(None)
        );
        assert_eq!(
            normalize_unlink("future_kind", Some("integrity_delete_parent")),
            Ok(None)
        );
        assert_eq!(normalize_unlink("future_kind", Some("")), Ok(None));
        assert_eq!(
            normalize_unlink("future_kind", Some("future_reason")),
            Ok(None)
        );
        assert_eq!(
            normalize_unlink("constructor", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("__defineGetter__", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("__defineSetter__", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("hasOwnProperty", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("__lookupGetter__", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("__lookupSetter__", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("isPrototypeOf", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("propertyIsEnumerable", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("toString", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("valueOf", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("__proto__", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
        assert_eq!(
            normalize_unlink("toLocaleString", None),
            Err(super::super::GroupParseError::UnsupportedActionType(
                "unlink_type"
            ))
        );
    }
}
