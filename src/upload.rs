use anyhow::{Result, anyhow};
use base64::Engine;
use serde::Deserialize;
use wacore::download::MediaType;

use crate::client::Client;
use crate::http::{HttpRequest, HttpResponse};
use crate::mediaconn::{MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS, is_media_auth_error};

/// Files >= 5 MiB check for existing/partial upload before sending.
/// Matches WA Web's `_checkIfAlreadyUploaded` flow.
const RESUMABLE_UPLOAD_THRESHOLD: usize = 5 * 1024 * 1024;

/// Result of checking if an upload already exists on the server.
enum UploadExistsResult {
    /// Upload is complete — server already has the file.
    Complete { url: String, direct_path: String },
    /// Upload is partially done — resume from this byte offset.
    Resume { byte_offset: u64 },
    /// No previous upload found — start from scratch.
    NotFound,
}

/// Server response for upload progress check (`?resume=1`).
#[derive(Deserialize)]
struct UploadProgressResponse {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    direct_path: Option<String>,
    /// "complete" or a byte offset as string.
    #[serde(default)]
    resume: Option<String>,
}

/// Parse an upload progress response into an `UploadExistsResult`.
fn parse_upload_progress(resp: &HttpResponse, total_size: u64) -> UploadExistsResult {
    if resp.status_code >= 400 {
        return UploadExistsResult::NotFound;
    }
    let Ok(progress) = serde_json::from_slice::<UploadProgressResponse>(&resp.body) else {
        return UploadExistsResult::NotFound;
    };
    match progress.resume.as_deref() {
        Some("complete") => {
            if let (Some(url), Some(direct_path)) = (progress.url, progress.direct_path) {
                UploadExistsResult::Complete { url, direct_path }
            } else {
                UploadExistsResult::NotFound
            }
        }
        Some(offset_str) => match offset_str.parse::<u64>() {
            Ok(offset) if offset > 0 && offset < total_size => UploadExistsResult::Resume {
                byte_offset: offset,
            },
            _ => UploadExistsResult::NotFound,
        },
        _ => UploadExistsResult::NotFound,
    }
}

/// URL + headers only; the body is supplied by `send_body`, so one retry/resume
/// loop serves both the buffered and streaming paths.
fn build_upload_request(
    hostname: &str,
    upload_path: &str,
    auth: &str,
    token: &str,
    file_offset: Option<u64>,
) -> HttpRequest {
    let mut url = format!("https://{hostname}{upload_path}/{token}?auth={auth}&token={token}");
    if let Some(offset) = file_offset {
        url.push_str("&file_offset=");
        url.push_str(itoa::Buffer::new().format(offset));
    }

    HttpRequest::post(url)
        .with_header("Content-Type", "application/octet-stream")
        .with_header("Origin", "https://web.whatsapp.com")
}

fn build_resume_check_request(
    hostname: &str,
    upload_path: &str,
    auth: &str,
    token: &str,
) -> HttpRequest {
    let url = format!("https://{hostname}{upload_path}/{token}?auth={auth}&token={token}&resume=1");
    HttpRequest::post(url).with_header("Origin", "https://web.whatsapp.com")
}

fn upload_error_from_response(response: HttpResponse) -> anyhow::Error {
    match response.body_string() {
        Ok(body) => anyhow!("Upload failed {} body={}", response.status_code, body),
        Err(body_err) => anyhow!(
            "Upload failed {} and failed to read response body: {}",
            response.status_code,
            body_err
        ),
    }
}

/// Crypto metadata needed to finalize an upload, independent of how the
/// encrypted body is transmitted (buffered or streamed).
struct UploadCrypto {
    media_key: [u8; 32],
    file_sha256: [u8; 32],
    file_enc_sha256: [u8; 32],
    streaming_sidecar: Option<Vec<u8>>,
}

impl UploadCrypto {
    fn response(
        &self,
        url: String,
        direct_path: String,
        file_length: u64,
        media_key_timestamp: i64,
    ) -> UploadResponse {
        UploadResponse {
            url,
            direct_path,
            media_key: self.media_key,
            file_enc_sha256: self.file_enc_sha256,
            file_sha256: self.file_sha256,
            file_length,
            media_key_timestamp,
            streaming_sidecar: self.streaming_sidecar.clone(),
        }
    }
}

/// Drives host failover, auth refresh, and resumable upload. `file_length` is the
/// plaintext size (for the response); `ciphertext_len` is the encrypted blob size
/// (for the resume threshold/offsets). `send_body` transmits the body from a given
/// offset; `execute_request` serves the body-less resume check.
#[allow(clippy::too_many_arguments)]
async fn upload_media_with_retry<GMC, GMCFut, IMC, IMCFut, EXR, EXRFut, SB, SBFut>(
    crypto: UploadCrypto,
    media_type: MediaType,
    file_length: u64,
    ciphertext_len: u64,
    media_key_timestamp: i64,
    mut get_media_conn: GMC,
    mut invalidate_media_conn: IMC,
    mut execute_request: EXR,
    mut send_body: SB,
) -> Result<UploadResponse>
where
    GMC: FnMut(bool) -> GMCFut,
    GMCFut: std::future::Future<Output = Result<crate::mediaconn::MediaConn>>,
    IMC: FnMut() -> IMCFut,
    IMCFut: std::future::Future<Output = ()>,
    EXR: FnMut(HttpRequest) -> EXRFut,
    EXRFut: std::future::Future<Output = Result<HttpResponse>>,
    SB: FnMut(HttpRequest, u64, u64) -> SBFut,
    SBFut: std::future::Future<Output = Result<HttpResponse>>,
{
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(crypto.file_enc_sha256);
    let upload_path = media_type.upload_path();
    let mut force_refresh = false;
    let mut last_error: Option<anyhow::Error> = None;

    for attempt in 0..=MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS {
        let media_conn = get_media_conn(force_refresh).await?;
        if media_conn.hosts.is_empty() {
            return Err(anyhow!("No media hosts"));
        }

        let mut retry_with_fresh_auth = false;

        for host in &media_conn.hosts {
            let mut offset: u64 = 0;
            let mut file_offset: Option<u64> = None;

            // For large files, check whether the upload already exists or can be
            // resumed. Matches WA Web's _checkIfAlreadyUploaded flow.
            if ciphertext_len >= RESUMABLE_UPLOAD_THRESHOLD as u64 {
                let check_req = build_resume_check_request(
                    &host.hostname,
                    upload_path,
                    &media_conn.auth,
                    &token,
                );
                if let Ok(check_resp) = execute_request(check_req).await {
                    match parse_upload_progress(&check_resp, ciphertext_len) {
                        UploadExistsResult::Complete { url, direct_path } => {
                            return Ok(crypto.response(
                                url,
                                direct_path,
                                file_length,
                                media_key_timestamp,
                            ));
                        }
                        UploadExistsResult::Resume { byte_offset } => {
                            if byte_offset >= ciphertext_len {
                                log::warn!(
                                    "Server resume offset {byte_offset} exceeds data length {ciphertext_len}; uploading from start"
                                );
                            } else {
                                log::info!(
                                    "Resuming upload from byte {byte_offset}/{ciphertext_len}"
                                );
                                offset = byte_offset;
                                file_offset = Some(byte_offset);
                            }
                        }
                        UploadExistsResult::NotFound => {}
                    }
                }
                // Non-fatal: if the check request itself fails, proceed with full upload.
            }

            let request = build_upload_request(
                &host.hostname,
                upload_path,
                &media_conn.auth,
                &token,
                file_offset,
            );

            let response = match send_body(request, offset, ciphertext_len - offset).await {
                Ok(response) => response,
                Err(err) => {
                    last_error = Some(err);
                    continue;
                }
            };

            if response.status_code < 400 {
                let raw: RawUploadResponse = serde_json::from_slice(&response.body)?;
                return Ok(crypto.response(
                    raw.url,
                    raw.direct_path,
                    file_length,
                    media_key_timestamp,
                ));
            }

            let status_code = response.status_code;
            let err = upload_error_from_response(response);

            if is_media_auth_error(status_code) {
                if attempt == 0 {
                    invalidate_media_conn().await;
                    force_refresh = true;
                    retry_with_fresh_auth = true;
                    break;
                }

                return Err(err);
            }

            last_error = Some(err);
        }

        if !retry_with_fresh_auth {
            break;
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("Failed to upload to all available media hosts")))
}

#[derive(Debug, Clone)]
pub struct UploadResponse {
    pub url: String,
    pub direct_path: String,
    pub media_key: [u8; 32],
    pub file_enc_sha256: [u8; 32],
    pub file_sha256: [u8; 32],
    pub file_length: u64,
    /// Unix timestamp (seconds) when the media key was generated.
    pub media_key_timestamp: i64,
    /// Per-64-KiB HMAC table for progressive playback/seek (audio/video only);
    /// pass to `AudioMessage`/`VideoMessage.streaming_sidecar`.
    pub streaming_sidecar: Option<Vec<u8>>,
}

impl From<UploadResponse> for wacore::sticker_pack::MediaUploadInfo {
    fn from(r: UploadResponse) -> Self {
        Self::new(
            r.direct_path,
            r.media_key,
            r.file_sha256,
            r.file_enc_sha256,
            r.file_length,
            r.media_key_timestamp,
        )
    }
}

#[derive(Deserialize)]
struct RawUploadResponse {
    url: String,
    direct_path: String,
}

#[non_exhaustive]
#[derive(Default, Clone)]
pub struct UploadOptions {
    /// Reuse an existing media key instead of generating a fresh one.
    pub media_key: Option<[u8; 32]>,
    /// Override streaming-sidecar generation; `None` selects it by media type
    /// (audio/video only).
    pub streaming_sidecar: Option<bool>,
}

impl std::fmt::Debug for UploadOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadOptions")
            .field("media_key", &self.media_key.as_ref().map(|_| "<redacted>"))
            .field("streaming_sidecar", &self.streaming_sidecar)
            .finish()
    }
}

impl UploadOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_media_key(mut self, key: [u8; 32]) -> Self {
        self.media_key = Some(key);
        self
    }

    pub fn with_streaming_sidecar(mut self, enabled: bool) -> Self {
        self.streaming_sidecar = Some(enabled);
        self
    }
}

impl Client {
    /// Encrypts and uploads media to WhatsApp's CDN.
    ///
    /// Only needed for new or modified media. To forward existing media unchanged,
    /// reuse the original message's CDN fields directly, no round-trip required.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.upload", level = "debug", skip_all, fields(kind = ?media_type, len = data.len()), err(Debug)))]
    pub async fn upload(
        &self,
        data: Vec<u8>,
        media_type: MediaType,
        options: UploadOptions,
    ) -> Result<UploadResponse> {
        let file_length = data.len() as u64;
        let media_key = options.media_key;
        let sidecar = options.streaming_sidecar;
        let enc = wacore::runtime::blocking(&*self.runtime, move || {
            wacore::upload::encrypt_media_with_key_and_sidecar(
                &data,
                media_type,
                media_key.as_ref(),
                sidecar,
            )
        })
        .await?;

        let ciphertext_len = enc.data_to_upload.len() as u64;
        let crypto = UploadCrypto {
            media_key: enc.media_key,
            file_sha256: enc.file_sha256,
            file_enc_sha256: enc.file_enc_sha256,
            streaming_sidecar: enc.streaming_sidecar,
        };
        let ciphertext = enc.data_to_upload;

        upload_media_with_retry(
            crypto,
            media_type,
            file_length,
            ciphertext_len,
            wacore::time::now_secs(),
            |force| async move { self.refresh_media_conn(force).await.map_err(Into::into) },
            || async { self.invalidate_media_conn().await },
            |request| async move { self.http_client.execute(request).await },
            |request, offset, _remaining| {
                let body = ciphertext[offset as usize..].to_vec();
                async move { self.http_client.execute(request.with_body(body)).await }
            },
        )
        .await
    }

    /// Uploads already-encrypted media streamed from `source`, keeping memory
    /// constant regardless of file size. Encrypt the plaintext first with
    /// [`wacore::upload::encrypt_media_streaming`] (or `..._with_key`) into the
    /// storage of your choice, then pass that storage as `source` plus the
    /// returned [`wacore::upload::EncryptedMediaInfo`]. The caller owns where the
    /// ciphertext lives (temp file, memory, …); this method never touches disk.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.upload_stream", level = "debug", skip_all, fields(kind = ?media_type), err(Debug)))]
    pub async fn upload_stream<S>(
        &self,
        source: S,
        info: wacore::upload::EncryptedMediaInfo,
        media_type: MediaType,
    ) -> Result<UploadResponse>
    where
        S: wacore::upload::UploadSource + 'static,
    {
        let file_length = info.file_length;
        let ciphertext_len = source.len();
        let crypto = UploadCrypto {
            media_key: info.media_key,
            file_sha256: info.file_sha256,
            file_enc_sha256: info.file_enc_sha256,
            streaming_sidecar: info.streaming_sidecar,
        };
        let source = std::sync::Arc::new(source);

        upload_media_with_retry(
            crypto,
            media_type,
            file_length,
            ciphertext_len,
            wacore::time::now_secs(),
            |force| async move { self.refresh_media_conn(force).await.map_err(Into::into) },
            || async { self.invalidate_media_conn().await },
            |request| async move { self.http_client.execute(request).await },
            |request, offset, remaining| {
                let source = std::sync::Arc::clone(&source);
                let http = std::sync::Arc::clone(&self.http_client);
                async move {
                    // reader_from may open/seek a file, so run it in the blocking
                    // task with execute_upload, not on the async worker.
                    wacore::runtime::blocking(&*self.runtime, move || {
                        let reader = source.reader_from(offset)?;
                        http.execute_upload(request, reader, remaining)
                    })
                    .await
                }
            },
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mediaconn::{MediaConn, MediaConnHost};
    use async_lock::Mutex;
    use std::sync::Arc;
    use wacore::time::Instant;

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

    fn crypto_from(enc: &wacore::upload::EncryptedMedia) -> UploadCrypto {
        UploadCrypto {
            media_key: enc.media_key,
            file_sha256: enc.file_sha256,
            file_enc_sha256: enc.file_enc_sha256,
            streaming_sidecar: enc.streaming_sidecar.clone(),
        }
    }

    fn ok_json(url: &str, direct_path: &str) -> HttpResponse {
        HttpResponse {
            status_code: 200,
            body: format!(r#"{{"url":"{url}","direct_path":"{direct_path}"}}"#).into_bytes(),
        }
    }

    /// Resume check is skipped below the 5 MiB threshold, so a check call here
    /// would be a logic error.
    async fn unreachable_check(_req: HttpRequest) -> Result<HttpResponse> {
        panic!("resume check must not run below the resumable threshold")
    }

    #[tokio::test]
    async fn upload_retries_with_forced_media_conn_refresh_after_auth_error() {
        let enc = wacore::upload::encrypt_media(b"retry me", MediaType::Image)
            .expect("encryption should succeed");
        let len = enc.data_to_upload.len() as u64;
        let first_conn = media_conn("stale-auth", &["cdn1.example.com"]);
        let refreshed_conn = media_conn("fresh-auth", &["cdn2.example.com"]);
        let refresh_calls = Arc::new(Mutex::new(Vec::new()));
        let invalidations = Arc::new(Mutex::new(0usize));
        let seen_urls = Arc::new(Mutex::new(Vec::new()));

        let result = upload_media_with_retry(
            crypto_from(&enc),
            MediaType::Image,
            8,
            len,
            0,
            {
                let refresh_calls = Arc::clone(&refresh_calls);
                move |force| {
                    let refresh_calls = Arc::clone(&refresh_calls);
                    let first_conn = first_conn.clone();
                    let refreshed_conn = refreshed_conn.clone();
                    async move {
                        refresh_calls.lock().await.push(force);
                        Ok(if force { refreshed_conn } else { first_conn })
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
            unreachable_check,
            {
                let seen_urls = Arc::clone(&seen_urls);
                move |request: HttpRequest, _offset, _remaining| {
                    let seen_urls = Arc::clone(&seen_urls);
                    async move {
                        seen_urls.lock().await.push(request.url.clone());
                        if request.url.contains("stale-auth") {
                            Ok(HttpResponse {
                                status_code: 401,
                                body: b"expired".to_vec(),
                            })
                        } else {
                            Ok(ok_json(
                                "https://cdn2.example.com/file",
                                "/v/t62.7118-24/123",
                            ))
                        }
                    }
                }
            },
        )
        .await
        .expect("upload should succeed after refreshing media auth");

        assert_eq!(*refresh_calls.lock().await, vec![false, true]);
        assert_eq!(*invalidations.lock().await, 1);

        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("cdn1.example.com"));
        assert!(seen_urls[0].contains("auth=stale-auth"));
        assert!(seen_urls[1].contains("cdn2.example.com"));
        assert!(seen_urls[1].contains("auth=fresh-auth"));
        assert_eq!(result.direct_path, "/v/t62.7118-24/123");
        assert_eq!(result.url, "https://cdn2.example.com/file");
        assert_eq!(result.media_key_timestamp, 0);
    }

    #[tokio::test]
    async fn upload_fails_over_to_next_host_after_non_auth_error() {
        let enc = wacore::upload::encrypt_media(b"retry host", MediaType::Image)
            .expect("encryption should succeed");
        let len = enc.data_to_upload.len() as u64;
        let conn = media_conn("shared-auth", &["cdn1.example.com", "cdn2.example.com"]);
        let seen_urls = Arc::new(Mutex::new(Vec::new()));

        let result = upload_media_with_retry(
            crypto_from(&enc),
            MediaType::Image,
            10,
            len,
            0,
            move |_force| {
                let conn = conn.clone();
                async move { Ok(conn) }
            },
            || async {},
            unreachable_check,
            {
                let seen_urls = Arc::clone(&seen_urls);
                move |request: HttpRequest, _offset, _remaining| {
                    let seen_urls = Arc::clone(&seen_urls);
                    async move {
                        seen_urls.lock().await.push(request.url.clone());
                        if request.url.contains("cdn1.example.com") {
                            Ok(HttpResponse {
                                status_code: 500,
                                body: b"try another host".to_vec(),
                            })
                        } else {
                            Ok(ok_json(
                                "https://cdn2.example.com/file",
                                "/v/t62.7118-24/456",
                            ))
                        }
                    }
                }
            },
        )
        .await
        .expect("upload should succeed on the second host");

        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("cdn1.example.com"));
        assert!(seen_urls[1].contains("cdn2.example.com"));
        assert_eq!(result.direct_path, "/v/t62.7118-24/456");
        assert_eq!(result.media_key_timestamp, 0);
    }

    /// Above the threshold, a server `resume=<offset>` must drive `send_body` to
    /// upload only the tail (`file_offset` in the URL, `remaining = len - offset`).
    #[tokio::test]
    async fn resumable_upload_sends_tail_from_offset() {
        let total: u64 = 6 * 1024 * 1024;
        let offset: u64 = 1024 * 1024;
        let conn = media_conn("auth", &["cdn1.example.com"]);
        let captured: Arc<Mutex<Option<(String, u64, u64)>>> = Arc::new(Mutex::new(None));

        let crypto = UploadCrypto {
            media_key: [0u8; 32],
            file_sha256: [0u8; 32],
            file_enc_sha256: [9u8; 32],
            streaming_sidecar: Some(vec![1, 2, 3]),
        };

        let result = upload_media_with_retry(
            crypto,
            MediaType::Video,
            total,
            total,
            0,
            move |_force| {
                let conn = conn.clone();
                async move { Ok(conn) }
            },
            || async {},
            move |_req| async move {
                Ok(HttpResponse {
                    status_code: 200,
                    body: format!(r#"{{"resume":"{offset}"}}"#).into_bytes(),
                })
            },
            {
                let captured = Arc::clone(&captured);
                move |request: HttpRequest, off, remaining| {
                    let captured = Arc::clone(&captured);
                    async move {
                        *captured.lock().await = Some((request.url.clone(), off, remaining));
                        Ok(ok_json("https://cdn1.example.com/f", "/dp"))
                    }
                }
            },
        )
        .await
        .expect("resumable upload should succeed");

        let (url, sent_offset, remaining) = captured.lock().await.clone().unwrap();
        assert_eq!(sent_offset, offset);
        assert_eq!(remaining, total - offset);
        assert!(url.contains(&format!("file_offset={offset}")), "url: {url}");
        // Sidecar must survive into the response.
        assert_eq!(result.streaming_sidecar.as_deref(), Some(&[1u8, 2, 3][..]));
    }

    /// `UploadSource` over `Arc<[u8]>` must report the length and re-read the
    /// tail from any offset, repeatably.
    #[test]
    fn arc_upload_source_reads_from_offset_repeatably() {
        use std::io::Read;
        use wacore::upload::UploadSource;

        let bytes: Arc<[u8]> = (0u8..200).collect::<Vec<_>>().into();
        assert_eq!(UploadSource::len(&bytes), 200);

        let read_at = |offset: u64| {
            let mut r = bytes.reader_from(offset).unwrap();
            let mut out = Vec::new();
            r.read_to_end(&mut out).unwrap();
            out
        };

        assert_eq!(read_at(0), (0u8..200).collect::<Vec<_>>());
        assert_eq!(read_at(50), (50u8..200).collect::<Vec<_>>());
        // Independent, repeatable readers (host failover / auth refresh).
        assert_eq!(read_at(50), read_at(50));
        assert_eq!(read_at(200), Vec::<u8>::new());
    }
}
