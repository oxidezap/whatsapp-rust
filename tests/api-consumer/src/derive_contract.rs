//! Expand the published macros in a downstream crate, including on WASM.

use wacore::protocol::ProtocolNode;

#[derive(Debug, PartialEq, wacore::ProtocolNode)]
#[protocol(tag = "synthetic")]
pub struct Attributes {
    #[attr(name = "value")]
    pub value: String,
}

#[derive(wacore::EmptyNode)]
#[protocol(tag = "synthetic-empty")]
pub struct Empty;

pub fn parse_attributes(node: &wacore_binary::Node) -> anyhow::Result<Attributes> {
    Attributes::try_from_node(node)
}

#[derive(Debug, PartialEq, wacore::WireEnum)]
pub enum TextCode {
    #[wire = "known"]
    Known,
    #[wire_fallback]
    Unknown(String),
}

#[derive(Debug, PartialEq, wacore::WireEnum)]
#[wire(kind = "int")]
pub enum NumericCode {
    #[wire = 1]
    Known,
    #[wire_fallback]
    Unknown(i32),
}

#[derive(wacore::WireEnum)]
#[wire(tag = "kind")]
pub enum Payload {
    #[wire = "known"]
    Known { value: String },
}

pub fn serialize_payload(value: String) -> serde_json::Value {
    serde_json::to_value(Payload::Known { value }).expect("synthetic payload serializes")
}

pub fn recognize_tag(value: &str) -> bool {
    matches!(PayloadTag::try_from(value), Ok(PayloadTag::Known))
}

#[test]
fn expansion_preserves_unknown_values_and_generated_tag_dispatch() {
    let node = Attributes {
        value: "synthetic".into(),
    }
    .into_node();
    assert_eq!(parse_attributes(&node).unwrap().value, "synthetic");
    assert_eq!(Empty.into_node().tag, "synthetic-empty");
    let text: TextCode = serde_json::from_str("\"future-code\"").unwrap();
    assert_eq!(text, TextCode::Unknown("future-code".into()));
    assert_eq!(serde_json::to_string(&text).unwrap(), "\"future-code\"");
    let number: NumericCode = serde_json::from_str("987654").unwrap();
    assert_eq!(number, NumericCode::Unknown(987654));
    assert_eq!(serde_json::to_string(&number).unwrap(), "987654");
    assert!(recognize_tag("known"));
    assert!(!recognize_tag("future-code"));
    assert_eq!(
        serialize_payload("synthetic".into()),
        serde_json::json!({"kind":"known","value":"synthetic"})
    );
}
