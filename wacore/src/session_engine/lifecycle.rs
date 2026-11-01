use crate::protocol::reconnect::{
    backoff_delay, fibonacci_base_millis, next_backoff_attempt, should_reset_backoff,
};
use crate::time::Instant;
use std::num::NonZeroU64;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WorkerGeneration(u64);

impl WorkerGeneration {
    pub const INITIAL: Self = Self(0);
    pub const MAX: u64 = u64::MAX >> 2;
    pub fn get(self) -> u64 {
        self.0
    }
    pub fn new(value: u64) -> Result<Self, LifecycleError> {
        if value > Self::MAX {
            return Err(LifecycleError::GenerationExhausted);
        }
        Ok(Self(value))
    }
    fn next(self) -> Result<Self, LifecycleError> {
        Self::new(
            self.0
                .checked_add(1)
                .ok_or(LifecycleError::GenerationExhausted)?,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttemptTicket {
    id: NonZeroU64,
    pause_generation: u64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransportTicket(NonZeroU64);
impl AttemptTicket {
    pub fn transport(self) -> TransportTicket {
        TransportTicket(self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticationTicket(WorkerGeneration);
impl AuthenticationTicket {
    pub fn generation(self) -> WorkerGeneration {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveLease(NonZeroU64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    Supervisor,
    DirectReader,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisorPhase {
    Connecting,
    Reading,
    Backoff,
    Parked,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveState {
    Idle,
    Supervisor {
        lease: DriveLease,
        phase: SupervisorPhase,
    },
    DirectReader(DriveLease),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPhase {
    Absent,
    Opening,
    Handshaking,
    Established,
    Retiring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationPhase {
    LoggedOut,
    Claimed(AuthenticationTicket),
    Admitted(AuthenticationTicket),
    Ready(AuthenticationTicket),
}

impl AuthenticationPhase {
    fn ticket(self) -> Option<AuthenticationTicket> {
        match self {
            Self::LoggedOut => None,
            Self::Claimed(t) | Self::Admitted(t) | Self::Ready(t) => Some(t),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectPolicy {
    Enabled,
    Disabled,
    Forbidden,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectIntent {
    Unexpected,
    Expected,
    Intentional,
    ExpectedAndIntentional,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesiredConnection {
    Active,
    Paused,
    Shutdown { was_paused: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopReason {
    Shutdown,
    ReconnectDisabled,
    NoDriver,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NextConnection {
    Stop(StopReason),
    Park,
    Connect,
    SampleBackoff(BackoffRequest),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffRequest {
    serial: NonZeroU64,
    base_millis: u64,
    attempt: u32,
    pause_generation: u64,
}
impl BackoffRequest {
    pub fn sample_max(self) -> u64 {
        (self.base_millis / 10) * 2
    }
    pub fn attempt(self) -> u32 {
        self.attempt
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LifecycleError {
    #[error("session is shut down")]
    Shutdown,
    #[error("session is paused")]
    Paused,
    #[error("session already has a driver")]
    AlreadyDriving,
    #[error("connection attempt already in progress")]
    AlreadyConnecting,
    #[error("transport is already established")]
    AlreadyConnected,
    #[error("completion belongs to a retired operation")]
    StaleCompletion,
    #[error("transition is invalid in the current phase")]
    InvalidTransition,
    #[error("session generation is exhausted")]
    GenerationExhausted,
    #[error("backoff sample is outside its requested range")]
    InvalidBackoffSample,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LifecycleSnapshot {
    transport: TransportPhase,
    authentication: AuthenticationPhase,
    drive: DriveState,
    desired: DesiredConnection,
    policy: ReconnectPolicy,
    intent: DisconnectIntent,
    generation: WorkerGeneration,
    pause_generation: u64,
    attempt_active: bool,
    pause_exit_pending: bool,
    backoff_attempt: u32,
    penalty: bool,
    ready_reached: bool,
}

impl LifecycleSnapshot {
    pub fn transport(self) -> TransportPhase {
        self.transport
    }
    pub fn authentication(self) -> AuthenticationPhase {
        self.authentication
    }
    pub fn authentication_generation(self) -> Option<WorkerGeneration> {
        self.authentication.ticket().map(|ticket| ticket.0)
    }
    pub fn drive(self) -> DriveState {
        self.drive
    }
    pub fn desired(self) -> DesiredConnection {
        self.desired
    }
    pub fn policy(self) -> ReconnectPolicy {
        self.policy
    }
    pub fn intent(self) -> DisconnectIntent {
        self.intent
    }
    pub fn generation(self) -> WorkerGeneration {
        self.generation
    }
    pub fn pause_generation(self) -> u64 {
        self.pause_generation
    }
    pub fn attempt_active(self) -> bool {
        self.attempt_active
    }
    pub fn pause_exit_pending(self) -> bool {
        self.pause_exit_pending
    }
    pub fn backoff_attempt(self) -> u32 {
        self.backoff_attempt
    }
    pub fn penalty_pending(self) -> bool {
        self.penalty
    }
    pub fn driving(self) -> bool {
        !matches!(self.drive, DriveState::Idle)
    }
    pub fn connected(self) -> bool {
        matches!(
            self.transport,
            TransportPhase::Established | TransportPhase::Retiring
        )
    }
    pub fn logged_in(self) -> bool {
        !matches!(self.authentication, AuthenticationPhase::LoggedOut)
    }
    pub fn ready(self) -> bool {
        self.ready_reached
    }
    pub fn paused(self) -> bool {
        matches!(
            self.desired,
            DesiredConnection::Paused | DesiredConnection::Shutdown { was_paused: true }
        )
    }
    pub fn shut_down(self) -> bool {
        matches!(self.desired, DesiredConnection::Shutdown { .. })
    }
    pub fn can_reach_server(self) -> bool {
        self.connected()
            && self.driving()
            && !self.paused()
            && !self.shut_down()
            && !matches!(
                self.intent,
                DisconnectIntent::Expected | DisconnectIntent::ExpectedAndIntentional
            )
            && matches!(self.authentication, AuthenticationPhase::Admitted(t) | AuthenticationPhase::Ready(t) if t.0 == self.generation)
    }
}

#[derive(Debug)]
pub struct Lifecycle {
    snapshot: LifecycleSnapshot,
    next_operation: u64,
    attempt: Option<AttemptTicket>,
    transport: Option<TransportTicket>,
    connected_at: Option<Instant>,
    backoff: Option<BackoffRequest>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new(ReconnectPolicy::Enabled)
    }
}

impl Lifecycle {
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            snapshot: LifecycleSnapshot {
                transport: TransportPhase::Absent,
                authentication: AuthenticationPhase::LoggedOut,
                drive: DriveState::Idle,
                desired: DesiredConnection::Active,
                policy,
                intent: DisconnectIntent::Unexpected,
                generation: WorkerGeneration::INITIAL,
                pause_generation: 0,
                attempt_active: false,
                pause_exit_pending: false,
                backoff_attempt: 0,
                penalty: false,
                ready_reached: false,
            },
            next_operation: 0,
            attempt: None,
            transport: None,
            connected_at: None,
            backoff: None,
        }
    }

    pub fn snapshot(&self) -> LifecycleSnapshot {
        self.snapshot
    }

    fn operation(&mut self) -> Result<NonZeroU64, LifecycleError> {
        let next = self
            .next_operation
            .checked_add(1)
            .ok_or(LifecycleError::GenerationExhausted)?;
        let operation = NonZeroU64::new(next).ok_or(LifecycleError::GenerationExhausted)?;
        self.next_operation = next;
        Ok(operation)
    }

    pub fn start_drive(&mut self, kind: DriveKind) -> Result<DriveLease, LifecycleError> {
        if self.snapshot.shut_down() {
            return Err(LifecycleError::Shutdown);
        }
        if self.snapshot.driving() {
            return Err(LifecycleError::AlreadyDriving);
        }
        let lease = DriveLease(self.operation()?);
        self.snapshot.drive = match kind {
            DriveKind::Supervisor => DriveState::Supervisor {
                lease,
                phase: SupervisorPhase::Connecting,
            },
            DriveKind::DirectReader => DriveState::DirectReader(lease),
        };
        Ok(lease)
    }

    pub fn stop_driving(&mut self) {
        self.snapshot.drive = DriveState::Idle;
    }

    pub fn release_drive(&mut self, lease: DriveLease) -> bool {
        let owns = matches!(self.snapshot.drive, DriveState::Supervisor {lease: active, ..} | DriveState::DirectReader(active) if active == lease);
        if owns {
            self.snapshot.drive = DriveState::Idle;
        }
        owns
    }

    fn connect_refusal(&self) -> Result<(), LifecycleError> {
        if self.snapshot.shut_down() || self.snapshot.policy == ReconnectPolicy::Forbidden {
            return Err(LifecycleError::Shutdown);
        }
        if self.snapshot.paused() {
            return Err(LifecycleError::Paused);
        }
        Ok(())
    }

    pub fn begin_attempt(&mut self) -> Result<AttemptTicket, LifecycleError> {
        self.connect_refusal()?;
        if self.attempt.is_some() {
            return Err(LifecycleError::AlreadyConnecting);
        }
        if self.snapshot.connected() {
            return Err(LifecycleError::AlreadyConnected);
        }
        let ticket = AttemptTicket {
            id: self.operation()?,
            pause_generation: self.snapshot.pause_generation,
        };
        self.attempt = Some(ticket);
        self.snapshot.attempt_active = true;
        self.snapshot.transport = TransportPhase::Opening;
        self.snapshot.authentication = AuthenticationPhase::LoggedOut;
        self.snapshot.ready_reached = false;
        self.snapshot.intent = DisconnectIntent::Unexpected;
        if let DriveState::Supervisor { lease, .. } = self.snapshot.drive {
            self.snapshot.drive = DriveState::Supervisor {
                lease,
                phase: SupervisorPhase::Connecting,
            };
        }
        Ok(ticket)
    }

    fn check_attempt(&self, ticket: AttemptTicket) -> Result<(), LifecycleError> {
        self.connect_refusal()?;
        if self.attempt != Some(ticket) || self.snapshot.pause_generation != ticket.pause_generation
        {
            return Err(LifecycleError::StaleCompletion);
        }
        Ok(())
    }

    pub fn transport_opened(&mut self, ticket: AttemptTicket) -> Result<(), LifecycleError> {
        self.check_attempt(ticket)?;
        self.snapshot.transport = TransportPhase::Handshaking;
        Ok(())
    }

    pub fn handshake_ready(
        &mut self,
        ticket: AttemptTicket,
    ) -> Result<WorkerGeneration, LifecycleError> {
        self.check_attempt(ticket)?;
        if !matches!(
            self.snapshot.transport,
            TransportPhase::Opening | TransportPhase::Handshaking
        ) {
            return Err(LifecycleError::InvalidTransition);
        }
        let generation = self.snapshot.generation;
        self.snapshot.transport = TransportPhase::Established;
        self.transport = Some(ticket.transport());
        if let DriveState::Supervisor { lease, .. } = self.snapshot.drive {
            self.snapshot.drive = DriveState::Supervisor {
                lease,
                phase: SupervisorPhase::Reading,
            };
        }
        Ok(generation)
    }

    pub fn finish_attempt(&mut self, ticket: AttemptTicket) -> bool {
        if self.attempt != Some(ticket) {
            return false;
        }
        self.attempt = None;
        self.snapshot.attempt_active = false;
        if matches!(
            self.snapshot.transport,
            TransportPhase::Opening | TransportPhase::Handshaking
        ) {
            self.snapshot.transport = TransportPhase::Absent;
        }
        true
    }

    pub fn claim_authentication(&mut self) -> Result<AuthenticationTicket, LifecycleError> {
        if self.snapshot.shut_down() {
            return Err(LifecycleError::Shutdown);
        }
        if self.snapshot.paused() {
            return Err(LifecycleError::Paused);
        }
        if matches!(
            self.snapshot.intent,
            DisconnectIntent::Expected | DisconnectIntent::ExpectedAndIntentional
        ) || self.snapshot.transport != TransportPhase::Established
        {
            return Err(LifecycleError::InvalidTransition);
        }
        if self.snapshot.logged_in() {
            return Err(LifecycleError::InvalidTransition);
        }
        let ticket = AuthenticationTicket(self.snapshot.generation.next()?);
        self.snapshot.generation = ticket.0;
        self.snapshot.authentication = AuthenticationPhase::Claimed(ticket);
        Ok(ticket)
    }

    pub fn admit_authentication(&mut self, ticket: AuthenticationTicket, now: Instant) -> bool {
        if self.snapshot.shut_down()
            || self.snapshot.paused()
            || self.snapshot.generation != ticket.0
            || matches!(
                self.snapshot.intent,
                DisconnectIntent::Expected | DisconnectIntent::ExpectedAndIntentional
            )
        {
            return false;
        }
        if self.snapshot.authentication != AuthenticationPhase::Claimed(ticket) {
            return false;
        }
        self.snapshot.authentication = AuthenticationPhase::Admitted(ticket);
        self.connected_at = Some(now);
        self.snapshot.penalty = false;
        true
    }

    pub fn ready(&mut self, ticket: AuthenticationTicket) -> bool {
        if self.snapshot.shut_down()
            || self.snapshot.paused()
            || self.snapshot.generation != ticket.0
            || matches!(
                self.snapshot.intent,
                DisconnectIntent::Expected | DisconnectIntent::ExpectedAndIntentional
            )
        {
            return false;
        }
        if self.snapshot.authentication != AuthenticationPhase::Admitted(ticket) {
            return false;
        }
        self.snapshot.authentication = AuthenticationPhase::Ready(ticket);
        self.snapshot.ready_reached = true;
        true
    }

    pub fn invalidate_authentication(&mut self) {
        self.snapshot.authentication = AuthenticationPhase::LoggedOut;
    }

    pub fn clear_readiness(&mut self) {
        self.snapshot.ready_reached = false;
    }

    pub fn mark_ready(&mut self) -> bool {
        let Some(ticket) = self.snapshot.authentication.ticket() else {
            return false;
        };
        self.ready(ticket)
    }

    pub fn advance_worker_generation(&mut self) -> Result<WorkerGeneration, LifecycleError> {
        let previous = self.snapshot.generation;
        self.snapshot.generation = previous.next()?;
        Ok(previous)
    }

    #[cfg(feature = "test-util")]
    pub fn fixture_generation(&mut self, generation: WorkerGeneration) {
        self.snapshot.generation = generation;
    }

    #[cfg(feature = "test-util")]
    pub fn fixture_authentication(&mut self, logged_in: bool) {
        self.snapshot.authentication = if logged_in {
            AuthenticationPhase::Admitted(AuthenticationTicket(self.snapshot.generation))
        } else {
            AuthenticationPhase::LoggedOut
        };
    }

    #[cfg(feature = "test-util")]
    pub fn fixture_readiness(&mut self, ready: bool) {
        self.snapshot.ready_reached = ready;
    }

    pub fn abort_authentication(&mut self, ticket: AuthenticationTicket) -> bool {
        if self.snapshot.authentication != AuthenticationPhase::Claimed(ticket) {
            return false;
        }
        self.snapshot.authentication = AuthenticationPhase::LoggedOut;
        true
    }

    pub fn retire_transport(&mut self) {
        self.transport = None;
        self.snapshot.transport = TransportPhase::Absent;
        self.snapshot.authentication = AuthenticationPhase::LoggedOut;
    }

    pub fn transport_lost(&mut self, ticket: TransportTicket) -> bool {
        if self.transport != Some(ticket) {
            return false;
        }
        self.transport = None;
        self.snapshot.transport = TransportPhase::Absent;
        self.snapshot.authentication = AuthenticationPhase::LoggedOut;
        true
    }

    pub fn pause(&mut self) -> Result<bool, LifecycleError> {
        if self.snapshot.shut_down() || self.snapshot.policy == ReconnectPolicy::Forbidden {
            return Ok(false);
        }
        let generation = self
            .snapshot
            .pause_generation
            .checked_add(1)
            .ok_or(LifecycleError::GenerationExhausted)?;
        self.snapshot.pause_generation = generation;
        self.snapshot.desired = DesiredConnection::Paused;
        self.snapshot.pause_exit_pending = true;
        if self.snapshot.connected() {
            self.request_intentional_disconnect();
            self.snapshot.transport = TransportPhase::Retiring;
        }
        Ok(true)
    }

    pub fn resume(&mut self) -> bool {
        match self.snapshot.desired {
            DesiredConnection::Paused => {
                self.snapshot.desired = DesiredConnection::Active;
                true
            }
            DesiredConnection::Shutdown { was_paused: true } => {
                self.snapshot.desired = DesiredConnection::Shutdown { was_paused: false };
                true
            }
            _ => false,
        }
    }

    pub fn set_reconnect(&mut self, enabled: bool) -> bool {
        let previous = self.snapshot.policy == ReconnectPolicy::Enabled;
        if self.snapshot.policy != ReconnectPolicy::Forbidden {
            self.snapshot.policy = if enabled {
                ReconnectPolicy::Enabled
            } else {
                ReconnectPolicy::Disabled
            };
        }
        previous
    }

    pub fn forbid_reconnect(&mut self) {
        self.snapshot.policy = ReconnectPolicy::Forbidden;
    }

    pub fn expect_disconnect(&mut self) {
        self.snapshot.intent = match self.snapshot.intent {
            DisconnectIntent::Intentional | DisconnectIntent::ExpectedAndIntentional => {
                DisconnectIntent::ExpectedAndIntentional
            }
            _ => DisconnectIntent::Expected,
        };
    }

    pub fn clear_expected_disconnect(&mut self) {
        self.snapshot.intent = match self.snapshot.intent {
            DisconnectIntent::ExpectedAndIntentional => DisconnectIntent::Intentional,
            DisconnectIntent::Expected => DisconnectIntent::Unexpected,
            other => other,
        };
    }

    pub fn request_intentional_disconnect(&mut self) {
        self.snapshot.intent = match self.snapshot.intent {
            DisconnectIntent::Expected | DisconnectIntent::ExpectedAndIntentional => {
                DisconnectIntent::ExpectedAndIntentional
            }
            _ => DisconnectIntent::Intentional,
        };
    }

    pub fn consume_intentional_disconnect(&mut self) -> bool {
        let previous = self.snapshot.intent;
        self.snapshot.intent = match previous {
            DisconnectIntent::ExpectedAndIntentional => DisconnectIntent::Expected,
            DisconnectIntent::Intentional => DisconnectIntent::Unexpected,
            other => other,
        };
        matches!(
            previous,
            DisconnectIntent::Intentional | DisconnectIntent::ExpectedAndIntentional
        )
    }

    pub fn mark_pause_exit_pending(&mut self) {
        self.snapshot.pause_exit_pending = true;
    }
    pub fn consume_pause_exit_pending(&mut self) -> bool {
        std::mem::take(&mut self.snapshot.pause_exit_pending)
    }

    /// Manual reconnect starts at the five-second Fibonacci base delay.
    pub const MANUAL_RECONNECT_ATTEMPT: u32 = 4;

    pub fn request_reconnect(&mut self, immediate: bool) {
        if immediate {
            self.expect_disconnect();
        } else {
            self.request_intentional_disconnect();
            self.snapshot.penalty = true;
            self.snapshot.backoff_attempt = Self::MANUAL_RECONNECT_ATTEMPT;
        }
        if self.snapshot.connected() {
            self.snapshot.transport = TransportPhase::Retiring;
        }
    }

    pub fn apply_backoff_penalty(&mut self, steps: u32) {
        self.snapshot.backoff_attempt = self.snapshot.backoff_attempt.saturating_add(steps);
        self.snapshot.penalty = true;
    }

    #[cfg(feature = "test-util")]
    pub fn fixture_backoff(&mut self, attempt: u32) {
        self.snapshot.backoff_attempt = attempt;
    }
    #[cfg(feature = "test-util")]
    pub fn fixture_penalty(&mut self, penalty: bool) {
        self.snapshot.penalty = penalty;
    }
    #[cfg(feature = "test-util")]
    pub fn fixture_authenticated_at(&mut self, at: Option<Instant>) {
        self.connected_at = at;
    }
    #[cfg(feature = "test-util")]
    pub fn authenticated_at(&self) -> Option<Instant> {
        self.connected_at
    }

    pub fn shutdown(&mut self) {
        let was_paused = self.snapshot.paused();
        self.snapshot.desired = DesiredConnection::Shutdown { was_paused };
        self.snapshot.intent = DisconnectIntent::Expected;
        self.snapshot.drive = DriveState::Idle;
        self.backoff = None;
        if self.snapshot.connected() {
            self.snapshot.transport = TransportPhase::Retiring;
        }
    }

    pub fn reset_backoff(&mut self) {
        self.snapshot.backoff_attempt = 0;
        self.connected_at = None;
        self.backoff = None;
    }

    pub fn next_connection(&mut self, now: Instant) -> Result<NextConnection, LifecycleError> {
        if self.snapshot.shut_down() {
            self.reset_backoff();
            return Ok(NextConnection::Stop(StopReason::Shutdown));
        }
        if !self.snapshot.driving() {
            self.reset_backoff();
            return Ok(NextConnection::Stop(StopReason::NoDriver));
        }
        if self.snapshot.policy != ReconnectPolicy::Enabled {
            return Ok(NextConnection::Stop(StopReason::ReconnectDisabled));
        }
        if matches!(
            self.snapshot.intent,
            DisconnectIntent::Expected | DisconnectIntent::ExpectedAndIntentional
        ) {
            self.reset_backoff();
            return Ok(NextConnection::Connect);
        }
        if self.snapshot.pause_exit_pending || self.snapshot.paused() {
            self.snapshot.pause_exit_pending = false;
            self.reset_backoff();
            return Ok(if self.snapshot.paused() {
                if let DriveState::Supervisor { lease, .. } = self.snapshot.drive {
                    self.snapshot.drive = DriveState::Supervisor {
                        lease,
                        phase: SupervisorPhase::Parked,
                    };
                }
                NextConnection::Park
            } else {
                NextConnection::Connect
            });
        }
        if should_reset_backoff(self.connected_at.take(), now, self.snapshot.penalty) {
            self.snapshot.backoff_attempt = 0;
        }
        let attempt = self.snapshot.backoff_attempt;
        self.snapshot.backoff_attempt = next_backoff_attempt(attempt);
        let request = BackoffRequest {
            serial: self.operation()?,
            base_millis: fibonacci_base_millis(attempt),
            attempt,
            pause_generation: self.snapshot.pause_generation,
        };
        self.backoff = Some(request);
        Ok(NextConnection::SampleBackoff(request))
    }

    pub fn sample_backoff(
        &mut self,
        request: BackoffRequest,
        sample: u64,
    ) -> Result<Duration, LifecycleError> {
        self.connect_refusal()?;
        if self.backoff != Some(request)
            || self.snapshot.pause_generation != request.pause_generation
        {
            return Err(LifecycleError::StaleCompletion);
        }
        let delay = backoff_delay(request.base_millis, sample)
            .ok_or(LifecycleError::InvalidBackoffSample)?;
        self.backoff = None;
        if let DriveState::Supervisor { lease, .. } = self.snapshot.drive {
            self.snapshot.drive = DriveState::Supervisor {
                lease,
                phase: SupervisorPhase::Backoff,
            };
        }
        Ok(delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn connected() -> (Lifecycle, TransportTicket, AuthenticationTicket) {
        let mut state = Lifecycle::default();
        state.start_drive(DriveKind::Supervisor).unwrap();
        let attempt = state.begin_attempt().unwrap();
        state.transport_opened(attempt).unwrap();
        state.handshake_ready(attempt).unwrap();
        state.finish_attempt(attempt);
        let auth = state.claim_authentication().unwrap();
        assert!(state.admit_authentication(auth, Instant::ZERO));
        (state, attempt.transport(), auth)
    }

    #[test]
    fn pause_resume_retires_a_handshake_even_after_pause_flag_clears() {
        let mut state = Lifecycle::default();
        let attempt = state.begin_attempt().unwrap();
        state.pause().unwrap();
        assert!(state.resume());
        assert_eq!(
            state.handshake_ready(attempt),
            Err(LifecycleError::StaleCompletion)
        );
        assert!(!state.snapshot().connected());
    }

    #[test]
    fn stale_transport_and_login_completions_do_not_touch_a_replacement() {
        let (mut state, old, auth) = connected();
        state.transport_lost(old);
        let new = state.begin_attempt().unwrap();
        state.handshake_ready(new).unwrap();
        state.finish_attempt(new);
        assert!(!state.transport_lost(old));
        assert!(!state.ready(auth));
        assert!(state.snapshot().connected());
    }

    #[test]
    fn reconnect_forbidden_does_not_disable_an_existing_deregistration_iq() {
        let (mut state, _, _) = connected();
        state.forbid_reconnect();
        state.set_reconnect(true);
        assert_eq!(state.snapshot().policy(), ReconnectPolicy::Forbidden);
        assert!(state.snapshot().can_reach_server());
        assert_eq!(state.begin_attempt(), Err(LifecycleError::Shutdown));
    }

    #[test]
    fn shutdown_cannot_be_undone_by_resume_or_late_authentication() {
        let (mut state, _, auth) = connected();
        state.pause().unwrap();
        state.shutdown();
        assert!(state.resume());
        assert!(state.snapshot().shut_down());
        assert!(!state.snapshot().paused());
        assert!(!state.ready(auth));
        assert_eq!(
            state.next_connection(Instant::ZERO),
            Ok(NextConnection::Stop(StopReason::Shutdown))
        );
    }

    #[test]
    fn a_released_reader_lease_cannot_stop_a_new_driver() {
        let mut state = Lifecycle::default();
        let old = state.start_drive(DriveKind::DirectReader).unwrap();
        assert!(state.release_drive(old));
        state.start_drive(DriveKind::Supervisor).unwrap();
        assert!(!state.release_drive(old));
        assert!(state.snapshot().driving());
    }

    #[test]
    fn pause_teardown_owes_no_backoff_after_immediate_resume() {
        let (mut state, old, _) = connected();
        state.pause().unwrap();
        state.transport_lost(old);
        state.resume();
        assert_eq!(
            state.next_connection(Instant::ZERO),
            Ok(NextConnection::Connect)
        );
    }

    #[test]
    fn backoff_sampling_is_bounded_and_generation_scoped() {
        let mut state = Lifecycle::default();
        state.start_drive(DriveKind::Supervisor).unwrap();
        let NextConnection::SampleBackoff(request) = state.next_connection(Instant::ZERO).unwrap()
        else {
            panic!("backoff")
        };
        assert_eq!(
            state.sample_backoff(request, request.sample_max() + 1),
            Err(LifecycleError::InvalidBackoffSample)
        );
        assert_eq!(
            state.sample_backoff(request, 0),
            Ok(Duration::from_millis(900))
        );
        assert_eq!(
            state.sample_backoff(request, 0),
            Err(LifecycleError::StaleCompletion)
        );
    }

    #[test]
    fn worker_generation_cannot_overflow_the_flush_scheduler_encoding() {
        assert_eq!(
            WorkerGeneration::new(WorkerGeneration::MAX).unwrap().next(),
            Err(LifecycleError::GenerationExhausted)
        );
        assert!(WorkerGeneration::new(u64::MAX).is_err());
    }
}
