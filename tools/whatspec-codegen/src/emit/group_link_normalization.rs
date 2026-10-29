//! Qualified LINK decisions and named wire readers. This overlay is independently
//! pinned while the main notification IR remains unchanged.
use super::{header, rust_str};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::BTreeSet;

const CONTRACT: &str = include_str!("../../fixtures/group_link.json");
const ORACLE: &str = include_str!("../../fixtures/group_link_oracle.json");
const EVIDENCE: &str = include_str!("../../fixtures/group_link_evidence.json");
// Root fills these together with the immutable upstream publication.
const REV: &str = "ed8da49c24a0e943854e1c7077c60129e169aeb4";
const CONTRACT_SHA256: &str = "0947df9df11850633040d9f55e73599f5695a6af56e48b1bd9bfa62f0bee0901";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Contract {
    module_name: String,
    start_byte: usize,
    end_byte: usize,
    wire_tag: String,
    link_type_attribute: String,
    group_child_tag: String,
    group_fields: Vec<Field>,
    rows: Vec<Row>,
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
struct Row {
    link_type: Option<String>,
    action_type: Option<String>,
    #[serde(default)]
    unsupported_action: bool,
    hidden_subgroup_child: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleCase {
    kind: String,
    hidden: bool,
    #[serde(default)]
    unsupported_action: bool,
    result: OracleResult,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleResult {
    #[serde(default)]
    action_type: Option<String>,
    #[serde(default)]
    action_value_type: Option<String>,
    group_datas: Vec<OracleGroup>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct OracleGroup {
    id: String,
    subject: String,
    subject_time: i64,
    hidden_subgroup: Option<bool>,
}

pub fn generate(wa_version: &str) -> Result<String> {
    ensure!(
        wa_version == "2.3000.1047483476",
        "LINK overlay requires snapshot qualification"
    );
    ensure!(
        REV.len() == 40 && REV.bytes().all(|b| b.is_ascii_hexdigit()),
        "LINK upstream revision is not pinned"
    );
    ensure!(
        crate::source::digest(CONTRACT.as_bytes()) == format!("sha256:{CONTRACT_SHA256}"),
        "LINK contract differs from the independently pinned projection"
    );
    generate_reviewed(CONTRACT, ORACLE, EVIDENCE, wa_version)
}
fn generate_reviewed(json: &str, oracle: &str, evidence: &str, wa_version: &str) -> Result<String> {
    let contract: Contract = serde_json::from_str(json)?;
    ensure!(
        contract.wire_tag == "link",
        "LINK public wire tag needs qualification"
    );
    ensure!(
        contract.module_name == "WAWebHandleGroupNotification"
            && contract.start_byte < contract.end_byte,
        "invalid LINK provenance span"
    );
    ensure!(
        contract
            .group_fields
            .iter()
            .map(|f| (f.name.as_str(), f.method.as_str()))
            .collect::<Vec<_>>()
            == [
                ("id", "attrGroupJid"),
                ("subject", "attrString"),
                ("subjectTime", "attrInt")
            ],
        "unsupported LINK field/read ordering"
    );
    let mut keys = BTreeSet::new();
    for row in &contract.rows {
        ensure!(
            keys.insert(row.link_type.as_deref()),
            "duplicate LINK matrix input"
        );
        ensure!(
            !row.unsupported_action || row.action_type.is_none(),
            "non-string LINK result also names an action"
        );
        ensure!(
            !row.unsupported_action || row.link_type.is_some(),
            "unknown fallback cannot be uniformly unsupported"
        );
    }
    ensure!(
        keys.contains(&None),
        "missing ordinary unknown LINK fallback"
    );
    let actions: BTreeSet<_> = contract
        .rows
        .iter()
        .filter_map(|r| r.action_type.as_deref())
        .collect();
    ensure!(
        actions == BTreeSet::from(["parent_group_link", "sibling_group_link", "sub_group_link"]),
        "unsupported LINK action vocabulary"
    );
    let hidden_tags: BTreeSet<_> = contract
        .rows
        .iter()
        .filter_map(|r| r.hidden_subgroup_child.as_deref())
        .collect();
    ensure!(
        hidden_tags.len() == 1,
        "unsupported LINK hidden-child vocabulary"
    );
    let evidence: serde_json::Value = serde_json::from_str(evidence)?;
    ensure!(
        evidence["waVersion"].as_str() == Some(wa_version),
        "LINK oracle snapshot differs"
    );
    let mut out = header("qualified group LINK normalization", wa_version);
    out.push_str(&format!("//! Source contract: https://github.com/oxidezap/whatspec/blob/{REV}/generated/notif/group-link.json\n//! Module-relative LINK case bytes {}..{}.\n//! Plain Object.prototype results remain explicitly unsupported.\n",contract.start_byte,contract.end_byte));
    for module in evidence["modules"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing LINK module evidence"))?
    {
        out.push_str(&format!(
            "//! {}: bundle {}, module {}, bytes {}..{}.\n",
            module["name"],
            module["bundleSha256"],
            module["moduleSha256"],
            module["startByte"],
            module["endByte"]
        ));
    }
    out.push_str("\nuse super::super::normalized::GroupParseError;\n");
    for (name, value) in [
        ("LINK_TYPE_ATTRIBUTE", contract.link_type_attribute.as_str()),
        ("GROUP_CHILD_TAG", contract.group_child_tag.as_str()),
        (
            "GROUP_JID_ATTRIBUTE",
            contract.group_fields[0].wire_name.as_str(),
        ),
        (
            "GROUP_SUBJECT_ATTRIBUTE",
            contract.group_fields[1].wire_name.as_str(),
        ),
        (
            "GROUP_SUBJECT_TIME_ATTRIBUTE",
            contract.group_fields[2].wire_name.as_str(),
        ),
    ] {
        out.push_str(&format!(
            "pub(super) const {name}: &str = {};\n",
            rust_str(value)
        ));
    }
    out.push_str("\npub(super) fn normalize_link(kind:&str)->Result<Option<&'static str>,GroupParseError> {\nmatch kind {\n");
    for row in contract.rows.iter().filter(|r| r.link_type.is_some()) {
        let kind = rust_str(row.link_type.as_deref().unwrap_or_default());
        let outcome = if row.unsupported_action {
            "Err(GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE))".into()
        } else {
            format!("Ok({})", opt_string(row.action_type.as_deref()))
        };
        out.push_str(&format!("{kind} => {outcome},\n"));
    }
    let fallback = contract
        .rows
        .iter()
        .find(|r| r.link_type.is_none())
        .ok_or_else(|| anyhow::anyhow!("missing fallback"))?;
    ensure!(
        !fallback.unsupported_action && fallback.action_type.is_none(),
        "changed ordinary unknown result"
    );
    out.push_str("_ => Ok(None),\n}\n}\n");
    out.push_str(
        "\npub(super) fn hidden_subgroup_child(kind:&str)->Option<&'static str> {\nmatch kind {\n",
    );
    for row in contract.rows.iter().filter(|r| r.link_type.is_some()) {
        out.push_str(&format!(
            "{} => {},\n",
            rust_str(row.link_type.as_deref().unwrap_or_default()),
            opt_string(row.hidden_subgroup_child.as_deref())
        ));
    }
    out.push_str(&format!(
        "_ => {},\n}}\n}}\n",
        opt_string(fallback.hidden_subgroup_child.as_deref())
    ));
    let cases: Vec<OracleCase> = serde_json::from_str(oracle)?;
    ensure!(
        cases.len() == 34,
        "incomplete independent LINK oracle matrix"
    );
    let mut oracle_keys = BTreeSet::new();
    for case in &cases {
        ensure!(
            oracle_keys.insert((case.kind.as_str(), case.hidden)),
            "duplicate independent LINK oracle input"
        );
        let row = contract
            .rows
            .iter()
            .find(|row| row.link_type.as_deref() == Some(case.kind.as_str()))
            .unwrap_or(fallback);
        ensure!(
            row.unsupported_action == case.unsupported_action,
            "LINK row loses an official unsupported result"
        );
        ensure!(
            row.action_type == case.result.action_type,
            "LINK action differs from independent official result"
        );
        let [group] = case.result.group_datas.as_slice() else {
            anyhow::bail!("oracle group shape changed");
        };
        ensure!(
            group.hidden_subgroup == row.hidden_subgroup_child.as_ref().map(|_| case.hidden),
            "LINK hidden field presence/value differs from official result"
        );
    }

    out.push_str("\n#[cfg(test)]\nmod tests {\nuse super::*;\nuse super::super::parse_link;\nuse wacore_binary::builder::NodeBuilder;\n#[test]\nfn official_link_cases_cover_normalization_and_field_presence() {\nfor (kind, hidden, unsupported, expected_action, jid, subject, subject_time, expected_hidden) in [\n");
    for case in cases {
        let [group] = case.result.group_datas.as_slice() else {
            anyhow::bail!("oracle group shape changed");
        };
        ensure!(
            !case.unsupported_action
                || matches!(
                    case.result.action_value_type.as_deref(),
                    Some("function" | "object")
                ),
            "unsupported oracle result was collapsed"
        );
        ensure!(
            case.unsupported_action || case.result.action_value_type.is_none(),
            "ordinary oracle result gained an opaque value"
        );
        out.push_str(&format!(
            "({}, {}, {}, {}, {}, {}, {}i64, {}),\n",
            rust_str(&case.kind),
            case.hidden,
            case.unsupported_action,
            opt_string(case.result.action_type.as_deref()),
            rust_str(&group.id),
            rust_str(&group.subject),
            group.subject_time,
            group
                .hidden_subgroup
                .map_or("None".into(), |v| format!("Some({v})"))
        ));
    }
    out.push_str("] {\nlet group = NodeBuilder::new(\"group\").attr(\"jid\",jid).attr(\"subject\",subject).attr(\"s_t\",subject_time.to_string()).children(if hidden {vec![NodeBuilder::new(\"hidden_group\").build()]} else {vec![]}).build();\nlet node = NodeBuilder::new(\"link\").attr(\"link_type\",kind).children([group]).build();\nlet actual = parse_link(&node.as_node_ref());\nif unsupported {assert_eq!(actual.unwrap_err(),GroupParseError::UnsupportedActionType(LINK_TYPE_ATTRIBUTE));continue;}\nlet actual=actual.unwrap();\nassert_eq!(actual.kind.map(|v|v.as_str()),expected_action,\"{kind}/{hidden}\");\nassert_eq!(actual.groups.len(),1);\nassert_eq!(actual.groups[0].jid.to_string(),jid);\nassert_eq!(actual.groups[0].subject,subject);\nassert_eq!(actual.groups[0].subject_time,subject_time);\nassert_eq!(actual.groups[0].hidden_subgroup,expected_hidden,\"{kind}/{hidden}\");\n}\n}\n}\n");
    Ok(out)
}
fn opt_string(value: Option<&str>) -> String {
    value.map_or("None".into(), |v| format!("Some({})", rust_str(v)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn future_snapshot_fails_qualification() {
        assert!(generate("future").is_err());
    }
    #[test]
    fn malformed_matrix_rejects_duplicate_inputs() {
        let mut value: serde_json::Value = serde_json::from_str(CONTRACT).unwrap();
        let duplicate = value["rows"][0].clone();
        value["rows"].as_array_mut().unwrap().push(duplicate);
        assert!(
            generate_reviewed(&value.to_string(), ORACLE, EVIDENCE, "2.3000.1047483476").is_err()
        );
    }
    #[test]
    fn malformed_fields_cannot_silently_change_decoder() {
        let mut value: serde_json::Value = serde_json::from_str(CONTRACT).unwrap();
        value["groupFields"][0]["method"] = "attrString".into();
        assert!(
            generate_reviewed(&value.to_string(), ORACLE, EVIDENCE, "2.3000.1047483476").is_err()
        );
    }
}
