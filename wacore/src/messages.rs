use crate::libsignal::crypto::CryptographicHash;
use anyhow::{Result, anyhow};
use base64::Engine as _;
use buffa::MessageView;
// Encode/decode of proto trees is routed through `waproto::codec` so the tree is
// instantiated once in waproto; tests still call the trait methods directly.
#[cfg(test)]
use buffa::Message as _;
use waproto::whatsapp as wa;

pub struct MessageUtils;

impl MessageUtils {
    fn random_pad_len() -> u8 {
        use rand::RngExt;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        // Uniform 1..=16, matching WA Web / whatsmeow (rand%16 + 1). The prior
        // `& 0x0F` with a 0->15 remap skewed toward 15 and never produced 16.
        (rng.random::<u8>() & 0x0F) + 1
    }

    pub fn pad_message_v2(mut plaintext: Vec<u8>) -> Vec<u8> {
        let pad = Self::random_pad_len();
        plaintext.resize(plaintext.len() + pad as usize, pad);
        plaintext
    }

    /// Encode + pad in a single pre-sized allocation.
    ///
    /// Runs ONE `compute_size` pass over the message tree and reuses its
    /// `SizeCache` for the write. The previous `encoded_len()` + `encode()`
    /// ran `compute_size` twice (once to size the buffer, once inside `encode`)
    /// over the whole tree on this per-recipient send hot path.
    pub fn encode_and_pad(msg: &wa::Message) -> Vec<u8> {
        let pad = Self::random_pad_len();
        let mut cache = buffa::SizeCache::new();
        let size = waproto::codec::message_compute_size(msg, &mut cache);
        let mut buf = Vec::with_capacity(size + pad as usize);
        waproto::codec::message_write_to(msg, &mut cache, &mut buf);
        buf.resize(buf.len() + pad as usize, pad);
        buf
    }

    /// Encode + pad with an extra `message_context_info` spliced on, in a single
    /// pre-sized allocation. `extra_context` carries the reporting-token fields
    /// (message_secret + reporting_token_version) the send path used to inject by
    /// deep-cloning the whole message via `prepare_message_with_context`.
    ///
    /// The extra context is appended as a second `message_context_info` field after the
    /// message's own fields. When the message already carries one, the wire decoder
    /// merges the two occurrences (later set fields win), reproducing
    /// `prepare_message_with_context` (existing fields preserved, message_secret +
    /// reporting_token_version overwritten) without the clone. `extra_context = None`
    /// is exactly `encode_and_pad`. Locked by `group_encode_with_context_*` tests.
    pub fn encode_and_pad_with_context(
        msg: &wa::Message,
        extra_context: Option<&wa::MessageContextInfo>,
    ) -> Vec<u8> {
        let pad = Self::random_pad_len();
        // Size the extra mci once; the same cache feeds the write below.
        let mut c_cache = buffa::SizeCache::new();
        let extra_inner = extra_context
            .map(|c| waproto::codec::message_context_info_compute_size(c, &mut c_cache));
        let extra_len = extra_inner.map_or(0, |sz| len_delimited_len(TAG_MESSAGE_CONTEXT_INFO, sz));
        let mut msg_cache = buffa::SizeCache::new();
        let msg_size = waproto::codec::message_compute_size(msg, &mut msg_cache);
        let mut buf = Vec::with_capacity(msg_size + extra_len + pad as usize);
        waproto::codec::message_write_to(msg, &mut msg_cache, &mut buf);
        if let (Some(c), Some(sz)) = (extra_context, extra_inner) {
            push_message_field_sized(TAG_MESSAGE_CONTEXT_INFO, c, sz, &mut c_cache, &mut buf);
        }
        buf.resize(buf.len() + pad as usize, pad);
        buf
    }

    /// Same wire output as [`encode_and_pad_with_context`](Self::encode_and_pad_with_context)
    /// but the message is already encoded into `content`. The DM send path shares one
    /// encode between the reporting token (field extraction) and the wire plaintext, so
    /// the shared bytes are spliced + padded here instead of re-encoding the message.
    /// `content` must be the message's encoding with no reporting context spliced on (the
    /// caller only takes this path when the message has no top-level message_context_info).
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.send.dm_plaintext", level = "debug", skip_all)
    )]
    pub fn pad_with_context_from_encoded(
        content: &[u8],
        extra_context: Option<&wa::MessageContextInfo>,
    ) -> Vec<u8> {
        let pad = Self::random_pad_len();
        let mut c_cache = buffa::SizeCache::new();
        let extra_inner = extra_context
            .map(|c| waproto::codec::message_context_info_compute_size(c, &mut c_cache));
        let extra_len = extra_inner.map_or(0, |sz| len_delimited_len(TAG_MESSAGE_CONTEXT_INFO, sz));
        let mut buf = Vec::with_capacity(content.len() + extra_len + pad as usize);
        buf.extend_from_slice(content);
        if let (Some(c), Some(sz)) = (extra_context, extra_inner) {
            push_message_field_sized(TAG_MESSAGE_CONTEXT_INFO, c, sz, &mut c_cache, &mut buf);
        }
        buf.resize(buf.len() + pad as usize, pad);
        buf
    }

    /// Build both DM plaintexts (recipient and own-device DeviceSentMessage) from a
    /// single encode of the shared message content, splicing in the reporting-token
    /// `extra_context` (message_secret + reporting_token_version) without cloning the
    /// message.
    ///
    /// The recipient plaintext and the DSM inner carry the same message, so the previous
    /// path encoded it twice (once for the recipient, once inside `wrap_device_sent` +
    /// `encode_and_pad`) and boxed a whole `Message` per send, on top of the deep clone
    /// `prepare_message_with_context` made to attach the reporting secret. Here the
    /// content is encoded once from `&message` and spliced into both: the recipient
    /// appends `extra_context` out of tag order (protobuf decoders accept any field
    /// order), and the DSM frames the content as `device_sent_message.message` with the
    /// `extra_context` hoisted onto the outer message. Equivalent, after decode, to
    /// `prepare_message_with_context(message, secret)` then `encode_and_pad(..)` /
    /// `encode_and_pad(wrap_device_sent(..))` (locked by the `splice_*` differential
    /// tests).
    ///
    /// The hot send path has no top-level `message_context_info` on `message`, so the
    /// common branch below borrows it and never clones. The rare case (the message
    /// already carries one, e.g. a forwarded message or a poll with a caller-set secret)
    /// hoisting onto the DSM wrapper needs an owned message, so it clones once and folds
    /// `extra_context` in before splicing.
    pub fn encode_dm_plaintexts(
        message: &wa::Message,
        extra_context: Option<&wa::MessageContextInfo>,
        destination_jid: &str,
    ) -> DmPlaintexts {
        if message.message_context_info.is_set() {
            let mut owned = message.clone();
            if let Some(extra) = extra_context {
                // Fold the reporting context into the existing mci via the same merge the
                // wire decoder performs (later set fields win), matching
                // prepare_message_with_context without enumerating its fields here.
                let ctx = owned
                    .message_context_info
                    .as_option_mut()
                    .expect("mci is set");
                waproto::codec::message_context_info_merge(
                    ctx,
                    &waproto::codec::message_context_info_to_vec(extra),
                )
                .expect("merge MessageContextInfo");
            }
            return Self::encode_dm_plaintexts_owned(owned, destination_jid);
        }

        // Common path: `message` has no top-level message_context_info, so its encoding
        // is the shared content and `extra_context` is the only mci to splice on.
        // Padding is uniform 1..=16, so reserving 16 lets both buffers hold their
        // worst-case pad without reallocating.
        const MAX_PAD: usize = 16;

        let mci_field_len = extra_context.map_or(0, |m| {
            let mut c = buffa::SizeCache::new();
            len_delimited_len(
                TAG_MESSAGE_CONTEXT_INFO,
                waproto::codec::message_context_info_compute_size(m, &mut c),
            )
        });
        let mut msg_cache = buffa::SizeCache::new();
        let content_len = waproto::codec::message_compute_size(message, &mut msg_cache);
        let dest = destination_jid.as_bytes();

        // recipient = content (encoded once) + the extra message_context_info field.
        // Pre-size for content + the appended mci field + padding so it never
        // reallocates; the content bytes are then spliced into the own-device buffer.
        let mut recipient = Vec::with_capacity(content_len + mci_field_len + MAX_PAD);
        waproto::codec::message_write_to(message, &mut msg_cache, &mut recipient);

        // own-device plaintext = Message { device_sent_message { destination_jid,
        // message }, [message_context_info] }. The DeviceSentMessage length is
        // pre-computed so the spliced content goes straight in, and the buffer is sized
        // exactly (device_sent_message field + mci field + padding): one allocation, no
        // reallocation regardless of whether extra_context is present.
        let dsm_len = len_delimited_len(TAG_DSM_DESTINATION_JID, dest.len())
            + len_delimited_len(TAG_DSM_MESSAGE, content_len);
        let own_cap = len_delimited_len(TAG_DEVICE_SENT_MESSAGE, dsm_len) + mci_field_len + MAX_PAD;
        let mut own_devices = Vec::with_capacity(own_cap);
        push_wire_tag(
            TAG_DEVICE_SENT_MESSAGE,
            buffa::encoding::WireType::LengthDelimited,
            &mut own_devices,
        );
        push_varint(dsm_len as u64, &mut own_devices); // DeviceSentMessage length
        push_len_delimited(TAG_DSM_DESTINATION_JID, dest, &mut own_devices);
        push_len_delimited(TAG_DSM_MESSAGE, &recipient[..content_len], &mut own_devices);
        if let Some(extra) = extra_context {
            push_message_field(TAG_MESSAGE_CONTEXT_INFO, extra, &mut own_devices);
        }

        // Finish the recipient now that its content has been spliced into own_devices.
        if let Some(extra) = extra_context {
            push_message_field(TAG_MESSAGE_CONTEXT_INFO, extra, &mut recipient);
        }

        DmPlaintexts {
            recipient: Self::pad_message_v2(recipient),
            own_devices: Self::pad_message_v2(own_devices),
        }
    }

    /// Same wire output as [`encode_dm_plaintexts`](Self::encode_dm_plaintexts)'s common
    /// (no top-level message_context_info) path, but the shared content is already encoded
    /// into `content`. The DM send path reuses the single message encode it also feeds to
    /// the reporting token, so the message is no longer encoded twice per send. `content`
    /// must be the message's encoding with no reporting context spliced on.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.send.dm_plaintexts", level = "debug", skip_all)
    )]
    pub fn dm_plaintexts_from_encoded(
        content: &[u8],
        extra_context: Option<&wa::MessageContextInfo>,
        destination_jid: &str,
    ) -> DmPlaintexts {
        const MAX_PAD: usize = 16;

        let mci_field_len = extra_context.map_or(0, |m| {
            let mut c = buffa::SizeCache::new();
            len_delimited_len(
                TAG_MESSAGE_CONTEXT_INFO,
                waproto::codec::message_context_info_compute_size(m, &mut c),
            )
        });
        let content_len = content.len();
        let dest = destination_jid.as_bytes();

        let mut recipient = Vec::with_capacity(content_len + mci_field_len + MAX_PAD);
        recipient.extend_from_slice(content);

        let dsm_len = len_delimited_len(TAG_DSM_DESTINATION_JID, dest.len())
            + len_delimited_len(TAG_DSM_MESSAGE, content_len);
        let own_cap = len_delimited_len(TAG_DEVICE_SENT_MESSAGE, dsm_len) + mci_field_len + MAX_PAD;
        let mut own_devices = Vec::with_capacity(own_cap);
        push_wire_tag(
            TAG_DEVICE_SENT_MESSAGE,
            buffa::encoding::WireType::LengthDelimited,
            &mut own_devices,
        );
        push_varint(dsm_len as u64, &mut own_devices); // DeviceSentMessage length
        push_len_delimited(TAG_DSM_DESTINATION_JID, dest, &mut own_devices);
        push_len_delimited(TAG_DSM_MESSAGE, content, &mut own_devices);
        if let Some(extra) = extra_context {
            push_message_field(TAG_MESSAGE_CONTEXT_INFO, extra, &mut own_devices);
        }

        if let Some(extra) = extra_context {
            push_message_field(TAG_MESSAGE_CONTEXT_INFO, extra, &mut recipient);
        }

        DmPlaintexts {
            recipient: Self::pad_message_v2(recipient),
            own_devices: Self::pad_message_v2(own_devices),
        }
    }

    /// Owned-message DM splice for the rare case where `message` already carries a
    /// top-level `message_context_info` that must be hoisted onto the DSM wrapper
    /// (`wrap_device_sent` semantics). Hoisting requires ownership; the common path in
    /// [`encode_dm_plaintexts`] borrows instead and never reaches this.
    fn encode_dm_plaintexts_owned(mut message: wa::Message, destination_jid: &str) -> DmPlaintexts {
        const MAX_PAD: usize = 16;

        // Hoist message_context_info onto the wrapper (as wrap_device_sent does) so the
        // remaining content is identical for both plaintexts and encoded once. Keep the
        // mci struct (not a temp Vec): it is small and encoded straight into each buffer.
        let mci = message.message_context_info.take();
        let mci_field_len = mci.as_ref().map_or(0, |m| {
            let mut c = buffa::SizeCache::new();
            len_delimited_len(
                TAG_MESSAGE_CONTEXT_INFO,
                waproto::codec::message_context_info_compute_size(m, &mut c),
            )
        });
        let mut msg_cache = buffa::SizeCache::new();
        let content_len = waproto::codec::message_compute_size(&message, &mut msg_cache);
        let dest = destination_jid.as_bytes();

        let mut recipient = Vec::with_capacity(content_len + mci_field_len + MAX_PAD);
        waproto::codec::message_write_to(&message, &mut msg_cache, &mut recipient);

        let dsm_len = len_delimited_len(TAG_DSM_DESTINATION_JID, dest.len())
            + len_delimited_len(TAG_DSM_MESSAGE, content_len);
        let own_cap = len_delimited_len(TAG_DEVICE_SENT_MESSAGE, dsm_len) + mci_field_len + MAX_PAD;
        let mut own_devices = Vec::with_capacity(own_cap);
        push_wire_tag(
            TAG_DEVICE_SENT_MESSAGE,
            buffa::encoding::WireType::LengthDelimited,
            &mut own_devices,
        );
        push_varint(dsm_len as u64, &mut own_devices); // DeviceSentMessage length
        push_len_delimited(TAG_DSM_DESTINATION_JID, dest, &mut own_devices);
        push_len_delimited(TAG_DSM_MESSAGE, &recipient[..content_len], &mut own_devices);
        if let Some(mci) = &mci {
            push_message_field(TAG_MESSAGE_CONTEXT_INFO, mci, &mut own_devices);
        }

        if let Some(mci) = &mci {
            push_message_field(TAG_MESSAGE_CONTEXT_INFO, mci, &mut recipient);
        }

        DmPlaintexts {
            recipient: Self::pad_message_v2(recipient),
            own_devices: Self::pad_message_v2(own_devices),
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.send.participant_hash", level = "debug", skip_all)
    )]
    pub fn participant_list_hash<'a>(
        devices: impl IntoIterator<Item = &'a wacore_binary::Jid>,
    ) -> Result<String> {
        // Format every device into one shared arena and sort range views over
        // it: two allocations total instead of a heap String per device (this
        // runs over the full device set on every group send). Sorting the
        // slices is the same lexicographic order as sorting the individual
        // ad_strings, so the hashed concatenation is byte-identical.
        let devices = devices.into_iter();
        let mut ranges: Vec<(usize, usize)> = Vec::with_capacity(devices.size_hint().0);
        let mut arena = String::with_capacity(ranges.capacity() * 36);
        for jid in devices {
            let start = arena.len();
            jid.push_ad_to(&mut arena);
            ranges.push((start, arena.len()));
        }
        ranges.sort_unstable_by(|a, b| arena[a.0..a.1].cmp(&arena[b.0..b.1]));

        let mut h = CryptographicHash::new("SHA-256")
            .map_err(|e| anyhow!("failed to initialize SHA-256 hasher: {:?}", e))?;
        for &(start, end) in &ranges {
            h.update(&arena.as_bytes()[start..end]);
        }

        let full_hash = h
            .finalize_sha256_array()
            .map_err(|e| anyhow!("failed to finalize hash: {:?}", e))?;

        // Standard base64 ('+'/'/'), matching whatsmeow (`base64.RawStdEncoding`)
        // and WA Web (`WABase64.encodeB64`). URL-safe ('-'/'_') diverges from the
        // server on ~22% of phashes (any output hitting base64 index 62/63).
        let mut out = String::with_capacity(10);
        out.push_str("2:");
        base64::prelude::BASE64_STANDARD_NO_PAD.encode_string(&full_hash[..6], &mut out);
        Ok(out)
    }

    /// Validate a broadcast-contact-list hash from an incoming `deviceSentMessage`
    /// against the message's `<participants>` set. Mirrors WA Web `validateBclHash`:
    /// the sender (our own other device) hashes the broadcast recipients with
    /// phashV2; here we recompute via [`participant_list_hash`](Self::participant_list_hash)
    /// and return `true` only when the computed hash equals `expected` (including
    /// for an empty participant list, which hashes to its own deterministic value
    /// — not a trivial pass).
    pub fn validate_bcl_hash(participants: &[wacore_binary::Jid], expected: &str) -> bool {
        Self::participant_list_hash(participants).is_ok_and(|computed| computed == expected)
    }

    pub fn unpadded_message_len(plaintext: &[u8], version: u8) -> Result<usize> {
        if version == 3 {
            return Ok(plaintext.len());
        }
        if plaintext.is_empty() {
            return Err(anyhow::anyhow!("plaintext is empty, cannot unpad"));
        }
        let pad_len = plaintext[plaintext.len() - 1] as usize;
        if pad_len == 0 || pad_len > plaintext.len() {
            return Err(anyhow::anyhow!("invalid padding length: {}", pad_len));
        }
        let (data, padding) = plaintext.split_at(plaintext.len() - pad_len);
        for &byte in padding {
            if byte != pad_len as u8 {
                return Err(anyhow::anyhow!("invalid padding bytes"));
            }
        }
        Ok(data.len())
    }

    pub fn unpad_message_ref(plaintext: &[u8], version: u8) -> Result<&[u8]> {
        let unpadded_len = Self::unpadded_message_len(plaintext, version)?;
        Ok(&plaintext[..unpadded_len])
    }
}

/// Decode padded ciphertext into a `wa::Message`.
///
/// Unpads the plaintext (using the given padding version) and decodes the
/// protobuf bytes into a WhatsApp Message. This is the pure,
/// runtime-independent portion of `handle_decrypted_plaintext`.
pub fn decode_plaintext(padded_plaintext: &[u8], padding_version: u8) -> Result<wa::Message> {
    let plaintext_slice = MessageUtils::unpad_message_ref(padded_plaintext, padding_version)?;
    // Route through the pinned codec entry point so the Message decode tree
    // (BotMetadata/ProtocolMessage/ContextInfo merge_field, etc.) is
    // instantiated once in waproto instead of copied into every calling crate.
    waproto::codec::message_decode(plaintext_slice)
        .map_err(|e| anyhow::anyhow!("Failed to decode decrypted plaintext: {e}"))
}

/// History-sync metadata detached from a decoded plaintext while the large
/// inline blob keeps sharing the decrypt buffer. The generated owned protobuf
/// type remains the single source for protocol fields; only its byte payload is
/// represented separately so async processing does not need a megabyte copy.
#[derive(Debug)]
pub struct DetachedHistorySyncNotification {
    pub notification: wa::message::HistorySyncNotification,
    pub inline_payload: Option<buffa::bytes::Bytes>,
}

impl From<wa::message::HistorySyncNotification> for DetachedHistorySyncNotification {
    fn from(mut notification: wa::message::HistorySyncNotification) -> Self {
        let inline_payload = notification
            .initial_hist_bootstrap_inline_payload
            .take()
            .map(buffa::bytes::Bytes::from);
        Self {
            notification,
            inline_payload,
        }
    }
}

/// Decode an owned plaintext while detaching an inline history-sync payload as
/// a zero-copy `Bytes` slice.
///
/// Only the generated schema tags needed to reach the inline byte field are
/// inspected. The field is removed from a lazily rewritten wire buffer before
/// the regular generated decoder runs, so the large payload is never copied
/// into the owned protobuf tree. Every other field is still decoded by Buffa's
/// generated implementation, keeping protobuf merge and unknown-field
/// semantics in one place.
pub fn decode_plaintext_detached_history_sync(
    padded_plaintext: Vec<u8>,
    padding_version: u8,
) -> Result<(wa::Message, Option<DetachedHistorySyncNotification>)> {
    let unpadded_len = MessageUtils::unpadded_message_len(&padded_plaintext, padding_version)?;
    let source = buffa::bytes::Bytes::from(padded_plaintext).slice(0..unpadded_len);

    // Mirror `unwrap_device_sent`: once a DSM carries an inner message, only
    // that message is dispatched. Inspecting just this generated-tag path
    // avoids decoding/materializing the complete MessageView graph.
    let history_path = if contains_nested_message_field(&source, DEVICE_SENT_INNER_MESSAGE_PATH)? {
        DEVICE_SENT_HISTORY_PAYLOAD_PATH
    } else {
        DIRECT_HISTORY_PAYLOAD_PATH
    };

    #[cfg(feature = "tracing")]
    let decode_span = tracing::trace_span!(
        "wa.message.detach_history_sync_payload",
        plaintext_bytes = source.len() as u64
    );
    #[cfg(feature = "tracing")]
    let decode_guard = decode_span.enter();
    let redaction = redact_nested_bytes_field(&source, 0, history_path)?;
    #[cfg(feature = "tracing")]
    drop(decode_guard);

    #[cfg(feature = "tracing")]
    let materialize_span = tracing::trace_span!(
        "wa.message.decode_plaintext",
        plaintext_bytes = source.len() as u64,
        payload_detached = redaction.detached_value.is_some()
    );
    #[cfg(feature = "tracing")]
    let _materialize_guard = materialize_span.enter();
    let encoded = redaction.rewritten.as_deref().unwrap_or(&source);
    let mut message = waproto::codec::message_decode(encoded)
        .map_err(|e| anyhow::anyhow!("Failed to decode decrypted plaintext: {e}"))?;

    let mut history_sync =
        take_history_sync_notification(&mut message).map(DetachedHistorySyncNotification::from);
    if let Some(payload_range) = redaction.detached_value {
        let detached = history_sync
            .as_mut()
            .ok_or_else(|| anyhow!("inline history-sync payload had no decoded notification"))?;
        detached.inline_payload = Some(source.slice(payload_range));
    }

    Ok((message, history_sync))
}

fn take_history_sync_notification(
    message: &mut wa::Message,
) -> Option<wa::message::HistorySyncNotification> {
    // Mirror `unwrap_device_sent`: when a valid DSM wrapper exists, only its
    // inner message is dispatched. Otherwise the top-level message is used.
    let message = match message.device_sent_message.as_option_mut() {
        Some(device_sent) if device_sent.message.is_set() => device_sent.message.as_option_mut()?,
        _ => message,
    };
    message
        .protocol_message
        .as_option_mut()?
        .history_sync_notification
        .take()
}

#[derive(Default)]
struct WireRedaction {
    /// Present only when at least one target field was removed. Rewriting is
    /// lazy so plaintexts without inline history never clone their wire bytes.
    rewritten: Option<Vec<u8>>,
    /// Byte range in the original plaintext. For a repeated singular bytes
    /// field, protobuf merge semantics select the final occurrence.
    detached_value: Option<std::ops::Range<usize>>,
}

/// Whether a nested message field path is set on the wire.
///
/// This deliberately recognizes only length-delimited fields, the schema wire
/// type for every segment in the supplied generated-tag paths. A mismatched
/// known field is left for the regular decoder to reject with its canonical
/// error after routing has been selected.
fn contains_nested_message_field(message: &[u8], path: &[u32]) -> Result<bool, buffa::DecodeError> {
    let Some((&field_number, remaining_path)) = path.split_first() else {
        return Ok(false);
    };

    let mut remaining = message;
    while !remaining.is_empty() {
        let mut after_tag = remaining;
        let tag = buffa::encoding::Tag::decode(&mut after_tag)?;
        if tag.field_number() == field_number
            && tag.wire_type() == buffa::encoding::WireType::LengthDelimited
        {
            let value_range = length_delimited_value_range(message.len(), after_tag)?;
            if remaining_path.is_empty()
                || contains_nested_message_field(&message[value_range.clone()], remaining_path)?
            {
                return Ok(true);
            }
            remaining = &message[value_range.end..];
        } else {
            buffa::encoding::skip_field_depth(tag, &mut after_tag, buffa::RECURSION_LIMIT)?;
            remaining = after_tag;
        }
    }
    Ok(false)
}

/// Remove every occurrence of the bytes field at `path`, rebuilding only the
/// containing length-delimited fields whose sizes changed. Unrelated wire
/// fields are copied byte-for-byte, while their eventual interpretation stays
/// with the generated decoder.
fn redact_nested_bytes_field(
    message: &[u8],
    absolute_offset: usize,
    path: &[u32],
) -> Result<WireRedaction, buffa::DecodeError> {
    let Some((&field_number, remaining_path)) = path.split_first() else {
        return Ok(WireRedaction::default());
    };

    let mut result = WireRedaction::default();
    let mut copied_until = 0;
    let mut remaining = message;

    while !remaining.is_empty() {
        let field_start = message.len() - remaining.len();
        let mut after_tag = remaining;
        let tag = buffa::encoding::Tag::decode(&mut after_tag)?;

        if tag.field_number() == field_number
            && tag.wire_type() == buffa::encoding::WireType::LengthDelimited
        {
            let tag_end = message.len() - after_tag.len();
            let value_range = length_delimited_value_range(message.len(), after_tag)?;

            if remaining_path.is_empty() {
                let rewritten = result.rewritten.get_or_insert_with(Vec::new);
                rewritten.extend_from_slice(&message[copied_until..field_start]);
                copied_until = value_range.end;
                result.detached_value =
                    Some(absolute_offset + value_range.start..absolute_offset + value_range.end);
            } else {
                let child = redact_nested_bytes_field(
                    &message[value_range.clone()],
                    absolute_offset + value_range.start,
                    remaining_path,
                )?;
                if let Some(child_range) = child.detached_value {
                    result.detached_value = Some(child_range);
                }
                if let Some(child_bytes) = child.rewritten {
                    let rewritten = result.rewritten.get_or_insert_with(Vec::new);
                    rewritten.extend_from_slice(&message[copied_until..field_start]);
                    // Preserve the original tag bytes. Only the containing
                    // length changes, so that varint is intentionally rebuilt.
                    rewritten.extend_from_slice(&message[field_start..tag_end]);
                    push_varint(child_bytes.len() as u64, rewritten);
                    rewritten.extend_from_slice(&child_bytes);
                    copied_until = value_range.end;
                }
            }

            remaining = &message[value_range.end..];
        } else {
            buffa::encoding::skip_field_depth(tag, &mut after_tag, buffa::RECURSION_LIMIT)?;
            remaining = after_tag;
        }
    }

    if let Some(rewritten) = result.rewritten.as_mut() {
        rewritten.extend_from_slice(&message[copied_until..]);
    }
    Ok(result)
}

fn length_delimited_value_range(
    message_len: usize,
    mut after_tag: &[u8],
) -> Result<std::ops::Range<usize>, buffa::DecodeError> {
    let value_len = buffa::encoding::decode_varint(&mut after_tag)?;
    let value_len = usize::try_from(value_len).map_err(|_| buffa::DecodeError::MessageTooLarge)?;
    let value_start = message_len - after_tag.len();
    let value_end = value_start
        .checked_add(value_len)
        .ok_or(buffa::DecodeError::MessageTooLarge)?;
    if value_end > message_len {
        return Err(buffa::DecodeError::UnexpectedEof);
    }
    Ok(value_start..value_end)
}

/// Use when borrowed fields are enough and a full owned message is avoidable.
pub fn decode_plaintext_view(
    padded_plaintext: &[u8],
    padding_version: u8,
) -> Result<wa::MessageView<'_>> {
    let plaintext_slice = MessageUtils::unpad_message_ref(padded_plaintext, padding_version)?;
    wa::MessageView::decode_view(plaintext_slice)
        .map_err(|e| anyhow::anyhow!("Failed to decode decrypted plaintext: {e}"))
}

/// Decode a plaintext buffer into a self-contained Buffa view.
pub fn decode_plaintext_owned_view(
    padded_plaintext: Vec<u8>,
    padding_version: u8,
) -> Result<wa::MessageOwnedView> {
    let unpadded_len = MessageUtils::unpadded_message_len(&padded_plaintext, padding_version)?;
    let plaintext = buffa::bytes::Bytes::from(padded_plaintext).slice(0..unpadded_len);
    wa::MessageOwnedView::decode(plaintext)
        .map_err(|e| anyhow::anyhow!("Failed to decode decrypted plaintext: {e}"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SenderKeyDistributionOnlyPlaintext<'a> {
    pub axolotl_sender_key_distribution_message: Option<&'a [u8]>,
}

/// Conservative fast path for SKDM-only plaintexts before owned dispatch decode.
pub fn sender_key_distribution_only_plaintext(
    padded_plaintext: &[u8],
    padding_version: u8,
) -> Result<Option<SenderKeyDistributionOnlyPlaintext<'_>>> {
    let plaintext_slice = MessageUtils::unpad_message_ref(padded_plaintext, padding_version)?;
    if !has_only_sender_key_distribution_top_level_fields(plaintext_slice)? {
        return Ok(None);
    }

    let view = wa::MessageView::decode_view(plaintext_slice)
        .map_err(|e| anyhow::anyhow!("Failed to decode decrypted plaintext: {e}"))?;
    let axolotl_sender_key_distribution_message = view
        .sender_key_distribution_message
        .as_option()
        .and_then(|skdm| skdm.axolotl_sender_key_distribution_message);

    Ok(Some(SenderKeyDistributionOnlyPlaintext {
        axolotl_sender_key_distribution_message,
    }))
}

pub fn has_only_sender_key_distribution_top_level_fields(
    encoded: &[u8],
) -> Result<bool, buffa::DecodeError> {
    // Generated tag constants, so a proto renumber updates the classifier
    // instead of silently misrouting SKDM-only messages.
    use waproto::tags::message as m;
    let mut cur = encoded;
    let mut has_sender_key_distribution = false;
    while !cur.is_empty() {
        let tag = buffa::encoding::Tag::decode(&mut cur)?;
        match tag.field_number() {
            m::SENDER_KEY_DISTRIBUTION_MESSAGE
            | m::FAST_RATCHET_KEY_SENDER_KEY_DISTRIBUTION_MESSAGE => {
                has_sender_key_distribution = true
            }
            m::MESSAGE_CONTEXT_INFO => {}
            _ => return Ok(false),
        }
        buffa::encoding::skip_field_depth(tag, &mut cur, buffa::RECURSION_LIMIT)?;
    }
    Ok(has_sender_key_distribution)
}

/// The two padded plaintexts a DM send needs, built from a single encode of the
/// shared message content. See [`MessageUtils::encode_dm_plaintexts`].
pub struct DmPlaintexts {
    /// Plaintext encrypted to the recipient's devices (the message itself).
    pub recipient: Vec<u8>,
    /// Plaintext encrypted to our own other devices (the message wrapped in a
    /// DeviceSentMessage), equivalent to `encode_and_pad(wrap_device_sent(..))`.
    pub own_devices: Vec<u8>,
}

// Protobuf field numbers spliced by `encode_dm_plaintexts`, sourced from the
// generated schema tags so a .proto renumber breaks here at compile time
// instead of silently changing the wire payload.
const TAG_DEVICE_SENT_MESSAGE: u32 = waproto::tags::message::DEVICE_SENT_MESSAGE;
const TAG_MESSAGE_CONTEXT_INFO: u32 = waproto::tags::message::MESSAGE_CONTEXT_INFO;
const TAG_DSM_DESTINATION_JID: u32 = waproto::tags::message::device_sent_message::DESTINATION_JID;
const TAG_DSM_MESSAGE: u32 = waproto::tags::message::device_sent_message::MESSAGE;

const DEVICE_SENT_INNER_MESSAGE_PATH: &[u32] = &[TAG_DEVICE_SENT_MESSAGE, TAG_DSM_MESSAGE];
const DIRECT_HISTORY_PAYLOAD_PATH: &[u32] = &[
    waproto::tags::message::PROTOCOL_MESSAGE,
    waproto::tags::message::protocol_message::HISTORY_SYNC_NOTIFICATION,
    waproto::tags::message::history_sync_notification::INITIAL_HIST_BOOTSTRAP_INLINE_PAYLOAD,
];
const DEVICE_SENT_HISTORY_PAYLOAD_PATH: &[u32] = &[
    TAG_DEVICE_SENT_MESSAGE,
    TAG_DSM_MESSAGE,
    waproto::tags::message::PROTOCOL_MESSAGE,
    waproto::tags::message::protocol_message::HISTORY_SYNC_NOTIFICATION,
    waproto::tags::message::history_sync_notification::INITIAL_HIST_BOOTSTRAP_INLINE_PAYLOAD,
];

const PROTOBUF_WIRE_TYPE_BITS: u32 = 3;

/// Append a base-128 varint (protobuf wire format).
#[inline]
fn push_varint(mut v: u64, out: &mut Vec<u8>) {
    while v >= 0x80 {
        out.push((v as u8) | 0x80);
        v >>= 7;
    }
    out.push(v as u8);
}

#[inline]
fn wire_tag_value(field: u32, wire_type: buffa::encoding::WireType) -> u64 {
    (u64::from(field) << PROTOBUF_WIRE_TYPE_BITS) | wire_type as u64
}

#[inline]
fn push_wire_tag(field: u32, wire_type: buffa::encoding::WireType, out: &mut Vec<u8>) {
    push_varint(wire_tag_value(field, wire_type), out);
}

/// Append a length-delimited protobuf field (wire type 2): a string field, or a
/// nested message field carrying already-encoded `bytes`. The latter is the splice
/// point that lets the shared content be reused without re-encoding it.
#[inline]
fn push_len_delimited(field: u32, bytes: &[u8], out: &mut Vec<u8>) {
    push_wire_tag(field, buffa::encoding::WireType::LengthDelimited, out);
    push_varint(bytes.len() as u64, out);
    out.extend_from_slice(bytes);
}

/// Bytes a base-128 varint occupies.
#[inline]
fn varint_len(mut v: u64) -> usize {
    let mut n = 1;
    while v >= 0x80 {
        v >>= 7;
        n += 1;
    }
    n
}

/// Encoded size of the length-delimited field `field` carrying `payload_len`
/// bytes (key + length varint + payload). Mirrors what `push_len_delimited`
/// writes, so a nested field's length can be pre-computed without a temp buffer.
#[inline]
fn len_delimited_len(field: u32, payload_len: usize) -> usize {
    varint_len(wire_tag_value(
        field,
        buffa::encoding::WireType::LengthDelimited,
    )) + varint_len(payload_len as u64)
        + payload_len
}

/// Append a message_context_info as a nested length-delimited field, encoding it
/// straight into `out` (no intermediate `Vec`). Used for the small
/// `message_context_info` field on both plaintexts.
#[inline]
fn push_message_field(field: u32, msg: &wa::MessageContextInfo, out: &mut Vec<u8>) {
    let mut cache = buffa::SizeCache::new();
    let size = waproto::codec::message_context_info_compute_size(msg, &mut cache);
    push_message_field_sized(field, msg, size, &mut cache, out);
}

/// Same as [`push_message_field`] but reuses a `SizeCache` the caller already
/// filled by `message_context_info_compute_size` (e.g. for a buffer capacity
/// estimate). `cache` must hold exactly that message's sizes with the cursor at
/// 0; `write_to` consumes them, so this avoids measuring the sub-tree twice.
#[inline]
fn push_message_field_sized(
    field: u32,
    msg: &wa::MessageContextInfo,
    size: usize,
    cache: &mut buffa::SizeCache,
    out: &mut Vec<u8>,
) {
    push_wire_tag(field, buffa::encoding::WireType::LengthDelimited, out);
    push_varint(size as u64, out);
    waproto::codec::message_context_info_write_to(msg, cache, out);
}

/// Wrap a message into a DeviceSentMessage for own-device sync, hoisting
/// `message_context_info` onto the outer message (matching WA Web). Inverse of
/// [`unwrap_device_sent`].
pub fn wrap_device_sent(mut message: wa::Message, destination_jid: String) -> wa::Message {
    let context = std::mem::take(&mut message.message_context_info);
    wa::Message {
        message_context_info: context,
        device_sent_message: wa::message::DeviceSentMessage {
            destination_jid: Some(destination_jid),
            message: message.into(),
            ..Default::default()
        }
        .into(),
        ..Default::default()
    }
}

/// Unwrap a DeviceSentMessage wrapper, returning the inner message.
///
/// When a message is sent from our own device, the actual content is nested
/// inside `device_sent_message.message`.  This function extracts that inner
/// message (preserving `message_context_info`), or returns the original
/// message unchanged when there is no wrapper or the wrapper has no inner
/// message.
pub fn unwrap_device_sent(mut msg: wa::Message) -> wa::Message {
    if let Some(mut dsm) = msg.device_sent_message.take() {
        if let Some(mut inner) = dsm.message.take() {
            inner.message_context_info = crate::proto_helpers::merge_dsm_context(
                inner.message_context_info.take(),
                msg.message_context_info.as_option(),
            )
            .map(buffa::MessageField::some)
            .unwrap_or_default();
            return inner;
        }
        msg.device_sent_message = buffa::MessageField::some(dsm);
    }
    msg
}

/// Returns `true` if the message contains only a SenderKey distribution
/// (internal key-exchange for group encryption) and no user-visible content.
///
/// When sending a group message, WhatsApp includes the SKDM in a separate
/// `pkmsg` enc node.  We must process it (store the sender key) but should
/// not surface it as a user event.
pub fn is_sender_key_distribution_only(msg: &mut wa::Message) -> bool {
    if msg.sender_key_distribution_message.is_unset()
        && msg
            .fast_ratchet_key_sender_key_distribution_message
            .is_unset()
    {
        return false;
    }

    // Fast path: most common user-visible fields (avoids the slow path for the typical case).
    if msg.conversation.is_some()
        || msg.extended_text_message.is_set()
        || msg.image_message.is_set()
        || msg.video_message.is_set()
        || msg.audio_message.is_set()
        || msg.document_message.is_set()
        || msg.reaction_message.is_set()
        || msg.protocol_message.is_set()
        || msg.sticker_message.is_set()
        || msg.contact_message.is_set()
        || msg.location_message.is_set()
        || msg.live_location_message.is_set()
    {
        return false;
    }

    // Slow path: temporarily take out the carrier fields and compare the rest to
    // default to catch all current and future fields, then restore them. This
    // avoids deep-cloning the whole Message just to clear three fields.
    let skdm = msg.sender_key_distribution_message.take();
    let fast = msg.fast_ratchet_key_sender_key_distribution_message.take();
    let ctx = msg.message_context_info.take();

    // proto fields only encode when non-default, so encoded length 0 means all
    // remaining fields are at default — i.e. the message has no user content.
    let mut cache = buffa::SizeCache::new();
    let only = waproto::codec::message_compute_size(msg, &mut cache) == 0;

    msg.sender_key_distribution_message = skdm.map(buffa::MessageField::some).unwrap_or_default();
    msg.fast_ratchet_key_sender_key_distribution_message =
        fast.map(buffa::MessageField::some).unwrap_or_default();
    msg.message_context_info = ctx.map(buffa::MessageField::some).unwrap_or_default();

    only
}

/// Parse a message stanza into a `MessageInfo` struct.
///
/// This is a pure function that extracts message metadata from a node's
/// attributes. It requires the own JID and optional LID to determine
/// `is_from_me`.
pub fn parse_message_info(
    node: &wacore_binary::NodeRef<'_>,
    own_jid: &wacore_binary::Jid,
    own_lid: Option<&wacore_binary::Jid>,
) -> Result<crate::types::message::MessageInfo> {
    use crate::types::message::{
        AddressingMode, EditAttribute, MessageCategory, MessageInfo, MessageSource,
    };
    use wacore_binary::{JidExt as _, STATUS_BROADCAST_USER, Server};

    let mut attrs = node.attrs();
    let id = attrs.required_string("id")?;
    anyhow::ensure!(
        !id.is_empty(),
        "message stanza has an empty required 'id' attribute"
    );
    let id = id.into_owned();
    let from = attrs.required_jid("from")?;
    let addressing_mode = attrs
        .optional_string("addressing_mode")
        .and_then(|s| AddressingMode::try_from(s.as_ref()).ok());

    let mut source = if from.server == Server::Broadcast {
        let participant = attrs.required_jid("participant")?;
        let is_from_me = participant.matches_user_or_lid(own_jid, own_lid);

        // Match WAWebMsgParser: read participant_lid/_pn unconditionally so
        // the LID-PN cache can re-warm from the stanza.
        let sender_alt = if participant.server.is_pn_family() {
            attrs.optional_jid("participant_lid")
        } else if participant.server.is_lid_family() {
            attrs.optional_jid("participant_pn")
        } else {
            None
        };

        MessageSource {
            chat: from.clone(),
            sender: participant.clone(),
            is_from_me,
            is_group: true,
            broadcast_list_owner: if from.user != STATUS_BROADCAST_USER {
                Some(participant.clone())
            } else {
                None
            },
            sender_alt,
            ..Default::default()
        }
    } else if from.is_group() {
        let sender = attrs.required_jid("participant")?;
        let sender_alt = match addressing_mode {
            Some(AddressingMode::Lid) => attrs.optional_jid("participant_pn"),
            Some(AddressingMode::Pn) => attrs.optional_jid("participant_lid"),
            None => None,
        };

        let is_from_me = sender.matches_user_or_lid(own_jid, own_lid);

        MessageSource {
            chat: from.clone(),
            sender: sender.clone(),
            is_from_me,
            is_group: true,
            sender_alt,
            ..Default::default()
        }
    } else if from.matches_user_or_lid(own_jid, own_lid) {
        let recipient = attrs.optional_jid_result("recipient")?;
        let chat = recipient
            .as_ref()
            .map(|r| r.to_non_ad())
            .unwrap_or_else(|| from.to_non_ad());
        // Populate sender_alt so LID-PN cache warms from self-messages
        let sender_alt = if from.server == Server::Lid {
            Some(own_jid.clone())
        } else if from.server == Server::Pn && own_lid.is_some() {
            own_lid.cloned()
        } else {
            None
        };
        MessageSource {
            chat,
            sender: from.clone(),
            is_from_me: true,
            recipient,
            sender_alt,
            ..Default::default()
        }
    } else {
        let sender_alt = if from.server == Server::Lid {
            attrs.optional_jid("sender_pn")
        } else {
            attrs.optional_jid("sender_lid")
        };

        MessageSource {
            chat: from.to_non_ad(),
            sender: from.clone(),
            is_from_me: false,
            sender_alt,
            ..Default::default()
        }
    };

    source.addressing_mode = addressing_mode;

    // Broadcast/status only: collect <participants><to jid> so the receive path
    // can validate a deviceSentMessage.phash (WA Web validateBclHash). Group
    // <participants> carry the device fanout, not a bcl, so they're skipped.
    let bcl_participants: Vec<wacore_binary::Jid> = if from.server == Server::Broadcast {
        node.get_optional_child("participants")
            .map(|p| {
                p.get_children_by_tag("to")
                    .filter_map(|to| to.attrs().optional_jid("jid"))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let category = attrs
        .optional_string("category")
        .map(|s| MessageCategory::from(s.as_ref()))
        .unwrap_or_default();

    let server_id = attrs
        .optional_u64("server_id")
        .filter(|&v| (99..=2_147_476_647).contains(&v))
        .unwrap_or(0) as i32;

    if source.chat.is_newsletter() {
        source.chat.device = 0;
        source.chat.agent = 0;
    }

    let is_offline = attrs.optional_string("offline").is_some();

    // Envelope enrichment (mirrors WAWebHandleMsgParser y() function).
    let server_timestamp_us = attrs
        .optional_u64("sts")
        .and_then(|v| i64::try_from(v).ok());
    let verified_level = attrs
        .optional_string("verified_level")
        .map(|s| s.into_owned());
    let verified_name_serial = attrs
        .optional_u64("verified_name")
        .and_then(|v| i64::try_from(v).ok());
    // The display name only exists inside the <verified_name> child's cert protobuf.
    let verified_name = node
        .get_optional_child("verified_name")
        .and_then(|vn| crate::stanza::business::VerifiedName::try_from_node(vn).ok())
        .map(Box::new);
    let peer_recipient_pn = attrs.optional_jid("peer_recipient_pn");

    // <meta> child attrs (WAWebHandleMsgParser b()) and <reporting> children
    // (I() function). Both are optional; absence is the common case.
    let mut meta_info = crate::types::message::MsgMetaInfo::default();
    if let Some(meta) = node.get_optional_child("meta") {
        let mut ma = meta.attrs();
        meta_info.content_type = ma.optional_string("content_type").map(|s| s.into_owned());
        meta_info.appdata = ma.optional_string("appdata").map(|s| s.into_owned());
        // msmsg addon path needs the trio (target_id, target_sender_jid,
        // target_chat_jid) to look up the parent messageSecret.
        meta_info.target_id = ma.optional_string("target_id").map(|s| s.into_owned());
        meta_info.target_sender = ma.optional_jid("target_sender_jid");
        meta_info.target_chat = ma.optional_jid("target_chat_jid");
    }
    if let Some(reporting) = node.get_optional_child("reporting")
        && let Some(tag) = reporting.get_optional_child("reporting_tag")
    {
        meta_info.reporting_tag = tag.content_bytes().map(|b| b.to_vec());
    }
    if let Some(reporting) = node.get_optional_child("reporting")
        && let Some(token) = reporting.get_optional_child("reporting_token")
    {
        meta_info.reporting_token = token.content_bytes().map(|b| b.to_vec());
        // WA Web `I()`: `c.maybeAttrInt("v")!=null?_:1`. Missing `v` is
        // not a parse failure — token format version defaults to 1.
        meta_info.reporting_token_version = Some(
            token
                .attrs()
                .optional_u64("v")
                .and_then(|v| i64::try_from(v).ok())
                .unwrap_or(1),
        );
    }

    // <bot edit="..."> child. Mirror WA Web `f()`: read `edit_target_id`
    // unconditionally so the msmsg regular-bot fallback path can consume it
    // regardless of edit_type. fbid (`h()`) only uses it for INNER/LAST,
    // but parsing it always is a strict superset.
    let bot_info = node.get_optional_child("bot").map(|bot_node| {
        let mut ba = bot_node.attrs();
        crate::types::message::MsgBotInfo {
            edit_type: ba
                .optional_string("edit")
                .and_then(|s| crate::types::message::BotEditType::from_wire(s.as_ref())),
            edit_target_id: ba.optional_string("edit_target_id").map(|s| s.into_owned()),
            edit_sender_timestamp_ms: ba
                .optional_u64("sender_timestamp_ms")
                .and_then(|ms| i64::try_from(ms).ok())
                .and_then(crate::time::from_millis),
        }
    });

    Ok(MessageInfo {
        source,
        id,
        server_id,
        push_name: attrs
            .optional_string("notify")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        timestamp: crate::time::from_secs_or_now(attrs.unix_time("t")),
        category,
        edit: attrs
            .optional_string("edit")
            .map(|s| EditAttribute::from(s.to_string()))
            .unwrap_or_default(),
        is_offline,
        server_timestamp_us,
        verified_level,
        verified_name,
        verified_name_serial,
        peer_recipient_pn,
        meta_info,
        bot_info,
        bcl_participants,
        ..Default::default()
    })
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod plaintext_view_tests {
    use super::*;

    fn padded(msg: &wa::Message) -> Vec<u8> {
        MessageUtils::pad_message_v2(msg.encode_to_vec())
    }

    fn skdm(bytes: &[u8]) -> wa::message::SenderKeyDistributionMessage {
        wa::message::SenderKeyDistributionMessage {
            group_id: Some("120000000000000000@g.us".to_string()),
            axolotl_sender_key_distribution_message: Some(bytes.to_vec()),
        }
    }

    fn history_notification(payload: Vec<u8>) -> wa::message::HistorySyncNotification {
        wa::message::HistorySyncNotification {
            file_length: Some(payload.len() as u64),
            sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
            initial_hist_bootstrap_inline_payload: Some(payload),
            progress: Some(73),
            ..Default::default()
        }
    }

    fn message_with_history(payload: Vec<u8>, text: &str) -> wa::Message {
        wa::Message {
            conversation: Some(text.to_owned()),
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                history_sync_notification: buffa::MessageField::some(history_notification(payload)),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn decode_plaintext_view_borrows_message_fields() {
        let msg = wa::Message {
            conversation: Some("hello".to_string()),
            ..Default::default()
        };
        let padded = padded(&msg);

        let view = decode_plaintext_view(&padded, 2).expect("view decode should succeed");

        assert_eq!(view.conversation, Some("hello"));
    }

    #[test]
    fn decode_plaintext_owned_view_keeps_unpadded_bytes() {
        let msg = wa::Message {
            conversation: Some("hello".to_string()),
            ..Default::default()
        };
        let padded = padded(&msg);
        let padded_len = padded.len();

        let view =
            decode_plaintext_owned_view(padded, 2).expect("owned view decode should succeed");

        assert_eq!(view.conversation(), Some("hello"));
        assert!(view.bytes().len() < padded_len);
    }

    #[test]
    fn detached_history_payload_shares_plaintext_and_preserves_message() {
        let inline_payload = vec![0xA5; 1024];
        let padded = padded(&message_with_history(inline_payload.clone(), "preserved"));
        let plaintext_start = padded.as_ptr() as usize;
        let plaintext_end = plaintext_start + padded.len();

        let (decoded, detached) = decode_plaintext_detached_history_sync(padded, 2)
            .expect("owned view decode should succeed");
        let detached = detached.expect("history notification should be detached");
        let payload = detached
            .inline_payload
            .expect("inline history payload should be detached");

        assert_eq!(decoded.conversation.as_deref(), Some("preserved"));
        assert!(
            decoded
                .protocol_message
                .as_option()
                .is_some_and(|protocol| !protocol.history_sync_notification.is_set()),
            "only the detached history field should be cleared"
        );
        assert_eq!(detached.notification.file_length, Some(1024));
        assert_eq!(detached.notification.progress, Some(73));
        assert_eq!(payload.as_ref(), inline_payload);
        assert!(
            (plaintext_start..plaintext_end).contains(&(payload.as_ptr() as usize)),
            "the detached payload must remain a slice of the original decrypt buffer"
        );
    }

    #[test]
    fn detached_history_follows_device_sent_unwrap_semantics() {
        let inner_payload = vec![0x11; 64];
        let inner = message_with_history(inner_payload.clone(), "inner");
        let mut wrapped = wrap_device_sent(inner, "1@s.whatsapp.net".into());
        wrapped.protocol_message = buffa::MessageField::some(wa::message::ProtocolMessage {
            history_sync_notification: buffa::MessageField::some(history_notification(vec![
                0x22;
                32
            ])),
            ..Default::default()
        });

        let (decoded, detached) = decode_plaintext_detached_history_sync(padded(&wrapped), 2)
            .expect("device-sent message should decode");
        let decoded = unwrap_device_sent(decoded);
        let detached = detached.expect("inner history notification should be detached");

        assert_eq!(decoded.conversation.as_deref(), Some("inner"));
        assert!(
            decoded
                .protocol_message
                .as_option()
                .is_some_and(|protocol| !protocol.history_sync_notification.is_set())
        );
        assert_eq!(detached.inline_payload.as_deref(), Some(&inner_payload[..]));
    }

    #[test]
    fn sender_key_distribution_only_plaintext_returns_borrowed_axolotl() {
        let msg = wa::Message {
            sender_key_distribution_message: buffa::MessageField::some(skdm(&[1, 2, 3])),
            ..Default::default()
        };
        let padded = padded(&msg);

        let found = sender_key_distribution_only_plaintext(&padded, 2)
            .expect("view decode should succeed")
            .expect("SKDM-only plaintext should be detected");

        assert_eq!(
            found.axolotl_sender_key_distribution_message,
            Some(&[1, 2, 3][..])
        );
    }

    #[test]
    fn sender_key_distribution_only_plaintext_rejects_user_content() {
        let msg = wa::Message {
            conversation: Some("hello".to_string()),
            sender_key_distribution_message: buffa::MessageField::some(skdm(&[1, 2, 3])),
            ..Default::default()
        };
        let padded = padded(&msg);

        let found =
            sender_key_distribution_only_plaintext(&padded, 2).expect("view scan should succeed");

        assert!(found.is_none());
    }

    #[test]
    fn sender_key_distribution_only_plaintext_allows_fast_ratchet_only() {
        let msg = wa::Message {
            fast_ratchet_key_sender_key_distribution_message: buffa::MessageField::some(skdm(&[
                4, 5, 6,
            ])),
            ..Default::default()
        };
        let padded = padded(&msg);

        let found = sender_key_distribution_only_plaintext(&padded, 2)
            .expect("view decode should succeed")
            .expect("fast-ratchet SKDM-only plaintext should be detected");

        assert_eq!(found.axolotl_sender_key_distribution_message, None);
    }
}

#[cfg(test)]
mod parse_message_info_tests {
    use super::*;
    use std::str::FromStr;
    use wacore_binary::Jid;
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn invalid_routing_and_identity_attributes_are_rejected() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let cases = [
            NodeBuilder::new("message")
                .attr("from", "559980000001@s.whatsapp.net")
                .attr("id", "")
                .build(),
            NodeBuilder::new("message")
                .attr("from", "not-a-jid")
                .attr("id", "INVALID-FROM")
                .build(),
            NodeBuilder::new("message")
                .attr("from", "120363021033254949@g.us")
                .attr("id", "MISSING-PARTICIPANT")
                .build(),
            NodeBuilder::new("message")
                .attr("from", "120363021033254949@g.us")
                .attr("participant", "not-a-jid")
                .attr("id", "INVALID-PARTICIPANT")
                .build(),
            NodeBuilder::new("message")
                .attr("from", "559900000000:4@s.whatsapp.net")
                .attr("recipient", "not-a-jid")
                .attr("id", "INVALID-SELF-RECIPIENT")
                .build(),
        ];

        for node in &cases {
            assert!(
                parse_message_info(&node.as_node_ref(), &own_pn, None).is_err(),
                "invalid identity or routing attributes must be rejected: {node:?}"
            );
        }
    }

    #[test]
    fn status_broadcast_with_participant_lid_populates_sender_alt() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let own_lid = Jid::from_str("100000000000000@lid").unwrap();
        let pn_user = "559980000001";
        let lid_user = "100000012345678";
        let node = NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("type", "media")
            .attr("id", "TEST_MSG_ID")
            .attr("t", "1777415965")
            .attr("participant", format!("{pn_user}@s.whatsapp.net").as_str())
            .attr("participant_lid", format!("{lid_user}@lid").as_str())
            .build();

        let info = parse_message_info(&node.as_node_ref(), &own_pn, Some(&own_lid))
            .expect("parse_message_info should succeed for status broadcast");

        assert_eq!(info.source.sender.user, pn_user);
        assert_eq!(info.source.sender.server, wacore_binary::Server::Pn);
        let alt = info
            .source
            .sender_alt
            .as_ref()
            .expect("status broadcast must expose participant_lid as sender_alt");
        assert_eq!(alt.user, lid_user);
        assert_eq!(alt.server, wacore_binary::Server::Lid);
    }

    /// Envelope-enrichment attributes (`sts`, `verified_level`,
    /// `verified_name`, `peer_recipient_pn`) flow into `MessageInfo` fields.
    /// Mirrors `WAWebHandleMsgParser` y() function which threads
    /// `serverStoreTimeMicros`/`verifiedLevel`/`verifiedNameSerial`/
    /// `peerRecipientPn` into the msgInfo result.
    #[test]
    fn envelope_enrichment_fields_are_captured() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-ENV-1")
            .attr("t", "1777415965")
            .attr("sts", "1777415965123456")
            .attr("verified_level", "unknown")
            .attr("verified_name", "12345")
            .attr("peer_recipient_pn", "559980000099@s.whatsapp.net")
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();

        assert_eq!(info.server_timestamp_us, Some(1777415965123456));
        assert_eq!(info.verified_level.as_deref(), Some("unknown"));
        assert_eq!(info.verified_name_serial, Some(12345));
        assert_eq!(
            info.peer_recipient_pn.as_ref().map(|j| j.user.as_str()),
            Some("559980000099")
        );
    }

    /// The `<verified_name>` child cert decodes into the business display
    /// name (WAWebHandleMsgParser reads the same child into
    /// `verifiedNameCert`; the name lives in the cert's details protobuf).
    #[test]
    #[allow(clippy::disallowed_methods)]
    fn envelope_verified_name_cert_is_decoded() {
        use buffa::Message;
        let details = wa::verified_name_certificate::Details {
            verified_name: Some("Fictitious Biz Ltd".into()),
            issuer: Some("smb:wa".into()),
            serial: Some(12345),
            ..Default::default()
        };
        let cert = wa::VerifiedNameCertificate {
            details: Some(details.encode_to_vec()),
            ..Default::default()
        };
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-VN-1")
            .attr("t", "1777415965")
            .attr("verified_name", "12345")
            .children([NodeBuilder::new("verified_name")
                .attr("v", "2")
                .bytes(cert.encode_to_vec())
                .build()])
            .build();

        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();

        let vn = info.verified_name.expect("cert must reach MessageInfo");
        assert_eq!(vn.name.as_deref(), Some("Fictitious Biz Ltd"));
        assert_eq!(vn.serial.as_deref(), Some("12345"));
        assert_eq!(vn.issuer.as_deref(), Some("smb:wa"));
        assert_eq!(info.verified_name_serial, Some(12345));
    }

    /// Undecodable cert bytes must not fail message parsing; the node is
    /// still surfaced, just without a decoded name.
    #[test]
    fn envelope_verified_name_bad_cert_does_not_fail_parse() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-VN-2")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("verified_name")
                .bytes(vec![0xff, 0x00, 0x13, 0x37])
                .build()])
            .build();

        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();

        let vn = info.verified_name.expect("node presence is surfaced");
        assert!(vn.name.is_none());
    }

    /// Envelope without any of the optional enrichment attrs leaves all
    /// four fields as `None`. Regression guard against accidentally
    /// defaulting them.
    #[test]
    fn envelope_enrichment_is_optional() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-ENV-NONE")
            .attr("t", "1777415965")
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();

        assert!(info.server_timestamp_us.is_none());
        assert!(info.verified_level.is_none());
        assert!(info.verified_name.is_none());
        assert!(info.verified_name_serial.is_none());
        assert!(info.peer_recipient_pn.is_none());
    }

    /// `<meta content_type="add_on"/>` (reactions/edits) and
    /// `<meta appdata="default"/>` are captured on `MsgMetaInfo`.
    /// Real shape observed in production for reactions.
    #[test]
    fn meta_child_attrs_are_captured() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "reaction")
            .attr("id", "MSG-REACT-1")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("meta")
                .attr("content_type", "add_on")
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(info.meta_info.content_type.as_deref(), Some("add_on"));
        assert!(info.meta_info.appdata.is_none());
    }

    /// `<reporting><reporting_tag>{bytes}</reporting_tag>
    /// <reporting_token v="2">{bytes}</reporting_token></reporting>` shape
    /// from production. Tag may be 16 or 20 bytes; token usually 16.
    #[test]
    fn reporting_token_and_tag_are_captured() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let tag_bytes: Vec<u8> = (0..16).collect();
        let token_bytes: Vec<u8> = (16..32).collect();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-REP-1")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("reporting")
                .children([
                    NodeBuilder::new("reporting_tag")
                        .bytes(tag_bytes.clone())
                        .build(),
                    NodeBuilder::new("reporting_token")
                        .attr("v", "2")
                        .bytes(token_bytes.clone())
                        .build(),
                ])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(
            info.meta_info.reporting_tag.as_deref(),
            Some(tag_bytes.as_slice())
        );
        assert_eq!(
            info.meta_info.reporting_token.as_deref(),
            Some(token_bytes.as_slice())
        );
        assert_eq!(info.meta_info.reporting_token_version, Some(2));
    }

    /// Missing `v` attr on `<reporting_token>` defaults the version to 1
    /// (matches WA Web `I()`: `c.maybeAttrInt("v") != null ? _ : 1`).
    #[test]
    fn reporting_token_missing_version_defaults_to_one() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-REP-V")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("reporting")
                .children([NodeBuilder::new("reporting_token")
                    .bytes(vec![0xAA; 16])
                    .build()])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(info.meta_info.reporting_token_version, Some(1));
    }

    /// `<reporting>` with ONLY `<reporting_tag>` (no token) is also valid
    /// in production; token fields stay `None`.
    #[test]
    fn reporting_tag_only_leaves_token_none() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-REP-2")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("reporting")
                .children([NodeBuilder::new("reporting_tag")
                    .bytes(vec![1u8; 16])
                    .build()])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert!(info.meta_info.reporting_tag.is_some());
        assert!(info.meta_info.reporting_token.is_none());
        assert!(info.meta_info.reporting_token_version.is_none());
    }

    /// Message with no `<meta>` and no `<reporting>` leaves all the new
    /// `MsgMetaInfo` fields `None`.
    #[test]
    fn meta_and_reporting_absent_leaves_all_none() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-PLAIN")
            .attr("t", "1777415965")
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert!(info.meta_info.content_type.is_none());
        assert!(info.meta_info.appdata.is_none());
        assert!(info.meta_info.reporting_tag.is_none());
        assert!(info.meta_info.reporting_token.is_none());
    }

    /// Symmetric branch: when `participant` is a LID, `sender_alt` must come
    /// from `participant_pn`. Pins the `Server::Lid`/`is_lid_family()` arm.
    #[test]
    fn status_broadcast_with_participant_pn_populates_sender_alt() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let own_lid = Jid::from_str("100000000000000@lid").unwrap();
        let pn_user = "559980000001";
        let lid_user = "100000012345678";
        let node = NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("type", "media")
            .attr("id", "TEST_LID_FIRST_MSG_ID")
            .attr("t", "1777415965")
            .attr("participant", format!("{lid_user}@lid").as_str())
            .attr(
                "participant_pn",
                format!("{pn_user}@s.whatsapp.net").as_str(),
            )
            .build();

        let info = parse_message_info(&node.as_node_ref(), &own_pn, Some(&own_lid))
            .expect("parse_message_info should succeed for LID-addressed status");

        assert_eq!(info.source.sender.user, lid_user);
        assert_eq!(info.source.sender.server, wacore_binary::Server::Lid);
        let alt = info
            .source
            .sender_alt
            .as_ref()
            .expect("LID-addressed status broadcast must expose participant_pn as sender_alt");
        assert_eq!(alt.user, pn_user);
        assert_eq!(alt.server, wacore_binary::Server::Pn);
    }

    #[test]
    fn random_pad_len_is_uniform_1_to_16() {
        // WA Web / whatsmeow pad with rand%16 + 1; the value must always land
        // in 1..=16 (never 0, never >16). The old `& 0x0F` 0->15 remap could
        // never produce 16; assert 16 is reachable over many samples.
        let mut saw_16 = false;
        for _ in 0..5_000 {
            let p = super::MessageUtils::random_pad_len();
            assert!((1..=16).contains(&p), "pad len {p} out of 1..=16");
            saw_16 |= p == 16;
        }
        assert!(
            saw_16,
            "pad len 16 must be reachable (was unreachable before)"
        );
    }

    // Cross-impl phash parity vs whatsmeow (`base64.RawStdEncoding`) and WA Web
    // (`WABase64.encodeB64` = standard '+'/'/'). Inputs engineered so
    // sha256(adstrings)[..6] hits base64 index 62/63 — these are exactly the
    // bytes that URL-safe ('-'/'_') would have encoded differently from the
    // server. Pins our output to the standard alphabet the server expects.
    #[test]
    fn phash_crosscheck_vectors() {
        fn dev(user: &str, device: u16, server: wacore_binary::Server) -> Jid {
            Jid {
                user: user.into(),
                server,
                agent: 0,
                device,
                integrator: 0,
            }
        }

        let single = vec![dev("5511999999999", 3, wacore_binary::Server::Pn)];
        assert_eq!(single[0].to_ad_string(), "5511999999999.0:3@s.whatsapp.net");
        let h_single = MessageUtils::participant_list_hash(&single).unwrap();

        let control = vec![dev("5511999999999", 0, wacore_binary::Server::Pn)];
        let h_control = MessageUtils::participant_list_hash(&control).unwrap();

        let multi = vec![
            dev("5511988887777", 14, wacore_binary::Server::Pn),
            dev("7469250125917", 21, wacore_binary::Server::Pn),
        ];
        let h_multi = MessageUtils::participant_list_hash(&multi).unwrap();

        eprintln!("RUST_PHASH single   = {h_single}");
        eprintln!("RUST_PHASH control  = {h_control}");
        eprintln!("RUST_PHASH multi    = {h_multi}");

        // Standard-base64 outputs (match whatsmeow + WA Web = the server).
        // `single` and `multi` carry a 62/63 byte, so they differ from the
        // old URL-safe output (`2:5s-YxCff` / `2:AAv_hwhn`); `control` has
        // neither, so it is unchanged across alphabets.
        assert_eq!(h_single, "2:5s+YxCff");
        assert_eq!(h_control, "2:RJWVxcMQ");
        assert_eq!(h_multi, "2:AAv/hwhn");
    }

    /// Locks the arena-sorted phash against the straightforward reference
    /// (one String per device, sorted, concatenated) over a mixed device set:
    /// unsorted input, duplicate JIDs, agents, multiple servers, and the
    /// prefix-ordering edge ("111" vs "1110" users) where a slice comparator
    /// bug would diverge from String ordering.
    #[test]
    fn phash_arena_matches_per_string_reference() {
        use sha2::{Digest, Sha256};

        fn dev(user: &str, agent: u8, device: u16, server: wacore_binary::Server) -> Jid {
            Jid {
                user: user.into(),
                server,
                agent,
                device,
                integrator: 0,
            }
        }

        let devices = vec![
            dev("5511999990000", 0, 14, wacore_binary::Server::Pn),
            dev("111", 0, 0, wacore_binary::Server::Pn),
            dev("1110", 0, 0, wacore_binary::Server::Pn),
            dev("100000000000001", 2, 3, wacore_binary::Server::Lid),
            dev("5511999990000", 0, 14, wacore_binary::Server::Pn),
            dev("5511888880000", 1, 0, wacore_binary::Server::Hosted),
            dev("999", 0, 65535, wacore_binary::Server::Bot),
        ];

        let mut reference: Vec<String> = devices.iter().map(|j| j.to_ad_string()).collect();
        reference.sort_unstable();
        let mut hasher = Sha256::new();
        for jid in &reference {
            hasher.update(jid.as_bytes());
        }
        let digest = hasher.finalize();
        let mut expected = String::with_capacity(10);
        expected.push_str("2:");
        use base64::Engine as _;
        base64::prelude::BASE64_STANDARD_NO_PAD.encode_string(&digest[..6], &mut expected);

        assert_eq!(
            MessageUtils::participant_list_hash(&devices).unwrap(),
            expected
        );
    }

    // #6 — validate_bcl_hash accepts the matching phashV2 and rejects a tampered
    // one (the WA Web validateBclHash check on device-sent broadcasts).
    #[test]
    fn validate_bcl_hash_matches_and_rejects() {
        let participants = vec![
            Jid::from_str("100000000000001@lid").unwrap(),
            Jid::from_str("100000000000002@lid").unwrap(),
        ];
        let good = MessageUtils::participant_list_hash(&participants).unwrap();
        assert!(MessageUtils::validate_bcl_hash(&participants, &good));
        assert!(!MessageUtils::validate_bcl_hash(&participants, "2:wrongxx"));
    }

    // #6 — broadcast/status stanzas expose <participants><to jid> as
    // `bcl_participants` (for the device-sent phash check).
    #[test]
    fn broadcast_populates_bcl_participants() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("type", "media")
            .attr("id", "BCL-1")
            .attr("t", "1777415965")
            .attr("participant", "559980000001@s.whatsapp.net")
            .children([NodeBuilder::new("participants")
                .children([
                    NodeBuilder::new("to")
                        .attr("jid", "100000000000001@lid")
                        .build(),
                    NodeBuilder::new("to")
                        .attr("jid", "100000000000002@lid")
                        .build(),
                ])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(info.bcl_participants.len(), 2);
    }

    // #6 — a group's <participants> is the device fanout, NOT a bcl, so it must
    // not feed the bcl hash check.
    #[test]
    fn group_participants_do_not_populate_bcl() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "120363000000000001@g.us")
            .attr("participant", "559980000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "G-1")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("participants")
                .children([NodeBuilder::new("to")
                    .attr("jid", "559980000002:3@s.whatsapp.net")
                    .build()])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert!(
            info.bcl_participants.is_empty(),
            "group fanout participants are not a bcl"
        );
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod device_sent_tests {
    use super::*;

    fn msg_with_secret(secret: &[u8]) -> wa::Message {
        wa::Message {
            conversation: Some("hi".into()),
            message_context_info: wa::MessageContextInfo {
                message_secret: Some(secret.to_vec()),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        }
    }

    #[test]
    fn wrap_hoists_context_to_outer_on_wire() {
        let secret = [7u8; 32];
        let wrapped = wrap_device_sent(msg_with_secret(&secret), "1@s.whatsapp.net".into());

        let bytes = wrapped.encode_to_vec();
        let decoded = wa::Message::decode_from_slice(bytes.as_slice()).unwrap();

        assert_eq!(
            decoded
                .message_context_info
                .as_option()
                .and_then(|c| c.message_secret.as_deref()),
            Some(secret.as_slice())
        );
        let inner = decoded
            .device_sent_message
            .as_option()
            .unwrap()
            .message
            .as_option()
            .unwrap();
        assert!(inner.message_context_info.is_unset());
        assert_eq!(inner.conversation.as_deref(), Some("hi"));
    }

    #[test]
    fn wrap_without_context_leaves_outer_empty() {
        let inner = wa::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        let wrapped = wrap_device_sent(inner, "1@s.whatsapp.net".into());

        assert!(wrapped.message_context_info.is_unset());
        let dsm = wrapped.device_sent_message.as_option().unwrap();
        assert_eq!(dsm.destination_jid.as_deref(), Some("1@s.whatsapp.net"));
        assert!(
            dsm.message
                .as_option()
                .unwrap()
                .message_context_info
                .is_unset()
        );
    }

    #[test]
    fn wrap_then_unwrap_preserves_non_secret_context_fields() {
        let inner = wa::Message {
            message_context_info: wa::MessageContextInfo {
                message_add_on_duration_in_secs: Some(604800),
                ..Default::default()
            }
            .into(),
            ..Default::default()
        };
        let unwrapped = unwrap_device_sent(wrap_device_sent(inner, "1@s.whatsapp.net".into()));
        assert_eq!(
            unwrapped
                .message_context_info
                .as_option()
                .and_then(|c| c.message_add_on_duration_in_secs),
            Some(604800)
        );
    }

    #[test]
    fn wrap_then_unwrap_round_trips_secret() {
        let secret = [9u8; 32];
        let wrapped = wrap_device_sent(msg_with_secret(&secret), "1@s.whatsapp.net".into());
        let unwrapped = unwrap_device_sent(wrapped);

        assert_eq!(unwrapped.conversation.as_deref(), Some("hi"));
        assert_eq!(
            unwrapped
                .message_context_info
                .as_option()
                .and_then(|c| c.message_secret.as_deref()),
            Some(secret.as_slice())
        );
    }

    // Unpad (v2) + buffa-decode a padded plaintext.
    fn decode_padded(b: &[u8]) -> wa::Message {
        wa::Message::decode_from_slice(MessageUtils::unpad_message_ref(b, 2).unwrap()).unwrap()
    }

    /// The spliced plaintexts must decode to exactly what the prost-based path
    /// produces: recipient == encode(message), own_devices == encode(wrap_device_sent).
    /// `extra` is `None` here (no reporting context); the with-context cases are
    /// covered by `splice_with_reporting_context_matches_prepare`.
    fn assert_splice_matches(message: wa::Message, dest: &str) {
        let recipient_old = decode_padded(&MessageUtils::encode_and_pad(&message));
        let dsm_old = decode_padded(&MessageUtils::encode_and_pad(&wrap_device_sent(
            message.clone(),
            dest.to_string(),
        )));

        let DmPlaintexts {
            recipient,
            own_devices,
        } = MessageUtils::encode_dm_plaintexts(&message, None, dest);

        assert_eq!(
            decode_padded(&recipient),
            recipient_old,
            "recipient mismatch"
        );
        assert_eq!(
            decode_padded(&own_devices),
            dsm_old,
            "own-device DSM mismatch"
        );
    }

    #[test]
    fn splice_matches_prost_across_message_shapes() {
        let dest = "5511999998888:3@s.whatsapp.net";

        // plain conversation text
        assert_splice_matches(
            wa::Message {
                conversation: Some("ping".into()),
                ..Default::default()
            },
            dest,
        );
        // unicode + long text (multi-byte content, larger than one varint length)
        assert_splice_matches(
            wa::Message {
                conversation: Some("héllo 🚀 ".repeat(500)),
                ..Default::default()
            },
            dest,
        );
        // with message_context_info (reporting-token path: secret hoisted to wrapper)
        assert_splice_matches(msg_with_secret(&[42u8; 32]), dest);
        // extended text + nested context_info (forwarded) AND top-level mci
        assert_splice_matches(
            wa::Message {
                extended_text_message: wa::message::ExtendedTextMessage {
                    text: Some("quoted".into()),
                    context_info: wa::ContextInfo {
                        is_forwarded: Some(true),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                }
                .into(),
                message_context_info: wa::MessageContextInfo {
                    message_secret: Some(vec![1, 2, 3, 4]),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            dest,
        );
        // media message (refs/keys), no mci
        assert_splice_matches(
            wa::Message {
                image_message: wa::message::ImageMessage {
                    url: Some("https://mmg.example/abc".into()),
                    media_key: Some(vec![9u8; 32]),
                    file_sha256: Some(vec![8u8; 32]),
                    mimetype: Some("image/jpeg".into()),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            dest,
        );
        // empty message (no content, no mci)
        assert_splice_matches(wa::Message::default(), dest);
        // mci-only (no content body)
        assert_splice_matches(
            wa::Message {
                message_context_info: wa::MessageContextInfo {
                    message_secret: Some(vec![7u8; 32]),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
            dest,
        );
        // empty destination_jid (degenerate but must still match)
        assert_splice_matches(
            wa::Message {
                conversation: Some("x".into()),
                ..Default::default()
            },
            "",
        );
    }

    /// Pin the spliced field numbers to the generated schema: encode a probe
    /// with only the relevant field set and read the first protobuf key. If the
    /// .proto ever renumbers one of these fields, Buffa regenerates and this fails
    /// with a precise message, so the hand-written framing cannot silently drift.
    #[test]
    fn splice_tags_match_generated_schema() {
        fn first_field_number(mut bytes: &[u8]) -> u32 {
            buffa::encoding::Tag::decode(&mut bytes)
                .expect("probe should start with a valid protobuf tag")
                .field_number()
        }

        let outer_dsm = wa::Message {
            device_sent_message: wa::message::DeviceSentMessage::default().into(),
            ..Default::default()
        };
        assert_eq!(
            first_field_number(&outer_dsm.encode_to_vec()),
            TAG_DEVICE_SENT_MESSAGE,
            "Message.device_sent_message tag drifted from the .proto"
        );

        let outer_mci = wa::Message {
            message_context_info: wa::MessageContextInfo::default().into(),
            ..Default::default()
        };
        assert_eq!(
            first_field_number(&outer_mci.encode_to_vec()),
            TAG_MESSAGE_CONTEXT_INFO,
            "Message.message_context_info tag drifted from the .proto"
        );

        let dsm_dest = wa::message::DeviceSentMessage {
            destination_jid: Some("x".into()),
            ..Default::default()
        };
        assert_eq!(
            first_field_number(&dsm_dest.encode_to_vec()),
            TAG_DSM_DESTINATION_JID,
            "DeviceSentMessage.destination_jid tag drifted from the .proto"
        );

        let dsm_msg = wa::message::DeviceSentMessage {
            message: wa::Message::default().into(),
            ..Default::default()
        };
        assert_eq!(
            first_field_number(&dsm_msg.encode_to_vec()),
            TAG_DSM_MESSAGE,
            "DeviceSentMessage.message tag drifted from the .proto"
        );
    }

    /// Message shapes exercising both encode-with-context paths: no embedded mci
    /// (common, no clone) and an embedded mci carrying a non-reporting field that
    /// must survive the merge while message_secret is overwritten (rare, clone).
    fn context_test_shapes() -> Vec<wa::Message> {
        vec![
            wa::Message {
                conversation: Some("ping".into()),
                ..Default::default()
            },
            wa::Message {
                conversation: Some("poll".into()),
                message_context_info: wa::MessageContextInfo {
                    // preserved by the merge
                    message_add_on_duration_in_secs: Some(604800),
                    // overwritten by the reporting context
                    message_secret: Some(vec![1u8; 32]),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
        ]
    }

    /// The reporting context the send path injects (message_secret +
    /// reporting_token_version), matching `prepare_message_with_context`.
    fn reporting_context(secret: &[u8; 32]) -> wa::MessageContextInfo {
        wa::MessageContextInfo {
            message_secret: Some(secret.to_vec()),
            reporting_token_version: Some(crate::reporting_token::REPORTING_TOKEN_VERSION),
            ..Default::default()
        }
    }

    /// `encode_dm_plaintexts(&msg, Some(reporting_ctx), dest)` must decode to exactly
    /// what the old clone-based path produced: `prepare_message_with_context(msg, secret)`
    /// then prost `encode_and_pad` (recipient) / `encode_and_pad(wrap_device_sent(..))`
    /// (own devices). The real `prepare_message_with_context` is the oracle, so the merge
    /// semantics (existing fields kept, message_secret + version overwritten) are pinned.
    #[test]
    fn splice_with_reporting_context_matches_prepare() {
        let dest = "5511999998888:3@s.whatsapp.net";
        let secret = [0x5Au8; 32];
        let extra = reporting_context(&secret);

        for message in context_test_shapes() {
            let reference = crate::reporting_token::prepare_message_with_context(&message, &secret);
            let recipient_ref = decode_padded(&MessageUtils::encode_and_pad(&reference));
            let dsm_ref = decode_padded(&MessageUtils::encode_and_pad(&wrap_device_sent(
                reference.clone(),
                dest.to_string(),
            )));

            let DmPlaintexts {
                recipient,
                own_devices,
            } = MessageUtils::encode_dm_plaintexts(&message, Some(&extra), dest);

            assert_eq!(
                decode_padded(&recipient),
                recipient_ref,
                "recipient mismatch for {message:?}"
            );
            assert_eq!(
                decode_padded(&own_devices),
                dsm_ref,
                "own-device DSM mismatch for {message:?}"
            );
        }
    }

    /// `prepare_dm_stanza` skips the DeviceSentMessage buffer (and its destination-jid
    /// stringify) when there are no own companion devices, encoding the recipient via
    /// `encode_and_pad_with_context` instead of `encode_dm_plaintexts`. That swap is only
    /// sound while both agree on the recipient bytes for messages without a top-level
    /// `message_context_info` — exactly the gate `prepare_dm_stanza` uses. Pin that
    /// agreement (with and without a reporting context) so a change to either encoder
    /// can't silently corrupt the no-own-device send path.
    #[test]
    fn recipient_only_encode_matches_dm_recipient_without_top_level_mci() {
        let dest = "5511999998888:3@s.whatsapp.net";
        let reporting_ctx = reporting_context(&[0x5Au8; 32]);

        let shapes = [
            wa::Message {
                conversation: Some("ping".into()),
                ..Default::default()
            },
            wa::Message {
                image_message: wa::message::ImageMessage {
                    url: Some("https://mmg.example/abc".into()),
                    media_key: Some(vec![9u8; 32]),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
        ];

        for message in shapes {
            assert!(
                message.message_context_info.is_unset(),
                "fast path only applies to messages without a top-level mci"
            );
            for extra in [None, Some(&reporting_ctx)] {
                let recipient_only = MessageUtils::encode_and_pad_with_context(&message, extra);
                let dm_recipient =
                    MessageUtils::encode_dm_plaintexts(&message, extra, dest).recipient;
                assert_eq!(
                    decode_padded(&recipient_only),
                    decode_padded(&dm_recipient),
                    "recipient-only encode diverged from encode_dm_plaintexts ({message:?}, extra={extra:?})"
                );
            }
        }
    }

    /// The `_from_encoded` encoders let the DM send path reuse the single message encode
    /// it also feeds to the reporting token. They must produce byte-identical wire output
    /// (modulo random padding) to re-encoding the message via `encode_and_pad_with_context`
    /// / `encode_dm_plaintexts`, or the shared-encode send path would corrupt the payload.
    /// Their precondition is a message with no top-level mci, so only those shapes apply.
    #[test]
    fn from_encoded_encoders_match_reencoding_without_top_level_mci() {
        let dest = "5511999998888:3@s.whatsapp.net";
        let reporting_ctx = reporting_context(&[0x5Au8; 32]);

        let unpad = |b: &[u8]| MessageUtils::unpad_message_ref(b, 2).unwrap().to_vec();

        let shapes = [
            wa::Message {
                conversation: Some("ping".into()),
                ..Default::default()
            },
            wa::Message {
                conversation: Some("héllo 🚀 ".repeat(500)),
                ..Default::default()
            },
            wa::Message {
                image_message: wa::message::ImageMessage {
                    url: Some("https://mmg.example/abc".into()),
                    media_key: Some(vec![9u8; 32]),
                    ..Default::default()
                }
                .into(),
                ..Default::default()
            },
        ];

        for message in shapes {
            assert!(
                message.message_context_info.is_unset(),
                "the _from_encoded path only applies to messages without a top-level mci"
            );
            let content = message.encode_to_vec();
            for extra in [None, Some(&reporting_ctx)] {
                assert_eq!(
                    unpad(&MessageUtils::pad_with_context_from_encoded(
                        &content, extra
                    )),
                    unpad(&MessageUtils::encode_and_pad_with_context(&message, extra)),
                    "pad_with_context_from_encoded diverged ({message:?}, extra={extra:?})"
                );

                let from_encoded = MessageUtils::dm_plaintexts_from_encoded(&content, extra, dest);
                let reencoded = MessageUtils::encode_dm_plaintexts(&message, extra, dest);
                assert_eq!(
                    unpad(&from_encoded.recipient),
                    unpad(&reencoded.recipient),
                    "dm_plaintexts_from_encoded recipient diverged ({message:?}, extra={extra:?})"
                );
                assert_eq!(
                    unpad(&from_encoded.own_devices),
                    unpad(&reencoded.own_devices),
                    "dm_plaintexts_from_encoded own_devices diverged ({message:?}, extra={extra:?})"
                );
            }
        }
    }

    /// `encode_and_pad_with_context` (the group path) with a reporting context must
    /// decode to `prepare_message_with_context(msg, secret)` encoded by prost; with
    /// `None` it must equal plain `encode_and_pad`.
    #[test]
    fn group_encode_with_context_matches_prepare() {
        let secret = [0x33u8; 32];
        let extra = reporting_context(&secret);

        for message in context_test_shapes() {
            let reference = crate::reporting_token::prepare_message_with_context(&message, &secret);
            let ref_decoded = decode_padded(&MessageUtils::encode_and_pad(&reference));
            let got = decode_padded(&MessageUtils::encode_and_pad_with_context(
                &message,
                Some(&extra),
            ));
            assert_eq!(got, ref_decoded, "group encode-with-context mismatch");
        }

        let plain = wa::Message {
            conversation: Some("x".into()),
            ..Default::default()
        };
        assert_eq!(
            decode_padded(&MessageUtils::encode_and_pad_with_context(&plain, None)),
            decode_padded(&MessageUtils::encode_and_pad(&plain)),
            "encode_and_pad_with_context(None) must equal encode_and_pad"
        );
    }
}
