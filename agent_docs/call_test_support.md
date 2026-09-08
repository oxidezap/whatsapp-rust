# Native call test support

Enable `test-support` only for native development dependencies. The feature is
off by default and the fixture module does not compile on wasm. It uses the
existing `InMemoryBackend`, Tokio runtime and Noise certificate test utility.
It does not import the root `test_utils` module or require SQLite, a WebSocket
client, HTTP access, or native media transport.

The native target guard also applies when `cfg(test)` is set. The shared
session helper, its test-utils delegate, and the native caller suites are
excluded together, so WASM tests cannot pull in blocking session setup through
the test-only path.

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dev-dependencies]
whatsapp-rust = { workspace = true, features = ["test-support"] }
```

The workspace dependency must already point at a revision containing this
feature. Use the same git source and branch for the WhatsApp workspace crates;
do not introduce a second copy just for the fixture.

## Public API

`whatsapp_rust::test_support::CallFixture` is the native fixture.

- `new().await` completes production Noise XX, login and empty offline delivery.
  It waits on the production readiness notification. There are no readiness
  stores or setters in the fixture.
- `client()` returns the real `Arc<Client>`. Use its normal outgoing builder,
  event subscriptions and `CallHandle` methods.
- `peer()` names the fictitious recipient. Its primary device 0 and companion
  device 2 have seeded device records and Signal sessions.
- `next_offer().await` returns `PendingOffer` at transport entry. Its `stanza()`
  contains the actual production video advertisement and encrypted per-device
  destinations. `complete()` releases send completion; `fail()` or dropping it
  fails the send. None of these operations acknowledges the offer.
- `inject(Node).await` marshals a stanza and runs production `process_node`,
  routing and handlers. It returns after handling and synchronous sends, not
  merely after queueing. A malformed stanza can be ignored by the real handler;
  returning `Ok` does not certify that the handler accepted its action.
- `call_snapshot(id)` reads a detached production session snapshot. This can
  observe winner selection while the outgoing builder is still blocked. It
  cannot mutate the live registry.
- `outgoing_stanzas()` returns decoded stanzas in transport-entry order, not a
  claim that pending sends completed. `events()` returns production event-bus
  events. Both report observation overflow rather than silently truncating.
- `shutdown().await` disconnects the real client and joins its reader. Dropping
  the fixture signals shutdown and lets the reader perform production cleanup.

Before construction succeeds, an abort-on-drop guard owns the reader. If
`Client::connect` fails, `new` joins that reader and returns its original typed
error rather than the readiness channel's closure. Constructor deadlines abort
and join the reader; cancellation aborts it when the constructor future drops.
Only successful construction transfers reader ownership to the fixture's
graceful teardown path. `shutdown` also guards a reader taken out for joining,
so a cancelled or timed-out join cannot leave it running detached.

There is no fabricated `OutgoingReady`, winner setter or event sender. Subscribe
through `client().subscribe_handler(...)` for callbacks. For the complete inbound
accept advertisement, hold `client().acquire_raw_node_forwarding()` and inspect
`Event::RawNode`; `Event::IncomingCall` is the production parsed representation.

## Example

```rust
use std::sync::Arc;
use whatsapp_rust::test_support::CallFixture;
use wacore_binary::builder::NodeBuilder;

# async fn example() -> anyhow::Result<()> {
let fixture = Arc::new(CallFixture::new().await?);
let client = fixture.client().clone();
let peer = fixture.peer().clone();
let (_mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(1);
let (speaker_tx, _speaker_rx) = async_channel::bounded::<Vec<i16>>(1);
let (_video_tx, video_rx) = async_channel::bounded::<Vec<u8>>(1);
let (sink_tx, _sink_rx) = async_channel::bounded::<wacore::voip::VideoFrame>(1);
let starting = tokio::spawn(async move {
    client.voip().call(&peer)
        .audio(mic_rx, speaker_tx)
        .video(video_rx, sink_tx)
        .start().await
});

let offer = fixture.next_offer().await?;
assert!(!starting.is_finished());
// Inspect offer.stanza() here. The production send is still pending.
offer.complete()?;
let handle = starting.await??;
assert_eq!(handle.peer_jid(), *fixture.peer());

let winner = fixture.peer().clone().with_device(2);
fixture.inject(NodeBuilder::new("call")
    .attr("from", winner.clone())
    .attr("id", "SYNTHETIC-ACCEPT")
    .attr("t", "1788840000")
    .children([NodeBuilder::new("accept")
        .attr("call-id", handle.call_id())
        .attr("call-creator", fixture.client().lid().unwrap())
        .children([
            NodeBuilder::new("audio").attr("enc", "opus").attr("rate", "16000").build(),
            NodeBuilder::new("video").attr("dec", "H264").attr("device_orientation", "0").build(),
        ]).build()])
    .build()).await?;
assert_eq!(handle.peer_jid(), winner);
fixture.shutdown().await?;
# Ok(())
# }
```

For acceptance before the builder returns, keep `PendingOffer` outstanding,
spawn `inject` separately, and wait until `call_snapshot(id).answering_device`
shows the handler's selection. Sibling dismissal blocks behind the pending
offer send. Complete the offer, then await both futures. The external test
`accept_before_builder_completion_selects_winner_through_handler` demonstrates
this ordering without a sleep or a readiness setter.

## Coverage boundaries

### Ordered peer video events

The production `whatsapp_rust::voip::CallEvent::PeerVideoStateChanged` variant
is available with `voip-runtime`, including on wasm. It does not require the
native `test-support` feature. Its fields are:

```text
source: Jid
call_creator: Jid
state: VideoState
orientation: Option<u8>
upgrade_token: Option<VideoUpgradeToken>
```

Consume this variant from one `CallHandle::events()` receiver for all peer
video states, including upgrade requests, accepts and stops. Pass its token to
`accept_video` when accepting a request. Do not join a separate global
`IncomingCall` stream to recover identity or update the same state from that
stream; the two consumers can run in a different order.

The source-bearing event is published after the existing typed-ACK and state
commit checks, while holding the existing video-transition lock. Direct calls
then publish the unchanged legacy `VideoStateChanged` on that same queue with
identical state, orientation and token. New consumers must ignore the legacy
companion; existing consumers can keep matching the legacy variant and ignore
unknown variants. The old variant's fields and token type are unchanged.

`source` is the parsed `participant` when present, otherwise `from`. It is not
replaced with the stored winning device. `call_creator` is the incoming value,
not an inference from the matched call ID. Group PN aliases remain PN aliases
in the event, while the existing orientation path still canonicalizes them for
the media registry. Groups publish only the new participant-scoped variant,
with no upgrade token, after the existing post-ACK roster reauthorization.
They still do not emit call-wide legacy video state or enter direct negotiation.

Neither identity field certifies authorization. The current direct handler can
apply a sibling's state or a state carrying a different creator, and the event
reports those inputs faithfully so a consumer can apply its own policy.

The queue keeps its bounded eviction behavior. It is not a lossless history or
an atomic pair mailbox. Normal handles have room for both variants; a custom
single-slot core queue retains the legacy event, preserving its old behavior.
Both JID allocations are included in the existing queue byte accounting.

The fixture regression first failed on the original implementation with four
states in order but four absent sources. It now checks source B's upgrade
accept followed by source A's stop, then a request/token from B followed by a
stop from A. Every direct pair agrees exactly, and the stopped request's token
is expired. Separate tests cover routed identity, supplied creator, ignored
states, group aliases, ACK ordering and legacy single-slot behavior.

This changes the in-process event API, not wire signaling, codecs, sender
matching or authorization. It is not evidence of WhatsApp protocol parity.

### Fixture transport

The synthetic server performs real Noise key agreement and authenticates
outgoing encrypted frames. Its certificates and ADV identity are synthetic;
the fixture uses the existing per-client certificate-signature bypass, never
the production default. Seeded Signal peers are not remote WhatsApp receivers.
The fixture proves builder/handler behavior, not peer decryption or protocol
equivalence.

Explicit stanza injection bypasses inbound Noise framing but not the parser or
handler. The normal reader runs for login IQ replies and the offline marker.
Only the active-mode IQ gets a successful synthetic response; unsupported IQs
get an explicit 503. HTTP is refused. Media is dormant because no offer ACK is
generated; an installed refusing relay provider also prevents accidental real
networking when native media features are enabled. To test media, install your
own synthetic `RelayTransportProvider` before injecting a relay-bearing ACK.

The fixture deliberately preserves the base revision's weak direct-call policy.
An unrung device can become the first winner. A later non-busy sibling reject
can terminate the winning call. A busy reject leaves the call ringing, and the
first accept dismisses the other device with `accepted_elsewhere`. A late
accept does not replace the selected winner. The tests characterize these
behaviors; they do not declare them secure or add filtering to hide them.

## Verification

```sh
cargo test -p whatsapp-rust --no-default-features --features test-support --test voip_call_fixture
cargo clippy -p whatsapp-rust --no-default-features --features test-support --test voip_call_fixture -- -D warnings
cargo check -p whatsapp-rust --no-default-features --features test-support --lib
cargo check -p whatsapp-rust --no-default-features --lib
cargo test -p whatsapp-rust --features voip-mlow,test-support --lib voip::facade::tests
cargo test -p whatsapp-rust --features test-support --lib test_support::call::tests
cargo test -p whatsapp-rust --test native_test_support_cfg -- --ignored
cargo fmt --all -- --check
```

The existing CI shareable-feature task includes new features automatically and
runs both library and integration tests. No workflow-specific allowlist is
needed for this fixture.

The explicit cfg probe requires an installed `wasm32-unknown-unknown` target.
It extracts the actual fixture/helper/caller guards with `syn` and compiles
them with rustc on WASM for all four combinations of test and test-support
cfgs. A native positive control proves every checked path remains enabled.
This probe checks exclusion, not the complete root unit-test dependency graph.
The latter still enables Tokio's `full` dev feature and fails in `mio` on
`wasm32-unknown-unknown` before the root tests compile. Production WASM checks
with and without `test-support` do not have that dev-dependency limitation.
