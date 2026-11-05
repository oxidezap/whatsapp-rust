//! Fabricated observations only. No APK runner or official behavior is exercised.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use mobile_oracle::{ComparisonStatus, compare_observations};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn blob(path: &str, bytes: &[u8]) -> Value {
    json!({"path": path, "size_bytes": bytes.len(), "sha256": hash(bytes)})
}

struct Fixture {
    dir: tempfile::TempDir,
    lock: PathBuf,
    corpus_path: PathBuf,
    apk_path: PathBuf,
    rust_path: PathBuf,
    corpus: Value,
    apk: Value,
    rust: Value,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let apk = b"synthetic APK bytes, not an Android application";
        let signature = b"synthetic untrusted signature evidence";
        let body = b"synthetic selector body";
        let input = b"synthetic input fixture";
        let output = b"{\"synthetic\":true,\"field\":null}";
        let clock = b"synthetic fixed clock profile";
        let randomness = b"synthetic deterministic randomness profile";
        let providers = b"synthetic provider results";
        let schema = b"synthetic opaque output encoding profile";
        for (path, bytes) in [
            ("base.apk", &apk[..]),
            ("signature.txt", &signature[..]),
            ("body.bin", &body[..]),
            ("input.bin", &input[..]),
            ("apk.out", &output[..]),
            ("rust.out", &output[..]),
        ] {
            fs::write(dir.path().join(path), bytes).unwrap();
        }
        for (path, bytes) in [
            ("clock.profile", &clock[..]),
            ("randomness.profile", &randomness[..]),
            ("providers.profile", &providers[..]),
            ("schema.profile", &schema[..]),
        ] {
            fs::write(dir.path().join(path), bytes).unwrap();
        }
        let mut artifacts = Vec::new();
        for (path, kind, bytes) in [
            ("base.apk", "base_apk", &apk[..]),
            ("signature.txt", "signature_report", &signature[..]),
            ("body.bin", "dex", &body[..]),
        ] {
            let mut entry = blob(path, bytes);
            entry["kind"] = json!(kind);
            artifacts.push(entry);
        }
        let lock = json!({
            "schema_version": 1, "package_name": "com.whatsapp", "version_name": "synthetic",
            "version_code": 1, "abi": "arm64-v8a", "min_sdk": 23, "target_sdk": 35,
            "distribution_channel": "synthetic", "declared_installation_complete": true,
            "acquisition": {"source_url": "https://example.invalid/synthetic", "acquired_at_utc": "2026-11-04T22:00:00Z", "access_conditions": "fabricated"},
            "signature": {"signer_certificate_sha256": "00".repeat(32), "trusted_signer_reference": "fabricated, not trusted", "verification_report_path": "signature.txt", "verifier_version": "synthetic"},
            "artifacts": artifacts,
            "extractions": [{"output_path": "body.bin", "input_apk_path": "base.apk", "input_apk_sha256": hash(apk), "archive_member": "classes.dex", "tool": {"name": "synthetic", "version": "1", "artifact_sha256": "11".repeat(32), "arguments": []}}],
            "selectors": [{"id": "synthetic-selector", "artifact_path": "body.bin", "offset_bytes": 0, "size_bytes": body.len(), "sha256": hash(body), "rationale": "fabricated byte range; not official semantics"}]
        });
        let lock_bytes = serde_json::to_vec(&lock).unwrap();
        let lock_path = dir.path().join("lock.json");
        fs::write(&lock_path, &lock_bytes).unwrap();
        let corpus = json!({
            "schema_version": 1, "synthetic_inputs": true, "lock_sha256": hash(&lock_bytes),
            "context": {"reference_abi": "arm64-v8a", "clock_profile_sha256": hash(clock), "randomness_profile_sha256": hash(randomness), "providers_profile_sha256": hash(providers), "observation_schema_sha256": hash(schema)},
            "profiles": {"clock": blob("clock.profile", clock), "randomness": blob("randomness.profile", randomness), "providers": blob("providers.profile", providers), "observation_schema": blob("schema.profile", schema)},
            "cases": [{"id": "synthetic-case", "selector_id": "synthetic-selector", "input": blob("input.bin", input)}]
        });
        let corpus_bytes = serde_json::to_vec(&corpus).unwrap();
        let observations = |role: &str, path: &str, implementation: String, target: &str| {
            json!({
                "schema_version": 1, "role": role,
                "lock_sha256": hash(&lock_bytes), "corpus_sha256": hash(&corpus_bytes),
                "context": corpus["context"],
                "provenance": {"runner": {"name": "synthetic-runner", "version": "1", "artifact_sha256": "66".repeat(32)}, "image": {"name": "synthetic-image", "version": "1", "artifact_sha256": "77".repeat(32)}, "implementation": {"name": "synthetic-implementation", "version": "1", "artifact_sha256": implementation}, "execution_target": target},
                "cases": [{"id": "synthetic-case", "input_sha256": hash(input), "selector_sha256": hash(body), "result": {"status": "completed", "output": blob(path, output)}}]
            })
        };
        let apk_report = observations("apk_reference", "apk.out", hash(apk), "arm64-v8a");
        let rust_report = observations(
            "rust_candidate",
            "rust.out",
            "88".repeat(32),
            "wasm32-unknown-unknown",
        );
        let mut fixture = Self {
            corpus_path: dir.path().join("corpus.json"),
            apk_path: dir.path().join("apk.json"),
            rust_path: dir.path().join("rust.json"),
            dir,
            lock: lock_path,
            corpus,
            apk: apk_report,
            rust: rust_report,
        };
        fixture.write();
        fixture
    }

    fn write(&mut self) {
        let bytes = serde_json::to_vec(&self.corpus).unwrap();
        let digest = hash(&bytes);
        self.apk["corpus_sha256"] = json!(digest);
        self.rust["corpus_sha256"] = json!(digest);
        fs::write(&self.corpus_path, bytes).unwrap();
        fs::write(&self.apk_path, serde_json::to_vec(&self.apk).unwrap()).unwrap();
        fs::write(&self.rust_path, serde_json::to_vec(&self.rust).unwrap()).unwrap();
    }

    fn compare(&self) -> anyhow::Result<mobile_oracle::ComparisonReport> {
        compare_observations(
            &self.lock,
            self.dir.path(),
            &self.corpus_path,
            &self.apk_path,
            &self.rust_path,
        )
    }

    fn cli(&self) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_mobile-oracle"))
            .arg("compare")
            .arg(&self.lock)
            .arg(self.dir.path())
            .arg(&self.corpus_path)
            .arg(&self.apk_path)
            .arg(&self.rust_path)
            .output()
            .unwrap()
    }
}

#[test]
fn agreement_is_derived_from_bytes_without_authenticating_execution() {
    let fixture = Fixture::new();
    let report = fixture.compare().unwrap();
    assert_eq!(report.status, ComparisonStatus::Pass);
    assert_eq!(report.scope, "supplied_observation_agreement");
    assert!(!report.qualified_execution);
    assert_eq!(
        report.execution_evidence,
        "operator_supplied_not_authenticated"
    );
    let cli = fixture.cli();
    assert!(
        cli.status.success(),
        "{}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let json: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(json["authenticity"], "requires_independent_review");
    assert_eq!(json["protocol_execution"], "not_run");
    assert_eq!(
        json["rust_provenance"]["execution_target"],
        "wasm32-unknown-unknown"
    );
}

#[test]
fn mismatch_preserves_exact_output_encoding_and_exits_nonzero() {
    let mut fixture = Fixture::new();
    let output = b"{\"synthetic\":true}"; // Omitted field differs from explicit null.
    fs::write(fixture.dir.path().join("rust.out"), output).unwrap();
    fixture.rust["cases"][0]["result"]["output"] = blob("rust.out", output);
    fixture.write();
    assert_eq!(
        fixture.compare().unwrap().status,
        ComparisonStatus::Mismatch
    );
    let cli = fixture.cli();
    assert_eq!(cli.status.code(), Some(2));
    let report: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(report["status"], "mismatch");
}

#[test]
fn equal_length_output_bytes_are_actually_compared() {
    let mut fixture = Fixture::new();
    let output = b"{\"synthetic\":true,\"field\":true}";
    assert_eq!(
        fs::metadata(fixture.dir.path().join("apk.out"))
            .unwrap()
            .len(),
        output.len() as u64
    );
    fs::write(fixture.dir.path().join("rust.out"), output).unwrap();
    fixture.rust["cases"][0]["result"]["output"] = blob("rust.out", output);
    fixture.write();
    assert_eq!(
        fixture.compare().unwrap().status,
        ComparisonStatus::Mismatch
    );
}

#[test]
fn repeated_blob_references_consume_the_read_budget() {
    let mut fixture = Fixture::new();
    let bytes = vec![b'x'; 64 * 1024];
    for path in ["input.bin", "apk.out", "rust.out"] {
        fs::write(fixture.dir.path().join(path), &bytes).unwrap();
    }
    let mut cases = Vec::new();
    let mut reference = Vec::new();
    let mut candidate = Vec::new();
    for sequence in 0..220 {
        let id = format!("synthetic-{sequence}");
        let mut case = fixture.corpus["cases"][0].clone();
        case["id"] = json!(id);
        case["input"] = blob("input.bin", &bytes);
        cases.push(case);
        for (template, output, destination) in [
            (&fixture.apk, "apk.out", &mut reference),
            (&fixture.rust, "rust.out", &mut candidate),
        ] {
            let mut observed = template["cases"][0].clone();
            observed["id"] = json!(id);
            observed["input_sha256"] = json!(hash(&bytes));
            observed["result"]["output"] = blob(output, &bytes);
            destination.push(observed);
        }
    }
    fixture.corpus["cases"] = json!(cases);
    fixture.apk["cases"] = json!(reference);
    fixture.rust["cases"] = json!(candidate);
    fixture.write();
    let error = fixture.compare().unwrap_err();
    assert!(
        error.to_string().contains("set exceeds budget"),
        "{error:#}"
    );
}

#[test]
fn unsupported_and_unrun_cannot_turn_a_ci_check_green() {
    for (declared, expected) in [
        ("unsupported", ComparisonStatus::Unsupported),
        ("not_run", ComparisonStatus::NotRun),
    ] {
        let mut fixture = Fixture::new();
        fixture.apk["cases"][0]["result"] =
            json!({"status": declared, "reason": "synthetic runner did not execute"});
        fixture.write();
        let report = fixture.compare().unwrap();
        assert_eq!(report.status, expected);
        assert_eq!(report.cases[0].status, expected);
        assert_eq!(fixture.cli().status.code(), Some(2));
    }
}

#[test]
fn corpus_order_does_not_hide_an_unexecuted_case_or_an_extra_case() {
    let mut fixture = Fixture::new();
    let mut second = fixture.corpus["cases"][0].clone();
    second["id"] = json!("another-synthetic-case");
    fixture.corpus["cases"].as_array_mut().unwrap().push(second);
    for report in [&mut fixture.apk, &mut fixture.rust] {
        let mut observed = report["cases"][0].clone();
        observed["id"] = json!("another-synthetic-case");
        observed["result"] =
            json!({"status": "unsupported", "reason": "synthetic missing provider"});
        report["cases"].as_array_mut().unwrap().push(observed);
    }
    fixture.apk["cases"].as_array_mut().unwrap().reverse();
    fixture.write();
    let report = fixture.compare().unwrap();
    assert_eq!(report.status, ComparisonStatus::Unsupported);
    assert_eq!(report.cases.len(), 2);
    assert_eq!(report.cases[0].id, "another-synthetic-case");
    assert_eq!(report.cases[1].status, ComparisonStatus::Pass);
    assert_eq!(fixture.cli().status.code(), Some(2));
    fixture.apk["cases"][0]["id"] = json!("case-not-in-corpus");
    fixture.write();
    assert!(fixture.compare().is_err());
    let extra = fixture.apk["cases"][0].clone();
    fixture.apk["cases"].as_array_mut().unwrap().push(extra);
    fixture.write();
    assert!(fixture.compare().is_err());
}

#[test]
fn reports_cannot_declare_pass_or_hide_missing_cases() {
    let mut fixture = Fixture::new();
    fixture.apk["qualified_execution"] = json!(true);
    fixture.write();
    assert!(fixture.compare().is_err());
    for result in [
        json!({"status": "pass"}),
        json!({"status": "not_run", "reason": "", "output": null}),
    ] {
        let mut fixture = Fixture::new();
        fixture.apk["cases"][0]["result"] = result;
        fixture.write();
        assert!(fixture.compare().is_err());
        assert!(!fixture.cli().status.success());
    }
    let mut fixture = Fixture::new();
    fixture.apk["cases"] = json!([]);
    fixture.write();
    assert!(fixture.compare().is_err());
    fixture.corpus["cases"] = json!([]);
    fixture.write();
    assert!(fixture.compare().is_err());
}

#[test]
fn actual_input_and_output_corruption_is_not_a_comparison_pass() {
    for path in [
        "input.bin",
        "apk.out",
        "body.bin",
        "base.apk",
        "clock.profile",
        "randomness.profile",
        "providers.profile",
        "schema.profile",
    ] {
        let fixture = Fixture::new();
        fs::write(fixture.dir.path().join(path), b"tampered bytes").unwrap();
        assert!(fixture.compare().is_err());
    }
}

#[test]
fn profiles_must_exist_match_context_and_stay_within_budget() {
    let mut fixture = Fixture::new();
    fs::remove_file(fixture.dir.path().join("clock.profile")).unwrap();
    assert!(fixture.compare().is_err());
    fixture = Fixture::new();
    fixture.corpus["profiles"]["clock"]["sha256"] = json!("00".repeat(32));
    fixture.write();
    assert!(fixture.compare().is_err());
    fixture = Fixture::new();
    fixture.corpus["profiles"]["clock"]["size_bytes"] = json!(64 * 1024 + 1);
    fixture.write();
    assert!(fixture.compare().is_err());
    fixture = Fixture::new();
    fs::write(fixture.dir.path().join("clock.profile"), b"").unwrap();
    fixture.corpus["profiles"]["clock"] = blob("clock.profile", b"");
    fixture.corpus["context"]["clock_profile_sha256"] = json!(hash(b""));
    fixture.apk["context"] = fixture.corpus["context"].clone();
    fixture.rust["context"] = fixture.corpus["context"].clone();
    fixture.write();
    assert!(fixture.compare().is_err());
}

#[test]
fn stale_roles_contexts_selectors_and_metadata_are_rejected() {
    for path in [
        "/role",
        "/lock_sha256",
        "/corpus_sha256",
        "/context/providers_profile_sha256",
        "/cases/0/input_sha256",
        "/cases/0/selector_sha256",
        "/provenance/execution_target",
        "/provenance/implementation/artifact_sha256",
    ] {
        let mut fixture = Fixture::new();
        *fixture.apk.pointer_mut(path).unwrap() = json!("wrong");
        // Write directly so the stale corpus digest is not automatically refreshed.
        fs::write(&fixture.apk_path, serde_json::to_vec(&fixture.apk).unwrap()).unwrap();
        assert!(fixture.compare().is_err(), "accepted mutation {path}");
    }
    let mut fixture = Fixture::new();
    fixture.corpus["synthetic_inputs"] = json!(false);
    fixture.write();
    assert!(fixture.compare().is_err());
}

#[test]
fn duplicates_extra_cases_unsafe_paths_and_limits_fail_closed() {
    let mut fixture = Fixture::new();
    fixture.corpus["cases"] = json!([fixture.corpus["cases"][0], fixture.corpus["cases"][0]]);
    fixture.apk["cases"] = json!([fixture.apk["cases"][0], fixture.apk["cases"][0]]);
    fixture.rust["cases"] = json!([fixture.rust["cases"][0], fixture.rust["cases"][0]]);
    fixture.write();
    assert!(fixture.compare().is_err());
    for path in [
        "../escape",
        "/absolute",
        "C:\\unsafe",
        "https://example.invalid/data",
    ] {
        let mut fixture = Fixture::new();
        fixture.corpus["cases"][0]["input"]["path"] = json!(path);
        fixture.write();
        assert!(fixture.compare().is_err());
    }
    let mut fixture = Fixture::new();
    fixture.corpus["cases"][0]["input"]["size_bytes"] = json!(16 * 1024 * 1024 + 1);
    fixture.write();
    assert!(fixture.compare().is_err());
    fs::write(&fixture.apk_path, vec![b' '; 4 * 1024 * 1024 + 1]).unwrap();
    assert!(fixture.compare().is_err());
}

#[cfg(unix)]
#[test]
fn input_symlinks_cannot_escape_the_root() {
    let mut fixture = Fixture::new();
    let outside = tempfile::NamedTempFile::new().unwrap();
    fs::write(outside.path(), b"synthetic input fixture").unwrap();
    std::os::unix::fs::symlink(outside.path(), fixture.dir.path().join("outside.bin")).unwrap();
    fixture.corpus["cases"][0]["input"]["path"] = json!("outside.bin");
    fixture.write();
    assert!(fixture.compare().is_err());
}
