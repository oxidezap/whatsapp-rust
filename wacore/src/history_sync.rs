use bytes::Bytes;
use thiserror::Error;
use wacore_binary::zlib_pool::decompress_zlib_pooled;
use waproto::whatsapp as wa;

use buffa::view::MessageView as _;

#[derive(Debug, Error)]
pub enum HistorySyncError {
    #[error("Failed to decompress history sync data: {0}")]
    DecompressionError(#[from] std::io::Error),
    #[error("Failed to decode HistorySync protobuf: {0}")]
    ProtobufDecodeError(#[from] buffa::DecodeError),
    #[error("Malformed protobuf: {0}")]
    MalformedProtobuf(String),
}

#[derive(Debug)]
pub struct HistorySyncResult {
    pub own_pushname: Option<String>,
    /// NCT salt from HistorySync field 19 (nctSalt).
    /// Delivered during initial pairing so cstoken is available immediately.
    /// Source: WAWeb/History/MsgHandlerAction.js:storeNctSaltFromHistorySync
    pub nct_salt: Option<Vec<u8>>,
    pub conversations_processed: usize,
    /// Tctoken candidates extracted from 1:1 conversations during streaming.
    pub tc_token_candidates: Vec<TcTokenCandidate>,
    pub msg_secret_records: Vec<HistoryMsgSecretRecord>,
    /// The full decompressed protobuf blob, only retained when event
    /// listeners exist. Wrapped in `LazyHistorySync` for on-demand decoding.
    pub decompressed_bytes: Option<Bytes>,
}

/// Decompress and process a history sync blob.
///
/// **Memory strategy**: Decompresses the entire blob into a single `Bytes`.
/// Buffa views borrow from that buffer while extracting internal cache data,
/// so strings and bytes are only cloned when they are persisted.
///
/// After decompression, the compressed input is dropped immediately, so peak
/// memory = max(compressed, decompressed) + small overhead, not both.
pub fn process_history_sync(
    compressed_data: Vec<u8>,
    own_user: Option<&str>,
    retain_blob: bool,
    _compressed_size_hint: Option<u64>,
) -> Result<HistorySyncResult, HistorySyncError> {
    // Hard limit to prevent OOM on malformed blobs.
    // Typical InitialBootstrap: 5-20 MB decompressed.
    const MAX_DECOMPRESSED: u64 = 64 * 1024 * 1024;

    let decompressed = decompress_zlib_pooled(&compressed_data, MAX_DECOMPRESSED)
        .map_err(HistorySyncError::DecompressionError)?;
    drop(compressed_data);

    let buf = Bytes::from(decompressed);
    let mut result = HistorySyncResult {
        own_pushname: None,
        nct_salt: None,
        conversations_processed: 0,
        tc_token_candidates: Vec::new(),
        msg_secret_records: Vec::new(),
        decompressed_bytes: if retain_blob { Some(buf.clone()) } else { None },
    };

    let view = wa::HistorySyncView::decode_view(&buf)?;

    for conversation in &view.conversations {
        result.conversations_processed += 1;
        let extracted = extract_conversation_fields(conversation);
        if let Some(candidate) = extracted.tc_token_candidate {
            result.tc_token_candidates.push(candidate);
        }
        result
            .msg_secret_records
            .extend(extracted.msg_secret_records);
    }

    if let Some(own) = own_user
        && result.own_pushname.is_none()
    {
        for pushname in &view.pushnames {
            if pushname.id == Some(own)
                && let Some(name) = pushname.pushname
            {
                result.own_pushname = Some(name.to_string());
                break;
            }
        }
    }

    if let Some(salt) = view.nct_salt.filter(|salt| !salt.is_empty()) {
        result.nct_salt = Some(salt.to_vec());
    }

    Ok(result)
}

struct ConversationExtraction {
    tc_token_candidate: Option<TcTokenCandidate>,
    msg_secret_records: Vec<HistoryMsgSecretRecord>,
}

/// Message-secret data extracted from a conversation during streaming.
#[derive(Debug)]
pub struct HistoryMsgSecretRecord {
    pub chat_id: String,
    pub from_me: bool,
    pub key_participant: Option<String>,
    pub web_msg_participant: Option<String>,
    pub msg_id: String,
    pub secret: Vec<u8>,
}

fn extract_conversation_fields(conv: &wa::ConversationView<'_>) -> ConversationExtraction {
    let tc_token_candidate = extract_tc_token_fields(conv);
    let msg_secret_records = extract_msg_secret_records(conv);

    ConversationExtraction {
        tc_token_candidate,
        msg_secret_records,
    }
}

/// Returns `None` for groups, newsletters, bots, or conversations without tctokens.
fn extract_tc_token_fields(conv: &wa::ConversationView<'_>) -> Option<TcTokenCandidate> {
    let id = conv.id;
    if id.is_empty() {
        return None;
    }

    // Early-out for non-1:1 conversations
    if let Some(parts) = wacore_binary::jid::parse_jid_fast(id)
        && (parts.server == "g.us" || parts.server == "newsletter" || parts.server == "bot")
    {
        return None;
    }

    let tc_token = conv.tc_token.filter(|t| !t.is_empty())?.to_vec();
    let tc_token_timestamp = conv.tc_token_timestamp?;

    Some(TcTokenCandidate {
        id: id.to_string(),
        tc_token,
        tc_token_timestamp,
        tc_token_sender_timestamp: conv.tc_token_sender_timestamp,
    })
}

fn extract_msg_secret_records(conv: &wa::ConversationView<'_>) -> Vec<HistoryMsgSecretRecord> {
    let mut records = Vec::new();
    let chat_id = conv.id;
    if chat_id.is_empty() {
        return records;
    }

    for history_msg in &conv.messages {
        let Some(web_msg) = history_msg.message.as_option() else {
            continue;
        };
        let Some(key) = web_msg.key.as_option() else {
            continue;
        };
        let Some(msg_id) = key.id else {
            continue;
        };
        if web_msg
            .message
            .as_option()
            .is_some_and(message_is_forwarded)
        {
            continue;
        }
        let Some(secret) = web_msg.message_secret.or_else(|| {
            web_msg
                .message
                .as_option()
                .and_then(extract_message_context_secret)
        }) else {
            continue;
        };

        records.push(HistoryMsgSecretRecord {
            chat_id: chat_id.to_string(),
            from_me: key.from_me == Some(true),
            key_participant: key.participant.map(str::to_string),
            web_msg_participant: web_msg.participant.map(str::to_string),
            msg_id: msg_id.to_string(),
            secret: secret.to_vec(),
        });
    }

    records
}

fn extract_message_context_secret<'a>(message: &'a wa::MessageView<'a>) -> Option<&'a [u8]> {
    message.message_context_info.as_option()?.message_secret
}

fn message_is_forwarded(message: &wa::MessageView<'_>) -> bool {
    message_is_forwarded_at_depth(message, 0)
}

fn message_is_forwarded_at_depth(message: &wa::MessageView<'_>, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }

    if let Some(inner) = first_wrapped_message(message) {
        return message_is_forwarded_at_depth(inner, depth + 1);
    }

    message_context_is_forwarded(message)
}

fn first_wrapped_message<'a>(message: &'a wa::MessageView<'a>) -> Option<&'a wa::MessageView<'a>> {
    if let Some(wrapper) = message.device_sent_message.as_option()
        && let Some(inner) = wrapper.message.as_option()
    {
        return Some(inner);
    }

    macro_rules! future_proof_inner {
        ($($field:ident),* $(,)?) => {
            $(
                if let Some(wrapper) = message.$field.as_option()
                    && let Some(inner) = wrapper.message.as_option()
                {
                    return Some(inner);
                }
            )*
        };
    }

    future_proof_inner!(
        ephemeral_message,
        view_once_message,
        view_once_message_v2,
        view_once_message_v2_extension,
        document_with_caption_message,
        edited_message,
        group_mentioned_message,
        bot_invoke_message,
        lottie_sticker_message,
        event_cover_image,
        status_mention_message,
        poll_creation_option_image_message,
        associated_child_message,
        group_status_mention_message,
        poll_creation_message_v4,
        status_add_yours,
        group_status_message,
        limit_sharing_message,
        bot_task_message,
        question_message,
        group_status_message_v2,
        bot_forwarded_message,
        question_reply_message,
        newsletter_admin_profile_message,
        newsletter_admin_profile_message_v2,
        spoiler_message,
    );

    None
}

fn message_context_is_forwarded(message: &wa::MessageView<'_>) -> bool {
    macro_rules! has_forwarded_context {
        ($($field:ident),* $(,)?) => {
            $(
                if message.$field.as_option()
                    .and_then(|message| message.context_info.as_option())
                    .is_some_and(context_info_is_forwarded)
                {
                    return true;
                }
            )*
        };
    }

    has_forwarded_context!(
        event_message,
        template_message,
        template_button_reply_message,
        buttons_response_message,
        list_response_message,
        poll_creation_message,
        poll_creation_message_v2,
        poll_creation_message_v3,
        poll_creation_message_v5,
        poll_creation_message_v6,
        newsletter_admin_invite_message,
        group_invite_message,
        list_message,
        buttons_message,
        sticker_pack_message,
        interactive_message,
        interactive_response_message,
        image_message,
        contact_message,
        location_message,
        extended_text_message,
        document_message,
        audio_message,
        video_message,
        ptv_message,
        contacts_array_message,
        live_location_message,
        sticker_message,
        product_message,
        order_message,
        rich_response_message,
        message_history_notice,
        event_invite_message,
    );

    false
}

fn context_info_is_forwarded(context: &wa::ContextInfoView<'_>) -> bool {
    context.is_forwarded == Some(true)
}

/// Tctoken data extracted from a conversation during streaming.
#[derive(Debug)]
pub struct TcTokenCandidate {
    pub id: String,
    pub tc_token: Vec<u8>,
    pub tc_token_timestamp: u64,
    pub tc_token_sender_timestamp: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use buffa::Message;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;
    use waproto::whatsapp as wa;

    /// Encode a HistorySync proto and zlib-compress it.
    fn encode_and_compress(hs: &wa::HistorySync) -> Vec<u8> {
        let proto_bytes = hs.encode_to_vec();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&proto_bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn test_nct_salt_extracted_from_history_sync() {
        let salt = vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            nct_salt: Some(salt.clone()),
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, None, false, None).unwrap();

        assert_eq!(result.nct_salt, Some(salt));
    }

    #[test]
    fn test_nct_salt_none_when_absent() {
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, None, false, None).unwrap();

        assert!(result.nct_salt.is_none());
    }

    #[test]
    fn test_nct_salt_and_pushname_coexist() {
        let salt = vec![0x01, 0x02, 0x03];
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            nct_salt: Some(salt.clone()),
            pushnames: vec![wa::Pushname {
                id: Some("0000000000".into()),
                pushname: Some("TestUser".into()),
            }],
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, Some("0000000000"), false, None).unwrap();

        assert_eq!(result.nct_salt, Some(salt));
        assert_eq!(result.own_pushname.as_deref(), Some("TestUser"));
    }

    #[test]
    fn test_message_secrets_extracted_from_history_sync() {
        let chat = "5511777776666@s.whatsapp.net";
        let participant = "5511888889999@s.whatsapp.net";
        let top_level_secret = vec![0x44u8; 32];
        let context_secret = vec![0x55u8; 32];
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![
                    wa::HistorySyncMsg {
                        message: buffa::MessageField::some(wa::WebMessageInfo {
                            key: buffa::MessageField::some(wa::MessageKey {
                                remote_jid: Some(chat.to_string()),
                                from_me: Some(false),
                                id: Some("HIST_TOP_LEVEL".to_string()),
                                participant: Some(participant.to_string()),
                            }),
                            message_secret: Some(top_level_secret.clone()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    wa::HistorySyncMsg {
                        message: buffa::MessageField::some(wa::WebMessageInfo {
                            key: buffa::MessageField::some(wa::MessageKey {
                                remote_jid: Some(chat.to_string()),
                                from_me: Some(true),
                                id: Some("HIST_CONTEXT".to_string()),
                                participant: None,
                            }),
                            message: buffa::MessageField::some(wa::Message {
                                message_context_info: buffa::MessageField::some(
                                    wa::MessageContextInfo {
                                        message_secret: Some(context_secret.clone()),
                                        ..Default::default()
                                    },
                                ),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, None, false, None).unwrap();

        assert_eq!(result.msg_secret_records.len(), 2);
        assert_eq!(result.msg_secret_records[0].chat_id, chat);
        assert_eq!(result.msg_secret_records[0].msg_id, "HIST_TOP_LEVEL");
        assert_eq!(
            result.msg_secret_records[0].key_participant.as_deref(),
            Some(participant)
        );
        assert_eq!(result.msg_secret_records[0].secret, top_level_secret);
        assert_eq!(result.msg_secret_records[1].msg_id, "HIST_CONTEXT");
        assert!(result.msg_secret_records[1].from_me);
        assert_eq!(result.msg_secret_records[1].secret, context_secret);
    }

    #[test]
    fn test_forwarded_message_secrets_skipped_from_history_sync() {
        let chat = "5511000000001@s.whatsapp.net";
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: buffa::MessageField::some(wa::WebMessageInfo {
                        key: buffa::MessageField::some(wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some("HIST_FORWARDED".to_string()),
                            ..Default::default()
                        }),
                        message: buffa::MessageField::some(wa::Message {
                            extended_text_message: buffa::MessageField::some(
                                wa::message::ExtendedTextMessage {
                                    text: Some("forwarded".into()),
                                    context_info: buffa::MessageField::some(wa::ContextInfo {
                                        is_forwarded: Some(true),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                },
                            ),
                            message_context_info: buffa::MessageField::some(
                                wa::MessageContextInfo {
                                    message_secret: Some(vec![0x66u8; 32]),
                                    ..Default::default()
                                },
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, None, false, None).unwrap();

        assert!(result.msg_secret_records.is_empty());
    }

    #[test]
    fn test_nested_forwarded_message_secrets_skipped_from_history_sync() {
        let chat = "5511000000002@s.whatsapp.net";
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: buffa::MessageField::some(wa::WebMessageInfo {
                        key: buffa::MessageField::some(wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some("HIST_NESTED_FORWARDED".to_string()),
                            ..Default::default()
                        }),
                        message: buffa::MessageField::some(wa::Message {
                            view_once_message: buffa::MessageField::some(
                                wa::message::FutureProofMessage {
                                    message: buffa::MessageField::some(wa::Message {
                                        ephemeral_message: buffa::MessageField::some(
                                            wa::message::FutureProofMessage {
                                                message: buffa::MessageField::some(wa::Message {
                                                    extended_text_message:
                                                        buffa::MessageField::some(
                                                            wa::message::ExtendedTextMessage {
                                                                text: Some("nested".into()),
                                                                context_info:
                                                                    buffa::MessageField::some(
                                                                        wa::ContextInfo {
                                                                            is_forwarded: Some(
                                                                                true,
                                                                            ),
                                                                            ..Default::default()
                                                                        },
                                                                    ),
                                                                ..Default::default()
                                                            },
                                                        ),
                                                    ..Default::default()
                                                }),
                                            },
                                        ),
                                        ..Default::default()
                                    }),
                                },
                            ),
                            message_context_info: buffa::MessageField::some(
                                wa::MessageContextInfo {
                                    message_secret: Some(vec![0x77u8; 32]),
                                    ..Default::default()
                                },
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, None, false, None).unwrap();

        assert!(result.msg_secret_records.is_empty());
    }
}
