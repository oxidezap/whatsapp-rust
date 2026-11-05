use super::EnvironmentBinding;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Identity of the public keys used by one registration attempt sequence.
/// This is an accidental-mixup guard, not authentication of persisted storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegistrationIdentity([u8; 32]);

impl RegistrationIdentity {
    /// Public-key identity digest for binding host persistence, not private key material.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn from_public_keys(identity_key: &[u8; 32], noise_key: &[u8; 32]) -> Self {
        let mut hash = Sha256::new();
        hash.update(b"whatsapp-rust/mobile-registration-identity/v1");
        hash.update(identity_key);
        hash.update(noise_key);
        Self(hash.finalize().into())
    }
}

/// Local workflow operations, not endpoint names or wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum RegistrationOperation {
    CheckAccount,
    RequestCode,
    SubmitCode,
    SubmitChallenge,
    SubmitPin,
}

/// Host-supplied identity for one persisted workflow, independent of account keys.
///
/// Generate a fresh value through host entropy or a durably unique allocator
/// before starting a new workflow. Restore the persisted value after restart.
/// The core does not generate entropy or enforce uniqueness across hosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegistrationRunId([u8; 32]);

impl RegistrationRunId {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

/// Correlates a completion with its persisted run, environment, keys and attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttemptToken {
    sequence: u32,
    context: [u8; 32],
}

/// Correlates final identity-state durability with one verified checkpoint.
///
/// This is distinct from the token for persisting/sending a request attempt.
/// Only a verified workflow can issue it through `pending_persistence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegistrationCommitToken {
    attempt: AttemptToken,
    checkpoint_time_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum RegistrationStage {
    Ready,
    InFlight {
        token: AttemptToken,
        operation: RegistrationOperation,
        deadline_ms: u64,
    },
    AwaitingCode,
    AwaitingChallenge,
    AwaitingPin,
    OutcomeUnknown {
        token: AttemptToken,
        operation: RegistrationOperation,
    },
    /// Server verification is not registration durability. Host must commit first.
    Verified,
    Registered,
    Rejected,
}

/// Semantic outcomes supplied by a separately qualified response parser.
/// These are not assertions about WhatsApp's wire response vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegistrationOutcome {
    Ready,
    NeedsCode,
    NeedsChallenge,
    NeedsPin,
    RetryAfter { delay_ms: u64 },
    Verified,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RegistrationPolicy {
    max_attempts: u32,
    request_timeout_ms: u64,
}

impl RegistrationPolicy {
    pub fn new(max_attempts: u32, request_timeout_ms: u64) -> Result<Self, RegistrationError> {
        if max_attempts == 0 || request_timeout_ms == 0 {
            return Err(RegistrationError::InvalidPolicy);
        }
        Ok(Self {
            max_attempts,
            request_timeout_ms,
        })
    }
}

impl<'de> serde::Deserialize<'de> for RegistrationPolicy {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct StoredPolicy {
            max_attempts: u32,
            request_timeout_ms: u64,
        }
        let policy = <StoredPolicy as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(policy.max_attempts, policy.request_timeout_ms).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum RegistrationError {
    #[error("invalid mobile registration policy")]
    InvalidPolicy,
    #[error("invalid registration checkpoint")]
    InvalidCheckpoint,
    #[error("registration checkpoint belongs to another environment")]
    EnvironmentMismatch,
    #[error("registration checkpoint belongs to different identity keys")]
    IdentityMismatch,
    #[error("registration operation is not allowed in the current stage")]
    InvalidTransition,
    #[error("registration attempt budget exhausted")]
    AttemptLimit,
    #[error("registration cooldown has not elapsed")]
    Cooldown,
    #[error("registration completion is stale")]
    StaleCompletion,
    #[error("registration timestamp overflow")]
    TimestampOverflow,
    #[error("registration clock moved backwards")]
    ClockMovedBackwards,
}

/// Versioned local checkpoint. This contains no phone number, PIN, OTP or keys.
///
/// Persist it alongside environment-bound identity material before sending an
/// attempt. Restoring an in-flight operation yields OutcomeUnknown, never an
/// automatic resend. The host owns atomic publication and durable writes.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegistrationCheckpoint {
    schema_version: u32,
    environment: EnvironmentBinding,
    identity: RegistrationIdentity,
    run_id: RegistrationRunId,
    policy: RegistrationPolicy,
    stage: RegistrationStage,
    attempts: u32,
    retry_stage: RegistrationStage,
    retry_not_before_ms: u64,
    last_time_ms: u64,
}

/// Pure local registration workflow policy. No I/O or runtime is required.
///
/// All timestamps are caller-supplied milliseconds on one persistent timebase.
/// Backwards time is an explicit error. This does not parse wire responses or
/// authorize a request; the host must supply qualified builders and providers.
#[derive(Debug)]
pub struct RegistrationWorkflow {
    checkpoint: RegistrationCheckpoint,
    context: [u8; 32],
}

impl RegistrationWorkflow {
    /// Start one new run with a fresh host-supplied run ID. For restart, use
    /// `restore` instead. The host must keep only one active workflow per run
    /// and prevent checkpoint rollback/forks; these are not storage authentication.
    pub fn new(
        environment: EnvironmentBinding,
        identity: RegistrationIdentity,
        run_id: RegistrationRunId,
        policy: RegistrationPolicy,
    ) -> Self {
        Self {
            checkpoint: RegistrationCheckpoint {
                schema_version: 2,
                environment,
                identity,
                run_id,
                policy,
                stage: RegistrationStage::Ready,
                attempts: 0,
                retry_stage: RegistrationStage::Ready,
                retry_not_before_ms: 0,
                last_time_ms: 0,
            },
            context: attempt_context(environment, identity, run_id),
        }
    }

    pub fn restore(
        mut checkpoint: RegistrationCheckpoint,
        environment: EnvironmentBinding,
        identity: RegistrationIdentity,
    ) -> Result<Self, RegistrationError> {
        if checkpoint.environment != environment {
            return Err(RegistrationError::EnvironmentMismatch);
        }
        if checkpoint.identity != identity {
            return Err(RegistrationError::IdentityMismatch);
        }
        if checkpoint.schema_version != 2
            || checkpoint.policy.max_attempts == 0
            || checkpoint.policy.request_timeout_ms == 0
            || checkpoint.attempts > checkpoint.policy.max_attempts
            || checkpoint.attempts == 0
                && (!matches!(
                    checkpoint.stage,
                    RegistrationStage::Ready | RegistrationStage::AwaitingCode
                ) || checkpoint.retry_not_before_ms != 0)
            || checkpoint.attempts == 0 && checkpoint.retry_stage != checkpoint.stage
            || matches!(checkpoint.stage, RegistrationStage::InFlight { deadline_ms, .. } if deadline_ms <= checkpoint.last_time_ms)
            || !matches!(
                checkpoint.retry_stage,
                RegistrationStage::Ready
                    | RegistrationStage::AwaitingCode
                    | RegistrationStage::AwaitingChallenge
                    | RegistrationStage::AwaitingPin
            )
        {
            return Err(RegistrationError::InvalidCheckpoint);
        }
        let pending = match checkpoint.stage {
            RegistrationStage::InFlight {
                token, operation, ..
            }
            | RegistrationStage::OutcomeUnknown { token, operation } => Some((token, operation)),
            _ => None,
        };
        let context = attempt_context(environment, identity, checkpoint.run_id);
        if pending.is_some_and(|(token, operation)| {
            token.sequence == 0
                || token.sequence != checkpoint.attempts
                || token.context != context
                || !allows(checkpoint.retry_stage, operation)
        }) {
            return Err(RegistrationError::InvalidCheckpoint);
        }
        if let Some((token, operation)) = pending {
            checkpoint.stage = RegistrationStage::OutcomeUnknown { token, operation };
        }
        Ok(Self {
            checkpoint,
            context,
        })
    }

    pub fn checkpoint(&self) -> RegistrationCheckpoint {
        self.checkpoint.clone()
    }

    pub fn stage(&self) -> RegistrationStage {
        self.checkpoint.stage
    }

    /// Accept the caller's explicit declaration that a code was requested elsewhere.
    /// This sends no request and does not consume an HTTP attempt or prove delivery.
    pub fn await_external_code(&mut self, now_ms: u64) -> Result<(), RegistrationError> {
        self.check_time(now_ms)?;
        if self.checkpoint.stage != RegistrationStage::Ready {
            return Err(RegistrationError::InvalidTransition);
        }
        if now_ms < self.checkpoint.retry_not_before_ms {
            return Err(RegistrationError::Cooldown);
        }
        self.checkpoint.stage = RegistrationStage::AwaitingCode;
        self.checkpoint.retry_stage = RegistrationStage::AwaitingCode;
        self.checkpoint.retry_not_before_ms = 0;
        self.checkpoint.last_time_ms = now_ms;
        Ok(())
    }

    /// Abandon a reserved attempt only when the host guarantees HTTP dispatch never began.
    /// The attempt budget remains consumed. Uncertain deliveries cannot use this escape.
    pub fn abandon_unsent(
        &mut self,
        expected: AttemptToken,
        now_ms: u64,
    ) -> Result<(), RegistrationError> {
        self.check_time(now_ms)?;
        let RegistrationStage::InFlight { token, .. } = self.checkpoint.stage else {
            return Err(RegistrationError::InvalidTransition);
        };
        if token != expected {
            return Err(RegistrationError::StaleCompletion);
        }
        self.checkpoint.stage = self.checkpoint.retry_stage;
        self.checkpoint.retry_not_before_ms = 0;
        self.checkpoint.last_time_ms = now_ms;
        Ok(())
    }

    /// Reserve an attempt before the host persists this checkpoint and sends it.
    /// A reserved attempt consumes budget even if the host crashes before sending.
    pub fn begin(
        &mut self,
        operation: RegistrationOperation,
        now_ms: u64,
    ) -> Result<AttemptToken, RegistrationError> {
        self.check_time(now_ms)?;
        if now_ms < self.checkpoint.retry_not_before_ms {
            return Err(RegistrationError::Cooldown);
        }
        if !allows(self.checkpoint.stage, operation) {
            return Err(RegistrationError::InvalidTransition);
        }
        if self.checkpoint.attempts >= self.checkpoint.policy.max_attempts {
            return Err(RegistrationError::AttemptLimit);
        }
        let deadline_ms = now_ms
            .checked_add(self.checkpoint.policy.request_timeout_ms)
            .ok_or(RegistrationError::TimestampOverflow)?;
        let token = AttemptToken {
            sequence: self.checkpoint.attempts + 1,
            context: self.context,
        };
        self.checkpoint.retry_stage = self.checkpoint.stage;
        self.checkpoint.attempts = token.sequence;
        self.checkpoint.last_time_ms = now_ms;
        self.checkpoint.stage = RegistrationStage::InFlight {
            token,
            operation,
            deadline_ms,
        };
        Ok(token)
    }

    /// Process a response only for the matching attempt, including a late
    /// response after timeout. A rate limit returns to the prior input stage.
    pub fn complete(
        &mut self,
        token: AttemptToken,
        outcome: RegistrationOutcome,
        now_ms: u64,
    ) -> Result<(), RegistrationError> {
        self.check_time(now_ms)?;
        self.pending_operation(token)?;
        let retry_not_before_ms = if let RegistrationOutcome::RetryAfter { delay_ms } = outcome {
            now_ms
                .checked_add(delay_ms)
                .ok_or(RegistrationError::TimestampOverflow)?
        } else {
            0
        };
        self.checkpoint.stage = match outcome {
            RegistrationOutcome::Ready => RegistrationStage::Ready,
            RegistrationOutcome::NeedsCode => RegistrationStage::AwaitingCode,
            RegistrationOutcome::NeedsChallenge => RegistrationStage::AwaitingChallenge,
            RegistrationOutcome::NeedsPin => RegistrationStage::AwaitingPin,
            RegistrationOutcome::RetryAfter { .. } => self.checkpoint.retry_stage,
            RegistrationOutcome::Verified => RegistrationStage::Verified,
            RegistrationOutcome::Rejected => RegistrationStage::Rejected,
        };
        self.checkpoint.retry_not_before_ms = retry_not_before_ms;
        self.checkpoint.last_time_ms = now_ms;
        Ok(())
    }

    /// Call when the host loses certainty about delivery, including cancellation.
    /// No retry is allowed from OutcomeUnknown without a matching completion.
    pub fn delivery_uncertain(&mut self, token: AttemptToken) -> Result<(), RegistrationError> {
        let operation = self.pending_operation(token)?;
        self.checkpoint.stage = RegistrationStage::OutcomeUnknown { token, operation };
        Ok(())
    }

    pub fn expire(&mut self, now_ms: u64) -> Result<bool, RegistrationError> {
        self.check_time(now_ms)?;
        let expired = match self.checkpoint.stage {
            RegistrationStage::InFlight {
                token, deadline_ms, ..
            } if now_ms >= deadline_ms => {
                self.delivery_uncertain(token)?;
                true
            }
            _ => false,
        };
        self.checkpoint.last_time_ms = now_ms;
        Ok(expired)
    }

    /// Correlate a host-owned durable write with the current verified checkpoint.
    ///
    /// Capture this before starting the write, including after checkpoint restore.
    /// A failed or cancelled write must not be acknowledged. A later checkpoint
    /// clock update invalidates an older write's token. This scopes completion;
    /// it does not prove or perform a storage write.
    pub fn pending_persistence(&self) -> Option<RegistrationCommitToken> {
        (self.checkpoint.stage == RegistrationStage::Verified).then_some(RegistrationCommitToken {
            attempt: AttemptToken {
                sequence: self.checkpoint.attempts,
                context: self.context,
            },
            checkpoint_time_ms: self.checkpoint.last_time_ms,
        })
    }

    /// Publish Registered only after the host has durably committed identity state.
    /// Pass the token captured before that write, not a token read at completion.
    pub fn confirm_persisted(
        &mut self,
        token: RegistrationCommitToken,
    ) -> Result<(), RegistrationError> {
        if self.checkpoint.stage != RegistrationStage::Verified {
            return Err(RegistrationError::InvalidTransition);
        }
        if self.pending_persistence() != Some(token) {
            return Err(RegistrationError::StaleCompletion);
        }
        self.checkpoint.stage = RegistrationStage::Registered;
        Ok(())
    }

    fn pending_operation(
        &self,
        expected: AttemptToken,
    ) -> Result<RegistrationOperation, RegistrationError> {
        match self.checkpoint.stage {
            RegistrationStage::InFlight {
                token, operation, ..
            }
            | RegistrationStage::OutcomeUnknown { token, operation }
                if token == expected =>
            {
                Ok(operation)
            }
            _ => Err(RegistrationError::StaleCompletion),
        }
    }

    fn check_time(&self, now_ms: u64) -> Result<(), RegistrationError> {
        if now_ms < self.checkpoint.last_time_ms {
            return Err(RegistrationError::ClockMovedBackwards);
        }
        Ok(())
    }
}

fn attempt_context(
    environment: EnvironmentBinding,
    identity: RegistrationIdentity,
    run_id: RegistrationRunId,
) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"whatsapp-rust/mobile-registration-attempt/v2");
    hash.update(environment.as_bytes());
    hash.update(identity.0);
    hash.update(run_id.0);
    hash.finalize().into()
}

fn allows(stage: RegistrationStage, operation: RegistrationOperation) -> bool {
    matches!(
        (stage, operation),
        (
            RegistrationStage::Ready,
            RegistrationOperation::CheckAccount | RegistrationOperation::RequestCode
        ) | (
            RegistrationStage::AwaitingCode,
            RegistrationOperation::SubmitCode | RegistrationOperation::RequestCode
        ) | (
            RegistrationStage::AwaitingChallenge,
            RegistrationOperation::SubmitChallenge
        ) | (
            RegistrationStage::AwaitingPin,
            RegistrationOperation::SubmitPin
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mobile::{MobileEnvironment, MobileTrust, RegistrationEndpoint, TcpEndpoint};

    fn workflow(max: u32) -> RegistrationWorkflow {
        let env = MobileEnvironment::new(
            "test",
            RegistrationEndpoint::new("https://register.test/v2").unwrap(),
            TcpEndpoint::new("stream.test", 443).unwrap(),
            MobileTrust::new([1; 32], [2; 32]).unwrap(),
        )
        .unwrap();
        RegistrationWorkflow::new(
            env.binding(),
            RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]),
            RegistrationRunId::from_bytes([7; 32]),
            RegistrationPolicy::new(max, 100).unwrap(),
        )
    }

    #[test]
    fn externally_requested_code_resumes_without_spending_http_attempt() {
        let mut flow = workflow(1);
        flow.await_external_code(10).unwrap();
        assert_eq!(flow.checkpoint().attempts, 0);
        let checkpoint = flow.checkpoint();
        let mut restored = RegistrationWorkflow::restore(
            checkpoint.clone(),
            checkpoint.environment,
            checkpoint.identity,
        )
        .unwrap();
        assert_eq!(restored.stage(), RegistrationStage::AwaitingCode);
        let token = restored
            .begin(RegistrationOperation::SubmitCode, 11)
            .unwrap();
        restored
            .complete(token, RegistrationOutcome::Verified, 12)
            .unwrap();
        assert_eq!(restored.stage(), RegistrationStage::Verified);
    }

    #[test]
    fn known_unsent_reservations_can_retry_without_refunding_budget() {
        let mut flow = workflow(2);
        let first = flow.begin(RegistrationOperation::RequestCode, 10).unwrap();
        flow.abandon_unsent(first, 11).unwrap();
        assert_eq!(flow.stage(), RegistrationStage::Ready);
        assert_eq!(flow.checkpoint().attempts, 1);
        let second = flow.begin(RegistrationOperation::RequestCode, 12).unwrap();
        let before = flow.checkpoint();
        assert_eq!(
            flow.abandon_unsent(first, 13),
            Err(RegistrationError::StaleCompletion)
        );
        assert_eq!(flow.checkpoint(), before);
        flow.delivery_uncertain(second).unwrap();
        let uncertain = flow.checkpoint();
        assert_eq!(
            flow.abandon_unsent(second, 14),
            Err(RegistrationError::InvalidTransition)
        );
        assert_eq!(flow.checkpoint(), uncertain);
    }

    #[test]
    fn external_code_declaration_cannot_erase_cooldown_or_inflight_state() {
        let mut flow = workflow(3);
        let token = flow.begin(RegistrationOperation::CheckAccount, 10).unwrap();
        let before = flow.checkpoint();
        assert_eq!(
            flow.await_external_code(11),
            Err(RegistrationError::InvalidTransition)
        );
        assert_eq!(flow.checkpoint(), before);
        flow.complete(token, RegistrationOutcome::RetryAfter { delay_ms: 10 }, 12)
            .unwrap();
        assert_eq!(
            flow.await_external_code(21),
            Err(RegistrationError::Cooldown)
        );
        flow.await_external_code(22).unwrap();
        assert_eq!(flow.stage(), RegistrationStage::AwaitingCode);
    }

    fn foreign_workflows(checkpoint: &RegistrationCheckpoint) -> [RegistrationWorkflow; 3] {
        let other_environment = MobileEnvironment::new(
            "other-test",
            RegistrationEndpoint::new("https://other-register.test/v2").unwrap(),
            TcpEndpoint::new("other-stream.test", 443).unwrap(),
            MobileTrust::new([1; 32], [2; 32]).unwrap(),
        )
        .unwrap();
        [
            RegistrationWorkflow::new(
                other_environment.binding(),
                checkpoint.identity,
                checkpoint.run_id,
                checkpoint.policy,
            ),
            RegistrationWorkflow::new(
                checkpoint.environment,
                RegistrationIdentity::from_public_keys(&[5; 32], &[4; 32]),
                checkpoint.run_id,
                checkpoint.policy,
            ),
            RegistrationWorkflow::new(
                checkpoint.environment,
                checkpoint.identity,
                RegistrationRunId::from_bytes([8; 32]),
                checkpoint.policy,
            ),
        ]
    }

    #[test]
    fn lost_response_and_restart_never_trigger_automatic_resend() {
        let mut flow = workflow(4);
        let token = flow.begin(RegistrationOperation::RequestCode, 10).unwrap();
        let stored = serde_json::to_vec(&flow.checkpoint()).unwrap();
        let checkpoint = serde_json::from_slice(&stored).unwrap();
        let mut restored = RegistrationWorkflow::restore(
            checkpoint,
            flow.checkpoint.environment,
            flow.checkpoint.identity,
        )
        .unwrap();
        assert!(matches!(
            restored.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert_eq!(
            restored.begin(RegistrationOperation::RequestCode, 120),
            Err(RegistrationError::InvalidTransition)
        );
        assert!(!flow.expire(109).unwrap());
        assert!(flow.expire(110).unwrap());
        flow.complete(token, RegistrationOutcome::NeedsCode, 115)
            .unwrap();
        assert_eq!(flow.stage(), RegistrationStage::AwaitingCode);
    }

    #[test]
    fn budget_and_server_wait_survive_restart_and_stale_completions() {
        let mut flow = workflow(2);
        let first = flow.begin(RegistrationOperation::RequestCode, 0).unwrap();
        flow.complete(first, RegistrationOutcome::RetryAfter { delay_ms: 500 }, 10)
            .unwrap();
        let mut flow = RegistrationWorkflow::restore(
            flow.checkpoint(),
            flow.checkpoint.environment,
            flow.checkpoint.identity,
        )
        .unwrap();
        assert_eq!(
            flow.begin(RegistrationOperation::RequestCode, 509),
            Err(RegistrationError::Cooldown)
        );
        let second = flow.begin(RegistrationOperation::RequestCode, 510).unwrap();
        assert_eq!(
            flow.complete(first, RegistrationOutcome::Verified, 511),
            Err(RegistrationError::StaleCompletion)
        );
        flow.complete(second, RegistrationOutcome::NeedsCode, 512)
            .unwrap();
        assert_eq!(
            flow.begin(RegistrationOperation::SubmitCode, 513),
            Err(RegistrationError::AttemptLimit)
        );
    }

    #[test]
    fn verification_requires_explicit_durable_publication() {
        let mut flow = workflow(5);
        let token = flow.begin(RegistrationOperation::RequestCode, 1).unwrap();
        flow.complete(token, RegistrationOutcome::NeedsCode, 2)
            .unwrap();
        let token = flow.begin(RegistrationOperation::SubmitCode, 3).unwrap();
        flow.complete(token, RegistrationOutcome::NeedsPin, 4)
            .unwrap();
        assert_eq!(
            flow.begin(RegistrationOperation::SubmitCode, 5),
            Err(RegistrationError::InvalidTransition)
        );
        let token = flow.begin(RegistrationOperation::SubmitPin, 5).unwrap();
        flow.complete(token, RegistrationOutcome::Verified, 6)
            .unwrap();
        assert_eq!(flow.stage(), RegistrationStage::Verified);
        let commit = flow.pending_persistence().unwrap();
        flow.confirm_persisted(commit).unwrap();
        assert_eq!(flow.stage(), RegistrationStage::Registered);
    }

    #[test]
    fn invalid_checkpoint_and_policy_are_rejected() {
        assert!(
            serde_json::from_str::<RegistrationPolicy>(
                r#"{"max_attempts":0,"request_timeout_ms":100}"#
            )
            .is_err()
        );
        let flow = workflow(2);
        assert!(matches!(
            RegistrationWorkflow::restore(
                flow.checkpoint(),
                flow.checkpoint.environment,
                RegistrationIdentity::from_public_keys(&[5; 32], &[4; 32])
            ),
            Err(RegistrationError::IdentityMismatch)
        ));
        for stage in ["Registered", "Verified", "AwaitingCode", "Rejected"] {
            let mut value = serde_json::to_value(flow.checkpoint()).unwrap();
            value["stage"] = serde_json::json!(stage);
            let checkpoint = serde_json::from_value(value).unwrap();
            assert!(matches!(
                RegistrationWorkflow::restore(
                    checkpoint,
                    flow.checkpoint.environment,
                    flow.checkpoint.identity
                ),
                Err(RegistrationError::InvalidCheckpoint)
            ));
        }
        let mut checkpoint = flow.checkpoint();
        checkpoint.schema_version = 1;
        assert!(matches!(
            RegistrationWorkflow::restore(
                checkpoint,
                flow.checkpoint.environment,
                flow.checkpoint.identity
            ),
            Err(RegistrationError::InvalidCheckpoint)
        ));
        let mut checkpoint = flow.checkpoint();
        checkpoint.stage = RegistrationStage::InFlight {
            token: AttemptToken {
                sequence: 99,
                context: flow.context,
            },
            operation: RegistrationOperation::RequestCode,
            deadline_ms: 100,
        };
        assert!(matches!(
            RegistrationWorkflow::restore(
                checkpoint,
                flow.checkpoint.environment,
                flow.checkpoint.identity
            ),
            Err(RegistrationError::InvalidCheckpoint)
        ));
    }

    #[test]
    fn errors_leave_state_unchanged() {
        let mut flow = workflow(2);
        let before = flow.checkpoint();
        assert_eq!(
            flow.begin(RegistrationOperation::RequestCode, u64::MAX),
            Err(RegistrationError::TimestampOverflow)
        );
        assert_eq!(flow.checkpoint(), before);
        let token = flow.begin(RegistrationOperation::RequestCode, 10).unwrap();
        let before = flow.checkpoint();
        assert_eq!(
            flow.complete(
                token,
                RegistrationOutcome::RetryAfter { delay_ms: u64::MAX },
                11
            ),
            Err(RegistrationError::TimestampOverflow)
        );
        assert_eq!(flow.checkpoint(), before);
        assert_eq!(
            flow.complete(token, RegistrationOutcome::NeedsCode, 9),
            Err(RegistrationError::ClockMovedBackwards)
        );
        assert_eq!(flow.checkpoint(), before);
    }

    #[test]
    fn completion_tokens_do_not_cross_environments_identity_keys_or_runs() {
        let mut original = workflow(2);
        let checkpoint = original.checkpoint();
        let mut foreign_workflows = foreign_workflows(&checkpoint);
        let own_token = original
            .begin(RegistrationOperation::RequestCode, 10)
            .unwrap();
        let before = original.checkpoint();
        for foreign in &mut foreign_workflows {
            let foreign_token = foreign
                .begin(RegistrationOperation::RequestCode, 10)
                .unwrap();
            assert_eq!(
                original.complete(foreign_token, RegistrationOutcome::Verified, 11),
                Err(RegistrationError::StaleCompletion)
            );
            assert_eq!(original.checkpoint(), before);
            assert_eq!(
                original.delivery_uncertain(foreign_token),
                Err(RegistrationError::StaleCompletion)
            );
            assert_eq!(original.checkpoint(), before);
        }
        original
            .complete(own_token, RegistrationOutcome::NeedsCode, 11)
            .unwrap();
        assert_eq!(original.stage(), RegistrationStage::AwaitingCode);
    }

    #[test]
    fn restored_pending_token_must_match_its_persisted_run() {
        let mut flow = workflow(2);
        let token = flow.begin(RegistrationOperation::RequestCode, 10).unwrap();
        let original = flow.checkpoint();
        let encoded = serde_json::to_value(&original).unwrap();
        let mut changed_run = encoded.clone();
        changed_run["run_id"] = serde_json::to_value([8_u8; 32]).unwrap();
        let mut changed_token = encoded.clone();
        changed_token["stage"]["InFlight"]["token"]["context"] =
            serde_json::to_value([0_u8; 32]).unwrap();
        for changed in [changed_run, changed_token] {
            let changed = serde_json::from_value(changed).unwrap();
            assert!(matches!(
                RegistrationWorkflow::restore(changed, original.environment, original.identity),
                Err(RegistrationError::InvalidCheckpoint)
            ));
        }
        let mut restored = RegistrationWorkflow::restore(
            serde_json::from_value(encoded).unwrap(),
            flow.checkpoint.environment,
            flow.checkpoint.identity,
        )
        .unwrap();
        restored
            .complete(token, RegistrationOutcome::NeedsCode, 11)
            .unwrap();
        assert_eq!(restored.stage(), RegistrationStage::AwaitingCode);
    }

    #[test]
    fn persistence_confirmation_cannot_publish_a_different_verified_run() {
        let mut original = workflow(2);
        let checkpoint = original.checkpoint();
        let own = original
            .begin(RegistrationOperation::RequestCode, 10)
            .unwrap();
        original
            .complete(own, RegistrationOutcome::Verified, 11)
            .unwrap();
        let before = original.checkpoint();
        for mut foreign in foreign_workflows(&checkpoint) {
            let other = foreign
                .begin(RegistrationOperation::RequestCode, 10)
                .unwrap();
            foreign
                .complete(other, RegistrationOutcome::Verified, 11)
                .unwrap();
            assert_eq!(
                original.confirm_persisted(foreign.pending_persistence().unwrap()),
                Err(RegistrationError::StaleCompletion)
            );
            assert_eq!(original.checkpoint(), before);
        }
        original
            .confirm_persisted(original.pending_persistence().unwrap())
            .unwrap();
        assert_eq!(original.pending_persistence(), None);
    }

    #[test]
    fn durable_confirmation_survives_restore_but_not_failed_or_stale_writes() {
        let mut flow = workflow(3);
        assert_eq!(flow.pending_persistence(), None);
        let previous = flow.begin(RegistrationOperation::CheckAccount, 10).unwrap();
        assert_eq!(flow.pending_persistence(), None);
        flow.complete(previous, RegistrationOutcome::Ready, 11)
            .unwrap();
        let verified = flow.begin(RegistrationOperation::RequestCode, 12).unwrap();
        flow.complete(verified, RegistrationOutcome::Verified, 13)
            .unwrap();
        let completion = flow.pending_persistence().unwrap();
        assert_eq!(completion.attempt, verified);
        let before = flow.checkpoint();
        // A host write failed or was cancelled: do not acknowledge its token.
        let write_result: Result<(), ()> = Err(());
        if write_result.is_ok() {
            flow.confirm_persisted(completion).unwrap();
        }
        assert_eq!(flow.stage(), RegistrationStage::Verified);
        assert_eq!(flow.checkpoint(), before);
        assert_eq!(
            flow.confirm_persisted(RegistrationCommitToken {
                attempt: previous,
                checkpoint_time_ms: flow.checkpoint.last_time_ms,
            }),
            Err(RegistrationError::StaleCompletion)
        );
        assert_eq!(flow.checkpoint(), before);

        let checkpoint = serde_json::from_slice(&serde_json::to_vec(&before).unwrap()).unwrap();
        let mut restored =
            RegistrationWorkflow::restore(checkpoint, before.environment, before.identity).unwrap();
        assert_eq!(restored.pending_persistence(), Some(completion));
        restored.confirm_persisted(completion).unwrap();
        assert_eq!(restored.stage(), RegistrationStage::Registered);
        assert_eq!(restored.pending_persistence(), None);
        let registered = restored.checkpoint();
        assert_eq!(
            restored.confirm_persisted(completion),
            Err(RegistrationError::InvalidTransition)
        );
        assert_eq!(restored.checkpoint(), registered);
    }

    #[test]
    fn final_write_confirmation_must_match_the_current_checkpoint_revision() {
        let mut flow = workflow(2);
        let attempt = flow.begin(RegistrationOperation::RequestCode, 10).unwrap();
        flow.complete(attempt, RegistrationOutcome::Verified, 11)
            .unwrap();
        let old_write = flow.pending_persistence().unwrap();
        assert!(!flow.expire(20).unwrap());
        let before = flow.checkpoint();
        assert_eq!(
            flow.confirm_persisted(old_write),
            Err(RegistrationError::StaleCompletion)
        );
        assert_eq!(flow.checkpoint(), before);
        let current_write = flow.pending_persistence().unwrap();
        assert_ne!(current_write, old_write);
        assert!(!flow.expire(20).unwrap());
        assert_eq!(flow.pending_persistence(), Some(current_write));
        let checkpoint =
            serde_json::from_slice(&serde_json::to_vec(&flow.checkpoint()).unwrap()).unwrap();
        let mut restored =
            RegistrationWorkflow::restore(checkpoint, before.environment, before.identity).unwrap();
        assert_eq!(restored.pending_persistence(), Some(current_write));
        restored.confirm_persisted(current_write).unwrap();
        assert_eq!(restored.stage(), RegistrationStage::Registered);
    }
}
