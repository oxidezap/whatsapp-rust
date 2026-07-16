//! Audio extraction from video files
//!
//! This module provides audio extraction functionality for MP4 video files.
//! The video decoding is handled by StreamingVideoDecoder in streaming.rs.

use std::io::Cursor;

use mp4::{Mp4Reader, TrackType};

/// ADTS sample rate to frequency index mapping
const ADTS_FREQ_TABLE: [(u32, u8); 13] = [
    (96000, 0),
    (88200, 1),
    (64000, 2),
    (48000, 3),
    (44100, 4),
    (32000, 5),
    (24000, 6),
    (22050, 7),
    (16000, 8),
    (12000, 9),
    (11025, 10),
    (8000, 11),
    (7350, 12),
];

/// Decoded audio data from video (always mono after conversion)
#[derive(Clone)]
pub struct VideoAudio {
    /// PCM samples (f32, mono)
    pub samples: Vec<f32>,
    /// Sample rate in Hz
    pub sample_rate: u32,
}

/// Extract audio from MP4 using mp4 crate for demuxing and symphonia for AAC decoding
pub fn extract_audio_from_mp4(mp4_data: &[u8]) -> Option<VideoAudio> {
    use symphonia::core::codecs::CodecParameters;
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    // First, use mp4 crate to find and extract audio track
    let cursor = Cursor::new(mp4_data);
    let mut mp4 = match Mp4Reader::read_header(cursor, mp4_data.len() as u64) {
        Ok(mp4) => mp4,
        Err(e) => {
            log::warn!("Failed to read MP4 for audio extraction: {}", e);
            return None;
        }
    };

    // Find audio track
    let audio_track = mp4
        .tracks()
        .values()
        .find(|t| matches!(t.track_type(), Ok(TrackType::Audio)))?;

    let track_id = audio_track.track_id();
    let sample_count = audio_track.sample_count();

    // Get audio parameters
    let sample_rate = audio_track
        .sample_freq_index()
        .ok()
        .map(|f| f.freq())
        .unwrap_or(44100);
    let channels = audio_track
        .channel_config()
        .ok()
        .map(|c| c as u8)
        .unwrap_or(2);

    log::info!(
        "Audio track found: id={}, {} samples, {} Hz, {} channels",
        track_id,
        sample_count,
        sample_rate,
        channels
    );

    // Extract raw AAC frames from MP4 (reusing the same reader)
    let mut aac_frames: Vec<Vec<u8>> = Vec::new();
    for sample_idx in 1..=sample_count {
        if let Ok(Some(sample)) = mp4.read_sample(track_id, sample_idx) {
            aac_frames.push(sample.bytes.to_vec());
        }
    }

    if aac_frames.is_empty() {
        log::info!("No AAC frames extracted");
        return None;
    }

    log::info!("Extracted {} AAC frames from MP4", aac_frames.len());

    // Convert raw AAC frames to ADTS format for symphonia
    let adts_data = wrap_aac_as_adts(&aac_frames, sample_rate, channels);

    log::info!("Created ADTS stream: {} bytes", adts_data.len());

    // Now decode ADTS using symphonia
    let cursor = Cursor::new(adts_data);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let mut hint = Hint::new();
    hint.with_extension("aac");
    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();

    let mut format =
        match symphonia::default::get_probe().probe(&hint, mss, format_opts, metadata_opts) {
            Ok(f) => f,
            Err(e) => {
                log::warn!("Failed to probe ADTS format: {}", e);
                return None;
            }
        };

    // Find the audio track in ADTS
    let track = format.tracks().first()?;

    let adts_track_id = track.id;
    let Some(CodecParameters::Audio(audio_params)) = track.codec_params.clone() else {
        log::warn!("ADTS track carries no audio codec parameters");
        return None;
    };
    let decoder_opts = AudioDecoderOptions::default();
    let mut decoder =
        match symphonia::default::get_codecs().make_audio_decoder(&audio_params, &decoder_opts) {
            Ok(d) => d,
            Err(e) => {
                log::warn!("Failed to create AAC decoder: {}", e);
                return None;
            }
        };

    let mut all_samples: Vec<f32> = Vec::new();
    let mut frame: Vec<f32> = Vec::new();

    // Decode all audio packets
    while let Ok(Some(packet)) = format.next_packet() {
        if packet.track_id != adts_track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                decoded.copy_to_vec_interleaved(&mut frame);
                all_samples.extend_from_slice(&frame);
            }
            Err(e) => {
                log::debug!("Audio decode error (skipping frame): {}", e);
            }
        }
    }

    if all_samples.is_empty() {
        log::info!("No audio samples decoded");
        return None;
    }

    // Downmix to mono if needed (average across all channels; the API promises mono)
    let mono_samples = if channels > 1 {
        let mono: Vec<f32> = all_samples
            .chunks(channels as usize)
            .map(|chunk| chunk.iter().sum::<f32>() / chunk.len() as f32)
            .collect();
        log::info!(
            "Converted {} interleaved samples to {} mono samples",
            all_samples.len(),
            mono.len()
        );
        mono
    } else {
        all_samples
    };

    log::info!(
        "Decoded {} audio samples ({:.2}s)",
        mono_samples.len(),
        mono_samples.len() as f32 / sample_rate as f32
    );

    Some(VideoAudio {
        samples: mono_samples,
        sample_rate,
    })
}

/// Wrap raw AAC frames in ADTS format for symphonia
fn wrap_aac_as_adts(frames: &[Vec<u8>], sample_rate: u32, channels: u8) -> Vec<u8> {
    let mut adts = Vec::new();

    // Map sample rate to ADTS frequency index using lookup table
    let freq_idx = ADTS_FREQ_TABLE
        .iter()
        .find(|(rate, _)| *rate == sample_rate)
        .map(|(_, idx)| *idx)
        .unwrap_or(4); // Default to 44100 (index 4)

    // ADTS profile field stores Audio Object Type minus one; AAC-LC AOT = 2
    let profile = 2u8; // AAC-LC

    for frame in frames {
        let frame_len = frame.len() + 7; // ADTS header is 7 bytes

        // Build 7-byte ADTS header
        let header: [u8; 7] = [
            0xFF,
            0xF1, // Syncword + MPEG-4 + no CRC
            ((profile - 1) << 6) | (freq_idx << 2) | ((channels >> 2) & 0x01),
            ((channels & 0x03) << 6) | ((frame_len >> 11) & 0x03) as u8,
            ((frame_len >> 3) & 0xFF) as u8,
            (((frame_len & 0x07) << 5) | 0x1F) as u8,
            0xFC, // Buffer fullness VBR + 0 frames - 1
        ];

        adts.extend_from_slice(&header);
        adts.extend_from_slice(frame);
    }

    adts
}
