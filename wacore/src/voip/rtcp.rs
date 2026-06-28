//! RTCP: WhatsApp compact reports (PT 208/209) and a Sender Report (PT 200).
//! The SR's NTP timestamp is taken as a `now_ms` argument so this stays
//! pure/no-clock (caller passes `wacore::time`).

use crate::voip::rtp::RTP_PAYLOAD_TYPE_OPUS;

pub const RTCP_PT_SR: u8 = 200;
pub const RTCP_PT_WA_COMPACT: u8 = 208;
pub const RTCP_PT_WA_COMPACT2: u8 = 209;
pub const RTCP_HEADER_LEN: usize = 8;
pub const SRTCP_TRAILER_LEN: usize = 14;

/// NTP epoch offset (seconds between 1900-01-01 and 1970-01-01).
const NTP_UNIX_OFFSET_SECS: u64 = 2_208_988_800;

pub fn is_rtcp_packet(data: &[u8]) -> bool {
    if data.len() < RTCP_HEADER_LEN + SRTCP_TRAILER_LEN {
        return false;
    }
    if (data[0] >> 6) & 0x03 != 2 {
        return false;
    }
    // WhatsApp RTP uses X=1 (byte0 0x90) and a 7-bit PT in byte1; RTCP uses the full byte1 as PT.
    if data[0] & 0x10 != 0 && data[1] & 0x7f == RTP_PAYLOAD_TYPE_OPUS {
        return false;
    }
    data[1] >= 64
}

pub fn rtcp_payload_type(data: &[u8]) -> Option<u8> {
    is_rtcp_packet(data).then(|| data[1])
}

pub fn parse_rtcp_sender_ssrc(data: &[u8]) -> Option<u32> {
    if data.len() < 8 || (data[0] >> 6) & 0x03 != 2 {
        return None;
    }
    Some(u32::from_be_bytes([data[4], data[5], data[6], data[7]]))
}

#[derive(Clone, Copy, Debug)]
pub struct RtcpSenderStats {
    pub packets_sent: u32,
    pub octets_sent: u32,
    pub rtp_timestamp: u32,
}

/// 12-byte compact RTCP (PT 208, RC=1); 26 bytes on the wire with the SRTCP trailer.
pub fn build_compact_rtcp_208(local_ssrc: u32, remote_ssrc: u32) -> [u8; 12] {
    let mut buf = [0u8; 12];
    buf[0] = 0x81; // V=2, P=0, RC=1
    buf[1] = RTCP_PT_WA_COMPACT;
    buf[2] = 0;
    buf[3] = 2; // (2+1)*4 = 12 bytes
    buf[4..8].copy_from_slice(&local_ssrc.to_be_bytes());
    buf[8..12].copy_from_slice(&remote_ssrc.to_be_bytes());
    buf
}

/// 8-byte compact RTCP (PT 209, RC=1): pre-speech ladder.
pub fn build_compact_rtcp_209(local_ssrc: u32) -> [u8; 8] {
    let mut buf = [0u8; 8];
    buf[0] = 0x81;
    buf[1] = RTCP_PT_WA_COMPACT2;
    buf[2] = 0;
    buf[3] = 1; // (1+1)*4 = 8 bytes
    buf[4..8].copy_from_slice(&local_ssrc.to_be_bytes());
    buf
}

/// 28-byte Sender Report (PT 200, RC=0). `now_ms` is the wall clock in milliseconds.
pub fn build_sender_report(local_ssrc: u32, stats: &RtcpSenderStats, now_ms: u64) -> [u8; 28] {
    let mut buf = [0u8; 28];
    buf[0] = 0x80; // V=2, RC=0
    buf[1] = RTCP_PT_SR;
    buf[2] = 0;
    buf[3] = 6; // (6+1)*4 = 28 bytes
    buf[4..8].copy_from_slice(&local_ssrc.to_be_bytes());
    // NTP timestamp: seconds (upper 32) since 1900, fraction (lower 32). Both fields
    // truncate to u32 (wrapping), matching the reference encoder.
    let ntp_sec = (now_ms / 1000).wrapping_add(NTP_UNIX_OFFSET_SECS) as u32;
    let ntp_frac = ((now_ms % 1000) as f64 / 1000.0 * 4_294_967_296.0) as u32;
    buf[8..12].copy_from_slice(&ntp_sec.to_be_bytes());
    buf[12..16].copy_from_slice(&ntp_frac.to_be_bytes());
    buf[16..20].copy_from_slice(&stats.rtp_timestamp.to_be_bytes());
    buf[20..24].copy_from_slice(&stats.packets_sent.to_be_bytes());
    buf[24..28].copy_from_slice(&stats.octets_sent.to_be_bytes());
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::kats;

    #[test]
    fn compact_reports_match_kat() {
        let k = kats();
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        let remote = k["rtcp"]["remoteSsrc"].as_u64().unwrap() as u32;
        assert_eq!(
            hex::encode(build_compact_rtcp_208(ssrc, remote)),
            k["rtcp"]["compact208"].as_str().unwrap()
        );
        assert_eq!(
            hex::encode(build_compact_rtcp_209(ssrc)),
            k["rtcp"]["compact209"].as_str().unwrap()
        );
    }

    #[test]
    fn sender_report_matches_kat() {
        let k = kats();
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        let now_ms = k["rtcp"]["nowMs"].as_u64().unwrap();
        let stats = RtcpSenderStats {
            packets_sent: k["rtcp"]["stats"]["packetsSent"].as_u64().unwrap() as u32,
            octets_sent: k["rtcp"]["stats"]["octetsSent"].as_u64().unwrap() as u32,
            rtp_timestamp: k["rtcp"]["stats"]["rtpTimestamp"].as_u64().unwrap() as u32,
        };
        assert_eq!(
            hex::encode(build_sender_report(ssrc, &stats, now_ms)),
            k["rtcp"]["senderReport"].as_str().unwrap()
        );
    }

    #[test]
    fn rtcp_classification() {
        let k = kats();
        let sr = hex::decode(k["rtcp"]["senderReport"].as_str().unwrap()).unwrap();
        // SR alone is 28 bytes (>= 8+14), V=2, PT=200 >= 64.
        assert!(is_rtcp_packet(&sr));
        assert_eq!(rtcp_payload_type(&sr), Some(200));
        // An RTP speech header (X=1, PT=120) must NOT be classified as RTCP.
        let rtp = hex::decode(k["rtp"]["speechHeader16"].as_str().unwrap()).unwrap();
        let mut padded = rtp.clone();
        padded.resize(40, 0);
        assert!(!is_rtcp_packet(&padded));
    }
}
