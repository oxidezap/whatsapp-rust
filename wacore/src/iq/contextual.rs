//! Auto-generated IQ stanza specs (WhatsApp 2.3000.1052559453). DO NOT EDIT.
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

/// IQ namespace `md`. Source: WASmaxOutMdGetCountryCodeRequest.
pub mod md {
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
    /// Source: `WASmaxOutMdGetCountryCodeRequest`
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxOutMdGetCountryCodeRequest: guards.request_context_required: use parse_response_with_request"
            )
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
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeGetCountryCodeRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeGetCountryCodeRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakeGetCountryCodeRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeGetCountryCodeRequestResponse, anyhow::Error> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                anyhow::bail!(
                    "WASmaxOutMdGetCountryCodeRequest: response.from does not match request_to"
                );
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!(
                    "WASmaxOutMdGetCountryCodeRequest: response.id does not match request_id"
                );
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
                let __r: Result<MakeGetCountryCodeRequestGetCountryCodeResponse, anyhow::Error> = (|| -> Result<MakeGetCountryCodeRequestGetCountryCodeResponse, anyhow::Error> {


                    let country_code_wrap = response.get_optional_child("country_code")
                        .ok_or_else(|| anyhow::anyhow!("missing <country_code>"))?;
                    let country_code_iso = country_code_wrap.get_attr("iso")
                        .ok_or_else(|| anyhow::anyhow!("missing iso"))?
                        .as_str()
                        .to_string();
                    let r#type = response.get_attr("type")
                        .ok_or_else(|| anyhow::anyhow!("missing type"))?
                        .as_str()
                        .to_string();
                    anyhow::ensure!(r#type.as_str() == "result", "type is pinned to result: {:?}", r#type);
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
                let __r: Result<MakeGetCountryCodeRequestError, anyhow::Error> =
                    (|| -> Result<MakeGetCountryCodeRequestError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeGetCountryCodeRequestErrorErrorCountryCodeErrors, anyhow::Error> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorBadRequest, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "bad-request", "text is pinned to bad-request: {:?}", text);
                                Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorInternalServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "internal-server-error", "text is pinned to internal-server-error: {:?}", text);
                                Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeGetCountryCodeRequestErrorErrorCountryCodeErrors::IQErrorInternalServerError(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeGetCountryCodeRequestError {
                            r#type: "error".to_string(),
                            error_country_code_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_country_code_errors.is_none() { anyhow::bail!("GetCountryCodeResponseError: required payload errorCountryCodeErrors did not match"); } Ok(value) });
                return Ok(MakeGetCountryCodeRequestResponse::Error(__r?));
            }
            anyhow::bail!("MakeGetCountryCodeRequestResponse: no response variant matched")
        }
    }
}

/// IQ namespace `passive`. Source: WASmaxPassiveModeActiveIQRPC, WASmaxPassiveModePassiveIQRPC.
pub mod passive {
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxPassiveModeActiveIQRPC: guards.request_context_required: use parse_response_with_request"
            )
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeActiveIQRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeActiveIQRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakeActiveIQRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeActiveIQRequestResponse, anyhow::Error> {
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!(
                    "WASmaxPassiveModeActiveIQRPC: response.id does not match request_id"
                );
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
                && response.get_attr("from").map(|x| x.as_str()).as_deref()
                    == Some("s.whatsapp.net")
                && (response.content_bytes().is_none()
                    && response.get_children_by_tag("active").count() == 1)
            {
                let __r: Result<MakeActiveIQRequestActiveIQResponseSuccess, anyhow::Error> =
                    (|| -> Result<MakeActiveIQRequestActiveIQResponseSuccess, anyhow::Error> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or_else(|| anyhow::anyhow!("missing type"))?
                            .as_str()
                            .to_string();
                        anyhow::ensure!(
                            r#type.as_str() == "result",
                            "type is pinned to result: {:?}",
                            r#type
                        );
                        let from = response
                            .get_attr("from")
                            .and_then(|v| v.to_jid())
                            .ok_or_else(|| anyhow::anyhow!("missing from"))?;
                        anyhow::ensure!(
                            response.get_attr("from").map(|v| v.as_str()).as_deref()
                                == Some("s.whatsapp.net"),
                            "from not accepted: {:?}",
                            response.get_attr("from").map(|v| v.as_str()).as_deref()
                        );
                        Ok(MakeActiveIQRequestActiveIQResponseSuccess {
                            r#type,
                            from,
                            ..Default::default()
                        })
                    })();
                return Ok(MakeActiveIQRequestResponse::ActiveIQResponseSuccess(__r?));
            }
            anyhow::bail!("MakeActiveIQRequestResponse: no response variant matched")
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxPassiveModePassiveIQRPC: guards.request_context_required: use parse_response_with_request"
            )
        }
        fn uses_contextual_response(&self) -> bool {
            true
        }
        fn parse_response_with_context(
            &self,
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakePassiveIQRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakePassiveIQRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakePassiveIQRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakePassiveIQRequestResponse, anyhow::Error> {
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!(
                    "WASmaxPassiveModePassiveIQRPC: response.id does not match request_id"
                );
            }
            if response.tag.as_ref() == "iq"
                && response.get_attr("type").map(|x| x.as_str()).as_deref() == Some("result")
                && response.get_attr("from").map(|x| x.as_str()).as_deref()
                    == Some("s.whatsapp.net")
                && (response.content_bytes().is_none()
                    && response.get_children_by_tag("passive").count() == 1)
            {
                let __r: Result<MakePassiveIQRequestPassiveIQResponseSuccess, anyhow::Error> =
                    (|| -> Result<MakePassiveIQRequestPassiveIQResponseSuccess, anyhow::Error> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or_else(|| anyhow::anyhow!("missing type"))?
                            .as_str()
                            .to_string();
                        anyhow::ensure!(
                            r#type.as_str() == "result",
                            "type is pinned to result: {:?}",
                            r#type
                        );
                        let from = response
                            .get_attr("from")
                            .and_then(|v| v.to_jid())
                            .ok_or_else(|| anyhow::anyhow!("missing from"))?;
                        anyhow::ensure!(
                            response.get_attr("from").map(|v| v.as_str()).as_deref()
                                == Some("s.whatsapp.net"),
                            "from not accepted: {:?}",
                            response.get_attr("from").map(|v| v.as_str()).as_deref()
                        );
                        Ok(MakePassiveIQRequestPassiveIQResponseSuccess {
                            r#type,
                            from,
                            ..Default::default()
                        })
                    })();
                return Ok(MakePassiveIQRequestResponse::PassiveIQResponseSuccess(__r?));
            }
            anyhow::bail!("MakePassiveIQRequestResponse: no response variant matched")
        }
    }
}

/// IQ namespace `w:g2`. Source: WASmaxGroupsAcceptGroupAddRPC, WASmaxGroupsDeleteParentGroupRPC, WASmaxGroupsJoinLinkedGroupRPC, WASmaxGroupsSetSubjectRPC.
pub mod w_g2 {
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxGroupsAcceptGroupAddRPC: guards.request_context_required: use parse_response_with_request"
            )
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
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeAcceptGroupAddRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeAcceptGroupAddRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakeAcceptGroupAddRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeAcceptGroupAddRequestResponse, anyhow::Error> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                anyhow::bail!(
                    "WASmaxGroupsAcceptGroupAddRPC: response.from does not match request_to"
                );
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!(
                    "WASmaxGroupsAcceptGroupAddRPC: response.id does not match request_id"
                );
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
                let __r: Result<MakeAcceptGroupAddRequestGroupJoinRequestSuccess, anyhow::Error> = (|| -> Result<MakeAcceptGroupAddRequestGroupJoinRequestSuccess, anyhow::Error> {


                    let r#type = response.get_attr("type")
                        .ok_or_else(|| anyhow::anyhow!("missing type"))?
                        .as_str()
                        .to_string();
                    anyhow::ensure!(r#type.as_str() == "result", "type is pinned to result: {:?}", r#type);
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
                let __r: Result<MakeAcceptGroupAddRequestSuccess, anyhow::Error> =
                    (|| -> Result<MakeAcceptGroupAddRequestSuccess, anyhow::Error> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or_else(|| anyhow::anyhow!("missing type"))?
                            .as_str()
                            .to_string();
                        anyhow::ensure!(
                            r#type.as_str() == "result",
                            "type is pinned to result: {:?}",
                            r#type
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
                let __r: Result<MakeAcceptGroupAddRequestClientError, anyhow::Error> =
                    (|| -> Result<MakeAcceptGroupAddRequestClientError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors, anyhow::Error> {
                        if (code == 304i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("already-exists")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorAlreadyExists, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "already-exists", "text is pinned to already-exists: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorAlreadyExists {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorAlreadyExists(value)); }
                        }
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorBadRequest, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "bad-request", "text is pinned to bad-request: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAuthorized, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-authorized", "text is pinned to not-authorized: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 403i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("forbidden")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorForbidden, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "forbidden", "text is pinned to forbidden: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorForbidden {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorForbidden(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorItemNotFound, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "item-not-found", "text is pinned to item-not-found: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorItemNotFound(value)); }
                        }
                        if (code == 409i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("conflict")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorConflict, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "conflict", "text is pinned to conflict: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorConflict {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorConflict(value)); }
                        }
                        if (code == 410i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("gone")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorGone, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "gone", "text is pinned to gone: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorGone {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorGone(value)); }
                        }
                        if (code == 419i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("resource-limit")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceLimit, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "resource-limit", "text is pinned to resource-limit: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceLimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorResourceLimit(value)); }
                        }
                        if (code == 423i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("locked")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorLocked, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "locked", "text is pinned to locked: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorLocked {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorLocked(value)); }
                        }
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("resource-constraint")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceConstraint, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "resource-constraint", "text is pinned to resource-constraint: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorResourceConstraint {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorResourceConstraint(value)); }
                        }
                        if (code == 405i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-allowed")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAllowed, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-allowed", "text is pinned to not-allowed: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorNotAllowed {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorNotAllowed(value)); }
                        }
                        if (code == 412i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("linked-groups-participants-resource-limit")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorParentLinkedGroupsParticipantsResourceLimit, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "linked-groups-participants-resource-limit", "text is pinned to linked-groups-participants-resource-limit: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorParentLinkedGroupsParticipantsResourceLimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorParentLinkedGroupsParticipantsResourceLimit(value)); }
                        }
                        if (code >= 400i64 && code <= 499i64) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorFallbackClient, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrorsIQErrorFallbackClient {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestClientErrorErrorAcceptGroupAddClientErrors::IQErrorFallbackClient(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeAcceptGroupAddRequestClientError {
                            r#type: "error".to_string(),
                            error_accept_group_add_client_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_accept_group_add_client_errors.is_none() { anyhow::bail!("AcceptGroupAddResponseClientError: required payload errorAcceptGroupAddClientErrors did not match"); } Ok(value) });
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
                let __r: Result<MakeAcceptGroupAddRequestServerError, anyhow::Error> =
                    (|| -> Result<MakeAcceptGroupAddRequestServerError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrors, anyhow::Error> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorInternalServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "internal-server-error", "text is pinned to internal-server-error: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "service-unavailable", "text is pinned to service-unavailable: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorPartialServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "partial-server-error", "text is pinned to partial-server-error: {:?}", text);
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorFallbackServer, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeAcceptGroupAddRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeAcceptGroupAddRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_server_errors.is_none() { anyhow::bail!("AcceptGroupAddResponseServerError: required payload errorServerErrors did not match"); } Ok(value) });
                return Ok(MakeAcceptGroupAddRequestResponse::ServerError(__r?));
            }
            anyhow::bail!("MakeAcceptGroupAddRequestResponse: no response variant matched")
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxGroupsDeleteParentGroupRPC: guards.request_context_required: use parse_response_with_request"
            )
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
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeDeleteParentGroupRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeDeleteParentGroupRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakeDeleteParentGroupRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeDeleteParentGroupRequestResponse, anyhow::Error> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                anyhow::bail!(
                    "WASmaxGroupsDeleteParentGroupRPC: response.from does not match request_to"
                );
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!(
                    "WASmaxGroupsDeleteParentGroupRPC: response.id does not match request_id"
                );
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
                let __r: Result<MakeDeleteParentGroupRequestSuccess, anyhow::Error> =
                    (|| -> Result<MakeDeleteParentGroupRequestSuccess, anyhow::Error> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or_else(|| anyhow::anyhow!("missing type"))?
                            .as_str()
                            .to_string();
                        anyhow::ensure!(
                            r#type.as_str() == "result",
                            "type is pinned to result: {:?}",
                            r#type
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
                let __r: Result<MakeDeleteParentGroupRequestClientError, anyhow::Error> =
                    (|| -> Result<MakeDeleteParentGroupRequestClientError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError, anyhow::Error> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorBadRequest, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "bad-request", "text is pinned to bad-request: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorBadRequest(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorNotAuthorized, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-authorized", "text is pinned to not-authorized: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorItemNotFound, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "item-not-found", "text is pinned to item-not-found: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorItemNotFound(value)); }
                        }
                        if (code == 429i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("rate-overlimit")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorRateOverlimit, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "rate-overlimit", "text is pinned to rate-overlimit: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientErrorIQErrorRateOverlimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestClientErrorErrorDeleteParentGroupClientError::IQErrorRateOverlimit(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeDeleteParentGroupRequestClientError {
                            r#type: "error".to_string(),
                            error_delete_parent_group_client_error: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_delete_parent_group_client_error.is_none() { anyhow::bail!("DeleteParentGroupResponseClientError: required payload errorDeleteParentGroupClientError did not match"); } Ok(value) });
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
                let __r: Result<MakeDeleteParentGroupRequestServerError, anyhow::Error> =
                    (|| -> Result<MakeDeleteParentGroupRequestServerError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrors, anyhow::Error> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "internal-server-error", "text is pinned to internal-server-error: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "service-unavailable", "text is pinned to service-unavailable: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "partial-server-error", "text is pinned to partial-server-error: {:?}", text);
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeDeleteParentGroupRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeDeleteParentGroupRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_server_errors.is_none() { anyhow::bail!("DeleteParentGroupResponseServerError: required payload errorServerErrors did not match"); } Ok(value) });
                return Ok(MakeDeleteParentGroupRequestResponse::ServerError(__r?));
            }
            anyhow::bail!("MakeDeleteParentGroupRequestResponse: no response variant matched")
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxGroupsJoinLinkedGroupRPC: guards.request_context_required: use parse_response_with_request"
            )
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
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeJoinLinkedGroupRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeJoinLinkedGroupRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakeJoinLinkedGroupRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeJoinLinkedGroupRequestResponse, anyhow::Error> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                anyhow::bail!(
                    "WASmaxGroupsJoinLinkedGroupRPC: response.from does not match request_to"
                );
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!(
                    "WASmaxGroupsJoinLinkedGroupRPC: response.id does not match request_id"
                );
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
                let __r: Result<MakeJoinLinkedGroupRequestGroupJoinRequestSuccess, anyhow::Error> = (|| -> Result<MakeJoinLinkedGroupRequestGroupJoinRequestSuccess, anyhow::Error> {


                    let r#type = response.get_attr("type")
                        .ok_or_else(|| anyhow::anyhow!("missing type"))?
                        .as_str()
                        .to_string();
                    anyhow::ensure!(r#type.as_str() == "result", "type is pinned to result: {:?}", r#type);
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
                let __r: Result<MakeJoinLinkedGroupRequestSuccess, anyhow::Error> =
                    (|| -> Result<MakeJoinLinkedGroupRequestSuccess, anyhow::Error> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or_else(|| anyhow::anyhow!("missing type"))?
                            .as_str()
                            .to_string();
                        anyhow::ensure!(
                            r#type.as_str() == "result",
                            "type is pinned to result: {:?}",
                            r#type
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
                let __r: Result<MakeJoinLinkedGroupRequestClientError, anyhow::Error> =
                    (|| -> Result<MakeJoinLinkedGroupRequestClientError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors, anyhow::Error> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorBadRequest, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "bad-request", "text is pinned to bad-request: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 403i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("forbidden")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorForbidden, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "forbidden", "text is pinned to forbidden: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorForbidden {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorForbidden(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorItemNotFound, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "item-not-found", "text is pinned to item-not-found: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorItemNotFound(value)); }
                        }
                        if (code == 405i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-allowed")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAllowed, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-allowed", "text is pinned to not-allowed: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAllowed {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorNotAllowed(value)); }
                        }
                        if (code == 406i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-acceptable")) && error.content_bytes().is_none() && error.get_children_by_tag("field").count() <= 1 {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAcceptable, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-acceptable", "text is pinned to not-acceptable: {:?}", text);
                                let (name, reason) = if let Some(field) = error.get_optional_child("field") {
                                    let name = field.get_attr("name")
                                        .ok_or_else(|| anyhow::anyhow!("missing name"))?
                                        .as_str()
                                        .to_string();
                                    let reason = field.get_attr("reason")
                                        .ok_or_else(|| anyhow::anyhow!("missing reason"))?
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
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorConflict, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "conflict", "text is pinned to conflict: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorConflict {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorConflict(value)); }
                        }
                        if (code == 419i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("resource-limit")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorResourceLimit, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "resource-limit", "text is pinned to resource-limit: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorResourceLimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorResourceLimit(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAuthorized, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-authorized", "text is pinned to not-authorized: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 426i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("upgrade-required")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorUpgradeRequired, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "upgrade-required", "text is pinned to upgrade-required: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorUpgradeRequired {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorUpgradeRequired(value)); }
                        }
                        if (code == 304i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("already-exists")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorAlreadyExists, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "already-exists", "text is pinned to already-exists: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorAlreadyExists {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorAlreadyExists(value)); }
                        }
                        if (code >= 400i64 && code <= 499i64) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorFallbackClient, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrorsIQErrorFallbackClient {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestClientErrorErrorJoinLinkedGroupClientErrors::IQErrorFallbackClient(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeJoinLinkedGroupRequestClientError {
                            r#type: "error".to_string(),
                            error_join_linked_group_client_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_join_linked_group_client_errors.is_none() { anyhow::bail!("JoinLinkedGroupResponseClientError: required payload errorJoinLinkedGroupClientErrors did not match"); } Ok(value) });
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
                let __r: Result<MakeJoinLinkedGroupRequestServerError, anyhow::Error> =
                    (|| -> Result<MakeJoinLinkedGroupRequestServerError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrors, anyhow::Error> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "internal-server-error", "text is pinned to internal-server-error: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "service-unavailable", "text is pinned to service-unavailable: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "partial-server-error", "text is pinned to partial-server-error: {:?}", text);
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeJoinLinkedGroupRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeJoinLinkedGroupRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_server_errors.is_none() { anyhow::bail!("JoinLinkedGroupResponseServerError: required payload errorServerErrors did not match"); } Ok(value) });
                return Ok(MakeJoinLinkedGroupRequestResponse::ServerError(__r?));
            }
            anyhow::bail!("MakeJoinLinkedGroupRequestResponse: no response variant matched")
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
        ) -> Result<Self::Response, anyhow::Error> {
            anyhow::bail!(
                "WASmaxGroupsSetSubjectRPC: guards.request_context_required: use parse_response_with_request"
            )
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
        ) -> Result<Self::Response, anyhow::Error> {
            Self::parse_sent_response(response, request)
        }
    }
    impl MakeSetSubjectRequestSpec {
        pub fn parse_sent_response(
            response: &wacore_binary::NodeRef<'_>,
            request: &wacore_binary::NodeRef<'_>,
        ) -> Result<MakeSetSubjectRequestResponse, anyhow::Error> {
            anyhow::ensure!(request.tag.as_ref() == "iq", "request context is not an IQ");
            let request_id = request
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("request context has no id"))?;
            let request_to = request
                .get_attr("to")
                .ok_or_else(|| anyhow::anyhow!("request context has no to"))?;
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
        ) -> Result<MakeSetSubjectRequestResponse, anyhow::Error> {
            Self::parse_correlated_response(response, request_id, request_to)
        }
        fn parse_correlated_response(
            response: &wacore_binary::NodeRef<'_>,
            request_id: &str,
            request_to: &str,
        ) -> Result<MakeSetSubjectRequestResponse, anyhow::Error> {
            if response.get_attr("from").map(|v| v.as_str()).as_deref() != Some(request_to) {
                anyhow::bail!("WASmaxGroupsSetSubjectRPC: response.from does not match request_to");
            }
            if response.get_attr("id").map(|v| v.as_str()).as_deref() != Some(request_id) {
                anyhow::bail!("WASmaxGroupsSetSubjectRPC: response.id does not match request_id");
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
                let __r: Result<MakeSetSubjectRequestSuccess, anyhow::Error> =
                    (|| -> Result<MakeSetSubjectRequestSuccess, anyhow::Error> {
                        let r#type = response
                            .get_attr("type")
                            .ok_or_else(|| anyhow::anyhow!("missing type"))?
                            .as_str()
                            .to_string();
                        anyhow::ensure!(
                            r#type.as_str() == "result",
                            "type is pinned to result: {:?}",
                            r#type
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
                let __r: Result<MakeSetSubjectRequestClientError, anyhow::Error> =
                    (|| -> Result<MakeSetSubjectRequestClientError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors, anyhow::Error> {
                        if (code == 400i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("bad-request")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorBadRequest, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "bad-request", "text is pinned to bad-request: {:?}", text);
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorBadRequest {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorBadRequest(value)); }
                        }
                        if (code == 404i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("item-not-found")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorItemNotFound, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "item-not-found", "text is pinned to item-not-found: {:?}", text);
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorItemNotFound {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorItemNotFound(value)); }
                        }
                        if (code == 401i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-authorized")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAuthorized, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-authorized", "text is pinned to not-authorized: {:?}", text);
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAuthorized {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorNotAuthorized(value)); }
                        }
                        if (code == 403i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("forbidden")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorForbidden, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "forbidden", "text is pinned to forbidden: {:?}", text);
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorForbidden {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorForbidden(value)); }
                        }
                        if (code == 406i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("not-acceptable")) && error.content_bytes().is_none() && error.get_children_by_tag("field").count() <= 1 {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorNotAcceptable, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "not-acceptable", "text is pinned to not-acceptable: {:?}", text);
                                let (name, reason) = if let Some(field) = error.get_optional_child("field") {
                                    let name = field.get_attr("name")
                                        .ok_or_else(|| anyhow::anyhow!("missing name"))?
                                        .as_str()
                                        .to_string();
                                    let reason = field.get_attr("reason")
                                        .ok_or_else(|| anyhow::anyhow!("missing reason"))?
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
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorLocked, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "locked", "text is pinned to locked: {:?}", text);
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorLocked {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorLocked(value)); }
                        }
                        if (code == 429i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("rate-overlimit")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorRateOverlimit, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "rate-overlimit", "text is pinned to rate-overlimit: {:?}", text);
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorRateOverlimit {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorRateOverlimit(value)); }
                        }
                        if (code >= 400i64 && code <= 499i64) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorFallbackClient, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrorsIQErrorFallbackClient {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestClientErrorErrorSetSubjectClientErrors::IQErrorFallbackClient(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeSetSubjectRequestClientError {
                            r#type: "error".to_string(),
                            error_set_subject_client_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_set_subject_client_errors.is_none() { anyhow::bail!("SetSubjectResponseClientError: required payload errorSetSubjectClientErrors did not match"); } Ok(value) });
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
                let __r: Result<MakeSetSubjectRequestServerError, anyhow::Error> =
                    (|| -> Result<MakeSetSubjectRequestServerError, anyhow::Error> {
                        let error = response
                            .get_optional_child("error")
                            .ok_or_else(|| anyhow::anyhow!("missing error"))?;
                        let code = error
                            .get_attr("code")
                            .and_then(|v| __iq_error_code(v.as_str().as_ref()))
                            .ok_or_else(|| anyhow::anyhow!("invalid error code"))?;
                        let payload = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrors, anyhow::Error> {
                        if (code == 500i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("internal-server-error")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorInternalServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "internal-server-error", "text is pinned to internal-server-error: {:?}", text);
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorInternalServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorInternalServerError(value)); }
                        }
                        if (code == 503i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("service-unavailable")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "service-unavailable", "text is pinned to service-unavailable: {:?}", text);
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorServiceUnavailable {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorServiceUnavailable(value)); }
                        }
                        if (code == 530i64 && error.get_attr("text").map(|v| v.as_str()).as_deref() == Some("partial-server-error")) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorPartialServerError, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                anyhow::ensure!(text.as_str() == "partial-server-error", "text is pinned to partial-server-error: {:?}", text);
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorPartialServerError {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorPartialServerError(value)); }
                        }
                        if (code >= 500i64 && code <= 599i64) {
                            let parsed = (|| -> Result<MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorFallbackServer, anyhow::Error> {
                                let text = error.get_attr("text")
                                    .ok_or_else(|| anyhow::anyhow!("missing text"))?
                                    .as_str()
                                    .to_string();
                                Ok(MakeSetSubjectRequestServerErrorErrorServerErrorsIQErrorFallbackServer {
                                    text,
                                    ..Default::default()
                                })
                            })();
                            if let Ok(mut value) = parsed { value.code = code as u64; return Ok(MakeSetSubjectRequestServerErrorErrorServerErrors::IQErrorFallbackServer(value)); }
                        }
                        anyhow::bail!("no error payload matched")
                    })()?;
                        Ok(MakeSetSubjectRequestServerError {
                            r#type: "error".to_string(),
                            error_server_errors: Some(payload),
                        })
                    })();
                let __r = __r.and_then(|value| { if value.error_server_errors.is_none() { anyhow::bail!("SetSubjectResponseServerError: required payload errorServerErrors did not match"); } Ok(value) });
                return Ok(MakeSetSubjectRequestResponse::ServerError(__r?));
            }
            anyhow::bail!("MakeSetSubjectRequestResponse: no response variant matched")
        }
    }
}
