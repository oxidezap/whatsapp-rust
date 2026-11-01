use super::lifecycle::{
    AttemptTicket, AuthenticationTicket, DriveKind, DriveLease, Lifecycle, LifecycleError,
    LifecycleSnapshot, NextConnection, TransportTicket,
};
use crate::request::pending::PendingRequests;
use crate::time::Instant;
use std::num::NonZeroU64;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExchangeTicket {
    id: String,
    generation: NonZeroU64,
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
    Attempt(AttemptTicket),
    Authentication(AuthenticationTicket),
    Connection(NextConnection),
    ExchangeRegistered(ExchangeTicket),
    ExchangeFinished {
        ticket: ExchangeTicket,
        outcome: ExchangeOutcome,
    },
    Applied(bool),
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SimulationError {
    #[error(transparent)]
    Lifecycle(#[from] LifecycleError),
    #[error("virtual time overflow")]
    TimeOverflow,
    #[error("request ID is already registered")]
    DuplicateExchange,
}

/// An executor-free session driver. Input order resolves simultaneous events.
/// Advancing time expires both sending and response-waiting exchanges.
pub struct SessionSimulator {
    lifecycle: Lifecycle,
    now: Instant,
    exchanges: PendingRequests<Exchange>,
}

impl Default for SessionSimulator {
    fn default() -> Self {
        Self {
            lifecycle: Lifecycle::default(),
            now: Instant::ZERO,
            exchanges: PendingRequests::default(),
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
            Input::StartDriver(kind) => Output::Driver(self.lifecycle.start_drive(kind)?),
            Input::CancelDriver(lease) => Output::Applied(self.lifecycle.release_drive(lease)),
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
            Input::SendFailed(ticket) => self.finish(ticket, ExchangeOutcome::SendFailed),
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
