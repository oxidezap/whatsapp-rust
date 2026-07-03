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
use crate::iq::tctoken::build_tc_token_node;
use crate::request::InfoQuery;
use crate::stanza::business::VerifiedName;
use anyhow::anyhow;
use log::warn;
use std::collections::HashMap;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Server};
use wacore_binary::{Node, NodeContent, NodeContentRef, NodeRef};

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
}

#[derive(Debug, Clone)]
pub struct IsOnWhatsAppUser {
    pub jid: Jid,
    /// Helps server optimize the lookup (WA Web pre-populates this from its LID cache).
    pub known_lid: Option<wacore_binary::CompactString>,
}

fn build_user_nodes(users: &[IsOnWhatsAppUser]) -> Vec<Node> {
    users
        .iter()
        .map(|user| {
            if user.jid.is_pn() {
                let phone = if user.jid.user.starts_with('+') {
                    user.jid.user.to_string()
                } else {
                    format!("+{}", user.jid.user)
                };
                let mut children = vec![NodeBuilder::new("contact").string_content(phone).build()];
                if let Some(lid) = &user.known_lid {
                    children.push(
                        NodeBuilder::new("lid")
                            .attr("jid", Jid::lid(lid.as_str()))
                            .build(),
                    );
                }
                NodeBuilder::new("user").children(children).build()
            } else {
                NodeBuilder::new("user")
                    .attr("jid", user.jid.to_non_ad())
                    .build()
            }
        })
        .collect()
}

/// Common fields parsed from a usync `<user>` node.
struct ParsedUserFields {
    jid: Jid,
    lid: Option<Jid>,
    lid_error: Option<UsyncSubprotocolError>,
    is_business: bool,
    business_error: Option<UsyncSubprotocolError>,
    status: Option<String>,
    status_error: Option<UsyncSubprotocolError>,
    verified_name: Option<VerifiedName>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct UsyncSubprotocolError {
    pub code: Option<u16>,
    pub text: Option<String>,
    pub backoff: Option<u32>,
}

fn parse_subprotocol_error(protocol_node: &NodeRef<'_>) -> Option<UsyncSubprotocolError> {
    let error_node = protocol_node.get_optional_child("error")?;
    Some(UsyncSubprotocolError {
        code: error_node
            .attrs()
            .optional_string("code")
            .and_then(|s| s.parse().ok()),
        text: error_node
            .attrs()
            .optional_string("text")
            .map(|s| s.to_string()),
        backoff: error_node
            .attrs()
            .optional_string("backoff")
            .and_then(|s| s.parse().ok()),
    })
}

fn usync_subprotocol_error_message(tag: &str, error: &UsyncSubprotocolError) -> String {
    let code = error
        .code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let text = error.text.as_deref().unwrap_or("");
    format!("usync {tag} error {code}: {text}")
}

fn parse_lid_fields(user_node: &NodeRef<'_>) -> (Option<Jid>, Option<UsyncSubprotocolError>) {
    match user_node.get_optional_child("lid") {
        Some(lid_node) => {
            if let Some(error) = parse_subprotocol_error(lid_node) {
                (None, Some(error))
            } else {
                (
                    lid_node
                        .attrs()
                        .optional_string("val")
                        .and_then(|val| val.parse::<Jid>().ok()),
                    None,
                )
            }
        }
        None => (None, None),
    }
}

fn parse_contact_fields(
    user_node: &NodeRef<'_>,
    jid: &Jid,
) -> (bool, Option<UsyncSubprotocolError>) {
    match user_node.get_optional_child("contact") {
        Some(contact_node) => {
            if let Some(error) = parse_subprotocol_error(contact_node) {
                (false, Some(error))
            } else {
                (
                    contact_node
                        .get_attr("type")
                        .is_some_and(|value| value.as_str() == "in"),
                    None,
                )
            }
        }
        None if jid.is_lid() => (true, None),
        None => (false, None),
    }
}

fn parse_verified_name_from_business(business_node: &NodeRef<'_>) -> Option<VerifiedName> {
    let vn_node = business_node.get_optional_child("verified_name")?;
    // Treat an <error> child or an empty marker (no name, no cert) as absent,
    // mirroring the status/picture parsers, so `verified_name.is_some()` means a
    // real verified name was returned.
    if vn_node.get_optional_child("error").is_some() {
        return None;
    }
    let parsed = VerifiedName::try_from_node(vn_node).ok()?;
    if parsed.name.is_none() && parsed.certificate.is_none() {
        return None;
    }
    Some(parsed)
}

fn parse_business_fields(
    user_node: &NodeRef<'_>,
) -> (bool, Option<VerifiedName>, Option<UsyncSubprotocolError>) {
    match user_node.get_optional_child("business") {
        Some(business_node) => {
            if let Some(error) = parse_subprotocol_error(business_node) {
                (false, None, Some(error))
            } else {
                (true, parse_verified_name_from_business(business_node), None)
            }
        }
        None => (false, None, None),
    }
}

/// Parse common fields from a usync `<user>` node.
fn parse_user_common_fields(user_node: &NodeRef<'_>) -> Option<ParsedUserFields> {
    let jid = user_node
        .attrs()
        .optional_string("jid")?
        .parse::<Jid>()
        .ok()?;

    let (lid, lid_error) = parse_lid_fields(user_node);

    let (status, status_error) = match user_node.get_optional_child("status") {
        Some(status_node) => {
            if let Some(error) = parse_subprotocol_error(status_node) {
                (None, Some(error))
            } else {
                let status = match status_node.content.as_deref() {
                    Some(NodeContentRef::String(s)) if !s.is_empty() => Some(s.to_string()),
                    _ => None,
                };
                (status, None)
            }
        }
        None => (None, None),
    };

    let (is_business, verified_name, business_error) = parse_business_fields(user_node);

    Some(ParsedUserFields {
        jid,
        lid,
        lid_error,
        is_business,
        business_error,
        status,
        status_error,
        verified_name,
    })
}

fn parse_picture_id_fields(
    user_node: &NodeRef<'_>,
) -> (Option<String>, Option<UsyncSubprotocolError>) {
    match user_node.get_optional_child("picture") {
        Some(pic_node) => {
            if let Some(error) = parse_subprotocol_error(pic_node) {
                (None, Some(error))
            } else {
                (
                    pic_node
                        .attrs()
                        .optional_string("id")
                        .map(|s| s.to_string()),
                    None,
                )
            }
        }
        None => (None, None),
    }
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

fn build_business_query_node() -> Node {
    NodeBuilder::new("business")
        .children(vec![NodeBuilder::new("verified_name").build()])
        .build()
}

/// Check `<usync><result>` for per-protocol errors.
fn check_usync_result_errors(usync: &NodeRef<'_>) -> Result<(), anyhow::Error> {
    check_usync_result_errors_for(
        usync,
        &["contact", "lid", "business", "status", "picture", "devices"],
    )
}

fn check_usync_result_errors_for(
    usync: &NodeRef<'_>,
    tags: &[&'static str],
) -> Result<(), anyhow::Error> {
    let Some(result_node) = usync.get_optional_child("result") else {
        return Ok(());
    };
    for &tag in tags {
        if let Some(protocol_node) = result_node.get_optional_child(tag)
            && let Some(error) = parse_subprotocol_error(protocol_node)
        {
            return Err(anyhow!(usync_subprotocol_error_message(tag, &error)));
        }
    }
    Ok(())
}

fn warn_usync_result_error(usync: &NodeRef<'_>, tag: &'static str) {
    if let Some(result_node) = usync.get_optional_child("result")
        && let Some(protocol_node) = result_node.get_optional_child(tag)
        && let Some(error) = parse_subprotocol_error(protocol_node)
    {
        warn!(
            target: "usync",
            "{}; continuing with returned users",
            usync_subprotocol_error_message(tag, &error)
        );
    }
}

impl IqSpec for IsOnWhatsAppSpec {
    type Response = Vec<IsOnWhatsAppResult>;

    fn build_iq(&self) -> InfoQuery<'static> {
        let mut query_children = Vec::new();
        if self.query_type == IsOnWhatsAppQueryType::Pn {
            query_children.push(NodeBuilder::new("contact").build());
        }
        query_children.push(NodeBuilder::new("lid").build());
        query_children.push(build_business_query_node());

        let query_node = NodeBuilder::new("query").children(query_children).build();

        let user_nodes = build_user_nodes(&self.users);
        let list_node = NodeBuilder::new("list").children(user_nodes).build();

        let usync_node = NodeBuilder::new("usync")
            .attr("sid", self.sid.as_str())
            .attr("mode", UsyncMode::Query.as_str())
            .attr("last", "true")
            .attr("index", "0")
            .attr("context", UsyncContext::Interactive.as_str())
            .children(vec![query_node, list_node])
            .build();

        InfoQuery::get(
            "usync",
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![usync_node])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let usync = response
            .get_optional_child("usync")
            .ok_or_else(|| anyhow!("Response missing <usync> node"))?;

        check_usync_result_errors(usync)?;

        let list = usync
            .get_optional_child("list")
            .ok_or_else(|| anyhow!("Response missing <list> node"))?;

        let mut results = Vec::new();

        for user_node in list.get_children_by_tag("user") {
            let Some(jid_str) = user_node.attrs().optional_string("jid") else {
                continue;
            };
            let Ok(jid) = jid_str.parse::<Jid>() else {
                continue;
            };

            let pn_jid = user_node
                .attrs()
                .optional_string("pn_jid")
                .and_then(|s| s.parse::<Jid>().ok());

            let (lid, lid_error) = parse_lid_fields(user_node);
            let (is_registered, contact_error) = parse_contact_fields(user_node, &jid);
            let (is_business, verified_name, business_error) = parse_business_fields(user_node);

            results.push(IsOnWhatsAppResult {
                jid,
                lid,
                pn_jid,
                is_registered,
                contact_error,
                lid_error,
                is_business,
                business_error,
                verified_name,
            });
        }

        Ok(results)
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

impl IqSpec for UserInfoSpec {
    type Response = HashMap<Jid, UserInfo>;

    fn build_iq(&self) -> InfoQuery<'static> {
        let query_node = NodeBuilder::new("query")
            .children(vec![
                NodeBuilder::new("business")
                    .children(vec![NodeBuilder::new("verified_name").build()])
                    .build(),
                NodeBuilder::new("status").build(),
                NodeBuilder::new("picture").build(),
                NodeBuilder::new("devices").attr("version", "2").build(),
                NodeBuilder::new("lid").build(),
            ])
            .build();

        let user_nodes: Vec<Node> = self
            .jids
            .iter()
            .map(|jid| {
                let key = jid.to_non_ad().to_string();
                let mut builder = NodeBuilder::new("user").attr("jid", key.clone());
                if let Some(token) = self.tc_tokens.get(&key) {
                    builder = builder.children([build_tc_token_node(token)]);
                }
                builder.build()
            })
            .collect();

        let list_node = NodeBuilder::new("list").children(user_nodes).build();

        let usync_node = NodeBuilder::new("usync")
            .attr("sid", self.sid.as_str())
            .attr("mode", UsyncMode::Full.as_str())
            .attr("last", "true")
            .attr("index", "0")
            .attr("context", UsyncContext::Background.as_str())
            .children(vec![query_node, list_node])
            .build();

        InfoQuery::get(
            "usync",
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![usync_node])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let usync = response
            .get_optional_child("usync")
            .ok_or_else(|| anyhow!("Response missing <usync> node"))?;
        check_usync_result_errors(usync)?;

        let list = usync
            .get_optional_child("list")
            .ok_or_else(|| anyhow!("Response missing <list> node"))?;

        let mut results = HashMap::new();

        for user_node in list.get_children_by_tag("user") {
            if let Some(fields) = parse_user_common_fields(user_node) {
                let (picture_id, picture_error) = parse_picture_id_fields(user_node);
                let (devices, devices_error) = parse_user_device_ids(user_node);
                results.insert(
                    fields.jid.clone(),
                    UserInfo {
                        jid: fields.jid,
                        lid: fields.lid,
                        lid_error: fields.lid_error,
                        status: fields.status,
                        status_error: fields.status_error,
                        picture_id,
                        picture_error,
                        is_business: fields.is_business,
                        business_error: fields.business_error,
                        verified_name: fields.verified_name,
                        devices,
                        devices_error,
                    },
                );
            }
        }

        Ok(results)
    }
}

/// Device IDs plus a subprotocol error if `<devices><error>` was returned.
fn parse_user_device_ids(user_node: &NodeRef<'_>) -> (Vec<u16>, Option<UsyncSubprotocolError>) {
    let Some(devices_node) = user_node.get_optional_child("devices") else {
        return (Vec::new(), None);
    };
    if let Some(error) = parse_subprotocol_error(devices_node) {
        return (Vec::new(), Some(error));
    }
    let Some(device_list) = devices_node.get_optional_child("device-list") else {
        return (Vec::new(), None);
    };
    let devices = device_list
        .get_children_by_tag("device")
        .filter_map(|d| d.attrs().optional_string("id").and_then(|s| s.parse().ok()))
        .collect();
    (devices, None)
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
    pub hashes: std::collections::HashMap<Jid, (String, i64)>,
}

impl DeviceListSpec {
    pub fn new(jids: Vec<Jid>, sid: impl Into<String>) -> Self {
        Self {
            jids,
            sid: sid.into(),
            hashes: std::collections::HashMap::new(),
        }
    }

    /// Like [`new`](Self::new) but carries per-user `device_hash`/`ts` hints so the
    /// server can skip unchanged users. Keys are bare (`to_non_ad`) jids.
    pub fn with_hashes(
        jids: Vec<Jid>,
        sid: impl Into<String>,
        hashes: std::collections::HashMap<Jid, (String, i64)>,
    ) -> Self {
        Self {
            jids,
            sid: sid.into(),
            hashes,
        }
    }
}

impl IqSpec for DeviceListSpec {
    type Response = DeviceListResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        let query_node = NodeBuilder::new("query")
            .children(vec![
                NodeBuilder::new("devices").attr("version", "2").build(),
            ])
            .build();

        let user_nodes: Vec<Node> = self
            .jids
            .iter()
            .map(|jid| {
                let bare = jid.to_non_ad();
                let mut builder = NodeBuilder::new("user").attr("jid", bare.clone());
                // WA Web sends the cached per-user device list hash so the server
                // can answer "unchanged" by omitting the user from the response.
                if let Some((device_hash, ts)) = self.hashes.get(&bare) {
                    builder = builder.children([NodeBuilder::new("devices")
                        .attr("device_hash", device_hash.as_str())
                        .attr("ts", ts.to_string())
                        .build()]);
                }
                builder.build()
            })
            .collect();

        let list_node = NodeBuilder::new("list").children(user_nodes).build();

        let usync_node = NodeBuilder::new("usync")
            .attr("sid", self.sid.as_str())
            .attr("mode", UsyncMode::Query.as_str())
            .attr("last", "true")
            .attr("index", "0")
            .attr("context", UsyncContext::Message.as_str())
            .children(vec![query_node, list_node])
            .build();

        InfoQuery::get(
            "usync",
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![usync_node])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let usync = response
            .get_optional_child("usync")
            .ok_or_else(|| anyhow!("<usync> not found in usync response"))?;
        check_usync_result_errors_for(usync, &["contact", "lid", "business", "status", "picture"])?;
        warn_usync_result_error(usync, "devices");
        let list_node = usync
            .get_optional_child("list")
            .ok_or_else(|| anyhow!("<list> not found in usync response"))?;

        let mut device_lists = Vec::new();
        let mut lid_mappings = Vec::new();

        for user_node in list_node.get_children_by_tag("user") {
            let user_jid = user_node
                .attrs()
                .optional_jid("jid")
                .ok_or_else(|| anyhow!("user node missing required 'jid' attribute"))?;
            if let Some(devices_node) = user_node.get_optional_child("devices")
                && let Some(error) = parse_subprotocol_error(devices_node)
            {
                warn!(
                    target: "usync",
                    "{} for {user_jid}; skipping user",
                    usync_subprotocol_error_message("devices", &error)
                );
                continue;
            }

            // Extract LID mapping if present
            if user_jid.server == wacore_binary::Server::Pn
                && let Some(lid_node) = user_node.get_optional_child("lid")
            {
                let lid_val = lid_node.attrs().optional_string("val").unwrap_or_default();
                if !lid_val.is_empty()
                    && let Ok(lid_jid) = lid_val.parse::<Jid>()
                    && lid_jid.server == wacore_binary::Server::Lid
                {
                    lid_mappings.push(UsyncLidMapping {
                        phone_number: user_jid.user.clone(),
                        lid: lid_jid.user.clone(),
                    });
                }
            }

            // Extract device list - skip user if not present
            let device_list_node = match user_node
                .get_optional_child_by_tag(&["devices", "device-list"])
            {
                Some(node) => node,
                None => {
                    warn!(target: "usync", "<device-list> not found for user {user_jid}, skipping");
                    continue;
                }
            };

            // Extract phash from device-list node attributes
            let phash = device_list_node
                .attrs()
                .optional_string("hash")
                .map(|s| s.to_string());

            // Parse key-index-list from <devices> node
            let devices_parent = user_node.get_optional_child("devices");
            let key_index_bytes = devices_parent
                .and_then(|dp| dp.get_optional_child("key-index-list"))
                .and_then(|ki| match ki.content.as_deref() {
                    Some(NodeContentRef::Bytes(b)) if !b.is_empty() => Some(b.to_vec()),
                    _ => None,
                });

            let mut devices = Vec::new();
            for device_node in device_list_node.get_children_by_tag("device") {
                let Some(device_id_str) = device_node.attrs().optional_string("id") else {
                    warn!(target: "usync", "device node missing 'id' attribute for user {user_jid}, skipping device");
                    continue;
                };
                let Ok(device_id) = device_id_str.parse::<u16>() else {
                    warn!(target: "usync", "invalid device id '{}' for user {user_jid}, skipping device", device_id_str);
                    continue;
                };

                let key_index = device_node
                    .attrs()
                    .optional_string("key-index")
                    .and_then(|s| s.parse::<u32>().ok());
                devices.push(crate::usync::UsyncDevice {
                    device: device_id,
                    key_index,
                });
            }

            let has_companion = devices.iter().any(|d| d.device != 0);
            if has_companion && key_index_bytes.is_none() {
                warn!(
                    target: "usync",
                    "User {user_jid} has companion devices but no signedKeyIndexBytes, skipping"
                );
                continue;
            }

            device_lists.push(UserDeviceList {
                user: user_jid.to_non_ad(),
                devices,
                phash,
                key_index_bytes,
            });
        }

        Ok(DeviceListResponse {
            device_lists,
            lid_mappings,
        })
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

/// Response: just the LID mappings learned.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct LidQueryResponse {
    pub lid_mappings: Vec<UsyncLidMapping>,
}

impl IqSpec for LidQuerySpec {
    type Response = LidQueryResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        let query_node = NodeBuilder::new("query")
            .children(vec![NodeBuilder::new("lid").build()])
            .build();

        let user_nodes: Vec<Node> = self
            .jids
            .iter()
            .map(|jid| {
                NodeBuilder::new("user")
                    .attr("jid", jid.to_non_ad())
                    .build()
            })
            .collect();

        let list_node = NodeBuilder::new("list").children(user_nodes).build();

        let usync_node = NodeBuilder::new("usync")
            .attr("sid", self.sid.as_str())
            .attr("mode", UsyncMode::Query.as_str())
            .attr("last", "true")
            .attr("index", "0")
            // WA Web ContactSyncApi uses "background" for LID resolution
            .attr("context", UsyncContext::Background.as_str())
            .children(vec![query_node, list_node])
            .build();

        InfoQuery::get(
            "usync",
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![usync_node])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let usync = response
            .get_optional_child("usync")
            .ok_or_else(|| anyhow!("LID query response missing <usync> node"))?;
        check_usync_result_errors(usync)?;
        let list = usync
            .get_optional_child("list")
            .ok_or_else(|| anyhow!("LID query response missing <list> node"))?;
        for user_node in list.get_children_by_tag("user") {
            let user_jid = user_node
                .attrs()
                .optional_string("jid")
                .map(|jid| jid.to_string())
                .unwrap_or_else(|| "<unknown>".to_string());
            if let Some(lid_node) = user_node.get_optional_child("lid")
                && let Some(error) = parse_subprotocol_error(lid_node)
            {
                warn!(
                    target: "usync",
                    "{} for {user_jid}; skipping user",
                    usync_subprotocol_error_message("lid", &error)
                );
                continue;
            }
        }

        let lid_mappings = crate::usync::parse_lid_mappings_from_response(response);
        Ok(LidQueryResponse { lid_mappings })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_device_list_spec_build_iq_with_device_hash() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let mut hashes = std::collections::HashMap::new();
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
        let spec = DeviceListSpec::new(vec![a, b], "sid-omit");

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

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.device_lists.len(), 1, "omitted user must not appear");
        assert_eq!(result.device_lists[0].user.user, "1111111111");
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
                                        NodeBuilder::new("device").attr("id", "5").build(),
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
        let spec = DeviceListSpec::new(vec![jid1, jid2], "test-sid");

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

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.device_lists.len(), 1);
        assert_eq!(result.device_lists[0].user.user, "9876543210");
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

        // <verified_name><error/></verified_name> -> absent
        let err = business(
            NodeBuilder::new("verified_name")
                .children([NodeBuilder::new("error").attr("code", "404").build()])
                .build(),
        );
        assert!(parse_verified_name_from_business(&err.as_node_ref()).is_none());

        // empty <verified_name/> (no attrs, no cert) -> absent
        let empty = business(NodeBuilder::new("verified_name").build());
        assert!(parse_verified_name_from_business(&empty.as_node_ref()).is_none());

        // real name attr -> present
        let real = business(
            NodeBuilder::new("verified_name")
                .attr("name", "Acme")
                .build(),
        );
        assert_eq!(
            parse_verified_name_from_business(&real.as_node_ref())
                .expect("real name")
                .name
                .as_deref(),
            Some("Acme")
        );
    }
}
