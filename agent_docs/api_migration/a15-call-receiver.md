# A15: one owner for call events

`CallHandle::events()` is removed. It previously cloned a competitive receiver each
time it was called, so two apparently independent observers silently divided the
call's lifecycle and diagnostic events. Acquire once instead:

```rust,ignore
// Before
let events = call.events();
// After (in the component responsible for the consumer loop)
let events = call.take_events().ok_or_else(|| anyhow::anyhow!("call events already owned"))?;
while let Ok(event) = events.recv().await {
    // process event
}
```

All clones of one `CallHandle` share the acquisition slot. The first
`take_events(&self)` returns `Some(async_channel::Receiver<CallEvent>)`; every
subsequent attempt returns `None`, without panic. Dropping the receiver, dropping
the acquiring handle, cancelling a receive future, or ending the call does **not**
restore acquisition. Keep the receiver, or move it to its next owner. A first
acquisition after termination can still drain queued events before channel closure.
There is no legacy accessor that bypasses this rule.

The receiver itself remains cloneable for hosts deliberately implementing competing
workers. Its clones share one queue; they are not independent subscriptions. This
API guarantees one acquisition through the handle, not an uncloneable endpoint or
exclusive access against a backend that separately exposes its session. It does
not promise exactly-once processing, replay, redelivery, or broadcast. Capacity,
overflow, and event production remain the backend's existing contract; this change
adds no queues and does not increase their capacities. The handle no longer retains
a hidden receiver after acquisition. A host that drops every acquired receiver may
therefore cause subsequent backend publication to fail; receiver Drop is not hangup.

`wait_ended()` remains sticky and independent of event consumption. Handle Drop
still does not end the call. `hangup_local()` is silent and local;
`terminate()` still attempts peer notification and reports `PeerNotified`,
`PartlyNotified`, `LocalOnly`, or `AlreadyEnded`. Call generations and weak Client
ownership are unchanged.

## Imports, features, and checks

Use `whatsapp_rust::voip::{CallHandle, CallEvent}` (`voip-control`, also enabled by
`voip-runtime` and the codec profiles). The return type is
`whatsapp_rust::async_channel::Receiver<CallEvent>`. No new public DTO or required
backend trait method was introduced. The neutral `voip_control` API remains
engine-free, and portable `RelayTransportProvider` injection is unchanged.

The standalone consumer in `tests/fixtures/api_a15_call_receiver` implements the
external backend and names the final acquisition type without depending directly
on wacore, Tokio, or the resident engine. Invoke Cargo from outside the checkout
so it does not inherit the workspace's `.cargo/config.toml` (set `CHECKOUT` first):

```sh
manifest="$CHECKOUT/tests/fixtures/api_a15_call_receiver/Cargo.toml"
cd /
cargo +nightly-2026-06-16 check --manifest-path "$manifest"
cargo +nightly-2026-06-16 check --manifest-path "$manifest" --features runtime
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo +nightly-2026-06-16 check \
  --manifest-path "$manifest" --target wasm32-unknown-unknown
```

Set a private `CARGO_TARGET_DIR` for these commands when running concurrent lanes.

Native ownership tests live in `src/voip/facade.rs::event_ownership_tests`; they run
with `voip-control` alone as well as the runtime. WASM checks are compilation only,
not browser or embedded execution. No new protocol investigation is needed: this
changes consumer ownership, not signaling or wire behavior.
