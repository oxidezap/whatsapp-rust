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
# Bootstrap image; rustup installs the exact stable version from the copied file.
FROM rust:alpine AS chef
RUN apk add --no-cache musl-dev
COPY rust-toolchain.toml .
# Pin cargo-chef and its dependency graph for reproducible dependency layers.
RUN rustup show && cargo install cargo-chef --locked --version 0.1.77
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# --- Builder: cook deps (cached layer), then compile source ---
FROM chef AS builder

# Use the stable prebuilt musl std instead of nightly -Zbuild-std. Both the
# dependency cook and final build use the same toolchain and portable target.
COPY rust-toolchain.toml .
COPY --from=planner /app/recipe.json recipe.json
# Cargo resolves the workspace's path patch while cooking the dependency recipe.
COPY vendor vendor
# Derive the image's own host triple so
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
