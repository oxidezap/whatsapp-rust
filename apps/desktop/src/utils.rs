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

/// Scale media dimensions to fit within `max_size` without upscaling, with a
/// ~50px floor on the short side. Both fit and floor are single uniform
/// factors so aspect ratio is always preserved; the floor yields to the
/// `max_size` cap, so a pathological ratio (e.g. 200x20) keeps its shape and
/// accepts a sub-50px short side instead of stretching or overflowing.
pub fn scale_media_dimensions(width: u32, height: u32, max_size: f32) -> (f32, f32) {
    let w = width.max(1) as f32;
    let h = height.max(1) as f32;
    let fit = (max_size / w).min(max_size / h).min(1.0);
    let floor = 50.0 / (w.min(h) * fit);
    let cap = (max_size / (w.max(h) * fit)).max(1.0);
    let scale = fit * floor.clamp(1.0, cap);
    (w * scale, h * scale)
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

#[cfg(test)]
mod tests {
    use super::scale_media_dimensions;

    fn assert_close(actual: (f32, f32), expected: (f32, f32)) {
        assert!(
            (actual.0 - expected.0).abs() < 0.01 && (actual.1 - expected.1).abs() < 0.01,
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn shrinks_large_media_uniformly() {
        assert_close(scale_media_dimensions(4000, 3000, 300.0), (300.0, 225.0));
    }

    #[test]
    fn floors_tiny_media_uniformly() {
        assert_close(scale_media_dimensions(10, 10, 300.0), (50.0, 50.0));
    }

    #[test]
    fn extreme_ratio_keeps_shape_and_respects_cap() {
        // 10:1 stays 10:1; the floor grow stops at max_size instead of
        // stretching only the short side (the old per-axis behavior).
        assert_close(scale_media_dimensions(200, 20, 300.0), (300.0, 30.0));
    }
}
