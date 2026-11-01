use super::HistorySecretSeedCollector;
use bytes::Bytes;
use wacore::history_sync::{
    HistorySyncError, HistorySyncResult, process_history_sync_bytes_with_record_sink,
};
use wacore::store::traits::MsgSecretEntry;
use wacore_binary::CompactString;

pub(super) struct HistoryParseJob {
    compressed: Bytes,
    own_user: Option<CompactString>,
    collector: HistorySecretSeedCollector,
}

impl HistoryParseJob {
    pub(super) fn new(
        compressed: Bytes,
        own_user: Option<CompactString>,
        collector: HistorySecretSeedCollector,
    ) -> Self {
        Self {
            compressed,
            own_user,
            collector,
        }
    }

    pub(super) fn runs_inline(compressed_len: usize) -> bool {
        compressed_len < 256 * 1024
    }

    pub(super) fn run(
        mut self,
    ) -> (
        Result<HistorySyncResult, HistorySyncError>,
        Vec<MsgSecretEntry>,
    ) {
        let result = process_history_sync_bytes_with_record_sink(
            self.compressed,
            self.own_user.as_deref(),
            true,
            &mut self.collector,
        );
        (result, self.collector.into_entries())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache_config::{CacheConfig, RuntimeCacheConfig};
    use crate::history_sync::HistorySecretSeedConfig;
    use std::io::Write;
    use waproto::whatsapp as wa;

    fn job(bytes: Bytes) -> HistoryParseJob {
        let config = RuntimeCacheConfig::from(&CacheConfig::default());
        let policy = HistorySecretSeedConfig::at(&config, 100);
        HistoryParseJob::new(
            bytes,
            None,
            HistorySecretSeedCollector::new(policy, None, None),
        )
    }

    #[test]
    fn execution_threshold_is_exact() {
        assert!(HistoryParseJob::runs_inline(256 * 1024 - 1));
        assert!(!HistoryParseJob::runs_inline(256 * 1024));
    }

    #[test]
    fn parse_returns_the_original_compressed_allocation() {
        let mut compressor =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        let history = wa::HistorySync::default();
        compressor
            .write_all(&waproto::codec::history_sync_to_vec(&history))
            .unwrap();
        let bytes = Bytes::from(compressor.finish().unwrap());
        let pointer = bytes.as_ptr();
        let (result, entries) = job(bytes).run();
        let parsed = result.unwrap();
        assert_eq!(parsed.compressed_bytes.unwrap().as_ptr(), pointer);
        assert!(entries.is_empty());
    }

    #[test]
    fn invalid_input_produces_no_seed_entries() {
        let (result, entries) = job(Bytes::from_static(b"invalid zlib")).run();
        assert!(result.is_err());
        assert!(entries.is_empty());
    }

    #[test]
    fn separate_policy_snapshots_use_independent_times() {
        let config = RuntimeCacheConfig::from(&CacheConfig::default());
        let before = HistorySecretSeedConfig::at(&config, 100);
        let after = HistorySecretSeedConfig::at(&config, 200);
        assert_eq!(before.now, 100);
        assert_eq!(after.now, 200);
    }
}
