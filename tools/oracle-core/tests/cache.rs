//! Wasmtime disk-cache persistence and module identity.

mod common;

use wasmtime::{Cache, CacheConfig, Config, Engine, Instance, Module, Store};

fn module(value: i32) -> Vec<u8> {
    use wasm_encoder::{
        CodeSection, ExportKind, ExportSection, Function, FunctionSection, Instruction,
        TypeSection, ValType,
    };
    let mut module = wasm_encoder::Module::new();
    let mut types = TypeSection::new();
    types.ty().function([], [ValType::I32]);
    module.section(&types);
    let mut functions = FunctionSection::new();
    functions.function(0);
    module.section(&functions);
    let mut exports = ExportSection::new();
    exports.export("value", ExportKind::Func, 0);
    module.section(&exports);
    let mut body = Function::new([]);
    body.instruction(&Instruction::I32Const(value));
    body.instruction(&Instruction::End);
    let mut code = CodeSection::new();
    code.function(&body);
    module.section(&code);
    module.finish()
}

fn execute(cache: Cache, bytes: &[u8]) -> i32 {
    let mut config = Config::new();
    config.cache(Some(cache));
    config.cranelift_opt_level(wasmtime::OptLevel::None);
    let engine = Engine::new(&config).unwrap();
    let module = Module::new(&engine, bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    instance
        .get_typed_func::<(), i32>(&mut store, "value")
        .unwrap()
        .call(&mut store, ())
        .unwrap()
}

#[test]
fn compiled_module_cache_survives_fresh_engines_and_distinguishes_module_bytes() {
    let _guard = common::threaded_guard();
    let root =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cache/oracle-cache-tests");
    std::fs::create_dir_all(&root).unwrap();
    let directory = tempfile::tempdir_in(root.canonicalize().unwrap()).unwrap();
    let cache = || {
        let mut config = CacheConfig::new();
        config.with_directory(directory.path());
        Cache::new(config).unwrap()
    };
    let first = cache();
    assert_eq!(execute(first.clone(), &module(7)), 7);
    assert!(first.cache_misses() > 0);
    assert_eq!(first.cache_hits(), 0);
    let second = cache();
    assert_eq!(execute(second.clone(), &module(7)), 7);
    assert!(second.cache_hits() > 0);
    assert_eq!(second.cache_misses(), 0);
    let changed = cache();
    assert_eq!(execute(changed.clone(), &module(9)), 9);
    assert!(changed.cache_misses() > 0);
}
