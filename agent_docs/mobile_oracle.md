# Android evidence and oracle tooling

`tools/mobile-oracle` is an unpublished host tool. It must stay outside
`default-members` and must never become a dependency of the published runtime.
The mobile protocol core remains independent of the runtime, filesystem,
network, APK tools, Android and this verifier. Host tooling is not a portability
qualification for whatsapp-rust-bridge, browser WASM or ESP32.

## Acquire the first snapshot without running the application

The official consumer Android download entry is
[WhatsApp for Android](https://www.whatsapp.com/download/android). The older
[Android URL](https://www.whatsapp.com/android) redirects there. Its "Download
WhatsApp directly" link points to an APK on `scontent.whatsapp.net`; that CDN
address may include an expiring signed query. Treat the landing page as the
source reference and the downloaded bytes/hash as the snapshot identity. Do
not paste a signed CDN query into a lock or assume the current URL pins a
historical version. This source does not establish Business or iOS coverage.

The official `https://www.whatsapp.com/android/current/WhatsApp.apk` endpoint
is another acquisition location. It is mutable, so record the final bytes,
not an assumption that `current` will restore the same release later.
For independent signer review, fetch
`https://www.whatsapp.com/.well-known/assetlinks.json` separately and compare
the `com.whatsapp` certificate SHA-256 fingerprints with apksigner's output.
Preserve the reference hash and acquisition time privately. Check every
SDK-specific signer and rotation result; do not assume a single signer applies
to every Android version. The published association corroborates certificate
ownership, while apksigner validates the acquired APK signature.

For an operator handoff, download the original APK through that official
button and make the file available in the workspace through an approved
file-transfer path. No account, installation, execution or
local SDK setup is needed for that handoff. Do not copy an expiring CDN query into shared logs or the lock;
record the landing-page URL. The verification steps below can be performed on
the received file. Do not re-sign or modify it.

The capture qualification path is:

1. On a machine permitted to access the official page/CDN, download through
   that page into a fresh capture directory. Preserve the original file.
   Downloading is enough for static work; do not install, launch or register.
2. With a pinned Android SDK Build Tools installation, run
   `apksigner verify --verbose --print-certs base.apk`, retain its output and
   require a successful exit status. Independently review the signer digest
   and rotation evidence against a trusted reference. Signature validity alone
   is not an authenticity decision.
3. Use the pinned SDK's `apkanalyzer apk summary base.apk` and manifest commands
   to obtain package/version/SDK metadata. Inspect the contained native
   libraries to select the intended ABI. If acquiring an already installed
   authorized package instead, preserve its complete base/split set; do not
   infer completeness from one exported APK.
4. Compute file hashes/sizes, fill the lock with acquisition conditions and
   signer evidence, then run the local verifier. Keep raw artifacts in approved
   local/private storage; do not publish the APK or decompiled code in public
   repositories or releases by default.

If the selected executor's network policy does not allow the official
origin/CDN, request the supported policy change or bring in an
operator-acquired capture. Do not unset proxies, switch
to an unofficial mirror or otherwise route around the policy. There is no need
to contact registration endpoints, send SMS or use an account for these steps.

Tool references: [apksigner](https://developer.android.com/tools/apksigner) and
[apkanalyzer](https://developer.android.com/tools/apkanalyzer).

## Reproduce acquisition by content identity

Use `cargo xt mobile-apk --help` for host-only acquisition tasks. An acquisition
lock pins the exact file SHA-256 and byte length, with a declared version label
and the stable official source page. Commit the reviewed lock, not the APK or
the temporary CDN URL. Pinning a local file is an explicit operation; fetching
or importing against an existing lock must never update its expected hash.

The version label is operator metadata. It does not prove the APK's manifest
version, package, signer or authenticity. The acquisition lock is separate from
the qualified evidence lock below, which also records independently reviewed
signature and analysis evidence.

Restore from the content-addressed local cache or an approved durable private
copy, or supply a current HTTPS download location through the command's named
environment variable. Every path must check the same locked hash and size.
A rotated URL is acceptable only if it delivers those exact bytes. A new APK
requires a new reviewed lock; do not fall back to the latest release.

This follows whatspec's separation of content identity from fetch location.
The official CDN does not promise historical availability. A hash pins bytes
but cannot make an expired URL work or recover bytes deleted from storage.
Preserve a permitted private copy for long-term restoration. Do not create
public APK release assets automatically, infer redistribution permission from
whatspec's Web bundle store, or publish signed download queries.

```sh
# Create a new acquisition lock after reviewing the source and version label.
cargo xt mobile-apk pin --input /capture/WhatsApp.apk --version VERSION --lock /capture/download.lock.json
# Import the exact locked bytes, including when network access is unavailable.
cargo xt mobile-apk import --lock /capture/download.lock.json --input /capture/WhatsApp.apk
cargo xt mobile-apk verify --lock /capture/download.lock.json
# Set WHATSAPP_APK_URL privately to a current, permitted direct HTTPS location.
cargo xt mobile-apk fetch --lock /capture/download.lock.json
```

The default cache is `target/mobile-apk/<sha256>/WhatsApp.apk`; `--cache` selects
another operator-controlled cache root. Preserve durable copies outside disposable
build directories. `--url-env` selects a different environment variable name.
Do not paste the signed URL into tracked scripts, shell history or CI output.
Redirects are rejected; supply the direct final location. Normal proxy and TLS
validation remain enabled. Configure a trusted PEM bundle through `SSL_CERT_FILE`
when the permitted proxy requires it; do not disable certificate verification.
Neither download nor import runs or installs the APK.

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
installation or protocol conformance. Raw archive-member verification can establish
the byte relation described below; transformed outputs retain declared provenance.
Reports always state `authenticity: requires_independent_review`
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
contain no whitespace, controls, credentials, query or fragment. Do not put account information,
access tokens, OTPs, PINs, session files or signing secrets in any capture.

Before accepting an official snapshot, independently verify base/split
signatures with `apksigner verify --verbose --print-certs`, compare certificate
digests against a trusted reference and record rotations. The locked report
records this evidence; the Rust verifier does not invoke apksigner or validate
its output. An APK can have a valid signature from an untrusted signer.
Review package/version/ABI metadata with `apkanalyzer` and compare it with the
lock. Acquisition metadata remains declared; opt-in raw ZIP-member checks do not
parse or authenticate the Android manifest or signing block.

Preserve original bytes, extraction tool versions/options and hashes for every
output. Use JADX/baksmali as navigation aids, confirming relevant decisions in
DEX; use Ghidra or equivalent for JNI/ELF. Record the ELF build-id as additional
analysis metadata, never as a substitute for SHA-256. Avoid redistributing APKs
or decompiled proprietary code by default. Acquisition and storage permissions
need their own review for the actual channel and jurisdiction.

## Extraction provenance and byte-range selectors

A base/split-only lock can omit `extractions` and `selectors`. Any declared
DEX, native-library or resource artifact must carry one extraction record
referencing its locked source APK, the same source hash, the original archive
member and the extractor name/version/distribution hash/arguments. The default
`operator_declared` mode records provenance without proving that a tool ran or
that its output follows from the source entry. Use it for transformed outputs.
Record command arguments without credentials or session data.

Set `mode` to `archive_member` only for an exact decoded copy of a ZIP member.
The verifier reads that member from the locked APK and compares its streamed
size and SHA-256 with the locked derived artifact. Successful reports distinguish
`archive_member_verified` from `operator_declared`. No files are unpacked and no
extractor or APK code runs. This establishes raw byte ancestry, not tool
execution, manifest authenticity, signature validity or behavioral equivalence.

The raw-member path supports stored and deflated members in classic,
single-disk ZIP archives, including the gap used by an APK signing block.
ZIP64, multi-disk, ambiguous directory records and duplicate member names fail
explicitly. Selected members must be unencrypted and use a supported compression
method. The preflight bounds central
and aggregate local metadata at 16 MiB each, with at most 65534 entries before
invoking the ZIP parser. Decoded names have a separate 64 MiB budget. Selected
compressed bytes and decoded output have separate aggregate budgets of 4 GiB.
This is a deliberately bounded archive profile, not qualification of every APK
distribution format. Preserve original base/splits and report unsupported inputs.

Selectors can only reference locked derived artifacts. Each selector requires
a unique ID, file offset, nonempty byte length, SHA-256 and an explanation of
the call-path/guard analysis. The verifier checks the actual selected bytes
with bounded streaming after verifying the complete artifact. Offsets are
file offsets, never runtime addresses or function indices. A fingerprint
match establishes byte identity only; it does not establish that the range is
a function, that a guard is active or that any behavior was executed.

Example optional records, with placeholders requiring real reviewed values:

```json
{
  "extractions": [
    {
      "output_path": "classes.dex",
      "input_apk_path": "base.apk",
      "input_apk_sha256": "<same hash as locked base.apk>",
      "archive_member": "classes.dex",
      "mode": "archive_member",
      "tool": {
        "name": "<extractor>",
        "version": "<exact version>",
        "artifact_sha256": "<tool distribution SHA-256>",
        "arguments": ["<exact non-secret arguments>"]
      }
    }
  ],
  "selectors": [
    {
      "id": "<analysis-local stable ID>",
      "artifact_path": "classes.dex",
      "offset_bytes": 1,
      "size_bytes": 1,
      "sha256": "<selected byte-range SHA-256>",
      "rationale": "<callers, guards, locator and why this exact range matters>"
    }
  ]
}
```

Add `classes.dex` to `artifacts` with kind `dex` and its own complete file
hash/size. The examples do not identify any real WhatsApp function. Missing
lineage, a stale source hash, missing/invalid tool metadata, unsafe archive
members, unknown selector artifacts, overflow/out-of-bounds ranges and stale
range hashes fail. There are at most 4096 extraction and 4096 selector
records, with an additional aggregate selector-read budget of 4 GiB.

`diff` reports selector changes and extraction-provenance changes separately
from whole-artifact changes. A tool or analysis change is visible even when
output bytes did not change. Reordering records is not semantic drift. The
command does not requalify selectors, execute code or update expectations.

## Executable oracle qualification

The [offline observation comparison contract](mobile_observations.md) defines
strict corpus and runner-output inputs for `mobile-oracle compare`. It can check
supplied byte agreement before a runner is available, while keeping execution
and authenticity explicitly unqualified.

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
