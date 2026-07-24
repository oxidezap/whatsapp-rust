//! Canonical typed model and wire codec for USync queries.
//!
//! This module deliberately models only protocols observed in WhatsApp Web.
//! Arbitrary protocol nodes remain available through the low-level IQ API;
//! keeping that escape hatch separate prevents custom wire data from weakening
//! the invariants of the typed path.

use super::{UsyncContext, UsyncMode, UsyncSubprotocolError};
use crate::WireEnum;
use crate::iq::spec::IqSpec;
use crate::iq::tctoken::build_tc_token_node;
use crate::request::InfoQuery;
use crate::stanza::business::VerifiedName;
use anyhow::{Context, anyhow};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{CompactString, Jid, Node, NodeContent, NodeContentRef, NodeRef, Server};

const USYNC_NAMESPACE: &str = "usync";
const TAG_USYNC: &str = "usync";
const TAG_QUERY: &str = "query";
const TAG_RESULT: &str = "result";
const TAG_LIST: &str = "list";
const TAG_USER: &str = "user";
const TAG_ERROR: &str = "error";
const TAG_VERIFIED_NAME: &str = "verified_name";
const TAG_DEVICE_LIST: &str = "device-list";
const TAG_DEVICE: &str = "device";
const TAG_KEY_INDEX_LIST: &str = "key-index-list";
const TAG_PROFILE: &str = "profile";
const TAG_NAME: &str = "name";
const TAG_ATTRIBUTES: &str = "attributes";
const TAG_DESCRIPTION: &str = "description";
const TAG_CATEGORY: &str = "category";
const TAG_DEFAULT: &str = "default";
const TAG_PROMPTS: &str = "prompts";
const TAG_PROMPT: &str = "prompt";
const TAG_EMOJI: &str = "emoji";
const TAG_TEXT: &str = "text";
const TAG_COMMANDS: &str = "commands";
const TAG_COMMAND: &str = "command";
const TAG_IS_META_CREATED: &str = "is_meta_created";
const TAG_CREATOR: &str = "creator";
const TAG_PROFILE_URL: &str = "profile_url";
const TAG_POSING_AS_PROFESSIONAL: &str = "posing_as_professional";

const ATTR_SID: &str = "sid";
const ATTR_MODE: &str = "mode";
const ATTR_CONTEXT: &str = "context";
const ATTR_INDEX: &str = "index";
const ATTR_LAST: &str = "last";
const ATTR_JID: &str = "jid";
const ATTR_PN_JID: &str = "pn_jid";
const ATTR_VERSION: &str = "version";
const ATTR_BOT_VERSION: &str = "v";
const ATTR_LID: &str = "lid";
const ATTR_ADDRESSING_MODE: &str = "addressing_mode";
const ATTR_DEVICE_HASH: &str = "device_hash";
const ATTR_TIMESTAMP: &str = "ts";
const ATTR_TIME: &str = "t";
const ATTR_EXPECTED_TIMESTAMP: &str = "expected_ts";
const ATTR_PERSONA_ID: &str = "persona_id";
const ATTR_USERNAME: &str = "username";
const ATTR_PIN: &str = "pin";
const ATTR_TYPE: &str = "type";
const ATTR_VAL: &str = "val";
const ATTR_VALUE: &str = "value";
const ATTR_CODE: &str = "code";
const ATTR_TEXT: &str = "text";
const ATTR_BACKOFF: &str = "backoff";
const ATTR_REFRESH: &str = "refresh";
const ATTR_ID: &str = "id";
const ATTR_KEY_INDEX: &str = "key-index";
const ATTR_IS_HOSTED: &str = "is_hosted";
const ATTR_HASH: &str = "hash";
const ATTR_DURATION: &str = "duration";
const ATTR_EPHEMERALITY_DISABLED: &str = "ephemerality_disabled";
const ATTR_CONTENT: &str = "content";
const ATTR_EPHEMERAL_DURATION: &str = "ephemeral_duration_sec";
const ATTR_LAST_UPDATE_TIME: &str = "last_update_time";

const FIRST_PAGE_INDEX: &str = "0";
const LAST_PAGE: &str = "true";
const DEVICES_VERSION: &str = "2";
const BOT_PROFILE_VERSION: &str = "1";
const E164_PREFIX: char = '+';
const E164_MAX_DIGITS: usize = 15;

/// Addressing mode used by the contact subprotocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, WireEnum)]
#[non_exhaustive]
pub enum UsyncAddressingMode {
    #[wire_default]
    #[wire = "pn"]
    Pn,
    #[wire = "lid"]
    Lid,
}

/// Known USync protocol tags from the captured WhatsApp Web client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, WireEnum)]
#[non_exhaustive]
pub enum UsyncProtocolKind {
    #[wire = "feature"]
    Feature,
    #[wire = "devices"]
    Devices,
    #[wire = "contact"]
    Contact,
    #[wire = "picture"]
    Picture,
    #[wire = "status"]
    Status,
    #[wire = "business"]
    Business,
    #[wire = "disappearing_mode"]
    DisappearingMode,
    #[wire = "lid"]
    Lid,
    #[wire = "bot"]
    Bot,
    #[wire = "username"]
    Username,
    #[wire = "text_status"]
    TextStatus,
}

impl UsyncProtocolKind {
    const fn bit(self) -> u16 {
        match self {
            Self::Feature => 1 << 0,
            Self::Devices => 1 << 1,
            Self::Contact => 1 << 2,
            Self::Picture => 1 << 3,
            Self::Status => 1 << 4,
            Self::Business => 1 << 5,
            Self::DisappearingMode => 1 << 6,
            Self::Lid => 1 << 7,
            Self::Bot => 1 << 8,
            Self::Username => 1 << 9,
            Self::TextStatus => 1 << 10,
        }
    }
}

/// Feature names accepted by the USync feature protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, WireEnum)]
#[non_exhaustive]
pub enum UsyncFeature {
    #[wire = "document"]
    Document,
    #[wire = "encrypt"]
    Encrypt,
    #[wire = "encrypt_blist"]
    EncryptBlocklist,
    #[wire = "encrypt_contact"]
    EncryptContact,
    #[wire = "encrypt_group_gen2"]
    EncryptGroupGen2,
    #[wire = "encrypt_image"]
    EncryptImage,
    #[wire = "encrypt_location"]
    EncryptLocation,
    #[wire = "encrypt_url"]
    EncryptUrl,
    #[wire = "encrypt_v2"]
    EncryptV2,
    #[wire = "voip"]
    Voip,
    #[wire = "multi_agent"]
    MultiAgent,
}

/// Per-user cache hints carried by the devices v2 subprotocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncDeviceSyncHint {
    /// Cached device-list hash, if known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_hash: Option<CompactString>,
    /// Timestamp associated with the cached hash.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
    /// Expected timestamp used to detect stale key-index state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_timestamp: Option<i64>,
}

impl UsyncDeviceSyncHint {
    /// Construct an empty hint.
    pub const fn new() -> Self {
        Self {
            device_hash: None,
            timestamp: None,
            expected_timestamp: None,
        }
    }

    /// Attach a cached device-list hash.
    pub fn with_device_hash(mut self, device_hash: impl Into<CompactString>) -> Self {
        self.device_hash = Some(device_hash.into());
        self
    }

    /// Attach the cached device-list timestamp.
    pub const fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }

    /// Attach the expected key-index timestamp.
    pub const fn with_expected_timestamp(mut self, expected_timestamp: i64) -> Self {
        self.expected_timestamp = Some(expected_timestamp);
        self
    }

    /// Whether this hint contributes no wire attributes.
    pub fn is_empty(&self) -> bool {
        self.device_hash.is_none() && self.timestamp.is_none() && self.expected_timestamp.is_none()
    }
}

impl Default for UsyncDeviceSyncHint {
    fn default() -> Self {
        Self::new()
    }
}

/// A USync user input.
///
/// Constructors establish an identity and protocol-specific additions use
/// explicit builder methods. Deserialization applies the same phone and JID
/// normalization as those constructors; all invariants are validated when the
/// user becomes part of [`UsyncQuery::new`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UsyncUser {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_usync_jid"
    )]
    id: Option<Jid>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_usync_jid"
    )]
    pn_jid: Option<Jid>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_usync_phone"
    )]
    phone: Option<CompactString>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_optional_usync_jid"
    )]
    known_lid: Option<Jid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_sync: Option<UsyncDeviceSyncHint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    persona_id: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username_pin: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contact_type: Option<CompactString>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::serde_helpers::serialize_optional_bytes",
        deserialize_with = "crate::serde_helpers::deserialize_optional_bytes"
    )]
    tc_token: Option<Vec<u8>>,
}

impl UsyncUser {
    pub fn from_jid(jid: Jid) -> Self {
        Self::empty().with_id(jid)
    }

    pub fn from_phone(phone: impl Into<CompactString>) -> Self {
        Self::empty().with_phone(phone)
    }

    pub fn from_username(username: impl Into<CompactString>) -> Self {
        Self::empty().with_username(username)
    }

    pub fn from_pn_jid(pn_jid: Jid) -> Self {
        Self::empty().with_pn_jid(pn_jid)
    }

    fn empty() -> Self {
        Self {
            id: None,
            pn_jid: None,
            phone: None,
            known_lid: None,
            device_sync: None,
            persona_id: None,
            username: None,
            username_pin: None,
            contact_type: None,
            tc_token: None,
        }
    }

    pub fn with_id(mut self, jid: Jid) -> Self {
        self.id = Some(normalize_usync_user_jid(jid));
        self
    }

    pub fn with_pn_jid(mut self, jid: Jid) -> Self {
        self.pn_jid = Some(normalize_usync_user_jid(jid));
        self
    }

    pub fn with_phone(mut self, phone: impl Into<CompactString>) -> Self {
        self.phone = Some(normalize_usync_phone(phone.into()));
        self
    }

    pub fn with_known_lid(mut self, lid: Jid) -> Self {
        self.known_lid = Some(normalize_usync_user_jid(lid));
        self
    }

    pub fn with_device_sync(mut self, hint: UsyncDeviceSyncHint) -> Self {
        self.device_sync = Some(hint);
        self
    }

    pub fn with_persona_id(mut self, persona_id: impl Into<CompactString>) -> Self {
        self.persona_id = Some(persona_id.into());
        self
    }

    pub fn with_username(mut self, username: impl Into<CompactString>) -> Self {
        self.username = Some(username.into());
        self
    }

    pub fn with_username_pin(mut self, pin: impl Into<CompactString>) -> Self {
        self.username_pin = Some(pin.into());
        self
    }

    pub fn with_contact_type(mut self, contact_type: impl Into<CompactString>) -> Self {
        self.contact_type = Some(contact_type.into());
        self
    }

    pub fn with_tc_token(mut self, token: impl Into<Vec<u8>>) -> Self {
        self.tc_token = Some(token.into());
        self
    }

    fn has_identity(&self) -> bool {
        self.id.is_some()
            || self.pn_jid.is_some()
            || self.phone.is_some()
            || self.username.is_some()
    }

    fn validate(
        &self,
        index: usize,
        protocols: &[UsyncProtocol],
    ) -> Result<(), UsyncValidationError> {
        if !self.has_identity() {
            return Err(UsyncValidationError::MissingUserIdentity { index });
        }
        if let Some(jid) = &self.id {
            validate_user_jid(jid, index)?;
        }
        if let Some(pn_jid) = &self.pn_jid {
            validate_user_jid(pn_jid, index)?;
            if !matches!(pn_jid.server, Server::Pn | Server::Hosted) {
                return Err(UsyncValidationError::InvalidPnJid { index });
            }
        }
        if let Some(phone) = &self.phone {
            if phone.is_empty() {
                return Err(UsyncValidationError::EmptyPhone { index });
            }
            if !is_canonical_usync_phone(phone) {
                return Err(UsyncValidationError::InvalidPhone { index });
            }
        }
        if self.username.as_ref().is_some_and(CompactString::is_empty) {
            return Err(UsyncValidationError::EmptyUsername { index });
        }
        if self.username_pin.is_some() && self.username.is_none() {
            return Err(UsyncValidationError::UsernamePinWithoutUsername { index });
        }
        if let Some(lid) = &self.known_lid
            && (lid.user.is_empty() || !matches!(lid.server, Server::Lid | Server::HostedLid))
        {
            return Err(UsyncValidationError::InvalidKnownLid { index });
        }
        if self
            .device_sync
            .as_ref()
            .and_then(|hint| hint.device_hash.as_ref())
            .is_some_and(CompactString::is_empty)
        {
            return Err(UsyncValidationError::EmptyDeviceHash { index });
        }

        let has_protocol = |kind| protocols.iter().any(|protocol| protocol.kind() == kind);
        let contact_inputs = usize::from(self.phone.is_some())
            + usize::from(self.username.is_some())
            + usize::from(self.contact_type.is_some());
        if contact_inputs > 1 {
            return Err(UsyncValidationError::ConflictingContactInputs { index });
        }
        if contact_inputs != 0 && !has_protocol(UsyncProtocolKind::Contact) {
            return Err(UsyncValidationError::ContactInputWithoutProtocol { index });
        }
        if self
            .device_sync
            .as_ref()
            .is_some_and(|hint| !hint.is_empty())
            && !has_protocol(UsyncProtocolKind::Devices)
        {
            return Err(UsyncValidationError::DeviceSyncWithoutProtocol { index });
        }
        if self.tc_token.is_some() && !has_protocol(UsyncProtocolKind::Status) {
            return Err(UsyncValidationError::TcTokenWithoutProtocol { index });
        }
        if self.persona_id.is_some() && !has_protocol(UsyncProtocolKind::Bot) {
            return Err(UsyncValidationError::PersonaIdWithoutProtocol { index });
        }
        let known_lid_is_used = has_protocol(UsyncProtocolKind::Lid)
            || (has_protocol(UsyncProtocolKind::Contact) && self.username.is_some());
        if self.known_lid.is_some() && !known_lid_is_used {
            return Err(UsyncValidationError::KnownLidWithoutProtocol { index });
        }

        Ok(())
    }
}

fn normalize_usync_phone(mut phone: CompactString) -> CompactString {
    if !phone.is_empty() && phone.bytes().all(|byte| byte.is_ascii_digit()) {
        phone.insert(0, E164_PREFIX);
    }
    phone
}

fn is_canonical_usync_phone(phone: &str) -> bool {
    let Some(digits) = phone.strip_prefix(E164_PREFIX) else {
        return false;
    };
    let Some((&first, rest)) = digits.as_bytes().split_first() else {
        return false;
    };
    digits.len() <= E164_MAX_DIGITS
        && first.is_ascii_digit()
        && first != b'0'
        && rest.iter().all(u8::is_ascii_digit)
}

fn deserialize_optional_usync_phone<'de, D>(
    deserializer: D,
) -> Result<Option<CompactString>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<CompactString>::deserialize(deserializer).map(|phone| phone.map(normalize_usync_phone))
}

fn deserialize_optional_usync_jid<'de, D>(deserializer: D) -> Result<Option<Jid>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Jid>::deserialize(deserializer).map(|jid| jid.map(normalize_usync_user_jid))
}

/// Remove device addressing while retaining identity-bearing fields.
///
/// PN/LID domain bytes can surface through `agent` during binary decoding but
/// are not part of those user identities. Other namespaces render the agent,
/// so validation can reject unsupported agent-qualified query inputs instead
/// of silently querying a different user.
fn normalize_usync_user_jid(mut jid: Jid) -> Jid {
    jid.device = 0;
    if !jid.server.renders_agent() {
        jid.agent = 0;
    }
    jid
}

fn validate_user_jid(jid: &Jid, index: usize) -> Result<(), UsyncValidationError> {
    let supported = !jid.user.is_empty()
        && matches!(
            jid.server,
            Server::Pn
                | Server::Lid
                | Server::Hosted
                | Server::HostedLid
                | Server::Messenger
                | Server::Interop
                | Server::Bot
        );
    if supported && jid.agent == 0 {
        Ok(())
    } else {
        Err(UsyncValidationError::InvalidUserJid {
            index,
            jid: jid.to_string(),
        })
    }
}

/// A known, typed USync subprotocol request.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[wire(tag = "type", content = "data")]
#[non_exhaustive]
pub enum UsyncProtocol {
    #[wire = "contact"]
    Contact {
        addressing_mode: UsyncAddressingMode,
    },
    #[wire = "devices"]
    DevicesV2,
    #[wire = "status"]
    Status,
    #[wire = "text_status"]
    TextStatus,
    #[wire = "disappearing_mode"]
    DisappearingMode,
    #[wire = "business"]
    BusinessVerifiedName,
    #[wire = "picture"]
    Picture,
    #[wire = "lid"]
    Lid,
    #[wire = "username"]
    Username,
    #[wire = "bot"]
    BotProfileV1,
    #[wire = "feature"]
    Features(Vec<UsyncFeature>),
}

impl UsyncProtocol {
    pub const fn kind(&self) -> UsyncProtocolKind {
        match self {
            Self::Contact { .. } => UsyncProtocolKind::Contact,
            Self::DevicesV2 => UsyncProtocolKind::Devices,
            Self::Status => UsyncProtocolKind::Status,
            Self::TextStatus => UsyncProtocolKind::TextStatus,
            Self::DisappearingMode => UsyncProtocolKind::DisappearingMode,
            Self::BusinessVerifiedName => UsyncProtocolKind::Business,
            Self::Picture => UsyncProtocolKind::Picture,
            Self::Lid => UsyncProtocolKind::Lid,
            Self::Username => UsyncProtocolKind::Username,
            Self::BotProfileV1 => UsyncProtocolKind::Bot,
            Self::Features(_) => UsyncProtocolKind::Feature,
        }
    }

    fn validate(&self) -> Result<(), UsyncValidationError> {
        if matches!(self, Self::Features(features) if features.is_empty()) {
            return Err(UsyncValidationError::EmptyFeatureSet);
        }
        Ok(())
    }

    fn build_query_node(&self) -> Node {
        let tag = self.wire_tag();
        match self {
            Self::Contact { addressing_mode } => {
                let builder = NodeBuilder::new(tag);
                if *addressing_mode == UsyncAddressingMode::Lid {
                    builder
                        .attr(ATTR_ADDRESSING_MODE, addressing_mode.as_str())
                        .build()
                } else {
                    builder.build()
                }
            }
            Self::DevicesV2 => NodeBuilder::new(tag)
                .attr(ATTR_VERSION, DEVICES_VERSION)
                .build(),
            Self::BusinessVerifiedName => NodeBuilder::new(tag)
                .children([NodeBuilder::new(TAG_VERIFIED_NAME).build()])
                .build(),
            Self::BotProfileV1 => NodeBuilder::new(tag)
                .children([NodeBuilder::new(TAG_PROFILE)
                    .attr(ATTR_BOT_VERSION, BOT_PROFILE_VERSION)
                    .build()])
                .build(),
            Self::Features(features) => NodeBuilder::new(tag)
                .children(
                    features
                        .iter()
                        .map(|feature| NodeBuilder::new(feature.as_str()).build()),
                )
                .build(),
            _ => NodeBuilder::new(tag).build(),
        }
    }

    fn build_user_node(&self, user: &UsyncUser) -> Option<Node> {
        match self {
            Self::Contact { .. } => {
                if let Some(phone) = &user.phone {
                    return Some(
                        NodeBuilder::new(UsyncProtocolKind::Contact.as_str())
                            .string_content(phone.clone())
                            .build(),
                    );
                }
                if let Some(username) = &user.username {
                    let mut builder = NodeBuilder::new(UsyncProtocolKind::Contact.as_str())
                        .attr(ATTR_USERNAME, username.clone());
                    if let Some(pin) = &user.username_pin {
                        builder = builder.attr(ATTR_PIN, pin.clone());
                    }
                    if let Some(lid) = &user.known_lid {
                        builder = builder.attr(ATTR_LID, lid.clone());
                    }
                    return Some(builder.build());
                }
                user.contact_type.as_ref().map(|contact_type| {
                    NodeBuilder::new(UsyncProtocolKind::Contact.as_str())
                        .attr(ATTR_TYPE, contact_type.clone())
                        .build()
                })
            }
            Self::DevicesV2 => user.device_sync.as_ref().and_then(|hint| {
                if hint.is_empty() {
                    return None;
                }
                let mut builder = NodeBuilder::new(UsyncProtocolKind::Devices.as_str());
                if let Some(hash) = &hint.device_hash {
                    builder = builder.attr(ATTR_DEVICE_HASH, hash.clone());
                }
                if let Some(timestamp) = hint.timestamp {
                    builder = builder.attr(ATTR_TIMESTAMP, timestamp.to_string());
                }
                if let Some(expected) = hint.expected_timestamp {
                    builder = builder.attr(ATTR_EXPECTED_TIMESTAMP, expected.to_string());
                }
                Some(builder.build())
            }),
            Self::Status => user
                .tc_token
                .as_ref()
                .map(|token| build_tc_token_node(token)),
            Self::Lid => user.known_lid.as_ref().map(|lid| {
                NodeBuilder::new(UsyncProtocolKind::Lid.as_str())
                    .attr(ATTR_JID, lid.clone())
                    .build()
            }),
            Self::BotProfileV1 => {
                let mut profile = NodeBuilder::new(TAG_PROFILE);
                if let Some(persona_id) = &user.persona_id {
                    profile = profile.attr(ATTR_PERSONA_ID, persona_id.clone());
                }
                Some(
                    NodeBuilder::new(UsyncProtocolKind::Bot.as_str())
                        .children([profile.build()])
                        .build(),
                )
            }
            _ => None,
        }
    }
}

/// Validation failure at the boundary of the typed USync API.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum UsyncValidationError {
    #[error("a USync query must contain at least one protocol")]
    EmptyProtocols,
    #[error("a USync query must contain at least one user")]
    EmptyUsers,
    #[error("duplicate USync protocol: {0}")]
    DuplicateProtocol(UsyncProtocolKind),
    #[error("the USync feature protocol requires at least one feature")]
    EmptyFeatureSet,
    #[error("USync user at index {index} has no identity")]
    MissingUserIdentity { index: usize },
    #[error("USync user at index {index} has an invalid JID: {jid}")]
    InvalidUserJid { index: usize, jid: String },
    #[error("USync user at index {index} has a non-PN pn_jid value")]
    InvalidPnJid { index: usize },
    #[error("USync user at index {index} has an empty phone number")]
    EmptyPhone { index: usize },
    #[error("USync user at index {index} has an invalid phone number")]
    InvalidPhone { index: usize },
    #[error("USync user at index {index} has an empty username")]
    EmptyUsername { index: usize },
    #[error("USync user at index {index} has a username pin without a username")]
    UsernamePinWithoutUsername { index: usize },
    #[error("USync user at index {index} has a non-LID known_lid value")]
    InvalidKnownLid { index: usize },
    #[error("USync user at index {index} has an empty device hash")]
    EmptyDeviceHash { index: usize },
    #[error("USync user at index {index} has conflicting contact inputs")]
    ConflictingContactInputs { index: usize },
    #[error("USync user at index {index} has contact input without the contact protocol")]
    ContactInputWithoutProtocol { index: usize },
    #[error("USync user at index {index} has device sync input without the devices protocol")]
    DeviceSyncWithoutProtocol { index: usize },
    #[error("USync user at index {index} has a trusted-contact token without the status protocol")]
    TcTokenWithoutProtocol { index: usize },
    #[error("USync user at index {index} has a persona ID without the bot protocol")]
    PersonaIdWithoutProtocol { index: usize },
    #[error("USync user at index {index} has a known LID unused by the selected protocols")]
    KnownLidWithoutProtocol { index: usize },
    #[error("USync sid must not be empty")]
    EmptySid,
}

/// A complete typed USync query, validated before it reaches the network.
///
/// Deserialization always delegates to [`Self::new`], so a serialized input
/// cannot bypass protocol uniqueness or per-user validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UsyncQueryData")]
pub struct UsyncQuery {
    mode: UsyncMode,
    context: UsyncContext,
    protocols: Vec<UsyncProtocol>,
    users: Vec<UsyncUser>,
}

#[derive(Deserialize)]
struct UsyncQueryData {
    mode: UsyncMode,
    context: UsyncContext,
    protocols: Vec<UsyncProtocol>,
    users: Vec<UsyncUser>,
}

impl TryFrom<UsyncQueryData> for UsyncQuery {
    type Error = UsyncValidationError;

    fn try_from(value: UsyncQueryData) -> Result<Self, Self::Error> {
        Self::new(value.mode, value.context, value.protocols, value.users)
    }
}

impl UsyncQuery {
    pub fn new(
        mode: UsyncMode,
        context: UsyncContext,
        protocols: Vec<UsyncProtocol>,
        users: Vec<UsyncUser>,
    ) -> Result<Self, UsyncValidationError> {
        if protocols.is_empty() {
            return Err(UsyncValidationError::EmptyProtocols);
        }
        if users.is_empty() {
            return Err(UsyncValidationError::EmptyUsers);
        }

        let mut seen = 0_u16;
        for protocol in &protocols {
            protocol.validate()?;
            let kind = protocol.kind();
            let bit = kind.bit();
            if seen & bit != 0 {
                return Err(UsyncValidationError::DuplicateProtocol(kind));
            }
            seen |= bit;
        }
        for (index, user) in users.iter().enumerate() {
            user.validate(index, &protocols)?;
        }

        Ok(Self {
            mode,
            context,
            protocols,
            users,
        })
    }

    pub fn mode(&self) -> UsyncMode {
        self.mode
    }

    pub fn context(&self) -> UsyncContext {
        self.context
    }

    pub fn protocols(&self) -> &[UsyncProtocol] {
        &self.protocols
    }

    pub fn users(&self) -> &[UsyncUser] {
        &self.users
    }

    pub(super) fn from_parts_unchecked(
        mode: UsyncMode,
        context: UsyncContext,
        protocols: Vec<UsyncProtocol>,
        users: Vec<UsyncUser>,
    ) -> Self {
        Self {
            mode,
            context,
            protocols,
            users,
        }
    }

    pub(crate) fn build_node(&self, sid: &str) -> Node {
        let query = NodeBuilder::new(TAG_QUERY)
            .children(self.protocols.iter().map(UsyncProtocol::build_query_node))
            .build();

        let users = self.users.iter().map(|user| {
            let mut builder = NodeBuilder::new(TAG_USER);
            if let Some(jid) = &user.id {
                builder = builder.attr(ATTR_JID, jid.clone());
            }
            if let Some(pn_jid) = &user.pn_jid {
                builder = builder.attr(ATTR_PN_JID, pn_jid.clone());
            }
            builder
                .children(
                    self.protocols
                        .iter()
                        .filter_map(|protocol| protocol.build_user_node(user)),
                )
                .build()
        });

        let list = NodeBuilder::new(TAG_LIST).children(users).build();
        NodeBuilder::new(TAG_USYNC)
            .attr(ATTR_SID, sid)
            .attr(ATTR_MODE, self.mode.as_str())
            .attr(ATTR_CONTEXT, self.context.as_str())
            .attr(ATTR_INDEX, FIRST_PAGE_INDEX)
            .attr(ATTR_LAST, LAST_PAGE)
            .children([query, list])
            .build()
    }

    pub(super) fn build_info_query(&self, sid: &str) -> InfoQuery<'static> {
        InfoQuery::get(
            USYNC_NAMESPACE,
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![self.build_node(sid)])),
        )
    }
}

/// Low-level IQ spec used by the runtime-aware client wrapper.
#[derive(Debug, Clone)]
pub struct UsyncQuerySpec {
    query: UsyncQuery,
    sid: String,
}

impl UsyncQuerySpec {
    pub fn new(query: UsyncQuery, sid: impl Into<String>) -> Result<Self, UsyncValidationError> {
        let sid = sid.into();
        if sid.is_empty() {
            return Err(UsyncValidationError::EmptySid);
        }
        Ok(Self { query, sid })
    }
}

impl IqSpec for UsyncQuerySpec {
    type Response = UsyncResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        self.query.build_info_query(&self.sid)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        parse_usync_response(response)
    }
}

/// Result-level state for a single protocol.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncProtocolState {
    pub protocol: UsyncProtocolKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<UsyncSubprotocolError>,
}

/// Full neutral response from a typed USync query.
///
/// Its serialization walks this model by reference; no projection tree is
/// constructed by the core.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncResponse {
    pub protocol_states: Vec<UsyncProtocolState>,
    pub users: Vec<UsyncUserResult>,
}

impl UsyncResponse {
    pub fn protocol_state(&self, protocol: UsyncProtocolKind) -> Option<&UsyncProtocolState> {
        self.protocol_states
            .iter()
            .find(|state| state.protocol == protocol)
    }
}

/// Per-user response. `id` is optional because contact-only results without a
/// JID are accepted by WhatsApp Web.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncUserResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Jid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pn_jid: Option<Jid>,
    pub protocols: Vec<UsyncProtocolResult>,
}

impl UsyncUserResult {
    pub fn protocol(&self, kind: UsyncProtocolKind) -> Option<&UsyncProtocolResult> {
        self.protocols.iter().find(|result| result.kind() == kind)
    }
}

/// A protocol value or its per-user error. Errors are boxed because they are
/// rare and contain owned strings; this keeps every successful result variant
/// from inheriting their size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum UsyncOutcome<T> {
    Value(T),
    Error(Box<UsyncSubprotocolError>),
}

impl<T> UsyncOutcome<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Error(_) => None,
        }
    }

    pub fn error(&self) -> Option<&UsyncSubprotocolError> {
        match self {
            Self::Value(_) => None,
            Self::Error(error) => Some(error),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncContactResult {
    pub contact_type: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncDeviceResult {
    pub id: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_index: Option<u32>,
    pub is_hosted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncDeviceListResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<CompactString>,
    pub devices: Vec<UsyncDeviceResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncKeyIndexResult {
    pub timestamp: i64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "crate::serde_helpers::serialize_optional_bytes",
        deserialize_with = "crate::serde_helpers::deserialize_optional_bytes"
    )]
    pub signed_key_index_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_timestamp: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncDevicesResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_list: Option<UsyncDeviceListResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_index: Option<UsyncKeyIndexResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncBusinessResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_name: Option<VerifiedName>,
}

/// About/status payload. WhatsApp Web consumes only `status`, while the
/// optional wire timestamp is retained for callers that need a lossless
/// projection of responses carrying `t`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncStatusResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncDisappearingModeResult {
    pub duration_seconds: u32,
    pub setting_timestamp: i64,
    pub ephemerality_disabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncTextStatusResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ephemeral_duration_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_update_time: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncFeatureResult {
    pub feature: UsyncFeature,
    pub value: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncBotPrompt {
    pub emoji: CompactString,
    pub text: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncBotCommand {
    pub name: CompactString,
    pub description: CompactString,
}

#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum UsyncBotProfessionalType {
    #[wire = "unknown"]
    Unknown,
    #[wire = "yes"]
    Yes,
    #[wire = "no"]
    No,
    #[wire_fallback]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncBotProfileResult {
    pub name: CompactString,
    pub attributes: CompactString,
    pub description: CompactString,
    pub category: CompactString,
    pub is_default: bool,
    pub prompts: Vec<UsyncBotPrompt>,
    pub persona_id: CompactString,
    pub commands: Vec<UsyncBotCommand>,
    pub commands_description: CompactString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_meta_created: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_name: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_profile_url: Option<CompactString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub posing_as_professional: Option<UsyncBotProfessionalType>,
}

/// Sparse per-user result. Large, uncommon payloads are boxed so their layout
/// does not inflate every status/contact/device item in large sync responses.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[wire(tag = "type", content = "data")]
#[non_exhaustive]
pub enum UsyncProtocolResult {
    #[wire = "contact"]
    Contact(UsyncOutcome<UsyncContactResult>),
    #[wire = "devices"]
    Devices(UsyncOutcome<UsyncDevicesResult>),
    #[wire = "status"]
    Status(UsyncOutcome<UsyncStatusResult>),
    #[wire = "text_status"]
    TextStatus(UsyncOutcome<UsyncTextStatusResult>),
    #[wire = "disappearing_mode"]
    DisappearingMode(UsyncOutcome<UsyncDisappearingModeResult>),
    #[wire = "business"]
    Business(UsyncOutcome<Box<UsyncBusinessResult>>),
    #[wire = "picture"]
    Picture(UsyncOutcome<u64>),
    #[wire = "lid"]
    Lid(UsyncOutcome<Option<Jid>>),
    #[wire = "username"]
    Username(UsyncOutcome<Option<CompactString>>),
    #[wire = "bot"]
    Bot(UsyncOutcome<Box<UsyncBotProfileResult>>),
    #[wire = "feature"]
    Features(UsyncOutcome<Vec<UsyncFeatureResult>>),
}

impl UsyncProtocolResult {
    pub const fn kind(&self) -> UsyncProtocolKind {
        match self {
            Self::Contact(_) => UsyncProtocolKind::Contact,
            Self::Devices(_) => UsyncProtocolKind::Devices,
            Self::Status(_) => UsyncProtocolKind::Status,
            Self::TextStatus(_) => UsyncProtocolKind::TextStatus,
            Self::DisappearingMode(_) => UsyncProtocolKind::DisappearingMode,
            Self::Business(_) => UsyncProtocolKind::Business,
            Self::Picture(_) => UsyncProtocolKind::Picture,
            Self::Lid(_) => UsyncProtocolKind::Lid,
            Self::Username(_) => UsyncProtocolKind::Username,
            Self::Bot(_) => UsyncProtocolKind::Bot,
            Self::Features(_) => UsyncProtocolKind::Feature,
        }
    }
}

pub(crate) fn parse_usync_response(response: &NodeRef<'_>) -> Result<UsyncResponse, anyhow::Error> {
    let usync = response
        .get_optional_child(TAG_USYNC)
        .ok_or_else(|| anyhow!("USync response is missing <{TAG_USYNC}>"))?;

    let protocol_states = match usync.get_optional_child(TAG_RESULT) {
        Some(result) => parse_protocol_states(result)?,
        None => Vec::new(),
    };

    let list = usync
        .get_optional_child(TAG_LIST)
        .ok_or_else(|| anyhow!("USync response is missing <{TAG_LIST}>"))?;
    let capacity = list.children().map_or(0, <[_]>::len);
    let mut users = Vec::with_capacity(capacity);
    for user in list.get_children_by_tag(TAG_USER) {
        if let Some(parsed) = parse_user_result(user)? {
            users.push(parsed);
        }
    }

    Ok(UsyncResponse {
        protocol_states,
        users,
    })
}

fn parse_protocol_states(result: &NodeRef<'_>) -> Result<Vec<UsyncProtocolState>, anyhow::Error> {
    let capacity = result.children().map_or(0, <[_]>::len);
    let mut states = Vec::with_capacity(capacity);
    for node in result.children().into_iter().flatten() {
        let Ok(protocol) = UsyncProtocolKind::try_from(node.tag.as_ref()) else {
            continue;
        };
        let error = parse_subprotocol_error(node)?;
        let mut attrs = node.attrs();
        let refresh_seconds = attrs
            .optional_u64(ATTR_REFRESH)
            .map(|value| checked_u32(value, ATTR_REFRESH))
            .transpose()?;
        attrs.finish()?;
        states.push(UsyncProtocolState {
            protocol,
            refresh_seconds,
            error,
        });
    }
    Ok(states)
}

fn parse_user_result(user: &NodeRef<'_>) -> Result<Option<UsyncUserResult>, anyhow::Error> {
    let mut attrs = user.attrs();
    let id = attrs.optional_jid(ATTR_JID).map(normalize_usync_user_jid);
    let pn_jid = attrs
        .optional_jid(ATTR_PN_JID)
        .map(normalize_usync_user_jid);
    attrs.finish()?;

    let capacity = user.children().map_or(0, <[_]>::len);
    let mut protocols = Vec::with_capacity(capacity);
    let mut has_contact = false;
    for node in user.children().into_iter().flatten() {
        let Ok(tag) = UsyncProtocolResultTag::try_from(node.tag.as_ref()) else {
            continue;
        };
        has_contact |= tag == UsyncProtocolResultTag::Contact;
        protocols.push(parse_protocol_result(tag, node)?);
    }

    // Matches WA Web's parser: an id-less entry is retained only when the
    // contact protocol returned a node.
    if id.is_none() && !has_contact {
        return Ok(None);
    }

    Ok(Some(UsyncUserResult {
        id,
        pn_jid,
        protocols,
    }))
}

fn parse_protocol_result(
    tag: UsyncProtocolResultTag,
    node: &NodeRef<'_>,
) -> Result<UsyncProtocolResult, anyhow::Error> {
    macro_rules! outcome {
        ($parser:expr) => {
            match parse_subprotocol_error(node)? {
                Some(error) => UsyncOutcome::Error(Box::new(error)),
                None => UsyncOutcome::Value($parser?),
            }
        };
    }

    Ok(match tag {
        UsyncProtocolResultTag::Contact => {
            UsyncProtocolResult::Contact(outcome!(parse_contact(node)))
        }
        UsyncProtocolResultTag::Devices => {
            UsyncProtocolResult::Devices(outcome!(parse_devices(node)))
        }
        UsyncProtocolResultTag::Status => UsyncProtocolResult::Status(outcome!(parse_status(node))),
        UsyncProtocolResultTag::TextStatus => {
            UsyncProtocolResult::TextStatus(outcome!(parse_text_status(node)))
        }
        UsyncProtocolResultTag::DisappearingMode => {
            UsyncProtocolResult::DisappearingMode(outcome!(parse_disappearing_mode(node)))
        }
        UsyncProtocolResultTag::Business => {
            UsyncProtocolResult::Business(outcome!(parse_business(node).map(Box::new)))
        }
        UsyncProtocolResultTag::Picture => {
            UsyncProtocolResult::Picture(outcome!(required_u64(node, ATTR_ID)))
        }
        UsyncProtocolResultTag::Lid => {
            UsyncProtocolResult::Lid(outcome!(optional_lid(node, ATTR_VAL)))
        }
        UsyncProtocolResultTag::Username => {
            UsyncProtocolResult::Username(outcome!(Ok::<_, anyhow::Error>(
                node.content_as_string()
            )))
        }
        UsyncProtocolResultTag::Bot => {
            UsyncProtocolResult::Bot(outcome!(parse_bot_profile(node).map(Box::new)))
        }
        UsyncProtocolResultTag::Features => {
            UsyncProtocolResult::Features(outcome!(parse_features(node)))
        }
    })
}

fn parse_subprotocol_error(
    protocol: &NodeRef<'_>,
) -> Result<Option<UsyncSubprotocolError>, anyhow::Error> {
    let Some(error) = protocol.get_optional_child(TAG_ERROR) else {
        return Ok(None);
    };
    let mut attrs = error.attrs();
    let code = attrs
        .optional_u64(ATTR_CODE)
        .map(|value| checked_u16(value, ATTR_CODE))
        .transpose()?;
    let text = attrs
        .optional_string(ATTR_TEXT)
        .map(|value| value.into_owned());
    let backoff = attrs
        .optional_u64(ATTR_BACKOFF)
        .map(|value| checked_u32(value, ATTR_BACKOFF))
        .transpose()?;
    attrs.finish()?;
    Ok(Some(UsyncSubprotocolError {
        code,
        text,
        backoff,
    }))
}

fn parse_contact(node: &NodeRef<'_>) -> Result<UsyncContactResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let contact_type = CompactString::from(attrs.required_string(ATTR_TYPE)?.as_ref());
    let username = attrs
        .optional_string(ATTR_USERNAME)
        .map(|value| CompactString::from(value.as_ref()));
    attrs.finish()?;
    Ok(UsyncContactResult {
        contact_type,
        username,
        content: node.content_as_string(),
    })
}

fn parse_devices(node: &NodeRef<'_>) -> Result<UsyncDevicesResult, anyhow::Error> {
    let device_list = node
        .get_optional_child(TAG_DEVICE_LIST)
        .map(parse_device_list)
        .transpose()?;
    let key_index = node
        .get_optional_child(TAG_KEY_INDEX_LIST)
        .map(parse_key_index)
        .transpose()?;
    Ok(UsyncDevicesResult {
        device_list,
        key_index,
    })
}

fn parse_device_list(node: &NodeRef<'_>) -> Result<UsyncDeviceListResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let hash = attrs
        .optional_string(ATTR_HASH)
        .map(|value| CompactString::from(value.as_ref()));
    attrs.finish()?;

    let capacity = node.children().map_or(0, <[_]>::len);
    let mut devices = Vec::with_capacity(capacity);
    for device in node.get_children_by_tag(TAG_DEVICE) {
        devices.push(parse_device(device)?);
    }
    Ok(UsyncDeviceListResult { hash, devices })
}

fn parse_device(node: &NodeRef<'_>) -> Result<UsyncDeviceResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let raw_id = attrs
        .optional_u64(ATTR_ID)
        .ok_or_else(|| anyhow!("USync device is missing '{ATTR_ID}'"))?;
    let id = checked_u16(raw_id, ATTR_ID)?;
    let key_index = attrs
        .optional_u64(ATTR_KEY_INDEX)
        .map(|value| checked_u32(value, ATTR_KEY_INDEX))
        .transpose()?;
    let is_hosted = attrs.optional_bool_value(ATTR_IS_HOSTED).unwrap_or(false);
    attrs.finish()?;
    Ok(UsyncDeviceResult {
        id,
        key_index,
        is_hosted,
    })
}

fn parse_key_index(node: &NodeRef<'_>) -> Result<UsyncKeyIndexResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let timestamp = attrs
        .optional_unix_time(ATTR_TIMESTAMP)
        .ok_or_else(|| anyhow!("USync key-index-list is missing '{ATTR_TIMESTAMP}'"))?;
    let expected_timestamp = attrs.optional_unix_time(ATTR_EXPECTED_TIMESTAMP);
    attrs.finish()?;

    let signed_key_index_bytes = match node.content.as_deref() {
        Some(NodeContentRef::Bytes(bytes)) => Some(bytes.to_vec()),
        None => None,
        Some(_) => {
            return Err(anyhow!(
                "USync key-index-list contains non-binary signed key bytes"
            ));
        }
    };
    Ok(UsyncKeyIndexResult {
        timestamp,
        signed_key_index_bytes,
        expected_timestamp,
    })
}

fn parse_status(node: &NodeRef<'_>) -> Result<UsyncStatusResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let code = attrs.optional_u64(ATTR_CODE);
    let timestamp = attrs.optional_unix_time(ATTR_TIME);
    attrs.finish()?;
    let status = match node.content_as_string() {
        Some(content) if !content.is_empty() => Some(content),
        Some(_) => None,
        None if code == Some(401) => Some(CompactString::from("")),
        None => None,
    };
    Ok(UsyncStatusResult { status, timestamp })
}

fn parse_text_status(node: &NodeRef<'_>) -> Result<UsyncTextStatusResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let text = attrs
        .optional_string(ATTR_TEXT)
        .map(|value| CompactString::from(value.as_ref()));
    let ephemeral_duration_seconds = attrs
        .optional_u64(ATTR_EPHEMERAL_DURATION)
        .map(|value| checked_u32(value, ATTR_EPHEMERAL_DURATION))
        .transpose()?;
    let last_update_time = attrs
        .optional_string(ATTR_LAST_UPDATE_TIME)
        .map(|value| CompactString::from(value.as_ref()));
    attrs.finish()?;

    let emoji = match node.get_optional_child(TAG_EMOJI) {
        Some(emoji) => optional_compact_string(emoji, ATTR_CONTENT)?,
        None => None,
    };
    Ok(UsyncTextStatusResult {
        text,
        emoji,
        ephemeral_duration_seconds,
        last_update_time,
    })
}

fn parse_disappearing_mode(
    node: &NodeRef<'_>,
) -> Result<UsyncDisappearingModeResult, anyhow::Error> {
    let mut attrs = node.attrs();
    let duration_seconds = attrs
        .optional_u64(ATTR_DURATION)
        .map(|value| checked_u32(value, ATTR_DURATION))
        .transpose()?
        .unwrap_or_default();
    let setting_timestamp = attrs
        .optional_unix_time(ATTR_TIME)
        .ok_or_else(|| anyhow!("USync disappearing mode is missing '{ATTR_TIME}'"))?;
    let ephemerality_disabled = attrs
        .optional_bool_value(ATTR_EPHEMERALITY_DISABLED)
        .unwrap_or(false);
    attrs.finish()?;
    Ok(UsyncDisappearingModeResult {
        duration_seconds,
        setting_timestamp,
        ephemerality_disabled,
    })
}

fn parse_business(node: &NodeRef<'_>) -> Result<UsyncBusinessResult, anyhow::Error> {
    let verified_name = match node.get_optional_child(TAG_VERIFIED_NAME) {
        Some(verified_name) if verified_name.get_optional_child(TAG_ERROR).is_none() => {
            let parsed = VerifiedName::try_from_node(verified_name)?;
            (parsed.name.is_some()
                || parsed.serial.is_some()
                || parsed.issuer.is_some()
                || parsed.certificate.is_some())
            .then_some(parsed)
        }
        _ => None,
    };
    Ok(UsyncBusinessResult { verified_name })
}

fn parse_features(node: &NodeRef<'_>) -> Result<Vec<UsyncFeatureResult>, anyhow::Error> {
    let capacity = node.children().map_or(0, <[_]>::len);
    let mut features = Vec::with_capacity(capacity);
    for child in node.children().into_iter().flatten() {
        let Ok(feature) = UsyncFeature::try_from(child.tag.as_ref()) else {
            continue;
        };
        let value = required_compact_string(child, ATTR_VALUE)?;
        features.push(UsyncFeatureResult { feature, value });
    }
    Ok(features)
}

fn parse_bot_profile(node: &NodeRef<'_>) -> Result<UsyncBotProfileResult, anyhow::Error> {
    let profile = required_child(node, TAG_PROFILE)?;
    let name = required_child_text(profile, TAG_NAME)?;
    let attributes = required_child_text(profile, TAG_ATTRIBUTES)?;
    let description = required_child_text(profile, TAG_DESCRIPTION)?;
    let category = required_child_text(profile, TAG_CATEGORY)?;
    let is_default = profile
        .get_optional_child(TAG_DEFAULT)
        .and_then(NodeRef::content_as_string)
        .is_some_and(|value| value == "true");
    let persona_id = optional_compact_string(profile, ATTR_PERSONA_ID)?.unwrap_or_default();
    let prompts = parse_bot_prompts(profile.get_optional_child(TAG_PROMPTS))?;
    let (commands, commands_description) =
        parse_bot_commands(profile.get_optional_child(TAG_COMMANDS))?;
    let is_meta_created = profile
        .get_optional_child(TAG_IS_META_CREATED)
        .and_then(NodeRef::content_as_string)
        .map(|value| value == "true");

    let (creator_name, creator_profile_url) = match profile.get_optional_child(TAG_CREATOR) {
        Some(creator) => (
            optional_child_text(creator, TAG_NAME),
            optional_child_text(creator, TAG_PROFILE_URL),
        ),
        None => (None, None),
    };
    let posing_as_professional = profile
        .get_optional_child(TAG_POSING_AS_PROFESSIONAL)
        .map(parse_bot_professional_type)
        .transpose()?;

    Ok(UsyncBotProfileResult {
        name,
        attributes,
        description,
        category,
        is_default,
        prompts,
        persona_id,
        commands,
        commands_description,
        is_meta_created,
        creator_name,
        creator_profile_url,
        posing_as_professional,
    })
}

fn parse_bot_prompts(node: Option<&NodeRef<'_>>) -> Result<Vec<UsyncBotPrompt>, anyhow::Error> {
    let Some(node) = node else {
        return Ok(Vec::new());
    };
    let capacity = node.children().map_or(0, <[_]>::len);
    let mut prompts = Vec::with_capacity(capacity);
    for prompt in node.get_children_by_tag(TAG_PROMPT) {
        prompts.push(UsyncBotPrompt {
            emoji: optional_child_text(prompt, TAG_EMOJI).unwrap_or_default(),
            text: optional_child_text(prompt, TAG_TEXT).unwrap_or_default(),
        });
    }
    Ok(prompts)
}

fn parse_bot_commands(
    node: Option<&NodeRef<'_>>,
) -> Result<(Vec<UsyncBotCommand>, CompactString), anyhow::Error> {
    let Some(node) = node else {
        return Ok((Vec::new(), CompactString::default()));
    };
    let capacity = node.children().map_or(0, <[_]>::len);
    let mut commands = Vec::with_capacity(capacity);
    for command in node.get_children_by_tag(TAG_COMMAND) {
        commands.push(UsyncBotCommand {
            name: optional_child_text(command, TAG_NAME).unwrap_or_default(),
            description: optional_child_text(command, TAG_DESCRIPTION).unwrap_or_default(),
        });
    }
    Ok((
        commands,
        optional_child_text(node, TAG_DESCRIPTION).unwrap_or_default(),
    ))
}

fn required_child<'a>(node: &'a NodeRef<'a>, tag: &str) -> Result<&'a NodeRef<'a>, anyhow::Error> {
    node.get_optional_child(tag)
        .ok_or_else(|| anyhow!("<{}> is missing required <{tag}> child", node.tag))
}

fn required_child_text(node: &NodeRef<'_>, tag: &str) -> Result<CompactString, anyhow::Error> {
    required_child(node, tag)?
        .content_as_string()
        .ok_or_else(|| anyhow!("<{tag}> is missing text content"))
}

fn optional_child_text(node: &NodeRef<'_>, tag: &str) -> Option<CompactString> {
    node.get_optional_child(tag)
        .and_then(NodeRef::content_as_string)
}

fn optional_compact_string(
    node: &NodeRef<'_>,
    key: &str,
) -> Result<Option<CompactString>, anyhow::Error> {
    let mut attrs = node.attrs();
    let value = attrs
        .optional_string(key)
        .map(|value| CompactString::from(value.as_ref()));
    attrs.finish()?;
    Ok(value)
}

fn required_compact_string(node: &NodeRef<'_>, key: &str) -> Result<CompactString, anyhow::Error> {
    let mut attrs = node.attrs();
    let value = CompactString::from(attrs.required_string(key)?.as_ref());
    attrs.finish()?;
    Ok(value)
}

fn required_u64(node: &NodeRef<'_>, key: &str) -> Result<u64, anyhow::Error> {
    let mut attrs = node.attrs();
    let value = attrs
        .optional_u64(key)
        .ok_or_else(|| anyhow!("<{}> is missing required '{key}' attribute", node.tag))?;
    attrs.finish()?;
    Ok(value)
}

fn parse_bot_professional_type(
    node: &NodeRef<'_>,
) -> Result<UsyncBotProfessionalType, anyhow::Error> {
    let mut attrs = node.attrs();
    let value = UsyncBotProfessionalType::from(attrs.required_string(ATTR_TYPE)?.as_ref());
    attrs.finish()?;
    Ok(value)
}

fn optional_lid(node: &NodeRef<'_>, key: &str) -> Result<Option<Jid>, anyhow::Error> {
    let mut attrs = node.attrs();
    let value = attrs.optional_jid(key).map(|jid| jid.to_non_ad());
    attrs.finish()?;
    match value {
        Some(jid) if !jid.server.is_lid_family() => Err(anyhow!(
            "<{}> contains a non-LID '{key}' attribute",
            node.tag
        )),
        value => Ok(value),
    }
}

fn checked_u16(value: u64, attribute: &str) -> Result<u16, anyhow::Error> {
    u16::try_from(value).with_context(|| format!("'{attribute}' value {value} exceeds u16"))
}

fn checked_u32(value: u64, attribute: &str) -> Result<u32, anyhow::Error> {
    u32::try_from(value).with_context(|| format!("'{attribute}' value {value} exceeds u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_PHONE: &str = "13135550100";
    const TEST_E164_PHONE: &str = "+13135550100";
    const TEST_PN_JID: &str = "13135550100@s.whatsapp.net";
    const TEST_AGENT_BOT_JID: &str = "13135550100.2@bot";

    fn response(result: Vec<Node>, users: Vec<Node>) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new(TAG_USYNC)
                .children([
                    NodeBuilder::new(TAG_RESULT).children(result).build(),
                    NodeBuilder::new(TAG_LIST).children(users).build(),
                ])
                .build()])
            .build()
    }

    fn protocol_error(code: u16, text: &str, backoff: Option<u32>) -> Node {
        let mut error = NodeBuilder::new(TAG_ERROR)
            .attr(ATTR_CODE, code.to_string())
            .attr(ATTR_TEXT, text);
        if let Some(backoff) = backoff {
            error = error.attr(ATTR_BACKOFF, backoff.to_string());
        }
        error.build()
    }

    #[test]
    fn typed_query_builds_canonical_envelope_and_protocol_inputs() {
        let user = UsyncUser::from_jid(Jid::lid("100000001").with_device(7))
            .with_pn_jid(Jid::pn(TEST_PHONE).with_device(8))
            .with_known_lid(Jid::lid("100000001").with_device(9))
            .with_device_sync(
                UsyncDeviceSyncHint::new()
                    .with_device_hash("2:hash")
                    .with_timestamp(100)
                    .with_expected_timestamp(101),
            )
            .with_tc_token(vec![1, 2, 3]);
        let query = UsyncQuery::new(
            UsyncMode::Delta,
            UsyncContext::Voip,
            vec![
                UsyncProtocol::Contact {
                    addressing_mode: UsyncAddressingMode::Lid,
                },
                UsyncProtocol::DevicesV2,
                UsyncProtocol::Status,
                UsyncProtocol::Lid,
                UsyncProtocol::Features(vec![UsyncFeature::Voip]),
            ],
            vec![user],
        )
        .unwrap();
        let spec = UsyncQuerySpec::new(query, "sid-1").unwrap();
        let iq = spec.build_iq();
        let NodeContent::Nodes(nodes) = iq.content.as_ref().unwrap() else {
            panic!("USync IQ content must contain child nodes");
        };
        let usync = nodes.first().unwrap();
        assert_eq!(usync.attrs.get(ATTR_SID).unwrap().as_str(), "sid-1");
        assert_eq!(usync.attrs.get(ATTR_MODE).unwrap().as_str(), "delta");
        assert_eq!(usync.attrs.get(ATTR_CONTEXT).unwrap().as_str(), "voip");

        let query_node = usync.get_optional_child(TAG_QUERY).unwrap();
        let contact = query_node
            .get_optional_child(UsyncProtocolKind::Contact.as_str())
            .unwrap();
        assert_eq!(
            contact.attrs.get(ATTR_ADDRESSING_MODE).unwrap().as_str(),
            "lid"
        );
        let feature = query_node
            .get_optional_child(UsyncProtocolKind::Feature.as_str())
            .unwrap();
        assert!(
            feature
                .get_optional_child(UsyncFeature::Voip.as_str())
                .is_some()
        );

        let user = usync
            .get_optional_child(TAG_LIST)
            .and_then(|list| list.get_optional_child(TAG_USER))
            .unwrap();
        assert_eq!(user.attrs.get(ATTR_JID).unwrap().as_str(), "100000001@lid");
        assert_eq!(user.attrs.get(ATTR_PN_JID).unwrap().as_str(), TEST_PN_JID);
        let devices = user
            .get_optional_child(UsyncProtocolKind::Devices.as_str())
            .unwrap();
        assert_eq!(
            devices.attrs.get(ATTR_EXPECTED_TIMESTAMP).unwrap().as_str(),
            "101"
        );
        let lid = user
            .get_optional_child(UsyncProtocolKind::Lid.as_str())
            .unwrap();
        assert_eq!(lid.attrs.get(ATTR_JID).unwrap().as_str(), "100000001@lid");
        assert!(user.get_optional_child("tctoken").is_some());
    }

    #[test]
    fn digit_only_phone_inputs_are_normalized_to_e164() {
        assert_eq!(
            UsyncUser::from_phone(TEST_PHONE).phone.as_deref(),
            Some(TEST_E164_PHONE)
        );
        assert_eq!(
            UsyncUser::from_phone(TEST_E164_PHONE).phone.as_deref(),
            Some(TEST_E164_PHONE)
        );
    }

    #[test]
    fn query_serde_round_trip_reuses_canonical_validation_and_normalization() {
        let query = UsyncQuery::new(
            UsyncMode::Delta,
            UsyncContext::Background,
            vec![
                UsyncProtocol::Contact {
                    addressing_mode: UsyncAddressingMode::Pn,
                },
                UsyncProtocol::DevicesV2,
                UsyncProtocol::Status,
                UsyncProtocol::Features(vec![UsyncFeature::EncryptV2, UsyncFeature::Voip]),
            ],
            vec![
                UsyncUser::from_phone(TEST_PHONE)
                    .with_device_sync(
                        UsyncDeviceSyncHint::new()
                            .with_device_hash("2:hash")
                            .with_timestamp(100),
                    )
                    .with_tc_token(vec![1, 2, 3]),
            ],
        )
        .unwrap();

        let encoded = serde_json::to_vec(&query).unwrap();
        let encoded_value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(encoded_value["protocols"][1]["type"], "devices");
        assert_eq!(encoded_value["protocols"][3]["type"], "feature");
        let decoded: UsyncQuery = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, query);

        let normalized: UsyncQuery = serde_json::from_value(serde_json::json!({
            "mode": "query",
            "context": "interactive",
            "protocols": [{
                "type": "contact",
                "data": { "addressing_mode": "pn" }
            }],
            "users": [{ "phone": TEST_PHONE }]
        }))
        .unwrap();
        assert_eq!(
            normalized.users()[0].phone.as_deref(),
            Some(TEST_E164_PHONE)
        );

        let duplicate = serde_json::from_value::<UsyncQuery>(serde_json::json!({
            "mode": "query",
            "context": "interactive",
            "protocols": [{ "type": "status" }, { "type": "status" }],
            "users": [{ "phone": TEST_PHONE }]
        }))
        .unwrap_err();
        assert!(
            duplicate.to_string().contains("duplicate USync protocol"),
            "unexpected deserialization error: {duplicate}"
        );
    }

    #[test]
    fn query_validation_rejects_non_canonical_phone_inputs() {
        const INVALID_PHONES: &[&str] = &["+", "abc", "+12 34", "+0123", "+1234567890123456"];

        for phone in INVALID_PHONES {
            let error = UsyncQuery::new(
                UsyncMode::Query,
                UsyncContext::Interactive,
                vec![UsyncProtocol::Contact {
                    addressing_mode: UsyncAddressingMode::Pn,
                }],
                vec![UsyncUser::from_phone(*phone)],
            )
            .unwrap_err();
            assert_eq!(error, UsyncValidationError::InvalidPhone { index: 0 });
        }
    }

    #[test]
    fn query_validation_rejects_ambiguous_or_invalid_inputs() {
        let duplicate = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![UsyncProtocol::Status, UsyncProtocol::Status],
            vec![UsyncUser::from_jid(Jid::pn(TEST_PHONE))],
        )
        .unwrap_err();
        assert_eq!(
            duplicate,
            UsyncValidationError::DuplicateProtocol(UsyncProtocolKind::Status)
        );

        let contact_without_protocol = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![UsyncProtocol::Status],
            vec![UsyncUser::from_phone(TEST_E164_PHONE)],
        )
        .unwrap_err();
        assert_eq!(
            contact_without_protocol,
            UsyncValidationError::ContactInputWithoutProtocol { index: 0 }
        );

        let invalid_pn = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![UsyncProtocol::Status],
            vec![UsyncUser::from_pn_jid(Jid::lid("100000001"))],
        )
        .unwrap_err();
        assert_eq!(invalid_pn, UsyncValidationError::InvalidPnJid { index: 0 });

        let empty_hash = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![UsyncProtocol::DevicesV2],
            vec![UsyncUser::from_jid(Jid::pn(TEST_PHONE)).with_device_sync(
                UsyncDeviceSyncHint::new().with_device_hash(CompactString::default()),
            )],
        )
        .unwrap_err();
        assert_eq!(
            empty_hash,
            UsyncValidationError::EmptyDeviceHash { index: 0 }
        );

        let agent_qualified_bot = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![UsyncProtocol::Status],
            vec![UsyncUser::from_jid(Jid {
                user: TEST_PHONE.into(),
                server: Server::Bot,
                agent: 2,
                device: 7,
                integrator: 0,
            })],
        )
        .unwrap_err();
        assert_eq!(
            agent_qualified_bot,
            UsyncValidationError::InvalidUserJid {
                index: 0,
                jid: TEST_AGENT_BOT_JID.to_owned(),
            }
        );
    }

    #[test]
    fn query_validation_never_silently_drops_user_inputs() {
        let cases = [
            (
                UsyncUser::from_jid(Jid::pn(TEST_PHONE))
                    .with_phone(TEST_E164_PHONE)
                    .with_contact_type("in"),
                vec![UsyncProtocol::Contact {
                    addressing_mode: UsyncAddressingMode::Pn,
                }],
                UsyncValidationError::ConflictingContactInputs { index: 0 },
            ),
            (
                UsyncUser::from_jid(Jid::pn(TEST_PHONE))
                    .with_device_sync(UsyncDeviceSyncHint::new().with_timestamp(1)),
                vec![UsyncProtocol::Status],
                UsyncValidationError::DeviceSyncWithoutProtocol { index: 0 },
            ),
            (
                UsyncUser::from_jid(Jid::pn(TEST_PHONE)).with_tc_token(vec![1]),
                vec![UsyncProtocol::DevicesV2],
                UsyncValidationError::TcTokenWithoutProtocol { index: 0 },
            ),
            (
                UsyncUser::from_jid(Jid::pn(TEST_PHONE)).with_persona_id("persona-1"),
                vec![UsyncProtocol::Status],
                UsyncValidationError::PersonaIdWithoutProtocol { index: 0 },
            ),
            (
                UsyncUser::from_jid(Jid::pn(TEST_PHONE)).with_known_lid(Jid::lid("100000001")),
                vec![UsyncProtocol::Status],
                UsyncValidationError::KnownLidWithoutProtocol { index: 0 },
            ),
        ];

        for (user, protocols, expected) in cases {
            let error = UsyncQuery::new(
                UsyncMode::Query,
                UsyncContext::Interactive,
                protocols,
                vec![user],
            )
            .unwrap_err();
            assert_eq!(error, expected);
        }
    }

    #[test]
    fn typed_query_uses_protocol_specific_attribute_names() {
        let query = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![
                UsyncProtocol::Contact {
                    addressing_mode: UsyncAddressingMode::Pn,
                },
                UsyncProtocol::BotProfileV1,
            ],
            vec![
                UsyncUser::from_username("test_user")
                    .with_username_pin("test_pin")
                    .with_known_lid(Jid::lid("100000001"))
                    .with_persona_id("persona-1"),
            ],
        )
        .unwrap();
        let usync = query.build_node("sid-1");
        let query_node = usync.get_optional_child(TAG_QUERY).unwrap();
        let bot_profile = query_node
            .get_optional_child(UsyncProtocolKind::Bot.as_str())
            .and_then(|bot| bot.get_optional_child(TAG_PROFILE))
            .unwrap();
        assert_eq!(
            bot_profile.attrs.get(ATTR_BOT_VERSION).unwrap().as_str(),
            BOT_PROFILE_VERSION
        );
        assert!(!bot_profile.attrs.contains_key(ATTR_VERSION));

        let user = usync
            .get_optional_child(TAG_LIST)
            .and_then(|list| list.get_optional_child(TAG_USER))
            .unwrap();
        let contact = user
            .get_optional_child(UsyncProtocolKind::Contact.as_str())
            .unwrap();
        assert_eq!(contact.attrs.get(ATTR_USERNAME).unwrap(), "test_user");
        assert_eq!(contact.attrs.get(ATTR_PIN).unwrap(), "test_pin");
        assert_eq!(
            contact.attrs.get(ATTR_LID).unwrap().as_str(),
            "100000001@lid"
        );
        assert!(!contact.attrs.contains_key(ATTR_JID));

        let persona = user
            .get_optional_child(UsyncProtocolKind::Bot.as_str())
            .and_then(|bot| bot.get_optional_child(TAG_PROFILE))
            .and_then(|profile| profile.attrs.get(ATTR_PERSONA_ID))
            .unwrap();
        assert_eq!(persona, "persona-1");
    }

    #[test]
    fn parser_preserves_global_errors_refresh_and_idless_contacts() {
        let node = response(
            vec![
                NodeBuilder::new(UsyncProtocolKind::Status.as_str())
                    .attr(ATTR_REFRESH, "300")
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::Devices.as_str())
                    .children([protocol_error(429, "rate limited", Some(30))])
                    .build(),
            ],
            vec![
                NodeBuilder::new(TAG_USER)
                    .children([NodeBuilder::new(UsyncProtocolKind::Contact.as_str())
                        .attr(ATTR_TYPE, "out")
                        .string_content(TEST_E164_PHONE)
                        .build()])
                    .build(),
            ],
        );
        let parsed = parse_usync_response(&node.as_node_ref()).unwrap();
        assert_eq!(parsed.protocol_states.len(), 2);
        assert_eq!(
            parsed
                .protocol_state(UsyncProtocolKind::Status)
                .unwrap()
                .refresh_seconds,
            Some(300)
        );
        let error = parsed
            .protocol_state(UsyncProtocolKind::Devices)
            .unwrap()
            .error
            .as_ref()
            .unwrap();
        assert_eq!(error.code, Some(429));
        assert_eq!(error.backoff, Some(30));
        assert_eq!(parsed.users[0].id, None);
        assert!(matches!(
            parsed.users[0].protocols[0],
            UsyncProtocolResult::Contact(UsyncOutcome::Value(_))
        ));
    }

    #[test]
    fn parser_preserves_identity_bearing_agents_while_stripping_devices() {
        let user = NodeBuilder::new(TAG_USER)
            .attr(
                ATTR_JID,
                Jid {
                    user: TEST_PHONE.into(),
                    server: Server::Bot,
                    agent: 2,
                    device: 7,
                    integrator: 0,
                },
            )
            .children([NodeBuilder::new(UsyncProtocolKind::Status.as_str()).build()])
            .build();

        let parsed = parse_usync_response(&response(Vec::new(), vec![user]).as_node_ref()).unwrap();
        let id = parsed.users[0].id.as_ref().unwrap();
        assert_eq!(id.agent, 2);
        assert_eq!(id.device, 0);
    }

    #[test]
    fn status_parser_preserves_absent_null_empty_and_text() {
        let users = vec![
            NodeBuilder::new(TAG_USER)
                .attr(ATTR_JID, Jid::lid("1"))
                .build(),
            NodeBuilder::new(TAG_USER)
                .attr(ATTR_JID, Jid::lid("2"))
                .children([NodeBuilder::new(UsyncProtocolKind::Status.as_str())
                    .attr(ATTR_TIMESTAMP, "999")
                    .build()])
                .build(),
            NodeBuilder::new(TAG_USER)
                .attr(ATTR_JID, Jid::lid("3"))
                .children([NodeBuilder::new(UsyncProtocolKind::Status.as_str())
                    .attr(ATTR_CODE, "401")
                    .build()])
                .build(),
            NodeBuilder::new(TAG_USER)
                .attr(ATTR_JID, Jid::lid("4"))
                .children([NodeBuilder::new(UsyncProtocolKind::Status.as_str())
                    .attr(ATTR_TIME, "123")
                    .string_content("available")
                    .build()])
                .build(),
        ];
        let parsed = parse_usync_response(&response(Vec::new(), users).as_node_ref()).unwrap();
        assert!(parsed.users[0].protocols.is_empty());
        assert!(matches!(
            &parsed.users[1].protocols[0],
            UsyncProtocolResult::Status(UsyncOutcome::Value(value))
                if value.status.is_none() && value.timestamp.is_none()
        ));
        assert!(matches!(
            &parsed.users[2].protocols[0],
            UsyncProtocolResult::Status(UsyncOutcome::Value(value))
                if value.status.as_ref().is_some_and(CompactString::is_empty)
        ));
        assert!(matches!(
            &parsed.users[3].protocols[0],
            UsyncProtocolResult::Status(UsyncOutcome::Value(value))
                if value.status.as_deref() == Some("available") && value.timestamp == Some(123)
        ));
    }

    #[test]
    fn devices_parser_preserves_hosting_and_key_index_metadata() {
        let devices = NodeBuilder::new(UsyncProtocolKind::Devices.as_str())
            .children([
                NodeBuilder::new(TAG_DEVICE_LIST)
                    .attr(ATTR_HASH, "2:abc")
                    .children([
                        NodeBuilder::new(TAG_DEVICE).attr(ATTR_ID, "0").build(),
                        NodeBuilder::new(TAG_DEVICE)
                            .attr(ATTR_ID, "7")
                            .attr(ATTR_KEY_INDEX, "9")
                            .attr(ATTR_IS_HOSTED, "true")
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new(TAG_KEY_INDEX_LIST)
                    .attr(ATTR_TIMESTAMP, "1000")
                    .attr(ATTR_EXPECTED_TIMESTAMP, "1001")
                    .bytes(vec![1, 2, 3])
                    .build(),
            ])
            .build();
        let user = NodeBuilder::new(TAG_USER)
            .attr(ATTR_JID, Jid::pn(TEST_PHONE))
            .children([devices])
            .build();
        let parsed = parse_usync_response(&response(Vec::new(), vec![user]).as_node_ref()).unwrap();
        let UsyncProtocolResult::Devices(UsyncOutcome::Value(devices)) =
            &parsed.users[0].protocols[0]
        else {
            panic!("expected devices result");
        };
        let list = devices.device_list.as_ref().unwrap();
        assert_eq!(list.hash.as_deref(), Some("2:abc"));
        assert!(list.devices[1].is_hosted);
        assert_eq!(list.devices[1].key_index, Some(9));
        let key_index = devices.key_index.as_ref().unwrap();
        assert_eq!(key_index.timestamp, 1000);
        assert_eq!(key_index.expected_timestamp, Some(1001));
        assert_eq!(
            key_index.signed_key_index_bytes.as_deref(),
            Some(&[1, 2, 3][..])
        );
    }

    #[test]
    fn devices_parser_rejects_malformed_lists_instead_of_returning_partial_data() {
        let malformed_devices = [
            NodeBuilder::new(TAG_DEVICE).build(),
            NodeBuilder::new(TAG_DEVICE)
                .attr(ATTR_ID, "invalid")
                .build(),
            NodeBuilder::new(TAG_DEVICE)
                .attr(ATTR_ID, (u64::from(u16::MAX) + 1).to_string())
                .build(),
        ];

        for malformed in malformed_devices {
            let devices = NodeBuilder::new(UsyncProtocolKind::Devices.as_str())
                .children([NodeBuilder::new(TAG_DEVICE_LIST)
                    .children([
                        NodeBuilder::new(TAG_DEVICE).attr(ATTR_ID, "7").build(),
                        malformed,
                    ])
                    .build()])
                .build();
            let user = NodeBuilder::new(TAG_USER)
                .attr(ATTR_JID, Jid::pn(TEST_PHONE))
                .children([devices])
                .build();

            assert!(parse_usync_response(&response(Vec::new(), vec![user]).as_node_ref()).is_err());
        }
    }

    #[test]
    fn parser_handles_disappearing_text_status_features_and_user_error() {
        let user = NodeBuilder::new(TAG_USER)
            .attr(ATTR_JID, Jid::lid("100000001"))
            .children([
                NodeBuilder::new(UsyncProtocolKind::DisappearingMode.as_str())
                    .attr(ATTR_DURATION, "86400")
                    .attr(ATTR_TIME, "123")
                    .attr(ATTR_EPHEMERALITY_DISABLED, "true")
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::TextStatus.as_str())
                    .attr(ATTR_TEXT, "hello")
                    .attr(ATTR_EPHEMERAL_DURATION, "60")
                    .attr(ATTR_LAST_UPDATE_TIME, "124")
                    .children([NodeBuilder::new(TAG_EMOJI).attr(ATTR_CONTENT, "👋").build()])
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::Feature.as_str())
                    .children([NodeBuilder::new(UsyncFeature::Voip.as_str())
                        .attr(ATTR_VALUE, "1")
                        .build()])
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::Username.as_str())
                    .children([protocol_error(404, "missing", None)])
                    .build(),
            ])
            .build();
        let parsed = parse_usync_response(&response(Vec::new(), vec![user]).as_node_ref()).unwrap();
        assert!(matches!(
            parsed.users[0].protocol(UsyncProtocolKind::DisappearingMode),
            Some(UsyncProtocolResult::DisappearingMode(UsyncOutcome::Value(value)))
                if value.duration_seconds == 86400 && value.ephemerality_disabled
        ));
        assert!(matches!(
            parsed.users[0].protocol(UsyncProtocolKind::TextStatus),
            Some(UsyncProtocolResult::TextStatus(UsyncOutcome::Value(value)))
                if value.emoji.as_deref() == Some("👋")
        ));
        assert!(matches!(
            parsed.users[0].protocol(UsyncProtocolKind::Feature),
            Some(UsyncProtocolResult::Features(UsyncOutcome::Value(values)))
                if values == &[UsyncFeatureResult { feature: UsyncFeature::Voip, value: "1".into() }]
        ));
        assert!(matches!(
            parsed.users[0].protocol(UsyncProtocolKind::Username),
            Some(UsyncProtocolResult::Username(UsyncOutcome::Error(error)))
                if error.code == Some(404)
        ));
    }

    #[test]
    fn parser_projects_picture_lid_username_and_bot_results() {
        let bot_profile = NodeBuilder::new(TAG_PROFILE)
            .attr(ATTR_PERSONA_ID, "persona-1")
            .children([
                NodeBuilder::new(TAG_NAME)
                    .string_content("Test Bot")
                    .build(),
                NodeBuilder::new(TAG_ATTRIBUTES)
                    .string_content("helpful")
                    .build(),
                NodeBuilder::new(TAG_DESCRIPTION)
                    .string_content("Description")
                    .build(),
                NodeBuilder::new(TAG_CATEGORY)
                    .string_content("utility")
                    .build(),
                NodeBuilder::new(TAG_DEFAULT).string_content("true").build(),
                NodeBuilder::new(TAG_PROMPTS)
                    .children([NodeBuilder::new(TAG_PROMPT)
                        .children([
                            NodeBuilder::new(TAG_EMOJI).string_content("👋").build(),
                            NodeBuilder::new(TAG_TEXT).string_content("Hello").build(),
                        ])
                        .build()])
                    .build(),
                NodeBuilder::new(TAG_COMMANDS)
                    .children([
                        NodeBuilder::new(TAG_DESCRIPTION)
                            .string_content("Available commands")
                            .build(),
                        NodeBuilder::new(TAG_COMMAND)
                            .children([
                                NodeBuilder::new(TAG_NAME).string_content("help").build(),
                                NodeBuilder::new(TAG_DESCRIPTION)
                                    .string_content("Show help")
                                    .build(),
                            ])
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new(TAG_IS_META_CREATED)
                    .string_content("false")
                    .build(),
                NodeBuilder::new(TAG_CREATOR)
                    .children([
                        NodeBuilder::new(TAG_NAME).string_content("Creator").build(),
                        NodeBuilder::new(TAG_PROFILE_URL)
                            .string_content("https://example.test/profile")
                            .build(),
                    ])
                    .build(),
                NodeBuilder::new(TAG_POSING_AS_PROFESSIONAL)
                    .attr(ATTR_TYPE, "yes")
                    .build(),
            ])
            .build();
        let user = NodeBuilder::new(TAG_USER)
            .attr(ATTR_JID, Jid::pn(TEST_PHONE))
            .children([
                NodeBuilder::new(UsyncProtocolKind::Picture.as_str())
                    .attr(ATTR_ID, "42")
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::Lid.as_str())
                    .attr(ATTR_VAL, Jid::lid("100000001"))
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::Username.as_str())
                    .string_content("test_user")
                    .build(),
                NodeBuilder::new(UsyncProtocolKind::Bot.as_str())
                    .children([bot_profile])
                    .build(),
            ])
            .build();
        let parsed = parse_usync_response(&response(Vec::new(), vec![user]).as_node_ref()).unwrap();
        let user = &parsed.users[0];
        assert!(matches!(
            user.protocol(UsyncProtocolKind::Picture),
            Some(UsyncProtocolResult::Picture(UsyncOutcome::Value(42)))
        ));
        assert!(matches!(
            user.protocol(UsyncProtocolKind::Lid),
            Some(UsyncProtocolResult::Lid(UsyncOutcome::Value(Some(lid))))
                if *lid == Jid::lid("100000001")
        ));
        assert!(matches!(
            user.protocol(UsyncProtocolKind::Username),
            Some(UsyncProtocolResult::Username(UsyncOutcome::Value(Some(username))))
                if username == "test_user"
        ));
        let Some(UsyncProtocolResult::Bot(UsyncOutcome::Value(bot))) =
            user.protocol(UsyncProtocolKind::Bot)
        else {
            panic!("expected bot result");
        };
        assert_eq!(bot.persona_id, "persona-1");
        assert_eq!(bot.prompts[0].text, "Hello");
        assert_eq!(bot.commands[0].name, "help");
        assert_eq!(bot.commands_description, "Available commands");
        assert_eq!(bot.is_meta_created, Some(false));
        assert_eq!(bot.creator_name.as_deref(), Some("Creator"));
        assert_eq!(
            bot.posing_as_professional,
            Some(UsyncBotProfessionalType::Yes)
        );
        assert_eq!(
            UsyncBotProfessionalType::from("future"),
            UsyncBotProfessionalType::Other("future".to_owned())
        );
    }

    #[test]
    fn picture_without_id_is_rejected() {
        let user = NodeBuilder::new(TAG_USER)
            .attr(ATTR_JID, Jid::pn(TEST_PHONE))
            .children([NodeBuilder::new(UsyncProtocolKind::Picture.as_str()).build()])
            .build();

        assert!(parse_usync_response(&response(Vec::new(), vec![user]).as_node_ref()).is_err());
    }

    #[test]
    fn response_serde_round_trip_preserves_sparse_and_binary_results() {
        let response = UsyncResponse {
            protocol_states: vec![UsyncProtocolState {
                protocol: UsyncProtocolKind::Status,
                refresh_seconds: Some(60),
                error: Some(UsyncSubprotocolError {
                    code: Some(401),
                    text: None,
                    backoff: Some(5),
                }),
            }],
            users: vec![UsyncUserResult {
                id: Some(Jid::new("100000001", Server::HostedLid)),
                pn_jid: Some(Jid::new(TEST_PHONE, Server::Hosted)),
                protocols: vec![
                    UsyncProtocolResult::Devices(UsyncOutcome::Value(UsyncDevicesResult {
                        device_list: Some(UsyncDeviceListResult {
                            hash: Some("2:hash".into()),
                            devices: vec![UsyncDeviceResult {
                                id: 7,
                                key_index: Some(3),
                                is_hosted: true,
                            }],
                        }),
                        key_index: Some(UsyncKeyIndexResult {
                            timestamp: 100,
                            signed_key_index_bytes: Some(vec![1, 2, 3, 4]),
                            expected_timestamp: None,
                        }),
                    })),
                    UsyncProtocolResult::Status(UsyncOutcome::Value(UsyncStatusResult {
                        status: Some(CompactString::default()),
                        timestamp: None,
                    })),
                    UsyncProtocolResult::Business(UsyncOutcome::Value(Box::new(
                        UsyncBusinessResult {
                            verified_name: Some(VerifiedName {
                                name: Some("Business".to_owned()),
                                serial: None,
                                issuer: None,
                                certificate: Some(Vec::new()),
                            }),
                        },
                    ))),
                    UsyncProtocolResult::Bot(UsyncOutcome::Value(Box::new(
                        UsyncBotProfileResult {
                            name: "Assistant".into(),
                            attributes: "helpful".into(),
                            description: "Description".into(),
                            category: "utility".into(),
                            is_default: false,
                            prompts: Vec::new(),
                            persona_id: "persona-1".into(),
                            commands: Vec::new(),
                            commands_description: CompactString::default(),
                            is_meta_created: None,
                            creator_name: None,
                            creator_profile_url: None,
                            posing_as_professional: Some(UsyncBotProfessionalType::Other(
                                "future".to_owned(),
                            )),
                        },
                    ))),
                ],
            }],
        };

        let encoded = serde_json::to_vec(&response).unwrap();
        let decoded: UsyncResponse = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, response);
    }

    #[test]
    fn parser_rejects_non_lid_values() {
        let invalid_lid = NodeBuilder::new(TAG_USER)
            .attr(ATTR_JID, Jid::pn(TEST_PHONE))
            .children([NodeBuilder::new(UsyncProtocolKind::Lid.as_str())
                .attr(ATTR_VAL, Jid::pn(TEST_PHONE))
                .build()])
            .build();
        assert!(
            parse_usync_response(&response(Vec::new(), vec![invalid_lid]).as_node_ref()).is_err()
        );
    }

    #[test]
    fn sparse_result_layout_stays_bounded() {
        assert!(size_of::<UsyncProtocolResult>() <= 96);
    }
}
