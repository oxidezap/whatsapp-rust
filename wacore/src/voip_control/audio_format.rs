//! Fundamental audio format types, shared by the neutral contract and the engine.
//!
//! These used to live in the `voip`-gated audio module, so the control plane could not name them
//! and carried flat twins (`MediaAudioFormat` and its codec/profile enums) with lossy conversions
//! between the two. The twins are gone: this is the one type, and the `voip` audio module
//! re-exports it, so `crate::voip::audio::AudioFormat` still resolves to the same type every
//! existing caller already uses.
//!
//! The format data and the pure helpers live here. The payload-inspecting helpers
//! (`inbound_codec`, `payload_is_mlow_escape`, `accepts_encoded_payload`) stay in the engine's
//! audio module as a second `impl`: same crate, so an inherent impl is allowed, and they reach for
//! Opus packet parsing that has no business in the control-plane contract.

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

/// RFC 7587 Opus payload type used when the 48 kHz RTP-clock setting is enabled.
pub const RTP_PAYLOAD_TYPE_OPUS: u8 = 111;
/// Default WhatsApp audio payload type; the negotiated codec decides whether its bytes are MLOW or
/// native Opus.
pub const RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO: u8 = 120;
/// MLOW name for the shared default WhatsApp audio payload type.
pub const RTP_PAYLOAD_TYPE_MLOW: u8 = RTP_PAYLOAD_TYPE_WHATSAPP_AUDIO;
/// MLOW SplitRed redundancy payload type (`mlow-red-1`).
pub const RTP_PAYLOAD_TYPE_MLOW_RED: u8 = 121;

/// Fixed audio timing for one call.
///
/// `#[non_exhaustive]` with a builder, like every other DTO on the neutral side: an external
/// backend with its own timing builds one instead of being limited to the named constants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bon::Builder)]
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

    /// The sibling format carrying `codec` at this format's exact timing, if one exists.
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

    /// Whether every timing and channel value is usable. A format with a zero field describes no
    /// real RTP stream, so a backend that validates an externally supplied format checks this.
    #[must_use]
    pub fn is_valid(self) -> bool {
        self.signaling_rate != 0
            && self.sample_rate != 0
            && self.channels != 0
            && self.samples_per_frame != 0
            && self.rtp_clock_rate != 0
            && self.rtp_timestamp_step != 0
            && self.rtp_payload_type <= 127
    }
}

/// Whether a payload's top two bits mark it as MLOW's embedded-Opus escape.
#[must_use]
pub fn is_mlow_embedded_opus(payload: &[u8]) -> bool {
    payload.first().is_some_and(|byte| byte & 0xC0 == 0xC0)
}
