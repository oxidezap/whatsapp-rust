# Native Plugin Architecture

This document defines the native plugin contract, its lifecycle and ownership
invariants, and the boundary a future foreign-language adapter must preserve.
The implementation is intentionally native-only today: bridge, sidecar, and
wire-protocol work starts only with a concrete consumer.

## Scope

The initial plugin model supports:

- build-time registration and transactional installation;
- type-safe Rust APIs exposed through a plugin marker;
- capability-shaped access to core events, tasks, messaging, IQ, and custom
  events;
- install-scoped and connection-generation-scoped work;
- bounded custom-event delivery with explicit backpressure;
- on-demand health, resource, and queue snapshots.

It does not support dynamic installation, ingress interception, pre-ack
decisions, a foreign wire protocol, process isolation, or sandboxing. Those
features must be justified by a real use case because they add materially
stronger compatibility and durability contracts.

The host lives in the main `whatsapp-rust` crate, not `wacore`: plugins need
high-level client operations and lifecycle coordination. It is enabled by the
opt-in `plugins` feature, which enables `client-lifecycle`. A default build has
neither plugin/lifecycle fields nor their runtime branches.

The public feature surface stays intentionally small: `plugins` is the normal
opt-in and `client-lifecycle` is the advanced low-level seam for hosts that need
lifecycle integration without the native plugin host. Capabilities and
individual plugins do not become Cargo features. An external plugin crate can
enable `whatsapp-rust/plugins` in its own dependency, so Cargo feature unification
activates the host for its consumer. The host remains opt-in because LTO is not
a compatibility guarantee for client layout, reachable branches, dependencies,
compile time, or final binary size.

## Construction boundary

`ClientBuilder` is the canonical low-level construction path. It validates
dependencies at runtime and returns `Result`; `BotBuilder` remains the
typestate-preserving facade and delegates to the same path.

Construction follows one publication boundary:

1. validate client dependencies and all plugin manifests;
2. resolve plugin dependencies topologically;
3. assemble an inert `Arc<Client>`;
4. install the upstream lifecycle and plugins while staging their APIs;
5. start client services;
6. atomically activate lifecycle/plugin resources and publish the completed
   build.

Plugin tasks requested during installation remain parked until activation. A
client leaked through an installation `Weak<Client>` cannot run before the
construction gate opens. Plugin APIs, manifests, diagnostics, and custom-event
routing also remain hidden until that final publication succeeds.

Installation is transactional. Duplicate IDs or marker types, malformed
versions, missing/duplicate dependencies, and cycles fail before client
assembly. If an install fails, is cancelled, panics, or races terminal
shutdown, staged resources close synchronously and asynchronous shutdown hooks
run in reverse installation order. Staged APIs stay alive through task drains
and shutdown hooks.

Plugins are install-once for one `Client`; reconnecting does not replace the
plugin instance or its exposed API.

## Type-safe APIs

Each plugin chooses an associated API type:

```rust
use std::sync::Arc;

use anyhow::Result;
use whatsapp_rust::{ClientPlugin, PluginContext, PluginFuture, PluginManifest};

struct SearchPlugin;

struct SearchApi {
    // Clone capability handles or plugin-owned state here.
}

impl SearchApi {
    async fn search(&self, query: &str) -> Result<Vec<String>> {
        // Plugin-specific behavior.
        Ok(vec![query.to_owned()])
    }
}

impl ClientPlugin for SearchPlugin {
    type Api = SearchApi;

    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("example.search", "0.1.0")
    }

    fn install(&self, _context: PluginContext) -> PluginFuture<'_, Result<Arc<Self::Api>>> {
        Box::pin(async { Ok(Arc::new(SearchApi {})) })
    }
}
```

Register and consume it as follows:

```rust
let client = Client::builder()
    // platform dependencies...
    .with_plugin(SearchPlugin)
    .build()
    .await?
    .into_client();

let search: Arc<SearchApi> = client
    .plugin::<SearchPlugin>()
    .expect("search plugin is installed");
let matches = search.search("hello").await?;
```

The registry is keyed by `TypeId` of the plugin marker, not the API type. Two
plugins may therefore expose the same API type without colliding.
`Client::plugin::<P>()` returns `Option<Arc<P::Api>>` because the set of plugins
is selected at runtime by the builder. Encoding that set in `Client` generics
would make the client type viral and substantially increase monomorphization.

An adapter that represents runtime-defined plugins implements
`UntypedClientPlugin` and registers each instance with
`with_untyped_plugin(...)`. Those instances are keyed only by manifest ID, may
share one concrete Rust adapter type, and do not appear in
`Client::plugin::<P>()`. Native plugins keep the typed path above; a future
bridge multiplexes its language-specific handles behind the untyped adapter.
`with_untyped_plugin_arc(...)` also accepts a trait object when a host needs to
erase multiple adapter implementations before configuring the client.

During installation, `PluginContext::plugin::<P>()` exposes only directly
declared dependencies. The context keeps a weak dependency view so an API that
retains its context cannot create a registry ownership cycle. APIs should keep
plugin-owned state and cloned capability handles; they do not receive the raw
backend or Signal stores.

The workspace crate `plugins/metrics` is the public-API conformance example. It
must remain buildable without private access to the main crate.

## Capabilities and trust

A native plugin is trusted in-process Rust code. Its manifest requests
capabilities, and `PluginContext` exposes only the corresponding small handles:

| Capability identifier | Native handle | Boundary |
| --- | --- | --- |
| `events.core.observe` | `PluginCoreEvents` | Selective observation of sealed core events |
| `tasks.spawn` | `PluginTasks` / `PluginConnectionTasks` | Runtime-agnostic, cancellation-tracked work |
| `messaging.send` | `PluginMessaging` | High-level message sends |
| `iq.execute` | `PluginIq` | Typed `IqSpec` execution |
| `events.plugin.publish` | `PluginEvents` | Publication only in the plugin's own namespace |

This is API shaping, not a security boundary: native code can use any other
crate dependency available to its process. Runtime grant enforcement belongs
at a future FFI/sidecar boundary, where every foreign command must be checked.
Do not add a per-call native capability checker that suggests sandboxing it
cannot provide.

Capability handles keep `Weak<Client>` internally and reject calls before
activation or after shutdown. This avoids `Client -> plugin API -> Client`
cycles and gives terminal resource invalidation a synchronous boundary.

## Lifecycle and task ownership

The host maps the client's existing `connection_generation` to
`PluginConnectionScope`:

```text
install once
    |
    +-- install-scoped tasks ---------------------------> terminal shutdown
    |
    +-- generation N: ready -> cancel -> closed
    +-- generation N+1: ready -> cancel -> closed
```

- `install` runs once while the client is inert.
- `on_ready` runs in dependency order after authentication for that generation.
- scope cancellation is synchronous when a reconnect or terminal teardown
  starts.
- generation task cancellation is signalled before `on_closed`; the host waits
  for the drain up to its configured timeout, then continues in reverse
  dependency order and marks the plugin degraded if work remains.
- install task cancellation follows the same bounded drain before terminal
  `shutdown`; shutdown hooks run in reverse dependency order.
- the separately configured upstream lifecycle wraps the plugin order: it is
  readied first and closed/shut down last.

`PluginTasks` survives reconnects and receives cancellation only on rollback or
terminal shutdown. `PluginConnectionTasks` is tied to one generation. Their
default `spawn` drops the future when cancellation wins. `spawn_cooperative`
instead keeps polling accepted work after signalling shutdown; that future must
observe `shutdown_signal()` or `cancellation_signal()` and finish itself. The
host waits only through the configured task-drain deadline, then proceeds and
marks the plugin degraded if work remains. Plugin tasks must not block an
executor thread or detach untracked work.

`PluginHostConfig` independently configures installation, per-callback, and
per-task-drain deadlines. Installation defaults to thirty seconds; callbacks
and drains default to five seconds; all reject zero. A timed-out partial
installation is cancelled and follows the same LIFO rollback as an explicit
failure. Callbacks are serialized, bounded by their timeout, and isolated from
panics, including panics while constructing, polling, cancelling, or destroying
their futures. One faulty plugin must not suppress later callbacks. Stale
`Ready` work is bounded under reconnect pressure; every accepted `Closed`
callback is lossless and precedes terminal `Shutdown`, so the queue may
temporarily exceed its target to preserve cleanup.

`signal_shutdown_sync()` closes tasks, subscriptions, event routes, and
capability handles promptly. `disconnect().await` remains required for async
task barriers, hooks, durability flushing, and transport teardown. `Drop` can
only provide the synchronous signal.

## Event boundaries

Core events remain the sealed `wacore::types::events::Event` contract.
Subscriptions use explicit `EventInterest`; interest changes go through the
retained `Subscription`, and the aggregate 128-bit mask provides the producer
fast path. Plugin core handlers run inline, must not block, and should hand work
to a task capability. `PluginCoreEvents::subscribe` returns an owned token;
dropping or explicitly unsubscribing it removes the handler immediately, while
host shutdown invalidates tokens retained by plugin APIs. Updating its interest
also acquires or releases the `RawNode` forwarding lease in the same operation.

Custom events never enter the core enum or consume an `EventInterest` bit.
`PluginEventRouter` routes exact `(plugin_id, topic)` selectors and gives each
consumer an independently bounded queue with `DropNewest` or `DropOldest`.
Fanout shares one immutable envelope/payload across matching queues.

The native envelope carries:

- plugin ID and validated topic;
- schema version and payload encoding;
- opaque payload bytes;
- connection generation at publication;
- route-local monotonic sequence.

Dropped events consume sequence numbers so consumers can detect gaps. A route
clock resets only after its final subscriber leaves. Publishers can check the
exact route before serializing, and the router filters before constructing an
envelope; a future adapter must preserve that filter before waking or crossing
FFI.

`PluginEventSubscription` is an RAII endpoint. Dropping it atomically removes
all its selectors. Router shutdown rejects new work but lets receivers drain
already queued envelopes.

## Diagnostics

`Client::plugin_stats()` reports per-plugin lifecycle state, sticky health,
callback failures, spawned-task panics, drain timeouts, active task scopes,
subscriptions, and publisher counters. Spawned task panics are isolated during
polling and cancellation so a dead worker cannot remain falsely healthy.
`PluginEventRouter::stats()` and `PluginEvents::stats()` expose queue and
backpressure totals. `Client::memory_report()` includes plugin resources and
counts a shared queued payload once across fanout.

Snapshots are on-demand, approximate under concurrency, and contain no JIDs,
phone numbers, or message bodies. See `observability.md` for accounting rules.

## Future foreign-language adapter seam

A future bridge should be a Rust adapter at the host boundary, not a second
client lifecycle. Each runtime-defined instance can use
`UntypedClientPlugin`, so one adapter type can host multiple manifest IDs
without colliding in the native `TypeId` API registry. It can map a foreign
endpoint onto the existing semantics:

- build-time registration and stable install-scoped handles across reconnects;
- explicit capability grants checked for every foreign command;
- exact core/custom-event subscriptions before serialization or FFI wake-up;
- one bounded queue and overflow policy per endpoint;
- lifecycle events keyed by connection generation;
- event sequences, drop counters, timeouts, and typed failures;
- synchronous terminal invalidation followed by bounded asynchronous cleanup.

The native structs are not the wire schema. When a bridge consumer is in scope,
define a separate versioned protocol from a working native + foreign vertical
slice. It must specify payload/command size limits, batching, unknown-field
behavior, removed-field reservations, error codes, lifecycle deadlines, schema
generation, and drift tests. Capability identifiers may be reused, but native
traits, `Client`, runtime objects, stores, and raw backend access must not cross
that protocol.

Sidecars, WASM Components/WIT, and sandboxing remain separate decisions. A
sidecar is justified only when process isolation or a non-FFI runtime is a real
consumer requirement.

## Review checklist

When extending the host:

- keep default builds free of plugin fields, branches, and linked code;
- keep `wacore` independent of the high-level plugin host;
- add capabilities narrowly; never expose the raw backend or Signal stores;
- choose install- or connection-scoped ownership explicitly for every task;
- keep core-event handlers non-blocking and custom-event queues bounded;
- preserve LIFO rollback/shutdown and per-generation close ordering;
- isolate faults so one plugin cannot strand unrelated cleanup;
- add plugin resource accounting and health degradation for new retained state
  or failure modes;
- validate native and `wasm32-unknown-unknown` builds;
- measure both feature-disabled and enabled-with-no-plugin paths before claiming
  zero overhead;
- defer interception/pre-ack work until its interaction with
  `signal_durability.md` has a dedicated design.
