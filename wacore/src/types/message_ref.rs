//! Message content identifiers and operation identifiers are different domains.
//!
//! A chat message uses a client-generated [`MessageId`] and its author. A
//! newsletter can additionally have a [`ServerMessageId`], needed by reactions
//! and votes. [`StanzaId`] names an outgoing operation for ACK correlation; it
//! neither identifies that operation's target nor proves recipient delivery.

use std::fmt;
use std::str::FromStr;

use thiserror::Error;
use wacore_binary::{CompactString, Jid, JidExt};
use waproto::whatsapp as wa;

use super::events::{InboundMessage, ServerAck};
use super::message::{MessageInfo, MessageSource};

/// Invalid or incomplete addressing, detected before an operation is sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MessageRefError {
    #[error("message id cannot be empty")]
    EmptyMessageId,
    #[error("stanza id cannot be empty")]
    EmptyStanzaId,
    #[error("newsletter references require a newsletter JID")]
    ExpectedNewsletter,
    #[error("chat references cannot address newsletters; use NewsletterMessageRef")]
    ExpectedChat,
    #[error("the original sender is required for this message")]
    MissingSender,
    #[error("this operation requires a message sent by us")]
    NotFromMe,
    #[error("receipts require an incoming message")]
    ExpectedIncoming,
    #[error("this operation is not supported for this message origin")]
    UnsupportedOrigin,
    #[error("the newsletter message has no client message id")]
    MissingMessageId,
    #[error("the newsletter message has no server message id")]
    MissingServerMessageId,
}

macro_rules! string_id {
    ($name:ident, $error:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(CompactString);

        impl $name {
            /// Preserve the supplied wire spelling; only an empty id is rejected.
            pub fn new(value: impl AsRef<str>) -> Result<Self, MessageRefError> {
                let value = value.as_ref();
                if value.is_empty() {
                    return Err(MessageRefError::$error);
                }
                Ok(Self(value.into()))
            }

            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Explicit raw escape for APIs that still accept a string.
            pub fn into_string(self) -> String {
                self.0.into()
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl FromStr for $name {
            type Err = MessageRefError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }
        impl TryFrom<&str> for $name {
            type Error = MessageRefError;
            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }
        impl TryFrom<String> for $name {
            type Error = MessageRefError;
            fn try_from(value: String) -> Result<Self, Self::Error> {
                if value.is_empty() {
                    return Err(MessageRefError::$error);
                }
                Ok(Self(value.into()))
            }
        }
    };
}

string_id!(
    MessageId,
    EmptyMessageId,
    "Client-generated identifier of message content within its chat and author scope."
);
string_id!(
    StanzaId,
    EmptyStanzaId,
    "Identifier of an outgoing stanza, for ACK correlation, not recipient delivery."
);

// Raw wire fields can be compared without erasing either typed domain.
impl PartialEq<str> for MessageId {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<&str> for MessageId {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}
impl PartialEq<String> for MessageId {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}
impl PartialEq<MessageId> for String {
    fn eq(&self, other: &MessageId) -> bool {
        self == other.as_str()
    }
}
impl PartialEq<MessageId> for &str {
    fn eq(&self, other: &MessageId) -> bool {
        *self == other.as_str()
    }
}

impl StanzaId {
    /// Observe a validated emitted message envelope as an ACK correlation id.
    /// This explicit projection does not identify an operation's original target.
    pub fn from_message_id(id: &MessageId) -> Self {
        Self(id.0.clone())
    }

    /// Match a message-class ACK (including a negative ACK). The caller must
    /// still check its chat scope and `error`; a match is not a delivery receipt.
    pub fn matches_message_ack(&self, ack: &ServerAck) -> bool {
        ack.class.as_deref() == Some("message") && ack.id == self.as_str()
    }
}

/// Server-assigned newsletter content id, used by reactions, votes and cursors.
///
/// All `u64` values are retained, including zero. This is an encoding contract,
/// not a server acceptance guarantee. Absence is `Option::None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServerMessageId(u64);

impl ServerMessageId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for ServerMessageId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl fmt::Display for ServerMessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Borrowed addressing of a DM, group, status or decrypted comment.
///
/// No message protobuf is copied or retained. Only the short id is owned
/// (inline up to 24 bytes); chat, author and inbound metadata are borrowed.
/// Own send results may not know the author: `from_me` identifies them and an
/// operation that needs our group identity resolves it internally.
#[derive(Debug, Clone)]
pub struct MessageRef<'a> {
    chat: &'a Jid,
    id: MessageId,
    sender: Option<&'a Jid>,
    from_me: bool,
    source: Option<&'a MessageSource>,
    ephemeral_expiration: Option<u32>,
    comment_target: Option<&'a wa::MessageKey>,
}

impl<'a> MessageRef<'a> {
    /// Construct from addressing metadata, without a message body. Prefer the
    /// accessors on inbound messages, contexts and send results when available.
    pub fn new(
        chat: &'a Jid,
        id: MessageId,
        sender: Option<&'a Jid>,
        from_me: bool,
    ) -> Result<Self, MessageRefError> {
        if chat.is_newsletter() {
            return Err(MessageRefError::ExpectedChat);
        }
        if sender.is_some_and(|s| s.user.is_empty())
            || ((!from_me
                && (chat.is_group() || chat.is_status_broadcast() || chat.is_broadcast_list()))
                && sender.is_none())
        {
            return Err(MessageRefError::MissingSender);
        }
        Ok(Self {
            chat,
            id,
            sender,
            from_me,
            source: None,
            ephemeral_expiration: None,
            comment_target: None,
        })
    }

    pub fn from_info(info: &'a MessageInfo) -> Result<Self, MessageRefError> {
        let mut reference = Self::new(
            &info.source.chat,
            MessageId::new(info.id.as_str())?,
            Some(&info.source.sender),
            info.source.is_from_me,
        )?;
        reference.source = Some(&info.source);
        Ok(reference)
    }

    /// Attach the inbound-only metadata; `comment_target` names the parent
    /// post, never the comment this reference addresses.
    pub fn with_inbound_metadata(
        mut self,
        ephemeral_expiration: Option<u32>,
        comment_target: Option<&'a wa::MessageKey>,
    ) -> Self {
        self.ephemeral_expiration = ephemeral_expiration;
        self.comment_target = comment_target;
        self
    }

    pub fn chat(&self) -> &'a Jid {
        self.chat
    }
    pub fn id(&self) -> &MessageId {
        &self.id
    }
    pub fn sender(&self) -> Option<&'a Jid> {
        self.sender
    }
    pub fn from_me(&self) -> bool {
        self.from_me
    }
    pub fn source(&self) -> Option<&'a MessageSource> {
        self.source
    }
    pub fn ephemeral_expiration(&self) -> Option<u32> {
        self.ephemeral_expiration
    }
    pub fn comment_target(&self) -> Option<&'a wa::MessageKey> {
        self.comment_target
    }

    /// Explicit referential key escape for add-ons/app-state. This is NOT an
    /// edit/revoke key: a sender revoke omits participant, while a group edit
    /// needs our chat-specific identity even if a send result lacks it. Own
    /// group references can omit `participant` here; typed client add-on
    /// operations resolve it without changing the borrowed reference.
    pub fn to_raw_key(&self) -> wa::MessageKey {
        let needs_sender = self.chat.is_group()
            || self.chat.is_status_broadcast()
            || self.chat.is_broadcast_list()
            || self.source.is_some_and(|s| s.is_group);
        wa::MessageKey {
            remote_jid: Some(self.chat.to_string()),
            id: Some(self.id.to_string()),
            from_me: Some(self.from_me),
            participant: if needs_sender {
                self.sender.map(ToString::to_string)
            } else {
                None
            },
        }
    }

    /// The receipt participant, absent for a DM regardless of its author.
    pub fn receipt_sender(&self) -> Option<&'a Jid> {
        if self.chat.is_group() || self.chat.is_status_broadcast() || self.chat.is_broadcast_list()
        {
            self.sender
        } else {
            None
        }
    }

    /// Check the common DM/group operation boundary before any identity lookup.
    pub fn require_chat_operation(&self) -> Result<(), MessageRefError> {
        if self.chat.is_status_broadcast() || self.chat.is_broadcast_list() {
            return Err(MessageRefError::UnsupportedOrigin);
        }
        Ok(())
    }

    pub fn require_own(&self) -> Result<(), MessageRefError> {
        if !self.from_me {
            return Err(MessageRefError::NotFromMe);
        }
        Ok(())
    }
}

impl<'a> TryFrom<&'a InboundMessage> for MessageRef<'a> {
    type Error = MessageRefError;
    fn try_from(message: &'a InboundMessage) -> Result<Self, Self::Error> {
        Self::from_info(&message.info).map(|r| {
            r.with_inbound_metadata(
                message.ephemeral_expiration,
                message.comment_target.as_deref(),
            )
        })
    }
}

/// Newsletter addressing with independent optional content ids.
///
/// A freshly sent post may have only a client id; a live counter update only a
/// server id. Editing/revoking requires `message_id`, reacting/voting requires
/// `server_id`. Neither is substituted for the other. No E2E MessageKey coercion.
///
/// ```
/// use wacore::types::message_ref::{MessageId, NewsletterMessageRef, ServerMessageId};
/// let chat = "120363000000000001@newsletter".parse()?;
/// let post = NewsletterMessageRef::new(&chat, Some(MessageId::new("POST")?),
///     Some(ServerMessageId::new(42)))?;
/// assert_eq!(post.require_message_id()?.as_str(), "POST");
/// assert_eq!(post.require_server_id()?.get(), 42);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug, Clone)]
pub struct NewsletterMessageRef<'a> {
    chat: &'a Jid,
    message_id: Option<MessageId>,
    server_id: Option<ServerMessageId>,
    from_me: Option<bool>,
}

impl<'a> NewsletterMessageRef<'a> {
    pub fn new(
        chat: &'a Jid,
        message_id: Option<MessageId>,
        server_id: Option<ServerMessageId>,
    ) -> Result<Self, MessageRefError> {
        if !chat.is_newsletter() {
            return Err(MessageRefError::ExpectedNewsletter);
        }
        Ok(Self {
            chat,
            message_id,
            server_id,
            from_me: None,
        })
    }
    /// Explicit caller declaration of ownership, not inferred envelope provenance.
    pub fn with_from_me(mut self, from_me: bool) -> Self {
        self.from_me = Some(from_me);
        self
    }
    pub fn chat(&self) -> &'a Jid {
        self.chat
    }
    pub fn message_id(&self) -> Option<&MessageId> {
        self.message_id.as_ref()
    }
    pub fn server_id(&self) -> Option<ServerMessageId> {
        self.server_id
    }
    pub fn from_me(&self) -> Option<bool> {
        self.from_me
    }
    pub fn require_message_id(&self) -> Result<&MessageId, MessageRefError> {
        self.message_id
            .as_ref()
            .ok_or(MessageRefError::MissingMessageId)
    }
    pub fn require_server_id(&self) -> Result<ServerMessageId, MessageRefError> {
        self.server_id
            .ok_or(MessageRefError::MissingServerMessageId)
    }
}

#[cfg(test)]
mod tests;
