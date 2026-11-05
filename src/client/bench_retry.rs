//! Offline measurements of the client's real retry-count bookkeeping.

use crate::bench_support::MultiLaneReceiveHarness;

/// A client with its normal cache policy. The existing offline fixture owns
/// its runtime, but these portable futures never poll its background workers.
pub struct RetryCountHarness(MultiLaneReceiveHarness);

impl Default for RetryCountHarness {
    fn default() -> Self {
        Self(MultiLaneReceiveHarness::new(1))
    }
}

impl RetryCountHarness {
    /// Reset retry bookkeeping outside measurement.
    pub async fn prepare_retry_counts(&self, keys: &[String], count: u8) {
        let client = self.0.client();
        for key in keys {
            client.message_retry_counts.invalidate(key).await;
            if count != 0 {
                client.preseed_retry_count(key, count).await;
            }
        }
    }

    /// Execute the real saturation decision and update without decrypting a
    /// message or sending a retry receipt.
    pub async fn increment_retry_counts(&self, keys: &[String]) -> usize {
        let client = self.0.client();
        let mut sum = 0usize;
        for key in keys {
            if let Some(count) = client
                .increment_retry_count(key, wacore::protocol::retry::RetryReason::BadMac)
                .await
            {
                sum += usize::from(count);
            }
        }
        sum
    }
}
