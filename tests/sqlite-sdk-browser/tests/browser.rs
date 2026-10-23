#[cfg(target_family = "wasm")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg_attr(not(target_family = "wasm"), tokio::test)]
#[cfg_attr(target_family = "wasm", wasm_bindgen_test::wasm_bindgen_test)]
async fn sdk_js_supplies_sqlite_browser_hooks() {
    assert_eq!(
        sqlite_sdk_browser::probe().await.unwrap(),
        "migrations + provision + exists passed"
    );
}
