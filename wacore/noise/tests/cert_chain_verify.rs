//! Integration tests for the `HandshakeUtils::verify_server_cert` path.
//! These load `wacore-noise` as a regular dependency and exercise the real
//! verify: the public helper always verifies strictly, and the default
//! policy is strict while the explicit per-handshake bypass still drives
//! zero-signed fixtures.

#![allow(clippy::disallowed_methods)]

use buffa::Message;
use waproto::whatsapp::{self as wa, cert_chain::noise_certificate};

use wacore_noise::HandshakeUtils;

/// Build a structurally valid `CertChain` blob with zero-filled signatures.
/// Copy of the fixture in `wacore_noise::test_util` so this integration test
/// doesn't need the `test-util` feature.
fn build_zero_signed_chain(server_static_pub: &[u8; 32]) -> Vec<u8> {
    build_chain_with_issuer_serial(server_static_pub, 0)
}

fn build_chain_with_issuer_serial(server_static_pub: &[u8; 32], issuer_serial: u32) -> Vec<u8> {
    let intermediate_details = noise_certificate::Details {
        serial: Some(1),
        issuer_serial: Some(issuer_serial),
        key: Some(vec![0xCC; 32]),
        not_before: Some(1_700_000_000),
        not_after: Some(1_900_000_000),
    };
    let intermediate_details_bytes = intermediate_details.encode_to_vec();

    let leaf_details = noise_certificate::Details {
        serial: Some(2),
        issuer_serial: Some(1),
        key: Some(server_static_pub.to_vec()),
        not_before: Some(1_700_000_500),
        not_after: Some(1_899_999_500),
    };
    let leaf_details_bytes = leaf_details.encode_to_vec();

    let chain = wa::CertChain {
        leaf: buffa::MessageField::some(wa::cert_chain::NoiseCertificate {
            details: Some(leaf_details_bytes),
            signature: Some(vec![0u8; 64]),
        }),
        intermediate: buffa::MessageField::some(wa::cert_chain::NoiseCertificate {
            details: Some(intermediate_details_bytes),
            signature: Some(vec![0u8; 64]),
        }),
    };
    chain.encode_to_vec()
}

// The public helper always verifies strictly: the zero-signed fixture
// must fail XEdDSA verify in every build. Bypass remains available only
// through the explicit-policy handshake constructors, whose outcomes
// carry no trusted chain.
#[test]
fn verify_server_cert_rejects_fixture_with_zero_signed_certs() {
    // The chain is structurally valid (right shape, leaf.key matches the
    // server static) but the intermediate signature is all zeros. The
    // production verify path must reject it because the XEdDSA(WA_CERT_PUB_KEY,
    // intermediate.details) check fails.
    let server_static_pub = [0xAAu8; 32];
    let chain_bytes = build_zero_signed_chain(&server_static_pub);

    let err = HandshakeUtils::verify_server_cert(&chain_bytes, &server_static_pub)
        .expect_err("zero-signed intermediate must fail XEdDSA verify");
    let msg = err.to_string();
    assert!(
        msg.contains("intermediate signature failed XEdDSA verify"),
        "expected an intermediate XEdDSA-verify failure, got: {msg}"
    );
}

// The default policy is strict, yet the public helper must still reject
// the zero-signed chain — it never consults the default, so no
// `VerifiedServerCertChain` exists here that `From` could mark
// `signature_verified`.
#[test]
fn verify_server_cert_default_is_strict_and_helper_stays_strict() {
    use wacore_noise::NoiseCertPolicy;

    assert_eq!(NoiseCertPolicy::default(), NoiseCertPolicy::Strict);
    let server_static_pub = [0xAAu8; 32];
    let chain_bytes = build_zero_signed_chain(&server_static_pub);
    let err = HandshakeUtils::verify_server_cert(&chain_bytes, &server_static_pub)
        .expect_err("public helper must stay strict");
    assert!(
        err.to_string()
            .contains("intermediate signature failed XEdDSA verify"),
        "expected an XEdDSA-verify failure, got: {err}"
    );
}

#[test]
fn verify_server_cert_rejects_when_leaf_key_does_not_match_static() {
    // Structural check fires before XEdDSA: if leaf.key != decrypted static
    // the caller hasn't even received a cert that binds to its session.
    let real_static = [0xAAu8; 32];
    let chain_for_other_static = build_zero_signed_chain(&[0xBBu8; 32]);
    let err = HandshakeUtils::verify_server_cert(&chain_for_other_static, &real_static)
        .expect_err("leaf key != decrypted static must be a CertVerification error");
    assert!(
        err.to_string()
            .contains("Server certificate verification failed")
    );
}

#[test]
fn verify_server_cert_rejects_unexpected_issuer_serial() {
    // The issuer pin is structural and runs under every policy.
    let server_static_pub = [0xAAu8; 32];
    let chain_bytes = build_chain_with_issuer_serial(&server_static_pub, 7);
    let err = HandshakeUtils::verify_server_cert(&chain_bytes, &server_static_pub)
        .expect_err("wrong issuer serial must be a CertVerification error");
    assert!(
        err.to_string()
            .contains("Unexpected intermediate issuer serial"),
        "unexpected error: {err}"
    );
}
