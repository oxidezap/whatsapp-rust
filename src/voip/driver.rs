//! Native (Tokio) glue over the portable [`wacore::voip::run_call`] loop: it injects the Tokio
//! runtime so the call orchestration itself stays in the sans-IO core. This is the whole "native
//! driver" -- the engine does the work; this only supplies a runtime for the timer.

use std::sync::Arc;

use wacore::runtime::Runtime;
use wacore::voip::engine::CallEngine;
use wacore::voip::transport::{RelayTransport, RelayTransportEvent};
use wacore::voip::{CallChannels, run_call};

use crate::runtime_impl::TokioRuntime;

pub use crate::voip::transport::RandTxIds;

/// Drive one call to completion on the Tokio runtime. Returns when the relay disconnects, a send
/// fails, or the event stream closes. Spawn it with the client/bot runtime and keep the
/// [`AbortHandle`](wacore::runtime::AbortHandle) (e.g. in a `CallRegistry`) to tear the call down.
pub async fn run_call_tokio(
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    engine: CallEngine,
) {
    let rt: Arc<dyn Runtime> = Arc::new(TokioRuntime);
    run_call(rt, transport, relay_events, channels, engine).await;
}
