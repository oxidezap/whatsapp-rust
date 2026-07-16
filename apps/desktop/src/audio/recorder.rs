//! Audio recording using cpal
//!
//! Captures audio from the default input device at 48kHz mono.
//! The samples are stored and can be resampled to 16kHz for Opus encoding.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, Sample as _, SampleFormat, SizedSample, Stream, StreamConfig};
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

        if self.sample_rate.is_multiple_of(TARGET_SAMPLE_RATE) {
            // Integer decimation (the common 48kHz case): low-pass BEFORE
            // dropping samples, or everything above the 8kHz target Nyquist
            // folds back into the voice band (a box average only manages
            // ~10dB there). Windowed-sinc FIR, ~7kHz cutoff at the input
            // rate, unity DC gain so speech level is preserved; evaluated
            // only at the kept samples, O(n·taps) — fine for PTT lengths.
            const CUTOFF_HZ: f32 = 7_000.0;
            let step = (self.sample_rate / TARGET_SAMPLE_RATE) as usize;
            // The transition band scales with the input rate, so the tap
            // count must too: 63 taps suit 48kHz (step 3), but a 96/192kHz
            // fallback device needs proportionally more or content above
            // 8kHz still folds into the output.
            let taps = (21 * step) | 1;
            let fc = CUTOFF_HZ / self.sample_rate as f32;
            let center = (taps - 1) / 2;
            let mut fir = vec![0.0f32; taps];
            for (k, tap) in fir.iter_mut().enumerate() {
                let n = k as f32 - center as f32;
                let sinc = if n == 0.0 {
                    2.0 * fc
                } else {
                    (std::f32::consts::TAU * fc * n).sin() / (std::f32::consts::PI * n)
                };
                let hamming =
                    0.54 - 0.46 * (std::f32::consts::TAU * k as f32 / (taps - 1) as f32).cos();
                *tap = sinc * hamming;
            }
            let dc_gain: f32 = fir.iter().sum();
            for tap in fir.iter_mut() {
                *tap /= dc_gain;
            }
            // saturating: an empty capture must not underflow (the loop below
            // is a no-op then anyway).
            let last = self.samples.len().saturating_sub(1);
            for i in 0..self.samples.len() / step {
                let mid = (i * step) as isize;
                let mut acc = 0.0f32;
                for (k, &tap) in fir.iter().enumerate() {
                    // Clamped edges: replicating the boundary sample beats
                    // zero-padding, which would fade the clip's ends.
                    let src = (mid + k as isize - center as isize).clamp(0, last as isize);
                    acc += tap * self.samples[src as usize];
                }
                output.push(acc);
            }
        } else {
            for i in 0..output_len {
                // Linear interpolation: nearest-neighbor sample dropping
                // aliases audibly on voice.
                let src_pos = i as f32 * ratio;
                let idx = src_pos as usize;
                let frac = src_pos - idx as f32;
                let Some(&a) = self.samples.get(idx) else {
                    break;
                };
                let b = self.samples.get(idx + 1).copied().unwrap_or(a);
                output.push(a + (b - a) * frac);
            }
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
    sample_format: SampleFormat,
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
            sample_format: SampleFormat::F32,
            sample_rate: CAPTURE_SAMPLE_RATE,
        }
    }

    pub fn init(&mut self) -> Result<(), RecorderError> {
        let host = cpal::default_host();

        let device = host
            .default_input_device()
            .ok_or(RecorderError::NoInputDevice)?;

        info!("Using default input device");

        let supported = device
            .supported_input_configs()
            .map_err(|e| RecorderError::DeviceError(e.to_string()))?;

        // Prefer F32 (native to our buffer), but i16/u16-only mics still
        // record: the callback converts per sample. Format outranks 48kHz
        // support, which outranks mono: multichannel is downmixed anyway,
        // while a low capture rate permanently costs voice bandwidth.
        let mut best: Option<(u8, _)> = None;
        for config in supported {
            if !matches!(
                config.sample_format(),
                SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
            ) {
                continue;
            }
            let supports_rate = config.min_sample_rate() <= CAPTURE_SAMPLE_RATE
                && config.max_sample_rate() >= CAPTURE_SAMPLE_RATE;
            let score = u8::from(config.sample_format() == SampleFormat::F32) * 4
                + u8::from(supports_rate) * 2
                + u8::from(config.channels() == 1);
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

        self.sample_format = supported_config.sample_format();
        let stream_config: StreamConfig = supported_config.into();
        self.sample_rate = stream_config.sample_rate;

        info!(
            "Audio config: {} Hz, {} channel(s), {:?}",
            stream_config.sample_rate, stream_config.channels, self.sample_format
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

        let stream = match self.sample_format {
            SampleFormat::F32 => build_input_stream::<f32>(device, config, samples),
            SampleFormat::I16 => build_input_stream::<i16>(device, config, samples),
            SampleFormat::U16 => build_input_stream::<u16>(device, config, samples),
            other => Err(RecorderError::StreamError(format!(
                "unsupported input sample format {other:?}"
            ))),
        }?;

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

/// Build the input stream for the device's sample format, converting to f32
/// in the callback (same dispatch as the player's output path).
fn build_input_stream<T: SizedSample>(
    device: &Device,
    config: StreamConfig,
    samples: Arc<Mutex<Vec<f32>>>,
) -> Result<Stream, RecorderError>
where
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    device
        .build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                let Ok(mut buffer) = samples.lock() else {
                    return;
                };
                if channels == 1 {
                    buffer.extend(data.iter().map(|&s| f32::from_sample(s)));
                } else {
                    // Downmix to mono
                    for chunk in data.chunks(channels) {
                        let mono: f32 = chunk.iter().map(|&s| f32::from_sample(s)).sum::<f32>()
                            / channels as f32;
                        buffer.push(mono);
                    }
                }
            },
            move |err| {
                error!("Audio input stream error: {}", err);
            },
            None,
        )
        .map_err(|e| RecorderError::StreamError(e.to_string()))
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

    #[test]
    fn resample_decimation_has_unity_dc_gain() {
        // The FIR taps are normalized to unity DC gain: a constant signal
        // must come out at the same level (edges included — they clamp to
        // the boundary sample, so even they see pure DC).
        let audio = RecordedAudio {
            samples: vec![0.5; 4800],
            sample_rate: 48_000,
            duration_secs: 0,
        };
        let out = audio.resample_to_16khz();
        assert_eq!(out.len(), 1600);
        for &s in &out {
            assert!((s - 0.5).abs() < 1e-3, "DC gain drifted: {s}");
        }
    }

    #[test]
    fn resample_decimation_attenuates_aliasing_band() {
        // 12kHz at 48k folds to 4kHz after naive 3:1 decimation — squarely
        // in the voice band. The low-pass must crush it while passing 1kHz
        // essentially untouched.
        let rate = 48_000u32;
        let tone = |freq: f32| -> RecordedAudio {
            RecordedAudio {
                samples: (0..rate as usize)
                    .map(|i| (std::f32::consts::TAU * freq * i as f32 / rate as f32).sin())
                    .collect(),
                sample_rate: rate,
                duration_secs: 1,
            }
        };
        let rms = |samples: &[f32]| -> f32 {
            (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
        };
        let low = tone(1_000.0).resample_to_16khz();
        let high = tone(12_000.0).resample_to_16khz();
        // Skip the clamped edges; a full-scale sine has ~0.707 rms.
        let low_rms = rms(&low[100..low.len() - 100]);
        let high_rms = rms(&high[100..high.len() - 100]);
        assert!(low_rms > 0.65, "1kHz should pass through, rms {low_rms}");
        assert!(
            high_rms < 0.02,
            "12kHz should alias-filter to near silence, rms {high_rms}"
        );
    }
}
