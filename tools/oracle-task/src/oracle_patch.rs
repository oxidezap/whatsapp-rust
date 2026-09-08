//! Diagnostic patches preserve refusal rules; they never change the canonical captures.
use anyhow::{Context, Result, ensure};
use std::path::Path;
use wasm_encoder::{Encode, Instruction};

/// wasmparser 0.258 reports byte ranges and positions as `u64`. File offsets
/// here stay `usize`, so every crossing converts fallibly instead of
/// truncating with `as`.
fn to_usize(offset: u64) -> Result<usize> {
    usize::try_from(offset).context("wasm offset does not fit in usize")
}

fn to_u64(offset: usize) -> Result<u64> {
    u64::try_from(offset).context("wasm offset does not fit in u64")
}
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
            let range = to_usize(range.start)?..to_usize(range.end)?;
            let mut reader =
                wasmparser::BinaryReader::new(&bytes[range.clone()], to_u64(range.start)?);
            let existing = reader.read_var_u32()?;
            let entries_start = to_usize(reader.original_position())?;
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
            section_start = to_usize(range.end)?;
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
    let site = guard_site(&bytes)?;
    edit(&mut bytes, site, &[0x20, 2], &[0x41, 1])?;
    // The rewrite must still validate as a module, not merely contain the
    // patched bytes.
    wasmparser::Validator::new().validate_all(&bytes)?;
    write(destination, &bytes)
}

/// Offset of the `local.get 2` opening the offer-guard sequence, located
/// through decoded operators: a whole-file byte scan also matches data
/// segments and encoded immediates that merely contain the same bytes.
fn guard_site(bytes: &[u8]) -> Result<usize> {
    let mut bodies = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload? {
            let range = body.range();
            let range = to_usize(range.start)?..to_usize(range.end)?;
            let start = to_usize(body.get_operators_reader()?.original_position())?;
            bodies.push((range, start));
        }
    }
    let mut hits = Vec::new();
    for (range, start) in &bodies {
        let reader = wasmparser::OperatorsReader::new(wasmparser::BinaryReader::new(
            &bytes[range.clone()][start - range.start..],
            to_u64(*start)?,
        ));
        let operators = reader
            .into_iter_with_offsets()
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for window in operators.windows(5) {
            let [
                (wasmparser::Operator::LocalGet { local_index: 2 }, at),
                (wasmparser::Operator::BrIf { relative_depth: 0 }, _),
                (wasmparser::Operator::LocalGet { local_index: 0 }, _),
                (wasmparser::Operator::I32Load8U { memarg }, _),
                (wasmparser::Operator::BrIf { relative_depth: 0 }, _),
            ] = window
            else {
                continue;
            };
            if memarg.align == 0 && memarg.offset == 662166 {
                hits.push(to_usize(*at)?);
            }
        }
    }
    ensure!(
        hits.len() == 1,
        "offer guard has {} matches; refusing to guess",
        hits.len()
    );
    Ok(hits[0])
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
    fn guard_matches_decode_code_not_raw_bytes() {
        use wasm_encoder::{
            BlockType, CodeSection, DataSection, ExportKind, ExportSection, Function,
            FunctionSection, MemArg, MemorySection, MemoryType, Module, TypeSection, ValType,
        };
        fn guard_module(copies: usize, decoy: bool) -> Vec<u8> {
            let mut types = TypeSection::new();
            types
                .ty()
                .function([ValType::I32, ValType::I32, ValType::I32], [ValType::I32]);
            let mut functions = FunctionSection::new();
            for _ in 0..copies {
                functions.function(0);
            }
            let mut memories = MemorySection::new();
            memories.memory(MemoryType {
                minimum: 16,
                maximum: None,
                memory64: false,
                shared: false,
                page_size_log2: None,
            });
            let mut exports = ExportSection::new();
            exports.export("memory", ExportKind::Memory, 0);
            let mut code = CodeSection::new();
            for _ in 0..copies {
                let mut body = Function::new([]);
                body.instructions()
                    .block(BlockType::Empty)
                    .local_get(2)
                    .br_if(0)
                    .local_get(0)
                    .i32_load8_u(MemArg {
                        offset: 662166,
                        align: 0,
                        memory_index: 0,
                    })
                    .br_if(0)
                    .end()
                    .local_get(0)
                    .end();
                code.function(&body);
            }
            let mut module = Module::new();
            module
                .section(&types)
                .section(&functions)
                .section(&memories)
                .section(&exports)
                .section(&code);
            if decoy {
                let mut pattern = vec![0x20, 2, 0x0d, 0, 0x20, 0, 0x2d, 0];
                662166u32.encode(&mut pattern);
                pattern.extend([0x0d, 0]);
                let mut data = DataSection::new();
                data.active(0, &wasm_encoder::ConstExpr::i32_const(0), pattern);
                module.section(&data);
            }
            module.finish()
        }
        // One code match beside a byte-identical data decoy still patches.
        let cache = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cache");
        std::fs::create_dir_all(&cache).unwrap();
        let directory = tempfile::tempdir_in(cache).unwrap();
        let source = directory.path().join("input.wasm");
        let out = directory.path().join("output.wasm");
        std::fs::write(&source, guard_module(1, true)).unwrap();
        offer_guard(&source, &out).unwrap();
        let result = std::fs::read(&out).unwrap();
        wasmparser::Validator::new().validate_all(&result).unwrap();
        let input = std::fs::read(&source).unwrap();
        assert_eq!(input.len(), result.len());
        let diffs: Vec<usize> = input
            .iter()
            .zip(&result)
            .enumerate()
            .filter_map(|(index, (before, after))| (before != after).then_some(index))
            .collect();
        assert_eq!(diffs.len(), 2, "only the guard head is rewritten");
        assert_eq!(&result[diffs[0]..diffs[0] + 2], &[0x41, 1]);
        // Two code matches refuse to guess.
        std::fs::write(&source, guard_module(2, false)).unwrap();
        assert!(offer_guard(&source, &out).is_err());
        // Bytes outside code never match on their own.
        std::fs::write(&source, guard_module(0, true)).unwrap();
        assert!(offer_guard(&source, &out).is_err());
    }

    #[test]
    fn invalid_modules_do_not_write_outputs() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("input.wasm");
        let out = dir.path().join("output.wasm");
        std::fs::write(&source, b"unrelated bytes").unwrap();
        assert!(offer_guard(&source, &out).is_err());
        assert!(!out.exists());
        // Raw guard bytes are not a module, so they never match on their own.
        let mut pattern = vec![0x20, 2, 0x0d, 0, 0x20, 0, 0x2d, 0];
        662166u32.encode(&mut pattern);
        pattern.extend([0x0d, 0]);
        std::fs::write(&source, &pattern).unwrap();
        assert!(offer_guard(&source, &out).is_err());
        assert!(!out.exists());
    }
}
