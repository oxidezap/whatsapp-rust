use divan::black_box;
use wacore_libsignal::engine::{CryptoProvider, RandomSource, RustCrypto};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}
struct Fixed;
impl RandomSource for Fixed {
    fn fill(&mut self, out: &mut [u8]) -> wacore_libsignal::engine::Result<()> {
        out.fill(42);
        Ok(())
    }
}
#[divan::bench]
fn agreement(b: divan::Bencher) {
    let public = RustCrypto
        .public_key(&[7; 32])
        .expect("synthetic peer key must produce an X25519 public key");
    b.bench(|| {
        RustCrypto
            .agreement(black_box(&[3; 32]), black_box(&public))
            .expect("synthetic X25519 keys must produce a nonzero shared secret")
    });
}
#[divan::bench]
fn keygen(b: divan::Bencher) {
    b.bench(|| {
        RustCrypto
            .public_key(black_box(&[3; 32]))
            .expect("synthetic private key must produce an X25519 public key")
    });
}
#[divan::bench]
fn sign(b: divan::Bencher) {
    b.bench(|| {
        RustCrypto
            .sign(black_box(&[3; 32]), black_box(&[5; 128]), &mut Fixed)
            .expect("synthetic signing fixture must produce an XEdDSA signature")
    });
}
#[divan::bench]
fn verify(b: divan::Bencher) {
    let public = RustCrypto
        .public_key(&[3; 32])
        .expect("synthetic private key must produce an X25519 public key");
    let signature = RustCrypto
        .sign(&[3; 32], &[5; 128], &mut Fixed)
        .expect("synthetic signing fixture must produce an XEdDSA signature");
    b.bench(|| {
        RustCrypto
            .verify(
                black_box(&public),
                black_box(&[5; 128]),
                black_box(&signature),
            )
            .expect("synthetic XEdDSA signature must verify with its public key")
    });
}

#[divan::bench]
fn sign_prepared(b: divan::Bencher) {
    let private = wacore_libsignal::core::PrivateKey::deserialize(&[3; 32])
        .expect("synthetic private key must have the required 32-byte encoding");
    let prepared = private.prepare_signing_key();
    b.bench(|| {
        prepared
            .calculate_signature_with_random_source(black_box(&[5; 128]), &mut Fixed)
            .expect("prepared synthetic signing key must produce an XEdDSA signature")
    });
}

#[divan::bench]
fn verify_prepared(b: divan::Bencher) {
    let public = RustCrypto
        .public_key(&[3; 32])
        .expect("synthetic private key must produce an X25519 public key");
    let public = wacore_libsignal::core::PublicKey::from_djb_public_key_bytes(&public)
        .expect("synthetic public key must have the required 32-byte encoding");
    let prepared = public
        .prepare_verifying_key()
        .expect("synthetic public key must be a valid point outside the torsion subgroup");
    let signature = RustCrypto
        .sign(&[3; 32], &[5; 128], &mut Fixed)
        .expect("synthetic signing fixture must produce an XEdDSA signature");
    b.bench(|| assert!(prepared.verify_signature(black_box(&[5; 128]), black_box(&signature))));
}
