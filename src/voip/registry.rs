//! Per-call registry of active sessions. The implementation is runtime-agnostic and lives in
//! `wacore::voip_control::registry`; this re-export keeps the `whatsapp_rust::voip::registry` path
//! stable. Media-task handles, counters, and the event stream live on the session behind the seam,
//! not here.

pub use wacore::voip_control::registry::CallRegistry;
