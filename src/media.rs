//! High-level media-message builders.
//!
//! Turn an [`UploadResponse`] (from [`Client::upload`](crate::Client::upload)) plus
//! typed options into a ready-to-send [`wa::Message`], so callers don't hand-assemble
//! the CDN/crypto fields (url, direct_path, media_key, file_sha256, file_enc_sha256,
//! file_length, media_key_timestamp, streaming_sidecar) every time. Mirrors WA Web's
//! send-media path, which builds the proto from the upload result internally.
//! The resulting [`wa::Message`] is sent with `client.send_message(to, msg)`.
//!
//! ```no_run
//! # fn build(upload: whatsapp_rust::upload::UploadResponse) {
//! use whatsapp_rust::media::{self, ImageOptions};
//! let _msg = media::image_message(upload, ImageOptions { caption: Some("hi".into()), ..Default::default() });
//! # }
//! ```

use crate::upload::UploadResponse;
use waproto::whatsapp as wa;

#[derive(Debug, Clone, Default)]
pub struct ImageOptions {
    pub caption: Option<String>,
    /// Defaults to `image/jpeg`.
    pub mimetype: Option<String>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub struct VideoOptions {
    pub caption: Option<String>,
    /// Defaults to `video/mp4`.
    pub mimetype: Option<String>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub duration_seconds: Option<u32>,
    /// Send as a looping GIF-style clip.
    pub gif_playback: Option<bool>,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentOptions {
    /// Defaults to `application/octet-stream`.
    pub mimetype: Option<String>,
    /// File name shown to the recipient.
    pub file_name: Option<String>,
    pub title: Option<String>,
    pub caption: Option<String>,
    pub page_count: Option<u32>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Default)]
pub struct AudioOptions {
    /// Defaults to `audio/ogg; codecs=opus`.
    pub mimetype: Option<String>,
    pub duration_seconds: Option<u32>,
    /// Push-to-talk (voice note) flag.
    pub ptt: Option<bool>,
    /// PCM waveform preview bytes (voice notes).
    pub waveform: Option<Vec<u8>>,
}

/// Build an image message from an upload result.
pub fn image_message(upload: UploadResponse, opts: ImageOptions) -> wa::Message {
    wa::Message {
        image_message: Some(Box::new(wa::message::ImageMessage {
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key.to_vec()),
            file_sha256: Some(upload.file_sha256.to_vec()),
            file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
            file_length: Some(upload.file_length),
            media_key_timestamp: Some(upload.media_key_timestamp),
            mimetype: Some(opts.mimetype.unwrap_or_else(|| "image/jpeg".to_string())),
            caption: opts.caption,
            jpeg_thumbnail: opts.jpeg_thumbnail,
            ..Default::default()
        })),
        ..Default::default()
    }
}

/// Build a video message from an upload result. Carries the streaming sidecar
/// (progressive-playback HMAC table) from the upload when present.
pub fn video_message(upload: UploadResponse, opts: VideoOptions) -> wa::Message {
    wa::Message {
        video_message: Some(Box::new(wa::message::VideoMessage {
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key.to_vec()),
            file_sha256: Some(upload.file_sha256.to_vec()),
            file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
            file_length: Some(upload.file_length),
            media_key_timestamp: Some(upload.media_key_timestamp),
            streaming_sidecar: upload.streaming_sidecar,
            mimetype: Some(opts.mimetype.unwrap_or_else(|| "video/mp4".to_string())),
            caption: opts.caption,
            jpeg_thumbnail: opts.jpeg_thumbnail,
            seconds: opts.duration_seconds,
            gif_playback: opts.gif_playback,
            ..Default::default()
        })),
        ..Default::default()
    }
}

/// Build a document message from an upload result.
pub fn document_message(upload: UploadResponse, opts: DocumentOptions) -> wa::Message {
    wa::Message {
        document_message: Some(Box::new(wa::message::DocumentMessage {
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key.to_vec()),
            file_sha256: Some(upload.file_sha256.to_vec()),
            file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
            file_length: Some(upload.file_length),
            media_key_timestamp: Some(upload.media_key_timestamp),
            mimetype: Some(
                opts.mimetype
                    .unwrap_or_else(|| "application/octet-stream".to_string()),
            ),
            file_name: opts.file_name,
            title: opts.title,
            caption: opts.caption,
            page_count: opts.page_count,
            jpeg_thumbnail: opts.jpeg_thumbnail,
            ..Default::default()
        })),
        ..Default::default()
    }
}

/// Build an audio / voice-note message from an upload result. Carries the
/// streaming sidecar from the upload when present.
pub fn audio_message(upload: UploadResponse, opts: AudioOptions) -> wa::Message {
    wa::Message {
        audio_message: Some(Box::new(wa::message::AudioMessage {
            url: Some(upload.url),
            direct_path: Some(upload.direct_path),
            media_key: Some(upload.media_key.to_vec()),
            file_sha256: Some(upload.file_sha256.to_vec()),
            file_enc_sha256: Some(upload.file_enc_sha256.to_vec()),
            file_length: Some(upload.file_length),
            media_key_timestamp: Some(upload.media_key_timestamp),
            streaming_sidecar: upload.streaming_sidecar,
            mimetype: Some(
                opts.mimetype
                    .unwrap_or_else(|| "audio/ogg; codecs=opus".to_string()),
            ),
            seconds: opts.duration_seconds,
            ptt: opts.ptt,
            waveform: opts.waveform,
            ..Default::default()
        })),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_upload() -> UploadResponse {
        UploadResponse {
            url: "https://cdn/u".into(),
            direct_path: "/d".into(),
            media_key: [1u8; 32],
            file_enc_sha256: [2u8; 32],
            file_sha256: [3u8; 32],
            file_length: 4096,
            media_key_timestamp: 1_700_000_000,
            streaming_sidecar: Some(vec![9, 9, 9]),
        }
    }

    #[test]
    fn image_maps_cdn_fields_and_defaults_mimetype() {
        let msg = image_message(sample_upload(), ImageOptions::default());
        let im = msg.image_message.unwrap();
        assert_eq!(im.url.as_deref(), Some("https://cdn/u"));
        assert_eq!(im.direct_path.as_deref(), Some("/d"));
        assert_eq!(im.media_key.as_deref(), Some(&[1u8; 32][..]));
        assert_eq!(im.file_sha256.as_deref(), Some(&[3u8; 32][..]));
        assert_eq!(im.file_enc_sha256.as_deref(), Some(&[2u8; 32][..]));
        assert_eq!(im.file_length, Some(4096));
        assert_eq!(im.media_key_timestamp, Some(1_700_000_000));
        assert_eq!(im.mimetype.as_deref(), Some("image/jpeg"));
    }

    #[test]
    fn video_carries_sidecar_and_options() {
        let msg = video_message(
            sample_upload(),
            VideoOptions {
                caption: Some("c".into()),
                duration_seconds: Some(12),
                gif_playback: Some(true),
                ..Default::default()
            },
        );
        let vm = msg.video_message.unwrap();
        assert_eq!(vm.streaming_sidecar.as_deref(), Some(&[9, 9, 9][..]));
        assert_eq!(vm.seconds, Some(12));
        assert_eq!(vm.gif_playback, Some(true));
        assert_eq!(vm.caption.as_deref(), Some("c"));
        assert_eq!(vm.mimetype.as_deref(), Some("video/mp4"));
    }

    #[test]
    fn document_and_audio_set_type_specific_fields() {
        let doc = document_message(
            sample_upload(),
            DocumentOptions {
                file_name: Some("f.pdf".into()),
                page_count: Some(3),
                ..Default::default()
            },
        )
        .document_message
        .unwrap();
        assert_eq!(doc.file_name.as_deref(), Some("f.pdf"));
        assert_eq!(doc.page_count, Some(3));
        assert_eq!(doc.mimetype.as_deref(), Some("application/octet-stream"));

        let audio = audio_message(
            sample_upload(),
            AudioOptions {
                ptt: Some(true),
                duration_seconds: Some(5),
                ..Default::default()
            },
        )
        .audio_message
        .unwrap();
        assert_eq!(audio.ptt, Some(true));
        assert_eq!(audio.seconds, Some(5));
        assert_eq!(audio.streaming_sidecar.as_deref(), Some(&[9, 9, 9][..]));
    }
}
