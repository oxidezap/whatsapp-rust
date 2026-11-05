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

#[cfg(test)]
mod tests {
    use super::*;
    use std::future::Future;
    use std::pin::pin;
    use std::task::{Context, Poll, Waker};

    fn complete_without_runtime<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        match future.as_mut().poll(&mut context) {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("retry bookkeeping unexpectedly needs a runtime wakeup"),
        }
    }

    #[test]
    fn retry_fixture_completes_without_polling_background_tasks() {
        let fixture = RetryCountHarness::default();
        let keys = [
            "synthetic-message-a".to_owned(),
            "synthetic-message-b".to_owned(),
        ];
        for (seed, expected) in [(0, 2), (3, 8), (5, 0)] {
            complete_without_runtime(fixture.prepare_retry_counts(&keys, seed));
            assert_eq!(
                complete_without_runtime(fixture.increment_retry_counts(&keys)),
                expected,
            );
        }
    }
}
