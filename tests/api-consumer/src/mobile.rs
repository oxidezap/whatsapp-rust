//! Mobile contracts must be usable by an isolated host with defaults disabled.
//!
//! This probe performs no I/O and does not require an executor, even through the
//! SDK re-export. APK, mock-server and target execution are separate validation.

use whatsapp_rust::mobile::{
    EnvironmentBound, MobileConfigurationError, MobileEnvironment, MobileTrust,
    RegistrationEndpoint, RegistrationIdentity, RegistrationPolicy, RegistrationWorkflow,
    TcpEndpoint,
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
    policy: RegistrationPolicy,
) -> RegistrationWorkflow {
    RegistrationWorkflow::new(environment.binding(), identity, policy)
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
        let mut original = workflow(&environment, identity, policy);
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
            policy,
        );
        assert_eq!(core_workflow.stage(), RegistrationStage::Ready);
    }

    #[cfg(feature = "mobile-transport-native")]
    #[test]
    fn native_adapter_is_optional_and_does_not_dial_on_construction() {
        let environment = mock_environment(4443).expect("synthetic environment");
        let _factory = native_transport(&environment).expect("construct optional adapter");
    }
}
