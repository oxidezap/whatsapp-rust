//! Audio/video callback capture for future full media differential oracles.

use oracle_core::{
    MediaObservation, MediaStream, MediaWatch, Runtime, compare_media, read_media_trace,
    write_media_trace,
};
use wasm_encoder::{
    CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
    FunctionSection, ImportSection, MemorySection, MemoryType, Module, TypeSection, ValType,
};

const PAYLOAD: &[u8] = &[0x65, 0x88, 0x84, 0x21];

fn fixture(length: i32) -> Vec<u8> {
    let mut types = TypeSection::new();
    types
        .ty()
        .function([ValType::I32, ValType::I32, ValType::I32, ValType::I32], []);
    types.ty().function([], []);

    let mut imports = ImportSection::new();
    imports.import("env", "emit_media", EntityType::Function(0));

    let mut functions = FunctionSection::new();
    functions.function(1);

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut data = DataSection::new();
    data.active(0, &ConstExpr::i32_const(1024), PAYLOAD.iter().copied());

    let mut body = Function::new([]);
    body.instructions()
        .i32_const(1024)
        .i32_const(length)
        .i32_const(7)
        .i32_const(960)
        .call(0)
        .end();
    let mut code = CodeSection::new();
    code.function(&body);

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("run", ExportKind::Func, 1);

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&memories);
    module.section(&exports);
    module.section(&code);
    module.section(&data);
    module.finish()
}

fn watch() -> MediaWatch {
    MediaWatch::new("env", "emit_media", MediaStream::Video, 0, 1)
        .unwrap()
        .with_sequence_arg(2)
        .with_timestamp_arg(3)
}

#[test]
fn callback_payload_and_transport_metadata_are_captured_together() {
    let mut runtime = Runtime::instantiate(&fixture(PAYLOAD.len() as i32)).unwrap();
    runtime.watch_media([watch()]).unwrap();
    runtime.call("run", &[]).unwrap();

    let observations = runtime.take_media_observations().unwrap();
    assert_eq!(
        observations,
        [MediaObservation {
            stream: MediaStream::Video,
            symbol: "env::emit_media".to_owned(),
            ordinal: 0,
            sequence: Some(7),
            timestamp: Some(960),
            payload: PAYLOAD.to_vec(),
        }]
    );
}

#[test]
fn malformed_or_oversized_callbacks_fail_the_probe() {
    let mut runtime = Runtime::instantiate(&fixture(i32::MAX)).unwrap();
    assert!(runtime.watch_media([watch().with_sequence_arg(0)]).is_err());
    runtime.watch_media([watch()]).unwrap();
    runtime.call("run", &[]).unwrap();
    let error = runtime.take_media_observations().unwrap_err();
    assert!(error.to_string().contains("exceeds"), "{error:#}");
}

#[test]
fn comparison_reports_the_first_payload_difference() {
    let record = MediaObservation {
        stream: MediaStream::Audio,
        symbol: "env::audio".to_owned(),
        ordinal: 0,
        sequence: Some(1),
        timestamp: Some(960),
        payload: vec![1, 2, 3],
    };
    assert!(compare_media(std::slice::from_ref(&record), std::slice::from_ref(&record)).is_ok());
    let mut changed = record;
    changed.payload[2] = 4;
    let error = compare_media(
        &[changed],
        &[MediaObservation {
            stream: MediaStream::Audio,
            symbol: "rust::audio".to_owned(),
            ordinal: 0,
            sequence: Some(1),
            timestamp: Some(960),
            payload: vec![1, 2, 3],
        }],
    )
    .unwrap_err();
    assert!(error.to_string().contains("payload differs"), "{error:#}");
}

#[test]
fn persisted_traces_are_content_addressed_and_sweep_stale_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let observation = MediaObservation {
        stream: MediaStream::Audio,
        symbol: "env::audio".to_owned(),
        ordinal: 0,
        sequence: Some(3),
        timestamp: Some(960),
        payload: vec![1, 2, 3],
    };
    std::fs::write(directory.path().join("record-9999.bin"), b"stale").unwrap();
    write_media_trace(directory.path(), std::slice::from_ref(&observation)).unwrap();
    assert!(!directory.path().join("record-9999.bin").exists());
    assert_eq!(read_media_trace(directory.path()).unwrap(), [observation]);

    std::fs::write(directory.path().join("record-0000.bin"), b"tampered").unwrap();
    assert!(read_media_trace(directory.path()).is_err());
}
