#![cfg(target_arch = "wasm32")]

#[wasm_bindgen_test::wasm_bindgen_test]
fn local_runtime_cancellation() {
    api_consumer::runtime::local_runtime_contract();
}

#[path = "../src/runtime_lifecycle.rs"]
mod lifecycle;
