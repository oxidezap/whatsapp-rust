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
    // disallowed_methods: the generated impls call the buffa trait methods on
    // nested messages; that IS the pinned instantiation.
    // unused_qualifications: the generator emits fully-qualified paths on
    // purpose so the output cannot collide with names in the including scope.
    #![allow(
        non_camel_case_types,
        non_snake_case,
        unreachable_patterns,
        unused_qualifications,
        clippy::derivable_impls,
        clippy::match_single_binding,
        clippy::needless_else,
        clippy::disallowed_methods
    )]
    #[rustfmt::skip]
    buffa::include_proto!("whatsapp");
}

/// Field-level serde for `Option<EnumValue<E>>` fields (open enums, currently
/// only `SyncdMutation.operation`), wired in via `field_attribute` in
/// `build.rs`.
///
/// `EnumValue`'s own serde impls speak exact protobuf names (`SET`) on both
/// sides, which would silently break the per-feature contracts the closed
/// enums honor: `serde-enum-repr` serializes numerically for the JS bridge,
/// and `serde-snake-case` deserializes lowercase variant names. Routing the
/// known-value case through the enum's derived impls keeps an opened field on
/// exactly the closed-enum contract; only `Unknown` values add behavior
/// (serialized as, and deserializable from, the raw integer).
pub mod open_enum_serde {
    use buffa::{EnumValue, Enumeration};

    pub fn serialize<E, S>(value: &Option<EnumValue<E>>, s: S) -> Result<S::Ok, S::Error>
    where
        E: Enumeration + serde::Serialize,
        S: serde::Serializer,
    {
        match value {
            None => s.serialize_none(),
            Some(EnumValue::Known(e)) => s.serialize_some(e),
            Some(EnumValue::Unknown(n)) => s.serialize_some(n),
        }
    }

    #[cfg(feature = "serde-deserialize")]
    pub fn deserialize<'de, E, D>(d: D) -> Result<Option<EnumValue<E>>, D::Error>
    where
        E: Enumeration + serde::Deserialize<'de>,
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Wire<E> {
            // The enum's derived impl first, so every feature-selected input
            // shape (lowercase names, numeric repr) keeps working; the raw
            // integer fallback only catches values the enum doesn't know.
            Known(E),
            Raw(i32),
        }
        Ok(
            match <Option<Wire<E>> as serde::Deserialize>::deserialize(d)? {
                None => None,
                Some(Wire::Known(e)) => Some(EnumValue::Known(e)),
                Some(Wire::Raw(n)) => Some(EnumValue::from(n)),
            },
        )
    }
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
// The pinning wrappers are the one sanctioned direct-call site (see
// clippy.toml disallowed-methods).
#[allow(clippy::disallowed_methods)]
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

    /// [`conversation_decode`] into an existing struct. buffa's `clear()`
    /// retains `Vec` capacity, so a caller draining thousands of conversations
    /// through one reused struct skips the per-entry message-spine reallocs.
    /// On error the struct is left cleared-then-partially-merged; callers must
    /// treat it as garbage until the next successful merge.
    #[inline(never)]
    pub fn conversation_merge_from_slice(
        conv: &mut whatsapp::Conversation,
        bytes: &[u8],
    ) -> Result<(), buffa::DecodeError> {
        conv.clear();
        conv.merge_from_slice(bytes)
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

    /// App state sync roots. wacore-appstate and wacore each instantiated
    /// these decode/encode trees themselves (SyncActionData drags in the very
    /// wide SyncActionValue subtree), and buffa 0.9's larger per-instantiation
    /// decode codegen made those per-crate copies the dominant .text cost of
    /// the upgrade; pinning them here keeps one copy, shared across the
    /// snapshot/patch/mutations paths.
    #[inline(never)]
    pub fn syncd_snapshot_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::SyncdSnapshot, buffa::DecodeError> {
        whatsapp::SyncdSnapshot::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn syncd_mutations_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::SyncdMutations, buffa::DecodeError> {
        whatsapp::SyncdMutations::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn syncd_patch_decode(bytes: &[u8]) -> Result<whatsapp::SyncdPatch, buffa::DecodeError> {
        whatsapp::SyncdPatch::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn syncd_patch_to_vec(patch: &whatsapp::SyncdPatch) -> Vec<u8> {
        patch.encode_to_vec()
    }

    #[inline(never)]
    pub fn external_blob_reference_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ExternalBlobReference, buffa::DecodeError> {
        whatsapp::ExternalBlobReference::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn sync_action_data_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::SyncActionData, buffa::DecodeError> {
        whatsapp::SyncActionData::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn sync_action_data_to_vec(data: &whatsapp::SyncActionData) -> Vec<u8> {
        data.encode_to_vec()
    }

    /// Signal storage records. Decoded/encoded from wacore-libsignal and the
    /// prekey upload paths in the main crate — two private copies of each tree
    /// before being pinned here.
    #[inline(never)]
    pub fn pre_key_record_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::PreKeyRecordStructure, buffa::DecodeError> {
        whatsapp::PreKeyRecordStructure::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn pre_key_record_to_vec(record: &whatsapp::PreKeyRecordStructure) -> Vec<u8> {
        record.encode_to_vec()
    }

    /// Append the encoded record to `out`; the bulk prekey upload packs many
    /// records into one shared buffer.
    #[inline(never)]
    pub fn pre_key_record_encode_into(record: &whatsapp::PreKeyRecordStructure, out: &mut Vec<u8>) {
        record.encode(out);
    }

    #[inline(never)]
    pub fn signed_pre_key_record_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::SignedPreKeyRecordStructure, buffa::DecodeError> {
        whatsapp::SignedPreKeyRecordStructure::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn signed_pre_key_record_to_vec(record: &whatsapp::SignedPreKeyRecordStructure) -> Vec<u8> {
        record.encode_to_vec()
    }

    #[inline(never)]
    pub fn sender_key_record_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::SenderKeyRecordStructure, buffa::DecodeError> {
        whatsapp::SenderKeyRecordStructure::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn sender_key_record_to_vec(record: &whatsapp::SenderKeyRecordStructure) -> Vec<u8> {
        record.encode_to_vec()
    }

    /// Pairing / device-identity roots. The ADV trees were instantiated in
    /// both wacore (adv verification, pair-success, device store) and the main
    /// crate (pairing events, retry receipts).
    #[inline(never)]
    pub fn adv_signed_device_identity_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ADVSignedDeviceIdentity, buffa::DecodeError> {
        whatsapp::ADVSignedDeviceIdentity::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn adv_signed_device_identity_to_vec(id: &whatsapp::ADVSignedDeviceIdentity) -> Vec<u8> {
        id.encode_to_vec()
    }

    #[inline(never)]
    pub fn adv_device_identity_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ADVDeviceIdentity, buffa::DecodeError> {
        whatsapp::ADVDeviceIdentity::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn adv_signed_device_identity_hmac_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ADVSignedDeviceIdentityHMAC, buffa::DecodeError> {
        whatsapp::ADVSignedDeviceIdentityHMAC::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn adv_signed_key_index_list_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ADVSignedKeyIndexList, buffa::DecodeError> {
        whatsapp::ADVSignedKeyIndexList::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn adv_signed_key_index_list_to_vec(
        key_index: &whatsapp::ADVSignedKeyIndexList,
    ) -> Vec<u8> {
        key_index.encode_to_vec()
    }

    #[inline(never)]
    pub fn adv_key_index_list_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ADVKeyIndexList, buffa::DecodeError> {
        whatsapp::ADVKeyIndexList::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn adv_key_index_list_to_vec(key_index: &whatsapp::ADVKeyIndexList) -> Vec<u8> {
        key_index.encode_to_vec()
    }

    #[inline(never)]
    pub fn client_pairing_props_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::ClientPairingProps, buffa::DecodeError> {
        whatsapp::ClientPairingProps::decode_from_slice(bytes)
    }

    /// Connection handshake roots (once per connection, but the ClientPayload
    /// tree — UserAgent, WebInfo, DevicePairingRegistrationData — is wide).
    #[inline(never)]
    pub fn client_payload_to_vec(payload: &whatsapp::ClientPayload) -> Vec<u8> {
        payload.encode_to_vec()
    }

    #[inline(never)]
    pub fn device_props_to_vec(props: &whatsapp::DeviceProps) -> Vec<u8> {
        props.encode_to_vec()
    }

    #[inline(never)]
    pub fn handshake_message_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::HandshakeMessage, buffa::DecodeError> {
        whatsapp::HandshakeMessage::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn handshake_message_to_vec(msg: &whatsapp::HandshakeMessage) -> Vec<u8> {
        msg.encode_to_vec()
    }

    #[inline(never)]
    pub fn lid_migration_mapping_sync_payload_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::LIDMigrationMappingSyncPayload, buffa::DecodeError> {
        whatsapp::LIDMigrationMappingSyncPayload::decode_from_slice(bytes)
    }

    /// Owned decode of the Go-style SKDM fallback in message/special.rs; the
    /// hot SKDM path stays on libsignal's view decode.
    #[inline(never)]
    pub fn sender_key_distribution_message_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::SenderKeyDistributionMessage, buffa::DecodeError> {
        whatsapp::SenderKeyDistributionMessage::decode_from_slice(bytes)
    }

    /// Noise server cert chain, verified once per connection in wacore-noise.
    #[inline(never)]
    pub fn cert_chain_decode(bytes: &[u8]) -> Result<whatsapp::CertChain, buffa::DecodeError> {
        whatsapp::CertChain::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn noise_certificate_details_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::cert_chain::noise_certificate::Details, buffa::DecodeError> {
        whatsapp::cert_chain::noise_certificate::Details::decode_from_slice(bytes)
    }

    /// Business verified-name certificates (usync / business profile parsing).
    #[inline(never)]
    pub fn verified_name_certificate_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::VerifiedNameCertificate, buffa::DecodeError> {
        whatsapp::VerifiedNameCertificate::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn verified_name_certificate_details_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::verified_name_certificate::Details, buffa::DecodeError> {
        whatsapp::verified_name_certificate::Details::decode_from_slice(bytes)
    }

    /// SHORTCAKE passkey-pairing protos (one flow per device link).
    #[inline(never)]
    pub fn companion_ephemeral_identity_to_vec(
        id: &whatsapp::CompanionEphemeralIdentity,
    ) -> Vec<u8> {
        id.encode_to_vec()
    }

    #[inline(never)]
    pub fn prologue_payload_to_vec(payload: &whatsapp::ProloguePayload) -> Vec<u8> {
        payload.encode_to_vec()
    }

    #[inline(never)]
    pub fn primary_ephemeral_identity_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::PrimaryEphemeralIdentity, buffa::DecodeError> {
        whatsapp::PrimaryEphemeralIdentity::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn pairing_request_to_vec(req: &whatsapp::PairingRequest) -> Vec<u8> {
        req.encode_to_vec()
    }

    #[inline(never)]
    pub fn encrypted_pairing_request_to_vec(req: &whatsapp::EncryptedPairingRequest) -> Vec<u8> {
        req.encode_to_vec()
    }

    /// Secret-addon payloads (enc reactions, event responses, bot msmsg
    /// replies) and per-message sidecars; small trees, but wacore and the
    /// main crate each stamped private copies.
    #[inline(never)]
    pub fn reaction_message_to_vec(msg: &whatsapp::message::ReactionMessage) -> Vec<u8> {
        msg.encode_to_vec()
    }

    #[inline(never)]
    pub fn reaction_message_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::message::ReactionMessage, buffa::DecodeError> {
        whatsapp::message::ReactionMessage::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn event_response_message_to_vec(msg: &whatsapp::message::EventResponseMessage) -> Vec<u8> {
        msg.encode_to_vec()
    }

    #[inline(never)]
    pub fn event_response_message_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::message::EventResponseMessage, buffa::DecodeError> {
        whatsapp::message::EventResponseMessage::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn message_secret_message_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::MessageSecretMessage, buffa::DecodeError> {
        whatsapp::MessageSecretMessage::decode_from_slice(bytes)
    }

    #[inline(never)]
    pub fn server_error_receipt_to_vec(receipt: &whatsapp::ServerErrorReceipt) -> Vec<u8> {
        receipt.encode_to_vec()
    }

    /// App-state key-share fingerprints, persisted alongside each sync key.
    #[inline(never)]
    pub fn app_state_sync_key_fingerprint_to_vec(
        fp: &whatsapp::message::AppStateSyncKeyFingerprint,
    ) -> Vec<u8> {
        fp.encode_to_vec()
    }

    #[inline(never)]
    pub fn app_state_sync_key_fingerprint_decode(
        bytes: &[u8],
    ) -> Result<whatsapp::message::AppStateSyncKeyFingerprint, buffa::DecodeError> {
        whatsapp::message::AppStateSyncKeyFingerprint::decode_from_slice(bytes)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
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
