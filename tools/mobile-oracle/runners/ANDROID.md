# Original DEX and JNI probes on Android

These opt-in runners exercise small reviewed paths from a signature-qualified,
hash-pinned official Android snapshot. They do not install or launch the
application, create an Android account, register a number or contact a server.
Keep original APK/DEX/native bytes, generated DEX and reports private and ignored.
The [APK evidence guide](../../../agent_docs/mobile_oracle.md) remains required.

## Separate execution modes

| Runner | What executes | What remains unqualified |
| --- | --- | --- |
| `DexHmacProbe` | One method translated to a host JVM, initializer excluded | Original ART, Android providers and JNI |
| `AndroidCryptoProbe` | Original unmodified DEX in ART, original class initializer retained, synthetic HMAC and PBKDF2-8BIT inputs | Installed application/manifest policy, token caller/material selection, server acceptance |
| `AndroidSuperpackProbe` | Original Superpack library through Android's native loader/JNI, native registration and synthetic array marker | Decompression, packed internal libraries and protocol cryptography |

Android reports use `android_art_original_dex` or `android_art_original_jni`.
They are not the input schema of `mobile-oracle compare`. The opt-in Rust test
consumes an operator-supplied ART report; it cannot authenticate its origin or
promote the entire application to qualified status.

`AndroidCryptoProbe` loads the exact original DEX using `PathClassLoader`. The
current reviewed profile selects static methods `A0M([B[[B)[B` and
`A08(String,byte[],char[],int,int):SecretKey` from an explicitly supplied class.
Names must be requalified against each snapshot. A changed DEX pin, missing
method or provider fails, with no fallback or replacement method. Before running,
review the original class initializer and its reachable constructors as well as
the methods. Hash/fingerprint the complete DEX and the original instruction
ranges, including separate review of pools and exception tables.

The crypto probe runs two RFC 2202 HMAC cases, a binary-key/empty-message case,
and a fabricated token-message case. It checks empty message-part arrays fail.
For PBKDF2 it supplies package bytes `com.whatsapp` plus synthetic logo bytes
`00 ff 80 01`, salt bytes 0..15, 128 iterations and 512 output bits. Password
bytes are cast individually to Java chars, including negative-byte sign extension,
and passed to the original `PBKDF2WithHmacSHA1And8BIT` helper. This tests Android's
actual provider behavior with binary input, not a production salt or token.

The JNI probe requires the original APK on the runner's own classpath so JNI
registration resolves original classes in the same classloader. It separately
hash-checks `libsuperpack.so`, loads it with `System.load`, calls the original
native `get_architecture`, and tests `testDecompressorLibraryUsable` with an
8-byte zero array and an undersized 2-byte array. The marker expected from the
reviewed native instructions is `47 77 53`, with untouched trailing zero bytes.
The short array must raise the VM's array-bounds exception. The architecture
integer is recorded without inventing platform-wide enum meanings.

## Isolated disposable runtime

Use a fresh official AOSP emulator image. Pin the emulator, platform-tools,
image and Build Tools distributions and preserve their exact hashes with the
private report. The first exercised setup was Android 15/API 35, AOSP default
x86_64 r02, with stable Google Emulator 37.2.13 and Platform Tools 37.0.1:

| Official Google archive | SHA-256 |
| --- | --- |
| `emulator-linux_x64-16740729.zip` | `cbc7c3cfb83220b8a0f9c513fdab557712e74d339ec19ddffa43f23350ae9bdd` |
| `sys-img/android/x86_64-35_r02.zip` | `6dd7de33e63ef105cf2fabea6badda1dbe7665c96d8908e5f6e1407e63ff4556` |
| `platform-tools_r37.0.1-linux.zip` | `d230f13842f60f782a8645f9c813f8f845bf36089ea7289f28c48f17979313f1` |

These are paths under `https://dl.google.com/android/repository/`. Verify them
against the official repository catalogs and the pinned SHA-256 values; do not
substitute a newer download silently. Build Tools 36's pin is in the
[translated probe instructions](README.md). No image or proprietary artifact is
bundled with this repository, and no tool is downloaded by a Rust test.

Run emulator and ADB inside an isolated network namespace with only loopback
and no routes. Set `ANDROID_USER_HOME`, `ANDROID_EMULATOR_HOME` and
`ANDROID_AVD_HOME` to fresh private state. Use a dedicated ADB server port, such
as 5041, and emulator ports 5580/5581. Start that server with
`--one-device emulator-5580`, which prevents it connecting to a physical USB
serial, and target every command explicitly with `-s emulator-5580`. Never reuse
the user's ADB server, Android credentials, emulator state or physical device.

A Linux user/network namespace can be created with
`unshare --user --map-root-user --net bash --noprofile --norc`. Bring up only its
loopback with `ip link set lo up`; retain `ip -brief address` and `ip route`
evidence showing there are no external interfaces/routes. Set up the disposable
AVD using the pinned image and SDK, then start with `-accel off -no-window
-no-audio -no-snapshot -no-boot-anim -gpu swiftshader` when KVM is unavailable.
Software emulation can take several minutes. Do not weaken an existing network
policy or use an external emulator/device to get around a blocked setup.

Use `adb shell -n` to avoid consuming subsequent host commands from stdin. Copy
only the pinned artifacts and synthetic runner into `/data/local/tmp/mobile-oracle`,
verify their hashes, and make them read-only. Do not use `adb install`, `am start`,
registration APIs or an Android account. App process execution does not reproduce
an installed application's target-SDK/manifest policy, lifecycle or services.
A running ART probe is not proof of completed system boot or application readiness.

## Build and execute

From the repository root, compile these two source files together using the
verified tools. `BUILD_TOOLS` below points to the pinned Build Tools directory:

```sh
mkdir -p .cache/mobile-oracle/android/runner-classes .cache/mobile-oracle/android/runner-dex
javac --release 8 -Xlint:all,-options -Werror -d .cache/mobile-oracle/android/runner-classes tools/mobile-oracle/runners/AndroidCryptoProbe.java tools/mobile-oracle/runners/AndroidSuperpackProbe.java
"$BUILD_TOOLS/d8" --min-api 23 --output .cache/mobile-oracle/android/runner-dex .cache/mobile-oracle/android/runner-classes/AndroidCryptoProbe.class .cache/mobile-oracle/android/runner-classes/AndroidSuperpackProbe.class
```

Hash the generated runner DEX. The Java 8 option-warning category alone is
suppressed because newer JDKs deprecate that source target. Actual Java source
warnings remain errors.

After copying and checking the inputs, run the following **inside the isolated
namespace**. Replace the reviewed pins and class name; keep output reports in
the private cache. `original.dex` is an unchanged verified raw APK member:

```sh
timeout 120s adb -P 5041 -s emulator-5580 shell -n 'CLASSPATH=/data/local/tmp/mobile-oracle/runner.dex timeout 100 app_process -Xmx256m /system/bin AndroidCryptoProbe /data/local/tmp/mobile-oracle/original.dex REVIEWED_DEX_SHA256 reviewed.Class' > .cache/mobile-oracle/android/art-report.json
timeout 100s adb -P 5041 -s emulator-5580 shell -n 'CLASSPATH=/data/local/tmp/mobile-oracle/runner.dex:/data/local/tmp/mobile-oracle/base.apk timeout 80 app_process -Xmx256m /system/bin AndroidSuperpackProbe /data/local/tmp/mobile-oracle/libsuperpack.so REVIEWED_LIBRARY_SHA256' > .cache/mobile-oracle/android/jni-report.json
```

Retain exit statuses and stderr. A Java exception, deadline or crash is a failed
probe, even if a partial output file exists. Do not synthesize a successful
report after an unsupported provider, class initializer or JNI dependency.
Record the APK, raw members, selectors, runner DEX, image, provider and report
hashes together. For ELF selectors, map virtual addresses to file offsets
explicitly; build IDs alone do not pin bytes. The Superpack container and its
JNI loader are different artifacts; a loader diagnostic does not qualify the
packed internal libraries.

Run the opt-in Rust comparison with an absolute report path, since Cargo runs
tests from the crate directory:

```sh
MOBILE_ART_PROBE_REPORT="$PWD/.cache/mobile-oracle/android/art-report.json" \
MOBILE_ART_EXPECTED_DEX_SHA256=REVIEWED_DEX_SHA256 \
MOBILE_ART_EXPECTED_FINGERPRINT=REVIEWED_IMAGE_FINGERPRINT \
cargo test --locked -p mobile-oracle --test android_probe -- --ignored
```

This compares the synthetic token message and binary-password derivation against
the actual Rust implementation. It does not download artifacts or execute Java.
The default suite ignores it; explicit invocation fails when evidence is absent.
Do not make it part of unattended CI without a separately reviewed artifact and
runtime policy. Stop the disposable emulator and its dedicated ADB server after
capture. Preserve private originals/evidence according to the acquisition policy.

## Remaining limits

There are no Android/JNI success stubs in these probes. Dynamic import tracing,
full dependency/initialization coverage, physical-device behavior, alternate
SDKs/ABIs, production token-material selection, signing-history behavior, JNI
protocol crypto, attestation, account registration and server acceptance remain
unqualified. Neither Android evidence nor JVM agreement qualifies iOS, ESP32 or
an application's complete mobile API.

The next independently scoped probes cover [original Superpack extraction and Curve25519 JNI](PROTOCOL.md). They add a bounded native crypto comparison while retaining the application/session limits above.

A separate [registration field-format probe](REGISTRATION_FORM.md) exercises the original builder/formatter offline with all byte values and synthetic UTF-8 text; it does not invoke HTTP or registration.
