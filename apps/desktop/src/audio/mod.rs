//! Audio module for PTT voice message recording and call audio
//!
//! This module provides:
//! - Audio capture using cpal
//! - Opus encoding to OGG container
//! - Waveform generation for WhatsApp PTT messages
//! - Audio playback for received voice messages
//! - The cpal mic/speaker bridge for VoIP calls (engine lives in the library)

mod call_device;
mod encoder;
mod player;
mod recorder;
mod waveform;

pub use call_device::{spawn_mic, spawn_speaker};
pub use encoder::encode_to_opus_ogg;
pub use player::AudioPlayer;
pub use recorder::AudioRecorder;
pub use waveform::generate_waveform;
