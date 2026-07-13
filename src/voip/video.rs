//! Video endpoints for WhatsApp calls. The library transports pre-encoded H.264 — it never touches
//! pixels — so a source hands us complete Annex-B access units (start codes included) and a sink
//! receives reassembled peer AUs. The codec lives with the consumer (ffmpeg, WebCodecs, a hardware
//! encoder); reference parameters: H.264 Constrained Baseline (avc1.42E01F), 640x480 @ 15 fps,
//! ~500 kbps, a keyframe every ~30 frames, SPS/PPS repeated on each IDR (the peer can join
//! mid-stream on an upgrade).

pub use wacore::voip::VideoFrame;

/// A video source for a call: one complete H.264 Annex-B access unit per item. Channel-factory
/// shaped for the same reasons as [`AudioSource`](crate::voip::audio::AudioSource); a closed
/// channel (encoder gone) does NOT end the call — audio keeps running.
pub trait VideoSource: Send + Sync + 'static {
    /// The channel the facade reads encoded AUs from. Called once when video starts.
    fn frames(&self) -> async_channel::Receiver<Vec<u8>>;
}

/// A video sink for a call: reassembled peer access units, with keyframe/orientation metadata.
/// VoIP is loss tolerant, so the facade drops a frame if the sink can't keep up.
pub trait VideoSink: Send + Sync + 'static {
    /// The channel the facade writes received AUs to. Called once when video starts.
    fn playout(&self) -> async_channel::Sender<VideoFrame>;
}

/// Blanket impls so a bare `async_channel` endpoint is usable directly, like the audio ones.
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
