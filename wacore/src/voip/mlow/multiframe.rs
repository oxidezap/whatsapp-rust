//! MLow's multiframe envelope: several complete MLow packets carried in one RTP payload.
//!
//! This is how the real client reaches a packet longer than its frame length. Its encoder caches a
//! fixed three blocks of 20 ms, so a `frame_length_ms` of 120 does not produce one 120 ms block: it
//! produces two 60 ms blocks aggregated here. A decoder that does not implement this reads the
//! envelope's first byte as a TOC, finds bit 7 set, calls it a SID, and emits comfort noise for
//! every packet of the call. The call goes completely silent with no error anywhere, which is worse
//! than the bug this port was written to fix, because at least that one logged something.
//!
//! The envelope indicator is `(b & 0xC0) != 0xC0 && (b & 0x82) == 0x82`. That point is unreachable
//! as a real TOC by construction: bit 7 is SID and bit 1 is FEC, and the encoder asserts a SID frame
//! never carries FEC, so no frame it emits can have both set. The collision is deliberate.

/// Frames the real parser accepts in one envelope. Above this it refuses the packet outright.
const MAX_FRAMES: usize = 18;

/// Bits of the count byte that hold the frame count; the rest are the padding flag and one the
/// client ignores.
const COUNT_MASK: u8 = 0x3f;
/// Opus-style padding follows the count byte when this is set.
const PADDING_FLAG: u8 = 0x40;

/// Why an envelope could not be split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub(crate) enum MultiframeError {
    #[error("the envelope ended before its header did")]
    Truncated,
    #[error("frame count {0} is outside 1..=18")]
    BadFrameCount(usize),
    #[error("a declared frame length runs past the end of the envelope")]
    LengthOverrun,
    #[error("a sub-frame is empty")]
    EmptySubFrame,
}

/// Is this payload a multiframe envelope rather than a single MLow frame?
///
/// Checked before the TOC is interpreted, because under the TOC grammar these bytes read as a SID
/// and would be silently turned into comfort noise.
#[must_use]
pub(crate) fn is_multiframe(payload: &[u8]) -> bool {
    // Two bytes minimum: the indicator and the count. The real parser requires the same.
    payload.len() >= 2 && payload[0] & 0xC0 != 0xC0 && payload[0] & 0x82 == 0x82
}

/// Split one envelope into the complete MLow packets it carries, in transmission order.
///
/// Each element is a whole packet with its own TOC, decodable exactly as if it had arrived alone.
/// The caller decodes them in order and concatenates: the envelope's RTP timestamp belongs to the
/// **last** sub-frame, and the earlier ones run backwards from it, so playing them in array order
/// is what puts the audio back in time order.
///
/// Borrows the input; no allocation beyond the returned `Vec` of slices, which is bounded by
/// [`MAX_FRAMES`].
pub(crate) fn split_multiframe(payload: &[u8]) -> Result<Vec<&[u8]>, MultiframeError> {
    let count_byte = *payload.get(1).ok_or(MultiframeError::Truncated)?;
    let frames = usize::from(count_byte & COUNT_MASK);
    if frames == 0 || frames > MAX_FRAMES {
        return Err(MultiframeError::BadFrameCount(frames));
    }
    let mut rest = payload.get(2..).ok_or(MultiframeError::Truncated)?;

    // Opus padding: a run of 255s, then a final byte giving the remaining padding length. The bytes
    // themselves are meaningless; only the accounting matters, and it has to not overrun.
    let mut padding = 0usize;
    if count_byte & PADDING_FLAG != 0 {
        loop {
            let (&byte, next) = rest.split_first().ok_or(MultiframeError::Truncated)?;
            rest = next;
            if byte == 255 {
                padding = padding
                    .checked_add(254)
                    .ok_or(MultiframeError::LengthOverrun)?;
            } else {
                padding = padding
                    .checked_add(usize::from(byte))
                    .ok_or(MultiframeError::LengthOverrun)?;
                break;
            }
        }
    }

    // One explicit length per frame except the last, which takes whatever is left. Same one-or-two
    // byte encoding standard Opus uses.
    let mut lengths = [0usize; MAX_FRAMES];
    let mut declared = 0usize;
    for slot in lengths.iter_mut().take(frames - 1) {
        let (&first, next) = rest.split_first().ok_or(MultiframeError::Truncated)?;
        let length = if first < 252 {
            rest = next;
            usize::from(first)
        } else {
            let (&second, after) = next.split_first().ok_or(MultiframeError::Truncated)?;
            rest = after;
            usize::from(first) + usize::from(second) * 4
        };
        *slot = length;
        declared = declared
            .checked_add(length)
            .ok_or(MultiframeError::LengthOverrun)?;
    }

    let bodies = rest
        .len()
        .checked_sub(padding)
        .ok_or(MultiframeError::LengthOverrun)?;
    let last = bodies
        .checked_sub(declared)
        .ok_or(MultiframeError::LengthOverrun)?;
    if last == 0 {
        return Err(MultiframeError::EmptySubFrame);
    }
    lengths[frames - 1] = last;

    let mut out = Vec::with_capacity(frames);
    let mut cursor = rest;
    for &length in lengths.iter().take(frames) {
        if length == 0 {
            return Err(MultiframeError::EmptySubFrame);
        }
        let (frame, next) = cursor
            .split_at_checked(length)
            .ok_or(MultiframeError::LengthOverrun)?;
        out.push(frame);
        cursor = next;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The envelope a peer with frames-per-packet 2 emits: the indicator carries the sub-frames'
    /// fixed bits, and each body is a complete MLow packet.
    fn envelope(frames: &[&[u8]]) -> Vec<u8> {
        let mut out = vec![0x82 | (frames[0][0] & 0x39), frames.len() as u8];
        for frame in &frames[..frames.len() - 1] {
            assert!(frame.len() < 252, "fixture keeps lengths single-byte");
            out.push(frame.len() as u8);
        }
        for frame in frames {
            out.extend_from_slice(frame);
        }
        out
    }

    // Two 60 ms blocks in one envelope: exactly what the real client emits for a 120 ms frame
    // length, and exactly what this port used to read as a SID.
    #[test]
    fn a_two_frame_envelope_splits_into_two_whole_packets() {
        let first: Vec<u8> = core::iter::once(0x50u8).chain(0..40u8).collect();
        let second: Vec<u8> = core::iter::once(0x50u8).chain(40..90u8).collect();
        let packet = envelope(&[&first, &second]);
        assert!(is_multiframe(&packet));
        let frames = split_multiframe(&packet).expect("a well-formed envelope");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0], first.as_slice());
        assert_eq!(frames[1], second.as_slice());
    }

    // The indicator is unreachable as a real TOC because the encoder never emits a SID frame that
    // also carries FEC. If this ever stopped holding, an ordinary frame would be split as an
    // envelope and the call would break in a new way.
    //
    // Asserted against the REAL parser, not a local re-derivation of the bit layout: a test that
    // recomputes the classification it is checking proves only that two copies of the same
    // expression agree.
    #[test]
    fn the_indicator_is_not_reachable_as_an_ordinary_toc() {
        use super::super::toc::parse_mlow_toc;

        let mut envelopes = 0;
        for byte in 0..=255u8 {
            let toc = parse_mlow_toc(byte);
            let looks_like_envelope = is_multiframe(&[byte, 0x01, 0xaa]);
            if !looks_like_envelope {
                continue;
            }
            envelopes += 1;
            // Every byte the splitter claims is an envelope must be one the encoder cannot emit as
            // a frame: it declares SID, and the SID path is the one that would have silenced it.
            assert!(
                toc.sid,
                "envelope indicator {byte:#04x} would otherwise be a decodable frame"
            );
            assert!(
                !toc.std_opus,
                "the CELT escape must not be mistaken for an envelope ({byte:#04x})"
            );
            // A SID never carries FEC in anything the encoder produces, which is what makes the
            // bit-1 collision safe. `MlowToc::fec` is `vad && bit1`, and a SID has vad clear, so the
            // raw bit is what has to be checked here.
            assert_eq!(byte & 0x02, 0x02);
            assert!(!toc.fec, "a SID frame never reports FEC");
        }
        assert_eq!(
            envelopes, 32,
            "the indicator space is bits 7 and 1 set with bit 6 clear"
        );
    }

    #[test]
    fn a_single_byte_payload_is_never_an_envelope() {
        assert!(!is_multiframe(&[0x92]));
    }

    #[test]
    fn a_frame_count_outside_the_accepted_range_is_refused() {
        for count in [0u8, 19, 63] {
            let mut packet = vec![0x92u8, count];
            packet.extend(0..40u8);
            assert_eq!(
                split_multiframe(&packet),
                Err(MultiframeError::BadFrameCount(usize::from(count))),
            );
        }
    }

    #[test]
    fn a_length_that_runs_past_the_end_is_refused() {
        // Two frames, the first declared as 200 bytes inside a 10-byte body.
        let mut packet = vec![0x92u8, 0x02, 200];
        packet.extend(0..10u8);
        assert_eq!(
            split_multiframe(&packet),
            Err(MultiframeError::LengthOverrun)
        );
    }

    #[test]
    fn an_envelope_with_nothing_left_for_the_last_frame_is_refused() {
        // Two frames, the first taking the whole body.
        let mut packet = vec![0x92u8, 0x02, 10];
        packet.extend(0..10u8);
        assert_eq!(
            split_multiframe(&packet),
            Err(MultiframeError::EmptySubFrame)
        );
    }

    #[test]
    fn a_two_byte_length_is_decoded_the_way_opus_encodes_it() {
        // 252 + 1*4 = 256 bytes in the first frame.
        let mut packet = vec![0x92u8, 0x02, 252, 1];
        packet.extend((0..300u32).map(|i| i as u8));
        let frames = split_multiframe(&packet).expect("well formed");
        assert_eq!(frames[0].len(), 256);
        assert_eq!(frames[1].len(), 300 - 256);
    }

    #[test]
    fn declared_padding_is_removed_from_the_last_frame() {
        // One frame, 4 bytes of padding: the body is 20 bytes, so the frame is 16.
        let mut packet = vec![0x92u8, 0x41, 4];
        packet.extend(0..20u8);
        let frames = split_multiframe(&packet).expect("well formed");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].len(), 16);
    }

    #[test]
    fn padding_that_overruns_the_envelope_is_refused() {
        let mut packet = vec![0x92u8, 0x41, 200];
        packet.extend(0..10u8);
        assert_eq!(
            split_multiframe(&packet),
            Err(MultiframeError::LengthOverrun)
        );
    }

    #[test]
    fn a_truncated_envelope_is_refused_without_panicking() {
        assert_eq!(split_multiframe(&[0x92]), Err(MultiframeError::Truncated));
        assert_eq!(
            split_multiframe(&[0x92, 0x02]),
            Err(MultiframeError::Truncated)
        );
        assert_eq!(
            split_multiframe(&[0x92, 0x02, 5]),
            Err(MultiframeError::LengthOverrun)
        );
    }

    // Every input either splits into a sane set of sub-frames or is refused, and the refusals are
    // counted so a splitter that started rejecting everything could not pass by never entering the
    // `Ok` arm.
    #[test]
    fn every_short_input_either_splits_sanely_or_is_refused() {
        let (mut ok, mut refused) = (0usize, 0usize);
        for count in 0..=255u8 {
            for len in 0..24usize {
                let packet: Vec<u8> = [0x92u8, count]
                    .into_iter()
                    .chain((0..len).map(|i| (i % 253) as u8))
                    .collect();
                match split_multiframe(&packet) {
                    Ok(frames) => {
                        ok += 1;
                        assert!((1..=MAX_FRAMES).contains(&frames.len()));
                        assert!(frames.iter().all(|frame| !frame.is_empty()));
                        let total: usize = frames.iter().map(|frame| frame.len()).sum();
                        assert!(total <= packet.len());
                    }
                    Err(_) => refused += 1,
                }
            }
        }
        assert!(
            ok > 0,
            "a splitter that refuses everything must not pass this test"
        );
        assert!(refused > 0, "malformed envelopes must still be refused");
    }
}
