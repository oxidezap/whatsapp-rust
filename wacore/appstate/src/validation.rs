//! Server-sync protobuf validation derived from the pinned Web methods.
//!
//! Validation happens before downloads or state changes. This module validates
//! presence and ordering; encrypted-content integrity remains in the processor.
pub use super::validation_batch_generated::{
    BatchEffects, BatchMutation, SyncDataType, validate_no_duplicate_patch_version,
    validate_no_same_index,
};
use super::validation_generated as generated;
pub use super::validation_value::ValidationError;
use super::validation_value::{Transforms, Value};
use waproto::whatsapp as wa;

/// A validated patch borrows the original protobuf and exposes its selected arm.
#[derive(Debug)]
pub struct ValidatedPatch<'a> {
    patch: &'a wa::SyncdPatch,
    debug_data: Option<wa::PatchDebugData>,
}
#[derive(Debug)]
pub enum MutationSource<'a> {
    Inline(&'a [wa::SyncdMutation]),
    External(&'a wa::ExternalBlobReference),
}
impl<'a> ValidatedPatch<'a> {
    pub fn mutation_source(&self) -> MutationSource<'a> {
        match self.patch.external_mutations.as_option() {
            Some(reference) => MutationSource::External(reference),
            None => MutationSource::Inline(&self.patch.mutations),
        }
    }
    /// The official external-mutations return arm omits exitCode.
    pub fn exit_code(&self) -> Option<&'a wa::ExitCode> {
        if self.patch.external_mutations.is_set() {
            None
        } else {
            self.patch.exit_code.as_option()
        }
    }
    pub fn debug_data(&self) -> Option<&wa::PatchDebugData> {
        self.debug_data.as_ref()
    }
    pub fn protobuf(&self) -> &'a wa::SyncdPatch {
        self.patch
    }
}
// Keep protobuf projection at one call boundary instead of duplicating its
// allocation and cleanup paths at every runtime ingestion site under LTO.
#[inline(never)]
pub fn validate_patch<'a>(
    collection: &str,
    patch: &'a wa::SyncdPatch,
) -> Result<ValidatedPatch<'a>, ValidationError> {
    let mut transforms = NativeTransforms::default();
    generated::validate_patch_protobuf(
        &mut transforms,
        &Value::Text(collection.into()),
        &patch_view(patch),
    )?;
    Ok(ValidatedPatch {
        patch,
        debug_data: transforms.debug_data,
    })
}
#[inline(never)]
pub fn validate_snapshot(
    collection: &str,
    snapshot: &wa::SyncdSnapshot,
) -> Result<(), ValidationError> {
    generated::validate_snapshot_protobuf(
        &mut NativeTransforms::default(),
        &Value::Text(collection.into()),
        &object(vec![
            (
                "version",
                snapshot
                    .version
                    .as_option()
                    .map_or(Value::Undefined, version_view),
            ),
            (
                "records",
                Value::list(snapshot.records.iter().map(record_view).collect()),
            ),
            ("mac", bytes(snapshot.mac.as_deref())),
            (
                "keyId",
                snapshot
                    .key_id
                    .as_option()
                    .map_or(Value::Undefined, key_view),
            ),
        ]),
    )?;
    Ok(())
}
#[inline(never)]
pub fn validate_external_reference(
    collection: &str,
    reference: &wa::ExternalBlobReference,
) -> Result<(), ValidationError> {
    generated::validate_external_blob_reference(
        &mut NativeTransforms::default(),
        &Value::Text(collection.into()),
        &external_view(reference),
    )?;
    Ok(())
}
#[inline(never)]
pub fn validate_mutation(
    collection: &str,
    mutation: &wa::SyncdMutation,
) -> Result<(), ValidationError> {
    generated::validate_mutation_protobuf(
        &mut NativeTransforms::default(),
        &Value::Text(collection.into()),
        &mutation_view(mutation),
    )?;
    Ok(())
}
/// Input to the decoded SET boundary. Identity is the original decoded index
/// string, preserving JavaScript Set equality rather than reserializing JSON.
#[derive(Debug)]
pub struct SetMutationInput<'a> {
    pub index: &'a str,
    pub version: u64,
    pub key_id: &'a [u8],
    pub index_mac: &'a [u8],
    pub value_mac: &'a [u8],
    pub collection: &'a str,
    pub binary_sync_data: &'a [u8],
}
#[derive(Debug)]
pub struct ValidatedSetMutation<'a> {
    pub input: &'a SetMutationInput<'a>,
    pub timestamp: i64,
}
#[inline(never)]
pub fn validate_set_mutations<'a>(
    collection: &str,
    inputs: &'a [SetMutationInput<'a>],
) -> Result<Vec<ValidatedSetMutation<'a>>, ValidationError> {
    let view = Value::list(
        inputs
            .iter()
            .map(|m| {
                object(vec![
                    ("index", Value::Text(m.index.into())),
                    ("version", long(m.version)),
                    ("keyId", Value::Bytes(m.key_id)),
                    ("indexMac", Value::Bytes(m.index_mac)),
                    ("valueMac", Value::Bytes(m.value_mac)),
                    ("collection", Value::Text(m.collection.into())),
                    ("binarySyncData", Value::Bytes(m.binary_sync_data)),
                ])
            })
            .collect(),
    );
    let result = super::validation_batch_generated::validate_and_type_set_mutations(
        &mut NativeTransforms::default(),
        &Value::Text(collection.into()),
        &view,
    )?;
    drop(view);
    inputs
        .iter()
        .zip(result.array()?)
        .map(|(input, value)| {
            Ok(ValidatedSetMutation {
                input,
                timestamp: value.field("timestamp")?.number()? as i64,
            })
        })
        .collect()
}
fn object<'a>(fields: Vec<(&'static str, Value<'a>)>) -> Value<'a> {
    Value::object(fields)
}
fn bytes(value: Option<&[u8]>) -> Value<'_> {
    value.map_or(Value::Undefined, Value::Bytes)
}
fn long(value: u64) -> Value<'static> {
    if value <= 9_007_199_254_740_991 {
        Value::Number(value as f64)
    } else {
        Value::Long(value)
    }
}
#[inline(never)]
fn key_view(key: &wa::KeyId) -> Value<'_> {
    object(vec![("id", bytes(key.id.as_deref()))])
}
#[inline(never)]
fn version_view(version: &wa::SyncdVersion) -> Value<'_> {
    object(vec![(
        "version",
        version.version.map_or(Value::Undefined, long),
    )])
}
#[inline(never)]
fn record_view(record: &wa::SyncdRecord) -> Value<'_> {
    object(vec![
        (
            "index",
            record.index.as_option().map_or(Value::Undefined, |index| {
                object(vec![("blob", bytes(index.blob.as_deref()))])
            }),
        ),
        (
            "value",
            record.value.as_option().map_or(Value::Undefined, |value| {
                object(vec![("blob", bytes(value.blob.as_deref()))])
            }),
        ),
        (
            "keyId",
            record.key_id.as_option().map_or(Value::Undefined, key_view),
        ),
    ])
}
#[inline(never)]
fn mutation_view(mutation: &wa::SyncdMutation) -> Value<'_> {
    object(vec![
        (
            "operation",
            mutation.operation.map_or(Value::Undefined, |operation| {
                Value::Number(operation.to_i32() as f64)
            }),
        ),
        (
            "record",
            mutation
                .record
                .as_option()
                .map_or(Value::Undefined, record_view),
        ),
    ])
}
#[inline(never)]
fn external_view(reference: &wa::ExternalBlobReference) -> Value<'_> {
    object(vec![
        ("mediaKey", bytes(reference.media_key.as_deref())),
        (
            "directPath",
            reference
                .direct_path
                .as_ref()
                .map_or(Value::Undefined, |s| Value::Text(s.as_str().into())),
        ),
        ("fileSha256", bytes(reference.file_sha256.as_deref())),
        ("fileEncSha256", bytes(reference.file_enc_sha256.as_deref())),
        (
            "handle",
            reference
                .handle
                .as_ref()
                .map_or(Value::Undefined, |s| Value::Text(s.as_str().into())),
        ),
        (
            "fileSizeBytes",
            reference.file_size_bytes.map_or(Value::Undefined, long),
        ),
    ])
}
#[inline(never)]
fn patch_view(patch: &wa::SyncdPatch) -> Value<'_> {
    object(vec![
        (
            "version",
            patch
                .version
                .as_option()
                .map_or(Value::Undefined, version_view),
        ),
        (
            "mutations",
            Value::list(patch.mutations.iter().map(mutation_view).collect()),
        ),
        (
            "externalMutations",
            patch
                .external_mutations
                .as_option()
                .map_or(Value::Undefined, external_view),
        ),
        ("snapshotMac", bytes(patch.snapshot_mac.as_deref())),
        ("patchMac", bytes(patch.patch_mac.as_deref())),
        (
            "keyId",
            patch.key_id.as_option().map_or(Value::Undefined, key_view),
        ),
        ("clientDebugData", bytes(patch.client_debug_data.as_deref())),
        (
            "deviceIndex",
            patch
                .device_index
                .map_or(Value::Undefined, |i| Value::Number(i as f64)),
        ),
        (
            "exitCode",
            patch
                .exit_code
                .as_option()
                .map_or(Value::Undefined, |_| object(vec![])),
        ),
    ])
}
#[derive(Default)]
struct NativeTransforms {
    debug_data: Option<wa::PatchDebugData>,
}
impl<'a> Transforms<'a> for NativeTransforms {
    fn relational_greater(
        &mut self,
        left: Value<'a>,
        right: Value<'a>,
    ) -> Result<bool, ValidationError> {
        // The emitter proves these operands are integer literals or lengths
        // of ordinary arrays supplied by this typed protobuf adapter.
        Ok(left.number()? > right.number()?)
    }
    fn number_or_throw_if_too_large(
        &mut self,
        value: Value<'a>,
    ) -> Result<Value<'a>, ValidationError> {
        match value {
            Value::Number(_) => Ok(value),
            _ => Err(ValidationError::dependency(
                "Error",
                "numberOrThrowIfTooLarge is given a non-safe integer",
            )),
        }
    }
    fn to_sync_key_id(&mut self, value: Value<'a>) -> Result<Value<'a>, ValidationError> {
        Ok(value)
    }
    fn decode_patch_debug_data(&mut self, value: Value<'a>) -> Result<Value<'a>, ValidationError> {
        let Value::Bytes(bytes) = value else {
            return Err(ValidationError::dependency(
                "TypeError",
                "expected debug bytes",
            ));
        };
        self.debug_data = Some(
            waproto::codec::patch_debug_data_decode(bytes)
                .map_err(|e| ValidationError::dependency("DecodeError", &e.to_string()))?,
        );
        Ok(object(vec![]))
    }
    fn decode_sync_action_data(&mut self, value: Value<'a>) -> Result<Value<'a>, ValidationError> {
        let Value::Bytes(bytes) = value else {
            return Err(ValidationError::dependency(
                "TypeError",
                "expected action bytes",
            ));
        };
        let action = waproto::codec::sync_action_data_decode(bytes)
            .map_err(|e| ValidationError::dependency("DecodeError", &e.to_string()))?;
        let value = action.value.as_option().map_or(Value::Undefined, |value| {
            object(vec![(
                "timestamp",
                value.timestamp.map_or(Value::Undefined, |timestamp| {
                    if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&timestamp) {
                        Value::Number(timestamp as f64)
                    } else {
                        Value::Long(timestamp as u64)
                    }
                }),
            )])
        });
        Ok(object(vec![("value", value)]))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use buffa::Message;
    fn make<T: Default>(set: impl FnOnce(&mut T)) -> T {
        let mut value = T::default();
        set(&mut value);
        value
    }
    fn key() -> wa::KeyId {
        make(|k: &mut wa::KeyId| k.id = Some(vec![]))
    }
    fn version() -> wa::SyncdVersion {
        make(|v: &mut wa::SyncdVersion| v.version = Some(0))
    }
    fn record() -> wa::SyncdRecord {
        make(|r: &mut wa::SyncdRecord| {
            r.index = make(|i: &mut wa::SyncdIndex| i.blob = Some(vec![])).into();
            r.value = make(|v: &mut wa::SyncdValue| v.blob = Some(vec![])).into();
            r.key_id = key().into();
        })
    }
    fn patch() -> wa::SyncdPatch {
        make(|p: &mut wa::SyncdPatch| {
            p.version = version().into();
            p.snapshot_mac = Some(vec![]);
            p.patch_mac = Some(vec![]);
            p.key_id = key().into();
        })
    }
    #[test]
    fn byte_presence_and_zero_operation_survive_native_adaptation() {
        let mut mutation = wa::SyncdMutation::default();
        mutation.operation = Some(wa::syncd_mutation::SyncdOperation::SET.into());
        mutation.record = record().into();
        assert!(validate_mutation("test", &mutation).is_ok());
        mutation.operation = None;
        assert_eq!(
            validate_mutation("test", &mutation)
                .unwrap_err()
                .metric
                .as_deref(),
            Some("MISSING_MUTATION_OPERATION")
        );
        assert!(validate_patch("test", &patch()).is_ok());
    }
    #[test]
    fn snapshot_records_precede_mac_and_key_errors() {
        let snapshot = make(|s: &mut wa::SyncdSnapshot| {
            s.version = version().into();
            s.records.push(wa::SyncdRecord::default());
        });
        assert_eq!(
            validate_snapshot("test", &snapshot)
                .unwrap_err()
                .metric
                .as_deref(),
            Some("MISSING_MUTATION_INDEX")
        );
    }
    #[test]
    fn inline_external_exclusion_precedes_mac_and_selects_projection() {
        let reference = make(|r: &mut wa::ExternalBlobReference| {
            r.media_key = Some(vec![]);
            r.direct_path = Some(String::new());
            r.file_sha256 = Some(vec![]);
            r.file_enc_sha256 = Some(vec![]);
        });
        let mut p = patch();
        p.external_mutations = reference.into();
        p.exit_code = wa::ExitCode::default().into();
        let validated = validate_patch("test", &p).unwrap();
        assert!(matches!(
            validated.mutation_source(),
            MutationSource::External(_)
        ));
        assert!(validated.exit_code().is_none());
        p.mutations.push(wa::SyncdMutation::default());
        p.patch_mac = None;
        assert_eq!(
            validate_patch("test", &p).unwrap_err().metric.as_deref(),
            Some("PATCH_WITH_BOTH_INLINE_AND_EXTERNAL_MUTATIONS")
        );
    }
    #[test]
    fn long_conversion_runs_after_key_validation() {
        let mut p = patch();
        p.version = make(|v: &mut wa::SyncdVersion| v.version = Some(9_007_199_254_740_992)).into();
        p.key_id = Default::default();
        assert_eq!(
            validate_patch("test", &p).unwrap_err().metric.as_deref(),
            Some("MISSING_PATCH_KEY_ID")
        );
        p.key_id = key().into();
        assert_eq!(validate_patch("test", &p).unwrap_err().class, "Error");
    }
    #[test]
    fn real_debug_decoder_errors_precede_inline_mutation_errors() {
        let mut p = patch();
        p.mutations.push(wa::SyncdMutation::default());
        p.client_debug_data = Some(vec![0xff, 0xff, 0xff]);
        assert_eq!(validate_patch("test", &p).unwrap_err().class, "DecodeError");
        p.client_debug_data = None;
        assert_eq!(
            validate_patch("test", &p).unwrap_err().metric.as_deref(),
            Some("MISSING_MUTATION_OPERATION")
        );
    }
    #[test]
    fn real_decoded_set_value_and_timestamp_are_required() {
        let missing = wa::SyncActionData::default().encode_to_vec();
        let zero = make(|a: &mut wa::SyncActionData| {
            a.value = make(|v: &mut wa::SyncActionValue| v.timestamp = Some(0)).into()
        })
        .encode_to_vec();
        let no_timestamp =
            make(|a: &mut wa::SyncActionData| a.value = wa::SyncActionValue::default().into())
                .encode_to_vec();
        let input = |binary_sync_data| SetMutationInput {
            index: "synthetic",
            version: 0,
            key_id: &[],
            index_mac: &[],
            value_mac: &[],
            collection: "test",
            binary_sync_data,
        };
        assert_eq!(
            validate_set_mutations("test", &[input(&missing)])
                .unwrap_err()
                .metric
                .as_deref(),
            Some("MISSING_ACTION_VALUE")
        );
        assert_eq!(
            validate_set_mutations("test", &[input(&no_timestamp)])
                .unwrap_err()
                .metric
                .as_deref(),
            Some("MISSING_ACTION_TIMESTAMP")
        );
        assert_eq!(
            validate_set_mutations("test", &[input(&zero)]).unwrap()[0].timestamp,
            0
        );
    }
}
