//! Taking over a stanza before the built-in pipeline sees it.
//!
//! The client models the stanzas it knows about and nacks the rest, which is
//! the right default: a `<nack>` tells the server this client cannot act on
//! something, and silence would leave the stanza in the offline queue forever.
//!
//! But it leaves no room for a consumer that *can* act on it. A stanza this
//! version does not model gets nacked whether or not the application would have
//! known what to do with it, and there is no way to say otherwise:
//! [`StanzaRouter::register`] panics on a duplicate tag, so even a handler for
//! an existing tag cannot be replaced.
//!
//! An interceptor is that room. It runs before dispatch, sees every decoded
//! stanza, and either steps aside or claims the stanza — in which case the
//! built-in pipeline is skipped and the stanza is acknowledged normally, so the
//! server does not redeliver it.
//!
//! # What claiming does not skip
//!
//! Handling, not housekeeping. Offline-sync tracking, response-waiter
//! resolution and stream shutdown run before dispatch and keep running whether
//! or not a stanza is claimed — they are what keeps the connection working, and
//! an interceptor that could switch them off would be a way to break a client
//! rather than to extend one.
//!
//! The acknowledgement is the same: a claimed stanza is acked exactly as it
//! would have been. The server is owed one either way.
//!
//! [`StanzaRouter::register`]: crate::handlers::router::StanzaRouter::register
//!
//! # Cost
//!
//! Nothing runs while no interceptor is registered: the read loop checks one
//! relaxed atomic and carries on. Registering is what turns the check into a
//! walk.
//!
//! # Example
//!
//! Handling a stanza the client does not model, instead of nacking it:
//!
//! ```no_run
//! use std::sync::Arc;
//! use whatsapp_rust::client::interceptor::{Interception, StanzaInterceptor};
//! use wacore_binary::node::OwnedNodeRef;
//!
//! struct Vendor;
//!
//! impl StanzaInterceptor for Vendor {
//!     fn intercept(&self, node: &OwnedNodeRef) -> Interception {
//!         if node.tag() == "vendor:thing" {
//!             // … act on it …
//!             Interception::Handled
//!         } else {
//!             Interception::Pass
//!         }
//!     }
//! }
//!
//! # fn example(client: &Arc<whatsapp_rust::Client>) {
//! let handle = client.add_stanza_interceptor(Arc::new(Vendor));
//! // Dropping `handle` removes it.
//! # let _ = handle;
//! # }
//! ```

use std::sync::{Arc, Weak};

use wacore::sync_marker::MaybeSendSync;
use wacore_binary::node::OwnedNodeRef;

use crate::Client;

/// What an interceptor decided about a stanza.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Interception {
    /// Leave the stanza to the client.
    ///
    /// The default, so an interceptor that only cares about one tag needs no
    /// branch for the rest.
    #[default]
    Pass,
    /// The interceptor took the stanza.
    ///
    /// The built-in pipeline is skipped. The stanza is still acknowledged the
    /// way it would have been, because the server is owed an ack either way —
    /// withholding one leaves the stanza queued for redelivery.
    Handled,
}

impl Interception {
    /// Whether the built-in pipeline should be skipped.
    #[must_use]
    pub const fn is_handled(self) -> bool {
        matches!(self, Self::Handled)
    }
}

/// Sees each decoded stanza before the built-in pipeline.
///
/// Runs on the read loop, so it must return quickly: time spent here is time
/// the next stanza waits. Work that can take a while belongs on a task.
///
/// Must not panic. Like [`EventHandler`], this is called directly by the read
/// loop, and an unwind there takes the connection with it. The plugin host
/// catches panics from plugins; a directly registered interceptor is trusted.
///
/// [`EventHandler`]: wacore::types::events::EventHandler
pub trait StanzaInterceptor: MaybeSendSync + 'static {
    /// Decide what happens to `node`.
    fn intercept(&self, node: &OwnedNodeRef) -> Interception;
}

impl<F> StanzaInterceptor for F
where
    F: Fn(&OwnedNodeRef) -> Interception + MaybeSendSync + 'static,
{
    fn intercept(&self, node: &OwnedNodeRef) -> Interception {
        self(node)
    }
}

/// Keeps an interceptor registered. Dropping it removes the interceptor.
///
/// Holds a weak client reference, so a forgotten handle cannot keep a client
/// alive.
#[must_use = "dropping the handle immediately removes the interceptor"]
#[derive(Debug)]
pub struct InterceptorHandle {
    pub(crate) client: Weak<Client>,
    pub(crate) id: u64,
}

impl Drop for InterceptorHandle {
    fn drop(&mut self) {
        if let Some(client) = self.client.upgrade() {
            client.remove_stanza_interceptor(self.id);
        }
    }
}

/// One registered interceptor.
pub(crate) struct Registration {
    pub(crate) id: u64,
    pub(crate) interceptor: Arc<dyn StanzaInterceptor>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_is_the_default_so_narrow_interceptors_stay_short() {
        assert_eq!(Interception::default(), Interception::Pass);
        assert!(!Interception::Pass.is_handled());
        assert!(Interception::Handled.is_handled());
    }

    #[test]
    fn a_closure_is_an_interceptor() {
        fn takes(_: impl StanzaInterceptor) {}
        takes(|_node: &OwnedNodeRef| Interception::Pass);
    }

    #[test]
    fn interception_is_comparable_and_debuggable() {
        assert_eq!(Interception::Pass, Interception::Pass);
        assert_ne!(Interception::Pass, Interception::Handled);
        assert!(!format!("{:?}", Interception::Handled).is_empty());
    }
}
