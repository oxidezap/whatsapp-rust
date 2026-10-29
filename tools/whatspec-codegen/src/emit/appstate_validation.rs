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
        qualify_native_comparisons(name, &f.body)?;
        out.push_str(&format!("#[allow(unused_variables)]\n#[inline(never)]\npub(super) fn {}<'a>(transforms: &mut impl Transforms<'a>, {})->Result<Value<'a>,Box<ValidationError>>{{\n",snake_case(name),(0..f.parameters).map(|i|format!("a{i}:&Value<'a>")).collect::<Vec<_>>().join(",")));
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
                out.push_str(&format!("if ({}).truthy() {{\n", receiver(condition)?));
                statements(yes, out)?;
                out.push_str("} else {\n");
                statements(no, out)?;
                out.push_str("}\n");
            }
            S::Fatal { metric, message } => out.push_str(&format!(
                "return Err(ValidationError::fatal_boxed({},{}));\n",
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
        E::Field { object, field } => {
            let receiver = receiver(object)?;
            format!("({receiver}).field({})?", rust_lit(field))
        }
        E::Not { value } => format!("Value::Bool(!({}).truthy())", receiver(value)?),
        E::Nullish { value, negate } => format!(
            "Value::Bool({}({}).nullish())",
            if *negate { "!" } else { "" },
            receiver(value)?
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
            "{{let left={};let right={};Value::Bool(transforms.relational_greater(left,right)?)}}",
            expr(left)?,
            expr(right)?
        ),
        E::Choose { condition, yes, no } => format!(
            "if ({}).truthy() {{{}}} else {{{}}}",
            receiver(condition)?,
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
            "Value::object(vec![{}])",
            fields
                .iter()
                .map(|(k, v)| Ok(format!("({}, {})", rust_lit(k), expr(v)?)))
                .collect::<Result<Vec<_>, String>>()?
                .join(",")
        ),
        E::Validate {
            validator,
            arguments,
        } => validation_call(validator, arguments)?,
        E::Map { input, body } => {
            let mut rendered = String::new();
            for i in owned_slots(body) {
                rendered.push_str(&format!(
                    "#[allow(unused_assignments,unused_mut)] let mut s{i}=Value::Undefined;"
                ));
            }
            statements(body, &mut rendered)?;
            format!(
                "{{let input={};let mut mapped=Vec::new();for item in input.array()?{{mapped.push((||->Result<Value,Box<ValidationError>>{{{rendered}}})()?);}}Value::list(mapped)}}",
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

fn validation_call(validator: &str, arguments: &[E]) -> Result<String, String> {
    let mut rendered = String::from("{");
    let mut passed = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        let local = format!("call_arg_{index}");
        if let Some(borrowed) = borrowed_call_argument(argument, &arguments[index + 1..]) {
            rendered.push_str(&format!("let {local}={borrowed};"));
            passed.push(local);
        } else {
            rendered.push_str(&format!("let {local}={};", expr(argument)?));
            passed.push(format!("&{local}"));
        }
    }
    rendered.push_str(&format!(
        "{}(transforms,{})?}}",
        snake_case(validator),
        passed.join(",")
    ));
    Ok(rendered)
}

// Arguments are immutable, while slots can be reassigned by subsequent argument
// evaluation. Snapshot those slots before later effects, as JavaScript does.
fn borrowed_call_argument(argument: &E, later: &[E]) -> Option<String> {
    match argument {
        E::Argument { index: 3 } => Some("item".into()),
        E::Argument { index } => Some(format!("a{index}")),
        E::Slot { index } => {
            let mut writes = std::collections::BTreeMap::new();
            let mut expressions = Vec::new();
            for argument in later {
                collect_comparison_expression(argument, &mut writes, &mut expressions);
            }
            (!writes.contains_key(index)).then(|| format!("&s{index}"))
        }
        _ => None,
    }
}

// Tests and field reads only borrow a slot/argument. Other expressions still
// evaluate to owned values, preserving their assignments and dependency calls.
fn receiver(e: &E) -> Result<String, String> {
    match e {
        E::Argument { index: 3 } => Ok("item".into()),
        E::Argument { index } => Ok(format!("a{index}")),
        E::Slot { index } => Ok(format!("s{index}")),
        _ => expr(e),
    }
}

// The native host accepts buffa protobufs, whose repeated fields are ordinary
// arrays. Qualify every comparator before letting that host omit JavaScript
// coercion. The reference host keeps the complete comparator for arbitrary
// differential inputs. These are adapter input types, not validation rules.
fn qualify_native_comparisons(name: &str, body: &[S]) -> Result<(), String> {
    let mut bindings = std::collections::BTreeMap::<usize, Vec<&E>>::new();
    let mut expressions = Vec::new();
    collect_comparison_inputs(body, &mut bindings, &mut expressions);
    // These argument types come from the native entry adapters. A generated
    // call could supply a different Value, so it needs a separate proof.
    if expressions.iter().any(|expression| matches!(expression,
        E::Validate { validator, .. }
            if matches!(validator.as_str(), "validatePatchProtobuf" | "validateSnapshotProtobuf")
    )) {
        return Err(format!("native typed-array entry called internally in {name}"));
    }
    let mut arrays = std::collections::BTreeSet::new();
    for statement in body {
        let mut local_expressions = Vec::new();
        collect_comparison_inputs(
            std::slice::from_ref(statement),
            &mut std::collections::BTreeMap::new(),
            &mut local_expressions,
        );
        for expression in local_expressions {
            if let E::Greater { left, right } = expression
                && (!numeric_operand(name, left, &arrays) || !numeric_operand(name, right, &arrays))
            {
                return Err(format!("unqualified native numeric comparison in {name}"));
            }
        }
        // Only preceding, unconditional and uniquely assigned bindings prove
        // an alias. Branch/callback-local bindings cannot establish dominance.
        if let S::Bind { slot, value } = statement
            && bindings.get(slot).is_some_and(|values| values.len() == 1)
            && ordinary_array(name, value, &arrays)
        {
            arrays.insert(*slot);
        }
    }
    Ok(())
}

fn numeric_operand(name: &str, value: &E, arrays: &std::collections::BTreeSet<usize>) -> bool {
    match value {
        E::Integer { .. } => true,
        E::Field { object, field } if field == "length" => ordinary_array(name, object, arrays),
        _ => false,
    }
}

fn ordinary_array(name: &str, value: &E, arrays: &std::collections::BTreeSet<usize>) -> bool {
    match value {
        E::Field { object, field } if matches!(object.as_ref(), E::Argument { index: 1 }) => {
            matches!(
                (name, field.as_str()),
                ("validatePatchProtobuf", "mutations") | ("validateSnapshotProtobuf", "records")
            )
        }
        E::Map { .. } => true,
        E::Slot { index } => arrays.contains(index),
        _ => false,
    }
}

fn collect_comparison_inputs<'a>(
    body: &'a [S],
    bindings: &mut std::collections::BTreeMap<usize, Vec<&'a E>>,
    expressions: &mut Vec<&'a E>,
) {
    for statement in body {
        match statement {
            S::Bind { slot, value } => {
                bindings.entry(*slot).or_default().push(value);
                collect_comparison_expression(value, bindings, expressions);
            }
            S::If { condition, yes, no } => {
                collect_comparison_expression(condition, bindings, expressions);
                collect_comparison_inputs(yes, bindings, expressions);
                collect_comparison_inputs(no, bindings, expressions);
            }
            S::Evaluate { value } | S::Return { value } => {
                collect_comparison_expression(value, bindings, expressions);
            }
            S::Fatal { metric, .. } => {
                collect_comparison_expression(metric, bindings, expressions);
            }
        }
    }
}

fn collect_comparison_expression<'a>(
    value: &'a E,
    bindings: &mut std::collections::BTreeMap<usize, Vec<&'a E>>,
    expressions: &mut Vec<&'a E>,
) {
    expressions.push(value);
    match value {
        E::Field { object: value, .. } | E::Not { value } | E::Nullish { value, .. } => {
            collect_comparison_expression(value, bindings, expressions);
        }
        E::Assign { slot, value } => {
            bindings.entry(*slot).or_default().push(value);
            collect_comparison_expression(value, bindings, expressions);
        }
        E::And { left, right } | E::Or { left, right } | E::Greater { left, right } => {
            collect_comparison_expression(left, bindings, expressions);
            collect_comparison_expression(right, bindings, expressions);
        }
        E::Choose { condition, yes, no } => {
            for value in [condition, yes, no] {
                collect_comparison_expression(value, bindings, expressions);
            }
        }
        E::Sequence { values }
        | E::Validate {
            arguments: values, ..
        }
        | E::Transform {
            arguments: values, ..
        } => {
            for value in values {
                collect_comparison_expression(value, bindings, expressions);
            }
        }
        E::Object { fields } => {
            for (_, value) in fields {
                collect_comparison_expression(value, bindings, expressions);
            }
        }
        E::Map { input, body } => {
            collect_comparison_expression(input, bindings, expressions);
            // Counting callback-local writes conservatively rejects ambiguous
            // slot aliases rather than accidentally proving a shadowed slot.
            collect_comparison_inputs(body, bindings, expressions);
        }
        E::Argument { .. }
        | E::Slot { .. }
        | E::Undefined
        | E::Null
        | E::Integer { .. }
        | E::String { .. }
        | E::Metric { .. } => {}
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_calls_preserve_argument_snapshots_and_effect_order() {
        let function = |parameters, body| wa_ir::ValidationFunction {
            module: "synthetic argument semantics".into(),
            start_byte: 0,
            end_byte: 1,
            parameters,
            body,
        };
        let invoke = |arguments| E::Validate {
            validator: "echoArguments".into(),
            arguments,
        };
        let program = AppstateValidationIr {
            format_version: 1,
            wa_version: "synthetic".into(),
            module_sha256: "0".repeat(64),
            functions: std::collections::BTreeMap::from([
                (
                    "echoArguments".into(),
                    function(
                        2,
                        vec![S::Return {
                            value: E::Object {
                                fields: vec![
                                    ("first".into(), E::Argument { index: 0 }),
                                    ("second".into(), E::Argument { index: 1 }),
                                ],
                            },
                        }],
                    ),
                ),
                (
                    "captureProbe".into(),
                    function(
                        0,
                        vec![
                            S::Bind {
                                slot: 0,
                                value: E::Integer { value: 1 },
                            },
                            S::Return {
                                value: invoke(vec![
                                    E::Slot { index: 0 },
                                    E::Assign {
                                        slot: 0,
                                        value: Box::new(E::Integer { value: 2 }),
                                    },
                                ]),
                            },
                        ],
                    ),
                ),
                (
                    "effectProbe".into(),
                    function(
                        0,
                        vec![S::Return {
                            value: invoke(
                                [1, 2]
                                    .into_iter()
                                    .map(|value| E::Transform {
                                        transform: T::SafeInteger,
                                        arguments: vec![E::Integer { value }],
                                    })
                                    .collect(),
                            ),
                        }],
                    ),
                ),
            ]),
        };
        let generated = generate_appstate_validation(&program).unwrap();
        let source = format!(
            "#![allow(dead_code,unused_variables,unused_assignments,unused_mut,unused_parens)]\nmod validation_value {{\n{}\n}}\nmod generated {{\n{generated}\n}}\n{}",
            include_str!("appstate_validation_value.rs.txt"),
            r#"
use validation_value::{Transforms, ValidationError, Value};
#[derive(Default)]
struct Host { trace: Vec<f64> }
impl<'a> Transforms<'a> for Host {
    fn number_or_throw_if_too_large(&mut self, value: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> {
        self.trace.push(value.number()?);
        Ok(value)
    }
    fn to_sync_key_id(&mut self, value: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> { Ok(value) }
    fn decode_sync_action_data(&mut self, value: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> { Ok(value) }
    fn decode_patch_debug_data(&mut self, value: Value<'a>) -> Result<Value<'a>, Box<ValidationError>> { Ok(value) }
}
fn assert_fields(result: Value<'_>) {
    assert_eq!(result.field("first").expect("first field").number().expect("first number"), 1.0);
    assert_eq!(result.field("second").expect("second field").number().expect("second number"), 2.0);
}
fn main() {
    assert!(std::mem::size_of::<Result<Value<'_>, Box<ValidationError>>>()
        < std::mem::size_of::<Result<Value<'_>, ValidationError>>());
    let boxed = ValidationError::fatal_boxed(Value::Text("metric".into()), "failure");
    let metric_address = boxed.metric.as_ref().expect("metric").as_ptr();
    let message_address = boxed.message.as_ptr();
    let error = ValidationError::from(boxed);
    assert_eq!(error.class, "SyncdFatalError");
    assert_eq!(error.metric.as_deref(), Some("metric"));
    assert_eq!(error.message, "failure");
    assert_eq!(error.metric.as_ref().expect("metric").as_ptr(), metric_address);
    assert_eq!(error.message.as_ptr(), message_address);
    let mut host = Host::default();
    assert_fields(generated::capture_probe(&mut host).expect("argument snapshot"));
    assert!(host.trace.is_empty());
    assert_fields(generated::effect_probe(&mut host).expect("ordered transform arguments"));
    assert_eq!(host.trace, vec![1.0, 2.0]);
}
"#
        );
        run_generated_program(&source);
    }

    fn run_generated_program(source: &str) {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        struct RemoveOnDrop(std::path::PathBuf);
        impl Drop for RemoveOnDrop {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = std::env::temp_dir().join(format!(
            "appstate-generated-semantics-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let cleanup = RemoveOnDrop(directory);
        let input = cleanup.0.join("probe.rs");
        let executable = cleanup
            .0
            .join(format!("probe{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&input, source).unwrap();
        let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
        let compiled = std::process::Command::new(compiler)
            .arg("--edition=2024")
            .arg("--crate-name=appstate_generated_semantics")
            .arg(&input)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "generated program did not compile:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = std::process::Command::new(&executable).output().unwrap();
        assert!(
            executed.status.success(),
            "generated argument semantics failed:\n{}",
            String::from_utf8_lossy(&executed.stderr)
        );
    }

    #[test]
    fn borrowed_arguments_detect_nested_callback_writes() {
        let later = E::Map {
            input: Box::new(E::Argument { index: 1 }),
            body: vec![S::Return {
                value: E::Assign {
                    slot: 2,
                    value: Box::new(E::Null),
                },
            }],
        };
        assert!(borrowed_call_argument(&E::Slot { index: 2 }, &[later]).is_none());
        assert_eq!(
            borrowed_call_argument(&E::Slot { index: 2 }, &[]),
            Some("&s2".into())
        );
        assert_eq!(
            borrowed_call_argument(&E::Argument { index: 3 }, &[]),
            Some("item".into())
        );
    }

    fn array() -> E {
        E::Field {
            object: Box::new(E::Argument { index: 1 }),
            field: "mutations".into(),
        }
    }
    fn binding(slot: usize, value: E) -> S {
        S::Bind { slot, value }
    }
    fn comparison(slot: usize) -> S {
        S::Evaluate {
            value: E::Greater {
                left: Box::new(E::Field {
                    object: Box::new(E::Slot { index: slot }),
                    field: "length".into(),
                }),
                right: Box::new(E::Integer { value: 0 }),
            },
        }
    }
    fn generate(body: Vec<S>) -> Result<String, String> {
        let ir = AppstateValidationIr {
            format_version: 1,
            wa_version: "synthetic".into(),
            module_sha256: "synthetic".into(),
            functions: std::collections::BTreeMap::from([(
                "validatePatchProtobuf".into(),
                wa_ir::ValidationFunction {
                    module: "synthetic".into(),
                    start_byte: 0,
                    end_byte: 1,
                    parameters: 2,
                    body,
                },
            )]),
        };
        generate_appstate_validation(&ir)
    }

    #[test]
    fn generation_rejects_string_and_bytes_lengths_and_callback_rebinding() {
        for value in [
            E::String {
                value: "synthetic".into(),
            },
            E::Field {
                object: Box::new(E::Argument { index: 1 }),
                field: "patchMac".into(),
            },
        ] {
            assert!(generate(vec![binding(0, value), comparison(0)]).is_err());
        }
        assert!(
            generate(vec![
                binding(0, array()),
                S::Evaluate {
                    value: E::Map {
                        input: Box::new(array()),
                        body: vec![
                            binding(0, E::Null),
                            comparison(0),
                            S::Return { value: E::Null }
                        ],
                    }
                },
            ])
            .is_err()
        );
    }

    #[test]
    fn generation_requires_dominance_for_comparisons_inside_branches() {
        let branch = |body| S::If {
            condition: E::Integer { value: 1 },
            yes: body,
            no: vec![],
        };
        assert!(
            generate(vec![
                binding(0, array()),
                branch(vec![comparison(0)]),
                S::Return { value: E::Null },
            ])
            .is_ok()
        );
        assert!(generate(vec![branch(vec![binding(0, array()), comparison(0)])]).is_err());
        assert!(
            generate(vec![
                binding(0, array()),
                branch(vec![binding(0, E::Null), comparison(0)]),
            ])
            .is_err()
        );
    }

    #[test]
    fn native_comparison_proof_requires_typed_array_and_dominating_unique_aliases() {
        let qualify = |body: &[S]| qualify_native_comparisons("validatePatchProtobuf", body);
        assert!(qualify(&[binding(0, array()), comparison(0)]).is_ok());
        assert!(
            qualify(&[
                binding(0, array()),
                binding(1, E::Slot { index: 0 }),
                comparison(1),
            ])
            .is_ok()
        );
        assert!(qualify(&[comparison(0), binding(0, array())]).is_err());
        assert!(
            qualify(&[
                binding(0, E::Slot { index: 1 }),
                binding(1, array()),
                comparison(0),
            ])
            .is_err()
        );
        assert!(qualify(&[binding(0, array()), comparison(0), binding(0, E::Null),]).is_err());
        assert!(
            qualify(&[
                S::If {
                    condition: E::Integer { value: 1 },
                    yes: vec![binding(0, array())],
                    no: vec![],
                },
                comparison(0),
            ])
            .is_err()
        );
        assert!(
            qualify(&[
                binding(0, array()),
                S::Evaluate {
                    value: E::Assign {
                        slot: 0,
                        value: Box::new(E::Null)
                    }
                },
                comparison(0),
            ])
            .is_err()
        );
        assert!(
            qualify(&[
                binding(
                    0,
                    E::Field {
                        object: Box::new(E::Argument { index: 1 }),
                        field: "unknown".into(),
                    }
                ),
                comparison(0),
            ])
            .is_err()
        );
        assert!(
            qualify_native_comparisons("unknown", &[binding(0, array()), comparison(0)]).is_err()
        );
        assert!(
            qualify(&[S::Evaluate {
                value: E::Validate {
                    validator: "validatePatchProtobuf".into(),
                    arguments: vec![E::Null, E::Null],
                }
            }])
            .is_err()
        );
    }

    #[test]
    fn locked_program_qualifies() {
        let ir: AppstateValidationIr = serde_json::from_str(include_str!(
            "../../fixtures/appstate-validation/validation.json"
        ))
        .unwrap();
        generate_appstate_validation(&ir).unwrap();
    }
}
