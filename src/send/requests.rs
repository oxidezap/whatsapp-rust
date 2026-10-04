//! Small, non-generic inputs to the shared send pipeline.

use std::sync::Arc;

use waproto::whatsapp as wa;

use super::{EditOptions, SendOptions};
use crate::{Jid, MessageRef, MessageSecret};

/// Canonical input to [`crate::Client::send`].
///
/// Construction puts the owned protobuf directly in the allocation returned by
/// `SendResult`. The request carries no client, future, or per-chat cache.
///
/// ```
/// # use whatsapp_rust::{Jid, MessageId, SendOptions, SendRequest};
/// # use whatsapp_rust::proto_helpers::MessageBuilderExt;
/// # use whatsapp_rust::waproto::whatsapp::Message;
/// # fn example(chat: &Jid) -> Result<(), whatsapp_rust::MessageRefError> {
/// let request = SendRequest::new(chat, Message::text("hello"))
///     .with_options(SendOptions::default().with_message_id(MessageId::new("CONTENT")?));
/// # Ok(())
/// # }
/// ```
#[must_use]
pub struct SendRequest {
    pub(super) to: Jid,
    pub(super) message: Arc<wa::Message>,
    pub(super) options: SendOptions,
}

impl SendRequest {
    /// Move content into its shared allocation before the send future is built.
    pub fn new(to: &Jid, message: wa::Message) -> Self {
        Self::from_owned(to.clone(), message)
    }

    // Default shortcuts already own their converted JID; keep their move path
    // rather than cloning it once more through the borrowed public constructor.
    pub(super) fn from_owned(to: Jid, message: wa::Message) -> Self {
        Self {
            to,
            message: Arc::new(message),
            options: SendOptions::default(),
        }
    }

    /// Replace the default send options.
    pub fn with_options(mut self, options: SendOptions) -> Self {
        self.options = options;
        self
    }
}

/// Canonical input to [`crate::Client::edit_message`].
///
/// The target identifies original content; `options.stanza_id` identifies the
/// new operation. An encrypted edit additionally needs the original creator
/// and secret, not a later PN/LID alias lookup. Both requests intentionally omit
/// `Debug`: manually supplied protobuf content can contain secrets.
#[must_use]
pub struct EditRequest<'a> {
    pub(crate) target: MessageRef<'a>,
    pub(crate) content: Arc<wa::Message>,
    pub(crate) options: EditOptions,
    pub(crate) encryption: Option<(&'a Jid, &'a MessageSecret)>,
}

impl<'a> EditRequest<'a> {
    /// Borrow addressing only, without retaining or copying the target body.
    pub fn new(target: MessageRef<'a>, content: wa::Message) -> Self {
        Self {
            target,
            content: Arc::new(content),
            options: EditOptions::default(),
            encryption: None,
        }
    }

    /// Replace the default operation options.
    pub fn with_options(mut self, options: EditOptions) -> Self {
        self.options = options;
        self
    }

    /// Use the original creation's exact cryptographic creator and secret.
    ///
    /// This is an explicit association supplied by the host, e.g. from a
    /// `CreatedEvent`. Address aliases are not cryptographically equivalent.
    /// No secret-store read or identity substitution is performed for it.
    /// Event/poll replacement content selects its corresponding encrypted edit
    /// kind; other replacement content uses the ordinary message-edit kind.
    pub fn with_secret(mut self, creator: &'a Jid, secret: &'a MessageSecret) -> Self {
        self.encryption = Some((creator, secret));
        self
    }
}
