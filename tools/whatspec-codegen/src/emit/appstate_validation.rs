//! Native Rust control flow from the closed validation program. No evaluator.
use crate::naming::{rust_lit, snake_case};
use wa_ir::{
    AppstateValidationIr, ValidationExpression as E, ValidationStatement as S,
    ValidationTransform as T,
};

pub fn generate_appstate_validation(ir: &AppstateValidationIr) -> Result<String, String> {
    if ir.format_version != 1 {
        return Err("unsupported validation format".into());
    }
    let mut out = String::from(
        "// Generated app-state protobuf validators. DO NOT EDIT.\nuse super::validation_value::{Value, ValidationError, Transforms};\n",
    );
    for (name, f) in &ir.functions {
        if f.parameters > 3 {
            return Err("validator arity".into());
        }
        out.push_str(&format!("#[allow(unused_variables)]\npub(super) fn {}<'a>(transforms: &mut impl Transforms<'a>, {})->Result<Value<'a>,ValidationError>{{\n",snake_case(name),(0..f.parameters).map(|i|format!("a{i}:Value<'a>")).collect::<Vec<_>>().join(",")));
        for i in owned_slots(&f.body) {
            out.push_str(&format!(
                "#[allow(unused_assignments,unused_mut)] let mut s{i}=Value::Undefined;\n"
            ));
        }
        statements(&f.body, &mut out)?;
        out.push_str("}\n");
    }
    Ok(out)
}
fn owned_slots(body: &[S]) -> std::collections::BTreeSet<usize> {
    let mut slots = std::collections::BTreeSet::new();
    for statement in body {
        match statement {
            S::Bind { slot, .. } => {
                slots.insert(*slot);
            }
            S::If { yes, no, .. } => {
                slots.extend(owned_slots(yes));
                slots.extend(owned_slots(no));
            }
            _ => {}
        }
    }
    slots
}
fn statements(body: &[S], out: &mut String) -> Result<(), String> {
    for s in body {
        match s {
            S::Bind { slot, value } => out.push_str(&format!("s{slot}={};\n", expr(value)?)),
            S::Evaluate { value } => out.push_str(&format!("let _={};\n", expr(value)?)),
            S::If { condition, yes, no } => {
                out.push_str(&format!("if ({}).truthy() {{\n", expr(condition)?));
                statements(yes, out)?;
                out.push_str("} else {\n");
                statements(no, out)?;
                out.push_str("}\n");
            }
            S::Fatal { metric, message } => out.push_str(&format!(
                "return Err(ValidationError::fatal({},{}));\n",
                expr(metric)?,
                rust_lit(message)
            )),
            S::Return { value } => out.push_str(&format!("return Ok({});\n", expr(value)?)),
        }
    }
    Ok(())
}
fn expr(e: &E) -> Result<String, String> {
    Ok(match e {
        E::Argument { index: 3 } => "item.clone()".into(),
        E::Argument { index } => format!("a{index}.clone()"),
        E::Slot { index } => format!("s{index}.clone()"),
        E::Undefined => "Value::Undefined".into(),
        E::Null => "Value::Null".into(),
        E::Integer { value } => format!("Value::Number({value}_f64)"),
        E::String { value } | E::Metric { name: value } => {
            format!("Value::Text({}.into())", rust_lit(value))
        }
        E::Field { object, field } => format!("({}).field({})?", expr(object)?, rust_lit(field)),
        E::Not { value } => format!("Value::Bool(!({}).truthy())", expr(value)?),
        E::Nullish { value, negate } => format!(
            "Value::Bool({}({}).nullish())",
            if *negate { "!" } else { "" },
            expr(value)?
        ),
        E::And { left, right } => format!(
            "{{let left={};if left.truthy(){{{}}}else{{left}}}}",
            expr(left)?,
            expr(right)?
        ),
        E::Or { left, right } => format!(
            "{{let left={};if left.truthy(){{left}}else{{{}}}}}",
            expr(left)?,
            expr(right)?
        ),
        E::Greater { left, right } => format!(
            "Value::Bool(({}).number()? > ({}).number()?)",
            expr(left)?,
            expr(right)?
        ),
        E::Choose { condition, yes, no } => format!(
            "if ({}).truthy() {{{}}} else {{{}}}",
            expr(condition)?,
            expr(yes)?,
            expr(no)?
        ),
        E::Assign { slot, value } => format!("{{s{slot}={};s{slot}.clone()}}", expr(value)?),
        E::Sequence { values } => {
            let mut v = String::from("{");
            for (i, x) in values.iter().enumerate() {
                if i + 1 < values.len() {
                    v.push_str(&format!("let _={};", expr(x)?));
                } else {
                    v.push_str(&expr(x)?);
                }
            }
            v.push('}');
            v
        }
        E::Object { fields } => format!(
            "Value::Object(vec![{}])",
            fields
                .iter()
                .map(|(k, v)| Ok(format!("({}, {})", rust_lit(k), expr(v)?)))
                .collect::<Result<Vec<_>, String>>()?
                .join(",")
        ),
        E::Validate {
            validator,
            arguments,
        } => format!(
            "{}(transforms,{})?",
            snake_case(validator),
            arguments
                .iter()
                .map(expr)
                .collect::<Result<Vec<_>, _>>()?
                .join(",")
        ),
        E::Map { input, body } => {
            let mut rendered = String::new();
            for i in owned_slots(body) {
                rendered.push_str(&format!(
                    "#[allow(unused_assignments,unused_mut)] let mut s{i}=Value::Undefined;"
                ));
            }
            statements(body, &mut rendered)?;
            format!(
                "{{let input={};let mut mapped=Vec::new();for item in input.array()?{{mapped.push((||->Result<Value,ValidationError>{{{rendered}}})()?);}}Value::Array(mapped)}}",
                expr(input)?
            )
        }
        E::Transform {
            transform,
            arguments,
        } => {
            if arguments.len() != 1 {
                return Err("transform arity".into());
            }
            let value = expr(&arguments[0])?;
            match transform {
                T::SafeInteger => format!("transforms.number_or_throw_if_too_large({value})?"),
                T::SyncKeyId => format!("transforms.to_sync_key_id({value})?"),
                T::DecodePatchDebugData => format!("transforms.decode_patch_debug_data({value})?"),
                T::DecodeSyncActionData => format!("transforms.decode_sync_action_data({value})?"),
                T::MaybeNumber => {
                    format!("transforms.maybe_number_or_throw_if_too_large({value})?")
                }
            }
        }
    })
}

pub fn generate_appstate_batch_validation(
    ir: &wa_ir::AppstateBatchValidationIr,
) -> Result<String, String> {
    if ir.format_version != 1 || ir.recipe != wa_ir::BatchValidationRecipe::SeparateSetRemoveV1 {
        return Err("unsupported batch validation recipe".into());
    }
    let mut functions = std::collections::BTreeMap::new();
    functions.insert("validateAndTypeSetMutations".into(), ir.type_set.clone());
    let typed = AppstateValidationIr {
        format_version: 1,
        wa_version: ir.wa_version.clone(),
        module_sha256: ir.module_sha256.clone(),
        functions,
    };
    let mut result = include_str!("appstate_validation_batch.rs.txt")
        .replace("@SET@", &ir.set_operation.to_string())
        .replace("@REMOVE@", &ir.remove_operation.to_string());
    // Both programs share the same native data view.
    result.push_str(&generate_appstate_validation(&typed)?.replace(
        "use super::validation_value::{Value, ValidationError, Transforms};",
        "use super::validation_value::Transforms;",
    ));
    Ok(result)
}
