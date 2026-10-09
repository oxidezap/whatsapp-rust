//! Bounded consumer of the two qualified IQ request/result contracts.
//!
//! Only argument bindings and public-result mappings belong downstream. Wire
//! names and ordered presence gates come from IR. Correlation and error handling
//! remain in the existing request layer; this is not the upstream full RPC parser.

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value;

use super::{header, rust_str};
use crate::naming::{ensure_ident, snake_case};

const PILOTS: &[&str] = &["SetSubject", "AcceptGroupAdd"];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    namespace: String,
    iq_type: String,
    target: String,
    target_arg_path: Vec<Argument>,
    children: Vec<Child>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Argument {
    key: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Child {
    tag: String,
    attrs: Vec<Attribute>,
    children: Vec<Child>,
    content: Option<Content>,
    repeats: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Attribute {
    name: String,
    kind: String,
    required: bool,
    arg_path: Vec<Argument>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Content {
    kind: String,
    arg_path: Vec<Argument>,
}

fn argument(path: &[Argument]) -> Result<String> {
    let [arg] = path else {
        bail!("pilot argument must be a single property")
    };
    Ok(ensure_ident(&snake_case(&arg.key)))
}

pub fn generate(json: &str, wa_version: &str) -> Result<String> {
    let ir: Value = serde_json::from_str(json)?;
    let stanzas = ir["stanzas"].as_array().context("missing IQ stanzas")?;
    let mut out = header("private IQ pilot builders and result payloads", wa_version);
    out.push_str("//! Envelope correlation and error conversion stay in the request layer.\n\nuse crate::request::InfoQuery;\nuse wacore_binary::{Jid, NodeContent, NodeRef, builder::NodeBuilder};\n\n");
    for pilot in PILOTS {
        let module = format!("WASmaxOutGroups{pilot}Request");
        let matches: Vec<_> = stanzas
            .iter()
            .filter(|s| s["moduleName"] == module)
            .collect();
        ensure!(matches.len() == 1, "expected exactly one {module}");
        let op = matches[0];
        let req: Request = serde_json::from_value(op["request"].clone())
            .with_context(|| format!("unsupported request contract for {module}"))?;
        ensure!(
            req.target == "group_jid" && req.iq_type == "set",
            "unsupported pilot target/type"
        );
        ensure!(
            op["namespace"] == req.namespace
                && op["iqType"] == req.iq_type
                && op["target"] == req.target,
            "outer/request IQ disagreement"
        );
        let [child] = req.children.as_slice() else {
            bail!("pilot must have one request child")
        };
        ensure!(
            !child.repeats && child.children.is_empty(),
            "unsupported pilot child shape"
        );
        let target = argument(&req.target_arg_path)?;
        let mut params = vec![format!("pub(super) {target}: &'a Jid")];
        let mut bindings = vec![target.clone()];
        let mut builder = format!("NodeBuilder::new({})", rust_str(&child.tag));
        for attr in &child.attrs {
            ensure!(
                attr.required,
                "optional pilot attributes need explicit admission"
            );
            let arg = argument(&attr.arg_path)?;
            let ty = match attr.kind.as_str() {
                "string" => "&'a str",
                "integer" => "i64",
                "user_jid" => "&'a Jid",
                other => bail!("unsupported pilot attribute kind {other}"),
            };
            ensure!(!bindings.contains(&arg), "duplicate pilot argument {arg}");
            bindings.push(arg.clone());
            params.push(format!("pub(super) {arg}: {ty}"));
            builder.push_str(&format!(".attr({}, {arg})", rust_str(&attr.name)));
        }
        if let Some(content) = &child.content {
            ensure!(content.kind == "dynamic", "unsupported pilot content");
            let arg = argument(&content.arg_path)?;
            // The public subject API supplies UTF-8 text. Keep NodeContent::String
            // and its wire encoding, without a temporary byte-vector conversion.
            ensure!(!bindings.contains(&arg), "duplicate pilot argument {arg}");
            bindings.push(arg.clone());
            params.push(format!("pub(super) {arg}: &'a str"));
            builder.push_str(&format!(".string_content({arg})"));
        }
        let name = snake_case(pilot);
        // Named borrowed inputs bind the SDK adapter to argPath identities.
        // Positional arguments would silently keep the old mapping after a
        // same-typed source argument rename or reorder.
        out.push_str(&format!("pub(super) struct {pilot}Request<'a> {{\n{}\n}}\n\npub(super) fn build_{name}(request: {pilot}Request<'_>) -> InfoQuery<'static> {{\nlet {pilot}Request {{ {} }} = request;\nInfoQuery::set_ref({}, {target}, Some(NodeContent::Nodes(vec![{builder}.build()])))\n}}\n\n", params.join(",\n"), bindings.join(", "), rust_str(&req.namespace)));
        emit_success(op, pilot, &name, &mut out)?;
    }
    Ok(out)
}

fn emit_success(op: &Value, pilot: &str, name: &str, out: &mut String) -> Result<()> {
    let response = &op["response"];
    ensure!(
        response["assertions"] == serde_json::json!([]),
        "new outer response guard needs review"
    );
    let variants = response["variants"]
        .as_array()
        .context("missing pilot outcomes")?;
    let mut cases = Vec::new();
    let mut seen_error = false;
    for variant in variants {
        match variant["kind"].as_str() {
            Some("success") => {}
            Some("error" | "client_error" | "server_error") => {
                seen_error = true;
                continue;
            }
            _ => bail!("unsupported pilot outcome kind: {}", variant["kind"]),
        }
        ensure!(
            !seen_error,
            "success after an error requires full RPC dispatch"
        );
        ensure!(
            variant["fields"]
                == serde_json::json!([{
                    "method":"attrString", "name":"type", "wireName":"type", "type":"string",
                    "parserRequired":true, "literalValue":"result"
                }]),
            "new success payload requires a public-result mapping"
        );
        let tag = variant["tag"].as_str().context("missing success name")?;
        let case = tag
            .strip_prefix(&format!("{pilot}Response"))
            .context("unexpected outcome name")?;
        let assertions = variant["assertions"]
            .as_array()
            .context("missing success guards")?;
        let mut children = Vec::new();
        let mut envelope = Vec::new();
        for assertion in assertions {
            if assertion["kind"] == "child" {
                ensure!(
                    assertion.as_object().is_some_and(|a| a.len() == 2),
                    "unsupported child guard"
                );
                children.push(
                    assertion["name"]
                        .as_str()
                        .context("child guard has no name")?,
                );
            } else {
                envelope.push(assertion.clone());
            }
        }
        // Deliberate boundary: IqSpec is called after the runtime's result/error
        // dispatch and has no request ID. Never derive request context from a reply.
        ensure!(
            envelope
                == serde_json::json!([
                    {"kind":"tag","name":"iq"},
                    {"kind":"reference","name":"id","referencePath":["id"]},
                    {"kind":"reference","name":"from","referencePath":["to"]},
                    {"kind":"attr","name":"type","value":"result"}
                ])
                .as_array()
                .context("envelope array")?
                .as_slice(),
            "pilot envelope changed; review the request-layer boundary"
        );
        ensure!(
            cases
                .iter()
                .all(|(_, gates): &(&str, Vec<&str>)| !gates.is_empty()),
            "unreachable success after bare outcome"
        );
        cases.push((case, children));
    }
    ensure!(
        cases.last().is_some_and(|(_, gates)| gates.is_empty()),
        "pilot has no bare success fallback"
    );
    out.push_str(&format!(
        "#[derive(Debug, PartialEq, Eq)]\npub(super) enum {pilot}Success {{\n"
    ));
    for (case, _) in &cases {
        out.push_str(&format!("{case},\n"));
    }
    out.push_str("}\n\n");
    let param = if cases.iter().any(|(_, gates)| !gates.is_empty()) {
        "response"
    } else {
        "_response"
    };
    out.push_str(&format!(
        "pub(super) fn parse_{name}_payload({param}: &NodeRef<'_>) -> {pilot}Success {{\n"
    ));
    for (case, gates) in &cases {
        if gates.is_empty() {
            out.push_str(&format!("{pilot}Success::{case}\n"));
        } else {
            let guard = gates
                .iter()
                .map(|g| {
                    format!(
                        "response.get_children_by_tag({}).take(2).count() == 1",
                        rust_str(g)
                    )
                })
                .collect::<Vec<_>>()
                .join(" && ");
            out.push_str(&format!(
                "if {guard} {{ return {pilot}Success::{case}; }}\n"
            ));
        }
    }
    out.push_str("}\n\n");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> Value {
        let ops: Vec<_> = PILOTS.iter().map(|pilot| {
            let assertions = json!([
                {"kind":"tag","name":"iq"},
                {"kind":"reference","name":"id","referencePath":["id"]},
                {"kind":"reference","name":"from","referencePath":["to"]},
                {"kind":"attr","name":"type","value":"result"}
            ]);
            let fields = json!([{"method":"attrString","name":"type","wireName":"type","type":"string","parserRequired":true,"literalValue":"result"}]);
            let bare = json!({"tag":format!("{pilot}ResponseSuccess"),"kind":"success","assertions":assertions,"fields":fields});
            let mut gated = bare.clone();
            gated["tag"] = json!(format!("{pilot}ResponseConditionalSuccess"));
            gated["assertions"].as_array_mut().unwrap().push(json!({"kind":"child","name":"source_gate"}));
            json!({
                "moduleName":format!("WASmaxOutGroups{pilot}Request"),
                "namespace":"test:wire", "iqType":"set", "target":"group_jid",
                "request": {"namespace":"test:wire", "iqType":"set", "target":"group_jid",
                    "targetArgPath":[{"key":"destination"}],
                    "children":[{"tag":"source_tag","attrs":[{"name":"source_attr","kind":"integer","required":true,"argPath":[{"key":"inputValue"}]}],"children":[],"repeats":false}]},
                "response":{"assertions":[],"variants":[gated,bare]}
            })
        }).collect();
        json!({"stanzas":ops})
    }

    fn emit(v: &Value) -> Result<String> {
        generate(&v.to_string(), "test")
    }

    #[test]
    fn wire_names_argument_bindings_and_order_are_derived() {
        let input = fixture();
        let output = emit(&input).unwrap();
        assert_eq!(output, emit(&input).unwrap());
        assert!(output.contains("pub(super) destination: &'a Jid"));
        assert!(output.contains("pub(super) input_value: i64"));
        assert!(output.contains("InfoQuery::set_ref(\"test:wire\", destination"));
        assert!(
            output.contains("NodeBuilder::new(\"source_tag\").attr(\"source_attr\", input_value)")
        );
        assert!(output.contains("get_children_by_tag(\"source_gate\").take(2).count() == 1"));
        assert!(!output.contains("membership_approval_request"));
        assert!(!output.contains("RESPONSE_GENERATION_ERROR"));
    }

    #[test]
    fn missing_duplicate_and_shadowed_contracts_fail_generation() {
        let mut v = fixture();
        v["stanzas"].as_array_mut().unwrap().pop();
        assert!(emit(&v).is_err());
        let mut v = fixture();
        let duplicate = v["stanzas"][0].clone();
        v["stanzas"].as_array_mut().unwrap().push(duplicate);
        assert!(emit(&v).is_err());
        let mut v = fixture();
        v["stanzas"][0]["response"]["variants"]
            .as_array_mut()
            .unwrap()
            .reverse();
        assert!(emit(&v).is_err());
    }

    #[test]
    fn unmodeled_request_contracts_fail_generation() {
        for (pointer, replacement) in [
            ("/request/children/0/repeats", json!(true)),
            ("/request/children/0/optional", json!(true)),
            ("/request/children/0/attrs/0/required", json!(false)),
            ("/request/children/0/attrs/0/kind", json!("future")),
            ("/request/children/0/attrs/0/argPath", json!([])),
            (
                "/request/children/0/attrs/0/argPath",
                json!([{"key":"destination"}]),
            ),
            ("/request/target", json!("server")),
        ] {
            let mut v = fixture();
            let op = &mut v["stanzas"][0];
            if pointer.ends_with("/optional") {
                op["request"]["children"][0]["optional"] = replacement;
            } else {
                *op.pointer_mut(pointer).unwrap() = replacement;
            }
            assert!(emit(&v).is_err(), "accepted {pointer}");
        }
    }

    #[test]
    fn new_payload_or_envelope_guard_requires_review() {
        for (pointer, replacement) in [
            ("/response/variants/0/fields", json!([])),
            (
                "/response/variants/0/assertions/1/referencePath",
                json!(["other"]),
            ),
            (
                "/response/assertions",
                json!([{"kind":"child","name":"new"}]),
            ),
        ] {
            let mut v = fixture();
            *v["stanzas"][0].pointer_mut(pointer).unwrap() = replacement;
            assert!(emit(&v).is_err(), "accepted {pointer}");
        }
    }

    #[test]
    fn only_known_error_kinds_can_be_delegated_to_runtime() {
        let expected = emit(&fixture()).unwrap();
        for kind in ["error", "client_error", "server_error"] {
            let mut v = fixture();
            v["stanzas"][0]["response"]["variants"]
                .as_array_mut()
                .unwrap()
                .push(json!({"kind":kind}));
            assert_eq!(emit(&v).unwrap(), expected);
        }
        for outcome in [
            json!({"kind":"future"}),
            json!({"kind":"conditional_success"}),
            json!({"kind":null}),
            json!({"kind":true}),
            json!({}),
        ] {
            let mut v = fixture();
            v["stanzas"][0]["response"]["variants"]
                .as_array_mut()
                .unwrap()
                .push(outcome);
            assert!(
                emit(&v)
                    .unwrap_err()
                    .to_string()
                    .contains("unsupported pilot outcome kind")
            );
        }
    }

    #[test]
    fn success_dispatch_requires_order_fallback_and_exact_child_guards() {
        let mut v = fixture();
        v["stanzas"][0]["response"]["variants"]
            .as_array_mut()
            .unwrap()
            .insert(1, json!({"kind":"error"}));
        assert!(
            emit(&v)
                .unwrap_err()
                .to_string()
                .contains("success after an error")
        );
        let mut v = fixture();
        v["stanzas"][0]["response"]["variants"]
            .as_array_mut()
            .unwrap()
            .pop();
        assert!(
            emit(&v)
                .unwrap_err()
                .to_string()
                .contains("no bare success fallback")
        );
        let mut v = fixture();
        v["stanzas"][0]["response"]["variants"][0]["assertions"][4]["value"] =
            json!("extra operand");
        assert!(
            emit(&v)
                .unwrap_err()
                .to_string()
                .contains("unsupported child guard")
        );
    }
}
