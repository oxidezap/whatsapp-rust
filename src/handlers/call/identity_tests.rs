use super::*;
use crate::lid_pn_cache::LearningSource;
use crate::test_utils::{create_test_client, node_to_owned_ref};
use wacore::iq::abprops::web;
use wacore::types::events::ChannelEventHandler;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node};

const LID: &str = "100000000000001";
const OTHER_LID: &str = "100000000000002";
const PN: &str = "15550000001";

fn offer(from: Jid, creator: Jid, pn: Option<Jid>, username: Option<&str>, offline: bool) -> Node {
    let mut action = NodeBuilder::new("offer")
        .attr("call-id", "CALL-IDENTITY-TEST")
        .attr("call-creator", creator);
    if let Some(pn) = pn {
        action = action.attr("caller_pn", pn);
    }
    if let Some(username) = username {
        action = action.attr("username", username);
    }
    let mut stanza = NodeBuilder::new("call")
        .attr("from", from)
        .attr("id", "STANZA-IDENTITY-TEST")
        .attr("t", "1766847151")
        .children([action.build()]);
    if offline {
        stanza = stanza.attr("offline", "1");
    }
    stanza.build()
}

async fn deliver(client: &Arc<Client>, node: &Node) -> Vec<Arc<Event>> {
    let (handler, events) = ChannelEventHandler::new();
    let subscription = client.subscribe_handler(handler);
    let mut cancelled = false;
    assert!(
        CallHandler
            .handle(client.clone(), node_to_owned_ref(node), &mut cancelled)
            .await
    );
    assert!(
        !cancelled,
        "identity learning must not suppress the router ACK"
    );
    let result = std::iter::from_fn(|| events.try_recv().ok()).collect();
    drop(subscription);
    result
}

async fn assert_learned(client: &Client, lid: &str, pn: &str) {
    let entry = client
        .get_lid_pn_entry(&Jid::lid(lid))
        .await
        .unwrap()
        .expect("caller_pn must seed the incoming peer's LID");
    assert_eq!(&*entry.phone_number, pn);
    assert_eq!(entry.learning_source, LearningSource::Other);
    assert_eq!(
        client.lid_pn_cache.get_current_lid(pn).await.as_deref(),
        Some(lid)
    );
}

async fn assert_persisted(client: &Client, lid: &str, pn: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Ok(Some(entry)) = client
                .persistence_manager
                .backend()
                .get_lid_mapping(lid)
                .await
            {
                assert_eq!(entry.phone_number, pn);
                assert_eq!(entry.learning_source, "other");
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("online call mapping must reach persistent storage");
}

#[tokio::test]
async fn offer_learns_device_jids_and_persists_without_prior_message() {
    let client = create_test_client().await;
    let node = offer(
        Jid::lid(LID).with_device(2),
        Jid::lid(LID),
        Some(Jid::pn(PN).with_device(3)),
        None,
        false,
    );
    let events = deliver(&client, &node).await;
    assert_eq!(events.len(), 1);
    assert!(matches!(&*events[0], Event::IncomingCall(_)));
    assert_learned(&client, LID, PN).await;
    assert_persisted(&client, LID, PN).await;
}

#[tokio::test]
async fn offer_maps_from_instead_of_a_different_call_creator() {
    let client = create_test_client().await;
    deliver(
        &client,
        &offer(
            Jid::lid(LID),
            Jid::lid(OTHER_LID),
            Some(Jid::pn(PN)),
            None,
            true,
        ),
    )
    .await;
    assert_learned(&client, LID, PN).await;
    assert!(
        client
            .get_lid_pn_entry(&Jid::lid(OTHER_LID))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn offline_offer_learns_without_ringing_and_live_replay_persists() {
    let client = create_test_client().await;
    let events = deliver(
        &client,
        &offer(Jid::lid(LID), Jid::lid(LID), Some(Jid::pn(PN)), None, true),
    )
    .await;
    assert_eq!(events.len(), 1);
    assert!(matches!(&*events[0], Event::MissedCall(_)));
    assert_learned(&client, LID, PN).await;
    assert!(
        client
            .persistence_manager
            .backend()
            .get_lid_mapping(LID)
            .await
            .unwrap()
            .is_none()
    );
    deliver(
        &client,
        &offer(Jid::lid(LID), Jid::lid(LID), Some(Jid::pn(PN)), None, false),
    )
    .await;
    assert_persisted(&client, LID, PN).await;
}

#[tokio::test]
async fn offer_accepts_hosted_identity_families() {
    let client = create_test_client().await;
    deliver(
        &client,
        &offer(
            Jid::new(LID, Server::HostedLid).with_device(99),
            Jid::lid(LID),
            Some(Jid::new(PN, Server::Hosted).with_device(99)),
            None,
            true,
        ),
    )
    .await;
    assert_learned(&client, LID, PN).await;
}

#[tokio::test]
async fn offer_ignores_missing_or_non_phone_identity_and_non_lid_sender() {
    for (from, pn) in [
        (Jid::lid(LID), None),
        (Jid::lid(LID), Some(Jid::lid(OTHER_LID))),
        (Jid::lid(LID), Some(Jid::group("100-200"))),
        (Jid::pn(PN), Some(Jid::pn(PN))),
        (Jid::new("GROUP-CALL", Server::Call), Some(Jid::pn(PN))),
    ] {
        let client = create_test_client().await;
        let events = deliver(&client, &offer(from, Jid::lid(LID), pn, None, true)).await;
        assert_eq!(events.len(), 1);
        assert!(
            client
                .get_lid_pn_entry(&Jid::lid(LID))
                .await
                .unwrap()
                .is_none()
        );
        assert!(client.lid_pn_cache.get_current_lid(PN).await.is_none());
    }
}

#[tokio::test]
async fn offer_preserves_a_known_conflicting_mapping() {
    let client = create_test_client().await;
    let known_pn = "15550000002";
    client
        .add_lid_pn_mapping(LID, known_pn, LearningSource::Usync)
        .await
        .unwrap();
    deliver(
        &client,
        &offer(Jid::lid(LID), Jid::lid(LID), Some(Jid::pn(PN)), None, true),
    )
    .await;
    let entry = client
        .get_lid_pn_entry(&Jid::lid(LID))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(&*entry.phone_number, known_pn);
    assert_eq!(entry.learning_source, LearningSource::Usync);
    assert!(client.lid_pn_cache.get_current_lid(PN).await.is_none());
    assert_eq!(
        client
            .persistence_manager
            .backend()
            .get_lid_mapping(LID)
            .await
            .unwrap()
            .unwrap()
            .phone_number,
        known_pn
    );
}

#[tokio::test]
async fn offer_respects_username_privacy_flags_from_server_props() {
    for (display, privacy, username, should_learn) in [
        (true, true, Some("sample_user"), false),
        (false, false, Some("sample_user"), true),
        (true, true, Some(""), true),
        (true, true, None, true),
        (false, true, Some("sample_user"), true),
        (true, false, Some("sample_user"), true),
    ] {
        let client = create_test_client().await;
        // No watch() here: production must retain both server flags itself.
        client
            .ab_props
            .apply_props(
                false,
                [
                    (
                        web::USERNAME_CONTACT_DISPLAY.code,
                        if display { "1" } else { "0" }.into(),
                    ),
                    (
                        web::ENABLE_CALLING_PHONE_NUMBER_PRIVACY.code,
                        if privacy { "1" } else { "0" }.into(),
                    ),
                ]
                .into_iter(),
            )
            .await;
        let events = deliver(
            &client,
            &offer(
                Jid::lid(LID),
                Jid::lid(LID),
                Some(Jid::pn(PN)),
                username,
                true,
            ),
        )
        .await;
        assert_eq!(
            events.len(),
            1,
            "privacy gating must not drop the call event"
        );
        let entry = client.get_lid_pn_entry(&Jid::lid(LID)).await.unwrap();
        assert_eq!(
            entry.is_some(),
            should_learn,
            "display={display}, privacy={privacy}, username={username:?}"
        );
    }
}

#[tokio::test]
async fn non_offer_does_not_learn_an_unmodeled_caller_pn() {
    let client = create_test_client().await;
    let node = NodeBuilder::new("call")
        .attr("from", Jid::lid(LID))
        .attr("id", "STANZA-NON-OFFER")
        .attr("t", "1766847151")
        .children([NodeBuilder::new("terminate")
            .attr("call-creator", Jid::lid(LID))
            .attr("call-id", "CALL-NON-OFFER")
            .attr("reason", "timeout")
            .attr("caller_pn", Jid::pn(PN))
            .build()])
        .build();
    deliver(&client, &node).await;
    assert!(
        client
            .get_lid_pn_entry(&Jid::lid(LID))
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn identity_is_available_inside_the_call_event_callback() {
    use futures::FutureExt;
    use wacore::types::events::EventHandler;

    struct CheckAtDispatch(std::sync::Weak<Client>);
    impl EventHandler for CheckAtDispatch {
        fn handle_event(&self, event: Arc<Event>) {
            if matches!(&*event, Event::MissedCall(_)) {
                let client = self.0.upgrade().unwrap();
                // Offline delivery has no background writer. Poll the cache once,
                // without blocking this synchronous callback or its runtime.
                let entry = client
                    .lid_pn_cache
                    .get_entry_by_lid(LID)
                    .now_or_never()
                    .flatten();
                assert_eq!(
                    entry.as_ref().map(|entry| &*entry.phone_number),
                    Some(PN),
                    "call identity must be visible before event dispatch"
                );
            }
        }
    }

    let client = create_test_client().await;
    let _subscription =
        client.subscribe_handler(Arc::new(CheckAtDispatch(Arc::downgrade(&client))));
    let events = deliver(
        &client,
        &offer(Jid::lid(LID), Jid::lid(LID), Some(Jid::pn(PN)), None, true),
    )
    .await;
    assert_eq!(events.len(), 1);
    assert!(matches!(&*events[0], Event::MissedCall(_)));
}

#[cfg(feature = "voip-control")]
#[tokio::test]
async fn ringing_is_registered_before_identity_learning_can_suspend() {
    let client = create_test_client().await;
    let guard = client.lid_pn_cache.lock_mutation().await;
    let node = node_to_owned_ref(&offer(
        Jid::lid(LID),
        Jid::lid(LID),
        Some(Jid::pn(PN)),
        None,
        false,
    ));
    let mut cancelled = false;
    let mut handling = Box::pin(CallHandler.handle(client.clone(), node, &mut cancelled));
    assert!(futures::poll!(handling.as_mut()).is_pending());
    assert!(
        client.call_registry().take_ringing("CALL-IDENTITY-TEST"),
        "a racing terminate must already see the unanswered offer while identity learning waits"
    );
    drop(guard);
    assert!(handling.await);
    assert!(
        !client.call_registry().take_ringing("CALL-IDENTITY-TEST"),
        "resuming identity learning must not recreate a ringing flag consumed by terminate"
    );
    assert_learned(&client, LID, PN).await;
}
