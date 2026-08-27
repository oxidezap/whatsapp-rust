//! Whether a record leases outbound counters ahead of durability.
//!
//! Shared by `SessionRecord` and `SenderKeyRecord`, which lease the same way
//! over different units. What the policy means to a consumer, and what waiving
//! costs it, is documented on `SessionRecord::waive_counter_lease`, the public
//! surface that offers the choice.

use crate::protocol::consts;

/// Reservation state, or the consumer's declaration that it needs none.
///
/// `Waived` carries no ceiling and no pending flag, so a record cannot hold a
/// reservation the send path would gate on while also having waived the lease
/// that reservation implements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CounterLease {
    Leased {
        /// Exclusive ceiling of counters a durable snapshot may already have
        /// published.
        ceiling: u32,
        /// A reservation was raised but not yet durably flushed. While set,
        /// the owning ciphertext must not reach the wire.
        pending_flush: bool,
    },
    Waived,
}

impl Default for CounterLease {
    fn default() -> Self {
        Self::Leased {
            ceiling: 0,
            pending_flush: false,
        }
    }
}

impl CounterLease {
    pub(crate) fn from_persisted_ceiling(ceiling: u32) -> Self {
        Self::Leased {
            ceiling,
            pending_flush: false,
        }
    }

    /// Exclusive ceiling of the current reservation; zero when nothing is
    /// reserved, which is also what a waived lease reports, since there is
    /// nothing for a reload or an export to advance past.
    pub(crate) fn ceiling(self) -> u32 {
        match self {
            Self::Leased { ceiling, .. } => ceiling,
            Self::Waived => 0,
        }
    }

    pub(crate) fn is_pending_flush(self) -> bool {
        matches!(
            self,
            Self::Leased {
                pending_flush: true,
                ..
            }
        )
    }

    /// Lease a fresh batch after `spent_counter` was issued.
    ///
    /// Reservations only ever rise: a counter must not be published under a
    /// ceiling lower than one a durable snapshot already carries, so a spent
    /// counter still inside the current batch changes nothing.
    pub(crate) fn reserve(&mut self, spent_counter: u32) {
        if let Self::Leased {
            ceiling,
            pending_flush,
        } = self
            && spent_counter >= *ceiling
        {
            *ceiling = spent_counter.saturating_add(consts::SENDER_CHAIN_RESERVATION_BATCH);
            *pending_flush = true;
        }
    }

    pub(crate) fn set_pending_flush(&mut self, pending: bool) {
        if let Self::Leased { pending_flush, .. } = self {
            *pending_flush = pending;
        }
    }

    /// Drop the reservation without changing whether the lease is in force.
    pub(crate) fn clear_reservation(&mut self) {
        if let Self::Leased { ceiling, .. } = self {
            *ceiling = 0;
        }
    }

    /// Cap the ceiling at one batch after a ratchet replaced the leased chain.
    pub(crate) fn rebase(&mut self) {
        if let Self::Leased { ceiling, .. } = self {
            *ceiling = (*ceiling).min(consts::SENDER_CHAIN_RESERVATION_BATCH);
        }
    }

    /// Drop the lease.
    ///
    /// The caller materializes [`Self::ceiling`] first and only gets here once
    /// that succeeded: a record loaded from a snapshot written while the lease
    /// was in force carries a reservation that may already have been published,
    /// and dropping the ceiling before advancing past it would leave the record
    /// free to reissue those counters.
    pub(crate) fn waive(&mut self) {
        *self = Self::Waived;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_waived_lease_reserves_nothing_and_never_gates_the_wire() {
        let mut lease = CounterLease::Waived;
        lease.reserve(0);
        lease.reserve(u32::MAX);

        assert_eq!(lease, CounterLease::Waived);
        assert_eq!(lease.ceiling(), 0);
        assert!(!lease.is_pending_flush());
    }

    #[test]
    fn a_leased_reservation_rises_and_gates_the_wire() {
        let mut lease = CounterLease::default();
        assert_eq!(lease.ceiling(), 0);
        assert!(!lease.is_pending_flush());

        lease.reserve(0);
        assert_eq!(lease.ceiling(), consts::SENDER_CHAIN_RESERVATION_BATCH);
        assert!(lease.is_pending_flush());

        // Inside the batch: nothing to raise, so the send stays uncovered by a
        // fresh gate.
        lease.set_pending_flush(false);
        lease.reserve(1);
        assert_eq!(lease.ceiling(), consts::SENDER_CHAIN_RESERVATION_BATCH);
        assert!(!lease.is_pending_flush());

        lease.reserve(consts::SENDER_CHAIN_RESERVATION_BATCH);
        assert_eq!(lease.ceiling(), consts::SENDER_CHAIN_RESERVATION_BATCH * 2);
        assert!(lease.is_pending_flush());
    }

    #[test]
    fn waiving_reports_nothing_left_to_materialize() {
        let mut lease = CounterLease::from_persisted_ceiling(512);
        assert_eq!(lease.ceiling(), 512);

        lease.waive();
        assert_eq!(lease, CounterLease::Waived);
        // Converged: nothing left for a later load to advance past.
        assert_eq!(lease.ceiling(), 0);
    }

    #[test]
    fn rebasing_a_waived_lease_is_inert() {
        let mut lease = CounterLease::Waived;
        lease.rebase();
        assert_eq!(lease, CounterLease::Waived);
    }
}
