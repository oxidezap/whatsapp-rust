//! Tests derived from a separately reviewed Web bundle oracle, not the Rust parser.
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use serde_json::Value;

use super::{header, rust_str};

const ORACLE: &str = include_str!("../../fixtures/accept_group_add.json");

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Oracle {
    schema_version: u32,
    whatspec_rev: String,
    wa_version: String,
    set_hash: String,
    archive_sha256: String,
    asset: String,
    modules: Vec<Module>,
    success_contracts: Vec<Value>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Module {
    name: String,
    bundle_sha256: String,
    start_byte: usize,
    end_byte: usize,
    module_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    children: Vec<String>,
    scalar: bool,
    expected: String,
}

/// Verify bundle and complete Acorn-extracted module bytes without executing JS.
pub fn verify_bundles(dir: &Path) -> Result<()> {
    let oracle: Oracle = serde_json::from_str(ORACLE)?;
    for module in oracle.modules {
        let bytes = std::fs::read(dir.join(format!("{}.js", module.bundle_sha256)))
            .with_context(|| format!("reading bundle for {}", module.name))?;
        verify_module(&bytes, &module)?;
    }
    Ok(())
}

fn verify_module(bytes: &[u8], module: &Module) -> Result<()> {
    ensure!(
        crate::source::digest(bytes) == format!("sha256:{}", module.bundle_sha256),
        "bundle digest mismatch for {}",
        module.name
    );
    let span = bytes
        .get(module.start_byte..module.end_byte)
        .with_context(|| format!("invalid module span for {}", module.name))?;
    ensure!(
        crate::source::digest(span) == format!("sha256:{}", module.module_sha256),
        "module digest mismatch for {}",
        module.name
    );
    ensure!(
        span.starts_with(format!("__d({:?},", module.name).as_bytes()),
        "module name mismatch for {}",
        module.name
    );
    Ok(())
}

pub fn validate_revision(rev: &str) -> Result<()> {
    let oracle: Oracle = serde_json::from_str(ORACLE)?;
    ensure!(
        oracle.whatspec_rev == rev,
        "IQ bundle oracle was reviewed at {}; review it before changing the pin to {rev}",
        oracle.whatspec_rev
    );
    Ok(())
}

pub fn generate(json: &str, wa_version: &str) -> Result<String> {
    let oracle: Oracle = serde_json::from_str(ORACLE)?;
    ensure!(
        oracle.schema_version == 1 && oracle.wa_version == wa_version,
        "IQ oracle needs review for this version"
    );
    let ir: Value = serde_json::from_str(json)?;
    let ops: Vec<_> = ir["stanzas"]
        .as_array()
        .context("missing stanzas")?
        .iter()
        .filter(|op| op["moduleName"] == "WASmaxGroupsAcceptGroupAddRPC")
        .collect();
    let [op] = ops.as_slice() else {
        anyhow::bail!("expected one AcceptGroupAdd contract")
    };
    let successes: Vec<_> = op["response"]["variants"]
        .as_array()
        .context("missing variants")?
        .iter()
        .filter(|variant| variant["kind"] == "success")
        .cloned()
        .collect();
    ensure!(
        successes == oracle.success_contracts,
        "AcceptGroupAdd IR differs from the reviewed bundle oracle; review guards, payload and precedence"
    );
    // Retain the pilot's request, envelope and delegated-error guards.
    super::iq_pilots::generate(json, wa_version)?;
    let mut out = header("private IQ bundle conformance cases", wa_version);
    out.push_str(&format!("//! Reviewed whatspec {}.\n//! Bundle set {}.\n//! Archive SHA-256 {}.\n//! Source asset: {}\n//! Correlation/error dispatch are outside this payload-only fixture.\n", oracle.whatspec_rev, oracle.set_hash, oracle.archive_sha256, oracle.asset));
    for module in oracle.modules {
        out.push_str(&format!(
            "//! {}: {} bytes {}..{}, module SHA-256 {}.\n",
            module.name,
            module.bundle_sha256,
            module.start_byte,
            module.end_byte,
            module.module_sha256
        ));
    }
    out.push_str("\nuse super::pilots::{AcceptGroupAddSuccess, parse_accept_group_add_payload};\nuse wacore_binary::{NodeContent, builder::NodeBuilder};\n\n#[test]\nfn accept_group_add_matches_reviewed_bundle_cases() {\n");
    for case in oracle.cases {
        ensure!(
            ["Success", "GroupJoinRequestSuccess"].contains(&case.expected.as_str()),
            "unreviewed oracle outcome"
        );
        ensure!(
            !case.scalar || case.children.is_empty(),
            "ambiguous oracle content"
        );
        let content = if case.scalar {
            "Some(NodeContent::Bytes(vec![0x66, 0x75, 0x74, 0x75, 0x72, 0x65]))".to_string()
        } else if case.children.is_empty() {
            "None".to_string()
        } else {
            format!(
                "Some(NodeContent::Nodes(vec![{}]))",
                case.children
                    .iter()
                    .map(|tag| format!("NodeBuilder::new({}).build()", rust_str(tag)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        out.push_str(&format!("let response = NodeBuilder::new(\"iq\").attr(\"type\", \"result\").apply_content({content}).build();\nassert_eq!(parse_accept_group_add_payload(&response.as_node_ref()), AcceptGroupAddSuccess::{}, {});\n", case.expected, rust_str(&case.name)));
    }
    out.push_str("}\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir() -> Value {
        // Bind the compact pilot input to the separately reviewed success shapes.
        let oracle: Value = serde_json::from_str(ORACLE).unwrap();
        let mut ir = super::super::iq_pilots::tests::fixture();
        let op = &mut ir["stanzas"][1];
        op["response"]["variants"] = oracle["successContracts"].clone();
        ir
    }

    #[test]
    fn source_verification_rejects_changed_bytes_spans_and_names() {
        let bytes = b"__d(\"fixture\",[],function(){})";
        let digest = crate::source::digest(bytes)
            .trim_start_matches("sha256:")
            .to_owned();
        let mut module = Module {
            name: "fixture".to_owned(),
            bundle_sha256: digest.clone(),
            start_byte: 0,
            end_byte: bytes.len(),
            module_sha256: digest,
        };
        assert!(verify_module(bytes, &module).is_ok());
        assert!(verify_module(b"changed", &module).is_err());
        module.end_byte += 1;
        assert!(verify_module(bytes, &module).is_err());
        module.end_byte -= 1;
        module.name = "other".to_owned();
        assert!(verify_module(bytes, &module).is_err());
    }

    #[test]
    fn provenance_cannot_follow_a_new_pin_without_review() {
        assert!(validate_revision("6cc5b0766eac00fc387a6a1f50d71a9ef6d5385a").is_ok());
        assert!(validate_revision("0000000000000000000000000000000000000000").is_err());
    }

    #[test]
    fn deterministic_cases_have_independently_recorded_expectations() {
        let output = generate(&ir().to_string(), "2.3000.1052559453").unwrap();
        assert_eq!(
            output,
            generate(&ir().to_string(), "2.3000.1052559453").unwrap()
        );
        assert!(output.contains("unknown_between_duplicates"));
        assert!(output.contains("AcceptGroupAddSuccess::Success, \"duplicate\""));
        assert!(output.contains("AcceptGroupAddSuccess::GroupJoinRequestSuccess, \"unique\""));
    }

    #[test]
    fn changed_order_guards_payload_or_version_require_bundle_review() {
        let original = ir();
        for mutation in 0..4 {
            let mut ir = original.clone();
            let variants = &mut ir["stanzas"][1]["response"]["variants"];
            match mutation {
                0 => variants.as_array_mut().unwrap().reverse(),
                1 => variants[0]["assertions"][4]["name"] = "new_child".into(),
                2 => variants[0]["fields"] = serde_json::json!([]),
                _ => {
                    let extra = variants[0].clone();
                    variants.as_array_mut().unwrap().push(extra);
                }
            }
            assert!(generate(&ir.to_string(), "2.3000.1052559453").is_err());
        }
        assert!(generate(&original.to_string(), "future").is_err());
    }
}
