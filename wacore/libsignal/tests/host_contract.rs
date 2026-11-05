//! Run the pre-rewrite behavioral contracts against the production adapter.
//! These contracts are self-contained within the published crate.
#[path = "contracts/counter_lease.rs"]
mod counter_lease;
#[path = "contracts/session_divergence.rs"]
mod session_divergence;
