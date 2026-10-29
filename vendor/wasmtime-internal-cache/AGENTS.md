# Imported Wasmtime cache source

Files under `src/` and `tests/` are unchanged upstream source, with original hashes
in `UPSTREAM.json`. Preserve them byte-for-byte, including upstream `unwrap()` and
nested `if let` patterns; the root style rules for those patterns do not apply to
these imported files. This exception does not cover locally modified files or
permit new source changes. The local `build.rs` follows the root rules.
