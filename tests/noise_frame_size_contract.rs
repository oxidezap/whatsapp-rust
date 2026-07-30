//! The sender writes a frame's length prefix from `plaintext.len() + 16` before
//! the ciphertext exists, which holds only because `TransportAead` is
//! AES-256-GCM by contract. This pins what happens when an installed provider
//! breaks that contract: the frame must be refused, never written.
//!
//! Its own integration binary because `set_crypto_provider` writes a
//! process-wide `OnceLock`. Installing a deliberately broken backend inside the
//! unit-test binary would poison every other test in it.

use std::sync::Arc;
use std::sync::Mutex;

use wacore::libsignal::crypto::{
    CryptoProviderError, GcmInPlaceBuffer, RustCryptoProvider, SignalCryptoProvider, TransportAead,
    set_crypto_provider,
};

/// Grows the buffer by one byte less than the GCM tag, so the frame body ends
/// up shorter than the prefix already written for it.
struct ShortTagAead;

impl TransportAead for ShortTagAead {
    fn encrypt_in_place(
        &self,
        _nonce: &[u8; 12],
        _aad: &[u8],
        buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        // 15 bytes instead of 16: reports success, leaves the wire inconsistent.
        buffer.resize(buffer.as_slice().len() + 15, 0);
        Ok(())
    }

    fn decrypt_in_place(
        &self,
        _nonce: &[u8; 12],
        _aad: &[u8],
        _buffer: &mut dyn GcmInPlaceBuffer,
    ) -> Result<(), CryptoProviderError> {
        Ok(())
    }
}

/// Delegates everything to the real provider except the transport AEAD, so only
/// the contract under test is broken.
struct ShortTagProvider(RustCryptoProvider);

impl SignalCryptoProvider for ShortTagProvider {
    fn aes_256_cbc_encrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        self.0.aes_256_cbc_encrypt(key, iv, plaintext, out)
    }

    fn aes_256_cbc_decrypt(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        ciphertext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        self.0.aes_256_cbc_decrypt(key, iv, ciphertext, out)
    }

    fn aes_256_gcm_encrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        plaintext: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        self.0.aes_256_gcm_encrypt(key, nonce, aad, plaintext, out)
    }

    fn aes_256_gcm_decrypt(
        &self,
        key: &[u8; 32],
        nonce: &[u8; 12],
        aad: &[u8],
        ciphertext_with_tag: &[u8],
        out: &mut Vec<u8>,
    ) -> Result<(), CryptoProviderError> {
        self.0
            .aes_256_gcm_decrypt(key, nonce, aad, ciphertext_with_tag, out)
    }

    fn hmac_sha256(&self, key: &[u8], input: &[u8]) -> [u8; 32] {
        self.0.hmac_sha256(key, input)
    }

    fn transport_aead(
        &'static self,
        _key: &[u8; 32],
    ) -> Result<Box<dyn TransportAead>, CryptoProviderError> {
        Ok(Box::new(ShortTagAead))
    }
}

/// Records writes so the test can assert nothing reached the wire.
struct RecordingTransport {
    writes: Mutex<Vec<bytes::Bytes>>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl whatsapp_rust::transport::Transport for RecordingTransport {
    async fn send(&self, data: bytes::Bytes) -> Result<(), anyhow::Error> {
        self.writes.lock().expect("writes mutex").push(data);
        Ok(())
    }
    async fn disconnect(&self) {}
}

#[tokio::test]
async fn a_provider_that_breaks_the_tag_size_contract_cannot_write_a_frame() {
    set_crypto_provider(ShortTagProvider(RustCryptoProvider))
        .expect("this binary installs the provider exactly once");

    let key = [0x11u8; 32];
    let transport = Arc::new(RecordingTransport {
        writes: Mutex::new(Vec::new()),
    });
    let socket = whatsapp_rust::socket::NoiseSocket::new(
        Arc::new(whatsapp_rust::runtime_impl::TokioRuntime),
        transport.clone(),
        wacore::handshake::NoiseCipher::new(&key).expect("32-byte key"),
        wacore::handshake::NoiseCipher::new(&key).expect("32-byte key"),
    );

    let err = socket
        .encrypt_and_send(bytes::Bytes::from(vec![7u8; 64]))
        .await
        .expect_err("a frame whose sealed size disagrees with its prefix must be refused");

    assert!(
        matches!(
            err.kind,
            whatsapp_rust::socket::error::EncryptSendErrorKind::Crypto
        ),
        "the mismatch is a crypto-layer fault, got {err:?}"
    );
    assert!(
        transport.writes.lock().expect("writes mutex").is_empty(),
        "a frame with a prefix that disagrees with its body would desync the peer's parser, so it must never be written"
    );
}
