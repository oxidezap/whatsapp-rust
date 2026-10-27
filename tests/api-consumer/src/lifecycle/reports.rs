//! Awaiting shutdown must not silently discard persistence failures.
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! async fn discard(client: &std::sync::Arc<whatsapp_rust::Client>) {
//!     client.shutdown().await;
//! }
//! ```
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! async fn discard(client: &std::sync::Arc<whatsapp_rust::Client>) {
//!     client.logout().await;
//! }
//! ```
//!
//! ```compile_fail
//! #![deny(unused_must_use)]
//! async fn discard(handle: whatsapp_rust::bot::BotHandle) {
//!     handle.shutdown().await;
//! }
//! ```

pub async fn observe(client: &std::sync::Arc<whatsapp_rust::Client>) {
    let report = client.shutdown().await;
    let _device_result = report.device;
    let _secret_failures = report.message_secrets.failed_batches;
}

pub async fn observe_logout(client: &std::sync::Arc<whatsapp_rust::Client>) {
    let report = client.logout().await;
    let _deregistration = report.deregistration;
}

pub async fn observe_bot(handle: whatsapp_rust::bot::BotHandle) {
    let report = handle.shutdown().await;
    let _run = report.run;
}
