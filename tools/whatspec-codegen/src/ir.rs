//! The slice of the whatspec IR this repository consumes.
//!
//! Field names and the `untagged` variant order mirror `wa-ir` exactly. An
//! untagged enum resolves to its first matching variant, so reordering `Scalar`
//! or [`TypeNode`] silently retypes generated fields; keep them as declared.

use std::collections::BTreeMap;

use serde::Deserialize;

/// The IR contract version this tool was written against. A whatspec bundle
/// stamping a different major reshapes the documents below, so refuse it rather
/// than deserialize a shape we no longer understand.
pub const SUPPORTED_SCHEMA_MAJOR: &str = "4";

/// Fields every domain document carries, used to prove the domains were all
/// extracted from one WhatsApp build.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Envelope {
    pub schema_version: String,
    pub wa_version: String,
}

/// `generated/manifest.json`, reduced to the stamp we check.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: String,
    pub wa_version: String,
}

/// A JSON scalar literal. `untagged`, so it renders as the bare JSON value and
/// the surrounding `valueType` says which arm to expect.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Scalar {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
}

/// The declared value type of an A/B prop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AbPropType {
    Bool,
    Int,
    String,
    Float,
}

/// One feature flag: name, the numeric code sent in the `<props>` IQ, and its
/// default. A flag can appear in more than one registry, so `module` is part of
/// its identity.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbPropConfig {
    pub module: String,
    pub name: String,
    pub code: u32,
    pub value_type: AbPropType,
    pub default: Scalar,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AbPropsIr {
    pub wa_version: String,
    pub configs: Vec<AbPropConfig>,
}

/// Whether an enum's variants carry strings or integers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EnumValueKind {
    String,
    Int,
}

/// One variant: the upstream member name and the value it carries on the wire.
#[derive(Debug, Clone, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub value: Scalar,
}

/// One entry of the enum catalog. A name is only unique together with its
/// module -- `ACK` and `ENUM_LID_PN` each appear in several -- so both halves
/// are part of its identity.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumDef {
    pub name: String,
    pub module: String,
    pub value_kind: EnumValueKind,
    pub variants: Vec<EnumVariant>,
    /// `true` when whatspec invented the name by concatenating the variant
    /// values, because the bundle no longer carries the upstream one. Such a
    /// name changes whenever a variant is added, so it cannot be a Rust type's
    /// identity.
    #[serde(default)]
    pub synthetic_name: Option<bool>,
    /// `true` when the integer values are bit *positions* rather than the
    /// values themselves, so a consumer has to shift before masking.
    #[serde(default)]
    pub bit_position: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnumsIr {
    pub wa_version: String,
    pub enums: Vec<EnumDef>,
}

/// Who a request is addressed to, as whatspec resolves it from the builder.
///
/// The distinction only became visible in schema 4: before it, every `w:g2`
/// stanza reported its namespace's base target, so a request sent to the wrong
/// one of the two was indistinguishable from a correct one. The symptom is a
/// server that never answers and a caller that waits out its timeout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum IqTarget {
    /// The main server, `s.whatsapp.net`.
    #[serde(rename = "s.whatsapp.net")]
    MainServer,
    /// The group server, `g.us`, which answers about groups in general.
    #[serde(rename = "g.us")]
    GroupServer,
    /// The one group the request acts on.
    #[serde(rename = "group_jid")]
    GroupJid,
    /// whatspec could not resolve the target from the builder.
    #[serde(rename = "unknown")]
    Unknown,
}

/// One outgoing IQ builder found in the bundle. A name is unique only together
/// with its module, and not always then: `resetGroupInviteCode` appears twice
/// in `WAWebGroupInviteJob` with different targets.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IqStanza {
    pub module_name: String,
    pub namespace: String,
    pub iq_type: String,
    pub target: IqTarget,
    pub exported_function: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IqIr {
    pub wa_version: String,
    pub stanzas: Vec<IqStanza>,
}

/// One entry of the top-level stanza dispatcher, and one notification kind.
///
/// Both name a value this client compares against on the wire: a stanza's tag,
/// and a `<notification type>`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifStanzaTag {
    pub tag: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifKind {
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotifIr {
    pub wa_version: String,
    pub stanza_tags: Vec<NotifStanzaTag>,
    pub notifications: Vec<NotifKind>,
}

/// An incoming server request. Read only for the stanza tag it arrives under:
/// `iq` appears here and in no other document.
#[derive(Debug, Clone, Deserialize)]
pub struct SrvReq {
    pub tag: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrvReqIr {
    pub requests: Vec<SrvReq>,
}

/// An outgoing stanza. Read only for its type, which is the tag it is sent
/// under: `ack` appears here and in no dispatcher table.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutgoingStanza {
    pub stanza_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StanzaIr {
    pub stanzas: Vec<OutgoingStanza>,
}

/// One element of an action's mutation index; `type` discriminates the shape.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum IndexPart {
    #[serde(rename = "literal")]
    Literal { value: String },
    #[serde(rename = "jid")]
    Jid { name: String },
    #[serde(rename = "boolString")]
    BoolString { name: String },
    #[serde(rename = "jidOrZero")]
    JidOrZero { name: String },
    #[serde(rename = "enum", rename_all = "camelCase")]
    Enum { name: String, proto_enum: String },
    #[serde(rename = "string")]
    StringPart { name: String },
    #[serde(rename = "unknown")]
    Unknown { name: String },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppstateAction {
    pub module: String,
    pub name: String,
    pub collection: Option<String>,
    pub version: Option<u32>,
    pub scope: String,
    pub value_field: Option<String>,
    pub value_proto_type: Option<String>,
    pub value_enum_fields: Option<BTreeMap<String, String>>,
    pub chat_jid_index: Option<i64>,
    pub index_parts: Vec<IndexPart>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppstateIr {
    pub wa_version: String,
    pub collections: Vec<String>,
    pub actions: BTreeMap<String, AppstateAction>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MexOperationKind {
    Query,
    Mutation,
}

impl MexOperationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Mutation => "mutation",
        }
    }
}

/// A node in a `variablesShape` / `response` type tree: an object, a
/// single-element array standing for a plural field, or a scalar type tag
/// (`"string"`, `"number"`, `"boolean"`, `"enum:A|B"`, `"unknown"`).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum TypeNode {
    Object(BTreeMap<String, TypeNode>),
    Array(Vec<TypeNode>),
    Leaf(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MexOperation {
    pub original_name: String,
    pub doc_id: String,
    pub operation_kind: MexOperationKind,
    #[serde(default)]
    pub variables_shape: BTreeMap<String, TypeNode>,
    #[serde(default)]
    pub response: BTreeMap<String, TypeNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MexIr {
    pub wa_version: String,
    pub operations: BTreeMap<String, MexOperation>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokensIr {
    pub single_byte: Vec<String>,
    pub double_byte: Vec<Vec<String>>,
}

/// Whether a WAM value is a boolean, an integer, a float, a string, a timer or
/// a member of one of the catalog's enums.
///
/// `timer` is the one kind whose name is not its representation: it is a
/// millisecond duration written on the wire as an integer, which is why the
/// codec treats it as one and only the generated field type keeps the
/// distinction visible.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WamValueKind {
    Boolean,
    Integer,
    Number,
    String,
    Timer,
    /// `module` is the defining `WAWebWamEnum…` module, the key into
    /// [`WamIr::enums`].
    Enum {
        module: String,
    },
}

/// One member of a WAM enum. WAM enum values are always integers.
#[derive(Debug, Clone, Deserialize)]
pub struct WamEnumVariant {
    pub key: String,
    pub value: i64,
}

/// A WAM enum referenced as a field or global type.
#[derive(Debug, Clone, Deserialize)]
pub struct WamEnum {
    pub name: String,
    /// Defining module; the catalog's key, since names repeat.
    pub module: String,
    pub variants: Vec<WamEnumVariant>,
}

/// One field of a WAM event: its camelCase name, its numeric wire id and its
/// type.
#[derive(Debug, Clone, Deserialize)]
pub struct WamField {
    pub name: String,
    pub id: u32,
    #[serde(flatten)]
    pub kind: WamValueKind,
}

/// One buffer-level global. Same id space and type vocabulary as a field, plus
/// the channels on which writing it is legal.
#[derive(Debug, Clone, Deserialize)]
pub struct WamGlobal {
    pub name: String,
    pub id: u32,
    #[serde(flatten)]
    pub kind: WamValueKind,
    pub channels: Vec<String>,
}

/// How a call site writes one field: as a key of the constructor's argument
/// object, or as a later assignment on the constructed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WamFieldWrite {
    Constructor,
    Assigned,
}

/// One field a call site writes.
#[derive(Debug, Clone, Deserialize)]
pub struct WamCallSiteField {
    pub name: String,
    pub write: WamFieldWrite,
}

/// One place in WA Web that constructs an event, and the fields it is seen
/// writing there.
#[derive(Debug, Clone, Deserialize)]
pub struct WamCallSite {
    pub module: String,
    #[serde(default)]
    pub fields: Vec<WamCallSiteField>,
    /// `true` when the site also writes fields the scan could not read, which
    /// makes `fields` a lower bound rather than the site's full set.
    #[serde(default)]
    pub partial: bool,
}

/// One WAM event definition.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WamEventDef {
    pub name: String,
    pub code: u32,
    pub module: String,
    pub channel: String,
    /// The catalog's declared weights, in `defineEvents` source order
    /// (`[alpha, beta, release]`). Never the weight a given buffer carries: a
    /// runtime lookup may override it.
    pub weights: Vec<u32>,
    pub fields: Vec<WamField>,
    #[serde(default)]
    pub private_stats_id: Option<i64>,
    #[serde(default)]
    pub call_sites: Vec<WamCallSite>,
}

/// One literal `WAWebWamConstants` exports.
#[derive(Debug, Clone, Deserialize)]
pub struct WamConstant {
    pub name: String,
    pub value: i64,
    pub module: String,
}

/// One private-stats rotation group.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WamPrivateStatsId {
    pub key: String,
    pub id: i64,
    pub rotation_period_days: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WamIr {
    pub events: Vec<WamEventDef>,
    pub enums: Vec<WamEnum>,
    #[serde(default)]
    pub globals: Vec<WamGlobal>,
    #[serde(default)]
    pub constants: Vec<WamConstant>,
    #[serde(default)]
    pub private_stats_ids: Vec<WamPrivateStatsId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_arms_follow_the_json_type() {
        let parse = |s: &str| serde_json::from_str::<Scalar>(s).expect("scalar");
        assert!(matches!(parse("true"), Scalar::Bool(true)));
        assert!(matches!(parse("7"), Scalar::Int(7)));
        assert!(matches!(parse("1.5"), Scalar::Float(_)));
        assert!(matches!(parse("\"x\""), Scalar::Str(_)));
        // A whole-number JSON literal is an Int even when the flag is declared
        // float; the emitter renders the arm, not the declared type.
        assert!(matches!(parse("0"), Scalar::Int(0)));
    }

    #[test]
    fn type_node_prefers_object_then_array_then_leaf() {
        let parse = |s: &str| serde_json::from_str::<TypeNode>(s).expect("node");
        assert!(matches!(parse(r#"{"a":"string"}"#), TypeNode::Object(_)));
        assert!(matches!(parse(r#"["string"]"#), TypeNode::Array(_)));
        assert!(matches!(parse(r#""number""#), TypeNode::Leaf(_)));
    }

    #[test]
    fn index_part_reads_every_wire_tag() {
        let parts: Vec<IndexPart> = serde_json::from_str(
            r#"[{"type":"literal","value":"a"},
                {"type":"jid","name":"chatJid"},
                {"type":"boolString","name":"fromMe"},
                {"type":"jidOrZero","name":"participant"},
                {"type":"enum","name":"k","protoEnum":"A.B"},
                {"type":"string","name":"id"},
                {"type":"unknown","name":"x"}]"#,
        )
        .expect("index parts");
        assert_eq!(parts.len(), 7);
        assert!(matches!(&parts[4], IndexPart::Enum { proto_enum, .. } if proto_enum == "A.B"));
    }

    #[test]
    fn an_unknown_index_part_tag_is_rejected() {
        // A whatspec release that adds an index-part kind must fail loudly here
        // rather than emit a registry missing an index slot.
        let err = serde_json::from_str::<IndexPart>(r#"{"type":"quantum","name":"x"}"#);
        assert!(err.is_err(), "unknown index-part tag must not deserialize");
    }
}
