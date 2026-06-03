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
        // Grown on demand (#691): a full pre-count pass scanned the whole blob
        // just to size a Vec that only holds the secret-record subset (it
        // over-allocated and cost ~2.5% of the decode); plain growth is cheaper.
        msg_secret_records: Vec::new(),
        // Always retained on this path: the `!retain_blob` case returned above
        // and ran the streaming variant, so control only reaches here when the
        // caller wants the blob.
        decompressed_bytes: Some(buf.clone()),
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

                // Best-effort per-message scan: a malformed conversation (or a
                // single bad message inside it) must not abort the sync.
                result.conversations_processed += 1;
                if let Some(candidate) =
                    extract_conversation_fields(&buf[pos..end], &mut result.msg_secret_records)
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
                    && let Some(name) = extract_own_pushname(&buf[pos..end], own)
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
                            // Best-effort per-message scan (reader still advances).
                            result.conversations_processed += 1;
                            if let Some(candidate) =
                                extract_conversation_fields(value, &mut result.msg_secret_records)
                            {
                                result.tc_token_candidates.push(candidate);
                            }
                        }
                        // pushnames (repeated) — only our own is needed
                        7 => {
                            if result.own_pushname.is_none()
                                && let Some(own) = own_user
                                && let Some(name) = extract_own_pushname(value, own)
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
    // Single-byte fast-path: most history-sync varints (tags, small lengths)
    // fit in one byte (#686).
    let Some(&first) = data.first() else {
        return Err(HistorySyncError::MalformedProtobuf(
            "unexpected end of data in varint".into(),
        ));
    };
    if first < 0x80 {
        return Ok((first as u64, 1));
    }
    let mut value = (first & 0x7F) as u64;
    let mut shift = 7u32;
    for (i, &byte) in data[1..].iter().enumerate() {
        // The 10th overall byte (index 8 here; first byte consumed above) may
        // only carry a single value bit, otherwise the varint overflows u64.
        if i == 8 && (byte & 0xFE) != 0 {
            return Err(HistorySyncError::MalformedProtobuf(
                "varint overflows u64".into(),
            ));
        }
        value |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Ok((value, i + 2));
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

// Best-effort: a malformed pushname (optional metadata) must not abort the sync.
fn extract_own_pushname(data: &[u8], own_user: &str) -> Option<String> {
    let pushname = wa::PushnameView::decode_view(data).ok()?;
    if pushname.id == Some(own_user)
        && let Some(name) = pushname.pushname
    {
        return Some(name.to_string());
    }
    None
}

/// Message-secret data extracted from a conversation during streaming.
#[derive(Debug, PartialEq, Eq)]
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

/// Scan one conversation's raw protobuf fields directly (HsConversationExtract
/// tags: 1=id, 2=messages[], 21/22/28=tctoken) and decode each message (field 2)
/// as a view ONE AT A TIME, appending its secret record into `records`. Returns
/// the conversation's tctoken candidate, if any.
///
/// Best-effort, mirroring the pre-buffa per-message decode: a single malformed
/// message is skipped without discarding the rest of the conversation OR its
/// tctoken. Decoding the whole conversation as one view (all-or-nothing) would
/// drop every record + the tctoken on any single bad message, and would also
/// materialize the entire conversation tree at once.
fn extract_conversation_fields(
    data: &[u8],
    records: &mut Vec<HistoryMsgSecretRecord>,
) -> Option<TcTokenCandidate> {
    let mut pos = 0;
    // Field 1 (id) precedes messages/tctoken in tag order, so it is captured
    // before any message is processed.
    let mut chat_id: &str = "";
    let mut tc_token: &[u8] = &[];
    let mut tc_token_timestamp: Option<u64> = None;
    let mut tc_token_sender_timestamp: Option<u64> = None;

    while pos < data.len() {
        let Ok((tag, br)) = read_varint(&data[pos..]) else {
            break;
        };
        pos += br;
        let field = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;
        match (field, wt) {
            (1, wire_type::LENGTH_DELIMITED) => {
                let Ok((len, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                pos += vl;
                let Ok(end) = checked_end(pos, len, data.len(), "conv-id") else {
                    break;
                };
                // A real conversation id is a JID (always UTF-8); a non-UTF-8
                // id means the whole conversation is malformed, so skip it
                // rather than push records under an empty chat id. Field 1
                // precedes field 2, so nothing has been extracted yet.
                let Ok(id) = std::str::from_utf8(&data[pos..end]) else {
                    return None;
                };
                chat_id = id;
                pos = end;
            }
            (2, wire_type::LENGTH_DELIMITED) => {
                let Ok((len, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                pos += vl;
                let Ok(end) = checked_end(pos, len, data.len(), "conv-msg") else {
                    break;
                };
                // Decode this single message as a view; skip best-effort on a
                // malformed one without aborting the conversation.
                if let Ok(history_msg) =
                    wa::HsHistorySyncMsgExtractView::decode_view(&data[pos..end])
                {
                    push_msg_secret_record(chat_id, &history_msg, records);
                }
                pos = end;
            }
            (21, wire_type::LENGTH_DELIMITED) => {
                let Ok((len, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                pos += vl;
                let Ok(end) = checked_end(pos, len, data.len(), "conv-tctoken") else {
                    break;
                };
                tc_token = &data[pos..end];
                pos = end;
            }
            (22, wire_type::VARINT) => {
                let Ok((v, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                tc_token_timestamp = Some(v);
                pos += vl;
            }
            (28, wire_type::VARINT) => {
                let Ok((v, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                tc_token_sender_timestamp = Some(v);
                pos += vl;
            }
            _ => match skip_field(wt, data, pos) {
                Ok(np) => pos = np,
                Err(_) => break,
            },
        }
    }

    if chat_id.is_empty() {
        return None;
    }

    // tctoken candidate: only for 1:1 chats that actually carry a token.
    if let Some(parts) = wacore_binary::jid::parse_jid_fast(chat_id)
        && (parts.server == "g.us" || parts.server == "newsletter" || parts.server == "bot")
    {
        return None;
    }
    if tc_token.is_empty() {
        return None;
    }
    Some(TcTokenCandidate {
        id: chat_id.to_string(),
        tc_token: tc_token.to_vec(),
        tc_token_timestamp: tc_token_timestamp?,
        tc_token_sender_timestamp,
    })
}

/// Append a single message's secret record (if any) into `records`.
fn push_msg_secret_record(
    chat_id: &str,
    history_msg: &wa::HsHistorySyncMsgExtractView<'_>,
    records: &mut Vec<HistoryMsgSecretRecord>,
) {
    let Some(web_msg) = history_msg.message.as_option() else {
        return;
    };
    let Some(key) = web_msg.key.as_option() else {
        return;
    };
    let Some(msg_id) = key.id else {
        return;
    };
    if web_msg
        .message
        .as_option()
        .is_some_and(message_is_forwarded)
    {
        return;
    }
    let Some(secret) = web_msg.message_secret.or_else(|| {
        web_msg
            .message
            .as_option()
            .and_then(extract_message_context_secret)
    }) else {
        return;
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
#[derive(Debug, PartialEq, Eq)]
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

    /// Regression: a single malformed message inside a conversation must NOT
    /// discard the conversation's other message secrets or its tctoken. The
    /// pre-fix code decoded the whole conversation as one view (all-or-nothing),
    /// so one bad message dropped everything.
    #[test]
    fn malformed_message_does_not_drop_conversation_secrets_or_tctoken() {
        let chat = "5511777776666@s.whatsapp.net";
        let secret = vec![0x44u8; 32];
        let tc_token = vec![0x99u8; 16];

        // Hand-build the conversation bytes: id (1), a valid message (2), a
        // CORRUPT message (2) with a length-delimited subfield whose declared
        // length runs past its own bytes, then tctoken (21) + ts (22).
        fn write_tag(buf: &mut Vec<u8>, field: u32, wt: u32) {
            let tag = (field << 3) | wt;
            let mut v = tag as u64;
            loop {
                let b = (v & 0x7f) as u8;
                v >>= 7;
                if v != 0 {
                    buf.push(b | 0x80);
                } else {
                    buf.push(b);
                    break;
                }
            }
        }
        fn write_len(buf: &mut Vec<u8>, mut n: u64) {
            loop {
                let b = (n & 0x7f) as u8;
                n >>= 7;
                if n != 0 {
                    buf.push(b | 0x80);
                } else {
                    buf.push(b);
                    break;
                }
            }
        }
        fn write_ld(buf: &mut Vec<u8>, field: u32, payload: &[u8]) {
            write_tag(buf, field, 2);
            write_len(buf, payload.len() as u64);
            buf.extend_from_slice(payload);
        }

        // A valid HsHistorySyncMsgExtract (field 1 = WebMessageInfo with key+secret).
        let valid_msg = wa::HistorySyncMsg {
            message: buffa::MessageField::some(wa::WebMessageInfo {
                key: buffa::MessageField::some(wa::MessageKey {
                    remote_jid: Some(chat.to_string()),
                    from_me: Some(false),
                    id: Some("GOOD_MSG".to_string()),
                    participant: None,
                }),
                message_secret: Some(secret.clone()),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();

        // Corrupt message: field 1 (LEN) claims 50 bytes but only 1 follows.
        let corrupt_msg = {
            let mut m = Vec::new();
            write_tag(&mut m, 1, 2);
            write_len(&mut m, 50);
            m.push(0x00);
            m
        };

        let mut conv = Vec::new();
        write_ld(&mut conv, 1, chat.as_bytes()); // id
        write_ld(&mut conv, 2, &corrupt_msg); // bad message FIRST (worst case)
        write_ld(&mut conv, 2, &valid_msg); // good message after the bad one
        write_ld(&mut conv, 21, &tc_token); // tctoken
        write_tag(&mut conv, 22, 0); // tctoken timestamp (varint)
        write_len(&mut conv, 1_700_000_000);

        // Wrap conv as HistorySync.conversations[0] (field 2).
        let mut hs_bytes = Vec::new();
        write_tag(&mut hs_bytes, 1, 0); // sync_type (varint)
        write_len(
            &mut hs_bytes,
            wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP as u64,
        );
        write_ld(&mut hs_bytes, 2, &conv);

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&hs_bytes).unwrap();
        let compressed = encoder.finish().unwrap();

        // Both paths (full retain + streaming) must keep the good secret + tctoken.
        for retain in [true, false] {
            let result = process_history_sync(compressed.clone(), None, retain, None).unwrap();
            assert_eq!(
                result.msg_secret_records.len(),
                1,
                "good message secret must survive a malformed sibling (retain={retain})"
            );
            assert_eq!(result.msg_secret_records[0].msg_id, "GOOD_MSG");
            assert_eq!(result.msg_secret_records[0].secret, secret);
            assert_eq!(
                result.tc_token_candidates.len(),
                1,
                "tctoken must survive a malformed message (retain={retain})"
            );
            assert_eq!(result.tc_token_candidates[0].tc_token, tc_token);
        }
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

    /// Streaming (>64 KB crossover) and full-decompress paths must extract
    /// identical secrets, tctokens, pushname and nctSalt. Also guards the
    /// `Hs*Extract` projection field numbers against drift from the canonical
    /// proto: a wrong tag would silently change what gets extracted here.
    #[test]
    fn streaming_and_full_paths_produce_identical_results() {
        let own = "5511000000000";
        let dm = "5511777776666@s.whatsapp.net";
        let group = "123456789-987654321@g.us";
        let participant = "5511888889999@s.whatsapp.net";

        // Big 1:1 conversation (>64 KB decompressed) carrying a tctoken.
        let mut big_msgs = Vec::new();
        for i in 0..1500u32 {
            big_msgs.push(wa::HistorySyncMsg {
                message: buffa::MessageField::some(wa::WebMessageInfo {
                    key: buffa::MessageField::some(wa::MessageKey {
                        remote_jid: Some(dm.to_string()),
                        from_me: Some(i % 2 == 0),
                        id: Some(format!("BIG-{i}")),
                        participant: Some(participant.to_string()),
                    }),
                    message_timestamp: Some(1_700_000_000 + i as u64),
                    message_secret: Some(vec![(i % 251) as u8; 32]),
                    ..Default::default()
                }),
                msg_order_id: Some(i as u64 + 1),
            });
        }
        let big_conv = wa::Conversation {
            id: dm.to_string(),
            messages: big_msgs,
            tc_token: Some(vec![0xABu8; 16]),
            tc_token_timestamp: Some(1_700_000_123),
            ..Default::default()
        };

        // Group conversation: a secret message, but its tctoken must be ignored.
        let group_conv = wa::Conversation {
            id: group.to_string(),
            messages: vec![wa::HistorySyncMsg {
                message: buffa::MessageField::some(wa::WebMessageInfo {
                    key: buffa::MessageField::some(wa::MessageKey {
                        remote_jid: Some(group.to_string()),
                        from_me: Some(false),
                        id: Some("GRP-1".to_string()),
                        participant: Some(participant.to_string()),
                    }),
                    message_secret: Some(vec![0x33u8; 32]),
                    ..Default::default()
                }),
                msg_order_id: Some(1),
            }],
            tc_token: Some(vec![0xCDu8; 16]),
            tc_token_timestamp: Some(1_700_000_456),
            ..Default::default()
        };

        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![big_conv, group_conv],
            pushnames: vec![wa::Pushname {
                id: Some(own.to_string()),
                pushname: Some("Me".into()),
            }],
            nct_salt: Some(vec![0x01, 0x02, 0x03, 0x04]),
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let full = process_history_sync(compressed.clone(), Some(own), true, None).unwrap();
        let streamed = process_history_sync(compressed, Some(own), false, None).unwrap();

        assert!(full.decompressed_bytes.is_some(), "full path retains blob");
        assert!(
            streamed.decompressed_bytes.is_none(),
            "streaming path drops blob"
        );
        assert_eq!(full.nct_salt, streamed.nct_salt);
        assert_eq!(full.own_pushname, streamed.own_pushname);
        assert_eq!(full.own_pushname.as_deref(), Some("Me"));
        assert_eq!(
            full.conversations_processed,
            streamed.conversations_processed
        );
        assert_eq!(full.conversations_processed, 2);
        assert_eq!(full.tc_token_candidates, streamed.tc_token_candidates);
        assert_eq!(
            full.tc_token_candidates.len(),
            1,
            "only the DM has a tctoken"
        );
        assert_eq!(full.msg_secret_records, streamed.msg_secret_records);
        assert_eq!(full.msg_secret_records.len(), 1500 + 1);
    }
}
