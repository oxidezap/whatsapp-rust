//! Oracle coordination and lossless fixtures. The guest implementation stays in unwasm.
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use xtask_support::{capture, cbor, read_json, run as execute, sha256, write, write_json};
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
    /// Re-derive and verify every primary fixture with the pinned Rust oracle task.
    Regenerate {
        #[arg(long)]
        oracle_repo: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long)]
        from_derived: Option<PathBuf>,
        #[arg(long)]
        check: bool,
        #[arg(long)]
        allow_tool_worktree: bool,
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
            ensure!(
                cbor::decompress(&std::fs::read(data.join(&name))?)? == cbor::encode(&selected),
                "C audit drift: {name}"
            );
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
        if check {
            ensure!(
                cbor::decompress(&std::fs::read(data.join(&target))?)? == raw,
                "C raw drift: {name}"
            );
        } else {
            write(&data.join(&target), &packed)?;
        }
        manifest[target] = json!({"oracle":"C audit","source_revision":revision,"source_file":name,"source_sha256":sha256(&original),"raw_sha256":sha256(&raw),"raw_bytes":raw.len(),"packed_bytes":packed.len()});
    }
    if !check {
        write_json(&data.join("packed-fixtures.json"), &manifest)?;
    }
    println!("C audit corpus verified");
    Ok(())
}
fn regenerate(
    root: &Path,
    tool: &Path,
    out: &Path,
    cached: bool,
    check: bool,
    allow_dirty: bool,
) -> Result<()> {
    let tool = tool.canonicalize()?;
    let pin = read_json(&root.join("scripts/mlow-vectors/oracle.lock.json"))?;
    let revision = String::from_utf8(
        capture(
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .current_dir(&tool),
        )?
        .stdout,
    )?
    .trim()
    .to_owned();
    if !allow_dirty {
        ensure!(
            pin["revision"].as_str() == Some(&revision),
            "oracle revision differs: {revision}"
        );
        execute(
            Command::new("git")
                .args([
                    "diff",
                    "--quiet",
                    "HEAD",
                    "--",
                    "crates",
                    "tools",
                    "specs",
                    "Cargo.lock",
                    "Cargo.toml",
                    ".cargo/config.toml",
                    "wasm.lock.json",
                ])
                .current_dir(&tool),
        )?;
    }
    ensure!(
        sha256(&std::fs::read(tool.join("specs/mlow.lock.json"))?)
            == pin["derivation_lock_sha256"].as_str().context("lock pin")?,
        "derivation lock drift"
    );
    ensure!(
        std::fs::read(tool.join("specs/synth_mic.raw"))?
            == std::fs::read(root.join(DATA).join("synth_mic.raw"))?,
        "synthetic input mismatch"
    );
    let mut cmd = Command::new("cargo");
    cmd.args(["+stable", "xt", "mlow", "verify", "--out"])
        .arg(out)
        .current_dir(&tool)
        .env("CARGO_ENCODED_RUSTFLAGS", "");
    if cached {
        cmd.arg("--from-derived");
    }
    execute(&mut cmd)?;
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
    if !check {
        write_json(
            &data.join("wasm-fixtures.json"),
            &json!({"derivation_lock_sha256":pin["derivation_lock_sha256"],"files":metadata}),
        )?;
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
        Task::Regenerate {
            oracle_repo,
            out,
            from_derived,
            check,
            allow_tool_worktree,
        } => {
            let cached = from_derived.is_some();
            let out = std::path::absolute(
                from_derived
                    .or(out)
                    .unwrap_or(root.join(".derive-mlow/wasm")),
            )?;
            regenerate(root, &oracle_repo, &out, cached, check, allow_tool_worktree)
        }
        Task::CReference { check } => c_reference(root, check),
    }
}
