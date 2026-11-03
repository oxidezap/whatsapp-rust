// SPDX-License-Identifier: MIT
//! Standard RustCrypto operations. Provider objects belong to their caller.
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use aes_gcm::{Aes256Gcm, KeyInit, aead::AeadInOut};
use hmac::{Hmac, Mac};
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
pub trait SignalCryptoProvider: Send + Sync {
    fn transport_aead(&self, key: &[u8; 32])
    -> Result<Box<dyn TransportAead>, CryptoProviderError>;
}
impl SignalCryptoProvider for RustCryptoProvider {
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
    provider: &dyn SignalCryptoProvider,
    key: &[u8; 32],
) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
    provider.transport_aead(key)
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
    pub fn finalize_sha256_array(&self) -> Result<[u8; 32], Error> {
        Ok(self.0.clone().finalize().into())
    }
    pub fn finalize(&self) -> Vec<u8> {
        self.0.clone().finalize().to_vec()
    }
}
#[derive(Clone)]
pub struct CryptographicMac(Hmac<Sha256>);
impl CryptographicMac {
    pub fn new(algorithm: &str, key: &[u8]) -> Result<Self, Error> {
        if algorithm != "HmacSha256" {
            return Err(Error::UnknownAlgorithm("MAC", algorithm.into()));
        }
        Ok(Self(
            <Hmac<Sha256> as KeyInit>::new_from_slice(key).map_err(|_| Error::BadKeyOrIv)?,
        ))
    }
    pub fn update(&mut self, data: &[u8]) {
        self.0.update(data)
    }
    pub fn finalize_sha256_array(&self) -> Result<[u8; 32], Error> {
        Ok(self.0.clone().finalize().into_bytes().into())
    }
    pub fn finalize(&self) -> Vec<u8> {
        self.0.clone().finalize().into_bytes().to_vec()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cbc_append_and_padding_boundaries() {
        for len in [0, 1, 15, 16, 17, 32, 255] {
            let mut out = vec![99];
            let plaintext = vec![7; len];
            aes_256_cbc_encrypt_into(&plaintext, &[3; 32], &[4; 16], &mut out).unwrap();
            assert_eq!(out[0], 99);
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
