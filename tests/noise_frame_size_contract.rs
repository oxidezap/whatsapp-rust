//! The sender writes a frame's length prefix from `plaintext.len() + 16` before
//! the ciphertext exists, which holds only because `TransportAead` is
//! AES-256-GCM by contract. This pins what happens when an installed provider
//! breaks that contract: the frame must be refused, never written.
//!
//! Provider selection belongs to each cipher, so this fault injection cannot
//! change the provider used by another socket.

use std::sync::Arc;
use std::sync::Mutex;

use wacore::libsignal::crypto::{
    CryptoProviderError, GcmInPlaceBuffer, SignalCryptoProvider, TransportAead,
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

/// Selects the deliberately broken transport AEAD for this socket only.
struct ShortTagProvider;

impl SignalCryptoProvider for ShortTagProvider {
    fn transport_aead(
        &self,
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
    let key = [0x11u8; 32];
    let default_cipher = wacore::handshake::NoiseCipher::new(&key).unwrap();
    assert_eq!(
        default_cipher
            .encrypt_with_counter(0, &[7; 64])
            .unwrap()
            .len(),
        80
    );
    let transport = Arc::new(RecordingTransport {
        writes: Mutex::new(Vec::new()),
    });
    let socket = whatsapp_rust::socket::NoiseSocket::new(
        Arc::new(whatsapp_rust::runtime_impl::TokioRuntime),
        transport.clone(),
        wacore::handshake::NoiseCipher::with_provider(&key, &ShortTagProvider)
            .expect("test provider prepares cipher"),
        wacore::handshake::NoiseCipher::with_provider(&key, &ShortTagProvider)
            .expect("test provider prepares cipher"),
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
