//! Per-call registry of active sessions and their media-task abort handles. The implementation is
//! runtime-agnostic (it uses `wacore::runtime::AbortHandle`) and lives in `wacore::voip::registry`;
//! this re-export keeps the `whatsapp_rust::voip::registry` path stable.

pub use wacore::voip::registry::CallRegistry;
