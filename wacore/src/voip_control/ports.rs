//! The platform media endpoints a call reads from and writes to, and the frame type video flows as.
//!
//! These are the ports a media backend needs to actually move media: a microphone source, a
//! speaker sink, and the encoded/video equivalents. They live in the neutral contract so a foreign
//! backend can name them without the engine, and so the resident backend can be handed the same
//! ports the facade already collects. The `voip` crate re-exports every one under its historical
//! `whatsapp_rust::voip::*` path.

use std::sync::Arc;

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

/// A pre-encoded video access unit with an RTP-clock capture timestamp, as the drive loop consumes
/// it: the source generation lets a replaced source's stale AUs be discarded.
#[derive(Debug, Clone)]
pub struct VideoInput {
    /// Complete Annex-B H.264 access unit.
    pub data: Vec<u8>,
    /// Capture timestamp in the 90 kHz RTP clock, compared modulo `u32`.
    pub timestamp: u32,
    /// Source generation assigned by the caller. Stale generations are discarded at the driver.
    pub generation: u64,
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

/// The audio ports a session reads and writes, selected by the negotiated I/O mode.
pub enum MediaAudioPorts {
    /// PCM frames in and out: a microphone source and a speaker sink.
    Pcm {
        source: Arc<dyn AudioSource>,
        sink: Arc<dyn AudioSink>,
    },
    /// Codec payloads in and out, transcoded nowhere.
    Encoded {
        source: Arc<dyn EncodedAudioSource>,
        sink: Arc<dyn EncodedAudioSink>,
    },
}

/// The optional video ports a session reads and writes.
pub struct MediaVideoPorts {
    pub source: Arc<dyn VideoSource>,
    pub sink: Arc<dyn VideoSink>,
}

/// Everything a backend needs to bring a reserved session operational, besides the spec.
///
/// This is the neutral opening context: the platform's endpoints, the public event sink, and the
/// one-shot recv-rekey source. The executor and the relay transport are deliberately absent -- they
/// are Rust trait objects that cannot cross a process boundary, so they are constructor state of
/// the backend, never context (F9).
///
/// A backend's [`open`](super::VoipMediaBackend::open) owns the rest of the lifecycle: it builds
/// its media, wires the session's own mailboxes, and starts driving. The control plane never hands
/// a backend its internal mailboxes through the contract.
/// The drive-loop halves of the video channels, pre-created by the control plane.
///
/// The control plane creates the video plumbing once at registration (so a dormant handle can
/// attach or detach endpoints before media exists) and hands the loop's halves in here. The
/// backend must use these rather than create its own, or the sender the handle steers would reach
/// a different channel.
pub struct MediaVideoChannels {
    /// Plane control (enable/disable/orientation/keyframe) the drive loop reads.
    pub control: super::control::VideoControlReceiver,
    /// The sender half of `control`. The backend installs it on the session so
    /// `submit(MediaCommand::EnableVideo …)` reaches this receiver.
    pub control_sender: super::control::VideoControlSender,
    /// Outbound AUs produced by the source feed.
    pub video_in: async_channel::Receiver<Vec<u8>>,
    /// Optional capture-timestamped AUs.
    pub timed_video_in: Option<async_channel::Receiver<VideoInput>>,
    /// Reassembled peer AUs the loop writes; the control plane drains this to the sink.
    pub video_out: async_channel::Sender<VideoFrame>,
}

pub struct MediaOpenContext {
    pub audio: MediaAudioPorts,
    pub video: Option<MediaVideoPorts>,
    /// The drive-loop video halves, when the caller pre-created the video plumbing.
    pub video_channels: Option<MediaVideoChannels>,
    /// The public, ordered call event sink the handle reads.
    pub events: async_channel::Sender<super::CallEvent>,
    /// The caller-only recv-rekey receiver; `None` on the callee side.
    pub rekey: Option<async_channel::Receiver<super::control::PeerAnswer>>,
    /// The microphone mute flag, shared with the consumer's `CallHandle`. A backend that wraps a
    /// PCM source through its own feed zeroes frames while this is set.
    pub muted: Arc<std::sync::atomic::AtomicBool>,
    /// A raw keygen-v2 epoch the caller already authenticated and fanned out, to install on the
    /// engine before media starts. A group call that needed its initiator's epoch applied holds it
    /// here rather than on the engine, because the backend builds the engine.
    pub group_epoch: Option<(u32, super::MediaGroupEpoch)>,
    /// A codec the caller selected from the peer's capability that the engine must adopt before its
    /// first packet, without changing the source's grammar. `None` when the negotiated format is
    /// already correct.
    pub initial_codec: Option<super::MediaAudioCodec>,
}

impl MediaOpenContext {
    /// A minimal context for tests that only need `open` to be callable: stub ports, an event sink
    /// nobody reads, and no rekey.
    #[must_use]
    pub fn for_test() -> Self {
        let (events, events_rx) = async_channel::bounded(1);
        events_rx.close();
        let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(1);
        mic_rx.close();
        drop(mic_tx);
        let (speaker, speaker_rx) = async_channel::bounded::<Vec<i16>>(1);
        speaker_rx.close();
        Self {
            audio: MediaAudioPorts::Pcm {
                source: Arc::new(mic_rx),
                sink: Arc::new(speaker),
            },
            video: None,
            video_channels: None,
            events,
            rekey: None,
            muted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            group_epoch: None,
            initial_codec: None,
        }
    }
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
