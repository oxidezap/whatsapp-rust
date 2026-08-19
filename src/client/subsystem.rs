//! The core's single attachment point for an optional subsystem.
//!
//! [`SUBSYSTEMS`] is the one place the core is allowed to name one. A subsystem
//! that passes the cut rule in `agent_docs/subsystem_boundary.md` parks its
//! per-client state here and lists the notification types it models, so turning
//! it on adds no `Client` field and no `cfg` anywhere else;
//! `tests/subsystem_boundary.rs` fails on a mention outside that budget.
//!
//! Everything here is written for a reader the core does not have: the table's
//! entries, the state it builds and the lookup that finds it again all belong
//! to the attached subsystems, so a build with none attached has no user for
//! any of it. That is the seam working, not rot, hence the module-wide allow.
#![allow(dead_code)]

use std::any::Any;
use std::sync::Arc;

use wacore::runtime::BoxFuture;
use wacore::stanza::wire_tags::NotificationType;
use wacore::stats::CollectionStats;
use wacore_binary::{NodeRef, OwnedNodeRef};

use super::Client;

/// Per-client state a subsystem parks on the core. `Send + Sync` only where the
/// client itself is: wasm32 is single-threaded and its handles may be `!Send`,
/// the same split `wacore::sync_marker` makes for trait objects.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type SubsystemState = Arc<dyn Any + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub(crate) type SubsystemState = Arc<dyn Any>;

/// What a subsystem reports as retained, named as `MemoryReport` prints it.
pub(crate) type RetainedCollections = Vec<(&'static str, CollectionStats)>;

/// One optional subsystem attached to the core.
///
/// Every hook is optional and every one is a plain function pointer, so an
/// unattached build folds the whole table away instead of paying for an empty
/// vtable.
pub(crate) struct Subsystem {
    /// Builds this subsystem's per-client state, once, during client assembly.
    pub state: fn() -> SubsystemState,
    /// The notification types this subsystem models. The core routes on the
    /// type alone and never looks inside the stanza.
    pub notifications: &'static [NotificationType],
    pub handle_notification:
        for<'a> fn(&'a Arc<Client>, NotificationType, Arc<OwnedNodeRef>) -> BoxFuture<'a, ()>,
    /// Runs while a connection's state is torn down, before the socket closes.
    pub on_connection_cleanup: Option<for<'a> fn(&'a Client) -> BoxFuture<'a, ()>>,
    /// Sees a response before the waiter it belongs to is woken. On the ack
    /// path and synchronous, so an implementation has to stay cheap; the point
    /// of running here rather than on the waiter's side is that the waking is
    /// what a subsystem would otherwise race.
    pub on_response: Option<fn(&Client, &NodeRef<'_>)>,
    /// Retained collections for `Client::memory_report`, named as the report
    /// prints them.
    pub memory: Option<fn(&Client) -> RetainedCollections>,
}

/// Every optional subsystem attached to this build.
pub(crate) const SUBSYSTEMS: &[Subsystem] = &[
    #[cfg(feature = "passkey")]
    crate::passkey::SUBSYSTEM,
    #[cfg(feature = "voip-runtime")]
    crate::voip::SUBSYSTEM,
];

/// The state of every attached subsystem, in [`SUBSYSTEMS`] order. Empty, and
/// allocation-free, in a build with none attached.
pub(crate) struct Subsystems {
    state: Box<[SubsystemState]>,
}

impl Default for Subsystems {
    fn default() -> Self {
        Self {
            state: SUBSYSTEMS
                .iter()
                .map(|subsystem| (subsystem.state)())
                .collect(),
        }
    }
}

impl Subsystems {
    /// The state parked by the subsystem that owns `T`, or `None` when it is
    /// not attached to this build.
    pub(crate) fn state<T: Any>(&self) -> Option<&T> {
        self.state
            .iter()
            .find_map(|state| state.downcast_ref::<T>())
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.state.len()
    }
}

/// Counts stanzas handed to a subsystem, so a test can tell routing through the
/// table apart from a core arm that quietly took the stanza instead.
///
/// Thread-local rather than a process-wide counter: a sibling test dispatching
/// the same type concurrently would otherwise satisfy the assertion on its own,
/// which is the failure the counter exists to catch.
#[cfg(test)]
thread_local! {
    pub(crate) static DISPATCHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Hands a notification to the subsystem that models its type. `false` means no
/// attached subsystem claims it, leaving the core's own fallback in charge.
pub(crate) async fn dispatch_notification(
    client: &Arc<Client>,
    notification_type: NotificationType,
    node: &Arc<OwnedNodeRef>,
) -> bool {
    for subsystem in SUBSYSTEMS {
        if subsystem.notifications.contains(&notification_type) {
            #[cfg(test)]
            DISPATCHED.with(|dispatched| dispatched.set(dispatched.get() + 1));
            (subsystem.handle_notification)(client, notification_type, Arc::clone(node)).await;
            return true;
        }
    }
    false
}

/// Lets every attached subsystem release what one connection owned.
pub(crate) async fn on_connection_cleanup(client: &Client) {
    for subsystem in SUBSYSTEMS {
        if let Some(hook) = subsystem.on_connection_cleanup {
            hook(client).await;
        }
    }
}

/// Offers a response to every attached subsystem before its waiter is woken.
pub(crate) fn on_response(client: &Client, node: &NodeRef<'_>) {
    for subsystem in SUBSYSTEMS {
        if let Some(hook) = subsystem.on_response {
            hook(client, node);
        }
    }
}

/// Collects what the attached subsystems retain, for the memory report.
pub(crate) fn memory(client: &Client) -> RetainedCollections {
    let mut collected = Vec::new();
    for subsystem in SUBSYSTEMS {
        if let Some(hook) = subsystem.memory {
            collected.extend(hook(client));
        }
    }
    collected
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Absent;

    #[test]
    fn detached_table_hands_out_no_state() {
        let subsystems = Subsystems {
            state: Box::new([]),
        };

        assert!(
            subsystems.state::<Absent>().is_none(),
            "a subsystem that is not attached must not resolve its state"
        );
    }

    #[test]
    fn every_attached_subsystem_gets_its_state_built() {
        assert_eq!(
            Subsystems::default().len(),
            SUBSYSTEMS.len(),
            "client assembly must build one state per attached subsystem"
        );
    }

    #[test]
    fn no_two_subsystems_claim_the_same_notification_type() {
        let mut claimed: Vec<NotificationType> = Vec::new();
        for subsystem in SUBSYSTEMS {
            for notification_type in subsystem.notifications {
                assert!(
                    !claimed.contains(notification_type),
                    "{notification_type:?} is claimed by two subsystems; dispatch would pick by table order"
                );
                claimed.push(*notification_type);
            }
        }
    }
}
