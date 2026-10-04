# Independent API consumer gate

`tools/xtask/consumers.json` is the execution registry for every standalone
`Cargo.toml` below `tests/`. The only exclusions are the two root workspace test
members (`tests/e2e` and `tests/bench-integration`). Discovery recursively scans
source directories, ignoring hidden directories and `target`; it does not depend
on a directory being named `consumer`. Unregistered, missing, duplicate and
non-standalone registrations fail the gate with their paths. Symlinks in the
source tree are rejected rather than silently bypassing discovery.

```sh
cargo xt ci consumers check
cargo xt ci consumers run --lane native --dry-run
cargo xt ci consumers run --lane msrv --toolchain 1.94.1 --dry-run
cargo xt ci consumers run --lane wasm --dry-run
cargo xt ci consumers run --lane native \
  --manifest tests/fixtures/download_consumer/Cargo.toml
```

CI runs native and MSRV hosts in `.github/workflows/consumers.yml` and WASM
check/build modes in `.github/workflows/wasm.yml`. The xtask driver requires the
**tooling** toolchain; the published crates' MSRV is lower. Build the driver with
nightly and use `--toolchain 1.94.1` for the hosts, not `cargo +1.94.1 xt`.
Registry unit tests also run in the consumer workflow. Matrix lanes do not imply
a feature cross-product: only each entry's explicitly chosen features run.

## Registering a host

Keep the host's own `[workspace]`, dependencies and lockfile independent. Add its
path and meaningful commands in the registry in the same change as its fixture:

```json
{
  "manifest": "tests/fixtures/example_consumer/Cargo.toml",
  "locked": true,
  "commands": [
    { "lanes": ["native", "msrv"], "mode": "test" },
    { "lanes": ["wasm"], "mode": "check", "lib": true }
  ]
}
```

Modes are `check`, `build`, `test` and `run`. A command can specify `features`,
`no_default_features`, `release`, and either `bin` or `lib`. Native `test` without
a target filter includes rustdoc controls; **do not replace it with nextest or
`--lib`** when compile-fail documentation is part of the contract. WASM entries
are check/build only: they do not claim browser or authenticated runtime tests.
Exclude native-only features explicitly (e.g. event delivery uses
`no_default_features` for WASM). No `--all-features` is inferred.

The runner uses jobs=1, a shared local `target/consumers` output directory, empty
native Rust flags and the WASM getrandom backend cfg. Separate Cargo invocations
still resolve each workspace independently; sharing build artifacts does not
unify host features or dependencies. It runs all selected commands and returns
the first failure, with the failed manifest/mode printed. For resource-heavy
local validation in this campaign, wrap the runner with the shared build flock.

Existing checked-in fixture locks retain `--locked`. Legacy hosts with no
checked-in lock explicitly use `locked: false`; they resolve their own graph,
not the root graph. Their owners can commit a fixture-local lock and change the
entry to `true` without touching the shared root lock.

## Directed removal controls

Keep the fixture's positive control and existing precise negative harness. For
an intentionally failing optional binary, use an explicit diagnostic expectation:

```json
{
  "lanes": ["native", "msrv"],
  "mode": "check",
  "features": ["removed-video-state"],
  "bin": "removed-video-state",
  "expect_failure": {
    "error_code": "E0599",
    "contains": ["VideoStateChanged", "CallEvent"]
  }
}
```

This succeeds only when compilation fails with `error[E0599]` and every listed
API-specific diagnostic fragment. An unexpectedly successful compile, missing
dependency, unrelated failure, or absent fragment fails the gate. Stable rustdoc
alone does not enforce an error-code annotation. Deliberately failing optional
features are never enabled by a general all-features walk.
