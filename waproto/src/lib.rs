//! Auto-generated protobuf definitions for the WhatsApp wire format.
//!
//! The Rust source (`whatsapp.rs`) is produced by `build.rs` from the
//! pre-compiled descriptor set `whatsapp.desc`, and written to `OUT_DIR` —
//! not tracked in git. To regenerate the descriptor after editing
//! `whatsapp.proto`, run `scripts/regenerate-proto-desc.sh` (wraps `protoc`).

#![allow(clippy::large_enum_variant)]
/// Re-exported because its types permeate the generated API; depending on it
/// directly would require version-matching this crate exactly.
pub use buffa;

pub mod whatsapp {
    #![allow(
        non_camel_case_types,
        non_snake_case,
        unreachable_patterns,
        clippy::derivable_impls,
        clippy::match_single_binding,
        clippy::needless_else
    )]
    #[rustfmt::skip]
    buffa::include_proto!("whatsapp");
}

/// Wire tags of every message field in `whatsapp.proto`, generated alongside
/// the buffa code. Hand-written partial decoders must reference these consts
/// (or compile-time assert against them) instead of magic numbers, so schema
/// changes surface as compile errors rather than silent wire-format drift.
pub mod tags {
    include!(concat!(env!("OUT_DIR"), "/tags.rs"));
}

/// Pinned, non-generic codec entry points for the hottest protobuf roots.
///
/// buffa's `Message` encode/decode methods are generic over the buffer type, so
/// rustc instantiates them in every crate that calls them; the per-crate copies
/// carry distinct instantiating-crate symbol hashes that LTO cannot merge, and
/// each calling crate ends up shipping its own copy of the full encode/decode
/// tree. Routing calls through these functions pins a single instantiation in
/// this crate; `#[inline(never)]` keeps MIR inlining from re-expanding them at
/// call sites, which would silently reintroduce the per-crate copies.
///
/// Decode helpers take `&[u8]` and decode via `decode_from_slice`, the buffer
/// shape the rest of the workspace already instantiates, so no second
/// buffer-type tree exists.
pub mod codec {
    use crate::whatsapp;
    use buffa::Message as _;

    #[inline(never)]
    pub fn message_encoded_len(msg: &whatsapp::Message) -> usize {
        msg.encoded_len() as usize
    }

    /// Append the encoded message to `out`. Infallible into a `Vec`.
    #[inline(never)]
    pub fn message_encode_into(msg: &whatsapp::Message, out: &mut Vec<u8>) {
        msg.encode(out);
    }

    #[inline(never)]
    pub fn message_to_vec(msg: &whatsapp::Message) -> Vec<u8> {
        msg.encode_to_vec()
    }

    /// Two-pass encode with a caller-owned `SizeCache`: `compute_size` fills the
    /// cache, `write_to` reuses it. The send path needs the size before writing
    /// (to pre-size buffers and splice nested fields by hand), so it drives the
    /// two passes itself instead of calling `encode`. Pinning both keeps the
    /// `Message` encode tree out of the calling crate.
    #[inline(never)]
    pub fn message_compute_size(msg: &whatsapp::Message, cache: &mut buffa::SizeCache) -> usize {
        msg.compute_size(cache) as usize
    }

    #[inline(never)]
    pub fn message_write_to(
        msg: &whatsapp::Message,
        cache: &mut buffa::SizeCache,
        out: &mut Vec<u8>,
    ) {
        msg.write_to(cache, out);
    }

    #[inline(never)]
    pub fn message_decode(bytes: &[u8]) -> Result<whatsapp::Message, buffa::DecodeError> {
        whatsapp::Message::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn web_message_info_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::WebMessageInfo, buffa::DecodeError> {
        whatsapp::WebMessageInfo::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn history_sync_decode(bytes: &[u8]) -> Result<whatsapp::HistorySync, buffa::DecodeError> {
        whatsapp::HistorySync::decode_from_slice(bytes)
    }

    /// History-sync streaming decodes individual `HistorySyncMsg`/`Conversation`
    /// records; pinning them here keeps their nested `WebMessageInfo`/`Message`
    /// decode tree from being re-instantiated in the calling crate.
    #[inline(never)]
    pub fn history_sync_msg_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::HistorySyncMsg, buffa::DecodeError> {
        whatsapp::HistorySyncMsg::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn conversation_decode(bytes: &[u8]) -> Result<whatsapp::Conversation, buffa::DecodeError> {
        whatsapp::Conversation::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn message_context_info_encoded_len(mci: &whatsapp::MessageContextInfo) -> usize {
        mci.encoded_len() as usize
    }

    /// Append the encoded `MessageContextInfo` to `out`. Infallible into a `Vec`.
    #[inline(never)]
    pub fn message_context_info_encode_into(mci: &whatsapp::MessageContextInfo, out: &mut Vec<u8>) {
        mci.encode(out);
    }

    #[inline(never)]
    pub fn message_context_info_to_vec(mci: &whatsapp::MessageContextInfo) -> Vec<u8> {
        mci.encode_to_vec()
    }

    /// `SizeCache`-driven two-pass encode for `MessageContextInfo`, mirroring
    /// [`message_compute_size`]/[`message_write_to`]; the send path splices the
    /// mci as a nested length-delimited field, so it needs the size before the
    /// write.
    #[inline(never)]
    pub fn message_context_info_compute_size(
        mci: &whatsapp::MessageContextInfo,
        cache: &mut buffa::SizeCache,
    ) -> usize {
        mci.compute_size(cache) as usize
    }

    #[inline(never)]
    pub fn message_context_info_write_to(
        mci: &whatsapp::MessageContextInfo,
        cache: &mut buffa::SizeCache,
        out: &mut Vec<u8>,
    ) {
        mci.write_to(cache, out);
    }

    /// Merge wire bytes into an existing `MessageContextInfo` (proto merge
    /// semantics: later-set fields win).
    #[inline(never)]
    pub fn message_context_info_merge(
        mci: &mut whatsapp::MessageContextInfo,
        bytes: &[u8],
    ) -> Result<(), buffa::DecodeError> {
        mci.merge_from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::whatsapp as wa;
    use buffa::Message;
    use buffa::view::MessageView;

    #[test]
    fn generated_views_and_oneofs_round_trip() {
        let msg = wa::Message {
            interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                interactive_message: Some(
                    wa::message::interactive_message::InteractiveMessage::NativeFlowMessage(
                        Box::new(wa::message::interactive_message::NativeFlowMessage {
                            buttons: vec![
                                wa::message::interactive_message::native_flow_message::NativeFlowButton {
                                    name: Some("quick_reply".to_string()),
                                    ..Default::default()
                                },
                            ],
                            message_version: Some(1),
                            ..Default::default()
                        }),
                    ),
                ),
                ..Default::default()
            }),
            ..Default::default()
        };

        let bytes = msg.encode_to_vec();
        let decoded = wa::Message::decode_from_slice(&bytes).unwrap();
        let interactive = decoded.interactive_message.as_option().unwrap();
        let Some(wa::message::interactive_message::InteractiveMessage::NativeFlowMessage(native)) =
            interactive.interactive_message.as_ref()
        else {
            panic!("expected native flow oneof");
        };
        assert_eq!(native.buttons[0].name.as_deref(), Some("quick_reply"));

        let view = wa::MessageView::decode_view(&bytes).unwrap();
        let interactive = view.interactive_message.as_option().unwrap();
        let Some(wa::message::interactive_message::InteractiveMessageView::NativeFlowMessage(
            native,
        )) = interactive.interactive_message.as_ref()
        else {
            panic!("expected native flow view oneof");
        };
        assert_eq!(native.buttons[0].name, Some("quick_reply"));
    }
}
