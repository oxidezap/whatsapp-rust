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

    /// Wire `edit` value derived from a fully-constructed message body.
    /// Mirrors WAWebSendMsgCommonApi.editAttribute. Used both for the initial
    /// send (so the outer `<message>` carries the right attribute) and for the
    /// retry-resend path (which has no other signal source than the cached
    /// protobuf).
    ///
    /// `from_me` for the protocolMessage Revoke branch comes from
    /// `protocolMessage.key.fromMe` as a proxy for the `subtype` argument WA
    /// Web threads through from the MessageRecord. The convention is that an
    /// admin revoking someone else's message sets `fromMe=false`.
    pub fn infer_from_message(msg: &waproto::whatsapp::Message) -> Option<Self> {
        use waproto::whatsapp::message::protocol_message::Type as ProtocolType;
        use waproto::whatsapp::message::secret_encrypted_message::SecretEncType;

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
        if let Some(sec) = msg.secret_encrypted_message.as_ref()
            && let Some(enc_type) = sec.secret_enc_type
            && (enc_type == SecretEncType::MessageEdit as i32
                || enc_type == SecretEncType::EventEdit as i32)
        {
            return Some(Self::MessageEdit);
        }
        // Reaction with empty text == sender-revoke of a previous reaction.
        if let Some(react) = msg.reaction_message.as_ref()
            && react.text.as_deref() == Some("")
        {
            return Some(Self::SenderRevoke);
        }
        // KeepInChat UNDO_KEEP_FOR_ALL is a sender-revoke at the wire level.
        if let Some(keep) = msg.keep_in_chat_message.as_ref()
            && keep.key.as_ref().and_then(|k| k.from_me) == Some(true)
            && keep.keep_type == Some(waproto::whatsapp::KeepType::UndoKeepForAll as i32)
        {
            return Some(Self::SenderRevoke);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum BotEditType {
    #[wire = "first"]
    First,
    #[wire = "inner"]
    Inner,
    #[wire = "last"]
    Last,
}

impl BotEditType {
    /// Parse the wire string from the `<bot edit="…">` attribute.
    pub fn from_wire(s: &str) -> Option<Self> {
        Self::try_from(s).ok()
    }
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
    /// `<meta target_chat_jid="…">` — present when the bot reply addresses a
    /// chat distinct from the stanza-level `from` (used for msmsg secret
    /// lookup; see WA Web `decryptMsmsgBotMessage`).
    pub target_chat: Option<Jid>,
    pub deprecated_lid_session: Option<bool>,
    pub thread_message_id: Option<MessageId>,
    pub thread_message_sender_jid: Option<Jid>,
    /// `<meta content_type=...>` attr. Server marks reactions/edits as
    /// `"add_on"`; mirrors `WAWebHandleMsgParser` b()'s metadata read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// `<meta appdata=...>` attr. `"default"` is the only observed value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub appdata: Option<String>,
    /// `<reporting><reporting_tag>` content bytes (16 or 20). Pre-requisite
    /// for the server-side report-abuse flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_tag: Option<Vec<u8>>,
    /// `<reporting><reporting_token>` content bytes (16). Pre-requisite
    /// for the server-side report-abuse flow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_token: Option<Vec<u8>>,
    /// `v` attr on `<reporting_token>`. WA Web defaults to 1 when missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reporting_token_version: Option<i64>,
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
    /// Server-store timestamp in microseconds (envelope `sts` attr). Used by
    /// WA Web for read-self watermark ordering across companion devices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_timestamp_us: Option<i64>,
    /// Envelope `verified_level` attr (e.g. "unknown"/"low"/"high"). For
    /// business messages this is the server-asserted verification tier; for
    /// regular messages it is absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_level: Option<String>,
    /// Envelope `verified_name` int attr (business name certificate serial).
    /// Separate from the `verified_name` child cert bytes already on this
    /// struct.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_name_serial: Option<i64>,
    /// Envelope `peer_recipient_pn` attr. Present on companion-device
    /// self-synced DM stanzas to identify the peer's PN (so the receipt
    /// goes to the right routing target).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_recipient_pn: Option<Jid>,
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
