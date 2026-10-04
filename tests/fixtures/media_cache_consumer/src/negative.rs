//! Intentionally invalid modes, compiled one at a time by the negative test.
#![allow(unused_imports)]
use super::*;

#[cfg(feature = "old-image-literal")]
pub fn old_image_literal() {
    let _ = ImageOptions {
        caption: Some("old".into()),
        ..Default::default()
    };
}

#[cfg(feature = "old-video-literal")]
pub fn old_video_literal() {
    let _ = VideoOptions {
        duration_seconds: Some(12),
        ..Default::default()
    };
}

#[cfg(feature = "old-document-literal")]
pub fn old_document_literal() {
    let _ = DocumentOptions {
        file_name: Some("old.pdf".into()),
        ..Default::default()
    };
}

#[cfg(feature = "old-audio-literal")]
pub fn old_audio_literal() {
    let _ = AudioOptions {
        ptt: Some(true),
        ..Default::default()
    };
}

#[cfg(feature = "old-catalog-literal")]
pub fn old_catalog_literal() {
    let _ = CatalogOptions {
        limit: 20,
        ..Default::default()
    };
}

#[cfg(feature = "old-collection-literal")]
pub fn old_collection_literal() {
    let _ = CollectionOptions {
        item_limit: 20,
        ..Default::default()
    };
}

#[cfg(feature = "old-cache-config-literal")]
pub fn old_cache_config_literal() {
    let _ = CacheConfig {
        chat_lanes_capacity: 1_000,
        ..Default::default()
    };
}

#[cfg(feature = "old-cache-entry-literal")]
pub fn old_cache_entry_literal() {
    let _ = CacheEntryConfig {
        timeout: None,
        capacity: 42,
    };
}

#[cfg(feature = "old-cache-stores-literal")]
pub fn old_cache_stores_literal() {
    let _ = CacheStores {
        group_cache: Some(Arc::new(EmptyCache)),
        ..Default::default()
    };
}

#[cfg(feature = "exhaustive-product-availability")]
pub fn exhaustive_availability(value: ProductAvailability) {
    match value {
        ProductAvailability::InStock => (),
        ProductAvailability::OutOfStock => (),
        ProductAvailability::AvailableForAnotherPostcode => (),
        ProductAvailability::Other(_) => (),
    }
}
