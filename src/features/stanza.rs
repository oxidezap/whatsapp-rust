//! Typed operations for responding to inbound protocol stanzas.

use thiserror::Error;

use crate::cache::Freshness;
use crate::client::ClientError;
use wacore_binary::Jid;
use waproto::whatsapp as wa;

pub(crate) fn required_stanza_attr<'node, 'data>(
    node: &'node wacore_binary::NodeRef<'data>,
    name: &'static str,
) -> Result<&'node wacore_binary::node::ValueRef<'data>, StanzaResponseError> {
    match node.get_attr(name) {
        Some(value) if value != "" => Ok(value),
        _ => Err(StanzaResponseError::MissingAttribute(name)),
    }
}

pub use wacore::protocol::nack::NackReason;
pub use wacore::protocol::retry::RetryReason;

/// A protocol rejection sent as an `<ack error="...">` stanza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StanzaRejection {
    reason: NackReason,
    failure_reason: Option<i32>,
}

impl StanzaRejection {
    /// Reject with a protocol reason and no protobuf failure detail.
    pub const fn new(reason: NackReason) -> Self {
        Self {
            reason,
            failure_reason: None,
        }
    }

    /// Reject malformed protobuf content, optionally attaching its typed failure detail.
    pub const fn invalid_protobuf(failure_reason: Option<i32>) -> Self {
        Self {
            reason: NackReason::InvalidProtobuf,
            failure_reason,
        }
    }

    /// Protocol reason encoded in the rejection.
    pub const fn reason(self) -> NackReason {
        self.reason
    }

    /// Optional protobuf failure detail, present only for `InvalidProtobuf`.
    pub const fn failure_reason(self) -> Option<i32> {
        self.failure_reason
    }
}

impl From<NackReason> for StanzaRejection {
    fn from(reason: NackReason) -> Self {
        Self::new(reason)
    }
}

/// Failure while confirming or rejecting an inbound stanza.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StanzaResponseError {
    #[error("stanza is missing required '{0}' attribute")]
    MissingAttribute(&'static str),
    #[error("the local device identity is unavailable")]
    MissingLocalIdentity,
    #[error("the stanza class does not support this response")]
    UnsupportedStanzaClass,
    #[error("failed to encode stanza response")]
    Encoding(#[from] wacore_binary::error::BinaryError),
    #[error("{0}")]
    Client(#[from] ClientError),
}

/// Options for requesting retransmission of one inbound message stanza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct RetryRequestOptions {
    reason: RetryReason,
    force_include_keys: bool,
}

impl RetryRequestOptions {
    /// Use the default retry reason without forcing a key bundle.
    pub const fn new() -> Self {
        Self {
            reason: RetryReason::UnknownError,
            force_include_keys: false,
        }
    }

    /// Attach the diagnostic reason sent to the original sender.
    pub const fn with_reason(mut self, reason: RetryReason) -> Self {
        self.reason = reason;
        self
    }

    /// Require key material even before the normal retry threshold.
    pub const fn with_force_include_keys(mut self, force_include_keys: bool) -> Self {
        self.force_include_keys = force_include_keys;
        self
    }

    /// The diagnostic reason sent with this request.
    pub const fn reason(self) -> RetryReason {
        self.reason
    }

    /// Whether key material is required before the normal retry threshold.
    pub const fn force_include_keys(self) -> bool {
        self.force_include_keys
    }
}

impl Default for RetryRequestOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a retransmission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RetryRequestOutcome {
    /// A retry receipt reached the transport.
    Sent {
        /// Shared attempt count encoded in the retry receipt.
        retry_count: u8,
        /// Whether this attempt carried the local key bundle.
        included_keys: bool,
    },
    /// The protocol excludes this sender/chat combination from retry receipts.
    Suppressed {
        /// Shared attempt count consumed by the suppressed request.
        retry_count: u8,
    },
    /// The shared retry counter had already reached its configured limit.
    LimitReached,
}

/// Failure while parsing or sending a retransmission request.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum RetryRequestError {
    #[error("retry requests require a message stanza")]
    UnsupportedStanzaClass,
    #[error("stanza is missing required '{0}' attribute")]
    MissingAttribute(&'static str),
    #[error("the local device identity is unavailable")]
    MissingLocalIdentity,
    #[error("invalid message stanza")]
    InvalidStanza(#[source] anyhow::Error),
    #[error("{0}")]
    Client(#[from] ClientError),
    #[error("failed to prepare retry request")]
    Internal(#[from] anyhow::Error),
}

/// A request to retransmit an already-sent message to one requesting device.
///
/// The client derives the wire stanza and owns all routing, encryption, session,
/// sender-key, and persistence decisions. This type never accepts a pre-built
/// retry stanza.
#[derive(Debug)]
#[non_exhaustive]
pub struct MessageRetransmission {
    pub(crate) chat: Jid,
    pub(crate) requester: Jid,
    pub(crate) message: wa::Message,
    pub(crate) message_id: String,
    pub(crate) retry_count: u8,
    pub(crate) recipient: Option<Jid>,
    pub(crate) group_metadata_freshness: Freshness,
}

impl MessageRetransmission {
    /// Describe a retransmission to the device that requested it.
    pub fn new(
        chat: Jid,
        requester: Jid,
        message: wa::Message,
        message_id: String,
        retry_count: u8,
    ) -> Self {
        Self {
            chat,
            requester,
            message,
            message_id,
            retry_count,
            recipient: None,
            group_metadata_freshness: Freshness::CachePreferred,
        }
    }

    /// Preserve the receipt's recipient for self-device and bot retry routes.
    pub fn with_recipient(mut self, recipient: Jid) -> Self {
        self.recipient = Some(recipient);
        self
    }

    /// Select how group metadata is obtained for this operation.
    pub fn with_group_metadata_freshness(mut self, freshness: Freshness) -> Self {
        self.group_metadata_freshness = freshness;
        self
    }

    pub fn chat(&self) -> &Jid {
        &self.chat
    }

    pub fn requester(&self) -> &Jid {
        &self.requester
    }

    pub fn message(&self) -> &wa::Message {
        &self.message
    }

    pub fn message_id(&self) -> &str {
        &self.message_id
    }

    pub const fn retry_count(&self) -> u8 {
        self.retry_count
    }

    pub fn recipient(&self) -> Option<&Jid> {
        self.recipient.as_ref()
    }

    pub const fn group_metadata_freshness(&self) -> Freshness {
        self.group_metadata_freshness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_protobuf_is_the_only_rejection_with_failure_detail() {
        let rejection = StanzaRejection::invalid_protobuf(Some(17));
        assert_eq!(rejection.reason(), NackReason::InvalidProtobuf);
        assert_eq!(rejection.failure_reason(), Some(17));

        let rejection = StanzaRejection::new(NackReason::ParsingError);
        assert_eq!(rejection.reason(), NackReason::ParsingError);
        assert_eq!(rejection.failure_reason(), None);
    }

    #[test]
    fn retry_request_options_have_protocol_safe_defaults() {
        let defaults = RetryRequestOptions::default();
        assert_eq!(defaults.reason(), RetryReason::UnknownError);
        assert!(!defaults.force_include_keys());

        let configured = defaults
            .with_reason(RetryReason::BadMac)
            .with_force_include_keys(true);
        assert_eq!(configured.reason(), RetryReason::BadMac);
        assert!(configured.force_include_keys());
    }

    #[test]
    fn internal_retry_error_preserves_its_source() {
        use std::error::Error as _;

        let error = RetryRequestError::from(anyhow::anyhow!("storage sentinel"));
        assert_eq!(
            error.source().map(ToString::to_string).as_deref(),
            Some("storage sentinel")
        );
    }

    #[test]
    fn response_encoding_error_preserves_its_source() {
        use std::error::Error as _;

        let error = StanzaResponseError::from(wacore_binary::error::BinaryError::MissingAttr(
            "sentinel".to_owned(),
        ));
        assert!(
            error
                .source()
                .is_some_and(|source| source.to_string().contains("sentinel"))
        );
    }
}
