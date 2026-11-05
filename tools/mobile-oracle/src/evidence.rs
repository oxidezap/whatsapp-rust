//! Evidence bound to locked artifact bytes, without executing extractor output.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    ApkLock, ArtifactKind, SET_LIMIT, digest_valid, hash_reader, nonempty, open_artifact,
    validate_relative,
};

const EVIDENCE_COUNT_LIMIT: usize = 4096;

/// Operator-declared extraction provenance, not proof that a tool was executed.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionEvidence {
    pub output_path: String,
    pub input_apk_path: String,
    pub input_apk_sha256: String,
    /// Path inside the source APK archive, not a host filesystem destination.
    pub archive_member: String,
    pub tool: ExtractionTool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExtractionTool {
    pub name: String,
    pub version: String,
    /// SHA-256 of the executable/JAR/distribution used by the operator.
    pub artifact_sha256: String,
    pub arguments: Vec<String>,
}

/// Exact byte-range fingerprint. A byte match alone establishes no semantics.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorEvidence {
    pub id: String,
    pub artifact_path: String,
    /// File offset, not a runtime virtual address or a function index.
    pub offset_bytes: u64,
    pub size_bytes: u64,
    pub sha256: String,
    /// Call-path/guard analysis establishing why this range was selected.
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VerifiedSelector {
    pub id: String,
    pub artifact_path: String,
    pub offset_bytes: u64,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SelectorChange {
    pub id: String,
    pub before: Option<SelectorEvidence>,
    pub after: Option<SelectorEvidence>,
}

fn derived(kind: ArtifactKind) -> bool {
    matches!(
        kind,
        ArtifactKind::Dex | ArtifactKind::NativeLibrary | ArtifactKind::Resources
    )
}

pub(crate) fn validate(lock: &ApkLock) -> Result<()> {
    ensure!(
        lock.extractions.len() <= EVIDENCE_COUNT_LIMIT
            && lock.selectors.len() <= EVIDENCE_COUNT_LIMIT,
        "too many extraction/selector records"
    );
    let artifacts: BTreeMap<_, _> = lock.artifacts.iter().map(|a| (&a.path, a)).collect();
    let mut outputs = BTreeSet::new();
    for extraction in &lock.extractions {
        validate_relative(&extraction.output_path)?;
        validate_relative(&extraction.input_apk_path)?;
        validate_relative(&extraction.archive_member)?;
        ensure!(
            outputs.insert(&extraction.output_path),
            "duplicate extraction output"
        );
        let output = artifacts
            .get(&extraction.output_path)
            .context("extraction output is not locked")?;
        ensure!(
            derived(output.kind),
            "extraction output must be DEX, native library or resources"
        );
        let input = artifacts
            .get(&extraction.input_apk_path)
            .context("extraction input APK is not locked")?;
        ensure!(
            matches!(input.kind, ArtifactKind::BaseApk | ArtifactKind::SplitApk),
            "extraction input must be an APK"
        );
        ensure!(
            input.sha256 == extraction.input_apk_sha256,
            "extraction input hash differs from locked APK"
        );
        nonempty(&extraction.tool.name, "extraction tool name")?;
        nonempty(&extraction.tool.version, "extraction tool version")?;
        ensure!(
            digest_valid(&extraction.tool.artifact_sha256),
            "invalid extraction tool SHA-256"
        );
        ensure!(
            extraction.tool.arguments.len() <= 256,
            "too many extraction arguments"
        );
        for argument in &extraction.tool.arguments {
            nonempty(argument, "extraction argument")?;
        }
    }
    // Older/base-only locks remain valid, but any derived bytes added now must
    // carry explicit lineage rather than silently borrowing APK authority.
    for artifact in &lock.artifacts {
        ensure!(
            !derived(artifact.kind) || outputs.contains(&artifact.path),
            "derived artifact lacks extraction provenance: {}",
            artifact.path
        );
    }
    let mut ids = BTreeSet::new();
    let mut total = 0_u64;
    for selector in &lock.selectors {
        nonempty(&selector.id, "selector id")?;
        ensure!(ids.insert(&selector.id), "duplicate selector id");
        validate_relative(&selector.artifact_path)?;
        let artifact = artifacts
            .get(&selector.artifact_path)
            .context("selector artifact is not locked")?;
        ensure!(
            derived(artifact.kind),
            "selector must refer to derived artifact bytes"
        );
        ensure!(selector.size_bytes > 0, "selector range must not be empty");
        let end = selector
            .offset_bytes
            .checked_add(selector.size_bytes)
            .context("selector range overflow")?;
        ensure!(
            end <= artifact.size_bytes,
            "selector range exceeds artifact"
        );
        ensure!(digest_valid(&selector.sha256), "invalid selector SHA-256");
        nonempty(&selector.rationale, "selector rationale")?;
        total = total
            .checked_add(selector.size_bytes)
            .context("selector size overflow")?;
        ensure!(
            total <= SET_LIMIT,
            "selector verification exceeds read budget"
        );
    }
    Ok(())
}

pub(crate) fn verify_selectors(lock: &ApkLock, root: &Path) -> Result<Vec<VerifiedSelector>> {
    let artifacts: BTreeMap<_, _> = lock.artifacts.iter().map(|a| (&a.path, a)).collect();
    let mut result = Vec::with_capacity(lock.selectors.len());
    for selector in &lock.selectors {
        let artifact = artifacts
            .get(&selector.artifact_path)
            .context("selector artifact is not locked")?;
        let mut file = open_artifact(root, artifact)?;
        file.seek(SeekFrom::Start(selector.offset_bytes))
            .context("seeking selector range")?;
        let (size_bytes, sha256) = hash_reader(&mut file.take(selector.size_bytes))?;
        ensure!(
            size_bytes == selector.size_bytes && sha256 == selector.sha256,
            "selector fingerprint mismatch: {}",
            selector.id
        );
        result.push(VerifiedSelector {
            id: selector.id.clone(),
            artifact_path: selector.artifact_path.clone(),
            offset_bytes: selector.offset_bytes,
            size_bytes,
            sha256,
        });
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(result)
}

pub(crate) fn selector_changes(before: &ApkLock, after: &ApkLock) -> Vec<SelectorChange> {
    let old: BTreeMap<_, _> = before.selectors.iter().map(|s| (&s.id, s)).collect();
    let new: BTreeMap<_, _> = after.selectors.iter().map(|s| (&s.id, s)).collect();
    let ids: BTreeSet<_> = old.keys().chain(new.keys()).copied().collect();
    ids.into_iter()
        .filter_map(|id| {
            let previous = old.get(id).copied();
            let next = new.get(id).copied();
            (previous != next).then(|| SelectorChange {
                id: id.clone(),
                before: previous.cloned(),
                after: next.cloned(),
            })
        })
        .collect()
}

pub(crate) fn normalize(lock: &mut ApkLock) {
    lock.extractions
        .sort_by(|a, b| a.output_path.cmp(&b.output_path));
    lock.selectors.sort_by(|a, b| a.id.cmp(&b.id));
}
