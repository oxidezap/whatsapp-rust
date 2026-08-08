//! MEX (Meta Exchange) GraphQL feature.
//!
//! Protocol types are defined in `wacore::iq::mex`.

use crate::client::Client;
use crate::request::IqError;
use serde::Serialize;
use thiserror::Error;
use wacore::WireEnum;
use wacore::iq::mex::MexQuerySpec;
use wacore::iq::mex_operations::{fetch_new_chat_message_capping_info, fetch_reachout_timelock};
use wacore_binary::jid::JidError;

// Re-export types from wacore
pub use wacore::iq::mex::{MexDoc, MexErrorExtensions, MexGraphQLError, MexResponse};
pub use wacore::iq::mex_operations::fetch_reachout_timelock::Xwa2FetchAccountReachoutTimelock as ReachoutTimelock;

/// Error types for MEX operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MexError {
    /// Payload missing or otherwise malformed in a way that has no underlying
    /// typed source (descriptive message only — e.g. "missing data").
    #[error("MEX payload parsing error: {0}")]
    PayloadParsing(String),

    #[error("MEX payload contained an invalid JID")]
    InvalidJid(#[from] JidError),

    #[error("MEX extension error: code={code}, message='{message}'")]
    ExtensionError { code: i32, message: String },

    #[error("IQ request failed")]
    Request(#[from] IqError),

    #[error("JSON error")]
    Json(#[from] serde_json::Error),
}

/// MEX request: a persisted-query descriptor plus its typed variables.
///
/// Variables are serialized straight to the wire in the IQ spec (no intermediate
/// `serde_json::Value`). Build one with the `mex_request!` macro, which pulls
/// `NAME`/`DOC_ID` from a generated [`wacore::iq::mex_operations`] module so the
/// op is named once.
#[derive(Debug, Clone)]
pub struct MexRequest<V> {
    /// GraphQL persisted-query descriptor (name + id).
    pub doc: MexDoc,
    /// Typed query variables: a generated `Variables`, or any `Serialize` value
    /// (e.g. a `json!` object) for inputs the generated mirror types too loosely.
    pub variables: V,
}

impl<V> MexRequest<V> {
    /// Pair a `(name, id)` from a generated op module with its variables.
    /// Prefer the `mex_request!` macro, which names the op once.
    pub fn new(name: &'static str, id: &'static str, variables: V) -> Self {
        Self {
            doc: MexDoc { name, id },
            variables,
        }
    }
}

/// Build a [`MexRequest`] from a generated mex operation module, pulling its
/// `NAME`/`DOC_ID` so the op is named once. Two forms:
///
/// ```ignore
/// // typed Variables, struct-literal sugar:
/// mex_request!(join_newsletter { newsletter_id: Some(jid.to_string()) })
/// // explicit value (typed Variables value, or a json! for loosely-typed inputs):
/// mex_request!(update_group_property, serde_json::json!({ "group_id": id }))
/// ```
macro_rules! mex_request {
    ($op:path { $($body:tt)* }) => {{
        use $op as __mex_op;
        $crate::features::mex::MexRequest::new(
            __mex_op::NAME,
            __mex_op::DOC_ID,
            __mex_op::Variables { $($body)* },
        )
    }};
    ($op:path, $vars:expr $(,)?) => {{
        use $op as __mex_op;
        $crate::features::mex::MexRequest::new(__mex_op::NAME, __mex_op::DOC_ID, $vars)
    }};
}
pub(crate) use mex_request;

/// Capping surface the quota is asked about. WhatsApp Web only ever asks about
/// the one-on-one new-chat thread cap, so this is a constant rather than a knob.
const NEW_CHAT_THREAD_CAPPING_TYPE: &str = "INDIVIDUAL_NEW_CHAT_THREAD";

/// Where the account stands against the cap.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum CappingStatus {
    #[wire = "NONE"]
    None,
    #[wire = "FIRST_WARNING"]
    FirstWarning,
    #[wire = "SECOND_WARNING"]
    SecondWarning,
    #[wire = "CAPPED"]
    Capped,
    #[wire_fallback]
    Other(String),
}

/// Eligibility for the one-time extension that lifts the cap for a cycle.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum CappingOteStatus {
    #[wire = "NOT_ELIGIBLE"]
    NotEligible,
    #[wire = "ELIGIBLE"]
    Eligible,
    #[wire = "ACTIVE_IN_CURRENT_CYCLE"]
    ActiveInCurrentCycle,
    #[wire = "EXHAUSTED"]
    Exhausted,
    #[wire_fallback]
    Other(String),
}

/// Meta Verified subscription state, which is what lifts the cap permanently.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum CappingMvStatus {
    #[wire = "NOT_ELIGIBLE"]
    NotEligible,
    #[wire = "NOT_ACTIVE"]
    NotActive,
    #[wire = "ACTIVE"]
    Active,
    #[wire = "ACTIVE_UPGRADE_AVAILABLE"]
    ActiveUpgradeAvailable,
    #[wire_fallback]
    Other(String),
}

/// How many new one-on-one conversations the account may still start in the
/// current cycle, and why.
///
/// Every field is optional because the server omits the ones that do not apply
/// to an account's tier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct NewChatMessageCapping {
    pub capping_status: Option<CappingStatus>,
    pub ote_status: Option<CappingOteStatus>,
    pub mv_status: Option<CappingMvStatus>,
    /// New chats allowed per cycle.
    pub total_quota: Option<u64>,
    /// New chats already started this cycle.
    pub used_quota: Option<u64>,
    /// Unix seconds.
    pub cycle_start_timestamp: Option<i64>,
    /// Unix seconds. WhatsApp Web treats the cap as lifted once this passes.
    pub cycle_end_timestamp: Option<i64>,
    /// Unix seconds; the server's clock when it answered.
    pub server_sent_timestamp: Option<i64>,
}

impl NewChatMessageCapping {
    /// New chats still available this cycle, when both quota fields are present.
    pub fn remaining_quota(&self) -> Option<u64> {
        let total = self.total_quota?;
        Some(total.saturating_sub(self.used_quota?))
    }
}

/// Feature handle for MEX GraphQL operations.
pub struct Mex<'a> {
    client: &'a Client,
}

impl<'a> Mex<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Execute a GraphQL query.
    #[inline]
    pub async fn query<V: Serialize>(
        &self,
        request: MexRequest<V>,
    ) -> Result<MexResponse, MexError> {
        self.execute_request(request).await
    }

    /// Execute a GraphQL mutation.
    #[inline]
    pub async fn mutate<V: Serialize>(
        &self,
        request: MexRequest<V>,
    ) -> Result<MexResponse, MexError> {
        self.execute_request(request).await
    }

    /// Fetch the account's current reachout-timelock state.
    pub async fn fetch_reachout_timelock(&self) -> Result<ReachoutTimelock, MexError> {
        let response = self.query(mex_request!(fetch_reachout_timelock {})).await?;
        decode_reachout_timelock(response.data)
    }

    /// Fetch the cap on how many new one-on-one conversations this account may
    /// start in the current cycle.
    ///
    /// WhatsApp Web issues this at app launch and refreshes it on a TTL
    /// (`wa_individual_new_chat_msg_capping_fetch_ttl_seconds`, 1h), gated on
    /// `wa_individual_new_chat_msg_capping_enabled`. Accounts that the cap does
    /// not apply to still get an answer; it just reports `NONE`.
    pub async fn fetch_new_chat_message_capping_info(
        &self,
    ) -> Result<NewChatMessageCapping, MexError> {
        let response = self
            .query(mex_request!(fetch_new_chat_message_capping_info {
                input: Some(fetch_new_chat_message_capping_info::Input {
                    r#type: Some(NEW_CHAT_THREAD_CAPPING_TYPE.to_string()),
                }),
            }))
            .await?;
        decode_new_chat_message_capping(response.data)
    }

    #[inline]
    async fn execute_request<V: Serialize>(
        &self,
        request: MexRequest<V>,
    ) -> Result<MexResponse, MexError> {
        // Serialize the variables here so a caller-side serialization error
        // surfaces as MexError::Json instead of a malformed empty request.
        let spec = MexQuerySpec::new(request.doc, &request.variables)?;
        self.execute_spec(spec).await
    }

    // Non-generic so the execute/error-handling body instantiates once, not
    // per variables type.
    async fn execute_spec(&self, spec: MexQuerySpec) -> Result<MexResponse, MexError> {
        let response = self.client.execute(spec).await?;

        // Check for fatal errors (the IqSpec already checks, but we want to return our error type)
        if let Some(fatal) = response.fatal_error() {
            let code = fatal.error_code().unwrap_or(500);
            return Err(MexError::ExtensionError {
                code,
                message: fatal.message.clone(),
            });
        }

        Ok(response)
    }
}

fn decode_reachout_timelock(data: Option<serde_json::Value>) -> Result<ReachoutTimelock, MexError> {
    let data = data.ok_or_else(|| {
        MexError::PayloadParsing("reachout timelock response missing data".into())
    })?;
    let response: fetch_reachout_timelock::Response = serde_json::from_value(data)?;
    response
        .xwa2_fetch_account_reachout_timelock
        .ok_or_else(|| {
            MexError::PayloadParsing("reachout timelock response missing account state".into())
        })
}

/// Read a GraphQL scalar that the schema types as a string but the server has
/// been observed to send either way. WhatsApp Web coerces every one of these
/// with `Number(...)`, so neither form is exceptional.
fn numeric_scalar(value: Option<&serde_json::Value>) -> Option<i64> {
    match value? {
        serde_json::Value::Number(n) => n.as_i64(),
        serde_json::Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn string_scalar(value: Option<&serde_json::Value>) -> Option<&str> {
    value?.as_str()
}

fn decode_new_chat_message_capping(
    data: Option<serde_json::Value>,
) -> Result<NewChatMessageCapping, MexError> {
    let data = data.ok_or_else(|| {
        MexError::PayloadParsing("new chat message capping response missing data".into())
    })?;
    // The generated `Response` mirror types the quota and timestamp scalars from
    // a static read of the bundle and gets them wrong in both directions, so the
    // payload is read field by field here rather than deserialized into it.
    let info = data.get("xwa2_message_capping_info").ok_or_else(|| {
        MexError::PayloadParsing("new chat message capping response missing capping info".into())
    })?;
    // Anything that is not an object — null included — would read as an all-absent
    // cap, which a caller cannot tell apart from a genuinely sparse one. WhatsApp
    // Web raises a 500 on the null case; treat every other non-object the same way.
    if !info.is_object() {
        return Err(MexError::PayloadParsing(
            "new chat message capping response has a non-object capping info".into(),
        ));
    }

    Ok(NewChatMessageCapping {
        capping_status: string_scalar(info.get("capping_status")).map(CappingStatus::from),
        ote_status: string_scalar(info.get("ote_status")).map(CappingOteStatus::from),
        mv_status: string_scalar(info.get("mv_status")).map(CappingMvStatus::from),
        total_quota: numeric_scalar(info.get("total_quota")).and_then(|v| u64::try_from(v).ok()),
        used_quota: numeric_scalar(info.get("used_quota")).and_then(|v| u64::try_from(v).ok()),
        cycle_start_timestamp: numeric_scalar(info.get("cycle_start_timestamp")),
        cycle_end_timestamp: numeric_scalar(info.get("cycle_end_timestamp")),
        server_sent_timestamp: numeric_scalar(info.get("server_sent_timestamp")),
    })
}

impl Client {
    #[inline]
    pub fn mex(&self) -> Mex<'_> {
        Mex::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_mex_request_carries_doc() {
        const DOC: MexDoc = MexDoc {
            name: "WAWebMexTestQuery",
            id: "29829202653362039",
        };
        let request = MexRequest {
            doc: DOC,
            variables: json!({}),
        };

        assert_eq!(request.doc.id, "29829202653362039");
        assert_eq!(request.doc.name, "WAWebMexTestQuery");
    }

    #[test]
    fn test_mex_response_deserialization() {
        let json_str = r#"{
            "data": {
                "xwa2_fetch_wa_users": [
                    {"jid": "1234567890@s.whatsapp.net", "country_code": "1"}
                ]
            }
        }"#;

        let response: MexResponse = serde_json::from_str(json_str).unwrap();
        assert!(response.has_data());
        assert!(!response.has_errors());
        assert!(response.fatal_error().is_none());
    }

    #[test]
    fn test_mex_response_with_error_code_is_fatal() {
        // WhatsApp Web treats any error with error_code as fatal
        let json_str = r#"{
            "data": null,
            "errors": [
                {
                    "message": "User not found",
                    "extensions": {
                        "error_code": 404,
                        "is_summary": false,
                        "is_retryable": false,
                        "severity": "WARNING"
                    }
                }
            ]
        }"#;

        let response: MexResponse = serde_json::from_str(json_str).unwrap();
        assert!(!response.has_data());
        assert!(response.has_errors());

        let fatal = response.fatal_error();
        assert!(fatal.is_some());
        assert_eq!(fatal.unwrap().error_code(), Some(404));
    }

    #[test]
    fn test_mex_response_with_fatal_error() {
        let json_str = r#"{
            "data": null,
            "errors": [
                {
                    "message": "Fatal server error",
                    "extensions": {
                        "error_code": 500,
                        "is_summary": true,
                        "severity": "CRITICAL"
                    }
                }
            ]
        }"#;

        let response: MexResponse = serde_json::from_str(json_str).unwrap();
        assert!(!response.has_data());
        assert!(response.has_errors());

        let fatal = response.fatal_error();
        assert!(fatal.is_some());

        let fatal = fatal.unwrap();
        assert_eq!(fatal.message, "Fatal server error");
        assert_eq!(fatal.error_code(), Some(500));
        assert!(fatal.is_summary());
    }

    #[test]
    fn test_mex_response_real_world() {
        let json_str = r#"{
            "data": {
                "xwa2_fetch_wa_users": [
                    {
                        "__typename": "XWA2User",
                        "about_status_info": {
                            "__typename": "XWA2AboutStatus",
                            "text": "Hello",
                            "timestamp": "1766267670"
                        },
                        "country_code": "BR",
                        "id": null,
                        "jid": "551199887766@s.whatsapp.net",
                        "username_info": {
                            "__typename": "XWA2ResponseStatus",
                            "status": "EMPTY"
                        }
                    }
                ]
            }
        }"#;

        let response: MexResponse = serde_json::from_str(json_str).unwrap();
        assert!(response.has_data());
        assert!(!response.has_errors());

        let data = response.data.unwrap();
        let users = data["xwa2_fetch_wa_users"].as_array().unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0]["country_code"], "BR");
        assert_eq!(users[0]["jid"], "551199887766@s.whatsapp.net");
    }

    #[test]
    fn test_reachout_timelock_response() {
        let result = decode_reachout_timelock(Some(json!({
            "xwa2_fetch_account_reachout_timelock": {
                "is_active": true,
                "time_enforcement_ends": "1770000000",
                "enforcement_type": "DEFAULT"
            }
        })))
        .expect("reachout payload");

        assert_eq!(result.is_active, Some(true));
        assert_eq!(result.time_enforcement_ends.as_deref(), Some("1770000000"));
        assert_eq!(result.enforcement_type.as_deref(), Some("DEFAULT"));
        assert!(matches!(
            decode_reachout_timelock(None),
            Err(MexError::PayloadParsing(_))
        ));
        assert!(matches!(
            decode_reachout_timelock(Some(json!({
                "xwa2_fetch_account_reachout_timelock": null
            }))),
            Err(MexError::PayloadParsing(_))
        ));
    }

    #[test]
    fn new_chat_capping_request_carries_the_thread_type() {
        let request = mex_request!(fetch_new_chat_message_capping_info {
            input: Some(fetch_new_chat_message_capping_info::Input {
                r#type: Some(NEW_CHAT_THREAD_CAPPING_TYPE.to_string()),
            }),
        });

        assert_eq!(
            request.doc.name,
            "WAWebMexFetchNewChatMessageCappingInfoJobQuery"
        );
        assert_eq!(request.doc.id, "24503548349331633");
        assert_eq!(
            serde_json::to_value(&request.variables).expect("variables serialize"),
            json!({ "input": { "type": "INDIVIDUAL_NEW_CHAT_THREAD" } })
        );
    }

    #[test]
    fn new_chat_capping_response_decodes_string_scalars() {
        // The live payload sends the quota and timestamp scalars as strings.
        let capping = decode_new_chat_message_capping(Some(json!({
            "xwa2_message_capping_info": {
                "capping_status": "FIRST_WARNING",
                "ote_status": "ELIGIBLE",
                "mv_status": "NOT_ACTIVE",
                "total_quota": "50",
                "used_quota": "27",
                "cycle_start_timestamp": "1770000000",
                "cycle_end_timestamp": "1772592000",
                "server_sent_timestamp": "1770500000"
            }
        })))
        .expect("capping payload");

        assert_eq!(capping.capping_status, Some(CappingStatus::FirstWarning));
        assert_eq!(capping.ote_status, Some(CappingOteStatus::Eligible));
        assert_eq!(capping.mv_status, Some(CappingMvStatus::NotActive));
        assert_eq!(capping.total_quota, Some(50));
        assert_eq!(capping.used_quota, Some(27));
        assert_eq!(capping.cycle_start_timestamp, Some(1770000000));
        assert_eq!(capping.cycle_end_timestamp, Some(1772592000));
        assert_eq!(capping.server_sent_timestamp, Some(1770500000));
        assert_eq!(capping.remaining_quota(), Some(23));
    }

    #[test]
    fn new_chat_capping_response_decodes_numeric_scalars() {
        let capping = decode_new_chat_message_capping(Some(json!({
            "xwa2_message_capping_info": {
                "capping_status": "CAPPED",
                "total_quota": 50,
                "used_quota": 50,
                "cycle_end_timestamp": 1772592000i64
            }
        })))
        .expect("capping payload");

        assert_eq!(capping.capping_status, Some(CappingStatus::Capped));
        assert_eq!(capping.total_quota, Some(50));
        assert_eq!(capping.used_quota, Some(50));
        assert_eq!(capping.cycle_end_timestamp, Some(1772592000));
        // Absent fields stay absent rather than collapsing to zero.
        assert_eq!(capping.ote_status, None);
        assert_eq!(capping.mv_status, None);
        assert_eq!(capping.cycle_start_timestamp, None);
        assert_eq!(capping.server_sent_timestamp, None);
        assert_eq!(capping.remaining_quota(), Some(0));
    }

    #[test]
    fn new_chat_capping_keeps_unknown_status_values() {
        let capping = decode_new_chat_message_capping(Some(json!({
            "xwa2_message_capping_info": { "capping_status": "THIRD_WARNING" }
        })))
        .expect("capping payload");

        assert_eq!(
            capping.capping_status,
            Some(CappingStatus::Other("THIRD_WARNING".to_string()))
        );
    }

    #[test]
    fn new_chat_capping_missing_payload_is_an_error() {
        // No data at all — what a fatal-free but empty MEX response looks like.
        assert!(matches!(
            decode_new_chat_message_capping(None),
            Err(MexError::PayloadParsing(_))
        ));
        // Data present, capping info absent.
        assert!(matches!(
            decode_new_chat_message_capping(Some(json!({}))),
            Err(MexError::PayloadParsing(_))
        ));
        // WhatsApp Web raises a 500 on exactly this shape.
        assert!(matches!(
            decode_new_chat_message_capping(Some(json!({ "xwa2_message_capping_info": null }))),
            Err(MexError::PayloadParsing(_))
        ));
        // Any other non-object would otherwise read as a cap with every field absent.
        for malformed in [json!("CAPPED"), json!(0), json!([]), json!(true)] {
            assert!(
                matches!(
                    decode_new_chat_message_capping(Some(
                        json!({ "xwa2_message_capping_info": malformed })
                    )),
                    Err(MexError::PayloadParsing(_))
                ),
                "expected a parse error for {malformed}"
            );
        }
    }

    #[test]
    fn test_mex_error_extensions_all_fields() {
        let json_str = r#"{
            "error_code": 400,
            "is_summary": false,
            "is_retryable": true,
            "severity": "WARNING"
        }"#;

        let ext: MexErrorExtensions = serde_json::from_str(json_str).unwrap();
        assert_eq!(ext.error_code, Some(400));
        assert_eq!(ext.is_summary, Some(false));
        assert_eq!(ext.is_retryable, Some(true));
        assert_eq!(ext.severity, Some("WARNING".to_string()));
    }

    #[test]
    fn test_mex_error_extensions_minimal() {
        let json_str = r#"{}"#;

        let ext: MexErrorExtensions = serde_json::from_str(json_str).unwrap();
        assert!(ext.error_code.is_none());
        assert!(ext.is_summary.is_none());
        assert!(ext.is_retryable.is_none());
        assert!(ext.severity.is_none());
    }

    #[test]
    fn invalid_jid_preserves_jid_error_source() {
        let raw: Result<wacore_binary::Jid, JidError> = "not-a-valid-jid".parse();
        let jid_err = raw.unwrap_err();
        let me: MexError = jid_err.into();
        let src = std::error::Error::source(&me).expect("source preserved");
        let inner = src
            .downcast_ref::<JidError>()
            .expect("downcasts to JidError");
        assert!(matches!(inner, JidError::InvalidFormat(_)));
    }

    #[test]
    fn request_preserves_iq_error_source() {
        let iq = IqError::ServerError {
            code: 404,
            text: "not-found".into(),
            error_type: None,
            backoff: None,
        };
        let me: MexError = iq.into();
        let src = std::error::Error::source(&me).expect("source preserved");
        let inner = src.downcast_ref::<IqError>().expect("downcasts to IqError");
        assert!(matches!(inner, IqError::ServerError { code: 404, .. }));
    }
}
