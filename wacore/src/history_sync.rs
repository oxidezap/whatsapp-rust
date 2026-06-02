use bytes::Bytes;
use thiserror::Error;
use wacore_binary::zlib_pool::{InflateReader, decompress_zlib_pooled};

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

    // When the caller doesn't need the full decompressed blob (no Event::HistorySync
    // consumer), stream-decompress and extract incrementally so peak memory stays
    // ~one conversation instead of the whole blob.
    if !retain_blob {
        return process_history_sync_streaming(&compressed_data, own_user, MAX_DECOMPRESSED);
    }

    let decompressed = decompress_zlib_pooled(&compressed_data, MAX_DECOMPRESSED)
        .map_err(HistorySyncError::DecompressionError)?;
    drop(compressed_data);

    let buf = Bytes::from(decompressed);
    let mut pos = 0;
    // Pre-size the secret-record accumulator from a single allocation-free count
    // pass, so it never grows by repeated doubling as conversations stream in.
    let estimated_records = count_history_sync_messages(&buf);
    let mut result = HistorySyncResult {
        own_pushname: None,
        nct_salt: None,
        conversations_processed: 0,
        tc_token_candidates: Vec::new(),
        msg_secret_records: Vec::with_capacity(estimated_records),
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
            // field 2 = conversations (repeated, length-delimited)
            2 if wire_type_raw == wire_type::LENGTH_DELIMITED => {
                let (len, vlen) = read_varint(&buf[pos..])?;
                pos += vlen;
                let end = checked_end(pos, len, buf.len(), "conversation")?;

                result.conversations_processed += 1;
                if let Some(candidate) =
                    extract_conversation_fields(&buf[pos..end], &mut result.msg_secret_records)
                {
                    result.tc_token_candidates.push(candidate);
                }
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
    // Single-byte fast-path: most history-sync varints (tags, small lengths) fit in one byte.
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

/// Best-effort count of total `HistorySyncMsg` entries (field 2 inside each
/// field-2 conversation) in a decompressed HistorySync blob. Allocation-free;
/// used to pre-size the message-secret accumulator in one shot instead of letting
/// it grow by repeated doubling. An under-count (e.g. on a malformed tail) only
/// costs a few late re-grows, never correctness — the decode loop re-validates.
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

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct HistorySyncMsgInternalFields {
    // Decoded one message at a time (see `extract_conversation_fields`), so this
    // is a short-lived stack value rather than an element of a big Vec — no box
    // needed.
    #[prost(message, optional, tag = "1")]
    pub message: Option<WebMessageInfoInternalFields>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct WebMessageInfoInternalFields {
    #[prost(message, optional, tag = "1")]
    pub key: Option<MessageKeyInternalFields>,
    #[prost(message, optional, tag = "2")]
    pub message: Option<MessageInternalFields>,
    /// Parent message event time (unix seconds). Drives msg-secret retention
    /// so a horizon expires by the message's real age, not when we seeded it.
    #[prost(uint64, optional, tag = "3")]
    pub message_timestamp: Option<u64>,
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
    /// Raw `BotMetadata` bytes; only its presence matters (a bot invocation),
    /// so it stays opaque to keep the partial decode cheap.
    #[prost(bytes = "vec", optional, tag = "7")]
    pub bot_metadata: Option<Vec<u8>>,
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

    /// Whether the message invokes a bot, detected via `botMetadata` presence.
    /// botMetadata sits on the top-level `MessageContextInfo` even when wrapped,
    /// so check both the outer message and the unwrapped base. (Mentions are not
    /// decoded in this partial path; a mention-only prompt falls back to text.)
    fn invokes_bot(&self) -> bool {
        let has = |m: &Self| {
            m.message_context_info
                .as_ref()
                .is_some_and(|c| c.bot_metadata.is_some())
        };
        has(self) || has(self.base_message())
    }

    /// Whether the (unwrapped) message is a poll-creation or event message.
    /// These carry the longer poll/event retention horizon.
    fn is_poll_or_event(&self) -> bool {
        let base = self.base_message();
        base.poll_creation_message.is_some()
            || base.poll_creation_message_v2.is_some()
            || base.poll_creation_message_v3.is_some()
            || base.event_message.is_some()
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

/// Message-secret data extracted from a conversation during streaming.
#[derive(Debug, PartialEq)]
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

/// Partial reader for one conversation: walks its protobuf fields directly
/// (Conversation tags: 1=id, 2=messages[], 21/22/28=tctoken) and decodes each
/// `HistorySyncMsg` (field 2) ONE AT A TIME, extracting its secret record and
/// dropping it immediately. This avoids materializing the whole
/// `Vec<HistorySyncMsgInternalFields>` (and a heap allocation per message) just
/// to scan it — only one message is decoded at a time. The complex per-message
/// flag logic stays in prost via `HistorySyncMsgInternalFields`.
///
/// Best-effort on malformed bytes: stops at the first bad field, keeping records
/// already extracted (a malformed tail no longer discards a whole conversation).
fn extract_conversation_fields(
    data: &[u8],
    secrets_out: &mut Vec<HistoryMsgSecretRecord>,
) -> Option<TcTokenCandidate> {
    use prost::Message;

    let mut pos = 0;
    // Conversation.id (field 1) precedes messages/tctoken in tag order, so it is
    // captured before any message is processed.
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
                let Ok(id) = std::str::from_utf8(&data[pos..end]) else {
                    // A real conversation id is a JID (always UTF-8). If it
                    // isn't, the conversation is malformed; skip it rather than
                    // pushing its secrets under an empty chat id. Field 1
                    // precedes messages (field 2) in tag order, so nothing has
                    // been extracted from it yet.
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
                if let Ok(msg) = HistorySyncMsgInternalFields::decode(&data[pos..end]) {
                    push_secret_record(chat_id, &msg, secrets_out);
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

    // tc-token candidate: only for 1:1 chats that actually carry a token.
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

/// Extract a single message's secret record (if any) into `out`. The decode +
/// forwarded/poll/bot detection stays in prost via the typed fields/methods.
fn push_secret_record(
    chat_id: &str,
    history_msg: &HistorySyncMsgInternalFields,
    out: &mut Vec<HistoryMsgSecretRecord>,
) {
    let Some(web_msg) = history_msg.message.as_ref() else {
        return;
    };
    let Some(key) = web_msg.key.as_ref() else {
        return;
    };
    let Some(msg_id) = key.id.as_ref() else {
        return;
    };
    if let Some(message) = web_msg.message.as_ref()
        && message.is_forwarded()
    {
        return;
    }
    let Some(secret) = web_msg.message_secret.as_ref().or_else(|| {
        web_msg
            .message
            .as_ref()
            .and_then(|m| m.message_context_info.as_ref())
            .and_then(|mci| mci.message_secret.as_ref())
    }) else {
        return;
    };

    let is_poll_or_event = web_msg
        .message
        .as_ref()
        .map(|m| m.is_poll_or_event())
        .unwrap_or(false);
    let is_bot_invocation = web_msg
        .message
        .as_ref()
        .map(|m| m.invokes_bot())
        .unwrap_or(false);

    out.push(HistoryMsgSecretRecord {
        chat_id: chat_id.to_string(),
        from_me: key.from_me == Some(true),
        key_participant: key.participant.clone(),
        web_msg_participant: web_msg.participant.clone(),
        msg_id: msg_id.clone(),
        secret: secret.clone(),
        timestamp: web_msg.message_timestamp,
        is_poll_or_event,
        is_bot_invocation,
    });
}

/// Tctoken data extracted from a conversation during streaming.
#[derive(Debug, PartialEq)]
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

    /// The streaming path (retain_blob=false) must produce byte-for-byte the same
    /// extraction as the full-decompress path (retain_blob=true), across multiple
    /// conversations, a >64 KB conversation that spans decompress chunks, a group
    /// (no tctoken), pushname and nctSalt.
    #[test]
    fn streaming_and_full_paths_produce_identical_results() {
        use wa::history_sync::HistorySyncType;
        let own = "5511000000000";
        let dm = "5511777776666@s.whatsapp.net";
        let group = "123456789-987654321@g.us";
        let participant = "5511888889999@s.whatsapp.net";

        // Big 1:1 conversation (>64 KB decompressed) carrying a tctoken.
        let mut big_msgs = Vec::new();
        for i in 0..1500u32 {
            big_msgs.push(wa::HistorySyncMsg {
                message: Some(wa::WebMessageInfo {
                    key: wa::MessageKey {
                        remote_jid: Some(dm.to_string()),
                        from_me: Some(i % 2 == 0),
                        id: Some(format!("BIG-{i}")),
                        participant: Some(participant.to_string()),
                    },
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

        // Group conversation: a secret message, but the tctoken must be ignored.
        let group_conv = wa::Conversation {
            id: group.to_string(),
            messages: vec![wa::HistorySyncMsg {
                message: Some(wa::WebMessageInfo {
                    key: wa::MessageKey {
                        remote_jid: Some(group.to_string()),
                        from_me: Some(false),
                        id: Some("GRP-1".to_string()),
                        participant: Some(participant.to_string()),
                    },
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
            sync_type: HistorySyncType::InitialBootstrap as i32,
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
