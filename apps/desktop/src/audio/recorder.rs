//! Audio recording using cpal
//!
//! Captures audio from the default input device at 48kHz mono.
//! The samples are stored and can be resampled to 16kHz for Opus encoding.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream, StreamConfig};
use log::{error, info, warn};
use wacore::time::Instant;

/// Target sample rate for Opus encoding (WhatsApp standard)
pub const TARGET_SAMPLE_RATE: u32 = 16000;

/// Capture sample rate (most hardware supports this)
const CAPTURE_SAMPLE_RATE: u32 = 48000;

pub struct RecordedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_secs: u32,
}

impl RecordedAudio {
    pub fn resample_to_16khz(&self) -> Vec<f32> {
        if self.sample_rate == TARGET_SAMPLE_RATE {
            return self.samples.clone();
        }

        let ratio = self.sample_rate as f32 / TARGET_SAMPLE_RATE as f32;
        let output_len = (self.samples.len() as f32 / ratio) as usize;
        let mut output = Vec::with_capacity(output_len);

        for i in 0..output_len {
            // Linear interpolation: nearest-neighbor sample dropping aliases
            // audibly on voice.
            let src_pos = i as f32 * ratio;
            let idx = src_pos as usize;
            let frac = src_pos - idx as f32;
            let Some(&a) = self.samples.get(idx) else {
                break;
            };
            let b = self.samples.get(idx + 1).copied().unwrap_or(a);
            output.push(a + (b - a) * frac);
        }

        output
    }
}

pub struct AudioRecorder {
    stream: Option<Stream>,
    samples: Arc<Mutex<Vec<f32>>>,
    is_recording: bool,
    start_time: Option<Instant>,
    device: Option<Device>,
    config: Option<StreamConfig>,
    sample_rate: u32,
}

impl Default for AudioRecorder {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioRecorder {
    pub fn new() -> Self {
        Self {
            stream: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            is_recording: false,
            start_time: None,
            device: None,
            config: None,
            sample_rate: CAPTURE_SAMPLE_RATE,
        }
    }

    pub fn init(&mut self) -> Result<(), RecorderError> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or(RecorderError::NoInputDevice)?;

        info!(
            "Using input device: {}",
            device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_default()
        );

        let supported = device
            .supported_input_configs()
            .map_err(|e| RecorderError::DeviceError(e.to_string()))?;

        // The callback is built for f32 frames, so only F32 configs are usable.
        // Any channel count works (the callback downmixes); prefer mono at 48kHz.
        let mut best: Option<(u8, _)> = None;
        for config in supported {
            if config.sample_format() != cpal::SampleFormat::F32 {
                continue;
            }
            let supports_rate = config.min_sample_rate() <= CAPTURE_SAMPLE_RATE
                && config.max_sample_rate() >= CAPTURE_SAMPLE_RATE;
            let score = u8::from(config.channels() == 1) * 2 + u8::from(supports_rate);
            if best.as_ref().is_none_or(|(s, _)| score > *s) {
                let candidate = if supports_rate {
                    config.with_sample_rate(CAPTURE_SAMPLE_RATE)
                } else {
                    config.with_max_sample_rate()
                };
                best = Some((score, candidate));
            }
        }

        let supported_config = best
            .map(|(_, c)| c)
            .ok_or(RecorderError::NoSupportedConfig)?;

        let stream_config: StreamConfig = supported_config.into();
        self.sample_rate = stream_config.sample_rate;

        info!(
            "Audio config: {} Hz, {} channel(s)",
            stream_config.sample_rate, stream_config.channels
        );

        self.device = Some(device);
        self.config = Some(stream_config);

        Ok(())
    }

    pub fn start(&mut self) -> Result<(), RecorderError> {
        if self.is_recording {
            return Err(RecorderError::AlreadyRecording);
        }

        if self.device.is_none() {
            self.init()?;
        }

        let device = self.device.as_ref().ok_or(RecorderError::NotInitialized)?;
        let config = self.config.ok_or(RecorderError::NotInitialized)?;

        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }

        let samples = self.samples.clone();
        let channels = config.channels as usize;

        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let Ok(mut buffer) = samples.lock() else {
                        return;
                    };
                    if channels == 1 {
                        buffer.extend_from_slice(data);
                    } else {
                        // Downmix to mono
                        for chunk in data.chunks(channels) {
                            let mono: f32 = chunk.iter().sum::<f32>() / channels as f32;
                            buffer.push(mono);
                        }
                    }
                },
                move |err| {
                    error!("Audio input stream error: {}", err);
                },
                None,
            )
            .map_err(|e| RecorderError::StreamError(e.to_string()))?;

        stream
            .play()
            .inspect_err(|_| {
                self.device = None;
                self.config = None;
            })
            .map_err(|e| RecorderError::StreamError(e.to_string()))?;

        self.stream = Some(stream);
        self.is_recording = true;
        self.start_time = Some(Instant::now());

        info!("Recording started");
        Ok(())
    }

    pub fn stop(&mut self) -> Result<RecordedAudio, RecorderError> {
        if !self.is_recording {
            return Err(RecorderError::NotRecording);
        }

        self.stream.take();
        self.is_recording = false;

        let duration = self.start_time.map_or(Duration::ZERO, |t| t.elapsed());
        let samples = self.samples.lock().map(|b| b.clone()).unwrap_or_default();

        info!(
            "Recording stopped: {} samples, {:.1}s",
            samples.len(),
            duration.as_secs_f32()
        );

        Ok(RecordedAudio {
            samples,
            sample_rate: self.sample_rate,
            duration_secs: duration.as_secs() as u32,
        })
    }

    pub fn cancel(&mut self) {
        self.stream.take();
        self.is_recording = false;
        self.start_time = None;
        if let Ok(mut samples) = self.samples.lock() {
            samples.clear();
        }
        warn!("Recording cancelled");
    }
}

#[derive(Debug)]
pub enum RecorderError {
    NoInputDevice,
    NoSupportedConfig,
    NotInitialized,
    AlreadyRecording,
    NotRecording,
    DeviceError(String),
    StreamError(String),
}

impl std::fmt::Display for RecorderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoInputDevice => write!(f, "No audio input device found"),
            Self::NoSupportedConfig => write!(f, "No supported audio configuration found"),
            Self::NotInitialized => write!(f, "Recorder not initialized"),
            Self::AlreadyRecording => write!(f, "Already recording"),
            Self::NotRecording => write!(f, "Not recording"),
            Self::DeviceError(e) => write!(f, "Audio device error: {}", e),
            Self::StreamError(e) => write!(f, "Audio stream error: {}", e),
        }
    }
}

impl std::error::Error for RecorderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_interpolates_between_source_samples() {
        // A linear ramp resamples to exact fractional positions; the old
        // nearest-neighbor drop would return [0.0, 1.0, 3.0, 4.0].
        let audio = RecordedAudio {
            samples: vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0],
            sample_rate: 24_000,
            duration_secs: 0,
        };
        assert_eq!(audio.resample_to_16khz(), vec![0.0, 1.5, 3.0, 4.5]);
    }
}
