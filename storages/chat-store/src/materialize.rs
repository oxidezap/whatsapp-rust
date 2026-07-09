//! Pure event-to-row transforms: what a `wa::Message` means for the tables.
//! No I/O here so every rule is unit-testable without a database.

use buffa::Message as _;
use wacore::proto_helpers::MessageExt;
use waproto::whatsapp as wa;

/// What the writer should do with one inbound message.
#[derive(Debug)]
pub(crate) enum MessageOp {
    /// Regular content: insert (or refresh) a message row.
    Store {
        kind: &'static str,
        text: Option<String>,
    },
    /// A reaction to another message. `emoji` empty means "remove my reaction".
    Reaction { target_id: String, emoji: String },
    /// An edit of another message: replace its content in place.
    Edit {
        target_id: String,
        new_text: Option<String>,
        new_kind: &'static str,
        new_proto: Vec<u8>,
    },
    /// A revoke of another message: tombstone it.
    Revoke { target_id: String },
    /// Protocol/bookkeeping payloads that don't belong in a chat log.
    Ignore,
}

/// UI-facing content class, derived from the unwrapped message. Coarser than
/// the proto (one label per renderable bubble type), finer than WA's stanza
/// `type` attribute (which collapses everything to text/media).
pub(crate) fn message_kind(base: &wa::Message) -> &'static str {
    if base.conversation.is_some() || base.extended_text_message.is_set() {
        "text"
    } else if base.image_message.is_set() {
        "image"
    } else if base.ptv_message.is_set() {
        "ptv"
    } else if base.video_message.is_set() {
        "video"
    } else if let Some(audio) = base.audio_message.as_option() {
        if audio.ptt.unwrap_or(false) {
            "ptt"
        } else {
            "audio"
        }
    } else if base.sticker_message.is_set() || base.lottie_sticker_message.is_set() {
        "sticker"
    } else if base.document_message.is_set() {
        "document"
    } else if base.contact_message.is_set() || base.contacts_array_message.is_set() {
        "contact"
    } else if base.location_message.is_set() || base.live_location_message.is_set() {
        "location"
    } else if base.poll_creation_message.is_set()
        || base.poll_creation_message_v2.is_set()
        || base.poll_creation_message_v3.is_set()
    {
        "poll"
    } else if base.event_message.is_set() {
        "event"
    } else if base.group_invite_message.is_set() {
        "group_invite"
    } else {
        "unknown"
    }
}

/// Kind label for a placeholder row of a message we could not decrypt.
pub(crate) const KIND_UNDECRYPTABLE: &str = "undecryptable";

/// Classify one decrypted message into its materialization op.
pub(crate) fn classify(msg: &wa::Message) -> MessageOp {
    let base = msg.get_base_message();

    if let Some(reaction) = base.reaction_message.as_option() {
        let Some(target_id) = reaction.key.as_option().and_then(|k| k.id.clone()) else {
            return MessageOp::Ignore;
        };
        return MessageOp::Reaction {
            target_id,
            emoji: reaction.text.clone().unwrap_or_default(),
        };
    }

    if let Some(pm) = base.protocol_message.as_option() {
        use wa::message::protocol_message::Type as ProtocolType;
        let target_id = pm.key.as_option().and_then(|k| k.id.clone());
        match (pm.r#type, target_id) {
            (Some(ProtocolType::REVOKE), Some(target_id)) => {
                return MessageOp::Revoke { target_id };
            }
            (Some(ProtocolType::MESSAGE_EDIT), Some(target_id)) => {
                if let Some(edited) = pm.edited_message.as_option() {
                    let edited_base = edited.get_base_message();
                    return MessageOp::Edit {
                        target_id,
                        new_text: extract_text(edited_base),
                        new_kind: message_kind(edited_base),
                        new_proto: edited.encode_to_vec(),
                    };
                }
                return MessageOp::Ignore;
            }
            // Key shares, history-sync notifications, peer data requests, ...
            _ => return MessageOp::Ignore,
        }
    }

    let kind = message_kind(base);
    if kind == "unknown" && !has_any_content(base) {
        // Bare senderKeyDistribution / messageContextInfo carriers.
        return MessageOp::Ignore;
    }
    MessageOp::Store {
        kind,
        text: extract_text(base),
    }
}

/// Text projection for list previews and full-text search: body text or media
/// caption.
pub(crate) fn extract_text(base: &wa::Message) -> Option<String> {
    base.text_content()
        .or_else(|| base.get_caption())
        .map(str::to_owned)
}

/// Whether the unwrapped message carries anything a chat log should show.
/// Guards against storing rows for pure-bookkeeping payloads that classify as
/// "unknown". Decided structurally: strip the bookkeeping carriers and check
/// whether ANY other field remains set, so an unclassified-but-real bubble
/// (e.g. a future WA message type) that also carries `message_context_info`
/// is still stored.
fn has_any_content(base: &wa::Message) -> bool {
    let mut probe = base.clone();
    probe.sender_key_distribution_message = Default::default();
    probe.fast_ratchet_key_sender_key_distribution_message = Default::default();
    probe.message_context_info = Default::default();
    // Encoded emptiness, not PartialEq: presence of an empty submessage still
    // costs wire bytes, while buffa's equality folds it into "absent".
    !probe.encode_to_vec().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use buffa::MessageField;
    use wacore::proto_helpers::MessageBuilderExt;

    fn key_for(id: &str) -> MessageField<wa::MessageKey> {
        MessageField::some(wa::MessageKey {
            id: Some(id.into()),
            ..Default::default()
        })
    }

    #[test]
    fn classifies_plain_text() {
        let msg = wa::Message::text("hello");
        match classify(&msg) {
            MessageOp::Store { kind, text } => {
                assert_eq!(kind, "text");
                assert_eq!(text.as_deref(), Some("hello"));
            }
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn classifies_ephemeral_wrapped_text() {
        let msg = wa::Message {
            ephemeral_message: MessageField::some(wa::message::FutureProofMessage {
                message: MessageField::from_box(Box::new(wa::Message::text("secret"))),
            }),
            ..Default::default()
        };
        match classify(&msg) {
            MessageOp::Store { kind, text } => {
                assert_eq!(kind, "text");
                assert_eq!(text.as_deref(), Some("secret"));
            }
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn classifies_image_with_caption() {
        let msg = wa::Message {
            image_message: MessageField::some(wa::message::ImageMessage {
                caption: Some("look".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        match classify(&msg) {
            MessageOp::Store { kind, text } => {
                assert_eq!(kind, "image");
                assert_eq!(text.as_deref(), Some("look"));
            }
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn ptt_and_audio_are_distinct() {
        let ptt = wa::Message {
            audio_message: MessageField::some(wa::message::AudioMessage {
                ptt: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        };
        let audio = wa::Message {
            audio_message: MessageField::some(wa::message::AudioMessage::default()),
            ..Default::default()
        };
        assert_eq!(message_kind(&ptt), "ptt");
        assert_eq!(message_kind(&audio), "audio");
    }

    #[test]
    fn classifies_reaction_add_and_remove() {
        let add = wa::Message {
            reaction_message: MessageField::some(wa::message::ReactionMessage {
                key: key_for("MSG1"),
                text: Some("👍".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        match classify(&add) {
            MessageOp::Reaction { target_id, emoji } => {
                assert_eq!(target_id, "MSG1");
                assert_eq!(emoji, "👍");
            }
            other => panic!("expected Reaction, got {other:?}"),
        }

        let remove = wa::Message {
            reaction_message: MessageField::some(wa::message::ReactionMessage {
                key: key_for("MSG1"),
                text: Some(String::new()),
                ..Default::default()
            }),
            ..Default::default()
        };
        match classify(&remove) {
            MessageOp::Reaction { emoji, .. } => assert!(emoji.is_empty()),
            other => panic!("expected Reaction, got {other:?}"),
        }
    }

    #[test]
    fn reaction_without_target_key_is_ignored() {
        let msg = wa::Message {
            reaction_message: MessageField::some(wa::message::ReactionMessage {
                text: Some("👍".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(matches!(classify(&msg), MessageOp::Ignore));
    }

    #[test]
    fn classifies_revoke_and_edit() {
        use wa::message::protocol_message::Type as ProtocolType;
        let revoke = wa::Message {
            protocol_message: MessageField::some(wa::message::ProtocolMessage {
                key: key_for("MSG2"),
                r#type: Some(ProtocolType::REVOKE),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(matches!(
            classify(&revoke),
            MessageOp::Revoke { target_id } if target_id == "MSG2"
        ));

        let edit = wa::Message {
            protocol_message: MessageField::some(wa::message::ProtocolMessage {
                key: key_for("MSG3"),
                r#type: Some(ProtocolType::MESSAGE_EDIT),
                edited_message: MessageField::from_box(Box::new(wa::Message::text("fixed"))),
                ..Default::default()
            }),
            ..Default::default()
        };
        match classify(&edit) {
            MessageOp::Edit {
                target_id,
                new_text,
                new_kind,
                new_proto,
            } => {
                assert_eq!(target_id, "MSG3");
                assert_eq!(new_text.as_deref(), Some("fixed"));
                assert_eq!(new_kind, "text");
                assert!(!new_proto.is_empty());
            }
            other => panic!("expected Edit, got {other:?}"),
        }
    }

    #[test]
    fn bookkeeping_only_messages_are_ignored() {
        let skdm_only = wa::Message {
            sender_key_distribution_message: MessageField::some(
                wa::message::SenderKeyDistributionMessage::default(),
            ),
            ..Default::default()
        };
        assert!(matches!(classify(&skdm_only), MessageOp::Ignore));

        let key_share = wa::Message {
            protocol_message: MessageField::some(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::APP_STATE_SYNC_KEY_SHARE),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(matches!(classify(&key_share), MessageOp::Ignore));
    }

    #[test]
    fn unknown_content_with_context_info_is_still_stored() {
        // A future/unclassified bubble type often carries message_context_info
        // (msg secrets); that must not demote it to bookkeeping-only.
        let msg = wa::Message {
            send_payment_message: MessageField::some(wa::message::SendPaymentMessage::default()),
            message_context_info: MessageField::some(wa::MessageContextInfo::default()),
            ..Default::default()
        };
        match classify(&msg) {
            MessageOp::Store { kind, .. } => assert_eq!(kind, "unknown"),
            other => panic!("expected Store, got {other:?}"),
        }

        // While a PURE bookkeeping payload still has no bubble.
        let carrier_only = wa::Message {
            sender_key_distribution_message: MessageField::some(
                wa::message::SenderKeyDistributionMessage::default(),
            ),
            message_context_info: MessageField::some(wa::MessageContextInfo::default()),
            ..Default::default()
        };
        assert!(matches!(classify(&carrier_only), MessageOp::Ignore));
    }

    #[test]
    fn unclassified_but_present_content_stores_as_unknown() {
        let msg = wa::Message {
            send_payment_message: MessageField::some(wa::message::SendPaymentMessage::default()),
            ..Default::default()
        };
        match classify(&msg) {
            MessageOp::Store { kind, .. } => assert_eq!(kind, "unknown"),
            other => panic!("expected Store, got {other:?}"),
        }
    }
}
