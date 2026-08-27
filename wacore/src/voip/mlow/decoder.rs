//! MLow top-level decoder: RED strip -> TOC routing -> active-frame decode (chained 20 ms internal
//! frames: LSF -> pulses -> pitch/gains -> CELP synthesis) -> PCM. The synthesis
//! (`smpl_celpdec`) runs the excitation in the codec's float domain (gen_noise + LPC synthesis). The
//! cross-frame predictor and synthesis history persist across calls because the stream is
//! continuous.

use super::multiframe::{is_multiframe, split_multiframe};
use super::rangecoder::RangeDecoder;
use super::red::depack_split_red;
use super::smpl_cc_tables::load_cc_tables;
use super::smpl_celpdec::CelpDecParams;
use super::smpl_decode::{decode_smpl_lsf, load_smpl_tables};
use super::smpl_gains::decode_smpl_gains;
use super::smpl_mem::load_smpl_mem;
use super::smpl_pitch::decode_smpl_pitch;
use super::smpl_pulse::decode_smpl_pulses;
use super::smpl_synth::{
    SMPL_INTF_LEN, SmplDecodeRollback, SmplDecoderState, load_smpl_synth_tables,
    smpl_reconstruct_nlsf,
};
use super::toc::parse_mlow_toc;

const OPUS_FRAME_SAMPS: usize = 960; // 60 ms @ 16 kHz
/// The decoder synthesises at 16 kHz regardless of the internal rate a TOC declares.
const OUTPUT_SAMPLES_PER_MS: i32 = 16;

/// The longest audio one packet may carry, in samples at 16 kHz.
///
/// 120 ms, which is both the longest duration a TOC can declare and the ceiling the shipped
/// client applies when it totals a multiframe envelope. It is a hard bound and not a hint: without
/// it an envelope of eighteen sub-frames turns a few hundred bytes into tens of thousands of
/// samples, and the jitter buffer on the other side of this function has no way to tell that
/// expansion from real audio.
const MAX_PACKET_SAMPS: usize = 1920;

/// Samples a frame that produced no audio should occupy.
///
/// Its own declared duration, not a fixed 60 ms slot. A fixed slot makes a run of concealed 20 ms
/// frames arrive at three times real time, which floods the jitter buffer and gets speech trimmed
/// off its head to hold the latency ceiling: the concealment evicts the audio that did decode.
fn silence_samps(declared: usize) -> usize {
    declared.clamp(1, MAX_PACKET_SAMPS)
}

/// Internal 20 ms frames chained inside one packet, or `None` for a duration this decoder cannot
/// run. A packet is not a single unit of decode: the reference derives the loop count from the
/// declared duration while the geometry inside each iteration stays fixed, so 20/60/120 ms differ
/// only in how many times the same decode repeats.
///
/// 10 ms is the exception and stays unsupported: it halves the internal frame length and the
/// subframe count, which the synthesis does not implement, and decoding it under the wrong geometry
/// would consume the payload with the wrong symbol count and desync the range coder.
fn internal_frames(frame_ms: i32) -> Option<usize> {
    (frame_ms > 10).then(|| ((frame_ms + 10) / 20) as usize)
}

/// Does a decode that ended after `consumed` bytes of a `storage`-byte body land where a valid
/// stream can end? Under-running is always malformed; the upper slack absorbs the range coder's
/// final carry bytes, which the encoder does not emit.
///
/// The bound is four, taken from the shipped decoder rather than from the C fork: `WhatsAppNative.dll`
/// @ `0x180337e30` does `add r8d, 0x4` before its second compare, while the fork's
/// `smpl_check_end_result` allows only `+2`. Two is the stricter of the pair, so it would conceal
/// frames the official client plays — a false report of corruption, silencing audio that is fine.
fn endpoint_is_valid(storage: u32, consumed: u32) -> bool {
    storage <= consumed && consumed <= storage + 4
}

/// What one call to [`MlowDecoder::decode`] did with the packet it was given.
///
/// The decoder answers with PCM either way, so without this a concealed frame, a frame outside the
/// operating point and a frame of genuine background noise are the same observation: silence. That
/// ambiguity is what let issue #1105 stay open, and it is the whole reason the engine counts.
///
/// Counts rather than flags because one packet can chain several internal frames, and a multiframe
/// envelope can chain several packets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MlowFrameReport {
    /// Frames that produced coded audio.
    pub decoded: u8,
    /// Frames concealed: an empty payload, a RED envelope that would not unwrap, or a body whose
    /// decode did not end where the body said it should.
    pub concealed: u8,
    /// Frames refused by the operating-point guard, including a standard-Opus escape this decoder
    /// does not run.
    pub off_point: u8,
    /// Frames that carried no coded voice (SID / comfort noise).
    pub inactive_or_sid: u8,
}

/// Stateful pure-Rust MLow decoder. Decodes one RTP payload (a bare MLow frame, or a SplitRed
/// packet when redundancy was negotiated) into a PCM frame at 16 kHz, one 20 ms internal frame
/// per chained frame in the packet.
pub struct MlowDecoder {
    state: SmplDecoderState,
    redundancy: i32,
    /// Sticky: set whenever the inner range decoder raised its error flag during any decode. That flag
    /// reflects a degenerate decode table, not arbitrary frame corruption (over-reads return zero
    /// silently), so it does not detect a tampered payload. Read via `had_error`. Diagnostic only,
    /// never gates output.
    had_error: bool,
    /// Count of inbound frames dropped because they fall outside this decoder's single operating point
    /// (16kHz wideband, low_rate=0, and a duration whose internal geometry it implements). Such a
    /// frame would desync the range coder if decoded, so it is dropped (treated as a lost frame). The
    /// count drives a once + every-100th `warn` naming the offending dimension.
    dropped_unsupported: u32,
    /// Frames concealed because the decode did not end where the body said it should. Drives a
    /// once + every-100th `warn`, so a peer sending a stream this decoder cannot read is visible in
    /// a log rather than silently quiet.
    malformed: u32,
    /// Snapshot of the part of `state` an active decode advances, so a malformed body can be undone.
    ///
    /// Owned by the decoder and reused: see the `save` in `decode_active_frame` for why this is a
    /// field rather than a local, and [`SmplDecodeRollback`] for what it leaves out.
    state_backup: SmplDecodeRollback,
    /// What the in-flight `decode` call has done so far. Reset at the top of every `decode` and
    /// drained by the caller through [`MlowDecoder::take_frame_report`].
    report: MlowFrameReport,
    /// Samples the last packet DECLARED, from its TOC, which is not always what `decode` returned:
    /// a SID, a drop or a standard-Opus escape emits a fixed slot regardless of duration. Consumers
    /// sizing a jitter cushion need the declared value, since that is what sets arrival cadence.
    last_packet_samps: usize,
}

impl Default for MlowDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MlowDecoder {
    pub fn new() -> Self {
        MlowDecoder {
            state: SmplDecoderState::default(),
            state_backup: SmplDecodeRollback::default(),
            redundancy: 0,
            had_error: false,
            dropped_unsupported: 0,
            malformed: 0,
            report: MlowFrameReport::default(),
            last_packet_samps: OPUS_FRAME_SAMPS,
        }
    }

    /// Whether any decode since construction (or `reset`) raised the range decoder's error flag (a
    /// degenerate decode table). It does not flag a corrupted payload, which the decoder absorbs.
    /// Diagnostic only; consumed by the regression suites, so it is gated to test builds.
    #[cfg(test)]
    pub(crate) fn had_error(&self) -> bool {
        self.had_error
    }

    /// Samples in the most recent packet as its TOC declared them, independent of what `decode`
    /// emitted for it. Starts at a 60 ms packet.
    pub fn last_packet_samps(&self) -> usize {
        self.last_packet_samps
    }

    /// Take what the last [`MlowDecoder::decode`] did, clearing it.
    ///
    /// Cleared on read so a caller that forgets to drain cannot silently accumulate one packet's
    /// verdict into the next one's.
    pub fn take_frame_report(&mut self) -> MlowFrameReport {
        core::mem::take(&mut self.report)
    }

    /// Set the negotiated RED redundancy level (0 = bare frames, the common case).
    pub fn set_redundancy(&mut self, n: i32) {
        self.redundancy = n;
    }

    /// Clear the cross-frame state (call at a stream discontinuity).
    pub fn reset(&mut self) {
        self.state = SmplDecoderState::default();
        self.state_backup = SmplDecodeRollback::default();
        self.had_error = false;
    }

    /// Decode one RTP MLow payload into a PCM frame, float in [-1, 1]. The sample count follows
    /// the packet's declared duration; a dropped or silenced frame yields a 60 ms slot.
    pub fn decode(&mut self, payload: &[u8]) -> Vec<f32> {
        self.report = MlowFrameReport::default();
        if payload.is_empty() {
            self.report.concealed = self.report.concealed.saturating_add(1);
            return vec![0.0; OPUS_FRAME_SAMPS];
        }
        if self.redundancy > 0 {
            return match depack_split_red(payload) {
                // the main (current) frame is last; its slice borrows `payload`, not `self`, so it
                // can drive the decode directly (no copy).
                //
                // Through `decode_inner`, not `decode_frame`: a redundancy wrapper is chosen by the
                // payload type, and nothing stops the frame inside it from being a multiframe
                // envelope. Going straight to `decode_frame` would read that envelope's indicator
                // as a SID and answer comfort noise -- the same total silence this triage exists to
                // prevent, just reached through the RED payload type instead of the bare one.
                Ok(frames) => match frames.last() {
                    Some(main) => self.decode_inner(main.data),
                    None => self.decode_frame(&[]),
                },
                Err(e) => {
                    log::warn!("mlow RED depacketization failed: {e:?}");
                    self.report.concealed = self.report.concealed.saturating_add(1);
                    vec![0.0; OPUS_FRAME_SAMPS]
                }
            };
        }
        self.decode_inner(payload)
    }

    /// Route one bare MLow payload: multiframe envelope, or a single frame.
    ///
    /// The envelope is checked before the TOC is read, because under the TOC grammar its first byte
    /// reads as a SID (bit 7 set). A decoder that skips this check answers every packet of such a
    /// call with comfort noise and reports nothing: total silence with no symptom at all.
    fn decode_inner(&mut self, payload: &[u8]) -> Vec<f32> {
        if is_multiframe(payload) {
            return self.decode_multiframe(payload);
        }
        self.decode_frame(payload)
    }

    /// Decode every sub-frame of an envelope, in transmission order, into one contiguous packet.
    ///
    /// Order matters and is not obvious: the envelope's RTP timestamp belongs to the LAST sub-frame
    /// and the earlier ones run backwards from it, so array order is time order and concatenating
    /// is what puts the audio back where it belongs.
    fn decode_multiframe(&mut self, payload: &[u8]) -> Vec<f32> {
        let frames = match split_multiframe(payload) {
            Ok(frames) => frames,
            Err(e) => {
                self.malformed += 1;
                if self.malformed == 1 || self.malformed.is_multiple_of(100) {
                    log::warn!(
                        "mlow: failed to parse multiframe packet #{} (indicator 0x{:02x}): {e}",
                        self.malformed,
                        payload[0]
                    );
                }
                self.report.concealed = self.report.concealed.saturating_add(1);
                self.last_packet_samps = OPUS_FRAME_SAMPS;
                return vec![0.0; OPUS_FRAME_SAMPS];
            }
        };
        // Refuse before decoding anything, not while appending: a packet whose sub-frames total
        // more than the format allows is malformed as a whole, and half-decoding it would leave the
        // predictor advanced by frames the peer never meant as one packet. The shipped client
        // applies the same 120 ms ceiling when it totals an envelope.
        let mut declared = 0usize;
        for frame in &frames {
            let ms = frame.first().map_or(0, |&b| parse_mlow_toc(b).frame_ms);
            declared += silence_samps((16 * ms.max(0)) as usize);
        }
        if declared == 0 || declared > MAX_PACKET_SAMPS {
            self.malformed += 1;
            if self.malformed == 1 || self.malformed.is_multiple_of(100) {
                log::warn!(
                    "mlow: multiframe packet #{} declares {declared} samples, past the {MAX_PACKET_SAMPS} ceiling",
                    self.malformed
                );
            }
            self.report.concealed = self.report.concealed.saturating_add(1);
            self.last_packet_samps = OPUS_FRAME_SAMPS;
            return vec![0.0; OPUS_FRAME_SAMPS];
        }

        let mut out = Vec::with_capacity(declared);
        for frame in frames {
            out.extend_from_slice(&self.decode_frame(frame));
        }
        // The playout cushion sizes to the whole envelope, not to one sub-frame: the packet is what
        // arrives on the wire, and its cadence is what the buffer has to absorb.
        self.last_packet_samps = declared;
        out
    }

    fn decode_frame(&mut self, frame: &[u8]) -> Vec<f32> {
        if frame.is_empty() {
            self.report.concealed = self.report.concealed.saturating_add(1);
            return vec![0.0; OPUS_FRAME_SAMPS];
        }
        let toc = parse_mlow_toc(frame[0]);
        // In OUTPUT samples, always 16 kHz: the synthesis runs at one rate, so a frame declaring a
        // 32 kHz internal rate still occupies its duration's worth of 16 kHz samples. Sizing this
        // from the declared rate would double the cushion for a frame the decoder then drops.
        if toc.frame_ms > 0 {
            self.last_packet_samps = (OUTPUT_SAMPLES_PER_MS * toc.frame_ms) as usize;
        }
        if toc.std_opus {
            let out_len = silence_samps((OUTPUT_SAMPLES_PER_MS * toc.frame_ms) as usize);
            log::debug!(
                "mlow: standard-opus TOC 0x{:02x} -> {out_len} samples silence",
                frame[0]
            );
            self.report.off_point = self.report.off_point.saturating_add(1);
            return vec![0.0; out_len];
        }
        // A SID (DTX/CNG) frame carries comfort noise rather than coded voice and is silenced without
        // opening the range coder, so its geometry can never desync. Handle it before the
        // operating-point guard so an off-point SID is the benign silence it is rather than tripping
        // the "dropped" canary. A full 60ms slot keeps the playout cadence regardless of the frame's
        // nominal duration.
        //
        // A frame that is merely coded inactive is NOT silence: with DTX off the encoder keeps sending
        // background noise this way, and the reference decodes it. It goes through the normal path.
        if toc.sid {
            let samps = silence_samps(self.last_packet_samps);
            log::debug!(
                "mlow: SID TOC 0x{:02x} -> {samps} samples of silence",
                frame[0]
            );
            self.report.inactive_or_sid = self.report.inactive_or_sid.saturating_add(1);
            return vec![0.0; samps];
        }
        // Operating-point guard for active frames: a different internal rate, the low_rate=1 2x160
        // geometry, or a duration whose internal geometry differs would consume the payload with the
        // wrong symbol count and desync the range coder (garbage plus a poisoned cross-frame predictor
        // that propagates to later packets). Drop those as lost frames so the predictor holds its last
        // good values. flag2 is the smpl TOC's low_rate bit.
        let frames = internal_frames(toc.frame_ms);
        let off_point = if toc.sample_rate != 16000 {
            Some(("rate", i64::from(toc.sample_rate / 1000)))
        } else if toc.low_rate {
            Some(("low_rate", 1))
        } else if frames.is_none() {
            Some(("frame_ms", i64::from(toc.frame_ms)))
        } else {
            None
        };
        if let Some((dim, val)) = off_point {
            self.dropped_unsupported += 1;
            if self.dropped_unsupported == 1 || self.dropped_unsupported.is_multiple_of(100) {
                log::warn!(
                    "mlow: dropping out-of-operating-point frame #{} ({dim}={val}, TOC 0x{:02x}); \
                     the decoder runs 16kHz / low_rate=0 / 20-120ms",
                    self.dropped_unsupported,
                    frame[0]
                );
            }
            self.report.off_point = self.report.off_point.saturating_add(1);
            return vec![0.0; silence_samps(self.last_packet_samps)];
        }
        let frames = frames.expect("the guard above rejected every unsupported duration");
        self.decode_active_frame(frame, frames * SMPL_INTF_LEN, frames, toc.active)
    }

    /// `coded_as_active_voice` gates two symbols that a frame coded inactive never puts on the wire;
    /// reading them would consume symbols that were never written and desync everything after.
    fn decode_active_frame(
        &mut self,
        frame: &[u8],
        out_len: usize,
        frames: usize,
        coded_as_active_voice: bool,
    ) -> Vec<f32> {
        let config = (frame[0] >> 2) as usize & 1;
        let tbl = load_smpl_tables();
        let synth_t = load_smpl_synth_tables();
        let mem = load_smpl_mem();
        let cc = load_cc_tables();
        let mut dec = RangeDecoder::new(&frame[1..]);

        // The low_rate bit of the smpl TOC (this capture is low_rate==0; the synth gates on it).
        let low_rate = (frame[0] >> 2) & 1 != 0;

        // The overrun that invalidates a body is only detectable after the last internal frame, by
        // which point the loop has already advanced the LSF predictor, the CELP history and
        // `prev_nlsf`. Keep a copy so concealment can undo them: parameters invented past the end of
        // a bad body must not seed the next packet. The reference leaves them advanced, but it never
        // meets a stream it cannot read; this decoder does, and the leak is audible in the frame
        // after.
        //
        // `clone_from` into a buffer the decoder owns, not a fresh `clone()`. Every field here is a
        // `Vec` of fixed length, so cloning into an existing snapshot reuses its allocations and the
        // copy costs a `memcpy`; a fresh clone allocates four buffers on every packet, which at
        // ~17 packets a second is pure churn on a heap that an ESP32 target has to keep unfragmented.
        //
        // And it copies only what this loop can reach: the harmonic postfilter runs after the
        // endpoint check below, so a concealed packet never advanced it. See [`SmplDecodeRollback`].
        self.state_backup.save(&self.state);

        let mut out: Vec<f32> = Vec::with_capacity(frames * SMPL_INTF_LEN);
        // Collect the per-40-block lags (8 per internal frame) and the average normalized bitrate
        // for the per-packet harmonic postfilter.
        let mut packet_lags: Vec<f32> = Vec::with_capacity(frames * 8);
        let mut avg_norm_br = 0.0f32;
        for f in 0..frames {
            let lsf = decode_smpl_lsf(
                &mut dec,
                tbl,
                &mut self.state.lstate,
                config,
                f,
                coded_as_active_voice,
            );
            let pulses = decode_smpl_pulses(
                &mut dec,
                cc,
                SMPL_INTF_LEN as i32,
                4,
                i32::from(coded_as_active_voice),
                config as i32,
                lsf.stage1,
            );
            let voiced = lsf.stage1 == 1;
            let mut params = CelpDecParams {
                voiced,
                sf_pulses: pulses.subfr,
                fcbg_idx: [0; 4],
                nrgres_dbq_q14: [0; 4],
                acbg_idx: [0; 4],
                block_lags: [0.0; 8],
                total_pulses: pulses.subfr.iter().sum(),
            };
            if voiced {
                let pr = decode_smpl_pitch(
                    &mut dec,
                    mem,
                    cc,
                    &mut self.state.lstate,
                    SMPL_INTF_LEN as i32,
                    4,
                    config as i32,
                    pulses.subfr,
                );
                // lag = laginds*0.5 + SMPL_MIN_PITCH_LAG, clamped; one per 40-block, 8 per frame.
                for b in 0..8 {
                    params.block_lags[b] =
                        ((pr.block_lags[b] as f64 * 0.5 + 32.0).min(320.0)) as f32;
                }
                for sf in 0..4 {
                    params.acbg_idx[sf] = pr.gain_idx[sf];
                    // The voiced FCB gain index is decoded in the pitch block (filt_idx).
                    params.fcbg_idx[sf] = pr.filt_idx[sf].max(0);
                }
            } else {
                let g = decode_smpl_gains(&mut dec, cc, 4, pulses.subfr);
                // The unvoiced gains decode yields gain_q (the nrgres_dbq_Q14 field) and nrg_res (the
                // fcbg_idx field).
                params.nrgres_dbq_q14 = g.gain_q;
                params.fcbg_idx = g.nrg_res;
            }
            packet_lags.extend_from_slice(&params.block_lags);
            avg_norm_br += super::smpl_gennoise::smpl_get_normalized_bitrate(
                params.total_pulses,
                SMPL_INTF_LEN as i32,
            );

            let nlsf = smpl_reconstruct_nlsf(
                synth_t,
                lsf.stage1 as usize,
                config,
                lsf.grid as usize,
                &lsf.stage2,
                &self.state.prev_nlsf,
            );
            let mut sig = [0f32; SMPL_INTF_LEN];
            self.state.celp.synth_frame(
                &nlsf,
                lsf.extra as usize,
                &pulses.pulses,
                &params,
                low_rate,
                SMPL_INTF_LEN as i32,
                &mut sig,
            );
            self.state.prev_nlsf = nlsf;
            out.extend_from_slice(&sig);
        }

        // Endpoint check, before the postfilter as in the reference: the range decoder returns zero
        // past either end of its storage WITHOUT flagging it, so an impossible stream decodes into
        // plausible-looking symbols and the synthesis can diverge to full scale. Comparing where the
        // decode ended against what the body actually held is the only way to see it. Anything
        // outside the accepted window is a malformed frame, concealed as a lost one rather than
        // synthesized, and the state the loop advanced is rolled back below -- deliberately unlike
        // the reference, for the reason given where the snapshot is taken.
        let consumed_bytes = (dec.tell().max(0) as u32).div_ceil(8);
        let body = dec.storage();
        if !endpoint_is_valid(body, consumed_bytes) || dec.err != 0 {
            // Sticky flag first: this branch swallows the range-decoder failure it is reporting, and
            // `had_error` is what the suites read to see it.
            if dec.err != 0 {
                self.had_error = true;
            }
            // Copied back rather than swapped: the snapshot is narrower than the state, so it has
            // no `harm` to trade for the live one. It keeps its own buffers either way, which is
            // what the swap was protecting.
            self.state_backup.restore(&mut self.state);
            self.malformed += 1;
            if self.malformed == 1 || self.malformed.is_multiple_of(100) {
                log::warn!(
                    "mlow: concealing malformed frame #{} (TOC 0x{:02x}: decode ended at {} bytes \
                     of a {}-byte body)",
                    self.malformed,
                    frame[0],
                    consumed_bytes,
                    body
                );
            }
            self.report.concealed = self.report.concealed.saturating_add(1);
            return vec![0.0; out_len];
        }
        self.report.decoded = self.report.decoded.saturating_add(frames as u8);

        // Per-packet harmonic postfilter (the codec's final pitch comb + 48-sample group delay), run
        // once over the whole packet with the 24 per-40-block lags and the average normalized bitrate.
        let plen = out.len();
        super::smpl_harm_postfilter::smpl_harm_postfilter(
            &mut self.state.harm,
            &mut out,
            plen,
            &packet_lags,
            packet_lags.len(),
            avg_norm_br / frames as f32,
        );

        // The C-domain synthesis output is already float in [-1, 1]; clamp in place.
        for v in &mut out {
            *v = v.clamp(-1.0, 1.0);
        }
        if out_len > 0 && out_len != out.len() {
            out.resize(out_len, 0.0);
        }
        if dec.err != 0 {
            // Sticky flag for `had_error`; does not alter `out` (the frame still plays).
            self.had_error = true;
            log::warn!("mlow: range decoder raised its error flag after active-frame decode");
        }
        log::debug!(
            "mlow: active frame decoded -> {} samples (config={config})",
            out.len()
        );
        out
    }
}

/// Per-subframe param snapshot for the param-decode-match (T1) test.
#[cfg(test)]
pub(crate) struct DiagParam {
    pub(crate) packet: usize,
    pub(crate) frame: usize,
    pub(crate) sf: usize,
    pub(crate) voiced: bool,
    /// The `gain_q` value, i.e. the `nrgres_dbq_Q14` field.
    pub(crate) nrgres_dbq_q14: i32,
    /// The per-subframe `nrg_res` / voiced `filt_idx` symbol, i.e. the `fcbg_idx` field.
    pub(crate) fcbg_idx: i32,
}

/// Re-run the active-frame decode over the capture and capture per-subframe unvoiced params, keyed
/// by (packet, frame, sf), to compare against the reference dump (see testdata/PROVENANCE.md).
#[cfg(test)]
pub(crate) fn diag_decode_params() -> Vec<DiagParam> {
    let frames: Vec<String> =
        serde_json::from_str(include_str!("testdata/inbound_capture_frames.json")).unwrap();
    let tbl = load_smpl_tables();
    let mem = load_smpl_mem();
    let cc = load_cc_tables();
    let mut lstate = super::smpl_decode::SmplLsfState::default();
    let mut out = Vec::new();
    for (packet, hex_frame) in frames.iter().enumerate() {
        let frame = hex::decode(hex_frame).unwrap();
        if frame.is_empty() {
            continue;
        }
        let toc = parse_mlow_toc(frame[0]);
        if toc.std_opus || toc.sid || !toc.active {
            continue;
        }
        let config = (frame[0] >> 2) as usize & 1;
        let mut dec = RangeDecoder::new(&frame[1..]);
        for f in 0..3 {
            let lsf = decode_smpl_lsf(&mut dec, tbl, &mut lstate, config, f, true);
            let pulses = decode_smpl_pulses(
                &mut dec,
                cc,
                SMPL_INTF_LEN as i32,
                4,
                1,
                config as i32,
                lsf.stage1,
            );
            if lsf.stage1 == 1 {
                let pr = decode_smpl_pitch(
                    &mut dec,
                    mem,
                    cc,
                    &mut lstate,
                    SMPL_INTF_LEN as i32,
                    4,
                    config as i32,
                    pulses.subfr,
                );
                for sf in 0..4 {
                    out.push(DiagParam {
                        packet,
                        frame: f,
                        sf,
                        voiced: true,
                        nrgres_dbq_q14: pr.gain_idx[sf],
                        fcbg_idx: pr.filt_idx[sf],
                    });
                }
            } else {
                let g = decode_smpl_gains(&mut dec, cc, 4, pulses.subfr);
                for sf in 0..4 {
                    out.push(DiagParam {
                        packet,
                        frame: f,
                        sf,
                        voiced: false,
                        nrgres_dbq_q14: g.gain_q[sf],
                        fcbg_idx: g.nrg_res[sf],
                    });
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The loop count per packet duration, against the reference decoder's
    /// `num_frames = (packet_len_ms + 10) / 20`. 10 ms is excluded because it also changes the
    /// internal frame length and subframe count, which the synthesis does not implement.
    #[test]
    fn internal_frame_count_matches_the_reference_geometry() {
        assert_eq!(internal_frames(20), Some(1));
        assert_eq!(internal_frames(60), Some(3));
        assert_eq!(internal_frames(120), Some(6));
        assert_eq!(internal_frames(10), None);
    }

    /// A 120 ms MLow packet decodes to its full duration.
    ///
    /// Note what this does NOT claim. TOC `0x58` was originally taken as evidence that WhatsApp
    /// Desktop sends 120 ms MLow packets; it is not. The packets that prompted that reading are
    /// standard Opus SILK wideband at 60 ms, where the same byte is an RFC 6716 config-11 TOC (see
    /// `voip::opus_packet`). The reference geometry `num_frames = (ms + 10) / 20` is nonetheless
    /// real, verified against the shipped decoder, so a single-block 120 ms packet is legal and is
    /// decoded here. The shipped encoder does not emit one - it aggregates 60 ms blocks through the
    /// multiframe envelope instead - so this is correctness for a form that exists in the format
    /// rather than support for an observed stream.
    #[test]
    fn multi_frame_packet_decodes_to_its_full_duration() {
        let toc = parse_mlow_toc(0x58);
        assert_eq!(toc.frame_ms, 120, "0x58 declares a 120 ms packet");
        assert!(toc.active && !toc.sid && !toc.std_opus);
        assert_eq!(toc.sample_rate, 16000);
        assert!(!toc.low_rate, "0x58 is the supported rate mode");

        let mut dec = MlowDecoder::new();
        let out = dec.decode(&[0x58, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22]);
        assert_eq!(
            out.len(),
            6 * SMPL_INTF_LEN,
            "a 120 ms packet must yield 120 ms of PCM"
        );
    }

    /// A 20 ms packet shares the same internal geometry and must decode to exactly one frame.
    #[test]
    fn single_frame_packet_decodes_to_one_internal_frame() {
        let mut dec = MlowDecoder::new();
        let out = dec.decode(&[0x48, 0xAA, 0xBB, 0xCC]);
        assert_eq!(
            out.len(),
            SMPL_INTF_LEN,
            "a 20 ms packet is one 20 ms frame"
        );
    }

    /// The failure case the geometry guard exists for: 10 ms halves the internal frame length and
    /// the subframe count, so it must still be dropped rather than decoded under the wrong geometry,
    /// into the same 60 ms silence slot the other drops use.
    #[test]
    fn ten_ms_active_packet_is_still_dropped() {
        let toc = parse_mlow_toc(0x40);
        assert_eq!(toc.frame_ms, 10);
        assert!(toc.active);

        let mut dec = MlowDecoder::new();
        let out = dec.decode(&[0x40, 0xAA, 0xBB, 0xCC]);
        // Dropped, but still occupying the 10 ms it declares: a run of these must not arrive at six
        // times real time and push decoded speech out of the jitter buffer.
        assert_eq!(out.len(), 160);
        assert!(
            out.iter().all(|&s| s == 0.0),
            "10 ms runs a geometry the synthesis does not implement"
        );
        assert!(!dec.had_error(), "the drop must not open the range decoder");
    }

    /// Silence occupies the time the packet claims, and the cushion follows the declared duration.
    ///
    /// A frame that produces no audio still has to occupy its slot: emitting a fixed 60 ms for a
    /// packet the peer sends every 120 ms starves playout, and emitting it for one sent every 20 ms
    /// floods the buffer at three times real time until the latency ceiling trims speech off the
    /// head. Either way the concealment displaces audio that did decode.
    #[test]
    fn declared_duration_survives_a_dtx_transition() {
        let mut dec = MlowDecoder::new();
        let _ = dec.decode(&[0x58, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22]);
        assert_eq!(dec.last_packet_samps(), 6 * SMPL_INTF_LEN);

        // A SID that still declares 120 ms (0x98 = SID | 120 ms) fills 120 ms of silence.
        let sid = dec.decode(&[0x98, 0xAA, 0xBB, 0xCC]);
        assert_eq!(
            sid.len(),
            6 * SMPL_INTF_LEN,
            "silence occupies the duration the packet declares"
        );
        assert_eq!(
            dec.last_packet_samps(),
            6 * SMPL_INTF_LEN,
            "the peer is still on 120 ms packets; the cushion must not shrink"
        );

        // A peer that genuinely moves to 60 ms is learned.
        let _ = dec.decode(&[0x50, 0xAA, 0xBB, 0xCC]);
        assert_eq!(dec.last_packet_samps(), 3 * SMPL_INTF_LEN);
    }

    /// The way the shipped client actually reaches a packet longer than its frame length.
    ///
    /// Its encoder caches a fixed three blocks of 20 ms, so `frame_length_ms = 120` produces two
    /// 60 ms blocks in one envelope, not one 120 ms block. Before this was implemented the
    /// envelope's first byte read as a SID and every packet of such a call became comfort noise:
    /// a totally silent call with no error anywhere.
    #[test]
    fn a_multiframe_envelope_decodes_instead_of_becoming_comfort_noise() {
        let mut encoder = super::super::encode::MlowEncoder::new();
        let tone: Vec<f32> = (0..OPUS_FRAME_SAMPS)
            .map(|i| 0.3 * (i as f32 * 0.05).sin())
            .collect();
        let first = encoder.encode(&tone).expect("mlow encode");
        let second = encoder.encode(&tone).expect("mlow encode");
        assert!(first.len() < 252 && second.len() < 252);

        let mut envelope = vec![0x82 | (first[0] & 0x39), 0x02, first.len() as u8];
        envelope.extend_from_slice(&first);
        envelope.extend_from_slice(&second);

        // The bug this replaces: read as a TOC, the indicator has bit 7 set and is a SID.
        assert!(parse_mlow_toc(envelope[0]).sid);

        let mut decoder = MlowDecoder::new();
        let pcm = decoder.decode(&envelope);
        let report = decoder.take_frame_report();
        assert_eq!(
            report.decoded, 6,
            "two 60 ms blocks are six internal frames"
        );
        assert_eq!(report.inactive_or_sid, 0, "the envelope is not a SID");
        assert_eq!(pcm.len(), 2 * OPUS_FRAME_SAMPS);
        assert_eq!(
            decoder.last_packet_samps(),
            2 * OPUS_FRAME_SAMPS,
            "the playout cushion must size to the whole envelope, not one sub-frame"
        );
        let energy: f32 = pcm.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "a decoded envelope must not be silence");
    }

    /// A malformed envelope is concealed and counted, never mistaken for comfort noise.
    #[test]
    fn a_malformed_multiframe_envelope_is_concealed_and_reported() {
        let mut decoder = MlowDecoder::new();
        // Two frames declared, the first taking the entire body.
        let mut envelope = vec![0x92u8, 0x02, 10];
        envelope.extend(0..10u8);
        let pcm = decoder.decode(&envelope);
        let report = decoder.take_frame_report();
        assert_eq!(report.concealed, 1);
        assert_eq!(report.inactive_or_sid, 0);
        assert!(pcm.iter().all(|&s| s == 0.0));
    }

    /// A `VoA=00` packet is a normal frame carrying background noise, not a SID: the reference
    /// decodes it, and with DTX off a peer sends nothing else during a pause. Silencing it drops
    /// ~12% of a real stream on the floor while the call merely sounds quiet.
    #[test]
    fn dtx_off_frames_decode_to_audio() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/mlow_dtx_off_frames.json"))
                .expect("mlow_dtx_off_frames.json");
        let refp: Vec<f32> = include_bytes!("testdata/ref_dtx_off_expected.raw")
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();

        let mut dec = MlowDecoder::new();
        let mut out: Vec<f32> = Vec::new();
        let mut spans = Vec::new();
        for hex_frame in &frames {
            let frame = hex::decode(hex_frame).unwrap();
            let start = out.len();
            out.extend_from_slice(&dec.decode(&frame));
            // The frames this covers: not SID, VoA=0, hang-over clear, i.e. coded_as_active_voice
            // == 0. `0x12` shares VoA=0 but sets the hang-over bit, which makes it active and
            // already decoded, so it must not be counted here.
            if frame[0] & 0xC2 == 0 {
                spans.push((start, out.len()));
            }
        }
        assert!(
            !spans.is_empty(),
            "fixture lost its DTX-off frames: this path is no longer covered"
        );
        assert_eq!(out.len(), refp.len());

        let energy = |v: &[f32]| v.iter().map(|&x| (x as f64).powi(2)).sum::<f64>();
        let (mut e_ref, mut e_ours, mut n) = (0.0, 0.0, 0usize);
        for (s, e) in &spans {
            e_ref += energy(&refp[*s..*e]);
            e_ours += energy(&out[*s..*e]);
            n += e - s;
        }
        let (rms_ref, rms_ours) = ((e_ref / n as f64).sqrt(), (e_ours / n as f64).sqrt());
        assert!(
            rms_ref > 0.01,
            "the reference itself has no audio here; the fixture is wrong"
        );
        assert!(
            rms_ours > rms_ref * 0.5,
            "DTX-off frames decoded to {rms_ours:.5} rms against the reference's {rms_ref:.5}"
        );
    }

    /// A stream that ends where it claims to must be accepted. This is the guard against the
    /// endpoint check being too strict: the synthetic 120 ms vector is a well-formed six-frame
    /// packet, and rejecting it would silence audio that decodes correctly.
    #[test]
    fn endpoint_check_accepts_a_well_formed_packet() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/mlow_120ms_frames.json")).unwrap();
        let mut dec = MlowDecoder::new();
        for hex_frame in &frames {
            let out = dec.decode(&hex::decode(hex_frame).unwrap());
            assert!(
                out.iter().any(|&s| s != 0.0),
                "a valid packet must not be rejected as malformed"
            );
        }
    }

    /// The accepted window is the shipped decoder's, not the C fork's. The fork stops at `+2`, so a
    /// stream ending three or four bytes long would be concealed here and played by the official
    /// client. Pinned as arithmetic because no synthetic body lands on `+3` on demand.
    #[test]
    fn endpoint_window_matches_the_shipped_decoder() {
        assert!(endpoint_is_valid(40, 40), "exact end is valid");
        assert!(endpoint_is_valid(40, 42), "the fork's +2 stays valid");
        assert!(
            endpoint_is_valid(40, 44),
            "+4 is valid: WhatsAppNative.dll @ 0x180337e30 adds 4 before its second compare, and \
             rejecting here would silence audio the official client plays"
        );
        assert!(!endpoint_is_valid(40, 45), "+5 over-runs");
        assert!(
            !endpoint_is_valid(40, 39),
            "under-running is always malformed"
        );
    }

    /// A body whose decode consumes far more bits than it holds is malformed: the range decoder
    /// returns zero past the end without flagging it, so the synthesis runs on invented symbols and
    /// can diverge to full scale. Conceal it as a lost frame rather than emitting that.
    #[test]
    fn endpoint_check_rejects_an_overrunning_body() {
        // A 120 ms TOC with a body far too short for six internal frames.
        let mut dec = MlowDecoder::new();
        let out = dec.decode(&[0x58, 0x03, 0x1a, 0xfb, 0x0a]);
        assert_eq!(out.len(), 6 * SMPL_INTF_LEN, "still a full 120 ms slot");
        assert!(
            out.iter().all(|&s| s == 0.0),
            "an over-running body must be concealed, not synthesized"
        );
    }

    /// Real 120 ms packets captured from a live WhatsApp Desktop peer must decode to speech, not to
    /// full-scale noise. Reported on #1105 after the multi-frame admission landed: frames are now
    /// accepted, but the decode diverges partway through the packet and saturates, which is audibly
    /// worse than the silence it replaced.
    ///
    /// The assertions are deliberately coarse — this pins "the output is not garbage", which is what
    /// regressed, without pretending to a bit-exact target the fixture cannot supply.
    ///
    /// Concealing a malformed frame must leave no trace: the decode loop mutates the LSF predictor,
    /// the CELP history and `prev_nlsf` before the overrun is detectable, so without a rollback the
    /// NEXT packet is synthesized partly from parameters invented past the end of the bad body.
    /// The reference does not roll back, but it also never meets these packets; we do, repeatedly.
    #[test]
    fn a_concealed_frame_does_not_contaminate_the_next() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json")).unwrap();
        let real = hex::decode(&frames[0]).unwrap();

        let mut fresh = MlowDecoder::new();
        let want = fresh.decode(&real);

        let mut contaminated = MlowDecoder::new();
        let bad = contaminated.decode(&[0x58, 0x03, 0x1a, 0xfb, 0x0a]);
        assert!(bad.iter().all(|&s| s == 0.0), "the bad frame is concealed");
        let got = contaminated.decode(&real);

        assert_eq!(got.len(), want.len());
        assert!(
            got == want,
            "a real frame after a concealed one must decode identically to one decoded on a fresh \
             decoder; state from the malformed body leaked into it"
        );
    }

    /// The two halves of a cross-check vector come out of one harness run and mean nothing apart:
    /// refreshing only one leaves the comparison reading mismatched data, which surfaces as a
    /// correlation number that moved rather than as an obvious error. Pin what ties them, and pin
    /// that the fixture still exercises the multi-frame path it was added for.
    #[test]
    fn multi_frame_fixture_halves_stay_in_step() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/mlow_120ms_frames.json"))
                .expect("mlow_120ms_frames.json");
        let pcm_bytes = include_bytes!("testdata/ref_120ms_expected.raw").len();

        // An absolute count, not just "non-empty": a regeneration that ends early produces a
        // shorter vector whose PCM length still matches its own frame count, so a relative check
        // would accept it and the coverage loss would be invisible.
        assert_eq!(
            frames.len(),
            8,
            "fixture no longer holds the 8 packets it was generated with; a short regeneration              silently reduces coverage"
        );
        for (i, f) in frames.iter().enumerate() {
            let toc = hex::decode(f).expect("hex frame")[0];
            assert_eq!(
                toc, 0x58,
                "frame {i} is TOC {toc:#04x}, not the 120 ms packet this fixture exists to cover"
            );
        }
        assert_eq!(
            pcm_bytes,
            frames.len() * 6 * SMPL_INTF_LEN * 2,
            "reference PCM does not match the frame count; regenerate both halves together with \
             scripts/regenerate-mlow-vectors.sh"
        );
    }

    /// The content check: decode a stream of real 120 ms packets and compare against the reference
    /// decoder's own output for the same bytes. Geometry alone is not enough, since running the loop
    /// the wrong number of times would still produce plausibly-shaped audio while consuming the
    /// payload at the wrong symbol count. See testdata/PROVENANCE.md for the oracle.
    #[test]
    fn multi_frame_decode_matches_the_reference() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/mlow_120ms_frames.json"))
                .expect("mlow_120ms_frames.json");
        let refp: Vec<f32> = include_bytes!("testdata/ref_120ms_expected.raw")
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();

        let mut dec = MlowDecoder::new();
        let mut out: Vec<f32> = Vec::new();
        for hex_frame in &frames {
            let frame = hex::decode(hex_frame).unwrap();
            assert_eq!(frame[0], 0x58, "the fixture must stay 120 ms packets");
            out.extend_from_slice(&dec.decode(&frame));
        }
        assert_eq!(out.len(), refp.len(), "decode length vs reference");

        let n = refp.len();
        let (mr, mo) = (
            refp.iter().map(|&v| v as f64).sum::<f64>() / n as f64,
            out.iter().map(|&v| v as f64).sum::<f64>() / n as f64,
        );
        let (mut sxy, mut sxx, mut syy) = (0f64, 0f64, 0f64);
        for i in 0..n {
            let (dr, dz) = (refp[i] as f64 - mr, out[i] as f64 - mo);
            sxy += dr * dz;
            sxx += dr * dr;
            syy += dz * dz;
        }
        let corr = sxy / (sxx * syy).sqrt();
        assert!(corr > 0.999, "lag-0 corr {corr:.6} vs reference");
    }

    /// Decoding a multi-frame packet must leave the cross-frame predictor usable: a real 60 ms frame
    /// after it still has to produce audio.
    #[test]
    fn multi_frame_packet_does_not_poison_later_frames() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json")).unwrap();
        let real = hex::decode(&frames[0]).unwrap();

        let mut dec = MlowDecoder::new();
        let _ = dec.decode(&[0x58, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x11, 0x22]);
        let after = dec.decode(&real);
        assert_eq!(after.len(), 960);
        assert!(
            after.iter().any(|&s| s != 0.0),
            "a real 60 ms frame after a 120 ms packet must still decode"
        );
    }

    // End-to-end: decode the whole capture and compare against the reference output
    // (`ref_usesmpl_expected.raw`; see testdata/PROVENANCE.md).
    //
    // An earlier target (`e2e_vectors.json`) was proven wrong: it used the int16-domain `*nrgres`
    // excitation with no shaped noise (a tail-off bug) and correlates ~0 with the true codec. With
    // the per-block voiced ACB/LTP lags, the HP postfilter, and the harmonic postfilter (which emits
    // the SMPL_TOT_POSTFILT_DELAY = 48-sample group delay) all in place, the decode now aligns
    // sample-for-sample at lag 0.
    #[test]
    fn e2e_decode_matches_usesmpl() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json"))
                .expect("inbound_capture_frames.json");
        let refp: Vec<f32> = include_bytes!("testdata/ref_usesmpl_expected.raw")
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
            .collect();

        let mut dec = MlowDecoder::new();
        let mut out: Vec<f32> = Vec::new();
        for hex_frame in &frames {
            let frame = hex::decode(hex_frame).unwrap();
            out.extend_from_slice(&dec.decode(&frame));
        }
        assert_eq!(out.len(), refp.len(), "decode length vs reference");

        // Aligned at lag 0 now (the harmonic postfilter emits the 48-sample group delay).
        const LAG: usize = 0;
        let n = refp.len() - LAG;
        let (r, o) = (&refp[LAG..LAG + n], &out[..n]);
        let mr: f64 = r.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
        let mo: f64 = o.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
        let (mut sxy, mut sxx, mut syy) = (0f64, 0f64, 0f64);
        for i in 0..n {
            let dr = r[i] as f64 - mr;
            let dz = o[i] as f64 - mo;
            sxy += dr * dz;
            sxx += dr * dr;
            syy += dz * dz;
        }
        let corr = sxy / (sxx * syy).sqrt();
        assert!(corr > 0.95, "lag-0 corr {corr:.4} vs reference");
    }

    /// Inputs the corpus below produces: 8000 LCG buffers, then per capture frame
    /// eight bit-flips and one truncation per byte plus the frame itself.
    ///
    /// Asserted at the end of every shard rather than derived, so growing the
    /// corpus without re-dividing it fails instead of leaving the new tail
    /// unfuzzed.
    const FUZZ_CORPUS_LEN: usize = 149_104;
    const FUZZ_SHARDS: usize = 4;

    // R2 (fuzz no-panic): the decoder is fed adversarial inputs and must neither panic nor over-emit.
    // Corpus: a deterministic LCG of random byte vectors, plus every capture frame with each single
    // byte flipped and each prefix truncation. The contract is purely structural (no panic, bounded
    // output); the range decoder absorbs corruption by returning zero, so `had_error` is not asserted.
    //
    // Output length is data-driven by the TOC: `sample_rate/1000 * frame_ms`, where the TOC fields
    // span {16,32} kHz and {10,20,60,120} ms. The hard ceiling is therefore 32 * 120 = 3840 samples,
    // not the 960 of a common 60 ms / 16 kHz frame; a fuzzed TOC can legitimately declare a larger
    // frame, which the decoder fills with silence on the SID/inactive/std-opus paths.
    //
    // Sharded because ~149k decoder calls in one process were 90% of the unit suite's wall clock
    // with three of the runner's four cores idle. Each shard walks the whole corpus and decodes
    // only its own contiguous slice, through a decoder it keeps for the length of that slice: the
    // property includes what a poisoned frame leaves behind for the next one, which a fresh decoder
    // per input would not exercise. Four slices, contiguous and disjoint, so their union is the
    // corpus the single test fed. Do not merge them back.
    fn fuzz_decode_shard(shard: usize) {
        const MAX_SAMPS: usize = 32 * 120; // max sample_rate(kHz) * max frame_ms across all TOCs
        let lo = FUZZ_CORPUS_LEN * shard / FUZZ_SHARDS;
        let hi = FUZZ_CORPUS_LEN * (shard + 1) / FUZZ_SHARDS;

        let mut index = 0usize;
        let mut decoded = 0usize;
        let mut dec = MlowDecoder::new();
        let mut check = |dec: &mut MlowDecoder, input: &[u8]| {
            let mine = index >= lo && index < hi;
            index += 1;
            if !mine {
                return;
            }
            decoded += 1;
            let out = dec.decode(input);
            assert!(
                out.len() <= MAX_SAMPS,
                "decode emitted {} > {MAX_SAMPS} samples for input len {}",
                out.len(),
                input.len()
            );
        };

        // Deterministic LCG (numerical-recipes constants) over thousands of random-length buffers.
        let mut seed: u32 = 0x1234_5678;
        let next = |s: &mut u32| {
            *s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            *s
        };
        for _ in 0..8000 {
            let len = (next(&mut seed) % 400) as usize;
            let mut buf = Vec::with_capacity(len);
            for _ in 0..len {
                buf.push((next(&mut seed) >> 24) as u8);
            }
            check(&mut dec, &buf);
        }

        // Mutations of the real capture frames: every single-byte flip and every truncation.
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json"))
                .expect("inbound_capture_frames.json");
        for hex_frame in &frames {
            let frame = hex::decode(hex_frame).unwrap();
            for i in 0..frame.len() {
                for bit in 0..8 {
                    let mut m = frame.clone();
                    m[i] ^= 1 << bit;
                    check(&mut dec, &m);
                }
                check(&mut dec, &frame[..i]); // truncation at every prefix length
            }
            check(&mut dec, &frame);
        }

        assert_eq!(
            index, FUZZ_CORPUS_LEN,
            "corpus is {index} inputs, not the {FUZZ_CORPUS_LEN} the shards divide"
        );
        assert_eq!(
            decoded,
            hi - lo,
            "shard {shard} covered {decoded} of {}",
            hi - lo
        );
    }

    #[test]
    fn fuzz_decode_no_panic_bounded_output_shard_0() {
        fuzz_decode_shard(0);
    }

    #[test]
    fn fuzz_decode_no_panic_bounded_output_shard_1() {
        fuzz_decode_shard(1);
    }

    #[test]
    fn fuzz_decode_no_panic_bounded_output_shard_2() {
        fuzz_decode_shard(2);
    }

    #[test]
    fn fuzz_decode_no_panic_bounded_output_shard_3() {
        fuzz_decode_shard(3);
    }

    // Fail-loud guards: a frame outside our single operating point (32kHz/fullband, or low_rate=1) is
    // DROPPED to 60ms silence WITHOUT touching the range coder, so it can't desync + poison the
    // cross-frame predictor. A real low_rate=0 frame before and after the drops still decodes (proves
    // the drop is a clean "lost frame", not a desync), and the guards don't false-positive on it.
    #[test]
    fn unsupported_frames_drop_clean_without_desync() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json")).unwrap();
        let real = hex::decode(&frames[0]).unwrap();
        let mut dec = MlowDecoder::new();

        // A real 0x50 (16kHz, low_rate=0) frame decodes to audio — the guards don't false-positive.
        let normal = dec.decode(&real);
        assert_eq!(normal.len(), 960);
        assert!(
            normal.iter().any(|&s| s != 0.0),
            "a real low_rate=0 frame must decode to audio"
        );

        // A 32kHz/fullband TOC (bit5=1, e.g. 0x70) -> dropped. It declares 60 ms, and the slot is
        // 60 ms of OUTPUT samples: the synthesis rate does not follow the declared internal rate.
        let out_32k = dec.decode(&[0x70, 0xAA, 0xBB, 0xCC]);
        assert_eq!(out_32k.len(), 960);
        assert!(
            out_32k.iter().all(|&s| s == 0.0),
            "32kHz frame must drop to silence"
        );

        // A low_rate=1 TOC -> dropped to 60ms silence. 0x54 = 0b0101_0100: bit5=0 (16kHz, so the rate
        // branch does NOT catch it), bits4:3=10 (60ms), bit2=1 (low_rate) -> exercises the low_rate branch.
        let out_lr = dec.decode(&[0x54, 0xAA, 0xBB, 0xCC]);
        assert_eq!(out_lr.len(), 960);
        assert!(
            out_lr.iter().all(|&s| s == 0.0),
            "low_rate=1 frame must drop to silence"
        );

        // A 10ms ACTIVE 16kHz/low_rate=0 TOC (0x40) must also drop: it is the one duration whose
        // internal frame length and subframe count differ, so decoding it under the implemented
        // geometry would desync the range coder. 20/60/120ms all decode (see the geometry tests).
        let out_10ms = dec.decode(&[0x40, 0xAA, 0xBB, 0xCC]);
        assert_eq!(out_10ms.len(), 160, "the slot is the declared 10 ms");
        assert!(
            out_10ms.iter().all(|&s| s == 0.0),
            "a 10ms active frame must drop to silence"
        );

        // The drops never opened the range decoder, so the predictor is intact: the real frame still
        // decodes to audio afterwards (no desync from the dropped frames in between).
        assert!(
            !dec.had_error(),
            "dropped frames must not touch the range decoder"
        );
        let after = dec.decode(&real);
        assert!(
            after.iter().any(|&s| s != 0.0),
            "a real frame after the drops must still decode (no poisoned predictor)"
        );
    }

    // A SID frame (TOC 0x80, the DTX/CNG marker) is comfort noise, not a desync hazard: it is
    // silenced without opening the range coder regardless of geometry. It must take the quiet path (a
    // full 60ms silence slot, range coder untouched) and must NOT count as an out-of-operating-point
    // drop, which is reserved for frames that would have lost decodable audio.
    #[test]
    fn sid_frame_is_silenced_not_dropped() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json")).unwrap();
        let real = hex::decode(&frames[0]).unwrap();
        let mut dec = MlowDecoder::new();

        assert!(
            dec.decode(&real).iter().any(|&s| s != 0.0),
            "a real low_rate=0 frame must decode to audio"
        );

        // TOC 0x80 -> SID: silenced via the comfort-noise path, not the operating-point drop. It
        // declares 10 ms, so it fills 10 ms: silence tracks the cadence the peer is sending at.
        let inactive = dec.decode(&[0x80, 0xAA, 0xBB, 0xCC]);
        assert_eq!(
            inactive.len(),
            160,
            "a SID occupies the duration it declares"
        );
        assert!(
            inactive.iter().all(|&s| s == 0.0),
            "an inactive frame must be silence"
        );
        assert_eq!(
            dec.dropped_unsupported, 0,
            "a SID frame is the comfort-noise path, not an operating-point drop"
        );
        assert!(
            !dec.had_error(),
            "the SID path must not open the range decoder"
        );

        // Contrast: an active off-point frame (0x60 = vad=true, 32 kHz) IS counted, proving the drop
        // counter discriminates real audio loss from benign inactive silence.
        let _ = dec.decode(&[0x60, 0xAA, 0xBB, 0xCC]);
        assert_eq!(
            dec.dropped_unsupported, 1,
            "an active off-point frame must count as a drop"
        );

        // After both the inactive frame and the drop, a real frame still decodes: no desync, no
        // poisoned predictor.
        assert!(
            dec.decode(&real).iter().any(|&s| s != 0.0),
            "a real frame must still decode after an inactive frame and a drop"
        );
    }

    // R7 (RED round-trip): a bare frame wrapped in a 1-redundant SplitRed envelope must decode to the
    // exact same PCM as the bare frame at redundancy 0. Exercises the `redundancy > 0` strip path
    // (which forwards the main/last frame) end-to-end.
    #[test]
    fn red_envelope_decodes_to_bare_main() {
        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json"))
                .expect("inbound_capture_frames.json");
        let bare = hex::decode(&frames[0]).unwrap();

        let mut bare_dec = MlowDecoder::new();
        let bare_out = bare_dec.decode(&bare);

        // SplitRed N=1: red_hdr [0x80 | tc, size], main_marker (high bit clear), red payload, main.
        // The main (last) frame is the bare frame, so the strip path must reproduce `bare_out`.
        let red_payload = [0xAAu8, 0xBB];
        let mut env = vec![0x80u8, red_payload.len() as u8, 0x00];
        env.extend_from_slice(&red_payload);
        env.extend_from_slice(&bare);

        let mut red_dec = MlowDecoder::new();
        red_dec.set_redundancy(1);
        let red_out = red_dec.decode(&env);

        assert_eq!(
            red_out, bare_out,
            "RED-wrapped main differs from bare decode"
        );
        assert!(
            !red_dec.had_error(),
            "RED decode raised the range decoder error flag"
        );
    }
}
