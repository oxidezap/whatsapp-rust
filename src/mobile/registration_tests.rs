use super::*;
use async_trait::async_trait;
use futures::channel::oneshot;
use rand::SeedableRng;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use wacore::libsignal::protocol::KeyPair;
use wacore::mobile::registration_wire::{
    Base64Padding, RegistrationAppVersion, RegistrationCommonFields, RegistrationEncoding,
    RegistrationHeaderProfile, RegistrationIdentityFields, RegistrationPlatform,
};
use wacore::mobile::{MobileTrust, RegistrationEndpoint, TcpEndpoint};
use wacore::runtime::AbortHandle;
use wacore::store::{InMemoryBackend, traits::DeviceStore};
use wacore::time::{Instant, VirtualClock};

struct TestRuntime;
#[async_trait]
impl Runtime for TestRuntime {
    fn spawn(&self, _: BoxFuture<'static, ()>) -> AbortHandle {
        panic!("driver must not spawn")
    }
    fn sleep(&self, _: Duration) -> BoxFuture<'static, ()> {
        Box::pin(futures::future::pending())
    }
    fn spawn_blocking(&self, _: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
        panic!("driver must not perform blocking I/O")
    }
    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        None
    }
}
struct TestProof;
impl RegistrationAttestation for TestProof {
    fn attest<'a>(
        &'a self,
        encrypted_body: &'a [u8],
    ) -> BoxFuture<'a, Result<RegistrationAttestationValue, MobileRegistrationError>> {
        Box::pin(async move {
            assert!(!encrypted_body.is_empty());
            Ok(RegistrationAttestationValue {
                body_attestation: Some(Zeroizing::new("synthetic-proof".into())),
                authorization: None,
            })
        })
    }
}
enum Reply {
    Ready(&'static [u8]),
    Deferred(oneshot::Receiver<&'static [u8]>),
    DeferredStatus(u16, oneshot::Receiver<&'static [u8]>),
    Failure,
}
struct MockHttp {
    backend: Arc<InMemoryBackend>,
    replies: Mutex<VecDeque<Reply>>,
    calls: AtomicUsize,
}
#[async_trait]
impl HttpClient for MockHttp {
    async fn execute(&self, request: HttpRequest) -> anyhow::Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let durable = self
            .backend
            .load()
            .await?
            .expect("keys durable before network");
        let checkpoint: RegistrationCheckpoint = serde_json::from_slice(
            durable
                .mobile_session
                .as_ref()
                .unwrap()
                .registration_checkpoint
                .as_ref()
                .unwrap(),
        )
        .unwrap();
        let restored = RegistrationWorkflow::restore(
            checkpoint,
            environment().binding(),
            RegistrationIdentity::from_public_keys(
                durable.identity_key.public_key.public_key_bytes(),
                durable.noise_key.public_key.public_key_bytes(),
            ),
        )?;
        assert!(matches!(
            restored.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert!(request.url.starts_with("http://mock.invalid/mobile/"));
        assert!(request.body.as_ref().unwrap().starts_with(b"ENC="));
        assert_eq!(
            request.headers.get("request_token").map(String::as_str),
            Some(UUID)
        );
        let reply = self.replies.lock().unwrap().pop_front().unwrap();
        let (status, bytes) = match reply {
            Reply::Ready(bytes) => (200, bytes),
            Reply::Deferred(receiver) => (200, receiver.await?),
            Reply::DeferredStatus(status, receiver) => (status, receiver.await?),
            Reply::Failure => anyhow::bail!("synthetic transport failure"),
        };
        Ok(HttpResponse::new(status, bytes.to_vec()))
    }
}
const UUID: &str = "00000000-0000-4000-8000-000000000001";
const POLICY: AttestationPolicy = AttestationPolicy {
    require_body_attestation: true,
    require_authorization: false,
};
fn environment() -> MobileEnvironment {
    MobileEnvironment::new(
        "synthetic",
        RegistrationEndpoint::for_test_http("http://mock.invalid/mobile").unwrap(),
        TcpEndpoint::new("mock.invalid", 5222).unwrap(),
        MobileTrust::new([9; 32], [7; 32]).unwrap(),
    )
    .unwrap()
}
struct Fixture {
    keys: MobileDeviceKeys,
    noise: [u8; 32],
    identity: [u8; 32],
    prekey: [u8; 32],
    header: RegistrationHeaderProfile,
    encoding: RegistrationEncoding,
    fields: RegistrationForm,
}
impl Fixture {
    fn new() -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(773);
        let identity = KeyPair::generate(&mut rng);
        let noise = KeyPair::generate(&mut rng);
        let prekey = KeyPair::generate(&mut rng);
        let signature = identity
            .private_key
            .calculate_signature(&prekey.public_key.serialize(), &mut rng)
            .unwrap();
        Self {
            noise: *noise.public_key.public_key_bytes(),
            identity: *identity.public_key.public_key_bytes(),
            prekey: *prekey.public_key.public_key_bytes(),
            keys: MobileDeviceKeys {
                registration_id: 123,
                noise_key: noise,
                identity_key: identity,
                signed_pre_key: prekey,
                signed_pre_key_id: 1,
                signed_pre_key_signature: signature,
                adv_secret_key: [6; 32],
            },
            header: RegistrationHeaderProfile::new(
                RegistrationPlatform::Android,
                &RegistrationAppVersion::new(&[2, 26, 1, 1]).unwrap(),
                "15",
                "Synthetic",
                "Test",
                vec![],
            )
            .unwrap(),
            encoding: RegistrationEncoding {
                signal_padding: Base64Padding::Padded,
                token_on_register: true,
                mirror_code_and_register_query: false,
                code_order: vec![],
                register_order: vec![],
            },
            fields: RegistrationForm::new(),
        }
    }
    fn builder(&self) -> RegistrationRequestBuilder<'_> {
        self.builder_with_access_session("synthetic-session")
    }
    fn builder_with_access_session<'a>(
        &'a self,
        access_session_id: &'a str,
    ) -> RegistrationRequestBuilder<'a> {
        RegistrationRequestBuilder::new(
            &self.header,
            &self.encoding,
            RegistrationCommonFields {
                country_code: "999",
                national_number: "01000000",
                release_channel: 0,
                language: "en",
                locale_country: "ZZ",
                family_device_id: UUID,
                access_session_id,
                registration_token: Some("synthetic-token"),
                business_certificate: None,
                platform_fields: &self.fields,
                identity: RegistrationIdentityFields {
                    noise_public_key: &self.noise,
                    registration_id: self.keys.registration_id,
                    identity_public_key: &self.identity,
                    signed_prekey_id: 1,
                    signed_prekey_public: &self.prekey,
                    signed_prekey_signature: &self.keys.signed_pre_key_signature,
                    experience_id: &[5; 16],
                    identity_id: b"synthetic-id",
                },
            },
        )
        .unwrap()
    }
    async fn client(&self, replies: Vec<Reply>) -> (MobileRegistrationClient, Arc<MockHttp>) {
        let backend = Arc::new(InMemoryBackend::new());
        let http = Arc::new(MockHttp {
            backend: backend.clone(),
            replies: Mutex::new(replies.into()),
            calls: AtomicUsize::new(0),
        });
        let client = MobileRegistrationClient::initialize(
            backend,
            self.keys.clone(),
            &self.builder(),
            environment(),
            StoredMobilePlatform::Android,
            RegistrationRunId::from_bytes([12; 32]),
            RegistrationPolicy::new(10, 1000).unwrap(),
            http.clone(),
            Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap()),
            Arc::new(TestRuntime),
        )
        .await
        .unwrap();
        (client, http)
    }
}
#[tokio::test]
async fn wire_workflow_persists_keys_before_dispatch_and_identity_before_success() {
    {
        let fixture = Fixture::new();
        let (mut client, http) = fixture
            .client(vec![
                Reply::Ready(br#"{"status":"fail","reason":"incorrect"}"#),
                Reply::Ready(br#"{"status":"sent","sms_wait":5}"#),
                Reply::Ready(br#"{"status":"ok","login":"99901000001"}"#),
            ])
            .await;
        let builder = fixture.builder();
        let lookup = client
            .check_account(
                &builder,
                Some(UUID),
                EphemeralRegistrationKey::from_bytes([21; 32]),
                POLICY,
                &TestProof,
            )
            .await
            .unwrap();
        assert_eq!(
            lookup.decision,
            RegistrationResponseDecision::AccountKeysDiffer
        );
        assert_eq!(lookup.stage, RegistrationStage::Ready);
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        client
            .request_code(
                &builder,
                VerificationMethod::Sms,
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([22; 32]),
                POLICY,
                &TestProof,
            )
            .await
            .unwrap();
        assert_eq!(client.stage(), RegistrationStage::AwaitingCode);
        assert!(matches!(
            client
                .request_code(
                    &builder,
                    VerificationMethod::Sms,
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([28; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::Workflow(
                RegistrationError::Cooldown
            ))
        ));
        assert_eq!(http.calls.load(Ordering::SeqCst), 2);
        let complete = client
            .submit_code(
                &builder,
                "123456",
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([23; 32]),
                POLICY,
                &TestProof,
            )
            .await
            .unwrap();
        assert_eq!(complete.stage, RegistrationStage::Registered);
        let account = client.registered_account().unwrap();
        assert_eq!(account.canonical_phone_number(), "99901000001");
        assert_eq!(account.material().login_metadata().family_device_id, UUID);
        assert_eq!(account.environment().binding(), environment().binding());
        struct NoEntropyDuringFactory;
        impl crate::mobile_session::MobileEphemeralKeyProvider for NoEntropyDuringFactory {
            fn generate(&self) -> Result<KeyPair, wacore::mobile::MobileSessionError> {
                panic!("factory must not acquire entropy")
            }
        }
        let login_profile = wacore::mobile::MobileLoginProfile::new(
            wacore::mobile::MobilePlatform::Android,
            wacore::mobile::MobileAppVersion::parse("2.26.1.1").unwrap(),
            wacore::mobile::MobileDevice::builder()
                .manufacturer("Synthetic".into())
                .device("Test".into())
                .os_version("15".into())
                .os_build_number("synthetic-build".into())
                .phone_id(UUID.to_ascii_uppercase())
                .locale_language("en".into())
                .locale_country("ZZ".into())
                .build(),
        )
        .unwrap();
        let session = crate::mobile_session::RegisteredMobileSession::from_registered_account(
            &account,
            login_profile,
            wacore::mobile::MobileLoginOptions::default(),
            wacore::mobile::MobileNoisePrologue::new(0),
            Arc::new(NoEntropyDuringFactory),
            crate::mobile_session::RegistrationLoginPhoneIdMapping::FdidUppercase,
        )
        .unwrap()
        .with_primary_identity_policy(wacore::send::PrimaryDeviceIdentityPolicy::Omit);
        assert_eq!(session.username(), 99901000001);
        assert_eq!(session.environment().binding(), environment().binding());
        session
            .validate_device(&client.store.get_device_snapshot().core)
            .unwrap();
        let durable = http.backend.load().await.unwrap().unwrap();
        assert_eq!(durable.pn.unwrap().user, "99901000001");
        let restored = MobileRegistrationClient::restore(
            client.store.clone(),
            &builder,
            environment(),
            http.clone(),
            client.clock.clone(),
            Arc::new(TestRuntime),
        )
        .unwrap();
        assert_eq!(restored.stage(), RegistrationStage::Registered);
        let (restored, material) = MobileRegistrationClient::restore_persisted(
            client.store.clone(),
            environment(),
            http.clone(),
            client.clock.clone(),
            Arc::new(TestRuntime),
        )
        .unwrap();
        assert_eq!(restored.stage(), RegistrationStage::Registered);
        assert_eq!(
            material.builder().unwrap().context_fingerprint().unwrap(),
            builder.context_fingerprint().unwrap()
        );
        assert_eq!(http.calls.load(Ordering::SeqCst), 3);
    }
}
#[tokio::test]
async fn canceled_exchange_resumes_without_another_dispatch() {
    {
        let fixture = Fixture::new();
        let (sender, receiver) = oneshot::channel();
        let (mut client, http) = fixture.client(vec![Reply::Deferred(receiver)]).await;
        let builder = fixture.builder();
        let mut attempt = Box::pin(client.request_code(
            &builder,
            VerificationMethod::Sms,
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([24; 32]),
            POLICY,
            &TestProof,
        ));
        assert!(futures::poll!(&mut attempt).is_pending());
        drop(attempt);
        assert!(matches!(
            client.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        let mut restarted = MobileRegistrationClient::restore(
            client.store.clone(),
            &builder,
            environment(),
            http.clone(),
            client.clock.clone(),
            Arc::new(TestRuntime),
        )
        .unwrap();
        assert!(restarted.recovery_required());
        assert!(matches!(
            restarted
                .check_account(
                    &builder,
                    Some(UUID),
                    EphemeralRegistrationKey::from_bytes([56; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::DeliveryUnknown)
        ));
        assert!(matches!(
            client
                .check_account(
                    &builder,
                    Some(UUID),
                    EphemeralRegistrationKey::from_bytes([25; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::DeliveryUnknown)
        ));
        sender.send(br#"{"status":"sent"}"#).unwrap();
        assert_eq!(
            client.resume_pending().await.unwrap().unwrap().stage,
            RegistrationStage::AwaitingCode
        );
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn transport_failure_never_requests_an_otp_or_reports_fresh_account() {
    {
        let fixture = Fixture::new();
        let (mut client, http) = fixture.client(vec![Reply::Failure]).await;
        assert!(matches!(
            client
                .check_account(
                    &fixture.builder(),
                    Some(UUID),
                    EphemeralRegistrationKey::from_bytes([26; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::DeliveryUnknown)
        ));
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(
            client.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert!(matches!(
            client.resume_pending().await,
            Err(MobileRegistrationError::DeliveryUnknown)
        ));
    }
}
#[tokio::test]
async fn challenge_and_pin_are_explicit_paths_and_profile_drift_is_rejected() {
    {
        let mut fixture = Fixture::new();
        let (mut client, http) = fixture
            .client(vec![
                Reply::Ready(br#"{"status":"fail","image_blob":"AQI="}"#),
                Reply::Ready(br#"{"status":"fail","reason":"2fa_required"}"#),
                Reply::Ready(br#"{"status":"ok","login":"99901000000"}"#),
            ])
            .await;
        {
            let builder = fixture.builder();
            let result = client
                .request_code(
                    &builder,
                    VerificationMethod::Voice,
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([31; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(result.stage, RegistrationStage::AwaitingChallenge);
            assert_eq!(
                result.response.challenge().unwrap().image.as_deref(),
                Some([1, 2].as_slice())
            );
            let result = client
                .submit_challenge(
                    &builder,
                    "synthetic-answer",
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([32; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(result.stage, RegistrationStage::AwaitingPin);
            let result = client
                .submit_pin(
                    &builder,
                    "123456",
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([33; 32]),
                    POLICY,
                    &TestProof,
                )
                .await
                .unwrap();
            assert_eq!(result.stage, RegistrationStage::Registered);
        }
        fixture.keys.registration_id += 1;
        assert!(matches!(
            MobileRegistrationClient::restore(
                client.store.clone(),
                &fixture.builder(),
                environment(),
                http.clone(),
                client.clock.clone(),
                Arc::new(TestRuntime)
            ),
            Err(MobileRegistrationError::BindingMismatch)
        ));
        assert_eq!(http.calls.load(Ordering::SeqCst), 3);
    }
}

#[tokio::test]
async fn external_code_and_unsent_attestation_failure_have_no_http_side_effect() {
    struct FailedProof;
    impl RegistrationAttestation for FailedProof {
        fn attest<'a>(
            &'a self,
            _: &'a [u8],
        ) -> BoxFuture<'a, Result<RegistrationAttestationValue, MobileRegistrationError>> {
            Box::pin(async { Err(MobileRegistrationError::Attestation) })
        }
    }
    {
        let fixture = Fixture::new();
        let (mut client, http) = fixture.client(vec![]).await;
        assert!(matches!(
            client
                .check_account(
                    &fixture.builder(),
                    Some(UUID),
                    EphemeralRegistrationKey::from_bytes([34; 32]),
                    POLICY,
                    &FailedProof
                )
                .await,
            Err(MobileRegistrationError::Attestation)
        ));
        assert_eq!(client.stage(), RegistrationStage::Ready);
        client.await_external_code().await.unwrap();
        assert_eq!(client.stage(), RegistrationStage::AwaitingCode);
        assert!(matches!(
            client.registered_account(),
            Err(MobileRegistrationError::Workflow(
                RegistrationError::InvalidTransition
            ))
        ));
        assert!(client.verification_delivery().is_none());
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        let restored = MobileRegistrationClient::restore(
            client.store.clone(),
            &fixture.builder(),
            environment(),
            http,
            client.clock.clone(),
            Arc::new(TestRuntime),
        )
        .unwrap();
        assert_eq!(restored.stage(), RegistrationStage::AwaitingCode);
    }
}
#[tokio::test]
async fn invalid_otp_can_be_corrected_but_zero_login_cannot_be_committed() {
    {
        let fixture = Fixture::new();
        let (mut client, http) = fixture
            .client(vec![
                Reply::Ready(br#"{"status":"fail","reason":"bad_code"}"#),
                Reply::Ready(br#"{"status":"ok","login":"0"}"#),
            ])
            .await;
        client.await_external_code().await.unwrap();
        let builder = fixture.builder();
        let rejected = client
            .submit_code(
                &builder,
                "111111",
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([35; 32]),
                POLICY,
                &TestProof,
            )
            .await
            .unwrap();
        assert_eq!(rejected.decision, RegistrationResponseDecision::Rejected);
        assert_eq!(rejected.stage, RegistrationStage::AwaitingCode);
        assert!(matches!(
            client
                .submit_code(
                    &builder,
                    "123456",
                    Some(UUID),
                    &fixture.fields,
                    EphemeralRegistrationKey::from_bytes([36; 32]),
                    POLICY,
                    &TestProof
                )
                .await,
            Err(MobileRegistrationError::MissingIdentity)
        ));
        assert!(matches!(
            client.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert!(http.backend.load().await.unwrap().unwrap().pn.is_none());
    }
}
async fn gate_device_saves(
    client: &mut MobileRegistrationClient,
    backend: Arc<InMemoryBackend>,
) -> async_channel::Sender<()> {
    let (mut probe, _) = crate::store::release::tests::ProbeBackend::new();
    let (unpark, gate) = async_channel::bounded(2);
    probe.backend = backend;
    probe.gate = Some(gate);
    probe.gate_method = "save";
    client.store = Arc::new(PersistenceManager::new(Arc::new(probe)).await.unwrap());
    unpark
}

#[tokio::test]
async fn canceled_pre_send_checkpoint_never_dispatches_and_retains_attempt_budget() {
    {
        let fixture = Fixture::new();
        let (mut client, http) = fixture.client(vec![]).await;
        let unpark = gate_device_saves(&mut client, http.backend.clone()).await;
        let builder = fixture.builder();
        let mut request = Box::pin(client.check_account(
            &builder,
            Some(UUID),
            EphemeralRegistrationKey::from_bytes([37; 32]),
            POLICY,
            &TestProof,
        ));
        assert!(futures::poll!(&mut request).is_pending());
        drop(request);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        assert!(client.recovery_required());
        assert!(matches!(
            client.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        unpark.send(()).await.unwrap();
        unpark.send(()).await.unwrap();
        assert!(client.resume_pending().await.unwrap().is_none());
        assert_eq!(client.stage(), RegistrationStage::Ready);
        let checkpoint = serde_json::to_value(client.workflow.checkpoint()).unwrap();
        assert_eq!(checkpoint["attempts"], 1);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn canceled_final_commit_cannot_publish_registered_until_retained_save_finishes() {
    {
        let fixture = Fixture::new();
        let (mut client, http) = fixture
            .client(vec![Reply::Ready(
                br#"{"status":"ok","login":"99901000000"}"#,
            )])
            .await;
        client.await_external_code().await.unwrap();
        let unpark = gate_device_saves(&mut client, http.backend.clone()).await;
        unpark.send(()).await.unwrap(); // Only the pre-send checkpoint may complete.
        let builder = fixture.builder();
        let mut request = Box::pin(client.submit_code(
            &builder,
            "123456",
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([38; 32]),
            POLICY,
            &TestProof,
        ));
        assert!(futures::poll!(&mut request).is_pending());
        drop(request);
        assert!(matches!(
            client.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert!(http.backend.load().await.unwrap().unwrap().pn.is_none());
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
        unpark.send(()).await.unwrap();
        let result = client.resume_pending().await.unwrap().unwrap();
        assert_eq!(result.stage, RegistrationStage::Registered);
        assert!(http.backend.load().await.unwrap().unwrap().pn.is_some());
        assert_eq!(http.calls.load(Ordering::SeqCst), 1);
    }
}
#[tokio::test]
async fn verified_without_login_retains_validated_durable_submitted_number() {
    let fixture = Fixture::new();
    let (mut client, http) = fixture
        .client(vec![Reply::Ready(br#"{"status":"verified"}"#)])
        .await;
    client.await_external_code().await.unwrap();
    let result = client
        .submit_code(
            &fixture.builder(),
            "123456",
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([39; 32]),
            POLICY,
            &TestProof,
        )
        .await
        .unwrap();
    assert_eq!(result.stage, RegistrationStage::Registered);
    assert_eq!(
        http.backend.load().await.unwrap().unwrap().pn.unwrap().user,
        "99901000000"
    );
}
#[tokio::test]
async fn timeout_retains_exchange_and_recovery_rejects_unrelated_publication() {
    struct ElapsedTimer;
    #[async_trait]
    impl Runtime for ElapsedTimer {
        fn spawn(&self, _: BoxFuture<'static, ()>) -> AbortHandle {
            panic!("unexpected spawn")
        }
        fn sleep(&self, _: Duration) -> BoxFuture<'static, ()> {
            Box::pin(async {})
        }
        fn spawn_blocking(&self, _: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
            panic!("unexpected I/O")
        }
        fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
            None
        }
    }
    let fixture = Fixture::new();
    let (sender, receiver) = oneshot::channel();
    let (mut client, http) = fixture.client(vec![Reply::Deferred(receiver)]).await;
    client.runtime = Arc::new(ElapsedTimer);
    let builder = fixture.builder();
    assert!(matches!(
        client
            .request_code(
                &builder,
                VerificationMethod::Sms,
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([40; 32]),
                POLICY,
                &TestProof
            )
            .await,
        Err(MobileRegistrationError::DeliveryUnknown)
    ));
    sender.send(br#"{"status":"sent"}"#).unwrap();
    assert_eq!(
        client.resume_pending().await.unwrap().unwrap().stage,
        RegistrationStage::AwaitingCode
    );
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);

    let (sender, receiver) = oneshot::channel();
    let (mut client, http) = fixture.client(vec![Reply::Deferred(receiver)]).await;
    let mut request = Box::pin(client.request_code(
        &builder,
        VerificationMethod::Sms,
        Some(UUID),
        &fixture.fields,
        EphemeralRegistrationKey::from_bytes([41; 32]),
        POLICY,
        &TestProof,
    ));
    assert!(futures::poll!(&mut request).is_pending());
    drop(request);
    let unrelated = RegistrationWorkflow::new(
        environment().binding(),
        builder.registration_identity(),
        RegistrationRunId::from_bytes([99; 32]),
        RegistrationPolicy::new(10, 1000).unwrap(),
    );
    let mut next = client.binding.clone();
    next.registration_checkpoint = Some(encode(&unrelated).unwrap());
    client
        .store
        .update_mobile_registration(client.binding.clone(), next, None)
        .await
        .unwrap();
    sender.send(br#"{"status":"sent"}"#).unwrap();
    assert!(matches!(
        client.resume_pending().await,
        Err(MobileRegistrationError::BindingMismatch)
    ));
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn retry_after_only_response_persists_cooldown_without_automatic_retry() {
    let fixture = Fixture::new();
    let (mut client, http) = fixture
        .client(vec![Reply::Ready(
            br#"{"status":"fail","reason":"too_recent","retry_after":2}"#,
        )])
        .await;
    let result = client
        .request_code(
            &fixture.builder(),
            VerificationMethod::Voice,
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([42; 32]),
            POLICY,
            &TestProof,
        )
        .await
        .unwrap();
    assert_eq!(result.decision, RegistrationResponseDecision::RateLimited);
    assert_eq!(result.stage, RegistrationStage::Ready);
    assert!(matches!(
        client
            .request_code(
                &fixture.builder(),
                VerificationMethod::Voice,
                Some(UUID),
                &fixture.fields,
                EphemeralRegistrationKey::from_bytes([43; 32]),
                POLICY,
                &TestProof
            )
            .await,
        Err(MobileRegistrationError::Workflow(
            RegistrationError::Cooldown
        ))
    ));
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn flash_delivery_metadata_survives_restart_without_storing_entered_codes() {
    let fixture = Fixture::new();
    let (mut client, http) = fixture.client(vec![Reply::Ready(br#"{"status":"sent","length":6,"cli_filter":"(.*)373(.*)","cli_prefix":"373","cli_cc":"39","flash_timeout":30}"#)]).await;
    client
        .request_code(
            &fixture.builder(),
            VerificationMethod::Flash,
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([44; 32]),
            POLICY,
            &TestProof,
        )
        .await
        .unwrap();
    let (restored, material) = MobileRegistrationClient::restore_persisted(
        client.store.clone(),
        environment(),
        http.clone(),
        client.clock.clone(),
        Arc::new(TestRuntime),
    )
    .unwrap();
    let delivery = restored.verification_delivery().unwrap();
    assert_eq!(delivery.method(), VerificationMethod::Flash);
    assert_eq!(delivery.observed_at_ms(), 1000);
    assert_eq!(delivery.flash_expires_at_ms(), Some(31000));
    assert_eq!(delivery.hints().flash_prefix(), Some("373"));
    assert_eq!(delivery.hints().flash_country_code(), Some("39"));
    assert_eq!(
        &*restored
            .extract_flash_code("+39 373 9799312", None, None)
            .unwrap(),
        "799312"
    );
    assert_eq!(
        &*restored.extract_flash_code("123456", None, None).unwrap(),
        "123456"
    );
    assert_eq!(
        material.builder().unwrap().context_fingerprint().unwrap(),
        fixture.builder().context_fingerprint().unwrap()
    );
    let snapshot = restored.store.get_device_snapshot();
    let context = String::from_utf8(
        snapshot
            .mobile_session
            .as_ref()
            .unwrap()
            .registration_context
            .clone(),
    )
    .unwrap();
    assert!(!context.contains("9799312"));
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn external_delivery_metadata_is_explicit_bounded_and_durable() {
    let fixture = Fixture::new();
    let (mut client, http) = fixture.client(vec![]).await;
    let hints = RegistrationDeliveryHints::new(Some(4), None, None, None, Some(20)).unwrap();
    let future = RegistrationDeliveryContext::new(VerificationMethod::Flash, 1001, hints.clone());
    assert!(matches!(
        client.await_external_code_with_delivery(Some(future)).await,
        Err(MobileRegistrationError::Clock)
    ));
    let declared = RegistrationDeliveryContext::new(VerificationMethod::Flash, 900, hints);
    client
        .await_external_code_with_delivery(Some(declared))
        .await
        .unwrap();
    let (restored, _) = MobileRegistrationClient::restore_persisted(
        client.store.clone(),
        environment(),
        http.clone(),
        client.clock.clone(),
        Arc::new(TestRuntime),
    )
    .unwrap();
    assert_eq!(
        restored.verification_delivery().unwrap().method(),
        VerificationMethod::Flash
    );
    assert_eq!(
        &*restored
            .extract_flash_code("+999 010 1234", None, None)
            .unwrap(),
        "1234"
    );
    assert_eq!(http.calls.load(Ordering::SeqCst), 0);
}

#[test]
fn unknown_persisted_delivery_method_is_rejected_instead_of_defaulting_to_sms() {
    let unknown = serde_json::json!({
        "method": "future_route",
        "observed_at_ms": 1000,
        "hints": {
            "code_length": null,
            "flash_filter": null,
            "flash_prefix": null,
            "flash_country_code": null,
            "flash_timeout_seconds": null
        }
    });
    assert!(serde_json::from_value::<RegistrationDeliveryContext>(unknown).is_err());
}

#[tokio::test]
async fn initialization_reserves_space_for_every_bounded_delivery_hint_before_installing_keys() {
    async fn initialize_size(
        fixture: &Fixture,
        size: usize,
    ) -> (
        Result<MobileRegistrationClient, MobileRegistrationError>,
        Arc<InMemoryBackend>,
    ) {
        let access_session = "s".repeat(size);
        let backend = Arc::new(InMemoryBackend::new());
        let http = Arc::new(MockHttp {
            backend: backend.clone(),
            replies: Mutex::new(VecDeque::new()),
            calls: AtomicUsize::new(0),
        });
        let result = MobileRegistrationClient::initialize(
            backend.clone(),
            fixture.keys.clone(),
            &fixture.builder_with_access_session(&access_session),
            environment(),
            StoredMobilePlatform::Android,
            RegistrationRunId::from_bytes([55; 32]),
            RegistrationPolicy::new(10, 1000).unwrap(),
            http.clone(),
            Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap()),
            Arc::new(TestRuntime),
        )
        .await;
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        if result.is_err() {
            assert!(backend.load().await.unwrap().is_none());
        }
        (result, backend)
    }
    let fixture = Fixture::new();
    let (mut accepted_size, mut rejected_size) = (1, 60 * 1024);
    while accepted_size + 1 < rejected_size {
        let middle = accepted_size + (rejected_size - accepted_size) / 2;
        match initialize_size(&fixture, middle).await.0 {
            Ok(_) => accepted_size = middle,
            Err(MobileRegistrationError::Wire(RegistrationWireError::SizeLimit)) => {
                rejected_size = middle
            }
            Err(error) => panic!("unexpected initialization failure: {error}"),
        }
    }
    let accepted = initialize_size(&fixture, accepted_size).await.0.unwrap();
    let initial_len = accepted.binding.registration_context.len();
    // The fingerprint is a JSON byte array: changing the input changes decimal
    // digit widths as well as base64 length. Adjacent inputs can jump by more
    // than one base64 quantum; this still tests immediately around the cutoff.
    assert!(
        (REGISTRATION_CONTEXT_LIMIT - DYNAMIC_CONTEXT_HEADROOM - 128
            ..=REGISTRATION_CONTEXT_LIMIT - DYNAMIC_CONTEXT_HEADROOM)
            .contains(&initial_len),
        "accepted initial context length: {initial_len}"
    );
    let mut maximum_dynamic = accepted.context.clone();
    maximum_dynamic.code_not_before = [u64::MAX; 5];
    maximum_dynamic.delivery = Some(RegistrationDeliveryContext::new(
        VerificationMethod::Email,
        u64::MAX,
        RegistrationDeliveryHints::new(
            Some(32),
            Some("\"".repeat(128)),
            Some("\"".repeat(64)),
            Some("\"".repeat(64)),
            Some(u32::MAX),
        )
        .unwrap(),
    ));
    assert!(maximum_dynamic.encode().unwrap().len() <= REGISTRATION_CONTEXT_LIMIT);
    assert!(matches!(
        initialize_size(&fixture, rejected_size).await.0,
        Err(MobileRegistrationError::Wire(
            RegistrationWireError::SizeLimit
        ))
    ));
}

#[tokio::test]
async fn restore_cannot_publish_verified_or_registered_without_durable_phone_identity() {
    let fixture = Fixture::new();
    for registered in [false, true] {
        let (client, http) = fixture.client(vec![]).await;
        let builder = fixture.builder();
        let mut inconsistent = RegistrationWorkflow::new(
            environment().binding(),
            builder.registration_identity(),
            RegistrationRunId::from_bytes([57; 32]),
            RegistrationPolicy::new(10, 1000).unwrap(),
        );
        let token = inconsistent
            .begin(RegistrationOperation::CheckAccount, 1000)
            .unwrap();
        inconsistent
            .complete(token, RegistrationOutcome::Verified, 1000)
            .unwrap();
        if registered {
            inconsistent
                .confirm_persisted(inconsistent.pending_persistence().unwrap())
                .unwrap();
        }
        let mut next = client.binding.clone();
        next.registration_checkpoint.zeroize();
        next.registration_checkpoint = Some(encode(&inconsistent).unwrap());
        client
            .store
            .update_mobile_registration(client.binding.clone(), next, None)
            .await
            .unwrap();
        assert!(matches!(
            MobileRegistrationClient::restore(
                client.store.clone(),
                &builder,
                environment(),
                http.clone(),
                client.clock.clone(),
                Arc::new(TestRuntime)
            ),
            Err(MobileRegistrationError::BindingMismatch)
        ));
        assert!(http.backend.load().await.unwrap().unwrap().pn.is_none());
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn durable_registered_checkpoint_restores_and_settles_without_http() {
    let fixture = Fixture::new();
    let (mut client, http) = fixture
        .client(vec![Reply::Ready(br#"{"status":"verified"}"#)])
        .await;
    client.await_external_code().await.unwrap();
    client
        .submit_code(
            &fixture.builder(),
            "123456",
            Some(UUID),
            &fixture.fields,
            EphemeralRegistrationKey::from_bytes([59; 32]),
            POLICY,
            &TestProof,
        )
        .await
        .unwrap();
    assert_eq!(client.stage(), RegistrationStage::Registered);
    let mut registered = client.binding.clone();
    registered.registration_checkpoint.zeroize();
    registered.registration_checkpoint = Some(encode(&client.workflow).unwrap());
    client.publication = Some(registered.clone());
    client
        .store
        .process_command(wacore::store::DeviceCommand::SetMobileSessionBinding(
            registered,
        ))
        .await;
    client.store.flush().await.unwrap();

    assert!(client.resume_pending().await.unwrap().is_none());
    assert_eq!(client.stage(), RegistrationStage::Registered);
    let restored = MobileRegistrationClient::restore(
        client.store.clone(),
        &fixture.builder(),
        environment(),
        http.clone(),
        client.clock.clone(),
        Arc::new(TestRuntime),
    )
    .unwrap();
    assert_eq!(restored.stage(), RegistrationStage::Registered);
    assert_eq!(
        restored
            .registered_account()
            .unwrap()
            .canonical_phone_number(),
        "99901000000"
    );
    let (restored, _) = MobileRegistrationClient::restore_persisted(
        client.store.clone(),
        environment(),
        http.clone(),
        client.clock.clone(),
        Arc::new(TestRuntime),
    )
    .unwrap();
    assert_eq!(restored.stage(), RegistrationStage::Registered);
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn canceled_exchange_retains_http_error_status_without_interpreting_or_resending_it() {
    let fixture = Fixture::new();
    let (sender, receiver) = oneshot::channel();
    let (mut client, http) = fixture
        .client(vec![Reply::DeferredStatus(429, receiver)])
        .await;
    let builder = fixture.builder();
    let mut attempt = Box::pin(client.request_code(
        &builder,
        VerificationMethod::Sms,
        Some(UUID),
        &fixture.fields,
        EphemeralRegistrationKey::from_bytes([60; 32]),
        POLICY,
        &TestProof,
    ));
    assert!(futures::poll!(&mut attempt).is_pending());
    drop(attempt);
    // Even a success-shaped body cannot qualify the non-success HTTP status.
    sender.send(br#"{"status":"sent"}"#).unwrap();
    for _ in 0..2 {
        assert!(matches!(
            client.resume_pending().await,
            Err(MobileRegistrationError::HttpStatus)
        ));
        assert!(matches!(
            client.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
    }
    let mut restarted = MobileRegistrationClient::restore(
        client.store.clone(),
        &builder,
        environment(),
        http.clone(),
        client.clock.clone(),
        Arc::new(TestRuntime),
    )
    .unwrap();
    assert!(matches!(
        restarted.resume_pending().await,
        Err(MobileRegistrationError::DeliveryUnknown)
    ));
    assert_eq!(http.calls.load(Ordering::SeqCst), 1);
}
