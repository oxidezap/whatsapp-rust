use super::*;

impl Client {
    #[inline]
    pub(crate) fn driver_running(&self, ordering: Ordering) -> bool {
        self.session_state.driving(ordering)
    }

    #[cfg(any(test, feature = "bench-harness", feature = "test-support"))]
    pub(crate) fn set_driver_running_for_test(&self, running: bool, ordering: Ordering) {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if running && !state.snapshot().driving() {
            state
                .start_drive(wacore::session_engine::lifecycle::DriveKind::DirectReader)
                .expect("fixture driver counter exhausted");
        } else if !running {
            state.stop_driving();
        }
        self.session_state.publish(&state, ordering);
    }

    pub(super) fn claim_driver(
        &self,
        kind: wacore::session_engine::lifecycle::DriveKind,
    ) -> Result<
        wacore::session_engine::lifecycle::DriveLease,
        wacore::session_engine::lifecycle::LifecycleError,
    > {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if self.driver_running(Ordering::SeqCst) {
            return Err(wacore::session_engine::lifecycle::LifecycleError::AlreadyDriving);
        }
        let lease = state.start_drive(kind)?;
        self.session_state.publish(&state, Ordering::SeqCst);
        Ok(lease)
    }

    pub(super) fn release_driver(&self, lease: wacore::session_engine::lifecycle::DriveLease) {
        self.finish_driver(lease, false);
    }

    pub(super) fn cancel_driver(&self, lease: wacore::session_engine::lifecycle::DriveLease) {
        self.finish_driver(lease, true);
    }

    fn finish_driver(&self, lease: wacore::session_engine::lifecycle::DriveLease, cancelled: bool) {
        let released = {
            let mut state = self
                .session_state
                .lifecycle_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let direct = matches!(
                state.snapshot().drive(),
                wacore::session_engine::lifecycle::DriveState::DirectReader(_)
            );
            let released = if cancelled {
                state.cancel_drive(lease)
            } else {
                state.release_drive(lease)
            };
            if released {
                if direct {
                    state.consume_pause_exit_pending();
                }
                self.session_state.publish(&state, Ordering::SeqCst);
                true
            } else {
                false
            }
        };
        if released {
            self.notify_session_state();
        }
    }

    pub(super) fn needs_orphan_cleanup(&self) -> bool {
        self.session_state.status.load(Ordering::Acquire) & SessionState::ORPHANED != 0
    }

    pub(super) fn connection_owner(
        &self,
    ) -> Option<wacore::session_engine::lifecycle::TransportTicket> {
        self.session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .connection_owner()
    }

    pub(super) fn begin_connection_cleanup(
        &self,
        owner: Option<wacore::session_engine::lifecycle::TransportTicket>,
    ) -> Result<
        wacore::session_engine::lifecycle::CleanupTicket,
        wacore::session_engine::lifecycle::LifecycleError,
    > {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.begin_cleanup(owner))
    }

    pub(super) fn cancel_connection_cleanup(
        &self,
        ticket: wacore::session_engine::lifecycle::CleanupTicket,
    ) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.cancel_cleanup(ticket));
    }

    pub(super) fn complete_connection_cleanup(
        &self,
        ticket: wacore::session_engine::lifecycle::CleanupTicket,
    ) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.complete_cleanup(ticket));
    }

    pub(super) fn stop_driver_state(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.stop_driving());
    }

    /// Shutdown and driver retirement publish together so reconnect cannot read a torn verdict.
    pub(super) fn publish_terminal_verdict(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.shutdown());
    }

    /// Forget what the connection that just ended owed the reconnect backoff.
    ///
    /// A planned end is not a failure, whether the application asked for it or a
    /// protocol step like the post-pairing 515 did, so the consecutive-failure
    /// count driving the Fibonacci delay goes back to zero. Both planned exits
    /// call this because `stats().reconnect_errors` reports that count, and a
    /// number that depended on which exit was taken would make a change of
    /// branch a change of public behaviour.
    ///
    /// The auth timestamp is consumed for a separate reason: left standing, a
    /// later failed connect reads this cycle's value as a "stable" connection
    /// and resets the backoff on the strength of it.
    pub(super) fn clear_connection_backoff_state(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.reset_backoff());
    }

    pub(crate) fn reconnect_errors(&self, ordering: Ordering) -> u32 {
        self.session_state.reconnect_errors.load(ordering)
    }

    #[cfg(test)]
    pub(crate) fn backoff_penalty_pending(&self, ordering: Ordering) -> bool {
        self.session_state.status.load(ordering) & SessionState::PENALTY != 0
    }

    pub(super) fn request_manual_reconnect(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.request_reconnect(false));
    }

    pub(crate) fn apply_reconnect_penalty(&self, steps: u32) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.apply_backoff_penalty(steps));
    }

    pub(super) fn next_reconnect_delay(&self) -> Option<(Duration, u32)> {
        use wacore::session_engine::lifecycle::NextConnection;
        let now = self.runtime.monotonic_time();
        let request = {
            let mut state = self
                .session_state
                .lifecycle_state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let next = state.next_connection(now).ok()?;
            self.session_state.publish(&state, Ordering::SeqCst);
            match next {
                NextConnection::SampleBackoff(request) => request,
                _ => return None,
            }
        };
        let sample = rand::rng().random_range(0..=request.sample_max());
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let delay = state.sample_backoff(request, sample).ok()?;
        self.session_state.publish(&state, Ordering::SeqCst);
        Some((delay, request.attempt()))
    }

    #[cfg(test)]
    pub(crate) fn set_reconnect_errors_for_test(&self, attempt: u32, _ordering: Ordering) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.fixture_backoff(attempt));
    }

    #[cfg(test)]
    pub(crate) fn set_backoff_penalty_for_test(&self, penalty: bool, _ordering: Ordering) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.fixture_penalty(penalty));
    }

    #[cfg(test)]
    pub(super) fn set_authenticated_at_for_test(&self, at: wacore::time::Instant) {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.fixture_authenticated_at(Some(at));
    }

    #[cfg(test)]
    pub(super) fn authenticated_at_for_test(&self) -> Option<wacore::time::Instant> {
        self.session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .authenticated_at()
    }

    pub(crate) fn expected_session_disconnect(&self, ordering: Ordering) -> bool {
        self.session_state.status.load(ordering) & SessionState::EXPECTED != 0
    }

    #[cfg(test)]
    pub(crate) fn intentional_session_disconnect(&self, ordering: Ordering) -> bool {
        self.session_state.status.load(ordering) & SessionState::INTENTIONAL != 0
    }

    #[cfg(test)]
    pub(crate) fn pause_exit_pending(&self, ordering: Ordering) -> bool {
        self.session_state.status.load(ordering) & SessionState::PAUSE_EXIT != 0
    }

    pub(super) fn pause_session_state(&self) {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.pause().expect("pause generation exhausted");
        self.pause_generation
            .store(state.snapshot().pause_generation(), Ordering::SeqCst);
        self.session_state.publish(&state, Ordering::SeqCst);
    }

    pub(super) fn resume_session_state(&self) -> bool {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.resume())
    }

    pub(crate) fn expect_session_disconnect(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.expect_disconnect());
    }

    pub(crate) fn clear_expected_session_disconnect(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.clear_expected_disconnect());
    }

    pub(super) fn request_intentional_session_disconnect(&self) {
        self.session_state.transition(Ordering::SeqCst, |state| {
            state.request_intentional_disconnect()
        });
    }

    pub(super) fn consume_intentional_session_disconnect(&self) -> bool {
        self.session_state.transition(Ordering::SeqCst, |state| {
            state.consume_intentional_disconnect()
        })
    }

    pub(super) fn mark_pause_exit_pending(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.mark_pause_exit_pending());
    }

    pub(super) fn consume_pause_exit_pending(&self) -> bool {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.consume_pause_exit_pending())
    }

    pub(crate) fn session_ready_reached(&self, ordering: Ordering) -> bool {
        self.session_state.status.load(ordering) & SessionState::READY != 0
    }

    pub(crate) fn invalidate_session_authentication(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.invalidate_authentication());
    }

    pub(super) fn clear_session_readiness(&self) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.clear_readiness());
    }

    pub(super) fn mark_session_ready(&self) -> bool {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.mark_ready())
    }

    pub(crate) fn claim_session_authentication(
        &self,
    ) -> Option<wacore::session_engine::lifecycle::AuthenticationTicket> {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let ticket = state.claim_authentication().ok()?;
        self.connection_generation
            .store(ticket.generation().get(), Ordering::SeqCst);
        self.session_state.publish(&state, Ordering::SeqCst);
        Some(ticket)
    }

    pub(crate) fn admit_session_authentication(
        &self,
        ticket: wacore::session_engine::lifecycle::AuthenticationTicket,
    ) -> bool {
        let now = self.runtime.monotonic_time();
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !state.admit_authentication(ticket, now) {
            return false;
        }
        self.authenticated_generation
            .store(ticket.generation().get(), Ordering::SeqCst);
        self.session_state.publish(&state, Ordering::SeqCst);
        true
    }

    #[cfg(feature = "client-lifecycle")]
    pub(crate) fn abort_session_authentication(
        &self,
        ticket: wacore::session_engine::lifecycle::AuthenticationTicket,
    ) {
        self.session_state
            .transition(Ordering::SeqCst, |state| state.abort_authentication(ticket));
    }

    pub(super) fn advance_session_generation(&self) -> u64 {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = state
            .advance_worker_generation()
            .expect("worker generation exhausted");
        self.connection_generation
            .store(state.snapshot().generation().get(), Ordering::SeqCst);
        self.session_state.publish(&state, Ordering::SeqCst);
        previous.get()
    }

    #[cfg(test)]
    pub(crate) fn set_generation_for_test(&self, generation: u64, ordering: Ordering) {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.fixture_generation(
            wacore::session_engine::lifecycle::WorkerGeneration::new(generation)
                .expect("fixture generation"),
        );
        self.connection_generation.store(generation, ordering);
        self.session_state.publish(&state, Ordering::SeqCst);
    }

    #[cfg(test)]
    pub(crate) fn advance_generation_for_test(&self, increment: u64, ordering: Ordering) -> u64 {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let previous = state.snapshot().generation().get();
        let next = previous
            .checked_add(increment)
            .expect("fixture generation overflow");
        state.fixture_generation(
            wacore::session_engine::lifecycle::WorkerGeneration::new(next)
                .expect("fixture generation"),
        );
        self.connection_generation.store(
            next,
            match ordering {
                Ordering::AcqRel => Ordering::Release,
                Ordering::Acquire => Ordering::Relaxed,
                other => other,
            },
        );
        self.session_state.publish(&state, Ordering::SeqCst);
        previous
    }

    #[cfg(test)]
    pub(crate) fn set_logged_in_for_test(&self, logged_in: bool, ordering: Ordering) {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.fixture_authentication(logged_in);
        self.session_state.publish(&state, ordering);
    }

    #[cfg(test)]
    pub(crate) fn set_ready_for_test(&self, ready: bool, ordering: Ordering) {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.fixture_readiness(ready);
        self.session_state.publish(&state, ordering);
    }

    #[cfg(test)]
    pub(crate) fn connection_attempt_active(&self, ordering: Ordering) -> bool {
        self.session_state.status.load(ordering) & SessionState::CONNECTING != 0
    }

    pub(super) fn lifecycle_connect_error(
        error: wacore::session_engine::lifecycle::LifecycleError,
    ) -> ConnectError {
        use wacore::session_engine::lifecycle::LifecycleError;
        match error {
            LifecycleError::Paused | LifecycleError::StaleCompletion => ConnectError::Paused,
            LifecycleError::Shutdown | LifecycleError::GenerationExhausted => {
                ConnectError::Shutdown
            }
            _ => ConnectError::AlreadyConnected,
        }
    }

    pub(super) fn begin_connection_attempt(
        &self,
    ) -> Result<wacore::session_engine::lifecycle::AttemptTicket, ConnectError> {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let attempt = state
            .begin_attempt()
            .map_err(Self::lifecycle_connect_error)?;
        self.session_state.publish(&state, Ordering::SeqCst);
        Ok(attempt)
    }

    pub(super) fn finish_connection_attempt(
        &self,
        attempt: wacore::session_engine::lifecycle::AttemptTicket,
    ) {
        self.session_state
            .transition(Ordering::Release, |state| state.finish_attempt(attempt));
    }

    pub(super) fn publish_connection(
        &self,
        attempt: wacore::session_engine::lifecycle::AttemptTicket,
    ) -> Result<(), ConnectError> {
        let mut state = self
            .session_state
            .lifecycle_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .handshake_ready(attempt)
            .map_err(Self::lifecycle_connect_error)?;
        self.session_state.publish(&state, Ordering::Release);
        Ok(())
    }

    pub(super) fn retire_connection(&self) {
        self.session_state
            .transition(Ordering::Release, |state| state.retire_transport());
    }
}
