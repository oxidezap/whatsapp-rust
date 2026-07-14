//! Audio playback using cpal
//!
//! Plays Opus/OGG audio files for PTT voice message playback.

use std::io::Cursor;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use log::{error, info, warn};
use ogg::reading::PacketReader;
use opus::{Channels, Decoder as OpusDecoder};
use tokio::sync::oneshot;

/// Audio player for PTT voice messages.
pub struct AudioPlayer {
    stream: Option<Stream>,
    is_playing: Arc<AtomicBool>,
    position: Arc<AtomicUsize>,
    total_samples: u64,
    sample_rate: u32,
    completion_tx: Option<oneshot::Sender<()>>,
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioPlayer {
    pub fn new() -> Self {
        Self {
            stream: None,
            is_playing: Arc::new(AtomicBool::new(false)),
            position: Arc::new(AtomicUsize::new(0)),
            total_samples: 0,
            sample_rate: 48000,
            completion_tx: None,
        }
    }

    /// Returns a receiver that fires when playback completes.
    pub fn on_complete(&mut self) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.completion_tx = Some(tx);
        rx
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed)
    }

    pub fn play(&mut self, ogg_data: Vec<u8>) -> Result<(), PlayerError> {
        let samples = decode_ogg(&ogg_data)?;
        if samples.is_empty() {
            return Err(PlayerError::EmptyAudio);
        }

        info!("Decoded {} samples for playback", samples.len());
        self.play_samples(samples, 48000)
    }

    /// Play raw f32 PCM samples at the specified sample rate.
    pub fn play_samples(
        &mut self,
        samples: Vec<f32>,
        src_sample_rate: u32,
    ) -> Result<(), PlayerError> {
        // Preserve completion sender through stop() since it may have been set by on_complete()
        let saved_completion_tx = self.completion_tx.take();
        self.stop();
        self.completion_tx = saved_completion_tx;

        if samples.is_empty() {
            return Err(PlayerError::EmptyAudio);
        }

        info!(
            "Playing {} samples at {} Hz",
            samples.len(),
            src_sample_rate
        );

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(PlayerError::NoOutputDevice)?;

        info!("Using default output device");

        // Prefer F32 (native to our samples), but i16/u16-only devices still
        // play: the callback converts per sample. Formats build_stream can't
        // dispatch (i32/f64/...) are filtered out up front so a fallback pick
        // never lands on one while a buildable range exists.
        let supported_configs: Vec<_> = device
            .supported_output_configs()
            .map_err(|e| PlayerError::DeviceError(e.to_string()))?
            .filter(|c| {
                matches!(
                    c.sample_format(),
                    SampleFormat::F32 | SampleFormat::I16 | SampleFormat::U16
                )
            })
            .collect();

        let supports_48k = |c: &cpal::SupportedStreamConfigRange| {
            c.min_sample_rate() <= 48000 && c.max_sample_rate() >= 48000
        };
        let is_f32 = |c: &cpal::SupportedStreamConfigRange| c.sample_format() == SampleFormat::F32;
        let chosen = supported_configs
            .iter()
            .find(|c| is_f32(c) && supports_48k(c))
            .or_else(|| supported_configs.iter().find(|c| is_f32(c)))
            .or_else(|| supported_configs.iter().find(|c| supports_48k(c)))
            .or_else(|| supported_configs.first())
            .ok_or(PlayerError::NoSupportedConfig)?;

        let sample_format = chosen.sample_format();
        let config: StreamConfig = if supports_48k(chosen) {
            chosen.with_sample_rate(48000)
        } else {
            chosen.with_sample_rate(chosen.min_sample_rate())
        }
        .into();
        self.sample_rate = config.sample_rate;
        let output_channels = config.channels as usize;

        info!(
            "Output config: {} Hz, {} channels, {:?}",
            config.sample_rate, output_channels, sample_format
        );

        let resampled =
            resample_audio(&samples, src_sample_rate, self.sample_rate, output_channels);
        self.total_samples = resampled.len() as u64;

        let is_playing = self.is_playing.clone();
        let position = self.position.clone();
        position.store(0, Ordering::Relaxed);
        is_playing.store(true, Ordering::Relaxed);

        let completion_tx: Arc<Mutex<Option<oneshot::Sender<()>>>> =
            Arc::new(Mutex::new(self.completion_tx.take()));
        let audio_data = Arc::new(resampled);

        let stream = match sample_format {
            SampleFormat::F32 => build_stream::<f32>(
                &device,
                config,
                audio_data,
                position,
                is_playing,
                completion_tx,
            ),
            SampleFormat::I16 => build_stream::<i16>(
                &device,
                config,
                audio_data,
                position,
                is_playing,
                completion_tx,
            ),
            SampleFormat::U16 => build_stream::<u16>(
                &device,
                config,
                audio_data,
                position,
                is_playing,
                completion_tx,
            ),
            other => Err(PlayerError::StreamError(format!(
                "unsupported output sample format {other:?}"
            ))),
        }?;

        stream
            .play()
            .map_err(|e| PlayerError::StreamError(e.to_string()))?;

        self.stream = Some(stream);
        info!("Audio playback started");

        Ok(())
    }

    pub fn stop(&mut self) {
        self.stream.take();
        self.is_playing.store(false, Ordering::Relaxed);
        self.position.store(0, Ordering::Relaxed);
        self.total_samples = 0;
        self.completion_tx = None;
    }

    pub fn pause(&mut self) {
        if let Some(ref stream) = self.stream {
            let _ = stream.pause();
            self.is_playing.store(false, Ordering::Relaxed);
        }
    }

    pub fn resume(&mut self) {
        if let Some(ref stream) = self.stream {
            let _ = stream.play();
            self.is_playing.store(true, Ordering::Relaxed);
        }
    }
}

/// Build the output stream for the device's sample format, converting our
/// f32 samples in the callback (same dispatch as call_device's speaker path).
fn build_stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: StreamConfig,
    audio: Arc<Vec<f32>>,
    position: Arc<AtomicUsize>,
    is_playing: Arc<AtomicBool>,
    completion_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
) -> Result<Stream, PlayerError> {
    device
        .build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut pos = position.load(Ordering::Relaxed);

                for sample in data.iter_mut() {
                    let s = if pos < audio.len() {
                        let s = audio[pos];
                        pos += 1;
                        s
                    } else {
                        // Mark as done and notify completion (only once)
                        if is_playing.swap(false, Ordering::Relaxed)
                            && let Ok(mut guard) = completion_tx.lock()
                            && let Some(tx) = guard.take()
                        {
                            let _ = tx.send(());
                        }
                        0.0
                    };
                    *sample = T::from_sample(s);
                }

                position.store(pos, Ordering::Relaxed);
            },
            move |err| {
                error!("Audio output error: {}", err);
            },
            None,
        )
        .map_err(|e| PlayerError::StreamError(e.to_string()))
}

fn decode_ogg(ogg_data: &[u8]) -> Result<Vec<f32>, PlayerError> {
    let cursor = Cursor::new(ogg_data);
    let mut packet_reader = PacketReader::new(cursor);
    let mut all_samples: Vec<f32> = Vec::new();
    let mut packet_count = 0;
    let mut decoder: Option<OpusDecoder> = None;
    let mut channel_count = 1usize;
    let mut pre_skip = 0usize;

    while let Some(packet) = packet_reader
        .read_packet()
        .map_err(|e| PlayerError::DecodeError(format!("OGG read error: {}", e)))?
    {
        packet_count += 1;

        // First packet: OpusHead header, second packet: OpusTags (skip both)
        if packet_count == 1 {
            if packet.data.len() >= 12 && &packet.data[0..8] == b"OpusHead" {
                let channels = packet.data[9];
                // 48kHz priming samples the decoder must discard before real audio
                pre_skip = u16::from_le_bytes([packet.data[10], packet.data[11]]) as usize;
                channel_count = if channels > 1 { 2 } else { 1 };
                let opus_channels = if channel_count == 2 {
                    Channels::Stereo
                } else {
                    Channels::Mono
                };
                decoder =
                    Some(OpusDecoder::new(48000, opus_channels).map_err(|e| {
                        PlayerError::DecodeError(format!("Opus decoder init: {}", e))
                    })?);
            }
            continue;
        }
        if packet_count == 2 {
            continue;
        }

        // Some malformed-but-playable streams omit OpusHead.
        if decoder.is_none() {
            decoder = Some(
                OpusDecoder::new(48000, Channels::Mono)
                    .map_err(|e| PlayerError::DecodeError(format!("Opus decoder init: {e}")))?,
            );
        }
        let Some(dec) = decoder.as_mut() else {
            return Err(PlayerError::DecodeError(
                "Opus decoder was not initialized".to_string(),
            ));
        };

        let mut output = vec![0.0f32; 5760 * 2];
        match dec.decode_float(&packet.data, &mut output, false) {
            Ok(n) => {
                // n is frames per channel; the buffer is interleaved
                output.truncate(n * channel_count);
                let mono = if channel_count == 2 {
                    output
                        .chunks_exact(2)
                        .map(|pair| (pair[0] + pair[1]) / 2.0)
                        .collect()
                } else {
                    output
                };
                let skip = pre_skip.min(mono.len());
                pre_skip -= skip;
                all_samples.extend_from_slice(&mono[skip..]);
            }
            Err(e) => warn!("Opus decode error (packet {}): {}", packet_count, e),
        }
    }

    info!(
        "Decoded {} packets, {} samples",
        packet_count,
        all_samples.len()
    );

    if all_samples.is_empty() {
        return Err(PlayerError::DecodeError("No samples decoded".to_string()));
    }

    Ok(all_samples)
}

fn resample_audio(samples: &[f32], src_rate: u32, dst_rate: u32, channels: usize) -> Vec<f32> {
    if src_rate == 0 || dst_rate == 0 {
        return samples.to_vec();
    }

    if src_rate == dst_rate && channels == 1 {
        return samples.to_vec();
    }

    let ratio = dst_rate as f32 / src_rate as f32;
    let output_len = (samples.len() as f32 * ratio) as usize;
    let mut output = Vec::with_capacity(output_len * channels);

    for i in 0..output_len {
        let src_idx = (i as f32 / ratio) as usize;
        let sample = samples.get(src_idx).copied().unwrap_or(0.0);
        output.extend(std::iter::repeat_n(sample, channels));
    }

    output
}

#[derive(Debug)]
pub enum PlayerError {
    NoOutputDevice,
    NoSupportedConfig,
    EmptyAudio,
    DeviceError(String),
    StreamError(String),
    DecodeError(String),
}

impl std::fmt::Display for PlayerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoOutputDevice => write!(f, "No audio output device found"),
            Self::NoSupportedConfig => write!(f, "No supported audio configuration"),
            Self::EmptyAudio => write!(f, "No audio data to play"),
            Self::DeviceError(e) => write!(f, "Audio device error: {}", e),
            Self::StreamError(e) => write!(f, "Audio stream error: {}", e),
            Self::DecodeError(e) => write!(f, "Audio decode error: {}", e),
        }
    }
}

impl std::error::Error for PlayerError {}
