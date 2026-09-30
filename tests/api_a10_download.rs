// CI covers the shared API test source. The standalone manifest/features are
// validated separately from outside the workspace (see the A10 migration doc).
#[path = "fixtures/api_a10_download/src/lib.rs"]
mod consumer;
