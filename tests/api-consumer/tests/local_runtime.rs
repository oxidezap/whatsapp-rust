#![cfg(target_arch = "wasm32")]

#[wasm_bindgen_test::wasm_bindgen_test]
fn local_runtime_cancellation() {
    api_consumer::runtime::local_runtime_contract();
}

#[path = "../src/runtime_lifecycle.rs"]
mod lifecycle;

#[wasm_bindgen_test::wasm_bindgen_test]
fn observation_tokens_use_browser_entropy_and_preserve_correlation() {
    let token = wacore_binary::jid::observe_token("19990000006");
    assert_ne!(token, 0, "browser entropy must be available in this supported runtime");
    assert_eq!(token, wacore_binary::jid::observe_token("19990000006"));
    assert_ne!(token, wacore_binary::jid::observe_token("19990000007"));
}
