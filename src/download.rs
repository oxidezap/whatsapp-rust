use crate::client::Client;
use crate::http::{
    HTTP_STATUS_GONE, HTTP_STATUS_NOT_FOUND, HTTP_STATUS_OK, HTTP_STATUS_REDIRECTION_START,
    HttpClient, HttpStatusError,
};
use crate::mediaconn::{MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS, MediaConn, is_media_auth_error};
use anyhow::{Result, anyhow};
use std::sync::Arc;
use wacore::runtime::Runtime;
use wacore::sync_marker::MaybeSend;

pub use wacore::download::{
    DEFAULT_MEDIA_HOSTS, DownloadUtils, DownloadWriter, Downloadable, MediaDecryption,
    MediaDecryptionError, MediaHost, MediaRoute, MediaType,
};

/// Cap on the speculative capacity pre-allocated for the in-memory download
/// buffer. Sized to the plaintext length the message declares, but a bogus
/// length must not drive a multi-GB allocation before a single byte arrives;
/// beyond this the buffer grows on demand. Comfortably above typical
/// image/video/audio media so the common case is a single allocation.
const DOWNLOAD_PREALLOC_CAP: u64 = 64 * 1024 * 1024;

impl From<&MediaConn> for MediaRoute {
    fn from(conn: &MediaConn) -> Self {
        MediaRoute::new(
            conn.hosts
                .iter()
                .map(|h| MediaHost::new(h.hostname.clone()))
                .collect(),
        )
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

/// Keep integrity metadata beside the public request, without changing the
/// fields consumers construct or match in MediaDecryption / DownloadRequest.
#[derive(Default)]
struct ExpectedMediaHashes {
    encrypted: Option<Vec<u8>>,
    plaintext: Option<Vec<u8>>,
}

impl ExpectedMediaHashes {
    fn from_downloadable(downloadable: &dyn Downloadable) -> Self {
        if downloadable.is_encrypted() {
            Self {
                encrypted: downloadable.file_enc_sha256().map(<[u8]>::to_vec),
                plaintext: downloadable.file_sha256().map(<[u8]>::to_vec),
            }
        } else {
            Self::default()
        }
    }
}

/// Why a media download failed, for callers that have no session to refresh.
///
/// [`Client`] downloads return [`ClientDownloadError`], which also represents
/// session acquisition/refresh failures. This downloader never asks for a session.
///
/// `Display` and `Debug` omit opaque cause text, which may contain signed URLs.
/// Inspect [`std::error::Error::source`] explicitly to recover the original cause.
#[derive(thiserror::Error)]
#[non_exhaustive]
pub enum MediaDownloadError {
    /// The CDN rejected the reference itself (401/403/404/410). The direct path
    /// or its token is expired or revoked; another host cannot serve it either.
    #[error("the CDN rejected the media reference: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    ReferenceRejected(#[source] anyhow::Error),
    /// Every host in the route failed for a reason other than the reference:
    /// transport failure, unexpected status, or a body that failed to verify.
    #[error("every media host failed: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    HostsUnreachable(#[source] anyhow::Error),
    /// The local destination could not be truncated, written or rewound.
    /// This is terminal: switching CDN hosts cannot repair the sink.
    #[error("local media writer failed: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    WriterIo(#[source] anyhow::Error),
    /// The route named no hosts, so nothing was ever contacted.
    #[error("the media route names no hosts")]
    NoHosts,
    /// No host was contacted and none could be: the reference is too incomplete
    /// to build a URL from, so the fix is the metadata, not the network.
    #[error("{:?}", MediaErrorDiagnostic(.0.as_ref()))]
    Other(#[from] anyhow::Error),
    /// Cleanup failed; `failure` preserves the original classification and source
    /// chain, while `cleanup` explains why the sink may retain unverified bytes.
    #[error("{failure}; failed to clear the writer: {:?}", .cleanup.kind())]
    WriterCleanup {
        #[source]
        failure: Box<MediaDownloadError>,
        cleanup: std::io::Error,
    },
}

/// Final failure of a [`Client`] download, after any applicable refresh and host
/// failover. Local sink failures stop immediately. Unlike [`MediaDownloadError`],
/// this includes the session operation needed to obtain a CDN route.
///
/// Like [`MediaDownloadError`], both diagnostic formats redact opaque causes;
/// the original errors remain accessible through the standard source chain.
#[derive(thiserror::Error)]
#[non_exhaustive]
pub enum ClientDownloadError {
    /// The reference is still rejected after any applicable refresh. Static
    /// URLs have no refreshable route, so their first rejection is final.
    #[error("the CDN rejected the media reference: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    ReferenceRejected(#[source] anyhow::Error),
    /// Every host failed (transport, status, or integrity); the last cause is
    /// retained, including its HTTP status or decryption error when available.
    #[error("every media host failed: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    HostsUnreachable(#[source] anyhow::Error),
    /// The local destination could not be truncated, written or rewound.
    /// No further host or media-session refresh is attempted for this failure.
    #[error("local media writer failed: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    WriterIo(#[source] anyhow::Error),
    /// The obtained route has no hosts. No HTTP request was executed.
    #[error("the media route names no hosts")]
    NoHosts,
    /// A forced refresh after rejection yielded no hosts. The prior rejection
    /// remains the source: unlike `NoHosts`, an HTTP exchange already occurred.
    #[error("the refreshed media route names no hosts after CDN rejection: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    NoHostsAfterRefresh(#[source] anyhow::Error),
    /// Metadata could not be turned into a request.
    #[error("could not prepare the media reference: {:?}", MediaErrorDiagnostic(.0.as_ref()))]
    Preparation(#[source] anyhow::Error),
    /// Failed to obtain a session route, or refresh it after a CDN rejection.
    /// The IQ error retains rejection metadata, timeout and transport causes.
    #[error("failed to obtain media session (forced refresh: {force_refresh}): {:?}", MediaErrorDiagnostic(.source))]
    MediaSession {
        force_refresh: bool,
        #[source]
        source: crate::request::IqError,
    },
    /// Failure cleanup was attempted but failed. Follow `failure` (also the
    /// standard source chain) for the download cause, and inspect `cleanup` for
    /// the sink error. The destination must not be treated as verified media.
    #[error("{failure}; failed to clear the writer: {:?}", .cleanup.kind())]
    WriterCleanup {
        #[source]
        failure: Box<ClientDownloadError>,
        cleanup: std::io::Error,
    },
}

// Opaque adapter errors may repeat signed URLs in any source or context. Keep
// those causes intact for callers, but only format typed facts in diagnostics.
pub(crate) struct MediaErrorDiagnostic<'a>(pub &'a (dyn std::error::Error + 'static));

impl std::fmt::Debug for MediaErrorDiagnostic<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use crate::error::ErrorChainExt;
        use crate::request::IqError;
        let io_kind = ErrorChainExt::sources(self.0)
            .find_map(|e| e.downcast_ref::<std::io::Error>())
            .map(std::io::Error::kind);
        let media_validation =
            ErrorChainExt::sources(self.0).any(|e| e.is::<MediaDecryptionError>());
        let iq_kind = ErrorChainExt::sources(self.0)
            .find_map(|e| e.downcast_ref::<IqError>())
            .map(|e| match e {
                IqError::Timeout => "timeout",
                IqError::NotConnected => "not_connected",
                IqError::Socket(_) => "socket",
                IqError::EncryptSend(_) => "encrypt_send",
                IqError::ClientState(_) => "client_state",
                IqError::Disconnected(_) => "disconnected",
                IqError::ServerError { .. } => "server_rejection",
                IqError::UnexpectedResponseType { .. } => "unexpected_response_type",
                IqError::InternalChannelClosed => "channel_closed",
                IqError::Unclassified(_) => "unclassified",
                IqError::DuplicateRequestId(_) => "duplicate_request_id",
                IqError::EncodeError(_) => "encode",
                IqError::ParseError(_) => "parse",
            });
        let iq_code = self.0.server_rejection().map(|rejection| rejection.code);
        let preparation = ErrorChainExt::sources(self.0)
            .find_map(|e| e.downcast_ref::<wacore::download::DownloadPreparationError>());
        f.debug_struct("MediaFailure")
            .field("preparation", &preparation)
            .field("iq_kind", &iq_kind)
            .field("iq_code", &iq_code)
            .field("http_status", &self.0.http_status())
            .field("io_kind", &io_kind)
            .field("media_validation", &media_validation)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for MediaDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReferenceRejected(e) => f
                .debug_tuple("ReferenceRejected")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::HostsUnreachable(e) => f
                .debug_tuple("HostsUnreachable")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::WriterIo(e) => f
                .debug_tuple("WriterIo")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::Other(e) => f
                .debug_tuple("Other")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::NoHosts => f.write_str("NoHosts"),
            Self::WriterCleanup { failure, cleanup } => f
                .debug_struct("WriterCleanup")
                .field("failure", failure)
                .field("cleanup_kind", &cleanup.kind())
                .finish(),
        }
    }
}

impl std::fmt::Debug for ClientDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReferenceRejected(e) => f
                .debug_tuple("ReferenceRejected")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::HostsUnreachable(e) => f
                .debug_tuple("HostsUnreachable")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::WriterIo(e) => f
                .debug_tuple("WriterIo")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::NoHostsAfterRefresh(e) => f
                .debug_tuple("NoHostsAfterRefresh")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::Preparation(e) => f
                .debug_tuple("Preparation")
                .field(&MediaErrorDiagnostic(e.as_ref()))
                .finish(),
            Self::NoHosts => f.write_str("NoHosts"),
            Self::MediaSession {
                force_refresh,
                source,
            } => f
                .debug_struct("MediaSession")
                .field("force_refresh", force_refresh)
                .field("source", &MediaErrorDiagnostic(source))
                .finish(),
            Self::WriterCleanup { failure, cleanup } => f
                .debug_struct("WriterCleanup")
                .field("failure", failure)
                .field("cleanup_kind", &cleanup.kind())
                .finish(),
        }
    }
}

impl From<DownloadRequestError> for ClientDownloadError {
    fn from(err: DownloadRequestError) -> Self {
        match err {
            DownloadRequestError::Auth(e) | DownloadRequestError::NotFound(e) => {
                Self::ReferenceRejected(e)
            }
            DownloadRequestError::Other(e) => Self::HostsUnreachable(e),
            DownloadRequestError::WriterIo(e) => Self::WriterIo(e),
            DownloadRequestError::Prepare(e) => Self::Preparation(e),
            DownloadRequestError::NoHosts => Self::NoHosts,
            DownloadRequestError::NoHostsAfterRefresh(e) => Self::NoHostsAfterRefresh(e),
            DownloadRequestError::Session {
                force_refresh,
                source,
            } => Self::MediaSession {
                force_refresh,
                source,
            },
            DownloadRequestError::Cleanup { failure, cleanup } => Self::WriterCleanup {
                failure: Box::new((*failure).into()),
                cleanup,
            },
        }
    }
}

impl From<DownloadRequestError> for MediaDownloadError {
    fn from(err: DownloadRequestError) -> Self {
        match err {
            DownloadRequestError::Auth(e) | DownloadRequestError::NotFound(e) => {
                Self::ReferenceRejected(e)
            }
            DownloadRequestError::Other(e) => Self::HostsUnreachable(e),
            DownloadRequestError::WriterIo(e) => Self::WriterIo(e),
            DownloadRequestError::Prepare(e) => Self::Other(e),
            DownloadRequestError::NoHosts => Self::NoHosts,
            DownloadRequestError::Session { source, .. } => Self::Other(source.into()),
            DownloadRequestError::NoHostsAfterRefresh(e) => Self::Other(e),
            DownloadRequestError::Cleanup { failure, cleanup } => Self::WriterCleanup {
                failure: Box::new((*failure).into()),
                cleanup,
            },
        }
    }
}

#[derive(Debug)]
enum DownloadRequestError {
    Auth(anyhow::Error),
    /// 404/410 — media URL expired or not found. May need refreshed hosts.
    /// Matches WA Web's `MediaNotFoundError` handling.
    NotFound(anyhow::Error),
    Other(anyhow::Error),
    WriterIo(anyhow::Error),
    /// The request list could not be built, so no host was ever contacted and
    /// no host ever could be: the reference itself is incomplete.
    Prepare(anyhow::Error),
    /// No request was ever executed: the route carried no hosts.
    NoHosts,
    NoHostsAfterRefresh(anyhow::Error),
    Session {
        force_refresh: bool,
        source: crate::request::IqError,
    },
    Cleanup {
        failure: Box<DownloadRequestError>,
        cleanup: std::io::Error,
    },
}

impl DownloadRequestError {
    fn auth(status_code: u16) -> Self {
        Self::Auth(Self::refused(
            status_code,
            format!("Download failed with status: {status_code}"),
        ))
    }

    fn not_found(status_code: u16) -> Self {
        Self::NotFound(Self::refused(
            status_code,
            format!("Download media not found/expired with status: {status_code}"),
        ))
    }

    /// A status this path does not act on itself (429, 5xx, …). Still carries
    /// the status: the retry loop ignoring it does not mean the caller will,
    /// and "back off" and "upstream is broken" are its calls to make.
    fn refused_status(status_code: u16) -> Self {
        Self::Other(Self::refused(
            status_code,
            format!("Download failed with status: {status_code}"),
        ))
    }

    /// The message stays what it was; the status also becomes a typed node in
    /// the chain so [`ErrorChainExt::http_status`] can recover it instead of a
    /// consumer parsing this text.
    ///
    /// [`ErrorChainExt::http_status`]: crate::error::ErrorChainExt::http_status
    fn refused(status_code: u16, context: String) -> anyhow::Error {
        HttpStatusError {
            status: status_code,
        }
        .into_error(context)
    }

    /// For failures with no HTTP status at all — a socket that never connected,
    /// a body that would not decrypt. `http_status()` reports `None` for these,
    /// which is how a caller tells our bug from the CDN's.
    fn other(err: impl Into<anyhow::Error>) -> Self {
        Self::Other(err.into())
    }

    fn writer_io(error: std::io::Error) -> Self {
        Self::WriterIo(error.into())
    }

    fn streamed(error: anyhow::Error) -> Self {
        let tagged = error
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .is_some_and(|cause| cause.is::<WriterIoCause>());
        if !tagged {
            return Self::Other(error);
        }
        // Strip the private origin tag: direct downcasts must expose the same
        // original OS code and message as buffered writer failures.
        match error.downcast::<std::io::Error>() {
            Ok(error) => {
                let kind = error.kind();
                match error
                    .into_inner()
                    .map(|inner| inner.downcast::<WriterIoCause>())
                {
                    Some(Ok(cause)) => Self::writer_io(cause.0),
                    // Defensive fallbacks retain available causes without a
                    // panic; the immutable tag check above excludes them.
                    Some(Err(cause)) => Self::writer_io(std::io::Error::new(kind, cause)),
                    None => Self::writer_io(kind.into()),
                }
            }
            Err(error) => Self::WriterIo(error),
        }
    }

    fn is_auth(&self) -> bool {
        matches!(self, Self::Auth(_))
    }

    /// Returns true for 404/410 (expired URL) — should trigger auth refresh like auth errors.
    fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound(_))
    }

    fn no_hosts(previous_rejection: Option<anyhow::Error>) -> Self {
        match previous_rejection {
            Some(cause) => Self::NoHostsAfterRefresh(cause),
            None => Self::NoHosts,
        }
    }

    fn into_anyhow(self) -> anyhow::Error {
        match self {
            Self::Auth(err)
            | Self::NotFound(err)
            | Self::Other(err)
            | Self::WriterIo(err)
            | Self::Prepare(err) => err,
            Self::NoHosts => anyhow!("Failed to download from all available media hosts"),
            Self::Session { source, .. } => source.into(),
            Self::NoHostsAfterRefresh(e) => ClientDownloadError::NoHostsAfterRefresh(e).into(),
            Self::Cleanup { failure, cleanup } => {
                anyhow::Error::new(ClientDownloadError::WriterCleanup {
                    failure: Box::new((*failure).into()),
                    cleanup,
                })
            }
        }
    }
}

fn validate_download_status(status_code: u16) -> std::result::Result<(), DownloadRequestError> {
    if status_code < HTTP_STATUS_REDIRECTION_START {
        return Ok(());
    }

    let error = if is_media_auth_error(status_code) {
        DownloadRequestError::auth(status_code)
    } else if matches!(status_code, HTTP_STATUS_NOT_FOUND | HTTP_STATUS_GONE) {
        DownloadRequestError::not_found(status_code)
    } else {
        DownloadRequestError::refused_status(status_code)
    };
    Err(error)
}

fn decrypt_or_validate_buffered_body(
    body: &mut Vec<u8>,
    decryption: &MediaDecryption,
    hashes: &ExpectedMediaHashes,
) -> std::result::Result<(), DownloadRequestError> {
    match decryption {
        MediaDecryption::Encrypted {
            media_key,
            media_type,
        } => DownloadUtils::verify_and_decrypt_in_place_with_hashes(
            body,
            media_key,
            *media_type,
            hashes.encrypted.as_deref(),
            hashes.plaintext.as_deref(),
        )
        .map_err(DownloadRequestError::other),
        MediaDecryption::Plaintext { file_sha256 } => {
            DownloadUtils::validate_plaintext_sha256(body, file_sha256)
                .map_err(DownloadRequestError::other)
        }
    }
}

/// Auth-refresh + host-failover retry loop that returns the decrypted bytes.
/// Unlike [`download_to_writer_with_retry`] each attempt gets a FRESH buffer
/// (the executor allocates its own), so a failed host that wrote a longer body
/// (e.g. a CDN error page that decrypts to more bytes before its MAC fails)
/// can't leave a stale tail behind a shorter successful retry.
///
/// `max_refresh_attempts` is 0 for a caller with no media conn to refresh: the
/// URLs it would re-derive are the ones that just failed.
async fn download_media_with_retry<
    PrepareRequests,
    PrepareRequestsFut,
    InvalidateMediaConn,
    InvalidateMediaConnFut,
    ExecuteRequest,
    ExecuteRequestFut,
>(
    max_refresh_attempts: usize,
    mut prepare_requests: PrepareRequests,
    mut invalidate_media_conn: InvalidateMediaConn,
    mut execute_request: ExecuteRequest,
) -> std::result::Result<Vec<u8>, DownloadRequestError>
where
    PrepareRequests: FnMut(bool) -> PrepareRequestsFut,
    PrepareRequestsFut: Future<
        Output = std::result::Result<Vec<wacore::download::DownloadRequest>, DownloadRequestError>,
    >,
    InvalidateMediaConn: FnMut() -> InvalidateMediaConnFut,
    InvalidateMediaConnFut: Future<Output = ()>,
    ExecuteRequest: FnMut(wacore::download::DownloadRequest) -> ExecuteRequestFut,
    ExecuteRequestFut: Future<Output = std::result::Result<Vec<u8>, DownloadRequestError>>,
{
    let mut force_refresh = false;
    let mut last_err: Option<anyhow::Error> = None;
    let mut last_rejection = None;

    for attempt in 0..=max_refresh_attempts {
        let requests = prepare_requests(force_refresh).await?;
        if requests.is_empty() {
            return Err(DownloadRequestError::no_hosts(last_rejection));
        }
        let mut retry_with_fresh_route = false;

        for request in requests {
            match execute_request(request.clone()).await {
                Ok(data) => return Ok(data),
                Err(err @ DownloadRequestError::WriterIo(_)) => return Err(err),
                Err(err)
                    if (err.is_auth() || err.is_not_found()) && attempt < max_refresh_attempts =>
                {
                    // An empty refreshed route is not the same as never making
                    // a request: retain the rejection that triggered this refresh.
                    last_rejection = Some(err.into_anyhow());
                    invalidate_media_conn().await;
                    force_refresh = true;
                    retry_with_fresh_route = true;
                    break;
                }
                Err(err) if err.is_auth() || err.is_not_found() => return Err(err),
                Err(err) => {
                    let err = err.into_anyhow();
                    // Transport error text can include the signed URL too.
                    // Preserve the cause for callers without printing it here.
                    log_download_retry(&request, &err);
                    last_err = Some(err);
                }
            }
        }

        if !retry_with_fresh_route {
            break;
        }
    }

    match last_err {
        Some(err) => Err(DownloadRequestError::Other(err)),
        None => Err(DownloadRequestError::NoHosts),
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
    max_refresh_attempts: usize,
    runtime: &Arc<dyn Runtime>,
    mut writer: W,
    mut prepare_requests: PrepareRequests,
    mut invalidate_media_conn: InvalidateMediaConn,
    mut execute_request: ExecuteRequest,
) -> std::result::Result<W, DownloadRequestError>
where
    W: DownloadWriter + MaybeSend + 'static,
    PrepareRequests: FnMut(bool) -> PrepareRequestsFut,
    PrepareRequestsFut: Future<
        Output = std::result::Result<Vec<wacore::download::DownloadRequest>, DownloadRequestError>,
    >,
    InvalidateMediaConn: FnMut() -> InvalidateMediaConnFut,
    InvalidateMediaConnFut: Future<Output = ()>,
    ExecuteRequest: FnMut(wacore::download::DownloadRequest, W) -> ExecuteRequestFut,
    ExecuteRequestFut: Future<Output = Result<(W, std::result::Result<(), DownloadRequestError>)>>,
{
    let mut force_refresh = false;
    let mut last_err: Option<anyhow::Error> = None;
    let mut last_rejection = None;

    for attempt in 0..=max_refresh_attempts {
        let requests = match prepare_requests(force_refresh).await {
            Ok(requests) => requests,
            Err(err) => {
                return Err(discard_failed_write(runtime, writer, err).await);
            }
        };
        if requests.is_empty() {
            let failure = DownloadRequestError::no_hosts(last_rejection);
            return Err(discard_failed_write(runtime, writer, failure).await);
        }
        let mut retry_with_fresh_route = false;

        for request in requests {
            let (next_writer, result) = match execute_request(request.clone(), writer).await {
                Ok(outcome) => outcome,
                // The writer went into the failed executor and did not come back,
                // so there is nothing left here to clean up.
                Err(err) => return Err(DownloadRequestError::Other(err)),
            };
            writer = next_writer;

            match result {
                Ok(()) => return Ok(writer),
                Err(err @ DownloadRequestError::WriterIo(_)) => {
                    return Err(discard_failed_write(runtime, writer, err).await);
                }
                Err(err)
                    if (err.is_auth() || err.is_not_found()) && attempt < max_refresh_attempts =>
                {
                    last_rejection = Some(err.into_anyhow());
                    invalidate_media_conn().await;
                    force_refresh = true;
                    retry_with_fresh_route = true;
                    break;
                }
                Err(err) if err.is_auth() || err.is_not_found() => {
                    return Err(discard_failed_write(runtime, writer, err).await);
                }
                Err(err) => {
                    let err = err.into_anyhow();
                    log_download_retry(&request, &err);
                    last_err = Some(err);
                }
            }
        }

        if !retry_with_fresh_route {
            break;
        }
    }

    let failure = match last_err {
        Some(err) => DownloadRequestError::Other(err),
        None => DownloadRequestError::NoHosts,
    };
    Err(discard_failed_write(runtime, writer, failure).await)
}

/// Fetch one prepared request into memory, decrypting as it goes when the HTTP
/// client can stream and reusing the buffered response allocation when it can't.
async fn execute_request_into_memory(
    http_client: &Arc<dyn HttpClient>,
    runtime: &Arc<dyn Runtime>,
    request: &wacore::download::DownloadRequest,
    hashes: ExpectedMediaHashes,
    capacity: usize,
) -> std::result::Result<Vec<u8>, DownloadRequestError> {
    if http_client.supports_streaming() {
        let writer = std::io::Cursor::new(Vec::with_capacity(capacity));
        match streaming_download_and_decrypt(http_client, runtime, request, hashes, writer).await {
            Ok((writer, Ok(()))) => Ok(writer.into_inner()),
            Ok((_, Err(e))) => Err(e),
            Err(e) => Err(DownloadRequestError::other(e)),
        }
    } else {
        buffered_download_to_vec(http_client, runtime, request, hashes).await
    }
}

/// Speculative capacity for one download attempt, from the declared plaintext
/// length.
fn download_capacity(downloadable: &dyn Downloadable) -> usize {
    downloadable
        .file_length()
        .unwrap_or(0)
        .min(DOWNLOAD_PREALLOC_CAP) as usize
}

/// Downloads media from the CDN with no connected [`Client`] behind it.
///
/// Everything a download needs beyond the CDN hosts already lives in the
/// [`Downloadable`] itself, and the hosts are injected here rather than fetched,
/// so persisted references stay usable after the session is gone. A live client
/// keeps asking the server for its hosts; this is the path for callers that have
/// no session to ask with.
pub struct MediaDownloader {
    http_client: Arc<dyn HttpClient>,
    runtime: Arc<dyn Runtime>,
    route: MediaRoute,
}

/// The refresh budget for a caller with no media conn behind it. Its
/// counterpart is [`MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS`], which the `Client`
/// paths pass.
const NO_MEDIA_CONN_REFRESH: usize = 0;

fn client_refresh_budget(downloadable: &dyn Downloadable) -> usize {
    // Rebuilding a static URL cannot change it; refreshing a session would be
    // wasted work and can mask the CDN rejection with a disconnected-IQ error.
    if downloadable.static_url().is_some() {
        NO_MEDIA_CONN_REFRESH
    } else {
        MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS
    }
}

impl MediaDownloader {
    pub fn new(
        http_client: Arc<dyn HttpClient>,
        runtime: Arc<dyn Runtime>,
        route: MediaRoute,
    ) -> Self {
        Self {
            http_client,
            runtime,
            route,
        }
    }

    /// [`Self::new`] over [`MediaRoute::default_hosts`].
    pub fn with_default_hosts(http_client: Arc<dyn HttpClient>, runtime: Arc<dyn Runtime>) -> Self {
        Self::new(http_client, runtime, MediaRoute::default_hosts())
    }

    pub fn route(&self) -> &MediaRoute {
        &self.route
    }

    /// Mirrors [`Client::download`], minus the media-conn refresh: there is no
    /// session to refresh hosts from, so a rejected reference is terminal.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.media.download_via_route",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub async fn download(
        &self,
        downloadable: &dyn Downloadable,
    ) -> std::result::Result<Vec<u8>, MediaDownloadError> {
        let capacity = download_capacity(downloadable);
        download_media_with_retry(
            NO_MEDIA_CONN_REFRESH,
            |_force| async {
                DownloadUtils::prepare_download_requests(downloadable, &self.route)
                    .map_err(DownloadRequestError::Prepare)
            },
            || async {},
            |request| async move {
                execute_request_into_memory(
                    &self.http_client,
                    &self.runtime,
                    &request,
                    ExpectedMediaHashes::from_downloadable(downloadable),
                    capacity,
                )
                .await
            },
        )
        .await
        .map_err(MediaDownloadError::from)
    }

    /// Mirrors [`Client::download_to_writer`], including authenticated success
    /// and best-effort failure cleanup. A failed cleanup is reported separately.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.media.download_via_route_to_writer",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    ///
    /// Writers must be `Send` on native targets, where processing runs on the
    /// blocking pool. On wasm32, local writers are processed inline.
    pub async fn download_to_writer<W: DownloadWriter + MaybeSend + 'static>(
        &self,
        downloadable: &dyn Downloadable,
        writer: W,
    ) -> std::result::Result<W, MediaDownloadError> {
        download_to_writer_with_retry(
            NO_MEDIA_CONN_REFRESH,
            &self.runtime,
            writer,
            |_force| async {
                DownloadUtils::prepare_download_requests(downloadable, &self.route)
                    .map_err(DownloadRequestError::Prepare)
            },
            || async {},
            |request, writer| async move {
                streaming_download_and_decrypt(
                    &self.http_client,
                    &self.runtime,
                    &request,
                    ExpectedMediaHashes::from_downloadable(downloadable),
                    writer,
                )
                .await
            },
        )
        .await
        .map_err(MediaDownloadError::from)
    }
}

impl Client {
    /// Downloads and decrypts media from WhatsApp's CDN into memory.
    ///
    /// The canonical buffered entry for both message types and [`DownloadParams`].
    /// Pass parameters directly as `client.download(&params).await`.
    /// Returns a [`ClientDownloadError`] only after applicable refresh/failover;
    /// static URLs need no session and cannot benefit from refreshing one.
    ///
    /// Only needed when you need the plaintext bytes (processing, transcoding,
    /// re-upload). To forward existing media unchanged, reuse the original
    /// message's CDN fields directly, no round-trip required.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.media.download", level = "debug", skip_all, err(Debug))
    )]
    pub async fn download(
        &self,
        downloadable: &dyn Downloadable,
    ) -> std::result::Result<Vec<u8>, ClientDownloadError> {
        // Each attempt owns a fresh buffer, so failed hosts cannot leave a stale
        // tail behind a shorter retry. Streaming clients decrypt directly into a
        // pre-sized output. Buffered clients already paid for a complete response
        // Vec, so authenticate and decrypt that allocation in place instead of
        // keeping a second file-sized output alive beside it.
        let capacity = download_capacity(downloadable);
        download_media_with_retry(
            client_refresh_budget(downloadable),
            |force| self.prepare_requests(downloadable, force),
            || async { self.invalidate_media_conn().await },
            |request| async move {
                execute_request_into_memory(
                    &self.http_client,
                    &self.runtime,
                    &request,
                    ExpectedMediaHashes::from_downloadable(downloadable),
                    capacity,
                )
                .await
            },
        )
        .await
        .map_err(ClientDownloadError::from)
    }

    /// Fetch a first-party sticker pack's metadata and sticker list from the CDN.
    ///
    /// Each returned [`wacore::sticker_pack::StickerPackItem`] is [`Downloadable`],
    /// so individual stickers can be fetched with [`Self::download`]. The locale
    /// only affects localized pack names; `"en"` mirrors whatsmeow's default.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.media.fetch_sticker_pack", level = "debug", skip_all,)
    )]
    pub async fn fetch_sticker_pack(
        &self,
        pack_id: &str,
        locale: &str,
    ) -> Result<wacore::sticker_pack::StickerPack> {
        async {
            let url = wacore::sticker_pack::sticker_pack_data_url(pack_id, locale);
            let response = self
                .http_client
                .execute(crate::http::HttpRequest::get(&url))
                .await
                .map_err(|e| e.context("sticker pack request failed"))?;
            if response.status_code != HTTP_STATUS_OK {
                let status = response.status_code;
                return Err(HttpStatusError { status }
                    .into_error(format!("sticker pack endpoint returned status {status}")));
            }
            wacore::sticker_pack::parse_sticker_pack_response(&response.body)
        }
        .await
        .inspect_err(|_error| {
            #[cfg(feature = "tracing")]
            tracing::error!(
                error = ?MediaErrorDiagnostic(_error.as_ref()),
                "sticker pack fetch failed"
            );
        })
    }

    async fn prepare_requests(
        &self,
        downloadable: &dyn Downloadable,
        force_refresh: bool,
    ) -> std::result::Result<Vec<wacore::download::DownloadRequest>, DownloadRequestError> {
        // A static URL is fetched verbatim, so the media-conn IQ would be a round
        // trip whose answer is discarded before a byte of it is read.
        let route =
            if downloadable.static_url().is_some() {
                MediaRoute::new(Vec::new())
            } else {
                MediaRoute::from(&self.refresh_media_conn(force_refresh).await.map_err(
                    |source| DownloadRequestError::Session {
                        force_refresh,
                        source,
                    },
                )?)
            };
        DownloadUtils::prepare_download_requests(downloadable, &route)
            .map_err(DownloadRequestError::Prepare)
    }

    /// Downloads authenticated media into a truncatable writer. The canonical
    /// writer entry for both message types and [`DownloadParams`]. Pass parameters
    /// directly as `client.download_to_writer(&params, writer).await`.
    ///
    /// With a streaming HTTP adapter, download, decryption and writes happen in
    /// a single blocking task. Without streaming, the HTTP response is buffered
    /// before in-place decryption and writing. Success rewinds to position 0.
    ///
    /// On success the writer holds exactly the decrypted media and nothing else.
    /// Every attempt starts by emptying it, so neither content the caller left
    /// behind nor a host that streamed out plaintext before failing its MAC can
    /// survive into the result. Providing that is what [`DownloadWriter`] is for,
    /// and why this does not take a plain `Write + Seek`.
    ///
    /// On failure the writer is emptied too, on a best-effort basis: a sink that
    /// refuses to empty produces [`ClientDownloadError::WriterCleanup`], retaining
    /// both errors. Cancellation or an executor panic cannot guarantee cleanup
    /// or a returned error. A separately retained handle may then expose
    /// unverified bytes; do not publish/read the sink before successful completion.
    ///
    /// Streaming uses bounded read/crypto state (~40KB, excluding the sink).
    /// A non-streaming adapter necessarily buffers the full response.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.media.download_to_writer",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    ///
    /// Writers must be `Send` on native targets, where processing runs on the
    /// blocking pool. On wasm32, local writers are processed inline.
    pub async fn download_to_writer<W: DownloadWriter + MaybeSend + 'static>(
        &self,
        downloadable: &dyn Downloadable,
        writer: W,
    ) -> std::result::Result<W, ClientDownloadError> {
        download_to_writer_with_retry(
            client_refresh_budget(downloadable),
            &self.runtime,
            writer,
            |force| self.prepare_requests(downloadable, force),
            || async { self.invalidate_media_conn().await },
            |request, writer| async move {
                streaming_download_and_decrypt(
                    &self.http_client,
                    &self.runtime,
                    &request,
                    ExpectedMediaHashes::from_downloadable(downloadable),
                    writer,
                )
                .await
            },
        )
        .await
        .map_err(ClientDownloadError::from)
    }
}

// Crypto helpers return anyhow errors for both reader and writer I/O. Mark
// only the sink boundary so a network read error (even the same ErrorKind) is
// never mistaken for a local destination failure. The original I/O error is
// retained as a source; bytes are forwarded without buffering or copying.
#[derive(Debug, thiserror::Error)]
#[error("writer I/O failed: {0}")]
struct WriterIoCause(#[source] std::io::Error);

struct WriterIoOrigin<'a, W>(&'a mut W);

fn tag_writer_io(error: std::io::Error) -> std::io::Error {
    std::io::Error::new(error.kind(), WriterIoCause(error))
}

impl<W: std::io::Write> std::io::Write for WriterIoOrigin<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.write(bytes).map_err(tag_writer_io)
    }

    fn write_all(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.0.write_all(bytes).map_err(tag_writer_io)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.0.flush().map_err(tag_writer_io)
    }
}

/// Empty a writer and rewind it, so whatever is written next is all it holds.
///
/// Emptying rather than only rewinding is what makes the length of a finished
/// attempt knowable. It also keeps the guarantee independent of how the sink
/// treats position: a [`std::fs::File`] opened for appending ignores seeks and
/// writes at the end, so only a sink whose end has been brought back to zero
/// puts an append-mode write where the media belongs.
fn clear_writer<W: DownloadWriter>(writer: &mut W) -> std::io::Result<()> {
    writer.truncate(0)?;
    writer.rewind()?;
    Ok(())
}

/// Rewind a verified attempt for the caller to read back.
///
/// No truncation here: the attempt began on a sink [`clear_writer`] had emptied,
/// so the bytes it wrote are the only bytes present. That is what lets a host
/// which streamed out plaintext before failing its MAC be replaced by a shorter
/// one without leaving a tail behind the media.
fn finish_verified_write<W: DownloadWriter>(
    writer: &mut W,
) -> std::result::Result<(), DownloadRequestError> {
    writer.rewind().map_err(DownloadRequestError::writer_io)
}

/// Empty a writer whose download is not coming back.
///
/// Runs on the blocking pool because it can reach a filesystem — or a
/// third-party [`DownloadWriter`] — and this is the async retry future, which
/// shares its runtime with the read loop.
///
/// Retain both failures if cleanup itself is rejected by the sink. The original
/// classification remains available instead of being overwritten by an I/O error.
async fn discard_failed_write<W: DownloadWriter + MaybeSend + 'static>(
    runtime: &Arc<dyn Runtime>,
    mut writer: W,
    failure: DownloadRequestError,
) -> DownloadRequestError {
    match wacore::runtime::blocking(&**runtime, move || clear_writer(&mut writer)).await {
        Ok(()) => failure,
        Err(cleanup) => DownloadRequestError::Cleanup {
            failure: Box::new(failure),
            cleanup,
        },
    }
}

/// Download + decrypt to a writer. Uses streaming when available,
/// falls back to buffered otherwise. Returns writer for retry.
async fn streaming_download_and_decrypt<W: DownloadWriter + MaybeSend + 'static>(
    http_client: &Arc<dyn HttpClient>,
    runtime: &Arc<dyn Runtime>,
    request: &wacore::download::DownloadRequest,
    hashes: ExpectedMediaHashes,
    writer: W,
) -> Result<(W, std::result::Result<(), DownloadRequestError>)> {
    if !http_client.supports_streaming() {
        return buffered_download_and_decrypt(http_client, runtime, request, hashes, writer).await;
    }

    let http_client = http_client.clone();
    let url = request.url.clone();
    let decryption = request.decryption.clone();

    Ok(wacore::runtime::blocking(&**runtime, move || {
        let mut writer = writer;

        if let Err(e) = clear_writer(&mut writer) {
            return (writer, Err(DownloadRequestError::writer_io(e)));
        }

        let result = (|| -> std::result::Result<(), DownloadRequestError> {
            let http_request = crate::http::HttpRequest::get(url);
            let resp = http_client
                .execute_streaming(http_request)
                .map_err(DownloadRequestError::other)?;

            validate_download_status(resp.status_code)?;

            match &decryption {
                MediaDecryption::Encrypted {
                    media_key,
                    media_type,
                } => {
                    DownloadUtils::decrypt_stream_to_writer_with_hashes(
                        resp.body,
                        media_key,
                        *media_type,
                        hashes.encrypted.as_deref(),
                        hashes.plaintext.as_deref(),
                        &mut WriterIoOrigin(&mut writer),
                    )
                    .map_err(DownloadRequestError::streamed)?;
                }
                MediaDecryption::Plaintext { file_sha256 } => {
                    DownloadUtils::copy_and_validate_plaintext_to_writer(
                        resp.body,
                        file_sha256,
                        &mut WriterIoOrigin(&mut writer),
                    )
                    .map_err(DownloadRequestError::streamed)?;
                }
            }
            finish_verified_write(&mut writer)
        })();

        (writer, result)
    })
    .await)
}

/// Buffered fallback when streaming is not available.
async fn buffered_download_and_decrypt<W: DownloadWriter + MaybeSend + 'static>(
    http_client: &Arc<dyn HttpClient>,
    runtime: &Arc<dyn Runtime>,
    request: &wacore::download::DownloadRequest,
    hashes: ExpectedMediaHashes,
    writer: W,
) -> Result<(W, std::result::Result<(), DownloadRequestError>)> {
    let mut body = match buffered_download_body(http_client, request).await {
        Ok(body) => body,
        Err(err) => return Ok((writer, Err(err))),
    };
    let decryption = request.decryption.clone();

    // Keep authentication/decryption and writer I/O in one blocking task so
    // non-streaming writer downloads pay for a single executor round-trip.
    Ok(wacore::runtime::blocking(&**runtime, move || {
        let mut writer = writer;
        let result = (|| {
            decrypt_or_validate_buffered_body(&mut body, &decryption, &hashes)?;
            clear_writer(&mut writer).map_err(DownloadRequestError::writer_io)?;
            writer
                .write_all(&body)
                .map_err(DownloadRequestError::writer_io)?;
            finish_verified_write(&mut writer)
        })();

        (writer, result)
    })
    .await)
}

// Log only typed classifications: backend error strings may contain signed URLs.
fn log_download_retry(request: &wacore::download::DownloadRequest, error: &anyhow::Error) {
    let io_kind = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
        .map(std::io::Error::kind);
    let media_validation = error
        .chain()
        .any(|cause| cause.is::<MediaDecryptionError>());
    log::warn!(
        "Failed to download {request:?} (HTTP status {:?}, I/O kind {io_kind:?}, media validation failure {media_validation}). Trying next host.",
        crate::error::ErrorChainExt::http_status(&**error)
    );
}

async fn buffered_download_body(
    http_client: &Arc<dyn HttpClient>,
    request: &wacore::download::DownloadRequest,
) -> std::result::Result<Vec<u8>, DownloadRequestError> {
    let http_request = crate::http::HttpRequest::get(request.url.clone());
    let response = http_client
        .execute(http_request)
        .await
        .map_err(DownloadRequestError::other)?;
    validate_download_status(response.status_code)?;
    Ok(response.body)
}

/// Execute a non-streaming HTTP download and reuse the response allocation
/// for the final plaintext. This is especially important on WASM, where the
/// JS `Uint8Array` must already be copied into linear memory at the FFI edge.
async fn buffered_download_to_vec(
    http_client: &Arc<dyn HttpClient>,
    runtime: &Arc<dyn Runtime>,
    request: &wacore::download::DownloadRequest,
    hashes: ExpectedMediaHashes,
) -> std::result::Result<Vec<u8>, DownloadRequestError> {
    let mut body = buffered_download_body(http_client, request).await?;
    let decryption = request.decryption.clone();
    wacore::runtime::blocking(&**runtime, move || {
        decrypt_or_validate_buffered_body(&mut body, &decryption, &hashes)?;
        Ok(body)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorChainExt;
    use crate::mediaconn::{MediaConn, MediaConnHost};
    use async_lock::Mutex;
    use std::io::{Cursor, Seek, SeekFrom, Write};
    use std::sync::Arc;
    use wacore::time::Instant;
    use waproto::whatsapp as wa;

    #[cfg(feature = "tracing")]
    mod diagnostics {
        use super::*;
        use std::error::Error;
        use tracing::instrument::WithSubscriber;

        const TOKEN: &str = "synthetic-auth-token-do-not-log";
        const CONTEXT: &str = "synthetic-backend-context-do-not-log";

        #[derive(Debug)]
        struct BackendFailure {
            url: String,
            status: HttpStatusError,
        }
        impl std::fmt::Display for BackendFailure {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "transport rejected signed URL {}", self.url)
            }
        }
        impl Error for BackendFailure {
            fn source(&self) -> Option<&(dyn Error + 'static)> {
                Some(&self.status)
            }
        }

        struct FailingHttp;
        impl FailingHttp {
            fn error(url: String) -> anyhow::Error {
                anyhow::Error::new(BackendFailure {
                    url: format!("{url}&auth={TOKEN}"),
                    status: HttpStatusError { status: 503 },
                })
                .context(CONTEXT)
            }
        }
        #[async_trait::async_trait]
        impl HttpClient for FailingHttp {
            async fn execute(
                &self,
                request: crate::http::HttpRequest,
            ) -> Result<crate::http::HttpResponse> {
                Err(Self::error(request.url))
            }
            fn execute_upload(
                &self,
                request: crate::http::HttpRequest,
                _body: Box<dyn std::io::Read + Send>,
                _len: u64,
            ) -> Result<crate::http::HttpResponse> {
                Err(Self::error(request.url))
            }
        }

        #[derive(Clone, Default)]
        struct Capture(Arc<std::sync::Mutex<Vec<u8>>>);
        impl Write for Capture {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        async fn capture_failure<E: Error + 'static>(future: impl Future<Output = E>, span: &str) {
            let capture = Capture::default();
            let writer = capture.clone();
            let subscriber = tracing_subscriber::fmt()
                .with_ansi(false)
                .without_time()
                .with_max_level(tracing::Level::TRACE)
                .with_writer(move || writer.clone())
                .finish();
            let error = future.with_subscriber(subscriber).await;
            assert_eq!(error.http_status(), Some(503));
            let backend = error
                .sources()
                .find_map(|e| e.downcast_ref::<BackendFailure>())
                .unwrap();
            assert!(backend.url.contains(TOKEN));
            let output = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
            assert!(output.contains(span), "missing span: {output}");
            assert!(output.contains("503"), "missing HTTP status: {output}");
            for secret in [TOKEN, CONTEXT, "https://", "auth="] {
                assert!(!output.contains(secret), "leaked {secret}: {output}");
            }
        }

        #[tokio::test]
        async fn public_media_entries_redact_tracing_and_preserve_sources() {
            let client = crate::test_utils::create_test_client_with_http(
                "media-diagnostics",
                Arc::new(FailingHttp),
            )
            .await;
            *client.media_conn.write().await = Some(media_conn(TOKEN, &["cdn.example.com"]));
            let route = MediaDownloader::new(
                Arc::new(FailingHttp),
                Arc::new(crate::TokioRuntime),
                MediaRoute::from(&media_conn(TOKEN, &["cdn.example.com"])),
            );
            let (mut params, _) = encrypted_params(b"diagnostic media");
            params.direct_path = format!("/media?signature={TOKEN}");
            capture_failure(
                async { route.download(&params).await.unwrap_err() },
                "wa.media.download_via_route",
            )
            .await;
            capture_failure(
                async {
                    route
                        .download_to_writer(&params, Cursor::new(Vec::new()))
                        .await
                        .unwrap_err()
                },
                "wa.media.download_via_route_to_writer",
            )
            .await;
            capture_failure(
                async { client.download(&params).await.unwrap_err() },
                "wa.media.download",
            )
            .await;
            capture_failure(
                async {
                    client
                        .download_to_writer(&params, Cursor::new(Vec::new()))
                        .await
                        .unwrap_err()
                },
                "wa.media.download_to_writer",
            )
            .await;
            // anyhow itself does not implement Error. A transparent typed wrapper
            // lets this same assertion traverse the public source chain.
            #[derive(Debug)]
            struct Returned(anyhow::Error);
            impl std::fmt::Display for Returned {
                fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Display::fmt(&self.0, f)
                }
            }
            impl Error for Returned {
                fn source(&self) -> Option<&(dyn Error + 'static)> {
                    Some(self.0.as_ref())
                }
            }
            capture_failure(
                async {
                    Returned(
                        client
                            .upload(
                                b"media".to_vec(),
                                MediaType::Image,
                                crate::upload::UploadOptions::default(),
                            )
                            .await
                            .unwrap_err(),
                    )
                },
                "wa.media.upload",
            )
            .await;
            let mut encrypted = Vec::new();
            let info = wacore::upload::encrypt_media_streaming(
                &mut Cursor::new(b"media"),
                &mut encrypted,
                MediaType::Image,
            )
            .unwrap();
            capture_failure(
                async {
                    Returned(
                        client
                            .upload_stream(bytes::Bytes::from(encrypted), info, MediaType::Image)
                            .await
                            .unwrap_err(),
                    )
                },
                "wa.media.upload_stream",
            )
            .await;
            capture_failure(
                async { Returned(client.fetch_sticker_pack("test", "en").await.unwrap_err()) },
                "wa.media.fetch_sticker_pack",
            )
            .await;
        }

        #[test]
        fn download_error_debug_redacts_nested_causes_and_cleanup() {
            let cause = || FailingHttp::error(format!("https://cdn.example.com/?auth={TOKEN}"));
            let cleanup = || std::io::Error::other(CONTEXT);
            let errors: Vec<Box<dyn Error>> = vec![
                Box::new(MediaDownloadError::ReferenceRejected(cause())),
                Box::new(MediaDownloadError::HostsUnreachable(cause())),
                Box::new(MediaDownloadError::WriterIo(cause())),
                Box::new(MediaDownloadError::Other(cause())),
                Box::new(MediaDownloadError::WriterCleanup {
                    failure: Box::new(MediaDownloadError::HostsUnreachable(cause())),
                    cleanup: cleanup(),
                }),
                Box::new(ClientDownloadError::ReferenceRejected(cause())),
                Box::new(ClientDownloadError::HostsUnreachable(cause())),
                Box::new(ClientDownloadError::WriterIo(cause())),
                Box::new(ClientDownloadError::NoHostsAfterRefresh(cause())),
                Box::new(ClientDownloadError::Preparation(cause())),
                Box::new(ClientDownloadError::MediaSession {
                    force_refresh: true,
                    source: crate::request::IqError::ParseError(cause()),
                }),
                Box::new(ClientDownloadError::WriterCleanup {
                    failure: Box::new(ClientDownloadError::HostsUnreachable(cause())),
                    cleanup: cleanup(),
                }),
            ];
            for error in errors {
                for rendered in [
                    format!("{error}"),
                    format!("{error:?}"),
                    format!("{error:#?}"),
                ] {
                    assert!(rendered.contains("503"), "{rendered}");
                    assert!(!rendered.contains(TOKEN), "{rendered}");
                    assert!(!rendered.contains(CONTEXT), "{rendered}");
                }
                assert!(ErrorChainExt::sources(error.as_ref()).any(|e| e.is::<BackendFailure>()));
            }
        }
    }

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

    /// Answers a single request with `status` and `body`, then closes. Stands in
    /// for one CDN host.
    #[cfg(feature = "ureq-client")]
    fn spawn_cdn_server(status: u16, reason: &'static str, body: Vec<u8>) -> String {
        use std::io::Read;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        std::thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut buf = Vec::new();
            let mut tmp = [0u8; 1024];
            while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut tmp) {
                    Ok(0) | Err(_) => return,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }
            let header = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
        });
        format!("http://{addr}")
    }

    #[cfg(feature = "ureq-client")]
    #[tokio::test]
    async fn media_server_receives_existing_query_and_separate_token() {
        use std::io::Read;
        use std::net::TcpListener;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut received = Vec::new();
            let mut buf = [0; 1024];
            while !received.windows(4).any(|w| w == b"\r\n\r\n") {
                let count = stream.read(&mut buf).unwrap();
                assert_ne!(count, 0);
                received.extend_from_slice(&buf[..count]);
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8(received).unwrap()
        });
        let media = PlaintextDownloadable {
            direct_path: "/mms/a%2Fb?x=one%26two&plus=%2B#fragment".into(),
            file_sha256: vec![3; 32],
        };
        let mut requests = DownloadUtils::prepare_download_requests(
            &media,
            &MediaRoute::new(vec![MediaHost::new(address.to_string())]),
        )
        .unwrap();
        let mut request = requests.remove(0);
        // The loopback fixture serves HTTP; keep the generated authority, path
        // and query unchanged. Production route construction still requires HTTPS.
        request.url = request.url.replacen("https://", "http://", 1);
        let client = ureq_client().await;
        assert_eq!(
            buffered_download_body(&client.http_client, &request)
                .await
                .unwrap(),
            b"ok"
        );
        let received = server.join().unwrap();
        let target = received
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap();
        let uri: http::Uri = target.parse().unwrap();
        assert_eq!(uri.path(), "/mms/a%2Fb");
        let query = uri.query().unwrap();
        let fields = query.split('&').collect::<Vec<_>>();
        assert_eq!(&fields[..2], &["x=one%26two", "plus=%2B"]);
        assert_eq!(fields.len(), 3);
        assert!(fields[2].starts_with("token="));
        assert!(!target.contains('#'));
    }

    #[cfg(feature = "ureq-client")]
    fn spawn_cdn_status_server(status: u16, reason: &'static str) -> String {
        spawn_cdn_server(status, reason, b"denied".to_vec())
    }

    #[cfg(feature = "ureq-client")]
    fn plaintext_request(url: String) -> wacore::download::DownloadRequest {
        wacore::download::DownloadRequest {
            url,
            decryption: MediaDecryption::Plaintext {
                file_sha256: vec![0u8; 32],
            },
        }
    }

    #[cfg(feature = "ureq-client")]
    async fn ureq_client() -> Arc<Client> {
        crate::test_utils::create_test_client_with_http(
            "cdn-status",
            Arc::new(whatsapp_rust_ureq_http_client::UreqHttpClient::new()),
        )
        .await
    }

    // Regression (#1185): `validate_download_status` was unit-tested while being
    // unreachable — the HTTP client turned every non-2xx into a transport error,
    // so a stale-auth 403 classified as `Other` and the whole host list was
    // retried with the same dead token instead of refreshing the media conn.
    // These two drive the real HTTP client against a real socket, so nothing
    // between the CDN status and the classifier is stubbed out.
    #[cfg(feature = "ureq-client")]
    #[tokio::test]
    async fn cdn_auth_status_reaches_the_classifier_on_the_streaming_path() {
        for status in [401u16, 403] {
            let url = spawn_cdn_status_server(status, "Forbidden");
            let client = ureq_client().await;
            let (_writer, result) = streaming_download_and_decrypt(
                &client.http_client,
                &client.runtime,
                &plaintext_request(url),
                ExpectedMediaHashes::default(),
                Cursor::new(Vec::new()),
            )
            .await
            .expect("the request itself completes; the status is the failure");
            let err = result.expect_err("a non-2xx CDN response must fail the download");
            assert!(
                err.is_auth(),
                "{status} must classify as an auth error so the media conn is refreshed, got {err:?}"
            );
        }
    }

    #[cfg(feature = "ureq-client")]
    #[tokio::test]
    async fn cdn_expired_status_reaches_the_classifier_on_the_buffered_path() {
        for status in [404u16, 410] {
            let url = spawn_cdn_status_server(status, "Gone");
            let client = ureq_client().await;
            let err = buffered_download_body(&client.http_client, &plaintext_request(url))
                .await
                .expect_err("a non-2xx CDN response must fail the download");
            assert!(
                err.is_not_found(),
                "{status} must classify as expired so the URL is re-derived, got {err:?}"
            );
        }
    }

    /// The chain the two tests above only prove one link of: a real CDN 403 must
    /// reach `invalidate_media_conn()` and let the forced-refresh attempt
    /// succeed. Before the fix the 403 arrived as an opaque transport error, so
    /// this loop rotated hosts on the same dead auth token and never refreshed.
    #[cfg(feature = "ureq-client")]
    #[tokio::test]
    async fn stale_auth_403_invalidates_the_media_conn_and_the_retry_recovers() {
        let body = b"download me".to_vec();
        let file_sha256 = plaintext_sha256(&body);
        let stale_host = spawn_cdn_status_server(403, "Forbidden");
        let fresh_host = spawn_cdn_server(200, "OK", body.clone());
        let client = ureq_client().await;
        let invalidations = Arc::new(Mutex::new(0usize));
        let attempts = Arc::new(Mutex::new(Vec::new()));

        let downloaded = download_media_with_retry(
            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
            {
                let attempts = Arc::clone(&attempts);
                move |force| {
                    let attempts = Arc::clone(&attempts);
                    let url = if force {
                        fresh_host.clone()
                    } else {
                        stale_host.clone()
                    };
                    let file_sha256 = file_sha256.clone();
                    async move {
                        attempts.lock().await.push(force);
                        Ok(vec![wacore::download::DownloadRequest {
                            url,
                            decryption: MediaDecryption::Plaintext { file_sha256 },
                        }])
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
            // Mirrors `Client::download`'s executor: streaming into a fresh buffer.
            |request| {
                let client = Arc::clone(&client);
                async move {
                    match streaming_download_and_decrypt(
                        &client.http_client,
                        &client.runtime,
                        &request,
                        ExpectedMediaHashes::default(),
                        Cursor::new(Vec::new()),
                    )
                    .await
                    {
                        Ok((writer, Ok(()))) => Ok(writer.into_inner()),
                        Ok((_, Err(e))) => Err(e),
                        Err(e) => Err(DownloadRequestError::other(e)),
                    }
                }
            },
        )
        .await
        .expect("the forced-refresh retry must recover the download");

        assert_eq!(downloaded, body);
        assert_eq!(
            *invalidations.lock().await,
            1,
            "a 403 must invalidate the cached media conn"
        );
        assert_eq!(
            *attempts.lock().await,
            vec![false, true],
            "the second attempt must ask for a refreshed media conn"
        );
    }

    /// Regression (#1193): the retry loop classified the CDN status correctly
    /// and then dropped the classification on the way out — `into_anyhow`
    /// unwrapped every variant to the same bare message, so a consumer of the
    /// public path could only recover the status by parsing `Display`.
    ///
    /// Driven through the real loop against a real socket rather than by
    /// building the error here: #1185 was a classifier that was unit-tested
    /// while unreachable, and a test that constructs its own error would repeat
    /// exactly that mistake.
    #[cfg(feature = "ureq-client")]
    #[tokio::test]
    async fn the_cdn_status_survives_to_the_public_error() {
        // One per branch of `validate_download_status`, since each built its
        // error by a different route: auth, not-found, and the `Other` arm that
        // the loop does not act on but a caller still has to tell apart.
        for (status, reason) in [
            (403u16, "Forbidden"),
            (410, "Gone"),
            (429, "Too Many Requests"),
        ] {
            // Each server answers once. 403 and 410 make the loop refresh the
            // media conn and try again, so the retry needs a host of its own —
            // still refusing, which is the case where the caller finally sees
            // the error.
            let first = spawn_cdn_status_server(status, reason);
            let refreshed = spawn_cdn_status_server(status, reason);
            let client = ureq_client().await;

            // Use the same final conversion as Client::download, retaining
            // both its domain classification and the original HTTP status.
            let err = download_media_with_retry(
                MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
                move |force| {
                    let url = if force {
                        refreshed.clone()
                    } else {
                        first.clone()
                    };
                    async move { Ok(vec![plaintext_request(url)]) }
                },
                || async {},
                |request| {
                    let client = Arc::clone(&client);
                    async move {
                        match streaming_download_and_decrypt(
                            &client.http_client,
                            &client.runtime,
                            &request,
                            ExpectedMediaHashes::default(),
                            Cursor::new(Vec::new()),
                        )
                        .await
                        {
                            Ok((writer, Ok(()))) => Ok(writer.into_inner()),
                            Ok((_, Err(e))) => Err(e),
                            Err(e) => Err(DownloadRequestError::other(e)),
                        }
                    }
                },
            )
            .await
            .map_err(ClientDownloadError::from)
            .expect_err("a non-2xx CDN response must fail the download");

            let cause: &(dyn std::error::Error + 'static) = &err;
            assert_eq!(
                cause.http_status(),
                Some(status),
                "the consumer must recover {status} by type, got: {err:?}"
            );
            // The status stayed in the message too, so nothing that logs the
            // error today reads differently.
            assert!(
                format!("{err}").contains(&status.to_string()),
                "the message should still name the status, got: {err}"
            );
        }
    }

    /// A failure with no HTTP status must not acquire one. That is the
    /// difference between "the CDN says this is gone" and "we broke", and a
    /// caller passing a status upstream needs it to be the CDN's.
    #[cfg(feature = "ureq-client")]
    #[tokio::test]
    async fn a_failure_with_no_exchange_reports_no_status() {
        let client = ureq_client().await;
        // Nothing is listening, so the exchange never completes.
        let request = plaintext_request("http://127.0.0.1:1".to_string());

        let err = download_media_with_retry(
            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
            move |_force| {
                let request = request.clone();
                async move { Ok(vec![request]) }
            },
            || async {},
            |request| {
                let client = Arc::clone(&client);
                async move {
                    match streaming_download_and_decrypt(
                        &client.http_client,
                        &client.runtime,
                        &request,
                        ExpectedMediaHashes::default(),
                        Cursor::new(Vec::new()),
                    )
                    .await
                    {
                        Ok((writer, Ok(()))) => Ok(writer.into_inner()),
                        Ok((_, Err(e))) => Err(e),
                        Err(e) => Err(DownloadRequestError::other(e)),
                    }
                }
            },
        )
        .await
        .map_err(ClientDownloadError::from)
        .expect_err("a refused connection must fail the download");

        let cause: &(dyn std::error::Error + 'static) = &err;
        assert_eq!(
            cause.http_status(),
            None,
            "a transport failure must not be laundered into an upstream status, got: {err:?}"
        );
    }

    #[test]
    fn buffered_body_is_decrypted_in_its_own_allocation() {
        // Every buffered download funnels through this helper, so a copy here
        // would mean holding two full copies of a multi-megabyte file at once.
        let plaintext = vec![0x5Eu8; 512 * 1024];
        let enc = wacore::upload::encrypt_media(&plaintext, MediaType::Video)
            .expect("encryption should succeed");
        let mut body = enc.data_to_upload;
        let allocation = body.as_ptr();
        let capacity = body.capacity();

        decrypt_or_validate_buffered_body(
            &mut body,
            &MediaDecryption::Encrypted {
                media_key: enc.media_key.to_vec(),
                media_type: MediaType::Video,
            },
            &ExpectedMediaHashes {
                encrypted: Some(enc.file_enc_sha256.to_vec()),
                plaintext: Some(enc.file_sha256.to_vec()),
            },
        )
        .expect("decryption should succeed");

        assert_eq!(body, plaintext);
        assert_eq!(body.as_ptr(), allocation, "allocation must be reused");
        assert_eq!(body.capacity(), capacity, "capacity must be reused");
    }

    #[test]
    fn download_statuses_have_one_shared_classification() {
        use crate::http::{HTTP_STATUS_FORBIDDEN, HTTP_STATUS_UNAUTHORIZED};

        assert!(validate_download_status(HTTP_STATUS_OK).is_ok());
        assert!(matches!(
            validate_download_status(HTTP_STATUS_UNAUTHORIZED),
            Err(DownloadRequestError::Auth(_))
        ));
        assert!(matches!(
            validate_download_status(HTTP_STATUS_FORBIDDEN),
            Err(DownloadRequestError::Auth(_))
        ));
        assert!(matches!(
            validate_download_status(HTTP_STATUS_NOT_FOUND),
            Err(DownloadRequestError::NotFound(_))
        ));
        assert!(matches!(
            validate_download_status(HTTP_STATUS_GONE),
            Err(DownloadRequestError::NotFound(_))
        ));
        assert!(matches!(
            validate_download_status(HTTP_STATUS_REDIRECTION_START),
            Err(DownloadRequestError::Other(_))
        ));
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
            matches!(&err, MediaDecryptionError::InvalidMac),
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
            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
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
                            &MediaRoute::from(&media_conn),
                        )
                        .map_err(DownloadRequestError::Prepare)
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
                        if url.contains("cdn1.example.com") {
                            Err(DownloadRequestError::auth(401))
                        } else {
                            Ok(body)
                        }
                    }
                }
            },
        )
        .await
        .expect("download should succeed after refreshing media hosts");

        assert_eq!(downloaded, body);
        assert_eq!(*refresh_calls.lock().await, vec![false, true]);
        assert_eq!(*invalidations.lock().await, 1);

        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("cdn1.example.com"));
        assert!(seen_urls.iter().all(|url| !url.contains("auth=")));
        assert!(seen_urls[1].contains("cdn2.example.com"));
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
            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
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
                            &MediaRoute::from(&conn),
                        )
                        .map_err(DownloadRequestError::Prepare)
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
            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
            {
                let downloadable = &downloadable;
                let conn = conn.clone();
                move |_force| {
                    let conn = conn.clone();
                    async move {
                        DownloadUtils::prepare_download_requests(
                            downloadable,
                            &MediaRoute::from(&conn),
                        )
                        .map_err(DownloadRequestError::Prepare)
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
        .expect_err("all hosts failing must surface an error")
        .into_anyhow();

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

        let runtime: Arc<dyn Runtime> = Arc::new(crate::TokioRuntime);
        let writer = download_to_writer_with_retry(
            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
            &runtime,
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
                            &MediaRoute::from(&media_conn),
                        )
                        .map_err(DownloadRequestError::Prepare)
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
                        if url.contains("cdn1.example.com") {
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
        .expect("streaming download should succeed after refreshing media hosts");

        assert_eq!(writer.into_inner(), body);
        assert_eq!(*refresh_calls.lock().await, vec![false, true]);
        assert_eq!(*invalidations.lock().await, 1);

        let seen_urls = seen_urls.lock().await.clone();
        assert_eq!(seen_urls.len(), 2);
        assert!(seen_urls[0].contains("cdn1.example.com"));
        assert!(seen_urls.iter().all(|url| !url.contains("auth=")));
        assert!(seen_urls[1].contains("cdn2.example.com"));
    }

    // ── Session-less downloads ──────────────────────────────────────────────

    /// Answers by first substring match on the requested URL, recording every
    /// URL it was asked for so host order and retry count are observable.
    struct RoutedHttpClient {
        routes: Vec<(&'static str, u16, Vec<u8>)>,
        fallback: (u16, Vec<u8>),
        streaming: bool,
        // std, not async: `execute_streaming` is a blocking call.
        seen_urls: std::sync::Mutex<Vec<String>>,
    }

    impl RoutedHttpClient {
        fn new(routes: Vec<(&'static str, u16, Vec<u8>)>, fallback: (u16, Vec<u8>)) -> Arc<Self> {
            Arc::new(Self {
                routes,
                fallback,
                streaming: false,
                seen_urls: std::sync::Mutex::new(Vec::new()),
            })
        }

        fn streaming(
            routes: Vec<(&'static str, u16, Vec<u8>)>,
            fallback: (u16, Vec<u8>),
        ) -> Arc<Self> {
            Arc::new(Self {
                routes,
                fallback,
                streaming: true,
                seen_urls: std::sync::Mutex::new(Vec::new()),
            })
        }
    }

    impl RoutedHttpClient {
        fn record(&self, url: &str) {
            self.seen_urls
                .lock()
                .expect("test mutex is never poisoned")
                .push(url.to_string());
        }

        fn urls(&self) -> Vec<String> {
            self.seen_urls
                .lock()
                .expect("test mutex is never poisoned")
                .clone()
        }

        fn respond(&self, url: &str) -> (u16, Vec<u8>) {
            self.routes
                .iter()
                .find(|(needle, _, _)| url.contains(needle))
                .map(|(_, status, body)| (*status, body.clone()))
                .unwrap_or_else(|| self.fallback.clone())
        }
    }

    #[async_trait::async_trait]
    impl HttpClient for RoutedHttpClient {
        async fn execute(
            &self,
            request: crate::http::HttpRequest,
        ) -> Result<crate::http::HttpResponse> {
            self.record(&request.url);
            let (status_code, body) = self.respond(&request.url);
            Ok(crate::http::HttpResponse { status_code, body })
        }

        // Implemented so `download_to_writer` exercises the streaming branch
        // rather than silently falling back to the buffered one.
        fn supports_streaming(&self) -> bool {
            self.streaming
        }

        fn execute_streaming(
            &self,
            request: crate::http::HttpRequest,
        ) -> Result<wacore::net::StreamingHttpResponse> {
            self.record(&request.url);
            let (status_code, body) = self.respond(&request.url);
            Ok(wacore::net::StreamingHttpResponse {
                status_code,
                body: Box::new(Cursor::new(body)),
            })
        }
    }

    fn downloader(http: Arc<RoutedHttpClient>, hosts: &[&str]) -> MediaDownloader {
        MediaDownloader::new(
            http,
            Arc::new(crate::TokioRuntime),
            MediaRoute::new(hosts.iter().copied().map(MediaHost::new).collect()),
        )
    }

    fn encrypted_params(data: &[u8]) -> (DownloadParams, Vec<u8>) {
        let enc = wacore::upload::encrypt_media(data, MediaType::Image)
            .expect("encryption should succeed");
        let params = DownloadParams::encrypted(
            "/v/t62.7118-24/no-session",
            &enc.media_key,
            &enc.file_sha256,
            &enc.file_enc_sha256,
            data.len() as u64,
            MediaType::Image,
        );
        (params, enc.data_to_upload)
    }

    async fn reject_declared_hash_mismatch(streaming: bool, to_writer: bool) {
        // More than one streaming buffer, plus a partial final AES block.
        let original = vec![0x36; 32 * 1024 + 13];
        for corrupt_encrypted_hash in [true, false] {
            for malformed_length in [false, true] {
                let (mut params, encrypted) = encrypted_params(&original);
                let hash = if corrupt_encrypted_hash {
                    params.file_enc_sha256.as_mut().unwrap()
                } else {
                    &mut params.file_sha256
                };
                if malformed_length {
                    hash.clear();
                } else {
                    hash[0] ^= 1;
                }
                let http = if streaming {
                    RoutedHttpClient::streaming(Vec::new(), (200, encrypted))
                } else {
                    RoutedHttpClient::new(Vec::new(), (200, encrypted))
                };
                let dl = downloader(http, &["cdn.example.com"]);
                let error = if to_writer {
                    let sink = SharedWriter::new();
                    sink.with(|w| w.write_all(b"old destination contents").unwrap());
                    let result = dl.download_to_writer(&params, sink.clone()).await;
                    assert!(
                        result.is_err(),
                        "valid HMAC must not excuse a declared hash mismatch: streaming={streaming}, encrypted_hash={corrupt_encrypted_hash}, malformed={malformed_length}"
                    );
                    assert!(
                        sink.contents().is_empty(),
                        "failed output must be discarded"
                    );
                    result.unwrap_err()
                } else {
                    dl.download(&params)
                        .await
                        .expect_err("valid HMAC must not excuse a declared hash mismatch")
                };
                assert!(matches!(error, MediaDownloadError::HostsUnreachable(_)));
            }
        }
    }

    #[tokio::test]
    async fn encrypted_hashes_are_checked_for_buffered_memory_downloads() {
        reject_declared_hash_mismatch(false, false).await;
    }

    #[tokio::test]
    async fn encrypted_hashes_are_checked_for_streaming_memory_downloads() {
        reject_declared_hash_mismatch(true, false).await;
    }

    #[tokio::test]
    async fn encrypted_hashes_are_checked_for_buffered_writer_downloads() {
        reject_declared_hash_mismatch(false, true).await;
    }

    #[tokio::test]
    async fn encrypted_hashes_are_checked_for_streaming_writer_downloads() {
        reject_declared_hash_mismatch(true, true).await;
    }

    #[tokio::test]
    async fn client_downloads_check_declared_hashes_too() {
        for streaming in [false, true] {
            for to_writer in [false, true] {
                for corrupt_encrypted_hash in [false, true] {
                    let enc = wacore::upload::encrypt_media(
                        &vec![0x58; 32 * 1024 + 13],
                        MediaType::Image,
                    )
                    .unwrap();
                    let mut message = waproto::whatsapp::message::ImageMessage {
                        media_key: Some(enc.media_key.to_vec()),
                        file_enc_sha256: Some(enc.file_enc_sha256.to_vec()),
                        file_sha256: Some(enc.file_sha256.to_vec()),
                        static_url: Some("https://cdn.example.com/p1-fixture".to_owned()),
                        ..Default::default()
                    };
                    if corrupt_encrypted_hash {
                        message.file_enc_sha256.as_mut().unwrap()[0] ^= 1;
                    } else {
                        message.file_sha256.as_mut().unwrap()[0] ^= 1;
                    }
                    let http = if streaming {
                        RoutedHttpClient::streaming(Vec::new(), (200, enc.data_to_upload))
                    } else {
                        RoutedHttpClient::new(Vec::new(), (200, enc.data_to_upload))
                    };
                    let client =
                        crate::test_utils::create_test_client_with_http("p1-hashes", http).await;
                    let error = if to_writer {
                        let sink = SharedWriter::new();
                        let error = client
                            .download_to_writer(&message, sink.clone())
                            .await
                            .unwrap_err();
                        assert!(sink.contents().is_empty());
                        error
                    } else {
                        let result = client.download(&message).await;
                        assert!(result.is_err());
                        result.unwrap_err()
                    };
                    let ClientDownloadError::HostsUnreachable(cause) = error else {
                        panic!("expected final host failure, got {error:?}");
                    };
                    assert!(matches!(
                        (
                            corrupt_encrypted_hash,
                            cause.downcast_ref::<MediaDecryptionError>()
                        ),
                        (true, Some(MediaDecryptionError::EncryptedSha256Mismatch))
                            | (false, Some(MediaDecryptionError::PlaintextSha256Mismatch))
                    ));
                }
            }
        }
    }

    #[tokio::test]
    async fn encrypted_hash_mismatch_can_recover_on_another_host() {
        let original = b"the requested file";
        for streaming in [false, true] {
            for to_writer in [false, true] {
                let (params, good) = encrypted_params(original);
                let media_key: [u8; 32] = params.media_key.as_deref().unwrap().try_into().unwrap();
                // A different object under the same key still has a valid MAC.
                // Its presence on one host does not prove every host is wrong.
                let wrong = wacore::upload::encrypt_media_with_key(
                    &vec![0x47; 32 * 1024 + 13],
                    MediaType::Image,
                    Some(&media_key),
                )
                .unwrap()
                .data_to_upload;
                let routes = vec![("bad-host", 200, wrong), ("good-host", 200, good)];
                let http = if streaming {
                    RoutedHttpClient::streaming(routes, (500, Vec::new()))
                } else {
                    RoutedHttpClient::new(routes, (500, Vec::new()))
                };
                let dl = downloader(
                    http.clone(),
                    &["bad-host.example.com", "good-host.example.com"],
                );
                let actual = if to_writer {
                    dl.download_to_writer(&params, Cursor::new(Vec::new()))
                        .await
                        .unwrap()
                        .into_inner()
                } else {
                    dl.download(&params).await.unwrap()
                };
                assert_eq!(
                    actual, original,
                    "only the body matching the declared hashes may succeed"
                );
                assert_eq!(http.urls().len(), 2);
            }
        }
    }

    /// A body encrypted under `media_key` that decrypts to `plaintext` and only
    /// then fails its MAC — the shape a CDN error page takes on the wire once the
    /// reference's key is applied to it.
    fn forged_body(plaintext: &[u8], media_key: &[u8; 32]) -> Vec<u8> {
        let mut body =
            wacore::upload::encrypt_media_with_key(plaintext, MediaType::Image, Some(media_key))
                .expect("encryption should succeed")
                .data_to_upload;
        let last = body.len() - 1;
        body[last] ^= 1;
        body
    }

    /// Media is authenticated by one MAC over the whole ciphertext, so a failing
    /// host streams plaintext into the writer and only then turns out to be
    /// forged. If the retry that replaces it writes fewer bytes, whatever the
    /// first host left past that point must not survive into a successful
    /// download. Regression test for #1196.
    #[tokio::test]
    async fn a_failed_host_leaves_no_tail_behind_a_shorter_successful_retry() {
        let media_key = [0x5b; 32];
        let original = b"short but verified".to_vec();
        let good =
            wacore::upload::encrypt_media_with_key(&original, MediaType::Image, Some(&media_key))
                .expect("encryption should succeed");
        let params = DownloadParams::encrypted(
            "/v/t62.7118-24/tail",
            &good.media_key,
            &good.file_sha256,
            &good.file_enc_sha256,
            original.len() as u64,
            MediaType::Image,
        );

        // Far longer than the real media, and longer than one 8KB decrypt chunk,
        // so the plaintext is already in the writer when the MAC check fails.
        let forged = forged_body(&vec![0xAA; 64 * 1024], &media_key);
        assert!(forged.len() > good.data_to_upload.len() * 10);

        for streaming in [true, false] {
            let routes = vec![
                ("forging-host", 200, forged.clone()),
                ("honest-host", 200, good.data_to_upload.clone()),
            ];
            let http = if streaming {
                RoutedHttpClient::streaming(routes, (500, Vec::new()))
            } else {
                RoutedHttpClient::new(routes, (500, Vec::new()))
            };

            let writer = downloader(
                http.clone(),
                &["forging-host.example.com", "honest-host.example.com"],
            )
            .download_to_writer(&params, Cursor::new(Vec::new()))
            .await
            .expect("the honest host must still satisfy the download");

            assert_eq!(
                http.urls().len(),
                2,
                "the forged host must have been tried first (streaming={streaming})"
            );
            assert_eq!(
                writer.into_inner(),
                original,
                "the forged host's plaintext must not survive past the media (streaming={streaming})"
            );
        }
    }

    /// The same guarantee from the other direction: content the caller left in
    /// the writer is not part of the media either.
    #[tokio::test]
    async fn a_writer_that_arrives_with_content_still_ends_up_holding_only_the_media() {
        let original = b"exactly this".to_vec();
        let (params, encrypted) = encrypted_params(&original);
        let http = RoutedHttpClient::streaming(Vec::new(), (200, encrypted));

        let writer = downloader(http, &["cdn.example.com"])
            .download_to_writer(&params, Cursor::new(vec![0xFF; 4096]))
            .await
            .expect("download should succeed");

        assert_eq!(writer.into_inner(), original);
    }

    /// A file opened for appending writes at the end no matter where it was
    /// seeked, so rewinding alone would leave the media behind whatever the file
    /// already held. Emptying the sink is what puts an append-mode write at the
    /// start, and a real `File` is the only way to prove it — `Cursor` honours
    /// seeks and cannot express the mode.
    #[tokio::test]
    async fn an_append_mode_file_still_ends_up_holding_only_the_media() {
        let original = b"appended, yet exact".to_vec();
        let (params, encrypted) = encrypted_params(&original);

        let path = std::env::temp_dir().join(format!(
            "wa-rust-append-{}-{:?}.bin",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::write(&path, b"stale bytes the caller left behind")
            .expect("fixture write should succeed");
        let file = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .expect("append open should succeed");

        let http = RoutedHttpClient::streaming(Vec::new(), (200, encrypted));
        downloader(http, &["cdn.example.com"])
            .download_to_writer(&params, file)
            .await
            .expect("download should succeed");

        let written = std::fs::read(&path).expect("read back should succeed");
        let _ = std::fs::remove_file(&path);
        assert_eq!(written, original);
    }

    /// A [`DownloadWriter`] whose bytes outlive it.
    ///
    /// `download_to_writer` takes its writer by value and only hands it back on
    /// success, so a failed download's writer is otherwise unobservable. Being a
    /// third-party implementation, this also pins that the trait can be
    /// implemented from outside the crate.
    #[derive(Clone, Debug)]
    struct SharedWriter(Arc<std::sync::Mutex<Cursor<Vec<u8>>>>);

    impl SharedWriter {
        fn new() -> Self {
            Self(Arc::new(std::sync::Mutex::new(Cursor::new(Vec::new()))))
        }

        fn contents(&self) -> Vec<u8> {
            self.with(|inner| inner.get_ref().clone())
        }

        fn with<T>(&self, f: impl FnOnce(&mut Cursor<Vec<u8>>) -> T) -> T {
            f(&mut self.0.lock().expect("test mutex is never poisoned"))
        }
    }

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.with(|inner| inner.write(buf))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            self.with(|inner| inner.flush())
        }
    }

    impl Seek for SharedWriter {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.with(|inner| inner.seek(pos))
        }
    }

    impl DownloadWriter for SharedWriter {
        fn truncate(&mut self, len: u64) -> std::io::Result<()> {
            self.with(|inner| inner.truncate(len))
        }
    }

    /// A download that never succeeds must not leave a half-written body behind
    /// for a caller to mistake for media.
    #[tokio::test]
    async fn a_download_that_fails_everywhere_empties_the_writer() {
        let media_key = [0x77; 32];
        let reference = encrypted_params(b"unused").0;
        let params = DownloadParams::encrypted(
            "/v/t62.7118-24/doomed",
            &media_key,
            &reference.file_sha256,
            reference.file_enc_sha256.as_deref().unwrap_or_default(),
            12,
            MediaType::Image,
        );
        let forged = forged_body(&vec![0x11; 32 * 1024], &media_key);
        let http = RoutedHttpClient::streaming(vec![("only-host", 200, forged)], (500, Vec::new()));

        let sink = SharedWriter::new();
        let err = downloader(http, &["only-host.example.com"])
            .download_to_writer(&params, sink.clone())
            .await
            .expect_err("a forged body must not be reported as a download");

        assert!(matches!(err, MediaDownloadError::HostsUnreachable(_)));
        assert!(
            sink.contents().is_empty(),
            "a failed download must not leave plaintext behind"
        );
    }

    #[tokio::test]
    async fn media_downloader_fetches_without_a_session_or_auth() {
        let original = b"media that outlived its session".to_vec();
        let (params, encrypted) = encrypted_params(&original);
        let http = RoutedHttpClient::new(
            vec![("good-host", 200, encrypted)],
            (500, b"server error".to_vec()),
        );

        let downloaded = downloader(
            http.clone(),
            &["bad-host.example.com", "good-host.example.com"],
        )
        .download(&params)
        .await
        .expect("an injected host list is all a download needs");

        assert_eq!(downloaded, original);
        let seen = http.urls();
        assert_eq!(seen.len(), 2, "the failing host must fail over to the next");
        assert!(
            seen[0].starts_with("https://bad-host.example.com/v/t62.7118-24/no-session?token=")
        );
        assert!(
            seen[1].starts_with("https://good-host.example.com/v/t62.7118-24/no-session?token=")
        );
        assert!(seen.iter().all(|url| !url.contains("auth=")));
    }

    #[tokio::test]
    async fn media_downloader_streams_to_a_writer_without_a_session() {
        let original = b"streamed without a session".to_vec();
        let (params, encrypted) = encrypted_params(&original);
        let http = RoutedHttpClient::streaming(Vec::new(), (200, encrypted.clone()));
        assert!(
            http.supports_streaming(),
            "this must not fall back to buffered"
        );

        let writer = downloader(http, &["cdn.example.com"])
            .download_to_writer(&params, Cursor::new(Vec::new()))
            .await
            .expect("the streaming path must work without a session too");
        assert_eq!(writer.into_inner(), original);

        // Same request over a client without streaming, to keep the buffered
        // fallback covered as well.
        let buffered = RoutedHttpClient::new(Vec::new(), (200, encrypted));
        let writer = downloader(buffered, &["cdn.example.com"])
            .download_to_writer(&params, Cursor::new(Vec::new()))
            .await
            .expect("the buffered fallback must work too");
        assert_eq!(writer.into_inner(), original);
    }

    // A reference too incomplete to build a URL from never contacts a host, so
    // it must not be reported as though every host had failed.
    #[tokio::test]
    async fn preparation_diagnostics_preserve_safe_reasons_without_reference_text() {
        use wacore::download::DownloadPreparationError;
        for (path, host, expected) in [
            (
                "/file?auth=synthetic-secret",
                "cdn.example.com",
                DownloadPreparationError::MissingEncryptedHash,
            ),
            (
                "/file",
                "cdn.example.com/synthetic-secret",
                DownloadPreparationError::InvalidRouteHost,
            ),
            (
                "https://other.example/file?auth=synthetic-secret",
                "cdn.example.com",
                DownloadPreparationError::OriginChanged,
            ),
        ] {
            let (mut params, _) = encrypted_params(b"not fetched");
            params.direct_path = path.into();
            if expected == DownloadPreparationError::MissingEncryptedHash {
                params.file_enc_sha256 = None;
            }
            let http = RoutedHttpClient::new(Vec::new(), (200, Vec::new()));
            let error = downloader(http.clone(), &[host])
                .download(&params)
                .await
                .unwrap_err();
            assert_eq!(
                ErrorChainExt::sources(&error)
                    .find_map(|e| e.downcast_ref::<DownloadPreparationError>()),
                Some(&expected)
            );
            for rendered in [format!("{error}"), format!("{error:?}")] {
                assert!(rendered.contains(&format!("{expected:?}")), "{rendered}");
                assert!(!rendered.contains("synthetic-secret"), "{rendered}");
            }
            assert!(http.urls().is_empty());
        }
    }

    #[tokio::test]
    async fn media_downloader_separates_an_unbuildable_reference_from_a_dead_host() {
        let params = DownloadParams {
            direct_path: "/v/t62.7118-24/incomplete".to_string(),
            media_key: Some(vec![1u8; 32]),
            file_sha256: vec![2u8; 32],
            file_enc_sha256: None,
            file_length: 16,
            media_type: MediaType::Image,
        };
        let http = RoutedHttpClient::new(Vec::new(), (200, Vec::new()));

        let err = downloader(http.clone(), &["cdn1.example.com", "cdn2.example.com"])
            .download(&params)
            .await
            .expect_err("an incomplete reference cannot succeed");

        assert!(
            matches!(err, MediaDownloadError::Other(_)),
            "a reference that cannot build a URL is not a host failure, got {err:?}"
        );
        assert!(
            ErrorChainExt::sources(&err)
                .skip(1)
                .any(|cause| cause.to_string().contains("Missing file_enc_sha256")),
            "the cause must survive the classification, got: {err}"
        );
        assert!(http.urls().is_empty());
    }

    #[tokio::test]
    async fn media_downloader_separates_an_expired_reference_from_a_dead_host() {
        let (params, _) = encrypted_params(b"gone");

        let expired = RoutedHttpClient::new(Vec::new(), (410, b"gone".to_vec()));
        let err = downloader(expired.clone(), &["cdn1.example.com", "cdn2.example.com"])
            .download(&params)
            .await
            .expect_err("an expired reference must not read as success");
        assert!(
            matches!(err, MediaDownloadError::ReferenceRejected(_)),
            "expected a rejected reference, got {err:?}"
        );
        assert_eq!(
            expired.urls().len(),
            1,
            "no host rotation and no refresh: nothing here can re-sign the reference"
        );

        let dead = RoutedHttpClient::new(Vec::new(), (500, b"boom".to_vec()));
        let err = downloader(dead.clone(), &["cdn1.example.com", "cdn2.example.com"])
            .download(&params)
            .await
            .expect_err("every host failing must not read as success");
        assert!(
            matches!(err, MediaDownloadError::HostsUnreachable(_)),
            "expected unreachable hosts, got {err:?}"
        );
        assert_eq!(dead.urls().len(), 2, "every host is tried");
    }

    #[tokio::test]
    async fn media_downloader_defaults_to_the_known_cdn_hosts() {
        let downloader = MediaDownloader::with_default_hosts(
            RoutedHttpClient::new(Vec::new(), (200, Vec::new())),
            Arc::new(crate::TokioRuntime),
        );
        assert_eq!(
            downloader
                .route()
                .hosts
                .iter()
                .map(|h| h.hostname.as_str())
                .collect::<Vec<_>>(),
            DEFAULT_MEDIA_HOSTS.to_vec(),
        );
    }

    #[tokio::test]
    async fn media_downloader_without_hosts_reports_it() {
        let (params, _) = encrypted_params(b"nowhere to go");
        let http = RoutedHttpClient::new(Vec::new(), (200, Vec::new()));

        let err = downloader(http.clone(), &[])
            .download(&params)
            .await
            .expect_err("an empty route cannot succeed");

        assert!(
            matches!(err, MediaDownloadError::NoHosts),
            "expected NoHosts, got {err:?}"
        );
        assert!(http.urls().is_empty());
    }

    // Regression: a `static_url` download used to fetch a media conn over the
    // wire and then throw it away, which also made the download impossible
    // offline. A disconnected client makes the discarded IQ observable.
    #[tokio::test]
    async fn static_url_download_asks_for_no_media_conn() {
        let client = crate::test_utils::create_test_client_with_name("static_url_no_iq").await;

        let with_static_url = wa::message::ImageMessage {
            static_url: Some("https://static.cdn.example.com/media/abc123".to_string()),
            direct_path: Some("/v/t62.7118-24/unused".to_string()),
            file_sha256: Some(vec![7u8; 32]),
            ..Default::default()
        };
        let requests = client
            .prepare_requests(&with_static_url, false)
            .await
            .expect("a static URL needs no hosts, so it must not need a session");
        assert_eq!(requests.len(), 1);
        assert_eq!(
            requests[0].url,
            "https://static.cdn.example.com/media/abc123"
        );

        // The same client still asks the server for hosts when there is no
        // static URL, which is what makes the assertion above meaningful.
        let without_static_url = wa::message::ImageMessage {
            direct_path: Some("/v/t62.7118-24/needs-hosts".to_string()),
            file_sha256: Some(vec![7u8; 32]),
            ..Default::default()
        };
        let err = client
            .prepare_requests(&without_static_url, false)
            .await
            .expect_err("host construction still needs a media conn");
        assert!(matches!(
            err,
            DownloadRequestError::Session {
                force_refresh: false,
                source: crate::request::IqError::NotConnected,
            }
        ));
    }

    #[tokio::test]
    async fn client_params_and_messages_share_routes_and_outputs() {
        let original = b"one canonical route";
        let (params, encrypted) = encrypted_params(original);
        let message = wa::message::ImageMessage {
            direct_path: Some(params.direct_path.clone()),
            media_key: params.media_key.clone(),
            file_sha256: Some(params.file_sha256.clone()),
            file_enc_sha256: params.file_enc_sha256.clone(),
            file_length: Some(params.file_length),
            ..Default::default()
        };
        for streaming in [false, true] {
            let http = if streaming {
                RoutedHttpClient::streaming(Vec::new(), (200, encrypted.clone()))
            } else {
                RoutedHttpClient::new(Vec::new(), (200, encrypted.clone()))
            };
            let client =
                crate::test_utils::create_test_client_with_http("download-params", http.clone())
                    .await;
            *client.media_conn.write().await = Some(media_conn("auth", &["cdn.example.com"]));
            assert_eq!(client.download(&params).await.unwrap(), original);
            assert_eq!(client.download(&message).await.unwrap(), original);
            assert_eq!(
                client
                    .download_to_writer(&params, Cursor::new(vec![0xFF; 1024]))
                    .await
                    .unwrap()
                    .into_inner(),
                original
            );
            assert_eq!(
                client
                    .download_to_writer(&message, Cursor::new(Vec::new()))
                    .await
                    .unwrap()
                    .into_inner(),
                original
            );
            let urls = http.urls();
            assert_eq!(urls.len(), 4);
            assert!(urls.iter().all(|url| url == &urls[0]));
            assert!(!urls[0].contains("auth="));
        }
    }

    #[tokio::test]
    async fn client_static_rejection_is_final_without_invalidating_a_session() {
        let message = wa::message::ImageMessage {
            static_url: Some("https://cdn.example.com/static".into()),
            file_sha256: Some(vec![0; 32]),
            ..Default::default()
        };
        for streaming in [false, true] {
            let http = if streaming {
                RoutedHttpClient::streaming(Vec::new(), (403, Vec::new()))
            } else {
                RoutedHttpClient::new(Vec::new(), (403, Vec::new()))
            };
            let client =
                crate::test_utils::create_test_client_with_http("download-static", http.clone())
                    .await;
            *client.media_conn.write().await =
                Some(media_conn("untouched", &["unused.example.com"]));
            let errors = [
                client.download(&message).await.unwrap_err(),
                client
                    .download_to_writer(&message, Cursor::new(Vec::new()))
                    .await
                    .unwrap_err(),
            ];
            for error in errors {
                assert!(matches!(error, ClientDownloadError::ReferenceRejected(_)));
                let cause: &(dyn std::error::Error + 'static) = &error;
                assert_eq!(cause.http_status(), Some(403));
            }
            assert_eq!(http.urls(), vec![message.static_url.clone().unwrap(); 2]);
            assert_eq!(
                client.media_conn.read().await.as_ref().unwrap().auth,
                "untouched"
            );
        }
    }

    #[tokio::test]
    async fn client_distinguishes_preparation_route_and_session_failures() {
        let (mut params, _) = encrypted_params(b"unrequested");
        let http = RoutedHttpClient::new(Vec::new(), (403, Vec::new()));
        let client =
            crate::test_utils::create_test_client_with_http("download-errors", http.clone()).await;
        for to_writer in [false, true] {
            for case in 0..4 {
                params.file_enc_sha256 = Some(vec![1; 32]);
                *client.media_conn.write().await = match case {
                    0 => Some(media_conn("auth", &[])),
                    1 | 3 => Some(media_conn("auth", &["cdn.example.com"])),
                    _ => None,
                };
                if case == 1 {
                    params.file_enc_sha256 = None;
                }
                let sink = SharedWriter::new();
                sink.with(|w| w.write_all(b"old contents").unwrap());
                let error = if to_writer {
                    let result = client.download_to_writer(&params, sink.clone()).await;
                    assert!(sink.contents().is_empty());
                    result.unwrap_err()
                } else {
                    client.download(&params).await.unwrap_err()
                };
                match case {
                    0 => assert!(matches!(error, ClientDownloadError::NoHosts)),
                    1 => assert!(matches!(error, ClientDownloadError::Preparation(_))),
                    2 | 3 => {
                        let ClientDownloadError::MediaSession {
                            force_refresh,
                            source,
                        } = error
                        else {
                            panic!("expected session failure, got {error:?}");
                        };
                        assert_eq!(force_refresh, case == 3);
                        assert!(matches!(source, crate::request::IqError::NotConnected));
                    }
                    _ => unreachable!(),
                }
            }
        }
        // Only the cached rejected route reaches HTTP, once per API. Initial
        // acquisition, malformed metadata and empty routes contact no hosts.
        assert_eq!(http.urls().len(), 2);
    }

    #[tokio::test]
    async fn client_rejection_after_refresh_preserves_final_classification_and_status() {
        let (params, _) = encrypted_params(b"revoked");
        let http = RoutedHttpClient::new(Vec::new(), (410, Vec::new()));
        let runtime: Arc<dyn Runtime> = Arc::new(crate::TokioRuntime);
        let conn = media_conn("fresh-or-stale", &["cdn.example.com"]);
        for to_writer in [false, true] {
            let calls = std::sync::Mutex::new(Vec::new());
            let invalidations = std::sync::atomic::AtomicUsize::new(0);
            let prepare = |force| {
                calls.lock().unwrap().push(force);
                let requests =
                    DownloadUtils::prepare_download_requests(&params, &MediaRoute::from(&conn))
                        .map_err(DownloadRequestError::Prepare);
                async move { requests }
            };
            let invalidate = || async {
                invalidations.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            };
            let error = if to_writer {
                download_to_writer_with_retry(
                    MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
                    &runtime,
                    Cursor::new(Vec::new()),
                    prepare,
                    invalidate,
                    |request, writer| {
                        let http: Arc<dyn HttpClient> = http.clone();
                        let runtime = runtime.clone();
                        async move {
                            streaming_download_and_decrypt(
                                &http,
                                &runtime,
                                &request,
                                ExpectedMediaHashes::default(),
                                writer,
                            )
                            .await
                        }
                    },
                )
                .await
                .unwrap_err()
            } else {
                download_media_with_retry(
                    MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
                    prepare,
                    invalidate,
                    |request| {
                        let http: Arc<dyn HttpClient> = http.clone();
                        let runtime = runtime.clone();
                        async move {
                            execute_request_into_memory(
                                &http,
                                &runtime,
                                &request,
                                ExpectedMediaHashes::default(),
                                0,
                            )
                            .await
                        }
                    },
                )
                .await
                .unwrap_err()
            };
            let error = ClientDownloadError::from(error);
            assert!(matches!(error, ClientDownloadError::ReferenceRejected(_)));
            let cause: &(dyn std::error::Error + 'static) = &error;
            assert_eq!(cause.http_status(), Some(410));
            assert_eq!(*calls.lock().unwrap(), vec![false, true]);
            assert_eq!(invalidations.load(std::sync::atomic::Ordering::Relaxed), 1);
        }
    }

    #[tokio::test]
    async fn client_empty_refreshed_route_keeps_the_prior_rejection_and_clears_writer() {
        let (params, _) = encrypted_params(b"revoked");
        let runtime: Arc<dyn Runtime> = Arc::new(crate::TokioRuntime);
        for status in [403, 410] {
            for streaming in [false, true] {
                for to_writer in [false, true] {
                    let http = if streaming {
                        RoutedHttpClient::streaming(Vec::new(), (status, Vec::new()))
                    } else {
                        RoutedHttpClient::new(Vec::new(), (status, Vec::new()))
                    };
                    let calls = std::sync::Mutex::new(Vec::new());
                    let prepare = |force| {
                        calls.lock().unwrap().push(force);
                        let hosts = if force {
                            Vec::new()
                        } else {
                            vec![MediaHost::new("cdn.example.com")]
                        };
                        let requests = DownloadUtils::prepare_download_requests(
                            &params,
                            &MediaRoute::new(hosts),
                        )
                        .map_err(DownloadRequestError::Prepare);
                        async move { requests }
                    };
                    let sink = SharedWriter::new();
                    sink.with(|w| w.write_all(b"old destination").unwrap());
                    let error = if to_writer {
                        let result = download_to_writer_with_retry(
                            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
                            &runtime,
                            sink.clone(),
                            prepare,
                            || async {},
                            |request, writer| {
                                let http: Arc<dyn HttpClient> = http.clone();
                                let runtime = runtime.clone();
                                async move {
                                    streaming_download_and_decrypt(
                                        &http,
                                        &runtime,
                                        &request,
                                        ExpectedMediaHashes::default(),
                                        writer,
                                    )
                                    .await
                                }
                            },
                        )
                        .await;
                        assert!(sink.contents().is_empty());
                        result.unwrap_err()
                    } else {
                        download_media_with_retry(
                            MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS,
                            prepare,
                            || async {},
                            |request| {
                                let http: Arc<dyn HttpClient> = http.clone();
                                let runtime = runtime.clone();
                                async move {
                                    execute_request_into_memory(
                                        &http,
                                        &runtime,
                                        &request,
                                        ExpectedMediaHashes::default(),
                                        0,
                                    )
                                    .await
                                }
                            },
                        )
                        .await
                        .unwrap_err()
                    };
                    let error = ClientDownloadError::from(error);
                    assert!(matches!(error, ClientDownloadError::NoHostsAfterRefresh(_)));
                    assert_eq!(error.http_status(), Some(status));
                    assert_eq!(http.urls().len(), 1);
                    assert_eq!(*calls.lock().unwrap(), vec![false, true]);
                }
            }
        }
    }

    #[tokio::test]
    async fn client_session_rejection_keeps_iq_metadata_and_source_chain() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore_binary::builder::NodeBuilder;
        let (client, transport) = create_iq_test_client().await;
        let task = tokio::spawn({
            let client = client.clone();
            async move {
                let (params, _) = encrypted_params(b"never fetched");
                client.download(&params).await
            }
        });
        let sent = decode_sent_iq(&transport, 0).await;
        assert!(sent.get().get_optional_child("media_conn").is_some());
        let node = sent.get().to_owned();
        let id = node.attrs().optional_string("id").unwrap().into_owned();
        let response = NodeBuilder::new("iq")
            .attr("type", "error")
            .attr("id", &id)
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("error")
                .attr("code", "429")
                .attr("text", "rate limited")
                .attr("type", "wait")
                .attr("backoff", "17")
                .build()])
            .build();
        answer_iq(&client, &id, &response).await;
        let error = tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        assert!(matches!(
            error,
            ClientDownloadError::MediaSession {
                force_refresh: false,
                ..
            }
        ));
        let rejection = error.server_rejection().unwrap();
        assert_eq!(rejection.code, 429);
        assert_eq!(rejection.error_type, Some("wait"));
        assert_eq!(rejection.backoff, Some(17));
        for diagnostic in [format!("{error}"), format!("{error:?}")] {
            assert!(diagnostic.contains("server_rejection"), "{diagnostic}");
            assert!(diagnostic.contains("429"), "{diagnostic}");
            assert!(!diagnostic.contains("rate limited"), "{diagnostic}");
        }
        let timeout = ClientDownloadError::MediaSession {
            force_refresh: false,
            source: crate::request::IqError::Timeout,
        };
        let diagnostic = format!("{timeout:?}");
        assert!(diagnostic.contains("timeout"), "{diagnostic}");
        assert!(diagnostic.contains("iq_code: None"), "{diagnostic}");
        let ClientDownloadError::MediaSession {
            source: crate::request::IqError::ServerError { response, .. },
            ..
        } = error
        else {
            panic!("expected IQ rejection source");
        };
        assert_eq!(
            response
                .get()
                .get_optional_child("error")
                .unwrap()
                .attrs()
                .optional_u64("backoff"),
            Some(17)
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum DestinationFault {
        Truncate,
        InitialRewind,
        Write,
        WriteZero,
        FinalRewind,
    }

    #[derive(Debug)]
    struct FaultingDestination {
        sink: SharedWriter,
        fault: DestinationFault,
        refuse_cleanup: bool,
        truncates: usize,
        seeks: usize,
        writes: usize,
    }

    impl Write for FaultingDestination {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.writes += 1;
            match self.fault {
                DestinationFault::Write if self.writes > 1 => {
                    Err(std::io::Error::from_raw_os_error(28))
                }
                DestinationFault::Write => self.sink.write(&bytes[..bytes.len().min(1)]),
                DestinationFault::WriteZero => Ok(0),
                _ => self.sink.write(bytes),
            }
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.sink.flush()
        }
    }
    impl Seek for FaultingDestination {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.seeks += 1;
            if matches!(self.fault, DestinationFault::InitialRewind) && self.seeks == 1
                || matches!(self.fault, DestinationFault::FinalRewind) && self.seeks == 2
            {
                return Err(std::io::Error::from_raw_os_error(28));
            }
            self.sink.seek(pos)
        }
    }
    impl DownloadWriter for FaultingDestination {
        fn truncate(&mut self, len: u64) -> std::io::Result<()> {
            self.truncates += 1;
            if self.truncates > 1 && self.refuse_cleanup {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "destination refuses cleanup",
                ));
            }
            if matches!(self.fault, DestinationFault::Truncate) && self.truncates == 1 {
                return Err(std::io::Error::from_raw_os_error(28));
            }
            self.sink.truncate(len)
        }
    }

    #[tokio::test]
    async fn local_destination_faults_are_terminal_and_preserve_io_and_cleanup_sources() {
        for streaming in [false, true] {
            for with_client in [false, true] {
                for refuse_cleanup in [false, true] {
                    for fault in [
                        DestinationFault::Truncate,
                        DestinationFault::InitialRewind,
                        DestinationFault::Write,
                        DestinationFault::WriteZero,
                        DestinationFault::FinalRewind,
                    ] {
                        let (params, body) =
                            encrypted_params(b"authenticated destination contents");
                        let http = if streaming {
                            RoutedHttpClient::streaming(Vec::new(), (200, body))
                        } else {
                            RoutedHttpClient::new(Vec::new(), (200, body))
                        };
                        let sink = SharedWriter::new();
                        sink.with(|inner| *inner = Cursor::new(b"old destination".to_vec()));
                        let writer = FaultingDestination {
                            sink: sink.clone(),
                            fault,
                            refuse_cleanup,
                            truncates: 0,
                            seeks: 0,
                            writes: 0,
                        };
                        let error: anyhow::Error = if with_client {
                            let client = crate::test_utils::create_test_client_with_http(
                                "destination-fault",
                                http.clone(),
                            )
                            .await;
                            *client.media_conn.write().await = Some(media_conn(
                                "unchanged",
                                &["first.example.com", "second.example.com"],
                            ));
                            let error = client
                                .download_to_writer(&params, writer)
                                .await
                                .unwrap_err();
                            // A sink fault must not invalidate or fetch a media session.
                            assert_eq!(
                                client.media_conn.read().await.as_ref().unwrap().auth,
                                "unchanged"
                            );
                            error.into()
                        } else {
                            downloader(http.clone(), &["first.example.com", "second.example.com"])
                                .download_to_writer(&params, writer)
                                .await
                                .unwrap_err()
                                .into()
                        };
                        let (cause, cleanup) = if with_client {
                            match error.downcast_ref::<ClientDownloadError>().unwrap() {
                                ClientDownloadError::WriterIo(cause) => (cause, None),
                                ClientDownloadError::WriterCleanup { failure, cleanup } => {
                                    let ClientDownloadError::WriterIo(cause) = failure.as_ref()
                                    else {
                                        panic!("wrong primary classification: {error:?}");
                                    };
                                    (cause, Some(cleanup))
                                }
                                _ => panic!("wrong local destination classification: {error:?}"),
                            }
                        } else {
                            match error.downcast_ref::<MediaDownloadError>().unwrap() {
                                MediaDownloadError::WriterIo(cause) => (cause, None),
                                MediaDownloadError::WriterCleanup { failure, cleanup } => {
                                    let MediaDownloadError::WriterIo(cause) = failure.as_ref()
                                    else {
                                        panic!("wrong primary classification: {error:?}");
                                    };
                                    (cause, Some(cleanup))
                                }
                                _ => panic!("wrong local destination classification: {error:?}"),
                            }
                        };
                        if matches!(fault, DestinationFault::WriteZero) {
                            assert!(cause.chain().any(|source| {
                                source
                                    .downcast_ref::<std::io::Error>()
                                    .is_some_and(|io| io.kind() == std::io::ErrorKind::WriteZero)
                            }));
                        } else {
                            let io = cause.downcast_ref::<std::io::Error>().unwrap();
                            assert_eq!(io.raw_os_error(), Some(28), "lost original I/O: {error:?}");
                            assert_eq!(
                                io.to_string(),
                                std::io::Error::from_raw_os_error(28).to_string()
                            );
                        }
                        assert_eq!(cleanup.is_some(), refuse_cleanup);
                        if let Some(cleanup) = cleanup {
                            assert_eq!(cleanup.kind(), std::io::ErrorKind::PermissionDenied);
                            assert_eq!(cleanup.to_string(), "destination refuses cleanup");
                        } else {
                            assert!(sink.contents().is_empty());
                            assert_eq!(sink.with(|inner| inner.position()), 0);
                        }
                        let expected_requests = usize::from(
                            !streaming
                                || !matches!(
                                    fault,
                                    DestinationFault::Truncate | DestinationFault::InitialRewind,
                                ),
                        );
                        let urls = http.urls();
                        assert_eq!(
                            urls.len(),
                            expected_requests,
                            "{streaming}/{fault:?}: {urls:?}"
                        );
                        assert!(urls.iter().all(|url| url.contains("first.example.com")));
                    }
                }
            }
        }
    }

    struct FailingNetworkReader;
    impl std::io::Read for FailingNetworkReader {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            // Same OS error as the destination fixture: classify by origin,
            // never ErrorKind or a downcast to arbitrary std::io::Error.
            Err(std::io::Error::from_raw_os_error(28))
        }
    }
    struct ReadFailingHttp {
        inner: Arc<RoutedHttpClient>,
        all_hosts: bool,
    }
    #[async_trait::async_trait]
    impl HttpClient for ReadFailingHttp {
        async fn execute(
            &self,
            request: crate::http::HttpRequest,
        ) -> Result<crate::http::HttpResponse> {
            if self.all_hosts || request.url.contains("first.example.com") {
                self.inner.record(&request.url);
                return Err(std::io::Error::from_raw_os_error(28).into());
            }
            self.inner.execute(request).await
        }
        fn supports_streaming(&self) -> bool {
            self.inner.supports_streaming()
        }
        fn execute_streaming(
            &self,
            request: crate::http::HttpRequest,
        ) -> Result<wacore::net::StreamingHttpResponse> {
            if self.all_hosts || request.url.contains("first.example.com") {
                self.inner.record(&request.url);
                return Ok(wacore::net::StreamingHttpResponse {
                    status_code: 200,
                    body: Box::new(FailingNetworkReader),
                });
            }
            self.inner.execute_streaming(request)
        }
    }

    #[tokio::test]
    async fn http_read_io_still_fails_over_and_is_not_a_local_writer_failure() {
        for streaming in [false, true] {
            for to_writer in [false, true] {
                for all_hosts in [false, true] {
                    let original = b"network failover remains authenticated";
                    let (params, body) = encrypted_params(original);
                    let inner = if streaming {
                        RoutedHttpClient::streaming(Vec::new(), (200, body))
                    } else {
                        RoutedHttpClient::new(Vec::new(), (200, body))
                    };
                    let client = crate::test_utils::create_test_client_with_http(
                        "network-read-fault",
                        Arc::new(ReadFailingHttp {
                            inner: inner.clone(),
                            all_hosts,
                        }),
                    )
                    .await;
                    *client.media_conn.write().await = Some(media_conn(
                        "unchanged",
                        &["first.example.com", "second.example.com"],
                    ));
                    let result = if to_writer {
                        client
                            .download_to_writer(&params, Cursor::new(Vec::new()))
                            .await
                            .map(Cursor::into_inner)
                    } else {
                        client.download(&params).await
                    };
                    if all_hosts {
                        let error = result.unwrap_err();
                        let ClientDownloadError::HostsUnreachable(cause) = error else {
                            panic!("network failure mislabeled: {error:?}");
                        };
                        assert_eq!(
                            cause
                                .downcast_ref::<std::io::Error>()
                                .unwrap()
                                .raw_os_error(),
                            Some(28)
                        );
                    } else {
                        assert_eq!(result.unwrap(), original);
                    }
                    assert_eq!(inner.urls().len(), 2);
                    assert_eq!(
                        client.media_conn.read().await.as_ref().unwrap().auth,
                        "unchanged"
                    );
                }
            }
        }
    }

    /// Allows the initial emptying, then refuses failure cleanup. Its separately
    /// held handle makes the retained unverified bytes observable to the test.
    #[derive(Debug)]
    struct CleanupRefusingWriter {
        sink: SharedWriter,
        cleared: bool,
    }

    impl Write for CleanupRefusingWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.sink.write(buf)
        }
        fn flush(&mut self) -> std::io::Result<()> {
            self.sink.flush()
        }
    }
    impl Seek for CleanupRefusingWriter {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.sink.seek(pos)
        }
    }
    impl DownloadWriter for CleanupRefusingWriter {
        fn truncate(&mut self, len: u64) -> std::io::Result<()> {
            if self.cleared {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "fixture refuses cleanup",
                ));
            }
            self.cleared = true;
            self.sink.truncate(len)
        }
    }

    fn assert_mac_cause(cause: &anyhow::Error) {
        assert!(matches!(
            cause.downcast_ref::<MediaDecryptionError>(),
            Some(MediaDecryptionError::InvalidMac)
        ));
        assert_eq!(cause.chain().count(), 1);
        let diagnostic = format!("{:?}", MediaErrorDiagnostic(cause.as_ref()));
        assert!(
            diagnostic.contains("media_validation: true"),
            "{diagnostic}"
        );
    }

    #[tokio::test]
    async fn mac_cause_survives_cleanup_for_both_http_modes() {
        async fn check<W: DownloadWriter + MaybeSend + 'static>(
            streaming: bool,
            cleanup_fails: bool,
            writer: W,
            sink: SharedWriter,
        ) {
            let key = [0x79; 32];
            let body = forged_body(&vec![0x11; 32 * 1024], &key);
            let http: Arc<dyn HttpClient> = if streaming {
                RoutedHttpClient::streaming(Vec::new(), (200, body))
            } else {
                RoutedHttpClient::new(Vec::new(), (200, body))
            };
            let runtime: Arc<dyn Runtime> = Arc::new(crate::TokioRuntime);
            let request = wacore::download::DownloadRequest {
                url: "https://cdn.example.com/forged".into(),
                decryption: MediaDecryption::Encrypted {
                    media_key: key.to_vec(),
                    media_type: MediaType::Image,
                },
            };
            let (writer, result) = streaming_download_and_decrypt(
                &http,
                &runtime,
                &request,
                ExpectedMediaHashes::default(),
                writer,
            )
            .await
            .unwrap();
            let error = result.unwrap_err();
            let DownloadRequestError::Other(cause) = &error else {
                panic!("unexpected executor failure {error:?}")
            };
            assert_mac_cause(cause);
            let error = discard_failed_write(&runtime, writer, error).await;
            let failure = if cleanup_fails {
                let DownloadRequestError::Cleanup { failure, cleanup } = error else {
                    panic!("expected cleanup failure, got {error:?}");
                };
                assert_eq!(cleanup.kind(), std::io::ErrorKind::PermissionDenied);
                assert!(!sink.contents().is_empty());
                *failure
            } else {
                assert!(sink.contents().is_empty());
                error
            };
            let DownloadRequestError::Other(cause) = failure else {
                panic!("cleanup changed the integrity classification: {failure:?}");
            };
            assert_mac_cause(&cause);
        }
        for streaming in [true, false] {
            let sink = SharedWriter::new();
            sink.with(|w| w.write_all(b"old destination").unwrap());
            check(streaming, false, sink.clone(), sink).await;
            let sink = SharedWriter::new();
            sink.with(|w| w.write_all(b"old destination").unwrap());
            let writer = CleanupRefusingWriter {
                sink: sink.clone(),
                cleared: !streaming,
            };
            check(streaming, true, writer, sink).await;
        }
    }

    #[tokio::test]
    async fn cleanup_failure_preserves_both_causes_and_never_claims_empty_output() {
        let key = [0x79; 32];
        let (mut params, _) = encrypted_params(b"unused");
        params.media_key = Some(key.to_vec());
        let body = forged_body(&vec![0x11; 32 * 1024], &key);
        for with_client in [false, true] {
            let http = RoutedHttpClient::streaming(Vec::new(), (200, body.clone()));
            let sink = SharedWriter::new();
            let writer = CleanupRefusingWriter {
                sink: sink.clone(),
                cleared: false,
            };
            if with_client {
                let client =
                    crate::test_utils::create_test_client_with_http("download-cleanup", http).await;
                *client.media_conn.write().await = Some(media_conn("auth", &["cdn.example.com"]));
                let error = client
                    .download_to_writer(&params, writer)
                    .await
                    .unwrap_err();
                let ClientDownloadError::WriterCleanup { failure, cleanup } = error else {
                    panic!("expected cleanup failure, got {error:?}")
                };
                let ClientDownloadError::HostsUnreachable(cause) = *failure else {
                    panic!("cleanup replaced the download cause: {failure:?}");
                };
                assert_mac_cause(&cause);
                assert_eq!(cleanup.kind(), std::io::ErrorKind::PermissionDenied);
            } else {
                let error = downloader(http, &["cdn.example.com"])
                    .download_to_writer(&params, writer)
                    .await
                    .unwrap_err();
                let MediaDownloadError::WriterCleanup { failure, cleanup } = error else {
                    panic!("expected cleanup failure, got {error:?}")
                };
                let MediaDownloadError::HostsUnreachable(cause) = *failure else {
                    panic!("cleanup replaced the download cause: {failure:?}");
                };
                assert_mac_cause(&cause);
                assert_eq!(cleanup.kind(), std::io::ErrorKind::PermissionDenied);
            }
            assert!(
                !sink.contents().is_empty(),
                "a refused cleanup really can leave unverified plaintext"
            );
        }
    }

    /// HTTP client that records the requested URL and returns a canned response.
    struct CannedHttpClient {
        status: u16,
        body: Vec<u8>,
        seen_url: Mutex<Option<String>>,
    }

    #[async_trait::async_trait]
    impl HttpClient for CannedHttpClient {
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
        let err = client
            .fetch_sticker_pack("P1", "en")
            .await
            .expect_err("a non-200 sticker pack response must fail");
        // Same contract as the media paths: the status is recoverable by type,
        // not only readable in the message.
        let cause: &(dyn std::error::Error + 'static) = err.as_ref();
        assert_eq!(cause.http_status(), Some(404), "got: {err:?}");
    }
}
