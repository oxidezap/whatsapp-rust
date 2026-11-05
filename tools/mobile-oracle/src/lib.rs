//! Local integrity checks for explicitly acquired Android oracle evidence.
//!
//! This verifies the declared files, not APK authenticity, acquisition rights,
//! inventory completeness, extracted-file ancestry, or protocol conformance.
//! Signature and provenance fields record an operator's evidence and require
//! independent review before any artifact can be treated as an official oracle.

mod evidence;
pub use evidence::{
    ExtractionEvidence, ExtractionTool, SelectorChange, SelectorEvidence, VerifiedSelector,
};

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Take};
use std::path::{Component, Path};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const LOCK_LIMIT: u64 = 1024 * 1024;
const ARTIFACT_LIMIT: u64 = 2 * 1024 * 1024 * 1024;
const SET_LIMIT: u64 = 4 * 1024 * 1024 * 1024;
const ARTIFACT_COUNT_LIMIT: usize = 4096;

/// Versioned declaration; unknown fields and unknown schema versions fail.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ApkLock {
    pub schema_version: u32,
    pub package_name: String,
    pub version_name: String,
    pub version_code: u64,
    pub abi: String,
    pub min_sdk: u32,
    pub target_sdk: u32,
    pub distribution_channel: String,
    pub acquisition: Acquisition,
    pub signature: SignatureEvidence,
    /// The base and every split from one installation/version must be listed.
    /// This declaration is reviewed, not established by the integrity verifier.
    pub declared_installation_complete: bool,
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extractions: Vec<ExtractionEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selectors: Vec<SelectorEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Acquisition {
    /// Credential-free HTTPS origin. Queries and fragments are not accepted.
    pub source_url: String,
    /// Human-readable UTC acquisition timestamp, e.g. 2026-11-04T22:00:00Z.
    pub acquired_at_utc: String,
    /// Acquisition/access conditions, without tokens or private session data.
    pub access_conditions: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SignatureEvidence {
    pub signer_certificate_sha256: String,
    /// Independently reviewed reference for this signer and any key rotation.
    pub trusted_signer_reference: String,
    /// Path of a locked artifact containing `apksigner verify` output.
    pub verification_report_path: String,
    pub verifier_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Artifact {
    pub path: String,
    pub kind: ArtifactKind,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    BaseApk,
    SplitApk,
    Dex,
    NativeLibrary,
    Resources,
    SignatureReport,
}

/// Integrity report only. The execution status cannot be promoted by a lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerificationReport {
    pub schema_version: u32,
    pub lock_sha256: String,
    pub artifact_integrity: IntegrityStatus,
    pub authenticity: AuthenticityStatus,
    pub protocol_execution: ExecutionStatus,
    pub verified_artifacts: Vec<VerifiedArtifact>,
    pub verified_selectors: Vec<VerifiedSelector>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrityStatus {
    Verified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthenticityStatus {
    RequiresIndependentReview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedArtifact {
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

fn nonempty(value: &str, field: &str) -> Result<()> {
    ensure!(!value.trim().is_empty(), "{field} must not be empty");
    ensure!(
        !value.chars().any(char::is_control),
        "{field} contains control characters"
    );
    Ok(())
}

fn digest_valid(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn validate_relative(path: &str) -> Result<()> {
    nonempty(path, "artifact path")?;
    ensure!(
        !path.contains('\\'),
        "artifact path must use portable forward slashes"
    );
    ensure!(
        Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "artifact path must be a relative path without traversal"
    );
    ensure!(
        !path.contains(':'),
        "artifact path cannot contain a drive or URI scheme"
    );
    Ok(())
}

impl ApkLock {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported APK lock schema version"
        );
        ensure!(
            matches!(
                self.package_name.as_str(),
                "com.whatsapp" | "com.whatsapp.w4b"
            ),
            "unsupported APK package"
        );
        nonempty(&self.version_name, "version_name")?;
        ensure!(self.version_code != 0, "version_code must be positive");
        ensure!(
            matches!(
                self.abi.as_str(),
                "arm64-v8a" | "armeabi-v7a" | "x86" | "x86_64"
            ),
            "unsupported ABI"
        );
        ensure!(
            self.min_sdk > 0 && self.min_sdk <= self.target_sdk,
            "invalid SDK range"
        );
        nonempty(&self.distribution_channel, "distribution_channel")?;
        nonempty(&self.acquisition.source_url, "source_url")?;
        ensure!(
            !self.acquisition.source_url.chars().any(char::is_whitespace),
            "acquisition URL must not contain whitespace"
        );
        let origin =
            url::Url::parse(&self.acquisition.source_url).context("invalid acquisition URL")?;
        ensure!(
            origin.scheme() == "https"
                && origin.host_str().is_some()
                && origin.username().is_empty()
                && origin.password().is_none()
                && origin.query().is_none()
                && origin.fragment().is_none(),
            "acquisition URL must be credential-free HTTPS without a query or fragment"
        );
        nonempty(&self.acquisition.acquired_at_utc, "acquired_at_utc")?;
        chrono::DateTime::parse_from_rfc3339(&self.acquisition.acquired_at_utc)
            .context("invalid acquisition timestamp")?;
        ensure!(
            self.acquisition.acquired_at_utc.ends_with('Z'),
            "acquisition timestamp must explicitly use UTC"
        );
        nonempty(&self.acquisition.access_conditions, "access_conditions")?;
        ensure!(
            digest_valid(&self.signature.signer_certificate_sha256),
            "invalid signer certificate SHA-256"
        );
        nonempty(
            &self.signature.trusted_signer_reference,
            "trusted_signer_reference",
        )?;
        nonempty(&self.signature.verifier_version, "verifier_version")?;
        validate_relative(&self.signature.verification_report_path)?;
        ensure!(
            self.declared_installation_complete,
            "incomplete installation declaration"
        );
        ensure!(
            !self.artifacts.is_empty() && self.artifacts.len() <= ARTIFACT_COUNT_LIMIT,
            "invalid artifact count"
        );
        let mut paths = BTreeSet::new();
        let mut total = 0_u64;
        let mut bases = 0;
        let mut signature_report = false;
        for artifact in &self.artifacts {
            validate_relative(&artifact.path)?;
            ensure!(
                paths.insert(&artifact.path),
                "duplicate artifact path: {}",
                artifact.path
            );
            ensure!(
                artifact.size_bytes > 0 && artifact.size_bytes <= ARTIFACT_LIMIT,
                "invalid artifact size: {}",
                artifact.path
            );
            ensure!(
                digest_valid(&artifact.sha256),
                "invalid artifact SHA-256: {}",
                artifact.path
            );
            total = total
                .checked_add(artifact.size_bytes)
                .context("artifact set size overflow")?;
            ensure!(total <= SET_LIMIT, "artifact set exceeds size budget");
            bases += usize::from(artifact.kind == ArtifactKind::BaseApk);
            signature_report |= artifact.path == self.signature.verification_report_path
                && artifact.kind == ArtifactKind::SignatureReport;
        }
        ensure!(bases == 1, "exactly one base APK is required");
        ensure!(
            signature_report,
            "signature verification report must be a locked artifact"
        );
        evidence::validate(self)?;
        Ok(())
    }
}

fn hash_reader(reader: &mut Take<File>) -> Result<(u64, String)> {
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    let mut count = 0_u64;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count += read as u64;
        digest.update(&buffer[..read]);
    }
    Ok((count, hex::encode(digest.finalize())))
}

fn read_lock(lock_path: &Path) -> Result<(ApkLock, String)> {
    ensure!(
        std::fs::metadata(lock_path)
            .context("reading APK lock metadata")?
            .is_file(),
        "APK lock must be a regular file"
    );
    let lock_file = File::open(lock_path).context("opening APK lock")?;
    ensure!(
        lock_file.metadata()?.is_file(),
        "APK lock must be a regular file"
    );
    let mut bytes = Vec::new();
    lock_file.take(LOCK_LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= LOCK_LIMIT,
        "APK lock exceeds size budget"
    );
    let lock: ApkLock = serde_json::from_slice(&bytes).context("parsing APK lock")?;
    lock.validate()?;
    Ok((lock, hex::encode(Sha256::digest(&bytes))))
}

/// Snapshot identity declarations, not independently established APK metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotIdentity {
    pub package_name: String,
    pub version_name: String,
    pub version_code: u64,
    pub abi: String,
    pub min_sdk: u32,
    pub target_sdk: u32,
    pub distribution_channel: String,
    pub signer_certificate_sha256: String,
}

impl From<&ApkLock> for SnapshotIdentity {
    fn from(lock: &ApkLock) -> Self {
        Self {
            package_name: lock.package_name.clone(),
            version_name: lock.version_name.clone(),
            version_code: lock.version_code,
            abi: lock.abi.clone(),
            min_sdk: lock.min_sdk,
            target_sdk: lock.target_sdk,
            distribution_channel: lock.distribution_channel.clone(),
            signer_certificate_sha256: lock.signature.signer_certificate_sha256.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtifactChange {
    pub path: String,
    pub before: Option<Artifact>,
    pub after: Option<Artifact>,
}

/// A declaration diff never executes artifacts or rewrites either lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DriftReport {
    pub schema_version: u32,
    pub before_lock_sha256: String,
    pub after_lock_sha256: String,
    pub declarations_changed: bool,
    pub before: SnapshotIdentity,
    pub after: SnapshotIdentity,
    pub artifact_changes: Vec<ArtifactChange>,
    pub extraction_provenance_changed: bool,
    pub selector_changes: Vec<SelectorChange>,
    pub protocol_execution: ExecutionStatus,
}

/// Compare validated lock declarations without opening or executing artifacts.
///
/// Artifact integrity must be checked separately with [`verify`]. Every changed
/// artifact needs selector/expectation requalification; this diff establishes no
/// behavioral equivalence and does not automatically update golden files.
pub fn compare_locks(before_path: &Path, after_path: &Path) -> Result<DriftReport> {
    let (before, before_lock_sha256) = read_lock(before_path)?;
    let (after, after_lock_sha256) = read_lock(after_path)?;
    let before_artifacts: BTreeMap<_, _> = before.artifacts.iter().map(|a| (&a.path, a)).collect();
    let after_artifacts: BTreeMap<_, _> = after.artifacts.iter().map(|a| (&a.path, a)).collect();
    let paths: BTreeSet<_> = before_artifacts
        .keys()
        .chain(after_artifacts.keys())
        .copied()
        .collect();
    let mut artifact_changes = Vec::new();
    for path in paths {
        let previous = before_artifacts.get(path).copied();
        let next = after_artifacts.get(path).copied();
        if previous != next {
            artifact_changes.push(ArtifactChange {
                path: path.clone(),
                before: previous.cloned(),
                after: next.cloned(),
            });
        }
    }
    let mut normalized_before = before.clone();
    let mut normalized_after = after.clone();
    normalized_before
        .artifacts
        .sort_by(|a, b| a.path.cmp(&b.path));
    normalized_after
        .artifacts
        .sort_by(|a, b| a.path.cmp(&b.path));
    evidence::normalize(&mut normalized_before);
    evidence::normalize(&mut normalized_after);
    let extraction_provenance_changed =
        normalized_before.extractions != normalized_after.extractions;
    let declarations_changed = normalized_before != normalized_after;
    Ok(DriftReport {
        schema_version: 1,
        before_lock_sha256,
        after_lock_sha256,
        declarations_changed,
        before: SnapshotIdentity::from(&before),
        after: SnapshotIdentity::from(&after),
        artifact_changes,
        extraction_provenance_changed,
        selector_changes: evidence::selector_changes(&before, &after),
        protocol_execution: ExecutionStatus::NotRun,
    })
}

fn open_artifact(root: &Path, artifact: &Artifact) -> Result<File> {
    let path = root
        .join(&artifact.path)
        .canonicalize()
        .with_context(|| format!("resolving artifact {}", artifact.path))?;
    ensure!(
        path.starts_with(root),
        "artifact escapes root: {}",
        artifact.path
    );
    ensure!(
        std::fs::metadata(&path)?.is_file(),
        "artifact is not a regular file: {}",
        artifact.path
    );
    let file = File::open(path).with_context(|| format!("opening artifact {}", artifact.path))?;
    let metadata = file.metadata()?;
    ensure!(
        metadata.is_file(),
        "artifact is not a regular file: {}",
        artifact.path
    );
    ensure!(
        metadata.len() == artifact.size_bytes,
        "artifact size mismatch: {}",
        artifact.path
    );
    Ok(file)
}

/// Verify all declared artifacts under one local root with bounded streaming.
///
/// Missing/corrupt files and escaping symlinks are errors. The artifact directory
/// must be a stable, operator-controlled snapshot; this is not a sandbox for a
/// directory that another process can mutate concurrently.
pub fn verify(lock_path: &Path, artifact_root: &Path) -> Result<VerificationReport> {
    let (lock, lock_sha256) = read_lock(lock_path)?;
    let root = artifact_root
        .canonicalize()
        .context("resolving artifact root")?;
    ensure!(root.is_dir(), "artifact root must be a directory");
    let mut verified_artifacts = Vec::with_capacity(lock.artifacts.len());
    for artifact in &lock.artifacts {
        let file = open_artifact(&root, artifact)?;
        let (size_bytes, sha256) = hash_reader(&mut file.take(artifact.size_bytes + 1))?;
        if size_bytes != artifact.size_bytes || sha256 != artifact.sha256 {
            bail!("artifact digest/size mismatch: {}", artifact.path);
        }
        verified_artifacts.push(VerifiedArtifact {
            path: artifact.path.clone(),
            size_bytes,
            sha256,
        });
    }
    let verified_selectors = evidence::verify_selectors(&lock, &root)?;
    verified_artifacts.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(VerificationReport {
        schema_version: 1,
        lock_sha256,
        artifact_integrity: IntegrityStatus::Verified,
        authenticity: AuthenticityStatus::RequiresIndependentReview,
        protocol_execution: ExecutionStatus::NotRun,
        verified_artifacts,
        verified_selectors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, ApkLock) {
        let dir = tempfile::tempdir().unwrap();
        let files = [
            (
                "base.apk",
                ArtifactKind::BaseApk,
                "synthetic APK bytes, not an official APK",
            ),
            (
                "signature.txt",
                ArtifactKind::SignatureReport,
                "synthetic signature report, not verified",
            ),
        ];
        let artifacts = files
            .into_iter()
            .map(|(path, kind, content)| {
                fs::write(dir.path().join(path), content).unwrap();
                Artifact {
                    path: path.into(),
                    kind,
                    size_bytes: content.len() as u64,
                    sha256: hex::encode(Sha256::digest(content.as_bytes())),
                }
            })
            .collect();
        let lock = ApkLock {
            schema_version: 1,
            package_name: "com.whatsapp".into(),
            version_name: "synthetic-1".into(),
            version_code: 1,
            abi: "arm64-v8a".into(),
            min_sdk: 23,
            target_sdk: 35,
            distribution_channel: "synthetic".into(),
            acquisition: Acquisition {
                source_url: "https://example.invalid/test-artifact".into(),
                acquired_at_utc: "2026-11-04T22:00:00Z".into(),
                access_conditions: "Synthetic local test only".into(),
            },
            signature: SignatureEvidence {
                signer_certificate_sha256: "00".repeat(32),
                trusted_signer_reference: "Synthetic signer, no authenticity established".into(),
                verification_report_path: "signature.txt".into(),
                verifier_version: "synthetic".into(),
            },
            declared_installation_complete: true,
            artifacts,
            extractions: Vec::new(),
            selectors: Vec::new(),
        };
        (dir, lock)
    }

    fn write_lock(dir: &TempDir, lock: &ApkLock) -> std::path::PathBuf {
        let path = dir.path().join("apk.lock.json");
        fs::write(&path, serde_json::to_vec_pretty(lock).unwrap()).unwrap();
        path
    }

    #[test]
    fn verifies_declared_bytes_without_claiming_authenticity_or_execution() {
        let (dir, mut lock) = fixture();
        lock.artifacts.reverse();
        let path = write_lock(&dir, &lock);
        let report = verify(&path, dir.path()).unwrap();
        assert_eq!(report, verify(&path, dir.path()).unwrap());
        assert_eq!(report.protocol_execution, ExecutionStatus::NotRun);
        assert_eq!(
            report.authenticity,
            AuthenticityStatus::RequiresIndependentReview
        );
        assert_eq!(report.verified_artifacts[0].path, "base.apk");
    }

    #[test]
    fn rejects_corruption_and_missing_files() {
        let (dir, lock) = fixture();
        let path = write_lock(&dir, &lock);
        let original = fs::read(dir.path().join("base.apk")).unwrap();
        fs::write(dir.path().join("base.apk"), vec![b'x'; original.len()]).unwrap();
        assert!(
            verify(&path, dir.path())
                .unwrap_err()
                .to_string()
                .contains("mismatch")
        );
        fs::remove_file(dir.path().join("base.apk")).unwrap();
        assert!(verify(&path, dir.path()).is_err());
    }

    #[test]
    fn rejects_unlocked_signature_report_and_incomplete_installation() {
        let (_, mut lock) = fixture();
        lock.signature.verification_report_path = "other.txt".into();
        assert!(lock.validate().is_err());
        lock.signature.verification_report_path = "signature.txt".into();
        lock.declared_installation_complete = false;
        assert!(lock.validate().is_err());
    }

    #[test]
    fn rejects_unknown_schema_and_unknown_fields() {
        let (dir, mut lock) = fixture();
        lock.schema_version = 2;
        let path = write_lock(&dir, &lock);
        assert!(verify(&path, dir.path()).is_err());
        let mut value = serde_json::to_value(fixture().1).unwrap();
        value["protocol_execution"] = "pass".into();
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(verify(&path, dir.path()).is_err());
    }

    #[test]
    fn rejects_unsafe_paths_duplicates_and_invalid_digests() {
        for path in [
            "../base.apk",
            "/base.apk",
            "a/../../base.apk",
            "C:\\base.apk",
            "a\\base.apk",
            "./base.apk",
        ] {
            let (_, mut lock) = fixture();
            lock.artifacts[0].path = path.into();
            assert!(lock.validate().is_err(), "accepted {path}");
        }
        let (_, mut lock) = fixture();
        lock.artifacts.push(lock.artifacts[0].clone());
        assert!(lock.validate().is_err());
        let (_, mut lock) = fixture();
        lock.artifacts[0].sha256 = "bad".into();
        assert!(lock.validate().is_err());
    }

    #[test]
    fn rejects_unbounded_artifact_and_manifest_sizes() {
        let (dir, mut lock) = fixture();
        lock.artifacts[0].size_bytes = ARTIFACT_LIMIT + 1;
        assert!(lock.validate().is_err());
        let path = dir.path().join("large.json");
        fs::write(&path, vec![b' '; LOCK_LIMIT as usize + 1]).unwrap();
        assert!(verify(&path, dir.path()).is_err());
    }

    #[test]
    fn rejects_credentials_in_provenance_url() {
        for source in [
            "http://example.invalid/a",
            "https://name:secret@example.invalid/a",
            "https://example.invalid/a?token=secret",
            "https://example.invalid/a#token",
        ] {
            let (_, mut lock) = fixture();
            lock.acquisition.source_url = source.into();
            assert!(lock.validate().is_err());
        }
    }

    #[test]
    fn reports_declared_drift_without_mutating_locks_or_executing_artifacts() {
        let (dir, lock) = fixture();
        let before_path = write_lock(&dir, &lock);
        let unchanged = compare_locks(&before_path, &before_path).unwrap();
        assert!(!unchanged.declarations_changed);
        assert!(unchanged.artifact_changes.is_empty());
        let mut after = lock.clone();
        after.version_code += 1;
        after.artifacts[0].sha256 = "11".repeat(32);
        after.artifacts.push(Artifact {
            path: "split.apk".into(),
            kind: ArtifactKind::SplitApk,
            size_bytes: 1,
            sha256: "22".repeat(32),
        });
        let after_path = dir.path().join("next.lock.json");
        let encoded = serde_json::to_vec(&after).unwrap();
        fs::write(&after_path, &encoded).unwrap();
        let diff = compare_locks(&before_path, &after_path).unwrap();
        assert!(diff.declarations_changed);
        assert_eq!(diff.artifact_changes.len(), 2);
        assert_eq!(diff.artifact_changes[0].path, "base.apk");
        assert!(diff.artifact_changes[1].before.is_none());
        assert_eq!(diff.protocol_execution, ExecutionStatus::NotRun);
        assert_eq!(fs::read(after_path).unwrap(), encoded);
        assert!(verify(&dir.path().join("next.lock.json"), dir.path()).is_err());
        assert_eq!(
            verify(&before_path, dir.path())
                .unwrap()
                .verified_artifacts
                .len(),
            2
        );
    }

    #[test]
    fn artifact_reordering_does_not_report_semantic_drift() {
        let (dir, mut lock) = fixture();
        let before_path = write_lock(&dir, &lock);
        lock.artifacts.reverse();
        let after_path = dir.path().join("reordered.lock.json");
        fs::write(&after_path, serde_json::to_vec(&lock).unwrap()).unwrap();
        let diff = compare_locks(&before_path, &after_path).unwrap();
        assert_ne!(diff.before_lock_sha256, diff.after_lock_sha256);
        assert!(!diff.declarations_changed);
        assert!(diff.artifact_changes.is_empty());
    }

    #[test]
    fn rejects_invalid_timestamp_and_sdk_range() {
        let (_, mut lock) = fixture();
        lock.acquisition.acquired_at_utc = "invalidZ".into();
        assert!(lock.validate().is_err());
        lock.acquisition.acquired_at_utc = "2026-11-04T22:00:00Z".into();
        lock.min_sdk = lock.target_sdk + 1;
        assert!(lock.validate().is_err());
    }

    fn extraction_fixture() -> (TempDir, ApkLock) {
        let (dir, mut lock) = fixture();
        let data = b"synthetic-header:method-body:synthetic-tail";
        fs::write(dir.path().join("classes.dex"), data).unwrap();
        lock.artifacts.push(Artifact {
            path: "classes.dex".into(),
            kind: ArtifactKind::Dex,
            size_bytes: data.len() as u64,
            sha256: hex::encode(Sha256::digest(data)),
        });
        lock.extractions.push(ExtractionEvidence {
            output_path: "classes.dex".into(),
            input_apk_path: "base.apk".into(),
            input_apk_sha256: lock.artifacts[0].sha256.clone(),
            archive_member: "classes.dex".into(),
            tool: ExtractionTool {
                name: "synthetic-extractor".into(),
                version: "1".into(),
                artifact_sha256: "33".repeat(32),
                arguments: vec!["base.apk".into()],
            },
        });
        lock.selectors.push(SelectorEvidence {
            id: "synthetic-method".into(),
            artifact_path: "classes.dex".into(),
            offset_bytes: 17,
            size_bytes: 11,
            sha256: hex::encode(Sha256::digest(&data[17..28])),
            rationale: "Synthetic byte range only; no real DEX method or protocol behavior".into(),
        });
        (dir, lock)
    }

    #[test]
    fn verifies_exact_selector_bytes_without_claiming_method_execution() {
        let (dir, lock) = extraction_fixture();
        let path = write_lock(&dir, &lock);
        let report = verify(&path, dir.path()).unwrap();
        assert_eq!(report.verified_selectors.len(), 1);
        assert_eq!(
            report.verified_selectors[0].sha256,
            lock.selectors[0].sha256
        );
        assert_eq!(report.protocol_execution, ExecutionStatus::NotRun);
    }

    #[test]
    fn rejects_stale_selector_fingerprint_even_when_whole_file_hash_matches() {
        let (dir, mut lock) = extraction_fixture();
        lock.selectors[0].sha256 = "44".repeat(32);
        let path = write_lock(&dir, &lock);
        let error = verify(&path, dir.path()).unwrap_err();
        assert!(error.to_string().contains("selector fingerprint mismatch"));
    }

    #[test]
    fn rejects_selector_overflow_out_of_bounds_and_index_only_evidence() {
        for (offset, size) in [(u64::MAX, 1), (0, 1000), (0, 0)] {
            let (_, mut lock) = extraction_fixture();
            lock.selectors[0].offset_bytes = offset;
            lock.selectors[0].size_bytes = size;
            assert!(lock.validate().is_err());
        }
        let (_, mut lock) = extraction_fixture();
        lock.selectors[0].sha256.clear();
        assert!(lock.validate().is_err());
        let (_, mut lock) = extraction_fixture();
        lock.selectors[0].rationale.clear();
        assert!(lock.validate().is_err());
    }

    #[test]
    fn requires_lineage_for_derived_files_and_rejects_stale_source_or_tool_hash() {
        let (_, mut lock) = extraction_fixture();
        lock.extractions.clear();
        assert!(lock.validate().is_err());
        let (_, mut lock) = extraction_fixture();
        lock.extractions[0].input_apk_sha256 = "55".repeat(32);
        assert!(lock.validate().is_err());
        let (_, mut lock) = extraction_fixture();
        lock.extractions[0].tool.artifact_sha256 = "missing".into();
        assert!(lock.validate().is_err());
        let (_, mut lock) = extraction_fixture();
        lock.extractions[0].archive_member = "../classes.dex".into();
        assert!(lock.validate().is_err());
    }

    #[test]
    fn reports_selector_and_tool_drift_without_artifact_changes() {
        let (dir, lock) = extraction_fixture();
        let before = write_lock(&dir, &lock);
        let mut after = lock.clone();
        after.selectors[0].rationale = "Changed analysis requires requalification".into();
        after.extractions[0].tool.version = "2".into();
        let after_path = dir.path().join("after.lock.json");
        fs::write(&after_path, serde_json::to_vec(&after).unwrap()).unwrap();
        let report = compare_locks(&before, &after_path).unwrap();
        assert!(report.declarations_changed);
        assert!(report.extraction_provenance_changed);
        assert_eq!(report.selector_changes.len(), 1);
        assert!(report.artifact_changes.is_empty());
        assert_eq!(report.protocol_execution, ExecutionStatus::NotRun);
    }

    #[test]
    fn evidence_record_order_is_not_semantic_drift() {
        let (dir, mut lock) = extraction_fixture();
        let mut second_artifact = lock.artifacts[2].clone();
        second_artifact.path = "classes2.dex".into();
        lock.artifacts.push(second_artifact);
        let mut second_extraction = lock.extractions[0].clone();
        second_extraction.output_path = "classes2.dex".into();
        second_extraction.archive_member = "classes2.dex".into();
        lock.extractions.push(second_extraction);
        let mut second_selector = lock.selectors[0].clone();
        second_selector.id = "other-method".into();
        second_selector.artifact_path = "classes2.dex".into();
        lock.selectors.push(second_selector);
        let before = write_lock(&dir, &lock);
        lock.extractions.reverse();
        lock.selectors.reverse();
        let after = dir.path().join("reordered-evidence.lock.json");
        fs::write(&after, serde_json::to_vec(&lock).unwrap()).unwrap();
        let report = compare_locks(&before, &after).unwrap();
        assert!(!report.declarations_changed);
        assert!(!report.extraction_provenance_changed);
        assert!(report.selector_changes.is_empty());
    }

    #[test]
    fn rejects_hidden_controls_in_provenance_url_before_url_normalization() {
        let (_, mut lock) = fixture();
        for source in [
            "https://example.invalid/\nartifact",
            " https://example.invalid/artifact",
            "https://example.invalid/artifact ",
        ] {
            lock.acquisition.source_url = source.into();
            assert!(
                lock.validate().is_err(),
                "accepted nonliteral URL {source:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        let (dir, lock) = fixture();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("base.apk"), "outside").unwrap();
        fs::remove_file(dir.path().join("base.apk")).unwrap();
        std::os::unix::fs::symlink(outside.path().join("base.apk"), dir.path().join("base.apk"))
            .unwrap();
        let path = write_lock(&dir, &lock);
        assert!(
            verify(&path, dir.path())
                .unwrap_err()
                .to_string()
                .contains("escapes")
        );
    }
}
