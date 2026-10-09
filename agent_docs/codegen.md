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
