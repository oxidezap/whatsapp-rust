# whatsapp-rust Docker build
#
# Produces a fully static musl binary running on a scratch (empty) container.
# musl is preferred over glibc for long-running processes: predictable memory
# usage with no fragmentation from glibc's per-thread arena allocator.
#
# Build:  docker build -t whatsapp-rust .
# Run:    docker run -v whatsapp-data:/data whatsapp-rust
#
# The /data volume persists the SQLite database across restarts. The image runs
# unprivileged (uid 65532); use a named volume (as below) so it inherits that
# ownership and stays writable — a host bind mount would need matching ownership.
#
# Upgrading from the old root-running image? An existing volume keeps its
# root-owned files, which uid 65532 can't write. Chown it once with any image
# that ships chown (scratch doesn't):
#   docker run --rm -v whatsapp-data:/data alpine chown -R 65532:65532 /data
#
# Pass --phone <number> for pair code auth:
#   docker run -v whatsapp-data:/data whatsapp-rust --phone 15551234567

# --- Planner: extract dependency recipe ---
FROM rust:alpine AS chef
RUN apk add --no-cache musl-dev
COPY rust-toolchain.toml .
# rust-src feeds -Zbuild-std in the builder stage. cargo-chef is pinned to an
# exact release (plus --locked for its dependency graph) so image rebuilds are
# deterministic instead of tracking the latest crates.io release.
RUN rustup show && rustup component add rust-src && cargo install cargo-chef --locked --version 0.1.77
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Builder: cook deps (cached layer), then compile source ---
FROM chef AS builder

# -Zshare-generics reuses upstream monomorphizations instead of re-codegening
# them per crate, deduplicating most cross-crate generic/coroutine copies
# (measured: -666 KiB, -5.6% .text). Nightly-only, which this image already
# pins via rust-toolchain.toml; with fat LTO the historical inlining downside
# does not apply since LTO sees all bitcode anyway.
# No -C target-cpu: a published image must run on any host of its arch, so we
# keep the portable musl baseline instead of pinning the build machine's CPU
# (target-cpu=native would risk SIGILL on older/different hosts and is
# meaningless under emulated cross-arch builds).
ENV RUSTFLAGS="-Zshare-generics=y"

# build-std recompiles std with the release profile so it participates in fat
# LTO and dead-code elimination instead of linking the prebuilt rustup std
# (measured: another -303 KiB). The env form reaches both the chef cook and
# the final build, keeping the dependency cache layer valid; build-std
# requires an explicit --target, hence the musl triple on both invocations.
ENV CARGO_UNSTABLE_BUILD_STD="std,panic_abort"

# The dependency cook runs before the source COPY, so make the nightly
# override explicit in /app instead of relying on rustup walking up to the
# chef stage's copy at /; the nightly-only RUSTFLAGS above depends on it.
COPY rust-toolchain.toml .
COPY --from=planner /app/recipe.json recipe.json
# build-std demands an explicit --target; derive the image's own host triple so
# buildx per-arch builds (linux/amd64, linux/arm64) each target their own arch.
# No pipe, so a rustc failure isn't masked by sed's exit status (Hadolint DL4006).
RUN rustc -vV > /rustc-version \
    && sed -n 's/^host: //p' /rustc-version > /rust-target \
    && test -s /rust-target \
    && rm /rustc-version
# Cook examples so the demo's dev-deps (env_logger, …) are cached, not rebuilt
# after the source COPY. cargo-chef exposes only the plural --examples (no
# --example <name>); under default features that builds just demo, since the
# other examples gate on extra features.
RUN cargo chef cook --release --recipe-path recipe.json --target "$(cat /rust-target)" --examples
COPY . .
# The client lives in examples/demo.rs (the package no longer ships a bin); the
# example artifact lands under release/examples/. Default features cover it.
RUN cargo build --release --example demo --target "$(cat /rust-target)" \
    && cp "target/$(cat /rust-target)/release/examples/demo" /app/whatsapp-rust-bin

# Empty dirs to stage into the scratch image; the COPY --chown below grants them
# to the unprivileged uid so /data (DB; a fresh named volume inherits the
# ownership) and /tmp (SQLite temp files, absent on scratch) stay writable.
RUN mkdir -p /newroot/data /newroot/tmp

# --- Runtime: static binary on empty image, unprivileged ---
FROM scratch
COPY --from=builder --chown=65532:65532 /newroot/tmp /tmp
COPY --from=builder --chown=65532:65532 /newroot/data /data
COPY --from=builder /app/whatsapp-rust-bin /whatsapp-rust
ENV TMPDIR=/tmp
WORKDIR /data
USER 65532:65532
ENTRYPOINT ["/whatsapp-rust"]
