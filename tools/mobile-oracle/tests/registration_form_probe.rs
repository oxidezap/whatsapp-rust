//! Explicitly pinned original-ART request field/entry formatting observations.
//! This consumes supplied evidence, never a server or live registration result.
use serde_json::Value;
use std::{env, fs, io::Read};
use wacore::mobile::registration_wire::{Base64Padding, RegistrationForm};

#[test]
#[ignore = "requires separately executed original Android registration-form observations"]
fn explicit_form_policies_match_original_android_fields() {
    let path = env::var("MOBILE_FORM_PROBE_REPORT").expect("MOBILE_FORM_PROBE_REPORT");
    let dex = env::var("MOBILE_FORM_EXPECTED_DEX_SHA256").expect("MOBILE_FORM_EXPECTED_DEX_SHA256");
    let image =
        env::var("MOBILE_FORM_EXPECTED_FINGERPRINT").expect("MOBILE_FORM_EXPECTED_FINGERPRINT");
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
    assert_eq!(report["execution"], "android_original_registration_form");
    assert_eq!(report["http"], "not_called");
    assert_eq!(report["registration"], "not_run");
    assert_eq!(report["stubs"], serde_json::json!([]));
    assert_eq!(report["reference_dex_sha256"], dex);
    assert_eq!(report["build_fingerprint"], image);
    assert_eq!(report["formatter_profile"], 46);
    assert_eq!(report["encoded_flag_retained"], true);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 3);
    for (i, id) in ["raw-all-bytes", "base64-all-bytes", "optional-text-uuid"]
        .iter()
        .enumerate()
    {
        assert_eq!(cases[i]["id"], *id);
    }
    let all: Vec<u8> = (0..=255).collect();
    let mut raw = RegistrationForm::new();
    raw.push_rfc3986_bytes("probe_blob", &all).unwrap();
    assert_eq!(
        &*raw.encode(&[]).unwrap(),
        cases[0]["body"].as_str().unwrap()
    );
    let mut b64 = RegistrationForm::new();
    b64.push_base64("probe_blob", &all, Base64Padding::Unpadded)
        .unwrap();
    assert_eq!(
        &*b64.encode(&[]).unwrap(),
        cases[1]["body"].as_str().unwrap()
    );
    let mut mixed = RegistrationForm::new();
    mixed
        .push_form_text("label", "synthetic space +%&/é~*")
        .unwrap();
    mixed.push_wire("yes", "true").unwrap();
    mixed.push_wire("no", "false").unwrap();
    let uuid: Vec<u8> = (0..16).collect();
    mixed
        .push_base64("probe_uuid", &uuid, Base64Padding::Unpadded)
        .unwrap();
    assert_eq!(
        &*mixed.encode(&[]).unwrap(),
        cases[2]["body"].as_str().unwrap()
    );
}
