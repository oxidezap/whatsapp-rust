//! Qualified decision tables and independent current-snapshot normalization cases.
use super::{header, rust_str};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;

pub(super) const VERSION: &str = "2.3000.1052559453";
const REV: &str = "ae3880925bacd23863cc30ec480c9f82bfd869f1";
const LINK: &str = include_str!("../../fixtures/group_link.json");
const UNLINK: &str = include_str!("../../fixtures/group_unlink.json");
const ORACLE: &str = include_str!("../../fixtures/group_normalization_oracle.json");
const EVIDENCE: &str = include_str!("../../fixtures/group_link_evidence.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct NullResult {
    template: Vec<String>,
    event: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    link_type: Option<String>,
    unlink_type: Option<String>,
    reason: Option<String>,
    action_type: Option<String>,
    #[serde(default)]
    unsupported_action: bool,
    null_result: Option<NullResult>,
    hidden_subgroup_child: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Field {
    name: String,
    wire_name: String,
    method: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Contract {
    module_name: String,
    start_byte: usize,
    end_byte: usize,
    wire_tag: Option<String>,
    link_type_attribute: Option<String>,
    group_child_tag: Option<String>,
    #[serde(default)]
    group_fields: Vec<Field>,
    rows: Vec<Row>,
}
fn opt(value: Option<&str>) -> String {
    value.map_or("None".into(), |v| format!("Some({})", rust_str(v)))
}
fn result(row: &Row, attribute: &str) -> Result<String> {
    ensure!(
        [
            row.action_type.is_some(),
            row.unsupported_action,
            row.null_result.is_some()
        ]
        .into_iter()
        .filter(|v| *v)
        .count()
            == 1,
        "decision must have exactly one qualified outcome"
    );
    if row.unsupported_action {
        return Ok(format!(
            "Err(GroupParseError::UnsupportedActionType({}))",
            rust_str(attribute)
        ));
    }
    if let Some(action) = &row.action_type {
        return Ok(format!("Ok(Decision::Action({}))", rust_str(action)));
    }
    let null = row.null_result.as_ref().context("missing null decision")?;
    ensure!(
        null.template.len() == 2 && !null.event.is_empty(),
        "unsupported warning operation"
    );
    Ok(format!(
        "Ok(Decision::Null(WarningSpec{{template:&[{},{}],event:{}}}))",
        rust_str(&null.template[0]),
        rust_str(&null.template[1]),
        rust_str(&null.event)
    ))
}
fn contract(json: &str, family: &str, version: &str) -> Result<Contract> {
    ensure!(
        version == VERSION,
        "group normalization requires snapshot qualification"
    );
    verify_pin(
        ORACLE,
        "bc29dabb2069e9e33567acb09001ad94243c809ec72c2fdf6a679bee10db3459",
    )?;
    verify_pin(
        EVIDENCE,
        "8f7551630e7ce7a6045c1bb3a5aa2173612a62cbe2ecbc7657acd7182588d2c0",
    )?;
    let c: Contract = serde_json::from_str(json)?;
    ensure!(
        c.module_name == "WAWebHandleGroupNotification" && c.start_byte < c.end_byte,
        "invalid source projection"
    );
    let mut keys = BTreeSet::new();
    for row in &c.rows {
        let kind = if family == "link" {
            &row.link_type
        } else {
            &row.unlink_type
        };
        ensure!(
            keys.insert((kind.as_deref(), row.reason.as_deref())),
            "duplicate normalization matrix input"
        );
        ensure!(
            row.null_result.is_some() == kind.is_none(),
            "only ordinary unknown types may return null"
        );
        ensure!(
            row.null_result.is_none() || row.hidden_subgroup_child.is_none(),
            "null LINK result cannot read a hidden child"
        );
        let allowed: &[&str] = if family == "link" {
            &["parent_group_link", "sub_group_link", "sibling_group_link"]
        } else {
            &[
                "parent_group_unlink",
                "sub_group_unlink",
                "sibling_group_unlink",
                "delete_parent_group_unlink",
                "integrity_parent_group_unlink",
                "integrity_sub_group_unlink",
            ]
        };
        ensure!(
            row.action_type
                .as_deref()
                .is_none_or(|a| allowed.contains(&a)),
            "unqualified action identity"
        );
        let _ = result(
            row,
            if family == "link" {
                "link_type"
            } else {
                "unlink_type"
            },
        )?;
    }
    if family == "link" {
        ensure!(
            c.wire_tag.as_deref() == Some("link")
                && c.link_type_attribute.is_some()
                && c.group_child_tag.is_some(),
            "LINK envelope changed"
        );
        ensure!(
            c.group_fields
                .iter()
                .map(|f| (f.name.as_str(), f.method.as_str()))
                .collect::<Vec<_>>()
                == [
                    ("id", "attrGroupJid"),
                    ("subject", "attrString"),
                    ("subjectTime", "attrInt")
                ],
            "LINK field/read order changed"
        );
        ensure!(
            c.rows.iter().any(|r| r.link_type.is_none()),
            "missing LINK unknown fallback"
        );
        ensure!(
            c.rows
                .iter()
                .all(|r| r.unlink_type.is_none() && r.reason.is_none()),
            "LINK acquired unrelated decision inputs"
        );
        ensure!(
            c.rows
                .iter()
                .filter_map(|r| r.hidden_subgroup_child.as_deref())
                .collect::<BTreeSet<_>>()
                .len()
                == 1,
            "LINK hidden-child vocabulary changed"
        );
    } else {
        ensure!(
            c.wire_tag.is_none()
                && c.link_type_attribute.is_none()
                && c.group_child_tag.is_none()
                && c.group_fields.is_empty(),
            "UNLINK acquired unmodeled readers"
        );
        let kinds = c
            .rows
            .iter()
            .filter_map(|r| r.unlink_type.as_deref())
            .collect::<BTreeSet<_>>();
        let reasons = c
            .rows
            .iter()
            .filter_map(|r| r.reason.as_deref())
            .collect::<BTreeSet<_>>();
        ensure!(
            c.rows.len() == (kinds.len() + 1) * (reasons.len() + 1),
            "incomplete UNLINK matrix"
        );
        ensure!(
            c.rows
                .iter()
                .all(|r| r.link_type.is_none() && r.hidden_subgroup_child.is_none()),
            "UNLINK acquired unrelated fields"
        );
    }
    verify_oracle(&c, family)?;
    Ok(c)
}
fn verify_oracle(c: &Contract, family: &str) -> Result<()> {
    let oracle: Value = serde_json::from_str(ORACLE)?;
    let evidence: Value = serde_json::from_str(EVIDENCE)?;
    ensure!(
        oracle["waVersion"] == VERSION
            && evidence["waVersion"] == VERSION
            && oracle["setHash"] == evidence["setHash"],
        "normalization oracle provenance differs"
    );
    let cases = oracle["cases"]
        .as_array()
        .context("missing normalization cases")?;
    ensure!(
        cases.len() == 204,
        "incomplete current normalization oracle"
    );
    let reasons = c
        .rows
        .iter()
        .filter_map(|r| r.reason.as_deref())
        .collect::<BTreeSet<_>>();
    for case in cases.iter().filter(|v| v["family"] == family) {
        let kind = case["kind"].as_str().context("non-string oracle kind")?;
        let known = c.rows.iter().any(|r| {
            if family == "link" {
                r.link_type.as_deref() == Some(kind)
            } else {
                r.unlink_type.as_deref() == Some(kind)
            }
        });
        let reason = case["reason"].as_str().filter(|r| reasons.contains(*r));
        let row = c
            .rows
            .iter()
            .find(|r| {
                (if family == "link" {
                    r.link_type.as_deref()
                } else {
                    r.unlink_type.as_deref()
                }) == known.then_some(kind)
                    && (family == "link" || r.reason.as_deref() == reason)
            })
            .context("missing oracle decision")?;
        ensure!(
            case["returnsNull"].as_bool() == Some(row.null_result.is_some()),
            "null-result differs from original source"
        );
        if row.null_result.is_some() {
            ensure!(case["groupDatas"].is_null(), "null action retained payload");
        } else if family == "link" {
            let groups = case["groupDatas"]
                .as_array()
                .context("missing original group payload")?;
            ensure!(groups.len() == 1, "unexpected original group payload count");
            let actual_hidden = groups[0].get("hiddenSubgroup");
            let expected_hidden = row.hidden_subgroup_child.as_ref().map(|_| &case["hidden"]);
            ensure!(
                actual_hidden == expected_hidden,
                "hidden-subgroup value differs from original source"
            );
        }
        if row.unsupported_action {
            ensure!(
                matches!(
                    case["actionType"]["valueType"].as_str(),
                    Some("function" | "object")
                ),
                "opaque source action lost"
            );
        } else {
            ensure!(
                case["actionType"] == row.action_type.as_ref().map_or(Value::Null, |v| json!(v)),
                "source action identity differs"
            );
        }
        let calls = if let Some(n) = &row.null_result {
            json!([["WARN", n.template, kind], ["sendLogs", n.event]])
        } else {
            json!([])
        };
        ensure!(
            case["calls"] == calls,
            "warning arguments/order differ from original source"
        );
        let mut reads = if family == "link" {
            vec!["link_type"]
        } else {
            vec!["unlink_type", "unlink_reason"]
        };
        if family == "unlink" && !case["reason"].is_null() {
            reads.push("unlink_reason");
        }
        if family == "unlink" || row.null_result.is_none() {
            reads.extend(["groups", "jid", "subject", "s_t"]);
            if family == "link" && row.hidden_subgroup_child.is_some() {
                reads.push("hidden_group");
            }
        }
        ensure!(
            case["reads"] == json!(reads),
            "payload/warning read ordering differs from source"
        );
    }
    Ok(())
}
fn preamble(family: &str, version: &str, c: &Contract) -> String {
    format!(
        "{}//! Qualified source: <https://github.com/oxidezap/whatspec/blob/{REV}/generated/notif/group-{family}.json>\n//! Module-relative source bytes {}..{}. Null results preserve ordered warnings.\nuse super::super::normalization_warning::{{Decision,WarningSpec}};\nuse super::super::normalized::GroupParseError;\n",
        header(&format!("group {family} decisions"), version),
        c.start_byte,
        c.end_byte
    )
}
pub(super) fn generate_link_reviewed(json: &str, version: &str) -> Result<String> {
    let c = contract(json, "link", version)?;
    let mut out = preamble("link", version, &c);
    for (name, value) in [
        (
            "LINK_TYPE_ATTRIBUTE",
            c.link_type_attribute
                .as_deref()
                .context("missing LINK attribute")?,
        ),
        (
            "GROUP_CHILD_TAG",
            c.group_child_tag.as_deref().context("missing group tag")?,
        ),
        ("GROUP_JID_ATTRIBUTE", c.group_fields[0].wire_name.as_str()),
        (
            "GROUP_SUBJECT_ATTRIBUTE",
            c.group_fields[1].wire_name.as_str(),
        ),
        (
            "GROUP_SUBJECT_TIME_ATTRIBUTE",
            c.group_fields[2].wire_name.as_str(),
        ),
    ] {
        out.push_str(&format!(
            "pub(super) const {name}: &str={};\n",
            rust_str(value)
        ));
    }
    out.push_str(
        "pub(super) fn decide_link(kind:&str)->Result<Decision,GroupParseError>{match kind {\n",
    );
    for row in c.rows.iter().filter(|r| r.link_type.is_some()) {
        out.push_str(&format!(
            "{}=>{},\n",
            rust_str(row.link_type.as_deref().context("missing kind")?),
            result(row, "link_type")?
        ));
    }
    let fallback = c
        .rows
        .iter()
        .find(|r| r.link_type.is_none())
        .context("missing LINK fallback")?;
    out.push_str(&format!("_=>{},\n}}}}\n", result(fallback, "link_type")?));
    out.push_str(
        "pub(super) fn hidden_subgroup_child(kind:&str)->Option<&'static str>{match kind{\n",
    );
    for row in c.rows.iter().filter(|r| r.link_type.is_some()) {
        out.push_str(&format!(
            "{}=>{},\n",
            rust_str(row.link_type.as_deref().context("missing kind")?),
            opt(row.hidden_subgroup_child.as_deref())
        ));
    }
    out.push_str(&format!(
        "_=>{},\n}}}}\n",
        opt(fallback.hidden_subgroup_child.as_deref())
    ));
    Ok(out)
}
pub fn generate(version: &str) -> Result<String> {
    verify_pin(UNLINK, UNLINK_SHA)?;
    let c = contract(UNLINK, "unlink", version)?;
    let mut out = preamble("unlink", version, &c);
    let kinds = c
        .rows
        .iter()
        .filter_map(|r| r.unlink_type.as_deref())
        .collect::<BTreeSet<_>>();
    let reasons = c
        .rows
        .iter()
        .filter_map(|r| r.reason.as_deref())
        .collect::<BTreeSet<_>>();
    out.push_str(&format!("pub(super) fn decide_unlink(kind:&str,reason:Option<&str>)->Result<Decision,GroupParseError>{{let kind=match kind{{{}=>Some(kind),_=>None}};let reason=match reason{{Some({})=>reason,_=>None}};match(kind,reason){{\n",kinds.iter().map(|v|rust_str(v)).collect::<Vec<_>>().join("|"),reasons.iter().map(|v|rust_str(v)).collect::<Vec<_>>().join("|")));
    for row in &c.rows {
        out.push_str(&format!(
            "({},{})=>{},\n",
            opt(row.unlink_type.as_deref()),
            opt(row.reason.as_deref()),
            result(row, "unlink_type")?
        ));
    }
    out.push_str("_=>Err(GroupParseError::Field(\"unqualified unlink decision\")),\n}}\n");
    Ok(out)
}
pub(super) fn verify_pin(source: &str, digest: &str) -> Result<()> {
    ensure!(
        crate::source::digest(source.as_bytes()) == format!("sha256:{digest}"),
        "group contract differs from qualified immutable pin"
    );
    Ok(())
}
const UNLINK_SHA: &str = "4ad96a230e7850d6e5407bd8b3e4c073d13ab0a2c123bad54907c09e13405ff2";
pub(super) const LINK_SHA: &str =
    "80f39ae823f54827de3ad7af9e96e103d1212a91a109d0a97888913ec902b641";
pub(super) fn generate_link(version: &str) -> Result<String> {
    verify_pin(LINK, LINK_SHA)?;
    generate_link_reviewed(LINK, version)
}

pub fn conformance(version: &str) -> Result<String> {
    let _ = contract(LINK, "link", version)?;
    let _ = contract(UNLINK, "unlink", version)?;
    let oracle: Value = serde_json::from_str(ORACLE)?;
    let mut out = header("original LINK/UNLINK normalization conformance", version);
    out.push_str("use super::{link_normalized::parse_link_with_warnings,normalized::{parse_unlink_with_warnings,GroupParseError},normalization_warning::GroupWarningSink};\nuse wacore_binary::builder::NodeBuilder;\nuse serde_json::{Value,json};\n#[derive(Default)] struct Recorder{calls:Vec<Value>}\nimpl GroupWarningSink for Recorder{fn warn(&mut self,template:&'static[&'static str],value:&str)->Result<(),GroupParseError>{self.calls.push(json!([\"WARN\",template,value]));Ok(())}fn send_logs(&mut self,event:&'static str)->Result<(),GroupParseError>{self.calls.push(json!([\"sendLogs\",event]));Ok(())}}\n#[test]fn original_normalization_matrix(){\n");
    for case in oracle["cases"].as_array().context("missing cases")? {
        let family = case["family"].as_str().context("missing family")?;
        let kind = case["kind"].as_str().context("missing kind")?;
        let hidden = case["hidden"].as_bool().context("missing hidden")?;
        let unsupported = case["actionType"].is_object();
        let null = case["returnsNull"].as_bool().context("missing null")?;
        let expected = case["actionType"].as_str();
        out.push_str("{let mut recorder=Recorder::default();let group=NodeBuilder::new(\"group\").attr(\"jid\",\"123456@g.us\").attr(\"subject\",\"synthetic\").attr(\"s_t\",\"123\")");
        if hidden {
            out.push_str(".children([NodeBuilder::new(\"hidden_group\").build()])");
        }
        out.push_str(&format!(
            ".build();let node=NodeBuilder::new({}).attr({}, {})",
            rust_str(family),
            rust_str(&format!("{family}_type")),
            rust_str(kind)
        ));
        if let Some(reason) = case["reason"].as_str() {
            out.push_str(&format!(".attr(\"unlink_reason\",{})", rust_str(reason)));
        }
        out.push_str(".children([group]).build();");
        out.push_str(&format!(
            "let actual=parse_{family}_with_warnings(&node.as_node_ref(),&mut recorder);"
        ));
        if unsupported {
            out.push_str(&format!(
                "assert_eq!(actual.unwrap_err(),GroupParseError::UnsupportedActionType({}));",
                rust_str(&format!("{family}_type"))
            ));
        } else {
            out.push_str(&format!("let actual=actual.unwrap();assert_eq!(actual.kind.map(|v|v.as_str()),{});assert_eq!(actual.groups.len(),{});",opt(expected),usize::from(!null)));
            if family == "link" && !null {
                out.push_str(&format!(
                    "assert_eq!(actual.groups[0].hidden_subgroup,{});",
                    case["groupDatas"][0]
                        .get("hiddenSubgroup")
                        .map_or_else(|| "None".into(), |value| format!("Some({value})"))
                ));
            }
        }
        out.push_str(&format!("assert_eq!(Value::Array(recorder.calls),serde_json::from_str::<Value>({}).unwrap());}}\n",rust_str(&case["calls"].to_string())));
    }
    out.push_str("}\n");
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn version_drift_requires_review() {
        assert!(generate("future").is_err());
    }
    #[test]
    fn qualified_matrix_covers_null_and_prototypes() {
        assert!(generate(VERSION).unwrap().contains("Decision::Null"));
        assert!(
            conformance(VERSION)
                .unwrap()
                .contains("original_normalization_matrix")
        );
    }
    #[test]
    fn hidden_values_are_checked_against_original_group_payloads() {
        let mut value: Value = serde_json::from_str(LINK).unwrap();
        let row = value["rows"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|r| r["hiddenSubgroupChild"] == "hidden_group")
            .unwrap();
        row["hiddenSubgroupChild"] = Value::Null;
        let contract: Contract = serde_json::from_value(value).unwrap();
        assert!(
            verify_oracle(&contract, "link")
                .unwrap_err()
                .to_string()
                .contains("hidden-subgroup value")
        );
    }
    #[test]
    fn warnings_and_read_order_cannot_be_changed() {
        for mutation in 0..5 {
            let mut c: Value = serde_json::from_str(LINK).unwrap();
            match mutation {
                0 => {
                    c["rows"][0].as_object_mut().unwrap().remove("nullResult");
                }
                1 => c["rows"][0]["nullResult"]["event"] = json!("wrong"),
                2 => c["groupFields"].as_array_mut().unwrap().reverse(),
                3 => c["rows"][0]["hiddenSubgroupChild"] = json!("hidden_group"),
                _ => {
                    let duplicate = c["rows"][0].clone();
                    c["rows"].as_array_mut().unwrap().push(duplicate);
                }
            };
            assert!(generate_link_reviewed(&c.to_string(), VERSION).is_err());
        }
    }
}
