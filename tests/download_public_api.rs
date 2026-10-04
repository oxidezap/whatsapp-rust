// CI covers the shared API test source. Validate the standalone manifest and
// minimal features separately from outside the workspace.
#[path = "api-consumer/src/download/native.rs"]
mod consumer;

#[allow(dead_code)] // Compile-only future and platform contracts.
#[path = "api-consumer/src/download/platform.rs"]
mod platform;
