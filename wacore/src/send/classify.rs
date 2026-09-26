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

/// The `FutureProofMessage` wrappers WA Web's `getUnwrappedProtobufMessage`
/// traverses for classification. Single definition: [`unwrap_message`] and
/// [`contains_group_history_payload`] both expand from here, so the two
/// traversals cannot silently diverge.
macro_rules! for_each_classified_fp_wrapper {
    ($callback:ident) => {
        $callback!(ephemeral_message);
        $callback!(view_once_message);
        $callback!(view_once_message_v2);
        $callback!(view_once_message_v2_extension);
        $callback!(document_with_caption_message);
        $callback!(group_mentioned_message);
        $callback!(bot_invoke_message);
        $callback!(associated_child_message);
        $callback!(poll_creation_option_image_message);
        $callback!(event_cover_image);
        $callback!(group_status_message);
        $callback!(group_status_message_v2);
        $callback!(group_status_mention_message);
        $callback!(status_add_yours);
        $callback!(status_mention_message);
        $callback!(question_message);
        $callback!(question_reply_message);
        $callback!(spoiler_message);
        $callback!(lottie_sticker_message);
        $callback!(limit_sharing_message);
        $callback!(newsletter_admin_profile_message);
        $callback!(newsletter_admin_profile_message_v2);
        $callback!(poll_creation_message_v4);
        $callback!(bot_forwarded_message);
    };
}

/// Every `FutureProofMessage` wrapper the current schema can nest a message
/// in: the classification subset above plus the wrappers WA Web
/// deliberately does not unwrap (`edited_message` is itself a signal
/// callers may need). History detection traverses the full set: for the
/// audience gate the safe direction is the superset. Adding a wrapper to
/// the schema means adding its field to one of these two lists.
macro_rules! for_each_fp_wrapper {
    ($callback:ident) => {
        for_each_classified_fp_wrapper!($callback);
        $callback!(edited_message);
        $callback!(bot_task_message);
        $callback!(newsletter_admin_profile_status_message);
        $callback!(bot_platform_registration_success_message);
    };
}

/// Reject history media keys hidden in any recognized message wrapper, not
/// only the first wrapper the stanza classifier happens to unwrap.
///
/// Unlike `unwrap_message`, which follows WA Web's
/// `getUnwrappedProtobufMessage` subset for classification, this traverses
/// every wrapper above plus `device_sent_message` and `comment_message`:
/// for the audience gate the safe direction is the superset, so a bundle
/// nested anywhere a `Message` can hide never reaches the sender-key
/// broadcast path. `template_message` needs no traversal: its subtree
/// carries only highly-structured template content, never a full nested
/// `Message`. `message_add_ons` needs none either: addons live on
/// `WebMessageInfo`, while this gate inspects the `Message` payload the
/// send funnel carries, and history selection projects shared records
/// without addons.
pub fn contains_group_history_payload(msg: &wa::Message) -> bool {
    if msg.message_history_bundle.is_set() || msg.message_history_notice.is_set() {
        return true;
    }
    macro_rules! check_fp_wrapper {
        ($field:ident) => {
            if msg.$field.as_option().is_some_and(|wrapper| {
                wrapper
                    .message
                    .as_option()
                    .is_some_and(contains_group_history_payload)
            }) {
                return true;
            }
        };
    }
    for_each_fp_wrapper!(check_fp_wrapper);
    if msg.device_sent_message.as_option().is_some_and(|wrapper| {
        wrapper
            .message
            .as_option()
            .is_some_and(contains_group_history_payload)
    }) || msg.comment_message.as_option().is_some_and(|wrapper| {
        wrapper
            .message
            .as_option()
            .is_some_and(contains_group_history_payload)
    }) {
        return true;
    }
    false
}

/// Unwrap wrapper message types to reach the inner message.
/// Matches WA Web's getUnwrappedProtobufMessage. Does not unwrap
/// `edited_message`; that field is itself a signal callers may need.
pub(crate) fn unwrap_message(msg: &wa::Message) -> &wa::Message {
    macro_rules! try_unwrap_one {
        ($field:ident) => {
            if let Some(w) = msg.$field.as_option() {
                if let Some(inner) = w.message.as_option() {
                    return unwrap_message(inner);
                }
            }
        };
    }
    // Remaining FutureProofMessage wrappers from WA Web's
    // getUnwrappedProtobufMessage list; classify by the inner message.
    // WA Web's typeAttributeFromProtobuf re-checks bot_forwarded_message
    // rather than classifying it, so the inner message decides the type — a
    // botForwardedMessage carrying an imageMessage is type="media", not
    // "text". Unwrapping here also reaches `mediaTypeFromProtobuf`, whose
    // own list omits the wrapper: the same deliberate divergence #692 made
    // for group_status_message_v2, and the one that delivers, because
    // `media` with a mediatype renders and `media` without one does not.
    for_each_classified_fp_wrapper!(try_unwrap_one);
    if let Some(dsm) = msg.device_sent_message.as_option()
        && let Some(inner) = dsm.message.as_option()
    {
        return unwrap_message(inner);
    }
    msg
}

/// Matches WAWebE2EProtoUtils.typeAttributeFromProtobuf.
pub fn stanza_type_from_message(msg: &wa::Message) -> &'static str {
    let msg = unwrap_message(msg);

    if msg.reaction_message.is_set() || msg.enc_reaction_message.is_set() {
        return stanza::MSG_TYPE_REACTION;
    }
    if msg.event_message.is_set() || msg.enc_event_response_message.is_set() {
        return stanza::MSG_TYPE_EVENT;
    }
    if let Some(sec) = msg.secret_encrypted_message.as_option() {
        use wa::message::secret_encrypted_message::SecretEncType;
        match sec.secret_enc_type {
            Some(SecretEncType::EventEdit) => return stanza::MSG_TYPE_EVENT,
            Some(SecretEncType::MessageEdit) => return stanza::MSG_TYPE_TEXT,
            Some(SecretEncType::PollEdit | SecretEncType::PollAddOption) => {
                return stanza::MSG_TYPE_POLL;
            }
            _ => {}
        }
    }
    if msg.poll_creation_message.is_set()
        || msg.poll_creation_message_v2.is_set()
        || msg.poll_creation_message_v3.is_set()
        || msg.poll_creation_message_v5.is_set()
        || msg.poll_update_message.is_set()
    {
        return stanza::MSG_TYPE_POLL;
    }
    if msg.conversation.is_some()
        || msg.protocol_message.is_set()
        || msg.keep_in_chat_message.is_set()
        || msg.edited_message.is_set()
        || msg.pin_in_chat_message.is_set()
        || msg.interactive_message.is_set()
        || msg.template_button_reply_message.is_set()
        || msg.request_phone_number_message.is_set()
        || msg.enc_comment_message.is_set()
        || msg.newsletter_admin_invite_message.is_set()
        || msg.newsletter_follower_invite_message_v2.is_set()
        || msg.message_history_notice.is_set()
        || msg.album_message.is_set()
        || msg.rich_response_message.is_set()
        // Reaching here with the wrapper still set means `unwrap_message` found
        // no inner to descend into — the one branch where WA Web answers text
        // (`return h ? f(h, n+1) : text`). With an inner present this is the
        // unwrapped message instead, and the inner decides.
        || msg.bot_forwarded_message.is_set()
        // Payment family. WA Web's typeAttributeFromProtobuf leaves these at the media
        // default, but media-without-mediatype is dropped by the server (so is a bare
        // "pay" stanza); text is what delivers and renders on Android.
        || msg.request_payment_message.is_set()
        || msg.send_payment_message.is_set()
        || msg.payment_invite_message.is_set()
        || msg.decline_payment_request_message.is_set()
        || msg.cancel_payment_request_message.is_set()
    {
        return stanza::MSG_TYPE_TEXT;
    }
    // pollResultSnapshotMessage maps to "text" by default in WA Web
    // (gated behind isPollResultSnapshotPollTypeEnvelopeEnabled for "poll")
    if msg.poll_result_snapshot_message.is_set() || msg.poll_result_snapshot_message_v3.is_set() {
        return stanza::MSG_TYPE_TEXT;
    }
    if let Some(ext) = msg.extended_text_message.as_option() {
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
        .as_option()
        .and_then(|pm| pm.peer_data_operation_request_message.as_option())
        .and_then(|pdo| pdo.peer_data_operation_request_type);

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
    // A lottie behind `bot_forwarded_message` counts too: that wrapper is absent
    // from mediaTypeFromProtobuf's own list but present in the shared unwrap, so
    // without this the check above misses it, the unwrap descends past it, and
    // the stanza goes out `media` with no mediatype — the shape the recipient
    // drops.
    if msg.lottie_sticker_message.is_set()
        || msg
            .bot_forwarded_message
            .as_option()
            .and_then(|w| w.message.as_option())
            .is_some_and(|inner| inner.lottie_sticker_message.is_set())
    {
        return Some("sticker");
    }

    let msg = unwrap_message(msg);

    if msg.image_message.is_set() {
        return Some("image");
    }
    if let Some(vid) = msg.video_message.as_option() {
        return if vid.gif_playback == Some(true) {
            Some("gif")
        } else {
            Some("video")
        };
    }
    if msg.ptv_message.is_set() {
        return Some("ptv");
    }
    if let Some(audio) = msg.audio_message.as_option() {
        return if audio.ptt == Some(true) {
            Some("ptt")
        } else {
            Some("audio")
        };
    }
    if msg.document_message.is_set() {
        return Some("document");
    }
    if msg.sticker_message.is_set() {
        return Some("sticker");
    }
    if msg.sticker_pack_message.is_set() {
        return Some("sticker_pack");
    }
    if let Some(loc) = msg.location_message.as_option() {
        return if loc.is_live == Some(true) {
            Some("livelocation")
        } else {
            Some("location")
        };
    }
    if msg.live_location_message.is_set() {
        return Some("livelocation");
    }
    if msg.contact_message.is_set() {
        return Some("vcard");
    }
    if msg.contacts_array_message.is_set() {
        return Some("contact_array");
    }
    if let Some(ext) = msg.extended_text_message.as_option()
        && ext
            .matched_text
            .as_ref()
            .is_some_and(|t| !t.trim().is_empty())
    {
        return Some("url");
    }
    if msg.group_invite_message.is_set() {
        return Some("url");
    }
    // Interactive / business message families. WA Web's mediaTypeFromProtobuf maps
    // each to a concrete mediatype; without it the server drops the type="media"
    // stanza. buttonsMessage is intentionally absent: WA Web maps it to
    // EncMediaType.Button, which its string mapper drops (no attribute).
    if msg.list_message.is_set() {
        return Some("list");
    }
    if msg.list_response_message.is_set() {
        return Some("list_response");
    }
    if msg.buttons_response_message.is_set() {
        return Some("buttons_response");
    }
    if msg.order_message.is_set() {
        return Some("order");
    }
    if msg.product_message.is_set() {
        return Some("product");
    }
    if msg.interactive_response_message.is_set() {
        return Some("native_flow_response");
    }
    if msg.message_history_bundle.is_set() {
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

    msg.reaction_message.is_set()
        || msg.enc_reaction_message.is_set()
        || msg.pin_in_chat_message.is_set()
        || msg.edited_message.is_set()
        || msg.keep_in_chat_message.is_set()
        || msg.enc_event_response_message.is_set()
        || msg
            .poll_update_message
            .as_option()
            .is_some_and(|p| p.vote.is_set())
        || msg.message_history_notice.is_set()
        || msg.conditional_reveal_message.is_set()
        || msg.secret_encrypted_message.as_option().is_some_and(|s| {
            matches!(
                s.secret_enc_type,
                Some(
                    SecretEncType::EventEdit
                        | SecretEncType::PollEdit
                        | SecretEncType::PollAddOption
                )
            )
        })
        || msg
            .bot_invoke_message
            .as_option()
            .and_then(|b| b.message.as_option())
            .and_then(|m| m.protocol_message.as_option())
            .is_some_and(|p| p.r#type == Some(ProtocolType::RequestWelcomeMessage))
        || msg.protocol_message.as_option().is_some_and(|p| {
            matches!(
                p.r#type,
                Some(t) if t == ProtocolType::EphemeralSyncResponse
                    || t == ProtocolType::RequestWelcomeMessage
                    || t == ProtocolType::GroupMemberLabelChange
            ) || p.edited_message.is_set()
        })
}

#[cfg(test)]
mod history_wrapper_tests {
    use super::*;

    fn fp_wrapped(
        inner: wa::Message,
        wrap: fn(wa::message::FutureProofMessage) -> wa::Message,
    ) -> wa::Message {
        wrap(wa::message::FutureProofMessage {
            message: buffa::MessageField::some(inner),
        })
    }

    fn history_bundle() -> wa::Message {
        wa::Message {
            message_history_bundle: buffa::MessageField::some(Default::default()),
            ..Default::default()
        }
    }

    fn history_notice() -> wa::Message {
        wa::Message {
            message_history_notice: buffa::MessageField::some(Default::default()),
            ..Default::default()
        }
    }

    /// Every wrapper in [`for_each_fp_wrapper`] plus the non-`FutureProof`
    /// carriers must trip history detection, singly and doubly nested.
    /// The table mirrors the production traversal one entry per wrapper:
    /// adding a wrapper to the macro without a row here leaves the new
    /// traversal unproven, removing a row breaks the assertion below.
    /// One table row per traversed wrapper: name plus a constructor that
    /// nests a payload one level deep in that wrapper.
    type WrapperCase = (&'static str, fn(wa::Message) -> wa::Message);

    #[test]
    fn every_wrapper_routes_history_detection() {
        let wrappers: &[WrapperCase] = &[
            ("ephemeral_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    ephemeral_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("view_once_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    view_once_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("view_once_message_v2", |m| {
                fp_wrapped(m, |w| wa::Message {
                    view_once_message_v2: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("view_once_message_v2_extension", |m| {
                fp_wrapped(m, |w| wa::Message {
                    view_once_message_v2_extension: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("document_with_caption_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    document_with_caption_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("group_mentioned_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    group_mentioned_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("bot_invoke_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    bot_invoke_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("associated_child_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    associated_child_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("poll_creation_option_image_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    poll_creation_option_image_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("event_cover_image", |m| {
                fp_wrapped(m, |w| wa::Message {
                    event_cover_image: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("group_status_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    group_status_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("group_status_message_v2", |m| {
                fp_wrapped(m, |w| wa::Message {
                    group_status_message_v2: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("group_status_mention_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    group_status_mention_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("status_add_yours", |m| {
                fp_wrapped(m, |w| wa::Message {
                    status_add_yours: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("status_mention_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    status_mention_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("question_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    question_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("question_reply_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    question_reply_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("spoiler_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    spoiler_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("lottie_sticker_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    lottie_sticker_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("limit_sharing_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    limit_sharing_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("newsletter_admin_profile_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    newsletter_admin_profile_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("newsletter_admin_profile_message_v2", |m| {
                fp_wrapped(m, |w| wa::Message {
                    newsletter_admin_profile_message_v2: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("poll_creation_message_v4", |m| {
                fp_wrapped(m, |w| wa::Message {
                    poll_creation_message_v4: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("bot_forwarded_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    bot_forwarded_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("edited_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    edited_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("bot_task_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    bot_task_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("newsletter_admin_profile_status_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    newsletter_admin_profile_status_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("bot_platform_registration_success_message", |m| {
                fp_wrapped(m, |w| wa::Message {
                    bot_platform_registration_success_message: buffa::MessageField::some(w),
                    ..Default::default()
                })
            }),
            ("device_sent_message", |m| wa::Message {
                device_sent_message: buffa::MessageField::some(wa::message::DeviceSentMessage {
                    message: buffa::MessageField::some(m),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ("comment_message", |m| wa::Message {
                comment_message: buffa::MessageField::some(wa::message::CommentMessage {
                    message: buffa::MessageField::some(m),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        ];
        assert_eq!(wrappers.len(), 30, "one row per traversed wrapper");
        for (name, wrap) in wrappers {
            for payload in [history_bundle(), history_notice()] {
                assert!(
                    contains_group_history_payload(&wrap(payload.clone())),
                    "{name} must trip history detection"
                );
                assert!(
                    contains_group_history_payload(&wrap(wrap(payload))),
                    "{name} doubly nested must trip history detection"
                );
            }
        }
        assert!(!contains_group_history_payload(&wa::Message {
            conversation: Some("plain".into()),
            ..Default::default()
        }));
    }
}
