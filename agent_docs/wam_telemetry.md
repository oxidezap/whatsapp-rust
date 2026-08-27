# WAM telemetry

WAM is the telemetry the official WhatsApp client uploads about itself: a binary
buffer of numbered events, preceded by buffer-level globals, sent under
`<iq xmlns="w:stats">`. This document is the record of how it is modelled here,
what it may and may not say, and why the honest version is much smaller than the
catalog.

Read it before adding a WAM event, before changing the codec, and before
believing that a field can be filled.

Two crates, neither in the default build:

| crate | what it is |
| --- | --- |
| `plugins/wam-catalog` (`whatsapp-rust-wam-catalog`) | the generated catalog (every event, field id, enum member, global and constant WA Web declares) plus the buffer codec |
| `plugins/wam` (`whatsapp-rust-plugin-wam`) | the runtime: observation, sampling, buffering, flush, upload |

The `git diff` evidence for "the core gains nothing" is narrower than it sounds
and is worth stating precisely: production code under `src/` and `wacore/` is
untouched apart from the files whatspec regenerates, and one pinned literal in a
`mod tests` moved with the spec bump.

## The one rule

**An event is emitted only when every field it writes is honestly derivable from
activity this client actually saw.** No invented value, no sentinel, no
placeholder. A field this client does not know is absent, and the buffer format
distinguishes absent from zero, so absence is a thing the server can read.

The rule is enforced, not asserted. `plugins/wam/src/parity.rs` builds every
emittable event at its maximum, encodes it through the real codec, reads the
field ids back out of the bytes, and checks each against the call sites whatspec
recovered from WA Web. A field the official client never writes fails the build.

What that check does *not* prove is the value. A call site says WA Web writes
`messageType` on `MessageReceive`; nothing static says what it puts there. That
half is ordinary review, and it is why every mapping in `derive.rs` carries the
reasoning for what it declines to map.

## Why a plugin

`subsystem_boundary.md` asks four questions of anything attaching to the core.
WAM fails the first: a subsystem is entered on a dispatch key the core already
routes on (a stanza tag, a notification type, an IQ namespace) and WAM claims
none. It wants to *watch* work the core does for its own reasons, which is the
shape that document calls coupled.

Nor does it clear the bar for a fifth `Subsystem` hook, which needs two askers
and a measured floor. One subsystem wanting an observation point is that
subsystem's problem, and the plugin host already solves it: `events.core.observe`
is exactly a watcher's seam. The result is that the core gains nothing: no
field, no feature gate, no line, which `tests/subsystem_boundary.rs` and the
`git diff` above both confirm.

What the capability surface is missing is recorded under "What is not emitted"
below, since it is the same list.

## The buffer format

```text
"WAM" | version:u8 | streamId:u8 | sequence:u16le | channel:u8
then, per record: flags:u8 | id:u8|u16le | payload
```

`flags` low two bits: 0 global, 1 event, 2 field. Bit 2 marks the last record of
an event's group. Bit 3 widens the id. The high nibble is the payload's size
class: no payload for 0 and 1, then i8/i16/i32, f64, and three string classes
whose length prefix is one, two or four bytes.

Three details a codec written from a description gets wrong, and which the
vectors catch:

- **A number is an integer only if it survives a 32-bit truncation round trip.**
  A value outside `i32`, a large timestamp or a byte count, goes out as a
  *double*, not an int64. So does `2.5`; `4.0` does not.
- **The weight is written negated.** A positive number in that slot is a
  different value to the server, not a different spelling.
- **A global is written only when its value changed**, except the per-event
  timestamp and the private-stats id, which are rewritten before every event.
  Staged globals and the timestamp go out together in ascending id order.

`TIMESTAMP_FIELD` (47) and `SEQUENCE_FIELD` (3433) are hand-written constants in
the catalog crate, not generated: the runtime writes them directly and no
`defineGlobal` declares them, so the IR has no source for either. Everything
else in the crate comes from the IR.

### Byte-exactness

`plugins/wam-catalog/src/tests.rs` asserts whole buffers against vectors produced
by WA Web's own `WABinary` and `WAWebWamLibProtocol` modules, loaded out of the
bundle set the pinned whatspec commit records and driven by
`plugins/wam-catalog/tools/wa-web-vectors.js`. Every size-class boundary is
covered: 0, 1, the i8/i16/i32 edges, the first value past `i32`, a non-integral
double, string lengths at 255/256/65535/65536, multi-byte UTF-8, and a field id
past 255.

What is *not* claimed is byte identity with a particular WA Web call: the
official writer emits an event's fields in the order the calling code assigned
them, and this encoder emits them in the order the catalog declares them. Each
record is identical; the order of the field records within one event is this
repository's, deterministically. The wire is id-tagged, so both are valid.

## Sampling

`weights` is `[alpha, beta, release]` and the client picks one **by gate, not by
position**: with no gate set it uses a literal `1`, one gate selects index 1 and
another selects index 2. The same two gates decide the `webcEnv` global, where
the second means `PROD`, so a shipping web client uses index 2, and index 0 is
unreachable from the definition at all. `sampling::RELEASE` is that index.

Weight 0 means *always keep*, not never: the official client skips the sampling
test entirely for it. Otherwise the event is kept with probability `1/weight`.

The catalog's weight is a default. The IR records four call sites that override
it at runtime, so a client that later reads `abprops` may find a different one;
nothing here does yet.

## Flush and upload

Four constants, all from the IR, and the pair that looks redundant is not:

| constant | value | what it bounds |
| --- | ---: | --- |
| `WAM_IN_MEMORY_BUFFERING_DURATION_IN_SECS` | 5 | how long events accumulate before being written |
| `WAM_BUFFER_ROTATE_INTERVAL_IN_SECS` | 120 | how long a small buffer may live before going out anyway |
| `WAM_MAX_BUFFER_SIZE` | 50000 | the size at which a buffer is finished rather than added to, and the cap on everything retained across a failure |
| `WAM_MAX_BUFFER_SIZE_FOR_UPLOAD` | 64000 | the ceiling on what the upload stanza may carry |

The size check runs *between* events, so one event can carry a buffer past the
first threshold; the second is the hard ceiling, and a buffer past it is dropped
rather than sent. That is the difference between them, and both are exercised by
tests.

A failed upload starts a cooldown (one second, doubling, capped at two minutes)
and the retry rides a later tick. The official client instead retries inline,
waiting between two attempts; here that wait would sit on the task that also
drains the queue, so a slow server would stop events being *written* as well as
sent. The spacing is the same curve without occupying anything.

A buffer the server *refused* is dropped rather than kept, which is the one
place this diverges from the official client deliberately. A 4xx is a refusal of
that buffer, so retaining it buys a retry that fails the same way every two
minutes until unrelated traffic pushes it past the retention cap. It is counted
the way a buffer past the upload ceiling is counted. A timeout or a lost
connection is not a refusal and is retained.

Shutdown gets one best-effort flush, inside the deadline the plugin host already
applies to every shutdown callback rather than a new one of its own. The host
drains the plugin's tasks first, so the flush loop has parked its buffers and
nothing races for them; an upload that does not finish inside the bound leaves
the buffer retained rather than delaying the teardown. A cancelled task drops its
future, so the loop may never reach its parking step; the flush then starts a
fresh writer, which loses the in-progress buffer and saves the queue, and the
queue is the larger of the two. It is the moment the
official client writes `WebWamForceFlush`, so this writes it too.

A refusal is classified by the XMPP error class rather than by the code alone,
since `wait` is the server's own word for "ask again": a `wait` of any code is
retried, and so are 408 and 429, which mean "not now" whether or not the server
attaches a class. Everything else in the 4xx range is this buffer being refused
and is dropped.

A retained buffer is removed from the store only once the server has answered.
Removing it first would lose it to a crash between the delete and the answer,
which is the one window a durable store exists to close, and a buffer left in
place is already counted against the retention cap.

Nothing on this path can reach the client. Telemetry that cannot be encoded,
stored or uploaded is dropped and counted, and the count is reported as the
`WamClientErrors` and `WamDroppedEvent` events the official client uses for the
same purpose.

## Persistence

Two things outlive a buffer: the per-channel sequence number and the buffers not
yet accepted. Both go through the `WamStore` trait, which the plugin owns.

There is no storage capability in the plugin host and this batch does not propose
one: a capability is a promise about every plugin, and one plugin needing a
key-value store is not that. `InMemoryWamStore` is the default; an embedder that
wants durability writes an impl against its own database and the plugin reports
which it got through `WamStats::store_is_durable`.

With the in-memory store a restart renumbers from 1, which is what a fresh
browser profile does, and loses whatever had not been delivered, which was
best-effort already.

## What each emitted event is derived from

Seven of 436. Each one names the unit it counts, and the observation point is
chosen to match that unit rather than to be convenient:

| event | derived from | the unit it counts |
| --- | --- | --- |
| `E2eMessageRecv` | `DecryptedPayload`, some `EncDecryptFailed` | one `<enc>` this client tried to decrypt |
| `MessageReceive` | `Messages` | one decrypted message |
| `ReceiptStanzaReceive` | `RawNode`, filtered to `<receipt>` | one inbound receipt stanza |
| `WebcSocketConnect` | `Connected` | one authenticated socket |
| `WebWamForceFlush` | plugin shutdown | one flush ahead of schedule |
| `WamClientErrors`, `WamDroppedEvent` | the runtime's own losses | one abandoned buffer or event |

Two of those pairings are the result of getting them wrong first, and both are
worth keeping in mind when adding an event:

**`ReceiptStanzaReceive` reads the stanza, not `Event::Receipt`.** That event is
not one per receipt stanza. The aggregated shape dispatches one for every
`<user>` it names, and a retry receipt is consumed by the retry pipeline without
dispatching one at all, so a per-stanza metric derived from it would report
several stanzas where one arrived and none where one did. `RawNode` is one event
per decoded stanza, which is the unit the metric's name claims. The cost is that
every inbound stanza crosses the plugin bus for a tag comparison.

**Not every `EncDecryptFailed` is an E2E failure.** `e2eSuccessful: false` claims
this client read this `<enc>`'s ciphertext and could not turn it into plaintext,
and two groups of reasons make that claim false in opposite directions.

Below the line is `EncDecryptFailureReason::decryption_was_attempted`, the core's
own name for it: those nodes were set aside before any ciphertext was read, and
the last of them is not even about the stanza, only about when it arrived.
Counting them moves a success rate with something that was never a decryption.
`UnsupportedEncType` crosses back the other way, because unlike the rest of that
group it is a fact about this `<enc>`'s own `type` attribute and WAM names
exactly it.

Above the line is `PlaintextUnusable`, where the decryption succeeded and
something after it could not use the bytes. When the padding was the usable part,
the same `<enc>` already produced a `DecryptedPayload` and was already counted as
a success, so a second metric would contradict the first. WAM has no member for
"decrypted but undecodable", so the honest report is none.

## What is not emitted, and why

The catalog carries 436 events. The plugin emits seven. The gap is the rule, and
it splits into five causes worth telling apart.

**Blocked on an outbound observation point (the largest group).** Eighteen
regular-channel events describe an outgoing message or its media:

`MessageSend` (95 fields), `MediaUpload2` (62), `StatusPost` (61), `E2eMessageSend`
(28), `ForwardSend` (27), `StatusReply` (24), `EphemeralSyncResponseSend` (20),
`StickerSend` (15), `StatusInteractionSent` (12), `EditMessageSend` (9),
`PinInChatMessageSend` (9), `NonMessagePeerDataMediaUpload` (9), `RevokeMessageSend`
(6), `SendDocument` (4), `WebcMessageSend` (4), `SendRevokeMessage` (3),
`DeepLinkMsgSent` (2), `GifFromProviderSent` (1).

`Event::SentFrame` publishes the marshaled bytes of a stanza after the write and
carries none of the send's own semantics: no message type, no per-device
encryption count, no stage timings. Re-deriving a 95-field event from it
would be reconstruction, not observation. `AndroidMessageSendPerf` and
`WebcMediaEditorSend` are excluded from the count: one is another platform's, the
other a UI event.

**Blocked on the inbound surface.** A second, smaller class, and the one worth
noticing: `MessageHighRetryCount` and `MdRetryFromUnknownDevice` describe things
this client already measures. `wacore/src/telemetry.rs` has a counter for each
and its doc comments name these very WAM ids. What is missing is an `Event`
carrying them, and a plugin sees only the event bus. `UnknownStanza` is the same
shape: the client has no "I dropped this stanza" signal a watcher can see, and
`stanza.intercept` runs *before* the pipeline, so it cannot tell a stanza the
client will handle from one it will not.

**Blocked on the `private` channel.** Fifty events needing a blind-signed token
(VOPRF over Ed25519), the `dit.whatsapp.net` endpoint, and persisted rotation of
an anonymous id. The catalog carries the nine rotation groups so a later batch
starts with them.

**Browser facts this client has no equivalent of.** `WebcPageResume`
(`webcResumeCount`) and `WebcStreamModeChange` (`webcStreamMode`) are the
lifecycle events left on the table. Both come from WA Web's stream model and both
write exactly one field: a count of page resumes and a stream mode. This client
has no page to resume and no stream mode of that vocabulary, so the field would
have to be guessed and the event is not emitted. `WebcSocketConnect` and
`WebWamForceFlush` are the two from the same family that *are* emitted, because
WA Web writes no field at either call site: a fieldless event is the whole event,
so there is nothing left to guess.

**Deliberately not implemented.** Beaconing: the official client gives one client
in a hundred a per-event sequence number, rolled once per UTC day and remembered.
The roll only means anything if it happens once per client per day, and a process
that restarts five times a day against a store that cannot remember would roll
five times and over-represent itself in a cohort built on the opposite
assumption. Doing it right needs a durable per-event counter; not doing it at all
is the honest answer to not having one.

## The cost that recurs

Turning the plugin on holds three lease-gated core events open for the life of
the client, and they are the only cost here that recurs per message rather than
once:

| event | dispatched | what the plugin does with it |
| --- | --- | --- |
| `DecryptedPayload` | per `<enc>` that decrypted | derives one `E2eMessageRecv` |
| `EncDecryptFailed` | per `<enc>` that did not | derives one `E2eMessageRecv`, or nothing |
| `RawNode` | per decoded inbound stanza | compares one tag, keeps `<receipt>` |

The payload is `Bytes`, so no plaintext is copied and the per-`<enc>` cost is
building and dispatching the event, not cloning it; `RawNode` hands over an `Arc`
clone. **None of this is measured.** There is no receive-side benchmark in this
repository (`benches/` covers group send), so the number would have to come from
a bench written for the purpose. It is stated as unmeasured rather than
estimated, because a per-message cost with no number and no note is the wrong
kind of silence.

## Globals

Almost every one of the 46 globals is a fact about a browser this library is not:
the window's memory class, the tab id, the CPU count the page can see. The
default identity writes five, and each traces to the pairing `ClientPayload` or
to a fact about this client:

| global | value | where it comes from |
| --- | --- | --- |
| `appVersion` | the announced WhatsApp build | `wacore::version::WA_WEB_VERSION_STR` |
| `platform` | the client family | `ClientProfile::user_agent_platform` |
| `osVersion` | the OS string | `ClientProfile::os_version` |
| `ocVersion` | 0 | this is not the official client |
| `appIsBetaRelease` | false | the payload announces the `RELEASE` channel |

A `browser` or `osVersion` that disagreed with what the pairing payload announced
would be worse than nothing, because the server has both, so the values that can
be derived are derived from `ClientProfile`, the same struct the payload is built
from. `deviceName` is deliberately not filled from `ClientProfile::device`, which
holds a device *class* ("Desktop") and not the OS name the official client puts
there.

A global carries the channels it may legally be written on, and `WamBuffer`
refuses one the buffer's channel does not allow rather than skipping it the way
WA Web does: skipping silently is how a caller ends up believing a value was
sent.

## Regenerating

The catalog and the parity table are two of the eleven artifacts
`cargo run -p whatspec-codegen` writes, and regeneration is all-or-nothing, so
they always describe the same WhatsApp build as `waproto` and `abprops`. See
`wa_web_reference.md`.

The parity table sits behind the catalog's `parity` feature: it is evidence about
WA Web, not part of the catalog a client links, so it never reaches a release
binary. The plugin enables it as a dev-dependency.
