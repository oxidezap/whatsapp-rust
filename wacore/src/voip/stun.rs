//! STUN/WARP relay framing: RFC 5389 TLV encoder with WhatsApp's MESSAGE-INTEGRITY
//! (HMAC-SHA1) and FINGERPRINT (CRC-32), the non-protobuf allocate builders, the
//! WhatsApp ping, and the response parsers.
//!
//! Transaction IDs are passed in (the I/O layer supplies 12 random bytes) so this stays
//! pure and deterministically testable. Protobuf-based allocate builders (0x4024/0x4025
//! dynamic) come with the waproto voip schemas.
//!
//! wacrg spec: stun-relay (REL-02), relay-candidates (REL-01).

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

const STUN_MAGIC: u32 = 0x2112_a442;
const STUN_FINGERPRINT_XOR: u32 = 0x5354_554e;
const STUN_XOR_PORT: u16 = 0x2112;
const STUN_XOR_ADDR: [u8; 4] = [0x21, 0x12, 0xa4, 0x42];

const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
const ATTR_FINGERPRINT: u16 = 0x8028;
const ATTR_ERROR_CODE: u16 = 0x0009;
const ATTR_RELAY_TOKEN: u16 = 0x4000;
const STUN_ATTR_STREAM_DESCRIPTORS: u16 = 0x4024;
const STUN_ATTR_WASM_RELAY_ENDPOINT: u16 = 0x0016;

pub const MSG_BINDING_REQUEST: u16 = 0x0001;
pub const MSG_ALLOCATE_REQUEST: u16 = 0x0003;
pub const MSG_BINDING_SUCCESS: u16 = 0x0101;
pub const MSG_BINDING_ERROR: u16 = 0x0111;
pub const MSG_ALLOCATE_SUCCESS: u16 = 0x0103;
pub const MSG_ALLOCATE_ERROR: u16 = 0x0113;
pub const MSG_WHATSAPP_PING: u16 = 0x0801;
pub const MSG_WHATSAPP_PONG: u16 = 0x0802;

fn pad4(n: usize) -> usize {
    (4 - (n % 4)) % 4
}

/// Encode a single STUN attribute (type, length, value, 4-byte alignment padding).
fn stun_attr(attr_type: u16, value: &[u8]) -> Vec<u8> {
    let pad = pad4(value.len());
    let mut buf = Vec::with_capacity(4 + value.len() + pad);
    buf.extend_from_slice(&attr_type.to_be_bytes());
    buf.extend_from_slice(&(value.len() as u16).to_be_bytes());
    buf.extend_from_slice(value);
    buf.resize(buf.len() + pad, 0);
    buf
}

/// CRC-32 (IEEE, reflected, poly 0xedb88320) for the STUN FINGERPRINT.
fn crc32(buf: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for &b in buf {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

fn stun_pseudo_header(msg_type: u16, msg_len: u16, transaction_id: &[u8; 12]) -> [u8; 20] {
    let mut h = [0u8; 20];
    h[0..2].copy_from_slice(&msg_type.to_be_bytes());
    h[2..4].copy_from_slice(&msg_len.to_be_bytes());
    h[4..8].copy_from_slice(&STUN_MAGIC.to_be_bytes());
    h[8..20].copy_from_slice(transaction_id);
    h
}

/// Encode a STUN request per RFC 5389: header + attrs, then optional MESSAGE-INTEGRITY
/// (over a pseudo-header whose length already counts the MI attr) and FINGERPRINT.
pub fn encode_stun_request(
    msg_type: u16,
    transaction_id: &[u8; 12],
    attrs: &[u8],
    integrity_key: Option<&[u8]>,
    include_fingerprint: bool,
) -> Vec<u8> {
    let mut body = attrs.to_vec();

    if let Some(key) = integrity_key {
        let msg_len = (body.len() + 24) as u16; // attrs + MI attr (4 + 20)
        let header = stun_pseudo_header(msg_type, msg_len, transaction_id);
        let mut mac = HmacSha1::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(&header);
        mac.update(&body);
        let mi = mac.finalize().into_bytes(); // 20 bytes
        body.extend_from_slice(&stun_attr(ATTR_MESSAGE_INTEGRITY, &mi));
    }

    if include_fingerprint {
        let msg_len = (body.len() + 8) as u16; // attrs + FINGERPRINT attr (4 + 4)
        let header = stun_pseudo_header(msg_type, msg_len, transaction_id);
        let mut crc_input = Vec::with_capacity(20 + body.len());
        crc_input.extend_from_slice(&header);
        crc_input.extend_from_slice(&body);
        let fp = crc32(&crc_input) ^ STUN_FINGERPRINT_XOR;
        body.extend_from_slice(&stun_attr(ATTR_FINGERPRINT, &fp.to_be_bytes()));
    }

    let mut out = Vec::with_capacity(20 + body.len());
    out.extend_from_slice(&msg_type.to_be_bytes());
    out.extend_from_slice(&(body.len() as u16).to_be_bytes());
    out.extend_from_slice(&STUN_MAGIC.to_be_bytes());
    out.extend_from_slice(transaction_id);
    out.extend_from_slice(&body);
    out
}

/// Native WA sender subscription: 1-byte count + big-endian SSRC (attr 0x4023).
pub fn create_native_sender_subscription(ssrc: u32) -> [u8; 5] {
    let mut buf = [0u8; 5];
    buf[0] = 1;
    buf[1..5].copy_from_slice(&ssrc.to_be_bytes());
    buf
}

/// XOR-encoded IPv4:port (6 bytes) for the WASM relay endpoint attr.
pub fn encode_xor_relay_endpoint(ipv4: &str, port: u16) -> Option<[u8; 6]> {
    let octets: Vec<u8> = ipv4
        .split('.')
        .filter_map(|n| n.parse::<u8>().ok())
        .collect();
    if octets.len() != 4 {
        return None;
    }
    let xor_port = port ^ STUN_XOR_PORT;
    let mut buf = [0u8; 6];
    buf[0..2].copy_from_slice(&xor_port.to_be_bytes());
    for i in 0..4 {
        buf[2 + i] = octets[i] ^ STUN_XOR_ADDR[i];
    }
    Some(buf)
}

/// WASM attr 0x0016 value: `00 01` followed by the 6-byte XOR relay endpoint.
fn create_wasm_relay_endpoint_attr(endpoint_xor: &[u8; 6]) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0..2].copy_from_slice(&1u16.to_be_bytes());
    buf[2..8].copy_from_slice(endpoint_xor);
    buf
}

/// `(stream_index, sub_type, slot_word)` in WASM wire order. Stream indices are audio/video0/video1;
/// sub-types are media/FEC/NACK.
const WASM_STREAM_SLOTS: [(u32, u32, u32); 9] = [
    (0, 0, 0),
    (0, 1, 1),
    (0, 2, 4),
    (1, 0, 2),
    (1, 1, 3),
    (1, 2, 5),
    (2, 0, 7),
    (2, 1, 8),
    (2, 2, 6),
];

/// Build call-specific WASM `StreamDescriptors` (0x4024).
pub fn create_wasm_stream_descriptors(call_id: &str, self_participant_id: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for &(stream_index, sub_type, slot) in &WASM_STREAM_SLOTS {
        let ssrc =
            crate::voip::ssrc::derive_wasm_participant_ssrc(call_id, self_participant_id, slot);
        let mut d = Vec::new();
        if stream_index != 0 {
            pb_tag(&mut d, 1, 0);
            pb_varint(&mut d, stream_index as u64);
        }
        if sub_type != 0 {
            pb_tag(&mut d, 2, 0);
            pb_varint(&mut d, sub_type as u64);
        }
        pb_tag(&mut d, 3, 0);
        pb_varint(&mut d, ssrc as u64);
        pb_len_delim(&mut out, 1, &d);
    }
    out
}

/// WASM/Web DataChannel Allocate: 0x4000 token + 0x4024 stream desc + 0x0016 endpoint + MI, no FP.
/// Descriptors must carry this call's derived SSRCs rather than values captured from another call.
pub fn build_wasm_stun_allocate_request(
    transaction_id: &[u8; 12],
    relay_token: &[u8],
    endpoint_xor: &[u8; 6],
    integrity_key: &[u8],
    call_id: &str,
    self_participant_id: &str,
) -> Vec<u8> {
    let mut attrs = stun_attr(ATTR_RELAY_TOKEN, relay_token);
    attrs.extend_from_slice(&stun_attr(
        STUN_ATTR_STREAM_DESCRIPTORS,
        &create_wasm_stream_descriptors(call_id, self_participant_id),
    ));
    attrs.extend_from_slice(&stun_attr(
        STUN_ATTR_WASM_RELAY_ENDPOINT,
        &create_wasm_relay_endpoint_attr(endpoint_xor),
    ));
    encode_stun_request(
        MSG_ALLOCATE_REQUEST,
        transaction_id,
        &attrs,
        Some(integrity_key),
        false,
    )
}

/// WhatsApp consent ping (type 0x0801, empty body).
pub fn build_whatsapp_ping(transaction_id: &[u8; 12]) -> [u8; 20] {
    let mut out = [0u8; 20];
    out[0..2].copy_from_slice(&MSG_WHATSAPP_PING.to_be_bytes());
    out[4..8].copy_from_slice(&STUN_MAGIC.to_be_bytes());
    out[8..20].copy_from_slice(transaction_id);
    out
}

pub fn is_stun_packet(data: &[u8]) -> bool {
    data.len() >= 2 && (data[0] & 0xc0) == 0x00
}

pub fn stun_message_type(data: &[u8]) -> Option<u16> {
    (data.len() >= 2).then(|| (((data[0] & 0x3f) as u16) << 8) | data[1] as u16)
}

pub fn stun_transaction_id(data: &[u8]) -> Option<&[u8]> {
    (data.len() >= 20).then(|| &data[8..20])
}

/// A full STUN message: STUN-prefixed, a 20-byte header, and carrying the magic cookie. The cookie
/// separates a real STUN packet from garbage that merely starts with a STUN-looking type, so the
/// allocate success/error decisions can't be driven by a malformed packet.
fn is_complete_stun(data: &[u8]) -> bool {
    if !(is_stun_packet(data) && data.len() >= 20 && data[4..8] == STUN_MAGIC.to_be_bytes()) {
        return false;
    }
    // Bytes 2..4 are the message-length field (body after the 20-byte header). STUN bodies are
    // 32-bit aligned, and a packet claiming more body than arrived is truncated; either makes it
    // malformed, so it must not drive any allocate success/error decision.
    let body_len = ((data[2] as usize) << 8) | data[3] as usize;
    body_len.is_multiple_of(4) && data.len() >= 20 + body_len
}

pub fn is_allocate_or_binding_success(data: &[u8]) -> bool {
    is_complete_stun(data)
        && matches!(
            stun_message_type(data),
            Some(MSG_ALLOCATE_SUCCESS | MSG_BINDING_SUCCESS)
        )
}

pub fn is_allocate_error(data: &[u8]) -> bool {
    is_complete_stun(data) && stun_message_type(data) == Some(MSG_ALLOCATE_ERROR)
}

pub fn is_whatsapp_pong(data: &[u8], transaction_id: Option<&[u8]>) -> bool {
    if !is_stun_packet(data) || stun_message_type(data) != Some(MSG_WHATSAPP_PONG) {
        return false;
    }
    match transaction_id {
        None | Some(&[]) => true,
        Some(want) => stun_transaction_id(data) == Some(want),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StunAttribute<'a> {
    pub attr_type: u16,
    /// Borrows the packet: parsing a handshake's attributes no longer copies
    /// each value out (it was one heap `Vec` per attribute).
    pub value: &'a [u8],
}

/// The 4-byte STUN attribute TLV header (type, length), as a typed view so the
/// walk reads fields instead of doing index math per attribute.
#[derive(zerocopy::FromBytes, zerocopy::KnownLayout, zerocopy::Immutable, zerocopy::Unaligned)]
#[repr(C)]
struct StunAttrHeader {
    attr_type: zerocopy::big_endian::U16,
    length: zerocopy::big_endian::U16,
}

/// Parse the STUN attributes after the 20-byte header, borrowing each value.
pub fn parse_stun_attributes(data: &[u8]) -> Vec<StunAttribute<'_>> {
    if !is_stun_packet(data) || data.len() < 20 {
        return Vec::new();
    }
    let mut attrs = Vec::new();
    let mut off = 20;
    // `data.get(off..)` not `&data[off..]`: a truncated final padding can push
    // `off` past the end, and a bare slice there panics on an untrusted packet.
    while let Some(rest) = data.get(off..)
        && let Ok((hdr, _)) = zerocopy::Ref::<_, StunAttrHeader>::from_prefix(rest)
    {
        let len = hdr.length.get() as usize;
        off += 4;
        if off + len > data.len() {
            break;
        }
        attrs.push(StunAttribute {
            attr_type: hdr.attr_type.get(),
            value: &data[off..off + len],
        });
        off += len + pad4(len);
    }
    attrs
}

/// Parse the numeric error code (class*100 + number) from an Allocate-error response.
pub fn parse_stun_error_code(data: &[u8]) -> Option<u16> {
    // Defense-in-depth: reject incomplete/truncated packets regardless of caller order. A complete
    // STUN packet already satisfies the len/cookie checks, so this never rejects a valid one.
    if !is_complete_stun(data) {
        return None;
    }
    let t = stun_message_type(data)?;
    if t != MSG_ALLOCATE_ERROR && t != MSG_BINDING_ERROR {
        return None;
    }
    let body_len = ((data[2] as usize) << 8) | data[3] as usize;
    let end = (20 + body_len).min(data.len());
    let mut off = 20;
    while off + 4 <= end {
        let attr_type = ((data[off] as u16) << 8) | data[off + 1] as u16;
        let len = ((data[off + 2] as usize) << 8) | data[off + 3] as usize;
        // Bound the class/number read to the DECLARED body (`end`), not `data.len()`: an ERROR-CODE
        // whose header sits in-body but whose value bytes fall in trailing padding must not be trusted.
        if attr_type == ATTR_ERROR_CODE && len >= 4 && off + 8 <= end {
            let class = data[off + 6] as u16;
            let number = data[off + 7] as u16;
            return Some(class * 100 + number);
        }
        off += 4 + len + pad4(len);
    }
    None
}

const ATTR_SENDER_SUBSCRIPTIONS_V2: u16 = 0x4025;

// --- Minimal protobuf wire encoding for the STUN subscription attrs ---

use crate::voip::encode_varint as pb_varint;

fn pb_tag(out: &mut Vec<u8>, field: u32, wire: u32) {
    pb_varint(out, ((field << 3) | wire) as u64);
}

fn pb_zigzag(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

fn pb_len_delim(out: &mut Vec<u8>, field: u32, bytes: &[u8]) {
    pb_tag(out, field, 2);
    pb_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

/// `voip.SenderSubscriptions` (WASM, STUN attr 0x4000): one audio sender (ssrc as uint32).
pub fn create_voip_sender_subscriptions(ssrc: u32) -> Vec<u8> {
    let mut sender = Vec::new();
    pb_tag(&mut sender, 3, 0); // ssrc
    pb_varint(&mut sender, ssrc as u64);
    pb_tag(&mut sender, 5, 0); // stream_layer = AUDIO(0)
    pb_varint(&mut sender, 0);
    pb_tag(&mut sender, 6, 0); // payload_type = MEDIA(0)
    pb_varint(&mut sender, 0);
    let mut out = Vec::new();
    pb_len_delim(&mut out, 1, &sender); // senders[0]
    out
}

/// `wa.voip.SenderSubscriptions` (APK, STUN attr 0x4025): one audio ssrc (sint64), optional pid.
pub fn create_apk_sender_subscriptions(ssrc: u32, pid: Option<u32>) -> Vec<u8> {
    let mut ssrc_layers = Vec::new();
    pb_tag(&mut ssrc_layers, 1, 0); // ssrcs[0] (sint64, zigzag)
    pb_varint(&mut ssrc_layers, pb_zigzag(ssrc as i64));
    if let Some(pid) = pid {
        let mut p = Vec::new();
        pb_tag(&mut p, 1, 0); // pid (sint64)
        pb_varint(&mut p, pb_zigzag(pid as i64));
        pb_len_delim(&mut p, 2, b"audio"); // layerId
        pb_len_delim(&mut ssrc_layers, 2, &p); // pids[0]
    }
    let mut ext = Vec::new();
    pb_len_delim(&mut ext, 1, &ssrc_layers); // ssrcLayers
    let mut out = Vec::new();
    pb_len_delim(&mut out, 1, &ext); // subscriptions[0]
    out
}

/// `wa.voip.StreamDescriptors` (APK, STUN attr 0x4024): one audio/OPUS descriptor (ssrc sint64).
pub fn create_apk_stream_descriptors(ssrc: u32) -> Vec<u8> {
    let mut sd = Vec::new();
    pb_len_delim(&mut sd, 1, b"audio"); // stream_layer
    pb_len_delim(&mut sd, 2, b"OPUS"); // payload_type
    pb_tag(&mut sd, 3, 0); // ssrc (sint64)
    pb_varint(&mut sd, pb_zigzag(ssrc as i64));
    pb_tag(&mut sd, 4, 0); // is_uplink_prefetch_enabled = false
    pb_varint(&mut sd, 0);
    let mut out = Vec::new();
    pb_len_delim(&mut out, 1, &sd); // stream_descriptors[0]
    out
}

/// APK Allocate: 0x4000 token + 0x4025 sender subs + 0x4024 stream desc + MI.
pub fn build_android_stun_allocate_request(
    transaction_id: &[u8; 12],
    relay_token: &[u8],
    ssrc: u32,
    pid: Option<u32>,
    integrity_key: &[u8],
    include_fingerprint: bool,
) -> Vec<u8> {
    let mut attrs = stun_attr(ATTR_RELAY_TOKEN, relay_token);
    attrs.extend_from_slice(&stun_attr(
        ATTR_SENDER_SUBSCRIPTIONS_V2,
        &create_apk_sender_subscriptions(ssrc, pid),
    ));
    attrs.extend_from_slice(&stun_attr(
        STUN_ATTR_STREAM_DESCRIPTORS,
        &create_apk_stream_descriptors(ssrc),
    ));
    encode_stun_request(
        MSG_ALLOCATE_REQUEST,
        transaction_id,
        &attrs,
        Some(integrity_key),
        include_fingerprint,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::{hexd, kats};

    fn tx12(k: &serde_json::Value) -> [u8; 12] {
        let mut tx = [0u8; 12];
        tx.copy_from_slice(&hexd(k, &["stun", "tx"]));
        tx
    }

    #[test]
    fn crc32_is_ieee() {
        let k = kats();
        assert_eq!(
            crc32(b"abc") as u64,
            k["stun"]["crc32_abc"].as_u64().unwrap()
        );
        assert_eq!(crc32(b"abc"), 0x3524_41c2);
    }

    #[test]
    fn attr_and_endpoint_match_kat() {
        let k = kats();
        let token = hexd(&k, &["stun", "relayToken"]);
        assert_eq!(
            hex::encode(stun_attr(ATTR_RELAY_TOKEN, &token)),
            k["stun"]["attr_token"].as_str().unwrap()
        );
        let ep = encode_xor_relay_endpoint("157.240.226.133", 3478).unwrap();
        assert_eq!(hex::encode(ep), k["stun"]["xorEndpoint"].as_str().unwrap());
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        assert_eq!(
            hex::encode(create_native_sender_subscription(ssrc)),
            k["stun"]["nativeSenderSub"].as_str().unwrap()
        );
    }

    #[test]
    fn encode_request_mi_and_fingerprint_match_kat() {
        let k = kats();
        let tx = tx12(&k);
        let token = hexd(&k, &["stun", "relayToken"]);
        let mi_key = hexd(&k, &["stun", "miKey"]);
        let attrs = stun_attr(ATTR_RELAY_TOKEN, &token);
        let minimal = encode_stun_request(MSG_ALLOCATE_REQUEST, &tx, &attrs, Some(&mi_key), false);
        assert_eq!(
            hex::encode(&minimal),
            k["stun"]["minimalMi"].as_str().unwrap()
        );
        let with_fp = encode_stun_request(MSG_ALLOCATE_REQUEST, &tx, &attrs, Some(&mi_key), true);
        assert_eq!(hex::encode(&with_fp), k["stun"]["withFp"].as_str().unwrap());
    }

    #[test]
    fn truncated_stun_drives_no_allocate_decision() {
        let tx = [0u8; 12];
        // A bare success (no attributes, body length 0) is a complete packet and is accepted.
        let ok = encode_stun_request(MSG_ALLOCATE_SUCCESS, &tx, &[], None, false);
        assert!(is_allocate_or_binding_success(&ok));

        // Right type + magic cookie, but the message-length field claims a 64-byte body that never
        // arrived: neither the success nor the error path may fire (else garbage clears the deadline
        // or terminates the call).
        let mut truncated_ok = ok.clone();
        truncated_ok[2] = 0x00;
        truncated_ok[3] = 0x40;
        assert!(!is_allocate_or_binding_success(&truncated_ok));

        let mut truncated_err = encode_stun_request(MSG_ALLOCATE_ERROR, &tx, &[], None, false);
        truncated_err[2] = 0x00;
        truncated_err[3] = 0x40;
        assert!(!is_allocate_error(&truncated_err));
    }

    #[test]
    fn error_code_value_beyond_body_is_not_parsed() {
        // Allocate-error header, declared body length = 4, magic cookie, zero txid (20-byte header).
        let mut pkt = vec![0x01, 0x13, 0x00, 0x04];
        pkt.extend_from_slice(&STUN_MAGIC.to_be_bytes());
        pkt.extend_from_slice(&[0u8; 12]);
        // ERROR-CODE attr header (type 0x0009, len 4) fills the declared 4-byte body (offset 20..24).
        pkt.extend_from_slice(&[0x00, 0x09, 0x00, 0x04]);
        // Its class/number value bytes sit in TRAILING padding past the declared body (offset 24..28).
        pkt.extend_from_slice(&[0x00, 0x00, 0x04, 0x01]);

        // The packet is "complete" (len >= 20 + body_len), so is_allocate_error accepts it...
        assert!(is_allocate_error(&pkt));
        // ...but the ERROR-CODE value lies beyond the declared body, so it must not be parsed, and
        // the engine must therefore not terminate the call on it.
        assert_eq!(parse_stun_error_code(&pkt), None);
    }

    #[test]
    fn unaligned_stun_body_length_is_rejected() {
        let tx = [0u8; 12];
        // Right type + magic cookie and the claimed body byte is present, but a non-multiple-of-4
        // length is malformed STUN, so it must not drive an allocate-success decision.
        let mut pkt = encode_stun_request(MSG_ALLOCATE_SUCCESS, &tx, &[], None, false);
        pkt.push(0xAA);
        pkt[2] = 0x00;
        pkt[3] = 0x01;
        assert!(!is_allocate_or_binding_success(&pkt));
    }

    fn dv(b: &[u8], i: &mut usize) -> u64 {
        let (mut val, mut shift) = (0u64, 0u32);
        loop {
            let byte = b[*i];
            *i += 1;
            val |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                return val;
            }
            shift += 7;
        }
    }

    /// Decode the WASM StreamDescriptors protobuf into `(stream_index, sub_type) -> ssrc`.
    fn decode_stream_descriptors(buf: &[u8]) -> std::collections::HashMap<(u32, u32), u32> {
        let mut out = std::collections::HashMap::new();
        let mut i = 0;
        while i < buf.len() {
            assert_eq!(dv(buf, &mut i), (1 << 3) | 2, "top-level repeated field 1");
            let end = i + dv(buf, &mut i) as usize;
            let (mut sidx, mut sub, mut ssrc) = (0u32, 0u32, None);
            while i < end {
                let field = dv(buf, &mut i) >> 3;
                match field {
                    1 => sidx = dv(buf, &mut i) as u32,
                    2 => sub = dv(buf, &mut i) as u32,
                    3 => ssrc = Some(dv(buf, &mut i) as u32),
                    other => panic!("unexpected descriptor field {other}"),
                }
            }
            out.insert((sidx, sub), ssrc.expect("descriptor carries an ssrc"));
        }
        out
    }

    #[test]
    fn wasm_allocate_carries_dynamic_stream_descriptors() {
        let k = kats();
        let tx = tx12(&k);
        let token = hexd(&k, &["stun", "relayToken"]);
        let mi_key = hexd(&k, &["stun", "miKey"]);
        let ep = encode_xor_relay_endpoint("157.240.226.133", 3478).unwrap();
        let call_id = "CALL-ID-0001";
        let participant = crate::voip::ssrc::format_e2e_srtp_participant_id("12345:0@lid");
        let alloc =
            build_wasm_stun_allocate_request(&tx, &token, &ep, &mi_key, call_id, &participant);

        let attrs = parse_stun_attributes(&alloc);
        assert_eq!(attrs[0].attr_type, ATTR_RELAY_TOKEN);
        let sd = attrs
            .iter()
            .find(|a| a.attr_type == STUN_ATTR_STREAM_DESCRIPTORS)
            .expect("stream descriptors attr present");
        assert_eq!(
            sd.value,
            create_wasm_stream_descriptors(call_id, &participant)
        );
        assert!(
            attrs
                .iter()
                .any(|a| a.attr_type == STUN_ATTR_WASM_RELAY_ENDPOINT)
        );
        let mi = attrs
            .iter()
            .find(|a| a.attr_type == ATTR_MESSAGE_INTEGRITY)
            .expect("message integrity present");
        assert_eq!(mi.value.len(), 20);

        assert_eq!(
            hex::encode(build_whatsapp_ping(&tx)),
            k["stun"]["ping"].as_str().unwrap()
        );
    }

    #[test]
    fn stream_descriptors_announce_the_live_media_ssrcs() {
        let call_id = "CALL-ID-0001";
        let participant = crate::voip::ssrc::format_e2e_srtp_participant_id("12345:0@lid");
        let entries =
            decode_stream_descriptors(&create_wasm_stream_descriptors(call_id, &participant));

        // The video0 media descriptor must name the VideoPipeline SSRC.
        assert_eq!(
            entries.get(&(1, 0)).copied(),
            Some(crate::voip::ssrc::derive_video_participant_ssrc(
                call_id,
                &participant
            ))
        );
        assert_eq!(
            entries.get(&(0, 0)).copied(),
            Some(crate::voip::ssrc::derive_wasm_participant_ssrc(
                call_id,
                &participant,
                0
            ))
        );
        // audio + video0 + video1, each with media/FEC/NACK.
        assert_eq!(entries.len(), 9);
    }

    #[test]
    fn parse_round_trips_attributes() {
        let k = kats();
        let minimal = hexd(&k, &["stun", "minimalMi"]);
        assert!(is_stun_packet(&minimal));
        assert_eq!(stun_message_type(&minimal), Some(MSG_ALLOCATE_REQUEST));
        let attrs = parse_stun_attributes(&minimal);
        // relay token (0x4000) + message-integrity (0x0008)
        assert_eq!(attrs.len(), 2);
        assert_eq!(attrs[0].attr_type, ATTR_RELAY_TOKEN);
        assert_eq!(attrs[0].value, hexd(&k, &["stun", "relayToken"]));
        assert_eq!(attrs[1].attr_type, ATTR_MESSAGE_INTEGRITY);
        assert_eq!(attrs[1].value.len(), 20);
    }

    /// A truncated final padding pushes the cursor past the packet end; the walk
    /// must stop, not panic slicing `data[off..]` on an untrusted relay packet.
    #[test]
    fn parse_attributes_truncated_padding_does_not_panic() {
        // 20-byte STUN header + one attribute: type, length=1, one value byte,
        // and no padding bytes at all. Consuming it advances off to 28 with only
        // 25 bytes present.
        let mut p = vec![0u8; 20];
        p[0] = 0x00; // STUN-looking first byte
        p.extend_from_slice(&[0x40, 0x00, 0x00, 0x01, 0xAB]);
        assert_eq!(p.len(), 25);
        let attrs = parse_stun_attributes(&p);
        assert_eq!(attrs.len(), 1);
        assert_eq!(attrs[0].value, [0xAB]);
    }

    #[test]
    fn protobuf_payloads_match_kat() {
        let k = kats();
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        assert_eq!(
            hex::encode(create_voip_sender_subscriptions(ssrc)),
            k["stun_proto"]["voip_sender_subscriptions"]
                .as_str()
                .unwrap()
        );
        assert_eq!(
            hex::encode(create_apk_sender_subscriptions(ssrc, None)),
            k["stun_proto"]["apk_sender_subscriptions_nopid"]
                .as_str()
                .unwrap()
        );
        assert_eq!(
            hex::encode(create_apk_sender_subscriptions(ssrc, Some(7))),
            k["stun_proto"]["apk_sender_subscriptions_pid"]
                .as_str()
                .unwrap()
        );
        assert_eq!(
            hex::encode(create_apk_stream_descriptors(ssrc)),
            k["stun_proto"]["apk_stream_descriptors"].as_str().unwrap()
        );
    }

    #[test]
    fn android_allocate_carries_three_attrs() {
        let k = kats();
        let tx = tx12(&k);
        let token = hexd(&k, &["stun", "relayToken"]);
        let mi_key = hexd(&k, &["stun", "miKey"]);
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        let pkt = build_android_stun_allocate_request(&tx, &token, ssrc, None, &mi_key, false);
        let attrs = parse_stun_attributes(&pkt);
        // 0x4000 token, 0x4025 sender subs, 0x4024 stream desc, 0x0008 MI
        assert_eq!(attrs[0].attr_type, ATTR_RELAY_TOKEN);
        assert_eq!(attrs[1].attr_type, ATTR_SENDER_SUBSCRIPTIONS_V2);
        assert_eq!(attrs[2].attr_type, STUN_ATTR_STREAM_DESCRIPTORS);
        assert_eq!(attrs[3].attr_type, ATTR_MESSAGE_INTEGRITY);
        assert_eq!(attrs[2].value, create_apk_stream_descriptors(ssrc));
    }

    #[test]
    fn pong_matching() {
        let k = kats();
        let tx = tx12(&k);
        let mut pong = build_whatsapp_ping(&tx).to_vec();
        pong[0..2].copy_from_slice(&MSG_WHATSAPP_PONG.to_be_bytes());
        assert!(is_whatsapp_pong(&pong, Some(&tx)));
        assert!(is_whatsapp_pong(&pong, None));
        let wrong_tx = [0u8; 12];
        assert!(!is_whatsapp_pong(&pong, Some(&wrong_tx)));
    }
}
