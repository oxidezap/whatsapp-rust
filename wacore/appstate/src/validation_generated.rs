// Generated app-state protobuf validators. DO NOT EDIT.
use super::validation_value::{Transforms, ValidationError, Value};
#[allow(unused_variables)]
#[inline(never)]
pub(super) fn validate_external_blob_reference<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: &Value<'a>,
    a1: &Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    #[allow(unused_assignments, unused_mut)]
    let mut s0 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s1 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s2 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s3 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s4 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s5 = Value::Undefined;
    s0 = (a1).field("directPath")?;
    s1 = (a1).field("fileEncSha256")?;
    s2 = (a1).field("fileSha256")?;
    s3 = (a1).field("fileSizeBytes")?;
    s4 = (a1).field("handle")?;
    s5 = (a1).field("mediaKey")?;
    if (Value::Bool(!(s5).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_EXTERNAL_BLOB_REFERENCE_MEDIA_KEY".into()),
            "missing external blob reference media key",
        ));
    } else {
    }
    if (Value::Bool((s0).nullish())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_EXTERNAL_BLOB_REFERENCE_DIRECT_PATH".into()),
            "missing external blob reference direct path",
        ));
    } else {
    }
    if (Value::Bool(!(s2).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_EXTERNAL_BLOB_REFERENCE_FILE_SHA256".into()),
            "missing external blob reference file SHA256",
        ));
    } else {
    }
    if (Value::Bool(!(s1).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_EXTERNAL_BLOB_REFERENCE_FILE_ENC_SHA256".into()),
            "missing external blob reference file enc SHA256",
        ));
    } else {
    }
    return Ok(Value::object(vec![
        ("mediaKey", s5.clone()),
        ("directPath", s0.clone()),
        ("handle", s4.clone()),
        ("fileSizeBytes", s3.clone()),
        ("fileSha256", s2.clone()),
        ("fileEncSha256", s1.clone()),
    ]));
}
#[allow(unused_variables)]
#[inline(never)]
pub(super) fn validate_key_id_protobuf<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: &Value<'a>,
    a1: &Value<'a>,
    a2: &Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    #[allow(unused_assignments, unused_mut)]
    let mut s0 = Value::Undefined;
    s0 = if (Value::Bool((a1).nullish())).truthy() {
        Value::Undefined
    } else {
        (a1).field("id")?
    };
    if ({
        let left = Value::Bool(!(a1).truthy());
        if left.truthy() {
            left
        } else {
            Value::Bool(!(s0).truthy())
        }
    })
    .truthy()
    {
        return Err(ValidationError::fatal(
            a2.clone(),
            "missing mutation key id",
        ));
    } else {
    }
    return Ok(Value::object(vec![(
        "id",
        transforms.to_sync_key_id(s0.clone())?,
    )]));
}
#[allow(unused_variables)]
#[inline(never)]
pub(super) fn validate_mutation_protobuf<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: &Value<'a>,
    a1: &Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    #[allow(unused_assignments, unused_mut)]
    let mut s0 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s1 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s2 = Value::Undefined;
    s0 = (a1).field("operation")?;
    s1 = (a1).field("record")?;
    if (Value::Bool((s0).nullish())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_MUTATION_OPERATION".into()),
            "missing mutation operation",
        ));
    } else {
    }
    if (Value::Bool(!(s1).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_MUTATION_RECORD".into()),
            "missing mutation record",
        ));
    } else {
    }
    s2 = {
        let call_arg_0 = a0;
        let call_arg_1 = &s1;
        validate_record_protobuf(transforms, call_arg_0, call_arg_1)?
    };
    return Ok(Value::object(vec![
        ("operation", s0.clone()),
        ("record", s2.clone()),
    ]));
}
#[allow(unused_variables)]
#[inline(never)]
pub(super) fn validate_patch_protobuf<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: &Value<'a>,
    a1: &Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    #[allow(unused_assignments, unused_mut)]
    let mut s0 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s1 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s2 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s3 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s4 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s5 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s6 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s7 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s8 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s9 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s10 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s11 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s12 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s13 = Value::Undefined;
    s0 = (a1).field("clientDebugData")?;
    s1 = (a1).field("deviceIndex")?;
    s2 = (a1).field("exitCode")?;
    s3 = (a1).field("externalMutations")?;
    s4 = (a1).field("keyId")?;
    s5 = (a1).field("mutations")?;
    s6 = (a1).field("patchMac")?;
    s7 = (a1).field("snapshotMac")?;
    s8 = (a1).field("version")?;
    s9 = if (Value::Bool((s8).nullish())).truthy() {
        Value::Undefined
    } else {
        (s8).field("version")?
    };
    if ({
        let left = Value::Bool(!(s8).truthy());
        if left.truthy() {
            left
        } else {
            Value::Bool((s9).nullish())
        }
    })
    .truthy()
    {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_PATCH_VERSION".into()),
            "missing patch version",
        ));
    } else {
    }
    if ({
        let left = {
            let left = s5.clone();
            if left.truthy() {
                {
                    let left = (s5).field("length")?;
                    let right = Value::Number(0_f64);
                    Value::Bool(transforms.relational_greater(left, right)?)
                }
            } else {
                left
            }
        };
        if left.truthy() { s3.clone() } else { left }
    })
    .truthy()
    {
        return Err(ValidationError::fatal(
            Value::Text("PATCH_WITH_BOTH_INLINE_AND_EXTERNAL_MUTATIONS".into()),
            "patch with both inline and external mutations",
        ));
    } else {
    }
    if (Value::Bool(!(s7).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_PATCH_SNAPSHOT_MAC".into()),
            "missing patch snapshot mac",
        ));
    } else {
    }
    if (Value::Bool(!(s6).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_PATCH_MAC".into()),
            "missing patch mac",
        ));
    } else {
    }
    s10 = {
        let call_arg_0 = a0;
        let call_arg_1 = &s4;
        let call_arg_2 = Value::Text("MISSING_PATCH_KEY_ID".into());
        validate_key_id_protobuf(transforms, call_arg_0, call_arg_1, &call_arg_2)?
    };
    s11 = Value::Undefined;
    if ({
        let _ = {
            let left = Value::Bool(!(s0).nullish());
            if left.truthy() {
                {
                    s11 = transforms.decode_patch_debug_data(s0.clone())?;
                    s11.clone()
                }
            } else {
                left
            }
        };
        s3.clone()
    })
    .truthy()
    {
        s12 = {
            let call_arg_0 = a0;
            let call_arg_1 = &s3;
            validate_external_blob_reference(transforms, call_arg_0, call_arg_1)?
        };
        return Ok(Value::object(vec![
            (
                "version",
                Value::object(vec![(
                    "version",
                    transforms.number_or_throw_if_too_large(s9.clone())?,
                )]),
            ),
            ("mutations", Value::Undefined),
            ("externalMutations", s12.clone()),
            ("snapshotMac", s7.clone()),
            ("patchMac", s6.clone()),
            ("keyId", s10.clone()),
            ("deviceIndex", s1.clone()),
            ("clientDebugData", s11.clone()),
        ]));
    } else {
    }
    s13 = {
        let input = s5.clone();
        let mut mapped = Vec::new();
        for item in input.array()? {
            mapped.push((|| -> Result<Value, ValidationError> {
                return Ok({
                    let call_arg_0 = a0;
                    let call_arg_1 = item;
                    validate_mutation_protobuf(transforms, call_arg_0, call_arg_1)?
                });
            })()?);
        }
        Value::list(mapped)
    };
    return Ok(Value::object(vec![
        (
            "version",
            Value::object(vec![(
                "version",
                transforms.number_or_throw_if_too_large(s9.clone())?,
            )]),
        ),
        ("mutations", s13.clone()),
        ("externalMutations", Value::Undefined),
        ("exitCode", s2.clone()),
        ("snapshotMac", s7.clone()),
        ("patchMac", s6.clone()),
        ("keyId", s10.clone()),
        ("deviceIndex", s1.clone()),
        ("clientDebugData", s11.clone()),
    ]));
}
#[allow(unused_variables)]
#[inline(never)]
pub(super) fn validate_record_protobuf<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: &Value<'a>,
    a1: &Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    #[allow(unused_assignments, unused_mut)]
    let mut s0 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s1 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s2 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s3 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s4 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s5 = Value::Undefined;
    s0 = (a1).field("index")?;
    s1 = (a1).field("keyId")?;
    s2 = (a1).field("value")?;
    s3 = if (Value::Bool((s0).nullish())).truthy() {
        Value::Undefined
    } else {
        (s0).field("blob")?
    };
    s4 = if (Value::Bool((s2).nullish())).truthy() {
        Value::Undefined
    } else {
        (s2).field("blob")?
    };
    if ({
        let left = Value::Bool(!(s0).truthy());
        if left.truthy() {
            left
        } else {
            Value::Bool(!(s3).truthy())
        }
    })
    .truthy()
    {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_MUTATION_INDEX".into()),
            "missing mutation index",
        ));
    } else {
    }
    if ({
        let left = Value::Bool(!(s2).truthy());
        if left.truthy() {
            left
        } else {
            Value::Bool(!(s4).truthy())
        }
    })
    .truthy()
    {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_MUTATION_VALUE".into()),
            "missing mutation value",
        ));
    } else {
    }
    s5 = {
        let call_arg_0 = a0;
        let call_arg_1 = &s1;
        let call_arg_2 = Value::Text("MISSING_MUTATION_KEY_ID".into());
        validate_key_id_protobuf(transforms, call_arg_0, call_arg_1, &call_arg_2)?
    };
    return Ok(Value::object(vec![
        ("index", Value::object(vec![("blob", s3.clone())])),
        ("value", Value::object(vec![("blob", s4.clone())])),
        ("keyId", s5.clone()),
    ]));
}
#[allow(unused_variables)]
#[inline(never)]
pub(super) fn validate_snapshot_protobuf<'a>(
    transforms: &mut impl Transforms<'a>,
    a0: &Value<'a>,
    a1: &Value<'a>,
) -> Result<Value<'a>, ValidationError> {
    #[allow(unused_assignments, unused_mut)]
    let mut s0 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s1 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s2 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s3 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s4 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s5 = Value::Undefined;
    #[allow(unused_assignments, unused_mut)]
    let mut s6 = Value::Undefined;
    s0 = (a1).field("keyId")?;
    s1 = (a1).field("mac")?;
    s2 = (a1).field("records")?;
    s3 = (a1).field("version")?;
    s4 = if (Value::Bool((s3).nullish())).truthy() {
        Value::Undefined
    } else {
        (s3).field("version")?
    };
    if ({
        let left = Value::Bool(!(s3).truthy());
        if left.truthy() {
            left
        } else {
            Value::Bool((s4).nullish())
        }
    })
    .truthy()
    {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_SNAPSHOT_VERSION".into()),
            "missing snapshot version",
        ));
    } else {
    }
    s5 = {
        let input = s2.clone();
        let mut mapped = Vec::new();
        for item in input.array()? {
            mapped.push((|| -> Result<Value, ValidationError> {
                return Ok({
                    let call_arg_0 = a0;
                    let call_arg_1 = item;
                    validate_record_protobuf(transforms, call_arg_0, call_arg_1)?
                });
            })()?);
        }
        Value::list(mapped)
    };
    if (Value::Bool(!(s1).truthy())).truthy() {
        return Err(ValidationError::fatal(
            Value::Text("MISSING_SNAPSHOT_MAC".into()),
            "missing snapshot mac",
        ));
    } else {
    }
    s6 = {
        let call_arg_0 = a0;
        let call_arg_1 = &s0;
        let call_arg_2 = Value::Text("MISSING_SNAPSHOT_KEY_ID".into());
        validate_key_id_protobuf(transforms, call_arg_0, call_arg_1, &call_arg_2)?
    };
    return Ok(Value::object(vec![
        (
            "version",
            Value::object(vec![(
                "version",
                transforms.number_or_throw_if_too_large(s4.clone())?,
            )]),
        ),
        ("records", s5.clone()),
        ("mac", s1.clone()),
        ("keyId", s6.clone()),
    ]));
}
