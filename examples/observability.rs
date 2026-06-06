//! Wiring observability for `whatsapp-rust`.
//!
//! Run with:
//!     cargo run --example observability --features tracing
//!
//! The library only *emits* `tracing` spans/events (and keeps its existing `log`
//! calls). It never installs a subscriber and never depends on OpenTelemetry —
//! that is the application's job, shown here.
//!
//! Two things happen below:
//!
//! 1. A `tracing-subscriber` is installed. Its default `tracing-log` feature
//!    bridges the library's existing `log::{info,warn,error}!` calls into
//!    tracing, so they become events attached to the active `wa.*` span.
//! 2. Span/level/target filtering is driven by `RUST_LOG` (EnvFilter), e.g.
//!    `RUST_LOG="info,whatsapp_rust=debug,wacore=debug"`. The library groups
//!    spans under `wa.*` names and reuses its `target: "Client/AppState"`-style
//!    targets, so you can filter per area.
//!
//! IMPORTANT: do NOT enable the `log` feature on the `tracing` crate together
//! with a log->tracing bridge — that recurses. This crate already pins
//! `tracing` with `default-features = false` so the hazard cannot happen.
//!
//! PII note: the bridged `log` lines surface alongside the redacted `wa.*` spans.
//! The library renders JIDs in its log messages through `Jid::observe()` (phone
//! numbers become `pn#<keyed-token>`), so they carry the same redaction as the
//! span fields. If your own application code logs raw JIDs/phone numbers, those
//! will still reach the trace exporter — scrub them with `Jid::observe()` too, or
//! drop the `whatsapp_rust`/`wacore` `log` targets from your export layer. The
//! `tracing-pii` cargo feature (off) renders raw numbers for local debugging only.

fn main() {
    use tracing_subscriber::prelude::*;

    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,whatsapp_rust=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        // ── OpenTelemetry (OTLP) ────────────────────────────────────────────
        // Add the application deps `opentelemetry`, `opentelemetry-otlp` and
        // `tracing-opentelemetry`, then append a layer here:
        //
        //     let tracer = opentelemetry_otlp::new_pipeline()
        //         .tracing()
        //         .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        //         .install_batch(opentelemetry_sdk::runtime::Tokio)?;
        //     .with(tracing_opentelemetry::layer().with_tracer(tracer))
        //
        // Every `wa.*` span is then exported as an OTLP span with its fields
        // (chat/peer/msg_id are already privacy-redacted via `Jid::observe()`).
        .init();

    tracing::info!("observability initialized — RUST_LOG drives filtering");

    // From here you would build and run a `whatsapp_rust::Client` as usual; all
    // connect / recv / decrypt / send / iq / appstate / pair / media spans and
    // the bridged log events will flow into the subscriber above.
}
