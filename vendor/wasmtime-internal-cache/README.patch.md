# Wasmtime cache dependency backport

This is the published `wasmtime-internal-cache` 48.0.7 crate, with its Rust source,
tests and license copied unchanged from the verified registry archive.
`UPSTREAM.json` records the archive digest, original revision and file digests.

The manifest raises only the zstd dependency floor from 0.13.0 to 0.14.3, following
[Wasmtime's upstream update](https://github.com/bytecodealliance/wasmtime/commit/04f0fcf70248909cc0b6a3ec1d406c7e2d543637).
The private copy is also marked non-publishable and given a standalone workspace
for running its upstream tests. This retains disk caching and the
existing Wasmtime API/MSRV while removing the versions affected by
[RUSTSEC-2026-0391](https://rustsec.org/advisories/RUSTSEC-2026-0391.html),
[RUSTSEC-2026-0398](https://rustsec.org/advisories/RUSTSEC-2026-0398.html) and
[RUSTSEC-2026-0403](https://rustsec.org/advisories/RUSTSEC-2026-0403.html).
No advisory is ignored. The published SDK does not depend on Wasmtime.

Remove this patch when the selected Wasmtime release includes the dependency update.
The build script preserves registry behavior explicitly: compiler identity is the crate
version and executable mtime is excluded. Otherwise its enclosing Git checkout would
partition the shared cache by commit and executable. `UPSTREAM.json` retains the
original build-script digest and records this local exception separately.

Keep library source changes out of this copy; re-import an audited upstream crate if needed.
