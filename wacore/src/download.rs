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
use wacore_binary::{Jid, JidExt};
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
    #[error("SHA-256 mismatch for encrypted media bytes")]
    EncryptedSha256Mismatch,
    #[error("SHA-256 mismatch for decrypted media bytes")]
    PlaintextSha256Mismatch,
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
    /// Opt-in group-history bundle shared on direct member adds.
    /// WA Web derives its media keys under the `Group History` HKDF context.
    GroupHistory,
    MusicArtwork,
    NewsletterMusicArtwork,
}

impl MediaType {
    pub fn app_info(&self) -> &'static str {
        match self {
            MediaType::Image => "WhatsApp Image Keys",
            MediaType::Video => "WhatsApp Video Keys",
            MediaType::Audio => "WhatsApp Audio Keys",
            MediaType::Document => "WhatsApp Document Keys",
            MediaType::History => "WhatsApp History Keys",
            MediaType::GroupHistory => "Group History",
            MediaType::AppState => "WhatsApp App State Keys",
            MediaType::Sticker => "WhatsApp Image Keys",
            MediaType::StickerPack => "WhatsApp Sticker Pack Keys",
            MediaType::StickerPackThumbnail => "WhatsApp Sticker Pack Thumbnail Keys",
            MediaType::LinkThumbnail => "WhatsApp Link Thumbnail Keys",
            // Unencrypted: app_info unused, but keep a value for the type system.
            MediaType::ProductCatalogImage => "WhatsApp Image Keys",
            MediaType::MusicArtwork | MediaType::NewsletterMusicArtwork => {
                "WhatsApp Music Artwork Keys"
            }
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
            MediaType::GroupHistory => "group-history",
            MediaType::AppState => "md-app-state",
            MediaType::StickerPack => "sticker-pack",
            MediaType::StickerPackThumbnail => "thumbnail-sticker-pack",
            MediaType::LinkThumbnail => "thumbnail-link",
            MediaType::ProductCatalogImage => "product-catalog-image",
            MediaType::MusicArtwork => "music-artwork",
            MediaType::NewsletterMusicArtwork => "newsletter-music-artwork",
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
            MediaType::GroupHistory => "/mms/group-history",
            MediaType::AppState => "/mms/md-app-state",
            MediaType::StickerPack => "/mms/sticker-pack",
            MediaType::StickerPackThumbnail => "/mms/thumbnail-sticker-pack",
            MediaType::LinkThumbnail => "/mms/thumbnail-link",
            MediaType::ProductCatalogImage => "/product/image",
            MediaType::MusicArtwork => "/mms/music-artwork",
            MediaType::NewsletterMusicArtwork => "/mms/newsletter-music-artwork",
        }
    }

    /// Whether this media type is encrypted (E2E).
    /// Product catalog images and newsletter artwork are unencrypted.
    pub fn is_encrypted(&self) -> bool {
        !matches!(
            self,
            MediaType::ProductCatalogImage | MediaType::NewsletterMusicArtwork
        )
    }
}

/// Describes how downloaded media bytes should be processed after HTTP fetch.
///
/// Mirrors WhatsApp Web's `isMediaCryptoExpectedForMediaType()` pattern:
/// encrypted (E2EE) media requires AES-256-CBC decryption + HMAC verification,
/// while unencrypted media (newsletters/channels) only needs SHA-256 validation.
#[derive(Clone)]
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

impl std::fmt::Debug for MediaDecryption {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encrypted { media_type, .. } => f
                .debug_struct("Encrypted")
                .field("media_key", &"<redacted>")
                .field("media_type", media_type)
                .finish(),
            Self::Plaintext { file_sha256 } => f
                .debug_struct("Plaintext")
                .field("file_sha256", file_sha256)
                .finish(),
        }
    }
}

/// Media references must be `Send + Sync` on native targets. Browser hosts may
/// implement this trait with local state such as `Rc` on wasm32.
pub trait Downloadable: crate::sync_marker::MaybeSendSync {
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
    /// Artwork requires encryption unless its newsletter media type is explicit;
    /// other media follows key presence.
    fn is_encrypted(&self) -> bool {
        match self.app_info() {
            MediaType::MusicArtwork => true,
            MediaType::NewsletterMusicArtwork => false,
            _ => self.media_key().is_some(),
        }
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

impl Downloadable for wa::EmbeddedMusic {
    fn direct_path(&self) -> Option<&str> {
        self.artwork_direct_path.as_deref()
    }

    fn media_key(&self) -> Option<&[u8]> {
        self.artwork_media_key.as_deref()
    }

    fn file_enc_sha256(&self) -> Option<&[u8]> {
        self.artwork_enc_sha256.as_deref()
    }

    fn file_sha256(&self) -> Option<&[u8]> {
        self.artwork_sha256.as_deref()
    }

    fn file_length(&self) -> Option<u64> {
        None
    }

    fn app_info(&self) -> MediaType {
        MediaType::MusicArtwork
    }
}

/// Artwork is plaintext only when its actual message chat is a newsletter.
#[derive(Debug, Clone, Copy)]
pub struct MusicArtwork<'a> {
    metadata: &'a wa::EmbeddedMusic,
    media_type: MediaType,
}

impl<'a> MusicArtwork<'a> {
    pub fn for_chat(metadata: &'a wa::EmbeddedMusic, chat: &Jid) -> Self {
        Self {
            metadata,
            media_type: if chat.is_newsletter() {
                MediaType::NewsletterMusicArtwork
            } else {
                MediaType::MusicArtwork
            },
        }
    }
}

impl Downloadable for MusicArtwork<'_> {
    fn direct_path(&self) -> Option<&str> {
        self.metadata.direct_path()
    }

    fn media_key(&self) -> Option<&[u8]> {
        self.metadata.media_key()
    }

    fn file_enc_sha256(&self) -> Option<&[u8]> {
        self.metadata.file_enc_sha256()
    }

    fn file_sha256(&self) -> Option<&[u8]> {
        self.metadata.file_sha256()
    }

    fn file_length(&self) -> Option<u64> {
        None
    }

    fn app_info(&self) -> MediaType {
        self.media_type
    }
}

/// A received group-history bundle carries the same download references as
/// other media (direct path, media key, hashes) but no declared plaintext
/// length, so only the capacity hint is absent. This lets the typed download
/// path (`prepare_download_requests` + `MediaDecryption::Encrypted` with
/// `MediaType::GroupHistory`) handle these bytes instead of each caller
/// re-deriving the `Group History` HKDF context and `/mms/group-history`
/// URL shape by hand. Building `messageHistoryBundle` and choosing
/// `historyReceivers` stays with the caller.
impl Downloadable for wa::message::MessageHistoryBundle {
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
        None
    }

    fn app_info(&self) -> MediaType {
        MediaType::GroupHistory
    }
}

#[derive(Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub decryption: MediaDecryption,
}

impl std::fmt::Debug for DownloadRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let url = fluent_uri::Uri::parse(self.url.as_str()).ok();
        f.debug_struct("DownloadRequest")
            .field(
                "host",
                &url.as_ref()
                    .and_then(|url| url.authority())
                    .map(|a| a.host()),
            )
            .field("decryption", &self.decryption)
            .finish()
    }
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

/// CDN hosts to try in order. Download authorization comes from the media
/// reference; session upload credentials are not part of a download route.
#[derive(Debug, Clone, Default)]
pub struct MediaRoute {
    pub hosts: Vec<MediaHost>,
}

impl MediaRoute {
    /// Route through server-provided or caller-provided hosts, in failover order.
    pub fn new(hosts: Vec<MediaHost>) -> Self {
        Self { hosts }
    }

    /// [`Self::new`] over [`DEFAULT_MEDIA_HOSTS`].
    pub fn default_hosts() -> Self {
        Self::new(
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

        let candidates = route.hosts.iter().map(|host| {
            use fluent_uri::{Uri, UriRef, component::Host, pct_enc::EStr};

            let base = Uri::parse(format!("https://{}/", host.hostname))
                .map_err(|_| anyhow!("Invalid media route host"))?;
            let base_authority = base
                .authority()
                .ok_or_else(|| anyhow!("Invalid media route host"))?;
            // CDN authorities must be DNS names or IP literals, not arbitrary
            // RFC reg-names that an HTTP backend might interpret differently.
            let valid_host = match base_authority.host_parsed() {
                Host::IpvFuture { .. } => false,
                Host::RegName(name) => {
                    !name.is_empty()
                        && name
                            .as_str()
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
                }
                _ => true,
            };
            if !valid_host
                || base_authority.userinfo().is_some()
                || base.path().as_str() != "/"
                || base.query().is_some()
                || base.fragment().is_some()
                || base_authority.port().is_some_and(|port| port.is_empty())
            {
                return Err(anyhow!("Invalid media route host"));
            }
            let base_port = base_authority
                .port_to_u16()
                .map_err(|_| anyhow!("Invalid media route port"))?
                .unwrap_or(443);
            // Signed CDN references are already URI-encoded. Reject malformed
            // escapes, raw whitespace and backslashes instead of repairing them.
            let url = UriRef::parse(direct_path)
                .map_err(|_| anyhow!("Invalid media direct path"))?
                .resolve_against(&base)
                .map_err(|_| anyhow!("Invalid media direct path"))?;
            let authority = url
                .authority()
                .ok_or_else(|| anyhow!("Media direct path changes the route origin"))?;
            if url.scheme() != base.scheme()
                || !authority.host().eq_ignore_ascii_case(base_authority.host())
                || authority
                    .port_to_u16()
                    .map_err(|_| anyhow!("Invalid media direct path port"))?
                    .unwrap_or(443)
                    != base_port
                || authority.userinfo().is_some()
                || authority.port().is_some_and(|port| port.is_empty())
            {
                return Err(anyhow!("Media direct path changes the route origin"));
            }
            // Preserve the signed query byte-for-byte; only our URL-safe base64
            // token is appended. The URI builder retains the fragment separately.
            let mut query = url.query().map_or("", |q| q.as_str()).to_owned();
            if !query.is_empty() {
                query.push('&');
            }
            query.push_str("token=");
            query.push_str(&token);
            let url = Uri::builder()
                .scheme(url.scheme())
                .authority(authority)
                .path(url.path())
                .query(EStr::new(&query).ok_or_else(|| anyhow!("Invalid media query"))?)
                .optional(
                    |builder, fragment| builder.fragment(fragment),
                    url.fragment(),
                )
                .build()
                .map_err(|_| anyhow!("Invalid media download URL"))?;
            Ok(DownloadRequest {
                url: url.into_string(),
                decryption: decryption.clone(),
            })
        });
        let mut requests = Vec::with_capacity(route.hosts.len());
        let mut last_error = None;
        for candidate in candidates {
            match candidate {
                Ok(request) => requests.push(request),
                Err(error) => last_error = Some(error),
            }
        }
        // A bad host must not discard a usable fallback. Keep preparation
        // errors when no candidate can be used, and preserve the empty route.
        if requests.is_empty()
            && let Some(error) = last_error
        {
            return Err(error);
        }
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
        reader: R,
        media_key: &[u8],
        app_info: MediaType,
        writer: &mut W,
    ) -> Result<u64> {
        Self::decrypt_stream_to_writer_with_hashes(reader, media_key, app_info, None, None, writer)
    }

    /// Authenticate and decrypt, checking each declared hash when present.
    /// The encrypted hash covers ciphertext plus its trailing MAC; the plaintext
    /// hash excludes padding. Uses constant memory. As with the MAC-only helper,
    /// callers must discard writer contents on any error.
    pub fn decrypt_stream_to_writer_with_hashes<R: std::io::Read, W: std::io::Write>(
        mut reader: R,
        media_key: &[u8],
        app_info: MediaType,
        expected_enc_sha256: Option<&[u8]>,
        expected_sha256: Option<&[u8]>,
        writer: &mut W,
    ) -> Result<u64> {
        use aes::Aes256;
        use aes::cipher::{KeyInit, KeyIvInit};
        use sha2::Digest;

        let mut encrypted_hasher = expected_enc_sha256.map(|_| Sha256::new());
        let mut plaintext_hasher = expected_sha256.map(|_| Sha256::new());

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
            if let Some(hasher) = &mut encrypted_hasher {
                // Only new bytes: the carry was hashed before it was moved.
                hasher.update(&buf[filled..filled + n]);
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
            if let Some(hasher) = &mut plaintext_hasher {
                hasher.update(&buf[..processable]);
            }
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
        if let Some((hasher, expected)) = encrypted_hasher.zip(expected_enc_sha256)
            && hasher.finalize().as_slice() != expected
        {
            return Err(MediaDecryptionError::EncryptedSha256Mismatch.into());
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
        if let Some(hasher) = &mut plaintext_hasher {
            hasher.update(final_plain);
        }
        writer.write_all(final_plain)?;
        bytes_written += final_plain.len() as u64;

        if let Some((hasher, expected)) = plaintext_hasher.zip(expected_sha256)
            && hasher.finalize().as_slice() != expected
        {
            return Err(MediaDecryptionError::PlaintextSha256Mismatch.into());
        }

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

    /// Authenticate and decrypt in the same allocation, then verify the declared
    /// ciphertext and plaintext hashes when present. The encrypted digest must
    /// be computed before decryption removes the MAC and overwrites ciphertext.
    /// On any error the caller must discard the buffer, which may be mutated.
    pub fn verify_and_decrypt_in_place_with_hashes(
        encrypted_payload: &mut Vec<u8>,
        media_key: &[u8],
        media_type: MediaType,
        expected_enc_sha256: Option<&[u8]>,
        expected_sha256: Option<&[u8]>,
    ) -> std::result::Result<(), MediaDecryptionError> {
        use sha2::Digest;
        let encrypted_digest = expected_enc_sha256.map(|_| Sha256::digest(&*encrypted_payload));
        Self::verify_and_decrypt_in_place(encrypted_payload, media_key, media_type)?;
        if let Some((actual, expected)) = encrypted_digest.zip(expected_enc_sha256)
            && actual.as_slice() != expected
        {
            return Err(MediaDecryptionError::EncryptedSha256Mismatch);
        }
        if let Some(expected) = expected_sha256
            && Sha256::digest(encrypted_payload).as_slice() != expected
        {
            return Err(MediaDecryptionError::PlaintextSha256Mismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn existing_media_type_numeric_casts_remain_stable() {
        for (media_type, expected) in [
            (MediaType::Image, 0),
            (MediaType::Video, 1),
            (MediaType::Audio, 2),
            (MediaType::Document, 3),
            (MediaType::History, 4),
            (MediaType::AppState, 5),
            (MediaType::Sticker, 6),
            (MediaType::StickerPack, 7),
            (MediaType::StickerPackThumbnail, 8),
            (MediaType::LinkThumbnail, 9),
            (MediaType::ProductCatalogImage, 10),
        ] {
            assert_eq!(media_type as isize, expected, "{media_type:?}");
        }
    }

    #[test]
    fn music_artwork_uses_the_fixed_hkdf_context_and_offsets() {
        let key: [u8; 32] = std::array::from_fn(|i| i as u8);
        let mut expanded = [0; 112];
        crate::crypto::hkdf_sha256_into(
            &key,
            None,
            MediaType::MusicArtwork.app_info().as_bytes(),
            &mut expanded,
        )
        .unwrap();
        let expected = hex::decode("7e8118adc593efbab3564bd1c11c16bf215fa9ee4b83c91b9878fbe04895379cbb30e875056436bf91a36f80c22be3ef8c3dd4822ed2140b4ded31e13bfded333e0ca74c166f6b46dd82cd39656a09c463d427103e1d14ef0f4ee035fcefa74d7d85b465c4d878f70a8cd03bbfdecb80").unwrap();
        assert_eq!(expanded.as_slice(), expected);
        let (iv, cipher, mac) =
            DownloadUtils::get_media_keys(&key, MediaType::MusicArtwork).unwrap();
        assert_eq!(iv.as_slice(), &expected[..16]);
        assert_eq!(cipher.as_slice(), &expected[16..48]);
        assert_eq!(mac.as_slice(), &expected[48..80]);
        assert_eq!(MediaType::MusicArtwork.mms_type(), "music-artwork");
        assert_eq!(MediaType::MusicArtwork.upload_path(), "/mms/music-artwork");
    }

    #[test]
    fn music_artwork_downloads_and_rejects_wrong_hashes() {
        let plaintext = b"synthetic music artwork";
        let key = [7; 32];
        let enc =
            crate::upload::encrypt_media_with_key(plaintext, MediaType::MusicArtwork, Some(&key))
                .unwrap();
        let metadata = wa::EmbeddedMusic {
            artwork_direct_path: Some("/mms/music-artwork/synthetic".into()),
            artwork_media_key: Some(key.to_vec()),
            artwork_sha256: Some(enc.file_sha256.to_vec()),
            artwork_enc_sha256: Some(enc.file_enc_sha256.to_vec()),
            ..Default::default()
        };
        assert_eq!(metadata.app_info(), MediaType::MusicArtwork);
        assert_eq!(metadata.file_length(), None);
        let requests = DownloadUtils::prepare_download_requests(&metadata, &mock_route()).unwrap();
        assert!(requests[0].url.contains(&format!(
            "/mms/music-artwork/synthetic?token={}",
            BASE64_URL_SAFE_NO_PAD.encode(enc.file_enc_sha256)
        )));
        assert!(
            matches!(&requests[0].decryption, MediaDecryption::Encrypted { media_key, media_type: MediaType::MusicArtwork } if media_key == &key)
        );
        let mut output = Vec::new();
        DownloadUtils::decrypt_stream_to_writer_with_hashes(
            std::io::Cursor::new(enc.data_to_upload.as_slice()),
            &key,
            MediaType::MusicArtwork,
            Some(&enc.file_enc_sha256),
            Some(&enc.file_sha256),
            &mut output,
        )
        .unwrap();
        assert_eq!(output, plaintext);
        for (encrypted_hash, plaintext_hash, expected_error) in [
            (
                [0; 32],
                enc.file_sha256,
                "SHA-256 mismatch for encrypted media bytes",
            ),
            (
                enc.file_enc_sha256,
                [0; 32],
                "SHA-256 mismatch for decrypted media bytes",
            ),
        ] {
            let error = DownloadUtils::decrypt_stream_to_writer_with_hashes(
                std::io::Cursor::new(enc.data_to_upload.as_slice()),
                &key,
                MediaType::MusicArtwork,
                Some(&encrypted_hash),
                Some(&plaintext_hash),
                &mut Vec::new(),
            )
            .unwrap_err();
            assert!(error.to_string().contains(expected_error), "{error}");
        }
    }

    #[test]
    fn music_artwork_missing_key_never_implies_plaintext() {
        let mut metadata = wa::EmbeddedMusic {
            artwork_direct_path: Some("/mms/music-artwork/synthetic".into()),
            artwork_sha256: Some(vec![1; 32]),
            artwork_enc_sha256: Some(vec![2; 32]),
            ..Default::default()
        };
        let chat = "15551234567@s.whatsapp.net".parse().unwrap();
        let artwork = MusicArtwork::for_chat(&metadata, &chat);
        let raw = MockDownloadable {
            direct_path: metadata.artwork_direct_path.clone(),
            static_url: None,
            media_key: None,
            file_sha256: metadata.artwork_sha256.clone(),
            file_enc_sha256: metadata.artwork_enc_sha256.clone(),
            media_type: MediaType::MusicArtwork,
        };
        assert!(metadata.is_encrypted() && artwork.is_encrypted());
        for downloadable in [
            &metadata as &dyn Downloadable,
            &artwork as &dyn Downloadable,
            &raw as &dyn Downloadable,
        ] {
            assert!(
                DownloadUtils::prepare_download_requests(downloadable, &mock_route())
                    .unwrap_err()
                    .to_string()
                    .contains("Missing media_key")
            );
        }
        metadata.artwork_media_key = Some(vec![7; 32]);
        metadata.artwork_enc_sha256 = None;
        assert!(
            DownloadUtils::prepare_download_requests(&metadata, &mock_route())
                .unwrap_err()
                .to_string()
                .contains("Missing file_enc_sha256")
        );
    }

    #[test]
    fn music_artwork_newsletter_context_uses_plaintext_and_its_own_route() {
        use sha2::Digest;
        let plaintext = b"synthetic newsletter artwork";
        let hash = Sha256::digest(plaintext);
        let metadata = wa::EmbeddedMusic {
            artwork_direct_path: Some("/mms/newsletter-music-artwork/synthetic".into()),
            artwork_sha256: Some(hash.to_vec()),
            artwork_media_key: Some(vec![7; 32]),
            ..Default::default()
        };
        let chat = Jid::newsletter("42");
        let artwork = MusicArtwork::for_chat(&metadata, &chat);
        assert!(!artwork.is_encrypted());
        assert_eq!(artwork.app_info(), MediaType::NewsletterMusicArtwork);
        assert_eq!(artwork.app_info().mms_type(), "newsletter-music-artwork");
        assert_eq!(
            artwork.app_info().upload_path(),
            "/mms/newsletter-music-artwork"
        );
        let requests = DownloadUtils::prepare_download_requests(&artwork, &mock_route()).unwrap();
        assert!(
            requests[0]
                .url
                .ends_with(&format!("token={}", BASE64_URL_SAFE_NO_PAD.encode(hash)))
        );
        assert!(
            matches!(&requests[0].decryption, MediaDecryption::Plaintext { file_sha256 } if file_sha256 == hash.as_slice())
        );
        DownloadUtils::validate_plaintext_sha256(plaintext, &hash).unwrap();
        assert!(DownloadUtils::validate_plaintext_sha256(plaintext, &[0; 32]).is_err());
    }

    #[test]
    fn group_history_uses_its_own_media_path_and_key_derivation_context() {
        assert_eq!(MediaType::GroupHistory.app_info(), "Group History");
        assert_eq!(MediaType::GroupHistory.mms_type(), "group-history");
        assert_eq!(MediaType::GroupHistory.upload_path(), "/mms/group-history");
        assert_ne!(
            MediaType::GroupHistory.app_info(),
            MediaType::History.app_info()
        );

        let media_key = [0x5A; 32];
        let group_history_keys = DownloadUtils::get_media_keys(&media_key, MediaType::GroupHistory)
            .expect("group history media keys");
        let history_keys = DownloadUtils::get_media_keys(&media_key, MediaType::History)
            .expect("history media keys");
        assert_ne!(group_history_keys, history_keys);
        // Independent Node.js hkdfSync('sha256', [0x5a; 32], empty salt,
        // 'Group History', 112) fixture from the pinned WA Web media info.
        assert_eq!(
            group_history_keys.0,
            [
                0x71, 0xba, 0xe6, 0x97, 0x44, 0x61, 0xbf, 0x53, 0xab, 0xc8, 0x9a, 0xcd, 0xf9, 0xca,
                0xbe, 0x19
            ]
        );
    }

    #[test]
    fn group_history_bundle_downloads_through_the_typed_media_path() {
        let plaintext = b"synthetic group history bundle bytes";
        let enc = crate::upload::encrypt_media(plaintext, MediaType::GroupHistory)
            .expect("encrypt group history fixture");
        let bundle = wa::message::MessageHistoryBundle {
            mimetype: Some("application/protobuf".into()),
            file_sha256: Some(enc.file_sha256.to_vec()),
            media_key: Some(enc.media_key.to_vec()),
            file_enc_sha256: Some(enc.file_enc_sha256.to_vec()),
            direct_path: Some("/v/synthetic-group-history.enc".into()),
            ..Default::default()
        };
        assert!(bundle.file_length().is_none());
        assert_eq!(bundle.app_info(), MediaType::GroupHistory);

        let requests = DownloadUtils::prepare_download_requests(&bundle, &mock_route())
            .expect("bundle download requests");
        assert!(!requests.is_empty());
        let MediaDecryption::Encrypted {
            media_key,
            media_type,
        } = &requests[0].decryption
        else {
            panic!("group history bundle must decrypt as E2E media");
        };
        assert_eq!(*media_type, MediaType::GroupHistory);
        let decrypted =
            DownloadUtils::verify_and_decrypt(&enc.data_to_upload, media_key, *media_type)
                .expect("decrypt group history fixture");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn hash_checked_decryption_covers_padding_and_short_reads() {
        use std::io::{Cursor, Read};
        struct ShortReads<'a> {
            remaining: &'a [u8],
            chunk: usize,
        }
        impl Read for ShortReads<'_> {
            fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
                let n = out.len().min(self.chunk).min(self.remaining.len());
                out[..n].copy_from_slice(&self.remaining[..n]);
                self.remaining = &self.remaining[n..];
                Ok(n)
            }
        }

        for len in [0, 1, 15, 16, 17, 8191, 8192, 8193, 32781] {
            let plaintext = vec![0x59; len];
            let enc = crate::upload::encrypt_media(&plaintext, MediaType::Image).unwrap();
            for chunk in [1, 7, 10, 16, 31, 8192] {
                let mut output = Cursor::new(Vec::new());
                let written = DownloadUtils::decrypt_stream_to_writer_with_hashes(
                    ShortReads {
                        remaining: &enc.data_to_upload,
                        chunk,
                    },
                    &enc.media_key,
                    MediaType::Image,
                    Some(&enc.file_enc_sha256),
                    Some(&enc.file_sha256),
                    &mut output,
                )
                .unwrap();
                assert_eq!(written, len as u64);
                assert_eq!(output.into_inner(), plaintext);
            }
            let mut buffered = enc.data_to_upload.clone();
            DownloadUtils::verify_and_decrypt_in_place_with_hashes(
                &mut buffered,
                &enc.media_key,
                MediaType::Image,
                Some(&enc.file_enc_sha256),
                Some(&enc.file_sha256),
            )
            .unwrap();
            assert_eq!(buffered, plaintext);
        }
    }

    #[test]
    fn hash_checked_decryption_still_rejects_bad_mac_and_padding() {
        use hmac::KeyInit;
        use sha2::Digest;

        let plaintext = b"authenticated fixture";
        let media_key = [0x4A; 32];
        let mut bad_mac = encrypted_media_fixture(plaintext, &media_key, MediaType::Image);
        let bad_mac_len = bad_mac.len() - 1;
        bad_mac[bad_mac_len] ^= 1;
        let expected_bad_mac_hash = Sha256::digest(&bad_mac);

        let mut bad_padding = encrypted_media_fixture(&[0x62; 16], &media_key, MediaType::Image);
        let ciphertext_len = bad_padding.len() - MEDIA_MAC_SIZE;
        // Alter the previous CBC block so the final PKCS#7 byte becomes 17,
        // then authenticate the malformed ciphertext with the synthetic key.
        bad_padding[ciphertext_len - AES_BLOCK_SIZE - 1] ^= 1;
        let (iv, _, mac_key) = DownloadUtils::get_media_keys(&media_key, MediaType::Image).unwrap();
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(&mac_key).unwrap();
        mac.update(&iv);
        mac.update(&bad_padding[..ciphertext_len]);
        let tag = mac.finalize().into_bytes();
        bad_padding[ciphertext_len..].copy_from_slice(&tag[..MEDIA_MAC_SIZE]);
        let expected_bad_padding_hash = Sha256::digest(&bad_padding);

        for (payload, encrypted_hash) in [
            (&bad_mac, expected_bad_mac_hash.as_slice()),
            (&bad_padding, expected_bad_padding_hash.as_slice()),
        ] {
            let mut output = Vec::new();
            assert!(
                DownloadUtils::decrypt_stream_to_writer_with_hashes(
                    payload.as_slice(),
                    &media_key,
                    MediaType::Image,
                    Some(encrypted_hash),
                    Some(Sha256::digest(plaintext).as_slice()),
                    &mut output,
                )
                .is_err()
            );

            let mut buffered = payload.clone();
            assert!(
                DownloadUtils::verify_and_decrypt_in_place_with_hashes(
                    &mut buffered,
                    &media_key,
                    MediaType::Image,
                    Some(encrypted_hash),
                    Some(Sha256::digest(plaintext).as_slice()),
                )
                .is_err()
            );
        }
    }

    #[test]
    fn hash_checked_decryption_distinguishes_absent_and_incorrect_hashes() {
        let plaintext = b"independent hash checks";
        let enc = crate::upload::encrypt_media(plaintext, MediaType::Image).unwrap();
        for (encrypted, decrypted) in [
            (None, None),
            (Some(enc.file_enc_sha256.as_slice()), None),
            (None, Some(enc.file_sha256.as_slice())),
        ] {
            let mut output = Vec::new();
            DownloadUtils::decrypt_stream_to_writer_with_hashes(
                enc.data_to_upload.as_slice(),
                &enc.media_key,
                MediaType::Image,
                encrypted,
                decrypted,
                &mut output,
            )
            .unwrap();
            assert_eq!(output, plaintext);
            let mut buffered = enc.data_to_upload.clone();
            DownloadUtils::verify_and_decrypt_in_place_with_hashes(
                &mut buffered,
                &enc.media_key,
                MediaType::Image,
                encrypted,
                decrypted,
            )
            .unwrap();
            assert_eq!(buffered, plaintext);
        }
        let mut wrong = enc.file_sha256;
        wrong[0] ^= 1;
        for bad in [&wrong[..], &[][..]] {
            let mut output = Vec::new();
            let error = DownloadUtils::decrypt_stream_to_writer_with_hashes(
                enc.data_to_upload.as_slice(),
                &enc.media_key,
                MediaType::Image,
                Some(&enc.file_enc_sha256),
                Some(bad),
                &mut output,
            )
            .unwrap_err();
            assert!(matches!(
                error.downcast_ref::<MediaDecryptionError>(),
                Some(MediaDecryptionError::PlaintextSha256Mismatch)
            ));
            let error = DownloadUtils::verify_and_decrypt_in_place_with_hashes(
                &mut enc.data_to_upload.clone(),
                &enc.media_key,
                MediaType::Image,
                Some(&enc.file_enc_sha256),
                Some(bad),
            )
            .unwrap_err();
            assert!(matches!(
                error,
                MediaDecryptionError::PlaintextSha256Mismatch
            ));
        }
    }

    #[test]
    fn download_debug_redacts_nested_keys_and_signed_urls() {
        let decryption = MediaDecryption::Encrypted {
            media_key: vec![179; 32],
            media_type: MediaType::Video,
        };
        for rendered in [format!("{decryption:?}"), format!("{decryption:#?}")] {
            assert!(!rendered.contains("179"), "{rendered}");
            assert!(rendered.contains("Video"), "{rendered}");
        }
        let request = DownloadRequest {
            url: "https://synthetic-user:synthetic-password@cdn.example.com/file?auth=synthetic-auth#synthetic-fragment".into(),
            decryption,
        };
        for rendered in [format!("{request:?}"), format!("{request:#?}")] {
            assert!(!rendered.contains("synthetic-"), "{rendered}");
            assert!(!rendered.contains("179"), "{rendered}");
            assert!(rendered.contains("Video"), "{rendered}");
            assert!(rendered.contains("cdn.example.com"), "{rendered}");
        }
        assert!(
            matches!(request.decryption, MediaDecryption::Encrypted { media_key, .. } if media_key == vec![179; 32])
        );
    }

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

    fn mock_route() -> MediaRoute {
        MediaRoute::new(mock_hosts())
    }

    /// Every variant. The exhaustive match in
    /// `every_media_type_builds_download_urls` is what forces a new
    /// one to be named here instead of silently skipping the URL assertions.
    const ALL_MEDIA_TYPES: [MediaType; 14] = [
        MediaType::Image,
        MediaType::Video,
        MediaType::Audio,
        MediaType::Document,
        MediaType::History,
        MediaType::GroupHistory,
        MediaType::AppState,
        MediaType::Sticker,
        MediaType::StickerPack,
        MediaType::StickerPackThumbnail,
        MediaType::LinkThumbnail,
        MediaType::ProductCatalogImage,
        MediaType::MusicArtwork,
        MediaType::NewsletterMusicArtwork,
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
    fn download_urls_preserve_signed_queries_encoding_and_fragments() {
        for direct_path in [
            "/mms/a%2Fb.enc?x=one%26two&plus=%2B&empty=#part%20one",
            "mms/a%2Fb.enc?x=one%26two&plus=%2B&empty=#part%20one",
        ] {
            let media = MockDownloadable {
                direct_path: Some(direct_path.into()),
                static_url: None,
                media_key: Some(vec![1; 32]),
                file_sha256: Some(vec![2; 32]),
                file_enc_sha256: Some(vec![3; 32]),
                media_type: MediaType::Image,
            };
            let requests = DownloadUtils::prepare_download_requests(&media, &mock_route()).unwrap();
            assert_eq!(requests.len(), 2);
            for (index, request) in requests.iter().enumerate() {
                let url = url::Url::parse(&request.url).unwrap();
                assert_eq!(url.host_str(), Some(mock_hosts()[index].hostname.as_str()));
                assert_eq!(url.path(), "/mms/a%2Fb.enc");
                assert_eq!(url.fragment(), Some("part%20one"));
                let pairs = url.query_pairs().collect::<Vec<_>>();
                assert_eq!(
                    pairs
                        .iter()
                        .map(|(k, v)| (k.as_ref(), v.as_ref()))
                        .collect::<Vec<_>>(),
                    vec![
                        ("x", "one&two"),
                        ("plus", "+"),
                        ("empty", ""),
                        ("token", BASE64_URL_SAFE_NO_PAD.encode([3; 32]).as_str()),
                    ]
                );
                assert!(request.url.contains("x=one%26two&plus=%2B&empty=&token="));
            }
        }
    }

    #[test]
    fn adding_a_token_preserves_existing_query_bytes() {
        let query = "space=a%20b&plus=a+b&lower=%2f&upper=%2F&empty=&flag&dup=1&dup=2";
        let media = MockDownloadable {
            direct_path: Some(format!("/media?{query}#fragment")),
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let requests = DownloadUtils::prepare_download_requests(&media, &mock_route()).unwrap();
        let expected = format!("{query}&token={}", BASE64_URL_SAFE_NO_PAD.encode([3; 32]));
        for request in requests {
            let url = url::Url::parse(&request.url).unwrap();
            assert_eq!(url.query(), Some(expected.as_str()));
            assert_eq!(url.fragment(), Some("fragment"));
        }
    }

    #[test]
    fn uri_download_resolution_matches_the_url_oracle() {
        for (host, path) in [
            (
                "cdn.example.com",
                "/a%2Fb?x=%20&plus=+&dup=1&dup=2#part%20one",
            ),
            ("cdn.example.com", "relative/../file?flag&empty=&tail=1&"),
            ("cdn.example.com", "../file?x=%2f"),
            ("cdn.example.com", "?x=one%26two#fragment"),
            ("cdn.example.com", "#fragment"),
            ("cdn.example.com", ""),
            ("cdn.example.com", "//cdn.example.com/file"),
            ("cdn.example.com", "https://CDN.EXAMPLE.COM:443/file?x=1"),
            ("cdn.example.com:8443", "/file?x=1"),
            ("[::1]:8443", "https://[::1]:8443/file?x=1#part"),
            ("127.0.0.1:8080", "/file"),
        ] {
            let media = MockDownloadable {
                direct_path: Some(path.into()),
                static_url: None,
                media_key: Some(vec![1; 32]),
                file_sha256: Some(vec![2; 32]),
                file_enc_sha256: Some(vec![3; 32]),
                media_type: MediaType::Image,
            };
            let mut oracle = url::Url::parse(&format!("https://{host}/"))
                .unwrap()
                .join(path)
                .unwrap();
            oracle
                .query_pairs_mut()
                .append_pair("token", &BASE64_URL_SAFE_NO_PAD.encode([3; 32]));
            let requests = DownloadUtils::prepare_download_requests(
                &media,
                &MediaRoute::new(vec![MediaHost::new(host)]),
            )
            .unwrap();
            let actual = url::Url::parse(&requests[0].url).unwrap();
            assert_eq!(actual, oracle, "{host} {path}");
        }
    }

    #[test]
    fn download_references_reject_uri_repairs_and_ambiguous_authorities() {
        let mut media = MockDownloadable {
            direct_path: None,
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let route = MediaRoute::new(vec![MediaHost::new("cdn.example.com")]);
        for path in [
            "/raw space",
            "/raw\tcontrol",
            "/café",
            "/file?bad=%zz",
            "/file?bad=%",
            "https://cdn.example.com:65536/file",
            "https://cdn.example.com:/file",
            "https://%63dn.example.com/file",
            "https://@cdn.example.com/file",
            "https:///evil.example/file",
            "https:evil.example/file",
        ] {
            media.direct_path = Some(path.into());
            assert!(
                DownloadUtils::prepare_download_requests(&media, &route).is_err(),
                "{path}"
            );
        }
        media.direct_path = Some("/file".into());
        for host in [
            "",
            "cdn.example.com:",
            "cdn.example.com:65536",
            "cdn.example.com:port",
            "%63dn.example.com",
            "cdn.example.com%2f.evil.example",
            "[v1.example]",
            "@cdn.example.com",
            "cdn.example.com;other",
            "cdn.example.com\n",
        ] {
            let route = MediaRoute::new(vec![MediaHost::new(host)]);
            assert!(
                DownloadUtils::prepare_download_requests(&media, &route).is_err(),
                "{host}"
            );
        }
    }

    #[test]
    fn invalid_hosts_do_not_discard_valid_fallbacks() {
        let media = MockDownloadable {
            direct_path: Some("/media?x=1".into()),
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let route = MediaRoute::new(vec![
            MediaHost::new("bad.example/path"),
            MediaHost::new("cdn1.example.com"),
            MediaHost::new("bad.example?query"),
            MediaHost::new("cdn2.example.com"),
        ]);
        let requests = DownloadUtils::prepare_download_requests(&media, &route).unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[0].url.starts_with("https://cdn1.example.com/"));
        assert!(requests[1].url.starts_with("https://cdn2.example.com/"));
    }

    #[test]
    fn download_direct_paths_cannot_change_the_route_origin() {
        let mut media = MockDownloadable {
            direct_path: None,
            static_url: None,
            media_key: Some(vec![1; 32]),
            file_sha256: Some(vec![2; 32]),
            file_enc_sha256: Some(vec![3; 32]),
            media_type: MediaType::Image,
        };
        let route = MediaRoute::new(vec![MediaHost::new("cdn.example.com")]);
        for path in [
            "//evil.example/file",
            "https://evil.example/file",
            "http://cdn.example.com/file",
            "https://cdn.example.com:444/file",
            "https://user:secret@cdn.example.com/file",
            "\\\\evil.example/file",
        ] {
            media.direct_path = Some(path.into());
            assert!(
                DownloadUtils::prepare_download_requests(&media, &route).is_err(),
                "{path}"
            );
        }
        media.direct_path = Some("https://cdn.example.com/file?existing=value".into());
        assert!(DownloadUtils::prepare_download_requests(&media, &route).is_ok());
        for host in [
            "cdn.example.com/other",
            "user:secret@cdn.example.com",
            "cdn.example.com?secret=value",
            "cdn.example.com#secret",
        ] {
            let route = MediaRoute::new(vec![MediaHost::new(host)]);
            assert!(
                DownloadUtils::prepare_download_requests(&media, &route).is_err(),
                "{host}"
            );
        }
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
        let reqs = DownloadUtils::prepare_download_requests(&d, &mock_route()).unwrap();
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
        let route = MediaRoute::new(mock_hosts());
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
    fn default_route_uses_the_known_cdn_hosts_without_auth() {
        let route = MediaRoute::default_hosts();
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
    fn every_media_type_builds_download_urls() {
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
                | MediaType::GroupHistory
                | MediaType::AppState
                | MediaType::Sticker
                | MediaType::StickerPack
                | MediaType::StickerPackThumbnail
                | MediaType::LinkThumbnail
                | MediaType::ProductCatalogImage
                | MediaType::MusicArtwork
                | MediaType::NewsletterMusicArtwork => {}
            }

            let d = MockDownloadable {
                direct_path: Some("/v/t1/media.enc".into()),
                static_url: None,
                media_key: Some(vec![1; 32]),
                file_sha256: Some(vec![2; 32]),
                file_enc_sha256: Some(vec![3; 32]),
                media_type,
            };
            let token = BASE64_URL_SAFE_NO_PAD.encode(if d.is_encrypted() {
                [3u8; 32]
            } else {
                [2u8; 32]
            });

            let requests =
                DownloadUtils::prepare_download_requests(&d, &MediaRoute::new(mock_hosts()))
                    .unwrap();
            assert_eq!(
                requests[0].url,
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
            DownloadUtils::prepare_download_requests(&d, &MediaRoute::new(Vec::new())).unwrap();
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
        let reqs = DownloadUtils::prepare_download_requests(&d, &mock_route()).unwrap();
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
        let reqs = DownloadUtils::prepare_download_requests(&d, &mock_route()).unwrap();
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
            DownloadUtils::prepare_download_requests(&d, &MediaRoute::new(Vec::new())).unwrap();
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
        let err = DownloadUtils::prepare_download_requests(&d, &mock_route()).unwrap_err();
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
