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
#[test]
fn original_normalization_matrix() {
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "parent_group")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_link"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(actual.groups[0].hidden_subgroup, None);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "parent_group")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_link"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(actual.groups[0].hidden_subgroup, None);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "sub_group")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_link"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(actual.groups[0].hidden_subgroup, Some(false));
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "sub_group")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_link"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(actual.groups[0].hidden_subgroup, Some(true));
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "sibling_group")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sibling_group_link"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(actual.groups[0].hidden_subgroup, Some(false));
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "sibling_group")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sibling_group_link"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(actual.groups[0].hidden_subgroup, Some(true));
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized link_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-link-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized link_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-link-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "future_type")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized link_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-link-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "future_type")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized link_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-link-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "constructor")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "constructor")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__defineGetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__defineGetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__defineSetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__defineSetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "hasOwnProperty")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "hasOwnProperty")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__lookupGetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__lookupGetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__lookupSetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__lookupSetter__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "isPrototypeOf")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "isPrototypeOf")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "propertyIsEnumerable")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "propertyIsEnumerable")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "toString")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "toString")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "valueOf")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "valueOf")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__proto__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "__proto__")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "toLocaleString")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("link")
            .attr("link_type", "toLocaleString")
            .children([group])
            .build();
        let actual = parse_link_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("link_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("delete_parent_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("delete_parent_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("integrity_parent_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("integrity_parent_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "parent_group")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("parent_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("delete_parent_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("delete_parent_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("integrity_sub_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("integrity_sub_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sub_group")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), Some("sub_group_unlink"));
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "sibling_group")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(
            actual.kind.map(|v| v.as_str()),
            Some("sibling_group_unlink")
        );
        assert_eq!(actual.groups.len(), 1);
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "future_type")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        let actual = actual.unwrap();
        assert_eq!(actual.kind.map(|v| v.as_str()), None);
        assert_eq!(actual.groups.len(), 0);
        assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>("[[\"WARN\",[\"Unrecognized unlink_type: \",\"\"],\"future_type\"],[\"sendLogs\",\"group-unrecognized-unlink-type\"]]").unwrap());
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "constructor")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineGetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__defineSetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "hasOwnProperty")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupGetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__lookupSetter__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "isPrototypeOf")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "propertyIsEnumerable")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toString")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "valueOf")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "__proto__")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "integrity_delete_parent")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
    {
        let mut recorder = Recorder::default();
        let group = NodeBuilder::new("group")
            .attr("jid", "123456@g.us")
            .attr("subject", "synthetic")
            .attr("s_t", "123")
            .children([NodeBuilder::new("hidden_group").build()])
            .build();
        let node = NodeBuilder::new("unlink")
            .attr("unlink_type", "toLocaleString")
            .attr("unlink_reason", "future_reason")
            .children([group])
            .build();
        let actual = parse_unlink_with_warnings(&node.as_node_ref(), &mut recorder);
        assert_eq!(
            actual.unwrap_err(),
            GroupParseError::UnsupportedActionType("unlink_type")
        );
        assert_eq!(
            Value::Array(recorder.calls),
            serde_json::from_str::<Value>("[]").unwrap()
        );
    }
}
