//! Integration test for the production `HandshakeUtils::verify_server_cert`
//! path. Lives here (not under `#[cfg(test)] mod tests` inside the lib)
//! because the in-crate cfg(test) gate inside `verify_cert_step` short-circuits
//! the XEdDSA verification so other unit tests can use the zero-signed
//! `build_cert_chain_bytes` fixture. Integration tests load `wacore-noise` as
//! a regular dep without `cfg(test)`, so the real verify actually runs here.

#![cfg(not(feature = "danger-skip-cert-chain-verify"))]
// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use buffa::Message;
use waproto::whatsapp::{self as wa, cert_chain::noise_certificate};

use wacore_noise::HandshakeUtils;

/// Build a structurally valid `CertChain` blob with zero-filled signatures.
/// Copy of the fixture in `wacore_noise::test_util` so this integration test
/// doesn't need to enable the `test-util` feature (Cargo can't enable a lib
/// feature from the lib's own dev-deps).
fn build_zero_signed_chain(server_static_pub: &[u8; 32]) -> Vec<u8> {
    let intermediate_details = noise_certificate::Details {
        serial: Some(1),
        issuer_serial: Some(0),
        key: Some(vec![0xCC; 32]),
        not_before: Some(1_700_000_000),
        not_after: Some(1_900_000_000),
        ..Default::default()
    };
    let intermediate_details_bytes = intermediate_details.encode_to_vec();

    let leaf_details = noise_certificate::Details {
        serial: Some(2),
        issuer_serial: Some(1),
        key: Some(server_static_pub.to_vec()),
        not_before: Some(1_700_000_500),
        not_after: Some(1_899_999_500),
        ..Default::default()
    };
    let leaf_details_bytes = leaf_details.encode_to_vec();

    let chain = wa::CertChain {
        leaf: buffa::MessageField::some(wa::cert_chain::NoiseCertificate {
            details: Some(leaf_details_bytes),
            signature: Some(vec![0u8; 64]),
            ..Default::default()
        }),
        intermediate: buffa::MessageField::some(wa::cert_chain::NoiseCertificate {
            details: Some(intermediate_details_bytes),
            signature: Some(vec![0u8; 64]),
            ..Default::default()
        }),
        ..Default::default()
    };
    chain.encode_to_vec()
}

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
