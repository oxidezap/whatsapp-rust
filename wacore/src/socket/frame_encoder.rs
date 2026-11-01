//! Noise frame encoding without a transport, executor or clock.

use super::error::EncryptSendError;
use crate::handshake::{NoiseCipher, NoiseError};
use crate::libsignal::crypto::GcmInPlaceBuffer;
use bytes::BytesMut;
use std::sync::Arc;

pub const TAG_LEN: usize = 16;

/// The region of the batch buffer one frame's ciphertext occupies, exposed to
/// AES-GCM as if it were a buffer of its own.
pub struct FrameBody<'a> {
    pub out: &'a mut BytesMut,
    /// Offset in `out` where this frame's ciphertext starts, i.e. just past its
    /// length prefix. Held as an offset rather than a slice so the AEAD can grow
    /// the buffer by the tag through the same view.
    pub base: usize,
}

impl GcmInPlaceBuffer for FrameBody<'_> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.out[self.base..]
    }

    fn as_slice(&self) -> &[u8] {
        &self.out[self.base..]
    }

    fn resize(&mut self, new_len: usize, value: u8) {
        self.out.resize(self.base + new_len, value);
    }

    fn truncate(&mut self, len: usize) {
        self.out.truncate(self.base + len);
    }
}

pub struct FrameEncoder {
    key: Arc<NoiseCipher>,
    counter: u32,
    poisoned: bool,
}

impl FrameEncoder {
    pub fn new(key: NoiseCipher) -> Self {
        Self {
            key: Arc::new(key),
            counter: 0,
            poisoned: false,
        }
    }

    pub(crate) fn from_shared_key(key: Arc<NoiseCipher>) -> Self {
        Self {
            key,
            counter: 0,
            poisoned: false,
        }
    }

    pub fn poison(&mut self) {
        self.poisoned = true;
    }

    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    pub fn encode_into(
        &mut self,
        plaintext: &[u8],
        out: &mut BytesMut,
    ) -> Result<usize, EncryptSendError> {
        self.check_ready()?;
        encode_inline(&self.key, &mut self.counter, plaintext, out)
    }

    pub(crate) fn check_ready(&self) -> Result<(), EncryptSendError> {
        if self.poisoned {
            Err(EncryptSendError::poisoned())
        } else if self.counter == u32::MAX {
            Err(EncryptSendError::crypto(NoiseError::CounterExhausted))
        } else {
            Ok(())
        }
    }

    pub(crate) fn encryption_inputs(&self) -> (Arc<NoiseCipher>, u32) {
        (Arc::clone(&self.key), self.counter)
    }

    pub(crate) fn append_ciphertext(
        &mut self,
        ciphertext: &[u8],
        out: &mut BytesMut,
    ) -> Result<usize, EncryptSendError> {
        self.check_ready()?;
        append_ciphertext(&mut self.counter, ciphertext, out)
    }
}

pub(crate) fn encode_inline(
    key: &NoiseCipher,
    counter: &mut u32,
    plaintext: &[u8],
    out: &mut BytesMut,
) -> Result<usize, EncryptSendError> {
    if *counter == u32::MAX {
        return Err(EncryptSendError::crypto(NoiseError::CounterExhausted));
    }
    let before = out.len();
    let body_len = plaintext.len() + TAG_LEN;
    crate::framing::append_frame_header_into(body_len, None, out)
        .map_err(EncryptSendError::framing)?;
    let base = out.len();
    out.extend_from_slice(plaintext);
    if let Err(error) = key.encrypt_in_place_with_counter(*counter, &mut FrameBody { out, base }) {
        out.truncate(before);
        return Err(EncryptSendError::crypto(error));
    }
    if out.len() - base != body_len {
        out.truncate(before);
        return Err(EncryptSendError::crypto(NoiseError::Encrypt(
            crate::libsignal::crypto::CryptoProviderError::BackendFailed,
        )));
    }
    *counter += 1;
    Ok(out.len() - before)
}

pub(crate) fn append_ciphertext(
    counter: &mut u32,
    ciphertext: &[u8],
    out: &mut BytesMut,
) -> Result<usize, EncryptSendError> {
    let before = out.len();
    crate::framing::append_frame_into(ciphertext, None, out).map_err(EncryptSendError::framing)?;
    *counter += 1;
    Ok(out.len() - before)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encoder() -> FrameEncoder {
        FrameEncoder::new(NoiseCipher::new(&[42; 32]).unwrap())
    }

    #[test]
    fn consecutive_frames_match_explicit_nonce_encryption() {
        let mut state = encoder();
        let key = NoiseCipher::new(&[42; 32]).unwrap();
        let mut batch = BytesMut::new();
        for (counter, text) in [b"".as_slice(), b"hello", b"next"].into_iter().enumerate() {
            let before = batch.len();
            state.encode_into(text, &mut batch).unwrap();
            let ciphertext = key.encrypt_with_counter(counter as u32, text).unwrap();
            let mut expected = BytesMut::new();
            crate::framing::append_frame_into(&ciphertext, None, &mut expected).unwrap();
            assert_eq!(&batch[before..], &expected[..]);
        }
        assert_eq!(state.counter, 3);
    }

    #[test]
    fn failed_write_permanently_retires_the_key() {
        let mut state = encoder();
        let mut out = BytesMut::new();
        state.encode_into(b"one", &mut out).unwrap();
        let before = out.clone();
        state.poison();
        assert!(state.encode_into(b"two", &mut out).is_err());
        assert_eq!(state.counter, 1);
        assert_eq!(out, before);
    }

    #[test]
    fn exhausted_counter_preserves_prior_frames() {
        let mut state = encoder();
        state.counter = u32::MAX;
        let mut out = BytesMut::from(b"prior".as_slice());
        assert!(state.encode_into(b"no nonce", &mut out).is_err());
        assert_eq!(&out[..], b"prior");
    }

    #[test]
    fn offloaded_and_inline_encoding_are_identical() {
        let mut inline = encoder();
        let mut offloaded = encoder();
        let text = vec![3; 32 * 1024];
        let mut a = BytesMut::new();
        let mut b = BytesMut::new();
        inline.encode_into(&text, &mut a).unwrap();
        let (key, counter) = offloaded.encryption_inputs();
        let encrypted = key.encrypt_with_counter(counter, &text).unwrap();
        offloaded.append_ciphertext(&encrypted, &mut b).unwrap();
        assert_eq!(a, b);
        assert_eq!(inline.counter, offloaded.counter);
    }
}
