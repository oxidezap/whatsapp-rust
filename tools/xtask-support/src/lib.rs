//! Shared host-side task utilities. Nothing in the codec/decompiler runtime depends on this crate.

#[cfg(feature = "compression")]
pub mod cbor;

use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::{Command, Output};

/// SHA-256 in lowercase hexadecimal.
pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Read JSON with context and correctly rounded float parsing.
pub fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(
        &std::fs::read(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))
}

/// Atomically replace a generated file after its content has been validated.
pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.persist(path)
        .with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// Write deterministic, sorted, pretty JSON with a final newline.
pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write(path, &bytes)
}

/// Run a command, forwarding its output and refusing nonzero exit status.
pub fn run(command: &mut Command) -> Result<()> {
    let program = command.get_program().to_string_lossy().into_owned();
    let status = command
        .status()
        .with_context(|| format!("running {program}"))?;
    ensure!(status.success(), "{program} failed with {status}");
    Ok(())
}

/// Capture a command's output. Arguments are never rendered (they may carry credentials).
pub fn capture(command: &mut Command) -> Result<Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    let output = command
        .output()
        .with_context(|| format!("running {program}"))?;
    ensure!(
        output.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}

/// Decode named little-endian scalar arrays while preserving integer/float JSON types.
pub fn unpack(bytes: &[u8], format: &str) -> Result<Vec<Value>> {
    let mut values = Vec::new();
    let mut offset = 0usize;
    let mut count: Option<usize> = None;
    for kind in format.chars() {
        if let Some(digit) = kind.to_digit(10) {
            count = Some(
                count
                    .unwrap_or(0)
                    .checked_mul(10)
                    .and_then(|n| n.checked_add(digit as usize))
                    .context("format count overflow")?,
            );
            continue;
        }
        let width = match kind {
            'b' => 1,
            'h' => 2,
            'i' | 'I' | 'f' => 4,
            'd' => 8,
            _ => anyhow::bail!("unknown scalar format {kind}"),
        };
        for _ in 0..count.unwrap_or(1) {
            let end = offset.checked_add(width).context("scalar span overflow")?;
            let part = bytes.get(offset..end).context("truncated scalar array")?;
            let value = match kind {
                'b' => Value::from(part[0] as i8),
                'h' => Value::from(i16::from_le_bytes(part.try_into()?)),
                'i' => Value::from(i32::from_le_bytes(part.try_into()?)),
                'I' => Value::from(u32::from_le_bytes(part.try_into()?)),
                'f' => {
                    let f = f32::from_le_bytes(part.try_into()?) as f64;
                    ensure!(f.is_finite(), "non-finite oracle scalar");
                    Value::from(f)
                }
                'd' => {
                    let f = f64::from_le_bytes(part.try_into()?);
                    ensure!(f.is_finite(), "non-finite oracle scalar");
                    Value::from(f)
                }
                _ => anyhow::bail!("unsupported scalar format"),
            };
            values.push(value);
            offset = end;
        }
        count = None;
    }
    ensure!(
        count.is_none() && offset == bytes.len(),
        "scalar array length mismatch"
    );
    Ok(values)
}

/// Validate every manifest output and hash the same filename/length/digest tree in both repos.
pub fn output_tree(manifest: &Value, directory: &Path) -> Result<String> {
    let mut records = manifest["outputs"]
        .as_array()
        .context("manifest outputs missing")?
        .iter()
        .collect::<Vec<_>>();
    records.sort_by_key(|r| r["file"].as_str().unwrap_or_default());
    let mut hash = Sha256::new();
    let mut seen = std::collections::BTreeSet::new();
    for record in records {
        let name = record["file"].as_str().context("output filename missing")?;
        ensure!(seen.insert(name), "duplicate output {name}");
        ensure!(
            !Path::new(name).is_absolute()
                && Path::new(name)
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "invalid output path {name}"
        );
        let payload = std::fs::read(directory.join(name))?;
        ensure!(
            record["bytes"].as_u64() == Some(payload.len() as u64)
                && record["sha256"].as_str() == Some(sha256(&payload).as_str()),
            "corrupt output {name}"
        );
        hash.update(u32::try_from(name.len())?.to_le_bytes());
        hash.update(name.as_bytes());
        hash.update((payload.len() as u64).to_le_bytes());
        hash.update(Sha256::digest(&payload));
    }
    Ok(hex::encode(hash.finalize()))
}

/// Regenerate a protobuf descriptor and its source/descriptor digest sidecar.
/// The compiler stays external; all task logic and hashing are shared Rust.
pub fn descriptor(proto: &Path, descriptor: &Path, source_info: bool) -> Result<()> {
    let directory = proto.parent().context("proto directory")?;
    let temporary = tempfile::NamedTempFile::new_in(directory)?;
    let mut cmd = Command::new("protoc");
    cmd.arg(format!(
        "--descriptor_set_out={}",
        temporary.path().display()
    ))
    .arg("--include_imports");
    if source_info {
        cmd.arg("--include_source_info");
    }
    run(cmd.arg("-I").arg(directory).arg(proto))?;
    let data = std::fs::read(temporary.path())?;
    let sidecar = format!(
        "proto {}\ndesc {}\n",
        sha256(&std::fs::read(proto)?),
        sha256(&data)
    );
    write(descriptor, &data)?;
    write(
        &descriptor.with_extension("desc.sha256"),
        sidecar.as_bytes(),
    )
}

/// Preserve a process exit code, including conventional Unix signal exit statuses.
pub fn exit_code(status: std::process::ExitStatus) -> u8 {
    if let Some(code) = status.code() {
        return code as u8;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return (128 + signal) as u8;
        }
    }
    1
}

/// Hash a file or explicit hexadecimal bytes for reproducible fixture recipes.
pub fn hash_input(value: &str, hexadecimal: bool) -> Result<String> {
    let bytes = if hexadecimal {
        hex::decode(value).context("invalid hexadecimal input")?
    } else {
        std::fs::read(value).with_context(|| format!("reading {value}"))?
    };
    Ok(sha256(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_scalar_snapshots_are_rejected() {
        assert!(unpack(&[0, 0, 0], "f").is_err());
        assert!(unpack(&[0; 8], "i").is_err());
        assert!(unpack(&[], "4").is_err());
        assert!(unpack(&f32::NAN.to_le_bytes(), "f").is_err());
        assert_eq!(
            unpack(&(-7i32).to_le_bytes(), "i").unwrap(),
            vec![serde_json::json!(-7)]
        );
    }
    #[cfg(unix)]
    #[test]
    fn exit_status_preserves_failures_and_signals() {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(exit_code(std::process::ExitStatus::from_raw(7 << 8)), 7);
        assert_eq!(exit_code(std::process::ExitStatus::from_raw(9)), 137);
    }
}
