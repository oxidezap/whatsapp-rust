//! Integration tests against the real captured modules.
//!
//! They skip rather than fail when the capture directory is absent, because the
//! artifacts live in the whatsapp-rust checkout and this crate must stay usable
//! on its own.

use oracle_core::inspect::{EntryKind, Requirement};
use oracle_core::{Catalog, Inspection, Runtime};

/// The VoIP capture. Named once: see the note on the same constant in
/// `abi_inference.rs`.
const VOIP: &str = "JgwtTQVeWPm";

fn catalog() -> Option<Catalog> {
    Catalog::discover().ok()
}

macro_rules! skip_without_captures {
    () => {
        match catalog() {
            Some(catalog) => catalog,
            None => {
                eprintln!("skipping: no capture directory (set WA_WASM_DIR)");
                return;
            }
        }
    };
}

#[test]
fn catalog_finds_captured_modules() {
    let catalog = skip_without_captures!();
    assert!(
        !catalog.modules().is_empty(),
        "capture directory {} holds no .wasm files",
        catalog.dir().display()
    );
}

#[test]
fn mozjpeg_module_is_identifiable_and_loads() {
    let catalog = skip_without_captures!();
    let Ok(module) = catalog.resolve("php8T1oSIZM") else {
        eprintln!("skipping: mozjpeg module not in this capture");
        return;
    };

    let report = Inspection::run(&module).expect("inspection");
    assert!(
        report.requires.is_empty(),
        "mozjpeg should need no special proposal: {:?}",
        report.requires
    );

    // The module is minified, so its identity comes from data segments rather
    // than from symbol names.
    let bytes = std::fs::read(&module.path).expect("read module");
    let data = oracle_core::data::extract(&bytes, 6).expect("data segments");
    assert!(
        data.strings
            .iter()
            .any(|string| string.value.contains("mozjpeg")),
        "expected a mozjpeg marker in the data segments"
    );

    assert!(
        report
            .exports
            .iter()
            .any(|export| matches!(export.kind, EntryKind::Func(_))),
        "expected exported functions"
    );
}

#[test]
fn mozjpeg_instantiates_with_every_import_satisfied() {
    let catalog = skip_without_captures!();
    let Ok(module) = catalog.resolve("php8T1oSIZM") else {
        eprintln!("skipping: mozjpeg module not in this capture");
        return;
    };

    let bytes = std::fs::read(&module.path).expect("read module");
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");

    assert!(
        runtime.unstubbable.is_empty(),
        "every mozjpeg import should be satisfiable: {:?}",
        runtime.unstubbable
    );
    assert!(!runtime.functions().is_empty(), "expected callable exports");
}

#[test]
fn voip_module_is_reported_as_needing_threads() {
    let catalog = skip_without_captures!();
    let Ok(module) = catalog.resolve(VOIP) else {
        eprintln!("skipping: voip module not in this capture");
        return;
    };

    let report = Inspection::run(&module).expect("inspection");

    // Read straight from the memory section rather than inferred from a failed
    // load, so the diagnosis costs nothing and cannot drift with a runtime.
    assert!(
        report.requires.contains(&Requirement::Threads),
        "voip module should be reported as needing threads, got {:?}",
        report.requires
    );
}

/// The host must find the guest's memory whatever it is called.
///
/// A minified module exports it under a single letter — mozjpeg uses `x` — and
/// a host that looks only for `memory` cannot read that module at all. The
/// failure is silent and misattributed: every read returns an error and the
/// module looks broken.
#[test]
fn memory_is_found_under_a_minified_export_name() {
    let catalog = skip_without_captures!();
    let Ok(module) = catalog.resolve("php8T1oSIZM") else {
        eprintln!("skipping: mozjpeg module not in this capture");
        return;
    };

    let report = Inspection::run(&module).expect("inspection");
    let memory = report
        .exports
        .iter()
        .find(|export| matches!(export.kind, EntryKind::Memory { .. }))
        .expect("mozjpeg exports a memory");
    assert_ne!(
        memory.name, "memory",
        "this test is pointless if the export uses the conventional name"
    );

    let bytes = std::fs::read(&module.path).expect("read module");
    let runtime = Runtime::instantiate(&bytes).expect("instantiate");
    assert!(
        runtime.memory_size() > 0,
        "host did not locate the guest memory"
    );
}

/// What the mozjpeg module's minified exports do.
///
/// There is no glue in the capture, so the shapes come from behaviour. `B`
/// hands out increasing in-bounds pointers and refuses negative sizes, which is
/// an allocator; `H` takes nothing and reports a pointer into the same region.
/// Pinned so the reading is on record and a capture that renames them fails
/// here rather than somewhere confusing.
#[test]
fn mozjpeg_exposes_an_allocator() {
    let catalog = skip_without_captures!();
    let Ok(module) = catalog.resolve("php8T1oSIZM") else {
        eprintln!("skipping: mozjpeg module not in this capture");
        return;
    };
    let bytes = std::fs::read(&module.path).expect("read module");
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");

    // `y` is the start function: it takes and returns nothing.
    runtime.call("y", &[]).expect("start function");
    runtime.refuel();

    let size = runtime.memory_size();
    let alloc = |runtime: &mut Runtime, n: i32| -> Option<i32> {
        let out = runtime.call("B", &[wasmtime::Val::I32(n)]).ok()?;
        runtime.refuel();
        match out.first() {
            Some(wasmtime::Val::I32(ptr)) => Some(*ptr),
            _ => None,
        }
    };

    let first = alloc(&mut runtime, 16).expect("allocate");
    let second = alloc(&mut runtime, 16).expect("allocate");
    assert!(
        first > 0 && (first as usize) < size,
        "pointer out of memory"
    );
    assert!(
        second > first,
        "an allocator should not hand back the same address twice"
    );

    // A negative size is refused rather than wrapping into a huge allocation.
    assert_eq!(alloc(&mut runtime, -16), Some(0));
}
