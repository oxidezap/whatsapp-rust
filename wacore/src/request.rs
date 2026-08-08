use crate::WireEnum;
use rand::Rng;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use thiserror::Error;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, JidExt, LEGACY_USER_SERVER};
use wacore_binary::{Node, NodeContent, NodeContentRef, NodeRef};

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
        let count = self.id_counter.fetch_add(1, Ordering::Relaxed);
        format!(
            "{unique_id}-{count}",
            unique_id = self.unique_id,
            count = count
        )
    }

    pub fn generate_message_id(&self, user_jid: Option<&Jid>) -> String {
        self.generate_message_id_at(user_jid, crate::time::now_secs_u64())
    }

    /// Same as [`Self::generate_message_id`], but against a caller-supplied
    /// second, so a send that already sampled the clock for its own timestamps
    /// derives the id from that same instant instead of reading again.
    pub fn generate_message_id_at(&self, user_jid: Option<&Jid>, unix_secs: u64) -> String {
        Self::message_id_at(user_jid, unix_secs)
    }

    /// The id derivation itself, which reads nothing from `self`. Exposed
    /// separately so a caller on the send path does not have to materialize a
    /// `RequestUtils` (and clone the unique id inside it) just to name a
    /// message.
    pub fn message_id_at(user_jid: Option<&Jid>, unix_secs: u64) -> String {
        // Fed straight into the digest instead of through a staging Vec: this
        // runs once per outgoing message and the input is never needed as a
        // contiguous buffer.
        let mut hasher = Sha256::new();
        hasher.update(unix_secs.to_be_bytes());

        if let Some(jid) = user_jid {
            hasher.update(jid.user.as_bytes());
            hasher.update(b"@");
            hasher.update(LEGACY_USER_SERVER.as_bytes());
        }

        // The thread-local generator directly: seeding a fresh StdRng per
        // message ran a full ChaCha key schedule to produce 16 bytes.
        let mut random_bytes = [0u8; 16];
        rand::rng().fill_bytes(&mut random_bytes);
        hasher.update(random_bytes);

        const HEX_UPPER: &[u8; 16] = b"0123456789ABCDEF";

        let hash = hasher.finalize();
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
                // backoff is the server's directed retry delay (seconds). These four names
                // are mirrored in PARSED_ERROR_ATTRS, which decides what counts as unread.
                let error_type = parser.optional_string("type").map(|s| s.into_owned());
                // Drop an out-of-range backoff rather than wrapping it to a wrong delay.
                let backoff = parser
                    .optional_u64("backoff")
                    .and_then(|b| u32::try_from(b).ok());
                warn_on_unread_error_detail(error_node);
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

/// Names of what an `<error>` node carries that [`RequestUtils::parse_iq_response`] does not
/// read. Names only, never values: a value can hold a JID, and this is reported at a level
/// production enables.
#[derive(Debug, PartialEq, Eq)]
struct UnreadErrorDetail<'a> {
    /// Attribute names outside [`PARSED_ERROR_ATTRS`].
    attrs: Vec<&'a str>,
    /// Tags of the child nodes, such as the `<text>`/application-condition elements XMPP
    /// allows.
    children: Vec<&'a str>,
    /// The kind of payload `<error>` held when it carried one instead of child nodes. A payload
    /// is content rather than a name, so only its kind is reported.
    payload: Option<&'static str>,
}

/// What [`RequestUtils::parse_iq_response`] reads off an `<error>` node, mirrored so
/// [`unread_error_detail`] can name the rest. Reading a fifth attribute there means adding it
/// here, or every response starts reporting the new attribute as unread.
const PARSED_ERROR_ATTRS: [&str; 4] = ["code", "text", "type", "backoff"];

/// `None`, the common case, when the node holds nothing beyond [`PARSED_ERROR_ATTRS`].
///
/// Those four are what WA Web's `parseIqResponse` keeps, so reading only them is the right
/// default. What was never checked is whether the server sends more: a bare `bad-request` gives
/// no way to tell an empty error from a detailed one this parser read nothing of. WA Web's escape
/// hatch for that case is `parseIqResponse`'s third argument, a parser over the same node; this
/// probe is what would say a call site needs the equivalent.
fn unread_error_detail<'a>(error_node: &'a NodeRef<'_>) -> Option<UnreadErrorDetail<'a>> {
    let attrs: Vec<&str> = error_node
        .attrs
        .iter()
        .map(|(name, _)| name.as_ref())
        .filter(|name| !PARSED_ERROR_ATTRS.contains(name))
        .collect();

    let mut children = Vec::new();
    let mut payload = None;
    // Not `children()`: it answers `None` for a byte or string payload, which goes unread just
    // the same.
    match error_node.content.as_ref() {
        Some(NodeContentRef::Nodes(nodes)) => {
            children.extend(nodes.iter().map(|child| child.tag.as_ref()));
        }
        Some(NodeContentRef::Bytes(bytes)) if !bytes.is_empty() => payload = Some("bytes"),
        Some(NodeContentRef::String(text)) if !text.is_empty() => payload = Some("text"),
        _ => {}
    }

    if attrs.is_empty() && children.is_empty() && payload.is_none() {
        return None;
    }
    Some(UnreadErrorDetail {
        attrs,
        children,
        payload,
    })
}

/// Reported once per process. The finding is a note to this library's maintainers rather than
/// something the calling application has to act on — every caller hands in the node and still
/// holds it, so what goes unread here is unread, not lost — and rejected IQs arrive in bursts
/// (usync, prekey and app-state fan-outs all retry), so warning per occurrence would be noise at
/// exactly the moment the log matters.
static UNREAD_ERROR_DETAIL_WARNED: AtomicBool = AtomicBool::new(false);

/// `#[cold]` so the probe stays out of the caller's body: `parse_iq_response` is on the receive
/// path and its code size is gated in CI.
#[cold]
fn warn_on_unread_error_detail(error_node: &NodeRef<'_>) {
    // Both guards come before the scan, not after: once the warning is out, or with warnings
    // filtered off, every further rejected IQ costs one relaxed load instead of walking the node.
    if UNREAD_ERROR_DETAIL_WARNED.load(Ordering::Relaxed) || !log::log_enabled!(log::Level::Warn) {
        return;
    }
    let Some(detail) = unread_error_detail(error_node) else {
        return;
    };
    // Racing callers both reach here; the swap picks the single one that logs.
    if UNREAD_ERROR_DETAIL_WARNED.swap(true, Ordering::Relaxed) {
        return;
    }
    log::warn!(
        "IQ error carries detail this parser does not read: attributes={:?} children={:?} \
         payload={:?}. Not lost — read them off the stanza itself. Names only, no values, since \
         a value can hold a JID. Reported once per process.",
        detail.attrs,
        detail.children,
        detail.payload,
    );
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

#[cfg(test)]
mod message_id_tests {
    use super::RequestUtils;
    use wacore_binary::Jid;

    fn jid() -> Jid {
        "13135550100@s.whatsapp.net".parse().expect("valid jid")
    }

    /// The wire format is what WA Web produces; a client that drifts from it is
    /// identifiable as non-official, so it is pinned rather than left implicit.
    #[test]
    fn message_id_keeps_the_wa_web_shape() {
        let id = RequestUtils::message_id_at(Some(&jid()), 1_700_000_000);
        assert_eq!(id.len(), 22, "id must be 3EB0 plus 18 hex chars: {id}");
        assert!(id.starts_with("3EB0"), "id must carry the WA prefix: {id}");
        assert!(
            id[4..]
                .chars()
                .all(|c| c.is_ascii_digit() || ('A'..='F').contains(&c)),
            "id must be upper-case hex after the prefix: {id}"
        );
    }

    /// Two ids from the same second and JID must still differ: the entropy comes
    /// from the random block, not from the clock.
    #[test]
    fn message_id_is_unique_within_one_second() {
        let jid = jid();
        let ids: std::collections::HashSet<String> = (0..64)
            .map(|_| RequestUtils::message_id_at(Some(&jid), 1_700_000_000))
            .collect();
        assert_eq!(ids.len(), 64, "ids repeated within the same second");
    }

    /// The JID is optional on this path (pre-pairing sends); dropping it must
    /// not change the shape.
    #[test]
    fn message_id_without_jid_keeps_the_shape() {
        let id = RequestUtils::message_id_at(None, 1_700_000_000);
        assert_eq!(id.len(), 22);
        assert!(id.starts_with("3EB0"));
    }
}

#[cfg(test)]
mod unread_error_detail_tests {
    use super::unread_error_detail;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::node::{Node, NodeContent};

    /// The shape every rejected IQ observed so far has: nothing goes unread, so the probe has
    /// nothing to say.
    #[test]
    fn a_fully_parsed_error_reports_nothing() {
        let node = NodeBuilder::new("error")
            .attr("code", "429")
            .attr("text", "rate-overlimit")
            .attr("type", "wait")
            .attr("backoff", "30")
            .build();
        let node_ref = node.as_node_ref();
        assert_eq!(unread_error_detail(&node_ref), None);
    }

    #[test]
    fn an_empty_error_reports_nothing() {
        let node = NodeBuilder::new("error").build();
        let node_ref = node.as_node_ref();
        assert_eq!(unread_error_detail(&node_ref), None);
    }

    #[test]
    fn an_unread_attribute_is_named() {
        let node = NodeBuilder::new("error")
            .attr("code", "400")
            .attr("xmlns", "w:profile:picture")
            .build();
        let node_ref = node.as_node_ref();
        let detail = unread_error_detail(&node_ref).expect("xmlns is not parsed");
        assert_eq!(detail.attrs, ["xmlns"]);
        assert!(detail.children.is_empty());
        assert_eq!(detail.payload, None);
    }

    #[test]
    fn child_tags_are_named() {
        let node = NodeBuilder::new("error")
            .attr("code", "400")
            .children([
                NodeBuilder::new("bad-request").build(),
                NodeBuilder::new("text").build(),
            ])
            .build();
        let node_ref = node.as_node_ref();
        let detail = unread_error_detail(&node_ref).expect("children are not parsed");
        assert!(detail.attrs.is_empty());
        assert_eq!(detail.children, ["bad-request", "text"]);
        assert_eq!(detail.payload, None);
    }

    /// A payload is detail too, and it is the case `children()` alone misses.
    #[test]
    fn a_raw_payload_is_reported_by_kind_only() {
        let bytes_node = Node::new(
            "error",
            [("code".into(), "400".into())].into_iter().collect(),
            Some(NodeContent::Bytes(vec![1, 2, 3])),
        );
        let bytes_ref = bytes_node.as_node_ref();
        let detail = unread_error_detail(&bytes_ref).expect("a payload is not parsed");
        assert_eq!(detail.payload, Some("bytes"));

        let text_node = Node::new(
            "error",
            Default::default(),
            Some(NodeContent::String("x".into())),
        );
        let text_ref = text_node.as_node_ref();
        let detail = unread_error_detail(&text_ref).expect("a payload is not parsed");
        assert_eq!(detail.payload, Some("text"));
    }

    /// An empty payload is what an absent one decodes to on some paths; it is not detail.
    #[test]
    fn an_empty_payload_reports_nothing() {
        let node = Node::new(
            "error",
            Default::default(),
            Some(NodeContent::Bytes(vec![])),
        );
        let node_ref = node.as_node_ref();
        assert_eq!(unread_error_detail(&node_ref), None);
    }
}
