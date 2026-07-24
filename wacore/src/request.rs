use crate::WireEnum;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::time::Duration;
use thiserror::Error;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, JidExt, LEGACY_USER_SERVER};
use wacore_binary::{Node, NodeContent, NodeRef};

/// IQ request type for WhatsApp protocol queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum InfoQueryType {
    #[wire = "set"]
    Set,
    #[wire = "get"]
    Get,
}

#[derive(Debug, Clone)]
pub struct InfoQuery<'a> {
    pub namespace: &'a str,
    pub query_type: InfoQueryType,
    pub to: Jid,
    pub target: Option<Jid>,
    pub id: Option<String>,
    pub content: Option<NodeContent>,
    pub timeout: Option<Duration>,
}

impl<'a> InfoQuery<'a> {
    pub fn get(namespace: &'a str, to: Jid, content: Option<NodeContent>) -> Self {
        Self {
            namespace,
            query_type: InfoQueryType::Get,
            to,
            target: None,
            id: None,
            content,
            timeout: None,
        }
    }

    pub fn set(namespace: &'a str, to: Jid, content: Option<NodeContent>) -> Self {
        Self {
            namespace,
            query_type: InfoQueryType::Set,
            to,
            target: None,
            id: None,
            content,
            timeout: None,
        }
    }

    pub fn with_target(mut self, target: Jid) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Create a GET query from a Jid reference (avoids clone at call site).
    pub fn get_ref(namespace: &'a str, to: &Jid, content: Option<NodeContent>) -> Self {
        Self::get(namespace, to.clone(), content)
    }

    /// Create a SET query from a Jid reference (avoids clone at call site).
    pub fn set_ref(namespace: &'a str, to: &Jid, content: Option<NodeContent>) -> Self {
        Self::set(namespace, to.clone(), content)
    }

    /// Set target from a Jid reference (avoids clone at call site).
    pub fn with_target_ref(self, target: &Jid) -> Self {
        self.with_target(target.clone())
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IqError {
    #[error("IQ request timed out")]
    Timeout,
    #[error("client is not connected")]
    NotConnected,
    #[error("received disconnect node during IQ wait: {0:?}")]
    Disconnected(Box<Node>),
    #[error("received a server error response: code={code}, text='{text}'")]
    ServerError {
        code: u16,
        text: String,
        /// XMPP error class from the `type` attr (e.g. "wait" vs "cancel"); `None` if absent.
        error_type: Option<String>,
        /// Server-directed retry delay in seconds from the `backoff` attr; `None` if absent.
        /// WA Web honors this (`setProtocolBackoffMs`) before retrying throttled IQs.
        backoff: Option<u32>,
    },
    #[error("received unexpected IQ response type: {got:?}")]
    UnexpectedResponseType { got: Option<String> },
    #[error("internal channel closed unexpectedly")]
    InternalChannelClosed,
}

impl IqError {
    /// The transport is gone rather than the request having been refused.
    ///
    /// Sole owner of that judgement for this type: callers that classify an
    /// error chain read it here instead of restating the variant list.
    pub fn is_transport_unavailable(&self) -> bool {
        matches!(
            self,
            IqError::NotConnected | IqError::Disconnected(_) | IqError::InternalChannelClosed
        )
    }

    /// The request went out and no answer came back in time.
    ///
    /// Matched exhaustively so a new variant has to be classified here rather
    /// than defaulting to "not a timeout" unnoticed.
    pub fn is_timeout(&self) -> bool {
        match self {
            IqError::Timeout => true,
            IqError::NotConnected
            | IqError::Disconnected(_)
            | IqError::ServerError { .. }
            | IqError::UnexpectedResponseType { .. }
            | IqError::InternalChannelClosed => false,
        }
    }
}

/// Lightweight server error that can be embedded in `anyhow::Error` and
/// downcast from any crate. Used as a shared type across crate boundaries
/// when `wacore::request::IqError` isn't directly available (e.g., errors
/// originating from the high-level crate's own `IqError`).
///
/// To check a specific code: `err.downcast_ref::<ServerErrorCode>().is_some_and(|e| e.code == 406)`
#[derive(Debug, Clone, Error)]
#[error("server error: code={code}, text='{text}'")]
pub struct ServerErrorCode {
    pub code: u16,
    pub text: String,
    /// XMPP error class from the `type` attr; `None` if absent.
    pub error_type: Option<String>,
    /// Server-directed retry delay in seconds from the `backoff` attr; `None` if absent.
    pub backoff: Option<u32>,
}

impl ServerErrorCode {
    pub fn from_anyhow(err: &anyhow::Error) -> Option<&Self> {
        err.chain().find_map(|cause| cause.downcast_ref::<Self>())
    }
}

pub struct RequestUtils {
    unique_id: String,
    id_counter: std::sync::Arc<portable_atomic::AtomicU64>,
}

impl RequestUtils {
    pub fn new(unique_id: String) -> Self {
        Self {
            unique_id,
            id_counter: std::sync::Arc::new(portable_atomic::AtomicU64::new(0)),
        }
    }

    pub fn with_counter(
        unique_id: String,
        id_counter: std::sync::Arc<portable_atomic::AtomicU64>,
    ) -> Self {
        Self {
            unique_id,
            id_counter,
        }
    }

    pub fn generate_request_id(&self) -> String {
        let count = self
            .id_counter
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        format!(
            "{unique_id}-{count}",
            unique_id = self.unique_id,
            count = count
        )
    }

    pub fn generate_message_id(&self, user_jid: Option<&Jid>) -> String {
        let mut data = Vec::with_capacity(8 + 20 + 16);

        let timestamp = crate::time::now_secs_u64();
        data.extend_from_slice(&timestamp.to_be_bytes());

        if let Some(jid) = user_jid {
            data.extend_from_slice(jid.user.as_bytes());
            data.extend_from_slice(b"@");
            data.extend_from_slice(LEGACY_USER_SERVER.as_bytes());
        }

        let mut random_bytes = [0u8; 16];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut random_bytes);
        data.extend_from_slice(&random_bytes);

        const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

        let hash = Sha256::digest(&data);
        let truncated = &hash[..9];

        // WA Web message IDs are "3EB0" + 18 hex chars (9-byte truncated hash)
        let mut id = String::with_capacity(22);
        id.push_str("3EB0");
        for &b in truncated {
            id.push(HEX_UPPER[(b >> 4) as usize] as char);
            id.push(HEX_UPPER[(b & 0x0F) as usize] as char);
        }
        id
    }

    pub fn build_iq_node(&self, query: InfoQuery<'_>, req_id: Option<String>) -> Node {
        let id = req_id.unwrap_or_else(|| self.generate_request_id());

        let mut builder = NodeBuilder::new("iq")
            .attr("id", id)
            .attr("xmlns", query.namespace)
            .attr("type", query.query_type.as_str())
            .attr("to", query.to);

        if let Some(target) = query.target
            && !target.is_empty()
        {
            builder = builder.attr("target", target);
        }

        builder.apply_content(query.content).build()
    }

    pub fn parse_iq_response(&self, response_node: &NodeRef<'_>) -> Result<(), IqError> {
        if response_node.tag == "stream:error" || response_node.tag == "xmlstreamend" {
            return Err(IqError::Disconnected(Box::new(response_node.to_owned())));
        }

        let response_type = response_node.get_attr("type");

        if response_type
            .as_ref()
            .is_some_and(|res_type| res_type.as_str() == "error")
        {
            let error_child = response_node.get_optional_child_by_tag(&["error"]);
            if let Some(error_node) = error_child {
                let mut parser = error_node.attrs();
                let code = parser.optional_u64("code").unwrap_or(0) as u16;
                let text = parser
                    .optional_string("text")
                    .as_deref()
                    .unwrap_or("")
                    .to_string();
                // WA Web's parseIqResponse also keeps errorType + errorBackoff; the
                // backoff is the server's directed retry delay (seconds).
                let error_type = parser.optional_string("type").map(|s| s.into_owned());
                // Drop an out-of-range backoff rather than wrapping it to a wrong delay.
                let backoff = parser
                    .optional_u64("backoff")
                    .and_then(|b| u32::try_from(b).ok());
                return Err(IqError::ServerError {
                    code,
                    text,
                    error_type,
                    backoff,
                });
            }
            return Err(IqError::ServerError {
                code: 0,
                text: "Malformed error response".to_string(),
                error_type: None,
                backoff: None,
            });
        }

        let got = response_type.map(|res_type| res_type.to_string());
        if got.as_deref() != Some("result") {
            return Err(IqError::UnexpectedResponseType { got });
        }

        Ok(())
    }
}

#[cfg(test)]
mod iq_error_tests {
    use super::{IqError, RequestUtils};
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn parse_iq_response_extracts_error_type_and_backoff() {
        let node = NodeBuilder::new("iq")
            .attr("type", "error")
            .children([NodeBuilder::new("error")
                .attr("code", "429")
                .attr("text", "rate-overlimit")
                .attr("type", "wait")
                .attr("backoff", "30")
                .build()])
            .build();
        let err = RequestUtils::new("t".to_string())
            .parse_iq_response(&node.as_node_ref())
            .unwrap_err();
        match err {
            IqError::ServerError {
                code,
                text,
                error_type,
                backoff,
            } => {
                assert_eq!(code, 429);
                assert_eq!(text, "rate-overlimit");
                assert_eq!(error_type.as_deref(), Some("wait"));
                assert_eq!(backoff, Some(30));
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn parse_iq_response_error_without_backoff_is_none() {
        let node = NodeBuilder::new("iq")
            .attr("type", "error")
            .children([NodeBuilder::new("error").attr("code", "404").build()])
            .build();
        let err = RequestUtils::new("t".to_string())
            .parse_iq_response(&node.as_node_ref())
            .unwrap_err();
        match err {
            IqError::ServerError {
                code,
                error_type,
                backoff,
                ..
            } => {
                assert_eq!(code, 404);
                assert!(error_type.is_none());
                assert!(backoff.is_none());
            }
            other => panic!("expected ServerError, got {other:?}"),
        }
    }

    #[test]
    fn parse_iq_response_accepts_result_type() {
        let node = NodeBuilder::new("iq").attr("type", "result").build();

        RequestUtils::new("t".to_string())
            .parse_iq_response(&node.as_node_ref())
            .unwrap();
    }

    #[test]
    fn parse_iq_response_rejects_unexpected_type() {
        let node = NodeBuilder::new("iq").attr("type", "get").build();

        let err = RequestUtils::new("t".to_string())
            .parse_iq_response(&node.as_node_ref())
            .unwrap_err();

        match err {
            IqError::UnexpectedResponseType { got } => assert_eq!(got.as_deref(), Some("get")),
            other => panic!("expected UnexpectedResponseType, got {other:?}"),
        }
    }

    #[test]
    fn parse_iq_response_rejects_missing_type() {
        let node = NodeBuilder::new("iq").build();

        let err = RequestUtils::new("t".to_string())
            .parse_iq_response(&node.as_node_ref())
            .unwrap_err();

        match err {
            IqError::UnexpectedResponseType { got } => assert!(got.is_none()),
            other => panic!("expected UnexpectedResponseType, got {other:?}"),
        }
    }
}
