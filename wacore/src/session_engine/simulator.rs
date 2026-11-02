use super::lifecycle::{
    AttemptTicket, AuthenticationTicket, BackoffRequest, DriveKind, DriveLease, Lifecycle,
    LifecycleError, LifecycleSnapshot, NextConnection, TransportTicket,
};
use super::lifecycle::{CleanupTicket, WorkerGeneration};
use super::publication::{CommittingSignal, PreparedSignal, SignalCommitOutcome};
use crate::request::pending::PendingRequests;
use crate::time::Instant;
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeTicket {
    id: String,
    generation: NonZeroU64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalTicket(NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalDiscardReason {
    PersistenceFailed,
    Cancelled,
}

struct Exchange {
    generation: NonZeroU64,
    deadline: Instant,
    sent: bool,
    response: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
pub enum Input {
    Advance(Duration),
    StartDriver(DriveKind),
    CancelDriver(DriveLease),
    BeginCleanup(Option<TransportTicket>),
    CompleteCleanup(CleanupTicket),
    CancelCleanup(CleanupTicket),
    RetireWorkers,
    PrepareSignal(Vec<u8>),
    ConfirmSignal {
        ticket: SignalTicket,
        outcome: SignalCommitOutcome,
    },
    FailSignalCommit(SignalTicket),
    CancelSignal(SignalTicket),
    Connect,
    TransportOpened(AttemptTicket),
    HandshakeComplete(AttemptTicket),
    FinishAttempt(AttemptTicket),
    ClaimAuthentication,
    AdmitAuthentication(AuthenticationTicket),
    Ready(AuthenticationTicket),
    TransportLost(TransportTicket),
    Pause,
    Resume,
    Shutdown,
    NextConnection,
    SampleBackoff {
        request: BackoffRequest,
        sample: u64,
    },
    RequestReconnect {
        immediate: bool,
    },
    ReconnectPenalty(u32),
    RegisterExchange {
        id: String,
        timeout: Duration,
    },
    SendComplete(ExchangeTicket),
    SendFailed(ExchangeTicket),
    Response {
        ticket: ExchangeTicket,
        bytes: Vec<u8>,
    },
    CancelExchange(ExchangeTicket),
    DisconnectExchanges,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExchangeOutcome {
    Response(Vec<u8>),
    SendFailed,
    TimedOut,
    Cancelled,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Driver(DriveLease),
    Cleanup(CleanupTicket),
    WorkersRetired(WorkerGeneration),
    SignalPrepared(SignalTicket),
    PublishSignal {
        ticket: SignalTicket,
        bytes: Vec<u8>,
    },
    SignalDiscarded {
        ticket: SignalTicket,
        reason: SignalDiscardReason,
    },
    Attempt(AttemptTicket),
    Authentication(AuthenticationTicket),
    Connection(NextConnection),
    Backoff(Duration),
    ExchangeRegistered(ExchangeTicket),
    ExchangeFinished {
        ticket: ExchangeTicket,
        outcome: ExchangeOutcome,
    },
    Applied(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SimulationError {
    #[error("{0}")]
    Lifecycle(#[from] LifecycleError),
    #[error("virtual time overflow")]
    TimeOverflow,
    #[error("request ID is already registered")]
    DuplicateExchange,
    #[error("signal operation sequence exhausted")]
    SignalSequenceExhausted,
}

/// An executor-free session driver. Input order resolves simultaneous events.
/// Advancing time expires both sending and response-waiting exchanges.
pub struct SessionSimulator {
    lifecycle: Lifecycle,
    now: Instant,
    exchanges: PendingRequests<Exchange>,
    signals: HashMap<SignalTicket, CommittingSignal<Vec<u8>>>,
    next_signal: u64,
}

impl Default for SessionSimulator {
    fn default() -> Self {
        Self {
            lifecycle: Lifecycle::default(),
            now: Instant::ZERO,
            exchanges: PendingRequests::default(),
            signals: HashMap::new(),
            next_signal: 0,
        }
    }
}

impl SessionSimulator {
    pub fn now(&self) -> Instant {
        self.now
    }

    pub fn snapshot(&self) -> LifecycleSnapshot {
        self.lifecycle.snapshot()
    }

    pub fn pending_exchanges(&self) -> usize {
        self.exchanges.len()
    }

    pub fn step(&mut self, input: Input) -> Result<Vec<Output>, SimulationError> {
        let output = match input {
            Input::Advance(duration) => {
                self.now = self
                    .now
                    .checked_add(duration)
                    .ok_or(SimulationError::TimeOverflow)?;
                let now = self.now;
                let mut expired = Vec::new();
                self.exchanges.retain(|id, exchange| {
                    if exchange.deadline <= now {
                        expired.push(Output::ExchangeFinished {
                            ticket: ExchangeTicket {
                                id: id.to_owned(),
                                generation: exchange.generation,
                            },
                            outcome: ExchangeOutcome::TimedOut,
                        });
                        false
                    } else {
                        true
                    }
                });
                // HashMap iteration must not leak into replay order.
                expired.sort_by(|a, b| match (a, b) {
                    (
                        Output::ExchangeFinished { ticket: a, .. },
                        Output::ExchangeFinished { ticket: b, .. },
                    ) => a.id.cmp(&b.id),
                    _ => std::cmp::Ordering::Equal,
                });
                return Ok(expired);
            }
            Input::BeginCleanup(owner) => Output::Cleanup(self.lifecycle.begin_cleanup(owner)?),
            Input::CompleteCleanup(ticket) => {
                Output::Applied(self.lifecycle.complete_cleanup(ticket))
            }
            Input::CancelCleanup(ticket) => Output::Applied(self.lifecycle.cancel_cleanup(ticket)),
            Input::RetireWorkers => {
                Output::WorkersRetired(self.lifecycle.advance_worker_generation()?)
            }
            Input::PrepareSignal(bytes) => {
                self.next_signal = self
                    .next_signal
                    .checked_add(1)
                    .ok_or(SimulationError::SignalSequenceExhausted)?;
                let ticket = SignalTicket(
                    NonZeroU64::new(self.next_signal).expect("checked signal sequence"),
                );
                self.signals
                    .insert(ticket, PreparedSignal::new(bytes).begin_commit());
                Output::SignalPrepared(ticket)
            }
            Input::ConfirmSignal { ticket, outcome } => match self.signals.remove(&ticket) {
                Some(pending) => Output::PublishSignal {
                    ticket,
                    bytes: pending.confirm(outcome).into_payload(),
                },
                None => Output::Applied(false),
            },
            Input::FailSignalCommit(ticket) => {
                self.discard_signal(ticket, SignalDiscardReason::PersistenceFailed)
            }
            Input::CancelSignal(ticket) => {
                self.discard_signal(ticket, SignalDiscardReason::Cancelled)
            }
            Input::StartDriver(kind) => Output::Driver(self.lifecycle.start_drive(kind)?),
            Input::CancelDriver(lease) => Output::Applied(self.lifecycle.cancel_drive(lease)),
            Input::Connect => Output::Attempt(self.lifecycle.begin_attempt()?),
            Input::TransportOpened(ticket) => {
                self.lifecycle.transport_opened(ticket)?;
                Output::Applied(true)
            }
            Input::HandshakeComplete(ticket) => {
                self.lifecycle.handshake_ready(ticket)?;
                Output::Applied(true)
            }
            Input::FinishAttempt(ticket) => Output::Applied(self.lifecycle.finish_attempt(ticket)),
            Input::ClaimAuthentication => {
                Output::Authentication(self.lifecycle.claim_authentication()?)
            }
            Input::AdmitAuthentication(ticket) => {
                Output::Applied(self.lifecycle.admit_authentication(ticket, self.now))
            }
            Input::Ready(ticket) => Output::Applied(self.lifecycle.ready(ticket)),
            Input::TransportLost(ticket) => Output::Applied(self.lifecycle.transport_lost(ticket)),
            Input::Pause => Output::Applied(self.lifecycle.pause()?),
            Input::Resume => Output::Applied(self.lifecycle.resume()),
            Input::Shutdown => {
                self.lifecycle.shutdown();
                Output::Applied(true)
            }
            Input::SampleBackoff { request, sample } => {
                Output::Backoff(self.lifecycle.sample_backoff(request, sample)?)
            }
            Input::RequestReconnect { immediate } => {
                self.lifecycle.request_reconnect(immediate);
                Output::Applied(true)
            }
            Input::ReconnectPenalty(steps) => {
                self.lifecycle.apply_backoff_penalty(steps);
                Output::Applied(true)
            }
            Input::NextConnection => Output::Connection(self.lifecycle.next_connection(self.now)?),
            Input::RegisterExchange { id, timeout } => {
                let deadline = self
                    .now
                    .checked_add(timeout)
                    .ok_or(SimulationError::TimeOverflow)?;
                let generation = self
                    .exchanges
                    .try_insert(
                        id.clone(),
                        Exchange {
                            generation: NonZeroU64::MIN,
                            deadline,
                            sent: false,
                            response: None,
                        },
                    )
                    .ok_or(SimulationError::DuplicateExchange)?;
                let exchange = self
                    .exchanges
                    .get_mut(&id)
                    .expect("registration was just inserted");
                exchange.generation = generation;
                Output::ExchangeRegistered(ExchangeTicket { id, generation })
            }
            Input::SendComplete(ticket) => {
                let Some(exchange) = self.current(&ticket) else {
                    return Ok(vec![Output::Applied(false)]);
                };
                if exchange.sent {
                    return Ok(vec![Output::Applied(false)]);
                }
                exchange.sent = true;
                if let Some(bytes) = exchange.response.take() {
                    self.finish(ticket, ExchangeOutcome::Response(bytes))
                } else {
                    Output::Applied(true)
                }
            }
            Input::Response { ticket, bytes } => {
                let Some(exchange) = self.current(&ticket) else {
                    return Ok(vec![Output::Applied(false)]);
                };
                if exchange.response.is_some() {
                    return Ok(vec![Output::Applied(false)]);
                }
                if exchange.sent {
                    self.finish(ticket, ExchangeOutcome::Response(bytes))
                } else {
                    exchange.response = Some(bytes);
                    Output::Applied(true)
                }
            }
            Input::SendFailed(ticket) => {
                if self.current(&ticket).is_some_and(|exchange| !exchange.sent) {
                    self.finish(ticket, ExchangeOutcome::SendFailed)
                } else {
                    Output::Applied(false)
                }
            }
            Input::CancelExchange(ticket) => self.finish(ticket, ExchangeOutcome::Cancelled),
            Input::DisconnectExchanges => {
                let mut disconnected = Vec::new();
                self.exchanges.retain(|id, exchange| {
                    disconnected.push(Output::ExchangeFinished {
                        ticket: ExchangeTicket {
                            id: id.to_owned(),
                            generation: exchange.generation,
                        },
                        outcome: ExchangeOutcome::Disconnected,
                    });
                    false
                });
                self.exchanges.clear();
                disconnected.sort_by(|a, b| match (a, b) {
                    (
                        Output::ExchangeFinished { ticket: a, .. },
                        Output::ExchangeFinished { ticket: b, .. },
                    ) => a.id.cmp(&b.id),
                    _ => std::cmp::Ordering::Equal,
                });
                return Ok(disconnected);
            }
        };
        Ok(vec![output])
    }

    fn discard_signal(&mut self, ticket: SignalTicket, reason: SignalDiscardReason) -> Output {
        if self.signals.remove(&ticket).is_some() {
            Output::SignalDiscarded { ticket, reason }
        } else {
            Output::Applied(false)
        }
    }

    fn current(&mut self, ticket: &ExchangeTicket) -> Option<&mut Exchange> {
        self.exchanges
            .get_mut(&ticket.id)
            .filter(|exchange| exchange.generation == ticket.generation)
    }

    fn finish(&mut self, ticket: ExchangeTicket, outcome: ExchangeOutcome) -> Output {
        if self
            .exchanges
            .remove_guarded(&ticket.id, ticket.generation)
            .is_some()
        {
            Output::ExchangeFinished { ticket, outcome }
        } else {
            Output::Applied(false)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn register(sim: &mut SessionSimulator, id: &str) -> ExchangeTicket {
        let outputs = sim
            .step(Input::RegisterExchange {
                id: id.into(),
                timeout: Duration::from_secs(5),
            })
            .unwrap();
        let [Output::ExchangeRegistered(ticket)]: [Output; 1] = outputs.try_into().unwrap() else {
            panic!("registration output")
        };
        ticket
    }

    #[test]
    fn stalled_send_expires_without_waiting_for_send_completion() {
        let mut sim = SessionSimulator::default();
        let ticket = register(&mut sim, "stalled");
        assert_eq!(
            sim.step(Input::Advance(Duration::from_secs(5))).unwrap(),
            vec![Output::ExchangeFinished {
                ticket: ticket.clone(),
                outcome: ExchangeOutcome::TimedOut
            }]
        );
        assert_eq!(
            sim.step(Input::SendComplete(ticket)).unwrap(),
            vec![Output::Applied(false)]
        );
    }

    #[test]
    fn early_response_waits_for_send_and_send_failure_wins() {
        for fail in [false, true] {
            let mut sim = SessionSimulator::default();
            let ticket = register(&mut sim, "early");
            sim.step(Input::Response {
                ticket: ticket.clone(),
                bytes: vec![42],
            })
            .unwrap();
            assert_eq!(sim.pending_exchanges(), 1);
            let input = if fail {
                Input::SendFailed(ticket.clone())
            } else {
                Input::SendComplete(ticket.clone())
            };
            let outcome = if fail {
                ExchangeOutcome::SendFailed
            } else {
                ExchangeOutcome::Response(vec![42])
            };
            assert_eq!(
                sim.step(input).unwrap(),
                vec![Output::ExchangeFinished { ticket, outcome }]
            );
        }
    }

    #[test]
    fn cancellation_and_disconnect_cannot_remove_reused_ids() {
        let mut sim = SessionSimulator::default();
        let old = register(&mut sim, "reused");
        sim.step(Input::DisconnectExchanges).unwrap();
        let new = register(&mut sim, "reused");
        assert_ne!(old, new);
        assert_eq!(
            sim.step(Input::CancelExchange(old)).unwrap(),
            vec![Output::Applied(false)]
        );
        assert_eq!(sim.pending_exchanges(), 1);
        sim.step(Input::CancelExchange(new)).unwrap();
        assert_eq!(sim.pending_exchanges(), 0);
    }

    #[test]
    fn timeout_replay_order_is_stable() {
        let replay = || {
            let mut sim = SessionSimulator::default();
            for id in ["z", "a", "m"] {
                register(&mut sim, id);
            }
            sim.step(Input::Advance(Duration::from_secs(5))).unwrap()
        };
        for _ in 0..32 {
            assert_eq!(replay(), replay());
        }
    }

    #[test]
    fn reconnect_sampling_is_replayable_and_pause_retires_the_old_sample() {
        let mut sim = SessionSimulator::default();
        sim.step(Input::StartDriver(DriveKind::Supervisor)).unwrap();
        let next = sim.step(Input::NextConnection).unwrap();
        let Output::Connection(NextConnection::SampleBackoff(request)) = next[0] else {
            panic!("sample request")
        };
        assert_eq!(sim.step(Input::NextConnection).unwrap(), next);
        assert_eq!(
            sim.step(Input::SampleBackoff {
                request,
                sample: request.sample_max() + 1
            }),
            Err(SimulationError::Lifecycle(
                LifecycleError::InvalidBackoffSample
            ))
        );
        assert_eq!(
            sim.step(Input::SampleBackoff { request, sample: 0 })
                .unwrap(),
            vec![Output::Backoff(Duration::from_millis(900))]
        );
        let next = sim.step(Input::NextConnection).unwrap();
        let Output::Connection(NextConnection::SampleBackoff(old)) = next[0] else {
            panic!("next request")
        };
        sim.step(Input::Pause).unwrap();
        sim.step(Input::Resume).unwrap();
        assert_eq!(
            sim.step(Input::SampleBackoff {
                request: old,
                sample: 0
            }),
            Err(SimulationError::Lifecycle(LifecycleError::StaleCompletion))
        );
        assert_eq!(
            sim.step(Input::NextConnection).unwrap(),
            vec![Output::Connection(NextConnection::Connect)]
        );
    }

    #[test]
    fn a_late_send_failure_cannot_override_a_completed_send() {
        let mut sim = SessionSimulator::default();
        let ticket = register(&mut sim, "sent");
        sim.step(Input::SendComplete(ticket.clone())).unwrap();
        assert_eq!(
            sim.step(Input::SendFailed(ticket.clone())).unwrap(),
            vec![Output::Applied(false)]
        );
        assert_eq!(
            sim.step(Input::Response {
                ticket: ticket.clone(),
                bytes: vec![42]
            })
            .unwrap(),
            vec![Output::ExchangeFinished {
                ticket,
                outcome: ExchangeOutcome::Response(vec![42])
            }]
        );
    }

    #[test]
    fn signal_publication_requires_success_and_stale_completions_are_ignored() {
        let mut sim = SessionSimulator::default();
        let output = sim.step(Input::PrepareSignal(vec![1, 2, 3])).unwrap();
        let Output::SignalPrepared(old) = output[0] else {
            panic!("prepared output")
        };
        assert_eq!(
            sim.step(Input::FailSignalCommit(old)).unwrap(),
            vec![Output::SignalDiscarded {
                ticket: old,
                reason: SignalDiscardReason::PersistenceFailed
            }]
        );
        let output = sim.step(Input::PrepareSignal(vec![4, 5, 6])).unwrap();
        let Output::SignalPrepared(current) = output[0] else {
            panic!("replacement")
        };
        assert_ne!(old, current);
        assert_eq!(
            sim.step(Input::ConfirmSignal {
                ticket: old,
                outcome: SignalCommitOutcome::CommittedWrite
            })
            .unwrap(),
            vec![Output::Applied(false)]
        );
        assert_eq!(
            sim.step(Input::ConfirmSignal {
                ticket: current,
                outcome: SignalCommitOutcome::DurableLease
            })
            .unwrap(),
            vec![Output::PublishSignal {
                ticket: current,
                bytes: vec![4, 5, 6]
            }]
        );
        assert_eq!(
            sim.step(Input::ConfirmSignal {
                ticket: current,
                outcome: SignalCommitOutcome::DurableLease
            })
            .unwrap(),
            vec![Output::Applied(false)]
        );
    }

    #[test]
    fn cancelled_publication_and_sequence_exhaustion_never_emit_bytes() {
        let mut sim = SessionSimulator::default();
        let output = sim.step(Input::PrepareSignal(vec![7])).unwrap();
        let Output::SignalPrepared(ticket) = output[0] else {
            panic!("prepared")
        };
        sim.step(Input::CancelSignal(ticket)).unwrap();
        assert_eq!(
            sim.step(Input::ConfirmSignal {
                ticket,
                outcome: SignalCommitOutcome::CommittedWrite
            })
            .unwrap(),
            vec![Output::Applied(false)]
        );
        sim.next_signal = u64::MAX;
        assert_eq!(
            sim.step(Input::PrepareSignal(vec![8])),
            Err(SimulationError::SignalSequenceExhausted)
        );
    }

    #[test]
    fn cancellation_cleanup_and_redial_replay_identically() {
        let replay = || {
            let mut sim = SessionSimulator::default();
            let mut trace = Vec::new();
            let drive = sim.step(Input::StartDriver(DriveKind::Supervisor)).unwrap();
            let Output::Driver(lease) = drive[0] else {
                panic!("driver")
            };
            trace.extend(drive);
            let connect = sim.step(Input::Connect).unwrap();
            let Output::Attempt(attempt) = connect[0] else {
                panic!("attempt")
            };
            trace.extend(connect);
            trace.extend(sim.step(Input::HandshakeComplete(attempt)).unwrap());
            trace.extend(sim.step(Input::FinishAttempt(attempt)).unwrap());
            trace.extend(sim.step(Input::CancelDriver(lease)).unwrap());
            assert_eq!(
                sim.step(Input::Connect),
                Err(SimulationError::Lifecycle(LifecycleError::CleanupRequired))
            );
            let cleanup = sim
                .step(Input::BeginCleanup(Some(attempt.transport())))
                .unwrap();
            let Output::Cleanup(ticket) = cleanup[0] else {
                panic!("cleanup")
            };
            trace.extend(cleanup);
            trace.extend(sim.step(Input::RetireWorkers).unwrap());
            trace.extend(sim.step(Input::TransportLost(attempt.transport())).unwrap());
            trace.extend(sim.step(Input::CompleteCleanup(ticket)).unwrap());
            trace.extend(sim.step(Input::Connect).unwrap());
            trace
        };
        assert_eq!(replay(), replay());
    }

    #[test]
    fn pause_resume_rejects_late_handshake() {
        let mut sim = SessionSimulator::default();
        let output = sim.step(Input::Connect).unwrap();
        let Output::Attempt(attempt) = output[0] else {
            panic!("attempt")
        };
        sim.step(Input::Pause).unwrap();
        sim.step(Input::Resume).unwrap();
        assert_eq!(
            sim.step(Input::HandshakeComplete(attempt)),
            Err(SimulationError::Lifecycle(LifecycleError::StaleCompletion))
        );
    }
}
