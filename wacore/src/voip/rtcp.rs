//! RTCP: standard sender/source reports and WhatsApp compact reports (PT 208/209).
//! The SR's NTP timestamp is taken as a `now_ms` argument so this stays
//! pure/no-clock (caller passes `wacore::time`).

pub const RTCP_PT_SR: u8 = 200;
pub const RTCP_PT_RR: u8 = 201;
pub const RTCP_PT_SDES: u8 = 202;
pub const RTCP_PT_RTPFB: u8 = 205;
pub const RTCP_PT_PSFB: u8 = 206;
pub const RTCP_PT_WA_COMPACT: u8 = 208;
pub const RTCP_PT_WA_COMPACT2: u8 = 209;
pub const RTCP_HEADER_LEN: usize = 8;
pub const SRTCP_TRAILER_LEN: usize = 14;
pub const WHATSAPP_RTCP_CNAME_LEN: usize = 18;

/// NTP epoch offset (seconds between 1900-01-01 and 1970-01-01).
const NTP_UNIX_OFFSET_SECS: u64 = 2_208_988_800;

pub fn is_rtcp_packet(data: &[u8]) -> bool {
    data.len() >= RTCP_HEADER_LEN && (data[0] >> 6) & 0x03 == 2 && (192..=223).contains(&data[1])
}

pub fn rtcp_payload_type(data: &[u8]) -> Option<u8> {
    is_rtcp_packet(data).then(|| data[1])
}

pub fn parse_rtcp_sender_ssrc(data: &[u8]) -> Option<u32> {
    if !is_rtcp_packet(data) {
        return None;
    }
    Some(u32::from_be_bytes([data[4], data[5], data[6], data[7]]))
}

/// Fields useful for proving whether the peer reports or requests one of our streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpSummary {
    pub packet_types: Vec<u8>,
    pub sender_ssrc: u32,
    pub referenced_ssrcs: Vec<u32>,
    pub report_blocks: Vec<RtcpReportBlock>,
    pub feedback: Vec<RtcpFeedback>,
    pub sdes_cname_lengths: Vec<usize>,
    /// The native video profile sets bit 4 separately from the four-bit report count.
    pub uses_whatsapp_profile_extension: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpFeedback {
    pub packet_type: u8,
    pub fmt: u8,
    pub sender_ssrc: u32,
    pub media_ssrc: u32,
    pub fci: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtcpReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: i32,
    pub extended_highest_sequence: u32,
    pub jitter: u32,
    pub last_sender_report: u32,
    pub delay_since_last_sender_report: u32,
    /// WhatsApp appends per-stream fields after the RFC 3550 block.
    pub profile_extension: Vec<u8>,
}

fn parse_sdes_cname_lengths(packet: &[u8], source_count: usize) -> Option<Vec<usize>> {
    let mut cursor = 4usize;
    let mut cname_lengths = Vec::new();
    for _ in 0..source_count {
        cursor = cursor.checked_add(4)?;
        packet.get(cursor - 4..cursor)?;
        loop {
            let item_type = *packet.get(cursor)?;
            cursor += 1;
            if item_type == 0 {
                while cursor & 3 != 0 {
                    if *packet.get(cursor)? != 0 {
                        return None;
                    }
                    cursor += 1;
                }
                break;
            }
            let item_len = *packet.get(cursor)? as usize;
            cursor += 1;
            packet.get(cursor..cursor.checked_add(item_len)?)?;
            if item_type == 1 {
                cname_lengths.push(item_len);
            }
            cursor += item_len;
        }
    }
    (cursor == packet.len()).then_some(cname_lengths)
}

fn parse_report_blocks(
    packet: &[u8],
    start: usize,
    raw_count: usize,
) -> Option<(Vec<RtcpReportBlock>, bool)> {
    let available = packet.len().checked_sub(start)?;
    let mut report_count = raw_count;
    let mut uses_whatsapp_profile_extension = false;
    if available < report_count.checked_mul(24)? && raw_count & 0x10 != 0 {
        report_count = raw_count & 0x0f;
        uses_whatsapp_profile_extension = true;
    }
    if report_count == 0 {
        return (available == 0).then_some((Vec::new(), uses_whatsapp_profile_extension));
    }
    if available < report_count.checked_mul(24)? {
        return None;
    }

    let stride = if available % report_count == 0 {
        available / report_count
    } else {
        24
    };
    if stride < 24 {
        return None;
    }

    let mut reports = Vec::with_capacity(report_count);
    for index in 0..report_count {
        let at = start.checked_add(index.checked_mul(stride)?)?;
        let block = packet.get(at..at.checked_add(stride)?)?;
        let cumulative_raw = u32::from_be_bytes([0, block[5], block[6], block[7]]) as i32;
        let cumulative_lost = if cumulative_raw & 0x80_0000 != 0 {
            cumulative_raw | !0x00ff_ffff
        } else {
            cumulative_raw
        };
        reports.push(RtcpReportBlock {
            ssrc: u32::from_be_bytes(block[0..4].try_into().ok()?),
            fraction_lost: block[4],
            cumulative_lost,
            extended_highest_sequence: u32::from_be_bytes(block[8..12].try_into().ok()?),
            jitter: u32::from_be_bytes(block[12..16].try_into().ok()?),
            last_sender_report: u32::from_be_bytes(block[16..20].try_into().ok()?),
            delay_since_last_sender_report: u32::from_be_bytes(block[20..24].try_into().ok()?),
            profile_extension: block[24..].to_vec(),
        });
    }
    Some((reports, uses_whatsapp_profile_extension))
}

/// Parse a decrypted RTCP compound packet. Referenced SSRCs include SR/RR report blocks and the
/// media targets of transport/payload feedback (NACK, PLI, FIR).
pub fn summarize_rtcp(data: &[u8]) -> Option<RtcpSummary> {
    let mut offset = 0usize;
    let mut packet_types = Vec::new();
    let mut referenced_ssrcs = Vec::new();
    let mut report_blocks = Vec::new();
    let mut feedback = Vec::new();
    let mut sdes_cname_lengths = Vec::new();
    let mut sender_ssrc = None;
    let mut uses_whatsapp_profile_extension = false;

    while offset < data.len() {
        let header = data.get(offset..offset + 4)?;
        if (header[0] >> 6) & 0x03 != 2 || !(192..=223).contains(&header[1]) {
            return None;
        }
        let words = u16::from_be_bytes([header[2], header[3]]) as usize + 1;
        let packet_len = words.checked_mul(4)?;
        let packet = data.get(offset..offset.checked_add(packet_len)?)?;
        if packet.len() < RTCP_HEADER_LEN {
            return None;
        }

        let packet_type = packet[1];
        let raw_count = (packet[0] & 0x1f) as usize;
        let this_sender = u32::from_be_bytes(packet[4..8].try_into().ok()?);
        sender_ssrc.get_or_insert(this_sender);
        packet_types.push(packet_type);

        let report_start = match packet_type {
            RTCP_PT_SR => Some(28),
            RTCP_PT_RR => Some(8),
            _ => None,
        };
        if let Some(start) = report_start {
            let (mut parsed, uses_profile) = parse_report_blocks(packet, start, raw_count)?;
            uses_whatsapp_profile_extension |= uses_profile;
            referenced_ssrcs.extend(parsed.iter().map(|report| report.ssrc));
            report_blocks.append(&mut parsed);
        }

        if matches!(packet_type, RTCP_PT_RTPFB | RTCP_PT_PSFB) && packet.len() >= 12 {
            let media_ssrc = u32::from_be_bytes(packet[8..12].try_into().ok()?);
            feedback.push(RtcpFeedback {
                packet_type,
                fmt: raw_count as u8,
                sender_ssrc: this_sender,
                media_ssrc,
                fci: packet[12..].to_vec(),
            });
            if media_ssrc != 0 {
                referenced_ssrcs.push(media_ssrc);
            }
            // FIR commonly leaves the media-source field zero and names targets in 8-byte FCI rows.
            if packet_type == RTCP_PT_PSFB && raw_count == 4 {
                for fci in packet[12..].chunks_exact(8) {
                    referenced_ssrcs.push(u32::from_be_bytes(fci[..4].try_into().ok()?));
                }
            }
        } else if packet_type == RTCP_PT_WA_COMPACT && packet.len() >= 12 {
            referenced_ssrcs.push(u32::from_be_bytes(packet[8..12].try_into().ok()?));
        } else if packet_type == RTCP_PT_SDES {
            let (parsed, uses_profile) =
                if let Some(parsed) = parse_sdes_cname_lengths(packet, raw_count) {
                    (parsed, false)
                } else if raw_count & 0x10 != 0 {
                    (parse_sdes_cname_lengths(packet, raw_count & 0x0f)?, true)
                } else {
                    return None;
                };
            uses_whatsapp_profile_extension |= uses_profile;
            sdes_cname_lengths.extend(parsed);
        }

        offset += packet_len;
    }

    referenced_ssrcs.sort_unstable();
    referenced_ssrcs.dedup();
    Some(RtcpSummary {
        packet_types,
        sender_ssrc: sender_ssrc?,
        referenced_ssrcs,
        report_blocks,
        feedback,
        sdes_cname_lengths,
        uses_whatsapp_profile_extension,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct RtcpSenderStats {
    pub packets_sent: u32,
    pub octets_sent: u32,
    pub rtp_timestamp: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RtcpReceptionReport {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: i32,
    pub extended_highest_sequence: u32,
    pub jitter: u32,
    pub last_sender_report: u32,
    pub delay_since_last_sender_report: u32,
}

#[derive(Debug, Default)]
pub(crate) struct RtpReceptionStats {
    ssrc: Option<u32>,
    base_sequence: u16,
    max_sequence: u16,
    sequence_cycles: u32,
    received: u32,
    expected_prior: u32,
    received_prior: u32,
    transit: Option<u32>,
    jitter_q4: u64,
    last_sender_report: u32,
    last_sender_report_at_ms: Option<u64>,
}

impl RtpReceptionStats {
    pub(crate) fn observe(
        &mut self,
        ssrc: u32,
        sequence: u16,
        rtp_timestamp: u32,
        arrival_ms: u64,
        clock_rate: u32,
    ) {
        if self.ssrc != Some(ssrc) {
            *self = Self {
                ssrc: Some(ssrc),
                base_sequence: sequence,
                max_sequence: sequence,
                received: 1,
                ..Self::default()
            };
        } else {
            let delta = sequence.wrapping_sub(self.max_sequence);
            if delta != 0 && delta < 0x8000 {
                if sequence < self.max_sequence {
                    self.sequence_cycles = self.sequence_cycles.wrapping_add(1 << 16);
                }
                self.max_sequence = sequence;
            }
            self.received = self.received.wrapping_add(1);
        }

        let arrival_rtp = ((arrival_ms as u128 * clock_rate as u128) / 1000) as u32;
        let transit = arrival_rtp.wrapping_sub(rtp_timestamp);
        if let Some(previous) = self.transit {
            let delta = transit.wrapping_sub(previous) as i32;
            let distance = (delta as i64).unsigned_abs();
            let decay = (self.jitter_q4 + 8) >> 4;
            self.jitter_q4 = self
                .jitter_q4
                .saturating_add(distance)
                .saturating_sub(decay);
        }
        self.transit = Some(transit);
    }

    pub(crate) fn observe_sender_report(
        &mut self,
        sender_ssrc: u32,
        ntp_seconds: u32,
        ntp_fraction: u32,
        arrival_ms: u64,
    ) {
        if self.ssrc != Some(sender_ssrc) {
            return;
        }
        self.last_sender_report = (ntp_seconds << 16) | (ntp_fraction >> 16);
        self.last_sender_report_at_ms = Some(arrival_ms);
    }

    pub(crate) fn report(&mut self, now_ms: u64) -> Option<RtcpReceptionReport> {
        let ssrc = self.ssrc?;
        let expected = self
            .sequence_cycles
            .wrapping_add(self.max_sequence as u32)
            .wrapping_sub(self.base_sequence as u32)
            .wrapping_add(1);
        let expected_interval = expected.wrapping_sub(self.expected_prior);
        let received_interval = self.received.wrapping_sub(self.received_prior);
        let lost_interval = expected_interval as i64 - received_interval as i64;
        let fraction_lost = if expected_interval == 0 || lost_interval <= 0 {
            0
        } else {
            ((lost_interval * 256) / expected_interval as i64).min(255) as u8
        };
        self.expected_prior = expected;
        self.received_prior = self.received;

        let cumulative_lost =
            (expected as i64 - self.received as i64).clamp(-0x80_0000, 0x7f_ffff) as i32;
        let delay_since_last_sender_report = self
            .last_sender_report_at_ms
            .map(|at| {
                let elapsed = now_ms.saturating_sub(at);
                ((elapsed as u128 * 65_536) / 1000).min(u32::MAX as u128) as u32
            })
            .unwrap_or(0);

        Some(RtcpReceptionReport {
            ssrc,
            fraction_lost,
            cumulative_lost,
            extended_highest_sequence: self.sequence_cycles | self.max_sequence as u32,
            jitter: (self.jitter_q4 >> 4).min(u32::MAX as u64) as u32,
            last_sender_report: self.last_sender_report,
            delay_since_last_sender_report,
        })
    }
}

pub(crate) fn parse_sender_report_timing(data: &[u8]) -> Option<(u32, u32, u32)> {
    let mut offset = 0usize;
    while offset < data.len() {
        let header = data.get(offset..offset + 4)?;
        let packet_len =
            (u16::from_be_bytes([header[2], header[3]]) as usize + 1).checked_mul(4)?;
        let packet = data.get(offset..offset.checked_add(packet_len)?)?;
        if header[1] == RTCP_PT_SR && packet.len() >= 28 {
            return Some((
                u32::from_be_bytes(packet[4..8].try_into().ok()?),
                u32::from_be_bytes(packet[8..12].try_into().ok()?),
                u32::from_be_bytes(packet[12..16].try_into().ok()?),
            ));
        }
        offset += packet_len;
    }
    None
}

/// Native WhatsApp streams use eleven random lowercase hex nibbles around a fixed `@pj...org`
/// suffix. Production entropy comes from the engine's injected random source.
pub fn build_whatsapp_rtcp_cname(entropy: &[u8; 12]) -> [u8; WHATSAPP_RTCP_CNAME_LEN] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut random_hex = [0u8; 11];
    for (nibble, out) in random_hex.iter_mut().enumerate() {
        let byte = entropy[6 + nibble / 2];
        let value = if nibble & 1 == 0 {
            byte >> 4
        } else {
            byte & 0x0f
        };
        *out = HEX[value as usize];
    }
    let mut cname = [0u8; WHATSAPP_RTCP_CNAME_LEN];
    cname[..5].copy_from_slice(&random_hex[..5]);
    cname[5..8].copy_from_slice(b"@pj");
    cname[8..14].copy_from_slice(&random_hex[5..]);
    cname[14..].copy_from_slice(b".org");
    cname
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

fn encode_reception_report(report: &RtcpReceptionReport, out: &mut Vec<u8>) {
    out.extend_from_slice(&report.ssrc.to_be_bytes());
    out.push(report.fraction_lost);
    let lost = report.cumulative_lost as u32;
    out.extend_from_slice(&lost.to_be_bytes()[1..]);
    out.extend_from_slice(&report.extended_highest_sequence.to_be_bytes());
    out.extend_from_slice(&report.jitter.to_be_bytes());
    out.extend_from_slice(&report.last_sender_report.to_be_bytes());
    out.extend_from_slice(&report.delay_since_last_sender_report.to_be_bytes());
}

const WHATSAPP_RECEPTION_REPORT_EXTENSION_LEN: usize = 24;

fn encode_whatsapp_reception_report(report: &RtcpReceptionReport, out: &mut Vec<u8>) {
    encode_reception_report(report, out);
    // These are optional transport/BWE statistics in the native 1:1 report. Zero is the
    // native unavailable value; the RFC fields above remain authoritative.
    out.resize(out.len() + WHATSAPP_RECEPTION_REPORT_EXTENSION_LEN, 0);
}

fn build_sender_report_with_reports(
    local_ssrc: u32,
    stats: &RtcpSenderStats,
    now_ms: u64,
    reports: &[RtcpReceptionReport],
) -> Vec<u8> {
    let reports = &reports[..reports.len().min(31)];
    let mut packet = build_sender_report(local_ssrc, stats, now_ms).to_vec();
    packet[0] |= reports.len() as u8;
    for report in reports {
        encode_reception_report(report, &mut packet);
    }
    let words = packet.len() / 4 - 1;
    packet[2..4].copy_from_slice(&(words as u16).to_be_bytes());
    packet
}

/// Native WhatsApp's 32-byte SDES packet: one chunk, its 18-byte CNAME, END, and padding.
pub fn build_source_description(
    local_ssrc: u32,
    cname: &[u8; WHATSAPP_RTCP_CNAME_LEN],
) -> [u8; 32] {
    let mut packet = [0u8; 32];
    packet[0] = 0x81;
    packet[1] = RTCP_PT_SDES;
    packet[2..4].copy_from_slice(&7u16.to_be_bytes());
    packet[4..8].copy_from_slice(&local_ssrc.to_be_bytes());
    packet[8] = 1;
    packet[9] = WHATSAPP_RTCP_CNAME_LEN as u8;
    packet[10..28].copy_from_slice(cname);
    packet
}

pub(crate) fn build_whatsapp_source_description(
    local_ssrc: u32,
    cname: &[u8; WHATSAPP_RTCP_CNAME_LEN],
    profile_extension: bool,
) -> [u8; 32] {
    let mut packet = build_source_description(local_ssrc, cname);
    if profile_extension {
        packet[0] |= 0x10;
    }
    packet
}

/// Native RTCP sessions without a CNAME still send an empty 12-byte SDES chunk.
pub fn build_empty_source_description(local_ssrc: u32) -> [u8; 12] {
    let mut packet = [0u8; 12];
    packet[0] = 0x81;
    packet[1] = RTCP_PT_SDES;
    packet[2..4].copy_from_slice(&2u16.to_be_bytes());
    packet[4..8].copy_from_slice(&local_ssrc.to_be_bytes());
    packet
}

/// WhatsApp's periodic sender report is a compound `SR + SDES` datagram.
pub fn build_sender_report_with_sdes(
    local_ssrc: u32,
    stats: &RtcpSenderStats,
    now_ms: u64,
    cname: &[u8; WHATSAPP_RTCP_CNAME_LEN],
) -> Vec<u8> {
    build_sender_report_with_sdes_and_reports(local_ssrc, stats, now_ms, cname, &[])
}

pub(crate) fn build_sender_report_with_sdes_and_reports(
    local_ssrc: u32,
    stats: &RtcpSenderStats,
    now_ms: u64,
    cname: &[u8; WHATSAPP_RTCP_CNAME_LEN],
    reports: &[RtcpReceptionReport],
) -> Vec<u8> {
    let sdes = build_source_description(local_ssrc, cname);
    let sender_report = build_sender_report_with_reports(local_ssrc, stats, now_ms, reports);
    let mut compound = Vec::with_capacity(sender_report.len() + sdes.len());
    compound.extend_from_slice(&sender_report);
    compound.extend_from_slice(&sdes);
    compound
}

/// The native 1:1 path associates one receive stream with each RTCP sender. Its report block is
/// the RFC 3550 block followed by 24 bytes of WhatsApp transport/BWE statistics.
pub(crate) fn build_whatsapp_sender_report_with_sdes(
    local_ssrc: u32,
    stats: &RtcpSenderStats,
    now_ms: u64,
    cname: &[u8; WHATSAPP_RTCP_CNAME_LEN],
    report: Option<&RtcpReceptionReport>,
    profile_extension: bool,
) -> Vec<u8> {
    let mut sender_report = build_sender_report(local_ssrc, stats, now_ms).to_vec();
    if profile_extension {
        sender_report[0] |= 0x10;
    }
    if let Some(report) = report {
        sender_report[0] |= 1;
        encode_whatsapp_reception_report(report, &mut sender_report);
        let words = sender_report.len() / 4 - 1;
        sender_report[2..4].copy_from_slice(&(words as u16).to_be_bytes());
    }
    let sdes = build_whatsapp_source_description(local_ssrc, cname, profile_extension);
    sender_report.extend_from_slice(&sdes);
    sender_report
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
    fn native_cname_and_sdes_match_wire_layout() {
        let entropy = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
        ];
        let cname = build_whatsapp_rtcp_cname(&entropy);
        assert_eq!(&cname, b"66778@pj899aab.org");

        let sdes = build_source_description(0x1234_5678, &cname);
        assert_eq!(
            hex::encode(sdes),
            "81ca0007123456780112363637373840706a3839396161622e6f726700000000"
        );
        let summary = summarize_rtcp(&sdes).unwrap();
        assert_eq!(summary.packet_types, [RTCP_PT_SDES]);
        assert_eq!(summary.sdes_cname_lengths, [WHATSAPP_RTCP_CNAME_LEN]);
    }

    #[test]
    fn empty_sdes_is_one_padded_chunk() {
        let sdes = build_empty_source_description(0x1234_5678);
        assert_eq!(hex::encode(sdes), "81ca00021234567800000000");
        let summary = summarize_rtcp(&sdes).unwrap();
        assert!(summary.sdes_cname_lengths.is_empty());
    }

    #[test]
    fn sender_report_compound_includes_sdes() {
        let stats = RtcpSenderStats {
            packets_sent: 5,
            octets_sent: 600,
            rtp_timestamp: 1600,
        };
        let cname = *b"66778@pj899aab.org";
        let compound =
            build_sender_report_with_sdes(0x1234_5678, &stats, 1_718_000_000_000, &cname);
        let summary = summarize_rtcp(&compound).unwrap();
        assert_eq!(summary.packet_types, [RTCP_PT_SR, RTCP_PT_SDES]);
        assert_eq!(summary.sdes_cname_lengths, [WHATSAPP_RTCP_CNAME_LEN]);
    }

    #[test]
    fn standard_sender_report_encodes_two_report_blocks() {
        let stats = RtcpSenderStats {
            packets_sent: 9,
            octets_sent: 4_096,
            rtp_timestamp: 90_000,
        };
        let reports = [
            RtcpReceptionReport {
                ssrc: 0x1111_2222,
                fraction_lost: 0x40,
                cumulative_lost: 2,
                extended_highest_sequence: 0x0001_0002,
                jitter: 123,
                last_sender_report: 0x3344_5566,
                delay_since_last_sender_report: 0x7788_99aa,
            },
            RtcpReceptionReport {
                ssrc: 0x3333_4444,
                fraction_lost: 0,
                cumulative_lost: -1,
                extended_highest_sequence: 7,
                jitter: 8,
                last_sender_report: 0,
                delay_since_last_sender_report: 0,
            },
        ];
        let compound = build_sender_report_with_sdes_and_reports(
            0x5555_6666,
            &stats,
            1_700_000_000_000,
            b"00000@pj000000.org",
            &reports,
        );

        assert_eq!(compound.len(), 76 + 32);
        assert_eq!(&compound[..4], &[0x82, RTCP_PT_SR, 0, 18]);
        assert_eq!(&compound[28..32], &reports[0].ssrc.to_be_bytes());
        assert_eq!(&compound[32..36], &[0x40, 0, 0, 2]);
        assert_eq!(&compound[52..56], &reports[1].ssrc.to_be_bytes());
        assert_eq!(&compound[56..60], &[0, 0xff, 0xff, 0xff]);
        assert_eq!(&compound[76..80], &[0x81, RTCP_PT_SDES, 0, 7]);

        let summary = summarize_rtcp(&compound).expect("valid SR+SDES compound");
        assert_eq!(summary.packet_types, [RTCP_PT_SR, RTCP_PT_SDES]);
        assert_eq!(summary.referenced_ssrcs, [reports[0].ssrc, reports[1].ssrc]);
    }

    #[test]
    fn parses_native_video_profile_report_and_sdes_layouts() {
        let sender = 0x1111_2222u32;
        let target = 0x3333_4444u32;
        let mut sender_report = vec![0x91, RTCP_PT_SR, 0, 18];
        sender_report.extend_from_slice(&sender.to_be_bytes());
        sender_report.extend_from_slice(&[0; 20]);
        sender_report.extend_from_slice(&target.to_be_bytes());
        sender_report.extend_from_slice(&[0x20, 0xff, 0xff, 0xfe]);
        sender_report.extend_from_slice(&0x0001_2345u32.to_be_bytes());
        sender_report.extend_from_slice(&77u32.to_be_bytes());
        sender_report.extend_from_slice(&0x5566_7788u32.to_be_bytes());
        sender_report.extend_from_slice(&0x0000_8000u32.to_be_bytes());
        let extension: Vec<u8> = (0..WHATSAPP_RECEPTION_REPORT_EXTENSION_LEN as u8).collect();
        sender_report.extend_from_slice(&extension);
        assert_eq!(sender_report.len(), 76);

        let mut full_sdes = build_source_description(sender, b"00000@pj000000.org");
        full_sdes[0] = 0x91;
        let mut full_compound = sender_report.clone();
        full_compound.extend_from_slice(&full_sdes);
        assert_eq!(full_compound.len(), 108);

        let summary = summarize_rtcp(&full_compound).expect("native full-SDES compound");
        assert_eq!(summary.packet_types, [RTCP_PT_SR, RTCP_PT_SDES]);
        assert_eq!(summary.referenced_ssrcs, [target]);
        assert!(summary.uses_whatsapp_profile_extension);
        assert_eq!(summary.sdes_cname_lengths, [WHATSAPP_RTCP_CNAME_LEN]);
        assert_eq!(
            summary.report_blocks,
            [RtcpReportBlock {
                ssrc: target,
                fraction_lost: 0x20,
                cumulative_lost: -2,
                extended_highest_sequence: 0x0001_2345,
                jitter: 77,
                last_sender_report: 0x5566_7788,
                delay_since_last_sender_report: 0x0000_8000,
                profile_extension: extension,
            }]
        );

        let mut empty_sdes = build_empty_source_description(sender);
        empty_sdes[0] = 0x91;
        let mut empty_compound = sender_report;
        empty_compound.extend_from_slice(&empty_sdes);
        assert_eq!(empty_compound.len(), 88);
        let summary = summarize_rtcp(&empty_compound).expect("native empty-SDES compound");
        assert_eq!(summary.packet_types, [RTCP_PT_SR, RTCP_PT_SDES]);
        assert!(summary.sdes_cname_lengths.is_empty());
        assert!(summary.uses_whatsapp_profile_extension);
    }

    #[test]
    fn native_video_sender_report_matches_profile_structure() {
        let report = RtcpReceptionReport {
            ssrc: 0x3333_4444,
            fraction_lost: 5,
            cumulative_lost: 2,
            extended_highest_sequence: 91,
            jitter: 7,
            last_sender_report: 8,
            delay_since_last_sender_report: 9,
        };
        let compound = build_whatsapp_sender_report_with_sdes(
            0x1111_2222,
            &RtcpSenderStats {
                packets_sent: 3,
                octets_sent: 400,
                rtp_timestamp: 90_000,
            },
            1_700_000_000_000,
            b"00000@pj000000.org",
            Some(&report),
            true,
        );

        assert_eq!(compound.len(), 108);
        assert_eq!(&compound[..4], &[0x91, RTCP_PT_SR, 0, 18]);
        assert_eq!(&compound[28..32], &report.ssrc.to_be_bytes());
        assert_eq!(&compound[52..76], &[0; 24]);
        assert_eq!(&compound[76..80], &[0x91, RTCP_PT_SDES, 0, 7]);
        let summary = summarize_rtcp(&compound).expect("native video compound");
        assert_eq!(summary.referenced_ssrcs, [report.ssrc]);
        assert_eq!(summary.report_blocks[0].profile_extension, [0; 24]);
        assert!(summary.uses_whatsapp_profile_extension);
    }

    #[test]
    fn reception_stats_track_loss_wrap_lsr_and_dlsr() {
        let mut reception = RtpReceptionStats::default();
        reception.observe(0x1234_5678, 65_534, 90_000, 1_000, 90_000);
        reception.observe(0x1234_5678, 65_535, 95_400, 1_060, 90_000);
        // Sequence zero is deliberately lost; sequence one proves the rollover.
        reception.observe(0x1234_5678, 1, 106_200, 1_180, 90_000);
        reception.observe_sender_report(0x1234_5678, 0x0102_0304, 0x5060_7080, 1_180);

        let report = reception.report(1_680).expect("observed RTP source");
        assert_eq!(report.ssrc, 0x1234_5678);
        assert_eq!(report.fraction_lost, 64);
        assert_eq!(report.cumulative_lost, 1);
        assert_eq!(report.extended_highest_sequence, 0x0001_0001);
        assert_eq!(report.jitter, 0);
        assert_eq!(report.last_sender_report, 0x0304_5060);
        assert_eq!(report.delay_since_last_sender_report, 32_768);

        let next = reception.report(1_680).expect("source remains active");
        assert_eq!(next.fraction_lost, 0);
        assert_eq!(next.cumulative_lost, 1);
    }

    #[test]
    fn parses_sender_report_ntp_timing_from_compound() {
        let report = build_sender_report(
            0x1234_5678,
            &RtcpSenderStats {
                packets_sent: 0,
                octets_sent: 0,
                rtp_timestamp: 0,
            },
            1_700_000_000_250,
        );
        assert_eq!(
            parse_sender_report_timing(&report),
            Some((
                0x1234_5678,
                (NTP_UNIX_OFFSET_SECS + 1_700_000_000) as u32,
                1 << 30,
            ))
        );
    }

    #[test]
    fn rtcp_classification() {
        let k = kats();
        let sr = hex::decode(k["rtcp"]["senderReport"].as_str().unwrap()).unwrap();
        assert!(is_rtcp_packet(&sr));
        assert_eq!(rtcp_payload_type(&sr), Some(200));
        // RTP uses a 7-bit PT. Marker+H264 is 0xe1, outside RTCP's mux range.
        let video = [0x80, 0xe1, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1];
        assert!(!is_rtcp_packet(&video));
        let rtp = hex::decode(k["rtp"]["speechHeader16"].as_str().unwrap()).unwrap();
        assert!(!is_rtcp_packet(&rtp));
    }

    #[test]
    fn summarizes_rr_and_feedback_targets() {
        let sender = 0x1111_2222u32;
        let audio = 0x3333_4444u32;
        let video = 0x5555_6666u32;

        // RR with one 24-byte report block.
        let mut compound = vec![0x81, RTCP_PT_RR, 0, 7];
        compound.extend_from_slice(&sender.to_be_bytes());
        compound.extend_from_slice(&audio.to_be_bytes());
        compound.extend_from_slice(&[0; 20]);
        // PLI for the video SSRC.
        compound.extend_from_slice(&[0x81, RTCP_PT_PSFB, 0, 2]);
        compound.extend_from_slice(&sender.to_be_bytes());
        compound.extend_from_slice(&video.to_be_bytes());

        let summary = summarize_rtcp(&compound).expect("valid compound RTCP");
        assert_eq!(summary.packet_types, [RTCP_PT_RR, RTCP_PT_PSFB]);
        assert_eq!(summary.sender_ssrc, sender);
        assert_eq!(summary.referenced_ssrcs, [audio, video]);
        assert_eq!(
            summary.feedback,
            [RtcpFeedback {
                packet_type: RTCP_PT_PSFB,
                fmt: 1,
                sender_ssrc: sender,
                media_ssrc: video,
                fci: Vec::new(),
            }]
        );
        assert!(summary.sdes_cname_lengths.is_empty());
    }

    #[test]
    fn summarizes_fir_target_from_feedback_control_information() {
        let sender = 0x1111_2222u32;
        let video = 0x5555_6666u32;
        let mut fir = vec![0x84, RTCP_PT_PSFB, 0, 4];
        fir.extend_from_slice(&sender.to_be_bytes());
        fir.extend_from_slice(&0u32.to_be_bytes());
        fir.extend_from_slice(&video.to_be_bytes());
        fir.extend_from_slice(&[7, 0, 0, 0]);

        let summary = summarize_rtcp(&fir).expect("valid FIR");

        assert_eq!(summary.referenced_ssrcs, [video]);
        assert_eq!(
            summary.feedback,
            [RtcpFeedback {
                packet_type: RTCP_PT_PSFB,
                fmt: 4,
                sender_ssrc: sender,
                media_ssrc: 0,
                fci: [video.to_be_bytes().as_slice(), &[7, 0, 0, 0]].concat(),
            }]
        );
    }

    #[test]
    fn malformed_compound_rtcp_is_rejected() {
        // Claims 32 bytes but only carries the common header.
        assert!(summarize_rtcp(&[0x80, RTCP_PT_RR, 0, 7, 0, 0, 0, 1]).is_none());
    }

    #[test]
    fn malformed_sdes_is_rejected() {
        let mut sdes = build_source_description(1, b"00000@pj000000.org");
        *sdes.last_mut().unwrap() = 1;
        assert!(summarize_rtcp(&sdes).is_none());
    }
}
