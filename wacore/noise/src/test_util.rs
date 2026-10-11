//! Test fixtures shared between this crate's unit tests and downstream
//! integration tests. Visible only under `#[cfg(test)]` (this crate) or
//! when the `test-util` feature is enabled.

// Test fixtures may exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use buffa::Message;
use waproto::whatsapp::{self as wa, cert_chain::noise_certificate};

/// Builds a minimal `CertChain` blob whose leaf.key matches `server_static_pub`.
///
/// The validity windows are pinned (`not_before = 1_700_000_000` for both
/// certs, `not_after` slightly under `1_900_000_000`) so callers can exercise
/// `select_pattern`'s clock checks against deterministic boundaries.
///
/// Signatures are zero-filled — the client today does NOT verify the
/// intermediate's Ed25519 signature against `WA_CERT_PUB_KEY`, so the bytes
/// only need to round-trip through protobuf encoding.
pub fn build_cert_chain_bytes(server_static_pub: &[u8; 32]) -> Vec<u8> {
    let intermediate_details = {
        let mut proto = noise_certificate::Details::default();
        proto.serial = Some(1);
        proto.issuer_serial = Some(0);
        proto.key = Some(vec![0xCC; 32]);
        proto.not_before = Some(1_700_000_000);
        proto.not_after = Some(1_900_000_000);
        proto
    };
    let intermediate_details_bytes = intermediate_details.encode_to_vec();

    let leaf_details = {
        let mut proto = noise_certificate::Details::default();
        proto.serial = Some(2);
        proto.issuer_serial = Some(1);
        proto.key = Some(server_static_pub.to_vec());
        proto.not_before = Some(1_700_000_500);
        proto.not_after = Some(1_899_999_500);
        proto
    };
    let leaf_details_bytes = leaf_details.encode_to_vec();

    let chain = {
        let mut proto = wa::CertChain::default();
        proto.leaf = buffa::MessageField::some({
            let mut proto = wa::cert_chain::NoiseCertificate::default();
            proto.details = Some(leaf_details_bytes);
            proto.signature = Some(vec![0u8; 64]);
            proto
        });
        proto.intermediate = buffa::MessageField::some({
            let mut proto = wa::cert_chain::NoiseCertificate::default();
            proto.details = Some(intermediate_details_bytes);
            proto.signature = Some(vec![0u8; 64]);
            proto
        });
        proto
    };
    chain.encode_to_vec()
}
