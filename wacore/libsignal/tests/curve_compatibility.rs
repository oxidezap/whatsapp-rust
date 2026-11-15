// Transcript recorded before replacement against crates.io curve25519-dalek 5.0.0.
use sha2::{Digest, Sha512};
use wacore_libsignal::{
    core::{PreparedVerifyingKey, PrivateKey, PublicKey},
    engine::{CryptoProvider, RandomSource, RustCrypto},
};

struct Fixed(u8);
impl RandomSource for Fixed {
    fn fill(&mut self, output: &mut [u8]) -> wacore_libsignal::engine::Result<()> {
        for (i, byte) in output.iter_mut().enumerate() {
            *byte = self.0.wrapping_add(i as u8);
        }
        Ok(())
    }
}

#[test]
fn deterministic_curve_transcript() {
    let mut hash = Sha512::new();
    for seed in 0u16..256 {
        let private = std::array::from_fn(|i| (seed as u8).wrapping_mul(37).wrapping_add(i as u8));
        let public = RustCrypto.public_key(&private).unwrap();
        hash.update(public);
        let peer = RustCrypto.public_key(&[seed as u8; 32]).unwrap();
        hash.update(RustCrypto.agreement(&private, &peer).unwrap());
        let signing = PrivateKey::deserialize(&private)
            .unwrap()
            .prepare_signing_key();
        let verifying =
            PreparedVerifyingKey::new(&PublicKey::from_djb_public_key_bytes(&public).unwrap())
                .unwrap();
        for len in [0, 1, 31, 32, 33, 128, 1024] {
            let message = vec![seed as u8; len];
            let signature = RustCrypto
                .sign(&private, &message, &mut Fixed(seed as u8))
                .unwrap();
            hash.update(signature);
            assert_eq!(
                signature,
                signing
                    .calculate_signature_with_random_source(&message, &mut Fixed(seed as u8))
                    .unwrap()
            );
            assert!(verifying.verify_signature(&message, &signature));
            assert!(RustCrypto.verify(&public, &message, &signature).is_ok());
            let mut changed = signature;
            changed[seed as usize % 64] ^= 1;
            hash.update([u8::from(
                RustCrypto.verify(&public, &message, &changed).is_ok(),
            )]);
            hash.update([u8::from(verifying.verify_signature(&message, &changed))]);
        }
    }
    let digest = hex::encode(hash.finalize());
    assert_eq!(
        digest,
        "8acc89152510d44320ae3597919fd35f7037090e18f56d4a6fc71a7983f3a5ea2f130636a678c22eaa6d715d24c336745a2357b1126360f9d49b06fdd3d0e811"
    );
}
