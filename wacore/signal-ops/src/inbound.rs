// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Buffered, idempotent receive for the Signal service.
//!
//! A decrypt advances the ratchet, so the same ciphertext cannot be decrypted
//! twice. Each result is kept in the sealed decrypt buffer, written in the
//! same batch as the ratchet advance, until the app confirms delivery with
//! [`SignalOps::mark_delivered`]. A redelivered ciphertext returns the
//! buffered result; once delivered it returns [`Received::AlreadyDelivered`].

use crate::error::OpsError;
use crate::ops::{EncKind, ServiceStore, SignalOps, chain_key, session_key};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use wacore::libsignal::protocol::ProtocolAddress;
use wacore::message_processing::{DecryptedMessageResult, process_decrypted_plaintext};
use wacore_recordstore::Namespace;
use waproto::whatsapp as wa;

const STATE_PENDING: u8 = 1;
const STATE_DELIVERED: u8 = 2;
const HEADER_LEN: usize = 1 + 1 + 1 + 8;

/// The `<enc type>` of a received payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, wacore::WireEnum)]
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
    /// The sender is one of this account's own devices.
    pub is_from_me: bool,
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
    /// Pass to [`SignalOps::mark_delivered`] once the app has the message.
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
    /// An app-state key share, only when sent by one of this account's own
    /// devices (WA Web's `isMeAccount` gate). The only message content this
    /// crate returns to its caller.
    pub app_state_key_share: Option<wa::message::AppStateSyncKeyShare>,
}

fn receipt_key(request: &ReceiveRequest<'_>) -> String {
    let mut hasher = Sha256::new();
    for part in [
        request.kind.as_str().as_bytes(),
        request.chat.as_bytes(),
        request.sender.as_str().as_bytes(),
    ] {
        hasher.update(part);
        hasher.update([0]);
    }
    hasher.update(request.ciphertext);
    hex::encode(hasher.finalize())
}

fn now_ms() -> u64 {
    u64::try_from(wacore::time::now_utc().timestamp_millis()).unwrap_or(0)
}

/// Flag bits in a buffer entry's third byte.
const FLAG_IDENTITY_CHANGED: u8 = 1;
const FLAG_SENDER_KEY_REJECTED: u8 = 2;

struct BufferEntry {
    state: u8,
    padding_version: u8,
    identity_changed: bool,
    /// Kept so a redelivery reports a rejected distribution the first
    /// response may have lost.
    sender_key_rejected: bool,
    inserted_ms: u64,
    padded_plaintext: Vec<u8>,
}

impl BufferEntry {
    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(HEADER_LEN + self.padded_plaintext.len());
        out.push(self.state);
        out.push(self.padding_version);
        let mut flags = 0;
        if self.identity_changed {
            flags |= FLAG_IDENTITY_CHANGED;
        }
        if self.sender_key_rejected {
            flags |= FLAG_SENDER_KEY_REJECTED;
        }
        out.push(flags);
        out.extend_from_slice(&self.inserted_ms.to_be_bytes());
        out.extend_from_slice(&self.padded_plaintext);
        out
    }

    fn decode(bytes: Vec<u8>) -> Result<Self, OpsError> {
        if bytes.len() < HEADER_LEN {
            return Err(OpsError::InvalidInput(
                "decrypt buffer entry is truncated".into(),
            ));
        }
        let mut inserted = [0u8; 8];
        inserted.copy_from_slice(&bytes[3..HEADER_LEN]);
        Ok(Self {
            state: bytes[0],
            padding_version: bytes[1],
            identity_changed: bytes[2] & FLAG_IDENTITY_CHANGED != 0,
            sender_key_rejected: bytes[2] & FLAG_SENDER_KEY_REJECTED != 0,
            inserted_ms: u64::from_be_bytes(inserted),
            padded_plaintext: bytes[HEADER_LEN..].to_vec(),
        })
    }

    fn delivered(inserted_ms: u64) -> Self {
        Self {
            state: STATE_DELIVERED,
            padding_version: 0,
            identity_changed: false,
            sender_key_rejected: false,
            inserted_ms,
            padded_plaintext: Vec::new(),
        }
    }
}

fn classify(
    padded: &[u8],
    padding_version: u8,
    is_from_me: bool,
) -> Result<DecryptedMessageResult, OpsError> {
    let mut content = process_decrypted_plaintext(padded, padding_version, is_from_me)
        .map_err(|e| OpsError::InvalidInput(format!("plaintext did not decode: {e}")))?;
    if !is_from_me {
        strip_app_state_keys(&mut content);
    }
    Ok(content)
}

/// App-state key shares and requests are only honoured from this account's
/// own devices (WA Web's `isMeAccount` gate). From anyone else they are
/// removed everywhere, so they cannot reach the app through the message.
fn strip_app_state_keys(content: &mut DecryptedMessageResult) {
    if let Some(protocol) = content.message.protocol_message.as_option_mut() {
        protocol.app_state_sync_key_share = Default::default();
        protocol.app_state_sync_key_request = Default::default();
    }
    if let Some(info) = content.protocol_message.as_mut() {
        info.app_state_sync_key_share = None;
    }
}

fn key_share(
    content: &DecryptedMessageResult,
    is_from_me: bool,
) -> Option<wa::message::AppStateSyncKeyShare> {
    if !is_from_me {
        return None;
    }
    content
        .protocol_message
        .as_ref()
        .and_then(|p| p.app_state_sync_key_share.clone())
}

impl<S: ServiceStore> SignalOps<S> {
    /// Decrypt, buffer and classify one received payload. Sender-key
    /// distributions inside it are stored in the same batch as the decrypt.
    pub async fn receive(&self, request: ReceiveRequest<'_>) -> Result<Received, OpsError> {
        let receipt_key = receipt_key(&request);
        // Pairwise payloads can carry a sender-key distribution for this chat,
        // so they also lock the sender's chain.
        let chain = chain_key(request.chat, request.sender.as_str());
        let keys = match request.kind {
            ReceiveKind::SenderKey => vec![chain],
            _ => vec![session_key(request.sender), chain],
        };
        let held = self.lock_keys(keys).await?;
        held.run(async {
            if let Some(bytes) = self
                .store()
                .load_aux(Namespace::DecryptBuffer, &receipt_key)
                .await?
            {
                let entry = BufferEntry::decode(bytes)?;
                if entry.state == STATE_DELIVERED {
                    return Ok(Received::AlreadyDelivered { receipt_key });
                }
                let content = classify(
                    &entry.padded_plaintext,
                    entry.padding_version,
                    request.is_from_me,
                )?;
                return Ok(Received::Message(Box::new(ReceivedMessage {
                    app_state_key_share: key_share(&content, request.is_from_me),
                    receipt_key,
                    content,
                    identity_changed: entry.identity_changed,
                    redelivered: true,
                    sender_key_rejected: entry.sender_key_rejected,
                })));
            }

            let staged = Arc::new(self.store().staged());
            let (padded, identity_changed) = match request.kind {
                ReceiveKind::PreKey | ReceiveKind::Message => {
                    let kind = if request.kind == ReceiveKind::PreKey {
                        EncKind::PreKey
                    } else {
                        EncKind::Message
                    };
                    let decrypted = self
                        .decrypt_into(&staged, request.sender, kind, request.ciphertext)
                        .await?;
                    (decrypted.plaintext, decrypted.identity_changed)
                }
                ReceiveKind::SenderKey => (
                    self.group_decrypt_into(
                        &staged,
                        request.chat,
                        request.sender,
                        request.ciphertext,
                    )
                    .await?,
                    false,
                ),
            };

            let inserted_ms = now_ms();
            let content = match classify(&padded, request.padding_version, request.is_from_me) {
                Ok(content) => content,
                Err(error) => {
                    // The ratchet already advanced; keep that, and record the
                    // ciphertext as handled so redelivery does not loop.
                    staged
                        .put_aux(
                            Namespace::DecryptBuffer,
                            &receipt_key,
                            &BufferEntry::delivered(inserted_ms).encode(),
                        )
                        .await?;
                    staged.commit().await?;
                    return Err(error);
                }
            };

            // A bad distribution must not block the message carrying it: failing
            // here would discard the ratchet advance, and every redelivery would
            // fail the same way. The client logs and continues the same way.
            let mut sender_key_rejected = false;
            if let Some(skdm) = &content.skdm
                && let Some(axolotl) = &skdm.axolotl_sender_key_distribution_message
                && self
                    .process_sender_key_distribution_into(
                        &staged,
                        request.chat,
                        request.sender,
                        axolotl,
                    )
                    .await
                    .is_err()
            {
                sender_key_rejected = true;
            }

            let entry = BufferEntry {
                state: STATE_PENDING,
                padding_version: request.padding_version,
                identity_changed,
                sender_key_rejected,
                inserted_ms,
                padded_plaintext: padded,
            };
            staged
                .put_aux(Namespace::DecryptBuffer, &receipt_key, &entry.encode())
                .await?;
            staged.commit().await?;

            Ok(Received::Message(Box::new(ReceivedMessage {
                app_state_key_share: key_share(&content, request.is_from_me),
                receipt_key,
                content,
                identity_changed,
                redelivered: false,
                sender_key_rejected,
            })))
        })
        .await
    }

    /// The app has the message: drop its plaintext and keep a marker so a
    /// redelivered ciphertext is recognised.
    pub async fn mark_delivered(&self, receipt_key: &str) -> Result<(), OpsError> {
        let store = self.store();
        if let Some(bytes) = store
            .load_aux(Namespace::DecryptBuffer, receipt_key)
            .await?
        {
            let entry = BufferEntry::decode(bytes)?;
            if entry.state != STATE_DELIVERED {
                store
                    .put_aux(
                        Namespace::DecryptBuffer,
                        receipt_key,
                        &BufferEntry::delivered(entry.inserted_ms).encode(),
                    )
                    .await?;
            }
        }
        Ok(())
    }

    /// Delete buffer entries inserted before `cutoff_ms`, delivered or not.
    /// Returns how many were removed.
    pub async fn prune_decrypt_buffer(&self, cutoff_ms: u64) -> Result<usize, OpsError> {
        let store = self.store();
        let mut expired = Vec::new();
        for key in store.scan_aux(Namespace::DecryptBuffer, "").await? {
            // An entry that no longer decodes cannot answer a replay; delete it
            // rather than let one corrupt entry stop every later prune.
            if let Some(bytes) = store.load_aux(Namespace::DecryptBuffer, &key).await?
                && BufferEntry::decode(bytes).map_or(true, |entry| entry.inserted_ms < cutoff_ms)
            {
                expired.push(key);
            }
        }
        let refs: Vec<&str> = expired.iter().map(String::as_str).collect();
        store.delete_aux(Namespace::DecryptBuffer, &refs).await?;
        Ok(expired.len())
    }
}
