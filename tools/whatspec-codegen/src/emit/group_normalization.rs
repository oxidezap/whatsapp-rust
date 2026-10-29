//! A separately pinned experimental contract. Keep the main IR pin untouched
//! until the full notification projection is qualified and integrated upstream.
use super::{header, rust_str};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::BTreeSet;

const CONTRACT: &str = include_str!("../../fixtures/group_unlink.json");
const REV: &str = "57494ccd865fa6471743264fea036f00d26d89cb";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Contract {
    module_name: String,
    start_byte: usize,
    end_byte: usize,
    rows: Vec<Row>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Row {
    unlink_type: Option<String>,
    reason: Option<String>,
    action_type: Option<String>,
    #[serde(default)]
    unsupported_action: bool,
}

pub fn generate(wa_version: &str) -> Result<String> {
    ensure!(
        wa_version == "2.3000.1047483476",
        "group normalization overlay requires snapshot review"
    );
    ensure!(
        crate::source::digest(CONTRACT.as_bytes())
            == "sha256:c7f306f66616f284f48a27d2612d201139de727042927d3a3f851e73acacae7f",
        "group unlink contract differs from the independent pinned revision"
    );
    let contract: Contract = serde_json::from_str(CONTRACT)?;
    ensure!(
        contract.module_name == "WAWebHandleGroupNotification"
            && contract.start_byte < contract.end_byte,
        "invalid source projection"
    );
    let kinds: BTreeSet<_> = contract
        .rows
        .iter()
        .filter_map(|r| r.unlink_type.as_deref())
        .collect();
    let reasons: BTreeSet<_> = contract
        .rows
        .iter()
        .filter_map(|r| r.reason.as_deref())
        .collect();
    ensure!(
        contract.rows.len() == (kinds.len() + 1) * (reasons.len() + 1),
        "incomplete unlink matrix"
    );
    let mut keys = BTreeSet::new();
    for row in &contract.rows {
        ensure!(
            !row.unsupported_action || row.action_type.is_none(),
            "non-string unlink value also names an action"
        );
        ensure!(
            row.action_type.as_deref().is_none_or(|v| [
                "parent_group_unlink",
                "sub_group_unlink",
                "sibling_group_unlink",
                "delete_parent_group_unlink",
                "integrity_parent_group_unlink",
                "integrity_sub_group_unlink"
            ]
            .contains(&v)),
            "unqualified unlink action"
        );
        ensure!(
            keys.insert((&row.unlink_type, &row.reason)),
            "duplicate matrix row"
        );
    }
    let evidence: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/group_unlink_evidence.json"))?;
    ensure!(
        evidence["waVersion"].as_str() == Some(wa_version),
        "oracle provenance version mismatch"
    );
    let mut out = header("experimental group unlink normalization", wa_version);
    out.push_str(&format!("//! Independent contract pin: https://github.com/oxidezap/whatspec/blob/{REV}/generated/notif/group-unlink.json\n//! Module-relative helper bytes {}..{}. Payload parsing remains handwritten.\n\n",contract.start_byte,contract.end_byte));
    for module in evidence["modules"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("missing oracle provenance"))?
    {
        out.push_str(&format!(
            "//! {}: bundle {}, module {}, bundle bytes {}..{}.\n",
            module["name"],
            module["bundleSha256"],
            module["moduleSha256"],
            module["startByte"],
            module["endByte"]
        ));
    }
    out.push_str("use super::GroupParseError;\npub(super) fn normalize_unlink(kind: &str, reason: Option<&str>) -> Result<Option<&'static str>,GroupParseError> {\n");
    out.push_str(&format!(
        "let kind = match kind {{ {} => Some(kind), _ => None }};\n",
        kinds
            .iter()
            .map(|s| rust_str(s))
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    out.push_str(&format!(
        "let reason = match reason {{ Some({}) => reason, _ => None }};\nmatch (kind,reason) {{\n",
        reasons
            .iter()
            .map(|s| rust_str(s))
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    for row in &contract.rows {
        let opt =
            |s: Option<String>| s.map_or("None".into(), |s| format!("Some({})", rust_str(&s)));
        out.push_str(&format!(
            "({}, {}) => {},\n",
            opt(row.unlink_type.clone()),
            opt(row.reason.clone()),
            if row.unsupported_action {
                "Err(GroupParseError::UnsupportedActionType(\"unlink_type\"))".to_owned()
            } else {
                format!("Ok({})", opt(row.action_type.clone()))
            }
        ));
    }
    out.push_str("_ => Ok(None),\n}\n}\n");
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct OracleCase {
        kind: String,
        reason: Option<String>,
        action_type: Option<String>,
        #[serde(default)]
        unsupported_action: bool,
    }
    let cases: Vec<OracleCase> =
        serde_json::from_str(include_str!("../../fixtures/group_unlink_oracle.json"))?;
    for case in &cases {
        let kind = if kinds.contains(case.kind.as_str()) {
            Some(case.kind.as_str())
        } else {
            None
        };
        let reason = case.reason.as_deref().filter(|v| reasons.contains(*v));
        let row = contract
            .rows
            .iter()
            .find(|r| r.unlink_type.as_deref() == kind && r.reason.as_deref() == reason)
            .ok_or_else(|| anyhow::anyhow!("missing oracle matrix row"))?;
        ensure!(
            row.action_type == case.action_type
                && row.unsupported_action == case.unsupported_action,
            "unlink IR differs from independent official oracle"
        );
    }
    out.push_str("\n#[cfg(test)]\nmod tests {\nuse super::normalize_unlink;\n#[test]\nfn isolated_official_js_cases() {\n");
    for case in cases {
        let opt =
            |s: Option<String>| s.map_or("None".into(), |s| format!("Some({})", rust_str(&s)));
        out.push_str(&format!(
            "assert_eq!(normalize_unlink({},{}),{});\n",
            rust_str(&case.kind),
            opt(case.reason),
            if case.unsupported_action {
                "Err(super::super::GroupParseError::UnsupportedActionType(\"unlink_type\"))"
                    .to_owned()
            } else {
                format!("Ok({})", opt(case.action_type))
            }
        ));
    }
    out.push_str("}\n}\n");
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
    fn matrix_covers_unknowns() {
        let out = generate("2.3000.1047483476").unwrap();
        assert!(out.contains("sibling_group_unlink"));
        assert!(out.contains("integrity_sub_group_unlink"));
    }
}
