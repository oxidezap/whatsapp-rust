//! Registered primary-mobile session configuration, independent of host I/O.
use std::sync::Arc;
use wacore::libsignal::protocol::KeyPair;
use wacore::mobile::{
    MobileEnvironment, MobileLoginOptions, MobileLoginProfile, MobileNoisePrologue,
    MobileNoiseTrust, MobileSessionError, RegistrationIdentity,
};
use wacore::store::{Device, StoredMobilePlatform};
use wacore::sync_marker::MaybeSendSync;

/// Hosts provide fresh ephemeral keys on every connection, including reconnects.
/// Implementations must use their host's cryptographic entropy source.
pub trait MobileEphemeralKeyProvider: MaybeSendSync + 'static {
    fn generate(&self) -> Result<KeyPair, MobileSessionError>;
}

/// Context for one payload-generation attempt, after persisted role/key validation.
/// The SDK reserves a zero-based counter per client immediately before each XX attempt.
/// Failed handshakes count, and hosts may derive a different wire counter if their profile requires it.
/// Failed transport dialing does not generate a payload and does not advance it.
pub struct MobileLoginAttempt<'a> {
    pub device: &'a Device,
    pub zero_based_attempt: u32,
}

/// Optional host policy for per-attempt login fields and host-owned entropy.
/// Return the complete options for this attempt: field presence remains a profile choice.
/// The static default inserts no session ID, counter, or other optional field.
pub trait MobileLoginOptionsProvider: MaybeSendSync + 'static {
    fn options(
        &self,
        context: MobileLoginAttempt<'_>,
        configured: &MobileLoginOptions,
    ) -> Result<MobileLoginOptions, MobileSessionError>;
}

/// Explicit registration-to-login installation identity mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationLoginPhoneIdMapping {
    /// Profiles that send the registered FDID uppercased as the login phone ID.
    FdidUppercase,
    /// A stable, independently supplied installation ID in the host login profile.
    IndependentHostId,
}

/// Validate metadata whose registration/login meaning is shared by the profile.
/// HTTP model IDs and manufacturer encodings may differ from login marketing names;
/// callers map those fields explicitly when constructing their login profile.
pub fn validate_registration_login_profile(
    profile: &MobileLoginProfile,
    metadata: &wacore::mobile::registration_wire::RegistrationLoginMetadata<'_>,
    phone_id_mapping: RegistrationLoginPhoneIdMapping,
) -> Result<(), MobileSessionError> {
    use wacore::mobile::registration_wire::RegistrationPlatform;
    use wacore::mobile::{MobileAppVersion, MobilePlatform};
    let platform = match metadata.platform {
        RegistrationPlatform::Android => MobilePlatform::Android,
        RegistrationPlatform::AndroidBusiness => MobilePlatform::AndroidBusiness,
        RegistrationPlatform::Ios => MobilePlatform::Ios,
        RegistrationPlatform::IosBusiness => MobilePlatform::IosBusiness,
    };
    let version = metadata
        .app_version
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let expected_version = MobileAppVersion::parse(&version)?;
    let user_agent = profile.user_agent();
    if user_agent.platform != Some(platform.to_protobuf())
        || profile.app_version() != &expected_version
        || user_agent.os_version.as_deref() != Some(metadata.os_version)
        || user_agent.locale_language_iso6391.as_deref() != Some(metadata.language)
        || user_agent.locale_country_iso31661_alpha2.as_deref() != Some(metadata.locale_country)
        || user_agent
            .release_channel
            .is_some_and(|value| value as u32 != metadata.release_channel)
    {
        return Err(MobileSessionError::InvalidProfile(
            "registration_login_metadata",
        ));
    }
    if phone_id_mapping == RegistrationLoginPhoneIdMapping::FdidUppercase
        && user_agent.phone_id.as_deref()
            != Some(metadata.family_device_id.to_ascii_uppercase().as_str())
    {
        return Err(MobileSessionError::InvalidProfile(
            "registration_login_phone_id",
        ));
    }
    Ok(())
}

/// Immutable connection profile bound to one account, key identity and environment.
/// No Web defaults, ambient entropy, or endpoint discovery are applied.
pub struct RegisteredMobileSession {
    media_http_profile: Option<Arc<wacore::download::MobileMediaHttpProfile>>,
    environment: MobileEnvironment,
    profile: MobileLoginProfile,
    options: MobileLoginOptions,
    options_provider: Option<Arc<dyn MobileLoginOptionsProvider>>,
    username: u64,
    identity: RegistrationIdentity,
    prologue: MobileNoisePrologue,
    trust: MobileNoiseTrust,
    ephemeral_keys: Arc<dyn MobileEphemeralKeyProvider>,
    app_state_key: Option<crate::mobile_primary::PrimaryAppStateKeyInput>,
    primary_identity_policy: Option<wacore::send::PrimaryDeviceIdentityPolicy>,
}

impl RegisteredMobileSession {
    /// Build from the durable registration result, retaining canonical account/environment/key binding.
    /// Login-only metadata stays explicit; registration metadata must agree where meanings match.
    pub fn from_registered_account(
        account: &crate::mobile_registration::RegisteredMobileAccount,
        profile: MobileLoginProfile,
        options: MobileLoginOptions,
        prologue: MobileNoisePrologue,
        ephemeral_keys: Arc<dyn MobileEphemeralKeyProvider>,
        phone_id_mapping: RegistrationLoginPhoneIdMapping,
    ) -> Result<Self, MobileSessionError> {
        validate_registration_login_profile(
            &profile,
            &account.material().login_metadata(),
            phone_id_mapping,
        )?;
        let username = account
            .canonical_phone_number()
            .parse()
            .map_err(|_| MobileSessionError::InvalidUsername)?;
        Self::new(
            account.environment().clone(),
            profile,
            options,
            username,
            account.identity(),
            prologue,
            ephemeral_keys,
        )
    }

    /// Resolve profile-specific dynamic fields once per XX payload attempt.
    /// The provider supplies its own entropy; no fields are inferred or inserted.
    pub fn with_login_options_provider(
        mut self,
        provider: Arc<dyn MobileLoginOptionsProvider>,
    ) -> Self {
        self.options_provider = Some(provider);
        self
    }

    pub fn encode_attempt_payload(
        &self,
        device: &Device,
        zero_based_attempt: u32,
    ) -> Result<Vec<u8>, MobileSessionError> {
        self.validate_device(device)?;
        let options = match &self.options_provider {
            Some(provider) => provider.options(
                MobileLoginAttempt {
                    device,
                    zero_based_attempt,
                },
                &self.options,
            )?,
            None => self.options.clone(),
        };
        self.profile.encode_payload(self.username, &options)
    }

    /// Select primary device-identity attachment explicitly for the qualified profile.
    /// References differ: callers must select a policy explicitly before connecting.
    /// No platform-wide behavior is inferred, and trusted identity is never synthesized.
    pub fn with_primary_identity_policy(
        mut self,
        policy: wacore::send::PrimaryDeviceIdentityPolicy,
    ) -> Self {
        self.primary_identity_policy = Some(policy);
        self
    }

    pub fn primary_identity_policy(&self) -> Option<wacore::send::PrimaryDeviceIdentityPolicy> {
        self.primary_identity_policy
    }

    /// Supply host-generated app-state material for a primary without a persisted key.
    pub fn with_app_state_key(
        mut self,
        input: crate::mobile_primary::PrimaryAppStateKeyInput,
    ) -> Result<Self, crate::mobile_primary::PrimaryManagementError> {
        input.validate()?;
        self.app_state_key = Some(input);
        Ok(self)
    }

    pub fn app_state_key(&self) -> Option<&crate::mobile_primary::PrimaryAppStateKeyInput> {
        self.app_state_key.as_ref()
    }

    pub fn new(
        environment: MobileEnvironment,
        profile: MobileLoginProfile,
        options: MobileLoginOptions,
        username: u64,
        identity: RegistrationIdentity,
        prologue: MobileNoisePrologue,
        ephemeral_keys: Arc<dyn MobileEphemeralKeyProvider>,
    ) -> Result<Self, MobileSessionError> {
        profile.build_payload(username, &options)?;
        let trust = MobileNoiseTrust::new(*environment.trust().noise_root())?;
        Ok(Self {
            media_http_profile: None,
            environment,
            profile,
            options,
            options_provider: None,
            username,
            identity,
            prologue,
            trust,
            ephemeral_keys,
            app_state_key: None,
            primary_identity_policy: None,
        })
    }

    /// Select the exact mobile media headers/routes for this session.
    pub fn with_media_http_profile(
        mut self,
        profile: Arc<wacore::download::MobileMediaHttpProfile>,
    ) -> Self {
        self.media_http_profile = Some(profile);
        self
    }
    pub fn media_http_profile(&self) -> Option<&wacore::download::MobileMediaHttpProfile> {
        self.media_http_profile.as_deref()
    }

    pub fn environment(&self) -> &MobileEnvironment {
        &self.environment
    }
    pub fn profile(&self) -> &MobileLoginProfile {
        &self.profile
    }
    pub fn username(&self) -> u64 {
        self.username
    }
    pub fn prologue(&self) -> MobileNoisePrologue {
        self.prologue
    }
    pub fn trust(&self) -> MobileNoiseTrust {
        self.trust
    }
    pub fn fresh_ephemeral_key(&self) -> Result<KeyPair, MobileSessionError> {
        self.ephemeral_keys.generate()
    }
    /// Encode configured fields for offline inspection without invoking the host policy.
    /// Connections use `encode_attempt_payload` with a validated snapshot and attempt counter.
    pub fn encode_payload(&self) -> Result<Vec<u8>, MobileSessionError> {
        self.profile.encode_payload(self.username, &self.options)
    }

    /// Fail before sending any bytes when persisted credentials belong elsewhere.
    pub fn validate_device(&self, device: &Device) -> Result<(), MobileSessionError> {
        if self.primary_identity_policy.is_none() {
            return Err(MobileSessionError::InvalidProfile(
                "primary_identity_policy",
            ));
        }
        device
            .validate_mobile_session()
            .map_err(|_| MobileSessionError::InvalidKeyMaterial)?;
        let pn = device
            .pn
            .as_ref()
            .ok_or(MobileSessionError::InvalidUsername)?;
        if !pn.is_pn()
            || pn.device != 0
            || pn.agent != 0
            || pn.integrator != 0
            || pn.user.as_str() != self.username.to_string()
        {
            return Err(MobileSessionError::InvalidUsername);
        }
        let identity_key: [u8; 32] = device
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| MobileSessionError::InvalidKeyMaterial)?;
        let noise_key: [u8; 32] = device
            .noise_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| MobileSessionError::InvalidKeyMaterial)?;
        if RegistrationIdentity::from_public_keys(&identity_key, &noise_key) != self.identity {
            return Err(MobileSessionError::InvalidKeyMaterial);
        }
        let binding = device
            .mobile_session
            .as_ref()
            .ok_or(MobileSessionError::InvalidProfile("mobile_session"))?;
        binding
            .validate()
            .map_err(|_| MobileSessionError::InvalidProfile("mobile_session"))?;
        let expected_platform = match self.profile.user_agent().platform {
            Some(waproto::whatsapp::client_payload::user_agent::Platform::ANDROID) => {
                StoredMobilePlatform::Android
            }
            Some(waproto::whatsapp::client_payload::user_agent::Platform::SMB_ANDROID) => {
                StoredMobilePlatform::AndroidBusiness
            }
            Some(waproto::whatsapp::client_payload::user_agent::Platform::IOS) => {
                StoredMobilePlatform::Ios
            }
            Some(waproto::whatsapp::client_payload::user_agent::Platform::SMB_IOS) => {
                StoredMobilePlatform::IosBusiness
            }
            _ => return Err(MobileSessionError::InvalidProfile("platform")),
        };
        if binding.environment_hash != *self.environment.binding().as_bytes()
            || binding.identity_hash != *self.identity.as_bytes()
            || binding.platform != expected_platform
        {
            return Err(MobileSessionError::InvalidProfile("mobile_session_binding"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::mobile::{
        MobileAppVersion, MobileDevice, MobilePlatform, MobileTrust, RegistrationEndpoint,
        TcpEndpoint,
    };
    use wacore::store::commands::apply_command_to_device;
    use wacore::store::{DeviceCommand, MobileSessionBinding};
    struct Keys;
    impl MobileEphemeralKeyProvider for Keys {
        fn generate(&self) -> Result<KeyPair, MobileSessionError> {
            Err(MobileSessionError::InvalidKeyMaterial)
        }
    }
    fn config(device: &Device, env_id: &str) -> RegisteredMobileSession {
        let environment = MobileEnvironment::new(
            env_id,
            RegistrationEndpoint::for_test_http("http://mock.invalid/registration").unwrap(),
            TcpEndpoint::new("mock.invalid", 5222).unwrap(),
            MobileTrust::new([1; 32], [2; 32]).unwrap(),
        )
        .unwrap();
        let profile = MobileLoginProfile::new(
            MobilePlatform::Android,
            MobileAppVersion::parse("2.26.1").unwrap(),
            MobileDevice::builder()
                .manufacturer("Acme".into())
                .device("Model".into())
                .os_version("1".into())
                .os_build_number("B1".into())
                .phone_id("synthetic".into())
                .locale_language("en".into())
                .locale_country("GB".into())
                .build(),
        )
        .unwrap();
        RegisteredMobileSession::new(
            environment,
            profile,
            MobileLoginOptions::default(),
            15550000001,
            RegistrationIdentity::from_public_keys(
                device
                    .identity_key
                    .public_key
                    .public_key_bytes()
                    .try_into()
                    .unwrap(),
                device
                    .noise_key
                    .public_key
                    .public_key_bytes()
                    .try_into()
                    .unwrap(),
            ),
            MobileNoisePrologue::new(3),
            Arc::new(Keys),
        )
        .unwrap()
    }
    // Independent wire-field inspection: generated decoder is not the oracle here.
    fn counters(mut input: &[u8]) -> (Option<i32>, Option<u32>, Option<i32>) {
        fn take_varint(input: &mut &[u8]) -> u64 {
            let mut result = 0;
            for shift in (0..70).step_by(7) {
                let byte = input[0];
                *input = &input[1..];
                result |= u64::from(byte & 127) << shift;
                if byte & 128 == 0 {
                    return result;
                }
            }
            panic!("overlong fixture varint");
        }
        let mut result = (None, None, None);
        while !input.is_empty() {
            let tag = take_varint(&mut input);
            match tag & 7 {
                0 => {
                    let value = take_varint(&mut input);
                    match tag >> 3 {
                        9 => result.0 = Some(value as i32),
                        16 => result.1 = Some(value as u32),
                        24 => result.2 = Some(value as i32),
                        _ => {}
                    }
                }
                2 => {
                    let count = take_varint(&mut input) as usize;
                    input = &input[count..];
                }
                _ => panic!("unexpected fixture wire type"),
            }
        }
        result
    }

    #[test]
    fn registration_handoff_checks_shared_metadata_and_explicit_phone_id_mapping() {
        use wacore::mobile::registration_wire::{RegistrationLoginMetadata, RegistrationPlatform};
        let device = Device::new();
        let config = config(&device, "synthetic-handoff");
        let mut metadata = RegistrationLoginMetadata {
            platform: RegistrationPlatform::Android,
            app_version: &[2, 26, 1],
            os_version: "1",
            registration_model: "model_code_different_from_login_name",
            android_manufacturer: Some("encoded_manufacturer"),
            family_device_id: "synthetic",
            language: "en",
            locale_country: "GB",
            release_channel: 0,
        };
        validate_registration_login_profile(
            config.profile(),
            &metadata,
            RegistrationLoginPhoneIdMapping::IndependentHostId,
        )
        .unwrap();
        assert!(
            validate_registration_login_profile(
                config.profile(),
                &metadata,
                RegistrationLoginPhoneIdMapping::FdidUppercase
            )
            .is_err()
        );
        metadata.platform = RegistrationPlatform::Ios;
        assert!(
            validate_registration_login_profile(
                config.profile(),
                &metadata,
                RegistrationLoginPhoneIdMapping::IndependentHostId
            )
            .is_err()
        );
        metadata.platform = RegistrationPlatform::Android;
        metadata.app_version = &[2, 26, 2];
        assert!(
            validate_registration_login_profile(
                config.profile(),
                &metadata,
                RegistrationLoginPhoneIdMapping::IndependentHostId
            )
            .is_err()
        );
        metadata.app_version = &[2, 26, 1];
        metadata.locale_country = "US";
        assert!(
            validate_registration_login_profile(
                config.profile(),
                &metadata,
                RegistrationLoginPhoneIdMapping::IndependentHostId
            )
            .is_err()
        );
    }

    struct DynamicOptions {
        session_ids: std::sync::atomic::AtomicI32,
    }
    impl MobileLoginOptionsProvider for DynamicOptions {
        fn options(
            &self,
            context: MobileLoginAttempt<'_>,
            _configured: &MobileLoginOptions,
        ) -> Result<MobileLoginOptions, MobileSessionError> {
            // Deterministic synthetic entropy stand-in, deliberately confined to tests.
            let session_id = self
                .session_ids
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(MobileLoginOptions::builder()
                .session_id(session_id)
                .connect_attempt_count(context.zero_based_attempt)
                .login_counter(context.device.login_counter)
                .build())
        }
    }

    #[test]
    fn dynamic_payload_reads_validated_persisted_counter_and_host_entropy_per_attempt() {
        let mut device = Device::new();
        let session = config(&device, "synthetic-dynamic")
            .with_primary_identity_policy(wacore::send::PrimaryDeviceIdentityPolicy::Omit)
            .with_login_options_provider(Arc::new(DynamicOptions {
                session_ids: std::sync::atomic::AtomicI32::new(100),
            }));
        // Reject unbound state before invoking the host's entropy-producing provider.
        assert!(session.encode_attempt_payload(&device, 0).is_err());
        let binding = MobileSessionBinding::new(
            *session.environment().binding().as_bytes(),
            device.mobile_identity_hash(),
            StoredMobilePlatform::Android,
            vec![],
            None,
        )
        .unwrap();
        apply_command_to_device(&mut device, DeviceCommand::SetMobileSessionBinding(binding));
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetId(Some("15550000001@s.whatsapp.net".parse().unwrap())),
        );
        let first = counters(&session.encode_attempt_payload(&device, 0).unwrap());
        apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
        let second = counters(&session.encode_attempt_payload(&device, 1).unwrap());
        assert_eq!(first, (Some(100), Some(0), Some(0)));
        assert_eq!(second, (Some(101), Some(1), Some(1)));
        // Provider context remains per-client even when an immutable configuration is shared.
        let other_client = counters(&session.encode_attempt_payload(&device, 0).unwrap());
        assert_eq!(other_client.1, Some(0));
        assert_eq!(other_client.0, Some(102));
    }

    #[test]
    fn resume_requires_matching_primary_identity_environment_and_platform() {
        let mut device = Device::new();
        let session = config(&device, "synthetic-a");
        assert_eq!(session.primary_identity_policy(), None);
        assert!(matches!(
            session.validate_device(&device),
            Err(MobileSessionError::InvalidProfile(
                "primary_identity_policy"
            ))
        ));
        let session =
            session.with_primary_identity_policy(wacore::send::PrimaryDeviceIdentityPolicy::Omit);
        let binding = MobileSessionBinding::new(
            *session.environment().binding().as_bytes(),
            device.mobile_identity_hash(),
            StoredMobilePlatform::Android,
            vec![],
            None,
        )
        .unwrap();
        apply_command_to_device(&mut device, DeviceCommand::SetMobileSessionBinding(binding));
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetId(Some("15550000001@s.whatsapp.net".parse().unwrap())),
        );
        session.validate_device(&device).unwrap();
        let first = session.encode_attempt_payload(&device, 0).unwrap();
        assert_eq!(first, session.encode_attempt_payload(&device, 1).unwrap());
        assert_eq!(counters(&first), (None, None, None));

        assert!(
            config(&device, "synthetic-b")
                .with_primary_identity_policy(wacore::send::PrimaryDeviceIdentityPolicy::Omit)
                .validate_device(&device)
                .is_err()
        );
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetId(Some("15550000001:1@s.whatsapp.net".parse().unwrap())),
        );
        assert!(session.validate_device(&device).is_err());
        apply_command_to_device(
            &mut device,
            DeviceCommand::SetId(Some("15550000002@s.whatsapp.net".parse().unwrap())),
        );
        assert!(session.validate_device(&device).is_err());
        assert!(session.fresh_ephemeral_key().is_err());
    }
}
