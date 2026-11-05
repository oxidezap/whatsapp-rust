//! Registered-primary mobile payloads and the flat-certificate Noise XX path.
//!
//! Reference traces: Zapo bceb268c846763a2cd0b494da830a54aacdc3818
//! `src/transport/noise/WaMobileClientPayload.ts`; Cobalt
//! cb9fbf172507626948fbc3611d0a6fb00bb3a0f1 `WhatsAppSocketClient.java`
//! (mobileClientPayload, mobileUserAgent and flat certificate verification);
//! whalibmob 8f50db4c586534dc0af4d13b26bdd09ef42660c0 `lib/noise.js`.
//! These traces establish implementation behavior, not official APK qualification.
//!
//! Field presence is explicit: reference mobile clients differ on optional login
//! fields. Callers supply installation metadata, build version, entropy and time.

use subtle::ConstantTimeEq;
use thiserror::Error;
use wacore_libsignal::protocol::{KeyPair, PublicKey};
use wacore_noise::{HandshakeError, HandshakeUtils, NoiseCipher, NoiseHandshake};
use waproto::whatsapp::{self as wa, client_payload as cp};
use zeroize::Zeroizing;

const MAX_TEXT: usize = 256;
const MAX_PAYLOAD: usize = 64 * 1024;
const MAX_SERVER_HELLO: usize = 128 * 1024;
const MAX_CERTIFICATE: usize = 16 * 1024;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MobileSessionError {
    #[error("mobile sessions require a host-supplied clock")]
    MissingClock,
    #[error("mobile session clock returned an invalid calendar reading")]
    MalformedClock,
    #[error("invalid mobile profile field: {0}")]
    InvalidProfile(&'static str),
    #[error("invalid mobile application version")]
    InvalidVersion,
    #[error("invalid mobile primary account identifier")]
    InvalidUsername,
    #[error("a primary mobile login cannot advertise a companion device number")]
    InvalidDevice,
    #[error("invalid or reused Noise key material")]
    InvalidKeyMaterial,
    #[error("invalid mobile Noise trust root")]
    InvalidTrust,
    #[error("mobile handshake input exceeds its size budget")]
    SizeLimit,
    #[error("invalid mobile certificate: {0}")]
    InvalidCertificate(&'static str),
    #[error("mobile server certificate has expired")]
    ExpiredCertificate,
    #[error("{0}")]
    Protobuf(#[from] buffa::DecodeError),
    #[error("{0}")]
    Noise(#[from] HandshakeError),
}

/// Platform/product selection, mapped to the generated protocol enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobilePlatform {
    Android,
    AndroidBusiness,
    Ios,
    IosBusiness,
}

impl MobilePlatform {
    pub fn to_protobuf(self) -> cp::user_agent::Platform {
        match self {
            Self::Android => cp::user_agent::Platform::ANDROID,
            Self::AndroidBusiness => cp::user_agent::Platform::SMB_ANDROID,
            Self::Ios => cp::user_agent::Platform::IOS,
            Self::IosBusiness => cp::user_agent::Platform::SMB_IOS,
        }
    }
}

/// Explicit build version; no network lookup or fallback version is selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileAppVersion(Vec<u32>);

impl MobileAppVersion {
    pub fn parse(value: &str) -> Result<Self, MobileSessionError> {
        if value.len() > 64 {
            return Err(MobileSessionError::InvalidVersion);
        }
        let mut components = Vec::with_capacity(5);
        for part in value.split('.') {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(MobileSessionError::InvalidVersion);
            }
            components.push(
                part.parse()
                    .map_err(|_| MobileSessionError::InvalidVersion)?,
            );
            if components.len() > 5 {
                return Err(MobileSessionError::InvalidVersion);
            }
        }
        if components.len() < 3 || components[0] == 0 {
            return Err(MobileSessionError::InvalidVersion);
        }
        Ok(Self(components))
    }

    pub fn to_protobuf(&self) -> cp::user_agent::AppVersion {
        let mut version = cp::user_agent::AppVersion::default();
        version.primary = Some(self.0[0]);
        version.secondary = Some(self.0[1]);
        version.tertiary = Some(self.0[2]);
        version.quaternary = self.0.get(3).copied();
        version.quinary = self.0.get(4).copied();
        version
    }
}

/// Installation metadata supplied by the host, never generated during login.
/// Optional fields preserve absence rather than inserting carrier/locale values.
#[derive(Clone, bon::Builder)]
pub struct MobileDevice {
    manufacturer: String,
    device: String,
    os_version: String,
    os_build_number: String,
    phone_id: String,
    locale_language: String,
    locale_country: String,
    mcc: Option<String>,
    mnc: Option<String>,
    device_board: Option<String>,
    device_exp_id: Option<String>,
    device_model_type: Option<String>,
    release_channel: Option<cp::user_agent::ReleaseChannel>,
    device_type: Option<cp::user_agent::DeviceType>,
    distribution_channel: Option<cp::user_agent::DistributionChannel>,
}

/// Validated primary-phone metadata. Debug deliberately does not expose device IDs.
#[derive(Clone)]
pub struct MobileLoginProfile {
    version: MobileAppVersion,
    user_agent: cp::UserAgent,
}

fn text(value: &str, field: &'static str) -> Result<(), MobileSessionError> {
    if value.trim().is_empty() || value.len() > MAX_TEXT || value.chars().any(char::is_control) {
        return Err(MobileSessionError::InvalidProfile(field));
    }
    Ok(())
}

impl MobileLoginProfile {
    pub fn new(
        platform: MobilePlatform,
        version: MobileAppVersion,
        device: MobileDevice,
    ) -> Result<Self, MobileSessionError> {
        for (field, value) in [
            ("manufacturer", &device.manufacturer),
            ("device", &device.device),
            ("os_version", &device.os_version),
            ("os_build_number", &device.os_build_number),
            ("phone_id", &device.phone_id),
            ("locale_language", &device.locale_language),
            ("locale_country", &device.locale_country),
        ] {
            text(value, field)?;
        }
        for (field, value) in [
            ("mcc", &device.mcc),
            ("mnc", &device.mnc),
            ("device_board", &device.device_board),
            ("device_exp_id", &device.device_exp_id),
            ("device_model_type", &device.device_model_type),
        ] {
            if let Some(value) = value {
                text(value, field)?;
            }
        }
        if device
            .device_type
            .is_some_and(|kind| kind != cp::user_agent::DeviceType::PHONE)
        {
            return Err(MobileSessionError::InvalidProfile("device_type"));
        }
        let mut user_agent = cp::UserAgent::default();
        user_agent.platform = Some(platform.to_protobuf());
        user_agent.app_version = buffa::MessageField::some(version.to_protobuf());
        user_agent.manufacturer = Some(device.manufacturer);
        user_agent.device = Some(device.device);
        user_agent.os_version = Some(device.os_version);
        user_agent.os_build_number = Some(device.os_build_number);
        user_agent.phone_id = Some(device.phone_id);
        user_agent.locale_language_iso6391 = Some(device.locale_language);
        user_agent.locale_country_iso31661_alpha2 = Some(device.locale_country);
        user_agent.mcc = device.mcc;
        user_agent.mnc = device.mnc;
        user_agent.device_board = device.device_board;
        user_agent.device_exp_id = device.device_exp_id;
        user_agent.device_model_type = device.device_model_type;
        user_agent.release_channel = device.release_channel;
        user_agent.device_type = device.device_type;
        user_agent.distribution_channel = device.distribution_channel;
        Ok(Self {
            version,
            user_agent,
        })
    }

    pub fn app_version(&self) -> &MobileAppVersion {
        &self.version
    }

    pub fn user_agent(&self) -> &cp::UserAgent {
        &self.user_agent
    }

    pub fn build_payload(
        &self,
        username: u64,
        options: &MobileLoginOptions,
    ) -> Result<wa::ClientPayload, MobileSessionError> {
        if username == 0 {
            return Err(MobileSessionError::InvalidUsername);
        }
        if options.device.is_some_and(|device| device != 0) {
            return Err(MobileSessionError::InvalidDevice);
        }
        if let Some(name) = &options.push_name {
            text(name, "push_name")?;
        }
        let mut payload = wa::ClientPayload::default();
        payload.username = Some(username);
        payload.user_agent = buffa::MessageField::some(self.user_agent.clone());
        payload.passive = options.passive;
        payload.pull = options.pull;
        payload.device = options.device;
        payload.push_name = options.push_name.clone();
        payload.session_id = options.session_id;
        payload.short_connect = options.short_connect;
        payload.connect_type = options.connect_type;
        payload.connect_reason = options.connect_reason;
        payload.connect_attempt_count = options.connect_attempt_count;
        payload.lc = options.login_counter;
        payload.oc = options.oc;
        payload.product = options.product;
        payload.year_class = options.year_class;
        payload.mem_class = options.mem_class;
        payload.dns_source = options
            .dns_source
            .clone()
            .map_or_else(buffa::MessageField::default, buffa::MessageField::some);
        payload.traffic_anonymization = options.traffic_anonymization;
        payload.connection_sequence_info = options.connection_sequence_info;
        payload.lid_db_migrated = options.lid_db_migrated;
        Ok(payload)
    }

    pub fn encode_payload(
        &self,
        username: u64,
        options: &MobileLoginOptions,
    ) -> Result<Vec<u8>, MobileSessionError> {
        Ok(waproto::codec::client_payload_to_vec(
            &self.build_payload(username, options)?,
        ))
    }
}

/// Explicit optional login fields; no Web login defaults or ambient entropy.
#[derive(Clone, Default, bon::Builder)]
pub struct MobileLoginOptions {
    passive: Option<bool>,
    pull: Option<bool>,
    device: Option<u32>,
    push_name: Option<String>,
    session_id: Option<i32>,
    short_connect: Option<bool>,
    connect_type: Option<cp::ConnectType>,
    connect_reason: Option<cp::ConnectReason>,
    connect_attempt_count: Option<u32>,
    login_counter: Option<i32>,
    oc: Option<bool>,
    product: Option<cp::Product>,
    year_class: Option<i32>,
    mem_class: Option<i32>,
    dns_source: Option<cp::DNSSource>,
    traffic_anonymization: Option<cp::TrafficAnonymization>,
    connection_sequence_info: Option<i32>,
    lid_db_migrated: Option<bool>,
}

impl MobileLoginOptions {
    /// Replace only this attempt field; `None` explicitly omits it.
    pub fn with_session_id(mut self, value: Option<i32>) -> Self {
        self.session_id = value;
        self
    }
    /// Replace only this attempt field; `None` explicitly omits it.
    pub fn with_connect_attempt_count(mut self, value: Option<u32>) -> Self {
        self.connect_attempt_count = value;
        self
    }
    /// Replace only this attempt field; `None` explicitly omits it.
    pub fn with_login_counter(mut self, value: Option<i32>) -> Self {
        self.login_counter = value;
        self
    }
}

/// Native-mobile protocol shape with an explicitly selected dictionary version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MobileNoisePrologue([u8; 4]);

impl MobileNoisePrologue {
    pub const fn new(dictionary_version: u8) -> Self {
        Self([b'W', b'A', 5, dictionary_version])
    }

    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
}

/// Immutable strict flat-certificate trust; it never accepts a signature bypass.
#[derive(Clone, Copy)]
pub struct MobileNoiseTrust {
    root: [u8; 32],
    expiry_policy: MobileCertificateExpiryPolicy,
}

/// Missing expiration is rejected unless the host explicitly selects a profile
/// permitting its absence. Present expiration is always checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MobileCertificateExpiryPolicy {
    Required,
    IfPresent,
}

impl MobileNoiseTrust {
    pub fn new(root: [u8; 32]) -> Result<Self, MobileSessionError> {
        if root == [0; 32] {
            return Err(MobileSessionError::InvalidTrust);
        }
        Ok(Self {
            root,
            expiry_policy: MobileCertificateExpiryPolicy::Required,
        })
    }

    pub fn with_expiry_policy(mut self, policy: MobileCertificateExpiryPolicy) -> Self {
        self.expiry_policy = policy;
        self
    }

    pub fn verify_certificate(
        &self,
        encoded: &[u8],
        expected_static: &[u8; 32],
        now_secs: u64,
    ) -> Result<VerifiedMobileCertificate, MobileSessionError> {
        if encoded.len() > MAX_CERTIFICATE {
            return Err(MobileSessionError::SizeLimit);
        }
        if expected_static == &[0; 32] {
            return Err(MobileSessionError::InvalidCertificate("zero static key"));
        }
        strict_certificate_fields(encoded, false)?;
        let certificate = waproto::codec::mobile_noise_certificate_decode(encoded)?;
        let details = certificate
            .details
            .ok_or(MobileSessionError::InvalidCertificate("missing details"))?;
        let signature = certificate
            .signature
            .ok_or(MobileSessionError::InvalidCertificate("missing signature"))?;
        if signature.len() != 64 {
            return Err(MobileSessionError::InvalidCertificate("signature length"));
        }
        let root = PublicKey::from_djb_public_key_bytes(&self.root)
            .map_err(|_| MobileSessionError::InvalidTrust)?;
        if !root.verify_signature(&details, &signature) {
            return Err(MobileSessionError::InvalidCertificate("signature"));
        }
        strict_certificate_fields(&details, true)?;
        let details = waproto::codec::mobile_noise_certificate_details_decode(&details)?;
        let key = details
            .key
            .ok_or(MobileSessionError::InvalidCertificate("missing key"))?;
        if key.as_slice() != expected_static {
            return Err(MobileSessionError::InvalidCertificate("static key binding"));
        }
        if details.expires.is_none()
            && self.expiry_policy == MobileCertificateExpiryPolicy::Required
        {
            return Err(MobileSessionError::InvalidCertificate("missing expiration"));
        }
        if details.expires.is_some_and(|expires| expires <= now_secs) {
            return Err(MobileSessionError::ExpiredCertificate);
        }
        Ok(VerifiedMobileCertificate {
            static_key: *expected_static,
            serial: details.serial,
            issuer: details.issuer,
            subject: details.subject,
            expires: details.expires,
        })
    }
}

/// Only constructed after signature and handshake-static verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMobileCertificate {
    pub static_key: [u8; 32],
    pub serial: Option<u32>,
    pub issuer: Option<String>,
    pub subject: Option<String>,
    /// None is possible only under an explicit IfPresent policy. It establishes
    /// signature/static binding without an expiration constraint.
    pub expires: Option<u64>,
}

fn varint(bytes: &[u8], offset: &mut usize) -> Result<u64, MobileSessionError> {
    let mut result = 0_u64;
    for shift in (0..70).step_by(7) {
        let byte = *bytes
            .get(*offset)
            .ok_or(MobileSessionError::InvalidCertificate("truncated varint"))?;
        *offset += 1;
        if shift == 63 && byte > 1 {
            return Err(MobileSessionError::InvalidCertificate("varint overflow"));
        }
        result |= u64::from(byte & 127) << shift;
        if byte & 128 == 0 {
            return Ok(result);
        }
    }
    Err(MobileSessionError::InvalidCertificate("varint overflow"))
}

// Strict one-message decoding avoids accepting a Web CertChain or silently
// dropping a future certificate shape during generated-protobuf decoding.
fn strict_certificate_fields(bytes: &[u8], details: bool) -> Result<(), MobileSessionError> {
    let mut offset = 0;
    let mut seen = 0_u8;
    while offset < bytes.len() {
        let tag = varint(bytes, &mut offset)?;
        let field = tag >> 3;
        let wire = tag & 7;
        let max = if details { 5 } else { 2 };
        if field == 0 || field > max || seen & (1 << field) != 0 {
            return Err(MobileSessionError::InvalidCertificate(
                "unknown or duplicate field",
            ));
        }
        seen |= 1 << field;
        let scalar = details && (field == 1 || field == 3);
        if wire != if scalar { 0 } else { 2 } {
            return Err(MobileSessionError::InvalidCertificate("field wire type"));
        }
        let value = varint(bytes, &mut offset)?;
        if scalar {
            if field == 1 && value > u64::from(u32::MAX) {
                return Err(MobileSessionError::InvalidCertificate("serial overflow"));
            }
        } else {
            let length = usize::try_from(value).map_err(|_| MobileSessionError::SizeLimit)?;
            offset = offset
                .checked_add(length)
                .filter(|end| *end <= bytes.len())
                .ok_or(MobileSessionError::InvalidCertificate("field length"))?;
        }
    }
    Ok(())
}

fn checked_agreement(
    noise: &mut NoiseHandshake,
    key: &KeyPair,
    peer: &[u8; 32],
) -> Result<(), MobileSessionError> {
    let shared = Zeroizing::new(
        wacore_libsignal::crypto::x25519_agreement(key.private_key.serialize(), peer)
            .map_err(|_| MobileSessionError::InvalidKeyMaterial)?,
    );
    if bool::from(shared.ct_eq(&[0; 32])) {
        return Err(MobileSessionError::InvalidKeyMaterial);
    }
    noise.mix_into_key(shared.as_ref())?;
    Ok(())
}

/// One Noise XX connection attempt. Host-generated ephemeral keys are consumed
/// together with this state; a rejected ServerHello cannot be retried on it.
pub struct MobileXxHandshake {
    noise: NoiseHandshake,
    static_key: KeyPair,
    ephemeral_key: KeyPair,
    payload: Vec<u8>,
    trust: MobileNoiseTrust,
}

pub struct MobileXxHandshakeOutcome {
    pub client_finish: Vec<u8>,
    pub write_cipher: NoiseCipher,
    pub read_cipher: NoiseCipher,
    pub server_certificate: VerifiedMobileCertificate,
}

impl MobileXxHandshake {
    pub fn new(
        static_key: KeyPair,
        ephemeral_key: KeyPair,
        payload: Vec<u8>,
        prologue: MobileNoisePrologue,
        trust: MobileNoiseTrust,
    ) -> Result<Self, MobileSessionError> {
        if payload.len() > MAX_PAYLOAD {
            return Err(MobileSessionError::SizeLimit);
        }
        for key in [&static_key, &ephemeral_key] {
            if key.private_key.public_key().ok().as_ref() != Some(&key.public_key) {
                return Err(MobileSessionError::InvalidKeyMaterial);
            }
        }
        if static_key.public_key == ephemeral_key.public_key {
            return Err(MobileSessionError::InvalidKeyMaterial);
        }
        let mut noise =
            NoiseHandshake::new(wacore_binary::consts::NOISE_PATTERN_XX, prologue.as_bytes())?;
        noise.authenticate(ephemeral_key.public_key.public_key_bytes());
        Ok(Self {
            noise,
            static_key,
            ephemeral_key,
            payload,
            trust,
        })
    }

    pub fn client_hello(&self) -> Vec<u8> {
        waproto::codec::handshake_message_to_vec(&HandshakeUtils::build_client_hello(
            self.ephemeral_key.public_key.public_key_bytes(),
        ))
    }

    pub fn receive_server_hello(
        mut self,
        response: &[u8],
        now_secs: u64,
    ) -> Result<MobileXxHandshakeOutcome, MobileSessionError> {
        if response.len() > MAX_SERVER_HELLO {
            return Err(MobileSessionError::SizeLimit);
        }
        let (ephemeral, encrypted_static, encrypted_certificate) =
            HandshakeUtils::parse_server_hello(response)?;
        let ephemeral: [u8; 32] = ephemeral
            .try_into()
            .map_err(|_| MobileSessionError::InvalidKeyMaterial)?;
        self.noise.authenticate(&ephemeral);
        checked_agreement(&mut self.noise, &self.ephemeral_key, &ephemeral)?;
        let decrypted_static = self.noise.decrypt(&encrypted_static)?;
        let server_static: [u8; 32] = decrypted_static
            .try_into()
            .map_err(|_| MobileSessionError::InvalidKeyMaterial)?;
        checked_agreement(&mut self.noise, &self.ephemeral_key, &server_static)?;
        let certificate = self.noise.decrypt(&encrypted_certificate)?;
        let server_certificate =
            self.trust
                .verify_certificate(&certificate, &server_static, now_secs)?;
        let encrypted_static = self
            .noise
            .encrypt(self.static_key.public_key.public_key_bytes())?;
        checked_agreement(&mut self.noise, &self.static_key, &ephemeral)?;
        let encrypted_payload = self.noise.encrypt(&self.payload)?;
        let client_finish = waproto::codec::handshake_message_to_vec(
            &HandshakeUtils::build_client_finish(encrypted_static, encrypted_payload),
        );
        let (write_cipher, read_cipher) = self.noise.finish()?;
        Ok(MobileXxHandshakeOutcome {
            client_finish,
            write_cipher,
            read_cipher,
            server_certificate,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use wacore_libsignal::protocol::PrivateKey;
    use wacore_noise::NoiseState;

    fn key(seed: u8) -> KeyPair {
        KeyPair::try_from(PrivateKey::deserialize(&[seed; 32]).unwrap()).unwrap()
    }

    fn device() -> MobileDevice {
        MobileDevice::builder()
            .manufacturer("Acme".into())
            .device("Model".into())
            .os_version("1".into())
            .os_build_number("B1".into())
            .phone_id("synthetic-installation".into())
            .locale_language("en".into())
            .locale_country("US".into())
            .build()
    }

    fn profile(platform: MobilePlatform) -> MobileLoginProfile {
        MobileLoginProfile::new(
            platform,
            MobileAppVersion::parse("2.26.1").unwrap(),
            device(),
        )
        .unwrap()
    }

    fn push_varint(out: &mut Vec<u8>, mut value: u64) {
        while value > 127 {
            out.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        out.push(value as u8);
    }

    // Independent fixture writer follows the pinned .proto field numbers, not
    // the generated message serializer used by production payload construction.
    fn scalar(out: &mut Vec<u8>, field: u8, value: u64) {
        push_varint(out, u64::from(field) << 3);
        push_varint(out, value);
    }

    fn bytes(out: &mut Vec<u8>, field: u8, value: &[u8]) {
        push_varint(out, (u64::from(field) << 3) | 2);
        push_varint(out, value.len() as u64);
        out.extend_from_slice(value);
    }

    fn signed_certificate(root: &KeyPair, server_key: &[u8], expires: Option<u64>) -> Vec<u8> {
        let mut details = Vec::new();
        scalar(&mut details, 1, 7);
        bytes(&mut details, 2, b"synthetic-root");
        if let Some(expires) = expires {
            scalar(&mut details, 3, expires);
        }
        bytes(&mut details, 4, b"synthetic-server");
        bytes(&mut details, 5, server_key);
        sign_details(root, &details)
    }

    fn sign_details(root: &KeyPair, details: &[u8]) -> Vec<u8> {
        let mut random = rand::rngs::StdRng::seed_from_u64(99);
        let signature = root.calculate_signature(details, &mut random).unwrap();
        let mut encoded = Vec::new();
        bytes(&mut encoded, 1, details);
        bytes(&mut encoded, 2, &signature);
        encoded
    }

    fn trust(root: &KeyPair) -> MobileNoiseTrust {
        MobileNoiseTrust::new(root.public_key.public_key_bytes().try_into().unwrap()).unwrap()
    }

    #[test]
    fn payload_matches_independent_wire_fixture_and_preserves_omissions() {
        let mut version = Vec::new();
        scalar(&mut version, 1, 2);
        scalar(&mut version, 2, 26);
        scalar(&mut version, 3, 1);
        let mut agent = Vec::new();
        scalar(&mut agent, 1, 0); // ANDROID, including explicit zero presence.
        bytes(&mut agent, 2, &version);
        for (field, value) in [
            (5, "1"),
            (6, "Acme"),
            (7, "Model"),
            (8, "B1"),
            (9, "synthetic-installation"),
            (11, "en"),
            (12, "US"),
        ] {
            bytes(&mut agent, field, value.as_bytes());
        }
        let mut expected = Vec::new();
        scalar(&mut expected, 1, 19_990_000_001);
        bytes(&mut expected, 5, &agent);
        let profile = profile(MobilePlatform::Android);
        assert_eq!(
            profile
                .encode_payload(19_990_000_001, &MobileLoginOptions::default())
                .unwrap(),
            expected
        );
        let payload = profile
            .build_payload(19_990_000_001, &MobileLoginOptions::default())
            .unwrap();
        assert!(payload.web_info.is_unset());
        assert!(payload.device_pairing_data.is_unset());
        assert_eq!(payload.device, None);
        assert_eq!(payload.pull, None);
        assert_eq!(payload.lid_db_migrated, None);
        assert_eq!(payload.user_agent.mcc, None);
        let options = MobileLoginOptions::builder()
            .device(0)
            .passive(false)
            .pull(false)
            .build();
        let payload = profile.build_payload(19_990_000_001, &options).unwrap();
        assert_eq!(payload.device, Some(0));
        assert_eq!(payload.passive, Some(false));
        assert_eq!(payload.pull, Some(false));
    }

    #[test]
    fn profiles_select_all_mobile_products_without_web_defaults() {
        for (platform, expected) in [
            (MobilePlatform::Android, cp::user_agent::Platform::ANDROID),
            (
                MobilePlatform::AndroidBusiness,
                cp::user_agent::Platform::SMB_ANDROID,
            ),
            (MobilePlatform::Ios, cp::user_agent::Platform::IOS),
            (
                MobilePlatform::IosBusiness,
                cp::user_agent::Platform::SMB_IOS,
            ),
        ] {
            let profile = profile(platform);
            assert_eq!(profile.user_agent().platform, Some(expected));
            assert_eq!(profile.user_agent().release_channel, None);
            assert_eq!(profile.user_agent().device_type, None);
            assert_eq!(profile.user_agent().distribution_channel, None);
        }
        let version = MobileAppVersion::parse("2.26.32.84.1")
            .unwrap()
            .to_protobuf();
        assert_eq!(version.quaternary, Some(84));
        assert_eq!(version.quinary, Some(1));
        for invalid in [
            "",
            "2",
            "2.1",
            "0.1.2",
            "2.1.x",
            "2.-1.2",
            " 2.1.2",
            "2.1.2.",
            "2.1.2.3.4.5",
            "2.1.4294967296",
        ] {
            assert!(MobileAppVersion::parse(invalid).is_err(), "{invalid}");
        }
        assert!(
            profile(MobilePlatform::Android)
                .build_payload(0, &MobileLoginOptions::default())
                .is_err()
        );
        assert!(
            profile(MobilePlatform::Android)
                .build_payload(1, &MobileLoginOptions::builder().device(1).build())
                .is_err()
        );
        let mut invalid = device();
        invalid.phone_id.clear();
        assert!(
            MobileLoginProfile::new(
                MobilePlatform::Android,
                MobileAppVersion::parse("2.1.2").unwrap(),
                invalid
            )
            .is_err()
        );
    }

    #[test]
    fn explicit_options_and_installation_metadata_are_not_normalized() {
        let mut device = device();
        device.mcc = Some("123".into());
        device.mnc = Some("45".into());
        device.device_exp_id = Some("Case-Sensitive".into());
        device.distribution_channel = Some(cp::user_agent::DistributionChannel::TESTFLIGHT);
        device.device_type = Some(cp::user_agent::DeviceType::PHONE);
        let profile = MobileLoginProfile::new(
            MobilePlatform::IosBusiness,
            MobileAppVersion::parse("2.1.2").unwrap(),
            device,
        )
        .unwrap();
        let mut dns = cp::DNSSource::default();
        dns.dns_method = Some(cp::dns_source::DNSResolutionMethod::SOCKS_PROXY);
        dns.app_cached = Some(false);
        let options = MobileLoginOptions::builder()
            .session_id(123)
            .connect_type(cp::ConnectType::WIFI_UNKNOWN)
            .connect_reason(cp::ConnectReason::ERROR_RECONNECT)
            .short_connect(true)
            .connect_attempt_count(3)
            .login_counter(2)
            .oc(false)
            .product(cp::Product::WHATSAPP)
            .year_class(2020)
            .mem_class(256)
            .dns_source(dns)
            .traffic_anonymization(cp::TrafficAnonymization::OFF)
            .connection_sequence_info(5)
            .lid_db_migrated(false)
            .build();
        let payload = profile.build_payload(19_990_000_001, &options).unwrap();
        assert_eq!(
            payload.user_agent.phone_id.as_deref(),
            Some("synthetic-installation")
        );
        assert_eq!(
            payload.user_agent.device_exp_id.as_deref(),
            Some("Case-Sensitive")
        );
        assert_eq!(payload.user_agent.mnc.as_deref(), Some("45"));
        assert_eq!(
            payload.dns_source.dns_method,
            Some(cp::dns_source::DNSResolutionMethod::SOCKS_PROXY)
        );
        assert_eq!(payload.session_id, Some(123));
        assert_eq!(payload.lc, Some(2));
        assert_eq!(payload.connect_attempt_count, Some(3));
    }

    #[test]
    fn strict_flat_certificate_requires_signature_static_binding_and_freshness() {
        let root = key(11);
        let server = key(12);
        let server_public: [u8; 32] = server.public_key.public_key_bytes().try_into().unwrap();
        let encoded = signed_certificate(&root, &server_public, Some(101));
        let verified = trust(&root)
            .verify_certificate(&encoded, &server_public, 100)
            .unwrap();
        assert_eq!(verified.expires, Some(101));
        assert_eq!(verified.serial, Some(7));
        assert!(matches!(
            trust(&root).verify_certificate(&encoded, &server_public, 101),
            Err(MobileSessionError::ExpiredCertificate)
        ));
        assert!(
            trust(&key(13))
                .verify_certificate(&encoded, &server_public, 100)
                .is_err()
        );
        assert!(
            trust(&root)
                .verify_certificate(&encoded, &[9; 32], 100)
                .is_err()
        );
        let mut corrupt = encoded.clone();
        *corrupt.last_mut().unwrap() ^= 1;
        assert!(
            trust(&root)
                .verify_certificate(&corrupt, &server_public, 100)
                .is_err()
        );
        let no_expiration = signed_certificate(&root, &server_public, None);
        assert!(
            trust(&root)
                .verify_certificate(&no_expiration, &server_public, 100)
                .is_err()
        );
        assert_eq!(
            trust(&root)
                .with_expiry_policy(MobileCertificateExpiryPolicy::IfPresent)
                .verify_certificate(&no_expiration, &server_public, 100)
                .unwrap()
                .expires,
            None
        );
        assert!(
            trust(&root)
                .with_expiry_policy(MobileCertificateExpiryPolicy::IfPresent)
                .verify_certificate(&encoded, &server_public, 101)
                .is_err()
        );
        let zero_key = signed_certificate(&root, &[0; 32], Some(101));
        assert!(
            trust(&root)
                .verify_certificate(&zero_key, &[0; 32], 100)
                .is_err()
        );
        let mut unknown = encoded.clone();
        scalar(&mut unknown, 3, 1);
        assert!(
            trust(&root)
                .verify_certificate(&unknown, &server_public, 100)
                .is_err()
        );
        let mut duplicate = encoded.clone();
        bytes(&mut duplicate, 1, b"duplicate");
        assert!(
            trust(&root)
                .verify_certificate(&duplicate, &server_public, 100)
                .is_err()
        );
        let mut details = Vec::new();
        bytes(&mut details, 5, &server_public);
        scalar(&mut details, 6, 1);
        assert!(
            trust(&root)
                .verify_certificate(&sign_details(&root, &details), &server_public, 100)
                .is_err()
        );
        assert!(MobileNoiseTrust::new([0; 32]).is_err());
    }

    /// Independently drives the responder transcript with the generic Noise
    /// state and an independently encoded, root-signed mobile certificate.
    fn peer_server_hello(
        hello: &[u8],
        root: &KeyPair,
        prologue: MobileNoisePrologue,
    ) -> (Vec<u8>, NoiseState, KeyPair, Vec<u8>) {
        let hello = waproto::codec::handshake_message_decode(hello).unwrap();
        let client_ephemeral = hello.client_hello.ephemeral.as_ref().unwrap();
        let server_static = key(21);
        let server_ephemeral = key(22);
        let mut peer = NoiseState::new(
            wacore_binary::consts::NOISE_PATTERN_XX.as_bytes(),
            prologue.as_bytes(),
        )
        .unwrap();
        peer.authenticate(client_ephemeral);
        peer.authenticate(server_ephemeral.public_key.public_key_bytes());
        let client = PublicKey::from_djb_public_key_bytes(client_ephemeral).unwrap();
        peer.mix_key(&server_ephemeral.calculate_agreement(&client).unwrap())
            .unwrap();
        let encrypted_static = peer
            .encrypt(server_static.public_key.public_key_bytes())
            .unwrap();
        peer.mix_key(&server_static.calculate_agreement(&client).unwrap())
            .unwrap();
        let certificate = signed_certificate(
            root,
            server_static.public_key.public_key_bytes(),
            Some(1000),
        );
        let encrypted_certificate = peer.encrypt(&certificate).unwrap();
        let mut response = wa::HandshakeMessage::default();
        let mut server_hello = wa::handshake_message::ServerHello::default();
        server_hello.ephemeral = Some(server_ephemeral.public_key.public_key_bytes().to_vec());
        server_hello.r#static = Some(encrypted_static);
        server_hello.payload = Some(encrypted_certificate);
        response.server_hello = buffa::MessageField::some(server_hello);
        (
            waproto::codec::handshake_message_to_vec(&response),
            peer,
            server_ephemeral,
            server_static.public_key.public_key_bytes().to_vec(),
        )
    }

    #[test]
    fn mobile_xx_completes_with_independent_peer_and_both_cipher_directions() {
        let root = key(20);
        let prologue = MobileNoisePrologue::new(3);
        assert_eq!(prologue.as_bytes(), b"WA\x05\x03");
        let payload = profile(MobilePlatform::Android)
            .encode_payload(
                19_990_000_001,
                &MobileLoginOptions::builder()
                    .device(0)
                    .passive(false)
                    .build(),
            )
            .unwrap();
        let client =
            MobileXxHandshake::new(key(23), key(24), payload.clone(), prologue, trust(&root))
                .unwrap();
        let (response, mut peer, server_ephemeral, server_static) =
            peer_server_hello(&client.client_hello(), &root, prologue);
        let outcome = client.receive_server_hello(&response, 100).unwrap();
        assert_eq!(
            outcome.server_certificate.static_key.as_slice(),
            server_static
        );
        let finish = waproto::codec::handshake_message_decode(&outcome.client_finish).unwrap();
        let client_static = peer
            .decrypt(finish.client_finish.r#static.as_ref().unwrap())
            .unwrap();
        assert_eq!(client_static, key(23).public_key.public_key_bytes());
        peer.mix_key(
            &server_ephemeral
                .calculate_agreement(&PublicKey::from_djb_public_key_bytes(&client_static).unwrap())
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            peer.decrypt(finish.client_finish.payload.as_ref().unwrap())
                .unwrap(),
            payload
        );
        let peer = peer.split().unwrap();
        let mut request = outcome
            .write_cipher
            .encrypt_with_counter(0, b"synthetic request")
            .unwrap();
        // Generic Noise split yields initiator order, so the responder uses
        // the first cipher for reads and the second for writes.
        peer.write
            .decrypt_in_place_with_counter(0, &mut request)
            .unwrap();
        assert_eq!(request, b"synthetic request");
        let mut response = peer
            .read
            .encrypt_with_counter(0, b"synthetic response")
            .unwrap();
        outcome
            .read_cipher
            .decrypt_in_place_with_counter(0, &mut response)
            .unwrap();
        assert_eq!(response, b"synthetic response");
    }

    #[test]
    fn handshake_rejects_key_reuse_mismatch_and_untrusted_peer() {
        let root = key(20);
        let prologue = MobileNoisePrologue::new(3);
        assert!(MobileXxHandshake::new(key(23), key(23), vec![], prologue, trust(&root)).is_err());
        let mut mismatched = key(24);
        mismatched.public_key = key(25).public_key;
        assert!(
            MobileXxHandshake::new(key(23), mismatched, vec![], prologue, trust(&root)).is_err()
        );
        let client = MobileXxHandshake::new(
            key(23),
            key(24),
            b"test".to_vec(),
            prologue,
            trust(&key(30)),
        )
        .unwrap();
        let (response, _, _, _) = peer_server_hello(&client.client_hello(), &root, prologue);
        assert!(client.receive_server_hello(&response, 100).is_err());
        let client =
            MobileXxHandshake::new(key(23), key(24), b"test".to_vec(), prologue, trust(&root))
                .unwrap();
        assert!(client.receive_server_hello(&[0], 100).is_err());
    }

    #[test]
    fn mobile_dh_rejects_zero_and_order_one_points_at_every_stage() {
        let root = key(20);
        let prologue = MobileNoisePrologue::new(3);
        let mut order_one = [0; 32];
        order_one[0] = 1;
        for low_order in [[0; 32], order_one] {
            // Malicious first ServerHello ephemeral is rejected at ee before
            // processing the supplied encrypted fields.
            let client =
                MobileXxHandshake::new(key(23), key(24), vec![], prologue, trust(&root)).unwrap();
            let mut message = wa::HandshakeMessage::default();
            let mut server = wa::handshake_message::ServerHello::default();
            server.ephemeral = Some(low_order.to_vec());
            server.r#static = Some(vec![0; 48]);
            server.payload = Some(vec![0; 16]);
            message.server_hello = buffa::MessageField::some(server);
            assert!(matches!(
                client
                    .receive_server_hello(&waproto::codec::handshake_message_to_vec(&message), 100),
                Err(MobileSessionError::InvalidKeyMaterial)
            ));

            // ee is genuine, but the authenticated ciphertext decrypts to a
            // low-order server static. Reject es before certificate parsing.
            let client =
                MobileXxHandshake::new(key(23), key(24), vec![], prologue, trust(&root)).unwrap();
            let client_public = key(24).public_key;
            let ephemeral = key(22);
            let mut peer = NoiseState::new(
                wacore_binary::consts::NOISE_PATTERN_XX.as_bytes(),
                prologue.as_bytes(),
            )
            .unwrap();
            peer.authenticate(client_public.public_key_bytes());
            peer.authenticate(ephemeral.public_key.public_key_bytes());
            peer.mix_key(&ephemeral.calculate_agreement(&client_public).unwrap())
                .unwrap();
            let mut server = wa::handshake_message::ServerHello::default();
            server.ephemeral = Some(ephemeral.public_key.public_key_bytes().to_vec());
            server.r#static = Some(peer.encrypt(&low_order).unwrap());
            server.payload = Some(vec![0; 16]);
            message.server_hello = buffa::MessageField::some(server);
            assert!(matches!(
                client
                    .receive_server_hello(&waproto::codec::handshake_message_to_vec(&message), 100),
                Err(MobileSessionError::InvalidKeyMaterial)
            ));

            // se uses the same checked helper, independently of the earlier
            // handshake steps and without retaining the all-zero DH result.
            let mut noise =
                NoiseHandshake::new(wacore_binary::consts::NOISE_PATTERN_XX, prologue.as_bytes())
                    .unwrap();
            assert!(matches!(
                checked_agreement(&mut noise, &key(23), &low_order),
                Err(MobileSessionError::InvalidKeyMaterial)
            ));
        }
    }
}
