# Checking an implementation against real WhatsApp Web

Before adding or changing protocol logic, check it against what the official client actually does. There are two sources, and the cheap one comes first.

| Source | Use it for |
| --- | --- |
| **whatspec IR** — `generated/*/index.json` | Structured, queryable answers: does this stanza exist, what attributes does it carry, what is this enum's wire value, what is this limit |
| **Raw bundle** — `docs/captured-js/` | Everything the extractor doesn't model: control flow, ordering, retry policy, when a request is sent at all |

## whatspec

[`oxidezap/whatspec`](https://github.com/oxidezap/whatspec) parses the WhatsApp Web JS bundle with an `oxc` AST and emits a language-neutral IR: IQ stanzas, protobuf schemas, GraphQL persisted operations, app-state actions, feature flags, wire enums, notification dispatch, binary-protocol token dictionaries. The IR is a derived model of the contract — a static reading of minified code, not the contract itself — and the committed Rust modules are one consumer of that model. Treat it as high-quality evidence, not as a specification; the limits are spelled out below.

This repo already vendors parts of it — `wacore/src/iq/mex_operations.rs` is copied verbatim from `generated/mex/operations.rs`, and the protobuf and app-state work came from the same place. Refreshing a vendored file is a `cp`.

```sh
git clone https://github.com/oxidezap/whatspec
cd whatspec

./scripts/regen.sh                      # offline: rebuild generated/ from pinned bundles and verify it
cargo run --release -p whatspec -- update    # fetch the current bundle and regenerate
cargo run --release -p whatspec -- diff old-generated/ generated/   # what a WA version bump changed
```

`generated/` is committed, so **reading the IR needs no build at all** — clone and query the JSON. `generated/manifest.json` stamps the WhatsApp version, per-domain counts, content hashes, and extraction diagnostics.

### Domains

| Domain | Answers |
| --- | --- |
| `iq` | Every `<iq>` request builder and response parser, per namespace |
| `stanza`, `srvreq`, `incoming` | Non-IQ outgoing stanzas, server requests, incoming stanza shapes |
| `notif` | `<notification type="…">` kinds, their handlers, and typed content |
| `proto` | `WAProto.proto` — diff against `waproto/` |
| `mex` | Relay/GraphQL persisted operations: doc id, kind, typed variables and response |
| `appstate` | App-state (syncd) action schemas and their indexing |
| `abprops` | ~2400 A/B feature flags: name, code, type, default. Where protocol limits actually live |
| `enums` | Wire-enum catalog — nack codes, chat and receipt types |
| `tokens` | Binary-protocol token dictionaries (single-byte + double-byte) |
| `wam` | Client telemetry event schemas |

Each has a JSON Schema under `generated/schema/`.

### Queries that work

Run these from the checkout's `generated/` directory:

```sh

# Which group requests does WA Web build?
jq -r '.stanzas[] | select(.namespace=="w:g2") | .exportedFunction' iq/index.json

# The exact request shape — tags, attrs, which are required, which are constants
jq '.stanzas[] | select(.exportedFunction=="makeGetLinkedGroupRequest") | .request' iq/index.json

# What the official parser reads out of the response, with wire names and types
jq '.stanzas[] | select(.exportedFunction=="makeGetLinkedGroupRequest") | .response.fields' iq/index.json

# A wire enum's real values
jq '.enums[] | select(.name=="…") | .variants' enums/index.json

# A protocol limit, straight from the flag registry
jq -r '.configs[] | select(.name|test("group.*(size|subject|description)";"i")) | "\(.name) = \(.default)"' abprops/index.json

# Notification types and their handlers
jq -r '.notifications[] | "\(.type) -> \(.handlerFunction)"' notif/index.json
```

### A worked check

`wacore/src/iq/groups.rs` declares `GROUP_SUBJECT_MAX_LENGTH = 100`, `GROUP_DESCRIPTION_MAX_LENGTH = 2048`, `GROUP_SIZE_LIMIT = 257`. The registry gives `group_max_subject = 100`, `group_description_length = 2048`, `group_size_limit = 257`. Three for three — the constants are confirmed, not folklore, and the next person to doubt them has a one-line command instead of an argument.

That is the shape of a good check: name the value in our code, name the field in the IR, report the comparison. A mismatch is a finding either way — our bug, or a WhatsApp change worth a PR note.

## Where the IR is not the truth

The extractor is a static analysis of minified code, and it says so in the manifest — `diagnostics` carries per-domain `unparseable`, `degradedResponses`, and drop reasons. Check them before concluding a stanza doesn't exist.

Known sharp edges:

- **Generated `Response` types are heuristic mirrors, not safe deserialize targets.** They carry no enums and type some string fields as numbers, so `serde_json::from_value` breaks on real payloads. Take the typed *input* (`Variables`); parse the output in a domain layer. This is why `NewsletterMetadata` and friends parse `data: serde_json::Value` by hand.
- **One-of objects flatten to `String`.** `update_group_property`'s `update` field is the known case; `GroupPropertyUpdate` in `src/features/groups.rs` corrects it and a wire-shape test locks it.
- **`waVersion` moves.** A doc id or default that matched last month may not match today; `whatspec diff` is how you tell a real change from a stale memory.
- **The IR describes shape, never sequencing.** Whether a request is sent eagerly, retried, debounced, or gated on a flag is control flow — that lives in the bundle.

## Falling back to the bundle

`docs/captured-js/` is the raw dump (untracked, local). Navigation and grep patterns are in `feature_implementation.md`. Go there when the IR is silent, degraded, or when the question is *when* rather than *what*.

Either way, reading gives you a hypothesis, not a fact. Validating one against a real capture — and keeping that capture's contents out of tests, commits, and PR bodies — is covered under "Reading evidence honestly" in `feature_implementation.md`.
