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
    let intermediate_details = noise_certificate::Details {
        serial: Some(1),
        issuer_serial: Some(0),
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
