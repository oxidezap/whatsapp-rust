use crate::libsignal::protocol::{AddressBuf, DeviceId, ProtocolAddress};
use crate::libsignal::store::sender_key_name::SenderKeyName;
use wacore_binary::{DEFAULT_USER_SERVER, Jid, LEGACY_USER_SERVER, Server};

/// Real WhatsApp logs show max signal address length of 53 chars.
/// 64 bytes covers all known addresses without reallocation.
const SIGNAL_ADDRESS_CAPACITY: usize = 64;

/// WhatsApp encodes the device in the address name, not in the
/// Signal device_id field. The device_id is always 0.
const SIGNAL_DEVICE_ID: DeviceId = DeviceId::new(0);

/// WA Web's Signal address format uses the legacy `c.us` server
/// instead of `s.whatsapp.net`.
#[inline]
fn mapped_server(s: &str) -> &str {
    if s == DEFAULT_USER_SERVER {
        LEGACY_USER_SERVER
    } else {
        s
    }
}

/// Create a pre-allocated buffer for address formatting in hot loops.
pub fn make_address_buffer() -> String {
    String::with_capacity(SIGNAL_ADDRESS_CAPACITY)
}

/// Create a reusable `ProtocolAddress` for hot loops.
/// Call `reset_protocol_address` to fill without allocation.
pub fn make_reusable_protocol_address() -> ProtocolAddress {
    ProtocolAddress::empty(SIGNAL_DEVICE_ID)
}

/// Somewhere an address name can be written.
///
/// The address format lives in exactly one function, and that function has to
/// serve both a plain `String` and the buffer inside a `ProtocolAddress` (which
/// is inline, not a `String`). This is what lets it do that without the format
/// existing in two places.
pub trait AddressSink {
    fn clear(&mut self);
    fn push_str(&mut self, s: &str);
    fn push(&mut self, c: char);
}

impl AddressSink for String {
    #[inline]
    fn clear(&mut self) {
        String::clear(self);
    }
    #[inline]
    fn push_str(&mut self, s: &str) {
        String::push_str(self, s);
    }
    #[inline]
    fn push(&mut self, c: char) {
        String::push(self, c);
    }
}

impl AddressSink for AddressBuf {
    #[inline]
    fn clear(&mut self) {
        AddressBuf::clear(self);
    }
    #[inline]
    fn push_str(&mut self, s: &str) {
        AddressBuf::push_str(self, s);
    }
    #[inline]
    fn push(&mut self, c: char) {
        AddressBuf::push(self, c);
    }
}

/// Append the signal address name (`{user}[:device]@{server}`) to `buf`,
/// keeping whatever is already there. All other address helpers delegate to
/// this; the appending form exists so an address can be written into the middle
/// of a larger buffer (a `SenderKeyName`) without a temporary to copy out of.
pub fn push_signal_address_to<W: AddressSink + ?Sized>(jid: &Jid, buf: &mut W) {
    let server = mapped_server(jid.server.as_str());
    buf.push_str(&jid.user);
    if jid.device != 0 {
        buf.push(':');
        buf.push_str(itoa::Buffer::new().format(jid.device));
    }
    buf.push('@');
    buf.push_str(server);
}

/// Append the full protocol address (`{signal_address}.0`) to `buf`.
pub fn push_protocol_address_to<W: AddressSink + ?Sized>(jid: &Jid, buf: &mut W) {
    push_signal_address_to(jid, buf);
    buf.push_str(".0");
}

/// Upper bound on the rendered protocol address of `jid`, for sizing a buffer
/// in one allocation. The device adds at most six bytes (a `:` and five
/// digits), the `@` one more and the `.0` suffix two; `mapped_server` only ever
/// shortens the server, never lengthens it.
#[inline]
pub fn protocol_address_len_hint(jid: &Jid) -> usize {
    jid.user.len() + jid.server.as_str().len() + 9
}

/// Write the signal address name (`{user}[:device]@{server}`) into `buf`,
/// clearing it first.
pub fn write_signal_address_to<W: AddressSink + ?Sized>(jid: &Jid, buf: &mut W) {
    buf.clear();
    push_signal_address_to(jid, buf);
}

/// Write the full protocol address (`{signal_address}.0`) into `buf`.
pub fn write_protocol_address_to<W: AddressSink + ?Sized>(jid: &Jid, buf: &mut W) {
    buf.clear();
    push_protocol_address_to(jid, buf);
}

/// Consistent ordering for deadlock-free multi-lock acquisition.
pub fn cmp_for_lock_order(a: &Jid, b: &Jid) -> std::cmp::Ordering {
    mapped_server(a.server.as_str())
        .cmp(mapped_server(b.server.as_str()))
        .then_with(|| a.user.cmp(&b.user))
        .then_with(|| a.device.cmp(&b.device))
}

/// Sort and deduplicate by user identity (user + server).
pub fn sort_dedup_by_user(jids: &mut Vec<Jid>) {
    jids.sort_unstable_by(|a, b| a.user.cmp(&b.user).then_with(|| a.server.cmp(&b.server)));
    jids.dedup_by(|a, b| a.user == b.user && a.server == b.server);
}

/// Sort and deduplicate by device identity.
///
/// Keyed on exactly what `Jid`'s equality compares — user, server, device,
/// integrator, and `identity_agent` — so the fan-out cannot disagree with `==`
/// in either direction. Both directions are real: keying on the raw `agent`
/// would let two JIDs that are one device (an inert agent on Pn/Lid/Hosted/
/// HostedLid, same AD-JID, same Signal address) both survive and give one
/// session two concurrent encryption jobs; dropping the agent entirely would
/// collapse two genuinely different `@bot`/`@interop` devices, which do render
/// it, and silently lose a destination.
pub fn sort_dedup_by_device(jids: &mut Vec<Jid>) {
    fn key(j: &Jid) -> (&str, Server, u16, u16, u8) {
        (
            &j.user,
            j.server,
            j.device,
            j.integrator,
            j.identity_agent(),
        )
    }
    jids.sort_unstable_by(|a, b| key(a).cmp(&key(b)));
    jids.dedup_by(|a, b| key(a) == key(b));
}

/// Build a `SenderKeyName` from the group JID and the sender's JID in a single
/// allocation, rendering the sender's Signal address straight into the final
/// buffer.
///
/// The `ProtocolAddress` overload below has to render the address into the
/// address's own buffer first and then copy it across; here there is nothing to
/// copy, so a caller holding the sender as a JID should use this one. The
/// result is byte-identical: the sender half is the full protocol address,
/// device suffix included.
pub fn make_sender_key_name_for_jid(group_jid: &Jid, sender: &Jid) -> SenderKeyName {
    let mut buf =
        String::with_capacity(group_jid.user.len() + 20 + 1 + protocol_address_len_hint(sender));
    group_jid.push_to(&mut buf);
    let group_len = buf.len();
    buf.push(':');
    push_protocol_address_to(sender, &mut buf);
    SenderKeyName::from_buf(buf, group_len)
}

/// Build a `SenderKeyName` from a `&Jid` + `&ProtocolAddress` in a single
/// allocation. Pushes the group JID and sender address directly into the
/// final buffer — no intermediate `to_string()` or temp buffers.
pub fn make_sender_key_name(group_jid: &Jid, sender: &ProtocolAddress) -> SenderKeyName {
    let sender_str = sender.as_str();
    let mut buf = String::with_capacity(group_jid.user.len() + 20 + 1 + sender_str.len());
    group_jid.push_to(&mut buf);
    let group_len = buf.len();
    buf.push(':');
    buf.push_str(sender_str);
    SenderKeyName::from_buf(buf, group_len)
}

pub trait JidExt {
    fn to_protocol_address(&self) -> ProtocolAddress;
    fn to_signal_address_string(&self) -> String;
    fn to_protocol_address_string(&self) -> String;

    /// Rewrite a reusable `ProtocolAddress` in place for this JID.
    /// Writes directly into the address — no intermediate buffer needed.
    fn reset_protocol_address(&self, addr: &mut ProtocolAddress);
}

impl JidExt for Jid {
    fn to_signal_address_string(&self) -> String {
        let mut buf = make_address_buffer();
        write_signal_address_to(self, &mut buf);
        buf
    }

    fn to_protocol_address(&self) -> ProtocolAddress {
        // Written straight into the address: the intermediate `String` this
        // used to build was allocated only to be copied in and dropped.
        let mut addr = make_reusable_protocol_address();
        self.reset_protocol_address(&mut addr);
        addr
    }

    fn to_protocol_address_string(&self) -> String {
        let mut buf = make_address_buffer();
        write_protocol_address_to(self, &mut buf);
        buf
    }

    fn reset_protocol_address(&self, addr: &mut ProtocolAddress) {
        let jid = self;
        addr.reset_with(|name| write_signal_address_to(jid, name));
    }
}

/// Privacy-aware rendering of a Signal [`ProtocolAddress`] for tracing/logs.
///
/// The address name embeds the peer JID (a phone number for PN peers) plus the
/// device, so logging it directly leaks PII. This replaces the whole name with a
/// keyed token (same per-process scheme as `Jid::observe`): stable per peer-device
/// for correlation, but not reversible to the number. (The Signal `device_id` is
/// always 0 here — the device lives inside the name — so it is not shown.)
pub fn observe_protocol_address(addr: &ProtocolAddress) -> String {
    if cfg!(feature = "tracing-pii") {
        return addr.name().to_string();
    }
    format!(
        "addr#{:016x}",
        wacore_binary::jid::observe_token(addr.name())
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_signal_address_string_lid() {
        let jid = Jid::from_str("123456789@lid").unwrap();
        assert_eq!(jid.to_signal_address_string(), "123456789@lid");
    }

    #[test]
    fn test_signal_address_string_lid_with_device() {
        let jid = Jid::from_str("123456789:33@lid").unwrap();
        assert_eq!(jid.to_signal_address_string(), "123456789:33@lid");
    }

    #[test]
    fn test_signal_address_string_phone() {
        let jid = Jid::from_str("15550000001@s.whatsapp.net").unwrap();
        assert_eq!(jid.to_signal_address_string(), "15550000001@c.us");
    }

    #[test]
    fn test_protocol_address_format() {
        let jid = Jid::from_str("123456789:33@lid").unwrap();
        let addr = jid.to_protocol_address();
        assert_eq!(addr.name(), "123456789:33@lid");
        assert_eq!(addr.to_string(), "123456789:33@lid.0");
    }

    #[test]
    fn test_protocol_address_string_matches_to_string() {
        let jids = [
            "123456789@lid",
            "123456789:33@lid",
            "100000000000001.1:75@lid",
            "15550000001@s.whatsapp.net",
            "15550000001:33@s.whatsapp.net",
        ];
        for jid_str in &jids {
            let jid = Jid::from_str(jid_str).unwrap();
            assert_eq!(
                jid.to_protocol_address_string(),
                jid.to_protocol_address().to_string(),
            );
        }
    }

    #[test]
    fn test_reset_protocol_address_matches_fresh() {
        let cases = [
            ("123456789@lid", "123456789@lid", "123456789@lid.0"),
            ("123456789:33@lid", "123456789:33@lid", "123456789:33@lid.0"),
            (
                "100000000000001.1:75@lid",
                "100000000000001.1:75@lid",
                "100000000000001.1:75@lid.0",
            ),
            (
                "15550000001@s.whatsapp.net",
                "15550000001@c.us",
                "15550000001@c.us.0",
            ),
        ];
        let mut addr = make_reusable_protocol_address();
        for (jid_str, expected_name, expected_display) in &cases {
            let jid = Jid::from_str(jid_str).unwrap();
            jid.reset_protocol_address(&mut addr);
            assert_eq!(addr.name(), *expected_name);
            assert_eq!(addr.as_str(), *expected_display);
        }
    }

    /// The one writer must produce the same bytes into either sink, or the
    /// heap path and the inline path would name the same device differently.
    #[test]
    fn both_sinks_receive_the_same_address() {
        let cases = [
            "123456789@lid",
            "123456789:33@lid",
            "100000000000001.1:75@lid",
            "15550000001@s.whatsapp.net",
            "15550000001:33@s.whatsapp.net",
            "120363000000000001@g.us",
            "999999999999999999@newsletter",
        ];
        for jid_str in cases {
            let jid = Jid::from_str(jid_str).unwrap();

            let mut string_sink = String::new();
            write_protocol_address_to(&jid, &mut string_sink);

            let mut address = make_reusable_protocol_address();
            jid.reset_protocol_address(&mut address);

            assert_eq!(
                address.as_str(),
                string_sink,
                "the two sinks disagree for {jid_str}"
            );
        }
    }

    /// Reusing one buffer across JIDs must leave no trace of the previous one,
    /// including when the previous name was longer.
    #[test]
    fn a_reused_address_keeps_nothing_from_the_previous_jid() {
        let long = Jid::from_str("100000000000001.1:75@lid").unwrap();
        let short = Jid::from_str("1@lid").unwrap();

        let mut address = make_reusable_protocol_address();
        address.reset_with(|buf| buf.push_str(&"z".repeat(200)));
        jid_reset(&mut address, &long);
        assert_eq!(address.as_str(), "100000000000001.1:75@lid.0");
        jid_reset(&mut address, &short);
        assert_eq!(address.as_str(), "1@lid.0");
        assert_eq!(address.name(), "1@lid");
    }

    fn jid_reset(address: &mut ProtocolAddress, jid: &Jid) {
        jid.reset_protocol_address(address);
    }

    #[test]
    fn test_write_functions_dry() {
        let jid = Jid::from_str("15550000001@s.whatsapp.net").unwrap();
        let mut buf = String::new();

        write_signal_address_to(&jid, &mut buf);
        assert_eq!(buf, "15550000001@c.us");

        write_protocol_address_to(&jid, &mut buf);
        assert_eq!(buf, "15550000001@c.us.0");
    }

    /// The JID route and the `ProtocolAddress` route name the same chain, or a
    /// send and the receive that follows it would key their sender keys
    /// differently and the chain would look missing.
    #[test]
    fn sender_key_name_from_jid_matches_the_address_route() {
        let group = Jid::from_str("120363000000000001@g.us").unwrap();
        let senders = [
            "15550000001@s.whatsapp.net",
            "15550000001:33@s.whatsapp.net",
            "123456789@lid",
            "100000000000001.1:75@lid",
        ];
        for sender_str in senders {
            let sender = Jid::from_str(sender_str).unwrap();
            let direct = make_sender_key_name_for_jid(&group, &sender);
            let via_address = make_sender_key_name(&group, &sender.to_protocol_address());
            assert_eq!(direct.cache_key(), via_address.cache_key());
            assert_eq!(direct.group_id(), via_address.group_id());
            assert_eq!(direct.sender_id(), via_address.sender_id());
        }
    }

    /// The writer's "clears it first" contract holds for the inline sink too:
    /// a reused buffer must be overwritten, not appended to.
    #[test]
    fn the_inline_sink_is_cleared_before_each_write() {
        let first = Jid::from_str("15550000001@s.whatsapp.net").unwrap();
        let second = Jid::from_str("123456789:33@lid").unwrap();

        let mut buf = AddressBuf::empty();
        write_signal_address_to(&first, &mut buf);
        assert_eq!(buf.as_str(), "15550000001@c.us");

        write_protocol_address_to(&second, &mut buf);
        assert_eq!(buf.as_str(), "123456789:33@lid.0");
    }

    /// The fan-out uses this to collapse duplicate wire destinations, so it has
    /// to agree with `Jid`'s equality. Two LID JIDs differing only in the agent
    /// are one device — same AD-JID on the wire, same Signal address — and must
    /// not both survive, or the group send builds two encryption jobs against
    /// one session.
    #[test]
    fn device_dedup_collapses_jids_that_differ_only_in_an_inert_agent() {
        let plain = Jid {
            user: "123456789012345".into(),
            server: Server::Lid,
            agent: 0,
            device: 33,
            integrator: 0,
        };
        let with_agent = Jid {
            agent: 1,
            ..plain.clone()
        };
        assert_eq!(plain, with_agent, "precondition: one identity");
        assert_eq!(
            plain.to_signal_address_string(),
            with_agent.to_signal_address_string(),
            "precondition: one Signal address"
        );

        let mut jids = vec![plain.clone(), with_agent];
        sort_dedup_by_device(&mut jids);
        assert_eq!(jids, vec![plain.clone()], "one device, one entry");

        // A different device still survives as its own entry.
        let other_device = Jid {
            device: 34,
            ..plain.clone()
        };
        let mut jids = vec![plain.clone(), other_device.clone()];
        sort_dedup_by_device(&mut jids);
        assert_eq!(jids.len(), 2);
    }

    /// The mirror of the case above: on the servers that DO render the agent it
    /// is identity, `==` treats those JIDs as different devices, and collapsing
    /// them here would silently drop a destination from the fan-out.
    #[test]
    fn device_dedup_keeps_agents_apart_where_the_server_renders_them() {
        for server in [Server::Bot, Server::Interop] {
            let a = Jid {
                user: "123456789".into(),
                server,
                agent: 1,
                device: 0,
                integrator: 0,
            };
            let b = Jid {
                agent: 2,
                ..a.clone()
            };
            assert_ne!(a, b, "{server:?}: renders the agent, so these differ");

            let mut jids = vec![a, b];
            sort_dedup_by_device(&mut jids);
            assert_eq!(
                jids.len(),
                2,
                "{server:?}: dedup must not merge two rendered agents"
            );
        }

        // `integrator` is identity too, and the key has to carry it.
        let base = Jid {
            user: "123456789".into(),
            server: Server::Interop,
            agent: 0,
            device: 0,
            integrator: 1,
        };
        let other = Jid {
            integrator: 2,
            ..base.clone()
        };
        assert_ne!(base, other);
        let mut jids = vec![base, other];
        sort_dedup_by_device(&mut jids);
        assert_eq!(jids.len(), 2, "integrator must not be dropped from the key");
    }
}
