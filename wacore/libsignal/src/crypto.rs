// SPDX-License-Identifier: MIT
//! Standard RustCrypto operations. Provider objects belong to their caller.
use crate::core::ThreadSafe;
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInOut};
use hmac::{Hmac, HmacReset, Mac};
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CryptoProviderError {
    #[error("invalid cryptographic input")]
    BadInput,
    #[error("authentication failed")]
    AuthFailed,
    #[error("cryptographic backend failed")]
    BackendFailed,
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DecryptionError {
    #[error("invalid key or IV")]
    BadKeyOrIv,
    #[error("invalid ciphertext: {0}")]
    BadCiphertext(&'static str),
    #[error("provider failure: {0}")]
    Provider(#[from] CryptoProviderError),
}
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("unknown {0} algorithm: {1}")]
    UnknownAlgorithm(&'static str, String),
    #[error("invalid key or IV")]
    BadKeyOrIv,
    #[error("cryptographic provider failed: {0}")]
    Provider(#[from] CryptoProviderError),
}
pub trait GcmInPlaceBuffer {
    fn len(&self) -> usize {
        self.as_slice().len()
    }
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn as_mut_slice(&mut self) -> &mut [u8];
    fn as_slice(&self) -> &[u8];
    fn resize(&mut self, new_len: usize, value: u8);
    fn truncate(&mut self, len: usize);
}
impl GcmInPlaceBuffer for Vec<u8> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.as_mut()
    }
    fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }
    fn resize(&mut self, n: usize, v: u8) {
        Vec::resize(self, n, v)
    }
    fn truncate(&mut self, n: usize) {
        Vec::truncate(self, n)
    }
}
impl GcmInPlaceBuffer for bytes::BytesMut {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.as_mut()
    }
    fn as_slice(&self) -> &[u8] {
        self.as_ref()
    }
    fn resize(&mut self, n: usize, v: u8) {
        bytes::BytesMut::resize(self, n, v)
    }
    fn truncate(&mut self, n: usize) {
        bytes::BytesMut::truncate(self, n)
    }
}
pub trait TransportAead: ThreadSafe {
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
struct StandardAead(Aes256Gcm);
impl TransportAead for StandardAead {
    fn encrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let tag = self
            .0
            .encrypt_inout_detached(nonce.into(), aad, buffer.as_mut_slice().into())
            .map_err(|_| CryptoProviderError::BackendFailed)?;
        let len = buffer.as_slice().len();
        buffer.resize(len + 16, 0);
        buffer.as_mut_slice()[len..].copy_from_slice(&tag);
        Ok(())
    }
    fn decrypt_in_place(
        &self,
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let len = buffer
            .as_slice()
            .len()
            .checked_sub(16)
            .ok_or(CryptoProviderError::BadInput)?;
        let mut tag = [0; 16];
        tag.copy_from_slice(&buffer.as_slice()[len..]);
        self.0
            .decrypt_inout_detached(
                nonce.into(),
                aad,
                (&mut buffer.as_mut_slice()[..len]).into(),
                (&tag).into(),
            )
            .map_err(|_| CryptoProviderError::AuthFailed)?;
        buffer.truncate(len);
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct RustCryptoProvider;
/// Caller-owned symmetric crypto port. Implementations may accelerate large
/// payloads while retaining the default Rust operations for smaller messages.
/// Out-of-place operations append to `out`. In-place methods preserve the
/// caller's allocation when supported.
/// Callbacks must validate padding and AEAD tags. Backend failures, including
/// HMAC failures, are returned to the caller rather than replaced by a fallback.
pub trait SymmetricCryptoProvider: ThreadSafe {
    fn aes_256_cbc_encrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        aes_256_cbc_encrypt_into(plaintext, key, iv, out).map_err(|error| match error {
            Error::BadKeyOrIv => CryptoProviderError::BadInput,
            Error::Provider(error) => error,
            Error::UnknownAlgorithm(..) => CryptoProviderError::BackendFailed,
        })
    }
    fn aes_256_cbc_decrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        aes_256_cbc_decrypt_into(ciphertext, key, iv, out).map_err(provider_decryption_error)
    }
    fn aes_256_cbc_decrypt_in_place(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        buffer: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        // Use this provider's decryption callback unless it offers a dedicated
        // in-place implementation. Never silently select another backend.
        if buffer.is_empty() || !buffer.len().is_multiple_of(16) {
            return Err(CryptoProviderError::BadInput);
        }
        let mut plaintext = zeroize::Zeroizing::new(Vec::new());
        self.aes_256_cbc_decrypt(key, iv, buffer, &mut plaintext)?;
        if !buffer
            .len()
            .checked_sub(plaintext.len())
            .is_some_and(|n| (1..=16).contains(&n))
        {
            return Err(CryptoProviderError::BackendFailed);
        }
        buffer.clear();
        buffer.extend_from_slice(&plaintext);
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
        aes_256_gcm_encrypt(key, nonce, aad, plaintext, out)
    }
    fn aes_256_gcm_decrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        aes_256_gcm_decrypt(key, nonce, aad, ciphertext, out)
    }
    fn aes_256_gcm_encrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let mut output = Vec::new();
        self.aes_256_gcm_encrypt(key, nonce, aad, buffer.as_slice(), &mut output)?;
        let expected = buffer
            .len()
            .checked_add(16)
            .ok_or(CryptoProviderError::BadInput)?;
        if output.len() != expected {
            return Err(CryptoProviderError::BackendFailed);
        }
        buffer.resize(expected, 0);
        buffer.as_mut_slice().copy_from_slice(&output);
        Ok(())
    }
    fn aes_256_gcm_decrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        let expected = buffer
            .len()
            .checked_sub(16)
            .ok_or(CryptoProviderError::BadInput)?;
        let mut output = zeroize::Zeroizing::new(Vec::new());
        self.aes_256_gcm_decrypt(key, nonce, aad, buffer.as_slice(), &mut output)?;
        if output.len() != expected {
            return Err(CryptoProviderError::BackendFailed);
        }
        buffer.as_mut_slice()[..expected].copy_from_slice(&output);
        buffer.truncate(expected);
        Ok(())
    }
    /// Authenticate borrowed chunks in order without concatenating the payload.
    fn hmac_sha256(&self, key: &[u8], parts: &[&[u8]]) -> Result<[u8; 32], CryptoProviderError> {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
            .map_err(|_| CryptoProviderError::BadInput)?;
        for part in parts {
            mac.update(part);
        }
        Ok(mac.finalize().into_bytes().into())
    }
}
/// Creates an owned connection-lifetime AEAD with a prepared key schedule.
/// Transport selection is independent of one-shot media and Signal primitives.
pub trait TransportAeadFactory: ThreadSafe {
    fn transport_aead(&self, key: &[u8; 32])
    -> Result<Box<dyn TransportAead>, CryptoProviderError>;
}
fn provider_decryption_error(error: DecryptionError) -> CryptoProviderError {
    match error {
        DecryptionError::BadKeyOrIv => CryptoProviderError::BadInput,
        DecryptionError::BadCiphertext(_) => CryptoProviderError::AuthFailed,
        DecryptionError::Provider(error) => error,
    }
}
impl SymmetricCryptoProvider for RustCryptoProvider {
    fn aes_256_cbc_decrypt_in_place(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        buffer: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        aes_256_cbc_decrypt_in_place(buffer, key, iv).map_err(provider_decryption_error)
    }
    fn aes_256_gcm_encrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        StandardAead(Aes256Gcm::new(key.into())).encrypt_in_place(nonce, aad, buffer)
    }
    fn aes_256_gcm_decrypt_in_place(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        StandardAead(Aes256Gcm::new(key.into())).decrypt_in_place(nonce, aad, buffer)
    }
}
impl TransportAeadFactory for RustCryptoProvider {
    fn transport_aead(
        &self,
        key: &[u8; 32],
    ) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
        Ok(Box::new(StandardAead(Aes256Gcm::new(key.into()))))
    }
}

pub fn transport_aead(key: &[u8; 32]) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
    RustCryptoProvider.transport_aead(key)
}
pub fn transport_aead_with_provider(
    provider: &dyn TransportAeadFactory,
    key: &[u8; 32],
) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
    provider.transport_aead(key)
}
/// Inject a caller-owned symmetric backend into a sans-I/O engine. Curve
/// operations and HKDF retain the selected engine provider's implementation.
pub struct ProviderCrypto<'a, C = crate::engine::RustCrypto> {
    crypto: C,
    provider: &'a dyn SymmetricCryptoProvider,
}
impl<'a> ProviderCrypto<'a> {
    pub fn new(provider: &'a dyn SymmetricCryptoProvider) -> Self {
        Self::with_crypto(crate::engine::RustCrypto, provider)
    }
}
impl<'a, C> ProviderCrypto<'a, C> {
    pub(crate) fn with_crypto(crypto: C, provider: &'a dyn SymmetricCryptoProvider) -> Self {
        Self { crypto, provider }
    }
}
fn engine_provider_error(error: CryptoProviderError) -> crate::engine::Error {
    match error {
        CryptoProviderError::AuthFailed => crate::engine::Error::Authentication,
        CryptoProviderError::BadInput | CryptoProviderError::BackendFailed => {
            crate::engine::Error::Crypto
        }
    }
}
impl<C: crate::engine::CryptoProvider> crate::engine::CryptoProvider for ProviderCrypto<'_, C> {
    fn public_key(&self, private: &[u8; 32]) -> crate::engine::Result<[u8; 32]> {
        self.crypto.public_key(private)
    }
    fn agreement(&self, private: &[u8; 32], public: &[u8; 32]) -> crate::engine::Result<[u8; 32]> {
        self.crypto.agreement(private, public)
    }
    fn sign(
        &self,
        private: &[u8; 32],
        message: &[u8],
        random: &mut dyn crate::engine::RandomSource,
    ) -> crate::engine::Result<[u8; 64]> {
        self.crypto.sign(private, message, random)
    }
    fn verify(
        &self,
        public: &[u8; 32],
        message: &[u8],
        signature: &[u8; 64],
    ) -> crate::engine::Result<()> {
        self.crypto.verify(public, message, signature)
    }
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        self.crypto.sha256(data)
    }
    fn hmac_sha256(&self, key: &[u8], parts: &[&[u8]]) -> crate::engine::Result<[u8; 32]> {
        self.provider
            .hmac_sha256(key, parts)
            .map_err(|_| crate::engine::Error::Crypto)
    }
    fn hkdf_sha256(
        &self,
        salt: &[u8],
        input: &[u8],
        info: &[u8],
        output: &mut [u8],
    ) -> crate::engine::Result<()> {
        self.crypto.hkdf_sha256(salt, input, info, output)
    }
    fn encrypt_cbc(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
    ) -> crate::engine::Result<Vec<u8>> {
        let expected = plaintext
            .len()
            .checked_div(16)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_mul(16))
            .ok_or(crate::engine::Error::Crypto)?;
        let mut out = zeroize::Zeroizing::new(Vec::new());
        self.provider
            .aes_256_cbc_encrypt(key, iv, plaintext, &mut out)
            .map_err(engine_provider_error)?;
        if out.len() != expected {
            return Err(crate::engine::Error::Crypto);
        }
        Ok(std::mem::take(&mut *out))
    }
    fn decrypt_cbc(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
    ) -> crate::engine::Result<Vec<u8>> {
        if ciphertext.is_empty() || !ciphertext.len().is_multiple_of(16) {
            return Err(crate::engine::Error::Authentication);
        }
        let mut out = zeroize::Zeroizing::new(Vec::new());
        self.provider
            .aes_256_cbc_decrypt(key, iv, ciphertext, &mut out)
            .map_err(engine_provider_error)?;
        if !ciphertext
            .len()
            .checked_sub(out.len())
            .is_some_and(|n| (1..=16).contains(&n))
        {
            return Err(crate::engine::Error::Crypto);
        }
        Ok(std::mem::take(&mut *out))
    }
}
struct Tail<'a> {
    out: &'a mut Vec<u8>,
    start: usize,
}
impl GcmInPlaceBuffer for Tail<'_> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.out[self.start..]
    }
    fn as_slice(&self) -> &[u8] {
        &self.out[self.start..]
    }
    fn resize(&mut self, n: usize, v: u8) {
        self.out.resize(self.start + n, v)
    }
    fn truncate(&mut self, n: usize) {
        self.out.truncate(self.start + n)
    }
}
pub fn aes_256_gcm_encrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    plaintext: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), CryptoProviderError> {
    let key: &[u8; 32] = key.try_into().map_err(|_| CryptoProviderError::BadInput)?;
    let nonce = nonce
        .try_into()
        .map_err(|_| CryptoProviderError::BadInput)?;
    let aead = StandardAead(Aes256Gcm::new(key.into()));
    let start = out.len();
    out.reserve(plaintext.len() + 16);
    out.extend_from_slice(plaintext);
    let result = aead.encrypt_in_place(nonce, aad, &mut Tail { out, start });
    if result.is_err() {
        out.truncate(start)
    }
    result
}
pub fn aes_256_gcm_decrypt(
    key: &[u8],
    nonce: &[u8],
    aad: &[u8],
    ciphertext: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), CryptoProviderError> {
    let key: &[u8; 32] = key.try_into().map_err(|_| CryptoProviderError::BadInput)?;
    let nonce: &[u8; 12] = nonce
        .try_into()
        .map_err(|_| CryptoProviderError::BadInput)?;
    let body_len = ciphertext
        .len()
        .checked_sub(16)
        .ok_or(CryptoProviderError::BadInput)?;
    let (body, tag) = ciphertext.split_at(body_len);
    let tag: &[u8; 16] = tag.try_into().map_err(|_| CryptoProviderError::BadInput)?;
    let aead = Aes256Gcm::new(key.into());
    let start = out.len();
    out.extend_from_slice(body);
    let result = aead
        .decrypt_inout_detached(nonce.into(), aad, (&mut out[start..]).into(), tag.into())
        .map_err(|_| CryptoProviderError::AuthFailed);
    if result.is_err() {
        use zeroize::Zeroize;
        out[start..].zeroize();
        out.truncate(start)
    }
    result
}
pub fn aes_256_cbc_encrypt_into(
    plaintext: &[u8],
    key: &[u8],
    iv: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), Error> {
    let cipher =
        cbc::Encryptor::<aes::Aes256>::new_from_slices(key, iv).map_err(|_| Error::BadKeyOrIv)?;
    let start = out.len();
    let padded = plaintext
        .len()
        .checked_div(16)
        .and_then(|n| n.checked_add(1))
        .and_then(|n| n.checked_mul(16))
        .ok_or(Error::BadKeyOrIv)?;
    let end = start.checked_add(padded).ok_or(Error::BadKeyOrIv)?;
    out.extend_from_slice(plaintext);
    out.resize(end, 0);
    if cipher
        .encrypt_padded::<Pkcs7>(&mut out[start..], plaintext.len())
        .is_err()
    {
        out.truncate(start);
        return Err(Error::Provider(CryptoProviderError::BackendFailed));
    }
    Ok(())
}
pub fn aes_256_cbc_decrypt_into(
    ciphertext: &[u8],
    key: &[u8],
    iv: &[u8],
    out: &mut Vec<u8>,
) -> Result<(), DecryptionError> {
    let cipher = cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)
        .map_err(|_| DecryptionError::BadKeyOrIv)?;
    let start = out.len();
    out.extend_from_slice(ciphertext);
    let result = cipher
        .decrypt_padded::<Pkcs7>(&mut out[start..])
        .map(|plain| plain.len());
    match result {
        Ok(len) => {
            out.truncate(start + len);
            Ok(())
        }
        Err(_) => {
            use zeroize::Zeroize;
            out[start..].zeroize();
            out.truncate(start);
            Err(DecryptionError::BadCiphertext("invalid PKCS#7 padding"))
        }
    }
}
pub fn aes_256_cbc_decrypt_in_place(
    ciphertext: &mut Vec<u8>,
    key: &[u8],
    iv: &[u8],
) -> Result<(), DecryptionError> {
    let len = cbc::Decryptor::<aes::Aes256>::new_from_slices(key, iv)
        .map_err(|_| DecryptionError::BadKeyOrIv)?
        .decrypt_padded::<Pkcs7>(ciphertext)
        .map_err(|_| DecryptionError::BadCiphertext("invalid PKCS#7 padding"))?
        .len();
    ciphertext.truncate(len);
    Ok(())
}
pub fn aes_256_ctr_apply(key: &[u8; 32], iv: &[u8; 16], buffer: &mut [u8]) {
    use aes::cipher::StreamCipher;
    ctr::Ctr128BE::<aes::Aes256>::new(key.into(), iv.into()).apply_keystream(buffer)
}
pub fn hmac_sha256_two_part(key: &[u8], first: &[u8], second: &[u8]) -> [u8; 32] {
    let mut mac =
        <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(first);
    mac.update(second);
    mac.finalize().into_bytes().into()
}
/// Streaming SHA-256. Each finalizer resets the hash for the next message.
#[derive(Clone)]
pub struct CryptographicHash(Sha256);
impl CryptographicHash {
    pub fn new(algorithm: &str) -> Result<Self, Error> {
        if algorithm != "SHA-256" {
            return Err(Error::UnknownAlgorithm("hash", algorithm.into()));
        }
        Ok(Self(Sha256::new()))
    }
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data)
    }
    pub fn finalize_sha256_array(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.0.finalize_reset().into())
    }
    pub fn finalize(&mut self) -> Vec<u8> {
        self.0.finalize_reset().to_vec()
    }
}
/// Streaming HMAC-SHA256. Finalization resets the message while retaining the key.
#[derive(Clone)]
pub struct CryptographicMac(HmacReset<Sha256>);
impl CryptographicMac {
    pub fn new(algorithm: &str, key: &[u8]) -> Result<Self, Error> {
        if algorithm != "HmacSha256" {
            return Err(Error::UnknownAlgorithm("MAC", algorithm.into()));
        }
        Ok(Self(
            <HmacReset<Sha256> as KeyInit>::new_from_slice(key).map_err(|_| Error::BadKeyOrIv)?,
        ))
    }
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data)
    }
    pub fn finalize_sha256_array(&mut self) -> Result<[u8; 32], Error> {
        Ok(self.0.finalize_reset().into_bytes().into())
    }
    pub fn finalize(&mut self) -> Vec<u8> {
        self.0.finalize_reset().into_bytes().to_vec()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn paired_chain_derivation_keeps_selected_callbacks_and_failure_rollback() {
        struct FailAt {
            call: AtomicUsize,
            fail_at: usize,
        }
        impl SymmetricCryptoProvider for FailAt {
            fn hmac_sha256(
                &self,
                key: &[u8],
                parts: &[&[u8]],
            ) -> Result<[u8; 32], CryptoProviderError> {
                let call = self.call.fetch_add(1, Ordering::SeqCst) + 1;
                assert_eq!(parts, &[&[call as u8][..]]);
                if call == self.fail_at {
                    Err(CryptoProviderError::BackendFailed)
                } else {
                    RustCryptoProvider.hmac_sha256(key, parts)
                }
            }
        }
        for fail_at in [1, 2] {
            let provider = FailAt {
                call: AtomicUsize::new(0),
                fail_at,
            };
            let crypto = ProviderCrypto::new(&provider);
            let mut chain = crate::engine::Chain::new([1; 32], Some([2; 32]), [3; 32]);
            let before = chain.clone();
            assert!(chain.step(&crypto).is_err());
            assert!(chain == before);
            assert_eq!(provider.call.load(Ordering::SeqCst), fail_at);
        }
    }

    #[derive(Default)]
    struct ObservedProvider {
        cbc: AtomicUsize,
        gcm: AtomicUsize,
        hmac: AtomicUsize,
    }
    impl SymmetricCryptoProvider for ObservedProvider {
        fn aes_256_cbc_encrypt(
            &self,
            key: &[u8; 32],
            iv: &[u8; 16],
            plaintext: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            self.cbc.fetch_add(1, Ordering::Relaxed);
            RustCryptoProvider.aes_256_cbc_encrypt(key, iv, plaintext, out)
        }
        fn aes_256_cbc_decrypt(
            &self,
            key: &[u8; 32],
            iv: &[u8; 16],
            ciphertext: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            self.cbc.fetch_add(1, Ordering::Relaxed);
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
            self.gcm.fetch_add(1, Ordering::Relaxed);
            RustCryptoProvider.aes_256_gcm_encrypt(key, nonce, aad, plaintext, out)
        }
        fn aes_256_gcm_decrypt(
            &self,
            key: &[u8; 32],
            nonce: &[u8; 12],
            aad: &[u8],
            ciphertext: &[u8],
            out: &mut Vec<u8>,
        ) -> Result<(), CryptoProviderError> {
            self.gcm.fetch_add(1, Ordering::Relaxed);
            RustCryptoProvider.aes_256_gcm_decrypt(key, nonce, aad, ciphertext, out)
        }
        fn hmac_sha256(
            &self,
            key: &[u8],
            parts: &[&[u8]],
        ) -> Result<[u8; 32], CryptoProviderError> {
            self.hmac.fetch_add(1, Ordering::Relaxed);
            RustCryptoProvider.hmac_sha256(key, parts)
        }
    }
    #[test]
    fn caller_owned_providers_execute_callbacks_and_remain_isolated() {
        use crate::engine::CryptoProvider;
        let first = ObservedProvider::default();
        let second = ObservedProvider::default();
        let crypto = ProviderCrypto::new(&first);
        for size in [0, 1, 15, 16, 4096] {
            let plaintext = vec![7; size];
            let ciphertext = crypto.encrypt_cbc(&[3; 32], &[4; 16], &plaintext).unwrap();
            assert_eq!(
                crate::engine::RustCrypto
                    .decrypt_cbc(&[3; 32], &[4; 16], &ciphertext)
                    .unwrap(),
                plaintext
            );
            let mut inplace = ciphertext.clone();
            first
                .aes_256_cbc_decrypt_in_place(&[3; 32], &[4; 16], &mut inplace)
                .unwrap();
            assert_eq!(inplace, plaintext);
            assert_eq!(
                crypto.decrypt_cbc(&[3; 32], &[4; 16], &ciphertext).unwrap(),
                plaintext
            );
        }
        assert_eq!(first.cbc.load(Ordering::Relaxed), 15);
        assert_eq!(second.cbc.load(Ordering::Relaxed), 0);
        assert_eq!(
            first.hmac_sha256(b"key", &[b"first", b"second"]).unwrap(),
            hmac_sha256_two_part(b"key", b"first", b"second")
        );
        assert_eq!(first.hmac.load(Ordering::Relaxed), 1);
        assert_eq!(second.hmac.load(Ordering::Relaxed), 0);
        let mut buffer = vec![9; 4096];
        let original = buffer.clone();
        first
            .aes_256_gcm_encrypt_in_place(&[3; 32], &[4; 12], b"aad", &mut buffer)
            .unwrap();
        let ciphertext = buffer.clone();
        assert!(
            first
                .aes_256_gcm_decrypt_in_place(&[3; 32], &[4; 12], b"wrong", &mut buffer)
                .is_err()
        );
        assert_eq!(buffer, ciphertext);
        first
            .aes_256_gcm_decrypt_in_place(&[3; 32], &[4; 12], b"aad", &mut buffer)
            .unwrap();
        assert_eq!(buffer, original);
        assert_eq!(first.gcm.load(Ordering::Relaxed), 3);
        assert_eq!(second.gcm.load(Ordering::Relaxed), 0);
    }
    #[test]
    fn malformed_provider_output_is_rejected_before_publication() {
        use crate::engine::CryptoProvider;
        struct Invalid;
        impl SymmetricCryptoProvider for Invalid {
            fn aes_256_cbc_encrypt(
                &self,
                _: &[u8; 32],
                _: &[u8; 16],
                _: &[u8],
                out: &mut Vec<u8>,
            ) -> Result<(), CryptoProviderError> {
                out.extend_from_slice(&[7; 15]);
                Ok(())
            }
            fn aes_256_cbc_decrypt(
                &self,
                _: &[u8; 32],
                _: &[u8; 16],
                _: &[u8],
                out: &mut Vec<u8>,
            ) -> Result<(), CryptoProviderError> {
                out.extend_from_slice(&[7; 17]);
                Ok(())
            }
            fn aes_256_gcm_encrypt(
                &self,
                _: &[u8; 32],
                _: &[u8; 12],
                _: &[u8],
                _: &[u8],
                _: &mut Vec<u8>,
            ) -> Result<(), CryptoProviderError> {
                Ok(())
            }
            fn aes_256_gcm_decrypt(
                &self,
                _: &[u8; 32],
                _: &[u8; 12],
                _: &[u8],
                _: &[u8],
                out: &mut Vec<u8>,
            ) -> Result<(), CryptoProviderError> {
                out.push(7);
                Ok(())
            }
        }
        let crypto = ProviderCrypto::new(&Invalid);
        assert!(crypto.encrypt_cbc(&[3; 32], &[4; 16], b"test").is_err());
        assert!(crypto.decrypt_cbc(&[3; 32], &[4; 16], &[0; 16]).is_err());
        let mut buffer = vec![1; 16];
        let original = buffer.clone();
        assert!(
            Invalid
                .aes_256_cbc_decrypt_in_place(&[3; 32], &[4; 16], &mut buffer)
                .is_err()
        );
        assert_eq!(buffer, original);
        assert!(
            Invalid
                .aes_256_gcm_encrypt_in_place(&[3; 32], &[4; 12], b"", &mut buffer)
                .is_err()
        );
        assert_eq!(buffer, original);
        assert!(
            Invalid
                .aes_256_gcm_decrypt_in_place(&[3; 32], &[4; 12], b"", &mut buffer)
                .is_err()
        );
        assert_eq!(buffer, original);
    }
    #[test]
    fn finalization_resets_hash_and_keyed_mac_for_reuse() {
        let key = b"synthetic key";
        let mut mac = CryptographicMac::new("HmacSha256", key).unwrap();
        mac.update(b"first");
        assert_eq!(
            mac.finalize_sha256_array().unwrap(),
            hmac_sha256_two_part(key, b"first", b"")
        );
        mac.update(b"second");
        assert_eq!(
            mac.finalize(),
            hmac_sha256_two_part(key, b"second", b"").to_vec()
        );
        assert_eq!(
            mac.finalize_sha256_array().unwrap(),
            hmac_sha256_two_part(key, b"", b"")
        );
        assert_eq!(mac.finalize(), hmac_sha256_two_part(key, b"", b"").to_vec());
        let mut hash = CryptographicHash::new("SHA-256").unwrap();
        hash.update(b"first");
        assert_eq!(
            hash.finalize_sha256_array().unwrap(),
            <[u8; 32]>::from(Sha256::digest(b"first"))
        );
        hash.update(b"second");
        assert_eq!(hash.finalize(), Sha256::digest(b"second").to_vec());
        assert_eq!(
            hash.finalize_sha256_array().unwrap(),
            <[u8; 32]>::from(Sha256::digest(b""))
        );
        assert_eq!(hash.finalize(), Sha256::digest(b"").to_vec());
    }

    #[test]
    fn cbc_append_and_padding_boundaries() {
        for len in [0, 1, 15, 16, 17, 32, 255] {
            let mut out = vec![99];
            let plaintext = vec![7; len];
            aes_256_cbc_encrypt_into(&plaintext, &[3; 32], &[4; 16], &mut out).unwrap();
            assert_eq!(out[0], 99);
            let mut appended = vec![88];
            RustCryptoProvider
                .aes_256_cbc_decrypt(&[3; 32], &[4; 16], &out[1..], &mut appended)
                .unwrap();
            assert_eq!(appended[0], 88);
            assert_eq!(&appended[1..], plaintext);
            let mut cipher = out[1..].to_vec();
            aes_256_cbc_decrypt_in_place(&mut cipher, &[3; 32], &[4; 16]).unwrap();
            assert_eq!(cipher, plaintext);
        }
        assert!(aes_256_cbc_decrypt_in_place(&mut vec![0; 15], &[3; 32], &[4; 16]).is_err());
    }
    #[test]
    fn nist_gcm_and_append_contract() {
        let mut out = vec![99];
        aes_256_gcm_encrypt(&[0; 32], &[0; 12], b"", &[0; 16], &mut out).unwrap();
        let expected = [
            0xce, 0xa7, 0x40, 0x3d, 0x4d, 0x60, 0x6b, 0x6e, 0x07, 0x4e, 0xc5, 0xd3, 0xba, 0xf3,
            0x9d, 0x18, 0xd0, 0xd1, 0xc8, 0xa7, 0x99, 0x99, 0x6b, 0xf0, 0x26, 0x5b, 0x98, 0xb5,
            0xd4, 0x8a, 0xb9, 0x19,
        ];
        assert_eq!(&out[1..], expected);
        let mut plain = vec![88];
        aes_256_gcm_decrypt(&[0; 32], &[0; 12], b"", &out[1..], &mut plain).unwrap();
        assert_eq!(&plain[1..], &[0; 16]);
        out[1] ^= 1;
        assert!(aes_256_gcm_decrypt(&[0; 32], &[0; 12], b"", &out[1..], &mut plain).is_err());
        assert_eq!(plain.len(), 17);
    }
}
