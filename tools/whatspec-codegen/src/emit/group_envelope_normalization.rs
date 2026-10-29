//! Compile the typed envelope program into static Rust. No program JSON is
//! shipped in the runtime crate; unsupported operands or locals stop codegen.
use anyhow::{Result, bail, ensure};
#[path = "group_envelope_ir.rs"]
mod ir;
use super::{header, rust_str};
use ir::*;

const REV: &str = "ed8da49c24a0e943854e1c7077c60129e169aeb4";
const CONTRACT_SHA: &str = "657e610570e886784cdda871495912926ff2f8caf8369c7c8c086aa0c38aae99";
const CONTRACT: &str = include_str!("../../fixtures/group_envelope.json");
struct Compiler {
    locals: u32,
    next: usize,
}
impl Compiler {
    fn local(&self, local: u32) -> Result<()> {
        ensure!(local < self.locals, "envelope local outside frame");
        Ok(())
    }
    fn temp(&mut self) -> String {
        let value = format!("envelope_tmp_{}", self.next);
        self.next += 1;
        value
    }
    fn expr(&mut self, expr: &GroupEnvelopeExpr) -> Result<String> {
        use GroupEnvelopeExpr as E;
        Ok(match expr {
            E::Node => bail!("raw parser node escapes envelope program"),
            E::Local { local } => {
                self.local(*local)?;
                format!("state.local({local})?")
            }
            E::String { value } => format!("Value::String({}.into())", rust_str(value)),
            E::Integer { value } => {
                ensure!(
                    value.unsigned_abs() <= 9_007_199_254_740_991,
                    "non-exact envelope integer"
                );
                format!("Value::Integer({value})")
            }
            E::Bool { value } => format!("Value::Bool({value})"),
            E::Null => "Value::Null".into(),
            E::Undefined => "Value::Undefined".into(),
            E::Context { fact } => format!("context(context_value, Fact::{fact:?})?"),
            E::Read { method, name } => format!("read(node,Read::{method:?},{})?", rust_str(name)),
            E::Convert { conversion, value } => format!(
                "convert(context_value,Conversion::{conversion:?},{})?",
                self.expr(value)?
            ),
            E::IsEmpty { value } => format!("is_empty({})", self.expr(value)?),
            E::Not { value } => format!("Value::Bool(!({}).truthy())", self.expr(value)?),
            E::Binary {
                operator,
                left,
                right,
            } => {
                let left = self.expr(left)?;
                let right = self.expr(right)?;
                if *operator == GroupEnvelopeBinary::StrictNotEqual {
                    format!("Value::Bool(!binary(Binary::StrictEqual,{left},{right})?.truthy())")
                } else {
                    format!("binary(Binary::{operator:?},{left},{right})?")
                }
            }
            E::Logical {
                operator,
                left,
                right,
            } => {
                let left = self.expr(left)?;
                let right = self.expr(right)?;
                let temp = self.temp();
                match operator {
                    GroupEnvelopeLogical::And => {
                        format!("{{let {temp}={left};if {temp}.truthy(){{{right}}}else{{{temp}}}}}")
                    }
                    GroupEnvelopeLogical::Or => {
                        format!("{{let {temp}={left};if {temp}.truthy(){{{temp}}}else{{{right}}}}}")
                    }
                }
            }
            E::Conditional {
                condition,
                then_value,
                else_value,
            } => format!(
                "if ({}).truthy(){{{}}}else{{{}}}",
                self.expr(condition)?,
                self.expr(then_value)?,
                self.expr(else_value)?
            ),
            E::Assign { local, value } => {
                self.local(*local)?;
                let value = self.expr(value)?;
                let temp = self.temp();
                format!("{{let {temp}={value};state.assign({local},{temp})?}}")
            }
            E::Sequence { values } => {
                let (last, before) = values
                    .split_last()
                    .ok_or_else(|| anyhow::anyhow!("empty envelope sequence"))?;
                let mut out = "{".to_string();
                for expr in before {
                    out.push_str(&format!("let _={};", self.expr(expr)?));
                }
                out.push_str(&self.expr(last)?);
                out.push('}');
                out
            }
            E::Reject { message } => format!("reject({})?", self.expr(message)?),
            E::Object { fields } => {
                let mut keys = std::collections::BTreeSet::new();
                let mut values = vec![];
                for field in fields {
                    ensure!(keys.insert(&field.name), "duplicate envelope output");
                    values.push(format!(
                        "({}, {})",
                        rust_str(&field.name),
                        self.expr(&field.value)?
                    ));
                }
                format!("Value::Object(vec![{}])", values.join(","))
            }
        })
    }
    fn ops(&mut self, ops: &[GroupEnvelopeOp]) -> Result<String> {
        let mut out = String::new();
        for op in ops {
            match op {
                GroupEnvelopeOp::Bind { local, value } => {
                    self.local(*local)?;
                    if let Some(value) = value {
                        let value = self.expr(value)?;
                        let temp = self.temp();
                        out.push_str(&format!(
                            "let {temp}={value};let _=state.assign({local},{temp})?;\n"
                        ));
                    }
                }
                GroupEnvelopeOp::Evaluate { value } => {
                    out.push_str(&format!("let _={};\n", self.expr(value)?))
                }
                GroupEnvelopeOp::If {
                    condition,
                    then_ops,
                    else_ops,
                } => {
                    let condition = self.expr(condition)?;
                    let then_body = self.ops(then_ops)?;
                    let else_body = self.ops(else_ops)?;
                    if then_ops.is_empty() && else_ops.is_empty() {
                        out.push_str(&format!("let _={condition};\n"));
                    } else if else_ops.is_empty() {
                        out.push_str(&format!("if ({condition}).truthy(){{{then_body}}}\n"));
                    } else if then_ops.is_empty() {
                        out.push_str(&format!("if !({condition}).truthy(){{{else_body}}}\n"));
                    } else {
                        out.push_str(&format!(
                            "if ({condition}).truthy(){{{then_body}}}else{{{else_body}}}\n"
                        ));
                    }
                }
                GroupEnvelopeOp::TryDiagnostic {
                    body,
                    message_parts,
                } => {
                    let body = self.ops(body)?;
                    let temp = self.temp();
                    let template = message_parts
                        .iter()
                        .map(|s| rust_str(s))
                        .collect::<Vec<_>>()
                        .join(",");
                    out.push_str(&format!("let {temp}=(||->Result<(),GroupParseError>{{{body}Ok(())}})();if let Err(error)={temp}{{state.diagnostics.push(Diagnostic{{message_parts:&[{template}],error}});}}\n"));
                }
            }
        }
        Ok(out)
    }
}
fn generate_contract(contract: &GroupEnvelopeContract, wa_version: &str) -> Result<String> {
    ensure!(
        wa_version == "2.3000.1047483476",
        "envelope program needs version review"
    );
    ensure!(
        contract.module_name == "WAWebHandleGroupNotification"
            && contract.start_byte < contract.actions.start_byte
            && contract.actions.start_byte < contract.actions.end_byte
            && contract.actions.end_byte < contract.end_byte,
        "invalid envelope source boundaries"
    );
    ensure!(
        contract.locals > 0 && contract.locals <= 64,
        "unsupported envelope frame size"
    );
    ensure!(
        contract.actions.result_local != contract.actions.missing_identification_local,
        "aliased action result and flag"
    );
    ensure!(
        contract.actions.missing_identification_tag == "missing_participant_identification"
            && contract.actions.unknown_action_error_prefix == "Unrecognized group action ",
        "action adapter constants require review"
    );
    let mut compiler = Compiler {
        locals: contract.locals,
        next: 0,
    };
    compiler.local(contract.actions.result_local)?;
    compiler.local(contract.actions.missing_identification_local)?;
    let prefix = compiler.ops(&contract.prefix)?;
    let GroupEnvelopeExpr::Object { fields } = &contract.projection else {
        bail!("envelope projection must be an object");
    };
    let required_fields = std::collections::BTreeSet::from([
        "externalId",
        "chatId",
        "isLidAddressingMode",
        "author",
        "authorPhoneNumber",
        "authorUsername",
        "authorUsernameCountryCode",
        "ts",
        "pushname",
        "offline",
        "actions",
        "hasIncompleteParticipantInformation",
    ]);
    ensure!(
        fields.len() == required_fields.len()
            && fields
                .iter()
                .all(|field| required_fields.contains(field.name.as_str())),
        "public envelope projection requires adapter review"
    );
    ensure!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>()
            == [
                "externalId",
                "chatId",
                "isLidAddressingMode",
                "author",
                "authorPhoneNumber",
                "authorUsername",
                "authorUsernameCountryCode",
                "ts",
                "pushname",
                "offline",
                "actions",
                "hasIncompleteParticipantInformation"
            ],
        "envelope fields/order require typed adapter qualification"
    );
    ensure!(fields.iter().any(|f|f.name=="actions" && matches!(f.value,GroupEnvelopeExpr::Local{local} if local==contract.actions.result_local)),"projection lost actual action result");
    let mut action_fields = vec![];
    for name in ["chatId", "author", "isLidAddressingMode"] {
        let field = fields
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| anyhow::anyhow!("missing action context projection"))?;
        let GroupEnvelopeExpr::Local { local } = &field.value else {
            bail!("action context requires a previously parsed local");
        };
        compiler.local(*local)?;
        action_fields.push(format!("({},state.local({local})?)", rust_str(name)));
    }
    let timestamp_locals = contract
        .prefix
        .iter()
        .filter_map(|op| match op {
            GroupEnvelopeOp::Bind {
                local,
                value:
                    Some(GroupEnvelopeExpr::Read {
                        method: GroupEnvelopeRead::AttrTime,
                        ..
                    }),
            } => Some(*local),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [timestamp_local] = timestamp_locals.as_slice() else {
        bail!("action timestamp must be one unconditional prefix read");
    };
    compiler.local(*timestamp_local)?;
    action_fields.push(format!("(\"timestamp\",state.local({timestamp_local})?)"));
    let projection = compiler.expr(&contract.projection)?;
    let mut out = header("ordered group envelope guards", wa_version);
    out.push_str(&format!("//! Source contract https://github.com/oxidezap/whatspec/blob/{REV}/generated/notif/group-envelope.json\n"));
    out.push_str(&format!("//! {} callback bytes {}..{}; child action boundary {}..{}.\n//! Actions are supplied by the normalized action parser between prefix and projection.\n",contract.module_name,contract.start_byte,contract.end_byte,contract.actions.start_byte,contract.actions.end_byte));
    out.push_str("use super::{State,Value,Diagnostic,Read,Fact,Conversion,Binary,read,context,convert,is_empty,binary,reject};\nuse super::{GroupParseContext,GroupParseError,NodeRef};\n");
    out.push_str(&format!("pub(in crate::stanza::groups) const MISSING_IDENTIFICATION_TAG:&str={};\npub(in crate::stanza::groups) const UNKNOWN_ACTION_ERROR_PREFIX:&str={};\n",rust_str(&contract.actions.missing_identification_tag),rust_str(&contract.actions.unknown_action_error_prefix)));
    out.push_str(&format!("pub(in crate::stanza::groups) fn action_facts(state:&State)->Result<Value,GroupParseError>{{Ok(Value::Object(vec![{}]))}}\n",action_fields.join(",")));

    out.push_str(&format!("pub(in crate::stanza::groups) fn parse_prefix(node:&NodeRef<'_>,context_value:&GroupParseContext<'_>)->Result<State,GroupParseError>{{let mut state=State::new({},{},{});{prefix}Ok(state)}}\n",contract.locals,contract.actions.missing_identification_local,contract.actions.result_local));
    out.push_str(&format!("pub(in crate::stanza::groups) fn finish_projection(node:&NodeRef<'_>,context_value:&GroupParseContext<'_>,state:&mut State)->Result<Value,GroupParseError>{{let _=context_value;state.require_actions()?;Ok({projection})}}\n"));
    Ok(out)
}
pub fn generate(wa_version: &str) -> Result<String> {
    ensure!(
        crate::source::digest(CONTRACT.as_bytes()) == format!("sha256:{CONTRACT_SHA}"),
        "envelope contract differs from independent pin"
    );
    let contract: GroupEnvelopeContract = serde_json::from_str(CONTRACT)?;
    generate_contract(&contract, wa_version)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn current_program_has_lazy_context_and_actual_action_locals() {
        let out = generate("2.3000.1047483476").unwrap();
        assert!(out.contains("Fact::MeDeviceLid"));
        assert!(out.contains("State::new(17,15,16)"));
        assert!(out.contains("finish_projection"));
    }
    #[test]
    fn malformed_local_and_action_boundary_fail_closed() {
        let mut c: GroupEnvelopeContract = serde_json::from_str(CONTRACT).unwrap();
        c.actions.result_local = c.locals;
        assert!(generate_contract(&c, "2.3000.1047483476").is_err());
    }
    #[test]
    fn version_drift_requires_qualification() {
        assert!(generate("future").is_err());
    }
    #[test]
    fn action_facts_require_already_parsed_locals() {
        let mut contract: GroupEnvelopeContract = serde_json::from_str(CONTRACT).unwrap();
        let GroupEnvelopeExpr::Object { fields } = &mut contract.projection else {
            panic!("projection")
        };
        fields
            .iter_mut()
            .find(|field| field.name == "chatId")
            .unwrap()
            .value = GroupEnvelopeExpr::Read {
            method: GroupEnvelopeRead::AttrGroupJid,
            name: "from".into(),
        };
        assert!(generate_contract(&contract, "2.3000.1047483476").is_err());
    }
}
