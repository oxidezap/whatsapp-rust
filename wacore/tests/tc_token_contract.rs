#![cfg(not(target_arch = "wasm32"))]
#[path = "support/tc_token_contract.rs"]
mod contract;
use wacore::store::{InMemoryBackend, traits::ProtocolStore};

#[tokio::test]
async fn memory_preserves_both_token_fields_and_store_isolation() {
    let a = InMemoryBackend::new();
    let b = InMemoryBackend::new();
    b.store_received_tc_token("100001@lid", b"other-device", 10)
        .await
        .unwrap();
    b.touch_tc_token_sender_timestamp("100001@lid", 20)
        .await
        .unwrap();
    contract::assert_tc_token_contract(&a).await;
    let other = b.get_tc_token("100001@lid").await.unwrap().unwrap();
    assert_eq!(other.token, b"other-device");
    assert_eq!(other.token_timestamp, 10);
    assert_eq!(other.sender_timestamp, Some(20));
    assert!(b.get_tc_token("race-0@lid").await.unwrap().is_none());
}

#[test]
fn memory_token_merges_contend_across_threads() {
    use std::sync::Barrier;

    // Separate OS threads can contend even when each individual async lock
    // acquisition is immediately ready; join! on one executor cannot force that.
    let store = InMemoryBackend::new();
    let barrier = Barrier::new(4);
    let mut mismatches = Vec::new();
    std::thread::scope(|scope| {
        for writer in 0..3 {
            let store = &store;
            let barrier = &barrier;
            scope.spawn(move || {
                for round in 0..512 {
                    let jid = format!("thread-race-{round}@lid");
                    barrier.wait();
                    futures::executor::block_on(async {
                        match writer {
                            0 => store.store_received_tc_token(&jid, b"old", 100).await,
                            1 => store.store_received_tc_token(&jid, b"new", 200).await,
                            _ => store.touch_tc_token_sender_timestamp(&jid, 300).await,
                        }
                        .unwrap();
                    });
                    barrier.wait();
                }
            });
        }
        for round in 0..512 {
            barrier.wait();
            barrier.wait();
            let row = futures::executor::block_on(
                store.get_tc_token(&format!("thread-race-{round}@lid")),
            )
            .unwrap()
            .unwrap();
            if row.token != b"new"
                || row.token_timestamp != 200
                || row.sender_timestamp != Some(300)
            {
                mismatches.push((round, row));
            }
        }
    });
    assert!(
        mismatches.is_empty(),
        "lost concurrent updates: {mismatches:?}"
    );
}
