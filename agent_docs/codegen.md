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

The reviewed wallpaper migration is the exceptional same-number, different-wire
case: `build_support/wallpaper.rs` retains historical string fields alongside the
upstream fixed32 ARGB fields. Its descriptor and AST guards require the reviewed
shape, and both owned and view decoders dispatch on number and wire type. Keep
the frozen API snapshot unchanged; do not generalize this exception to unrelated
field collisions.

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

App-state protobuf validation uses a separate reviewed pin in
`tools/whatspec-codegen/fixtures/appstate-validation/lock.json`. This keeps the
other domains on their existing commit while the new upstream format is reviewed.
Regenerate with `cargo run -p whatspec-codegen --bin appstate-validation`; add
`--check` to verify the vendored IR hashes and all three generated Rust files.
Update this pin and its immutable JSON inputs together, after the upstream PR.
The normal all-domain generator continues to own the existing action registry.

The generated boundary covers six protobuf methods and the decoded SET method.
The batch recipe preserves separate SET/REMOVE identities and exposes diagnostic
hooks for Patch/Snapshot/Local. Its conformance fixtures qualify the ordered
methods against the official Web snapshot; protobuf decoding is stubbed in that
oracle and native adapter tests exercise the real buffa decoder separately.
`wacore::appstate_sync` validates wire protobufs before downloading or mutating
collection state. Crypto, persistence, recovery, and the existing processor's
post-decryption batch integration remain handwritten. The public batch/SET APIs
consume the original decrypted index string and binary payload; the existing
`Mutation` API does not retain those inputs and is preserved.

Runtime collection ingestion moves owned inputs through `prepare_patch_list` on
blocking workers. Query keys through the prepared list, then pass that token to
`process_prepared_patch_list` or `process_one_prepared_patch_list`; this preserves
validation across the key wait without repeating snapshot scans. Runtime CDN
prefetch also checks metadata before admitting jobs or reserving download budget.
Missing snapshot MAC/key guards retain `SnapshotMACMissing` as recovery context
while preserving the generated validation error for downcasting.

The borrowed `missing_key_ids_after_inline` compatibility probe validates original
metadata before inlining, so conflicting sources cannot disappear. It retains
synchronous validation/decoding; owned runtime callers use the prepared API above.

Group normalization consumes separately pinned whatspec envelope, LINK and UNLINK
contracts, with checked digests. The normal generator checks them against the
main IR pin and emits static Rust guards, ordered projections and decision tables.
Independent original-JS oracle fixtures qualify the handwritten owned CREATE and
other action parsers as well. Update fixtures, digests, revisions and oracles
together; regenerate and run the group conformance tests.

For the qualified 2.3000.1052559453 capture, ordinary unknown LINK/UNLINK types
produce a null result and ordered warning operations. The additive
`*_with_warnings` parser APIs accept a `GroupWarningSink`; the existing pure
parser entry points discard warnings. LINK returns before payload reads, while
UNLINK validates its payload before warning. The complete notification parser
omits null actions. Historical fixtures remain in their versioned directory.

Runtime adoption requires the nondefault `generated-group-notifications` feature
and a `GroupNotificationFactsProvider` installed on the client. The provider
supplies effective flags and country lookup; device identities come from the
cached device snapshot. Without a provider the existing adapter remains active.
Normalized parsers own typed data; the compatibility adapter preserves the public
event API. Cache, rotation, ACK and storage effects remain in their existing
handlers. The tests use explicit account/Wid/telemetry mocks, with no authenticated
session. Numeric values outside the supported safe-integer or legacy field range
are rejected explicitly. See the pinned whatspec conformance evidence for the
exact preserved bundle snapshot and source spans.
