//! Opus audio encoder for WhatsApp PTT messages.

use std::io::Cursor;

use log::info;
use ogg::writing::PacketWriteEndInfo;
use opus::{Application, Channels, Encoder};

use super::recorder::RecordedAudio;

const SAMPLE_RATE: u32 = 16000;
const CHANNELS: Channels = Channels::Mono;
const BITRATE: i32 = 16000;
const FRAME_SIZE_MS: usize = 20;
const FRAME_SIZE_SAMPLES: usize = (SAMPLE_RATE as usize * FRAME_SIZE_MS) / 1000;
const GRANULE_RATE: u32 = 48000;
const GRANULE_PER_FRAME: u64 = (GRANULE_RATE as u64 * FRAME_SIZE_MS as u64) / 1000;
const PRE_SKIP: u16 = 312;

pub fn encode_to_opus_ogg(audio: &RecordedAudio) -> Result<Vec<u8>, EncoderError> {
    let samples = audio.resample_to_16khz();
    if samples.is_empty() {
        return Err(EncoderError::EmptyAudio);
    }

    info!(
        "Encoding {} samples ({:.1}s) to Opus/OGG",
        samples.len(),
        samples.len() as f32 / SAMPLE_RATE as f32
    );

    let mut encoder = Encoder::new(SAMPLE_RATE, CHANNELS, Application::Voip)
        .map_err(|e| EncoderError::OpusError(e.to_string()))?;
    encoder
        .set_bitrate(opus::Bitrate::Bits(BITRATE))
        .map_err(|e| EncoderError::OpusError(e.to_string()))?;

    let mut ogg_buffer = Vec::new();
    let serial = rand_serial();

    let samples_i16: Vec<i16> = samples
        .iter()
        .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
        .collect();

    let mut encoded_packets: Vec<Vec<u8>> = Vec::new();
    for chunk in samples_i16.chunks(FRAME_SIZE_SAMPLES) {
        let mut frame = chunk.to_vec();
        if frame.len() < FRAME_SIZE_SAMPLES {
            frame.resize(FRAME_SIZE_SAMPLES, 0);
        }

        let mut output = vec![0u8; 4000];
        let len = encoder
            .encode(&frame, &mut output)
            .map_err(|e| EncoderError::OpusError(e.to_string()))?;
        output.truncate(len);
        encoded_packets.push(output);
    }

    // EOS granule reflects the real capture length plus pre-skip: decoders
    // discard PRE_SKIP samples up front, so trimming to the granule must land
    // on the capture's end, not PRE_SKIP samples before it.
    let eos_granule =
        PRE_SKIP as u64 + samples_i16.len() as u64 * (GRANULE_RATE / SAMPLE_RATE) as u64;
    // When the final frame's zero-padding can't absorb the pre-skip (exact
    // frame multiples have none at all), one extra silence frame keeps the
    // packet stream covering the full logical duration.
    if eos_granule > encoded_packets.len() as u64 * GRANULE_PER_FRAME {
        let silence = vec![0i16; FRAME_SIZE_SAMPLES];
        let mut output = vec![0u8; 4000];
        let len = encoder
            .encode(&silence, &mut output)
            .map_err(|e| EncoderError::OpusError(e.to_string()))?;
        output.truncate(len);
        encoded_packets.push(output);
    }

    {
        let cursor = Cursor::new(&mut ogg_buffer);
        let mut writer = ogg::PacketWriter::new(cursor);

        writer
            .write_packet(
                create_opus_id_header(),
                serial,
                PacketWriteEndInfo::EndPage,
                0,
            )
            .map_err(|e| EncoderError::OggError(e.to_string()))?;

        writer
            .write_packet(
                create_opus_comment_header(),
                serial,
                PacketWriteEndInfo::EndPage,
                0,
            )
            .map_err(|e| EncoderError::OggError(e.to_string()))?;

        let total_packets = encoded_packets.len();
        let mut current_granule: u64 = 0;

        for (i, packet) in encoded_packets.into_iter().enumerate() {
            current_granule += GRANULE_PER_FRAME;
            let (end_info, granule) = if i == total_packets - 1 {
                (PacketWriteEndInfo::EndStream, eos_granule)
            } else {
                (PacketWriteEndInfo::NormalPacket, current_granule)
            };
            writer
                .write_packet(packet, serial, end_info, granule)
                .map_err(|e| EncoderError::OggError(e.to_string()))?;
        }
    }

    info!("Encoded to {} bytes OGG", ogg_buffer.len());
    Ok(ogg_buffer)
}

fn create_opus_id_header() -> Vec<u8> {
    let mut header = Vec::with_capacity(19);
    header.extend_from_slice(b"OpusHead");
    header.push(1); // Version
    header.push(1); // Channels (mono)
    header.extend_from_slice(&PRE_SKIP.to_le_bytes());
    header.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    header.extend_from_slice(&0u16.to_le_bytes()); // Output gain (0 dB)
    header.push(0); // Channel mapping family
    header
}

fn create_opus_comment_header() -> Vec<u8> {
    let mut header = Vec::new();
    header.extend_from_slice(b"OpusTags");
    let vendor = b"whatsapp-rust";
    header.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    header.extend_from_slice(vendor);
    header.extend_from_slice(&0u32.to_le_bytes()); // No comments
    header
}

fn rand_serial() -> u32 {
    let seed = wacore::time::now_millis() as u32;
    seed.wrapping_mul(1103515245).wrapping_add(12345)
}

#[derive(Debug)]
pub enum EncoderError {
    EmptyAudio,
    OpusError(String),
    OggError(String),
}

impl std::fmt::Display for EncoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyAudio => write!(f, "No audio data to encode"),
            Self::OpusError(e) => write!(f, "Opus encoder error: {}", e),
            Self::OggError(e) => write!(f, "OGG writer error: {}", e),
        }
    }
}

impl std::error::Error for EncoderError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_id_header() {
        let header = create_opus_id_header();
        assert_eq!(&header[0..8], b"OpusHead");
        assert_eq!(header[8], 1); // version
        assert_eq!(header[9], 1); // channels
    }

    #[test]
    fn test_opus_comment_header() {
        let header = create_opus_comment_header();
        assert_eq!(&header[0..8], b"OpusTags");
    }

    #[test]
    fn test_encode_simple_audio() {
        // Generate 1 second of silence
        let audio = RecordedAudio {
            samples: vec![0.0f32; 16000],
            sample_rate: 16000,
            duration_secs: 1,
        };

        let result = encode_to_opus_ogg(&audio);
        assert!(result.is_ok());

        let ogg_data = result.unwrap();
        // Check OGG magic number
        assert_eq!(&ogg_data[0..4], b"OggS");
    }

    #[test]
    fn test_exact_frame_multiple_keeps_full_duration() {
        // 16000 samples = exactly 50 frames: no zero-padding to absorb the
        // pre-skip, so a capped EOS granule would clip ~6.5ms of real audio.
        let samples = 16000u64;
        let audio = RecordedAudio {
            samples: vec![0.0f32; samples as usize],
            sample_rate: 16000,
            duration_secs: 1,
        };

        let ogg_data = encode_to_opus_ogg(&audio).unwrap();

        // The EOS granule lives in the header of the last OGG page
        // (byte offset 6, 8 bytes LE after the "OggS" capture pattern).
        let last_page = ogg_data
            .windows(4)
            .rposition(|w| w == b"OggS")
            .expect("no OGG page found");
        let granule_bytes: [u8; 8] = ogg_data[last_page + 6..last_page + 14].try_into().unwrap();
        let eos_granule = u64::from_le_bytes(granule_bytes);
        assert_eq!(
            eos_granule,
            PRE_SKIP as u64 + samples * (GRANULE_RATE / SAMPLE_RATE) as u64
        );
    }
}
