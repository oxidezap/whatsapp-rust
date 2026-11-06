use std::fs;
use std::process::Command;

use serde_json::json;
use sha2::{Digest, Sha256};

#[test]
fn cli_verifies_and_diffs_synthetic_declarations_without_execution() {
    let dir = tempfile::tempdir().unwrap();
    let data = b"synthetic bytes; not a real APK";
    fs::write(dir.path().join("base.apk"), data).unwrap();
    fs::write(dir.path().join("signature.txt"), data).unwrap();
    let sha256 = hex::encode(Sha256::digest(data));
    let lock = json!({
        "schema_version": 1,
        "package_name": "com.whatsapp",
        "version_name": "synthetic",
        "version_code": 1,
        "abi": "arm64-v8a",
        "min_sdk": 23,
        "target_sdk": 35,
        "distribution_channel": "synthetic",
        "acquisition": {
            "source_url": "https://example.invalid/synthetic",
            "acquired_at_utc": "2026-11-04T22:00:00Z",
            "access_conditions": "synthetic only"
        },
        "signature": {
            "signer_certificate_sha256": "00".repeat(32),
            "trusted_signer_reference": "synthetic, not trusted",
            "verification_report_path": "signature.txt",
            "verifier_version": "synthetic"
        },
        "declared_installation_complete": true,
        "artifacts": [
            { "path": "base.apk", "kind": "base_apk", "size_bytes": data.len(), "sha256": sha256 },
            { "path": "signature.txt", "kind": "signature_report", "size_bytes": data.len(), "sha256": sha256 }
        ]
    });
    let lock_path = dir.path().join("apk.lock.json");
    fs::write(&lock_path, serde_json::to_vec(&lock).unwrap()).unwrap();
    let verify = Command::new(env!("CARGO_BIN_EXE_mobile-oracle"))
        .arg("verify")
        .arg(&lock_path)
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(verify.status.success(), "{:?}", verify);
    let report: serde_json::Value = serde_json::from_slice(&verify.stdout).unwrap();
    assert_eq!(report["protocol_execution"], "not_run");
    assert_eq!(report["authenticity"], "requires_independent_review");
    let diff = Command::new(env!("CARGO_BIN_EXE_mobile-oracle"))
        .arg("diff")
        .arg(&lock_path)
        .arg(&lock_path)
        .output()
        .unwrap();
    assert!(diff.status.success());
    let report: serde_json::Value = serde_json::from_slice(&diff.stdout).unwrap();
    assert_eq!(report["declarations_changed"], false);
    assert_eq!(report["protocol_execution"], "not_run");

    fs::remove_file(dir.path().join("base.apk")).unwrap();
    let missing = Command::new(env!("CARGO_BIN_EXE_mobile-oracle"))
        .arg("verify")
        .arg(&lock_path)
        .arg(dir.path())
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
}

#[test]
fn invalid_invocations_fail_without_success_output() {
    for arguments in [
        vec![],
        vec!["unknown", "a", "b"],
        vec!["verify", "a", "b", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mobile-oracle"))
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
    }
}
