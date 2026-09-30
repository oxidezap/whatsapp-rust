//! Keep the standalone fixture's public backend/API source in ordinary all-target CI builds too.
//! Separate no-default/WASM package checks still verify isolation from feature unification.
#![cfg(feature = "voip-control")]

#[path = "fixtures/voip_control_consumer/src/lib.rs"]
mod consumer;

use whatsapp_rust::voip_control::{CallDirection, MediaEvent, MediaSessionKey, VoipMediaBackend};

#[test]
fn external_backend_and_receiver_imports_remain_public() {
    let _acquire = consumer::acquire;
    let _transport = consumer::install_transport;
    let key = MediaSessionKey::builder()
        .call_id("CALL-EVENT-CONSUMER".into())
        .generation(1)
        .build();
    let session = consumer::ExternalBackend.reserve(&key, CallDirection::Outgoing);
    let receiver = session.subscribe();
    assert!(session.publish(MediaEvent::RelayAllocated));
    assert_eq!(receiver.try_recv(), Ok(MediaEvent::RelayAllocated));
}
