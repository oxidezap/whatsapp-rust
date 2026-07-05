use wacore_binary::Jid;

/// Opt-in policy that decides whether an inbound group or status retry receipt
/// enters the repair path, or is dropped before any work runs.
///
/// WhatsApp Web has no volume-based throttle on inbound retry receipts: it
/// serializes them per chat, refuses past `MAX_RETRY`, and otherwise processes
/// every one (`markForgetSenderKey`, key-bundle processing, resend). This SDK
/// mirrors that by default. In a large group a cohort of members whose pairwise
/// sessions never establish can send a retry for every single message, driving
/// that repair path at storm rate. A single-user WA Web client never sends at
/// the volume that produces this; a bot does.
///
/// A `RetryAdmission` policy is the seam for bot-scale deployments that want to
/// bound that cost, without the SDK itself diverging from WA Web. Registering a
/// policy is a deliberate, operator-owned decision to drop some eligible repair
/// requests; leaving it unset keeps exact WA Web behavior. The check is a single
/// [`std::sync::OnceLock::get`] on the receive path, so an unset policy costs
/// nothing.
///
/// Scope: the policy is consulted only for group and `status@broadcast` retry
/// receipts from other accounts. Retries from our own companion devices
/// (`is_peer`) and all DM retries are always admitted and never reach the
/// policy, because dropping those has no safe SKDM fallback. When the policy
/// returns `false`, the receipt is dropped before the group-info fetch, the
/// unknown-device `rotateKey` block, `markForgetSenderKey`, key-bundle
/// processing and the resend. A dropped receipt is not queued: the requester
/// re-requests on its own timer, so a policy should refill over time to keep
/// genuine recovery possible.
///
/// See `examples/retry_quarantine.rs` for a token-bucket implementation keyed by
/// (chat, requester).
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait RetryAdmission: wacore::sync_marker::MaybeSendSync {
    /// Return `true` to admit the retry receipt (WA Web behavior), `false` to
    /// drop it before any repair work runs.
    ///
    /// `chat` is the group or `status@broadcast` JID, `requester` the retrying
    /// participant device, and `retry_count` the receipt's attempt number. The
    /// device is intentionally part of `requester` but a policy may key on the
    /// user only: WA Web re-targets a whole user's sender key, so all devices of
    /// one broken account can share a budget.
    async fn admit(&self, chat: &Jid, requester: &Jid, retry_count: u8) -> bool;
}
