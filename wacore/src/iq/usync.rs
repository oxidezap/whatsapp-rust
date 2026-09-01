//! Usync IQ specifications.
//!
//! The usync protocol is used for user synchronization operations including:
//! - Checking if phone numbers or LIDs are registered on WhatsApp
//! - Fetching user information by JID
//! - Fetching device lists
//!
//! ## Wire Format
//! ```xml
//! <!-- Request (phone number query) -->
//! <iq xmlns="usync" type="get" to="s.whatsapp.net" id="...">
//!   <usync sid="..." mode="query" last="true" index="0" context="interactive">
//!     <query>
//!       <contact/>
//!       <lid/>
//!       <business><verified_name/></business>
//!     </query>
//!     <list>
//!       <user>
//!         <contact>+1234567890</contact>
//!       </user>
//!     </list>
//!   </usync>
//! </iq>
//!
//! <!-- Request (LID query) -->
//! <iq xmlns="usync" type="get" to="s.whatsapp.net" id="...">
//!   <usync sid="..." mode="query" last="true" index="0" context="interactive">
//!     <query>
//!       <lid/>
//!       <business><verified_name/></business>
//!     </query>
//!     <list>
//!       <user jid="100000001@lid"/>
//!     </list>
//!   </usync>
//! </iq>
//!
//! <!-- Response -->
//! <iq from="s.whatsapp.net" id="..." type="result">
//!   <usync>
//!     <list>
//!       <user jid="1234567890@s.whatsapp.net" pn_jid="1234567890@s.whatsapp.net">
//!         <contact type="in"/>
//!         <lid val="100000001@lid"/>
//!         <business/>
//!       </user>
//!     </list>
//!   </usync>
//! </iq>
//! ```

use crate::WireEnum;
use crate::iq::spec::IqSpec;
use crate::request::InfoQuery;
use crate::stanza::business::VerifiedName;
use anyhow::anyhow;
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wacore_binary::NodeRef;
use wacore_binary::{CompactString, Jid};

#[cfg(test)]
use wacore_binary::builder::NodeBuilder;
#[cfg(test)]
use wacore_binary::{Node, NodeContent};

mod query;

pub use query::*;

const USYNC_ERROR_PREFIX: &str = "usync ";
const USYNC_ERROR_CODE_SEPARATOR: &str = " error ";
const USYNC_ERROR_TEXT_SEPARATOR: &str = ": ";
const UNKNOWN_ERROR_CODE: &str = "unknown";

/// Usync mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum UsyncMode {
    /// Query mode - used for contact lookups.
    #[wire_default]
    #[wire = "query"]
    Query,
    /// Full mode - used for user info with more details.
    #[wire = "full"]
    Full,
    /// Delta mode - used for incremental contact synchronization.
    #[wire = "delta"]
    Delta,
}

/// Usync context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
pub enum UsyncContext {
    /// Interactive context - for user-initiated operations.
    #[wire_default]
    #[wire = "interactive"]
    Interactive,
    /// Background context - for background sync operations.
    #[wire = "background"]
    Background,
    /// Message context - for message-related operations.
    #[wire = "message"]
    Message,
    /// VoIP context - used by call setup when refreshing device lists.
    #[wire = "voip"]
    Voip,
}

#[derive(Debug, Clone)]
pub struct IsOnWhatsAppUser {
    pub jid: Jid,
    /// Helps server optimize the lookup (WA Web pre-populates this from its LID cache).
    pub known_lid: Option<CompactString>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct UsyncSubprotocolError {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backoff: Option<u32>,
}

fn usync_subprotocol_error_message(tag: &str, error: &UsyncSubprotocolError) -> String {
    let mut code_buffer = itoa::Buffer::new();
    let code = error
        .code
        .map(|code| code_buffer.format(code))
        .unwrap_or(UNKNOWN_ERROR_CODE);
    let text = error.text.as_deref().filter(|text| !text.is_empty());
    let capacity = USYNC_ERROR_PREFIX.len()
        + tag.len()
        + USYNC_ERROR_CODE_SEPARATOR.len()
        + code.len()
        + text.map_or(0, |text| USYNC_ERROR_TEXT_SEPARATOR.len() + text.len());
    let mut message = String::with_capacity(capacity);
    message.push_str(USYNC_ERROR_PREFIX);
    message.push_str(tag);
    message.push_str(USYNC_ERROR_CODE_SEPARATOR);
    message.push_str(code);
    if let Some(text) = text {
        message.push_str(USYNC_ERROR_TEXT_SEPARATOR);
        message.push_str(text);
    }
    message
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct IsOnWhatsAppResult {
    pub jid: Jid,
    pub lid: Option<Jid>,
    /// From `pn_jid` response attribute; present when server returns LID as primary JID.
    pub pn_jid: Option<Jid>,
    pub is_registered: bool,
    pub contact_error: Option<UsyncSubprotocolError>,
    pub lid_error: Option<UsyncSubprotocolError>,
    pub is_business: bool,
    pub business_error: Option<UsyncSubprotocolError>,
    /// Verified business name (decoded from `<business><verified_name>`), if any.
    pub verified_name: Option<VerifiedName>,
    /// Meta username, without the display-only `@` prefix. `None` means the
    /// server reported no username, which is also how it reports one that was
    /// deleted; it is never an empty string.
    pub username: Option<CompactString>,
    pub username_error: Option<UsyncSubprotocolError>,
}

/// User information from usync.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct UserInfo {
    pub jid: Jid,
    pub lid: Option<Jid>,
    pub lid_error: Option<UsyncSubprotocolError>,
    pub status: Option<String>,
    pub status_error: Option<UsyncSubprotocolError>,
    pub picture_id: Option<String>,
    pub picture_error: Option<UsyncSubprotocolError>,
    pub is_business: bool,
    pub business_error: Option<UsyncSubprotocolError>,
    /// Verified business name (decoded from `<business><verified_name>`), if any.
    pub verified_name: Option<VerifiedName>,
    /// Device IDs from the `<devices version="2">` sublist the same usync query
    /// returns (device 0 is the primary). Empty if the server omitted it.
    pub devices: Vec<u16>,
    pub devices_error: Option<UsyncSubprotocolError>,
    /// Meta username, without the display-only `@` prefix. See
    /// [`IsOnWhatsAppResult::username`].
    pub username: Option<CompactString>,
    pub username_error: Option<UsyncSubprotocolError>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IsOnWhatsAppQueryType {
    /// PN query: `<contact/>` + `<lid/>` + `<business><verified_name/></business>`.
    Pn,
    /// LID query: `<lid/>` + `<business><verified_name/></business>` (no contact).
    Lid,
}

/// Check if JIDs are registered on WhatsApp.
///
/// Query protocols differ by type:
/// - PN: `<contact/>`, `<lid/>`, `<business><verified_name/></business>`
/// - LID: `<lid/>`, `<business><verified_name/></business>`
#[derive(Debug, Clone)]
pub struct IsOnWhatsAppSpec {
    pub users: Vec<IsOnWhatsAppUser>,
    pub sid: String,
    pub query_type: IsOnWhatsAppQueryType,
}

impl IsOnWhatsAppSpec {
    pub fn new(
        users: Vec<IsOnWhatsAppUser>,
        sid: impl Into<String>,
        query_type: IsOnWhatsAppQueryType,
    ) -> Self {
        Self {
            users,
            sid: sid.into(),
            query_type,
        }
    }
}

const LEGACY_RESULT_PROTOCOLS: &[UsyncProtocolKind] = &[
    UsyncProtocolKind::Contact,
    UsyncProtocolKind::Lid,
    UsyncProtocolKind::Business,
    UsyncProtocolKind::Status,
    UsyncProtocolKind::Picture,
    UsyncProtocolKind::Devices,
];

fn reject_result_errors(
    response: &UsyncResponse,
    protocols: &[UsyncProtocolKind],
) -> Result<(), anyhow::Error> {
    for state in &response.protocol_states {
        if protocols.contains(&state.protocol)
            && let Some(error) = &state.error
        {
            return Err(anyhow!(usync_subprotocol_error_message(
                state.protocol.as_str(),
                error
            )));
        }
    }
    Ok(())
}

fn warn_result_error(response: &UsyncResponse, protocol: UsyncProtocolKind) {
    if let Some(error) = response
        .protocol_state(protocol)
        .and_then(|state| state.error.as_ref())
    {
        warn!(
            target: "usync",
            "{}; continuing with returned users",
            usync_subprotocol_error_message(protocol.as_str(), error)
        );
    }
}

fn project_lid(user: &UsyncUserResult) -> (Option<Jid>, Option<UsyncSubprotocolError>) {
    match user.protocol(UsyncProtocolKind::Lid) {
        Some(UsyncProtocolResult::Lid(UsyncOutcome::Value(lid))) => (lid.clone(), None),
        Some(UsyncProtocolResult::Lid(UsyncOutcome::Error(error))) => {
            (None, Some((**error).clone()))
        }
        _ => (None, None),
    }
}

fn project_business(
    user: &UsyncUserResult,
) -> (bool, Option<VerifiedName>, Option<UsyncSubprotocolError>) {
    match user.protocol(UsyncProtocolKind::Business) {
        Some(UsyncProtocolResult::Business(UsyncOutcome::Value(business))) => {
            (true, business.verified_name.clone(), None)
        }
        Some(UsyncProtocolResult::Business(UsyncOutcome::Error(error))) => {
            (false, None, Some((**error).clone()))
        }
        _ => (false, None, None),
    }
}

/// Username carried by a successful `<contact>` result, if any.
fn contact_username(user: &UsyncUserResult) -> Option<CompactString> {
    match user.protocol(UsyncProtocolKind::Contact) {
        Some(UsyncProtocolResult::Contact(UsyncOutcome::Value(contact))) => contact
            .username
            .clone()
            .filter(|username| !username.is_empty()),
        _ => None,
    }
}

/// The server publishes a username on two independent channels: the `username`
/// attribute of the `<contact>` result, and the `<username>` subprotocol result.
///
/// A `<username>` *value* is the whole answer, an empty one included: WhatsApp
/// Web reads a present-but-empty node as a deletion (`WAWebHandleUsernameSync`),
/// so letting the contact attribute fill that in would republish a username the
/// server just retracted. A `<username>` *error* is the opposite case, and the
/// distinction is the point: a subprotocol that failed has retracted nothing, so
/// a username the contact result supplied on its own still stands and is
/// reported next to the error rather than in place of it.
fn project_username(
    user: &UsyncUserResult,
) -> (Option<CompactString>, Option<UsyncSubprotocolError>) {
    match user.protocol(UsyncProtocolKind::Username) {
        Some(UsyncProtocolResult::Username(UsyncOutcome::Value(username))) => (
            username.clone().filter(|username| !username.is_empty()),
            None,
        ),
        Some(UsyncProtocolResult::Username(UsyncOutcome::Error(error))) => {
            (contact_username(user), Some((**error).clone()))
        }
        _ => (contact_username(user), None),
    }
}

pub(crate) fn project_lid_mapping(user: &UsyncUserResult) -> Option<UsyncLidMapping> {
    let user_jid = user.id.as_ref()?;
    if user_jid.server.is_lid_family() {
        let pn_jid = user
            .pn_jid
            .as_ref()
            .filter(|jid| jid.server.is_pn_family())?;
        return Some(UsyncLidMapping {
            phone_number: pn_jid.user.clone(),
            lid: user_jid.user.clone(),
        });
    }
    if !user_jid.server.is_pn_family() {
        return None;
    }
    let Some(UsyncProtocolResult::Lid(UsyncOutcome::Value(Some(lid)))) =
        user.protocol(UsyncProtocolKind::Lid)
    else {
        return None;
    };
    Some(UsyncLidMapping {
        phone_number: user_jid.user.clone(),
        lid: lid.user.clone(),
    })
}

#[inline]
fn device_list_identity_matches(
    returned: &Jid,
    expected: &Jid,
    mappings: &[UsyncLidMapping],
) -> bool {
    if returned.user == expected.user && returned.server == expected.server {
        return true;
    }

    mappings.iter().any(|mapping| {
        (returned.server.is_lid_family()
            && expected.server.is_pn_family()
            && returned.user == mapping.lid
            && expected.user == mapping.phone_number)
            || (returned.server.is_pn_family()
                && expected.server.is_lid_family()
                && returned.user == mapping.phone_number
                && expected.user == mapping.lid)
    })
}

fn is_on_whatsapp_query(spec: &IsOnWhatsAppSpec) -> UsyncQuery {
    let mut protocols = Vec::with_capacity(4);
    if spec.query_type == IsOnWhatsAppQueryType::Pn {
        protocols.push(UsyncProtocol::Contact {
            addressing_mode: UsyncAddressingMode::Pn,
        });
    }
    // WhatsApp Web's own existence query (WAWebQueryExistsJob.queryExist) asks
    // for the username subprotocol on both its PN and LID paths, ungated; the
    // `username_search` flag only gates what the UI does with the answer.
    protocols.extend([
        UsyncProtocol::Lid,
        UsyncProtocol::BusinessVerifiedName,
        UsyncProtocol::Username,
    ]);

    let users = spec
        .users
        .iter()
        .map(|user| {
            if user.jid.is_pn() {
                let mut typed = UsyncUser::from_phone(user.jid.user.clone());
                if let Some(lid) = &user.known_lid {
                    typed = typed.with_known_lid(Jid::lid(lid.as_str()));
                }
                typed
            } else {
                UsyncUser::from_jid(user.jid.to_non_ad())
            }
        })
        .collect();

    // Existing specs historically allow an empty list. The public typed API is
    // stricter, but legacy construction keeps its established wire behavior.
    UsyncQuery::from_parts_unchecked(
        UsyncMode::Query,
        UsyncContext::Interactive,
        protocols,
        users,
    )
}

impl IqSpec for IsOnWhatsAppSpec {
    type Response = Vec<IsOnWhatsAppResult>;

    fn build_iq(&self) -> InfoQuery<'static> {
        is_on_whatsapp_query(self).build_info_query(&self.sid)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let response = parse_usync_response(response)?;
        reject_result_errors(&response, LEGACY_RESULT_PROTOCOLS)?;
        // A username the server declines to answer for is not a reason to fail
        // an existence check, which is what callers actually asked for.
        warn_result_error(&response, UsyncProtocolKind::Username);

        let mut results = Vec::with_capacity(response.users.len());
        for user in response.users {
            let Some(jid) = user.id.clone() else {
                continue;
            };
            let (is_registered, contact_error) = match user.protocol(UsyncProtocolKind::Contact) {
                Some(UsyncProtocolResult::Contact(UsyncOutcome::Value(contact))) => {
                    (contact.contact_type == CONTACT_TYPE_IN, None)
                }
                Some(UsyncProtocolResult::Contact(UsyncOutcome::Error(error))) => {
                    (false, Some((**error).clone()))
                }
                _ => (jid.is_lid(), None),
            };
            let (lid, lid_error) = project_lid(&user);
            let (is_business, verified_name, business_error) = project_business(&user);
            let (username, username_error) = project_username(&user);

            results.push(IsOnWhatsAppResult {
                jid,
                lid,
                pn_jid: user.pn_jid,
                is_registered,
                contact_error,
                lid_error,
                is_business,
                business_error,
                verified_name,
                username,
                username_error,
            });
        }

        Ok(results)
    }
}

/// `<contact type>` for a user that is reachable on WhatsApp.
const CONTACT_TYPE_IN: &str = "in";
/// `<contact type>` for a user that is not.
const CONTACT_TYPE_OUT: &str = "out";

/// Shortest username the official client will send to the server
/// (`WAWebUsernameConstants.USERNAME_MIN_LENGTH`).
pub const USERNAME_MIN_LENGTH: usize = 3;
/// Longest username the official client will send to the server
/// (`WAWebUsernameConstants.USERNAME_MAX_LENGTH`).
pub const USERNAME_MAX_LENGTH: usize = 35;

/// Why a username lookup could not be built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum UsernameLookupError {
    #[error(
        "username length {length} is outside the {USERNAME_MIN_LENGTH}..={USERNAME_MAX_LENGTH} the server accepts"
    )]
    InvalidLength { length: usize },
    #[error("the username query is not a valid usync query: {0}")]
    Query(#[from] UsyncValidationError),
}

/// What the server said about a username.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum UsernameLookup {
    /// No account answers to this username, or it is not reachable from here.
    /// This is the `<contact type="out">` case, and also an empty result list.
    NotFound,
    /// The username exists but the server withheld the identity behind it: the
    /// lookup has to be repeated with the account's username key.
    KeyRequired {
        /// Username as the server spelled it back, when it did.
        username: Option<CompactString>,
    },
    /// The username resolved to an account.
    Found(UsernameLookupUser),
}

/// The account a username resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UsernameLookupUser {
    /// Identity the server returned. The query addresses contacts by LID, so
    /// this is normally a LID.
    pub jid: Jid,
    /// Phone-number JID, when the server disclosed one on `<business>`.
    pub pn_jid: Option<Jid>,
    /// Username as the server spelled it back.
    pub username: Option<CompactString>,
    pub is_business: bool,
    /// Verified business name (decoded from `<business><verified_name>`), if any.
    pub verified_name: Option<VerifiedName>,
}

/// Look an account up by its Meta username.
///
/// **Experimental.** The request is built exactly as WhatsApp Web's
/// `WAWebQueryExistsJob.queryUsernameExists` builds it, but no capture of a
/// server answering it backs this: only the request shape is verified. Treat a
/// failure as "unsupported for this account" rather than as a bug.
///
/// An account may protect its username with a key (the 4-digit "username key"
/// WhatsApp Web calls a pin). Without it the server answers
/// [`UsernameLookup::KeyRequired`] instead of an identity.
#[derive(Debug, Clone)]
pub struct UsernameLookupSpec {
    query: UsyncQuery,
    sid: String,
}

impl UsernameLookupSpec {
    /// `username` is the bare handle, with no `@` prefix.
    pub fn new(
        username: &str,
        username_key: Option<&str>,
        sid: impl Into<String>,
    ) -> Result<Self, UsernameLookupError> {
        let length = username.chars().count();
        if !(USERNAME_MIN_LENGTH..=USERNAME_MAX_LENGTH).contains(&length) {
            return Err(UsernameLookupError::InvalidLength { length });
        }
        let mut user = UsyncUser::from_username(username);
        if let Some(key) = username_key {
            user = user.with_username_pin(key);
        }
        // Contact addressing is LID here because the query has no phone number
        // to address by, and `<business pn_jid>` is what discloses the PN.
        let query = UsyncQuery::new(
            UsyncMode::Query,
            UsyncContext::Interactive,
            vec![
                UsyncProtocol::Contact {
                    addressing_mode: UsyncAddressingMode::Lid,
                },
                UsyncProtocol::BusinessVerifiedName,
            ],
            vec![user],
        )?;
        Ok(Self {
            query,
            sid: sid.into(),
        })
    }
}

impl IqSpec for UsernameLookupSpec {
    type Response = UsernameLookup;

    fn build_iq(&self) -> InfoQuery<'static> {
        self.query.build_info_query(&self.sid)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let response = parse_usync_response(response)?;
        reject_result_errors(&response, LEGACY_RESULT_PROTOCOLS)?;

        let Some(user) = response.users.into_iter().next() else {
            return Ok(UsernameLookup::NotFound);
        };
        // The contact result is the whole answer to a username lookup, so
        // anything short of a successful one must not read as a resolved or
        // key-gated account: `find_by_username` would persist a LID/PN pair off
        // it. WhatsApp Web reads `contact.type` unconditionally here, which says
        // it never expects the node to be missing either.
        let contact = match user.protocol(UsyncProtocolKind::Contact) {
            Some(UsyncProtocolResult::Contact(UsyncOutcome::Value(contact))) => contact,
            Some(UsyncProtocolResult::Contact(UsyncOutcome::Error(error))) => {
                return Err(anyhow!(usync_subprotocol_error_message(
                    UsyncProtocolKind::Contact.as_str(),
                    error
                )));
            }
            _ => return Err(anyhow!("username lookup answered without a contact result")),
        };
        // WA Web only rejects "out" here, while the PN path of the same job
        // requires "in". Take the stricter reading: a type this parser cannot
        // name is not a confirmation, and a false `Found` writes an identity
        // mapping to the LID/PN cache.
        if contact.contact_type == CONTACT_TYPE_OUT {
            return Ok(UsernameLookup::NotFound);
        }
        if contact.contact_type != CONTACT_TYPE_IN {
            return Err(anyhow!(
                "username lookup answered with an unknown contact type: {}",
                contact.contact_type
            ));
        }
        let username = contact
            .username
            .clone()
            .filter(|username| !username.is_empty());
        let (is_business, verified_name, _) = project_business(&user);
        let pn_jid = match user.protocol(UsyncProtocolKind::Business) {
            Some(UsyncProtocolResult::Business(UsyncOutcome::Value(business))) => {
                business.pn_jid.clone()
            }
            _ => None,
        };

        let Some(jid) = user.id else {
            return Ok(UsernameLookup::KeyRequired { username });
        };
        Ok(UsernameLookup::Found(UsernameLookupUser {
            jid,
            pn_jid,
            username,
            is_business,
            verified_name,
        }))
    }
}

/// Get user information by JID.
#[derive(Debug, Clone)]
pub struct UserInfoSpec {
    pub jids: Vec<Jid>,
    pub sid: String,
    /// Per-user trusted-contact token, keyed by the query JID's non-ad string
    /// form (domain-qualified, so PN and LID forms don't collide), attached to
    /// the matching `<user>` node for privacy-gated subprotocols (status/about),
    /// matching WA Web's `USyncStatusProtocol.getUserElement` /
    /// `USyncUser.withTcToken`.
    pub tc_tokens: HashMap<String, Vec<u8>>,
}

impl UserInfoSpec {
    pub fn new(jids: Vec<Jid>, sid: impl Into<String>) -> Self {
        Self {
            jids,
            sid: sid.into(),
            tc_tokens: HashMap::new(),
        }
    }

    /// Attach per-user trusted-contact tokens keyed by the query JID's non-ad
    /// string form (see [`UserInfoSpec::tc_tokens`]).
    pub fn with_tc_tokens(mut self, tc_tokens: HashMap<String, Vec<u8>>) -> Self {
        self.tc_tokens = tc_tokens;
        self
    }
}

fn user_info_query(spec: &UserInfoSpec) -> UsyncQuery {
    let users = spec
        .jids
        .iter()
        .map(|jid| {
            let bare = jid.to_non_ad();
            let key = bare.to_string();
            let mut user = UsyncUser::from_jid(bare);
            if let Some(token) = spec.tc_tokens.get(&key) {
                user = user.with_tc_token(token.clone());
            }
            user
        })
        .collect();
    UsyncQuery::from_parts_unchecked(
        UsyncMode::Full,
        UsyncContext::Background,
        vec![
            UsyncProtocol::BusinessVerifiedName,
            UsyncProtocol::Status,
            UsyncProtocol::Picture,
            UsyncProtocol::DevicesV2,
            UsyncProtocol::Lid,
            UsyncProtocol::Username,
        ],
        users,
    )
}

impl IqSpec for UserInfoSpec {
    type Response = HashMap<Jid, UserInfo>;

    fn build_iq(&self) -> InfoQuery<'static> {
        user_info_query(self).build_info_query(&self.sid)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let response = parse_usync_response(response)?;
        reject_result_errors(&response, LEGACY_RESULT_PROTOCOLS)?;
        warn_result_error(&response, UsyncProtocolKind::Username);

        let mut results = HashMap::with_capacity(response.users.len());
        for user in response.users {
            let Some(jid) = user.id.clone() else {
                continue;
            };
            let (lid, lid_error) = project_lid(&user);
            let (status, status_error) = match user.protocol(UsyncProtocolKind::Status) {
                Some(UsyncProtocolResult::Status(UsyncOutcome::Value(status))) => {
                    (status.status.as_ref().map(ToString::to_string), None)
                }
                Some(UsyncProtocolResult::Status(UsyncOutcome::Error(error))) => {
                    (None, Some((**error).clone()))
                }
                _ => (None, None),
            };
            let (picture_id, picture_error) = match user.protocol(UsyncProtocolKind::Picture) {
                Some(UsyncProtocolResult::Picture(UsyncOutcome::Value(id))) => {
                    (Some(id.to_string()), None)
                }
                Some(UsyncProtocolResult::Picture(UsyncOutcome::Error(error))) => {
                    (None, Some((**error).clone()))
                }
                _ => (None, None),
            };
            let (is_business, verified_name, business_error) = project_business(&user);
            let (username, username_error) = project_username(&user);
            let (devices, devices_error) = match user.protocol(UsyncProtocolKind::Devices) {
                Some(UsyncProtocolResult::Devices(UsyncOutcome::Value(devices))) => (
                    devices
                        .device_list
                        .as_ref()
                        .map(|list| list.devices.iter().map(|device| device.id).collect())
                        .unwrap_or_default(),
                    None,
                ),
                Some(UsyncProtocolResult::Devices(UsyncOutcome::Error(error))) => {
                    (Vec::new(), Some((**error).clone()))
                }
                _ => (Vec::new(), None),
            };

            results.insert(
                jid.clone(),
                UserInfo {
                    jid,
                    lid,
                    lid_error,
                    status,
                    status_error,
                    picture_id,
                    picture_error,
                    is_business,
                    business_error,
                    verified_name,
                    devices,
                    devices_error,
                    username,
                    username_error,
                },
            );
        }
        Ok(results)
    }
}

// Re-export types from wacore::usync for convenience
pub use crate::usync::{UserDeviceList, UsyncLidMapping};

/// Response from device list query containing device lists and any LID mappings.
#[derive(Debug, Clone)]
pub struct DeviceListResponse {
    pub device_lists: Vec<UserDeviceList>,
    pub lid_mappings: Vec<UsyncLidMapping>,
}

/// Get device list for JIDs.
///
/// ## Wire Format
/// ```xml
/// <!-- Request -->
/// <iq xmlns="usync" type="get" to="s.whatsapp.net" id="...">
///   <usync sid="..." mode="query" last="true" index="0" context="message">
///     <query>
///       <devices version="2"/>
///     </query>
///     <list>
///       <user jid="1234567890@s.whatsapp.net"/>
///     </list>
///   </usync>
/// </iq>
///
/// <!-- Response -->
/// <iq from="s.whatsapp.net" id="..." type="result">
///   <usync>
///     <list>
///       <user jid="1234567890@s.whatsapp.net">
///         <devices>
///           <device-list hash="2:abcdef123456">
///             <device id="0"/>
///             <device id="1"/>
///           </device-list>
///         </devices>
///       </user>
///     </list>
///   </usync>
/// </iq>
/// ```
#[derive(Debug, Clone)]
pub struct DeviceListSpec {
    pub jids: Vec<Jid>,
    pub sid: String,
    /// Optional per-user device-list hint `(device_hash, ts)`, keyed by the bare
    /// (`to_non_ad`) jid. When present, the query emits
    /// `<user jid="..."><devices device_hash="2:.." ts="N"/></user>` so the server
    /// returns only CHANGED users (WA Web `syncDeviceList`). Users the server omits
    /// from the response are UNCHANGED and their cached devices must be preserved.
    pub hashes: HashMap<Jid, (String, i64)>,
}

impl DeviceListSpec {
    pub fn new(jids: Vec<Jid>, sid: impl Into<String>) -> Self {
        Self {
            jids,
            sid: sid.into(),
            hashes: HashMap::new(),
        }
    }

    /// Like [`new`](Self::new) but carries per-user `device_hash`/`ts` hints so the
    /// server can skip unchanged users. Keys are bare (`to_non_ad`) jids.
    pub fn with_hashes(
        jids: Vec<Jid>,
        sid: impl Into<String>,
        hashes: HashMap<Jid, (String, i64)>,
    ) -> Self {
        Self {
            jids,
            sid: sid.into(),
            hashes,
        }
    }

    /// Reject a response that omits any requested user or returns no devices
    /// for one. Use for authoritative refreshes; ordinary fanout remains
    /// best-effort when an individual user cannot be resolved.
    pub fn require_complete_response(self) -> CompleteDeviceListSpec {
        CompleteDeviceListSpec(self)
    }
}

/// A device-list query whose response must contain a usable entry for every
/// requested user.
///
/// This wrapper keeps [`DeviceListSpec`]'s public data model unchanged while
/// making authoritative refresh semantics explicit in the type system. It has
/// no additional runtime state.
#[derive(Debug, Clone)]
pub struct CompleteDeviceListSpec(DeviceListSpec);

pub(crate) fn device_list_query(
    jids: &[Jid],
    hashes: Option<&HashMap<Jid, (String, i64)>>,
) -> UsyncQuery {
    let users = jids
        .iter()
        .map(|jid| {
            let bare = jid.to_non_ad();
            let mut user = UsyncUser::from_jid(bare.clone());
            if let Some((device_hash, timestamp)) = hashes.and_then(|hashes| hashes.get(&bare)) {
                user = user.with_device_sync(
                    UsyncDeviceSyncHint::new()
                        .with_device_hash(device_hash.as_str())
                        .with_timestamp(*timestamp),
                );
            }
            user
        })
        .collect();
    UsyncQuery::from_parts_unchecked(
        UsyncMode::Query,
        UsyncContext::Message,
        vec![UsyncProtocol::DevicesV2],
        users,
    )
}

pub(crate) fn project_device_list_response(
    response: UsyncResponse,
) -> Result<DeviceListResponse, anyhow::Error> {
    const NON_DEVICE_PROTOCOLS: &[UsyncProtocolKind] = &[
        UsyncProtocolKind::Contact,
        UsyncProtocolKind::Lid,
        UsyncProtocolKind::Business,
        UsyncProtocolKind::Status,
        UsyncProtocolKind::Picture,
    ];

    reject_result_errors(&response, NON_DEVICE_PROTOCOLS)?;
    warn_result_error(&response, UsyncProtocolKind::Devices);

    let mut device_lists = Vec::with_capacity(response.users.len());
    let mut lid_mappings = Vec::new();
    for user in response.users {
        if let Some(mapping) = project_lid_mapping(&user) {
            lid_mappings.push(mapping);
        }
        let Some(user_jid) = user.id else {
            continue;
        };

        let devices = user
            .protocols
            .into_iter()
            .find(|result| result.kind() == UsyncProtocolKind::Devices);
        let devices = match devices {
            Some(UsyncProtocolResult::Devices(UsyncOutcome::Value(devices))) => devices,
            Some(UsyncProtocolResult::Devices(UsyncOutcome::Error(error))) => {
                warn!(
                    target: "usync",
                    "{} for {user_jid}; skipping user",
                    usync_subprotocol_error_message("devices", &error)
                );
                continue;
            }
            _ => {
                warn!(target: "usync", "<device-list> not found for user {user_jid}, skipping");
                continue;
            }
        };
        let Some(device_list) = devices.device_list else {
            warn!(target: "usync", "<device-list> not found for user {user_jid}, skipping");
            continue;
        };
        let key_index_bytes = devices
            .key_index
            .and_then(|key_index| key_index.signed_key_index_bytes)
            .filter(|bytes| !bytes.is_empty());
        let mut parsed_devices = device_list
            .devices
            .into_iter()
            .map(|device| {
                crate::usync::UsyncDevice::new(device.id, device.key_index)
                    .with_hosting(device.is_hosted)
            })
            .collect::<Vec<_>>();

        // Companions we cannot validate are dropped; the identity is not. WA
        // Web's `handleOmittedResult` collapses such a record to the primary
        // (`devices = [{id: DEFAULT_DEVICE_ID, keyIndex: 0}]`) and its fan-out
        // falls back to the bare user for any wid it has no list for. Losing
        // the primary here takes the user out of every group send's sender-key
        // targets and out of the phash, where nothing downstream can see it.
        let mut phash = device_list.hash.map(|hash| hash.to_string());
        let has_companion = parsed_devices.iter().any(|device| device.device != 0);
        if has_companion && key_index_bytes.is_none() {
            parsed_devices.retain(|device| device.device == 0);
            if parsed_devices.is_empty() {
                warn!(
                    target: "usync",
                    "User {user_jid} has no signedKeyIndexBytes and no primary device, skipping"
                );
                continue;
            }
            warn!(
                target: "usync",
                "User {user_jid} has companion devices but no signedKeyIndexBytes; keeping only the primary"
            );
            // The server's hash covers the devices it sent, not the ones kept.
            // Storing it would make the next query claim this truncated list is
            // current and let the server omit the user, stranding the dropped
            // companions.
            phash = None;
        }

        device_lists.push(UserDeviceList {
            user: user_jid,
            devices: parsed_devices,
            phash,
            key_index_bytes,
        });
    }

    Ok(DeviceListResponse {
        device_lists,
        lid_mappings,
    })
}

impl IqSpec for DeviceListSpec {
    type Response = DeviceListResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        device_list_query(&self.jids, Some(&self.hashes)).build_info_query(&self.sid)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        project_device_list_response(parse_usync_response(response)?)
    }
}

impl IqSpec for CompleteDeviceListSpec {
    type Response = DeviceListResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        self.0.build_iq()
    }

    fn encode_iq_direct(&self, request_id: &str, out: &mut Vec<u8>) -> Result<bool, anyhow::Error> {
        self.0.encode_iq_direct(request_id, out)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let parsed = self.0.parse_response(response)?;
        for expected in &self.0.jids {
            let Some(returned) = parsed.device_lists.iter().find(|returned| {
                device_list_identity_matches(&returned.user, expected, &parsed.lid_mappings)
            }) else {
                anyhow::bail!("device-list response omitted user {expected}");
            };
            if returned.devices.is_empty() {
                anyhow::bail!("device-list response returned no devices for {expected}");
            }
        }
        Ok(parsed)
    }
}

/// Resolve PN→LID mappings for JIDs without a known LID.
/// Matches WA Web's `ensurePhoneNumberToLidMapping` (PhoneNumberMappingJob.js).
/// Uses a separate usync with only `<lid/>` in the query to avoid side effects
/// on device registries or sender key state.
#[derive(Debug, Clone)]
pub struct LidQuerySpec {
    pub jids: Vec<Jid>,
    pub sid: String,
}

impl LidQuerySpec {
    pub fn new(jids: Vec<Jid>, sid: impl Into<String>) -> Self {
        Self {
            jids,
            sid: sid.into(),
        }
    }
}

fn lid_query(spec: &LidQuerySpec) -> UsyncQuery {
    let users = spec
        .jids
        .iter()
        .map(|jid| UsyncUser::from_jid(jid.to_non_ad()))
        .collect();
    UsyncQuery::from_parts_unchecked(
        UsyncMode::Query,
        UsyncContext::Background,
        vec![UsyncProtocol::Lid],
        users,
    )
}

/// Response: just the LID mappings learned.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LidQueryResponse {
    pub lid_mappings: Vec<UsyncLidMapping>,
}

impl IqSpec for LidQuerySpec {
    type Response = LidQueryResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        lid_query(self).build_info_query(&self.sid)
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let response = parse_usync_response(response)?;
        reject_result_errors(&response, LEGACY_RESULT_PROTOCOLS)?;

        let mut lid_mappings = Vec::with_capacity(response.users.len());
        for user in response.users {
            let Some(user_jid) = user.id.as_ref() else {
                continue;
            };
            match user.protocol(UsyncProtocolKind::Lid) {
                Some(UsyncProtocolResult::Lid(UsyncOutcome::Error(error))) => {
                    warn!(
                        target: "usync",
                        "{} for {user_jid}; skipping user",
                        usync_subprotocol_error_message("lid", error)
                    );
                }
                _ => {
                    if let Some(mapping) = project_lid_mapping(&user) {
                        lid_mappings.push(mapping);
                    }
                }
            }
        }
        Ok(LidQueryResponse { lid_mappings })
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use wacore_binary::Server;

    /// Build a dummy key-index-list node for device IDs (used in test fixtures)
    fn build_test_key_index_list_node(device_ids: &[u16]) -> Node {
        use buffa::Message;
        let valid_indexes: Vec<u32> = device_ids.iter().map(|&id| id as u32).collect();
        let key_index = waproto::whatsapp::ADVKeyIndexList {
            raw_id: Some(1),
            timestamp: Some(1000),
            current_index: Some(valid_indexes.iter().copied().max().unwrap_or(0)),
            valid_indexes,
            ..Default::default()
        };
        let signed = waproto::whatsapp::ADVSignedKeyIndexList {
            details: Some(key_index.encode_to_vec()),
            ..Default::default()
        };
        NodeBuilder::new("key-index-list")
            .attr("ts", "1000")
            .bytes(signed.encode_to_vec())
            .build()
    }

    #[test]
    fn test_usync_mode() {
        assert_eq!(UsyncMode::Query.as_str(), "query");
        assert_eq!(UsyncMode::Full.as_str(), "full");
    }

    #[test]
    fn test_usync_context() {
        assert_eq!(UsyncContext::Interactive.as_str(), "interactive");
        assert_eq!(UsyncContext::Background.as_str(), "background");
        assert_eq!(UsyncContext::Message.as_str(), "message");
    }

    fn pn_user(phone: &str) -> IsOnWhatsAppUser {
        IsOnWhatsAppUser {
            jid: Jid::pn(phone),
            known_lid: None,
        }
    }

    #[test]
    fn test_is_on_whatsapp_spec_build_iq() {
        let spec = IsOnWhatsAppSpec::new(
            vec![pn_user("1234567890")],
            "test-sid",
            IsOnWhatsAppQueryType::Pn,
        );
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "usync");

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes.len(), 1);
            let usync = &nodes[0];
            assert_eq!(usync.tag, "usync");
            assert!(usync.attrs.get("sid").is_some_and(|s| s == "test-sid"));
            assert!(usync.attrs.get("mode").is_some_and(|s| s == "query"));
            assert!(
                usync
                    .attrs
                    .get("context")
                    .is_some_and(|s| s == "interactive")
            );

            let query = usync.get_optional_child("query").unwrap();
            assert!(query.get_optional_child("contact").is_some());
            assert!(query.get_optional_child("lid").is_some());
            assert!(query.get_optional_child("business").is_some());

            let contact = usync
                .get_optional_child("list")
                .and_then(|list| list.get_optional_child("user"))
                .and_then(|user| user.get_optional_child("contact"))
                .unwrap();
            assert_eq!(contact.content_as_string().as_deref(), Some("+1234567890"));
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_is_on_whatsapp_spec_build_iq_lid() {
        let spec = IsOnWhatsAppSpec::new(
            vec![IsOnWhatsAppUser {
                jid: Jid::lid("100000001"),
                known_lid: None,
            }],
            "test-sid",
            IsOnWhatsAppQueryType::Lid,
        );
        let iq = spec.build_iq();

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            let usync = &nodes[0];
            let query = usync.get_optional_child("query").unwrap();
            assert!(query.get_optional_child("contact").is_none());
            assert!(query.get_optional_child("lid").is_some());
            assert!(query.get_optional_child("business").is_some());

            let list = usync.get_optional_child("list").unwrap();
            let user = list.get_children_by_tag("user").next().unwrap();
            assert!(user.attrs.get("jid").is_some_and(|s| s == "100000001@lid"));
            assert!(user.get_optional_child("contact").is_none());
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_is_on_whatsapp_spec_build_iq_with_known_lid() {
        let spec = IsOnWhatsAppSpec::new(
            vec![IsOnWhatsAppUser {
                jid: Jid::pn("1234567890"),
                known_lid: Some("100000001".into()),
            }],
            "sid",
            IsOnWhatsAppQueryType::Pn,
        );
        let iq = spec.build_iq();

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            let list = nodes[0].get_optional_child("list").unwrap();
            let user = list.get_children_by_tag("user").next().unwrap();
            let lid_child = user.get_optional_child("lid").unwrap();
            assert!(
                lid_child
                    .attrs
                    .get("jid")
                    .is_some_and(|s| s == "100000001@lid")
            );
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_is_on_whatsapp_spec_parse_response() {
        let spec = IsOnWhatsAppSpec::new(
            vec![pn_user("1234567890")],
            "test-sid",
            IsOnWhatsAppQueryType::Pn,
        );

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([
                            NodeBuilder::new("contact").attr("type", "in").build(),
                            NodeBuilder::new("lid").attr("val", "100000001@lid").build(),
                            NodeBuilder::new("business").build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].jid.user, "1234567890");
        assert!(results[0].is_registered);
        assert!(results[0].is_business);
        assert!(results[0].lid.is_some());
        assert_eq!(results[0].lid.as_ref().unwrap().user, "100000001");
    }

    #[test]
    fn test_is_on_whatsapp_spec_parse_not_registered() {
        let spec = IsOnWhatsAppSpec::new(
            vec![pn_user("1234567890")],
            "test-sid",
            IsOnWhatsAppQueryType::Pn,
        );

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([NodeBuilder::new("contact").attr("type", "out").build()])
                        .build()])
                    .build()])
                .build()])
            .build();

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_registered);
        assert!(!results[0].is_business);
        assert!(results[0].lid.is_none());
    }

    #[test]
    fn test_is_on_whatsapp_spec_parse_pn_jid() {
        let spec = IsOnWhatsAppSpec::new(
            vec![IsOnWhatsAppUser {
                jid: Jid::lid("100000001"),
                known_lid: None,
            }],
            "test-sid",
            IsOnWhatsAppQueryType::Lid,
        );

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "100000001@lid")
                        .attr("pn_jid", "1234567890@s.whatsapp.net")
                        .build()])
                    .build()])
                .build()])
            .build();

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].jid.user, "100000001");
        assert!(results[0].jid.is_lid());
        // LID query with no contact node: presence implies registration
        assert!(results[0].is_registered);
        assert_eq!(results[0].pn_jid.as_ref().unwrap().user, "1234567890");
    }

    #[test]
    fn is_on_whatsapp_preserves_user_subprotocol_errors() {
        let spec = IsOnWhatsAppSpec::new(
            vec![pn_user("1234567890")],
            "test-sid",
            IsOnWhatsAppQueryType::Pn,
        );

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([
                            NodeBuilder::new("contact")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "403")
                                    .attr("text", "blocked")
                                    .build()])
                                .build(),
                            NodeBuilder::new("lid")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "404")
                                    .attr("text", "missing")
                                    .build()])
                                .build(),
                            NodeBuilder::new("business")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "500")
                                    .attr("text", "server")
                                    .build()])
                                .build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_registered);
        assert_eq!(results[0].contact_error.as_ref().unwrap().code, Some(403));
        assert!(results[0].lid.is_none());
        assert_eq!(results[0].lid_error.as_ref().unwrap().code, Some(404));
        assert!(!results[0].is_business);
        assert_eq!(results[0].business_error.as_ref().unwrap().code, Some(500));
    }

    #[test]
    fn test_user_info_spec_build_iq() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = UserInfoSpec::new(vec![jid], "test-sid");
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "usync");

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            let usync = &nodes[0];
            assert!(usync.attrs.get("mode").is_some_and(|s| s == "full"));
            assert!(
                usync
                    .attrs
                    .get("context")
                    .is_some_and(|s| s == "background")
            );
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_user_info_spec_parse_response() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = UserInfoSpec::new(vec![jid.clone()], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([
                            NodeBuilder::new("lid").attr("val", "100000001@lid").build(),
                            NodeBuilder::new("status")
                                .string_content("Hello World")
                                .build(),
                            NodeBuilder::new("picture").attr("id", "123456789").build(),
                            NodeBuilder::new("business").build(),
                            NodeBuilder::new("devices")
                                .attr("version", "2")
                                .children([NodeBuilder::new("device-list")
                                    .children([
                                        NodeBuilder::new("device").attr("id", "0").build(),
                                        NodeBuilder::new("device").attr("id", "1").build(),
                                    ])
                                    .build()])
                                .build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results.len(), 1);
        let info = results.get(&jid).unwrap();
        assert_eq!(info.jid.user, "1234567890");
        assert!(info.is_business);
        assert_eq!(info.status, Some("Hello World".to_string()));
        assert_eq!(info.picture_id, Some("123456789".to_string()));
        assert!(info.lid.is_some());
        // The <devices> sublist the same query returns is now surfaced, not dropped.
        assert_eq!(info.devices, vec![0, 1]);
    }

    #[test]
    fn user_info_preserves_subprotocol_errors() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = UserInfoSpec::new(vec![jid.clone()], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([
                            NodeBuilder::new("lid")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "409")
                                    .attr("text", "lid-conflict")
                                    .build()])
                                .build(),
                            NodeBuilder::new("status")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "401")
                                    .attr("text", "privacy")
                                    .build()])
                                .build(),
                            NodeBuilder::new("picture")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "404")
                                    .attr("text", "missing")
                                    .build()])
                                .build(),
                            NodeBuilder::new("devices")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "500")
                                    .attr("text", "server")
                                    .build()])
                                .build(),
                            NodeBuilder::new("business")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "406")
                                    .attr("text", "business-error")
                                    .build()])
                                .build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        let info = results.get(&jid).unwrap();

        assert!(info.lid.is_none());
        assert_eq!(info.lid_error.as_ref().unwrap().code, Some(409));
        assert!(info.status.is_none());
        assert_eq!(info.status_error.as_ref().unwrap().code, Some(401));
        assert_eq!(
            info.status_error.as_ref().unwrap().text.as_deref(),
            Some("privacy")
        );
        assert!(info.picture_id.is_none());
        assert_eq!(info.picture_error.as_ref().unwrap().code, Some(404));
        assert!(info.devices.is_empty());
        assert_eq!(info.devices_error.as_ref().unwrap().code, Some(500));
        assert!(!info.is_business);
        assert_eq!(info.business_error.as_ref().unwrap().code, Some(406));
    }

    #[test]
    fn user_info_attaches_per_user_tctoken() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let mut tokens = HashMap::new();
        tokens.insert(jid.to_non_ad().to_string(), vec![0xDE, 0xAD]);
        let spec = UserInfoSpec::new(vec![jid], "sid").with_tc_tokens(tokens);

        let iq = spec.build_iq();
        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("expected usync nodes");
        };
        let list = nodes[0].get_children_by_tag("list").next().unwrap();
        let user = list.get_children_by_tag("user").next().unwrap();
        let tctoken = user
            .get_children_by_tag("tctoken")
            .next()
            .expect("user node should carry a tctoken");
        match &tctoken.content {
            Some(NodeContent::Bytes(b)) => assert_eq!(b, &[0xDE, 0xAD]),
            _ => panic!("tctoken should carry bytes"),
        }
    }

    #[test]
    fn user_info_without_tctoken_omits_it() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let iq = UserInfoSpec::new(vec![jid], "sid").build_iq();
        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("expected usync nodes");
        };
        let list = nodes[0].get_children_by_tag("list").next().unwrap();
        let user = list.get_children_by_tag("user").next().unwrap();
        assert!(user.get_children_by_tag("tctoken").next().is_none());
    }

    #[test]
    fn user_info_result_subprotocol_error_is_rejected() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = UserInfoSpec::new(vec![jid], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([
                    NodeBuilder::new("result")
                        .children([NodeBuilder::new("status")
                            .children([NodeBuilder::new("error")
                                .attr("code", "403")
                                .attr("text", "blocked")
                                .build()])
                            .build()])
                        .build(),
                    NodeBuilder::new("list").build(),
                ])
                .build()])
            .build();

        let err = spec.parse_response(&response.as_node_ref()).unwrap_err();
        assert!(err.to_string().contains("usync status error 403: blocked"));
    }

    #[test]
    fn subprotocol_error_message_omits_empty_text_separator() {
        let error = |text| UsyncSubprotocolError {
            code: Some(401),
            text,
            backoff: None,
        };

        assert_eq!(
            usync_subprotocol_error_message("status", &error(None)),
            "usync status error 401"
        );
        assert_eq!(
            usync_subprotocol_error_message("status", &error(Some(String::new()))),
            "usync status error 401"
        );
        assert_eq!(
            usync_subprotocol_error_message("status", &error(Some("blocked".to_owned()))),
            "usync status error 401: blocked"
        );
    }

    #[test]
    fn test_pn_user_phone_formatting() {
        // PN JIDs always have the user part without +, build_user_nodes adds +
        let spec = IsOnWhatsAppSpec::new(
            vec![pn_user("1234567890")],
            "sid",
            IsOnWhatsAppQueryType::Pn,
        );
        let iq = spec.build_iq();

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            let list = nodes[0].get_optional_child("list").unwrap();
            let user = list.get_children_by_tag("user").next().unwrap();
            let contact = user.get_optional_child("contact").unwrap();

            match &contact.content {
                Some(NodeContent::String(s)) => assert_eq!(s, "+1234567890"),
                _ => panic!("Expected string content"),
            }
            // PN user nodes should NOT have a jid attribute
            assert!(user.attrs.get("jid").is_none());
        }
    }

    #[test]
    fn test_device_list_spec_build_iq() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid], "test-sid");
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "usync");

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            let usync = &nodes[0];
            assert!(usync.attrs.get("sid").is_some_and(|s| s == "test-sid"));
            assert!(usync.attrs.get("mode").is_some_and(|s| s == "query"));
            assert!(usync.attrs.get("context").is_some_and(|s| s == "message"));

            let query = usync.get_optional_child("query").unwrap();
            let devices = query.get_optional_child("devices").unwrap();
            assert!(devices.attrs.get("version").is_some_and(|s| s == "2"));

            // Without a hint, the <user> node is bare (no per-user <devices>).
            let user = usync
                .get_optional_child("list")
                .unwrap()
                .get_optional_child("user")
                .unwrap();
            assert!(user.get_optional_child("devices").is_none());
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn device_list_spec_preserves_its_public_data_shape() {
        let jid = Jid::pn("1234567890");
        let spec = DeviceListSpec {
            jids: vec![jid.clone()],
            sid: "test-sid".to_string(),
            hashes: HashMap::new(),
        };

        let DeviceListSpec { jids, sid, hashes } = spec;
        assert_eq!(jids, [jid]);
        assert_eq!(sid, "test-sid");
        assert!(hashes.is_empty());
    }

    #[test]
    fn test_device_list_spec_build_iq_with_device_hash() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let mut hashes = HashMap::new();
        hashes.insert(jid.clone(), ("2:cachedhash".to_string(), 1_700_000_000i64));
        let spec = DeviceListSpec::with_hashes(vec![jid], "sid-h", hashes);
        let iq = spec.build_iq();

        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("Expected NodeContent::Nodes");
        };
        let usync = &nodes[0];
        // Query-level <devices version="2"> still declares the protocol.
        let query = usync.get_optional_child("query").unwrap();
        assert!(
            query
                .get_optional_child("devices")
                .unwrap()
                .attrs
                .get("version")
                .is_some_and(|s| s == "2")
        );
        // Per-user <devices device_hash ts> carries the cached hash.
        let user = usync
            .get_optional_child("list")
            .unwrap()
            .get_optional_child("user")
            .unwrap();
        let dev = user.get_optional_child("devices").unwrap();
        assert!(
            dev.attrs
                .get("device_hash")
                .is_some_and(|s| s == "2:cachedhash")
        );
        assert!(dev.attrs.get("ts").is_some_and(|s| s == "1700000000"));
    }

    #[test]
    fn test_device_list_spec_parse_omits_unchanged_user() {
        // Queried two users; the server returns only one (the other unchanged →
        // omitted). The parser must yield only the present user so the caller
        // keeps the omitted user's cached devices (device_hash merge-safety).
        let a: Jid = "1111111111@s.whatsapp.net".parse().unwrap();
        let b: Jid = "2222222222@s.whatsapp.net".parse().unwrap();
        let best_effort = DeviceListSpec::new(vec![a.clone(), b.clone()], "sid-best-effort");
        let complete = DeviceListSpec::new(vec![a.clone(), b.clone()], "sid-complete")
            .require_complete_response();
        let incremental = DeviceListSpec::with_hashes(vec![a, b], "sid-omit", HashMap::new());

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1111111111@s.whatsapp.net")
                        .children([NodeBuilder::new("devices")
                            .children([NodeBuilder::new("device-list")
                                .attr("hash", "2:hashA")
                                .children([NodeBuilder::new("device").attr("id", "0").build()])
                                .build()])
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();

        assert!(
            complete.parse_response(&response.as_node_ref()).is_err(),
            "a complete query must not accept a partial snapshot"
        );
        for spec in [best_effort, incremental] {
            let result = spec.parse_response(&response.as_node_ref()).unwrap();
            assert_eq!(result.device_lists.len(), 1, "omitted user must not appear");
            assert_eq!(result.device_lists[0].user.user, "1111111111");
        }
    }

    #[test]
    fn test_device_list_spec_parse_response() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([NodeBuilder::new("devices")
                            .children([
                                NodeBuilder::new("device-list")
                                    .attr("hash", "2:abcdef123456")
                                    .children([
                                        NodeBuilder::new("device").attr("id", "0").build(),
                                        NodeBuilder::new("device").attr("id", "1").build(),
                                        NodeBuilder::new("device")
                                            .attr("id", "5")
                                            .attr("is_hosted", "true")
                                            .build(),
                                    ])
                                    .build(),
                                build_test_key_index_list_node(&[0, 1, 5]),
                            ])
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.device_lists.len(), 1);
        assert_eq!(result.device_lists[0].user.user, "1234567890");
        assert_eq!(result.device_lists[0].devices.len(), 3);
        assert_eq!(result.device_lists[0].devices[0].device, 0);
        assert_eq!(result.device_lists[0].devices[1].device, 1);
        assert_eq!(result.device_lists[0].devices[2].device, 5);
        assert!(result.device_lists[0].devices[2].is_hosted);
        assert_eq!(
            result.device_lists[0].phash,
            Some("2:abcdef123456".to_string())
        );
        assert!(result.lid_mappings.is_empty());
    }

    #[test]
    fn device_list_devices_error_skips_only_that_user() {
        let jid1: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let jid2: Jid = "9876543210@s.whatsapp.net".parse().unwrap();
        let best_effort =
            DeviceListSpec::new(vec![jid1.clone(), jid2.clone()], "test-sid-best-effort");
        let complete = DeviceListSpec::new(vec![jid1.clone(), jid2.clone()], "test-sid-complete")
            .require_complete_response();
        let incremental = DeviceListSpec::with_hashes(vec![jid1, jid2], "test-sid", HashMap::new());

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", "1234567890@s.whatsapp.net")
                            .children([NodeBuilder::new("devices")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "500")
                                    .attr("text", "server")
                                    .build()])
                                .build()])
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", "9876543210@s.whatsapp.net")
                            .children([NodeBuilder::new("devices")
                                .children([
                                    NodeBuilder::new("device-list")
                                        .attr("hash", "2:ok")
                                        .children([NodeBuilder::new("device")
                                            .attr("id", "0")
                                            .build()])
                                        .build(),
                                    build_test_key_index_list_node(&[0]),
                                ])
                                .build()])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();

        assert!(
            complete.parse_response(&response.as_node_ref()).is_err(),
            "a per-user error makes a complete snapshot unusable"
        );
        for spec in [best_effort, incremental] {
            let result = spec.parse_response(&response.as_node_ref()).unwrap();
            assert_eq!(result.device_lists.len(), 1);
            assert_eq!(result.device_lists[0].user.user, "9876543210");
        }
    }

    /// A user whose companions cannot be validated still has a primary, and the
    /// primary is the device that carries the group sender key. WA Web's
    /// `handleOmittedResult` collapses such a record to
    /// `devices = [{id: DEFAULT_DEVICE_ID, keyIndex: 0}]` rather than dropping
    /// the identity, and `getFanOutList` falls back to the bare user wid for any
    /// wid it has no device list for ("no device for N wids => primary").
    /// Dropping the whole user takes them out of the SKDM target set and out of
    /// the phash, so nothing downstream can tell they were ever expected.
    #[test]
    fn user_without_key_index_keeps_its_primary() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([NodeBuilder::new("devices")
                            .children([NodeBuilder::new("device-list")
                                .attr("hash", "2:abcdef123456")
                                .children([
                                    NodeBuilder::new("device").attr("id", "0").build(),
                                    NodeBuilder::new("device").attr("id", "33").build(),
                                ])
                                .build()])
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(
            result.device_lists.len(),
            1,
            "an unvalidatable companion must not take its user's primary with it"
        );
        let user = &result.device_lists[0];
        assert_eq!(user.user.user, "1234567890");
        assert_eq!(
            user.devices.iter().map(|d| d.device).collect::<Vec<_>>(),
            [0],
            "only the primary survives: the companion has no signedKeyIndexBytes to validate it"
        );
        assert!(
            user.key_index_bytes.is_none(),
            "no key index was returned, so none is invented"
        );
        assert!(
            user.phash.is_none(),
            "the server's hash describes the devices it sent, not the ones kept: \
             persisting it would let a later query call this truncated list current"
        );
    }

    /// The degradation above has a floor: a device list that does not even carry
    /// a primary says nothing usable, so the user is still skipped rather than
    /// persisted as an empty record.
    #[test]
    fn user_without_key_index_or_primary_is_still_skipped() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([NodeBuilder::new("devices")
                            .children([NodeBuilder::new("device-list")
                                .children([NodeBuilder::new("device").attr("id", "33").build()])
                                .build()])
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert!(result.device_lists.is_empty());
    }

    #[test]
    fn device_list_result_devices_error_is_warn_only() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([
                    NodeBuilder::new("result")
                        .children([NodeBuilder::new("devices")
                            .children([NodeBuilder::new("error")
                                .attr("code", "500")
                                .attr("text", "server")
                                .build()])
                            .build()])
                        .build(),
                    NodeBuilder::new("list")
                        .children([NodeBuilder::new("user")
                            .attr("jid", "1234567890@s.whatsapp.net")
                            .children([NodeBuilder::new("devices")
                                .children([
                                    NodeBuilder::new("device-list")
                                        .attr("hash", "2:ok")
                                        .children([NodeBuilder::new("device")
                                            .attr("id", "0")
                                            .build()])
                                        .build(),
                                    build_test_key_index_list_node(&[0]),
                                ])
                                .build()])
                            .build()])
                        .build(),
                ])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.device_lists.len(), 1);
        assert_eq!(result.device_lists[0].user.user, "1234567890");
    }

    #[test]
    fn lid_query_lid_error_skips_only_that_user() {
        let jid1: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let jid2: Jid = "9876543210@s.whatsapp.net".parse().unwrap();
        let spec = LidQuerySpec::new(vec![jid1, jid2], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", "1234567890@s.whatsapp.net")
                            .children([NodeBuilder::new("lid")
                                .children([NodeBuilder::new("error")
                                    .attr("code", "404")
                                    .attr("text", "missing")
                                    .build()])
                                .build()])
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", "9876543210@s.whatsapp.net")
                            .children([NodeBuilder::new("lid")
                                .attr("val", "100000000000987@lid")
                                .build()])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.lid_mappings.len(), 1);
        assert_eq!(result.lid_mappings[0].phone_number, "9876543210");
        assert_eq!(result.lid_mappings[0].lid, "100000000000987");
    }

    #[test]
    fn lid_mapping_projection_accepts_standard_and_hosted_namespaces() {
        for (pn_server, lid_server) in [
            (Server::Pn, Server::Lid),
            (Server::Hosted, Server::HostedLid),
        ] {
            let user = UsyncUserResult {
                id: Some(Jid::new("13135550100", pn_server)),
                pn_jid: None,
                protocols: vec![UsyncProtocolResult::Lid(UsyncOutcome::Value(Some(
                    Jid::new("100000000000100", lid_server),
                )))],
            };

            let mapping = project_lid_mapping(&user).expect("expected LID mapping");
            assert_eq!(mapping.phone_number, "13135550100");
            assert_eq!(mapping.lid, "100000000000100");

            let canonicalized = UsyncUserResult {
                id: Some(Jid::new("100000000000100", lid_server)),
                pn_jid: Some(Jid::new("13135550100", pn_server)),
                protocols: Vec::new(),
            };
            let mapping = project_lid_mapping(&canonicalized).expect("expected pn_jid mapping");
            assert_eq!(mapping.phone_number, "13135550100");
            assert_eq!(mapping.lid, "100000000000100");
        }
    }

    #[test]
    fn complete_device_list_accepts_server_canonicalized_lid() {
        let requested = Jid::pn("12025550100");
        let spec =
            DeviceListSpec::new(vec![requested], "sid-canonicalized").require_complete_response();

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "100000000000100@lid")
                        .attr("pn_jid", "12025550100@s.whatsapp.net")
                        .children([NodeBuilder::new("devices")
                            .children([NodeBuilder::new("device-list")
                                .children([NodeBuilder::new("device").attr("id", "0").build()])
                                .build()])
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();

        let parsed = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(parsed.device_lists[0].user, Jid::lid("100000000000100"));
        assert_eq!(parsed.lid_mappings.len(), 1);
        assert_eq!(parsed.lid_mappings[0].phone_number, "12025550100");
        assert_eq!(parsed.lid_mappings[0].lid, "100000000000100");
    }

    #[test]
    fn test_device_list_spec_parse_response_multiple_users() {
        let jid1: Jid = "1111111111@s.whatsapp.net".parse().unwrap();
        let jid2: Jid = "2222222222@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid1, jid2], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", "1111111111@s.whatsapp.net")
                            .children([NodeBuilder::new("devices")
                                .children([NodeBuilder::new("device-list")
                                    .attr("hash", "2:hash1")
                                    .children([NodeBuilder::new("device").attr("id", "0").build()])
                                    .build()])
                                .build()])
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", "2222222222@s.whatsapp.net")
                            .children([NodeBuilder::new("devices")
                                .children([
                                    NodeBuilder::new("device-list")
                                        .attr("hash", "2:hash2")
                                        .children([
                                            NodeBuilder::new("device").attr("id", "0").build(),
                                            NodeBuilder::new("device").attr("id", "1").build(),
                                        ])
                                        .build(),
                                    build_test_key_index_list_node(&[0, 1]),
                                ])
                                .build()])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.device_lists.len(), 2);
        assert_eq!(result.device_lists[0].user.user, "1111111111");
        assert_eq!(result.device_lists[0].devices.len(), 1);
        assert_eq!(result.device_lists[0].phash, Some("2:hash1".to_string()));
        assert_eq!(result.device_lists[1].user.user, "2222222222");
        assert_eq!(result.device_lists[1].devices.len(), 2);
        assert_eq!(result.device_lists[1].phash, Some("2:hash2".to_string()));
    }

    #[test]
    fn test_device_list_spec_parse_response_with_lid() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = DeviceListSpec::new(vec![jid], "test-sid");

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", "1234567890@s.whatsapp.net")
                        .children([
                            NodeBuilder::new("lid")
                                .attr("val", "100000012345678@lid")
                                .build(),
                            NodeBuilder::new("devices")
                                .children([NodeBuilder::new("device-list")
                                    .attr("hash", "2:abcdef")
                                    .children([NodeBuilder::new("device").attr("id", "0").build()])
                                    .build()])
                                .build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.device_lists.len(), 1);
        assert_eq!(result.lid_mappings.len(), 1);
        assert_eq!(result.lid_mappings[0].phone_number, "1234567890");
        assert_eq!(result.lid_mappings[0].lid, "100000012345678");
    }

    #[test]
    fn parse_verified_name_skips_error_and_empty() {
        let business = |vn: Node| NodeBuilder::new("business").children([vn]).build();
        let parse = |business: Node| {
            let response = NodeBuilder::new("iq")
                .attr("type", "result")
                .children([NodeBuilder::new("usync")
                    .children([NodeBuilder::new("list")
                        .children([NodeBuilder::new("user")
                            .attr("jid", Jid::pn("1234567890"))
                            .children([business])
                            .build()])
                        .build()])
                    .build()])
                .build();
            let parsed = parse_usync_response(&response.as_node_ref()).unwrap();
            match parsed.users[0].protocol(UsyncProtocolKind::Business) {
                Some(UsyncProtocolResult::Business(UsyncOutcome::Value(result))) => {
                    result.verified_name.clone()
                }
                other => panic!("unexpected business result: {other:?}"),
            }
        };

        // <verified_name><error/></verified_name> -> absent
        let err = business(
            NodeBuilder::new("verified_name")
                .children([NodeBuilder::new("error").attr("code", "404").build()])
                .build(),
        );
        assert!(parse(err).is_none());

        // empty <verified_name/> (no attrs, no cert) -> absent
        let empty = business(NodeBuilder::new("verified_name").build());
        assert!(parse(empty).is_none());

        // real name attr -> present
        let real = business(
            NodeBuilder::new("verified_name")
                .attr("name", "Acme")
                .build(),
        );
        assert_eq!(
            parse(real).expect("real name").name.as_deref(),
            Some("Acme")
        );
    }

    /// `<user>` wrapper for a usync `type="result"` response.
    fn usync_result(users: Vec<Node>) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list").children(users).build()])
                .build()])
            .build()
    }

    fn pn_spec() -> IsOnWhatsAppSpec {
        IsOnWhatsAppSpec::new(
            vec![pn_user("1234567890")],
            "test-sid",
            IsOnWhatsAppQueryType::Pn,
        )
    }

    #[test]
    fn is_on_whatsapp_surfaces_the_username_on_the_contact_node() {
        let spec = pn_spec();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([NodeBuilder::new("contact")
                    .attr("type", "in")
                    .attr("username", "example.handle")
                    .build()])
                .build(),
        ]);

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results[0].username.as_deref(), Some("example.handle"));
        assert!(results[0].username_error.is_none());
    }

    #[test]
    fn is_on_whatsapp_prefers_the_username_subprotocol_over_the_contact_attribute() {
        let spec = pn_spec();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([
                    NodeBuilder::new("contact")
                        .attr("type", "in")
                        .attr("username", "stale.handle")
                        .build(),
                    NodeBuilder::new("username")
                        .string_content("fresh.handle")
                        .build(),
                ])
                .build(),
        ]);

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results[0].username.as_deref(), Some("fresh.handle"));
    }

    #[test]
    fn is_on_whatsapp_absent_username_stays_absent() {
        let spec = pn_spec();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([
                    NodeBuilder::new("contact").attr("type", "in").build(),
                    // An empty <username/> is how the server reports a deleted one.
                    NodeBuilder::new("username").build(),
                ])
                .build(),
            NodeBuilder::new("user")
                .attr("jid", "1234567891@s.whatsapp.net")
                .children([
                    NodeBuilder::new("contact").attr("type", "in").build(),
                    NodeBuilder::new("username").string_content("").build(),
                ])
                .build(),
        ]);

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.username.is_none()));
    }

    #[test]
    fn is_on_whatsapp_username_error_does_not_drop_the_user() {
        let spec = pn_spec();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([
                    NodeBuilder::new("contact").attr("type", "in").build(),
                    NodeBuilder::new("username")
                        .children([NodeBuilder::new("error").attr("code", "429").build()])
                        .build(),
                ])
                .build(),
        ]);

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert!(results[0].is_registered);
        assert!(results[0].username.is_none());
        assert_eq!(
            results[0].username_error.as_ref().and_then(|e| e.code),
            Some(429)
        );
    }

    #[test]
    fn is_on_whatsapp_query_asks_for_the_username_subprotocol() {
        for query_type in [IsOnWhatsAppQueryType::Pn, IsOnWhatsAppQueryType::Lid] {
            let jid = match query_type {
                IsOnWhatsAppQueryType::Pn => Jid::pn("1234567890"),
                IsOnWhatsAppQueryType::Lid => Jid::lid("100000001"),
            };
            let spec = IsOnWhatsAppSpec::new(
                vec![IsOnWhatsAppUser {
                    jid,
                    known_lid: None,
                }],
                "test-sid",
                query_type,
            );
            let iq = spec.build_iq();
            let Some(NodeContent::Nodes(nodes)) = &iq.content else {
                panic!("Expected NodeContent::Nodes");
            };
            let query = nodes[0].get_optional_child("query").unwrap();
            let username = query.get_optional_child("username").unwrap();
            assert!(username.attrs.is_empty());
            // The user node carries no username: nothing here is a lookup by one.
            let user = nodes[0]
                .get_optional_child("list")
                .and_then(|list| list.get_children_by_tag("user").next())
                .unwrap();
            assert!(user.get_optional_child("username").is_none());
        }
    }

    #[test]
    fn user_info_query_asks_for_the_username_subprotocol() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = UserInfoSpec::new(vec![jid], "test-sid");
        let iq = spec.build_iq();

        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("Expected NodeContent::Nodes");
        };
        let query = nodes[0].get_optional_child("query").unwrap();
        assert!(query.get_optional_child("username").is_some());
    }

    #[test]
    fn user_info_surfaces_the_username() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = UserInfoSpec::new(vec![jid.clone()], "test-sid");
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([NodeBuilder::new("username")
                    .string_content("example.handle")
                    .build()])
                .build(),
        ]);

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(
            results.get(&jid).unwrap().username.as_deref(),
            Some("example.handle")
        );
    }

    #[test]
    fn username_lookup_builds_the_official_contact_node() {
        let spec = UsernameLookupSpec::new("example.handle", Some("1234"), "sid-1").unwrap();
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "usync");
        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("Expected NodeContent::Nodes");
        };
        let usync = &nodes[0];
        assert!(usync.attrs.get("mode").is_some_and(|s| s == "query"));
        assert!(
            usync
                .attrs
                .get("context")
                .is_some_and(|s| s == "interactive")
        );

        let query = usync.get_optional_child("query").unwrap();
        let contact = query.get_optional_child("contact").unwrap();
        assert!(
            contact
                .attrs
                .get("addressing_mode")
                .is_some_and(|s| s == "lid")
        );
        assert!(query.get_optional_child("business").is_some());
        // No username subprotocol: the answer rides on <contact>, as WA Web's
        // queryUsernameExists expects.
        assert!(query.get_optional_child("username").is_none());

        let user_contact = usync
            .get_optional_child("list")
            .and_then(|list| list.get_children_by_tag("user").next())
            .and_then(|user| user.get_optional_child("contact"))
            .unwrap();
        assert_eq!(
            user_contact.attrs.get("username").unwrap(),
            "example.handle"
        );
        assert_eq!(user_contact.attrs.get("pin").unwrap(), "1234");
    }

    #[test]
    fn username_lookup_rejects_lengths_the_server_would_not_accept() {
        for username in ["", "ab", &"a".repeat(USERNAME_MAX_LENGTH + 1)] {
            let error = UsernameLookupSpec::new(username, None, "sid-1").unwrap_err();
            assert_eq!(
                error,
                UsernameLookupError::InvalidLength {
                    length: username.chars().count()
                }
            );
        }
        assert!(UsernameLookupSpec::new("abc", None, "sid-1").is_ok());
    }

    #[test]
    fn username_lookup_finds_the_account_behind_the_handle() {
        let spec = UsernameLookupSpec::new("example.handle", None, "sid-1").unwrap();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "100000001@lid")
                .children([
                    NodeBuilder::new("contact")
                        .attr("type", "in")
                        .attr("username", "example.handle")
                        .build(),
                    NodeBuilder::new("business")
                        .attr("pn_jid", "1234567890@s.whatsapp.net")
                        .children([NodeBuilder::new("verified_name")
                            .attr("name", "Acme")
                            .build()])
                        .build(),
                ])
                .build(),
        ]);

        let UsernameLookup::Found(user) = spec.parse_response(&response.as_node_ref()).unwrap()
        else {
            panic!("expected a resolved account");
        };
        assert_eq!(user.jid, Jid::lid("100000001"));
        assert_eq!(
            user.pn_jid.as_ref().map(|jid| jid.user.as_str()),
            Some("1234567890")
        );
        assert_eq!(user.username.as_deref(), Some("example.handle"));
        assert!(user.is_business);
        assert_eq!(
            user.verified_name.and_then(|name| name.name).as_deref(),
            Some("Acme")
        );
    }

    #[test]
    fn username_lookup_reports_a_withheld_identity_as_key_required() {
        let spec = UsernameLookupSpec::new("example.handle", None, "sid-1").unwrap();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .children([NodeBuilder::new("contact")
                    .attr("type", "in")
                    .attr("username", "example.handle")
                    .build()])
                .build(),
        ]);

        assert_eq!(
            spec.parse_response(&response.as_node_ref()).unwrap(),
            UsernameLookup::KeyRequired {
                username: Some("example.handle".into())
            }
        );
    }

    #[test]
    fn username_lookup_reports_an_unknown_handle_as_not_found() {
        let spec = UsernameLookupSpec::new("example.handle", None, "sid-1").unwrap();
        let out = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "100000001@lid")
                .children([NodeBuilder::new("contact").attr("type", "out").build()])
                .build(),
        ]);
        assert_eq!(
            spec.parse_response(&out.as_node_ref()).unwrap(),
            UsernameLookup::NotFound
        );

        let empty = usync_result(Vec::new());
        assert_eq!(
            spec.parse_response(&empty.as_node_ref()).unwrap(),
            UsernameLookup::NotFound
        );
    }

    /// A `<user>` the contact subprotocol never spoke for is not a resolution,
    /// however much of it the server filled in.
    #[test]
    fn username_lookup_refuses_to_resolve_without_a_contact_result() {
        let spec = UsernameLookupSpec::new("example.handle", None, "sid-1").unwrap();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "100000001@lid")
                .children([NodeBuilder::new("business").build()])
                .build(),
        ]);

        let error = spec
            .parse_response(&response.as_node_ref())
            .expect_err("a user with no contact result cannot resolve a handle")
            .to_string();
        assert!(error.contains("contact result"), "{error}");
    }

    /// A deletion the server reports through the subprotocol must not be undone
    /// by the `<contact>` attribute the same response happens to carry.
    #[test]
    fn is_on_whatsapp_empty_username_result_outranks_the_contact_attribute() {
        let spec = pn_spec();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([
                    NodeBuilder::new("contact")
                        .attr("type", "in")
                        .attr("username", "stale.handle")
                        .build(),
                    NodeBuilder::new("username").build(),
                ])
                .build(),
        ]);

        let results = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(results[0].username, None);
    }

    /// A rate-limited subprotocol has retracted nothing, so a username the
    /// contact result supplied on its own survives next to the error.
    #[test]
    fn is_on_whatsapp_username_error_keeps_what_the_contact_result_said() {
        let spec = pn_spec();
        let errored = |contact: Node| {
            usync_result(vec![
                NodeBuilder::new("user")
                    .attr("jid", "1234567890@s.whatsapp.net")
                    .children([
                        contact,
                        NodeBuilder::new("username")
                            .children([NodeBuilder::new("error").attr("code", "429").build()])
                            .build(),
                    ])
                    .build(),
            ])
        };

        let with_attribute = errored(
            NodeBuilder::new("contact")
                .attr("type", "in")
                .attr("username", "example.handle")
                .build(),
        );
        let results = spec.parse_response(&with_attribute.as_node_ref()).unwrap();
        assert_eq!(results[0].username.as_deref(), Some("example.handle"));
        assert_eq!(
            results[0].username_error.as_ref().and_then(|e| e.code),
            Some(429)
        );

        // Nothing to fall back on: the error is all there is to report.
        let without = errored(NodeBuilder::new("contact").attr("type", "in").build());
        let results = spec.parse_response(&without.as_node_ref()).unwrap();
        assert_eq!(results[0].username, None);
        assert!(results[0].username_error.is_some());
    }

    #[test]
    fn username_lookup_refuses_to_resolve_a_failed_contact() {
        let spec = UsernameLookupSpec::new("example.handle", None, "sid-1").unwrap();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "100000001@lid")
                .children([NodeBuilder::new("contact")
                    .children([NodeBuilder::new("error")
                        .attr("code", "403")
                        .attr("text", "forbidden")
                        .build()])
                    .build()])
                .build(),
        ]);

        let error = spec
            .parse_response(&response.as_node_ref())
            .expect_err("a failed contact subprotocol cannot resolve a handle")
            .to_string();
        assert!(error.contains("contact"), "{error}");
        assert!(error.contains("403"), "{error}");
    }

    /// A `pn_jid` the server spelled wrong is one user's bonus field, not a
    /// reason to reject the batch it rode in on.
    #[test]
    fn a_malformed_business_pn_jid_does_not_reject_the_response() {
        let spec = pn_spec();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "1234567890@s.whatsapp.net")
                .children([
                    NodeBuilder::new("contact").attr("type", "in").build(),
                    NodeBuilder::new("business")
                        .attr("pn_jid", "not a jid")
                        .build(),
                ])
                .build(),
        ]);

        let results = spec
            .parse_response(&response.as_node_ref())
            .expect("the response still parses");
        assert!(results[0].is_registered);
        assert!(results[0].is_business);
    }

    /// Only "in" is a confirmation. A type this parser cannot name must not
    /// resolve a handle, because `find_by_username` caches what it resolves.
    #[test]
    fn username_lookup_refuses_an_unknown_contact_type() {
        let spec = UsernameLookupSpec::new("example.handle", None, "sid-1").unwrap();
        let response = usync_result(vec![
            NodeBuilder::new("user")
                .attr("jid", "100000001@lid")
                .children([NodeBuilder::new("contact").attr("type", "sideways").build()])
                .build(),
        ]);

        let error = spec
            .parse_response(&response.as_node_ref())
            .expect_err("an unnameable contact type cannot resolve a handle")
            .to_string();
        assert!(error.contains("unknown contact type"), "{error}");
        assert!(error.contains("sideways"), "{error}");
    }
}
