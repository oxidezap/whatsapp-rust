use crate::client::Client;
use crate::types::message::EditAttribute;
use anyhow::anyhow;
use log::debug;
use wacore::client::context::SendContextResolver;
use wacore::libsignal::protocol::SignalProtocolError;
use wacore::send::StanzaType;
use wacore::types::jid::JidExt;
use wacore::types::message::AddressingMode;
#[cfg(test)]
use wacore_binary::DeviceKey;
use wacore_binary::Node;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, JidExt as _, Server};
use waproto::whatsapp as wa;

use crate::client::ClientError;
use crate::features::GroupError;
use crate::request::IqError;
use thiserror::Error;

mod actions;
mod tctoken_lifecycle;

/// Error returned by the message send path ([`Client::send_message`],
/// [`Client::send_text`], [`Client::forward_message`], reactions, edits,
/// revokes, pins, polls, events, comments, status) and the bot
/// [`crate::bot::MessageContext`] helpers.
///
/// Wraps the shared [`ClientError`] (transport/connection/IQ) and surfaces the
/// actionable send-time failure modes explicitly. `Internal` is the last-resort
/// catch-all for crypto/encoding paths that still thread `anyhow` internally.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SendError {
    /// Connection/transport/IQ failure (embeds the shared base error).
    // No `#[from]`: the manual `From<ClientError>` impl flattens a bare `?` so
    // `NotLoggedIn`/`Iq` stay matchable instead of nesting under `Client(..)`.
    #[error(transparent)]
    Client(ClientError),
    /// The client has no PN/LID identity yet (not paired / mid LID migration).
    #[error("client is not logged in")]
    NotLoggedIn,
    /// An IQ issued as part of the send (e.g. a group-info query) failed.
    #[error("IQ request failed: {0}")]
    Iq(#[from] IqError),
    /// The recipient JID or send arguments are invalid for this operation
    /// (e.g. a newsletter JID on the E2E path, an empty status recipient list).
    #[error("invalid send request: {0}")]
    InvalidRequest(String),
    /// Catch-all for internal send failures (Signal encrypt, protobuf, group
    /// resolution) that have no dedicated variant yet. Transparent so the
    /// underlying error's `Display`/source chain is preserved.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl SendError {
    /// Map an `anyhow::Error` bubbled up from a helper that still threads
    /// `anyhow` (e.g. `send_message_impl`, `require_pn`) into a typed
    /// `SendError`, recovering the concrete [`ClientError`]. Without this the
    /// blanket `#[from] anyhow::Error` would funnel a logged-out
    /// `ClientError::NotLoggedIn` into the un-matchable `Internal` catch-all.
    pub(crate) fn from_anyhow(err: anyhow::Error) -> Self {
        // A validation deeper in the pipeline may already be a typed `SendError`
        // (e.g. send_message_impl's newsletter/status guards); recover it so it
        // stays matchable instead of collapsing into `Internal`.
        let err = match err.downcast::<SendError>() {
            Ok(send) => return send,
            Err(other) => other,
        };
        // A group-metadata IQ in the send path (e.g. query_info) bubbles up as
        // `GroupError`; flatten it before the `ClientError` check so an IQ
        // failure surfaces as `SendError::Iq`, not the `Internal` catch-all.
        let err = match err.downcast::<GroupError>() {
            Ok(group) => return group.into(),
            Err(other) => other,
        };
        match err.downcast::<ClientError>() {
            Ok(client) => client.into(),
            Err(other) => match other.downcast::<IqError>() {
                Ok(iq) => SendError::Iq(iq),
                Err(other) => SendError::Internal(other),
            },
        }
    }
}

impl From<ClientError> for SendError {
    fn from(err: ClientError) -> Self {
        match err {
            ClientError::NotLoggedIn => SendError::NotLoggedIn,
            ClientError::Iq(iq) => SendError::Iq(iq),
            client => SendError::Client(client),
        }
    }
}

impl From<GroupError> for SendError {
    fn from(err: GroupError) -> Self {
        match err {
            GroupError::Iq(iq) => SendError::Iq(iq),
            GroupError::InvalidRequest(msg) => SendError::InvalidRequest(msg),
            GroupError::Internal(e) => SendError::from_anyhow(e),
            // No dedicated variant for MEX mutations; preserve the full typed
            // error as the `Internal` source so its Display/source chain survives.
            group @ GroupError::Mex(_) => SendError::Internal(group.into()),
        }
    }
}

/// Returns a `GroupInfo` whose participant list is guaranteed to contain our own
/// sending JID, without deep-cloning the shared (cached) metadata in the common
/// case where the server's participant list already includes us.
fn ensure_self_in_group(
    info: std::sync::Arc<wacore::client::context::GroupInfo>,
    own_sending_jid: &Jid,
) -> std::sync::Arc<wacore::client::context::GroupInfo> {
    if info
        .participants
        .iter()
        .any(|participant| participant.is_same_user_as(own_sending_jid))
    {
        info
    } else {
        let mut owned = (*info).clone();
        owned.participants.push(own_sending_jid.to_non_ad());
        std::sync::Arc::new(owned)
    }
}

/// Options for [`Client::send_message_with_options`].
#[derive(Debug, Clone, Default)]
pub struct SendOptions {
    /// Override the auto-generated message ID.
    /// Useful for resending a failed message with the same ID or idempotency.
    pub message_id: Option<String>,
    /// Extra XML child nodes on the message stanza.
    pub extra_stanza_nodes: Vec<Node>,
    /// Ephemeral duration in seconds. Sets `contextInfo.expiration` on the
    /// message (WA Web `EProtoGenerator.js:183` parity).
    /// Common values: 86400 (24h), 604800 (7d), 7776000 (90d).
    pub ephemeral_expiration: Option<u32>,
    /// Force the `<message type="...">` attribute instead of deriving it from
    /// content. Escape hatch for a type the classifier can't infer.
    pub stanza_type_override: Option<StanzaType>,
}

/// Result of a successfully sent message.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SendResult {
    pub message_id: String,
    pub to: Jid,
}

impl SendResult {
    /// `participant` is `None` -- only valid for the sender's own messages.
    pub fn message_key(&self) -> wa::MessageKey {
        wa::MessageKey {
            remote_jid: Some(self.to.to_string()),
            from_me: Some(true),
            id: Some(self.message_id.clone()),
            participant: None,
        }
    }
}

/// Duration for pinned messages. Default is 7 days (matches WA Web).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum PinDuration {
    Hours24,
    #[default]
    Days7,
    Days30,
}

impl PinDuration {
    fn as_secs(self) -> u32 {
        match self {
            Self::Hours24 => 86_400,
            Self::Days7 => 604_800,
            Self::Days30 => 2_592_000,
        }
    }
}

/// Specifies who is revoking (deleting) the message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum RevokeType {
    /// The message sender deleting their own message.
    #[default]
    Sender,
    /// A group admin deleting another user's message.
    /// `original_sender` is the JID of the user who sent the message being deleted.
    Admin { original_sender: Jid },
}

/// Derive stanza-level edit attribute and meta node from message content.
///
/// The `edit` attribute and the `<meta>` child are independent in WA Web: the
/// edit attribute comes from `editAttribute(msg, subtype)` and the meta node
/// from `genMetaNode(...)`. A message can carry both (e.g. a poll vote sets
/// `polltype=vote` meta; an event edit sets both `event_type=edit` meta and
/// `edit="1"` attribute).
pub(crate) fn infer_stanza_metadata(msg: &wa::Message) -> (Option<EditAttribute>, Option<Node>) {
    use wacore::proto_helpers::MessageExt;
    let edit = EditAttribute::infer_from_message(msg);

    // genMetaNode builds a single <meta> carrying every applicable attr together,
    // so accumulate onto one node instead of emitting at most one attr.
    let mut meta = NodeBuilder::new("meta");
    let mut has_attr = false;

    if msg.poll_creation_message.is_set()
        || msg.poll_creation_message_v2.is_set()
        || msg.poll_creation_message_v3.is_set()
    {
        meta = meta.attr("polltype", "creation");
        has_attr = true;
    } else if let Some(poll_update) = msg.poll_update_message.as_option()
        && poll_update.vote.is_set()
    {
        meta = meta.attr("polltype", "vote");
        has_attr = true;
        // TODO: polltype="result_snapshot" for poll_result_snapshot_message (gated behind AB flag)
    } else if msg.event_message.is_set() {
        meta = meta.attr("event_type", "creation");
        has_attr = true;
    } else if msg.enc_event_response_message.is_set() {
        meta = meta.attr("event_type", "response");
        has_attr = true;
    } else if let Some(sec) = msg.secret_encrypted_message.as_option()
        && sec.secret_enc_type
            == Some(wa::message::secret_encrypted_message::SecretEncType::EventEdit)
    {
        meta = meta.attr("event_type", "edit");
        has_attr = true;
    } else if let Some(ml) = msg
        .protocol_message
        .as_option()
        .and_then(|pm| pm.member_label.as_option())
    {
        // genMetaNode (MsgMetaNode `d`/`p`): a member_label protocol message carries
        // appdata="member_tag" and tag_reason="user_delete" when the label is cleared
        // (empty/absent), "user_update" otherwise.
        let tag_reason = if ml.label.as_deref().unwrap_or("").is_empty() {
            "user_delete"
        } else {
            "user_update"
        };
        meta = meta
            .attr("appdata", "member_tag")
            .attr("tag_reason", tag_reason);
        has_attr = true;
    }

    // genMetaNode: `view_once="true"` whenever the media is view-once (wrapper or
    // inline flag). Detection covers both via MessageExt::is_view_once.
    if msg.is_view_once() {
        meta = meta.attr("view_once", "true");
        has_attr = true;
    }

    (edit, has_attr.then(|| meta.build()))
}

/// Offset subtracted from the current unix timestamp to produce the
/// `privacy_mode_ts` attr value on a `<biz>` stanza. Empirically confirmed
/// against live WhatsApp servers.
const BIZ_PRIVACY_MODE_TS_OFFSET: u64 = 77_980_457;

enum BizCategory<'a> {
    /// `<biz actual_actors host_storage privacy_mode_ts native_flow_name=X/>` — no children.
    PaymentSimple(&'a str),
    /// Nested form preserving the button's flow name.
    NestedNamed(&'a str),
    /// Nested form with `name="mixed"`. Fallback for buttons the server
    /// silently drops when sent under their literal name.
    Mixed,
}

fn classify_button(button_name: &str) -> BizCategory<'_> {
    match button_name {
        "payment_info" => BizCategory::PaymentSimple("payment_info"),
        "review_and_pay" => BizCategory::PaymentSimple("order_details"),
        "review_order" | "order_status" => BizCategory::PaymentSimple("order_status"),
        "payment_status" => BizCategory::PaymentSimple("payment_status"),
        "payment_method" => BizCategory::PaymentSimple("payment_method"),
        "payment_reminder" => BizCategory::PaymentSimple("payment_reminder"),

        "cta_url" => BizCategory::NestedNamed("cta_url"),
        "cta_catalog" => BizCategory::NestedNamed("cta_catalog"),
        "catalog_message" => BizCategory::NestedNamed("catalog_message"),
        "galaxy_message" => BizCategory::NestedNamed("galaxy_message"),
        "booking_confirmation" => BizCategory::NestedNamed("booking_confirmation"),
        "call_permission_request" => BizCategory::NestedNamed("call_permission_request"),
        "open_webview" => BizCategory::NestedNamed("message_with_link"),
        "message_with_link_status" => BizCategory::NestedNamed("message_with_link_status"),

        // quick_reply / cta_copy / cta_call / single_select / send_location
        // and every other unknown name go through the mixed fallback. The
        // server silently drops messages sent under the literal name for
        // these buttons.
        _ => BizCategory::Mixed,
    }
}

/// Derive the `<biz>` stanza child for native-flow interactive messages.
///
/// Returns `None` when the message has no native-flow interactive payload.
/// Otherwise returns the assembled `<biz>` node. The caller is responsible
/// for prepending `<bot biz_bot="1"/>` for DM-bound sends (see
/// `build_extra_stanza_nodes`).
///
/// `now_unix_secs` is the current wall-clock time in unix seconds. Taking it
/// as a parameter keeps the function pure and lets tests pin the resulting
/// `privacy_mode_ts` deterministically without touching the global time
/// provider.
fn infer_biz_node(msg: &wa::Message, now_unix_secs: u64) -> Option<Node> {
    let interactive = extract_interactive_message(msg)?;
    let wa::message::interactive_message::InteractiveMessage::NativeFlowMessage(nf) =
        interactive.interactive_message.as_ref()?
    else {
        return None;
    };

    let first_button_name = nf.buttons.first()?.name.as_deref()?;
    let category = classify_button(first_button_name);
    let privacy_mode_ts = now_unix_secs
        .saturating_sub(BIZ_PRIVACY_MODE_TS_OFFSET)
        .to_string();

    Some(match category {
        BizCategory::PaymentSimple(flow_name) => NodeBuilder::new("biz")
            .attr("actual_actors", "2")
            .attr("host_storage", "2")
            .attr("privacy_mode_ts", &privacy_mode_ts)
            .attr("native_flow_name", flow_name)
            .build(),
        BizCategory::NestedNamed(flow_name) => build_nested_biz(&privacy_mode_ts, flow_name),
        BizCategory::Mixed => build_nested_biz(&privacy_mode_ts, "mixed"),
    })
}

fn build_nested_biz(privacy_mode_ts: &str, flow_name: &str) -> Node {
    NodeBuilder::new("biz")
        .attr("actual_actors", "2")
        .attr("host_storage", "2")
        .attr("privacy_mode_ts", privacy_mode_ts)
        .children([
            NodeBuilder::new("interactive")
                .attr("type", "native_flow")
                .attr("v", "1")
                .children([NodeBuilder::new("native_flow")
                    .attr("v", "9")
                    .attr("name", flow_name)
                    .build()])
                .build(),
            NodeBuilder::new("quality_control")
                .attr("source_type", "third_party")
                .build(),
        ])
        .build()
}

fn extract_interactive_message(msg: &wa::Message) -> Option<&wa::message::InteractiveMessage> {
    // Only checks documentWithCaptionMessage wrapper (for media headers) and direct field.
    // Does not use unwrap_message() since we need the InteractiveMessage specifically.
    if let Some(doc) = msg.document_with_caption_message.as_option()
        && let Some(inner) = doc.message.as_option()
        && let Some(im) = inner.interactive_message.as_option()
    {
        return Some(im);
    }
    msg.interactive_message.as_option()
}

/// Assemble the `extra_stanza_nodes` vector for a non-newsletter send.
///
/// Order: `inferred_meta`, optional `<bot biz_bot="1"/>` (DM only), `<biz>`,
/// then any user-provided extra nodes. Pure so the caller stays trivial and
/// the assembly logic is unit-testable.
fn build_extra_stanza_nodes(
    to: &Jid,
    inferred_meta: Option<Node>,
    biz: Option<Node>,
    user_nodes: Vec<Node>,
) -> Vec<Node> {
    if inferred_meta.is_none() && biz.is_none() {
        return user_nodes;
    }
    let bot_emitted = biz.is_some() && !to.is_group();
    let extra = inferred_meta.is_some() as usize + biz.is_some() as usize + bot_emitted as usize;
    let mut nodes = Vec::with_capacity(user_nodes.len() + extra);
    nodes.extend(inferred_meta);
    if let Some(node) = biz {
        if bot_emitted {
            nodes.push(NodeBuilder::new("bot").attr("biz_bot", "1").build());
        }
        nodes.push(node);
    }
    nodes.extend(user_nodes);
    nodes
}

fn build_revoke_message(
    remote_jid: &Jid,
    from_me: bool,
    message_id: String,
    participant: Option<String>,
) -> wa::Message {
    wa::Message {
        protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
            key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(remote_jid.to_string()),
                from_me: Some(from_me),
                id: Some(message_id),
                participant,
            }),
            r#type: Some(wa::message::protocol_message::Type::Revoke),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// A newsletter (channel) admin op on an existing message: edit (with the
/// replacement body) or revoke. Keeping content tied to the variant makes the
/// invalid edit-without-body / revoke-with-body states unrepresentable.
pub(crate) enum NewsletterEdit<'a> {
    Edit(&'a wa::Message),
    Revoke,
}

/// Build a newsletter (channel) plaintext edit/revoke stanza. The target is keyed
/// by `message_id` (the original message's stanza id string, the wire `id`), NOT
/// by `server_id`: WA Web (mergeNewsletterClientIDMixin -> `id`) and whatsmeow
/// (sendNewsletter, req.ID = protocolMessage.key.id) both reference edit/revoke by
/// the message id and emit no `server_id` (that attr is reaction-only).
pub(crate) fn build_newsletter_edit_node(
    to: &Jid,
    message_id: &str,
    op: NewsletterEdit<'_>,
) -> Node {
    use crate::types::message::EditAttribute;
    let mut plaintext = NodeBuilder::new("plaintext");
    let (edit, stanza_type, body) = match op {
        NewsletterEdit::Edit(m) => {
            if let Some(mt) = wacore::send::media_type_from_message(m) {
                plaintext = plaintext.attr("mediatype", mt);
            }
            (
                EditAttribute::AdminEdit,
                wacore::send::stanza_type_from_message(m),
                waproto::codec::message_to_vec(m),
            )
        }
        NewsletterEdit::Revoke => (EditAttribute::AdminRevoke, "text", Vec::new()),
    };
    NodeBuilder::new("message")
        .attr("to", to)
        .attr("id", message_id)
        .attr("type", stanza_type)
        .attr("edit", edit.to_string_val())
        .children([plaintext.bytes(body).build()])
        .build()
}

/// Build a message edit in WA Web's wire shape: a top-level
/// protocolMessage(type=MESSAGE_EDIT) carrying the new content under
/// editedMessage, same as build_revoke_message and our own receive path. The
/// top-level Message.editedMessage FutureProofMessage is the history/storage
/// form, not what WA Web sends on the wire.
pub(crate) fn build_edit_message(
    remote_jid: &Jid,
    message_id: String,
    participant: Option<String>,
    new_content: wa::Message,
    timestamp_ms: i64,
) -> wa::Message {
    wa::Message {
        protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
            key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(remote_jid.to_string()),
                from_me: Some(true),
                id: Some(message_id),
                participant,
            }),
            r#type: Some(wa::message::protocol_message::Type::MessageEdit),
            edited_message: buffa::MessageField::some(new_content),
            timestamp_ms: Some(timestamp_ms),
            ..Default::default()
        }),
        ..Default::default()
    }
}

impl Client {
    /// Send a message to a user, group, or newsletter.
    ///
    /// Newsletter messages are sent as plaintext (no E2E encryption).
    /// For status/story updates use [`Client::status()`] instead.
    pub fn send_message(
        &self,
        to: impl Into<Jid>,
        message: wa::Message,
    ) -> impl Future<Output = Result<SendResult, SendError>> + '_ {
        // Sync-prologue box: a plain async fn would hold the ~1 KB message
        // by value in every embedder's frame.
        let to = to.into();
        let message = Box::new(message);
        async move {
            // Box::pin: the inner future carries ~1 KB of pre-encrypt locals.
            Box::pin(self.send_message_with_options_inner(to, message, SendOptions::default()))
                .await
        }
    }

    /// Plain-text convenience over [`Client::send_message`].
    pub fn send_text(
        &self,
        to: impl Into<Jid>,
        text: impl Into<String>,
    ) -> impl Future<Output = Result<SendResult, SendError>> + '_ {
        use wacore::proto_helpers::MessageBuilderExt;
        let to = to.into();
        let message = Box::new(wa::Message::text(text));
        async move {
            Box::pin(self.send_message_with_options_inner(to, message, SendOptions::default()))
                .await
        }
    }

    /// Forward an existing message to a chat.
    ///
    /// Builds a forward-ready copy of `message` (sets `is_forwarded`, bumps the
    /// forwarding score, strips the reply/quote chain, and drops the source
    /// `message_secret`) via [`MessageExt::prepare_for_forward`], then sends it.
    /// `message` may be a received body or a wrapper (ephemeral/view-once); the
    /// inner content is unwrapped before forwarding. Existing media is relayed
    /// from the same CDN blob rather than re-uploaded.
    pub fn forward_message(
        &self,
        to: impl Into<Jid>,
        message: &wa::Message,
    ) -> impl Future<Output = Result<SendResult, SendError>> + '_ {
        use wacore::proto_helpers::MessageExt;
        let to = to.into();
        let body = message.get_base_message().prepare_for_forward();
        async move {
            Box::pin(self.send_message_with_options_inner(to, body, SendOptions::default())).await
        }
    }

    /// Send a message with additional options.
    pub fn send_message_with_options(
        &self,
        to: impl Into<Jid>,
        message: wa::Message,
        options: SendOptions,
    ) -> impl Future<Output = Result<SendResult, SendError>> + '_ {
        // Thin generic shim: the large async body below stays monomorphic so
        // each `Into<Jid>` instantiation does not duplicate the state machine.
        // Sync-prologue box + Box::pin as in send_message.
        let to = to.into();
        let message = Box::new(message);
        async move { Box::pin(self.send_message_with_options_inner(to, message, options)).await }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.send.message",
            level = "debug",
            skip_all,
            fields(
                to = %to.observe(),
                lid = tracing::field::Empty,
                pn = tracing::field::Empty
            ),
            err(Debug)
        )
    )]
    async fn send_message_with_options_inner(
        &self,
        to: Jid,
        mut message: Box<wa::Message>,
        options: SendOptions,
    ) -> Result<SendResult, SendError> {
        #[cfg(feature = "tracing")]
        self.record_identity_on_span(&tracing::Span::current());

        let _t = wacore::telemetry::timer(wacore::telemetry::SEND_DURATION);
        wacore::telemetry::send(match to.server {
            wacore_binary::Server::Group => "group",
            wacore_binary::Server::Broadcast => "status",
            wacore_binary::Server::Newsletter => "newsletter",
            _ => "dm",
        });
        if let Some(exp) = options.ephemeral_expiration
            && exp > 0
        {
            use wacore::proto_helpers::MessageExt;
            if !message.set_ephemeral_expiration(exp) {
                // Bare `conversation` messages have no contextInfo field.
                log::warn!("Could not set contextInfo.expiration on this message type");
            }
        }

        let stanza_type_override = options.stanza_type_override;
        let request_id = match options.message_id {
            Some(id) => id,
            None => self.generate_message_id(),
        };
        // Both paths below consume `to` and `request_id`, so save copies for the result.
        let result = SendResult {
            message_id: request_id.clone(),
            to: to.clone(),
        };

        // Newsletters are not E2E encrypted — send as plaintext via SMAX stanza.
        // Matches WA Web's OutMessagePublishNewsletterRequest + ContentType mixins.
        if to.is_newsletter() {
            let stanza_type = stanza_type_override
                .map(StanzaType::as_wire)
                .unwrap_or_else(|| wacore::send::stanza_type_from_message(&message));
            let (_, meta_node) = infer_stanza_metadata(&message);
            let mut plaintext_builder = NodeBuilder::new("plaintext");
            if let Some(mt) = wacore::send::media_type_from_message(&message) {
                plaintext_builder = plaintext_builder.attr("mediatype", mt);
            }
            let mut children = vec![
                plaintext_builder
                    .bytes(waproto::codec::message_to_vec(&message))
                    .build(),
            ];
            children.extend(meta_node);
            children.extend(options.extra_stanza_nodes);
            let stanza = NodeBuilder::new("message")
                .attr("to", to)
                .attr("type", stanza_type)
                .attr("id", &request_id)
                .children(children)
                .build();
            self.send_node(stanza).await?;
            return Ok(result);
        }

        let (edit, inferred_meta) = infer_stanza_metadata(&message);
        let now_unix_secs = wacore::time::now_secs_u64();
        let biz = infer_biz_node(&message, now_unix_secs);

        let extra_nodes =
            build_extra_stanza_nodes(&to, inferred_meta, biz, options.extra_stanza_nodes);
        // send_message_impl's state machine is ~13 KB (the whole send path in
        // one async fn). Boxing keeps `send_message`'s future pointer-sized, so
        // callers embedding it in their own futures (event handlers, spawned
        // tasks) don't inherit those 13 KB per instance; the box is allocated
        // only when a send actually runs.
        Box::pin(self.send_message_impl(
            to,
            &message,
            Some(request_id),
            false,
            false,
            edit,
            extra_nodes,
            stanza_type_override,
        ))
        .await
        .map_err(SendError::from_anyhow)?;
        Ok(result)
    }

    /// Send a status/story update using sender-key encryption.
    ///
    /// Status uses LID addressing (matches `WAWebEncryptAndSendStatusMsg`):
    /// LID recipients pass through, PN recipients are resolved to LID via
    /// `Client::get_lid_pn_entry` (cache-aside), and unresolvable recipients
    /// are skipped silently. The resulting `GroupInfo` carries
    /// `AddressingMode::Lid`; `prepare_group_stanza` signs with `own_lid`
    /// and emits `addressing_mode="lid"` on the stanza. Errors only if no
    /// recipient could be resolved.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.status", level = "debug", skip_all, fields(count = recipients.len()), err(Debug)))]
    pub(crate) async fn send_status_message(
        &self,
        message: wa::Message,
        recipients: &[Jid],
        options: crate::features::status::StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        use wacore::client::context::GroupInfo;
        use wacore_binary::builder::NodeBuilder;

        if recipients.is_empty() {
            return Err(SendError::InvalidRequest(
                "cannot send status with no recipients".into(),
            ));
        }

        // Status posts don't go through send_message_with_options, so count them here.
        let _t = wacore::telemetry::timer(wacore::telemetry::SEND_DURATION);
        wacore::telemetry::send("status");

        let to = Jid::status_broadcast();
        let request_id = self.generate_message_id();

        // Borrow from the held snapshot: no field clones, the Arc keeps it alive.
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let account_info = &device_snapshot.account;
        let own_jid = device_snapshot.pn.as_ref().ok_or(SendError::NotLoggedIn)?;
        // Status is LID-addressed (matches WA Web post-LID-migration). Without
        // a real device LID we can't sign or fan out correctly; refuse rather
        // than silently emit `addressing_mode="lid"` with a PN sender.
        let own_lid = device_snapshot.lid.as_ref().ok_or_else(|| {
            SendError::InvalidRequest(
                "cannot send status: device has no LID yet. Finish pairing / LID \
                 migration before posting status."
                    .into(),
            )
        })?;

        // Fail fast for any JID that isn't a user (PN or LID). Mirrors WA
        // Web's `asUserWidOrThrow` inside `toUserLid`: non-user inputs are a
        // programming bug, not something to silently drop during resolution.
        for jid in recipients {
            if !(jid.is_pn() || jid.is_lid()) {
                return Err(SendError::InvalidRequest(format!(
                    "invalid status recipient {jid}: must be a user JID (PN or LID), \
                     not a group/broadcast/newsletter/hosted/etc."
                )));
            }
        }

        use std::collections::HashMap;
        let mut resolved: Vec<Option<Jid>> = Vec::with_capacity(recipients.len());
        let mut lid_to_pn_map: HashMap<wacore_binary::CompactString, Jid> =
            HashMap::with_capacity(recipients.len() + 1);
        for jid in recipients {
            if let Some(lid_jid) = self.resolve_recipient_to_lid(jid).await {
                if jid.is_pn() {
                    lid_to_pn_map.insert(lid_jid.user.clone(), jid.to_non_ad());
                }
                resolved.push(Some(lid_jid));
            } else {
                resolved.push(None);
            }
        }
        lid_to_pn_map.insert(own_lid.user.clone(), own_jid.to_non_ad());

        let participants = wacore::send::assemble_status_participants(resolved, own_lid)?;
        let mut group_info =
            GroupInfo::with_lid_to_pn_map(participants, AddressingMode::Lid, lid_to_pn_map);

        // One encode feeds retry cache and wire; mci-hoist re-encodes (folded context).
        let shared_content = message
            .message_context_info
            .is_unset()
            .then(|| std::sync::Arc::new(waproto::codec::message_to_vec(&message)));
        self.add_recent_message(&to, &request_id, &message, shared_content.clone())
            .await;

        let device_store_arc = self.persistence_manager.get_device_arc().await;
        let to_str = to.to_string();

        let force_skdm = {
            use wacore::libsignal::store::sender_key_name::SenderKeyName;
            // Sender key name tracks the addressing mode of the group stanza.
            // Since status now uses LID addressing (see send_status_message
            // header), the key is stored under own_lid, matching the address
            // prepare_group_stanza derives internally.
            let sender_address = own_lid.to_protocol_address();
            let sender_key_name = SenderKeyName::from_parts(&to_str, sender_address.as_str());

            let key_exists = self
                .signal_cache
                .get_sender_key(&sender_key_name, &*device_snapshot.backend)
                .await?
                .is_some();

            !key_exists
        };

        let mut store_adapter = self.signal_adapter_from(device_store_arc.clone());
        let mut stores = store_adapter.as_signal_stores();

        // Determine which devices need SKDM using the unified per-device map.
        // Status keeps the prior phash behavior, so we drop the full device set
        // and only use the SKDM-target subset.
        let skdm_target_devices: Option<Vec<Jid>> = if force_skdm {
            None
        } else {
            self.resolve_skdm_targets(&to_str, &group_info, own_lid)
                .await
                .map(|(_all, needs)| needs)
        };

        // prepare_group_stanza and ensure_status_participants both read the
        // participant list and expect self present. Done after SKDM resolution
        // to preserve the prior ordering (resolve ran without self appended).
        let own_status_base = own_lid.to_non_ad();
        if !group_info
            .participants
            .iter()
            .any(|participant| participant.is_same_user_as(&own_status_base))
        {
            group_info.participants.push(own_status_base);
        }

        // `<meta status_setting>` describes the POSTER's privacy on their own
        // status. Reactions go through WA Web's addon path and never visit
        // `WAWebEncryptAndSendStatusMsg`; attaching the meta on a reaction
        // gets the stanza NACK'd with 479 (SmaxInvalid). Revokes also skip it.
        let extra_stanza_nodes = if wacore::send::status_carries_privacy_meta(&message) {
            vec![
                NodeBuilder::new("meta")
                    .attr("status_setting", options.privacy.as_str())
                    .build(),
            ]
        } else {
            vec![]
        };

        let prepared = match wacore::send::prepare_group_stanza(
            &*self.runtime,
            &mut stores,
            self,
            &group_info,
            own_jid,
            own_lid,
            account_info.as_deref(),
            to.clone(),
            &message,
            request_id.clone(),
            force_skdm,
            skdm_target_devices,
            // Status broadcasts keep the prior phash behavior (no full-set/self
            // augmentation) — that path is group-only.
            None,
            None,
            &extra_stanza_nodes,
            shared_content.clone(),
        )
        .await
        {
            Ok(prepared) => prepared,
            Err(e) => {
                if let Some(SignalProtocolError::NoSenderKeyState(_)) =
                    e.downcast_ref::<SignalProtocolError>()
                {
                    log::warn!("No sender key for status broadcast, forcing distribution.");

                    if let Err(e) = self
                        .persistence_manager
                        .clear_sender_key_devices(&to_str)
                        .await
                    {
                        log::warn!(
                            "Failed to clear status SKDM recipients for {}: {:?}",
                            to_str,
                            e
                        );
                    }
                    self.sender_key_device_cache.invalidate(&to_str).await;

                    let mut store_adapter_retry =
                        self.signal_adapter_from(device_store_arc.clone());
                    let mut stores_retry = store_adapter_retry.as_signal_stores();

                    wacore::send::prepare_group_stanza(
                        &*self.runtime,
                        &mut stores_retry,
                        self,
                        &group_info,
                        own_jid,
                        own_lid,
                        account_info.as_deref(),
                        to.clone(),
                        &message,
                        request_id.clone(),
                        true,
                        None,
                        None,
                        None,
                        &extra_stanza_nodes,
                        shared_content.clone(),
                    )
                    .await?
                } else {
                    return Err(e.into());
                }
            }
        };

        let stanza = self
            .ensure_status_participants(prepared.node, &group_info)
            .await?;

        let ack = if let Some(phash) = stanza
            .attrs()
            .optional_string("phash")
            .map(|s| s.into_owned())
        {
            let rx = self.register_ack_waiter(&request_id).await;
            Some((rx, phash))
        } else {
            None
        };

        if let Err(e) = self.send_node(stanza).await {
            if ack.is_some() {
                self.response_waiters.lock().await.remove(&request_id);
            }
            return Err(e.into());
        }

        if let Some((rx, phash)) = ack {
            self.spawn_phash_validation(rx, phash, to.clone(), true, request_id.clone());
        }

        self.update_sender_key_devices(&to_str, &prepared.skdm_devices)
            .await;

        for user in &prepared.stale_device_users {
            self.invalidate_device_cache(user).await;
        }

        self.flush_signal_cache_logged("send_status_message", None)
            .await;

        Ok(SendResult {
            message_id: request_id,
            to,
        })
    }

    /// Resolve the group's device set for a warm/partial send. Returns
    /// `None` when device resolution fails (caller falls back to the full
    /// `force_skdm` path), otherwise `Some((all_devices, needs_skdm))` where
    /// `all_devices` is the complete resolved set (feeds the phash) and
    /// `needs_skdm` is the subset still missing the sender key (feeds SKDM
    /// distribution). `needs_skdm` may be empty (fully warm send).
    ///
    /// For LID mode, uses `group_info.phone_jid_for_lid_user` to query devices
    /// via PN when available (LID usync is unreliable for own JID), then
    /// converts the result back to LID. Same fallback as `prepare_group_stanza`.
    /// Load (or lazily build) the per-group sender-key device map.
    ///
    /// Atomic get-or-init: if another task invalidated the cache during our
    /// DB read, get_or_init's single-flight guarantee means the stale data
    /// won't be inserted — the invalidation wins and the next caller re-inits.
    async fn skdm_device_map(
        &self,
        group_jid: &str,
    ) -> std::sync::Arc<crate::sender_key_device_cache::SenderKeyDeviceMap> {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;
        let pm = self.persistence_manager.clone();
        self.sender_key_device_cache
            .get_or_init(group_jid, async {
                let db_rows = pm
                    .get_sender_key_devices(group_jid)
                    .await
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "Failed to read sender key devices for {}: {:?}",
                            group_jid,
                            e
                        );
                        vec![]
                    });
                std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&db_rows))
            })
            .await
    }

    /// Filter the resolved device set down to the subset still needing SKDM.
    ///
    /// No empty-cache early-exit: WA Web iterates an empty `senderKey` Map
    /// as `false` per participant, so the filter must run unconditionally.
    fn filter_skdm_targets(
        &self,
        group_jid: &str,
        all_devices: &[Jid],
        cached_map: &crate::sender_key_device_cache::SenderKeyDeviceMap,
        own_sending_jid: &Jid,
    ) -> Vec<Jid> {
        let needs_skdm: Vec<Jid> = all_devices
            .iter()
            .filter(|device| {
                if device.is_hosted() {
                    return false;
                }
                if device.user == own_sending_jid.user && device.device == own_sending_jid.device {
                    return false;
                }
                // WA Web parity (ParticipantStore.js skDistribList): a device is
                // warm only when it AND its primary (device 0) hold the key, so a
                // forgotten primary redistributes the whole user while a forgotten
                // companion redistributes only itself. One inner-map resolution
                // per device (single user-string hash) instead of two.
                !cached_map.device_and_primary_warm(&device.user, device.device)
            })
            .cloned()
            .collect();

        log::debug!(
            "Resolved {} devices ({} need SKDM) for {}",
            all_devices.len(),
            needs_skdm.len(),
            group_jid
        );
        needs_skdm
    }

    /// SKDM target resolution for the status path, whose `GroupInfo` is built
    /// fresh per send (no stable identity to memoize against).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.resolve_skdm_targets", level = "debug", skip_all, fields(group = %wacore_binary::jid::observe_str(group_jid))))]
    async fn resolve_skdm_targets(
        &self,
        group_jid: &str,
        group_info: &wacore::client::context::GroupInfo,
        own_sending_jid: &Jid,
    ) -> Option<(std::sync::Arc<wacore::send::ResolvedGroupDevices>, Vec<Jid>)> {
        let cached_map = self.skdm_device_map(group_jid).await;

        let is_lid_mode = group_info.addressing_mode == wacore::types::message::AddressingMode::Lid;
        let jids_to_resolve: Vec<Jid> = group_info
            .participants
            .iter()
            .map(|jid| {
                if is_lid_mode
                    && jid.is_lid()
                    && let Some(pn) = group_info.phone_jid_for_lid_user(&jid.user)
                {
                    return pn.to_non_ad();
                }
                jid.to_non_ad()
            })
            .collect();

        match SendContextResolver::resolve_devices(self, &jids_to_resolve).await {
            Ok(all_devices) => {
                let all_devices: Vec<Jid> = if is_lid_mode {
                    all_devices
                        .into_iter()
                        .map(|d| group_info.phone_device_jid_into_lid(d))
                        .collect()
                } else {
                    all_devices
                };
                let all_devices =
                    std::sync::Arc::new(wacore::send::ResolvedGroupDevices::new(all_devices));
                let needs_skdm = self.filter_skdm_targets(
                    group_jid,
                    all_devices.devices(),
                    &cached_map,
                    own_sending_jid,
                );
                Some((all_devices, needs_skdm))
            }
            Err(e) => {
                log::warn!(
                    "Failed to resolve devices for SKDM check in {}: {:?}",
                    group_jid,
                    e
                );
                None
            }
        }
    }

    /// SKDM target resolution for cached-group sends: the full device set
    /// comes from the per-group memo (`resolve_group_devices_memoized`), so a
    /// warm repeat send skips the per-member registry fan-out entirely.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.resolve_skdm_targets_memoized", level = "debug", skip_all, fields(group = %group_jid)))]
    async fn resolve_skdm_targets_memoized(
        &self,
        group: &Jid,
        group_jid: &str,
        group_info: &std::sync::Arc<wacore::client::context::GroupInfo>,
        own_sending_jid: &Jid,
    ) -> Option<(std::sync::Arc<wacore::send::ResolvedGroupDevices>, Vec<Jid>)> {
        let cached_map = self.skdm_device_map(group_jid).await;
        match self
            .resolve_group_devices_memoized(group, group_info, own_sending_jid)
            .await
        {
            Ok(all_devices) => {
                // Skip the O(devices) filter_skdm_targets scan when the same
                // (devices, sender-key-map) Arc pair was already fully warm. Both
                // Arcs swap on any warm-state or membership change, so a stale skip
                // is impossible. Needs the device memo for a stable devices Arc.
                if self.group_devices_memo_enabled
                    && let Some((dw, cw)) = self.skdm_warm_memo.get(group).await
                    && std::ptr::eq(dw.as_ptr(), std::sync::Arc::as_ptr(&all_devices))
                    && std::ptr::eq(cw.as_ptr(), std::sync::Arc::as_ptr(&cached_map))
                {
                    return Some((all_devices, Vec::new()));
                }
                let needs_skdm = self.filter_skdm_targets(
                    group_jid,
                    all_devices.devices(),
                    &cached_map,
                    own_sending_jid,
                );
                if needs_skdm.is_empty() && self.group_devices_memo_enabled {
                    self.skdm_warm_memo
                        .insert(
                            group.clone(),
                            (
                                std::sync::Arc::downgrade(&all_devices),
                                std::sync::Arc::downgrade(&cached_map),
                            ),
                        )
                        .await;
                }
                Some((all_devices, needs_skdm))
            }
            Err(e) => {
                log::warn!(
                    "Failed to resolve devices for SKDM check in {}: {:?}",
                    group_jid,
                    e
                );
                None
            }
        }
    }

    /// Update sender key device tracking after a successful group/status send.
    ///
    /// Called AFTER `send_node()` succeeds (WA Web: `markHasSenderKey` after server ACK).
    /// On full distribution, clears old state and marks the provided device list.
    /// On partial, marks only the specific SKDM recipients.
    ///
    /// The `all_resolved_devices` parameter carries the exact device list resolved
    /// for the stanza, avoiding a redundant `resolve_devices` call and preventing
    /// the clear-then-fail race where a transient resolver failure leaves the map empty.
    /// Mark devices as `has_key=true` after successful SKDM distribution.
    async fn update_sender_key_devices(&self, group_jid: &str, devices: &[Jid]) {
        if devices.is_empty() {
            return;
        }

        if let Err(e) = self
            .set_sender_key_status_for_devices(group_jid, devices, true, false)
            .await
        {
            log::warn!(
                "Failed to update sender key devices for {}: {:?}",
                group_jid,
                e
            );
        }
        self.sender_key_device_cache.invalidate(group_jid).await;
    }

    /// Spawn a background task to validate phash from server ack.
    /// On mismatch, invalidates sender key device cache and group info cache.
    fn spawn_phash_validation(
        &self,
        rx: futures::channel::oneshot::Receiver<std::sync::Arc<wacore_binary::OwnedNodeRef>>,
        our_phash: String,
        jid: Jid,
        invalidate_group_cache: bool,
        message_id: String,
    ) {
        let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) else {
            return;
        };
        self.runtime
            .spawn(Box::pin(async move {
                let ack = match wacore::runtime::timeout(
                    &*client.runtime,
                    std::time::Duration::from_secs(10),
                    rx,
                )
                .await
                {
                    Ok(Ok(node)) => node,
                    _ => {
                        // Remove leaked waiter to prevent keepalive suppression
                        client.response_waiters.lock().await.remove(&message_id);
                        return;
                    }
                };
                // Cold path: box the heavy mismatch handler so the common
                // (phash matches) spawned future stays small instead of carrying
                // all the invalidation/clear awaits inline.
                if let Some(server) = ack.get().get_attr("phash").map(|v| v.as_str())
                    && server != our_phash
                {
                    Box::pin(client.handle_phash_mismatch(
                        &jid,
                        &our_phash,
                        &server,
                        invalidate_group_cache,
                    ))
                    .await;
                }
            }))
            .detach();
    }

    /// Cold path of [`spawn_phash_validation`](Self::spawn_phash_validation): the
    /// server's phash disagreed with ours, so invalidate the relevant
    /// device/group caches and (for groups) force sender-key redistribution.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.phash_mismatch", level = "debug", skip_all, fields(jid = %jid.observe())))]
    async fn handle_phash_mismatch(
        &self,
        jid: &Jid,
        our_phash: &str,
        server_phash: &str,
        invalidate_group_cache: bool,
    ) {
        log::warn!(
            "Phash mismatch for {}: ours={our_phash}, server={server_phash}. Invalidating caches.",
            jid.observe()
        );
        // DM phash covers both recipient + own devices
        // (WA Web: syncDeviceListJob([recipient, me]))
        if !jid.is_group() && !jid.is_status_broadcast() {
            self.invalidate_device_cache(&jid.user).await;
            if let Some(own_pn) = &self.persistence_manager.get_device_snapshot().pn {
                self.invalidate_device_cache(&own_pn.user).await;
            }
        }
        let jid_str = jid.to_string();
        // Cache-only invalidation re-reads the same stale rows on the next send.
        // Drop the persisted state too so the next send takes the full-
        // distribution path. If the clear fails, fall back to deleting the bot's
        // own sender key for the chat — the next send will see `!key_exists` and
        // force_skdm without depending on the tracker.
        if jid.is_group() || jid.is_status_broadcast() {
            let cleared = self
                .persistence_manager
                .clear_sender_key_devices(&jid_str)
                .await;
            if let Err(e) = cleared {
                log::warn!(
                    "phash mismatch: clear_sender_key_devices failed: {e} — \
                     deleting own sender key as fallback to force redistribution"
                );
                use wacore::libsignal::store::sender_key_name::SenderKeyName;
                use wacore::types::jid::JidExt;
                let snapshot = self.persistence_manager.get_device_snapshot();
                for own in snapshot.lid.iter().chain(snapshot.pn.iter()) {
                    let sk =
                        SenderKeyName::from_parts(&jid_str, own.to_protocol_address().as_str());
                    self.signal_cache.delete_sender_key(sk.cache_key()).await;
                }
                let _ = self
                    .flush_signal_cache_logged("phash-mismatch-fallback", None)
                    .await;
            }
        }
        self.sender_key_device_cache.invalidate(&jid_str).await;
        if invalidate_group_cache {
            self.get_group_cache().await.invalidate(jid).await;
        }
    }

    /// Ensure the status stanza has a <participants> node listing all recipient
    /// user JIDs. WhatsApp Web's `participantList` uses bare USER JIDs (not
    /// device JIDs) — `<to jid="user@s.whatsapp.net"/>` — to tell the server
    /// which users should receive the skmsg. The SKDM distribution list
    /// (already in <participants>) uses device JIDs with <enc> children.
    async fn ensure_status_participants(
        &self,
        stanza: wacore_binary::Node,
        group_info: &wacore::client::context::GroupInfo,
    ) -> Result<wacore_binary::Node, anyhow::Error> {
        Ok(wacore::send::ensure_status_participants(stanza, group_info))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.impl", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn send_message_impl(
        &self,
        to: Jid,
        message: &wa::Message,
        request_id_override: Option<String>,
        peer: bool,
        force_key_distribution: bool,
        edit: Option<crate::types::message::EditAttribute>,
        extra_stanza_nodes: Vec<Node>,
        stanza_type_override: Option<StanzaType>,
    ) -> Result<(), anyhow::Error> {
        // Newsletters are plaintext channels and never use the E2E path. Text
        // sends go through the <plaintext> branch in send_message_with_options;
        // edit/revoke have dedicated plaintext methods (newsletter().edit_message
        // / revoke_message). A newsletter JID here is a mis-routed pin/edit/revoke
        // (pin is not a channel op), so reject it.
        if to.is_newsletter() {
            return Err(SendError::InvalidRequest(
                "newsletter JIDs are not valid on the E2E send path; use \
                 newsletter().edit_message/revoke_message (pin is unsupported on channels)"
                    .into(),
            )
            .into());
        }

        // status@broadcast reactions fan out pairwise to the author's devices;
        // status posts keep going through send_status_message (owns recipients).
        let (to, is_status_addon) = if to.is_status_broadcast() {
            let author = message
                .reaction_message
                .as_option()
                .and_then(|rm| rm.key.as_option())
                .and_then(|k| k.participant.as_ref())
                .and_then(|p| p.parse::<Jid>().ok())
                .filter(|jid| jid.is_pn() || jid.is_lid())
                .ok_or_else(|| {
                    SendError::InvalidRequest(
                        "send_message to status@broadcast requires \
                         reaction_message.key.participant = status author (user JID). \
                         Use client.status() for posting new statuses."
                            .into(),
                    )
                })?;
            (author, true)
        } else {
            (to, false)
        };

        // Generate request ID early (doesn't need lock)
        let request_id = match request_id_override {
            Some(id) => id,
            None => self.generate_message_id(),
        };
        // `request_id` is moved into the branch-specific stanza builders below;
        // keep a copy for the post-send messageSecret persistence (the secret
        // itself is generated inside prepare_dm/group_stanza, not on `message`,
        // so it's threaded back out via PreparedStanza.message_secret below).
        let outbound_id_clone = request_id.clone();
        let mut outbound_msg_secret: Option<[u8; 32]> = None;
        // Group prepares pick LID or PN based on group addressing_mode;
        // capture it so the persisted secret keys match what
        // `<meta target_sender_jid>` echoes back. For DMs we resolve from
        // chat.server (LID for bot, PN otherwise) after send_node succeeds.
        let mut outbound_group_sender_identity: Option<Jid> = None;

        // SKDM update data — only populated for group sends, deferred until after send_node().
        // This matches WhatsApp Web which only calls markHasSenderKey() after server ACK.
        struct SkdmUpdate {
            to_str: String,
            devices: Vec<Jid>,
            stale_users: Vec<String>,
        }
        let mut skdm_update: Option<SkdmUpdate> = None;
        // Single-flight for cold group sends: held from SKDM target resolution
        // through `update_sender_key_devices` (bottom of this function) so a
        // concurrent cold send re-resolves against the winner's warm marking
        // instead of redoing the full per-member fan-out. None on warm sends.
        let mut distribution_guard: Option<async_lock::MutexGuardArc<()>> = None;
        let mut should_issue_tc_token_after_send = false;
        let mut used_cached_tc_token_key: Option<String> = None;
        let tc_issue_target = to.clone();

        let mut dm_phash: Option<String> = None;
        let stanza_to_send: wacore_binary::Node = if peer && !to.is_group() {
            // Peer messages are only valid for individual users, not groups
            // Resolve encryption JID and acquire lock ONLY for encryption
            let encryption_jid = self.resolve_encryption_jid(&to).await;
            let signal_addr = encryption_jid.to_protocol_address();

            let session_mutex = self.session_lock_for(signal_addr.as_str()).await;
            let _session_guard = session_mutex.lock().await;

            let mut store_adapter = self.signal_adapter().await;

            let device_snapshot = self.persistence_manager.get_device_snapshot();
            wacore::send::prepare_peer_stanza(
                &mut store_adapter.session_store,
                &mut store_adapter.identity_store,
                to,
                &signal_addr,
                message,
                request_id,
                device_snapshot.account.as_deref(),
            )
            .await?
        } else if to.is_group() {
            // No send-level lock: encrypt_group_message serializes the
            // sender-key chain advance per (group, sender) at the cipher.
            let group_info = self.groups().query_info(&to).await?;

            // Borrow from the held snapshot: no field clones, the Arc keeps it alive.
            let device_snapshot = self.persistence_manager.get_device_snapshot();
            let account_info = &device_snapshot.account;
            let own_jid = device_snapshot
                .pn
                .as_ref()
                .ok_or(crate::client::ClientError::NotLoggedIn)?;
            let own_lid = device_snapshot
                .lid
                .as_ref()
                .ok_or_else(|| anyhow!("LID not set, cannot send to group"))?;

            // One encode feeds retry cache and wire; mci-hoist re-encodes (folded context).
            let shared_content = message
                .message_context_info
                .is_unset()
                .then(|| std::sync::Arc::new(waproto::codec::message_to_vec(message)));
            // Store serialized message bytes for retry (lightweight)
            self.add_recent_message(&to, &request_id, message, shared_content.clone())
                .await;

            let device_store_arc = self.persistence_manager.get_device_arc().await;
            let to_str = to.to_string();

            let (own_sending_jid, _) = match group_info.addressing_mode {
                crate::types::message::AddressingMode::Lid => (own_lid.clone(), "lid"),
                crate::types::message::AddressingMode::Pn => (own_jid.clone(), "pn"),
            };

            // Memo identity must be the CACHED Arc: ensure_self_in_group clones
            // a fresh GroupInfo whenever self is absent from the snapshot, which
            // would make the memo miss on every send to such groups. The memoized
            // resolver applies the same self-append internally.
            let group_info_for_memo = std::sync::Arc::clone(&group_info);
            // resolve_skdm_targets and prepare_group_stanza both read the
            // participant list and expect self to be present.
            let group_info = ensure_self_in_group(group_info, &own_sending_jid);

            // Side-effect-free cold check: does the sender key record exist,
            // and has its chain advanced past the rotation threshold? Reads
            // the record without deleting anything, so a false positive (a
            // concurrent send already rotating/recreating) costs only the
            // re-check under the lock below.
            use wacore::libsignal::store::sender_key_name::SenderKeyName;
            let sender_address = own_sending_jid.to_protocol_address();
            let sender_key_name = SenderKeyName::from_parts(&to_str, sender_address.as_str());
            // WA Web posts SenderKeyExpired with `PERIODIC_ROTATION` after
            // a chain advances past a threshold. Captured-js doesn't show
            // the value; 1000 mirrors common Signal hygiene defaults.
            const SENDER_KEY_ROTATION_THRESHOLD: u32 = 1000;
            let read_sender_key_state = || async {
                let record = self
                    .signal_cache
                    .get_sender_key(&sender_key_name, &*device_snapshot.backend)
                    .await?;
                let key_exists = record.is_some();
                // Read the chain iteration through the shared `Arc` without cloning
                // the record: borrow the current state instead of `*_mut().cloned()`.
                let needs_rotation = record
                    .as_ref()
                    .and_then(|r| r.sender_key_state().ok())
                    .and_then(|state| state.sender_chain_key())
                    .map(|ck| ck.iteration())
                    .is_some_and(|iter| iter >= SENDER_KEY_ROTATION_THRESHOLD);
                Ok::<(bool, bool), anyhow::Error>((key_exists, needs_rotation))
            };

            let (key_exists, needs_rotation) = read_sender_key_state().await?;
            let mut force_skdm = force_key_distribution || !key_exists || needs_rotation;
            if force_skdm {
                // Serialize the whole rotation/redistribution under the
                // per-group guard and RE-CHECK once inside it: a send that
                // merely raced the winner's delete->recreate window sees the
                // fresh record here and downgrades to a warm send instead of
                // redistributing to every member again.
                distribution_guard = Some(self.group_distribution_lock(&to).await);
                let (key_exists, needs_rotation) = read_sender_key_state().await?;
                force_skdm = force_key_distribution || !key_exists || needs_rotation;
                if needs_rotation {
                    log::info!(
                        "Periodic sender-key rotation for {} (chain iteration >= {SENDER_KEY_ROTATION_THRESHOLD})",
                        to.observe()
                    );
                    self.signal_cache
                        .delete_sender_key(sender_key_name.cache_key())
                        .await;
                    if let Err(e) = self
                        .persistence_manager
                        .clear_sender_key_devices(&to_str)
                        .await
                    {
                        log::warn!("periodic rotation: clear_sender_key_devices failed: {e}");
                    }
                    self.sender_key_device_cache.invalidate(&to_str).await;
                }
                if !force_skdm {
                    distribution_guard = None;
                }
            }

            let mut store_adapter = self.signal_adapter_from(device_store_arc.clone());

            let mut stores = store_adapter.as_signal_stores();

            // Determine which devices need SKDM distribution using the unified
            // per-device sender key map (matches WA Web's participant.senderKey Map).
            // `all_devices_for_phash` carries the FULL resolved set so the phash
            // covers every device + self even on a warm send (WA Web sends a
            // phash on every group send); `skdm_target_devices` is the subset
            // still missing the key. On the cold/`force_skdm` path both are
            // `None` and `prepare_group_stanza` resolves the set itself.
            let (all_devices_for_phash, skdm_target_devices): (
                Option<std::sync::Arc<wacore::send::ResolvedGroupDevices>>,
                Option<Vec<Jid>>,
            ) = if force_skdm {
                (None, None)
            } else {
                match self
                    .resolve_skdm_targets_memoized(
                        &to,
                        &to_str,
                        &group_info_for_memo,
                        &own_sending_jid,
                    )
                    .await
                {
                    Some((all, needs)) if needs.is_empty() => (Some(all), Some(needs)),
                    Some((first_all, first_needs)) => {
                        // Cold: wait for any in-flight distribution, then
                        // re-resolve. The loser usually finds every device
                        // already marked warm by the winner and downgrades to a
                        // plain skmsg send; if the winner failed, the targets
                        // are still cold and this send distributes normally.
                        distribution_guard = Some(self.group_distribution_lock(&to).await);
                        // Force a DB re-read: a concurrent warm send may have
                        // started the cache init before the winner's marking
                        // landed and then published that stale (empty) map,
                        // which would otherwise turn this into a full
                        // re-distribution to every member.
                        self.sender_key_device_cache.invalidate(&to_str).await;
                        match self
                            .resolve_skdm_targets_memoized(
                                &to,
                                &to_str,
                                &group_info_for_memo,
                                &own_sending_jid,
                            )
                            .await
                        {
                            Some((all, needs)) => {
                                if needs.is_empty() {
                                    distribution_guard = None;
                                }
                                (Some(all), Some(needs))
                            }
                            // Transient re-resolve failure: keep the first
                            // resolve's targets rather than silently sending
                            // without the distribution it already knew was
                            // needed.
                            None => (Some(first_all), Some(first_needs)),
                        }
                    }
                    None => (None, None),
                }
            };

            match wacore::send::prepare_group_stanza(
                &*self.runtime,
                &mut stores,
                self,
                &group_info,
                own_jid,
                own_lid,
                account_info.as_deref(),
                to.clone(),
                message,
                request_id.clone(),
                force_skdm,
                skdm_target_devices,
                all_devices_for_phash,
                edit.clone(),
                &extra_stanza_nodes,
                shared_content.clone(),
            )
            .await
            {
                Ok(prepared) => {
                    skdm_update = Some(SkdmUpdate {
                        to_str: to_str.clone(),
                        devices: prepared.skdm_devices,
                        stale_users: prepared.stale_device_users,
                    });
                    outbound_msg_secret = prepared.message_secret;
                    outbound_group_sender_identity = Some(prepared.sender_identity);
                    prepared.node
                }
                Err(e) => {
                    if let Some(SignalProtocolError::NoSenderKeyState(_)) =
                        e.downcast_ref::<SignalProtocolError>()
                    {
                        log::warn!(
                            "No sender key for group {}, forcing distribution.",
                            to.observe()
                        );

                        // This retry redistributes, so it needs the same
                        // single-flight guard as a cold send (a warm send that
                        // lost its sender key arrives here without one).
                        if distribution_guard.is_none() {
                            distribution_guard = Some(self.group_distribution_lock(&to).await);
                        }

                        // Re-check under the guard: a concurrent retry may have
                        // already recreated the key and marked the devices, in
                        // which case this send retries warm instead of clearing
                        // the tracking and redistributing to every member again.
                        let (key_recreated, _) = read_sender_key_state().await?;
                        let warm_targets = if key_recreated {
                            self.sender_key_device_cache.invalidate(&to_str).await;
                            self.resolve_skdm_targets_memoized(
                                &to,
                                &to_str,
                                &group_info_for_memo,
                                &own_sending_jid,
                            )
                            .await
                        } else {
                            None
                        };
                        let (retry_force, retry_targets, retry_all) = match warm_targets {
                            Some((all, needs)) => (false, Some(needs), Some(all)),
                            None => {
                                if let Err(e) = self
                                    .persistence_manager
                                    .clear_sender_key_devices(&to_str)
                                    .await
                                {
                                    log::warn!("Failed to clear SKDM recipients: {:?}", e);
                                }
                                self.sender_key_device_cache.invalidate(&to_str).await;
                                (true, None, None)
                            }
                        };

                        let mut store_adapter_retry =
                            self.signal_adapter_from(device_store_arc.clone());
                        let mut stores_retry = store_adapter_retry.as_signal_stores();

                        let retry_prepared = wacore::send::prepare_group_stanza(
                            &*self.runtime,
                            &mut stores_retry,
                            self,
                            &group_info,
                            own_jid,
                            own_lid,
                            account_info.as_deref(),
                            to,
                            message,
                            request_id,
                            retry_force,
                            retry_targets,
                            retry_all,
                            edit.clone(),
                            &extra_stanza_nodes,
                            shared_content.clone(),
                        )
                        .await?;

                        skdm_update = Some(SkdmUpdate {
                            to_str,
                            devices: retry_prepared.skdm_devices,
                            stale_users: retry_prepared.stale_device_users,
                        });
                        outbound_msg_secret = retry_prepared.message_secret;
                        outbound_group_sender_identity = Some(retry_prepared.sender_identity);
                        retry_prepared.node
                    } else {
                        return Err(e);
                    }
                }
            }
        } else {
            // Per-device locking to match decrypt path (message.rs:684),
            // preventing ratchet desync on concurrent send/receive.

            // One encode feeds retry cache and wire; mci-hoist re-encodes (folded context).
            let shared_content = message
                .message_context_info
                .is_unset()
                .then(|| std::sync::Arc::new(waproto::codec::message_to_vec(message)));
            // Status reaction retries arrive with `from=status@broadcast`;
            // cache under the broadcast chat so take_recent_message hits.
            if is_status_addon {
                self.add_recent_message(
                    &Jid::status_broadcast(),
                    &request_id,
                    message,
                    shared_content.clone(),
                )
                .await;
            } else {
                self.add_recent_message(&to, &request_id, message, shared_content.clone())
                    .await;
            }

            let device_snapshot = self.persistence_manager.get_device_snapshot();
            let own_jid = device_snapshot
                .pn
                .as_ref()
                .ok_or(crate::client::ClientError::NotLoggedIn)?;

            // PN→LID mapping (WA Web: ManagePhoneNumberMappingJob)
            if to.is_pn() && self.lid_pn_cache.get_current_lid(&to.user).await.is_none() {
                let sid = self.generate_request_id();
                let spec = wacore::iq::usync::LidQuerySpec::new(vec![to.to_non_ad()], sid);
                // Best-effort: WA Web also catches and warns on failure
                match self.execute(spec).await {
                    Ok(resp) => {
                        for mapping in &resp.lid_mappings {
                            if let Err(e) = self
                                .add_lid_pn_mapping(
                                    &mapping.lid,
                                    &mapping.phone_number,
                                    crate::lid_pn_cache::LearningSource::Usync,
                                )
                                .await
                            {
                                log::warn!(
                                    "Failed to persist LID mapping {} -> {}: {e:?}",
                                    mapping.phone_number,
                                    mapping.lid
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::warn!(
                            "LID query failed for {}, falling back to PN: {e:?}",
                            to.observe()
                        );
                    }
                }
            }

            // DM fanout: all known recipient devices + own companions.
            // WAWebSendUserMsgJob reads local device table only on the send
            // path; WAWebDBDeviceListFanout excludes hosted devices.
            // The LID-vs-PN wire namespace is an account-level decision: the
            // server 400-nacks LID-addressed DMs from accounts that are not
            // 1:1-LID-migrated (issue #941).
            let recipient_bare = self.resolve_dm_wire_jid(&to).await;
            let recipient_is_lid = recipient_bare.is_lid();

            let stanza_to = dm_stanza_to(&recipient_bare, &to);

            // Local registry first; network warm only on miss to avoid
            // unnecessary LID-migration side effects from get_user_devices
            let mut recipient_cached = self.get_devices_from_registry(&recipient_bare).await;
            if recipient_cached.is_none() {
                let _ = self.get_user_devices(std::slice::from_ref(&to)).await;
                recipient_cached = self.get_devices_from_registry(&recipient_bare).await;
            }

            let is_self_dm =
                is_self_dm_recipient(&recipient_bare, own_jid, device_snapshot.lid.as_ref());

            // Skip the own-device lookup only when we already have the
            // recipient's list — that record covers every own device in a
            // single namespace. If `recipient_cached` is `None` (cache miss
            // + warmup failed), the PN-keyed `own_cached` is the only thing
            // standing between us and a bare-JID fallback that would drop
            // companion devices.
            let own_cached: Option<Vec<Jid>> = if is_self_dm && recipient_cached.is_some() {
                None
            } else {
                let mut cached = self.get_devices_from_registry(own_jid).await;
                if cached.is_none() {
                    let _ = self.get_user_devices(std::slice::from_ref(own_jid)).await;
                    cached = self.get_devices_from_registry(own_jid).await;
                }
                cached
            };

            // Build device list, filter hosted in-place, reuse Vecs
            let mut all_dm_jids = match recipient_cached {
                Some(mut devices) => {
                    devices.retain(|j| !j.is_hosted());
                    devices
                }
                // No record at all — bare JID, server handles fanout
                None => vec![recipient_bare],
            };

            if let Some(mut own_devices) = own_cached {
                own_devices.retain(|j| !j.is_hosted());
                all_dm_jids.append(&mut own_devices);
            }

            // Exclude exact sender device (WA Web: isMeDevice in getFanOutList)
            // so ensure_e2e_sessions never creates a self-session
            let own_lid = device_snapshot.lid.as_ref();
            all_dm_jids.retain(|j| {
                let is_sender = (j.is_same_user_as(own_jid) && j.device == own_jid.device)
                    || own_lid.is_some_and(|lid| j.is_same_user_as(lid) && j.device == lid.device);
                !is_sender
            });

            // own_cached is keyed by the bot's PN, so own devices come back
            // PN-addressed. The server rejects a stanza that mixes PN and LID
            // participants, so align own devices to LID for a LID recipient
            // (whatsmeow switches ownID to LID before fanout).
            if recipient_is_lid {
                let lid = own_lid.ok_or_else(|| {
                    anyhow!("Cannot send a LID-addressed DM before the device LID is known")
                })?;
                for j in all_dm_jids.iter_mut() {
                    if j.is_pn() && j.is_same_user_as(own_jid) {
                        *j = Jid::lid_device(lid.user.clone(), j.device);
                    }
                }
            }

            // Same-namespace dedup only; cross-namespace overlap is avoided
            // upstream via `is_self_dm_recipient`.
            wacore::types::jid::sort_dedup_by_device(&mut all_dm_jids);

            self.ensure_e2e_sessions(&all_dm_jids).await?;

            let mut extra_stanza_nodes = extra_stanza_nodes;
            // tctoken applies to 1:1 chats; status reactions share the fanout
            // path but WA Web does not attach tctokens to them.
            if !to.is_group() && !to.is_newsletter() && !is_status_addon {
                let (should_issue_after_send, cached_token_key) = self
                    .maybe_include_tc_token(&to, &mut extra_stanza_nodes)
                    .await;
                should_issue_tc_token_after_send = should_issue_after_send;
                if should_issue_after_send {
                    used_cached_tc_token_key = cached_token_key;
                }
            }
            if should_issue_tc_token_after_send {
                debug!(target: "Client/TcToken", "Scheduled tc token issuance after send for {}", to.observe());
            }

            let lock_jids = self.build_session_lock_keys(&all_dm_jids).await;
            let _session_mutexes = self.session_mutexes_for(&lock_jids).await;
            let mut _session_guards = Vec::with_capacity(_session_mutexes.len());
            for mutex in &_session_mutexes {
                _session_guards.push(mutex.lock().await);
            }

            let mut store_adapter = self.signal_adapter().await;

            let mut stores = store_adapter.as_signal_stores();

            let prepared = wacore::send::prepare_dm_stanza(
                &*self.runtime,
                &mut stores,
                self,
                own_jid,
                device_snapshot.lid.as_ref(),
                device_snapshot.account.as_deref(),
                stanza_to,
                message,
                request_id,
                edit,
                &extra_stanza_nodes,
                all_dm_jids,
                shared_content,
            )
            .await?;
            dm_phash = prepared.phash;
            outbound_msg_secret = prepared.message_secret;
            prepared.node
        };

        let ack = if let Some(phash) = dm_phash
            && let Some(msg_id) = stanza_to_send
                .attrs()
                .optional_string("id")
                .map(|s| s.into_owned())
        {
            let rx = self.register_ack_waiter(&msg_id).await;
            Some((rx, phash, msg_id))
        } else {
            None
        };

        // Server expects the outer `to` as the broadcast chat even though
        // encryption targeted the author's devices (mirrors incoming `from`).
        let mut stanza_to_send = stanza_to_send;
        if is_status_addon {
            stanza_to_send.attrs.insert("to", Jid::status_broadcast());
        }
        if let Some(t) = stanza_type_override {
            stanza_to_send.attrs.insert("type", t.as_wire());
        }

        if let Err(e) = self.send_node(stanza_to_send).await {
            if let Some((_, _, ref msg_id)) = ack {
                self.response_waiters.lock().await.remove(msg_id);
            }
            return Err(e.into());
        }

        if let Some(secret) = outbound_msg_secret.as_ref() {
            let sender = match outbound_group_sender_identity {
                Some(s) => Some(s),
                None => self.dm_sender_identity_for(&tc_issue_target).await,
            };
            if let Some(sender) = sender {
                let is_bot_chat = tc_issue_target.is_bot();
                let class = wacore::msg_secret::classify(message, is_bot_chat);
                self.persist_outbound_msg_secret(
                    &tc_issue_target,
                    &sender,
                    &outbound_id_clone,
                    secret,
                    class,
                )
                .await;
            }
        }

        if let Some((rx, phash, msg_id)) = ack {
            // Group sends also invalidate group cache on mismatch — server's
            // participant set diverged, the next send needs a fresh query.
            let invalidate_group = tc_issue_target.is_group();
            self.spawn_phash_validation(
                rx,
                phash,
                tc_issue_target.clone(),
                invalidate_group,
                msg_id,
            );
        }

        if let Some(update) = skdm_update {
            self.update_sender_key_devices(&update.to_str, &update.devices)
                .await;
            for user in &update.stale_users {
                self.invalidate_device_cache(user).await;
            }
        }
        // Warm marking is visible; a waiting cold send may now re-resolve.
        drop(distribution_guard);

        // Flush cached Signal state to DB after encryption
        self.flush_signal_cache_logged("send_message_impl", None)
            .await;

        // Issue new tc token after send if a bucket boundary was crossed.
        // Fire-and-forget so send_message returns without waiting for the IQ
        if should_issue_tc_token_after_send {
            if let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) {
                let target = tc_issue_target;
                let cached_key = used_cached_tc_token_key;
                self.runtime
                    .spawn(Box::pin(async move {
                        let issued_ok = client.issue_tc_token_after_send(&target).await;
                        if issued_ok && let Some(token_key) = cached_key {
                            client.mark_tc_token_used_after_send(&token_key).await;
                        }
                    }))
                    .detach();
            } else {
                log::debug!(target: "Client/TcToken", "Skipping fire-and-forget issuance: client dropped");
            }
        }

        Ok(())
    }

    /// Persist a generated `MessageContextInfo.message_secret` keyed by
    /// `(chat_non_ad, sender_non_ad, msg_id)`. The sender identity must
    /// match what `<meta target_sender_jid>` echoes back at GET time —
    /// LID for bot chats and LID-mode groups, PN otherwise.
    pub(crate) async fn persist_outbound_msg_secret(
        &self,
        chat: &Jid,
        sender: &Jid,
        msg_id: &str,
        secret: &[u8; wacore::reporting_token::MESSAGE_SECRET_SIZE],
        class: wacore::msg_secret::RetentionClass,
    ) {
        let policy = self.cache_config.msg_secret_policy;
        if !policy.persists() {
            return;
        }
        // BotOnly keeps only bot-context secrets; a group message that invokes a
        // bot classifies as Bot, so its reply can still be decrypted.
        if policy.bot_only() && class != wacore::msg_secret::RetentionClass::Bot {
            return;
        }
        // Outbound secrets are minted "now", so the parent event time is the
        // current clock.
        let now = wacore::time::now_secs();
        let expires_at = wacore::msg_secret::expires_at(
            policy,
            &self.cache_config.msg_secret_retention,
            class,
            u64::try_from(now).ok(),
            now,
        );
        let entry = wacore::store::traits::MsgSecretEntry {
            chat: chat.to_non_ad_string(),
            sender: sender.to_non_ad_string(),
            msg_id: msg_id.to_string(),
            secret: secret.to_vec(),
            expires_at,
            message_ts: now,
        };
        // Same write-behind buffer as inbound captures: visible immediately,
        // flushed off the send path (msmsg replies read buffer-first).
        self.msg_secret_buffer.queue(vec![entry]).await;
    }

    /// Decide the identity (LID vs PN) under which an outbound DM's
    /// `messageSecret` should be persisted. Group sends should use
    /// `PreparedGroupStanza.sender_identity` directly instead of this.
    pub(crate) async fn dm_sender_identity_for(&self, to: &Jid) -> Option<Jid> {
        if to.server == wacore_binary::Server::Bot {
            self.get_lid()
        } else {
            self.get_pn()
        }
    }

    /// Build sorted, deduplicated per-device session lock keys.
    /// INVARIANT: Keys are sorted to prevent deadlocks when acquiring multiple
    /// session locks (e.g. DM sends that encrypt for recipient + own devices).
    /// Resolve encryption JIDs and sort for deadlock-free lock acquisition.
    pub(crate) async fn build_session_lock_keys(&self, device_jids: &[Jid]) -> Vec<Jid> {
        let mut keys: Vec<Jid> = Vec::with_capacity(device_jids.len());
        for jid in device_jids {
            keys.push(self.resolve_encryption_jid(jid).await);
        }
        keys.sort_unstable_by(wacore::types::jid::cmp_for_lock_order);
        keys.dedup_by(|a, b| wacore::types::jid::cmp_for_lock_order(a, b).is_eq());
        keys
    }

    /// Fetch per-device session mutexes in deadlock-free order.
    pub(crate) async fn session_mutexes_for(
        &self,
        jids: &[Jid],
    ) -> Vec<std::sync::Arc<async_lock::Mutex<()>>> {
        let mut mutexes = Vec::with_capacity(jids.len());
        let mut buf = wacore::types::jid::make_address_buffer();
        for jid in jids {
            wacore::types::jid::write_protocol_address_to(jid, &mut buf);
            mutexes.push(self.session_lock_for(&buf).await);
        }
        mutexes
    }
}

/// Self-DM detection: appending an own-device lookup on top of the
/// recipient's list would address each physical device twice (LID + PN),
/// which the server rejects with `ack error="400"`.
/// WAWebDBDeviceListFanout never re-fetches the own list for the same account.
pub(crate) fn is_self_dm_recipient(
    recipient_bare: &Jid,
    own_pn: &Jid,
    own_lid: Option<&Jid>,
) -> bool {
    match recipient_bare.server {
        Server::Lid => own_lid.is_some_and(|lid| recipient_bare.user == lid.user),
        Server::Pn => recipient_bare.user == own_pn.user,
        _ => false,
    }
}

/// The outer `<message to>`, the DeviceSentMessage destinationJid, and the
/// reporting-token remote jid must share the participants' namespace.
/// WAWebSendMsgCreateFanoutStanza builds the whole stanza from one CHAT_JID
/// (always a bare user wid), so the `to` is the resolved wire jid whenever
/// the caller's namespace differs from it (LID upgrade, or PN downgrade on
/// an unmigrated account), and a device-qualified caller jid is normalized
/// to the bare chat jid. A `to` mixing namespaces with the participants is
/// rejected wholesale by the server with `ack error="400"`.
pub(crate) fn dm_stanza_to(recipient_bare: &Jid, to: &Jid) -> Jid {
    if recipient_bare.is_lid() || to.is_lid() {
        recipient_bare.clone()
    } else {
        to.to_non_ad()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn dm_stanza_to_follows_resolved_wire_namespace() {
        let pn: Jid = "5511987650001@s.whatsapp.net".parse().unwrap();
        let lid: Jid = "111000011112222@lid".parse().unwrap();

        // PN caller, PN wire (unmigrated or unmapped): caller jid preserved.
        assert_eq!(dm_stanza_to(&pn, &pn), pn);
        // PN caller upgraded to LID wire: `to` must be the LID.
        assert_eq!(dm_stanza_to(&lid, &pn), lid);
        // LID caller kept on LID wire: unchanged.
        assert_eq!(dm_stanza_to(&lid, &lid), lid);
        // LID caller downgraded to PN wire (unmigrated account): `to` must be
        // the PN — reusing the caller's LID would mix namespaces.
        assert_eq!(dm_stanza_to(&pn, &lid), pn);
        // Device-qualified caller jid is normalized to the bare chat jid.
        let pn_device: Jid = "5511987650001:5@s.whatsapp.net".parse().unwrap();
        assert_eq!(dm_stanza_to(&pn, &pn_device), pn);
    }

    #[test]
    fn ensure_self_in_group_shares_when_present_and_appends_when_absent() {
        use wacore::client::context::GroupInfo;
        use wacore::types::message::AddressingMode;

        let own: Jid = "999999999999@s.whatsapp.net".parse().unwrap();
        let other: Jid = "111111111111@s.whatsapp.net".parse().unwrap();

        // Self already a member (the common case): the shared Arc passes through
        // untouched, with no deep clone of the participant list.
        let with_self = std::sync::Arc::new(GroupInfo::new(
            vec![other.to_non_ad(), own.to_non_ad()],
            AddressingMode::Pn,
        ));
        let out = ensure_self_in_group(with_self.clone(), &own);
        assert!(std::sync::Arc::ptr_eq(&with_self, &out));

        // Self missing: a fresh GroupInfo is built with self appended.
        let without_self =
            std::sync::Arc::new(GroupInfo::new(vec![other.to_non_ad()], AddressingMode::Pn));
        let out = ensure_self_in_group(without_self.clone(), &own);
        assert!(!std::sync::Arc::ptr_eq(&without_self, &out));
        assert_eq!(out.participants.len(), 2);
        assert!(out.participants.iter().any(|p| p.is_same_user_as(&own)));
    }

    #[tokio::test]
    async fn send_message_to_status_without_reaction_errors() {
        let client = crate::test_utils::create_test_client().await;
        let to = Jid::status_broadcast();
        let err = client
            .send_message(
                to,
                wa::Message {
                    conversation: Some("hi".into()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("status@broadcast without reaction must error");
        let msg = format!("{err}");
        assert!(
            msg.contains("reaction_message") || msg.contains("status"),
            "unexpected error: {msg}"
        );
    }

    // A logged-out send goes through send_message_impl, whose internal
    // `ClientError::NotLoggedIn` is threaded as `anyhow`. The wrapper must
    // surface the typed `SendError::NotLoggedIn`, not the `Internal` catch-all,
    // so callers can match it (regression test for r3432644890).
    #[tokio::test]
    async fn send_message_logged_out_dm_returns_not_logged_in() {
        let client = crate::test_utils::create_test_client().await;
        let to: Jid = "111111111111@s.whatsapp.net".parse().unwrap();
        let err = client
            .send_message(
                to,
                wa::Message {
                    conversation: Some("hi".into()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("logged-out DM send must error");
        assert!(
            matches!(err, SendError::NotLoggedIn),
            "expected SendError::NotLoggedIn, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn send_message_to_status_reaction_rejects_non_user_participant() {
        let client = crate::test_utils::create_test_client().await;
        let to = Jid::status_broadcast();
        let err = client
            .send_message(
                to,
                wa::Message {
                    reaction_message: buffa::MessageField::some(wa::message::ReactionMessage {
                        key: buffa::MessageField::some(wa::MessageKey {
                            remote_jid: Some("status@broadcast".into()),
                            from_me: Some(false),
                            id: Some("ORIGID".into()),
                            participant: Some("120363040237990503@g.us".into()),
                        }),
                        text: Some("❤️".into()),
                        sender_timestamp_ms: Some(1),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .expect_err("group JID as participant must error");
        assert!(
            format!("{err}").contains("user JID"),
            "expected user-JID error, got: {err}"
        );
    }

    #[tokio::test]
    async fn send_message_to_status_reaction_without_participant_errors() {
        let client = crate::test_utils::create_test_client().await;
        let to = Jid::status_broadcast();
        let err = client
            .send_message(
                to,
                wa::Message {
                    reaction_message: buffa::MessageField::some(wa::message::ReactionMessage {
                        key: buffa::MessageField::some(wa::MessageKey {
                            remote_jid: Some("status@broadcast".into()),
                            from_me: Some(false),
                            id: Some("ORIGID".into()),
                            participant: None,
                        }),
                        text: Some("❤️".into()),
                        sender_timestamp_ms: Some(1),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            )
            .await
            .expect_err("reaction without key.participant must error");
        assert!(
            format!("{err}").contains("participant"),
            "expected participant error, got: {err}"
        );
    }

    #[test]
    fn test_revoke_type_default_is_sender() {
        // RevokeType::Sender is the default (for deleting own messages)
        let revoke_type = RevokeType::default();
        assert_eq!(revoke_type, RevokeType::Sender);
    }

    #[test]
    fn test_force_skdm_only_for_admin_revoke() {
        // Admin revokes require force_skdm=true to get proper message structure
        // with phash, <participants>, and <device-identity> that WhatsApp Web uses.
        // Without this, the server returns error 479.
        let sender_jid = Jid::from_str("123456@s.whatsapp.net").unwrap();

        let sender_revoke = RevokeType::Sender;
        let admin_revoke = RevokeType::Admin {
            original_sender: sender_jid,
        };

        // This matches the logic in revoke_message()
        let force_skdm_sender = matches!(sender_revoke, RevokeType::Admin { .. });
        let force_skdm_admin = matches!(admin_revoke, RevokeType::Admin { .. });

        assert!(!force_skdm_sender, "Sender revoke should NOT force SKDM");
        assert!(force_skdm_admin, "Admin revoke MUST force SKDM");
    }

    #[test]
    fn test_sender_revoke_message_key_structure() {
        // Sender revoke (edit="7"): from_me=true, participant=None
        // The sender is identified by from_me=true, no participant field needed
        let to = Jid::from_str("120363040237990503@g.us").unwrap();
        let message_id = "3EB0ABC123".to_string();

        let (from_me, participant, edit_attr) = match RevokeType::Sender {
            RevokeType::Sender => (
                true,
                None,
                crate::types::message::EditAttribute::SenderRevoke,
            ),
            RevokeType::Admin { original_sender } => (
                false,
                Some(original_sender.to_non_ad_string()),
                crate::types::message::EditAttribute::AdminRevoke,
            ),
        };

        assert!(from_me, "Sender revoke must have from_me=true");
        assert!(
            participant.is_none(),
            "Sender revoke must NOT set participant"
        );
        assert_eq!(edit_attr.to_string_val(), "7");

        let revoke_message = build_revoke_message(&to, from_me, message_id.clone(), participant);

        let proto_msg = revoke_message.protocol_message.into_option().unwrap();
        let key = proto_msg.key.into_option().unwrap();
        assert_eq!(key.from_me, Some(true));
        assert_eq!(key.participant, None);
        assert_eq!(key.id, Some(message_id));
    }

    #[test]
    fn test_admin_revoke_message_key_structure() {
        // Admin revoke (edit="8"): from_me=false, participant=original_sender
        // The participant field identifies whose message is being deleted
        let to = Jid::from_str("120363040237990503@g.us").unwrap();
        let message_id = "3EB0ABC123".to_string();
        let original_sender = Jid::from_str("236395184570386:22@lid").unwrap();

        let revoke_type = RevokeType::Admin {
            original_sender: original_sender.clone(),
        };
        let (from_me, participant, edit_attr) = match revoke_type {
            RevokeType::Sender => (
                true,
                None,
                crate::types::message::EditAttribute::SenderRevoke,
            ),
            RevokeType::Admin { original_sender } => (
                false,
                Some(original_sender.to_non_ad_string()),
                crate::types::message::EditAttribute::AdminRevoke,
            ),
        };

        assert!(!from_me, "Admin revoke must have from_me=false");
        assert!(
            participant.is_some(),
            "Admin revoke MUST set participant to original sender"
        );
        assert_eq!(edit_attr.to_string_val(), "8");

        let revoke_message =
            build_revoke_message(&to, from_me, message_id.clone(), participant.clone());

        let proto_msg = revoke_message.protocol_message.into_option().unwrap();
        let key = proto_msg.key.into_option().unwrap();
        assert_eq!(key.from_me, Some(false));
        // Participant should be the original sender with device number stripped
        assert_eq!(key.participant, Some("236395184570386@lid".to_string()));
        assert_eq!(key.id, Some(message_id));
    }

    // Fictitious JIDs (not real PII):
    //   own PN user = "5500000000000"
    //   own LID user = "111111111111111"
    //   other LID user = "222222222222222"
    const SELF_PN: &str = "5500000000000";
    const SELF_LID: &str = "111111111111111";
    const SELF_DEVICE: u16 = 7;
    const OTHER_LID: &str = "222222222222222";

    #[test]
    fn self_dm_lid_recipient_matches_own_lid() {
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let own_lid = Jid::lid_device(SELF_LID, SELF_DEVICE);
        let recipient = Jid::lid(SELF_LID);

        assert!(is_self_dm_recipient(&recipient, &own_pn, Some(&own_lid)));
    }

    #[test]
    fn self_dm_pn_recipient_matches_own_pn() {
        // Self-DM addressed in PN namespace (no LID mapping resolved yet).
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let own_lid = Jid::lid_device(SELF_LID, SELF_DEVICE);
        let recipient = Jid::pn(SELF_PN);

        assert!(is_self_dm_recipient(&recipient, &own_pn, Some(&own_lid)));
    }

    #[test]
    fn self_dm_pn_recipient_self_dm_even_without_own_lid() {
        // PN-keyed self-detection does not require an own_lid to be known.
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let recipient = Jid::pn(SELF_PN);

        assert!(is_self_dm_recipient(&recipient, &own_pn, None));
    }

    #[test]
    fn non_self_lid_recipient_is_not_self_dm() {
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let own_lid = Jid::lid_device(SELF_LID, SELF_DEVICE);
        let recipient = Jid::lid(OTHER_LID);

        assert!(!is_self_dm_recipient(&recipient, &own_pn, Some(&own_lid)));
    }

    #[test]
    fn lid_recipient_without_own_lid_is_not_self_dm() {
        // WAWebUserPrefsMeUser.isMeAccount keys on isSameAccountAndAddressingMode;
        // PN-string equality across namespaces must NOT trigger.
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let recipient = Jid::lid(SELF_PN);

        assert!(!is_self_dm_recipient(&recipient, &own_pn, None));
    }

    #[test]
    fn group_or_broadcast_recipient_is_not_self_dm() {
        // Defensive: only PN/LID DMs ever take the self-DM short-circuit.
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let own_lid = Jid::lid_device(SELF_LID, SELF_DEVICE);

        assert!(!is_self_dm_recipient(
            &Jid::group("120363000000000000"),
            &own_pn,
            Some(&own_lid),
        ));
        assert!(!is_self_dm_recipient(
            &Jid::status_broadcast(),
            &own_pn,
            Some(&own_lid),
        ));
    }

    #[test]
    fn self_dm_with_no_recipient_cache_still_appends_own_devices() {
        // Edge case raised in PR review: if `recipient_cached` ends up `None`
        // (cache eviction + warmup failed), the self-DM short-circuit must
        // still let `own_cached` populate the fanout. Otherwise the bare-JID
        // fallback drops every companion device.
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let own_lid = Jid::lid_device(SELF_LID, SELF_DEVICE);
        let recipient_bare = Jid::lid(SELF_LID);
        assert!(is_self_dm_recipient(
            &recipient_bare,
            &own_pn,
            Some(&own_lid)
        ));

        let recipient_cached: Option<Vec<Jid>> = None;
        let own_cached_pn: Vec<Jid> = [0u16, 3, SELF_DEVICE]
            .into_iter()
            .map(|d| Jid::pn_device(SELF_PN, d))
            .collect();

        // Mirrors the call-site logic: we keep own_cached when recipient_cached is None
        // even in a self-DM.
        let keep_own = recipient_cached.is_none();
        assert!(keep_own);

        let mut all_dm_jids = match recipient_cached {
            Some(devices) => devices,
            None => vec![recipient_bare],
        };
        if keep_own {
            all_dm_jids.extend(own_cached_pn.iter().cloned());
        }
        all_dm_jids.retain(|j| {
            let is_sender = (j.is_same_user_as(&own_pn) && j.device == own_pn.device)
                || (j.is_same_user_as(&own_lid) && j.device == own_lid.device);
            !is_sender
        });
        wacore::types::jid::sort_dedup_by_device(&mut all_dm_jids);

        // Must contain the bare LID plus the two non-sender PN companion devices.
        assert!(
            all_dm_jids.iter().any(|j| j.is_lid()),
            "bare recipient LID must remain"
        );
        assert_eq!(
            all_dm_jids.iter().filter(|j| j.is_pn()).count(),
            2,
            "companion PN devices must survive when recipient_cached is None"
        );
    }

    #[test]
    fn old_merge_produced_lid_pn_duplicates_for_self_dm() {
        // Pinning regression: the OLD merge path (recipient_cached LID ++
        // own_cached PN, then sort_dedup_by_device) left every device listed
        // twice for a self-DM, which the server rejects with ack error="400".
        let own_pn = Jid::pn_device(SELF_PN, SELF_DEVICE);
        let own_lid = Jid::lid_device(SELF_LID, SELF_DEVICE);
        let recipient_bare = Jid::lid(SELF_LID);

        let devices = [0u16, 3, 5, SELF_DEVICE];
        let recipient_cached: Vec<Jid> = devices
            .iter()
            .map(|&d| Jid::lid_device(SELF_LID, d))
            .collect();
        let own_cached: Vec<Jid> = devices
            .iter()
            .map(|&d| Jid::pn_device(SELF_PN, d))
            .collect();

        let retain_non_sender = |j: &Jid| {
            let is_sender = (j.is_same_user_as(&own_pn) && j.device == own_pn.device)
                || (j.is_same_user_as(&own_lid) && j.device == own_lid.device);
            !is_sender
        };

        let mut buggy = recipient_cached.clone();
        buggy.extend(own_cached.clone());
        buggy.retain(retain_non_sender);
        wacore::types::jid::sort_dedup_by_device(&mut buggy);
        assert_eq!(buggy.len(), (devices.len() - 1) * 2);

        assert!(is_self_dm_recipient(
            &recipient_bare,
            &own_pn,
            Some(&own_lid)
        ));

        let mut fixed = recipient_cached;
        fixed.retain(retain_non_sender);
        wacore::types::jid::sort_dedup_by_device(&mut fixed);
        assert_eq!(fixed.len(), devices.len() - 1);
        for j in &fixed {
            assert!(j.is_lid());
        }
    }

    #[test]
    fn test_admin_revoke_preserves_lid_format() {
        // LID JIDs must NOT be converted to PN (phone number) format.
        // This was a bug that caused error 479 - the participant field must
        // preserve the original JID format exactly (with device stripped).
        let lid_sender = Jid::from_str("236395184570386:22@lid").unwrap();
        let participant_str = lid_sender.to_non_ad_string();

        // Must preserve @lid suffix, device number stripped
        assert_eq!(participant_str, "236395184570386@lid");
        assert!(
            participant_str.ends_with("@lid"),
            "LID participant must preserve @lid suffix"
        );
    }

    // SKDM Recipient Filtering Tests - validates DeviceKey-based filtering

    #[test]
    fn test_skdm_recipient_filtering_basic() {
        use std::collections::HashSet;

        let known_recipients: Vec<Jid> = [
            "1234567890:0@s.whatsapp.net",
            "1234567890:5@s.whatsapp.net",
            "9876543210:0@s.whatsapp.net",
        ]
        .into_iter()
        .map(|s| Jid::from_str(s).unwrap())
        .collect();

        let all_devices: Vec<Jid> = [
            "1234567890:0@s.whatsapp.net",
            "1234567890:5@s.whatsapp.net",
            "9876543210:0@s.whatsapp.net",
            "5555555555:0@s.whatsapp.net", // new
        ]
        .into_iter()
        .map(|s| Jid::from_str(s).unwrap())
        .collect();

        let known_set: HashSet<DeviceKey<'_>> =
            known_recipients.iter().map(|j| j.device_key()).collect();

        let new_devices: Vec<Jid> = all_devices
            .into_iter()
            .filter(|device| !known_set.contains(&device.device_key()))
            .collect();

        assert_eq!(new_devices.len(), 1);
        assert_eq!(new_devices[0].user, "5555555555");
    }

    #[test]
    fn test_skdm_recipient_filtering_lid_jids() {
        use std::collections::HashSet;

        let known_recipients: Vec<Jid> = [
            "236395184570386:91@lid",
            "129171292463295:0@lid",
            "45857667830004:14@lid",
        ]
        .into_iter()
        .map(|s| Jid::from_str(s).unwrap())
        .collect();

        let all_devices: Vec<Jid> = [
            "236395184570386:91@lid",
            "129171292463295:0@lid",
            "45857667830004:14@lid",
            "45857667830004:15@lid", // new
        ]
        .into_iter()
        .map(|s| Jid::from_str(s).unwrap())
        .collect();

        let known_set: HashSet<DeviceKey<'_>> =
            known_recipients.iter().map(|j| j.device_key()).collect();

        let new_devices: Vec<Jid> = all_devices
            .into_iter()
            .filter(|device| !known_set.contains(&device.device_key()))
            .collect();

        assert_eq!(new_devices.len(), 1);
        assert_eq!(new_devices[0].user, "45857667830004");
        assert_eq!(new_devices[0].device, 15);
    }

    #[test]
    fn test_skdm_recipient_filtering_all_known() {
        use std::collections::HashSet;

        let known_recipients: Vec<Jid> =
            ["1234567890:0@s.whatsapp.net", "1234567890:5@s.whatsapp.net"]
                .into_iter()
                .map(|s| Jid::from_str(s).unwrap())
                .collect();

        let all_devices: Vec<Jid> = ["1234567890:0@s.whatsapp.net", "1234567890:5@s.whatsapp.net"]
            .into_iter()
            .map(|s| Jid::from_str(s).unwrap())
            .collect();

        let known_set: HashSet<DeviceKey<'_>> =
            known_recipients.iter().map(|j| j.device_key()).collect();

        let new_devices: Vec<Jid> = all_devices
            .into_iter()
            .filter(|device| !known_set.contains(&device.device_key()))
            .collect();

        assert!(new_devices.is_empty());
    }

    #[test]
    fn test_skdm_recipient_filtering_all_new() {
        use std::collections::HashSet;

        let known_recipients: Vec<Jid> = vec![];

        let all_devices: Vec<Jid> = ["1234567890:0@s.whatsapp.net", "9876543210:0@s.whatsapp.net"]
            .into_iter()
            .map(|s| Jid::from_str(s).unwrap())
            .collect();

        let known_set: HashSet<DeviceKey<'_>> =
            known_recipients.iter().map(|j| j.device_key()).collect();

        let new_devices: Vec<Jid> = all_devices
            .clone()
            .into_iter()
            .filter(|device| !known_set.contains(&device.device_key()))
            .collect();

        assert_eq!(new_devices.len(), all_devices.len());
    }

    #[test]
    fn test_device_key_comparison() {
        // Jid parse/display normalizes :0 (omitted in Display, missing ':N' parses as device 0).
        // This test ensures DeviceKey comparisons work correctly under that normalization.
        let test_cases = [
            (
                "1234567890:0@s.whatsapp.net",
                "1234567890@s.whatsapp.net",
                true,
            ),
            (
                "1234567890:5@s.whatsapp.net",
                "1234567890:5@s.whatsapp.net",
                true,
            ),
            (
                "1234567890:5@s.whatsapp.net",
                "1234567890:6@s.whatsapp.net",
                false,
            ),
            ("236395184570386:91@lid", "236395184570386:91@lid", true),
            ("236395184570386:0@lid", "236395184570386@lid", true),
            ("user1@s.whatsapp.net", "user2@s.whatsapp.net", false),
        ];

        for (jid1_str, jid2_str, should_match) in test_cases {
            let jid1: Jid = jid1_str.parse().expect("should parse jid1");
            let jid2: Jid = jid2_str.parse().expect("should parse jid2");

            let key1 = jid1.device_key();
            let key2 = jid2.device_key();

            assert_eq!(
                key1 == key2,
                should_match,
                "DeviceKey comparison failed for '{}' vs '{}': expected match={}, got match={}",
                jid1_str,
                jid2_str,
                should_match,
                key1 == key2
            );

            assert_eq!(
                jid1.device_eq(&jid2),
                should_match,
                "device_eq failed for '{}' vs '{}'",
                jid1_str,
                jid2_str
            );
        }
    }

    #[test]
    fn empty_sender_key_device_map_marks_all_devices_for_skdm() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let map = SenderKeyDeviceMap::from_db_rows(&[]);
        assert_eq!(map.device_has_key("271060335329480", 0), None);

        let all_resolved_devices: Vec<Jid> = [
            "271060335329480@lid",
            "77610646245392@lid",
            "276661023027320:5@lid",
        ]
        .into_iter()
        .map(|s| Jid::from_str(s).unwrap())
        .collect();

        let needs_skdm: Vec<&Jid> = all_resolved_devices
            .iter()
            .filter(|device| {
                !map.device_has_key(&device.user, device.device)
                    .unwrap_or(false)
                    || !map.device_has_key(&device.user, 0).unwrap_or(false)
            })
            .collect();

        assert_eq!(needs_skdm.len(), all_resolved_devices.len());
    }

    /// Fails if the empty-cache early-exit is reintroduced.
    #[tokio::test]
    async fn resolve_skdm_targets_distributes_when_cache_empty_but_devices_known() {
        use wacore::client::context::GroupInfo;
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};
        use wacore::types::message::AddressingMode;

        let client = crate::test_utils::create_test_client().await;
        let group_jid = "120363161500776365@g.us";
        let own_lid = Jid::from_str("193832511623409:13@lid").unwrap();

        let participant_users = ["271060335329480", "77610646245392", "276661023027320"];

        // Pre-populate so `resolve_devices` succeeds without a transport.
        for user in &participant_users {
            let record = DeviceListRecord {
                user: (*user).into(),
                devices: vec![DeviceInfo {
                    device_id: 0,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            };
            client
                .device_registry_cache
                .raw_insert_for_tests((*user).into(), Arc::new(record))
                .await;
        }

        let participants: Vec<Jid> = participant_users
            .iter()
            .map(|u| Jid::from_str(&format!("{u}@lid")).unwrap())
            .collect();

        let group_info = GroupInfo::new(participants.clone(), AddressingMode::Lid);

        let (all_devices, needs_skdm) = client
            .resolve_skdm_targets(group_jid, &group_info, &own_lid)
            .await
            .expect("None means device resolution failed");

        // Empty cache → every participant needs SKDM, and the full set equals
        // the target set on this cold path.
        assert_eq!(needs_skdm.len(), participants.len());
        assert_eq!(all_devices.devices().len(), participants.len());
        for user in &participant_users {
            assert!(needs_skdm.iter().any(|j| j.user == *user));
            assert!(all_devices.devices().iter().any(|j| j.user == *user));
        }
    }

    #[test]
    fn single_forgotten_row_keeps_full_distribution() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let map = SenderKeyDeviceMap::from_db_rows(&[("271060335329480@lid".to_string(), false)]);
        assert_eq!(map.device_has_key("271060335329480", 0), Some(false));

        let all_resolved_devices: Vec<Jid> = [
            "271060335329480@lid",
            "77610646245392@lid",
            "276661023027320:5@lid",
        ]
        .into_iter()
        .map(|s| Jid::from_str(s).unwrap())
        .collect();

        let needs_skdm: Vec<&Jid> = all_resolved_devices
            .iter()
            .filter(|device| {
                !map.device_has_key(&device.user, device.device)
                    .unwrap_or(false)
                    || !map.device_has_key(&device.user, 0).unwrap_or(false)
            })
            .collect();

        assert_eq!(
            needs_skdm.len(),
            3,
            "after retry inserts one row, ALL devices correctly flagged for SKDM \
             (this is what unblocks redistribution on the SECOND message)"
        );
    }

    /// WA Web primary-device gate (ParticipantStore.js): a companion is warm only
    /// when it AND its primary (device 0) hold the key. A forgotten companion
    /// redistributes only itself (no per-user amplification); a forgotten primary
    /// redistributes the whole user. Drives the real `filter_skdm_targets`.
    #[tokio::test]
    async fn filter_skdm_targets_uses_primary_device_gate() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;

        let client = crate::test_utils::create_test_client().await;
        let group = "120363161500776365@g.us";
        let own = Jid::from_str("999999999999999:1@lid").unwrap();

        // Companion forgotten, primary warm: only the companion redistributes.
        let map = SenderKeyDeviceMap::from_db_rows(&[
            ("100100100100100@lid".to_string(), true),
            ("100100100100100:5@lid".to_string(), false),
        ]);
        let devices = [
            Jid::from_str("100100100100100@lid").unwrap(),
            Jid::from_str("100100100100100:5@lid").unwrap(),
        ];
        let needs = client.filter_skdm_targets(group, &devices, &map, &own);
        assert_eq!(needs.len(), 1, "warm primary keeps the keyed companion out");
        assert_eq!(needs[0].device, 5);

        // Primary forgotten, companion warm: the whole user redistributes (WA Web
        // marks a companion cold when its primary is cold).
        let map = SenderKeyDeviceMap::from_db_rows(&[
            ("200200200200200@lid".to_string(), false),
            ("200200200200200:5@lid".to_string(), true),
        ]);
        let devices = [
            Jid::from_str("200200200200200@lid").unwrap(),
            Jid::from_str("200200200200200:5@lid").unwrap(),
        ];
        let needs = client.filter_skdm_targets(group, &devices, &map, &own);
        assert_eq!(needs.len(), 2, "cold primary redistributes the whole user");

        // Companion warm but the primary row is absent (None): WA Web's `?? false`
        // treats a missing primary as cold, so the companion still redistributes.
        let map = SenderKeyDeviceMap::from_db_rows(&[("300300300300300:5@lid".to_string(), true)]);
        let devices = [Jid::from_str("300300300300300:5@lid").unwrap()];
        let needs = client.filter_skdm_targets(group, &devices, &map, &own);
        assert_eq!(
            needs.len(),
            1,
            "absent primary is cold, companion redistributes"
        );
    }

    #[test]
    fn test_skdm_filtering_large_group() {
        use std::collections::HashSet;

        let mut known_recipients: Vec<Jid> = Vec::with_capacity(1000);
        let mut all_devices: Vec<Jid> = Vec::with_capacity(1010);

        for i in 0..1000i64 {
            let jid_str = format!("{}:1@lid", 100000000000000i64 + i);
            let jid = Jid::from_str(&jid_str).unwrap();
            known_recipients.push(jid.clone());
            all_devices.push(jid);
        }

        for i in 1000i64..1010i64 {
            let jid_str = format!("{}:1@lid", 100000000000000i64 + i);
            all_devices.push(Jid::from_str(&jid_str).unwrap());
        }

        let known_set: HashSet<DeviceKey<'_>> =
            known_recipients.iter().map(|j| j.device_key()).collect();

        let new_devices: Vec<Jid> = all_devices
            .into_iter()
            .filter(|device| !known_set.contains(&device.device_key()))
            .collect();

        assert_eq!(new_devices.len(), 10);
    }

    mod infer_stanza {
        use super::*;

        #[test]
        fn regular_message_returns_none() {
            let msg = wa::Message {
                conversation: Some("hello".into()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            assert!(node.is_none());
        }

        #[test]
        fn pin_returns_edit_attribute() {
            let msg = wa::Message {
                pin_in_chat_message: buffa::MessageField::some(Default::default()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert_eq!(edit, Some(EditAttribute::PinInChat));
            assert!(node.is_none());
        }

        #[test]
        fn poll_creation_v3_returns_meta_node() {
            let msg = wa::Message {
                poll_creation_message_v3: buffa::MessageField::some(Default::default()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            let node = node.expect("should have meta node");
            assert_eq!(node.tag, "meta");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("polltype").unwrap().as_ref(),
                "creation"
            );
        }

        #[test]
        fn event_returns_meta_node() {
            let msg = wa::Message {
                event_message: buffa::MessageField::some(Default::default()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            let node = node.expect("should have meta node");
            assert_eq!(node.tag, "meta");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("event_type").unwrap().as_ref(),
                "creation"
            );
        }

        #[test]
        fn empty_message_returns_none() {
            let (edit, node) = infer_stanza_metadata(&wa::Message::default());
            assert!(edit.is_none());
            assert!(node.is_none());
        }

        #[test]
        fn member_label_set_returns_member_tag_user_update() {
            let msg = wacore::send::build_member_label_message("VIP".to_string(), 1_700_000_000);
            let (_, node) = infer_stanza_metadata(&msg);
            let node = node.expect("member_label should have meta node");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("appdata").unwrap().as_ref(),
                "member_tag"
            );
            assert_eq!(
                attrs.optional_string("tag_reason").unwrap().as_ref(),
                "user_update"
            );
        }

        #[test]
        fn member_label_clear_returns_user_delete() {
            // Empty label = clearing the tag → tag_reason "user_delete".
            let msg = wacore::send::build_member_label_message(String::new(), 1_700_000_000);
            let (_, node) = infer_stanza_metadata(&msg);
            let node = node.expect("member_label should have meta node");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("appdata").unwrap().as_ref(),
                "member_tag"
            );
            assert_eq!(
                attrs.optional_string("tag_reason").unwrap().as_ref(),
                "user_delete"
            );
        }

        #[test]
        fn poll_creation_v1_returns_meta_node() {
            let msg = wa::Message {
                poll_creation_message: buffa::MessageField::some(Default::default()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            let node = node.expect("should have meta node");
            assert_eq!(node.tag, "meta");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("polltype").unwrap().as_ref(),
                "creation"
            );
        }

        #[test]
        fn poll_creation_v2_returns_meta_node() {
            let msg = wa::Message {
                poll_creation_message_v2: buffa::MessageField::some(Default::default()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            let node = node.expect("should have meta node");
            assert_eq!(node.tag, "meta");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("polltype").unwrap().as_ref(),
                "creation"
            );
        }

        #[test]
        fn poll_vote_returns_meta_node() {
            let msg = wa::Message {
                poll_update_message: buffa::MessageField::some(wa::message::PollUpdateMessage {
                    vote: buffa::MessageField::some(wa::message::PollEncValue::default()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            let node = node.expect("should have meta node");
            assert_eq!(node.tag, "meta");
            let mut attrs = node.attrs();
            assert_eq!(attrs.optional_string("polltype").unwrap().as_ref(), "vote");
        }

        #[test]
        fn view_once_image_emits_view_once_meta() {
            let msg = wa::Message {
                image_message: buffa::MessageField::some(wa::message::ImageMessage {
                    view_once: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (_, node) = infer_stanza_metadata(&msg);
            let node = node.expect("view-once image should emit meta");
            assert_eq!(node.tag, "meta");
            assert_eq!(
                node.attrs().optional_string("view_once").unwrap().as_ref(),
                "true"
            );
        }

        #[test]
        fn plain_image_emits_no_meta() {
            let msg = wa::Message {
                image_message: buffa::MessageField::some(wa::message::ImageMessage::default()),
                ..Default::default()
            };
            assert!(infer_stanza_metadata(&msg).1.is_none());
        }

        #[test]
        fn event_response_returns_meta_node() {
            let msg = wa::Message {
                enc_event_response_message: buffa::MessageField::some(Default::default()),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            let node = node.expect("should have meta node");
            assert_eq!(node.tag, "meta");
            let mut attrs = node.attrs();
            assert_eq!(
                attrs.optional_string("event_type").unwrap().as_ref(),
                "response"
            );
        }

        #[test]
        fn poll_update_without_vote_returns_none() {
            let msg = wa::Message {
                poll_update_message: buffa::MessageField::some(wa::message::PollUpdateMessage {
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert!(edit.is_none());
            assert!(node.is_none());
        }

        #[test]
        fn revoked_reaction_returns_sender_revoke() {
            let msg = wa::Message {
                reaction_message: buffa::MessageField::some(wa::message::ReactionMessage {
                    text: Some(String::new()),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (edit, _) = infer_stanza_metadata(&msg);
            assert_eq!(edit, Some(EditAttribute::SenderRevoke));
        }

        #[test]
        fn keep_in_chat_undo_returns_sender_revoke() {
            let msg = wa::Message {
                keep_in_chat_message: buffa::MessageField::some(wa::message::KeepInChatMessage {
                    key: buffa::MessageField::some(wa::MessageKey {
                        from_me: Some(true),
                        ..Default::default()
                    }),
                    keep_type: Some(wa::KeepType::UndoKeepForAll),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let (edit, _) = infer_stanza_metadata(&msg);
            assert_eq!(edit, Some(EditAttribute::SenderRevoke));
        }

        #[test]
        fn secret_encrypted_message_edit_returns_message_edit() {
            let msg = wa::Message {
                secret_encrypted_message: buffa::MessageField::some(
                    wa::message::SecretEncryptedMessage {
                        secret_enc_type: Some(
                            wa::message::secret_encrypted_message::SecretEncType::MessageEdit,
                        ),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            };
            let (edit, _) = infer_stanza_metadata(&msg);
            assert_eq!(edit, Some(EditAttribute::MessageEdit));
        }

        #[test]
        fn secret_encrypted_event_edit_emits_both_edit_attr_and_meta_node() {
            // EVENT_EDIT is the one case where the edit attribute AND the
            // meta node both fire: `event_type=edit` meta + `edit="1"` attr.
            let msg = wa::Message {
                secret_encrypted_message: buffa::MessageField::some(
                    wa::message::SecretEncryptedMessage {
                        secret_enc_type: Some(
                            wa::message::secret_encrypted_message::SecretEncType::EventEdit,
                        ),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            };
            let (edit, node) = infer_stanza_metadata(&msg);
            assert_eq!(edit, Some(EditAttribute::MessageEdit));
            let node = node.expect("should have meta node");
            assert_eq!(
                node.attrs().optional_string("event_type").unwrap().as_ref(),
                "edit"
            );
        }

        #[test]
        fn top_level_edited_message_returns_message_edit() {
            let msg = wa::Message {
                edited_message: buffa::MessageField::some(wa::message::FutureProofMessage {
                    message: buffa::MessageField::some(wa::Message::default()),
                }),
                ..Default::default()
            };
            let (edit, _) = infer_stanza_metadata(&msg);
            assert_eq!(edit, Some(EditAttribute::MessageEdit));
        }

        #[test]
        fn build_edit_message_uses_top_level_protocol_message() {
            use std::str::FromStr;
            let to = Jid::from_str("5511999999999@s.whatsapp.net").unwrap();
            let new_content = wa::Message {
                conversation: Some("edited".to_string()),
                ..Default::default()
            };
            let msg = build_edit_message(
                &to,
                "ORIG_ID".to_string(),
                None,
                new_content,
                1_700_000_000_000,
            );

            // Canonical WA Web shape: top-level protocolMessage(type=MESSAGE_EDIT),
            // not the Message.editedMessage FutureProofMessage history wrapper.
            assert!(
                msg.edited_message.is_unset(),
                "edit must not use the FutureProofMessage wrapper"
            );
            let pm = msg
                .protocol_message
                .as_option()
                .expect("top-level protocol_message");
            assert_eq!(
                pm.r#type,
                Some(wa::message::protocol_message::Type::MessageEdit)
            );
            assert_eq!(
                pm.key.as_option().and_then(|k| k.id.as_deref()),
                Some("ORIG_ID")
            );
            assert_eq!(pm.key.as_option().and_then(|k| k.from_me), Some(true));
            assert_eq!(
                pm.edited_message
                    .as_option()
                    .and_then(|m| m.conversation.as_deref()),
                Some("edited")
            );
            // The send path still derives the edit attribute from this shape.
            assert_eq!(
                infer_stanza_metadata(&msg).0,
                Some(EditAttribute::MessageEdit)
            );
        }
    }

    mod biz_node_tests {
        use super::*;
        use std::str::FromStr;
        use wa::message::interactive_message::{
            self, NativeFlowMessage, native_flow_message::NativeFlowButton,
        };

        // Fixed unix seconds for deterministic privacy_mode_ts assertions.
        const FIXED_NOW: u64 = 1_700_000_000;
        // FIXED_NOW - BIZ_PRIVACY_MODE_TS_OFFSET = 1_700_000_000 - 77_980_457
        const EXPECTED_PRIVACY_TS: &str = "1622019543";

        fn msg_with_native_flow_button(button_name: &str) -> wa::Message {
            wa::Message {
                interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                    interactive_message: Some(
                        interactive_message::InteractiveMessage::NativeFlowMessage(Box::new(
                            NativeFlowMessage {
                                buttons: vec![NativeFlowButton {
                                    name: Some(button_name.to_string()),
                                    button_params_json: None,
                                }],
                                message_version: Some(1),
                                message_params_json: None,
                            },
                        )),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            }
        }

        fn assert_biz_common_attrs(node: &Node, ctx: &str) {
            assert_eq!(node.tag, "biz", "{ctx}");
            let mut a = node.attrs();
            assert_eq!(
                a.optional_string("actual_actors").unwrap().as_ref(),
                "2",
                "{ctx}"
            );
            assert_eq!(
                a.optional_string("host_storage").unwrap().as_ref(),
                "2",
                "{ctx}"
            );
            assert_eq!(
                a.optional_string("privacy_mode_ts").unwrap().as_ref(),
                EXPECTED_PRIVACY_TS,
                "{ctx}"
            );
        }

        fn assert_nested_biz(node: &Node, expected_flow_name: &str, ctx: &str) {
            assert_biz_common_attrs(node, ctx);
            assert!(
                node.attrs().optional_string("native_flow_name").is_none(),
                "{ctx}: nested form has no native_flow_name attr"
            );
            let interactive = node
                .get_optional_child("interactive")
                .unwrap_or_else(|| panic!("{ctx}: missing <interactive>"));
            let mut ia = interactive.attrs();
            assert_eq!(
                ia.optional_string("type").unwrap().as_ref(),
                "native_flow",
                "{ctx}"
            );
            assert_eq!(ia.optional_string("v").unwrap().as_ref(), "1", "{ctx}");

            let nf = interactive
                .get_optional_child("native_flow")
                .unwrap_or_else(|| panic!("{ctx}: missing <native_flow>"));
            let mut nfa = nf.attrs();
            assert_eq!(nfa.optional_string("v").unwrap().as_ref(), "9", "{ctx}");
            assert_eq!(
                nfa.optional_string("name").unwrap().as_ref(),
                expected_flow_name,
                "{ctx}"
            );

            let qc = node
                .get_optional_child("quality_control")
                .unwrap_or_else(|| panic!("{ctx}: missing <quality_control>"));
            assert_eq!(
                qc.attrs().optional_string("source_type").unwrap().as_ref(),
                "third_party",
                "{ctx}"
            );
        }

        /// Payment-family buttons emit the flat `<biz>` form with
        /// `native_flow_name` as an attr and NO children.
        #[test]
        fn payment_simple_form() {
            let cases: &[(&str, &str)] = &[
                ("payment_info", "payment_info"),
                ("review_and_pay", "order_details"),
                ("review_order", "order_status"),
                ("order_status", "order_status"),
                ("payment_status", "payment_status"),
                ("payment_method", "payment_method"),
                ("payment_reminder", "payment_reminder"),
            ];
            for (button, expected_flow) in cases {
                let biz = infer_biz_node(&msg_with_native_flow_button(button), FIXED_NOW)
                    .unwrap_or_else(|| panic!("{button}: should produce biz"));
                assert_biz_common_attrs(&biz, button);
                assert_eq!(
                    biz.attrs()
                        .optional_string("native_flow_name")
                        .unwrap()
                        .as_ref(),
                    *expected_flow,
                    "{button}: native_flow_name attr"
                );
                assert!(
                    biz.children().unwrap_or(&[]).is_empty(),
                    "{button}: PaymentSimple has no children"
                );
            }
        }

        /// Named-nested buttons keep their flow name and gain the new
        /// privacy attrs plus `<quality_control>`.
        #[test]
        fn nested_named_form() {
            let cases: &[(&str, &str)] = &[
                ("cta_url", "cta_url"),
                ("cta_catalog", "cta_catalog"),
                ("catalog_message", "catalog_message"),
                ("galaxy_message", "galaxy_message"),
                ("booking_confirmation", "booking_confirmation"),
                ("call_permission_request", "call_permission_request"),
                ("open_webview", "message_with_link"),
                ("message_with_link_status", "message_with_link_status"),
            ];
            for (button, expected_flow) in cases {
                let biz = infer_biz_node(&msg_with_native_flow_button(button), FIXED_NOW)
                    .unwrap_or_else(|| panic!("{button}: should produce biz"));
                assert_nested_biz(&biz, expected_flow, button);
            }
        }

        /// quick_reply / cta_copy / cta_call / single_select / send_location
        /// and unknown future button names route through `name="mixed"`.
        #[test]
        fn mixed_form_for_dropped_buttons() {
            let cases: &[&str] = &[
                "quick_reply",
                "cta_copy",
                "cta_call",
                "single_select",
                "send_location",
                "future_button_xyz",
            ];
            for button in cases {
                let biz = infer_biz_node(&msg_with_native_flow_button(button), FIXED_NOW)
                    .unwrap_or_else(|| panic!("{button}: should produce biz"));
                assert_nested_biz(&biz, "mixed", button);
            }
        }

        /// Non-interactive messages produce no `<biz>` (no fan-out into the
        /// extra_stanza_nodes path).
        #[test]
        fn no_interactive_returns_none() {
            let msg = wa::Message {
                conversation: Some("hello".into()),
                ..Default::default()
            };
            assert!(infer_biz_node(&msg, FIXED_NOW).is_none());
        }

        /// Interactive but not native-flow (e.g. CollectionMessage) yields None.
        #[test]
        fn interactive_without_native_flow_returns_none() {
            let msg = wa::Message {
                interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                    interactive_message: Some(
                        interactive_message::InteractiveMessage::CollectionMessage(
                            Default::default(),
                        ),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(infer_biz_node(&msg, FIXED_NOW).is_none());
        }

        /// NativeFlow with empty button list yields None — no signal to classify.
        #[test]
        fn native_flow_without_buttons_returns_none() {
            let msg = wa::Message {
                interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                    interactive_message: Some(
                        interactive_message::InteractiveMessage::NativeFlowMessage(Box::new(
                            NativeFlowMessage {
                                buttons: vec![],
                                message_version: Some(1),
                                message_params_json: None,
                            },
                        )),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(infer_biz_node(&msg, FIXED_NOW).is_none());
        }

        /// Button with `name = None` is treated as missing classifier → None.
        #[test]
        fn button_without_name_returns_none() {
            let msg = wa::Message {
                interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                    interactive_message: Some(
                        interactive_message::InteractiveMessage::NativeFlowMessage(Box::new(
                            NativeFlowMessage {
                                buttons: vec![NativeFlowButton {
                                    name: None,
                                    button_params_json: None,
                                }],
                                message_version: Some(1),
                                message_params_json: None,
                            },
                        )),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            };
            assert!(infer_biz_node(&msg, FIXED_NOW).is_none());
        }

        /// Messages wrapped in `documentWithCaptionMessage` still pick up the
        /// native_flow payload from the inner message.
        #[test]
        fn document_with_caption_wrapper() {
            let inner = wa::Message {
                interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                    interactive_message: Some(
                        interactive_message::InteractiveMessage::NativeFlowMessage(Box::new(
                            NativeFlowMessage {
                                buttons: vec![NativeFlowButton {
                                    name: Some("quick_reply".into()),
                                    button_params_json: None,
                                }],
                                message_version: Some(1),
                                message_params_json: None,
                            },
                        )),
                    ),
                    ..Default::default()
                }),
                ..Default::default()
            };
            let msg = wa::Message {
                document_with_caption_message: buffa::MessageField::some(
                    wa::message::FutureProofMessage {
                        message: buffa::MessageField::some(inner),
                    },
                ),
                ..Default::default()
            };
            let biz = infer_biz_node(&msg, FIXED_NOW)
                .expect("doc-with-caption wrapper should propagate the inner native_flow");
            assert_nested_biz(&biz, "mixed", "doc-with-caption/quick_reply");
        }

        // -- build_extra_stanza_nodes assembly tests --

        fn quick_reply_biz() -> Node {
            infer_biz_node(&msg_with_native_flow_button("quick_reply"), FIXED_NOW)
                .expect("quick_reply produces biz")
        }

        fn payment_biz() -> Node {
            infer_biz_node(&msg_with_native_flow_button("payment_info"), FIXED_NOW)
                .expect("payment_info produces biz")
        }

        fn jid(s: &str) -> Jid {
            Jid::from_str(s).expect("valid jid in test")
        }

        /// DM: `<bot biz_bot="1"/>` is prepended before the `<biz>`. The
        /// order matters — this is the shape the upstream Baileys
        /// reproducer emits.
        #[test]
        fn dm_emits_bot_before_biz() {
            let nodes = build_extra_stanza_nodes(
                &jid("5511999999999@s.whatsapp.net"),
                None,
                Some(quick_reply_biz()),
                vec![],
            );
            assert_eq!(nodes.len(), 2, "expected [<bot>, <biz>]");
            assert_eq!(nodes[0].tag, "bot");
            assert_eq!(
                nodes[0]
                    .attrs()
                    .optional_string("biz_bot")
                    .unwrap()
                    .as_ref(),
                "1"
            );
            assert_eq!(nodes[1].tag, "biz");
        }

        /// Group: `<bot>` is NOT emitted; only `<biz>`.
        #[test]
        fn group_omits_bot() {
            let nodes = build_extra_stanza_nodes(
                &jid("120363000000000001@g.us"),
                None,
                Some(quick_reply_biz()),
                vec![],
            );
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].tag, "biz");
        }

        /// LID DM (non-group): `<bot>` is still emitted.
        #[test]
        fn lid_dm_emits_bot() {
            let nodes = build_extra_stanza_nodes(
                &jid("100000000000001@lid"),
                None,
                Some(payment_biz()),
                vec![],
            );
            assert_eq!(nodes.len(), 2);
            assert_eq!(nodes[0].tag, "bot");
        }

        /// No biz + no meta → user nodes pass through untouched.
        #[test]
        fn no_biz_no_meta_passthrough() {
            let user_nodes = vec![NodeBuilder::new("custom").build()];
            let nodes =
                build_extra_stanza_nodes(&jid("X@s.whatsapp.net"), None, None, user_nodes.clone());
            assert_eq!(nodes.len(), 1);
            assert_eq!(nodes[0].tag, "custom");
        }

        /// Full ordering: [meta, bot, biz, user_nodes...].
        #[test]
        fn full_ordering_meta_bot_biz_user() {
            let meta = NodeBuilder::new("meta").attr("appdata", "default").build();
            let user_a = NodeBuilder::new("user_a").build();
            let user_b = NodeBuilder::new("user_b").build();
            let nodes = build_extra_stanza_nodes(
                &jid("X@s.whatsapp.net"),
                Some(meta),
                Some(quick_reply_biz()),
                vec![user_a, user_b],
            );
            assert_eq!(nodes.len(), 5);
            assert_eq!(nodes[0].tag, "meta");
            assert_eq!(nodes[1].tag, "bot");
            assert_eq!(nodes[2].tag, "biz");
            assert_eq!(nodes[3].tag, "user_a");
            assert_eq!(nodes[4].tag, "user_b");
        }

        /// Meta-only (no biz) preserves order: meta then user nodes; no bot.
        #[test]
        fn meta_only_preserves_order() {
            let meta = NodeBuilder::new("meta").build();
            let user = NodeBuilder::new("u").build();
            let nodes =
                build_extra_stanza_nodes(&jid("X@s.whatsapp.net"), Some(meta), None, vec![user]);
            assert_eq!(nodes.len(), 2);
            assert_eq!(nodes[0].tag, "meta");
            assert_eq!(nodes[1].tag, "u");
        }
    }

    /// Regression tests for #462: send path session lock keys must match decrypt path.
    mod session_lock_regression {
        use super::*;

        #[tokio::test]
        async fn per_device_lock_keys_cover_all_devices() {
            let client = crate::test_utils::create_test_client().await;

            let devices: Vec<Jid> = [
                "100000012345678@lid",
                "100000012345678:5@lid",
                "100000012345678:33@lid",
            ]
            .iter()
            .map(|s| Jid::from_str(s).unwrap())
            .collect();

            // Uses the production helper (resolve_encryption_jid + sort + dedup)
            let send_lock_keys = client.build_session_lock_keys(&devices).await;

            assert_eq!(send_lock_keys.len(), 3);
            // Sorted by (server, user, device_numeric): 0, 5, 33
            assert_eq!(send_lock_keys[0].device, 0);
            assert_eq!(send_lock_keys[1].device, 5);
            assert_eq!(send_lock_keys[2].device, 33);

            // Send keys must cover every device
            for device_jid in &devices {
                assert!(
                    send_lock_keys.contains(device_jid),
                    "device {device_jid} not in send keys: {send_lock_keys:?}"
                );
            }

            // Bare JID key alone wouldn't protect linked devices
            let bare_key = devices[0].to_protocol_address_string();
            let device5_key = devices[1].to_protocol_address_string();
            assert_ne!(bare_key, device5_key);
        }

        #[tokio::test]
        async fn per_device_lock_serializes_concurrent_session_access() {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicU32, Ordering};

            let session_locks: crate::cache::Cache<String, Arc<async_lock::Mutex<()>>> =
                crate::cache::Cache::builder().max_capacity(100).build();

            let lock_key = "100000012345678:5@lid.0".to_string();
            let access_counter = Arc::new(AtomicU32::new(0));
            let max_concurrent = Arc::new(AtomicU32::new(0));

            let mut handles = Vec::new();
            for _ in 0..10 {
                let locks = session_locks.clone();
                let key = lock_key.clone();
                let counter = access_counter.clone();
                let max = max_concurrent.clone();

                handles.push(tokio::spawn(async move {
                    let mutex: Arc<async_lock::Mutex<()>> = locks
                        .get_with_by_ref(&key, async { Arc::new(async_lock::Mutex::new(())) })
                        .await;
                    // lock_arc() needed: guard must own the Arc since mutex is a local
                    // (production uses lock() with a separate Vec keeping Arcs alive)
                    let _guard = mutex.lock_arc().await;

                    let active = counter.fetch_add(1, Ordering::SeqCst) + 1;
                    max.fetch_max(active, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    counter.fetch_sub(1, Ordering::SeqCst);
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }

            assert_eq!(max_concurrent.load(Ordering::SeqCst), 1);
        }

        #[tokio::test]
        async fn different_device_locks_are_independent() {
            use std::sync::Arc;
            use std::sync::atomic::{AtomicU32, Ordering};

            let session_locks: crate::cache::Cache<String, Arc<async_lock::Mutex<()>>> =
                crate::cache::Cache::builder().max_capacity(100).build();

            let max_concurrent = Arc::new(AtomicU32::new(0));
            let counter = Arc::new(AtomicU32::new(0));
            let barrier = Arc::new(tokio::sync::Barrier::new(2));

            let keys = ["100000012345678@lid.0", "100000012345678:5@lid.0"];

            let mut handles = Vec::new();
            for key in keys {
                let locks = session_locks.clone();
                let key = key.to_string();
                let c = counter.clone();
                let m = max_concurrent.clone();
                let b = barrier.clone();

                handles.push(tokio::spawn(async move {
                    let mutex: Arc<async_lock::Mutex<()>> = locks
                        .get_with_by_ref(&key, async { Arc::new(async_lock::Mutex::new(())) })
                        .await;
                    // lock_arc(): same reason as above
                    let _guard = mutex.lock_arc().await;

                    let active = c.fetch_add(1, Ordering::SeqCst) + 1;
                    m.fetch_max(active, Ordering::SeqCst);
                    b.wait().await;
                    c.fetch_sub(1, Ordering::SeqCst);
                }));
            }

            for handle in handles {
                handle.await.unwrap();
            }

            assert_eq!(max_concurrent.load(Ordering::SeqCst), 2);
        }

        /// Regression: 1:1 DM recipient must use bare Signal address matching
        /// the receive path. Starts from device-specific JID and verifies
        /// to_non_ad() normalization produces the correct bare key.
        #[tokio::test]
        async fn dm_recipient_uses_bare_address() {
            let client = crate::test_utils::create_test_client().await;

            // Start from device-specific JID, exercise the production path
            let recipient_device33 = Jid::from_str("100000012345678:33@lid").unwrap();
            let own_device_5 = Jid::from_str("999999999999:5@s.whatsapp.net").unwrap();

            // Same normalization as send_message_impl
            let recipient_bare = client
                .resolve_encryption_jid(&recipient_device33)
                .await
                .to_non_ad();

            let all_dm_jids = vec![recipient_bare.clone(), own_device_5.clone()];
            let lock_jids = client.build_session_lock_keys(&all_dm_jids).await;

            // Recipient lock key must be BARE (device 0), matching decrypt path
            assert_eq!(
                recipient_bare.to_protocol_address_string(),
                "100000012345678@lid.0"
            );
            assert!(lock_jids.contains(&recipient_bare));

            // Own device lock key must be device-specific
            assert!(lock_jids.contains(&own_device_5));

            // Device-specific recipient key must NOT be present
            assert!(
                !lock_jids.contains(&recipient_device33),
                "recipient must NOT use device-specific address"
            );
        }

        /// Verify bare normalization deduplicates multiple recipient devices.
        #[test]
        fn bare_normalization_deduplicates_recipient_devices() {
            let devices: Vec<Jid> = [
                "100000012345678@lid",
                "100000012345678:5@lid",
                "100000012345678:33@lid",
            ]
            .iter()
            .map(|s| Jid::from_str(s).unwrap())
            .collect();

            // All collapse to the same bare JID
            let bare: Vec<Jid> = devices.iter().map(|j| j.to_non_ad()).collect();
            assert!(bare.windows(2).all(|w| w[0] == w[1]));
            assert_eq!(
                bare[0].to_protocol_address_string(),
                "100000012345678@lid.0"
            );
        }
    }

    // ---- outbound messageSecret capture ---------------------------------

    use crate::store::commands::DeviceCommand;
    use std::sync::Arc;

    async fn seed_pn(client: &Arc<Client>, pn: &str) {
        client
            .persistence_manager
            .process_command(DeviceCommand::SetId(Some(pn.parse().expect("pn"))))
            .await;
    }

    async fn seed_pn_and_lid(client: &Arc<Client>, pn: &str, lid: &str) {
        client
            .persistence_manager
            .process_command(DeviceCommand::SetId(Some(pn.parse().expect("pn"))))
            .await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLid(Some(lid.parse().expect("lid"))))
            .await;
    }

    fn peer_test_account_proto() -> wa::ADVSignedDeviceIdentity {
        wa::ADVSignedDeviceIdentity {
            details: Some(vec![0u8; 32]),
            account_signature_key: Some(vec![0u8; 32]),
            account_signature: Some(vec![0u8; 64]),
            device_signature: Some(vec![0u8; 64]),
        }
    }

    async fn seed_peer_send_state(client: &Arc<Client>, peer: &Jid) {
        use wacore::libsignal::protocol::{
            IdentityKeyPair, KeyPair, PreKeyBundle, SignalProtocolError, UsePQRatchet,
            process_prekey_bundle,
        };

        client
            .persistence_manager
            .process_command(DeviceCommand::SetAccount(Some(peer_test_account_proto())))
            .await;

        let bundle =
            tokio::task::spawn_blocking(|| -> Result<PreKeyBundle, SignalProtocolError> {
                let mut rng = rand::make_rng::<rand::rngs::StdRng>();
                let receiver = IdentityKeyPair::generate(&mut rng);
                let spk = KeyPair::generate(&mut rng);
                let opk = KeyPair::generate(&mut rng);
                let sig = receiver
                    .private_key()
                    .calculate_signature(&spk.public_key.serialize(), &mut rng)?;

                PreKeyBundle::new(
                    1,
                    1u32.into(),
                    Some((1u32.into(), opk.public_key)),
                    1u32.into(),
                    spk.public_key,
                    sig.to_vec(),
                    *receiver.identity_key(),
                )
            })
            .await
            .expect("prekey bundle task")
            .expect("prekey bundle");

        let mut adapter = client.signal_adapter().await;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        process_prekey_bundle(
            &peer.to_protocol_address(),
            &mut adapter.session_store,
            &mut adapter.identity_store,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("peer session");
    }

    fn pdo_request_message(request_type: wa::message::PeerDataOperationRequestType) -> wa::Message {
        wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::PeerDataOperationRequestMessage),
                peer_data_operation_request_message: buffa::MessageField::some(
                    wa::message::PeerDataOperationRequestMessage {
                        peer_data_operation_request_type: Some(request_type),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn peer_pdo_send_path_stamps_history_sync_options() {
        let client = crate::test_utils::create_test_client_with_name("peer_pdo_attrs").await;
        let peer: Jid = "100000000000001@s.whatsapp.net".parse().unwrap();
        seed_peer_send_state(&client, &peer).await;

        let request_id = "PDO_PEER_ATTRS_1";
        let waiter = client
            .wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", request_id));
        let msg =
            pdo_request_message(wa::message::PeerDataOperationRequestType::HistorySyncOnDemand);

        let result = client
            .send_message_impl(
                peer,
                &msg,
                Some(request_id.to_string()),
                true,
                false,
                None,
                vec![],
                None,
            )
            .await;
        assert!(
            result.is_err(),
            "test client has no socket; send should fail after stanza capture"
        );

        let node = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("sent node should be captured")
            .expect("sent node waiter should resolve");
        assert_eq!(
            node.attrs().optional_string("category").unwrap().as_ref(),
            "peer"
        );
        assert_eq!(
            node.attrs()
                .optional_string("push_priority")
                .unwrap()
                .as_ref(),
            "high_force"
        );
        assert_eq!(
            node.attrs()
                .optional_string("privacy_sensitive")
                .unwrap()
                .as_ref(),
            "1"
        );
    }

    #[tokio::test]
    async fn stanza_type_override_sets_wire_type_attr() {
        let client = crate::test_utils::create_test_client_with_name("stanza_type_override").await;
        let peer: Jid = "100000000000003@s.whatsapp.net".parse().unwrap();
        seed_peer_send_state(&client, &peer).await;

        let request_id = "STANZA_TYPE_OVERRIDE_1";
        let waiter = client
            .wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", request_id));
        let msg =
            pdo_request_message(wa::message::PeerDataOperationRequestType::HistorySyncOnDemand);

        // Poll is never the type for this message; it can only come from the override.
        let result = client
            .send_message_impl(
                peer,
                &msg,
                Some(request_id.to_string()),
                true,
                false,
                None,
                vec![],
                Some(wacore::send::StanzaType::Poll),
            )
            .await;
        assert!(
            result.is_err(),
            "test client has no socket; send should fail after stanza capture"
        );

        let node = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("sent node should be captured")
            .expect("sent node waiter should resolve");
        assert_eq!(
            node.attrs().optional_string("type").unwrap().as_ref(),
            wacore::send::StanzaType::Poll.as_wire()
        );
    }

    /// Shared setup for the DM wire-namespace regression tests: own PN/LID +
    /// account, the peer's LID mapping, device-registry entries for both peer
    /// namespaces and self, offline-sync completion, and a seeded Signal
    /// session for the peer's LID device so the offline fanout can encrypt
    /// without a socket. Returns `(peer_pn, peer_lid)`.
    async fn seed_dm_wire_namespace_state(client: &Arc<Client>) -> (Jid, Jid) {
        use wacore::libsignal::protocol::{
            IdentityKeyPair, KeyPair, PreKeyBundle, SignalProtocolError, UsePQRatchet,
            process_prekey_bundle,
        };

        // A LID-addressed DM requires the device's own PN and LID to be known.
        let own_pn: Jid = "111111111111@s.whatsapp.net".parse().unwrap();
        let own_lid: Jid = "222222222222@lid".parse().unwrap();
        client
            .persistence_manager
            .process_command(DeviceCommand::SetId(Some(own_pn.clone())))
            .await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLid(Some(own_lid)))
            .await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetAccount(Some(peer_test_account_proto())))
            .await;

        // The peer is LID-mapped: the wire namespace is then decided solely by
        // the account's migration state.
        let peer_pn: Jid = "100000000000777@s.whatsapp.net".parse().unwrap();
        let peer_lid: Jid = "555000000000777@lid".parse().unwrap();
        client
            .add_lid_pn_mapping(
                peer_lid.user.as_str(),
                peer_pn.user.as_str(),
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .expect("seed lid mapping");

        // Pre-seed the device registry for the peer (both namespaces) and self
        // so the offline send resolves the fanout from cache instead of
        // blocking on a network device-list fetch (which would time out with
        // no socket).
        for user in [
            peer_lid.user.to_string(),
            peer_pn.user.to_string(),
            own_pn.user.to_string(),
        ] {
            client
                .update_device_list(wacore::store::traits::DeviceListRecord {
                    user,
                    devices: vec![wacore::store::traits::DeviceInfo {
                        device_id: 0,
                        key_index: None,
                    }],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: None,
                })
                .await
                .expect("seed device registry");
        }

        // The test client never connects, so the send's `ensure_e2e_sessions`
        // would otherwise block on `wait_for_offline_delivery_end` until timeout.
        client.complete_offline_sync(0);

        // Seed a Signal session for the peer's LID device so the offline fanout
        // can encrypt without fetching prekeys over the (absent) socket. The
        // session lives under the LID address in both tests: Signal addressing
        // is LID-first regardless of the wire namespace (WAWebSignalAddress).
        let lid_addr = peer_lid.to_non_ad();
        let bundle =
            tokio::task::spawn_blocking(|| -> Result<PreKeyBundle, SignalProtocolError> {
                let mut rng = rand::make_rng::<rand::rngs::StdRng>();
                let receiver = IdentityKeyPair::generate(&mut rng);
                let spk = KeyPair::generate(&mut rng);
                let opk = KeyPair::generate(&mut rng);
                let sig = receiver
                    .private_key()
                    .calculate_signature(&spk.public_key.serialize(), &mut rng)?;
                PreKeyBundle::new(
                    1,
                    1u32.into(),
                    Some((1u32.into(), opk.public_key)),
                    1u32.into(),
                    spk.public_key,
                    sig.to_vec(),
                    *receiver.identity_key(),
                )
            })
            .await
            .expect("prekey bundle task")
            .expect("prekey bundle");
        {
            let mut adapter = client.signal_adapter().await;
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            process_prekey_bundle(
                &lid_addr.to_protocol_address(),
                &mut adapter.session_store,
                &mut adapter.identity_store,
                &bundle,
                &mut rng,
                UsePQRatchet::No,
            )
            .await
            .expect("peer lid session");
        }

        (peer_pn, peer_lid)
    }

    /// Regression for #730: on a 1:1-LID-migrated account, a DM to a
    /// LID-mapped peer must address the outer `<message to>` by LID, matching
    /// the LID `<participants>`. Pre-fix the outer `to` kept the caller's PN,
    /// so a PN-to over LID participants was rejected wholesale by the server
    /// with `ack error="400"` and never delivered (while the send still
    /// returned Ok). WAWebSendMsgCreateFanoutStanza builds the whole stanza
    /// from one CHAT_JID (the LID after migration).
    #[tokio::test]
    async fn dm_to_lid_mapped_peer_addresses_outer_to_by_lid() {
        let client = crate::test_utils::create_test_client_with_name("lid_dm_to").await;
        let (peer_pn, peer_lid) = seed_dm_wire_namespace_state(&client).await;

        // LID wire addressing is gated on the account being 1:1-LID-migrated.
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLidMigrated(true))
            .await;

        let request_id = "LID_DM_TO_1";
        let waiter = client
            .wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", request_id));
        let msg = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        // Caller passes the PN form; the resolved namespace must win on the wire.
        let result = client
            .send_message_impl(
                peer_pn,
                &msg,
                Some(request_id.to_string()),
                false,
                false,
                None,
                vec![],
                None,
            )
            .await;
        assert!(
            result.is_err(),
            "test client has no socket; send captures the stanza then errors"
        );

        let node = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("sent node should be captured")
            .expect("sent node waiter should resolve");

        // The fix: outer `<message to>` is the LID, not the caller's PN.
        let to_str = node
            .attrs()
            .optional_string("to")
            .expect("message has a to")
            .into_owned();
        let to_jid: Jid = to_str.parse().expect("to parses");
        assert!(
            to_jid.is_lid(),
            "outer <message to> must be LID to match the LID participants, got {to_str}"
        );
        assert_eq!(
            to_jid.user.as_str(),
            peer_lid.user.as_str(),
            "outer to user must be the peer LID"
        );

        // Uniformity guard: every <participants>/<to> is LID too (no mix).
        let participants = node
            .get_optional_child("participants")
            .expect("stanza has participants");
        let entries = participants.children().expect("participants has children");
        assert!(
            !entries.is_empty(),
            "fanout must target at least the recipient"
        );
        for entry in entries {
            let pj: Jid = entry
                .attrs()
                .optional_string("jid")
                .expect("participant jid")
                .parse()
                .expect("participant jid parses");
            assert!(
                pj.is_lid(),
                "participant {pj} must be LID (uniform namespace)"
            );
        }
    }

    /// Regression for #941: an account that is NOT 1:1-LID-migrated must keep
    /// DM wire addressing on PN even with a cached LID mapping — the server
    /// 400-nacks LID-addressed DMs from unmigrated accounts. WA Web only
    /// addresses 1:1 chats by LID once `Lid1X1MigrationUtils.isLidMigrated()`.
    #[tokio::test]
    async fn dm_from_unmigrated_account_addresses_outer_to_by_pn() {
        let client = crate::test_utils::create_test_client_with_name("pn_dm_to").await;
        let (peer_pn, _peer_lid) = seed_dm_wire_namespace_state(&client).await;

        let request_id = "PN_DM_TO_1";
        let waiter = client
            .wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("id", request_id));
        let msg = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        let result = client
            .send_message_impl(
                peer_pn.clone(),
                &msg,
                Some(request_id.to_string()),
                false,
                false,
                None,
                vec![],
                None,
            )
            .await;
        assert!(
            result.is_err(),
            "test client has no socket; send captures the stanza then errors"
        );

        let node = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("sent node should be captured")
            .expect("sent node waiter should resolve");

        let to_str = node
            .attrs()
            .optional_string("to")
            .expect("message has a to")
            .into_owned();
        let to_jid: Jid = to_str.parse().expect("to parses");
        assert!(
            to_jid.is_pn(),
            "outer <message to> must stay PN on an unmigrated account, got {to_str}"
        );
        assert_eq!(
            to_jid.user.as_str(),
            peer_pn.user.as_str(),
            "outer to user must be the peer PN"
        );

        // Uniformity guard: every <participants>/<to> is PN too (no mix).
        let participants = node
            .get_optional_child("participants")
            .expect("stanza has participants");
        let entries = participants.children().expect("participants has children");
        assert!(
            !entries.is_empty(),
            "fanout must target at least the recipient"
        );
        for entry in entries {
            let pj: Jid = entry
                .attrs()
                .optional_string("jid")
                .expect("participant jid")
                .parse()
                .expect("participant jid parses");
            assert!(
                pj.is_pn(),
                "participant {pj} must be PN (uniform namespace)"
            );
        }
    }

    /// Newsletter JIDs must be rejected at the E2E send path root (covers the
    /// mis-routed pin/edit/revoke producers that call send_message_impl directly).
    #[tokio::test]
    async fn newsletter_jid_rejected_on_e2e_send_path() {
        let client = crate::test_utils::create_test_client_with_name("newsletter_e2e_guard").await;
        let channel: Jid = "120363000000000001@newsletter".parse().unwrap();
        let msg = wa::Message {
            conversation: Some("x".to_string()),
            ..Default::default()
        };
        let err = client
            .send_message_impl(channel, &msg, None, false, false, None, vec![], None)
            .await
            .expect_err("newsletter JID must be rejected on the E2E send path");
        assert!(
            err.to_string().to_lowercase().contains("newsletter"),
            "error should name the newsletter mis-route, got: {err}"
        );
    }

    /// The pin producer routes through send_message_impl, so a newsletter pin is
    /// rejected rather than building an encrypted fanout against a channel.
    #[tokio::test]
    async fn pin_message_rejects_newsletter() {
        let client = crate::test_utils::create_test_client_with_name("newsletter_pin_guard").await;
        let channel: Jid = "120363000000000002@newsletter".parse().unwrap();
        let key = wa::MessageKey {
            remote_jid: Some(channel.to_string()),
            from_me: Some(true),
            id: Some("MID".to_string()),
            participant: None,
        };
        let err = client
            .pin_message(channel, key, PinDuration::Days7)
            .await
            .expect_err("pinning a newsletter message must be rejected");
        assert!(
            err.to_string().to_lowercase().contains("newsletter"),
            "error should name the newsletter mis-route, got: {err}"
        );
    }

    /// Newsletter edit: plaintext `<message edit="3">` keyed by server_id, with the
    /// new content in `<plaintext>`. Keyed by the message id STRING (not server_id),
    /// and a text edit carries no mediatype.
    #[test]
    fn build_newsletter_edit_node_emits_plaintext_edit() {
        use buffa::Message as _;
        let to: Jid = "120363000000000001@newsletter".parse().unwrap();
        let content = wa::Message {
            conversation: Some("edited text".to_string()),
            ..Default::default()
        };
        let node =
            build_newsletter_edit_node(&to, "3EB0EDITTARGET", NewsletterEdit::Edit(&content));

        let mut a = node.attrs();
        assert_eq!(a.optional_string("id").unwrap().as_ref(), "3EB0EDITTARGET");
        assert_eq!(a.optional_string("type").unwrap().as_ref(), "text");
        assert_eq!(a.optional_string("edit").unwrap().as_ref(), "3");

        let pt = node
            .get_optional_child("plaintext")
            .expect("plaintext child");
        assert!(
            pt.attrs().optional_string("mediatype").is_none(),
            "a text edit must not carry a mediatype attr"
        );
        let bytes = match pt.content.as_ref() {
            Some(wacore_binary::NodeContent::Bytes(b)) => b.clone(),
            other => panic!("expected plaintext bytes, got {other:?}"),
        };
        let decoded = wa::Message::decode_from_slice(bytes.as_slice()).expect("decode plaintext");
        assert_eq!(decoded.conversation.as_deref(), Some("edited text"));
    }

    /// Media newsletter edit: type="media" + `<plaintext mediatype="image">`.
    #[test]
    fn build_newsletter_edit_node_media_edit() {
        let to: Jid = "120363000000000001@newsletter".parse().unwrap();
        let content = wa::Message {
            image_message: buffa::MessageField::some(wa::message::ImageMessage {
                caption: Some("new caption".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let node = build_newsletter_edit_node(&to, "3EB0MEDIA", NewsletterEdit::Edit(&content));

        let mut a = node.attrs();
        assert_eq!(a.optional_string("id").unwrap().as_ref(), "3EB0MEDIA");
        assert_eq!(a.optional_string("type").unwrap().as_ref(), "media");
        assert_eq!(a.optional_string("edit").unwrap().as_ref(), "3");
        let pt = node
            .get_optional_child("plaintext")
            .expect("plaintext child");
        assert_eq!(
            pt.attrs().optional_string("mediatype").unwrap().as_ref(),
            "image"
        );
    }

    /// Newsletter revoke: plaintext `<message type="text" edit="8">` keyed by the
    /// message id STRING, with an empty `<plaintext>`.
    #[test]
    fn build_newsletter_edit_node_revoke_is_empty_plaintext() {
        let to: Jid = "120363000000000002@newsletter".parse().unwrap();
        let node = build_newsletter_edit_node(&to, "3EB0REVOKETARGET", NewsletterEdit::Revoke);

        let mut a = node.attrs();
        assert_eq!(
            a.optional_string("id").unwrap().as_ref(),
            "3EB0REVOKETARGET"
        );
        assert_eq!(a.optional_string("type").unwrap().as_ref(), "text");
        assert_eq!(a.optional_string("edit").unwrap().as_ref(), "8");

        let pt = node
            .get_optional_child("plaintext")
            .expect("plaintext child");
        let empty = match pt.content.as_ref() {
            None => true,
            Some(wacore_binary::NodeContent::Bytes(b)) => b.is_empty(),
            _ => false,
        };
        assert!(empty, "revoke must carry an empty plaintext");
    }

    /// The public newsletter().edit_message wrapper emits the plaintext edit stanza
    /// keyed by the message id it was given.
    #[tokio::test]
    async fn newsletter_edit_message_wrapper_sends_plaintext_edit() {
        let client = crate::test_utils::create_test_client_with_name("nl_edit_wrap").await;
        let channel: Jid = "120363000000000001@newsletter".parse().unwrap();
        let waiter =
            client.wait_for_sent_node(crate::client::NodeFilter::tag("message").attr("edit", "3"));
        let content = wa::Message {
            conversation: Some("edited".to_string()),
            ..Default::default()
        };
        // No socket on the test client: send_node captures the node, then errors.
        let _ = client
            .newsletter()
            .edit_message(&channel, "TARGETMID", content)
            .await;

        let node = tokio::time::timeout(std::time::Duration::from_secs(1), waiter)
            .await
            .expect("sent node captured")
            .expect("waiter resolves");
        let mut a = node.attrs();
        assert_eq!(a.optional_string("id").unwrap().as_ref(), "TARGETMID");
        assert_eq!(a.optional_string("edit").unwrap().as_ref(), "3");
    }

    /// The newsletter edit/revoke methods reject non-newsletter JIDs, so a misuse
    /// cannot send plaintext content to a DM/group (it would not be E2E-encrypted).
    #[tokio::test]
    async fn newsletter_edit_revoke_reject_non_newsletter_jid() {
        let client = crate::test_utils::create_test_client_with_name("nl_reject_nonchannel").await;
        let dm: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let group: Jid = "120363000000000009@g.us".parse().unwrap();

        let e1 = client
            .newsletter()
            .edit_message(
                &dm,
                "MID",
                wa::Message {
                    conversation: Some("x".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("edit_message must reject a DM JID");
        assert!(e1.to_string().to_lowercase().contains("newsletter"));

        let e2 = client
            .newsletter()
            .revoke_message(&group, "MID")
            .await
            .expect_err("revoke_message must reject a group JID");
        assert!(e2.to_string().to_lowercase().contains("newsletter"));
    }

    /// An empty message_id (NewsletterMessage.message_id may be empty if the server
    /// omitted the id) is rejected rather than sending a target-less id="" stanza.
    #[tokio::test]
    async fn newsletter_edit_revoke_reject_empty_message_id() {
        let client = crate::test_utils::create_test_client_with_name("nl_reject_empty_id").await;
        let channel: Jid = "120363000000000001@newsletter".parse().unwrap();

        let e1 = client
            .newsletter()
            .edit_message(
                &channel,
                "",
                wa::Message {
                    conversation: Some("x".to_string()),
                    ..Default::default()
                },
            )
            .await
            .expect_err("edit_message must reject an empty message_id");
        assert!(e1.to_string().to_lowercase().contains("message_id"));

        let e2 = client
            .newsletter()
            .revoke_message(&channel, "")
            .await
            .expect_err("revoke_message must reject an empty message_id");
        assert!(e2.to_string().to_lowercase().contains("message_id"));
    }

    #[tokio::test]
    async fn persist_outbound_msg_secret_writes_under_chat_sender_id() {
        let client = crate::test_utils::create_test_client_with_name("secret_chat_id").await;
        seed_pn(&client, "5511000000001:0@s.whatsapp.net").await;
        let chat: Jid = "5511777776666@s.whatsapp.net".parse().unwrap();
        let sender: Jid = "5511000000001:0@s.whatsapp.net".parse().unwrap();
        let secret = [0x55u8; 32];
        client
            .persist_outbound_msg_secret(
                &chat,
                &sender,
                "MID_1",
                &secret,
                wacore::msg_secret::RetentionClass::Text,
            )
            .await;
        client.msg_secret_buffer.wait_flushed().await;
        let got = client
            .persistence_manager
            .backend()
            .get_msg_secret(
                "5511777776666@s.whatsapp.net",
                "5511000000001@s.whatsapp.net",
                "MID_1",
            )
            .await
            .expect("get");
        assert_eq!(got.as_deref(), Some(&secret[..]));
    }

    #[tokio::test]
    async fn persist_outbound_msg_secret_strips_devices_in_key() {
        let client = crate::test_utils::create_test_client_with_name("secret_strip").await;
        let chat_with_dev: Jid = "5511777776666:7@s.whatsapp.net".parse().unwrap();
        let sender_with_dev: Jid = "5511000000001:3@s.whatsapp.net".parse().unwrap();
        client
            .persist_outbound_msg_secret(
                &chat_with_dev,
                &sender_with_dev,
                "MID_4",
                &[2u8; 32],
                wacore::msg_secret::RetentionClass::Text,
            )
            .await;
        client.msg_secret_buffer.wait_flushed().await;
        let got = client
            .persistence_manager
            .backend()
            .get_msg_secret(
                "5511777776666@s.whatsapp.net",
                "5511000000001@s.whatsapp.net",
                "MID_4",
            )
            .await
            .unwrap();
        assert_eq!(
            got.as_deref(),
            Some(&[2u8; 32][..]),
            "chat and sender must be stored non-AD"
        );
    }

    #[tokio::test]
    async fn dm_sender_identity_picks_lid_for_bot_else_pn() {
        let client = crate::test_utils::create_test_client_with_name("dm_id_pick").await;
        seed_pn_and_lid(
            &client,
            "5511000000001:0@s.whatsapp.net",
            "999888777666555:0@lid",
        )
        .await;
        let bot_chat: Jid = "867051314767696@bot".parse().unwrap();
        let pn_chat: Jid = "5511777776666@s.whatsapp.net".parse().unwrap();
        let lid_chat: Jid = "111222333444555@lid".parse().unwrap();
        assert_eq!(
            client
                .dm_sender_identity_for(&bot_chat)
                .await
                .map(|j| j.to_non_ad_string()),
            Some("999888777666555@lid".to_string()),
            "bot chats must resolve to our LID"
        );
        assert_eq!(
            client
                .dm_sender_identity_for(&pn_chat)
                .await
                .map(|j| j.to_non_ad_string()),
            Some("5511000000001@s.whatsapp.net".to_string()),
            "PN chats must resolve to our PN"
        );
        // LID-DM is presently routed under PN; flagged as a follow-up only
        // because production hasn't surfaced it. Documented behaviour.
        assert_eq!(
            client
                .dm_sender_identity_for(&lid_chat)
                .await
                .map(|j| j.to_non_ad_string()),
            Some("5511000000001@s.whatsapp.net".to_string()),
        );
    }

    /// Regression for Codex P2 (LID-mode group bot replies): the persisted
    /// sender must match whatever `prepare_group_stanza` picked for the
    /// group's addressing_mode, surfaced via `PreparedGroupStanza.sender_identity`.
    #[tokio::test]
    async fn persist_uses_group_sender_identity_for_lid_mode_groups() {
        let client = crate::test_utils::create_test_client_with_name("secret_lid_group").await;
        seed_pn_and_lid(
            &client,
            "5511000000001:0@s.whatsapp.net",
            "999888777666555:0@lid",
        )
        .await;
        // Simulate a LID-mode group: addressing identity is our LID, not PN.
        let group_chat: Jid = "120363021033254949@g.us".parse().unwrap();
        let lid_sender: Jid = "999888777666555:0@lid".parse().unwrap();
        let secret = [0x4Du8; 32];
        client
            .persist_outbound_msg_secret(
                &group_chat,
                &lid_sender,
                "GROUP_MID",
                &secret,
                wacore::msg_secret::RetentionClass::Text,
            )
            .await;
        client.msg_secret_buffer.wait_flushed().await;
        let got = client
            .persistence_manager
            .backend()
            .get_msg_secret(
                "120363021033254949@g.us",
                "999888777666555@lid",
                "GROUP_MID",
            )
            .await
            .unwrap();
        assert_eq!(
            got.as_deref(),
            Some(&secret[..]),
            "LID-mode group secrets must key under our LID, not PN"
        );
        let under_pn = client
            .persistence_manager
            .backend()
            .get_msg_secret(
                "120363021033254949@g.us",
                "5511000000001@s.whatsapp.net",
                "GROUP_MID",
            )
            .await
            .unwrap();
        assert!(
            under_pn.is_none(),
            "LID-mode group must NOT key under our PN"
        );
    }

    /// Regression: `wacore::send::prepare_dm_stanza` mints the
    /// `message_secret` on a CLONE of the caller's message. Verify the secret
    /// is surfaced via `PreparedDmStanza.message_secret` so the post-send hook
    /// can persist it -- without this an original-message-based check would
    /// miss every ordinary outbound bot prompt.
    #[test]
    fn prepared_dm_stanza_exposes_generated_message_secret() {
        use wacore::reporting_token::generate_reporting_token;

        let msg = wa::Message {
            conversation: Some("hi bot".into()),
            ..Default::default()
        };
        let to: Jid = "867051314767696@bot".parse().unwrap();
        let result = generate_reporting_token(&msg, "MID_X", &to, &to, None);
        assert!(
            result.is_some(),
            "ordinary text messages must produce a reporting token + secret"
        );
        let result = result.unwrap();
        assert_eq!(result.message_secret.len(), 32);
        // PreparedDmStanza/PreparedGroupStanza now carry this exact array
        // through to send_message_impl which calls persist_outbound_msg_secret.
        let prepared = wacore::send::PreparedDmStanza {
            node: wacore_binary::builder::NodeBuilder::new("message").build(),
            phash: None,
            message_secret: Some(result.message_secret),
        };
        assert_eq!(prepared.message_secret.as_ref().unwrap().len(), 32);
    }
}

#[cfg(test)]
mod jid_into_convention {
    use super::*;

    /// Compile-time guard for the `impl Into<Jid>` convention: every core
    /// method must accept BOTH an owned `Jid` (move, zero copy) and a `&Jid`
    /// (one clone via `From<&Jid>`). Never executed; compilation is the test.
    #[allow(dead_code)]
    async fn both_call_styles_compile(client: &crate::client::Client, jid: Jid) {
        let msg = wa::Message::default();
        let _ = client.send_message(&jid, msg.clone()).await;
        let _ = client
            .send_message_with_options(&jid, msg.clone(), SendOptions::default())
            .await;
        let _ = client.forward_message(&jid, &msg).await;
        let _ = client
            .edit_message(&jid, "ID", wa::Message::default())
            .await;
        let _ = client.revoke_message(&jid, "ID", RevokeType::Sender).await;
        let _ = client
            .pin_message(&jid, wa::MessageKey::default(), PinDuration::default())
            .await;
        let _ = client.unpin_message(&jid, wa::MessageKey::default()).await;
        let _ = client
            .send_reaction(&jid, wa::MessageKey::default(), "x")
            .await;
        let _ = client
            .keep_message(&jid, wa::MessageKey::default(), true)
            .await;
        // Owned style: moves, no clone. Each method consumes its own copy so
        // the whole core surface is pinned, not just send_message.
        let _ = client.send_message(jid.clone(), msg.clone()).await;
        let _ = client
            .send_message_with_options(jid.clone(), msg.clone(), SendOptions::default())
            .await;
        let _ = client.forward_message(jid.clone(), &msg).await;
        let _ = client
            .edit_message(jid.clone(), "ID", wa::Message::default())
            .await;
        let _ = client
            .revoke_message(jid.clone(), "ID", RevokeType::Sender)
            .await;
        let _ = client
            .pin_message(
                jid.clone(),
                wa::MessageKey::default(),
                PinDuration::default(),
            )
            .await;
        let _ = client
            .unpin_message(jid.clone(), wa::MessageKey::default())
            .await;
        let _ = client
            .send_reaction(jid.clone(), wa::MessageKey::default(), "x")
            .await;
        let _ = client
            .keep_message(jid, wa::MessageKey::default(), true)
            .await;
    }
}

#[cfg(test)]
mod future_size_tests {
    /// The public send futures embed in every event-handler and spawned-task
    /// frame, so their size is a per-event heap cost. Keep them pointer-scale
    /// (measured 64-128 B; the bound leaves slack only for layout drift).
    #[tokio::test]
    async fn send_futures_stay_small() {
        let client = crate::test_utils::create_test_client().await;
        let jid: wacore_binary::jid::Jid = "5511999990000@s.whatsapp.net".parse().unwrap();
        let msg = waproto::whatsapp::Message::default();

        let f = client.send_message(jid.clone(), msg.clone());
        assert!(std::mem::size_of_val(&f) <= 192, "send_message future grew");
        drop(f);
        let f = client.send_text(jid.clone(), "x");
        assert!(std::mem::size_of_val(&f) <= 192, "send_text future grew");
        drop(f);
        let f = client.forward_message(jid.clone(), &msg);
        assert!(
            std::mem::size_of_val(&f) <= 192,
            "forward_message future grew"
        );
        drop(f);
        let f = client.send_message_with_options(jid, msg, Default::default());
        assert!(
            std::mem::size_of_val(&f) <= 192,
            "send_message_with_options future grew"
        );
        drop(f);
    }
}
