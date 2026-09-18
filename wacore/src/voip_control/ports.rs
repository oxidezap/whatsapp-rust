//! The platform media endpoints a call reads from and writes to, and the frame type video flows as.
//!
//! These are the ports a media backend needs to actually move media: a microphone source, a
//! speaker sink, and the encoded/video equivalents. They live in the neutral contract so a foreign
//! backend can name them without the engine, and so the resident backend can be handed the same
//! ports the facade already collects. The `voip` crate re-exports every one under its historical
//! `whatsapp_rust::voip::*` path.

use bytes::Bytes;

use super::MediaEncodedFrame;

/// A microphone source for a call: 60 ms / 960-sample mono i16 frames at 16 kHz.
///
/// The media backend pulls frames from the returned channel; a closed channel (e.g. the OS muted the
/// device) does NOT end the call. Channel-factory shaped so a producer can run on its own task and
/// the backend can select on the receiver directly. Frames MUST be exactly 960 samples.
pub trait AudioSource: Send + Sync + 'static {
    fn frames(&self) -> async_channel::Receiver<Vec<i16>>;
}

/// A speaker sink for a call: decoded 16 kHz mono i16 playout frames.
///
/// A backend drops a frame if the sink cannot keep up; VoIP is loss tolerant.
pub trait AudioSink: Send + Sync + 'static {
    fn playout(&self) -> async_channel::Sender<Vec<i16>>;
}

/// A source of complete codec payloads: one raw MLOW or profile-compatible Opus packet per item.
pub trait EncodedAudioSource: Send + Sync + 'static {
    fn frames(&self) -> async_channel::Receiver<Bytes>;
}

/// A sink for decrypted codec payloads with their original RTP metadata.
pub trait EncodedAudioSink: Send + Sync + 'static {
    fn frames(&self) -> async_channel::Sender<MediaEncodedFrame>;
}

/// One received access unit, reassembled back into Annex-B form.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VideoFrame {
    /// Annex-B access unit (`00 00 00 01` start codes included).
    pub data: Vec<u8>,
    /// The AU carries an IDR/SPS/PPS NAL: safe point to (re)start a decoder.
    pub keyframe: bool,
    /// Frame rotation bits (0..3) from RTP metadata.
    pub orientation: u8,
    /// Group sender identity. Absent on 1:1 video.
    pub sender: Option<wacore_binary::Jid>,
    /// Group sender device identity. Absent on 1:1 video.
    pub device: Option<wacore_binary::Jid>,
    /// Relay participant id from the authoritative roster.
    pub pid: Option<u32>,
    /// RTP capture timestamp of the access unit (90 kHz video clock).
    pub timestamp: u32,
    /// Call media generation that produced this frame.
    pub generation: u64,
}

/// One captured access unit with its 90 kHz RTP capture timestamp.
#[derive(Debug, Clone)]
pub struct TimedVideoFrame {
    pub data: Vec<u8>,
    pub timestamp: u32,
}

/// Video RTP timestamp clock (90 kHz).
pub const VIDEO_CLOCK_RATE: u32 = 90_000;
/// RTP clock increment per access unit at the reference 15 fps cadence.
pub const VIDEO_TS_STRIDE_15FPS: u32 = VIDEO_CLOCK_RATE / 15;

/// A video source for a call: one complete H.264 Annex-B access unit per item.
pub trait VideoSource: Send + Sync + 'static {
    fn frames(&self) -> async_channel::Receiver<Vec<u8>>;

    /// An optional capture-timestamped channel; sources without it use the fixed-cadence path.
    fn timed_frames(&self) -> Option<async_channel::Receiver<TimedVideoFrame>> {
        None
    }

    /// RTP clock increment between access units. Must match the source's pacing and be non-zero.
    fn rtp_timestamp_stride(&self) -> u32 {
        VIDEO_TS_STRIDE_15FPS
    }
}

/// A video sink for a call: reassembled peer access units with keyframe/orientation metadata.
pub trait VideoSink: Send + Sync + 'static {
    fn playout(&self) -> async_channel::Sender<VideoFrame>;
}

// Blanket impls so a bare `async_channel` endpoint is usable directly as a source/sink, matching the
// audio ports: the common case is "I already have a channel".
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

impl EncodedAudioSource for async_channel::Receiver<Bytes> {
    fn frames(&self) -> async_channel::Receiver<Bytes> {
        self.clone()
    }
}

impl EncodedAudioSink for async_channel::Sender<MediaEncodedFrame> {
    fn frames(&self) -> async_channel::Sender<MediaEncodedFrame> {
        self.clone()
    }
}

impl VideoSource for async_channel::Receiver<Vec<u8>> {
    fn frames(&self) -> async_channel::Receiver<Vec<u8>> {
        self.clone()
    }
}

impl VideoSink for async_channel::Sender<VideoFrame> {
    fn playout(&self) -> async_channel::Sender<VideoFrame> {
        self.clone()
    }
}
