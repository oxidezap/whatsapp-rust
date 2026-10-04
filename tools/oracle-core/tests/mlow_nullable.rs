//! Pinned-source evidence for the nullable LSF conditional-centroid argument.

mod common;

use anyhow::{Context, Result};
use oracle_core::derive::{function_body_sha256, run_spec};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use wasmparser::{BlockType, Operator, Parser, Payload, TypeRef, ValType};

#[test]
fn lsf_conditional_centroids_are_guarded_by_the_pointer() -> Result<()> {
    let Some(bytes) = common::capture("JgwtTQVeWPm")? else {
        eprintln!("skipping: JgwtTQVeWPm unavailable (set WA_WASM_DIR)");
        return Ok(());
    };
    assert_eq!(
        hex::encode(Sha256::digest(&bytes)),
        "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db"
    );
    assert_eq!(
        function_body_sha256(&bytes, 10804)?,
        "1ef81d8df9e1e437283901331033dee945ff0871a30233536f94d7c8e33d51a6"
    );
    let mut index = 0;
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.map_err(anyhow::Error::msg)? {
            Payload::ImportSection(reader) => {
                for import in reader.into_imports() {
                    if matches!(
                        import.map_err(anyhow::Error::msg)?.ty,
                        TypeRef::Func(_) | TypeRef::FuncExact(_)
                    ) {
                        index += 1;
                    }
                }
            }
            Payload::CodeSectionEntry(body) => {
                if index == 10804 {
                    let ops = body
                        .get_operators_reader()
                        .map_err(anyhow::Error::msg)?
                        .into_iter()
                        .collect::<std::result::Result<Vec<_>, _>>()
                        .map_err(anyhow::Error::msg)?;
                    // The first access to param7 branches on null before reading
                    // its centroids; null chooses 16 unconditional candidates,
                    // nonnull adds the 17th (conditional) candidate.
                    assert!(matches!(ops[356], Operator::LocalGet { local_index: 7 }));
                    assert!(matches!(
                        ops[357],
                        Operator::If {
                            blockty: BlockType::Type(ValType::I32)
                        }
                    ));
                    assert!(matches!(ops[427], Operator::I32Const { value: 17 }));
                    assert!(matches!(ops[428], Operator::Else));
                    assert!(matches!(ops[429], Operator::I32Const { value: 16 }));
                    assert!(matches!(ops[430], Operator::End));
                    // Its state layout also selects the unconditional path on
                    // null; address zero is not a conditional-centroid buffer.
                    assert!(matches!(ops[462], Operator::I32Const { value: 2048 }));
                    assert!(matches!(ops[463], Operator::I32Const { value: 21572 }));
                    assert!(matches!(ops[464], Operator::LocalGet { local_index: 7 }));
                    assert!(matches!(ops[465], Operator::Select));
                    return Ok(());
                }
                index += 1;
            }
            _ => {}
        }
    }
    anyhow::bail!("pinned LSF core function missing")
}

#[test]
fn nullable_lsf_observation_preserves_all_packets_pcm_and_present_centroids() -> Result<()> {
    let Some(bytes) = common::capture("JgwtTQVeWPm")? else {
        eprintln!("skipping: JgwtTQVeWPm unavailable (set WA_WASM_DIR)");
        return Ok(());
    };
    let (_local, _cross_process) = common::threaded_guard();
    let work = tempfile::tempdir_in(env!("CARGO_MANIFEST_DIR"))?;
    let module = work.path().join("JgwtTQVeWPm.wasm");
    std::fs::write(&module, bytes)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut spec: Value = serde_json::from_str(include_str!("../specs/mlow_110frames.json"))?;
    spec["functions"]["lsf_core"] = json!({
        "index_hint": 10804,
        "expect_body_sha256": "1ef81d8df9e1e437283901331033dee945ff0871a30233536f94d7c8e33d51a6"
    });
    let steps = spec["steps"].as_array_mut().context("base steps")?;
    // Absolute input paths avoid changing the process-wide current directory.
    for step in steps.iter_mut().filter(|s| s["op"] == "write_file") {
        step["file"] = json!(root.join(step["file"].as_str().context("input file")?));
    }
    steps.splice(0..0, [
        json!({"op":"capture_memory","func":"lsf_core","instruction":0,"local":7,"len":2176,"count":330,"out":"lsf_cond"}),
        json!({"op":"capture_value","func":"lsf_core","instruction":0,"local":7,"count":330,"out":"lsf_cond_ptr"}),
    ]);
    let path = work.path().join("probe.json");
    std::fs::write(&path, serde_json::to_vec(&spec)?)?;
    let raw_dir = work.path().join("raw");
    let raw = run_spec(&path, &module, &raw_dir)?;
    spec["steps"][0]["nullable"] = json!(true);
    std::fs::write(&path, serde_json::to_vec(&spec)?)?;
    let nullable_dir = work.path().join("nullable");
    let nullable = run_spec(&path, &module, &nullable_dir)?;
    assert_eq!(raw.outputs.len(), nullable.outputs.len());
    let mut absent = 0;
    let mut present = 0;
    for (before, after) in raw.outputs.iter().zip(&nullable.outputs) {
        assert_eq!(before.file, after.file);
        if let Some(suffix) = before.file.strip_prefix("lsf_cond_")
            && !suffix.starts_with("ptr_")
            && std::fs::read(raw_dir.join(format!("lsf_cond_ptr_{suffix}")))? == [0; 4]
        {
            absent += 1;
            assert_eq!(before.bytes, 2176);
            assert_eq!(after.bytes, 0);
            assert!(std::fs::read(nullable_dir.join(&after.file))?.is_empty());
        } else {
            if before.file.starts_with("lsf_cond_") && !before.file.starts_with("lsf_cond_ptr_") {
                present += 1;
            }
            assert_eq!(before.bytes, after.bytes, "{}", before.file);
            assert_eq!(before.sha256, after.sha256, "{}", before.file);
        }
    }
    assert_eq!(absent, 111);
    assert_eq!(present, 219);
    eprintln!(
        "pinned J LSF: 111 absent + 219 present centroids; all 220 packet/PCM hashes preserved"
    );
    Ok(())
}
