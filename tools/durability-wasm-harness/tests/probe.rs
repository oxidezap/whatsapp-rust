#![cfg(target_arch = "wasm32")]

// Compile the builder's identifier generator, not a test copy. The root
// crate's native dev-dependencies prevent running its unit tests on wasm.
#[path = "../../../src/client/durability_probe_id.rs"]
mod durability_probe_id;

#[wasm_bindgen_test::wasm_bindgen_test]
fn durability_probe_ids_work_without_process_ids() {
    let first = durability_probe_id::next();
    let second = durability_probe_id::next();
    assert_ne!(first, second);
    for id in [first, second] {
        assert!(id.starts_with("__wa_durability_probe_"));
        assert!(id.ends_with("__"));
    }
}
