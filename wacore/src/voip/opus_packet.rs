//! Structural read of a standard Opus packet header (RFC 6716 section 3.1), with no decoder.
//!
//! This exists because MLow and standard Opus share RTP payload type 120, and their first bytes
//! collide: `0x58` is a 120 ms MLow packet under one grammar and a 60 ms SILK wideband Opus packet
//! under the other. Issue #1105 is that collision, and no amount of staring at the byte resolves it.
//!
//! What resolves it is a second, independent statement by the same peer: the RTP timestamp step.
//! A packet whose Opus header *declares* a duration that matches the step the peer is actually
//! advancing by is Opus; one that does not is not. Both numbers come from the peer, and a peer that
//! is not sending Opus has no reason to make them agree.
//!
//! Deliberately no libopus: this has to run on wasm32 and ESP32, where the C library is not
//! available, and a structural read needs no codec anyway. It allocates nothing and, apart from
//! walking a code-3 padding run, reads a handful of bytes at the front of the packet.

/// Which Opus mode the configuration selects. Not used for the decision, but a caller logging a
/// rejected packet wants to know whether it looked like SILK or CELT.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusMode {
    /// Linear-prediction coder; what WhatsApp's standard-Opus path uses at 16 kHz.
    Silk,
    /// SILK below and CELT above the crossover, for super-wideband and fullband.
    Hybrid,
    /// Transform coder, and the only mode MLOW's in-profile escape carries.
    Celt,
}

/// The declared audio bandwidth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpusBandwidth {
    /// 4 kHz.
    Narrow,
    /// 6 kHz.
    Medium,
    /// 8 kHz. The band a 16 kHz WhatsApp audio stream uses.
    Wide,
    /// 12 kHz.
    SuperWide,
    /// 20 kHz.
    Full,
}

/// What a well-formed Opus packet header says about itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpusPacketShape {
    /// Coding mode the configuration selects.
    pub mode: OpusMode,
    /// Audio bandwidth the configuration declares.
    pub bandwidth: OpusBandwidth,
    /// Two channels rather than one.
    pub stereo: bool,
    /// Frames chained in this packet, 1..=48.
    pub frames: u8,
    /// Duration of ONE frame in microseconds. Microseconds rather than milliseconds because the
    /// CELT 2.5 ms configuration is not an integer number of milliseconds and rounding it here
    /// would make the sample count wrong.
    pub frame_duration_us: u32,
}

impl OpusPacketShape {
    /// Samples this packet claims to carry, per channel, at `clock_rate`.
    ///
    /// `None` when the product is not an exact number of samples: an RTP clock that cannot express
    /// the declared duration means the two are not describing the same stream, which is a rejection
    /// and not a rounding problem.
    #[must_use]
    pub fn total_samples(self, clock_rate: u32) -> Option<u32> {
        let total_us = self.frame_duration_us.checked_mul(u32::from(self.frames))?;
        let numerator = u64::from(total_us) * u64::from(clock_rate);
        (numerator % 1_000_000 == 0).then_some((numerator / 1_000_000) as u32)
    }
}

/// Frames per packet is capped at 48 by RFC 6716 (code 3, `M` is six bits and zero is invalid).
const MAX_FRAMES: u8 = 48;
/// RFC 6716 section 3.1: a packet may not exceed 120 ms of audio.
const MAX_PACKET_DURATION_US: u32 = 120_000;

/// Read the header of a standard Opus packet.
///
/// Returns `None` for anything that is not a structurally valid packet: a truncated code-2 length,
/// an odd body under code 1, a code-3 frame count of zero or past 48, padding that overruns, or a
/// total duration past the 120 ms the RFC allows. Being strict is the point. A permissive reader
/// would accept MLow bodies as "valid Opus" and hand the decision back to guesswork.
#[must_use]
pub fn opus_packet_shape(payload: &[u8]) -> Option<OpusPacketShape> {
    let (&toc, rest) = payload.split_first()?;
    let config = toc >> 3;
    let stereo = toc & 0x04 != 0;
    let (mode, bandwidth, frame_duration_us) = decode_config(config);

    let frames = match toc & 0x03 {
        // Code 0: one frame, the rest of the packet is its body.
        0 => 1,
        // Code 1: two equal frames, so the body has to split evenly.
        1 => {
            if rest.len() % 2 != 0 {
                return None;
            }
            2
        }
        // Code 2: two frames with an explicit length for the first. The length is one byte below
        // 252 and two above, and it has to fit inside what is left.
        2 => {
            let (&first, after) = rest.split_first()?;
            let (length, header_len) = if first < 252 {
                (u32::from(first), 1usize)
            } else {
                let (&second, _) = after.split_first()?;
                (u32::from(first) + u32::from(second) * 4, 2usize)
            };
            let available = rest.len().checked_sub(header_len)?;
            if length as usize > available {
                return None;
            }
            2
        }
        // Code 3: an explicit frame count, optionally VBR, optionally padded.
        _ => {
            let (&count_byte, after) = rest.split_first()?;
            let frames = count_byte & 0x3f;
            if frames == 0 || frames > MAX_FRAMES {
                return None;
            }
            let mut remaining = after;
            // Padding is a run of 255s ended by a final byte. Each 255 contributes 254 bytes and
            // costs another length byte, so the total ACCUMULATES; checking each byte against what
            // is left, without adding them up, accepts a packet declaring far more padding than the
            // packet contains. `split_multiframe` gets this right and this used to not, which is
            // how two Opus padding readers in one change came to disagree.
            let mut padding = 0usize;
            if count_byte & 0x40 != 0 {
                loop {
                    let (&pad, next) = remaining.split_first()?;
                    remaining = next;
                    if pad == 255 {
                        padding = padding.checked_add(254)?;
                    } else {
                        padding = padding.checked_add(usize::from(pad))?;
                        break;
                    }
                }
            }
            let bodies = remaining.len().checked_sub(padding)?;
            if count_byte & 0x80 == 0 {
                // CBR (the VBR bit clear) means every frame is the same size, so RFC 6716
                // section 3.2.5 requires the remaining bytes to divide evenly by the frame count.
                // Without this the reader accepts packets libopus refuses, and each one it accepts
                // is extra surface for the codec probe to mistake for the codec we are looking for.
                if !bodies.is_multiple_of(usize::from(frames)) {
                    return None;
                }
            } else {
                // VBR carries `M-1` explicit lengths and leaves the last frame implicit. Skipping
                // them entirely admits a two-byte packet as three 20 ms frames, and three of those
                // at the negotiated cadence are exactly what the codec probe accepts as proof of a
                // standard-Opus peer -- so a reader that does not walk them can flip the call's
                // codec on bytes that describe no audio at all.
                let mut declared = 0usize;
                let mut header = 0usize;
                let mut lengths = remaining.get(..bodies)?;
                for _ in 1..frames {
                    let (&first, after) = lengths.split_first()?;
                    let (length, width) = if first < 252 {
                        (usize::from(first), 1usize)
                    } else {
                        let (&second, _) = after.split_first()?;
                        (usize::from(first) + usize::from(second) * 4, 2usize)
                    };
                    lengths = lengths.get(width..)?;
                    declared = declared.checked_add(length)?;
                    header = header.checked_add(width)?;
                }
                // The implicit last frame takes what is left, so the declared ones plus their own
                // length fields must still leave a non-negative remainder.
                header
                    .checked_add(declared)
                    .filter(|used| *used <= bodies)?;
            }
            frames
        }
    };

    let shape = OpusPacketShape {
        mode,
        bandwidth,
        stereo,
        frames,
        frame_duration_us,
    };
    let total_us = frame_duration_us.checked_mul(u32::from(frames))?;
    (total_us <= MAX_PACKET_DURATION_US).then_some(shape)
}

/// RFC 6716 table 2: the 32 configurations, as (mode, bandwidth, frame duration).
const fn decode_config(config: u8) -> (OpusMode, OpusBandwidth, u32) {
    // SILK durations are 10/20/40/60 ms; CELT are 2.5/5/10/20 ms. Both indexed by the low two bits.
    const SILK_US: [u32; 4] = [10_000, 20_000, 40_000, 60_000];
    const CELT_US: [u32; 4] = [2_500, 5_000, 10_000, 20_000];
    let low = (config & 0x03) as usize;
    match config {
        0..=3 => (OpusMode::Silk, OpusBandwidth::Narrow, SILK_US[low]),
        4..=7 => (OpusMode::Silk, OpusBandwidth::Medium, SILK_US[low]),
        8..=11 => (OpusMode::Silk, OpusBandwidth::Wide, SILK_US[low]),
        // Hybrid carries only 10 and 20 ms, selected by the low bit.
        12..=13 => (
            OpusMode::Hybrid,
            OpusBandwidth::SuperWide,
            if config & 1 == 0 { 10_000 } else { 20_000 },
        ),
        14..=15 => (
            OpusMode::Hybrid,
            OpusBandwidth::Full,
            if config & 1 == 0 { 10_000 } else { 20_000 },
        ),
        16..=19 => (OpusMode::Celt, OpusBandwidth::Narrow, CELT_US[low]),
        20..=23 => (OpusMode::Celt, OpusBandwidth::Wide, CELT_US[low]),
        24..=27 => (OpusMode::Celt, OpusBandwidth::SuperWide, CELT_US[low]),
        _ => (OpusMode::Celt, OpusBandwidth::Full, CELT_US[low]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte at the centre of issue #1105.
    const DESKTOP_TOC: u8 = 0x58;

    #[test]
    fn the_toc_behind_the_bug_reads_as_silk_wideband_sixty_milliseconds() {
        let packet: Vec<u8> = core::iter::once(DESKTOP_TOC).chain(0..80u8).collect();
        let shape = opus_packet_shape(&packet).expect("a valid code-0 packet");
        assert_eq!(shape.mode, OpusMode::Silk);
        assert_eq!(shape.bandwidth, OpusBandwidth::Wide);
        assert!(!shape.stereo);
        assert_eq!(shape.frames, 1);
        assert_eq!(shape.frame_duration_us, 60_000);
        // 60 ms at 16 kHz is 960 samples, which is exactly the RTP timestamp step a WhatsApp audio
        // stream advances by. That agreement is the whole discriminator.
        assert_eq!(shape.total_samples(16_000), Some(960));
    }

    // The proof that the cross-check cannot promote a packet the MLow decoder is decoding today.
    //
    // A frame MLow currently accepts has bits 4:3 = 10 (60 ms), bit 5 = 0 (16 kHz), bit 2 = 0
    // (low_rate) and bit 7 = 0 (not SID), which is exactly {0x10..=0x13, 0x50..=0x53}. Read as an
    // Opus TOC each of those is config 2 or 10, both 40 ms, so every reachable duration is a
    // multiple of 640 samples at 16 kHz. The stream's timestamp step is 960, and 960 is not a
    // multiple of 640, so no body of any length can make the two agree.
    #[test]
    fn no_packet_the_mlow_decoder_accepts_can_be_mistaken_for_a_960_sample_opus_packet() {
        for toc in [0x10u8, 0x11, 0x12, 0x13, 0x50, 0x51, 0x52, 0x53] {
            for len in 1..400usize {
                let packet: Vec<u8> = core::iter::once(toc)
                    .chain((0..len).map(|i| (i % 251) as u8))
                    .collect();
                if let Some(shape) = opus_packet_shape(&packet) {
                    assert_ne!(
                        shape.total_samples(16_000),
                        Some(960),
                        "TOC {toc:#04x} with a {len}-byte body must never claim 960 samples"
                    );
                }
            }
        }
    }

    #[test]
    fn code_one_requires_an_even_body() {
        let odd: Vec<u8> = core::iter::once(0x59u8).chain(0..7u8).collect();
        assert_eq!(opus_packet_shape(&odd), None);
        let even: Vec<u8> = core::iter::once(0x59u8).chain(0..8u8).collect();
        let shape = opus_packet_shape(&even).expect("an even body splits");
        assert_eq!(shape.frames, 2);
        // Two 60 ms frames is 120 ms, the RFC ceiling, so it is accepted but nothing longer is.
        assert_eq!(shape.total_samples(16_000), Some(1920));
    }

    #[test]
    fn a_packet_longer_than_the_rfc_ceiling_is_rejected() {
        // Code 3, three 60 ms frames: 180 ms, past the 120 ms limit.
        let packet: Vec<u8> = [0x5bu8, 0x03].into_iter().chain(0..30u8).collect();
        assert_eq!(opus_packet_shape(&packet), None);
    }

    #[test]
    fn a_code_three_frame_count_must_be_within_range() {
        for count in [0u8, 49, 63] {
            let packet: Vec<u8> = [0x0bu8, count].into_iter().chain(0..30u8).collect();
            assert_eq!(
                opus_packet_shape(&packet),
                None,
                "frame count {count} is out of range"
            );
        }
        // Config 1 is SILK narrowband 20 ms, so six frames is 120 ms and legal.
        let packet: Vec<u8> = [0x0bu8, 0x06].into_iter().chain(0..30u8).collect();
        assert_eq!(
            opus_packet_shape(&packet).map(|shape| shape.frames),
            Some(6)
        );
    }

    #[test]
    fn a_truncated_code_two_length_is_rejected() {
        // Declares a 200-byte first frame inside a 4-byte body.
        let packet = [0x5au8, 200, 1, 2];
        assert_eq!(opus_packet_shape(&packet), None);
        // A length that fits is accepted.
        let packet: Vec<u8> = [0x5au8, 3].into_iter().chain(0..8u8).collect();
        assert_eq!(
            opus_packet_shape(&packet).map(|shape| shape.frames),
            Some(2)
        );
    }

    #[test]
    fn a_two_byte_code_two_length_needs_both_bytes() {
        assert_eq!(opus_packet_shape(&[0x5au8, 252]), None);
    }

    #[test]
    fn padding_that_overruns_the_packet_is_rejected() {
        // Code 3 with the padding bit set and a padding length past the end.
        let packet = [0x0bu8, 0x41, 200, 1, 2];
        assert_eq!(opus_packet_shape(&packet), None);
    }

    // Each 255 in a padding run is worth 254 bytes and costs another length byte, so the run has to
    // be summed. Checking each byte on its own accepts a packet claiming far more padding than it
    // carries -- and libopus rejects exactly that packet.
    #[test]
    fn a_padding_run_accumulates_rather_than_being_checked_byte_by_byte() {
        // Three 255s plus a 3: 3*254 + 3 = 765 bytes of padding declared inside a 300-byte body.
        let packet: Vec<u8> = [0x0bu8, 0x43, 255, 255, 255, 3]
            .into_iter()
            .chain((0..300u32).map(|i| i as u8))
            .collect();
        assert_eq!(opus_packet_shape(&packet), None);
    }

    // CBR means every frame is the same size, so the remaining bytes must divide by the frame
    // count. Accepting a packet that does not is extra surface for the codec probe to trip over.
    #[test]
    fn a_constant_bitrate_packet_whose_body_does_not_divide_is_rejected() {
        // Code 3, VBR clear, three frames, ten body bytes.
        let packet: Vec<u8> = [0x0bu8, 0x03].into_iter().chain(0..10u8).collect();
        assert_eq!(opus_packet_shape(&packet), None);
        // Nine divides by three.
        let packet: Vec<u8> = [0x0bu8, 0x03].into_iter().chain(0..9u8).collect();
        assert_eq!(
            opus_packet_shape(&packet).map(|shape| shape.frames),
            Some(3)
        );
        // VBR set: the sizes are explicit, so divisibility says nothing.
        let packet: Vec<u8> = [0x0bu8, 0x83].into_iter().chain(0..10u8).collect();
        assert_eq!(
            opus_packet_shape(&packet).map(|shape| shape.frames),
            Some(3)
        );
    }

    // The VBR lengths are not decoration: three packets that pass this reader at the negotiated
    // cadence are what the codec probe accepts as proof of a standard-Opus peer, so a packet whose
    // declared frames are not in it must not read as a shape at all.
    #[test]
    fn a_variable_bitrate_packet_whose_frames_are_not_there_is_rejected() {
        // Code 3, VBR, three frames -- and nothing after the count byte. Two length fields and
        // three frame bodies are all missing.
        assert_eq!(opus_packet_shape(&[0x4b, 0x83]), None);
        // The two lengths are present but describe more than the body holds: 200 + 200 in 100 bytes.
        let packet: Vec<u8> = [0x4bu8, 0x83, 200, 200]
            .into_iter()
            .chain((0..100u32).map(|i| i as u8))
            .collect();
        assert_eq!(opus_packet_shape(&packet), None);
        // The same lengths inside a body that holds them, plus an implicit last frame.
        let packet: Vec<u8> = [0x4bu8, 0x83, 200, 200]
            .into_iter()
            .chain((0..450u32).map(|i| i as u8))
            .collect();
        assert_eq!(
            opus_packet_shape(&packet).map(|shape| shape.frames),
            Some(3)
        );
        // A two-byte length that runs off the end takes the packet with it.
        assert_eq!(opus_packet_shape(&[0x4b, 0x82, 252]), None);
    }

    #[test]
    fn an_empty_payload_has_no_shape() {
        assert_eq!(opus_packet_shape(&[]), None);
    }

    // A clock that cannot express the declared duration means the two are not describing the same
    // stream. Rejecting beats rounding, which would invent an agreement that is not there.
    #[test]
    fn a_duration_the_clock_cannot_express_yields_no_sample_count() {
        // CELT 2.5 ms at 8 kHz is 20 samples; at 300 Hz it is not an integer.
        let shape = OpusPacketShape {
            mode: OpusMode::Celt,
            bandwidth: OpusBandwidth::Full,
            stereo: false,
            frames: 1,
            frame_duration_us: 2_500,
        };
        assert_eq!(shape.total_samples(8_000), Some(20));
        assert_eq!(shape.total_samples(300), None);
    }

    #[test]
    fn every_toc_byte_either_parses_or_is_refused_without_panicking() {
        for toc in 0..=255u8 {
            for len in 0..12usize {
                let packet: Vec<u8> = core::iter::once(toc)
                    .chain((0..len).map(|i| i as u8))
                    .collect();
                if let Some(shape) = opus_packet_shape(&packet) {
                    assert!((1..=MAX_FRAMES).contains(&shape.frames));
                    let total = shape.frame_duration_us * u32::from(shape.frames);
                    assert!(total <= MAX_PACKET_DURATION_US);
                }
            }
        }
    }
}
