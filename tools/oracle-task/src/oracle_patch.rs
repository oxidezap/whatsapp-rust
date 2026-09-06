//! Diagnostic patches preserve refusal rules; they never change the canonical captures.
use anyhow::{Context, Result, ensure};
use std::path::Path;
use wasm_encoder::{Encode, Instruction};
use xtask_support::write;

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
const D5: &str = "D5pLH9sfOOl";

fn read_d5(source: &Path) -> Result<Vec<u8>> {
    oracle_core::catalog::read_pinned_capture(D5, &source.join(format!("{D5}.wasm")))
}

fn write_diagnostic(destination: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = destination.join(format!("{D5}.{name}.wasm"));
    write(&path, bytes)?;
    println!("wrote {}", path.display());
    Ok(())
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
    let mut bytes = read_d5(source)?;
    ensure!(
        bytes.get(8338642..8338647) == Some(&[0x41, 0xd8, 0xf2, 0xd2, 0x00]),
        "profiler constant moved"
    );
    edit(&mut bytes, 8338647, &[0x2d, 0, 0], &[0x1a, 0x41, 0])?;
    write_diagnostic(destination, "profiler", &bytes)
}
pub fn offer_errors(source: &Path, destination: &Path) -> Result<()> {
    let mut bytes = read_d5(source)?;
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
    write_diagnostic(destination, "offer-errors", &bytes)
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
    fn a_matching_patch_site_does_not_authenticate_the_capture() {
        use std::io::{Seek, SeekFrom, Write};
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cache");
        std::fs::create_dir_all(&cache).unwrap();
        let directory = tempfile::tempdir_in(cache).unwrap();
        let source = directory.path().join("source");
        let destination = directory.path().join("patched");
        std::fs::create_dir(&source).unwrap();
        let mut file = std::fs::File::create(source.join("D5pLH9sfOOl.wasm")).unwrap();
        file.set_len(9_794_866).unwrap();
        let mut header = b"\0asm\x01\0\0\0".to_vec();
        header.push(0);
        9_794_853u32.encode(&mut header);
        header.push(0);
        file.write_all(&header).unwrap();
        file.seek(SeekFrom::Start(8_338_642)).unwrap();
        file.write_all(&[0x41, 0xd8, 0xf2, 0xd2, 0, 0x2d, 0, 0])
            .unwrap();
        drop(file);
        assert!(profiler(&source, &destination).is_err());
        assert!(!destination.exists());
    }

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
}
