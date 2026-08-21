//! Message stanza types with ProtocolNode derive macro.
//!
//! Provides type-safe message stanzas with JID-aware attributes.

use crate::ProtocolNode;
use wacore_binary::{CompactString, Jid};

/// Typed 1-to-1 or generic message stanza with JID-safe attributes.
///
/// The text attributes are `CompactString`: a message id, a unix timestamp, a
/// type and an addressing mode all fit in the 24 inline bytes, so parsing a
/// stanza takes no heap allocation for them.
///
/// Wire format:
/// ```xml
/// <message from="jid" to="jid" id="..." t="..." type="..."/>
/// ```
#[derive(Debug, Clone, ProtocolNode)]
#[protocol(tag = "message")]
pub struct MessageStanza {
    /// Sender JID (required)
    #[attr(name = "from", jid)]
    pub from: Jid,

    /// Recipient JID (optional; not always present)
    #[attr(name = "to", jid, optional)]
    pub to: Option<Jid>,

    /// Message ID (required)
    #[attr(name = "id")]
    pub id: CompactString,

    /// Timestamp (required, seconds since epoch)
    #[attr(name = "t")]
    pub timestamp: CompactString,

    /// Message type (default: "text")
    #[attr(name = "type", default = "text")]
    pub msg_type: CompactString,

    /// Optional sender LID (JID)
    #[attr(name = "sender_lid", jid, optional)]
    pub sender_lid: Option<Jid>,

    /// Optional participant JID (group sender)
    #[attr(name = "participant", jid, optional)]
    pub participant: Option<Jid>,

    /// Optional participant phone JID
    #[attr(name = "participant_pn", jid, optional)]
    pub participant_pn: Option<Jid>,

    /// Optional addressing mode (e.g., "lid")
    #[attr(name = "addressing_mode", optional)]
    pub addressing_mode: Option<CompactString>,

    /// Optional push name of sender (server-injected on forwarded messages)
    #[attr(name = "notify", optional)]
    pub notify: Option<CompactString>,
}
