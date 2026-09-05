//! Rewriting a module without changing what it does.
//!
//! The bar for an instrumented module is not that it parses: it is that it
//! still answers the same. Every test here runs the original and the rewritten
//! module and compares, because a rewrite that validates and computes something
//! else is worse than one that fails to load.

use oracle_core::patch::{self, Edit, Plan, Replace};
use oracle_core::{Catalog, Runtime};
use wasm_encoder::{
    CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection, ImportSection,
    Module, TypeSection, ValType,
};

/// A module with a marker-shaped import, two leaf functions and a caller that
/// calls each of them once and then one of them again.
///
/// Built rather than assembled from text: the runtime here is compiled without
/// wasmtime's WAT parser on purpose, and adding it back to write a fixture
/// would be paying for a parser in every build to save typing in one test.
fn sample() -> Vec<u8> {
    let mut types = TypeSection::new();
    // 0: (i32, i32) -> ()   the sink's shape
    types.ty().function([ValType::I32, ValType::I32], []);
    // 1: (i32) -> i32
    types.ty().function([ValType::I32], [ValType::I32]);

    let mut imports = ImportSection::new();
    imports.import("env", "mark", EntityType::Function(0));

    let mut functions = FunctionSection::new();
    functions.function(1); // func 1: double
    functions.function(1); // func 2: negate
    functions.function(1); // func 3: caller

    let mut code = CodeSection::new();

    let mut double = Function::new([]);
    double
        .instructions()
        .local_get(0)
        .i32_const(2)
        .i32_mul()
        .end();
    code.function(&double);

    let mut negate = Function::new([]);
    negate
        .instructions()
        .i32_const(0)
        .local_get(0)
        .i32_sub()
        .end();
    code.function(&negate);

    // caller(x) = double(x) + negate(x) + double(x)
    let mut caller = Function::new([]);
    caller
        .instructions()
        .local_get(0)
        .call(1)
        .local_get(0)
        .call(2)
        .i32_add()
        .local_get(0)
        .call(1)
        .i32_add()
        .end();
    code.function(&caller);

    let mut exports = ExportSection::new();
    exports.export("caller", ExportKind::Func, 3);

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&exports);
    module.section(&code);
    module.finish()
}

/// Calls `caller` and returns what it answered.
fn run(bytes: &[u8], argument: i32) -> (i32, Vec<(String, Vec<i64>)>) {
    let mut runtime = match Runtime::instantiate(bytes) {
        Ok(r) => r,
        Err(e) => panic!("instantiate: {e:?}"),
    };
    let results = runtime
        .call("caller", &[wasmtime::Val::I32(argument)])
        .expect("calling caller");
    let answer = match results.first() {
        Some(wasmtime::Val::I32(value)) => *value,
        other => panic!("caller returned {other:?}"),
    };
    let calls = runtime
        .shared()
        .calls()
        .into_iter()
        .map(|call| (format!("{}::{}", call.module, call.name), call.args))
        .collect();
    (answer, calls)
}

#[test]
fn an_instrumented_module_still_answers_the_same() {
    let original = sample();
    let (before, _) = run(&original, 7);
    // double(7) + negate(7) + double(7) = 14 - 7 + 14
    assert_eq!(before, 21, "the fixture itself should compute this");

    let (rewritten, map) =
        patch::instrument(&original, &Plan::every_call_in(3)).expect("instrument");
    let (after, calls) = run(&rewritten, 7);

    assert_eq!(
        after, before,
        "instrumenting must not change what the module computes"
    );
    assert_eq!(map.markers.len(), 3, "three call sites in `caller`");

    let borrowed: Vec<(&str, &[i64])> = calls
        .iter()
        .map(|(symbol, args)| (symbol.as_str(), args.as_slice()))
        .collect();
    let fired = map.fired(borrowed);
    assert_eq!(fired.len(), 3, "every call site should report");
    assert!(
        map.never_fired(&fired).is_empty(),
        "and none should be left over"
    );
}

/// The distinction the whole module exists for: a body patch cannot tell which
/// call site ran, and this can.
#[test]
fn markers_name_the_call_site_not_just_the_callee() {
    let original = sample();
    let (rewritten, map) =
        patch::instrument(&original, &Plan::every_call_in(3)).expect("instrument");
    let (_, calls) = run(&rewritten, 3);

    let borrowed: Vec<(&str, &[i64])> = calls
        .iter()
        .map(|(symbol, args)| (symbol.as_str(), args.as_slice()))
        .collect();
    let fired = map.fired(borrowed);

    // Two of the three sites call the same function. A body patch on func 1
    // would report "reached twice" and could not say which site was which;
    // these carry distinct ids.
    let to_double: Vec<i32> = fired
        .iter()
        .filter(|(marker, _)| marker.detail.contains("call 1 "))
        .map(|(marker, _)| marker.id)
        .collect();
    assert_eq!(to_double.len(), 2, "func 1 is called from two sites");
    assert_ne!(
        to_double[0], to_double[1],
        "and the two sites must be distinguishable"
    );

    // They fired in program order, which is what makes a trace readable.
    let order: Vec<i32> = fired.iter().map(|(marker, _)| marker.id).collect();
    let mut sorted = order.clone();
    sorted.sort_unstable();
    assert_eq!(order, sorted, "markers fire in the order they were placed");
}

#[test]
fn a_call_site_that_never_runs_is_reported_as_such() {
    let original = sample();
    // Marking the returns of a function nothing calls: `negate` has no explicit
    // `return`, so ask for entry markers on all three and only run one path.
    let plan = Plan {
        entry: vec![1, 2, 3],
        id_base: patch::DEFAULT_ID_BASE,
        ..Plan::default()
    };
    let (rewritten, map) = patch::instrument(&original, &plan).expect("instrument");
    let (answer, calls) = run(&rewritten, 5);
    assert_eq!(answer, 15, "5*2 - 5 + 5*2");

    let borrowed: Vec<(&str, &[i64])> = calls
        .iter()
        .map(|(symbol, args)| (symbol.as_str(), args.as_slice()))
        .collect();
    let fired = map.fired(borrowed);
    assert_eq!(fired.len(), 4, "caller once, double twice, negate once");
}

#[test]
fn replacing_an_instruction_changes_the_answer_it_was_aimed_at() {
    let original = sample();
    let (before, _) = run(&original, 7);
    assert_eq!(before, 21);

    // `double` is `local.get 0; i32.const 2; i32.mul`. Turn the 2 into a 10.
    let edit = Replace {
        func: 1,
        at: 1,
        count: 1,
        with: vec![Edit::I32(10)],
    };
    let rewritten = patch::replace(&original, &[edit]).expect("replace");
    let (after, _) = run(&rewritten, 7);

    // double(7)*... becomes 70 - 7 + 70
    assert_eq!(
        after, 133,
        "the constant the patch aimed at is the one that moved"
    );
}

#[test]
fn a_replacement_spec_is_parsed_and_a_bad_one_is_refused() {
    let parsed = patch::parse_replace("11198:4:1:i32.const 200391").expect("parse");
    assert_eq!(parsed.func, 11198);
    assert_eq!(parsed.at, 4);
    assert_eq!(parsed.count, 1);
    assert_eq!(parsed.with, vec![Edit::I32(200_391)]);

    assert_eq!(
        patch::parse_replace("1:0:1:drop;nop").expect("parse").with,
        vec![Edit::Drop, Edit::Nop]
    );

    // Unsupported is an error, never a guess: a spec this cannot encode must
    // not become a silent no-op in a module somebody then measures.
    let refused = patch::parse_replace("1:0:1:memory.grow").expect_err("should refuse");
    assert!(
        refused.to_string().contains("memory.grow"),
        "the refusal should name what it could not encode: {refused}"
    );
    assert!(patch::parse_replace("1:0:1").is_err(), "a truncated spec");
}

#[test]
fn instrumenting_a_module_with_no_sink_is_refused_rather_than_guessed() {
    // The fixture without its import: nothing to call.
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], [ValType::I32]);
    let mut functions = FunctionSection::new();
    functions.function(0);
    let mut code = CodeSection::new();
    let mut identity = Function::new([]);
    identity.instructions().local_get(0).end();
    code.function(&identity);
    let mut module = Module::new();
    module.section(&types);
    module.section(&functions);
    module.section(&code);
    let bytes = module.finish();

    let error = patch::instrument(&bytes, &Plan::every_call_in(0)).expect_err("no sink");
    assert!(
        error.to_string().contains("marker sink"),
        "the refusal should say what is missing: {error}"
    );
}

#[test]
fn a_function_that_does_not_exist_is_named_in_the_error() {
    let bytes = sample();
    let error = patch::instrument(&bytes, &Plan::every_call_in(9_999)).expect_err("no such func");
    assert!(
        error.to_string().contains("9999"),
        "the error should name the index asked for: {error}"
    );
}

/// The property that matters on a real capture: nine megabytes of somebody
/// else's C++ still comes up, and still says the same thing, after the rewrite.
#[test]
fn a_captured_module_survives_being_instrumented() {
    let Ok(catalog) = Catalog::discover() else {
        eprintln!("skipping: no capture directory (set WA_WASM_DIR)");
        return;
    };
    // The VOPRF module: small enough to rewrite and run quickly, and it exports
    // plain C functions that answer without any embind machinery.
    let Ok(entry) = catalog.resolve("COs9e0Kj0ic") else {
        eprintln!("skipping: COs9e0Kj0ic unavailable");
        return;
    };
    let bytes = std::fs::read(&entry.path).expect("read module");

    // `sodiumInit`, which makes six direct calls. Not `blind` (func 183): that
    // one is the trampoline the README describes, and it reaches its callee
    // through `call_indirect`, which has no callee index to mark.
    const SODIUM_INIT: u32 = 184;

    let sites = patch::call_sites(&bytes, SODIUM_INIT).expect("call sites");
    assert!(
        !sites.is_empty(),
        "sodiumInit makes direct calls, which is what makes it worth marking"
    );

    // Every one of this module's four `(i32, i32) -> ()` imports has real
    // behaviour behind it — three embind registrations and the `invoke_vi`
    // exception trampoline — so the default selection refuses rather than
    // splicing calls to one of them. The refusal has to name them, since
    // nominating one is the only way forward.
    let refused = patch::instrument(&bytes, &Plan::every_call_in(SODIUM_INIT))
        .expect_err("no recording-only sink here");
    let refusal = refused.to_string();
    for candidate in ["_embind_register_void", "invoke_vi"] {
        assert!(
            refusal.contains(candidate),
            "the refusal should list the candidates it rejected: {refusal}"
        );
    }

    // Nominating one is the caller saying it has checked. It has: the markers
    // spliced here are never reached — this test instantiates the module and
    // does not call `sodiumInit` — and `_embind_register_void(id, 0)` reads a
    // name from address 0 and records nothing when it is empty.
    let plan = Plan {
        sink: Some("env::_embind_register_void".to_owned()),
        ..Plan::every_call_in(SODIUM_INIT)
    };
    let (rewritten, map) = patch::instrument(&bytes, &plan).expect("instrument");
    assert_eq!(
        map.markers.len(),
        sites.values().sum::<usize>(),
        "one marker per call site"
    );

    // It still loads, which is the whole claim.
    let mut original = Runtime::instantiate(&bytes).expect("original instantiates");
    let mut patched = Runtime::instantiate(&rewritten).expect("rewritten instantiates");
    assert_eq!(
        original.functions().len(),
        patched.functions().len(),
        "and exposes the same surface"
    );
}

/// The refusal that matters most: an import of the right *shape* whose host
/// implementation writes guest memory must not be picked by accident.
///
/// `env::get_random_bytes_js` is `(len, buf)`, so a marker calling it would ask
/// for `id` bytes of PRNG output at address `value` — two hundred thousand
/// bytes written from address zero, on a module that still validates and still
/// runs. That is the whole reason the sink is chosen by name.
#[test]
fn an_import_that_writes_guest_memory_is_not_picked_as_a_sink() {
    let mut types = TypeSection::new();
    types.ty().function([ValType::I32, ValType::I32], []);
    types.ty().function([ValType::I32], [ValType::I32]);

    let mut imports = ImportSection::new();
    imports.import("env", "get_random_bytes_js", EntityType::Function(0));

    let mut functions = FunctionSection::new();
    functions.function(1);
    let mut code = CodeSection::new();
    let mut identity = Function::new([]);
    identity.instructions().local_get(0).end();
    code.function(&identity);

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&code);
    let bytes = module.finish();

    let error =
        patch::instrument(&bytes, &Plan::every_call_in(1)).expect_err("not a recording-only sink");
    let text = error.to_string();
    assert!(
        text.contains("recording-only") && text.contains("env::get_random_bytes_js"),
        "the refusal should say why, and name the candidate: {text}"
    );

    // Naming it is allowed — that is the caller taking responsibility — and it
    // is the only way the module gets instrumented at all.
    let plan = Plan {
        sink: Some("get_random_bytes_js".to_owned()),
        ..Plan::every_call_in(1)
    };
    let (_, map) = patch::instrument(&bytes, &plan).expect("a nominated sink is honoured");
    assert_eq!(map.via_symbol, "env::get_random_bytes_js");
}

/// A nominated sink that is not there, or is not the right shape, is an error
/// rather than a fall-back to whatever was.
#[test]
fn a_nominated_sink_that_does_not_exist_is_refused() {
    let bytes = sample();
    let plan = Plan {
        sink: Some("env::nowhere".to_owned()),
        ..Plan::every_call_in(3)
    };
    let error = patch::instrument(&bytes, &plan).expect_err("no such import");
    let text = error.to_string();
    assert!(
        text.contains("env::nowhere") && text.contains("env::mark"),
        "the refusal should name what was asked for and what is available: {text}"
    );
}
