// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Behaviour every [`RecordStore`] + [`LeaseStore`] adapter must show.
//!
//! Adapter crates enable the `conformance` feature in dev-dependencies and run
//! [`run_all`] against a real database or emulator. Each check uses its own
//! scope with a random suffix, so the suite can share a database with other
//! tests and with earlier runs.

use crate::record::{
    Fence, LeaseStore, Namespace, RecordStore, WriteOp, is_fence_lost, is_record_exists,
};
use rand::Rng;

const TTL: u64 = 10_000;

/// Run every check. Panics with the failing check's name.
pub async fn run_all<S: RecordStore + LeaseStore>(store: &S) {
    let run = format!("{:08x}", rand::rng().next_u32());
    leases_exclude_other_holders_until_expiry(store, &run).await;
    same_holder_keeps_its_generation(store, &run).await;
    renew_fails_after_takeover(store, &run).await;
    release_advances_the_next_generation(store, &run).await;
    writes_need_the_current_fence(store, &run).await;
    a_stale_batch_applies_nothing(store, &run).await;
    a_batch_applies_every_op(store, &run).await;
    update_never_inserts(store, &run).await;
    insert_is_exclusive_and_atomic(store, &run).await;
    inserts_see_earlier_ops_in_the_same_batch(store, &run).await;
    insert_then_delete_preserves_absence_and_batch_atomicity(store, &run).await;
    scan_orders_filters_and_limits(store, &run).await;
    scopes_and_namespaces_are_isolated(store, &run).await;
    get_many_returns_only_existing_keys(store, &run).await;
}

fn scope(run: &str, check: &str) -> String {
    format!("conformance-{run}-{check}")
}

pub async fn insert_then_delete_preserves_absence_and_batch_atomicity<
    S: RecordStore + LeaseStore,
>(
    store: &S,
    run: &str,
) {
    let scope = scope(run, "insert-then-delete");
    let fence = take(store, &scope, "holder", 1_000).await;
    let ns = Namespace::PreKey;
    let conditional_delete = |key: &str| {
        vec![
            WriteOp::Insert {
                ns,
                key: key.into(),
                value: b"sealed-placeholder".to_vec(),
            },
            WriteOp::Delete {
                ns,
                key: key.into(),
            },
            put(Namespace::Device, "other", b"whole-batch"),
        ]
    };
    store
        .write(&fence, &conditional_delete("absent"))
        .await
        .unwrap();
    assert_eq!(store.get(&scope, ns, "absent").await.unwrap(), None);
    assert_eq!(
        store.get(&scope, Namespace::Device, "other").await.unwrap(),
        Some(b"whole-batch".to_vec())
    );
    store
        .write(
            &fence,
            &[
                put(ns, "present", b"independent"),
                put(Namespace::Device, "other", b"original"),
            ],
        )
        .await
        .unwrap();
    let error = store
        .write(&fence, &conditional_delete("present"))
        .await
        .unwrap_err();
    assert!(is_record_exists(&error));
    assert_eq!(
        store.get(&scope, ns, "present").await.unwrap(),
        Some(b"independent".to_vec())
    );
    assert_eq!(
        store.get(&scope, Namespace::Device, "other").await.unwrap(),
        Some(b"original".to_vec())
    );
}

fn put(ns: Namespace, key: &str, value: &[u8]) -> WriteOp {
    WriteOp::Put {
        ns,
        key: key.to_owned(),
        value: value.to_vec(),
    }
}

async fn take<S: LeaseStore>(store: &S, scope: &str, holder: &str, now: u64) -> Fence {
    store
        .acquire(scope, holder, now, TTL)
        .await
        .expect("acquire")
        .unwrap_or_else(|| panic!("{holder} could not acquire {scope}"))
        .fence
}

pub async fn leases_exclude_other_holders_until_expiry<S: LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "exclusive");
    let first = take(store, &scope, "a", 1_000).await;
    assert!(
        store
            .acquire(&scope, "b", 1_000 + TTL - 1, TTL)
            .await
            .unwrap()
            .is_none(),
        "leases_exclude_other_holders_until_expiry: b took a live lease"
    );
    let second = take(store, &scope, "b", 1_000 + TTL).await;
    assert!(
        second.generation > first.generation,
        "leases_exclude_other_holders_until_expiry: generation did not advance"
    );
}

pub async fn same_holder_keeps_its_generation<S: LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "same-holder");
    let first = take(store, &scope, "a", 1_000).await;
    let again = take(store, &scope, "a", 2_000).await;
    assert_eq!(
        first, again,
        "same_holder_keeps_its_generation: re-acquire changed the fence"
    );
}

pub async fn renew_fails_after_takeover<S: LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "renew");
    let lease = store
        .acquire(&scope, "a", 1_000, TTL)
        .await
        .unwrap()
        .unwrap();
    let renewed = store.renew(&lease, 2_000, TTL).await.unwrap();
    assert!(
        renewed.is_some(),
        "renew_fails_after_takeover: live renew failed"
    );
    take(store, &scope, "b", 2_000 + TTL).await;
    assert!(
        store
            .renew(&lease, 2_000 + TTL, TTL)
            .await
            .unwrap()
            .is_none(),
        "renew_fails_after_takeover: lost lease renewed"
    );
}

pub async fn release_advances_the_next_generation<S: RecordStore + LeaseStore>(
    store: &S,
    run: &str,
) {
    let scope = scope(run, "release");
    let lease = store
        .acquire(&scope, "a", 1_000, TTL)
        .await
        .unwrap()
        .unwrap();
    store.release(&lease).await.unwrap();
    let next = take(store, &scope, "b", 1_001).await;
    assert!(
        next.generation > lease.fence.generation,
        "release_advances_the_next_generation: b reused a's generation"
    );
    let stale = store
        .write(&lease.fence, &[put(Namespace::Session, "k", b"v")])
        .await;
    assert!(
        stale.as_ref().is_err_and(is_fence_lost),
        "release_advances_the_next_generation: released fence still wrote"
    );
}

pub async fn writes_need_the_current_fence<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "fence");
    let never_leased = Fence {
        scope: scope.as_str().into(),
        generation: 1,
    };
    let refused = store
        .write(&never_leased, &[put(Namespace::Session, "k", b"v")])
        .await;
    assert!(
        refused.as_ref().is_err_and(is_fence_lost),
        "writes_need_the_current_fence: wrote without any lease"
    );
    let a = take(store, &scope, "a", 1_000).await;
    store
        .write(&a, &[put(Namespace::Session, "k", b"from-a")])
        .await
        .expect("writes_need_the_current_fence: holder write failed");
    let b = take(store, &scope, "b", 1_000 + TTL).await;
    let late = store
        .write(&a, &[put(Namespace::Session, "k", b"late-a")])
        .await;
    assert!(
        late.as_ref().is_err_and(is_fence_lost),
        "writes_need_the_current_fence: superseded holder wrote"
    );
    store
        .write(&b, &[put(Namespace::Session, "k", b"from-b")])
        .await
        .unwrap();
    assert_eq!(
        store
            .get(&scope, Namespace::Session, "k")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"from-b"[..]),
        "writes_need_the_current_fence: wrong final value"
    );
}

pub async fn a_stale_batch_applies_nothing<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "stale-batch");
    let a = take(store, &scope, "a", 1_000).await;
    store
        .write(&a, &[put(Namespace::Session, "keep", b"original")])
        .await
        .unwrap();
    take(store, &scope, "b", 1_000 + TTL).await;
    let stale = store
        .write(
            &a,
            &[
                put(Namespace::Session, "new", b"x"),
                put(Namespace::Session, "keep", b"overwritten"),
                WriteOp::Delete {
                    ns: Namespace::Session,
                    key: "keep".into(),
                },
            ],
        )
        .await;
    assert!(
        stale.as_ref().is_err_and(is_fence_lost),
        "a_stale_batch_applies_nothing: stale batch was not refused as fence lost"
    );
    assert_eq!(
        store
            .get(&scope, Namespace::Session, "keep")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"original"[..]),
        "a_stale_batch_applies_nothing: stale batch changed a record"
    );
    assert!(
        store
            .get(&scope, Namespace::Session, "new")
            .await
            .unwrap()
            .is_none(),
        "a_stale_batch_applies_nothing: stale batch inserted a record"
    );
}

pub async fn a_batch_applies_every_op<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "batch");
    let fence = take(store, &scope, "a", 1_000).await;
    store
        .write(
            &fence,
            &[
                put(Namespace::PreKey, "1", b"one"),
                put(Namespace::PreKey, "2", b"two"),
            ],
        )
        .await
        .unwrap();
    store
        .write(
            &fence,
            &[
                WriteOp::Delete {
                    ns: Namespace::PreKey,
                    key: "1".into(),
                },
                WriteOp::Update {
                    ns: Namespace::PreKey,
                    key: "2".into(),
                    value: b"two-updated".to_vec(),
                },
                put(Namespace::PreKey, "3", b"three"),
            ],
        )
        .await
        .unwrap();
    assert!(
        store
            .get(&scope, Namespace::PreKey, "1")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        store
            .get(&scope, Namespace::PreKey, "2")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"two-updated"[..]),
        "a_batch_applies_every_op: update missing"
    );
    assert_eq!(
        store
            .get(&scope, Namespace::PreKey, "3")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"three"[..]),
        "a_batch_applies_every_op: put missing"
    );
}

pub async fn update_never_inserts<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "update");
    let fence = take(store, &scope, "a", 1_000).await;
    store
        .write(
            &fence,
            &[WriteOp::Update {
                ns: Namespace::PreKey,
                key: "gone".into(),
                value: b"x".to_vec(),
            }],
        )
        .await
        .unwrap();
    assert!(
        store
            .get(&scope, Namespace::PreKey, "gone")
            .await
            .unwrap()
            .is_none(),
        "update_never_inserts: update resurrected a missing key"
    );
}

pub async fn scan_orders_filters_and_limits<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "scan");
    let fence = take(store, &scope, "a", 1_000).await;
    let keys = ["user:2@s", "user@s", "user:1@s", "other@s", "usera@s"];
    let ops: Vec<_> = keys
        .iter()
        .map(|k| put(Namespace::Session, k, b"v"))
        .collect();
    store.write(&fence, &ops).await.unwrap();
    assert_eq!(
        store
            .scan_keys(&scope, Namespace::Session, "user:", None)
            .await
            .unwrap(),
        vec!["user:1@s".to_owned(), "user:2@s".to_owned()],
        "scan_orders_filters_and_limits: prefix scan"
    );
    assert_eq!(
        store
            .scan_keys(&scope, Namespace::Session, "", Some(2))
            .await
            .unwrap(),
        vec!["other@s".to_owned(), "user:1@s".to_owned()],
        "scan_orders_filters_and_limits: limited full scan"
    );
}

pub async fn scopes_and_namespaces_are_isolated<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let first = scope(run, "iso-1");
    let second = scope(run, "iso-1x");
    let a = take(store, &first, "a", 1_000).await;
    let b = take(store, &second, "a", 1_000).await;
    store
        .write(&a, &[put(Namespace::Session, "k", b"first-session")])
        .await
        .unwrap();
    store
        .write(&b, &[put(Namespace::Session, "k", b"second-session")])
        .await
        .unwrap();
    assert!(
        store
            .get(&first, Namespace::Identity, "k")
            .await
            .unwrap()
            .is_none(),
        "scopes_and_namespaces_are_isolated: namespace leaked"
    );
    assert_eq!(
        store
            .get(&first, Namespace::Session, "k")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"first-session"[..]),
        "scopes_and_namespaces_are_isolated: scope leaked"
    );
    assert_eq!(
        store
            .scan_keys(&first, Namespace::Session, "", None)
            .await
            .unwrap(),
        vec!["k".to_owned()],
        "scopes_and_namespaces_are_isolated: scan crossed into a sibling scope"
    );
}

pub async fn get_many_returns_only_existing_keys<S: RecordStore + LeaseStore>(
    store: &S,
    run: &str,
) {
    let scope = scope(run, "get-many");
    let fence = take(store, &scope, "a", 1_000).await;
    store
        .write(
            &fence,
            &[
                put(Namespace::SenderKey, "x", b"1"),
                put(Namespace::SenderKey, "y", b"2"),
            ],
        )
        .await
        .unwrap();
    let mut found = store
        .get_many(&scope, Namespace::SenderKey, &["x", "missing", "y"])
        .await
        .unwrap();
    found.sort();
    assert_eq!(
        found,
        vec![
            ("x".to_owned(), b"1".to_vec()),
            ("y".to_owned(), b"2".to_vec())
        ],
        "get_many_returns_only_existing_keys"
    );
}

pub async fn insert_is_exclusive_and_atomic<S: RecordStore + LeaseStore>(store: &S, run: &str) {
    let scope = scope(run, "insert");
    let fence = take(store, &scope, "a", 1_000).await;
    let insert = |key: &str, value: &[u8]| WriteOp::Insert {
        ns: Namespace::Device,
        key: key.to_owned(),
        value: value.to_vec(),
    };
    store.write(&fence, &[insert("k", b"first")]).await.unwrap();
    let refused = store
        .write(
            &fence,
            &[
                put(Namespace::Session, "side-effect", b"x"),
                insert("k", b"second"),
            ],
        )
        .await;
    assert!(
        refused.as_ref().is_err_and(is_record_exists),
        "insert_is_exclusive_and_atomic: second insert was not refused"
    );
    assert_eq!(
        store
            .get(&scope, Namespace::Device, "k")
            .await
            .unwrap()
            .as_deref(),
        Some(&b"first"[..]),
        "insert_is_exclusive_and_atomic: existing record changed"
    );
    assert!(
        store
            .get(&scope, Namespace::Session, "side-effect")
            .await
            .unwrap()
            .is_none(),
        "insert_is_exclusive_and_atomic: refused batch applied another op"
    );
}

pub async fn inserts_see_earlier_ops_in_the_same_batch<S: RecordStore + LeaseStore>(
    store: &S,
    run: &str,
) {
    let scope = scope(run, "insert-in-batch");
    let fence = take(store, &scope, "a", 1_000).await;
    let ns = Namespace::Device;
    let insert = |key: &str, value: &[u8]| WriteOp::Insert {
        ns,
        key: key.to_owned(),
        value: value.to_vec(),
    };
    let refused_cases: [(&str, Vec<WriteOp>); 2] = [
        ("dup", vec![insert("dup", b"a"), insert("dup", b"b")]),
        ("put", vec![put(ns, "put", b"a"), insert("put", b"b")]),
    ];
    for (key, mut ops) in refused_cases {
        ops.push(put(Namespace::Session, "side-effect", b"x"));
        let refused = store.write(&fence, &ops).await;
        assert!(
            refused.as_ref().is_err_and(is_record_exists),
            "inserts_see_earlier_ops_in_the_same_batch: {key} batch was not refused"
        );
        assert!(
            store.get(&scope, ns, key).await.unwrap().is_none(),
            "inserts_see_earlier_ops_in_the_same_batch: {key} refused batch wrote a record"
        );
        assert!(
            store
                .get(&scope, Namespace::Session, "side-effect")
                .await
                .unwrap()
                .is_none(),
            "inserts_see_earlier_ops_in_the_same_batch: {key} refused batch applied another op"
        );
    }
    store
        .write(
            &fence,
            &[
                insert("again", b"a"),
                WriteOp::Delete {
                    ns,
                    key: "again".into(),
                },
                insert("again", b"b"),
            ],
        )
        .await
        .expect("inserts_see_earlier_ops_in_the_same_batch: insert after delete refused");
    assert_eq!(
        store.get(&scope, ns, "again").await.unwrap().as_deref(),
        Some(&b"b"[..]),
        "inserts_see_earlier_ops_in_the_same_batch: insert after delete lost"
    );
}
