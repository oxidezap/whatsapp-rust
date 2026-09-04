#![cfg(target_family = "wasm")]

use js_sys::Date;
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use whatsapp_rust_sqlite_storage::test_retry_backoff;

wasm_bindgen_test_configure!(run_in_node);

#[wasm_bindgen_test]
async fn retry_backoff_uses_a_real_cancellable_wasm_timer() {
    let start = Date::now();
    test_retry_backoff(10).await;
    assert!(Date::now() - start >= 5.0);

    let cancelled = test_retry_backoff(1_000);
    drop(cancelled);
    test_retry_backoff(1).await;
}
