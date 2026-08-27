use crate::client::Client;
use crate::types::events::EncDecryptFailureReason;
use crate::types::events::Event;
use crate::types::message::MessageInfo;
use log::{debug, warn};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use wacore::libsignal::crypto::DecryptionError;
use wacore::libsignal::protocol::SenderKeyStore;
use wacore::libsignal::protocol::group_decrypt;
use wacore::libsignal::protocol::{
    CiphertextMessage, DecryptionResult, IdentityChange, OwnedCiphertextMessage,
    PreKeySignalMessage, SignalMessage, SignalProtocolError, UsePQRatchet, message_decrypt,
    message_decrypt_owned,
};
use wacore::message_processing::EncType;
use wacore::protocol::nack::NackReason;
use wacore::types::jid::{JidExt, make_sender_key_name};
use wacore_binary::Jid;
use wacore_binary::JidExt as _;
use wacore_binary::node::ValueRef;
use wacore_binary::{NodeRef, OwnedNodeRef};
use waproto::whatsapp::{self as wa};

use wacore::protocol::retry::MAX_RETRY_COUNT as MAX_DECRYPT_RETRIES;

#[inline]
fn sender_retry_count(enc_node: &NodeRef<'_>) -> u8 {
    enc_node
        .get_attr("count")
        .and_then(|value| match value {
            ValueRef::String(value) => value.parse::<u64>().ok(),
            ValueRef::Jid(_) => None,
        })
        .map(|count| count.min(MAX_DECRYPT_RETRIES as u64) as u8)
        .unwrap_or(0)
}

#[inline]
fn attr_matches_jid(value: &ValueRef<'_>, jid: &Jid) -> bool {
    match value {
        ValueRef::String(value) => wacore_binary::jid::parse_jid_ref(value)
            .map(|parsed| jid == &parsed)
            .unwrap_or_else(|| value.parse::<Jid>().is_ok_and(|parsed| jid == &parsed)),
        ValueRef::Jid(value) => jid == value,
    }
}

fn message_enc_nodes_for_device<'node, 'data: 'node>(
    node: &'node NodeRef<'data>,
    own_jid: Option<&'node Jid>,
) -> impl Iterator<Item = &'node NodeRef<'data>> + 'node {
    let per_device = node
        .get_optional_child("participants")
        .into_iter()
        .flat_map(|participants| participants.get_children_by_tag("to"))
        .filter(move |to_node| {
            own_jid.is_some_and(|ours| {
                to_node
                    .get_attr("jid")
                    .is_some_and(|value| attr_matches_jid(value, ours))
            })
        })
        .flat_map(|to_node| to_node.get_children_by_tag("enc"));

    node.get_children_by_tag("enc").chain(per_device)
}

/// Pre-extracted enc node payload. Holds owned copies of the fields needed for
/// decryption so the async decrypt phase doesn't borrow the original NodeRef tree.
pub(crate) struct EncPayload {
    pub ciphertext: bytes::Bytes,
    pub enc_type: EncType,
    pub padding_version: u8,
    /// Position in the order [`message_enc_nodes_for_device`] yields, counting
    /// from zero: direct `<enc>` children first, then this device's under
    /// `<participants><to>`.
    ///
    /// Recorded during classification because nothing downstream can recover
    /// it: payloads are split into per-kind buckets and encs that produce no
    /// payload are skipped, so a position within a bucket is not a position in
    /// the stanza.
    pub enc_index: usize,
    /// The node's `state` attribute, verbatim. Absent on ordinary traffic, so
    /// the common case allocates nothing.
    pub state: Option<String>,
    /// The node's `session_type` attribute, verbatim. Absent on ordinary
    /// traffic, so the common case allocates nothing.
    pub session_type: Option<String>,
}

impl EncPayload {
    fn from_parts(
        ciphertext: bytes::Bytes,
        enc_node: &NodeRef<'_>,
        enc_index: usize,
    ) -> Option<Self> {
        let enc_type = EncType::from_wire(enc_node.attrs().optional_string("type")?.as_ref())?;
        let padding_version = enc_node.attrs().optional_u64("v").unwrap_or(2) as u8;
        let mut attrs = enc_node.attrs();
        Some(Self {
            ciphertext,
            enc_type,
            padding_version,
            enc_index,
            state: attrs.optional_string("state").map(|s| s.into_owned()),
            session_type: attrs
                .optional_string("session_type")
                .map(|s| s.into_owned()),
        })
    }

    /// Zero-copy extraction from an OwnedNodeRef.
    pub(crate) fn from_owned_node(
        owner: &OwnedNodeRef,
        enc_node: &NodeRef<'_>,
        enc_index: usize,
    ) -> Option<Self> {
        Self::from_parts(
            owner.slice_bytes(enc_node.content_bytes()?),
            enc_node,
            enc_index,
        )
    }

    /// Copying extraction from a NodeRef (used in tests where there's no OwnedNodeRef).
    #[cfg(test)]
    pub(crate) fn from_node_ref(node: &NodeRef<'_>, enc_index: usize) -> Option<Self> {
        Self::from_parts(
            bytes::Bytes::copy_from_slice(node.content_bytes()?),
            node,
            enc_index,
        )
    }
}

/// The `<enc>` attributes this build carries to the consumer without acting on
/// them. Borrowed from the payload they came from, so passing them costs
/// nothing on a node that declared neither.
#[derive(Clone, Copy, Default)]
pub(crate) struct EncNodeAnnotations<'a> {
    pub state: Option<&'a str>,
    pub session_type: Option<&'a str>,
}

impl EncPayload {
    pub(crate) fn annotations(&self) -> EncNodeAnnotations<'_> {
        EncNodeAnnotations {
            state: self.state.as_deref(),
            session_type: self.session_type.as_deref(),
        }
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

/// Outcome of a PN→LID migration retry decrypt. On `Decrypted` the plaintext
/// has already been pushed onto the caller's deferred-handling buffer (it runs
/// after the session lock is released), so no dispatch flags travel back here.
#[derive(Clone, Copy, Debug)]
enum MigrationDecryptResult {
    /// Decrypted; plaintext buffered for post-lock handling.
    Decrypted,
    /// Server redelivered an already-processed message.
    Duplicate,
    /// Migration didn't apply, or applied and the retry still failed; the
    /// caller sends a retry receipt either way.
    ///
    /// `Some` carries the terminal cause when a migration actually ran and its
    /// retry decrypt failed. Without it the caller would report the error that
    /// sent it here — typically `NoSession` — for a message whose session was
    /// in fact found and whose retry then failed a MAC or a store read.
    /// `None` means nothing was migrated, so the caller's own error still is
    /// the terminal one.
    NotDecrypted(Option<EncDecryptFailureReason>),
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct PlaintextHandleOutcome {
    dispatched: bool,
    skdm_only: bool,
}

const INBOUND_COMMIT_PENDING: u8 = 0;
const INBOUND_COMMIT_DURABLE: u8 = 1;
const INBOUND_COMMIT_DROPPED: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InboundCommitTicketState {
    Pending,
    Durable,
    Dropped,
}

#[derive(Clone)]
pub(crate) struct InboundCommitTicket(Arc<AtomicU8>);

impl InboundCommitTicket {
    fn new() -> Self {
        Self(Arc::new(AtomicU8::new(INBOUND_COMMIT_PENDING)))
    }

    fn state(&self) -> InboundCommitTicketState {
        match self.0.load(Ordering::Acquire) {
            INBOUND_COMMIT_DURABLE => InboundCommitTicketState::Durable,
            INBOUND_COMMIT_DROPPED => InboundCommitTicketState::Dropped,
            _ => InboundCommitTicketState::Pending,
        }
    }

    fn resolve(&self, state: u8) {
        let _ = self.0.compare_exchange(
            INBOUND_COMMIT_PENDING,
            state,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    fn mark_durable(&self) {
        self.resolve(INBOUND_COMMIT_DURABLE);
    }

    fn mark_dropped(&self) {
        self.resolve(INBOUND_COMMIT_DROPPED);
    }
}

pub(crate) enum InboundCommitState {
    Durable,
    Deferred(Option<InboundCommitTicket>),
    Failed,
}

/// A decrypted session plaintext buffered during the locked decrypt loop and
/// handled after the per-sender session lock is released.
struct DeferredPlaintext {
    enc_type: &'static str,
    plaintext: Vec<u8>,
    padding_version: u8,
    /// Which `<enc>` in the stanza produced this — [`EncPayload::enc_index`],
    /// carried through because the buffer drains after the decrypt loop.
    enc_index: usize,
    state: Option<String>,
    session_type: Option<String>,
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

/// Errors libsignal raises while turning bytes into a message of their declared
/// type: too short to hold the signature, a version this build predates or does
/// not know, or a body that is not the protobuf it claims. No key material is
/// used to reach any of them.
///
/// Shared by the session and group arms so one libsignal error cannot be
/// reported as a malformed envelope on one path and an unclassified failure on
/// the other. `UnrecognizedMessageVersion` is deliberately absent: it is the
/// *state* mismatch `group_decrypt` raises after parsing, not a parse failure —
/// `UnrecognizedCiphertextVersion` is that one.
fn is_malformed_envelope_error(e: &SignalProtocolError) -> bool {
    matches!(
        e,
        SignalProtocolError::CiphertextMessageTooShort(_)
            | SignalProtocolError::LegacyCiphertextVersion(_)
            | SignalProtocolError::UnrecognizedCiphertextVersion(_)
            | SignalProtocolError::InvalidProtobufEncoding
    )
}

/// Cause reported for a libsignal error the decrypt arms do not name themselves.
///
/// Shared by the session catch-all and the group arm so the same error cannot be
/// classified two ways. `BackendError` is local storage failing to answer — the
/// store adapter wraps every backend error in it — and not the ciphertext
/// failing: reporting it as a cryptographic error would blame the peer for our
/// own disk, and corrupt any per-peer health signal built on this event.
fn signal_error_reason(e: &SignalProtocolError) -> EncDecryptFailureReason {
    if is_malformed_envelope_error(e) {
        EncDecryptFailureReason::MalformedCiphertext
    } else if matches!(e, SignalProtocolError::BackendError(_, _)) {
        EncDecryptFailureReason::StorageFailure
    } else if matches!(e, SignalProtocolError::KeyAgreementFailed(_)) {
        // "the active crypto provider failed the key agreement" — our provider,
        // not the sender's bytes, which were never judged.
        EncDecryptFailureReason::LocalCryptoFailure
    } else if e.is_stored_session_corruption() {
        // A stored `SessionRecord` that decoded and then would not yield usable
        // state. The predicate lives in libsignal because the distinction is
        // drawn on `InvalidSessionStructure`'s message, and only the crate that
        // writes those messages can keep the two in step.
        EncDecryptFailureReason::StorageFailure
    } else if matches!(e, SignalProtocolError::InvalidSenderKeySession) {
        // A sender-key record that loaded but does not hold usable state: no
        // chain key, a signing key that will not parse, or a chain whose derived
        // key/IV the cipher rejects. Every site `group_decrypt` can reach it
        // from is reading our stored record, which is why libsignal's own log
        // there says the state is corrupt. The peer's copy is judged by
        // `SignatureValidationFailed` and `InvalidMessage` instead.
        EncDecryptFailureReason::StorageFailure
    } else if matches!(e, SignalProtocolError::UnrecognizedMessageVersion(_)) {
        // The group arm reaches this one through `group_decrypt_retry_reason`
        // and calls it an invalid message. Naming it here too keeps a session
        // `<enc>` and an `skmsg` from reporting the same rejection differently.
        EncDecryptFailureReason::InvalidMessage
    } else {
        EncDecryptFailureReason::SignalError
    }
}

/// Cause for a terminal error on the 1:1 session path, matching what the arms
/// of `process_session_enc_batch` report for the same libsignal errors.
///
/// Used where an error reaches a reporting site that has no arm of its own —
/// the PN→LID migration's retry decrypt — so a migrated session that then fails
/// a MAC is not reported under the error that opened the migration.
fn session_error_reason(e: &SignalProtocolError) -> EncDecryptFailureReason {
    match e {
        SignalProtocolError::SessionNotFound(_) => EncDecryptFailureReason::NoSession,
        SignalProtocolError::BadMac(_) => EncDecryptFailureReason::BadMac,
        SignalProtocolError::InvalidMessage(_, _) => EncDecryptFailureReason::InvalidMessage,
        SignalProtocolError::InvalidPreKeyId | SignalProtocolError::InvalidSignedPreKeyId => {
            EncDecryptFailureReason::UnknownPreKey
        }
        SignalProtocolError::UntrustedIdentity(_) => EncDecryptFailureReason::UntrustedIdentity,
        other => signal_error_reason(other),
    }
}

/// WA Web treats every `SignalDecryptionError` as `SignalRetryable`, so a
/// sender-key desync must request a resend rather than NACK (which stops the
/// server retransmitting). `None` = keep the NACK (genuinely non-Signal error).
fn group_decrypt_retry_reason(e: &SignalProtocolError) -> Option<RetryReason> {
    match e {
        SignalProtocolError::SignatureValidationFailed => Some(RetryReason::InvalidSignature),
        SignalProtocolError::InvalidSenderKeySession => Some(RetryReason::InvalidSession),
        SignalProtocolError::UnrecognizedMessageVersion(_) => Some(RetryReason::InvalidMessage),
        SignalProtocolError::InvalidMessage(_, _) => Some(RetryReason::InvalidMessage),
        _ => None,
    }
}

pub(crate) use wacore::protocol::retry::RetryReason;

pub(crate) mod commit_batch;
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
