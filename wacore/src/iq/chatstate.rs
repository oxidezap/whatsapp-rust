//! Chatstate protocol types following the ProtocolNode pattern.
//!
//! This module provides type-safe structures for parsing incoming `<chatstate>` stanzas
//! (typing indicators) following the patterns defined in `wacore/src/protocol.rs`.
//!
//! ## Wire Format
//!
//! Incoming chatstates (from other users) have a `from` attribute:
//! ```xml
//! <chatstate from="user@s.whatsapp.net">
//!   <composing/>
//! </chatstate>
//! ```
//!
//! Self-echo chatstates (our own typing echoed for multi-device sync) have a `to` attribute:
//! ```xml
//! <chatstate to="user@s.whatsapp.net">
//!   <composing/>
//! </chatstate>
//! ```

use crate::protocol::ProtocolNode;
use crate::types::presence::ChatActivity;
use anyhow::Result;
use thiserror::Error;
use wacore_binary::Jid;
use wacore_binary::Node;
use wacore_binary::NodeRef;

/// Error type for chatstate parsing failures.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ChatstateParseError {
    /// Stanza has wrong tag (not `<chatstate>`)
    #[error("expected <chatstate>, got <{0}>")]
    WrongTag(String),

    /// Self-echo chatstate (has `to` attribute instead of `from`).
    /// This is our own typing indicator echoed back for multi-device sync.
    #[error("self-echo chatstate (has 'to' but no 'from')")]
    SelfEcho,

    /// Missing required `from` attribute
    #[error("missing required attribute 'from'")]
    MissingFrom,

    /// Invalid JID in attribute
    #[error("invalid JID: {0}")]
    InvalidJid(#[from] anyhow::Error),
}

impl ChatActivity {
    /// Project semantic activity into the chatstate child sent on the wire.
    pub fn into_child_node(self) -> Node {
        use wacore_binary::builder::NodeBuilder;
        match self {
            Self::Typing => NodeBuilder::new("composing").build(),
            Self::RecordingAudio => NodeBuilder::new("composing").attr("media", "audio").build(),
            Self::Idle => NodeBuilder::new("paused").build(),
        }
    }

    /// Parse chat state from a chatstate stanza's child node.
    ///
    /// Wire format (from WhatsApp Web's `WAHandleChatStateProtocol.parseChatStatus`):
    /// - `<composing/>` → Typing
    /// - `<composing media="audio"/>` → RecordingAudio
    /// - `<paused/>` → Idle
    ///
    /// Retains the parser's permissive fallback: unknown media on composing is
    /// typing, and unknown child tags are idle. This is a semantic projection,
    /// not a lossless raw-node parser.
    pub fn from_child_node(child: &NodeRef<'_>) -> Self {
        match child.tag.as_ref() {
            "composing" => {
                // Check for media="audio" to distinguish recording from typing
                if child
                    .get_attr("media")
                    .map(|v| v.as_str())
                    .is_some_and(|s| s == "audio")
                {
                    Self::RecordingAudio
                } else {
                    Self::Typing
                }
            }
            "paused" => Self::Idle,
            _ => Self::Idle, // Default to idle for unknown states
        }
    }
}

/// Source of a chatstate event.
///
/// WhatsApp Web distinguishes between user (1:1) and group chatstates
/// via `WASmaxInChatstateFromUserMixin` and `WASmaxInChatstateFromGroupMixin`.
#[derive(Debug, Clone)]
pub enum ChatstateSource {
    /// From a 1:1 chat (user JID in `from`)
    User { from: Jid },
    /// From a group chat (group JID in `from`, sender in `participant`)
    Group { from: Jid, participant: Jid },
}

/// Parsed chatstate stanza.
///
/// Wire format:
/// ```xml
/// <!-- 1:1 chat -->
/// <chatstate from="user@s.whatsapp.net">
///   <composing/>
/// </chatstate>
///
/// <!-- Group chat -->
/// <chatstate from="group@g.us" participant="user@s.whatsapp.net">
///   <composing media="audio"/>
/// </chatstate>
/// ```
#[derive(Debug, Clone)]
pub struct ChatstateStanza {
    pub source: ChatstateSource,
    pub state: ChatActivity,
}

impl ChatstateStanza {
    /// Parse a chatstate stanza, returning a typed error.
    ///
    /// Use this method when you need to distinguish between different failure modes
    /// (e.g., to ignore self-echo chatstates without logging warnings).
    pub fn parse(node: &NodeRef<'_>) -> Result<Self, ChatstateParseError> {
        if node.tag != "chatstate" {
            return Err(ChatstateParseError::WrongTag(node.tag.to_string()));
        }

        let mut attrs = node.attrs();
        let from = match attrs.optional_jid("from") {
            Some(jid) => jid,
            None => {
                if attrs.optional_jid("to").is_some() {
                    return Err(ChatstateParseError::SelfEcho);
                }
                return Err(ChatstateParseError::MissingFrom);
            }
        };

        // Parse 'participant' attribute (optional, present in groups)
        let source = match attrs.optional_jid("participant") {
            Some(participant) => ChatstateSource::Group { from, participant },
            None => ChatstateSource::User { from },
        };

        // Parse state from first child node
        let state = node
            .children()
            .and_then(|children| children.first())
            .map(ChatActivity::from_child_node)
            .unwrap_or(ChatActivity::Idle);

        Ok(Self { source, state })
    }
}

impl ProtocolNode for ChatstateStanza {
    fn tag(&self) -> &'static str {
        "chatstate"
    }

    fn into_node(self) -> Node {
        // Chatstate stanzas are incoming-only; outgoing uses features/chatstate.rs
        unimplemented!("ChatstateStanza is incoming-only")
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self> {
        Self::parse(node).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;

    // WA Web 2.3000.1045368834: WASendChatStateProtocol and
    // WASmaxOutChatstate{Composing,Paused}Mixin. Semantic names are not wire tags.
    #[test]
    fn semantic_projection_retains_existing_unknown_payload_fallbacks() {
        for (tag, media, expected) in [
            ("composing", "video", ChatActivity::Typing),
            ("paused", "audio", ChatActivity::Idle),
            ("unknown", "audio", ChatActivity::Idle),
        ] {
            let child = NodeBuilder::new(tag)
                .attr("media", media)
                .attr("extra", "value")
                .build();
            assert_eq!(
                ChatActivity::from_child_node(&child.as_node_ref()),
                expected
            );
        }
    }

    #[test]
    fn activity_projects_to_exact_wire_children() {
        for (activity, tag, media, json) in [
            (ChatActivity::Typing, "composing", None, "typing"),
            (
                ChatActivity::RecordingAudio,
                "composing",
                Some("audio"),
                "recording_audio",
            ),
            (ChatActivity::Idle, "paused", None, "idle"),
        ] {
            let child = activity.into_child_node();
            assert_eq!(child.tag, tag);
            assert_eq!(
                child.attrs.get("media").map(|v| v.as_str()).as_deref(),
                media
            );
            assert_eq!(child.attrs.len(), usize::from(media.is_some()));
            assert!(child.content.is_none());
            assert_eq!(
                ChatActivity::from_child_node(&child.as_node_ref()),
                activity
            );
            assert_eq!(serde_json::to_value(activity).unwrap(), json);
            assert_eq!(
                serde_json::from_value::<ChatActivity>(serde_json::json!(json)).unwrap(),
                activity
            );
        }
    }

    #[test]
    fn test_parse_user_typing() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("composing").build()])
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert!(matches!(stanza.source, ChatstateSource::User { .. }));
        assert_eq!(stanza.state, ChatActivity::Typing);

        if let ChatstateSource::User { from } = stanza.source {
            assert_eq!(from.user, "1234567890");
        }
    }

    #[test]
    fn test_parse_user_recording() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("composing").attr("media", "audio").build()])
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert_eq!(stanza.state, ChatActivity::RecordingAudio);
    }

    #[test]
    fn test_parse_user_paused() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("paused").build()])
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert_eq!(stanza.state, ChatActivity::Idle);
    }

    #[test]
    fn test_parse_group_typing() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "123456789-1234567890@g.us")
            .attr("participant", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("composing").build()])
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert!(matches!(stanza.source, ChatstateSource::Group { .. }));
        assert_eq!(stanza.state, ChatActivity::Typing);

        if let ChatstateSource::Group { from, participant } = stanza.source {
            assert_eq!(from.user, "123456789-1234567890");
            assert_eq!(participant.user, "1234567890");
        }
    }

    #[test]
    fn test_parse_group_recording() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "123456789-1234567890@g.us")
            .attr("participant", "5678@s.whatsapp.net")
            .children([NodeBuilder::new("composing").attr("media", "audio").build()])
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert!(matches!(stanza.source, ChatstateSource::Group { .. }));
        assert_eq!(stanza.state, ChatActivity::RecordingAudio);
    }

    #[test]
    fn test_parse_missing_from_error() {
        // Chatstate with neither 'from' nor 'to' should error with MissingFrom
        let node = NodeBuilder::new("chatstate")
            .children([NodeBuilder::new("composing").build()])
            .build();

        let result = ChatstateStanza::parse(&node.as_node_ref());
        assert!(matches!(result, Err(ChatstateParseError::MissingFrom)));
    }

    #[test]
    fn test_parse_self_echo_chatstate() {
        // Self-echo chatstates have 'to' instead of 'from' (our own typing echoed for multi-device sync)
        let node = NodeBuilder::new("chatstate")
            .attr("to", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("composing").build()])
            .build();

        let result = ChatstateStanza::parse(&node.as_node_ref());
        assert!(matches!(result, Err(ChatstateParseError::SelfEcho)));
    }

    #[test]
    fn test_parse_wrong_tag_error() {
        let node = NodeBuilder::new("message")
            .attr("from", "1234567890@s.whatsapp.net")
            .build();

        let result = ChatstateStanza::parse(&node.as_node_ref());
        assert!(matches!(result, Err(ChatstateParseError::WrongTag(_))));
    }

    #[test]
    fn test_parse_no_children_defaults_to_idle() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "1234567890@s.whatsapp.net")
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert_eq!(stanza.state, ChatActivity::Idle);
    }

    #[test]
    fn test_parse_unknown_child_defaults_to_idle() {
        let node = NodeBuilder::new("chatstate")
            .attr("from", "1234567890@s.whatsapp.net")
            .children([NodeBuilder::new("unknown_state").build()])
            .build();

        let stanza = ChatstateStanza::try_from_node(&node).unwrap();
        assert_eq!(stanza.state, ChatActivity::Idle);
    }

    #[test]
    fn test_parse_lid_jid_chatstate() {
        // LID JIDs (Linked IDs) should parse correctly when stored as strings
        let node = NodeBuilder::new("chatstate")
            .attr("from", "236395184570386@lid")
            .children([NodeBuilder::new("composing").build()])
            .build();

        let stanza = ChatstateStanza::parse(&node.as_node_ref()).unwrap();
        assert!(matches!(stanza.source, ChatstateSource::User { .. }));
        assert_eq!(stanza.state, ChatActivity::Typing);

        if let ChatstateSource::User { from } = stanza.source {
            assert_eq!(from.user, "236395184570386");
            assert_eq!(from.server, "lid");
        }
    }

    #[test]
    fn test_parse_jid_attribute_as_jid_type() {
        // In the binary protocol, JID attributes are stored as actual JID types,
        // not strings. This test simulates that by passing a Jid directly to attr().
        use wacore_binary::Jid;

        let jid: Jid = "236395184570386@lid".parse().unwrap();
        let node = NodeBuilder::new("chatstate")
            .attr("from", jid)
            .children([NodeBuilder::new("composing").build()])
            .build();

        let stanza = ChatstateStanza::parse(&node.as_node_ref()).unwrap();
        assert!(matches!(stanza.source, ChatstateSource::User { .. }));
        assert_eq!(stanza.state, ChatActivity::Typing);

        if let ChatstateSource::User { from } = stanza.source {
            assert_eq!(from.user, "236395184570386");
            assert_eq!(from.server, "lid");
        }
    }

    #[test]
    fn test_parse_group_chatstate_with_jid_types() {
        // Test group chatstate with JID-typed attributes (as binary protocol stores them)
        use wacore_binary::Jid;

        let group_jid: Jid = "123456789-1234567890@g.us".parse().unwrap();
        let participant_jid: Jid = "236395184570386@lid".parse().unwrap();

        let node = NodeBuilder::new("chatstate")
            .attr("from", group_jid)
            .attr("participant", participant_jid)
            .children([NodeBuilder::new("composing").build()])
            .build();

        let stanza = ChatstateStanza::parse(&node.as_node_ref()).unwrap();
        assert!(matches!(stanza.source, ChatstateSource::Group { .. }));
        assert_eq!(stanza.state, ChatActivity::Typing);

        if let ChatstateSource::Group { from, participant } = stanza.source {
            assert_eq!(from.server, "g.us");
            assert_eq!(participant.user, "236395184570386");
            assert_eq!(participant.server, "lid");
        }
    }
}
