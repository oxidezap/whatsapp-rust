//! Typed recovery over the error chain.
//!
//! [`ErrorChainExt`] answers a few questions about any error without the caller
//! knowing its concrete type, so it need not walk [`std::error::Error::source`]
//! itself nor learn that three different types can carry a server rejection. It
//! is a read-only view with a blanket impl: a domain error added later answers
//! the same questions without implementing anything, and no new error type or
//! parallel hierarchy exists.
//!
//! ```no_run
//! use whatsapp_rust::ErrorChainExt;
//!
//! # fn demo(err: whatsapp_rust::features::GroupError) {
//! if let Some(rejection) = err.server_rejection() {
//!     eprintln!("server said {}: {}", rejection.code, rejection.text);
//! } else if err.is_transport_unavailable() {
//!     eprintln!("offline, will retry");
//! }
//! # }
//! ```
//!
//! From an `anyhow::Error`, annotate the cast: it carries two
//! `AsRef<dyn Error>` impls and both are covered here.
//!
//! ```no_run
//! # use whatsapp_rust::ErrorChainExt;
//! # fn demo(err: whatsapp_rust::anyhow::Error) {
//! let cause: &(dyn std::error::Error + 'static) = err.as_ref();
//! let _ = cause.server_rejection();
//! # }
//! ```
//!
//! # Scope
//!
//! Only questions the crate already answers internally are exposed. There is
//! deliberately no "invalid input", "protocol violation" or "internal" query:
//! each domain spells those as its own `InvalidRequest(String)`-style variant
//! with no shared representation, so any such split would be invented here
//! rather than recovered. [`crate::features::MexError::ExtensionError`] is
//! likewise not reported as a server rejection: its `code` is a GraphQL
//! extension code, a different space from the IQ `code` attribute, and merging
//! the two would make the number meaningless.
//!
//! # Rendering
//!
//! A wrapping variant renders exactly what it wraps, so each error's own
//! `Display` is unchanged. A caller that concatenates the whole chain will see
//! consecutive nodes repeat the same sentence, which is the price of keeping
//! the wrapped error downcastable. Print the innermost cause, or collapse equal
//! neighbours, rather than joining every node.

use std::error::Error as StdError;

use crate::request::IqError as ClientIqError;
use wacore::request::{IqError as CoreIqError, ServerErrorCode};
use wacore::store::error::StoreError;

/// A rejection the server sent in response to a request.
///
/// Borrowed from whichever error in the chain carried it, so recovering one
/// costs no allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ServerRejection<'a> {
    /// The `code` attribute of the `<error>` node.
    pub code: u16,
    /// The `text` attribute; empty when the server sent none.
    pub text: &'a str,
    /// XMPP error class from the `type` attribute (e.g. `"wait"` vs
    /// `"cancel"`); `None` if absent.
    pub error_type: Option<&'a str>,
    /// Server-directed retry delay in seconds from the `backoff` attribute;
    /// `None` if absent.
    pub backoff: Option<u32>,
}

/// Iterator over an error and everything reachable from its
/// [`source`](StdError::source).
#[derive(Clone)]
pub struct Sources<'a> {
    next: Option<&'a (dyn StdError + 'static)>,
}

impl<'a> Iterator for Sources<'a> {
    type Item = &'a (dyn StdError + 'static);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.next?;
        self.next = current.source();
        Some(current)
    }
}

/// Answers a few questions about any error without knowing its concrete type.
///
/// Implemented for every [`std::error::Error`]; see the [module
/// docs](self) for what is deliberately left out.
pub trait ErrorChainExt {
    /// The receiver as a trait object, so the provided methods can walk it.
    #[doc(hidden)]
    fn as_dyn_error(&self) -> &(dyn StdError + 'static);

    /// This error and every error reachable from it, nearest first.
    ///
    /// Use this to recover a domain type this trait does not model.
    fn sources(&self) -> Sources<'_> {
        Sources {
            next: Some(self.as_dyn_error()),
        }
    }

    /// The server rejection behind this error, if any.
    ///
    /// Reports IQ-level rejections only. See the [module docs](self) for why
    /// MEX extension errors are excluded.
    fn server_rejection(&self) -> Option<ServerRejection<'_>> {
        self.sources().find_map(server_rejection_of)
    }

    /// Whether the operation ran out of time waiting for the server.
    ///
    /// Covers a request that got no answer and a connect or handshake step
    /// that never completed.
    fn is_timeout(&self) -> bool {
        self.sources().any(|cause| {
            if let Some(iq) = cause.downcast_ref::<ClientIqError>() {
                return iq.is_timeout();
            }
            if let Some(iq) = cause.downcast_ref::<CoreIqError>() {
                return iq.is_timeout();
            }
            if let Some(connect) = cause.downcast_ref::<crate::client::ConnectError>() {
                return connect.is_timeout();
            }
            cause
                .downcast_ref::<crate::handshake::HandshakeError>()
                .is_some_and(crate::handshake::HandshakeError::is_timeout)
        })
    }

    /// Whether the failure was the transport being gone rather than the
    /// operation being refused.
    ///
    /// Mirrors the judgement the send and receive paths already make when
    /// deciding whether a failure is worth retrying.
    fn is_transport_unavailable(&self) -> bool {
        self.sources().any(|cause| {
            if let Some(client) = cause.downcast_ref::<crate::client::ClientError>() {
                return client.is_transport_unavailable();
            }
            if let Some(iq) = cause.downcast_ref::<ClientIqError>() {
                return iq.is_transport_unavailable();
            }
            if let Some(encrypt) = cause.downcast_ref::<crate::socket::error::EncryptSendError>() {
                return encrypt.is_transport_unavailable();
            }
            cause
                .downcast_ref::<CoreIqError>()
                .is_some_and(CoreIqError::is_transport_unavailable)
        })
    }

    /// The persistence failure behind this error, if any.
    fn store_failure(&self) -> Option<&StoreError> {
        self.sources().find_map(|cause| cause.downcast_ref())
    }
}

fn server_rejection_of<'a>(cause: &'a (dyn StdError + 'static)) -> Option<ServerRejection<'a>> {
    if let Some(CoreIqError::ServerError {
        code,
        text,
        error_type,
        backoff,
    }) = cause.downcast_ref::<CoreIqError>()
    {
        return Some(ServerRejection {
            code: *code,
            text,
            error_type: error_type.as_deref(),
            backoff: *backoff,
        });
    }
    if let Some(ClientIqError::ServerError {
        code,
        text,
        error_type,
        backoff,
    }) = cause.downcast_ref::<ClientIqError>()
    {
        return Some(ServerRejection {
            code: *code,
            text,
            error_type: error_type.as_deref(),
            backoff: *backoff,
        });
    }
    let shared = cause.downcast_ref::<ServerErrorCode>()?;
    Some(ServerRejection {
        code: shared.code,
        text: &shared.text,
        error_type: shared.error_type.as_deref(),
        backoff: shared.backoff,
    })
}

impl<E: StdError + 'static> ErrorChainExt for E {
    fn as_dyn_error(&self) -> &(dyn StdError + 'static) {
        self
    }
}

impl ErrorChainExt for dyn StdError + 'static {
    fn as_dyn_error(&self) -> &(dyn StdError + 'static) {
        self
    }
}

// `anyhow::Error` derefs to this shape, so a caller holding one can reach the
// same answers via `err.as_ref()` without this crate naming `anyhow` in the API.
impl ErrorChainExt for dyn StdError + Send + Sync + 'static {
    fn as_dyn_error(&self) -> &(dyn StdError + 'static) {
        self
    }
}
