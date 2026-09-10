//! H.264 Annex-B handling for the video media plane: NAL splitting, RFC 6184
//! packetization (single NAL / FU-A out, single NAL / STAP-A / FU-A in), and an
//! access-unit splitter for byte streams delimited by AUD NALs.
//!
//! The library never encodes or decodes pixels — callers hand us pre-encoded
//! Annex-B access units and receive reassembled ones. Pure, no-Tokio, wasm-safe.

use wacore_binary::Jid;

/// Largest RTP payload we emit before fragmenting a NAL into FU-A units.
/// Matches the reference relay MTU budget used by live WhatsApp interop.
pub const H264_SINGLE_NAL_MAX: usize = 800;
/// FU-A fragment body size: `H264_SINGLE_NAL_MAX` minus indicator + FU header.
const H264_FUA_FRAG_SIZE: usize = H264_SINGLE_NAL_MAX - 2;
/// Reassembly cap: a "NAL" that grows past this is garbage or an attack, not video.
pub const H264_MAX_AU_BYTES: usize = 4 * 1024 * 1024;

const NAL_TYPE_IDR: u8 = 5;
const NAL_TYPE_SEI: u8 = 6;
const NAL_TYPE_SPS: u8 = 7;
const NAL_TYPE_PPS: u8 = 8;
const NAL_TYPE_AUD: u8 = 9;
const NAL_TYPE_STAP_A: u8 = 24;
const NAL_TYPE_FU_A: u8 = 28;

const START_CODE: [u8; 4] = [0, 0, 0, 1];

/// One received access unit, reassembled back into Annex-B form.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoFrame {
    /// Annex-B access unit (`00 00 00 01` start codes included).
    pub data: Vec<u8>,
    /// The AU carries an IDR/SPS/PPS NAL — safe point to (re)start a decoder.
    pub keyframe: bool,
    /// Frame rotation bits (0..3) from RTP metadata, falling back to
    /// `<video device_orientation>` when absent. Display turns clockwise are
    /// respectively 0, 270, 180, and 90 degrees, as verified by the WASM oracle.
    pub orientation: u8,
    /// Group sender identity. Absent on 1:1 video.
    pub sender: Option<Jid>,
    /// Group sender device identity. Absent on 1:1 video.
    pub device: Option<Jid>,
    /// Relay participant id from the authoritative roster.
    pub pid: Option<u32>,
    /// RTP capture timestamp of the access unit (90 kHz video clock).
    pub timestamp: u32,
    /// Call media generation that produced this frame.
    pub generation: u64,
}

impl VideoFrame {
    pub fn new(data: Vec<u8>) -> Self {
        let keyframe = au_is_keyframe(&data);
        Self {
            data,
            keyframe,
            orientation: 0,
            sender: None,
            device: None,
            pid: None,
            timestamp: 0,
            generation: 0,
        }
    }
}

pub fn nal_unit_type(nal: &[u8]) -> u8 {
    nal.first().map(|b| b & 0x1f).unwrap_or(0)
}

/// Iterate the NAL units of an Annex-B buffer (start codes stripped, empty NALs skipped).
pub fn split_annexb(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    SplitAnnexB { data, pos: 0 }
}

struct SplitAnnexB<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Iterator for SplitAnnexB<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<&'a [u8]> {
        loop {
            let start = find_start_code(self.data, self.pos)?;
            let nal_begin = start.end;
            let nal_end = match find_start_code(self.data, nal_begin) {
                Some(next) => next.begin,
                None => self.data.len(),
            };
            self.pos = nal_end;
            let nal = &self.data[nal_begin..nal_end];
            if !nal.is_empty() {
                return Some(nal);
            }
        }
    }
}

struct StartCode {
    /// Index of the first byte of the start code (including a leading zero of
    /// the 4-byte form).
    begin: usize,
    /// Index of the first NAL byte after the start code.
    end: usize,
}

fn find_start_code(data: &[u8], from: usize) -> Option<StartCode> {
    let hay = data.get(from..)?;
    let mut i = 0;
    while i + 3 <= hay.len() {
        if hay[i] == 0 && hay[i + 1] == 0 {
            if hay[i + 2] == 1 {
                // Fold a preceding zero into the start code so AU slicing keeps
                // the conventional 4-byte form intact.
                let begin = if i > 0 && hay[i - 1] == 0 { i - 1 } else { i };
                return Some(StartCode {
                    begin: from + begin,
                    end: from + i + 3,
                });
            }
            if hay[i + 2] == 0 {
                i += 1;
                continue;
            }
        }
        i += 1;
    }
    None
}

/// True when the AU contains an IDR slice or parameter set — the points a
/// decoder can (re)sync from. Use [`au_has_idr`] instead to gate a *resume*
/// after loss: a parameter-set-only AU is a sync marker but not a decodable
/// restart on its own.
pub fn au_is_keyframe(au: &[u8]) -> bool {
    split_annexb(au).any(|nal| {
        matches!(
            nal_unit_type(nal),
            NAL_TYPE_IDR | NAL_TYPE_SPS | NAL_TYPE_PPS
        )
    })
}

/// True when the AU carries an IDR slice — a self-contained decodable restart
/// point. Resuming a dropped stream here (rather than on any parameter set)
/// avoids handing the decoder dependent frames it can't decode yet.
pub fn au_has_idr(au: &[u8]) -> bool {
    split_annexb(au).any(|nal| nal_unit_type(nal) == NAL_TYPE_IDR)
}

/// Capacity a [`PacketizedAu`] keeps between frames. A 1080p keyframe packs into tens of KB,
/// so this holds an ordinary stream's working set without ever reallocating; an access unit
/// may be up to [`H264_MAX_AU_BYTES`] (4 MiB), and since the send path keeps one buffer for
/// the whole call, a single outlier frame would otherwise pin that much per video pipeline
/// until the call ended. `Vec<Vec<u8>>` did not have this problem: clearing it dropped every
/// fragment's allocation outright.
const PACKETIZED_AU_RETAINED_BYTES: usize = 128 * 1024;

/// Payload-count sibling of [`PACKETIZED_AU_RETAINED_BYTES`]: no fragment exceeds
/// `H264_SINGLE_NAL_MAX`, so this many boundaries cover the retained byte budget.
const PACKETIZED_AU_RETAINED_PAYLOADS: usize =
    PACKETIZED_AU_RETAINED_BYTES / H264_SINGLE_NAL_MAX + 1;

/// One access unit's RTP payloads, packed back-to-back in a single buffer.
///
/// The send path holds one across the whole call and hands it to [`packetize_au`] per
/// frame: clearing keeps both allocations, so a steady stream packetizes with zero
/// allocations after the first access unit. A `Vec<Vec<u8>>` instead allocates and frees
/// one buffer per fragment -- roughly 40 per 1080p keyframe, ~1200/s at 30 fps.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct PacketizedAu {
    data: Vec<u8>,
    /// End offset of each payload within `data`; a payload starts where the previous one
    /// ended (0 for the first), so the boundaries need one `usize` per payload, not two.
    ends: Vec<usize>,
}

impl PacketizedAu {
    pub fn len(&self) -> usize {
        self.ends.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&[u8]> {
        let end = *self.ends.get(index)?;
        let start = if index == 0 { 0 } else { self.ends[index - 1] };
        self.data.get(start..end)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &[u8]> {
        (0..self.len()).map(|i| self.get(i).unwrap_or_default())
    }

    /// Drop the previous access unit's payloads, keeping both allocations.
    fn clear(&mut self) {
        self.data.clear();
        self.ends.clear();
    }

    /// Hand back the capacity an outlier frame grew, instead of pinning it for the rest of
    /// the call. Runs once the frame is packed, so it keys on what that frame actually
    /// needed: a stream that stays large never shrinks (and so never regrows), while one
    /// that spikes and returns to normal releases the excess on the very next frame.
    fn release_outlier_capacity(&mut self) {
        if self.data.len() <= PACKETIZED_AU_RETAINED_BYTES
            && self.data.capacity() > PACKETIZED_AU_RETAINED_BYTES
        {
            self.data.shrink_to(PACKETIZED_AU_RETAINED_BYTES);
        }
        if self.ends.len() <= PACKETIZED_AU_RETAINED_PAYLOADS
            && self.ends.capacity() > PACKETIZED_AU_RETAINED_PAYLOADS
        {
            self.ends.shrink_to(PACKETIZED_AU_RETAINED_PAYLOADS);
        }
    }

    /// Bytes currently reserved for payload storage. Test-only: the retention behaviour above
    /// is invisible through the payload accessors, so this is what pins it.
    #[cfg(test)]
    fn payload_capacity(&self) -> usize {
        self.data.capacity()
    }

    /// Close the payload whose bytes were just appended to `data`, recording its boundary.
    fn finish_payload(&mut self) {
        self.ends.push(self.data.len());
    }
}

impl core::ops::Index<usize> for PacketizedAu {
    type Output = [u8];

    fn index(&self, index: usize) -> &[u8] {
        self.get(index).expect("payload index out of range")
    }
}

/// Packetize one Annex-B access unit into WhatsApp RTP payloads (no RTP headers): each
/// media NAL goes out as a single-NAL payload when it fits, or a run of FU-A
/// fragments otherwise. Encoder-only AUDs are omitted, and SEI units are
/// omitted until the first VCL NAL: supplemental metadata no decoder needs
/// for rendering that would otherwise take NALU index 0 from SPS ahead of an
/// IDR. `out` is cleared and refilled so the send path can reuse one buffer
/// per AU.
pub fn packetize_au(au: &[u8], out: &mut PacketizedAu) {
    packetize_au_inner(au, out, false);
}

/// [`packetize_au`] preserving every SEI unit, for the explicit opt-in case of
/// a peer that needs the metadata. The default strips leading SEI; callers
/// keep it only deliberately, never by accident.
pub fn packetize_au_keep_sei(au: &[u8], out: &mut PacketizedAu) {
    packetize_au_inner(au, out, true);
}

fn packetize_au_inner(au: &[u8], out: &mut PacketizedAu, keep_sei: bool) {
    out.clear();
    let mut seen_vcl = false;
    for nal in split_annexb(au) {
        let unit_type = nal_unit_type(nal);
        if unit_type == NAL_TYPE_AUD {
            continue;
        }
        if !keep_sei && !seen_vcl && unit_type == NAL_TYPE_SEI {
            continue;
        }
        if matches!(unit_type, 1..=5) {
            seen_vcl = true;
        }
        if nal.len() <= H264_SINGLE_NAL_MAX {
            out.data.extend_from_slice(nal);
            out.finish_payload();
            continue;
        }
        let indicator = (nal[0] & 0xe0) | NAL_TYPE_FU_A;
        let orig_type = nal[0] & 0x1f;
        let body = &nal[1..];
        let n_frags = body.len().div_ceil(H264_FUA_FRAG_SIZE);
        for (i, chunk) in body.chunks(H264_FUA_FRAG_SIZE).enumerate() {
            let mut fu_header = orig_type;
            if i == 0 {
                fu_header |= 0x80; // S
            }
            if i == n_frags - 1 {
                fu_header |= 0x40; // E
            }
            out.data.push(indicator);
            out.data.push(fu_header);
            out.data.extend_from_slice(chunk);
            out.finish_payload();
        }
    }
    out.release_outlier_capacity();
}

/// Access units completed but not yet returned can briefly exceed one when a
/// timestamp boundary and a marker land in the same packet; cap the backlog so
/// pathological loss can't grow it without bound.
const H264_MAX_READY_AUS: usize = 4;

/// Reassemble RTP payloads back into Annex-B access units. Feed each payload
/// with its RTP sequence number, timestamp, and marker bit; a completed AU is
/// returned when its marker arrives OR when a new RTP timestamp begins the next
/// AU (so a lost marker packet doesn't merge two frames). Malformed or
/// partially-lost input degrades to dropped NALs, never a panic.
#[derive(Default)]
pub struct H264Depacketizer {
    au_buf: Vec<u8>,
    fu_buf: Vec<u8>,
    fu_active: bool,
    /// The sequence number the NEXT fragment of the in-progress FU must carry.
    /// FU-A fragments of one NAL are consecutive packets (RFC 6184 §5.8), so a
    /// gap means a lost fragment: the partial NAL is dropped rather than
    /// emitted truncated (silent corruption until the next keyframe).
    fu_next_seq: u16,
    /// RTP timestamp of the AU currently in `au_buf`. All packets of one AU share
    /// it (RFC 3550), so a change marks a new AU even if the previous marker was
    /// lost.
    au_timestamp: Option<u32>,
    /// Keeps a marker-completed AU closed when one of its packets arrives late.
    last_completed_timestamp: Option<u32>,
    /// AUs completed but not yet returned, paired with their RTP timestamps.
    ready: std::collections::VecDeque<(u32, Vec<u8>)>,
}

impl H264Depacketizer {
    pub fn reset(&mut self) {
        self.au_buf.clear();
        self.fu_buf.clear();
        self.fu_active = false;
        self.au_timestamp = None;
        self.last_completed_timestamp = None;
        self.ready.clear();
    }

    fn queue_ready(&mut self, timestamp: u32, au: Vec<u8>) {
        if self.ready.len() >= H264_MAX_READY_AUS {
            self.ready.pop_front();
        }
        self.ready.push_back((timestamp, au));
    }

    /// Take another completed AU while preserving its RTP timestamp.
    pub fn pop_ready(&mut self) -> Option<(u32, Vec<u8>)> {
        self.ready.pop_front()
    }

    /// A lost end-fragment leaves a stale partial NAL; drop it rather than
    /// splice its bytes into the next NAL.
    fn drop_partial_fu(&mut self) {
        self.fu_buf.clear();
        self.fu_active = false;
    }

    fn append_nal(&mut self, nal: &[u8]) {
        if nal.is_empty() || self.au_buf.len() + START_CODE.len() + nal.len() > H264_MAX_AU_BYTES {
            return;
        }
        self.au_buf.extend_from_slice(&START_CODE);
        self.au_buf.extend_from_slice(nal);
    }

    pub fn push(
        &mut self,
        seq: u16,
        timestamp: u32,
        payload: &[u8],
        marker: bool,
    ) -> Option<(u32, Vec<u8>)> {
        if let Some(cur) = self.au_timestamp {
            if timestamp != cur {
                // Signed wrap-aware compare (RFC 3550): a FORWARD jump begins a new AU, so flush the
                // buffered one even if its marker was lost (two frames must not merge). A BACKWARD one
                // is a reordered packet from an already-past AU — discard it rather than flush the
                // current partial as complete, which would corrupt video on normal reordering.
                if (timestamp.wrapping_sub(cur) as i32) > 0 {
                    // A timestamp boundary invalidates any partial FU, even when
                    // no complete NAL has reached the access-unit buffer yet.
                    self.drop_partial_fu();
                    if !self.au_buf.is_empty() {
                        let au = std::mem::take(&mut self.au_buf);
                        self.queue_ready(cur, au);
                    }
                    self.last_completed_timestamp = Some(cur);
                    self.au_timestamp = Some(timestamp);
                } else {
                    return self.pop_ready();
                }
            }
        } else {
            if let Some(completed) = self.last_completed_timestamp
                && (timestamp.wrapping_sub(completed) as i32) <= 0
            {
                return self.pop_ready();
            }
            self.au_timestamp = Some(timestamp);
        }
        match nal_unit_type(payload) {
            NAL_TYPE_STAP_A => {
                self.drop_partial_fu();
                let mut rest = &payload[1..];
                while rest.len() >= 2 {
                    let len = u16::from_be_bytes([rest[0], rest[1]]) as usize;
                    rest = &rest[2..];
                    if len == 0 || len > rest.len() {
                        break;
                    }
                    let (nal, tail) = rest.split_at(len);
                    self.append_nal(nal);
                    rest = tail;
                }
            }
            NAL_TYPE_FU_A if payload.len() >= 2 => {
                let fu_header = payload[1];
                let start = fu_header & 0x80 != 0;
                let end = fu_header & 0x40 != 0;
                if start {
                    self.drop_partial_fu();
                    self.fu_active = true;
                    self.fu_buf.push((payload[0] & 0xe0) | (fu_header & 0x1f));
                } else if self.fu_active && seq != self.fu_next_seq {
                    // A lost (or reordered) fragment: the bytes on hand no longer
                    // form a prefix of the NAL, so emitting them would hand the
                    // decoder a truncated slice. Drop the partial instead.
                    self.drop_partial_fu();
                }
                if self.fu_active {
                    self.fu_next_seq = seq.wrapping_add(1);
                    if self.fu_buf.len() + payload.len() > H264_MAX_AU_BYTES {
                        self.drop_partial_fu();
                    } else {
                        self.fu_buf.extend_from_slice(&payload[2..]);
                        if end {
                            let nal = std::mem::take(&mut self.fu_buf);
                            self.fu_active = false;
                            self.append_nal(&nal);
                            self.fu_buf = nal; // reuse the allocation
                            self.fu_buf.clear();
                        }
                    }
                }
                // A middle fragment with no start on record means the start was
                // lost: ignore it and wait for the next start bit.
            }
            t if (1..=23).contains(&t) => {
                self.drop_partial_fu();
                self.append_nal(payload);
            }
            // Type 0 (empty/garbage) and unsupported aggregation types are ignored.
            _ => {}
        }
        if marker && !self.au_buf.is_empty() {
            self.drop_partial_fu();
            let completed_timestamp = self.au_timestamp.take().unwrap_or(timestamp);
            self.last_completed_timestamp = Some(completed_timestamp);
            let au = std::mem::take(&mut self.au_buf);
            self.queue_ready(completed_timestamp, au);
        }
        self.pop_ready()
    }
}

/// Split a raw Annex-B byte stream (e.g. an encoder's stdout) into access
/// units, cutting at AUD NALs (type 9) or, for AUD-less encoders, at SPS NALs
/// (type 7) and at IDRs that complete an earlier group: every IDR group opens
/// with SPS, so a parameter set with bytes before it starts the next group,
/// and an IDR arriving when the buffer already holds one closes the previous
/// group instead of batching a GOP under one timestamp. Feed arbitrary
/// chunks; complete AUs come back as they close. PPS alone never cuts — it
/// belongs with the SPS that precedes it, not the slices that follow. Once an
/// AUD is observed, AUDs own the framing and SPS/IDR no longer cut, so
/// AUD-bearing streams behave exactly as before.
#[derive(Default)]
pub struct AnnexBAuSplitter {
    buf: Vec<u8>,
    /// Scan resume point: everything before it was already searched for an AUD.
    scan_pos: usize,
    /// An AUD has been observed: SPS/IDR cutting stays off from here on.
    seen_aud: bool,
    /// The buffered bytes already hold an IDR slice.
    buf_has_idr: bool,
}

impl AnnexBAuSplitter {
    pub fn push(&mut self, data: &[u8], out: &mut Vec<Vec<u8>>) {
        self.buf.extend_from_slice(data);
        loop {
            let Some(sc) = find_start_code(&self.buf, self.scan_pos) else {
                // No start code found. A stream that never yields one (garbage, or a producer
                // emitting no AUDs) would otherwise grow `buf` unbounded across pushes, so cap it
                // here too — keep only the last few bytes so a start code split across chunks still
                // reassembles.
                if self.buf.len() > H264_MAX_AU_BYTES {
                    self.buf.clear();
                }
                self.scan_pos = self.buf.len().saturating_sub(3);
                break;
            };
            let Some(&nal_byte) = self.buf.get(sc.end) else {
                // Start code at the buffer edge: wait for the NAL type byte.
                self.scan_pos = sc.begin;
                break;
            };
            let unit_type = nal_byte & 0x1f;
            if unit_type == NAL_TYPE_AUD {
                self.seen_aud = true;
            }
            // An IDR closes the previous group only when the buffer already
            // holds one: the IDR's own parameter sets still arrive after it
            // here (SPS-first ordering), so cutting on every IDR would orphan
            // them. Evaluated before recording this NAL below.
            let cuts_here = sc.begin > 0
                && (unit_type == NAL_TYPE_AUD
                    || (!self.seen_aud
                        && (unit_type == NAL_TYPE_SPS
                            || (unit_type == NAL_TYPE_IDR && self.buf_has_idr))));
            if cuts_here {
                let rest = self.buf.split_off(sc.begin);
                let au = std::mem::replace(&mut self.buf, rest);
                out.push(au);
                self.scan_pos = 0;
                self.buf_has_idr = false;
            } else {
                self.scan_pos = sc.end;
            }
            if unit_type == NAL_TYPE_IDR {
                self.buf_has_idr = true;
            }
            if self.buf.len() > H264_MAX_AU_BYTES {
                // Runaway buffer means the stream has no AUDs; dropping is
                // safer than emitting a cut mid-NAL.
                self.buf.clear();
                self.scan_pos = 0;
                self.buf_has_idr = false;
            }
        }
    }

    /// Flush the trailing AU on end-of-stream.
    pub fn finish(&mut self) -> Option<Vec<u8>> {
        self.scan_pos = 0;
        if self.buf.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buf))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nal(t: u8, len: usize) -> Vec<u8> {
        let mut n = vec![0x60 | t];
        n.extend((0..len.saturating_sub(1)).map(|i| (i % 251) as u8));
        n
    }

    fn au_from_nals(nals: &[Vec<u8>]) -> Vec<u8> {
        let mut au = Vec::new();
        for n in nals {
            au.extend_from_slice(&START_CODE);
            au.extend_from_slice(n);
        }
        au
    }

    /// Drive the depacketizer with consecutive sequence numbers (the on-wire
    /// order the sender emits), returning the last AU produced.
    fn depacketize_all<'a>(payloads: impl ExactSizeIterator<Item = &'a [u8]>) -> Option<Vec<u8>> {
        let mut d = H264Depacketizer::default();
        let last = payloads.len() - 1;
        let mut au = None;
        // All packets of one AU share a timestamp.
        for (i, p) in payloads.enumerate() {
            if let Some((_, got)) = d.push(i as u16, 9000, p, i == last) {
                au = Some(got);
            }
        }
        au
    }

    #[test]
    fn split_annexb_handles_3_and_4_byte_start_codes() {
        let mut data = vec![0, 0, 1];
        data.extend_from_slice(&nal(1, 4));
        data.extend_from_slice(&START_CODE);
        data.extend_from_slice(&nal(5, 6));
        let nals: Vec<_> = split_annexb(&data).collect();
        assert_eq!(nals.len(), 2);
        assert_eq!(nal_unit_type(nals[0]), 1);
        assert_eq!(nal_unit_type(nals[1]), 5);
    }

    #[test]
    fn split_annexb_skips_empty_nals_and_garbage_prefix() {
        // Garbage before the first start code, and back-to-back start codes.
        let mut data = vec![0xaa, 0xbb];
        data.extend_from_slice(&START_CODE);
        data.extend_from_slice(&START_CODE);
        data.extend_from_slice(&nal(1, 3));
        let nals: Vec<_> = split_annexb(&data).collect();
        assert_eq!(nals.len(), 1);
        assert!(split_annexb(&[]).next().is_none());
        assert!(split_annexb(&[0, 0]).next().is_none());
    }

    #[test]
    fn keyframe_detection() {
        assert!(au_is_keyframe(&au_from_nals(&[
            nal(7, 4),
            nal(8, 4),
            nal(5, 100)
        ])));
        assert!(au_is_keyframe(&au_from_nals(&[nal(9, 2), nal(5, 10)])));
        assert!(!au_is_keyframe(&au_from_nals(&[nal(9, 2), nal(1, 100)])));
        assert!(!au_is_keyframe(&[]));
    }

    /// A spike frame must not pin its buffer for the rest of the call. `Vec<Vec<u8>>` dropped
    /// every fragment allocation on each clear; the packed buffer has to release an outlier
    /// deliberately, or one 4 MiB access unit costs that much per video pipeline until hangup.
    #[test]
    fn an_outlier_access_unit_does_not_pin_its_buffer_for_the_call() {
        let mut payloads = PacketizedAu::default();
        let big = au_from_nals(&[nal(5, 512 * 1024)]);
        packetize_au(&big, &mut payloads);
        assert!(
            payloads.payload_capacity() > PACKETIZED_AU_RETAINED_BYTES,
            "the large AU should have grown the buffer past the retained cap"
        );

        // Back to ordinary frames: the spike's capacity is handed back.
        let small = au_from_nals(&[nal(1, 50)]);
        packetize_au(&small, &mut payloads);
        assert!(
            payloads.payload_capacity() <= PACKETIZED_AU_RETAINED_BYTES,
            "an outlier's capacity must not survive the next frame"
        );
        assert_eq!(depacketize_all(payloads.iter()), Some(small));

        // And the steady state still reuses one buffer: no growth across ordinary frames.
        let steady = payloads.payload_capacity();
        for _ in 0..4 {
            packetize_au(&au_from_nals(&[nal(1, 400)]), &mut payloads);
            assert_eq!(
                payloads.payload_capacity(),
                steady,
                "an ordinary frame must not reallocate"
            );
        }

        // A run of large frames keeps its buffer rather than shrinking and regrowing each time.
        packetize_au(&big, &mut payloads);
        let large = payloads.payload_capacity();
        packetize_au(&big, &mut payloads);
        assert_eq!(
            payloads.payload_capacity(),
            large,
            "consecutive large frames must not thrash the allocator"
        );
    }

    #[test]
    fn single_nal_round_trips() {
        let au = au_from_nals(&[nal(1, 100)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        assert_eq!(payloads.len(), 1);
        assert!(payloads.iter().all(|p| p.len() <= H264_SINGLE_NAL_MAX));
        assert_eq!(depacketize_all(payloads.iter()), Some(au));
    }

    #[test]
    fn whatsapp_packetization_omits_aud_before_parameter_sets() {
        let sps = nal(7, 20);
        let pps = nal(8, 8);
        let idr = nal(5, 100);
        let au = au_from_nals(&[nal(9, 2), sps.clone(), pps.clone(), idr.clone()]);
        let mut payloads = PacketizedAu::default();

        packetize_au(&au, &mut payloads);

        assert_eq!(
            payloads.iter().map(nal_unit_type).collect::<Vec<_>>(),
            [NAL_TYPE_SPS, NAL_TYPE_PPS, NAL_TYPE_IDR]
        );
        assert_eq!(
            depacketize_all(payloads.iter()),
            Some(au_from_nals(&[sps, pps, idr]))
        );
    }

    #[test]
    fn aud_only_access_unit_produces_no_rtp_payload() {
        let mut payloads = PacketizedAu::default();
        packetize_au(&au_from_nals(&[nal(1, 40)]), &mut payloads);
        packetize_au(&au_from_nals(&[nal(9, 2)]), &mut payloads);
        assert!(payloads.is_empty());
    }

    #[test]
    fn boundary_800_stays_single_and_801_fragments() {
        let au = au_from_nals(&[nal(1, H264_SINGLE_NAL_MAX)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        assert_eq!(payloads.len(), 1, "800-byte NAL must not fragment");

        let au = au_from_nals(&[nal(1, H264_SINGLE_NAL_MAX + 1)]);
        packetize_au(&au, &mut payloads);
        assert_eq!(payloads.len(), 2, "801-byte NAL must fragment");
        assert_eq!(nal_unit_type(&payloads[0]), NAL_TYPE_FU_A);
        assert_eq!(payloads[0][1] & 0x80, 0x80, "first fragment sets S");
        assert_eq!(payloads[1][1] & 0x40, 0x40, "last fragment sets E");
        assert_eq!(depacketize_all(payloads.iter()), Some(au));
    }

    #[test]
    fn large_idr_round_trips_via_fua() {
        let au = au_from_nals(&[nal(7, 20), nal(8, 8), nal(5, 3000)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        assert!(payloads.len() >= 5);
        let got = depacketize_all(payloads.iter()).expect("AU must reassemble");
        assert_eq!(got, au);
        assert!(au_is_keyframe(&got));
    }

    #[test]
    fn stap_a_unpacks_both_nals() {
        let a = nal(7, 5);
        let b = nal(8, 3);
        let mut stap = vec![0x60 | NAL_TYPE_STAP_A];
        for n in [&a, &b] {
            stap.extend_from_slice(&(n.len() as u16).to_be_bytes());
            stap.extend_from_slice(n);
        }
        let got = depacketize_all([stap.as_slice()].into_iter()).expect("STAP-A must unpack");
        assert_eq!(got, au_from_nals(&[a, b]));
    }

    // A lost middle fragment leaves the sequence gapped, so the truncated NAL
    // must be DROPPED (not emitted truncated, which would corrupt the decode
    // until the next keyframe), and a following AU with contiguous seqs decodes.
    #[test]
    fn lost_middle_fragment_drops_the_truncated_nal() {
        let au = au_from_nals(&[nal(5, 3000)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        assert!(payloads.len() >= 3);
        let mut d = H264Depacketizer::default();
        // seq 0 = start; seq 1 is LOST; seq 2.. arrive, so the E fragment lands
        // on a gapped sequence and the partial NAL is discarded.
        let mut seq = 0u16;
        for (i, p) in payloads.iter().enumerate() {
            if i == 1 {
                seq = seq.wrapping_add(1); // the lost fragment still consumed a seq
                continue;
            }
            let marker = i == payloads.len() - 1;
            let out = d.push(seq, 7000, p, marker);
            if marker {
                assert_eq!(
                    out, None,
                    "a NAL with a lost interior fragment must be dropped, not emitted truncated"
                );
            }
            seq = seq.wrapping_add(1);
        }
        // A subsequent complete AU (contiguous seqs) still reassembles.
        let au2 = au_from_nals(&[nal(1, 50)]);
        let mut p2 = PacketizedAu::default();
        packetize_au(&au2, &mut p2);
        assert_eq!(depacketize_all(p2.iter()), Some(au2));
    }

    // A reordered fragment (seq jumps) is treated the same as loss: the partial
    // NAL is dropped rather than spliced out of order.
    #[test]
    fn reordered_fu_fragment_drops_the_partial() {
        let au = au_from_nals(&[nal(5, 2500)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        let mut d = H264Depacketizer::default();
        // Feed start at seq 0, then jump the next fragment's seq forward.
        assert_eq!(d.push(0, 7000, &payloads[0], false), None);
        let out = d.push(5, 7000, &payloads[1], false); // gap: 1..5 missing
        assert_eq!(
            out, None,
            "a gapped fragment must not extend the partial NAL"
        );
    }

    #[test]
    fn timestamp_change_clears_incomplete_fu_before_middle_fragment() {
        let au = au_from_nals(&[nal(5, 2500)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        assert!(payloads.len() >= 3);

        let mut d = H264Depacketizer::default();
        assert_eq!(d.push(0, 1000, &payloads[0], false), None);
        // A middle fragment from a new AU must not inherit the old FU.
        assert_eq!(d.push(1, 2000, &payloads[1], false), None);
        assert_eq!(d.push(2, 2000, &payloads[2], true), None);

        let next = nal(1, 40);
        assert_eq!(
            d.push(3, 3000, &next, true),
            Some((3000, au_from_nals(&[next])))
        );
    }

    #[test]
    fn lost_end_fragment_discards_partial_and_keeps_next_nal() {
        let au = au_from_nals(&[nal(5, 2000)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        let mut d = H264Depacketizer::default();
        // The E fragment is "lost": everything but the last payload arrives.
        for (i, p) in payloads.iter().take(payloads.len() - 1).enumerate() {
            assert_eq!(d.push(i as u16, 7000, p, false), None);
        }
        // Next single NAL arrives with the marker: partial FU is dropped, the
        // fresh NAL survives alone.
        let tail = nal(1, 40);
        let got = d
            .push(100, 13000, &tail, true)
            .expect("fresh NAL must flush");
        assert_eq!(got, (13000, au_from_nals(&[tail])));
    }

    #[test]
    fn depacketizer_survives_garbage() {
        let bufs: Vec<Vec<u8>> = vec![
            vec![],
            vec![0x7c],                   // FU-A indicator with no header
            vec![0x7c, 0x00],             // FU-A middle with no start seen
            vec![0x7c, 0xc0],             // FU-A with S and E, empty body
            vec![0x78, 0x00, 0xff, 0xaa], // STAP-A with lying length
            vec![0x78],                   // STAP-A with no body
            vec![0x1f; 3],                // reserved type 31
            vec![0x00; 8],                // type 0
            vec![0xff; 900],
        ];
        let mut d = H264Depacketizer::default();
        for (i, b) in bufs.iter().enumerate() {
            let _ = d.push(i as u16, i as u32, b, false);
            let _ = d.push(i as u16, i as u32, b, true);
        }
        // Still functional afterwards.
        let au = au_from_nals(&[nal(1, 10)]);
        let mut p = PacketizedAu::default();
        packetize_au(&au, &mut p);
        assert_eq!(depacketize_all(p.iter()), Some(au));
    }

    // A lost marker packet must not merge two AUs: the next AU's new RTP timestamp flushes the
    // buffered one, so each frame comes back separately (the first missing only its lost NAL).
    #[test]
    fn lost_marker_does_not_merge_frames_across_a_timestamp_change() {
        let mut d = H264Depacketizer::default();
        // AU1 @ ts 1000: two single-NAL packets, but the SECOND (its marker) is "lost".
        let a1 = nal(1, 40);
        assert_eq!(d.push(0, 1000, &a1, false), None);
        // (the marker packet of AU1 never arrives)

        // AU2 @ ts 2000 begins: its first packet's new timestamp flushes AU1.
        let b1 = nal(1, 50);
        let flushed = d
            .push(2, 2000, &b1, false)
            .expect("a new timestamp must flush the previous AU whose marker was lost");
        assert_eq!(flushed, (1000, au_from_nals(&[a1])));
        // AU2 completes normally on its marker.
        let b2 = nal(5, 30);
        let au2 = d
            .push(3, 2000, &b2, true)
            .expect("AU2 completes on its marker");
        assert_eq!(au2, (2000, au_from_nals(&[b1, b2])));
    }

    // A reordered packet from an OLDER timestamp must not flush the current AU as complete: it is
    // discarded, and the in-progress AU keeps accumulating and completes normally.
    #[test]
    fn reordered_older_timestamp_packet_is_discarded_not_flushed() {
        let mut d = H264Depacketizer::default();
        // AU1 @ ts 1000 completes cleanly.
        let a1 = nal(1, 20);
        let want_a1 = au_from_nals(std::slice::from_ref(&a1));
        assert_eq!(d.push(0, 1000, &a1, true), Some((1000, want_a1)));
        // AU2 @ ts 2000 starts (first of two packets, no marker yet).
        let b1 = nal(1, 30);
        assert_eq!(d.push(1, 2000, &b1, false), None);
        // A LATE reordered packet from ts 1000 arrives — it must be discarded, NOT flush the partial
        // AU2 as if complete.
        let stale = nal(1, 10);
        assert_eq!(
            d.push(2, 1000, &stale, false),
            None,
            "a stale reordered packet must not flush the in-progress AU"
        );
        // AU2 completes normally on its marker, intact.
        let b2 = nal(5, 25);
        assert_eq!(
            d.push(3, 2000, &b2, true),
            Some((2000, au_from_nals(&[b1, b2]))),
            "the in-progress AU survives the reordered packet and completes on its marker"
        );
    }

    #[test]
    fn late_packet_after_marker_cannot_seed_a_stale_au() {
        let mut d = H264Depacketizer::default();
        let completed = nal(5, 20);
        assert_eq!(
            d.push(10, 1000, &completed, true),
            Some((1000, au_from_nals(std::slice::from_ref(&completed))))
        );

        let late = nal(1, 15);
        assert_eq!(d.push(9, 1000, &late, false), None);
        let next = nal(1, 25);
        assert_eq!(
            d.push(11, 2000, &next, true),
            Some((2000, au_from_nals(std::slice::from_ref(&next)))),
            "a late packet from the completed timestamp must not leak into the next AU"
        );
    }

    // The rare case where a timestamp boundary AND a marker fire for the same push: both AUs must
    // surface immediately, even if no later packet arrives.
    #[test]
    fn boundary_and_marker_in_one_push_surface_both_aus() {
        let mut d = H264Depacketizer::default();
        // AU1 @ ts 1000 buffered, marker lost.
        let a1 = nal(1, 20);
        assert_eq!(d.push(0, 1000, &a1, false), None);
        // AU2 @ ts 2000 is a single-packet AU WITH a marker: boundary flushes AU1, marker completes AU2.
        let b1 = nal(5, 25);
        let first = d
            .push(1, 2000, &b1, true)
            .expect("boundary flush returns AU1");
        assert_eq!(first, (1000, au_from_nals(&[a1])));
        let second = d
            .pop_ready()
            .expect("the second completed AU is ready without another packet");
        assert_eq!(second, (2000, au_from_nals(&[b1])));
        assert_eq!(d.pop_ready(), None);
    }

    #[test]
    fn empty_au_yields_no_payloads_and_marker_alone_yields_none() {
        let mut payloads = PacketizedAu::default();
        packetize_au(&au_from_nals(&[nal(1, 40)]), &mut payloads);
        packetize_au(&[], &mut payloads);
        assert!(payloads.is_empty(), "packetize_au must clear stale output");
        let mut d = H264Depacketizer::default();
        assert_eq!(d.push(0, 0, &[], true), None);
    }

    /// An AUD-less encoder still frames: every IDR group opens with SPS (the
    /// decoder keyframe gate requires it), so SPS starts a new access unit
    /// when bytes precede it. Without this, an AUD-less stream accumulates to
    /// the 4 MiB cap and is cleared — the call sends nothing, the peer PLIs
    /// forever, and forced IDRs die in the same buffer.
    #[test]
    fn au_splitter_cuts_on_sps_without_aud() {
        let group = |id: u8| {
            let mut au = au_from_nals(&[nal(7, 4), nal(8, 4), nal(5, 60)]);
            au[6] = id;
            au
        };
        let delta = au_from_nals(&[nal(1, 40)]);
        let mut stream = group(1);
        stream.extend_from_slice(&delta);
        stream.extend_from_slice(&group(2));
        let mut s = AnnexBAuSplitter::default();
        let mut out = Vec::new();
        s.push(&stream, &mut out);
        // Like the AUD cut, the boundary lands when the NEXT group's opener
        // arrives: the delta rides with the group it follows, exactly as with
        // a trailing AUD.
        let mut first = group(1);
        first.extend_from_slice(&delta);
        assert_eq!(out, vec![first]);
        assert_eq!(s.finish(), Some(group(2)));
    }

    /// An IDR arriving when the buffer already holds one closes the previous
    /// group instead of batching a GOP under one timestamp: delta frames ride
    /// with the group they follow, and the trailing IDR starts the next.
    #[test]
    fn au_splitter_cuts_on_idr_after_a_complete_group() {
        let group = au_from_nals(&[nal(7, 4), nal(8, 4), nal(5, 60)]);
        let delta = au_from_nals(&[nal(1, 40)]);
        let lone_idr = au_from_nals(&[nal(5, 60)]);
        let mut stream = group.clone();
        stream.extend_from_slice(&delta);
        stream.extend_from_slice(&lone_idr);
        let mut s = AnnexBAuSplitter::default();
        let mut out = Vec::new();
        s.push(&stream, &mut out);
        let mut first = group;
        first.extend_from_slice(&delta);
        assert_eq!(out, vec![first]);
        assert_eq!(s.finish(), Some(lone_idr));
    }

    #[test]
    fn au_splitter_cuts_on_aud() {
        let au1 = au_from_nals(&[nal(9, 2), nal(7, 4), nal(5, 60)]);
        let au2 = au_from_nals(&[nal(9, 2), nal(1, 40)]);
        let mut stream = au1.clone();
        stream.extend_from_slice(&au2);
        let mut s = AnnexBAuSplitter::default();
        let mut out = Vec::new();
        s.push(&stream, &mut out);
        assert_eq!(out, vec![au1]);
        assert_eq!(s.finish(), Some(au2));
        assert_eq!(s.finish(), None);
    }

    #[test]
    fn au_splitter_handles_start_code_split_across_chunks() {
        let au1 = au_from_nals(&[nal(9, 2), nal(1, 30)]);
        let au2 = au_from_nals(&[nal(9, 2), nal(1, 20)]);
        let mut stream = au1.clone();
        stream.extend_from_slice(&au2);
        // Feed byte by byte: start codes and the AUD type byte land on every
        // possible chunk boundary.
        let mut s = AnnexBAuSplitter::default();
        let mut out = Vec::new();
        for b in &stream {
            s.push(std::slice::from_ref(b), &mut out);
        }
        assert_eq!(out, vec![au1]);
        assert_eq!(s.finish(), Some(au2));
    }

    #[test]
    fn au_splitter_without_aud_does_not_grow_unbounded() {
        let mut s = AnnexBAuSplitter::default();
        let mut out = Vec::new();
        let chunk = au_from_nals(&[nal(1, 64 * 1024)]);
        for _ in 0..80 {
            s.push(&chunk, &mut out);
        }
        assert!(out.is_empty());
        // Internal buffer was capped, not grown to ~5 MiB.
        assert!(s.buf.len() <= H264_MAX_AU_BYTES);
    }

    #[test]
    fn au_splitter_pure_garbage_no_start_code_is_capped() {
        // A stream that never yields a start code must not grow the buffer without bound.
        let mut s = AnnexBAuSplitter::default();
        let mut out = Vec::new();
        let chunk = vec![0xabu8; 64 * 1024];
        for _ in 0..80 {
            s.push(&chunk, &mut out);
        }
        assert!(out.is_empty());
        assert!(
            s.buf.len() <= H264_MAX_AU_BYTES,
            "no-start-code stream must be capped, got {}",
            s.buf.len()
        );
    }

    #[test]
    fn video_frame_new_detects_keyframe() {
        let f = VideoFrame::new(au_from_nals(&[nal(5, 30)]));
        assert!(f.keyframe);
        assert_eq!(f.orientation, 0);
        let f = VideoFrame::new(au_from_nals(&[nal(1, 30)]));
        assert!(!f.keyframe);
    }

    /// Decoder keyframe gate (WhatsApp decoder): every IDR group must open
    /// with SPS at NALU index 0 plus PPS, SPS bytes constant all call, no
    /// SEI/AUD at index 0. Round-trips three IDR groups with one constant SPS
    /// through the real send path (`packetize_au`, including an FU-A-split
    /// IDR) into the depacketizer and pins the emitted order.
    #[test]
    fn outbound_idr_groups_open_sps_pps_idr_with_constant_sps() {
        let sps = nal(7, 24);
        let pps = nal(8, 8);
        // Two IDR groups sharing one SPS, with a delta frame between: the
        // stream shape a live call repeats.
        let groups = [
            au_from_nals(&[sps.clone(), pps.clone(), nal(5, 2000)]),
            au_from_nals(&[nal(1, 60)]),
            au_from_nals(&[sps.clone(), pps.clone(), nal(5, 900)]),
        ];
        for au in &groups {
            let mut payloads = PacketizedAu::default();
            packetize_au(au, &mut payloads);
            let got = depacketize_all(payloads.iter()).expect("reassembled AU");
            let nals: Vec<_> = split_annexb(&got).collect();
            let types: Vec<u8> = nals.iter().map(|n| nal_unit_type(n)).collect();
            if au_has_idr(au) {
                assert!(
                    types.len() >= 3,
                    "an IDR group must keep SPS, PPS and IDR, got {types:?}"
                );
                assert_eq!(
                    types[0], 7,
                    "SPS must open the IDR group at NALU index 0, got {types:?}"
                );
                assert_eq!(types[1], 8, "PPS must follow SPS at index 1, got {types:?}");
                assert!(
                    types.contains(&5),
                    "the IDR slice must survive, got {types:?}"
                );
                assert!(
                    !types.contains(&9),
                    "no AUD may reach the wire, got {types:?}"
                );
                assert_eq!(
                    nals[0],
                    sps.as_slice(),
                    "SPS bytes must stay constant across the call"
                );
            } else {
                assert_eq!(types, [1], "delta frames pass through untouched");
            }
        }
    }

    /// Encoder-only AUDs never reach the wire: the packetizer drops them so a
    /// supplier-side AUD cannot take NALU index 0 from SPS.
    #[test]
    fn outbound_aud_first_input_still_opens_sps() {
        let sps = nal(7, 24);
        let au = au_from_nals(&[nal(9, 2), sps.clone(), nal(8, 8), nal(5, 100)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        let got = depacketize_all(payloads.iter()).expect("reassembled AU");
        let types: Vec<u8> = split_annexb(&got).map(nal_unit_type).collect();
        assert_eq!(types[0], 7, "AUD dropped, SPS opens, got {types:?}");
    }

    /// SEI NALs are supplemental metadata no decoder needs for rendering, and
    /// a leading SEI breaks the decoder keyframe gate (SPS must open at index
    /// 0). The packetizer strips them exactly like encoder-only AUDs; a peer
    /// that needs SEI passthrough uses `packetize_au_keep_sei` explicitly.
    #[test]
    fn outbound_sei_first_input_goes_out_sps_first() {
        let sps = nal(7, 24);
        let au = au_from_nals(&[nal(6, 12), sps.clone(), nal(8, 8), nal(5, 100)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        let got = depacketize_all(payloads.iter()).expect("reassembled AU");
        let nals: Vec<_> = split_annexb(&got).collect();
        let types: Vec<u8> = nals.iter().map(|n| nal_unit_type(n)).collect();
        assert_eq!(types[0], 7, "SEI stripped, SPS opens, got {types:?}");
        assert_eq!(nals[0], sps.as_slice(), "SPS bytes intact");
        assert!(
            !types.contains(&6),
            "no SEI may reach the wire, got {types:?}"
        );
    }

    /// Explicit opt-in for the strip default above: keeps SEI for a peer that
    /// needs the metadata, verified byte-identical apart from the kept unit.
    #[test]
    fn keep_sei_variant_preserves_sei() {
        let au = au_from_nals(&[nal(6, 12), nal(7, 24), nal(8, 8), nal(5, 100)]);
        let mut payloads = PacketizedAu::default();
        packetize_au_keep_sei(&au, &mut payloads);
        let got = depacketize_all(payloads.iter()).expect("reassembled AU");
        let types: Vec<u8> = split_annexb(&got).map(nal_unit_type).collect();
        assert_eq!(types[0], 6, "keep variant preserves SEI, got {types:?}");
    }

    /// SEI after the first VCL NAL is metadata, not a gate violation: only
    /// leading SEI is stripped.
    #[test]
    fn outbound_trailing_sei_is_preserved() {
        let au = au_from_nals(&[nal(7, 24), nal(8, 8), nal(5, 100), nal(6, 12)]);
        let mut payloads = PacketizedAu::default();
        packetize_au(&au, &mut payloads);
        let got = depacketize_all(payloads.iter()).expect("reassembled AU");
        let types: Vec<u8> = split_annexb(&got).map(nal_unit_type).collect();
        assert_eq!(types, [7, 8, 5, 6], "trailing SEI kept, got {types:?}");
    }
}
