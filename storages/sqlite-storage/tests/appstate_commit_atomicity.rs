#![cfg(not(target_family = "wasm"))]
#[path = "support/storage_fixture.rs"]
mod fixture;

use diesel::connection::SimpleConnection;
use wacore::appstate::{hash::HashState, processor::AppStateMutationMAC};
use wacore::store::{error::StoreError, traits::AppSyncStore};
use whatsapp_rust_sqlite_storage::SqliteDatabase;

#[tokio::test]
async fn patch_write_failure_rolls_back_cursor_and_both_mac_changes() {
    for statement in [
        "INSERT ON app_state_versions",
        "DELETE ON app_state_mutation_macs",
        "INSERT ON app_state_mutation_macs",
    ] {
        let fixture = fixture::Fixture::new();
        let db = SqliteDatabase::open(&fixture.url(), Default::default())
            .await
            .unwrap();
        let store = db.provision_device(1).await.unwrap();
        let other = db.create_device().await.unwrap();
        let before = HashState {
            version: 1,
            hash: [1; 128],
            ..Default::default()
        };
        let after = HashState {
            version: 2,
            hash: [2; 128],
            ..Default::default()
        };
        let old = AppStateMutationMAC {
            index_mac: vec![1; 32],
            value_mac: vec![11; 32],
        };
        let new = AppStateMutationMAC {
            index_mac: vec![2; 32],
            value_mac: vec![22; 32],
        };
        for scope in [&store, &other] {
            scope
                .commit_patch("regular", before.clone(), &[], std::slice::from_ref(&old))
                .await
                .unwrap();
        }
        let trigger = format!(
            "CREATE TRIGGER fail_commit BEFORE {statement} BEGIN SELECT RAISE(ABORT, 'synthetic commit failure'); END;"
        );
        db.shared()
            .run(move |conn| {
                conn.batch_execute(&trigger)
                    .map_err(|e| StoreError::Database(Box::new(e)))
            })
            .await
            .unwrap();
        let err = store
            .commit_patch(
                "regular",
                after.clone(),
                std::slice::from_ref(&old.index_mac),
                std::slice::from_ref(&new),
            )
            .await
            .unwrap_err();
        let source = std::error::Error::source(&err).expect("database error keeps its cause");
        assert!(source.downcast_ref::<diesel::result::Error>().is_some());
        assert!(
            source.to_string().contains("synthetic commit failure"),
            "{source}"
        );
        for scope in [&store, &other] {
            let held = scope.get_version("regular").await.unwrap().unwrap();
            assert_eq!(held.version, before.version, "{statement}");
            assert_eq!(held.hash, before.hash);
            assert_eq!(
                scope
                    .get_mutation_mac("regular", &old.index_mac)
                    .await
                    .unwrap(),
                Some(old.value_mac.clone())
            );
            assert!(
                scope
                    .get_mutation_mac("regular", &new.index_mac)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        db.shared()
            .run(|conn| {
                conn.batch_execute("DROP TRIGGER fail_commit;")
                    .map_err(|e| StoreError::Database(Box::new(e)))
            })
            .await
            .unwrap();
        store
            .commit_patch(
                "regular",
                after,
                std::slice::from_ref(&old.index_mac),
                std::slice::from_ref(&new),
            )
            .await
            .unwrap();
        assert_eq!(
            store.get_version("regular").await.unwrap().unwrap().version,
            2
        );
        assert!(
            store
                .get_mutation_mac("regular", &old.index_mac)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            store
                .get_mutation_mac("regular", &new.index_mac)
                .await
                .unwrap(),
            Some(new.value_mac)
        );
        assert_eq!(
            other.get_version("regular").await.unwrap().unwrap().version,
            1
        );
    }
}

#[tokio::test]
async fn post_commit_barrier_failure_is_atomic_but_does_not_mean_rollback() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use whatsapp_rust_sqlite_storage::{
        CommitBarrierError, CommitBarrierHook, SqliteDatabaseConfig,
    };
    let fail = Arc::new(AtomicBool::new(false));
    let hook: CommitBarrierHook = {
        let fail = fail.clone();
        Arc::new(move || {
            let fail = fail.clone();
            Box::pin(async move {
                if fail.load(Ordering::SeqCst) {
                    Err(StoreError::Validation(
                        "synthetic durability failure".into(),
                    ))
                } else {
                    Ok(())
                }
            })
        })
    };
    let fixture = fixture::Fixture::new();
    let db = SqliteDatabase::open(
        &fixture.url(),
        SqliteDatabaseConfig::default().with_commit_barrier(hook),
    )
    .await
    .unwrap();
    let store = db.provision_device(1).await.unwrap();
    let old = AppStateMutationMAC {
        index_mac: vec![1; 32],
        value_mac: vec![11; 32],
    };
    let new = AppStateMutationMAC {
        index_mac: vec![2; 32],
        value_mac: vec![22; 32],
    };
    store
        .commit_patch(
            "regular",
            HashState {
                version: 1,
                ..Default::default()
            },
            &[],
            std::slice::from_ref(&old),
        )
        .await
        .unwrap();
    fail.store(true, Ordering::SeqCst);
    let error = store
        .commit_patch(
            "regular",
            HashState {
                version: 2,
                ..Default::default()
            },
            std::slice::from_ref(&old.index_mac),
            std::slice::from_ref(&new),
        )
        .await
        .unwrap_err();
    let cause = std::error::Error::source(&error).unwrap();
    assert!(cause.downcast_ref::<CommitBarrierError>().is_some());
    assert_eq!(
        store.get_version("regular").await.unwrap().unwrap().version,
        2
    );
    assert!(
        store
            .get_mutation_mac("regular", &old.index_mac)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .get_mutation_mac("regular", &new.index_mac)
            .await
            .unwrap(),
        Some(new.value_mac)
    );
}

#[path = "../../../wacore/tests/support/appstate_commit_contract.rs"]
mod contract;

#[tokio::test]
async fn sqlite_patch_commit_preserves_order_and_collection_scope() {
    let fixture = fixture::Fixture::new();
    let db = SqliteDatabase::open(&fixture.url(), Default::default())
        .await
        .unwrap();
    let store = db.provision_device(1).await.unwrap();
    contract::assert_patch_commit_contract(&store).await;
}
