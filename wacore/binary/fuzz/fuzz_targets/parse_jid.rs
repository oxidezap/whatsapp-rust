#![no_main]

//! Fuzz the JID parser and formatter.
//!
//! JID strings arrive as attacker-controlled attribute values on every stanza,
//! and `Jid`'s parse path mixes a hand-rolled fast scanner with a string-slicing
//! fallback — exactly the shape where a stray index panics the whole client.
//!
//! Two properties are checked:
//!
//! 1. **Parsing arbitrary text never panics.** Any input either yields a `Jid`
//!    or an error; slicing, `u16` parsing and the agent range check must all
//!    stay inside the `Result`. Display of a parsed JID must not panic either.
//! 2. **A valid JID round-trips.** JIDs built from the fuzz bytes (rather than
//!    from arbitrary text) are rendered and re-parsed, and must come back
//!    unchanged. Only constructed JIDs are held to this: `Display` is
//!    deliberately lossy for an empty user (renders the bare server) and for
//!    servers that suppress the agent byte, so parsed-from-text inputs cannot
//!    all be canonical.

use libfuzzer_sys::fuzz_target;
use wacore_binary::jid::{Jid, Server, parse_jid_fast, parse_jid_ref};

const SERVERS: [Server; 12] = [
    Server::Pn,
    Server::Lid,
    Server::Group,
    Server::Broadcast,
    Server::Newsletter,
    Server::Hosted,
    Server::HostedLid,
    Server::Messenger,
    Server::Interop,
    Server::Bot,
    Server::Legacy,
    Server::Call,
];

/// Characters real user parts are made of (phone numbers, group ids). `@`, `:`
/// and `.` are excluded on purpose: they are the parser's separators, so a user
/// containing them is not expected to survive a display round-trip.
const USER_CHARS: &[u8] = b"0123456789-";

/// Build a JID that is valid by construction, driven by the fuzz bytes.
fn build_jid(data: &[u8]) -> Jid {
    let byte = |i: usize| data.get(i).copied().unwrap_or(0);

    let server = SERVERS[byte(0) as usize % SERVERS.len()];
    let device = u16::from_le_bytes([byte(1), byte(2)]);
    // The formatter suppresses the agent for AD servers, so only ask for one
    // where it is actually rendered.
    let agent = if server.renders_agent() { byte(3) } else { 0 };

    let mut user: String = data
        .iter()
        .skip(4)
        .take(32)
        .map(|b| USER_CHARS[*b as usize % USER_CHARS.len()] as char)
        .collect();
    if user.is_empty() {
        // A bare server carries neither agent nor device, so it cannot
        // round-trip one; give the JID a user part.
        user.push('1');
    }

    let mut jid = Jid::new(user, server);
    jid.agent = agent;
    jid.device = device;
    jid
}

fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        let _ = parse_jid_fast(text);
        let _ = parse_jid_ref(text);

        if let Ok(jid) = text.parse::<Jid>() {
            let _ = jid.to_string();
            let _ = jid.to_ad_string();
            let _ = jid.to_non_ad_string();
            let _ = jid.device_key();
            assert!(jid.display_eq(&jid.to_string()));
        }
    }

    let jid = build_jid(data);
    let rendered = jid.to_string();
    let Ok(reparsed) = rendered.parse::<Jid>() else {
        panic!("valid JID {rendered:?} failed to re-parse (built from {jid:?})");
    };
    assert_eq!(
        reparsed, jid,
        "display/parse round-trip lost data for {rendered:?}"
    );
    assert_eq!(
        reparsed.to_string(),
        rendered,
        "re-rendering a re-parsed JID must be stable"
    );
});
