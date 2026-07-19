//
// Copyright 2020-2021 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use buffa::Message;
use buffa::view::MessageView;
use bytes::Bytes;
use hmac::{Hmac, KeyInit, Mac};
use rand::{CryptoRng, Rng};
use sha2::Sha256;
use std::ops::Range;
use std::sync::OnceLock;
use subtle::ConstantTimeEq;

use crate::protocol::state::{PreKeyId, SignedPreKeyId};
use crate::protocol::{IdentityKey, PrivateKey, PublicKey, Result, SignalProtocolError, Timestamp};

/// Get-or-init for `OnceLock<Box<[u8]>>` with a fallible initializer.
fn get_or_try_init_bytes(
    cache: &OnceLock<Box<[u8]>>,
    init: impl FnOnce() -> Result<Box<[u8]>>,
) -> Result<&[u8]> {
    if let Some(val) = cache.get() {
        return Ok(val);
    }
    let _ = cache.set(init()?);
    // get() can't be None: even if a racing set() lost, the winner's value is stored
    Ok(cache.get().expect("just set"))
}

fn subslice_range(parent: &[u8], child: &[u8]) -> Option<Range<usize>> {
    let start = (child.as_ptr() as usize).checked_sub(parent.as_ptr() as usize)?;
    let end = start.checked_add(child.len())?;
    (end <= parent.len()).then_some(start..end)
}

/// Own a borrowed wire buffer in one allocation. Constructing `Bytes` from a
/// boxed slice keeps its allocation directly recoverable by
/// [`bytes_into_boxed_slice`] until sharing is actually requested.
#[inline]
fn bytes_from_slice(value: &[u8]) -> Bytes {
    Bytes::from(Box::<[u8]>::from(value))
}

/// Preserve the historical owned-return API. `Bytes -> Vec` reuses a unique,
/// full backing allocation and copies only when the input is a shared slice,
/// which is the only case where returning one standalone `Box<[u8]>` requires
/// new ownership.
#[inline]
fn bytes_into_boxed_slice(value: Bytes) -> Box<[u8]> {
    Vec::<u8>::from(value).into_boxed_slice()
}

const PROTOBUF_TAG_WIRE_BITS: u32 = 3;
const VERSION_NIBBLE_BITS: u32 = 4;
const VERSION_NIBBLE_MASK: u8 = (1 << VERSION_NIBBLE_BITS) - 1;

#[inline]
fn wire_tag_value(field: u32, wire_type: buffa::encoding::WireType) -> u64 {
    (u64::from(field) << PROTOBUF_TAG_WIRE_BITS) | wire_type as u64
}

#[inline]
fn wire_tag_len(field: u32, wire_type: buffa::encoding::WireType) -> usize {
    buffa::encoding::varint_len(wire_tag_value(field, wire_type))
}

#[inline]
fn varint_field_len(field: u32, value: u64) -> usize {
    wire_tag_len(field, buffa::encoding::WireType::Varint) + buffa::encoding::varint_len(value)
}

#[inline]
fn bytes_field_len(field: u32, value_len: usize) -> usize {
    wire_tag_len(field, buffa::encoding::WireType::LengthDelimited)
        + buffa::encoding::varint_len(value_len as u64)
        + value_len
}

#[inline]
fn push_varint_field(field: u32, value: u64, output: &mut Vec<u8>) {
    buffa::encoding::Tag::new(field, buffa::encoding::WireType::Varint).encode(output);
    buffa::encoding::encode_varint(value, output);
}

/// Append a protobuf bytes field and return the value's range in `output`.
#[inline]
fn push_bytes_field(field: u32, value: &[u8], output: &mut Vec<u8>) -> Range<usize> {
    buffa::encoding::Tag::new(field, buffa::encoding::WireType::LengthDelimited).encode(output);
    buffa::encoding::encode_varint(value.len() as u64, output);
    let start = output.len();
    output.extend_from_slice(value);
    start..output.len()
}

#[inline]
fn encode_version_byte(message_version: u8, current_version: u8) -> u8 {
    ((message_version & VERSION_NIBBLE_MASK) << VERSION_NIBBLE_BITS)
        | (current_version & VERSION_NIBBLE_MASK)
}

#[inline]
fn decode_message_version(version_byte: u8) -> u8 {
    version_byte >> VERSION_NIBBLE_BITS
}

// Signal's original implementation uses version 4, but WhatsApp Web,
// Baileys (libsignal-node), and whatsmeow all use version 3.
pub const CIPHERTEXT_MESSAGE_CURRENT_VERSION: u8 = 3;
pub const SENDERKEY_MESSAGE_CURRENT_VERSION: u8 = 3;

const MIN_SUPPORTED_VERSION: u8 = 3;
const MAX_SUPPORTED_VERSION: u8 = 4;

struct ParsedSignalMessage {
    message_version: u8,
    sender_ratchet_key: PublicKey,
    counter: u32,
    ciphertext_range: Range<usize>,
}

/// Expand validation into each ownership adapter so parse errors are lowered
/// directly into its final `Result`. A function returning an intermediate
/// parsed value prevents that optimization on current LLVM and adds work to
/// the common format-probing failure path.
macro_rules! parse_signal_message {
    ($value:expr) => {{
        let value: &[u8] = $value;
        if value.len() < SignalMessage::MAC_LENGTH + 1 {
            return Err(SignalProtocolError::CiphertextMessageTooShort(value.len()));
        }
        let message_version = decode_message_version(value[0]);
        if !(MIN_SUPPORTED_VERSION..=MAX_SUPPORTED_VERSION).contains(&message_version) {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(
                message_version,
            ));
        }

        let view = SignalMessage::decode_view_from(value)?;
        let sender_ratchet_key = view
            .ratchet_key
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let sender_ratchet_key = PublicKey::deserialize(sender_ratchet_key)?;
        let counter = view
            .counter
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let ciphertext_range = SignalMessage::ciphertext_range_from(value, &view)?;

        ParsedSignalMessage {
            message_version,
            sender_ratchet_key,
            counter,
            ciphertext_range,
        }
    }};
}

struct ParsedPreKeySignalMessage {
    message_version: u8,
    registration_id: u32,
    pre_key_id: Option<PreKeyId>,
    signed_pre_key_id: SignedPreKeyId,
    base_key: PublicKey,
    identity_key: IdentityKey,
    message_range: Range<usize>,
    message: ParsedSignalMessage,
}

/// Parse both layers of a PreKey envelope before either ownership adapter
/// allocates or increments a reference count.
macro_rules! parse_pre_key_signal_message {
    ($value:expr) => {{
        let value: &[u8] = $value;
        if value.is_empty() {
            return Err(SignalProtocolError::CiphertextMessageTooShort(value.len()));
        }

        let message_version = decode_message_version(value[0]);
        if !(MIN_SUPPORTED_VERSION..=MAX_SUPPORTED_VERSION).contains(&message_version) {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(
                message_version,
            ));
        }

        let view = waproto::whatsapp::PreKeySignalMessageView::decode_view(&value[1..])
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        let base_key = view
            .base_key
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let identity_key = view
            .identity_key
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let message_bytes = view
            .message
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let signed_pre_key_id = view
            .signed_pre_key_id
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let message_range = subslice_range(value, message_bytes)
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;

        ParsedPreKeySignalMessage {
            message_version,
            registration_id: view.registration_id.unwrap_or_default(),
            pre_key_id: view.pre_key_id.map(Into::into),
            signed_pre_key_id: signed_pre_key_id.into(),
            base_key: PublicKey::deserialize(base_key)?,
            identity_key: IdentityKey::try_from(identity_key)?,
            message_range,
            message: parse_signal_message!(message_bytes),
        }
    }};
}

#[derive(Debug)]
pub enum CiphertextMessage {
    SignalMessage(SignalMessage),
    PreKeySignalMessage(PreKeySignalMessage),
    SenderKeyMessage(SenderKeyMessage),
    PlaintextContent(PlaintextContent),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, derive_more::TryFrom)]
#[repr(u8)]
#[try_from(repr)]
pub enum CiphertextMessageType {
    Whisper = 2,
    PreKey = 3,
    SenderKey = 7,
    Plaintext = 8,
}

impl CiphertextMessage {
    pub fn message_type(&self) -> CiphertextMessageType {
        match self {
            CiphertextMessage::SignalMessage(_) => CiphertextMessageType::Whisper,
            CiphertextMessage::PreKeySignalMessage(_) => CiphertextMessageType::PreKey,
            CiphertextMessage::SenderKeyMessage(_) => CiphertextMessageType::SenderKey,
            CiphertextMessage::PlaintextContent(_) => CiphertextMessageType::Plaintext,
        }
    }

    pub fn serialize(&self) -> &[u8] {
        match self {
            CiphertextMessage::SignalMessage(x) => x.serialized(),
            CiphertextMessage::PreKeySignalMessage(x) => x.serialized(),
            CiphertextMessage::SenderKeyMessage(x) => x.serialized(),
            CiphertextMessage::PlaintextContent(x) => x.serialized(),
        }
    }
}

#[derive(Debug, Clone)]
struct SignalStorage {
    /// Immutable, reference-counted ownership lets a decoded Signal envelope
    /// remain a slice of its transport or enclosing PreKey allocation.
    serialized: Bytes,
    ciphertext_range: Range<usize>,
}

impl SignalStorage {
    #[inline]
    fn serialized(&self) -> &[u8] {
        self.serialized.as_ref()
    }

    #[inline]
    fn ciphertext(&self) -> &[u8] {
        &self.serialized[self.ciphertext_range.clone()]
    }

    #[inline]
    fn into_boxed_slice(self) -> Box<[u8]> {
        bytes_into_boxed_slice(self.serialized)
    }
}

#[derive(Debug, Clone)]
pub struct SignalMessage {
    message_version: u8,
    sender_ratchet_key: PublicKey,
    counter: u32,
    storage: SignalStorage,
}

impl SignalMessage {
    const MAC_LENGTH: usize = 8;

    // Signal wire protos are (de)serialized only in this crate, so direct
    // buffa calls duplicate nothing; no codec pin needed (here and below).
    #[allow(clippy::too_many_arguments, clippy::disallowed_methods)]
    pub fn new(
        message_version: u8,
        mac_key: &[u8],
        sender_ratchet_key: PublicKey,
        counter: u32,
        previous_counter: u32,
        ciphertext: &[u8],
        sender_identity_key: &IdentityKey,
        receiver_identity_key: &IdentityKey,
    ) -> Result<Self> {
        use waproto::tags::signal_message as tags;

        // Encode the tiny envelope straight into its final allocation. The
        // generated field tags keep this schema-driven, while direct framing
        // avoids allocating a temporary Vec for both the ratchet key and the
        // potentially large ciphertext before copying them into the envelope.
        let ratchet_key = sender_ratchet_key.serialize();
        let proto_len = bytes_field_len(tags::RATCHET_KEY, ratchet_key.len())
            + varint_field_len(tags::COUNTER, u64::from(counter))
            + varint_field_len(tags::PREVIOUS_COUNTER, u64::from(previous_counter))
            + bytes_field_len(tags::CIPHERTEXT, ciphertext.len());
        let mut serialized = Vec::with_capacity(1 + proto_len + Self::MAC_LENGTH);
        serialized.push(encode_version_byte(
            message_version,
            CIPHERTEXT_MESSAGE_CURRENT_VERSION,
        ));
        push_bytes_field(tags::RATCHET_KEY, &ratchet_key, &mut serialized);
        push_varint_field(tags::COUNTER, u64::from(counter), &mut serialized);
        push_varint_field(
            tags::PREVIOUS_COUNTER,
            u64::from(previous_counter),
            &mut serialized,
        );
        let ciphertext_range = push_bytes_field(tags::CIPHERTEXT, ciphertext, &mut serialized);
        debug_assert_eq!(serialized.len(), 1 + proto_len);

        let mac = Self::compute_mac(
            sender_identity_key,
            receiver_identity_key,
            mac_key,
            &serialized,
        )?;
        serialized.extend_from_slice(&mac);
        Ok(Self {
            message_version,
            sender_ratchet_key,
            counter,
            storage: SignalStorage {
                serialized: Bytes::from(serialized),
                ciphertext_range,
            },
        })
    }

    #[inline]
    pub fn message_version(&self) -> u8 {
        self.message_version
    }

    #[inline]
    pub fn sender_ratchet_key(&self) -> &PublicKey {
        &self.sender_ratchet_key
    }

    #[inline]
    pub fn counter(&self) -> u32 {
        self.counter
    }

    #[inline]
    pub fn serialized(&self) -> &[u8] {
        self.storage.serialized()
    }

    #[inline]
    pub fn into_serialized(self) -> Box<[u8]> {
        self.storage.into_boxed_slice()
    }

    #[inline]
    pub fn body(&self) -> Result<&[u8]> {
        Ok(self.storage.ciphertext())
    }

    #[inline]
    fn decode_view_from(serialized: &[u8]) -> Result<waproto::whatsapp::SignalMessageView<'_>> {
        let proto_bytes = &serialized[1..serialized.len() - Self::MAC_LENGTH];
        waproto::whatsapp::SignalMessageView::decode_view(proto_bytes)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)
    }

    fn ciphertext_range_from(
        serialized: &[u8],
        view: &waproto::whatsapp::SignalMessageView<'_>,
    ) -> Result<Range<usize>> {
        let ciphertext = view
            .ciphertext
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        subslice_range(serialized, ciphertext).ok_or(SignalProtocolError::InvalidProtobufEncoding)
    }

    fn rebase_serialized(&mut self, serialized: Bytes) {
        let ciphertext_range = self.storage.ciphertext_range.clone();
        debug_assert_eq!(self.storage.serialized(), serialized.as_ref());
        self.storage = SignalStorage {
            serialized,
            ciphertext_range,
        };
    }

    #[inline]
    fn from_parsed(serialized: Bytes, parsed: ParsedSignalMessage) -> Self {
        Self {
            message_version: parsed.message_version,
            sender_ratchet_key: parsed.sender_ratchet_key,
            counter: parsed.counter,
            storage: SignalStorage {
                serialized,
                ciphertext_range: parsed.ciphertext_range,
            },
        }
    }

    #[inline]
    fn try_from_borrowed(value: &[u8]) -> Result<Self> {
        // Validate before taking ownership. Besides avoiding wasted work for
        // malformed input, this preserves the allocation-free rejection path
        // used when callers probe a ciphertext's Signal envelope type.
        let parsed = parse_signal_message!(value);
        Ok(Self::from_parsed(bytes_from_slice(value), parsed))
    }

    #[inline]
    fn try_from_shared(serialized: Bytes) -> Result<Self> {
        let parsed = parse_signal_message!(&serialized);
        Ok(Self::from_parsed(serialized, parsed))
    }

    pub fn verify_mac(
        &self,
        sender_identity_key: &IdentityKey,
        receiver_identity_key: &IdentityKey,
        mac_key: &[u8],
    ) -> Result<bool> {
        let serialized = self.storage.serialized();
        let our_mac = &Self::compute_mac(
            sender_identity_key,
            receiver_identity_key,
            mac_key,
            &serialized[..serialized.len() - Self::MAC_LENGTH],
        )?;
        let their_mac = &serialized[serialized.len() - Self::MAC_LENGTH..];
        let result: bool = our_mac.ct_eq(their_mac).into();
        if !result {
            // A warning instead of an error because we try multiple sessions.
            log::warn!(
                "Bad Mac! Their Mac: {} Our Mac: {}",
                hex::encode(their_mac),
                hex::encode(our_mac)
            );
        }
        Ok(result)
    }

    fn compute_mac(
        sender_identity_key: &IdentityKey,
        receiver_identity_key: &IdentityKey,
        mac_key: &[u8],
        message: &[u8],
    ) -> Result<[u8; Self::MAC_LENGTH]> {
        if mac_key.len() != 32 {
            return Err(SignalProtocolError::InvalidMacKeyLength(mac_key.len()));
        }
        let mut mac = Hmac::<Sha256>::new_from_slice(mac_key)
            .expect("HMAC-SHA256 should accept any size key");

        mac.update(sender_identity_key.public_key().serialize().as_ref());
        mac.update(receiver_identity_key.public_key().serialize().as_ref());
        mac.update(message);
        let mut result = [0u8; Self::MAC_LENGTH];
        result.copy_from_slice(&mac.finalize().into_bytes()[..Self::MAC_LENGTH]);
        Ok(result)
    }
}

impl AsRef<[u8]> for SignalMessage {
    fn as_ref(&self) -> &[u8] {
        self.storage.serialized()
    }
}

impl TryFrom<&[u8]> for SignalMessage {
    type Error = SignalProtocolError;

    fn try_from(value: &[u8]) -> Result<Self> {
        Self::try_from_borrowed(value)
    }
}

impl TryFrom<Bytes> for SignalMessage {
    type Error = SignalProtocolError;

    fn try_from(value: Bytes) -> Result<Self> {
        Self::try_from_shared(value)
    }
}

#[derive(Debug, Clone)]
pub struct PreKeySignalMessage {
    message_version: u8,
    registration_id: u32,
    pre_key_id: Option<PreKeyId>,
    signed_pre_key_id: SignedPreKeyId,
    base_key: PublicKey,
    identity_key: IdentityKey,
    message: SignalMessage,
    serialized: Bytes,
}

impl PreKeySignalMessage {
    #[allow(clippy::disallowed_methods)]
    pub fn new(
        message_version: u8,
        registration_id: u32,
        pre_key_id: Option<PreKeyId>,
        signed_pre_key_id: SignedPreKeyId,
        base_key: PublicKey,
        identity_key: IdentityKey,
        mut message: SignalMessage,
    ) -> Result<Self> {
        use waproto::tags::pre_key_signal_message as tags;

        let pre_key_id_value = pre_key_id.map(Into::<u32>::into);
        let signed_pre_key_id_value: u32 = signed_pre_key_id.into();
        let base_key_bytes = base_key.serialize();
        let identity_key_bytes = identity_key.serialize();
        let nested_message = message.serialized();

        // Follow the generated encoder's ascending-tag order while writing all
        // byte fields directly into the final envelope. This removes three
        // temporary Vec allocations/copies without changing canonical wire
        // output.
        let proto_len = pre_key_id_value
            .map(|id| varint_field_len(tags::PRE_KEY_ID, u64::from(id)))
            .unwrap_or_default()
            + bytes_field_len(tags::BASE_KEY, base_key_bytes.len())
            + bytes_field_len(tags::IDENTITY_KEY, identity_key_bytes.len())
            + bytes_field_len(tags::MESSAGE, nested_message.len())
            + varint_field_len(tags::REGISTRATION_ID, u64::from(registration_id))
            + varint_field_len(tags::SIGNED_PRE_KEY_ID, u64::from(signed_pre_key_id_value));
        let mut serialized = Vec::with_capacity(1 + proto_len);
        serialized.push(encode_version_byte(
            message_version,
            CIPHERTEXT_MESSAGE_CURRENT_VERSION,
        ));
        if let Some(id) = pre_key_id_value {
            push_varint_field(tags::PRE_KEY_ID, u64::from(id), &mut serialized);
        }
        push_bytes_field(tags::BASE_KEY, &base_key_bytes, &mut serialized);
        push_bytes_field(tags::IDENTITY_KEY, &identity_key_bytes, &mut serialized);
        let message_range = push_bytes_field(tags::MESSAGE, nested_message, &mut serialized);
        push_varint_field(
            tags::REGISTRATION_ID,
            u64::from(registration_id),
            &mut serialized,
        );
        push_varint_field(
            tags::SIGNED_PRE_KEY_ID,
            u64::from(signed_pre_key_id_value),
            &mut serialized,
        );
        debug_assert_eq!(serialized.len(), 1 + proto_len);

        let serialized = Bytes::from(serialized);
        // The outer envelope must be contiguous, so it necessarily copied the
        // inner bytes once. Rebase the retained parsed message onto that copy
        // and release its old allocation: steady-state PreKey storage is now a
        // single allocation shared by both handles.
        message.rebase_serialized(serialized.slice(message_range));
        Ok(Self {
            message_version,
            registration_id,
            pre_key_id,
            signed_pre_key_id,
            base_key,
            identity_key,
            message,
            serialized,
        })
    }

    #[inline]
    fn from_parsed(serialized: Bytes, parsed: ParsedPreKeySignalMessage) -> Self {
        let ParsedPreKeySignalMessage {
            message_version,
            registration_id,
            pre_key_id,
            signed_pre_key_id,
            base_key,
            identity_key,
            message_range,
            message,
        } = parsed;
        let message = SignalMessage::from_parsed(serialized.slice(message_range), message);

        Self {
            message_version,
            registration_id,
            pre_key_id,
            signed_pre_key_id,
            base_key,
            identity_key,
            message,
            serialized,
        }
    }

    #[inline]
    pub fn message_version(&self) -> u8 {
        self.message_version
    }

    #[inline]
    pub fn registration_id(&self) -> u32 {
        self.registration_id
    }

    #[inline]
    pub fn pre_key_id(&self) -> Option<PreKeyId> {
        self.pre_key_id
    }

    #[inline]
    pub fn signed_pre_key_id(&self) -> SignedPreKeyId {
        self.signed_pre_key_id
    }

    #[inline]
    pub fn base_key(&self) -> &PublicKey {
        &self.base_key
    }

    #[inline]
    pub fn identity_key(&self) -> &IdentityKey {
        &self.identity_key
    }

    #[inline]
    pub fn message(&self) -> &SignalMessage {
        &self.message
    }

    #[inline]
    pub fn serialized(&self) -> &[u8] {
        self.serialized.as_ref()
    }

    #[inline]
    pub fn into_serialized(self) -> Box<[u8]> {
        let Self {
            message,
            serialized,
            ..
        } = self;
        // Release the nested slice first so a uniquely-owned outer allocation
        // can be recovered by `Bytes -> Vec` without a copy.
        drop(message);
        bytes_into_boxed_slice(serialized)
    }
}

impl AsRef<[u8]> for PreKeySignalMessage {
    fn as_ref(&self) -> &[u8] {
        self.serialized.as_ref()
    }
}

impl TryFrom<&[u8]> for PreKeySignalMessage {
    type Error = SignalProtocolError;

    fn try_from(value: &[u8]) -> Result<Self> {
        let parsed = parse_pre_key_signal_message!(value);
        Ok(Self::from_parsed(bytes_from_slice(value), parsed))
    }
}

impl TryFrom<Bytes> for PreKeySignalMessage {
    type Error = SignalProtocolError;

    fn try_from(value: Bytes) -> Result<Self> {
        let parsed = parse_pre_key_signal_message!(&value);
        Ok(Self::from_parsed(value, parsed))
    }
}

#[derive(Debug)]
pub struct SenderKeyMessage {
    message_version: u8,
    chain_id: u32,
    iteration: u32,
    serialized: Box<[u8]>,
    // Ciphertext is cached after the first parse to avoid re-decoding.
    ciphertext_cache: OnceLock<Box<[u8]>>,
}

impl Clone for SenderKeyMessage {
    fn clone(&self) -> Self {
        let ciphertext_cache = OnceLock::new();
        if let Some(ciphertext) = self.ciphertext_cache.get() {
            let _ = ciphertext_cache.set(ciphertext.clone());
        }

        Self {
            message_version: self.message_version,
            chain_id: self.chain_id,
            iteration: self.iteration,
            serialized: self.serialized.clone(),
            ciphertext_cache,
        }
    }
}

impl SenderKeyMessage {
    const SIGNATURE_LEN: usize = 64;

    #[allow(clippy::disallowed_methods)]
    pub fn new<R: CryptoRng + Rng>(
        message_version: u8,
        chain_id: u32,
        iteration: u32,
        ciphertext: Box<[u8]>,
        csprng: &mut R,
        signature_key: &PrivateKey,
    ) -> Result<Self> {
        let proto_message = waproto::whatsapp::SenderKeyMessage {
            id: Some(chain_id),
            iteration: Some(iteration),
            ciphertext: Some(ciphertext.into_vec()),
        };

        // Build serialized buffer directly: [version_byte || proto || signature]
        // Sign over [version_byte || proto], then append signature
        let shifted_version = (message_version << 4) | 3u8;
        let mut size_cache = buffa::SizeCache::new();
        let proto_len = proto_message.compute_size(&mut size_cache) as usize;
        let mut serialized = Vec::with_capacity(1 + proto_len + Self::SIGNATURE_LEN);
        serialized.push(shifted_version);
        proto_message.write_to(&mut size_cache, &mut serialized);

        // Sign the data we've built so far (version + proto)
        let signature = signature_key
            .calculate_signature(&serialized, csprng)
            .map_err(|_| SignalProtocolError::SignatureValidationFailed)?;
        serialized.extend_from_slice(&signature);

        Ok(Self {
            message_version,
            chain_id,
            iteration,
            serialized: serialized.into_boxed_slice(),
            ciphertext_cache: OnceLock::new(),
        })
    }

    pub fn verify_signature(&self, signature_key: &PublicKey) -> Result<bool> {
        let valid = signature_key.verify_signature(
            &self.serialized[..self.serialized.len() - Self::SIGNATURE_LEN],
            &self.serialized[self.serialized.len() - Self::SIGNATURE_LEN..],
        );

        Ok(valid)
    }

    /// Like [`Self::verify_signature`], against a cached verifier: the
    /// per-key Edwards derivations are reused across messages instead of
    /// recomputed per signature.
    pub fn verify_signature_prepared(
        &self,
        signature_key: &crate::core::curve::PreparedVerifyingKey,
    ) -> Result<bool> {
        let valid = signature_key.verify_signature(
            &self.serialized[..self.serialized.len() - Self::SIGNATURE_LEN],
            &self.serialized[self.serialized.len() - Self::SIGNATURE_LEN..],
        );

        Ok(valid)
    }

    #[inline]
    pub fn message_version(&self) -> u8 {
        self.message_version
    }

    #[inline]
    pub fn chain_id(&self) -> u32 {
        self.chain_id
    }

    #[inline]
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    /// Returns the ciphertext, parsing and caching it on first access.
    ///
    /// The ciphertext is extracted from the protobuf-encoded `serialized` bytes
    /// and cached to avoid repeated parsing.
    ///
    /// # Performance Note
    ///
    /// Callers should avoid calling this in hot loops when possible.
    pub fn ciphertext(&self) -> Result<&[u8]> {
        get_or_try_init_bytes(&self.ciphertext_cache, || self.decode_ciphertext())
    }

    fn decode_ciphertext(&self) -> Result<Box<[u8]>> {
        // serialized layout: [version_byte || protobuf || signature]
        let proto_bytes = &self.serialized[1..self.serialized.len() - Self::SIGNATURE_LEN];
        let view = waproto::whatsapp::SenderKeyMessageView::decode_view(proto_bytes)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        match view.ciphertext {
            Some(ciphertext) => Ok(Box::from(ciphertext)),
            None => Err(SignalProtocolError::InvalidProtobufEncoding),
        }
    }

    #[inline]
    pub fn serialized(&self) -> &[u8] {
        &self.serialized
    }

    #[inline]
    pub fn into_serialized(self) -> Box<[u8]> {
        self.serialized
    }
}

impl AsRef<[u8]> for SenderKeyMessage {
    fn as_ref(&self) -> &[u8] {
        &self.serialized
    }
}

impl TryFrom<&[u8]> for SenderKeyMessage {
    type Error = SignalProtocolError;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.len() < 1 + Self::SIGNATURE_LEN {
            return Err(SignalProtocolError::CiphertextMessageTooShort(value.len()));
        }
        let message_version = value[0] >> 4;
        if message_version < SENDERKEY_MESSAGE_CURRENT_VERSION {
            return Err(SignalProtocolError::LegacyCiphertextVersion(
                message_version,
            ));
        }
        if message_version > SENDERKEY_MESSAGE_CURRENT_VERSION {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(
                message_version,
            ));
        }
        let view = waproto::whatsapp::SenderKeyMessageView::decode_view(
            &value[1..value.len() - Self::SIGNATURE_LEN],
        )
        .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;

        let Some(chain_id) = view.id else {
            return Err(SignalProtocolError::InvalidProtobufEncoding);
        };
        let Some(iteration) = view.iteration else {
            return Err(SignalProtocolError::InvalidProtobufEncoding);
        };
        let Some(ciphertext) = view.ciphertext else {
            return Err(SignalProtocolError::InvalidProtobufEncoding);
        };
        let ciphertext: Box<[u8]> = Box::from(ciphertext);

        let ciphertext_cache = OnceLock::new();
        let _ = ciphertext_cache.set(ciphertext);

        Ok(SenderKeyMessage {
            message_version,
            chain_id,
            iteration,
            serialized: Box::from(value),
            ciphertext_cache,
        })
    }
}

#[derive(Debug, Clone)]
pub struct SenderKeyDistributionMessage {
    message_version: u8,
    chain_id: u32,
    iteration: u32,
    chain_key: [u8; 32],
    signing_key: PublicKey,
    serialized: Box<[u8]>,
}

impl SenderKeyDistributionMessage {
    #[allow(clippy::disallowed_methods)]
    pub fn new(
        message_version: u8,
        chain_id: u32,
        iteration: u32,
        chain_key: [u8; 32],
        signing_key: PublicKey,
    ) -> Result<Self> {
        let proto_message = waproto::whatsapp::SenderKeyDistributionMessage {
            id: Some(chain_id),
            iteration: Some(iteration),
            chain_key: Some(chain_key.to_vec()),
            signing_key: Some(signing_key.serialize().to_vec()),
        };
        let mut size_cache = buffa::SizeCache::new();
        let message_len = proto_message.compute_size(&mut size_cache) as usize;
        let mut serialized = Vec::with_capacity(1 + message_len);
        serialized.push(((message_version & 0xF) << 4) | SENDERKEY_MESSAGE_CURRENT_VERSION);
        proto_message.write_to(&mut size_cache, &mut serialized);

        Ok(Self {
            message_version,
            chain_id,
            iteration,
            chain_key,
            signing_key,
            serialized: serialized.into_boxed_slice(),
        })
    }

    #[inline]
    pub fn message_version(&self) -> u8 {
        self.message_version
    }

    #[inline]
    pub fn chain_id(&self) -> u32 {
        self.chain_id
    }

    #[inline]
    pub fn iteration(&self) -> u32 {
        self.iteration
    }

    #[inline]
    pub fn chain_key(&self) -> &[u8; 32] {
        &self.chain_key
    }

    #[inline]
    pub fn signing_key(&self) -> &PublicKey {
        &self.signing_key
    }

    #[inline]
    pub fn serialized(&self) -> &[u8] {
        &self.serialized
    }

    #[inline]
    pub fn into_serialized(self) -> Box<[u8]> {
        self.serialized
    }
}

impl AsRef<[u8]> for SenderKeyDistributionMessage {
    fn as_ref(&self) -> &[u8] {
        &self.serialized
    }
}

impl TryFrom<&[u8]> for SenderKeyDistributionMessage {
    type Error = SignalProtocolError;

    fn try_from(value: &[u8]) -> Result<Self> {
        // The message contains at least a X25519 key and a chain key
        if value.len() < 1 + 32 + 32 {
            return Err(SignalProtocolError::CiphertextMessageTooShort(value.len()));
        }

        let message_version = value[0] >> 4;

        if message_version < SENDERKEY_MESSAGE_CURRENT_VERSION {
            return Err(SignalProtocolError::LegacyCiphertextVersion(
                message_version,
            ));
        }
        if message_version > SENDERKEY_MESSAGE_CURRENT_VERSION {
            return Err(SignalProtocolError::UnrecognizedCiphertextVersion(
                message_version,
            ));
        }

        let view = waproto::whatsapp::SenderKeyDistributionMessageView::decode_view(&value[1..])
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;

        let chain_id = view
            .id
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let iteration = view
            .iteration
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let chain_key_bytes = view
            .chain_key
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let signing_key = view
            .signing_key
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;

        if chain_key_bytes.len() != 32 || signing_key.len() != 33 {
            return Err(SignalProtocolError::InvalidProtobufEncoding);
        }

        let chain_key: [u8; 32] = chain_key_bytes
            .try_into()
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        let signing_key = PublicKey::deserialize(signing_key)?;

        Ok(SenderKeyDistributionMessage {
            message_version,
            chain_id,
            iteration,
            chain_key,
            signing_key,
            serialized: Box::from(value),
        })
    }
}

#[derive(Debug, Clone)]
pub struct PlaintextContent {
    serialized: Box<[u8]>,
}

impl PlaintextContent {
    /// Identifies a serialized PlaintextContent.
    ///
    /// This ensures someone doesn't try to serialize an arbitrary Content message as
    /// PlaintextContent; only messages that are okay to send as plaintext should be allowed.
    const PLAINTEXT_CONTEXT_IDENTIFIER_BYTE: u8 = 0xC0;

    #[inline]
    pub fn body(&self) -> &[u8] {
        &self.serialized[1..]
    }

    #[inline]
    pub fn serialized(&self) -> &[u8] {
        &self.serialized
    }
}

#[derive(Clone, PartialEq, Default)]
pub struct DecryptionErrorMessageProto {
    pub ratchet_key: Option<Vec<u8>>,
    pub timestamp: Option<u64>,
    pub device_id: Option<u32>,
}

impl buffa::DefaultInstance for DecryptionErrorMessageProto {
    fn default_instance() -> &'static Self {
        static VALUE: buffa::__private::OnceBox<DecryptionErrorMessageProto> =
            buffa::__private::OnceBox::new();
        VALUE.get_or_init(|| Box::new(DecryptionErrorMessageProto::default()))
    }
}

impl buffa::Message for DecryptionErrorMessageProto {
    fn compute_size(&self, _cache: &mut buffa::SizeCache) -> u32 {
        let mut size = 0u32;
        if let Some(ref v) = self.ratchet_key {
            size += 1 + buffa::types::bytes_encoded_len(v) as u32;
        }
        if let Some(v) = self.timestamp {
            size += 1 + buffa::types::uint64_encoded_len(v) as u32;
        }
        if let Some(v) = self.device_id {
            size += 1 + buffa::types::uint32_encoded_len(v) as u32;
        }
        size
    }

    fn write_to(&self, _cache: &mut buffa::SizeCache, buf: &mut impl buffa::EncodeSink) {
        if let Some(ref v) = self.ratchet_key {
            buffa::encoding::Tag::new(1, buffa::encoding::WireType::LengthDelimited).encode(buf);
            buffa::types::encode_bytes(v, buf);
        }
        if let Some(v) = self.timestamp {
            buffa::encoding::Tag::new(2, buffa::encoding::WireType::Varint).encode(buf);
            buffa::types::encode_uint64(v, buf);
        }
        if let Some(v) = self.device_id {
            buffa::encoding::Tag::new(3, buffa::encoding::WireType::Varint).encode(buf);
            buffa::types::encode_uint32(v, buf);
        }
    }

    fn merge_field(
        &mut self,
        tag: buffa::encoding::Tag,
        buf: &mut impl buffa::bytes::Buf,
        ctx: buffa::DecodeContext<'_>,
    ) -> core::result::Result<(), buffa::DecodeError> {
        use buffa::encoding::WireType;
        // Validate wire type per field; a mismatch falls through to skip_field
        // instead of mis-decoding peer input.
        match tag.field_number() {
            1 if tag.wire_type() == WireType::LengthDelimited => {
                buffa::types::merge_bytes(self.ratchet_key.get_or_insert_with(Vec::new), buf)?;
            }
            2 if tag.wire_type() == WireType::Varint => {
                self.timestamp = Some(buffa::types::decode_uint64(buf)?);
            }
            3 if tag.wire_type() == WireType::Varint => {
                self.device_id = Some(buffa::types::decode_uint32(buf)?);
            }
            _ => {
                // Thread the live recursion budget through: a bare skip_field
                // restarts it at RECURSION_LIMIT, which unknown group fields
                // could exploit for depth-doubling.
                buffa::encoding::skip_field_depth(tag, buf, ctx.depth())?;
            }
        }
        Ok(())
    }

    fn clear(&mut self) {
        self.ratchet_key = None;
        self.timestamp = None;
        self.device_id = None;
    }
}

impl TryFrom<&[u8]> for PlaintextContent {
    type Error = SignalProtocolError;

    fn try_from(value: &[u8]) -> Result<Self> {
        if value.is_empty() {
            return Err(SignalProtocolError::CiphertextMessageTooShort(0));
        }
        if value[0] != Self::PLAINTEXT_CONTEXT_IDENTIFIER_BYTE {
            return Err(SignalProtocolError::UnrecognizedMessageVersion(
                value[0] as u32,
            ));
        }
        Ok(Self {
            serialized: Box::from(value),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DecryptionErrorMessage {
    ratchet_key: Option<PublicKey>,
    timestamp: Timestamp,
    device_id: u32,
    serialized: Box<[u8]>,
}

impl DecryptionErrorMessage {
    #[allow(clippy::disallowed_methods)]
    pub fn for_original(
        original_bytes: &[u8],
        original_type: CiphertextMessageType,
        original_timestamp: Timestamp,
        original_sender_device_id: u32,
    ) -> Result<Self> {
        let ratchet_key = match original_type {
            CiphertextMessageType::Whisper => {
                Some(*SignalMessage::try_from(original_bytes)?.sender_ratchet_key())
            }
            CiphertextMessageType::PreKey => Some(
                *PreKeySignalMessage::try_from(original_bytes)?
                    .message()
                    .sender_ratchet_key(),
            ),
            CiphertextMessageType::SenderKey => None,
            CiphertextMessageType::Plaintext => {
                return Err(SignalProtocolError::InvalidArgument(
                    "cannot create a DecryptionErrorMessage for plaintext content; it is not encrypted".to_string()
                ));
            }
        };

        let proto_message = DecryptionErrorMessageProto {
            timestamp: Some(original_timestamp.epoch_millis()),
            ratchet_key: ratchet_key.map(|k| k.serialize().into()),
            device_id: Some(original_sender_device_id),
        };
        let serialized = proto_message.encode_to_vec();

        Ok(Self {
            ratchet_key,
            timestamp: original_timestamp,
            device_id: original_sender_device_id,
            serialized: serialized.into_boxed_slice(),
        })
    }

    #[inline]
    pub fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    #[inline]
    pub fn ratchet_key(&self) -> Option<&PublicKey> {
        self.ratchet_key.as_ref()
    }

    #[inline]
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    #[inline]
    pub fn serialized(&self) -> &[u8] {
        &self.serialized
    }
}

impl TryFrom<&[u8]> for DecryptionErrorMessage {
    type Error = SignalProtocolError;

    #[allow(clippy::disallowed_methods)]
    fn try_from(value: &[u8]) -> Result<Self> {
        let proto_structure = DecryptionErrorMessageProto::decode_from_slice(value)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        let timestamp = proto_structure
            .timestamp
            .map(Timestamp::from_epoch_millis)
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
        let ratchet_key = proto_structure
            .ratchet_key
            .map(|k| PublicKey::deserialize(&k))
            .transpose()?;
        let device_id = proto_structure.device_id.unwrap_or_default();
        Ok(Self {
            timestamp,
            ratchet_key,
            device_id,
            serialized: Box::from(value),
        })
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    const TEST_MESSAGE_VERSION: u8 = CIPHERTEXT_MESSAGE_CURRENT_VERSION;
    const TEST_MAC_KEY: [u8; 32] = [0xA5; 32];
    const TEST_RATCHET_SEED: u8 = 0x11;
    const TEST_SENDER_IDENTITY_SEED: u8 = 0x22;
    const TEST_RECEIVER_IDENTITY_SEED: u8 = 0x33;
    const TEST_BASE_KEY_SEED: u8 = 0x44;

    fn public_key_from_seed(seed: u8) -> PublicKey {
        PrivateKey::deserialize(&[seed; 32])
            .expect("test private key")
            .public_key()
            .expect("test public key")
    }

    fn test_signal_message(
        counter: u32,
        previous_counter: u32,
        ciphertext: &[u8],
    ) -> SignalMessage {
        SignalMessage::new(
            TEST_MESSAGE_VERSION,
            &TEST_MAC_KEY,
            public_key_from_seed(TEST_RATCHET_SEED),
            counter,
            previous_counter,
            ciphertext,
            &IdentityKey::new(public_key_from_seed(TEST_SENDER_IDENTITY_SEED)),
            &IdentityKey::new(public_key_from_seed(TEST_RECEIVER_IDENTITY_SEED)),
        )
        .expect("test SignalMessage")
    }

    fn generated_signal_wire(counter: u32, previous_counter: u32, ciphertext: &[u8]) -> Vec<u8> {
        let sender_identity = IdentityKey::new(public_key_from_seed(TEST_SENDER_IDENTITY_SEED));
        let receiver_identity = IdentityKey::new(public_key_from_seed(TEST_RECEIVER_IDENTITY_SEED));
        let proto = waproto::whatsapp::SignalMessage {
            ratchet_key: Some(public_key_from_seed(TEST_RATCHET_SEED).serialize().to_vec()),
            counter: Some(counter),
            previous_counter: Some(previous_counter),
            ciphertext: Some(ciphertext.to_vec()),
        };
        let mut wire =
            Vec::with_capacity(1 + proto.encoded_len() as usize + SignalMessage::MAC_LENGTH);
        wire.push(encode_version_byte(
            TEST_MESSAGE_VERSION,
            CIPHERTEXT_MESSAGE_CURRENT_VERSION,
        ));
        wire.extend_from_slice(&proto.encode_to_vec());
        let mac =
            SignalMessage::compute_mac(&sender_identity, &receiver_identity, &TEST_MAC_KEY, &wire)
                .expect("test MAC");
        wire.extend_from_slice(&mac);
        wire
    }

    fn generated_pre_key_wire(
        registration_id: u32,
        pre_key_id: Option<PreKeyId>,
        signed_pre_key_id: SignedPreKeyId,
        nested_message: &[u8],
    ) -> Vec<u8> {
        let proto = waproto::whatsapp::PreKeySignalMessage {
            registration_id: Some(registration_id),
            pre_key_id: pre_key_id.map(Into::into),
            signed_pre_key_id: Some(signed_pre_key_id.into()),
            base_key: Some(
                public_key_from_seed(TEST_BASE_KEY_SEED)
                    .serialize()
                    .to_vec(),
            ),
            identity_key: Some(
                public_key_from_seed(TEST_SENDER_IDENTITY_SEED)
                    .serialize()
                    .to_vec(),
            ),
            message: Some(nested_message.to_vec()),
            kyber_pre_key_id: None,
            kyber_ciphertext: None,
        };
        let mut wire = Vec::with_capacity(1 + proto.encoded_len() as usize);
        wire.push(encode_version_byte(
            TEST_MESSAGE_VERSION,
            CIPHERTEXT_MESSAGE_CURRENT_VERSION,
        ));
        wire.extend_from_slice(&proto.encode_to_vec());
        wire
    }

    #[test]
    fn direct_signal_encoder_matches_generated_wire() {
        let counters = [0, 127, 128, u32::MAX];
        let ciphertexts = [Vec::new(), vec![0x5A], vec![0xC3; 128]];

        for counter in counters {
            for previous_counter in counters {
                for ciphertext in &ciphertexts {
                    let direct = test_signal_message(counter, previous_counter, ciphertext);
                    let generated = generated_signal_wire(counter, previous_counter, ciphertext);
                    assert_eq!(direct.serialized(), generated);
                }
            }
        }
    }

    #[test]
    fn direct_pre_key_encoder_matches_generated_wire() {
        let signal_wire = generated_signal_wire(128, 127, &[0x7E; 64]);
        let cases = [
            (0, None, 0),
            (127, Some(127), 127),
            (128, Some(128), 128),
            (u32::MAX, Some(u32::MAX), u32::MAX),
        ];

        for (registration_id, pre_key_id, signed_pre_key_id) in cases {
            let pre_key_id = pre_key_id.map(Into::into);
            let signed_pre_key_id = signed_pre_key_id.into();
            let direct = PreKeySignalMessage::new(
                TEST_MESSAGE_VERSION,
                registration_id,
                pre_key_id,
                signed_pre_key_id,
                public_key_from_seed(TEST_BASE_KEY_SEED),
                IdentityKey::new(public_key_from_seed(TEST_SENDER_IDENTITY_SEED)),
                SignalMessage::try_from(signal_wire.as_slice()).expect("nested SignalMessage"),
            )
            .expect("test PreKeySignalMessage");
            let generated = generated_pre_key_wire(
                registration_id,
                pre_key_id,
                signed_pre_key_id,
                &signal_wire,
            );
            assert_eq!(direct.serialized(), generated);
        }
    }

    #[test]
    fn parsed_signal_body_shares_its_wire_allocation() {
        let wire = test_signal_message(7, 3, &[0x42; 256]).into_serialized();

        let borrowed = SignalMessage::try_from(wire.as_ref()).expect("borrowed SignalMessage");
        assert!(
            subslice_range(
                borrowed.serialized(),
                borrowed.body().expect("borrowed Signal body"),
            )
            .is_some()
        );

        let transport = Bytes::from(wire);
        let transport_ptr = transport.as_ptr();
        let shared = SignalMessage::try_from(transport).expect("shared SignalMessage");
        assert_eq!(shared.serialized().as_ptr(), transport_ptr);
        assert!(
            subslice_range(
                shared.serialized(),
                shared.body().expect("shared Signal body"),
            )
            .is_some()
        );
    }

    #[test]
    fn parsed_pre_key_and_nested_signal_share_one_wire_allocation() {
        let signal_wire = generated_signal_wire(9, 4, &[0x24; 256]);
        let wire = PreKeySignalMessage::new(
            TEST_MESSAGE_VERSION,
            128,
            Some(127.into()),
            129.into(),
            public_key_from_seed(TEST_BASE_KEY_SEED),
            IdentityKey::new(public_key_from_seed(TEST_SENDER_IDENTITY_SEED)),
            SignalMessage::try_from(signal_wire.as_slice()).expect("nested SignalMessage"),
        )
        .expect("test PreKeySignalMessage")
        .into_serialized();

        let borrowed =
            PreKeySignalMessage::try_from(wire.as_ref()).expect("borrowed PreKeySignalMessage");
        assert!(subslice_range(borrowed.serialized(), borrowed.message().serialized()).is_some());
        assert!(
            subslice_range(
                borrowed.serialized(),
                borrowed
                    .message()
                    .body()
                    .expect("borrowed nested Signal body"),
            )
            .is_some()
        );

        let transport = Bytes::from(wire);
        let transport_ptr = transport.as_ptr();
        let shared = PreKeySignalMessage::try_from(transport).expect("shared PreKeySignalMessage");
        assert_eq!(shared.serialized().as_ptr(), transport_ptr);
        assert!(subslice_range(shared.serialized(), shared.message().serialized()).is_some());
        assert!(
            subslice_range(
                shared.serialized(),
                shared.message().body().expect("shared nested Signal body"),
            )
            .is_some()
        );
    }

    #[test]
    fn consuming_unique_encoded_messages_reuses_their_wire_allocations() {
        let signal = test_signal_message(1, 0, b"signal");
        let signal_ptr = signal.serialized().as_ptr();
        let signal_wire = signal.into_serialized();
        assert_eq!(signal_wire.as_ptr(), signal_ptr);

        let pre_key = PreKeySignalMessage::new(
            TEST_MESSAGE_VERSION,
            1,
            Some(1.into()),
            1.into(),
            public_key_from_seed(TEST_BASE_KEY_SEED),
            IdentityKey::new(public_key_from_seed(TEST_SENDER_IDENTITY_SEED)),
            SignalMessage::try_from(signal_wire.as_ref()).expect("nested SignalMessage"),
        )
        .expect("test PreKeySignalMessage");
        let pre_key_ptr = pre_key.serialized().as_ptr();
        let pre_key_wire = pre_key.into_serialized();
        assert_eq!(pre_key_wire.as_ptr(), pre_key_ptr);
    }

    #[test]
    fn decryption_error_proto_uses_buffa_size_cache_encoding() {
        let proto = DecryptionErrorMessageProto {
            ratchet_key: Some(vec![1, 2, 3]),
            timestamp: Some(150),
            device_id: Some(7),
        };

        let bytes = proto.encode_to_vec();
        assert_eq!(bytes, [0x0a, 3, 1, 2, 3, 0x10, 0x96, 0x01, 0x18, 7]);
        assert_eq!(proto.encoded_len() as usize, bytes.len());

        let decoded =
            DecryptionErrorMessageProto::decode_from_slice(&bytes).expect("decode test proto");
        assert_eq!(decoded.ratchet_key.as_deref(), Some(&[1, 2, 3][..]));
        assert_eq!(decoded.timestamp, Some(150));
        assert_eq!(decoded.device_id, Some(7));
    }
}
