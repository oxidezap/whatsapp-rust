// Generated from qualified SeparateSetRemoveV1. DO NOT EDIT.
use super::validation_value::{ValidationError, Value};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
pub enum SyncDataType<'a> {
    Patch,
    Snapshot,
    Local,
    Other(&'a str),
}
#[derive(Debug, Clone, Copy)]
pub struct BatchMutation<'a> {
    pub index: &'a str,
    pub operation: i32,
    pub binary_sync_data: &'a [u8],
    pub key_id: &'a [u8],
}
/// These dependencies produce diagnostics only. Snapshot inspection may decode
/// payloads and fail; its error must propagate at the original call position.
pub trait BatchEffects {
    fn duplicate(&mut self, mutation: &BatchMutation<'_>);
    fn report(&mut self, metric: &'static str, is_fatal: bool);
    fn inspect_all(&mut self, mutations: &[BatchMutation<'_>]) -> Result<(), ValidationError>;
    fn local_warning(&mut self);
}
pub fn validate_no_duplicate_patch_version(
    versions: impl IntoIterator<Item = u64>,
) -> Result<(), ValidationError> {
    let mut seen = HashSet::new();
    for version in versions {
        if !seen.insert(version) {
            return Err(ValidationError::fatal(
                Value::Text("DUPLICATE_PATCH_VERSION_IN_COLLECTION".into()),
                "duplicate patch version in collection",
            ));
        }
    }
    Ok(())
}
pub fn validate_no_same_index(
    mutations: &[BatchMutation<'_>],
    kind: SyncDataType<'_>,
    effects: &mut impl BatchEffects,
) -> Result<(), ValidationError> {
    let mut sets = HashSet::new();
    let mut removes = HashSet::new();
    for mutation in mutations {
        let duplicate = match mutation.operation {
            0 => !sets.insert(mutation.index),
            1 => !removes.insert(mutation.index),
            _ => false,
        };
        if !duplicate {
            continue;
        }
        effects.duplicate(mutation);
        match kind {
            SyncDataType::Patch => {
                effects.report("SAME_INDEX_FOR_MULTIPLE_MUTATIONS_IN_PATCH", true);
                return Err(ValidationError::fatal(
                    Value::Text("SAME_INDEX_FOR_MULTIPLE_MUTATIONS_IN_PATCH".into()),
                    "same index for multiple mutations in patch",
                ));
            }
            SyncDataType::Snapshot => {
                effects.report("SAME_INDEX_FOR_MULTIPLE_MUTATIONS_IN_SNAPSHOT", false);
                effects.inspect_all(mutations)?;
            }
            SyncDataType::Local => effects.local_warning(),
            SyncDataType::Other(value) => {
                return Err(ValidationError::dependency(
                    "Error",
                    &format!(
                        "Match: No case succesfully matched. Make exhaustive or add a wildcard case using '_'. Argument: {value}"
                    ),
                ));
            }
        }
    }
    Ok(())
}
// Generated app-state protobuf validators. DO NOT EDIT.
use super::validation_value::Transforms;
#[allow(unused_variables)]
pub(super) fn validate_and_type_set_mutations<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: Value<'a>,
    a1: Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    return Ok({
        let input = a1.clone();
        let mut mapped = Vec::new();
        for item in input.array()? {
            mapped.push((|| -> Result<Value, ValidationError> {
                #[allow(unused_assignments, unused_mut)]
                let mut s0 = Value::Undefined;
                #[allow(unused_assignments, unused_mut)]
                let mut s1 = Value::Undefined;
                #[allow(unused_assignments, unused_mut)]
                let mut s2 = Value::Undefined;
                s0 = transforms.decode_sync_action_data((item.clone()).field("binarySyncData")?)?;
                s1 = (s0.clone()).field("value")?;
                if (Value::Bool(!(s1.clone()).truthy())).truthy() {
                    return Err(ValidationError::fatal(
                        Value::Text("MISSING_ACTION_VALUE".into()),
                        "missing action value",
                    ));
                } else {
                }
                s2 = transforms
                    .maybe_number_or_throw_if_too_large((s1.clone()).field("timestamp")?)?;
                if (Value::Bool((s2.clone()).nullish())).truthy() {
                    return Err(ValidationError::fatal(
                        Value::Text("MISSING_ACTION_TIMESTAMP".into()),
                        "missing action timestamp",
                    ));
                } else {
                }
                return Ok(Value::Object(vec![
                    ("index", (item.clone()).field("index")?),
                    ("version", (item.clone()).field("version")?),
                    ("keyId", (item.clone()).field("keyId")?),
                    ("indexMac", (item.clone()).field("indexMac")?),
                    ("valueMac", (item.clone()).field("valueMac")?),
                    ("collection", (item.clone()).field("collection")?),
                    ("binarySyncData", (item.clone()).field("binarySyncData")?),
                    ("timestamp", s2.clone()),
                ]));
            })()?);
        }
        Value::Array(mapped)
    });
}
