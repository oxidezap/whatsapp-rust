use bytes::Bytes;
use compact_str::CompactString;
use std::sync::Arc;
use thiserror::Error;
use wacore_binary::zlib_pool::{InflateReader, decompress_zlib_pooled};
use waproto::tags;

#[derive(Debug, Error)]
#[non_exhaustive]
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
    pub const START_GROUP: u32 = 3;
    pub const END_GROUP: u32 = 4;
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
    let mut result = HistorySyncResult {
        own_pushname: None,
        nct_salt: None,
        conversations_processed: 0,
        tc_token_candidates: Vec::new(),
        // Grown on demand: a full pre-count pass scanned the whole blob just to
        // size a Vec that only holds the secret-record subset (it over-allocated
        // and cost ~2.5% of the decode); plain growth is cheaper here.
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
            // conversations (repeated, length-delimited)
            tags::history_sync::CONVERSATIONS if wire_type_raw == wire_type::LENGTH_DELIMITED => {
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

            // pushnames (repeated, length-delimited).
            // Uses `Option::is_some()` in the guard rather than an
            // `if let` guard — the latter requires Rust 1.94+. The inner
            // `if let` is the defensive complement: if the guard's
            // invariant is ever weakened by a future refactor, we skip
            // the arm body instead of panicking.
            tags::history_sync::PUSHNAMES
                if own_user.is_some()
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

            // nctSalt (optional bytes, length-delimited)
            // Delivered during initial pairing so cstoken is available immediately.
            // Source: storeNctSaltFromHistorySync in WAWeb/History/MsgHandlerAction.js
            tags::history_sync::NCT_SALT if wire_type_raw == wire_type::LENGTH_DELIMITED => {
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
                        tags::history_sync::CONVERSATIONS => {
                            result.conversations_processed += 1;
                            if let Some(candidate) =
                                extract_conversation_fields(value, &mut result.msg_secret_records)
                            {
                                result.tc_token_candidates.push(candidate);
                            }
                        }
                        // pushnames (repeated) — only our own is needed
                        tags::history_sync::PUSHNAMES => {
                            if result.own_pushname.is_none()
                                && let Some(own) = own_user
                                && let Some(name) = extract_own_pushname(value, own)
                            {
                                result.own_pushname = Some(name);
                            }
                        }
                        tags::history_sync::NCT_SALT if !value.is_empty() => {
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
#[inline(always)]
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
// inline(always): per-field hot path; the thin-LTO bench profile keeps plain
// #[inline] candidates outlined and the call overhead dominates the walk.
#[inline(always)]
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
        // The 10th byte of a 64-bit varint may only carry one bit; prost
        // rejects the overflow instead of silently truncating it.
        if shift == 63 && byte > 1 {
            return Err(HistorySyncError::MalformedProtobuf(
                "varint overflows 64 bits".into(),
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

/// Skip a protobuf field based on wire type, returning the new position.
#[inline(always)]
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
            // id (string)
            tags::pushname::ID if wt == wire_type::LENGTH_DELIMITED => {
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
            // pushname (string)
            tags::pushname::PUSHNAME if wt == wire_type::LENGTH_DELIMITED => {
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

// Schema pinning for every hand-written `#[prost(tag)]` literal above (prost
// attributes only accept literals, so they cannot reference the generated
// consts directly). If whatsapp.proto renumbers, renames or removes any of
// these fields, compilation fails here instead of the partial decoder silently
// reading the wrong wire field.
const _: () = {
    assert!(tags::history_sync_msg::MESSAGE == 1);

    assert!(tags::web_message_info::KEY == 1);
    assert!(tags::web_message_info::MESSAGE == 2);
    assert!(tags::web_message_info::MESSAGE_TIMESTAMP == 3);
    assert!(tags::web_message_info::PARTICIPANT == 5);
    assert!(tags::web_message_info::MESSAGE_SECRET == 49);

    assert!(tags::message_key::FROM_ME == 2);
    assert!(tags::message_key::ID == 3);
    assert!(tags::message_key::PARTICIPANT == 4);

    assert!(tags::message::IMAGE_MESSAGE == 3);
    assert!(tags::message::CONTACT_MESSAGE == 4);
    assert!(tags::message::LOCATION_MESSAGE == 5);
    assert!(tags::message::EXTENDED_TEXT_MESSAGE == 6);
    assert!(tags::message::DOCUMENT_MESSAGE == 7);
    assert!(tags::message::AUDIO_MESSAGE == 8);
    assert!(tags::message::VIDEO_MESSAGE == 9);
    assert!(tags::message::CONTACTS_ARRAY_MESSAGE == 13);
    assert!(tags::message::LIVE_LOCATION_MESSAGE == 18);
    assert!(tags::message::TEMPLATE_MESSAGE == 25);
    assert!(tags::message::STICKER_MESSAGE == 26);
    assert!(tags::message::GROUP_INVITE_MESSAGE == 28);
    assert!(tags::message::TEMPLATE_BUTTON_REPLY_MESSAGE == 29);
    assert!(tags::message::PRODUCT_MESSAGE == 30);
    assert!(tags::message::DEVICE_SENT_MESSAGE == 31);
    assert!(tags::message::MESSAGE_CONTEXT_INFO == 35);
    assert!(tags::message::LIST_MESSAGE == 36);
    assert!(tags::message::VIEW_ONCE_MESSAGE == 37);
    assert!(tags::message::ORDER_MESSAGE == 38);
    assert!(tags::message::LIST_RESPONSE_MESSAGE == 39);
    assert!(tags::message::EPHEMERAL_MESSAGE == 40);
    assert!(tags::message::BUTTONS_MESSAGE == 42);
    assert!(tags::message::BUTTONS_RESPONSE_MESSAGE == 43);
    assert!(tags::message::INTERACTIVE_MESSAGE == 45);
    assert!(tags::message::INTERACTIVE_RESPONSE_MESSAGE == 48);
    assert!(tags::message::POLL_CREATION_MESSAGE == 49);
    assert!(tags::message::DOCUMENT_WITH_CAPTION_MESSAGE == 53);
    assert!(tags::message::VIEW_ONCE_MESSAGE_V2 == 55);
    assert!(tags::message::EDITED_MESSAGE == 58);
    assert!(tags::message::POLL_CREATION_MESSAGE_V2 == 60);
    assert!(tags::message::POLL_CREATION_MESSAGE_V3 == 64);
    assert!(tags::message::EVENT_MESSAGE == 75);
    assert!(tags::message::NEWSLETTER_ADMIN_INVITE_MESSAGE == 78);
    assert!(tags::message::STICKER_PACK_MESSAGE == 86);

    assert!(tags::message_context_info::MESSAGE_SECRET == 3);
    assert!(tags::message_context_info::BOT_METADATA == 7);

    assert!(tags::context_info::IS_FORWARDED == 22);

    assert!(tags::message::device_sent_message::MESSAGE == 2);
    assert!(tags::message::future_proof_message::MESSAGE == 1);

    // ContextInfoTagN carriers: pin the `contextInfo` field number of every
    // proto message each carrier stands in for.
    assert!(tags::message::event_message::CONTEXT_INFO == 1); // Tag1
    assert!(tags::message::template_message::CONTEXT_INFO == 3); // Tag3
    assert!(tags::message::template_button_reply_message::CONTEXT_INFO == 3);
    assert!(tags::message::buttons_response_message::CONTEXT_INFO == 3);
    assert!(tags::message::list_response_message::CONTEXT_INFO == 4); // Tag4
    assert!(tags::message::poll_creation_message::CONTEXT_INFO == 5); // Tag5 (v2/v3 share the type)
    assert!(tags::message::newsletter_admin_invite_message::CONTEXT_INFO == 6); // Tag6
    assert!(tags::message::group_invite_message::CONTEXT_INFO == 7); // Tag7
    assert!(tags::message::list_message::CONTEXT_INFO == 8); // Tag8
    assert!(tags::message::buttons_message::CONTEXT_INFO == 8);
    assert!(tags::message::sticker_pack_message::CONTEXT_INFO == 11); // Tag11
    assert!(tags::message::interactive_message::CONTEXT_INFO == 15); // Tag15
    assert!(tags::message::interactive_response_message::CONTEXT_INFO == 15);
    assert!(tags::message::image_message::CONTEXT_INFO == 17); // Tag17
    assert!(tags::message::contact_message::CONTEXT_INFO == 17);
    assert!(tags::message::location_message::CONTEXT_INFO == 17);
    assert!(tags::message::extended_text_message::CONTEXT_INFO == 17);
    assert!(tags::message::document_message::CONTEXT_INFO == 17);
    assert!(tags::message::audio_message::CONTEXT_INFO == 17);
    assert!(tags::message::video_message::CONTEXT_INFO == 17);
    assert!(tags::message::contacts_array_message::CONTEXT_INFO == 17);
    assert!(tags::message::live_location_message::CONTEXT_INFO == 17);
    assert!(tags::message::sticker_message::CONTEXT_INFO == 17);
    assert!(tags::message::product_message::CONTEXT_INFO == 17);
    assert!(tags::message::order_message::CONTEXT_INFO == 17);
};

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

/// Message secret bytes, inline up to 32 bytes (the universal size of real
/// message secrets) so extracting a record costs no heap allocation. Larger
/// payloads spill to the heap, preserving arbitrary-length wire semantics.
/// Inline capacity of [`SecretBytes`]; real message secrets are 32 bytes.
pub const SECRET_INLINE_CAP: usize = 32;

#[derive(Clone)]
pub enum SecretBytes {
    Inline {
        len: u8,
        buf: [u8; SECRET_INLINE_CAP],
    },
    Heap(Vec<u8>),
}

impl SecretBytes {
    pub const INLINE_CAP: usize = SECRET_INLINE_CAP;

    pub fn as_slice(&self) -> &[u8] {
        match self {
            SecretBytes::Inline { len, buf } => &buf[..*len as usize],
            SecretBytes::Heap(v) => v,
        }
    }

    pub fn into_vec(self) -> Vec<u8> {
        match self {
            SecretBytes::Inline { len, buf } => buf[..len as usize].to_vec(),
            SecretBytes::Heap(v) => v,
        }
    }
}

impl From<&[u8]> for SecretBytes {
    fn from(bytes: &[u8]) -> Self {
        if bytes.len() <= Self::INLINE_CAP {
            let mut buf = [0u8; Self::INLINE_CAP];
            buf[..bytes.len()].copy_from_slice(bytes);
            SecretBytes::Inline {
                len: bytes.len() as u8,
                buf,
            }
        } else {
            SecretBytes::Heap(bytes.to_vec())
        }
    }
}

impl From<Vec<u8>> for SecretBytes {
    fn from(bytes: Vec<u8>) -> Self {
        // Compacting small vectors frees the prost-side allocation early on
        // the fallback path.
        if bytes.len() <= Self::INLINE_CAP {
            SecretBytes::from(bytes.as_slice())
        } else {
            SecretBytes::Heap(bytes)
        }
    }
}

impl std::ops::Deref for SecretBytes {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

// Content equality: an inline and a heap value holding the same bytes are the
// same secret (the representation is an allocation detail).
impl PartialEq for SecretBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for SecretBytes {}

impl std::fmt::Debug for SecretBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("SecretBytes")
            .field(&self.as_slice())
            .finish()
    }
}

/// Outcome of the single-pass borrowed extraction of one `HistorySyncMsg`.
enum FastExtract<'a> {
    /// No record: no secret, missing key id, forwarded, or wire bytes that
    /// prost's decode would reject (the failed-decode skip).
    NoRecord,
    Record(FastRecord<'a>),
    /// Wire shapes whose prost merge semantics a single-pass walk cannot
    /// reproduce (repeated occurrences of message-typed fields, pathological
    /// nesting). The caller reruns the prost mirror-struct decode, which is
    /// exact by construction.
    Fallback,
}

struct FastRecord<'a> {
    from_me: bool,
    msg_id: &'a str,
    key_participant: Option<&'a str>,
    web_msg_participant: Option<&'a str>,
    timestamp: Option<u64>,
    secret: &'a [u8],
    is_poll_or_event: bool,
    is_bot_invocation: bool,
}

/// Carrier slots inspected by the forwarded check; mirrors the
/// `is_forwarded` chain of `MessageInternalFields`.
const N_CARRIERS: usize = 27;

/// Map a `Message` field tag to its forwarded-carrier slot and the
/// `contextInfo` tag inside that carrier. A `match` so the dispatch compiles
/// to a jump table; schema-pinned through the tags consts in the patterns.
#[rustfmt::skip]
fn carrier_slot(field: u32) -> Option<(usize, u32)> {
    Some(match field {
        tags::message::EXTENDED_TEXT_MESSAGE => (0, tags::message::extended_text_message::CONTEXT_INFO),
        tags::message::IMAGE_MESSAGE => (1, tags::message::image_message::CONTEXT_INFO),
        tags::message::VIDEO_MESSAGE => (2, tags::message::video_message::CONTEXT_INFO),
        tags::message::AUDIO_MESSAGE => (3, tags::message::audio_message::CONTEXT_INFO),
        tags::message::DOCUMENT_MESSAGE => (4, tags::message::document_message::CONTEXT_INFO),
        tags::message::STICKER_MESSAGE => (5, tags::message::sticker_message::CONTEXT_INFO),
        tags::message::LOCATION_MESSAGE => (6, tags::message::location_message::CONTEXT_INFO),
        tags::message::LIVE_LOCATION_MESSAGE => (7, tags::message::live_location_message::CONTEXT_INFO),
        tags::message::CONTACT_MESSAGE => (8, tags::message::contact_message::CONTEXT_INFO),
        tags::message::CONTACTS_ARRAY_MESSAGE => (9, tags::message::contacts_array_message::CONTEXT_INFO),
        tags::message::BUTTONS_MESSAGE => (10, tags::message::buttons_message::CONTEXT_INFO),
        tags::message::BUTTONS_RESPONSE_MESSAGE => (11, tags::message::buttons_response_message::CONTEXT_INFO),
        tags::message::LIST_MESSAGE => (12, tags::message::list_message::CONTEXT_INFO),
        tags::message::LIST_RESPONSE_MESSAGE => (13, tags::message::list_response_message::CONTEXT_INFO),
        tags::message::TEMPLATE_MESSAGE => (14, tags::message::template_message::CONTEXT_INFO),
        tags::message::TEMPLATE_BUTTON_REPLY_MESSAGE => (15, tags::message::template_button_reply_message::CONTEXT_INFO),
        tags::message::INTERACTIVE_MESSAGE => (16, tags::message::interactive_message::CONTEXT_INFO),
        tags::message::INTERACTIVE_RESPONSE_MESSAGE => (17, tags::message::interactive_response_message::CONTEXT_INFO),
        tags::message::POLL_CREATION_MESSAGE => (18, tags::message::poll_creation_message::CONTEXT_INFO),
        tags::message::POLL_CREATION_MESSAGE_V2 => (19, tags::message::poll_creation_message::CONTEXT_INFO),
        tags::message::POLL_CREATION_MESSAGE_V3 => (20, tags::message::poll_creation_message::CONTEXT_INFO),
        tags::message::PRODUCT_MESSAGE => (21, tags::message::product_message::CONTEXT_INFO),
        tags::message::ORDER_MESSAGE => (22, tags::message::order_message::CONTEXT_INFO),
        tags::message::GROUP_INVITE_MESSAGE => (23, tags::message::group_invite_message::CONTEXT_INFO),
        tags::message::EVENT_MESSAGE => (24, tags::message::event_message::CONTEXT_INFO),
        tags::message::STICKER_PACK_MESSAGE => (25, tags::message::sticker_pack_message::CONTEXT_INFO),
        tags::message::NEWSLETTER_ADMIN_INVITE_MESSAGE => (26, tags::message::newsletter_admin_invite_message::CONTEXT_INFO),
        _ => return None,
    })
}

/// Wrapper slots for `base_message`, in the same priority order as
/// `MessageInternalFields::base_message`.
const N_WRAPPERS: usize = 6;

/// Inner-message tag inside each wrapper, indexed by slot.
#[rustfmt::skip]
const WRAPPER_INNER_TAGS: [u32; N_WRAPPERS] = [
    tags::message::device_sent_message::MESSAGE,
    tags::message::future_proof_message::MESSAGE,
    tags::message::future_proof_message::MESSAGE,
    tags::message::future_proof_message::MESSAGE,
    tags::message::future_proof_message::MESSAGE,
    tags::message::future_proof_message::MESSAGE,
];

fn wrapper_slot(field: u32) -> Option<usize> {
    Some(match field {
        tags::message::DEVICE_SENT_MESSAGE => 0,
        tags::message::EPHEMERAL_MESSAGE => 1,
        tags::message::VIEW_ONCE_MESSAGE => 2,
        tags::message::VIEW_ONCE_MESSAGE_V2 => 3,
        tags::message::DOCUMENT_WITH_CAPTION_MESSAGE => 4,
        tags::message::EDITED_MESSAGE => 5,
        _ => return None,
    })
}

enum WalkStop {
    /// Wire bytes prost's decode would reject; maps to the no-record skip.
    Malformed,
    /// Wire shapes the single-pass walk does not reproduce (proto2 groups,
    /// repeated message-typed fields, deep nesting); rerun prost.
    Fallback,
}

/// Iterate one protobuf level, yielding a [`WireField`] per field. Yields one
/// `Err` on malformed framing (or a group field, which only prost can skip),
/// then stops.
struct FieldIter<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> FieldIter<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
}

/// One field of a protobuf level: number, wire type, the value slice for
/// length-delimited fields, and the raw value for varint fields.
struct WireField<'a> {
    field: u32,
    wt: u32,
    value: Option<&'a [u8]>,
    varint: u64,
}

impl<'a> Iterator for FieldIter<'a> {
    type Item = Result<WireField<'a>, WalkStop>;

    // inline(always): one call per protobuf field; outlined (as the thin-LTO
    // bench build does by default) the WireField/Result plumbing goes through
    // memory and this single function is ~30% of the history-sync profile.
    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() {
            return None;
        }
        let Ok((tag, br)) = read_varint(&self.data[self.pos..]) else {
            self.pos = self.data.len();
            return Some(Err(WalkStop::Malformed));
        };
        // prost decodes the key as u32 and requires field >= 1; mirror both
        // so malformed keys fail like a failed decode instead of truncating
        // into a (possibly mirrored) field number.
        if tag > u64::from(u32::MAX) || (tag >> 3) == 0 {
            self.pos = self.data.len();
            return Some(Err(WalkStop::Malformed));
        }
        self.pos += br;
        let field = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;
        match wt {
            wire_type::LENGTH_DELIMITED => {
                let Ok((len, vl)) = read_varint(&self.data[self.pos..]) else {
                    self.pos = self.data.len();
                    return Some(Err(WalkStop::Malformed));
                };
                self.pos += vl;
                let Ok(end) = checked_end(self.pos, len, self.data.len(), "field") else {
                    self.pos = self.data.len();
                    return Some(Err(WalkStop::Malformed));
                };
                let value = &self.data[self.pos..end];
                self.pos = end;
                Some(Ok(WireField {
                    field,
                    wt,
                    value: Some(value),
                    varint: 0,
                }))
            }
            wire_type::VARINT => {
                let Ok((v, vl)) = read_varint(&self.data[self.pos..]) else {
                    self.pos = self.data.len();
                    return Some(Err(WalkStop::Malformed));
                };
                self.pos += vl;
                Some(Ok(WireField {
                    field,
                    wt,
                    value: None,
                    varint: v,
                }))
            }
            // proto2 groups have no length prefix; only prost's recursive
            // skipper can step over them.
            wire_type::START_GROUP | wire_type::END_GROUP => {
                self.pos = self.data.len();
                Some(Err(WalkStop::Fallback))
            }
            _ => match skip_field(wt, self.data, self.pos) {
                Ok(np) => {
                    self.pos = np;
                    Some(Ok(WireField {
                        field,
                        wt,
                        value: None,
                        varint: 0,
                    }))
                }
                Err(_) => {
                    self.pos = self.data.len();
                    Some(Err(WalkStop::Malformed))
                }
            },
        }
    }
}

/// Single-pass borrowed extraction of one `HistorySyncMsg`, replacing the
/// prost decode of the 30+-field mirror struct for well-formed messages.
///
/// Semantics contract (verified by the differential oracle tests): the outcome
/// matches `HistorySyncMsgInternalFields::decode` + `push_secret_record`
/// exactly. Key equivalences:
/// - scalar fields repeated across (merged) occurrences: overwrite-when-present
///   in document order == prost merge last-wins;
/// - presence-only flags (poll/event/bot): OR across occurrences == merge,
///   since a later occurrence can never unset presence;
/// - wire bytes prost would reject (wrong wire type on a mirrored field,
///   malformed framing, invalid UTF-8 in a mirrored string): `NoRecord`,
///   matching the failed-decode skip;
/// - shapes that need real recursive merging (a repeated message-typed field
///   that the flag logic reads) return `Fallback` and rerun prost.
fn fast_extract(history_msg: &[u8]) -> FastExtract<'_> {
    let mut from_me = false;
    let mut msg_id: Option<&str> = None;
    let mut key_participant: Option<&str> = None;
    let mut web_msg_participant: Option<&str> = None;
    let mut timestamp: Option<u64> = None;
    let mut top_secret: Option<&[u8]> = None;
    let mut msg_slice: Option<&[u8]> = None;

    for item in FieldIter::new(history_msg) {
        let f = match item {
            Ok(f) => f,
            Err(stop) => return stop.into(),
        };
        if f.field == tags::history_sync_msg::MESSAGE {
            // WebMessageInfo (message-typed: must be length-delimited).
            let Some(web_msg) = f.value else {
                return FastExtract::NoRecord;
            };
            for item in FieldIter::new(web_msg) {
                let f = match item {
                    Ok(f) => f,
                    Err(stop) => return stop.into(),
                };
                match f.field {
                    tags::web_message_info::KEY => {
                        let Some(key) = f.value else {
                            return FastExtract::NoRecord;
                        };
                        for item in FieldIter::new(key) {
                            let f = match item {
                                Ok(f) => f,
                                Err(stop) => return stop.into(),
                            };
                            match f.field {
                                tags::message_key::FROM_ME => {
                                    if f.wt != wire_type::VARINT {
                                        return FastExtract::NoRecord;
                                    }
                                    from_me = f.varint != 0;
                                }
                                tags::message_key::ID => {
                                    let Some(Ok(s)) = f.value.map(std::str::from_utf8) else {
                                        return FastExtract::NoRecord;
                                    };
                                    msg_id = Some(s);
                                }
                                tags::message_key::PARTICIPANT => {
                                    let Some(Ok(s)) = f.value.map(std::str::from_utf8) else {
                                        return FastExtract::NoRecord;
                                    };
                                    key_participant = Some(s);
                                }
                                _ => {}
                            }
                        }
                    }
                    tags::web_message_info::MESSAGE => {
                        let Some(msg) = f.value else {
                            return FastExtract::NoRecord;
                        };
                        if msg_slice.is_some() {
                            // Two occurrences merge recursively in prost; the
                            // flag logic then reads the merged struct.
                            return FastExtract::Fallback;
                        }
                        msg_slice = Some(msg);
                    }
                    tags::web_message_info::MESSAGE_TIMESTAMP => {
                        if f.wt != wire_type::VARINT {
                            return FastExtract::NoRecord;
                        }
                        timestamp = Some(f.varint);
                    }
                    tags::web_message_info::PARTICIPANT => {
                        let Some(Ok(s)) = f.value.map(std::str::from_utf8) else {
                            return FastExtract::NoRecord;
                        };
                        web_msg_participant = Some(s);
                    }
                    tags::web_message_info::MESSAGE_SECRET => {
                        let Some(secret) = f.value else {
                            return FastExtract::NoRecord;
                        };
                        top_secret = Some(secret);
                    }
                    _ => {}
                }
            }
        }
    }

    // One pass over the outer message level: context secret, bot metadata,
    // wrapper slices and this level's flag carriers, all collected together.
    let outer = match msg_slice.map(scan_message_level) {
        None => None,
        Some(Ok(level)) => Some(level),
        Some(Err(stop)) => return stop.into(),
    };

    let context_secret = outer.as_ref().and_then(|l| l.context_secret);
    let Some(secret) = top_secret.or(context_secret) else {
        return FastExtract::NoRecord;
    };
    let Some(msg_id) = msg_id else {
        return FastExtract::NoRecord;
    };

    let mut is_poll_or_event = false;
    let mut is_bot_invocation = outer.as_ref().is_some_and(|l| l.has_bot_metadata);
    if let Some(outer_level) = outer {
        // botMetadata counts on the outer message and on the unwrapped base
        // (prost parity: `has(self) || has(base_message())`).
        let base = match unwrap_to_base(outer_level) {
            Ok(base) => base,
            Err(stop) => return stop.into(),
        };
        if base.forwarded.contains(&Some(true)) {
            return FastExtract::NoRecord;
        }
        is_poll_or_event = base.is_poll_or_event;
        is_bot_invocation |= base.has_bot_metadata;
    }

    FastExtract::Record(FastRecord {
        from_me,
        msg_id,
        key_participant,
        web_msg_participant,
        timestamp,
        secret,
        is_poll_or_event,
        is_bot_invocation,
    })
}

impl From<WalkStop> for FastExtract<'_> {
    fn from(stop: WalkStop) -> Self {
        match stop {
            WalkStop::Malformed => FastExtract::NoRecord,
            WalkStop::Fallback => FastExtract::Fallback,
        }
    }
}

/// Everything one pass over a `Message` level yields for the record logic.
struct MsgLevel<'a> {
    /// `message_context_info.message_secret`, last occurrence wins.
    context_secret: Option<&'a [u8]>,
    has_bot_metadata: bool,
    is_poll_or_event: bool,
    /// Final merged `context_info.is_forwarded` per carrier slot.
    forwarded: [Option<bool>; N_CARRIERS],
    /// Wrapper payloads found at this level, by priority slot.
    wrappers: [Option<&'a [u8]>; N_WRAPPERS],
}

/// Scan one `Message` level in a single pass, mirroring how prost would merge
/// it: scalars overwrite-when-present in document order, presence flags OR.
fn scan_message_level(msg: &[u8]) -> Result<MsgLevel<'_>, WalkStop> {
    let mut level = MsgLevel {
        context_secret: None,
        has_bot_metadata: false,
        is_poll_or_event: false,
        forwarded: [None; N_CARRIERS],
        wrappers: [None; N_WRAPPERS],
    };

    for item in FieldIter::new(msg) {
        let f = item?;
        match f.field {
            tags::message::MESSAGE_CONTEXT_INFO => {
                let mci = f.value.ok_or(WalkStop::Malformed)?;
                let (secret, bot) = scan_context_info(mci)?;
                if let Some(s) = secret {
                    level.context_secret = Some(s);
                }
                level.has_bot_metadata |= bot;
            }
            tags::message::POLL_CREATION_MESSAGE
            | tags::message::POLL_CREATION_MESSAGE_V2
            | tags::message::POLL_CREATION_MESSAGE_V3
            | tags::message::EVENT_MESSAGE => {
                // Message-typed in the mirror: a non-length-delimited
                // occurrence fails prost's decode.
                f.value.ok_or(WalkStop::Malformed)?;
                level.is_poll_or_event = true;
            }
            _ => {}
        }

        if let Some(slot) = wrapper_slot(f.field) {
            let value = f.value.ok_or(WalkStop::Malformed)?;
            if level.wrappers[slot].is_some() {
                // Two occurrences merge recursively in prost.
                return Err(WalkStop::Fallback);
            }
            level.wrappers[slot] = Some(value);
        } else if let Some((slot, ctx_tag)) = carrier_slot(f.field) {
            let carrier = f.value.ok_or(WalkStop::Malformed)?;
            for item in FieldIter::new(carrier) {
                let f = item?;
                if f.field == ctx_tag {
                    let ctx = f.value.ok_or(WalkStop::Malformed)?;
                    for item in FieldIter::new(ctx) {
                        let f = item?;
                        if f.field == tags::context_info::IS_FORWARDED {
                            if f.wt != wire_type::VARINT {
                                return Err(WalkStop::Malformed);
                            }
                            level.forwarded[slot] = Some(f.varint != 0);
                        }
                    }
                }
            }
        }
    }

    Ok(level)
}

/// Follow the wrapper chain (device-sent/ephemeral/view-once/...) to the base
/// message, mirroring `MessageInternalFields::base_message`: at each level the
/// first wrapper in priority order that has an inner message wins.
fn unwrap_to_base(mut level: MsgLevel<'_>) -> Result<MsgLevel<'_>, WalkStop> {
    // prost aborts decode past its recursion limit (no record); deeper chains
    // defer to it rather than re-deriving the exact cutoff here.
    const MAX_UNWRAPS: usize = 40;

    for _ in 0..MAX_UNWRAPS {
        let mut next: Option<&[u8]> = None;
        for (&inner_tag, wrapper) in WRAPPER_INNER_TAGS.iter().zip(level.wrappers) {
            let Some(wrapper) = wrapper else {
                continue;
            };
            let mut inner: Option<&[u8]> = None;
            for item in FieldIter::new(wrapper) {
                let f = item?;
                if f.field == inner_tag {
                    let value = f.value.ok_or(WalkStop::Malformed)?;
                    if inner.is_some() {
                        return Err(WalkStop::Fallback);
                    }
                    inner = Some(value);
                }
            }
            if let Some(inner) = inner {
                next = Some(inner);
                break;
            }
        }

        match next {
            Some(inner) => level = scan_message_level(inner)?,
            None => return Ok(level),
        }
    }
    Err(WalkStop::Fallback)
}

/// Scan one `MessageContextInfo` occurrence: `(message_secret, bot present)`.
fn scan_context_info(mci: &[u8]) -> Result<(Option<&[u8]>, bool), WalkStop> {
    let mut secret: Option<&[u8]> = None;
    let mut bot = false;
    for item in FieldIter::new(mci) {
        let f = item?;
        match f.field {
            tags::message_context_info::MESSAGE_SECRET => {
                secret = Some(f.value.ok_or(WalkStop::Malformed)?);
            }
            tags::message_context_info::BOT_METADATA => {
                f.value.ok_or(WalkStop::Malformed)?;
                bot = true;
            }
            _ => {}
        }
    }
    Ok((secret, bot))
}

/// Message-secret data extracted from a conversation during streaming.
#[derive(Debug, PartialEq)]
pub struct HistoryMsgSecretRecord {
    /// Conversation JID. `Arc<str>` because every record of a conversation
    /// shares the same id: one allocation per conversation, not per record.
    pub chat_id: Arc<str>,
    pub from_me: bool,
    pub key_participant: Option<String>,
    pub web_msg_participant: Option<String>,
    /// Message id; inline for the typical 20-22 char WhatsApp id.
    pub msg_id: CompactString,
    /// Secret bytes; inline for the universal 32-byte size.
    pub secret: SecretBytes,
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
/// (id, messages[], tctoken trio) and decodes each `HistorySyncMsg` ONE AT A
/// TIME, extracting its secret record and dropping it immediately. This avoids
/// materializing the whole `Vec<HistorySyncMsgInternalFields>` (and a heap
/// allocation per message) just to scan it — only one message is decoded at a
/// time. The complex per-message flag logic stays in prost via
/// `HistorySyncMsgInternalFields`.
///
/// Best-effort on malformed bytes: stops at the first bad field, keeping records
/// already extracted (a malformed tail no longer discards a whole conversation).
fn extract_conversation_fields(
    data: &[u8],
    secrets_out: &mut Vec<HistoryMsgSecretRecord>,
) -> Option<TcTokenCandidate> {
    use prost::Message;

    let mut pos = 0;
    // Conversation.id precedes messages/tctoken in tag order, so it is
    // captured before any message is processed.
    let mut chat_id: &str = "";
    // Shared id handed to every record of this conversation; built on first
    // use and invalidated if a (malformed) blob re-orders the id after data.
    let mut chat_id_shared: Option<Arc<str>> = None;
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
            (tags::conversation::ID, wire_type::LENGTH_DELIMITED) => {
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
                    // pushing its secrets under an empty chat id. The id
                    // precedes messages in tag order, so nothing has been
                    // extracted from it yet.
                    return None;
                };
                chat_id = id;
                chat_id_shared = None;
                pos = end;
            }
            (tags::conversation::MESSAGES, wire_type::LENGTH_DELIMITED) => {
                let Ok((len, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                pos += vl;
                let Ok(end) = checked_end(pos, len, data.len(), "conv-msg") else {
                    break;
                };
                // The id guard keeps records from a (malformed) blob that
                // re-orders messages before the conversation id from landing
                // under an empty chat id.
                if !chat_id.is_empty() {
                    match fast_extract(&data[pos..end]) {
                        FastExtract::NoRecord => {}
                        FastExtract::Record(r) => {
                            secrets_out.push(HistoryMsgSecretRecord {
                                chat_id: chat_id_shared
                                    .get_or_insert_with(|| Arc::from(chat_id))
                                    .clone(),
                                from_me: r.from_me,
                                key_participant: r.key_participant.map(str::to_owned),
                                web_msg_participant: r.web_msg_participant.map(str::to_owned),
                                msg_id: CompactString::new(r.msg_id),
                                secret: SecretBytes::from(r.secret),
                                timestamp: r.timestamp,
                                is_poll_or_event: r.is_poll_or_event,
                                is_bot_invocation: r.is_bot_invocation,
                            });
                        }
                        // Rare wire shapes (repeated message-typed fields,
                        // pathological nesting): prost merge is the oracle.
                        FastExtract::Fallback => {
                            if let Ok(msg) = HistorySyncMsgInternalFields::decode(&data[pos..end]) {
                                push_secret_record(chat_id, &mut chat_id_shared, msg, secrets_out);
                            }
                        }
                    }
                }
                pos = end;
            }
            (tags::conversation::TC_TOKEN, wire_type::LENGTH_DELIMITED) => {
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
            (tags::conversation::TC_TOKEN_TIMESTAMP, wire_type::VARINT) => {
                let Ok((v, vl)) = read_varint(&data[pos..]) else {
                    break;
                };
                tc_token_timestamp = Some(v);
                pos += vl;
            }
            (tags::conversation::TC_TOKEN_SENDER_TIMESTAMP, wire_type::VARINT) => {
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
/// `chat_id_shared` memoizes the conversation id `Arc` so it is allocated only
/// when the first record is actually pushed.
fn push_secret_record(
    chat_id: &str,
    chat_id_shared: &mut Option<Arc<str>>,
    mut history_msg: HistorySyncMsgInternalFields,
    out: &mut Vec<HistoryMsgSecretRecord>,
) {
    // Takes `history_msg` by value: the message is decoded fresh per record and
    // dropped right after, so the owned fields are moved into the record instead
    // of cloned.
    let Some(web_msg) = history_msg.message.as_mut() else {
        return;
    };
    let Some(key) = web_msg.key.as_ref() else {
        return;
    };
    if key.id.is_none() {
        return;
    }
    let from_me = key.from_me == Some(true);

    if let Some(message) = web_msg.message.as_ref()
        && message.is_forwarded()
    {
        return;
    }

    // Read the Copy-flag fields by borrow before moving any owned field out.
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
    let timestamp = web_msg.message_timestamp;

    // Top-level message_secret takes priority over the context-info one (same
    // order as the previous `or_else`); take it rather than clone.
    let secret = if web_msg.message_secret.is_some() {
        web_msg.message_secret.take()
    } else {
        web_msg
            .message
            .as_mut()
            .and_then(|m| m.message_context_info.as_mut())
            .and_then(|mci| mci.message_secret.take())
    };
    let Some(secret) = secret else {
        return;
    };

    let key = web_msg.key.as_mut().expect("key presence checked above");
    let msg_id = key.id.take().expect("id presence checked above");
    let key_participant = key.participant.take();
    let web_msg_participant = web_msg.participant.take();

    out.push(HistoryMsgSecretRecord {
        chat_id: chat_id_shared
            .get_or_insert_with(|| Arc::from(chat_id))
            .clone(),
        from_me,
        key_participant,
        web_msg_participant,
        msg_id: msg_id.into(),
        secret: secret.into(),
        timestamp,
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

    /// Prost-only oracle: what the pre-fast-path pipeline produced for one
    /// raw `HistorySyncMsg`. The fast path must match this exactly.
    fn oracle_records(raw_msg: &[u8]) -> Vec<HistoryMsgSecretRecord> {
        let mut out = Vec::new();
        let mut shared = None;
        if let Ok(msg) = HistorySyncMsgInternalFields::decode(raw_msg) {
            push_secret_record("5511777776666@s.whatsapp.net", &mut shared, msg, &mut out);
        }
        out
    }

    fn emit_varint(out: &mut Vec<u8>, mut v: u64) {
        loop {
            if v < 0x80 {
                out.push(v as u8);
                return;
            }
            out.push((v as u8 & 0x7F) | 0x80);
            v >>= 7;
        }
    }

    /// Emit a length-delimited field with proper varint framing.
    fn emit_len_field(out: &mut Vec<u8>, field: u32, value: &[u8]) {
        emit_varint(out, ((field << 3) | wire_type::LENGTH_DELIMITED) as u64);
        emit_varint(out, value.len() as u64);
        out.extend_from_slice(value);
    }

    fn wrap_in_history_msg(web_msg: &wa::WebMessageInfo) -> Vec<u8> {
        wa::HistorySyncMsg {
            message: Some(web_msg.clone()),
            ..Default::default()
        }
        .encode_to_vec()
    }

    fn secret_ctx(secret: &[u8]) -> wa::MessageContextInfo {
        wa::MessageContextInfo {
            message_secret: Some(secret.to_vec()),
            ..Default::default()
        }
    }

    fn keyed(id: &str, from_me: bool, message: Option<wa::Message>) -> wa::WebMessageInfo {
        wa::WebMessageInfo {
            key: wa::MessageKey {
                from_me: Some(from_me),
                id: Some(id.to_string()),
                ..Default::default()
            },
            message,
            message_timestamp: Some(1_700_000_777),
            ..Default::default()
        }
    }

    fn fp(inner: wa::Message) -> Box<wa::message::FutureProofMessage> {
        Box::new(wa::message::FutureProofMessage {
            message: Some(Box::new(inner)),
        })
    }

    /// Differential corpus: every structurally interesting shape, prost-built
    /// and hand-crafted, must extract identically through the fast path and
    /// the prost oracle.
    #[test]
    fn differential_fast_path_matches_prost_oracle() {
        let emit = emit_len_field;
        let secret = vec![0x5Au8; 32];
        let mut corpus: Vec<(String, Vec<u8>)> = Vec::new();
        let mut add = |name: &str, raw: Vec<u8>| corpus.push((name.to_string(), raw));

        // Proto-built shapes.
        add(
            "plain text, no secret",
            wrap_in_history_msg(&keyed(
                "A1",
                false,
                Some(wa::Message {
                    conversation: Some("oi".into()),
                    ..Default::default()
                }),
            )),
        );
        let mut wm = keyed("A2", true, None);
        wm.message_secret = Some(secret.clone());
        add("top-level secret, no message", wrap_in_history_msg(&wm));
        add(
            "context secret",
            wrap_in_history_msg(&keyed(
                "A3",
                false,
                Some(wa::Message {
                    message_context_info: Some(secret_ctx(&secret)),
                    ..Default::default()
                }),
            )),
        );
        let mut wm = keyed(
            "A4",
            false,
            Some(wa::Message {
                message_context_info: Some(secret_ctx(&[0xBB; 32])),
                ..Default::default()
            }),
        );
        wm.message_secret = Some(secret.clone());
        add("both secrets, top wins", wrap_in_history_msg(&wm));
        for fwd in [true, false] {
            add(
                &format!("ETM forwarded={fwd} with context secret"),
                wrap_in_history_msg(&keyed(
                    "A5",
                    false,
                    Some(wa::Message {
                        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                            text: Some("x".into()),
                            context_info: Some(Box::new(wa::ContextInfo {
                                is_forwarded: Some(fwd),
                                ..Default::default()
                            })),
                            ..Default::default()
                        })),
                        message_context_info: Some(secret_ctx(&secret)),
                        ..Default::default()
                    }),
                )),
            );
        }
        add("ephemeral-wrapped forwarded image, top secret", {
            let mut wm = keyed(
                "A6",
                false,
                Some(wa::Message {
                    ephemeral_message: Some(fp(wa::Message {
                        image_message: Some(Box::new(wa::message::ImageMessage {
                            context_info: Some(Box::new(wa::ContextInfo {
                                is_forwarded: Some(true),
                                ..Default::default()
                            })),
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
            );
            wm.message_secret = Some(secret.clone());
            wrap_in_history_msg(&wm)
        });
        add(
            "viewOnceV2(poll) with context secret",
            wrap_in_history_msg(&keyed(
                "A7",
                false,
                Some(wa::Message {
                    view_once_message_v2: Some(fp(wa::Message {
                        poll_creation_message: Some(Box::new(wa::message::PollCreationMessage {
                            name: Some("poll".into()),
                            ..Default::default()
                        })),
                        ..Default::default()
                    })),
                    message_context_info: Some(secret_ctx(&secret)),
                    ..Default::default()
                }),
            )),
        );
        add("deviceSent(ephemeral(event)) top secret", {
            let mut wm = keyed(
                "A8",
                true,
                Some(wa::Message {
                    device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
                        destination_jid: Some("5511777776666@s.whatsapp.net".into()),
                        message: Some(Box::new(wa::Message {
                            ephemeral_message: Some(fp(wa::Message {
                                event_message: Some(Box::new(wa::message::EventMessage {
                                    name: Some("ev".into()),
                                    ..Default::default()
                                })),
                                ..Default::default()
                            })),
                            ..Default::default()
                        })),
                        phash: None,
                    })),
                    ..Default::default()
                }),
            );
            wm.message_secret = Some(secret.clone());
            wrap_in_history_msg(&wm)
        });
        add(
            "bot metadata outer + context secret",
            wrap_in_history_msg(&keyed(
                "A9",
                false,
                Some(wa::Message {
                    message_context_info: Some(wa::MessageContextInfo {
                        message_secret: Some(secret.clone()),
                        bot_metadata: Some(wa::BotMetadata::default()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            )),
        );
        add("bot metadata under ephemeral wrapper, top secret", {
            let mut wm = keyed(
                "A10",
                false,
                Some(wa::Message {
                    ephemeral_message: Some(fp(wa::Message {
                        message_context_info: Some(wa::MessageContextInfo {
                            bot_metadata: Some(wa::BotMetadata::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    })),
                    ..Default::default()
                }),
            );
            wm.message_secret = Some(secret.clone());
            wrap_in_history_msg(&wm)
        });
        add(
            "edited wrapper containing poll v3, context secret",
            wrap_in_history_msg(&keyed(
                "A11",
                false,
                Some(wa::Message {
                    edited_message: Some(fp(wa::Message {
                        poll_creation_message_v3: Some(Box::new(
                            wa::message::PollCreationMessage::default(),
                        )),
                        ..Default::default()
                    })),
                    message_context_info: Some(secret_ctx(&secret)),
                    ..Default::default()
                }),
            )),
        );
        add("missing key id, top secret", {
            let mut wm = wa::WebMessageInfo {
                key: wa::MessageKey {
                    from_me: Some(false),
                    ..Default::default()
                },
                ..Default::default()
            };
            wm.message_secret = Some(secret.clone());
            wrap_in_history_msg(&wm)
        });
        add("no key at all, top secret", {
            let wm = wa::WebMessageInfo {
                message_secret: Some(secret.clone()),
                ..Default::default()
            };
            wrap_in_history_msg(&wm)
        });
        add("participants on key and web msg", {
            let mut wm = keyed("A12", false, None);
            wm.key.participant = Some("5511888889999@s.whatsapp.net".into());
            wm.participant = Some("5511888887777@s.whatsapp.net".into());
            wm.message_secret = Some(secret.clone());
            wrap_in_history_msg(&wm)
        });
        add("empty top-level secret", {
            let mut wm = keyed("A13", false, None);
            wm.message_secret = Some(Vec::new());
            wrap_in_history_msg(&wm)
        });
        add("oversized secret (heap spill)", {
            let mut wm = keyed("A14", false, None);
            wm.message_secret = Some(vec![0xCC; 80]);
            wrap_in_history_msg(&wm)
        });

        // Hand-crafted wire shapes prost's encoder cannot produce.
        let key_a12 = wa::MessageKey {
            from_me: Some(false),
            id: Some("R1".into()),
            ..Default::default()
        }
        .encode_to_vec();
        let msg_plain = wa::Message {
            conversation: Some("a".into()),
            ..Default::default()
        }
        .encode_to_vec();
        let msg_secret = wa::Message {
            message_context_info: Some(secret_ctx(&secret)),
            ..Default::default()
        }
        .encode_to_vec();

        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE, &msg_plain);
        emit(&mut web, tags::web_message_info::MESSAGE, &msg_secret);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("repeated message field (merge -> fallback)", raw);

        let key_b = wa::MessageKey {
            participant: Some("5511888889999@s.whatsapp.net".into()),
            ..Default::default()
        }
        .encode_to_vec();
        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::KEY, &key_b);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("repeated key occurrences (leaf merge)", raw);

        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        let tag = (tags::web_message_info::MESSAGE_SECRET << 3) | wire_type::VARINT;
        web.push((tag as u8 & 0x7F) | 0x80);
        web.push((tag >> 7) as u8);
        web.push(0x05);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("secret with varint wire type", raw);

        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        web.extend_from_slice(&[0x1A, 0x55]); // truncated length-delimited tail
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("malformed tail after valid secret", raw);

        let bad_key = {
            let mut k = Vec::new();
            emit(&mut k, tags::message_key::ID, &[0xFF, 0xFE]);
            k
        };
        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &bad_key);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("invalid UTF-8 in key id", raw);

        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        let tag = (tags::web_message_info::MESSAGE_TIMESTAMP << 3) | wire_type::LENGTH_DELIMITED;
        web.push(tag as u8);
        web.push(2);
        web.extend_from_slice(&[0x01, 0x02]);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("timestamp with length-delimited wire type", raw);

        // Mirrored message-typed field with a varint wire type: prost fails
        // the decode, so no record.
        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        let mut msg = Vec::new();
        emit_varint(
            &mut msg,
            ((tags::message::POLL_CREATION_MESSAGE << 3) | wire_type::VARINT) as u64,
        );
        msg.push(0x01);
        emit(&mut web, tags::web_message_info::MESSAGE, &msg);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("poll field with varint wire type", raw);

        // Repeated occurrences of the same carrier: prost merges their
        // contextInfo fields; the eager overwrite-when-present walk must agree.
        let etm_fwd = wa::message::ExtendedTextMessage {
            context_info: Some(Box::new(wa::ContextInfo {
                is_forwarded: Some(true),
                ..Default::default()
            })),
            ..Default::default()
        }
        .encode_to_vec();
        let etm_plain = wa::message::ExtendedTextMessage {
            text: Some("x".into()),
            ..Default::default()
        }
        .encode_to_vec();
        for order in [[&etm_fwd, &etm_plain], [&etm_plain, &etm_fwd]] {
            let mut msg = Vec::new();
            for etm in order {
                emit(&mut msg, tags::message::EXTENDED_TEXT_MESSAGE, etm);
            }
            let mci = wa::MessageContextInfo {
                message_secret: Some(secret.clone()),
                ..Default::default()
            }
            .encode_to_vec();
            emit(&mut msg, tags::message::MESSAGE_CONTEXT_INFO, &mci);
            let mut web = Vec::new();
            emit(&mut web, tags::web_message_info::KEY, &key_a12);
            emit(&mut web, tags::web_message_info::MESSAGE, &msg);
            let mut raw = Vec::new();
            emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
            add("repeated carrier occurrences (merge)", raw);
        }

        // Unknown field with proto2 group wire type next to a secret: prost
        // skips the group; the fast path must defer rather than reject.
        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        web.push(((90 << 3) | wire_type::START_GROUP) as u8);
        web.push(((90 << 3) | wire_type::END_GROUP) as u8);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("unknown group field (fallback to prost)", raw);

        // Repeated ephemeral wrapper occurrences: inner messages merge.
        let eph_poll = wa::Message {
            ephemeral_message: Some(fp(wa::Message {
                poll_creation_message: Some(Box::new(wa::message::PollCreationMessage::default())),
                ..Default::default()
            })),
            ..Default::default()
        }
        .encode_to_vec();
        let eph_text = wa::Message {
            ephemeral_message: Some(fp(wa::Message {
                conversation: Some("t".into()),
                ..Default::default()
            })),
            ..Default::default()
        }
        .encode_to_vec();
        let mut merged_msg = eph_poll.clone();
        merged_msg.extend_from_slice(&eph_text);
        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE, &merged_msg);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("repeated ephemeral wrappers (fallback)", raw);

        // prost rejects field number 0, keys above u32 range, and varints
        // whose 10th byte overflows 64 bits; the fast path must produce the
        // same no-record outcome instead of skipping or truncating. Each case
        // carries a valid secret so a lax walk WOULD emit a record.
        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        web.push(wire_type::LENGTH_DELIMITED as u8); // key with field number 0
        web.push(0);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("field number zero", raw);

        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        // Key varint above u32 range whose truncated field number is benign.
        emit_varint(&mut web, (5u64 << 32) | (1000u64 << 3));
        web.push(0x01);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("key varint above u32 range", raw);

        let mut web = Vec::new();
        emit(&mut web, tags::web_message_info::KEY, &key_a12);
        emit(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
        web.push(((tags::web_message_info::MESSAGE_TIMESTAMP << 3) | wire_type::VARINT) as u8);
        web.extend_from_slice(&[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x02]);
        let mut raw = Vec::new();
        emit(&mut raw, tags::history_sync_msg::MESSAGE, &web);
        add("timestamp varint overflowing 64 bits", raw);

        // Nesting beyond the unwrap budget defers to prost's recursion rules
        // (decode fails past prost's limit -> no record; the fast path must
        // agree). Built iteratively from raw bytes: encoding a 120-deep owned
        // proto recurses per level and overflows the test stack.
        for depth in [50usize, 120] {
            let mut msg = wa::Message {
                poll_creation_message: Some(Box::new(wa::message::PollCreationMessage::default())),
                ..Default::default()
            }
            .encode_to_vec();
            for _ in 0..depth {
                let mut fpm = Vec::new();
                emit_len_field(&mut fpm, tags::message::future_proof_message::MESSAGE, &msg);
                let mut outer = Vec::new();
                emit_len_field(&mut outer, tags::message::EPHEMERAL_MESSAGE, &fpm);
                msg = outer;
            }
            let key = wa::MessageKey {
                from_me: Some(false),
                id: Some("DEEP".into()),
                ..Default::default()
            }
            .encode_to_vec();
            let mut web = Vec::new();
            emit_len_field(&mut web, tags::web_message_info::KEY, &key);
            emit_len_field(&mut web, tags::web_message_info::MESSAGE, &msg);
            emit_len_field(&mut web, tags::web_message_info::MESSAGE_SECRET, &secret);
            let mut raw = Vec::new();
            emit_len_field(&mut raw, tags::history_sync_msg::MESSAGE, &web);
            add(&format!("nesting depth {depth}"), raw);
        }

        for (name, raw) in &corpus {
            let fast = run_with_raw_history_msg(raw);
            let oracle = oracle_records(raw);
            assert_eq!(fast, oracle, "divergence in case: {name}");
        }
        assert!(corpus.len() >= 25, "corpus unexpectedly small");
    }

    /// Wrap raw `HistorySyncMsg` bytes in Conversation/HistorySync framing and
    /// run the full pipeline, returning the extracted records. Lets tests feed
    /// hand-crafted wire bytes that prost's encoder cannot produce (repeated
    /// field occurrences, wrong wire types, malformed tails).
    fn run_with_raw_history_msg(raw_msg: &[u8]) -> Vec<HistoryMsgSecretRecord> {
        let chat = "5511777776666@s.whatsapp.net";
        let mut conv = Vec::new();
        emit_len_field(&mut conv, tags::conversation::ID, chat.as_bytes());
        emit_len_field(&mut conv, tags::conversation::MESSAGES, raw_msg);
        let mut hs = Vec::new();
        emit_len_field(&mut hs, tags::history_sync::CONVERSATIONS, &conv);

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&hs).unwrap();
        let compressed = encoder.finish().unwrap();
        process_history_sync(compressed, None, false, None)
            .unwrap()
            .msg_secret_records
    }

    /// A (malformed) conversation that carries messages BEFORE its id must not
    /// emit records under an empty chat id.
    #[test]
    fn test_messages_before_conversation_id_yield_no_records() {
        let emit = |out: &mut Vec<u8>, field: u32, v: &[u8]| {
            out.push(((field << 3) | wire_type::LENGTH_DELIMITED) as u8);
            out.push(v.len() as u8);
            out.extend_from_slice(v);
        };

        let web_msg = wa::WebMessageInfo {
            key: wa::MessageKey {
                from_me: Some(false),
                id: Some("EARLY_MSG".into()),
                ..Default::default()
            },
            message_secret: Some(vec![0x22u8; 32]),
            ..Default::default()
        }
        .encode_to_vec();
        let mut history_msg = Vec::new();
        emit(&mut history_msg, tags::history_sync_msg::MESSAGE, &web_msg);

        // messages (field 2) deliberately emitted before id (field 1).
        let mut conv = Vec::new();
        emit(&mut conv, tags::conversation::MESSAGES, &history_msg);
        emit(
            &mut conv,
            tags::conversation::ID,
            b"5511777776666@s.whatsapp.net",
        );
        let mut hs = Vec::new();
        emit(&mut hs, tags::history_sync::CONVERSATIONS, &conv);

        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&hs).unwrap();
        let compressed = encoder.finish().unwrap();
        let result = process_history_sync(compressed, None, false, None).unwrap();
        assert!(result.msg_secret_records.is_empty());
    }

    /// The presence pre-scan must honor prost merge semantics: a secret carried
    /// by a LATER occurrence of a repeated message field still yields a record.
    #[test]
    fn test_secret_in_second_message_field_occurrence() {
        let msg_without_secret = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        }
        .encode_to_vec();
        let msg_with_secret = wa::Message {
            message_context_info: Some(wa::MessageContextInfo {
                message_secret: Some(vec![0x11u8; 32]),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let key = wa::MessageKey {
            from_me: Some(false),
            id: Some("DOUBLE_MSG".into()),
            ..Default::default()
        }
        .encode_to_vec();

        let mut web_msg = Vec::new();
        let emit = |out: &mut Vec<u8>, field: u32, v: &[u8]| {
            out.push(((field << 3) | wire_type::LENGTH_DELIMITED) as u8);
            out.push(v.len() as u8);
            out.extend_from_slice(v);
        };
        emit(&mut web_msg, tags::web_message_info::KEY, &key);
        emit(
            &mut web_msg,
            tags::web_message_info::MESSAGE,
            &msg_without_secret,
        );
        emit(
            &mut web_msg,
            tags::web_message_info::MESSAGE,
            &msg_with_secret,
        );

        let mut history_msg = Vec::new();
        emit(&mut history_msg, tags::history_sync_msg::MESSAGE, &web_msg);

        let records = run_with_raw_history_msg(&history_msg);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].msg_id, "DOUBLE_MSG");
        assert_eq!(records[0].secret.as_slice(), [0x11u8; 32]);
    }

    /// A secret field with the wrong wire type must not yield a record (prost
    /// fails the decode), and must not panic the pre-scan.
    #[test]
    fn test_wrong_wire_type_secret_yields_no_record() {
        let key = wa::MessageKey {
            from_me: Some(false),
            id: Some("BAD_WIRE".into()),
            ..Default::default()
        }
        .encode_to_vec();

        let mut web_msg = Vec::new();
        web_msg.push(((tags::web_message_info::KEY << 3) | wire_type::LENGTH_DELIMITED) as u8);
        web_msg.push(key.len() as u8);
        web_msg.extend_from_slice(&key);
        // message_secret (tag 49) as VARINT instead of bytes; tag 49 needs a
        // 2-byte tag varint (49 << 3 = 392).
        let tag = (tags::web_message_info::MESSAGE_SECRET << 3) | wire_type::VARINT;
        web_msg.push((tag as u8 & 0x7F) | 0x80);
        web_msg.push((tag >> 7) as u8);
        web_msg.push(0x05);

        let mut history_msg = Vec::new();
        history_msg
            .push(((tags::history_sync_msg::MESSAGE << 3) | wire_type::LENGTH_DELIMITED) as u8);
        history_msg.push(web_msg.len() as u8);
        history_msg.extend_from_slice(&web_msg);

        assert!(run_with_raw_history_msg(&history_msg).is_empty());
    }

    /// Presence of an EMPTY top-level secret still produces a record (the
    /// consumer filters by length), pinning presence-not-content semantics.
    #[test]
    fn test_empty_secret_still_yields_record() {
        let chat = "5511777776666@s.whatsapp.net";
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: Some(wa::WebMessageInfo {
                        key: wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some("EMPTY_SECRET".to_string()),
                            ..Default::default()
                        },
                        message_secret: Some(Vec::new()),
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
        assert_eq!(result.msg_secret_records.len(), 1);
        assert!(result.msg_secret_records[0].secret.is_empty());
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
        assert_eq!(&*result.msg_secret_records[0].chat_id, chat);
        assert_eq!(result.msg_secret_records[0].msg_id, "HIST_TOP_LEVEL");
        assert_eq!(
            result.msg_secret_records[0].key_participant.as_deref(),
            Some(participant)
        );
        assert_eq!(
            result.msg_secret_records[0].secret.as_slice(),
            top_level_secret
        );
        assert_eq!(result.msg_secret_records[1].msg_id, "HIST_CONTEXT");
        assert!(result.msg_secret_records[1].from_me);
        assert_eq!(
            result.msg_secret_records[1].secret.as_slice(),
            context_secret
        );
    }

    #[test]
    fn test_top_level_message_secret_takes_priority_over_context() {
        // A message carrying BOTH the top-level WebMessageInfo.message_secret and a
        // nested message_context_info.message_secret must extract the top-level one
        // (the move-based push_secret_record must `.take()` the right source).
        let chat = "5511777776666@s.whatsapp.net";
        let top_level_secret = vec![0xAAu8; 32];
        let context_secret = vec![0xBBu8; 32];
        let hs = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: Some(wa::WebMessageInfo {
                        key: wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some("HIST_BOTH".to_string()),
                            participant: Some("5511888889999@s.whatsapp.net".to_string()),
                        },
                        message_secret: Some(top_level_secret.clone()),
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
                }],
                ..Default::default()
            }],
            ..Default::default()
        };

        let compressed = encode_and_compress(&hs);
        let result = process_history_sync(compressed, None, false, None).unwrap();

        assert_eq!(result.msg_secret_records.len(), 1);
        assert_eq!(result.msg_secret_records[0].msg_id, "HIST_BOTH");
        assert_eq!(
            result.msg_secret_records[0].secret.as_slice(),
            top_level_secret,
            "top-level message_secret must win over the context-info one"
        );
        assert_eq!(
            result.msg_secret_records[0].key_participant.as_deref(),
            Some("5511888889999@s.whatsapp.net")
        );
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
