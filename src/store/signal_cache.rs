// Re-export from wacore — the canonical implementation lives there now.
pub use wacore::store::signal_cache::SignalStoreCache;

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::libsignal::protocol::ProtocolAddress;

    #[tokio::test]
    async fn a_fully_dirty_signal_cache_does_not_allocate_eviction_scratch() {
        let cache = SignalStoreCache::with_max_entries(32);
        for index in 0..256 {
            let address = ProtocolAddress::new(&format!("1999555{index:04}"), 1.into());
            cache.put_identity(&address, &[1; 32]).await;
        }
        let (_, identities, _) = cache.memory_stats().await;
        assert_eq!(identities.entries, 256);
        let address = ProtocolAddress::new("19995550000", 1.into());
        let allocations = crate::test_alloc::min_allocs(0, || {
            assert!(cache.try_put_identity(&address, &[1; 32]));
        });
        assert_eq!(
            allocations, 0,
            "no entry is evictable, so no scan scratch is needed"
        );
    }
}
