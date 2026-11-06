# Original registration field formatting

The Rust `form` scenario calls the original builder `X/KCq` and entry formatter
`X/KrQ` dispatch profile 46 through JNI in the [offline runtime](ANDROID.md).
Requalify these names, constructors and paths against every APK snapshot.
It never constructs a bridge or HTTP client. Rust joins original formatted
entries with `&`, matching the reviewed caller; HTTP and encrypted-envelope
execution remain unqualified.

The synthetic corpus covers all byte values 0..255 as RFC 3986 raw bytes and
URL-safe unpadded Base64. A mixed case covers Unicode text, spaces, `+%&/`, `~*`,
true/false, omitted nullable fields, an out-of-range boolean selector and a UUID
containing bytes 0..15. It retains the original encoded-field set and map order.

Use a configuration with `scenario: "form"` and the APK path/hash described in
ANDROID.md. The report hashes original `classes10.dex` directly from the pinned
APK. The ignored `registration_form_probe` Rust comparison requires explicit
expected DEX and image pins. It checks `push_rfc3986_bytes`, `push_form_text` and
unpadded Base64 against the captured original results. Existing request defaults
and the all-percent byte policy are unchanged.
