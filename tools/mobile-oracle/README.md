# Static Android material extraction

`cargo run -p mobile-oracle -- material <apk-lock.json> <artifact-root> <recipe.json>` verifies the locked files before parsing selected members. It emits a redacted JSON report and keeps the derived token material in memory. The library's `extract_material` returns that private material to a host integrating the registration API. No phone number, token or secret key is written to stdout. Disk persistence requires the explicit export flag described below.

A recipe explicitly selects the base manifest, base DEX, logo and ordered v1 signature blocks. It binds to the SHA256 of the **exact lock file bytes**:

```json
{
  "schema_version": 1,
  "lock_sha256": "<64 lowercase hex characters>",
  "manifest": { "apk_path": "base.apk", "member": "AndroidManifest.xml" },
  "dex": { "apk_path": "base.apk", "member": "classes.dex" },
  "logo": { "apk_path": "density.apk", "member": "res/drawable-hdpi/about_logo.png" },
  "signatures": [{ "apk_path": "base.apk", "member": "META-INF/EXAMPLE.RSA" }],
  "salt_hex": "<explicit reference-qualified salt>",
  "iterations": 128
}
```

These member names and iterations illustrate the schema; they are **not a verified current APK profile**. Keep actual recipes and derived material in the existing ignored private APK cache. Never commit actual keys, token outputs, APKs or telephone numbers. Synthetic fixture inputs in unit tests are deliberately public and do not represent an account.

The parser checks literal package/version/SDK fields against the evidence lock. It does not resolve Android resources or substitute a default package/version. Logo density, ABI, salt and certificate ordering are operator decisions recorded by the recipe hash; there is no implicit newest version or preferred-device fallback. Signature block selection must include the applicable v1 certificate collection in reference order. DER extraction preserves that order; it does not authenticate X.509, validate signerInfos or establish Android installation trust. Independently run the qualified `apksigner` workflow described by the evidence lock. Alternatively omit `signatures` and set `"signing_block": {"scheme":"v2", "platform_sdk":33}` (or `v3`). This selects one explicit signing scheme and, for v3, its SDK-applicable signer chains. Certificate order is preserved. These structural readers do not verify content digests, signature algorithms, anti-stripping checks or rotation proofs; independent `apksigner` review remains mandatory. ZIP64 and resource-valued version fields are explicitly unsupported.

Select `"scheme":"v3_1"` explicitly for the rotated v3.1 signing block with `platform_sdk` at least 33. Its signed-data layout matches v3, but its block ID is distinct. There is no fallback to v3 when v3.1 is absent or inapplicable. Repeated signing-block IDs are accepted only when their complete values are byte-identical; conflicting duplicates fail. This extracts the selected certificate chain, not Android PackageManager signing history. Do not infer token certificate selection from the installed signer alone. Rotation, history ordering and the actual caller must be qualified separately. See [AOSP v3.1](https://source.android.com/docs/security/features/apksigning/v3-1) and [pinned block IDs](https://android.googlesource.com/platform/tools/apksig/+/refs/tags/android-16.0.0_r1/src/main/java/com/android/apksig/internal/apk/v3/V3SchemeConstants.java).

DEX MD5/SHA256 uses a 16 KiB streaming buffer and a 256 MiB decompression budget. Manifest/logo inputs are limited to 8 MiB, each v1 block to 4 MiB, and imported certificate sets to 32 entries/1 MiB. Existing ZIP central/local ancestry checks reject ambiguous names and unsupported encryption/compression. The bounded AXML parser follows AOSP `ResourceTypes.h` (blob `e330410ed1a0890be266da0e62a3e6d1a25a3cdc`). It accepts a literal metadata subset, not all Android XML.

The token primitive follows whalibmob commit `8f50db4c586534dc0af4d13b26bdd09ef42660c0` (`AndroidApk.js`) and Cobalt commit `cb9fbf172507626948fbc3611d0a6fb00bb3a0f1` (`WhatsAppAndroidClientInfo.java`). Binary PBKDF2-HMAC-SHA1 derives 64 bytes from package UTF8 plus logo; ordered certificate DER, DEX MD5 and ASCII national digits feed HMAC-SHA1. Synthetic RFC6070 and independently computed standard-crypto vectors check the primitive. This is reference-derived behavior, **not execution or differential qualification against an official APK**.

For an explicit private export, append `--export-private <repository-root>` to `material`. The repository must ignore `/.cache/`. Export creates `.cache/mobile-oracle/private/<lock-hash>-<recipe-hash>.json` with mode 0600 in a mode 0700 directory, refuses existing files and symlink cache directories, and returns only its path and SHA256 receipt. The CLI never adds files to Git. Unix permission enforcement is required; other hosts reject file export/import. Library `import_private_material` requires that receipt's expected SHA256 plus the expected lock and recipe hashes, verifies the schema and private file permissions, and refuses symlink files. Retain receipts separately from secret files; hashes establish provenance, not APK signer trust.

Core also exposes `ios_registration_token(exact_version, static_token, national)`, reconstructed from the same pinned libraries' iOS helpers. Hosts must choose the consumer/business static token explicitly and bind it to their version profile. It computes lowercase MD5 hex of the supplied static token, lowercase MD5 hex of the exact version, and national ASCII digits. This has independent synthetic standard-crypto coverage, not official IPA execution coverage; Android APK material does not qualify iOS behavior.

Source references: [whalibmob Android material/token helper](https://github.com/Kunboruto20/whalibmob/blob/8f50db4c586534dc0af4d13b26bdd09ef42660c0/lib/AndroidApk.js), [whalibmob iOS token helper](https://github.com/Kunboruto20/whalibmob/blob/8f50db4c586534dc0af4d13b26bdd09ef42660c0/lib/Registration.js), [Cobalt Android helper](https://github.com/Auties00/Cobalt/blob/cb9fbf172507626948fbc3611d0a6fb00bb3a0f1/modules/lib/src/main/java/com/github/auties00/cobalt/client/linked/info/WhatsAppAndroidClientInfo.java), [Cobalt iOS helper](https://github.com/Auties00/Cobalt/blob/cb9fbf172507626948fbc3611d0a6fb00bb3a0f1/modules/lib/src/main/java/com/github/auties00/cobalt/client/linked/info/WhatsAppIosClientInfo.java), [AOSP v2 signing format](https://source.android.com/docs/security/features/apksigning/v2), [AOSP v3 signing format](https://source.android.com/docs/security/features/apksigning/v3).

The [Rust probe runner](runners/README.md) emits a DEX bootstrap and builds a Rust JNI library. It replaces the retired translated-DEX host-JVM probe; no translated-JVM execution is provided.

The opt-in [Android ART/JNI scenarios](runners/ANDROID.md) run reviewed original helpers in a disposable, network-isolated AOSP image through that runner. They preserve the distinction between pinned helper observations, native diagnostics and complete application/protocol qualification. Synthetic offline results do not establish server acceptance or qualify every APK version.
