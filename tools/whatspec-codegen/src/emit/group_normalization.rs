//! A separately pinned experimental contract. Keep the main IR pin untouched
//! until the full notification projection is qualified and integrated upstream.
use super::{header, rust_str};
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::collections::BTreeSet;

const CONTRACT: &str = include_str!("../../fixtures/group_unlink.json");
const REV: &str = "2292a11135c5ce0bacf55e06ee9471a6b3f7f219";
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
}

pub fn generate(wa_version: &str) -> Result<String> {
    ensure!(
        wa_version == "2.3000.1047483476",
        "group normalization overlay requires snapshot review"
    );
    ensure!(
        crate::source::digest(CONTRACT.as_bytes())
            == "sha256:420b8ffd4adb49a7c7e6a5c4b13e7efd3491ef8dcf0f6e016b7d039f3b1735a6",
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
    out.push_str("pub(super) fn normalize_unlink(kind: &str, reason: Option<&str>) -> Option<&'static str> {\n");
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
    for row in contract.rows {
        let opt =
            |s: Option<String>| s.map_or("None".into(), |s| format!("Some({})", rust_str(&s)));
        out.push_str(&format!(
            "({}, {}) => {},\n",
            opt(row.unlink_type),
            opt(row.reason),
            opt(row.action_type)
        ));
    }
    out.push_str("_ => None,\n}\n}\n");
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct OracleCase {
        kind: String,
        reason: Option<String>,
        action_type: Option<String>,
    }
    let cases: Vec<OracleCase> =
        serde_json::from_str(include_str!("../../fixtures/group_unlink_oracle.json"))?;
    out.push_str("\n#[cfg(test)]\nmod tests {\nuse super::normalize_unlink;\n#[test]\nfn isolated_official_js_cases() {\n");
    for case in cases {
        let opt =
            |s: Option<String>| s.map_or("None".into(), |s| format!("Some({})", rust_str(&s)));
        out.push_str(&format!(
            "assert_eq!(normalize_unlink({},{}),{});\n",
            rust_str(&case.kind),
            opt(case.reason),
            opt(case.action_type)
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
