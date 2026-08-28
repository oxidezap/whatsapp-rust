//! MLow RED ("SplitRed") depacketization: the OUTERMOST wire layer of a WhatsApp MLow RTP audio
//! payload.
//!
//! **The payload type selects this, not the redundancy level.** The shipped client applies the
//! wrapper to everything it sends on the `mlow-red-1` payload type, and a redundancy level of zero
//! still gets one: a single `0x00` marker byte and then the frame. Only a payload arriving on the
//! bare MLow payload type is unwrapped, and applying this to one would misread its TOC (whose high
//! bit is set) as a redundant block header.
//!
//! The engine keys off the payload type for exactly that reason; the level is a bitrate decision,
//! not a framing one, and treating it as framing means a zero-redundancy stream on the RED payload
//! type loses its first byte to the TOC.
//!
//! On-wire (N = redundancy): `[ red_hdr[0..N] (2B each) ][ main_marker (1B) ][ red_payloads ][ main_payload ]`.
//! `red_hdr[i]`: byte0 = `0x80 | (time_code & 0x7f)`, byte1 = payload size. `main_marker`: high bit
//! clear, low 7 bits = main time offset. It is not RFC 2198: no block carries a payload type, and
//! every block is decoded by the codec the payload type already selected.

/// One frame extracted from a SplitRed payload: raw MLow frame bytes (TOC + body) plus RED metadata.
/// `data` borrows the input payload (no copy).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MlowFrame<'a> {
    pub data: &'a [u8],
    pub time_code: u8,
    pub is_main: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedError {
    PktSizeZero,
    HeaderTooShort,
    RedundantTooShort,
    MainTooShort,
}

/// Parse a SplitRed RED packet into its frames (redundant blocks in header order, then the main
/// frame last). Errors on malformed RED inputs. Only call when RED was negotiated: a bare MLow
/// frame would be misparsed here (an active-voice TOC like 0x50 has its high bit clear and reads as
/// a 0-redundancy main marker, not an error), so for a bare-frame stream feed the whole RTP payload
/// as one MLow frame instead.
pub(crate) fn depack_split_red(p: &[u8]) -> Result<Vec<MlowFrame<'_>>, RedError> {
    let n = p.len();
    if n == 0 {
        return Err(RedError::PktSizeZero);
    }
    struct RedBlock {
        code: u8,
        size: u8,
    }
    let mut red: Vec<RedBlock> = Vec::new();
    let mut cur = 0usize;
    let mut rem = n;
    loop {
        if rem == 0 {
            return Err(RedError::HeaderTooShort);
        }
        let b0 = p[cur];
        if b0 < 0x80 {
            // main marker (high bit clear) terminates the header run
            if rem <= 1 {
                return Err(RedError::MainTooShort);
            }
            break;
        }
        if rem <= 2 {
            return Err(RedError::RedundantTooShort);
        }
        let size = p[cur + 1];
        if size as usize + 2 >= rem {
            return Err(RedError::RedundantTooShort);
        }
        red.push(RedBlock {
            code: b0 & 0x7f,
            size,
        });
        cur += 2;
        rem -= size as usize + 2;
    }

    let main_code = p[cur] & 0x7f;
    cur += 1;

    let mut frames = Vec::with_capacity(red.len() + 1);
    for r in &red {
        frames.push(MlowFrame {
            data: &p[cur..cur + r.size as usize],
            time_code: r.code,
            is_main: false,
        });
        cur += r.size as usize;
    }
    let main_size = rem - 1; // total - header_size - sum(redundant sizes)
    frames.push(MlowFrame {
        data: &p[cur..cur + main_size],
        time_code: main_code,
        is_main: true,
    });
    log::debug!(
        "mlow RED: {} redundant + 1 main frame ({}B main)",
        red.len(),
        main_size
    );
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depack_one_redundant_plus_main() {
        // N=1: hdr [0x85,0x03] [main_marker 0x00] | red payload [AA BB CC] | main [50 11 22 33].
        let p = [0x85, 0x03, 0x00, 0xAA, 0xBB, 0xCC, 0x50, 0x11, 0x22, 0x33];
        let frames = depack_split_red(&p).unwrap();
        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0],
            MlowFrame {
                data: &[0xAA, 0xBB, 0xCC],
                time_code: 5,
                is_main: false
            }
        );
        assert_eq!(
            frames[1],
            MlowFrame {
                data: &[0x50, 0x11, 0x22, 0x33],
                time_code: 0,
                is_main: true
            }
        );
    }

    #[test]
    fn depack_no_redundant_just_main() {
        // header is just the main marker (0x00), then the main payload.
        let p = [0x00, 0x50, 0x11, 0x22];
        let frames = depack_split_red(&p).unwrap();
        assert_eq!(frames.len(), 1);
        assert!(frames[0].is_main);
        assert_eq!(frames[0].data, &[0x50, 0x11, 0x22]);
    }

    #[test]
    fn empty_packet_errors() {
        assert_eq!(depack_split_red(&[]), Err(RedError::PktSizeZero));
    }

    #[test]
    fn bare_frame_is_rejected() {
        // A bare MLow frame (high-bit-set TOC like a DTX 0x90) must NOT parse as SplitRed.
        assert!(depack_split_red(&[0x90, 0x01, 0x02]).is_err());
    }
}
