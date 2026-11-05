#![cfg(target_arch = "wasm32")]

//! Browser execution of portable mobile policy, payloads, crypto and supplied SDK ports.
//! Replies and accounts are synthetic. Tests use only an in-memory persistence
//! port, with no transport, APK, filesystem database or real SMS. A backend reload
//! does not qualify process-crash persistence or server acceptance.

use api_consumer::mobile::{bind_state, mock_environment, workflow};
use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};
use whatsapp_rust::mobile::{
    EnvironmentBound, MobileConfigurationError, MobileEnvironment, RegistrationError,
    RegistrationIdentity, RegistrationOperation, RegistrationOutcome, RegistrationPolicy,
    RegistrationRunId, RegistrationStage, RegistrationWorkflow,
};

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn browser_executes_public_mobile_payload_and_configuration() {
    api_consumer::mobile::exercise_login_payload().expect("synthetic login profile");
}

#[wasm_bindgen_test]
fn browser_registration_envelope_matches_independent_crypto_vector() {
    api_consumer::mobile::exercise_registration_envelope().expect("RFC7748/Node envelope vector");
}

#[wasm_bindgen_test]
async fn browser_registration_persists_identity_through_supplied_ports() {
    api_consumer::mobile::runtime_fixture::exercise_durable_registration()
        .await
        .expect("synthetic SDK registration and durable restore");
}

fn identity() -> RegistrationIdentity {
    RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32])
}

fn new_workflow(environment: &MobileEnvironment, run: u8) -> RegistrationWorkflow {
    workflow(
        environment,
        identity(),
        RegistrationRunId::from_bytes([run; 32]),
        RegistrationPolicy::new(3, 100).expect("synthetic bounded policy"),
    )
}

fn roundtrip(
    workflow: &RegistrationWorkflow,
    environment: &MobileEnvironment,
) -> RegistrationWorkflow {
    let encoded = serde_json::to_vec(&workflow.checkpoint()).expect("serialize checkpoint");
    RegistrationWorkflow::restore(
        serde_json::from_slice(&encoded).expect("deserialize checkpoint"),
        environment.binding(),
        identity(),
    )
    .expect("restore matching checkpoint")
}

#[wasm_bindgen_test]
fn browser_resumes_uncertain_delivery_without_resending() {
    let environment = mock_environment(4443).expect("synthetic environment");
    let mut original = new_workflow(&environment, 7);
    let attempt = original
        .begin(RegistrationOperation::RequestCode, 10)
        .expect("reserve synthetic request");
    let mut restored = roundtrip(&original, &environment);
    assert!(matches!(
        restored.stage(),
        RegistrationStage::OutcomeUnknown { .. }
    ));
    let before_retry = restored.checkpoint();
    assert_eq!(
        restored.begin(RegistrationOperation::RequestCode, 20),
        Err(RegistrationError::InvalidTransition)
    );
    assert_eq!(restored.checkpoint(), before_retry);
    restored
        .complete(attempt, RegistrationOutcome::NeedsCode, 20)
        .expect("supplied late response");
    let submission = restored
        .begin(RegistrationOperation::SubmitCode, 21)
        .expect("reserve synthetic submission");
    restored
        .complete(submission, RegistrationOutcome::Verified, 22)
        .expect("supplied verification");
    assert_eq!(restored.stage(), RegistrationStage::Verified);
    let commit = restored
        .pending_persistence()
        .expect("current verified checkpoint");
    restored
        .confirm_persisted(commit)
        .expect("synthetic durable-write acknowledgement");
    assert_eq!(restored.stage(), RegistrationStage::Registered);
}

#[wasm_bindgen_test]
fn browser_rejects_stale_durability_and_restores_the_current_token() {
    let environment = mock_environment(4443).expect("synthetic environment");
    let mut original = new_workflow(&environment, 8);
    let attempt = original
        .begin(RegistrationOperation::RequestCode, 10)
        .expect("reserve");
    original
        .complete(attempt, RegistrationOutcome::Verified, 11)
        .expect("supplied verification");
    let stale = original.pending_persistence().expect("earlier snapshot");
    assert!(!original.expire(20).expect("advance supplied time"));
    let current = original.pending_persistence().expect("current snapshot");
    assert_ne!(stale, current);
    let before = original.checkpoint();
    assert_eq!(
        original.confirm_persisted(stale),
        Err(RegistrationError::StaleCompletion)
    );
    assert_eq!(original.checkpoint(), before);
    let mut restored = roundtrip(&original, &environment);
    assert_eq!(restored.pending_persistence(), Some(current));
    restored
        .confirm_persisted(current)
        .expect("supplied current durability acknowledgement");
    assert_eq!(restored.stage(), RegistrationStage::Registered);
    let published = restored.checkpoint();
    assert_eq!(
        restored.confirm_persisted(current),
        Err(RegistrationError::InvalidTransition)
    );
    assert_eq!(restored.checkpoint(), published);
}

#[wasm_bindgen_test]
fn browser_keeps_environment_state_and_responses_separate() {
    let environment = mock_environment(4443).expect("synthetic environment");
    let foreign_environment = mock_environment(4444).expect("separate synthetic environment");
    let state = bind_state(&environment, vec![1_u8, 2, 3]);
    let encoded = serde_json::to_vec(&state).expect("serialize bound state");
    let restored: EnvironmentBound<Vec<u8>> =
        serde_json::from_slice(&encoded).expect("deserialize bound state");
    assert_eq!(
        restored.get(&environment).expect("matching binding"),
        &[1, 2, 3]
    );
    assert_eq!(
        restored.get(&foreign_environment),
        Err(MobileConfigurationError::EnvironmentMismatch)
    );

    let mut current = new_workflow(&environment, 9);
    let attempt = current
        .begin(RegistrationOperation::RequestCode, 10)
        .expect("reserve current attempt");
    let mut foreign = new_workflow(&foreign_environment, 9);
    let foreign_attempt = foreign
        .begin(RegistrationOperation::RequestCode, 10)
        .expect("reserve foreign attempt");
    let before = current.checkpoint();
    assert_eq!(
        current.complete(foreign_attempt, RegistrationOutcome::Verified, 11),
        Err(RegistrationError::StaleCompletion)
    );
    assert_eq!(current.checkpoint(), before);
    assert_eq!(
        current.delivery_uncertain(foreign_attempt),
        Err(RegistrationError::StaleCompletion)
    );
    assert_eq!(current.checkpoint(), before);
    current
        .complete(attempt, RegistrationOutcome::NeedsCode, 11)
        .expect("supplied matching response");
    assert_eq!(current.stage(), RegistrationStage::AwaitingCode);
}
