use wacore_binary::Jid;

/// Borrowed metadata for an eligible automatic decrypt-failure retry.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct DecryptFailureContext<'a> {
    pub chat: &'a Jid,
    pub sender: &'a Jid,
    /// Attempt number after per-message accounting, including refused attempts.
    pub retry_count: u8,
}

/// Result of [`DecryptFailureAdmission::decide`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecryptFailureDecision {
    RequestRetry,
    /// Skip the retry receipt, but transport-acknowledge and emit the
    /// undecryptable event. The first-attempt phone recovery still runs.
    AcknowledgeWithoutRetry,
}

/// Synchronous, operator-owned gate on automatic outbound retry receipts.
///
/// Configure through `ClientBuilder::with_decrypt_failure_admission` or
/// `BotBuilder::with_decrypt_failure_admission`. Without a policy, eligible
/// decrypt failures request a retry up to the per-message cap. Own messages,
/// explicit retry requests, permanently suppressed bot retries and attempts
/// past the cap bypass this policy and never consume its budget.
///
/// Make a fast local decision without I/O or blocking. Refusal still consumes
/// a per-message attempt, retains first-attempt phone recovery and acknowledges
/// the stanza. The server therefore stops redelivering it; refusal can lose
/// recovery of this message. Live tuning belongs inside the policy.
/// The callback must return a decision. Panics are not translated into a
/// fallback verdict. They follow the application's panic strategy; with
/// unwinding enabled, they propagate through the task invoking the policy.
pub trait DecryptFailureAdmission: wacore::sync_marker::MaybeSendSync {
    fn decide(&self, context: &DecryptFailureContext<'_>) -> DecryptFailureDecision;
}
