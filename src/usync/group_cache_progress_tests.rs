use super::tests::{adv_wire_response, signed_key_index_bytes};
use super::*;
use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq, seed_adv_primary};
use std::time::Duration;
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore_binary::builder::NodeBuilder;

#[tokio::test]
async fn cold_group_registry_progress_preserves_current_devices_and_strict_refresh_fences() {
    for freshness in [
        crate::cache::Freshness::CachePreferred,
        crate::cache::Freshness::Refresh,
    ] {
        for complete_fill in [false, true] {
            let (client, transport) = create_iq_test_client().await;
            let first = Jid::pn("12025550131");
            let second = Jid::pn("12025550132");
            for user in [&first, &second] {
                seed_adv_primary(&client, user).await;
            }
            let running = client.clone();
            let users = vec![first.clone(), second.clone()];
            let mut operation = tokio::spawn(async move {
                match freshness {
                    crate::cache::Freshness::CachePreferred => {
                        running.get_group_user_devices_owned(users).await
                    }
                    crate::cache::Freshness::Refresh => running
                        .refresh_group_user_devices(users)
                        .await
                        .map(|devices| (devices, true)),
                }
            });
            let mut requests = 0;
            let result = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    tokio::select! {
                        result = &mut operation => break result.unwrap(),
                        iq = decode_sent_iq(&transport, requests) => {
                            assert!(requests < DEVICE_REFRESH_MAX_ATTEMPTS);
                            let requested: Vec<_> = iq.get().get_optional_child("usync").unwrap()
                                .get_optional_child("list").unwrap().children().unwrap().iter()
                                .map(|user| user.attrs().optional_jid("jid").unwrap()).collect();
                            assert!(requested.iter().all(|user| user == &first || user == &second));
                            if requested.contains(&first) {
                                // Discovery can publish current registry data while an IQ is in flight.
                                // A request that still includes this resolved contact races that publication again.
                                for user in std::iter::once(&first).chain(complete_fill.then_some(&second)) {
                                    client.update_device_list(DeviceListRecord {
                                        user: user.user.as_str().into(),
                                        devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                                        raw_id: Some(1), timestamp: 2, phash: None,
                                    }).await.unwrap();
                                }
                            } else {
                                assert_eq!(requested, vec![second.clone()]);
                            }
                            let rows: Vec<_> = requested.iter().map(|user| {
                                adv_wire_response(user, signed_key_index_bytes(vec![1], 1))
                                    .get_optional_child("usync").unwrap().get_optional_child("list").unwrap()
                                    .children().unwrap()[0].clone()
                            }).collect();
                            let response = NodeBuilder::new("iq").attr("type", "result")
                                .children([NodeBuilder::new("usync")
                                    .children([NodeBuilder::new("list").children(rows).build()]).build()]).build();
                            let id = iq.get().attrs().optional_string("id").unwrap().into_owned();
                            requests += 1;
                            answer_iq(&client, &id, &response).await;
                        }
                    }
                }
            }).await.unwrap();
            client.signal_shutdown_sync();
            match freshness {
                crate::cache::Freshness::CachePreferred => {
                    let (devices, complete) = result.unwrap();
                    assert!(complete);
                    assert!(devices.contains(&first.with_device(7)));
                    assert!(
                        !devices.contains(&first.with_device(2)),
                        "the rejected reply must not replace the current registry"
                    );
                    assert!(devices.contains(&second.with_device(if complete_fill {
                        7
                    } else {
                        2
                    })));
                    assert_eq!(requests, if complete_fill { 1 } else { 2 });
                }
                crate::cache::Freshness::Refresh => {
                    assert!(
                        result
                            .unwrap_err()
                            .to_string()
                            .contains("device registry kept changing")
                    );
                    assert_eq!(requests, DEVICE_REFRESH_MAX_ATTEMPTS);
                }
            }
        }
    }
}
