use crate::libsignal::crypto::CryptographicHash;
use anyhow::{Result, anyhow};
use base64::Engine as _;
use prost::Message as ProtoMessage;
use waproto::whatsapp as wa;

pub struct MessageUtils;

impl MessageUtils {
    fn random_pad_len() -> u8 {
        use rand::RngExt;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let v = rng.random::<u8>() & 0x0F;
        if v == 0 { 0x0F } else { v }
    }

    pub fn pad_message_v2(mut plaintext: Vec<u8>) -> Vec<u8> {
        let pad = Self::random_pad_len();
        plaintext.resize(plaintext.len() + pad as usize, pad);
        plaintext
    }

    /// Encode + pad in a single pre-sized allocation.
    pub fn encode_and_pad(msg: &wa::Message) -> Vec<u8> {
        let pad = Self::random_pad_len();
        let mut buf = Vec::with_capacity(msg.encoded_len() + pad as usize);
        msg.encode(&mut buf).expect("encode into pre-sized Vec");
        buf.resize(buf.len() + pad as usize, pad);
        buf
    }

    pub fn participant_list_hash(devices: &[wacore_binary::Jid]) -> Result<String> {
        // Hash sorted ad_strings incrementally (avoids join() allocation).
        let mut jids: Vec<String> = devices.iter().map(|j| j.to_ad_string()).collect();
        jids.sort_unstable();

        let mut h = CryptographicHash::new("SHA-256")
            .map_err(|e| anyhow!("failed to initialize SHA-256 hasher: {:?}", e))?;
        for jid in &jids {
            h.update(jid.as_bytes());
        }

        let full_hash = h
            .finalize_sha256_array()
            .map_err(|e| anyhow!("failed to finalize hash: {:?}", e))?;

        Ok(format!(
            "2:{hash}",
            hash = base64::prelude::BASE64_URL_SAFE_NO_PAD.encode(&full_hash[..6])
        ))
    }

    pub fn unpad_message_ref(plaintext: &[u8], version: u8) -> Result<&[u8]> {
        if version == 3 {
            return Ok(plaintext);
        }
        if plaintext.is_empty() {
            return Err(anyhow::anyhow!("plaintext is empty, cannot unpad"));
        }
        let pad_len = plaintext[plaintext.len() - 1] as usize;
        if pad_len == 0 || pad_len > plaintext.len() {
            return Err(anyhow::anyhow!("invalid padding length: {}", pad_len));
        }
        let (data, padding) = plaintext.split_at(plaintext.len() - pad_len);
        for &byte in padding {
            if byte != pad_len as u8 {
                return Err(anyhow::anyhow!("invalid padding bytes"));
            }
        }
        Ok(data)
    }
}

/// Decode padded ciphertext into a `wa::Message`.
///
/// Unpads the plaintext (using the given padding version) and decodes the
/// protobuf bytes into a WhatsApp Message. This is the pure,
/// runtime-independent portion of `handle_decrypted_plaintext`.
pub fn decode_plaintext(padded_plaintext: &[u8], padding_version: u8) -> Result<wa::Message> {
    let plaintext_slice = MessageUtils::unpad_message_ref(padded_plaintext, padding_version)?;
    wa::Message::decode(plaintext_slice)
        .map_err(|e| anyhow::anyhow!("Failed to decode decrypted plaintext: {e}"))
}

/// Unwrap a DeviceSentMessage wrapper, returning the inner message.
///
/// When a message is sent from our own device, the actual content is nested
/// inside `device_sent_message.message`.  This function extracts that inner
/// message (preserving `message_context_info`), or returns the original
/// message unchanged when there is no wrapper or the wrapper has no inner
/// message.
pub fn unwrap_device_sent(mut msg: wa::Message) -> wa::Message {
    if let Some(mut dsm) = msg.device_sent_message.take() {
        if let Some(mut inner) = dsm.message.take() {
            inner.message_context_info = crate::proto_helpers::merge_dsm_context(
                inner.message_context_info.take(),
                msg.message_context_info.as_ref(),
            );
            return *inner;
        }
        msg.device_sent_message = Some(dsm);
    }
    msg
}

/// Returns `true` if the message contains only a SenderKey distribution
/// (internal key-exchange for group encryption) and no user-visible content.
///
/// When sending a group message, WhatsApp includes the SKDM in a separate
/// `pkmsg` enc node.  We must process it (store the sender key) but should
/// not surface it as a user event.
pub fn is_sender_key_distribution_only(msg: &wa::Message) -> bool {
    if msg.sender_key_distribution_message.is_none()
        && msg
            .fast_ratchet_key_sender_key_distribution_message
            .is_none()
    {
        return false;
    }

    // Fast path: most common user-visible fields (avoids clone for the typical case).
    if msg.conversation.is_some()
        || msg.extended_text_message.is_some()
        || msg.image_message.is_some()
        || msg.video_message.is_some()
        || msg.audio_message.is_some()
        || msg.document_message.is_some()
        || msg.reaction_message.is_some()
        || msg.protocol_message.is_some()
    {
        return false;
    }

    // Slow path: clone and compare to default to catch all current and future fields.
    let mut stripped = msg.clone();
    stripped.sender_key_distribution_message = None;
    stripped.fast_ratchet_key_sender_key_distribution_message = None;
    stripped.message_context_info = None;
    stripped == wa::Message::default()
}

/// Parse a message stanza into a `MessageInfo` struct.
///
/// This is a pure function that extracts message metadata from a node's
/// attributes. It requires the own JID and optional LID to determine
/// `is_from_me`.
pub fn parse_message_info(
    node: &wacore_binary::NodeRef<'_>,
    own_jid: &wacore_binary::Jid,
    own_lid: Option<&wacore_binary::Jid>,
) -> Result<crate::types::message::MessageInfo> {
    use crate::types::message::{
        AddressingMode, EditAttribute, MessageCategory, MessageInfo, MessageSource,
    };
    use wacore_binary::{JidExt as _, STATUS_BROADCAST_USER, Server};

    let mut attrs = node.attrs();
    let from = attrs.jid("from");
    let addressing_mode = attrs
        .optional_string("addressing_mode")
        .and_then(|s| AddressingMode::try_from(s.as_ref()).ok());

    let mut source = if from.server == Server::Broadcast {
        let participant = attrs.jid("participant");
        let is_from_me = participant.matches_user_or_lid(own_jid, own_lid);

        // Match WAWebMsgParser: read participant_lid/_pn unconditionally so
        // the LID-PN cache can re-warm from the stanza.
        let sender_alt = if participant.server.is_pn_family() {
            attrs.optional_jid("participant_lid")
        } else if participant.server.is_lid_family() {
            attrs.optional_jid("participant_pn")
        } else {
            None
        };

        MessageSource {
            chat: from.clone(),
            sender: participant.clone(),
            is_from_me,
            is_group: true,
            broadcast_list_owner: if from.user != STATUS_BROADCAST_USER {
                Some(participant.clone())
            } else {
                None
            },
            sender_alt,
            ..Default::default()
        }
    } else if from.is_group() {
        let sender = attrs.jid("participant");
        let sender_alt = match addressing_mode {
            Some(AddressingMode::Lid) => attrs.optional_jid("participant_pn"),
            Some(AddressingMode::Pn) => attrs.optional_jid("participant_lid"),
            None => None,
        };

        let is_from_me = sender.matches_user_or_lid(own_jid, own_lid);

        MessageSource {
            chat: from.clone(),
            sender: sender.clone(),
            is_from_me,
            is_group: true,
            sender_alt,
            ..Default::default()
        }
    } else if from.matches_user_or_lid(own_jid, own_lid) {
        let recipient = attrs.optional_jid("recipient");
        let chat = recipient
            .as_ref()
            .map(|r| r.to_non_ad())
            .unwrap_or_else(|| from.to_non_ad());
        // Populate sender_alt so LID-PN cache warms from self-messages
        let sender_alt = if from.server == Server::Lid {
            Some(own_jid.clone())
        } else if from.server == Server::Pn && own_lid.is_some() {
            own_lid.cloned()
        } else {
            None
        };
        MessageSource {
            chat,
            sender: from.clone(),
            is_from_me: true,
            recipient,
            sender_alt,
            ..Default::default()
        }
    } else {
        let sender_alt = if from.server == Server::Lid {
            attrs.optional_jid("sender_pn")
        } else {
            attrs.optional_jid("sender_lid")
        };

        MessageSource {
            chat: from.to_non_ad(),
            sender: from.clone(),
            is_from_me: false,
            sender_alt,
            ..Default::default()
        }
    };

    source.addressing_mode = addressing_mode;

    let category = attrs
        .optional_string("category")
        .map(|s| MessageCategory::from(s.as_ref()))
        .unwrap_or_default();

    let id = attrs.required_string("id")?.to_string();
    let server_id = attrs
        .optional_u64("server_id")
        .filter(|&v| (99..=2_147_476_647).contains(&v))
        .unwrap_or(0) as i32;

    if source.chat.is_newsletter() {
        source.chat.device = 0;
        source.chat.agent = 0;
    }

    let is_offline = attrs.optional_string("offline").is_some();

    // Envelope enrichment (mirrors WAWebHandleMsgParser y() function).
    let server_timestamp_us = attrs
        .optional_u64("sts")
        .and_then(|v| i64::try_from(v).ok());
    let verified_level = attrs
        .optional_string("verified_level")
        .map(|s| s.into_owned());
    let verified_name_serial = attrs
        .optional_u64("verified_name")
        .and_then(|v| i64::try_from(v).ok());
    let peer_recipient_pn = attrs.optional_jid("peer_recipient_pn");

    // <meta> child attrs (WAWebHandleMsgParser b()) and <reporting> children
    // (I() function). Both are optional; absence is the common case.
    let mut meta_info = crate::types::message::MsgMetaInfo::default();
    if let Some(meta) = node.get_optional_child("meta") {
        let mut ma = meta.attrs();
        meta_info.content_type = ma.optional_string("content_type").map(|s| s.into_owned());
        meta_info.appdata = ma.optional_string("appdata").map(|s| s.into_owned());
        // msmsg addon path needs the trio (target_id, target_sender_jid,
        // target_chat_jid) to look up the parent messageSecret.
        meta_info.target_id = ma.optional_string("target_id").map(|s| s.into_owned());
        meta_info.target_sender = ma.optional_jid("target_sender_jid");
        meta_info.target_chat = ma.optional_jid("target_chat_jid");
    }
    if let Some(reporting) = node.get_optional_child("reporting")
        && let Some(tag) = reporting.get_optional_child("reporting_tag")
    {
        meta_info.reporting_tag = tag.content_bytes().map(|b| b.to_vec());
    }
    if let Some(reporting) = node.get_optional_child("reporting")
        && let Some(token) = reporting.get_optional_child("reporting_token")
    {
        meta_info.reporting_token = token.content_bytes().map(|b| b.to_vec());
        // WA Web `I()`: `c.maybeAttrInt("v")!=null?_:1`. Missing `v` is
        // not a parse failure — token format version defaults to 1.
        meta_info.reporting_token_version = Some(
            token
                .attrs()
                .optional_u64("v")
                .and_then(|v| i64::try_from(v).ok())
                .unwrap_or(1),
        );
    }

    // <bot edit="..."> child. Mirror WA Web `f()`: read `edit_target_id`
    // unconditionally so the msmsg regular-bot fallback path can consume it
    // regardless of edit_type. fbid (`h()`) only uses it for INNER/LAST,
    // but parsing it always is a strict superset.
    let bot_info = node.get_optional_child("bot").map(|bot_node| {
        let mut ba = bot_node.attrs();
        crate::types::message::MsgBotInfo {
            edit_type: ba
                .optional_string("edit")
                .and_then(|s| crate::types::message::BotEditType::from_wire(s.as_ref())),
            edit_target_id: ba.optional_string("edit_target_id").map(|s| s.into_owned()),
            edit_sender_timestamp_ms: ba
                .optional_u64("sender_timestamp_ms")
                .and_then(|ms| i64::try_from(ms).ok())
                .and_then(crate::time::from_millis),
        }
    });

    Ok(MessageInfo {
        source,
        id,
        server_id,
        push_name: attrs
            .optional_string("notify")
            .map(|s| s.to_string())
            .unwrap_or_default(),
        timestamp: crate::time::from_secs_or_now(attrs.unix_time("t")),
        category,
        edit: attrs
            .optional_string("edit")
            .map(|s| EditAttribute::from(s.to_string()))
            .unwrap_or_default(),
        is_offline,
        server_timestamp_us,
        verified_level,
        verified_name_serial,
        peer_recipient_pn,
        meta_info,
        bot_info,
        ..Default::default()
    })
}

#[cfg(test)]
mod parse_message_info_tests {
    use super::*;
    use std::str::FromStr;
    use wacore_binary::Jid;
    use wacore_binary::builder::NodeBuilder;

    #[test]
    fn status_broadcast_with_participant_lid_populates_sender_alt() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let own_lid = Jid::from_str("100000000000000@lid").unwrap();
        let pn_user = "559980000001";
        let lid_user = "100000012345678";
        let node = NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("type", "media")
            .attr("id", "TEST_MSG_ID")
            .attr("t", "1777415965")
            .attr("participant", format!("{pn_user}@s.whatsapp.net").as_str())
            .attr("participant_lid", format!("{lid_user}@lid").as_str())
            .build();

        let info = parse_message_info(&node.as_node_ref(), &own_pn, Some(&own_lid))
            .expect("parse_message_info should succeed for status broadcast");

        assert_eq!(info.source.sender.user, pn_user);
        assert_eq!(info.source.sender.server, wacore_binary::Server::Pn);
        let alt = info
            .source
            .sender_alt
            .as_ref()
            .expect("status broadcast must expose participant_lid as sender_alt");
        assert_eq!(alt.user, lid_user);
        assert_eq!(alt.server, wacore_binary::Server::Lid);
    }

    /// Envelope-enrichment attributes (`sts`, `verified_level`,
    /// `verified_name`, `peer_recipient_pn`) flow into `MessageInfo` fields.
    /// Mirrors `WAWebHandleMsgParser` y() function which threads
    /// `serverStoreTimeMicros`/`verifiedLevel`/`verifiedNameSerial`/
    /// `peerRecipientPn` into the msgInfo result.
    #[test]
    fn envelope_enrichment_fields_are_captured() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-ENV-1")
            .attr("t", "1777415965")
            .attr("sts", "1777415965123456")
            .attr("verified_level", "unknown")
            .attr("verified_name", "12345")
            .attr("peer_recipient_pn", "559980000099@s.whatsapp.net")
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();

        assert_eq!(info.server_timestamp_us, Some(1777415965123456));
        assert_eq!(info.verified_level.as_deref(), Some("unknown"));
        assert_eq!(info.verified_name_serial, Some(12345));
        assert_eq!(
            info.peer_recipient_pn.as_ref().map(|j| j.user.as_str()),
            Some("559980000099")
        );
    }

    /// Envelope without any of the optional enrichment attrs leaves all
    /// four fields as `None`. Regression guard against accidentally
    /// defaulting them.
    #[test]
    fn envelope_enrichment_is_optional() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-ENV-NONE")
            .attr("t", "1777415965")
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();

        assert!(info.server_timestamp_us.is_none());
        assert!(info.verified_level.is_none());
        assert!(info.verified_name_serial.is_none());
        assert!(info.peer_recipient_pn.is_none());
    }

    /// `<meta content_type="add_on"/>` (reactions/edits) and
    /// `<meta appdata="default"/>` are captured on `MsgMetaInfo`.
    /// Real shape observed in production for reactions.
    #[test]
    fn meta_child_attrs_are_captured() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "reaction")
            .attr("id", "MSG-REACT-1")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("meta")
                .attr("content_type", "add_on")
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(info.meta_info.content_type.as_deref(), Some("add_on"));
        assert!(info.meta_info.appdata.is_none());
    }

    /// `<reporting><reporting_tag>{bytes}</reporting_tag>
    /// <reporting_token v="2">{bytes}</reporting_token></reporting>` shape
    /// from production. Tag may be 16 or 20 bytes; token usually 16.
    #[test]
    fn reporting_token_and_tag_are_captured() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let tag_bytes: Vec<u8> = (0..16).collect();
        let token_bytes: Vec<u8> = (16..32).collect();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-REP-1")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("reporting")
                .children([
                    NodeBuilder::new("reporting_tag")
                        .bytes(tag_bytes.clone())
                        .build(),
                    NodeBuilder::new("reporting_token")
                        .attr("v", "2")
                        .bytes(token_bytes.clone())
                        .build(),
                ])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(
            info.meta_info.reporting_tag.as_deref(),
            Some(tag_bytes.as_slice())
        );
        assert_eq!(
            info.meta_info.reporting_token.as_deref(),
            Some(token_bytes.as_slice())
        );
        assert_eq!(info.meta_info.reporting_token_version, Some(2));
    }

    /// Missing `v` attr on `<reporting_token>` defaults the version to 1
    /// (matches WA Web `I()`: `c.maybeAttrInt("v") != null ? _ : 1`).
    #[test]
    fn reporting_token_missing_version_defaults_to_one() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-REP-V")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("reporting")
                .children([NodeBuilder::new("reporting_token")
                    .bytes(vec![0xAA; 16])
                    .build()])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert_eq!(info.meta_info.reporting_token_version, Some(1));
    }

    /// `<reporting>` with ONLY `<reporting_tag>` (no token) is also valid
    /// in production; token fields stay `None`.
    #[test]
    fn reporting_tag_only_leaves_token_none() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-REP-2")
            .attr("t", "1777415965")
            .children([NodeBuilder::new("reporting")
                .children([NodeBuilder::new("reporting_tag")
                    .bytes(vec![1u8; 16])
                    .build()])
                .build()])
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert!(info.meta_info.reporting_tag.is_some());
        assert!(info.meta_info.reporting_token.is_none());
        assert!(info.meta_info.reporting_token_version.is_none());
    }

    /// Message with no `<meta>` and no `<reporting>` leaves all the new
    /// `MsgMetaInfo` fields `None`.
    #[test]
    fn meta_and_reporting_absent_leaves_all_none() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let node = NodeBuilder::new("message")
            .attr("from", "99000000000001@s.whatsapp.net")
            .attr("type", "text")
            .attr("id", "MSG-PLAIN")
            .attr("t", "1777415965")
            .build();
        let info = parse_message_info(&node.as_node_ref(), &own_pn, None).unwrap();
        assert!(info.meta_info.content_type.is_none());
        assert!(info.meta_info.appdata.is_none());
        assert!(info.meta_info.reporting_tag.is_none());
        assert!(info.meta_info.reporting_token.is_none());
    }

    /// Symmetric branch: when `participant` is a LID, `sender_alt` must come
    /// from `participant_pn`. Pins the `Server::Lid`/`is_lid_family()` arm.
    #[test]
    fn status_broadcast_with_participant_pn_populates_sender_alt() {
        let own_pn = Jid::from_str("559900000000@s.whatsapp.net").unwrap();
        let own_lid = Jid::from_str("100000000000000@lid").unwrap();
        let pn_user = "559980000001";
        let lid_user = "100000012345678";
        let node = NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("type", "media")
            .attr("id", "TEST_LID_FIRST_MSG_ID")
            .attr("t", "1777415965")
            .attr("participant", format!("{lid_user}@lid").as_str())
            .attr(
                "participant_pn",
                format!("{pn_user}@s.whatsapp.net").as_str(),
            )
            .build();

        let info = parse_message_info(&node.as_node_ref(), &own_pn, Some(&own_lid))
            .expect("parse_message_info should succeed for LID-addressed status");

        assert_eq!(info.source.sender.user, lid_user);
        assert_eq!(info.source.sender.server, wacore_binary::Server::Lid);
        let alt = info
            .source
            .sender_alt
            .as_ref()
            .expect("LID-addressed status broadcast must expose participant_pn as sender_alt");
        assert_eq!(alt.user, pn_user);
        assert_eq!(alt.server, wacore_binary::Server::Pn);
    }
}
