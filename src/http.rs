pub use wacore::net::{HttpClient, HttpRequest, HttpResponse};

pub(crate) const HTTP_STATUS_OK: u16 = 200;
pub(crate) const HTTP_STATUS_REDIRECTION_START: u16 = 300;
pub(crate) const HTTP_STATUS_UNAUTHORIZED: u16 = 401;
pub(crate) const HTTP_STATUS_FORBIDDEN: u16 = 403;
pub(crate) const HTTP_STATUS_NOT_FOUND: u16 = 404;
pub(crate) const HTTP_STATUS_GONE: u16 = 410;

#[cfg(feature = "ureq-client")]
pub use whatsapp_rust_ureq_http_client::UreqHttpClient;
