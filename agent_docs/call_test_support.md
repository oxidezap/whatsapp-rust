# Native call test support

Enable `test-support` only for native development dependencies. The feature is
off by default and the fixture module does not compile on wasm. It uses the
existing `InMemoryBackend`, Tokio runtime and Noise certificate test utility.
It does not import the root `test_utils` module or require SQLite, a WebSocket
client, HTTP access, or native media transport.

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
cargo fmt --all -- --check
```

The existing CI shareable-feature task includes new features automatically and
runs both library and integration tests. No workflow-specific allowlist is
needed for this fixture.
