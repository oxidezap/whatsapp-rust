use sha2::{Digest, Sha256};
use std::fmt;

use zeroize::Zeroizing;

use super::{
    Base64Padding, EphemeralRegistrationKey, FIELD_LIMIT, RegistrationEndpointPath,
    RegistrationEnvelope, RegistrationForm, RegistrationWireError, header_value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum VerificationMethod {
    #[wire = "sms"]
    Sms,
    #[wire = "voice"]
    Voice,
    #[wire = "wa_old"]
    WhatsApp,
    #[wire = "flash"]
    Flash,
    #[wire = "email_otp"]
    Email,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum RegistrationPlatform {
    #[wire = "Android"]
    Android,
    #[wire = "SMBA"]
    AndroidBusiness,
    #[wire = "iOS"]
    Ios,
    #[wire = "SMB iOS"]
    IosBusiness,
}

impl RegistrationPlatform {
    pub fn is_android(self) -> bool {
        matches!(self, Self::Android | Self::AndroidBusiness)
    }
    pub fn is_business(self) -> bool {
        matches!(self, Self::AndroidBusiness | Self::IosBusiness)
    }
}

/// Exact version supplied from the selected application artifact/profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationAppVersion(Vec<u32>);

impl RegistrationAppVersion {
    pub fn new(parts: &[u32]) -> Result<Self, RegistrationWireError> {
        if !(3..=4).contains(&parts.len()) || parts[0] == 0 {
            return Err(RegistrationWireError::InvalidField("application version"));
        }
        Ok(Self(parts.to_vec()))
    }
}

impl fmt::Display for RegistrationAppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, part) in self.0.iter().enumerate() {
            if index != 0 {
                f.write_str(".")?;
            }
            write!(f, "{part}")?;
        }
        Ok(())
    }
}

/// Public header metadata. Additional headers encode only profile-specific differences.
pub struct RegistrationHeaderProfile {
    platform: RegistrationPlatform,
    user_agent: String,
    additional: Vec<(String, String)>,
    snapshot: HeaderSnapshot,
}

impl Drop for RegistrationHeaderProfile {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        for (_, value) in &mut self.additional {
            value.zeroize();
        }
    }
}

impl RegistrationHeaderProfile {
    pub fn new(
        platform: RegistrationPlatform,
        version: &RegistrationAppVersion,
        os_version: &str,
        manufacturer: &str,
        model: &str,
        additional: Vec<(String, String)>,
    ) -> Result<Self, RegistrationWireError> {
        for value in [os_version, model] {
            header_value(value)?;
        }
        let device = if platform.is_android() {
            header_value(manufacturer)?;
            format!("{manufacturer}-{model}")
        } else {
            model.to_owned()
        };
        let user_agent = format!("WhatsApp/{version} {platform}/{os_version} Device/{device}");
        header_value(&user_agent)?;
        if additional.len() > 32 {
            return Err(RegistrationWireError::SizeLimit);
        }
        let mut header_bytes = user_agent.len();
        for (index, (name, value)) in additional.iter().enumerate() {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                || [
                    "user-agent",
                    "content-type",
                    "content-encoding",
                    "authorization",
                    "request_token",
                    "wamsysrequest",
                    "host",
                    "content-length",
                    "transfer-encoding",
                    "connection",
                    "proxy-authorization",
                    "proxy-authenticate",
                    "proxy-connection",
                    "te",
                    "trailer",
                    "upgrade",
                    "cookie",
                    "expect",
                ]
                .iter()
                .any(|reserved| name.eq_ignore_ascii_case(reserved))
                || additional[..index]
                    .iter()
                    .any(|(old, _)| old.eq_ignore_ascii_case(name))
            {
                return Err(RegistrationWireError::InvalidField("header name"));
            }
            header_value(value)?;
            header_bytes = header_bytes
                .checked_add(name.len() + value.len())
                .ok_or(RegistrationWireError::SizeLimit)?;
            if header_bytes > 32 * 1024 {
                return Err(RegistrationWireError::SizeLimit);
            }
        }
        let snapshot = HeaderSnapshot {
            platform: match platform {
                RegistrationPlatform::Android => 0,
                RegistrationPlatform::AndroidBusiness => 1,
                RegistrationPlatform::Ios => 2,
                RegistrationPlatform::IosBusiness => 3,
            },
            version: version.0.clone(),
            os_version: os_version.to_owned(),
            manufacturer: manufacturer.to_owned(),
            model: model.to_owned(),
            additional: additional.clone(),
        };
        Ok(Self {
            platform,
            user_agent,
            additional,
            snapshot,
        })
    }

    fn headers(
        &self,
        request_token: Option<&str>,
    ) -> Result<Vec<(String, Zeroizing<String>)>, RegistrationWireError> {
        let mut headers = vec![
            ("User-Agent".into(), Zeroizing::new(self.user_agent.clone())),
            (
                "Content-Type".into(),
                Zeroizing::new("application/x-www-form-urlencoded".into()),
            ),
        ];
        if self.platform.is_android() {
            let token =
                request_token.ok_or(RegistrationWireError::MissingMaterial("request_token"))?;
            if token.len() != 36
                || !token.bytes().enumerate().all(|(i, b)| {
                    if [8, 13, 18, 23].contains(&i) {
                        b == b'-'
                    } else {
                        b.is_ascii_hexdigit()
                    }
                })
            {
                return Err(RegistrationWireError::InvalidField("request_token UUID"));
            }
            headers.push(("WaMsysRequest".into(), Zeroizing::new("1".into())));
            headers.push(("request_token".into(), Zeroizing::new(token.to_owned())));
        } else if request_token.is_some() {
            return Err(RegistrationWireError::InvalidField("iOS request_token"));
        }
        headers.extend(
            self.additional
                .iter()
                .map(|(k, v)| (k.clone(), Zeroizing::new(v.clone()))),
        );
        Ok(headers)
    }
}

/// Signal/Noise public registration identity. All key sizes are checked by type.
pub struct RegistrationIdentityFields<'a> {
    pub noise_public_key: &'a [u8; 32],
    pub registration_id: u32,
    pub identity_public_key: &'a [u8; 32],
    pub signed_prekey_id: u32,
    pub signed_prekey_public: &'a [u8; 32],
    pub signed_prekey_signature: &'a [u8; 64],
    pub experience_id: &'a [u8; 16],
    pub identity_id: &'a [u8],
}

/// Common caller-owned material, without ambient locale, country lookup or generated IDs.
pub struct RegistrationCommonFields<'a> {
    pub country_code: &'a str,
    pub national_number: &'a str,
    pub release_channel: u32,
    pub language: &'a str,
    pub locale_country: &'a str,
    pub family_device_id: &'a str,
    pub access_session_id: &'a str,
    pub registration_token: Option<&'a str>,
    pub business_certificate: Option<&'a str>,
    pub identity: RegistrationIdentityFields<'a>,
    /// Supplied push/attestation/platform values; absence is not invented evidence.
    pub platform_fields: &'a RegistrationForm,
}

/// Explicit differences between pinned reference encoders. There is no current-build default.
pub struct RegistrationEncoding {
    pub signal_padding: Base64Padding,
    pub token_on_register: bool,
    pub mirror_code_and_register_query: bool,
    pub code_order: Vec<String>,
    pub register_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttestationPolicy {
    pub require_body_attestation: bool,
    pub require_authorization: bool,
}

/// Opaque evidence supplied by a legitimate platform provider. No empty-success fallback.
pub struct BodyAttestation<'a> {
    pub body_attestation: Option<&'a str>,
    pub authorization: Option<&'a str>,
}

pub struct RegistrationRequestBuilder<'a> {
    profile: &'a RegistrationHeaderProfile,
    encoding: &'a RegistrationEncoding,
    common: RegistrationCommonFields<'a>,
}

pub enum CodeRequestPlan {
    AlreadyRequested,
    Request(RegistrationRequest),
}

/// Plaintext request awaiting encryption. Intentionally has no Debug/serde.
pub struct RegistrationRequest {
    endpoint: RegistrationEndpointPath,
    plaintext: Zeroizing<String>,
    headers: Vec<(String, Zeroizing<String>)>,
    mirror_query: bool,
}

impl<'a> RegistrationRequestBuilder<'a> {
    pub fn platform(&self) -> RegistrationPlatform {
        self.profile.platform
    }

    /// Identity binding for the exact public Noise and Signal keys in this request.
    pub fn registration_identity(&self) -> crate::mobile::RegistrationIdentity {
        crate::mobile::RegistrationIdentity::from_public_keys(
            self.common.identity.identity_public_key,
            self.common.identity.noise_public_key,
        )
    }

    /// Verify every persisted registration-key field, including the signed prekey.
    pub fn matches_device(&self, device: &crate::store::Device) -> bool {
        let identity = &self.common.identity;
        identity.registration_id == device.registration_id
            && identity.signed_prekey_id == device.signed_pre_key_id
            && identity.noise_public_key.as_slice()
                == device.noise_key.public_key.public_key_bytes()
            && identity.identity_public_key.as_slice()
                == device.identity_key.public_key.public_key_bytes()
            && identity.signed_prekey_public.as_slice()
                == device.signed_pre_key.public_key.public_key_bytes()
            && identity.signed_prekey_signature == &device.signed_pre_key_signature
    }

    /// Domain-separated, length-delimited immutable profile/material binding.
    /// OTPs, PINs, request tokens and per-attempt extras are deliberately excluded.
    /// The returned digest does not authorize untrusted storage or expose secrets.
    pub fn context_fingerprint(&self) -> Result<[u8; 32], RegistrationWireError> {
        let mut hash = Sha256::new();
        hash.update(b"whatsapp-rust/mobile-registration-context/v1");
        fn field(hash: &mut Sha256, bytes: &[u8]) {
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        field(&mut hash, self.profile.platform.as_str().as_bytes());
        field(&mut hash, self.profile.user_agent.as_bytes());
        hash.update((self.profile.additional.len() as u64).to_be_bytes());
        for (name, value) in &self.profile.additional {
            field(&mut hash, name.as_bytes());
            field(&mut hash, value.as_bytes());
        }
        hash.update([
            match self.encoding.signal_padding {
                Base64Padding::Padded => 1,
                Base64Padding::Unpadded => 0,
            },
            u8::from(self.encoding.token_on_register),
            u8::from(self.encoding.mirror_code_and_register_query),
        ]);
        for order in [&self.encoding.code_order, &self.encoding.register_order] {
            hash.update((order.len() as u64).to_be_bytes());
            for name in order {
                field(&mut hash, name.as_bytes());
            }
        }
        let c = &self.common;
        for value in [
            c.country_code,
            c.national_number,
            c.language,
            c.locale_country,
            c.family_device_id,
            c.access_session_id,
        ] {
            field(&mut hash, value.as_bytes());
        }
        hash.update(c.release_channel.to_be_bytes());
        for value in [c.registration_token, c.business_certificate] {
            hash.update([u8::from(value.is_some())]);
            if let Some(value) = value {
                field(&mut hash, value.as_bytes());
            }
        }
        let identity = &c.identity;
        hash.update(identity.registration_id.to_be_bytes());
        hash.update(identity.signed_prekey_id.to_be_bytes());
        for value in [
            identity.noise_public_key.as_slice(),
            identity.identity_public_key.as_slice(),
            identity.signed_prekey_public.as_slice(),
            identity.signed_prekey_signature.as_slice(),
            identity.experience_id.as_slice(),
            identity.identity_id,
        ] {
            field(&mut hash, value);
        }
        field(&mut hash, c.platform_fields.encode(&[])?.as_bytes());
        Ok(hash.finalize().into())
    }
    pub fn new(
        profile: &'a RegistrationHeaderProfile,
        encoding: &'a RegistrationEncoding,
        common: RegistrationCommonFields<'a>,
    ) -> Result<Self, RegistrationWireError> {
        if encoding.mirror_code_and_register_query && !profile.platform.is_android() {
            return Err(RegistrationWireError::InvalidField("iOS query mirroring"));
        }
        if !(1..=3).contains(&common.country_code.len())
            || common.country_code.starts_with('0')
            || !common.country_code.bytes().all(|b| b.is_ascii_digit())
            || common.national_number.is_empty()
            || common.national_number.len() > 15
            || common.country_code.len() + common.national_number.len() > 15
            || !common.national_number.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(RegistrationWireError::InvalidField("phone digits"));
        }
        if common.identity.signed_prekey_id > 0x00ff_ffff
            || common.identity.identity_id.is_empty()
            || common.identity.identity_id.len() > 64
        {
            return Err(RegistrationWireError::InvalidField("registration identity"));
        }
        if profile.platform.is_business() && common.business_certificate.is_none() {
            return Err(RegistrationWireError::MissingMaterial(
                "business certificate",
            ));
        }
        if !profile.platform.is_business() && common.business_certificate.is_some() {
            return Err(RegistrationWireError::InvalidField(
                "consumer business certificate",
            ));
        }
        if !uuid(common.family_device_id)
            || common.access_session_id.is_empty()
            || common.language.is_empty()
            || common.locale_country.is_empty()
            || common.business_certificate.is_some_and(str::is_empty)
        {
            return Err(RegistrationWireError::InvalidField("registration metadata"));
        }
        Ok(Self {
            profile,
            encoding,
            common,
        })
    }

    /// This lookup constructs only /exist. It never falls back to /code.
    pub fn check_account(
        &self,
        request_token: Option<&str>,
    ) -> Result<RegistrationRequest, RegistrationWireError> {
        self.build(
            RegistrationEndpointPath::Exist,
            request_token,
            None,
            None,
            &RegistrationForm::new(),
        )
    }

    /// None means the caller already requested a code elsewhere; no request is built.
    pub fn request_code(
        &self,
        method: Option<VerificationMethod>,
        request_token: Option<&str>,
        extras: &RegistrationForm,
    ) -> Result<CodeRequestPlan, RegistrationWireError> {
        let Some(method) = method else {
            return Ok(CodeRequestPlan::AlreadyRequested);
        };
        if method == VerificationMethod::Flash && !self.profile.platform.is_android() {
            return Err(RegistrationWireError::InvalidField("flash on iOS"));
        }
        self.build(
            RegistrationEndpointPath::Code,
            request_token,
            Some(method),
            None,
            extras,
        )
        .map(CodeRequestPlan::Request)
    }

    pub fn submit_code(
        &self,
        code: &str,
        request_token: Option<&str>,
        extras: &RegistrationForm,
    ) -> Result<RegistrationRequest, RegistrationWireError> {
        self.build(
            RegistrationEndpointPath::Register,
            request_token,
            None,
            Some(code),
            extras,
        )
    }

    pub fn submit_challenge(
        &self,
        answer: &str,
        request_token: Option<&str>,
        extras: &RegistrationForm,
    ) -> Result<RegistrationRequest, RegistrationWireError> {
        self.build(
            RegistrationEndpointPath::Challenge,
            request_token,
            None,
            Some(answer),
            extras,
        )
    }

    pub fn submit_pin(
        &self,
        pin: &str,
        request_token: Option<&str>,
        extras: &RegistrationForm,
    ) -> Result<RegistrationRequest, RegistrationWireError> {
        self.build(
            RegistrationEndpointPath::Security,
            request_token,
            None,
            Some(pin),
            extras,
        )
    }

    fn common_form(&self, include_token: bool) -> Result<RegistrationForm, RegistrationWireError> {
        let c = &self.common;
        let mut form = RegistrationForm::new();
        form.push_wire("cc", c.country_code)?;
        form.push_wire("in", c.national_number)?;
        form.push_wire("rc", &c.release_channel.to_string())?;
        form.push_wire("lg", c.language)?;
        form.push_wire("lc", c.locale_country)?;
        let padding = self.encoding.signal_padding;
        form.push_base64("authkey", c.identity.noise_public_key, padding)?;
        if let Some(cert) = c.business_certificate {
            form.push_wire("vname", cert)?;
        }
        form.push_base64(
            "e_regid",
            &c.identity.registration_id.to_be_bytes(),
            padding,
        )?;
        form.push_base64("e_keytype", &[5], padding)?;
        form.push_base64("e_ident", c.identity.identity_public_key, padding)?;
        form.push_base64(
            "e_skey_id",
            &c.identity.signed_prekey_id.to_be_bytes()[1..],
            padding,
        )?;
        form.push_base64("e_skey_val", c.identity.signed_prekey_public, padding)?;
        form.push_base64("e_skey_sig", c.identity.signed_prekey_signature, padding)?;
        let fdid = if self.profile.platform.is_android() {
            c.family_device_id.to_ascii_lowercase()
        } else {
            c.family_device_id.to_ascii_uppercase()
        };
        form.push_wire("fdid", &fdid)?;
        form.push_base64("expid", c.identity.experience_id, padding)?;
        form.push_percent_bytes("id", c.identity.identity_id)?;
        form.push_wire("access_session_id", c.access_session_id)?;
        if include_token {
            form.push_wire(
                "token",
                c.registration_token
                    .filter(|v| !v.is_empty())
                    .ok_or(RegistrationWireError::MissingMaterial("registration token"))?,
            )?;
        }
        form.append(c.platform_fields)?;
        Ok(form)
    }

    fn validate_stable_material(&self) -> Result<(), RegistrationWireError> {
        let _ = self.common_form(false)?;
        if let Some(token) = self.common.registration_token {
            let mut optional = RegistrationForm::new();
            optional.push_wire("token", token)?;
        }
        Ok(())
    }

    fn build(
        &self,
        endpoint: RegistrationEndpointPath,
        request_token: Option<&str>,
        method: Option<VerificationMethod>,
        code: Option<&str>,
        extras: &RegistrationForm,
    ) -> Result<RegistrationRequest, RegistrationWireError> {
        let uses_token = endpoint != RegistrationEndpointPath::Exist
            && (endpoint != RegistrationEndpointPath::Register || self.encoding.token_on_register);
        let mut form = self.common_form(uses_token)?;
        if let Some(method) = method {
            form.push_wire("method", method.as_str())?;
        }
        if let Some(code) = code {
            let code = Zeroizing::new(
                code.chars()
                    .filter(|c| !c.is_whitespace() && *c != '-')
                    .collect::<String>(),
            );
            if code.is_empty() || code.len() > 256 {
                return Err(RegistrationWireError::InvalidField("verification input"));
            }
            form.push_wire("code", &code)?;
        }
        form.append(extras)?;
        let order: &[String] = match endpoint {
            RegistrationEndpointPath::Code => &self.encoding.code_order,
            RegistrationEndpointPath::Register => &self.encoding.register_order,
            _ => &[],
        };
        let names: Vec<_> = order.iter().map(String::as_str).collect();
        let plaintext = form.encode(&names)?;
        let mirror_query = self.encoding.mirror_code_and_register_query
            && matches!(
                endpoint,
                RegistrationEndpointPath::Code | RegistrationEndpointPath::Register
            );
        if mirror_query
            && !plaintext
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-._~!$&'()*+,;=:@/?%".contains(&b))
        {
            return Err(RegistrationWireError::InvalidField("mirrored URI query"));
        }
        Ok(RegistrationRequest {
            endpoint,
            plaintext,
            headers: self.profile.headers(request_token)?,
            mirror_query,
        })
    }
}

impl RegistrationRequest {
    pub fn endpoint(&self) -> RegistrationEndpointPath {
        self.endpoint
    }
    /// Sensitive exact wire bytes, available for approved tracing/differential comparison.
    pub fn plaintext(&self) -> &str {
        &self.plaintext
    }
    pub fn seal(
        self,
        key: EphemeralRegistrationKey,
        server_public_key: &[u8; 32],
    ) -> Result<SealedRegistrationRequest, RegistrationWireError> {
        let envelope = key.seal(server_public_key, self.plaintext.as_bytes())?;
        Ok(SealedRegistrationRequest {
            request: self,
            envelope,
        })
    }
}

pub struct SealedRegistrationRequest {
    request: RegistrationRequest,
    envelope: RegistrationEnvelope,
}

/// Complete transport-neutral POST content. Query/body/authorization are sensitive.
pub struct RegistrationHttpRequest {
    endpoint: RegistrationEndpointPath,
    query: Option<Zeroizing<String>>,
    headers: Vec<(String, Zeroizing<String>)>,
    body: Zeroizing<String>,
}

impl RegistrationHttpRequest {
    pub fn endpoint(&self) -> RegistrationEndpointPath {
        self.endpoint
    }
    pub fn query(&self) -> Option<&str> {
        self.query.as_ref().map(|q| q.as_str())
    }
    pub fn headers(&self) -> impl Iterator<Item = (&str, &str)> {
        self.headers.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
    pub fn body(&self) -> &[u8] {
        self.body.as_bytes()
    }
}

/// Persistable stable registration inputs. It excludes per-attempt secrets and
/// platform-provider credentials supplied through the attestation callback.
/// Serialized bytes contain phone/token material and belong in protected storage.
/// Neither the material nor its snapshot implements Debug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationMaterialPersistence {
    /// Persist stable common fields only; opaque platform fields/headers require opt-in.
    StableOnly,
    /// Host explicitly designates all supplied platform fields/headers as durable.
    IncludeOpaquePlatformFields,
}

/// Shared registration metadata, without inventing login-only device fields.
/// HTTP model IDs may differ from the login model/marketing name by profile.
/// iOS registration HTTP headers do not carry a manufacturer.
pub struct RegistrationLoginMetadata<'a> {
    pub platform: RegistrationPlatform,
    pub app_version: &'a [u32],
    pub os_version: &'a str,
    pub registration_model: &'a str,
    pub android_manufacturer: Option<&'a str>,
    pub family_device_id: &'a str,
    pub language: &'a str,
    pub locale_country: &'a str,
    pub release_channel: u32,
}

pub struct RegistrationMaterial {
    snapshot: MaterialSnapshot,
    profile: RegistrationHeaderProfile,
    encoding: RegistrationEncoding,
    platform_fields: RegistrationForm,
    signature: [u8; 64],
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HeaderSnapshot {
    platform: u8,
    version: Vec<u32>,
    os_version: String,
    manufacturer: String,
    model: String,
    additional: Vec<(String, String)>,
}
impl Drop for HeaderSnapshot {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        for (_, value) in &mut self.additional {
            value.zeroize();
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct MaterialSnapshot {
    schema_version: u32,
    header: HeaderSnapshot,
    padded: bool,
    token_on_register: bool,
    mirror_query: bool,
    code_order: Vec<String>,
    register_order: Vec<String>,
    country_code: String,
    national_number: String,
    release_channel: u32,
    language: String,
    locale_country: String,
    family_device_id: String,
    access_session_id: String,
    registration_token: Option<String>,
    business_certificate: Option<String>,
    noise_public_key: [u8; 32],
    registration_id: u32,
    identity_public_key: [u8; 32],
    signed_prekey_id: u32,
    signed_prekey_public: [u8; 32],
    signed_prekey_signature: Vec<u8>,
    experience_id: [u8; 16],
    identity_id: Vec<u8>,
    platform_fields: Vec<(String, String)>,
}
impl Drop for MaterialSnapshot {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.country_code.zeroize();
        self.national_number.zeroize();
        self.family_device_id.zeroize();
        self.access_session_id.zeroize();
        self.registration_token.zeroize();
        self.business_certificate.zeroize();
        self.identity_id.zeroize();
        self.experience_id.zeroize();
        for (_, value) in &mut self.platform_fields {
            value.zeroize();
        }
        for (_, value) in &mut self.header.additional {
            value.zeroize();
        }
    }
}
impl RegistrationMaterial {
    const LIMIT: usize = 60 * 1024;

    pub fn capture(
        builder: &RegistrationRequestBuilder<'_>,
    ) -> Result<Self, RegistrationWireError> {
        Self::capture_with_policy(builder, RegistrationMaterialPersistence::StableOnly)
    }
    pub fn capture_with_policy(
        builder: &RegistrationRequestBuilder<'_>,
        policy: RegistrationMaterialPersistence,
    ) -> Result<Self, RegistrationWireError> {
        if policy == RegistrationMaterialPersistence::StableOnly
            && (!builder.profile.additional.is_empty()
                || !builder.common.platform_fields.encode(&[])?.is_empty())
        {
            return Err(RegistrationWireError::InvalidField(
                "opaque material persistence policy",
            ));
        }
        let c = &builder.common;
        for value in [
            c.country_code,
            c.national_number,
            c.language,
            c.locale_country,
            c.family_device_id,
            c.access_session_id,
            c.registration_token.unwrap_or(""),
            c.business_certificate.unwrap_or(""),
        ] {
            if value.len() > Self::LIMIT {
                return Err(RegistrationWireError::SizeLimit);
            }
        }
        builder.validate_stable_material()?;
        let i = &c.identity;
        let encoded = c.platform_fields.encode(&[])?;
        if encoded.len() > Self::LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        let mut platform_fields = Vec::new();
        if !encoded.is_empty() {
            for pair in encoded.split('&') {
                let (name, value) = pair
                    .split_once('=')
                    .ok_or(RegistrationWireError::InvalidField("material form"))?;
                platform_fields.push((name.to_owned(), value.to_owned()));
            }
        }
        let snapshot = MaterialSnapshot {
            schema_version: 1,
            header: builder.profile.snapshot.clone(),
            padded: builder.encoding.signal_padding == Base64Padding::Padded,
            token_on_register: builder.encoding.token_on_register,
            mirror_query: builder.encoding.mirror_code_and_register_query,
            code_order: builder.encoding.code_order.clone(),
            register_order: builder.encoding.register_order.clone(),
            country_code: c.country_code.to_owned(),
            national_number: c.national_number.to_owned(),
            release_channel: c.release_channel,
            language: c.language.to_owned(),
            locale_country: c.locale_country.to_owned(),
            family_device_id: c.family_device_id.to_owned(),
            access_session_id: c.access_session_id.to_owned(),
            registration_token: c.registration_token.map(str::to_owned),
            business_certificate: c.business_certificate.map(str::to_owned),
            noise_public_key: *i.noise_public_key,
            registration_id: i.registration_id,
            identity_public_key: *i.identity_public_key,
            signed_prekey_id: i.signed_prekey_id,
            signed_prekey_public: *i.signed_prekey_public,
            signed_prekey_signature: i.signed_prekey_signature.to_vec(),
            experience_id: *i.experience_id,
            identity_id: i.identity_id.to_vec(),
            platform_fields,
        };
        let material = Self::from_snapshot(snapshot)?;
        // Enforce the durable representation budget before any host side effect.
        let _ = material.encode()?;
        Ok(material)
    }

    pub fn encode(&self) -> Result<Zeroizing<Vec<u8>>, RegistrationWireError> {
        let bytes = Zeroizing::new(
            serde_json::to_vec(&self.snapshot)
                .map_err(|_| RegistrationWireError::InvalidField("material snapshot"))?,
        );
        if bytes.len() > Self::LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        Ok(bytes)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self, RegistrationWireError> {
        if bytes.len() > Self::LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        let snapshot = serde_json::from_slice(bytes)
            .map_err(|_| RegistrationWireError::InvalidField("material snapshot"))?;
        Self::from_snapshot(snapshot)
    }
    fn from_snapshot(snapshot: MaterialSnapshot) -> Result<Self, RegistrationWireError> {
        if snapshot.schema_version != 1
            || snapshot.code_order.len() > FIELD_LIMIT
            || snapshot.register_order.len() > FIELD_LIMIT
            || snapshot.platform_fields.len() > FIELD_LIMIT
        {
            return Err(RegistrationWireError::InvalidField("material snapshot"));
        }
        let platform = match snapshot.header.platform {
            0 => RegistrationPlatform::Android,
            1 => RegistrationPlatform::AndroidBusiness,
            2 => RegistrationPlatform::Ios,
            3 => RegistrationPlatform::IosBusiness,
            _ => return Err(RegistrationWireError::InvalidField("material platform")),
        };
        let version = RegistrationAppVersion::new(&snapshot.header.version)?;
        let profile = RegistrationHeaderProfile::new(
            platform,
            &version,
            &snapshot.header.os_version,
            &snapshot.header.manufacturer,
            &snapshot.header.model,
            snapshot.header.additional.clone(),
        )?;
        let encoding = RegistrationEncoding {
            signal_padding: if snapshot.padded {
                Base64Padding::Padded
            } else {
                Base64Padding::Unpadded
            },
            token_on_register: snapshot.token_on_register,
            mirror_code_and_register_query: snapshot.mirror_query,
            code_order: snapshot.code_order.clone(),
            register_order: snapshot.register_order.clone(),
        };
        let mut platform_fields = RegistrationForm::new();
        for (name, value) in &snapshot.platform_fields {
            platform_fields.push_wire(name, value)?;
        }
        let signature = snapshot
            .signed_prekey_signature
            .as_slice()
            .try_into()
            .map_err(|_| RegistrationWireError::InvalidField("material signature"))?;
        let material = Self {
            snapshot,
            profile,
            encoding,
            platform_fields,
            signature,
        };
        // Re-run normal construction validation rather than trust serialized fields.
        material.builder()?.validate_stable_material()?;
        Ok(material)
    }
    pub fn login_metadata(&self) -> RegistrationLoginMetadata<'_> {
        let s = &self.snapshot;
        RegistrationLoginMetadata {
            platform: self.profile.platform,
            app_version: &s.header.version,
            os_version: &s.header.os_version,
            registration_model: &s.header.model,
            android_manufacturer: self
                .profile
                .platform
                .is_android()
                .then_some(s.header.manufacturer.as_str()),
            family_device_id: &s.family_device_id,
            language: &s.language,
            locale_country: &s.locale_country,
            release_channel: s.release_channel,
        }
    }

    /// Validated submitted number, retained when the server omits its canonical
    /// login echo (Cobalt saveRegistrationStatus; whalibmob adoptCanonicalNumber).
    pub fn submitted_phone_number(&self) -> Zeroizing<String> {
        Zeroizing::new(format!(
            "{}{}",
            self.snapshot.country_code, self.snapshot.national_number
        ))
    }

    pub fn builder(&self) -> Result<RegistrationRequestBuilder<'_>, RegistrationWireError> {
        let s = &self.snapshot;
        RegistrationRequestBuilder::new(
            &self.profile,
            &self.encoding,
            RegistrationCommonFields {
                country_code: &s.country_code,
                national_number: &s.national_number,
                release_channel: s.release_channel,
                language: &s.language,
                locale_country: &s.locale_country,
                family_device_id: &s.family_device_id,
                access_session_id: &s.access_session_id,
                registration_token: s.registration_token.as_deref(),
                business_certificate: s.business_certificate.as_deref(),
                platform_fields: &self.platform_fields,
                identity: RegistrationIdentityFields {
                    noise_public_key: &s.noise_public_key,
                    registration_id: s.registration_id,
                    identity_public_key: &s.identity_public_key,
                    signed_prekey_id: s.signed_prekey_id,
                    signed_prekey_public: &s.signed_prekey_public,
                    signed_prekey_signature: &self.signature,
                    experience_id: &s.experience_id,
                    identity_id: &s.identity_id,
                },
            },
        )
    }
}

/// Extract a flash-call code from host-supplied caller ID, with explicit length.
///
/// Source: whalibmob 8f50db4c586534dc0af4d13b26bdd09ef42660c0,
/// Registration.js applyFlashFilter/flashCodeFromCallerId (2060-2090).
/// The supported server filter is `(.*)<literal decimal prefix>(.*)`; arbitrary
/// regexes fail closed. This uses bounded string matching, with no regex engine,
/// ambient configuration, telephony access or automatic code submission.
pub fn flash_code_from_caller_id(
    input: &str,
    length: usize,
    filter: Option<&str>,
) -> Result<Zeroizing<String>, RegistrationWireError> {
    if input.len() > 512 {
        return Err(RegistrationWireError::SizeLimit);
    }
    if !(1..=32).contains(&length) {
        return Err(RegistrationWireError::InvalidField("flash code length"));
    }
    let digits = Zeroizing::new(
        input
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>(),
    );
    if digits.is_empty() {
        return Err(RegistrationWireError::InvalidField("flash caller ID"));
    }
    let base = if let Some(filter) = filter {
        if filter.len() > 128 {
            return Err(RegistrationWireError::SizeLimit);
        }
        let prefix = filter
            .strip_prefix("(.*)")
            .and_then(|v| v.strip_suffix("(.*)"))
            .filter(|v| !v.is_empty() && v.len() <= 32 && v.bytes().all(|b| b.is_ascii_digit()))
            .ok_or(RegistrationWireError::InvalidField(
                "unsupported flash filter",
            ))?;
        if let Some(offset) = digits.rfind(prefix) {
            let tail = &digits[offset + prefix.len()..];
            if !tail.is_empty() {
                tail
            } else if offset > 0 {
                &digits[..offset]
            } else {
                digits.as_str()
            }
        } else {
            digits.as_str()
        }
    } else {
        &digits
    };
    Ok(Zeroizing::new(
        base[base.len().saturating_sub(length)..].to_owned(),
    ))
}

fn uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

impl SealedRegistrationRequest {
    pub fn endpoint(&self) -> RegistrationEndpointPath {
        self.request.endpoint
    }
    pub fn attestation_input(&self) -> &[u8] {
        self.envelope.encoded().as_bytes()
    }
    pub fn query(&self) -> Option<&str> {
        self.request.mirror_query.then_some(&self.request.plaintext)
    }
    pub fn finish(
        mut self,
        policy: AttestationPolicy,
        attestation: BodyAttestation<'_>,
    ) -> Result<RegistrationHttpRequest, RegistrationWireError> {
        if policy.require_body_attestation && attestation.body_attestation.is_none_or(str::is_empty)
        {
            return Err(RegistrationWireError::MissingMaterial("body attestation"));
        }
        if policy.require_authorization && attestation.authorization.is_none_or(str::is_empty) {
            return Err(RegistrationWireError::MissingMaterial("authorization"));
        }
        let mut body = Zeroizing::new(format!("ENC={}", self.envelope.encoded()));
        if let Some(suffix) = attestation.body_attestation {
            if suffix.is_empty()
                || suffix.len() > 64 * 1024
                || suffix.contains('&')
                || suffix.chars().any(char::is_control)
            {
                return Err(RegistrationWireError::InvalidField("body attestation"));
            }
            body.push_str("&H=");
            body.push_str(suffix);
        }
        if let Some(auth) = attestation.authorization {
            header_value(auth)?;
            self.request
                .headers
                .push(("Authorization".into(), Zeroizing::new(auth.to_owned())));
        }
        Ok(RegistrationHttpRequest {
            endpoint: self.request.endpoint,
            query: self.request.mirror_query.then_some(self.request.plaintext),
            headers: self.request.headers,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const UUID: &str = "12345678-1234-1234-1234-123456789abc";
    fn profile(
        additional: Vec<(String, String)>,
    ) -> Result<RegistrationHeaderProfile, RegistrationWireError> {
        RegistrationHeaderProfile::new(
            RegistrationPlatform::Android,
            &RegistrationAppVersion::new(&[2, 1, 2, 3]).unwrap(),
            "1",
            "Synthetic",
            "Fixture",
            additional,
        )
    }
    fn common<'a>(fields: &'a RegistrationForm) -> RegistrationCommonFields<'a> {
        RegistrationCommonFields {
            country_code: "999",
            national_number: "01000000",
            release_channel: 0,
            language: "en",
            locale_country: "ZZ",
            family_device_id: UUID,
            access_session_id: "synthetic-session",
            registration_token: Some("synthetic-token"),
            business_certificate: None,
            platform_fields: fields,
            identity: RegistrationIdentityFields {
                noise_public_key: &[1; 32],
                registration_id: 1,
                identity_public_key: &[2; 32],
                signed_prekey_id: 7,
                signed_prekey_public: &[3; 32],
                signed_prekey_signature: &[4; 64],
                experience_id: &[5; 16],
                identity_id: &[0, 255],
            },
        }
    }
    fn encoding() -> RegistrationEncoding {
        RegistrationEncoding {
            signal_padding: Base64Padding::Padded,
            token_on_register: true,
            mirror_code_and_register_query: true,
            code_order: vec![],
            register_order: vec![],
        }
    }
    #[test]
    fn rejects_transport_headers_and_metadata_exhaustion() {
        for name in [
            "Host",
            "Content-Length",
            "Transfer-Encoding",
            "Connection",
            "Proxy-Authorization",
            "User-Agent",
            "request_token",
        ] {
            assert!(profile(vec![(name.into(), "test".into())]).is_err());
        }
        assert!(profile((0..33).map(|i| (format!("X-{i}"), "a".into())).collect()).is_err());
        assert!(
            profile(vec![
                ("X-A".into(), "a".repeat(16000)),
                ("X-B".into(), "b".repeat(16000)),
                ("X-C".into(), "c".repeat(16000))
            ])
            .is_err()
        );
        assert!(profile(vec![("x".repeat(65), "a".into())]).is_err());
    }
    #[test]
    fn phone_shape_and_external_code_never_trigger_otp() {
        let fields = RegistrationForm::new();
        let profile = profile(vec![]).unwrap();
        let encoding = encoding();
        for (cc, number) in [("000", "1"), ("999", "1234567890123")] {
            let mut common = common(&fields);
            common.country_code = cc;
            common.national_number = number;
            assert!(RegistrationRequestBuilder::new(&profile, &encoding, common).is_err());
        }
        let builder =
            RegistrationRequestBuilder::new(&profile, &encoding, common(&fields)).unwrap();
        assert!(matches!(
            builder.request_code(None, None, &fields).unwrap(),
            CodeRequestPlan::AlreadyRequested
        ));
        let lookup = builder.check_account(Some(UUID)).unwrap();
        assert_eq!(lookup.endpoint(), RegistrationEndpointPath::Exist);
        assert!(lookup.plaintext().contains("in=01000000"));
        assert!(lookup.plaintext().contains("id=%00%FF"));
        assert!(!lookup.plaintext().contains("method="));
    }
    #[test]
    fn sealed_request_retains_query_and_requires_real_supplied_attestation() {
        let fields = RegistrationForm::new();
        let profile = profile(vec![]).unwrap();
        let encoding = encoding();
        let builder =
            RegistrationRequestBuilder::new(&profile, &encoding, common(&fields)).unwrap();
        let mut peer = [0; 32];
        peer[0] = 9;
        let request = builder
            .submit_code("12-3 456", Some(UUID), &fields)
            .unwrap();
        assert!(request.plaintext().ends_with("code=123456"));
        let sealed = request
            .seal(EphemeralRegistrationKey::from_bytes([7; 32]), &peer)
            .unwrap();
        let expected = sealed.query().unwrap().to_owned();
        assert!(
            sealed
                .finish(
                    AttestationPolicy {
                        require_body_attestation: true,
                        require_authorization: false
                    },
                    BodyAttestation {
                        body_attestation: None,
                        authorization: None
                    }
                )
                .is_err()
        );
        let request = builder.submit_code("123456", Some(UUID), &fields).unwrap();
        let sealed = request
            .seal(EphemeralRegistrationKey::from_bytes([8; 32]), &peer)
            .unwrap();
        let result = sealed
            .finish(
                AttestationPolicy {
                    require_body_attestation: true,
                    require_authorization: true,
                },
                BodyAttestation {
                    body_attestation: Some("synthetic-proof"),
                    authorization: Some("synthetic-authorization"),
                },
            )
            .unwrap();
        assert_eq!(result.query(), Some(expected.as_str()));
        assert!(result.body().starts_with(b"ENC="));
        assert!(result.body().ends_with(b"&H=synthetic-proof"));
        assert!(
            result
                .headers()
                .any(|(name, value)| name == "Authorization" && value == "synthetic-authorization")
        );
    }
    #[test]
    fn context_binding_covers_profile_registration_keys_and_stable_ids() {
        let fields = RegistrationForm::new();
        let header = profile(vec![]).unwrap();
        let encoding = encoding();
        let baseline = RegistrationRequestBuilder::new(&header, &encoding, common(&fields))
            .unwrap()
            .context_fingerprint()
            .unwrap();
        let mut changed = common(&fields);
        changed.identity.registration_id += 1;
        assert_ne!(
            baseline,
            RegistrationRequestBuilder::new(&header, &encoding, changed)
                .unwrap()
                .context_fingerprint()
                .unwrap()
        );
        let mut changed = common(&fields);
        changed.identity.signed_prekey_id += 1;
        assert_ne!(
            baseline,
            RegistrationRequestBuilder::new(&header, &encoding, changed)
                .unwrap()
                .context_fingerprint()
                .unwrap()
        );
        let mut changed = common(&fields);
        changed.access_session_id = "another-synthetic-session";
        assert_ne!(
            baseline,
            RegistrationRequestBuilder::new(&header, &encoding, changed)
                .unwrap()
                .context_fingerprint()
                .unwrap()
        );
        let mut different_encoding = self::encoding();
        different_encoding.token_on_register = !different_encoding.token_on_register;
        assert_ne!(
            baseline,
            RegistrationRequestBuilder::new(&header, &different_encoding, common(&fields))
                .unwrap()
                .context_fingerprint()
                .unwrap()
        );
    }
    #[test]
    fn owned_material_roundtrip_preserves_exact_wire_and_rejects_implicit_opaque_persistence() {
        let fields = RegistrationForm::new();
        let header = profile(vec![]).unwrap();
        let encoding = encoding();
        let builder = RegistrationRequestBuilder::new(&header, &encoding, common(&fields)).unwrap();
        let material = RegistrationMaterial::capture(&builder).unwrap();
        let bytes = material.encode().unwrap();
        let restored = RegistrationMaterial::decode(&bytes).unwrap();
        assert!(matches!(
            restored.builder().unwrap().check_account(None),
            Err(RegistrationWireError::MissingMaterial("request_token"))
        ));
        assert!(
            restored
                .builder()
                .unwrap()
                .check_account(Some(UUID))
                .is_ok()
        );
        assert_eq!(
            builder.context_fingerprint().unwrap(),
            restored.builder().unwrap().context_fingerprint().unwrap()
        );
        assert_eq!(
            builder.check_account(Some(UUID)).unwrap().plaintext(),
            restored
                .builder()
                .unwrap()
                .check_account(Some(UUID))
                .unwrap()
                .plaintext()
        );
        let mut future: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        future["schema_version"] = serde_json::json!(2);
        assert!(RegistrationMaterial::decode(&serde_json::to_vec(&future).unwrap()).is_err());
        let mut opaque = RegistrationForm::new();
        opaque
            .push_wire("gpia", "synthetic-platform-material")
            .unwrap();
        let builder = RegistrationRequestBuilder::new(&header, &encoding, common(&opaque)).unwrap();
        assert!(RegistrationMaterial::capture(&builder).is_err());
        assert!(
            RegistrationMaterial::capture_with_policy(
                &builder,
                RegistrationMaterialPersistence::IncludeOpaquePlatformFields
            )
            .is_ok()
        );
    }
    #[test]
    fn flash_extraction_is_bounded_and_never_guesses_an_unknown_filter() {
        assert_eq!(
            &*flash_code_from_caller_id("+39 373 9799312", 6, Some("(.*)373(.*)")).unwrap(),
            "799312"
        );
        assert_eq!(
            &*flash_code_from_caller_id("12-34", 6, None).unwrap(),
            "1234"
        );
        assert_eq!(
            &*flash_code_from_caller_id("+40 21 555 123456", 6, None).unwrap(),
            "123456"
        );
        assert!(flash_code_from_caller_id("123456", 6, Some("(a+)+$")).is_err());
        assert_eq!(
            &*flash_code_from_caller_id("123456", 6, Some("(.*)999(.*)")).unwrap(),
            "123456"
        );
        assert!(flash_code_from_caller_id("123456", 0, None).is_err());
        assert!(flash_code_from_caller_id(&"1".repeat(513), 6, None).is_err());
    }
}
