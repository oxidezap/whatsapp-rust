//! Auto-generated original LINK/UNLINK normalization conformance (WhatsApp 2.3000.1052559453). DO NOT EDIT.
//!
use super::{
    link_normalized::parse_link_with_warnings,
    normalization_warning::GroupWarningSink,
    normalized::{GroupParseError, parse_unlink_with_warnings},
};
use serde_json::{Value, json};
use wacore_binary::builder::NodeBuilder;
#[derive(Default)]
struct Recorder {
    calls: Vec<Value>,
}
impl GroupWarningSink for Recorder {
    fn warn(
        &mut self,
        template: &'static [&'static str],
        value: &str,
    ) -> Result<(), GroupParseError> {
        self.calls.push(json!(["WARN", template, value]));
        Ok(())
    }
    fn send_logs(&mut self, event: &'static str) -> Result<(), GroupParseError> {
        self.calls.push(json!(["sendLogs", event]));
        Ok(())
    }
}
struct Case(
    &'static str,
    &'static str,
    Option<&'static str>,
    bool,
    usize,
);
struct Outcome {
    kind: Option<&'static str>,
    unsupported: bool,
    groups: usize,
    hidden: Option<bool>,
    calls: &'static str,
}
const OUTCOMES: &[Outcome] = &[
    Outcome {
        kind: Some("parent_group_link"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: Some("sub_group_link"),
        unsupported: false,
        groups: 1,
        hidden: Some(false),
        calls: "[]",
    },
    Outcome {
        kind: Some("sub_group_link"),
        unsupported: false,
        groups: 1,
        hidden: Some(true),
        calls: "[]",
    },
    Outcome {
        kind: Some("sibling_group_link"),
        unsupported: false,
        groups: 1,
        hidden: Some(false),
        calls: "[]",
    },
    Outcome {
        kind: Some("sibling_group_link"),
        unsupported: false,
        groups: 1,
        hidden: Some(true),
        calls: "[]",
    },
    Outcome {
        kind: None,
        unsupported: false,
        groups: 0,
        hidden: None,
        calls: "[[\"WARN\",[\"Unrecognized link_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-link-type\"]]",
    },
    Outcome {
        kind: None,
        unsupported: false,
        groups: 0,
        hidden: None,
        calls: "[[\"WARN\",[\"Unrecognized link_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-link-type\"]]",
    },
    Outcome {
        kind: None,
        unsupported: true,
        groups: 1,
        hidden: Some(false),
        calls: "[]",
    },
    Outcome {
        kind: None,
        unsupported: true,
        groups: 1,
        hidden: Some(true),
        calls: "[]",
    },
    Outcome {
        kind: Some("parent_group_unlink"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: Some("delete_parent_group_unlink"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: Some("integrity_parent_group_unlink"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: Some("sub_group_unlink"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: Some("integrity_sub_group_unlink"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: Some("sibling_group_unlink"),
        unsupported: false,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
    Outcome {
        kind: None,
        unsupported: false,
        groups: 0,
        hidden: None,
        calls: "[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]",
    },
    Outcome {
        kind: None,
        unsupported: false,
        groups: 0,
        hidden: None,
        calls: "[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]",
    },
    Outcome {
        kind: None,
        unsupported: true,
        groups: 1,
        hidden: None,
        calls: "[]",
    },
];
const CASES: &[Case] = &[
    Case("link", "parent_group", None, false, 0),
    Case("link", "parent_group", None, true, 0),
    Case("link", "sub_group", None, false, 1),
    Case("link", "sub_group", None, true, 2),
    Case("link", "sibling_group", None, false, 3),
    Case("link", "sibling_group", None, true, 4),
    Case("link", "", None, false, 5),
    Case("link", "", None, true, 5),
    Case("link", "future_type", None, false, 6),
    Case("link", "future_type", None, true, 6),
    Case("link", "constructor", None, false, 7),
    Case("link", "constructor", None, true, 8),
    Case("link", "__defineGetter__", None, false, 7),
    Case("link", "__defineGetter__", None, true, 8),
    Case("link", "__defineSetter__", None, false, 7),
    Case("link", "__defineSetter__", None, true, 8),
    Case("link", "hasOwnProperty", None, false, 7),
    Case("link", "hasOwnProperty", None, true, 8),
    Case("link", "__lookupGetter__", None, false, 7),
    Case("link", "__lookupGetter__", None, true, 8),
    Case("link", "__lookupSetter__", None, false, 7),
    Case("link", "__lookupSetter__", None, true, 8),
    Case("link", "isPrototypeOf", None, false, 7),
    Case("link", "isPrototypeOf", None, true, 8),
    Case("link", "propertyIsEnumerable", None, false, 7),
    Case("link", "propertyIsEnumerable", None, true, 8),
    Case("link", "toString", None, false, 7),
    Case("link", "toString", None, true, 8),
    Case("link", "valueOf", None, false, 7),
    Case("link", "valueOf", None, true, 8),
    Case("link", "__proto__", None, false, 7),
    Case("link", "__proto__", None, true, 8),
    Case("link", "toLocaleString", None, false, 7),
    Case("link", "toLocaleString", None, true, 8),
    Case("unlink", "parent_group", None, false, 9),
    Case("unlink", "parent_group", None, true, 9),
    Case("unlink", "parent_group", Some("delete_parent"), false, 10),
    Case("unlink", "parent_group", Some("delete_parent"), true, 10),
    Case(
        "unlink",
        "parent_group",
        Some("integrity_delete_parent"),
        false,
        11,
    ),
    Case(
        "unlink",
        "parent_group",
        Some("integrity_delete_parent"),
        true,
        11,
    ),
    Case("unlink", "parent_group", Some(""), false, 9),
    Case("unlink", "parent_group", Some(""), true, 9),
    Case("unlink", "parent_group", Some("future_reason"), false, 9),
    Case("unlink", "parent_group", Some("future_reason"), true, 9),
    Case("unlink", "sub_group", None, false, 12),
    Case("unlink", "sub_group", None, true, 12),
    Case("unlink", "sub_group", Some("delete_parent"), false, 10),
    Case("unlink", "sub_group", Some("delete_parent"), true, 10),
    Case(
        "unlink",
        "sub_group",
        Some("integrity_delete_parent"),
        false,
        13,
    ),
    Case(
        "unlink",
        "sub_group",
        Some("integrity_delete_parent"),
        true,
        13,
    ),
    Case("unlink", "sub_group", Some(""), false, 12),
    Case("unlink", "sub_group", Some(""), true, 12),
    Case("unlink", "sub_group", Some("future_reason"), false, 12),
    Case("unlink", "sub_group", Some("future_reason"), true, 12),
    Case("unlink", "sibling_group", None, false, 14),
    Case("unlink", "sibling_group", None, true, 14),
    Case("unlink", "sibling_group", Some("delete_parent"), false, 14),
    Case("unlink", "sibling_group", Some("delete_parent"), true, 14),
    Case(
        "unlink",
        "sibling_group",
        Some("integrity_delete_parent"),
        false,
        14,
    ),
    Case(
        "unlink",
        "sibling_group",
        Some("integrity_delete_parent"),
        true,
        14,
    ),
    Case("unlink", "sibling_group", Some(""), false, 14),
    Case("unlink", "sibling_group", Some(""), true, 14),
    Case("unlink", "sibling_group", Some("future_reason"), false, 14),
    Case("unlink", "sibling_group", Some("future_reason"), true, 14),
    Case("unlink", "", None, false, 15),
    Case("unlink", "", None, true, 15),
    Case("unlink", "", Some("delete_parent"), false, 15),
    Case("unlink", "", Some("delete_parent"), true, 15),
    Case("unlink", "", Some("integrity_delete_parent"), false, 15),
    Case("unlink", "", Some("integrity_delete_parent"), true, 15),
    Case("unlink", "", Some(""), false, 15),
    Case("unlink", "", Some(""), true, 15),
    Case("unlink", "", Some("future_reason"), false, 15),
    Case("unlink", "", Some("future_reason"), true, 15),
    Case("unlink", "future_type", None, false, 16),
    Case("unlink", "future_type", None, true, 16),
    Case("unlink", "future_type", Some("delete_parent"), false, 16),
    Case("unlink", "future_type", Some("delete_parent"), true, 16),
    Case(
        "unlink",
        "future_type",
        Some("integrity_delete_parent"),
        false,
        16,
    ),
    Case(
        "unlink",
        "future_type",
        Some("integrity_delete_parent"),
        true,
        16,
    ),
    Case("unlink", "future_type", Some(""), false, 16),
    Case("unlink", "future_type", Some(""), true, 16),
    Case("unlink", "future_type", Some("future_reason"), false, 16),
    Case("unlink", "future_type", Some("future_reason"), true, 16),
    Case("unlink", "constructor", None, false, 17),
    Case("unlink", "constructor", None, true, 17),
    Case("unlink", "constructor", Some("delete_parent"), false, 17),
    Case("unlink", "constructor", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "constructor",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "constructor",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "constructor", Some(""), false, 17),
    Case("unlink", "constructor", Some(""), true, 17),
    Case("unlink", "constructor", Some("future_reason"), false, 17),
    Case("unlink", "constructor", Some("future_reason"), true, 17),
    Case("unlink", "__defineGetter__", None, false, 17),
    Case("unlink", "__defineGetter__", None, true, 17),
    Case(
        "unlink",
        "__defineGetter__",
        Some("delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__defineGetter__",
        Some("delete_parent"),
        true,
        17,
    ),
    Case(
        "unlink",
        "__defineGetter__",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__defineGetter__",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "__defineGetter__", Some(""), false, 17),
    Case("unlink", "__defineGetter__", Some(""), true, 17),
    Case(
        "unlink",
        "__defineGetter__",
        Some("future_reason"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__defineGetter__",
        Some("future_reason"),
        true,
        17,
    ),
    Case("unlink", "__defineSetter__", None, false, 17),
    Case("unlink", "__defineSetter__", None, true, 17),
    Case(
        "unlink",
        "__defineSetter__",
        Some("delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__defineSetter__",
        Some("delete_parent"),
        true,
        17,
    ),
    Case(
        "unlink",
        "__defineSetter__",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__defineSetter__",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "__defineSetter__", Some(""), false, 17),
    Case("unlink", "__defineSetter__", Some(""), true, 17),
    Case(
        "unlink",
        "__defineSetter__",
        Some("future_reason"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__defineSetter__",
        Some("future_reason"),
        true,
        17,
    ),
    Case("unlink", "hasOwnProperty", None, false, 17),
    Case("unlink", "hasOwnProperty", None, true, 17),
    Case("unlink", "hasOwnProperty", Some("delete_parent"), false, 17),
    Case("unlink", "hasOwnProperty", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "hasOwnProperty",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "hasOwnProperty",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "hasOwnProperty", Some(""), false, 17),
    Case("unlink", "hasOwnProperty", Some(""), true, 17),
    Case("unlink", "hasOwnProperty", Some("future_reason"), false, 17),
    Case("unlink", "hasOwnProperty", Some("future_reason"), true, 17),
    Case("unlink", "__lookupGetter__", None, false, 17),
    Case("unlink", "__lookupGetter__", None, true, 17),
    Case(
        "unlink",
        "__lookupGetter__",
        Some("delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__lookupGetter__",
        Some("delete_parent"),
        true,
        17,
    ),
    Case(
        "unlink",
        "__lookupGetter__",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__lookupGetter__",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "__lookupGetter__", Some(""), false, 17),
    Case("unlink", "__lookupGetter__", Some(""), true, 17),
    Case(
        "unlink",
        "__lookupGetter__",
        Some("future_reason"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__lookupGetter__",
        Some("future_reason"),
        true,
        17,
    ),
    Case("unlink", "__lookupSetter__", None, false, 17),
    Case("unlink", "__lookupSetter__", None, true, 17),
    Case(
        "unlink",
        "__lookupSetter__",
        Some("delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__lookupSetter__",
        Some("delete_parent"),
        true,
        17,
    ),
    Case(
        "unlink",
        "__lookupSetter__",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__lookupSetter__",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "__lookupSetter__", Some(""), false, 17),
    Case("unlink", "__lookupSetter__", Some(""), true, 17),
    Case(
        "unlink",
        "__lookupSetter__",
        Some("future_reason"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__lookupSetter__",
        Some("future_reason"),
        true,
        17,
    ),
    Case("unlink", "isPrototypeOf", None, false, 17),
    Case("unlink", "isPrototypeOf", None, true, 17),
    Case("unlink", "isPrototypeOf", Some("delete_parent"), false, 17),
    Case("unlink", "isPrototypeOf", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "isPrototypeOf",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "isPrototypeOf",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "isPrototypeOf", Some(""), false, 17),
    Case("unlink", "isPrototypeOf", Some(""), true, 17),
    Case("unlink", "isPrototypeOf", Some("future_reason"), false, 17),
    Case("unlink", "isPrototypeOf", Some("future_reason"), true, 17),
    Case("unlink", "propertyIsEnumerable", None, false, 17),
    Case("unlink", "propertyIsEnumerable", None, true, 17),
    Case(
        "unlink",
        "propertyIsEnumerable",
        Some("delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "propertyIsEnumerable",
        Some("delete_parent"),
        true,
        17,
    ),
    Case(
        "unlink",
        "propertyIsEnumerable",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "propertyIsEnumerable",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "propertyIsEnumerable", Some(""), false, 17),
    Case("unlink", "propertyIsEnumerable", Some(""), true, 17),
    Case(
        "unlink",
        "propertyIsEnumerable",
        Some("future_reason"),
        false,
        17,
    ),
    Case(
        "unlink",
        "propertyIsEnumerable",
        Some("future_reason"),
        true,
        17,
    ),
    Case("unlink", "toString", None, false, 17),
    Case("unlink", "toString", None, true, 17),
    Case("unlink", "toString", Some("delete_parent"), false, 17),
    Case("unlink", "toString", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "toString",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "toString",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "toString", Some(""), false, 17),
    Case("unlink", "toString", Some(""), true, 17),
    Case("unlink", "toString", Some("future_reason"), false, 17),
    Case("unlink", "toString", Some("future_reason"), true, 17),
    Case("unlink", "valueOf", None, false, 17),
    Case("unlink", "valueOf", None, true, 17),
    Case("unlink", "valueOf", Some("delete_parent"), false, 17),
    Case("unlink", "valueOf", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "valueOf",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "valueOf",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "valueOf", Some(""), false, 17),
    Case("unlink", "valueOf", Some(""), true, 17),
    Case("unlink", "valueOf", Some("future_reason"), false, 17),
    Case("unlink", "valueOf", Some("future_reason"), true, 17),
    Case("unlink", "__proto__", None, false, 17),
    Case("unlink", "__proto__", None, true, 17),
    Case("unlink", "__proto__", Some("delete_parent"), false, 17),
    Case("unlink", "__proto__", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "__proto__",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "__proto__",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "__proto__", Some(""), false, 17),
    Case("unlink", "__proto__", Some(""), true, 17),
    Case("unlink", "__proto__", Some("future_reason"), false, 17),
    Case("unlink", "__proto__", Some("future_reason"), true, 17),
    Case("unlink", "toLocaleString", None, false, 17),
    Case("unlink", "toLocaleString", None, true, 17),
    Case("unlink", "toLocaleString", Some("delete_parent"), false, 17),
    Case("unlink", "toLocaleString", Some("delete_parent"), true, 17),
    Case(
        "unlink",
        "toLocaleString",
        Some("integrity_delete_parent"),
        false,
        17,
    ),
    Case(
        "unlink",
        "toLocaleString",
        Some("integrity_delete_parent"),
        true,
        17,
    ),
    Case("unlink", "toLocaleString", Some(""), false, 17),
    Case("unlink", "toLocaleString", Some(""), true, 17),
    Case("unlink", "toLocaleString", Some("future_reason"), false, 17),
    Case("unlink", "toLocaleString", Some("future_reason"), true, 17),
];
#[test]
fn original_normalization_matrix() {
    assert_eq!(CASES.len(), 204);
    for &Case(family, kind, reason, hidden, index) in CASES {
        let expected = &OUTCOMES[index];
        let mut recorder = Recorder::default();
        let mut group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123");
        if hidden {
            group = group.children([NodeBuilder::new("hidden_group").build()]);
        }
        let mut node = NodeBuilder::new(family).attr(
            if family == "link" {
                "link_type"
            } else {
                "unlink_type"
            },
            kind,
        );
        if let Some(reason) = reason {
            node = node.attr("unlink_reason", reason);
        }
        let node = node.children([group.build()]).build();
        let actual = if family == "link" {
            parse_link_with_warnings(&node.as_node_ref(), &mut recorder).map(|value| {
                (
                    value.kind.map(|kind| kind.as_str()),
                    value.groups.len(),
                    value.groups.first().and_then(|group| group.hidden_subgroup),
                )
            })
        } else {
            parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder).map(|value| {
                (
                    value.kind.map(|kind| kind.as_str()),
                    value.groups.len(),
                    None,
                )
            })
        };
        if expected.unsupported {
            assert_eq!(
                actual.unwrap_err(),
                GroupParseError::UnsupportedActionType(if family == "link" {
                    "link_type"
                } else {
                    "unlink_type"
                })
            );
        } else {
            assert_eq!(
                actual.unwrap(),
                (expected.kind, expected.groups, expected.hidden),
                "{family}/{kind}/{reason:?}/{hidden}"
            );
        }
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>(expected.calls).unwrap(),
            "{family}/{kind}/{reason:?}/{hidden}"
        );
    }
}
