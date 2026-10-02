# Events consolidation contract (assessment §5.6)

Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`.
PR: https://github.com/oxidezap/whatsapp-rust/pull/1607

## Intentional source breaks / migration

| Removed | Canonical replacement |
| --- | --- |
| `Client::register_chatstate_handler(observer)` | `let subscription = client.subscribe_chatstate_handler(observer);` |
| `EventDelivery::Concurrent` | `EventDelivery::ConcurrentUnbounded` (explicit opt-in only) |

Store the subscription in the observer owner's state, or retain it in the scope
that should receive events. Do not immediately discard it. Async observers or
hosts selecting delivery limits use `CallbackEventHandler::from_callback` and
`client.subscribe_handler(handler)` instead, retaining that subscription too.

These removals are deliberate, without deprecated aliases or a version/release
change in this PR. The repository's last permanent chatstate caller now owns a
collection of subscriptions. The external consumer has negative compile-fail
controls for both removed names.

## Retained capabilities and limits

- `ChatStateEvent` and its adapter remain: real consumers use its three-state
  `ReceivedChatState` and optional group participant. This avoids repeating the
  presence/media conversion and source-to-participant projection. Its payload
  comes exclusively from the shared `Event::ChatPresence` bus fact; there is no
  second dispatcher.
- The callback default remains `BoundedConcurrent { capacity: 256,
  max_concurrency: 16 }`. `Ordered`, bounded concurrency and explicit unlimited
  concurrency retain their previous behavior. Chatstate's ordered mailbox stays
  at 256. Overflow drops the newest event; these observations are not durable.
- Host-implemented `EventHandler` remains synchronous, inline outside the bus
  table lock. Return promptly: non-blocking enqueue or host-owned scheduling.
  The async adapter still invokes callbacks on workers, never during dispatch.
- `Subscription` remains owned, with drop/unsubscribe, interest update and
  explicit detach. Removal stops future dispatch snapshots; an old snapshot may
  still deliver. It cannot retract events already accepted by a channel or a
  separately retained callback adapter. `CallbackEventHandler::cancel()` aborts
  accepted work explicitly; dropping its last owner also cancels it.
- `ChannelEventHandler::new()` stays bounded at 256, `with_capacity(0)` clamps to
  one, and `unbounded()` remains an explicitly unlimited host capability.
- Terminal and driver-scope cancellation, weak Client ownership, Runtime,
  protocol parsing, wire representation, queue limits and stats are unchanged.

## Export paths for the consolidated guide

Existing paths remain sufficient; no new aliases/reexports were added:
`whatsapp_rust::{ChatStateEvent, CallbackEventHandler, EventDelivery,
EventDeliveryStats}`, `whatsapp_rust::types::events::{EventHandler, Subscription,
EventInterest, ChannelEventHandler}`, and the existing prelude event exports.

The final cross-domain/versioned migration guide belongs to A08. This bounded
contract is its events input, not a replacement for that guide or release notes.
