//! Connection health decisions driven by caller-supplied monotonic time.

use super::keepalive::{KEEP_ALIVE_INTERVAL_MIN, elapsed_since_at, is_dead_socket_at};
use crate::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeOutcome {
    Ok,
    Skipped,
    TransientFailure,
    FatalFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthAction {
    Skip,
    Probe,
    Reconnect,
    Stop,
}

#[derive(Debug, Default)]
pub struct ConnectionHealth {
    failures: u32,
    failed_probe_deadline: Option<Instant>,
}

impl ConnectionHealth {
    pub fn failures(&self) -> u32 {
        self.failures
    }

    pub fn before_probe(
        &mut self,
        now: Instant,
        received: Option<Instant>,
        first_send: Option<Instant>,
    ) -> HealthAction {
        if elapsed_since_at(received, now).is_some_and(|age| age < KEEP_ALIVE_INTERVAL_MIN) {
            self.restore();
            return HealthAction::Skip;
        }
        if self.failures > 0
            && first_send == self.failed_probe_deadline
            && is_dead_socket_at(first_send, received, now)
        {
            return HealthAction::Reconnect;
        }
        HealthAction::Probe
    }

    pub fn probe_completed(
        &mut self,
        outcome: ProbeOutcome,
        now: Instant,
        received_before: Option<Instant>,
        received_after: Option<Instant>,
        first_send: Option<Instant>,
    ) -> HealthAction {
        match outcome {
            ProbeOutcome::Skipped => return HealthAction::Skip,
            ProbeOutcome::FatalFailure => return HealthAction::Stop,
            ProbeOutcome::Ok => self.restore(),
            ProbeOutcome::TransientFailure => {
                // An error response is receive activity too. Only the same
                // unanswered probe authorizes skipping a later probe.
                self.failed_probe_deadline = (received_before == received_after)
                    .then_some(first_send)
                    .flatten();
                self.failures = self.failures.saturating_add(1);
                if failures_are_terminal(self.failures) {
                    return HealthAction::Reconnect;
                }
            }
        }
        if is_dead_socket_at(first_send, received_after, now) {
            HealthAction::Reconnect
        } else {
            HealthAction::Skip
        }
    }

    fn restore(&mut self) {
        self.failures = 0;
        self.failed_probe_deadline = None;
    }
}

pub const fn failures_are_terminal(failures: u32) -> bool {
    failures >= 3
}

#[derive(Debug, Default)]
pub struct MaintenanceSchedule {
    retention: u32,
    session: u32,
    engine: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MaintenanceDue {
    pub retention: bool,
    pub session: bool,
    pub engine: bool,
}

impl MaintenanceSchedule {
    pub fn tick(&mut self, session_ticks: u32, engine_ticks: u32) -> MaintenanceDue {
        fn due(counter: &mut u32, period: u32) -> bool {
            *counter = counter.saturating_add(1);
            if *counter >= period {
                *counter = 0;
                true
            } else {
                false
            }
        }
        MaintenanceDue {
            retention: due(&mut self.retention, 12),
            session: due(&mut self.session, session_ticks),
            engine: due(&mut self.engine, engine_ticks),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(seconds: u64) -> Instant {
        Instant::ZERO + Duration::from_secs(seconds)
    }

    #[test]
    fn independent_health_states_need_no_global_clock() {
        let mut failed = ConnectionHealth::default();
        let mut healthy = ConnectionHealth::default();
        failed.probe_completed(ProbeOutcome::TransientFailure, at(10), None, None, None);
        healthy.probe_completed(ProbeOutcome::Ok, at(900), None, Some(at(900)), None);
        assert_eq!(failed.failures(), 1);
        assert_eq!(healthy.failures(), 0);
        assert_eq!(failed.before_probe(at(15), None, None), HealthAction::Probe);
        assert_eq!(
            healthy.before_probe(at(901), Some(at(900)), None),
            HealthAction::Skip
        );
    }

    #[test]
    fn only_a_failed_probe_for_the_same_deadline_skips_the_next_probe() {
        let mut health = ConnectionHealth::default();
        assert_eq!(
            health.before_probe(at(40), None, Some(at(1))),
            HealthAction::Probe
        );
        health.probe_completed(
            ProbeOutcome::TransientFailure,
            at(10),
            None,
            None,
            Some(at(1)),
        );
        assert_eq!(
            health.before_probe(at(21), None, Some(at(1))),
            HealthAction::Probe
        );
        assert_eq!(
            health.before_probe(at(22), None, Some(at(1))),
            HealthAction::Reconnect
        );
        assert_eq!(
            health.before_probe(at(50), Some(at(25)), Some(at(26))),
            HealthAction::Probe
        );
    }

    #[test]
    fn skipped_and_fatal_probes_do_not_authorize_watchdog_reconnect() {
        for (outcome, action) in [
            (ProbeOutcome::Skipped, HealthAction::Skip),
            (ProbeOutcome::FatalFailure, HealthAction::Stop),
        ] {
            let mut health = ConnectionHealth::default();
            assert_eq!(
                health.probe_completed(outcome, at(100), None, None, Some(at(1))),
                action
            );
            assert_eq!(health.failures(), 0);
        }
    }

    #[test]
    fn error_response_does_not_record_an_unanswered_deadline() {
        let mut health = ConnectionHealth::default();
        health.probe_completed(
            ProbeOutcome::TransientFailure,
            at(30),
            None,
            Some(at(29)),
            Some(at(1)),
        );
        assert_eq!(
            health.before_probe(at(100), Some(at(29)), Some(at(30))),
            HealthAction::Probe
        );
    }

    #[test]
    fn recent_activity_and_pong_restore_health() {
        for outcome in [ProbeOutcome::Ok, ProbeOutcome::TransientFailure] {
            let mut health = ConnectionHealth::default();
            health.probe_completed(ProbeOutcome::TransientFailure, at(10), None, None, None);
            if outcome == ProbeOutcome::Ok {
                health.probe_completed(outcome, at(20), None, Some(at(20)), None);
            } else {
                assert_eq!(
                    health.before_probe(at(21), Some(at(20)), None),
                    HealthAction::Skip
                );
            }
            assert_eq!(health.failures(), 0);
        }
    }

    #[test]
    fn three_failures_reconnect_even_without_an_armed_watchdog() {
        let mut health = ConnectionHealth::default();
        for i in 1..=3 {
            let action = health.probe_completed(
                ProbeOutcome::TransientFailure,
                at(i * 30),
                None,
                None,
                None,
            );
            assert_eq!(
                action,
                if i == 3 {
                    HealthAction::Reconnect
                } else {
                    HealthAction::Skip
                }
            );
        }
    }

    #[test]
    fn maintenance_runs_even_when_health_skips_pings() {
        let mut schedule = MaintenanceSchedule::default();
        for tick in 1..=120 {
            assert_eq!(
                schedule.tick(30, 20),
                MaintenanceDue {
                    retention: tick % 12 == 0,
                    session: tick % 30 == 0,
                    engine: tick % 20 == 0,
                }
            );
        }
    }
}
