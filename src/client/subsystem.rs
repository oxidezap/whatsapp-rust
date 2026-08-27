//! The core's single attachment point for an optional subsystem.
//!
//! The [`subsystems!`] list below is the one place the core is allowed to name
//! one. A subsystem that passes the cut rule in
//! `agent_docs/subsystem_boundary.md` implements [`Subsystem`], parks its
//! per-client state through the associated type and lists the notification
//! types it models, so turning it on adds no `Client` field and no `cfg`
//! anywhere else; `tests/subsystem_boundary.rs` fails on a mention outside that
//! budget.
//!
//! Three items here have no user in a build with no subsystem attached: the two
//! traits and the lookup. That is the seam working rather than rot, so each
//! carries its own `allow` and the module carries none. `expect` would be the
//! better tool and does not fit: the items are live as soon as one subsystem is
//! attached, so the expectation would go unfulfilled in every other build.

use std::sync::Arc;

use wacore::stanza::wire_tags::NotificationType;
use wacore::stats::CollectionStats;
use wacore::sync_marker::MaybeSendSync;
use wacore_binary::{NodeRef, OwnedNodeRef};

use super::{Client, SubsystemMemory};

/// One optional subsystem the core can carry.
///
/// The implementing type is the whole handle: its state, the notification types
/// it claims and the hooks it fills all hang off it, so they cannot drift apart
/// the way a record of function pointers lets them. Nothing here is `dyn`, so a
/// hook a subsystem does not fill costs no branch and no vtable.
#[allow(async_fn_in_trait)] // never used through `dyn`: every call names a concrete impl, so auto traits still flow
#[allow(
    dead_code,
    reason = "no implementor in a build with no subsystem attached"
)]
pub(crate) trait Subsystem: 'static {
    /// What one client retains for this subsystem. Stored inline in `Client`
    /// under its real type, so reaching it back is a field access and not a
    /// downcast that has to answer for failing. `MaybeSendSync` is the bound
    /// the client itself carries: `Send + Sync` on native, dropped on wasm32.
    type State: Default + MaybeSendSync;

    /// How `Client::memory_report` names this subsystem's collections.
    const NAME: &'static str;

    /// The notification types this subsystem models. The core routes on the
    /// type alone and never looks inside the stanza. Two subsystems claiming
    /// one type fails the build; see the assertion below.
    const NOTIFICATIONS: &'static [NotificationType] = &[];

    /// Handles one of [`Self::NOTIFICATIONS`]. Never called for a type this
    /// subsystem did not claim, which is why claiming none needs no handler.
    async fn handle_notification(
        _client: &Arc<Client>,
        _notification_type: NotificationType,
        _node: Arc<OwnedNodeRef>,
    ) {
    }

    /// Runs while a connection's state is torn down, before the socket closes.
    async fn on_connection_cleanup(_client: &Client) {}

    /// Sees a response before the waiter it belongs to is woken. On the ack
    /// path and synchronous, so an implementation has to stay cheap; the point
    /// of running here rather than on the waiter's side is that the waking is
    /// what a subsystem would otherwise race.
    fn on_response(_client: &Client, _node: &NodeRef<'_>) {}

    /// Retained collections, named as `MemoryReport` prints them under
    /// [`Self::NAME`]. Takes the state rather than the client, so reporting
    /// cannot quietly become a second way for a subsystem to read the core.
    fn memory(_state: &Self::State) -> Vec<(&'static str, CollectionStats)> {
        Vec::new()
    }
}

/// Generates the core's whole side of the seam from one list of subsystems:
/// where their state lives, which of them a build can ask for, and the four
/// points the core calls out from.
///
/// The generated functions carry `unused_variables` because a build with no
/// subsystem attached expands every one of them to a body that reads none of
/// its arguments.
macro_rules! subsystems {
    ($($(#[$attr:meta])* $field:ident: $subsystem:ty),* $(,)?) => {
        /// The state of every subsystem attached to this build, each under its
        /// own type. No fields, and nothing to construct, when none is.
        #[derive(Default)]
        pub(crate) struct Subsystems {
            $($(#[$attr])* $field: <$subsystem as Subsystem>::State,)*
        }

        /// Implemented only for a subsystem this build attached. That is what
        /// makes [`Client::subsystem`] infallible: asking for a detached
        /// subsystem's state is a compile error, not a `None` or a panic every
        /// caller has to carry a story for.
        #[allow(dead_code, reason = "no implementor in a build with no subsystem attached")]
        pub(crate) trait Attached: Subsystem {
            fn state(subsystems: &Subsystems) -> &Self::State;
        }

        $(
            $(#[$attr])*
            impl Attached for $subsystem {
                fn state(subsystems: &Subsystems) -> &Self::State {
                    &subsystems.$field
                }
            }
        )*

        /// What each attached subsystem claims, in list order.
        pub(crate) const CLAIMS: &[&[NotificationType]] = &[
            $($(#[$attr])* <$subsystem as Subsystem>::NOTIFICATIONS,)*
        ];

        /// Hands a notification to the subsystem that models its type. `false`
        /// means no attached subsystem claims it, leaving the core's own
        /// fallback in charge.
        #[allow(unused_variables)]
        pub(crate) async fn dispatch_notification(
            client: &Arc<Client>,
            notification_type: NotificationType,
            node: &Arc<OwnedNodeRef>,
        ) -> bool {
            $(
                $(#[$attr])*
                if <$subsystem as Subsystem>::NOTIFICATIONS.contains(&notification_type) {
                    #[cfg(test)]
                    DISPATCHED.with(|dispatched| dispatched.set(dispatched.get() + 1));
                    <$subsystem as Subsystem>::handle_notification(
                        client,
                        notification_type,
                        Arc::clone(node),
                    )
                    .await;
                    return true;
                }
            )*
            false
        }

        /// Lets every attached subsystem release what one connection owned.
        #[allow(unused_variables)]
        pub(crate) async fn on_connection_cleanup(client: &Client) {
            $(
                $(#[$attr])*
                <$subsystem as Subsystem>::on_connection_cleanup(client).await;
            )*
        }

        /// Offers a response to every attached subsystem before its waiter is woken.
        #[allow(unused_variables)]
        pub(crate) fn on_response(client: &Client, node: &NodeRef<'_>) {
            $(
                $(#[$attr])*
                <$subsystem as Subsystem>::on_response(client, node);
            )*
        }

        /// Collects what the attached subsystems retain, for the memory report.
        #[allow(unused_variables, unused_mut)]
        pub(crate) fn memory(subsystems: &Subsystems) -> Vec<SubsystemMemory> {
            let mut retained = Vec::new();
            $(
                $(#[$attr])*
                retained.extend(
                    <$subsystem as Subsystem>::memory(&subsystems.$field)
                        .into_iter()
                        .map(|(collection, stats)| SubsystemMemory {
                            subsystem: <$subsystem as Subsystem>::NAME,
                            collection,
                            stats,
                        }),
                );
            )*
            retained
        }
    };
}

subsystems! {
    #[cfg(feature = "passkey")]
    passkey: crate::passkey::Passkey,
    #[cfg(feature = "voip-runtime")]
    voip: crate::voip::Voip,
}

/// Dispatch picks the first subsystem whose claim matches, so two of them
/// claiming one notification type would make routing depend on list order and
/// silently starve the loser. Checked here rather than in a test because the
/// list is a `const`: there is no configuration where this is a runtime
/// question.
const _: () = assert!(
    !claims_overlap(CLAIMS),
    "two subsystems claim the same notification type"
);

/// Whether any two of `claims` name one notification type. `const` and hence
/// written as index loops: iterators are not available here.
const fn claims_overlap(claims: &[&[NotificationType]]) -> bool {
    let mut left = 0;
    while left < claims.len() {
        let mut right = left + 1;
        while right < claims.len() {
            let mut this = 0;
            while this < claims[left].len() {
                let mut other = 0;
                while other < claims[right].len() {
                    if claims[left][this] as usize == claims[right][other] as usize {
                        return true;
                    }
                    other += 1;
                }
                this += 1;
            }
            right += 1;
        }
        left += 1;
    }
    false
}

impl Client {
    /// The state `S` parked during client assembly.
    ///
    /// Infallible by construction: [`Attached`] is implemented only for a
    /// subsystem this build carries, so naming a detached one does not compile.
    #[allow(dead_code, reason = "no caller in a build with no subsystem attached")]
    pub(crate) fn subsystem<S: Attached>(&self) -> &S::State {
        S::state(&self.subsystems)
    }
}

// Counts stanzas handed to a subsystem, so a test can tell routing through the
// seam apart from a core arm that quietly took the stanza instead. Thread-local
// rather than process-wide: a sibling test dispatching the same type would
// otherwise satisfy the assertion on its own, which is the failure it exists to
// catch. A `///` here would be an unused doc comment, since rustdoc does not
// document a macro invocation.
#[cfg(test)]
thread_local! {
    pub(crate) static DISPATCHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `const` assertion above only fires when two subsystems really do
    /// collide, which no shipping configuration does. This proves the check
    /// that guards them is not vacuously true.
    #[test]
    fn overlapping_claims_are_detected() {
        const A: &[NotificationType] = &[NotificationType::Devices, NotificationType::Picture];
        const B: &[NotificationType] = &[NotificationType::Picture];
        const C: &[NotificationType] = &[NotificationType::Contacts];

        assert!(
            claims_overlap(&[A, B]),
            "two lists sharing a type must be reported as overlapping"
        );
        assert!(
            !claims_overlap(&[A, C]),
            "disjoint lists must not be reported as overlapping"
        );
        assert!(
            !claims_overlap(CLAIMS),
            "the shipping list must stay free of overlap"
        );
    }
}
