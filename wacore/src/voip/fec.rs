//! Video forward error correction, as WhatsApp's clients send it.
//!
//! The format is FlexFEC from draft-ietf-payload-flexible-fec-scheme-03, the draft WebRTC's
//! `flexfec-03` implements, in WhatsApp's multistream layout: one SSRC count, then per protected
//! stream its SSRC, base sequence number and a flexible mask. A repair packet is the XOR parity of
//! a group of video packets, so a receiver missing any ONE packet of the group rebuilds it from the
//! others and the repair packet, without a retransmission or a keyframe.
//!
//! ```text
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |R|F|P|X|  CC   |M| PT recovery |        length recovery        |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                          TS recovery                          |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |   SSRCCount   |                    reserved                   |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                             SSRC_i                            |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |           SN base_i           |k|          Mask [0-14]        |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |k|                   Mask [15-45] (optional)                   |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |k|                   Mask [46-108] (optional)                  |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! ```
//!
//! What the parity covers, as the draft's section 6.2 says and an iPhone's repair packets confirm
//! byte for byte: the first eight bytes of each packet's RTP header (the sequence number skipped),
//! its length minus the 12-byte fixed header, and everything after that fixed header -- CSRCs and
//! the header extension included, which is what lets a rebuilt packet's X bit mean anything. Its
//! repair payload is therefore 20 bytes longer than the largest protected packet minus 12.
//!
//! Mask bit `i` names `SN base + i`, and a `k` bit of 1 ends the mask. That is WebRTC's flexfec-03
//! reader and writer, and an iPhone's own repair packets: one with base 9 and mask `0xff00` (k set,
//! then seven bits) carried the RTP timestamp of packet 15, the last of 9 to 15 it protects. The
//! draft's prose says `SN base + i + 1`, which would have made that group 10 to 16.

use std::collections::VecDeque;

use crate::voip_control::VideoFecConfig;

/// RTP payload type of WhatsApp's video FEC stream.
pub const RTP_PAYLOAD_TYPE_VIDEO_FEC: u8 = 103;

/// The most consecutive packets one repair packet protects while its mask stays two bytes long.
pub const FEC_MAX_GROUP: usize = 15;

/// The repair header for one protected stream and a two-byte mask.
pub const FEC_HEADER_LEN: usize = 20;

/// The RTP fixed header the parity does not cover (all but its first eight bytes are recovered
/// from elsewhere: the sequence number from the mask, the SSRC from the repair header).
const RTP_FIXED_LEN: usize = 12;
/// R/F through the reserved word, before the per-stream blocks.
const FEC_FIXED_LEN: usize = 12;
/// SSRC and SN base of one protected stream.
const FEC_STREAM_LEN: usize = 6;
/// Each mask block's width in mask bits, after its `k` bit.
const MASK_BLOCK_BITS: [usize; 3] = [15, 31, 63];
/// Mask lengths in bytes for one, two and three blocks.
const MASK_LENS: [usize; 3] = [2, 6, 14];
/// Group sizes a [`VideoFecConfig`] may ask for; see its documentation.
const GROUP_RANGE: std::ops::RangeInclusive<u8> = 2..=8;
/// Interleaving depths a [`VideoFecConfig`] may ask for; see its documentation.
const DEPTH_RANGE: std::ops::RangeInclusive<u8> = 1..=8;
/// Packets the longest (14-byte) mask names.
const FEC_MAX_MASK_BITS: usize = 109;

/// A parsed repair header for one protected stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FecHeader {
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    /// XOR of the protected packets' lengths minus the 12-byte RTP fixed header.
    pub length: u16,
    pub timestamp: u32,
    pub protected_ssrc: u32,
    pub sequence_base: u16,
    /// Bit `i` set: the packet with sequence number `sequence_base + i` is protected.
    pub mask: u128,
}

impl FecHeader {
    /// Mask blocks needed to hold the highest protected packet, or `None` for an empty mask or one
    /// past the 109 packets the longest mask names.
    fn mask_blocks(&self) -> Option<usize> {
        let highest = 127usize.checked_sub(self.mask.leading_zeros() as usize)?;
        let mut capacity = 0;
        for (blocks, bits) in MASK_BLOCK_BITS.iter().enumerate() {
            capacity += bits;
            if highest < capacity {
                return Some(blocks + 1);
            }
        }
        None
    }

    /// The encoded length, or `None` when the mask cannot be encoded.
    pub fn byte_len(&self) -> Option<usize> {
        Some(FEC_FIXED_LEN + FEC_STREAM_LEN + MASK_LENS[self.mask_blocks()? - 1])
    }

    /// Append the encoded header. `false`, and nothing written, when the mask cannot be encoded.
    pub fn write_into(&self, out: &mut Vec<u8>) -> bool {
        let Some(blocks) = self.mask_blocks() else {
            return false;
        };
        out.push(
            (u8::from(self.padding) << 5)
                | (u8::from(self.extension) << 4)
                | (self.csrc_count & 0x0f),
        );
        out.push((u8::from(self.marker) << 7) | (self.payload_type & 0x7f));
        out.extend_from_slice(&self.length.to_be_bytes());
        out.extend_from_slice(&self.timestamp.to_be_bytes());
        // One protected stream and the 24 reserved bits.
        out.extend_from_slice(&[1, 0, 0, 0]);
        out.extend_from_slice(&self.protected_ssrc.to_be_bytes());
        out.extend_from_slice(&self.sequence_base.to_be_bytes());
        // The mask is a bit string, k bit first in each block, read most significant bit first.
        let mut bits: u128 = 0;
        let mut index = 0;
        for (block, width) in MASK_BLOCK_BITS.iter().take(blocks).enumerate() {
            bits = (bits << 1) | u128::from(block + 1 == blocks);
            for _ in 0..*width {
                bits = (bits << 1) | ((self.mask >> index) & 1);
                index += 1;
            }
        }
        let len = MASK_LENS[blocks - 1];
        out.extend_from_slice(&bits.to_be_bytes()[16 - len..]);
        true
    }

    /// Parse a repair header from the start of a repair payload, with the header's length.
    ///
    /// `None` for a retransmission (R=1), a fixed-offset mask (F=1), any number of protected
    /// streams but one, or a mask whose last `k` bit is missing.
    pub fn parse(data: &[u8]) -> Option<(Self, usize)> {
        let fixed = data.get(..FEC_FIXED_LEN + FEC_STREAM_LEN)?;
        if fixed[0] & 0xc0 != 0 || fixed[8] != 1 {
            return None;
        }
        let mut blocks = 0;
        let mut bits: u128 = 0;
        let mut offset = FEC_FIXED_LEN + FEC_STREAM_LEN;
        loop {
            let start = if blocks == 0 {
                0
            } else {
                MASK_LENS[blocks - 1]
            };
            let end = *MASK_LENS.get(blocks)?;
            for byte in data.get(offset + start..offset + end)? {
                bits = (bits << 8) | u128::from(*byte);
            }
            // The k bit leads the block just read.
            let k_shift = (end - start) * 8 - 1;
            let last = (bits >> k_shift) & 1 == 1;
            blocks += 1;
            if last {
                break;
            }
        }
        let len = MASK_LENS[blocks - 1];
        // Walk the bit string back out, k bits skipped, mask bit 0 first.
        let total_bits = len * 8;
        let mut mask: u128 = 0;
        let mut position = total_bits;
        let mut index = 0;
        for width in MASK_BLOCK_BITS.iter().take(blocks) {
            position -= 1; // the k bit
            for _ in 0..*width {
                position -= 1;
                mask |= ((bits >> position) & 1) << index;
                index += 1;
            }
        }
        offset += len;
        Some((
            Self {
                padding: fixed[0] & 0x20 != 0,
                extension: fixed[0] & 0x10 != 0,
                csrc_count: fixed[0] & 0x0f,
                marker: fixed[1] & 0x80 != 0,
                payload_type: fixed[1] & 0x7f,
                length: u16::from_be_bytes([fixed[2], fixed[3]]),
                timestamp: u32::from_be_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]),
                protected_ssrc: u32::from_be_bytes([fixed[12], fixed[13], fixed[14], fixed[15]]),
                sequence_base: u16::from_be_bytes([fixed[16], fixed[17]]),
                mask,
            },
            offset,
        ))
    }

    /// The sequence numbers this header protects, lowest first.
    pub fn protected(&self) -> impl Iterator<Item = u16> + '_ {
        (0..128u16)
            .filter(|bit| (self.mask >> bit) & 1 == 1)
            .map(|bit| self.sequence_base.wrapping_add(bit))
    }
}

/// Sequence number and SSRC of a plaintext RTP packet, or `None` for one too short to be RTP.
fn rtp_identity(packet: &[u8]) -> Option<(u16, u32)> {
    (packet.len() >= RTP_FIXED_LEN).then(|| {
        (
            u16::from_be_bytes([packet[2], packet[3]]),
            u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
        )
    })
}

/// The running XOR parity of one group of packets of one stream, lowest sequence number first.
#[derive(Default)]
struct Parity {
    head: [u8; 8],
    length: u16,
    body: Vec<u8>,
    count: usize,
    sequence_base: u16,
    ssrc: u32,
    /// Bit `i`: `sequence_base + i` is folded in.
    mask: u128,
}

impl Parity {
    /// Fold in one plaintext RTP packet, header and extension included. `false` for a packet too
    /// short to be RTP, too long for the 16-bit length, or past what the longest mask names from
    /// the group's first packet; it is not folded in.
    fn add(&mut self, packet: &[u8]) -> bool {
        let Some((sequence, ssrc)) = rtp_identity(packet) else {
            return false;
        };
        let body = &packet[RTP_FIXED_LEN..];
        let Ok(length) = u16::try_from(body.len()) else {
            return false;
        };
        if self.count == 0 {
            self.sequence_base = sequence;
            self.ssrc = ssrc;
        }
        let bit = usize::from(sequence.wrapping_sub(self.sequence_base));
        if bit >= FEC_MAX_MASK_BITS {
            return false;
        }
        for (acc, byte) in self.head.iter_mut().zip(packet) {
            *acc ^= byte;
        }
        self.length ^= length;
        if self.body.len() < body.len() {
            self.body.resize(body.len(), 0);
        }
        for (acc, byte) in self.body.iter_mut().zip(body) {
            *acc ^= byte;
        }
        self.mask |= 1 << bit;
        self.count += 1;
        true
    }

    fn reset(&mut self) {
        self.head = [0; 8];
        self.length = 0;
        self.body.clear();
        self.count = 0;
        self.mask = 0;
    }

    /// The repair payload for the packets folded in so far, and a fresh group after it.
    fn take(&mut self) -> Option<Vec<u8>> {
        if self.count == 0 {
            return None;
        }
        let header = FecHeader {
            padding: self.head[0] & 0x20 != 0,
            extension: self.head[0] & 0x10 != 0,
            csrc_count: self.head[0] & 0x0f,
            marker: self.head[1] & 0x80 != 0,
            payload_type: self.head[1] & 0x7f,
            length: self.length,
            timestamp: u32::from_be_bytes([self.head[4], self.head[5], self.head[6], self.head[7]]),
            protected_ssrc: self.ssrc,
            sequence_base: self.sequence_base,
            mask: self.mask,
        };
        let mut out = Vec::with_capacity(header.byte_len()? + self.body.len());
        let written = header.write_into(&mut out);
        debug_assert!(written, "`add` keeps every mask within the longest one");
        out.extend_from_slice(&self.body);
        self.reset();
        Some(out)
    }
}

/// The repair payload (header and parity) protecting `packets`: plaintext RTP packets of one
/// stream with consecutive sequence numbers, lowest first. `None` for an empty group, a gap, or
/// a packet that is not RTP.
pub fn encode_fec_payload(packets: &[&[u8]]) -> Option<Vec<u8>> {
    let mut parity = Parity::default();
    for packet in packets {
        let (sequence, ssrc) = rtp_identity(packet)?;
        if parity.count != 0
            && (ssrc != parity.ssrc
                || sequence != parity.sequence_base.wrapping_add(parity.count as u16))
        {
            return None;
        }
        if !parity.add(packet) {
            return None;
        }
    }
    parity.take()
}

/// Rebuild the one packet of a group that did not arrive, from the repair payload and the other
/// packets of its group (plaintext RTP, any order). `None` unless exactly one protected packet is
/// missing from `received`.
pub fn recover_packet(fec_payload: &[u8], received: &[&[u8]]) -> Option<Vec<u8>> {
    let (header, header_len) = FecHeader::parse(fec_payload)?;
    let parity = &fec_payload[header_len..];
    let sequence_of = |packet: &[u8]| -> Option<u16> {
        (packet.len() >= RTP_FIXED_LEN
            && u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]])
                == header.protected_ssrc)
            .then(|| u16::from_be_bytes([packet[2], packet[3]]))
    };
    let mut missing = header
        .protected()
        .filter(|sequence| !received.iter().any(|p| sequence_of(p) == Some(*sequence)));
    let lost = missing.next()?;
    if missing.next().is_some() {
        return None;
    }
    let mut head = [0u8; 8];
    head[0] = fec_payload[0] & 0x3f;
    head[1] = fec_payload[1];
    head[4..8].copy_from_slice(&fec_payload[4..8]);
    let mut length = header.length;
    let mut body = parity.to_vec();
    for packet in received {
        let Some(sequence) = sequence_of(packet) else {
            continue;
        };
        if !header.protected().any(|protected| protected == sequence) {
            continue;
        }
        for (acc, byte) in head.iter_mut().zip(packet.iter()) {
            *acc ^= byte;
        }
        let tail = &packet[RTP_FIXED_LEN..];
        length ^= u16::try_from(tail.len()).ok()?;
        if body.len() < tail.len() {
            return None;
        }
        for (acc, byte) in body.iter_mut().zip(tail) {
            *acc ^= byte;
        }
    }
    let length = usize::from(length);
    if length > body.len() {
        return None;
    }
    let mut packet = Vec::with_capacity(RTP_FIXED_LEN + length);
    packet.push(0x80 | (head[0] & 0x3f));
    packet.push(head[1]);
    packet.extend_from_slice(&lost.to_be_bytes());
    packet.extend_from_slice(&head[4..8]);
    packet.extend_from_slice(&header.protected_ssrc.to_be_bytes());
    packet.extend_from_slice(&body[..length]);
    Some(packet)
}

/// The longest span `depth` interleaved groups can cover while every group's mask stays two bytes
/// long: group 0 starts the span, so its last member, `depth * (members - 1)` past its first, is
/// the farthest and must be within the 15 bits of one short mask.
fn max_span(depth: usize) -> usize {
    depth * ((FEC_MAX_GROUP - 1) / depth + 1)
}

/// Groups one stream's outbound video packets and produces repair payloads for them.
///
/// Packets are taken in spans of [`Self::span_size`] consecutive packets, dealt round-robin to
/// [`Self::depth`] groups: packet `k` of a span goes to group `k % depth`, so the members of one
/// group are `depth` apart and a burst of up to `depth` lost packets takes at most one from each.
/// The span's repair payloads, one per group, lowest group first, follow its last packet.
///
/// A span always closes on a packet of the access unit being sent, its last or one partway
/// through it, and its repair packets go out right behind that packet, so a receiver waiting on a
/// picture's packets does not also wait on the next picture. A small access unit's packets carry
/// over into the next until there are enough for a span, so a run of one- or two-packet frames is
/// not protected at a ratio of one to one. A large one is cut into several spans of about the same
/// size, or, where those would take a group's mask past two bytes, into whole spans with the rest
/// carried over.
pub(crate) struct VideoFecSender {
    config: VideoFecConfig,
    fraction_lost: u8,
    /// One parity per interleaved group of the current span; its length is the span's depth.
    groups: Vec<Parity>,
    /// Packets of the current span so far.
    span_count: usize,
    span_base: u16,
    span_ssrc: u32,
    /// The size of each span the current access unit closes, counting packets carried over.
    plan: VecDeque<usize>,
}

impl VideoFecSender {
    pub(crate) fn new(config: VideoFecConfig) -> Self {
        Self {
            config,
            fraction_lost: 0,
            groups: Vec::new(),
            span_count: 0,
            span_base: 0,
            span_ssrc: 0,
            plan: VecDeque::new(),
        }
    }

    pub(crate) fn enabled(&self) -> bool {
        self.config.enabled
    }

    pub(crate) fn set_config(&mut self, config: VideoFecConfig) {
        self.config = config;
        if !config.enabled {
            self.reset();
        }
    }

    /// The fraction (in 256ths) of our video the peer's latest RTCP report says it lost.
    pub(crate) fn observe_loss(&mut self, fraction_lost: u8) {
        self.fraction_lost = fraction_lost;
    }

    fn lossy(&self) -> bool {
        self.fraction_lost > self.config.loss_threshold
    }

    /// Groups interleaved in one span at the moment.
    pub(crate) fn depth(&self) -> usize {
        let wanted = if self.lossy() {
            self.config.lossy_interleave
        } else {
            self.config.clean_interleave
        };
        usize::from(wanted.clamp(*DEPTH_RANGE.start(), *DEPTH_RANGE.end()))
    }

    /// Consecutive video packets one span covers at the moment: the depth times the group size,
    /// within what keeps every group's mask two bytes long.
    pub(crate) fn span_size(&self) -> usize {
        let wanted = if self.lossy() {
            self.config.lossy_group
        } else {
            self.config.clean_group
        };
        let group = usize::from(wanted.clamp(*GROUP_RANGE.start(), *GROUP_RANGE.end()));
        let depth = self.depth();
        (depth * group).min(max_span(depth))
    }

    /// Video packets per repair packet at the moment.
    pub(crate) fn group_size(&self) -> usize {
        self.span_size() / self.depth()
    }

    /// Plan the spans for an access unit of `packets` packets, before its first is pushed.
    pub(crate) fn begin_access_unit(&mut self, packets: usize) {
        self.plan.clear();
        if !self.config.enabled {
            return;
        }
        let span = self.span_size();
        let depth = self.depth();
        // A new threshold can shrink the target below the carried span, or change its depth.
        if self.span_count >= span || (self.span_count != 0 && self.groups.len() != depth) {
            self.reset_span();
        }
        let carried = self.span_count;
        let total = carried + packets;
        if total < span {
            return;
        }
        let max = max_span(depth);
        let spans = total / span;
        if total.div_ceil(spans) <= max {
            // Even spans of `span` to `2 * span - 1`, the last ending with the access unit. Each
            // is longer than the carried packets, so they fit in the first.
            let (each, extra) = (total / spans, total % spans);
            self.plan
                .extend((0..spans).map(|index| each + usize::from(index < extra)));
        } else {
            // Even spans would take a mask past two bytes. Whole spans instead, the last ending
            // inside the access unit, and the rest carried over like a small access unit's
            // packets: the ratio stays one repair packet per group size.
            self.plan.extend(std::iter::repeat_n(span, spans));
        }
        debug_assert!(
            self.plan.front().is_some_and(|first| *first > carried)
                && self.plan.iter().all(|span| *span <= max)
        );
    }

    fn reset_span(&mut self) {
        for group in &mut self.groups {
            group.reset();
        }
        self.span_count = 0;
    }

    pub(crate) fn reset(&mut self) {
        self.reset_span();
        self.plan.clear();
    }

    /// Whether `packet` continues the current span: same stream, next sequence number.
    fn continues(&self, packet: &[u8]) -> bool {
        self.span_count == 0
            || rtp_identity(packet).is_some_and(|(sequence, ssrc)| {
                ssrc == self.span_ssrc
                    && sequence == self.span_base.wrapping_add(self.span_count as u16)
            })
    }

    /// Add one plaintext RTP packet of the planned access unit. When it closes a span, the span's
    /// repair payloads, which go on the wire right after it in this order; otherwise none.
    pub(crate) fn push(&mut self, packet: &[u8]) -> Vec<Vec<u8>> {
        if !self.config.enabled {
            return Vec::new();
        }
        if !self.continues(packet) {
            // A renumbered stream, or packets that never reached us: the span so far cannot be
            // completed, and the plan counted it.
            self.reset();
        }
        if self.span_count == 0 {
            let Some((sequence, ssrc)) = rtp_identity(packet) else {
                self.reset();
                return Vec::new();
            };
            self.span_base = sequence;
            self.span_ssrc = ssrc;
            let depth = self.depth();
            self.groups.resize_with(depth, Parity::default);
            self.groups.truncate(depth);
        }
        let depth = self.groups.len();
        if !self.groups[self.span_count % depth].add(packet) {
            self.reset();
            return Vec::new();
        }
        self.span_count += 1;
        if self.plan.is_empty() && self.span_count >= self.span_size() {
            self.reset_span();
            return Vec::new();
        }
        if self.plan.front() == Some(&self.span_count) {
            self.plan.pop_front();
            self.span_count = 0;
            return self.groups.iter_mut().filter_map(Parity::take).collect();
        }
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plaintext RTP packet with the video header shape: V=2, X=1, a three-word extension
    /// (`hdr` 28 bytes), PT 97, and `payload` bytes of payload.
    fn video_packet(sequence: u16, timestamp: u32, marker: bool, payload: usize) -> Vec<u8> {
        let mut packet = vec![0x90, (u8::from(marker) << 7) | 97];
        packet.extend_from_slice(&sequence.to_be_bytes());
        packet.extend_from_slice(&timestamp.to_be_bytes());
        packet.extend_from_slice(&0x2da0_4dd6u32.to_be_bytes());
        packet.extend_from_slice(&[0xde, 0xbe, 0, 3]);
        packet.extend_from_slice(&[
            0x30, 0x03, 0x51, 0x2e, 0x2a, 0x61, 0, 0x2b, 0x91, 0x0f, 0xf5, 0,
        ]);
        packet
            .extend((0..payload).map(|i| (i as u8).wrapping_mul(31).wrapping_add(sequence as u8)));
        packet
    }

    // An iPhone's first repair packet of a call, as its header was logged: it covered video
    // sequence numbers 1 to 7 (payloads of 861, 860, 543, 543, 979, 515 and 514 bytes after a
    // 28-byte header, markers on 2, 4, 5 and 7) and began `10 61 03 e3`, and its payload was 1015
    // bytes. X = 1 and PT 97 are odd counts of ones, M = 0 an even one, and 0x03e3 is the XOR of
    // the lengths minus 12, which includes the 16 extension bytes; 1015 is 20 more than 995, the
    // largest of those. A second packet covering 8 to 15 began `00 00 00 d4` and is checked too.
    #[test]
    fn repair_header_matches_an_iphones_packets() {
        let first: Vec<Vec<u8>> = [
            (1, 469_350, false, 861),
            (2, 469_350, true, 860),
            (3, 474_570, false, 543),
            (4, 474_570, true, 543),
            (5, 480_510, true, 979),
            (6, 486_900, false, 515),
            (7, 486_900, true, 514),
        ]
        .into_iter()
        .map(|(sequence, timestamp, marker, payload)| {
            video_packet(sequence, timestamp, marker, payload)
        })
        .collect();
        let refs: Vec<&[u8]> = first.iter().map(Vec::as_slice).collect();
        let payload = encode_fec_payload(&refs).expect("a group");
        assert_eq!(hex::encode(&payload[..4]), "106103e3");
        assert_eq!(payload.len(), 1015);
        assert_eq!(&payload[8..12], &[1, 0, 0, 0], "one protected stream");
        assert_eq!(&payload[12..16], &0x2da0_4dd6u32.to_be_bytes());
        assert_eq!(&payload[16..18], &1u16.to_be_bytes(), "SN base");
        assert_eq!(&payload[18..20], &[0xff, 0x00], "k=1 and seven packets");

        // Sequence 8 carried a five-word extension (a 40-byte header): 12 more bytes of extension,
        // modelled here as payload, which the parity treats the same way.
        let second: Vec<Vec<u8>> = [
            (8, 493_560, true, 939 + 12),
            (9, 499_320, false, 500),
            (10, 499_320, true, 499),
            (11, 505_530, true, 862),
            (12, 511_650, true, 958),
            (13, 517_230, true, 939),
            (14, 523_350, false, 568),
            (15, 523_350, true, 567),
        ]
        .into_iter()
        .map(|(sequence, timestamp, marker, payload)| {
            video_packet(sequence, timestamp, marker, payload)
        })
        .collect();
        let refs: Vec<&[u8]> = second.iter().map(Vec::as_slice).collect();
        let payload = encode_fec_payload(&refs).expect("a group");
        assert_eq!(hex::encode(&payload[..4]), "000000d4");
        assert_eq!(payload.len(), 994);
    }

    #[test]
    fn an_iphone_mask_names_its_base_as_bit_zero() {
        // An iPhone's repair packet named base 9 and mask 0xff00 and carried packet 15's
        // timestamp; the other fields here are illustrative.
        let mut header = hex::decode("00e100000007fc56").expect("hex");
        header.extend_from_slice(&[1, 0, 0, 0]);
        header.extend_from_slice(&0x0102_0304u32.to_be_bytes());
        header.extend_from_slice(&9u16.to_be_bytes());
        header.extend_from_slice(&[0xff, 0x00]);
        let (parsed, len) = FecHeader::parse(&header).expect("a repair header");
        assert_eq!(len, 20);
        assert_eq!(parsed.sequence_base, 9);
        assert_eq!(parsed.timestamp, 523_350);
        assert_eq!(
            parsed.protected().collect::<Vec<_>>(),
            (9..=15).collect::<Vec<_>>()
        );
        let mut out = Vec::new();
        assert!(parsed.write_into(&mut out));
        assert_eq!(out, header);
    }

    #[test]
    fn headers_round_trip_through_every_mask_length() {
        for (mask, len) in [
            (0b1u128, 20),
            ((1u128 << 15) - 1, 20),
            (1u128 << 15, 24),
            ((1u128 << 46) - 1, 24),
            (1u128 << 46, 32),
            ((1u128 << 109) - 1, 32),
            (0b1010_0110u128 | (1 << 40) | (1 << 100), 32),
        ] {
            let header = FecHeader {
                padding: true,
                extension: false,
                csrc_count: 3,
                marker: true,
                payload_type: 97,
                length: 0x1234,
                timestamp: 0xdead_beef,
                protected_ssrc: 0x0102_0304,
                sequence_base: 65_530,
                mask,
            };
            let mut out = Vec::new();
            assert!(header.write_into(&mut out));
            assert_eq!(out.len(), len);
            assert_eq!(header.byte_len(), Some(len));
            assert_eq!(FecHeader::parse(&out), Some((header, len)));
        }
        let too_wide = FecHeader {
            padding: false,
            extension: false,
            csrc_count: 0,
            marker: false,
            payload_type: 0,
            length: 0,
            timestamp: 0,
            protected_ssrc: 0,
            sequence_base: 0,
            mask: 1 << 109,
        };
        assert!(!too_wide.write_into(&mut Vec::new()));
    }

    #[test]
    fn a_full_group_has_an_all_ones_mask_and_wraps_its_sequence_numbers() {
        let packets: Vec<Vec<u8>> = (0..15u16)
            .map(|i| {
                video_packet(
                    65_530u16.wrapping_add(i),
                    9_000,
                    i == 14,
                    100 + usize::from(i),
                )
            })
            .collect();
        let refs: Vec<&[u8]> = packets.iter().map(Vec::as_slice).collect();
        let payload = encode_fec_payload(&refs).unwrap();
        assert_eq!(&payload[18..20], &[0xff, 0xff]);
        let (header, _) = FecHeader::parse(&payload).unwrap();
        let protected: Vec<u16> = header.protected().collect();
        assert_eq!(protected.first(), Some(&65_530));
        assert_eq!(protected.last(), Some(&8));
        assert_eq!(protected.len(), 15);
    }

    #[test]
    fn a_gap_or_another_stream_is_not_a_group() {
        let a = video_packet(1, 0, false, 10);
        let b = video_packet(3, 0, true, 10);
        assert_eq!(encode_fec_payload(&[&a, &b]), None);
        let mut other = video_packet(2, 0, true, 10);
        other[8] ^= 1;
        assert_eq!(encode_fec_payload(&[&a, &other]), None);
        assert_eq!(encode_fec_payload(&[]), None);
    }

    #[test]
    fn any_one_lost_packet_is_rebuilt_byte_for_byte() {
        let mut packets: Vec<Vec<u8>> = (0..9u16)
            .map(|i| {
                video_packet(
                    700 + i,
                    3_000 * u32::from(i / 3),
                    i % 3 == 2,
                    40 + 97 * usize::from(i),
                )
            })
            .collect();
        // Padding and a CSRC change the P and CC bits, which come back from the parity too.
        packets[4][0] |= 0x20;
        packets[6][0] = (packets[6][0] & 0xf0) | 0x01;
        let refs: Vec<&[u8]> = packets.iter().map(Vec::as_slice).collect();
        let payload = encode_fec_payload(&refs).unwrap();
        for (lost, packet) in packets.iter().enumerate() {
            let received: Vec<&[u8]> = refs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != lost)
                .map(|(_, packet)| *packet)
                .collect();
            assert_eq!(
                recover_packet(&payload, &received).as_deref(),
                Some(packet.as_slice()),
                "packet {lost}"
            );
        }
        let two_short: Vec<&[u8]> = refs[2..].to_vec();
        assert_eq!(recover_packet(&payload, &two_short), None);
        assert_eq!(recover_packet(&payload, &refs), None, "nothing is missing");
    }

    /// The old behaviour: groups of consecutive packets.
    fn consecutive() -> VideoFecConfig {
        VideoFecConfig::builder()
            .clean_interleave(1)
            .lossy_interleave(1)
            .build()
    }

    fn run(sender: &mut VideoFecSender, frames: &[usize], first: u16) -> (Vec<Vec<u16>>, u16) {
        let mut sequence = first;
        let mut groups = Vec::new();
        for (frame, packets) in frames.iter().enumerate() {
            sender.begin_access_unit(*packets);
            for i in 0..*packets {
                let packet = video_packet(sequence, frame as u32, i + 1 == *packets, 200);
                sequence = sequence.wrapping_add(1);
                let repairs = sender.push(&packet);
                let mut span = Vec::new();
                for payload in &repairs {
                    let (header, _) = FecHeader::parse(payload).unwrap();
                    span.extend(header.protected());
                    groups.push(header.protected().collect());
                }
                if !repairs.is_empty() {
                    span.sort_by_key(|member: &u16| member.wrapping_sub(sequence));
                    assert_eq!(
                        span.last(),
                        Some(&sequence.wrapping_sub(1)),
                        "repair packets follow the last packet of their span"
                    );
                    assert!(
                        span.windows(2)
                            .all(|pair| pair[1] == pair[0].wrapping_add(1)),
                        "a span is consecutive and each packet is in one group"
                    );
                }
            }
        }
        (groups, sequence)
    }

    #[test]
    fn small_frames_share_a_group_that_ends_with_a_frame() {
        let mut sender = VideoFecSender::new(consecutive());
        assert_eq!(sender.group_size(), 8);
        let (groups, _) = run(&mut sender, &[2, 3, 2, 2, 3, 3, 1], 1);
        // 2+3+2 is short of eight; the fourth frame makes nine, which ends the first group.
        assert_eq!(groups, vec![(1..=9).collect::<Vec<u16>>()]);
    }

    #[test]
    fn a_large_frame_is_cut_into_even_groups_of_at_most_fifteen() {
        let mut sender = VideoFecSender::new(consecutive());
        let (groups, _) = run(&mut sender, &[30], 1);
        assert_eq!(
            groups.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![10, 10, 10]
        );
        let (groups, _) = run(&mut sender, &[15], 100);
        assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), vec![15]);
        // Carried packets go in the first group of the next frame.
        let (groups, _) = run(&mut sender, &[5, 20], 200);
        assert_eq!(
            groups.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![9, 8, 8]
        );
        assert!(groups.iter().all(|group| group.len() <= FEC_MAX_GROUP));
    }

    #[test]
    fn changing_the_threshold_reclassifies_the_latest_report() {
        let mut sender = VideoFecSender::new(consecutive());
        sender.observe_loss(13);
        assert_eq!(sender.group_size(), 4);
        sender.set_config(VideoFecConfig {
            loss_threshold: 20,
            ..consecutive()
        });
        assert_eq!(sender.group_size(), 8);
        sender.set_config(consecutive());
        assert_eq!(sender.group_size(), 4);
    }

    #[test]
    fn a_smaller_target_retires_a_larger_carried_group() {
        let mut sender = VideoFecSender::new(consecutive());
        let (groups, next) = run(&mut sender, &[7], 1);
        assert!(groups.is_empty());
        sender.observe_loss(13);
        let (groups, _) = run(&mut sender, &[4], next);
        assert_eq!(groups, vec![(next..next + 4).collect::<Vec<_>>()]);
    }

    #[test]
    fn a_mid_frame_discontinuity_does_not_poison_the_next_plan() {
        let mut sender = VideoFecSender::new(consecutive());
        sender.begin_access_unit(40);
        for sequence in (1..=40).filter(|sequence| *sequence != 2) {
            sender.push(&video_packet(sequence, 0, sequence == 40, 100));
        }
        let (groups, _) = run(&mut sender, &[8], 41);
        assert!(!groups.is_empty());
        assert!(sender.span_count < sender.span_size());
    }

    #[test]
    fn reported_loss_doubles_the_ratio_and_the_ratio_is_capped_at_half() {
        let mut sender = VideoFecSender::new(consecutive());
        sender.observe_loss(5);
        assert_eq!(sender.group_size(), 8, "2% is still a clean path");
        sender.observe_loss(13);
        assert_eq!(sender.group_size(), 4);
        let (groups, _) = run(&mut sender, &[40], 1);
        assert_eq!(groups.len(), 10);
        sender.observe_loss(0);
        assert_eq!(sender.group_size(), 8);

        let mut eager = VideoFecSender::new(
            VideoFecConfig::builder()
                .clean_group(0)
                .lossy_group(200)
                .clean_interleave(1)
                .lossy_interleave(1)
                .build(),
        );
        assert_eq!(
            eager.group_size(),
            2,
            "never more repair than half the video"
        );
        let (groups, _) = run(&mut eager, &[1; 10], 1);
        assert_eq!(groups.len(), 5);
        eager.observe_loss(255);
        assert_eq!(eager.group_size(), 8);
    }

    #[test]
    fn ratio_over_a_steady_stream() {
        let mut sender = VideoFecSender::new(consecutive());
        let frames = [3usize; 200];
        let (groups, _) = run(&mut sender, &frames, 1);
        let video: usize = frames.iter().sum();
        // Groups of 9 (three frames): one repair packet per nine video packets.
        assert_eq!(groups.len(), video / 9);
        assert!(groups.iter().all(|group| group.len() == 9));

        // Interleaved, the ratio holds: spans of 16 in two groups clean (ending inside a frame,
        // whose rest carries over), of 12 in three groups once the peer reports loss.
        let mut sender = VideoFecSender::new(VideoFecConfig::default());
        let (groups, next) = run(&mut sender, &frames, 1);
        assert_eq!(groups.len(), 2 * (video / 16));
        assert!(groups.iter().all(|group| group.len() == 8));
        sender.observe_loss(13);
        let (groups, _) = run(&mut sender, &frames, next);
        assert_eq!(groups.len(), video / 4);
        assert!(groups.iter().all(|group| group.len() == 4));
    }

    #[test]
    fn a_disabled_sender_sends_nothing_and_a_gap_restarts_the_group() {
        let mut sender = VideoFecSender::new(VideoFecConfig::disabled());
        let (groups, _) = run(&mut sender, &[30], 1);
        assert!(groups.is_empty());

        let mut sender = VideoFecSender::new(consecutive());
        let (groups, next) = run(&mut sender, &[5], 1);
        assert!(groups.is_empty());
        // Five carried packets, then a frame that skips a sequence number: the carried five cannot
        // be completed and the frame starts a group of its own.
        let (groups, _) = run(&mut sender, &[4, 4], next + 1);
        assert_eq!(groups, vec![(next + 1..next + 9).collect::<Vec<u16>>()]);
    }

    /// Frame sizes, loss reports and a sequence gap that exercise every planning path.
    fn golden_script() -> Vec<(usize, Option<u8>, u16)> {
        let mut script = Vec::new();
        let sizes = [
            2usize, 3, 2, 2, 3, 3, 1, 30, 15, 5, 20, 7, 4, 1, 1, 1, 40, 9, 8, 16, 17, 3,
        ];
        for round in 0..6u8 {
            for (i, size) in sizes.iter().enumerate() {
                let loss = match (round, i) {
                    (1, 0) | (3, 5) => Some(13),
                    (2, 0) | (4, 11) => Some(0),
                    _ => None,
                };
                let gap = u16::from(round == 2 && i == 9);
                script.push((*size, loss, gap));
            }
        }
        script
    }

    /// Every video packet and repair payload `sender` makes for the golden script, in wire order;
    /// `true` marks a repair payload.
    fn golden_stream(sender: &mut VideoFecSender) -> Vec<(bool, Vec<u8>)> {
        let mut stream = Vec::new();
        let mut sequence = 65_000u16;
        for (frame, (packets, loss, gap)) in golden_script().into_iter().enumerate() {
            if let Some(loss) = loss {
                sender.observe_loss(loss);
            }
            sequence = sequence.wrapping_add(gap);
            sender.begin_access_unit(packets);
            for i in 0..packets {
                let payload = 50 + (usize::from(sequence) * 7) % 900;
                let packet = video_packet(sequence, 90 * frame as u32, i + 1 == packets, payload);
                sequence = sequence.wrapping_add(1);
                let repairs = sender.push(&packet);
                stream.push((false, packet));
                stream.extend(repairs.into_iter().map(|repair| (true, repair)));
            }
        }
        stream
    }

    #[test]
    fn a_depth_of_one_sends_what_consecutive_groups_sent_byte_for_byte() {
        // The count and FNV-1a hash of the repair payloads the sender made for the golden script
        // before groups could be interleaved.
        let mut sender = VideoFecSender::new(consecutive());
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        let mut count = 0;
        for (repair, bytes) in golden_stream(&mut sender) {
            if repair {
                count += 1;
                for byte in bytes {
                    hash ^= u64::from(byte);
                    hash = hash.wrapping_mul(0x0100_0000_01b3);
                }
            }
        }
        assert_eq!((count, hash), (181, 0x29fb_6b1f_fdbe_ac98));
    }

    #[test]
    fn interleaved_masks_name_packets_depth_apart_and_round_trip() {
        let header = |mask: u128| FecHeader {
            padding: false,
            extension: true,
            csrc_count: 0,
            marker: true,
            payload_type: 97,
            length: 0x0abc,
            timestamp: 90_000,
            protected_ssrc: 0x2da0_4dd6,
            sequence_base: 65_534,
            mask,
        };
        let every = |step: usize, count: usize| (0..count).fold(0u128, |m, i| m | 1 << (i * step));
        // Eight packets two apart: k=1, then bits 0, 2, .. 14 of the 15.
        let pairs = header(every(2, 8));
        let mut out = Vec::new();
        assert!(pairs.write_into(&mut out));
        assert_eq!(&out[18..], &[0xd5, 0x55]);
        assert_eq!(
            pairs.protected().collect::<Vec<_>>(),
            vec![65_534, 0, 2, 4, 6, 8, 10, 12],
            "two apart across the sequence number's wrap"
        );
        for (mask, len) in [
            (every(2, 8), 20),
            (every(3, 5), 20),
            (every(3, 6), 24),
            (every(4, 12), 24),
            (every(5, 10), 24),
            (every(5, 11), 32),
            (every(9, 13), 32),
        ] {
            let header = header(mask);
            let mut out = Vec::new();
            assert!(header.write_into(&mut out));
            assert_eq!(out.len(), len, "{mask:#x}");
            assert_eq!(FecHeader::parse(&out), Some((header, len)));
        }
    }

    #[test]
    fn default_spans_interleave_two_groups_clean_and_three_lossy() {
        let mut sender = VideoFecSender::new(VideoFecConfig::default());
        assert_eq!(
            (sender.depth(), sender.group_size(), sender.span_size()),
            (2, 8, 16)
        );
        let (groups, next) = run(&mut sender, &[4, 6, 6], 1);
        assert_eq!(
            groups,
            vec![
                (1..=16).step_by(2).collect::<Vec<u16>>(),
                (2..=16).step_by(2).collect::<Vec<u16>>(),
            ]
        );
        sender.observe_loss(13);
        assert_eq!(
            (sender.depth(), sender.group_size(), sender.span_size()),
            (3, 4, 12)
        );
        let (groups, _) = run(&mut sender, &[12], next);
        assert_eq!(
            groups,
            (0..3)
                .map(|group| (next + group..next + 12).step_by(3).collect())
                .collect::<Vec<Vec<u16>>>()
        );
    }

    #[test]
    fn spans_stay_within_two_byte_masks() {
        assert_eq!(
            (1..=8).map(max_span).collect::<Vec<_>>(),
            vec![15, 16, 15, 16, 15, 18, 21, 16]
        );
        for depth in 1..=8u8 {
            for group in 2..=8u8 {
                let mut sender = VideoFecSender::new(
                    VideoFecConfig::builder()
                        .clean_group(group)
                        .clean_interleave(depth)
                        .build(),
                );
                let frames = [1, 30, 15, 2, 17, 61, 3, 3, 3, 3, 3, 3, 9];
                let mut sequence = 1u16;
                for packets in frames {
                    sender.begin_access_unit(packets);
                    for i in 0..packets {
                        let packet = video_packet(sequence, 0, i + 1 == packets, 300);
                        sequence += 1;
                        for repair in sender.push(&packet) {
                            let (header, len) = FecHeader::parse(&repair).unwrap();
                            assert_eq!(len, FEC_HEADER_LEN, "depth {depth} group {group}");
                            let members: Vec<u16> = header.protected().collect();
                            assert!(
                                members
                                    .windows(2)
                                    .all(|pair| pair[1] - pair[0] == u16::from(depth)),
                                "depth {depth} group {group}: {members:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn large_frames_are_cut_into_whole_spans_and_the_rest_carries_over() {
        let mut sender = VideoFecSender::new(VideoFecConfig::default());
        // Two even spans of 15 would be fine; one of 30 is past a short mask.
        let (groups, next) = run(&mut sender, &[30], 1);
        assert_eq!(groups.iter().map(Vec::len).collect::<Vec<_>>(), vec![8, 8]);
        assert_eq!(sender.span_count, 14);
        // Fourteen carried packets and a frame of three: one span of sixteen, ending inside the
        // frame, and its last packet carries over.
        let (groups, next) = run(&mut sender, &[3], next);
        assert_eq!(
            groups,
            vec![
                (17..=31).step_by(2).collect::<Vec<u16>>(),
                (18..=32).step_by(2).collect::<Vec<u16>>(),
            ]
        );
        assert_eq!(sender.span_count, 1);
        // Lossy, spans of twelve fit a short mask at depth three: 25 packets are two even spans.
        sender.observe_loss(13);
        let (groups, _) = run(&mut sender, &[25], next);
        assert_eq!(
            groups.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![5, 4, 4, 4, 4, 4]
        );
        assert_eq!(sender.span_count, 0);
    }

    #[test]
    fn a_new_depth_retires_the_carried_span() {
        let mut sender = VideoFecSender::new(VideoFecConfig::default());
        let (groups, next) = run(&mut sender, &[5], 1);
        assert!(groups.is_empty());
        sender.observe_loss(13);
        let (groups, _) = run(&mut sender, &[12], next);
        assert_eq!(
            groups,
            (0..3)
                .map(|group| (next + group..next + 12).step_by(3).collect())
                .collect::<Vec<Vec<u16>>>()
        );
    }

    #[test]
    fn any_burst_of_depth_losses_is_rebuilt_byte_for_byte() {
        for config in [
            VideoFecConfig::default(),
            VideoFecConfig::builder()
                .clean_group(4)
                .clean_interleave(4)
                .build(),
        ] {
            let mut sender = VideoFecSender::new(config);
            let stream = golden_stream(&mut sender);
            let video: Vec<&[u8]> = stream
                .iter()
                .filter(|(repair, _)| !repair)
                .map(|(_, packet)| packet.as_slice())
                .collect();
            let repairs: Vec<(FecHeader, &[u8])> = stream
                .iter()
                .filter(|(repair, _)| *repair)
                .map(|(_, payload)| (FecHeader::parse(payload).unwrap().0, payload.as_slice()))
                .collect();
            let sequence = |packet: &[u8]| u16::from_be_bytes([packet[2], packet[3]]);
            let owner_of: std::collections::HashMap<u16, usize> = repairs
                .iter()
                .enumerate()
                .flat_map(|(owner, (header, _))| header.protected().map(move |s| (s, owner)))
                .collect();
            let protected_by = |packet: &[u8]| owner_of.get(&sequence(packet)).copied();
            // A group's stride is its span's depth; a group of one is no constraint.
            let stride = |owner: usize| {
                let members: Vec<u16> = repairs[owner].0.protected().collect();
                members.get(1).map_or(usize::MAX, |second| {
                    usize::from(second.wrapping_sub(members[0]))
                })
            };
            let depths: std::collections::BTreeSet<usize> = (0..repairs.len())
                .map(stride)
                .filter(|depth| *depth != usize::MAX)
                .collect();
            // The golden script changes the loss report, so the default stream has both depths.
            assert!(depths.len() >= 2 || config.clean_interleave == config.lossy_interleave);
            let mut rebuilt = 0;
            for burst_len in 1..=*depths.last().unwrap() {
                for start in 0..=video.len() - burst_len {
                    let burst = &video[start..start + burst_len];
                    // Packets the script's gap or the trailing carry left unprotected are skipped,
                    // and so are bursts longer than a span they touch is deep.
                    let Some(owners) = burst
                        .iter()
                        .map(|packet| protected_by(packet))
                        .collect::<Option<Vec<usize>>>()
                    else {
                        continue;
                    };
                    if owners.iter().any(|owner| stride(*owner) < burst_len) {
                        continue;
                    }
                    let mut distinct = owners.clone();
                    distinct.sort_unstable();
                    distinct.dedup();
                    assert_eq!(distinct.len(), owners.len(), "one loss per group");
                    for (lost, owner) in burst.iter().zip(owners) {
                        let received: Vec<&[u8]> = video
                            .iter()
                            .enumerate()
                            .filter(|(i, packet)| {
                                !(start..start + burst_len).contains(i)
                                    && protected_by(packet) == Some(owner)
                            })
                            .map(|(_, packet)| *packet)
                            .collect();
                        assert_eq!(
                            recover_packet(repairs[owner].1, &received).as_deref(),
                            Some(*lost),
                            "sequence {}",
                            sequence(lost)
                        );
                        rebuilt += 1;
                    }
                }
            }
            assert!(rebuilt > video.len(), "{rebuilt} of {}", video.len());
        }
    }
}
