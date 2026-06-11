# whatsapp-rust Docker build
#
# Produces a fully static musl binary running on a scratch (empty) container.
# musl is preferred over glibc for long-running processes: predictable memory
# usage with no fragmentation from glibc's per-thread arena allocator.
#
# Build:  docker build -t whatsapp-rust .
# Run:    docker run -v whatsapp-data:/data whatsapp-rust
#
# The /data volume persists the SQLite database across restarts.
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
ENV RUSTFLAGS="-C target-cpu=native -Zshare-generics=y"

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
# build-std demands an explicit --target; use the image's own host triple so
# multi-arch builds (e.g. buildx linux/arm64) keep producing native binaries
# exactly like the implicit-target build did.
RUN rustc -vV | sed -n 's/^host: //p' > /rust-target && test -s /rust-target
RUN cargo chef cook --release --recipe-path recipe.json --target "$(cat /rust-target)"
COPY . .
RUN cargo build --release --target "$(cat /rust-target)" \
    && cp "target/$(cat /rust-target)/release/whatsapp-rust" /app/whatsapp-rust-bin

# --- Runtime: static binary on empty image ---
FROM scratch
COPY --from=builder /app/whatsapp-rust-bin /whatsapp-rust
WORKDIR /data
ENTRYPOINT ["/whatsapp-rust"]
