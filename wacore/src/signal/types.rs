// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use super::error::OpsError;
use crate::libsignal::protocol::{DeviceId, IdentityKey, PreKeyBundle, ProtocolAddress, PublicKey};
use crate::message_processing::DecryptedMessageResult;
use crate::types::message::AddressingMode;
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Everything the WhatsApp client needs to register and pair the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicIdentity {
    pub registration_id: u32,
    /// 32-byte Curve25519 public key, without the type byte.
    pub identity_key: [u8; 32],
    pub signed_prekey: PublicSignedPreKey,
    /// Shared with the primary through the QR code; an HMAC key for pairing,
    /// not a decryption key.
    pub adv_secret: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicSignedPreKey {
    pub id: u32,
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicPreKey {
    pub id: u32,
    pub public_key: [u8; 32],
}

/// Pairwise ciphertext kinds, named as in the `<enc type>` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum EncKind {
    /// First message of a session, carries X3DH material.
    #[wire = "pkmsg"]
    PreKey,
    /// A message on an established session.
    #[wire = "msg"]
    Message,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decrypted {
    /// Padded plaintext exactly as libsignal produced it.
    pub plaintext: Vec<u8>,
    /// The sender's identity key differs from the one stored before.
    pub identity_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Encrypted {
    pub kind: EncKind,
    pub ciphertext: Vec<u8>,
}

/// A peer device's prekey bundle, as fetched by the client from the server.
#[derive(Debug, Clone)]
pub struct RemoteBundle {
    pub registration_id: u32,
    pub device_id: u32,
    pub identity_key: [u8; 32],
    pub signed_prekey_id: u32,
    pub signed_prekey: [u8; 32],
    pub signed_prekey_signature: [u8; 64],
    pub prekey: Option<(u32, [u8; 32])>,
    /// The `<device-identity>` (`ADVSignedDeviceIdentity` bytes) the bundle
    /// came with. Required for companion devices; see [`crate::signal::SignalResolver`].
    pub device_identity: Option<Vec<u8>>,
}

impl RemoteBundle {
    /// The libsignal bundle, validating every key.
    pub fn to_prekey_bundle(&self) -> Result<PreKeyBundle, OpsError> {
        let prekey = match self.prekey {
            Some((id, key)) => Some((id.into(), public_key(&key, "prekey")?)),
            None => None,
        };
        Ok(PreKeyBundle::new(
            self.registration_id,
            DeviceId::from(self.device_id),
            prekey,
            self.signed_prekey_id.into(),
            public_key(&self.signed_prekey, "signed prekey")?,
            self.signed_prekey_signature,
            IdentityKey::new(public_key(&self.identity_key, "identity key")?),
        )?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingSignature {
    /// `ADVSignedDeviceIdentity` with the device signature added, ready for the
    /// pair-success response.
    pub signed_identity: Vec<u8>,
    pub key_index: u32,
}

fn public_key(bytes: &[u8; 32], what: &str) -> Result<PublicKey, OpsError> {
    PublicKey::from_djb_public_key_bytes(bytes)
        .map_err(|_| OpsError::InvalidInput(format!("{what} is not a valid public key")))
}

/// The `<enc type>` of a received payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum ReceiveKind {
    #[wire = "pkmsg"]
    PreKey,
    #[wire = "msg"]
    Message,
    /// Group sender-key message for the request's chat.
    #[wire = "skmsg"]
    SenderKey,
}

/// One `<enc>` payload from a `<message>` stanza.
#[derive(Debug, Clone, Copy)]
pub struct ReceiveRequest<'a> {
    /// The stanza's chat JID. For group traffic this is the group.
    pub chat: &'a str,
    /// The sending device's Signal address.
    pub sender: &'a ProtocolAddress,
    pub kind: ReceiveKind,
    pub ciphertext: &'a [u8],
    /// The `<enc v>` padding version.
    pub padding_version: u8,
    /// The client's claim that the sender is one of this account's devices.
    /// The service derives this from the sender and the stored own JIDs; a
    /// claim the sender does not support is refused with `InvalidInput`.
    pub is_from_me: bool,
    /// The stanza's `<device-identity>`, which a prekey message from a
    /// companion device carries. A prekey message whose sender does not
    /// verify against its pinned account is refused before decryption.
    pub device_identity: Option<&'a [u8]>,
}

#[derive(Debug)]
pub enum Received {
    Message(Box<ReceivedMessage>),
    /// Delivered before; the client only needs to send its receipt again.
    AlreadyDelivered {
        receipt_key: String,
    },
}

#[derive(Debug)]
pub struct ReceivedMessage {
    /// Pass to [`super::SignalBackend::mark_delivered`] once the app has the message.
    pub receipt_key: String,
    /// Unpadded content with own-device copies unwrapped, plus its sender-key
    /// distribution and protocol parts.
    pub content: DecryptedMessageResult,
    pub identity_changed: bool,
    /// Served from the buffer after an earlier decrypt was not acknowledged.
    pub redelivered: bool,
    /// The message carried a sender-key distribution that could not be
    /// stored. The rest of the message is still delivered; later group
    /// messages from this sender fail until a valid distribution arrives.
    pub sender_key_rejected: bool,
    /// An app-state key share authenticated as coming from one of this
    /// account's own devices.
    pub app_state_key_share: Option<wa::message::AppStateSyncKeyShare>,
}

/// A finished `<message>` stanza for the client to transmit.
#[derive(Debug, Clone)]
pub struct PreparedSend {
    /// The stanza, marshaled as the binary node format the socket writes.
    pub stanza: Vec<u8>,
    /// Devices that were addressed but got no `<enc>`.
    pub unreached_devices: Vec<Jid>,
    /// Device-list hash of the addressed set, for comparing with the server
    /// ack.
    pub phash: Option<String>,
}

/// A finished group `<message>` stanza.
#[derive(Debug, Clone)]
pub struct PreparedGroupSend {
    pub stanza: Vec<u8>,
    /// Devices this send distributed the sender key to. Once the server acks
    /// the stanza, pass them to [`super::SignalBackend::mark_sender_key_distributed`].
    pub distribution_targets: Vec<Jid>,
    /// The group's sender-key epoch this send distributed under. Pass it with
    /// the acknowledgment, so an acknowledgment that arrives after a rotation
    /// cannot mark devices as holding the replacement key.
    pub distribution_epoch: u64,
    /// Users whose devices were reported unregistered while fetching bundles;
    /// the client should refresh their device lists.
    pub stale_device_users: Vec<String>,
    pub phash: Option<String>,
}

/// Where a retried message goes, matching the client's retransmission routes.
#[derive(Debug, Clone)]
pub enum RetryRoute {
    /// A direct message; `recipient` is the original chat user when the
    /// requester is one of this account's own devices, and `None` otherwise.
    /// Any other value is refused.
    Direct {
        recipient: Option<Jid>,
    },
    Group {
        addressing_mode: AddressingMode,
    },
    BroadcastList,
}

/// A retry receipt the client received for a message this device sent.
#[derive(Debug, Clone)]
pub struct RetryRequest<'a> {
    /// The chat the original went to (the recipient user, group or list).
    pub chat: &'a Jid,
    pub message_id: &'a str,
    /// The device that asked, as addressed on the wire.
    pub requester: &'a Jid,
    /// The address to encrypt to (the requester's LID when known).
    pub encryption_jid: &'a Jid,
    pub route: RetryRoute,
    pub retry_count: u8,
    /// Keys the receipt carried, used to start a fresh session.
    pub bundle: Option<&'a RemoteBundle>,
}

/// Only public key material. Retry the same batch until the upload is acknowledged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreKeyUploadBatch {
    pub batch_id: String,
    pub pre_keys: Vec<PublicPreKey>,
}
