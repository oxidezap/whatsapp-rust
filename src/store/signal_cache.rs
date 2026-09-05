// Re-export from wacore — the canonical implementation lives there now.
pub use wacore::store::signal_cache::SignalStoreCache;

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use wacore::libsignal::protocol::ProtocolAddress;
    use wacore::store::in_memory::InMemoryBackend;
    use wacore::store::traits::SignalStore;

    #[tokio::test]
    async fn signal_eviction_scratch_scales_with_overflow_not_resident_entries() {
        let backend = InMemoryBackend::new();
        let cache = SignalStoreCache::with_max_entries(256);
        let addresses: Vec<_> = (0..289)
            .map(|index| ProtocolAddress::new(&format!("eviction-user-{index:04}"), 1.into()))
            .collect();
        for address in &addresses {
            backend
                .put_identity(address.as_str(), [1; 32])
                .await
                .unwrap();
            cache.get_identity(address, &backend).await.unwrap();
        }

        // Clear retains table capacity. Reloading the same rows forces an
        // eviction in every sample without including initial table growth.
        let largest = crate::test_alloc::min_max_block(2048, || {
            cache.clear_after_flush().now_or_never().unwrap();
            for address in &addresses {
                let loaded = cache
                    .get_identity(address, &backend)
                    .now_or_never()
                    .unwrap()
                    .unwrap()
                    .unwrap();
                assert_eq!(loaded.as_ref(), &[1; 32]);
            }
        });
        let (_, identities, _) = cache.memory_stats().await;
        assert_eq!(identities.entries, 256);
        eprintln!("largest eviction-path allocation: {largest} bytes");
        assert!(
            largest <= 2048,
            "eviction requested a {largest}-byte temporary block"
        );
    }

    #[tokio::test]
    async fn a_fully_dirty_signal_cache_does_not_allocate_eviction_scratch() {
        let cache = SignalStoreCache::with_max_entries(32);
        for index in 0..256 {
            let address = ProtocolAddress::new(&format!("cache-user-{index:04}"), 1.into());
            cache.put_identity(&address, &[1; 32]).await;
        }
        let (_, identities, _) = cache.memory_stats().await;
        assert_eq!(identities.entries, 256);
        let address = ProtocolAddress::new("cache-user-0000", 1.into());
        let allocations = crate::test_alloc::min_allocs(0, || {
            assert!(cache.try_put_identity(&address, &[1; 32]));
        });
        assert_eq!(
            allocations, 0,
            "no entry is evictable, so no scan scratch is needed"
        );
    }
}
