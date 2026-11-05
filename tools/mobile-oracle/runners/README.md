# Translated DEX probe

`DexHmacProbe.java` is an opt-in host experiment for one reviewed static
`([B[[B)[B` HMAC-SHA1 helper. It reads a hash-pinned standalone DEX, locates the
selected method's original instruction range, translates only that method with
dex2jar and invokes the resulting class on synthetic inputs. It never launches
an Android application. Raw DEX, translated classes and actual reports belong in
the ignored private cache, not in Git.

The input DEX must first have verified raw-member ancestry to an independently
signature-qualified APK. Use the APK lock, `archive_member` extraction and selector
records from the [evidence guide](../../../agent_docs/mobile_oracle.md). The
probe's whole-DEX SHA-256 binds pools, exception tables and metadata as well as
the separately reported instruction fingerprint. Record that range as a selector
only after reviewing its callers, guards and purpose in the original DEX.

## Scope

The report says `execution: translated_dex_host_jvm` and
`qualified_android_execution: false`. This is a different format from the
operator-supplied `mobile-oracle compare` observations. Do not rename it into
those observations or claim Android/ART/JNI qualification from this result.

The probe deliberately excludes the class initializer, fields, all other APK
methods and application lifecycle. It rejects unsupported inheritance, native
methods, field accesses, dynamic calls and calls outside a fixed short JDK
crypto/exception allowlist **before loading** the selected class. It adds no
Android or JNI success stubs. Translation errors fail instead of installing
exception-shaped replacement methods. The call-site inventory is static;
dynamic import tracing is not implemented. The classloader is an additional
restriction, not a replacement for OS isolation.

The cryptographic provider is the host JDK's provider, recorded in the report.
Clock and randomness are unused in these scenarios. This does not exercise
Android provider selection, PBKDF2's Android-specific 8BIT algorithm, package
signing history, certificate rotation, attestation, Keystore, JNI or any server.
In particular, the selected installation signer does not establish which
certificate a token caller obtains from PackageManager signing history.

Run in a network-disabled process/container with an explicit deadline and heap
limit. The probe reports the actual JVM heap limit; its deadline is externally
enforced. It caps the DEX at 64 MiB, original instruction range at 64 KiB and
translated class at 1 MiB. Keep the input stable while running. This is tooling
for a reviewed pure helper, not a general untrusted-code sandbox.

## Pinned tools and synthetic regression

The initial supported translator is the maintainer's
[dex2jar v2.4 release](https://github.com/pxb1988/dex2jar/releases/tag/v2.4),
asset `dex-tools-v2.4.zip`, SHA-256
`ee7c45eb3c1d2474a6145d8d447e651a736a22d9664b6d3d3be5a5a817dda23a`.
Verify the complete distribution before putting its `lib/*` on the classpath.
Do not silently substitute another release. The source is Java 17 compatible.
Record the actual JDK version, source and runner-distribution hashes with each
private run. Compile the entire runner directory together; hash a JAR containing
all generated `DexHmacProbe*.class` files, not just its outer class.

`DexHmacFixture.java` is wholly synthetic. Generate its DEX with a pinned Google
Android Build Tools D8. Build Tools 36.0.0's Linux distribution has SHA-256
`5d9ac77fb6ff43d9da518a337b4fcf8f9097113df531d99ccefe80ef7ce8250b`.
The source deliberately has a failing class initializer to check the stated
initializer exclusion. It also has forbidden call, field and JNI scenarios.

With `DEX_TOOLS` pointing to the verified dex-tools directory and `BUILD_TOOLS`
to the verified Android Build Tools directory, run from the repository root:

```sh
mkdir -p .cache/mobile-oracle/runner-classes .cache/mobile-oracle/synthetic/classes .cache/mobile-oracle/synthetic/dex
javac --release 17 -cp "$DEX_TOOLS/lib/*" -d .cache/mobile-oracle/runner-classes tools/mobile-oracle/runners/DexHmacProbe.java
javac --release 17 -cp ".cache/mobile-oracle/runner-classes:$DEX_TOOLS/lib/*" -d .cache/mobile-oracle/runner-classes tools/mobile-oracle/runners/DexHmacProbeTest.java
javac --release 17 -d .cache/mobile-oracle/synthetic/classes tools/mobile-oracle/runners/DexHmacFixture.java
"$BUILD_TOOLS/d8" --min-api 23 --output .cache/mobile-oracle/synthetic/dex .cache/mobile-oracle/synthetic/classes/fixtures/DexHmacFixture.class
timeout 30s java -Xmx256m -cp ".cache/mobile-oracle/runner-classes:$DEX_TOOLS/lib/*" DexHmacProbeTest .cache/mobile-oracle/synthetic/dex/classes.dex
```

The seven checks cover synthetic execution, hash mismatch, missing selector,
forbidden call, forbidden field access, native method and truncated DEX. Run
under the network isolation described above. There is no APK in these fixtures.

## Review and run one selected helper

Choose the exact DEX, expected DEX SHA-256, descriptor and method from the reviewed
snapshot. Names alone are not stable selectors. Substitute the reviewed values:

```sh
timeout 25s java -Xmx256m -cp ".cache/mobile-oracle/runner-classes:$DEX_TOOLS/lib/*" DexHmacProbe /private/classes.dex EXPECTED_SHA256 'Lreviewed/Class;' reviewedMethod > .cache/mobile-oracle/probe.json
```

The probe runs two RFC 2202 cases, a binary-key/empty-message case and one
synthetic token-message case, plus an empty-parts error check. Each completed
case records key, message and output hashes. Only synthetic outputs are printed.
The token-message key is bytes 0..63; its message is the fabricated certificate
`30 01 07`, digest bytes 0..15, and ten ASCII zero digits. It supplies one
already-concatenated message to the helper. It does not prove the real token
caller's certificate choice, concatenation, validation or wire encoding.

The opt-in Rust test compares this synthetic message against the actual
`AndroidTokenMaterial::token_base64` implementation. Supply the independently
reviewed DEX and instruction fingerprints, not values inferred as trusted merely
because a report contains them:

```sh
MOBILE_DEX_PROBE_REPORT="$PWD/.cache/mobile-oracle/probe.json" \
MOBILE_DEX_EXPECTED_SHA256=REVIEWED_DEX_SHA256 \
MOBILE_DEX_EXPECTED_SELECTOR_SHA256=REVIEWED_INSTRUCTION_SHA256 \
cargo test --locked -p mobile-oracle --test dex_probe -- --ignored
```

The test is ignored by default because official artifacts and Java tools are
not bundled or downloaded by the Rust suite. Explicit invocation requires the
report and both pins; missing evidence fails. Like the observation comparator,
it consumes operator-supplied evidence and cannot authenticate a runner. A green
comparison establishes only this synthetic pure-helper agreement. Full DEX/ART,
JNI and registration qualification remain separate work.

For separate original-DEX ART and original JNI execution, see the [Android probes](ANDROID.md). Their execution modes and limits differ from this translated JVM probe.
