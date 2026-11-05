# Original registration field formatting

`AndroidRegistrationFormProbe` exercises the original request-field builder and
its original entry formatter in the [isolated Android runtime](ANDROID.md).
It never constructs a bridge/HTTP client or calls a registration endpoint.
The formatter is the same reviewed function used by the HTTP caller to join
entries; the runner itself joins those formatted entries with `&` and records
the resulting synthetic body. This is field/body-format evidence, not execution
of the HTTP or encrypted-envelope caller.

The reviewed snapshot uses builder `X.KCq`, byte methods `A04`/`A06`, optional
methods `A02`/`A05`, boolean method `A00`, UUID method `A03`, and formatter
`X.KrQ` dispatch profile 46. Those names and the dispatch value require review
for every new snapshot. The builder creates an ordered map and an encoded-field
set. The formatter percent-encodes each field name, URL-form-encodes ordinary
values and preserves values already in the encoded-field set. Review the original
constructors, byte encoder and formatter branch before using another artifact.

## Exercised corpus

The runner supplies every byte value 0..255 in one binary field, both as raw
percent-encoded bytes and URL-safe Base64 without padding. It also supplies a
synthetic UTF-8 label with spaces, plus/percent/ampersand/slash, an accented
character, `~` and `*`; explicit true/false; absent nullable text/bytes; an ignored
out-of-range boolean selector; and a synthetic UUID containing bytes 0..15.
No identity, OTP, token material or real telephone number is used.

The raw-byte helper preserves RFC 3986's unreserved set and uses uppercase hex
for other bytes. Its flag prevents a later formatter pass from converting `%`
to `%25`. Ordinary text uses application/x-www-form-urlencoded rules: a space
becomes `+`, a literal plus is escaped, `*` is literal and `~` is escaped.
These are distinct encoding policies.

The portable Rust `RegistrationForm` exposes explicit `push_rfc3986_bytes` and
`push_form_text` helpers for those policies. Existing `push_percent_bytes`
continues its documented escape-every-byte policy. All return already encoded
field values for `encode` to join verbatim. The new methods do not select a new
registration profile or change existing request defaults. A different escaping
spelling alone is not evidence of a server incompatibility; server acceptance
has not been tested.

A regular core regression checks the policies and absence of double encoding.
The opt-in original-ART comparison checks exact body bytes for all three corpus
cases, Base64, optional omissions and field order. It requires explicit DEX and
image pins, never replaces a mismatch with a newly inferred expected body, and
remains ignored when external evidence is unavailable.

## Reproduce

Use the previously pinned Android image, Build Tools, original APK and isolated
ADB server. The original APK must be a separately verified, read-only classpath
entry. This profile additionally hashes its `classes10.dex` resource at runtime;
compare that identity against independently verified raw-member extraction.
The runtime resource hash is an identity check, not an authenticity proof.

From the repository root:

```sh
mkdir -p .cache/mobile-oracle/android/registration-classes .cache/mobile-oracle/android/registration-dex
javac --release 8 -Xlint:all,-options -Werror -d .cache/mobile-oracle/android/registration-classes tools/mobile-oracle/runners/AndroidCryptoProbe.java tools/mobile-oracle/runners/AndroidRegistrationFormProbe.java
"$BUILD_TOOLS/d8" --min-api 23 --output .cache/mobile-oracle/android/registration-dex .cache/mobile-oracle/android/registration-classes/AndroidCryptoProbe.class .cache/mobile-oracle/android/registration-classes/AndroidRegistrationFormProbe.class
```

Hash and copy the runner DEX into the disposable guest; retain original APK,
raw DEX, builder/formatter instruction selectors, tools and image pins privately.
Run **inside the loopback-only namespace**, with no other APK on the classpath:

```sh
timeout 120s adb -P 5041 -s emulator-5580 shell -n 'CLASSPATH=/data/local/tmp/mobile-oracle/registration-runner.dex:/data/local/tmp/mobile-oracle/base.apk timeout 100 app_process -Xmx256m /system/bin AndroidRegistrationFormProbe' > .cache/mobile-oracle/android/registration-form-report.json
```

Capture stderr separately and require exit status zero before interpreting the
report. The runner retains original constructors and checks classloader identity.
There are no Android/JNI success stubs. Keep the detailed bodies and source hashes
private with the other evidence. Only synthetic outputs occur in the report.

Compare with Rust using absolute paths because Cargo changes the test cwd:

```sh
MOBILE_FORM_PROBE_REPORT="$PWD/.cache/mobile-oracle/android/registration-form-report.json" \
MOBILE_FORM_EXPECTED_DEX_SHA256=REVIEWED_DEX_SHA256 \
MOBILE_FORM_EXPECTED_FINGERPRINT=REVIEWED_IMAGE_FINGERPRINT \
cargo test --locked -p mobile-oracle --test registration_form_probe -- --ignored
```

This consumes operator-supplied evidence and cannot authenticate the runner.
Default tests do not fetch an APK, execute Android or contact any endpoint.

## Remaining boundaries

Field serialization is independent from response parsing and provider/network
initialization. The full original registration bridge constructor reaches the
application's dependency graph, including its optional provider/context lookup.
Its HTTP/coroutine methods reach retry logic, platform providers and transmission.
Those methods were inspected as call sites, not invoked by this probe. Exercising
them would require a separately reviewed application-state/provider/HTTP harness;
bypassing the constructor or supplying successful transport stubs would not
qualify the original workflow.

No full registration response mapper, encrypted-request caller, retry behavior,
OTP delivery, application lifecycle, live account or server acceptance is claimed.
This scope also does not qualify every Java String case, alternate encodings,
other SDKs/ABIs, Business/iOS or runtime-specific bridge implementations.
