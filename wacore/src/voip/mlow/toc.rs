//! MLOW "smpl_toc": the first byte of a payload after negotiation selected the MLOW profile.
//! `(b & 0xC0) == 0xC0` is its in-profile standard Opus/CELT escape; it is not a global codec
//! discriminator.
//!
//! Bit layout (LSB = bit0): bit7=SID(DTX/CNG), bit6=VAD, bit5=internal rate(0->16k,1->32k),
//! bits4:3->frame size index into {10,20,60,120}ms, bit2=low_rate, bit1=FEC, bit0=stereo.
//!
//! The bit-1 and bit-0 names are not guesses: `smpl_decode_toc` was extracted from the shipped
//! WhatsApp Web VoIP module and executed against the C reference over all 192 in-profile bytes,
//! and all eight fields agree. Bit 1 is an in-band LBRR/FEC flag rather than a voicing decision
//! (voicing is decoded per subframe, further in), and bit 0 is the stereo flag that
//! `MLOW_TOC_FIXED_MASK` (0x39) carries into a multiframe indicator.

/// Decoded smpl TOC. `std_opus` true means the remaining fields are unused and the frame is a
/// standard Opus/CELT packet (decode with stock Opus, not the smpl path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MlowToc {
    pub std_opus: bool,
    pub sid: bool,
    pub vad: bool,
    pub sample_rate: i32,
    pub frame_ms: i32,
    /// The packet carries an in-band LBRR/FEC copy of the previous frame.
    pub fec: bool,
    /// The frame is coded as active voice, which gates two symbols in the LSF decode. NOT a claim
    /// that the frame has audio: a frame coded inactive still carries a decodable body.
    pub active: bool,
    /// The reduced-rate geometry (2x160 internal frames), which this decoder does not implement.
    pub low_rate: bool,
    pub stereo: bool,
}

/// Frame duration (ms) of the in-profile CELT escape.
///
/// **This is not the RFC 6716 TOC layout**, and reading it as one was a live contradiction inside
/// this crate: `packetize_opus_for_mlow` writes `0xC0 | mode << 2 | stereo << 1 | multi`, and
/// `depacketize_opus_from_mlow` reverses exactly that, while this function used to recover the
/// config as `b >> 3`. The two disagreed on the duration for 56 of the 64 escape bytes.
///
/// The real layout, confirmed by executing `opus_smpl_decode_TOC` out of the shipped module: bits
/// 5:2 are a CELT-only mode, its high two bits select the rate and its low two the duration from
/// {2.5, 5, 10, 20} ms. Only CELT is reachable here, because the escape writer refuses anything
/// else.
///
/// 2.5 ms rounds up to 3: the smpl path only needs a length for a slot it fills with silence, and
/// rounding down would under-fill it.
fn celt_escape_frame_ms(b: u8) -> i32 {
    // Only the low two bits of the mode select the duration; the high two select the rate, which
    // this path does not need because it emits a slot at the decoder's own 16 kHz. Masking the
    // whole nibble first would be dead arithmetic, so the bits are taken directly.
    [3, 5, 10, 20][((b >> 2) & 3) as usize]
}

/// Parse the smpl TOC byte. Emits a per-frame `trace!` while this parse is production-validated.
pub(crate) fn parse_mlow_toc(b: u8) -> MlowToc {
    if b & 0xC0 == 0xC0 {
        let toc = MlowToc {
            std_opus: true,
            sid: false,
            vad: false,
            sample_rate: 16000,
            frame_ms: celt_escape_frame_ms(b),
            fec: false,
            active: false,
            low_rate: false,
            stereo: false,
        };
        log::trace!(
            "mlow TOC 0x{b:02x}: standard-opus frame_ms={}",
            toc.frame_ms
        );
        return toc;
    }
    let bit1 = (b >> 1) & 1 != 0;
    let vad = (b >> 6) & 1 != 0;
    let toc = MlowToc {
        std_opus: false,
        sid: b >> 7 != 0,
        vad,
        sample_rate: if b & 0x20 != 0 { 32000 } else { 16000 },
        frame_ms: [10, 20, 60, 120][((b >> 3) & 3) as usize],
        fec: vad && bit1,
        active: vad || bit1,
        low_rate: (b >> 2) & 1 != 0,
        stereo: b & 1 != 0,
    };
    log::trace!(
        "mlow TOC 0x{b:02x}: sid={} vad={} sr={} ms={} fec={} active={} low_rate={} stereo={}",
        toc.sid,
        toc.vad,
        toc.sample_rate,
        toc.frame_ms,
        toc.fec,
        toc.active,
        toc.low_rate,
        toc.stereo
    );
    toc
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // Exhaustively validates the parse over every byte value against the captured vectors.
    #[test]
    fn toc_matches_go_all_256() {
        let recs: Value =
            serde_json::from_str(include_str!("testdata/toc_vectors.json")).expect("toc_vectors");
        let arr = recs.as_array().unwrap();
        assert_eq!(arr.len(), 256);
        for rec in arr {
            let b = rec["b"].as_u64().unwrap() as u8;
            let t = parse_mlow_toc(b);
            assert_eq!(t.std_opus, rec["std"].as_bool().unwrap(), "std b=0x{b:02x}");
            assert_eq!(t.sid, rec["sid"].as_bool().unwrap(), "sid b=0x{b:02x}");
            assert_eq!(t.vad, rec["vad"].as_bool().unwrap(), "vad b=0x{b:02x}");
            assert_eq!(
                t.sample_rate,
                rec["sr"].as_i64().unwrap() as i32,
                "sr b=0x{b:02x}"
            );
            // The escape range is deliberately NOT checked against this fixture. Its `ms` there
            // records the RFC 6716 reading (`config = b >> 3`), which is not the layout the escape
            // uses; `celt_escape_frame_ms` and `escape_durations_agree_with_the_escape_writer`
            // cover it instead. Editing the fixture to agree with the code is what hid a decoder
            // bug in this module once already, so it is left alone and its stale range is named.
            if !t.std_opus {
                assert_eq!(
                    t.frame_ms,
                    rec["ms"].as_i64().unwrap() as i32,
                    "ms b=0x{b:02x}"
                );
            }
            assert_eq!(
                t.fec,
                rec["voiced"].as_bool().unwrap(),
                "voiced b=0x{b:02x}"
            );
            assert_eq!(
                t.active,
                rec["active"].as_bool().unwrap(),
                "active b=0x{b:02x}"
            );
            assert_eq!(t.low_rate, rec["f2"].as_bool().unwrap(), "f2 b=0x{b:02x}");
            assert_eq!(t.stereo, rec["f0"].as_bool().unwrap(), "f0 b=0x{b:02x}");
        }
    }

    /// The escape reader must agree with the escape writer, which is in this same crate.
    ///
    /// `packetize_opus_for_mlow` builds the byte from an RFC config, so round-tripping every CELT
    /// configuration through it and back gives an independent statement of what each escape byte
    /// means. They disagreed for 56 of the 64 escape bytes before this: the writer was right and
    /// the reader was reading an RFC 6716 TOC that the escape does not use.
    #[test]
    fn escape_durations_agree_with_the_escape_writer() {
        // RFC 6716 table 2, CELT half: durations {2.5, 5, 10, 20} ms by the low two config bits.
        // 2.5 rounds up to 3 for the same reason the parser does.
        const CELT_MS: [i32; 4] = [3, 5, 10, 20];
        let mut checked = 0;
        for config in 16u8..32 {
            for stereo in [false, true] {
                let mut packet = vec![config << 3 | u8::from(stereo) << 2, 0x11, 0x22, 0x33];
                crate::voip::audio::packetize_opus_for_mlow(&mut packet)
                    .expect("every CELT config escapes");
                let toc = parse_mlow_toc(packet[0]);
                assert!(
                    toc.std_opus,
                    "config {config} must escape into the CELT range"
                );
                assert_eq!(
                    toc.frame_ms,
                    CELT_MS[(config & 3) as usize],
                    "config {config} round-tripped to the wrong duration (byte {:#04x})",
                    packet[0]
                );
                checked += 1;
            }
        }
        assert_eq!(checked, 32, "every CELT configuration must be covered");
    }
}
