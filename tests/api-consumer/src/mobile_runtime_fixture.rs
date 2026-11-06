//! Synthetic downstream SDK exercise. Every service is an explicitly supplied port.
//! No transport, native executor, APK, real telephone account or SMS is involved.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rand::SeedableRng as _;
use wacore::libsignal::protocol::KeyPair;
use wacore::net::{HttpClient, HttpRequest, HttpResponse};
use wacore::runtime::{AbortHandle, BoxFuture, Runtime};
use wacore::store::traits::DeviceStore as _;
use wacore::store::{InMemoryBackend, MobileDeviceKeys, StoredMobilePlatform};
use wacore::time::{Clock, Instant, VirtualClock};
use whatsapp_rust::mobile::registration_wire::{
    AttestationPolicy, Base64Padding, EphemeralRegistrationKey, RegistrationAppVersion,
    RegistrationCommonFields, RegistrationEncoding, RegistrationForm, RegistrationHeaderProfile,
    RegistrationIdentityFields, RegistrationPlatform, RegistrationRequestBuilder,
    VerificationMethod,
};
use whatsapp_rust::mobile::{
    MobileAppVersion, MobileDevice, MobileLoginOptions, MobileLoginProfile, MobileNoisePrologue,
    MobilePlatform, MobileSessionError, RegistrationError, RegistrationPolicy, RegistrationRunId,
    RegistrationStage,
};
use whatsapp_rust::mobile_registration::{
    MobileRegistrationClient, MobileRegistrationError, RegistrationAttestation,
    RegistrationAttestationValue,
};
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::{
    MobileEphemeralKeyProvider, PrimaryDeviceIdentityPolicy, RegisteredMobileSession,
    RegistrationLoginPhoneIdMapping,
};

const INSTALLATION: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const REQUEST_TOKEN: &str = "00000000-0000-4000-8000-000000000011";

struct SuppliedRuntime {
    clock: Arc<dyn Clock>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Runtime for SuppliedRuntime {
    fn clock(&self) -> Option<&Arc<dyn Clock>> {
        Some(&self.clock)
    }

    fn spawn(&self, _: BoxFuture<'static, ()>) -> AbortHandle {
        panic!("registration must not start an executor task")
    }

    fn sleep(&self, _: Duration) -> BoxFuture<'static, ()> {
        Box::pin(futures::future::pending())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_blocking(&self, _: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
        panic!("registration must not use native blocking I/O")
    }

    #[cfg(target_arch = "wasm32")]
    fn spawn_blocking(&self, _: Box<dyn FnOnce() + 'static>) -> BoxFuture<'static, ()> {
        panic!("registration must not use native blocking I/O")
    }

    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        None
    }
}

struct SuppliedAttestation;
impl RegistrationAttestation for SuppliedAttestation {
    fn attest<'a>(
        &'a self,
        encrypted_body: &'a [u8],
    ) -> BoxFuture<'a, Result<RegistrationAttestationValue, MobileRegistrationError>> {
        Box::pin(async move {
            assert!(!encrypted_body.is_empty());
            // A fabricated callback result exercises the required-proof boundary.
            Ok(RegistrationAttestationValue {
                body_attestation: Some("synthetic-proof".to_owned().into()),
                authorization: None,
            })
        })
    }
}

struct SuppliedHttp {
    backend: Arc<InMemoryBackend>,
    replies: Mutex<VecDeque<(&'static str, &'static [u8])>>,
    calls: AtomicUsize,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl HttpClient for SuppliedHttp {
    async fn execute(&self, request: HttpRequest) -> anyhow::Result<HttpResponse> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let durable = self
            .backend
            .load()
            .await?
            .expect("keys durable before HTTP");
        let binding = durable
            .mobile_session
            .as_ref()
            .expect("durable phone binding");
        let checkpoint: whatsapp_rust::mobile::RegistrationCheckpoint = serde_json::from_slice(
            binding
                .registration_checkpoint
                .as_ref()
                .expect("durable reservation"),
        )?;
        let identity = whatsapp_rust::mobile::RegistrationIdentity::from_public_keys(
            durable.identity_key.public_key.public_key_bytes(),
            durable.noise_key.public_key.public_key_bytes(),
        );
        let restored = whatsapp_rust::mobile::RegistrationWorkflow::restore(
            checkpoint,
            super::mock_environment(4443)?.binding(),
            identity,
        )?;
        assert!(matches!(
            restored.stage(),
            RegistrationStage::OutcomeUnknown { .. }
        ));
        assert!(request.url.starts_with("http://mock.test:8080/v2/"));
        assert!(
            request
                .body
                .as_ref()
                .expect("encrypted envelope")
                .starts_with(b"ENC=")
        );
        assert_eq!(
            request.headers.get("request_token").map(String::as_str),
            Some(REQUEST_TOKEN)
        );
        let (suffix, reply) = self
            .replies
            .lock()
            .expect("synthetic reply queue")
            .pop_front()
            .expect("no implicit registration request");
        assert!(request.url.ends_with(suffix));
        Ok(HttpResponse::new(200, reply.to_vec()))
    }
}

struct NoConnectionEntropy;
impl MobileEphemeralKeyProvider for NoConnectionEntropy {
    fn generate(&self) -> Result<KeyPair, MobileSessionError> {
        Err(MobileSessionError::InvalidKeyMaterial)
    }
}

/// Execute lookup, explicit SMS, verification, in-memory backend reload and session handoff.
/// Fabricated replies do not establish server acceptance or process-crash persistence.
pub async fn exercise_durable_registration() -> Result<(), Box<dyn std::error::Error>> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(773);
    let identity = KeyPair::generate(&mut rng);
    let noise = KeyPair::generate(&mut rng);
    let prekey = KeyPair::generate(&mut rng);
    let signature = identity
        .private_key
        .calculate_signature(&prekey.public_key.serialize(), &mut rng)?;
    let noise_public = *noise.public_key.public_key_bytes();
    let identity_public = *identity.public_key.public_key_bytes();
    let prekey_public = *prekey.public_key.public_key_bytes();
    let keys = MobileDeviceKeys {
        registration_id: 123,
        noise_key: noise,
        identity_key: identity,
        signed_pre_key: prekey,
        signed_pre_key_id: 1,
        signed_pre_key_signature: signature,
        adv_secret_key: [6; 32],
    };
    let header = RegistrationHeaderProfile::new(
        RegistrationPlatform::Android,
        &RegistrationAppVersion::new(&[2, 26, 1, 7])?,
        "15",
        "Synthetic",
        "fixture-model-id",
        vec![],
    )?;
    let encoding = RegistrationEncoding {
        signal_padding: Base64Padding::Padded,
        token_on_register: true,
        mirror_code_and_register_query: false,
        code_order: vec![],
        register_order: vec![],
    };
    let extras = RegistrationForm::new();
    let builder = RegistrationRequestBuilder::new(
        &header,
        &encoding,
        RegistrationCommonFields {
            country_code: "999",
            national_number: "10000000",
            release_channel: 0,
            language: "en",
            locale_country: "ZZ",
            family_device_id: INSTALLATION,
            access_session_id: "synthetic-session",
            registration_token: Some("synthetic-token"),
            business_certificate: None,
            platform_fields: &extras,
            identity: RegistrationIdentityFields {
                noise_public_key: &noise_public,
                registration_id: 123,
                identity_public_key: &identity_public,
                signed_prekey_id: 1,
                signed_prekey_public: &prekey_public,
                signed_prekey_signature: &signature,
                experience_id: &[5; 16],
                identity_id: b"synthetic-id",
            },
        },
    )?;
    let clock: Arc<dyn Clock> = Arc::new(VirtualClock::new(1000, Instant::ZERO)?);
    let runtime: Arc<dyn Runtime> = Arc::new(SuppliedRuntime {
        clock: clock.clone(),
    });
    let backend = Arc::new(InMemoryBackend::new().with_clock(clock.clone()));
    let http = Arc::new(SuppliedHttp {
        backend: backend.clone(),
        replies: Mutex::new(VecDeque::from([
            ("/exist", &br#"{"status":"fail","reason":"incorrect"}"#[..]),
            ("/code", &br#"{"status":"sent","sms_wait":5}"#[..]),
            (
                "/register",
                &br#"{"status":"ok","login":"99910000001"}"#[..],
            ),
        ])),
        calls: AtomicUsize::new(0),
    });
    let environment = super::mock_environment(4443)?;
    let mut client = MobileRegistrationClient::initialize(
        backend.clone(),
        keys,
        &builder,
        environment.clone(),
        StoredMobilePlatform::Android,
        RegistrationRunId::from_bytes([12; 32]),
        RegistrationPolicy::new(6, 1000)?,
        http.clone(),
        clock.clone(),
        runtime.clone(),
    )
    .await?;
    let policy = AttestationPolicy {
        require_body_attestation: true,
        require_authorization: false,
    };
    client
        .check_account(
            &builder,
            Some(REQUEST_TOKEN),
            EphemeralRegistrationKey::from_bytes([21; 32]),
            policy,
            &SuppliedAttestation,
        )
        .await?;
    assert_eq!(
        http.calls.load(Ordering::SeqCst),
        1,
        "lookup cannot request an OTP"
    );
    client
        .request_code(
            &builder,
            VerificationMethod::Sms,
            Some(REQUEST_TOKEN),
            &extras,
            EphemeralRegistrationKey::from_bytes([22; 32]),
            policy,
            &SuppliedAttestation,
        )
        .await?;
    assert_eq!(client.stage(), RegistrationStage::AwaitingCode);
    assert!(matches!(
        client
            .request_code(
                &builder,
                VerificationMethod::Sms,
                Some(REQUEST_TOKEN),
                &extras,
                EphemeralRegistrationKey::from_bytes([24; 32]),
                policy,
                &SuppliedAttestation
            )
            .await,
        Err(MobileRegistrationError::Workflow(
            RegistrationError::Cooldown
        ))
    ));
    assert_eq!(http.calls.load(Ordering::SeqCst), 2);
    client
        .submit_code(
            &builder,
            "123456",
            Some(REQUEST_TOKEN),
            &extras,
            EphemeralRegistrationKey::from_bytes([23; 32]),
            policy,
            &SuppliedAttestation,
        )
        .await?;
    assert_eq!(client.stage(), RegistrationStage::Registered);
    assert_eq!(http.calls.load(Ordering::SeqCst), 3);
    let durable = backend.load().await?.expect("registered identity durable");
    assert_eq!(
        durable.pn.as_ref().expect("canonical phone").user,
        "99910000001"
    );
    drop(client);
    let manager = Arc::new(PersistenceManager::new(backend).await?);
    let (restored, material) = MobileRegistrationClient::restore_persisted(
        manager,
        environment,
        http.clone(),
        clock,
        runtime,
    )?;
    assert_eq!(restored.stage(), RegistrationStage::Registered);
    assert_eq!(material.login_metadata().family_device_id, INSTALLATION);
    assert_eq!(
        http.calls.load(Ordering::SeqCst),
        3,
        "restart cannot resend HTTP"
    );
    let account = restored.registered_account()?;
    let login = MobileLoginProfile::new(
        MobilePlatform::Android,
        MobileAppVersion::parse("2.26.1.7")?,
        MobileDevice::builder()
            .manufacturer("Synthetic".into())
            .device("Fixture".into())
            .os_version("15".into())
            .os_build_number("fixture-build".into())
            .phone_id(INSTALLATION.to_ascii_uppercase())
            .locale_language("en".into())
            .locale_country("ZZ".into())
            .build(),
    )?;
    let session = RegisteredMobileSession::from_registered_account(
        &account,
        login,
        MobileLoginOptions::default(),
        MobileNoisePrologue::new(3),
        Arc::new(NoConnectionEntropy),
        RegistrationLoginPhoneIdMapping::FdidUppercase,
    )?
    .with_primary_identity_policy(PrimaryDeviceIdentityPolicy::Omit);
    assert_eq!(session.username(), 99910000001);
    assert!(!session.encode_payload()?.is_empty());
    Ok(())
}
