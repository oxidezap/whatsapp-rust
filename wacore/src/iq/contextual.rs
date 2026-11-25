//! Auto-generated IQ stanza specs (WhatsApp 2.3000.1054751086). DO NOT EDIT.
//!
//! One `pub mod` per IQ namespace; each holds the namespace const, shared child
//! types, and one `IqSpec` impl per stanza. Regenerated from the IQ IR by wa-codegen.

// A generated catalog: a consumer uses a subset (so unused specs/types are
// expected), and nested wrapper vars use a `base__path_wrap` convention.
#![allow(
    clippy::all,
    dead_code,
    non_snake_case,
    unused_parens,
    unused_variables,
    unused_imports
)]

macro_rules! iq_bail {
    ($err:expr) => {
        return Err($err)
    };
}

macro_rules! iq_ensure {
    ($cond:expr, $err:expr) => {
        if !($cond) {
            return Err($err);
        }
    };
}

/// Typed error returned by generated IQ response parsers.
#[derive(Clone, PartialEq, Eq)]
pub enum IqResponseError {
    RequestContextNotIq,
    RequestContextMissingId,
    RequestContextMissingTo,
    RequestContextRequired {
        module: &'static str,
        guard: &'static str,
    },
    CorrelationMismatch {
        module: &'static str,
        field: &'static str,
        expected: String,
    },
    ResponseGuardFailed {
        module: &'static str,
    },
    MissingChild {
        tag: &'static str,
    },
    MissingAttribute {
        name: &'static str,
    },
    MissingContent {
        tag: String,
    },
    NotANumber {
        field: &'static str,
    },
    NotAJid {
        field: &'static str,
    },
    OutOfRange {
        field: &'static str,
        value: String,
    },
    WrongLength {
        field: &'static str,
        length: usize,
    },
    ValueNotAccepted {
        field: &'static str,
        value: String,
    },
    ValuePinnedMismatch {
        field: &'static str,
        expected: &'static str,
        value: String,
    },
    PayloadRejected {
        union_name: &'static str,
        arm: &'static str,
    },
    InvalidErrorCode,
    NoErrorPayloadMatched,
    RequiredPayloadMismatch {
        variant: &'static str,
        field: &'static str,
    },
    NoResponseVariantMatched {
        union_name: &'static str,
    },
    NotSuccessResponse {
        reason: String,
    },
    Rejection {
        module: &'static str,
        reason: &'static str,
    },
    Other(String),
}

impl std::fmt::Display for IqResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RequestContextNotIq => write!(f, "request context is not an IQ"),
            Self::RequestContextMissingId => write!(f, "request context has no id"),
            Self::RequestContextMissingTo => write!(f, "request context has no to"),
            Self::RequestContextRequired { module, guard } => {
                write!(
                    f,
                    "{module}: guards.{guard}: use parse_response_with_request"
                )
            }
            Self::CorrelationMismatch {
                module,
                field,
                expected,
            } => {
                write!(f, "{module}: response.{field} does not match {expected}")
            }
            Self::ResponseGuardFailed { module } => {
                write!(f, "{module}: response guard failed")
            }
            Self::MissingChild { tag } => write!(f, "missing <{tag}>"),
            Self::MissingAttribute { name } => write!(f, "missing {name}"),
            Self::MissingContent { tag } => write!(f, "<{tag}> has no content"),
            Self::NotANumber { field } => write!(f, "{field} not a number"),
            Self::NotAJid { field } => write!(f, "{field} not a JID"),
            Self::OutOfRange { field, value } => write!(f, "{field} out of range: {value}"),
            Self::WrongLength { field, length } => write!(f, "{field} wrong length: {length}"),
            Self::ValueNotAccepted { field, value } => write!(f, "{field} not accepted: {value}"),
            Self::ValuePinnedMismatch {
                field,
                expected,
                value,
            } => {
                write!(f, "{field} is pinned to {expected}: {value}")
            }
            Self::PayloadRejected { union_name, arm } => {
                write!(f, "{union_name}: {arm} payload rejected")
            }
            Self::InvalidErrorCode => write!(f, "invalid error code"),
            Self::NoErrorPayloadMatched => write!(f, "no error payload matched"),
            Self::RequiredPayloadMismatch { variant, field } => {
                write!(f, "{variant}: required payload {field} did not match")
            }
            Self::NoResponseVariantMatched { union_name } => {
                write!(f, "{union_name}: no response variant matched")
            }
            Self::NotSuccessResponse { reason } => {
                write!(f, "not a success response: {reason}")
            }
            Self::Rejection { module, reason } => write!(f, "{module}: {reason}"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::fmt::Debug for IqResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for IqResponseError {}

impl From<std::num::ParseIntError> for IqResponseError {
    fn from(_: std::num::ParseIntError) -> Self {
        Self::NotANumber { field: "unknown" }
    }
}

/// IQ namespace `md`. Source: WASmaxMdGetCountryCodeRPC.
pub mod md {
    pub use super::IqResponseError;
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const MD_NAMESPACE: &str = "md";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetCountryCodeRequest:get IQ spec.
    ///
    /// Source: `WASmaxMdGetCountryCodeRPC`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestSpec;

    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestGetCountryCodeResponse {
        pub country_code_iso: String,
        pub r#type: String,
    }

    /// Discriminated union for response field `errorCountryCodeErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeGetCountryCodeRequestErrorErrorCountryCodeErrors {
        IQErrorBadRequest(MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorBadRequest),
        IQErrorInternalServerError(
            MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorInternalServerError,
        ),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorBadRequest {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorInternalServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestError {
        pub r#type: String,
        pub error_country_code_errors: Option<MakeGetCountryCodeRequestErrorErrorCountryCodeErrors>,
    }

    /// makeGetCountryCodeRequest:get — RPC outcome union (2 variants).
    #[derive(Debug, Clone)]
    pub enum MakeGetCountryCodeRequestResponse {
        GetCountryCodeResponse(MakeGetCountryCodeRequestGetCountryCodeResponse),
        Error(MakeGetCountryCodeRequestError),
    }

    impl IqSpec for MakeGetCountryCodeRequestSpec {
        type Response = MakeGetCountryCodeRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut link_code_companion_reg_node = NodeBuilder::new("link_code_companion_reg");
            link_code_companion_reg_node =
                link_code_companion_reg_node.attr("stage", "get_country_code");
            let link_code_companion_reg_node = link_code_companion_reg_node.build();

            InfoQuery::get(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![link_code_companion_reg_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxMdGetCountryCodeRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn handles_iq_errors(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeGetCountryCodeRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeGetCountryCodeRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeGetCountryCodeRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeGetCountryCodeRequestResponse, IqResponseError> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxMdGetCountryCodeRPC",
                    field: "from",
                    expected: request_to.to_string()
                });
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxMdGetCountryCodeRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            fn __iq_error_code(value: &str) -> Option<i64> {
                let value = value.trim_start_matches(|c| matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'));
                let offset =
                    usize::from(matches!(value.as_bytes().first(), Some(b'+') | Some(b'-')));
                let end = offset
                    + value
                        .bytes()
                        .skip(offset)
                        .take_while(u8::is_ascii_digit)
                        .count();
                if end == offset {
                    None
                } else {
                    value[..end].parse().ok()
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
            {
                let __r: Result<MakeGetCountryCodeRequestGetCountryCodeResponse, IqResponseError> = (|| -> Result<MakeGetCountryCodeRequestGetCountryCodeResponse, IqResponseError> {


                    let country_code_wrap = response.get_optional_child("country_code")
                        .ok_or(IqResponseError::MissingChild { tag: "country_code" })?;
                    let country_code_iso = country_code_wrap.get_attr("iso")
                        .ok_or(IqResponseError::MissingAttribute { name: "iso" })?
                        .as_str()
                        .to_string();
                    let r#type = response.get_attr("type")
                        .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                        .as_str()
                        .to_string();
                    iq_ensure!(r#type.as_str() == "result", IqResponseError::ValuePinnedMismatch { field: "type", expected: "result", value: format!("{:?}", r#type) });
                    Ok(MakeGetCountryCodeRequestGetCountryCodeResponse {
                        country_code_iso,
                        r#type,
                        ..Default::default()
                    })
                })();
                return Ok(MakeGetCountryCodeRequestResponse::GetCountryCodeResponse(
                    __r?,
                ));
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 400i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("bad-request"))
                                        || (code == 500i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("internal-server-error"))
                                })
                    }))
            {
                let __r: Result<MakeGetCountryCodeRequestError, IqResponseError> =
                    (|| -> Result<MakeGetCountryCodeRequestError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeGetCountryCodeRequestErrorErrorCountryCodeErrors, IqResponseError> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorBadRequest, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "bad-request", IqResponseError::ValuePinnedMismatch { field: "text", expected: "bad-request", value: format!("{:?}", text) });
                                Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorInternalServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "internal-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "internal-server-error", value: format!("{:?}", text) });
                                Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrors::IQErrorInternalServerError(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeGetCountryCodeRequestError {
                            r#type: "error".to_string(),
                            error_country_code_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_country_code_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "GetCountryCodeResponseError",
                            field: "errorCountryCodeErrors"
                        });
                    }
                    Ok(value)
                });
                return Ok(MakeGetCountryCodeRequestResponse::Error(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakeGetCountryCodeRequestResponse"
            })
        }
    }
}

/// IQ namespace `passive`. Source: WASmaxPassiveModeActiveIQRPC, WASmaxPassiveModePassiveIQRPC.
pub mod passive {
    pub use super::IqResponseError;
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const PASSIVE_NAMESPACE: &str = "passive";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeActiveIQRequest:set IQ spec.
    ///
    /// Source: `WASmaxPassiveModeActiveIQRPC`
    #[derive(Debug, Clone, Default)]
    pub struct MakeActiveIQRequestSpec;

    #[derive(Debug, Clone, Default)]
    pub struct MakeActiveIQRequestActiveIQResponseSuccess {
        pub r#type: String,
        pub from: Jid,
    }

    /// makeActiveIQRequest:set — RPC outcome union (1 variants).
    #[derive(Debug, Clone)]
    pub enum MakeActiveIQRequestResponse {
        ActiveIQResponseSuccess(MakeActiveIQRequestActiveIQResponseSuccess),
    }

    impl IqSpec for MakeActiveIQRequestSpec {
        type Response = MakeActiveIQRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            let active_node = NodeBuilder::new("active").build();

            InfoQuery::set(
                PASSIVE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![active_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxPassiveModeActiveIQRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeActiveIQRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeActiveIQRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeActiveIQRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeActiveIQRequestResponse, IqResponseError> {
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxPassiveModeActiveIQRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
                && response.get_attr("from").map(|x| x.as_str()).as_deref()
                    == Some("s.whatsapp.net")
                && (response.content_bytes().is_none()
                    && response.get_children_by_tag("active").count() == 1)
            {
                let __r: Result<MakeActiveIQRequestActiveIQResponseSuccess, IqResponseError> =
                    (|| -> Result<MakeActiveIQRequestActiveIQResponseSuccess, IqResponseError> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                            .as_str()
                            .to_string();
                        iq_ensure!(
                            r#type.as_str() == "result",
                            IqResponseError::ValuePinnedMismatch {
                                field: "type",
                                expected: "result",
                                value: format!("{:?}", r#type)
                            }
                        );
                        let from = response
                            .get_attr("from")
                            .ok_or(IqResponseError::MissingAttribute { name: "from" })?
                            .to_jid()
                            .ok_or(IqResponseError::NotAJid { field: "from" })?;
                        iq_ensure!(
                            response.get_attr("from").map(|v| v.as_str()).as_deref()
                                == Some("s.whatsapp.net"),
                            IqResponseError::ValueNotAccepted {
                                field: "from",
                                value: format!(
                                    "{:?}",
                                    response.get_attr("from").map(|v| v.as_str()).as_deref()
                                )
                            }
                        );
                        Ok(MakeActiveIQRequestActiveIQResponseSuccess {
                            r#type,
                            from,
                            ..Default::default()
                        })
                    })();
                return Ok(MakeActiveIQRequestResponse::ActiveIQResponseSuccess(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakeActiveIQRequestResponse"
            })
        }
    }

    /// makePassiveIQRequest:set IQ spec.
    ///
    /// Source: `WASmaxPassiveModePassiveIQRPC`
    #[derive(Debug, Clone, Default)]
    pub struct MakePassiveIQRequestSpec;

    #[derive(Debug, Clone, Default)]
    pub struct MakePassiveIQRequestPassiveIQResponseSuccess {
        pub r#type: String,
        pub from: Jid,
    }

    /// makePassiveIQRequest:set — RPC outcome union (1 variants).
    #[derive(Debug, Clone)]
    pub enum MakePassiveIQRequestResponse {
        PassiveIQResponseSuccess(MakePassiveIQRequestPassiveIQResponseSuccess),
    }

    impl IqSpec for MakePassiveIQRequestSpec {
        type Response = MakePassiveIQRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            let passive_node = NodeBuilder::new("passive").build();

            InfoQuery::set(
                PASSIVE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![passive_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxPassiveModePassiveIQRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakePassiveIQRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakePassiveIQRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakePassiveIQRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakePassiveIQRequestResponse, IqResponseError> {
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxPassiveModePassiveIQRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
                && response.get_attr("from").map(|x| x.as_str()).as_deref()
                    == Some("s.whatsapp.net")
                && (response.content_bytes().is_none()
                    && response.get_children_by_tag("passive").count() == 1)
            {
                let __r: Result<MakePassiveIQRequestPassiveIQResponseSuccess, IqResponseError> = (|| -> Result<MakePassiveIQRequestPassiveIQResponseSuccess, IqResponseError> {


                    let r#type = response.get_attr("type")
                        .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                        .as_str()
                        .to_string();
                    iq_ensure!(r#type.as_str() == "result", IqResponseError::ValuePinnedMismatch { field: "type", expected: "result", value: format!("{:?}", r#type) });
                    let from = response.get_attr("from").ok_or(IqResponseError::MissingAttribute { name: "from" })?.to_jid().ok_or(IqResponseError::NotAJid { field: "from" })?;
                    iq_ensure!(response.get_attr("from").map(|v| v.as_str()).as_deref() == Some("s.whatsapp.net"), IqResponseError::ValueNotAccepted { field: "from", value: format!("{:?}", response.get_attr("from").map(|v| v.as_str()).as_deref()) });
                    Ok(MakePassiveIQRequestPassiveIQResponseSuccess {
                        r#type,
                        from,
                        ..Default::default()
                    })
                })();
                return Ok(MakePassiveIQRequestResponse::PassiveIQResponseSuccess(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakePassiveIQRequestResponse"
            })
        }
    }
}

/// IQ namespace `w:g2`. Source: WASmaxGroupsAcceptGroupAddRPC, WASmaxGroupsDeleteParentGroupRPC, WASmaxGroupsJoinLinkedGroupRPC, WASmaxGroupsSetSubjectRPC.
pub mod w_g2 {
    pub use super::IqResponseError;
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_G2_NAMESPACE: &str = "w:g2";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeAcceptGroupAddRequest:set IQ spec.
    ///
    /// Source: `WASmaxGroupsAcceptGroupAddRPC`
    #[derive(Debug, Clone)]
    pub struct MakeAcceptGroupAddRequestSpec {
        pub code: String,
        pub expiration: u64,
        pub admin: Jid,
        pub target: Jid,
    }

    impl MakeAcceptGroupAddRequestSpec {
        pub fn new(code: impl Into<String>, expiration: u64, admin: &Jid, target: Jid) -> Self {
            Self {
                code: code.into(),
                expiration,
                admin: admin.clone(),
                target,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestGroupJoinRequestSuccess {
        pub r#type: String,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestSuccess {
        pub r#type: String,
    }

    /// Discriminated union for response field `errorAcceptGroupAddClientErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors {
        IQErrorAlreadyExists(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorAlreadyExists),
        IQErrorBadRequest(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorBadRequest),
        IQErrorNotAuthorized(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAuthorized),
        IQErrorForbidden(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorForbidden),
        IQErrorItemNotFound(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorItemNotFound),
        IQErrorConflict(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorConflict),
        IQErrorGone(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorGone),
        IQErrorResourceLimit(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceLimit),
        IQErrorLocked(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorLocked),
        IQErrorResourceConstraint(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceConstraint),
        IQErrorNotAllowed(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAllowed),
        IQErrorParentLinkedGroupsParticipantsResourceLimit(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorParentLinkedGroupsParticipantsResourceLimit),
        IQErrorFallbackClient(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorFallbackClient),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorAlreadyExists {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorBadRequest {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAuthorized {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorForbidden {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorItemNotFound {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorConflict {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorGone {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceLimit {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorLocked {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceConstraint
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAllowed {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorParentLinkedGroupsParticipantsResourceLimit
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorFallbackClient
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestClientError {
        pub r#type: String,
        pub error_accept_group_add_client_errors:
            Option<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors>,
    }

    /// Discriminated union for response field `errorServerErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeAcceptGroupAddRequestServerErrorErrorServerErrors {
        IQErrorInternalServerError(
            MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorInternalServerError,
        ),
        IQErrorServiceUnavailable(
            MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable,
        ),
        IQErrorPartialServerError(
            MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorPartialServerError,
        ),
        IQErrorFallbackServer(
            MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorFallbackServer,
        ),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestServerError {
        pub r#type: String,
        pub error_server_errors: Option<MakeAcceptGroupAddRequestServerErrorErrorServerErrors>,
    }

    /// makeAcceptGroupAddRequest:set — RPC outcome union (4 variants).
    #[derive(Debug, Clone)]
    pub enum MakeAcceptGroupAddRequestResponse {
        GroupJoinRequestSuccess(MakeAcceptGroupAddRequestGroupJoinRequestSuccess),
        Success(MakeAcceptGroupAddRequestSuccess),
        ClientError(MakeAcceptGroupAddRequestClientError),
        ServerError(MakeAcceptGroupAddRequestServerError),
    }

    impl IqSpec for MakeAcceptGroupAddRequestSpec {
        type Response = MakeAcceptGroupAddRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            // `target` is the group's own JID (`<group>@g.us`) — not the bare `g.us` server.
            let mut accept_node = NodeBuilder::new("accept");
            accept_node = accept_node.attr("code", &*self.code);
            accept_node = accept_node.attr("expiration", self.expiration.to_string());
            accept_node = accept_node.attr("admin", self.admin.clone());
            let accept_node = accept_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                self.target.clone(),
                Some(NodeContent::Nodes(vec![accept_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxGroupsAcceptGroupAddRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn handles_iq_errors(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeAcceptGroupAddRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeAcceptGroupAddRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeAcceptGroupAddRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeAcceptGroupAddRequestResponse, IqResponseError> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsAcceptGroupAddRPC",
                    field: "from",
                    expected: request_to.to_string()
                });
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsAcceptGroupAddRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            fn __iq_error_code(value: &str) -> Option<i64> {
                let value = value.trim_start_matches(|c| matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'));
                let offset =
                    usize::from(matches!(value.as_bytes().first(), Some(b'+') | Some(b'-')));
                let end = offset
                    + value
                        .bytes()
                        .skip(offset)
                        .take_while(u8::is_ascii_digit)
                        .count();
                if end == offset {
                    None
                } else {
                    value[..end].parse().ok()
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
                && (response.content_bytes().is_none()
                    && response
                        .get_children_by_tag("membership_approval_request")
                        .count()
                        == 1)
            {
                let __r: Result<MakeAcceptGroupAddRequestGroupJoinRequestSuccess, IqResponseError> = (|| -> Result<MakeAcceptGroupAddRequestGroupJoinRequestSuccess, IqResponseError> {


                    let r#type = response.get_attr("type")
                        .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                        .as_str()
                        .to_string();
                    iq_ensure!(r#type.as_str() == "result", IqResponseError::ValuePinnedMismatch { field: "type", expected: "result", value: format!("{:?}", r#type) });
                    Ok(MakeAcceptGroupAddRequestGroupJoinRequestSuccess {
                        r#type,
                        ..Default::default()
                    })
                })();
                if let Ok(__v) = __r {
                    return Ok(MakeAcceptGroupAddRequestResponse::GroupJoinRequestSuccess(
                        __v,
                    ));
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
            {
                let __r: Result<MakeAcceptGroupAddRequestSuccess, IqResponseError> =
                    (|| -> Result<MakeAcceptGroupAddRequestSuccess, IqResponseError> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                            .as_str()
                            .to_string();
                        iq_ensure!(
                            r#type.as_str() == "result",
                            IqResponseError::ValuePinnedMismatch {
                                field: "type",
                                expected: "result",
                                value: format!("{:?}", r#type)
                            }
                        );
                        Ok(MakeAcceptGroupAddRequestSuccess {
                            r#type,
                            ..Default::default()
                        })
                    })();
                return Ok(MakeAcceptGroupAddRequestResponse::Success(__r?));
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 304i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("already-exists"))
                                        || (code == 400i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("bad-request"))
                                        || (code == 401i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-authorized"))
                                        || (code == 403i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("forbidden"))
                                        || (code == 404i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("item-not-found"))
                                        || (code == 409i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("conflict"))
                                        || (code == 410i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("gone"))
                                        || (code == 419i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("resource-limit"))
                                        || (code == 423i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("locked"))
                                        || (code == 500i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("resource-constraint"))
                                        || (code == 405i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-allowed"))
                                        || (code == 412i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some(
                                                    "linked-groups-participants-resource-limit",
                                                ))
                                        || (code >= 400i64 && code <= 499i64)
                                })
                    }))
            {
                let __r: Result<MakeAcceptGroupAddRequestClientError, IqResponseError> =
                    (|| -> Result<MakeAcceptGroupAddRequestClientError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors, IqResponseError> {
                        if (code == 304i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("already-exists")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorAlreadyExists, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "already-exists", IqResponseError::ValuePinnedMismatch { field: "text", expected: "already-exists", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorAlreadyExists {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorAlreadyExists(value)); }
                        }
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorBadRequest, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "bad-request", IqResponseError::ValuePinnedMismatch { field: "text", expected: "bad-request", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAuthorized, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-authorized", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-authorized", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 403i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("forbidden")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorForbidden, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "forbidden", IqResponseError::ValuePinnedMismatch { field: "text", expected: "forbidden", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorForbidden {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorForbidden(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorItemNotFound, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "item-not-found", IqResponseError::ValuePinnedMismatch { field: "text", expected: "item-not-found", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorItemNotFound(value)); }
                        }
                        if (code == 409i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("conflict")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorConflict, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "conflict", IqResponseError::ValuePinnedMismatch { field: "text", expected: "conflict", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorConflict {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorConflict(value)); }
                        }
                        if (code == 410i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("gone")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorGone, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "gone", IqResponseError::ValuePinnedMismatch { field: "text", expected: "gone", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorGone {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorGone(value)); }
                        }
                        if (code == 419i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("resource-limit")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceLimit, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "resource-limit", IqResponseError::ValuePinnedMismatch { field: "text", expected: "resource-limit", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceLimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorResourceLimit(value)); }
                        }
                        if (code == 423i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("locked")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorLocked, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "locked", IqResponseError::ValuePinnedMismatch { field: "text", expected: "locked", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorLocked {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorLocked(value)); }
                        }
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("resource-constraint")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceConstraint, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "resource-constraint", IqResponseError::ValuePinnedMismatch { field: "text", expected: "resource-constraint", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceConstraint {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorResourceConstraint(value)); }
                        }
                        if (code == 405i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-allowed")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAllowed, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-allowed", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-allowed", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAllowed {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorNotAllowed(value)); }
                        }
                        if (code == 412i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("linked-groups-participants-resource-limit")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorParentLinkedGroupsParticipantsResourceLimit, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "linked-groups-participants-resource-limit", IqResponseError::ValuePinnedMismatch { field: "text", expected: "linked-groups-participants-resource-limit", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorParentLinkedGroupsParticipantsResourceLimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorParentLinkedGroupsParticipantsResourceLimit(value)); }
                        }
                        if (code >= 400i64 && code <= 499i64) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorFallbackClient, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorFallbackClient {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorFallbackClient(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeAcceptGroupAddRequestClientError {
                            r#type: "error".to_string(),
                            error_accept_group_add_client_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_accept_group_add_client_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "AcceptGroupAddResponseClientError",
                            field: "errorAcceptGroupAddClientErrors"
                        });
                    }
                    Ok(value)
                });
                if let Ok(__v) = __r {
                    return Ok(MakeAcceptGroupAddRequestResponse::ClientError(__v));
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 500i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("internal-server-error"))
                                        || (code == 503i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("service-unavailable"))
                                        || (code == 530i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("partial-server-error"))
                                        || (code >= 500i64 && code <= 599i64)
                                })
                    }))
            {
                let __r: Result<MakeAcceptGroupAddRequestServerError, IqResponseError> =
                    (|| -> Result<MakeAcceptGroupAddRequestServerError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrors, IqResponseError> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorInternalServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "internal-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "internal-server-error", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "service-unavailable", IqResponseError::ValuePinnedMismatch { field: "text", expected: "service-unavailable", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorPartialServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "partial-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "partial-server-error", value: format!("{:?}", text) });
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorFallbackServer, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeAcceptGroupAddRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_server_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "AcceptGroupAddResponseServerError",
                            field: "errorServerErrors"
                        });
                    }
                    Ok(value)
                });
                return Ok(MakeAcceptGroupAddRequestResponse::ServerError(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakeAcceptGroupAddRequestResponse"
            })
        }
    }

    /// makeDeleteParentGroupRequest:set IQ spec.
    ///
    /// Source: `WASmaxGroupsDeleteParentGroupRPC`
    #[derive(Debug, Clone)]
    pub struct MakeDeleteParentGroupRequestSpec {
        pub target: Jid,
    }

    impl MakeDeleteParentGroupRequestSpec {
        pub fn new(target: Jid) -> Self {
            Self { target }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestSuccess {
        pub r#type: String,
    }

    /// Discriminated union for response field `errorDeleteParentGroupClientError`.
    #[derive(Debug, Clone)]
    pub enum MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError {
        IQErrorBadRequest(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorBadRequest),
        IQErrorNotAuthorized(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorNotAuthorized),
        IQErrorItemNotFound(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorItemNotFound),
        IQErrorRateOverlimit(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorRateOverlimit),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorBadRequest
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorNotAuthorized
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorItemNotFound
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorRateOverlimit
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestClientError {
        pub r#type: String,
        pub error_delete_parent_group_client_error:
            Option<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError>,
    }

    /// Discriminated union for response field `errorServerErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeDeleteParentGroupRequestServerErrorErrorServerErrors {
        IQErrorInternalServerError(
            MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError,
        ),
        IQErrorServiceUnavailable(
            MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable,
        ),
        IQErrorPartialServerError(
            MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError,
        ),
        IQErrorFallbackServer(
            MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer,
        ),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestServerError {
        pub r#type: String,
        pub error_server_errors: Option<MakeDeleteParentGroupRequestServerErrorErrorServerErrors>,
    }

    /// makeDeleteParentGroupRequest:set — RPC outcome union (3 variants).
    #[derive(Debug, Clone)]
    pub enum MakeDeleteParentGroupRequestResponse {
        Success(MakeDeleteParentGroupRequestSuccess),
        ClientError(MakeDeleteParentGroupRequestClientError),
        ServerError(MakeDeleteParentGroupRequestServerError),
    }

    impl IqSpec for MakeDeleteParentGroupRequestSpec {
        type Response = MakeDeleteParentGroupRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            // `target` is the group's own JID (`<group>@g.us`) — not the bare `g.us` server.
            let delete_parent_node = NodeBuilder::new("delete_parent").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                self.target.clone(),
                Some(NodeContent::Nodes(vec![delete_parent_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxGroupsDeleteParentGroupRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn handles_iq_errors(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeDeleteParentGroupRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeDeleteParentGroupRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeDeleteParentGroupRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeDeleteParentGroupRequestResponse, IqResponseError> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsDeleteParentGroupRPC",
                    field: "from",
                    expected: request_to.to_string()
                });
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsDeleteParentGroupRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            fn __iq_error_code(value: &str) -> Option<i64> {
                let value = value.trim_start_matches(|c| matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'));
                let offset =
                    usize::from(matches!(value.as_bytes().first(), Some(b'+') | Some(b'-')));
                let end = offset
                    + value
                        .bytes()
                        .skip(offset)
                        .take_while(u8::is_ascii_digit)
                        .count();
                if end == offset {
                    None
                } else {
                    value[..end].parse().ok()
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
            {
                let __r: Result<MakeDeleteParentGroupRequestSuccess, IqResponseError> =
                    (|| -> Result<MakeDeleteParentGroupRequestSuccess, IqResponseError> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                            .as_str()
                            .to_string();
                        iq_ensure!(
                            r#type.as_str() == "result",
                            IqResponseError::ValuePinnedMismatch {
                                field: "type",
                                expected: "result",
                                value: format!("{:?}", r#type)
                            }
                        );
                        Ok(MakeDeleteParentGroupRequestSuccess {
                            r#type,
                            ..Default::default()
                        })
                    })();
                return Ok(MakeDeleteParentGroupRequestResponse::Success(__r?));
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 400i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("bad-request"))
                                        || (code == 401i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-authorized"))
                                        || (code == 404i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("item-not-found"))
                                        || (code == 429i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("rate-overlimit"))
                                })
                    }))
            {
                let __r: Result<MakeDeleteParentGroupRequestClientError, IqResponseError> =
                    (|| -> Result<MakeDeleteParentGroupRequestClientError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError, IqResponseError> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorBadRequest, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "bad-request", IqResponseError::ValuePinnedMismatch { field: "text", expected: "bad-request", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorBadRequest(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorNotAuthorized, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-authorized", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-authorized", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorItemNotFound, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "item-not-found", IqResponseError::ValuePinnedMismatch { field: "text", expected: "item-not-found", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorItemNotFound(value)); }
                        }
                        if (code == 429i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("rate-overlimit")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorRateOverlimit, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "rate-overlimit", IqResponseError::ValuePinnedMismatch { field: "text", expected: "rate-overlimit", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorRateOverlimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorRateOverlimit(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeDeleteParentGroupRequestClientError {
                            r#type: "error".to_string(),
                            error_delete_parent_group_client_error: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_delete_parent_group_client_error.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "DeleteParentGroupResponseClientError",
                            field: "errorDeleteParentGroupClientError"
                        });
                    }
                    Ok(value)
                });
                if let Ok(__v) = __r {
                    return Ok(MakeDeleteParentGroupRequestResponse::ClientError(__v));
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 500i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("internal-server-error"))
                                        || (code == 503i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("service-unavailable"))
                                        || (code == 530i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("partial-server-error"))
                                        || (code >= 500i64 && code <= 599i64)
                                })
                    }))
            {
                let __r: Result<MakeDeleteParentGroupRequestServerError, IqResponseError> =
                    (|| -> Result<MakeDeleteParentGroupRequestServerError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrors, IqResponseError> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "internal-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "internal-server-error", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "service-unavailable", IqResponseError::ValuePinnedMismatch { field: "text", expected: "service-unavailable", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "partial-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "partial-server-error", value: format!("{:?}", text) });
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeDeleteParentGroupRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_server_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "DeleteParentGroupResponseServerError",
                            field: "errorServerErrors"
                        });
                    }
                    Ok(value)
                });
                return Ok(MakeDeleteParentGroupRequestResponse::ServerError(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakeDeleteParentGroupRequestResponse"
            })
        }
    }

    /// makeJoinLinkedGroupRequest:set IQ spec.
    ///
    /// Source: `WASmaxGroupsJoinLinkedGroupRPC`
    #[derive(Debug, Clone)]
    pub struct MakeJoinLinkedGroupRequestSpec {
        pub r#type: Option<String>,
        pub jid: Jid,
        pub target: Jid,
    }

    impl MakeJoinLinkedGroupRequestSpec {
        pub fn new(r#type: Option<String>, jid: &Jid, target: Jid) -> Self {
            Self {
                r#type,
                jid: jid.clone(),
                target,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestGroupJoinRequestSuccess {
        pub r#type: String,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestSuccess {
        pub r#type: String,
    }

    /// Discriminated union for response field `errorJoinLinkedGroupClientErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors {
        IQErrorBadRequest(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorBadRequest),
        IQErrorForbidden(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorForbidden),
        IQErrorItemNotFound(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorItemNotFound),
        IQErrorNotAllowed(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAllowed),
        IQErrorNotAcceptable(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAcceptable),
        IQErrorConflict(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorConflict),
        IQErrorResourceLimit(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorResourceLimit),
        IQErrorNotAuthorized(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAuthorized),
        IQErrorUpgradeRequired(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorUpgradeRequired),
        IQErrorAlreadyExists(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorAlreadyExists),
        IQErrorFallbackClient(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorFallbackClient),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorBadRequest {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorForbidden {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorItemNotFound
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAllowed {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAcceptable
    {
        pub text: String,
        pub code: u64,
        pub name: String,
        pub reason: String,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorConflict {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorResourceLimit
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAuthorized
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorUpgradeRequired
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorAlreadyExists
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorFallbackClient
    {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestClientError {
        pub r#type: String,
        pub error_join_linked_group_client_errors:
            Option<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors>,
    }

    /// Discriminated union for response field `errorServerErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeJoinLinkedGroupRequestServerErrorErrorServerErrors {
        IQErrorInternalServerError(
            MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError,
        ),
        IQErrorServiceUnavailable(
            MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable,
        ),
        IQErrorPartialServerError(
            MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError,
        ),
        IQErrorFallbackServer(
            MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer,
        ),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestServerError {
        pub r#type: String,
        pub error_server_errors: Option<MakeJoinLinkedGroupRequestServerErrorErrorServerErrors>,
    }

    /// makeJoinLinkedGroupRequest:set — RPC outcome union (4 variants).
    #[derive(Debug, Clone)]
    pub enum MakeJoinLinkedGroupRequestResponse {
        GroupJoinRequestSuccess(MakeJoinLinkedGroupRequestGroupJoinRequestSuccess),
        Success(MakeJoinLinkedGroupRequestSuccess),
        ClientError(MakeJoinLinkedGroupRequestClientError),
        ServerError(MakeJoinLinkedGroupRequestServerError),
    }

    impl IqSpec for MakeJoinLinkedGroupRequestSpec {
        type Response = MakeJoinLinkedGroupRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            // `target` is the group's own JID (`<group>@g.us`) — not the bare `g.us` server.
            let mut join_linked_group_node = NodeBuilder::new("join_linked_group");
            if let Some(v) = &self.r#type {
                join_linked_group_node = join_linked_group_node.attr("type", v.as_str());
            }
            join_linked_group_node = join_linked_group_node.attr("jid", self.jid.clone());
            let join_linked_group_node = join_linked_group_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                self.target.clone(),
                Some(NodeContent::Nodes(vec![join_linked_group_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxGroupsJoinLinkedGroupRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn handles_iq_errors(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeJoinLinkedGroupRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeJoinLinkedGroupRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeJoinLinkedGroupRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeJoinLinkedGroupRequestResponse, IqResponseError> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsJoinLinkedGroupRPC",
                    field: "from",
                    expected: request_to.to_string()
                });
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsJoinLinkedGroupRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            fn __iq_error_code(value: &str) -> Option<i64> {
                let value = value.trim_start_matches(|c| matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'));
                let offset =
                    usize::from(matches!(value.as_bytes().first(), Some(b'+') | Some(b'-')));
                let end = offset
                    + value
                        .bytes()
                        .skip(offset)
                        .take_while(u8::is_ascii_digit)
                        .count();
                if end == offset {
                    None
                } else {
                    value[..end].parse().ok()
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
                && (response.content_bytes().is_none()
                    && response
                        .get_children_by_tag("membership_approval_request")
                        .count()
                        == 1)
            {
                let __r: Result<MakeJoinLinkedGroupRequestGroupJoinRequestSuccess, IqResponseError> = (|| -> Result<MakeJoinLinkedGroupRequestGroupJoinRequestSuccess, IqResponseError> {


                    let r#type = response.get_attr("type")
                        .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                        .as_str()
                        .to_string();
                    iq_ensure!(r#type.as_str() == "result", IqResponseError::ValuePinnedMismatch { field: "type", expected: "result", value: format!("{:?}", r#type) });
                    Ok(MakeJoinLinkedGroupRequestGroupJoinRequestSuccess {
                        r#type,
                        ..Default::default()
                    })
                })();
                if let Ok(__v) = __r {
                    return Ok(MakeJoinLinkedGroupRequestResponse::GroupJoinRequestSuccess(
                        __v,
                    ));
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
            {
                let __r: Result<MakeJoinLinkedGroupRequestSuccess, IqResponseError> =
                    (|| -> Result<MakeJoinLinkedGroupRequestSuccess, IqResponseError> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                            .as_str()
                            .to_string();
                        iq_ensure!(
                            r#type.as_str() == "result",
                            IqResponseError::ValuePinnedMismatch {
                                field: "type",
                                expected: "result",
                                value: format!("{:?}", r#type)
                            }
                        );
                        Ok(MakeJoinLinkedGroupRequestSuccess {
                            r#type,
                            ..Default::default()
                        })
                    })();
                return Ok(MakeJoinLinkedGroupRequestResponse::Success(__r?));
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 400i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("bad-request"))
                                        || (code == 403i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("forbidden"))
                                        || (code == 404i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("item-not-found"))
                                        || (code == 405i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-allowed"))
                                        || (code == 406i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-acceptable"))
                                        || (code == 409i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("conflict"))
                                        || (code == 419i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("resource-limit"))
                                        || (code == 401i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-authorized"))
                                        || (code == 426i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("upgrade-required"))
                                        || (code == 304i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("already-exists"))
                                        || (code >= 400i64 && code <= 499i64)
                                })
                    }))
            {
                let __r: Result<MakeJoinLinkedGroupRequestClientError, IqResponseError> =
                    (|| -> Result<MakeJoinLinkedGroupRequestClientError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors, IqResponseError> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorBadRequest, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "bad-request", IqResponseError::ValuePinnedMismatch { field: "text", expected: "bad-request", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 403i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("forbidden")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorForbidden, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "forbidden", IqResponseError::ValuePinnedMismatch { field: "text", expected: "forbidden", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorForbidden {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorForbidden(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorItemNotFound, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "item-not-found", IqResponseError::ValuePinnedMismatch { field: "text", expected: "item-not-found", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorItemNotFound(value)); }
                        }
                        if (code == 405i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-allowed")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAllowed, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-allowed", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-allowed", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAllowed {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorNotAllowed(value)); }
                        }
                        if (code == 406i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-acceptable")) && error.content_bytes().is_none() && error.get_children_by_tag("field").count() <= 1 {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAcceptable, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-acceptable", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-acceptable", value: format!("{:?}", text) });
                                let (name, reason) = if let Some(field) = error.get_optional_child("field") {
                                    let name = field.get_attr("name")
                                        .ok_or(IqResponseError::MissingAttribute { name: "name" })?
                                        .as_str()
                                        .to_string();
                                    let reason = field.get_attr("reason")
                                        .ok_or(IqResponseError::MissingAttribute { name: "reason" })?
                                        .as_str()
                                        .to_string();
                                    (name, reason)
                                } else {
                                    (Default::default(), Default::default())
                                };
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAcceptable {
                                    text,
                                    name,
                                    reason,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorNotAcceptable(value)); }
                        }
                        if (code == 409i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("conflict")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorConflict, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "conflict", IqResponseError::ValuePinnedMismatch { field: "text", expected: "conflict", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorConflict {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorConflict(value)); }
                        }
                        if (code == 419i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("resource-limit")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorResourceLimit, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "resource-limit", IqResponseError::ValuePinnedMismatch { field: "text", expected: "resource-limit", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorResourceLimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorResourceLimit(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAuthorized, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-authorized", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-authorized", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 426i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("upgrade-required")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorUpgradeRequired, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "upgrade-required", IqResponseError::ValuePinnedMismatch { field: "text", expected: "upgrade-required", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorUpgradeRequired {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorUpgradeRequired(value)); }
                        }
                        if (code == 304i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("already-exists")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorAlreadyExists, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "already-exists", IqResponseError::ValuePinnedMismatch { field: "text", expected: "already-exists", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorAlreadyExists {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorAlreadyExists(value)); }
                        }
                        if (code >= 400i64 && code <= 499i64) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorFallbackClient, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorFallbackClient {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorFallbackClient(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeJoinLinkedGroupRequestClientError {
                            r#type: "error".to_string(),
                            error_join_linked_group_client_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_join_linked_group_client_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "JoinLinkedGroupResponseClientError",
                            field: "errorJoinLinkedGroupClientErrors"
                        });
                    }
                    Ok(value)
                });
                if let Ok(__v) = __r {
                    return Ok(MakeJoinLinkedGroupRequestResponse::ClientError(__v));
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 500i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("internal-server-error"))
                                        || (code == 503i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("service-unavailable"))
                                        || (code == 530i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("partial-server-error"))
                                        || (code >= 500i64 && code <= 599i64)
                                })
                    }))
            {
                let __r: Result<MakeJoinLinkedGroupRequestServerError, IqResponseError> =
                    (|| -> Result<MakeJoinLinkedGroupRequestServerError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrors, IqResponseError> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "internal-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "internal-server-error", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "service-unavailable", IqResponseError::ValuePinnedMismatch { field: "text", expected: "service-unavailable", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "partial-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "partial-server-error", value: format!("{:?}", text) });
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeJoinLinkedGroupRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_server_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "JoinLinkedGroupResponseServerError",
                            field: "errorServerErrors"
                        });
                    }
                    Ok(value)
                });
                return Ok(MakeJoinLinkedGroupRequestResponse::ServerError(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakeJoinLinkedGroupRequestResponse"
            })
        }
    }

    /// makeSetSubjectRequest:set IQ spec.
    ///
    /// Source: `WASmaxGroupsSetSubjectRPC`
    #[derive(Debug, Clone)]
    pub struct MakeSetSubjectRequestSpec {
        pub target: Jid,
        pub subject_content: Vec<u8>,
    }

    impl MakeSetSubjectRequestSpec {
        pub fn new(target: Jid, subject_content: Vec<u8>) -> Self {
            Self {
                target,
                subject_content,
            }
        }
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestSuccess {
        pub r#type: String,
    }

    /// Discriminated union for response field `errorSetSubjectClientErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors {
        IQErrorBadRequest(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorBadRequest,
        ),
        IQErrorItemNotFound(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorItemNotFound,
        ),
        IQErrorNotAuthorized(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAuthorized,
        ),
        IQErrorForbidden(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorForbidden,
        ),
        IQErrorNotAcceptable(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAcceptable,
        ),
        IQErrorLocked(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorLocked),
        IQErrorRateOverlimit(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorRateOverlimit,
        ),
        IQErrorFallbackClient(
            MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorFallbackClient,
        ),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorBadRequest {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorItemNotFound {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAuthorized {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorForbidden {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAcceptable {
        pub text: String,
        pub code: u64,
        pub name: String,
        pub reason: String,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorLocked {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorRateOverlimit {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorFallbackClient {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestClientError {
        pub r#type: String,
        pub error_set_subject_client_errors:
            Option<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors>,
    }

    /// Discriminated union for response field `errorServerErrors`.
    #[derive(Debug, Clone)]
    pub enum MakeSetSubjectRequestServerErrorErrorServerErrors {
        IQErrorInternalServerError(
            MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorInternalServerError,
        ),
        IQErrorServiceUnavailable(
            MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable,
        ),
        IQErrorPartialServerError(
            MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorPartialServerError,
        ),
        IQErrorFallbackServer(
            MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorFallbackServer,
        ),
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
        pub text: String,
        pub code: u64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestServerError {
        pub r#type: String,
        pub error_server_errors: Option<MakeSetSubjectRequestServerErrorErrorServerErrors>,
    }

    /// makeSetSubjectRequest:set — RPC outcome union (3 variants).
    #[derive(Debug, Clone)]
    pub enum MakeSetSubjectRequestResponse {
        Success(MakeSetSubjectRequestSuccess),
        ClientError(MakeSetSubjectRequestClientError),
        ServerError(MakeSetSubjectRequestServerError),
    }

    impl IqSpec for MakeSetSubjectRequestSpec {
        type Response = MakeSetSubjectRequestResponse;
        type Error = IqResponseError;

        fn build_iq(&self) -> InfoQuery<'static> {
            // `target` is the group's own JID (`<group>@g.us`) — not the bare `g.us` server.
            let mut subject_node = NodeBuilder::new("subject");
            subject_node = subject_node.bytes(self.subject_content.clone());
            let subject_node = subject_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                self.target.clone(),
                Some(NodeContent::Nodes(vec![subject_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Err(IqResponseError::RequestContextRequired {
                module: "WASmaxGroupsSetSubjectRPC",
                guard: "request_context_required",
            })
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn handles_iq_errors(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, Self::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeSetSubjectRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeSetSubjectRequestResponse, IqResponseError> {
            iq_ensure!(
                request.tag.as_ref() == "iq",
                IqResponseError::RequestContextNotIq
            );
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| IqResponseError::RequestContextMissingId)?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| IqResponseError::RequestContextMissingTo)?;
            Self::parse_correlated_response(
                response,
                request_id.as_str().as_ref(),
                request_to.as_str().as_ref(),
            )
        }
        pub fn parse_response_with_request(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeSetSubjectRequestResponse, IqResponseError> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeSetSubjectRequestResponse, IqResponseError> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsSetSubjectRPC",
                    field: "from",
                    expected: request_to.to_string()
                });
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                iq_bail!(IqResponseError::CorrelationMismatch {
                    module: "WASmaxGroupsSetSubjectRPC",
                    field: "id",
                    expected: request_id.to_string()
                });
            }
            fn __iq_error_code(value: &str) -> Option<i64> {
                let value = value.trim_start_matches(|c| matches!(c, '\u{0009}'..='\u{000d}' | ' ' | '\u{00a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}'));
                let offset =
                    usize::from(matches!(value.as_bytes().first(), Some(b'+') | Some(b'-')));
                let end = offset
                    + value
                        .bytes()
                        .skip(offset)
                        .take_while(u8::is_ascii_digit)
                        .count();
                if end == offset {
                    None
                } else {
                    value[..end].parse().ok()
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
            {
                let __r: Result<MakeSetSubjectRequestSuccess, IqResponseError> =
                    (|| -> Result<MakeSetSubjectRequestSuccess, IqResponseError> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or(IqResponseError::MissingAttribute { name: "type" })?
                            .as_str()
                            .to_string();
                        iq_ensure!(
                            r#type.as_str() == "result",
                            IqResponseError::ValuePinnedMismatch {
                                field: "type",
                                expected: "result",
                                value: format!("{:?}", r#type)
                            }
                        );
                        Ok(MakeSetSubjectRequestSuccess {
                            r#type,
                            ..Default::default()
                        })
                    })();
                return Ok(MakeSetSubjectRequestResponse::Success(__r?));
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 400i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("bad-request"))
                                        || (code == 404i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("item-not-found"))
                                        || (code == 401i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-authorized"))
                                        || (code == 403i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("forbidden"))
                                        || (code == 406i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("not-acceptable"))
                                        || (code == 423i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("locked"))
                                        || (code == 429i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("rate-overlimit"))
                                        || (code >= 400i64 && code <= 499i64)
                                })
                    }))
            {
                let __r: Result<MakeSetSubjectRequestClientError, IqResponseError> =
                    (|| -> Result<MakeSetSubjectRequestClientError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors, IqResponseError> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorBadRequest, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "bad-request", IqResponseError::ValuePinnedMismatch { field: "text", expected: "bad-request", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorItemNotFound, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "item-not-found", IqResponseError::ValuePinnedMismatch { field: "text", expected: "item-not-found", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorItemNotFound(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAuthorized, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-authorized", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-authorized", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 403i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("forbidden")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorForbidden, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "forbidden", IqResponseError::ValuePinnedMismatch { field: "text", expected: "forbidden", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorForbidden {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorForbidden(value)); }
                        }
                        if (code == 406i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-acceptable")) && error.content_bytes().is_none() && error.get_children_by_tag("field").count() <= 1 {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAcceptable, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "not-acceptable", IqResponseError::ValuePinnedMismatch { field: "text", expected: "not-acceptable", value: format!("{:?}", text) });
                                let (name, reason) = if let Some(field) = error.get_optional_child("field") {
                                    let name = field.get_attr("name")
                                        .ok_or(IqResponseError::MissingAttribute { name: "name" })?
                                        .as_str()
                                        .to_string();
                                    let reason = field.get_attr("reason")
                                        .ok_or(IqResponseError::MissingAttribute { name: "reason" })?
                                        .as_str()
                                        .to_string();
                                    (name, reason)
                                } else {
                                    (Default::default(), Default::default())
                                };
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAcceptable {
                                    text,
                                    name,
                                    reason,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorNotAcceptable(value)); }
                        }
                        if (code == 423i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("locked")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorLocked, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "locked", IqResponseError::ValuePinnedMismatch { field: "text", expected: "locked", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorLocked {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorLocked(value)); }
                        }
                        if (code == 429i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("rate-overlimit")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorRateOverlimit, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "rate-overlimit", IqResponseError::ValuePinnedMismatch { field: "text", expected: "rate-overlimit", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorRateOverlimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorRateOverlimit(value)); }
                        }
                        if (code >= 400i64 && code <= 499i64) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorFallbackClient, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorFallbackClient {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorFallbackClient(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeSetSubjectRequestClientError {
                            r#type: "error".to_string(),
                            error_set_subject_client_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_set_subject_client_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "SetSubjectResponseClientError",
                            field: "errorSetSubjectClientErrors"
                        });
                    }
                    Ok(value)
                });
                if let Ok(__v) = __r {
                    return Ok(MakeSetSubjectRequestResponse::ClientError(__v));
                }
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("error")
                && (response.get_attr("type").map(|v| v.as_str()).as_deref() == Some("error")
                    && response.content_bytes().is_none()
                    && response.get_children_by_tag("error").count() == 1
                    && response.get_optional_child("error").is_some_and(|error| {
                        error.get_attr("text").is_some()
                            && error
                                .get_attr("code")
                                .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                                .is_some_and(|code| {
                                    (code == 500i64
                                        && error.get_attr("text").map(|v| v.as_str()).as_deref()
                                            == Some("internal-server-error"))
                                        || (code == 503i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("service-unavailable"))
                                        || (code == 530i64
                                            && error
                                                .get_attr("text")
                                                .map(|v| v.as_str())
                                                .as_deref()
                                                == Some("partial-server-error"))
                                        || (code >= 500i64 && code <= 599i64)
                                })
                    }))
            {
                let __r: Result<MakeSetSubjectRequestServerError, IqResponseError> =
                    (|| -> Result<MakeSetSubjectRequestServerError, IqResponseError> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| IqResponseError::MissingChild { tag: "error" })?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| IqResponseError::InvalidErrorCode)?;
                        let payload = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrors, IqResponseError> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorInternalServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "internal-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "internal-server-error", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "service-unavailable", IqResponseError::ValuePinnedMismatch { field: "text", expected: "service-unavailable", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorPartialServerError, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                iq_ensure!(text.as_str() == "partial-server-error", IqResponseError::ValuePinnedMismatch { field: "text", expected: "partial-server-error", value: format!("{:?}", text) });
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorFallbackServer, IqResponseError> {
                                let text = error.get_attr("text")
                                    .ok_or(IqResponseError::MissingAttribute { name: "text" })?
                                    .as_str()
                                    .to_string();
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        iq_bail!(IqResponseError::NoErrorPayloadMatched)
                    })()?;
                        Ok(MakeSetSubjectRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| {
                    if value.error_server_errors.is_none() {
                        iq_bail!(IqResponseError::RequiredPayloadMismatch {
                            variant: "SetSubjectResponseServerError",
                            field: "errorServerErrors"
                        });
                    }
                    Ok(value)
                });
                return Ok(MakeSetSubjectRequestResponse::ServerError(__r?));
            }
            iq_bail!(IqResponseError::NoResponseVariantMatched {
                union_name: "MakeSetSubjectRequestResponse"
            })
        }
    }
}
