//! Linked mobile-core consumer for the native and WASM binary profiles.
//!
//! Explicit inputs exercise configuration, binding and checkpoint recovery. No
//! runtime, network, ambient clock or entropy source is selected by this probe.

use std::hint::black_box;

use api_consumer::mobile::{bind_state, mock_environment, workflow};
use whatsapp_rust::mobile::{
    RegistrationIdentity, RegistrationOperation, RegistrationOutcome, RegistrationPolicy,
    RegistrationRunId, RegistrationStage, RegistrationWorkflow,
};

fn exercise() -> Result<RegistrationStage, Box<dyn std::error::Error>> {
    api_consumer::mobile::exercise_login_payload()?;
    api_consumer::mobile::exercise_registration_envelope()?;
    let environment = mock_environment(4443)?;
    let state = bind_state(&environment, black_box(7_u32));
    assert_eq!(*state.get(&environment)?, 7);

    let identity = RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]);
    let mut original = workflow(
        &environment,
        identity,
        RegistrationRunId::from_bytes([7; 32]),
        RegistrationPolicy::new(3, 100)?,
    );
    let token = original.begin(RegistrationOperation::RequestCode, black_box(10))?;
    let encoded = serde_json::to_vec(&original.checkpoint())?;
    let checkpoint = serde_json::from_slice(&encoded)?;
    let mut restored = RegistrationWorkflow::restore(checkpoint, environment.binding(), identity)?;
    assert!(matches!(
        restored.stage(),
        RegistrationStage::OutcomeUnknown { .. }
    ));
    restored.complete(token, RegistrationOutcome::NeedsCode, black_box(20))?;
    let token = restored.begin(RegistrationOperation::SubmitCode, black_box(21))?;
    restored.complete(token, RegistrationOutcome::Verified, black_box(22))?;
    // Synthetic host acknowledgement exercises the API; this probe accesses no store.
    let commit = restored
        .pending_persistence()
        .ok_or("missing final durability token")?;
    restored.confirm_persisted(commit)?;
    Ok(restored.stage())
}

fn main() {
    assert!(matches!(
        black_box(exercise()),
        Ok(RegistrationStage::Registered)
    ));
}
