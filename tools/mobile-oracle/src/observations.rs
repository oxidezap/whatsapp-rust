//! Compare supplied offline observations; never authenticate execution or run a client.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    Artifact, ArtifactKind, AuthenticityStatus, ExecutionStatus, digest_valid, hash_reader,
    nonempty, open_artifact, read_lock, validate_relative,
};

const DOCUMENT_LIMIT: u64 = 4 * 1024 * 1024;
const BLOB_LIMIT: u64 = 16 * 1024 * 1024;
const BLOB_SET_LIMIT: u64 = 64 * 1024 * 1024;
const CASE_LIMIT: usize = 1024;
const PROFILE_LIMIT: u64 = 64 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    schema_version: u32,
    synthetic_inputs: bool,
    lock_sha256: String,
    context: ComparisonContext,
    profiles: Profiles,
    cases: Vec<CorpusCase>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Profiles {
    clock: Blob,
    randomness: Blob,
    providers: Blob,
    observation_schema: Blob,
}

/// Profile identities are operator declarations, not proof that a runner applied them.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ComparisonContext {
    reference_abi: String,
    clock_profile_sha256: String,
    randomness_profile_sha256: String,
    providers_profile_sha256: String,
    observation_schema_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CorpusCase {
    id: String,
    selector_id: String,
    input: Blob,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Blob {
    path: String,
    size_bytes: u64,
    sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observations {
    schema_version: u32,
    role: Role,
    lock_sha256: String,
    corpus_sha256: String,
    context: ComparisonContext,
    provenance: Provenance,
    cases: Vec<Observation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Role {
    ApkReference,
    RustCandidate,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    runner: ToolIdentity,
    image: ToolIdentity,
    implementation: ToolIdentity,
    /// Producer-declared execution target, which can differ for portable Rust.
    execution_target: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ToolIdentity {
    name: String,
    version: String,
    artifact_sha256: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    id: String,
    input_sha256: String,
    selector_sha256: String,
    result: ObservationResult,
}

/// Input cannot declare a pass. Only the comparator decides byte agreement.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
enum ObservationResult {
    Completed { output: Blob },
    NotRun { reason: String },
    Unsupported { reason: String },
}

/// A pass is exact agreement of supplied observations, never protocol conformance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    Pass,
    Mismatch,
    Unsupported,
    NotRun,
}

#[derive(Debug, Serialize)]
pub struct ComparisonReport {
    pub schema_version: u32,
    pub scope: &'static str,
    pub status: ComparisonStatus,
    pub qualified_execution: bool,
    pub execution_evidence: &'static str,
    pub profile_integrity: &'static str,
    pub authenticity: AuthenticityStatus,
    pub protocol_execution: ExecutionStatus,
    pub lock_sha256: String,
    pub corpus_sha256: String,
    pub apk_observations_sha256: String,
    pub rust_observations_sha256: String,
    context: ComparisonContext,
    apk_provenance: Provenance,
    rust_provenance: Provenance,
    pub cases: Vec<CaseComparison>,
}

#[derive(Debug, Serialize)]
pub struct CaseComparison {
    pub id: String,
    pub selector_id: String,
    pub input_sha256: String,
    pub status: ComparisonStatus,
    pub apk_status: &'static str,
    pub rust_status: &'static str,
}

fn text(value: &str, field: &str) -> Result<()> {
    nonempty(value, field)?;
    ensure!(value.len() <= 256, "{field} exceeds length budget");
    Ok(())
}

fn digest(value: &str) -> Result<()> {
    ensure!(digest_valid(value), "invalid observation SHA-256");
    Ok(())
}

fn document<T: serde::de::DeserializeOwned>(path: &Path) -> Result<(T, String)> {
    let file = File::open(path).context("opening observation document")?;
    ensure!(
        file.metadata()?.is_file(),
        "observation document must be a regular file"
    );
    let mut bytes = Vec::new();
    file.take(DOCUMENT_LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= DOCUMENT_LIMIT,
        "observation document exceeds budget"
    );
    let value = serde_json::from_slice(&bytes).context("parsing observation document")?;
    Ok((value, hex::encode(Sha256::digest(&bytes))))
}

impl ComparisonContext {
    fn validate(&self) -> Result<()> {
        text(&self.reference_abi, "target ABI")?;
        for hash in [
            &self.clock_profile_sha256,
            &self.randomness_profile_sha256,
            &self.providers_profile_sha256,
            &self.observation_schema_sha256,
        ] {
            digest(hash)?;
        }
        Ok(())
    }
}

impl Provenance {
    fn validate(&self) -> Result<()> {
        text(&self.execution_target, "runtime ABI")?;
        for tool in [&self.runner, &self.image, &self.implementation] {
            text(&tool.name, "tool name")?;
            text(&tool.version, "tool version")?;
            digest(&tool.artifact_sha256)?;
        }
        Ok(())
    }
}

fn blob_file(blob: &Blob, root: &Path, budget: &mut u64) -> Result<File> {
    validate_relative(&blob.path)?;
    digest(&blob.sha256)?;
    ensure!(
        blob.size_bytes <= BLOB_LIMIT,
        "observation blob exceeds budget"
    );
    charge(budget, blob.size_bytes)?;
    let artifact = Artifact {
        path: blob.path.clone(),
        kind: ArtifactKind::Resources,
        size_bytes: blob.size_bytes,
        sha256: blob.sha256.clone(),
    };
    let file = open_artifact(root, &artifact)?;
    let (size, hash) = hash_reader(&mut file.take(blob.size_bytes + 1))?;
    ensure!(
        size == blob.size_bytes && hash == blob.sha256,
        "observation blob bytes differ"
    );
    open_artifact(root, &artifact)
}

fn charge(budget: &mut u64, size: u64) -> Result<()> {
    *budget = budget
        .checked_add(size)
        .context("observation budget overflow")?;
    ensure!(
        *budget <= BLOB_SET_LIMIT,
        "observation blob set exceeds budget"
    );
    Ok(())
}

fn files_equal(mut before: File, mut after: File, size: u64) -> Result<bool> {
    let mut left = [0; 16 * 1024];
    let mut right = [0; 16 * 1024];
    let mut remaining = size;
    while remaining > 0 {
        let count = remaining.min(left.len() as u64) as usize;
        before.read_exact(&mut left[..count])?;
        after.read_exact(&mut right[..count])?;
        if left[..count] != right[..count] {
            return Ok(false);
        }
        remaining -= count as u64;
    }
    Ok(true)
}

fn observation_status(value: &ObservationResult) -> &'static str {
    match value {
        ObservationResult::Completed { .. } => "completed",
        ObservationResult::NotRun { .. } => "not_run",
        ObservationResult::Unsupported { .. } => "unsupported",
    }
}

fn result_file(result: &ObservationResult, root: &Path, budget: &mut u64) -> Result<Option<File>> {
    match result {
        ObservationResult::Completed { output } => Ok(Some(blob_file(output, root, budget)?)),
        ObservationResult::NotRun { reason } | ObservationResult::Unsupported { reason } => {
            text(reason, "non-execution reason")?;
            Ok(None)
        }
    }
}

fn index<'a>(
    report: &'a Observations,
    role: Role,
    corpus: &Corpus,
    corpus_hash: &str,
) -> Result<BTreeMap<&'a str, &'a Observation>> {
    ensure!(report.schema_version == 1, "unsupported observation schema");
    ensure!(
        report.role == role,
        "observation role does not match argument position"
    );
    ensure!(
        report.lock_sha256 == corpus.lock_sha256 && report.corpus_sha256 == corpus_hash,
        "observation lock/corpus identity mismatch"
    );
    ensure!(
        report.context == corpus.context,
        "observation execution profile mismatch"
    );
    report.provenance.validate()?;
    ensure!(
        report.cases.len() == corpus.cases.len(),
        "observation case inventory mismatch"
    );
    let mut entries = BTreeMap::new();
    for case in &report.cases {
        text(&case.id, "case ID")?;
        digest(&case.input_sha256)?;
        digest(&case.selector_sha256)?;
        ensure!(
            entries.insert(case.id.as_str(), case).is_none(),
            "duplicate observation case"
        );
    }
    Ok(entries)
}

/// Validate locked evidence and compare actual supplied output bytes under one root.
///
/// This command performs no runner execution and authenticates no declarations.
/// Keep all files in a stable operator-controlled snapshot during comparison.
pub fn compare_observations(
    lock_path: &Path,
    artifact_root: &Path,
    corpus_path: &Path,
    apk_path: &Path,
    rust_path: &Path,
) -> Result<ComparisonReport> {
    let verified = crate::verify(lock_path, artifact_root)?;
    let (lock, lock_hash) = read_lock(lock_path)?;
    ensure!(
        lock_hash == verified.lock_sha256,
        "lock changed during comparison"
    );
    let root = artifact_root
        .canonicalize()
        .context("resolving observation root")?;
    let (corpus, corpus_hash): (Corpus, _) = document(corpus_path)?;
    ensure!(corpus.schema_version == 1, "unsupported corpus schema");
    ensure!(
        corpus.synthetic_inputs,
        "only synthetic observation corpora are supported"
    );
    ensure!(
        corpus.lock_sha256 == lock_hash,
        "corpus APK lock identity mismatch"
    );
    corpus.context.validate()?;
    ensure!(
        corpus.context.reference_abi == lock.abi,
        "corpus target ABI differs from APK lock"
    );
    ensure!(
        !corpus.cases.is_empty() && corpus.cases.len() <= CASE_LIMIT,
        "corpus must contain 1 to 1024 cases"
    );
    let mut budget = 0;
    for (blob, expected) in [
        (&corpus.profiles.clock, &corpus.context.clock_profile_sha256),
        (
            &corpus.profiles.randomness,
            &corpus.context.randomness_profile_sha256,
        ),
        (
            &corpus.profiles.providers,
            &corpus.context.providers_profile_sha256,
        ),
        (
            &corpus.profiles.observation_schema,
            &corpus.context.observation_schema_sha256,
        ),
    ] {
        ensure!(
            (1..=PROFILE_LIMIT).contains(&blob.size_bytes),
            "observation profile is empty or exceeds budget"
        );
        ensure!(
            &blob.sha256 == expected,
            "profile blob/context hash mismatch"
        );
        drop(blob_file(blob, &root, &mut budget)?);
    }
    let (apk, apk_hash): (Observations, _) = document(apk_path)?;
    let (rust, rust_hash): (Observations, _) = document(rust_path)?;
    let apk_cases = index(&apk, Role::ApkReference, &corpus, &corpus_hash)?;
    let rust_cases = index(&rust, Role::RustCandidate, &corpus, &corpus_hash)?;
    ensure!(
        apk.provenance.execution_target == corpus.context.reference_abi,
        "APK observation execution target differs from locked reference ABI"
    );
    let base = lock
        .artifacts
        .iter()
        .find(|a| a.kind == ArtifactKind::BaseApk)
        .context("missing base APK identity")?;
    ensure!(
        apk.provenance.implementation.artifact_sha256 == base.sha256,
        "APK observation implementation differs from locked base APK"
    );
    let selectors: BTreeMap<_, _> = verified
        .verified_selectors
        .iter()
        .map(|s| (s.id.as_str(), s.sha256.as_str()))
        .collect();
    let mut ids = BTreeSet::new();
    let mut comparisons = Vec::with_capacity(corpus.cases.len());
    for case in &corpus.cases {
        text(&case.id, "case ID")?;
        text(&case.selector_id, "selector ID")?;
        ensure!(ids.insert(&case.id), "duplicate corpus case");
        let selector_hash = selectors
            .get(case.selector_id.as_str())
            .context("corpus selector is not verified")?;
        drop(blob_file(&case.input, &root, &mut budget)?);
        let reference = apk_cases
            .get(case.id.as_str())
            .context("missing APK observation case")?;
        let candidate = rust_cases
            .get(case.id.as_str())
            .context("missing Rust observation case")?;
        for observed in [reference, candidate] {
            ensure!(
                observed.input_sha256 == case.input.sha256
                    && observed.selector_sha256 == *selector_hash,
                "observation input/selector identity mismatch"
            );
        }
        let left = result_file(&reference.result, &root, &mut budget)?;
        let right = result_file(&candidate.result, &root, &mut budget)?;
        let status = match (&reference.result, &candidate.result, left, right) {
            (
                ObservationResult::Completed { output: a },
                ObservationResult::Completed { output: b },
                Some(left),
                Some(right),
            ) => {
                if a.size_bytes == b.size_bytes {
                    charge(
                        &mut budget,
                        a.size_bytes
                            .checked_mul(2)
                            .context("comparison budget overflow")?,
                    )?;
                }
                if a.size_bytes == b.size_bytes && files_equal(left, right, a.size_bytes)? {
                    ComparisonStatus::Pass
                } else {
                    ComparisonStatus::Mismatch
                }
            }
            (ObservationResult::Unsupported { .. }, _, _, _)
            | (_, ObservationResult::Unsupported { .. }, _, _) => ComparisonStatus::Unsupported,
            _ => ComparisonStatus::NotRun,
        };
        comparisons.push(CaseComparison {
            id: case.id.clone(),
            selector_id: case.selector_id.clone(),
            input_sha256: case.input.sha256.clone(),
            status,
            apk_status: observation_status(&reference.result),
            rust_status: observation_status(&candidate.result),
        });
    }
    comparisons.sort_by(|a, b| a.id.cmp(&b.id));
    let status = [
        ComparisonStatus::Mismatch,
        ComparisonStatus::Unsupported,
        ComparisonStatus::NotRun,
    ]
    .into_iter()
    .find(|status| comparisons.iter().any(|c| c.status == *status))
    .unwrap_or(ComparisonStatus::Pass);
    Ok(ComparisonReport {
        schema_version: 1,
        scope: "supplied_observation_agreement",
        status,
        qualified_execution: false,
        execution_evidence: "operator_supplied_not_authenticated",
        profile_integrity: "verified_bytes_not_application",
        authenticity: AuthenticityStatus::RequiresIndependentReview,
        protocol_execution: ExecutionStatus::NotRun,
        lock_sha256: lock_hash,
        corpus_sha256: corpus_hash,
        apk_observations_sha256: apk_hash,
        rust_observations_sha256: rust_hash,
        context: corpus.context,
        apk_provenance: apk.provenance,
        rust_provenance: rust.provenance,
        cases: comparisons,
    })
}
