use crate::client::Client;
use crate::types::events::Event;
use crate::types::message::MessageInfo;
use log::{debug, warn};
use prost::Message as ProtoMessage;

use std::sync::Arc;
use wacore::libsignal::crypto::DecryptionError;
use wacore::libsignal::protocol::SenderKeyDistributionMessage;
use wacore::libsignal::protocol::group_decrypt;
use wacore::libsignal::protocol::process_sender_key_distribution_message;
use wacore::libsignal::protocol::{
    IdentityChange, PreKeySignalMessage, SignalMessage, SignalProtocolError, UsePQRatchet,
    message_decrypt,
};
use wacore::libsignal::protocol::{
    PublicKey as SignalPublicKey, SENDERKEY_MESSAGE_CURRENT_VERSION,
};
use wacore::message_processing::EncType;
use wacore::protocol::nack::NackReason;
use wacore::types::jid::{JidExt, make_sender_key_name};
use wacore_binary::Jid;
use wacore_binary::JidExt as _;
use wacore_binary::{NodeRef, OwnedNodeRef};
use waproto::whatsapp::{self as wa};

/// Maximum retry attempts per message (matches WhatsApp Web's MAX_RETRY = 5).
/// After this many retries, we stop sending retry receipts and rely solely on PDO.
const MAX_DECRYPT_RETRIES: u8 = 5;

/// Pre-extracted enc node payload. Holds owned copies of the fields needed for
/// decryption so the async decrypt phase doesn't borrow the original NodeRef tree.
pub(crate) struct EncPayload {
    pub ciphertext: bytes::Bytes,
    pub enc_type: EncType,
    pub padding_version: u8,
}

impl EncPayload {
    fn from_parts(ciphertext: bytes::Bytes, enc_node: &NodeRef<'_>) -> Option<Self> {
        let enc_type = EncType::from_wire(enc_node.attrs().optional_string("type")?.as_ref())?;
        let padding_version = enc_node.attrs().optional_u64("v").unwrap_or(2) as u8;
        Some(Self {
            ciphertext,
            enc_type,
            padding_version,
        })
    }

    /// Zero-copy extraction from an OwnedNodeRef.
    pub(crate) fn from_owned_node(owner: &OwnedNodeRef, enc_node: &NodeRef<'_>) -> Option<Self> {
        Self::from_parts(owner.slice_bytes(enc_node.content_bytes()?), enc_node)
    }

    /// Copying extraction from a NodeRef (used in tests where there's no OwnedNodeRef).
    #[cfg(test)]
    pub(crate) fn from_node_ref(node: &NodeRef<'_>) -> Option<Self> {
        Self::from_parts(bytes::Bytes::copy_from_slice(node.content_bytes()?), node)
    }
}

/// Parsed and classified message ready for decryption. All data is owned --
/// the original node tree is no longer borrowed.
pub(crate) struct ClassifiedMessage {
    pub info: Arc<MessageInfo>,
    pub sender_encryption_jid: Jid,
    pub session_payloads: Vec<EncPayload>,
    pub group_payloads: Vec<EncPayload>,
    pub bot_payloads: Vec<EncPayload>,
    pub max_sender_retry_count: u8,
    pub decrypt_fail_mode: crate::types::events::DecryptFailMode,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SessionBatchOutcome {
    decrypted: bool,
    duplicate: bool,
    undecryptable: bool,
    dispatched: bool,
    skdm_only: bool,
    plaintext_failed: bool,
    had_failure: bool,
}

#[derive(Clone, Copy, Debug, Default)]
struct MigrationDecryptOutcome {
    decrypted: bool,
    duplicate: bool,
    dispatched: bool,
    skdm_only: bool,
    plaintext_failed: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PlaintextHandleOutcome {
    dispatched: bool,
    skdm_only: bool,
}

fn should_process_skmsg_after_session(
    session_payloads_empty: bool,
    session_outcome: SessionBatchOutcome,
) -> bool {
    session_payloads_empty
        || (!session_outcome.had_failure
            && (session_outcome.decrypted || session_outcome.duplicate))
}

fn should_ack_skdm_only_session_fallback(
    session_outcome: SessionBatchOutcome,
    bot_payloads_empty: bool,
) -> bool {
    session_outcome.decrypted
        && session_outcome.skdm_only
        && !session_outcome.dispatched
        && !session_outcome.had_failure
        && !session_outcome.plaintext_failed
        && !session_outcome.undecryptable
        && bot_payloads_empty
}

/// Retry count threshold for logging high retry warnings.
/// WhatsApp Web logs metrics when retry count exceeds this value.
const HIGH_RETRY_COUNT_THRESHOLD: u8 = 3;

/// `decrypt-fail="hide"` failures are expected (addon/fan-out), so log them at
/// DEBUG to avoid WARN spam. Mode never changes control flow: retry + ack still
/// fire (WA Web retries regardless of `hide`).
fn decrypt_fail_log_level(mode: crate::types::events::DecryptFailMode) -> log::Level {
    match mode {
        crate::types::events::DecryptFailMode::Hide => log::Level::Debug,
        crate::types::events::DecryptFailMode::Show => log::Level::Warn,
    }
}

pub(crate) use wacore::protocol::retry::RetryReason;

mod dispatch;
mod durability;
mod msg_secret;
mod receive;
mod retry;
mod special;

/// Unwraps a `DeviceSentMessage` wrapper, returning the inner message with
/// merged `message_context_info`.
///
/// Self-sent messages synced from the primary device arrive with the actual
/// content (reactions, text, etc.) nested inside `device_sent_message.message`.
/// This extracts the inner message when present, merges `MessageContextInfo`
/// from outer and inner following WhatsApp Web's
/// `WAWebDeviceSentMessageProtoUtils.unwrapDeviceSentMessage` logic, or returns
/// the original message unchanged when there is no wrapper or the wrapper has
/// no inner message.
/// Re-export from wacore for backwards compatibility (used by tests via `super::*`).
#[cfg(test)]
fn unwrap_device_sent(msg: wa::Message) -> wa::Message {
    wacore::messages::unwrap_device_sent(msg)
}

/// Re-export from wacore for backwards compatibility (used by tests via `super::*`).
#[cfg(test)]
fn is_sender_key_distribution_only(msg: &mut wa::Message) -> bool {
    wacore::messages::is_sender_key_distribution_only(msg)
}

#[cfg(test)]
mod tests;
