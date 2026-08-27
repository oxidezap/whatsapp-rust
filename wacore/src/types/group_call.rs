//! Typed state carried by group-call signaling.
//!
//! Relay credentials and encrypted key payloads are intentionally excluded from
//! serde output. They are runtime material, not part of the public event surface.

use core::mem::size_of;

use serde::Serialize;
use wacore_binary::Jid;
use zeroize::Zeroize;

/// Maximum call membership from WA Web's `group_call_max_participants` AB prop.
pub const GROUP_CALL_MAX_PARTICIPANTS: usize = 32;
/// The local endpoint consumes one membership slot.
pub const GROUP_CALL_MAX_REMOTE_PARTICIPANTS: usize = GROUP_CALL_MAX_PARTICIPANTS - 1;

/// Audio/video mode of a reusable call link.
///
/// Hand-written rather than generated. The enum catalog holds exactly one
/// `audio`/`video` pair, `MediaType` in the status composer's module, and two
/// two-valued media enums agreeing on their values is not evidence that one
/// owns the other's wire format. Binding it would let a kind added for status
/// composition arrive here, in the type that builds and parses `<call_link>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum CallLinkMedia {
    #[wire = "audio"]
    Audio,
    #[wire = "video"]
    Video,
}

/// One device in an authoritative group-call roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupCallDevice {
    pub jid: Jid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capability_version: Option<u32>,
    #[serde(skip)]
    #[builder(default)]
    pub(crate) capability: Vec<u8>,
}

impl GroupCallDevice {
    pub fn new(jid: Jid) -> Self {
        Self {
            jid,
            platform: None,
            pid: None,
            capability_version: None,
            capability: Vec::new(),
        }
    }

    pub fn with_capability(mut self, version: u32, capability: impl Into<Vec<u8>>) -> Self {
        self.capability_version = Some(version);
        self.capability = capability.into();
        self
    }
}

impl crate::stats::HeapSize for GroupCallDevice {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        self.jid.heap_bytes()
            + self.platform.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.capability.heap_bytes()
    }
}

/// One user in an authoritative group-call roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupCallParticipant {
    pub jid: Jid,
    /// Phone-number aliases are retained for authenticated routing but never exposed in events.
    #[serde(skip)]
    pub pn: Option<Jid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub participant_type: Option<String>,
    pub devices: Vec<GroupCallDevice>,
}

impl GroupCallParticipant {
    pub fn new(jid: Jid, devices: Vec<GroupCallDevice>) -> Self {
        Self {
            jid,
            pn: None,
            state: None,
            participant_type: None,
            devices,
        }
    }

    /// Whether this participant belongs to the authoritative active roster.
    pub fn is_connected(&self) -> bool {
        self.state.as_deref() == Some("connected")
    }
}

impl crate::stats::HeapSize for GroupCallParticipant {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        self.jid.heap_bytes()
            + self.pn.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.state.as_ref().map_or(0, HeapSize::heap_bytes)
            + self
                .participant_type
                .as_ref()
                .map_or(0, HeapSize::heap_bytes)
            + self.devices.capacity() * size_of::<GroupCallDevice>()
            + self.devices.iter().map(HeapSize::heap_bytes).sum::<usize>()
    }
}

/// One address advertised by the shared group relay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupCallRelayEndpoint {
    pub relay_id: u32,
    pub token_id: u32,
    pub auth_token_id: u32,
    pub relay_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub domain_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rtt_ms: Option<u32>,
    pub is_fna: bool,
    #[serde(skip)]
    #[builder(default)]
    pub(crate) address: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ipv4: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

impl crate::stats::HeapSize for GroupCallRelayEndpoint {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        self.relay_name.heap_bytes()
            + self.domain_name.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.address.heap_bytes()
            + self.ipv4.as_ref().map_or(0, HeapSize::heap_bytes)
    }
}

/// Shared relay allocation embedded in a group snapshot.
#[derive(Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupCallRelay {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub self_pid: Option<u32>,
    pub uuid: String,
    pub participant_uuid: String,
    pub attribute_padding: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warp_mi_tag_len: Option<u32>,
    #[serde(skip)]
    #[builder(default)]
    pub(crate) key: Vec<u8>,
    #[serde(skip)]
    #[builder(default)]
    pub(crate) hbh_key: Vec<u8>,
    #[serde(skip)]
    #[builder(default)]
    pub(crate) tokens: Vec<Vec<u8>>,
    #[serde(skip)]
    #[builder(default)]
    pub(crate) auth_tokens: Vec<Vec<u8>>,
    pub endpoints: Vec<GroupCallRelayEndpoint>,
}

impl core::fmt::Debug for GroupCallRelay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GroupCallRelay")
            .field("transaction_id", &self.transaction_id)
            .field("self_pid", &self.self_pid)
            .field("uuid", &self.uuid)
            .field("participant_uuid", &self.participant_uuid)
            .field("attribute_padding", &self.attribute_padding)
            .field("warp_mi_tag_len", &self.warp_mi_tag_len)
            .field("key", &"[redacted]")
            .field("hbh_key", &"[redacted]")
            .field("tokens", &"[redacted]")
            .field("auth_tokens", &"[redacted]")
            .field("endpoints", &self.endpoints)
            .finish()
    }
}

impl GroupCallRelay {
    pub(crate) fn scrub(&mut self) {
        self.key.zeroize();
        self.hbh_key.zeroize();
        self.tokens.zeroize();
        self.auth_tokens.zeroize();
    }
}

impl Drop for GroupCallRelay {
    fn drop(&mut self) {
        self.scrub();
    }
}

impl crate::stats::HeapSize for GroupCallRelay {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        fn nested_bytes(values: &[Vec<u8>], capacity: usize) -> usize {
            capacity * size_of::<Vec<u8>>() + values.iter().map(Vec::capacity).sum::<usize>()
        }

        self.uuid.heap_bytes()
            + self.participant_uuid.heap_bytes()
            + self.key.heap_bytes()
            + self.hbh_key.heap_bytes()
            + nested_bytes(&self.tokens, self.tokens.capacity())
            + nested_bytes(&self.auth_tokens, self.auth_tokens.capacity())
            + self.endpoints.capacity() * size_of::<GroupCallRelayEndpoint>()
            + self
                .endpoints
                .iter()
                .map(HeapSize::heap_bytes)
                .sum::<usize>()
    }
}

/// One transaction-ordered authoritative group-call snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupCallUpdate {
    pub call_id: String,
    pub call_creator: Jid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_jid: Option<Jid>,
    pub transaction_id: u32,
    pub media: String,
    pub connected_limit: u32,
    pub joinable: bool,
    pub av_upgradable: bool,
    pub rekey_requested: bool,
    pub participants: Vec<GroupCallParticipant>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<GroupCallRelay>,
}

impl crate::stats::HeapSize for GroupCallUpdate {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        self.call_id.heap_bytes()
            + self.call_creator.heap_bytes()
            + self.group_jid.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.media.heap_bytes()
            + self.participants.capacity() * size_of::<GroupCallParticipant>()
            + self
                .participants
                .iter()
                .map(HeapSize::heap_bytes)
                .sum::<usize>()
            + self.relay.as_ref().map_or(0, HeapSize::heap_bytes)
    }
}

/// One encrypted keygen-v2 epoch delivered to a participant device.
#[derive(Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct GroupCallEncRekey {
    pub call_id: String,
    pub call_creator: Jid,
    pub transaction_id: u32,
    pub key_generation: u32,
    pub encryption_type: String,
    pub encryption_version: u32,
    #[serde(skip)]
    pub ciphertext: Vec<u8>,
}

impl core::fmt::Debug for GroupCallEncRekey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GroupCallEncRekey")
            .field("call_id", &self.call_id)
            .field("call_creator", &self.call_creator)
            .field("transaction_id", &self.transaction_id)
            .field("key_generation", &self.key_generation)
            .field("encryption_type", &self.encryption_type)
            .field("encryption_version", &self.encryption_version)
            .field("ciphertext", &"[redacted]")
            .finish()
    }
}

/// Result of creating a reusable call link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct CallLink {
    pub token: String,
    pub media: CallLinkMedia,
}

impl CallLink {
    pub fn url(&self) -> String {
        format!(
            "https://call.whatsapp.com/{}/{}",
            self.media.as_str(),
            self.token
        )
    }
}

/// Metadata returned without joining a reusable call link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct CallLinkPreview {
    pub token: String,
    pub media: CallLinkMedia,
    pub creator: Jid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_pn: Option<Jid>,
    pub waiting_room_enabled: bool,
    pub is_admin: bool,
}

/// One user in a waiting-room snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct WaitingRoomUser {
    pub jid: Jid,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pn: Option<Jid>,
    pub state: String,
}

impl crate::stats::HeapSize for WaitingRoomUser {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        self.jid.heap_bytes()
            + self.pn.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.state.heap_bytes()
    }
}

/// Authoritative waiting-room state for a call-link call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct WaitingRoom {
    pub call_id: String,
    pub call_creator: Jid,
    pub link_token: String,
    pub media: CallLinkMedia,
    pub enabled: bool,
    pub is_admin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<u32>,
    pub users: Vec<WaitingRoomUser>,
}

impl crate::stats::HeapSize for WaitingRoom {
    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        self.call_id.heap_bytes()
            + self.call_creator.heap_bytes()
            + self.link_token.heap_bytes()
            + self.users.capacity() * size_of::<WaitingRoomUser>()
            + self.users.iter().map(HeapSize::heap_bytes).sum::<usize>()
    }
}

/// Admission state returned after joining a reusable call link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct CallLinkJoin {
    pub token: String,
    pub media: CallLinkMedia,
    pub call_id: String,
    pub call_creator: Jid,
    pub waiting_room_enabled: bool,
    pub in_waiting_room: bool,
    pub is_admin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_room: Option<WaitingRoom>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupCallUpdate>,
}

impl CallLinkJoin {
    /// Seed the authoritative state container for a pending admission.
    pub fn pending_waiting_room(&self) -> Option<WaitingRoom> {
        if self.in_waiting_room {
            self.waiting_room.clone()
        } else {
            None
        }
    }
}

/// Wire state of a screen-share transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
#[wire(kind = "int")]
pub enum ScreenShareState {
    #[wire = 1]
    Started,
    #[wire = 2]
    Stopped,
}

/// Parsed screen-share state for one participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, bon::Builder)]
#[non_exhaustive]
pub struct ScreenShare {
    pub state: ScreenShareState,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_share_id: Option<u32>,
}

impl ScreenShare {
    pub fn new(state: ScreenShareState, screen_share_id: Option<u32>) -> Self {
        Self {
            state,
            version: 2,
            screen_share_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GroupCallRelay;

    #[test]
    fn group_call_relay_scrub_clears_every_secret_field() {
        let mut relay = GroupCallRelay::builder()
            .uuid("relay".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .key(vec![1, 2, 3])
            .hbh_key(vec![4, 5, 6])
            .tokens(vec![vec![7, 8, 9]])
            .auth_tokens(vec![vec![10, 11, 12]])
            .endpoints(Vec::new())
            .build();
        assert!(!relay.key.is_empty());
        assert!(!relay.hbh_key.is_empty());
        assert!(!relay.tokens.is_empty());
        assert!(!relay.auth_tokens.is_empty());

        relay.scrub();

        assert!(relay.key.is_empty());
        assert!(relay.hbh_key.is_empty());
        assert!(relay.tokens.is_empty());
        assert!(relay.auth_tokens.is_empty());
    }
}
