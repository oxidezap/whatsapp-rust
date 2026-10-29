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

This boundary does not replace the full RPC parser: `IqSpec::parse_response`
has no actual request ID, and the runtime converts errors before calling it.
Keep those paths unchanged; never synthesize correlation context from response
attributes. Adopting upstream's complete contextual parser needs a separately
reviewed integration of the actual sent ID/target and error handling.

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
