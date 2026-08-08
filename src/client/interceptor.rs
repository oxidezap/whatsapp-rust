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
//! An interceptor is that room. It runs where dispatch would have, and either
//! steps aside or claims the stanza — in which case the built-in handler is
//! skipped, and whatever answer the client owed the server becomes the
//! claimant's to send. For most tags that answer is a transport ack and the
//! client still sends it; see [Acknowledgement](#acknowledgement) for the ones
//! where it is not.
//!
//! # What an interceptor sees
//!
//! Stanzas that would have reached dispatch. A response the client already
//! correlated to a pending request, a `<xmlstreamend>` that ends the stream,
//! and the connection-critical tags below never get there, so an interceptor
//! does not see them either.
//!
//! To observe *everything* decoded, including those, use
//! [`Event::RawNode`] — it is emitted before any of the early returns.
//! The two are different tools: one watches, this one takes over.
//!
//! [`Event::RawNode`]: wacore::types::events::Event::RawNode
//!
//! # What cannot be claimed
//!
//! `success`, `failure`, `stream:error` and `ack` settle connection state:
//! authentication, shutdown and reconnection, and the waiters a send blocks on.
//! An interceptor that took one would not extend the client — it would leave it
//! authenticated-but-unaware, or never reconnecting, or waiting forever on a
//! send that already completed. They are never offered.
//!
//! Housekeeping is likewise untouched. Offline-sync tracking and
//! response-waiter resolution run before dispatch and keep running whether or
//! not a stanza is claimed.
//!
//! A server-initiated `<iq>` ping is not offered either, for the same reason as
//! the four above: a claimed ping is a pong never sent, and the server drops
//! the connection over it.
//!
//! Every other `<iq>` *is* offered, including ones the client answers on its
//! own — a pairing step, a query it models. Most `<iq>` traffic is exactly what
//! a consumer would want to extend. Claiming one leaves the server without the
//! reply it expects, so match narrowly.
//!
//! # Acknowledgement
//!
//! A claim does not change what the server is owed. Where the client would have
//! acked, it still acks; where it would have nacked a tag it does not model,
//! the claim turns that into an ack, because someone did handle it — answering
//! nothing would leave the stanza in the offline queue and keep the stream
//! recycling. Both need `id` and `from`: without them there is nothing to
//! address, and the client would not have answered either.
//!
//! A tag the client *does* model but answers some other way is answered by
//! nobody once claimed. A direct `<message>` draws a delivery `<receipt>` and
//! an `<iq>` draws an `<iq type="result">`; a generic `<ack>` is neither, so
//! the client does not send one. Claiming those means owing the reply.
//!
//! `<receipt>`, `<notification>` and `<call>` are not in that group: the client
//! answers those with a transport `<ack>` already, so a claim leaves the ack
//! exactly where it was.
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
    /// The built-in pipeline is skipped. Where the client's answer was a
    /// transport ack it still sends one; where the answer was something else —
    /// a delivery `<receipt>` for a direct `<message>`, an `<iq type="result">`
    /// — nothing is sent, and the claimant owes that reply. See the
    /// [acknowledgement](index.html#acknowledgement) rules.
    Handled,
}

impl Interception {
    /// Whether the built-in pipeline should be skipped.
    #[must_use]
    pub const fn is_handled(self) -> bool {
        matches!(self, Self::Handled)
    }
}

/// Sees a stanza on its way to the built-in pipeline.
///
/// Not every decoded stanza: see the module documentation for what never
/// reaches this point.
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
#[derive(Clone)]
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
