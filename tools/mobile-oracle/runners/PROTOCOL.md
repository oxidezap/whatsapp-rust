# Superpack extraction and Curve25519 JNI qualification

These probes continue the [isolated Android workflow](ANDROID.md). They execute
original APK native code in a disposable AOSP runtime, with synthetic crypto
inputs and no application installation, account, session or registration. Keep
all proprietary artifacts, detailed inventories and reports in the private cache.
Do not publish them with the runner source.

## Bounded original decompression

`AndroidDecompressProbe` takes a hash-pinned Superpack loader, a separately
hash-pinned native container and a fresh private guest output directory. It calls
the original `AssetDecompressor.decompress_range_from_so` for start index 0 and count 18,
using the separately reviewed primary-library profile. It requires exactly 18
native results and output files. This is a snapshot-specific range, not a rule
for future versions. Review the original caller, its primary-library set and
ordering before using another APK.

The driver requires an Android runtime, the original APK on its classpath, the
expected classloader, matching input hashes and a fresh canonical output under
`/data/local/tmp/mobile-oracle/`. It inventories ordinary output files and their
hashes after execution. The private guest partition and external deadline are
also required; post-execution size checks are not a sandbox against arbitrary
native code. Outputs are capped at 64 MiB per file and 512 MiB in aggregate.
The exercised first range is much smaller. Never reuse a populated output path
or silently skip a failed extraction.

Reproduce in two fresh directories and compare their complete private inventories.
Preserve both loader and container, the original APK/raw-member ancestry, runner
DEX, image identity, native call arguments, exit status and output hashes.
A decompressed library is a transformed output, not a raw ZIP member. Record its
extraction as `operator_declared`, identifying the locked container member and
original native decompressor. The current evidence verifier can check its bytes
and raw-container ancestry, but cannot cryptographically authenticate that a
native transformation ran. Do not relabel it `archive_member_verified`.

## Original Curve25519 provider

After successful extraction and independent inventory review,
`AndroidCurveProbe` loads the exact pinned curve library. It loads the original
`NativeCurve25519Provider` class from the original APK, retains its constructor
and checks the invoked methods are native. The reviewed constructor creates a
random-provider object and performs the original native smoke check. The probe
uses only explicitly supplied seeds/randomness, never the no-argument private-key
generator or `getRandom`.

Synthetic scenarios are:

- Public-key derivation and bidirectional agreement using the public
  [RFC 7748 section 6.1](https://www.rfc-editor.org/rfc/rfc7748#section-6.1) vectors.
- A signature of the ASCII message `mobile oracle synthetic`, using the RFC
  scalar and 64 explicitly zero random bytes. Native verification must accept
  the original message and reject a changed message.
- Torsion-free status of the normal public key, recorded as an observation.
- Agreement with an all-zero 32-byte public input, recording the actual return
  or exception. It is not automatically converted into a passing result.

The opt-in Rust test compares public keys and agreement, verifies the original
native signature, and requires byte-identical signature output for the same
explicit randomness. It separately checks tampering and low-order rejection.
If the JNI returns instead of rejecting the low-order input, or a deterministic
signature differs, the comparison fails visibly. Investigate the difference;
do not overwrite an expected value or relax the comparison to manufacture parity.

Native entry points need ELF file offsets, not virtual addresses. Check section
or load-segment mappings before creating selectors; these need not be equal.
Pin both the complete extracted library and reviewed entry ranges. Native helpers
can call internal routines outside an entry range, so an entry fingerprint alone
is not complete dependency qualification. Inspect the actual DT_NEEDED set and
reject unsupported dependencies instead of installing success-shaped stubs.

## Build and run

Use the pinned tools and network-isolated emulator/ADB described in `ANDROID.md`.
Compile all three source files together from the repository root:

```sh
mkdir -p .cache/mobile-oracle/android/protocol-classes .cache/mobile-oracle/android/protocol-dex
javac --release 8 -Xlint:all,-options -Werror -d .cache/mobile-oracle/android/protocol-classes tools/mobile-oracle/runners/AndroidCryptoProbe.java tools/mobile-oracle/runners/AndroidDecompressProbe.java tools/mobile-oracle/runners/AndroidCurveProbe.java
"$BUILD_TOOLS/d8" --min-api 23 --output .cache/mobile-oracle/android/protocol-dex .cache/mobile-oracle/android/protocol-classes/AndroidCryptoProbe.class .cache/mobile-oracle/android/protocol-classes/AndroidDecompressProbe.class .cache/mobile-oracle/android/protocol-classes/AndroidCurveProbe.class
```

Copy and verify the runner, original APK, loader and container, then make those
inputs read-only. Replace all reviewed pins and paths below. Run **inside the
isolated namespace**, targeting only its dedicated emulator:

```sh
timeout 160s adb -P 5041 -s emulator-5580 shell -n 'CLASSPATH=/data/local/tmp/mobile-oracle/protocol-runner.dex:/data/local/tmp/mobile-oracle/base.apk timeout 140 app_process -Xmx256m /system/bin AndroidDecompressProbe /data/local/tmp/mobile-oracle/libsuperpack.so REVIEWED_LOADER_SHA256 /data/local/tmp/mobile-oracle/libs.so REVIEWED_CONTAINER_SHA256 /data/local/tmp/mobile-oracle/fresh-primary' > .cache/mobile-oracle/android/decompression-report.json
timeout 100s adb -P 5041 -s emulator-5580 shell -n 'CLASSPATH=/data/local/tmp/mobile-oracle/protocol-runner.dex:/data/local/tmp/mobile-oracle/base.apk timeout 80 app_process -Xmx256m /system/bin AndroidCurveProbe /data/local/tmp/mobile-oracle/fresh-primary/libcurve25519.so REVIEWED_CURVE_SHA256' > .cache/mobile-oracle/android/curve-report.json
```

Retain stderr separately and require zero exit statuses before interpreting a
report. Copy outputs back into private storage, check host hashes against the
native inventory and compare independent fresh extractions. Do not execute an
unreviewed output merely because decompression succeeded.

Run the external-evidence Rust comparison with an absolute path:

```sh
MOBILE_CURVE_PROBE_REPORT="$PWD/.cache/mobile-oracle/android/curve-report.json" \
MOBILE_CURVE_EXPECTED_LIBRARY_SHA256=REVIEWED_CURVE_SHA256 \
MOBILE_CURVE_EXPECTED_FINGERPRINT=REVIEWED_IMAGE_FINGERPRINT \
cargo test --locked -p mobile-oracle --test curve_probe -- --ignored
```

The test is ignored by default because it requires separately acquired artifacts
and an independently run Android process. Explicit invocation fails when the
report/pins are missing. It does not execute Android or authenticate an
operator-supplied report. Use the reviewed extraction lock, not values trusted
only because they occur inside that report.

## Limits

Results are actual original-native execution on the exercised image and ABI.
They do not simulate a successful protocol session. No Android/JNI behavior is
stubbed. The native provider path is not proof of the production application's
active-provider selection. Full Signal state transitions, Noise, packet parsing,
VRF/VOPRF, all secondary libraries, other ABIs/SDKs, physical devices, attestation,
account-specific configuration, registration and server acceptance remain
unqualified. Dynamic import tracing and exhaustive native dependency coverage
are not implemented. Deterministic test randomness is strictly offline.

Stop only the dedicated emulator and its ADB server after capture. Prefer its
`adb ... emu kill` console command for a graceful shutdown. Preserve private
artifacts according to their acquisition/storage policy.
