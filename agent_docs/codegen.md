# Generated code and build choices

Run `cargo run -p whatspec-codegen -- --check` to verify committed output.
Regenerate all whatspec-derived files together with `cargo run -p whatspec-codegen`.
The pin and emitters live under `tools/whatspec-codegen`; inspect the current
manifest and emitter selection instead of copying catalog counts into guides.
Never hand-edit generated Rust, token dictionaries, protobufs or descriptors.

The private IQ pilots consume only SetSubject and AcceptGroupAdd requests and
ordered success payloads. The consumer emitter derives wire names, argument
bindings and unique-child gates from IR and rejects unsupported request shapes,
new success payloads or changed envelope guards. Public specs supply named,
borrowed inputs so an argument rename requires an adapter update, and map the
private outcomes. Keep legacy group/community result
extensions in the handwritten adapter.

Community creation consumes a separate named-subject, parent-only projection of
the whatspec Create contract. Wire tags, attributes, literals, child ordering
and presence argument bindings come from the IR. The adapter maps `closed` to
an optional unit value because upstream `optionalMerge` tests nullability,
including false as present. Ordinary group creation retains its existing builder.
Description configuration and public partial-success errors stay in the adapter.

Contextual IQ contracts use a separate immutable emitter/input projection pin in
`tools/whatspec-codegen/iq-contextual.lock.json`. This avoids repinning unrelated
catalogs while an IQ extraction change is under review. Refresh the selected
projection from that revision, update its full-source and projection digests,
and keep both tooling dependencies on the same revision. Verify against its
checkout with `--verify-iq-contextual PATH/TO/generated` during normal codegen.
The generator still regenerates every committed artifact together.

`IqSpec::uses_contextual_response` opts into the actual sent stanza after final
ID assignment. `handles_iq_errors` independently opts into protocol error nodes.
`Client::execute` keeps request IDs, destinations, target and child references
from the request, including direct-encoded bytes, and passes an immutable node.
Generated static `parse_sent_response` entry points let public adapters retain
their inputs and result/error APIs without reconstructing a generated request.
Legacy response-only parsers remain available; streaming contextual contracts
fail explicitly until a context-aware streaming interface exists. Timeout,
disconnect and cancellation handling remains in the runtime.

Selection is bounded to the modules listed in the contextual emitter. Admission
refuses unresolved request inputs and unsupported guards/outcomes; it is not a
claim that every recovered stanza or every field has independent qualification.
Request repetition and disjunctions stay outside this checked emitter until a
fallible builder can enforce them. No JavaScript runtime is linked into core.

Locally persisted protobuf fields belong in `LOCAL_FIELDS` in `waproto/build.rs`;
retained whole messages belong in the proto emitter's `LOCAL_BLOCKS`. Unlisted
app-state schemas and stale props use their hand-written sibling files. Select
wire enums by the owning module, not just an identical variant set. IQ targets
are generated only where the namespace does not already imply the target.
Incoming wire tags include notification, server-request and stanza domains.

`waproto` uses buffa/buffa-build. Use `cargo xt proto-desc`, `tables-desc` and
`wire-desc` for descriptors and hash sidecars; `cargo xt --help` lists tooling.
Host tooling must not become a published runtime dependency. Task implementations
belong in Rust, not new Bash/Python scripts.

Feature changes must preserve core-only and no-default consumer graphs, native
MSRV and WASM. Keep Tokio/SQLite dependencies target-gated where required.
`signal` enables OS signal handling, not the Signal protocol.

`.cargo/config.toml` applies only to this checkout; downstream applications do
not inherit it. Do not make CPU-specific flags a library default. Compare linked
binaries under identical compiler/profile/allocator choices and verify portable
fallbacks. A deployment-specific `target-feature` recommendation belongs in the
consumer docs and requires a declared hardware floor. Changing profile overrides
must retain optimization on per-message crypto/protocol code and pass the linked
size gate. See `Cargo.toml` and `.github/workflows/binary-size.yml`.
