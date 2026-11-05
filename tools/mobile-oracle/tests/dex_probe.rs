//! Opt-in comparison with a separately run translated-DEX probe. This consumes
//! operator-supplied evidence and does not authenticate JVM or Android execution.
use serde_json::Value;
use std::{env, fs, io::Read};
use wacore::mobile::AndroidTokenMaterial;

#[test]
#[ignore = "requires an explicitly pinned, separately executed synthetic DEX probe report"]
fn translated_dex_synthetic_token_message_matches_rust() {
    let path = env::var("MOBILE_DEX_PROBE_REPORT").expect("MOBILE_DEX_PROBE_REPORT");
    let expected_dex = env::var("MOBILE_DEX_EXPECTED_SHA256").expect("MOBILE_DEX_EXPECTED_SHA256");
    let expected_selector = env::var("MOBILE_DEX_EXPECTED_SELECTOR_SHA256")
        .expect("MOBILE_DEX_EXPECTED_SELECTOR_SHA256");
    assert_eq!(expected_dex.len(), 64);
    assert_eq!(expected_selector.len(), 64);
    let mut bytes = Vec::new();
    fs::File::open(path)
        .unwrap()
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 64 * 1024);
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["execution"], "translated_dex_host_jvm");
    assert_eq!(report["qualified_android_execution"], false);
    assert_eq!(report["android_runtime"], "not_run");
    assert_eq!(report["jni"], "not_run");
    assert_eq!(report["class_initialization"], "excluded");
    assert_eq!(report["stubs"], serde_json::json!([]));
    assert_eq!(report["dex_sha256"], expected_dex);
    assert_eq!(report["selector_sha256"], expected_selector);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    assert_eq!(cases[0]["id"], "synthetic-0");
    assert_eq!(
        cases[0]["output_hex"],
        "b617318655057264e28bc0b6fb378c8ef146be00"
    );
    assert_eq!(cases[1]["id"], "synthetic-1");
    assert_eq!(
        cases[1]["output_hex"],
        "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
    );
    assert_eq!(cases[2]["id"], "synthetic-2");
    assert_eq!(cases[3]["id"], "synthetic-3");
    assert_eq!(report["empty_parts"], "array_bounds_error");

    // This fabricated certificate-like byte string, digest and key are exactly
    // the synthetic-3 message assembled by DexHmacProbe, never APK material.
    let material = AndroidTokenMaterial::from_derived(
        std::array::from_fn(|i| i as u8),
        std::array::from_fn(|i| i as u8),
        vec![vec![0x30, 1, 7]],
    )
    .unwrap();
    let reference = hex::decode(cases[3]["output_hex"].as_str().unwrap()).unwrap();
    assert_eq!(reference.len(), 20);
    assert_eq!(
        material.token_base64("0000000000").unwrap(),
        cases[3]["output_base64"].as_str().unwrap()
    );
}
