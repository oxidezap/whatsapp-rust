//! Ordered, closed-vocabulary programs for the server-sync protobuf boundary.
//! No crypto, storage, download, or session recovery instructions are admitted.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppstateValidationIr {
    pub format_version: u32,
    pub wa_version: String,
    pub module_sha256: String,
    pub functions: BTreeMap<String, ValidationFunction>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidationFunction {
    pub module: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub parameters: usize,
    pub body: Vec<ValidationStatement>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum ValidationStatement {
    Bind {
        slot: usize,
        value: ValidationExpression,
    },
    If {
        condition: ValidationExpression,
        yes: Vec<Self>,
        no: Vec<Self>,
    },
    Evaluate {
        value: ValidationExpression,
    },
    Fatal {
        metric: ValidationExpression,
        message: String,
    },
    Return {
        value: ValidationExpression,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", deny_unknown_fields)]
pub enum ValidationExpression {
    Argument {
        index: usize,
    },
    Slot {
        index: usize,
    },
    Undefined,
    Null,
    Integer {
        value: i64,
    },
    String {
        value: String,
    },
    Field {
        object: Box<Self>,
        field: String,
    },
    Not {
        value: Box<Self>,
    },
    And {
        left: Box<Self>,
        right: Box<Self>,
    },
    Or {
        left: Box<Self>,
        right: Box<Self>,
    },
    Nullish {
        value: Box<Self>,
        negate: bool,
    },
    Greater {
        left: Box<Self>,
        right: Box<Self>,
    },
    Choose {
        condition: Box<Self>,
        yes: Box<Self>,
        no: Box<Self>,
    },
    Assign {
        slot: usize,
        value: Box<Self>,
    },
    Sequence {
        values: Vec<Self>,
    },
    Object {
        fields: Vec<(String, Self)>,
    },
    Map {
        input: Box<Self>,
        body: Vec<ValidationStatement>,
    },
    Validate {
        validator: String,
        arguments: Vec<Self>,
    },
    /// Closed dependency vocabulary, qualified separately against the bundle.
    Transform {
        transform: ValidationTransform,
        arguments: Vec<Self>,
    },
    Metric {
        name: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidationTransform {
    SafeInteger,
    SyncKeyId,
    DecodePatchDebugData,
    DecodeSyncActionData,
    MaybeNumber,
}

/// Qualified batch algorithm. Unlike the expression compiler, this recognizer
/// requires an alpha-equivalent full module and rejects any new control flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppstateBatchValidationIr {
    pub format_version: u32,
    pub wa_version: String,
    pub module_sha256: String,
    pub recipe: BatchValidationRecipe,
    pub set_operation: i32,
    pub remove_operation: i32,
    pub operation_enum_sha256: String,
    pub type_set: ValidationFunction,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchValidationRecipe {
    SeparateSetRemoveV1,
}
