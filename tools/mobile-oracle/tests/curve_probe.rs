//! Opt-in synthetic differential check against a separately run original JNI.
//! Reports remain operator-supplied, not authenticated execution attestations.
use serde_json::Value;
use std::{env, fs, io::Read};
use wacore::libsignal::engine::{CryptoProvider, RandomSource, RustCrypto};

struct SyntheticZeroRandom;
impl RandomSource for SyntheticZeroRandom {
    fn fill(&mut self, destination: &mut [u8]) -> wacore::libsignal::engine::Result<()> {
        assert_eq!(destination.len(), 64);
        destination.fill(0);
        Ok(())
    }
}
fn key(hex: &str) -> [u8; 32] {
    hex::decode(hex).unwrap().try_into().unwrap()
}
#[test]
#[ignore = "requires separately executed, pinned original Android Curve25519 JNI observations"]
fn original_curve_jni_matches_synthetic_rust_crypto() {
    let path = env::var("MOBILE_CURVE_PROBE_REPORT").expect("MOBILE_CURVE_PROBE_REPORT");
    let library = env::var("MOBILE_CURVE_EXPECTED_LIBRARY_SHA256")
        .expect("MOBILE_CURVE_EXPECTED_LIBRARY_SHA256");
    let fingerprint =
        env::var("MOBILE_CURVE_EXPECTED_FINGERPRINT").expect("MOBILE_CURVE_EXPECTED_FINGERPRINT");
    assert_eq!(library.len(), 64);
    assert!(!fingerprint.is_empty());
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
    assert_eq!(report["execution"], "android_original_curve_jni");
    assert_eq!(report["application_lifecycle"], "not_run");
    assert_eq!(report["registration"], "not_run");
    assert_eq!(report["stubs"], serde_json::json!([]));
    assert_eq!(report["randomness"], "explicit_synthetic_zero_64_bytes");
    assert_eq!(report["library_sha256"], library);
    assert_eq!(report["build_fingerprint"], fingerprint);
    let cases = report["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4);
    for (i, id) in ["alice-public", "bob-public", "shared", "signature"]
        .iter()
        .enumerate()
    {
        assert_eq!(cases[i]["id"], *id);
    }
    // Public RFC 7748 section 6.1 vectors, not session credentials.
    let alice = key("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
    let bob = key("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
    let alice_public = RustCrypto.public_key(&alice).unwrap();
    let bob_public = RustCrypto.public_key(&bob).unwrap();
    assert_eq!(
        hex::encode(alice_public),
        cases[0]["output_hex"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(bob_public),
        cases[1]["output_hex"].as_str().unwrap()
    );
    assert_eq!(
        hex::encode(RustCrypto.agreement(&alice, &bob_public).unwrap()),
        cases[2]["output_hex"].as_str().unwrap()
    );
    let message = b"mobile oracle synthetic";
    let native_signature: [u8; 64] = hex::decode(cases[3]["output_hex"].as_str().unwrap())
        .unwrap()
        .try_into()
        .unwrap();
    assert!(
        RustCrypto
            .verify(&alice_public, message, &native_signature)
            .is_ok()
    );
    let rust_signature = RustCrypto
        .sign(&alice, message, &mut SyntheticZeroRandom)
        .unwrap();
    assert_eq!(rust_signature, native_signature);
    assert_eq!(report["signature_valid"], true);
    assert_eq!(report["tampered_signature_valid"], false);
    assert!(
        RustCrypto
            .verify(
                &alice_public,
                b"altered synthetic message",
                &native_signature
            )
            .is_err()
    );
    assert_eq!(report["low_order_agreement"]["status"], "threw");
    assert_eq!(
        report["low_order_agreement"]["exception"],
        "java.lang.IllegalArgumentException"
    );
    assert!(RustCrypto.agreement(&alice, &[0; 32]).is_err());
}
