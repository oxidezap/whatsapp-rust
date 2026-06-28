//! Opus audio for WhatsApp calls. Standard libopus (the proprietary "mlow" encode is not
//! portable) at 16 kHz mono, Voip mode, 60 ms frames, which the peer accepts. Priming frames
//! are hardcoded constants (see `wacore::voip::rtp`) and bypass the encoder.

use anyhow::{Result, anyhow};
use opus::{Application, Bitrate, Channels, Decoder, Encoder};

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

pub const WA_SAMPLE_RATE: u32 = 16_000;
/// 60 ms @ 16 kHz.
pub const WA_FRAME_SAMPLES: usize = 960;
/// Max decode frame (60 ms @ 48 kHz headroom).
pub const WA_DECODE_MAX_SAMPLES: usize = 2880;
const WA_BITRATE: i32 = 25_000;
const WA_COMPLEXITY: i32 = 9;

/// Opus encoder configured to match the WhatsApp call encoder (minus the mlow layers).
pub struct WaOpusEncoder {
    enc: Encoder,
}

impl WaOpusEncoder {
    pub fn new() -> Result<Self> {
        let mut enc = Encoder::new(WA_SAMPLE_RATE, Channels::Mono, Application::Voip)
            .map_err(|e| anyhow!("opus encoder init: {e}"))?;
        enc.set_bitrate(Bitrate::Bits(WA_BITRATE))
            .map_err(|e| anyhow!("opus set_bitrate: {e}"))?;
        enc.set_complexity(WA_COMPLEXITY)
            .map_err(|e| anyhow!("opus set_complexity: {e}"))?;
        Ok(Self { enc })
    }

    /// Encode one mono frame of `WA_FRAME_SAMPLES` 16-bit samples to Opus.
    pub fn encode(&mut self, pcm: &[i16]) -> Result<Vec<u8>> {
        debug_assert_eq!(
            pcm.len(),
            WA_FRAME_SAMPLES,
            "WaOpusEncoder expects exactly one {WA_FRAME_SAMPLES}-sample frame"
        );
        self.enc
            .encode_vec(pcm, pcm.len() * 2 + 64)
            .map_err(|e| anyhow!("opus encode: {e}"))
    }
}

/// Opus decoder (mono, 16 kHz).
pub struct WaOpusDecoder {
    dec: Decoder,
}

impl WaOpusDecoder {
    pub fn new() -> Result<Self> {
        let dec = Decoder::new(WA_SAMPLE_RATE, Channels::Mono)
            .map_err(|e| anyhow!("opus decoder init: {e}"))?;
        Ok(Self { dec })
    }

    /// Decode one Opus frame to mono 16-bit PCM, returning the decoded samples.
    pub fn decode(&mut self, opus: &[u8]) -> Result<Vec<i16>> {
        let mut out = vec![0i16; WA_DECODE_MAX_SAMPLES];
        let n = self
            .dec
            .decode(opus, &mut out, false)
            .map_err(|e| anyhow!("opus decode: {e}"))?;
        out.truncate(n);
        Ok(out)
    }
}

#[cfg(test)]
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
}
