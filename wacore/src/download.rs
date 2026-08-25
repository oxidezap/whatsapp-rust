use crate::libsignal::crypto::{
    DecryptionError as AesCbcDecryptionError, Error as CryptoError, aes_256_cbc_decrypt_in_place,
    hmac_sha256_two_part,
};
use anyhow::{Result, anyhow};
use base64::Engine as _;
use base64::prelude::*;
use hmac::Hmac;
use hmac::Mac;
use sha2::Sha256;
use thiserror::Error;
use waproto::whatsapp as wa;
use waproto::whatsapp::ExternalBlobReference;
use waproto::whatsapp::message::HistorySyncNotification;

const MEDIA_MAC_SIZE: usize = 10;
const AES_BLOCK_SIZE: usize = 16;
const STREAM_CHUNK_SIZE: usize = 8 * 1024;
/// Bytes held back from processing while streaming: until the reader hits EOF,
/// the last `MEDIA_MAC_SIZE + AES_BLOCK_SIZE` bytes seen might be the final
/// ciphertext block plus the trailing MAC rather than more ciphertext.
const WITHHELD: usize = MEDIA_MAC_SIZE + AES_BLOCK_SIZE;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MediaDecryptionError {
    #[error("downloaded file is too short to contain MAC")]
    PayloadTooShort,
    #[error("invalid MAC signature")]
    InvalidMac,
    #[error("AES-CBC decryption failed")]
    Decryption(#[source] AesCbcDecryptionError),
    #[error("HMAC initialization failed")]
    Mac(#[source] CryptoError),
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Image,
    Video,
    Audio,
    Document,
    History,
    AppState,
    Sticker,
    StickerPack,
    StickerPackThumbnail,
    LinkThumbnail,
    /// Product catalog image — unencrypted, uploads to `/product/image`.
    /// WA Web: CreateMediaKeys.js throws for this type (no encryption).
    ProductCatalogImage,
}

impl MediaType {
    pub fn app_info(&self) -> &'static str {
        match self {
            MediaType::Image => "WhatsApp Image Keys",
            MediaType::Video => "WhatsApp Video Keys",
            MediaType::Audio => "WhatsApp Audio Keys",
            MediaType::Document => "WhatsApp Document Keys",
            MediaType::History => "WhatsApp History Keys",
            MediaType::AppState => "WhatsApp App State Keys",
            MediaType::Sticker => "WhatsApp Image Keys",
            MediaType::StickerPack => "WhatsApp Sticker Pack Keys",
            MediaType::StickerPackThumbnail => "WhatsApp Sticker Pack Thumbnail Keys",
            MediaType::LinkThumbnail => "WhatsApp Link Thumbnail Keys",
            // Unencrypted: app_info unused, but keep a value for the type system.
            MediaType::ProductCatalogImage => "WhatsApp Image Keys",
        }
    }

    /// Media type string for MMS path construction.
    /// Matches WAWebMmsMediaTypes and ClientFormatHashUrl.js path mapping.
    pub fn mms_type(&self) -> &'static str {
        match self {
            MediaType::Image | MediaType::Sticker => "image",
            MediaType::Video => "video",
            MediaType::Audio => "audio",
            MediaType::Document => "document",
            MediaType::History => "md-msg-hist",
            MediaType::AppState => "md-app-state",
            MediaType::StickerPack => "sticker-pack",
            MediaType::StickerPackThumbnail => "thumbnail-sticker-pack",
            MediaType::LinkThumbnail => "thumbnail-link",
            MediaType::ProductCatalogImage => "product-catalog-image",
        }
    }

    /// URL path prefix for upload/download.
    pub fn upload_path(&self) -> &'static str {
        match self {
            MediaType::Image | MediaType::Sticker => "/mms/image",
            MediaType::Video => "/mms/video",
            MediaType::Audio => "/mms/audio",
            MediaType::Document => "/mms/document",
            MediaType::History => "/mms/md-msg-hist",
            MediaType::AppState => "/mms/md-app-state",
            MediaType::StickerPack => "/mms/sticker-pack",
            MediaType::StickerPackThumbnail => "/mms/thumbnail-sticker-pack",
            MediaType::LinkThumbnail => "/mms/thumbnail-link",
            MediaType::ProductCatalogImage => "/product/image",
        }
    }

    /// Whether this media type is encrypted (E2E).
    /// Product catalog images are unencrypted per WA Web (CreateMediaKeys.js:75-76).
    pub fn is_encrypted(&self) -> bool {
        !matches!(self, MediaType::ProductCatalogImage)
    }
}

/// Describes how downloaded media bytes should be processed after HTTP fetch.
///
/// Mirrors WhatsApp Web's `isMediaCryptoExpectedForMediaType()` pattern:
/// encrypted (E2EE) media requires AES-256-CBC decryption + HMAC verification,
/// while unencrypted media (newsletters/channels) only needs SHA-256 validation.
#[derive(Debug, Clone)]
pub enum MediaDecryption {
    /// E2E encrypted media: decrypt with AES-256-CBC using HKDF-expanded
    /// keys from the media key, then verify HMAC-SHA256 integrity.
    Encrypted {
        media_key: Vec<u8>,
        media_type: MediaType,
    },
    /// Unencrypted media (newsletter/channel): verify SHA-256 hash of
    /// the raw downloaded bytes. No decryption needed.
    Plaintext { file_sha256: Vec<u8> },
}

pub trait Downloadable: Sync + Send {
    fn direct_path(&self) -> Option<&str>;
    fn media_key(&self) -> Option<&[u8]>;
    fn file_enc_sha256(&self) -> Option<&[u8]>;
    fn file_sha256(&self) -> Option<&[u8]>;
    fn file_length(&self) -> Option<u64>;
    fn app_info(&self) -> MediaType;

    /// Static CDN URL for direct download, bypassing host construction.
    /// Present on some message types (ImageMessage, VideoMessage) when
    /// sent in newsletter/channel chats.
    fn static_url(&self) -> Option<&str> {
        None
    }

    /// Whether this media requires decryption.
    /// Returns `true` if `media_key` is present (E2EE media),
    /// `false` otherwise (newsletter/channel media).
    fn is_encrypted(&self) -> bool {
        self.media_key().is_some()
    }
}

macro_rules! impl_downloadable {
    (@common $file_length_field:ident, $media_type:expr) => {
        fn direct_path(&self) -> Option<&str> {
            self.direct_path.as_deref()
        }

        fn media_key(&self) -> Option<&[u8]> {
            self.media_key.as_deref()
        }

        fn file_enc_sha256(&self) -> Option<&[u8]> {
            self.file_enc_sha256.as_deref()
        }

        fn file_sha256(&self) -> Option<&[u8]> {
            self.file_sha256.as_deref()
        }

        fn file_length(&self) -> Option<u64> {
            self.$file_length_field
        }

        fn app_info(&self) -> MediaType {
            $media_type
        }
    };
    ($type:ty, $media_type:expr, $file_length_field:ident) => {
        impl Downloadable for $type {
            impl_downloadable!(@common $file_length_field, $media_type);
        }
    };
    ($type:ty, $media_type:expr, $file_length_field:ident, static_url) => {
        impl Downloadable for $type {
            impl_downloadable!(@common $file_length_field, $media_type);

            fn static_url(&self) -> Option<&str> {
                self.static_url.as_deref()
            }
        }
    };
}

impl_downloadable!(
    wa::message::ImageMessage,
    MediaType::Image,
    file_length,
    static_url
);
impl_downloadable!(
    wa::message::VideoMessage,
    MediaType::Video,
    file_length,
    static_url
);
impl_downloadable!(
    wa::message::DocumentMessage,
    MediaType::Document,
    file_length
);
impl_downloadable!(wa::message::AudioMessage, MediaType::Audio, file_length);
impl_downloadable!(wa::message::StickerMessage, MediaType::Sticker, file_length);
impl_downloadable!(
    wa::message::StickerPackMessage,
    MediaType::StickerPack,
    file_length
);
impl_downloadable!(ExternalBlobReference, MediaType::AppState, file_size_bytes);
impl_downloadable!(HistorySyncNotification, MediaType::History, file_length);

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub decryption: MediaDecryption,
}

#[derive(Debug, Clone)]
pub struct MediaHost {
    pub hostname: String,
}

impl MediaHost {
    pub fn new(hostname: impl Into<String>) -> Self {
        Self {
            hostname: hostname.into(),
        }
    }
}

/// CDN hosts the official client carries in its binary-protocol token
/// dictionary, primary first. Only a convenience for callers with no session to
/// ask for a route: a live session must still take its hosts from the server,
/// which is what lets a test harness point downloads at itself.
pub const DEFAULT_MEDIA_HOSTS: [&str; 2] = ["mmg.whatsapp.net", "mmg-fallback.whatsapp.net"];

/// Where a media download is fetched from: the CDN hosts to try, in order, plus
/// the media auth token when the caller has a session to get one from.
///
/// `auth` is optional because the CDN gates a download on the signed
/// `direct_path` and its hash token, not on the session token: WA Web's own
/// download URL builder attaches no auth parameter at all, while its upload URL
/// builder does. A caller already holding the decryption references can name
/// the hosts itself and download with no session behind it.
#[derive(Clone, Default)]
pub struct MediaRoute {
    pub hosts: Vec<MediaHost>,
    pub auth: Option<String>,
}

/// Hand-written so the auth token, a live session credential, cannot reach a log
/// through a `{:?}` or a tracing field that captured a route.
impl std::fmt::Debug for MediaRoute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaRoute")
            .field("hosts", &self.hosts)
            .field("auth", &self.auth.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl MediaRoute {
    /// Route through server-provided hosts, carrying the session's media auth
    /// token.
    pub fn authenticated(hosts: Vec<MediaHost>, auth: String) -> Self {
        Self {
            hosts,
            auth: Some(auth),
        }
    }

    /// Route through caller-provided hosts, with no auth token.
    pub fn unauthenticated(hosts: Vec<MediaHost>) -> Self {
        Self { hosts, auth: None }
    }

    /// The same hosts without the auth token.
    ///
    /// A token outlives its session by less than the references it was fetched
    /// alongside do, and the CDN does not ask for one on download, so a route
    /// kept past disconnection is better off dropping it than sending a stale
    /// one and reading the refusal as an expired reference.
    pub fn without_auth(mut self) -> Self {
        self.auth = None;
        self
    }

    /// [`Self::unauthenticated`] over [`DEFAULT_MEDIA_HOSTS`].
    pub fn default_hosts() -> Self {
        Self::unauthenticated(
            DEFAULT_MEDIA_HOSTS
                .iter()
                .copied()
                .map(MediaHost::new)
                .collect(),
        )
    }
}

/// A sink a media download can be streamed into.
///
/// [`Self::truncate`] is the entire reason this is not simply `Write + Seek`.
/// Media carries a single MAC over the whole ciphertext, so
/// [`DownloadUtils::decrypt_stream_to_writer`] has necessarily written plaintext
/// by the time it can tell the payload was forged, and the retry that follows may
/// write fewer bytes than the attempt it replaces. Rewinding alone would then
/// leave verified media followed by a tail of stale bytes from the failed host.
/// `Write + Seek` cannot express the fix: shortening a sink lives on concrete
/// types (`File::set_len`, `Vec::truncate`), not on any std trait.
///
/// Implementations are provided for the sinks a download realistically targets —
/// [`std::fs::File`], an in-memory [`std::io::Cursor`], and the `BufWriter` and
/// `&mut` wrappers around them.
pub trait DownloadWriter: std::io::Write + std::io::Seek {
    /// Shorten the sink to `len` bytes, discarding anything beyond it.
    ///
    /// Only ever called with a length the sink already reaches, so an
    /// implementation never has to decide what extending would mean.
    fn truncate(&mut self, len: u64) -> std::io::Result<()>;
}

impl DownloadWriter for std::fs::File {
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        self.set_len(len)
    }
}

/// Saturating rather than fallible: `Vec::truncate` past the end is a no-op, which
/// is exactly the right answer when the length cannot be represented locally.
fn truncate_vec(buf: &mut Vec<u8>, len: u64) {
    buf.truncate(usize::try_from(len).unwrap_or(usize::MAX));
}

impl DownloadWriter for std::io::Cursor<Vec<u8>> {
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        truncate_vec(self.get_mut(), len);
        Ok(())
    }
}

impl DownloadWriter for std::io::Cursor<&mut Vec<u8>> {
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        truncate_vec(self.get_mut(), len);
        Ok(())
    }
}

/// Buffered bytes are part of the sink's length, so they have to reach it before
/// it can be measured against `len`.
impl<W: DownloadWriter> DownloadWriter for std::io::BufWriter<W> {
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        std::io::Write::flush(self)?;
        self.get_mut().truncate(len)
    }
}

impl<W: DownloadWriter + ?Sized> DownloadWriter for &mut W {
    fn truncate(&mut self, len: u64) -> std::io::Result<()> {
        (**self).truncate(len)
    }
}

/// Decrypt `buf` — a whole number of AES blocks — in place, advancing the CBC
/// chaining state held by `cbc`.
///
/// In place so a streaming decrypt can hand the writer one contiguous batch
/// instead of a 16-byte array per block; the ciphertext is already MAC'd by the
/// time this overwrites it. Going through the mode rather than block-at-a-time
/// also lets it use the AES backend's parallel-block path, which decrypts
/// several blocks per pipeline pass.
fn cbc_decrypt_blocks(cbc: &mut cbc::Decryptor<aes::Aes256>, buf: &mut [u8]) {
    use aes::cipher::{Block, BlockModeDecrypt};
    let (blocks, rest) = Block::<aes::Aes256>::slice_as_chunks_mut(buf);
    debug_assert!(rest.is_empty(), "batches are whole AES blocks");
    cbc.decrypt_blocks(blocks);
}

pub struct DownloadUtils;

impl DownloadUtils {
    pub fn prepare_download_requests(
        downloadable: &dyn Downloadable,
        route: &MediaRoute,
    ) -> Result<Vec<DownloadRequest>> {
        let is_encrypted = downloadable.is_encrypted();
        let media_type = downloadable.app_info();

        let decryption = if is_encrypted {
            let media_key = downloadable
                .media_key()
                .ok_or_else(|| anyhow!("Missing media_key for encrypted media"))?
                .to_vec();
            MediaDecryption::Encrypted {
                media_key,
                media_type,
            }
        } else {
            let file_sha256 = downloadable
                .file_sha256()
                .ok_or_else(|| anyhow!("Missing file_sha256 for unencrypted media"))?
                .to_vec();
            MediaDecryption::Plaintext { file_sha256 }
        };

        // Static URL: use directly without host construction.
        // WhatsApp Web uses staticUrl for newsletter CDN media.
        if let Some(static_url) = downloadable.static_url() {
            return Ok(vec![DownloadRequest {
                url: static_url.to_string(),
                decryption,
            }]);
        }

        let direct_path = downloadable
            .direct_path()
            .ok_or_else(|| anyhow!("Missing direct_path"))?;

        // Encrypted media uses file_enc_sha256 as URL token,
        // unencrypted (newsletter) uses file_sha256 instead.
        let token = if is_encrypted {
            let hash = downloadable
                .file_enc_sha256()
                .ok_or_else(|| anyhow!("Missing file_enc_sha256"))?;
            BASE64_URL_SAFE_NO_PAD.encode(hash)
        } else {
            let hash = downloadable
                .file_sha256()
                .ok_or_else(|| anyhow!("Missing file_sha256 for unencrypted media"))?;
            BASE64_URL_SAFE_NO_PAD.encode(hash)
        };

        let requests = route
            .hosts
            .iter()
            .map(|host| DownloadRequest {
                url: match route.auth.as_deref() {
                    Some(auth) => format!(
                        "https://{}{direct_path}?auth={auth}&token={token}",
                        host.hostname,
                    ),
                    None => format!("https://{}{direct_path}?token={token}", host.hostname),
                },
                decryption: decryption.clone(),
            })
            .collect();

        Ok(requests)
    }

    /// Validate SHA-256 hash of plaintext (unencrypted) media data.
    ///
    /// Used for newsletter/channel media which is not encrypted but
    /// still needs integrity verification (matches WhatsApp Web's
    /// `validateFilehash()` call for unencrypted downloads).
    pub fn validate_plaintext_sha256(data: &[u8], expected_sha256: &[u8]) -> Result<()> {
        use sha2::Digest;
        let actual = Sha256::digest(data);
        if actual.as_slice() != expected_sha256 {
            return Err(anyhow!(
                "SHA-256 mismatch for plaintext media: expected {}, got {}",
                hex::encode(expected_sha256),
                hex::encode(actual),
            ));
        }
        Ok(())
    }

    /// Stream plaintext (unencrypted) media to a writer while computing and
    /// validating the SHA-256 hash. Returns the number of bytes written.
    ///
    /// On hash mismatch, data has already been written to the writer;
    /// callers should discard writer contents on error.
    pub fn copy_and_validate_plaintext_to_writer<R: std::io::Read, W: std::io::Write>(
        mut reader: R,
        expected_sha256: &[u8],
        writer: &mut W,
    ) -> Result<u64> {
        use sha2::Digest;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; 8 * 1024];
        let mut total: u64 = 0;
        loop {
            let n = reader.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            writer.write_all(&buf[..n])?;
            total += n as u64;
        }
        let actual = hasher.finalize();
        if actual.as_slice() != expected_sha256 {
            return Err(anyhow!("SHA-256 mismatch for plaintext media"));
        }
        Ok(total)
    }

    /// Decrypt a media stream, writing plaintext chunks to the given writer.
    ///
    /// Reads encrypted data in 8KB chunks from `reader`, decrypts with AES-256-CBC,
    /// verifies HMAC-SHA256 integrity, and writes decrypted plaintext to `writer`.
    /// Returns the number of plaintext bytes written.
    ///
    /// If MAC verification fails, an error is returned. Note that some data may
    /// already have been written to `writer` before the MAC is checked (the MAC
    /// covers the last 10 bytes of the stream). Callers should discard the writer
    /// contents on error.
    pub fn decrypt_stream_to_writer<R: std::io::Read, W: std::io::Write>(
        mut reader: R,
        media_key: &[u8],
        app_info: MediaType,
        writer: &mut W,
    ) -> Result<u64> {
        use aes::Aes256;
        use aes::cipher::{KeyInit, KeyIvInit};

        let (iv, cipher_key, mac_key) = Self::get_media_keys(media_key, app_info)?;

        let mut hmac = <Hmac<Sha256> as KeyInit>::new_from_slice(&mac_key)
            .map_err(|_| anyhow!("Failed to init HMAC"))?;
        hmac.update(&iv);

        // Holds the AES key schedule and the CBC chaining block across refills.
        let mut cbc = cbc::Decryptor::<Aes256>::new(&cipher_key.into(), &iv.into());

        let mut bytes_written: u64 = 0;

        // Single buffer: refilled from `reader` after the retained bytes, then
        // decrypted in place so the whole batch reaches `writer` in one call.
        // It replaces the separate read buffer and `tail`/`final_plain` vectors,
        // and is kept to exactly `STREAM_CHUNK_SIZE` rather than that plus the
        // carry, so the stack this holds on an embedded target is no more than
        // the read buffer alone used to be. A refill after a carry just reads
        // that much less; the carry is at most 41 bytes, so a whole number of
        // blocks is always processable and the loop always advances.
        let mut buf = [0u8; STREAM_CHUNK_SIZE];
        let mut filled = 0usize;

        loop {
            let n = reader.read(&mut buf[filled..])?;
            if n == 0 {
                break;
            }
            filled += n;

            // The trailing `WITHHELD` bytes can't be processed yet: until EOF we
            // don't know whether they are ciphertext or the final MAC.
            if filled <= WITHHELD {
                continue;
            }
            let processable = (filled - WITHHELD) - ((filled - WITHHELD) % AES_BLOCK_SIZE);
            if processable == 0 {
                continue;
            }

            // MAC covers ciphertext, so hash before decrypting over it in place.
            hmac.update(&buf[..processable]);
            cbc_decrypt_blocks(&mut cbc, &mut buf[..processable]);
            writer.write_all(&buf[..processable])?;
            bytes_written += processable as u64;

            // Compact the carry to the front. It is at most 41 bytes, so this
            // is a register-sized move rather than a shift of the whole buffer.
            buf.copy_within(processable..filled, 0);
            filled -= processable;
        }

        let tail = &mut buf[..filled];
        if tail.len() < WITHHELD || !(tail.len() - MEDIA_MAC_SIZE).is_multiple_of(AES_BLOCK_SIZE) {
            return Err(anyhow!("Invalid final media size"));
        }
        let mac_index = tail.len() - MEDIA_MAC_SIZE;
        let (final_ciphertext, mac_bytes) = tail.split_at_mut(mac_index);
        hmac.update(final_ciphertext);
        let expected_mac_full = hmac.finalize().into_bytes();
        let expected_mac = &expected_mac_full[..MEDIA_MAC_SIZE];
        if subtle::ConstantTimeEq::ct_eq(&*mac_bytes, expected_mac).unwrap_u8() == 0 {
            return Err(anyhow!("MAC mismatch"));
        }

        cbc_decrypt_blocks(&mut cbc, final_ciphertext);
        let pad_len = match final_ciphertext.last() {
            Some(&v) => v as usize,
            None => return Err(anyhow!("Empty plaintext after decrypt")),
        };
        if pad_len == 0 || pad_len > AES_BLOCK_SIZE || pad_len > final_ciphertext.len() {
            return Err(anyhow!("Invalid PKCS7 padding"));
        }
        if !final_ciphertext[final_ciphertext.len() - pad_len..]
            .iter()
            .all(|&b| b as usize == pad_len)
        {
            return Err(anyhow!("Bad PKCS7 padding bytes"));
        }
        let final_plain = &final_ciphertext[..final_ciphertext.len() - pad_len];
        writer.write_all(final_plain)?;
        bytes_written += final_plain.len() as u64;

        Ok(bytes_written)
    }

    /// Decrypt a media stream, returning the plaintext as a `Vec<u8>`.
    ///
    /// This is a convenience wrapper around [`decrypt_stream_to_writer`](Self::decrypt_stream_to_writer) that
    /// accumulates output in memory.
    pub fn decrypt_stream<R: std::io::Read>(
        reader: R,
        media_key: &[u8],
        app_info: MediaType,
    ) -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        Self::decrypt_stream_to_writer(reader, media_key, app_info, &mut buf)?;
        Ok(buf)
    }

    pub fn get_media_keys(
        media_key: &[u8],
        app_info: MediaType,
    ) -> Result<([u8; 16], [u8; 32], [u8; 32])> {
        let mut expanded = [0u8; 112];
        crate::crypto::hkdf_sha256_into(
            media_key,
            None,
            app_info.app_info().as_bytes(),
            &mut expanded,
        )
        .map_err(|e| anyhow!("HKDF expand failed: {e}"))?;
        let iv: [u8; 16] = expanded[0..16]
            .try_into()
            .map_err(|_| anyhow!("HKDF output has unexpected length for IV"))?;
        let cipher_key: [u8; 32] = expanded[16..48]
            .try_into()
            .map_err(|_| anyhow!("HKDF output has unexpected length for cipher key"))?;
        let mac_key: [u8; 32] = expanded[48..80]
            .try_into()
            .map_err(|_| anyhow!("HKDF output has unexpected length for MAC key"))?;
        Ok((iv, cipher_key, mac_key))
    }

    pub fn verify_and_decrypt(
        encrypted_payload: &[u8],
        media_key: &[u8],
        media_type: MediaType,
    ) -> std::result::Result<Vec<u8>, MediaDecryptionError> {
        let mut output = encrypted_payload.to_vec();
        Self::verify_and_decrypt_in_place(&mut output, media_key, media_type)?;
        Ok(output)
    }

    /// Authenticate and decrypt an encrypted media payload in its existing
    /// allocation. This is the zero-copy counterpart to [`Self::verify_and_decrypt`]
    /// for buffered HTTP clients: the trailing MAC and PKCS#7 padding are removed
    /// by truncating the input vector after CBC decryption.
    ///
    /// Authentication is completed before any byte is mutated, so an invalid
    /// MAC leaves `encrypted_payload` unchanged. Errors after successful
    /// authentication, such as malformed padding, may leave the buffer
    /// truncated or partially decrypted.
    pub fn verify_and_decrypt_in_place(
        encrypted_payload: &mut Vec<u8>,
        media_key: &[u8],
        media_type: MediaType,
    ) -> std::result::Result<(), MediaDecryptionError> {
        if encrypted_payload.len() <= MEDIA_MAC_SIZE {
            return Err(MediaDecryptionError::PayloadTooShort);
        }

        let ciphertext_len = encrypted_payload.len() - MEDIA_MAC_SIZE;
        let received_mac: [u8; MEDIA_MAC_SIZE] = encrypted_payload[ciphertext_len..]
            .try_into()
            .map_err(|_| MediaDecryptionError::PayloadTooShort)?;
        let (iv, cipher_key, mac_key) = Self::get_media_keys(media_key, media_type)?;

        let computed_mac_full =
            hmac_sha256_two_part(&mac_key, &iv, &encrypted_payload[..ciphertext_len]);
        if subtle::ConstantTimeEq::ct_eq(&computed_mac_full[..MEDIA_MAC_SIZE], &received_mac)
            .unwrap_u8()
            == 0
        {
            return Err(MediaDecryptionError::InvalidMac);
        }
        if ciphertext_len == 0 || !ciphertext_len.is_multiple_of(AES_BLOCK_SIZE) {
            return Err(MediaDecryptionError::Decryption(
                AesCbcDecryptionError::BadCiphertext("invalid ciphertext length"),
            ));
        }

        encrypted_payload.truncate(ciphertext_len);
        aes_256_cbc_decrypt_in_place(encrypted_payload, &cipher_key, &iv)
            .map_err(MediaDecryptionError::Decryption)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDownloadable {
        direct_path: Option<String>,
        static_url: Option<String>,
        media_key: Option<Vec<u8>>,
        file_sha256: Option<Vec<u8>>,
        file_enc_sha256: Option<Vec<u8>>,
        media_type: MediaType,
    }

    impl Downloadable for MockDownloadable {
        fn direct_path(&self) -> Option<&str> {
            self.direct_path.as_deref()
        }
        fn media_key(&self) -> Option<&[u8]> {
            self.media_key.as_deref()
        }
        fn file_enc_sha256(&self) -> Option<&[u8]> {
            self.file_enc_sha256.as_deref()
        }
        fn file_sha256(&self) -> Option<&[u8]> {
            self.file_sha256.as_deref()
        }
        fn file_length(&self) -> Option<u64> {
            Some(1024)
        }
        fn app_info(&self) -> MediaType {
            self.media_type
        }
        fn static_url(&self) -> Option<&str> {
            self.static_url.as_deref()
        }
    }

    fn mock_hosts() -> Vec<MediaHost> {
        vec![
            MediaHost::new("cdn1.example.com"),
            MediaHost::new("cdn2.example.com"),
        ]
    }

    fn authenticated_route() -> MediaRoute {
        MediaRoute::authenticated(mock_hosts(), "test-auth-token".into())
    }

    /// Every variant. The exhaustive match in
    /// `every_media_type_builds_urls_for_both_route_kinds` is what forces a new
    /// one to be named here instead of silently skipping the URL assertions.
    const ALL_MEDIA_TYPES: [MediaType; 11] = [
        MediaType::Image,
        MediaType::Video,
        MediaType::Audio,
        MediaType::Document,
        MediaType::History,
        MediaType::AppState,
        MediaType::Sticker,
        MediaType::StickerPack,
        MediaType::StickerPackThumbnail,
        MediaType::LinkThumbnail,
        MediaType::ProductCatalogImage,
    ];

    fn encrypted_media_fixture(
        plaintext: &[u8],
        media_key: &[u8],
        media_type: MediaType,
    ) -> Vec<u8> {
        use crate::libsignal::crypto::{CryptographicMac, aes_256_cbc_encrypt_into};

        let (iv, cipher_key, mac_key) =
            DownloadUtils::get_media_keys(media_key, media_type).unwrap();
        let mut payload = Vec::new();
        aes_256_cbc_encrypt_into(plaintext, &cipher_key, &iv, &mut payload).unwrap();
        let mut mac = CryptographicMac::new("HmacSha256", &mac_key).unwrap();
        mac.update(&iv);
        mac.update(&payload);
        payload.extend_from_slice(&mac.finalize()[..MEDIA_MAC_SIZE]);
        payload
    }

    #[test]
    fn in_place_media_decrypt_matches_streaming_without_reallocating() {
        let media_key = [0x42; 32];
        for plaintext_len in [0_usize, 1, 15, 16, 17, 8 * 1024 + 7, 128 * 1024 + 3] {
            let plaintext: Vec<u8> = (0..plaintext_len)
                .map(|index| index.wrapping_mul(31) as u8)
                .collect();
            let encrypted = encrypted_media_fixture(&plaintext, &media_key, MediaType::History);
            let streaming = DownloadUtils::decrypt_stream(
                std::io::Cursor::new(&encrypted),
                &media_key,
                MediaType::History,
            )
            .unwrap();

            let mut in_place = encrypted;
            let allocation = in_place.as_ptr();
            let capacity = in_place.capacity();
            DownloadUtils::verify_and_decrypt_in_place(
                &mut in_place,
                &media_key,
                MediaType::History,
            )
            .unwrap();

            assert_eq!(in_place, plaintext, "plaintext length {plaintext_len}");
            assert_eq!(in_place, streaming, "plaintext length {plaintext_len}");
            assert_eq!(in_place.as_ptr(), allocation, "allocation must be reused");
            assert_eq!(in_place.capacity(), capacity, "capacity must be reused");
        }
    }

    /// A `Read` that never returns more than `step` bytes, so the streaming
    /// decryptor's carry/compaction path sees non-block-aligned refills.
    struct ThrottledReader<'a> {
        data: &'a [u8],
        step: usize,
    }

    impl std::io::Read for ThrottledReader<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let n = self.step.min(out.len()).min(self.data.len());
            out[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    /// A `Write` that records how many calls it received and how much they carried.
    #[derive(Default)]
    struct CountingWriter {
        writes: usize,
        bytes: Vec<u8>,
    }

    impl std::io::Write for CountingWriter {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.writes += 1;
            self.bytes.extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn streaming_decrypt_is_independent_of_reader_chunking() {
        // The refill loop withholds a MAC + one block and only ever processes a
        // block-aligned prefix, so odd read sizes must not shift a single byte.
        let media_key = [0x5B; 32];
        let plaintext: Vec<u8> = (0..70_000_usize)
            .map(|index| index.wrapping_mul(17) as u8)
            .collect();
        let encrypted = encrypted_media_fixture(&plaintext, &media_key, MediaType::Video);

        for step in [1_usize, 7, 15, 16, 17, 26, 4096, 8192, 8193, 100_000] {
            let out = DownloadUtils::decrypt_stream(
                ThrottledReader {
                    data: &encrypted,
                    step,
                },
                &media_key,
                MediaType::Video,
            )
            .unwrap();
            assert_eq!(out, plaintext, "reader step {step}");
        }
    }

    #[test]
    fn streaming_decrypt_writes_whole_batches_not_single_blocks() {
        // Pins the batching: an unbuffered writer (a `File`) pays one syscall
        // per write, so a 16-byte-per-block loop would be ~64k writes here.
        let media_key = [0x6C; 32];
        let plaintext = vec![0xA7u8; 1024 * 1024];
        let encrypted = encrypted_media_fixture(&plaintext, &media_key, MediaType::Document);

        let mut writer = CountingWriter::default();
        DownloadUtils::decrypt_stream_to_writer(
            std::io::Cursor::new(&encrypted),
            &media_key,
            MediaType::Document,
            &mut writer,
        )
        .unwrap();

        assert_eq!(writer.bytes, plaintext);
        let blocks = plaintext.len() / AES_BLOCK_SIZE;
        let max_writes = encrypted.len().div_ceil(STREAM_CHUNK_SIZE) + 2;
        assert!(
            writer.writes <= max_writes,
            "expected at most {max_writes} batched writes for {blocks} blocks, got {}",
            writer.writes
        );
    }

    #[test]
    fn in_place_media_decrypt_authenticates_before_mutating() {
        let media_key = [0x24; 32];
        let mut encrypted =
            encrypted_media_fixture(b"authenticated payload", &media_key, MediaType::History);
        let last = encrypted.len() - 1;
        encrypted[last] ^= 1;
        let original = encrypted.clone();

        assert!(matches!(
            DownloadUtils::verify_and_decrypt_in_place(
                &mut encrypted,
                &media_key,
                MediaType::History,
            ),
            Err(MediaDecryptionError::InvalidMac)
        ));
        assert_eq!(encrypted, original);
    }

    #[test]
    fn prepare_requests_encrypted() {
        let d = MockDownloadable {
            direct_path: Some("/v/t1/media.enc".into()),
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let reqs = DownloadUtils::prepare_download_requests(&d, &authenticated_route()).unwrap();
        assert_eq!(reqs.len(), 2);
        assert!(matches!(
            &reqs[0].decryption,
            MediaDecryption::Encrypted { media_type, .. } if *media_type == MediaType::Image
        ));
        let expected_token = BASE64_URL_SAFE_NO_PAD.encode([3u8; 32]);
        assert!(reqs[0].url.contains(&expected_token));
        assert!(reqs[0].url.starts_with("https://cdn1.example.com"));
        assert!(reqs[1].url.starts_with("https://cdn2.example.com"));
    }

    // The authenticated URL is the one real servers already accept, so it is
    // pinned literally: making `auth` optional must not move a single byte of it.
    #[test]
    fn authenticated_url_keeps_its_exact_shape() {
        let d = MockDownloadable {
            direct_path: Some("/v/t1/media.enc".into()),
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let reqs = DownloadUtils::prepare_download_requests(&d, &authenticated_route()).unwrap();
        let token = BASE64_URL_SAFE_NO_PAD.encode([3u8; 32]);
        assert_eq!(
            reqs[0].url,
            format!("https://cdn1.example.com/v/t1/media.enc?auth=test-auth-token&token={token}")
        );
        assert_eq!(
            reqs[1].url,
            format!("https://cdn2.example.com/v/t1/media.enc?auth=test-auth-token&token={token}")
        );
    }

    #[test]
    fn unauthenticated_urls_omit_auth_and_cover_every_host_in_order() {
        let d = MockDownloadable {
            direct_path: Some("/v/t1/media.enc".into()),
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let route = MediaRoute::unauthenticated(mock_hosts());
        let reqs = DownloadUtils::prepare_download_requests(&d, &route).unwrap();
        let token = BASE64_URL_SAFE_NO_PAD.encode([3u8; 32]);
        assert_eq!(
            reqs.iter().map(|r| r.url.as_str()).collect::<Vec<_>>(),
            vec![
                format!("https://cdn1.example.com/v/t1/media.enc?token={token}"),
                format!("https://cdn2.example.com/v/t1/media.enc?token={token}"),
            ],
        );
        assert!(reqs.iter().all(|r| !r.url.contains("auth=")));
    }

    #[test]
    fn without_auth_keeps_the_hosts_and_drops_the_token() {
        let route = authenticated_route().without_auth();
        assert!(route.auth.is_none());
        assert_eq!(
            route
                .hosts
                .iter()
                .map(|h| h.hostname.as_str())
                .collect::<Vec<_>>(),
            vec!["cdn1.example.com", "cdn2.example.com"],
        );
    }

    #[test]
    fn route_debug_redacts_the_auth_token() {
        let rendered = format!("{:?}", authenticated_route());
        assert!(!rendered.contains("test-auth-token"), "{rendered}");
        assert!(rendered.contains("cdn1.example.com"), "{rendered}");

        let rendered = format!("{:?}", MediaRoute::unauthenticated(mock_hosts()));
        assert!(rendered.contains("auth: None"), "{rendered}");
    }

    #[test]
    fn default_route_uses_the_known_cdn_hosts_without_auth() {
        let route = MediaRoute::default_hosts();
        assert!(route.auth.is_none());
        assert_eq!(
            route
                .hosts
                .iter()
                .map(|h| h.hostname.as_str())
                .collect::<Vec<_>>(),
            vec!["mmg.whatsapp.net", "mmg-fallback.whatsapp.net"],
        );
    }

    #[test]
    fn every_media_type_builds_urls_for_both_route_kinds() {
        for media_type in ALL_MEDIA_TYPES {
            // No wildcard arm: a new variant stops compiling here until it is
            // added to ALL_MEDIA_TYPES, which is the only thing that makes the
            // array's length a guarantee rather than a hand-kept count.
            match media_type {
                MediaType::Image
                | MediaType::Video
                | MediaType::Audio
                | MediaType::Document
                | MediaType::History
                | MediaType::AppState
                | MediaType::Sticker
                | MediaType::StickerPack
                | MediaType::StickerPackThumbnail
                | MediaType::LinkThumbnail
                | MediaType::ProductCatalogImage => {}
            }

            let d = MockDownloadable {
                direct_path: Some("/v/t1/media.enc".into()),
                static_url: None,
                media_key: Some(vec![1; 32]),
                file_sha256: Some(vec![2; 32]),
                file_enc_sha256: Some(vec![3; 32]),
                media_type,
            };
            let token = BASE64_URL_SAFE_NO_PAD.encode([3u8; 32]);

            let authenticated =
                DownloadUtils::prepare_download_requests(&d, &authenticated_route()).unwrap();
            assert_eq!(
                authenticated[0].url,
                format!(
                    "https://cdn1.example.com/v/t1/media.enc?auth=test-auth-token&token={token}"
                ),
                "{media_type:?}"
            );

            let unauthenticated = DownloadUtils::prepare_download_requests(
                &d,
                &MediaRoute::unauthenticated(mock_hosts()),
            )
            .unwrap();
            assert_eq!(
                unauthenticated[0].url,
                format!("https://cdn1.example.com/v/t1/media.enc?token={token}"),
                "{media_type:?}"
            );
        }
    }

    #[test]
    fn route_without_hosts_yields_no_requests() {
        let d = MockDownloadable {
            direct_path: Some("/v/t1/media.enc".into()),
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let reqs =
            DownloadUtils::prepare_download_requests(&d, &MediaRoute::unauthenticated(Vec::new()))
                .unwrap();
        assert!(reqs.is_empty());
    }

    #[test]
    fn prepare_requests_plaintext_newsletter() {
        let d = MockDownloadable {
            direct_path: Some("/newsletter/newsletter-image/abc".into()),
            static_url: None,
            media_key: None,
            file_sha256: Some(vec![4; 32]),
            file_enc_sha256: None,
            media_type: MediaType::Image,
        };
        let reqs = DownloadUtils::prepare_download_requests(&d, &authenticated_route()).unwrap();
        assert_eq!(reqs.len(), 2);
        assert!(matches!(
            &reqs[0].decryption,
            MediaDecryption::Plaintext { file_sha256 } if file_sha256 == &vec![4u8; 32]
        ));
        // Token should be base64url of file_sha256 (not file_enc_sha256)
        let expected_token = BASE64_URL_SAFE_NO_PAD.encode([4u8; 32]);
        assert!(reqs[0].url.contains(&expected_token));
    }

    #[test]
    fn prepare_requests_static_url() {
        let d = MockDownloadable {
            direct_path: Some("/unused".into()),
            static_url: Some("https://static.cdn.example.com/media/abc123".into()),
            media_key: None,
            file_sha256: Some(vec![5; 32]),
            file_enc_sha256: None,
            media_type: MediaType::Image,
        };
        let reqs = DownloadUtils::prepare_download_requests(&d, &authenticated_route()).unwrap();
        // Static URL bypasses host construction → single request
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].url, "https://static.cdn.example.com/media/abc123");
        assert!(matches!(
            &reqs[0].decryption,
            MediaDecryption::Plaintext { .. }
        ));
    }

    #[test]
    fn prepare_requests_static_url_needs_no_hosts() {
        let d = MockDownloadable {
            direct_path: Some("/unused".into()),
            static_url: Some("https://static.cdn.example.com/media/abc123".into()),
            media_key: None,
            file_sha256: Some(vec![5; 32]),
            file_enc_sha256: None,
            media_type: MediaType::Image,
        };
        let reqs =
            DownloadUtils::prepare_download_requests(&d, &MediaRoute::unauthenticated(Vec::new()))
                .unwrap();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].url, "https://static.cdn.example.com/media/abc123");
    }

    #[test]
    fn prepare_requests_missing_direct_path_no_static_url() {
        let d = MockDownloadable {
            direct_path: None,
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let err = DownloadUtils::prepare_download_requests(&d, &authenticated_route()).unwrap_err();
        assert!(err.to_string().contains("Missing direct_path"));
    }

    #[test]
    fn validate_plaintext_sha256_ok() {
        use sha2::Digest;
        let data = b"test newsletter media content";
        let hash = Sha256::digest(data);
        assert!(DownloadUtils::validate_plaintext_sha256(data, hash.as_slice()).is_ok());
    }

    #[test]
    fn validate_plaintext_sha256_mismatch() {
        let data = b"test newsletter media content";
        let wrong_hash = vec![0u8; 32];
        let err = DownloadUtils::validate_plaintext_sha256(data, &wrong_hash).unwrap_err();
        assert!(err.to_string().contains("SHA-256 mismatch"));
    }

    #[test]
    fn copy_and_validate_plaintext_ok() {
        use sha2::Digest;
        use std::io::Cursor;
        let data = b"streaming newsletter content";
        let hash = Sha256::digest(data);
        let reader = Cursor::new(data.to_vec());
        let mut writer = Vec::new();
        let bytes = DownloadUtils::copy_and_validate_plaintext_to_writer(
            reader,
            hash.as_slice(),
            &mut writer,
        )
        .unwrap();
        assert_eq!(bytes, data.len() as u64);
        assert_eq!(writer, data);
    }

    #[test]
    fn copy_and_validate_plaintext_mismatch() {
        use std::io::Cursor;
        let data = b"streaming newsletter content";
        let wrong_hash = vec![0u8; 32];
        let reader = Cursor::new(data.to_vec());
        let mut writer = Vec::new();
        let err =
            DownloadUtils::copy_and_validate_plaintext_to_writer(reader, &wrong_hash, &mut writer)
                .unwrap_err();
        assert!(err.to_string().contains("SHA-256 mismatch"));
    }

    #[test]
    fn media_decryption_decryption_preserves_aes_cbc_source() {
        let inner = AesCbcDecryptionError::BadKeyOrIv;
        let mde = MediaDecryptionError::Decryption(inner);
        let src = std::error::Error::source(&mde).expect("source preserved");
        let cbc = src
            .downcast_ref::<AesCbcDecryptionError>()
            .expect("downcasts to AesCbcDecryptionError");
        assert!(matches!(cbc, AesCbcDecryptionError::BadKeyOrIv));
    }

    #[test]
    fn media_decryption_mac_preserves_crypto_error_source() {
        let inner = CryptoError::UnknownAlgorithm("MAC", "BogusAlg".into());
        let mde = MediaDecryptionError::Mac(inner);
        let src = std::error::Error::source(&mde).expect("source preserved");
        let ce = src
            .downcast_ref::<CryptoError>()
            .expect("downcasts to CryptoError");
        assert!(matches!(ce, CryptoError::UnknownAlgorithm("MAC", _)));
    }
}
