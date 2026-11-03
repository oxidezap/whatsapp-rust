// SPDX-License-Identifier: MIT
use crate::engine::{Error, Result};
use subtle::ConstantTimeEq;

pub trait RandomSource {
    fn fill(&mut self, destination: &mut [u8]) -> Result<()>;
}
/// Providers must reject invalid DH inputs, including all-zero shared secrets,
/// and authenticate signatures without data-dependent equality checks.
/// AES operations use PKCS#7 padding. `hkdf` is RFC 5869 with SHA-256.
pub trait CryptoProvider {
    fn public_key(&self, private: &[u8; 32]) -> Result<[u8; 32]>;
    fn agreement(&self, private: &[u8; 32], public: &[u8; 32]) -> Result<[u8; 32]>;
    fn sign(
        &self,
        private: &[u8; 32],
        message: &[u8],
        random: &mut dyn RandomSource,
    ) -> Result<[u8; 64]>;
    fn verify(&self, public: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<()>;
    fn sha256(&self, data: &[u8]) -> [u8; 32];
    fn hmac_sha256(&self, key: &[u8], data: &[u8]) -> [u8; 32];
    fn hkdf_sha256(&self, salt: &[u8], input: &[u8], info: &[u8], output: &mut [u8]) -> Result<()>;
    fn encrypt_cbc(&self, key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> Result<Vec<u8>>;
    fn decrypt_cbc(&self, key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>>;
}
pub(crate) fn authenticate(expected: &[u8], actual: &[u8]) -> Result<()> {
    if bool::from(expected.ct_eq(actual)) {
        Ok(())
    } else {
        Err(Error::Authentication)
    }
}
pub(crate) fn root_step<C: CryptoProvider>(
    crypto: &C,
    root: &[u8; 32],
    private: &[u8; 32],
    public: &[u8; 32],
) -> Result<([u8; 32], [u8; 32])> {
    let shared = crypto.agreement(private, public)?;
    if bool::from(shared.ct_eq(&[0; 32])) {
        return Err(Error::Crypto);
    }
    let mut material = [0; 64];
    crypto.hkdf_sha256(root, &shared, b"WhisperRatchet", &mut material)?;
    let mut next_root = [0; 32];
    let mut chain = [0; 32];
    next_root.copy_from_slice(&material[..32]);
    chain.copy_from_slice(&material[32..]);
    Ok((next_root, chain))
}
