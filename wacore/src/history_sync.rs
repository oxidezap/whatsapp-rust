use bytes::Bytes;
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

/// True when a `HistorySyncMsg` carries a message secret, either the top-level
/// `WebMessageInfo.message_secret` (tag 49) or the nested
/// `Message.message_context_info.message_secret` (tags 2 -> 35 -> 3).
///
/// Pure presence walk so the expensive prost decode of the 30+-field message
/// struct only runs for messages that can actually yield a record. Mirrors
/// prost merge semantics: repeated occurrences of a message field merge, and a
/// later occurrence can never unset a secret, so any occurrence counts. On
/// malformed bytes it returns false, matching the decode-failure skip.
fn has_message_secret(history_msg: &[u8]) -> bool {
    scan_fields(history_msg, |field, wt, value| {
        field == tags::history_sync_msg::MESSAGE
            && wt == wire_type::LENGTH_DELIMITED
            && web_msg_has_secret(value)
    })
}

fn web_msg_has_secret(web_msg: &[u8]) -> bool {
    scan_fields(web_msg, |field, wt, value| {
        wt == wire_type::LENGTH_DELIMITED
            && match field {
                tags::web_message_info::MESSAGE_SECRET => true,
                tags::web_message_info::MESSAGE => message_has_context_secret(value),
                _ => false,
            }
    })
}

fn message_has_context_secret(message: &[u8]) -> bool {
    scan_fields(message, |field, wt, value| {
        field == tags::message::MESSAGE_CONTEXT_INFO
            && wt == wire_type::LENGTH_DELIMITED
            && scan_fields(value, |f, w, _| {
                f == tags::message_context_info::MESSAGE_SECRET && w == wire_type::LENGTH_DELIMITED
            })
    })
}

/// Walk one protobuf level, calling `hit` for each field with its value slice
/// (empty for non-length-delimited fields). Returns true on the first hit;
/// false at the end of the buffer or on the first malformed field.
#[inline]
fn scan_fields(data: &[u8], mut hit: impl FnMut(u32, u32, &[u8]) -> bool) -> bool {
    let mut pos = 0;
    while pos < data.len() {
        let Ok((tag, br)) = read_varint(&data[pos..]) else {
            return false;
        };
        pos += br;
        let field = (tag >> 3) as u32;
        let wt = (tag & 0x7) as u32;
        if wt == wire_type::LENGTH_DELIMITED {
            let Ok((len, vl)) = read_varint(&data[pos..]) else {
                return false;
            };
            pos += vl;
            let Ok(end) = checked_end(pos, len, data.len(), "scan") else {
                return false;
            };
            if hit(field, wt, &data[pos..end]) {
                return true;
            }
            pos = end;
        } else {
            if hit(field, wt, &[]) {
                return true;
            }
            match skip_field(wt, data, pos) {
                Ok(np) => pos = np,
                Err(_) => return false,
            }
        }
    }
    false
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
                // Decode only messages that can yield a record: the presence
                // walk is a fraction of the full prost decode, and most real
                // history messages carry no secret. The id guard keeps records
                // from a (malformed) blob that re-orders messages before the
                // conversation id from landing under an empty chat id.
                if !chat_id.is_empty()
                    && has_message_secret(&data[pos..end])
                    && let Ok(msg) = HistorySyncMsgInternalFields::decode(&data[pos..end])
                {
                    push_secret_record(chat_id, &mut chat_id_shared, msg, secrets_out);
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
        msg_id,
        secret,
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

    /// Wrap raw `HistorySyncMsg` bytes in Conversation/HistorySync framing and
    /// run the full pipeline, returning the extracted records. Lets tests feed
    /// hand-crafted wire bytes that prost's encoder cannot produce (repeated
    /// field occurrences, wrong wire types, malformed tails).
    fn run_with_raw_history_msg(raw_msg: &[u8]) -> Vec<HistoryMsgSecretRecord> {
        fn emit_len_field(out: &mut Vec<u8>, field: u32, value: &[u8]) {
            let mut tag_buf = [0u8; 5];
            let mut tag = (field << 3) | wire_type::LENGTH_DELIMITED;
            let mut i = 0;
            loop {
                if tag < 0x80 {
                    tag_buf[i] = tag as u8;
                    i += 1;
                    break;
                }
                tag_buf[i] = (tag as u8 & 0x7F) | 0x80;
                tag >>= 7;
                i += 1;
            }
            out.extend_from_slice(&tag_buf[..i]);
            assert!(value.len() < 0x80, "test helper supports short fields only");
            out.push(value.len() as u8);
            out.extend_from_slice(value);
        }

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
        assert_eq!(records[0].secret, vec![0x11u8; 32]);
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
        assert_eq!(result.msg_secret_records[0].secret, top_level_secret);
        assert_eq!(result.msg_secret_records[1].msg_id, "HIST_CONTEXT");
        assert!(result.msg_secret_records[1].from_me);
        assert_eq!(result.msg_secret_records[1].secret, context_secret);
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
            result.msg_secret_records[0].secret, top_level_secret,
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
