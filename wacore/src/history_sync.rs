use bytes::Bytes;
use thiserror::Error;
use wacore_binary::zlib_pool::decompress_zlib_pooled;

#[derive(Debug, Error)]
pub enum HistorySyncError {
    #[error("Failed to decompress history sync data: {0}")]
    DecompressionError(#[from] std::io::Error),
    #[error("Failed to decode HistorySync protobuf: {0}")]
    ProtobufDecodeError(#[from] prost::DecodeError),
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

mod wire_type {
    pub const VARINT: u32 = 0;
    pub const FIXED64: u32 = 1;
    pub const LENGTH_DELIMITED: u32 = 2;
    pub const FIXED32: u32 = 5;
}

/// Decompress and process a history sync blob.
///
/// **Memory strategy**: Decompresses the entire blob into a single `Bytes`
/// buffer, then scans top-level fields and partially decodes only the
/// conversation fields needed for internal caches.
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
    let mut pos = 0;
    let mut result = HistorySyncResult {
        own_pushname: None,
        nct_salt: None,
        conversations_processed: 0,
        tc_token_candidates: Vec::new(),
        msg_secret_records: Vec::new(),
        decompressed_bytes: if retain_blob { Some(buf.clone()) } else { None },
    };

    while pos < buf.len() {
        let (tag, bytes_read) = read_varint(&buf[pos..])?;
        pos += bytes_read;

        let field_number = (tag >> 3) as u32;
        let wire_type_raw = (tag & 0x7) as u32;

        match field_number {
            // field 2 = conversations (repeated, length-delimited)
            2 if wire_type_raw == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "conversation")?;

                result.conversations_processed += 1;
                let extracted = extract_conversation_fields(&buf[pos..end]);
                if let Some(candidate) = extracted.tc_token_candidate {
                    result.tc_token_candidates.push(candidate);
                }
                result
                    .msg_secret_records
                    .extend(extracted.msg_secret_records);
                pos = end;
            }

            // field 7 = pushnames (repeated, length-delimited).
            // Uses `Option::is_some()` in the guard rather than an
            // `if let` guard — the latter requires Rust 1.94+. The inner
            // `if let` is the defensive complement: if the guard's
            // invariant is ever weakened by a future refactor, we skip
            // the arm body instead of panicking.
            7 if own_user.is_some()
                && result.own_pushname.is_none()
                && wire_type_raw == wire_type::LENGTH_DELIMITED =>
            {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "pushname")?;

                if let Some(own) = own_user
                    && let Some(name) = extract_own_pushname(&buf[pos..end], own)
                {
                    result.own_pushname = Some(name);
                }
                pos = end;
            }

            // field 19 = nctSalt (optional bytes, length-delimited)
            // Delivered during initial pairing so cstoken is available immediately.
            // Source: storeNctSaltFromHistorySync in WAWeb/History/MsgHandlerAction.js
            19 if wire_type_raw == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "nctSalt")?;

                let salt = buf[pos..end].to_vec();
                if !salt.is_empty() {
                    result.nct_salt = Some(salt);
                }
                pos = end;
            }

            _ => {
                pos = skip_field(wire_type_raw, &buf, pos)?;
            }
        }
    }

    Ok(result)
}

/// Compute `pos + len` with overflow and bounds checking.
#[inline]
fn checked_end(
    pos: usize,
    len: u64,
    buf_len: usize,
    field: &str,
) -> Result<usize, HistorySyncError> {
    let len = usize::try_from(len).map_err(|_| {
        HistorySyncError::MalformedProtobuf(format!("{field} length overflows usize: {len}"))
    })?;
    let end = pos.checked_add(len).ok_or_else(|| {
        HistorySyncError::MalformedProtobuf(format!(
            "{field} field overflows: pos={pos}, len={len}"
        ))
    })?;
    if end > buf_len {
        return Err(HistorySyncError::MalformedProtobuf(format!(
            "{field} field overflows buffer: pos={pos}, len={len}, buf={buf_len}"
        )));
    }
    Ok(end)
}

/// Read a protobuf varint from `data`, returning (value, bytes_consumed).
#[inline]
fn read_varint(data: &[u8]) -> Result<(u64, usize), HistorySyncError> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    for (i, &byte) in data.iter().enumerate() {
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i + 1));
        }
        shift += 7;
        if shift >= 64 {
            return Err(HistorySyncError::MalformedProtobuf(
                "varint too long".into(),
            ));
        }
    }
    Err(HistorySyncError::MalformedProtobuf(
        "unexpected end of data in varint".into(),
    ))
}

/// Skip a protobuf field based on wire type, returning the new position.
#[inline]
fn skip_field(wire_type: u32, buf: &[u8], pos: usize) -> Result<usize, HistorySyncError> {
    match wire_type {
        wire_type::VARINT => {
            let (_, vlen) = read_varint(&buf[pos..])?;
            Ok(pos + vlen)
        }
        wire_type::FIXED64 => checked_end(pos, 8, buf.len(), "fixed64"),
        wire_type::LENGTH_DELIMITED => {
            let (len, vlen) = read_varint(&buf[pos..])?;
            checked_end(pos + vlen, len, buf.len(), "length-delimited")
        }
        wire_type::FIXED32 => checked_end(pos, 4, buf.len(), "fixed32"),
        _ => {
            log::warn!("Unknown wire type {wire_type} in history sync, cannot skip");
            Err(HistorySyncError::MalformedProtobuf(format!(
                "unknown wire type {wire_type}"
            )))
        }
    }
}

/// Manual pushname parser — Pushname proto has fields: id (tag 1) and pushname (tag 2).
/// Checks id first and only allocates the pushname string if id matches `own_user`.
fn extract_own_pushname(data: &[u8], own_user: &str) -> Option<String> {
    let mut pos = 0;
    let mut id_match = false;
    let mut pushname: Option<String> = None;

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            // id (tag 1, string)
            1 if wt == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(data.get(pos..)?).ok()?;
                pos += vlen;
                let len = usize::try_from(len).ok()?;
                let end = pos.checked_add(len).filter(|&e| e <= data.len())?;
                let id = std::str::from_utf8(data.get(pos..end)?).ok()?;
                id_match = id == own_user;
                if !id_match {
                    return None; // wrong user, skip entirely
                }
                pos = end;
            }
            // pushname (tag 2, string)
            2 if wt == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(data.get(pos..)?).ok()?;
                pos += vlen;
                let len = usize::try_from(len).ok()?;
                let end = pos.checked_add(len).filter(|&e| e <= data.len())?;
                let name = std::str::from_utf8(data.get(pos..end)?).ok()?;
                pushname = Some(name.to_string());
                pos = end;
            }
            _ => {
                pos = skip_field(wt, data, pos).ok()?;
            }
        }
    }

    if id_match { pushname } else { None }
}

/// Prost partial decode for internal HistorySync fields we persist while
/// streaming conversations.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ConversationInternalFields {
    #[prost(string, required, tag = "1")]
    pub id: String,
    #[prost(message, repeated, tag = "2")]
    pub messages: Vec<HistorySyncMsgInternalFields>,
    #[prost(bytes = "vec", optional, tag = "21")]
    pub tc_token: Option<Vec<u8>>,
    #[prost(uint64, optional, tag = "22")]
    pub tc_token_timestamp: Option<u64>,
    #[prost(uint64, optional, tag = "28")]
    pub tc_token_sender_timestamp: Option<u64>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct HistorySyncMsgInternalFields {
    #[prost(message, optional, tag = "1")]
    pub message: Option<WebMessageInfoInternalFields>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct WebMessageInfoInternalFields {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKeyInternalFields>,
    #[prost(message, optional, tag = "2")]
    pub message: Option<MessageInternalFields>,
    #[prost(string, optional, tag = "5")]
    pub participant: Option<String>,
    #[prost(bytes = "vec", optional, tag = "49")]
    pub message_secret: Option<Vec<u8>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct MessageKeyInternalFields {
    #[prost(bool, optional, tag = "2")]
    pub from_me: Option<bool>,
    #[prost(string, optional, tag = "3")]
    pub id: Option<String>,
    #[prost(string, optional, tag = "4")]
    pub participant: Option<String>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct MessageInternalFields {
    #[prost(message, optional, tag = "3")]
    pub image_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "4")]
    pub contact_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "5")]
    pub location_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "6")]
    pub extended_text_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "7")]
    pub document_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "8")]
    pub audio_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "9")]
    pub video_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "13")]
    pub contacts_array_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "18")]
    pub live_location_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "25")]
    pub template_message: Option<ContextInfoTag3InternalFields>,
    #[prost(message, optional, tag = "26")]
    pub sticker_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "28")]
    pub group_invite_message: Option<ContextInfoTag7InternalFields>,
    #[prost(message, optional, tag = "29")]
    pub template_button_reply_message: Option<ContextInfoTag3InternalFields>,
    #[prost(message, optional, tag = "30")]
    pub product_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "31")]
    pub device_sent_message: Option<DeviceSentMessageInternalFields>,
    #[prost(message, optional, tag = "35")]
    pub message_context_info: Option<MessageContextInfoInternalFields>,
    #[prost(message, optional, tag = "36")]
    pub list_message: Option<ContextInfoTag8InternalFields>,
    #[prost(message, optional, tag = "37")]
    pub view_once_message: Option<FutureProofMessageInternalFields>,
    #[prost(message, optional, tag = "38")]
    pub order_message: Option<ContextInfoTag17InternalFields>,
    #[prost(message, optional, tag = "39")]
    pub list_response_message: Option<ContextInfoTag4InternalFields>,
    #[prost(message, optional, tag = "40")]
    pub ephemeral_message: Option<FutureProofMessageInternalFields>,
    #[prost(message, optional, tag = "42")]
    pub buttons_message: Option<ContextInfoTag8InternalFields>,
    #[prost(message, optional, tag = "43")]
    pub buttons_response_message: Option<ContextInfoTag3InternalFields>,
    #[prost(message, optional, tag = "45")]
    pub interactive_message: Option<ContextInfoTag15InternalFields>,
    #[prost(message, optional, tag = "48")]
    pub interactive_response_message: Option<ContextInfoTag15InternalFields>,
    #[prost(message, optional, tag = "49")]
    pub poll_creation_message: Option<ContextInfoTag5InternalFields>,
    #[prost(message, optional, tag = "53")]
    pub document_with_caption_message: Option<FutureProofMessageInternalFields>,
    #[prost(message, optional, tag = "55")]
    pub view_once_message_v2: Option<FutureProofMessageInternalFields>,
    #[prost(message, optional, tag = "58")]
    pub edited_message: Option<FutureProofMessageInternalFields>,
    #[prost(message, optional, tag = "60")]
    pub poll_creation_message_v2: Option<ContextInfoTag5InternalFields>,
    #[prost(message, optional, tag = "64")]
    pub poll_creation_message_v3: Option<ContextInfoTag5InternalFields>,
    #[prost(message, optional, tag = "75")]
    pub event_message: Option<ContextInfoTag1InternalFields>,
    #[prost(message, optional, tag = "78")]
    pub newsletter_admin_invite_message: Option<ContextInfoTag6InternalFields>,
    #[prost(message, optional, tag = "86")]
    pub sticker_pack_message: Option<ContextInfoTag11InternalFields>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct MessageContextInfoInternalFields {
    #[prost(bytes = "vec", optional, tag = "3")]
    pub message_secret: Option<Vec<u8>>,
}

macro_rules! define_context_info_carrier {
    ($name:ident, $tag:literal) => {
        #[derive(Clone, PartialEq, prost::Message)]
        pub(crate) struct $name {
            #[prost(message, optional, tag = $tag)]
            pub context_info: Option<ContextInfoInternalFields>,
        }

        impl $name {
            fn is_forwarded(&self) -> bool {
                self.context_info
                    .as_ref()
                    .and_then(|ctx| ctx.is_forwarded)
                    .unwrap_or(false)
            }
        }
    };
}

define_context_info_carrier!(ContextInfoTag1InternalFields, "1");
define_context_info_carrier!(ContextInfoTag3InternalFields, "3");
define_context_info_carrier!(ContextInfoTag4InternalFields, "4");
define_context_info_carrier!(ContextInfoTag5InternalFields, "5");
define_context_info_carrier!(ContextInfoTag6InternalFields, "6");
define_context_info_carrier!(ContextInfoTag7InternalFields, "7");
define_context_info_carrier!(ContextInfoTag8InternalFields, "8");
define_context_info_carrier!(ContextInfoTag11InternalFields, "11");
define_context_info_carrier!(ContextInfoTag15InternalFields, "15");
define_context_info_carrier!(ContextInfoTag17InternalFields, "17");

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct ContextInfoInternalFields {
    #[prost(bool, optional, tag = "22")]
    pub is_forwarded: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct DeviceSentMessageInternalFields {
    #[prost(message, optional, boxed, tag = "2")]
    pub message: Option<Box<MessageInternalFields>>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct FutureProofMessageInternalFields {
    #[prost(message, optional, boxed, tag = "1")]
    pub message: Option<Box<MessageInternalFields>>,
}

impl MessageInternalFields {
    fn base_message(&self) -> &Self {
        let mut current = self;
        loop {
            let next = current
                .device_sent_message
                .as_ref()
                .and_then(|m| m.message.as_deref())
                .or_else(|| {
                    current
                        .ephemeral_message
                        .as_ref()
                        .and_then(|m| m.message.as_deref())
                })
                .or_else(|| {
                    current
                        .view_once_message
                        .as_ref()
                        .and_then(|m| m.message.as_deref())
                })
                .or_else(|| {
                    current
                        .view_once_message_v2
                        .as_ref()
                        .and_then(|m| m.message.as_deref())
                })
                .or_else(|| {
                    current
                        .document_with_caption_message
                        .as_ref()
                        .and_then(|m| m.message.as_deref())
                })
                .or_else(|| {
                    current
                        .edited_message
                        .as_ref()
                        .and_then(|m| m.message.as_deref())
                });

            match next {
                Some(msg) => current = msg,
                None => return current,
            }
        }
    }

    fn is_forwarded(&self) -> bool {
        let base = self.base_message();
        macro_rules! any_forwarded {
            ($($field:ident),+ $(,)?) => {
                false $(|| base.$field.as_ref().map(|m| m.is_forwarded()).unwrap_or(false))+
            };
        }

        any_forwarded!(
            extended_text_message,
            image_message,
            video_message,
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
    }
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

fn extract_conversation_fields(data: &[u8]) -> ConversationExtraction {
    use prost::Message;

    let Ok(conv) = ConversationInternalFields::decode(data) else {
        return ConversationExtraction {
            tc_token_candidate: None,
            msg_secret_records: Vec::new(),
        };
    };

    let tc_token_candidate = extract_tc_token_fields(&conv);
    let msg_secret_records = extract_msg_secret_records(&conv);

    ConversationExtraction {
        tc_token_candidate,
        msg_secret_records,
    }
}

/// Returns `None` for groups, newsletters, bots, or conversations without tctokens.
fn extract_tc_token_fields(conv: &ConversationInternalFields) -> Option<TcTokenCandidate> {
    // Early-out for non-1:1 conversations
    if let Some(parts) = wacore_binary::jid::parse_jid_fast(&conv.id)
        && (parts.server == "g.us" || parts.server == "newsletter" || parts.server == "bot")
    {
        return None;
    }

    let tc_token = conv.tc_token.as_ref().filter(|t| !t.is_empty())?.clone();
    let tc_token_timestamp = conv.tc_token_timestamp?;

    Some(TcTokenCandidate {
        id: conv.id.clone(),
        tc_token,
        tc_token_timestamp,
        tc_token_sender_timestamp: conv.tc_token_sender_timestamp,
    })
}

fn extract_msg_secret_records(conv: &ConversationInternalFields) -> Vec<HistoryMsgSecretRecord> {
    let mut records = Vec::new();

    for history_msg in &conv.messages {
        let Some(web_msg) = history_msg.message.as_ref() else {
            continue;
        };
        let Some(key) = web_msg.key.as_ref() else {
            continue;
        };
        let Some(msg_id) = key.id.as_ref() else {
            continue;
        };
        if let Some(message) = web_msg.message.as_ref()
            && message.is_forwarded()
        {
            continue;
        }
        let Some(secret) = web_msg.message_secret.as_ref().or_else(|| {
            web_msg
                .message
                .as_ref()
                .and_then(|m| m.message_context_info.as_ref())
                .and_then(|mci| mci.message_secret.as_ref())
        }) else {
            continue;
        };

        records.push(HistoryMsgSecretRecord {
            chat_id: conv.id.clone(),
            from_me: key.from_me == Some(true),
            key_participant: key.participant.clone(),
            web_msg_participant: web_msg.participant.clone(),
            msg_id: msg_id.clone(),
            secret: secret.clone(),
        });
    }

    records
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
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use prost::Message;
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
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
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
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
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
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
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
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![
                    wa::HistorySyncMsg {
                        message: Some(wa::WebMessageInfo {
                            key: wa::MessageKey {
                                remote_jid: Some(chat.to_string()),
                                from_me: Some(false),
                                id: Some("HIST_TOP_LEVEL".to_string()),
                                participant: Some(participant.to_string()),
                            },
                            message_secret: Some(top_level_secret.clone()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    wa::HistorySyncMsg {
                        message: Some(wa::WebMessageInfo {
                            key: wa::MessageKey {
                                remote_jid: Some(chat.to_string()),
                                from_me: Some(true),
                                id: Some("HIST_CONTEXT".to_string()),
                                participant: None,
                            },
                            message: Some(wa::Message {
                                message_context_info: Some(wa::MessageContextInfo {
                                    message_secret: Some(context_secret.clone()),
                                    ..Default::default()
                                }),
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
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: Some(wa::WebMessageInfo {
                        key: wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some("HIST_FORWARDED".to_string()),
                            ..Default::default()
                        },
                        message: Some(wa::Message {
                            extended_text_message: Some(Box::new(
                                wa::message::ExtendedTextMessage {
                                    text: Some("forwarded".into()),
                                    context_info: Some(Box::new(wa::ContextInfo {
                                        is_forwarded: Some(true),
                                        ..Default::default()
                                    })),
                                    ..Default::default()
                                },
                            )),
                            message_context_info: Some(wa::MessageContextInfo {
                                message_secret: Some(vec![0x66u8; 32]),
                                ..Default::default()
                            }),
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
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: Some(wa::WebMessageInfo {
                        key: wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some("HIST_NESTED_FORWARDED".to_string()),
                            ..Default::default()
                        },
                        message: Some(wa::Message {
                            view_once_message: Some(Box::new(wa::message::FutureProofMessage {
                                message: Some(Box::new(wa::Message {
                                    ephemeral_message: Some(Box::new(
                                        wa::message::FutureProofMessage {
                                            message: Some(Box::new(wa::Message {
                                                extended_text_message: Some(Box::new(
                                                    wa::message::ExtendedTextMessage {
                                                        text: Some("nested".into()),
                                                        context_info: Some(Box::new(
                                                            wa::ContextInfo {
                                                                is_forwarded: Some(true),
                                                                ..Default::default()
                                                            },
                                                        )),
                                                        ..Default::default()
                                                    },
                                                )),
                                                ..Default::default()
                                            })),
                                        },
                                    )),
                                    ..Default::default()
                                })),
                            })),
                            message_context_info: Some(wa::MessageContextInfo {
                                message_secret: Some(vec![0x77u8; 32]),
                                ..Default::default()
                            }),
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
