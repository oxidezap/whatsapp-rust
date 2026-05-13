use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wacore_binary::{Jid, JidExt, MessageId, MessageServerId};
use waproto::whatsapp as wa;

use crate::WireEnum;

/// Identifies a specific message within a chat.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatMessageId {
    pub chat: Jid,
    pub id: MessageId,
}

impl ChatMessageId {
    pub fn new(chat: Jid, id: MessageId) -> Self {
        Self { chat, id }
    }
}

/// Addressing mode for a group (phone number vs LID).
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum AddressingMode {
    #[wire_default]
    #[wire = "pn"]
    Pn,
    #[wire = "lid"]
    Lid,
}

#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum MessageCategory {
    #[wire_default]
    #[wire = ""]
    Empty,
    #[wire = "peer"]
    Peer,
    #[wire_fallback]
    Other(String),
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MessageSource {
    pub chat: Jid,
    pub sender: Jid,
    pub is_from_me: bool,
    pub is_group: bool,
    pub addressing_mode: Option<AddressingMode>,
    pub sender_alt: Option<Jid>,
    pub recipient_alt: Option<Jid>,
    pub broadcast_list_owner: Option<Jid>,
    pub recipient: Option<Jid>,
}

impl MessageSource {
    pub fn is_incoming_broadcast(&self) -> bool {
        (!self.is_from_me || self.broadcast_list_owner.is_some()) && self.chat.is_broadcast_list()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceSentMeta {
    pub destination_jid: String,
    pub phash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, crate::WireEnum)]
pub enum EditAttribute {
    #[wire_default]
    #[wire = ""]
    Empty,
    #[wire = "1"]
    MessageEdit,
    #[wire = "2"]
    PinInChat,
    #[wire = "3"]
    AdminEdit,
    #[wire = "7"]
    SenderRevoke,
    #[wire = "8"]
    AdminRevoke,
    #[wire_fallback]
    Unknown(String),
}

impl From<String> for EditAttribute {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}

impl EditAttribute {
    /// Returns the wire-format string value for the edit attribute.
    /// Preserves the original wire value for Unknown variants.
    pub fn to_string_val(&self) -> &str {
        self.as_str()
    }

    /// Recover the wire `edit` value from a cached protobuf so the retry path can
    /// re-emit `edit="N"` on resends. The original `subtype` (used by WA Web's
    /// `editAttribute`) isn't persisted, so `key.from_me` is used as a proxy to
    /// distinguish admin-vs-sender revoke: WA Web sets `from_me=false` on the
    /// revoke proto's `MessageKey` when an admin revokes someone else's message.
    pub fn infer_from_message(msg: &waproto::whatsapp::Message) -> Option<Self> {
        use waproto::whatsapp::message::protocol_message::Type as ProtocolType;

        // The operation signal can be nested under any neutral wrapper.
        let msg = crate::send::unwrap_message(msg);

        if msg.pin_in_chat_message.is_some() {
            return Some(Self::PinInChat);
        }
        if msg.edited_message.is_some() {
            return Some(Self::MessageEdit);
        }
        if let Some(pm) = msg.protocol_message.as_deref() {
            if pm.r#type == Some(ProtocolType::Revoke as i32) {
                let from_me = pm.key.as_ref().and_then(|k| k.from_me).unwrap_or(false);
                return Some(if from_me {
                    Self::SenderRevoke
                } else {
                    Self::AdminRevoke
                });
            }
            if pm.r#type == Some(ProtocolType::MessageEdit as i32) || pm.edited_message.is_some() {
                return Some(Self::MessageEdit);
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum BotEditType {
    First,
    Inner,
    Last,
}

#[derive(Debug, Clone, Serialize)]
pub struct MsgBotInfo {
    pub edit_type: Option<BotEditType>,
    pub edit_target_id: Option<MessageId>,
    pub edit_sender_timestamp_ms: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MsgMetaInfo {
    pub target_id: Option<MessageId>,
    pub target_sender: Option<Jid>,
    pub deprecated_lid_session: Option<bool>,
    pub thread_message_id: Option<MessageId>,
    pub thread_message_sender_jid: Option<Jid>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct MessageInfo {
    pub source: MessageSource,
    pub id: MessageId,
    pub server_id: MessageServerId,
    pub r#type: String,
    pub push_name: String,
    pub timestamp: DateTime<Utc>,
    pub category: MessageCategory,
    pub multicast: bool,
    pub media_type: String,
    pub edit: EditAttribute,
    pub bot_info: Option<MsgBotInfo>,
    pub meta_info: MsgMetaInfo,
    pub verified_name: Option<wa::VerifiedNameCertificate>,
    pub device_sent_meta: Option<DeviceSentMeta>,
    /// Ephemeral duration in seconds, extracted from `contextInfo.expiration`.
    pub ephemeral_expiration: Option<u32>,
    /// Whether this message was delivered during offline sync.
    pub is_offline: bool,
    /// Set when this message was recovered via PDO rather than normal decryption.
    /// Contains the PDO request message ID.
    pub unavailable_request_id: Option<String>,
}

impl MessageInfo {
    /// WA Web: expired status messages (>24h) are silently dropped — no retry receipts,
    /// no undecryptable events. Matches `WAWebMsgProcessingDecryptionHandler.E()`.
    pub fn is_expired_status(&self) -> bool {
        self.source.chat.is_status_broadcast()
            && (crate::time::now_utc() - self.timestamp) > chrono::Duration::hours(24)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_attribute_parsing_and_serialization() {
        // Test all known edit attribute values
        let attrs = vec![
            ("", EditAttribute::Empty),
            ("1", EditAttribute::MessageEdit),
            ("2", EditAttribute::PinInChat),
            ("3", EditAttribute::AdminEdit),
            ("7", EditAttribute::SenderRevoke),
            ("8", EditAttribute::AdminRevoke),
        ];

        for (string_val, expected_attr) in attrs {
            let parsed = EditAttribute::from(string_val.to_string());
            assert_eq!(parsed, expected_attr);
            assert_eq!(parsed.to_string_val(), string_val);
        }

        // Unknown values should be preserved (round-trip the wire value)
        assert_eq!(
            EditAttribute::from("99".to_string()),
            EditAttribute::Unknown("99".to_string())
        );
        assert_eq!(
            EditAttribute::Unknown("anything".to_string()).to_string_val(),
            "anything"
        );
    }

    #[test]
    fn test_decrypt_fail_hide_logic_for_edits() {
        // Documents the logic used in prepare_group_stanza (wacore/src/send.rs).
        // The decrypt-fail="hide" attribute is added for edited messages to hide
        // failed decryption attempts. However, admin revokes should NOT have it
        // because WhatsApp Web doesn't include it, and the server rejects it.

        fn should_add_decrypt_fail_hide(edit: &EditAttribute) -> bool {
            *edit != EditAttribute::Empty && *edit != EditAttribute::AdminRevoke
        }

        // Should add decrypt-fail="hide"
        assert!(should_add_decrypt_fail_hide(&EditAttribute::MessageEdit));
        assert!(should_add_decrypt_fail_hide(&EditAttribute::PinInChat));
        assert!(should_add_decrypt_fail_hide(&EditAttribute::AdminEdit));
        assert!(should_add_decrypt_fail_hide(&EditAttribute::SenderRevoke));

        // Should NOT add decrypt-fail="hide"
        assert!(!should_add_decrypt_fail_hide(&EditAttribute::Empty));
        assert!(!should_add_decrypt_fail_hide(&EditAttribute::AdminRevoke));
    }

    #[test]
    fn infer_from_message_admin_revoke() {
        let msg = waproto::whatsapp::Message {
            protocol_message: Some(Box::new(waproto::whatsapp::message::ProtocolMessage {
                key: Some(waproto::whatsapp::MessageKey {
                    from_me: Some(false),
                    ..Default::default()
                }),
                r#type: Some(waproto::whatsapp::message::protocol_message::Type::Revoke as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&msg),
            Some(EditAttribute::AdminRevoke)
        );
    }

    #[test]
    fn infer_from_message_sender_revoke() {
        let msg = waproto::whatsapp::Message {
            protocol_message: Some(Box::new(waproto::whatsapp::message::ProtocolMessage {
                key: Some(waproto::whatsapp::MessageKey {
                    from_me: Some(true),
                    ..Default::default()
                }),
                r#type: Some(waproto::whatsapp::message::protocol_message::Type::Revoke as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&msg),
            Some(EditAttribute::SenderRevoke)
        );
    }

    #[test]
    fn infer_from_message_top_level_edit() {
        let msg = waproto::whatsapp::Message {
            edited_message: Some(Box::new(waproto::whatsapp::message::FutureProofMessage {
                message: Some(Box::new(waproto::whatsapp::Message::default())),
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&msg),
            Some(EditAttribute::MessageEdit)
        );
    }

    #[test]
    fn infer_from_message_legacy_edit() {
        let msg = waproto::whatsapp::Message {
            protocol_message: Some(Box::new(waproto::whatsapp::message::ProtocolMessage {
                edited_message: Some(Box::new(waproto::whatsapp::Message::default())),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&msg),
            Some(EditAttribute::MessageEdit)
        );
    }

    #[test]
    fn infer_from_message_message_edit_sender() {
        let msg = waproto::whatsapp::Message {
            protocol_message: Some(Box::new(waproto::whatsapp::message::ProtocolMessage {
                key: Some(waproto::whatsapp::MessageKey {
                    from_me: Some(true),
                    ..Default::default()
                }),
                r#type: Some(
                    waproto::whatsapp::message::protocol_message::Type::MessageEdit as i32,
                ),
                edited_message: Some(Box::new(waproto::whatsapp::Message::default())),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&msg),
            Some(EditAttribute::MessageEdit)
        );
    }

    #[test]
    fn infer_from_message_plain_returns_none() {
        let msg = waproto::whatsapp::Message {
            conversation: Some("plain".into()),
            ..Default::default()
        };
        assert_eq!(EditAttribute::infer_from_message(&msg), None);
    }

    #[test]
    fn infer_from_message_unwraps_neutral_wrappers() {
        let inner_revoke = waproto::whatsapp::Message {
            protocol_message: Some(Box::new(waproto::whatsapp::message::ProtocolMessage {
                key: Some(waproto::whatsapp::MessageKey {
                    from_me: Some(false),
                    ..Default::default()
                }),
                r#type: Some(waproto::whatsapp::message::protocol_message::Type::Revoke as i32),
                ..Default::default()
            })),
            ..Default::default()
        };
        let wrapped = waproto::whatsapp::Message {
            ephemeral_message: Some(Box::new(waproto::whatsapp::message::FutureProofMessage {
                message: Some(Box::new(inner_revoke)),
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&wrapped),
            Some(EditAttribute::AdminRevoke)
        );

        // Same for pin wrapped in view_once and device_sent (double nesting).
        let inner_pin = waproto::whatsapp::Message {
            pin_in_chat_message: Some(waproto::whatsapp::message::PinInChatMessage::default()),
            ..Default::default()
        };
        let wrapped_pin = waproto::whatsapp::Message {
            device_sent_message: Some(Box::new(waproto::whatsapp::message::DeviceSentMessage {
                destination_jid: Some(String::new()),
                message: Some(Box::new(waproto::whatsapp::Message {
                    view_once_message: Some(Box::new(
                        waproto::whatsapp::message::FutureProofMessage {
                            message: Some(Box::new(inner_pin)),
                        },
                    )),
                    ..Default::default()
                })),
                ..Default::default()
            })),
            ..Default::default()
        };
        assert_eq!(
            EditAttribute::infer_from_message(&wrapped_pin),
            Some(EditAttribute::PinInChat)
        );
    }
}
