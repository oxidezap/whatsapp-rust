//! H.264 Annex-B handling for the video media plane: NAL splitting, RFC 6184
//! packetization (single NAL / FU-A out, single NAL / STAP-A / FU-A in), and an
//! access-unit splitter for byte streams delimited by AUD NALs.
//!
//! The library never encodes or decodes pixels — callers hand us pre-encoded
//! Annex-B access units and receive reassembled ones. Pure, no-Tokio, wasm-safe.

/// Largest RTP payload we emit before fragmenting a NAL into FU-A units.
/// Mirrors the reference implementation's relay MTU budget; unvalidated live.
pub const H264_SINGLE_NAL_MAX: usize = 800;
/// FU-A fragment body size: `H264_SINGLE_NAL_MAX` minus indicator + FU header.
const H264_FUA_FRAG_SIZE: usize = H264_SINGLE_NAL_MAX - 2;
/// Reassembly cap: a "NAL" that grows past this is garbage or an attack, not video.
pub const H264_MAX_AU_BYTES: usize = 4 * 1024 * 1024;

const NAL_TYPE_IDR: u8 = 5;
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
    /// Peer device orientation in 90° steps (0..3), from `<video device_orientation>`.
    pub orientation: u8,
}

impl VideoFrame {
    pub fn new(data: Vec<u8>) -> Self {
        let keyframe = au_is_keyframe(&data);
        Self {
            data,
            keyframe,
            orientation: 0,
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

/// Packetize one Annex-B access unit into WhatsApp RTP payloads (no RTP headers): each
/// media NAL goes out as a single-NAL payload when it fits, or a run of FU-A
/// fragments otherwise. Encoder-only AUDs are omitted because WhatsApp's H.264
/// decoder requires SPS/PPS to lead an IDR frame. `out` is cleared and refilled
/// so the send path can reuse one buffer per AU.
pub fn packetize_au(au: &[u8], out: &mut Vec<Vec<u8>>) {
    out.clear();
    for nal in split_annexb(au) {
        if nal_unit_type(nal) == NAL_TYPE_AUD {
            continue;
        }
        if nal.len() <= H264_SINGLE_NAL_MAX {
            out.push(nal.to_vec());
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
            let mut pkt = Vec::with_capacity(2 + chunk.len());
            pkt.push(indicator);
            pkt.push(fu_header);
            pkt.extend_from_slice(chunk);
            out.push(pkt);
        }
    }
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
    /// AUs completed this or a prior push but not yet handed back (drained one per
    /// `push`), so a timestamp boundary coinciding with a marker never drops one.
    ready: std::collections::VecDeque<Vec<u8>>,
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

    fn queue_ready(&mut self, au: Vec<u8>) {
        if self.ready.len() >= H264_MAX_READY_AUS {
            self.ready.pop_front();
        }
        self.ready.push_back(au);
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
    ) -> Option<Vec<u8>> {
        if let Some(cur) = self.au_timestamp {
            if timestamp != cur {
                // Signed wrap-aware compare (RFC 3550): a FORWARD jump begins a new AU, so flush the
                // buffered one even if its marker was lost (two frames must not merge). A BACKWARD one
                // is a reordered packet from an already-past AU — discard it rather than flush the
                // current partial as complete, which would corrupt video on normal reordering.
                if (timestamp.wrapping_sub(cur) as i32) > 0 {
                    if !self.au_buf.is_empty() {
                        self.drop_partial_fu();
                        let au = std::mem::take(&mut self.au_buf);
                        self.queue_ready(au);
                    }
                    self.last_completed_timestamp = Some(cur);
                    self.au_timestamp = Some(timestamp);
                } else {
                    return self.ready.pop_front();
                }
            }
        } else {
            if let Some(completed) = self.last_completed_timestamp
                && (timestamp.wrapping_sub(completed) as i32) <= 0
            {
                return self.ready.pop_front();
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
        if let Some(au) = self.flush_on(marker) {
            self.queue_ready(au);
        }
        // Drain one completed AU per push; any second (a timestamp boundary that coincided with a
        // marker) surfaces on the next call, adding at most one packet of latency.
        self.ready.pop_front()
    }

    fn flush_on(&mut self, marker: bool) -> Option<Vec<u8>> {
        if !marker {
            return None;
        }
        self.drop_partial_fu();
        self.last_completed_timestamp = self.au_timestamp.take();
        if self.au_buf.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.au_buf))
    }
}

/// Split a raw Annex-B byte stream (e.g. an encoder's stdout) into access
/// units, cutting at AUD NALs (type 9). Feed arbitrary chunks; complete AUs
/// come back as they close. Requires the producer to emit AUDs (ffmpeg:
/// `-bsf:v h264_metadata=aud=insert`).
#[derive(Default)]
pub struct AnnexBAuSplitter {
    buf: Vec<u8>,
    /// Scan resume point: everything before it was already searched for an AUD.
    scan_pos: usize,
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
            if nal_byte & 0x1f == NAL_TYPE_AUD && sc.begin > 0 {
                let rest = self.buf.split_off(sc.begin);
                let au = std::mem::replace(&mut self.buf, rest);
                out.push(au);
                self.scan_pos = 0;
            } else {
                self.scan_pos = sc.end;
            }
            if self.buf.len() > H264_MAX_AU_BYTES {
                // Runaway buffer means the stream has no AUDs; dropping is
                // safer than emitting a cut mid-NAL.
                self.buf.clear();
                self.scan_pos = 0;
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
    fn depacketize_all(payloads: &[Vec<u8>]) -> Option<Vec<u8>> {
        let mut d = H264Depacketizer::default();
        let last = payloads.len() - 1;
        let mut au = None;
        // All packets of one AU share a timestamp.
        for (i, p) in payloads.iter().enumerate() {
            if let Some(got) = d.push(i as u16, 9000, p, i == last) {
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

    #[test]
    fn single_nal_round_trips() {
        let au = au_from_nals(&[nal(1, 100)]);
        let mut payloads = Vec::new();
        packetize_au(&au, &mut payloads);
        assert_eq!(payloads.len(), 1);
        assert!(payloads.iter().all(|p| p.len() <= H264_SINGLE_NAL_MAX));
        assert_eq!(depacketize_all(&payloads), Some(au));
    }

    #[test]
    fn whatsapp_packetization_omits_aud_before_parameter_sets() {
        let sps = nal(7, 20);
        let pps = nal(8, 8);
        let idr = nal(5, 100);
        let au = au_from_nals(&[nal(9, 2), sps.clone(), pps.clone(), idr.clone()]);
        let mut payloads = Vec::new();

        packetize_au(&au, &mut payloads);

        assert_eq!(
            payloads
                .iter()
                .map(|nal| nal_unit_type(nal))
                .collect::<Vec<_>>(),
            [NAL_TYPE_SPS, NAL_TYPE_PPS, NAL_TYPE_IDR]
        );
        assert_eq!(
            depacketize_all(&payloads),
            Some(au_from_nals(&[sps, pps, idr]))
        );
    }

    #[test]
    fn aud_only_access_unit_produces_no_rtp_payload() {
        let mut payloads = vec![vec![1]];
        packetize_au(&au_from_nals(&[nal(9, 2)]), &mut payloads);
        assert!(payloads.is_empty());
    }

    #[test]
    fn boundary_800_stays_single_and_801_fragments() {
        let au = au_from_nals(&[nal(1, H264_SINGLE_NAL_MAX)]);
        let mut payloads = Vec::new();
        packetize_au(&au, &mut payloads);
        assert_eq!(payloads.len(), 1, "800-byte NAL must not fragment");

        let au = au_from_nals(&[nal(1, H264_SINGLE_NAL_MAX + 1)]);
        packetize_au(&au, &mut payloads);
        assert_eq!(payloads.len(), 2, "801-byte NAL must fragment");
        assert_eq!(nal_unit_type(&payloads[0]), NAL_TYPE_FU_A);
        assert_eq!(payloads[0][1] & 0x80, 0x80, "first fragment sets S");
        assert_eq!(payloads[1][1] & 0x40, 0x40, "last fragment sets E");
        assert_eq!(depacketize_all(&payloads), Some(au));
    }

    #[test]
    fn large_idr_round_trips_via_fua() {
        let au = au_from_nals(&[nal(7, 20), nal(8, 8), nal(5, 3000)]);
        let mut payloads = Vec::new();
        packetize_au(&au, &mut payloads);
        assert!(payloads.len() >= 5);
        let got = depacketize_all(&payloads).expect("AU must reassemble");
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
        let got = depacketize_all(&[stap]).expect("STAP-A must unpack");
        assert_eq!(got, au_from_nals(&[a, b]));
    }

    // A lost middle fragment leaves the sequence gapped, so the truncated NAL
    // must be DROPPED (not emitted truncated, which would corrupt the decode
    // until the next keyframe), and a following AU with contiguous seqs decodes.
    #[test]
    fn lost_middle_fragment_drops_the_truncated_nal() {
        let au = au_from_nals(&[nal(5, 3000)]);
        let mut payloads = Vec::new();
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
        let mut p2 = Vec::new();
        packetize_au(&au2, &mut p2);
        assert_eq!(depacketize_all(&p2), Some(au2));
    }

    // A reordered fragment (seq jumps) is treated the same as loss: the partial
    // NAL is dropped rather than spliced out of order.
    #[test]
    fn reordered_fu_fragment_drops_the_partial() {
        let au = au_from_nals(&[nal(5, 2500)]);
        let mut payloads = Vec::new();
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
    fn lost_end_fragment_discards_partial_and_keeps_next_nal() {
        let au = au_from_nals(&[nal(5, 2000)]);
        let mut payloads = Vec::new();
        packetize_au(&au, &mut payloads);
        payloads.pop(); // lose the E fragment
        let mut d = H264Depacketizer::default();
        for (i, p) in payloads.iter().enumerate() {
            assert_eq!(d.push(i as u16, 7000, p, false), None);
        }
        // Next single NAL arrives with the marker: partial FU is dropped, the
        // fresh NAL survives alone.
        let tail = nal(1, 40);
        let got = d
            .push(100, 13000, &tail, true)
            .expect("fresh NAL must flush");
        assert_eq!(got, au_from_nals(&[tail]));
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
        let mut p = Vec::new();
        packetize_au(&au, &mut p);
        assert_eq!(depacketize_all(&p), Some(au));
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
        assert_eq!(
            flushed,
            au_from_nals(&[a1]),
            "the flushed AU is the buffered first frame, not a merge of both"
        );
        // AU2 completes normally on its marker.
        let b2 = nal(5, 30);
        let au2 = d
            .push(3, 2000, &b2, true)
            .expect("AU2 completes on its marker");
        assert_eq!(au2, au_from_nals(&[b1, b2]));
    }

    // A reordered packet from an OLDER timestamp must not flush the current AU as complete: it is
    // discarded, and the in-progress AU keeps accumulating and completes normally.
    #[test]
    fn reordered_older_timestamp_packet_is_discarded_not_flushed() {
        let mut d = H264Depacketizer::default();
        // AU1 @ ts 1000 completes cleanly.
        let a1 = nal(1, 20);
        let want_a1 = au_from_nals(std::slice::from_ref(&a1));
        assert_eq!(d.push(0, 1000, &a1, true), Some(want_a1));
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
            Some(au_from_nals(&[b1, b2])),
            "the in-progress AU survives the reordered packet and completes on its marker"
        );
    }

    #[test]
    fn late_packet_after_marker_cannot_seed_a_stale_au() {
        let mut d = H264Depacketizer::default();
        let completed = nal(5, 20);
        assert_eq!(
            d.push(10, 1000, &completed, true),
            Some(au_from_nals(std::slice::from_ref(&completed)))
        );

        let late = nal(1, 15);
        assert_eq!(d.push(9, 1000, &late, false), None);
        let next = nal(1, 25);
        assert_eq!(
            d.push(11, 2000, &next, true),
            Some(au_from_nals(std::slice::from_ref(&next))),
            "a late packet from the completed timestamp must not leak into the next AU"
        );
    }

    // The rare case where a timestamp boundary AND a marker fire for the same push: both AUs must
    // surface (the second one push later), not be dropped.
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
        assert_eq!(first, au_from_nals(&[a1]));
        // AU2 surfaces on the next push (drained one-per-call). Use a fresh AU3 packet to pump it.
        let c1 = nal(1, 15);
        let second = d
            .push(2, 3000, &c1, false)
            .expect("the second completed AU surfaces on the next push");
        assert_eq!(second, au_from_nals(&[b1]));
    }

    #[test]
    fn empty_au_yields_no_payloads_and_marker_alone_yields_none() {
        let mut payloads = vec![vec![1u8]];
        packetize_au(&[], &mut payloads);
        assert!(payloads.is_empty(), "packetize_au must clear stale output");
        let mut d = H264Depacketizer::default();
        assert_eq!(d.push(0, 0, &[], true), None);
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
}
