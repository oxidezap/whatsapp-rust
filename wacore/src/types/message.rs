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

#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum PushPriority {
    #[wire = "high"]
    High,
    #[wire = "high_force"]
    HighForce,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum PrivacySensitiveType {
    #[wire = "1"]
    OnDemand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeerMessageOptions {
    push_priority: PushPriority,
    privacy_sensitive: Option<PrivacySensitiveType>,
}

impl Default for PeerMessageOptions {
    fn default() -> Self {
        Self::high()
    }
}

impl PeerMessageOptions {
    const fn new(
        push_priority: PushPriority,
        privacy_sensitive: Option<PrivacySensitiveType>,
    ) -> Self {
        Self {
            push_priority,
            privacy_sensitive,
        }
    }

    pub const fn high() -> Self {
        Self::new(PushPriority::High, None)
    }

    pub const fn high_force() -> Self {
        Self::new(PushPriority::HighForce, None)
    }

    pub const fn high_force_on_demand() -> Self {
        Self::new(
            PushPriority::HighForce,
            Some(PrivacySensitiveType::OnDemand),
        )
    }

    pub const fn push_priority(self) -> PushPriority {
        self.push_priority
    }

    pub const fn privacy_sensitive(self) -> Option<PrivacySensitiveType> {
        self.privacy_sensitive
    }
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

    /// Our own outgoing DM to a user or bot, echoed back to this device
    /// (`is_from_me` with a `recipient`). The server's offline queue only
    /// releases these on a `<receipt type="sender">`, so they must not be
    /// cleared with a bare transport ack. Group/status/newsletter threads are
    /// excluded (`chat` is checked too, since the own-from parser derives
    /// `chat` from `recipient` and leaves `is_group` defaulted).
    pub fn is_self_fanout(&self) -> bool {
        self.is_from_me
            && self.recipient.is_some()
            && !self.is_group
            && !self.chat.is_group()
            && !self.chat.is_status_broadcast()
            && !self.chat.is_newsletter()
    }

    /// The author is a bot but the chat is not a bot chat (WA Web's
    /// `h = !chat.isBot() && author.isBot()`). WA Web clears these with a
    /// bot-invoke-response `<ack>` (`sendBotInvokeResponseAcks`), NOT a
    /// `<receipt>`, so both the success/duplicate ack path and the
    /// decrypt-failure path must route them to the bare ack, not the sender
    /// receipt, even though such an own message is also an [`Self::is_self_fanout`].
    pub fn is_bot_authored_non_bot_chat(&self) -> bool {
        !self.chat.is_bot() && self.sender.is_bot()
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
    /// Broadcast-contact-list recipients from `<participants><to jid>` on an
    /// incoming broadcast/status stanza. Populated only for broadcasts; used to
    /// validate a `deviceSentMessage.phash` (WA Web `validateBclHash`). Empty
    /// otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub bcl_participants: Vec<Jid>,
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
    fn is_self_fanout_matches_only_own_dm_with_recipient() {
        let bot = MessageSource {
            chat: "200000000000002@bot".parse().unwrap(),
            sender: "100000000000001@lid".parse().unwrap(),
            recipient: Some("200000000000002@bot".parse().unwrap()),
            is_from_me: true,
            ..Default::default()
        };
        assert!(bot.is_self_fanout(), "own prompt to a @bot");

        let mut user = bot.clone();
        user.chat = "300000000000003@lid".parse().unwrap();
        user.recipient = Some("300000000000003@lid".parse().unwrap());
        assert!(user.is_self_fanout(), "own DM to a user");

        let mut incoming = bot.clone();
        incoming.is_from_me = false;
        assert!(!incoming.is_self_fanout(), "incoming is not a self-fanout");

        let mut note = bot.clone();
        note.recipient = None;
        assert!(!note.is_self_fanout(), "recipient-less self-note");

        // Load-bearing guard: the own-from parser leaves is_group=false and
        // derives chat from recipient, so a group/status/newsletter self-echo
        // must be excluded by the chat-based checks alone.
        let mut group_chat = bot.clone();
        group_chat.chat = "120363021033254949@g.us".parse().unwrap();
        group_chat.recipient = Some("120363021033254949@g.us".parse().unwrap());
        assert!(!group_chat.is_group);
        assert!(
            !group_chat.is_self_fanout(),
            "group chat excluded by chat.is_group() even with is_group=false"
        );

        let mut group_flag = bot.clone();
        group_flag.is_group = true;
        assert!(!group_flag.is_self_fanout(), "is_group flag excludes");

        let mut status = bot.clone();
        status.chat = "status@broadcast".parse().unwrap();
        assert!(!status.is_self_fanout(), "status broadcast excluded");

        let mut newsletter = bot.clone();
        newsletter.chat = "120363298765432100@newsletter".parse().unwrap();
        assert!(!newsletter.is_self_fanout(), "newsletter excluded");
    }

    #[test]
    fn is_bot_authored_non_bot_chat_matches_wa_web() {
        // WA Web aborts the retry receipt only when `!to.isBot() && participant.isBot()`,
        // with participant == null for DMs. A bot DM (chat == sender == bot) must therefore
        // NOT be suppressed; only a bot reply inside a non-bot group is.
        let bot_dm = MessageSource {
            chat: "200000000000002@bot".parse().unwrap(),
            sender: "200000000000002@bot".parse().unwrap(),
            ..Default::default()
        };
        assert!(
            !bot_dm.is_bot_authored_non_bot_chat(),
            "bot DM must not be suppressed (WA Web sends the retry)"
        );

        let group_bot = MessageSource {
            chat: "120363021033254949@g.us".parse().unwrap(),
            sender: "200000000000002@bot".parse().unwrap(),
            is_group: true,
            ..Default::default()
        };
        assert!(
            group_bot.is_bot_authored_non_bot_chat(),
            "bot reply in a non-bot group is suppressed"
        );

        let user_dm = MessageSource {
            chat: "300000000000003@lid".parse().unwrap(),
            sender: "300000000000003@lid".parse().unwrap(),
            ..Default::default()
        };
        assert!(
            !user_dm.is_bot_authored_non_bot_chat(),
            "normal user DM is never suppressed"
        );
    }

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
    fn peer_message_options_wire_values_match_stanza_attrs() {
        // These literals are owned by the WireEnum attributes above; stanza
        // builders consume the generated as_str() values directly.
        assert_eq!(PushPriority::High.as_str(), "high");
        assert_eq!(PushPriority::HighForce.as_str(), "high_force");
        assert_eq!(PrivacySensitiveType::OnDemand.as_str(), "1");

        let default = PeerMessageOptions::high();
        assert_eq!(default, PeerMessageOptions::default());
        assert_eq!(default.push_priority(), PushPriority::High);
        assert_eq!(default.privacy_sensitive(), None);

        let high_force = PeerMessageOptions::high_force();
        assert_eq!(high_force.push_priority(), PushPriority::HighForce);
        assert_eq!(high_force.privacy_sensitive(), None);

        let on_demand = PeerMessageOptions::high_force_on_demand();
        assert_eq!(on_demand.push_priority(), PushPriority::HighForce);
        assert_eq!(
            on_demand.privacy_sensitive(),
            Some(PrivacySensitiveType::OnDemand)
        );
    }

    #[test]
    fn test_decrypt_fail_hide_logic_for_edits() {
        // Exercise the real rule; both revoke kinds are excluded (WA Web never
        // hides REVOKE and the server drops revokes carrying the attribute).
        let plain = waproto::whatsapp::Message {
            conversation: Some("hi".into()),
            ..Default::default()
        };
        let hide =
            |e: EditAttribute| crate::send::should_hide_decrypt_fail_for_send(Some(&e), &plain);

        assert!(hide(EditAttribute::MessageEdit));
        assert!(hide(EditAttribute::PinInChat));
        assert!(hide(EditAttribute::AdminEdit));

        assert!(!hide(EditAttribute::SenderRevoke));
        assert!(!hide(EditAttribute::Empty));
        assert!(!hide(EditAttribute::AdminRevoke));
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
