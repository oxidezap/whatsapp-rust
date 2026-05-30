use bytes::Bytes;
use thiserror::Error;
use wacore_binary::zlib_pool::decompress_zlib_pooled;

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

struct ConversationInternalFields<'a> {
    id: Option<String>,
    messages: Vec<&'a [u8]>,
    tc_token: Option<Vec<u8>>,
    tc_token_timestamp: Option<u64>,
    tc_token_sender_timestamp: Option<u64>,
}

struct WebMessageInfoInternalFields {
    key: Option<MessageKeyInternalFields>,
    participant: Option<String>,
    message_secret: Option<Vec<u8>>,
    message_context_secret: Option<Vec<u8>>,
    message_is_forwarded: bool,
}

struct MessageKeyInternalFields {
    from_me: Option<bool>,
    id: Option<String>,
    participant: Option<String>,
}

fn extract_conversation_fields(data: &[u8]) -> ConversationExtraction {
    let Some(conv) = parse_conversation_internal_fields(data) else {
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

fn parse_conversation_internal_fields(data: &[u8]) -> Option<ConversationInternalFields<'_>> {
    let mut pos = 0;
    let mut conv = ConversationInternalFields {
        id: None,
        messages: Vec::new(),
        tc_token: None,
        tc_token_timestamp: None,
        tc_token_sender_timestamp: None,
    };

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            1 if wt == wire_type::LENGTH_DELIMITED => {
                conv.id = Some(read_string(data, &mut pos)?.to_string());
            }
            2 if wt == wire_type::LENGTH_DELIMITED => {
                conv.messages.push(read_length_delimited(data, &mut pos)?);
            }
            21 if wt == wire_type::LENGTH_DELIMITED => {
                conv.tc_token = Some(read_length_delimited(data, &mut pos)?.to_vec());
            }
            22 if wt == wire_type::VARINT => {
                let (val, vlen) = read_varint(data.get(pos..)?).ok()?;
                pos += vlen;
                conv.tc_token_timestamp = Some(val);
            }
            28 if wt == wire_type::VARINT => {
                let (val, vlen) = read_varint(data.get(pos..)?).ok()?;
                pos += vlen;
                conv.tc_token_sender_timestamp = Some(val);
            }
            _ => {
                pos = skip_field(wt, data, pos).ok()?;
            }
        }
    }

    Some(conv)
}

/// Returns `None` for groups, newsletters, bots, or conversations without tctokens.
fn extract_tc_token_fields(conv: &ConversationInternalFields<'_>) -> Option<TcTokenCandidate> {
    let id = conv.id.as_ref()?;

    // Early-out for non-1:1 conversations
    if let Some(parts) = wacore_binary::jid::parse_jid_fast(id)
        && (parts.server == "g.us" || parts.server == "newsletter" || parts.server == "bot")
    {
        return None;
    }

    let tc_token = conv.tc_token.as_ref().filter(|t| !t.is_empty())?.clone();
    let tc_token_timestamp = conv.tc_token_timestamp?;

    Some(TcTokenCandidate {
        id: id.clone(),
        tc_token,
        tc_token_timestamp,
        tc_token_sender_timestamp: conv.tc_token_sender_timestamp,
    })
}

fn extract_msg_secret_records(
    conv: &ConversationInternalFields<'_>,
) -> Vec<HistoryMsgSecretRecord> {
    let mut records = Vec::new();
    let Some(chat_id) = conv.id.as_ref() else {
        return records;
    };

    for history_msg in &conv.messages {
        let Some(web_msg) = parse_history_sync_msg_internal_fields(history_msg) else {
            continue;
        };
        let Some(key) = web_msg.key.as_ref() else {
            continue;
        };
        let Some(msg_id) = key.id.as_ref() else {
            continue;
        };
        if web_msg.message_is_forwarded {
            continue;
        }
        let Some(secret) = web_msg
            .message_secret
            .as_ref()
            .or(web_msg.message_context_secret.as_ref())
        else {
            continue;
        };

        records.push(HistoryMsgSecretRecord {
            chat_id: chat_id.clone(),
            from_me: key.from_me == Some(true),
            key_participant: key.participant.clone(),
            web_msg_participant: web_msg.participant.clone(),
            msg_id: msg_id.clone(),
            secret: secret.clone(),
        });
    }

    records
}

fn parse_history_sync_msg_internal_fields(data: &[u8]) -> Option<WebMessageInfoInternalFields> {
    let mut pos = 0;
    let mut web_msg = None;

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            1 if wt == wire_type::LENGTH_DELIMITED => {
                web_msg =
                    parse_web_message_info_internal_fields(read_length_delimited(data, &mut pos)?);
            }
            _ => pos = skip_field(wt, data, pos).ok()?,
        }
    }

    web_msg
}

fn parse_web_message_info_internal_fields(data: &[u8]) -> Option<WebMessageInfoInternalFields> {
    let mut pos = 0;
    let mut web_msg = WebMessageInfoInternalFields {
        key: None,
        participant: None,
        message_secret: None,
        message_context_secret: None,
        message_is_forwarded: false,
    };

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            1 if wt == wire_type::LENGTH_DELIMITED => {
                web_msg.key =
                    parse_message_key_internal_fields(read_length_delimited(data, &mut pos)?);
            }
            2 if wt == wire_type::LENGTH_DELIMITED => {
                let message = read_length_delimited(data, &mut pos)?;
                web_msg.message_context_secret = extract_message_context_secret(message);
                web_msg.message_is_forwarded = message_is_forwarded(message);
            }
            5 if wt == wire_type::LENGTH_DELIMITED => {
                web_msg.participant = Some(read_string(data, &mut pos)?.to_string());
            }
            49 if wt == wire_type::LENGTH_DELIMITED => {
                web_msg.message_secret = Some(read_length_delimited(data, &mut pos)?.to_vec());
            }
            _ => pos = skip_field(wt, data, pos).ok()?,
        }
    }

    Some(web_msg)
}

fn parse_message_key_internal_fields(data: &[u8]) -> Option<MessageKeyInternalFields> {
    let mut pos = 0;
    let mut key = MessageKeyInternalFields {
        from_me: None,
        id: None,
        participant: None,
    };

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            2 if wt == wire_type::VARINT => {
                let (val, vlen) = read_varint(data.get(pos..)?).ok()?;
                pos += vlen;
                key.from_me = Some(val != 0);
            }
            3 if wt == wire_type::LENGTH_DELIMITED => {
                key.id = Some(read_string(data, &mut pos)?.to_string());
            }
            4 if wt == wire_type::LENGTH_DELIMITED => {
                key.participant = Some(read_string(data, &mut pos)?.to_string());
            }
            _ => pos = skip_field(wt, data, pos).ok()?,
        }
    }

    Some(key)
}

fn extract_message_context_secret(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0;

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            35 if wt == wire_type::LENGTH_DELIMITED => {
                if let Some(secret) =
                    extract_message_context_info_secret(read_length_delimited(data, &mut pos)?)
                {
                    return Some(secret);
                }
            }
            _ => pos = skip_field(wt, data, pos).ok()?,
        }
    }

    None
}

fn extract_message_context_info_secret(data: &[u8]) -> Option<Vec<u8>> {
    let mut pos = 0;

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            3 if wt == wire_type::LENGTH_DELIMITED => {
                return Some(read_length_delimited(data, &mut pos)?.to_vec());
            }
            _ => pos = skip_field(wt, data, pos).ok()?,
        }
    }

    None
}

fn message_is_forwarded(data: &[u8]) -> bool {
    message_is_forwarded_at_depth(data, 0)
}

fn message_is_forwarded_at_depth(data: &[u8], depth: usize) -> bool {
    if depth > 16 {
        return false;
    }

    let mut pos = 0;
    let mut current_forwarded = false;
    let mut device_sent_inner = None;
    let mut ephemeral_inner = None;
    let mut view_once_inner = None;
    let mut view_once_v2_inner = None;
    let mut view_once_v2_extension_inner = None;
    let mut document_with_caption_inner = None;
    let mut edited_inner = None;
    let mut group_mentioned_inner = None;
    let mut bot_invoke_inner = None;
    let mut poll_creation_option_image_inner = None;
    let mut associated_child_inner = None;

    while pos < data.len() {
        let Some((tag, bytes_read)) = read_varint(data.get(pos..).unwrap_or_default()).ok() else {
            return false;
        };
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        if wt == wire_type::LENGTH_DELIMITED {
            let Some(value) = read_length_delimited(data, &mut pos) else {
                return false;
            };
            match field_number {
                31 => device_sent_inner = extract_wrapper_message(value, 2),
                37 => view_once_inner = extract_wrapper_message(value, 1),
                40 => ephemeral_inner = extract_wrapper_message(value, 1),
                53 => document_with_caption_inner = extract_wrapper_message(value, 1),
                55 => view_once_v2_inner = extract_wrapper_message(value, 1),
                58 => edited_inner = extract_wrapper_message(value, 1),
                59 => view_once_v2_extension_inner = extract_wrapper_message(value, 1),
                62 => group_mentioned_inner = extract_wrapper_message(value, 1),
                67 => bot_invoke_inner = extract_wrapper_message(value, 1),
                90 => poll_creation_option_image_inner = extract_wrapper_message(value, 1),
                91 => associated_child_inner = extract_wrapper_message(value, 1),
                field => {
                    if let Some(context_tag) = context_info_field_tag(field)
                        && context_info_carrier_is_forwarded(value, context_tag)
                    {
                        current_forwarded = true;
                    }
                }
            }
        } else {
            let Ok(new_pos) = skip_field(wt, data, pos) else {
                return false;
            };
            pos = new_pos;
        }
    }

    if let Some(inner) = [
        device_sent_inner,
        ephemeral_inner,
        view_once_inner,
        view_once_v2_inner,
        view_once_v2_extension_inner,
        document_with_caption_inner,
        edited_inner,
        group_mentioned_inner,
        bot_invoke_inner,
        poll_creation_option_image_inner,
        associated_child_inner,
    ]
    .into_iter()
    .flatten()
    .next()
    {
        return message_is_forwarded_at_depth(inner, depth + 1);
    }

    current_forwarded
}

fn extract_wrapper_message(data: &[u8], message_field: u32) -> Option<&[u8]> {
    let mut pos = 0;

    while pos < data.len() {
        let (tag, bytes_read) = read_varint(data.get(pos..)?).ok()?;
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        match field_number {
            n if n == message_field && wt == wire_type::LENGTH_DELIMITED => {
                return read_length_delimited(data, &mut pos);
            }
            _ => pos = skip_field(wt, data, pos).ok()?,
        }
    }

    None
}

fn context_info_field_tag(message_field: u32) -> Option<u32> {
    match message_field {
        75 => Some(1),
        25 | 29 | 43 => Some(3),
        39 => Some(4),
        49 | 60 | 64 => Some(5),
        78 => Some(6),
        28 => Some(7),
        36 | 42 => Some(8),
        86 => Some(11),
        45 | 48 => Some(15),
        3 | 4 | 5 | 6 | 7 | 8 | 9 | 13 | 18 | 26 | 30 | 38 => Some(17),
        _ => None,
    }
}

fn context_info_carrier_is_forwarded(data: &[u8], context_tag: u32) -> bool {
    let mut pos = 0;

    while pos < data.len() {
        let Some((tag, bytes_read)) = read_varint(data.get(pos..).unwrap_or_default()).ok() else {
            return false;
        };
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        if field_number == context_tag && wt == wire_type::LENGTH_DELIMITED {
            let Some(context) = read_length_delimited(data, &mut pos) else {
                return false;
            };
            if context_info_is_forwarded(context) {
                return true;
            }
        } else {
            let Ok(new_pos) = skip_field(wt, data, pos) else {
                return false;
            };
            pos = new_pos;
        }
    }

    false
}

fn context_info_is_forwarded(data: &[u8]) -> bool {
    let mut pos = 0;

    while pos < data.len() {
        let Some((tag, bytes_read)) = read_varint(data.get(pos..).unwrap_or_default()).ok() else {
            return false;
        };
        pos += bytes_read;
        let field_number = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;

        if field_number == 22 && wt == wire_type::VARINT {
            let Ok((val, vlen)) = read_varint(data.get(pos..).unwrap_or_default()) else {
                return false;
            };
            pos += vlen;
            if val != 0 {
                return true;
            }
        } else {
            let Ok(new_pos) = skip_field(wt, data, pos) else {
                return false;
            };
            pos = new_pos;
        }
    }

    false
}

fn read_length_delimited<'a>(data: &'a [u8], pos: &mut usize) -> Option<&'a [u8]> {
    let (len, vlen) = read_varint(data.get(*pos..)?).ok()?;
    *pos += vlen;
    let len = usize::try_from(len).ok()?;
    let end = (*pos).checked_add(len).filter(|&e| e <= data.len())?;
    let value = data.get(*pos..end)?;
    *pos = end;
    Some(value)
}

fn read_string<'a>(data: &'a [u8], pos: &mut usize) -> Option<&'a str> {
    std::str::from_utf8(read_length_delimited(data, pos)?).ok()
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
