//! Mobile contracts must be usable by an isolated host with defaults disabled.
//!
//! This probe performs no I/O and does not require an executor, even through the
//! SDK re-export. APK, mock-server and target execution are separate validation.
//!
//! A pre-send checkpoint acknowledgement cannot publish final registration.
//!
//! ```compile_fail,E0308
//! use whatsapp_rust::mobile::{AttemptToken, RegistrationWorkflow};
//! fn acknowledge_request_checkpoint(flow: &mut RegistrationWorkflow, token: AttemptToken) {
//!     flow.confirm_persisted(token).unwrap();
//! }
//! ```

use whatsapp_rust::mobile::{
    EnvironmentBound, MobileConfigurationError, MobileEnvironment, MobileTrust,
    RegistrationEndpoint, RegistrationIdentity, RegistrationPolicy, RegistrationRunId,
    RegistrationWorkflow, TcpEndpoint,
};

pub fn mock_environment(port: u16) -> Result<MobileEnvironment, MobileConfigurationError> {
    MobileEnvironment::new(
        "isolated-mobile-fixture",
        RegistrationEndpoint::for_test_http("http://mock.test:8080/v2")?,
        TcpEndpoint::new("mock.test", port)?,
        MobileTrust::new([1; 32], [2; 32])?,
    )
}

pub fn bind_state<T>(environment: &MobileEnvironment, state: T) -> EnvironmentBound<T> {
    environment.bind(state)
}

pub fn workflow(
    environment: &MobileEnvironment,
    identity: RegistrationIdentity,
    run_id: RegistrationRunId,
    policy: RegistrationPolicy,
) -> RegistrationWorkflow {
    RegistrationWorkflow::new(environment.binding(), identity, run_id, policy)
}

#[cfg(feature = "mobile-transport-native")]
pub fn native_transport(
    environment: &MobileEnvironment,
) -> Result<whatsapp_rust::transport::TokioTcpTransportFactory, Box<dyn std::error::Error>> {
    let destination = environment.connection();
    Ok(whatsapp_rust::transport::TokioTcpTransportFactory::new(
        destination.host(),
        destination.port(),
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use whatsapp_rust::mobile::{
        RegistrationError, RegistrationOperation, RegistrationOutcome, RegistrationStage,
    };

    #[test]
    fn persisted_attempt_does_not_resend_or_cross_environments() {
        let environment = mock_environment(4443).expect("synthetic environment");
        let state = bind_state(&environment, vec![1_u8, 2, 3]);
        let encoded = serde_json::to_vec(&state).expect("serialize bound state");
        let state: EnvironmentBound<Vec<u8>> =
            serde_json::from_slice(&encoded).expect("deserialize bound state");
        assert_eq!(
            state.get(&environment).expect("same environment"),
            &[1, 2, 3]
        );
        let changed = mock_environment(4444).expect("different synthetic environment");
        assert_eq!(
            state.get(&changed),
            Err(MobileConfigurationError::EnvironmentMismatch)
        );

        let policy = RegistrationPolicy::new(3, 100).expect("bounded policy");
        let identity = RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]);
        let mut original = workflow(
            &environment,
            identity,
            RegistrationRunId::from_bytes([7; 32]),
            policy,
        );
        let token = original
            .begin(RegistrationOperation::RequestCode, 10)
            .expect("reserve attempt");
        let encoded = serde_json::to_vec(&original.checkpoint()).expect("serialize checkpoint");
        let checkpoint = serde_json::from_slice(&encoded).expect("deserialize checkpoint");
        let mut restored =
            RegistrationWorkflow::restore(checkpoint, environment.binding(), identity)
                .expect("restore in-flight attempt");
        assert!(matches!(
            restored.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert_eq!(
            restored.begin(RegistrationOperation::RequestCode, 20),
            Err(RegistrationError::InvalidTransition)
        );
        restored
            .complete(token, RegistrationOutcome::NeedsCode, 20)
            .expect("reconcile matching response");
        assert_eq!(restored.stage(), RegistrationStage::AwaitingCode);
    }

    #[test]
    fn core_and_sdk_use_the_same_mobile_contracts() {
        let environment = mock_environment(4443).expect("synthetic environment");
        let core_binding: wacore::mobile::EnvironmentBinding = environment.binding();
        let policy = RegistrationPolicy::new(1, 100).expect("bounded policy");
        let core_workflow: wacore::mobile::RegistrationWorkflow = RegistrationWorkflow::new(
            core_binding,
            RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]),
            RegistrationRunId::from_bytes([7; 32]),
            policy,
        );
        assert_eq!(core_workflow.stage(), RegistrationStage::Ready);
    }

    #[test]
    fn durable_acknowledgement_is_phase_and_workflow_bound() {
        let environment = mock_environment(4443).expect("synthetic environment");
        let identity = RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]);
        let policy = RegistrationPolicy::new(2, 100).expect("bounded policy");
        let mut original = workflow(
            &environment,
            identity,
            RegistrationRunId::from_bytes([7; 32]),
            policy,
        );
        assert_eq!(original.pending_persistence(), None);
        let attempt = original
            .begin(RegistrationOperation::RequestCode, 10)
            .expect("reserve");
        original
            .complete(attempt, RegistrationOutcome::Verified, 11)
            .expect("verify");
        let commit: whatsapp_rust::mobile::RegistrationCommitToken = original
            .pending_persistence()
            .expect("final durability token");

        let encoded =
            serde_json::to_vec(&original.checkpoint()).expect("serialize verified checkpoint");
        let mut restored = RegistrationWorkflow::restore(
            serde_json::from_slice(&encoded).expect("deserialize"),
            environment.binding(),
            identity,
        )
        .expect("restore verified state");
        assert_eq!(restored.pending_persistence(), Some(commit));
        let mut foreign = workflow(
            &environment,
            identity,
            RegistrationRunId::from_bytes([8; 32]),
            policy,
        );
        let other_attempt = foreign
            .begin(RegistrationOperation::RequestCode, 10)
            .expect("reserve foreign");
        foreign
            .complete(other_attempt, RegistrationOutcome::Verified, 11)
            .expect("verify foreign");
        assert_eq!(
            restored.confirm_persisted(
                foreign
                    .pending_persistence()
                    .expect("foreign durability token")
            ),
            Err(RegistrationError::StaleCompletion)
        );
        assert_eq!(restored.stage(), RegistrationStage::Verified);
        restored
            .confirm_persisted(commit)
            .expect("host confirms its matching durable write");
        assert_eq!(restored.stage(), RegistrationStage::Registered);
        assert_eq!(restored.pending_persistence(), None);
        assert_eq!(
            restored.confirm_persisted(commit),
            Err(RegistrationError::InvalidTransition)
        );
    }

    #[cfg(feature = "mobile-transport-native")]
    #[test]
    fn portable_hosts_are_accepted_consistently_by_the_native_adapter() {
        use whatsapp_rust::transport::TokioTcpTransportFactory;
        for (host, valid) in [
            ("barback", true),
            ("bartender_mobile", true),
            ("fixture.test.", true),
            ("127.0.0.1", true),
            ("[::1]", true),
            ("[2001:db8::1]", true),
            ("[v1.fixture]", false),
            ("fixture%2etest", false),
            ("fixture!name", false),
            ("fixture+name", false),
            ("fixture@name", false),
            ("host/path", false),
        ] {
            assert_eq!(
                TcpEndpoint::new(host, 443).is_ok(),
                valid,
                "portable host: {host}"
            );
            assert_eq!(
                TokioTcpTransportFactory::new(host, 443).is_ok(),
                valid,
                "native host: {host}"
            );
        }
    }

    #[cfg(feature = "mobile-transport-native")]
    #[test]
    fn native_adapter_is_optional_and_does_not_dial_on_construction() {
        let environment = mock_environment(4443).expect("synthetic environment");
        let _factory = native_transport(&environment).expect("construct optional adapter");
    }
}
