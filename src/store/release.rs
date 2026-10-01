//! Backend ownership observation, independent of connection supervision.

use super::traits::Backend;
use std::future::Future;
use std::ops::Deref;
use std::sync::Arc;
use wacore::runtime::{ShutdownNotifier, ShutdownSignal, wait_for_shutdown};

/// Observes when the crate releases its ownership of a session's storage backend.
///
/// The handle and its waiters own only notification state, never the client,
/// persistence manager or backend. Detached crate work, including an in-progress
/// saver flush, delays completion until its backend ownership is actually dropped.
/// A runtime retaining task futures despite cancellation can delay it indefinitely.
///
/// This is **not** a durability verdict or database-close guarantee. Host-owned
/// raw backend handles (including `Device`/device snapshots returned to the host),
/// backend-internal I/O and other users of the same database are outside this
/// boundary and may keep the database open after completion.
///
/// A low-level host can replace `Weak<Client>` polling with this separate fence:
///
/// ```no_run
/// use std::sync::Arc;
/// use whatsapp_rust::Client;
///
/// async fn stop(client: Arc<Client>) {
///     let released = client.store_release();
///     let report = client.shutdown().await;
///     drop(client); // do not await release while retaining a client/manager
///     released.wait().await;
///     let _ = report; // independently inspect cleanup/durability results
/// }
/// ```
#[derive(Clone)]
pub struct StoreRelease {
    signal: ShutdownSignal,
}

impl StoreRelease {
    /// Wait for ownership release. The returned future borrows nothing, can be
    /// boxed/spawned, and may be cancelled without affecting backend ownership.
    /// Multiple and late waiters observe the same sticky release.
    pub fn wait(&self) -> impl Future<Output = ()> + Send + use<> {
        wait_for_shutdown(&self.signal)
    }
}

/// Crate-only ownership lease. Dereferencing borrows the original backend;
/// cloning shares ownership without replacing its public Arc identity.
#[derive(Clone)]
pub(crate) struct BackendLease(Arc<BackendOwnership>);

struct BackendOwnership {
    backend: Arc<dyn Backend>,
    // Declaration order is the release fence: the actual backend Arc drops
    // BEFORE notification. A Drop body on BackendOwnership would be too early.
    release: ReleaseGuard,
}

struct ReleaseGuard(ShutdownNotifier);

impl Drop for ReleaseGuard {
    fn drop(&mut self) {
        self.0.notify();
    }
}

impl BackendLease {
    pub(crate) fn new(backend: Arc<dyn Backend>) -> Self {
        Self(Arc::new(BackendOwnership {
            backend,
            release: ReleaseGuard(ShutdownNotifier::new()),
        }))
    }

    pub(crate) fn observer(&self) -> StoreRelease {
        StoreRelease {
            signal: self.0.release.0.subscribe(),
        }
    }

    pub(super) fn raw(&self) -> Arc<dyn Backend> {
        self.0.backend.clone()
    }
}

impl Deref for BackendLease {
    type Target = dyn Backend;

    fn deref(&self) -> &Self::Target {
        self.0.backend.as_ref()
    }
}

#[cfg(test)]
#[path = "release_tests.rs"]
pub(crate) mod tests;
