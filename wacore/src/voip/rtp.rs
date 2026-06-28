//! RTP WARP framing: WhatsApp's 16-byte speech / 20-byte DTX headers (extension
//! profile 0xdebe), Opus payload classifiers, and the send-side sequencer.

use crate::voip::warp::audio_piggyback_extension_for;

pub const RTP_PAYLOAD_TYPE_OPUS: u8 = 120;
pub const WHATSAPP_RTP_EXTENSION_PROFILE: u16 = 0xdebe;
/// RFC 3550 fixed RTP header length, before any CSRC list or extension block.
pub const RTP_FIXED_HEADER_LEN: usize = 12;
pub const WHATSAPP_RTP_HEADER_SIZE: usize = 16;
pub const WHATSAPP_RTP_HEADER_DTX_SIZE: usize = 20;
pub const WHATSAPP_RTP_EXTENSION_DTX_WORD: u32 = 0x3001_0000;
const RTP_VERSION: u8 = 2;
const SRTP_AUTH_TAG_LEN: usize = 10;
const SRTP_AUTH_TAG_LEN_SHORT: usize = 4;

/// Android first priming frame (18 bytes).
pub const OPUS_PRIMING_FRAME_1: [u8; 18] = [
    0x12, 0x36, 0x26, 0x2b, 0x4a, 0xc8, 0x2b, 0x09, 0xc9, 0x1f, 0x34, 0xc2, 0xd6, 0x7a, 0x01, 0x73,
    0x1b, 0x2e,
];
/// WASM/Web caller priming (24 bytes).
pub const OPUS_PRIMING_FRAME_1_WASM: [u8; 24] = [
    0x32, 0x36, 0x26, 0x2b, 0x4a, 0xcb, 0x1b, 0x5f, 0xba, 0x91, 0x68, 0x7e, 0xb8, 0x50, 0x93, 0x58,
    0xe6, 0xd0, 0xa3, 0xa9, 0xd7, 0x1d, 0x81, 0x8c,
];
/// Second priming frame (5 bytes).
pub const OPUS_PRIMING_FRAME_2: [u8; 5] = [0x90, 0xb8, 0x14, 0x14, 0xc4];

pub fn is_whatsapp_opus_rtp_payload(payload_type: u8) -> bool {
    payload_type == RTP_PAYLOAD_TYPE_OPUS || payload_type == 121
}

/// DTX / comfort-noise: RFC `0x10`, mlow `0x90`, and short warmup silence frames.
pub fn is_opus_dtx_payload(payload: &[u8]) -> bool {
    match payload.len() {
        0 => false,
        1 => matches!(payload[0], 0x10 | 0x88 | 0x90),
        n if n <= 15 => {
            let b0 = payload[0];
            if (b0 & 0xf8) == 0x08 || b0 == 0x0a {
                return true;
            }
            (b0 & 0xf0) == 0x30 && n <= 6
        }
        _ => false,
    }
}

/// mlow speech frame (20 ms `0x48..0x4f` or 60 ms `0x50..0x57`).
pub fn is_opus_mlow_speech_payload(payload: &[u8]) -> bool {
    if payload.len() < 18 {
        return false;
    }
    let b0 = payload[0];
    (b0 & 0xf8) == 0x48 || (b0 & 0xf8) == 0x50
}

pub fn is_opus_priming_payload(payload: &[u8]) -> bool {
    payload == OPUS_PRIMING_FRAME_1 || payload == OPUS_PRIMING_FRAME_2
}

/// Estimate on-wire SRTP size (header + opus + auth tag) for ladder frame picking.
pub fn estimate_srtp_rtp_wire_bytes(opus_payload: &[u8]) -> usize {
    let dtx = is_opus_dtx_payload(opus_payload);
    let header_size = if dtx {
        WHATSAPP_RTP_HEADER_DTX_SIZE
    } else {
        WHATSAPP_RTP_HEADER_SIZE
    };
    // header_size is itself derived from dtx, so the header_size comparisons are tautological:
    // short tag on DTX, on a priming frame, or on any <=18-byte (silence/short) payload.
    let short_tag = dtx || is_opus_priming_payload(opus_payload) || opus_payload.len() <= 18;
    let tag_len = if short_tag {
        SRTP_AUTH_TAG_LEN_SHORT
    } else {
        SRTP_AUTH_TAG_LEN
    };
    header_size + opus_payload.len() + tag_len
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RtpHeader {
    pub marker: bool,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    /// When set, the header carries one 0xdebe extension word (DTX/piggyback).
    pub extension_word: Option<u32>,
}

impl RtpHeader {
    pub fn byte_size(&self) -> usize {
        if self.extension_word.is_some() {
            WHATSAPP_RTP_HEADER_DTX_SIZE
        } else {
            WHATSAPP_RTP_HEADER_SIZE
        }
    }
}

/// Full on-wire RTP header size (fixed 12 + CSRC + optional extension block), or `None`.
pub fn rtp_header_byte_length(data: &[u8]) -> Option<usize> {
    if data.len() < RTP_FIXED_HEADER_LEN {
        return None;
    }
    if (data[0] >> 6) & 0x03 != RTP_VERSION {
        return None;
    }
    let cc = (data[0] & 0x0f) as usize;
    let mut header_len = RTP_FIXED_HEADER_LEN + cc * 4;
    if data.len() < header_len {
        return None;
    }
    let has_extension = (data[0] >> 4) & 1 == 1;
    if has_extension {
        if data.len() < header_len + 4 {
            return None;
        }
        let ext_words = ((data[header_len + 2] as usize) << 8) | data[header_len + 3] as usize;
        header_len += 4 + ext_words * 4;
        if data.len() < header_len {
            return None;
        }
    }
    Some(header_len)
}

pub fn is_rtp_version2(data: &[u8]) -> bool {
    data.len() >= RTP_FIXED_HEADER_LEN && (data[0] >> 6) & 0x03 == RTP_VERSION
}

/// Parse the fixed RTP header fields (the extension word is not decoded).
pub fn parse_rtp_header(data: &[u8]) -> Option<RtpHeader> {
    rtp_header_byte_length(data)?;
    Some(RtpHeader {
        marker: (data[1] >> 7) & 1 == 1,
        payload_type: data[1] & 0x7f,
        sequence_number: ((data[2] as u16) << 8) | data[3] as u16,
        timestamp: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
        ssrc: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
        extension_word: None,
    })
}

pub fn encode_rtp_header(header: &RtpHeader) -> Vec<u8> {
    let size = header.byte_size();
    let mut buf = vec![0u8; size];
    buf[0] = RTP_VERSION << 6;
    if size > RTP_FIXED_HEADER_LEN {
        buf[0] |= 0x10; // X=1 (WhatsApp 0xdebe extension)
    }
    buf[1] = ((header.marker as u8) << 7) | (header.payload_type & 0x7f);
    buf[2] = (header.sequence_number >> 8) as u8;
    buf[3] = header.sequence_number as u8;
    buf[4..8].copy_from_slice(&header.timestamp.to_be_bytes());
    buf[8..12].copy_from_slice(&header.ssrc.to_be_bytes());
    if size >= 16 {
        buf[12] = (WHATSAPP_RTP_EXTENSION_PROFILE >> 8) as u8;
        buf[13] = WHATSAPP_RTP_EXTENSION_PROFILE as u8;
        // Extension length in 32-bit words, 0 or 1, so the high byte (buf[14]) is always 0 (already
        // zero-init); only the low byte varies.
        buf[15] = header.extension_word.is_some() as u8;
    }
    if size >= 20
        && let Some(w) = header.extension_word
    {
        buf[16..20].copy_from_slice(&w.to_be_bytes());
    }
    buf
}

/// Send-side RTP sequencer: seq starts at 1, timestamp advances by `samples_per_packet`.
pub struct RtpStream {
    pub ssrc: u32,
    seq: u16,
    timestamp: u32,
    samples_per_packet: u32,
    speech_started: bool,
    audio_packet_index: usize,
    warp_piggyback: bool,
}

impl RtpStream {
    pub fn new(ssrc: u32, samples_per_packet: u32, warp_piggyback: bool) -> Self {
        Self {
            ssrc,
            seq: 1,
            timestamp: 0,
            samples_per_packet,
            speech_started: false,
            audio_packet_index: 0,
            warp_piggyback,
        }
    }

    fn resolve_warp_extension(&mut self, dtx: bool) -> Option<u32> {
        if dtx {
            return Some(WHATSAPP_RTP_EXTENSION_DTX_WORD);
        }
        if !self.warp_piggyback {
            return None;
        }
        let idx = self.audio_packet_index;
        self.audio_packet_index += 1;
        audio_piggyback_extension_for(idx, true, crate::voip::warp::WARP_PIGGYBACK_START_PACKET)
    }

    pub fn next_packet(&mut self, payload: &[u8], marker: bool) -> RtpHeader {
        let dtx = is_opus_dtx_payload(payload);
        let priming = is_opus_priming_payload(payload);
        let speech = !dtx && !priming;
        let use_marker = marker || (speech && !self.speech_started);
        if speech {
            self.speech_started = true;
        }
        let header = RtpHeader {
            marker: use_marker,
            payload_type: RTP_PAYLOAD_TYPE_OPUS,
            sequence_number: self.seq,
            timestamp: self.timestamp,
            ssrc: self.ssrc,
            extension_word: self.resolve_warp_extension(dtx),
        };
        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(self.samples_per_packet);
        header
    }

    /// Pre-speech ladder packet: advances seq/timestamp without a marker or speech latch.
    pub fn next_pre_speech_packet(&mut self) -> RtpHeader {
        let header = RtpHeader {
            marker: false,
            payload_type: RTP_PAYLOAD_TYPE_OPUS,
            sequence_number: self.seq,
            timestamp: self.timestamp,
            ssrc: self.ssrc,
            extension_word: self.resolve_warp_extension(false),
        };
        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(self.samples_per_packet);
        header
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::kats;

    #[test]
    fn encode_headers_match_kat() {
        let k = kats();
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        let speech = RtpHeader {
            marker: true,
            payload_type: 120,
            sequence_number: 1,
            timestamp: 0,
            ssrc,
            extension_word: None,
        };
        assert_eq!(
            hex::encode(encode_rtp_header(&speech)),
            k["rtp"]["speechHeader16"].as_str().unwrap()
        );
        let dtx = RtpHeader {
            marker: false,
            payload_type: 120,
            sequence_number: 2,
            timestamp: 320,
            ssrc,
            extension_word: Some(0x3001_0000),
        };
        assert_eq!(
            hex::encode(encode_rtp_header(&dtx)),
            k["rtp"]["dtxHeader20"].as_str().unwrap()
        );
    }

    #[test]
    fn parse_round_trips_fixed_fields() {
        let h = RtpHeader {
            marker: true,
            payload_type: 120,
            sequence_number: 0x1234,
            timestamp: 0xdead_beef,
            ssrc: 0x0102_0304,
            extension_word: None,
        };
        let bytes = encode_rtp_header(&h);
        assert_eq!(rtp_header_byte_length(&bytes), Some(16));
        assert_eq!(parse_rtp_header(&bytes), Some(h));
    }

    #[test]
    fn estimate_wire_bytes_match_kat() {
        let k = kats();
        let payload = hex::decode(k["inputs"]["payload"].as_str().unwrap()).unwrap();
        assert_eq!(
            estimate_srtp_rtp_wire_bytes(&payload) as u64,
            k["rtp"]["estimateSpeech12"].as_u64().unwrap()
        );
        assert_eq!(
            estimate_srtp_rtp_wire_bytes(&[0x10]) as u64,
            k["rtp"]["estimateDtx1"].as_u64().unwrap()
        );
        assert_eq!(
            estimate_srtp_rtp_wire_bytes(&OPUS_PRIMING_FRAME_2) as u64,
            k["rtp"]["estimatePriming2"].as_u64().unwrap()
        );
    }

    #[test]
    fn classifiers() {
        assert!(is_opus_dtx_payload(&[0x10]));
        assert!(is_opus_dtx_payload(&[0x90]));
        assert!(!is_opus_dtx_payload(&[]));
        assert!(is_opus_priming_payload(&OPUS_PRIMING_FRAME_1));
        assert!(is_opus_priming_payload(&OPUS_PRIMING_FRAME_2));
        assert!(!is_opus_priming_payload(&[0x12, 0x36]));
        assert!(is_opus_mlow_speech_payload(&[0x48; 20]));
        assert!(!is_opus_mlow_speech_payload(&[0x48; 4]));
        assert!(is_whatsapp_opus_rtp_payload(120));
        assert!(is_whatsapp_opus_rtp_payload(121));
        assert!(!is_whatsapp_opus_rtp_payload(96));
    }

    #[test]
    fn stream_sequence_and_marker() {
        let mut s = RtpStream::new(0xabcd, 320, false);
        // Priming/DTX before speech: no marker, no speech latch.
        let p0 = s.next_packet(&OPUS_PRIMING_FRAME_2, false);
        assert_eq!((p0.sequence_number, p0.timestamp, p0.marker), (1, 0, false));
        let d1 = s.next_packet(&[0x10], false);
        assert_eq!(
            (d1.sequence_number, d1.timestamp, d1.marker),
            (2, 320, false)
        );
        assert_eq!(d1.extension_word, Some(WHATSAPP_RTP_EXTENSION_DTX_WORD));
        // First speech frame latches the marker.
        let sp = s.next_packet(&[0x48; 40], false);
        assert_eq!(
            (sp.sequence_number, sp.timestamp, sp.marker),
            (3, 640, true)
        );
        // Subsequent speech has no marker.
        let sp2 = s.next_packet(&[0x48; 40], false);
        assert!(!sp2.marker);
    }
}
