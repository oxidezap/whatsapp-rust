use crate::node::NodeStr;
use compact_str::CompactString;
use std::fmt;
use std::str::FromStr;

/// Intermediate result from fast JID parsing.
/// This avoids allocations by returning byte indices into the original string.
#[derive(Debug, Clone, Copy)]
pub struct ParsedJidParts<'a> {
    pub user: &'a str,
    pub server: &'a str,
    pub agent: u8,
    pub device: u16,
    pub integrator: u16,
}

/// Decimal-only `u16` parser for the device and agent fields.
///
/// Deliberately not `str::parse`: `from_str_radix` is generic over radix and
/// signedness and carries the branches to prove it, which the JID scanner pays
/// on every device it splits out. The accepted grammar is the same one
/// `u16::from_str` accepts — an optional `+`, then decimal digits, rejecting
/// empty input and anything that overflows — and `decimal_fast_path_matches_u16_from_str`
/// holds the two together. A `-` is a non-digit here for the same reason it is
/// one in std: the sign is only recognised for signed types.
#[inline]
fn parse_u16_decimal(s: &str) -> Option<u16> {
    let mut bytes = s.as_bytes();
    if bytes.first() == Some(&b'+') {
        bytes = &bytes[1..];
    }
    if bytes.is_empty() {
        return None;
    }

    let mut value = 0u16;
    for &byte in bytes {
        let digit = byte.wrapping_sub(b'0');
        if digit > 9 {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(digit as u16)?;
    }
    Some(value)
}

/// Single-pass JID parser optimized for hot paths.
/// Scans the input string once to find all relevant separators (@, :)
/// and returns slices into the original string without allocation.
///
/// Returns `None` for JIDs that need full validation (edge cases, unknown servers, etc.)
#[inline]
pub fn parse_jid_fast(s: &str) -> Option<ParsedJidParts<'_>> {
    parse_jid_scan(s).map(|(parts, _)| parts)
}

/// Shared scanner behind [`parse_jid_fast`] and [`parse_jid_ref`]. The resolved
/// [`Server`] comes back alongside the borrowed parts because the scan already
/// had to classify the server to know whether dots in the user part are agent
/// separators — returning it lets `parse_jid_ref` skip a second lookup.
///
/// `None` in the second slot means the server suffix is not one we know; the
/// parts are still filled in (with the generic agent/device rules), which is
/// what `parse_jid_fast`'s callers rely on.
#[inline]
fn parse_jid_scan(s: &str) -> Option<(ParsedJidParts<'_>, Option<Server>)> {
    let bytes = s.as_bytes();

    // One pass over the *user* part only: everything after `@` is the server,
    // which carries no separators we care about, so the scan stops there.
    let mut at = usize::MAX;
    let mut colon_pos: Option<usize> = None;
    let mut last_dot_pos: Option<usize> = None;

    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'@' => {
                at = i;
                break;
            }
            b':' => colon_pos = Some(i),
            // Dots after the first colon belong to the device, not the agent.
            b'.' if colon_pos.is_none() => last_dot_pos = Some(i),
            _ => {}
        }
    }

    // No `@` (server-only JID) or an empty user: let the fallback validate it.
    if at == usize::MAX || at == 0 {
        return None;
    }

    let user_part = &s[..at];
    let server_str = &s[at + 1..];
    let server = Server::parse_known(server_str);

    match server {
        // LID user parts may contain dots, which are not agent separators.
        Some(Server::Lid) => {
            let (user, device) = match colon_pos {
                Some(pos) => (&s[..pos], parse_u16_decimal(&s[pos + 1..at]).unwrap_or(0)),
                None => (user_part, 0),
            };
            Some((
                ParsedJidParts {
                    user,
                    server: server_str,
                    agent: 0,
                    device,
                    integrator: 0,
                },
                server,
            ))
        }
        // `s.whatsapp.net` has no agent in the string form; a trailing dotted
        // number is the legacy device spelling.
        Some(Server::Pn) => {
            if let Some(pos) = colon_pos {
                return Some((
                    ParsedJidParts {
                        user: &s[..pos],
                        server: server_str,
                        agent: 0,
                        device: parse_u16_decimal(&s[pos + 1..at]).unwrap_or(0),
                        integrator: 0,
                    },
                    server,
                ));
            }
            if let Some(dot_pos) = last_dot_pos
                && let Some(device_val) = parse_u16_decimal(&s[dot_pos + 1..at])
            {
                return Some((
                    ParsedJidParts {
                        user: &s[..dot_pos],
                        server: server_str,
                        agent: 0,
                        device: device_val,
                        integrator: 0,
                    },
                    server,
                ));
            }
            Some((
                ParsedJidParts {
                    user: user_part,
                    server: server_str,
                    agent: 0,
                    device: 0,
                    integrator: 0,
                },
                server,
            ))
        }
        // Everything else (including unknown servers): `user.agent:device`.
        _ => {
            let (user_before_colon, device) = match colon_pos {
                Some(pos) => (&s[..pos], parse_u16_decimal(&s[pos + 1..at]).unwrap_or(0)),
                None => (user_part, 0),
            };
            // Deliberately `rfind` on the pre-colon slice rather than reusing
            // `last_dot_pos`: the two differ for pathological users that hold a
            // second colon (`a:1.5:2`), and this branch is cold enough that
            // matching the historical rule is worth the extra scan.
            let (final_user, agent) = match user_before_colon.rfind('.') {
                Some(dot_pos) => match parse_u16_decimal(&user_before_colon[dot_pos + 1..]) {
                    Some(agent_val) if agent_val <= u8::MAX as u16 => {
                        (&user_before_colon[..dot_pos], agent_val as u8)
                    }
                    _ => (user_before_colon, 0),
                },
                None => (user_before_colon, 0),
            };
            Some((
                ParsedJidParts {
                    user: final_user,
                    server: server_str,
                    agent,
                    device,
                    integrator: 0,
                },
                server,
            ))
        }
    }
}

/// Parse the allocation-free JID shapes into the same borrowed type used by
/// the binary decoder.
///
/// Returns `None` when the string needs [`Jid`]'s compatibility fallback or
/// names an unknown server. Callers that must accept those edge cases can fall
/// back to `s.parse::<Jid>()`; normal user/group/LID/bot JIDs stay borrowed.
#[inline]
pub fn parse_jid_ref(s: &str) -> Option<JidRef<'_>> {
    let (parts, server) = parse_jid_scan(s)?;
    Some(JidRef {
        user: NodeStr::Borrowed(parts.user),
        server: server?,
        agent: parts.agent,
        device: parts.device,
        integrator: parts.integrator,
    })
}

/// Known WhatsApp server identifiers.
///
/// Maps to the wire protocol's AD_JID domain type (u8) and the `@server` suffix
/// in JID string representation.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Server {
    #[default]
    Pn = 0,
    Lid = 1,
    Group = 2,
    Broadcast = 3,
    Newsletter = 4,
    Hosted = 5,
    HostedLid = 6,
    Messenger = 7,
    Interop = 8,
    Bot = 9,
    Legacy = 10,
    /// `@call` call-signaling JID; not an AD server, so it round-trips via JID_PAIR.
    Call = 11,
}

#[cfg(feature = "serde")]
impl serde::Serialize for Server {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for Server {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ServerVisitor;

        impl serde::de::Visitor<'_> for ServerVisitor {
            type Value = Server;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a known WhatsApp server identifier")
            }

            fn visit_borrowed_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Server::try_from(value).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Server::try_from(value).map_err(E::custom)
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Server::try_from(value.as_str()).map_err(E::custom)
            }
        }

        deserializer.deserialize_str(ServerVisitor)
    }
}

impl Server {
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pn => "s.whatsapp.net",
            Self::Lid => "lid",
            Self::Group => "g.us",
            Self::Broadcast => "broadcast",
            Self::Newsletter => "newsletter",
            Self::Hosted => "hosted",
            Self::HostedLid => "hosted.lid",
            Self::Messenger => "msgr",
            Self::Interop => "interop",
            Self::Bot => "bot",
            Self::Legacy => "c.us",
            Self::Call => "call",
        }
    }

    /// Phone-number-namespaced servers (`@s.whatsapp.net`, `@hosted`).
    /// The PN side of the LID↔PN mapping treats these as a single class.
    #[inline]
    pub fn is_pn_family(self) -> bool {
        matches!(self, Self::Pn | Self::Hosted)
    }

    /// LID-namespaced servers (`@lid`, `@hosted.lid`).
    #[inline]
    pub fn is_lid_family(self) -> bool {
        matches!(self, Self::Lid | Self::HostedLid)
    }

    /// Whether the `agent` byte is part of the rendered JID for this server.
    /// AD-capable servers (Pn/Lid/Hosted/HostedLid) have no agent of their own —
    /// their wire form spells the server as a domain byte, which the decoder
    /// resolves into `server` rather than keeping — so an agent set on one is
    /// meaningless and stays out of the rendered form. Others (e.g. `@bot`,
    /// `@interop`) render it. Single source of truth shared by the formatter and
    /// `Jid::is_same_chat_as`.
    #[inline]
    pub fn renders_agent(self) -> bool {
        !matches!(self, Self::Pn | Self::Lid | Self::Hosted | Self::HostedLid)
    }

    /// Whether the `user` part is (or can be) a real phone number, i.e. PII that
    /// must be redacted in tracing fields. LID-family/group/broadcast/newsletter/
    /// bot/call users are pseudonymous or non-personal and are safe to render raw.
    #[inline]
    pub fn carries_phone_number(self) -> bool {
        matches!(
            self,
            Self::Pn | Self::Hosted | Self::Legacy | Self::Messenger | Self::Interop
        )
    }
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for Server {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Server {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl Server {
    /// Allocation-free server lookup. `TryFrom<&str>` builds a `JidError` —
    /// and therefore a `String` — for an unknown suffix, which the JID scanner
    /// hits on every non-JID string it is asked to classify; this returns the
    /// same answer without paying for the message nobody reads.
    #[inline]
    pub fn parse_known(s: &str) -> Option<Self> {
        Some(match s {
            "s.whatsapp.net" => Self::Pn,
            "lid" => Self::Lid,
            "g.us" => Self::Group,
            "broadcast" => Self::Broadcast,
            "newsletter" => Self::Newsletter,
            "hosted" => Self::Hosted,
            "hosted.lid" => Self::HostedLid,
            "msgr" => Self::Messenger,
            "interop" => Self::Interop,
            "bot" => Self::Bot,
            "c.us" => Self::Legacy,
            "call" => Self::Call,
            _ => return None,
        })
    }
}

impl TryFrom<&str> for Server {
    type Error = JidError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Server::parse_known(s)
            .ok_or_else(|| JidError::InvalidFormat(format!("unknown server: {s}")))
    }
}

// Keep string constants for backward compatibility and use in non-JID contexts
pub const DEFAULT_USER_SERVER: &str = "s.whatsapp.net";
pub const SERVER_JID: &str = "s.whatsapp.net";
pub const GROUP_SERVER: &str = "g.us";
pub const LEGACY_USER_SERVER: &str = "c.us";
pub const BROADCAST_SERVER: &str = "broadcast";
pub const HIDDEN_USER_SERVER: &str = "lid";
pub const NEWSLETTER_SERVER: &str = "newsletter";
pub const HOSTED_SERVER: &str = "hosted";
pub const HOSTED_LID_SERVER: &str = "hosted.lid";
pub const MESSENGER_SERVER: &str = "msgr";
pub const INTEROP_SERVER: &str = "interop";
pub const BOT_SERVER: &str = "bot";
pub const STATUS_BROADCAST_USER: &str = "status";
pub const PSA_USER: &str = "0";

pub type MessageId = String;
pub type MessageServerId = i32;
#[derive(Debug)]
pub enum JidError {
    // REMOVE: #[error("...")]
    InvalidFormat(String),
    // REMOVE: #[error("...")]
    Parse(std::num::ParseIntError),
}

impl fmt::Display for JidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JidError::InvalidFormat(s) => write!(f, "Invalid JID format: {s}"),
            JidError::Parse(e) => write!(f, "Failed to parse component: {e}"),
        }
    }
}

impl std::error::Error for JidError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            JidError::Parse(e) => Some(e),
            _ => None,
        }
    }
}

// Add From impl
impl From<std::num::ParseIntError> for JidError {
    fn from(err: std::num::ParseIntError) -> Self {
        JidError::Parse(err)
    }
}

pub trait JidExt {
    fn user(&self) -> &str;
    fn server(&self) -> Server;
    fn device(&self) -> u16;
    fn integrator(&self) -> u16;

    fn is_ad(&self) -> bool {
        self.device() > 0
            && matches!(
                self.server(),
                Server::Pn | Server::Lid | Server::Hosted | Server::HostedLid
            )
    }

    fn is_interop(&self) -> bool {
        self.server() == Server::Interop && self.integrator() > 0
    }

    fn is_messenger(&self) -> bool {
        self.server() == Server::Messenger && self.device() > 0
    }

    fn is_group(&self) -> bool {
        self.server() == Server::Group
    }

    fn is_broadcast_list(&self) -> bool {
        self.server() == Server::Broadcast && self.user() != STATUS_BROADCAST_USER
    }

    fn is_status_broadcast(&self) -> bool {
        self.server() == Server::Broadcast && self.user() == STATUS_BROADCAST_USER
    }

    /// The system/announcements account (`0@s.whatsapp.net` / `0@c.us`).
    /// It never answers user-directed IQs, so requests must be short-circuited
    /// client-side (WA Web excludes it via `isPSA` before hitting the server).
    fn is_psa(&self) -> bool {
        matches!(self.server(), Server::Pn | Server::Legacy) && self.user() == PSA_USER
    }

    fn is_bot(&self) -> bool {
        (self.server() == Server::Pn
            && self.device() == 0
            && (self.user().starts_with("1313555") || self.user().starts_with("131655500")))
            || self.server() == Server::Bot
    }

    fn is_newsletter(&self) -> bool {
        self.server() == Server::Newsletter
    }

    /// Returns true if this is a hosted/Cloud API device.
    /// Hosted devices have device ID 99 or use @hosted/@hosted.lid server.
    /// These devices should be excluded from group message fanout.
    fn is_hosted(&self) -> bool {
        self.device() == 99 || matches!(self.server(), Server::Hosted | Server::HostedLid)
    }

    fn is_empty(&self) -> bool {
        self.user().is_empty()
    }

    fn is_same_user_as(&self, other: &impl JidExt) -> bool {
        self.user() == other.user()
    }
}

/// The part of `agent` that is actually part of a JID's identity.
///
/// `agent` is only meaningful where the server renders it. On Pn/Lid/Hosted/
/// HostedLid the wire spells the server as a domain byte instead, so nothing
/// encodes an agent set there (`server_to_domain_type`), nothing prints it
/// (`renders_agent`), and nothing hashes it (`push_phash_form_to` writes a literal `0`,
/// matching WA Web). Two JIDs differing only there address the same device.
///
/// Letting it into equality anyway is what made a JID decoded from the wire
/// unequal to the same JID read back from the store, which holds JIDs as text
/// (see `read_ad_jid`). Equality and `Hash` both go through here so they cannot
/// disagree, and `sort_dedup_by_device` keys on the same rule so the fan-out
/// cannot treat one device as two.
///
/// `integrator` is deliberately NOT normalised here. It is only ever non-zero on
/// interop, but `is_same_chat_as` and `jids_share_user_identity` compare it
/// unconditionally — folding it in here would make `==` disagree with them.
#[inline]
fn identity_agent(server: Server, agent: u8) -> u8 {
    if server.renders_agent() { agent } else { 0 }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default)]
pub struct Jid {
    /// Inline while it fits in `size_of::<String>()`: 24 bytes on a 64-bit host,
    /// 12 on the wasm32/32-bit builds. Phone, LID and modern group users clear
    /// the wide budget, not the narrow one; a legacy `<creator>-<timestamp>`
    /// group id clears neither. See `tests/jid_identity_alloc`.
    pub user: CompactString,
    pub server: Server,
    pub agent: u8,
    pub device: u16,
    pub integrator: u16,
}

#[derive(Debug, Clone, yoke::Yokeable)]
pub struct JidRef<'a> {
    pub user: NodeStr<'a>,
    pub server: Server,
    pub agent: u8,
    pub device: u16,
    pub integrator: u16,
}

impl JidExt for Jid {
    fn user(&self) -> &str {
        &self.user
    }
    fn server(&self) -> Server {
        self.server
    }
    fn device(&self) -> u16 {
        self.device
    }
    fn integrator(&self) -> u16 {
        self.integrator
    }
}

impl Jid {
    pub fn new(user: impl Into<CompactString>, server: Server) -> Self {
        Self {
            user: user.into(),
            server,
            ..Default::default()
        }
    }

    /// Create a phone number JID (s.whatsapp.net)
    pub fn pn(user: impl Into<CompactString>) -> Self {
        Self {
            user: user.into(),
            server: Server::Pn,
            ..Default::default()
        }
    }

    /// Create a LID JID (lid server)
    pub fn lid(user: impl Into<CompactString>) -> Self {
        Self {
            user: user.into(),
            server: Server::Lid,
            ..Default::default()
        }
    }

    /// Creates the `status@broadcast` JID used for status/story updates.
    pub fn status_broadcast() -> Self {
        Self {
            user: CompactString::from(STATUS_BROADCAST_USER),
            server: Server::Broadcast,
            agent: 0,
            device: 0,
            integrator: 0,
        }
    }

    /// Create a group JID (g.us).
    pub fn group(id: impl Into<CompactString>) -> Self {
        Self {
            user: id.into(),
            server: Server::Group,
            ..Default::default()
        }
    }

    /// Create a newsletter (channel) JID (newsletter server).
    pub fn newsletter(id: impl Into<CompactString>) -> Self {
        Self {
            user: id.into(),
            server: Server::Newsletter,
            ..Default::default()
        }
    }

    /// Create a phone number JID with device ID
    pub fn pn_device(user: impl Into<CompactString>, device: u16) -> Self {
        Self {
            user: user.into(),
            server: Server::Pn,
            device,
            ..Default::default()
        }
    }

    /// Create a LID JID with device ID
    pub fn lid_device(user: impl Into<CompactString>, device: u16) -> Self {
        Self {
            user: user.into(),
            server: Server::Lid,
            device,
            ..Default::default()
        }
    }

    /// Returns true if this is a Phone Number based JID (s.whatsapp.net)
    #[inline]
    pub fn is_pn(&self) -> bool {
        self.server == Server::Pn
    }

    /// Returns true if this is a LID based JID
    #[inline]
    pub fn is_lid(&self) -> bool {
        self.server == Server::Lid
    }

    /// Returns the user part without the device ID suffix (e.g., "123:4" -> "123")
    #[inline]
    pub fn user_base(&self) -> &str {
        if let Some((base, _)) = self.user.split_once(':') {
            base
        } else {
            &self.user
        }
    }

    /// Helper to construct a specific device JID from this one
    pub fn with_device(&self, device_id: u16) -> Self {
        Self {
            user: self.user.clone(),
            server: self.server,
            agent: self.agent,
            device: device_id,
            integrator: self.integrator,
        }
    }

    /// Construct a device JID and select the hosted variant of PN/LID when
    /// indicated by a device-list entry.
    ///
    /// Non-user namespaces are left unchanged because they have no hosted
    /// counterpart on the wire.
    pub fn with_device_hosting(&self, device_id: u16, is_hosted: bool) -> Self {
        let mut jid = self.with_device(device_id);
        jid.server = match (jid.server, is_hosted) {
            (Server::Pn | Server::Hosted, true) => Server::Hosted,
            (Server::Pn | Server::Hosted, false) => Server::Pn,
            (Server::Lid | Server::HostedLid, true) => Server::HostedLid,
            (Server::Lid | Server::HostedLid, false) => Server::Lid,
            (server, _) => server,
        };
        jid
    }

    pub fn to_non_ad(&self) -> Self {
        Self {
            user: self.user.clone(),
            server: self.server,
            integrator: self.integrator,
            ..Default::default()
        }
    }

    /// Consuming `to_non_ad`: reuses the owned `user` instead of cloning it.
    /// Prefer this when the receiver is a throwaway owned `Jid`.
    pub fn into_non_ad(self) -> Self {
        Self {
            user: self.user,
            server: self.server,
            integrator: self.integrator,
            agent: 0,
            device: 0,
        }
    }

    /// Device-insensitive "same chat" check: two JIDs address the same chat when
    /// they render to the same string ignoring the multi-device `device`. Compares
    /// `user`, `server`, `integrator`, and `agent` only where the server renders
    /// the agent (`@bot`/`@interop`); Pn/Lid/Hosted suppress it in display, so a
    /// stray decoded agent byte there must not split the chat. Allocates nothing.
    /// Stricter than `is_same_user_as`, which ignores `server`.
    #[inline]
    pub fn is_same_chat_as(&self, other: &Jid) -> bool {
        self.user == other.user
            && self.server == other.server
            && self.integrator == other.integrator
            && (!self.server.renders_agent() || self.agent == other.agent)
    }

    /// Canonical non-AD string form (`user@server`, device + agent stripped)
    /// in a single allocation. Equivalent to `to_non_ad().to_string()` but
    /// skips the throwaway intermediate `Jid` and its `CompactString` clone.
    pub fn to_non_ad_string(&self) -> String {
        let mut buf = String::with_capacity(self.user.len() + 1 + self.server.as_str().len());
        push_jid_to_string(&self.user, self.server, 0, 0, &mut buf);
        buf
    }

    /// [`Self::to_non_ad_string`] as a shareable `Arc<str>`, in exactly one
    /// allocation. Going through the `String` first costs two — the buffer, then
    /// the `Arc<str>` its bytes are copied into — and the message-secret rows
    /// build two of these per message.
    pub fn to_non_ad_arc_str(&self) -> std::sync::Arc<str> {
        let mut writer = JidStackWriter::new();
        if write_jid_fallible(&mut writer, &self.user, self.server, 0, 0).is_ok() {
            return std::sync::Arc::from(writer.as_str());
        }
        // A user part too long for the stack buffer (never seen on the wire)
        // still renders, just back through the heap.
        std::sync::Arc::from(self.to_non_ad_string())
    }

    /// Check if this JID matches the user or their LID.
    /// Useful for checking if a participant is "us" in group messages.
    #[inline]
    pub fn matches_user_or_lid(&self, user: &Jid, lid: Option<&Jid>) -> bool {
        self.is_same_user_as(user) || lid.is_some_and(|l| self.is_same_user_as(l))
    }

    /// See [`Jid::push_phash_form_to`]. Not a general JID rendering — use
    /// `Display`/`to_string` for that.
    pub fn to_phash_form_string(&self) -> String {
        let mut s = String::with_capacity(self.user.len() + 20);
        self.push_phash_form_to(&mut s);
        s
    }

    /// Append the form the participant hash is computed over
    /// (`user.0:device@server`) to `buf`, for callers that batch many JIDs into
    /// one shared buffer instead of paying a heap `String` per JID (see
    /// `participant_list_hash`).
    ///
    /// **This is not a general-purpose JID rendering.** The agent position is
    /// the literal `0`, never `self.agent`, and that is deliberate: WA Web's
    /// `formatFull` spelling hardcodes `".0"` unconditionally — there is no
    /// per-server carve-out, and `WAWebWid` has no agent field to read one from.
    /// The server recomputes this exact string to validate the phash, so writing
    /// our agent would mean a rejected hash for any JID that carried one.
    ///
    /// If you want the JID as it is addressed, including the agent on the servers
    /// that render it, use `Display` / [`Jid::push_to`] instead.
    #[inline]
    pub fn push_phash_form_to(&self, buf: &mut String) {
        if self.user.is_empty() {
            buf.push_str(self.server.as_str());
            return;
        }
        buf.push_str(&self.user);
        buf.push_str(".0:");
        buf.push_str(itoa::Buffer::new().format(self.device));
        buf.push('@');
        buf.push_str(self.server.as_str());
    }

    /// Append the Display representation to `buf` using direct push operations,
    /// bypassing `fmt::Display` and `dyn Write` dispatch.
    #[inline]
    pub fn push_to(&self, buf: &mut String) {
        push_jid_to_string(&self.user, self.server, self.agent, self.device, buf);
    }

    /// Write the Display representation to any [`fmt::Write`] sink without an
    /// intermediate `String`.
    #[inline]
    pub fn write_display_to<W: fmt::Write + ?Sized>(&self, writer: &mut W) -> fmt::Result {
        write_jid_fallible(writer, &self.user, self.server, self.agent, self.device)
    }

    /// Compare the display representation with `other` without allocating.
    #[inline]
    pub fn display_eq(&self, other: &str) -> bool {
        jid_display_eq(&self.user, self.server, self.agent, self.device, other)
    }

    /// Compare two JIDs by the representation emitted by [`fmt::Display`].
    #[inline]
    pub fn display_eq_jid(&self, other: &Self) -> bool {
        jid_displays_equal(
            (&self.user, self.server, self.agent, self.device),
            (&other.user, other.server, other.agent, other.device),
        )
    }

    /// Compare device identity (user, server, device) without allocation.
    #[inline]
    pub fn device_eq(&self, other: &Jid) -> bool {
        self.user == other.user && self.server == other.server && self.device == other.device
    }

    /// The `agent` as far as identity is concerned: the field itself where the
    /// server renders it (`@bot`, `@interop`), `0` where it does not.
    ///
    /// Exposed so callers that build their own key over a JID — sorting,
    /// deduplicating, indexing — can key on the same rule `==` and `Hash` use
    /// instead of on the raw field, which would split one device in two or, in
    /// reverse, merge two real ones.
    #[inline]
    pub fn identity_agent(&self) -> u8 {
        identity_agent(self.server, self.agent)
    }

    /// Get a borrowing key for O(1) HashSet lookups by device identity.
    #[inline]
    pub fn device_key(&self) -> DeviceKey<'_> {
        DeviceKey {
            user: &self.user,
            server: self.server,
            device: self.device,
        }
    }
}

/// Borrowing key for device identity (user, server, device). Use with HashSet for O(1) lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceKey<'a> {
    pub user: &'a str,
    pub server: Server,
    pub device: u16,
}

impl<'a> JidExt for JidRef<'a> {
    fn user(&self) -> &str {
        &self.user
    }
    fn server(&self) -> Server {
        self.server
    }
    fn device(&self) -> u16 {
        self.device
    }
    fn integrator(&self) -> u16 {
        self.integrator
    }
}

impl<'a> JidRef<'a> {
    pub fn to_owned(&self) -> Jid {
        Jid {
            user: self.user.to_compact_string(),
            server: self.server,
            agent: self.agent,
            device: self.device,
            integrator: self.integrator,
        }
    }

    /// Compare the display representation with `other` without allocating.
    #[inline]
    pub fn display_eq(&self, other: &str) -> bool {
        jid_display_eq(&self.user, self.server, self.agent, self.device, other)
    }

    /// Compare two borrowed JIDs by the representation emitted by [`fmt::Display`].
    #[inline]
    pub fn display_eq_jid(&self, other: &Self) -> bool {
        jid_displays_equal(
            (&self.user, self.server, self.agent, self.device),
            (&other.user, other.server, other.agent, other.device),
        )
    }
}

impl PartialEq for Jid {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.user == other.user
            && self.server == other.server
            && self.device == other.device
            && self.integrator == other.integrator
            // Equal raw agents are already equal identity agents — the servers
            // match by the check above, so both sides normalise the same way.
            // Skipping the lookup in that case is worth it because it is the
            // overwhelmingly common one: nothing off the wire carries an agent
            // on the AD servers. Load-bearing on the `self.server == other.server`
            // above; reordering these would make the shortcut wrong.
            && (self.agent == other.agent
                || identity_agent(self.server, self.agent)
                    == identity_agent(other.server, other.agent))
    }
}

impl Eq for Jid {}

impl std::hash::Hash for Jid {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.user.hash(state);
        self.server.hash(state);
        self.device.hash(state);
        self.integrator.hash(state);
        identity_agent(self.server, self.agent).hash(state);
    }
}

impl PartialEq for JidRef<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.user.as_ref() == other.user.as_ref()
            && self.server == other.server
            && self.device == other.device
            && self.integrator == other.integrator
            // See `PartialEq for Jid` for why the raw compare can short-circuit.
            && (self.agent == other.agent
                || identity_agent(self.server, self.agent)
                    == identity_agent(other.server, other.agent))
    }
}

impl Eq for JidRef<'_> {}

impl std::hash::Hash for JidRef<'_> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.user.as_ref().hash(state);
        self.server.hash(state);
        self.device.hash(state);
        self.integrator.hash(state);
        identity_agent(self.server, self.agent).hash(state);
    }
}

impl PartialEq<JidRef<'_>> for Jid {
    #[inline]
    fn eq(&self, other: &JidRef<'_>) -> bool {
        self.user.as_str() == other.user.as_ref()
            && self.server == other.server
            && self.device == other.device
            && self.integrator == other.integrator
            // See `PartialEq for Jid`.
            && (self.agent == other.agent
                || identity_agent(self.server, self.agent)
                    == identity_agent(other.server, other.agent))
    }
}

impl PartialEq<Jid> for JidRef<'_> {
    #[inline]
    fn eq(&self, other: &Jid) -> bool {
        other == self
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for JidRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Jid", 5)?;
        s.serialize_field("user", &*self.user)?;
        s.serialize_field("server", &self.server)?;
        s.serialize_field("agent", &self.agent)?;
        s.serialize_field("device", &self.device)?;
        s.serialize_field("integrator", &self.integrator)?;
        s.end()
    }
}

impl FromStr for Jid {
    type Err = JidError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Try fast path first for well-formed JIDs
        if let Some(jid) = parse_jid_ref(s) {
            return Ok(jid.to_owned());
        }

        // Fallback to original parsing for edge cases and validation
        // Keep server as &str to avoid allocation until we need it
        let (user_part, server) = match s.split_once('@') {
            Some((u, s)) => (u, s),
            None => ("", s),
        };

        if user_part.is_empty() && Server::try_from(server).is_err() {
            return Err(JidError::InvalidFormat(format!(
                "unknown server '{server}'"
            )));
        }

        // Special handling for LID JIDs, as their user part can contain dots
        // that should not be interpreted as agent separators.
        if server == HIDDEN_USER_SERVER {
            let (user, device) = if let Some((u, d_str)) = user_part.rsplit_once(':') {
                (u, d_str.parse()?)
            } else {
                (user_part, 0)
            };
            return Ok(Jid {
                user: CompactString::from(user),
                server: Server::try_from(server)?,
                device,
                agent: 0,
                integrator: 0,
            });
        }

        // Fallback to existing logic for other JID types (s.whatsapp.net, etc.)
        let mut user = user_part;
        let mut device = 0;
        let mut agent = 0;

        if let Some((u, d_str)) = user_part.rsplit_once(':') {
            user = u;
            device = d_str.parse()?;
        }

        if server != DEFAULT_USER_SERVER
            && server != HIDDEN_USER_SERVER
            && let Some((u, last_part)) = user.rsplit_once('.')
            && let Ok(num_val) = last_part.parse::<u16>()
        {
            if num_val > u8::MAX as u16 {
                return Err(JidError::InvalidFormat(format!(
                    "Agent component out of range: {num_val}"
                )));
            }
            user = u;
            agent = num_val as u8;
        }

        Ok(Jid {
            user: CompactString::from(user),
            server: Server::try_from(server)?,
            agent,
            device,
            integrator: 0,
        })
    }
}

/// Core JID formatting logic used by `fmt::Display`, `push_jid_to_string`, and
/// `push_jid_to_compact`. Writes `{user}[.{agent}][:{device}]@{server}`.
///
/// Two flavors via `$append`:
/// - **fallible** (`f.write_str(s)?`): for `fmt::Formatter` which returns `fmt::Result`
/// - **infallible** (`$buf.push_str(s)`): for `String`/`CompactString`
macro_rules! write_jid {
    // Infallible variant: push_str/push into a growable buffer
    (infallible $buf:expr, $user:expr, $server:expr, $agent:expr, $device:expr) => {{
        let (user, server, agent, device) = ($user, $server, $agent, $device);
        if user.is_empty() {
            $buf.push_str(server.as_str());
            return;
        }
        $buf.push_str(user);
        if agent > 0 && server.renders_agent() {
            $buf.push('.');
            $buf.push_str(itoa::Buffer::new().format(agent));
        }
        if device > 0 {
            $buf.push(':');
            $buf.push_str(itoa::Buffer::new().format(device));
        }
        $buf.push('@');
        $buf.push_str(server.as_str());
    }};
    // Fallible variant: write_str into fmt::Formatter
    (fallible $f:expr, $user:expr, $server:expr, $agent:expr, $device:expr) => {{
        let (user, server, agent, device) = ($user, $server, $agent, $device);
        if user.is_empty() {
            return $f.write_str(server.as_str());
        }
        $f.write_str(user)?;
        if agent > 0 && server.renders_agent() {
            $f.write_str(".")?;
            $f.write_str(itoa::Buffer::new().format(agent))?;
        }
        if device > 0 {
            $f.write_str(":")?;
            $f.write_str(itoa::Buffer::new().format(device))?;
        }
        $f.write_str("@")?;
        $f.write_str(server.as_str())
    }};
}

/// Write the JID display representation directly into a `String`,
/// bypassing `fmt::Display` and `dyn Write` dispatch entirely.
#[inline]
pub fn push_jid_to_string(user: &str, server: Server, agent: u8, device: u16, buf: &mut String) {
    write_jid!(infallible buf, user, server, agent, device);
}

/// Write the JID display representation directly into a `CompactString`,
/// bypassing `fmt::Display` and `dyn Write` dispatch entirely.
#[inline]
pub fn push_jid_to_compact(
    user: &str,
    server: Server,
    agent: u8,
    device: u16,
    buf: &mut CompactString,
) {
    write_jid!(infallible buf, user, server, agent, device);
}

/// Stack writer sized for any realistic JID, so `Display` can emit a single
/// `write_str`: a `ToString`-backed `String` then reserves once at the exact
/// length instead of reallocating per fragment. Overflow errors out and the
/// caller falls back to direct fragment writes.
struct JidStackWriter {
    buf: [u8; 64],
    len: usize,
}

impl JidStackWriter {
    #[inline]
    fn new() -> Self {
        Self {
            buf: [0; 64],
            len: 0,
        }
    }

    #[inline]
    fn as_str(&self) -> &str {
        // SAFETY: `write_str` below is the only writer, and it is all-or-nothing
        // — it returns `Err` before copying anything that would not fit, so it
        // can never leave a partial code point behind. `buf[..len]` is therefore
        // always a concatenation of whole `&str` fragments. Anything that writes
        // raw bytes here instead of going through `write_str` breaks this.
        unsafe { std::str::from_utf8_unchecked(&self.buf[..self.len]) }
    }
}

impl fmt::Write for JidStackWriter {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        if end > self.buf.len() {
            return Err(fmt::Error);
        }
        self.buf[self.len..end].copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[inline]
fn write_jid_fallible<W: fmt::Write + ?Sized>(
    w: &mut W,
    user: &str,
    server: Server,
    agent: u8,
    device: u16,
) -> fmt::Result {
    write_jid!(fallible w, user, server, agent, device)
}

struct StrEqWriter<'a> {
    target: &'a [u8],
    position: usize,
    matches: bool,
}

impl fmt::Write for StrEqWriter<'_> {
    #[inline]
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.matches {
            let bytes = value.as_bytes();
            let end = self.position + bytes.len();
            if end > self.target.len() || self.target[self.position..end] != *bytes {
                self.matches = false;
            }
            self.position = end;
        }
        Ok(())
    }
}

#[inline]
fn jid_display_eq(user: &str, server: Server, agent: u8, device: u16, other: &str) -> bool {
    let mut writer = StrEqWriter {
        target: other.as_bytes(),
        position: 0,
        matches: true,
    };
    let written = write_jid_fallible(&mut writer, user, server, agent, device).is_ok();
    written && writer.matches && writer.position == other.len()
}

#[inline]
fn jid_displays_equal(left: (&str, Server, u8, u16), right: (&str, Server, u8, u16)) -> bool {
    let (left_user, left_server, left_agent, left_device) = left;
    let (right_user, right_server, right_agent, right_device) = right;
    if left_user.is_empty() || right_user.is_empty() {
        return left_user.is_empty() && right_user.is_empty() && left_server == right_server;
    }

    left_user == right_user
        && left_server == right_server
        && left_device == right_device
        && (!left_server.renders_agent() || left_agent == right_agent)
}

impl fmt::Display for Jid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut w = JidStackWriter::new();
        if self.write_display_to(&mut w).is_ok() {
            return f.write_str(w.as_str());
        }
        self.write_display_to(f)
    }
}

/// Privacy-aware [`Display`](fmt::Display) wrapper for a [`Jid`], for use in tracing fields.
///
/// Pseudonymous (LID) and non-personal (newsletter/bot/call, modern group ids)
/// JIDs render in full, so the same peer/chat correlates across spans. JIDs whose
/// `user` is a phone number ([`Server::carries_phone_number`]) render the user as
/// a `pn#<token>` instead — preserving correlation without leaking the number.
/// Legacy group/broadcast ids of the form `<creator-phone>-<timestamp>` get only
/// the numeric prefix redacted (`pn#<token>-<timestamp>`), keeping the timestamp.
///
/// The token is a keyed hash (SipHash via a process-lifetime random key), not a
/// plain digest of the number: the phone-number search space is small, so an
/// unkeyed hash would be reversible by precomputation. The random key lives only
/// in process memory, so exported traces cannot be brute-forced back to numbers.
/// It is stable within a process run (correlation works) but not across restarts
/// (a fresh key each start). Enable the `tracing-pii` feature to render raw
/// numbers (local debugging only). The token is computed only while formatting an
/// already-enabled span, so it costs nothing on disabled call sites.
pub struct ObservedJid<'a>(&'a Jid);

impl Jid {
    /// Privacy-aware display for tracing spans/fields. See [`ObservedJid`].
    #[inline]
    pub fn observe(&self) -> ObservedJid<'_> {
        ObservedJid(self)
    }
}

/// Per-process keyed token for a sensitive string: a SipHash with a random key
/// created once per process. Stable within a run (so the same value correlates
/// across spans) but not precomputable from the input, so exported traces cannot
/// be brute-forced back to the original (e.g. an E.164 phone number). Public so
/// other layers can redact non-`Jid` identifiers (e.g. a Signal `ProtocolAddress`
/// name, which embeds a phone number) with the same keyed scheme.
pub fn observe_token(s: &str) -> u64 {
    use std::hash::BuildHasher;
    static KEY: std::sync::OnceLock<std::collections::hash_map::RandomState> =
        std::sync::OnceLock::new();
    KEY.get_or_init(std::collections::hash_map::RandomState::new)
        .hash_one(s)
}

/// Privacy-aware redaction of a JID supplied as a string (e.g. a group jid `&str`
/// in a span field). Parses it and applies [`Jid::observe`]; if it does not parse,
/// falls back to a keyed token so a raw number can never leak. Honors `tracing-pii`.
pub fn observe_str(s: &str) -> String {
    if cfg!(feature = "tracing-pii") {
        return s.to_string();
    }
    match s.parse::<Jid>() {
        Ok(jid) => jid.observe().to_string(),
        Err(_) => format!("?#{:016x}", observe_token(s)),
    }
}

impl fmt::Display for ObservedJid<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let jid = self.0;
        if jid.user.is_empty() || cfg!(feature = "tracing-pii") {
            return fmt::Display::fmt(jid, f);
        }
        // Decide the privacy-safe `user` rendering.
        let redacted: Option<String> = if jid.server.carries_phone_number() {
            // The whole user is a phone number.
            Some(format!("pn#{:016x}", observe_token(jid.user.as_str())))
        } else if matches!(jid.server, Server::Group | Server::Broadcast) {
            // Legacy group/broadcast ids embed the creator phone as
            // "<phone>-<timestamp>". Redact the numeric prefix and keep the
            // timestamp (not PII) so the group still correlates across spans.
            match jid.user.find('-') {
                Some(i) if i > 0 && jid.user.as_bytes()[..i].iter().all(|b| b.is_ascii_digit()) => {
                    Some(format!(
                        "pn#{:016x}{}",
                        observe_token(&jid.user[..i]),
                        &jid.user[i..]
                    ))
                }
                _ => None,
            }
        } else {
            None
        };
        let Some(user) = redacted else {
            // Pseudonymous (LID) or non-personal: safe to render in full.
            return fmt::Display::fmt(jid, f);
        };
        f.write_str(&user)?;
        // Preserve agent where it is part of the identity (e.g. Interop/Messenger),
        // mirroring the normal JID display so distinct IDs stay distinct.
        if jid.agent > 0 && jid.server.renders_agent() {
            write!(f, ".{}", jid.agent)?;
        }
        if jid.device > 0 {
            write!(f, ":{}", jid.device)?;
        }
        write!(f, "@{}", jid.server.as_str())
    }
}

impl<'a> fmt::Display for JidRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut w = JidStackWriter::new();
        if write_jid_fallible(&mut w, &self.user, self.server, self.agent, self.device).is_ok() {
            return f.write_str(w.as_str());
        }
        write_jid_fallible(f, &self.user, self.server, self.agent, self.device)
    }
}

impl From<Jid> for String {
    fn from(jid: Jid) -> Self {
        jid.to_string()
    }
}

/// Lets `impl Into<Jid>` APIs accept `&Jid` transparently: borrow-callers pay
/// one cheap clone (the user part is inline for typical numeric ids), owned
/// callers move for free.
impl From<&Jid> for Jid {
    fn from(jid: &Jid) -> Self {
        jid.clone()
    }
}

impl<'a> From<JidRef<'a>> for String {
    fn from(jid: JidRef<'a>) -> Self {
        jid.to_string()
    }
}

impl TryFrom<String> for Jid {
    type Error = JidError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Jid::from_str(&value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn is_psa_matches_system_jid_in_both_user_namespaces() {
        assert!(Jid::from_str("0@s.whatsapp.net").unwrap().is_psa());
        assert!(Jid::from_str("0@c.us").unwrap().is_psa());
        assert!(parse_jid_ref("0@s.whatsapp.net").unwrap().is_psa());
    }

    #[test]
    fn is_psa_rejects_regular_and_non_pn_jids() {
        assert!(!Jid::from_str("10@s.whatsapp.net").unwrap().is_psa());
        assert!(!Jid::from_str("0@lid").unwrap().is_psa());
        assert!(!Jid::from_str("status@broadcast").unwrap().is_psa());
        assert!(!Jid::from_str("0@g.us").unwrap().is_psa());
    }

    /// `JidStackWriter::as_str` reads its buffer back as UTF-8 without
    /// validating it, so every shape that reaches it has to be exercised
    /// somewhere Miri can see: multi-byte users, a user that fills the buffer
    /// exactly, and one that overflows it into the fallback path.
    #[test]
    fn display_reads_back_every_stack_buffer_shape() {
        // Multi-byte code points in the user part: a byte-wise buffer must not
        // be able to split one, and the readback must reproduce them exactly.
        let emoji = Jid::new("héllo→世界", Server::Lid);
        assert_eq!(emoji.to_string(), "héllo→世界@lid");
        assert_eq!(format!("{}", emoji.observe()), "héllo→世界@lid");

        // Exactly at the 64-byte stack buffer: "@lid" is 4 bytes, so a 60-byte
        // user is the longest that still renders through it.
        let full = Jid::new("9".repeat(60), Server::Lid);
        let rendered = full.to_string();
        assert_eq!(rendered.len(), 64);
        assert!(rendered.ends_with("@lid"));

        // One byte past it: `write_str` errors and Display falls back to
        // fragment writes, which must produce the same string.
        let over = Jid::new("9".repeat(61), Server::Lid);
        assert_eq!(over.to_string(), format!("{}@lid", "9".repeat(61)));

        // The borrowed formatter and the Arc<str> builder share the writer.
        let borrowed = parse_jid_ref("12025550111:7@s.whatsapp.net").unwrap();
        assert_eq!(borrowed.to_string(), "12025550111:7@s.whatsapp.net");
        assert_eq!(&*emoji.to_non_ad_arc_str(), "héllo→世界@lid");
        assert_eq!(
            &*over.to_non_ad_arc_str(),
            &*format!("{}@lid", "9".repeat(61))
        );
    }

    /// The phash form is recomputed and validated by the server, so
    /// it has to match WA Web byte for byte. WA Web writes a literal `.0` in the
    /// agent position (`formatFull`) and its Wid carries no agent at all, so ours
    /// must not leak one in either — and two JIDs that compare equal must produce
    /// the same string, or the phash memo (keyed by JID) can serve a hash computed
    /// for a different one.
    #[test]
    fn phash_form_writes_the_agent_position_as_zero_like_wa_web() {
        let plain = Jid {
            user: "5511999998888".into(),
            server: Server::Lid,
            agent: 0,
            device: 3,
            integrator: 0,
        };
        assert_eq!(plain.to_phash_form_string(), "5511999998888.0:3@lid");

        let with_agent = Jid {
            agent: 7,
            ..plain.clone()
        };
        assert_eq!(
            with_agent.to_phash_form_string(),
            "5511999998888.0:3@lid",
            "the agent must not reach the hashed string"
        );

        // The pairing the phash memo relies on: equal JIDs, equal AD strings.
        assert_eq!(plain, with_agent);
        assert_eq!(
            plain.to_phash_form_string(),
            with_agent.to_phash_form_string()
        );

        // A server-only JID still degenerates to the server, and device still counts.
        assert_eq!(
            Jid::new("", Server::Pn).to_phash_form_string(),
            "s.whatsapp.net"
        );
        assert_ne!(
            plain.to_phash_form_string(),
            Jid {
                device: 4,
                ..plain.clone()
            }
            .to_phash_form_string()
        );
    }

    /// An `agent` off an agent-rendering server is inert: nothing encodes it,
    /// prints it, or hashes it. Two JIDs differing only there address the same
    /// device, so equality and `Hash` must both say so — and must agree with each
    /// other, or a `HashMap<Jid, _>` gets an entry it can never look up again.
    ///
    /// `integrator` stays in identity: it is only non-zero on interop, but
    /// `is_same_chat_as` compares it unconditionally, and `==` must not disagree.
    #[test]
    fn inert_agent_stays_out_of_identity() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        fn hash_of(jid: &Jid) -> u64 {
            let mut h = DefaultHasher::new();
            jid.hash(&mut h);
            h.finish()
        }

        // The four AD servers spell the server as a domain byte, so an agent set
        // on one is a wire artefact, not identity.
        for server in [Server::Pn, Server::Lid, Server::Hosted, Server::HostedLid] {
            let clean = Jid {
                user: "123456789012345".into(),
                server,
                agent: 0,
                device: 7,
                integrator: 0,
            };
            let with_agent = Jid {
                agent: 1,
                ..clean.clone()
            };
            assert_eq!(
                clean, with_agent,
                "{server:?}: agent must not split identity"
            );
            assert_eq!(
                hash_of(&clean),
                hash_of(&with_agent),
                "{server:?}: Hash must agree with Eq"
            );

            // integrator is NOT normalised: `is_same_chat_as` compares it always,
            // and `==` must not disagree with it.
            let with_integrator = Jid {
                integrator: 0xBEEF,
                ..clean.clone()
            };
            assert_ne!(
                clean, with_integrator,
                "{server:?}: integrator stays in identity, matching is_same_chat_as"
            );
            assert_eq!(
                clean.is_same_chat_as(&with_integrator),
                clean == with_integrator,
                "{server:?}: == must agree with is_same_chat_as"
            );

            // The borrowed form and the cross-type comparison follow the same rule.
            let borrowed = JidRef {
                user: NodeStr::Borrowed("123456789012345"),
                server,
                agent: 1,
                device: 7,
                integrator: 0,
            };
            assert_eq!(clean, borrowed, "{server:?}: owned == borrowed");
            assert_eq!(borrowed, clean, "{server:?}: borrowed == owned");
        }

        // Where the server DOES render the agent it is identity, and must still split.
        let bot = Jid {
            user: "123456789".into(),
            server: Server::Interop,
            agent: 4,
            device: 0,
            integrator: 0,
        };
        let other_agent = Jid {
            agent: 5,
            ..bot.clone()
        };
        assert_ne!(
            bot, other_agent,
            "interop renders the agent, so it is identity"
        );
        assert_ne!(
            bot,
            Jid {
                integrator: 9,
                ..bot.clone()
            },
            "interop is where integrator is real"
        );

        // Fields that are always identity keep splitting.
        let pn = Jid::new("123456789012345", Server::Pn);
        assert_ne!(
            pn,
            Jid {
                device: 1,
                ..pn.clone()
            }
        );
        assert_ne!(pn, Jid::new("123456789012346", Server::Pn));
        assert_ne!(pn, Jid::new("123456789012345", Server::Lid));
    }

    #[test]
    fn display_eq_matches_owned_and_borrowed_jids_without_normalizing() {
        let canonical = "123456789.4:17@interop";
        let owned = Jid::from_str(canonical).unwrap();
        let borrowed = parse_jid_ref(canonical).unwrap();

        for value in [canonical, "123456789.4:16@interop", "123456789@interop", ""] {
            assert_eq!(owned.display_eq(value), value == canonical);
            assert_eq!(borrowed.display_eq(value), value == canonical);
        }

        let long_user = "a".repeat(128);
        let long_value = format!("{long_user}@lid");
        let long_jid = Jid::lid(long_user);
        assert!(long_jid.display_eq(&long_value));
        assert!(!long_jid.display_eq(&format!("{long_value}x")));
    }

    #[test]
    fn display_eq_jid_uses_only_rendered_components() {
        let pn = Jid {
            user: "12025550111".into(),
            server: Server::Pn,
            agent: 1,
            device: 7,
            integrator: 3,
        };
        let pn_same_display = Jid {
            agent: 2,
            integrator: 9,
            ..pn.clone()
        };
        assert_eq!(pn.to_string(), pn_same_display.to_string());
        assert!(pn.display_eq_jid(&pn_same_display));

        let pn_other_device = Jid {
            device: 8,
            ..pn.clone()
        };
        assert!(!pn.display_eq_jid(&pn_other_device));

        let bot = Jid {
            user: "13136555001".into(),
            server: Server::Bot,
            agent: 1,
            device: 0,
            integrator: 0,
        };
        let other_bot_agent = Jid {
            agent: 2,
            ..bot.clone()
        };
        assert!(!bot.display_eq_jid(&other_bot_agent));

        let server_only = Jid {
            user: "".into(),
            server: Server::Pn,
            agent: 1,
            device: 7,
            integrator: 3,
        };
        let same_server_only = Jid {
            agent: 2,
            device: 9,
            integrator: 4,
            ..server_only.clone()
        };
        assert_eq!(server_only.to_string(), same_server_only.to_string());
        assert!(server_only.display_eq_jid(&same_server_only));

        let borrowed = JidRef {
            user: NodeStr::Borrowed("12025550111"),
            server: Server::Pn,
            agent: 1,
            device: 7,
            integrator: 0,
        };
        let borrowed_same_display = JidRef {
            agent: 2,
            ..borrowed.clone()
        };
        assert!(borrowed.display_eq_jid(&borrowed_same_display));
    }

    #[test]
    fn owned_and_borrowed_jids_compare_without_conversion() {
        let owned = Jid {
            user: "12025550111".into(),
            server: Server::Pn,
            agent: 0,
            device: 7,
            integrator: 0,
        };
        let borrowed = JidRef {
            user: NodeStr::Borrowed("12025550111"),
            server: Server::Pn,
            agent: 0,
            device: 7,
            integrator: 0,
        };

        assert_eq!(owned, borrowed);
        assert_eq!(borrowed, owned);

        let other_device = JidRef {
            device: 8,
            ..borrowed.clone()
        };
        assert_ne!(owned, other_device);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn server_deserializes_borrowed_and_owned_strings() {
        let borrowed: Server = serde_json::from_str("\"s.whatsapp.net\"").unwrap();
        let owned: Server =
            serde_json::from_value(serde_json::Value::String("lid".to_owned())).unwrap();

        assert_eq!(borrowed, Server::Pn);
        assert_eq!(owned, Server::Lid);
    }

    /// `observe()` must never leak a raw phone number, must keep pseudonymous /
    /// non-personal JIDs intact for correlation, and must preserve device.
    #[test]
    #[cfg(not(feature = "tracing-pii"))]
    fn observe_redacts_phone_but_not_lid_or_group() {
        let pn = Jid::from_str("5511999998888:7@s.whatsapp.net").unwrap();
        let shown = pn.observe().to_string();
        assert!(shown.starts_with("pn#"), "{shown}");
        assert!(
            !shown.contains("5511999998888"),
            "raw number leaked: {shown}"
        );
        assert!(shown.ends_with(":7@s.whatsapp.net"), "device lost: {shown}");
        // Stable within the process so the same peer correlates across spans.
        assert_eq!(shown, pn.observe().to_string());

        // LID is pseudonymous and modern group ids are non-personal: rendered in full.
        let lid = Jid::from_str("123456789@lid").unwrap();
        assert_eq!(lid.observe().to_string(), lid.to_string());
        let group = Jid::from_str("120363012345678901@g.us").unwrap();
        assert_eq!(group.observe().to_string(), group.to_string());

        // Legacy group id embeds the creator phone ("<phone>-<ts>"): redact the
        // numeric prefix, keep the timestamp.
        let legacy = Jid::from_str("123456789-1620000000@g.us").unwrap();
        let ls = legacy.observe().to_string();
        assert!(
            ls.starts_with("pn#") && ls.ends_with("-1620000000@g.us"),
            "{ls}"
        );
        // Exact no-leak invariant: the creator phone must not appear anywhere.
        assert!(!ls.contains("123456789"), "creator phone leaked: {ls}");
        // The redacted prefix is the fixed-width keyed token, not the raw number.
        let mid = &ls["pn#".len()..ls.find('-').unwrap()];
        assert_eq!(mid.len(), 16, "token width: {ls}");
        assert!(
            mid.bytes().all(|b| b.is_ascii_hexdigit()),
            "token hex: {ls}"
        );
    }

    /// Helper function to test a full parsing and display round-trip.
    fn assert_jid_roundtrip(
        input: &str,
        expected_user: &str,
        expected_server: &str,
        expected_device: u16,
        expected_agent: u8,
    ) {
        assert_jid_parse_and_display(
            input,
            expected_user,
            expected_server,
            expected_device,
            expected_agent,
            input,
        );
    }

    /// Helper function to test parsing and display with a custom expected output.
    fn assert_jid_parse_and_display(
        input: &str,
        expected_user: &str,
        expected_server: &str,
        expected_device: u16,
        expected_agent: u8,
        expected_output: &str,
    ) {
        // 1. Test parsing from string (FromStr trait)
        let jid = Jid::from_str(input).unwrap_or_else(|_| panic!("Failed to parse JID: {}", input));

        assert_eq!(
            jid.user, expected_user,
            "User part did not match for {}",
            input
        );
        assert_eq!(
            jid.server, expected_server,
            "Server part did not match for {}",
            input
        );
        assert_eq!(
            jid.device, expected_device,
            "Device part did not match for {}",
            input
        );
        assert_eq!(
            jid.agent, expected_agent,
            "Agent part did not match for {}",
            input
        );

        // 2. Test formatting back to string (Display trait)
        let formatted = jid.to_string();
        assert_eq!(
            formatted, expected_output,
            "Formatted string did not match expected output for {}",
            input
        );
    }

    #[test]
    fn borrowed_parser_matches_owned_fast_path_and_bot_classification() {
        for raw in [
            "5511999998888:7@s.whatsapp.net",
            "120363012345678901@g.us",
            "123456789012345@lid",
            "assistant@bot",
            "13135551234@s.whatsapp.net",
        ] {
            let borrowed = parse_jid_ref(raw).expect("common JID should stay borrowed");
            let owned = raw.parse::<Jid>().unwrap();

            assert!(matches!(borrowed.user, NodeStr::Borrowed(_)));
            assert_eq!(borrowed.to_owned(), owned);
            assert_eq!(borrowed.is_bot(), owned.is_bot());
        }

        assert!(parse_jid_ref("g.us").is_none(), "server-only uses fallback");
        assert!(parse_jid_ref("user@unknown").is_none());
    }

    /// `parse_u16_decimal` stands in for `u16::from_str` inside the scanner, so
    /// it has to accept and reject exactly what std does — including the shapes
    /// nobody writes on purpose but a malformed stanza can still carry.
    #[test]
    fn decimal_fast_path_matches_u16_from_str() {
        for raw in [
            "", "+", "-", "0", "7", "+7", "007", "33", "255", "256", "65535", "65536", "99999",
            "-0", "-1", "1x", "x1", " 1", "1 ", "1_0", "+-1", "++1", "1.0", "٣", "𝟛",
        ] {
            assert_eq!(
                parse_u16_decimal(raw),
                raw.parse::<u16>().ok(),
                "mismatch for {raw:?}"
            );
        }
    }

    /// The scan stops at the first `@` and dispatches on the resolved `Server`,
    /// so every separator rule it folded together needs a case here: which
    /// separator wins per server, and the pathological users where the agent
    /// rule reads a different dot than the scan recorded.
    ///
    /// Asserted against written-out values rather than against `str::parse`,
    /// which would prove nothing: `FromStr` tries `parse_jid_ref` first, so for
    /// any server this path accepts it would be comparing the scanner with
    /// itself. These are the values `main` produces for the same inputs.
    #[test]
    fn fast_parse_pins_the_separator_rules_per_server() {
        // (input, user, agent, device)
        let cases = [
            // Generic servers read a trailing dotted number as the agent.
            ("123456789.4:17@interop", "123456789", 4u8, 17u16),
            ("123456789.4@interop", "123456789", 4, 0),
            // ...but an agent past u8 leaves the user whole instead.
            ("123.999@interop", "123.999", 0, 0),
            // On s.whatsapp.net that same trailing number is the legacy device.
            ("5511999998888.2@s.whatsapp.net", "5511999998888", 0, 2),
            // Dots in a lid user are part of the user.
            ("12345.678@lid", "12345.678", 0, 0),
            ("12345.678:9@lid", "12345.678", 0, 9),
            // `hosted.lid` and `c.us` are NOT in the lid family here — they take
            // the generic rule, so a dotted number that fits in u8 becomes the
            // agent. Pinned as the pre-existing behaviour, not endorsed as
            // correct; changing it needs WA Web as ground truth, not this PR.
            ("12345.6@hosted.lid", "12345", 6, 0),
            ("12345.678@hosted.lid", "12345.678", 0, 0),
            ("12345.6@c.us", "12345", 6, 0),
            // A second colon puts the last dot after the first one, which is
            // where reusing the scanned dot position would silently diverge.
            ("a:1.5:2@bot", "a:1", 5, 2),
            // Unparsable device degrades to 0 rather than rejecting the JID.
            ("5511999998888:x@s.whatsapp.net", "5511999998888", 0, 0),
        ];

        for (raw, user, agent, device) in cases {
            let fast =
                parse_jid_fast(raw).unwrap_or_else(|| panic!("{raw} should take the fast path"));
            assert_eq!(fast.user, user, "user for {raw}");
            assert_eq!(fast.agent, agent, "agent for {raw}");
            assert_eq!(fast.device, device, "device for {raw}");
        }
    }

    #[test]
    fn test_jid_parsing_and_display_roundtrip() {
        // Standard cases
        assert_jid_roundtrip(
            "1234567890@s.whatsapp.net",
            "1234567890",
            "s.whatsapp.net",
            0,
            0,
        );
        assert_jid_roundtrip(
            "1234567890:15@s.whatsapp.net",
            "1234567890",
            "s.whatsapp.net",
            15,
            0,
        );
        assert_jid_roundtrip("123-456@g.us", "123-456", "g.us", 0, 0);

        // Server-only JID: parsing "s.whatsapp.net" should display as "s.whatsapp.net" (no @ prefix)
        // This matches WhatsApp Web behavior where server-only JIDs don't have @ prefix
        assert_jid_roundtrip("s.whatsapp.net", "", "s.whatsapp.net", 0, 0);

        // LID JID cases (critical for the bug)
        assert_jid_roundtrip("12345.6789@lid", "12345.6789", "lid", 0, 0);
        assert_jid_roundtrip("12345.6789:25@lid", "12345.6789", "lid", 25, 0);

        // @call (call-signaling server) must parse and render, not be rejected.
        assert_jid_roundtrip("12345@call", "12345", "call", 0, 0);
    }

    #[test]
    fn test_special_from_str_parsing() {
        // Test parsing of JIDs with an agent, which should be stored in the struct
        let jid = Jid::from_str("1234567890.2:15@hosted").expect("test hosted JID should be valid");
        assert_eq!(jid.user, "1234567890");
        assert_eq!(jid.server, "hosted");
        assert_eq!(jid.device, 15);
        assert_eq!(jid.agent, 2);
    }

    #[test]
    fn test_manual_jid_formatting_edge_cases() {
        // This test directly validates the fixes for the parity failures.
        // We manually construct the Jid struct as the binary decoder would,
        // then we assert that its string representation is correct.

        // Failure Case 1: An AD-JID for s.whatsapp.net decoded with an agent.
        // The Display trait MUST NOT show the agent number.
        let jid1 = Jid {
            user: "1234567890".into(),
            server: Server::Pn,
            device: 15,
            agent: 2,
            integrator: 0,
        };
        assert_eq!(jid1.to_string(), "1234567890:15@s.whatsapp.net");

        let jid2 = Jid {
            user: "12345.6789".into(),
            server: Server::Lid,
            device: 25,
            agent: 1,
            integrator: 0,
        };
        assert_eq!(jid2.to_string(), "12345.6789:25@lid");

        let jid3 = Jid {
            user: "1234567890".into(),
            server: Server::Hosted,
            device: 15,
            agent: 2,
            integrator: 0,
        };
        assert_eq!(jid3.to_string(), "1234567890:15@hosted");

        // Agent SHOULD be displayed for non-AD servers (e.g., bot, interop)
        let jid4 = Jid {
            user: "user".into(),
            server: Server::Bot,
            device: 10,
            agent: 5,
            integrator: 0,
        };
        assert_eq!(jid4.to_string(), "user.5:10@bot");
    }

    #[test]
    fn test_invalid_jids_should_fail_to_parse() {
        assert!(Jid::from_str("thisisnotajid").is_err());
        assert!(Jid::from_str("").is_err());
        // "@s.whatsapp.net" is now valid - it's the protocol format for server-only JIDs
        assert!(Jid::from_str("@s.whatsapp.net").is_ok());
        // But "@unknown.server" should still fail
        assert!(Jid::from_str("@unknown.server").is_err());
        // Jid::from_str("2") should not be possible due to type constraints,
        // but if it were, it should fail. The string must contain '@'.
        assert!(Jid::from_str("2").is_err());
    }

    /// A device past the field's `u16` is dropped, not rejected: the fast
    /// scanner reads it with `parse_u16_decimal` and falls back to 0, so a bad
    /// attribute costs one device rather than the whole stanza. Pinned because
    /// `65535` and `65536` are one apart and land on opposite sides.
    ///
    /// This is the parser's boundary, not the wire's: the AD form spends one
    /// byte on the device, so PN/LID stop at 255 (see
    /// `ad_jid_device_is_one_byte_wide_unlike_interop` in `encoder`).
    #[test]
    fn out_of_range_device_parses_as_no_device() {
        let max = Jid::from_str("5511987650001:65535@s.whatsapp.net").unwrap();
        assert_eq!(max.device, 65535);
        assert_eq!(max.to_string(), "5511987650001:65535@s.whatsapp.net");

        for raw in [
            "5511987650001:65536@s.whatsapp.net",
            "5511987650001:70000@s.whatsapp.net",
            "5511987650001:-1@s.whatsapp.net",
        ] {
            let jid = Jid::from_str(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            assert_eq!(jid.device, 0, "{raw}");
            assert_eq!(jid.user, "5511987650001", "{raw}");
            assert_eq!(jid.to_string(), "5511987650001@s.whatsapp.net", "{raw}");
        }
    }

    /// Tests for HOSTED device detection (`is_hosted()` method).
    ///
    /// # Context: What are HOSTED devices?
    ///
    /// HOSTED devices (also known as Cloud API or Meta Business API devices) are
    /// WhatsApp Business accounts that use Meta's server-side infrastructure instead
    /// of traditional end-to-end encryption with Signal protocol.
    ///
    /// ## Key characteristics:
    /// - Device ID is always 99 (`:99`)
    /// - Server is `@hosted` (phone-based) or `@hosted.lid` (LID-based)
    /// - They do NOT use Signal protocol prekeys
    /// - They should be EXCLUDED from group message fanout
    /// - They CAN receive 1:1 messages (but prekey fetch will fail, causing graceful skip)
    ///
    /// ## Why exclude from groups?
    /// WhatsApp Web explicitly filters hosted devices from group SKDM (Sender Key
    /// Distribution Message) distribution. From WhatsApp Web JS (`getFanOutList`):
    /// ```javascript
    /// var isHosted = e.id === 99 || e.isHosted === true;
    /// var includeInFanout = !isHosted || isOneToOneChat;
    /// ```
    ///
    /// ## JID formats:
    /// - Phone-based: `5511999887766:99@hosted`
    /// - LID-based: `100000012345678:99@hosted.lid`
    /// - Regular device with ID 99: `5511999887766:99@s.whatsapp.net` (also hosted!)
    #[test]
    fn test_is_hosted_device_detection() {
        // === HOSTED DEVICES (should return true) ===

        // Case 1: Device ID 99 on regular server (Cloud API business account)
        // This is the most common case - a business using Meta's Cloud API
        let cloud_api_device: Jid = "5511999887766:99@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        assert!(
            cloud_api_device.is_hosted(),
            "Device ID 99 on s.whatsapp.net should be detected as hosted (Cloud API)"
        );

        // Case 2: Device ID 99 on LID server
        let cloud_api_lid: Jid = "100000012345678:99@lid"
            .parse()
            .expect("test JID should be valid");
        assert!(
            cloud_api_lid.is_hosted(),
            "Device ID 99 on lid server should be detected as hosted"
        );

        // Case 3: Explicit @hosted server (phone-based hosted JID)
        let hosted_server: Jid = "5511999887766:99@hosted"
            .parse()
            .expect("test JID should be valid");
        assert!(
            hosted_server.is_hosted(),
            "JID with @hosted server should be detected as hosted"
        );

        // Case 4: Explicit @hosted.lid server (LID-based hosted JID)
        let hosted_lid_server: Jid = "100000012345678:99@hosted.lid"
            .parse()
            .expect("test JID should be valid");
        assert!(
            hosted_lid_server.is_hosted(),
            "JID with @hosted.lid server should be detected as hosted"
        );

        // Case 5: @hosted server with different device ID (edge case)
        // Even with device ID != 99, if server is @hosted, it's a hosted device
        let hosted_server_other_device: Jid = "5511999887766:0@hosted"
            .parse()
            .expect("test JID should be valid");
        assert!(
            hosted_server_other_device.is_hosted(),
            "JID with @hosted server should be hosted regardless of device ID"
        );

        // === NON-HOSTED DEVICES (should return false) ===

        // Case 6: Regular phone device (primary phone, device 0)
        let regular_phone: Jid = "5511999887766:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !regular_phone.is_hosted(),
            "Regular phone device (ID 0) should NOT be hosted"
        );

        // Case 7: Companion device (WhatsApp Web, device 33+)
        let companion_device: Jid = "5511999887766:33@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !companion_device.is_hosted(),
            "Companion device (ID 33) should NOT be hosted"
        );

        // Case 8: Regular LID device
        let regular_lid: Jid = "100000012345678:0@lid"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !regular_lid.is_hosted(),
            "Regular LID device should NOT be hosted"
        );

        // Case 9: LID companion device
        let lid_companion: Jid = "100000012345678:33@lid"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !lid_companion.is_hosted(),
            "LID companion device (ID 33) should NOT be hosted"
        );

        // Case 10: Group JID (not a device at all)
        let group_jid: Jid = "120363012345678@g.us"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !group_jid.is_hosted(),
            "Group JID should NOT be detected as hosted"
        );

        // Case 11: User JID without device
        let user_jid: Jid = "5511999887766@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !user_jid.is_hosted(),
            "User JID without device should NOT be hosted"
        );

        // Case 12: Bot device
        let bot_jid: Jid = "13136555001:0@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid");
        assert!(
            !bot_jid.is_hosted(),
            "Bot JID should NOT be detected as hosted (different mechanism)"
        );
    }

    #[test]
    fn is_same_chat_as_matches_rendered_chat_identity() {
        let base: Jid = "5511999887766@s.whatsapp.net".parse().unwrap();

        // Device is ignored.
        assert!(base.is_same_chat_as(&base.with_device(33)));
        assert!(base.with_device(5).is_same_chat_as(&base.with_device(0)));

        // Different user or server -> different chat (server guards the
        // is_same_user_as looseness that ignores server).
        let other_user: Jid = "5521988776655@s.whatsapp.net".parse().unwrap();
        assert!(!base.is_same_chat_as(&other_user));
        let as_lid: Jid = "5511999887766@lid".parse().unwrap();
        assert!(!base.is_same_chat_as(&as_lid));

        // integrator participates in identity.
        let other_integrator = Jid {
            integrator: 1,
            ..base.clone()
        };
        assert!(!base.is_same_chat_as(&other_integrator));

        // Pn suppresses the agent in display, so a stray decoded agent byte must
        // not split the chat: same rendered string -> same chat.
        let pn_agent1 = Jid {
            agent: 1,
            ..base.clone()
        };
        assert_eq!(base.to_string(), pn_agent1.to_string());
        assert!(base.is_same_chat_as(&pn_agent1));

        // @bot renders the agent, so distinct agents are distinct chats; device
        // is still ignored.
        let bot_a = Jid {
            user: "13136555001".into(),
            server: Server::Bot,
            agent: 1,
            device: 0,
            integrator: 0,
        };
        let bot_b = Jid {
            agent: 2,
            ..bot_a.clone()
        };
        assert_ne!(bot_a.to_string(), bot_b.to_string());
        assert!(!bot_a.is_same_chat_as(&bot_b));
        assert!(bot_a.is_same_chat_as(&bot_a.with_device(7)));
    }

    /// Tests that document the filtering behavior for group messages.
    ///
    /// # Why this matters:
    /// When sending a group message, we distribute Sender Key Distribution Messages
    /// (SKDM) to all participant devices. However, HOSTED devices:
    /// 1. Don't use Signal protocol, so they can't process SKDM
    /// 2. WhatsApp Web explicitly excludes them from group fanout
    /// 3. Including them would cause unnecessary prekey fetch failures
    ///
    /// This test documents the expected behavior when filtering device lists.
    #[test]
    fn test_hosted_device_filtering_for_groups() {
        // Simulate a group with mixed device types
        let devices: Vec<Jid> = vec![
            // Regular devices that SHOULD receive SKDM
            "5511999887766:0@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Phone
            "5511999887766:33@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // WhatsApp Web
            "5521988776655:0@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Another user's phone
            "100000012345678:0@lid"
                .parse()
                .expect("test JID should be valid"), // LID device
            "100000012345678:33@lid"
                .parse()
                .expect("test JID should be valid"), // LID companion
            // HOSTED devices that should be EXCLUDED from group SKDM
            "5531977665544:99@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Cloud API business
            "100000087654321:99@lid"
                .parse()
                .expect("test JID should be valid"), // Cloud API on LID
            "5541966554433:99@hosted"
                .parse()
                .expect("test JID should be valid"), // Explicit hosted
        ];

        // Filter out hosted devices (this is what prepare_group_stanza does)
        let filtered: Vec<&Jid> = devices.iter().filter(|jid| !jid.is_hosted()).collect();

        // Verify correct filtering
        assert_eq!(
            filtered.len(),
            5,
            "Should have 5 non-hosted devices after filtering"
        );

        // All filtered devices should NOT be hosted
        for jid in &filtered {
            assert!(
                !jid.is_hosted(),
                "Filtered list should not contain hosted devices: {}",
                jid
            );
        }

        // Count how many hosted devices were filtered out
        let hosted_count = devices.iter().filter(|jid| jid.is_hosted()).count();
        assert_eq!(hosted_count, 3, "Should have filtered out 3 hosted devices");
    }

    #[test]
    fn test_jid_pn_factory() {
        let jid = Jid::pn("1234567890");
        assert_eq!(jid.user, "1234567890");
        assert_eq!(jid.server, DEFAULT_USER_SERVER);
        assert_eq!(jid.device, 0);
        assert!(jid.is_pn());
    }

    #[test]
    fn test_jid_lid_factory() {
        let jid = Jid::lid("100000012345678");
        assert_eq!(jid.user, "100000012345678");
        assert_eq!(jid.server, HIDDEN_USER_SERVER);
        assert_eq!(jid.device, 0);
        assert!(jid.is_lid());
    }

    #[test]
    fn test_jid_group_factory() {
        let jid = Jid::group("123456789-1234567890");
        assert_eq!(jid.user, "123456789-1234567890");
        assert_eq!(jid.server, GROUP_SERVER);
        assert!(jid.is_group());
    }

    #[test]
    fn test_jid_pn_device_factory() {
        let jid = Jid::pn_device("1234567890", 5);
        assert_eq!(jid.user, "1234567890");
        assert_eq!(jid.server, DEFAULT_USER_SERVER);
        assert_eq!(jid.device, 5);
        assert!(jid.is_pn());
        assert!(jid.is_ad());
    }

    #[test]
    fn test_jid_lid_device_factory() {
        let jid = Jid::lid_device("100000012345678", 33);
        assert_eq!(jid.user, "100000012345678");
        assert_eq!(jid.server, HIDDEN_USER_SERVER);
        assert_eq!(jid.device, 33);
        assert!(jid.is_lid());
        assert!(jid.is_ad());
    }

    #[test]
    fn with_device_hosting_preserves_addressing_family() {
        let hosted_pn = Jid::pn("1234567890").with_device_hosting(7, true);
        assert_eq!(hosted_pn.server, Server::Hosted);
        assert_eq!(hosted_pn.device, 7);

        let hosted_lid = Jid::lid("100000012345678").with_device_hosting(8, true);
        assert_eq!(hosted_lid.server, Server::HostedLid);
        assert_eq!(hosted_lid.device, 8);

        let regular = Jid::pn("1234567890").with_device_hosting(9, false);
        assert_eq!(regular.server, Server::Pn);

        let regular_from_hosted =
            Jid::new("1234567890", Server::Hosted).with_device_hosting(9, false);
        assert_eq!(regular_from_hosted.server, Server::Pn);

        let group = Jid::group("123-456").with_device_hosting(10, true);
        assert_eq!(group.server, Server::Group);
    }

    #[test]
    fn test_status_broadcast_jid() {
        let jid = Jid::status_broadcast();
        assert_eq!(jid.user, STATUS_BROADCAST_USER);
        assert_eq!(jid.server, BROADCAST_SERVER);
        assert_eq!(jid.device, 0);
        assert!(jid.is_status_broadcast());
        assert!(!jid.is_group());
        assert!(!jid.is_broadcast_list());
        assert_eq!(jid.to_string(), "status@broadcast");

        // Parsing round-trip
        let parsed: Jid = "status@broadcast".parse().expect("should parse");
        assert!(parsed.is_status_broadcast());
        assert_eq!(parsed.user, "status");
        assert_eq!(parsed.server, "broadcast");

        // Regular broadcast list should NOT be status broadcast
        let broadcast_list = Jid::new("12345", Server::Broadcast);
        assert!(broadcast_list.is_broadcast_list());
        assert!(!broadcast_list.is_status_broadcast());

        // A group is not a status broadcast (the send path gates addressing_mode on this).
        let group: Jid = "120363012345678901@g.us".parse().expect("should parse");
        assert!(group.is_group());
        assert!(!group.is_status_broadcast());
    }

    #[test]
    fn test_jid_to_non_ad_preserves_user_server() {
        // Verify to_non_ad strips device but keeps user/server
        let device_jid = Jid::pn_device("1234567890", 33);
        let non_ad = device_jid.to_non_ad();
        assert_eq!(non_ad.user, "1234567890");
        assert_eq!(non_ad.server, DEFAULT_USER_SERVER);
        assert_eq!(non_ad.device, 0);
        assert!(!non_ad.is_ad());

        // LID variant
        let lid_device = Jid::lid_device("100000012345678", 25);
        let lid_non_ad = lid_device.to_non_ad();
        assert_eq!(lid_non_ad.user, "100000012345678");
        assert_eq!(lid_non_ad.server, HIDDEN_USER_SERVER);
        assert_eq!(lid_non_ad.device, 0);

        // status@broadcast stays the same
        let status = Jid::status_broadcast();
        let status_non_ad = status.to_non_ad();
        assert_eq!(status_non_ad.to_string(), "status@broadcast");
    }

    #[test]
    fn test_to_non_ad_string_matches_to_non_ad_to_string() {
        // to_non_ad_string() must be byte-identical to to_non_ad().to_string()
        // across PN/LID/bot/group/status, with and without device + agent.
        for s in [
            "1234567890:33@s.whatsapp.net",
            "1234567890@s.whatsapp.net",
            "100000012345678:25@lid",
            "100000012345678@lid",
            "867051314767696:0@bot",
            "867051314767696@bot",
            "120363021033254949@g.us",
            "status@broadcast",
            "12-34@g.us",
        ] {
            let jid: Jid = s.parse().expect("parse");
            assert_eq!(
                jid.to_non_ad_string(),
                jid.to_non_ad().to_string(),
                "mismatch for {s}"
            );
        }
    }

    /// The stack-buffered `Arc<str>` form must render exactly what the `String`
    /// form does, including for inputs that overflow the stack buffer and fall
    /// back to the heap, and for multibyte user parts (the buffer is bounded in
    /// bytes, and a split fragment would be invalid UTF-8).
    #[test]
    fn to_non_ad_arc_str_matches_to_non_ad_string() {
        let long_user = "9".repeat(80);
        let multibyte_user = "ẞünïcodé-ñ".repeat(3);
        let owned = [
            format!("{long_user}:12@s.whatsapp.net"),
            format!("{multibyte_user}@g.us"),
            format!("{multibyte_user}@s.whatsapp.net"),
        ];
        let cases = [
            "1234567890:33@s.whatsapp.net",
            "1234567890@s.whatsapp.net",
            "100000012345678:25@lid",
            "867051314767696:0@bot",
            // Nonzero agent on a bot JID: both forms drop the agent (matching
            // whatsmeow's ToNonAD), while `is_same_chat_as` still treats it as
            // identity-significant. Pinning it here keeps that asymmetry
            // deliberate rather than something a later edit can erase quietly.
            "867051314767696.5:10@bot",
            "120363021033254949@g.us",
            "status@broadcast",
        ]
        .into_iter()
        .chain(owned.iter().map(String::as_str));

        for s in cases {
            let jid: Jid = s.parse().unwrap_or_else(|e| panic!("parse {s}: {e}"));
            assert_eq!(
                &*jid.to_non_ad_arc_str(),
                jid.to_non_ad_string().as_str(),
                "mismatch for {s}"
            );
        }

        // A default (empty-user) JID has no wire form to render but must still
        // agree with the String path rather than panic in the stack writer.
        let empty = Jid::default();
        assert_eq!(
            &*empty.to_non_ad_arc_str(),
            empty.to_non_ad_string().as_str()
        );
    }

    #[test]
    fn test_into_non_ad_matches_to_non_ad() {
        // into_non_ad (consuming) must produce a JID identical to to_non_ad (cloning).
        for s in [
            "1234567890.2:33@s.whatsapp.net",
            "1234567890@s.whatsapp.net",
            "100000012345678:25@lid",
            "user.5:10@bot",
            "447911123456.3@interop",
            "120363021033254949@g.us",
            "status@broadcast",
        ] {
            let jid: Jid = s.parse().expect("parse");
            assert_eq!(
                jid.clone().into_non_ad(),
                jid.to_non_ad(),
                "mismatch for {s}"
            );
        }
    }

    #[test]
    fn test_jid_factories_with_string_types() {
        // Test with &str
        let jid1 = Jid::pn("123");
        assert_eq!(jid1.user, "123");

        // Test with String
        let jid2 = Jid::lid(String::from("456"));
        assert_eq!(jid2.user, "456");

        // Test with owned String
        let user = "789".to_string();
        let jid3 = Jid::group(user);
        assert_eq!(jid3.user, "789");
    }

    /// Verify that all JID formatting paths produce identical output:
    /// `Jid::Display`, `JidRef::Display`, `push_jid_to_string`, `push_jid_to_compact`,
    /// `Jid::push_to`, and the generic writer. Exercises the agent-elision rules
    /// across server variants.
    #[test]
    fn test_jid_format_parity() {
        struct Case {
            user: &'static str,
            server: Server,
            agent: u8,
            device: u16,
        }

        let cases = [
            // Empty user (server-only JID)
            Case {
                user: "",
                server: Server::Pn,
                agent: 0,
                device: 0,
            },
            // Basic phone, no agent/device
            Case {
                user: "5511999887766",
                server: Server::Pn,
                agent: 0,
                device: 0,
            },
            // Phone with device
            Case {
                user: "5511999887766",
                server: Server::Pn,
                agent: 0,
                device: 2,
            },
            // Phone with agent (suppressed for Pn)
            Case {
                user: "5511999887766",
                server: Server::Pn,
                agent: 3,
                device: 15,
            },
            // LID with agent (suppressed for Lid)
            Case {
                user: "12345.6789",
                server: Server::Lid,
                agent: 1,
                device: 25,
            },
            // Hosted with agent (suppressed)
            Case {
                user: "100000012345678",
                server: Server::Hosted,
                agent: 2,
                device: 99,
            },
            // HostedLid with agent (suppressed)
            Case {
                user: "100000012345678",
                server: Server::HostedLid,
                agent: 1,
                device: 99,
            },
            // Group (no agent, no device)
            Case {
                user: "120363012345678901",
                server: Server::Group,
                agent: 0,
                device: 0,
            },
            // Bot with agent (shown)
            Case {
                user: "user",
                server: Server::Bot,
                agent: 5,
                device: 10,
            },
            // Interop with agent (shown)
            Case {
                user: "447911123456",
                server: Server::Interop,
                agent: 3,
                device: 0,
            },
            // Messenger with device, no agent
            Case {
                user: "messenger_user",
                server: Server::Messenger,
                agent: 0,
                device: 50,
            },
            // Broadcast
            Case {
                user: "status",
                server: Server::Broadcast,
                agent: 0,
                device: 0,
            },
            // Newsletter
            Case {
                user: "newsletter_id",
                server: Server::Newsletter,
                agent: 0,
                device: 0,
            },
            // Max values
            Case {
                user: "447911123456789",
                server: Server::Pn,
                agent: 255,
                device: 65535,
            },
            // Short user
            Case {
                user: "1",
                server: Server::Legacy,
                agent: 0,
                device: 1,
            },
        ];

        for (i, c) in cases.iter().enumerate() {
            let jid = Jid {
                user: c.user.into(),
                server: c.server,
                agent: c.agent,
                device: c.device,
                integrator: 0,
            };

            // Reference: Display impl (via write_jid! fallible)
            let display = jid.to_string();

            // JidRef Display
            let jid_ref = JidRef {
                user: NodeStr::Borrowed(c.user),
                server: c.server,
                agent: c.agent,
                device: c.device,
                integrator: 0,
            };
            let ref_display = jid_ref.to_string();

            // push_jid_to_string
            let mut string_buf = String::new();
            push_jid_to_string(c.user, c.server, c.agent, c.device, &mut string_buf);

            // push_jid_to_compact
            let mut compact_buf = CompactString::default();
            push_jid_to_compact(c.user, c.server, c.agent, c.device, &mut compact_buf);

            // Jid::push_to
            let mut push_buf = String::new();
            jid.push_to(&mut push_buf);

            // Generic fmt::Write path.
            let mut write_buf = String::new();
            jid.write_display_to(&mut write_buf).unwrap();

            assert_eq!(display, ref_display, "case {i}: Display vs JidRef::Display");
            assert_eq!(
                display, string_buf,
                "case {i}: Display vs push_jid_to_string"
            );
            assert_eq!(
                display,
                compact_buf.as_str(),
                "case {i}: Display vs push_jid_to_compact"
            );
            assert_eq!(display, push_buf, "case {i}: Display vs Jid::push_to");
            assert_eq!(display, write_buf, "case {i}: Display vs generic writer");
        }
    }
}
