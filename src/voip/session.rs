//! Call state machine and media pipeline composition. The implementation is pure (sans-IO) and
//! lives in `wacore::voip::session`; this re-export keeps the `whatsapp_rust::voip::session` path
//! stable for the Tokio driver and the example. Live media flow over the relay is deferred.

#[cfg(feature = "voip-engine-wacore")]
pub use wacore::voip::session::{MediaPipeline, MediaPipelineParams};
pub use wacore::voip_control::{CallDirection, CallPhase, CallSession};
