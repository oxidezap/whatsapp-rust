//! Message classification: stanza/media type, ciphertext extraction, decrypt-fail gating.

use super::*;

/// Extract (enc_type, is_prekey, serialized) from a CiphertextMessage.
pub fn extract_ciphertext(msg: CiphertextMessage) -> Option<(&'static str, bool, Box<[u8]>)> {
    match msg {
        CiphertextMessage::SignalMessage(m) => {
            Some((stanza::ENC_TYPE_MSG, false, m.into_serialized()))
        }
        CiphertextMessage::PreKeySignalMessage(m) => {
            Some((stanza::ENC_TYPE_PKMSG, true, m.into_serialized()))
        }
        _ => None,
    }
}

/// Unwrap wrapper message types to reach the inner message.
/// Matches WA Web's getUnwrappedProtobufMessage. Does not unwrap
/// `edited_message`; that field is itself a signal callers may need.
pub(crate) fn unwrap_message(msg: &wa::Message) -> &wa::Message {
    macro_rules! try_unwrap {
        ($($field:ident),+ $(,)?) => {
            $(
                if let Some(ref w) = msg.$field {
                    if let Some(ref inner) = w.message {
                        return unwrap_message(inner);
                    }
                }
            )+
        };
    }
    try_unwrap!(
        ephemeral_message,
        view_once_message,
        view_once_message_v2,
        view_once_message_v2_extension,
        document_with_caption_message,
        group_mentioned_message,
        bot_invoke_message,
        associated_child_message,
        poll_creation_option_image_message,
        // Remaining FutureProofMessage wrappers from WA Web's
        // getUnwrappedProtobufMessage list; classify by the inner message.
        event_cover_image,
        group_status_message,
        group_status_message_v2,
        group_status_mention_message,
        status_add_yours,
        status_mention_message,
        question_message,
        question_reply_message,
        spoiler_message,
        lottie_sticker_message,
        limit_sharing_message,
        newsletter_admin_profile_message,
        newsletter_admin_profile_message_v2,
        poll_creation_message_v4,
    );
    if let Some(ref dsm) = msg.device_sent_message
        && let Some(ref inner) = dsm.message
    {
        return unwrap_message(inner);
    }
    msg
}

/// Matches WAWebE2EProtoUtils.typeAttributeFromProtobuf.
pub fn stanza_type_from_message(msg: &wa::Message) -> &'static str {
    let msg = unwrap_message(msg);

    if msg.reaction_message.is_some() || msg.enc_reaction_message.is_some() {
        return stanza::MSG_TYPE_REACTION;
    }
    if msg.event_message.is_some() || msg.enc_event_response_message.is_some() {
        return stanza::MSG_TYPE_EVENT;
    }
    if let Some(ref sec) = msg.secret_encrypted_message {
        use wa::message::secret_encrypted_message::SecretEncType;
        match SecretEncType::try_from(sec.secret_enc_type.unwrap_or(0)) {
            Ok(SecretEncType::EventEdit) => return stanza::MSG_TYPE_EVENT,
            Ok(SecretEncType::MessageEdit) => return stanza::MSG_TYPE_TEXT,
            Ok(SecretEncType::PollEdit | SecretEncType::PollAddOption) => {
                return stanza::MSG_TYPE_POLL;
            }
            _ => {}
        }
    }
    if msg.poll_creation_message.is_some()
        || msg.poll_creation_message_v2.is_some()
        || msg.poll_creation_message_v3.is_some()
        || msg.poll_creation_message_v5.is_some()
        || msg.poll_update_message.is_some()
    {
        return stanza::MSG_TYPE_POLL;
    }
    if msg.conversation.is_some()
        || msg.protocol_message.is_some()
        || msg.keep_in_chat_message.is_some()
        || msg.edited_message.is_some()
        || msg.pin_in_chat_message.is_some()
        || msg.interactive_message.is_some()
        || msg.template_button_reply_message.is_some()
        || msg.request_phone_number_message.is_some()
        || msg.enc_comment_message.is_some()
        || msg.newsletter_admin_invite_message.is_some()
        || msg.newsletter_follower_invite_message_v2.is_some()
        || msg.message_history_notice.is_some()
        || msg.album_message.is_some()
        // Payment family. WA Web's typeAttributeFromProtobuf leaves these at the media
        // default, but media-without-mediatype is dropped by the server (so is a bare
        // "pay" stanza); text is what delivers and renders on Android.
        || msg.request_payment_message.is_some()
        || msg.send_payment_message.is_some()
        || msg.payment_invite_message.is_some()
        || msg.decline_payment_request_message.is_some()
        || msg.cancel_payment_request_message.is_some()
    {
        return stanza::MSG_TYPE_TEXT;
    }
    // pollResultSnapshotMessage maps to "text" by default in WA Web
    // (gated behind isPollResultSnapshotPollTypeEnvelopeEnabled for "poll")
    if msg.poll_result_snapshot_message.is_some() || msg.poll_result_snapshot_message_v3.is_some() {
        return stanza::MSG_TYPE_TEXT;
    }
    if let Some(ref ext) = msg.extended_text_message {
        if ext
            .matched_text
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
        {
            return stanza::MSG_TYPE_MEDIA;
        }
        return stanza::MSG_TYPE_TEXT;
    }
    stanza::MSG_TYPE_MEDIA
}

pub fn peer_message_options_from_message(msg: &wa::Message) -> PeerMessageOptions {
    use wa::message::PeerDataOperationRequestType as PdoType;

    // WAWebSendNonMessageDataRequest's A/F helpers gate rollout flags we do
    // not model; use the default-on wire shape for supported peer PDO flows.
    let request_type = unwrap_message(msg)
        .protocol_message
        .as_deref()
        .and_then(|pm| pm.peer_data_operation_request_message.as_ref())
        .and_then(|pdo| pdo.peer_data_operation_request_type)
        .and_then(|raw| PdoType::try_from(raw).ok());

    match request_type {
        Some(PdoType::HistorySyncOnDemand) => PeerMessageOptions::high_force_on_demand(),
        Some(
            PdoType::GenerateLinkPreview
            | PdoType::PlaceholderMessageResend
            | PdoType::CompanionCanonicalUserNonceFetch,
        ) => PeerMessageOptions::high_force(),
        _ => PeerMessageOptions::default(),
    }
}

/// Matches WAWebBackendJobsCommon.mediaTypeFromProtobuf + encodeMaybeMediaType.
/// Returns `None` when the attribute should be omitted.
pub fn media_type_from_message(msg: &wa::Message) -> Option<&'static str> {
    // WA Web's mediaTypeFromProtobuf treats a top-level lottieStickerMessage as a
    // terminal "sticker" and does NOT recurse into it (unlike typeAttributeFromProtobuf,
    // which unwraps it via getUnwrappedProtobufMessage). Check before the shared unwrap.
    if msg.lottie_sticker_message.is_some() {
        return Some("sticker");
    }

    let msg = unwrap_message(msg);

    if msg.image_message.is_some() {
        return Some("image");
    }
    if let Some(ref vid) = msg.video_message {
        return if vid.gif_playback == Some(true) {
            Some("gif")
        } else {
            Some("video")
        };
    }
    if msg.ptv_message.is_some() {
        return Some("ptv");
    }
    if let Some(ref audio) = msg.audio_message {
        return if audio.ptt == Some(true) {
            Some("ptt")
        } else {
            Some("audio")
        };
    }
    if msg.document_message.is_some() {
        return Some("document");
    }
    if msg.sticker_message.is_some() {
        return Some("sticker");
    }
    if msg.sticker_pack_message.is_some() {
        return Some("sticker_pack");
    }
    if let Some(ref loc) = msg.location_message {
        return if loc.is_live == Some(true) {
            Some("livelocation")
        } else {
            Some("location")
        };
    }
    if msg.live_location_message.is_some() {
        return Some("livelocation");
    }
    if msg.contact_message.is_some() {
        return Some("vcard");
    }
    if msg.contacts_array_message.is_some() {
        return Some("contact_array");
    }
    if let Some(ref ext) = msg.extended_text_message
        && ext
            .matched_text
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
    {
        return Some("url");
    }
    if msg.group_invite_message.is_some() {
        return Some("url");
    }
    // Interactive / business message families. WA Web's mediaTypeFromProtobuf maps
    // each to a concrete mediatype; without it the server drops the type="media"
    // stanza. buttonsMessage is intentionally absent: WA Web maps it to
    // EncMediaType.Button, which its string mapper drops (no attribute).
    if msg.list_message.is_some() {
        return Some("list");
    }
    if msg.list_response_message.is_some() {
        return Some("list_response");
    }
    if msg.buttons_response_message.is_some() {
        return Some("buttons_response");
    }
    if msg.order_message.is_some() {
        return Some("order");
    }
    if msg.product_message.is_some() {
        return Some("product");
    }
    if msg.interactive_response_message.is_some() {
        return Some("native_flow_response");
    }
    if msg.message_history_bundle.is_some() {
        return Some("group_history");
    }
    None
}

/// Canonical rule for `decrypt-fail="hide"` on outgoing `<enc>` nodes.
/// Shared by DM fanout, group SKDM and group SKMSG so the three paths can't drift.
/// Both revoke kinds are excluded: WA Web never hides REVOKE, and the server
/// drops revoke stanzas carrying the hide attribute.
pub fn should_hide_decrypt_fail_for_send(
    edit: Option<&crate::types::message::EditAttribute>,
    msg: &wa::Message,
) -> bool {
    use crate::types::message::EditAttribute;
    edit.is_some_and(|e| {
        *e != EditAttribute::Empty
            && *e != EditAttribute::AdminRevoke
            && *e != EditAttribute::SenderRevoke
    }) || should_hide_decrypt_fail(msg)
}

/// Infrastructure messages get decrypt-fail="hide" so recipients don't see
/// "waiting for this message" placeholders for things like reactions or pin changes.
pub fn should_hide_decrypt_fail(msg: &wa::Message) -> bool {
    let msg = unwrap_message(msg);

    use wa::message::protocol_message::Type as ProtocolType;
    use wa::message::secret_encrypted_message::SecretEncType;

    msg.reaction_message.is_some()
        || msg.enc_reaction_message.is_some()
        || msg.pin_in_chat_message.is_some()
        || msg.edited_message.is_some()
        || msg.keep_in_chat_message.is_some()
        || msg.enc_event_response_message.is_some()
        || msg
            .poll_update_message
            .as_ref()
            .is_some_and(|p| p.vote.is_some())
        || msg.message_history_notice.is_some()
        || msg.conditional_reveal_message.is_some()
        || msg.secret_encrypted_message.as_ref().is_some_and(|s| {
            matches!(
                SecretEncType::try_from(s.secret_enc_type.unwrap_or(0)),
                Ok(SecretEncType::EventEdit
                    | SecretEncType::PollEdit
                    | SecretEncType::PollAddOption)
            )
        })
        || msg
            .bot_invoke_message
            .as_ref()
            .and_then(|b| b.message.as_ref())
            .and_then(|m| m.protocol_message.as_ref())
            .is_some_and(|p| p.r#type == Some(ProtocolType::RequestWelcomeMessage as i32))
        || msg.protocol_message.as_ref().is_some_and(|p| {
            matches!(
                p.r#type,
                Some(t) if t == ProtocolType::EphemeralSyncResponse as i32
                    || t == ProtocolType::RequestWelcomeMessage as i32
                    || t == ProtocolType::GroupMemberLabelChange as i32
            ) || p.edited_message.is_some()
        })
}
