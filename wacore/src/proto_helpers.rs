use std::str::FromStr;
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

/// Single source of truth for the message types that carry a `context_info`
/// field. Consumed by `for_each_context_info_message!`, `set_context_info`
/// (via an inlined `try_attach!`), `get_ephemeral_expiration`, and
/// `set_ephemeral_expiration`. Add new WA message types with context_info here.
macro_rules! with_context_info_fields {
    ($callback:ident!($($prefix:tt)*)) => {
        $callback!($($prefix)*
            extended_text_message,
            image_message,
            video_message,
            ptv_message,
            audio_message,
            document_message,
            sticker_message,
            location_message,
            live_location_message,
            contact_message,
            contacts_array_message,
            buttons_message,
            buttons_response_message,
            list_message,
            list_response_message,
            template_message,
            template_button_reply_message,
            interactive_message,
            interactive_response_message,
            poll_creation_message,
            poll_creation_message_v2,
            poll_creation_message_v3,
            product_message,
            order_message,
            group_invite_message,
            event_message,
            sticker_pack_message,
            newsletter_admin_invite_message,
        )
    };
}

/// Applies an operation to all message types that have a `context_info` field.
///
/// Usage:
/// ```ignore
/// for_each_context_info_message!(msg, ctx, {
///     ctx.mentioned_jid.clear();
/// });
/// ```
macro_rules! for_each_context_info_message {
    ($msg:expr, $ctx:ident, $body:block) => {
        with_context_info_fields!(for_each_context_info_impl!($msg, $ctx, $body,))
    };
}

macro_rules! for_each_context_info_impl {
    ($msg:expr, $ctx:ident, $body:block, $($field:ident),+ $(,)?) => {
        $(
            if let Some(ref mut m) = $msg.$field {
                if let Some(ref mut $ctx) = m.context_info $body
            }
        )+
    };
}

/// Returns `Some(ctx)` for the first message variant carrying a `ContextInfo`,
/// short-circuiting on match (`break 'find`). Read-only variant of
/// [`for_each_context_info_message!`].
macro_rules! find_context_info_ref {
    ($msg:expr) => {{ with_context_info_fields!(find_context_info_impl!($msg,)) }};
}

macro_rules! find_context_info_impl {
    ($msg:expr, $($field:ident),+ $(,)?) => {{
        let mut found: Option<&wa::ContextInfo> = None;
        $(
            if found.is_none()
                && let Some(ref m) = $msg.$field
                && let Some(ref ctx) = m.context_info
            {
                found = Some(ctx);
            }
        )+
        found
    }};
}

/// Extension trait for wa::Message
pub trait MessageExt {
    /// Recursively unwraps ephemeral/view-once/document_with_caption/edited wrappers to get the core message.
    fn get_base_message(&self) -> &wa::Message;
    /// Consuming version of [`get_base_message`]. Moves the innermost message out of
    /// wrapper types (device_sent, ephemeral, view_once, etc.) without cloning.
    fn into_base_message(self) -> wa::Message;
    fn is_ephemeral(&self) -> bool;
    /// Covers the legacy `view_once_message{_v2,_v2_extension}` wrappers (in any
    /// nesting order under `device_sent`/`ephemeral`) and the inline `view_once`
    /// flag on modern image/video/audio/extended-text payloads.
    fn is_view_once(&self) -> bool;
    /// Gets the caption for media messages (Image, Video, Document).
    fn get_caption(&self) -> Option<&str>;
    /// Gets the primary text content of a message (from conversation or extendedTextMessage).
    fn text_content(&self) -> Option<&str>;

    /// Prepares a message to be quoted by stripping nested mentions and quote-chain fields.
    ///
    /// WhatsApp Web builds a fresh `ContextInfo` and does not carry over nested mentions.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wacore::proto_helpers::MessageExt;
    ///
    /// let context_info = wa::ContextInfo {
    ///     stanza_id: Some(message_id.clone()),
    ///     participant: Some(sender_jid.to_string()),
    ///     quoted_message: Some(original_message.prepare_for_quote()),
    ///     ..Default::default()
    /// };
    /// ```
    fn prepare_for_quote(&self) -> Box<wa::Message>;

    /// Prepares a copy of this message to be forwarded.
    ///
    /// Mirrors WA Web `WAWebChatForwardMessage` + `getMsgForwardingScoreWhenForwarded`:
    /// strips the reply/quote chain and mentions, sets `context_info.is_forwarded`,
    /// bumps `forwarding_score` (source score plus 1 if the source was already shown
    /// as forwarded, jumping to the `127` frequently-forwarded sentinel at `>= 5`),
    /// and drops `message_context_info` so the send path mints a fresh
    /// `message_secret` instead of reusing the source's.
    ///
    /// Forwards the message body as-is, so existing media is relayed from the same
    /// CDN blob (mediaKey/url are carried over) rather than re-uploaded.
    fn prepare_for_forward(&self) -> Box<wa::Message>;

    /// Sets context_info on the first supported message field.
    ///
    /// Returns `true` if context was set, otherwise `false`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wacore::proto_helpers::MessageExt;
    ///
    /// let mut reply = wa::Message {
    ///     image_message: Some(Box::new(wa::message::ImageMessage {
    ///         // ... image data
    ///         ..Default::default()
    ///     })),
    ///     ..Default::default()
    /// };
    ///
    /// let context = wa::ContextInfo {
    ///     stanza_id: Some("original-msg-id".to_string()),
    ///     participant: Some("sender@s.whatsapp.net".to_string()),
    ///     quoted_message: Some(original_msg.prepare_for_quote()),
    ///     ..Default::default()
    /// };
    ///
    /// reply.set_context_info(context);
    /// ```
    fn set_context_info(&mut self, context: wa::ContextInfo) -> bool;

    /// Reads `context_info.expiration` from the first message type that has it.
    fn get_ephemeral_expiration(&self) -> Option<u32>;

    /// Sets `context_info.expiration` on the first message type found, creating
    /// a default `context_info` if needed. A bare `conversation` body is
    /// promoted to `extended_text_message { text, context_info { expiration } }`
    /// (mirrors `WAWebMessageSendUtils`). Returns `true` on success or
    /// promotion, `false` only when no body can carry the timer.
    fn set_ephemeral_expiration(&mut self, expiration: u32) -> bool;

    /// `context_info.is_forwarded == Some(true)` on the first base message
    /// that carries a context_info. Mirrors WA Web's `x.isForwarded`
    /// guard in `processRenderableMessages` (which skips caching
    /// `messageSecret` for forwarded payloads).
    fn is_forwarded(&self) -> bool;

    /// `true` if `context_info.mentioned_jid` on any base message contains
    /// a JID whose user-form ends with `@bot`. Mirrors WA Web's
    /// `mentionedJidList.find(jid.isBot())` lookup used to derive
    /// `invokedBotWid` when `messageSecret` is present.
    fn mentions_any_bot(&self) -> bool;
}

impl MessageExt for wa::Message {
    fn get_base_message(&self) -> &wa::Message {
        let mut current = self;
        if let Some(msg) = self
            .device_sent_message
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        if let Some(msg) = current
            .ephemeral_message
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        if let Some(msg) = current
            .view_once_message
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        if let Some(msg) = current
            .view_once_message_v2
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        if let Some(msg) = current
            .view_once_message_v2_extension
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        if let Some(msg) = current
            .document_with_caption_message
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        if let Some(msg) = current
            .edited_message
            .as_ref()
            .and_then(|m| m.message.as_ref())
        {
            current = msg;
        }
        current
    }

    fn into_base_message(mut self) -> wa::Message {
        macro_rules! peel_wrapper {
            ($field:ident) => {
                if let Some(mut wrapper) = self.$field.take() {
                    if let Some(msg) = wrapper.message.take() {
                        self = *msg;
                    } else {
                        self.$field = Some(wrapper);
                    }
                }
            };
        }

        peel_wrapper!(device_sent_message);
        peel_wrapper!(ephemeral_message);
        peel_wrapper!(view_once_message);
        peel_wrapper!(view_once_message_v2);
        peel_wrapper!(view_once_message_v2_extension);
        peel_wrapper!(document_with_caption_message);
        peel_wrapper!(edited_message);
        self
    }

    fn is_ephemeral(&self) -> bool {
        self.ephemeral_message.is_some()
    }

    fn is_view_once(&self) -> bool {
        let mut current = self;
        loop {
            if current.view_once_message.is_some()
                || current.view_once_message_v2.is_some()
                || current.view_once_message_v2_extension.is_some()
            {
                return true;
            }
            if let Some(inner) = current
                .device_sent_message
                .as_ref()
                .and_then(|m| m.message.as_ref())
            {
                current = inner;
                continue;
            }
            if let Some(inner) = current
                .ephemeral_message
                .as_ref()
                .and_then(|m| m.message.as_ref())
            {
                current = inner;
                continue;
            }
            break;
        }

        let base = self.get_base_message();
        matches!(
            base.image_message.as_deref().and_then(|m| m.view_once),
            Some(true)
        ) || matches!(
            base.video_message.as_deref().and_then(|m| m.view_once),
            Some(true)
        ) || matches!(
            base.audio_message.as_deref().and_then(|m| m.view_once),
            Some(true)
        ) || matches!(
            base.extended_text_message
                .as_deref()
                .and_then(|m| m.view_once),
            Some(true)
        )
    }

    fn get_caption(&self) -> Option<&str> {
        let base = self.get_base_message();
        if let Some(msg) = &base.image_message {
            return msg.caption.as_deref();
        }
        if let Some(msg) = &base.video_message {
            return msg.caption.as_deref();
        }
        if let Some(msg) = &base.document_message {
            return msg.caption.as_deref();
        }
        None
    }

    fn text_content(&self) -> Option<&str> {
        let base = self.get_base_message();
        if let Some(text) = &base.conversation
            && !text.is_empty()
        {
            return Some(text);
        }
        if let Some(ext_text) = &base.extended_text_message
            && let Some(text) = &ext_text.text
        {
            return Some(text);
        }
        None
    }

    fn prepare_for_quote(&self) -> Box<wa::Message> {
        let mut msg = self.clone();
        strip_nested_context_info(&mut msg, false);
        Box::new(msg)
    }

    fn prepare_for_forward(&self) -> Box<wa::Message> {
        // WA Web's `FREQUENTLY_FORWARDED_SENTINEL` (Constants/Deprecated): the
        // score saturates by jumping here at the >= 5 threshold, not at 5.
        const FREQUENTLY_FORWARDED_SENTINEL: u32 = 127;

        let mut msg = self.clone();
        // Reuse the quote/mention stripping; forwarding always breaks the chain,
        // including for bot participants (no quote-preserve exception).
        strip_nested_context_info(&mut msg, true);
        // WA Web forward omits messageSecret; the send path generates a fresh one.
        msg.message_context_info = None;

        macro_rules! set_forward {
            ($($field:ident),+ $(,)?) => {
                $(
                    if let Some(ref mut m) = msg.$field {
                        let ctx = m
                            .context_info
                            .get_or_insert_with(|| Box::new(wa::ContextInfo::default()));
                        let n = ctx
                            .forwarding_score
                            .unwrap_or(0)
                            .saturating_add(u32::from(ctx.is_forwarded.unwrap_or(false)));
                        ctx.forwarding_score = Some(if n >= 5 {
                            FREQUENTLY_FORWARDED_SENTINEL
                        } else {
                            n
                        });
                        ctx.is_forwarded = Some(true);
                        return Box::new(msg);
                    }
                )+
            };
        }
        with_context_info_fields!(set_forward!());

        // Bare conversation carries no context_info; promote it like the other
        // setters do so the forward marker can attach.
        if let Some(text) = msg.conversation.take() {
            msg.extended_text_message = Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(text),
                context_info: Some(Box::new(wa::ContextInfo {
                    is_forwarded: Some(true),
                    forwarding_score: Some(0),
                    ..Default::default()
                })),
                ..Default::default()
            }));
        }
        Box::new(msg)
    }

    fn set_context_info(&mut self, context: wa::ContextInfo) -> bool {
        macro_rules! try_attach {
            ($($field:ident),+ $(,)?) => {
                $(
                    if let Some(ref mut m) = self.$field {
                        m.context_info = Some(Box::new(context));
                        return true;
                    }
                )+
            };
        }
        with_context_info_fields!(try_attach!());

        // Promote bare conversation to extended_text_message so the context
        // can attach; matches WAWebMessageSendUtils.
        if let Some(text) = self.conversation.take() {
            self.extended_text_message = Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(text),
                context_info: Some(Box::new(context)),
                ..Default::default()
            }));
            return true;
        }
        false
    }

    fn get_ephemeral_expiration(&self) -> Option<u32> {
        macro_rules! check {
            ($($field:ident),+ $(,)?) => {
                $(
                    if let Some(ref m) = self.$field {
                        if let Some(ref ctx) = m.context_info {
                            if let Some(exp) = ctx.expiration {
                                if exp > 0 {
                                    return Some(exp);
                                }
                            }
                        }
                    }
                )+
            };
        }
        with_context_info_fields!(check!());
        None
    }

    fn set_ephemeral_expiration(&mut self, expiration: u32) -> bool {
        if expiration == 0 {
            return false;
        }
        macro_rules! try_set {
            ($($field:ident),+ $(,)?) => {
                $(
                    if let Some(ref mut m) = self.$field {
                        let ctx = m.context_info.get_or_insert_with(|| Box::new(wa::ContextInfo::default()));
                        ctx.expiration = Some(expiration);
                        return true;
                    }
                )+
            };
        }
        with_context_info_fields!(try_set!());

        // Promote bare conversation so the timer can attach; matches
        // WAWebMessageSendUtils.
        if let Some(text) = self.conversation.take() {
            self.extended_text_message = Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(text),
                context_info: Some(Box::new(wa::ContextInfo {
                    expiration: Some(expiration),
                    ..Default::default()
                })),
                ..Default::default()
            }));
            return true;
        }

        false
    }

    fn is_forwarded(&self) -> bool {
        let base = self.get_base_message();
        find_context_info_ref!(base)
            .and_then(|ctx| ctx.is_forwarded)
            .unwrap_or(false)
    }

    fn mentions_any_bot(&self) -> bool {
        let base = self.get_base_message();
        let Some(ctx) = find_context_info_ref!(base) else {
            return false;
        };
        // Use the canonical `Jid::is_bot()` contract — it covers both the
        // `@bot` server and the legacy PN-form Meta bot (e.g. `1313555…`),
        // matching WA Web's `jid.isBot()`. The list is short and this only
        // runs on the group-mention path (chat isn't already a bot).
        ctx.mentioned_jid
            .iter()
            .filter_map(|s| Jid::from_str(s).ok())
            .any(|jid| jid.is_bot())
    }
}

/// Builds a `keepInChatMessage` body that keeps (or un-keeps) a message in a
/// disappearing chat for everyone.
///
/// Mirrors WA Web `WAWebGenerateKeepInChatMessageProto`: the body carries the
/// kept message's `key`, the keep type (`keep = true` -> `KEEP_FOR_ALL`,
/// `false` -> `UNDO_KEEP_FOR_ALL`), and `timestamp_ms` = the *send* time (not
/// the kept message's). The keep message itself is sent with a fresh
/// `MessageKey` by the send path, so only the target key goes in the body.
pub fn build_keep_in_chat_message(
    key: wa::MessageKey,
    keep: bool,
    timestamp_ms: i64,
) -> wa::Message {
    let keep_type = if keep {
        wa::KeepType::KeepForAll
    } else {
        wa::KeepType::UndoKeepForAll
    };
    wa::Message {
        keep_in_chat_message: Some(wa::message::KeepInChatMessage {
            key: Some(key),
            keep_type: Some(keep_type as i32),
            timestamp_ms: Some(timestamp_ms),
        }),
        ..Default::default()
    }
}

/// Strips nested context_info fields to match WhatsApp Web.
///
/// Clears quote-chain fields plus `mentioned_jid`/`group_mentions` to avoid
/// nested quote chains and accidental mentions. Used by
/// `MessageExt::prepare_for_quote()` and `prepare_for_forward()`.
///
/// `always_clear_quote`: quote sanitizing preserves the quote chain for bot
/// participants (WA Web keeps bot reply context on quotes); forwarding has no
/// such exception and must always break the chain, so it passes `true`.
pub(crate) fn strip_nested_context_info(msg: &mut wa::Message, always_clear_quote: bool) {
    fn clear_nested_context(ctx: &mut wa::ContextInfo, always_clear_quote: bool) {
        // Always clear mentions to avoid accidental tagging.
        ctx.mentioned_jid.clear();
        ctx.group_mentions.clear();

        // WhatsApp Web preserves quote chains for bot participants when quoting;
        // forwarding has no such exception.
        let is_bot = !always_clear_quote
            && ctx
                .participant
                .as_ref()
                .and_then(|p| Jid::from_str(p).ok())
                .is_some_and(|jid| jid.is_bot());

        if !is_bot {
            // Break the nested quote chain.
            ctx.quoted_message = None;
            ctx.stanza_id = None;
            ctx.remote_jid = None;
            ctx.participant = None;
        }
    }

    for_each_context_info_message!(msg, ctx, {
        clear_nested_context(ctx, always_clear_quote);
    });

    // Recurse into wrapper messages.
    macro_rules! recurse_into_wrapper {
        ($($wrapper:ident),+ $(,)?) => {
            $(
                if let Some(ref mut wrapper) = msg.$wrapper {
                    if let Some(ref mut inner) = wrapper.message {
                        strip_nested_context_info(inner, always_clear_quote);
                    }
                }
            )+
        };
    }
    recurse_into_wrapper!(
        ephemeral_message,
        view_once_message,
        view_once_message_v2,
        view_once_message_v2_extension,
        document_with_caption_message,
        edited_message,
    );

    // device_sent_message also contains a nested message.
    if let Some(ref mut wrapper) = msg.device_sent_message
        && let Some(ref mut inner) = wrapper.message
    {
        strip_nested_context_info(inner, always_clear_quote);
    }
}

/// Merges `MessageContextInfo` from the outer and inner messages of a
/// `DeviceSentMessage` wrapper, matching WhatsApp Web's
/// `WAWebDeviceSentMessageProtoUtils.unwrapDeviceSentMessage` logic.
///
/// Merge strategy:
/// - **Base**: all fields from `inner`
/// - **`message_secret`**: inner, falling back to outer
/// - **`message_association`**: inner, falling back to outer
/// - **`limit_sharing_v2`**: always from outer (unconditional override)
/// - **`thread_id`**: inner if non-empty, otherwise outer
/// - **`bot_metadata`**: inner, falling back to outer
pub fn merge_dsm_context(
    inner: Option<wa::MessageContextInfo>,
    outer: Option<&wa::MessageContextInfo>,
) -> Option<wa::MessageContextInfo> {
    match (inner, outer) {
        (None, None) => None,
        (Some(mut inner), None) => {
            // limit_sharing_v2 always comes from outer; clear it when outer is absent
            inner.limit_sharing_v2 = None;
            Some(inner)
        }
        // Inner was cleared by a WA-Web-style hoist; restore the full context the
        // sender moved to the outer message, not just the merge subset.
        (None, Some(outer)) => Some(outer.clone()),
        (Some(mut inner), Some(outer)) => {
            if inner.message_secret.is_none() {
                inner.message_secret = outer.message_secret.clone();
            }
            if inner.message_association.is_none() {
                inner.message_association = outer.message_association.clone();
            }
            // limit_sharing_v2: always from outer (WA Web unconditionally overrides)
            inner.limit_sharing_v2 = outer.limit_sharing_v2;
            if inner.thread_id.is_empty() {
                inner.thread_id = outer.thread_id.clone();
            }
            if inner.bot_metadata.is_none() {
                inner.bot_metadata = outer.bot_metadata.clone();
            }
            Some(inner)
        }
    }
}

/// Builds a quote context for replying to a message.
///
/// This is a standalone function that can be used without `MessageContext`,
/// useful for users who don't use the Bot API.
///
/// # Arguments
/// * `message_id` - The ID of the message being quoted
/// * `sender_jid` - The JID of the sender of the message being quoted
/// * `quoted_message` - The message being quoted
///
/// # Example
///
/// ```ignore
/// use wacore::proto_helpers::{build_quote_context, MessageExt};
///
/// let context = build_quote_context(
///     "3EB0123456789",
///     "1234567890@s.whatsapp.net",
///     &original_message,
/// );
///
/// let reply = wa::Message {
///     extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
///         text: Some("My reply".to_string()),
///         context_info: Some(Box::new(context)),
///         ..Default::default()
///     })),
///     ..Default::default()
/// };
/// ```
pub fn build_quote_context(
    message_id: impl Into<String>,
    sender_jid: impl Into<String>,
    quoted_message: &wa::Message,
) -> wa::ContextInfo {
    wa::ContextInfo {
        stanza_id: Some(message_id.into()),
        participant: Some(sender_jid.into()),
        quoted_message: Some(quoted_message.prepare_for_quote()),
        ..Default::default()
    }
}

/// Builds a quote ContextInfo matching WA Web's `msgContextInfo` + `getQuotedParticipantForContextInfo`.
///
/// `remote_jid` is emitted only for a cross-chat quote (the quoted message's
/// chat differs from `target_chat_jid`), mirroring WA Web's quote-context getter
/// (`remoteJid` set only when `quotedMsg.remote != targetChat`); a same-chat
/// reply omits it. The defense against re-notifying mentions inside the quoted
/// copy is `prepare_for_quote`, not `remote_jid`.
/// `participant`: newsletter uses the channel JID, otherwise the sender.
pub fn build_quote_context_with_info(
    message_id: impl Into<String>,
    sender_jid: &Jid,
    quoted_chat_jid: &Jid,
    target_chat_jid: &Jid,
    quoted_message: &wa::Message,
) -> wa::ContextInfo {
    // remote_jid only for a cross-chat quote, in device-less chat form: a chat
    // reference carries no device, and the compare above is device-insensitive.
    // with_device(0) keeps the agent that @bot/@interop chat JIDs render (to_non_ad
    // would wrongly drop it).
    let remote_jid = (!quoted_chat_jid.is_same_chat_as(target_chat_jid))
        .then(|| quoted_chat_jid.with_device(0).to_string());

    // Newsletter quotes use the channel JID as participant; others use the sender.
    let participant = if quoted_chat_jid.is_newsletter() {
        quoted_chat_jid.to_string()
    } else {
        sender_jid.to_string()
    };

    wa::ContextInfo {
        stanza_id: Some(message_id.into()),
        participant: Some(participant),
        remote_jid,
        quoted_message: Some(quoted_message.prepare_for_quote()),
        ..Default::default()
    }
}

/// Builds a `reactionMessage` matching WA Web's `WAWebReactionsGenerateReactionMessageProto`
/// (`{ key, text, senderTimestampMs }`).
///
/// `key` references the message being reacted to. An empty `emoji` is the
/// remove-reaction form: the wire stays a `reactionMessage` with empty `text`,
/// which the edit-attr classifier treats as a sender-revoke of the prior reaction.
pub fn build_reaction_message(
    key: wa::MessageKey,
    emoji: impl Into<String>,
    sender_timestamp_ms: i64,
) -> wa::Message {
    wa::Message {
        reaction_message: Some(wa::message::ReactionMessage {
            key: Some(key),
            text: Some(emoji.into()),
            sender_timestamp_ms: Some(sender_timestamp_ms),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Wraps a media message as an album child (WA Web `EProtoGenerator` parity).
/// Lifts `message_context_info` to the outer message and adds the album association.
pub fn wrap_as_album_child(
    mut inner_message: wa::Message,
    parent_key: wa::MessageKey,
) -> wa::Message {
    let existing_context = inner_message.message_context_info.take();

    // WA Web's outgoing association (ProtoUtils.js function m) only sets
    // associationType + parentMessageKey, not messageIndex.
    let association = wa::MessageAssociation {
        association_type: Some(wa::message_association::AssociationType::MediaAlbum as i32),
        parent_message_key: Some(parent_key),
        message_index: None,
    };

    let mut outer_context = existing_context.unwrap_or_default();
    outer_context.message_association = Some(association);

    wa::Message {
        associated_child_message: Some(Box::new(wa::message::FutureProofMessage {
            message: Some(Box::new(inner_message)),
        })),
        message_context_info: Some(outer_context),
        ..Default::default()
    }
}

/// Extension trait for wa::Conversation
pub trait ConversationExt {
    fn subject(&self) -> Option<&str>;
    fn participant_jids(&self) -> Vec<Jid>;
    fn admin_jids(&self) -> Vec<Jid>;
    fn is_locked(&self) -> bool;
    fn is_announce_only(&self) -> bool;
}

impl ConversationExt for wa::Conversation {
    fn subject(&self) -> Option<&str> {
        self.name.as_deref()
    }

    fn participant_jids(&self) -> Vec<Jid> {
        self.participant
            .iter()
            .filter_map(|p| Jid::from_str(&p.user_jid).ok())
            .collect()
    }

    fn admin_jids(&self) -> Vec<Jid> {
        use wa::group_participant::Rank;
        self.participant
            .iter()
            .filter(|p| matches!(p.rank(), Rank::Admin | Rank::Superadmin))
            .filter_map(|p| Jid::from_str(&p.user_jid).ok())
            .collect()
    }

    fn is_locked(&self) -> bool {
        self.locked.unwrap_or(false)
    }

    fn is_announce_only(&self) -> bool {
        // The Conversation proto does not carry an `announce` field.
        // Announce mode is only available from the group metadata IQ
        // response (restrict/announce attributes on the <group> node).
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a message with mentions in context_info.
    fn create_message_with_mentions() -> wa::Message {
        wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("Hello @user1 @user2".to_string()),
                context_info: Some(Box::new(wa::ContextInfo {
                    mentioned_jid: vec![
                        "111111@s.whatsapp.net".to_string(),
                        "222222@s.whatsapp.net".to_string(),
                    ],
                    group_mentions: vec![wa::GroupMention {
                        group_jid: Some("120363012345@g.us".to_string()),
                        group_subject: Some("Test Group".to_string()),
                    }],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    /// Test: prepare_for_quote strips nested mentions and preserves content.
    #[test]
    fn test_prepare_for_quote_strips_mentions_preserves_content() {
        use wa::message::extended_text_message::{FontType, PreviewType};

        let original = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("Hello @user1 @user2".to_string()),
                matched_text: Some("https://example.com".to_string()),
                description: Some("Example description".to_string()),
                title: Some("Example Title".to_string()),
                text_argb: Some(0xFFFFFF),
                background_argb: Some(0x000000),
                font: Some(FontType::SystemBold.into()),
                preview_type: Some(PreviewType::Video.into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    mentioned_jid: vec![
                        "111111@s.whatsapp.net".to_string(),
                        "222222@s.whatsapp.net".to_string(),
                    ],
                    group_mentions: vec![wa::GroupMention {
                        group_jid: Some("120363012345@g.us".to_string()),
                        group_subject: Some("Test Group".to_string()),
                    }],
                    // Other context_info fields that should be preserved
                    is_forwarded: Some(true),
                    forwarding_score: Some(5),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };

        let ext = original.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();
        assert_eq!(ctx.mentioned_jid.len(), 2);
        assert_eq!(ctx.group_mentions.len(), 1);

        let prepared = original.prepare_for_quote();

        let ext = prepared.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();
        assert!(
            ctx.mentioned_jid.is_empty(),
            "mentioned_jid should be empty after prepare_for_quote"
        );
        assert!(
            ctx.group_mentions.is_empty(),
            "group_mentions should be empty after prepare_for_quote"
        );

        assert!(
            ctx.quoted_message.is_none(),
            "quoted_message should be None after prepare_for_quote"
        );
        assert!(
            ctx.stanza_id.is_none(),
            "stanza_id should be None after prepare_for_quote"
        );
        assert!(
            ctx.participant.is_none(),
            "participant should be None after prepare_for_quote"
        );
        assert!(
            ctx.remote_jid.is_none(),
            "remote_jid should be None after prepare_for_quote"
        );

        assert_eq!(ext.text.as_deref(), Some("Hello @user1 @user2"));
        assert_eq!(ext.matched_text.as_deref(), Some("https://example.com"));
        assert_eq!(ext.description.as_deref(), Some("Example description"));
        assert_eq!(ext.title.as_deref(), Some("Example Title"));
        assert_eq!(ext.text_argb, Some(0xFFFFFF));
        assert_eq!(ext.background_argb, Some(0x000000));
        assert_eq!(ext.font(), FontType::SystemBold);
        assert_eq!(ext.preview_type(), PreviewType::Video);

        assert_eq!(ctx.is_forwarded, Some(true));
        assert_eq!(ctx.forwarding_score, Some(5));
    }

    /// Test: prepare_for_quote preserves media message fields (caption, url, dimensions, etc.)
    #[test]
    fn test_prepare_for_quote_preserves_media_fields() {
        let original = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                url: Some("https://mmg.whatsapp.net/...".to_string()),
                mimetype: Some("image/jpeg".to_string()),
                caption: Some("Check out this image!".to_string()),
                file_sha256: Some(vec![1, 2, 3, 4]),
                file_length: Some(12345),
                height: Some(1080),
                width: Some(1920),
                media_key: Some(vec![5, 6, 7, 8]),
                direct_path: Some("/v/t62.1234-5/...".to_string()),
                context_info: Some(Box::new(wa::ContextInfo {
                    mentioned_jid: vec!["someone@s.whatsapp.net".to_string()],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };

        let prepared = original.prepare_for_quote();

        let img = prepared.image_message.as_ref().unwrap();
        let ctx = img.context_info.as_ref().unwrap();

        assert!(ctx.mentioned_jid.is_empty());

        assert_eq!(img.url.as_deref(), Some("https://mmg.whatsapp.net/..."));
        assert_eq!(img.mimetype.as_deref(), Some("image/jpeg"));
        assert_eq!(img.caption.as_deref(), Some("Check out this image!"));
        assert_eq!(img.file_sha256, Some(vec![1, 2, 3, 4]));
        assert_eq!(img.file_length, Some(12345));
        assert_eq!(img.height, Some(1080));
        assert_eq!(img.width, Some(1920));
        assert_eq!(img.media_key, Some(vec![5, 6, 7, 8]));
        assert_eq!(img.direct_path.as_deref(), Some("/v/t62.1234-5/..."));
    }

    /// Test: prepare_for_quote breaks quote chains (Web: 3JJWKHeu5-P.js:48734-48742).
    #[test]
    fn test_prepare_for_quote_breaks_quote_chain() {
        let original = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("This is a reply".to_string()),
                context_info: Some(Box::new(wa::ContextInfo {
                    stanza_id: Some("original-msg-id".to_string()),
                    participant: Some("original-sender@s.whatsapp.net".to_string()),
                    remote_jid: Some("chat@s.whatsapp.net".to_string()),
                    quoted_message: Some(Box::new(wa::Message {
                        conversation: Some("The original message".to_string()),
                        ..Default::default()
                    })),
                    mentioned_jid: vec!["user@s.whatsapp.net".to_string()],
                    is_forwarded: Some(true),
                    forwarding_score: Some(3),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };

        let prepared = original.prepare_for_quote();

        let ext = prepared.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();

        assert!(
            ctx.quoted_message.is_none(),
            "quoted_message should be None (quote chain broken)"
        );
        assert!(
            ctx.stanza_id.is_none(),
            "stanza_id should be None (quote chain broken)"
        );
        assert!(
            ctx.participant.is_none(),
            "participant should be None (quote chain broken)"
        );
        assert!(
            ctx.remote_jid.is_none(),
            "remote_jid should be None (quote chain broken)"
        );
        assert!(
            ctx.mentioned_jid.is_empty(),
            "mentioned_jid should be empty"
        );

        assert_eq!(
            ctx.is_forwarded,
            Some(true),
            "is_forwarded should be preserved"
        );
        assert_eq!(
            ctx.forwarding_score,
            Some(3),
            "forwarding_score should be preserved"
        );

        assert_eq!(ext.text.as_deref(), Some("This is a reply"));
    }

    /// Test: set_context_info works for extended_text_message
    #[test]
    fn test_set_context_info_extended_text() {
        let mut msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("Reply text".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };

        let context = wa::ContextInfo {
            stanza_id: Some("test-id".to_string()),
            participant: Some("sender@s.whatsapp.net".to_string()),
            ..Default::default()
        };

        assert!(msg.set_context_info(context));

        let ext = msg.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();
        assert_eq!(ctx.stanza_id.as_deref(), Some("test-id"));
        assert_eq!(ctx.participant.as_deref(), Some("sender@s.whatsapp.net"));
    }

    /// Test: set_context_info works for image_message
    #[test]
    fn test_set_context_info_image() {
        let mut msg = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                caption: Some("Image caption".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };

        let context = wa::ContextInfo {
            stanza_id: Some("img-id".to_string()),
            ..Default::default()
        };

        assert!(msg.set_context_info(context));

        let img = msg.image_message.as_ref().unwrap();
        assert!(img.context_info.is_some());
        assert_eq!(
            img.context_info.as_ref().unwrap().stanza_id.as_deref(),
            Some("img-id")
        );
    }

    /// Test: set_context_info works for location_message
    #[test]
    fn test_set_context_info_location() {
        let mut msg = wa::Message {
            location_message: Some(Box::new(wa::message::LocationMessage {
                degrees_latitude: Some(40.7128),
                degrees_longitude: Some(-74.0060),
                name: Some("New York".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };

        let context = wa::ContextInfo {
            stanza_id: Some("loc-id".to_string()),
            ..Default::default()
        };

        assert!(msg.set_context_info(context));

        let loc = msg.location_message.as_ref().unwrap();
        assert!(loc.context_info.is_some());
    }

    #[test]
    fn test_set_context_info_promotes_bare_conversation() {
        let mut msg = wa::Message {
            conversation: Some("Simple text".to_string()),
            ..Default::default()
        };

        let context = wa::ContextInfo {
            stanza_id: Some("test-id".to_string()),
            ..Default::default()
        };

        assert!(msg.set_context_info(context));
        assert!(msg.conversation.is_none(), "conversation must be moved out");
        let ext = msg
            .extended_text_message
            .expect("promoted to extended_text_message");
        assert_eq!(ext.text.as_deref(), Some("Simple text"));
        assert_eq!(
            ext.context_info
                .as_ref()
                .and_then(|c| c.stanza_id.as_deref()),
            Some("test-id")
        );
    }

    #[test]
    fn test_set_context_info_returns_false_on_empty_message() {
        let mut msg = wa::Message::default();
        let context = wa::ContextInfo {
            stanza_id: Some("test-id".to_string()),
            ..Default::default()
        };
        assert!(!msg.set_context_info(context));
        assert!(msg.conversation.is_none());
        assert!(msg.extended_text_message.is_none());
    }

    /// Test: build_quote_context produces correct structure.
    #[test]
    fn test_build_quote_context() {
        let original = create_message_with_mentions();

        let context = build_quote_context("3EB0123456789", "1234567890@s.whatsapp.net", &original);

        assert_eq!(context.stanza_id.as_deref(), Some("3EB0123456789"));
        assert_eq!(
            context.participant.as_deref(),
            Some("1234567890@s.whatsapp.net")
        );

        let quoted = context.quoted_message.as_ref().unwrap();
        let ext = quoted.extended_text_message.as_ref().unwrap();
        let quoted_ctx = ext.context_info.as_ref().unwrap();
        assert!(
            quoted_ctx.mentioned_jid.is_empty(),
            "Quoted message mentions should be stripped"
        );
    }

    /// Test: prepare_for_quote handles ephemeral wrapper
    #[test]
    fn test_prepare_for_quote_ephemeral() {
        let ephemeral_msg = wa::Message {
            ephemeral_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(create_message_with_mentions())),
            })),
            ..Default::default()
        };

        let prepared = ephemeral_msg.prepare_for_quote();

        let inner = prepared
            .ephemeral_message
            .as_ref()
            .unwrap()
            .message
            .as_ref()
            .unwrap();
        let ext = inner.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();

        assert!(
            ctx.mentioned_jid.is_empty(),
            "Mentions inside ephemeral wrapper should be stripped"
        );
    }

    /// Test: prepare_for_quote handles view_once wrapper
    #[test]
    fn test_prepare_for_quote_view_once() {
        let view_once_msg = wa::Message {
            view_once_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message {
                    image_message: Some(Box::new(wa::message::ImageMessage {
                        context_info: Some(Box::new(wa::ContextInfo {
                            mentioned_jid: vec!["someone@s.whatsapp.net".to_string()],
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
            })),
            ..Default::default()
        };

        let prepared = view_once_msg.prepare_for_quote();

        let inner = prepared
            .view_once_message
            .as_ref()
            .unwrap()
            .message
            .as_ref()
            .unwrap();
        let img = inner.image_message.as_ref().unwrap();
        let ctx = img.context_info.as_ref().unwrap();

        assert!(
            ctx.mentioned_jid.is_empty(),
            "Mentions inside view_once wrapper should be stripped"
        );
    }

    /// Test: prepare_for_quote handles device_sent_message wrapper (other device).
    #[test]
    fn test_prepare_for_quote_device_sent_message() {
        let device_sent_msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some("1234567890@s.whatsapp.net".to_string()),
                message: Some(Box::new(wa::Message {
                    extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                        text: Some("Message from other device".to_string()),
                        context_info: Some(Box::new(wa::ContextInfo {
                            mentioned_jid: vec![
                                "user1@s.whatsapp.net".to_string(),
                                "user2@s.whatsapp.net".to_string(),
                            ],
                            group_mentions: vec![wa::GroupMention {
                                group_jid: Some("group@g.us".to_string()),
                                group_subject: Some("Group Name".to_string()),
                            }],
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                phash: Some("somephash".to_string()),
            })),
            ..Default::default()
        };

        let prepared = device_sent_msg.prepare_for_quote();

        let wrapper = prepared.device_sent_message.as_ref().unwrap();
        let inner = wrapper.message.as_ref().unwrap();
        let ext = inner.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();

        assert!(
            ctx.mentioned_jid.is_empty(),
            "mentioned_jid inside device_sent_message should be stripped"
        );
        assert!(
            ctx.group_mentions.is_empty(),
            "group_mentions inside device_sent_message should be stripped"
        );

        assert_eq!(ext.text.as_deref(), Some("Message from other device"));
        assert_eq!(
            wrapper.destination_jid.as_deref(),
            Some("1234567890@s.whatsapp.net")
        );
        assert_eq!(wrapper.phash.as_deref(), Some("somephash"));
    }

    /// Test: prepare_for_quote handles edited_message wrapper.
    #[test]
    fn test_prepare_for_quote_edited_message() {
        let edited_msg = wa::Message {
            edited_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message {
                    extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                        text: Some("Edited message text".to_string()),
                        context_info: Some(Box::new(wa::ContextInfo {
                            mentioned_jid: vec!["mentioned@s.whatsapp.net".to_string()],
                            group_mentions: vec![wa::GroupMention {
                                group_jid: Some("editedgroup@g.us".to_string()),
                                group_subject: Some("Edited Group".to_string()),
                            }],
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
            })),
            ..Default::default()
        };

        let prepared = edited_msg.prepare_for_quote();

        let inner = prepared
            .edited_message
            .as_ref()
            .unwrap()
            .message
            .as_ref()
            .unwrap();
        let ext = inner.extended_text_message.as_ref().unwrap();
        let ctx = ext.context_info.as_ref().unwrap();

        assert!(
            ctx.mentioned_jid.is_empty(),
            "mentioned_jid inside edited_message should be stripped"
        );
        assert!(
            ctx.group_mentions.is_empty(),
            "group_mentions inside edited_message should be stripped"
        );

        assert_eq!(ext.text.as_deref(), Some("Edited message text"));
    }

    /// Test: prepare_for_quote handles nested wrappers (device_sent -> ephemeral -> content).
    #[test]
    fn test_prepare_for_quote_nested_wrappers() {
        let nested_wrapper_msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some("dest@s.whatsapp.net".to_string()),
                message: Some(Box::new(wa::Message {
                    ephemeral_message: Some(Box::new(wa::message::FutureProofMessage {
                        message: Some(Box::new(wa::Message {
                            image_message: Some(Box::new(wa::message::ImageMessage {
                                caption: Some("Nested image".to_string()),
                                context_info: Some(Box::new(wa::ContextInfo {
                                    mentioned_jid: vec!["deep@s.whatsapp.net".to_string()],
                                    ..Default::default()
                                })),
                                ..Default::default()
                            })),
                            ..Default::default()
                        })),
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };

        let prepared = nested_wrapper_msg.prepare_for_quote();

        let device_sent = prepared.device_sent_message.as_ref().unwrap();
        let device_inner = device_sent.message.as_ref().unwrap();
        let ephemeral = device_inner.ephemeral_message.as_ref().unwrap();
        let ephemeral_inner = ephemeral.message.as_ref().unwrap();
        let img = ephemeral_inner.image_message.as_ref().unwrap();
        let ctx = img.context_info.as_ref().unwrap();

        assert!(
            ctx.mentioned_jid.is_empty(),
            "Mentions in deeply nested wrappers should be stripped"
        );

        assert_eq!(img.caption.as_deref(), Some("Nested image"));
    }

    /// Test: Multiple message types with context_info can have it set.
    #[test]
    fn test_set_context_info_various_types() {
        let test_cases: Vec<wa::Message> = vec![
            wa::Message {
                video_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                audio_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                document_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                sticker_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                contact_message: Some(Box::default()),
                ..Default::default()
            },
            wa::Message {
                poll_creation_message: Some(Box::default()),
                ..Default::default()
            },
        ];

        for mut msg in test_cases {
            let context = wa::ContextInfo {
                stanza_id: Some("test".to_string()),
                ..Default::default()
            };
            assert!(
                msg.set_context_info(context),
                "set_context_info should succeed for this message type"
            );
        }
    }

    /// Test: Bot quote chains are preserved (Web: 3JJWKHeu5-P.js:48737-48742).
    #[test]
    fn test_prepare_for_quote_preserves_bot_quote_chain() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("Bot reply".to_string()),
                context_info: Some(Box::new(wa::ContextInfo {
                    // Bot JID - starts with 1313555
                    participant: Some("131355512345@s.whatsapp.net".to_string()),
                    stanza_id: Some("bot-msg-id".to_string()),
                    remote_jid: Some("chat@g.us".to_string()),
                    quoted_message: Some(Box::new(wa::Message {
                        conversation: Some("Original user message".to_string()),
                        ..Default::default()
                    })),
                    mentioned_jid: vec!["user@s.whatsapp.net".to_string()],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };

        let prepared = msg.prepare_for_quote();
        let ctx = prepared
            .extended_text_message
            .as_ref()
            .unwrap()
            .context_info
            .as_ref()
            .unwrap();

        assert!(
            ctx.quoted_message.is_some(),
            "Bot quote chain should be preserved"
        );
        assert!(ctx.stanza_id.is_some(), "Bot stanza_id should be preserved");
        assert!(
            ctx.participant.is_some(),
            "Bot participant should be preserved"
        );
        assert!(
            ctx.remote_jid.is_some(),
            "Bot remote_jid should be preserved"
        );

        assert!(
            ctx.mentioned_jid.is_empty(),
            "Mentions should still be cleared even for bots"
        );
    }

    /// Test: Bot with @bot server also has quote chain preserved.
    #[test]
    fn test_prepare_for_quote_preserves_bot_server_quote_chain() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("Bot reply".to_string()),
                context_info: Some(Box::new(wa::ContextInfo {
                    // Bot JID with @bot server
                    participant: Some("mybot@bot".to_string()),
                    stanza_id: Some("bot-msg-id".to_string()),
                    quoted_message: Some(Box::new(wa::Message {
                        conversation: Some("Original".to_string()),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };

        let prepared = msg.prepare_for_quote();
        let ctx = prepared
            .extended_text_message
            .as_ref()
            .unwrap()
            .context_info
            .as_ref()
            .unwrap();

        assert!(
            ctx.quoted_message.is_some(),
            "Bot (@bot server) quote chain should be preserved"
        );
    }

    /// Test: Newsletter participant resolution uses chat JID.
    #[test]
    fn test_build_quote_context_newsletter() {
        let sender: Jid = "123456@s.whatsapp.net".parse().unwrap();
        let chat: Jid = "1234567890@newsletter".parse().unwrap();
        let msg = wa::Message::default();

        let ctx = build_quote_context_with_info("msg-id", &sender, &chat, &chat, &msg);

        assert_eq!(
            ctx.participant.as_deref(),
            Some("1234567890@newsletter"),
            "Newsletter participant should be the newsletter JID"
        );
        assert_eq!(ctx.stanza_id.as_deref(), Some("msg-id"));
    }

    /// Test: Normal message participant resolution uses sender JID.
    #[test]
    fn test_build_quote_context_normal_message() {
        let sender: Jid = "123456@s.whatsapp.net".parse().unwrap();
        let chat: Jid = "group@g.us".parse().unwrap();
        let msg = wa::Message::default();

        let ctx = build_quote_context_with_info("msg-id", &sender, &chat, &chat, &msg);

        assert_eq!(
            ctx.participant.as_deref(),
            Some("123456@s.whatsapp.net"),
            "Normal message participant should be the sender JID"
        );
    }

    /// Test: Status broadcast participant resolution uses sender JID (fallback).
    #[test]
    fn test_build_quote_context_status_broadcast() {
        let sender: Jid = "123456@s.whatsapp.net".parse().unwrap();
        let chat: Jid = "status@broadcast".parse().unwrap();
        let msg = wa::Message::default();

        let ctx = build_quote_context_with_info("msg-id", &sender, &chat, &chat, &msg);

        assert_eq!(
            ctx.participant.as_deref(),
            Some("123456@s.whatsapp.net"),
            "Status broadcast participant should fall back to sender"
        );
    }

    // ── into_base_message tests ──────────────────────────────────────────

    /// Test: into_base_message unwraps DeviceSentMessage containing a reaction.
    #[test]
    fn test_into_base_message_unwraps_device_sent_reaction() {
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
                message: Some(Box::new(wa::Message {
                    reaction_message: Some(wa::message::ReactionMessage {
                        text: Some("\u{2764}".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
                phash: None,
            })),
            ..Default::default()
        };

        let unwrapped = msg.into_base_message();
        assert!(
            unwrapped.device_sent_message.is_none(),
            "device_sent_message wrapper should be removed"
        );
        assert!(
            unwrapped.reaction_message.is_some(),
            "reaction_message should be accessible after unwrapping"
        );
        assert_eq!(
            unwrapped.reaction_message.as_ref().unwrap().text.as_deref(),
            Some("\u{2764}")
        );
    }

    /// Test: into_base_message unwraps nested DSM + ephemeral wrappers.
    #[test]
    fn test_into_base_message_unwraps_nested_dsm_ephemeral() {
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
                message: Some(Box::new(wa::Message {
                    ephemeral_message: Some(Box::new(wa::message::FutureProofMessage {
                        message: Some(Box::new(wa::Message {
                            conversation: Some("secret".to_string()),
                            ..Default::default()
                        })),
                    })),
                    ..Default::default()
                })),
                phash: None,
            })),
            ..Default::default()
        };

        let unwrapped = msg.into_base_message();
        assert_eq!(
            unwrapped.conversation.as_deref(),
            Some("secret"),
            "should unwrap through DSM then ephemeral to reach conversation"
        );
    }

    /// Test: into_base_message passes through a plain message unchanged.
    #[test]
    fn test_into_base_message_passthrough_plain() {
        let msg = wa::Message {
            conversation: Some("hello".to_string()),
            ..Default::default()
        };

        let unwrapped = msg.into_base_message();
        assert_eq!(unwrapped.conversation.as_deref(), Some("hello"));
    }

    /// Test: into_base_message handles DSM with no inner message.
    #[test]
    fn test_into_base_message_empty_dsm() {
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
                message: None,
                phash: None,
            })),
            ..Default::default()
        };

        let unwrapped = msg.into_base_message();
        // With no inner message the wrapper is preserved
        assert!(
            unwrapped.device_sent_message.is_some(),
            "empty DSM wrapper should be preserved"
        );
        assert!(unwrapped.conversation.is_none());
    }

    // ── merge_dsm_context tests ──────────────────────────────────────────

    #[test]
    fn test_merge_dsm_context_both_none() {
        assert!(merge_dsm_context(None, None).is_none());
    }

    #[test]
    fn test_merge_dsm_context_inner_only() {
        let inner = wa::MessageContextInfo {
            message_secret: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        let result = merge_dsm_context(Some(inner.clone()), None).unwrap();
        assert_eq!(result.message_secret, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_merge_dsm_context_outer_only() {
        let outer = wa::MessageContextInfo {
            message_secret: Some(vec![4, 5, 6]),
            limit_sharing_v2: Some(wa::LimitSharing::default()),
            ..Default::default()
        };
        let result = merge_dsm_context(None, Some(&outer)).unwrap();
        assert_eq!(
            result.message_secret,
            Some(vec![4, 5, 6]),
            "message_secret should come from outer when inner is None"
        );
        assert!(
            result.limit_sharing_v2.is_some(),
            "limit_sharing_v2 should come from outer"
        );
    }

    #[test]
    fn test_merge_dsm_context_outer_only_preserves_non_subset_fields() {
        let outer = wa::MessageContextInfo {
            message_add_on_duration_in_secs: Some(86400),
            ..Default::default()
        };
        let result = merge_dsm_context(None, Some(&outer)).unwrap();
        assert_eq!(
            result.message_add_on_duration_in_secs,
            Some(86400),
            "hoisted fields outside the merge subset must survive unwrap"
        );
    }

    #[test]
    fn test_merge_dsm_context_inner_preferred_for_secret() {
        let inner = wa::MessageContextInfo {
            message_secret: Some(vec![1, 2, 3]),
            ..Default::default()
        };
        let outer = wa::MessageContextInfo {
            message_secret: Some(vec![4, 5, 6]),
            ..Default::default()
        };
        let result = merge_dsm_context(Some(inner), Some(&outer)).unwrap();
        assert_eq!(
            result.message_secret,
            Some(vec![1, 2, 3]),
            "inner message_secret should be preferred over outer"
        );
    }

    #[test]
    fn test_merge_dsm_context_secret_fallback_to_outer() {
        let inner = wa::MessageContextInfo {
            message_secret: None,
            ..Default::default()
        };
        let outer = wa::MessageContextInfo {
            message_secret: Some(vec![4, 5, 6]),
            ..Default::default()
        };
        let result = merge_dsm_context(Some(inner), Some(&outer)).unwrap();
        assert_eq!(
            result.message_secret,
            Some(vec![4, 5, 6]),
            "should fall back to outer message_secret when inner is None"
        );
    }

    #[test]
    fn test_merge_dsm_context_limit_sharing_v2_always_outer() {
        let inner_ls = wa::LimitSharing {
            ..Default::default()
        };
        let outer_ls = wa::LimitSharing {
            ..Default::default()
        };
        let inner = wa::MessageContextInfo {
            limit_sharing_v2: Some(inner_ls),
            ..Default::default()
        };
        let outer = wa::MessageContextInfo {
            limit_sharing_v2: Some(outer_ls),
            ..Default::default()
        };
        let result = merge_dsm_context(Some(inner), Some(&outer)).unwrap();
        assert_eq!(
            result.limit_sharing_v2,
            Some(outer_ls),
            "limit_sharing_v2 should always come from outer"
        );

        // When outer is None, inner's limit_sharing_v2 should be cleared
        let inner_with_ls = wa::MessageContextInfo {
            limit_sharing_v2: Some(wa::LimitSharing::default()),
            ..Default::default()
        };
        let result = merge_dsm_context(Some(inner_with_ls), None).unwrap();
        assert_eq!(
            result.limit_sharing_v2, None,
            "limit_sharing_v2 should be cleared when outer is None"
        );
    }

    #[test]
    fn test_merge_dsm_context_thread_id_fallback() {
        let outer = wa::MessageContextInfo {
            thread_id: vec![wa::ThreadId::default()],
            ..Default::default()
        };
        // Inner has empty thread_id → should fall back to outer
        let inner_empty = wa::MessageContextInfo::default();
        let result = merge_dsm_context(Some(inner_empty), Some(&outer)).unwrap();
        assert_eq!(
            result.thread_id.len(),
            1,
            "should fall back to outer thread_id when inner is empty"
        );

        // Inner has non-empty thread_id → should keep inner
        let inner_filled = wa::MessageContextInfo {
            thread_id: vec![wa::ThreadId::default(), wa::ThreadId::default()],
            ..Default::default()
        };
        let result = merge_dsm_context(Some(inner_filled), Some(&outer)).unwrap();
        assert_eq!(
            result.thread_id.len(),
            2,
            "should keep inner thread_id when non-empty"
        );
    }

    #[test]
    fn quote_context_omits_remote_jid_same_chat_group() {
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let msg = wa::Message {
            conversation: Some("hello".into()),
            ..Default::default()
        };

        // Same-chat reply (quoted chat == target): WA Web omits remote_jid.
        let ctx = build_quote_context_with_info("msg-id-123", &sender, &group, &group, &msg);

        assert_eq!(ctx.stanza_id.as_deref(), Some("msg-id-123"));
        assert_eq!(
            ctx.participant.as_deref(),
            Some("551199887766@s.whatsapp.net")
        );
        assert_eq!(ctx.remote_jid, None);
        assert!(ctx.quoted_message.is_some());
        assert!(ctx.mentioned_jid.is_empty());
    }

    #[test]
    fn quote_context_omits_remote_jid_same_chat_dm() {
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let chat: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let msg = wa::Message {
            conversation: Some("ping".into()),
            ..Default::default()
        };

        let ctx = build_quote_context_with_info("msg-id-456", &sender, &chat, &chat, &msg);

        assert_eq!(ctx.remote_jid, None);
        assert_eq!(
            ctx.participant.as_deref(),
            Some("551199887766@s.whatsapp.net")
        );
    }

    #[test]
    fn quote_context_emits_remote_jid_cross_chat() {
        // Quoting a message from group A while sending into group B.
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let quoted_chat: Jid = "120363000000000001@g.us".parse().unwrap();
        let target_chat: Jid = "120363000000000002@g.us".parse().unwrap();
        let msg = wa::Message {
            conversation: Some("cross".into()),
            ..Default::default()
        };

        let ctx =
            build_quote_context_with_info("msg-id-x", &sender, &quoted_chat, &target_chat, &msg);

        assert_eq!(ctx.remote_jid.as_deref(), Some("120363000000000001@g.us"));
    }

    #[test]
    fn quote_context_status_reply_is_cross_chat() {
        // Replying in a DM to a status: status@broadcast != DM target.
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let status: Jid = "status@broadcast".parse().unwrap();
        let target: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let msg = wa::Message::default();

        let ctx = build_quote_context_with_info("msg-id-s", &sender, &status, &target, &msg);

        assert_eq!(ctx.remote_jid.as_deref(), Some("status@broadcast"));
    }

    #[test]
    fn quote_context_device_suffix_treated_as_same_chat() {
        // is_same_chat_as ignores the device suffix, so this stays a same-chat reply.
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let quoted_chat: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let target_chat = quoted_chat.with_device(5);
        let msg = wa::Message::default();

        let ctx =
            build_quote_context_with_info("msg-id-d", &sender, &quoted_chat, &target_chat, &msg);

        assert_eq!(ctx.remote_jid, None);
    }

    #[test]
    fn quote_context_cross_chat_remote_jid_drops_device_suffix() {
        // A device-scoped quoted chat must emit a device-less remote_jid: a chat
        // reference carries no device.
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let quoted_chat = sender.with_device(5);
        let target_chat: Jid = "5521988776655@s.whatsapp.net".parse().unwrap();
        let msg = wa::Message::default();

        let ctx =
            build_quote_context_with_info("msg-id-dev", &sender, &quoted_chat, &target_chat, &msg);

        assert_eq!(
            ctx.remote_jid.as_deref(),
            Some("551199887766@s.whatsapp.net")
        );
    }

    #[test]
    fn quote_context_newsletter_uses_channel_as_participant() {
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let newsletter: Jid = "120363099999999999@newsletter".parse().unwrap();
        let msg = wa::Message::default();

        // Same-chat newsletter reply: participant stays the channel; remote_jid omitted.
        let ctx =
            build_quote_context_with_info("msg-id-789", &sender, &newsletter, &newsletter, &msg);

        assert_eq!(
            ctx.participant.as_deref(),
            Some("120363099999999999@newsletter")
        );
        assert_eq!(ctx.remote_jid, None);
    }

    #[test]
    fn quote_context_newsletter_cross_chat_sets_both() {
        // Quoting a newsletter post while sending into a different newsletter:
        // participant stays the quoted channel AND remote_jid is emitted.
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let quoted: Jid = "120363099999999999@newsletter".parse().unwrap();
        let target: Jid = "120363011111111111@newsletter".parse().unwrap();
        let msg = wa::Message::default();

        let ctx = build_quote_context_with_info("msg-id-nx", &sender, &quoted, &target, &msg);

        assert_eq!(
            ctx.participant.as_deref(),
            Some("120363099999999999@newsletter")
        );
        assert_eq!(
            ctx.remote_jid.as_deref(),
            Some("120363099999999999@newsletter")
        );
    }

    #[test]
    fn quote_context_strips_mentions_from_quoted_message() {
        let sender: Jid = "551199887766@s.whatsapp.net".parse().unwrap();
        let group: Jid = "120363098765432100@g.us".parse().unwrap();
        let msg = create_message_with_mentions();

        let ctx = build_quote_context_with_info("msg-id", &sender, &group, &group, &msg);

        // The quoted message's nested context_info should have mentions stripped
        let quoted = ctx.quoted_message.unwrap();
        let inner_ctx = quoted.extended_text_message.unwrap().context_info.unwrap();
        assert!(inner_ctx.mentioned_jid.is_empty());
        assert!(inner_ctx.group_mentions.is_empty());
        // The outer context should have no mentions
        assert!(ctx.mentioned_jid.is_empty());
    }

    fn sample_parent_key() -> wa::MessageKey {
        wa::MessageKey {
            remote_jid: Some("5511999999999@s.whatsapp.net".to_string()),
            from_me: Some(true),
            id: Some("PARENT_MSG_ID".to_string()),
            participant: None,
        }
    }

    #[test]
    fn test_wrap_as_album_child_basic() {
        let inner = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                url: Some("https://mmg.whatsapp.net/test".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };

        let wrapped = wrap_as_album_child(inner, sample_parent_key());

        let future_proof = wrapped.associated_child_message.as_ref().unwrap();
        let inner_msg = future_proof.message.as_ref().unwrap();
        assert!(inner_msg.image_message.is_some());
        assert!(inner_msg.message_context_info.is_none());

        let ctx = wrapped.message_context_info.as_ref().unwrap();
        let assoc = ctx.message_association.as_ref().unwrap();
        assert_eq!(
            assoc.association_type,
            Some(wa::message_association::AssociationType::MediaAlbum as i32)
        );
        assert_eq!(assoc.parent_message_key, Some(sample_parent_key()));
        assert_eq!(assoc.message_index, None);
    }

    #[test]
    fn test_wrap_as_album_child_lifts_existing_context() {
        let secret = vec![1u8; 32];
        let inner = wa::Message {
            video_message: Some(Box::new(wa::message::VideoMessage {
                url: Some("https://mmg.whatsapp.net/vid".to_string()),
                ..Default::default()
            })),
            message_context_info: Some(wa::MessageContextInfo {
                message_secret: Some(secret.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let wrapped = wrap_as_album_child(inner, sample_parent_key());

        let ctx = wrapped.message_context_info.as_ref().unwrap();
        assert_eq!(ctx.message_secret.as_deref(), Some(secret.as_slice()));
        assert!(ctx.message_association.is_some());
    }

    #[test]
    fn is_view_once_detects_legacy_wrapper() {
        let msg = wa::Message {
            view_once_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message::default())),
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());

        let msg_v2 = wa::Message {
            view_once_message_v2: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message::default())),
            })),
            ..Default::default()
        };
        assert!(msg_v2.is_view_once());
    }

    #[test]
    fn is_view_once_detects_wrapper_nested_in_device_sent() {
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                message: Some(Box::new(wa::Message {
                    view_once_message_v2: Some(Box::new(wa::message::FutureProofMessage {
                        message: Some(Box::new(wa::Message::default())),
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn is_view_once_detects_inline_image_flag() {
        let msg = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                view_once: Some(true),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn is_view_once_detects_inline_video_flag() {
        let msg = wa::Message {
            video_message: Some(Box::new(wa::message::VideoMessage {
                view_once: Some(true),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn is_view_once_detects_inline_audio_flag() {
        let msg = wa::Message {
            audio_message: Some(Box::new(wa::message::AudioMessage {
                view_once: Some(true),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn is_view_once_detects_inline_extended_text_flag() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                view_once: Some(true),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn is_view_once_detects_inline_flag_through_device_sent() {
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                message: Some(Box::new(wa::Message {
                    image_message: Some(Box::new(wa::message::ImageMessage {
                        view_once: Some(true),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn is_view_once_false_for_plain_image() {
        let msg = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage::default())),
            ..Default::default()
        };
        assert!(!msg.is_view_once());

        let msg_explicit_false = wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                view_once: Some(false),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(!msg_explicit_false.is_view_once());
    }

    #[test]
    fn is_view_once_false_for_empty_message() {
        assert!(!wa::Message::default().is_view_once());
    }

    #[test]
    fn is_view_once_detects_v2_extension_wrapper() {
        let msg = wa::Message {
            view_once_message_v2_extension: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message::default())),
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn set_ephemeral_expiration_promotes_bare_conversation_to_extended_text() {
        let mut msg = wa::Message {
            conversation: Some("hello".to_string()),
            ..Default::default()
        };
        assert!(msg.set_ephemeral_expiration(86400));
        assert!(msg.conversation.is_none());
        let ext = msg.extended_text_message.unwrap();
        assert_eq!(ext.text.as_deref(), Some("hello"));
        assert_eq!(
            ext.context_info.as_ref().and_then(|c| c.expiration),
            Some(86400)
        );
    }

    #[test]
    fn set_ephemeral_expiration_returns_false_on_empty_message() {
        let mut msg = wa::Message::default();
        assert!(!msg.set_ephemeral_expiration(60));
        assert!(msg.conversation.is_none());
        assert!(msg.extended_text_message.is_none());
    }

    #[test]
    fn is_view_once_detects_ephemeral_device_sent_view_once() {
        let msg = wa::Message {
            ephemeral_message: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(wa::Message {
                    device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                        message: Some(Box::new(wa::Message {
                            view_once_message_v2: Some(Box::new(wa::message::FutureProofMessage {
                                message: Some(Box::new(wa::Message::default())),
                            })),
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
            })),
            ..Default::default()
        };
        assert!(msg.is_view_once());
    }

    #[test]
    fn mentions_any_bot_true_for_bot_jid_in_extended_text() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("@MetaAI hi".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    mentioned_jid: vec![
                        "5511999998888@s.whatsapp.net".into(),
                        "867051314767696@bot".into(),
                    ],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.mentions_any_bot());
    }

    #[test]
    fn mentions_any_bot_true_for_legacy_pn_form_bot() {
        // `Jid::is_bot()` also matches the legacy PN-form Meta bot; the old
        // `@bot`-only string split would have missed this.
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("@MetaAI".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    mentioned_jid: vec!["13135550002@s.whatsapp.net".into()],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(msg.mentions_any_bot());
    }

    #[test]
    fn mentions_any_bot_false_without_bot_jid() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("hi friends".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    mentioned_jid: vec![
                        "5511999998888@s.whatsapp.net".into(),
                        "120363021033254949@g.us".into(),
                    ],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(!msg.mentions_any_bot());
    }

    #[test]
    fn mentions_any_bot_false_for_no_context_info() {
        let msg = wa::Message {
            conversation: Some("plain".into()),
            ..Default::default()
        };
        assert!(!msg.mentions_any_bot());
    }

    #[test]
    fn mentions_any_bot_sees_through_device_sent_wrapper() {
        let msg = wa::Message {
            device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                destination_jid: Some("867051314767696@bot".into()),
                message: Some(Box::new(wa::Message {
                    extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                        text: Some("@MetaAI".into()),
                        context_info: Some(Box::new(wa::ContextInfo {
                            mentioned_jid: vec!["867051314767696@bot".into()],
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(
            msg.mentions_any_bot(),
            "must unwrap DeviceSentMessage before reading context_info"
        );
    }

    #[test]
    fn is_forwarded_true_and_false() {
        let fwd = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("fwd".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    is_forwarded: Some(true),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(fwd.is_forwarded());

        let plain = wa::Message {
            conversation: Some("plain".into()),
            ..Default::default()
        };
        assert!(!plain.is_forwarded());

        let not_fwd = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("hi".into()),
                context_info: Some(Box::new(wa::ContextInfo::default())),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert!(!not_fwd.is_forwarded());
    }

    #[test]
    fn prepare_for_forward_marks_fresh_conversation() {
        let msg = wa::Message {
            conversation: Some("hello".into()),
            ..Default::default()
        };
        let fwd = msg.prepare_for_forward();
        // A bare conversation is promoted to extended_text_message so the marker
        // can attach.
        let etm = fwd
            .extended_text_message
            .as_ref()
            .expect("conversation promoted to extended_text_message");
        assert_eq!(etm.text.as_deref(), Some("hello"));
        let ctx = etm.context_info.as_ref().expect("context_info present");
        assert_eq!(ctx.is_forwarded, Some(true));
        assert_eq!(ctx.forwarding_score, Some(0));
    }

    #[test]
    fn prepare_for_forward_bumps_score_when_source_already_forwarded() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("hi".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    is_forwarded: Some(true),
                    forwarding_score: Some(0),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let fwd = msg.prepare_for_forward();
        let ctx = fwd
            .extended_text_message
            .as_ref()
            .unwrap()
            .context_info
            .as_ref()
            .unwrap();
        assert_eq!(ctx.is_forwarded, Some(true));
        // n = score(0) + already_forwarded(1) = 1.
        assert_eq!(ctx.forwarding_score, Some(1));
    }

    #[test]
    fn prepare_for_forward_jumps_to_sentinel_at_threshold() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("hi".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    is_forwarded: Some(true),
                    // n = 4 + 1 = 5 -> frequently-forwarded sentinel, not 5.
                    forwarding_score: Some(4),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let fwd = msg.prepare_for_forward();
        let ctx = fwd
            .extended_text_message
            .as_ref()
            .unwrap()
            .context_info
            .as_ref()
            .unwrap();
        assert_eq!(ctx.forwarding_score, Some(127));
    }

    #[test]
    fn prepare_for_forward_strips_quote_chain() {
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("reply".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    stanza_id: Some("QUOTED".into()),
                    participant: Some("123@s.whatsapp.net".into()),
                    quoted_message: Some(Box::new(wa::Message {
                        conversation: Some("orig".into()),
                        ..Default::default()
                    })),
                    mentioned_jid: vec!["456@s.whatsapp.net".into()],
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let fwd = msg.prepare_for_forward();
        let ctx = fwd
            .extended_text_message
            .as_ref()
            .unwrap()
            .context_info
            .as_ref()
            .unwrap();
        assert!(ctx.stanza_id.is_none());
        assert!(ctx.quoted_message.is_none());
        assert!(ctx.participant.is_none());
        assert!(ctx.mentioned_jid.is_empty());
        assert_eq!(ctx.is_forwarded, Some(true));
    }

    #[test]
    fn prepare_for_forward_clears_bot_quote_chain() {
        // Quote sanitizing keeps the chain for bot participants, but forwarding
        // must always break it.
        let msg = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("reply to bot".into()),
                context_info: Some(Box::new(wa::ContextInfo {
                    stanza_id: Some("Q".into()),
                    participant: Some("mybot@bot".into()),
                    quoted_message: Some(Box::new(wa::Message {
                        conversation: Some("bot msg".into()),
                        ..Default::default()
                    })),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        let fwd = msg.prepare_for_forward();
        let ctx = fwd
            .extended_text_message
            .as_ref()
            .unwrap()
            .context_info
            .as_ref()
            .unwrap();
        assert!(ctx.quoted_message.is_none());
        assert!(ctx.participant.is_none());
        assert!(ctx.stanza_id.is_none());
    }

    #[test]
    fn get_base_message_unwraps_view_once_v2_extension() {
        let inner = wa::Message {
            conversation: Some("inner".into()),
            ..Default::default()
        };
        let wrapped = wa::Message {
            view_once_message_v2_extension: Some(Box::new(wa::message::FutureProofMessage {
                message: Some(Box::new(inner)),
            })),
            ..Default::default()
        };
        assert_eq!(
            wrapped.get_base_message().conversation.as_deref(),
            Some("inner")
        );
    }

    #[test]
    fn build_keep_in_chat_message_keep_for_all() {
        let key = wa::MessageKey {
            id: Some("MID".into()),
            from_me: Some(false),
            ..Default::default()
        };
        let msg = build_keep_in_chat_message(key, true, 12345);
        let k = msg
            .keep_in_chat_message
            .as_ref()
            .expect("keep_in_chat_message set");
        assert_eq!(k.keep_type, Some(wa::KeepType::KeepForAll as i32));
        assert_eq!(k.timestamp_ms, Some(12345));
        assert_eq!(
            k.key.as_ref().and_then(|key| key.id.as_deref()),
            Some("MID")
        );
    }

    #[test]
    fn prepare_for_forward_marks_ptv_message() {
        // ptv (video note) is a send-supported context-info carrier; forwarding
        // it must still attach the forwarded marker.
        let msg = wa::Message {
            ptv_message: Some(Box::new(wa::message::VideoMessage::default())),
            ..Default::default()
        };
        let fwd = msg.prepare_for_forward();
        let ctx = fwd
            .ptv_message
            .as_ref()
            .expect("ptv preserved")
            .context_info
            .as_ref()
            .expect("context_info attached");
        assert_eq!(ctx.is_forwarded, Some(true));
        assert_eq!(ctx.forwarding_score, Some(0));
    }

    #[test]
    fn build_keep_in_chat_message_undo() {
        let msg = build_keep_in_chat_message(wa::MessageKey::default(), false, 1);
        assert_eq!(
            msg.keep_in_chat_message.unwrap().keep_type,
            Some(wa::KeepType::UndoKeepForAll as i32)
        );
    }

    fn group_target_key() -> wa::MessageKey {
        wa::MessageKey {
            remote_jid: Some("120363012345@g.us".to_string()),
            from_me: Some(false),
            id: Some("ABCD1234".to_string()),
            participant: Some("15551230000@s.whatsapp.net".to_string()),
        }
    }

    #[test]
    fn build_reaction_populates_key_text_and_timestamp() {
        let key = group_target_key();
        let ts = 1_700_000_000_000;
        let msg = build_reaction_message(key.clone(), "👍", ts);

        let react = msg
            .reaction_message
            .as_ref()
            .expect("reaction_message must be set");
        assert_eq!(react.key.as_ref(), Some(&key));
        assert_eq!(react.text.as_deref(), Some("👍"));
        assert_eq!(react.sender_timestamp_ms, Some(ts));
        // Only the reaction field is populated.
        assert!(msg.conversation.is_none());
        assert!(react.grouping_key.is_none());
    }

    #[test]
    fn build_reaction_preserves_participant_for_group_target() {
        let key = group_target_key();
        let msg = build_reaction_message(key, "❤️", 1);
        let participant = msg
            .reaction_message
            .and_then(|r| r.key)
            .and_then(|k| k.participant);
        assert_eq!(participant.as_deref(), Some("15551230000@s.whatsapp.net"));
    }

    #[test]
    fn build_reaction_empty_emoji_is_unreact_form() {
        // Empty text stays a reaction with present-but-empty text (not None),
        // which the edit-attr classifier maps to a sender-revoke.
        let msg = build_reaction_message(group_target_key(), "", 1);
        let text = msg
            .reaction_message
            .as_ref()
            .and_then(|r| r.text.as_deref());
        assert_eq!(text, Some(""));
    }

    #[test]
    fn build_reaction_edit_attr_classification() {
        use crate::types::message::EditAttribute;

        // A non-empty reaction is a regular send, not an edit/revoke.
        let react = build_reaction_message(group_target_key(), "🔥", 1);
        assert_eq!(EditAttribute::infer_from_message(&react), None);

        // An empty reaction is the sender-revoke of a previous reaction.
        let unreact = build_reaction_message(group_target_key(), "", 1);
        assert_eq!(
            EditAttribute::infer_from_message(&unreact),
            Some(EditAttribute::SenderRevoke)
        );
    }
}
