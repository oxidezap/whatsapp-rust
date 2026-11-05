use anyhow::{Result, anyhow};
use base64::Engine;
use serde::Deserialize;
use wacore::download::MediaType;
use wacore::net::WHATSAPP_WEB_ORIGIN;
use wacore::sync_marker::MaybeSend;

use crate::client::Client;
use crate::http::{HttpRequest, HttpResponse, HttpStatusError};
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
        .with_header("Origin", WHATSAPP_WEB_ORIGIN)
}

fn build_resume_check_request(
    hostname: &str,
    upload_path: &str,
    auth: &str,
    token: &str,
) -> HttpRequest {
    let url = format!("https://{hostname}{upload_path}/{token}?auth={auth}&token={token}&resume=1");
    HttpRequest::post(url).with_header("Origin", WHATSAPP_WEB_ORIGIN)
}

fn upload_error_from_response(response: HttpResponse) -> anyhow::Error {
    let status = response.status_code;
    let context = match response.body_string() {
        Ok(body) => format!("Upload failed {status} body={body}"),
        Err(body_err) => {
            format!("Upload failed {status} and failed to read response body: {body_err}")
        }
    };
    // The status also goes in as a typed node, not only into the message: the
    // host-rotation loop below reads it via `is_media_auth_error`, and the
    // caller that ends up with `last_error` needs the same answer.
    HttpStatusError { status }.into_error(context)
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

/// Boxed future for the dyn-driven retry loop below. `Send` keeps the upload
/// futures spawnable on native; on wasm the `HttpClient` futures are `?Send`
/// (single-threaded runtime), so the bound is dropped there.
#[cfg(not(target_arch = "wasm32"))]
type BoxFut<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
type BoxFut<'a, T> = std::pin::Pin<Box<dyn Future<Output = T> + 'a>>;

// Only auto traits can join a `dyn Fn` bound, so the wasm variants must spell
// out the whole alias instead of borrowing `MaybeSend`.
#[cfg(not(target_arch = "wasm32"))]
mod dyn_callbacks {
    use super::*;
    pub(super) type GetMediaConnDyn<'a> =
        dyn FnMut(bool) -> BoxFut<'a, Result<crate::mediaconn::MediaConn>> + Send + 'a;
    pub(super) type InvalidateMediaConnDyn<'a> = dyn FnMut() -> BoxFut<'a, ()> + Send + 'a;
    pub(super) type ExecuteRequestDyn<'a> =
        dyn FnMut(HttpRequest) -> BoxFut<'a, Result<HttpResponse>> + Send + 'a;
    pub(super) type SendBodyDyn<'a> =
        dyn FnMut(HttpRequest, u64, u64) -> BoxFut<'a, Result<HttpResponse>> + Send + 'a;
}
#[cfg(target_arch = "wasm32")]
mod dyn_callbacks {
    use super::*;
    pub(super) type GetMediaConnDyn<'a> =
        dyn FnMut(bool) -> BoxFut<'a, Result<crate::mediaconn::MediaConn>> + 'a;
    pub(super) type InvalidateMediaConnDyn<'a> = dyn FnMut() -> BoxFut<'a, ()> + 'a;
    pub(super) type ExecuteRequestDyn<'a> =
        dyn FnMut(HttpRequest) -> BoxFut<'a, Result<HttpResponse>> + 'a;
    pub(super) type SendBodyDyn<'a> =
        dyn FnMut(HttpRequest, u64, u64) -> BoxFut<'a, Result<HttpResponse>> + 'a;
}
use dyn_callbacks::*;

/// Drives host failover, auth refresh, and resumable upload. `file_length` is the
/// plaintext size (for the response); `ciphertext_len` is the encrypted blob size
/// (for the resume threshold/offsets). `send_body` transmits the body from a given
/// offset; `execute_request` serves the body-less resume check.
///
/// Thin adapter: boxes the per-call futures so the large driver body
/// instantiates once instead of per closure combination.
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
    media_profile: Option<wacore::download::MobileMediaHttpProfile>,
) -> Result<UploadResponse>
where
    GMC: FnMut(bool) -> GMCFut + MaybeSend,
    GMCFut: Future<Output = Result<crate::mediaconn::MediaConn>> + MaybeSend,
    IMC: FnMut() -> IMCFut + MaybeSend,
    IMCFut: Future<Output = ()> + MaybeSend,
    EXR: FnMut(HttpRequest) -> EXRFut + MaybeSend,
    EXRFut: Future<Output = Result<HttpResponse>> + MaybeSend,
    SB: FnMut(HttpRequest, u64, u64) -> SBFut + MaybeSend,
    SBFut: Future<Output = Result<HttpResponse>> + MaybeSend,
{
    upload_media_with_retry_dyn(
        crypto,
        media_type,
        file_length,
        ciphertext_len,
        media_key_timestamp,
        &mut |force| Box::pin(get_media_conn(force)),
        &mut || Box::pin(invalidate_media_conn()),
        &mut |request| Box::pin(execute_request(request)),
        &mut |request, offset, remaining| Box::pin(send_body(request, offset, remaining)),
        media_profile,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn upload_media_with_retry_dyn<'a>(
    crypto: UploadCrypto,
    media_type: MediaType,
    file_length: u64,
    ciphertext_len: u64,
    media_key_timestamp: i64,
    get_media_conn: &mut GetMediaConnDyn<'a>,
    invalidate_media_conn: &mut InvalidateMediaConnDyn<'a>,
    execute_request: &mut ExecuteRequestDyn<'a>,
    send_body: &mut SendBodyDyn<'a>,
    media_profile: Option<wacore::download::MobileMediaHttpProfile>,
) -> Result<UploadResponse> {
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(crypto.file_enc_sha256);
    let upload_path = media_profile
        .as_ref()
        .map(|p| p.upload_path(media_type))
        .unwrap_or_else(|| media_type.upload_path());
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
                let check_req = match &media_profile {
                    Some(p) => p.apply(check_req),
                    None => check_req,
                };
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

            let request = match &media_profile {
                Some(p) => p.apply(request),
                None => request,
            };
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

#[derive(Clone)]
#[non_exhaustive]
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

impl std::fmt::Debug for UploadResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UploadResponse")
            .field("url", &"<redacted>")
            .field("direct_path", &"<redacted>")
            .field("media_key", &"<redacted>")
            .field("file_enc_sha256", &self.file_enc_sha256)
            .field("file_sha256", &self.file_sha256)
            .field("file_length", &self.file_length)
            .field("media_key_timestamp", &self.media_key_timestamp)
            .field(
                "streaming_sidecar_len",
                &self.streaming_sidecar.as_ref().map(Vec::len),
            )
            .finish()
    }
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
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.upload", level = "debug", skip_all, fields(kind = ?media_type, len = data.len())))]
    pub async fn upload(
        &self,
        data: Vec<u8>,
        media_type: MediaType,
        options: UploadOptions,
    ) -> Result<UploadResponse> {
        async {
            let media_profile = self.mobile_media_http_profile()?;
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
            // Bytes so each retry/resume attempt slices with a refcount bump instead of
            // copying the whole (multi-MB) ciphertext per attempt.
            let ciphertext = bytes::Bytes::from(enc.data_to_upload);

            upload_media_with_retry(
                crypto,
                media_type,
                file_length,
                ciphertext_len,
                self.runtime.wall_time_secs(),
                |force| async move { self.refresh_media_conn(force).await.map_err(Into::into) },
                || async { self.invalidate_media_conn().await },
                |request| async move { self.http_client.execute(request).await },
                |request, offset, _remaining| {
                    let body = ciphertext.slice(offset as usize..);
                    async move { self.http_client.execute(request.with_body(body)).await }
                },
                media_profile,
            )
            .await
        }
        .await
        .inspect_err(|_error| {
            #[cfg(feature = "tracing")]
            tracing::error!(
                error = ?crate::download::MediaErrorDiagnostic(_error.as_ref()),
                "media upload failed"
            );
        })
    }

    /// Uploads already-encrypted media streamed from `source`, keeping memory
    /// constant regardless of file size. Encrypt the plaintext first with
    /// [`wacore::upload::encrypt_media_streaming`] (or `..._with_key`) into the
    /// storage of your choice, then pass that storage as `source` plus the
    /// returned [`wacore::upload::EncryptedMediaInfo`]. The caller owns where the
    /// ciphertext lives (temp file, memory, …); this method never touches disk.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.upload_stream", level = "debug", skip_all, fields(kind = ?media_type)))]
    pub async fn upload_stream<S>(
        &self,
        source: S,
        info: wacore::upload::EncryptedMediaInfo,
        media_type: MediaType,
    ) -> Result<UploadResponse>
    where
        S: wacore::upload::UploadSource + 'static,
    {
        async {
            let media_profile = self.mobile_media_http_profile()?;
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
                self.runtime.wall_time_secs(),
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
                media_profile,
            )
            .await
        }
        .await
        .inspect_err(|_error| {
            #[cfg(feature = "tracing")]
            tracing::error!(
                error = ?crate::download::MediaErrorDiagnostic(_error.as_ref()),
                "media upload failed"
            );
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn upload_response_debug_redacts_media_secrets() {
        let response = UploadResponse {
            url: "https://cdn.example.com/file?auth=synthetic-url-secret".into(),
            direct_path: "/file?sig=synthetic-path-secret".into(),
            media_key: [179; 32],
            file_enc_sha256: [1; 32],
            file_sha256: [2; 32],
            file_length: 12345,
            media_key_timestamp: 6789,
            streaming_sidecar: Some(vec![17; 10]),
        };
        for rendered in [format!("{response:?}"), format!("{response:#?}")] {
            assert!(!rendered.contains("synthetic-"), "{rendered}");
            assert!(!rendered.contains("179"), "{rendered}");
            assert!(rendered.contains("12345"), "{rendered}");
            assert!(rendered.contains("6789"), "{rendered}");
        }
        let converted: wacore::sticker_pack::MediaUploadInfo = response.into();
        assert_eq!(converted.media_key, [179; 32]);
        assert_eq!(converted.file_length, 12345);
    }

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
        HttpResponse::new(
            200,
            format!(r#"{{"url":"{url}","direct_path":"{direct_path}"}}"#).into_bytes(),
        )
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
                            Ok(HttpResponse::new(401, b"expired".to_vec()))
                        } else {
                            Ok(ok_json(
                                "https://cdn2.example.com/file",
                                "/v/t62.7118-24/123",
                            ))
                        }
                    }
                }
            },
            None,
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
                            Ok(HttpResponse::new(500, b"try another host".to_vec()))
                        } else {
                            Ok(ok_json(
                                "https://cdn2.example.com/file",
                                "/v/t62.7118-24/456",
                            ))
                        }
                    }
                }
            },
            None,
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

    /// Regression (#1193): upload put the host's status only into a formatted
    /// message, so a caller that ran out of hosts got "Upload failed 507 …" as
    /// text and no way to tell a throttle from a broken upstream by type.
    ///
    /// Every host refuses here, which is the case whose error actually reaches
    /// the caller — the failover tests above only cover the ones it recovers
    /// from.
    #[tokio::test]
    async fn the_upload_status_survives_to_the_public_error() {
        use crate::error::ErrorChainExt;

        let enc = wacore::upload::encrypt_media(b"nowhere to go", MediaType::Image)
            .expect("encryption should succeed");
        let len = enc.data_to_upload.len() as u64;
        let conn = media_conn("shared-auth", &["cdn1.example.com", "cdn2.example.com"]);

        let err = upload_media_with_retry(
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
            move |_request: HttpRequest, _offset, _remaining| async move {
                Ok(HttpResponse::new(507, b"insufficient storage".to_vec()))
            },
            None,
        )
        .await
        .expect_err("every host refusing must fail the upload");

        let cause: &(dyn std::error::Error + 'static) = err.as_ref();
        assert_eq!(
            cause.http_status(),
            Some(507),
            "the consumer must recover the host's status by type, got: {err:?}"
        );
        assert!(
            format!("{err}").contains("507"),
            "the message should still name the status, got: {err}"
        );
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
                Ok(HttpResponse::new(
                    200,
                    format!(r#"{{"resume":"{offset}"}}"#).into_bytes(),
                ))
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
            None,
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

/// Media-only decorator: URL trust, redirects, streaming admission, byte budgets,
/// exact upload length and resource reporting retain the underlying contracts.
struct MobileMediaHttpClient {
    inner: std::sync::Arc<dyn wacore::net::HttpClient>,
    profile: wacore::download::MobileMediaHttpProfile,
}
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl wacore::net::HttpClient for MobileMediaHttpClient {
    async fn execute(&self, request: HttpRequest) -> Result<HttpResponse> {
        self.inner.execute(self.profile.apply(request)).await
    }
    fn supports_streaming(&self) -> bool {
        self.inner.supports_streaming()
    }
    fn streaming_admission(
        &self,
    ) -> Option<wacore::runtime::BoxFuture<'_, Result<wacore::net::StreamingRequestPermit>>> {
        self.inner.streaming_admission()
    }
    fn execute_streaming(
        &self,
        request: HttpRequest,
    ) -> Result<wacore::net::StreamingHttpResponse> {
        self.inner.execute_streaming(self.profile.apply(request))
    }
    fn supports_upload_streaming(&self) -> bool {
        self.inner.supports_upload_streaming()
    }
    fn execute_upload(
        &self,
        request: HttpRequest,
        body: wacore::net::UploadBody,
        content_length: u64,
    ) -> Result<HttpResponse> {
        self.inner
            .execute_upload(self.profile.apply(request), body, content_length)
    }
    fn resource_report(&self) -> Option<wacore::stats::HttpResourceReport> {
        self.inner.resource_report()
    }
}
pub(crate) fn profile_media_http_client(
    inner: std::sync::Arc<dyn wacore::net::HttpClient>,
    profile: Option<wacore::download::MobileMediaHttpProfile>,
) -> std::sync::Arc<dyn wacore::net::HttpClient> {
    match profile {
        Some(profile) => std::sync::Arc::new(MobileMediaHttpClient { inner, profile }),
        None => inner,
    }
}
impl Client {
    pub(crate) fn mobile_media_http_profile(
        &self,
    ) -> Result<Option<wacore::download::MobileMediaHttpProfile>> {
        #[cfg(feature = "mobile-core")]
        if let Some(session) = &self.mobile_session {
            session.validate_device(&self.persistence_manager.get_device_snapshot())?;
            return session
                .media_http_profile()
                .cloned()
                .map(Some)
                .ok_or_else(|| anyhow!("mobile media requires an explicit HTTP/routing profile"));
        }
        if self
            .persistence_manager
            .get_device_snapshot()
            .mobile_session
            .is_some()
        {
            return Err(anyhow!("mobile media requires a configured mobile session"));
        }
        Ok(None)
    }
    pub(crate) fn media_http_client(&self) -> Result<std::sync::Arc<dyn wacore::net::HttpClient>> {
        Ok(profile_media_http_client(
            self.http_client.clone(),
            self.mobile_media_http_profile()?,
        ))
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod mobile_media_http_tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use wacore::download::MobileMediaHttpProfile;
    use wacore::net::{HttpClient, StreamingHttpResponse, UploadBody};

    struct CaptureHttp {
        requests: Mutex<Vec<HttpRequest>>,
        download: Vec<u8>,
        streaming: bool,
    }
    impl CaptureHttp {
        fn capture(&self, request: HttpRequest) -> HttpResponse {
            let resume = request.url.contains("resume=1");
            self.requests.lock().unwrap().push(request);
            if resume {
                HttpResponse::new(200, br#"{"resume":"7"}"#.to_vec())
            } else {
                HttpResponse::new(
                    200,
                    br#"{"url":"https://cdn.invalid/file","direct_path":"/mms/sticker/file"}"#
                        .to_vec(),
                )
            }
        }
    }
    #[async_trait::async_trait]
    impl HttpClient for CaptureHttp {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse> {
            if request.method == "GET" {
                self.requests.lock().unwrap().push(request);
                Ok(HttpResponse::new(200, self.download.clone()))
            } else {
                Ok(self.capture(request))
            }
        }
        fn supports_streaming(&self) -> bool {
            self.streaming
        }
        fn execute_streaming(&self, request: HttpRequest) -> Result<StreamingHttpResponse> {
            self.requests.lock().unwrap().push(request);
            Ok(StreamingHttpResponse::new(
                200,
                Box::new(std::io::Cursor::new(self.download.clone())),
            ))
        }
        fn supports_upload_streaming(&self) -> bool {
            true
        }
        fn execute_upload(
            &self,
            request: HttpRequest,
            mut body: UploadBody,
            length: u64,
        ) -> Result<HttpResponse> {
            let copied = std::io::copy(&mut body, &mut std::io::sink())?;
            assert_eq!(copied, length);
            Ok(self.capture(request))
        }
    }
    fn assert_mobile_headers(request: &HttpRequest, origin: Option<&str>) {
        assert_eq!(
            request.headers.get("User-Agent").map(String::as_str),
            Some("WhatsApp/synthetic-mobile A")
        );
        assert_eq!(request.headers.get("Origin").map(String::as_str), origin);
        assert!(!request.headers.contains_key("origin"));
    }
    #[tokio::test]
    async fn mobile_buffered_and_streamed_upload_resume_use_profile_headers_and_routes() {
        for streaming in [false, true] {
            for origin in [None, Some("https://adapted.invalid")] {
                let http = Arc::new(CaptureHttp {
                    requests: Mutex::new(Vec::new()),
                    download: Vec::new(),
                    streaming: true,
                });
                let data = Arc::new(vec![19; RESUMABLE_UPLOAD_THRESHOLD + 32]);
                let profile = MobileMediaHttpProfile::new(
                    "WhatsApp/synthetic-mobile A".into(),
                    origin.map(str::to_owned),
                    vec![(MediaType::Sticker, "/mms/sticker".into())],
                )
                .unwrap();
                let result = upload_media_with_retry(
                    UploadCrypto {
                        media_key: [1; 32],
                        file_sha256: [2; 32],
                        file_enc_sha256: [3; 32],
                        streaming_sidecar: None,
                    },
                    MediaType::Sticker,
                    data.len() as u64,
                    data.len() as u64,
                    1,
                    |_| async {
                        Ok(crate::mediaconn::MediaConn {
                            auth: "synthetic".into(),
                            ttl: 60,
                            auth_ttl: None,
                            hosts: vec![crate::mediaconn::MediaConnHost::new("cdn.invalid".into())],
                            fetched_at: wacore::time::Instant::now(),
                        })
                    },
                    || async {},
                    {
                        let http = http.clone();
                        move |request| {
                            let http = http.clone();
                            async move { http.execute(request).await }
                        }
                    },
                    {
                        let http = http.clone();
                        let data = data.clone();
                        move |request, offset, remaining| {
                            let http = http.clone();
                            let bytes = data[offset as usize..].to_vec();
                            async move {
                                assert_eq!(bytes.len() as u64, remaining);
                                if streaming {
                                    http.execute_upload(
                                        request,
                                        Box::new(std::io::Cursor::new(bytes)),
                                        remaining,
                                    )
                                } else {
                                    http.execute(request.with_body(bytes)).await
                                }
                            }
                        }
                    },
                    Some(profile),
                )
                .await
                .unwrap();
                assert_eq!(result.direct_path, "/mms/sticker/file");
                let requests = http.requests.lock().unwrap();
                assert_eq!(requests.len(), 2);
                assert!(requests[0].url.contains("/mms/sticker/"));
                assert!(requests[0].url.contains("resume=1"));
                assert!(requests[1].url.contains("file_offset=7"));
                for request in requests.iter() {
                    assert_mobile_headers(request, origin);
                }
            }
        }
    }
    #[tokio::test]
    async fn mobile_download_profile_reaches_buffered_and_streamed_http_without_rewriting_url() {
        let profile =
            MobileMediaHttpProfile::native_mms("WhatsApp/synthetic-mobile A".into()).unwrap();
        let raw = Arc::new(CaptureHttp {
            requests: Mutex::new(Vec::new()),
            download: b"synthetic".to_vec(),
            streaming: true,
        });
        let http = profile_media_http_client(raw.clone(), Some(profile));
        let url = "https://cdn.invalid/mms/sticker/exact?sig=synthetic";
        let response = http.execute(HttpRequest::get(url)).await.unwrap();
        assert_eq!(response.body, b"synthetic");
        let mut response = http.execute_streaming(HttpRequest::get(url)).unwrap();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut response.body, &mut bytes).unwrap();
        assert_eq!(bytes, b"synthetic");
        assert!(http.supports_streaming());
        assert!(http.supports_upload_streaming());
        for request in raw.requests.lock().unwrap().iter() {
            assert_eq!(request.url, url);
            assert_mobile_headers(request, None);
        }
    }
    #[test]
    fn web_http_profile_is_the_original_adapter_and_request_defaults_stay_exact() {
        let raw: Arc<dyn HttpClient> = Arc::new(CaptureHttp {
            requests: Mutex::new(Vec::new()),
            download: Vec::new(),
            streaming: true,
        });
        let http = profile_media_http_client(raw.clone(), None);
        assert!(Arc::ptr_eq(&raw, &http));
        let upload = build_upload_request("cdn.invalid", "/mms/image", "auth", "token", None);
        assert_eq!(
            upload.headers.get("Origin").map(String::as_str),
            Some(WHATSAPP_WEB_ORIGIN)
        );
        assert!(!upload.headers.contains_key("User-Agent"));
    }
    #[tokio::test]
    async fn profiled_download_uses_real_decryption_and_preserves_host_validation() {
        for streaming in [false, true] {
            let plaintext = b"mobile CDN fixture with real media integrity";
            let encrypted = wacore::upload::encrypt_media_with_key(
                plaintext,
                MediaType::Sticker,
                Some(&[73; 32]),
            )
            .unwrap();
            let params = crate::download::DownloadParams::encrypted(
                "/mms/sticker/exact",
                &encrypted.media_key,
                &encrypted.file_sha256,
                &encrypted.file_enc_sha256,
                plaintext.len() as u64,
                MediaType::Sticker,
            )
            .unwrap();
            let raw = Arc::new(CaptureHttp {
                requests: Mutex::new(Vec::new()),
                download: encrypted.data_to_upload,
                streaming,
            });
            let downloader = crate::download::MediaDownloader::new(
                raw.clone(),
                Arc::new(crate::TokioRuntime),
                wacore::download::MediaRoute::new(vec![wacore::download::MediaHost::new(
                    "cdn.invalid",
                )]),
            )
            .with_mobile_http_profile(
                MobileMediaHttpProfile::native_mms("WhatsApp/synthetic-mobile A".into()).unwrap(),
            );
            let bytes = downloader
                .download(&params, Default::default())
                .await
                .unwrap();
            assert_eq!(bytes, plaintext);
            let requests = raw.requests.lock().unwrap();
            assert_eq!(requests.len(), 1);
            assert!(
                requests[0]
                    .url
                    .starts_with("https://cdn.invalid/mms/sticker/exact?")
            );
            assert_mobile_headers(&requests[0], None);
            drop(requests);
            let mut unsafe_static = waproto::whatsapp::message::ImageMessage::default();
            unsafe_static.static_url = Some("https://untrusted.invalid/image".into());
            unsafe_static.file_sha256 = Some(vec![0; 32]);
            assert!(
                downloader
                    .download(&unsafe_static, Default::default())
                    .await
                    .is_err()
            );
            assert_eq!(raw.requests.lock().unwrap().len(), 1);
        }
    }
}
