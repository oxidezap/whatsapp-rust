//! Independent downstream proof: no default features, no live WhatsApp session.
use std::sync::Arc;
use std::time::Duration;
use whatsapp_rust::anyhow::Result;
use whatsapp_rust::cache_config::CacheStore;
use whatsapp_rust::media::{AudioOptions, DocumentOptions, ImageOptions, VideoOptions};
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust::{
    BusinessError, CacheConfig, CacheEntryConfig, CacheStores, CatalogOptions, CollectionOptions,
    ProductAvailability, async_trait,
};

mod negative;

/// Host-owned mock: the cache trait remains implementable outside the library.
pub struct EmptyCache;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl CacheStore for EmptyCache {
    async fn get(&self, _: &str, _: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }
    async fn set(&self, _: &str, _: &str, _: &[u8], _: Option<Duration>) -> Result<()> {
        Ok(())
    }
    async fn delete(&self, _: &str, _: &str) -> Result<()> {
        Ok(())
    }
    async fn clear(&self, _: &str) -> Result<()> {
        Ok(())
    }
}

/// Compile this on native and WASM, even when tests are not being run.
pub fn construct_inputs_and_mock() -> CacheConfig {
    let image = ImageOptions::default()
        .with_caption("image")
        .with_mimetype("image/png")
        .with_jpeg_thumbnail(vec![1, 2])
        .with_context_info(Box::new(wa::ContextInfo::default()));
    let ImageOptions { caption, .. } = &image;
    assert_eq!(caption.as_deref(), Some("image"));
    assert_eq!(image.mimetype.as_deref(), Some("image/png"));
    assert_eq!(image.jpeg_thumbnail.as_deref(), Some(&[1, 2][..]));
    assert!(image.context_info.is_some());

    let video = VideoOptions::default()
        .with_caption("video")
        .with_mimetype("video/mp4")
        .with_jpeg_thumbnail(vec![3])
        .with_duration_seconds(12)
        .with_gif_playback(false)
        .with_context_info(Box::new(wa::ContextInfo::default()));
    let VideoOptions {
        duration_seconds, ..
    } = video;
    assert_eq!(duration_seconds, Some(12));
    assert_eq!(video.caption.as_deref(), Some("video"));
    assert_eq!(video.mimetype.as_deref(), Some("video/mp4"));
    assert_eq!(video.jpeg_thumbnail.as_deref(), Some(&[3][..]));
    assert_eq!(video.gif_playback, Some(false));
    assert!(video.context_info.is_some());

    let document = DocumentOptions::default()
        .with_mimetype("application/pdf")
        .with_file_name("manual.pdf")
        .with_title("Manual")
        .with_caption("document")
        .with_page_count(3)
        .with_jpeg_thumbnail(vec![4])
        .with_context_info(Box::new(wa::ContextInfo::default()));
    let DocumentOptions { page_count, .. } = document;
    assert_eq!(page_count, Some(3));
    assert_eq!(document.mimetype.as_deref(), Some("application/pdf"));
    assert_eq!(document.file_name.as_deref(), Some("manual.pdf"));
    assert_eq!(document.title.as_deref(), Some("Manual"));
    assert_eq!(document.caption.as_deref(), Some("document"));
    assert_eq!(document.jpeg_thumbnail.as_deref(), Some(&[4][..]));
    assert!(document.context_info.is_some());

    let mut audio = AudioOptions::default()
        .with_mimetype("audio/ogg; codecs=opus")
        .with_duration_seconds(5)
        .with_ptt(true)
        .with_waveform(vec![5])
        .with_context_info(Box::new(wa::ContextInfo::default()));
    let AudioOptions { ptt, .. } = audio;
    assert_eq!(ptt, Some(true));
    assert_eq!(audio.mimetype.as_deref(), Some("audio/ogg; codecs=opus"));
    assert_eq!(audio.duration_seconds, Some(5));
    assert_eq!(audio.waveform.as_deref(), Some(&[5][..]));
    assert!(audio.context_info.is_some());
    audio.ptt = None; // Public optional fields remain writable, including clearing.
    assert_eq!(audio.ptt, None);

    let catalog = CatalogOptions::default()
        .with_limit(20)
        .with_after("opaque catalog cursor")
        .with_image_dimensions(80, 60)
        .with_allow_shop_source(false);
    let CatalogOptions { limit, .. } = catalog;
    assert_eq!(limit, 20);
    assert_eq!(catalog.after.as_deref(), Some("opaque catalog cursor"));
    assert_eq!((catalog.image_width, catalog.image_height), (80, 60));
    assert!(!catalog.allow_shop_source);
    let collections = CollectionOptions::default()
        .with_collection_limit(2)
        .with_item_limit(4)
        .with_after("opaque collection cursor")
        .with_image_dimensions(100, 120);
    let CollectionOptions { item_limit, .. } = collections;
    assert_eq!(item_limit, 4);
    assert_eq!(collections.collection_limit, 2);
    assert_eq!(
        collections.after.as_deref(),
        Some("opaque collection cursor")
    );
    assert_eq!(
        (collections.image_width, collections.image_height),
        (100, 120)
    );

    let backend: Arc<dyn CacheStore> = Arc::new(EmptyCache);
    let stores = CacheStores::default()
        .with_group_cache(backend.clone())
        .with_device_registry_cache(backend.clone())
        .with_lid_pn_cache(backend.clone());
    let CacheStores { group_cache, .. } = &stores;
    assert!(
        group_cache
            .as_ref()
            .is_some_and(|store| Arc::ptr_eq(store, &backend))
    );
    assert!(
        stores
            .device_registry_cache
            .as_ref()
            .is_some_and(|store| Arc::ptr_eq(store, &backend))
    );
    assert!(
        stores
            .lid_pn_cache
            .as_ref()
            .is_some_and(|store| Arc::ptr_eq(store, &backend))
    );
    let all = CacheStores::all(backend.clone());
    assert!(
        all.group_cache
            .as_ref()
            .is_some_and(|store| Arc::ptr_eq(store, &backend))
    );
    assert!(
        all.device_registry_cache
            .as_ref()
            .is_some_and(|store| Arc::ptr_eq(store, &backend))
    );
    assert!(
        all.lid_pn_cache
            .as_ref()
            .is_some_and(|store| Arc::ptr_eq(store, &backend))
    );

    let entry = CacheEntryConfig::new(None, 42);
    let CacheEntryConfig { capacity, .. } = entry;
    assert_eq!(capacity, 42);
    let mut config = CacheConfig::default()
        .with_group_cache(entry)
        .with_device_registry_cache(CacheEntryConfig::new(Some(Duration::from_secs(30)), 24))
        .with_lid_pn_cache(CacheEntryConfig::new(None, u64::MAX))
        .with_recent_messages(CacheEntryConfig::new(Some(Duration::from_secs(60)), 10))
        .with_cache_stores(stores);
    config.chat_lanes_capacity = 1_000; // Advanced tuning still uses public fields.
    let CacheConfig { group_cache, .. } = &config;
    assert_eq!(group_cache.capacity, 42);
    assert_eq!(group_cache.timeout, None);
    assert_eq!(config.device_registry_cache.capacity, 24);
    assert_eq!(
        config.device_registry_cache.timeout,
        Some(Duration::from_secs(30))
    );
    assert_eq!(config.lid_pn_cache.capacity, u64::MAX);
    assert_eq!(config.recent_messages.capacity, 10);
    assert_eq!(
        config.recent_messages.timeout,
        Some(Duration::from_secs(60))
    );
    assert_eq!(config.chat_lanes_capacity, 1_000);
    assert!(config.cache_stores.group_cache.is_some());
    config
}

/// Compile the supported cache argument from the bot's otherwise ignored example.
pub fn configure_bot_cache_example() {
    let _ = whatsapp_rust::bot::Bot::builder().with_cache_config(
        CacheConfig::default()
            .with_group_cache(CacheEntryConfig::new(None, 1_000))
            .with_device_registry_cache(CacheEntryConfig::new(None, 5_000)),
    );
}

/// Extensible enum matching preserves unknown wire values and future variants.
pub fn availability_label(value: &ProductAvailability) -> &str {
    match value {
        ProductAvailability::InStock => "available",
        ProductAvailability::OutOfStock => "unavailable",
        ProductAvailability::AvailableForAnotherPostcode => "elsewhere",
        ProductAvailability::Other(raw) => raw,
        _ => "future variant",
    }
}

pub fn business_error_operation(error: &BusinessError) -> Option<&str> {
    match error {
        BusinessError::MalformedResponse { operation, .. } => Some(operation),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::futures::FutureExt;

    #[test]
    fn construction_reading_and_host_mock_work() {
        configure_bot_cache_example();
        let config = construct_inputs_and_mock();
        let store = config.cache_stores.group_cache.unwrap();
        // This mock never suspends; no executor feature is needed in the host.
        async {
            store.set("group", "fixture", b"mock", None).await.unwrap();
            assert!(store.get("group", "fixture").await.unwrap().is_none());
            store.delete("group", "fixture").await.unwrap();
            store.clear("group").await.unwrap();
            assert_eq!(store.entry_count("group").await.unwrap(), 0);
        }
        .now_or_never()
        .expect("mock cache operations are immediately ready");
    }

    #[test]
    fn neutral_defaults_and_extensible_patterns_work() {
        assert!(ImageOptions::default().caption.is_none());
        assert!(VideoOptions::default().gif_playback.is_none());
        assert!(DocumentOptions::default().file_name.is_none());
        assert!(AudioOptions::default().ptt.is_none());
        assert_eq!(CatalogOptions::default().limit, 10);
        assert_eq!(CollectionOptions::default().item_limit, 51);
        assert!(CacheStores::default().group_cache.is_none());
        assert_eq!(CacheConfig::default().recent_messages.capacity, 0);
        assert_eq!(
            availability_label(&ProductAvailability::Other("FUTURE".into())),
            "FUTURE"
        );
        let mock_error = BusinessError::MalformedResponse {
            operation: "mock catalog",
            detail: "missing products".into(),
        };
        assert_eq!(business_error_operation(&mock_error), Some("mock catalog"));
    }
}
