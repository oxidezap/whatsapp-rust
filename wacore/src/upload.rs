use crate::download::{DownloadUtils, MediaType};
use crate::libsignal::crypto::{CryptographicHash, CryptographicMac};
use aes::Aes256;
use aes::cipher::{Block, BlockCipherEncrypt, KeyInit};
use anyhow::Result;
use rand::RngExt;
use rand::rng;
use std::io::{Read, Write};

const BLOCK: usize = 16;
/// Length of the truncated HMAC-SHA256 appended to the ciphertext.
const MEDIA_MAC_LEN: usize = 10;

/// Streaming sidecar chunk size: one HMAC per 64 KiB of ciphertext.
const SIDECAR_CHUNK: usize = 64 * 1024;
/// Each sidecar entry is a 10-byte truncated HMAC-SHA256.
const SIDECAR_MAC_LEN: usize = 10;
/// 16-byte (one AES block) overlap between consecutive sidecar windows, needed
/// so each 64 KiB window carries the CBC chaining context of its neighbour.
const SIDECAR_OVERLAP: usize = 16;

pub struct EncryptedMedia {
    pub data_to_upload: Vec<u8>,
    pub media_key: [u8; 32],
    pub file_sha256: [u8; 32],
    pub file_enc_sha256: [u8; 32],
    /// Per-64-KiB HMAC table for progressive playback/seek (audio/video only).
    pub streaming_sidecar: Option<Vec<u8>>,
}

pub struct EncryptedMediaInfo {
    pub media_key: [u8; 32],
    pub file_sha256: [u8; 32],
    pub file_enc_sha256: [u8; 32],
    pub file_length: u64,
    /// Per-64-KiB HMAC table for progressive playback/seek (audio/video only).
    pub streaming_sidecar: Option<Vec<u8>>,
}

/// Chunk-based AES-256-CBC media encryptor.
///
/// Processes plaintext incrementally without requiring sync `Read`, enabling
/// use with async streams, network sources, or any chunk-at-a-time producer.
///
/// Two output modes (zero duplicated crypto logic):
/// - `update()` / `finalize()` — append to a `Vec<u8>`
/// - `update_to_writer()` / `finalize_to_writer()` — write directly, zero intermediate buffer
#[must_use = "call finalize() or finalize_to_writer() to complete encryption"]
pub struct MediaEncryptor {
    cipher: Aes256,
    hmac: CryptographicMac,
    sha256_plain: CryptographicHash,
    sha256_enc: CryptographicHash,
    prev_block: [u8; BLOCK],
    /// Partial plaintext that didn't fill a complete AES block (≤15 bytes).
    remainder: Vec<u8>,
    media_key: [u8; 32],
    file_length: u64,
    /// Present when a streaming sidecar is being accumulated over the ciphertext.
    sidecar: Option<SidecarAccumulator>,
}

impl MediaEncryptor {
    /// Initialize with a random media key (no streaming sidecar).
    pub fn new(media_type: MediaType) -> Result<Self> {
        Self::new_with_sidecar(media_type, false)
    }

    /// Initialize with a random media key, optionally accumulating a streaming sidecar.
    pub fn new_with_sidecar(media_type: MediaType, sidecar: bool) -> Result<Self> {
        let mut media_key = [0u8; 32];
        rng().fill(&mut media_key);
        Self::with_key_and_sidecar(media_key, media_type, sidecar)
    }

    /// Initialize with a caller-supplied key (no streaming sidecar). The key must
    /// be 32 cryptographically random bytes; reusing keys breaks confidentiality.
    pub fn with_key(media_key: [u8; 32], media_type: MediaType) -> Result<Self> {
        Self::with_key_and_sidecar(media_key, media_type, false)
    }

    /// Initialize with a caller-supplied key, optionally accumulating a streaming
    /// sidecar (one truncated HMAC per 64 KiB of ciphertext, for audio/video).
    pub fn with_key_and_sidecar(
        media_key: [u8; 32],
        media_type: MediaType,
        sidecar: bool,
    ) -> Result<Self> {
        let (iv, cipher_key, mac_key) = DownloadUtils::get_media_keys(&media_key, media_type)?;
        let cipher =
            Aes256::new_from_slice(&cipher_key).map_err(|_| anyhow::anyhow!("Bad AES key"))?;
        let mut hmac = CryptographicMac::new("HmacSha256", &mac_key)?;
        hmac.update(&iv);

        Ok(Self {
            cipher,
            hmac,
            sha256_plain: CryptographicHash::new("SHA-256")?,
            sha256_enc: CryptographicHash::new("SHA-256")?,
            prev_block: iv,
            remainder: Vec::with_capacity(BLOCK),
            media_key,
            file_length: 0,
            sidecar: sidecar.then(|| SidecarAccumulator::new(mac_key)),
        })
    }

    /// Feed plaintext, append encrypted blocks to `out`.
    pub fn update(&mut self, plaintext: &[u8], out: &mut Vec<u8>) {
        self.feed(plaintext, |block| out.extend_from_slice(block));
    }

    /// Feed plaintext, write encrypted blocks directly to `writer`.
    ///
    /// On error the encryptor state is unspecified — discard it.
    pub fn update_to_writer<W: Write>(
        &mut self,
        plaintext: &[u8],
        writer: &mut W,
    ) -> std::io::Result<()> {
        let mut err = Ok(());
        self.feed(plaintext, |block| {
            if err.is_ok() {
                err = writer.write_all(block);
            }
        });
        err
    }

    /// PKCS7 pad + 10-byte MAC. Appends final bytes to `out`.
    pub fn finalize(mut self, out: &mut Vec<u8>) -> Result<EncryptedMediaInfo> {
        self.pad_and_encrypt(|block| out.extend_from_slice(block));
        let mac = self.compute_mac()?;
        out.extend_from_slice(&mac);
        self.finish_hashes(&mac)
    }

    /// PKCS7 pad + 10-byte MAC. Writes directly to `writer`.
    pub fn finalize_to_writer<W: Write>(mut self, writer: &mut W) -> Result<EncryptedMediaInfo> {
        let mut io_err: std::io::Result<()> = Ok(());
        self.pad_and_encrypt(|block| {
            if io_err.is_ok() {
                io_err = writer.write_all(block);
            }
        });
        io_err?;

        let mac = self.compute_mac()?;
        writer.write_all(&mac)?;
        self.finish_hashes(&mac)
    }

    /// Hash plaintext, then encrypt complete blocks directly from the input
    /// without copying everything into `remainder` first. Only the trailing
    /// partial block (≤15 bytes) is buffered.
    fn feed(&mut self, plaintext: &[u8], mut emit: impl FnMut(&[u8; BLOCK])) {
        self.sha256_plain.update(plaintext);
        self.file_length += plaintext.len() as u64;

        // If there's leftover from the previous call, try to complete a block.
        let input = if !self.remainder.is_empty() {
            let need = BLOCK - self.remainder.len();
            if plaintext.len() < need {
                // Not enough to complete a block — just buffer.
                self.remainder.extend_from_slice(plaintext);
                return;
            }
            // Complete the partial block from remainder + head of plaintext.
            self.remainder.extend_from_slice(&plaintext[..need]);
            let completed = std::mem::take(&mut self.remainder);
            self.encrypt_and_emit(&completed, &mut emit);
            &plaintext[need..]
        } else {
            plaintext
        };

        // Process full blocks directly from input (no copy).
        let full = (input.len() / BLOCK) * BLOCK;
        if full > 0 {
            self.encrypt_and_emit(&input[..full], &mut emit);
        }

        // Buffer the leftover tail.
        let tail = &input[full..];
        if !tail.is_empty() {
            self.remainder.extend_from_slice(tail);
        }
    }

    /// Encrypt one or more complete blocks from `data` and emit each.
    fn encrypt_and_emit(&mut self, data: &[u8], emit: &mut impl FnMut(&[u8; BLOCK])) {
        debug_assert!(data.len().is_multiple_of(BLOCK));
        for chunk in data.chunks_exact(BLOCK) {
            self.cbc_encrypt(chunk.try_into().unwrap());
            emit(&self.prev_block);
            self.hmac.update(&self.prev_block);
            self.sha256_enc.update(&self.prev_block);
            if let Some(sc) = &mut self.sidecar {
                sc.push(&self.prev_block);
            }
        }
    }

    fn pad_and_encrypt(&mut self, mut emit: impl FnMut(&[u8; BLOCK])) {
        let pad_len = BLOCK - (self.remainder.len() % BLOCK);
        self.remainder
            .extend(std::iter::repeat_n(pad_len as u8, pad_len));
        let rem = std::mem::take(&mut self.remainder);
        self.encrypt_and_emit(&rem, &mut emit);
    }

    fn cbc_encrypt(&mut self, block_data: &[u8; BLOCK]) {
        let mut data = *block_data;
        for (b, &p) in data.iter_mut().zip(self.prev_block.iter()) {
            *b ^= p;
        }
        let mut block: Block<Aes256> = data.into();
        self.cipher.encrypt_block(&mut block);
        self.prev_block = block.into();
    }

    fn compute_mac(&mut self) -> Result<[u8; 10]> {
        let mac_full = self.hmac.finalize_sha256_array()?;
        let mut mac = [0u8; 10];
        mac.copy_from_slice(&mac_full[..10]);
        Ok(mac)
    }

    fn finish_hashes(mut self, mac: &[u8; 10]) -> Result<EncryptedMediaInfo> {
        self.sha256_enc.update(mac);
        // The sidecar covers the whole uploaded blob (ciphertext + trailing MAC),
        // so feed the MAC before sealing it.
        let streaming_sidecar = match self.sidecar.take() {
            Some(mut sc) => {
                sc.push(mac);
                Some(sc.finish()?)
            }
            None => None,
        };
        Ok(EncryptedMediaInfo {
            media_key: self.media_key,
            file_sha256: self.sha256_plain.finalize_sha256_array()?,
            file_enc_sha256: self.sha256_enc.finalize_sha256_array()?,
            file_length: self.file_length,
            streaming_sidecar,
        })
    }
}

/// Accumulates a WhatsApp streaming sidecar: one 10-byte truncated HMAC-SHA256
/// per 64 KiB window of ciphertext, each window overlapping the next by one AES
/// block (16 bytes) to preserve CBC chaining. Fed incrementally with the
/// encrypted blocks (and the trailing MAC) so it never holds the whole file.
struct SidecarAccumulator {
    mac_key: [u8; 32],
    /// Bytes buffered for the window currently being hashed.
    window: Vec<u8>,
    /// Concatenated 10-byte HMACs produced so far.
    result: Vec<u8>,
    /// Absolute offset where the current window logically starts.
    next_chunk_start: u64,
    /// Total bytes pushed so far.
    total_pushed: u64,
    /// First HMAC construction error, surfaced by `finish`.
    err: Option<anyhow::Error>,
}

impl SidecarAccumulator {
    fn new(mac_key: [u8; 32]) -> Self {
        Self {
            mac_key,
            window: Vec::with_capacity(SIDECAR_OVERLAP + SIDECAR_CHUNK),
            result: Vec::new(),
            next_chunk_start: 0,
            total_pushed: 0,
            err: None,
        }
    }

    fn push(&mut self, data: &[u8]) {
        let mut src = 0;
        while src < data.len() {
            let window_end = self.next_chunk_start + (SIDECAR_OVERLAP + SIDECAR_CHUNK) as u64;
            let remaining = (window_end - self.total_pushed) as usize;
            let to_copy = remaining.min(data.len() - src);
            self.window.extend_from_slice(&data[src..src + to_copy]);
            self.total_pushed += to_copy as u64;
            src += to_copy;
            if self.total_pushed == window_end {
                self.flush();
            }
        }
    }

    /// Emit the HMAC of the current window, then retain only the trailing
    /// 16-byte overlap as the start of the next window.
    fn flush(&mut self) {
        if self.window.is_empty() {
            return;
        }
        match hmac_sha256_trunc10(&self.mac_key, &self.window) {
            Ok(mac) => self.result.extend_from_slice(&mac),
            Err(e) => {
                self.err.get_or_insert(e);
            }
        }
        self.next_chunk_start += SIDECAR_CHUNK as u64;
        let keep_from = self.window.len().saturating_sub(SIDECAR_OVERLAP);
        self.window.drain(0..keep_from);
    }

    fn finish(mut self) -> Result<Vec<u8>> {
        if !self.window.is_empty() {
            self.flush();
        }
        match self.err {
            Some(e) => Err(e),
            None => Ok(self.result),
        }
    }
}

fn hmac_sha256_trunc10(mac_key: &[u8], data: &[u8]) -> Result<[u8; SIDECAR_MAC_LEN]> {
    let mut hmac = CryptographicMac::new("HmacSha256", mac_key)?;
    hmac.update(data);
    let full = hmac.finalize_sha256_array()?;
    let mut out = [0u8; SIDECAR_MAC_LEN];
    out.copy_from_slice(&full[..SIDECAR_MAC_LEN]);
    Ok(out)
}

/// Audio and video are the only media types that benefit from a streaming
/// sidecar (progressive playback/seek); images/documents are fetched whole.
fn media_type_uses_sidecar(media_type: MediaType) -> bool {
    matches!(media_type, MediaType::Audio | MediaType::Video)
}

/// A re-readable source of already-encrypted upload bytes.
///
/// The upload path may read the body more than once (host failover, auth
/// refresh) and from an arbitrary offset (server-driven resume), so a single
/// consumable `Read` is not enough. Implementors decide where the ciphertext
/// lives — this crate never creates temporary files.
///
/// An in-memory implementation is provided for `Arc<[u8]>`. For a disk-staged
/// body, implement this over your own handle, e.g.:
///
/// ```ignore
/// struct FileSource { path: PathBuf, len: u64 }
/// impl UploadSource for FileSource {
///     fn len(&self) -> u64 { self.len }
///     fn reader_from(&self, offset: u64) -> std::io::Result<Box<dyn Read + Send>> {
///         let mut f = std::fs::File::open(&self.path)?;
///         f.seek(std::io::SeekFrom::Start(offset))?;
///         Ok(Box::new(f))
///     }
/// }
/// ```
pub trait UploadSource: Send + Sync {
    /// Total length of the encrypted blob in bytes.
    fn len(&self) -> u64;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// A reader positioned at `offset`. Called once per upload attempt.
    fn reader_from(&self, offset: u64) -> std::io::Result<Box<dyn Read + Send>>;
}

impl UploadSource for std::sync::Arc<[u8]> {
    fn len(&self) -> u64 {
        (**self).len() as u64
    }

    fn reader_from(&self, offset: u64) -> std::io::Result<Box<dyn Read + Send>> {
        let mut cursor = std::io::Cursor::new(std::sync::Arc::clone(self));
        cursor.set_position(offset.min(self.len()));
        Ok(Box::new(cursor))
    }
}

/// O(1) to construct from an owned `Vec<u8>` (`Bytes::from(vec)` adopts the
/// buffer), unlike `Arc::<[u8]>::from(vec.into_boxed_slice())` which copies the
/// whole ciphertext into a fresh refcounted allocation. Prefer this when the
/// encrypted blob was produced into a `Vec` (e.g. by [`encrypt_media_streaming`]).
impl UploadSource for bytes::Bytes {
    fn len(&self) -> u64 {
        (**self).len() as u64
    }

    fn reader_from(&self, offset: u64) -> std::io::Result<Box<dyn Read + Send>> {
        let mut cursor = std::io::Cursor::new(self.clone());
        cursor.set_position(offset.min((**self).len() as u64));
        Ok(Box::new(cursor))
    }
}

/// The exact size, in bytes, of the encrypted blob that [`encrypt_media`] /
/// [`encrypt_media_streaming`] produce for a `plaintext_len`-byte input.
///
/// WhatsApp media encryption is AES-256-CBC with PKCS#7 padding followed by a
/// 10-byte truncated HMAC-SHA256. PKCS#7 always appends between 1 and 16 bytes —
/// a *full* block when the input is already 16-byte aligned — so the ciphertext
/// is `plaintext_len` rounded **up** to the next 16-byte boundary, plus 10.
///
/// Use it to size the destination buffer exactly so a `Vec` neither grows during
/// [`encrypt_media_streaming`]'s per-block writes nor shrink-reallocates when
/// wrapped as an [`UploadSource`] — the encrypted blob is the single biggest
/// allocation in the upload path:
///
/// ```
/// use wacore::upload::{encrypt_media_streaming, encrypted_len};
/// use wacore::download::MediaType;
/// let plaintext = vec![0u8; 1000];
/// let mut ciphertext = Vec::with_capacity(encrypted_len(plaintext.len()));
/// encrypt_media_streaming(&plaintext[..], &mut ciphertext, MediaType::Image).unwrap();
/// assert_eq!(ciphertext.len(), encrypted_len(plaintext.len()));
/// ```
pub const fn encrypted_len(plaintext_len: usize) -> usize {
    (plaintext_len / BLOCK + 1) * BLOCK + MEDIA_MAC_LEN
}

/// Encrypt media streaming with constant memory. A streaming sidecar is
/// generated automatically for audio/video.
pub fn encrypt_media_streaming<R: Read, W: Write>(
    reader: R,
    writer: W,
    media_type: MediaType,
) -> Result<EncryptedMediaInfo> {
    encrypt_media_streaming_with_key(reader, writer, media_type, None, None)
}

/// Like [`encrypt_media_streaming`] but accepts an optional pre-existing key and
/// an explicit sidecar override (`None` = automatic, by media type).
pub fn encrypt_media_streaming_with_key<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    media_type: MediaType,
    media_key: Option<&[u8; 32]>,
    sidecar: Option<bool>,
) -> Result<EncryptedMediaInfo> {
    let want_sidecar = sidecar.unwrap_or_else(|| media_type_uses_sidecar(media_type));
    let mut enc = match media_key {
        Some(key) => MediaEncryptor::with_key_and_sidecar(*key, media_type, want_sidecar)?,
        None => MediaEncryptor::new_with_sidecar(media_type, want_sidecar)?,
    };
    let mut buf = [0u8; 8 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        enc.update_to_writer(&buf[..n], &mut writer)?;
    }
    let info = enc.finalize_to_writer(&mut writer)?;
    writer.flush()?;
    Ok(info)
}

/// Encrypt media in memory. A streaming sidecar is generated automatically for
/// audio/video.
pub fn encrypt_media(plaintext: &[u8], media_type: MediaType) -> Result<EncryptedMedia> {
    encrypt_media_with_key(plaintext, media_type, None)
}

/// Like `encrypt_media` but accepts an optional pre-existing key.
pub fn encrypt_media_with_key(
    plaintext: &[u8],
    media_type: MediaType,
    media_key: Option<&[u8; 32]>,
) -> Result<EncryptedMedia> {
    encrypt_media_with_key_and_sidecar(plaintext, media_type, media_key, None)
}

/// Like [`encrypt_media_with_key`] but with an explicit sidecar override
/// (`None` = automatic, by media type).
pub fn encrypt_media_with_key_and_sidecar(
    plaintext: &[u8],
    media_type: MediaType,
    media_key: Option<&[u8; 32]>,
    sidecar: Option<bool>,
) -> Result<EncryptedMedia> {
    let want_sidecar = sidecar.unwrap_or_else(|| media_type_uses_sidecar(media_type));
    let mut enc = match media_key {
        Some(key) => MediaEncryptor::with_key_and_sidecar(*key, media_type, want_sidecar)?,
        None => MediaEncryptor::new_with_sidecar(media_type, want_sidecar)?,
    };
    let mut data_to_upload = Vec::new();
    enc.update(plaintext, &mut data_to_upload);
    let info = enc.finalize(&mut data_to_upload)?;
    Ok(EncryptedMedia {
        data_to_upload,
        media_key: info.media_key,
        file_sha256: info.file_sha256,
        file_enc_sha256: info.file_enc_sha256,
        streaming_sidecar: info.streaming_sidecar,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::download::DownloadUtils;
    use std::io::Cursor;

    #[test]
    fn encrypted_len_matches_encryptor_output() {
        // Tie the helper to the real encryptor across block boundaries (incl. the
        // aligned case, where PKCS#7 appends a full padding block) so a change to
        // the padding/MAC can't silently desync `encrypted_len` from reality.
        for &n in &[0usize, 1, 15, 16, 17, 31, 32, 100, 4096, 59383] {
            let enc = encrypt_media(&vec![0xABu8; n], MediaType::Image).unwrap();
            assert_eq!(
                enc.data_to_upload.len(),
                encrypted_len(n),
                "encrypted_len mismatch for plaintext_len={n}"
            );
        }
    }

    #[test]
    fn bytes_upload_source_reads_from_offset() {
        let data = bytes::Bytes::from(vec![1u8, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(UploadSource::len(&data), 8);
        assert!(!UploadSource::is_empty(&data));

        let mut reader = data.reader_from(3).unwrap();
        let mut out = Vec::new();
        reader.read_to_end(&mut out).unwrap();
        assert_eq!(out, vec![4, 5, 6, 7, 8]);

        // Offset at/past the end yields an empty read (mirrors the Arc impl).
        let mut past = data.reader_from(100).unwrap();
        let mut empty = Vec::new();
        past.read_to_end(&mut empty).unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn bytes_upload_source_matches_arc_slice() {
        // The two in-memory UploadSource impls must stream identical bytes.
        let raw = vec![7u8; 1000];
        let as_bytes = bytes::Bytes::from(raw.clone());
        let as_arc: std::sync::Arc<[u8]> = std::sync::Arc::from(raw.into_boxed_slice());
        assert_eq!(UploadSource::len(&as_bytes), UploadSource::len(&as_arc));
        let mut a = Vec::new();
        let mut b = Vec::new();
        as_bytes
            .reader_from(0)
            .unwrap()
            .read_to_end(&mut a)
            .unwrap();
        as_arc.reader_from(0).unwrap().read_to_end(&mut b).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn roundtrip_decrypt_stream() {
        let msg = b"Roundtrip encryption test payload.";
        let enc = encrypt_media(msg, MediaType::Image).expect("encrypt");
        let plain = DownloadUtils::decrypt_stream(
            Cursor::new(enc.data_to_upload),
            &enc.media_key,
            MediaType::Image,
        )
        .expect("decrypt");
        assert_eq!(plain, msg);
    }

    #[test]
    fn streaming_roundtrip() {
        let msg = b"Streaming encryption roundtrip test with enough data to span multiple blocks.";
        let mut encrypted = Vec::new();
        let info = encrypt_media_streaming(
            Cursor::new(msg.as_slice()),
            &mut encrypted,
            MediaType::Image,
        )
        .expect("encrypt");

        assert_eq!(info.file_length, msg.len() as u64);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(encrypted),
            &info.media_key,
            MediaType::Image,
        )
        .expect("decrypt");
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn streaming_matches_buffered() {
        let msg = vec![0xABu8; 8192 * 3 + 7];
        let mut encrypted = Vec::new();
        let info = encrypt_media_streaming(
            Cursor::new(msg.as_slice()),
            &mut encrypted,
            MediaType::Video,
        )
        .expect("encrypt");

        let expected_sha256 = {
            let mut h = CryptographicHash::new("SHA-256").unwrap();
            h.update(&msg);
            h.finalize_sha256_array().unwrap()
        };
        assert_eq!(info.file_sha256, expected_sha256);

        let actual_enc_sha256 = {
            let mut h = CryptographicHash::new("SHA-256").unwrap();
            h.update(&encrypted);
            h.finalize_sha256_array().unwrap()
        };
        assert_eq!(info.file_enc_sha256, actual_enc_sha256);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(encrypted),
            &info.media_key,
            MediaType::Video,
        )
        .expect("decrypt");
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn streaming_empty_input() {
        let mut encrypted = Vec::new();
        let info = encrypt_media_streaming(
            Cursor::new(Vec::<u8>::new()),
            &mut encrypted,
            MediaType::Document,
        )
        .expect("encrypt");

        assert_eq!(info.file_length, 0);
        assert_eq!(encrypted.len(), 16 + 10);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(encrypted),
            &info.media_key,
            MediaType::Document,
        )
        .expect("decrypt");
        assert!(decrypted.is_empty());
    }

    #[test]
    fn streaming_exact_block_boundary() {
        let msg = vec![0x42u8; 16];
        let mut encrypted = Vec::new();
        let info = encrypt_media_streaming(
            Cursor::new(msg.as_slice()),
            &mut encrypted,
            MediaType::Audio,
        )
        .expect("encrypt");

        assert_eq!(encrypted.len(), 32 + 10);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(encrypted),
            &info.media_key,
            MediaType::Audio,
        )
        .expect("decrypt");
        assert_eq!(decrypted, msg);
    }

    #[test]
    fn media_encryptor_chunk_api() {
        let msg = b"Test the chunk-based MediaEncryptor API directly.";
        let mut enc = MediaEncryptor::new(MediaType::Image).unwrap();

        let mut all_encrypted = Vec::new();
        for chunk in msg.chunks(7) {
            enc.update(chunk, &mut all_encrypted);
        }
        let info = enc.finalize(&mut all_encrypted).unwrap();

        assert_eq!(info.file_length, msg.len() as u64);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(all_encrypted),
            &info.media_key,
            MediaType::Image,
        )
        .expect("decrypt");
        assert_eq!(decrypted, msg.as_slice());
    }

    #[test]
    fn media_encryptor_single_byte_chunks() {
        let msg = b"One byte at a time to stress the remainder logic.";
        let mut enc = MediaEncryptor::new(MediaType::Document).unwrap();

        let mut all_encrypted = Vec::new();
        for &byte in msg.iter() {
            enc.update(&[byte], &mut all_encrypted);
        }
        let info = enc.finalize(&mut all_encrypted).unwrap();

        assert_eq!(info.file_length, msg.len() as u64);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(all_encrypted),
            &info.media_key,
            MediaType::Document,
        )
        .expect("decrypt");
        assert_eq!(decrypted, msg.as_slice());
    }

    #[test]
    fn media_encryptor_large_single_chunk() {
        let msg = vec![0xCDu8; 1024 * 1024]; // 1MB in one call
        let mut enc = MediaEncryptor::new(MediaType::Video).unwrap();

        let mut all_encrypted = Vec::new();
        enc.update(&msg, &mut all_encrypted);
        let info = enc.finalize(&mut all_encrypted).unwrap();

        assert_eq!(info.file_length, msg.len() as u64);

        let decrypted = DownloadUtils::decrypt_stream(
            Cursor::new(all_encrypted),
            &info.media_key,
            MediaType::Video,
        )
        .expect("decrypt");
        assert_eq!(decrypted, msg);
    }

    // ---- streaming/buffer parity & sidecar ----

    /// Deterministic pseudo-random payload (no `rand` needed; reproducible).
    fn payload(len: usize, seed: u8) -> Vec<u8> {
        let mut v = Vec::with_capacity(len);
        let mut x = seed;
        for i in 0..len {
            x = x.wrapping_mul(31).wrapping_add(i as u8).wrapping_add(7);
            v.push(x);
        }
        v
    }

    /// Independent ("naive") reimplementation of the sidecar straight from the
    /// full encrypted blob, used to pin the incremental accumulator's behaviour.
    fn naive_sidecar(enc_full: &[u8], mac_key: &[u8; 32]) -> Vec<u8> {
        let c = SIDECAR_CHUNK;
        let o = SIDECAR_OVERLAP;
        let mut out = Vec::new();
        let mut start = 0usize;
        while start + c + o <= enc_full.len() {
            out.extend_from_slice(
                &hmac_sha256_trunc10(mac_key, &enc_full[start..start + c + o]).unwrap(),
            );
            start += c;
        }
        // Final flush: always runs (the blob is never empty — it carries the MAC).
        out.extend_from_slice(&hmac_sha256_trunc10(mac_key, &enc_full[start..]).unwrap());
        out
    }

    fn mac_key_for(media_key: &[u8; 32], media_type: MediaType) -> [u8; 32] {
        let (_, _, mac_key) = DownloadUtils::get_media_keys(media_key, media_type).unwrap();
        mac_key
    }

    #[test]
    fn streaming_and_buffered_are_byte_identical() {
        // Same key + same media type must produce identical ciphertext, hashes,
        // and sidecar across the buffered and streaming paths.
        let key = [7u8; 32];
        for &len in &[0usize, 1, 15, 16, 17, 8192 * 3 + 5, 200_000] {
            let data = payload(len, len as u8);
            let buffered =
                encrypt_media_with_key_and_sidecar(&data, MediaType::Video, Some(&key), None)
                    .unwrap();

            let mut streamed_blob = Vec::new();
            let info = encrypt_media_streaming_with_key(
                Cursor::new(data.as_slice()),
                &mut streamed_blob,
                MediaType::Video,
                Some(&key),
                None,
            )
            .unwrap();

            assert_eq!(
                buffered.data_to_upload, streamed_blob,
                "ciphertext (len {len})"
            );
            assert_eq!(
                buffered.file_sha256, info.file_sha256,
                "file_sha256 (len {len})"
            );
            assert_eq!(
                buffered.file_enc_sha256, info.file_enc_sha256,
                "file_enc_sha256 (len {len})"
            );
            assert_eq!(
                buffered.streaming_sidecar, info.streaming_sidecar,
                "sidecar (len {len})"
            );
            assert!(buffered.streaming_sidecar.is_some(), "video has a sidecar");
        }
    }

    #[test]
    fn sidecar_matches_naive_reference() {
        // Cross every 64 KiB boundary: just below, exactly at, and just above
        // one and two windows of *ciphertext*.
        let key = [0x33u8; 32];
        let mac_key = mac_key_for(&key, MediaType::Audio);
        for &len in &[
            0usize,
            10,
            SIDECAR_CHUNK - 32,
            SIDECAR_CHUNK,
            SIDECAR_CHUNK + 16,
            SIDECAR_CHUNK + 64,
            2 * SIDECAR_CHUNK,
            2 * SIDECAR_CHUNK + 100,
            3 * SIDECAR_CHUNK + 7,
        ] {
            let data = payload(len, 0x5A);
            let enc = encrypt_media_with_key_and_sidecar(&data, MediaType::Audio, Some(&key), None)
                .unwrap();
            let sidecar = enc.streaming_sidecar.expect("audio sidecar");
            let expected = naive_sidecar(&enc.data_to_upload, &mac_key);
            assert_eq!(
                sidecar, expected,
                "sidecar mismatch for plaintext len {len}"
            );
            assert_eq!(
                sidecar.len() % SIDECAR_MAC_LEN,
                0,
                "sidecar must be a whole number of 10-byte entries"
            );
        }
    }

    #[test]
    fn sidecar_first_entry_is_hmac_of_first_window() {
        // Direct, hand-rolled check of the 64 KiB + 16 overlap window.
        let key = [0x91u8; 32];
        let mac_key = mac_key_for(&key, MediaType::Video);
        let data = payload(3 * SIDECAR_CHUNK, 0x11);
        let enc =
            encrypt_media_with_key_and_sidecar(&data, MediaType::Video, Some(&key), None).unwrap();
        let sidecar = enc.streaming_sidecar.unwrap();

        let first = hmac_sha256_trunc10(
            &mac_key,
            &enc.data_to_upload[..SIDECAR_CHUNK + SIDECAR_OVERLAP],
        )
        .unwrap();
        assert_eq!(&sidecar[..SIDECAR_MAC_LEN], &first[..]);
    }

    #[test]
    fn sidecar_independent_of_input_chunking() {
        // Feeding the encryptor in odd-sized pieces must not change the sidecar.
        let key = [0x44u8; 32];
        let data = payload(2 * SIDECAR_CHUNK + 123, 0x77);

        let oneshot =
            encrypt_media_with_key_and_sidecar(&data, MediaType::Video, Some(&key), None).unwrap();

        let mut enc =
            MediaEncryptor::with_key_and_sidecar([0x44u8; 32], MediaType::Video, true).unwrap();
        let mut blob = Vec::new();
        for chunk in data.chunks(7) {
            enc.update(chunk, &mut blob);
        }
        let info = enc.finalize(&mut blob).unwrap();

        assert_eq!(oneshot.data_to_upload, blob);
        assert_eq!(oneshot.streaming_sidecar, info.streaming_sidecar);
    }

    #[test]
    fn sidecar_only_for_audio_and_video_by_default() {
        let key = [1u8; 32];
        for mt in [MediaType::Audio, MediaType::Video] {
            let enc = encrypt_media_with_key(&payload(1000, 1), mt, Some(&key)).unwrap();
            assert!(
                enc.streaming_sidecar.is_some(),
                "{mt:?} should have a sidecar"
            );
        }
        for mt in [MediaType::Image, MediaType::Document, MediaType::Sticker] {
            let enc = encrypt_media_with_key(&payload(1000, 1), mt, Some(&key)).unwrap();
            assert!(
                enc.streaming_sidecar.is_none(),
                "{mt:?} should have no sidecar"
            );
        }
    }

    #[test]
    fn sidecar_override_forces_and_inhibits() {
        let key = [2u8; 32];
        // Force a sidecar on an image.
        let forced = encrypt_media_with_key_and_sidecar(
            &payload(1000, 2),
            MediaType::Image,
            Some(&key),
            Some(true),
        )
        .unwrap();
        assert!(forced.streaming_sidecar.is_some());
        // Inhibit a sidecar on a video.
        let inhibited = encrypt_media_with_key_and_sidecar(
            &payload(1000, 2),
            MediaType::Video,
            Some(&key),
            Some(false),
        )
        .unwrap();
        assert!(inhibited.streaming_sidecar.is_none());
    }
}
