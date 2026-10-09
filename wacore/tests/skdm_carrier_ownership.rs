#![allow(clippy::disallowed_methods)]

use buffa::{Message as _, MessageField};
use wacore::messages::is_sender_key_distribution_only;
use waproto::whatsapp as wa;

fn carrier_addresses(message: &wa::Message) -> [usize; 3] {
    [
        message
            .sender_key_distribution_message
            .as_option()
            .map_or(0, |value| value as *const _ as usize),
        message
            .fast_ratchet_key_sender_key_distribution_message
            .as_option()
            .map_or(0, |value| value as *const _ as usize),
        message
            .message_context_info
            .as_option()
            .map_or(0, |value| value as *const _ as usize),
    ]
}

#[test]
fn carrier_classification_preserves_boxes_and_all_wire_fields() {
    let future = [0xc2, 0x3e, 4, 11, 22, 33, 44];
    for mask in 0..8 {
        for content in ["none", "text", "future", "empty_child"] {
            let mut message = if content == "future" {
                wa::Message::decode_from_slice(&future).unwrap()
            } else {
                wa::Message::default()
            };
            if mask & 1 != 0 {
                message.sender_key_distribution_message = MessageField::some(
                    wa::message::SenderKeyDistributionMessage::decode_from_slice(&future).unwrap(),
                );
            }
            if mask & 2 != 0 {
                message.fast_ratchet_key_sender_key_distribution_message = MessageField::some(
                    wa::message::SenderKeyDistributionMessage::decode_from_slice(&future).unwrap(),
                );
            }
            if mask & 4 != 0 {
                message.message_context_info =
                    MessageField::some(wa::MessageContextInfo::decode_from_slice(&future).unwrap());
            }
            if content == "text" {
                message.conversation = Some("Synthetic content".into());
            } else if content == "empty_child" {
                message.buttons_message =
                    MessageField::some(wa::message::ButtonsMessage::default());
            }
            let wire = message.encode_to_vec();
            let addresses = carrier_addresses(&message);
            assert_eq!(
                is_sender_key_distribution_only(&mut message),
                mask & 3 != 0 && content == "none",
                "mask={mask}, content={content}"
            );
            assert_eq!(
                message.encode_to_vec(),
                wire,
                "mask={mask}, content={content}"
            );
            assert_eq!(
                carrier_addresses(&message),
                addresses,
                "mask={mask}, content={content}"
            );
        }
    }
}
