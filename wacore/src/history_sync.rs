use bytes::Bytes;
use thiserror::Error;
use wacore_binary::zlib_pool::{InflateReader, decompress_zlib_pooled};
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

mod wire_type {
    pub const VARINT: u32 = 0;
    pub const FIXED64: u32 = 1;
    pub const LENGTH_DELIMITED: u32 = 2;
    pub const FIXED32: u32 = 5;
}

/// Decompress and process a history sync blob.
///
/// **Memory strategy**: Decompresses the entire blob into a single `Bytes`,
/// then scans top-level fields and decodes each conversation as a Buffa view
/// one at a time. Strings and bytes are cloned only when they are persisted.
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

    // When the caller doesn't need the full decompressed blob (no
    // Event::HistorySync consumer), stream-decompress and extract incrementally
    // so peak memory stays ~one conversation instead of the whole blob.
    if !retain_blob {
        return process_history_sync_streaming(&compressed_data, own_user, MAX_DECOMPRESSED);
    }

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
        // Pre-size from a single allocation-free count pass so the accumulator
        // never grows by repeated doubling as conversations are processed.
        msg_secret_records: Vec::with_capacity(count_history_sync_messages(&buf)),
        decompressed_bytes: if retain_blob { Some(buf.clone()) } else { None },
    };

    while pos < buf.len() {
        let (tag, bytes_read) = read_varint(&buf[pos..])?;
        pos += bytes_read;

        let field_number = (tag >> 3) as u32;
        let wire_type_raw = (tag & 0x7) as u32;

        match field_number {
            2 if wire_type_raw == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "conversation")?;

                let conversation = wa::HsConversationExtractView::decode_view(&buf[pos..end])?;
                result.conversations_processed += 1;
                if let Some(candidate) =
                    extract_conversation_fields(&conversation, &mut result.msg_secret_records)
                {
                    result.tc_token_candidates.push(candidate);
                }
                pos = end;
            }
            7 if own_user.is_some()
                && result.own_pushname.is_none()
                && wire_type_raw == wire_type::LENGTH_DELIMITED =>
            {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "pushname")?;

                if let Some(own) = own_user
                    && let Some(name) = extract_own_pushname(&buf[pos..end], own)?
                {
                    result.own_pushname = Some(name);
                }
                pos = end;
            }
            19 if wire_type_raw == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "nctSalt")?;

                let salt = &buf[pos..end];
                if !salt.is_empty() {
                    result.nct_salt = Some(salt.to_vec());
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

/// Streaming variant of [`process_history_sync`] for when the full decompressed
/// blob is NOT needed (`retain_blob == false`). Decompresses incrementally and
/// parses each top-level field as soon as its bytes are buffered, so peak memory
/// is bounded by the largest single conversation rather than the whole blob.
/// Produces the same extraction results (secrets, tctokens, pushname, nctSalt)
/// as the full path, but with `decompressed_bytes == None`.
fn process_history_sync_streaming(
    compressed_data: &[u8],
    own_user: Option<&str>,
    max_decompressed: u64,
) -> Result<HistorySyncResult, HistorySyncError> {
    let mut reader = InflateReader::new(compressed_data, max_decompressed);
    let mut result = HistorySyncResult {
        own_pushname: None,
        nct_salt: None,
        conversations_processed: 0,
        tc_token_candidates: Vec::new(),
        msg_secret_records: Vec::new(),
        decompressed_bytes: None,
    };

    loop {
        // A field starts with a tag varint; stop cleanly when the stream ends.
        if !reader
            .ensure(1)
            .map_err(HistorySyncError::DecompressionError)?
        {
            break;
        }
        // A varint is at most 10 bytes (fewer is fine right at EOF).
        reader
            .ensure(10)
            .map_err(HistorySyncError::DecompressionError)?;
        let (tag, tlen) = read_varint(reader.available())?;
        reader.consume(tlen);

        let field_number = (tag >> 3) as u32;
        let wire_type_raw = (tag & 0x7) as u32;

        match wire_type_raw {
            wire_type::LENGTH_DELIMITED => {
                reader
                    .ensure(10)
                    .map_err(HistorySyncError::DecompressionError)?;
                let (len, vlen) = read_varint(reader.available())?;
                reader.consume(vlen);
                let len = usize::try_from(len).map_err(|_| {
                    HistorySyncError::MalformedProtobuf(format!(
                        "field length overflows usize: {len}"
                    ))
                })?;
                if !reader
                    .ensure(len)
                    .map_err(HistorySyncError::DecompressionError)?
                {
                    return Err(HistorySyncError::MalformedProtobuf(
                        "length-delimited field truncated".into(),
                    ));
                }
                {
                    let value = &reader.available()[..len];
                    match field_number {
                        // conversations (repeated)
                        2 => {
                            let conversation = wa::HsConversationExtractView::decode_view(value)?;
                            result.conversations_processed += 1;
                            if let Some(candidate) = extract_conversation_fields(
                                &conversation,
                                &mut result.msg_secret_records,
                            ) {
                                result.tc_token_candidates.push(candidate);
                            }
                        }
                        // pushnames (repeated) — only our own is needed
                        7 => {
                            if result.own_pushname.is_none()
                                && let Some(own) = own_user
                                && let Some(name) = extract_own_pushname(value, own)?
                            {
                                result.own_pushname = Some(name);
                            }
                        }
                        // nctSalt
                        19 if !value.is_empty() => {
                            result.nct_salt = Some(value.to_vec());
                        }
                        _ => {}
                    }
                }
                reader.consume(len);
            }
            wire_type::VARINT => {
                reader
                    .ensure(10)
                    .map_err(HistorySyncError::DecompressionError)?;
                let (_, vlen) = read_varint(reader.available())?;
                reader.consume(vlen);
            }
            wire_type::FIXED64 => {
                if !reader
                    .ensure(8)
                    .map_err(HistorySyncError::DecompressionError)?
                {
                    return Err(HistorySyncError::MalformedProtobuf(
                        "fixed64 field truncated".into(),
                    ));
                }
                reader.consume(8);
            }
            wire_type::FIXED32 => {
                if !reader
                    .ensure(4)
                    .map_err(HistorySyncError::DecompressionError)?
                {
                    return Err(HistorySyncError::MalformedProtobuf(
                        "fixed32 field truncated".into(),
                    ));
                }
                reader.consume(4);
            }
            _ => {
                return Err(HistorySyncError::MalformedProtobuf(format!(
                    "unknown wire type {wire_type_raw}"
                )));
            }
        }
    }

    Ok(result)
}

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

#[inline]
fn read_varint(data: &[u8]) -> Result<(u64, usize), HistorySyncError> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    for (i, &byte) in data.iter().enumerate() {
        if i == 9 && (byte & 0xFE) != 0 {
            return Err(HistorySyncError::MalformedProtobuf(
                "varint overflows u64".into(),
            ));
        }
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

fn extract_own_pushname(data: &[u8], own_user: &str) -> Result<Option<String>, buffa::DecodeError> {
    let pushname = wa::PushnameView::decode_view(data)?;
    if pushname.id == Some(own_user)
        && let Some(name) = pushname.pushname
    {
        return Ok(Some(name.to_string()));
    }
    Ok(None)
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
    /// Parent message event time (unix seconds), if present in the blob.
    /// Used by the seed-time retention filter; `None` falls back to seed time.
    pub timestamp: Option<u64>,
    /// Whether the parent is a poll-creation or event message. These get the
    /// longer poll/event retention horizon because their add-ons (poll votes,
    /// PollAddOption, EventEdit) have no sender-side time window.
    pub is_poll_or_event: bool,
    /// Whether the parent invokes a bot (botMetadata present). Kept so the seed
    /// classifies a group bot prompt as a bot context, matching live capture, so
    /// `BotOnly` retains it and a later bot reply can decrypt.
    pub is_bot_invocation: bool,
}

/// Appends this conversation's message-secret records directly into `records`
/// (no per-conversation Vec) and returns its tctoken candidate, if any.
fn extract_conversation_fields(
    conv: &wa::HsConversationExtractView<'_>,
    records: &mut Vec<HistoryMsgSecretRecord>,
) -> Option<TcTokenCandidate> {
    extract_msg_secret_records(conv, records);
    extract_tc_token_fields(conv)
}

/// Returns `None` for groups, newsletters, bots, or conversations without tctokens.
fn extract_tc_token_fields(conv: &wa::HsConversationExtractView<'_>) -> Option<TcTokenCandidate> {
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

fn extract_msg_secret_records(
    conv: &wa::HsConversationExtractView<'_>,
    records: &mut Vec<HistoryMsgSecretRecord>,
) {
    let chat_id = conv.id;
    if chat_id.is_empty() {
        return;
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

        let inner = web_msg.message.as_option();
        let is_poll_or_event = inner.is_some_and(message_is_poll_or_event);
        let is_bot_invocation = inner.is_some_and(message_invokes_bot);

        records.push(HistoryMsgSecretRecord {
            chat_id: chat_id.to_string(),
            from_me: key.from_me == Some(true),
            key_participant: key.participant.map(str::to_string),
            web_msg_participant: web_msg.participant.map(str::to_string),
            msg_id: msg_id.to_string(),
            secret: secret.to_vec(),
            timestamp: web_msg.message_timestamp,
            is_poll_or_event,
            is_bot_invocation,
        });
    }
}

/// Upper-bound count of message entries across all conversations, via an
/// allocation-free wire scan. Used to pre-size the secret-record accumulator.
fn count_history_sync_messages(buf: &[u8]) -> usize {
    let mut pos = 0;
    let mut total = 0;
    while pos < buf.len() {
        let Ok((tag, br)) = read_varint(&buf[pos..]) else {
            break;
        };
        pos += br;
        let field = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;
        if field == 2 && wt == wire_type::LENGTH_DELIMITED {
            let Ok((len, vl)) = read_varint(&buf[pos..]) else {
                break;
            };
            pos += vl;
            let Ok(end) = checked_end(pos, len, buf.len(), "conv-count") else {
                break;
            };
            total += count_conversation_messages(&buf[pos..end]);
            pos = end;
        } else {
            match skip_field(wt, buf, pos) {
                Ok(np) => pos = np,
                Err(_) => break,
            }
        }
    }
    total
}

/// Count field-2 (message) entries within a single conversation's bytes.
fn count_conversation_messages(buf: &[u8]) -> usize {
    let mut pos = 0;
    let mut n = 0;
    while pos < buf.len() {
        let Ok((tag, br)) = read_varint(&buf[pos..]) else {
            break;
        };
        pos += br;
        let field = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;
        if field == 2 && wt == wire_type::LENGTH_DELIMITED {
            n += 1;
        }
        match skip_field(wt, buf, pos) {
            Ok(np) => pos = np,
            Err(_) => break,
        }
    }
    n
}

/// Walk wrapper layers (deviceSent/ephemeral/viewOnce/etc.) to the innermost
/// message, mirroring main's `MessageInternalFields::base_message`. Bounded the
/// same way as [`message_is_forwarded_at_depth`].
fn base_message_view<'a>(
    message: &'a wa::HsMessageExtractView<'a>,
) -> &'a wa::HsMessageExtractView<'a> {
    let mut current = message;
    let mut depth = 0usize;
    while depth <= 16 {
        match first_wrapped_message(current) {
            Some(inner) => {
                current = inner;
                depth += 1;
            }
            None => break,
        }
    }
    current
}

/// Whether the (unwrapped) message is a poll-creation or event message. These
/// carry the longer poll/event retention horizon.
fn message_is_poll_or_event(message: &wa::HsMessageExtractView<'_>) -> bool {
    let base = base_message_view(message);
    base.poll_creation_message.as_option().is_some()
        || base.poll_creation_message_v2.as_option().is_some()
        || base.poll_creation_message_v3.as_option().is_some()
        || base.event_message.as_option().is_some()
}

/// Whether the message invokes a bot, detected via `botMetadata` presence.
/// botMetadata sits on the top-level `MessageContextInfo` even when wrapped,
/// so check both the outer message and the unwrapped base.
fn message_invokes_bot(message: &wa::HsMessageExtractView<'_>) -> bool {
    let has = |m: &wa::HsMessageExtractView<'_>| {
        m.message_context_info
            .as_option()
            .is_some_and(|c| c.bot_metadata.is_some())
    };
    has(message) || has(base_message_view(message))
}

fn extract_message_context_secret<'a>(
    message: &'a wa::HsMessageExtractView<'a>,
) -> Option<&'a [u8]> {
    message.message_context_info.as_option()?.message_secret
}

fn message_is_forwarded(message: &wa::HsMessageExtractView<'_>) -> bool {
    message_is_forwarded_at_depth(message, 0)
}

fn message_is_forwarded_at_depth(message: &wa::HsMessageExtractView<'_>, depth: usize) -> bool {
    if depth > 16 {
        return false;
    }

    if let Some(inner) = first_wrapped_message(message) {
        return message_is_forwarded_at_depth(inner, depth + 1);
    }

    message_context_is_forwarded(message)
}

fn first_wrapped_message<'a>(
    message: &'a wa::HsMessageExtractView<'a>,
) -> Option<&'a wa::HsMessageExtractView<'a>> {
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

    // Wrapper set mirrors the prost baseline (main); see HsMessageExtract.
    future_proof_inner!(
        ephemeral_message,
        view_once_message,
        view_once_message_v2,
        document_with_caption_message,
        edited_message,
    );

    None
}

fn message_context_is_forwarded(message: &wa::HsMessageExtractView<'_>) -> bool {
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

    // Content-variant set mirrors the prost baseline (main); see HsMessageExtract.
    has_forwarded_context!(
        event_message,
        template_message,
        template_button_reply_message,
        buttons_response_message,
        list_response_message,
        poll_creation_message,
        poll_creation_message_v2,
        poll_creation_message_v3,
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
        contacts_array_message,
        live_location_message,
        sticker_message,
        product_message,
        order_message,
    );

    false
}

fn context_info_is_forwarded(context: &wa::HsContextInfoExtractView<'_>) -> bool {
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
    fn read_varint_rejects_overflowing_tenth_byte() {
        let overflowing = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02];

        let err = read_varint(&overflowing).expect_err("10th byte above 0x01 must fail");

        assert!(
            matches!(err, HistorySyncError::MalformedProtobuf(msg) if msg == "varint overflows u64")
        );
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
