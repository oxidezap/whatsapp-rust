//! Standalone public lifecycle fixture, without workspace features or flags.
pub mod admission;
#[cfg(test)]
#[path = "../../lifecycle_outcomes.rs"]
mod contract;
