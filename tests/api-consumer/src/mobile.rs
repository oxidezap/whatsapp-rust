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

#[cfg(feature = "mobile-ports")]
#[path = "mobile_runtime_fixture.rs"]
pub mod runtime_fixture;

/// Execute the public SDK configuration and payload APIs without selecting I/O.
pub fn exercise_login_payload() -> Result<(), Box<dyn std::error::Error>> {
    use std::sync::Arc;
    use whatsapp_rust::mobile::{
        MobileAppVersion, MobileDevice, MobileLoginOptions, MobileLoginProfile,
        MobileNoisePrologue, MobilePlatform, MobileSessionError,
    };
    use whatsapp_rust::{
        MobileEphemeralKeyProvider, PrimaryDeviceIdentityPolicy, RegisteredMobileSession,
    };

    struct NoConnectionEntropy;
    impl MobileEphemeralKeyProvider for NoConnectionEntropy {
        fn generate(&self) -> Result<wacore::libsignal::protocol::KeyPair, MobileSessionError> {
            Err(MobileSessionError::InvalidKeyMaterial)
        }
    }

    let profile = MobileLoginProfile::new(
        MobilePlatform::Android,
        MobileAppVersion::parse("2.26.1.7")?,
        MobileDevice::builder()
            .manufacturer("Synthetic".into())
            .device("Fixture".into())
            .os_version("15".into())
            .os_build_number("fixture-build".into())
            .phone_id("synthetic-installation".into())
            .locale_language("en".into())
            .locale_country("ZZ".into())
            .build(),
    )?;
    let options = MobileLoginOptions::builder()
        .device(0)
        .passive(false)
        .pull(false)
        .build()
        .with_session_id(Some(42))
        .with_connect_attempt_count(Some(3))
        .with_login_counter(Some(7));
    let payload = profile.build_payload(99910000000, &options)?;
    assert_eq!(payload.username, Some(99910000000));
    assert_eq!(payload.device, Some(0));
    assert_eq!(payload.session_id, Some(42));
    assert_eq!(payload.connect_attempt_count, Some(3));
    assert_eq!(payload.lc, Some(7));
    assert_eq!(payload.passive, Some(false));
    assert_eq!(payload.pull, Some(false));
    assert!(payload.web_info.is_unset());
    let encoded = profile.encode_payload(99910000000, &options)?;
    let session = RegisteredMobileSession::new(
        mock_environment(4443)?,
        profile,
        options,
        99910000000,
        RegistrationIdentity::from_public_keys(&[3; 32], &[4; 32]),
        MobileNoisePrologue::new(3),
        Arc::new(NoConnectionEntropy),
    )?
    .with_primary_identity_policy(PrimaryDeviceIdentityPolicy::Omit);
    assert_eq!(session.encode_payload()?, encoded);
    Ok(())
}

/// Cross-target crypto golden generated independently with Node and RFC 7748 keys.
/// The fixed secrets and fictitious telephone fields are for this probe only.
pub fn exercise_registration_envelope() -> Result<(), Box<dyn std::error::Error>> {
    use whatsapp_rust::mobile::registration_wire::{
        EphemeralRegistrationKey, RegistrationWireError,
    };

    fn bytes(value: &str) -> Result<[u8; 32], std::num::ParseIntError> {
        let mut result = [0; 32];
        for (index, byte) in result.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)?;
        }
        Ok(result)
    }
    let secret = bytes("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a")?;
    let recipient = bytes("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")?;
    let envelope =
        EphemeralRegistrationKey::from_bytes(secret).seal(&recipient, b"cc=999&in=10000000")?;
    assert_eq!(
        envelope.encoded(),
        "hSDwCYkwp1R0i33ctD73Wg2_Og0mOBr066SpjqqbTmqzyqIiPBXyIaX7wcy1xwrck5Lj4_ZPR5frJIN2D73fK7Or"
    );
    assert!(matches!(
        EphemeralRegistrationKey::from_bytes([3; 32]).seal(&[0; 32], b"cc=999"),
        Err(RegistrationWireError::InvalidServerKey)
    ));
    Ok(())
}

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
    fn public_mobile_payload_and_crypto_execute_without_native_ports() {
        exercise_login_payload().expect("synthetic public configuration");
        exercise_registration_envelope().expect("independent synthetic vector");
    }

    #[cfg(feature = "mobile-ports")]
    #[test]
    fn registration_uses_only_supplied_host_ports() {
        futures::executor::block_on(runtime_fixture::exercise_durable_registration())
            .expect("synthetic registration and durable restore");
    }

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
