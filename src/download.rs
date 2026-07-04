use crate::client::Client;
use crate::mediaconn::{MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS, MediaConn, is_media_auth_error};
use anyhow::{Result, anyhow};
use std::io::{Seek, SeekFrom, Write};

pub use wacore::download::{
    DownloadUtils, Downloadable, MediaDecryption, MediaDecryptionError, MediaType,
};

/// Cap on the speculative capacity pre-allocated for the in-memory download
/// buffer. Sized to the plaintext length the message declares, but a bogus
/// length must not drive a multi-GB allocation before a single byte arrives;
/// beyond this the buffer grows on demand. Comfortably above typical
/// image/video/audio media so the common case is a single allocation.
const DOWNLOAD_PREALLOC_CAP: u64 = 64 * 1024 * 1024;

impl From<&MediaConn> for wacore::download::MediaConnection {
    fn from(conn: &MediaConn) -> Self {
        wacore::download::MediaConnection {
            hosts: conn
                .hosts
                .iter()
                .map(|h| wacore::download::MediaHost {
                    hostname: h.hostname.clone(),
                })
                .collect(),
            auth: conn.auth.clone(),
        }
    }
}

/// `Downloadable` built from raw CDN fields, for re-downloading media without
/// the original message in hand.
pub struct DownloadParams {
    pub direct_path: String,
    pub media_key: Option<Vec<u8>>,
    pub file_sha256: Vec<u8>,
    pub file_enc_sha256: Option<Vec<u8>>,
    pub file_length: u64,
    pub media_type: MediaType,
}

impl DownloadParams {
    /// Params for encrypted media. Slices are copied into the owned struct.
    pub fn encrypted(
        direct_path: impl Into<String>,
        media_key: &[u8],
        file_sha256: &[u8],
        file_enc_sha256: &[u8],
        file_length: u64,
        media_type: MediaType,
    ) -> Self {
        Self {
            direct_path: direct_path.into(),
            media_key: Some(media_key.to_vec()),
            file_sha256: file_sha256.to_vec(),
            file_enc_sha256: Some(file_enc_sha256.to_vec()),
            file_length,
            media_type,
        }
    }
}

impl Downloadable for DownloadParams {
    fn direct_path(&self) -> Option<&str> {
        Some(&self.direct_path)
    }
    fn media_key(&self) -> Option<&[u8]> {
        self.media_key.as_deref()
    }
    fn file_enc_sha256(&self) -> Option<&[u8]> {
        self.file_enc_sha256.as_deref()
    }
    fn file_sha256(&self) -> Option<&[u8]> {
        Some(&self.file_sha256)
    }
    fn file_length(&self) -> Option<u64> {
        Some(self.file_length)
    }
    fn app_info(&self) -> MediaType {
        self.media_type
    }
}

#[derive(Debug)]
enum DownloadRequestError {
    Auth(anyhow::Error),
    /// 404/410 — media URL expired or not found. Needs fresh auth + URL re-derivation.
    /// Matches WA Web's `MediaNotFoundError` handling.
    NotFound(anyhow::Error),
    Other(anyhow::Error),
}

impl DownloadRequestError {
    fn auth(status_code: u16) -> Self {
        Self::Auth(anyhow!("Download failed with status: {}", status_code))
    }

    fn not_found(status_code: u16) -> Self {
        Self::NotFound(anyhow!(
            "Download media not found/expired with status: {}",
            status_code
        ))
    }

    fn other(err: impl Into<anyhow::Error>) -> Self {
        Self::Other(err.into())
    }

    fn is_auth(&self) -> bool {
        matches!(self, Self::Auth(_))
    }

    /// Returns true for 404/410 (expired URL) — should trigger auth refresh like auth errors.
    fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Auth(err) | Self::NotFound(err) | Self::Other(err) => err,
        }
    }
}

/// Auth-refresh + host-failover retry loop that returns the decrypted bytes.
/// Unlike [`download_to_writer_with_retry`] each attempt gets a FRESH buffer
/// (the executor allocates its own), so a failed host that wrote a longer body
/// (e.g. a CDN error page that decrypts to more bytes before its MAC fails)
/// can't leave a stale tail behind a shorter successful retry.
async fn download_media_with_retry<
    PrepareRequests,
    PrepareRequestsFut,
    InvalidateMediaConn,
    InvalidateMediaConnFut,
    ExecuteRequest,
    ExecuteRequestFut,
>(
    mut prepare_requests: PrepareRequests,
    mut invalidate_media_conn: InvalidateMediaConn,
    mut execute_request: ExecuteRequest,
) -> Result<Vec<u8>>
where
    PrepareRequests: FnMut(bool) -> PrepareRequestsFut,
    PrepareRequestsFut:
        std::future::Future<Output = Result<Vec<wacore::download::DownloadRequest>>>,
    InvalidateMediaConn: FnMut() -> InvalidateMediaConnFut,
    InvalidateMediaConnFut: std::future::Future<Output = ()>,
    ExecuteRequest: FnMut(wacore::download::DownloadRequest) -> ExecuteRequestFut,
    ExecuteRequestFut:
        std::future::Future<Output = std::result::Result<Vec<u8>, DownloadRequestError>>,
{
    let mut force_refresh = false;
    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..=MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS {
        let requests = prepare_requests(force_refresh).await?;
        let mut retry_with_fresh_auth = false;

        for request in requests {
            match execute_request(request.clone()).await {
                Ok(data) => return Ok(data),
                Err(err) if (err.is_auth() || err.is_not_found()) && attempt == 0 => {
                    // Auth error or 404/410 (expired URL): refresh media conn and re-derive URLs.
                    invalidate_media_conn().await;
                    force_refresh = true;
                    retry_with_fresh_auth = true;
                    break;
                }
                Err(err) if err.is_auth() || err.is_not_found() => return Err(err.into_anyhow()),
                Err(err) => {
                    let err = err.into_anyhow();
                    log::warn!(
                        "Failed to download from URL {}: {:?}. Trying next host.",
                        request.url,
                        err
                    );
                    last_err = Some(err);
                }
            }
        }

        if !retry_with_fresh_auth {
            break;
        }
    }

    match last_err {
        Some(err) => Err(err),
        None => Err(anyhow!("Failed to download from all available media hosts")),
    }
}

async fn download_to_writer_with_retry<
    W,
    PrepareRequests,
    PrepareRequestsFut,
    InvalidateMediaConn,
    InvalidateMediaConnFut,
    ExecuteRequest,
    ExecuteRequestFut,
>(
    mut writer: W,
    mut prepare_requests: PrepareRequests,
    mut invalidate_media_conn: InvalidateMediaConn,
    mut execute_request: ExecuteRequest,
) -> Result<W>
where
    W: Write + Seek + Send + 'static,
    PrepareRequests: FnMut(bool) -> PrepareRequestsFut,
    PrepareRequestsFut:
        std::future::Future<Output = Result<Vec<wacore::download::DownloadRequest>>>,
    InvalidateMediaConn: FnMut() -> InvalidateMediaConnFut,
    InvalidateMediaConnFut: std::future::Future<Output = ()>,
    ExecuteRequest: FnMut(wacore::download::DownloadRequest, W) -> ExecuteRequestFut,
    ExecuteRequestFut:
        std::future::Future<Output = Result<(W, std::result::Result<(), DownloadRequestError>)>>,
{
    let mut force_refresh = false;
    let mut last_err: Option<anyhow::Error> = None;

    for attempt in 0..=MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS {
        let requests = prepare_requests(force_refresh).await?;
        let mut retry_with_fresh_auth = false;

        for request in requests {
            let (next_writer, result) = execute_request(request.clone(), writer).await?;
            writer = next_writer;

            match result {
                Ok(()) => return Ok(writer),
                Err(err) if (err.is_auth() || err.is_not_found()) && attempt == 0 => {
                    invalidate_media_conn().await;
                    force_refresh = true;
                    retry_with_fresh_auth = true;
                    break;
                }
                Err(err) if err.is_auth() || err.is_not_found() => return Err(err.into_anyhow()),
                Err(err) => {
                    let err = err.into_anyhow();
                    log::warn!(
                        "Failed to stream-download from URL {}: {:?}. Trying next host.",
                        request.url,
                        err
                    );
                    last_err = Some(err);
                }
            }
        }

        if !retry_with_fresh_auth {
            break;
        }
    }

    match last_err {
        Some(err) => Err(err),
        None => Err(anyhow!("Failed to download from all available media hosts")),
    }
}

impl Client {
    /// Downloads and decrypts media from WhatsApp's CDN into memory.
    ///
    /// Only needed when you need the plaintext bytes (processing, transcoding,
    /// re-upload). To forward existing media unchanged, reuse the original
    /// message's CDN fields directly, no round-trip required.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.media.download", level = "debug", skip_all, err(Debug))
    )]
    pub async fn download(&self, downloadable: &dyn Downloadable) -> Result<Vec<u8>> {
        // Stream each attempt into a FRESH in-memory buffer: the CDN read and the
        // AES/HMAC decrypt interleave in one blocking pass (overlapping network
        // with CPU, ~halving peak memory vs. fetch-whole-then-decrypt), and a new
        // buffer per attempt means a failed host that wrote a longer body can't
        // leave a stale tail behind a shorter successful retry. Pre-size to the
        // declared plaintext length, capped so a bogus length can't drive a huge
        // speculative allocation.
        let cap = downloadable
            .file_length()
            .unwrap_or(0)
            .min(DOWNLOAD_PREALLOC_CAP) as usize;
        download_media_with_retry(
            |force| self.prepare_requests(downloadable, force),
            || async { self.invalidate_media_conn().await },
            |request| async move {
                let writer = std::io::Cursor::new(Vec::with_capacity(cap));
                match self.streaming_download_and_decrypt(&request, writer).await {
                    Ok((writer, Ok(()))) => Ok(writer.into_inner()),
                    Ok((_, Err(e))) => Err(e),
                    Err(e) => Err(DownloadRequestError::other(e)),
                }
            },
        )
        .await
    }

    /// Fetch a first-party sticker pack's metadata and sticker list from the CDN.
    ///
    /// Each returned [`wacore::sticker_pack::StickerPackItem`] is [`Downloadable`],
    /// so individual stickers can be fetched with [`Self::download`]. The locale
    /// only affects localized pack names; `"en"` mirrors whatsmeow's default.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.media.fetch_sticker_pack",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub async fn fetch_sticker_pack(
        &self,
        pack_id: &str,
        locale: &str,
    ) -> Result<wacore::sticker_pack::StickerPack> {
        let url = wacore::sticker_pack::sticker_pack_data_url(pack_id, locale);
        let response = self
            .http_client
            .execute(crate::http::HttpRequest::get(&url))
            .await
            .map_err(|e| anyhow!("sticker pack request failed: {e}"))?;
        if response.status_code != 200 {
            return Err(anyhow!(
                "sticker pack endpoint returned status {}",
                response.status_code
            ));
        }
        wacore::sticker_pack::parse_sticker_pack_response(&response.body)
    }

    /// Downloads and decrypts media from raw parameters without needing the original message.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.download_from_params", level = "debug", skip_all, fields(kind = ?params.media_type), err(Debug)))]
    pub async fn download_from_params(&self, params: &DownloadParams) -> Result<Vec<u8>> {
        self.download(params).await
    }

    async fn prepare_requests(
        &self,
        downloadable: &dyn Downloadable,
        force_refresh: bool,
    ) -> Result<Vec<wacore::download::DownloadRequest>> {
        let media_conn = self.refresh_media_conn(force_refresh).await?;
        let core_media_conn = wacore::download::MediaConnection::from(&media_conn);
        DownloadUtils::prepare_download_requests(downloadable, &core_media_conn)
    }

    /// Downloads and decrypts media with streaming (constant memory usage).
    ///
    /// The entire HTTP download, decryption, and file write happen in a single
    /// blocking thread. The writer is seeked back to position 0 before returning.
    ///
    /// The `writer` MUST start empty. Retries/host-failover seek back to 0 and
    /// rewrite but do NOT truncate, so a writer that already held more bytes than
    /// the decrypted payload would keep a stale tail past the valid data. (The
    /// in-memory [`Self::download`] gives every attempt a fresh buffer for exactly
    /// this reason.)
    ///
    /// Memory usage: ~40KB regardless of file size (8KB read buffer + decrypt state).
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.media.download_to_writer",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub async fn download_to_writer<W: Write + Seek + Send + 'static>(
        &self,
        downloadable: &dyn Downloadable,
        writer: W,
    ) -> Result<W> {
        download_to_writer_with_retry(
            writer,
            |force| self.prepare_requests(downloadable, force),
            || async { self.invalidate_media_conn().await },
            |request, writer| async move { self.streaming_download_and_decrypt(&request, writer).await },
        )
        .await
    }

    /// Streaming variant of `download_from_params` that writes to a writer
    /// instead of buffering in memory.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.download_from_params_to_writer", level = "debug", skip_all, fields(kind = ?params.media_type), err(Debug)))]
    pub async fn download_from_params_to_writer<W: Write + Seek + Send + 'static>(
        &self,
        params: &DownloadParams,
        writer: W,
    ) -> Result<W> {
        self.download_to_writer(params, writer).await
    }

    /// Download + decrypt to a writer. Uses streaming when available,
    /// falls back to buffered otherwise. Returns writer for retry.
    async fn streaming_download_and_decrypt<W: Write + Seek + Send + 'static>(
        &self,
        request: &wacore::download::DownloadRequest,
        writer: W,
    ) -> Result<(W, std::result::Result<(), DownloadRequestError>)> {
        if !self.http_client.supports_streaming() {
            return self.buffered_download_and_decrypt(request, writer).await;
        }

        let http_client = self.http_client.clone();
        let url = request.url.clone();
        let decryption = request.decryption.clone();

        Ok(wacore::runtime::blocking(&*self.runtime, move || {
            let mut writer = writer;

            if let Err(e) = writer.seek(SeekFrom::Start(0)) {
                return (writer, Err(DownloadRequestError::other(e)));
            }

            let result = (|| -> std::result::Result<(), DownloadRequestError> {
                let http_request = crate::http::HttpRequest::get(url);
                let resp = http_client
                    .execute_streaming(http_request)
                    .map_err(DownloadRequestError::other)?;

                if resp.status_code >= 300 {
                    return Err(if is_media_auth_error(resp.status_code) {
                        DownloadRequestError::auth(resp.status_code)
                    } else if matches!(resp.status_code, 404 | 410) {
                        DownloadRequestError::not_found(resp.status_code)
                    } else {
                        DownloadRequestError::other(anyhow!(
                            "Download failed with status: {}",
                            resp.status_code
                        ))
                    });
                }

                match &decryption {
                    MediaDecryption::Encrypted {
                        media_key,
                        media_type,
                    } => {
                        DownloadUtils::decrypt_stream_to_writer(
                            resp.body,
                            media_key,
                            *media_type,
                            &mut writer,
                        )
                        .map_err(DownloadRequestError::other)?;
                    }
                    MediaDecryption::Plaintext { file_sha256 } => {
                        DownloadUtils::copy_and_validate_plaintext_to_writer(
                            resp.body,
                            file_sha256,
                            &mut writer,
                        )
                        .map_err(DownloadRequestError::other)?;
                    }
                }
                writer
                    .seek(SeekFrom::Start(0))
                    .map_err(DownloadRequestError::other)?;
                Ok(())
            })();

            (writer, result)
        })
        .await)
    }

    /// Buffered fallback when streaming is not available.
    async fn buffered_download_and_decrypt<W: Write + Seek + Send + 'static>(
        &self,
        request: &wacore::download::DownloadRequest,
        mut writer: W,
    ) -> Result<(W, std::result::Result<(), DownloadRequestError>)> {
        let http_request = crate::http::HttpRequest::get(request.url.clone());
        let resp = match self.http_client.execute(http_request).await {
            Ok(r) => r,
            Err(e) => return Ok((writer, Err(DownloadRequestError::other(e)))),
        };

        if resp.status_code >= 300 {
            let err = if is_media_auth_error(resp.status_code) {
                DownloadRequestError::auth(resp.status_code)
            } else if matches!(resp.status_code, 404 | 410) {
                DownloadRequestError::not_found(resp.status_code)
            } else {
                DownloadRequestError::other(anyhow!(
                    "Download failed with status: {}",
                    resp.status_code
                ))
            };
            return Ok((writer, Err(err)));
        }

        let decryption = request.decryption.clone();

        // Offload blocking decrypt+write to avoid stalling the async executor
        Ok(wacore::runtime::blocking(&*self.runtime, move || {
            if let Err(e) = writer.seek(SeekFrom::Start(0)) {
                return (writer, Err(DownloadRequestError::other(e)));
            }

            let result = (|| -> std::result::Result<(), DownloadRequestError> {
                let reader = std::io::Cursor::new(resp.body);
                match &decryption {
                    MediaDecryption::Encrypted {
                        media_key,
                        media_type,
                    } => {
                        DownloadUtils::decrypt_stream_to_writer(
                            reader,
                            media_key,
                            *media_type,
                            &mut writer,
                        )
                        .map_err(DownloadRequestError::other)?;
                    }
                    MediaDecryption::Plaintext { file_sha256 } => {
                        DownloadUtils::copy_and_validate_plaintext_to_writer(
                            reader,
                            file_sha256,
                            &mut writer,
                        )
                        .map_err(DownloadRequestError::other)?;
                    }
                }
                writer
                    .seek(SeekFrom::Start(0))
                    .map_err(DownloadRequestError::other)?;
                Ok(())
            })();

            (writer, result)
        })
        .await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mediaconn::{MediaConn, MediaConnHost};
    use async_lock::Mutex;
    use std::io::Cursor;
    use std::sync::Arc;
    use wacore::time::Instant;

    struct PlaintextDownloadable {
        direct_path: String,
        file_sha256: Vec<u8>,
    }

    impl Downloadable for PlaintextDownloadable {
        fn direct_path(&self) -> Option<&str> {
            Some(&self.direct_path)
        }

        fn media_key(&self) -> Option<&[u8]> {
            None
        }

        fn file_enc_sha256(&self) -> Option<&[u8]> {
            None
        }

        fn file_sha256(&self) -> Option<&[u8]> {
            Some(&self.file_sha256)
        }

        fn file_length(&self) -> Option<u64> {
            None
        }

        fn app_info(&self) -> MediaType {
            MediaType::Image
        }
    }

    fn media_conn(auth: &str, hosts: &[&str]) -> MediaConn {
        MediaConn {
            auth: auth.to_string(),
            ttl: 60,
            auth_ttl: None,
            hosts: hosts
                .iter()
                .map(|hostname| MediaConnHost::new((*hostname).to_string()))
                .collect(),
            fetched_at: Instant::now(),
        }
    }

    fn plaintext_sha256(data: &[u8]) -> Vec<u8> {
        wacore::upload::encrypt_media(data, MediaType::Image)
            .expect("hash derivation should succeed")
            .file_sha256
            .to_vec()
    }

    #[test]
    fn process_downloaded_media_ok() {
        let data = b"Hello media test";
        let enc = wacore::upload::encrypt_media(data, MediaType::Image)
            .expect("encryption should succeed");
        let mut cursor = Cursor::new(Vec::<u8>::new());
        let plaintext = DownloadUtils::verify_and_decrypt(
            &enc.data_to_upload,
            &enc.media_key,
            MediaType::Image,
        )
        .expect("decryption should succeed");
        cursor.write_all(&plaintext).expect("write should succeed");
        assert_eq!(cursor.into_inner(), data);
    }

    #[test]
    fn process_downloaded_media_bad_mac() {
        let data = b"Tamper";
        let mut enc = wacore::upload::encrypt_media(data, MediaType::Image)
            .expect("encryption should succeed");
        let last = enc.data_to_upload.len() - 1;
        enc.data_to_upload[last] ^= 0x01;

        let err = DownloadUtils::verify_and_decrypt(
            &enc.data_to_upload,
            &enc.media_key,
            MediaType::Image,
        )
        .unwrap_err();

        assert!(
            matches!(&err, wacore::download::MediaDecryptionError::InvalidMac),
            "Expected InvalidMac, got: {}",
            err
        );
    }

    // `download()` uses `download_media_with_retry` (fresh buffer per attempt);
    // cover its auth-refresh + host-failover retry behavior directly.
    #[tokio::test]
    async fn download_retries_with_forced_media_conn_refresh_after_auth_error() {
        let body = b"download me".to_vec();
        let downloadable = PlaintextDownloadable {
            direct_path: "/v/t62.7118-24/123".to_string(),
            file_sha256: plaintext_sha256(&body),
        };
        let first_conn = media_conn("stale-auth", &["cdn1.example.com"]);
        let refreshed_conn = media_conn("fresh-auth", &["cdn2.example.com"]);
        let refresh_calls = Arc::new(Mutex::new(Vec::new()));
        let invalidations = Arc::new(Mutex::new(0usize));
        let seen_urls = Arc::new(Mutex::new(Vec::new()));

        let downloaded = download_media_with_retry(
            {
                let refresh_calls = Arc::clone(&refresh_calls);
                let downloadable = &downloadable;
                move |force| {
                    let refresh_calls = Arc::clone(&refresh_calls);
                    let first_conn = first_conn.clone();
                    let refreshed_conn = refreshed_conn.clone();
                    async move {
                        refresh_calls.lock().await.push(force);
                        let media_conn = if force { refreshed_conn } else { first_conn };
                        DownloadUtils::prepare_download_requests(
                            downloadable,
                            &wacore::download::MediaConnection::from(&media_conn),
                        )
                    }
                }
            },
            {
                let invalidations = Arc::clone(&invalidations);
                move || {
                    let invalidations = Arc::clone(&invalidations);
                    async move {
                        *invalidations.lock().await += 1;
                    }
                }
            },
            {
                let seen_urls = Arc::clone(&seen_urls);
                let body = body.clone();
                move |request| {
                    let seen_urls = Arc::clone(&seen_urls);
                    let body = body.clone();
                    let url = request.url.clone();
                    async move {
                        seen_urls.lock().await.push(url.clone());
                        if url.contains("stale-auth") {
                            Err(DownloadRequestError::auth(401))
                        } else {
                            Ok(body)
                        }
                    }
                }
            },
        )
        .await
        .expect("download should succeed after refreshing media auth");

        assert_eq!(downloaded, body);
        assert_eq!(*refresh_calls.lock().await, vec![false, true]);
        assert_eq!(*invalidations.lock().await, 1);

        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("auth=stale-auth"));
        assert!(seen_urls[1].contains("auth=fresh-auth"));
    }

    // A generic (non-auth, non-404) error on one host must fall through to the
    // next host within the SAME attempt — no media-conn refresh — and succeed.
    #[tokio::test]
    async fn download_fails_over_to_next_host_without_refresh() {
        let body = b"failover me".to_vec();
        let downloadable = PlaintextDownloadable {
            direct_path: "/v/t62.7118-24/failover".to_string(),
            file_sha256: plaintext_sha256(&body),
        };
        let conn = media_conn(
            "auth-tok",
            &["bad-host.example.com", "good-host.example.com"],
        );
        let refresh_calls = Arc::new(Mutex::new(Vec::new()));
        let invalidations = Arc::new(Mutex::new(0usize));
        let seen_urls = Arc::new(Mutex::new(Vec::new()));

        let downloaded = download_media_with_retry(
            {
                let refresh_calls = Arc::clone(&refresh_calls);
                let downloadable = &downloadable;
                let conn = conn.clone();
                move |force| {
                    let refresh_calls = Arc::clone(&refresh_calls);
                    let conn = conn.clone();
                    async move {
                        refresh_calls.lock().await.push(force);
                        DownloadUtils::prepare_download_requests(
                            downloadable,
                            &wacore::download::MediaConnection::from(&conn),
                        )
                    }
                }
            },
            {
                let invalidations = Arc::clone(&invalidations);
                move || {
                    let invalidations = Arc::clone(&invalidations);
                    async move {
                        *invalidations.lock().await += 1;
                    }
                }
            },
            {
                let seen_urls = Arc::clone(&seen_urls);
                let body = body.clone();
                move |request| {
                    let seen_urls = Arc::clone(&seen_urls);
                    let body = body.clone();
                    let url = request.url.clone();
                    async move {
                        seen_urls.lock().await.push(url.clone());
                        if url.contains("bad-host") {
                            Err(DownloadRequestError::other(anyhow!("connection reset")))
                        } else {
                            Ok(body)
                        }
                    }
                }
            },
        )
        .await
        .expect("download should fail over to the healthy host");

        assert_eq!(downloaded, body);
        // Single attempt, no refresh: a generic error doesn't invalidate the media conn.
        assert_eq!(*refresh_calls.lock().await, vec![false]);
        assert_eq!(*invalidations.lock().await, 0);
        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("bad-host"));
        assert!(seen_urls[1].contains("good-host"));
    }

    // When every host fails with a generic error, the accumulated `last_err`
    // is surfaced (not the fallback "all hosts" message) and no refresh happens.
    #[tokio::test]
    async fn download_propagates_last_error_when_all_hosts_fail() {
        let body = b"never arrives".to_vec();
        let downloadable = PlaintextDownloadable {
            direct_path: "/v/t62.7118-24/allfail".to_string(),
            file_sha256: plaintext_sha256(&body),
        };
        let conn = media_conn("auth-tok", &["host-a.example.com", "host-b.example.com"]);
        let invalidations = Arc::new(Mutex::new(0usize));
        let seen_urls = Arc::new(Mutex::new(Vec::new()));

        let err = download_media_with_retry(
            {
                let downloadable = &downloadable;
                let conn = conn.clone();
                move |_force| {
                    let conn = conn.clone();
                    async move {
                        DownloadUtils::prepare_download_requests(
                            downloadable,
                            &wacore::download::MediaConnection::from(&conn),
                        )
                    }
                }
            },
            {
                let invalidations = Arc::clone(&invalidations);
                move || {
                    let invalidations = Arc::clone(&invalidations);
                    async move {
                        *invalidations.lock().await += 1;
                    }
                }
            },
            {
                let seen_urls = Arc::clone(&seen_urls);
                move |request| {
                    let seen_urls = Arc::clone(&seen_urls);
                    let url = request.url.clone();
                    async move {
                        seen_urls.lock().await.push(url.clone());
                        Err::<Vec<u8>, _>(DownloadRequestError::other(anyhow!("host {url} down")))
                    }
                }
            },
        )
        .await
        .expect_err("all hosts failing must surface an error");

        assert!(
            err.to_string().contains("down"),
            "expected the propagated last_err, got: {err}"
        );
        assert_eq!(*invalidations.lock().await, 0);
        assert_eq!(seen_urls.lock().await.len(), 2);
    }

    #[tokio::test]
    async fn download_to_writer_retries_with_forced_media_conn_refresh_after_auth_error() {
        let body = b"stream me".to_vec();
        let downloadable = PlaintextDownloadable {
            direct_path: "/v/t62.7118-24/stream".to_string(),
            file_sha256: plaintext_sha256(&body),
        };
        let first_conn = media_conn("stale-auth", &["cdn1.example.com"]);
        let refreshed_conn = media_conn("fresh-auth", &["cdn2.example.com"]);
        let refresh_calls = Arc::new(Mutex::new(Vec::new()));
        let invalidations = Arc::new(Mutex::new(0usize));
        let seen_urls = Arc::new(Mutex::new(Vec::new()));

        let writer = download_to_writer_with_retry(
            Cursor::new(Vec::<u8>::new()),
            {
                let refresh_calls = Arc::clone(&refresh_calls);
                let downloadable = &downloadable;
                move |force| {
                    let refresh_calls = Arc::clone(&refresh_calls);
                    let first_conn = first_conn.clone();
                    let refreshed_conn = refreshed_conn.clone();
                    async move {
                        refresh_calls.lock().await.push(force);
                        let media_conn = if force { refreshed_conn } else { first_conn };
                        DownloadUtils::prepare_download_requests(
                            downloadable,
                            &wacore::download::MediaConnection::from(&media_conn),
                        )
                    }
                }
            },
            {
                let invalidations = Arc::clone(&invalidations);
                move || {
                    let invalidations = Arc::clone(&invalidations);
                    async move {
                        *invalidations.lock().await += 1;
                    }
                }
            },
            {
                let seen_urls = Arc::clone(&seen_urls);
                let body = body.clone();
                move |request, mut writer| {
                    let seen_urls = Arc::clone(&seen_urls);
                    let body = body.clone();
                    let url = request.url.clone();
                    async move {
                        seen_urls.lock().await.push(url.clone());
                        writer.seek(SeekFrom::Start(0))?;
                        if url.contains("stale-auth") {
                            Ok((writer, Err(DownloadRequestError::auth(403))))
                        } else {
                            writer.write_all(&body)?;
                            writer.seek(SeekFrom::Start(0))?;
                            Ok((writer, Ok(())))
                        }
                    }
                }
            },
        )
        .await
        .expect("streaming download should succeed after refreshing media auth");

        assert_eq!(writer.into_inner(), body);
        assert_eq!(*refresh_calls.lock().await, vec![false, true]);
        assert_eq!(*invalidations.lock().await, 1);

        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("auth=stale-auth"));
        assert!(seen_urls[1].contains("auth=fresh-auth"));
    }

    /// HTTP client that records the requested URL and returns a canned response.
    struct CannedHttpClient {
        status: u16,
        body: Vec<u8>,
        seen_url: Mutex<Option<String>>,
    }

    #[async_trait::async_trait]
    impl crate::http::HttpClient for CannedHttpClient {
        async fn execute(
            &self,
            request: crate::http::HttpRequest,
        ) -> Result<crate::http::HttpResponse> {
            *self.seen_url.lock().await = Some(request.url);
            Ok(crate::http::HttpResponse {
                status_code: self.status,
                body: self.body.clone(),
            })
        }
    }

    #[tokio::test]
    async fn fetch_sticker_pack_hits_cdn_and_parses() {
        use base64::engine::{Engine, general_purpose::STANDARD};
        let body = format!(
            r#"[{{"sticker-pack-id":"P1","name":"Cats","stickers":[
                {{"media-key":"{}","file-hash":"{}","enc-file-hash":"{}","direct-path":"/d","file-size":9}}
            ]}}]"#,
            STANDARD.encode([1u8; 32]),
            STANDARD.encode([2u8; 32]),
            STANDARD.encode([3u8; 32]),
        );
        let http = Arc::new(CannedHttpClient {
            status: 200,
            body: body.into_bytes(),
            seen_url: Mutex::new(None),
        });
        let client =
            crate::test_utils::create_test_client_with_http("sticker_fetch", http.clone()).await;

        let pack = client.fetch_sticker_pack("P1", "en").await.unwrap();
        assert_eq!(pack.sticker_pack_id.as_deref(), Some("P1"));
        assert_eq!(pack.stickers.len(), 1);
        assert_eq!(pack.stickers[0].direct_path(), Some("/d"));

        let url = http.seen_url.lock().await.clone().unwrap();
        assert_eq!(
            url,
            "https://static.whatsapp.net/sticker?lottie=1&cat=sticker_pack_data&id=P1&lg=en"
        );
    }

    #[tokio::test]
    async fn fetch_sticker_pack_errors_on_non_200() {
        let http = Arc::new(CannedHttpClient {
            status: 404,
            body: Vec::new(),
            seen_url: Mutex::new(None),
        });
        let client = crate::test_utils::create_test_client_with_http("sticker_404", http).await;
        assert!(client.fetch_sticker_pack("P1", "en").await.is_err());
    }
}
