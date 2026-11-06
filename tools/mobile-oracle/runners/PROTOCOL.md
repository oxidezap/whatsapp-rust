# Original native scenarios from Rust

Follow the [isolated Android workflow](ANDROID.md). Each scenario loads a
separately hash-pinned library through `System.load` in the original APK's
classloader. Requalify original constructors, native registration and reachable
paths before each new snapshot. No account, installation or network is involved.

`superpack` calls the original `AssetDecompressor` marker and architecture
methods. An eight-byte zero buffer must become `47 77 53 00 00 00 00 00`; a
two-byte buffer must raise the original array-bounds exception. The architecture
integer is recorded without treating its mapping as a universal ABI enum.

`decompress` additionally requires `container`, `container_sha256` and `output`
in the private configuration. Output must be a fresh directory below the guest
capture root. It calls original `decompress_range_from_so` with format `spo`,
start 0 and count 18, matching the reviewed primary-library profile. It requires
18 returned names and 18 ordinary output files. It checks 64 MiB per file and
512 MiB aggregate after execution. These are postchecks; the disposable VM and
external deadline provide isolation. Run twice with fresh directories and compare
complete private inventories. Extracted provenance is operator-declared, not raw
APK-member ancestry; preserve the original loader/container ancestry separately.

`curve` loads a hash-pinned extracted library and calls the original
`NativeCurve25519Provider`. Its constructor's smoke check is retained; the selected
operations use explicit synthetic inputs, not its random provider. It exercises
RFC 7748 Alice/Bob keys and agreements, a fixed zero-randomness signature of a
synthetic message, valid/altered-message verification, torsion checking and
rejection of a low-order public key. The ignored `curve_probe` Rust test compares
the captured results with RustCrypto, including exact deterministic signatures.

These are original native invocations, not proof that registration or any server
accepts a request. Keep detailed native inventories and reports private.
