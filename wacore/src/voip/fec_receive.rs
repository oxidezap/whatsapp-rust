//! Inbound video FEC: the receiving half of [`super::fec`].
//!
//! A WhatsApp peer protects its video with repair packets (payload type
//! [`RTP_PAYLOAD_TYPE_VIDEO_FEC`](super::fec::RTP_PAYLOAD_TYPE_VIDEO_FEC)) that follow the packets
//! they protect. Reassembly reads packets strictly in arrival order and gives up on a picture the
//! moment one of its packets is missing, before the repair that could rebuild it arrives. So the
//! packets behind a sequence gap are held here, briefly, until the gap is filled -- by a late
//! packet or one rebuilt from a repair packet -- or the hold runs out and the gap is skipped.
//! Without a gap nothing is held and nothing is delayed.
//!
//! Everything here works on plaintext RTP packets: the header as it arrived (CSRCs and extension
//! included) followed by the decrypted payload, which is what a repair packet's parity covers.

use std::collections::VecDeque;

use super::fec::{FecHeader, recover_packet};
use super::rtp::{RTP_PAYLOAD_TYPE_H264, rtp_header_byte_length};

/// How long packets wait behind a gap for the packet that fills it. A repair packet follows the
/// span it closes, which for small pictures can be the next picture's; 100 ms covers that at the
/// frame rates WhatsApp sends, and is only ever spent while a gap is open.
const HOLD_MS: u64 = 100;
/// Packets held behind one gap at most, so a burst cannot grow the hold without bound.
const HOLD_MAX: usize = 128;
/// Packets kept for rebuilding: well past the widest span a repair packet's mask names.
const RECENT_MAX: usize = 256;
/// A forward jump this far is a restarted stream, not a gap worth waiting on.
const RESTART_JUMP: i16 = 1000;

/// What inbound FEC did since the last [`InboundFec::take_stats`]. Counts only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InboundFecStats {
    /// Repair packets received for the stream being reassembled.
    pub repairs_received: u32,
    /// Lost packets rebuilt from a repair packet and handed on.
    pub packets_recovered: u32,
    /// Repair packets that arrived with their whole group present and whose parity matched it:
    /// evidence that packets are kept in the form the sender protected.
    pub parity_matched: u32,
    /// The same, with parity that did not match. Recovery cannot work while these dominate.
    pub parity_mismatched: u32,
    /// Repair packets with two or more of their group missing, which one parity cannot rebuild.
    pub repairs_insufficient: u32,
    /// Repair packets whose one missing packet was no longer wanted: already skipped.
    pub repairs_late: u32,
    /// Rebuilt packets that did not look like the video packet they replace, and were dropped.
    pub recovered_rejected: u32,
    /// Gaps given up on: the hold ran out with the packet still missing.
    pub gaps_skipped: u32,
    /// Packets in those gaps.
    pub packets_skipped: u32,
}

/// Holds inbound video packets behind a sequence gap and rebuilds lost ones from repair packets.
#[derive(Default)]
pub struct InboundFec {
    /// The next sequence number reassembly expects, once a packet has arrived.
    expected: Option<u16>,
    /// Packets after the gap, ordered by sequence number.
    held: Vec<(u16, Vec<u8>)>,
    /// When the current gap opened.
    gap_since: Option<u64>,
    /// Recent packets of the stream, newest last, for rebuilding.
    recent: VecDeque<(u16, Vec<u8>)>,
    stats: InboundFecStats,
}

/// Signed distance from `from` to `to`, across the sequence number wrap.
fn distance(from: u16, to: u16) -> i16 {
    to.wrapping_sub(from) as i16
}

impl InboundFec {
    /// Forget the stream: the next packet starts a new one.
    pub fn reset(&mut self) {
        let stats = self.stats;
        *self = Self::default();
        self.stats = stats;
    }

    pub fn take_stats(&mut self) -> InboundFecStats {
        std::mem::take(&mut self.stats)
    }

    /// One video packet arrived. Returns the packets reassembly may take now, in order.
    pub fn arrive(&mut self, now: u64, sequence: u16, packet: Vec<u8>) -> Vec<Vec<u8>> {
        self.remember(sequence, &packet);
        let mut ready = Vec::new();
        match self.expected {
            None => {
                self.expected = Some(sequence.wrapping_add(1));
                ready.push(packet);
            }
            Some(expected) => {
                let ahead = distance(expected, sequence);
                if ahead == 0 {
                    ready.push(packet);
                    self.expected = Some(sequence.wrapping_add(1));
                    self.drain(&mut ready);
                } else if ahead < 0 {
                    // Late or repeated: reassembly decides what an old packet is worth.
                    ready.push(packet);
                } else if ahead >= RESTART_JUMP {
                    self.release_all(&mut ready);
                    ready.push(packet);
                    self.expected = Some(sequence.wrapping_add(1));
                } else {
                    self.hold(now, sequence, packet);
                }
            }
        }
        self.expire(now, &mut ready);
        ready
    }

    /// One repair packet arrived, already decrypted, for the stream `ssrc`. Returns the packets
    /// reassembly may take now, the rebuilt one among them.
    pub fn repair(&mut self, now: u64, ssrc: u32, payload: &[u8]) -> Vec<Vec<u8>> {
        let mut ready = Vec::new();
        if let Some((header, _)) = FecHeader::parse(payload)
            && header.protected_ssrc == ssrc
        {
            self.stats.repairs_received = self.stats.repairs_received.saturating_add(1);
            if let Some((sequence, packet)) = self.rebuild(&header, payload) {
                ready = self.arrive(now, sequence, packet);
            }
        }
        self.expire(now, &mut ready);
        ready
    }

    fn rebuild(&mut self, header: &FecHeader, payload: &[u8]) -> Option<(u16, Vec<u8>)> {
        let mut group = Vec::new();
        let mut missing = Vec::new();
        for sequence in header.protected() {
            match self.recent.iter().find(|(kept, _)| *kept == sequence) {
                Some((_, packet)) => group.push(packet.as_slice()),
                None => missing.push(sequence),
            }
        }
        match missing.as_slice() {
            [] => {
                // Nothing to rebuild: check the parity instead, by rebuilding one we have.
                let matched = group.split_first().is_some_and(|(first, rest)| {
                    recover_packet(payload, rest).is_some_and(|rebuilt| rebuilt == *first)
                });
                let counter = if matched {
                    &mut self.stats.parity_matched
                } else {
                    &mut self.stats.parity_mismatched
                };
                *counter = counter.saturating_add(1);
                None
            }
            [lost] => {
                let wanted = self
                    .expected
                    .is_some_and(|expected| distance(expected, *lost) >= 0);
                if !wanted {
                    self.stats.repairs_late = self.stats.repairs_late.saturating_add(1);
                    return None;
                }
                let rebuilt = recover_packet(payload, &group)
                    .filter(|packet| plausible(packet, *lost, header.protected_ssrc));
                if rebuilt.is_none() {
                    self.stats.recovered_rejected = self.stats.recovered_rejected.saturating_add(1);
                    return None;
                }
                self.stats.packets_recovered = self.stats.packets_recovered.saturating_add(1);
                rebuilt.map(|packet| (*lost, packet))
            }
            _ => {
                self.stats.repairs_insufficient = self.stats.repairs_insufficient.saturating_add(1);
                None
            }
        }
    }

    fn remember(&mut self, sequence: u16, packet: &[u8]) {
        if self.recent.iter().any(|(kept, _)| *kept == sequence) {
            return;
        }
        if self.recent.len() == RECENT_MAX {
            self.recent.pop_front();
        }
        self.recent.push_back((sequence, packet.to_vec()));
    }

    fn hold(&mut self, now: u64, sequence: u16, packet: Vec<u8>) {
        let Some(expected) = self.expected else {
            return;
        };
        let ahead = distance(expected, sequence);
        let at = self
            .held
            .partition_point(|(kept, _)| distance(expected, *kept) < ahead);
        if self.held.get(at).is_some_and(|(kept, _)| *kept == sequence) {
            return;
        }
        self.held.insert(at, (sequence, packet));
        self.gap_since.get_or_insert(now);
    }

    /// Hand on the held packets that now follow without a gap.
    fn drain(&mut self, ready: &mut Vec<Vec<u8>>) {
        while let Some(expected) = self.expected
            && self.held.first().is_some_and(|(kept, _)| *kept == expected)
        {
            let (_, packet) = self.held.remove(0);
            ready.push(packet);
            self.expected = Some(expected.wrapping_add(1));
        }
        if self.held.is_empty() {
            self.gap_since = None;
        }
    }

    /// Give up on gaps that have waited too long, or that hold too much behind them.
    fn expire(&mut self, now: u64, ready: &mut Vec<Vec<u8>>) {
        while let (Some(expected), Some(since), Some((first, _))) =
            (self.expected, self.gap_since, self.held.first())
        {
            if now.saturating_sub(since) < HOLD_MS && self.held.len() <= HOLD_MAX {
                break;
            }
            let skipped = distance(expected, *first).max(0) as u32;
            self.stats.gaps_skipped = self.stats.gaps_skipped.saturating_add(1);
            self.stats.packets_skipped = self.stats.packets_skipped.saturating_add(skipped);
            self.expected = Some(*first);
            self.drain(ready);
            if !self.held.is_empty() {
                // The next gap waits from now: its packets were only held because of this one.
                self.gap_since = Some(now);
            }
        }
    }

    fn release_all(&mut self, ready: &mut Vec<Vec<u8>>) {
        ready.extend(self.held.drain(..).map(|(_, packet)| packet));
        self.gap_since = None;
    }
}

/// A rebuilt packet must be the video packet it replaces: the right stream and sequence number,
/// the H.264 payload type, and a header that leaves a payload.
fn plausible(packet: &[u8], sequence: u16, ssrc: u32) -> bool {
    packet.len() > 12
        && packet[0] >> 6 == 2
        && packet[1] & 0x7f == RTP_PAYLOAD_TYPE_H264
        && u16::from_be_bytes([packet[2], packet[3]]) == sequence
        && u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]) == ssrc
        && rtp_header_byte_length(packet).is_some_and(|header| header < packet.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::fec::encode_fec_payload;

    const SSRC: u32 = 0x1234_5678;

    fn packet(sequence: u16, body: &[u8]) -> Vec<u8> {
        let mut packet = vec![0x80, RTP_PAYLOAD_TYPE_H264];
        packet.extend_from_slice(&sequence.to_be_bytes());
        packet.extend_from_slice(&9000u32.to_be_bytes());
        packet.extend_from_slice(&SSRC.to_be_bytes());
        packet.extend_from_slice(body);
        packet
    }

    fn sequences(packets: &[Vec<u8>]) -> Vec<u16> {
        packets
            .iter()
            .map(|p| u16::from_be_bytes([p[2], p[3]]))
            .collect()
    }

    #[test]
    fn in_order_packets_pass_straight_through() {
        let mut fec = InboundFec::default();
        for sequence in 10..15 {
            let ready = fec.arrive(0, sequence, packet(sequence, &[1, 2, 3]));
            assert_eq!(sequences(&ready), vec![sequence]);
        }
    }

    #[test]
    fn a_gap_holds_until_the_repair_rebuilds_it() {
        let packets: Vec<_> = (100..104)
            .map(|s| packet(s, &[s as u8, 7, 7, s as u8]))
            .collect();
        let refs: Vec<&[u8]> = packets.iter().map(Vec::as_slice).collect();
        let repair = encode_fec_payload(&refs).unwrap();
        let mut fec = InboundFec::default();
        assert_eq!(
            sequences(&fec.arrive(0, 100, packets[0].clone())),
            vec![100]
        );
        // 101 is lost: 102 and 103 wait for it.
        assert!(fec.arrive(5, 102, packets[2].clone()).is_empty());
        assert!(fec.arrive(10, 103, packets[3].clone()).is_empty());
        let ready = fec.repair(20, SSRC, &repair);
        assert_eq!(sequences(&ready), vec![101, 102, 103]);
        assert_eq!(ready[0], packets[1]);
        assert_eq!(fec.take_stats().packets_recovered, 1);
    }

    #[test]
    fn an_unfilled_gap_is_skipped_after_the_hold() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 1, packet(1, &[1]));
        assert!(fec.arrive(10, 3, packet(3, &[3])).is_empty());
        assert!(fec.arrive(50, 4, packet(4, &[4])).is_empty());
        let ready = fec.arrive(10 + HOLD_MS, 5, packet(5, &[5]));
        assert_eq!(sequences(&ready), vec![3, 4, 5]);
        let stats = fec.take_stats();
        assert_eq!((stats.gaps_skipped, stats.packets_skipped), (1, 1));
    }

    #[test]
    fn a_complete_group_checks_parity() {
        let packets: Vec<_> = (7..10).map(|s| packet(s, &[s as u8; 5])).collect();
        let refs: Vec<&[u8]> = packets.iter().map(Vec::as_slice).collect();
        let repair = encode_fec_payload(&refs).unwrap();
        let mut fec = InboundFec::default();
        for (sequence, p) in (7..10).zip(&packets) {
            fec.arrive(0, sequence, p.clone());
        }
        assert!(fec.repair(1, SSRC, &repair).is_empty());
        assert_eq!(fec.take_stats().parity_matched, 1);
    }

    #[test]
    fn a_late_packet_fills_the_gap_without_a_repair() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 65534, packet(65534, &[1]));
        assert!(fec.arrive(1, 0, packet(0, &[3])).is_empty());
        assert_eq!(
            sequences(&fec.arrive(2, 65535, packet(65535, &[2]))),
            vec![65535, 0]
        );
    }
}
