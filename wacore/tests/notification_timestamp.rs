use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use wacore::time::{TimeProvider, Timestamp, set_time_provider};
use wacore_binary::builder::NodeBuilder;

struct FixedTime(Arc<AtomicUsize>);
impl TimeProvider for FixedTime {
    fn now_millis(&self) -> i64 {
        self.0.fetch_add(1, Ordering::SeqCst);
        1_900_000_000_123
    }
}

#[test]
fn notification_timestamps_reject_wrapping_unsigned_values_and_keep_epoch() {
    let reads = Arc::new(AtomicUsize::new(0));
    set_time_provider(FixedTime(reads.clone())).unwrap();
    let fallback = Timestamp::from_millis(1_900_000_000_123).unwrap();
    for (index, value) in [
        u64::MAX.to_string(),
        (i64::MAX as u64 + 1).to_string(),
        (Timestamp::MAX_MILLIS / 1000 + 1).to_string(),
        "invalid".into(),
        "-1".into(),
    ]
    .into_iter()
    .enumerate()
    {
        let node = NodeBuilder::new("notification").attr("t", value).build();
        assert_eq!(
            wacore::stanza::notification::notification_timestamp(&node),
            fallback
        );
        assert_eq!(reads.load(Ordering::SeqCst), index + 1);
    }
    let missing = NodeBuilder::new("notification").build();
    assert_eq!(
        wacore::stanza::notification::notification_timestamp(&missing),
        fallback
    );
    let before = reads.load(Ordering::SeqCst);
    for seconds in [0, 1_773_519_041, Timestamp::MAX_MILLIS / 1000] {
        let node = NodeBuilder::new("notification").attr("t", seconds).build();
        assert_eq!(
            wacore::stanza::notification::notification_timestamp(&node),
            Timestamp::from_secs(seconds).unwrap()
        );
        assert_eq!(reads.load(Ordering::SeqCst), before);
    }
}
