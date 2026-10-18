use wacore_binary::Jid;

/// Borrowed metadata for an eligible inbound group/status retry receipt.
#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct RetryContext<'a> {
    pub chat: &'a Jid,
    /// Requesting device. Policies may share a budget across its user's devices.
    pub requester: &'a Jid,
    pub retry_count: u8,
}

/// Result of [`RetryAdmission::decide`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryDecision {
    Accept,
    /// Drop this receipt before sender-key repair and resend. Nothing is queued.
    Drop,
}

/// Synchronous, operator-owned gate on inbound group/status retry repair.
///
/// Configure through `ClientBuilder::with_retry_admission` or
/// `BotBuilder::with_retry_admission`. Without a policy, eligible receipts enter
/// repair. Own companion devices and DM retries bypass this policy because
/// dropping them has no sender-key fallback. The policy runs after routing and
/// unknown-device validation, before key installation, group repair and resend.
///
/// Make a fast local decision without I/O or blocking. A dropped request is not
/// queued by this client; allow recovery over time when imposing a budget.
/// Live tuning belongs inside the policy, for example with atomics.
/// See `examples/retry_quarantine.rs` for a bounded token bucket.
pub trait RetryAdmission: wacore::sync_marker::MaybeSendSync {
    fn decide(&self, context: &RetryContext<'_>) -> RetryDecision;
}
