// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, Payload},
};
use async_trait::async_trait;
use rand::Rng;

/// Seals record values before they reach a [`crate::RecordStore`].
///
/// Implementations other than [`AesGcmSealer`] exist where the key must not
/// enter this process, for example a non-extractable WebCrypto key in a
/// browser. `aad` must be authenticated, not encrypted.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Sealer: Send + Sync {
    async fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, SealError>;
    async fn open(&self, aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>, SealError>;
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SealError {
    /// Unknown format version: written by a newer release, or not sealed.
    #[error("unsupported sealed record format {0}")]
    UnsupportedFormat(u8),
    #[error("sealed record is truncated")]
    Truncated,
    /// Wrong key, tampered bytes, or a record moved to another slot.
    #[error("sealed record failed authentication")]
    Authentication,
    #[error("sealer backend failed: {0}")]
    Backend(String),
}

/// AES-256-GCM with a 256-bit data key held in memory.
///
/// Layout, format 1: `[0x01][12-byte random nonce][ciphertext || 16-byte tag]`.
/// Random 96-bit nonces stay within GCM's safety margin for well over 2^32
/// records per key; rotate the data key before that.
pub struct AesGcmSealer {
    cipher: Aes256Gcm,
}

const FORMAT_V1: u8 = 1;
const NONCE_LEN: usize = 12;
const TAG_LEN: usize = 16;

impl AesGcmSealer {
    pub fn new(data_key: &[u8; 32]) -> Self {
        // A 32-byte slice always matches Aes256Gcm's key size.
        let cipher = Aes256Gcm::new(data_key.into());
        Self { cipher }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Sealer for AesGcmSealer {
    async fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, SealError> {
        let mut nonce = [0u8; NONCE_LEN];
        rand::rng().fill_bytes(&mut nonce);
        let ciphertext = self
            .cipher
            .encrypt(
                &nonce.into(),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| SealError::Backend("aes-gcm encryption failed".into()))?;
        let mut out = Vec::with_capacity(1 + NONCE_LEN + ciphertext.len());
        out.push(FORMAT_V1);
        out.extend_from_slice(&nonce);
        out.extend_from_slice(&ciphertext);
        Ok(out)
    }

    async fn open(&self, aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>, SealError> {
        let (&format, rest) = sealed.split_first().ok_or(SealError::Truncated)?;
        if format != FORMAT_V1 {
            return Err(SealError::UnsupportedFormat(format));
        }
        if rest.len() < NONCE_LEN + TAG_LEN {
            return Err(SealError::Truncated);
        }
        let (nonce, ciphertext) = rest.split_at(NONCE_LEN);
        let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| SealError::Truncated)?;
        self.cipher
            .decrypt(
                &nonce.into(),
                Payload {
                    msg: ciphertext,
                    aad,
                },
            )
            .map_err(|_| SealError::Authentication)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    fn sealer(byte: u8) -> AesGcmSealer {
        AesGcmSealer::new(&[byte; 32])
    }

    #[test]
    fn round_trips_and_uses_fresh_nonces() {
        let s = sealer(7);
        let a = block_on(s.seal(b"slot", b"value")).unwrap();
        let b = block_on(s.seal(b"slot", b"value")).unwrap();
        assert_ne!(a, b, "two seals of the same value must differ");
        assert_eq!(block_on(s.open(b"slot", &a)).unwrap(), b"value");
    }

    #[test]
    fn a_record_moved_to_another_slot_fails() {
        let s = sealer(7);
        let sealed = block_on(s.seal(b"slot-a", b"value")).unwrap();
        assert_eq!(
            block_on(s.open(b"slot-b", &sealed)),
            Err(SealError::Authentication)
        );
    }

    #[test]
    fn a_different_key_fails() {
        let sealed = block_on(sealer(7).seal(b"slot", b"value")).unwrap();
        assert_eq!(
            block_on(sealer(8).open(b"slot", &sealed)),
            Err(SealError::Authentication)
        );
    }

    #[test]
    fn tampering_and_truncation_are_rejected() {
        let s = sealer(7);
        let mut sealed = block_on(s.seal(b"slot", b"value")).unwrap();
        let last = sealed.len() - 1;
        sealed[last] ^= 1;
        assert_eq!(
            block_on(s.open(b"slot", &sealed)),
            Err(SealError::Authentication)
        );
        assert_eq!(block_on(s.open(b"slot", &[])), Err(SealError::Truncated));
        assert_eq!(
            block_on(s.open(b"slot", &[FORMAT_V1, 0, 0])),
            Err(SealError::Truncated)
        );
        assert_eq!(
            block_on(s.open(b"slot", &[9; 40])),
            Err(SealError::UnsupportedFormat(9))
        );
    }
}
