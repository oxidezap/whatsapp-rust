//! Turning what this client observes into WAM events.
//!
//! One rule governs every function here: a field is written only when its value
//! follows from something this client actually saw. Where the official client
//! fills a field from state this library does not keep (a chat's unread count,
//! a message's reply target, the size of a device list), the field is absent,
//! and `parity.rs` proves that the ones that *are* written are ones WA Web
//! writes too.
//!
//! Everything here reads the stanza envelope, never the decoded message. That is
//! not only a privacy rule (no JID, phone number, message id or body can reach a
//! buffer if the body is never opened) but a correctness one: the envelope
//! attributes are what the official client's own receive metrics are built from.

use whatsapp_rust::wacore::types::events::{
    EncDecryptFailed, EncDecryptFailureReason, MessageBatch,
};
use whatsapp_rust::wacore::types::message::{AddressingMode, MessageInfo};
use whatsapp_rust::wacore::types::presence::ReceiptType;
use whatsapp_rust::wacore::types::wire_enums::EncMediaType;
use whatsapp_rust::wacore_binary::{Jid, JidExt, Server};
use whatsapp_rust_wam_catalog::{enums, events};

/// The WAM device type for the sender of an inbound stanza.
///
/// Two axes, both on the envelope: whose account it is (`fromMe`) and whether
/// the JID names the primary device or a companion. The hosted and coex members
/// are not produced, because this client does not distinguish a hosted companion from
/// an ordinary one, so guessing between them would be an invention.
fn sender_type(sender: &Jid, is_from_me: bool) -> enums::E2eDeviceType {
    match (is_from_me, sender.device == 0) {
        (true, true) => enums::E2eDeviceType::MyPrimary,
        (true, false) => enums::E2eDeviceType::MyCompanion,
        (false, true) => enums::E2eDeviceType::OtherPrimary,
        (false, false) => enums::E2eDeviceType::OtherCompanion,
    }
}

/// The WAM message type for a chat, from the chat JID's server.
///
/// `INTEROP`, `GREETING` and `MEDIA_HUB` have no envelope-level tell, so a chat
/// that is one of those is reported as whatever its server says it is rather
/// than guessed at.
fn message_type(chat: &Jid) -> Option<enums::MessageType> {
    Some(match chat.server {
        Server::Group => enums::MessageType::Group,
        Server::Broadcast if chat.is_status_broadcast() => enums::MessageType::Status,
        Server::Broadcast => enums::MessageType::Broadcast,
        Server::Newsletter => enums::MessageType::Channel,
        Server::Interop => enums::MessageType::Interop,
        Server::Pn | Server::Lid | Server::Hosted | Server::HostedLid => {
            enums::MessageType::Individual
        }
        _ => return None,
    })
}

/// Where an `<enc>` was addressed, which WAM numbers separately from the
/// message type.
fn destination(chat: &Jid) -> Option<enums::E2eDestination> {
    Some(match chat.server {
        Server::Group => enums::E2eDestination::Group,
        Server::Broadcast if chat.is_status_broadcast() => enums::E2eDestination::Status,
        Server::Broadcast => enums::E2eDestination::List,
        Server::Newsletter => enums::E2eDestination::Channel,
        Server::Interop => enums::E2eDestination::Interop,
        Server::Pn | Server::Lid | Server::Hosted | Server::HostedLid => {
            enums::E2eDestination::Individual
        }
        _ => return None,
    })
}

/// The WAM media type for the `mediatype` attribute the `<enc>` carried.
///
/// Absent when the stanza carried none. The catalog has a `NONE` member and this
/// deliberately does not use it: "no `mediatype` attribute" is what a plain text
/// message looks like *and* what an unrecognised one looks like, so reporting
/// `NONE` would assert something the envelope does not say.
fn media_type(media: &EncMediaType) -> Option<enums::MediaType> {
    Some(match media {
        EncMediaType::Image => enums::MediaType::Photo,
        EncMediaType::Video => enums::MediaType::Video,
        EncMediaType::Ptv => enums::MediaType::PushToVideo,
        EncMediaType::Audio => enums::MediaType::Audio,
        EncMediaType::Ptt => enums::MediaType::Ptt,
        EncMediaType::Location => enums::MediaType::Location,
        EncMediaType::Vcard => enums::MediaType::Contact,
        EncMediaType::Document => enums::MediaType::Document,
        EncMediaType::Url => enums::MediaType::Url,
        EncMediaType::Call => enums::MediaType::Call,
        EncMediaType::Gif => enums::MediaType::Gif,
        EncMediaType::Future => enums::MediaType::Future,
        EncMediaType::ContactArray => enums::MediaType::ContactArray,
        EncMediaType::LiveLocation => enums::MediaType::LiveLocation,
        EncMediaType::ProfilePic => enums::MediaType::ProfilePic,
        EncMediaType::Sticker => enums::MediaType::Sticker,
        EncMediaType::StickerPack => enums::MediaType::StickerPack,
        EncMediaType::Hsm => enums::MediaType::Hsm,
        EncMediaType::ProductImage => enums::MediaType::ProductImage,
        EncMediaType::Template => enums::MediaType::Template,
        // Anything this build models but WAM does not name identically stays
        // absent rather than being rounded to a neighbour.
        _ => return None,
    })
}

fn addressing_mode(mode: AddressingMode) -> enums::AddressingMode {
    match mode {
        AddressingMode::Pn => enums::AddressingMode::Pn,
        AddressingMode::Lid => enums::AddressingMode::Lid,
    }
}

/// The WAM ciphertext type for an `<enc type=…>`.
fn ciphertext_type(enc_type: &str) -> Option<enums::E2eCiphertextType> {
    Some(match enc_type {
        "msg" => enums::E2eCiphertextType::Message,
        "pkmsg" => enums::E2eCiphertextType::PrekeyMessage,
        "skmsg" => enums::E2eCiphertextType::SenderKeyMessage,
        "msmsg" => enums::E2eCiphertextType::MessageSecretMessage,
        _ => return None,
    })
}

/// The WAM failure reason for a decrypt failure this client classified.
///
/// Mapped only where the two vocabularies mean the same thing. WAM's list is
/// a hundred entries deep and mostly names internal steps of a different Signal
/// implementation; picking the nearest-sounding member for a reason this client
/// spells differently would put a value on the wire that does not describe what
/// happened. The unmapped reasons leave the field absent, which is what the
/// buffer format is for.
fn failure_reason(reason: &EncDecryptFailureReason) -> Option<enums::E2eFailureReason> {
    Some(match reason {
        EncDecryptFailureReason::NoSession => enums::E2eFailureReason::NoSessionAvailable,
        EncDecryptFailureReason::UntrustedIdentity => enums::E2eFailureReason::UntrustedIdentity,
        EncDecryptFailureReason::BadMac => enums::E2eFailureReason::InvalidMac,
        EncDecryptFailureReason::InvalidMessage => enums::E2eFailureReason::InvalidMessage,
        EncDecryptFailureReason::UnknownPreKey => {
            enums::E2eFailureReason::PreKeyMessageMissingPreKey
        }
        EncDecryptFailureReason::UnsupportedEncType => {
            enums::E2eFailureReason::UnknownCiphertextType
        }
        EncDecryptFailureReason::NoMessageSecret => enums::E2eFailureReason::MissingMessageSecret,
        _ => return None,
    })
}

/// `E2eMessageRecv` for one `<enc>` that decrypted.
///
/// `e2eCiphertextType` needs the `<enc type>` attribute, which only reaches a
/// consumer through the per-payload event, hence this takes the enc type
/// rather than reading it off the message.
pub fn e2e_message_recv(
    info: &MessageInfo,
    enc_type: &str,
    successful: bool,
    failure: Option<&EncDecryptFailureReason>,
) -> events::E2eMessageRecv {
    events::E2eMessageRecv {
        e2e_successful: Some(successful),
        e2e_ciphertext_type: ciphertext_type(enc_type),
        e2e_failure_reason: failure.and_then(failure_reason),
        e2e_sender_type: Some(sender_type(&info.source.sender, info.source.is_from_me)),
        e2e_destination: destination(&info.source.chat),
        is_lid: Some(info.source.sender.is_lid()),
        server_addressing_mode: info.source.addressing_mode.map(addressing_mode),
        message_media_type: info.media_type.as_ref().and_then(media_type),
        ..Default::default()
    }
}

/// `MessageReceive` for one decrypted inbound message.
pub fn message_receive(info: &MessageInfo) -> events::MessageReceive {
    events::MessageReceive {
        message_type: message_type(&info.source.chat),
        message_media_type: info.media_type.as_ref().and_then(media_type),
        message_is_offline: Some(info.is_offline),
        is_lid: Some(info.source.sender.is_lid()),
        e2e_sender_type: Some(sender_type(&info.source.sender, info.source.is_from_me)),
        server_addressing_mode: info.source.addressing_mode.map(addressing_mode),
        ..Default::default()
    }
}

/// `ReceiptStanzaReceive` for one inbound `<receipt>`.
///
/// `receiptStanzaStage` is `OVERALL` because that is what the official client's
/// own constructor writes: the other members name sub-steps of a pipeline this
/// client does not instrument, so they are not reachable from here.
pub fn receipt_stanza_receive(
    receipt_type: &ReceiptType,
    message_count: usize,
) -> events::ReceiptStanzaReceive {
    events::ReceiptStanzaReceive {
        receipt_stanza_type: Some(receipt_type.as_wire_str().to_string()),
        receipt_stanza_total_count: i64::try_from(message_count).ok(),
        receipt_stanza_stage: Some(enums::ReceiptStanzaStage::Overall),
        ..Default::default()
    }
}

/// The `E2eMessageRecv` and `MessageReceive` events one decrypted batch
/// produces.
///
/// A batch is one durable commit, so this is per message rather than per batch:
/// the official client counts a receive per message too.
pub fn from_batch(batch: &MessageBatch) -> Vec<crate::runtime::PendingEvent> {
    let mut out = Vec::with_capacity(batch.len());
    for message in batch.iter() {
        out.push(crate::runtime::PendingEvent::MessageReceive(
            message_receive(&message.info),
        ));
    }
    out
}

/// The `E2eMessageRecv` one failed `<enc>` produces.
pub fn from_enc_failure(failed: &EncDecryptFailed) -> events::E2eMessageRecv {
    e2e_message_recv(
        &failed.info,
        failed.enc_type.as_deref().unwrap_or_default(),
        false,
        Some(&failed.reason),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::wacore::types::message::MessageSource;

    /// A fictitious sender. No test in this crate uses a real number.
    fn jid(user: &str, server: Server, device: u16) -> Jid {
        Jid {
            user: user.into(),
            server,
            device,
            ..Default::default()
        }
    }

    fn source(chat: Jid, sender: Jid, from_me: bool) -> MessageSource {
        MessageSource {
            chat,
            sender,
            is_from_me: from_me,
            addressing_mode: Some(AddressingMode::Lid),
            ..Default::default()
        }
    }

    #[test]
    fn the_sender_type_reads_both_axes_of_the_envelope() {
        let primary = jid("15550000001", Server::Pn, 0);
        let companion = jid("15550000001", Server::Pn, 3);
        assert_eq!(
            sender_type(&primary, false),
            enums::E2eDeviceType::OtherPrimary
        );
        assert_eq!(
            sender_type(&companion, false),
            enums::E2eDeviceType::OtherCompanion
        );
        assert_eq!(sender_type(&primary, true), enums::E2eDeviceType::MyPrimary);
        assert_eq!(
            sender_type(&companion, true),
            enums::E2eDeviceType::MyCompanion
        );
    }

    #[test]
    fn a_status_broadcast_is_not_an_ordinary_broadcast() {
        let status = Jid::new("status", Server::Broadcast);
        let list = jid("15550000002-1600000000", Server::Broadcast, 0);
        assert_eq!(message_type(&status), Some(enums::MessageType::Status));
        assert_eq!(message_type(&list), Some(enums::MessageType::Broadcast));
        assert_eq!(destination(&status), Some(enums::E2eDestination::Status));
        assert_eq!(destination(&list), Some(enums::E2eDestination::List));
    }

    #[test]
    fn a_server_wam_does_not_model_leaves_the_field_absent() {
        // `@call` and `@bot` have no member in either enum. Rounding one to
        // INDIVIDUAL would put a value on the wire that is not true.
        assert_eq!(message_type(&Jid::new("x", Server::Call)), None);
        assert_eq!(destination(&Jid::new("x", Server::Bot)), None);
    }

    #[test]
    fn an_unmapped_failure_reason_writes_nothing() {
        assert_eq!(
            failure_reason(&EncDecryptFailureReason::NoSession),
            Some(enums::E2eFailureReason::NoSessionAvailable)
        );
        // This client's `StorageFailure` names where it stopped, not a Signal
        // error WAM has a member for.
        assert_eq!(
            failure_reason(&EncDecryptFailureReason::StorageFailure),
            None
        );
        assert_eq!(failure_reason(&EncDecryptFailureReason::NotAttempted), None);
    }

    #[test]
    fn an_unknown_enc_type_leaves_the_ciphertext_type_absent() {
        assert_eq!(
            ciphertext_type("pkmsg"),
            Some(enums::E2eCiphertextType::PrekeyMessage)
        );
        assert_eq!(ciphertext_type(""), None);
        assert_eq!(ciphertext_type("something-new"), None);
    }

    #[test]
    fn a_message_with_no_mediatype_attribute_writes_no_media_type() {
        let info = MessageInfo {
            source: source(
                jid("15550000003", Server::Lid, 0),
                jid("15550000003", Server::Lid, 0),
                false,
            ),
            media_type: None,
            ..info_fixture()
        };
        let event = message_receive(&info);
        assert_eq!(event.message_media_type, None);
        assert_eq!(event.message_type, Some(enums::MessageType::Individual));
        assert_eq!(event.is_lid, Some(true));
        assert_eq!(
            event.server_addressing_mode,
            Some(enums::AddressingMode::Lid)
        );
    }

    #[test]
    fn a_failed_enc_reports_the_failure_and_not_success() {
        let info = MessageInfo {
            source: source(
                Jid::new("15550000004-1600000000", Server::Group),
                jid("15550000005", Server::Pn, 2),
                false,
            ),
            media_type: Some(EncMediaType::Ptt),
            ..info_fixture()
        };
        let event = e2e_message_recv(
            &info,
            "skmsg",
            false,
            Some(&EncDecryptFailureReason::BadMac),
        );
        assert_eq!(event.e2e_successful, Some(false));
        assert_eq!(
            event.e2e_ciphertext_type,
            Some(enums::E2eCiphertextType::SenderKeyMessage)
        );
        assert_eq!(
            event.e2e_failure_reason,
            Some(enums::E2eFailureReason::InvalidMac)
        );
        assert_eq!(event.e2e_destination, Some(enums::E2eDestination::Group));
        assert_eq!(event.message_media_type, Some(enums::MediaType::Ptt));
    }

    #[test]
    fn a_receipt_reports_its_wire_type_and_how_many_ids_it_carried() {
        let event = receipt_stanza_receive(&ReceiptType::Read, 3);
        assert_eq!(event.receipt_stanza_type, Some("read".to_string()));
        assert_eq!(event.receipt_stanza_total_count, Some(3));
        assert_eq!(
            event.receipt_stanza_stage,
            Some(enums::ReceiptStanzaStage::Overall)
        );
        // Nothing about the messages themselves reaches the buffer.
        assert_eq!(event.message_type, None);
    }

    /// A `MessageInfo` with every field at its default, so a test only has to
    /// state what it is about.
    fn info_fixture() -> MessageInfo {
        MessageInfo::default()
    }
}
