//! Missed primary rotations must recover ordinary lists without trusting their envelope key.

use super::tests::{adv_wire_response, has_session, seed_fresh_session, signed_key_index_bytes};
use super::*;
use crate::cache::Freshness;
use crate::test_utils::{TestEventCollector, answer_iq, create_iq_test_client, decode_sent_iq};
use std::sync::Arc;
use std::time::Duration;
use wacore::libsignal::protocol::KeyPair;
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore::types::events::{Event, EventInterest, EventKind};
use wacore::types::jid::JidExt as _;
use wacore_binary::builder::NodeBuilder;

type DeleteHook =
    Box<dyn FnOnce() -> wacore::runtime::BoxFuture<'static, anyhow::Result<()>> + Send>;

struct OwnershipCallbackCache {
    rows: std::sync::Mutex<std::collections::HashMap<(String, String), Vec<u8>>>,
    manager:
        std::sync::Mutex<std::sync::Weak<crate::store::persistence_manager::PersistenceManager>>,
    enabled: std::sync::atomic::AtomicBool,
    delete_enabled: std::sync::atomic::AtomicBool,
    read_error: std::sync::atomic::AtomicBool,
    delete_hook: std::sync::Mutex<Option<DeleteHook>>,
    set_hook: std::sync::Mutex<Option<DeleteHook>>,
    retain_deleted: std::sync::atomic::AtomicBool,
    taken: std::sync::atomic::AtomicBool,
    sets: std::sync::atomic::AtomicUsize,
    cancelled: std::sync::atomic::AtomicBool,
    entered: async_channel::Sender<(tokio::task::AbortHandle, i64)>,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl wacore::store::CacheStore for OwnershipCallbackCache {
    async fn get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
        anyhow::ensure!(
            !self.read_error.load(std::sync::atomic::Ordering::Acquire),
            "synthetic cache read failure"
        );
        Ok(self
            .rows
            .lock()
            .unwrap()
            .get(&(namespace.into(), key.into()))
            .cloned())
    }

    async fn set(
        &self,
        namespace: &str,
        key: &str,
        value: &[u8],
        _: Option<Duration>,
    ) -> anyhow::Result<()> {
        use std::sync::atomic::Ordering;
        self.sets.fetch_add(1, Ordering::Relaxed);
        let hook = self.set_hook.lock().unwrap().take();
        if let Some(hook) = hook {
            hook().await?;
        }
        if self.enabled.load(Ordering::Acquire) && !self.taken.swap(true, Ordering::AcqRel) {
            let manager = self.manager.lock().unwrap().upgrade().unwrap();
            let record: DeviceListRecord = serde_json::from_slice(value).unwrap();
            let callback = tokio::spawn(async move {
                manager
                    .modify_device(|device| device.push_name = "registry callback".into())
                    .await;
            });
            self.entered
                .send((callback.abort_handle(), record.timestamp))
                .await
                .unwrap();
            if callback.await.is_err() {
                self.cancelled.store(true, Ordering::Release);
            }
        }
        self.rows
            .lock()
            .unwrap()
            .insert((namespace.into(), key.into()), value.to_vec());
        Ok(())
    }

    async fn delete(&self, namespace: &str, key: &str) -> anyhow::Result<()> {
        use std::sync::atomic::Ordering;
        if self.delete_enabled.load(Ordering::Acquire) && !self.taken.swap(true, Ordering::AcqRel) {
            let manager = self.manager.lock().unwrap().upgrade().unwrap();
            let callback = tokio::spawn(async move {
                manager
                    .modify_device(|device| device.push_name = "delete callback".into())
                    .await;
            });
            self.entered
                .send((callback.abort_handle(), 0))
                .await
                .unwrap();
            if callback.await.is_err() {
                self.cancelled.store(true, Ordering::Release);
            }
        }
        if !self.retain_deleted.load(Ordering::Acquire) {
            self.rows
                .lock()
                .unwrap()
                .remove(&(namespace.into(), key.into()));
        }
        let hook = self.delete_hook.lock().unwrap().take();
        if let Some(hook) = hook {
            hook().await?;
        }
        Ok(())
    }

    async fn clear(&self, namespace: &str) -> anyhow::Result<()> {
        self.rows
            .lock()
            .unwrap()
            .retain(|(stored, _), _| stored != namespace);
        Ok(())
    }
}

async fn check_hosted_cache_callback_ownership(cold_fill: bool, callback_enabled: bool) {
    use crate::cache_config::{CacheConfig, CacheStores};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let (entered, arrived) = async_channel::unbounded();
    let store = Arc::new(OwnershipCallbackCache {
        rows: std::sync::Mutex::new(Default::default()),
        manager: std::sync::Mutex::new(Default::default()),
        enabled: AtomicBool::new(false),
        delete_enabled: AtomicBool::new(false),
        read_error: AtomicBool::new(false),
        delete_hook: std::sync::Mutex::new(None),
        set_hook: std::sync::Mutex::new(None),
        retain_deleted: AtomicBool::new(false),
        taken: AtomicBool::new(false),
        sets: AtomicUsize::new(0),
        cancelled: AtomicBool::new(false),
        entered,
    });
    let backend = crate::test_utils::create_test_backend().await;
    let user = Jid::pn(if cold_fill {
        "12025550176"
    } else {
        "12025550175"
    });
    if cold_fill {
        backend
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [
                    DeviceInfo::new(0, None),
                    DeviceInfo::new(2, Some(1)).with_hosting(true),
                ]
                .into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
    }
    let config = CacheConfig::default().with_cache_stores(CacheStores {
        device_registry_cache: Some(store.clone()),
        ..Default::default()
    });
    let (client, transport) = crate::test_utils::create_iq_test_client_with_runtime_and_config(
        backend.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
        config,
    )
    .await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.lid_pn_cache.startup_finished_for_tests() {
            tokio::task::yield_now().await;
        }
        drop(client.lid_pn_cache.lock_mutation().await);
    })
    .await
    .unwrap();
    crate::test_utils::seed_adv_primary(&client, &user).await;
    client.flush_signal_cache_batch_safe().await.unwrap();
    let key = client.cached_adv_account_identity(&user).await.unwrap();
    *store.manager.lock().unwrap() = Arc::downgrade(&client.persistence_manager);
    store.rows.lock().unwrap().clear();
    store.sets.store(0, Ordering::Relaxed);
    store.enabled.store(callback_enabled, Ordering::Release);
    let mut lookup = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move { client.refresh_user_devices(vec![user]).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    let signature = hosted_epoch_response(&user).device_lists[0]
        .key_index_bytes
        .clone()
        .unwrap();
    answer_iq(
        &client,
        &id,
        &tests::adv_wire_response_with_hosting(&user, signature, 2, true),
    )
    .await;
    let abort = if callback_enabled {
        let (abort, timestamp) = tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        assert_ne!(
            timestamp, 1,
            "durable reads must not invoke host set callbacks"
        );
        Some(abort)
    } else {
        None
    };
    let result = tokio::time::timeout(Duration::from_secs(5), &mut lookup).await;
    if result.is_err() {
        if let Some(abort) = abort {
            abort.abort();
        }
        let drained = tokio::time::timeout(Duration::from_secs(5), &mut lookup)
            .await
            .is_ok();
        client.signal_shutdown_sync();
        panic!(
            "hosted cache callback waited on its publisher's ownership; cold_fill={cold_fill}, cleanup={drained}"
        );
    }
    let devices = result.unwrap().unwrap().unwrap();
    assert!(devices.contains(&user));
    assert!(
        devices
            .iter()
            .any(|device| device.user == user.user && device.device == 2 && device.is_hosted())
    );
    assert_eq!(
        client.cached_adv_account_identity(&user).await.unwrap(),
        key
    );
    assert!(!store.cancelled.load(Ordering::Acquire));
    assert_eq!(store.sets.load(Ordering::Relaxed), 1);
    if callback_enabled {
        assert_eq!(
            client.persistence_manager.get_device_snapshot().push_name,
            "registry callback"
        );
    }
    let record = backend.get_devices(&user.user).await.unwrap().unwrap();
    assert_eq!(
        record
            .devices
            .iter()
            .map(|device| device.device_id())
            .collect::<Vec<_>>(),
        [0, 2]
    );
    client.signal_shutdown_sync();
}

/// Publication must release device ownership before the host cache acknowledges its write.
#[tokio::test]
async fn adv_review_round8_hosted_publication_cache_callback_can_modify_device() {
    check_hosted_cache_callback_ownership(false, false).await;
    check_hosted_cache_callback_ownership(false, true).await;
}

/// A cold durable row must not move a host callback back inside device ownership.
#[tokio::test]
async fn adv_review_round8_hosted_cold_fill_cache_callback_can_modify_device() {
    check_hosted_cache_callback_ownership(true, false).await;
    check_hosted_cache_callback_ownership(true, true).await;
}

/// A read miss must not start a host set callback that recursively reads another
/// uncached user while the outer read still owns Registry.
#[tokio::test]
async fn adv_registry_cold_read_does_not_invoke_a_reentrant_host_set() {
    use crate::cache_config::{CacheConfig, CacheStores};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let (entered, _) = async_channel::unbounded();
    let store = Arc::new(OwnershipCallbackCache {
        rows: std::sync::Mutex::new(Default::default()),
        manager: std::sync::Mutex::new(Default::default()),
        enabled: AtomicBool::new(false),
        delete_enabled: AtomicBool::new(false),
        read_error: AtomicBool::new(false),
        delete_hook: std::sync::Mutex::new(None),
        set_hook: std::sync::Mutex::new(None),
        retain_deleted: AtomicBool::new(false),
        taken: AtomicBool::new(false),
        sets: AtomicUsize::new(0),
        cancelled: AtomicBool::new(false),
        entered,
    });
    let config = CacheConfig::default().with_cache_stores(CacheStores {
        device_registry_cache: Some(store.clone()),
        ..Default::default()
    });
    let client = crate::test_utils::create_test_client_with_config(
        "registry_reentrant_set",
        Arc::new(crate::test_utils::MockHttpClient),
        config,
    )
    .await;
    let outer = Jid::pn("12025550183");
    let nested = Jid::pn("12025550184");
    for user in [&outer, &nested] {
        client
            .persistence_manager
            .backend()
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
    }
    let weak = Arc::downgrade(&client);
    *store.set_hook.lock().unwrap() = Some(Box::new(move || {
        Box::pin(async move {
            let client = weak.upgrade().unwrap();
            let devices = client
                .signal()
                .get_user_devices(std::slice::from_ref(&nested))
                .await?;
            assert!(devices.contains(&nested));
            Ok(())
        })
    }));
    let record = tokio::time::timeout(
        Duration::from_secs(5),
        client.load_device_record_for_jid(&outer),
    )
    .await
    .expect("a cold read must not deadlock inside the cache host")
    .unwrap();
    assert_eq!(record.raw_id, Some(1));
    assert_eq!(store.sets.load(Ordering::Acquire), 0);
    assert!(store.set_hook.lock().unwrap().is_some());
    client.signal_shutdown_sync();
}

/// Exercise the public IQ path that previously deadlocked delete against modify_device.
#[tokio::test]
async fn adv_review_round9_hosted_rotation_cache_delete_can_modify_device() {
    use crate::cache_config::{CacheConfig, CacheStores};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    for callback_enabled in [false, true] {
        let (entered, arrived) = async_channel::unbounded();
        let store = Arc::new(OwnershipCallbackCache {
            rows: std::sync::Mutex::new(Default::default()),
            manager: std::sync::Mutex::new(Default::default()),
            enabled: AtomicBool::new(false),
            delete_enabled: AtomicBool::new(callback_enabled),
            read_error: AtomicBool::new(false),
            delete_hook: std::sync::Mutex::new(None),
            set_hook: std::sync::Mutex::new(None),
            retain_deleted: AtomicBool::new(false),
            taken: AtomicBool::new(false),
            sets: AtomicUsize::new(0),
            cancelled: AtomicBool::new(false),
            entered,
        });
        let config = CacheConfig::default().with_cache_stores(CacheStores {
            device_registry_cache: Some(store.clone()),
            ..Default::default()
        });
        let backend = crate::test_utils::create_test_backend().await;
        let (client, transport) = crate::test_utils::create_iq_test_client_with_runtime_and_config(
            backend.clone(),
            Arc::new(crate::runtime_impl::TokioRuntime),
            config,
        )
        .await;
        let user = Jid::pn("12025550174");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        client.flush_signal_cache_batch_safe().await.unwrap();
        *store.manager.lock().unwrap() = Arc::downgrade(&client.persistence_manager);
        let next_pair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let next = *next_pair.public_key.public_key_bytes();
        let mut lookup = {
            let client = client.clone();
            let user = user.clone();
            tokio::spawn(async move {
                client
                    .signal()
                    .get_user_devices(std::slice::from_ref(&user))
                    .await
            })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(next.to_vec());
        signed.account_signature = Some(
            next_pair
                .private_key
                .calculate_signature(
                    &[&[6, 2], signed.details.as_ref().unwrap().as_slice()].concat(),
                    &mut rand::make_rng::<rand::rngs::StdRng>(),
                )
                .unwrap()
                .to_vec(),
        );
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(
            &client,
            &id,
            &tests::adv_wire_response_with_hosting(
                &user,
                waproto::codec::adv_signed_key_index_list_to_vec(&signed),
                2,
                true,
            ),
        )
        .await;
        let abort = if callback_enabled {
            Some(
                tokio::time::timeout(Duration::from_secs(5), arrived.recv())
                    .await
                    .unwrap()
                    .unwrap()
                    .0,
            )
        } else {
            None
        };
        let result = tokio::time::timeout(Duration::from_secs(5), &mut lookup).await;
        if result.is_err() {
            if let Some(abort) = abort {
                abort.abort();
            }
            let drained = tokio::time::timeout(Duration::from_secs(5), &mut lookup)
                .await
                .is_ok();
            client.signal_shutdown_sync();
            panic!("cache delete callback deadlocked hosted rotation; cleanup={drained}");
        }
        let devices = result.unwrap().unwrap().unwrap();
        assert!(devices.contains(&user));
        assert!(
            devices
                .iter()
                .any(|device| device.device == 2 && device.is_hosted())
        );
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            Some(next)
        );
        assert_eq!(
            backend
                .load_identity(user.to_protocol_address().as_str())
                .await
                .unwrap(),
            Some(next)
        );
        assert!(!store.cancelled.load(Ordering::Acquire));
        if callback_enabled {
            assert_eq!(
                client.persistence_manager.get_device_snapshot().push_name,
                "delete callback"
            );
        }
        client.signal_shutdown_sync();
    }
}

#[derive(Clone, Copy, Debug)]
enum EvictionCase {
    MappingFromModifier,
    InstalledCompanion,
    InstalledCompanionIdentityOnly,
    RefilledCache,
    RefilledOnce,
    RepeatedRefill,
    MalformedCache,
    RetiredPrimary,
    ConnectionChanged,
    DeleteError,
    ReadError,
    CancelledCaller,
    CacheAheadOfDatabase,
    StrictAliasChanged,
}

async fn check_hosted_rotation_eviction(case: EvictionCase) {
    use crate::cache_config::{CacheConfig, CacheStores};
    use std::sync::atomic::{AtomicBool, AtomicUsize};
    let (entered, _) = async_channel::unbounded();
    let store = Arc::new(OwnershipCallbackCache {
        rows: std::sync::Mutex::new(Default::default()),
        manager: std::sync::Mutex::new(Default::default()),
        enabled: AtomicBool::new(false),
        delete_enabled: AtomicBool::new(false),
        read_error: AtomicBool::new(false),
        delete_hook: std::sync::Mutex::new(None),
        set_hook: std::sync::Mutex::new(None),
        retain_deleted: AtomicBool::new(false),
        taken: AtomicBool::new(false),
        sets: AtomicUsize::new(0),
        cancelled: AtomicBool::new(false),
        entered,
    });
    let backend = crate::test_utils::create_test_backend().await;
    let config = CacheConfig::default().with_cache_stores(CacheStores {
        device_registry_cache: Some(store.clone()),
        ..Default::default()
    });
    let (client, transport) = crate::test_utils::create_iq_test_client_with_runtime_and_config(
        backend.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
        config,
    )
    .await;
    crate::test_utils::poll_until("mapping startup", || {
        client.lid_pn_cache.startup_finished_for_tests()
    })
    .await;
    let user = Jid::pn("12025550173");
    crate::test_utils::seed_adv_primary(&client, &user).await;
    let previous = client
        .cached_adv_account_identity(&user)
        .await
        .unwrap()
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    let mut lookup = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move {
            if matches!(case, EvictionCase::StrictAliasChanged) {
                return client.refresh_user_devices(vec![user]).await;
            }
            client
                .signal()
                .get_user_devices(std::slice::from_ref(&user))
                .await
                .map_err(anyhow::Error::from)
        })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let old = DeviceListRecord {
        user: user.user.as_str().into(),
        devices: [
            DeviceInfo::new(0, None),
            DeviceInfo::new(2, Some(1)).with_hosting(true),
        ]
        .into(),
        raw_id: Some(1),
        timestamp: 1,
        phash: None,
    };
    backend.update_device_list(old.clone()).await.unwrap();
    let companion = seed_fresh_session(&client, &user.with_device(7)).await;
    client.flush_signal_cache_batch_safe().await.unwrap();
    if matches!(case, EvictionCase::CacheAheadOfDatabase) {
        let mut ahead = old.clone();
        ahead.edit_devices(|devices| devices.push(DeviceInfo::new(7, Some(2))));
        client
            .device_registry_cache
            .raw_insert_for_tests(ahead.user.clone(), Arc::new(ahead))
            .await;
    }
    let (blocked, arrived) = async_channel::bounded(1);
    let (release, gate) = async_channel::bounded(1);
    let weak = Arc::downgrade(&client);
    let hook_store = Arc::downgrade(&store);
    let hook_user = user.clone();
    *store.delete_hook.lock().unwrap() = Some(Box::new(move || {
        Box::pin(async move {
            let client = weak.upgrade().unwrap();
            match case {
                EvictionCase::MappingFromModifier => {
                    let manager = client.persistence_manager.clone();
                    manager
                        .modify_device_async(move |device| {
                            Box::pin(async move {
                                tokio::spawn(async move {
                                    let address = hook_user.to_protocol_address();
                                    let lock = client.session_lock_for(address.as_str()).await;
                                    let _session = lock.lock().await;
                                    let _mapping = client.lid_pn_cache.lock_mutation().await;
                                    let _registry = client.device_topology.lock_registry().await;
                                })
                                .await
                                .unwrap();
                                device.push_name = "mapping callback".into();
                            })
                        })
                        .await;
                }
                EvictionCase::InstalledCompanion | EvictionCase::InstalledCompanionIdentityOnly => {
                    let topology = client.device_topology.current();
                    let epoch = client.signal_cache.identity_continuity.primary_snapshot();
                    let companion = hook_user.with_device(9);
                    let key = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                    let bundle =
                        session_bundle(&key, 9, Some(crate::test_utils::adv_test_account()));
                    client
                        .install_prekey_bundle_cached(
                            &companion,
                            &bundle,
                            &mut client.signal_adapter(),
                            &mut rand::make_rng::<rand::rngs::StdRng>(),
                        )
                        .await
                        .unwrap();
                    let address = companion.to_protocol_address();
                    assert!(has_session(&client, &address).await);
                    if matches!(case, EvictionCase::InstalledCompanionIdentityOnly) {
                        client.signal_cache.delete_session(&address).await;
                    }
                    client.flush_signal_cache_batch_safe().await.unwrap();
                    assert!(
                        client
                            .persistence_manager
                            .backend_ref()
                            .load_identity(address.as_str())
                            .await
                            .unwrap()
                            .is_some()
                    );
                    assert_eq!(client.device_topology.current(), topology);
                    assert!(
                        client
                            .signal_cache
                            .identity_continuity
                            .unchanged_primary_for(epoch, [hook_user.user.as_str()])
                    );
                }
                EvictionCase::RefilledCache => {
                    assert!(client.load_device_record(&hook_user.user).await.is_some());
                    assert!(
                        client
                            .device_registry_cache
                            .get(&hook_user.user)
                            .await
                            .is_none()
                    );
                }
                EvictionCase::RefilledOnce | EvictionCase::RepeatedRefill => {
                    let record = client
                        .persistence_manager
                        .backend()
                        .get_devices(&hook_user.user)
                        .await
                        .unwrap()
                        .unwrap();
                    client
                        .device_registry_cache
                        .raw_insert_for_tests(record.user.clone(), Arc::new(record))
                        .await;
                    if matches!(case, EvictionCase::RepeatedRefill) {
                        hook_store
                            .upgrade()
                            .unwrap()
                            .retain_deleted
                            .store(true, std::sync::atomic::Ordering::Release);
                    }
                }
                EvictionCase::MalformedCache => {
                    hook_store.upgrade().unwrap().rows.lock().unwrap().insert(
                        ("device_registry".into(), hook_user.user.to_string()),
                        vec![0xff],
                    );
                }
                EvictionCase::RetiredPrimary => {
                    deliver_identity_reset(&client, &hook_user, None).await
                }
                EvictionCase::StrictAliasChanged => {
                    let guard = client.lid_pn_cache.lock_mutation().await;
                    client
                        .learn_lid_pn_mappings_batch_guarded(
                            vec![("100000000000173".into(), hook_user.user.to_string())],
                            crate::lid_pn_cache::LearningSource::Usync,
                            true,
                            &guard,
                        )
                        .await?;
                }
                EvictionCase::ConnectionChanged => {
                    use std::sync::atomic::Ordering;
                    client.set_generation_for_test(
                        client.connection_generation.load(Ordering::Acquire) + 1,
                        Ordering::Release,
                    );
                }
                EvictionCase::DeleteError => anyhow::bail!("synthetic cache delete failure"),
                EvictionCase::ReadError => {
                    hook_store
                        .upgrade()
                        .unwrap()
                        .read_error
                        .store(true, std::sync::atomic::Ordering::Release);
                }
                EvictionCase::CancelledCaller => {
                    blocked.send(()).await.unwrap();
                    gate.recv().await.unwrap();
                }
                EvictionCase::CacheAheadOfDatabase => {}
            }
            Ok(())
        })
    }));
    let next_pair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let next = *next_pair.public_key.public_key_bytes();
    let mut signed =
        waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
            .unwrap();
    signed.account_signature_key = Some(next.to_vec());
    signed.account_signature = Some(
        next_pair
            .private_key
            .calculate_signature(
                &[&[6, 2], signed.details.as_ref().unwrap().as_slice()].concat(),
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )
            .unwrap()
            .to_vec(),
    );
    let generation = client.device_topology.current();
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(
        &client,
        &id,
        &tests::adv_wire_response_with_hosting(
            &user,
            waproto::codec::adv_signed_key_index_list_to_vec(&signed),
            2,
            true,
        ),
    )
    .await;
    if matches!(case, EvictionCase::StrictAliasChanged) {
        let retry = tokio::time::timeout(Duration::from_secs(5), decode_sent_iq(&transport, 1))
            .await
            .expect("a changed alias must trigger a fresh IQ instead of a hard error");
        assert_eq!(
            client.device_list_account_identity(&user).await.unwrap(),
            Some(previous)
        );
        let retry_id = retry
            .get()
            .attrs()
            .optional_string("id")
            .unwrap()
            .into_owned();
        answer_iq(
            &client,
            &retry_id,
            &tests::adv_wire_response_with_hosting(
                &user,
                waproto::codec::adv_signed_key_index_list_to_vec(&signed),
                2,
                true,
            ),
        )
        .await;
    }
    if matches!(case, EvictionCase::CancelledCaller) {
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        lookup.abort();
        assert!(lookup.await.unwrap_err().is_cancelled());
        release.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            while backend
                .load_identity(user.to_protocol_address().as_str())
                .await
                .unwrap()
                != Some(next)
                || backend.get_devices(&user.user).await.unwrap().is_none()
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    } else {
        let result = tokio::time::timeout(Duration::from_secs(5), &mut lookup)
            .await
            .expect("delete callback and publication must drain")
            .unwrap();
        let rejected = matches!(
            case,
            EvictionCase::RepeatedRefill
                | EvictionCase::MalformedCache
                | EvictionCase::RetiredPrimary
                | EvictionCase::ConnectionChanged
                | EvictionCase::DeleteError
                | EvictionCase::ReadError
        );
        assert_eq!(result.is_err(), rejected, "{case:?}: {result:?}");
        let expected = if matches!(case, EvictionCase::RetiredPrimary) {
            None
        } else if rejected {
            Some(previous)
        } else {
            Some(next)
        };
        assert_eq!(
            if matches!(case, EvictionCase::StrictAliasChanged) {
                client.device_list_account_identity(&user).await.unwrap()
            } else {
                client.cached_adv_account_identity(&user).await.unwrap()
            },
            expected,
            "{case:?}"
        );
        if rejected && !matches!(case, EvictionCase::RetiredPrimary) {
            assert!(
                has_session(&client, &companion).await,
                "failed eviction must precede destructive cleanup"
            );
            assert_eq!(
                backend
                    .get_devices(&user.user)
                    .await
                    .unwrap()
                    .unwrap()
                    .timestamp,
                1
            );
        }
        if matches!(case, EvictionCase::CacheAheadOfDatabase) {
            assert!(
                !has_session(&client, &companion).await,
                "L1-only companions must be retired"
            );
        }
        if matches!(
            case,
            EvictionCase::InstalledCompanion | EvictionCase::InstalledCompanionIdentityOnly
        ) {
            let address = user.with_device(9).to_protocol_address();
            assert!(
                !has_session(&client, &address).await,
                "a companion installed during eviction must retire with its old primary"
            );
            assert!(
                client
                    .signal_cache
                    .get_identity(&address, backend.as_ref())
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(!backend.has_session(address.as_str()).await.unwrap());
            assert!(
                backend
                    .load_identity(address.as_str())
                    .await
                    .unwrap()
                    .is_none()
            );
        }
    }
    assert!(!client.device_topology.unchanged_for(
        generation,
        &crate::client::member_index::MemberIndex::from_users([user.user.as_str()])
    ));
    client.signal_shutdown_sync();
}

/// Deleting a cache row must release account locks before a modifier awaits mapping work.
#[tokio::test]
async fn adv_eviction_delete_callback_can_await_mapping_from_device_modifier() {
    check_hosted_rotation_eviction(EvictionCase::MappingFromModifier).await;
}
/// Reentry must discover sessions installed under the old primary while locks were released.
#[tokio::test]
async fn adv_eviction_retires_companions_installed_during_delete_callback() {
    check_hosted_rotation_eviction(EvictionCase::InstalledCompanion).await;
}
/// Cleanup must enumerate trusted identities even when their companion session is already gone.
#[tokio::test]
async fn adv_eviction_retires_identity_only_companions_created_during_callback() {
    check_hosted_rotation_eviction(EvictionCase::InstalledCompanionIdentityOnly).await;
}
/// A read during eviction must not reject an otherwise valid primary rotation.
#[tokio::test]
async fn adv_eviction_allows_a_durable_read_during_the_delete_callback() {
    check_hosted_rotation_eviction(EvictionCase::RefilledCache).await;
}
/// A cache-only refill may be evicted again after continuity is revalidated.
#[tokio::test]
async fn adv_eviction_retries_a_cache_refill_without_a_topology_change() {
    check_hosted_rotation_eviction(EvictionCase::RefilledOnce).await;
}
/// Persistent cache refills must terminate without publishing trust or destroying old sessions.
#[tokio::test]
async fn adv_eviction_bounds_retries_before_trust_or_session_cleanup() {
    check_hosted_rotation_eviction(EvictionCase::RepeatedRefill).await;
}
/// Malformed host bytes cannot establish that the old registry row was removed.
#[tokio::test]
async fn adv_eviction_malformed_cache_data_does_not_prove_absence() {
    check_hosted_rotation_eviction(EvictionCase::MalformedCache).await;
}
/// A callback cannot retire a primary and then publish the response authenticated before retirement.
#[tokio::test]
async fn adv_eviction_rejects_primary_retirement_during_callback() {
    check_hosted_rotation_eviction(EvictionCase::RetiredPrimary).await;
}
/// Reacquiring maintenance must preserve the original connection generation.
#[tokio::test]
async fn adv_eviction_rejects_connection_replacement_during_callback() {
    check_hosted_rotation_eviction(EvictionCase::ConnectionChanged).await;
}
/// Failed host deletion must remain before every destructive identity replacement step.
#[tokio::test]
async fn adv_eviction_delete_error_preserves_trust_and_sessions() {
    check_hosted_rotation_eviction(EvictionCase::DeleteError).await;
}
/// A failed verification read cannot be treated as confirmed cache absence.
#[tokio::test]
async fn adv_eviction_read_error_does_not_prove_cache_absence() {
    check_hosted_rotation_eviction(EvictionCase::ReadError).await;
}
/// The owned publisher must complete eviction and publication after its public waiter is cancelled.
#[tokio::test]
async fn adv_eviction_survives_caller_cancellation() {
    check_hosted_rotation_eviction(EvictionCase::CancelledCaller).await;
}
/// Retain L1-only device history through eviction so its old companion state is also retired.
#[tokio::test]
async fn adv_eviction_retires_companions_when_cache_is_ahead_of_database() {
    check_hosted_rotation_eviction(EvictionCase::CacheAheadOfDatabase).await;
}

/// Strict refresh must requery after a callback changes aliases, preserving the
/// previous trust until a reply authenticated under the fresh mapping arrives.
#[tokio::test]
async fn adv_eviction_strict_refresh_retries_after_an_alias_change() {
    check_hosted_rotation_eviction(EvictionCase::StrictAliasChanged).await;
}

struct HostedPublicationEventProbe {
    client: std::sync::Weak<Client>,
    observations: std::sync::Mutex<Vec<[bool; 3]>>,
}

/// Waiting for device ownership must not renew the publication snapshot after a mapping ACK.
#[tokio::test]
async fn adv_review_round8_busy_after_mapping_ack_keeps_the_original_generation() {
    use crate::store::release::tests::ProbeBackend;
    let (mut probe, _) = ProbeBackend::new();
    let backend = probe.backend.clone();
    let (entered, arrived) = async_channel::unbounded();
    let (release, gate) = async_channel::unbounded();
    probe.gate_method = "after:put_lid_mappings";
    probe.gate_once = true;
    probe.entered = Some(entered);
    probe.gate = Some(gate);
    let (client, _) = crate::test_utils::create_iq_test_client_with_backend(Arc::new(probe)).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.lid_pn_cache.startup_finished_for_tests() {
            tokio::task::yield_now().await;
        }
        drop(client.lid_pn_cache.lock_mutation().await);
    })
    .await
    .unwrap();
    let phone = Jid::pn("12025550174");
    let lid = Jid::lid("100000000000174");
    crate::test_utils::seed_adv_primary(&client, &phone).await;
    client.flush_signal_cache_batch_safe().await.unwrap();
    let old = client.cached_adv_account_identity(&phone).await.unwrap();
    let epoch = client.signal_cache.identity_continuity.primary_snapshot();
    let generation = client.device_topology.current();
    let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let mut response = hosted_epoch_response(&phone);
    response.device_lists[0].key_index_bytes = Some(signed_list(&replacement, true));
    response.lid_mappings.push(wacore::usync::UsyncLidMapping {
        phone_number: phone.user.clone(),
        lid: lid.user.clone(),
    });
    let ownership = client.persistence_manager.lock_device_ownership().await;
    let operation = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .apply_owned_device_list_response(
                    response,
                    DeviceListApplyMode::StrictRefresh,
                    Some(generation),
                    None,
                    DeviceListAuthentication {
                        prepared: Vec::new(),
                        primary_epoch: epoch,
                    },
                    None,
                )
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(backend.get_lid_mapping(&lid.user).await.unwrap().is_some());
    assert!(client.device_topology.current() > generation);
    release.send(()).await.unwrap();
    // These guards must become available while ownership is still unavailable.
    drop(
        tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
            .await
            .unwrap(),
    );
    drop(
        tokio::time::timeout(
            Duration::from_secs(5),
            client.device_topology.lock_registry(),
        )
        .await
        .unwrap(),
    );
    assert!(!operation.is_finished());
    drop(ownership);
    assert!(
        tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
            .is_none()
    );
    assert_eq!(
        client.device_list_account_identity(&phone).await.unwrap(),
        old
    );
    assert!(
        backend.get_lid_mapping(&lid.user).await.unwrap().is_some(),
        "Busy is not a rollback of the acknowledged mapping"
    );
    for user in [&phone, &lid] {
        assert!(backend.get_devices(&user.user).await.unwrap().is_none());
        assert!(client.device_registry_cache.get(&user.user).await.is_none());
    }
    client.signal_shutdown_sync();
}

fn hosted_epoch_response(user: &Jid) -> DeviceListResponse {
    let mut response = migration_response(user);
    let list = &mut response.device_lists[0];
    list.devices[1].is_hosted = true;
    let mut signed =
        waproto::codec::adv_signed_key_index_list_decode(list.key_index_bytes.as_deref().unwrap())
            .unwrap();
    signed.account_signature_key = Some(
        crate::test_utils::adv_test_account()
            .public_key
            .public_key_bytes()
            .to_vec(),
    );
    let bytes = waproto::codec::adv_signed_key_index_list_to_vec(&signed);
    assert!(wacore::adv::verify_hosted_key_index_list(&bytes).is_some());
    list.key_index_bytes = Some(bytes);
    response
}

/// Failure to capture the request epoch must fail before hosted trust publication.
#[tokio::test]
async fn adv_review_round7_unavailable_request_epoch_cannot_publish_a_hosted_list() {
    for available in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let user = Jid::pn("12025550182");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        client.flush_signal_cache_batch_safe().await.unwrap();
        let before = client.cached_adv_account_identity(&user).await.unwrap();
        let response = hosted_epoch_response(&user);
        let epoch = available
            .then(|| client.signal_cache.identity_continuity.primary_snapshot())
            .flatten();
        let result = client
            .apply_device_list_response_in_epoch(
                &response,
                DeviceListApplyMode::CachePreferred,
                None,
                None,
                epoch,
            )
            .await;
        assert_eq!(result.is_ok(), available);
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            before
        );
        let backend = client.persistence_manager.backend();
        assert_eq!(
            backend.get_devices(&user.user).await.unwrap().is_some(),
            available
        );
        assert_eq!(
            client.device_registry_cache.get(&user.user).await.is_some(),
            available
        );
        client.signal_shutdown_sync();
    }
}

/// An invalid hosted member must not suppress independently authenticated group members.
#[tokio::test]
async fn adv_review_round7_retired_hosted_epoch_does_not_reject_a_valid_group_peer() {
    for retire in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let retired = Jid::pn("12025550181");
        let valid = Jid::pn("12025550180");
        for user in [&retired, &valid] {
            crate::test_utils::seed_adv_primary(&client, user).await;
        }
        client.flush_signal_cache_batch_safe().await.unwrap();
        let epoch = client.signal_cache.identity_continuity.primary_snapshot();
        if retire {
            client
                .signal_cache
                .delete_identity(&retired.to_protocol_address())
                .await;
            client.flush_signal_cache_batch_safe().await.unwrap();
        }
        let mut response = hosted_epoch_response(&retired);
        response
            .device_lists
            .extend(hosted_epoch_response(&valid).device_lists);
        let devices = client
            .try_process_group_device_list_response_in_epoch(
                &response,
                &[retired.clone(), valid.clone()],
                client.device_topology.current(),
                epoch,
            )
            .await
            .unwrap()
            .unwrap();
        assert!(devices.contains(&valid));
        assert!(
            devices.iter().any(|device| device.user == valid.user
                && device.device == 7
                && device.is_hosted())
        );
        assert_eq!(
            devices.iter().any(|device| device.user == retired.user),
            !retire
        );
        let backend = client.persistence_manager.backend();
        assert_eq!(
            backend.get_devices(&retired.user).await.unwrap().is_some(),
            !retire
        );
        assert!(backend.get_devices(&valid.user).await.unwrap().is_some());
        assert_eq!(
            client
                .device_registry_cache
                .get(&retired.user)
                .await
                .is_some(),
            !retire
        );
        assert_eq!(
            client
                .cached_adv_account_identity(&retired)
                .await
                .unwrap()
                .is_some(),
            !retire
        );
        client.signal_shutdown_sync();
    }
}

/// Recovering one account must preserve the original authentication epoch of its batch peers.
#[tokio::test]
async fn adv_review_round7_recovery_cannot_refresh_another_hosted_rows_epoch() {
    let (client, transport) = create_iq_test_client().await;
    let ordinary = Jid::pn("12025550179");
    let hosted = Jid::pn("12025550178");
    for user in [&ordinary, &hosted] {
        crate::test_utils::seed_adv_primary(&client, user).await;
    }
    client.flush_signal_cache_batch_safe().await.unwrap();
    let previous = client.cached_adv_account_identity(&ordinary).await.unwrap();
    let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let ordinary_wire = adv_wire_response(&ordinary, signed_list(&replacement, true));
    let hosted_wire = tests::adv_wire_response_with_hosting(
        &hosted,
        hosted_epoch_response(&hosted).device_lists[0]
            .key_index_bytes
            .clone()
            .unwrap(),
        2,
        true,
    );
    let users: Vec<_> = [&ordinary_wire, &hosted_wire]
        .iter()
        .map(|wire| {
            wire.get_optional_child("usync")
                .unwrap()
                .get_optional_child("list")
                .unwrap()
                .children()
                .unwrap()[0]
                .clone()
        })
        .collect();
    let response = NodeBuilder::new("iq")
        .attr("type", "result")
        .children([NodeBuilder::new("usync")
            .children([NodeBuilder::new("list").children(users).build()])
            .build()])
        .build();
    let epoch = client.signal_cache.identity_continuity.primary_snapshot();
    let query = {
        let client = client.clone();
        let users = vec![ordinary.clone(), hosted.clone()];
        tokio::spawn(async move {
            client
                .fetch_user_devices_once(users, Freshness::CachePreferred)
                .await
        })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &response).await;
    let failure = query
        .await
        .unwrap()
        .unwrap_err()
        .downcast::<DeviceListAdvError>()
        .unwrap();
    assert_eq!(failure.authentication_epoch, epoch);
    let generation = client.device_topology.current();
    client
        .signal_cache
        .delete_identity(&hosted.to_protocol_address())
        .await;
    client.flush_signal_cache_batch_safe().await.unwrap();
    assert_eq!(client.device_topology.current(), generation);
    let recovery = {
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(failure.into()).await })
    };
    let iq = decode_sent_iq(&transport, 1).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(
        &client,
        &id,
        &primary_response(&ordinary, &replacement, true),
    )
    .await;
    let result = tokio::time::timeout(Duration::from_secs(5), recovery)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_err());
    assert_eq!(
        client.cached_adv_account_identity(&hosted).await.unwrap(),
        None
    );
    assert_eq!(
        client.cached_adv_account_identity(&ordinary).await.unwrap(),
        previous
    );
    let backend = client.persistence_manager.backend();
    for user in [&ordinary, &hosted] {
        assert!(backend.get_devices(&user.user).await.unwrap().is_none());
        assert!(client.device_registry_cache.get(&user.user).await.is_none());
    }
    client.signal_shutdown_sync();
}

impl wacore::types::events::EventHandler for HostedPublicationEventProbe {
    fn handle_event(&self, event: Arc<Event>) {
        if !matches!(event.as_ref(), Event::IdentityChange(_)) {
            return;
        }
        let client = self.client.upgrade().unwrap();
        let waker = futures::task::noop_waker();
        let mut context = std::task::Context::from_waker(&waker);
        let mut ownership = std::pin::pin!(client.persistence_manager.lock_device_ownership());
        let own_free = ownership.as_mut().poll(&mut context).is_ready();
        let mut mapping = std::pin::pin!(client.lid_pn_cache.lock_mutation());
        let map_free = mapping.as_mut().poll(&mut context).is_ready();
        let mut registry = std::pin::pin!(client.device_topology.lock_registry());
        let registry_free = registry.as_mut().poll(&mut context).is_ready();
        self.observations
            .lock()
            .unwrap()
            .push([own_free, map_free, registry_free]);
    }
}

/// Exercise the public IQ boundary, not direct response application.
#[tokio::test]
async fn adv_review_round7_hosted_cache_preferred_iq_cannot_restore_a_retired_primary() {
    for reset in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let user = Jid::pn("12025550185");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        client.flush_signal_cache_batch_safe().await.unwrap();
        let key = client
            .cached_adv_account_identity(&user)
            .await
            .unwrap()
            .unwrap();
        let backend = client.persistence_manager.backend();
        let operation = {
            let client = client.clone();
            let user = user.clone();
            tokio::spawn(async move {
                client
                    .signal()
                    .get_user_devices(std::slice::from_ref(&user))
                    .await
            })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        if reset {
            deliver_identity_reset(&client, &user, None).await;
            assert_eq!(
                client.cached_adv_account_identity(&user).await.unwrap(),
                None
            );
            assert_eq!(
                backend
                    .load_identity(user.to_protocol_address().as_str())
                    .await
                    .unwrap(),
                None
            );
        }
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(key.to_vec());
        let response = tests::adv_wire_response_with_hosting(
            &user,
            waproto::codec::adv_signed_key_index_list_to_vec(&signed),
            2,
            true,
        );
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &response).await;
        let result = tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap();
        if reset {
            assert_eq!(
                client.cached_adv_account_identity(&user).await.unwrap(),
                None,
                "a hosted IQ begun before retirement cannot restore its old primary"
            );
            assert_eq!(
                backend
                    .load_identity(user.to_protocol_address().as_str())
                    .await
                    .unwrap(),
                None
            );
            assert!(result.is_err());
            assert!(backend.get_devices(&user.user).await.unwrap().is_none());
            assert!(client.device_registry_cache.get(&user.user).await.is_none());
        } else {
            let devices = result.unwrap();
            assert!(devices.contains(&user));
            assert!(devices.iter().any(|device| device.user == user.user
                && device.device == 2
                && device.is_hosted()));
            let record = backend.get_devices(&user.user).await.unwrap().unwrap();
            assert_eq!(
                record
                    .devices
                    .iter()
                    .map(|device| device.device_id())
                    .collect::<Vec<_>>(),
                vec![0, 2]
            );
            assert_eq!(
                client.cached_adv_account_identity(&user).await.unwrap(),
                Some(key)
            );
        }
        client.signal_shutdown_sync();
    }
}

/// A host callback can await a different task without reentering its manager.
#[tokio::test]
async fn adv_review_round7_hosted_publication_cannot_deadlock_an_async_device_modifier() {
    use crate::lid_pn_cache::LearningSource;
    use std::sync::atomic::{AtomicBool, Ordering};
    for (hold_modifier, cancel, fail_registry) in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
        (false, false, true),
    ] {
        let (pending, waiting) = async_channel::unbounded();
        let (completed, finished) = async_channel::unbounded();
        let runtime = Arc::new(BootstrapWorkerRuntime {
            armed: AtomicBool::new(false),
            pending,
            completed,
        });
        let backend = crate::test_utils::create_test_backend().await;
        let backend: Arc<dyn crate::store::traits::Backend> = if fail_registry {
            let (mut probe, _) = crate::store::release::tests::ProbeBackend::new();
            probe.backend = backend;
            probe.fail_before_method =
                Some(("update_device_lists", Arc::new(AtomicBool::new(true))));
            Arc::new(probe)
        } else {
            backend
        };
        let (client, transport) =
            crate::test_utils::create_iq_test_client_with_runtime(backend, runtime.clone()).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            while !client.lid_pn_cache.startup_finished_for_tests() {
                tokio::task::yield_now().await;
            }
            drop(client.lid_pn_cache.lock_mutation().await);
        })
        .await
        .unwrap();
        let user = Jid::pn("12025550184");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        client.flush_signal_cache_batch_safe().await.unwrap();
        let probe = Arc::new(HostedPublicationEventProbe {
            client: Arc::downgrade(&client),
            observations: std::sync::Mutex::new(Vec::new()),
        });
        let subscription = client.subscribe_handler(probe.clone());
        assert!(subscription.update_interest(EventInterest::of(&[EventKind::IdentityChange])));
        subscription.detach();
        let next_pair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let next = *next_pair.public_key.public_key_bytes();
        let (release, callback_wait) = async_channel::bounded(1);
        let (entered, callback_entered) = async_channel::bounded(1);
        let mut modifier = hold_modifier.then(|| {
            let manager = client.persistence_manager.clone();
            tokio::spawn(async move {
                manager
                    .modify_device_async(move |_| {
                        Box::pin(async move {
                            entered.send(()).await.unwrap();
                            callback_wait.recv().await.unwrap();
                        })
                    })
                    .await;
            })
        });
        if hold_modifier {
            tokio::time::timeout(Duration::from_secs(5), callback_entered.recv())
                .await
                .unwrap()
                .unwrap();
        }
        let mut lookup = {
            let client = client.clone();
            let user = user.clone();
            tokio::spawn(async move {
                client
                    .signal()
                    .get_user_devices(std::slice::from_ref(&user))
                    .await
            })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(next.to_vec());
        signed.account_signature = Some(
            next_pair
                .private_key
                .calculate_signature(
                    &[&[6, 2], signed.details.as_ref().unwrap().as_slice()].concat(),
                    &mut rand::make_rng::<rand::rngs::StdRng>(),
                )
                .unwrap()
                .to_vec(),
        );
        let signed = waproto::codec::adv_signed_key_index_list_to_vec(&signed);
        assert_eq!(
            wacore::adv::verify_hosted_key_index_list(&signed).map(|(_, key)| key),
            Some(next)
        );
        let response = tests::adv_wire_response_with_hosting(&user, signed, 2, true);
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        if hold_modifier {
            runtime.armed.store(true, Ordering::SeqCst);
        }
        answer_iq(&client, &id, &response).await;
        if hold_modifier {
            tokio::time::timeout(Duration::from_secs(5), waiting.recv())
                .await
                .unwrap()
                .unwrap();
            if cancel {
                lookup.abort();
                assert!((&mut lookup).await.unwrap_err().is_cancelled());
            }
            let mut host_task = {
                let client = client.clone();
                tokio::spawn(async move {
                    let result = client
                        .add_lid_pn_mapping("100000000000186", "12025550186", LearningSource::Usync)
                        .await;
                    let _ = release.send(()).await;
                    result
                })
            };
            let outcome = tokio::time::timeout(Duration::from_secs(5), &mut host_task).await;
            if outcome.is_err() {
                let modifier = modifier.take().unwrap();
                modifier.abort();
                let _ = modifier.await;
                let host_drained = tokio::time::timeout(Duration::from_secs(5), &mut host_task)
                    .await
                    .is_ok();
                let publication_drained = tokio::time::timeout(Duration::from_secs(5), &mut lookup)
                    .await
                    .is_ok();
                client.signal_shutdown_sync();
                panic!(
                    "publication and the async modifier formed an ownership/mapping lock cycle; cleanup host={host_drained}, publication={publication_drained}"
                );
            }
            outcome.unwrap().unwrap().unwrap();
            tokio::time::timeout(Duration::from_secs(5), modifier.take().unwrap())
                .await
                .unwrap()
                .unwrap();
            tokio::time::timeout(Duration::from_secs(5), finished.recv())
                .await
                .unwrap()
                .unwrap();
        }
        if !cancel {
            let result = tokio::time::timeout(Duration::from_secs(5), lookup)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(result.is_err(), fail_registry);
            if let Ok(devices) = result {
                assert!(devices.contains(&user));
                assert!(devices.iter().any(|device| device.user == user.user
                    && device.device == 2
                    && device.is_hosted()));
            }
        }
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            Some(next)
        );
        let row = client
            .persistence_manager
            .backend()
            .get_devices(&user.user)
            .await
            .unwrap();
        if fail_registry {
            assert!(row.is_none());
        } else {
            assert_eq!(
                row.unwrap()
                    .devices
                    .iter()
                    .map(|device| device.device_id())
                    .collect::<Vec<_>>(),
                vec![0, 2]
            );
        }
        let observations = probe.observations.lock().unwrap();
        assert_eq!(observations.len(), 1);
        if !hold_modifier {
            assert_eq!(
                observations.as_slice(),
                [[true, true, true]],
                "host reactions must follow ownership, mapping and registry release, including after a failed registry ACK"
            );
        }
        client.signal_shutdown_sync();
    }
}

/// A cache miss must wait for the writer ACK before reading and promoting durable topology.
#[tokio::test]
async fn adv_review_round6_registry_read_waits_for_the_cache_first_writer_ack() {
    use crate::store::release::tests::ProbeBackend;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let enabled = Arc::new(AtomicBool::new(false));
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate_method = "update_device_list";
    probe.gate_once = true;
    probe.gate_enabled = Some(enabled.clone());
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, _) = crate::test_utils::create_iq_test_client_with_backend(Arc::new(probe)).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.lid_pn_cache.startup_finished_for_tests() {
            tokio::task::yield_now().await;
        }
        drop(client.lid_pn_cache.lock_mutation().await);
    })
    .await
    .unwrap();
    let peer = Jid::pn("12025550187");
    let record = |raw_id| DeviceListRecord {
        user: peer.user.as_str().into(),
        devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
        raw_id: Some(raw_id),
        timestamp: 1,
        phash: None,
    };
    raw.update_device_list(record(1)).await.unwrap();
    enabled.store(true, Ordering::Release);
    let writer = {
        let client = client.clone();
        let replacement = record(2);
        tokio::spawn(async move { client.update_device_list(replacement).await })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client
            .device_registry_cache
            .get(&peer.user)
            .await
            .unwrap()
            .raw_id,
        Some(2)
    );
    assert_eq!(
        raw.get_devices(&peer.user).await.unwrap().unwrap().raw_id,
        Some(1)
    );
    let generation = client.device_topology.current();
    // Model an ordinary L1 eviction after its new generation was published,
    // while the same writer is still waiting for its durable operation.
    client
        .device_registry_cache
        .raw_invalidate_for_tests(&peer.user)
        .await;
    assert_eq!(client.device_topology.current(), generation);
    let mut reader = Box::pin(client.load_device_record_for_jid(&peer));
    assert!(futures::poll!(reader.as_mut()).is_pending());
    release.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), writer)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let loaded = tokio::time::timeout(Duration::from_secs(5), reader)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.raw_id, Some(2));
    assert_eq!(
        client
            .device_registry_cache
            .get(&peer.user)
            .await
            .unwrap()
            .raw_id,
        Some(2)
    );
    assert_eq!(
        raw.get_devices(&peer.user).await.unwrap().unwrap().raw_id,
        Some(2)
    );
    client.signal_shutdown_sync();
}

/// Caller cancellation must not let delayed host writes restore rows after invalidation.
#[tokio::test]
async fn adv_review_round6_cancelled_registry_publication_retains_custom_store_ack() {
    use crate::cache_config::{CacheConfig, CacheStores};
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    type SyntheticCacheRows = Arc<std::sync::Mutex<HashMap<(String, String), Vec<u8>>>>;

    struct DelayedStore {
        rows: SyntheticCacheRows,
        taken: AtomicBool,
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
        fail_after_commit: bool,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl wacore::store::CacheStore for DelayedStore {
        async fn get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self
                .rows
                .lock()
                .unwrap()
                .get(&(namespace.into(), key.into()))
                .cloned())
        }

        async fn set(
            &self,
            namespace: &str,
            key: &str,
            value: &[u8],
            _: Option<Duration>,
        ) -> anyhow::Result<()> {
            let key = (namespace.to_owned(), key.to_owned());
            if self.taken.swap(true, Ordering::AcqRel) {
                self.rows.lock().unwrap().insert(key, value.to_vec());
                return Ok(());
            }
            // Model a host operation that continues after its Rust waiter is
            // dropped. The effect deliberately lands only after release.
            let rows = Arc::clone(&self.rows);
            let value = value.to_vec();
            let entered = self.entered.clone();
            let release = self.release.clone();
            let fail = self.fail_after_commit;
            let (send, receive) = tokio::sync::oneshot::channel();
            drop(tokio::spawn(async move {
                entered.send(()).await.unwrap();
                release.recv().await.unwrap();
                rows.lock().unwrap().insert(key, value);
                let result = if fail {
                    Err(anyhow::anyhow!("synthetic cache error after commit"))
                } else {
                    Ok(())
                };
                let _ = send.send(result);
            }));
            receive
                .await
                .map_err(|_| anyhow::anyhow!("synthetic cache worker stopped"))?
        }

        async fn delete(&self, namespace: &str, key: &str) -> anyhow::Result<()> {
            self.rows
                .lock()
                .unwrap()
                .remove(&(namespace.into(), key.into()));
            Ok(())
        }

        async fn clear(&self, namespace: &str) -> anyhow::Result<()> {
            self.rows
                .lock()
                .unwrap()
                .retain(|(stored, _), _| stored != namespace);
            Ok(())
        }
    }

    for fail_after_commit in [false, true] {
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        let store = Arc::new(DelayedStore {
            rows: Arc::new(std::sync::Mutex::new(HashMap::new())),
            taken: AtomicBool::new(false),
            entered,
            release: gate,
            fail_after_commit,
        });
        let config = CacheConfig::default().with_cache_stores(CacheStores {
            device_registry_cache: Some(store.clone()),
            ..Default::default()
        });
        let client = crate::test_utils::create_test_client_with_config(
            "registry_fill_ack",
            Arc::new(crate::test_utils::MockHttpClient),
            config,
        )
        .await;
        let peer = Jid::pn("12025550192");
        crate::test_utils::seed_adv_primary(&client, &peer).await;
        let backend = client.persistence_manager.backend();
        backend
            .update_device_list(DeviceListRecord {
                user: peer.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        let reader = {
            let client = client.clone();
            let peer = peer.clone();
            tokio::spawn(async move {
                client
                    .process_device_list_response(&migration_response(&peer), Freshness::Refresh)
                    .await
                    .unwrap();
            })
        };
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        reader.abort();
        assert!(reader.await.unwrap_err().is_cancelled());
        let mut invalidation = Box::pin(async {
            let guard = client.device_topology.lock_registry().await;
            client
                .try_invalidate_device_cache_guarded(&peer.user, &guard)
                .await
        });
        assert!(
            futures::poll!(invalidation.as_mut()).is_pending(),
            "caller cancellation must not release ordering before the cache store ACK"
        );
        assert!(backend.get_devices(&peer.user).await.unwrap().is_some());
        release.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), invalidation)
            .await
            .unwrap()
            .unwrap();
        assert!(backend.get_devices(&peer.user).await.unwrap().is_none());
        assert!(client.device_registry_cache.get(&peer.user).await.is_none());
        assert!(store.rows.lock().unwrap().is_empty());
        client.signal_shutdown_sync();
    }
}

/// A queued fallback must finish before invalidation deletes the durable and cached row.
#[tokio::test]
async fn adv_review_round6_queued_registry_fill_cannot_restore_a_deleted_row() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (mut probe, _) = ProbeBackend::new();
    let backend = probe.backend.clone();
    let enabled = Arc::new(AtomicBool::new(false));
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate_enabled = Some(enabled.clone());
    probe.gate_once = true;
    probe.gate_method = "after:get_devices";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let peer = Jid::pn("12025550188");
    backend
        .update_device_list(DeviceListRecord {
            user: peer.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.lid_pn_cache.startup_finished_for_tests() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    enabled.store(true, Ordering::Release);
    let mut load = Box::pin(client.load_device_record_for_jid(&peer));
    assert!(futures::poll!(load.as_mut()).is_pending());
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    enabled.store(false, Ordering::Release);
    let cache = client.device_registry_cache.local_for_tests().unwrap();
    let writer = cache.hold_write_for_test().await;
    release.send(()).await.unwrap();
    assert!(futures::poll!(load.as_mut()).is_pending()); // promotion queued behind writer
    drop(writer);
    // Keep the old fill unpolled until invalidation has had its first attempt.
    let mut invalidation = Box::pin(async {
        let registry = client.device_topology.lock_registry().await;
        client
            .try_invalidate_device_cache_guarded(&peer.user, &registry)
            .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        match futures::poll!(invalidation.as_mut()) {
            std::task::Poll::Ready(result) => {
                result.unwrap();
                let _ = load.await;
            }
            std::task::Poll::Pending => {
                let _ = load.await;
                invalidation.await.unwrap();
            }
        }
    })
    .await
    .expect("both admitted operations must finish");
    assert!(backend.get_devices(&peer.user).await.unwrap().is_none());
    assert!(
        client.device_registry_cache.get(&peer.user).await.is_none(),
        "a queued pre-delete DB fill must not resurrect the deleted cache row"
    );
    client.signal_shutdown_sync();
}

/// Produce real signed Signal material, retaining the validated certificate anchor.
fn session_bundle(
    identity: &KeyPair,
    device: u16,
    account: Option<&KeyPair>,
) -> wacore::libsignal::protocol::PreKeyBundle {
    use wacore::libsignal::protocol::{IdentityKey, PreKeyBundle};
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signed = KeyPair::generate(&mut rng);
    let signature = identity
        .private_key
        .calculate_signature(&signed.public_key.serialize(), &mut rng)
        .unwrap();
    let bundle = PreKeyBundle::new(
        1234,
        u32::from(device).into(),
        None,
        1.into(),
        signed.public_key,
        signature.to_vec(),
        IdentityKey::new(identity.public_key),
    )
    .unwrap();
    match account {
        Some(account) => bundle.with_account_identity(IdentityKey::new(account.public_key)),
        None => bundle,
    }
}

/// Encode a fictitious primary IQ response with a valid or corrupted prekey.
fn primary_response(user: &Jid, account: &KeyPair, valid: bool) -> wacore_binary::Node {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signed = KeyPair::generate(&mut rng);
    let mut signature = account
        .private_key
        .calculate_signature(&signed.public_key.serialize(), &mut rng)
        .unwrap()
        .to_vec();
    if !valid {
        signature[0] ^= 1;
    }
    NodeBuilder::new("iq")
        .attr("type", "result")
        .children([NodeBuilder::new("list")
            .children([NodeBuilder::new("user")
                .attr("jid", user.to_string())
                .children([
                    NodeBuilder::new("registration")
                        .bytes(1234u32.to_be_bytes().to_vec())
                        .build(),
                    NodeBuilder::new("type").bytes(vec![5]).build(),
                    NodeBuilder::new("identity")
                        .bytes(account.public_key.public_key_bytes().to_vec())
                        .build(),
                    NodeBuilder::new("skey")
                        .children([
                            NodeBuilder::new("id").bytes(vec![0, 0, 1]).build(),
                            NodeBuilder::new("value")
                                .bytes(signed.public_key.public_key_bytes().to_vec())
                                .build(),
                            NodeBuilder::new("signature").bytes(signature).build(),
                        ])
                        .build(),
                ])
                .build()])
            .build()])
        .build()
}

/// Sign the shared device-list fixture with a chosen account key.
fn signed_key_index_for_epoch(valid_indexes: Vec<u32>, current_index: u32, raw_id: u32) -> Vec<u8> {
    let mut details = waproto::whatsapp::ADVKeyIndexList::default();
    details.raw_id = Some(raw_id);
    details.timestamp = Some(1_700_000_000);
    details.current_index = Some(current_index);
    details.valid_indexes = valid_indexes;
    crate::test_utils::sign_adv_key_index(waproto::codec::adv_key_index_list_to_vec(&details))
}

fn signed_list(account: &KeyPair, embedded_key: bool) -> Vec<u8> {
    let mut signed =
        waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
            .unwrap();
    signed.account_signature = Some(
        account
            .private_key
            .calculate_signature(
                &[&[6, 2], signed.details.as_ref().unwrap().as_slice()].concat(),
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )
            .unwrap()
            .to_vec(),
    );
    signed.account_signature_key =
        embedded_key.then(|| account.public_key.public_key_bytes().to_vec());
    waproto::codec::adv_signed_key_index_list_to_vec(&signed)
}

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Rotated,
    InvalidPrekey,
    WrongPrimary,
    TopologyChanged,
    IdentityChanged,
    AliasIdentityChanged,
}

/// Report the real processing-permit slow path, so the test can order a
/// receiver's primary lock attempt after recovery has started waiting.
struct PermitWaitRuntime(async_channel::Sender<()>);

impl wacore::runtime::Runtime for PermitWaitRuntime {
    fn spawn(
        &self,
        future: wacore::runtime::BoxFuture<'static, ()>,
    ) -> wacore::runtime::AbortHandle {
        wacore::runtime::Runtime::spawn(&crate::runtime_impl::TokioRuntime, future)
    }
    fn sleep(&self, duration: Duration) -> wacore::runtime::BoxFuture<'static, ()> {
        if duration == Duration::from_secs(10) {
            let _ = self.0.try_send(());
        }
        wacore::runtime::Runtime::sleep(&crate::runtime_impl::TokioRuntime, duration)
    }
    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> wacore::runtime::BoxFuture<'static, ()> {
        wacore::runtime::Runtime::spawn_blocking(&crate::runtime_impl::TokioRuntime, f)
    }
    fn yield_now(&self) -> Option<wacore::runtime::BoxFuture<'static, ()>> {
        wacore::runtime::Runtime::yield_now(&crate::runtime_impl::TokioRuntime)
    }
}

/// Observe the selected owned worker itself, rather than assuming mutex waiter
/// ordering proves that a cancelled request's detached work has completed.
struct BootstrapWorkerRuntime {
    armed: std::sync::atomic::AtomicBool,
    pending: async_channel::Sender<()>,
    completed: async_channel::Sender<()>,
}

impl wacore::runtime::Runtime for BootstrapWorkerRuntime {
    fn spawn(
        &self,
        mut future: wacore::runtime::BoxFuture<'static, ()>,
    ) -> wacore::runtime::AbortHandle {
        let observe = self.armed.swap(false, std::sync::atomic::Ordering::SeqCst);
        let pending = self.pending.clone();
        let completed = self.completed.clone();
        wacore::runtime::Runtime::spawn(
            &crate::runtime_impl::TokioRuntime,
            Box::pin(async move {
                let mut reported = false;
                futures::future::poll_fn(|cx| {
                    let result = future.as_mut().poll(cx);
                    if observe && !reported && result.is_pending() {
                        reported = true;
                        pending.try_send(()).unwrap();
                    }
                    result
                })
                .await;
                if observe {
                    completed.try_send(()).unwrap();
                }
            }),
        )
    }
    fn sleep(&self, duration: Duration) -> wacore::runtime::BoxFuture<'static, ()> {
        wacore::runtime::Runtime::sleep(&crate::runtime_impl::TokioRuntime, duration)
    }
    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> wacore::runtime::BoxFuture<'static, ()> {
        wacore::runtime::Runtime::spawn_blocking(&crate::runtime_impl::TokioRuntime, f)
    }
    fn yield_now(&self) -> Option<wacore::runtime::BoxFuture<'static, ()>> {
        wacore::runtime::Runtime::yield_now(&crate::runtime_impl::TokioRuntime)
    }
}

/// Before the fix this parks the recovery on the sole permit while the
/// permit's receiver waits for the primary mutex held by that recovery.
#[tokio::test]
async fn offline_recovery_admits_processing_before_primary_locks() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    use wacore::usync::{UserDeviceList, UsyncDevice};
    let (waiting, recovery_waiting) = async_channel::unbounded();
    let (client, transport) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550199");
    crate::test_utils::seed_adv_primary(&client, &user).await;
    let old = client.cached_adv_account_identity(&user).await.unwrap();
    seed_fresh_session(&client, &user).await;
    seed_fresh_session(&client, &user.with_device(7)).await;
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let response = DeviceListResponse {
        device_lists: vec![UserDeviceList {
            user: user.clone(),
            devices: vec![UsyncDevice::new(0, None)],
            phash: None,
            key_index_bytes: Some(signed_list(&replacement, false)),
        }],
        lid_mappings: vec![],
    };
    let error = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap_err();
    client.enter_drain_mode_for_tests();
    assert!(!client.inbound_commit_batch.has_entries());
    let mutex = client
        .session_lock_for(user.to_protocol_address().as_str())
        .await;
    let (entered, owns_permit) = async_channel::unbounded();
    let (decrypt, start_decrypt) = async_channel::unbounded();
    let inbound = {
        let client = client.clone();
        tokio::spawn(async move {
            let _permit = client.acquire_message_processing_permit().await;
            entered.send(()).await.unwrap();
            start_decrypt.recv().await.unwrap();
            let _session = mutex.lock_arc().await;
        })
    };
    owns_permit.recv().await.unwrap();
    let recovery = {
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(error).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&user, &replacement, true)).await;
    tokio::time::timeout(Duration::from_secs(5), recovery_waiting.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&user).await.unwrap(),
        old
    );
    decrypt.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), inbound)
        .await
        .expect("recovery must not own the primary while awaiting the drain permit")
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), recovery)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&user).await.unwrap(),
        Some(*replacement.public_key.public_key_bytes())
    );
    assert!(!has_session(&client, &user.with_device(7).to_protocol_address()).await);
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn stale_maintenance_permit_cannot_flush_a_new_drain() {
    let (client, _) = create_iq_test_client().await;
    let scope = client.begin_signal_maintenance().await.unwrap();
    client.enter_drain_mode_for_tests();
    assert!(scope.flush().await.is_err());
    drop(scope);
    client.signal_shutdown_sync();
}

/// A response candidate is locked before authentication; a different alias
/// learned after that snapshot must still reject replay without resetting it.
#[tokio::test]
async fn ordinary_recovery_rejects_an_alias_restored_after_its_lock_snapshot() {
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    // This checkpoint excludes database scheduling as a source of Pending:
    // the warmed session locks and immediate in-memory reads leave only the
    // deliberately held candidate lock before authentication can start.
    let (client, transport) = crate::test_utils::create_iq_test_client_with_backend(Arc::new(
        wacore::store::in_memory::InMemoryBackend::new(),
    ))
    .await;
    let phone = Jid::pn("12025550193");
    let lid = Jid::lid("100000000000193");
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
        seed_fresh_session(&client, user).await;
    }
    client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            phone.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    client.lid_pn_cache.clear().await;
    assert!(client.device_list_identity_alias(&phone).await.is_none());
    let alias_lock = client
        .session_lock_for(lid.to_protocol_address().as_str())
        .await;
    let alias_guard = alias_lock.lock_arc().await;
    let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let error = DeviceListAdvError {
        user: phone.clone(),
        hosted: false,
        response: DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: phone.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: Some(signed_list(&replacement, false)),
            }],
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.user.clone(),
                lid: lid.user.clone(),
            }],
        },
        topology_generation: client.device_topology.current(),
        authentication_epoch: client.signal_cache.identity_continuity.primary_snapshot(),
    };
    let primary_lock = client
        .session_lock_for(phone.to_protocol_address().as_str())
        .await;
    let mut recovery = client.recover_ordinary_adv_identity(error.into());
    assert!(futures::poll!(recovery.as_mut()).is_pending());
    assert!(wacore::types::jid::cmp_for_lock_order(&phone, &lid).is_lt());
    assert!(
        primary_lock.try_lock().is_none(),
        "recovery must own the earlier primary lock while waiting for the candidate; a merely queued IQ releases that lock"
    );
    assert!(
        transport.sent().is_empty(),
        "the missing candidate lock must precede the IQ"
    );
    drop(alias_guard);
    let changed_alias = Jid::lid("100000000000194");
    let changed_lock = client
        .session_lock_for(changed_alias.to_protocol_address().as_str())
        .await;
    let changed_guard = changed_lock.lock_arc().await;
    let reply = async {
        let iq = decode_sent_iq(&transport, 0).await;
        client
            .lid_pn_cache
            .add(&crate::lid_pn_cache::LidPnEntry::new(
                changed_alias.user.to_string(),
                phone.user.to_string(),
                crate::lid_pn_cache::LearningSource::Usync,
            ))
            .await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&phone, &replacement, true)).await;
    };
    let (result, ()) = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(recovery, reply)
    })
    .await
    .unwrap();
    let error = result.unwrap_err();
    assert!(
        error.to_string().contains("account alias changed"),
        "{error:#}"
    );
    assert!(has_session(&client, &lid.to_protocol_address()).await);
    assert_eq!(
        client.cached_adv_account_identity(&lid).await.unwrap(),
        Some(
            *crate::test_utils::adv_test_account()
                .public_key
                .public_key_bytes()
        )
    );
    assert_eq!(
        client.device_list_identity_alias(&phone).await,
        Some(changed_alias)
    );
    drop(changed_guard);
    client.signal_shutdown_sync();
}

/// A different contact's refresh must wait outside the global topology locks.
/// Otherwise a recovery owning the drain permit can wait on that refresh
/// forever while the refresh waits for the permit to persist its cleanup.
#[tokio::test]
async fn offline_refresh_waits_for_processing_before_global_topology_locks() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    use wacore::usync::{UserDeviceList, UsyncDevice};
    let (waiting, permit_wait) = async_channel::unbounded();
    let (client, _) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550194");
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let refresh = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .process_device_list_response(
                    &DeviceListResponse {
                        device_lists: vec![UserDeviceList {
                            user,
                            devices: vec![UsyncDevice::new(0, None)],
                            phash: None,
                            key_index_bytes: None,
                        }],
                        lid_mappings: vec![],
                    },
                    Freshness::Refresh,
                )
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), permit_wait.recv())
        .await
        .unwrap()
        .unwrap();
    let mapping = tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
        .await
        .expect("refresh must not hold mappings while waiting for processing");
    let registry = tokio::time::timeout(
        Duration::from_secs(5),
        client.device_topology.lock_registry(),
    )
    .await
    .expect("refresh must not hold registry while waiting for processing");
    drop(registry);
    drop(mapping);
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), refresh)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn offline_device_removal_releases_registry_before_waiting_for_processing() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    let (waiting, permit_wait) = async_channel::unbounded();
    let (client, _) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550199");
    seed_fresh_session(&client, &user.with_device(7)).await;
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let removal = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move { client.patch_device_remove(&user.user, 7).await })
    };
    tokio::time::timeout(Duration::from_secs(5), permit_wait.recv())
        .await
        .unwrap()
        .unwrap();
    let registry = tokio::time::timeout(
        Duration::from_secs(5),
        client.device_topology.lock_registry(),
    )
    .await
    .expect("notification must release registry before waiting for processing");
    drop(registry);
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), removal)
        .await
        .unwrap()
        .unwrap();
    assert!(!has_session(&client, &user.with_device(7).to_protocol_address()).await);
    client.signal_shutdown_sync();
}

/// A crash while waiting for the drain permit must still leave the previous
/// raw_id in durable storage, so replay knows companion cleanup is required.
#[tokio::test]
async fn offline_device_add_does_not_publish_raw_id_before_durable_session_retirement() {
    use crate::test_utils::{create_iq_test_client_with_runtime, create_test_backend};
    use wacore::stanza::devices::{DeviceElement, KeyIndexInfo};
    let (waiting, permit_wait) = async_channel::unbounded();
    let (client, _) = create_iq_test_client_with_runtime(
        create_test_backend().await,
        Arc::new(PermitWaitRuntime(waiting)),
    )
    .await;
    let user = Jid::pn("12025550197");
    crate::test_utils::seed_adv_primary(&client, &user).await;
    seed_fresh_session(&client, &user.with_device(7)).await;
    client
        .update_device_list(DeviceListRecord {
            user: user.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
            raw_id: Some(2),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    client.flush_signal_cache_batch_safe().await.unwrap();
    let backend = client.persistence_manager.backend();
    let address = user.with_device(7).to_protocol_address();
    assert!(
        backend
            .get_session(address.as_str())
            .await
            .unwrap()
            .is_some()
    );
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let add = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move {
            client
                .patch_device_add(
                    &user.user,
                    &DeviceElement {
                        jid: user.with_device(7),
                        key_index: Some(1),
                        lid: None,
                    },
                    Some(&KeyIndexInfo {
                        timestamp: 100,
                        signed_bytes: Some(signed_key_index_bytes(vec![1], 1)),
                    }),
                )
                .await;
        })
    };
    tokio::time::timeout(Duration::from_secs(5), permit_wait.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        backend
            .get_devices(&user.user)
            .await
            .unwrap()
            .unwrap()
            .raw_id,
        Some(2)
    );
    assert!(
        backend
            .get_session(address.as_str())
            .await
            .unwrap()
            .is_some()
    );
    drop(permit);
    tokio::time::timeout(Duration::from_secs(5), add)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        backend
            .get_devices(&user.user)
            .await
            .unwrap()
            .unwrap()
            .raw_id,
        Some(1)
    );
    assert!(
        backend
            .get_session(address.as_str())
            .await
            .unwrap()
            .is_none()
    );
    client.signal_shutdown_sync();
}

/// Storage round trips stay bounded for cold groups while either direction's
/// durable history still forces admission before publishing the mapping.
#[tokio::test]
async fn mapping_history_uses_batches_for_both_cold_and_persisted_aliases() {
    use crate::store::release::tests::ProbeBackend;
    use wacore::usync::UsyncLidMapping;
    let mappings: Vec<_> = (0..256)
        .map(|index| UsyncLidMapping {
            phone_number: format!("1555000{index:05}").into(),
            lid: format!("100000000{index:06}").into(),
        })
        .collect();
    for method in ["get_devices", "get_devices_batch"] {
        let (mut probe, _) = ProbeBackend::new();
        let storage = probe.backend.clone();
        let (entered, calls) = async_channel::unbounded();
        probe.entered = Some(entered);
        probe.gate_method = method;
        let (client, _) =
            crate::test_utils::create_iq_test_client_with_backend(Arc::new(probe)).await;
        while calls.try_recv().is_ok() {}
        assert!(!client.device_mapping_has_registry_history(&mappings).await);
        storage
            .update_device_list(DeviceListRecord {
                user: mappings[255].lid.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        assert!(client.device_mapping_has_registry_history(&mappings).await);
        assert_eq!(
            calls.len(),
            if method == "get_devices_batch" { 2 } else { 0 },
            "history checks must not issue individual record reads"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn group_recovery_budget_counts_requests_across_repeated_batches() {
    use wacore::usync::{UserDeviceList, UsyncDevice};
    let (client, transport) = create_iq_test_client().await;
    let users = [
        Jid::pn("12025550101"),
        Jid::pn("12025550102"),
        Jid::pn("12025550103"),
    ];
    for user in &users {
        crate::test_utils::seed_adv_primary(&client, user).await;
    }
    let signer = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let bytes = signed_list(&signer, false);
    let response_for = |users: &[Jid]| DeviceListResponse {
        device_lists: users
            .iter()
            .map(|user| UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: Some(bytes.clone()),
            })
            .collect(),
        lid_mappings: vec![],
    };
    let first = response_for(&users[..1]);
    let later = response_for(&users[1..]);
    let operation = {
        let client = client.clone();
        tokio::spawn(async move {
            let mut budget = GroupAdvRecoveryBudget {
                remaining: 2,
                attempted: std::collections::HashSet::new(),
            };
            client
                .recover_group_adv_identities(&first, &mut budget)
                .await;
            // A topology retry must not repeat a known failed member.
            client
                .recover_group_adv_identities(&first, &mut budget)
                .await;
            client
                .recover_group_adv_identities(&later, &mut budget)
                .await;
            budget
        })
    };
    let wrong_primary = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    for (round, user) in users[..2].iter().enumerate() {
        for step in 0..3 {
            let iq = decode_sent_iq(&transport, round * 3 + step).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            if step == 2 {
                let requested = iq
                    .get()
                    .get_optional_child("key")
                    .unwrap()
                    .get_optional_child("user")
                    .unwrap()
                    .attrs()
                    .jid("jid");
                assert_eq!(&requested, user);
                answer_iq(&client, &id, &primary_response(user, &wrong_primary, true)).await;
            } else {
                let requested = iq
                    .get()
                    .get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap()
                    .children()
                    .unwrap();
                assert_eq!(requested[0].attrs().jid("jid"), *user);
                answer_iq(&client, &id, &adv_wire_response(user, bytes.clone())).await;
            }
        }
    }
    let budget = tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(budget.remaining, 0);
    assert_eq!(budget.attempted.len(), 2);
    assert_eq!(
        transport.sent().len(),
        6,
        "only two eligible members may issue recovery IQs"
    );
    for user in &users {
        assert_eq!(
            client.cached_adv_account_identity(user).await.unwrap(),
            Some(
                *crate::test_utils::adv_test_account()
                    .public_key
                    .public_key_bytes()
            )
        );
    }
    client.signal_shutdown_sync();
}

/// Real installers cannot restore old sessions while a cancelled caller's
/// recovery worker saves trust; orphan companions are retired under both aliases.
#[tokio::test]
async fn ordinary_recovery_holds_both_primary_locks_through_identity_publication() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use wacore::usync::{UserDeviceList, UsyncDevice};

    let (mut probe, _) = ProbeBackend::new();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.gate_method = "after:put_identity";
    let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let phone = Jid::pn("12025550192");
    let lid = Jid::lid("100000000000192");
    client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            phone.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
        let bundle = session_bundle(crate::test_utils::adv_test_account(), 0, None);
        client
            .install_prekey_bundle_cached(
                user,
                &bundle,
                &mut client.signal_adapter(),
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )
            .await
            .unwrap();
    }
    let orphan = lid.with_device(7);
    let device = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let old_companion = session_bundle(&device, 7, Some(crate::test_utils::adv_test_account()));
    client
        .install_prekey_bundle_cached(
            &orphan,
            &old_companion,
            &mut client.signal_adapter(),
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .unwrap();
    client
        .update_device_list(DeviceListRecord {
            user: phone.user.as_str().into(),
            devices: [DeviceInfo::new(0, None)].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let response = DeviceListResponse {
        device_lists: vec![UserDeviceList {
            user: phone.clone(),
            devices: vec![UsyncDevice::new(0, None)],
            phash: None,
            key_index_bytes: Some(signed_list(&account, false)),
        }],
        lid_mappings: vec![],
    };
    let error = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap_err();
    let mut caller = Some({
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(error).await })
    });
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&phone, &account, true)).await;
    let mut installers = Vec::new();
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let (started, entered) = async_channel::unbounded();
        for user in [&phone, &lid] {
            let mut bundle = session_bundle(crate::test_utils::adv_test_account(), 0, None);
            // The SDK records this observation before a primary key IQ.
            bundle.set_identity_snapshot(Some(
                *crate::test_utils::adv_test_account()
                    .public_key
                    .public_key_bytes(),
            ));
            let client = client.clone();
            let user = user.clone();
            let started = started.clone();
            installers.push(tokio::spawn(async move {
                started.send(()).await.unwrap();
                client
                    .install_prekey_bundle_cached(
                        &user,
                        &bundle,
                        &mut client.signal_adapter(),
                        &mut rand::make_rng::<rand::rngs::StdRng>(),
                    )
                    .await
            }));
        }
        for user in [&phone, &lid] {
            let mutex = client
                .session_lock_for(user.to_protocol_address().as_str())
                .await;
            assert!(
                mutex.try_lock().is_none(),
                "primary lock must cover the durable trust write"
            );
            assert!(!has_session(&client, &user.to_protocol_address()).await);
        }
        assert!(!has_session(&client, &orphan.to_protocol_address()).await);
        let running = client.clone();
        let waiting_orphan = orphan.clone();
        let waiting_bundle = old_companion.clone();
        installers.push(tokio::spawn(async move {
            started.send(()).await.unwrap();
            running
                .install_prekey_bundle_cached(
                    &waiting_orphan,
                    &waiting_bundle,
                    &mut running.signal_adapter(),
                    &mut rand::make_rng::<rand::rngs::StdRng>(),
                )
                .await
        }));
        for _ in 0..3 {
            tokio::time::timeout(Duration::from_secs(5), entered.recv())
                .await
                .unwrap()
                .unwrap();
        }
        for _ in 0..4 {
            tokio::task::yield_now().await;
        }
        assert!(
            installers.iter().all(|task| !task.is_finished()),
            "real installers must remain blocked until both aliases publish"
        );
        if let Some(caller) = caller.take() {
            caller.abort();
            assert!(caller.await.unwrap_err().is_cancelled());
        }
        release.send(()).await.unwrap();
    }
    for installer in installers {
        assert!(
            tokio::time::timeout(Duration::from_secs(5), installer)
                .await
                .unwrap()
                .unwrap()
                .is_err(),
            "a retained primary or companion bundle must not restore old trust"
        );
    }
    // Acquiring the guards is a completion barrier for the detached worker,
    // independent of the cancelled requester's join handle.
    for user in [&phone, &lid] {
        let mutex = client
            .session_lock_for(user.to_protocol_address().as_str())
            .await;
        drop(
            tokio::time::timeout(Duration::from_secs(5), mutex.lock_arc())
                .await
                .unwrap(),
        );
    }
    for user in [&phone, &lid] {
        assert_eq!(
            client.cached_adv_account_identity(user).await.unwrap(),
            Some(*account.public_key.public_key_bytes())
        );
        let mutex = client
            .session_lock_for(user.to_protocol_address().as_str())
            .await;
        assert!(mutex.try_lock().is_some());
        assert!(!has_session(&client, &user.to_protocol_address()).await);
    }
    let record = client.load_device_record_for_jid(&phone).await.unwrap();
    assert!(
        record.timestamp > 1,
        "the detached worker must publish topology"
    );
    assert!(
        client
            .install_prekey_bundle_cached(
                &orphan,
                &old_companion,
                &mut client.signal_adapter(),
                &mut rand::make_rng::<rand::rngs::StdRng>()
            )
            .await
            .is_err(),
        "a retained old certificate must fail after rotation"
    );
    assert!(!has_session(&client, &orphan.to_protocol_address()).await);
    let fresh = session_bundle(&device, 7, Some(&account));
    client
        .install_prekey_bundle_cached(
            &orphan,
            &fresh,
            &mut client.signal_adapter(),
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .unwrap();
    assert!(has_session(&client, &orphan.to_protocol_address()).await);
    client.signal_shutdown_sync();
}

/// Mix a rotated contact, a valid contact and an omitted cached group member.
async fn check_group_rotation_after_migration(migrate: bool) {
    for recover in [true, false] {
        let (client, transport) = create_iq_test_client().await;
        let rotated = Jid::pn("12025550193");
        let valid = Jid::pn("12025550194");
        let omitted = Jid::pn("12025550195");
        for user in [&rotated, &valid, &omitted] {
            crate::test_utils::seed_adv_primary(&client, user).await;
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                    raw_id: Some(1),
                    timestamp: 1,
                    phash: None,
                })
                .await
                .unwrap();
        }
        let lid = Jid::lid("100000000000193");
        let old = client.cached_adv_account_identity(&rotated).await.unwrap();
        if migrate {
            client
                .add_lid_pn_mapping(
                    &lid.user,
                    &rotated.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
                .unwrap();
            assert_eq!(
                client.cached_adv_account_identity(&rotated).await.unwrap(),
                None
            );
            assert_eq!(client.cached_adv_account_identity(&lid).await.unwrap(), old);
        }
        let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let rotated_wire = adv_wire_response(&rotated, signed_list(&account, true));
        let valid_wire = adv_wire_response(&valid, signed_key_index_bytes(vec![1], 1));
        let users: Vec<_> = [&rotated_wire, &valid_wire]
            .iter()
            .map(|wire| {
                wire.get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap()
                    .children()
                    .unwrap()[0]
                    .clone()
            })
            .collect();
        let group_wire = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list").children(users).build()])
                .build()])
            .build();
        let caller = {
            let client = client.clone();
            let users = vec![rotated.clone(), valid.clone(), omitted.clone()];
            tokio::spawn(async move { client.refresh_group_user_devices(users).await })
        };
        for index in 0..3 {
            let iq = decode_sent_iq(&transport, index).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            answer_iq(
                &client,
                &id,
                if index == 0 {
                    &group_wire
                } else {
                    &rotated_wire
                },
            )
            .await;
        }
        let iq = decode_sent_iq(&transport, 3).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&rotated, &account, recover)).await;
        if recover {
            let iq = decode_sent_iq(&transport, 4).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            answer_iq(&client, &id, &group_wire).await;
        }
        let devices = tokio::time::timeout(Duration::from_secs(5), caller)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(devices.contains(&valid.with_device(2)));
        assert!(devices.contains(&omitted.with_device(7)));
        assert!(devices.contains(&rotated.with_device(if recover { 2 } else { 7 })));
        assert_eq!(
            client.cached_adv_account_identity(&rotated).await.unwrap(),
            if recover {
                Some(*account.public_key.public_key_bytes())
            } else if migrate {
                None
            } else {
                old
            }
        );
        if migrate {
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                if recover {
                    Some(*account.public_key.public_key_bytes())
                } else {
                    old
                }
            );
        }
        assert_eq!(transport.sent().len(), if recover { 5 } else { 4 });
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn group_refresh_recovers_rotations_and_preserves_members_on_failure() {
    check_group_rotation_after_migration(false).await;
}

#[tokio::test]
async fn group_refresh_recovers_rotated_primary_after_namespace_migration() {
    check_group_rotation_after_migration(true).await;
}

/// A valid trusted signature must not let envelope metadata rotate trust.
#[tokio::test]
async fn ordinary_list_ignores_envelope_key_without_rotating_trust() {
    for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
        let (client, transport) = create_iq_test_client().await;
        let user = Jid::pn("12025550191");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        let trusted = client.cached_adv_account_identity(&user).await.unwrap();
        let collector = Arc::new(TestEventCollector::default());
        let subscription = client.subscribe_handler(collector.clone());
        assert!(subscription.update_interest(EventInterest::of(&[EventKind::IdentityChange])));
        subscription.detach();
        let primary = seed_fresh_session(&client, &user).await;
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(
            KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                .public_key
                .public_key_bytes()
                .to_vec(),
        );
        let response = adv_wire_response(
            &user,
            waproto::codec::adv_signed_key_index_list_to_vec(&signed),
        );
        let task = {
            let client = client.clone();
            let user = user.clone();
            tokio::spawn(async move {
                client
                    .fetch_user_devices_with_freshness(vec![user], freshness)
                    .await
            })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &response).await;
        let devices = tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(devices, vec![user.clone(), user.with_device(2)]);
        assert_eq!(transport.sent().len(), 1);
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            trusted
        );
        assert!(has_session(&client, &primary).await);
        assert!(collector.events().is_empty());
        client.signal_shutdown_sync();
    }
}

/// Rotations preserve the paired identity even with colliding numeric PN/LID IDs.
#[tokio::test]
async fn ordinary_rotation_recovers_only_with_a_fresh_authenticated_primary() {
    for user in [Jid::pn("12025550190"), Jid::lid("100000000000190")] {
        for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
            for embedded_key in [false, true] {
                for outcome in [
                    Outcome::Rotated,
                    Outcome::InvalidPrekey,
                    Outcome::WrongPrimary,
                    Outcome::TopologyChanged,
                    Outcome::IdentityChanged,
                    Outcome::AliasIdentityChanged,
                ] {
                    let (client, transport) = create_iq_test_client().await;
                    // A peer's numeric ID may equal our ID in the other
                    // namespace; it must still be recoverable as a peer.
                    use wacore::store::commands::DeviceCommand;
                    let own = if user.is_pn() {
                        Jid::lid(user.user.clone())
                    } else {
                        Jid::pn(user.user.clone())
                    };
                    client
                        .persistence_manager
                        .process_command(if user.is_pn() {
                            DeviceCommand::SetLid(Some(own.clone()))
                        } else {
                            DeviceCommand::SetId(Some(own.clone()))
                        })
                        .await;
                    let own_key = *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                        .public_key
                        .public_key_bytes();
                    client
                        .signal_cache
                        .put_identity(&own.to_protocol_address(), &own_key)
                        .await;
                    let alias = if user.is_pn() {
                        Jid::lid("100000000000190")
                    } else {
                        Jid::pn("12025550190")
                    };
                    client
                        .lid_pn_cache
                        .add(&crate::lid_pn_cache::LidPnEntry::new(
                            "100000000000190".to_string(),
                            "12025550190".to_string(),
                            crate::lid_pn_cache::LearningSource::Usync,
                        ))
                        .await;
                    crate::test_utils::seed_adv_primary(&client, &user).await;
                    crate::test_utils::seed_adv_primary(&client, &alias).await;
                    let old_key = client
                        .cached_adv_account_identity(&user)
                        .await
                        .unwrap()
                        .unwrap();
                    let collector = Arc::new(TestEventCollector::default());
                    let subscription = client.subscribe_handler(collector.clone());
                    assert!(
                        subscription
                            .update_interest(EventInterest::of(&[EventKind::IdentityChange]))
                    );
                    subscription.detach();
                    client
                        .update_device_list(DeviceListRecord {
                            user: user.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(2, Some(1))].into(),
                            raw_id: Some(1),
                            timestamp: 1,
                            phash: None,
                        })
                        .await
                        .unwrap();
                    let primary_session = seed_fresh_session(&client, &user).await;
                    let companion_session = seed_fresh_session(&client, &user.with_device(2)).await;
                    let alias_primary_session = seed_fresh_session(&client, &alias).await;
                    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                    let response = adv_wire_response(&user, signed_list(&account, embedded_key));
                    let task = {
                        let client = client.clone();
                        let user = user.clone();
                        tokio::spawn(async move {
                            client
                                .fetch_user_devices_with_freshness(vec![user], freshness)
                                .await
                        })
                    };
                    for index in 0..2 {
                        let iq = decode_sent_iq(&transport, index).await;
                        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
                        answer_iq(&client, &id, &response).await;
                    }
                    let iq = decode_sent_iq(&transport, 2).await;
                    let node = iq.get();
                    assert_eq!(
                        node.attrs().optional_string("xmlns").as_deref(),
                        Some("encrypt"),
                        "user={user} freshness={freshness:?} embedded={embedded_key} outcome={outcome:?}"
                    );
                    assert_eq!(
                        node.get_optional_child("key")
                            .unwrap()
                            .get_optional_child("user")
                            .unwrap()
                            .attrs()
                            .jid("jid"),
                        user
                    );
                    assert_eq!(
                        client.cached_adv_account_identity(&user).await.unwrap(),
                        Some(old_key)
                    );
                    assert!(has_session(&client, &primary_session).await);
                    assert!(has_session(&client, &companion_session).await);
                    let mut expected_key = old_key;
                    let mut expected_alias_key = old_key;
                    match outcome {
                        Outcome::TopologyChanged => {
                            client.invalidate_device_cache(&user.user).await
                        }
                        Outcome::IdentityChanged => {
                            expected_key =
                                *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                                    .public_key
                                    .public_key_bytes();
                            client
                                .signal_cache
                                .put_identity(&user.to_protocol_address(), &expected_key)
                                .await;
                        }
                        Outcome::AliasIdentityChanged => {
                            expected_alias_key =
                                *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                                    .public_key
                                    .public_key_bytes();
                            client
                                .signal_cache
                                .put_identity(&alias.to_protocol_address(), &expected_alias_key)
                                .await;
                        }
                        _ => {}
                    }
                    let different = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                    let key = if matches!(outcome, Outcome::WrongPrimary) {
                        &different
                    } else {
                        &account
                    };
                    let prekey =
                        primary_response(&user, key, !matches!(outcome, Outcome::InvalidPrekey));
                    let id = node.attrs().optional_string("id").unwrap().into_owned();
                    answer_iq(&client, &id, &prekey).await;
                    let result = tokio::time::timeout(Duration::from_secs(5), task)
                        .await
                        .unwrap()
                        .unwrap();
                    if matches!(outcome, Outcome::Rotated) {
                        assert_eq!(result.unwrap(), vec![user.clone(), user.with_device(2)]);
                        assert_eq!(
                            client.cached_adv_account_identity(&user).await.unwrap(),
                            Some(*account.public_key.public_key_bytes())
                        );
                        assert!(!has_session(&client, &primary_session).await);
                        assert!(!has_session(&client, &alias_primary_session).await);
                        assert!(!has_session(&client, &companion_session).await);
                        assert_eq!(
                            client.cached_adv_account_identity(&alias).await.unwrap(),
                            Some(*account.public_key.public_key_bytes())
                        );
                        assert!(collector.events().iter().any(|event| matches!(&**event,
                        Event::IdentityChange(change) if change.implicit)));
                    } else {
                        assert!(result.is_err(), "{outcome:?} must fail closed");
                        assert_eq!(
                            client.cached_adv_account_identity(&user).await.unwrap(),
                            Some(expected_key)
                        );
                        assert!(has_session(&client, &primary_session).await);
                        assert!(has_session(&client, &alias_primary_session).await);
                        assert!(has_session(&client, &companion_session).await);
                        assert_eq!(
                            client.cached_adv_account_identity(&alias).await.unwrap(),
                            Some(expected_alias_key)
                        );
                        assert!(collector.events().is_empty());
                    }
                    assert_eq!(transport.sent().len(), 3, "recovery must be bounded");
                    assert_eq!(
                        client.cached_adv_account_identity(&own).await.unwrap(),
                        Some(own_key)
                    );
                    client.signal_shutdown_sync();
                }
            }
        }
    }
}

/// A learned PN/LID mapping must preserve the local primary trust anchor.
#[tokio::test]
async fn prepared_primary_survives_known_namespace_migration() {
    use crate::lid_pn_cache::LearningSource;
    use wacore::usync::{UserDeviceList, UsyncDevice};
    for migrate in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let pn = Jid::pn("12025550123");
        let lid = Jid::lid("100000000000123");
        crate::test_utils::seed_adv_primary(&client, &pn).await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: pn.clone(),
                devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(1))],
                phash: None,
                key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
            }],
            lid_mappings: vec![],
        };
        let anchors = client
            .prepare_device_list_identities(&response, DeviceListApplyMode::StrictRefresh)
            .await
            .unwrap();
        let prepared = client.cached_adv_account_identity(&pn).await.unwrap();
        if migrate {
            client
                .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
                .await
                .unwrap();
            assert_eq!(client.cached_adv_account_identity(&pn).await.unwrap(), None);
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                prepared
            );
        }
        let result = client
            .apply_device_list_response_inner(
                &response,
                DeviceListApplyMode::StrictRefresh,
                None,
                None,
                DeviceListIdentityContext {
                    prepared: &anchors,
                    recovered: None,
                    publication_started: None,
                    authentication_epoch: None,
                    reactions: None,
                },
                None,
            )
            .await;
        let devices = result.unwrap().unwrap();
        assert_migration_publication(&client, &pn, &devices).await;
        client.signal_shutdown_sync();
    }
}

fn migration_response(user: &Jid) -> DeviceListResponse {
    use wacore::usync::{UserDeviceList, UsyncDevice};
    DeviceListResponse {
        device_lists: vec![UserDeviceList {
            user: user.clone(),
            devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(1))],
            phash: None,
            key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
        }],
        lid_mappings: vec![],
    }
}

/// Check the returned view and the registry independently of a successful status.
async fn assert_migration_publication(client: &Client, user: &Jid, devices: &[Jid]) {
    assert_eq!(devices, [user.clone(), user.with_device(7)]);
    let record = client.load_device_record_for_jid(user).await.unwrap();
    let ids: Vec<_> = record
        .devices
        .iter()
        .map(|device| device.device_id())
        .collect();
    assert_eq!(ids, [0, 7]);
}

/// Prepared trust is usable only while every current primary namespace still agrees.
#[tokio::test]
async fn prepared_primary_rejects_missing_changed_and_conflicting_identity() {
    use crate::lid_pn_cache::LearningSource;
    for case in ["unknown-alias", "missing", "changed", "conflict"] {
        let (client, _) = create_iq_test_client().await;
        let pn = Jid::pn("12025550123");
        let lid = Jid::lid("100000000000123");
        let response = migration_response(&pn);
        crate::test_utils::seed_adv_primary(&client, &pn).await;
        let prepared = client
            .prepare_device_list_identities(&response, DeviceListApplyMode::StrictRefresh)
            .await
            .unwrap();
        if case == "unknown-alias" {
            client.migrate_signal_sessions(&pn, &lid).await;
        } else if case == "missing" {
            client
                .signal_cache
                .delete_identity(&pn.to_protocol_address())
                .await;
        } else {
            client
                .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
                .await
                .unwrap();
            client
                .signal_cache
                .put_identity(&lid.to_protocol_address(), &[42; 32])
                .await;
            if case == "conflict" {
                crate::test_utils::seed_adv_primary(&client, &pn).await;
            }
        }
        assert!(
            client
                .apply_device_list_response_inner(
                    &response,
                    DeviceListApplyMode::StrictRefresh,
                    None,
                    None,
                    DeviceListIdentityContext {
                        prepared: &prepared,
                        recovered: None,
                        publication_started: None,
                        authentication_epoch: None,
                        reactions: None,
                    },
                    None,
                )
                .await
                .is_err(),
            "{case}"
        );
        assert!(
            client.load_device_record_for_jid(&pn).await.is_none(),
            "{case}"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn migrated_primary_survives_cache_reload_and_client_reopen() {
    use crate::lid_pn_cache::LearningSource;
    use crate::test_utils::{create_iq_test_client_with_backend, create_test_backend};
    let backend = create_test_backend().await;
    let (client, _) = create_iq_test_client_with_backend(backend.clone()).await;
    let pn = Jid::pn("12025550123");
    let lid = Jid::lid("100000000000123");
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    client
        .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
        .await
        .unwrap();
    client.signal_cache.flush(backend.as_ref()).await.unwrap();
    client.signal_cache.clear().await;
    let response = migration_response(&pn);
    let devices = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap();
    assert_migration_publication(&client, &pn, &devices).await;
    client.signal_shutdown_sync();
    // Reopen with durable trust/mapping but no prior topology to reuse.
    for user in [&pn, &lid] {
        backend.delete_devices(&user.user).await.unwrap();
    }
    let (reopened, _) = create_iq_test_client_with_backend(backend.clone()).await;
    reopened.warm_up_lid_pn_cache().await.unwrap();
    assert!(reopened.load_device_record_for_jid(&pn).await.is_none());
    let devices = reopened
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap();
    assert_migration_publication(&reopened, &pn, &devices).await;
    assert_eq!(
        reopened.cached_adv_account_identity(&pn).await.unwrap(),
        None
    );
    reopened.signal_shutdown_sync();
}

/// The registry wait is a deterministic pause after primary/mapping locks.
#[tokio::test]
async fn primary_migration_waits_until_adv_publication_finishes() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    use std::task::Poll;

    let (client, _) = create_iq_test_client().await;
    assert!(client.lid_pn_cache.startup_finished_for_tests());
    let pn = Jid::pn("12025550123");
    let lid = Jid::lid("100000000000123");
    client
        .lid_pn_cache
        .add(&LidPnEntry::new(
            lid.user.as_str(),
            pn.user.as_str(),
            LearningSource::Usync,
        ))
        .await;
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    let response = migration_response(&pn);
    let mut addresses = [pn.clone(), lid.clone()];
    addresses.sort_unstable_by(wacore::types::jid::cmp_for_lock_order);
    let first = client
        .session_lock_for(addresses[0].to_protocol_address().as_str())
        .await;
    let second = client
        .session_lock_for(addresses[1].to_protocol_address().as_str())
        .await;
    // Force an early suspension before the worker owns both primary locks.
    let blocked = first.lock_arc().await;
    let registry = client.device_topology.lock_registry().await;
    let publication = client.process_device_list_response(&response, Freshness::Refresh);
    futures::pin_mut!(publication);
    assert!(futures::poll!(publication.as_mut()).is_pending());
    assert!(second.try_lock().is_some());
    drop(blocked);
    // Registry keeps this complete guard set stable. The requester's first
    // Pending can precede its owned worker, so observe the actual boundary.
    tokio::time::timeout(
        Duration::from_secs(5),
        futures::future::poll_fn(|cx| {
            assert!(publication.as_mut().poll(cx).is_pending());
            if first.try_lock().is_none()
                && second.try_lock().is_none()
                && client.lid_pn_cache.try_lock_mutation().is_none()
            {
                return Poll::Ready(());
            }
            cx.waker().wake_by_ref();
            Poll::Pending
        }),
    )
    .await
    .expect("publication must own both primary locks and Map before migration");
    let migration = client.migrate_signal_sessions(&pn, &lid);
    futures::pin_mut!(migration);
    assert!(futures::poll!(migration.as_mut()).is_pending());
    drop(registry);
    let (publication_result, _) = tokio::time::timeout(Duration::from_secs(5), async {
        futures::join!(publication, migration)
    })
    .await
    .expect("publication and migration must both finish after Registry is released");
    assert_migration_publication(&client, &pn, &publication_result.unwrap()).await;
    assert!(
        client
            .cached_adv_account_identity(&pn)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        client
            .cached_adv_account_identity(&lid)
            .await
            .unwrap()
            .is_some()
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn stale_reverse_alias_cannot_supply_an_adv_primary() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    let (client, _) = create_iq_test_client().await;
    let pn = Jid::pn("12025550123");
    let old_lid = Jid::lid("100000000000123");
    let new_lid = Jid::lid("100000000000124");
    for lid in [&old_lid, &new_lid] {
        client
            .lid_pn_cache
            .add(&LidPnEntry::new(
                lid.user.as_str(),
                pn.user.as_str(),
                LearningSource::Usync,
            ))
            .await;
    }
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    let error = client
        .prepare_device_list_identities(
            &migration_response(&old_lid),
            DeviceListApplyMode::StrictRefresh,
        )
        .await
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("conflicting account alias"),
        "{error:#}"
    );
    client.signal_shutdown_sync();
}

/// Reject one contact's trust conflict while valid and omitted peers keep fanout.
#[tokio::test]
async fn group_identity_conflicts_retain_only_the_affected_contact() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    for case in [
        "conflict",
        "nonreciprocal",
        "changed-after-prepare",
        "conflict-after-prepare",
        "repaired-after-prepare",
        "malformed",
    ] {
        for group in [false, true] {
            if case == "repaired-after-prepare" && !group {
                continue;
            }
            let (client, transport) = create_iq_test_client().await;
            let pn = Jid::pn("12025550193");
            let lid = Jid::lid("100000000000193");
            client
                .lid_pn_cache
                .add(&LidPnEntry::new(
                    lid.user.as_str(),
                    pn.user.as_str(),
                    LearningSource::Usync,
                ))
                .await;
            let bad = if case == "nonreciprocal" {
                lid.clone()
            } else {
                pn.clone()
            };
            let good = Jid::pn("12025550194");
            let omitted = Jid::pn("12025550195");
            for user in [&pn, &lid, &good, &omitted] {
                crate::test_utils::seed_adv_primary(&client, user).await;
            }
            for user in [&bad, &good, &omitted] {
                client
                    .update_device_list(DeviceListRecord {
                        user: user.user.as_str().into(),
                        devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                        raw_id: Some(1),
                        timestamp: 1,
                        phash: None,
                    })
                    .await
                    .unwrap();
            }
            let mut response = migration_response(&bad);
            let prior_bad_session = seed_fresh_session(&client, &bad.with_device(3)).await;
            response
                .device_lists
                .extend(migration_response(&good).device_lists);
            let after = case.ends_with("after-prepare");
            if case == "repaired-after-prepare" {
                client
                    .signal_cache
                    .put_identity(&lid.to_protocol_address(), &[42; 32])
                    .await;
            }
            let prepared = if after {
                Some(
                    client
                        .prepare_device_list_identities(
                            &response,
                            if group {
                                DeviceListApplyMode::GroupRefresh
                            } else {
                                DeviceListApplyMode::StrictRefresh
                            },
                        )
                        .await
                        .unwrap(),
                )
            } else {
                None
            };
            if case == "nonreciprocal" {
                client
                    .lid_pn_cache
                    .add(&LidPnEntry::new(
                        "100000000000194",
                        pn.user.as_str(),
                        LearningSource::Usync,
                    ))
                    .await;
            } else if case == "repaired-after-prepare" {
                crate::test_utils::seed_adv_primary(&client, &lid).await;
            } else if case == "malformed" {
                client
                    .signal_cache
                    .put_identity(&lid.to_protocol_address(), &[42; 31])
                    .await;
            } else {
                client
                    .signal_cache
                    .put_identity(&lid.to_protocol_address(), &[42; 32])
                    .await;
                if case == "changed-after-prepare" {
                    client
                        .signal_cache
                        .put_identity(&pn.to_protocol_address(), &[42; 32])
                        .await;
                }
            }
            let users = [bad.clone(), good.clone(), omitted.clone()];
            let result = if let Some(prepared) = &prepared {
                // The public wrapper obtains this on a removal/cleanup retry.
                let maintenance = client.begin_signal_maintenance().await.unwrap();
                client
                    .apply_device_list_response_inner(
                        &response,
                        if group {
                            DeviceListApplyMode::GroupRefresh
                        } else {
                            DeviceListApplyMode::StrictRefresh
                        },
                        None,
                        group.then_some(users.as_slice()),
                        DeviceListIdentityContext {
                            prepared,
                            recovered: None,
                            publication_started: None,
                            authentication_epoch: None,
                            reactions: None,
                        },
                        Some(&maintenance),
                    )
                    .await
            } else if group {
                client
                    .try_process_group_device_list_response(
                        &response,
                        &users,
                        client.device_topology.current(),
                    )
                    .await
            } else {
                client
                    .apply_device_list_response(
                        &response,
                        DeviceListApplyMode::StrictRefresh,
                        None,
                        None,
                    )
                    .await
            };
            let partial = group && case != "malformed";
            if partial {
                let devices = result
                    .unwrap_or_else(|error| panic!("{case}: {error:#}"))
                    .unwrap();
                assert!(devices.contains(&good.with_device(7)), "{case}");
                assert!(devices.contains(&bad.with_device(3)), "{case}");
                assert!(devices.contains(&omitted.with_device(3)), "{case}");
                assert!(!devices.contains(&bad.with_device(7)), "{case}");
            } else {
                assert!(result.is_err(), "{case}");
            }
            for (user, expected) in [
                (&bad, 3),
                (&good, if partial { 7 } else { 3 }),
                (&omitted, 3),
            ] {
                let record = client.load_device_record_for_jid(user).await.unwrap();
                let ids: Vec<_> = record
                    .devices
                    .iter()
                    .map(|device| device.device_id())
                    .collect();
                assert_eq!(ids, [0, expected], "{case}, group={group}, user={user}");
            }
            assert!(
                transport.sent().is_empty(),
                "conflicts must not spend recovery IQs"
            );
            assert!(
                has_session(&client, &prior_bad_session).await,
                "{case}: rejected contact must retain its session"
            );
            client.signal_shutdown_sync();
        }
    }
}

/// A cold contact's alias conflict must not prevent valid peers from publishing.
#[tokio::test]
async fn group_refresh_publishes_peers_after_a_cold_bootstrap_alias_conflict() {
    let (client, transport) = create_iq_test_client().await;
    let cold = Jid::pn("12025550195");
    let good = Jid::pn("12025550196");
    let lid = Jid::lid("100000000000195");
    crate::test_utils::seed_adv_primary(&client, &good).await;
    for user in [&cold, &good] {
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
    }
    let mut response = migration_response(&cold);
    response
        .device_lists
        .extend(migration_response(&good).device_lists);
    let users = [cold.clone(), good.clone()];
    let worker = client.clone();
    let operation = tokio::spawn(async move {
        worker
            .apply_device_list_response(
                &response,
                DeviceListApplyMode::GroupRefresh,
                None,
                Some(&users),
            )
            .await
    });
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    client
        .lid_pn_cache
        .add(&crate::lid_pn_cache::LidPnEntry::new(
            lid.user.to_string(),
            cold.user.to_string(),
            crate::lid_pn_cache::LearningSource::Usync,
        ))
        .await;
    client.lid_pn_cache.evict_reverse_for_tests(&lid.user).await;
    answer_iq(
        &client,
        &id,
        &primary_response(&cold, crate::test_utils::adv_test_account(), true),
    )
    .await;
    let devices = tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .unwrap()
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(devices.contains(&good.with_device(7)), "{devices:?}");
    assert!(!devices.contains(&cold.with_device(7)));
    assert!(client.load_device_record_for_jid(&good).await.is_some());
    assert!(
        client
            .load_device_record_for_jid(&cold)
            .await
            .unwrap()
            .devices
            .iter()
            .any(|device| device.device_id() == 3)
    );
    assert_eq!(transport.sent_count(), 1);
    client.signal_shutdown_sync();
}

/// A store fault must not masquerade as a per-contact trust rejection.
#[tokio::test]
async fn group_identity_storage_failure_is_not_partial_success() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    let failing = Arc::new(AtomicBool::new(false));
    let (mut probe, _) = ProbeBackend::new();
    probe.fail_before_method = Some(("load_identity", failing.clone()));
    let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let users = [Jid::pn("12025550193"), Jid::pn("12025550194")];
    for user in &users {
        crate::test_utils::seed_adv_primary(&client, user).await;
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
    }
    client
        .signal_cache
        .flush(client.persistence_manager.backend().as_ref())
        .await
        .unwrap();
    client.signal_cache.clear().await;
    let mut response = migration_response(&users[0]);
    response
        .device_lists
        .extend(migration_response(&users[1]).device_lists);
    let prepared = client
        .prepare_device_list_identities(&response, DeviceListApplyMode::GroupRefresh)
        .await
        .unwrap();
    client.signal_cache.clear().await;
    failing.store(true, Ordering::SeqCst);
    let error = client
        .try_process_group_device_list_response(&response, &users, client.device_topology.current())
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("synthetic failure before commit"));
    let error = client
        .apply_device_list_response_inner(
            &response,
            DeviceListApplyMode::GroupRefresh,
            Some(client.device_topology.current()),
            Some(&users),
            DeviceListIdentityContext {
                prepared: &prepared,
                recovered: None,
                publication_started: None,
                authentication_epoch: None,
                reactions: None,
            },
            None,
        )
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("synthetic failure before commit"));
    for user in &users {
        let record = client.load_device_record_for_jid(user).await.unwrap();
        assert_eq!(
            record
                .devices
                .iter()
                .map(|device| device.device_id())
                .collect::<Vec<_>>(),
            [0, 3]
        );
    }
    assert!(transport.sent().is_empty());
    failing.store(false, Ordering::SeqCst);
    client.signal_shutdown_sync();
}

/// Cancellation must release partially acquired session locks and the complete
/// alias guard set while publication is waiting for the registry.
#[tokio::test]
async fn migrated_primary_cancellation_releases_adv_guards() {
    use crate::lid_pn_cache::LearningSource;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (pending, waiting) = async_channel::unbounded();
    let (completed, finished) = async_channel::unbounded();
    let runtime = Arc::new(BootstrapWorkerRuntime {
        armed: AtomicBool::new(false),
        pending,
        completed,
    });
    let (client, _) = crate::test_utils::create_iq_test_client_with_runtime(
        crate::test_utils::create_test_backend().await,
        runtime.clone(),
    )
    .await;
    let pn = Jid::pn("12025550123");
    let lid = Jid::lid("100000000000123");
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    client
        .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
        .await
        .unwrap();
    let anchor = client.cached_adv_account_identity(&lid).await.unwrap();
    let mut addresses = [pn.clone(), lid.clone()];
    addresses.sort_unstable_by(wacore::types::jid::cmp_for_lock_order);
    let first = client
        .session_lock_for(addresses[0].to_protocol_address().as_str())
        .await;
    let second = client
        .session_lock_for(addresses[1].to_protocol_address().as_str())
        .await;
    let blocked = second.lock_arc().await;
    let response = migration_response(&pn);
    let mut preparation = Box::pin(
        client.prepare_device_list_identities(&response, DeviceListApplyMode::StrictRefresh),
    );
    assert!(futures::poll!(preparation.as_mut()).is_pending());
    assert!(first.try_lock().is_none());
    drop(preparation);
    assert!(first.try_lock().is_some());
    drop(blocked);

    let registry = client.device_topology.lock_registry().await;
    runtime.armed.store(true, Ordering::SeqCst);
    let mut publication =
        Box::pin(client.process_device_list_response(&response, Freshness::Refresh));
    assert!(futures::poll!(publication.as_mut()).is_pending());
    tokio::time::timeout(Duration::from_secs(5), waiting.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(first.try_lock().is_none());
    assert!(second.try_lock().is_none());
    drop(publication);
    // The owned task must observe cancellation and release its partial guard
    // set BEFORE Registry is released, without publishing a new device list.
    tokio::time::timeout(Duration::from_secs(5), finished.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(first.try_lock().is_some());
    assert!(second.try_lock().is_some());
    drop(
        tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
            .await
            .unwrap(),
    );
    drop(registry);
    assert!(client.load_device_record_for_jid(&pn).await.is_none());
    assert_eq!(
        client.cached_adv_account_identity(&lid).await.unwrap(),
        anchor
    );
    assert_eq!(client.cached_adv_account_identity(&pn).await.unwrap(), None);
    let devices = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap();
    assert_migration_publication(&client, &pn, &devices).await;
    client.signal_shutdown_sync();
}

/// A companion's authenticated IQ cannot replace an absent alias behind the
/// publication check, even when its caller already owns a companion session.
#[tokio::test]
async fn companion_bootstrap_waits_for_publication_and_checks_the_known_alias() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    for conflicting in [true, false] {
        let (mut probe, _) = ProbeBackend::new();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate_method = "update_device_lists";
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let pn = Jid::pn("12025550123");
        let lid = Jid::lid("100000000000123");
        client
            .add_lid_pn_mapping(
                &lid.user,
                &pn.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        crate::test_utils::seed_adv_primary(&client, &pn).await;
        let old = client.cached_adv_account_identity(&pn).await.unwrap();
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            None
        );
        let response = migration_response(&pn);
        let mut publication =
            Box::pin(client.process_device_list_response(&response, Freshness::Refresh));
        assert!(futures::poll!(publication.as_mut()).is_pending());
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let companion = lid.with_device(7);
        let companion_mutex = client
            .session_lock_for(companion.to_protocol_address().as_str())
            .await;
        let _companion_guard = companion_mutex.lock_arc().await;
        let requested = [companion.clone()];
        let mut bootstrap = Box::pin(client.fetch_pre_keys(&requested, None));
        assert!(futures::poll!(bootstrap.as_mut()).is_pending());
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        let changed = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let account = if conflicting {
            &changed
        } else {
            crate::test_utils::adv_test_account()
        };
        let primary_wire = primary_response(&lid, account, true);
        let companion_wire = primary_response(&companion, account, true);
        let primary_user = primary_wire
            .get_optional_child("list")
            .unwrap()
            .children()
            .unwrap()[0]
            .clone();
        let companion_user = &companion_wire
            .get_optional_child("list")
            .unwrap()
            .children()
            .unwrap()[0];
        let certificate = waproto::codec::adv_signed_device_identity_to_vec(
            &wacore::adv::test_util::signed_identity(
                account,
                account,
                b"\x18\x01",
                &[6, 0],
                Some(&[6, 1]),
                true,
            ),
        );
        let mut fields = companion_user.children().unwrap().to_vec();
        fields.push(
            NodeBuilder::new("device-identity")
                .bytes(certificate)
                .build(),
        );
        let companion_user = NodeBuilder::new("user")
            .attr("jid", companion.to_string())
            .children(fields)
            .build();
        let answer = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("list")
                .children([primary_user, companion_user])
                .build()])
            .build();
        answer_iq(&client, &id, &answer).await;
        assert!(
            futures::poll!(bootstrap.as_mut()).is_pending(),
            "conflicting={conflicting}: bootstrap must wait through publication"
        );
        release.send(()).await.unwrap();
        let devices = publication.await.unwrap();
        assert_migration_publication(&client, &pn, &devices).await;
        let outcome = bootstrap.await;
        if conflicting {
            assert!(
                outcome
                    .err()
                    .unwrap()
                    .to_string()
                    .contains("primary identity changed during companion bootstrap")
            );
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                None
            );
        } else {
            assert!(outcome.unwrap().bundles.contains_key(&companion));
            assert_eq!(client.cached_adv_account_identity(&lid).await.unwrap(), old);
        }
        assert_eq!(client.cached_adv_account_identity(&pn).await.unwrap(), old);
        client.signal_shutdown_sync();
    }
}

/// A registry persistence rejection after real migration must preserve the
/// current anchor, release all newly held guards and permit an unchanged retry.
#[tokio::test]
async fn migrated_primary_registry_failure_preserves_anchor_and_allows_retry() {
    use crate::lid_pn_cache::LearningSource;
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    let failing = Arc::new(AtomicBool::new(false));
    let (mut probe, _) = ProbeBackend::new();
    probe.fail_before_method = Some(("update_device_lists", failing.clone()));
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let pn = Jid::pn("12025550123");
    let lid = Jid::lid("100000000000123");
    crate::test_utils::seed_adv_primary(&client, &pn).await;
    client
        .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Usync)
        .await
        .unwrap();
    let anchor = client.cached_adv_account_identity(&lid).await.unwrap();
    let response = migration_response(&pn);
    failing.store(true, Ordering::SeqCst);
    let error = client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .expect_err("the injected registry write rejection must surface");
    assert!(format!("{error:#}").contains("synthetic failure before commit"));
    // Existing registry semantics publish the already-verified cache row before
    // the durable write; distinguish that from a committed backend record.
    assert!(client.load_device_record_for_jid(&pn).await.is_some());
    for user in [&pn, &lid] {
        assert!(
            client
                .persistence_manager
                .backend()
                .get_devices(&user.user)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        client.cached_adv_account_identity(&lid).await.unwrap(),
        anchor
    );
    assert_eq!(client.cached_adv_account_identity(&pn).await.unwrap(), None);
    for user in [&pn, &lid] {
        assert!(
            client
                .session_lock_for(user.to_protocol_address().as_str())
                .await
                .try_lock()
                .is_some()
        );
    }
    drop(
        tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
            .await
            .unwrap(),
    );
    drop(
        tokio::time::timeout(
            Duration::from_secs(5),
            client.device_topology.lock_registry(),
        )
        .await
        .unwrap(),
    );
    failing.store(false, Ordering::SeqCst);
    client
        .process_device_list_response(&response, Freshness::Refresh)
        .await
        .unwrap();
    assert!(client.load_device_record_for_jid(&pn).await.is_some());
    assert!(
        client
            .persistence_manager
            .backend()
            .get_devices(&lid.user)
            .await
            .unwrap()
            .is_some()
    );
    client.signal_shutdown_sync();
}

/// A failed write leaves its dirty anchor pinned. Cancellation of the waiter
/// retains ordering until the owned writer receives storage acknowledgement.
#[tokio::test]
async fn bootstrap_fence_failure_and_cancellation_keep_the_pinned_anchor() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    for cancel in [false, true] {
        let failing = Arc::new(AtomicBool::new(!cancel));
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        probe.fail_before_method = Some(("put_identity", failing.clone()));
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        if cancel {
            probe.gate_method = "put_identity";
            probe.gate = Some(gate);
            probe.entered = Some(entered);
        }
        let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let pn = Jid::pn("12025550183");
        let lid = Jid::lid("100000000000183");
        client
            .add_lid_pn_mapping(
                &lid.user,
                &pn.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        let key = *crate::test_utils::adv_test_account()
            .public_key
            .public_key_bytes();
        if cancel {
            let worker = {
                let client = client.clone();
                let lid = lid.clone();
                tokio::spawn(async move { client.persist_adv_bootstrap_identity(&lid, &key).await })
            };
            tokio::time::timeout(Duration::from_secs(5), arrived.recv())
                .await
                .unwrap()
                .unwrap();
            worker.abort();
            assert!(worker.await.unwrap_err().is_cancelled());
            assert!(client.lid_pn_cache.try_lock_mutation().is_none());
            release.send(()).await.unwrap();
            drop(
                tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
                    .await
                    .unwrap(),
            );
        } else {
            assert!(
                client
                    .persist_adv_bootstrap_identity(&lid, &key)
                    .await
                    .is_err()
            );
        }
        assert!(client.lid_pn_cache.try_lock_mutation().is_some());
        drop(
            tokio::time::timeout(Duration::from_secs(5), client.lid_pn_cache.lock_mutation())
                .await
                .unwrap(),
        );
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            Some(key)
        );
        assert_eq!(
            raw.load_identity(lid.to_protocol_address().as_str())
                .await
                .unwrap(),
            cancel.then_some(key)
        );
        let other = *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
            .public_key
            .public_key_bytes();
        assert!(
            !client
                .persist_adv_bootstrap_identity(&lid, &other)
                .await
                .unwrap()
        );
        failing.store(false, Ordering::SeqCst);
        if cancel {
            release.send(()).await.unwrap();
        }
        assert!(
            client
                .persist_adv_bootstrap_identity(&lid, &key)
                .await
                .unwrap()
        );
        assert_eq!(
            raw.load_identity(lid.to_protocol_address().as_str())
                .await
                .unwrap(),
            Some(key)
        );
        client.signal_shutdown_sync();
    }
}

/// One batch cannot return old ordinary devices after replacing their account
/// trust through a hosted alias, regardless of response order.
#[tokio::test]
async fn mixed_hosted_and_ordinary_alias_lists_cannot_cross_trust_epochs() {
    use wacore::usync::{UserDeviceList, UsyncDevice};
    for same_epoch in [false, true] {
        for group in [false, true] {
            for hosted_first in [false, true] {
                let (client, _) = create_iq_test_client().await;
                let pn = Jid::pn("12025550172");
                let lid = Jid::lid("100000000000172");
                let peer = Jid::pn("12025550173");
                client
                    .add_lid_pn_mapping(
                        &lid.user,
                        &pn.user,
                        crate::lid_pn_cache::LearningSource::Usync,
                    )
                    .await
                    .unwrap();
                let previous = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                let account = if same_epoch {
                    crate::test_utils::adv_test_account()
                } else {
                    &previous
                };
                let old = *account.public_key.public_key_bytes();
                for user in [&pn, &lid] {
                    client
                        .signal_cache
                        .put_identity(&user.to_protocol_address(), &old)
                        .await;
                }
                crate::test_utils::seed_adv_primary(&client, &peer).await;
                for user in [&pn, &peer] {
                    client
                        .update_device_list(DeviceListRecord {
                            user: user.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                            raw_id: Some(1),
                            timestamp: 1,
                            phash: None,
                        })
                        .await
                        .unwrap();
                }
                let ordinary = UserDeviceList {
                    user: pn.clone(),
                    devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(1))],
                    phash: None,
                    key_index_bytes: Some(signed_list(account, false)),
                };
                let hosted = UserDeviceList {
                    user: lid.clone(),
                    devices: vec![
                        UsyncDevice::new(0, None),
                        UsyncDevice::new(99, Some(1)).with_hosting(true),
                    ],
                    phash: None,
                    key_index_bytes: Some(signed_list(crate::test_utils::adv_test_account(), true)),
                };
                let mut response = DeviceListResponse {
                    device_lists: if hosted_first {
                        vec![hosted, ordinary]
                    } else {
                        vec![ordinary, hosted]
                    },
                    lid_mappings: vec![],
                };
                response
                    .device_lists
                    .extend(migration_response(&peer).device_lists);
                let result = if group {
                    client
                        .try_process_group_device_list_response(
                            &response,
                            &[pn.clone(), lid.clone(), peer.clone()],
                            client.device_topology.current(),
                        )
                        .await
                } else {
                    client
                        .apply_device_list_response(
                            &response,
                            DeviceListApplyMode::StrictRefresh,
                            None,
                            None,
                        )
                        .await
                };
                if same_epoch {
                    let devices = result.unwrap().unwrap();
                    assert!(devices.contains(&peer.with_device(7)));
                    let selected = if hosted_first { 7 } else { 99 };
                    for user in [&pn, &lid] {
                        let record = client.load_device_record_for_jid(user).await.unwrap();
                        assert_eq!(
                            record
                                .devices
                                .iter()
                                .map(|device| device.device_id())
                                .collect::<Vec<_>>(),
                            [0, selected]
                        );
                        assert_ne!(record.timestamp, 1);
                    }
                    assert!(
                        devices
                            .iter()
                            .any(|jid| (jid.user == pn.user || jid.user == lid.user)
                                && jid.device == selected)
                    );
                    assert!(!devices.iter().any(|jid| (jid.user == pn.user
                        || jid.user == lid.user)
                        && jid.device == 3));
                } else if group {
                    let devices = result.unwrap().unwrap();
                    assert!(devices.contains(&peer.with_device(7)));
                    assert!(devices.contains(&pn.with_device(3)));
                    assert!(
                        !devices
                            .iter()
                            .any(|jid| (jid.user == pn.user || jid.user == lid.user)
                                && (jid.device == 7 || jid.device == 99))
                    );
                    let record = client.load_device_record_for_jid(&pn).await.unwrap();
                    assert_eq!(
                        record
                            .devices
                            .iter()
                            .map(|device| device.device_id())
                            .collect::<Vec<_>>(),
                        [0, 3]
                    );
                } else {
                    assert!(result.is_err(), "hosted_first={hosted_first}");
                    assert_eq!(
                        client
                            .load_device_record_for_jid(&peer)
                            .await
                            .unwrap()
                            .timestamp,
                        1
                    );
                }
                for user in [&pn, &lid] {
                    assert_eq!(
                        client.cached_adv_account_identity(user).await.unwrap(),
                        Some(old)
                    );
                }
                client.signal_shutdown_sync();
            }
        }
    }
}

/// A Signal alias change after preflight but before the ordered bootstrap CAS
/// must be observed, rather than installing the primary using the old alias.
#[tokio::test]
async fn bootstrap_alias_is_rechecked_after_waiting_for_the_flush_order() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    for changed in [true, false] {
        let (mut probe, _) = ProbeBackend::new();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate_method = "put_sessions_batch";
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let pn = Jid::pn("12025550176");
        let lid = Jid::lid("100000000000176");
        client
            .add_lid_pn_mapping(
                &lid.user,
                &pn.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        crate::test_utils::seed_adv_primary(&client, &lid).await;
        let original = client
            .cached_adv_account_identity(&lid)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(client.cached_adv_account_identity(&pn).await.unwrap(), None);
        let backend = client.persistence_manager.backend();
        client.signal_cache.flush(backend.as_ref()).await.unwrap();
        seed_fresh_session(&client, &pn.with_device(8)).await;
        let cache = client.signal_cache.clone();
        let flushing = tokio::spawn(async move { cache.flush(backend.as_ref()).await });
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let mut bootstrap = Box::pin(client.persist_adv_bootstrap_identity(&pn, &original));
        assert!(futures::poll!(bootstrap.as_mut()).is_pending());
        // Also works when the bootstrap is admitted by an owned worker.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match client.lid_pn_cache.try_lock_mutation() {
                    None => break,
                    Some(guard) => drop(guard),
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let latest = if changed {
            *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                .public_key
                .public_key_bytes()
        } else {
            original
        };
        client
            .signal_cache
            .put_identity(&lid.to_protocol_address(), &latest)
            .await;
        release.send(()).await.unwrap();
        flushing.await.unwrap().unwrap();
        assert_eq!(bootstrap.await.unwrap(), !changed);
        assert_eq!(
            client.cached_adv_account_identity(&pn).await.unwrap(),
            (!changed).then_some(original)
        );
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            Some(latest)
        );
        client.signal_shutdown_sync();
    }
}

/// The host write outlives the cancelled requester; a distinct legitimate
/// Signal identity must wait and remain durable after that old write finishes.
#[tokio::test]
async fn cancelled_bootstrap_waits_for_outstanding_host_write_before_new_trust() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    let key = *crate::test_utils::adv_test_account()
        .public_key
        .public_key_bytes();
    let newer_key = *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
        .public_key
        .public_key_bytes();
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.detach_identity_writes = true;
    probe.identity_gate_key = Some(key);
    probe.gate_method = "put_identity";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let primary = Jid::pn("12025550177");
    let requester = {
        let client = client.clone();
        let primary = primary.clone();
        tokio::spawn(async move { client.persist_adv_bootstrap_identity(&primary, &key).await })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    requester.abort();
    assert!(requester.await.unwrap_err().is_cancelled());
    let (published, observed) = async_channel::unbounded();
    let mut newer = Box::pin(async {
        let mutex = client
            .session_lock_for(primary.to_protocol_address().as_str())
            .await;
        let _guard = mutex.lock_arc().await;
        client
            .signal_cache
            .put_identity(&primary.to_protocol_address(), &newer_key)
            .await;
        published.send(()).await.unwrap();
        client
            .signal_cache
            .flush(client.persistence_manager.backend().as_ref())
            .await
            .unwrap();
    });
    assert!(futures::poll!(newer.as_mut()).is_pending());
    assert!(
        observed.try_recv().is_err(),
        "newer trust must wait for the outstanding bootstrap write"
    );
    assert_eq!(
        raw.load_identity(primary.to_protocol_address().as_str())
            .await
            .unwrap(),
        None
    );
    release.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), newer)
        .await
        .unwrap();
    assert_eq!(
        raw.load_identity(primary.to_protocol_address().as_str())
            .await
            .unwrap(),
        Some(newer_key)
    );
    assert_eq!(
        client.cached_adv_account_identity(&primary).await.unwrap(),
        Some(newer_key)
    );
    assert!(client.lid_pn_cache.try_lock_mutation().is_some());
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn bootstrap_malformed_anchor_is_a_storage_error() {
    for alias_row in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let pn = Jid::pn("12025550178");
        let lid = Jid::lid("100000000000178");
        client
            .add_lid_pn_mapping(
                &lid.user,
                &pn.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        let target = if alias_row { &lid } else { &pn };
        client
            .signal_cache
            .put_identity(&target.to_protocol_address(), &[1; 31])
            .await;
        let key = *crate::test_utils::adv_test_account()
            .public_key
            .public_key_bytes();
        let error = client
            .persist_adv_bootstrap_identity(&pn, &key)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid stored"));
        assert!(!error.is::<DeviceListIdentityConflict>());
        client.signal_shutdown_sync();
    }
}

/// Rejecting a new cross-key relation must preserve the PN's fanout, not select
/// the unrelated LID row or dispatch its PN-wins migration.
#[tokio::test]
async fn rejected_new_mapping_does_not_rebind_the_group_contact() {
    use wacore::usync::UsyncLidMapping;
    for case in ["conflict", "unsigned", "invalid-signature", "valid"] {
        let conflict = case == "conflict";
        for group in [false, true] {
            let (client, _) = create_iq_test_client().await;
            let pn = Jid::pn("12025550174");
            let lid = Jid::lid("100000000000174");
            let peer = Jid::pn("12025550175");
            for user in [&pn, &peer] {
                crate::test_utils::seed_adv_primary(&client, user).await;
            }
            let other = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            let lid_key = if conflict {
                *other.public_key.public_key_bytes()
            } else {
                *crate::test_utils::adv_test_account()
                    .public_key
                    .public_key_bytes()
            };
            client
                .signal_cache
                .put_identity(&lid.to_protocol_address(), &lid_key)
                .await;
            let pn_key = client.cached_adv_account_identity(&pn).await.unwrap();
            for (user, device) in [(&pn, 3), (&lid, 9), (&peer, 3)] {
                client
                    .update_device_list(DeviceListRecord {
                        user: user.user.as_str().into(),
                        devices: [DeviceInfo::new(0, None), DeviceInfo::new(device, Some(1))]
                            .into(),
                        raw_id: Some(1),
                        timestamp: 1,
                        phash: None,
                    })
                    .await
                    .unwrap();
            }
            let old_session = seed_fresh_session(&client, &lid.with_device(9)).await;
            let mut response = migration_response(&pn);
            if case == "unsigned" {
                response.device_lists[0].key_index_bytes = None;
            } else if case == "invalid-signature" {
                response.device_lists[0].key_index_bytes.as_mut().unwrap()[0] ^= 1;
            }
            response
                .device_lists
                .extend(migration_response(&peer).device_lists);
            response.lid_mappings.push(UsyncLidMapping {
                phone_number: pn.user.clone(),
                lid: lid.user.clone(),
            });
            let result = if group {
                client
                    .try_process_group_device_list_response(
                        &response,
                        &[pn.clone(), peer.clone()],
                        client.device_topology.current(),
                    )
                    .await
            } else {
                client
                    .apply_device_list_response(
                        &response,
                        DeviceListApplyMode::StrictRefresh,
                        None,
                        None,
                    )
                    .await
            };
            if case != "valid" {
                if group {
                    let devices = result.unwrap().unwrap();
                    assert!(
                        devices.contains(&pn.with_device(3)),
                        "rejected PN must retain its own fanout"
                    );
                    assert!(!devices.contains(&pn.with_device(9)));
                    assert!(devices.contains(&peer.with_device(7)));
                } else {
                    assert!(result.is_err());
                }
                assert!(
                    client
                        .lid_pn_cache
                        .get_current_lid(&pn.user)
                        .await
                        .is_none()
                );
                assert!(
                    client
                        .lid_pn_cache
                        .get_phone_number(&lid.user)
                        .await
                        .is_none()
                );
                assert_eq!(
                    client.cached_adv_account_identity(&pn).await.unwrap(),
                    pn_key
                );
                assert_eq!(
                    client.cached_adv_account_identity(&lid).await.unwrap(),
                    Some(lid_key)
                );
                assert!(has_session(&client, &old_session).await);
                assert!(
                    client
                        .persistence_manager
                        .backend()
                        .get_lid_mapping(&lid.user)
                        .await
                        .unwrap()
                        .is_none()
                );
            } else {
                let devices = result.unwrap().unwrap();
                assert!(devices.iter().any(|jid| (jid.user == pn.user || jid.user == lid.user) && jid.device == 7));
                for user in [&pn, &lid] {
                    let record = client.load_device_record_for_jid(user).await.unwrap();
                    assert_eq!(
                        record
                            .devices
                            .iter()
                            .map(|device| device.device_id())
                            .collect::<Vec<_>>(),
                        [0, 7]
                    );
                    assert_ne!(record.timestamp, 1);
                }
                assert_eq!(
                    client
                        .lid_pn_cache
                        .get_current_lid(&pn.user)
                        .await
                        .as_deref(),
                    Some(lid.user.as_str())
                );
            }
            client.signal_shutdown_sync();
        }
    }
}

/// A replacement binding cannot hide the previous owner's different anchor,
/// including when only that owner's row currently carries the LID identity.
#[tokio::test]
async fn replacement_mapping_preserves_the_previous_account_anchor() {
    use wacore::usync::UsyncLidMapping;
    for direct_lid_key in [false, true] {
        for group in [false, true] {
            let (client, _) = create_iq_test_client().await;
            let pn = Jid::pn("12025550179");
            let previous_pn = Jid::pn("12025550180");
            let lid = Jid::lid("100000000000179");
            let peer = Jid::pn("12025550181");
            client
                .add_lid_pn_mapping(
                    &lid.user,
                    &previous_pn.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
                .unwrap();
            for user in [&pn, &peer] {
                crate::test_utils::seed_adv_primary(&client, user).await;
                client
                    .update_device_list(DeviceListRecord {
                        user: user.user.as_str().into(),
                        devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                        raw_id: Some(1),
                        timestamp: 1,
                        phash: None,
                    })
                    .await
                    .unwrap();
            }
            let previous = *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                .public_key
                .public_key_bytes();
            client
                .signal_cache
                .put_identity(&previous_pn.to_protocol_address(), &previous)
                .await;
            if direct_lid_key {
                crate::test_utils::seed_adv_primary(&client, &lid).await;
            }
            let mut response = migration_response(&pn);
            response
                .device_lists
                .extend(migration_response(&peer).device_lists);
            response.lid_mappings.push(UsyncLidMapping {
                phone_number: pn.user.clone(),
                lid: lid.user.clone(),
            });
            let result = if group {
                client
                    .try_process_group_device_list_response(
                        &response,
                        &[pn.clone(), peer.clone()],
                        client.device_topology.current(),
                    )
                    .await
            } else {
                client
                    .apply_device_list_response(
                        &response,
                        DeviceListApplyMode::StrictRefresh,
                        None,
                        None,
                    )
                    .await
            };
            if group {
                let devices = result.unwrap().unwrap();
                assert!(devices.contains(&pn.with_device(3)));
                assert!(devices.contains(&peer.with_device(7)));
                assert!(!devices.contains(&pn.with_device(7)));
            } else {
                assert!(result.is_err());
            }
            assert!(
                client
                    .lid_pn_cache
                    .get_current_lid(&pn.user)
                    .await
                    .is_none()
            );
            assert_eq!(
                client
                    .lid_pn_cache
                    .get_current_lid(&previous_pn.user)
                    .await
                    .as_deref(),
                Some(lid.user.as_str())
            );
            assert_eq!(
                client
                    .lid_pn_cache
                    .get_phone_number(&lid.user)
                    .await
                    .as_deref(),
                Some(previous_pn.user.as_str())
            );
            assert_eq!(
                client
                    .cached_adv_account_identity(&previous_pn)
                    .await
                    .unwrap(),
                Some(previous)
            );
            client.signal_shutdown_sync();
        }
    }
}

/// Authenticate a rotation before publishing the response's first PN/LID
/// binding. Capturing no current alias must not permanently block that binding.
#[tokio::test]
async fn ordinary_rotation_recovers_with_its_first_response_mapping() {
    for candidate_known in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550182");
        let lid = Jid::lid("100000000000182");
        crate::test_utils::seed_adv_primary(&client, &phone).await;
        if candidate_known {
            crate::test_utils::seed_adv_primary(&client, &lid).await;
        }
        client
            .update_device_list(DeviceListRecord {
                user: phone.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        let old = client.cached_adv_account_identity(&phone).await.unwrap();
        let old_session = seed_fresh_session(&client, &phone.with_device(3)).await;
        let alias_session = seed_fresh_session(&client, &lid).await;
        let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let original = adv_wire_response(&phone, signed_list(&replacement, false));
        let devices = original
            .get_optional_child("usync")
            .unwrap()
            .get_optional_child("list")
            .unwrap()
            .get_optional_child("user")
            .unwrap()
            .get_optional_child("devices")
            .unwrap()
            .clone();
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", phone.to_string())
                        .children([
                            devices,
                            NodeBuilder::new("lid").attr("val", lid.to_string()).build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();
        let operation = {
            let client = client.clone();
            let phone = phone.clone();
            tokio::spawn(async move {
                client
                    .fetch_user_devices_with_freshness(vec![phone], Freshness::Refresh)
                    .await
            })
        };
        for index in 0..2 {
            let iq = decode_sent_iq(&transport, index).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            answer_iq(&client, &id, &response).await;
        }
        let iq = decode_sent_iq(&transport, 2).await;
        assert_eq!(
            iq.get().attrs().optional_string("xmlns").as_deref(),
            Some("encrypt")
        );
        assert!(client.device_list_identity_alias(&phone).await.is_none());
        assert_eq!(
            client.cached_adv_account_identity(&phone).await.unwrap(),
            old
        );
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            if candidate_known { old } else { None }
        );
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&phone, &replacement, true)).await;
        let result = tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.unwrap(), vec![phone.clone(), phone.with_device(2)]);
        assert_eq!(
            client.device_list_identity_alias(&phone).await,
            Some(lid.clone())
        );
        for user in [&phone, &lid] {
            assert_eq!(
                client.device_list_account_identity(user).await.unwrap(),
                Some(*replacement.public_key.public_key_bytes())
            );
            let record = client.load_device_record_for_jid(user).await.unwrap();
            assert_eq!(
                record
                    .devices
                    .iter()
                    .map(|device| device.device_id())
                    .collect::<Vec<_>>(),
                [0, 2]
            );
        }
        assert!(!has_session(&client, &old_session).await);
        assert!(!has_session(&client, &alias_session).await);
        assert_eq!(transport.sent().len(), 3);
        client.signal_shutdown_sync();
    }
}

fn first_mapping_failure(
    client: &Client,
    phone: &Jid,
    lid: &Jid,
    signer: &KeyPair,
) -> DeviceListAdvError {
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    DeviceListAdvError {
        user: phone.clone(),
        hosted: false,
        response: DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: phone.clone(),
                devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(2, Some(1))],
                phash: None,
                key_index_bytes: Some(signed_list(signer, false)),
            }],
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.user.clone(),
                lid: lid.user.clone(),
            }],
        },
        topology_generation: client.device_topology.current(),
        authentication_epoch: client.signal_cache.identity_continuity.primary_snapshot(),
    }
}

/// A first mapping is a captured proposal, never permission to ignore a key,
/// relationship or paired-account change while the primary IQ is outstanding.
async fn assert_round4_publication(client: &Client, lid: &Jid, devices: &[Jid]) {
    assert_eq!(
        devices.iter().map(|jid| jid.device).collect::<Vec<_>>(),
        [0, 2]
    );
    // Inspect cache before any fallback getter can fill it from storage.
    let cached = client.device_registry_cache.get(&lid.user).await.unwrap();
    let durable = client
        .persistence_manager
        .backend()
        .get_devices(&lid.user)
        .await
        .unwrap()
        .unwrap();
    for record in [cached.as_ref(), &durable] {
        assert_eq!(
            record
                .devices
                .iter()
                .map(|device| device.device_id())
                .collect::<Vec<_>>(),
            [0, 2]
        );
    }
}

#[tokio::test]
async fn adv_review_round4_mapping_free_recovery_rechecks_durable_ownership() {
    for foreign_owner in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550101");
        let lid = Jid::lid("100000000000301");
        let other = Jid::pn("12025550102");
        client
            .add_lid_pn_mapping(
                &lid.user,
                &phone.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        for user in [&phone, &lid] {
            crate::test_utils::seed_adv_primary(&client, user).await;
        }
        let original = client
            .cached_adv_account_identity(&phone)
            .await
            .unwrap()
            .unwrap();
        let old_session = seed_fresh_session(&client, &phone.with_device(3)).await;
        client
            .update_device_list(DeviceListRecord {
                user: lid.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        let backend = client.persistence_manager.backend();
        client.signal_cache.flush(backend.as_ref()).await.unwrap();
        let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let mut failure = first_mapping_failure(&client, &phone, &lid, &replacement);
        failure.response.lid_mappings.clear();
        let recovery = {
            let client = client.clone();
            tokio::spawn(async move { client.recover_ordinary_adv_identity(failure.into()).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        if foreign_owner {
            backend
                .put_lid_mapping(&wacore::store::traits::LidPnMappingEntry {
                    lid: lid.user.to_string(),
                    phone_number: other.user.to_string(),
                    created_at: client.runtime.wall_time_secs() + 1,
                    updated_at: client.runtime.wall_time_secs() + 1,
                    learning_source: "usync".into(),
                })
                .await
                .unwrap();
        }
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&phone, &replacement, true)).await;
        let result = tokio::time::timeout(Duration::from_secs(5), recovery)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.is_err(),
            foreign_owner,
            "durable owner changed={foreign_owner}"
        );
        if foreign_owner {
            for user in [&phone, &lid] {
                assert_eq!(
                    client.cached_adv_account_identity(user).await.unwrap(),
                    Some(original)
                );
                assert_eq!(
                    backend
                        .load_identity(user.to_protocol_address().as_str())
                        .await
                        .unwrap(),
                    Some(original)
                );
            }
            assert!(has_session(&client, &old_session).await);
            assert_eq!(
                backend
                    .get_devices(&lid.user)
                    .await
                    .unwrap()
                    .unwrap()
                    .timestamp,
                1
            );
            assert_eq!(
                backend
                    .get_lid_mapping(&lid.user)
                    .await
                    .unwrap()
                    .unwrap()
                    .phone_number,
                other.user.as_str()
            );
        } else {
            let devices = result.unwrap();
            assert_round4_publication(&client, &lid, &devices).await;
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                Some(*replacement.public_key.public_key_bytes())
            );
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_round4_recovery_rejects_primary_aba_during_authentication() {
    for retired in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550103");
        let lid = Jid::lid("100000000000303");
        crate::test_utils::seed_adv_primary(&client, &phone).await;
        let original = client
            .cached_adv_account_identity(&phone)
            .await
            .unwrap()
            .unwrap();
        let old_session = seed_fresh_session(&client, &phone.with_device(3)).await;
        client
            .update_device_list(DeviceListRecord {
                user: phone.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        let backend = client.persistence_manager.backend();
        client.signal_cache.flush(backend.as_ref()).await.unwrap();
        let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let failure = first_mapping_failure(&client, &phone, &lid, &replacement);
        let recovery = {
            let client = client.clone();
            tokio::spawn(async move { client.recover_ordinary_adv_identity(failure.into()).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        if retired {
            // Identity reactions are intentionally delayed: bytes and topology
            // are restored, but the intervening primary retirement is real.
            let lock = client
                .session_lock_for(phone.to_protocol_address().as_str())
                .await;
            let _guard = lock.lock_arc().await;
            let temporary = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            client
                .signal_cache
                .put_identity(
                    &phone.to_protocol_address(),
                    temporary.public_key.public_key_bytes(),
                )
                .await;
            client
                .signal_cache
                .put_identity(&phone.to_protocol_address(), &original)
                .await;
            client.signal_cache.flush(backend.as_ref()).await.unwrap();
        }
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&phone, &replacement, true)).await;
        let result = tokio::time::timeout(Duration::from_secs(5), recovery)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.is_err(), retired, "primary ABA={retired}");
        if retired {
            assert_eq!(
                client.cached_adv_account_identity(&phone).await.unwrap(),
                Some(original)
            );
            assert_eq!(
                client.cached_adv_account_identity(&lid).await.unwrap(),
                None
            );
            assert!(client.device_list_identity_alias(&phone).await.is_none());
            assert!(has_session(&client, &old_session).await);
            assert_eq!(
                backend
                    .get_devices(&phone.user)
                    .await
                    .unwrap()
                    .unwrap()
                    .timestamp,
                1
            );
        } else {
            let devices = result.unwrap();
            assert_round4_publication(&client, &lid, &devices).await;
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_round4_cold_durable_owner_cannot_be_rebound_by_adv() {
    for foreign_owner in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let phone = Jid::pn("12025550104");
        let other = Jid::pn("12025550105");
        let lid = Jid::lid("100000000000304");
        crate::test_utils::seed_adv_primary(&client, &phone).await;
        let foreign_key = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        client
            .signal_cache
            .put_identity(
                &other.to_protocol_address(),
                foreign_key.public_key.public_key_bytes(),
            )
            .await;
        let backend = client.persistence_manager.backend();
        client.signal_cache.flush(backend.as_ref()).await.unwrap();
        backend
            .put_lid_mapping(&wacore::store::traits::LidPnMappingEntry {
                lid: lid.user.to_string(),
                phone_number: if foreign_owner {
                    other.user.to_string()
                } else {
                    phone.user.to_string()
                },
                created_at: 1,
                updated_at: 1,
                learning_source: "usync".into(),
            })
            .await
            .unwrap();
        backend
            .update_device_list(DeviceListRecord {
                user: lid.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(9, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        assert!(client.device_list_identity_alias(&phone).await.is_none());
        assert!(client.device_list_identity_alias(&lid).await.is_none());
        let response =
            first_mapping_failure(&client, &phone, &lid, crate::test_utils::adv_test_account())
                .response;
        let result = client
            .apply_device_list_response(&response, DeviceListApplyMode::StrictRefresh, None, None)
            .await;
        assert_eq!(
            result.is_err(),
            foreign_owner,
            "cold durable owner changed={foreign_owner}"
        );
        if foreign_owner {
            assert_eq!(
                backend
                    .get_lid_mapping(&lid.user)
                    .await
                    .unwrap()
                    .unwrap()
                    .phone_number,
                other.user.as_str()
            );
            assert!(client.device_list_identity_alias(&phone).await.is_none());
            assert_eq!(
                backend
                    .get_devices(&lid.user)
                    .await
                    .unwrap()
                    .unwrap()
                    .timestamp,
                1
            );
            assert_eq!(
                client.cached_adv_account_identity(&other).await.unwrap(),
                Some(*foreign_key.public_key.public_key_bytes())
            );
        } else {
            let devices = result.unwrap().unwrap();
            assert_round4_publication(&client, &lid, &devices).await;
        }
        client.signal_shutdown_sync();
    }
}

/// Retirement during recovery preflight must reject mappings before their first publication.
#[tokio::test]
async fn adv_review_round4_retirement_during_preflight_cannot_publish_mapping() {
    use crate::test_utils::create_iq_test_client_with_backend;
    for retire in [false, true] {
        let backend: Arc<dyn crate::store::traits::Backend> =
            Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let (client, transport) = create_iq_test_client_with_backend(backend.clone()).await;
        tokio::time::timeout(Duration::from_secs(5), async {
            while !client.lid_pn_cache.startup_finished_for_tests() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let phone = Jid::pn("12025550115");
        let lid = Jid::lid("100000000000315");
        crate::test_utils::seed_adv_primary(&client, &phone).await;
        let signer = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let failure = first_mapping_failure(&client, &phone, &lid, &signer);
        let snapshot = client
            .capture_adv_recovery_snapshot(&phone, &failure.response)
            .await
            .unwrap();
        let authentication = {
            let client = client.clone();
            let phone = phone.clone();
            tokio::spawn(async move { client.fetch_adv_account_identity(&phone).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&phone, &signer, true)).await;
        let recovered = RecoveredAccountIdentity {
            user: phone.clone(),
            identity: authentication.await.unwrap().unwrap(),
            snapshot,
        };
        let reactions = AdvIdentityReactions::default();
        let registry = client.device_topology.lock_registry().await;
        // Drive the real publication future manually, so the reset cannot
        // update topology before publication resumes from this exact wait.
        let mut publication = Box::pin(client.apply_device_list_response_inner(
            &failure.response,
            DeviceListApplyMode::StrictRefresh,
            Some(failure.topology_generation),
            None,
            DeviceListIdentityContext {
                prepared: &[],
                recovered: Some(&recovered),
                publication_started: None,
                authentication_epoch: recovered.snapshot.primary_epoch,
                reactions: Some(&reactions),
            },
            None,
        ));
        assert!(futures::poll!(publication.as_mut()).is_pending());
        let mut mapping = Box::pin(client.lid_pn_cache.lock_mutation());
        assert!(futures::poll!(mapping.as_mut()).is_pending());
        drop(mapping);
        let epoch = client.signal_cache.identity_continuity.snapshot();
        let mut reset =
            retire.then(|| Box::pin(deliver_identity_reset(&client, &phone, Some(&lid))));
        if let Some(reset) = reset.as_mut() {
            assert!(futures::poll!(reset.as_mut()).is_pending());
            assert!(
                !client
                    .signal_cache
                    .identity_continuity
                    .unchanged_primary_for(epoch, [phone.user.as_str()])
            );
        }
        drop(registry);
        let result = publication.await;
        reactions.dispatch(&client);
        assert_eq!(result.is_err(), retire);
        if !retire {
            assert_round4_publication(&client, &lid, result.as_ref().unwrap().as_ref().unwrap())
                .await;
        }
        // Check before polling reset cleanup: it cannot mask publication's
        // mapping effects by invalidating topology first.
        assert_eq!(
            backend.get_lid_mapping(&lid.user).await.unwrap().is_some(),
            !retire
        );
        assert_eq!(
            client.device_list_identity_alias(&phone).await.is_some(),
            !retire
        );
        if let Some(reset) = reset {
            reset.await;
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_round4_historical_lid_does_not_block_a_new_unowned_lid() {
    let (client, _) = create_iq_test_client().await;
    let phone = Jid::pn("12025550109");
    let old_lid = Jid::lid("100000000000310");
    let new_lid = Jid::lid("100000000000309");
    crate::test_utils::seed_adv_primary(&client, &phone).await;
    let backend = client.persistence_manager.backend();
    backend
        .put_lid_mapping(&wacore::store::traits::LidPnMappingEntry {
            lid: old_lid.user.to_string(),
            phone_number: phone.user.to_string(),
            created_at: 1,
            updated_at: 1,
            learning_source: "usync".into(),
        })
        .await
        .unwrap();
    backend
        .update_device_list(DeviceListRecord {
            user: old_lid.user.as_str().into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(9, Some(1))].into(),
            raw_id: Some(1),
            timestamp: 1,
            phash: None,
        })
        .await
        .unwrap();
    let response = first_mapping_failure(
        &client,
        &phone,
        &new_lid,
        crate::test_utils::adv_test_account(),
    )
    .response;
    let devices = client
        .apply_device_list_response(&response, DeviceListApplyMode::StrictRefresh, None, None)
        .await
        .unwrap()
        .unwrap();
    assert_round4_publication(&client, &new_lid, &devices).await;
    for lid in [&old_lid, &new_lid] {
        assert_eq!(
            backend
                .get_lid_mapping(&lid.user)
                .await
                .unwrap()
                .unwrap()
                .phone_number,
            phone.user.as_str()
        );
    }
    assert_eq!(
        backend
            .get_devices(&old_lid.user)
            .await
            .unwrap()
            .unwrap()
            .timestamp,
        1
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn first_mapping_recovery_rejects_changes_during_authentication() {
    for change in [
        "primary-key",
        "candidate-key",
        "source-alias",
        "candidate-owner",
        "paired-candidate",
        "paired-primary",
        "wrong-primary",
        "invalid-primary",
    ] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550184");
        let lid = Jid::lid("100000000000184");
        crate::test_utils::seed_adv_primary(&client, &phone).await;
        let old_session = seed_fresh_session(&client, &phone.with_device(3)).await;
        client
            .update_device_list(DeviceListRecord {
                user: phone.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let error = first_mapping_failure(&client, &phone, &lid, &replacement);
        let recovery = {
            let client = client.clone();
            tokio::spawn(async move { client.recover_ordinary_adv_identity(error.into()).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        assert!(client.device_list_identity_alias(&phone).await.is_none());
        assert!(client.device_list_identity_alias(&lid).await.is_none());
        assert!(
            client.lid_pn_cache.try_lock_mutation().is_some(),
            "snapshot fence must be released before the IQ"
        );
        let other_key = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        match change {
            "primary-key" | "candidate-key" => {
                let target = if change == "primary-key" {
                    &phone
                } else {
                    &lid
                };
                let lock = client
                    .session_lock_for(target.to_protocol_address().as_str())
                    .await;
                let _guard = lock.lock_arc().await;
                client
                    .signal_cache
                    .put_identity(
                        &target.to_protocol_address(),
                        other_key.public_key.public_key_bytes(),
                    )
                    .await;
            }
            "source-alias" | "candidate-owner" => {
                let (lid, phone) = if change == "source-alias" {
                    (Jid::lid("100000000000185"), phone.clone())
                } else {
                    (lid.clone(), Jid::pn("12025550185"))
                };
                client
                    .lid_pn_cache
                    .add(&crate::lid_pn_cache::LidPnEntry::new(
                        lid.user.to_string(),
                        phone.user.to_string(),
                        crate::lid_pn_cache::LearningSource::Usync,
                    ))
                    .await;
            }
            "paired-candidate" | "paired-primary" => {
                client
                    .persistence_manager
                    .process_command(if change == "paired-candidate" {
                        wacore::store::commands::DeviceCommand::SetLid(Some(lid.clone()))
                    } else {
                        wacore::store::commands::DeviceCommand::SetId(Some(phone.clone()))
                    })
                    .await;
            }
            _ => {}
        }
        let expected_phone = client.cached_adv_account_identity(&phone).await.unwrap();
        let expected_lid = client.cached_adv_account_identity(&lid).await.unwrap();
        let phone_alias = client.device_list_identity_alias(&phone).await;
        let lid_alias = client.device_list_identity_alias(&lid).await;
        let signer = if change == "wrong-primary" {
            &other_key
        } else {
            &replacement
        };
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(
            &client,
            &id,
            &primary_response(&phone, signer, change != "invalid-primary"),
        )
        .await;
        let error = tokio::time::timeout(Duration::from_secs(5), recovery)
            .await
            .unwrap()
            .unwrap()
            .unwrap_err();
        if change == "paired-candidate" || change == "paired-primary" {
            assert!(error.to_string().contains("paired primary"), "{error:#}");
        }
        assert_eq!(
            client.cached_adv_account_identity(&phone).await.unwrap(),
            expected_phone,
            "{change}"
        );
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            expected_lid,
            "{change}"
        );
        assert_eq!(
            client.device_list_identity_alias(&phone).await,
            phone_alias,
            "{change}"
        );
        assert_eq!(
            client.device_list_identity_alias(&lid).await,
            lid_alias,
            "{change}"
        );
        assert!(has_session(&client, &old_session).await, "{change}");
        let record = client
            .persistence_manager
            .backend()
            .get_devices(&phone.user)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record
                .devices
                .iter()
                .map(|device| device.device_id())
                .collect::<Vec<_>>(),
            [0, 3]
        );
        assert_eq!(transport.sent().len(), 1);
        client.signal_shutdown_sync();
    }
}

/// An occupied, conflicting, paired or ambiguous destination cannot become a
/// recovery target merely because it appeared in an uncommitted response.
#[tokio::test]
async fn first_mapping_recovery_rejects_ineligible_candidates_before_iq() {
    use wacore::usync::UsyncLidMapping;
    for case in [
        "occupied",
        "reverse-only",
        "conflicting-key",
        "ambiguous-source",
        "ambiguous-target",
        "paired",
    ] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550186");
        let lid = Jid::lid("100000000000186");
        let other = Jid::pn("12025550187");
        let other_lid = Jid::lid("100000000000187");
        crate::test_utils::seed_adv_primary(&client, &phone).await;
        let old = client.cached_adv_account_identity(&phone).await.unwrap();
        let session = seed_fresh_session(&client, &phone).await;
        let signer = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let mut failure = first_mapping_failure(&client, &phone, &lid, &signer);
        match case {
            "occupied" | "reverse-only" => {
                client
                    .lid_pn_cache
                    .add(&crate::lid_pn_cache::LidPnEntry::new(
                        lid.user.to_string(),
                        other.user.to_string(),
                        crate::lid_pn_cache::LearningSource::Usync,
                    ))
                    .await;
                if case == "reverse-only" {
                    client
                        .lid_pn_cache
                        .add(&crate::lid_pn_cache::LidPnEntry::new(
                            other_lid.user.to_string(),
                            other.user.to_string(),
                            crate::lid_pn_cache::LearningSource::Usync,
                        ))
                        .await;
                }
            }
            "conflicting-key" => {
                client
                    .signal_cache
                    .put_identity(
                        &lid.to_protocol_address(),
                        signer.public_key.public_key_bytes(),
                    )
                    .await;
            }
            "ambiguous-source" => failure.response.lid_mappings.push(UsyncLidMapping {
                phone_number: phone.user.clone(),
                lid: other_lid.user.clone(),
            }),
            "ambiguous-target" => failure.response.lid_mappings.push(UsyncLidMapping {
                phone_number: other.user.clone(),
                lid: lid.user.clone(),
            }),
            "paired" => {
                client
                    .persistence_manager
                    .process_command(wacore::store::commands::DeviceCommand::SetLid(Some(
                        lid.clone(),
                    )))
                    .await;
                crate::test_utils::seed_adv_primary(&client, &lid).await;
            }
            _ => unreachable!(),
        }
        let candidate_key = client.cached_adv_account_identity(&lid).await.unwrap();
        let candidate_alias = client.device_list_identity_alias(&lid).await;
        if case == "occupied" || case == "reverse-only" {
            assert_eq!(candidate_alias, Some(other.clone()));
        }
        if case == "reverse-only" {
            assert_eq!(
                client.device_list_identity_alias(&other).await,
                Some(other_lid.clone())
            );
        }
        let owner_lock = client
            .session_lock_for(other.to_protocol_address().as_str())
            .await;
        let owner_guard = owner_lock.lock_arc().await;
        tokio::time::timeout(
            Duration::from_secs(5),
            client.recover_ordinary_adv_identity(failure.into()),
        )
        .await
        .expect("reject before waiting for an IQ")
        .unwrap_err();
        assert!(transport.sent().is_empty(), "{case}");
        assert_eq!(
            client.cached_adv_account_identity(&phone).await.unwrap(),
            old,
            "{case}"
        );
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            candidate_key,
            "{case}"
        );
        assert_eq!(
            client.device_list_identity_alias(&lid).await,
            candidate_alias,
            "{case}"
        );
        assert!(has_session(&client, &session).await, "{case}");
        assert!(client.device_list_identity_alias(&phone).await.is_none());
        drop(owner_guard);
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_bootstrap_rechecks_retirement_after_backend_ack() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    let key = *crate::test_utils::adv_test_account()
        .public_key
        .public_key_bytes();
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.detach_identity_writes = true;
    probe.identity_gate_key = Some(key);
    probe.gate_method = "put_identity";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let primary = Jid::pn("12025550135");
    let writer = {
        let client = client.clone();
        let primary = primary.clone();
        tokio::spawn(async move { client.persist_adv_bootstrap_identity(&primary, &key).await })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    let retirement = client
        .signal_cache
        .identity_continuity
        .changing_primary([primary.user.as_str()]);
    release.send(()).await.unwrap();
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), writer)
            .await
            .unwrap()
            .unwrap()
            .unwrap()
    );
    // Rejection must retire the acknowledged address itself: reset cleanup
    // may know only its alias and cannot be relied on to find this write.
    assert_eq!(
        raw.load_identity(primary.to_protocol_address().as_str())
            .await
            .unwrap(),
        None
    );
    client
        .signal_cache
        .flush(client.persistence_manager.backend().as_ref())
        .await
        .unwrap();
    drop(retirement);
    assert_eq!(
        raw.load_identity(primary.to_protocol_address().as_str())
            .await
            .unwrap(),
        None
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_staged_proofs_remain_bound_to_duplicate_response_rows() {
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    for group in [false, true] {
        for (last_device, last_index) in [(8, 2), (7, 1)] {
            let (client, _) = create_iq_test_client().await;
            let phone = Jid::pn("12025550136");
            let lid = Jid::lid("100000000000236");
            for user in [&phone, &lid] {
                crate::test_utils::seed_adv_primary(&client, user).await;
            }
            let response = DeviceListResponse {
                device_lists: [(7, 1), (last_device, last_index)]
                    .into_iter()
                    .map(|(device, index)| UserDeviceList {
                        user: phone.clone(),
                        devices: vec![
                            UsyncDevice::new(0, None),
                            UsyncDevice::new(device, Some(index)),
                        ],
                        phash: None,
                        key_index_bytes: Some(signed_key_index_for_epoch(
                            vec![index],
                            index,
                            index,
                        )),
                    })
                    .collect(),
                lid_mappings: vec![UsyncLidMapping {
                    phone_number: phone.user.clone(),
                    lid: lid.user.clone(),
                }],
            };
            let devices = client
                .apply_device_list_response(
                    &response,
                    if group {
                        DeviceListApplyMode::GroupRefresh
                    } else {
                        DeviceListApplyMode::StrictRefresh
                    },
                    None,
                    group.then_some(std::slice::from_ref(&phone)),
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                devices.iter().map(|jid| jid.device).collect::<Vec<_>>(),
                [0, last_device]
            );
            for user in [&phone, &lid] {
                let record = client.load_device_record_for_jid(user).await.unwrap();
                assert_eq!(
                    record
                        .devices
                        .iter()
                        .map(|device| (device.device_id(), device.key_index()))
                        .collect::<Vec<_>>(),
                    [(0, None), (last_device, Some(last_index))]
                );
                assert_eq!(record.raw_id, Some(last_index));
            }
            client.signal_shutdown_sync();
        }
    }
}

#[tokio::test]
async fn adv_review_recovery_rejects_a_durably_owned_cold_alias() {
    let (client, transport) = create_iq_test_client().await;
    let phone = Jid::pn("12025550137");
    let other = Jid::pn("12025550138");
    let lid = Jid::lid("100000000000237");
    crate::test_utils::seed_adv_primary(&client, &phone).await;
    client
        .persistence_manager
        .backend()
        .put_lid_mapping(&wacore::store::traits::LidPnMappingEntry {
            lid: lid.user.to_string(),
            phone_number: other.user.to_string(),
            created_at: 1,
            updated_at: 1,
            learning_source: "usync".into(),
        })
        .await
        .unwrap();
    assert!(client.device_list_identity_alias(&lid).await.is_none());
    let failure =
        first_mapping_failure(&client, &phone, &lid, crate::test_utils::adv_test_account());
    assert!(
        client
            .capture_adv_recovery_snapshot(&phone, &failure.response)
            .await
            .is_err(),
        "a cache miss must not erase durable ownership"
    );
    assert!(transport.sent().is_empty());
    assert_eq!(
        client
            .persistence_manager
            .backend()
            .get_lid_mapping(&lid.user)
            .await
            .unwrap()
            .unwrap()
            .phone_number,
        other.user.as_str()
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_paired_ownership_waits_through_recovery_persistence() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    let signer = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let key = *signer.public_key.public_key_bytes();
    let (mut probe, _) = ProbeBackend::new();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.detach_identity_writes = true;
    probe.identity_gate_key = Some(key);
    probe.gate_method = "put_identity";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let phone = Jid::pn("12025550139");
    let lid = Jid::lid("100000000000239");
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
    }
    let failure = first_mapping_failure(&client, &phone, &lid, &signer);
    let recovery = {
        let client = client.clone();
        tokio::spawn(async move { client.recover_ordinary_adv_identity(failure.into()).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&phone, &signer, true)).await;
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    let mut paired = Box::pin(client.persistence_manager.process_command(
        wacore::store::commands::DeviceCommand::SetLid(Some(lid.clone())),
    ));
    let blocked = futures::poll!(paired.as_mut()).is_pending();
    let watermarks = client.persistence_manager.get_device_snapshot();
    let mut unrelated = Box::pin(client.persistence_manager.process_command(
        wacore::store::commands::DeviceCommand::SetPreKeyWatermarks {
            next_pre_key_id: watermarks.next_pre_key_id + 1,
            first_unupload_pre_key_id: watermarks.first_unupload_pre_key_id,
        },
    ));
    assert!(
        futures::poll!(unrelated.as_mut()).is_ready(),
        "unrelated prekey watermarks must progress before the trust-storage ACK"
    );
    release.send(()).await.unwrap();
    release.send(()).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), recovery)
        .await
        .unwrap()
        .unwrap();
    if blocked {
        paired.await;
    }
    assert_eq!(
        client
            .persistence_manager
            .get_device_snapshot()
            .next_pre_key_id,
        watermarks.next_pre_key_id + 1
    );
    assert!(
        blocked,
        "paired-account ownership changed inside recovery persistence"
    );
    assert!(result.is_ok());
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_late_startup_warmup_invalidates_the_earlier_topology_snapshot() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate_method = "get_all_lid_mappings";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(!client.lid_pn_cache.startup_finished_for_tests());
    let phone = Jid::pn("12025550140");
    let lid = Jid::lid("100000000000240");
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
    }
    raw.put_lid_mapping(&wacore::store::traits::LidPnMappingEntry {
        lid: lid.user.to_string(),
        phone_number: phone.user.to_string(),
        created_at: 1,
        updated_at: 1,
        learning_source: "usync".into(),
    })
    .await
    .unwrap();
    let before_warmup = client.device_topology.current();
    release.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.lid_pn_cache.startup_finished_for_tests() {
            tokio::task::yield_now().await;
        }
        drop(client.lid_pn_cache.lock_mutation().await);
    })
    .await
    .unwrap();
    let response = migration_response(&phone);
    assert!(
        client
            .try_process_group_device_list_response(
                &response,
                std::slice::from_ref(&phone),
                before_warmup
            )
            .await
            .unwrap()
            .is_none(),
        "a real startup mutation must retain the stale-response gate"
    );
    let devices = client
        .try_process_group_device_list_response(
            &response,
            std::slice::from_ref(&phone),
            client.device_topology.current(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_migration_publication(&client, &phone, &devices).await;
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_wire_hosted_subjects_cannot_bypass_the_account_epoch_graph() {
    use wacore::iq::spec::IqSpec;
    for conflicting in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let phone = Jid::pn("12025550142");
        let lid = Jid::lid("100000000000242");
        for user in [&phone, &lid] {
            crate::test_utils::seed_adv_primary(&client, user).await;
        }
        let old = client.cached_adv_account_identity(&phone).await.unwrap();
        let other = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let hosted_account = if conflicting {
            &other
        } else {
            crate::test_utils::adv_test_account()
        };
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", phone.to_string())
                            .children([
                                NodeBuilder::new("lid").attr("val", lid.to_string()).build(),
                                NodeBuilder::new("devices")
                                    .children([
                                        NodeBuilder::new("key-index-list")
                                            .attr("ts", "1000")
                                            .bytes(signed_list(
                                                crate::test_utils::adv_test_account(),
                                                false,
                                            ))
                                            .build(),
                                        NodeBuilder::new("device-list")
                                            .children([
                                                NodeBuilder::new("device").attr("id", "0").build(),
                                                NodeBuilder::new("device")
                                                    .attr("id", "7")
                                                    .attr("key-index", "1")
                                                    .build(),
                                            ])
                                            .build(),
                                    ])
                                    .build(),
                            ])
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", format!("{}:99@hosted.lid", lid.user))
                            .attr("pn_jid", phone.to_string())
                            .children([NodeBuilder::new("devices")
                                .children([
                                    NodeBuilder::new("key-index-list")
                                        .attr("ts", "1000")
                                        .bytes(signed_list(hosted_account, true))
                                        .build(),
                                    NodeBuilder::new("device-list")
                                        .children([
                                            NodeBuilder::new("device").attr("id", "0").build(),
                                            NodeBuilder::new("device")
                                                .attr("id", "99")
                                                .attr("key-index", "1")
                                                .attr("is_hosted", "true")
                                                .build(),
                                        ])
                                        .build(),
                                ])
                                .build()])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();
        let parsed = DeviceListSpec::new(vec![phone.clone()], "hosted-subject")
            .require_complete_response()
            .parse_response(&response.as_node_ref())
            .unwrap();
        assert_eq!(parsed.device_lists[1].user, lid);
        let result = client
            .apply_device_list_response(&parsed, DeviceListApplyMode::StrictRefresh, None, None)
            .await;
        if conflicting {
            assert!(result.is_err());
            assert!(client.device_list_identity_alias(&phone).await.is_none());
            assert!(client.load_device_record_for_jid(&phone).await.is_none());
        } else {
            let devices = result.unwrap().unwrap();
            assert!(!devices.iter().any(|jid| jid.device == 7));
            assert_eq!(
                devices.iter().map(|jid| jid.device).collect::<Vec<_>>(),
                [0, 99]
            );
            assert!(
                devices
                    .iter()
                    .any(|jid| jid.device == 99 && jid.server.is_lid_family())
            );
            for user in [&phone, &lid] {
                assert_eq!(
                    client
                        .load_device_record_for_jid(user)
                        .await
                        .unwrap()
                        .devices
                        .iter()
                        .map(|device| device.device_id())
                        .collect::<Vec<_>>(),
                    [0, 99]
                );
            }
        }
        if conflicting {
            for user in [&phone, &lid] {
                assert_eq!(client.cached_adv_account_identity(user).await.unwrap(), old);
            }
        } else {
            let pn_before = client.cached_adv_account_identity(&phone).await.unwrap();
            assert!(pn_before.is_none() || pn_before == old);
            assert_eq!(client.cached_adv_account_identity(&lid).await.unwrap(), old);
            // A successful mapping may already have removed the redundant PN
            // copy. Complete that legal migration and check its canonical key.
            client
                .migrate_signal_sessions_on_lid_discovery(&phone.user, &lid.user)
                .await;
            assert_eq!(
                client.cached_adv_account_identity(&phone).await.unwrap(),
                None
            );
            assert_eq!(client.cached_adv_account_identity(&lid).await.unwrap(), old);
        }
        let hosted = Jid::new(lid.user.as_str(), wacore_binary::Server::HostedLid);
        assert_eq!(
            client.cached_adv_account_identity(&hosted).await.unwrap(),
            old
        );
        assert!(
            client
                .persistence_manager
                .backend()
                .load_identity(hosted.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none(),
            "hosted device addressing is not an account-primary namespace"
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_pending_mapping_write_fences_cold_alias_recovery() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    let (mut probe, _) = ProbeBackend::new();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate_method = "put_lid_mapping";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    tokio::time::timeout(Duration::from_secs(5), async {
        while !client.lid_pn_cache.startup_finished_for_tests() {
            tokio::task::yield_now().await;
        }
        drop(client.lid_pn_cache.lock_mutation().await);
    })
    .await
    .unwrap();
    let phone = Jid::pn("12025550143");
    let other = Jid::pn("12025550144");
    let lid = Jid::lid("100000000000243");
    crate::test_utils::seed_adv_primary(&client, &phone).await;
    client.cached_adv_account_identity(&lid).await.unwrap();
    for user in [&phone, &lid] {
        client
            .session_lock_for(user.to_protocol_address().as_str())
            .await;
    }
    let writer = {
        let client = client.clone();
        let lid = lid.clone();
        tokio::spawn(async move {
            client
                .add_lid_pn_mapping(
                    &lid.user,
                    &other.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    let mut clearing = Box::pin(client.lid_pn_cache.clear());
    let cleared_before_ack = futures::poll!(clearing.as_mut()).is_ready();
    let failure =
        first_mapping_failure(&client, &phone, &lid, crate::test_utils::adv_test_account());
    let mut capture = Box::pin(client.capture_adv_recovery_snapshot(&phone, &failure.response));
    let first = futures::poll!(capture.as_mut());
    let admitted_before_ack = matches!(&first, std::task::Poll::Ready(Ok(_)));
    release.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        let (written, ()) = futures::join!(writer, async {
            if !cleared_before_ack {
                clearing.await;
            }
            if first.is_pending() {
                assert!(capture.await.is_err());
            }
        });
        written.unwrap().unwrap();
    })
    .await
    .unwrap();
    eprintln!("cleared_before_ack={cleared_before_ack} admitted_before_ack={admitted_before_ack}");
    assert!(
        !cleared_before_ack,
        "mapping mutation must survive through the pending backend ACK"
    );
    assert!(
        !admitted_before_ack,
        "a detached write must not publish another owner after recovery admission"
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_mapping_workers_keep_ack_order_after_finish_or_cancellation() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    for (batch, fast) in [(false, false), (true, false), (false, true), (true, true)] {
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate_method = if batch {
            "put_lid_mappings"
        } else {
            "put_lid_mapping"
        };
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let lid = "100000000000245";
        let first = "12025550145";
        let second = "12025550146";
        let requester = {
            let client = client.clone();
            tokio::spawn(async move {
                match (batch, fast) {
                    (false, false) => {
                        client
                            .add_lid_pn_mapping(
                                lid,
                                first,
                                crate::lid_pn_cache::LearningSource::Usync,
                            )
                            .await
                    }
                    (true, false) => client
                        .add_lid_pn_mappings(
                            vec![(lid.into(), first.into())],
                            crate::lid_pn_cache::LearningSource::Usync,
                        )
                        .await
                        .map(|_| ()),
                    (false, true) => {
                        client
                            .learn_lid_pn_mapping_fast(
                                lid,
                                first,
                                crate::lid_pn_cache::LearningSource::Usync,
                                false,
                            )
                            .await;
                        Ok(())
                    }
                    (true, true) => {
                        client
                            .learn_lid_pn_mappings_batch(
                                vec![(lid.into(), first.into())],
                                crate::lid_pn_cache::LearningSource::Usync,
                                false,
                            )
                            .await;
                        Ok(())
                    }
                }
            })
        };
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        if fast {
            tokio::time::timeout(Duration::from_secs(5), requester)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
        } else {
            requester.abort();
            assert!(requester.await.unwrap_err().is_cancelled());
        }
        let mut newer = Box::pin(async {
            if batch {
                client
                    .add_lid_pn_mappings(
                        vec![(lid.into(), second.into())],
                        crate::lid_pn_cache::LearningSource::Usync,
                    )
                    .await
                    .map(|_| ())
            } else {
                client
                    .add_lid_pn_mapping(lid, second, crate::lid_pn_cache::LearningSource::Usync)
                    .await
            }
        });
        assert!(futures::poll!(newer.as_mut()).is_pending());
        tokio::task::yield_now().await;
        assert_eq!(
            client.lid_pn_cache.get_phone_number(lid).await.as_deref(),
            Some(first)
        );
        assert!(raw.get_lid_mapping(lid).await.unwrap().is_none());
        release.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            raw.get_lid_mapping(lid)
                .await
                .unwrap()
                .unwrap()
                .phone_number,
            first
        );
        release.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), newer)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            raw.get_lid_mapping(lid)
                .await
                .unwrap()
                .unwrap()
                .phone_number,
            second
        );
        assert_eq!(
            client.lid_pn_cache.get_phone_number(lid).await.as_deref(),
            Some(second)
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_mapped_publication_retains_its_store_write_after_cancellation() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use wacore::usync::UsyncLidMapping;
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate_method = "put_lid_mappings";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let phone = Jid::pn("12025550147");
    let lid = Jid::lid("100000000000247");
    for user in [&phone, &lid] {
        crate::test_utils::seed_adv_primary(&client, user).await;
    }
    let mut response = migration_response(&phone);
    response.lid_mappings.push(UsyncLidMapping {
        phone_number: phone.user.clone(),
        lid: lid.user.clone(),
    });
    let publication = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .apply_device_list_response(
                    &response,
                    DeviceListApplyMode::StrictRefresh,
                    None,
                    None,
                )
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    publication.abort();
    assert!(publication.await.unwrap_err().is_cancelled());
    let mut mapping = Box::pin(client.lid_pn_cache.lock_mutation());
    assert!(futures::poll!(mapping.as_mut()).is_pending());
    drop(mapping);
    assert!(raw.get_lid_mapping(&lid.user).await.unwrap().is_none());
    release.send(()).await.unwrap();
    let registry = tokio::time::timeout(
        Duration::from_secs(5),
        client.device_topology.lock_registry(),
    )
    .await
    .unwrap();
    assert_eq!(
        raw.get_lid_mapping(&lid.user)
            .await
            .unwrap()
            .unwrap()
            .phone_number,
        phone.user.as_str()
    );
    for user in [&phone, &lid] {
        assert_eq!(
            client
                .load_device_record_for_jid(user)
                .await
                .unwrap()
                .devices
                .iter()
                .map(|device| device.device_id())
                .collect::<Vec<_>>(),
            [0, 7]
        );
    }
    drop(registry);
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_owned_mapping_batch_persists_entries_without_cache_residency() {
    use crate::cache_config::{CacheConfig, CacheEntryConfig};
    use crate::store::persistence_manager::PersistenceManager;
    use crate::store::release::tests::ProbeBackend;
    for (capacity, timeout) in [(0, None), (1, None), (4, Some(Duration::ZERO))] {
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate_method = "put_lid_mappings";
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        let pm = Arc::new(PersistenceManager::new(Arc::new(probe)).await.unwrap());
        let (client, _) = Client::builder()
            .with_runtime_arc(Arc::new(crate::TokioRuntime))
            .with_persistence_manager(pm)
            .with_transport_factory_arc(Arc::new(
                crate::transport::mock::MockTransportFactory::new(),
            ))
            .with_http_client_arc(Arc::new(crate::test_utils::MockHttpClient))
            .with_cache_config(
                CacheConfig::default().with_lid_pn_cache(CacheEntryConfig::new(timeout, capacity)),
            )
            .build()
            .await
            .unwrap()
            .into_parts();
        client.enter_live_mode_for_tests();
        let rows: Vec<_> = (0..4)
            .map(|index| {
                (
                    format!("10000000000025{index}"),
                    format!("1202555025{index}"),
                )
            })
            .collect();
        let writer = {
            let client = client.clone();
            let rows = rows.clone();
            tokio::spawn(async move {
                client
                    .add_lid_pn_mappings(rows, crate::lid_pn_cache::LearningSource::Usync)
                    .await
            })
        };
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let mut resident = 0;
        for (lid, _) in &rows {
            resident += usize::from(client.lid_pn_cache.get_phone_number(lid).await.is_some());
        }
        assert!(
            resident < rows.len(),
            "the persistence batch must outlive evicted/expired rows"
        );
        release.send(()).await.unwrap();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(5), writer)
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            rows.len()
        );
        for (lid, phone) in &rows {
            assert_eq!(
                raw.get_lid_mapping(lid)
                    .await
                    .unwrap()
                    .unwrap()
                    .phone_number,
                *phone
            );
        }
        client.signal_shutdown_sync();
    }
}

async fn deliver_identity_reset(client: &Arc<Client>, user: &Jid, lid: Option<&Jid>) {
    use std::sync::atomic::Ordering;
    let mut node = NodeBuilder::new("notification")
        .attr("type", "encrypt")
        .attr("from", user.to_string())
        .attr("offline", "1")
        .children([NodeBuilder::new("identity").build()]);
    if let Some(lid) = lid {
        node = node.attr("lid", lid.to_string());
    }
    let node = node.build();
    let previous = client.offline_sync_completed.swap(false, Ordering::Relaxed);
    tokio::time::timeout(
        Duration::from_secs(5),
        crate::handlers::notification::handle_identity_change(client, &node.as_node_ref()),
    )
    .await
    .unwrap();
    client
        .offline_sync_completed
        .store(previous, Ordering::Relaxed);
}

/// Cancellation preserves the worker, not authority to reinstall a key after
/// an actual notification has retired it while that worker was queued.
#[tokio::test]
async fn adv_review_cancelled_bootstrap_cannot_restore_a_reset_primary() {
    for aliased in [false, true] {
        for reset in [true, false] {
            let (pending, waiting) = async_channel::unbounded();
            let (completed, finished) = async_channel::unbounded();
            let runtime = Arc::new(BootstrapWorkerRuntime {
                armed: std::sync::atomic::AtomicBool::new(false),
                pending,
                completed,
            });
            let (client, _) = crate::test_utils::create_iq_test_client_with_runtime(
                crate::test_utils::create_test_backend().await,
                runtime.clone(),
            )
            .await;
            let phone = Jid::pn("12025550130");
            let lid = Jid::lid("100000000000230");
            if aliased {
                client
                    .add_lid_pn_mapping(
                        &lid.user,
                        &phone.user,
                        crate::lid_pn_cache::LearningSource::Usync,
                    )
                    .await
                    .unwrap();
            }
            crate::test_utils::seed_adv_primary(&client, &phone).await;
            let key = client
                .cached_adv_account_identity(&phone)
                .await
                .unwrap()
                .unwrap();
            let primary = if aliased { &lid } else { &phone };
            let backend = client.persistence_manager.backend();
            client.signal_cache.flush(backend.as_ref()).await.unwrap();
            let mapping = client.lid_pn_cache.lock_mutation().await;
            runtime
                .armed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            let mut requester = Box::pin(client.persist_adv_bootstrap_identity(primary, &key));
            assert!(futures::poll!(requester.as_mut()).is_pending());
            tokio::time::timeout(Duration::from_secs(5), waiting.recv())
                .await
                .unwrap()
                .unwrap();
            drop(requester);
            if reset {
                deliver_identity_reset(&client, &phone, aliased.then_some(&lid)).await;
                assert_eq!(
                    client.cached_adv_account_identity(&phone).await.unwrap(),
                    None
                );
                assert_eq!(
                    backend
                        .load_identity(phone.to_protocol_address().as_str())
                        .await
                        .unwrap(),
                    None
                );
            }
            drop(mapping);
            tokio::time::timeout(Duration::from_secs(5), finished.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                client.cached_adv_account_identity(primary).await.unwrap(),
                (!reset).then_some(key),
                "aliased={aliased} reset={reset}"
            );
            assert_eq!(
                backend
                    .load_identity(primary.to_protocol_address().as_str())
                    .await
                    .unwrap(),
                (!reset).then_some(key)
            );
            if reset {
                let fresh = *KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
                    .public_key
                    .public_key_bytes();
                assert!(
                    client
                        .persist_adv_bootstrap_identity(primary, &fresh)
                        .await
                        .unwrap()
                );
                assert_eq!(
                    backend
                        .load_identity(primary.to_protocol_address().as_str())
                        .await
                        .unwrap(),
                    Some(fresh)
                );
            }
            client.signal_shutdown_sync();
        }
    }
}

/// A reply authenticated after its IQ crossed an identity notification must be
/// retried, including the no-prior-identity path which skips heavy cleanup.
#[tokio::test]
async fn adv_review_primary_iq_before_no_prior_notification_cannot_establish_trust() {
    let (client, transport) = create_iq_test_client().await;
    let user = Jid::pn("12025550131");
    let stale = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let operation = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move { client.ensure_adv_account_identity(&user).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    deliver_identity_reset(&client, &user, None).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&user, &stale, true)).await;
    let result = tokio::time::timeout(Duration::from_secs(5), operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&user).await.unwrap(),
        None,
        "a pre-notification reply must not establish new trust"
    );
    assert!(result.is_err());
    let fresh = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let retry = {
        let client = client.clone();
        let user = user.clone();
        tokio::spawn(async move { client.ensure_adv_account_identity(&user).await })
    };
    let iq = decode_sent_iq(&transport, 1).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&user, &fresh, true)).await;
    assert_eq!(
        retry.await.unwrap().unwrap(),
        *fresh.public_key.public_key_bytes()
    );
    assert_eq!(transport.sent().len(), 2);
    client.signal_shutdown_sync();
}

/// A one-sided cache eviction must not let a resolved alias outlive retirement.
#[tokio::test]
async fn adv_review_round2_pn_notification_retires_the_resolved_lid() {
    for (evict_reverse, explicit_lid) in [(true, false), (false, false), (true, true)] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550165");
        let lid = Jid::lid("100000000000265");
        client
            .add_lid_pn_mapping(
                &lid.user,
                &phone.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        if evict_reverse {
            client.lid_pn_cache.evict_reverse_for_tests(&lid.user).await;
        }
        assert_eq!(
            client.device_list_identity_alias(&phone).await,
            Some(lid.clone())
        );
        assert_eq!(
            client.device_list_identity_alias(&lid).await.is_none(),
            evict_reverse
        );
        let stale = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let operation = {
            let client = client.clone();
            let lid = lid.clone();
            tokio::spawn(async move { client.ensure_adv_account_identity(&lid).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        deliver_identity_reset(&client, &phone, explicit_lid.then_some(&lid)).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&lid, &stale, true)).await;
        let result = tokio::time::timeout(Duration::from_secs(5), operation)
            .await
            .unwrap()
            .unwrap();
        assert!(
            result.is_err(),
            "a PN notification must retire its resolved LID IQ"
        );
        assert_eq!(
            client.cached_adv_account_identity(&lid).await.unwrap(),
            None
        );
        assert!(
            client
                .persistence_manager
                .backend()
                .load_identity(lid.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        let fresh = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let retry = {
            let client = client.clone();
            let lid = lid.clone();
            tokio::spawn(async move { client.ensure_adv_account_identity(&lid).await })
        };
        let iq = decode_sent_iq(&transport, 1).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&lid, &fresh, true)).await;
        assert_eq!(
            retry.await.unwrap().unwrap(),
            *fresh.public_key.public_key_bytes()
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_round3_unknown_counterpart_retires_inflight_primary() {
    for reset_phone in [true, false] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550174");
        let lid = Jid::lid("100000000000274");
        let requested = if reset_phone { &lid } else { &phone };
        let reset = if reset_phone { &phone } else { &lid };
        assert!(client.device_list_identity_alias(requested).await.is_none());
        let stale = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let operation = {
            let client = client.clone();
            let requested = requested.clone();
            tokio::spawn(async move { client.ensure_adv_account_identity(&requested).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        deliver_identity_reset(&client, reset, None).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(requested, &stale, true)).await;
        let result = operation.await.unwrap();
        // The response did not disclose this relation before authentication.
        // Learn it through the real API only after the old IQ has completed.
        client
            .add_lid_pn_mapping(
                &lid.user,
                &phone.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        assert!(
            result.is_err(),
            "unknown counterpart must not authorize the pre-reset reply"
        );
        assert_eq!(
            client.cached_adv_account_identity(requested).await.unwrap(),
            None
        );
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_round3_ambiguous_reset_aliases_do_not_prove_the_counterpart() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    for case in ["reciprocal", "missing_reverse", "conflicting_reverse"] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550178");
        let old_lid = Jid::lid("100000000000279");
        let unknown_lid = Jid::lid("100000000000278");
        client
            .lid_pn_cache
            .add(&LidPnEntry::new(
                old_lid.user.as_str(),
                phone.user.as_str(),
                LearningSource::Usync,
            ))
            .await;
        if case == "missing_reverse" {
            client
                .lid_pn_cache
                .evict_reverse_for_tests(&old_lid.user)
                .await;
        } else if case == "conflicting_reverse" {
            client
                .lid_pn_cache
                .add(&LidPnEntry::new(
                    old_lid.user.as_str(),
                    "12025550180",
                    LearningSource::Usync,
                ))
                .await;
        }
        assert_eq!(
            client.device_list_identity_alias(&phone).await,
            Some(old_lid.clone())
        );
        assert_eq!(
            client.device_list_identity_alias(&old_lid).await.as_ref() == Some(&phone),
            case == "reciprocal"
        );
        let stale = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let operation = {
            let client = client.clone();
            let requested = unknown_lid.clone();
            tokio::spawn(async move { client.ensure_adv_account_identity(&requested).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        deliver_identity_reset(&client, &phone, None).await;
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(&unknown_lid, &stale, true)).await;
        let result = operation.await.unwrap();
        assert_eq!(result.is_ok(), case == "reciprocal", "scope case {case}");
        if case != "reciprocal" {
            // Disclose the actual counterpart only after the old request has
            // been rejected; cache ambiguity cannot authorize that earlier IQ.
            client
                .add_lid_pn_mapping(&unknown_lid.user, &phone.user, LearningSource::Usync)
                .await
                .unwrap();
            assert_eq!(
                client
                    .cached_adv_account_identity(&unknown_lid)
                    .await
                    .unwrap(),
                None
            );
            let fresh = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            let retry = {
                let client = client.clone();
                let requested = unknown_lid.clone();
                tokio::spawn(async move { client.ensure_adv_account_identity(&requested).await })
            };
            let iq = decode_sent_iq(&transport, 1).await;
            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
            answer_iq(&client, &id, &primary_response(&unknown_lid, &fresh, true)).await;
            assert_eq!(
                retry.await.unwrap().unwrap(),
                *fresh.public_key.public_key_bytes()
            );
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_round3_lid_reset_retires_an_existing_reciprocal_pn_anchor() {
    let (client, _) = create_iq_test_client().await;
    let phone = Jid::pn("12025550183");
    let lid = Jid::lid("100000000000283");
    client
        .add_lid_pn_mapping(
            &lid.user,
            &phone.user,
            crate::lid_pn_cache::LearningSource::Usync,
        )
        .await
        .unwrap();
    crate::test_utils::seed_adv_primary(&client, &phone).await;
    let backend = client.persistence_manager.backend();
    client.signal_cache.flush(backend.as_ref()).await.unwrap();
    deliver_identity_reset(&client, &lid, None).await;
    for user in [&phone, &lid] {
        assert_eq!(
            client.cached_adv_account_identity(user).await.unwrap(),
            None
        );
        assert!(
            backend
                .load_identity(user.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
    }
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_round3_alias_reset_retires_the_acknowledged_bootstrap() {
    for reset_known_alias in [true, false] {
        use crate::store::release::tests::ProbeBackend;
        use crate::test_utils::create_iq_test_client_with_backend;
        let key = *crate::test_utils::adv_test_account()
            .public_key
            .public_key_bytes();
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.detach_identity_writes = true;
        probe.identity_gate_key = Some(key);
        probe.gate_method = "put_identity";
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let phone = Jid::pn("12025550175");
        let lid = Jid::lid("100000000000275");
        let reset = if reset_known_alias {
            lid.clone()
        } else {
            Jid::lid("100000000000284")
        };
        client
            .add_lid_pn_mapping(
                &lid.user,
                &phone.user,
                crate::lid_pn_cache::LearningSource::Usync,
            )
            .await
            .unwrap();
        let epoch = client.signal_cache.identity_continuity.snapshot();
        let writer = {
            let client = client.clone();
            let phone = phone.clone();
            tokio::spawn(async move { client.persist_adv_bootstrap_identity(&phone, &key).await })
        };
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let notification = {
            let client = client.clone();
            let lid = reset.clone();
            tokio::spawn(async move { deliver_identity_reset(&client, &lid, None).await })
        };
        tokio::time::timeout(Duration::from_secs(5), async {
            while client
                .signal_cache
                .identity_continuity
                .unchanged_primary_for(epoch, [lid.user.as_str()])
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        release.send(()).await.unwrap();
        assert!(
            !tokio::time::timeout(Duration::from_secs(5), writer)
                .await
                .unwrap()
                .unwrap()
                .unwrap()
        );
        tokio::time::timeout(Duration::from_secs(5), notification)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            client.cached_adv_account_identity(&phone).await.unwrap(),
            None
        );
        assert!(
            raw.load_identity(phone.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        client.signal_cache.flush(raw.as_ref()).await.unwrap();
        assert!(
            raw.load_identity(phone.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        if !reset_known_alias {
            client
                .add_lid_pn_mapping(
                    &reset.user,
                    &phone.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
                .unwrap();
            assert_eq!(client.device_list_identity_alias(&phone).await, Some(reset));
        }
        client.signal_shutdown_sync();
    }
}

/// Moving a verified same-key account must preserve authenticated companion sessions.
#[tokio::test]
async fn adv_review_round2_known_same_key_migration_preserves_companion_authentication() {
    for case in ["same", "conflict", "unknown"] {
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550166");
        let lid = Jid::lid("100000000000266");
        let key = crate::test_utils::adv_test_account();
        if case != "unknown" {
            client
                .add_lid_pn_mapping(
                    &lid.user,
                    &phone.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
                .unwrap();
            crate::test_utils::seed_adv_primary(&client, &phone).await;
        }
        let primary = if case == "unknown" { &phone } else { &lid };
        let fetch = {
            let client = client.clone();
            let companion = primary.with_device(7);
            tokio::spawn(async move { client.fetch_pre_keys(&[companion], None).await })
        };
        let iq = decode_sent_iq(&transport, 0).await;
        if case == "unknown" {
            crate::test_utils::seed_adv_primary(&client, &phone).await;
        }
        if case == "conflict" {
            let changed = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            client
                .signal_cache
                .put_identity(
                    &lid.to_protocol_address(),
                    changed.public_key.public_key_bytes(),
                )
                .await;
        }
        tokio::time::timeout(Duration::from_secs(5), async {
            if case == "unknown" {
                // This control intentionally has no learned relation.
                // Exercise the explicit migration contract, which must
                // retire that authentication rather than invent an alias.
                client.migrate_signal_sessions(&phone, &lid).await;
                client
                    .signal_cache
                    .flush(client.persistence_manager.backend().as_ref())
                    .await
                    .unwrap();
            } else {
                client
                    .migrate_signal_sessions_on_lid_discovery(&phone.user, &lid.user)
                    .await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            client.cached_adv_account_identity(&phone).await.unwrap(),
            None
        );
        let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
        answer_iq(&client, &id, &primary_response(primary, key, true)).await;
        let result = tokio::time::timeout(Duration::from_secs(5), fetch)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            result.is_ok(),
            case == "same",
            "migration case {case}: {:?}",
            result.as_ref().err()
        );
        client.signal_shutdown_sync();
    }
}

/// Companion-only fetches also need the epoch from before their implicit
/// primary IQ, not a new epoch captured after the old reply has arrived.
#[tokio::test]
async fn adv_review_companion_primary_reply_cannot_revive_reset_identity() {
    let (client, transport) = create_iq_test_client().await;
    let phone = Jid::pn("12025550132");
    let lid = Jid::lid("100000000000232");
    client
        .add_lid_pn_mapping(
            &lid.user,
            &phone.user,
            crate::lid_pn_cache::LearningSource::Usync,
        )
        .await
        .unwrap();
    crate::test_utils::seed_adv_primary(&client, &phone).await;
    client
        .signal_cache
        .flush(client.persistence_manager.backend().as_ref())
        .await
        .unwrap();
    let stale = crate::test_utils::adv_test_account();
    let fetch = {
        let client = client.clone();
        let companion = lid.with_device(7);
        tokio::spawn(async move { client.fetch_pre_keys(&[companion], None).await })
    };
    let iq = decode_sent_iq(&transport, 0).await;
    deliver_identity_reset(&client, &phone, Some(&lid)).await;
    assert_eq!(
        client.cached_adv_account_identity(&phone).await.unwrap(),
        None
    );
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&lid, stale, true)).await;
    let result = tokio::time::timeout(Duration::from_secs(5), fetch)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&lid).await.unwrap(),
        None,
        "a pre-reset companion fetch must not restore its old primary"
    );
    assert!(result.is_err());
    let fresh = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let retry = {
        let client = client.clone();
        let companion = lid.with_device(7);
        tokio::spawn(async move { client.fetch_pre_keys(&[companion], None).await })
    };
    let iq = decode_sent_iq(&transport, 1).await;
    let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
    answer_iq(&client, &id, &primary_response(&lid, &fresh, true)).await;
    retry.await.unwrap().unwrap();
    assert_eq!(
        client.cached_adv_account_identity(&lid).await.unwrap(),
        Some(*fresh.public_key.public_key_bytes())
    );
    assert_eq!(transport.sent().len(), 2);
    client.signal_shutdown_sync();
}

/// A valid signature is insufficient when every projected device is revoked.
/// Learning the mapping first must not redirect a rejected PN's old fanout.
#[tokio::test]
async fn adv_review_empty_projection_does_not_publish_its_mapping() {
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    for group in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let phone = Jid::pn("12025550133");
        let lid = Jid::lid("100000000000233");
        for (user, companion) in [(&phone, 3), (&lid, 9)] {
            crate::test_utils::seed_adv_primary(&client, user).await;
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [
                        DeviceInfo::new(0, None),
                        DeviceInfo::new(companion, Some(1)),
                    ]
                    .into(),
                    raw_id: Some(1),
                    timestamp: 1,
                    phash: None,
                })
                .await
                .unwrap();
        }
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: phone.clone(),
                devices: vec![UsyncDevice::new(7, Some(9))],
                phash: None,
                key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
            }],
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.user.clone(),
                lid: lid.user.clone(),
            }],
        };
        let result = client
            .apply_device_list_response(
                &response,
                if group {
                    DeviceListApplyMode::GroupRefresh
                } else {
                    DeviceListApplyMode::StrictRefresh
                },
                None,
                group.then_some(std::slice::from_ref(&phone)),
            )
            .await;
        if group {
            let devices = result.unwrap().unwrap();
            assert!(devices.contains(&phone.with_device(3)));
            assert!(!devices.contains(&phone.with_device(9)));
        } else {
            assert!(result.is_err());
        }
        assert!(client.device_list_identity_alias(&phone).await.is_none());
        assert!(client.device_list_identity_alias(&lid).await.is_none());
        for (user, companion) in [(&phone, 3), (&lid, 9)] {
            let record = client.load_device_record_for_jid(user).await.unwrap();
            assert_eq!(
                record
                    .devices
                    .iter()
                    .map(|device| device.device_id())
                    .collect::<Vec<_>>(),
                [0, companion]
            );
        }
        client.signal_shutdown_sync();
    }
}

/// Registry IO must not expose a staged mapping before ownership admission succeeds.
#[tokio::test]
async fn adv_review_round2_registry_io_does_not_publish_unadmitted_mapping() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let failing = Arc::new(AtomicBool::new(false));
    let (release, gate) = async_channel::unbounded();
    let (entered, arrived) = async_channel::unbounded();
    probe.gate_method = "get_devices_batch";
    probe.gate = Some(gate);
    probe.entered = Some(entered);
    probe.fail_before_method = Some(("get_devices_batch", failing.clone()));
    let phone = Jid::pn("12025550178");
    let lid = Jid::lid("100000000000278");
    let peer = Jid::pn("12025550179");
    raw.update_device_list(DeviceListRecord {
        user: peer.user.as_str().into(),
        devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
        raw_id: Some(1),
        timestamp: 1,
        phash: None,
    })
    .await
    .unwrap();
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    crate::test_utils::seed_adv_primary(&client, &peer).await;
    let prepared = vec![(
        peer.clone(),
        PreparedAccountIdentity::Trusted(
            client
                .cached_adv_account_identity(&peer)
                .await
                .unwrap()
                .unwrap(),
        ),
    )];
    let response = DeviceListResponse {
        device_lists: vec![
            UserDeviceList {
                user: phone.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            },
            UserDeviceList {
                user: peer.clone(),
                devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(2, Some(1))],
                phash: None,
                key_index_bytes: Some(signed_key_index_for_epoch(vec![1], 1, 2)),
            },
        ],
        lid_mappings: vec![UsyncLidMapping {
            phone_number: phone.user.clone(),
            lid: lid.user.clone(),
        }],
    };
    let mut operation = {
        let client = client.clone();
        tokio::spawn(async move {
            client
                .apply_device_list_response_inner(
                    &response,
                    DeviceListApplyMode::CachePreferred,
                    None,
                    None,
                    DeviceListIdentityContext {
                        prepared: &prepared,
                        recovered: None,
                        publication_started: None,
                        authentication_epoch: None,
                        reactions: None,
                    },
                    None,
                )
                .await
        })
    };
    tokio::time::timeout(Duration::from_secs(5), arrived.recv())
        .await
        .unwrap()
        .unwrap();
    failing.store(true, Ordering::Relaxed);
    release.send(()).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(5), async {
        tokio::select! {
            result = &mut operation => result.unwrap(),
            second = arrived.recv() => {
                // Drive the old lossy path far enough to expose its partial
                // mapping: only staging fails, the processor's retry succeeds.
                second.unwrap();
                failing.store(false, Ordering::Relaxed);
                release.send(()).await.unwrap();
                (&mut operation).await.unwrap()
            }
        }
    })
    .await
    .unwrap();
    let error = result.unwrap_err();
    assert!(
        client.device_list_identity_alias(&phone).await.is_none(),
        "a failed staging read cannot prove that publishing the mapping needs no maintenance"
    );
    assert!(raw.get_lid_mapping(&lid.user).await.unwrap().is_none());
    assert!(raw.get_pn_mapping(&phone.user).await.unwrap().is_none());
    assert!(format!("{error:#}").contains("synthetic failure before commit"));
    assert_eq!(
        raw.get_devices(&peer.user).await.unwrap().unwrap().raw_id,
        Some(1)
    );
    client.signal_shutdown_sync();
}

#[tokio::test]
async fn adv_review_round2_cached_empty_projection_is_local_and_keeps_reset_cleanup() {
    use wacore::usync::{UserDeviceList, UsyncDevice, UsyncLidMapping};
    for changed_raw_id in [false, true] {
        let (client, _) = create_iq_test_client().await;
        let phone = Jid::pn("12025550169");
        let lid = Jid::lid("100000000000269");
        let empty = Jid::pn("12025550170");
        let empty_lid = Jid::lid("100000000000270");
        crate::test_utils::seed_adv_primary(&client, &empty).await;
        client
            .update_device_list(DeviceListRecord {
                user: empty.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(1))].into(),
                raw_id: Some(1),
                timestamp: 1,
                phash: None,
            })
            .await
            .unwrap();
        let response = DeviceListResponse {
            device_lists: vec![
                UserDeviceList {
                    user: phone.clone(),
                    devices: vec![UsyncDevice::new(0, None)],
                    phash: None,
                    key_index_bytes: None,
                },
                UserDeviceList {
                    user: empty.clone(),
                    devices: vec![UsyncDevice::new(7, Some(9))],
                    phash: None,
                    key_index_bytes: Some(signed_key_index_for_epoch(
                        vec![1],
                        1,
                        if changed_raw_id { 2 } else { 1 },
                    )),
                },
            ],
            lid_mappings: [(&phone, &lid), (&empty, &empty_lid)]
                .into_iter()
                .map(|(pn, lid)| UsyncLidMapping {
                    phone_number: pn.user.clone(),
                    lid: lid.user.clone(),
                })
                .collect(),
        };
        let devices = client
            .apply_device_list_response(&response, DeviceListApplyMode::CachePreferred, None, None)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(devices, vec![phone.clone()]);
        assert_eq!(
            client.device_list_identity_alias(&phone).await,
            Some(lid.clone())
        );
        assert_eq!(
            client
                .load_device_record_for_jid(&phone)
                .await
                .unwrap()
                .devices
                .len(),
            1
        );
        assert!(client.device_list_identity_alias(&empty).await.is_none());
        assert!(
            client
                .persistence_manager
                .backend()
                .get_lid_mapping(&empty_lid.user)
                .await
                .unwrap()
                .is_none()
        );
        let old = client.load_device_record_for_jid(&empty).await;
        if changed_raw_id {
            assert!(
                old.is_none(),
                "the rejected empty fanout must still retire its old raw-id epoch"
            );
        } else {
            let old = old.unwrap();
            assert_eq!(old.raw_id, Some(1));
            assert_eq!(
                old.devices
                    .iter()
                    .map(|d| d.device_id())
                    .collect::<Vec<_>>(),
                [0, 3]
            );
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_mapping_store_failure_leaves_the_pair_retryable() {
    use crate::lid_pn_cache::LearningSource;
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    for single in [false, true] {
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        let failing = Arc::new(AtomicBool::new(true));
        probe.fail_before_method = Some((
            if single {
                "put_lid_mapping"
            } else {
                "put_lid_mappings"
            },
            failing.clone(),
        ));
        let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let phone = "12025550151";
        let lid = "100000000000251";
        for fail in [true, false] {
            failing.store(fail, Ordering::Relaxed);
            let result = if single {
                client
                    .add_lid_pn_mapping(lid, phone, LearningSource::Usync)
                    .await
            } else {
                client
                    .add_lid_pn_mappings(
                        vec![(lid.to_owned(), phone.to_owned())],
                        LearningSource::Usync,
                    )
                    .await
                    .map(|_| ())
            };
            assert_eq!(result.is_err(), fail);
            assert_eq!(client.lid_pn_cache.is_persisted(phone, lid).await, !fail);
            assert_eq!(raw.get_lid_mapping(lid).await.unwrap().is_some(), !fail);
        }
        client.signal_shutdown_sync();
    }
}

#[tokio::test]
async fn adv_review_equal_timestamp_history_preserves_authenticated_rotation() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    use wacore::store::traits::LidPnMappingEntry;
    for by_lid in [false, true] {
        for case in ["tie", "cold", "unmarked", "foreign", "newer"] {
            let (client, transport) = create_iq_test_client().await;
            let backend = client.persistence_manager.backend();
            let phone = Jid::pn("12025550152");
            let lids = ["100000000000252", "100000000000253"];
            for lid in lids {
                backend
                    .put_lid_mapping(&LidPnMappingEntry {
                        lid: lid.into(),
                        phone_number: phone.user.to_string(),
                        created_at: 1000,
                        updated_at: 1000,
                        learning_source: "usync".into(),
                    })
                    .await
                    .unwrap();
            }
            // The backend is free to choose either historical row on a tie.
            // Make the other row the later, already-ACKed live observation.
            let historical = backend.get_pn_mapping(&phone.user).await.unwrap().unwrap();
            let lid = Jid::lid(lids.into_iter().find(|lid| *lid != historical.lid).unwrap());
            if case == "foreign" {
                backend
                    .put_lid_mapping(&LidPnMappingEntry {
                        lid: lid.user.to_string(),
                        phone_number: "12025550153".into(),
                        created_at: 1000,
                        updated_at: 1000,
                        learning_source: "usync".into(),
                    })
                    .await
                    .unwrap();
            }
            if case == "newer" {
                backend
                    .put_lid_mapping(&LidPnMappingEntry {
                        updated_at: 1001,
                        ..historical.clone()
                    })
                    .await
                    .unwrap();
            }
            if case != "cold" {
                client
                    .lid_pn_cache
                    .add(&LidPnEntry::with_timestamp(
                        lid.user.to_string(),
                        phone.user.to_string(),
                        1000,
                        LearningSource::Usync,
                    ))
                    .await;
                if case != "unmarked" {
                    client
                        .lid_pn_cache
                        .mark_persisted(
                            &Arc::from(phone.user.as_str()),
                            &Arc::from(lid.user.as_str()),
                        )
                        .await;
                }
            }
            for user in [&phone, &lid] {
                crate::test_utils::seed_adv_primary(&client, user).await;
            }
            let replacement = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
            let mut failure = first_mapping_failure(&client, &phone, &lid, &replacement);
            let user = if by_lid { &lid } else { &phone };
            if by_lid {
                failure.user = lid.clone();
                failure.response.device_lists[0].user = lid.clone();
            }
            let snapshot = client
                .capture_adv_recovery_snapshot(user, &failure.response)
                .await;
            assert_eq!(snapshot.is_ok(), case == "tie", "{case}, by_lid={by_lid}");
            assert!(transport.sent().is_empty());
            if case == "tie" {
                let recovery = {
                    let client = client.clone();
                    tokio::spawn(async move {
                        client.recover_ordinary_adv_identity(failure.into()).await
                    })
                };
                let iq = decode_sent_iq(&transport, 0).await;
                let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
                answer_iq(&client, &id, &primary_response(user, &replacement, true)).await;
                let devices = tokio::time::timeout(Duration::from_secs(5), recovery)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
                assert!(devices.iter().any(|device| device.device == 2));
                for endpoint in [&phone, &lid] {
                    assert_eq!(
                        client.cached_adv_account_identity(endpoint).await.unwrap(),
                        Some(*replacement.public_key.public_key_bytes())
                    );
                    assert_eq!(
                        client
                            .load_device_record_for_jid(endpoint)
                            .await
                            .unwrap()
                            .devices
                            .iter()
                            .map(|device| device.device_id())
                            .collect::<Vec<_>>(),
                        [0, 2]
                    );
                }
                assert_eq!(
                    backend
                        .get_pn_mapping(&phone.user)
                        .await
                        .unwrap()
                        .unwrap()
                        .lid,
                    historical.lid,
                    "rotation must not rewrite historical rows to hide the tie"
                );
            }
            client.signal_shutdown_sync();
        }
    }
}
