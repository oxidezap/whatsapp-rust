//! Downstream contracts across the SDK's public dependency reexports.
#![cfg(feature = "sdk")]

use buffa::{Message as _, MessageView as _};
use whatsapp_rust::{buffa, waproto::whatsapp as wa};

#[test]
fn open_envelope_enums_support_named_values_unknown_payloads_and_future_variants() {
    use whatsapp_rust::wacore::types::message::{EncMediaType, StanzaMessageType};

    let unknown = StanzaMessageType::from("synthetic-future-kind");
    let payload = match unknown {
        StanzaMessageType::Unknown(payload) => Some(payload),
        _ => None,
    };
    assert_eq!(payload.as_deref(), Some("synthetic-future-kind"));

    let media = EncMediaType::from("image");
    assert!(matches!(media, EncMediaType::Image));
    let raw = EncMediaType::from("synthetic-future-media");
    match raw {
        EncMediaType::Unknown(payload) => assert_eq!(payload, "synthetic-future-media"),
        _ => panic!("future media value must retain its payload"),
    }
}

#[test]
fn dependency_reexports_accept_the_same_public_types() {
    // A consumer can mix direct core imports with the SDK without adapters.
    let direct: wacore_binary::Jid = "12345@s.whatsapp.net".parse().unwrap();
    let exported: whatsapp_rust::Jid = direct;
    assert_eq!(exported.to_string(), "12345@s.whatsapp.net");

    let operation = whatsapp_rust::mex_operation!(
        whatsapp_rust::wacore::iq::mex_operations::create_labyrinth_backup
    );
    let mut variables = wacore::iq::mex_operations::create_labyrinth_backup::Variables::default();
    variables.input = Some("synthetic".into());
    let _request = operation.request(variables);

    let mut message = wa::Message::default();
    message.conversation = Some("synthetic".into());
    let field: whatsapp_rust::buffa::MessageField<wa::Message> = message.into();
    let _: whatsapp_rust::waproto::buffa::MessageField<wa::Message> = field;
}

#[test]
fn future_wire_fields_require_original_bytes_for_forwarding() {
    // conversation = "x", followed by synthetic unknown field 50000 = 7.
    // This fixture is independent of the generated encoder.
    let original = [0x0a, 1, b'x', 0x80, 0xb5, 0x18, 7];
    let decoded = wa::Message::decode_from_slice(&original).unwrap();
    let wa::Message { conversation, .. } = &decoded;
    assert_eq!(conversation.as_deref(), Some("x"));
    assert_eq!(decoded.encode_to_vec(), [0x0a, 1, b'x']);

    let view = wa::MessageView::decode_view(&original).unwrap();
    let wa::MessageView { conversation, .. } = &view;
    assert_eq!(*conversation, Some("x"));
    assert_eq!(view.to_owned_message().unwrap(), decoded);
}

#[test]
fn borrowed_message_can_be_promoted_before_the_input_is_released() {
    let owned = {
        let input = vec![0x0a, 3, b'o', b'l', b'd'];
        let view = wa::MessageView::decode_view(&input).unwrap();
        let text = view.conversation.unwrap();
        // The borrowed representation really points into the caller's input.
        assert_eq!(text.as_ptr(), input[2..].as_ptr());
        view.to_owned_message().unwrap()
    };
    assert_eq!(owned.conversation.as_deref(), Some("old"));
}
