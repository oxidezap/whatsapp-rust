//! Native (Tokio) glue over the portable [`wacore::voip::run_call`] loop: it injects the Tokio
//! runtime so the call orchestration itself stays in the sans-IO core. This is the whole "native
//! driver" -- the engine does the work; this only supplies a runtime for the timer.

#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
use std::sync::Arc;

#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
use wacore::runtime::Runtime;
#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
use wacore::voip::engine::CallEngine;
#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
use wacore::voip::transport::{RelayTransport, RelayTransportEvent};
#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
use wacore::voip::{CallChannels, run_call};

#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
use crate::runtime_impl::TokioRuntime;

/// OS-RNG-backed STUN transaction ids for production calls. The core's `SequentialTxIds` is
/// deterministic (test-only); real calls need unpredictable ids for consent freshness.
///
/// Here rather than beside the native relay, where it was: it reads an RNG and nothing else, so
/// binding it to the one transport that owns a UDP socket left a page with no way to name a
/// transaction id at all.
#[derive(Default)]
pub struct RandTxIds;

impl wacore::voip::engine::TxIdSource for RandTxIds {
    fn next_tx_id(&mut self) -> [u8; 12] {
        rand::random()
    }
}

/// Drive one call to completion on the Tokio runtime. Returns when the relay disconnects, a send
/// fails, or the event stream closes. Spawn it with the client/bot runtime and keep the
/// [`AbortHandle`](wacore::runtime::AbortHandle) (e.g. in a `CallRegistry`) to tear the call down.
///
/// The facade no longer goes through this -- it drives `run_call` on the client's own runtime,
/// which is the only shape a page can satisfy -- so this is the convenience for a consumer that
/// has a Tokio runtime and would rather not name one. Absent where `TokioRuntime` is: it spawns
/// through `tokio::spawn`, which on wasm32 compiles and then panics, and a function that is only
/// there to trap is worse than one that is not there.
#[cfg(all(feature = "tokio-runtime", not(target_arch = "wasm32")))]
pub async fn run_call_tokio(
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    engine: CallEngine,
) {
    let rt: Arc<dyn Runtime> = Arc::new(TokioRuntime);
    run_call(rt, transport, relay_events, channels, engine).await;
}
