#[test]
fn protocol_state_does_not_execute_io_or_read_global_time() {
    for (name, source) in [
        ("health", include_str!("../src/protocol/health.rs")),
        ("reconnect", include_str!("../src/protocol/reconnect.rs")),
        ("requests", include_str!("../src/request/pending.rs")),
        ("frames", include_str!("../src/socket/frame_encoder.rs")),
        (
            "appstate",
            include_str!("../src/appstate_sync/preparation.rs"),
        ),
    ] {
        let production = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "async fn",
            ".await",
            "crate::runtime",
            "tokio::",
            "std::fs::",
            "std::thread::",
            "Instant::now",
            "now_secs()",
            "rand::",
            "dyn Backend",
            "dyn Transport",
            "async_channel::",
        ] {
            assert!(
                !production.contains(forbidden),
                "{name} depends on {forbidden}"
            );
        }
    }
}
