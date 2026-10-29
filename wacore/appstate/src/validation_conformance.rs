use crate::{
    validation_batch_generated as batch, validation_generated as generated, validation_value,
};
use serde_json::{Value as Json, json};
use validation_value::{Transforms, ValidationError, Value};
struct OracleTransforms;
impl<'a> Transforms<'a> for OracleTransforms {
    fn number_or_throw_if_too_large(
        &mut self,
        v: Value<'a>,
    ) -> Result<Value<'a>, Box<ValidationError>> {
        match v {
            Value::Number(_) => Ok(v),
            _ => Err(ValidationError::dependency_boxed(
                "Error",
                "numberOrThrowIfTooLarge is given a non-safe integer",
            )),
        }
    }
    fn to_sync_key_id(&mut self, v: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> {
        Ok(v)
    }
    fn decode_sync_action_data(&mut self, v: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> {
        let code = match v {
            Value::Array(a) => a[0].number()? as u8,
            Value::Bytes(b) => b[0],
            _ => 255,
        };
        let value = match code {
            0 => return Ok(Value::object(vec![])),
            1 => Value::Null,
            2 => Value::object(vec![]),
            3 => Value::object(vec![("timestamp", Value::Null)]),
            4 => Value::object(vec![("timestamp", Value::Number(0.0))]),
            5 => Value::object(vec![("timestamp", Value::Number(1.0))]),
            255 => {
                return Err(ValidationError::dependency_boxed(
                    "Error",
                    "test decoder rejected",
                ));
            }
            _ => Value::object(vec![("timestamp", Value::Long(9007199254740992))]),
        };
        Ok(Value::object(vec![("value", value)]))
    }
    fn decode_patch_debug_data(&mut self, v: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> {
        match v {
            Value::Bytes([255, ..]) => Err(ValidationError::dependency_boxed(
                "Error",
                "test decoder rejected",
            )),
            _ => Ok(Value::object(vec![("decoded", Value::Bool(true))])),
        }
    }
}
fn read(v: &Json) -> Value<'static> {
    if v.get("$undefined") == Some(&Json::Bool(true)) {
        return Value::Undefined;
    }
    if let Some(bytes) = v.get("$bytes") {
        return Value::Bytes(Box::leak(
            bytes
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u8)
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        ));
    }
    if let Some(long) = v.get("$long") {
        return Value::Long(long.as_str().unwrap().parse().unwrap());
    }
    match v {
        Json::Null => Value::Null,
        Json::Bool(b) => Value::Bool(*b),
        Json::Number(n) => Value::Number(n.as_f64().unwrap()),
        Json::String(s) => Value::Text(s.clone().into()),
        Json::Array(a) => Value::list(a.iter().map(read).collect()),
        Json::Object(o) => Value::object(
            o.iter()
                .map(|(k, v)| (&*Box::leak(k.clone().into_boxed_str()), read(v)))
                .collect(),
        ),
    }
}
fn write(v: Value<'_>) -> Json {
    match v {
        Value::Undefined => json!({"$undefined":true}),
        Value::Null => Json::Null,
        Value::Bool(b) => json!(b),
        Value::Number(n) => {
            if n.fract() == 0.0 {
                json!(n as i64)
            } else {
                json!(n)
            }
        }
        Value::Long(n) => json!({"$long":n.to_string()}),
        Value::Text(s) => json!(s),
        Value::Bytes(b) => json!({"$bytes":b}),
        Value::Array(a) => Json::Array(a.iter().cloned().map(write).collect()),
        Value::Object(o) => Json::Object(
            o.iter()
                .map(|(k, v)| ((*k).into(), write(v.clone())))
                .collect(),
        ),
    }
}
#[test]
fn official_isolated_methods_match_generated_rust() {
    let cases: Vec<Json> =
        serde_json::from_str(include_str!("../tests/fixtures/appstate_validation.json")).unwrap();
    for (i, c) in cases.iter().enumerate() {
        let collection = Value::Text("test".into());
        let input = read(&c["input"]);
        let mut transforms = OracleTransforms;
        let result = match c["method"].as_str().unwrap() {
            "validateSnapshotProtobuf" => {
                generated::validate_snapshot_protobuf(&mut transforms, &collection, &input)
            }
            "validatePatchProtobuf" => {
                generated::validate_patch_protobuf(&mut transforms, &collection, &input)
            }
            "validateRecordProtobuf" => {
                generated::validate_record_protobuf(&mut transforms, &collection, &input)
            }
            "validateMutationProtobuf" => {
                generated::validate_mutation_protobuf(&mut transforms, &collection, &input)
            }
            "validateExternalBlobReference" => {
                generated::validate_external_blob_reference(&mut transforms, &collection, &input)
            }
            "validateKeyIdProtobuf" => generated::validate_key_id_protobuf(
                &mut transforms,
                &collection,
                &input,
                &read(&c["metric"]),
            ),
            other => panic!("unknown method {other}"),
        };
        if let Some(error) = c.get("error") {
            let actual = result.unwrap_err();
            assert_eq!(
                actual.class,
                error["class"].as_str().unwrap(),
                "case {i}: {c}"
            );
            assert_eq!(
                actual.metric.as_deref(),
                error["metric"].as_str(),
                "case {i}: {c}"
            );
            if let Some(message) = error["message"].as_str() {
                assert_eq!(actual.message, message, "case {i}: {c}");
            }
        } else {
            assert_eq!(write(result.unwrap()), c["result"], "case {i}: {c}");
        }
    }
}
#[derive(Default)]
struct Effects {
    events: Vec<Json>,
}
impl batch::BatchEffects for Effects {
    fn duplicate(&mut self, _: &batch::BatchMutation<'_>) {}
    fn report(&mut self, metric: &'static str, fatal: bool) {
        self.events.push(json!(["report", metric, fatal]));
    }
    fn local_warning(&mut self) {
        self.events.push(json!(["warning"]));
    }
    fn inspect_all(
        &mut self,
        mutations: &[batch::BatchMutation<'_>],
    ) -> Result<(), ValidationError> {
        self.events.push(json!(["inspect"]));
        let mut seen = std::collections::HashMap::new();
        for m in mutations {
            let _key_id = m.key_id;
            if let Some(previous) = seen.insert(m.index, m.binary_sync_data)
                && (m.binary_sync_data[0] == 255 || previous[0] == 255)
            {
                return Err(ValidationError::dependency(
                    "Error",
                    "test decoder rejected",
                ));
            }
        }
        Ok(())
    }
}
#[test]
fn official_batch_algorithms_match_generated_rust() {
    let cases: Vec<Json> = serde_json::from_str(include_str!(
        "../tests/fixtures/appstate_batch_validation.json"
    ))
    .unwrap();
    for (i, c) in cases.iter().enumerate() {
        let mut effects = Effects::default();
        let result = match c["method"].as_str().unwrap() {
            "validateAndTypeSetMutations" => batch::validate_and_type_set_mutations(
                &mut OracleTransforms,
                &Value::Text("test".into()),
                &read(&c["input"]),
            ),
            "validateNoDuplicatePatchVersionInCollection" => {
                let result = batch::validate_no_duplicate_patch_version(
                    c["input"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|x| x["version"]["version"].as_u64().unwrap()),
                );
                if let Err(e) = &result {
                    effects.events.push(json!(["report", e.metric, true]));
                }
                result.map(|_| Value::Undefined).map_err(Box::new)
            }
            "validateNoSameIndexForMultipleMutations" => {
                let storage: Vec<Vec<u8>> = c["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|m| vec![m["binarySyncData"][0].as_u64().unwrap() as u8])
                    .collect();
                let mutations: Vec<_> = c["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .enumerate()
                    .map(|(i, m)| batch::BatchMutation {
                        index: m["index"].as_str().unwrap(),
                        operation: m["operation"].as_i64().unwrap() as i32,
                        binary_sync_data: &storage[i],
                        key_id: &[1],
                    })
                    .collect();
                let kind = match c["kind"].as_str().unwrap() {
                    "Patch" => batch::SyncDataType::Patch,
                    "Snapshot" => batch::SyncDataType::Snapshot,
                    "Local" => batch::SyncDataType::Local,
                    _ => batch::SyncDataType::Other("Other"),
                };
                batch::validate_no_same_index(&mutations, kind, &mut effects)
                    .map(|_| Value::Undefined)
                    .map_err(Box::new)
            }
            _ => unreachable!(),
        };
        if c["error"].is_null() {
            assert_eq!(write(result.unwrap()), c["result"], "case {i}: {c}");
        } else {
            let actual = result.unwrap_err();
            assert_eq!(
                actual.class,
                c["error"]["class"].as_str().unwrap(),
                "case {i}: {c}"
            );
            if actual.class == "SyncdFatalError" {
                assert_eq!(actual.message, c["error"]["message"], "case {i}: {c}");
            }
            if c["method"] == "validateAndTypeSetMutations" && actual.metric.is_some() {
                effects.events.push(json!(["report", actual.metric, true]));
            }
        }
        assert_eq!(
            effects.events,
            c["events"].as_array().unwrap().clone(),
            "case {i}: {c}"
        );
    }
}
