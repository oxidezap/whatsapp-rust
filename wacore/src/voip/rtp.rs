//! RTP WARP framing: WhatsApp's 16-byte speech / 20-byte DTX headers (extension
//! profile 0xdebe), Opus payload classifiers, and the send-side sequencer.

use crate::voip::warp::audio_piggyback_extension_for;

/// RFC 7587 Opus payload type used when the 48 kHz RTP-clock setting is enabled.
pub const RTP_PAYLOAD_TYPE_OPUS: u8 = 111;
/// Default WhatsApp audio payload type; the negotiated codec decides whether its bytes are MLOW or
/// native Opus.
pub const RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO: u8 = 120;
/// MLOW name for the shared default WhatsApp audio payload type.
pub const RTP_PAYLOAD_TYPE_MLOW: u8 = RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO;
/// MLOW SplitRed redundancy payload type (`mlow-red-1`).
pub const RTP_PAYLOAD_TYPE_MLOW_RED: u8 = 121;
/// MLOW on a payload type of its own (`mlow-1`); see the contract's definition.
pub use crate::voip_control::audio_format::RTP_PAYLOAD_TYPE_MLOW_SEPARATE;
/// H.264 video payload type used by captured Android video RTP.
pub const RTP_PAYLOAD_TYPE_H264: u8 = 97;
/// RTC app-data stream carrying reactions and future in-call controls.
pub const RTP_PAYLOAD_TYPE_APP_DATA: u8 = 119;
/// Video RTP timestamp clock.
pub const VIDEO_CLOCK_RATE: u32 = 90_000;
/// Timestamp stride per access unit at the reference 15 fps (90000 / 15).
pub const VIDEO_TS_STRIDE_15FPS: u32 = VIDEO_CLOCK_RATE / 15;
/// Timestamp stride used by WhatsApp's 720p / 20 fps mode.
pub const VIDEO_TS_STRIDE_20FPS: u32 = VIDEO_CLOCK_RATE / 20;
pub const WHATSAPP_RTP_EXTENSION_PROFILE: u16 = 0xdebe;

/// RFC 3550 fixed RTP header length, before any CSRC list or extension block.
pub const RTP_FIXED_HEADER_LEN: usize = 12;
pub const WHATSAPP_RTP_HEADER_SIZE: usize = 16;
pub const WHATSAPP_RTP_HEADER_DTX_SIZE: usize = 20;
pub const WHATSAPP_VIDEO_RTP_HEADER_SIZE: usize = 28;
/// WhatsApp keyframe flag for an upright IDR access unit. The low two bits are
/// rotation, not presence. Captured WASM uses internal bit 0x800 for presence;
/// its one-byte RTP extension omits that bit. See the oracle's video_orientation test.
pub const VIDEO_MEDIA_FRAME_INFO_IDR: u8 = 0x08;
/// Upright dependent H.264 access unit, with no keyframe or rotation bits.
pub const VIDEO_MEDIA_FRAME_INFO_DELTA: u8 = 0x00;
pub const WHATSAPP_RTP_EXTENSION_DTX_WORD: u32 = 0x3001_0000;
const RTP_VERSION: u8 = 2;
const SRTP_AUTH_TAG_LEN: usize = 10;
const SRTP_AUTH_TAG_LEN_SHORT: usize = 4;

/// The four one-byte-header extensions emitted by WhatsApp's video sender.
/// IDs and ordering are pinned by the Android packet captured in
/// `video_header_matches_android_capture` and the corresponding WASM extenders.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VideoRtpExtension {
    pub media_frame_info: u8,
    pub initial_bandwidth: u16,
    pub short_offset: i16,
    pub transport_sequence: u16,
}

impl VideoRtpExtension {
    fn encode_into(self, out: &mut [u8; 12]) {
        let initial_bandwidth = self.initial_bandwidth.to_be_bytes();
        let short_offset = self.short_offset.to_be_bytes();
        let transport_sequence = self.transport_sequence.to_be_bytes();
        *out = [
            0x30,
            self.media_frame_info,
            0x51,
            initial_bandwidth[0],
            initial_bandwidth[1],
            0x61,
            short_offset[0],
            short_offset[1],
            0x91,
            transport_sequence[0],
            transport_sequence[1],
            0,
        ];
    }
}

/// The RTP timestamp `elapsed_ms` after the instant `timestamp` stands for, on a `clock_rate` Hz
/// media clock, for a Sender Report that pairs a stream's media clock with the wall clock.
pub(crate) fn rtp_timestamp_after(timestamp: u32, elapsed_ms: u64, clock_rate: u32) -> u32 {
    let ticks = u128::from(elapsed_ms) * u128::from(clock_rate) / 1000;
    timestamp.wrapping_add(ticks as u32)
}

/// Byte range of the `short_offset` value inside an encoded video header: element id 6 (`0x61`,
/// two bytes) is the third of [`VideoRtpExtension::encode_into`]'s elements, after the 12-byte
/// fixed header, the 4-byte extension header, and four bytes of the first two elements.
const VIDEO_SHORT_OFFSET_RANGE: core::ops::Range<usize> = 22..24;

/// The transmission offset for a packet leaving `elapsed_ms` after its picture was captured.
///
/// Extension id 6 is a millisecond offset from the RTP timestamp, the picture's capture, to the
/// packet's transmission. The Android keyframe and delta packets the tests pin carry 29 and 36; an
/// iPhone sends 10 to 23 on ordinary frames, equal or one higher from one packet of a
/// frame to the next, as stamping each packet when it leaves gives. Clamped to the field's
/// non-negative range, so a frame held for over 32 s saturates.
pub fn video_short_offset(elapsed_ms: u64) -> i16 {
    i16::try_from(elapsed_ms).unwrap_or(i16::MAX)
}

/// Where an encoded video packet's `short_offset` value sits, if the packet has the layout
/// [`encode_rtp_header_into`] writes for video: no CSRCs, the WhatsApp extension profile three
/// words long, and element id 6 in its place.
pub(crate) fn video_short_offset_range(packet: &[u8]) -> Option<core::ops::Range<usize>> {
    let header = packet.get(..WHATSAPP_VIDEO_RTP_HEADER_SIZE)?;
    let layout_matches = header[0] & 0x1f == 0x10
        && header[12..16] == [0xde, 0xbe, 0x00, 0x03]
        && header[VIDEO_SHORT_OFFSET_RANGE.start - 1] == 0x61;
    layout_matches.then_some(VIDEO_SHORT_OFFSET_RANGE)
}

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
    matches!(
        payload_type,
        RTP_PAYLOAD_TYPE_OPUS
            | RTP_PAYLOAD_TYPE_MLOW
            | RTP_PAYLOAD_TYPE_MLOW_RED
            | RTP_PAYLOAD_TYPE_MLOW_SEPARATE
    )
}

/// Standard Opus DTX / comfort-noise and short warmup silence frames.
pub fn is_opus_dtx_payload(payload: &[u8]) -> bool {
    match payload.len() {
        0 => false,
        // libopus defines packets up to two bytes as DTX. This also covers MLOW's one-byte SID.
        1..=2 => true,
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

fn is_mlow_dtx_payload(payload: &[u8]) -> bool {
    payload.first().is_some_and(|byte| byte & 0xC0 == 0x80)
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
    /// WhatsApp video metadata, encoded as the captured 12-byte 0xdebe block.
    pub video_extension: Option<VideoRtpExtension>,
}

impl RtpHeader {
    pub fn byte_size(&self) -> usize {
        if self.video_extension.is_some() {
            WHATSAPP_VIDEO_RTP_HEADER_SIZE
        } else if self.extension_word.is_some() {
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

/// Extension profile and its word-aligned payload. A valid RTP packet without X returns
/// `(None, &[])` so callers can distinguish it from malformed input.
pub fn rtp_extension_profile_and_data(data: &[u8]) -> Option<(Option<u16>, &[u8])> {
    if data.len() < RTP_FIXED_HEADER_LEN || (data[0] >> 6) & 0x03 != RTP_VERSION {
        return None;
    }
    let extension_offset = RTP_FIXED_HEADER_LEN + (data[0] & 0x0f) as usize * 4;
    if data.len() < extension_offset {
        return None;
    }
    if data[0] & 0x10 == 0 {
        return Some((None, &[]));
    }
    if data.len() < extension_offset + 4 {
        return None;
    }
    let words =
        u16::from_be_bytes([data[extension_offset + 2], data[extension_offset + 3]]) as usize;
    let data_start = extension_offset + 4;
    let data_end = data_start.checked_add(words.checked_mul(4)?)?;
    Some((
        Some(u16::from_be_bytes([
            data[extension_offset],
            data[extension_offset + 1],
        ])),
        data.get(data_start..data_end)?,
    ))
}

/// Read the frame-info byte independently of the other 0xdebe extensions.
/// Callers must authenticate the packet and establish its media type before using it.
/// A truncated element ends the scan without discarding earlier complete metadata.
pub fn parse_whatsapp_media_frame_info(data: &[u8]) -> Option<u8> {
    let (Some(WHATSAPP_RTP_EXTENSION_PROFILE), mut extension) =
        rtp_extension_profile_and_data(data)?
    else {
        return None;
    };
    let mut info = [None; 3];
    while let Some((&header, rest)) = extension.split_first() {
        extension = rest;
        let id = header >> 4;
        if id == 0 {
            continue;
        }
        let len = usize::from(header & 0x0f) + 1;
        let Some((value, rest)) = extension.split_at_checked(len) else {
            break;
        };
        extension = rest;
        if id == 3 {
            // Captured rtp_ext readers keep three formats separately; the getter
            // prefers short, then three-byte, then extended metadata, regardless of order.
            let format = match len {
                1..=2 => 0,
                3..=5 => 1,
                _ => 2,
            };
            info[format] = Some(value[0]);
        }
    }
    info.into_iter().flatten().next()
}

/// Read the media time offset (one-byte extension element 6, `short_offset` in
/// [`VideoRtpExtension`]) independently of the other 0xdebe extensions: a signed
/// millisecond offset from the RTP timestamp. Packets of one picture carry the
/// same value, or one 1 ms later; see [`super::h264::H264Depacketizer::push_with_time_offset`]
/// for a picture told apart by it. A truncated element ends the scan.
pub fn parse_whatsapp_media_time_offset(data: &[u8]) -> Option<i16> {
    let (Some(WHATSAPP_RTP_EXTENSION_PROFILE), mut extension) =
        rtp_extension_profile_and_data(data)?
    else {
        return None;
    };
    while let Some((&header, rest)) = extension.split_first() {
        extension = rest;
        let id = header >> 4;
        if id == 0 {
            continue;
        }
        let len = usize::from(header & 0x0f) + 1;
        let (value, rest) = extension.split_at_checked(len)?;
        extension = rest;
        if id == 6 && len == 2 {
            return Some(i16::from_be_bytes([value[0], value[1]]));
        }
    }
    None
}

/// RFC 8285's one-byte header extension profile. WhatsApp's own stack registers its one-byte
/// element parser under this profile as well as under [`WHATSAPP_RTP_EXTENSION_PROFILE`].
pub const RFC8285_ONE_BYTE_EXTENSION_PROFILE: u16 = 0xbede;

/// One-byte extension element of WhatsApp's audio piggyback (`AP` in its extender table).
///
/// Two bytes, big-endian: how many bytes at the start of the RTP payload are the packet's own
/// media. Whatever follows them is a complete audio RTP packet -- header, payload, no tag -- that
/// the sender's audio stream handed to this packet instead of sending it on its own. A WhatsApp
/// client does this in video calls: its video encoder takes the next queued audio packet onto each
/// video and video-FEC packet, and only audio that finds no carrier within a timeout goes out alone.
/// The receiver splits it back off after the outer packet authenticates and decrypts, which is why
/// the audio packet inside carries no SRTP of its own.
pub const WHATSAPP_RTP_EXTENSION_AUDIO_PIGGYBACK_ID: u8 = 7;

/// The payload offset at which a piggybacked audio packet starts, as the packet's header states it.
///
/// `None` when the packet has no audio piggyback element. The header is in the clear, so this can
/// be read before decryption, but the offset only means something for an authenticated packet.
/// A truncated element ends the scan, and the reserved id 15 stops it as RFC 8285 says.
pub fn parse_whatsapp_audio_piggyback_offset(data: &[u8]) -> Option<usize> {
    let (Some(profile), mut extension) = rtp_extension_profile_and_data(data)? else {
        return None;
    };
    if profile != WHATSAPP_RTP_EXTENSION_PROFILE && profile != RFC8285_ONE_BYTE_EXTENSION_PROFILE {
        return None;
    }
    while let Some((&header, rest)) = extension.split_first() {
        extension = rest;
        let id = header >> 4;
        if id == 0 {
            continue;
        }
        if id == 15 {
            break;
        }
        let len = usize::from(header & 0x0f) + 1;
        let Some((value, rest)) = extension.split_at_checked(len) else {
            break;
        };
        extension = rest;
        if id == WHATSAPP_RTP_EXTENSION_AUDIO_PIGGYBACK_ID && len == 2 {
            return Some(usize::from(u16::from_be_bytes([value[0], value[1]])));
        }
    }
    None
}

/// Split a piggybacked audio packet off the decrypted `payload` of `packet`.
///
/// On return `payload` holds the packet's own media only, and the audio packet comes back whole.
/// Mirrors the receiver in WhatsApp's own stack: an offset of zero, or one that leaves no audio
/// behind it, is not a piggyback and leaves the payload alone; RTP padding, when the outer packet
/// has the P bit, belongs to the media part and is removed from its end.
pub fn split_whatsapp_audio_piggyback(packet: &[u8], payload: &mut Vec<u8>) -> Option<Vec<u8>> {
    let offset = parse_whatsapp_audio_piggyback_offset(packet)?;
    if offset == 0 || offset >= payload.len() {
        return None;
    }
    let audio = payload.split_off(offset);
    let padded = packet.first().is_some_and(|byte| byte & 0x20 != 0);
    if padded && let Some(&padding) = payload.last() {
        let padding = usize::from(padding);
        if padding <= payload.len() {
            payload.truncate(payload.len() - padding);
        }
    }
    Some(audio)
}

/// Authenticated payload bytes, excluding RTP padding. In a WhatsApp carrier the padding
/// terminates the media part, before the embedded audio RTP packet.
pub(crate) fn received_payload_len(packet: &[u8], payload: &[u8]) -> usize {
    let media_end = parse_whatsapp_audio_piggyback_offset(packet)
        .filter(|&offset| offset > 0 && offset < payload.len())
        .unwrap_or(payload.len());
    let padding = if packet.first().is_some_and(|byte| byte & 0x20 != 0) {
        payload
            .get(..media_end)
            .and_then(|media| media.last())
            .copied()
            .map(usize::from)
            .filter(|&padding| padding <= media_end)
            .unwrap_or(0)
    } else {
        0
    };
    payload.len() - padding
}

/// The payload of a plain (unprotected) RTP packet: after the fixed header, CSRCs and extension
/// block, and without RTP padding. `None` for a packet with no payload left.
pub fn plain_rtp_payload(packet: &[u8]) -> Option<&[u8]> {
    let header_len = rtp_header_byte_length(packet)?;
    let mut payload = packet.get(header_len..)?;
    if packet[0] & 0x20 != 0 {
        let padding = usize::from(*payload.last()?);
        payload = payload.get(..payload.len().checked_sub(padding)?)?;
    }
    (!payload.is_empty()).then_some(payload)
}

/// Decode the exact outgoing WhatsApp video extension set. Other 0xdebe layouts,
/// including audio piggyback, deliberately return `None`. For received frame metadata,
/// use [`parse_whatsapp_media_frame_info`] without assuming the optional fields exist.
pub fn parse_whatsapp_video_extension(data: &[u8]) -> Option<VideoRtpExtension> {
    let (Some(profile), extension) = rtp_extension_profile_and_data(data)? else {
        return None;
    };
    if profile != WHATSAPP_RTP_EXTENSION_PROFILE
        || extension.len() != 12
        || extension[0] != 0x30
        || extension[2] != 0x51
        || extension[5] != 0x61
        || extension[8] != 0x91
        || extension[11] != 0
    {
        return None;
    }
    Some(VideoRtpExtension {
        media_frame_info: extension[1],
        initial_bandwidth: u16::from_be_bytes([extension[3], extension[4]]),
        short_offset: i16::from_be_bytes([extension[6], extension[7]]),
        transport_sequence: u16::from_be_bytes([extension[9], extension[10]]),
    })
}

pub fn is_rtp_version2(data: &[u8]) -> bool {
    data.len() >= RTP_FIXED_HEADER_LEN && (data[0] >> 6) & 0x03 == RTP_VERSION
}

/// Typed view of the fixed 12-byte RTP header: one structural bounds check via
/// `Ref::from_prefix` replaces the scattered index math.
#[derive(zerocopy::FromBytes, zerocopy::KnownLayout, zerocopy::Immutable, zerocopy::Unaligned)]
#[repr(C)]
struct RtpFixed {
    /// V(2) P(1) X(1) CC(4)
    vpxcc: u8,
    /// M(1) PT(7)
    mpt: u8,
    sequence_number: zerocopy::big_endian::U16,
    timestamp: zerocopy::big_endian::U32,
    ssrc: zerocopy::big_endian::U32,
}

/// Parse the fixed RTP header fields (the extension word is not decoded).
pub fn parse_rtp_header(data: &[u8]) -> Option<RtpHeader> {
    rtp_header_byte_length(data)?;
    let (h, _) = zerocopy::Ref::<_, RtpFixed>::from_prefix(data).ok()?;
    Some(RtpHeader {
        marker: (h.mpt >> 7) & 1 == 1,
        payload_type: h.mpt & 0x7f,
        sequence_number: h.sequence_number.get(),
        timestamp: h.timestamp.get(),
        ssrc: h.ssrc.get(),
        extension_word: None,
        video_extension: parse_whatsapp_video_extension(data),
    })
}

/// Append the encoded header to `out`, so the outbound path reuses one packet
/// buffer instead of allocating a throwaway header `Vec`. Direct writes, not
/// zerocopy `IntoBytes`: the latter measured +12% instructions on this 16-byte
/// header.
pub fn encode_rtp_header_into(header: &RtpHeader, out: &mut Vec<u8>) {
    let size = header.byte_size();
    debug_assert!(header.extension_word.is_none() || header.video_extension.is_none());
    let mut b = [0u8; WHATSAPP_VIDEO_RTP_HEADER_SIZE];
    b[0] = RTP_VERSION << 6;
    if size > RTP_FIXED_HEADER_LEN {
        b[0] |= 0x10; // X=1 (WhatsApp 0xdebe extension)
    }
    b[1] = ((header.marker as u8) << 7) | (header.payload_type & 0x7f);
    b[2..4].copy_from_slice(&header.sequence_number.to_be_bytes());
    b[4..8].copy_from_slice(&header.timestamp.to_be_bytes());
    b[8..12].copy_from_slice(&header.ssrc.to_be_bytes());
    if size >= 16 {
        b[12..14].copy_from_slice(&WHATSAPP_RTP_EXTENSION_PROFILE.to_be_bytes());
        // High byte stays zero: current extension blocks are at most three words.
        b[15] = if header.video_extension.is_some() {
            3
        } else {
            header.extension_word.is_some() as u8
        };
    }
    if let Some(extension) = header.video_extension {
        let mut encoded = [0u8; 12];
        extension.encode_into(&mut encoded);
        b[16..28].copy_from_slice(&encoded);
    } else if size >= 20
        && let Some(w) = header.extension_word
    {
        b[16..20].copy_from_slice(&w.to_be_bytes());
    }
    out.extend_from_slice(&b[..size]);
}

pub fn encode_rtp_header(header: &RtpHeader) -> Vec<u8> {
    let mut buf = Vec::with_capacity(header.byte_size());
    encode_rtp_header_into(header, &mut buf);
    buf
}

/// Send-side RTP sequencer: seq starts at 1, timestamp advances by `samples_per_packet`.
pub struct RtpStream {
    pub ssrc: u32,
    seq: u16,
    timestamp: u32,
    last_sent_timestamp: Option<u32>,
    samples_per_packet: u32,
    speech_started: bool,
    speech_start_markers: bool,
    mlow_profile: bool,
    audio_packet_index: usize,
    warp_piggyback: bool,
    payload_type: u8,
}

impl RtpStream {
    pub fn new(ssrc: u32, samples_per_packet: u32, warp_piggyback: bool) -> Self {
        Self {
            ssrc,
            seq: 1,
            timestamp: 0,
            last_sent_timestamp: None,
            samples_per_packet,
            speech_started: false,
            speech_start_markers: true,
            mlow_profile: true,
            audio_packet_index: 0,
            warp_piggyback,
            payload_type: RTP_PAYLOAD_TYPE_MLOW,
        }
    }

    pub fn set_payload_type(&mut self, payload_type: u8) -> bool {
        if payload_type > 127 {
            return false;
        }
        self.payload_type = payload_type;
        true
    }

    pub fn set_mlow_profile(&mut self, enabled: bool) {
        self.mlow_profile = enabled;
        self.speech_start_markers = enabled;
    }

    /// The current send timestamp (last emitted), for an RTCP Sender Report.
    pub fn rtp_timestamp(&self) -> u32 {
        self.last_sent_timestamp.unwrap_or(self.timestamp)
    }

    /// The last emitted timestamp and the one the next packet will carry, once a packet has gone.
    pub(crate) fn sent_and_next_timestamp(&self) -> Option<(u32, u32)> {
        self.last_sent_timestamp.map(|sent| (sent, self.timestamp))
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
        let dtx = if self.mlow_profile {
            is_mlow_dtx_payload(payload)
        } else {
            is_opus_dtx_payload(payload)
        };
        let priming = is_opus_priming_payload(payload);
        let speech = !dtx && !priming;
        let use_marker = marker || (self.speech_start_markers && speech && !self.speech_started);
        if dtx {
            self.speech_started = false;
        } else if speech {
            self.speech_started = true;
        }
        let header = RtpHeader {
            marker: use_marker,
            payload_type: self.payload_type,
            sequence_number: self.seq,
            timestamp: self.timestamp,
            ssrc: self.ssrc,
            extension_word: self.resolve_warp_extension(dtx),
            video_extension: None,
        };
        self.last_sent_timestamp = Some(header.timestamp);
        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(self.samples_per_packet);
        header
    }

    /// Pre-speech ladder packet: advances seq/timestamp without a marker or speech latch.
    pub fn next_pre_speech_packet(&mut self) -> RtpHeader {
        let header = RtpHeader {
            marker: false,
            payload_type: self.payload_type,
            sequence_number: self.seq,
            timestamp: self.timestamp,
            ssrc: self.ssrc,
            extension_word: self.resolve_warp_extension(false),
            video_extension: None,
        };
        self.last_sent_timestamp = Some(header.timestamp);
        self.seq = self.seq.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(self.samples_per_packet);
        header
    }
}

/// Send-side sequencer for the video stream: one header per RTP packet of an
/// access unit, the timestamp is shared by every packet of the AU and advances
/// by `ts_stride` only when the AU closes (marker packet).
pub struct VideoRtpStream {
    pub ssrc: u32,
    seq: u16,
    timestamp: u32,
    last_sent_timestamp: Option<u32>,
    ts_stride: u32,
    transport_sequence: u16,
}

impl VideoRtpStream {
    pub fn new(ssrc: u32, ts_stride: u32) -> Option<Self> {
        if ts_stride == 0 {
            return None;
        }
        Some(Self {
            ssrc,
            seq: 0,
            timestamp: 0,
            last_sent_timestamp: None,
            ts_stride,
            transport_sequence: 0,
        })
    }

    /// The current send timestamp (last emitted), for an RTCP Sender Report.
    pub fn rtp_timestamp(&self) -> u32 {
        self.last_sent_timestamp.unwrap_or(self.timestamp)
    }

    /// Change the cadence used after the current access unit. This keeps sequence numbers and the
    /// RTP clock continuous when an encoder changes frame rate.
    pub fn set_timestamp_stride(&mut self, ts_stride: u32) -> bool {
        if ts_stride == 0 {
            return false;
        }
        self.ts_stride = ts_stride;
        true
    }

    /// Set the timestamp for the next access unit from a source-provided RTP capture clock.
    pub(crate) fn set_timestamp(&mut self, timestamp: u32) -> bool {
        if let Some(last) = self.last_sent_timestamp
            && timestamp.wrapping_sub(last) > 0x8000_0000
        {
            return false;
        }
        self.timestamp = timestamp;
        true
    }

    /// Header for the next packet of the current AU; `last_in_au` sets the
    /// marker and moves the timestamp to the next AU. `media_frame_info` is an
    /// encoded-frame property and must be identical on every fragment of the AU.
    /// The transmission offset is zero: see [`Self::next_video_packet_with_offset`].
    pub fn next_video_packet(&mut self, last_in_au: bool, media_frame_info: u8) -> RtpHeader {
        self.next_video_packet_with_offset(last_in_au, media_frame_info, 0)
    }

    /// [`Self::next_video_packet`] carrying `short_offset`, the milliseconds from the picture's
    /// capture to the packet's transmission (see [`video_short_offset`]).
    pub fn next_video_packet_with_offset(
        &mut self,
        last_in_au: bool,
        media_frame_info: u8,
        short_offset: i16,
    ) -> RtpHeader {
        let header = RtpHeader {
            marker: last_in_au,
            payload_type: RTP_PAYLOAD_TYPE_H264,
            sequence_number: self.seq,
            timestamp: self.timestamp,
            ssrc: self.ssrc,
            extension_word: None,
            video_extension: Some(VideoRtpExtension {
                media_frame_info,
                initial_bandwidth: 0,
                short_offset,
                transport_sequence: self.transport_sequence,
            }),
        };
        self.last_sent_timestamp = Some(header.timestamp);
        self.seq = self.seq.wrapping_add(1);
        self.transport_sequence = self.transport_sequence.wrapping_add(1);
        if last_in_au {
            self.timestamp = self.timestamp.wrapping_add(self.ts_stride);
        }
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
            video_extension: None,
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
            video_extension: None,
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
            video_extension: None,
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
        assert!(is_opus_dtx_payload(&[0xBB, 0x03]));
        assert!(!is_opus_dtx_payload(&[]));
        assert!(is_mlow_dtx_payload(&[0x90]));
        assert!(!is_mlow_dtx_payload(&[0x48, 0x00]));
        assert!(is_opus_priming_payload(&OPUS_PRIMING_FRAME_1));
        assert!(is_opus_priming_payload(&OPUS_PRIMING_FRAME_2));
        assert!(!is_opus_priming_payload(&[0x12, 0x36]));
        assert!(is_opus_mlow_speech_payload(&[0x48; 20]));
        assert!(!is_opus_mlow_speech_payload(&[0x48; 4]));
        assert!(is_whatsapp_opus_rtp_payload(111));
        assert!(is_whatsapp_opus_rtp_payload(120));
        assert!(is_whatsapp_opus_rtp_payload(121));
        assert!(is_whatsapp_opus_rtp_payload(122));
        assert!(!is_whatsapp_opus_rtp_payload(96));
    }

    #[test]
    fn audio_stream_payload_type_is_configurable_without_resetting_timing() {
        let mut stream = RtpStream::new(0x1122_3344, 2_880, false);
        let first = stream.next_packet(&[0x48; 20], false);
        assert_eq!(first.payload_type, RTP_PAYLOAD_TYPE_MLOW);
        assert_eq!(first.timestamp, 0);

        assert!(stream.set_payload_type(RTP_PAYLOAD_TYPE_OPUS));
        let second = stream.next_packet(&[0x48; 20], false);
        assert_eq!(second.payload_type, RTP_PAYLOAD_TYPE_OPUS);
        assert_eq!(second.sequence_number, 2);
        assert_eq!(second.timestamp, 2_880);
        assert!(!stream.set_payload_type(128));
    }

    #[test]
    fn video_stream_marker_and_timestamp_advance_per_au() {
        let mut s = VideoRtpStream::new(0x1122_3344, VIDEO_TS_STRIDE_15FPS).unwrap();
        // 3-packet AU: ts shared, marker only on the last.
        let p0 = s.next_video_packet(false, VIDEO_MEDIA_FRAME_INFO_IDR);
        let p1 = s.next_video_packet(false, VIDEO_MEDIA_FRAME_INFO_IDR);
        let p2 = s.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_IDR);
        assert_eq!(
            (p0.sequence_number, p1.sequence_number, p2.sequence_number),
            (0, 1, 2)
        );
        assert_eq!((p0.timestamp, p1.timestamp, p2.timestamp), (0, 0, 0));
        assert_eq!((p0.marker, p1.marker, p2.marker), (false, false, true));
        assert_eq!(s.rtp_timestamp(), 0, "SR maps to the emitted AU timestamp");
        assert_eq!(
            [p0, p1, p2].map(|packet| packet.video_extension.unwrap().media_frame_info),
            [VIDEO_MEDIA_FRAME_INFO_IDR; 3]
        );
        assert_eq!(
            [p0, p1, p2].map(|packet| packet.video_extension.unwrap().transport_sequence),
            [0, 1, 2]
        );
        // Next AU: timestamp advanced by one stride.
        let p3 = s.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        assert_eq!(p3.timestamp, VIDEO_TS_STRIDE_15FPS);
        assert_eq!(p3.sequence_number, 3);
        assert_eq!(s.rtp_timestamp(), VIDEO_TS_STRIDE_15FPS);
        assert_eq!(
            p3.video_extension.unwrap().media_frame_info,
            VIDEO_MEDIA_FRAME_INFO_DELTA
        );
        assert_eq!(p3.video_extension.unwrap().transport_sequence, 3);
        for p in [p0, p1, p2, p3] {
            assert_eq!(p.payload_type, RTP_PAYLOAD_TYPE_H264);
            assert_eq!(p.extension_word, None);
            assert_eq!(p.byte_size(), WHATSAPP_VIDEO_RTP_HEADER_SIZE);
        }
    }

    #[test]
    fn video_stream_changes_cadence_without_resetting_timestamp() {
        let mut stream = VideoRtpStream::new(0x1122_3344, VIDEO_TS_STRIDE_15FPS).unwrap();
        let first = stream.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        assert_eq!(first.timestamp, 0);

        assert!(stream.set_timestamp_stride(VIDEO_TS_STRIDE_20FPS));
        let second = stream.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        let third = stream.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        assert_eq!(second.timestamp, VIDEO_TS_STRIDE_15FPS);
        assert_eq!(
            third.timestamp,
            VIDEO_TS_STRIDE_15FPS + VIDEO_TS_STRIDE_20FPS
        );
        assert!(!stream.set_timestamp_stride(0));
        assert!(VideoRtpStream::new(0x1122_3344, 0).is_none());
    }

    #[test]
    fn video_source_timestamp_accepts_repeat_and_wrap_but_rejects_late() {
        let mut stream = VideoRtpStream::new(0x1122_3344, VIDEO_TS_STRIDE_15FPS).unwrap();
        let _ = stream.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        assert!(stream.set_timestamp(0));
        assert!(stream.set_timestamp(50));
        assert!(!stream.set_timestamp(u32::MAX - 50));
        let mut wrapped = VideoRtpStream::new(0x1122_3344, VIDEO_TS_STRIDE_15FPS).unwrap();
        assert!(wrapped.set_timestamp(u32::MAX - 100));
        let _ = wrapped.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        assert!(wrapped.set_timestamp(50));
    }

    #[test]
    fn video_header_round_trips_through_parse() {
        let mut s = VideoRtpStream::new(0xdead_beef, VIDEO_TS_STRIDE_15FPS).unwrap();
        s.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_IDR);
        let h = s.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_DELTA);
        let bytes = encode_rtp_header(&h);
        assert_eq!(rtp_header_byte_length(&bytes), Some(28));
        assert_eq!(parse_rtp_header(&bytes), Some(h));
    }

    #[test]
    fn frame_info_is_independent_of_optional_extensions() {
        for info in 0..=255u8 {
            for length in 1..=16usize {
                let mut extension = vec![0; length + 3];
                extension[..4].copy_from_slice(&[0, 0x0f, 0x30 | (length - 1) as u8, info]);
                extension.resize(extension.len().next_multiple_of(4), 0);
                let mut packet = vec![
                    0x91,
                    97,
                    0,
                    1,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    1,
                    0,
                    0,
                    0,
                    2,
                    0xde,
                    0xbe,
                    0,
                    (extension.len() / 4) as u8,
                ];
                packet.extend_from_slice(&extension);
                assert_eq!(parse_whatsapp_media_frame_info(&packet), Some(info));
                assert_eq!(parse_whatsapp_video_extension(&packet), None);
                packet[16] = 0xbe;
                packet[17] = 0xde;
                assert_eq!(parse_whatsapp_media_frame_info(&packet), None);
            }
        }
    }

    #[test]
    fn frame_info_padding_duplicates_and_truncation_match_wasm() {
        for (extension, expected) in [
            (&[0x30, 0, 0, 0][..], Some(0)),
            (&[0, 0x0f, 0, 0][..], None),
            (&[0xf0, 0xff, 0x30, 3][..], Some(3)),
            (&[0x30, 1, 0x30, 3][..], Some(3)),
            (&[0x30, 3, 0x30, 1][..], Some(1)),
            (&[0x30, 1, 0x37, 3, 0, 0, 0, 0, 0, 0, 0, 0][..], Some(1)),
            (&[0x37, 3, 0, 0, 0, 0, 0, 0, 0, 0x30, 1, 0][..], Some(1)),
            (&[0x32, 1, 0, 0, 0x35, 3, 0, 0, 0, 0, 0, 0][..], Some(1)),
            (&[0x30, 3, 0xff, 0][..], Some(3)),
            (&[0xff, 0x30, 3, 0][..], None),
            (&[0x30, 1, 0x3f, 3][..], Some(1)),
            (&[0x3f, 3, 0, 0][..], None),
            (&[0, 0, 0, 0x30][..], None),
        ] {
            let mut packet = vec![
                0x90,
                97,
                0,
                1,
                0,
                0,
                0,
                1,
                0,
                0,
                0,
                1,
                0xde,
                0xbe,
                0,
                (extension.len() / 4) as u8,
            ];
            packet.extend_from_slice(extension);
            assert_eq!(
                parse_whatsapp_media_frame_info(&packet),
                expected,
                "{extension:02x?}"
            );
            packet[15] += 1;
            assert_eq!(
                parse_whatsapp_media_frame_info(&packet),
                None,
                "truncated block"
            );
        }
        for packet in [&[][..], &[0x80; 11], &[0x90; 12], &[0x80; 12], &[0x8f; 12]] {
            assert_eq!(parse_whatsapp_media_frame_info(packet), None);
        }
    }

    #[test]
    fn video_header_uses_wasm_extension_layout() {
        let mut stream = VideoRtpStream::new(0x1122_3344, VIDEO_TS_STRIDE_15FPS).unwrap();
        let header = stream.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_IDR);
        assert_eq!(
            encode_rtp_header(&header),
            [
                0x90, 0xe1, // V=2, X=1, marker, PT=97
                0x00, 0x00, // sequence
                0x00, 0x00, 0x00, 0x00, // timestamp
                0x11, 0x22, 0x33, 0x44, // SSRC
                0xde, 0xbe, 0x00, 0x03, // WARP profile, three words
                0x30, 0x08, // ID 3: keyframe, zero rotation
                0x51, 0x00, 0x00, // ID 5: two-byte initial bandwidth
                0x61, 0x00, 0x00, // ID 6: two-byte short timestamp offset
                0x91, 0x00, 0x00, // ID 9: two-byte transport sequence
                0x00, // word alignment
            ]
        );
        let encoded = encode_rtp_header(&header);
        assert_eq!(
            rtp_extension_profile_and_data(&encoded),
            Some((Some(WHATSAPP_RTP_EXTENSION_PROFILE), &encoded[16..28]))
        );
    }

    #[test]
    fn media_time_offset_reads_element_six_alone() {
        let header = |extension: &[u8]| {
            let mut packet = vec![
                0x90,
                RTP_PAYLOAD_TYPE_H264,
                0,
                1,
                0,
                0,
                0x2a,
                0xf8,
                0,
                0,
                0,
                1,
            ];
            packet.extend_from_slice(&WHATSAPP_RTP_EXTENSION_PROFILE.to_be_bytes());
            packet.extend_from_slice(&((extension.len() / 4) as u16).to_be_bytes());
            packet.extend_from_slice(extension);
            packet
        };
        // Only the offset, after padding, negative.
        assert_eq!(
            parse_whatsapp_media_time_offset(&header(&[0x00, 0x61, 0xff, 0x9c])),
            Some(-100)
        );
        // Behind the frame info, with none of the other elements.
        assert_eq!(
            parse_whatsapp_media_time_offset(&header(&[0x30, 0x08, 0x61, 0x00, 0x4f, 0, 0, 0])),
            Some(79)
        );
        assert_eq!(
            parse_whatsapp_media_time_offset(&header(&[0x30, 0x08, 0, 0])),
            None
        );
        // Truncated: the value runs past the block.
        assert_eq!(
            parse_whatsapp_media_time_offset(&header(&[0x00, 0x00, 0x61, 0x00])),
            None
        );
        let mut plain = header(&[]);
        plain[0] = 0x80;
        plain.truncate(RTP_FIXED_HEADER_LEN);
        assert_eq!(parse_whatsapp_media_time_offset(&plain), None);
    }

    #[test]
    fn video_short_offset_is_written_in_place_and_clamped() {
        assert_eq!(video_short_offset(0), 0);
        assert_eq!(video_short_offset(23), 23);
        assert_eq!(video_short_offset(32_767), i16::MAX);
        assert_eq!(video_short_offset(u64::MAX), i16::MAX);

        let mut stream = VideoRtpStream::new(0x1122_3344, VIDEO_TS_STRIDE_15FPS).unwrap();
        let header =
            stream.next_video_packet_with_offset(true, VIDEO_MEDIA_FRAME_INFO_DELTA, 0x0123);
        let encoded = encode_rtp_header(&header);
        let range = video_short_offset_range(&encoded).unwrap();
        assert_eq!(encoded[range], [0x01, 0x23]);
        assert_eq!(
            parse_rtp_header(&encoded)
                .unwrap()
                .video_extension
                .unwrap()
                .short_offset,
            0x0123
        );

        let audio = encode_rtp_header(&RtpHeader {
            marker: false,
            payload_type: RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO,
            sequence_number: 2,
            timestamp: 320,
            ssrc: 1,
            extension_word: Some(WHATSAPP_RTP_EXTENSION_DTX_WORD),
            video_extension: None,
        });
        assert_eq!(video_short_offset_range(&audio), None);
    }

    #[test]
    fn video_header_matches_android_capture() {
        let k = kats();
        let captured = hex::decode(k["rtp"]["androidVideoHeader28"].as_str().unwrap()).unwrap();
        assert_eq!(rtp_header_byte_length(&captured), Some(28));
        assert_eq!(
            hex::encode(rtp_extension_profile_and_data(&captured).unwrap().1),
            k["rtp"]["androidVideoExtension12"].as_str().unwrap()
        );
        assert_eq!(
            parse_rtp_header(&captured),
            Some(RtpHeader {
                marker: true,
                payload_type: RTP_PAYLOAD_TYPE_H264,
                sequence_number: 1,
                timestamp: 114_120,
                ssrc: 0x49c5_fb8c,
                extension_word: None,
                video_extension: Some(VideoRtpExtension {
                    media_frame_info: 0x09,
                    initial_bandwidth: 0,
                    short_offset: 29,
                    transport_sequence: 0x0c3f,
                }),
            })
        );
        assert_eq!(parse_whatsapp_media_time_offset(&captured), Some(29));
    }

    #[test]
    fn delta_video_header_matches_android_capture() {
        let k = kats();
        let captured =
            hex::decode(k["rtp"]["androidVideoDeltaHeader28"].as_str().unwrap()).unwrap();
        assert_eq!(rtp_header_byte_length(&captured), Some(28));
        assert_eq!(
            hex::encode(rtp_extension_profile_and_data(&captured).unwrap().1),
            k["rtp"]["androidVideoDeltaExtension12"].as_str().unwrap()
        );
        assert_eq!(
            parse_rtp_header(&captured),
            Some(RtpHeader {
                marker: true,
                payload_type: RTP_PAYLOAD_TYPE_H264,
                sequence_number: 7,
                timestamp: 143_010,
                ssrc: 0x9b19_59f0,
                extension_word: None,
                video_extension: Some(VideoRtpExtension {
                    media_frame_info: 0x01,
                    initial_bandwidth: 0,
                    short_offset: 36,
                    transport_sequence: 0x0a6a,
                }),
            })
        );
        assert_eq!(parse_whatsapp_media_time_offset(&captured), Some(36));
        assert_eq!(
            VIDEO_MEDIA_FRAME_INFO_IDR,
            VIDEO_MEDIA_FRAME_INFO_DELTA | 0x08
        );
    }

    /// A video header as WhatsApp's sender writes it when the packet carries an audio packet: the
    /// four elements the Android capture has, with the `AP` element (id 7, two bytes) among them.
    fn video_header_with_piggyback(offset: u16, padded: bool) -> Vec<u8> {
        let [high, low] = offset.to_be_bytes();
        let mut header = vec![
            if padded { 0xb0 } else { 0x90 },
            0xe1,
            0,
            7,
            0,
            0x02,
            0x2e,
            0xa2,
            0x9b,
            0x19,
            0x59,
            0xf0,
            0xde,
            0xbe,
            0,
            4,
        ];
        header.extend([
            0x30, 0x01, 0x51, 0, 0, 0x61, 0, 0x24, 0x71, high, low, 0x91, 0x0a, 0x6a, 0, 0,
        ]);
        header
    }

    #[test]
    fn the_audio_piggyback_offset_is_read_among_the_other_video_elements() {
        let header = video_header_with_piggyback(321, false);
        assert_eq!(parse_whatsapp_audio_piggyback_offset(&header), Some(321));
        // Its presence takes the packet out of the exact four-element shape, which is what the
        // frame-info reader exists for: it still finds the rotation.
        assert_eq!(parse_whatsapp_video_extension(&header), None);
        assert_eq!(parse_whatsapp_media_frame_info(&header), Some(0x01));

        let captured =
            hex::decode(kats()["rtp"]["androidVideoDeltaHeader28"].as_str().unwrap()).expect("hex");
        assert_eq!(
            parse_whatsapp_audio_piggyback_offset(&captured),
            None,
            "a video packet with no audio in it says so"
        );
    }

    #[test]
    fn only_a_well_formed_piggyback_element_is_believed() {
        // RFC 8285's own profile is read the same way.
        let mut rfc = video_header_with_piggyback(40, false);
        rfc[12..14].copy_from_slice(&RFC8285_ONE_BYTE_EXTENSION_PROFILE.to_be_bytes());
        assert_eq!(parse_whatsapp_audio_piggyback_offset(&rfc), Some(40));

        // Any other profile is someone else's extension.
        let mut other = video_header_with_piggyback(40, false);
        other[12..14].copy_from_slice(&0x1000u16.to_be_bytes());
        assert_eq!(parse_whatsapp_audio_piggyback_offset(&other), None);

        // Id 7 with a length other than two bytes is not the piggyback element.
        let mut wrong_length = video_header_with_piggyback(40, false);
        wrong_length[24] = 0x72;
        assert_eq!(parse_whatsapp_audio_piggyback_offset(&wrong_length), None);

        // The reserved id 15 ends the element list before the piggyback element is reached.
        let mut stopped = video_header_with_piggyback(40, false);
        stopped[21] = 0xf0;
        assert_eq!(parse_whatsapp_audio_piggyback_offset(&stopped), None);

        // No extension at all.
        let bare = [0x80, 0xe1, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1];
        assert_eq!(parse_whatsapp_audio_piggyback_offset(&bare), None);
    }

    #[test]
    fn a_piggybacked_audio_packet_is_split_off_the_media_it_rode_with() {
        let header = video_header_with_piggyback(5, false);
        let media = [0x41, 1, 2, 3, 4];
        let audio = [
            0x80, 0xfa, 0, 9, 0, 0, 0x21, 0xc0, 0x12, 0x34, 0x56, 0x78, 0x50, 7, 7,
        ];
        let mut payload = [media.as_slice(), audio.as_slice()].concat();
        assert_eq!(
            split_whatsapp_audio_piggyback(&header, &mut payload).as_deref(),
            Some(audio.as_slice())
        );
        assert_eq!(
            payload, media,
            "only the media is left for the depacketizer"
        );

        // An offset that leaves nothing behind it, or none at all, splits nothing.
        for offset in [0u16, 20, 21] {
            let header = video_header_with_piggyback(offset, false);
            let mut payload = [media.as_slice(), audio.as_slice()].concat();
            assert_eq!(split_whatsapp_audio_piggyback(&header, &mut payload), None);
            assert_eq!(payload.len(), media.len() + audio.len());
        }
    }

    #[test]
    fn received_payload_count_excludes_padding_before_embedded_audio() {
        let header = video_header_with_piggyback(7, true);
        let mut payload = vec![0x41, 1, 2, 3, 0, 0, 3];
        let audio = [
            0x80, 0xfa, 0, 9, 0, 0, 0x21, 0xc0, 0x12, 0x34, 0x56, 0x78, 0x50,
        ];
        payload.extend_from_slice(&audio);
        assert_eq!(received_payload_len(&header, &payload), 4 + audio.len());
        assert_eq!(received_payload_len(&[0xa0], &[1, 2, 0, 2]), 2);
        assert_eq!(received_payload_len(&[0x80], &[1, 2, 0, 2]), 4);
        assert_eq!(received_payload_len(&[0xa0], &[]), 0);
        assert_eq!(received_payload_len(&[0xa0], &[9]), 1);
    }

    #[test]
    fn padding_belongs_to_the_media_part_of_a_piggybacked_packet() {
        // P set: the sender's stack strips the count in the last byte of the media part, which
        // precedes the audio rather than ending the packet.
        let header = video_header_with_piggyback(7, true);
        let mut payload = vec![0x41, 1, 2, 3, 0, 0, 3];
        let audio = vec![
            0x80, 0xfa, 0, 9, 0, 0, 0x21, 0xc0, 0x12, 0x34, 0x56, 0x78, 0x50,
        ];
        payload.extend_from_slice(&audio);
        assert_eq!(
            split_whatsapp_audio_piggyback(&header, &mut payload),
            Some(audio)
        );
        assert_eq!(payload, [0x41, 1, 2, 3]);
    }

    #[test]
    fn a_plain_rtp_payload_skips_the_header_extension_and_padding() {
        // The 16-byte speech header: fixed header plus an empty 0xdebe block.
        let mut packet = encode_rtp_header(&RtpHeader {
            marker: false,
            payload_type: RTP_PAYLOAD_TYPE_MLOW_SEPARATE,
            sequence_number: 9,
            timestamp: 960,
            ssrc: 0x1234_5678,
            extension_word: None,
            video_extension: None,
        });
        assert_eq!(packet.len(), WHATSAPP_RTP_HEADER_SIZE);
        packet.extend([0x50, 1, 2]);
        assert_eq!(plain_rtp_payload(&packet), Some([0x50, 1, 2].as_slice()));

        packet[0] |= 0x20;
        packet.extend([0, 2]);
        assert_eq!(plain_rtp_payload(&packet), Some([0x50, 1, 2].as_slice()));

        let header_only = encode_rtp_header(&RtpHeader {
            marker: false,
            payload_type: RTP_PAYLOAD_TYPE_MLOW,
            sequence_number: 1,
            timestamp: 0,
            ssrc: 1,
            extension_word: None,
            video_extension: None,
        });
        assert_eq!(plain_rtp_payload(&header_only), None);
    }

    #[test]
    fn extension_view_handles_one_byte_profile_and_malformed_lengths() {
        let packet = [
            0x90, 97, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0xbe, 0xde, 0, 1, 0x31, 1, 2, 3, 0x65,
        ];
        assert_eq!(
            rtp_extension_profile_and_data(&packet),
            Some((Some(0xbede), &packet[16..20]))
        );
        assert_eq!(rtp_header_byte_length(&packet), Some(20));

        let mut malformed = packet;
        malformed[15] = 2;
        assert_eq!(rtp_extension_profile_and_data(&malformed), None);
    }

    #[test]
    fn video_stream_wraps_seq_and_timestamp() {
        let mut s = VideoRtpStream::new(1, u32::MAX).unwrap();
        s.seq = u16::MAX;
        s.timestamp = u32::MAX;
        let p = s.next_video_packet(true, VIDEO_MEDIA_FRAME_INFO_IDR);
        assert_eq!(p.sequence_number, u16::MAX);
        let p = s.next_video_packet(false, VIDEO_MEDIA_FRAME_INFO_DELTA);
        assert_eq!(p.sequence_number, 0, "seq must wrap, not panic");
        assert_eq!(p.timestamp, u32::MAX - 1, "timestamp must wrap, not panic");
    }

    #[test]
    fn stream_sequence_and_marker() {
        let mut s = RtpStream::new(0xabcd, 320, false);
        // Priming/DTX before speech: no marker, no speech latch.
        let p0 = s.next_packet(&OPUS_PRIMING_FRAME_2, false);
        assert_eq!((p0.sequence_number, p0.timestamp, p0.marker), (1, 0, false));
        let d1 = s.next_packet(&[0x90], false);
        assert_eq!(
            (d1.sequence_number, d1.timestamp, d1.marker),
            (2, 320, false)
        );
        assert_eq!(d1.extension_word, Some(WHATSAPP_RTP_EXTENSION_DTX_WORD));
        assert_eq!(s.rtp_timestamp(), 320);
        // First speech frame latches the marker.
        let sp = s.next_packet(&[0x48; 40], false);
        assert_eq!(
            (sp.sequence_number, sp.timestamp, sp.marker),
            (3, 640, true)
        );
        assert_eq!(s.rtp_timestamp(), 640);
        // Subsequent speech has no marker.
        let sp2 = s.next_packet(&[0x48; 40], false);
        assert!(!sp2.marker);

        let d2 = s.next_packet(&[0x90], false);
        assert!(!d2.marker);
        let resumed = s.next_packet(&[0x48; 40], false);
        assert!(resumed.marker, "speech after DTX must re-arm the marker");
    }

    #[test]
    fn native_opus_can_disable_mlow_speech_markers() {
        let mut stream = RtpStream::new(0xabcd, 960, false);
        stream.set_mlow_profile(false);

        assert!(!stream.next_packet(&[0x58; 80], false).marker);
        let _ = stream.next_packet(&[0x10], false);
        assert!(!stream.next_packet(&[0x58; 80], false).marker);
    }

    #[test]
    fn short_mlow_speech_is_not_classified_as_dtx() {
        let mut stream = RtpStream::new(0xabcd, 960, false);

        assert!(stream.next_packet(&[0x48, 0x00], false).marker);
        let next = stream.next_packet(&[0x48, 0x00], false);

        assert!(!next.marker);
        assert_eq!(next.extension_word, None);
    }
}
