# Android evidence and oracle tooling

`tools/mobile-oracle` is an unpublished host tool. It must stay outside
`default-members` and must never become a dependency of the published runtime.
The mobile protocol core remains independent of the runtime, filesystem,
network, APK tools, Android and this verifier. Host tooling is not a portability
qualification for whatsapp-rust-bridge, browser WASM or ESP32.

## Verify a local snapshot

```sh
cargo run -p mobile-oracle -- verify /capture/apk.lock.json /capture/artifacts
cargo run -p mobile-oracle -- diff /old/apk.lock.json /new/apk.lock.json
cargo test -p mobile-oracle
cargo clippy -p mobile-oracle --all-targets -- -D warnings
```

The command reads local files only. It does not fetch an APK, execute Android
code, contact WhatsApp, register a number or send SMS. Missing files, unknown
schema/fields, invalid paths, escaping symlinks, size-budget violations and
hash mismatches fail with a nonzero exit status. Successful output is a JSON
report with sorted artifact paths and the SHA-256 of the exact lock bytes.
Artifacts are streamed with a 16 KiB read buffer. The verifier caps a lock at
1 MiB, an artifact at 2 GiB, the declared set at 4 GiB and the count at 4096.

A successful verification means that the declared files have the expected bytes.
It does not establish APK authenticity, lawful acquisition, completeness of an
installation, the relation between extracted files and their APK, or protocol
conformance. Reports always state `authenticity: requires_independent_review`
and `protocol_execution: not_run`. Neither status can be promoted by editing a
lock. Synthetic tests are explicitly fabricated; no official APK is included.

Keep the snapshot directory stable and operator-controlled during verification.
The verifier rejects paths escaping the canonical root, but it is not a sandbox
against concurrent modification of the directory by another process.

## Lock format and provenance

Schema version 1 records one package/version/ABI and requires exactly one base
APK. List every split from that installation, any extracted DEX/native/resources
used by analysis, and the signature report. Include original base/splits even
when a test only exercises one extracted function. Do not combine artifacts
from different application versions, channels, device ABIs or installations.
The completeness flag records an operator assertion and requires review.

Example structure below uses placeholders. Replace every placeholder with
reviewed capture data and computed hashes before running verification.

```json
{
  "schema_version": 1,
  "package_name": "com.whatsapp",
  "version_name": "<exact version name>",
  "version_code": 1,
  "abi": "arm64-v8a",
  "min_sdk": 23,
  "target_sdk": 35,
  "distribution_channel": "<exact acquisition channel>",
  "acquisition": {
    "source_url": "https://example.invalid/replace-with-authorized-source",
    "acquired_at_utc": "2026-11-04T22:00:00Z",
    "access_conditions": "<authorized acquisition and storage conditions>"
  },
  "signature": {
    "signer_certificate_sha256": "<64 lowercase hex digits>",
    "trusted_signer_reference": "<independent signer reference and rotation evidence>",
    "verification_report_path": "signature.txt",
    "verifier_version": "<exact Android Build Tools version>"
  },
  "declared_installation_complete": true,
  "artifacts": [
    {
      "path": "base.apk",
      "kind": "base_apk",
      "size_bytes": 1,
      "sha256": "<64 lowercase hex digits>"
    },
    {
      "path": "signature.txt",
      "kind": "signature_report",
      "size_bytes": 1,
      "sha256": "<64 lowercase hex digits>"
    }
  ]
}
```

Other artifact kinds are `split_apk`, `dex`, `native_library` and `resources`.
Paths are relative to the supplied artifact root, use forward slashes and must
not contain traversal or URI/drive syntax. Acquisition URLs must be HTTPS and
contain no credentials, query or fragment. Do not put account information,
access tokens, OTPs, PINs, session files or signing secrets in any capture.

Before accepting an official snapshot, independently verify base/split
signatures with `apksigner verify --verbose --print-certs`, compare certificate
digests against a trusted reference and record rotations. The locked report
records this evidence; the Rust verifier does not invoke apksigner or validate
its output. An APK can have a valid signature from an untrusted signer.
Review package/version/ABI metadata with `apkanalyzer` and compare it with the
lock. The verifier currently validates declarations, not APK ZIP contents.

Preserve original bytes, extraction tool versions/options and hashes for every
output. Use JADX/baksmali as navigation aids, confirming relevant decisions in
DEX; use Ghidra or equivalent for JNI/ELF. Record the ELF build-id as additional
analysis metadata, never as a substitute for SHA-256. Avoid redistributing APKs
or decompiled proprietary code by default. Acquisition and storage permissions
need their own review for the actual channel and jurisdiction.

## Executable oracle qualification

The next qualification step is one pure function with synthetic inputs, not
an emulation claim for the entire Android application. Identify its artifact
hash, DEX class/method or native offset, callers, guards and dependencies.
Obfuscated names alone are insufficient selectors. Record selector anchors,
body fingerprints and tool versions together.

Run DEX in an isolated Android process when practical. JNI may require a real
Android image and matching ABI. A host-native loader, QEMU or unidbg does not
by itself reproduce ART, Keystore, hardware-backed attestation, push or Google
services. Reject unsupported dependencies rather than returning success-shaped
stub values. Record every import/provider reached and the exact synthetic
replacement, if the scenario permits one.

Each future scenario report must record input/output hashes, APK set identity,
runner and Android image versions, ABI, selector, clock/PRNG configuration,
imports/stubs reached and resource/deadline budgets. Distinguish `not_run`,
`unsupported`, `mismatch` and `pass`. A pass only qualifies the exercised
function/path with the stated substitutions. The current integrity command
cannot produce an execution pass.

Use fictitious identities, test-only keys and fabricated responses. Compare
identical inputs against official code and Rust, including bytes, omitted
versus present fields, validation errors and state effects. Fix clocks and
randomness only at offline test boundaries. Never use deterministic test
cryptography in live registration.

Do not patch pinning, bypass Play Integrity, forge certificates, execute
integrity scripts, use real accounts, send SMS or register devices without
separate explicit authorization. An offline oracle cannot establish server
acceptance, SMS delivery, account-specific remote configuration or hardware
attestation validity. APK Android evidence does not establish iOS behavior.

## Drift and mock integration

`diff` validates both lock declarations and reports old/new package, version,
SDK, ABI, channel and signer metadata, plus sorted added/removed/changed
artifacts. It does not open the artifacts or infer behavior from a changed hash.
Run `verify` separately for both snapshots. The command never rewrites locks
or goldens, and its execution status remains `not_run`.

Treat each changed APK/split hash as a new snapshot. Requalify selectors,
extraction assumptions and expectations together. Run a frozen synthetic corpus
against old/new official artifacts and against Rust, reporting separately
whether the client, extractor or runner changed. Do not automatically overwrite
golden expectations after a mismatch. Failure to resolve a selector is a
qualification failure, not a skipped success. Immutable snapshot storage can
be private or locally acquired; public Web bundle storage does not authorize
APK redistribution.

bartender/barback mobile E2E support is a future separate integration gate.
Pin its image/version and declared mobile capabilities, supply independent
endpoint/trust/provider configuration and use test-only credentials. No test
endpoint may silently fall back to production. Run mock E2E through the same
protocol transitions as other adapters; do not add mock-only protocol success
paths. Record mock coverage separately from APK differential qualification and
any separately authorized live observations.
