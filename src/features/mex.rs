//! MEX (Meta Exchange) GraphQL feature.
//!
//! Protocol types are defined in `wacore::iq::mex`.
//!
//! # Migration
//!
//! Replace `MexRequest::new(name, id, keys, variables)` with
//! `mex_operation!(module).request(variables)`, and both `query(request)` and
//! `mutate(request)` with `execute(request)`. Neither alias checked operation
//! kind; the canonical executor accepts queries and mutations alike.
//!
//! ```no_run
//! use whatsapp_rust::{Client, MexError, mex_operation,
//!     wacore::iq::mex_operations::join_newsletter};
//! # async fn join(client: &Client) -> Result<(), MexError> {
//! let request = mex_operation!(join_newsletter).request(join_newsletter::Variables {
//!     newsletter_id: Some("123456789@newsletter".into()),
//! });
//! let response = client.mex().execute(request).await?;
//! // Parse response.data in the domain layer, not a generated output mirror.
//! # Ok(())
//! # }
//! ```
//!
//! For custom documents retain explicit [`MexRequest::new_raw`] or
//! [`MexOperation::from_raw_parts`]; for imperfect generated inputs use
//! [`MexOperation::raw_request`]. Declared keys remain diagnostic, not mandatory.
//! Match fatal extension codes on [`MexError::GraphQl`] (formerly the source-less
//! `ExtensionError`); execution preserves the original IQ/parse source chain.
//!
//! Removed entry points are no longer available:
//! ```compile_fail
//! use whatsapp_rust::MexRequest;
//! let request = MexRequest::new("CustomQuery", "123456789", &[], ());
//! ```
//! ```compile_fail
//! # async fn removed(client: &whatsapp_rust::Client) {
//! use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::get_username};
//! client.mex().query(mex_operation!(get_username).request(get_username::Variables {})).await;
//! # }
//! ```
//! ```compile_fail
//! # async fn removed(client: &whatsapp_rust::Client) {
//! use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::get_username};
//! client.mex().mutate(mex_operation!(get_username).request(get_username::Variables {})).await;
//! # }
//! ```
//! ```compile_fail
//! use whatsapp_rust::MexError;
//! let error = MexError::ExtensionError { code: 404, message: "absent".into() };
//! ```

use crate::client::Client;
use crate::request::IqError;
use serde::Serialize;
use std::marker::PhantomData;
use thiserror::Error;
use wacore::WireEnum;
use wacore::iq::mex::MexQuerySpec;
use wacore::iq::mex_operations::{
    fetch_new_chat_message_capping_info, fetch_reachout_timelock, get_username,
};
use wacore_binary::jid::JidError;

// Re-export types from wacore
pub use wacore::iq::mex::{
    MexDoc, MexErrorExtensions, MexFatalError, MexGraphQLError, MexResponse,
};
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

    /// Fatal GraphQL rejection, retaining the original IQ/parse source chain.
    #[error("MEX GraphQL error: code={code}, message='{message}'")]
    GraphQl {
        code: i32,
        message: String,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    #[error("IQ request failed")]
    Request(#[from] IqError),

    #[error("JSON error")]
    Json(#[from] serde_json::Error),
}

/// A persisted MEX operation and its associated variables type.
///
/// Build with [`crate::mex_operation!`] to bind `NAME`, `DOC_ID`, `VARIABLE_KEYS`
/// and `Variables` from the same generated module. No response type is inferred:
/// generated response mirrors are heuristic, and domain parsers remain necessary.
/// Metadata is immutable; this descriptor owns no variables and is cheap to copy.
#[derive(Debug)]
pub struct MexOperation<V> {
    doc: MexDoc,
    declared_variables: &'static [&'static str],
    variables_type: PhantomData<fn(V) -> V>,
}

impl<V> Copy for MexOperation<V> {}

impl<V> Clone for MexOperation<V> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<V> MexOperation<V> {
    /// Advanced, unchecked binding for custom persisted operations.
    ///
    /// The caller is responsible for metadata consistency and choosing `V`.
    /// Prefer [`crate::mex_operation!`] for generated operations; this constructor
    /// is also the public implementation hook used by that macro.
    pub const fn from_raw_parts(doc: MexDoc, declared_variables: &'static [&'static str]) -> Self {
        Self {
            doc,
            declared_variables,
            variables_type: PhantomData,
        }
    }

    pub const fn doc(&self) -> MexDoc {
        self.doc
    }

    pub const fn declared_variables(&self) -> &'static [&'static str] {
        self.declared_variables
    }

    /// Bind precisely this operation's variables type.
    pub fn request(self, variables: V) -> MexRequest<V> {
        self.raw_request(variables)
    }

    /// Explicit escape hatch for inputs the generated mirror represents poorly.
    ///
    /// Metadata still comes from this operation, but `W` is not checked against
    /// its generated `Variables`. Any serializable value is written directly to
    /// the wire, without converting it to `serde_json::Value` first.
    pub fn raw_request<W>(self, variables: W) -> MexRequest<W> {
        MexRequest::new_raw(self.doc, self.declared_variables, variables)
    }
}

/// Bind a generated MEX module's metadata and variables type, naming it once.
///
/// ```
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::join_newsletter};
/// let op = mex_operation!(join_newsletter);
/// let request = op.request(join_newsletter::Variables {
///     newsletter_id: Some("123456789@newsletter".into()),
/// });
/// assert_eq!(request.doc().id, join_newsletter::DOC_ID);
/// ```
///
/// Unrelated variables do not compile (use `raw_request` deliberately instead):
/// ```compile_fail
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::{join_newsletter, get_username}};
/// mex_operation!(join_newsletter).request(get_username::Variables {});
/// ```
#[macro_export]
macro_rules! mex_operation {
    ($op:path $(,)?) => {{
        use $op as __mex_op;
        $crate::MexOperation::<__mex_op::Variables>::from_raw_parts(
            $crate::MexDoc {
                name: __mex_op::NAME,
                id: __mex_op::DOC_ID,
            },
            __mex_op::VARIABLE_KEYS,
        )
    }};
}

/// MEX request with immutable operation metadata and serializable variables.
///
/// Prefer [`crate::mex_operation!`] followed by [`MexOperation::request`] or
/// [`MexOperation::raw_request`]. Execution serializes directly to wire bytes;
/// only the opt-in [`Self::missing_variables`] diagnostic builds a JSON value.
///
/// Metadata cannot be overwritten independently after construction:
/// ```compile_fail
/// use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::join_newsletter};
/// let mut request = mex_operation!(join_newsletter).request(join_newsletter::Variables { newsletter_id: None });
/// request.doc.id = "another operation";
/// ```
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MexRequest<V> {
    doc: MexDoc,
    declared_variables: &'static [&'static str],
    pub variables: V,
}

impl<V> MexRequest<V> {
    /// Advanced raw construction for custom persisted documents.
    /// The caller is responsible for the document ID, name and declared keys.
    pub fn new_raw(doc: MexDoc, declared_variables: &'static [&'static str], variables: V) -> Self {
        Self {
            doc,
            declared_variables,
            variables,
        }
    }

    pub const fn doc(&self) -> MexDoc {
        self.doc
    }

    pub const fn declared_variables(&self) -> &'static [&'static str] {
        self.declared_variables
    }
}

impl<V: Serialize> MexRequest<V> {
    /// Declared variables this request's payload does not carry.
    ///
    /// Diagnostic only: declared keys do not encode requiredness. A non-empty
    /// result can be valid, including optional omissions in `fetch_all_subgroups`.
    /// Execution never invokes this diagnostic or rejects absent keys.
    pub fn missing_variables(&self) -> Result<Vec<&'static str>, serde_json::Error> {
        let value = serde_json::to_value(&self.variables)?;
        let Some(object) = value.as_object() else {
            return Ok(self.declared_variables.to_vec());
        };
        Ok(self
            .declared_variables
            .iter()
            .copied()
            .filter(|key| !object.contains_key(*key))
            .collect())
    }
}

// Internal construction sugar for domain callers. The comma form
// deliberately uses raw variables. Public callers use mex_operation! instead.
macro_rules! mex_request {
    ($op:path { $($body:tt)* }) => {{
        use $op as __mex_op;
        $crate::mex_operation!($op).request(__mex_op::Variables { $($body)* })
    }};
    ($op:path, $vars:expr $(,)?) => {{
        $crate::mex_operation!($op).raw_request($vars)
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

/// This account's own Meta username, as MEX reports it.
///
/// Every field is optional because the server omits the ones that do not apply;
/// an account that never set a username answers 404, which surfaces as `None`
/// from [`Mex::get_username`] rather than as an error.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct OwnUsername {
    /// The handle, without the display-only `@` prefix.
    pub username: Option<String>,
    /// `ACTIVE` or `RESERVED`.
    pub state: Option<String>,
    /// The numeric username key that guards lookups of this account by handle.
    pub key: Option<String>,
}

/// Feature handle for MEX GraphQL operations.
pub struct Mex<'a> {
    client: &'a Client,
}

impl<'a> Mex<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Execute a persisted MEX operation, whether query or mutation.
    ///
    /// Serializes variables directly to wire bytes. Declared-variable omissions
    /// are not validated: the catalog does not encode requiredness. Returns the
    /// raw GraphQL response, not a heuristically inferred response type.
    #[inline]
    pub async fn execute<V: Serialize>(
        &self,
        request: MexRequest<V>,
    ) -> Result<MexResponse, MexError> {
        let spec = MexQuerySpec::new(request.doc, &request.variables)?;
        self.execute_spec(spec).await
    }

    /// Fetch the account's current reachout-timelock state.
    pub async fn fetch_reachout_timelock(&self) -> Result<ReachoutTimelock, MexError> {
        let response = self
            .execute(mex_request!(fetch_reachout_timelock {}))
            .await?;
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
            .execute(mex_request!(fetch_new_chat_message_capping_info {
                input: Some(fetch_new_chat_message_capping_info::Input {
                    r#type: Some(NEW_CHAT_THREAD_CAPPING_TYPE.to_string()),
                }),
            }))
            .await?;
        decode_new_chat_message_capping(response.data)
    }

    /// Read this account's own username, its state, and its username key.
    ///
    /// `None` means no username is set: WhatsApp Web reads the same 404 that
    /// way. Only reads are exposed; setting a username or its key changes the
    /// account's identity in a way the server does not undo, so those two
    /// persisted operations stay unwrapped.
    pub async fn get_username(&self) -> Result<Option<OwnUsername>, MexError> {
        let response = match self.execute(mex_request!(get_username {})).await {
            Ok(response) => response,
            // The official job reads a 404 as "this account has no username",
            // and reaches it from both shapes: WAWebMexNativeClient raises the
            // same error for a fatal GraphQL extension code and for an IQ one.
            Err(MexError::GraphQl { code: 404, .. })
            | Err(MexError::Request(IqError::ServerError { code: 404, .. })) => return Ok(None),
            Err(err) => return Err(err),
        };
        decode_own_username(response.data)
    }

    // Non-generic so the execute/error-handling body instantiates once, not
    // per variables type.
    async fn execute_spec(&self, spec: MexQuerySpec) -> Result<MexResponse, MexError> {
        // A fatal GraphQL error fails the spec's own parse, so it arrives as an
        // `IqError::ParseError` carrying the typed source. Recovering the code
        // here is what lets a caller act on one, rather than on a message.
        self.client.execute(spec).await.map_err(classify_iq_error)
    }
}

fn classify_iq_error(source: IqError) -> MexError {
    if let IqError::ParseError(err) = &source
        && let Some(fatal) = err.downcast_ref::<MexFatalError>()
    {
        return MexError::GraphQl {
            code: fatal.code,
            message: fatal.message.clone(),
            source: Box::new(source),
        };
    }
    MexError::Request(source)
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

fn decode_own_username(data: Option<serde_json::Value>) -> Result<Option<OwnUsername>, MexError> {
    let data =
        data.ok_or_else(|| MexError::PayloadParsing("username response missing data".into()))?;
    let response: get_username::Response = serde_json::from_value(data)?;
    let Some(info) = response
        .xwa2_username_get
        .and_then(|username_get| username_get.username_info)
    else {
        return Ok(None);
    };
    Ok(Some(OwnUsername {
        username: info.username,
        state: info.state,
        key: info.pin,
    }))
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

    #[tokio::test]
    async fn execute_sends_the_mutation() {
        use std::sync::Arc;
        use wacore::iq::mex_operations::join_newsletter as op;
        use wacore_binary::{NodeContentRef, builder::NodeBuilder};

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let task = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                let request = crate::mex_operation!(op).request(op::Variables {
                    newsletter_id: Some("123456789@newsletter".into()),
                });
                client.mex().execute(request).await
            })
        };
        let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let id = sent.attrs().optional_string("id").unwrap().into_owned();
        let root = sent.get();
        let query = root.get_optional_child("query").unwrap();
        assert_eq!(
            query.attrs().optional_string("query_id").unwrap(),
            op::DOC_ID
        );
        let Some(NodeContentRef::Bytes(bytes)) = query.content.as_ref() else {
            panic!("wire payload");
        };
        assert_eq!(
            bytes.as_ref(),
            br#"{"variables":{"newsletter_id":"123456789@newsletter"}}"#
        );
        crate::test_utils::answer_iq(
            &client,
            &id,
            &NodeBuilder::new("iq")
                .attr("id", id.as_str())
                .attr("type", "result")
                .children([NodeBuilder::new("result")
                    .bytes(br#"{"data":{"ok":true}}"#.to_vec())
                    .build()])
                .build(),
        )
        .await;
        assert_eq!(task.await.unwrap().unwrap().data, Some(json!({"ok": true})));
    }

    #[tokio::test]
    async fn execute_accepts_an_omitted_optional_variable() {
        use std::sync::Arc;
        use wacore::iq::mex_operations::fetch_all_subgroups as op;
        use wacore_binary::builder::NodeBuilder;

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let task = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .mex()
                    .execute(crate::mex_operation!(op).request(op::Variables {
                        group_id: Some("123456789@g.us".into()),
                        query_context: Some("INTERACTIVE".into()),
                        sub_group_hint_id: None,
                    }))
                    .await
            })
        };
        let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let id = sent.attrs().optional_string("id").unwrap().into_owned();
        crate::test_utils::answer_iq(
            &client,
            &id,
            &NodeBuilder::new("iq")
                .attr("id", id.as_str())
                .attr("type", "result")
                .children([NodeBuilder::new("result")
                    .bytes(br#"{"data":{}}"#.to_vec())
                    .build()])
                .build(),
        )
        .await;
        assert!(task.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn username_only_maps_graphql_and_iq_404_to_absence() {
        use std::{error::Error, sync::Arc};
        use wacore_binary::builder::NodeBuilder;

        for graphql in [true, false] {
            for code in [404, 403, 500] {
                let (client, transport) = crate::test_utils::create_iq_test_client().await;
                let task = {
                    let client = Arc::clone(&client);
                    tokio::spawn(async move { client.mex().get_username().await })
                };
                let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
                let id = sent.attrs().optional_string("id").unwrap().into_owned();
                let child = if graphql {
                    NodeBuilder::new("result")
                        .bytes(serde_json::to_vec(&json!({
                            "errors": [{"message": "denied", "extensions": {"error_code": code}}]
                        })).unwrap())
                        .build()
                } else {
                    NodeBuilder::new("error")
                        .attr("code", code.to_string())
                        .attr("text", "denied")
                        .build()
                };
                crate::test_utils::answer_iq(
                    &client,
                    &id,
                    &NodeBuilder::new("iq")
                        .attr("id", id.as_str())
                        .attr("type", if graphql { "result" } else { "error" })
                        .children([child])
                        .build(),
                )
                .await;
                let result = task.await.unwrap();
                if code == 404 {
                    assert_eq!(result.unwrap(), None);
                } else {
                    let error = result.unwrap_err();
                    let iq = error.source().unwrap().downcast_ref::<IqError>().unwrap();
                    if graphql {
                        assert!(
                            matches!(error, MexError::GraphQl { code: actual, .. } if actual == code)
                        );
                        let fatal = iq
                            .source()
                            .unwrap()
                            .downcast_ref::<MexFatalError>()
                            .unwrap();
                        assert_eq!(fatal.query, get_username::NAME);
                        assert_eq!(fatal.code, code);
                        assert_eq!(fatal.message, "denied");
                    } else {
                        assert!(
                            matches!(iq, IqError::ServerError { code: actual, .. } if i32::from(*actual) == code)
                        );
                        assert!(matches!(error, MexError::Request(_)));
                    }
                }
            }
        }
    }

    #[test]
    fn graphql_and_json_parse_errors_keep_their_iq_source_chain() {
        use std::error::Error;
        use wacore::iq::spec::IqSpec;
        use wacore_binary::builder::NodeBuilder;
        let spec = MexQuerySpec::new(
            crate::mex_operation!(get_username).doc(),
            &get_username::Variables {},
        )
        .unwrap();
        for (payload, graphql) in [
            (
                br#"{"errors":[{"message":"no username","extensions":{"error_code":404}}]}"#
                    .as_slice(),
                true,
            ),
            (b"invalid-json".as_slice(), false),
        ] {
            let node = NodeBuilder::new("iq")
                .children([NodeBuilder::new("result").bytes(payload.to_vec()).build()])
                .build();
            let parse = spec.parse_response(&node.as_node_ref()).unwrap_err();
            let error = classify_iq_error(IqError::ParseError(parse));
            let iq = error.source().unwrap().downcast_ref::<IqError>().unwrap();
            let IqError::ParseError(inner) = iq else {
                panic!("parse source")
            };
            if graphql {
                assert!(matches!(error, MexError::GraphQl { code: 404, .. }));
                assert_eq!(
                    inner.downcast_ref::<MexFatalError>().unwrap().query,
                    get_username::NAME
                );
                assert!(iq.source().unwrap().is::<MexFatalError>());
            } else {
                assert!(matches!(error, MexError::Request(_)));
                assert!(inner.downcast_ref::<serde_json::Error>().is_some());
                assert!(iq.source().unwrap().is::<serde_json::Error>());
            }
        }
        let error: MexError = serde_json::from_str::<serde_json::Value>("invalid-json")
            .unwrap_err()
            .into();
        assert!(error.source().unwrap().is::<serde_json::Error>());
    }

    #[test]
    fn test_mex_request_carries_doc() {
        const DOC: MexDoc = MexDoc {
            name: "WAWebMexTestQuery",
            id: "29829202653362039",
        };
        let request = MexRequest {
            doc: DOC,
            declared_variables: &[],
            variables: json!({}),
        };

        assert_eq!(request.doc.id, "29829202653362039");
        assert_eq!(request.doc.name, "WAWebMexTestQuery");
    }

    /// Raw payloads can opt into the same declared-key diagnostic. This does
    /// not impose requiredness on construction or execution.
    #[test]
    fn missing_variables_names_what_a_payload_leaves_out() {
        use wacore::iq::mex_operations::fetch_all_newsletters_metadata as op;

        let complete = crate::mex_operation!(op)
            .raw_request(json!({ "fetch_status_metadata": false, "fetch_wamo_sub": false }));
        assert_eq!(
            complete.missing_variables().expect("serialize"),
            Vec::<&str>::new()
        );

        let partial = crate::mex_operation!(op).raw_request(json!({ "fetch_wamo_sub": true }));
        assert_eq!(
            partial.missing_variables().expect("serialize"),
            vec!["fetch_status_metadata"]
        );

        // A payload that is not an object binds nothing at all.
        let bogus = crate::mex_operation!(op).raw_request(json!("nope"));
        assert_eq!(
            bogus.missing_variables().expect("serialize"),
            vec!["fetch_status_metadata", "fetch_wamo_sub"]
        );
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
        // Pinned as a literal on purpose: a persisted-query id is WhatsApp's to
        // rotate, and this is the tripwire that makes a rotation visible in a
        // spec bump instead of silent. It moved once, at 12dbeeb.
        assert_eq!(request.doc.id, "27910975521856601");
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
        let iq = crate::test_utils::server_error_iq(404, "not-found", None, None);
        let me: MexError = iq.into();
        let src = std::error::Error::source(&me).expect("source preserved");
        let inner = src.downcast_ref::<IqError>().expect("downcasts to IqError");
        assert!(matches!(inner, IqError::ServerError { code: 404, .. }));
    }

    #[test]
    fn own_username_decodes_the_username_state_and_key() {
        let decoded = decode_own_username(Some(json!({
            "xwa2_username_get": {
                "username_info": {
                    "username": "example.handle",
                    "state": "ACTIVE",
                    "pin": "1234"
                }
            }
        })))
        .expect("decode")
        .expect("username info");

        assert_eq!(decoded.username.as_deref(), Some("example.handle"));
        assert_eq!(decoded.state.as_deref(), Some("ACTIVE"));
        assert_eq!(decoded.key.as_deref(), Some("1234"));
    }

    #[test]
    fn own_username_reads_an_account_without_one_as_absent() {
        assert_eq!(
            decode_own_username(Some(json!({ "xwa2_username_get": {} }))).expect("decode"),
            None
        );
        assert!(matches!(
            decode_own_username(None),
            Err(MexError::PayloadParsing(_))
        ));
    }
}
