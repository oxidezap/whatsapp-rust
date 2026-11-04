use super::EnvironmentBinding;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Identity of the public keys used by one registration attempt sequence.
/// This is an accidental-mixup guard, not authentication of persisted storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RegistrationIdentity([u8; 32]);

impl RegistrationIdentity {
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

/// Correlates a completion with its persisted attempt; stale responses are rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AttemptToken(u32);

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
#[derive(Debug, Clone)]
pub struct RegistrationWorkflow {
    checkpoint: RegistrationCheckpoint,
}

impl RegistrationWorkflow {
    pub fn new(
        environment: EnvironmentBinding,
        identity: RegistrationIdentity,
        policy: RegistrationPolicy,
    ) -> Self {
        Self {
            checkpoint: RegistrationCheckpoint {
                schema_version: 1,
                environment,
                identity,
                policy,
                stage: RegistrationStage::Ready,
                attempts: 0,
                retry_stage: RegistrationStage::Ready,
                retry_not_before_ms: 0,
                last_time_ms: 0,
            },
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
        if checkpoint.schema_version != 1
            || checkpoint.policy.max_attempts == 0
            || checkpoint.policy.request_timeout_ms == 0
            || checkpoint.attempts > checkpoint.policy.max_attempts
            || checkpoint.attempts == 0
                && (!matches!(checkpoint.stage, RegistrationStage::Ready)
                    || checkpoint.retry_not_before_ms != 0)
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
        if pending.is_some_and(|(token, operation)| {
            token.0 == 0
                || token.0 != checkpoint.attempts
                || !allows(checkpoint.retry_stage, operation)
        }) {
            return Err(RegistrationError::InvalidCheckpoint);
        }
        if let Some((token, operation)) = pending {
            checkpoint.stage = RegistrationStage::OutcomeUnknown { token, operation };
        }
        Ok(Self { checkpoint })
    }

    pub fn checkpoint(&self) -> RegistrationCheckpoint {
        self.checkpoint.clone()
    }

    pub fn stage(&self) -> RegistrationStage {
        self.checkpoint.stage
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
        let token = AttemptToken(self.checkpoint.attempts + 1);
        self.checkpoint.retry_stage = self.checkpoint.stage;
        self.checkpoint.attempts = token.0;
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

    /// Publish Registered only after the host has durably committed identity state.
    pub fn confirm_persisted(&mut self) -> Result<(), RegistrationError> {
        if self.checkpoint.stage != RegistrationStage::Verified {
            return Err(RegistrationError::InvalidTransition);
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
            RegistrationPolicy::new(max, 100).unwrap(),
        )
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
        flow.confirm_persisted().unwrap();
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
        checkpoint.schema_version = 2;
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
            token: AttemptToken(99),
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
}
