// Protobuf definitions, generated at build time into OUT_DIR from the
// committed `whatsapp.desc` descriptor (see build.rs). Consumers never need
// `protoc`; editing `whatsapp.proto` requires regenerating the descriptor via
// `scripts/regenerate-proto-desc.sh` and committing both files.

#![allow(clippy::large_enum_variant)]
pub mod whatsapp {
    include!(concat!(env!("OUT_DIR"), "/whatsapp.rs"));
}

/// Wire tags of every message field in `whatsapp.proto`, generated alongside
/// the prost code. Hand-written partial decoders must reference these consts
/// (or compile-time assert against them) instead of magic numbers, so schema
/// changes surface as compile errors rather than silent wire-format drift.
pub mod tags {
    include!(concat!(env!("OUT_DIR"), "/tags.rs"));
}

/// Pinned, non-generic codec entry points for the hottest protobuf roots.
///
/// prost's `Message` methods are generic, so rustc instantiates them in every
/// crate that calls them; the per-crate copies carry distinct
/// instantiating-crate symbol hashes that LTO cannot merge, and each calling
/// crate ends up shipping its own copy of the full encode or decode tree
/// (`whatsapp::Message::encode_raw` alone is ~160 KiB per copy). Routing
/// calls through these functions pins a single instantiation in this crate;
/// `#[inline(never)]` keeps MIR inlining from re-expanding them at call
/// sites, which would silently reintroduce the per-crate copies.
///
/// Decode helpers take `&[u8]` and decode via `&mut &[u8]`, the buffer shape
/// the rest of the workspace already instantiates, so no second buffer-type
/// tree exists.
pub mod codec {
    use crate::whatsapp;
    use prost::Message as _;

    #[inline(never)]
    pub fn message_encoded_len(msg: &whatsapp::Message) -> usize {
        msg.encoded_len()
    }

    /// Append the encoded message to `out`. Infallible into a `Vec`.
    #[inline(never)]
    pub fn message_encode_into(msg: &whatsapp::Message, out: &mut Vec<u8>) {
        msg.encode(out).expect("encode into Vec is infallible");
    }

    #[inline(never)]
    pub fn message_to_vec(msg: &whatsapp::Message) -> Vec<u8> {
        msg.encode_to_vec()
    }

    /// Decode a `Message` into a heap box without a by-value detour.
    ///
    /// `Message` is large enough that returning it by value made the decode path
    /// memcpy-bound (PR #857). `Box::new(Message::decode(..))` would still build
    /// the message in a by-value return slot and memcpy it into the box; this
    /// `default()` + `merge` parses straight into the heap allocation. The buffer
    /// is passed as `&mut &mut &[u8]` (the shape `Message::decode` uses
    /// internally) so prost reuses the workspace's single `merge_field`
    /// instantiation instead of duplicating it for a `&mut &[u8]` buffer
    /// (~210 KiB of `.text`).
    #[inline(never)]
    pub fn message_decode(mut bytes: &[u8]) -> Result<Box<whatsapp::Message>, prost::DecodeError> {
        let mut reader: &mut &[u8] = &mut bytes;
        let mut msg = Box::<whatsapp::Message>::default();
        msg.merge(&mut reader)?;
        Ok(msg)
    }

    #[inline(never)]
    pub fn web_message_info_decode(
        mut bytes: &[u8],
    ) -> Result<whatsapp::WebMessageInfo, prost::DecodeError> {
        whatsapp::WebMessageInfo::decode(&mut bytes)
    }

    #[inline(never)]
    pub fn history_sync_decode(
        mut bytes: &[u8],
    ) -> Result<whatsapp::HistorySync, prost::DecodeError> {
        whatsapp::HistorySync::decode(&mut bytes)
    }

    #[inline(never)]
    pub fn message_context_info_encoded_len(mci: &whatsapp::MessageContextInfo) -> usize {
        mci.encoded_len()
    }

    /// Append the encoded `MessageContextInfo` to `out`. Infallible into a `Vec`.
    #[inline(never)]
    pub fn message_context_info_encode_into(mci: &whatsapp::MessageContextInfo, out: &mut Vec<u8>) {
        mci.encode(out).expect("encode into Vec is infallible");
    }

    #[inline(never)]
    pub fn message_context_info_to_vec(mci: &whatsapp::MessageContextInfo) -> Vec<u8> {
        mci.encode_to_vec()
    }
}
