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
use whatsapp_rust::wacore_binary::node::NodeRef;
use whatsapp_rust::wacore_binary::{Jid, JidExt, Server};
use whatsapp_rust_wam_catalog::{enums, events};

/// The WAM device type for the sender of an inbound stanza.
///
/// Three axes, all on the envelope: whose account it is (`fromMe`), whether the
/// JID is a hosted endpoint, and whether it names the primary device or a
/// companion. The coex members are not produced, because nothing on the envelope
/// tells a coex device from an ordinary companion, so choosing between them
/// would be an invention.
fn sender_type(sender: &Jid, is_from_me: bool) -> enums::E2eDeviceType {
    // Hosted first, and not device number first: a hosted server carries a
    // device 0 of its own, and WAM has no hosted primary to report it as. Being
    // hosted is the stronger fact anyway, since it names an endpoint the account
    // did not pair for itself rather than a position in a device list.
    match (is_from_me, sender.is_hosted(), sender.device == 0) {
        (true, true, _) => enums::E2eDeviceType::MyHostedCompanion,
        (false, true, _) => enums::E2eDeviceType::OtherHostedCompanion,
        (true, false, true) => enums::E2eDeviceType::MyPrimary,
        (false, false, true) => enums::E2eDeviceType::OtherPrimary,
        (true, false, false) => enums::E2eDeviceType::MyCompanion,
        (false, false, false) => enums::E2eDeviceType::OtherCompanion,
    }
}

/// Whether a JID is LID-namespaced.
///
/// Not `Jid::is_lid`, which asks only about `@lid`. WAM's `isLid` names the
/// namespace a sender was addressed in, and `@hosted.lid` is in it; reporting
/// one of those as `false` would say the peer was addressed by phone number
/// when it was not.
fn is_lid(jid: &Jid) -> bool {
    jid.server.is_lid_family()
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
        is_lid: Some(is_lid(&info.source.sender)),
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
        is_lid: Some(is_lid(&info.source.sender)),
        e2e_sender_type: Some(sender_type(&info.source.sender, info.source.is_from_me)),
        server_addressing_mode: info.source.addressing_mode.map(addressing_mode),
        ..Default::default()
    }
}

/// How many message receipts one `<receipt>` stanza carries.
///
/// Counted off the wire rather than taken from a parsed receipt, and only the
/// count is taken: the ids themselves are message ids, and nothing in this crate
/// is allowed to hold one.
fn receipt_item_count(node: &NodeRef<'_>, is_view: bool) -> usize {
    // The aggregated shape names one peer per `<user>` and carries no `<list>`.
    if let Some(participants) = node.get_optional_child("participants") {
        return participants
            .children()
            .map_or(0, |users| users.iter().filter(|c| c.tag == "user").count());
    }
    // A view receipt names its ids as `server_id` and does not acknowledge the
    // stanza's own `id`; every other type does both. This is the rule the
    // client's own receipt parser applies, and the count has to describe the
    // same set of messages the stanza acknowledges or it is not that stanza's
    // count.
    let id_attr = if is_view { "server_id" } else { "id" };
    let listed = node
        .get_optional_child("list")
        .and_then(|list| list.children())
        .map_or(0, |items| {
            items
                .iter()
                .filter(|c| c.tag == "item" && c.attrs().optional_string(id_attr).is_some())
                .count()
        });
    listed + usize::from(!is_view)
}

/// `ReceiptStanzaReceive` for one inbound `<receipt>` stanza.
///
/// Takes the stanza, not the client's `Receipt` event, because this is a
/// per-stanza metric and that event is not one per stanza: the aggregated shape
/// dispatches one event for every `<user>` it names, and a retry receipt is
/// consumed by the retry pipeline without dispatching one at all. Deriving from
/// the event would report several stanzas where one arrived and none where one
/// did, and the count field would carry a per-peer number under a per-stanza
/// name.
///
/// `receiptStanzaStage` is `OVERALL` because that is what the official client's
/// own constructor writes: the other members name sub-steps of a pipeline this
/// client does not instrument, so they are not reachable from here.
///
/// Returns `None` for a `type` this client cannot name. The alternative is
/// putting a string the server chose into a telemetry field, which is neither a
/// derivation nor something the field's vocabulary covers.
pub fn receipt_stanza_receive(node: &NodeRef<'_>) -> Option<events::ReceiptStanzaReceive> {
    let raw_type = node.attrs().optional_string("type");
    // A `<receipt>` with no `type` is a delivery receipt, which is why the
    // official client sends that one as a dropped attribute.
    let raw_type = raw_type.as_deref().unwrap_or("delivery");
    let named = match ReceiptType::parse(raw_type) {
        // `view` has no variant of its own because the client branches on the
        // string rather than on a parsed type, but it is a type this client
        // names: that same branch is what decides which attribute the ids below
        // are read from.
        ReceiptType::Other(_) if raw_type == "view" => "view".to_string(),
        ReceiptType::Other(_) => return None,
        known => known.as_wire_str().to_string(),
    };
    Some(events::ReceiptStanzaReceive {
        receipt_stanza_type: Some(named),
        receipt_stanza_total_count: i64::try_from(receipt_item_count(node, raw_type == "view"))
            .ok(),
        receipt_stanza_stage: Some(enums::ReceiptStanzaStage::Overall),
        ..Default::default()
    })
}

/// The `MessageReceive` events one decrypted batch produces.
///
/// A batch is one durable commit, so this is per message rather than per batch:
/// the official client counts a receive per message too. The `E2eMessageRecv`
/// for the same traffic is derived per `<enc>` instead, which is the unit that
/// event counts.
pub fn from_batch(batch: &MessageBatch) -> Vec<crate::runtime::PendingEvent> {
    let mut out = Vec::with_capacity(batch.len());
    for message in batch.iter() {
        out.push(crate::runtime::PendingEvent::MessageReceive(
            message_receive(&message.info),
        ));
    }
    out
}

/// Whether a failure reason says anything about this `<enc>`'s decryption.
///
/// `e2eSuccessful: false` is a claim that this client read this `<enc>`'s
/// ciphertext and could not turn it into plaintext. Two groups of reasons make
/// that claim false, in opposite directions.
///
/// Below the line is `EncDecryptFailureReason::decryption_was_attempted`, the
/// core's own name for it: those nodes were set aside before any ciphertext was
/// read, and the last of them is not even about the stanza, only about when it
/// arrived. Counting them as failures moves a success rate with something that
/// was never a decryption.
///
/// Above it is `PlaintextUnusable`, where the decryption succeeded and
/// something after it could not use the bytes. When the padding was the usable
/// part, the same `<enc>` already produced a `DecryptedPayload` and was already
/// counted as a success, so a second metric would contradict the first. WAM has
/// no member for "decrypted but undecodable", so the honest report is none.
///
/// One reason crosses back the other way. `UnsupportedEncType` is not attempted
/// either, but unlike the rest of that group it is a fact about this `<enc>`'s
/// own `type` attribute, and WAM names exactly it, so it is reported with that
/// reason rather than dropped.
fn reports_a_decryption_outcome(reason: &EncDecryptFailureReason) -> bool {
    match reason {
        EncDecryptFailureReason::PlaintextUnusable => false,
        EncDecryptFailureReason::UnsupportedEncType => true,
        other => other.decryption_was_attempted(),
    }
}

/// The `E2eMessageRecv` one failed `<enc>` produces, when it produces one.
///
/// `None` for a reason that does not describe this `<enc>`'s decryption: one
/// the client set aside before reading any ciphertext, or one where the
/// decryption succeeded and something after it could not use the bytes. Such an
/// `<enc>` goes unreported rather than misreported, which is the side of the
/// choice that keeps the number meaning what it says. The exact line, and the
/// one reason that crosses it, are in `reports_a_decryption_outcome` below.
pub fn from_enc_failure(failed: &EncDecryptFailed) -> Option<events::E2eMessageRecv> {
    if !reports_a_decryption_outcome(&failed.reason) {
        return None;
    }
    Some(e2e_message_recv(
        &failed.info,
        failed.enc_type.as_deref().unwrap_or_default(),
        false,
        Some(&failed.reason),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::wacore::types::message::MessageSource;
    use whatsapp_rust::wacore_binary::builder::NodeBuilder;
    use whatsapp_rust::wacore_binary::marshal::marshal;
    use whatsapp_rust::wacore_binary::util::unpack;
    use whatsapp_rust::wacore_binary::{Node, OwnedNodeRef};

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
    fn a_receipt_reports_its_wire_type_and_how_many_messages_it_acknowledged() {
        let node = receipt(&[("type", "read")], Some(list("id", 2)));
        let event = derive_receipt(&node).expect("read is a type this client names");
        assert_eq!(event.receipt_stanza_type, Some("read".to_string()));
        // The two listed ids plus the stanza's own.
        assert_eq!(event.receipt_stanza_total_count, Some(3));
        assert_eq!(
            event.receipt_stanza_stage,
            Some(enums::ReceiptStanzaStage::Overall)
        );
        // Nothing about the messages themselves reaches the buffer.
        assert_eq!(event.message_type, None);
    }

    #[test]
    fn a_receipt_with_no_type_is_a_delivery_receipt_for_its_own_id() {
        let node = receipt(&[], None);
        let event = derive_receipt(&node).expect("a typeless receipt is a delivery receipt");
        assert_eq!(event.receipt_stanza_type, Some("delivery".to_string()));
        assert_eq!(event.receipt_stanza_total_count, Some(1));
    }

    #[test]
    fn an_aggregated_receipt_is_one_metric_counting_every_user_it_names() {
        // The shape that makes the client's `Receipt` event unusable here: one
        // stanza, several peers, and the client dispatches an event per peer.
        let users = NodeBuilder::new("participants")
            .attr("id", "AGG")
            .children((0..3).map(|i| {
                NodeBuilder::new("user")
                    .attr("jid", format!("1555000000{i}@s.whatsapp.net"))
                    .build()
            }))
            .build();
        let node = receipt(&[("type", "delivery")], Some(users));
        let event = derive_receipt(&node).expect("delivery is a type this client names");
        assert_eq!(event.receipt_stanza_total_count, Some(3));
    }

    #[test]
    fn a_view_receipt_counts_its_server_ids_and_not_the_stanza_id() {
        let node = receipt(&[("type", "view")], Some(list("server_id", 2)));
        let event = derive_receipt(&node).expect("view is a type this client names");
        assert_eq!(event.receipt_stanza_total_count, Some(2));
    }

    #[test]
    fn a_receipt_type_this_client_cannot_name_produces_no_metric() {
        // The alternative is copying a server-chosen string into a telemetry
        // field, which is neither derived nor in the field's vocabulary.
        let node = receipt(&[("type", "something-the-server-invented")], None);
        assert!(derive_receipt(&node).is_none());
    }

    /// A `<receipt>` stanza, through the same marshal and decode the receive
    /// path takes, so a test reads the node a real one would produce.
    fn receipt(attrs: &[(&'static str, &'static str)], child: Option<Node>) -> OwnedNodeRef {
        let mut builder = NodeBuilder::new("receipt")
            .attr("id", "STANZA-ID")
            .attr("from", "15550000009@s.whatsapp.net");
        for (key, value) in attrs {
            builder = builder.attr(key, *value);
        }
        let node = builder.children(child).build();
        let packed = marshal(&node).expect("a receipt marshals");
        let bytes = unpack(&packed).expect("a marshalled receipt unpacks");
        OwnedNodeRef::new(bytes.into_owned()).expect("a receipt decodes")
    }

    /// A `<list>` of `count` items, each naming a message under `id_attr`.
    fn list(id_attr: &'static str, count: usize) -> Node {
        NodeBuilder::new("list")
            .children((0..count).map(|i| {
                NodeBuilder::new("item")
                    .attr(id_attr, format!("MSG-{i}"))
                    .build()
            }))
            .build()
    }

    fn derive_receipt(node: &OwnedNodeRef) -> Option<events::ReceiptStanzaReceive> {
        receipt_stanza_receive(node.get())
    }

    #[test]
    fn an_enc_the_client_never_tried_produces_no_metric() {
        // The contract on `NotAttempted` says the ciphertext was never read,
        // and the torn-down-connection case is not even about this stanza.
        // Counting it as a decryption failure moves the success rate with
        // something that was never a decryption.
        assert!(from_enc_failure(&failure(EncDecryptFailureReason::NotAttempted)).is_none());
        assert!(from_enc_failure(&failure(EncDecryptFailureReason::MalformedNode)).is_none());

        // The one that is not attempted and is still reported: it names this
        // `<enc>`'s own `type` attribute, and WAM has a member for exactly that.
        let unsupported = from_enc_failure(&failure(EncDecryptFailureReason::UnsupportedEncType))
            .expect("an unusable enc type is a fact about this enc");
        assert_eq!(
            unsupported.e2e_failure_reason,
            Some(enums::E2eFailureReason::UnknownCiphertextType)
        );
    }

    #[test]
    fn a_decrypted_but_undecodable_enc_produces_no_metric() {
        // The Signal layer succeeded, so `e2eSuccessful: false` would be a lie,
        // and the same `<enc>` may already have been counted as a success. WAM
        // has no member for what actually happened, so nothing is reported.
        let unusable = failure(EncDecryptFailureReason::PlaintextUnusable);
        assert!(from_enc_failure(&unusable).is_none());

        // A real decryption failure still reports, so the suppression is about
        // that one reason and not about failures generally.
        let bad_mac = failure(EncDecryptFailureReason::BadMac);
        let event = from_enc_failure(&bad_mac).expect("a mac failure is a decryption failure");
        assert_eq!(event.e2e_successful, Some(false));
        assert_eq!(
            event.e2e_failure_reason,
            Some(enums::E2eFailureReason::InvalidMac)
        );
    }

    #[test]
    fn a_hosted_companion_is_not_an_ordinary_one() {
        // Two ways a device says it is hosted: the reserved device number and
        // the hosted servers. Both are on the envelope, so neither is a guess.
        let by_device = jid("15550000006", Server::Pn, 99);
        let by_server = jid("15550000007", Server::Hosted, 4);
        assert_eq!(
            sender_type(&by_device, false),
            enums::E2eDeviceType::OtherHostedCompanion
        );
        assert_eq!(
            sender_type(&by_server, true),
            enums::E2eDeviceType::MyHostedCompanion
        );
        // And an ordinary companion still is one.
        assert_eq!(
            sender_type(&jid("15550000008", Server::Pn, 4), false),
            enums::E2eDeviceType::OtherCompanion
        );

        // A hosted server carries a device 0 of its own, and WAM has no hosted
        // primary, so being hosted has to win over the device number.
        assert_eq!(
            sender_type(&jid("15550000011", Server::Hosted, 0), false),
            enums::E2eDeviceType::OtherHostedCompanion
        );
        assert_eq!(
            sender_type(&jid("15550000011", Server::HostedLid, 0), true),
            enums::E2eDeviceType::MyHostedCompanion
        );
    }

    #[test]
    fn a_hosted_lid_sender_is_reported_as_lid() {
        // `@hosted.lid` is in the LID namespace. Saying otherwise would report a
        // peer as addressed by phone number when it never was.
        assert!(is_lid(&jid("15550000010", Server::HostedLid, 0)));
        assert!(is_lid(&jid("15550000010", Server::Lid, 0)));
        assert!(!is_lid(&jid("15550000010", Server::Pn, 0)));
    }

    /// An `EncDecryptFailed` that says only which reason it carries.
    fn failure(reason: EncDecryptFailureReason) -> EncDecryptFailed {
        EncDecryptFailed::builder()
            .info(std::sync::Arc::new(info_fixture()))
            .enc_index(0)
            .enc_type(std::borrow::Cow::Borrowed("msg"))
            .reason(reason)
            .build()
    }

    /// A `MessageInfo` with every field at its default, so a test only has to
    /// state what it is about.
    fn info_fixture() -> MessageInfo {
        MessageInfo::default()
    }
}
