# Offline mobile observation comparison

Use this host-only command to compare supplied observations from a future APK
runner and a Rust implementation. It does not execute either implementation.
The [APK qualification guide](mobile_oracle.md) remains the prerequisite for
treating an artifact or runner as official evidence.

```console
cargo run --locked -p mobile-oracle -- compare /capture/apk.lock.json /capture/artifacts /capture/corpus.json /capture/apk-observations.json /capture/rust-observations.json
```

The five paths are the evidence lock, artifact root, corpus, APK observations
and Rust observations. Inputs, profiles and outputs use safe paths relative to
the artifact root. Keep that directory stable and operator-controlled throughout
comparison; path checks do not protect against concurrent file modification.

The command verifies locked artifacts and selectors before comparing observations.
Every input, profile and completed output must exist with its declared size and
SHA-256. Completed outputs are also compared byte for byte. Missing, extra,
duplicate or stale cases and incompatible contexts fail validation. There is no
implicit golden update, runner invocation, production request or APK download.

## Versioned documents

All three JSON documents require `schema_version: 1` and reject unknown fields.
SHA-256 values are 64 lowercase hexadecimal digits. A blob is an object with
`path`, `size_bytes` and `sha256`. A tool identity has `name`, `version` and
`artifact_sha256`.

| Document | Required fields |
| --- | --- |
| Corpus | `schema_version`, `synthetic_inputs: true`, `lock_sha256`, `context`, `profiles`, `cases` |
| APK observations | `schema_version`, `role: "apk_reference"`, `lock_sha256`, `corpus_sha256`, `context`, `provenance`, `cases` |
| Rust observations | Same observation fields, with `role: "rust_candidate"` |

The lock and corpus hashes identify their exact file bytes. Changing JSON
formatting changes that identity and requires new matching observations. Case
order is not execution order, but each document must contain the complete same
set of case IDs. Empty corpora cannot pass.

Each corpus case contains `id`, `selector_id` and an `input` blob. The selector
must exist in the verified evidence lock. Each observation case contains the
same `id`, `input_sha256`, `selector_sha256` and a `result`. A producer cannot
declare a pass. The only input result forms are:

```json
{"status": "completed", "output": {"path": "outputs/example.bin", "size_bytes": 1, "sha256": "<output SHA-256>"}}
```

```json
{"status": "not_run", "reason": "<bounded nonempty reason>"}
```

```json
{"status": "unsupported", "reason": "<bounded nonempty reason>"}
```

These are field illustrations with placeholders, not executable fixtures. The
complete fabricated fixtures in `tools/mobile-oracle/tests/observations.rs`
exercise the format without an official APK or runner. A result blob can encode
bytes, errors or state effects using the corpus's pinned observation format;
the comparator does not infer semantic equivalence between different encodings.

## Context and provenance

The shared `context` contains `reference_abi`, `clock_profile_sha256`,
`randomness_profile_sha256`, `providers_profile_sha256` and
`observation_schema_sha256`. Corpus `profiles` contains four corresponding
blobs named `clock`, `randomness`, `providers` and `observation_schema`. Their
actual nonempty bytes must match the context hashes. Profile content is opaque
to this tool; byte verification does not prove that a runner applied it.

Both observations must match that context. The reference ABI must match the
APK evidence lock. Provenance records separate `runner`, `image` and
`implementation` tool identities plus an `execution_target`. The APK target
must equal the reference ABI and its implementation hash must equal the locked
base APK. Rust may record a different actual target, such as a native or WASM
target. Matching inputs and deterministic profiles make that declared comparison
explicit; they do not qualify target execution.

Runner, image and implementation metadata remain operator declarations. Their
hashes are not authenticated or fetched here. Use synthetic identities, keys and
provider responses. Never put real PINs, OTPs, push tokens or account material in
the corpus, outputs, reasons or provenance.

## Results and budgets

The JSON report derives per-case and aggregate `pass`, `mismatch`, `unsupported`
or `not_run`. `pass` means agreement of the supplied observations only. Reports
retain `qualified_execution: false`, `protocol_execution: "not_run"`,
`authenticity: "requires_independent_review"` and
`execution_evidence: "operator_supplied_not_authenticated"`. Profile integrity is
reported as `verified_bytes_not_application`.

Exit status is zero only when every case compares successfully. A valid report
with mismatch, unsupported or unexecuted cases exits with status 2. Malformed
documents, missing evidence or integrity failures produce a nonzero error.
CI must retain the command's exit status rather than treating JSON output as
evidence of successful execution.

Each document is bounded at 4 MiB, the corpus at 1 to 1024 cases, each blob
at 16 MiB and each profile at 64 KiB. A 64 MiB aggregate read budget includes
repeated blob references and the additional output comparison pass. Evidence
lock/artifact verification has its separate budgets in the APK guide.

Store raw artifacts and supplied observations under approved access conditions.
Do not promote these reports into official execution evidence without separately
qualifying the runner, selected path, imports, stubs and actual applied profiles.
