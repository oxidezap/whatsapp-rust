//! Bounded, checked full-RPC consumers. Admission is separate from qualification.
use anyhow::{Context, Result, ensure};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceLock {
    rev: String,
    wa_version: String,
    source_iq_sha256: String,
    projection_sha256: String,
}

fn source_lock() -> Result<SourceLock> {
    let lock: SourceLock = serde_json::from_str(include_str!("../../iq-contextual.lock.json"))?;
    ensure!(
        lock.rev.len() == 40 && lock.rev.chars().all(|c| c.is_ascii_hexdigit()),
        "contextual IQ revision is not immutable"
    );
    ensure!(
        crate::source::digest(include_bytes!("../../fixtures/contextual_iq.json"))
            == lock.projection_sha256,
        "contextual IQ projection digest mismatch"
    );
    ensure!(
        include_str!("../../Cargo.toml").contains(&format!("rev = {:?}", lock.rev)),
        "IQ generator dependency must match the contextual source revision"
    );
    Ok(lock)
}

pub fn revision() -> Result<String> {
    Ok(source_lock()?.rev)
}

const MODULES: &[&str] = &[
    "WASmaxOutGroupsSetSubjectRequest",
    "WASmaxOutGroupsAcceptGroupAddRequest",
    "WASmaxOutGroupsJoinLinkedGroupRequest",
    "WASmaxOutGroupsDeleteParentGroupRequest",
    "WASmaxOutPassiveModeActiveIQRequest",
    "WASmaxOutPassiveModePassiveIQRequest",
    "WASmaxOutMdGetCountryCodeRequest",
];

pub fn generate(wa_version: &str) -> Result<String> {
    // Dedicated input projection avoids repinning unrelated domains while the
    // source IQ PR is still under review. See iq-contextual.lock.json.
    let json = include_str!("../../fixtures/contextual_iq.json");
    let lock = source_lock()?;
    ensure!(
        lock.wa_version == wa_version,
        "contextual IQ lock needs version review"
    );
    let mut ir: wa_ir_iq::IqIr = serde_json::from_str(json).context("reading contextual IQ IR")?;
    ensure!(
        ir.wa_version == wa_version,
        "contextual IQ snapshot needs review for this WhatsApp version"
    );
    for module in MODULES {
        ensure!(
            ir.stanzas
                .iter()
                .filter(|s| s.module_name == *module)
                .count()
                == 1,
            "expected exactly one {module}"
        );
    }
    ir.stanzas
        .retain(|s| MODULES.contains(&s.module_name.as_str()));
    wa_codegen_iq::generate_iq_checked(&ir).map_err(|d| {
        anyhow::anyhow!("contextual IQ generation refused incomplete contracts: {d:#?}")
    })
}

pub fn verify_projection(full: &str) -> Result<()> {
    let lock = source_lock()?;
    ensure!(
        crate::source::digest(full.as_bytes()) == lock.source_iq_sha256,
        "contextual IQ source digest mismatch at {}",
        lock.rev
    );
    let mut ir: serde_json::Value = serde_json::from_str(full)?;
    ir["stanzas"]
        .as_array_mut()
        .context("missing stanzas")?
        .retain(|s| {
            s["moduleName"]
                .as_str()
                .is_some_and(|m| MODULES.contains(&m))
        });
    ir["unparseable"] = serde_json::json!([]);
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../fixtures/contextual_iq.json"))?;
    ensure!(
        ir == expected,
        "contextual IQ projection differs from the source snapshot"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_projection_emits_and_wrong_source_or_version_fails() {
        let lock = source_lock().unwrap();
        let code = generate(&lock.wa_version).unwrap();
        assert!(code.contains("fn parse_response_with_context"));
        assert!(code.contains("fn handles_iq_errors"));
        assert!(code.contains("MakeGetCountryCodeRequestSpec"));
        assert!(generate("future").is_err());
        assert!(verify_projection("{}").is_err());
    }
}
