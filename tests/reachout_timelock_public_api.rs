use std::future::Future;
use whatsapp_rust::ReachoutTimelock;
use whatsapp_rust::features::{Mex, MexError, ReachoutTimelock as FeatureReachoutTimelock};
use whatsapp_rust::types::events::{Event, EventInterest, EventKind, ReachoutTimelockUpdate};

fn accept_pull_state(_: &ReachoutTimelock) {}

#[test]
fn reachout_timelock_push_uses_the_existing_pull_type() {
    // Existing aliases and struct-literal construction remain source-compatible.
    let state = FeatureReachoutTimelock {
        enforcement_type: Some("BIZ_QUALITY".into()),
        is_active: Some(true),
        time_enforcement_ends: Some("1704153600".into()),
    };
    let event =
        Event::ReachoutTimelockUpdate(ReachoutTimelockUpdate::builder().state(state).build());
    let interest = EventInterest::of(&[EventKind::ReachoutTimelockUpdate]);
    assert!(interest.wants(event.kind()));
    match &event {
        Event::ReachoutTimelockUpdate(update) => {
            accept_pull_state(&update.state);
            assert_eq!(update.state.is_active, Some(true));
            assert!(update.from.is_none());
            assert!(update.stanza_id.is_none());
            assert!(update.offline.is_none());
        }
        _ => panic!("expected reachout update"),
    }
}

// Compilation checks the method's public return contract without making a query.
fn pull_return_contract<'a>(
    mex: &'a Mex<'a>,
) -> impl Future<Output = Result<ReachoutTimelock, MexError>> + 'a {
    mex.fetch_reachout_timelock()
}

#[test]
fn reachout_timelock_pull_return_contract_is_unchanged() {
    let _ = pull_return_contract;
}
