//! PCM and encoded-audio endpoints for WhatsApp calls.

#[cfg(feature = "voip-libopus")]
use anyhow::{Result, anyhow, ensure};
use bytes::Bytes;
#[cfg(feature = "voip-libopus")]
use opus::{Application, Bandwidth, Bitrate, Channels, Decoder, Encoder};
use wacore::voip::EncodedAudioFrame;
#[cfg(feature = "voip-libopus")]
use wacore::voip::{depacketize_opus_from_mlow, packetize_opus_for_mlow};

/// A microphone source for a call: 60 ms / 960-sample mono i16 frames at 16 kHz. The media facade
/// pulls frames from the returned channel and feeds them to the engine; a closed channel (e.g. the
/// OS muted the device) does NOT end the call (the relay keepalive keeps running). The trait is a
/// channel factory rather than an `async fn next_frame` so a producer can run on its own task and
/// the facade can `select!` the receiver directly, matching the engine's drive loop; default
/// methods can be added here without breaking implementors.
///
/// Frames MUST be exactly 960 samples; the engine drops any other length (no RTP sent).
pub trait AudioSource: Send + Sync + 'static {
    /// The channel the facade reads mic frames from. Called once when the call starts.
    fn frames(&self) -> async_channel::Receiver<Vec<i16>>;
}

/// A speaker sink for a call: the facade pushes decoded 16 kHz mono i16 playout frames onto the
/// returned channel. VoIP is loss tolerant, so the facade drops a frame if the sink can't keep up.
/// Channel-factory shaped for the same reasons as [`AudioSource`]; default methods may be added.
pub trait AudioSink: Send + Sync + 'static {
    /// The channel the facade writes playout frames to. Called once when the call starts.
    fn playout(&self) -> async_channel::Sender<Vec<i16>>;
}

/// Blanket impls so a bare `async_channel` endpoint is usable directly as a source/sink without a
/// wrapper type: the common case is "I already have an mpsc of PCM frames".
impl AudioSource for async_channel::Receiver<Vec<i16>> {
    fn frames(&self) -> async_channel::Receiver<Vec<i16>> {
        self.clone()
    }
}

impl AudioSink for async_channel::Sender<Vec<i16>> {
    fn playout(&self) -> async_channel::Sender<Vec<i16>> {
        self.clone()
    }
}

/// A source of complete codec payloads. Each item must be one raw MLOW or profile-compatible Opus
/// packet; container framing such as Ogg pages is not accepted. The engine adds RTP/SRTP framing
/// without transcoding.
pub trait EncodedAudioSource: Send + Sync + 'static {
    fn frames(&self) -> async_channel::Receiver<Bytes>;
}

/// A sink for decrypted codec payloads with their original RTP metadata.
pub trait EncodedAudioSink: Send + Sync + 'static {
    fn frames(&self) -> async_channel::Sender<EncodedAudioFrame>;
}

impl EncodedAudioSource for async_channel::Receiver<Bytes> {
    fn frames(&self) -> async_channel::Receiver<Bytes> {
        self.clone()
    }
}

impl EncodedAudioSink for async_channel::Sender<EncodedAudioFrame> {
    fn frames(&self) -> async_channel::Sender<EncodedAudioFrame> {
        self.clone()
    }
}

pub const WA_SAMPLE_RATE: u32 = 16_000;
/// 60 ms @ 16 kHz.
pub const WA_FRAME_SAMPLES: usize = 960;
/// Opus permits at most 120 ms in one packet; the decoder emits 16 kHz PCM.
pub const WA_DECODE_MAX_SAMPLES: usize = 1_920;
#[cfg(feature = "voip-libopus")]
const WA_BITRATE: i32 = 24_000;
#[cfg(feature = "voip-libopus")]
const WA_COMPLEXITY: i32 = 5;
#[cfg(feature = "voip-libopus")]
const OPUS_MAX_PACKET_BYTES: usize = 1_275;

/// Opus encoder with constructors for standard RTP and MLOW's compatible CELT escape.
#[cfg(feature = "voip-libopus")]
pub struct WaOpusEncoder {
    enc: Encoder,
    frame_samples: usize,
    require_mlow_escape: bool,
}

#[cfg(feature = "voip-libopus")]
impl WaOpusEncoder {
    pub fn new() -> Result<Self> {
        let mut enc = Encoder::new(WA_SAMPLE_RATE, Channels::Mono, Application::Voip)
            .map_err(|e| anyhow!("opus encoder init: {e}"))?;
        Self::finish_init(&mut enc)?;
        Ok(Self {
            enc,
            frame_samples: WA_FRAME_SAMPLES,
            require_mlow_escape: false,
        })
    }

    /// Configure libopus for the standard-Opus escape inside WhatsApp's MLOW RTP profile.
    pub fn new_mlow_escape() -> Result<Self> {
        let mut enc = Encoder::new(WA_SAMPLE_RATE, Channels::Mono, Application::LowDelay)
            .map_err(|e| anyhow!("opus MLOW-escape encoder init: {e}"))?;
        Self::finish_init(&mut enc)?;
        enc.set_max_bandwidth(Bandwidth::Wideband)
            .map_err(|e| anyhow!("opus set_max_bandwidth: {e}"))?;
        enc.set_bandwidth(Bandwidth::Wideband)
            .map_err(|e| anyhow!("opus set_bandwidth: {e}"))?;
        Ok(Self {
            enc,
            frame_samples: WA_FRAME_SAMPLES,
            require_mlow_escape: true,
        })
    }

    fn finish_init(enc: &mut Encoder) -> Result<()> {
        enc.set_bitrate(Bitrate::Bits(WA_BITRATE))
            .map_err(|e| anyhow!("opus set_bitrate: {e}"))?;
        enc.set_complexity(WA_COMPLEXITY)
            .map_err(|e| anyhow!("opus set_complexity: {e}"))?;
        enc.set_dtx(true)
            .map_err(|e| anyhow!("opus set_dtx: {e}"))?;
        Ok(())
    }

    /// Encode one mono frame at the rate selected by the constructor.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        ensure!(
            pcm.len() == self.frame_samples,
            "WaOpusEncoder expects exactly {} samples, got {}",
            self.frame_samples,
            pcm.len()
        );
        let mut payload = self
            .enc
            .encode_vec(pcm, OPUS_MAX_PACKET_BYTES)
            .map_err(|e| anyhow!("opus encode: {e}"))?;
        if self.require_mlow_escape {
            packetize_opus_for_mlow(&mut payload)
                .map_err(|e| anyhow!("packetize Opus for MLOW: {e}"))?;
        }
        Ok(payload)
    }
}

/// Opus decoder (mono, 16 kHz).
#[cfg(feature = "voip-libopus")]
pub struct WaOpusDecoder {
    dec: Decoder,
    packet_scratch: Vec<u8>,
    pcm_scratch: Vec<i16>,
}

#[cfg(feature = "voip-libopus")]
impl WaOpusDecoder {
    pub fn new() -> Result<Self> {
        let dec = Decoder::new(WA_SAMPLE_RATE, Channels::Mono)
            .map_err(|e| anyhow!("opus decoder init: {e}"))?;
        Ok(Self {
            dec,
            packet_scratch: Vec::with_capacity(OPUS_MAX_PACKET_BYTES),
            pcm_scratch: vec![0; WA_DECODE_MAX_SAMPLES],
        })
    }

    /// Decode one Opus frame to mono 16-bit PCM. The returned view is valid until the next decode.
    pub fn decode(&mut self, opus: &[u8]) -> Result<&[i16]> {
        Self::decode_packet(&mut self.dec, &mut self.pcm_scratch, opus)
    }

    fn decode_packet<'a>(
        dec: &mut Decoder,
        pcm_scratch: &'a mut Vec<i16>,
        opus: &[u8],
    ) -> Result<&'a [i16]> {
        pcm_scratch.resize(WA_DECODE_MAX_SAMPLES, 0);
        let n = dec
            .decode(opus, pcm_scratch, false)
            .map_err(|e| anyhow!("opus decode: {e}"))?;
        pcm_scratch.truncate(n);
        Ok(pcm_scratch)
    }

    /// Restore MLOW's CELT TOC and decode with stock libopus.
    pub fn decode_mlow_escape(&mut self, opus: &[u8]) -> Result<&[i16]> {
        self.packet_scratch.clear();
        self.packet_scratch.extend_from_slice(opus);
        depacketize_opus_from_mlow(&mut self.packet_scratch)
            .map_err(|e| anyhow!("depacketize Opus from MLOW: {e}"))?;
        Self::decode_packet(&mut self.dec, &mut self.pcm_scratch, &self.packet_scratch)
    }
}

#[cfg(all(test, feature = "voip-libopus"))]
mod tests {
    use super::*;

    /// A 440 Hz sine over one 60 ms frame (mono, 16 kHz).
    fn sine_frame() -> Vec<i16> {
        (0..WA_FRAME_SAMPLES)
            .map(|i| {
                let t = i as f32 / WA_SAMPLE_RATE as f32;
                (16000.0 * (2.0 * std::f32::consts::PI * 440.0 * t).sin()) as i16
            })
            .collect()
    }

    #[test]
    fn opus_round_trip_recovers_a_frame() {
        let mut enc = WaOpusEncoder::new().unwrap();
        let mut dec = WaOpusDecoder::new().unwrap();
        let pcm = sine_frame();
        let encoded = enc.encode(&pcm).unwrap();
        assert!(!encoded.is_empty(), "encoder produced bytes");
        let decoded = dec.decode(&encoded).unwrap();
        // Opus is lossy but frame-length preserving: one 60 ms frame decodes to 960 samples.
        assert_eq!(decoded.len(), WA_FRAME_SAMPLES);
    }

    #[test]
    fn silence_encodes_and_decodes() {
        let mut enc = WaOpusEncoder::new().unwrap();
        let mut dec = WaOpusDecoder::new().unwrap();
        let silence = vec![0i16; WA_FRAME_SAMPLES];
        let encoded = enc.encode(&silence).unwrap();
        let decoded = dec.decode(&encoded).unwrap();
        assert_eq!(decoded.len(), WA_FRAME_SAMPLES);
    }

    #[test]
    fn decoder_reuses_pcm_scratch() {
        let mut enc = WaOpusEncoder::new().unwrap();
        let mut dec = WaOpusDecoder::new().unwrap();
        let encoded = enc.encode(&sine_frame()).unwrap();

        let first = dec.decode(&encoded).unwrap().as_ptr();
        let second = dec.decode(&encoded).unwrap().as_ptr();

        assert_eq!(first, second);
    }

    #[test]
    fn mlow_escape_encoder_emits_celt_toc_and_60ms_packet() {
        let mut enc = WaOpusEncoder::new_mlow_escape().unwrap();
        let mut dec = WaOpusDecoder::new().unwrap();
        let pcm = sine_frame();
        let encoded = enc.encode(&pcm).unwrap();

        assert_eq!(encoded[0], 0xDD);
        assert_eq!(encoded[1] & 0x3F, 3);
        assert_eq!(
            dec.decode_mlow_escape(&encoded).unwrap().len(),
            WA_FRAME_SAMPLES
        );
    }

    #[test]
    fn mlow_escape_encoder_maps_opus_dtx_to_mlow_sid() {
        let mut enc = WaOpusEncoder::new_mlow_escape().unwrap();
        let silence = vec![0i16; WA_FRAME_SAMPLES];

        let encoded = (0..20)
            .map(|_| enc.encode(&silence).unwrap())
            .find(|packet| packet.as_slice() == [0x90]);

        assert!(
            encoded.is_some(),
            "Opus did not enter DTX within 1.2 seconds"
        );
    }

    #[test]
    fn mlow_escape_encoder_maps_initial_silence_to_mlow_sid() {
        let mut enc = WaOpusEncoder::new_mlow_escape().unwrap();
        let silence = vec![0i16; WA_FRAME_SAMPLES];

        assert_eq!(enc.encode(&silence).unwrap(), [0x90]);
    }
}
