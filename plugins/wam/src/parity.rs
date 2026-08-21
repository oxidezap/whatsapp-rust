//! The conformance gate: every field this plugin writes is a field WA Web
//! writes.
//!
//! The whatspec IR records, per event, which modules of the official client
//! construct it and which fields each is seen writing there. That turns
//! "does this look like what WhatsApp sends?" from a reading of a minified
//! bundle into a set comparison a test can make, and it is the check this
//! repository has that a hand-written emitter cannot.
//!
//! # How it is run
//!
//! Not by inspecting the derivation functions: a test that reads source is a
//! test that lies the moment a field is added. Instead every event this plugin
//! can emit is built at its maximum, encoded through the real codec, and the
//! field ids are read back out of the bytes. Those ids are the fields that reach
//! the wire, whatever the code that produced them looks like.
//!
//! Two assertions, and they need each other:
//!
//! 1. Every field in a maximal event is one WA Web's own call sites write.
//! 2. Every field a *derivation* produces is in that maximal event, so the
//!    maximum cannot go stale while the derivations grow.
//!
//! # What it does not prove
//!
//! That the value is right. A call site says WA Web writes `messageType` on
//! `MessageReceive`; it does not say what it puts there, and no static reading
//! of the bundle could. It also does not prove the reverse direction: a field
//! WA Web writes and this plugin does not is not a failure, it is the whole
//! reason this batch emits five events and not four hundred.
//!
//! Where a call site is marked partial the extractor could not read every field
//! it writes, so the union is a lower bound and the check is weaker for that
//! event. None of the five events checked here has a partial site; the test
//! asserts that too, so the day one appears the weakening is visible rather
//! than silent.

use whatsapp_rust_wam_catalog::{Channel, WamBuffer, call_sites_for, enums, events};

use crate::runtime::PendingEvent;

/// A record read back out of an encoded buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Record {
    /// 0 global, 1 event, 2 field.
    kind: u8,
    id: u32,
}

/// Read the records out of an encoded buffer.
///
/// A decoder in the test rather than in the crate on purpose: the plugin has no
/// reason to read a buffer, and a decoder that shares code with the encoder
/// would agree with it about a mistake. This one is written from the format
/// description, so it disagrees when the encoder is wrong.
fn decode(bytes: &[u8]) -> Vec<Record> {
    let mut out = Vec::new();
    let mut i = 8; // "WAM", version, stream id, sequence, channel
    while i < bytes.len() {
        let flags = bytes[i];
        i += 1;
        let wide = flags & 8 != 0;
        let id = if wide {
            let id = u32::from(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
            i += 2;
            id
        } else {
            let id = u32::from(bytes[i]);
            i += 1;
            id
        };
        let payload = match flags & 0xf0 {
            0 | 16 | 32 => 0,
            48 => 1,
            64 => 2,
            80 => 4,
            96 | 112 => 8,
            128 => {
                let len = usize::from(bytes[i]);
                i += 1;
                len
            }
            144 => {
                let len = usize::from(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
                len
            }
            160 => {
                let len = u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]])
                    as usize;
                i += 4;
                len
            }
            other => panic!("unknown size class {other:#x}"),
        };
        i += payload;
        out.push(Record {
            kind: flags & 3,
            id,
        });
    }
    out
}

/// The field names one event writes, read back out of its own encoding.
fn written_fields(event: &PendingEvent) -> Vec<&'static str> {
    let mut buffer = WamBuffer::new(Channel::Regular, 1, 1);
    write(&mut buffer, event);
    let sites = call_sites_for(name_of(event)).expect("every emitted event is in the catalog");
    decode(buffer.as_bytes())
        .into_iter()
        .filter(|r| r.kind == 2)
        .map(|r| {
            sites
                .field_name(r.id)
                .unwrap_or_else(|| panic!("field id {} is not declared on {}", r.id, sites.event))
        })
        .collect()
}

fn write(buffer: &mut WamBuffer, event: &PendingEvent) {
    match event {
        PendingEvent::E2eMessageRecv(e) => buffer.write_event(e, 1_755_000_000, 1),
        PendingEvent::MessageReceive(e) => buffer.write_event(e, 1_755_000_000, 1),
        PendingEvent::ReceiptStanzaReceive(e) => buffer.write_event(e, 1_755_000_000, 1),
        PendingEvent::WamClientErrors(e) => buffer.write_event(e, 1_755_000_000, 1),
        PendingEvent::WamDroppedEvent(e) => buffer.write_event(e, 1_755_000_000, 1),
        PendingEvent::WebcSocketConnect(e) => buffer.write_event(e, 1_755_000_000, 1),
        PendingEvent::WebWamForceFlush(e) => buffer.write_event(e, 1_755_000_000, 1),
    }
    .expect("every emitted event belongs to the regular channel");
}

fn name_of(event: &PendingEvent) -> &'static str {
    use whatsapp_rust_wam_catalog::WamEvent as _;
    match event {
        PendingEvent::E2eMessageRecv(_) => events::E2eMessageRecv::NAME,
        PendingEvent::MessageReceive(_) => events::MessageReceive::NAME,
        PendingEvent::ReceiptStanzaReceive(_) => events::ReceiptStanzaReceive::NAME,
        PendingEvent::WamClientErrors(_) => events::WamClientErrors::NAME,
        PendingEvent::WamDroppedEvent(_) => events::WamDroppedEvent::NAME,
        PendingEvent::WebcSocketConnect(_) => events::WebcSocketConnect::NAME,
        PendingEvent::WebWamForceFlush(_) => events::WebWamForceFlush::NAME,
    }
}

/// Every event this plugin can emit, with every field it can ever fill set.
///
/// Hand-written, and checked in `no_derivation_writes_an_undeclared_field`
/// against what the derivations actually produce, so it cannot drift below them.
/// The values are arbitrary: this gate is about which fields are written, never
/// about what is in them.
fn maximal_events() -> Vec<PendingEvent> {
    vec![
        PendingEvent::E2eMessageRecv(events::E2eMessageRecv {
            e2e_successful: Some(true),
            e2e_ciphertext_type: Some(enums::E2eCiphertextType::PrekeyMessage),
            e2e_failure_reason: Some(enums::E2eFailureReason::NoSessionAvailable),
            e2e_sender_type: Some(enums::E2eDeviceType::OtherPrimary),
            e2e_destination: Some(enums::E2eDestination::Individual),
            is_lid: Some(true),
            server_addressing_mode: Some(enums::AddressingMode::Lid),
            message_media_type: Some(enums::MediaType::Photo),
            ..Default::default()
        }),
        PendingEvent::MessageReceive(events::MessageReceive {
            message_type: Some(enums::MessageType::Group),
            message_media_type: Some(enums::MediaType::Photo),
            message_is_offline: Some(true),
            is_lid: Some(true),
            e2e_sender_type: Some(enums::E2eDeviceType::OtherPrimary),
            server_addressing_mode: Some(enums::AddressingMode::Lid),
            ..Default::default()
        }),
        PendingEvent::ReceiptStanzaReceive(events::ReceiptStanzaReceive {
            receipt_stanza_type: Some("read".to_string()),
            receipt_stanza_total_count: Some(1),
            receipt_stanza_stage: Some(enums::ReceiptStanzaStage::Overall),
            ..Default::default()
        }),
        PendingEvent::WamClientErrors(events::WamClientErrors {
            wam_client_buffer_drop_error_count: Some(1),
            ..Default::default()
        }),
        PendingEvent::WamDroppedEvent(events::WamDroppedEvent {
            dropped_event_code: Some(450),
            dropped_event_count: Some(1),
            ..Default::default()
        }),
        // Both of these are maximal while empty: WA Web writes no field at
        // either call site, so filling one would be the thing this gate exists
        // to catch.
        PendingEvent::WebcSocketConnect(events::WebcSocketConnect::default()),
        PendingEvent::WebWamForceFlush(events::WebWamForceFlush::default()),
    ]
}

#[test]
fn every_field_this_plugin_writes_is_one_wa_web_writes() {
    let mut checked = 0;
    for event in maximal_events() {
        let name = name_of(&event);
        let sites =
            call_sites_for(name).unwrap_or_else(|| panic!("{name} is not in the call-site table"));
        let union: Vec<&str> = sites.union().collect();
        assert!(
            !sites.sites.is_empty(),
            "{name} has no recovered call site, so nothing can be checked against it"
        );
        assert!(
            !sites.any_partial(),
            "{name} has a partial call site, so its field union is only a lower bound; \
             re-read the site before trusting this check"
        );
        let written = written_fields(&event);
        // An event WA Web constructs without writing a field is a counter, and
        // this plugin has to emit it as one. Checking the two directions apart
        // keeps "the union is empty" from reading as "nothing to check".
        if union.is_empty() {
            assert!(
                written.is_empty(),
                "{name} is written with {} field(s) here and none at any of its \
                 {} call sites in WA Web",
                written.len(),
                sites.sites.len(),
            );
            checked += 1;
            continue;
        }
        for field in written {
            assert!(
                union.contains(&field),
                "{name}.{field} is written by this plugin and by no call site of \
                 {name} in WA Web ({} sites, {} fields)",
                sites.sites.len(),
                union.len(),
            );
            checked += 1;
        }
    }
    // A gate that checked nothing would pass silently.
    assert!(checked >= 20, "only {checked} fields were checked");
}

#[test]
fn no_derivation_writes_an_undeclared_field() {
    use whatsapp_rust::wacore::types::events::EncDecryptFailureReason;
    use whatsapp_rust::wacore::types::message::{AddressingMode, MessageInfo, MessageSource};
    use whatsapp_rust::wacore::types::wire_enums::EncMediaType;
    use whatsapp_rust::wacore_binary::builder::NodeBuilder;
    use whatsapp_rust::wacore_binary::marshal::marshal;
    use whatsapp_rust::wacore_binary::util::unpack;
    use whatsapp_rust::wacore_binary::{Jid, OwnedNodeRef, Server};

    // Inputs chosen so every optional field a derivation can fill is filled:
    // a lid sender on a companion device, in a group, carrying a media type,
    // delivered offline, with an addressing mode and a failure this client maps.
    let info = MessageInfo {
        source: MessageSource {
            chat: Jid::new("15550000000-1600000000", Server::Group),
            sender: Jid {
                user: "15550000001".into(),
                server: Server::Lid,
                device: 2,
                ..Default::default()
            },
            addressing_mode: Some(AddressingMode::Lid),
            ..Default::default()
        },
        media_type: Some(EncMediaType::Image),
        is_offline: true,
        ..Default::default()
    };

    // A read receipt for one message: the shape whose derivation fills every
    // field this plugin can fill on that event.
    let read_receipt = {
        let node = NodeBuilder::new("receipt")
            .attr("id", "STANZA-ID")
            .attr("from", "15550000002@s.whatsapp.net")
            .attr("type", "read")
            .build();
        let packed = marshal(&node).expect("a receipt marshals");
        let bytes = unpack(&packed).expect("a marshalled receipt unpacks");
        OwnedNodeRef::new(bytes.into_owned()).expect("a receipt decodes")
    };

    let produced = [
        PendingEvent::MessageReceive(crate::derive::message_receive(&info)),
        PendingEvent::E2eMessageRecv(crate::derive::e2e_message_recv(
            &info,
            "pkmsg",
            false,
            Some(&EncDecryptFailureReason::NoSession),
        )),
        PendingEvent::ReceiptStanzaReceive(
            crate::derive::receipt_stanza_receive(read_receipt.get())
                .expect("read is a type this client names"),
        ),
    ];

    for event in produced {
        let name = name_of(&event);
        let maximal: Vec<&str> = maximal_events()
            .iter()
            .filter(|candidate| name_of(candidate) == name)
            .flat_map(written_fields)
            .collect();
        let actual = written_fields(&event);
        assert!(
            !actual.is_empty(),
            "{name} produced no fields at all from a fully populated input"
        );
        for field in &actual {
            assert!(
                maximal.contains(field),
                "{name}.{field} is produced by a derivation but missing from the \
                 maximal event the parity gate checks"
            );
        }
    }
}

#[test]
fn the_decoder_reads_back_what_the_encoder_wrote() {
    // The gate is only as good as this: a decoder that silently skipped records
    // would make every parity assertion vacuous.
    let mut buffer = WamBuffer::new(Channel::Regular, 1, 1);
    write(
        &mut buffer,
        &PendingEvent::WamDroppedEvent(events::WamDroppedEvent {
            dropped_event_code: Some(450),
            dropped_event_count: Some(1),
            ..Default::default()
        }),
    );
    let records = decode(buffer.as_bytes());
    assert_eq!(
        records,
        vec![
            // The per-event timestamp global, then the event, then its fields.
            Record { kind: 0, id: 47 },
            Record { kind: 1, id: 4358 },
            Record { kind: 2, id: 1 },
            Record { kind: 2, id: 2 },
        ]
    );
}
