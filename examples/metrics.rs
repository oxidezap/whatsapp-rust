//! Wiring metrics for `whatsapp-rust`.
//!
//! Run with:
//!     cargo run --example metrics --features metrics
//!
//! The library only *emits* metrics through the `metrics` facade (the `wa_*`
//! counters/histograms/gauges in `whatsapp_rust::telemetry`). It never installs a
//! recorder or depends on Prometheus/OTLP; the application does, as shown here.
//! With the `metrics` feature off there is no dependency and every emit is a
//! zero-cost no-op.
//!
//! Metric labels are strictly categorical (outcome, kind, namespace, ...); JIDs,
//! phone numbers and message ids are never used as labels.

fn main() {
    // Install a Prometheus recorder. `install_recorder()` sets the global recorder
    // and returns a handle you can render from your own HTTP endpoint. Use
    // `PrometheusBuilder::install()` instead (inside a Tokio runtime) to also serve
    // `/metrics` on 0.0.0.0:9000 automatically.
    let handle = metrics_exporter_prometheus::PrometheusBuilder::new()
        .install_recorder()
        .expect("install prometheus recorder");

    // Register units/help for the wa_* metrics (optional, improves the output).
    whatsapp_rust::telemetry::describe();

    // From here you would build and run a `whatsapp_rust::Client` as usual; every
    // wa_* metric is recorded into the recorder above. A couple of demo emits:
    whatsapp_rust::telemetry::connect("ok");
    whatsapp_rust::telemetry::recv("decrypted");
    {
        let _t = whatsapp_rust::telemetry::timer(whatsapp_rust::telemetry::IQ_DURATION);
        // ... the IQ round-trip would happen here; the timer records on drop.
    }

    // Scrape this from your HTTP `/metrics` handler.
    println!("{}", handle.render());
}
