//! MLOW oracle derivation, lossless fixtures and independent C comparison.
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, cbor, run as execute, sha256, write, write_json};
const DATA: &str = "wacore/src/voip/mlow/testdata";
const SOURCE: &str = "0aa87c64ffd07fb288a7db8df5c46c30e92ff7fa";
#[derive(Subcommand)]
pub enum Task {
    /// Pack JSON as lossless CBOR+zstd and print its content hashes.
    Pack { source: PathBuf, output: PathBuf },
    /// Reconstruct compact, independent C auditors from immutable historical blobs.
    PackLegacy {
        #[arg(long)]
        check: bool,
    },
    /// Fetch the J/S wasm captures required by the MLOW derivation.
    Fetch,
    /// Execute and verify all locked MLOW derivations.
    Verify {
        #[arg(long, default_value = "all", value_parser = ["all", "JgwtTQVeWPm", "S_ivh1PriOA"])]
        capture: String,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        from_derived: bool,
        #[arg(long, conflicts_with = "refresh_spec_hashes")]
        update_lock: bool,
        #[arg(long)]
        refresh_spec_hashes: bool,
    },
    /// Generate every derived spec or check it against its recipe.
    Specs {
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        check: bool,
    },
    /// Generate one leaf trace specification.
    Spec {
        kind: String,
        #[arg(long)]
        out: PathBuf,
        #[arg(long, default_value_t = 330)]
        lsf_count: usize,
        #[arg(long, default_value_t = 1099)]
        end: usize,
    },
    /// Assemble captured snapshots into fixture JSON.
    Assemble {
        kind: Option<String>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        run: Option<PathBuf>,
        #[arg(long, alias = "lsf-output", alias = "harm-output")]
        secondary: Option<PathBuf>,
        #[arg(long, default_value_t = 330)]
        lsf_count: usize,
    },
    /// Re-derive and verify every primary fixture with the pinned Rust oracle task.
    Regenerate {
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        from_derived: Option<PathBuf>,
        #[arg(long)]
        check: bool,
    },
    /// Build the independent C reference harness and regenerate its packet/PCM pairs.
    CReference {
        #[arg(long)]
        check: bool,
    },
}
fn stored(root: &Path, name: &str) -> Result<Vec<u8>> {
    Ok(capture(
        Command::new("git")
            .args(["show", &format!("{SOURCE}:{DATA}/{name}")])
            .current_dir(root),
    )?
    .stdout)
}
fn select(root: &Path, name: &str, records: Value) -> Result<Value> {
    if name == "gennoise_vectors" {
        let records = records.as_array().context("noise records")?;
        let mut indices = (0..records.len())
            .step_by((records.len() / 32).max(1))
            .collect::<BTreeSet<_>>();
        let mut groups = BTreeMap::<(i64, bool, i64), Vec<usize>>::new();
        for (i, r) in records.iter().enumerate() {
            groups
                .entry((
                    r["voiced"].as_i64().context("voiced")?,
                    r["sf_pulses"].as_i64().context("pulses")? > 0,
                    r["ng_in"]["prev_voiced"]
                        .as_i64()
                        .context("previous voice")?,
                ))
                .or_default()
                .push(i);
        }
        for group in groups.values() {
            indices.insert(group[0]);
            indices.insert(*group.last().context("empty group")?);
        }
        return Ok(json!(
            indices
                .into_iter()
                .map(|i| records[i].clone())
                .collect::<Vec<_>>()
        ));
    }
    if matches!(name, "exc_pre_lags" | "gennoise_params_dump") {
        let frames: Vec<String> =
            serde_json::from_slice(&stored(root, "inbound_capture_frames.json")?)?;
        let frames = frames
            .iter()
            .map(hex::decode)
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut packets = (0..frames.len()).step_by(11).collect::<BTreeSet<_>>();
        let records = records.as_array().context("audit records")?;
        for voiced in [0, 1] {
            let mut eligible = Vec::new();
            for r in records {
                let packet = usize::try_from(r["packet"].as_u64().context("packet")?)?;
                if r["voiced"] == voiced
                    && frames
                        .get(packet)
                        .and_then(|f| f.first())
                        .is_some_and(|b| b & 0x42 != 0)
                {
                    eligible.push(packet);
                }
            }
            packets.insert(*eligible.first().context("audit lost a voice branch")?);
            packets.insert(*eligible.last().context("audit lost a voice branch")?);
        }
        return Ok(json!(
            records
                .iter()
                .filter(|r| r["packet"]
                    .as_u64()
                    .is_some_and(|p| packets.contains(&(p as usize))))
                .cloned()
                .collect::<Vec<_>>()
        ));
    }
    Ok(records)
}

fn stable_manifest(mut manifest: Value) -> Result<Value> {
    for (name, metadata) in manifest
        .as_object_mut()
        .context("C audit manifest is not an object")?
    {
        if name.ends_with(".cbor.zst") {
            let metadata = metadata
                .as_object_mut()
                .with_context(|| format!("C audit manifest entry {name} is not an object"))?;
            metadata.remove("json_sha256");
            metadata.remove("json_bytes");
        }
    }
    Ok(manifest)
}

fn pack_legacy(root: &Path, check: bool) -> Result<()> {
    let data = root.join(DATA);
    let mut manifest = json!({});
    let revision = String::from_utf8(
        capture(
            Command::new("git")
                .args(["rev-parse", SOURCE])
                .current_dir(root),
        )?
        .stdout,
    )?
    .trim()
    .to_owned();
    for name in [
        "e2e_vectors",
        "exc_pre_lags",
        "fe_dump",
        "gennoise_params_dump",
        "gennoise_vectors",
        "lsf_quant_io",
        "pitchio_ground_truth",
        "pulse_vectors",
        "rc_vectors",
        "sigmode_ground_truth",
    ] {
        let source = format!("{name}.json");
        let original = stored(root, &source)?;
        let selected = select(root, name, serde_json::from_slice(&original)?)?;
        let mut json = serde_json::to_vec(&selected)?;
        json.push(b'\n');
        let (packed, mut meta) = cbor::pack(&json)?;
        let name = format!("{name}.cbor.zst");
        if check {
            let committed = std::fs::read(data.join(&name))?;
            ensure!(
                cbor::decompress(&committed)? == cbor::encode(&selected),
                "C audit drift: {name}"
            );
            meta["zstd_sha256"] = json!(sha256(&committed));
            meta["packed_bytes"] = json!(committed.len());
        } else {
            write(&data.join(&name), &packed)?;
        }
        meta["oracle"] = json!("C audit");
        meta["source_revision"] = json!(revision);
        meta["source_file"] = json!(source);
        meta["source_sha256"] = json!(sha256(&original));
        manifest[name] = meta;
    }
    for name in ["harm_postfilter_vectors.raw", "hp_postfilter_vectors.raw"] {
        let original = stored(root, name)?;
        let mut raw = original.clone();
        if name == "hp_postfilter_vectors.raw" {
            let count =
                u32::from_le_bytes(original.get(..4).context("raw header")?.try_into()?) as usize;
            let width = 3932;
            ensure!(
                original.len() == 4 + count * width && count > 0,
                "raw audit size"
            );
            let mut indices = (0..count)
                .step_by((count / 32).max(1))
                .collect::<BTreeSet<_>>();
            indices.insert(count - 1);
            raw = (indices.len() as u32).to_le_bytes().to_vec();
            for i in indices {
                raw.extend(&original[4 + i * width..4 + (i + 1) * width]);
            }
        }
        let packed = cbor::compress(&raw)?;
        let target = format!("{name}.zst");
        let packed_bytes = if check {
            let committed = std::fs::read(data.join(&target))?;
            ensure!(cbor::decompress(&committed)? == raw, "C raw drift: {name}");
            committed.len()
        } else {
            write(&data.join(&target), &packed)?;
            packed.len()
        };
        manifest[target] = json!({"oracle":"C audit","source_revision":revision,"source_file":name,"source_sha256":sha256(&original),"raw_sha256":sha256(&raw),"raw_bytes":raw.len(),"packed_bytes":packed_bytes});
    }
    if check {
        let committed: Value =
            serde_json::from_slice(&std::fs::read(data.join("packed-fixtures.json"))?)?;
        ensure!(
            stable_manifest(committed)? == stable_manifest(manifest)?,
            "C audit manifest drift"
        );
    } else {
        write_json(&data.join("packed-fixtures.json"), &manifest)?;
    }
    println!("C audit corpus verified");
    Ok(())
}
fn regenerate(root: &Path, out: &Path, cached: bool, check: bool) -> Result<()> {
    let specs = root.join("tools/oracle-core/specs");
    let lock_sha256 = sha256(&std::fs::read(specs.join("mlow.lock.json"))?);
    ensure!(
        std::fs::read(specs.join("synth_mic.raw"))?
            == std::fs::read(root.join(DATA).join("synth_mic.raw"))?,
        "synthetic input mismatch"
    );
    super::derive_mlow::verify(root, out, "all", cached, false, false)?;
    let mut metadata = json!({});
    let data = root.join(DATA);
    for leaf in [
        "fe",
        "signal_mode",
        "pitch",
        "lsf_quant",
        "hp_postfilter",
        "harm_postfilter",
        "params",
        "gennoise",
    ] {
        let source = std::fs::read(out.join(format!("artifacts/wasm_{leaf}.json")))?;
        let (packed, record) = cbor::pack(&source)?;
        let name = format!("wasm_{leaf}.cbor.zst");
        if check {
            ensure!(
                sha256(&cbor::decompress(&std::fs::read(data.join(&name))?)?)
                    == record["cbor_sha256"].as_str().context("CBOR hash")?,
                "wasm fixture drift: {name}"
            );
        } else {
            write(&data.join(&name), &packed)?;
        }
        metadata[name] = record;
    }
    for name in [
        "wasm_derived_frames.json",
        "wasm_derived_ref.raw",
        "wasm_derived_vad.json",
        "wasm_derived_120ms_frames.json",
        "wasm_derived_120ms_ref.raw",
    ] {
        let payload = std::fs::read(out.join("artifacts").join(name))?;
        if check {
            ensure!(
                std::fs::read(data.join(name))? == payload,
                "stream fixture drift: {name}"
            );
        } else {
            write(&data.join(name), &payload)?;
        }
        metadata[name] = json!({"sha256":sha256(&payload),"bytes":payload.len()});
    }
    let manifest = json!({"derivation_lock_sha256":lock_sha256,"files":metadata});
    if check {
        ensure!(
            xtask_support::read_json(&data.join("wasm-fixtures.json"))? == manifest,
            "wasm fixture manifest drift"
        );
    } else {
        write_json(&data.join("wasm-fixtures.json"), &manifest)?;
    }
    println!("Wasm fixtures verified");
    Ok(())
}
fn source_newer(directory: &Path, modified: std::time::SystemTime) -> Result<bool> {
    ensure!(
        directory.is_dir(),
        "reference source directory missing: {}",
        directory.display()
    );
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_dir() && source_newer(&entry.path(), modified)? {
            return Ok(true);
        }
        if kind.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|e| e == "c" || e == "h")
            && entry.metadata()?.modified()? > modified
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn c_reference(root: &Path, check: bool) -> Result<()> {
    let reference = PathBuf::from(
        std::env::var_os("MLOW_REFERENCE")
            .context("set MLOW_REFERENCE to a built C oracle checkout")?,
    )
    .canonicalize()?;
    let lib = reference.join(".libs/libopus.a");
    ensure!(lib.is_file(), "build the C reference first");
    if let Ok(output) = capture(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&reference),
    ) {
        let revision = String::from_utf8(output.stdout)?;
        let dirty = capture(
            Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&reference),
        )?
        .stdout;
        ensure!(
            dirty.is_empty() || std::env::var("MLOW_ALLOW_DIRTY_REFERENCE").as_deref() == Ok("1"),
            "reference is dirty; set MLOW_ALLOW_DIRTY_REFERENCE=1 only for an intentional modified oracle"
        );
        if revision.trim() != "84b076e0809412df22e8a0d26f944610c4a3e40f" {
            eprintln!(
                "reference revision differs from the archived oracle: {}",
                revision.trim()
            );
        }
    } else {
        eprintln!("reference revision could not be identified");
    }
    let modified = std::fs::metadata(&lib)?.modified()?;
    for name in ["smpl", "src", "celt"] {
        ensure!(
            !source_newer(&reference.join(name), modified)?,
            "reference library is older than its sources; rebuild it"
        );
    }
    let work = tempfile::tempdir()?;
    let binary = work.path().join("mlow_frames");
    let mut cc = Command::new("cc");
    cc.arg("-O2");
    for path in [
        reference.join("include"),
        reference.join("src"),
        reference.join("celt"),
        reference.clone(),
    ] {
        cc.arg("-I").arg(path);
    }
    execute(
        cc.arg("-o")
            .arg(&binary)
            .arg(root.join("scripts/mlow-vectors/mlow_frames.c"))
            .arg(lib)
            .arg("-lm"),
    )?;
    for (ms, count, frames_name, pcm_name) in [
        (120, 8, "mlow_120ms_frames.json", "ref_120ms_expected.raw"),
        (
            60,
            110,
            "mlow_dtx_off_frames.json",
            "ref_dtx_off_expected.raw",
        ),
    ] {
        let output = capture(
            Command::new(&binary)
                .arg(root.join(DATA).join("synth_mic.raw"))
                .arg(ms.to_string())
                .arg(count.to_string()),
        )?;
        let text = String::from_utf8(output.stdout)?;
        let mut frames = Vec::new();
        let mut pcm = Vec::new();
        for line in text.lines() {
            let parts = line.split_whitespace().collect::<Vec<_>>();
            ensure!(parts.len() == 2, "malformed C harness output");
            hex::decode(parts[0])?;
            frames.push(parts[0]);
            pcm.extend(hex::decode(parts[1])?);
        }
        ensure!(frames.len() == count, "C harness packet count");
        let mut bytes = Vec::new();
        let mut serializer = serde_json::Serializer::with_formatter(
            &mut bytes,
            serde_json::ser::PrettyFormatter::with_indent(b" "),
        );
        serde::Serialize::serialize(&frames, &mut serializer)?;
        for (name, payload) in [(frames_name, bytes), (pcm_name, pcm)] {
            let path = root.join(DATA).join(name);
            if check {
                ensure!(
                    std::fs::read(path)? == payload,
                    "C reference fixture drift: {name}"
                );
            } else {
                write(&path, &payload)?;
            }
        }
    }
    Ok(())
}
pub fn run(root: &Path, task: Task) -> Result<()> {
    match task {
        Task::Pack { source, output } => {
            let (packed, meta) = cbor::pack(&std::fs::read(source)?)?;
            write(&output, &packed)?;
            println!("{}", serde_json::to_string_pretty(&meta)?);
            Ok(())
        }
        Task::PackLegacy { check } => pack_legacy(root, check),
        Task::Fetch => super::derive_mlow::fetch(root, Some(&["JgwtTQVeWPm", "S_ivh1PriOA"])),
        Task::Verify {
            capture,
            out,
            from_derived,
            update_lock,
            refresh_spec_hashes,
        } => super::derive_mlow::verify(
            root,
            &std::path::absolute(out.unwrap_or(root.join(".derive-mlow/wasm")))?,
            &capture,
            from_derived,
            update_lock,
            refresh_spec_hashes,
        ),
        Task::Specs { out, check } => super::derive_mlow::specs(
            root,
            &out.unwrap_or(root.join("tools/oracle-core/specs")),
            check,
        ),
        Task::Spec {
            kind,
            out,
            lsf_count,
            end,
        } => super::derive_mlow::spec(root, &kind, &out, lsf_count, end),
        Task::Assemble {
            kind,
            out,
            run,
            secondary,
            lsf_count,
        } => super::derive_mlow::assemble(
            kind.as_deref(),
            run.as_deref(),
            &out,
            secondary.as_deref(),
            lsf_count,
        ),
        Task::Regenerate {
            out,
            from_derived,
            check,
        } => {
            let cached = from_derived.is_some();
            let out = std::path::absolute(
                from_derived
                    .or(out)
                    .unwrap_or(root.join(".derive-mlow/wasm")),
            )?;
            regenerate(root, &out, cached, check)
        }
        Task::CReference { check } => c_reference(root, check),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_comparison_ignores_only_intermediate_json_rendering() {
        let left = json!({"fixture.cbor.zst":{"json_sha256":"a","json_bytes":1,"cbor_sha256":"stable","records":2}});
        let right = json!({"fixture.cbor.zst":{"json_sha256":"b","json_bytes":9,"cbor_sha256":"stable","records":2}});
        assert_eq!(
            stable_manifest(left).unwrap(),
            stable_manifest(right).unwrap()
        );

        let changed = json!({"fixture.cbor.zst":{"cbor_sha256":"changed","records":2}});
        assert_ne!(
            stable_manifest(json!({"fixture.cbor.zst":{"cbor_sha256":"stable","records":2}}))
                .unwrap(),
            stable_manifest(changed).unwrap()
        );
    }
}
