#![cfg(not(target_family = "wasm"))]

use bytes::Bytes;
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use wacore::store::error::StoreError;
use wacore::store::traits::{
    MsgSecretEntry, MsgSecretStore, ProtocolStore, SignalStore, TcTokenEntry,
};
use whatsapp_rust_sqlite_storage::{
    CommitBarrierError, CommitBarrierHook, SqliteDatabase, SqliteDatabaseConfig, Synchronous,
};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cache/sqlite-prepared-tests");
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir.join(format!(
            "{}-{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    fn url(&self) -> &str {
        self.0.to_str().unwrap()
    }

    async fn open(&self) -> SqliteDatabase {
        SqliteDatabase::open(
            self.url(),
            SqliteDatabaseConfig {
                synchronous: Synchronous::Full,
                ..Default::default()
            },
        )
        .await
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm", "-journal"] {
            let mut path = self.0.as_os_str().to_owned();
            path.push(suffix);
            let _ = std::fs::remove_file(path);
        }
    }
}

fn secret(id: &str, expires_at: i64, message_ts: i64, byte: u8) -> MsgSecretEntry {
    MsgSecretEntry {
        chat: "15550000001@s.whatsapp.net".into(),
        sender: "15550000002@lid".into(),
        msg_id: id.into(),
        secret: [byte; 32],
        expires_at,
        message_ts,
    }
}

#[derive(Debug, PartialEq, Eq, QueryableByName)]
struct SecretRow {
    #[diesel(sql_type = diesel::sql_types::Binary)]
    secret: Vec<u8>,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    expires_at: i64,
    #[diesel(sql_type = diesel::sql_types::BigInt)]
    message_ts: i64,
}

async fn rows(db: &SqliteDatabase, device_id: i32) -> Vec<SecretRow> {
    db.shared().read(move |conn| {
        diesel::sql_query("SELECT secret, expires_at, message_ts FROM msg_secrets WHERE device_id = ? ORDER BY msg_id")
            .bind::<diesel::sql_types::Integer, _>(device_id)
            .load(conn).map_err(|e| StoreError::Database(Box::new(e)))
    }).await.unwrap()
}

async fn uploaded(db: &SqliteDatabase, device: i32) -> i64 {
    #[derive(QueryableByName)]
    struct Count {
        #[diesel(sql_type = diesel::sql_types::BigInt)]
        count: i64,
    }
    db.shared()
        .read(move |conn| {
            diesel::sql_query(
                "SELECT COUNT(*) AS count FROM prekeys WHERE device_id = ? AND uploaded = 1",
            )
            .bind::<diesel::sql_types::Integer, _>(device)
            .get_result::<Count>(conn)
            .map(|row| row.count)
            .map_err(|e| StoreError::Database(Box::new(e)))
        })
        .await
        .unwrap()
}

#[tokio::test]
async fn token_unique_lookup_round_trips_bound_values_and_device_scope_after_reopen() {
    let fixture = Fixture::new();
    let keys = ["", "15550000001", "' OR 1=1; --", "á東京\0fim"];
    {
        let db = fixture.open().await;
        for device_id in [1, 2] {
            let store = db.store(device_id);
            for (i, key) in keys.iter().enumerate() {
                let entry = TcTokenEntry {
                    token: vec![0, 255, device_id as u8, i as u8],
                    token_timestamp: i as i64,
                    sender_timestamp: (i % 2 == 0).then_some(42),
                };
                store.put_tc_token(key, &entry).await.unwrap();
            }
        }
    }
    let db = fixture.open().await;
    for device_id in [1, 2] {
        let store = db.store(device_id);
        for (i, key) in keys.iter().enumerate() {
            let entry = store.get_tc_token(key).await.unwrap().unwrap();
            assert_eq!(entry.token, [0, 255, device_id as u8, i as u8]);
            assert_eq!(entry.token_timestamp, i as i64);
            assert_eq!(entry.sender_timestamp, (i % 2 == 0).then_some(42));
        }
        assert!(store.get_tc_token("absent").await.unwrap().is_none());
    }
    assert!(db.store(3).get_tc_token(keys[1]).await.unwrap().is_none());
}

#[tokio::test]
async fn singleton_and_bulk_secret_merges_match_and_survive_reopen() {
    let fixture = Fixture::new();
    let mut expected = Vec::new();
    {
        let db = fixture.open().await;
        let single = db.store(1);
        let bulk = db.store(2);
        assert_eq!(single.put_msg_secrets(Vec::new()).await.unwrap(), 0);
        for old_expiry in [0, 100, 200] {
            for new_expiry in [0, 100, 200] {
                for old_ts in [0, 100, 200] {
                    for new_ts in [0, 100, 200] {
                        let id = format!("{old_expiry:03}-{new_expiry:03}-{old_ts:03}-{new_ts:03}");
                        let first = secret(&id, old_expiry, old_ts, 1);
                        let second = secret(&id, new_expiry, new_ts, 2);
                        assert_eq!(
                            single.put_msg_secrets(vec![first.clone()]).await.unwrap(),
                            1
                        );
                        assert_eq!(
                            single.put_msg_secrets(vec![second.clone()]).await.unwrap(),
                            1
                        );
                        assert_eq!(bulk.put_msg_secrets(vec![first, second]).await.unwrap(), 2);
                        expected.push(SecretRow {
                            secret: vec![2; 32],
                            expires_at: if old_expiry == 0 || new_expiry == 0 {
                                0
                            } else {
                                old_expiry.max(new_expiry)
                            },
                            message_ts: old_ts.max(new_ts),
                        });
                    }
                }
            }
        }
        assert_eq!(rows(&db, 1).await, expected);
        assert_eq!(rows(&db, 2).await, expected);
        assert!(rows(&db, 3).await.is_empty());
    }
    let db = fixture.open().await;
    assert_eq!(rows(&db, 1).await, expected);
    assert_eq!(rows(&db, 2).await, expected);
}

#[tokio::test]
async fn prekey_batch_preserves_duplicate_order_upload_flag_and_device_scope() {
    let fixture = Fixture::new();
    {
        let db = fixture.open().await;
        for device in [1, 2] {
            let store = db.store(device);
            store.store_prekeys_batch(&[], false).await.unwrap();
            store
                .store_prekeys_batch(&[(1, Bytes::from_static(b"initial"))], false)
                .await
                .unwrap();
            let batch = [
                (1, Bytes::from_static(b"updated")),
                (2, Bytes::new()),
                (1, Bytes::from(vec![device as u8; 64])),
            ];
            store.store_prekeys_batch(&batch, true).await.unwrap();
            assert_eq!(uploaded(&db, device).await, 2);
            store
                .store_prekeys_batch(&[(2, Bytes::from_static(b"unuploaded"))], false)
                .await
                .unwrap();
            assert_eq!(uploaded(&db, device).await, 1);
        }
    }
    let db = fixture.open().await;
    for device in [1, 2] {
        let store = db.store(device);
        assert_eq!(
            store.load_prekey(1).await.unwrap(),
            Some(Bytes::from(vec![device as u8; 64]))
        );
        assert_eq!(
            store.load_prekey(2).await.unwrap(),
            Some(Bytes::from_static(b"unuploaded"))
        );
        assert_eq!(uploaded(&db, device).await, 1);
    }
    assert!(db.store(3).load_prekey(1).await.unwrap().is_none());
}

#[tokio::test]
async fn failed_secret_and_prekey_writes_roll_back_without_partial_changes() {
    let fixture = Fixture::new();
    let db = fixture.open().await;
    let store = db.store(1);
    store
        .put_msg_secrets(vec![secret("keep", 100, 100, 1)])
        .await
        .unwrap();
    store
        .store_prekeys_batch(&[(1, Bytes::from_static(b"keep"))], false)
        .await
        .unwrap();
    db.shared().run(|conn| {
        conn.batch_execute("CREATE TRIGGER reject_secret BEFORE INSERT ON msg_secrets WHEN NEW.msg_id = 'reject' BEGIN SELECT RAISE(ABORT, 'injected failure'); END; CREATE TRIGGER reject_prekey BEFORE INSERT ON prekeys WHEN NEW.id = 2 BEGIN SELECT RAISE(ABORT, 'injected failure'); END;")
            .map_err(|e| StoreError::Database(Box::new(e)))
    }).await.unwrap();
    assert!(
        store
            .put_msg_secrets(vec![secret("reject", 0, 0, 2)])
            .await
            .is_err()
    );
    assert!(
        store
            .put_msg_secrets(vec![secret("keep", 200, 200, 2), secret("reject", 0, 0, 2)])
            .await
            .is_err()
    );
    assert!(
        store
            .store_prekeys_batch(
                &[(1, Bytes::from_static(b"changed")), (2, Bytes::new())],
                true
            )
            .await
            .is_err()
    );
    assert_eq!(
        rows(&db, 1).await,
        vec![SecretRow {
            secret: vec![1; 32],
            expires_at: 100,
            message_ts: 100
        }]
    );
    assert_eq!(
        store.load_prekey(1).await.unwrap(),
        Some(Bytes::from_static(b"keep"))
    );
    assert!(store.load_prekey(2).await.unwrap().is_none());
    assert_eq!(uploaded(&db, 1).await, 0);
}

#[tokio::test]
async fn optimized_writes_report_post_commit_barrier_failures() {
    let fixture = Fixture::new();
    let fail = Arc::new(AtomicBool::new(false));
    let hook_fail = fail.clone();
    let hook: CommitBarrierHook = Arc::new(move || {
        let fail = hook_fail.load(Ordering::SeqCst);
        Box::pin(async move {
            if fail {
                Err(StoreError::Validation("injected barrier failure".into()))
            } else {
                Ok(())
            }
        })
    });
    let db = SqliteDatabase::open(
        fixture.url(),
        SqliteDatabaseConfig::default().with_commit_barrier(hook),
    )
    .await
    .unwrap();
    let store = db.store(1);
    fail.store(true, Ordering::SeqCst);
    let errors = [
        store
            .put_msg_secrets(vec![secret("one", 100, 100, 1)])
            .await
            .unwrap_err(),
        store
            .store_prekeys_batch(&[(1, Bytes::from_static(b"key"))], true)
            .await
            .unwrap_err(),
    ];
    for error in errors {
        let StoreError::Database(source) = error else {
            panic!("expected database error")
        };
        assert!(source.downcast_ref::<CommitBarrierError>().is_some());
    }
    assert_eq!(rows(&db, 1).await.len(), 1);
    assert_eq!(
        store.load_prekey(1).await.unwrap(),
        Some(Bytes::from_static(b"key"))
    );
    fail.store(false, Ordering::SeqCst);
    assert_eq!(
        store
            .put_msg_secrets(vec![secret("one", 100, 100, 1)])
            .await
            .unwrap(),
        1
    );
    store
        .store_prekeys_batch(&[(1, Bytes::from_static(b"key"))], true)
        .await
        .unwrap();
}

#[tokio::test]
async fn writes_wait_for_durability_and_cancelled_queued_operations_do_not_run() {
    for prekeys in [false, true] {
        let fixture = Fixture::new();
        let armed = Arc::new(AtomicBool::new(false));
        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let hook: CommitBarrierHook = Arc::new({
            let armed = armed.clone();
            let entered = entered.clone();
            let release = release.clone();
            move || {
                let wait = armed.load(Ordering::SeqCst);
                let entered = entered.clone();
                let release = release.clone();
                Box::pin(async move {
                    if wait {
                        entered.notify_one();
                        release.notified().await;
                    }
                    Ok(())
                })
            }
        });
        let db = SqliteDatabase::open(
            fixture.url(),
            SqliteDatabaseConfig::default().with_commit_barrier(hook),
        )
        .await
        .unwrap();
        armed.store(true, Ordering::SeqCst);
        let writer = db.store(1);
        let task = tokio::spawn(async move {
            if prekeys {
                writer
                    .store_prekeys_batch(&[(1, Bytes::from_static(b"committed"))], true)
                    .await
            } else {
                writer
                    .put_msg_secrets(vec![secret("committed", 100, 100, 1)])
                    .await
                    .map(|_| ())
            }
        });
        tokio::time::timeout(std::time::Duration::from_secs(10), entered.notified())
            .await
            .unwrap();
        assert!(
            !task.is_finished(),
            "SQL commit must still await the barrier"
        );
        let store = db.store(1);
        let mut queued = Box::pin(async {
            if prekeys {
                store
                    .store_prekeys_batch(&[(2, Bytes::from_static(b"cancelled"))], true)
                    .await
            } else {
                store
                    .put_msg_secrets(vec![secret("cancelled", 100, 100, 2)])
                    .await
                    .map(|_| ())
            }
        });
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(queued.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        drop(queued);
        let mut lookup = Box::pin(store.get_tc_token("absent"));
        assert!(
            std::future::poll_fn(|cx| std::task::Poll::Ready(lookup.as_mut().poll(cx)))
                .await
                .is_pending()
        );
        armed.store(false, Ordering::SeqCst);
        release.notify_one();
        tokio::time::timeout(std::time::Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(lookup.await.unwrap().is_none());
        if prekeys {
            assert_eq!(
                store.load_prekey(1).await.unwrap(),
                Some(Bytes::from_static(b"committed"))
            );
            assert!(store.load_prekey(2).await.unwrap().is_none());
        } else {
            assert_eq!(rows(&db, 1).await.len(), 1);
            assert!(
                store
                    .get_msg_secret("15550000001@s.whatsapp.net", "15550000002@lid", "cancelled")
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }
}
