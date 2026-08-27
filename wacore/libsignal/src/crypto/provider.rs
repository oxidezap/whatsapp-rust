//! Pluggable Signal crypto provider.
//!
//! Default uses RustCrypto (soft). Override via [`set_crypto_provider`] to
//! delegate the primitives to another backend: AES-256-CBC, AES-256-GCM,
//! HMAC-SHA256, the transport AEAD and the X25519 agreement. Must be set
//! before any crypto call, key agreement included.

use std::sync::OnceLock;

use aes::Aes256;
use aes::cipher::block_padding::Pkcs7;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit};
use bytes::BytesMut;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::crypto::aes_gcm::{Aes256GcmDecryption, Aes256GcmEncryption, Aes256GcmKey};

const AES_BLOCK_SIZE: usize = 16;
const GCM_TAG: usize = 16;

fn rust_hmac_sha256_parts(key: &[u8], inputs: &[&[u8]]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC-SHA256 accepts any key length");
    for input in inputs {
        mac.update(input);
    }
    mac.finalize().into_bytes().into()
}

/// Growable byte buffer usable with in-place AES-GCM operations.
///
/// Implemented for `Vec<u8>` and `bytes::BytesMut`. Both already expose
/// mutable-slice access plus `resize`/`truncate`, so providers can do the
/// actual CTR/XOR work without allocating a scratch buffer.
pub trait GcmInPlaceBuffer {
    fn as_mut_slice(&mut self) -> &mut [u8];
    fn as_slice(&self) -> &[u8];
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn resize(&mut self, new_len: usize, value: u8);
    fn truncate(&mut self, len: usize);
}

impl GcmInPlaceBuffer for Vec<u8> {
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.as_mut_slice()
    }
    #[inline]
    fn as_slice(&self) -> &[u8] {
        self.as_slice()
    }
    #[inline]
    fn resize(&mut self, new_len: usize, value: u8) {
        Vec::resize(self, new_len, value);
    }
    #[inline]
    fn truncate(&mut self, len: usize) {
        Vec::truncate(self, len);
    }
}

impl GcmInPlaceBuffer for BytesMut {
    #[inline]
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self
    }
    #[inline]
    fn as_slice(&self) -> &[u8] {
        self
    }
    #[inline]
    fn resize(&mut self, new_len: usize, value: u8) {
        BytesMut::resize(self, new_len, value);
    }
    #[inline]
    fn truncate(&mut self, len: usize) {
        BytesMut::truncate(self, len);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoProviderError {
    #[error("bad key/iv/nonce size or malformed input")]
    BadInput,
    #[error("authentication tag verification failed")]
    AuthFailed,
    #[error("provider backend reported failure")]
    BackendFailed,
}

/// Connection-lifetime AES-256-GCM handle for the Noise transport: one key,
/// many nonces. Lets a provider precompute key-dependent state once instead
/// of per frame; the default routes every call through the configured
/// provider so custom providers keep observing transport crypto.
pub trait TransportAead: Send + Sync {
    fn encrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError>;

    fn decrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError>;
}

/// Default [`TransportAead`]: per-call dispatch through the provider that
/// created it, with no precomputed state.
struct PerCallTransportAead<P: ?Sized + 'static> {
    provider: &'static P,
    key: [u8; 32],
}

impl<P: SignalCryptoProvider + ?Sized> TransportAead for PerCallTransportAead<P> {
    fn encrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        self.provider
            .aes_256_gcm_encrypt_in_place(&self.key, nonce, aad, buffer)
    }

    fn decrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        self.provider
            .aes_256_gcm_decrypt_in_place(&self.key, nonce, aad, buffer)
    }
}

/// Pluggable crypto primitives used by libsignal and higher-level callers.
///
/// All methods are one-shot (no streaming state): inputs fully known at call
/// time, output appended to `out`. Key/nonce sizes are compile-time arrays so
/// implementations can skip size checks.
pub trait SignalCryptoProvider: Send + Sync + 'static {
    /// AES-256-CBC encrypt with PKCS7 padding. Ciphertext appended to `out`.
    fn aes_256_cbc_encrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError>;

    /// AES-256-CBC decrypt with PKCS7 unpadding. `out` is cleared, then filled.
    fn aes_256_cbc_decrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError>;

    /// AES-256-CBC decrypt with PKCS7 unpadding in the caller-owned buffer.
    ///
    /// The fallback preserves compatibility with external providers by routing
    /// through their existing out-of-place implementation. Providers with a
    /// mutable-buffer primitive should override this to avoid holding the full
    /// ciphertext and plaintext allocations at the same time.
    fn aes_256_cbc_decrypt_in_place(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        buffer: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        let mut out = Vec::with_capacity(buffer.len());
        self.aes_256_cbc_decrypt(key, iv, buffer, &mut out)?;
        *buffer = out;
        Ok(())
    }

    /// AES-256-GCM seal. Appends `ciphertext || tag(16)` to `out`.
    /// Ciphertext length equals plaintext length.
    fn aes_256_gcm_encrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError>;

    /// AES-256-GCM open. `ciphertext_with_tag` must end with the 16-byte tag.
    /// On success appends plaintext to `out`. On tag failure, `out` is left
    /// unchanged and [`CryptoProviderError::AuthFailed`] is returned.
    fn aes_256_gcm_decrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext_with_tag: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError>;

    /// HMAC-SHA256 one-shot. Zero-alloc output.
    fn hmac_sha256(&self, key: &[u8], input: &[u8]) -> [u8; 32];

    /// HMAC-SHA256 over two adjacent logical inputs without concatenating them.
    /// The fallback preserves custom-provider routing; providers with an
    /// incremental primitive should override it to avoid the temporary buffer.
    fn hmac_sha256_two_part(&self, key: &[u8], first: &[u8], second: &[u8]) -> [u8; 32] {
        let mut input = Vec::with_capacity(first.len().saturating_add(second.len()));
        input.extend_from_slice(first);
        input.extend_from_slice(second);
        self.hmac_sha256(key, &input)
    }

    /// X25519 key agreement over the raw 32-byte keys as this crate stores
    /// them; an implementation clamps the private key itself, as X25519
    /// requires (clamping is idempotent, so repeating it is safe). Overriding
    /// this replaces the primitive for every agreement the crate performs.
    ///
    /// A backend that can fail reports it here instead of panicking or
    /// answering with fabricated bytes, which would let a session advance on
    /// key material nobody agreed to. The default is this crate's own
    /// implementation: it cannot fail, and keeps today's result byte for byte.
    fn x25519_agreement(
        &self,
        private_key: &[u8; 32],
        their_public_key: &[u8; 32],
    ) -> Result<[u8; 32], CryptoProviderError> {
        Ok(crate::core::curve::x25519_agreement(
            private_key,
            their_public_key,
        ))
    }

    /// In-place AES-256-GCM seal. On entry `buffer` holds the plaintext; on
    /// return it holds `ciphertext || tag` (length grown by 16).
    ///
    /// Default impl delegates to the allocating [`aes_256_gcm_encrypt`](Self::aes_256_gcm_encrypt) so
    /// existing providers keep working; override for zero-allocation variants.
    fn aes_256_gcm_encrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let mut out = Vec::with_capacity(buffer.len() + GCM_TAG);
        self.aes_256_gcm_encrypt(key, nonce, aad, buffer.as_slice(), &mut out)?;
        buffer.resize(out.len(), 0);
        buffer.as_mut_slice().copy_from_slice(&out);
        Ok(())
    }

    /// In-place AES-256-GCM open. On entry `buffer` holds `ciphertext || tag`;
    /// on success it holds plaintext (length shrunk by 16). On auth failure
    /// the buffer contents are indeterminate and the caller must treat the
    /// session as dead.
    ///
    /// Default impl delegates to the allocating [`aes_256_gcm_decrypt`](Self::aes_256_gcm_decrypt).
    fn aes_256_gcm_decrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let mut out = Vec::with_capacity(buffer.len().saturating_sub(GCM_TAG));
        self.aes_256_gcm_decrypt(key, nonce, aad, buffer.as_slice(), &mut out)?;
        buffer.resize(out.len(), 0);
        buffer.as_mut_slice().copy_from_slice(&out);
        Ok(())
    }

    /// Connection-lifetime transport AEAD for one fixed key. Default keeps
    /// per-call dispatch through this same provider; override to precompute
    /// key-dependent state (key schedule, GHASH subkey) once. The `'static`
    /// receiver ties the handle to the installed provider's lifetime.
    fn transport_aead(
        &'static self,
        key: &[u8; 32],
    ) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
        Ok(Box::new(PerCallTransportAead {
            provider: self,
            key: *key,
        }))
    }
}

static CRYPTO_PROVIDER: OnceLock<Box<dyn SignalCryptoProvider>> = OnceLock::new();

/// Install a custom provider. Must be called before any crypto call. Returns
/// `Err` if a provider was already set (including by `get_or_init` of the
/// default fallback).
pub fn set_crypto_provider(provider: impl SignalCryptoProvider) -> Result<(), &'static str> {
    CRYPTO_PROVIDER
        .set(Box::new(provider))
        .map_err(|_| "crypto provider already set")
}

#[inline]
pub(crate) fn provider() -> &'static dyn SignalCryptoProvider {
    CRYPTO_PROVIDER
        .get_or_init(|| Box::new(RustCryptoProvider))
        .as_ref()
}

/// Pure-Rust fallback implementation. Always used when no custom provider is
/// installed; also used by tests for deterministic behavior.
pub struct RustCryptoProvider;

impl SignalCryptoProvider for RustCryptoProvider {
    fn aes_256_cbc_encrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        let padding = AES_BLOCK_SIZE - (plaintext.len() % AES_BLOCK_SIZE);
        let encrypted_size = plaintext.len() + padding;
        let start = out.len();

        out.resize(start + encrypted_size, 0);
        out[start..start + plaintext.len()].copy_from_slice(plaintext);

        let encryptor = cbc::Encryptor::<Aes256>::new(key.into(), iv.into());
        let written = encryptor
            .encrypt_padded::<Pkcs7>(&mut out[start..], plaintext.len())
            .map_err(|_| CryptoProviderError::BadInput)?
            .len();
        out.truncate(start + written);
        Ok(())
    }

    fn aes_256_cbc_decrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(AES_BLOCK_SIZE) {
            return Err(CryptoProviderError::BadInput);
        }

        out.clear();
        out.reserve(ciphertext.len());
        out.extend_from_slice(ciphertext);

        self.aes_256_cbc_decrypt_in_place(key, iv, out)
    }

    fn aes_256_cbc_decrypt_in_place(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        buffer: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        if buffer.is_empty() || !buffer.len().is_multiple_of(AES_BLOCK_SIZE) {
            return Err(CryptoProviderError::BadInput);
        }

        let decryptor = cbc::Decryptor::<Aes256>::new(key.into(), iv.into());
        let decrypted_len = decryptor
            .decrypt_padded::<Pkcs7>(buffer)
            .map_err(|_| CryptoProviderError::BadInput)?
            .len();
        buffer.truncate(decrypted_len);
        Ok(())
    }

    fn aes_256_gcm_encrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        let mut enc =
            Aes256GcmEncryption::new(key, nonce, aad).map_err(|_| CryptoProviderError::BadInput)?;
        let start = out.len();
        out.extend_from_slice(plaintext);
        enc.encrypt(&mut out[start..]);
        let tag = enc.compute_tag();
        out.extend_from_slice(&tag);
        Ok(())
    }

    fn aes_256_gcm_decrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext_with_tag: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        const TAG: usize = 16;
        if ciphertext_with_tag.len() < TAG {
            return Err(CryptoProviderError::BadInput);
        }
        let (ct, tag) = ciphertext_with_tag.split_at(ciphertext_with_tag.len() - TAG);

        // Decrypt into a scratch, verify tag; only commit to `out` on success
        // so failures leave it untouched.
        let mut scratch = ct.to_vec();
        let mut dec =
            Aes256GcmDecryption::new(key, nonce, aad).map_err(|_| CryptoProviderError::BadInput)?;
        dec.decrypt(&mut scratch);
        dec.verify_tag(tag)
            .map_err(|_| CryptoProviderError::AuthFailed)?;

        out.extend_from_slice(&scratch);
        Ok(())
    }

    fn hmac_sha256(&self, key: &[u8], input: &[u8]) -> [u8; 32] {
        rust_hmac_sha256_parts(key, &[input])
    }

    fn hmac_sha256_two_part(&self, key: &[u8], first: &[u8], second: &[u8]) -> [u8; 32] {
        rust_hmac_sha256_parts(key, &[first, second])
    }

    fn aes_256_gcm_encrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let plaintext_len = buffer.len();
        let mut enc =
            Aes256GcmEncryption::new(key, nonce, aad).map_err(|_| CryptoProviderError::BadInput)?;
        enc.encrypt(buffer.as_mut_slice());
        let tag = enc.compute_tag();
        buffer.resize(plaintext_len + GCM_TAG, 0);
        buffer.as_mut_slice()[plaintext_len..].copy_from_slice(&tag);
        Ok(())
    }

    fn aes_256_gcm_decrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let total = buffer.len();
        if total < GCM_TAG {
            return Err(CryptoProviderError::BadInput);
        }
        let pt_len = total - GCM_TAG;
        let mut tag = [0u8; GCM_TAG];
        tag.copy_from_slice(&buffer.as_slice()[pt_len..]);

        let mut dec =
            Aes256GcmDecryption::new(key, nonce, aad).map_err(|_| CryptoProviderError::BadInput)?;
        dec.decrypt(&mut buffer.as_mut_slice()[..pt_len]);
        dec.verify_tag(&tag)
            .map_err(|_| CryptoProviderError::AuthFailed)?;
        buffer.truncate(pt_len);
        Ok(())
    }

    fn transport_aead(
        &'static self,
        key: &[u8; 32],
    ) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
        Ok(Box::new(
            Aes256GcmKey::new(key).map_err(|_| CryptoProviderError::BadInput)?,
        ))
    }
}

impl TransportAead for Aes256GcmKey {
    fn encrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let plaintext_len = buffer.len();
        let mut enc = Aes256GcmEncryption::new_with_key(self, nonce, aad)
            .map_err(|_| CryptoProviderError::BadInput)?;
        enc.encrypt(buffer.as_mut_slice());
        let tag = enc.compute_tag();
        buffer.resize(plaintext_len + GCM_TAG, 0);
        buffer.as_mut_slice()[plaintext_len..].copy_from_slice(&tag);
        Ok(())
    }

    fn decrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let total = buffer.len();
        if total < GCM_TAG {
            return Err(CryptoProviderError::BadInput);
        }
        let pt_len = total - GCM_TAG;
        let mut tag = [0u8; GCM_TAG];
        tag.copy_from_slice(&buffer.as_slice()[pt_len..]);

        let mut dec = Aes256GcmDecryption::new_with_key(self, nonce, aad)
            .map_err(|_| CryptoProviderError::BadInput)?;
        dec.decrypt(&mut buffer.as_mut_slice()[..pt_len]);
        dec.verify_tag(&tag)
            .map_err(|_| CryptoProviderError::AuthFailed)?;
        buffer.truncate(pt_len);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_provider_cbc_roundtrip() {
        let p = RustCryptoProvider;
        let key = [0x42; 32];
        let iv = [0x11; 16];
        let pt = b"hello cbc provider test";

        let mut ct = Vec::new();
        p.aes_256_cbc_encrypt(&key, &iv, pt, &mut ct).unwrap();

        let mut out = Vec::new();
        p.aes_256_cbc_decrypt(&key, &iv, &ct, &mut out).unwrap();
        assert_eq!(&out[..], pt);
    }

    #[test]
    fn rust_provider_gcm_roundtrip_with_aad() {
        let p = RustCryptoProvider;
        let key = [0x7f; 32];
        let nonce = [0x13; 12];
        let aad = b"extra auth data";
        let pt = b"hello gcm provider test with aad";

        let mut sealed = Vec::new();
        p.aes_256_gcm_encrypt(&key, &nonce, aad, pt, &mut sealed)
            .unwrap();
        assert_eq!(sealed.len(), pt.len() + 16);

        let mut opened = Vec::new();
        p.aes_256_gcm_decrypt(&key, &nonce, aad, &sealed, &mut opened)
            .unwrap();
        assert_eq!(&opened[..], pt);

        // Wrong AAD -> AuthFailed, `out` untouched.
        let mut bad = Vec::new();
        let err = p
            .aes_256_gcm_decrypt(&key, &nonce, b"different", &sealed, &mut bad)
            .unwrap_err();
        assert!(matches!(err, CryptoProviderError::AuthFailed));
        assert!(bad.is_empty());
    }

    #[test]
    fn rust_provider_hmac_matches_direct() {
        let p = RustCryptoProvider;
        let key = b"mac key";
        let data = b"message body";
        let got = p.hmac_sha256(key, data);

        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key).unwrap();
        mac.update(data);
        let expected: [u8; 32] = mac.finalize().into_bytes().into();
        assert_eq!(got, expected);
    }

    #[test]
    fn rust_provider_two_part_hmac_matches_contiguous_input() {
        let provider = RustCryptoProvider;
        let key = b"mac key";
        let first = b"message ";
        let second = b"body";

        assert_eq!(
            provider.hmac_sha256_two_part(key, first, second),
            provider.hmac_sha256(key, b"message body")
        );
    }

    /// A provider written before the agreement joined the trait: it implements
    /// only the required methods and must keep agreeing keys correctly.
    struct SymmetricOnlyProvider;

    impl SignalCryptoProvider for SymmetricOnlyProvider {
        fn aes_256_cbc_encrypt(
            &self,
            key: &[u8; 32],
            iv: &[u8; 16],
            plaintext: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            RustCryptoProvider.aes_256_cbc_encrypt(key, iv, plaintext, out)
        }

        fn aes_256_cbc_decrypt(
            &self,
            key: &[u8; 32],
            iv: &[u8; 16],
            ciphertext: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            RustCryptoProvider.aes_256_cbc_decrypt(key, iv, ciphertext, out)
        }

        fn aes_256_gcm_encrypt(
            &self,
            key: &[u8; 32],
            nonce: &[u8; 12],
            aad: &[u8],
            plaintext: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            RustCryptoProvider.aes_256_gcm_encrypt(key, nonce, aad, plaintext, out)
        }

        fn aes_256_gcm_decrypt(
            &self,
            key: &[u8; 32],
            nonce: &[u8; 12],
            aad: &[u8],
            ciphertext_with_tag: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            RustCryptoProvider.aes_256_gcm_decrypt(key, nonce, aad, ciphertext_with_tag, out)
        }

        fn hmac_sha256(&self, key: &[u8], input: &[u8]) -> [u8; 32] {
            RustCryptoProvider.hmac_sha256(key, input)
        }
    }

    /// RFC 7748 section 6.1, reached through the trait default.
    #[test]
    fn provider_without_agreement_override_keeps_the_pure_path() {
        let alice_private = [
            0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
            0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
            0x1d, 0xb9, 0x2c, 0x2a,
        ];
        let bob_public = [
            0xde, 0x9e, 0xdb, 0x7d, 0x7b, 0x7d, 0xc1, 0xb4, 0xd3, 0x5b, 0x61, 0xc2, 0xec, 0xe4,
            0x35, 0x37, 0x3f, 0x83, 0x43, 0xc8, 0x5b, 0x78, 0x67, 0x4d, 0xad, 0xfc, 0x7e, 0x14,
            0x6f, 0x88, 0x2b, 0x4f,
        ];
        let expected = [
            0x4a, 0x5d, 0x9d, 0x5b, 0xa4, 0xce, 0x2d, 0xe1, 0x72, 0x8e, 0x3b, 0xf4, 0x80, 0x35,
            0x0f, 0x25, 0xe0, 0x7e, 0x21, 0xc9, 0x47, 0xd1, 0x9e, 0x33, 0x76, 0xf0, 0x9b, 0x3c,
            0x1e, 0x16, 0x17, 0x42,
        ];

        assert_eq!(
            SymmetricOnlyProvider
                .x25519_agreement(&alice_private, &bob_public)
                .expect("the default cannot fail"),
            expected
        );
        assert_eq!(
            RustCryptoProvider
                .x25519_agreement(&alice_private, &bob_public)
                .expect("the default cannot fail"),
            expected
        );
    }

    /// A public key of low order agrees to the all-zero secret; the primitive
    /// reports it as bytes rather than failing, and callers must not read that
    /// as a usable secret.
    #[test]
    fn provider_agreement_with_low_order_point_is_all_zero() {
        let agreement = RustCryptoProvider
            .x25519_agreement(&[0x42; 32], &[0u8; 32])
            .expect("the default cannot fail");
        assert_eq!(agreement, [0u8; 32]);
    }

    /// NIST SP 800-38D Test Case 14: all-zero key/nonce, 128-bit plaintext.
    #[test]
    fn rust_provider_gcm_nist_tc14() {
        let p = RustCryptoProvider;
        let key = [0u8; 32];
        let nonce = [0u8; 12];
        let pt = [0u8; 16];

        let expected_ct: [u8; 16] = [
            0xce, 0xa7, 0x40, 0x3d, 0x4d, 0x60, 0x6b, 0x6e, 0x07, 0x4e, 0xc5, 0xd3, 0xba, 0xf3,
            0x9d, 0x18,
        ];
        let expected_tag: [u8; 16] = [
            0xd0, 0xd1, 0xc8, 0xa7, 0x99, 0x99, 0x6b, 0xf0, 0x26, 0x5b, 0x98, 0xb5, 0xd4, 0x8a,
            0xb9, 0x19,
        ];

        let mut sealed = Vec::new();
        p.aes_256_gcm_encrypt(&key, &nonce, b"", &pt, &mut sealed)
            .unwrap();
        assert_eq!(&sealed[..16], &expected_ct);
        assert_eq!(&sealed[16..], &expected_tag);
    }
}
