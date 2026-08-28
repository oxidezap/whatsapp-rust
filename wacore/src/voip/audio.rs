//! Audio format and I/O contracts shared by the sans-I/O engine and platform drivers.

use bytes::Bytes;
use wacore_binary::Jid;

use super::rtp::{
    RTP_PAYLOAD_TYPE_MLOW, RTP_PAYLOAD_TYPE_MLOW_RED, RTP_PAYLOAD_TYPE_OPUS,
    RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO,
};

/// Codec carried inside WhatsApp's audio RTP payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioCodec {
    Mlow,
    Opus,
}

/// RTP payload family selected for the call independently from the encoded bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioRtpProfile {
    /// WhatsApp MLOW framing: PT 120/121 with the in-profile Opus escape.
    Mlow,
    /// Native Opus bytes. Payload type and clock remain independently negotiated.
    StandardOpus,
}

/// Where encoding and decoding happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioIo {
    /// The core converts 16-bit PCM using its built-in codec.
    Pcm,
    /// The application supplies and consumes complete codec payloads.
    Encoded,
}

/// Fixed audio timing for one call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AudioFormat {
    /// Codec bytes supplied by the encoded source.
    pub codec: AudioCodec,
    /// RTP family negotiated with the peer. This is not implied by [`Self::codec`].
    pub rtp_profile: AudioRtpProfile,
    /// `<audio rate=…>` value used by call signaling.
    pub signaling_rate: u32,
    /// PCM rate expected by a codec adapter, when one is used.
    pub sample_rate: u32,
    pub channels: u8,
    /// PCM samples represented by one encoded payload, per channel.
    pub samples_per_frame: u32,
    /// RTP clock used for reception statistics.
    pub rtp_clock_rate: u32,
    /// RTP timestamp increment after each encoded payload.
    pub rtp_timestamp_step: u32,
    /// RTP payload type advertised by the selected WhatsApp media profile.
    pub rtp_payload_type: u8,
}

impl AudioFormat {
    /// The implemented MLOW operating point: mono, 16 kHz, 60 ms.
    pub const MLOW_16KHZ_60MS: Self = Self {
        codec: AudioCodec::Mlow,
        rtp_profile: AudioRtpProfile::Mlow,
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 16_000,
        rtp_timestamp_step: 960,
        rtp_payload_type: RTP_PAYLOAD_TYPE_MLOW,
    };

    /// Standard Opus/CELT carried through MLOW's native escape.
    ///
    /// The Opus packet must be CELT-only and use MLOW's rewritten TOC. The RTP clock remains the
    /// negotiated MLOW clock.
    pub const OPUS_MLOW_16KHZ_60MS: Self = Self {
        codec: AudioCodec::Opus,
        rtp_profile: AudioRtpProfile::Mlow,
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 16_000,
        rtp_timestamp_step: 960,
        rtp_payload_type: RTP_PAYLOAD_TYPE_MLOW,
    };

    /// Native Opus using WhatsApp's default PT 120 and 16 kHz RTP clock.
    ///
    /// Capability v1 index 31 disables MLOW decoding independently from PT and clock selection.
    pub const OPUS_16KHZ_60MS: Self = Self {
        codec: AudioCodec::Opus,
        rtp_profile: AudioRtpProfile::StandardOpus,
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 16_000,
        rtp_timestamp_step: 960,
        rtp_payload_type: RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO,
    };

    /// Native Opus with a 16 kHz codec adapter and RFC 7587's 48 kHz RTP clock.
    pub const OPUS_RFC7587_16KHZ_60MS: Self = Self {
        codec: AudioCodec::Opus,
        rtp_profile: AudioRtpProfile::StandardOpus,
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 48_000,
        rtp_timestamp_step: 2_880,
        rtp_payload_type: RTP_PAYLOAD_TYPE_OPUS,
    };

    /// RFC 7587 Opus timing. Useful for interop experiments with external RTP-aware codecs.
    pub const OPUS_RFC7587_48KHZ_60MS: Self = Self {
        codec: AudioCodec::Opus,
        rtp_profile: AudioRtpProfile::StandardOpus,
        signaling_rate: 16_000,
        sample_rate: 48_000,
        channels: 1,
        samples_per_frame: 2_880,
        rtp_clock_rate: 48_000,
        rtp_timestamp_step: 2_880,
        rtp_payload_type: RTP_PAYLOAD_TYPE_OPUS,
    };

    pub const fn accepts_rtp_payload_type(self, payload_type: u8) -> bool {
        payload_type == self.rtp_payload_type
            || matches!(self.rtp_profile, AudioRtpProfile::Mlow)
                && payload_type == RTP_PAYLOAD_TYPE_MLOW_RED
    }

    /// Identify the actual inbound codec after the negotiated RTP profile is known.
    pub fn inbound_codec(self, payload_type: u8, payload: &[u8]) -> AudioCodec {
        match self.rtp_profile {
            AudioRtpProfile::StandardOpus => AudioCodec::Opus,
            AudioRtpProfile::Mlow
                if payload_type != RTP_PAYLOAD_TYPE_MLOW_RED && is_mlow_embedded_opus(payload) =>
            {
                AudioCodec::Opus
            }
            AudioRtpProfile::Mlow => AudioCodec::Mlow,
        }
    }

    /// Whether this payload is MLOW's escape, as opposed to native Opus that merely looks like one.
    ///
    /// The marker alone cannot answer it: `is_mlow_embedded_opus` tests the top two bits, and every
    /// native Opus CELT config (24..=31) sets them -- a native 60 ms CELT packet starts 0xC3. Read
    /// on the marker alone, native CELT is called an escape and has a TOC that was never rewritten
    /// rewritten again, which does not decode.
    ///
    /// What separates them is the same arithmetic the content probe rests on. An escape's low TOC
    /// bit means "multiple frames", not an Opus frame count, so read back as Opus it is code 0 or
    /// code 1 -- one or two CELT frames, at most 40 ms. It can therefore never claim the negotiated
    /// packet duration, while native CELT at that duration claims it exactly. Parsing as Opus at
    /// this format's own cadence is thus a property only the native packet has.
    #[must_use]
    pub fn payload_is_mlow_escape(self, payload: &[u8]) -> bool {
        matches!(self.rtp_profile, AudioRtpProfile::Mlow)
            && is_mlow_embedded_opus(payload)
            && crate::voip::opus_packet_shape(payload)
                .and_then(|shape| shape.total_samples(self.rtp_clock_rate))
                != Some(self.rtp_timestamp_step)
    }

    /// Reject raw Opus that the MLOW receiver would parse as proprietary codec data.
    pub fn accepts_encoded_payload(self, payload: &[u8]) -> bool {
        !payload.is_empty()
            && (!matches!(
                (self.codec, self.rtp_profile),
                (AudioCodec::Opus, AudioRtpProfile::Mlow)
            ) || is_mlow_embedded_opus(payload)
                || payload == [0x90])
    }

    /// The format carrying `codec` within this one's RTP timing, if one exists.
    ///
    /// Two formats are interchangeable mid-call only when every timing field matches: payload type,
    /// clock rate, timestamp step, sample rate, channels, samples per frame -- and the signalling
    /// rate. Swapping such a pair changes no RTP header byte, so the peer has nothing to recover
    /// from and there is nothing to re-signal. Exactly one pair qualifies today, and the comparison
    /// below is what keeps that honest if a profile is ever added.
    ///
    /// The signalling rate belongs in that list even though it is not RTP timing: it is the
    /// `<audio rate>` the peer was told, so a pair differing there is precisely the case
    /// "nothing to re-signal" cannot claim. Omitting it let a format built on this payload type but
    /// signalled at another rate be switched live to the built-in 16 kHz sibling.
    #[must_use]
    pub fn sibling_for(self, codec: AudioCodec) -> Option<Self> {
        // Exhaustive on purpose: a new codec has to declare which format carries it instead of
        // falling into a wildcard that silently refuses every switch.
        let candidate = match codec {
            AudioCodec::Mlow => Self::MLOW_16KHZ_60MS,
            AudioCodec::Opus => Self::OPUS_16KHZ_60MS,
        };
        let same_timing = candidate.rtp_payload_type == self.rtp_payload_type
            && candidate.rtp_clock_rate == self.rtp_clock_rate
            && candidate.rtp_timestamp_step == self.rtp_timestamp_step
            && candidate.samples_per_frame == self.samples_per_frame
            && candidate.sample_rate == self.sample_rate
            && candidate.channels == self.channels
            && candidate.signaling_rate == self.signaling_rate;
        same_timing.then_some(candidate)
    }

    pub(crate) fn is_valid(self) -> bool {
        self.signaling_rate != 0
            && self.sample_rate != 0
            && self.channels != 0
            && self.samples_per_frame != 0
            && self.rtp_clock_rate != 0
            && self.rtp_timestamp_step != 0
            && self.rtp_payload_type <= 127
    }
}

fn is_mlow_embedded_opus(payload: &[u8]) -> bool {
    payload.first().is_some_and(|byte| byte & 0xC0 == 0xC0)
}

/// An audio codec the core cannot implement, supplied by the platform.
///
/// `wacore` is sans-io and builds for wasm32 and ESP32, so it cannot link libopus. MLow is pure
/// Rust and lives here; standard Opus does not. This is the seam: a runtime that has libopus hands
/// one of these to the engine, and one that does not passes `None` and gets an honest
/// [`crate::voip::CallEvent::AudioSilent`] instead of a call that pretends.
///
/// Implementations are stateful and per call. The engine owns exactly one and drives it from a
/// single task, so the bound is `Send` and not `Send + Sync`: nothing here is ever shared by
/// reference, and requiring `Sync` would exclude every real codec binding (libopus's decoder is
/// `Send` but not `Sync`) for a guarantee no caller needs.
pub trait ForeignAudioCodec: crate::sync_marker::MaybeSend {
    /// Decode one payload, appending samples to `out`. `out` is reused across calls and arrives
    /// empty; append rather than assigning so the caller keeps its allocation.
    fn decode(&mut self, payload: &[u8], out: &mut Vec<i16>) -> Result<(), ForeignCodecError>;

    /// Append `samples` of concealment for a packet that was lost or could not be decoded.
    fn conceal(&mut self, samples: usize, out: &mut Vec<i16>);

    /// Encode one frame of PCM, appending to `out` under the same contract as `decode`.
    fn encode(&mut self, pcm: &[i16], out: &mut Vec<u8>) -> Result<(), ForeignCodecError>;
}

/// Makes one [`ForeignAudioCodec`] per stream that needs one.
///
/// A group call needs a decoder PER PARTICIPANT: these codecs carry inter-frame state, so feeding
/// two speakers through one instance corrupts both. A single injected codec cannot serve them, and
/// the engine cannot clone one, so a runtime that has libopus supplies this instead and the engine
/// mints a decoder the first time each participant is heard from.
pub trait ForeignAudioCodecFactory: crate::sync_marker::MaybeSend {
    /// A fresh decoder, or `None` if one cannot be built right now. `None` is reported the same way
    /// a missing codec is on the direct path: honest silence, never a pretend decode.
    fn create(&self) -> Option<Box<dyn ForeignAudioCodec>>;
}

/// Why an injected codec refused a frame. Deliberately opaque: the engine counts and conceals, and
/// the specific complaint belongs in the implementation's own log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ForeignCodecError {
    #[error("the payload is not valid for this codec")]
    InvalidPayload,
    #[error("the frame size is not one this codec accepts")]
    BadFrameSize,
}

/// Failure while translating between RFC Opus and MLOW's CELT packet header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum OpusMlowPacketError {
    #[error("empty Opus packet")]
    Empty,
    #[error("Opus config {0} is not CELT-only")]
    NotCelt(u8),
    #[error("invalid Opus frame packing")]
    InvalidFramePacking,
    #[error("packet is not an MLOW CELT escape")]
    NotMlowEscape,
}

/// Rewrite one RFC Opus CELT packet for MLOW's in-profile escape.
///
/// Code-0 and code-3 packets are changed in place without allocation. The two-frame RFC shorthand
/// forms need one inserted descriptor byte. Codec payload bytes are never transcoded.
pub fn packetize_opus_for_mlow(packet: &mut Vec<u8>) -> Result<(), OpusMlowPacketError> {
    let Some(&toc) = packet.first() else {
        return Err(OpusMlowPacketError::Empty);
    };
    // libopus uses packets of at most two bytes for DTX, including a repacketized 60 ms frame.
    // MLOW needs its SID token so the peer does not consume the speech-resume RTP marker.
    if packet.len() <= 2 {
        packet[0] = 0x90;
        packet.truncate(1);
        return Ok(());
    }
    let config = toc >> 3;
    if config < 16 {
        return Err(OpusMlowPacketError::NotCelt(config));
    }
    // General Opus DTX has a hangover. Its canonical CELT silence frames still carry the MLOW VAD
    // bit after a TOC rewrite, which can consume the receiver's speech-resume marker before speech.
    if opus_celt_packet_is_silence(packet) {
        packet.clear();
        packet.push(0x90);
        return Ok(());
    }

    let packing = toc & 0x03;
    match packing {
        0 => {}
        1 => {
            let body_len = packet.len() - 1;
            if body_len == 0 || !body_len.is_multiple_of(2) {
                return Err(OpusMlowPacketError::InvalidFramePacking);
            }
            packet.insert(1, 2);
        }
        2 => {
            if !valid_two_frame_vbr_body(&packet[1..]) {
                return Err(OpusMlowPacketError::InvalidFramePacking);
            }
            packet.insert(1, 0x82);
        }
        3 => {
            if packet.get(1).is_none_or(|descriptor| {
                let frame_count = descriptor & 0x3F;
                frame_count == 0 || frame_count > 48
            }) {
                return Err(OpusMlowPacketError::InvalidFramePacking);
            }
        }
        _ => unreachable!(),
    }

    let mode = config - 16;
    let stereo = (toc >> 2) & 1;
    packet[0] = 0xC0 | mode << 2 | stereo << 1 | u8::from(packing != 0);
    Ok(())
}

/// Restore the RFC Opus TOC before passing an escaped MLOW payload to a stock Opus decoder.
pub fn depacketize_opus_from_mlow(packet: &mut [u8]) -> Result<(), OpusMlowPacketError> {
    let Some(&toc) = packet.first() else {
        return Err(OpusMlowPacketError::Empty);
    };
    if toc & 0xC0 != 0xC0 {
        return Err(OpusMlowPacketError::NotMlowEscape);
    }
    let multiple = toc & 1 != 0;
    if multiple
        && packet.get(1).is_none_or(|descriptor| {
            let frame_count = descriptor & 0x3F;
            frame_count == 0 || frame_count > 48
        })
    {
        return Err(OpusMlowPacketError::InvalidFramePacking);
    }

    let config = 16 + ((toc >> 2) & 0x0F);
    let stereo = (toc >> 1) & 1;
    packet[0] = config << 3 | stereo << 2 | if multiple { 3 } else { 0 };
    Ok(())
}

fn valid_two_frame_vbr_body(body: &[u8]) -> bool {
    let Some(&first) = body.first() else {
        return false;
    };
    let (size, field_len) = if first < 252 {
        (usize::from(first), 1)
    } else {
        let Some(&second) = body.get(1) else {
            return false;
        };
        (usize::from(first) + 4 * usize::from(second), 2)
    };
    size > 0 && body.len().saturating_sub(field_len + size) > 0
}

fn opus_celt_packet_is_silence(packet: &[u8]) -> bool {
    fn frame_is_silence(frame: &[u8]) -> bool {
        frame.is_empty() || frame == [0xFF, 0xFE]
    }

    fn parse_size(packet: &[u8], offset: &mut usize) -> Option<usize> {
        let first = *packet.get(*offset)?;
        *offset += 1;
        if first < 252 {
            return Some(usize::from(first));
        }
        let second = *packet.get(*offset)?;
        *offset += 1;
        Some(usize::from(first) + 4 * usize::from(second))
    }

    let Some(&toc) = packet.first() else {
        return false;
    };
    match toc & 0x03 {
        0 => frame_is_silence(&packet[1..]),
        1 => {
            let body = &packet[1..];
            body.len().is_multiple_of(2)
                && frame_is_silence(&body[..body.len() / 2])
                && frame_is_silence(&body[body.len() / 2..])
        }
        2 => {
            let mut offset = 1;
            let Some(first_len) = parse_size(packet, &mut offset) else {
                return false;
            };
            let Some(split) = offset.checked_add(first_len) else {
                return false;
            };
            split <= packet.len()
                && frame_is_silence(&packet[offset..split])
                && frame_is_silence(&packet[split..])
        }
        3 => {
            let Some(&descriptor) = packet.get(1) else {
                return false;
            };
            let frame_count = usize::from(descriptor & 0x3F);
            if frame_count == 0 || frame_count > 48 {
                return false;
            }
            let mut offset = 2;
            let mut padding = 0usize;
            if descriptor & 0x40 != 0 {
                loop {
                    let Some(&byte) = packet.get(offset) else {
                        return false;
                    };
                    offset += 1;
                    let added = if byte == 255 { 254 } else { usize::from(byte) };
                    let Some(total) = padding.checked_add(added) else {
                        return false;
                    };
                    padding = total;
                    if byte != 255 {
                        break;
                    }
                }
            }
            let Some(payload_end) = packet.len().checked_sub(padding) else {
                return false;
            };
            if offset > payload_end {
                return false;
            }
            if descriptor & 0x80 == 0 {
                let body = &packet[offset..payload_end];
                if !body.len().is_multiple_of(frame_count) {
                    return false;
                }
                let frame_len = body.len() / frame_count;
                return frame_len == 0 || body.chunks(frame_len).all(frame_is_silence);
            }

            let mut lengths = [0usize; 48];
            let mut encoded_len = 0usize;
            for length in &mut lengths[..frame_count - 1] {
                let Some(parsed) = parse_size(packet, &mut offset) else {
                    return false;
                };
                let Some(total) = encoded_len.checked_add(parsed) else {
                    return false;
                };
                encoded_len = total;
                *length = parsed;
            }
            let Some(last_len) = payload_end
                .checked_sub(offset)
                .and_then(|remaining| remaining.checked_sub(encoded_len))
            else {
                return false;
            };
            lengths[frame_count - 1] = last_len;
            lengths[..frame_count].iter().all(|&length| {
                let Some(end) = offset.checked_add(length) else {
                    return false;
                };
                let silence = end <= payload_end && frame_is_silence(&packet[offset..end]);
                offset = end;
                silence
            }) && offset == payload_end
        }
        _ => unreachable!(),
    }
}

/// Audio configuration owned by a call engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AudioConfig {
    pub format: AudioFormat,
    pub io: AudioIo,
}

impl AudioConfig {
    pub const MLOW_PCM: Self = Self {
        format: AudioFormat::MLOW_16KHZ_60MS,
        io: AudioIo::Pcm,
    };

    /// The other half of the swappable pair, for a peer outside the MLOW rollout.
    ///
    /// PCM I/O admits exactly these two formats, so they are named rather than left to a general
    /// constructor: this one needs a [`ForeignAudioCodec`], since standard Opus is not something
    /// `wacore` can implement, and the engine reports [`crate::voip::CallEvent::AudioSilent`]
    /// rather than pretending when none was installed.
    pub const OPUS_PCM: Self = Self {
        format: AudioFormat::OPUS_16KHZ_60MS,
        io: AudioIo::Pcm,
    };

    pub const fn encoded(format: AudioFormat) -> Self {
        Self {
            format,
            io: AudioIo::Encoded,
        }
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self::MLOW_PCM
    }
}

/// One decrypted codec payload received from the peer.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct EncodedAudioFrame {
    pub format: AudioFormat,
    /// Codec detected within the negotiated RTP profile for this packet.
    pub codec: AudioCodec,
    pub data: Bytes,
    /// Actual RTP payload type. MLOW redundancy uses PT 121 while the primary format uses PT 120.
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub marker: bool,
    /// Group sender identity. Absent on 1:1 audio.
    pub sender: Option<Jid>,
    /// Group sender device identity. Absent on 1:1 audio.
    pub device: Option<Jid>,
    /// Relay participant id from the authoritative roster.
    pub pid: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // The comparison exists so that "nothing to re-signal" stays true of any pair it admits. It
    // checked every RTP timing field and not the signalling rate -- which is the one thing in an
    // `AudioFormat` the peer learns from the `<audio rate>` rather than from the packets. A format
    // on this payload type signalled at another rate could therefore be switched live to the
    // built-in 16 kHz sibling, mid-call, against a peer that was told something else.
    #[test]
    fn a_sibling_has_to_agree_on_the_rate_the_peer_was_signalled() {
        let standard = AudioFormat::MLOW_16KHZ_60MS;
        assert_eq!(
            standard.sibling_for(AudioCodec::Opus),
            Some(AudioFormat::OPUS_16KHZ_60MS),
            "the one documented interchangeable pair still swaps"
        );

        let resignalled = AudioFormat {
            signaling_rate: 8_000,
            ..AudioFormat::MLOW_16KHZ_60MS
        };
        assert_eq!(
            resignalled.sibling_for(AudioCodec::Opus),
            None,
            "but not into a format the peer was never told about"
        );
    }

    #[test]
    fn native_opus_codec_selection_is_independent_from_rtp_clock_profile() {
        let native = AudioFormat::OPUS_16KHZ_60MS;
        assert_eq!(native.rtp_profile, AudioRtpProfile::StandardOpus);
        assert_eq!(native.rtp_payload_type, RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO);
        assert_eq!(native.rtp_clock_rate, 16_000);
        assert_eq!(native.rtp_timestamp_step, 960);

        let rfc7587 = AudioFormat::OPUS_RFC7587_16KHZ_60MS;
        assert_eq!(rfc7587.rtp_profile, AudioRtpProfile::StandardOpus);
        assert_eq!(rfc7587.rtp_payload_type, RTP_PAYLOAD_TYPE_OPUS);
        assert_eq!(rfc7587.rtp_clock_rate, 48_000);
        assert_eq!(rfc7587.rtp_timestamp_step, 2_880);
    }

    #[test]
    fn mlow_profile_accepts_primary_and_redundancy_payload_types() {
        assert!(AudioFormat::OPUS_MLOW_16KHZ_60MS.accepts_rtp_payload_type(RTP_PAYLOAD_TYPE_MLOW));
        assert!(
            AudioFormat::OPUS_MLOW_16KHZ_60MS.accepts_rtp_payload_type(RTP_PAYLOAD_TYPE_MLOW_RED)
        );
        assert!(!AudioFormat::OPUS_MLOW_16KHZ_60MS.accepts_rtp_payload_type(RTP_PAYLOAD_TYPE_OPUS));
    }

    #[test]
    fn mlow_profile_classifies_embedded_opus_only_on_primary_pt() {
        let format = AudioFormat::OPUS_MLOW_16KHZ_60MS;
        assert_eq!(
            format.inbound_codec(RTP_PAYLOAD_TYPE_MLOW, &[0xF8, 1, 2]),
            AudioCodec::Opus
        );
        assert_eq!(
            format.inbound_codec(RTP_PAYLOAD_TYPE_MLOW, &[0x50, 1, 2]),
            AudioCodec::Mlow
        );
        assert_eq!(
            format.inbound_codec(RTP_PAYLOAD_TYPE_MLOW_RED, &[0xC0, 1, 2]),
            AudioCodec::Mlow
        );
    }

    #[test]
    fn opus_in_mlow_requires_the_escape_toc() {
        let format = AudioFormat::OPUS_MLOW_16KHZ_60MS;
        assert!(format.accepts_encoded_payload(&[0xF8, 1, 2]));
        assert!(format.accepts_encoded_payload(&[0x90]));
        assert!(!format.accepts_encoded_payload(&[0x08, 1, 2]));
        assert!(!format.accepts_encoded_payload(&[]));
    }

    #[test]
    fn opus_celt_packet_round_trips_through_mlow_toc() {
        // RFC Opus: CELT-WB 20 ms, mono, arbitrary three-frame packet.
        let original = vec![0xBB, 0x03, 1, 2, 3, 4, 5, 6];
        let mut packet = original.clone();

        packetize_opus_for_mlow(&mut packet).unwrap();
        assert_eq!(packet[0], 0xDD);
        assert_eq!(&packet[1..], &original[1..]);
        depacketize_opus_from_mlow(&mut packet).unwrap();
        assert_eq!(packet, original);
    }

    #[test]
    fn two_frame_opus_shorthand_gets_an_explicit_descriptor() {
        let mut cbr = vec![0xB9, 1, 2, 3, 4];
        packetize_opus_for_mlow(&mut cbr).unwrap();
        assert_eq!(&cbr[..2], &[0xDD, 2]);

        let mut vbr = vec![0xBA, 2, 1, 2, 3];
        packetize_opus_for_mlow(&mut vbr).unwrap();
        assert_eq!(&vbr[..3], &[0xDD, 0x82, 2]);
    }

    #[test]
    fn opus_dtx_maps_to_mlow_sid() {
        let mut packet = vec![0xB8];
        packetize_opus_for_mlow(&mut packet).unwrap();
        assert_eq!(packet, [0x90]);

        let mut repacketized_60_ms = vec![0xBB, 0x03];
        packetize_opus_for_mlow(&mut repacketized_60_ms).unwrap();
        assert_eq!(repacketized_60_ms, [0x90]);
    }

    #[test]
    fn opus_celt_hangover_silence_maps_to_mlow_sid() {
        let mut cbr = vec![0xBB, 0x03, 0xFF, 0xFE, 0xFF, 0xFE, 0xFF, 0xFE];
        packetize_opus_for_mlow(&mut cbr).unwrap();
        assert_eq!(cbr, [0x90]);

        let mut vbr_refresh = vec![0xBB, 0x83, 0x02, 0x00, 0xFF, 0xFE];
        packetize_opus_for_mlow(&mut vbr_refresh).unwrap();
        assert_eq!(vbr_refresh, [0x90]);

        let mut active = vec![0xBB, 0x03, 0xFF, 0xFD, 0xFF, 0xFE, 0xFF, 0xFE];
        packetize_opus_for_mlow(&mut active).unwrap();
        assert_eq!(active[0], 0xDD);
    }

    #[test]
    fn silk_opus_cannot_use_mlow_celt_escape() {
        let mut silk = vec![0x08, 1, 2, 3];
        assert_eq!(
            packetize_opus_for_mlow(&mut silk),
            Err(OpusMlowPacketError::NotCelt(1))
        );
    }

    #[test]
    fn opus_escape_rejects_more_than_48_frames() {
        let mut packet = vec![0xBB, 49, 1, 2, 3];
        assert_eq!(
            packetize_opus_for_mlow(&mut packet),
            Err(OpusMlowPacketError::InvalidFramePacking)
        );

        let mut escaped = [0xDD, 49, 1, 2, 3];
        assert_eq!(
            depacketize_opus_from_mlow(&mut escaped),
            Err(OpusMlowPacketError::InvalidFramePacking)
        );
    }
}
