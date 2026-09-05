//! Diagnostic patches preserve refusal rules; they never change the canonical captures.
use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use std::path::{Path, PathBuf};
use wasm_encoder::{Encode, Instruction};
use xtask_support::write;

#[derive(Subcommand)]
pub enum Task {
    /// Fetch every capture pinned by the WhatsApp wasm oracle.
    Fetch,
    /// Export the first COUNT globals without rewriting unrelated sections.
    ExportGlobals {
        source: PathBuf,
        destination: PathBuf,
        count: u32,
    },
    /// Force the uniquely identified outgoing-offer guard.
    ForceOfferGuard {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Neutralize the profiler guard in the pinned D5 capture.
    NeutralizeThreadProfiler {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Tag the nine pinned D5 outgoing-offer failure sites.
    TagOfferErrorSites {
        source: PathBuf,
        destination: PathBuf,
    },
    /// Verify two persisted audio/video oracle traces byte for byte.
    CompareMedia { expected: PathBuf, actual: PathBuf },
}

pub fn run(root: &Path, task: Task) -> Result<()> {
    match task {
        Task::Fetch => super::derive_mlow::fetch(root, None),
        Task::ExportGlobals {
            source,
            destination,
            count,
        } => globals(&source, &destination, count),
        Task::ForceOfferGuard {
            source,
            destination,
        } => offer_guard(&source, &destination),
        Task::NeutralizeThreadProfiler {
            source,
            destination,
        } => profiler(&source, &destination),
        Task::TagOfferErrorSites {
            source,
            destination,
        } => offer_errors(&source, &destination),
        Task::CompareMedia { expected, actual } => {
            let expected = oracle_core::read_media_trace(&expected)?;
            let actual = oracle_core::read_media_trace(&actual)?;
            oracle_core::compare_media(&expected, &actual)?;
            println!("{} media record(s) match", expected.len());
            Ok(())
        }
    }
}

pub fn globals(source: &Path, destination: &Path, count: u32) -> Result<()> {
    use wasmparser::{Parser, Payload, TypeRef};
    let bytes = std::fs::read(source)?;
    let mut globals = 0u32;
    let mut section_start = 8;
    for payload in Parser::new(0).parse_all(&bytes) {
        let payload = payload?;
        match &payload {
            Payload::ImportSection(section) => {
                for import in section.clone().into_imports() {
                    if matches!(import?.ty, TypeRef::Global(_)) {
                        globals += 1;
                    }
                }
            }
            Payload::GlobalSection(section) => globals += section.count(),
            _ => {}
        }
        if let Payload::ExportSection(section) = &payload {
            ensure!(
                count <= globals,
                "requested {count} globals, module has {globals}"
            );
            for export in section.clone() {
                ensure!(
                    !export?.name.starts_with("__global_"),
                    "globals already exported"
                );
            }
            let range = section.range();
            let mut reader = wasmparser::BinaryReader::new(&bytes[range.clone()], range.start);
            let existing = reader.read_var_u32()?;
            let entries_start = reader.original_position();
            let mut body = Vec::new();
            existing
                .checked_add(count)
                .context("export count overflow")?
                .encode(&mut body);
            body.extend_from_slice(&bytes[entries_start..range.end]);
            for i in 0..count {
                let name = format!("__global_{i}");
                u32::try_from(name.len())?.encode(&mut body);
                body.extend_from_slice(name.as_bytes());
                body.push(3);
                i.encode(&mut body);
            }
            let mut result = bytes[..section_start].to_vec();
            result.push(7);
            u32::try_from(body.len())?.encode(&mut result);
            result.extend(body);
            result.extend_from_slice(&bytes[range.end..]);
            return write(destination, &result);
        }
        if let Some((_, range)) = payload.as_section() {
            section_start = range.end;
        }
    }
    anyhow::bail!("no export section found")
}
fn target(source: &Path, destination: &Path) -> Result<PathBuf> {
    ensure!(
        source.join("D5pLH9sfOOl.wasm").is_file(),
        "D5 capture missing"
    );
    std::fs::create_dir_all(destination)?;
    if source.canonicalize()? != destination.canonicalize()? {
        for entry in std::fs::read_dir(source)? {
            let path = entry?.path();
            if path.extension().is_some_and(|x| x == "wasm") {
                std::fs::copy(
                    &path,
                    destination.join(path.file_name().context("filename")?),
                )?;
            }
        }
    }
    Ok(destination.join("D5pLH9sfOOl.wasm"))
}
fn edit(bytes: &mut [u8], offset: usize, before: &[u8], after: &[u8]) -> Result<()> {
    ensure!(
        before.len() == after.len(),
        "patch changes instruction width"
    );
    let slice = bytes
        .get_mut(offset..offset + before.len())
        .context("patch outside module")?;
    ensure!(
        slice == before || slice == after,
        "capture differs at {offset:#x}; re-derive patch"
    );
    slice.copy_from_slice(after);
    Ok(())
}
pub fn profiler(source: &Path, destination: &Path) -> Result<()> {
    let path = target(source, destination)?;
    let mut bytes = std::fs::read(&path)?;
    ensure!(
        bytes.get(8338642..8338647) == Some(&[0x41, 0xd8, 0xf2, 0xd2, 0x00]),
        "profiler constant moved"
    );
    edit(&mut bytes, 8338647, &[0x2d, 0, 0], &[0x1a, 0x41, 0])?;
    write(&path, &bytes)
}
pub fn offer_errors(source: &Path, destination: &Path) -> Result<()> {
    let path = target(source, destination)?;
    let mut bytes = std::fs::read(&path)?;
    let sites = [
        5095464, 5096178, 5096316, 5097017, 5097359, 5098981, 5099045, 5099100, 5099125,
    ];
    let mut original = Vec::new();
    Instruction::I32Const(70008).encode(&mut original);
    for (offset, code) in sites.into_iter().zip([
        70001, 70002, 70003, 70004, 70005, 70006, 70007, 70009, 70010,
    ]) {
        let mut patch = Vec::new();
        Instruction::I32Const(code).encode(&mut patch);
        edit(&mut bytes, offset, &original, &patch)?;
    }
    write(&path, &bytes)
}
pub fn offer_guard(source: &Path, destination: &Path) -> Result<()> {
    let mut bytes = std::fs::read(source)?;
    let mut pattern = vec![0x20, 2, 0x0d, 0, 0x20, 0, 0x2d, 0];
    662166u32.encode(&mut pattern);
    pattern.extend([0x0d, 0]);
    let hits = bytes
        .windows(pattern.len())
        .enumerate()
        .filter_map(|(i, b)| (b == pattern).then_some(i))
        .collect::<Vec<_>>();
    ensure!(
        hits.len() == 1,
        "offer guard has {} matches; refusing to guess",
        hits.len()
    );
    edit(&mut bytes, hits[0], &[0x20, 2], &[0x41, 1])?;
    write(destination, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mismatched_and_ambiguous_patches_do_not_write_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.wasm");
        let out = dir.path().join("output.wasm");
        std::fs::write(&source, b"unrelated bytes").unwrap();
        assert!(offer_guard(&source, &out).is_err());
        assert!(!out.exists());
        let mut pattern = vec![0x20, 2, 0x0d, 0, 0x20, 0, 0x2d, 0];
        662166u32.encode(&mut pattern);
        pattern.extend([0x0d, 0]);
        let mut duplicate = pattern.clone();
        duplicate.extend(&pattern);
        std::fs::write(&source, duplicate).unwrap();
        assert!(offer_guard(&source, &out).is_err());
        assert!(!out.exists());
        std::fs::write(&source, &pattern).unwrap();
        offer_guard(&source, &out).unwrap();
        let result = std::fs::read(&out).unwrap();
        assert_eq!(&result[..2], &[0x41, 1]);
        assert_eq!(&result[2..], &pattern[2..]);
    }

    #[test]
    fn media_compare_checks_persisted_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let expected = dir.path().join("expected");
        let actual = dir.path().join("actual");
        let observation = oracle_core::MediaObservation {
            stream: oracle_core::MediaStream::Audio,
            symbol: "env::audio".to_owned(),
            ordinal: 0,
            sequence: Some(1),
            timestamp: Some(960),
            payload: vec![1, 2, 3],
        };
        oracle_core::write_media_trace(&expected, std::slice::from_ref(&observation)).unwrap();
        oracle_core::write_media_trace(&actual, &[observation]).unwrap();
        run(
            dir.path(),
            Task::CompareMedia {
                expected: expected.clone(),
                actual: actual.clone(),
            },
        )
        .unwrap();

        std::fs::write(actual.join("record-0000.bin"), [1, 2, 4]).unwrap();
        assert!(run(dir.path(), Task::CompareMedia { expected, actual }).is_err());
    }
}
