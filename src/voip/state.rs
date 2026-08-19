//! What the VoIP runtime parks on the core, and the hooks it fills.
//!
//! Every one of these used to be a `Client` field: five of them, each with its
//! own `cfg`, plus the branches that built and tore them down. They are now one
//! `Subsystem` impl (`src/client/subsystem.rs`), which is why nothing outside
//! `src/voip/`, `src/client/voip.rs` and `src/handlers/call.rs` names VoIP any
//! more.

use std::collections::HashMap;
use std::sync::Arc;

use async_lock::Mutex;
use wacore::stats::CollectionStats;
use wacore_binary::NodeRef;

use crate::client::Client;
use crate::client::subsystem::Subsystem;

/// What this subsystem reports to `Client::memory_report`, named once so a
/// lookup and the hook below cannot drift apart, and so a caller spells a
/// constant instead of a string.
pub mod collections {
    use super::Voip;
    use crate::client::SubsystemCollection;
    use crate::client::subsystem::Subsystem;

    /// Admission snapshots retained while a call-link join ACK is in flight.
    pub const PENDING_LINK_UPDATES: SubsystemCollection =
        SubsystemCollection::new(Voip::NAME, "pending_link_updates");
    /// Active and ringing calls, with their bounded pre-offer group controls.
    pub const ACTIVE_CALLS: SubsystemCollection =
        SubsystemCollection::new(Voip::NAME, "active_calls");
    /// Outgoing calls parked until the server sends the relay that owns them.
    pub const PENDING_OUTGOING: SubsystemCollection =
        SubsystemCollection::new(Voip::NAME, "pending_outgoing");
}

/// How many lanes stripe [`VoipState::answer_transition_locks`].
const ANSWER_TRANSITION_LANES: usize = 16;

/// VoIP's attachment to the core. It claims no notification type: calls arrive
/// as `<call>` stanzas, which the stanza router already dispatches to
/// `CallHandler`, so the two hooks below are all it takes from the core.
pub(crate) struct Voip;

impl Subsystem for Voip {
    type State = VoipState;

    const NAME: &'static str = "voip";

    /// The relay socket and signaling are connection-scoped, so no call
    /// survives a disconnect or reconnect.
    async fn on_connection_cleanup(client: &Client) {
        client.voip_state().call_registry.abort_all();
        // Dormant outgoing calls (the relay never arrived) live in pending_outgoing_calls, not the
        // registry, so abort_all misses them. Drain them and notify `ended` so any waiter wakes.
        super::facade::drain_pending_outgoing_on_disconnect(client);
    }

    fn on_response(client: &Client, node: &NodeRef<'_>) {
        client.bind_pending_call_link_join_ack(node);
    }

    fn memory(state: &VoipState) -> Vec<(&'static str, CollectionStats)> {
        let pending_link_updates = state
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .memory_stats();
        let pending_outgoing = state
            .pending_outgoing_calls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len() as u64;
        vec![
            (
                collections::PENDING_LINK_UPDATES.collection,
                pending_link_updates,
            ),
            (
                collections::ACTIVE_CALLS.collection,
                state.call_registry.memory_stats(),
            ),
            // Entry count only: each parked call retains engine material this
            // side cannot size without walking it.
            (
                collections::PENDING_OUTGOING.collection,
                CollectionStats::new(pending_outgoing, 0),
            ),
        ]
    }
}

/// Everything one client retains for VoIP.
pub(crate) struct VoipState {
    /// Active calls and their media-task abort handles.
    pub(crate) call_registry: Arc<wacore::voip::CallRegistry>,
    /// Admission snapshots that can race a call-link join ACK before its call id is registered.
    /// Kept client-side so `wacore` never has to authorize a call it does not know.
    pub(crate) pending_call_link_joins:
        Arc<std::sync::Mutex<crate::client::voip::PendingCallLinkJoins>>,
    /// Serializes call-link joins until the ACK reveals which call id owns any admission state
    /// buffered during the request, so a bounded overflow stays tied to one join. Not shared: the
    /// one caller locks it through `&Client`, unlike the striped answer lanes, which hand out an
    /// owned guard via `lock_arc` and so have to be `Arc`.
    pub(crate) pending_call_link_join_lane: Mutex<()>,
    /// Serializes incoming-answer registration with generation-aware teardown. A failed answer
    /// holds its call-id lane until `<terminate>` has been written, so a same-call-id re-offer
    /// cannot become current in the removal-before-send window.
    pub(crate) answer_transition_locks: [Arc<Mutex<()>>; ANSWER_TRANSITION_LANES],
    /// Outgoing calls awaiting their relay. The initiator's relay is not in the offer; it arrives
    /// from the server AFTER it, so each `voip().call()` parks the material needed to spawn the
    /// engine here, keyed by call-id, until a `<call>` carrying a `<relay>` for that id arrives.
    pub(crate) pending_outgoing_calls:
        Arc<std::sync::Mutex<HashMap<String, super::facade::PendingOutgoing>>>,
}

impl Default for VoipState {
    fn default() -> Self {
        Self {
            call_registry: Arc::new(wacore::voip::CallRegistry::new()),
            pending_call_link_joins: Arc::new(std::sync::Mutex::new(Default::default())),
            pending_call_link_join_lane: Mutex::new(()),
            answer_transition_locks: std::array::from_fn(|_| Arc::new(Mutex::new(()))),
            pending_outgoing_calls: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

impl Client {
    /// The VoIP state parked during client assembly. Named `voip_state` because
    /// `Client::voip()` is already the public facade accessor.
    pub(crate) fn voip_state(&self) -> &VoipState {
        self.subsystem::<Voip>()
    }
}
