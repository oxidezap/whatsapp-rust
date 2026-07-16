//! Audio format and I/O contracts shared by the sans-I/O engine and platform drivers.

use bytes::Bytes;

use super::rtp::{RTP_PAYLOAD_TYPE_MLOW, RTP_PAYLOAD_TYPE_MLOW_RED, RTP_PAYLOAD_TYPE_OPUS};

/// Codec carried inside WhatsApp's audio RTP payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioCodec {
    Mlow,
    Opus,
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
    pub codec: AudioCodec,
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
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 16_000,
        rtp_timestamp_step: 960,
        rtp_payload_type: RTP_PAYLOAD_TYPE_MLOW,
    };

    /// Standard Opus with a 16 kHz codec adapter and RFC 7587's 48 kHz RTP clock.
    pub const OPUS_16KHZ_60MS: Self = Self {
        codec: AudioCodec::Opus,
        signaling_rate: 8_000,
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
        signaling_rate: 8_000,
        sample_rate: 48_000,
        channels: 1,
        samples_per_frame: 2_880,
        rtp_clock_rate: 48_000,
        rtp_timestamp_step: 2_880,
        rtp_payload_type: RTP_PAYLOAD_TYPE_OPUS,
    };

    /// WhatsApp's signaling rate selects a media profile rather than the Opus PCM rate.
    pub const fn from_signaling_rate(rate: u32) -> Option<Self> {
        match rate {
            16_000 => Some(Self::MLOW_16KHZ_60MS),
            8_000 => Some(Self::OPUS_16KHZ_60MS),
            _ => None,
        }
    }

    pub const fn accepts_rtp_payload_type(self, payload_type: u8) -> bool {
        payload_type == self.rtp_payload_type
            || matches!(self.codec, AudioCodec::Mlow) && payload_type == RTP_PAYLOAD_TYPE_MLOW_RED
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
    pub data: Bytes,
    /// Actual RTP payload type. MLOW redundancy uses PT 121 while the primary format uses PT 120.
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub marker: bool,
}
