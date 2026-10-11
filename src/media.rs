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
//! let _msg = media::image_message(upload, ImageOptions::default().with_caption("hi"));
//! # }
//! ```

use crate::upload::UploadResponse;
use waproto::whatsapp as wa;

/// Optional image metadata. An empty configuration uses the default MIME type.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct ImageOptions {
    pub caption: Option<String>,
    /// Defaults to `image/jpeg`.
    pub mimetype: Option<String>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub context_info: Option<Box<wa::ContextInfo>>,
}

/// Optional video metadata. An empty configuration uses the default MIME type.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct VideoOptions {
    pub caption: Option<String>,
    /// Defaults to `video/mp4`.
    pub mimetype: Option<String>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub duration_seconds: Option<u32>,
    /// Send as a looping GIF-style clip.
    pub gif_playback: Option<bool>,
    pub context_info: Option<Box<wa::ContextInfo>>,
}

/// Optional document metadata. An empty configuration uses the default MIME type.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct DocumentOptions {
    /// Defaults to `application/octet-stream`.
    pub mimetype: Option<String>,
    /// File name shown to the recipient.
    pub file_name: Option<String>,
    pub title: Option<String>,
    pub caption: Option<String>,
    pub page_count: Option<u32>,
    pub jpeg_thumbnail: Option<Vec<u8>>,
    pub context_info: Option<Box<wa::ContextInfo>>,
}

/// Optional audio metadata. An empty configuration uses the default MIME type.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct AudioOptions {
    /// Defaults to `audio/ogg; codecs=opus`.
    pub mimetype: Option<String>,
    pub duration_seconds: Option<u32>,
    /// Push-to-talk (voice note) flag.
    pub ptt: Option<bool>,
    /// PCM waveform preview bytes (voice notes).
    pub waveform: Option<Vec<u8>>,
    pub context_info: Option<Box<wa::ContextInfo>>,
}

impl ImageOptions {
    /// Set the image caption.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Override the default MIME type.
    pub fn with_mimetype(mut self, mimetype: impl Into<String>) -> Self {
        self.mimetype = Some(mimetype.into());
        self
    }

    /// Set a JPEG thumbnail.
    pub fn with_jpeg_thumbnail(mut self, thumbnail: Vec<u8>) -> Self {
        self.jpeg_thumbnail = Some(thumbnail);
        self
    }

    /// Attach reply, mention or other message context.
    pub fn with_context_info(mut self, context_info: Box<wa::ContextInfo>) -> Self {
        self.context_info = Some(context_info);
        self
    }
}

impl VideoOptions {
    /// Set the video caption.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Override the default MIME type.
    pub fn with_mimetype(mut self, mimetype: impl Into<String>) -> Self {
        self.mimetype = Some(mimetype.into());
        self
    }

    /// Set a JPEG thumbnail.
    pub fn with_jpeg_thumbnail(mut self, thumbnail: Vec<u8>) -> Self {
        self.jpeg_thumbnail = Some(thumbnail);
        self
    }

    /// Set the clip duration in seconds.
    pub fn with_duration_seconds(mut self, seconds: u32) -> Self {
        self.duration_seconds = Some(seconds);
        self
    }

    /// Set whether to send as a looping GIF-style clip.
    pub fn with_gif_playback(mut self, enabled: bool) -> Self {
        self.gif_playback = Some(enabled);
        self
    }

    /// Attach reply, mention or other message context.
    pub fn with_context_info(mut self, context_info: Box<wa::ContextInfo>) -> Self {
        self.context_info = Some(context_info);
        self
    }
}

impl DocumentOptions {
    /// Override the default MIME type.
    pub fn with_mimetype(mut self, mimetype: impl Into<String>) -> Self {
        self.mimetype = Some(mimetype.into());
        self
    }

    /// Set the file name shown to the recipient.
    pub fn with_file_name(mut self, file_name: impl Into<String>) -> Self {
        self.file_name = Some(file_name.into());
        self
    }

    /// Set the document title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the document caption.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Set the document page count.
    pub fn with_page_count(mut self, page_count: u32) -> Self {
        self.page_count = Some(page_count);
        self
    }

    /// Set a JPEG thumbnail.
    pub fn with_jpeg_thumbnail(mut self, thumbnail: Vec<u8>) -> Self {
        self.jpeg_thumbnail = Some(thumbnail);
        self
    }

    /// Attach reply, mention or other message context.
    pub fn with_context_info(mut self, context_info: Box<wa::ContextInfo>) -> Self {
        self.context_info = Some(context_info);
        self
    }
}

impl AudioOptions {
    /// Override the default MIME type.
    pub fn with_mimetype(mut self, mimetype: impl Into<String>) -> Self {
        self.mimetype = Some(mimetype.into());
        self
    }

    /// Set the audio duration in seconds.
    pub fn with_duration_seconds(mut self, seconds: u32) -> Self {
        self.duration_seconds = Some(seconds);
        self
    }

    /// Set the push-to-talk (voice note) flag.
    pub fn with_ptt(mut self, enabled: bool) -> Self {
        self.ptt = Some(enabled);
        self
    }

    /// Set waveform preview bytes.
    pub fn with_waveform(mut self, waveform: Vec<u8>) -> Self {
        self.waveform = Some(waveform);
        self
    }

    /// Attach reply, mention or other message context.
    pub fn with_context_info(mut self, context_info: Box<wa::ContextInfo>) -> Self {
        self.context_info = Some(context_info);
        self
    }
}

/// Build an image message from an upload result.
pub fn image_message(upload: UploadResponse, opts: ImageOptions) -> wa::Message {
    wa::Message {
        image_message: buffa::MessageField::some(wa::message::ImageMessage {
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
            context_info: opts
                .context_info
                .map(|ci| buffa::MessageField::some(*ci))
                .unwrap_or_default(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Build a video message from an upload result. Carries the streaming sidecar
/// (progressive-playback HMAC table) from the upload when present.
pub fn video_message(upload: UploadResponse, opts: VideoOptions) -> wa::Message {
    wa::Message {
        video_message: buffa::MessageField::some(wa::message::VideoMessage {
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
            context_info: opts
                .context_info
                .map(|ci| buffa::MessageField::some(*ci))
                .unwrap_or_default(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Build a document message from an upload result.
pub fn document_message(upload: UploadResponse, opts: DocumentOptions) -> wa::Message {
    wa::Message {
        document_message: buffa::MessageField::some(wa::message::DocumentMessage {
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
            context_info: opts
                .context_info
                .map(|ci| buffa::MessageField::some(*ci))
                .unwrap_or_default(),
            ..Default::default()
        }),
        ..Default::default()
    }
}

/// Build an audio / voice-note message from an upload result. Carries the
/// streaming sidecar from the upload when present.
pub fn audio_message(upload: UploadResponse, opts: AudioOptions) -> wa::Message {
    wa::Message {
        audio_message: buffa::MessageField::some(wa::message::AudioMessage {
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
            context_info: opts
                .context_info
                .map(|ci| buffa::MessageField::some(*ci))
                .unwrap_or_default(),
            ..Default::default()
        }),
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
            VideoOptions::default()
                .with_caption("c")
                .with_duration_seconds(12)
                .with_gif_playback(true),
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
            DocumentOptions::default()
                .with_file_name("f.pdf")
                .with_page_count(3),
        )
        .document_message
        .unwrap();
        assert_eq!(doc.file_name.as_deref(), Some("f.pdf"));
        assert_eq!(doc.page_count, Some(3));
        assert_eq!(doc.mimetype.as_deref(), Some("application/octet-stream"));

        let audio = audio_message(
            sample_upload(),
            AudioOptions::default()
                .with_ptt(true)
                .with_duration_seconds(5),
        )
        .audio_message
        .unwrap();
        assert_eq!(audio.ptt, Some(true));
        assert_eq!(audio.seconds, Some(5));
        assert_eq!(audio.streaming_sidecar.as_deref(), Some(&[9, 9, 9][..]));
    }

    #[test]
    fn image_maps_context_info() {
        let context = Box::new(wa::ContextInfo::default());

        let image_msg = image_message(
            sample_upload(),
            ImageOptions::default().with_context_info(context),
        );

        let image = image_msg.image_message.unwrap();

        assert!(image.context_info.is_set());
    }

    #[test]
    fn video_maps_context_info() {
        let context = Box::new(wa::ContextInfo::default());

        let video_msg = video_message(
            sample_upload(),
            VideoOptions::default().with_context_info(context),
        );

        let video = video_msg.video_message.unwrap();

        assert!(video.context_info.is_set());
    }
}
