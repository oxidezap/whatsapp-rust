//! Bounded, ordered program for group-notification envelope validation.
//! Action parsing is an explicit external operation, not a raw-node result.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupEnvelopeContract {
    pub module_name: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub locals: u32,
    pub prefix: Vec<GroupEnvelopeOp>,
    pub actions: GroupEnvelopeActions,
    pub projection: GroupEnvelopeExpr,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupEnvelopeActions {
    pub start_byte: usize,
    pub end_byte: usize,
    pub result_local: u32,
    pub missing_identification_local: u32,
    pub missing_identification_tag: String,
    pub unknown_action_error_prefix: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GroupEnvelopeOp {
    Bind {
        local: u32,
        value: Option<GroupEnvelopeExpr>,
    },
    Evaluate {
        value: GroupEnvelopeExpr,
    },
    If {
        condition: GroupEnvelopeExpr,
        then_ops: Vec<Self>,
        else_ops: Vec<Self>,
    },
    TryDiagnostic {
        body: Vec<Self>,
        message_parts: Vec<String>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "expr",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GroupEnvelopeExpr {
    Node,
    Local {
        local: u32,
    },
    String {
        value: String,
    },
    Integer {
        value: i64,
    },
    Bool {
        value: bool,
    },
    Null,
    Undefined,
    Context {
        fact: GroupEnvelopeContextFact,
    },
    Read {
        method: GroupEnvelopeRead,
        name: String,
    },
    Convert {
        conversion: GroupEnvelopeConversion,
        value: Box<Self>,
    },
    IsEmpty {
        value: Box<Self>,
    },
    Not {
        value: Box<Self>,
    },
    Binary {
        operator: GroupEnvelopeBinary,
        left: Box<Self>,
        right: Box<Self>,
    },
    Logical {
        operator: GroupEnvelopeLogical,
        left: Box<Self>,
        right: Box<Self>,
    },
    Conditional {
        condition: Box<Self>,
        then_value: Box<Self>,
        else_value: Box<Self>,
    },
    Assign {
        local: u32,
        value: Box<Self>,
    },
    Sequence {
        values: Vec<Self>,
    },
    Reject {
        message: Box<Self>,
    },
    Object {
        fields: Vec<GroupEnvelopeField>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupEnvelopeField {
    pub name: String,
    pub value: GroupEnvelopeExpr,
}
macro_rules! simple_enum {($name:ident {$($v:ident),*})=>{
    #[derive(Debug,Clone,Copy,PartialEq,Eq,Serialize,Deserialize)]
    #[serde(rename_all="camelCase")]
    pub enum $name {$($v),*}
}}
simple_enum!(GroupEnvelopeContextFact {
    MeDeviceLid,
    MaybeMeDevicePn,
    UsernameDisplayed
});
simple_enum!(GroupEnvelopeRead {
    AssertTag,
    HasAttr,
    AttrString,
    AttrGroupJid,
    AttrUserJid,
    MaybeAttrPhoneUserJid,
    AttrTime,
    MaybeAttrString
});
simple_enum!(GroupEnvelopeConversion {
    GroupJidToWid,
    UserJidToWid,
    AsIsoCountryCode,
    ToJid
});
simple_enum!(GroupEnvelopeBinary {
    StrictEqual,
    StrictNotEqual,
    NullishEqual,
    NullishNotEqual,
    Add
});
simple_enum!(GroupEnvelopeLogical { And, Or });
