use std::time::Duration;

/// Optional host-owned pacing of [`Client::run`](crate::Client::run) dials.
///
/// The run loop calls [`delay`](Self::delay) once before each connect attempt,
/// including the first and forced reconnects, after its ordinary reconnect
/// backoff and pause gate. It serves the returned delay **before** calling
/// [`Client::connect`](crate::Client::connect), outside the transport/version
/// connect timeouts. It then calls [`recheck`](Self::recheck), including when
/// the initial delay is zero, so a host can extend a wait after learning of a
/// shared cooldown. Existing policies need only implement `delay`.
/// Direct calls to `connect()` do not consult this policy.
///
/// Shutdown or a pause (even one already resumed) abandons the wait without a
/// dial, connect error, reconnect count, or failure-backoff increment. Resume
/// obtains a fresh decision; it does not finish the abandoned wait. A decision
/// is therefore a reservation, not proof of a dial: existing connect refusals
/// and concurrent teardown can still prevent transport creation. There is no
/// cancellation/refund callback; hosts must account for abandoned reservations.
/// The SDK still owns WhatsApp's backoff, stable reset, and rate-limit penalties.
///
/// These synchronous callbacks run inline. They must return promptly without I/O,
/// blocking, or awaiting. Shutdown cannot interrupt a blocking callback; panics
/// propagate to the caller driving the run loop, with no recovery guarantee.
/// Share a policy across builders to coordinate a host-wide budget; no limiter
/// is installed by default. An unset policy adds only an optional-field check,
/// with no per-dial allocation or spawned task. The optional trait-object `Arc`
/// occupies two pointer words inline, even when unset.
///
/// # Migration
///
/// Move host pacing out of `TransportFactory::create_transport` to avoid
/// charging that wait to the transport timeout. Manual connect pacing remains
/// the caller's responsibility. Existing builders need no changes.
///
/// ```
/// use std::{sync::Arc, time::Duration};
/// use whatsapp_rust::{Client, ConnectAdmission};
///
/// struct HostPolicy;
/// impl ConnectAdmission for HostPolicy {
///     fn delay(&self) -> Duration {
///         // Replace with a fast local reservation against a shared budget.
///         Duration::from_millis(50)
///     }
/// }
/// let shared: Arc<dyn ConnectAdmission> = Arc::new(HostPolicy);
/// let builder = Client::builder().with_connect_admission_arc(shared);
/// // Supply the usual runtime, persistence, transport and HTTP dependencies.
/// # let _ = builder;
/// ```
pub trait ConnectAdmission: wacore::sync_marker::MaybeSendSync {
    /// Reserve a run-loop attempt and return how long to delay it.
    fn delay(&self) -> Duration;

    /// Recheck an existing reservation before dialing, without reserving again.
    ///
    /// Called after the initial delay, even if zero, and after every positive
    /// extension returned here. Return zero to proceed, or a positive duration
    /// to wait before checking again. The default preserves one-shot policies.
    /// Extensions remain outside connect timeouts and are cancelled by shutdown,
    /// supervision stop, or pause just like the initial wait. Resume reserves a
    /// new attempt through `delay`.
    ///
    /// This is a synchronous snapshot, not atomic exclusion with the dial:
    /// a host update after the final zero cannot revoke that permission.
    fn recheck(&self) -> Duration {
        Duration::ZERO
    }
}
