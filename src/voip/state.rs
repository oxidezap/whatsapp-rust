//! What the VoIP runtime parks on the core, and the hooks it fills.
//!
//! Every one of these used to be a `Client` field: five of them, each with its
//! own `cfg`, plus the branches that built and tore them down. The core now
//! holds one attachment table (`src/client/subsystem.rs`) and this is VoIP's
//! entry in it, which is why nothing outside `src/voip/`, `src/client/voip.rs`
//! and `src/handlers/call.rs` names VoIP any more.

use std::collections::HashMap;
use std::sync::Arc;

use async_lock::Mutex;
use wacore::runtime::BoxFuture;
use wacore::stats::CollectionStats;
use wacore_binary::NodeRef;

use crate::client::Client;
use crate::client::subsystem::{RetainedCollections, Subsystem, SubsystemState};

/// How many lanes stripe [`VoipState::answer_transition_locks`].
const ANSWER_TRANSITION_LANES: usize = 16;

/// VoIP's entry in the core's attachment table. It claims no notification type:
/// calls arrive as `<call>` stanzas, which the stanza router already dispatches
/// to `CallHandler`.
pub(crate) const SUBSYSTEM: Subsystem = Subsystem {
    state: new_state,
    notifications: &[],
    handle_notification: never_dispatched,
    on_connection_cleanup: Some(on_connection_cleanup),
    on_response: Some(on_response),
    memory: Some(memory),
};

/// Everything one client retains for VoIP.
pub(crate) struct VoipState {
    /// Active calls and their media-task abort handles.
    pub(crate) call_registry: Arc<wacore::voip::CallRegistry>,
    /// Admission snapshots that can race a call-link join ACK before its call id is registered.
    /// Kept client-side so `wacore` never has to authorize a call it does not know.
    pub(crate) pending_call_link_joins:
        Arc<std::sync::Mutex<crate::client::voip::PendingCallLinkJoins>>,
    /// Serializes call-link joins until the ACK reveals which call id owns any admission state
    /// buffered during the request, so a bounded overflow stays tied to one join.
    pub(crate) pending_call_link_join_lane: Arc<Mutex<()>>,
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
            pending_call_link_join_lane: Arc::new(Mutex::new(())),
            answer_transition_locks: std::array::from_fn(|_| Arc::new(Mutex::new(()))),
            pending_outgoing_calls: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

fn new_state() -> SubsystemState {
    Arc::new(VoipState::default())
}

/// Unreachable: `notifications` is empty, so the core never routes one here.
fn never_dispatched<'a>(
    _client: &'a Arc<Client>,
    _notification_type: wacore::stanza::wire_tags::NotificationType,
    _node: Arc<wacore_binary::OwnedNodeRef>,
) -> BoxFuture<'a, ()> {
    Box::pin(async {})
}

/// The relay socket and signaling are connection-scoped, so no call survives a
/// disconnect or reconnect.
fn on_connection_cleanup(client: &Client) -> BoxFuture<'_, ()> {
    Box::pin(async move {
        client.voip_state().call_registry.abort_all();
        // Dormant outgoing calls (the relay never arrived) live in pending_outgoing_calls, not the
        // registry, so abort_all misses them. Drain them and notify `ended` so any waiter wakes.
        super::facade::drain_pending_outgoing_on_disconnect(client);
    })
}

fn on_response(client: &Client, node: &NodeRef<'_>) {
    client.bind_pending_call_link_join_ack(node);
}

fn memory(client: &Client) -> RetainedCollections {
    let voip = client.voip_state();
    let pending_link_updates = voip
        .pending_call_link_joins
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .memory_stats();
    let pending_outgoing = voip
        .pending_outgoing_calls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .len() as u64;
    vec![
        ("voip pending_link_updates:", pending_link_updates),
        ("voip active_calls:", voip.call_registry.memory_stats()),
        // Entry count only: each parked call retains engine material this side
        // cannot size without walking it.
        (
            "voip pending_outgoing:",
            CollectionStats::new(pending_outgoing, 0),
        ),
    ]
}

impl Client {
    /// The VoIP state parked during client assembly.
    ///
    /// Infallible by construction: the table entry above and every caller of
    /// this are behind the same `voip-runtime` gate, so a build that can reach
    /// it always attached it.
    pub(crate) fn voip_state(&self) -> &VoipState {
        self.subsystems
            .state::<VoipState>()
            .expect("the voip subsystem is attached whenever this code compiles")
    }
}
