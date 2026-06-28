//! MLow "smpl_toc": the first byte of a bare MLow frame. The smpl TOC is only valid when
//! `(b & 0xC0) != 0xC0`; `(b & 0xC0) == 0xC0` marks a STANDARD Opus/CELT TOC instead, which is
//! routed to stock Opus.
//!
//! Bit layout (LSB = bit0): bit7=SID(DTX/CNG), bit6=VAD, bit5=internal rate(0->16k,1->32k),
//! bits4:3->frame size index into {10,20,60,120}ms, bit2=flag2, bit1=voiced-enable, bit0=flag0.

/// Decoded smpl TOC. `std_opus` true means the remaining fields are unused and the frame is a
/// standard Opus/CELT packet (decode with stock Opus, not the smpl path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MlowToc {
    pub std_opus: bool,
    pub sid: bool,
    pub vad: bool,
    pub sample_rate: i32,
    pub frame_ms: i32,
    pub voiced: bool,
    pub active: bool,
    pub flag2: bool,
    pub flag0: bool,
}

/// Frame duration (ms) of a standard Opus packet from its TOC config field `b>>3` (RFC 6716
/// Table 2). 2.5 ms is rounded up; the smpl path only needs an output length for CNG frames.
fn standard_opus_frame_ms(b: u8) -> i32 {
    let config = b >> 3;
    if config < 12 {
        [10, 20, 40, 60][(config & 3) as usize] // SILK NB/MB/WB
    } else if config < 16 {
        [10, 20][((config - 12) & 1) as usize] // Hybrid
    } else {
        match config & 3 {
            0 => 3, // 2.5 ms rounded up
            1 => 5,
            2 => 10,
            _ => 20,
        }
    }
}

/// Parse the smpl TOC byte. Emits a per-frame `trace!` so production logs can show the routing of
/// every inbound frame (this parse is not yet prod-validated, so keep it instrumented).
pub(crate) fn parse_mlow_toc(b: u8) -> MlowToc {
    if b & 0xC0 == 0xC0 {
        let toc = MlowToc {
            std_opus: true,
            sid: false,
            vad: false,
            sample_rate: 16000,
            frame_ms: standard_opus_frame_ms(b),
            voiced: false,
            active: false,
            flag2: false,
            flag0: false,
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
        voiced: vad && bit1,
        active: vad || bit1,
        flag2: (b >> 2) & 1 != 0,
        flag0: b & 1 != 0,
    };
    log::trace!(
        "mlow TOC 0x{b:02x}: sid={} vad={} sr={} ms={} voiced={} active={} f2={} f0={}",
        toc.sid,
        toc.vad,
        toc.sample_rate,
        toc.frame_ms,
        toc.voiced,
        toc.active,
        toc.flag2,
        toc.flag0
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
            assert_eq!(
                t.frame_ms,
                rec["ms"].as_i64().unwrap() as i32,
                "ms b=0x{b:02x}"
            );
            assert_eq!(
                t.voiced,
                rec["voiced"].as_bool().unwrap(),
                "voiced b=0x{b:02x}"
            );
            assert_eq!(
                t.active,
                rec["active"].as_bool().unwrap(),
                "active b=0x{b:02x}"
            );
            assert_eq!(t.flag2, rec["f2"].as_bool().unwrap(), "f2 b=0x{b:02x}");
            assert_eq!(t.flag0, rec["f0"].as_bool().unwrap(), "f0 b=0x{b:02x}");
        }
    }
}
