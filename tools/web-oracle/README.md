# Web reference captures from Rust

This nondefault workspace tool replaces the owned JavaScript WAM, retry metadata
and group normalization drivers. Corpus construction, mocks, capture and fixture
formatting are Rust. No JavaScript implementation is embedded in Rust strings.
The separately pinned original reference programs remain external JavaScript;
executing them is necessary for an independent oracle. None is copied into Git.
The product has no dependency on this crate or on its reference runtimes.

## Exact references

Follow [the bundle evidence workflow](../../agent_docs/wa_web_reference.md).
Use separate checkouts and input directories for these snapshots:

| Scenario | whatspec revision | WA version | Bundle set |
| --- | --- | --- | --- |
| WAM, groups | `ae3880925bacd23863cc30ec480c9f82bfd869f1` | `2.3000.1052559453` | `d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf` |
| Retry metadata | `eb16f595d455d8b0b3f7966a59444f4f9894c127` | `2.3000.1047483476` | `05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b` |

Restore the exact `bundles-<version>-<set>.tar.xz` from the `bundle-store` release
using that checkout's native restore command. The WAM archive SHA-256 is
`2958d99a3d84141fa8479f1f991e06246e68c063ca8f618a8d5d361287387f62`;
the retry archive SHA-256 is
`22243b9c2ff18a66cea8fd66551821fe0f9f09786d107e2be8284637db86c349`.
Keep restored bundles, captures and provenance in an ignored private directory.

WAM/retry verify the complete multiset of bundle hashes/sizes, including duplicate
multiplicity and the lock's own set fingerprint, before executing anything. The
verified bytes stay in memory until use. Extraction uses Oxc AST spans, rejects
parse errors and conflicting duplicate definitions, and preserves original bytes.
Boa 0.22 executes the selected module factories; the registry and narrowly scoped
legacy harness helpers are native Rust callbacks. Missing dependencies fail,
with no guessed empty mock. This is an isolated module execution with explicit
mocks, not an authenticated client or a server compatibility claim.

## Run and compare

Run these from the repository root; paths below name external verified inputs:

```sh
cargo run -p web-oracle -- wam /private/wam-bundles --lock /private/wam-whatspec/generated/bundles.lock.json --check tools/web-oracle/fixtures/wam-vectors.json
cargo run -p web-oracle -- retry /private/retry-bundles --lock /private/retry-whatspec/generated/bundles.lock.json --check wacore/tests/fixtures/retry_metadata.json
cargo run -p web-oracle -- group /private/wam-whatspec --check tools/whatspec-codegen/fixtures/group_normalization_oracle.json
```

Omit `--check` to print regenerated fixture data. WAM prints full hex buffers;
its committed check fixture stores lengths and SHA-256 values for all 31 vectors,
including large strings, from the previous independently executed Node driver.
`--modules-out /private/modules.js` optionally exports only the original AST-delimited
module calls for provenance or a separate engine comparison.

Retry preserves all 47 cases, original protobuf validation/unwrapping and metadata
construction. Its Rust mocks disable the same bot/origin metadata, poll snapshot
and diagnostic nesting gates as the retired driver. Synthetic media records do
not execute upload, view-once consumption, retry eligibility or registration.
Unexpected exceptions fail rather than being classified as invalid protobufs.

Groups requires Node with inspector support (qualified with Node 26.5.0 on Linux).
It verifies the upstream harness, evidence and both contracts, then executes that
unchanged harness. Its own source/module hash checks and trailing assertions still
run. Rust sets a breakpoint at its pinned capture boundary and reads ordinary
properties over CDP to obtain `groupDatas`; it never evaluates an expression,
injects a script or patches the source. All 204 payloads, nulls, inherited-value
cases, ordered reads and warning operations remain in the byte-identical compact
fixture. Warning-failure propagation is also checked by the upstream harness.
The inspector listens only on an ephemeral loopback port. Child cleanup runs on
errors. Use an external deadline and network isolation for reference execution.

## Tests

`cargo test -p web-oracle` checks rejection of changed/missing/extra bundles,
wrong lock fingerprints and undeclared dependencies. To run the original
reference tests explicitly, set absolute paths in `WEB_GROUP_WHATSPEC`,
`WEB_WAM_BUNDLES`, `WEB_WAM_LOCK`, `WEB_RETRY_BUNDLES`, `WEB_RETRY_LOCK`, then run:

```sh
cargo test -p web-oracle --test original_references -- --ignored --test-threads=1
cargo clippy -p web-oracle --all-targets -- -D warnings
```

The ordinary tests do not download client code. Original-reference tests fail on
missing evidence rather than silently succeeding or skipping.
