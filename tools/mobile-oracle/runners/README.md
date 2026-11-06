# Rust Android oracle

The maintained driver is [mobile-oracle-android](../../mobile-oracle-android/src/lib.rs).
Its scenarios call original, separately qualified APK classes through JNI in ART.
The host [bootstrap emitter](../src/bootstrap.rs) generates the minimal DEX entry
class directly from Rust. There is no Java source, Java compiler, dex2jar or
JavaScript glue in this runner.

Read [ANDROID.md](ANDROID.md) for isolation, build and invocation requirements,
[PROTOCOL.md](PROTOCOL.md) for native scenarios and
[REGISTRATION_FORM.md](REGISTRATION_FORM.md) for form observations.
Keep all original binaries, generated DEX/native libraries and captures private.
The retired translated-DEX/JVM experiment is historical evidence only; it is
superseded by the original-ART scenarios and has no maintained producer.
