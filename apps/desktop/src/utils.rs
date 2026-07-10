//! Shared utility functions for the WhatsApp UI

use chrono::{DateTime, Local, Utc};
use gpui::ImageFormat;

/// Convert a MIME type string to a GPUI ImageFormat
pub fn mime_to_image_format(mime: &str) -> ImageFormat {
    match mime {
        // image/jpg is non-standard but some senders emit it
        "image/jpeg" | "image/jpg" => ImageFormat::Jpeg,
        "image/png" => ImageFormat::Png,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::Webp,
        "image/bmp" => ImageFormat::Bmp,
        _ => {
            log::warn!("unrecognized image MIME type {mime}, falling back to PNG");
            ImageFormat::Png
        }
    }
}

/// Scale media dimensions to fit within `max_size` without upscaling, with a 50px floor.
pub fn scale_media_dimensions(width: u32, height: u32, max_size: f32) -> (f32, f32) {
    let w = width as f32;
    let h = height as f32;
    let scale = (max_size / w).min(max_size / h).min(1.0);
    ((w * scale).max(50.0), (h * scale).max(50.0))
}

/// Format a UTC timestamp as local time (HH:MM format).
///
/// Converts from UTC to the system's local timezone before formatting.
/// This ensures timestamps are displayed correctly regardless of where
/// the user is located.
pub fn format_time_local(timestamp: &DateTime<Utc>) -> String {
    let local: DateTime<Local> = timestamp.with_timezone(&Local);
    local.format("%H:%M").to_string()
}
