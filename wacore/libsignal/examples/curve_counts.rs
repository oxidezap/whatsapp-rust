//! Deterministic curve profiling without clocks or benchmark-framework hooks.
use std::{
    hint::black_box,
    io::{self, Write},
    process::ExitCode,
};
use wacore_libsignal::{
    core::{PreparedSigningKey, PreparedVerifyingKey, PrivateKey, PublicKey},
    engine::{CryptoProvider, RandomSource, RustCrypto},
};

const ROUNDS: usize = 16;

#[derive(Clone, Copy, Debug)]
enum Kernel {
    Agreement,
    Keygen,
    Sign,
    SignPrepared,
    Verify,
    VerifyPrepared,
}

struct Fixed;
impl RandomSource for Fixed {
    fn fill(&mut self, output: &mut [u8]) -> wacore_libsignal::engine::Result<()> {
        output.fill(42);
        Ok(())
    }
}

struct Case {
    private: [u8; 32],
    public: [u8; 32],
    peer: [u8; 32],
    message: Vec<u8>,
    signature: [u8; 64],
    signer: PreparedSigningKey,
    verifier: PreparedVerifyingKey,
}

fn cases() -> Vec<Case> {
    [0u8, 1, 7, 31, 127, 128, 254, 255]
        .into_iter()
        .flat_map(|seed| {
            [0, 128, 1024].into_iter().map(move |len| {
                let private = [seed; 32];
                let public = RustCrypto
                    .public_key(&private)
                    .expect("synthetic key must be valid");
                let peer = RustCrypto
                    .public_key(&[seed.wrapping_add(17); 32])
                    .expect("synthetic peer must be valid");
                let key = PublicKey::from_djb_public_key_bytes(&public)
                    .expect("synthetic public key must be 32 bytes");
                let verifier = key
                    .prepare_verifying_key()
                    .expect("synthetic point must be valid");
                let signer = PrivateKey::deserialize(&private)
                    .expect("synthetic private key must be 32 bytes")
                    .prepare_signing_key();
                let message = vec![seed; len];
                let signature = RustCrypto
                    .sign(&private, &message, &mut Fixed)
                    .expect("synthetic signing must succeed");
                Case {
                    private,
                    public,
                    peer,
                    message,
                    signature,
                    signer,
                    verifier,
                }
            })
        })
        .collect()
}

fn checksum(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .fold(0u64, |sum, byte| sum.rotate_left(1) ^ u64::from(*byte))
}

// Callgrind toggles collection at this boundary; fixture setup is excluded.
#[inline(never)]
fn measured(kernel: Kernel, cases: &[Case]) -> u64 {
    let mut total = 0u64;
    for _ in 0..ROUNDS {
        for case in cases {
            let value = match kernel {
                Kernel::Agreement => checksum(&black_box(
                    RustCrypto
                        .agreement(black_box(&case.private), black_box(&case.peer))
                        .expect("synthetic agreement must succeed"),
                )),
                Kernel::Keygen => checksum(&black_box(
                    RustCrypto
                        .public_key(black_box(&case.private))
                        .expect("synthetic key generation must succeed"),
                )),
                Kernel::Sign => checksum(&black_box(
                    RustCrypto
                        .sign(
                            black_box(&case.private),
                            black_box(&case.message),
                            &mut Fixed,
                        )
                        .expect("synthetic signing must succeed"),
                )),
                Kernel::SignPrepared => checksum(&black_box(
                    case.signer
                        .calculate_signature_with_random_source(
                            black_box(&case.message),
                            &mut Fixed,
                        )
                        .expect("prepared synthetic signing must succeed"),
                )),
                Kernel::Verify => {
                    RustCrypto
                        .verify(
                            black_box(&case.public),
                            black_box(&case.message),
                            black_box(&case.signature),
                        )
                        .expect("synthetic signature must verify");
                    1
                }
                Kernel::VerifyPrepared => {
                    assert!(
                        case.verifier
                            .verify_signature(black_box(&case.message), black_box(&case.signature))
                    );
                    1
                }
            };
            total = total.wrapping_add(black_box(value));
        }
    }
    black_box(total)
}

fn main() -> ExitCode {
    let kernel = match std::env::args().nth(1).as_deref() {
        Some("agreement") => Kernel::Agreement,
        Some("keygen") => Kernel::Keygen,
        Some("sign") => Kernel::Sign,
        Some("sign_prepared") => Kernel::SignPrepared,
        Some("verify") => Kernel::Verify,
        Some("verify_prepared") => Kernel::VerifyPrepared,
        _ => {
            let _ = writeln!(
                io::stderr().lock(),
                "usage: curve_counts <agreement|keygen|sign|sign_prepared|verify|verify_prepared>"
            );
            return ExitCode::from(2);
        }
    };
    let cases = cases();
    let result = measured(kernel, &cases);
    if writeln!(
        io::stdout().lock(),
        "{kernel:?}: {} operations, checksum {result:016x}",
        ROUNDS * cases.len()
    )
    .is_err()
    {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
