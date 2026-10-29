//! Auto-generated experimental group unlink normalization (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Independent contract pin: https://github.com/oxidezap/whatspec/blob/2292a11135c5ce0bacf55e06ee9471a6b3f7f219/generated/notif/group-unlink.json
//! Module-relative helper bytes 6032..7222. Payload parsing remains handwritten.

//! "WAWebHandleGroupNotification": bundle "09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8", module "a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1", bundle bytes 1693402..1716842.
//! "WAWebGroupType": bundle "082e0ef2faaa6247e6e17a225bb1685b7e583e946f7b0dc17ee58c431c1a07a8", module "d63035154901e9677bc08acd8ab3994845087875ca5ce84636ec765b72553519", bundle bytes 959207..963377.
pub(super) fn normalize_unlink(kind: &str, reason: Option<&str>) -> Option<&'static str> {
    let kind = match kind {
        "parent_group" | "sibling_group" | "sub_group" => Some(kind),
        _ => None,
    };
    let reason = match reason {
        Some("delete_parent" | "integrity_delete_parent") => reason,
        _ => None,
    };
    match (kind, reason) {
        (None, None) => None,
        (None, Some("delete_parent")) => None,
        (None, Some("integrity_delete_parent")) => None,
        (Some("parent_group"), None) => Some("parent_group_unlink"),
        (Some("parent_group"), Some("delete_parent")) => Some("delete_parent_group_unlink"),
        (Some("parent_group"), Some("integrity_delete_parent")) => {
            Some("integrity_parent_group_unlink")
        }
        (Some("sibling_group"), None) => Some("sibling_group_unlink"),
        (Some("sibling_group"), Some("delete_parent")) => Some("sibling_group_unlink"),
        (Some("sibling_group"), Some("integrity_delete_parent")) => Some("sibling_group_unlink"),
        (Some("sub_group"), None) => Some("sub_group_unlink"),
        (Some("sub_group"), Some("delete_parent")) => Some("delete_parent_group_unlink"),
        (Some("sub_group"), Some("integrity_delete_parent")) => Some("integrity_sub_group_unlink"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_unlink;
    #[test]
    fn isolated_official_js_cases() {
        assert_eq!(
            normalize_unlink("parent_group", None),
            Some("parent_group_unlink")
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("delete_parent")),
            Some("delete_parent_group_unlink")
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("integrity_delete_parent")),
            Some("integrity_parent_group_unlink")
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("")),
            Some("parent_group_unlink")
        );
        assert_eq!(
            normalize_unlink("parent_group", Some("future_reason")),
            Some("parent_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sibling_group", None),
            Some("sibling_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("delete_parent")),
            Some("sibling_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("integrity_delete_parent")),
            Some("sibling_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("")),
            Some("sibling_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sibling_group", Some("future_reason")),
            Some("sibling_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sub_group", None),
            Some("sub_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("delete_parent")),
            Some("delete_parent_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("integrity_delete_parent")),
            Some("integrity_sub_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("")),
            Some("sub_group_unlink")
        );
        assert_eq!(
            normalize_unlink("sub_group", Some("future_reason")),
            Some("sub_group_unlink")
        );
        assert_eq!(normalize_unlink("", None), None);
        assert_eq!(normalize_unlink("", Some("delete_parent")), None);
        assert_eq!(normalize_unlink("", Some("integrity_delete_parent")), None);
        assert_eq!(normalize_unlink("", Some("")), None);
        assert_eq!(normalize_unlink("", Some("future_reason")), None);
        assert_eq!(normalize_unlink("future_kind", None), None);
        assert_eq!(normalize_unlink("future_kind", Some("delete_parent")), None);
        assert_eq!(
            normalize_unlink("future_kind", Some("integrity_delete_parent")),
            None
        );
        assert_eq!(normalize_unlink("future_kind", Some("")), None);
        assert_eq!(normalize_unlink("future_kind", Some("future_reason")), None);
    }
}
