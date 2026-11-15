//! Execute the pinned packed-latency helper; no live call or election is driven.
mod common;
use anyhow::{Result, ensure};
use oracle_core::{Runtime, derive::function_body_sha256};
use wacore::types::relay_latency::PackedRelayLatency;
use wasm_encoder::{ExportKind, ExportSection, Module, RawSection};
use wasmparser::{ExternalKind, Parser, Payload};
use wasmtime::Val;

const INDEX: u32 = 12063;
const BODY: &str = "6e3de2be670f29dd3dfd480542f40e59170a83558168146cfbcd80929cc03e54";

// Only exports change. Every other section, including the encoded function body,
// is copied verbatim; the original capture is first verified by the catalog.
fn expose(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        function_body_sha256(bytes, INDEX)? == BODY,
        "unpack selector mismatch"
    );
    let mut exports = ExportSection::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ExportSection(section) = payload? {
            for entry in section {
                let entry = entry?;
                ensure!(
                    entry.name != "__latency_unpack",
                    "reserved export already exists"
                );
                let kind = match entry.kind {
                    ExternalKind::Func => ExportKind::Func,
                    ExternalKind::Memory => ExportKind::Memory,
                    ExternalKind::Table => ExportKind::Table,
                    ExternalKind::Global => ExportKind::Global,
                    ExternalKind::Tag => ExportKind::Tag,
                    other => anyhow::bail!("unsupported export: {other:?}"),
                };
                exports.export(entry.name, kind, entry.index);
            }
        }
    }
    exports.export("__latency_unpack", ExportKind::Func, INDEX);
    let mut module = Module::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Some((id, range)) = payload?.as_section() {
            if id == 7 {
                module.section(&exports);
            } else {
                module.section(&RawSection {
                    id,
                    data: &bytes[usize::try_from(range.start)?..usize::try_from(range.end)?],
                });
            }
        }
    }
    let result = module.finish();
    ensure!(
        function_body_sha256(&result, INDEX)? == BODY,
        "unpack body changed"
    );
    Ok(result)
}

#[test]
fn packed_latency_matches_original_wasm_across_rounds_flags_and_boundaries() -> Result<()> {
    let _guard = common::threaded_guard();
    let Some(bytes) = common::capture("u-Bw8hwXJGE")? else {
        eprintln!("skipping: pinned relay-latency capture missing");
        return Ok(());
    };
    let exposed = expose(&bytes)?;
    eprintln!(
        "capture_sha256={} export_only_sha256={}",
        oracle_core::derive::sha256_hex(&bytes),
        oracle_core::derive::sha256_hex(&exposed)
    );
    let mut runtime = Runtime::instantiate(&exposed)?;
    let ptr = 3_003_000;
    let mut values = vec![33_554_468, 33_554_485, 33_554_688, 0, u32::MAX];
    for round in 0..64u32 {
        for flags in [0, 1 << 24, 1 << 31, (1 << 31) | (1 << 24)] {
            for rtt in [0, 1, 0x7f_fffe, 0x7f_ffff, 0x80_0000, 0xff_ffff] {
                values.push((round << 25) | flags | rtt);
            }
        }
    }
    for raw in values {
        runtime.write_bytes_at(ptr, &[0; 16])?;
        runtime.call(
            "__latency_unpack",
            &[Val::I32(ptr as i32), Val::I32(raw as i32)],
        )?;
        let state = runtime.read(ptr, 16)?;
        let rust = PackedRelayLatency::new(raw);
        assert_eq!(rust.round(), state[0], "raw={raw:#x}");
        let native_rtt = i32::from_le_bytes(state[12..16].try_into()?);
        let native_status = u32::from_le_bytes(state[4..8].try_into()?);
        assert_eq!(
            rust.rtt_ms(),
            if native_status == 1 {
                None
            } else {
                Some(native_rtt)
            },
            "raw={raw:#x}"
        );
    }
    ensure!(
        runtime.shared().total_calls() == 0,
        "recorded host imports: {:?}",
        runtime.shared().hot_calls()
    );
    ensure!(
        runtime.stubs_called().is_empty(),
        "invoked unsupported host stub"
    );
    Ok(())
}
