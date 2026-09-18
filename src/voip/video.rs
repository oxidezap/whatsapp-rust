//! Video endpoints for WhatsApp calls. The library transports pre-encoded H.264 — it never touches
//! pixels — so a source hands us complete Annex-B access units (start codes included) and a sink
//! receives reassembled peer AUs. The codec lives with the consumer (ffmpeg, WebCodecs, a hardware
//! encoder). WhatsApp uses H.264 Constrained Baseline (avc1.42E01F), repeated SPS/PPS, and adapts
//! from a low-bandwidth 15 fps mode up to 1280x720 @ 20 fps / ~2 Mbps. A bare channel keeps the
//! compatibility cadence of 15 fps; custom sources report their RTP stride explicitly.
//!
//! The types themselves live in the neutral contract (`wacore::voip_control::ports`), re-exported
//! here so the historical `whatsapp_rust::voip::{VideoSource, VideoSink, VideoFrame}` paths resolve.

pub use wacore::voip_control::{TimedVideoFrame, VideoFrame, VideoSink, VideoSource};
