//! Auto-generated protobuf definitions for the WhatsApp wire format.
//!
//! The Rust source (`whatsapp.rs`) is produced by `build.rs` from the
//! pre-compiled descriptor set `whatsapp.desc`, and written to `OUT_DIR` —
//! not tracked in git. To regenerate the descriptor after editing
//! `whatsapp.proto`, run `scripts/regenerate-proto-desc.sh` (wraps `protoc`).

#![allow(clippy::large_enum_variant)]
pub mod whatsapp {
    #![allow(
        non_camel_case_types,
        non_snake_case,
        unreachable_patterns,
        clippy::derivable_impls,
        clippy::match_single_binding,
        clippy::needless_else
    )]
    #[rustfmt::skip]
    buffa::include_proto!("whatsapp");
}

#[cfg(test)]
mod tests {
    use super::whatsapp as wa;
    use buffa::Message;
    use buffa::view::MessageView;

    #[test]
    fn generated_views_and_oneofs_round_trip() {
        let msg = wa::Message {
            interactive_message: buffa::MessageField::some(wa::message::InteractiveMessage {
                interactiveMessage: Some(
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
            interactive.interactiveMessage.as_ref()
        else {
            panic!("expected native flow oneof");
        };
        assert_eq!(native.buttons[0].name.as_deref(), Some("quick_reply"));

        let view = wa::MessageView::decode_view(&bytes).unwrap();
        let interactive = view.interactive_message.as_option().unwrap();
        let Some(wa::message::interactive_message::InteractiveMessageView::NativeFlowMessage(
            native,
        )) = interactive.interactiveMessage.as_ref()
        else {
            panic!("expected native flow view oneof");
        };
        assert_eq!(native.buttons[0].name, Some("quick_reply"));
    }
}
