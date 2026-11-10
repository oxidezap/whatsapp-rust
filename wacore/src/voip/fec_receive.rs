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

/// Maximum wait behind a gap, counted from each packet's arrival. This is a local latency
/// policy, not a verified WhatsApp Web timeout. It permits reordered originals to fill a gap even
/// before any FEC repair has been seen. Repair spans can also cross picture boundaries.
pub(crate) const HOLD_MS: u64 = 100;
/// Packets held behind gaps at most, so a burst cannot grow the hold without bound.
const HOLD_MAX: usize = 128;
/// Packets kept for rebuilding: well past the widest span a repair packet's mask names.
const RECENT_MAX: usize = 256;
/// Repair packets kept while two or more of their packets are still missing, waiting for a late
/// one to arrive so a single parity can rebuild the other.
const PENDING_MAX: usize = 32;
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
    /// Repair packets given up on with two or more of their group still missing, which one parity
    /// cannot rebuild.
    pub repairs_insufficient: u32,
    /// Repair packets whose one missing packet was no longer wanted: already skipped.
    pub repairs_late: u32,
    /// Rebuilt packets that did not look like the video packet they replace, and were dropped.
    pub recovered_rejected: u32,
    /// Gaps given up on: the hold ran out with the packet still missing, or the stream jumped
    /// past it.
    pub gaps_skipped: u32,
    /// Packets in those gaps.
    pub packets_skipped: u32,
    /// Packets that arrived behind what was already handed on -- a repeat, an original already
    /// rebuilt, or one whose gap was skipped -- and were dropped. Reassembly has moved past them,
    /// and an old fragment fed to it cuts off the NAL it is rebuilding.
    pub packets_late: u32,
}

/// A packet handed on to reassembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Released {
    /// Plaintext: the header as it arrived, then the whole decrypted payload.
    pub packet: Vec<u8>,
    /// Rebuilt from a repair packet rather than received. Its piggybacked audio was never
    /// delivered, unlike a received packet's, which went on arrival.
    pub rebuilt: bool,
}

struct Held {
    sequence: u16,
    packet: Vec<u8>,
    rebuilt: bool,
    /// When it started waiting.
    since: u64,
}

/// Holds inbound video packets behind a sequence gap and rebuilds lost ones from repair packets.
#[derive(Default)]
pub struct InboundFec {
    /// The next sequence number reassembly expects, once a packet has arrived.
    expected: Option<u16>,
    /// Packets after a gap, ordered by sequence number.
    held: Vec<Held>,
    /// Recent packets of the stream, newest last, for rebuilding.
    recent: VecDeque<(u16, Vec<u8>)>,
    /// Repair packets still short of a packet, with when each arrived.
    pending: VecDeque<(u64, Vec<u8>)>,
    stats: InboundFecStats,
}

/// Signed distance from `from` to `to`, across the sequence number wrap.
fn distance(from: u16, to: u16) -> i16 {
    to.wrapping_sub(from) as i16
}

/// What one attempt to rebuild from a repair packet came to.
enum Attempt {
    /// Nothing more it can do: used, checked, or not wanted.
    Done,
    /// Still two or more short; worth another try when a packet arrives.
    Short,
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

    /// When the oldest held packet's hold runs out, if one is held: [`Self::poll`] is due then.
    #[must_use]
    pub fn deadline(&self) -> Option<u64> {
        self.held
            .iter()
            .map(|held| held.since)
            .min()
            .map(|since| since.saturating_add(HOLD_MS))
    }

    /// Give up on gaps whose hold ran out though nothing arrived since. Returns the packets
    /// reassembly may take now, in order.
    pub fn poll(&mut self, now: u64) -> Vec<Released> {
        let mut ready = Vec::new();
        self.expire(now, &mut ready);
        ready
    }

    /// One video packet arrived. Returns the packets reassembly may take now, in order.
    pub fn arrive(&mut self, now: u64, sequence: u16, packet: Vec<u8>) -> Vec<Released> {
        let mut ready = Vec::new();
        self.place(now, sequence, packet, false, &mut ready);
        self.retry_pending(now, &mut ready);
        self.expire(now, &mut ready);
        ready
    }

    /// One repair packet arrived, already decrypted, for the stream `ssrc`. Returns the packets
    /// reassembly may take now, the rebuilt one among them.
    pub fn repair(&mut self, now: u64, ssrc: u32, payload: &[u8]) -> Vec<Released> {
        let mut ready = Vec::new();
        if let Some((header, _)) = FecHeader::parse(payload)
            && header.protected_ssrc == ssrc
        {
            self.stats.repairs_received = self.stats.repairs_received.saturating_add(1);
            if let Attempt::Short = self.attempt(now, payload, true, &mut ready) {
                if self.pending.len() == PENDING_MAX {
                    self.pending.pop_front();
                    self.stats.repairs_insufficient =
                        self.stats.repairs_insufficient.saturating_add(1);
                }
                self.pending.push_back((now, payload.to_vec()));
            }
            self.retry_pending(now, &mut ready);
        }
        self.expire(now, &mut ready);
        ready
    }

    /// A received or rebuilt packet: handed on if it is next, held behind a gap, or dropped if
    /// reassembly is already past it.
    fn place(
        &mut self,
        now: u64,
        sequence: u16,
        packet: Vec<u8>,
        rebuilt: bool,
        ready: &mut Vec<Released>,
    ) {
        self.remember(sequence, &packet);
        let Some(expected) = self.expected else {
            self.expected = Some(sequence.wrapping_add(1));
            ready.push(Released { packet, rebuilt });
            return;
        };
        let ahead = distance(expected, sequence);
        if ahead == 0 {
            ready.push(Released { packet, rebuilt });
            self.expected = Some(sequence.wrapping_add(1));
            self.drain(ready);
        } else if ahead < 0 {
            // Kept above for parity checks; reassembly has moved past it.
            self.stats.packets_late = self.stats.packets_late.saturating_add(1);
        } else if ahead >= RESTART_JUMP {
            // The stream restarted rather than paused: nothing is worth waiting for, but the
            // packets it skipped are as lost as an expired gap's, and are counted as one.
            self.stats.gaps_skipped = self.stats.gaps_skipped.saturating_add(1);
            self.stats.packets_skipped = self
                .stats
                .packets_skipped
                .saturating_add((ahead as u32).saturating_sub(self.held.len() as u32));
            ready.extend(self.held.drain(..).map(|held| Released {
                packet: held.packet,
                rebuilt: held.rebuilt,
            }));
            ready.push(Released { packet, rebuilt });
            self.expected = Some(sequence.wrapping_add(1));
        } else {
            let at = self
                .held
                .partition_point(|held| distance(expected, held.sequence) < ahead);
            if self
                .held
                .get(at)
                .is_some_and(|held| held.sequence == sequence)
            {
                self.stats.packets_late = self.stats.packets_late.saturating_add(1);
                return;
            }
            self.held.insert(
                at,
                Held {
                    sequence,
                    packet,
                    rebuilt,
                    since: now,
                },
            );
        }
    }

    /// Rebuild from a repair packet if exactly one of its group is missing. `first` is its
    /// arrival; later attempts are retries of one already counted.
    fn attempt(
        &mut self,
        now: u64,
        payload: &[u8],
        first: bool,
        ready: &mut Vec<Released>,
    ) -> Attempt {
        let Some((header, _)) = FecHeader::parse(payload) else {
            return Attempt::Done;
        };
        let mut group = Vec::new();
        let mut missing = Vec::new();
        for sequence in header.protected() {
            match self.recent.iter().find(|(kept, _)| *kept == sequence) {
                Some((_, packet)) => group.push(packet.as_slice()),
                None => missing.push(sequence),
            }
        }
        let expected = self.expected;
        let wanted =
            |sequence: u16| expected.is_some_and(|expected| distance(expected, sequence) >= 0);
        match missing.as_slice() {
            [] => {
                if first {
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
                }
                Attempt::Done
            }
            [lost] => {
                let lost = *lost;
                if !wanted(lost) {
                    self.stats.repairs_late = self.stats.repairs_late.saturating_add(1);
                    return Attempt::Done;
                }
                let rebuilt = recover_packet(payload, &group)
                    .filter(|packet| plausible(packet, lost, header.protected_ssrc));
                match rebuilt {
                    Some(packet) => {
                        self.stats.packets_recovered =
                            self.stats.packets_recovered.saturating_add(1);
                        self.place(now, lost, packet, true, ready);
                    }
                    None => {
                        self.stats.recovered_rejected =
                            self.stats.recovered_rejected.saturating_add(1);
                    }
                }
                Attempt::Done
            }
            several if several.iter().any(|sequence| wanted(*sequence)) => Attempt::Short,
            _ => {
                self.stats.repairs_late = self.stats.repairs_late.saturating_add(1);
                Attempt::Done
            }
        }
    }

    /// Try the waiting repair packets again, until none rebuilds anything more.
    fn retry_pending(&mut self, now: u64, ready: &mut Vec<Released>) {
        loop {
            let recovered = self.stats.packets_recovered;
            let mut index = 0;
            while index < self.pending.len() {
                let payload = std::mem::take(&mut self.pending[index].1);
                if let Attempt::Short = self.attempt(now, &payload, false, ready) {
                    self.pending[index].1 = payload;
                    index += 1;
                } else {
                    self.pending.remove(index);
                }
            }
            if self.stats.packets_recovered == recovered {
                break;
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

    /// Hand on the held packets that now follow without a gap.
    fn drain(&mut self, ready: &mut Vec<Released>) {
        while let Some(expected) = self.expected
            && self
                .held
                .first()
                .is_some_and(|held| held.sequence == expected)
        {
            let held = self.held.remove(0);
            ready.push(Released {
                packet: held.packet,
                rebuilt: held.rebuilt,
            });
            self.expected = Some(expected.wrapping_add(1));
        }
    }

    /// Give up on gaps once the oldest packet behind them has waited its hold, or too much is
    /// held; and on repair packets that waited as long. Each packet waits from its own arrival,
    /// so several gaps in a row do not add their holds up.
    fn expire(&mut self, now: u64, ready: &mut Vec<Released>) {
        while let Some(deadline) = self.deadline()
            && let Some(expected) = self.expected
            && let Some(first) = self.held.first().map(|held| held.sequence)
        {
            if now < deadline && self.held.len() <= HOLD_MAX {
                break;
            }
            let skipped = distance(expected, first).max(0) as u32;
            self.stats.gaps_skipped = self.stats.gaps_skipped.saturating_add(1);
            self.stats.packets_skipped = self.stats.packets_skipped.saturating_add(skipped);
            self.expected = Some(first);
            self.drain(ready);
        }
        while self
            .pending
            .front()
            .is_some_and(|(since, _)| now.saturating_sub(*since) >= HOLD_MS)
        {
            self.pending.pop_front();
            self.stats.repairs_insufficient = self.stats.repairs_insufficient.saturating_add(1);
        }
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

    fn sequences(released: &[Released]) -> Vec<u16> {
        released
            .iter()
            .map(|r| u16::from_be_bytes([r.packet[2], r.packet[3]]))
            .collect()
    }

    fn repair_of(packets: &[Vec<u8>]) -> Vec<u8> {
        let refs: Vec<&[u8]> = packets.iter().map(Vec::as_slice).collect();
        encode_fec_payload(&refs).unwrap()
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
        assert_eq!(ready[0].packet, packets[1]);
        assert!(ready[0].rebuilt && !ready[1].rebuilt);
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

    // A repair can rebuild a packet that was only delayed. Its original must not reach reassembly
    // afterwards: an old fragment would cut off the NAL being rebuilt.
    #[test]
    fn an_original_arriving_after_its_rebuild_is_dropped() {
        let packets: Vec<_> = (100..103).map(|s| packet(s, &[s as u8; 4])).collect();
        let mut fec = InboundFec::default();
        fec.arrive(0, 100, packets[0].clone());
        fec.arrive(1, 102, packets[2].clone());
        assert_eq!(
            sequences(&fec.repair(2, SSRC, &repair_of(&packets))),
            vec![101, 102]
        );
        assert!(fec.arrive(3, 101, packets[1].clone()).is_empty());
        assert!(fec.arrive(4, 100, packets[0].clone()).is_empty());
        assert_eq!(fec.take_stats().packets_late, 2);
    }

    #[test]
    fn a_restarted_stream_counts_what_it_skipped() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 1, packet(1, &[1]));
        assert!(fec.arrive(1, 3, packet(3, &[3])).is_empty());
        let jump = 3 + RESTART_JUMP as u16;
        assert_eq!(
            sequences(&fec.arrive(2, jump, packet(jump, &[9]))),
            vec![3, jump]
        );
        let stats = fec.take_stats();
        assert_eq!(stats.gaps_skipped, 1);
        assert_eq!(stats.packets_skipped, u32::from(jump) - 2 - 1);
    }

    // Nothing more may arrive after a gap -- the peer's camera paused -- and the held picture must
    // still come out when its hold runs out.
    #[test]
    fn a_held_packet_is_released_by_the_timer() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 1, packet(1, &[1]));
        assert!(fec.arrive(10, 3, packet(3, &[3])).is_empty());
        assert_eq!(fec.deadline(), Some(10 + HOLD_MS));
        assert!(fec.poll(9 + HOLD_MS).is_empty());
        assert_eq!(sequences(&fec.poll(10 + HOLD_MS)), vec![3]);
        assert_eq!(fec.deadline(), None);
        assert_eq!(fec.take_stats().gaps_skipped, 1);
    }

    // Several gaps in a row: each packet waits from its own arrival, so packets that arrived
    // together come out together, not one hold per gap later.
    #[test]
    fn several_gaps_do_not_add_their_holds_up() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 1, packet(1, &[1]));
        for sequence in [3, 5, 7] {
            assert!(fec.arrive(10, sequence, packet(sequence, &[1])).is_empty());
        }
        assert_eq!(sequences(&fec.poll(10 + HOLD_MS)), vec![3, 5, 7]);
        let stats = fec.take_stats();
        assert_eq!((stats.gaps_skipped, stats.packets_skipped), (3, 3));
    }

    // 101 lost and 102 late when the repair comes: two missing, so it waits. 102 arriving makes
    // one parity enough, and 101 is rebuilt.
    #[test]
    fn a_repair_short_of_two_packets_waits_for_a_late_one() {
        let packets: Vec<_> = (100..104)
            .map(|s| packet(s, &[s as u8, 3, s as u8]))
            .collect();
        let mut fec = InboundFec::default();
        fec.arrive(0, 100, packets[0].clone());
        fec.arrive(1, 103, packets[3].clone());
        assert!(fec.repair(2, SSRC, &repair_of(&packets)).is_empty());
        let ready = fec.arrive(3, 102, packets[2].clone());
        assert_eq!(sequences(&ready), vec![101, 102, 103]);
        assert_eq!(ready[0].packet, packets[1]);
        let stats = fec.take_stats();
        assert_eq!(
            (stats.packets_recovered, stats.repairs_insufficient),
            (1, 0)
        );
    }

    #[test]
    fn a_repair_still_short_is_given_up_with_its_gap() {
        let packets: Vec<_> = (100..104).map(|s| packet(s, &[s as u8])).collect();
        let mut fec = InboundFec::default();
        fec.arrive(0, 100, packets[0].clone());
        fec.arrive(1, 103, packets[3].clone());
        assert!(fec.repair(2, SSRC, &repair_of(&packets)).is_empty());
        assert_eq!(sequences(&fec.poll(2 + HOLD_MS)), vec![103]);
        assert_eq!(fec.take_stats().repairs_insufficient, 1);
    }
    #[test]
    fn the_hold_bound_releases_before_the_deadline() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 1, packet(1, &[1]));
        for sequence in 3..3 + HOLD_MAX as u16 {
            assert!(fec.arrive(1, sequence, packet(sequence, &[1])).is_empty());
        }
        assert_eq!(fec.held.len(), HOLD_MAX);
        let last = 3 + HOLD_MAX as u16;
        let ready = fec.arrive(1, last, packet(last, &[1]));
        assert_eq!(sequences(&ready), (3..=last).collect::<Vec<_>>());
        assert!(fec.held.is_empty());
        let stats = fec.take_stats();
        assert_eq!((stats.gaps_skipped, stats.packets_skipped), (1, 1));
    }

    #[test]
    fn the_pending_bound_evicts_the_oldest_short_repair() {
        let mut fec = InboundFec::default();
        fec.arrive(0, 100, packet(100, &[1]));
        let repairs: Vec<_> = (0..=PENDING_MAX)
            .map(|n| repair_of(&[packet(101, &[n as u8]), packet(102, &[2])]))
            .collect();
        for repair in &repairs {
            assert!(fec.repair(1, SSRC, repair).is_empty());
        }
        assert_eq!(fec.pending.len(), PENDING_MAX);
        assert_eq!(fec.pending.front().unwrap().1, repairs[1]);
        assert_eq!(fec.take_stats().repairs_insufficient, 1);
    }
}
