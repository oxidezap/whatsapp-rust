//! Opt-in comparison with separately captured original-DEX ART observations.
//! Consuming a report does not authenticate the Android runner or its execution.
use hmac::{Hmac, Mac};
use serde_json::Value;
use sha1::Sha1;
use std::{env, fs, io::Read};
use wacore::mobile::AndroidTokenMaterial;

#[test]
#[ignore = "requires a separately executed and pinned synthetic Android ART report"]
fn original_dex_android_crypto_matches_synthetic_rust_inputs() {
    let path = env::var("MOBILE_ART_PROBE_REPORT").expect("MOBILE_ART_PROBE_REPORT");
    let dex = env::var("MOBILE_ART_EXPECTED_DEX_SHA256").expect("MOBILE_ART_EXPECTED_DEX_SHA256");
    let image =
        env::var("MOBILE_ART_EXPECTED_FINGERPRINT").expect("MOBILE_ART_EXPECTED_FINGERPRINT");
    assert_eq!(dex.len(), 64);
    assert!(!image.is_empty());
    let mut bytes = Vec::new();
    fs::File::open(path)
        .unwrap()
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(bytes.len() <= 64 * 1024);
    let report: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["driver"], "rust_jni");
    assert_eq!(report["execution"], "android_art_original_dex");
    assert_eq!(report["application_lifecycle"], "not_run");
    assert_eq!(report["registration"], "not_run");
    assert_eq!(report["class_initialization"], "original");
    assert_eq!(report["stubs"], serde_json::json!([]));
    assert_eq!(report["dex_sha256"], dex);
    assert_eq!(report["build_fingerprint"], image);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    assert_eq!(
        cases[0]["output_hex"],
        "b617318655057264e28bc0b6fb378c8ef146be00"
    );
    assert_eq!(
        cases[1]["output_hex"],
        "effcdf6ae5eb2fa2d27416d5f184df9c259a7c79"
    );
    assert_eq!(cases[2]["id"], "synthetic-2");
    let empty_mac = <Hmac<Sha1> as hmac::KeyInit>::new_from_slice(&[0x80, 0xff, 0, 1])
        .unwrap()
        .finalize()
        .into_bytes();
    assert_eq!(cases[2]["output_hex"], hex::encode(empty_mac));
    assert_eq!(cases[3]["id"], "synthetic-3");
    let material = AndroidTokenMaterial::from_derived(
        std::array::from_fn(|i| i as u8),
        std::array::from_fn(|i| i as u8),
        vec![vec![0x30, 1, 7]],
    )
    .unwrap();
    assert_eq!(
        material.token_base64("0000000000").unwrap(),
        cases[3]["output_base64"].as_str().unwrap()
    );
    assert_eq!(report["empty_parts"], "array_bounds_error");

    // Binary logo exercises Android's 8BIT password conversion. None of these
    // bytes come from the APK's actual token material or an account.
    assert_eq!(report["pbkdf"]["id"], "synthetic-binary-password-8bit");
    assert_eq!(report["pbkdf"]["iterations"], 128);
    assert_eq!(report["pbkdf"]["bits"], 512);
    let salt: [u8; 16] = std::array::from_fn(|i| i as u8);
    let derived =
        AndroidTokenMaterial::derive_secret("com.whatsapp", &[0, 255, 128, 1], &salt, 128).unwrap();
    assert_eq!(
        hex::encode(&derived[..]),
        report["pbkdf"]["output_hex"].as_str().unwrap()
    );
}
