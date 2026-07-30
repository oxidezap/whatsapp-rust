pub use wacore::net::{HttpClient, HttpRequest, HttpResponse};

/// An HTTP exchange that completed, carrying the status the client refused it
/// on.
///
/// Attached as the `source` of the errors the media paths return, so a consumer
/// recovers the status by type — [`ErrorChainExt::http_status`] — instead of
/// parsing a message. The same role
/// [`wacore::request::ServerErrorCode`] plays for IQ rejections, and embedded
/// the same way: `anyhow::Error::new(HttpStatusError { .. }).context(..)`.
///
/// Deliberately a different question from an IQ `code`. A CDN refusing a byte
/// range and the chat server refusing a stanza are different layers with
/// different remedies, so one accessor answering for both would tell a consumer
/// the number but not which thing to retry. [`ErrorChainExt`] keeps them apart
/// for the reason it already keeps `MexError`'s GraphQL code out of
/// `server_rejection` — see the [module docs](crate::error).
///
/// [`ErrorChainExt`]: crate::error::ErrorChainExt
/// [`ErrorChainExt::http_status`]: crate::error::ErrorChainExt::http_status
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("HTTP status {status}")]
pub struct HttpStatusError {
    /// The status the server answered with.
    pub status: u16,
}

impl HttpStatusError {
    /// Wraps `context` around this status so the message reads as the operation
    /// that failed while the status stays recoverable underneath it.
    pub(crate) fn into_error(self, context: String) -> anyhow::Error {
        anyhow::Error::new(self).context(context)
    }
}

pub(crate) const HTTP_STATUS_OK: u16 = 200;
pub(crate) const HTTP_STATUS_REDIRECTION_START: u16 = 300;
pub(crate) const HTTP_STATUS_UNAUTHORIZED: u16 = 401;
pub(crate) const HTTP_STATUS_FORBIDDEN: u16 = 403;
pub(crate) const HTTP_STATUS_NOT_FOUND: u16 = 404;
pub(crate) const HTTP_STATUS_GONE: u16 = 410;

#[cfg(feature = "ureq-client")]
pub use whatsapp_rust_ureq_http_client::UreqHttpClient;
