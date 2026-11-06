# Rust driver in original Android ART

This opt-in tool invokes reviewed original APK methods through JNI in a
disposable, offline Android VM. It does not install or launch the application,
create an account, register a number or contact a server. Original APK classes
and native libraries are external reference artifacts, never repository source.
The [evidence guide](../../../agent_docs/mobile_oracle.md) remains required.

## Build

Build the Rust cdylib for the guest ABI with a pinned official Android NDK and
Rust Android target. For an API 35 x86_64 guest:

```sh
rustup target add x86_64-linux-android
CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER="$NDK/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android35-clang" cargo build --release -p mobile-oracle-android --target x86_64-linux-android
cargo run -p mobile-oracle -- bootstrap /private/rust-entry.dex /data/local/tmp/mobile-oracle/libmobile_oracle_android.so
```

The emitter refuses to overwrite the output. Its generated DEX has one class:
`org.oxidezap.oracle.RustRunner`. The initializer calls `System.load` for the
literal library path; `main(String[])` is a native Rust entry point. It neither
rewrites original classes nor embeds source in another language. Inspect the
output with a separately pinned DEX disassembler before first use.

## Isolated execution

Use an official disposable AOSP image, a private ADB server restricted to that
emulator and a network namespace containing only loopback and no routes. Keep
physical devices out of the ADB server. Select the explicit emulator serial on
every command. Deny outbound network independently of the probe. Use an external
deadline and bound the guest data partition: native postchecks are not a sandbox.

Record exact emulator, platform-tools, image, NDK, Rust compiler, driver and
bootstrap identities privately. The exercised setup uses AOSP API 35 x86_64,
software emulation and `app_process`; it does not qualify installed application
manifest/target-SDK policy, another ABI or hardware behavior.

Push the pinned APK, driver `.so`, generated DEX and a private JSON configuration
under `/data/local/tmp/mobile-oracle/`. Configuration must contain `scenario`,
`apk` and `apk_sha256`; native scenarios also require `library` and
`library_sha256`. All input paths resolve inside that directory. APK/native
inputs are SHA-256 checked before use. The APK must be the only reference on
CLASSPATH, immediately after the generated entry DEX.

```sh
timeout 120s adb -P 5041 -s emulator-5580 shell -n 'CLASSPATH=/data/local/tmp/mobile-oracle/rust-entry.dex:/data/local/tmp/mobile-oracle/base.apk timeout 100 app_process -Xmx256m /system/bin org.oxidezap.oracle.RustRunner /data/local/tmp/mobile-oracle/crypto.json' > /private/crypto-report.json
```

A separate `self-test` configuration exercises the Rust JNI entry only and
explicitly reports `qualified_android_reference: false`. Unknown scenarios,
missing classes/providers, mismatched hashes and unexpected exceptions fail;
there is no substitute implementation or successful partial report. Stop the
emulator and private ADB server after capture; distinguish capture exit status
from emulator teardown failures.

## Crypto scenario

`crypto` retains the original initializer of reviewed class `X/00M` and calls
`A0M([B[[B)[B` and `A08(String,byte[],char[],int,int):SecretKey`. Requalify names,
initializers and reachable constructors for each snapshot, including original
instruction ranges, pools and exception tables.

The synthetic corpus contains two RFC 2202 HMAC vectors, binary-key/empty-message
and fabricated token-message cases, and an empty parts-array exception. PBKDF2
uses `PBKDF2WithHmacSHA1And8BIT`, package bytes `com.whatsapp`, synthetic logo
bytes `00 ff 80 01`, salt bytes 0..15, 128 iterations and 512 bits. Each password
byte is sign-extended to a Java char. It exercises actual Android provider
behavior without using production token material.

Reports distinguish original DEX from JNI execution and record `driver: rust_jni`.
The ignored `android_probe` Rust comparison checks a separately captured report
with explicit expected DEX and image pins. Consuming JSON cannot authenticate
its origin or qualify token material selection or server acceptance.
