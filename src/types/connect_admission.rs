use std::time::Duration;

/// Optional host-owned pacing of [`Client::run`](crate::Client::run) dials.
///
/// The run loop consults this policy once before starting each connect attempt,
/// including the first and forced reconnects, after its ordinary reconnect
/// backoff and pause gate. It serves the returned delay **before** calling
/// [`Client::connect`](crate::Client::connect), outside the transport/version
/// connect timeouts. Zero proceeds without sleeping or consulting again.
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
/// This synchronous callback runs inline. It must return promptly without I/O,
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
}
