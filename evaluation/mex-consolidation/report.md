# MEX validation report

Implementation contract and migration: [contract.md](contract.md).
Base `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`; final implementation commit
is identified by the published PR head (this report does not self-hash).

## Matched measurement

The same `tests/fixtures/mex_consumer/src/bin/size_probe.rs` is built before and
after the library change. Standalone consumer's unchanged Cargo release defaults,
no dependency default features, nightly-2026-06-16, jobs=1, standard Cargo through
configured mbx; CARGO_TARGET_DIR unset. Native flags come from .cargo/config.toml;
WASM uses `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`, target
wasm32-unknown-unknown. No wasm-opt, profile or size budget changes. The probe
keeps the canonical executor's poll reachable, but never connects/sends.

Consumer lock SHA256:
`53dc77dd2d75e7da4ec542f3a556312a8d1bc6eb5f93ee367f5fbee4812fc059`.
Probe SHA256:
`4f8d17b94531a82d1f12f5c878262c19a3c9bb289e842d7188f7d9ffffb6f3da`.

Base builds both exit 0: native stripped 3,052,328 bytes; .text 2,997,898;
allocated text+data+bss 3,049,482; WASM 564,836 bytes. Head measurement pending.
These are consumer-specific measurements, not the CI demo size or evidence of
RAM savings. Existing hard CI budgets and assertions are unchanged.

## Checks

- Source audit: no productive ExtensionError construction before removal.
- fmt and diff whitespace check: passed after implementation.
- Added public positive custom/raw construction and independent negative
  constructor/executor/error doctests in both root rustdoc and renamed-dependency
  standalone package. Existing typed linkage/privacy, optional-key omission and
  direct duplicate-key serializer controls retained.
- Mock IQ controls: mutation wire bytes, optional query execution, actual
  get_username GraphQL/IQ 404 vs 403/500, original parse/JSON sources and
  GraphQL-vs-IQ classification. Execution pending.
- Scoped tests, external consumer/MSRV, doctests, Clippy, WASM and actual head
  configured CI/bot reviews: pending, not represented as passed.

Raw logs, exits and measurement outputs stay in this directory (ignored).
Base build PID 18695 ended before source edits. Resources at head validation
planning: 22GiB available RAM, 181GiB free disk, visible compilers ~4.2GiB RSS;
one jobs=1 lane planned ~3GiB RAM/~15GiB additional disk, preserving 5GiB reserve.
