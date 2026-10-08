//! Exercise the public storage API using only the consumer package sources.
use wacore::appstate::{hash::HashState, processor::AppStateMutationMAC};
use wacore::store::traits::AppSyncStore;

#[test]
fn downstream_consumer_commits_patch_as_one_operation() {
    futures::executor::block_on(async {
    let store = &wacore::store::InMemoryBackend::new();
    let old = AppStateMutationMAC {
        index_mac: vec![1; 32],
        value_mac: vec![11; 32],
    };
    let replacement = AppStateMutationMAC {
        index_mac: old.index_mac.clone(),
        value_mac: vec![22; 32],
    };
    let removed = AppStateMutationMAC {
        index_mac: vec![2; 32],
        value_mac: vec![33; 32],
    };
    for name in ["regular", "regular_low"] {
        store
            .commit_patch(
                name,
                HashState {
                    version: 1,
                    ..Default::default()
                },
                &[],
                &[old.clone(), removed.clone()],
            )
            .await
            .unwrap();
    }
    store
        .commit_patch(
            "regular",
            HashState {
                version: 2,
                hash: [2; 128],
                ..Default::default()
            },
            &[old.index_mac.clone(), removed.index_mac.clone()],
            std::slice::from_ref(&replacement),
        )
        .await
        .unwrap();
    assert_eq!(
        store
            .get_mutation_mac("regular", &old.index_mac)
            .await
            .unwrap(),
        Some(replacement.value_mac.clone())
    );
    assert!(
        store
            .get_mutation_mac("regular", &removed.index_mac)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .get_mutation_mac("regular_low", &old.index_mac)
            .await
            .unwrap(),
        Some(old.value_mac)
    );
    assert_eq!(
        store
            .get_version("regular_low")
            .await
            .unwrap()
            .unwrap()
            .version,
        1
    );
    let held = store.get_version("regular").await.unwrap().unwrap();
    assert_eq!(held.version, 2);
    assert_eq!(held.hash, [2; 128]);
    store
        .commit_patch("regular", HashState { version: 3, ..held }, &[], &[])
        .await
        .unwrap();
    assert_eq!(
        store.get_version("regular").await.unwrap().unwrap().version,
        3
    );
    assert_eq!(
        store
            .get_mutation_mac("regular", &old.index_mac)
            .await
            .unwrap(),
        Some(replacement.value_mac)
    );
    });
}
