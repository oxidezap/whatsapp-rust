//! Pure event-to-row transforms: what a `wa::Message` means for the tables.
//! No I/O here so every rule is unit-testable without a database.

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
    /// A revoke of another message: tombstone it. `target_from_me`/`target_participant`
    /// come from the revoke KEY (they identify the target message's owner, not
    /// the revoker — an admin revoke is authored by someone else), used when
    /// the tombstone has to be created before its content arrives.
    Revoke {
        target_id: String,
        target_from_me: bool,
        target_participant: Option<String>,
    },
    /// Protocol/bookkeeping payloads that don't belong in a chat log.
    Ignore,
}

/// Content class of the unwrapped message, as a database label (the typed
/// read-model view is [`MessageKind`](crate::types::MessageKind)). Coarser
/// than the proto (one label per renderable bubble type), finer than WA's
/// stanza `type` attribute (which collapses everything to text/media).
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
    } else if base.template_message.is_set() {
        "template"
    } else if base.template_button_reply_message.is_set() {
        "template_reply"
    } else if base.buttons_message.is_set() {
        "buttons"
    } else if base.buttons_response_message.is_set() {
        "buttons_response"
    } else if base.list_message.is_set() {
        "list"
    } else if base.list_response_message.is_set() {
        "list_response"
    } else if base.interactive_message.is_set() {
        "interactive"
    } else if base.interactive_response_message.is_set() {
        "interactive_response"
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
                let key = pm.key.as_option();
                return MessageOp::Revoke {
                    target_id,
                    target_from_me: key.and_then(|k| k.from_me).unwrap_or(false),
                    target_participant: key.and_then(|k| k.participant.clone()),
                };
            }
            (Some(ProtocolType::MESSAGE_EDIT), Some(target_id)) => {
                if let Some(edited) = pm.edited_message.as_option() {
                    let edited_base = edited.get_base_message();
                    return MessageOp::Edit {
                        target_id,
                        new_text: extract_text(edited_base),
                        new_kind: message_kind(edited_base),
                        new_proto: waproto::codec::message_to_vec(edited),
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
        .or_else(|| business_text(base))
        .map(str::to_owned)
}

/// Body text of the business content carriers. Extraction mirrors WA Web's
/// per-type parsers; footers/buttons stay display-side (readable from the proto).
fn business_text(base: &wa::Message) -> Option<&str> {
    if let Some(tpl) = base.template_message.as_option() {
        use wa::message::template_message::Format;
        // WA Web reads hydratedTemplate ?? format's hydratedFourRowTemplate.
        return tpl
            .hydrated_template
            .as_option()
            .or_else(|| match tpl.format.as_ref() {
                Some(Format::HydratedFourRowTemplate(t)) => Some(t.as_ref()),
                _ => None,
            })
            .and_then(|t| t.hydrated_content_text.as_deref());
    }
    if let Some(reply) = base.template_button_reply_message.as_option() {
        return reply.selected_display_text.as_deref();
    }
    if let Some(buttons) = base.buttons_message.as_option() {
        return buttons.content_text.as_deref();
    }
    if let Some(resp) = base.buttons_response_message.as_option() {
        use wa::message::buttons_response_message::Response;
        return match resp.response.as_ref() {
            Some(Response::SelectedDisplayText(text)) => Some(text.as_str()),
            None => None,
        };
    }
    if let Some(list) = base.list_message.as_option() {
        return list.description.as_deref();
    }
    if let Some(resp) = base.list_response_message.as_option() {
        return resp.title.as_deref();
    }
    if let Some(interactive) = base.interactive_message.as_option() {
        return interactive.body.as_option().and_then(|b| b.text.as_deref());
    }
    if let Some(resp) = base.interactive_response_message.as_option() {
        return resp.body.as_option().and_then(|b| b.text.as_deref());
    }
    None
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
    waproto::codec::message_encoded_len(&probe) > 0
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

    fn store_result(msg: &wa::Message) -> (&'static str, Option<String>) {
        match classify(msg) {
            MessageOp::Store { kind, text } => (kind, text),
            other => panic!("expected Store, got {other:?}"),
        }
    }

    #[test]
    fn classifies_hydrated_template_via_field() {
        let msg = wa::Message {
            template_message: MessageField::some(wa::message::TemplateMessage {
                hydrated_template: MessageField::some(
                    wa::message::template_message::HydratedFourRowTemplate {
                        hydrated_content_text: Some("Dear customer, your bill is ready".into()),
                        hydrated_footer_text: Some("footer stays display-side".into()),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (kind, text) = store_result(&msg);
        assert_eq!(kind, "template");
        assert_eq!(text.as_deref(), Some("Dear customer, your bill is ready"));
    }

    #[test]
    fn classifies_hydrated_template_via_format_oneof() {
        use wa::message::template_message::Format;
        let msg = wa::Message {
            template_message: MessageField::some(wa::message::TemplateMessage {
                format: Some(Format::HydratedFourRowTemplate(Box::new(
                    wa::message::template_message::HydratedFourRowTemplate {
                        hydrated_content_text: Some("Your OTP is 000000".into()),
                        ..Default::default()
                    },
                ))),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (kind, text) = store_result(&msg);
        assert_eq!(kind, "template");
        assert_eq!(text.as_deref(), Some("Your OTP is 000000"));
    }

    /// Non-hydrated template (placeholders not filled): stored as a template
    /// row, just without extractable text.
    #[test]
    fn template_without_hydrated_content_stores_with_null_text() {
        use wa::message::template_message::Format;
        let msg = wa::Message {
            template_message: MessageField::some(wa::message::TemplateMessage {
                format: Some(Format::FourRowTemplate(Box::default())),
                ..Default::default()
            }),
            ..Default::default()
        };
        let (kind, text) = store_result(&msg);
        assert_eq!(kind, "template");
        assert!(text.is_none());
    }

    #[test]
    fn classifies_buttons_list_and_interactive_bodies() {
        let buttons = wa::Message {
            buttons_message: MessageField::some(wa::message::ButtonsMessage {
                content_text: Some("Choose an option".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            store_result(&buttons),
            ("buttons", Some("Choose an option".to_owned()))
        );

        let list = wa::Message {
            list_message: MessageField::some(wa::message::ListMessage {
                description: Some("Pick a plan".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            store_result(&list),
            ("list", Some("Pick a plan".to_owned()))
        );

        let interactive = wa::Message {
            interactive_message: MessageField::some(wa::message::InteractiveMessage {
                body: MessageField::some(wa::message::interactive_message::Body {
                    text: Some("Confirm your order".into()),
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            store_result(&interactive),
            ("interactive", Some("Confirm your order".to_owned()))
        );
    }

    #[test]
    fn classifies_business_response_messages() {
        use wa::message::buttons_response_message::Response;
        let buttons_resp = wa::Message {
            buttons_response_message: MessageField::some(wa::message::ButtonsResponseMessage {
                response: Some(Response::SelectedDisplayText("Yes, confirm".into())),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            store_result(&buttons_resp),
            ("buttons_response", Some("Yes, confirm".to_owned()))
        );

        let list_resp = wa::Message {
            list_response_message: MessageField::some(wa::message::ListResponseMessage {
                title: Some("Basic plan".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            store_result(&list_resp),
            ("list_response", Some("Basic plan".to_owned()))
        );

        let tpl_reply = wa::Message {
            template_button_reply_message: MessageField::some(
                wa::message::TemplateButtonReplyMessage {
                    selected_display_text: Some("Track order".into()),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };
        assert_eq!(
            store_result(&tpl_reply),
            ("template_reply", Some("Track order".to_owned()))
        );

        let interactive_resp = wa::Message {
            interactive_response_message: MessageField::some(
                wa::message::InteractiveResponseMessage {
                    body: MessageField::some(wa::message::interactive_response_message::Body {
                        text: Some("flow reply".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };
        assert_eq!(
            store_result(&interactive_resp),
            ("interactive_response", Some("flow reply".to_owned()))
        );
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
            MessageOp::Revoke { target_id, .. } if target_id == "MSG2"
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
