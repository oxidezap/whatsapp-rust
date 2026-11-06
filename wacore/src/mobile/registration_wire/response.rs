use std::collections::BTreeMap;
use std::fmt;

use base64::Engine as _;
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

use super::{RESPONSE_LIMIT, RegistrationEndpointPath, RegistrationWireError, VerificationMethod};

#[derive(Clone, PartialEq, Eq, crate::WireEnum)]
pub enum RegistrationResponseStatus {
    #[wire = "ok"]
    Ok,
    #[wire = "sent"]
    Sent,
    #[wire = "verified"]
    Verified,
    #[wire = "fail"]
    Fail,
    #[wire_fallback]
    Unknown(String),
}

impl fmt::Debug for RegistrationResponseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(_) => f.write_str("Unknown([redacted])"),
            known => f.write_str(known.as_str()),
        }
    }
}

struct UniqueObject(Map<String, Value>);
impl<'de> Deserialize<'de> for UniqueObject {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> Visitor<'de> for ObjectVisitor {
            type Value = UniqueObject;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a registration response object with unique keys")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut object = Map::new();
                while let Some((key, value)) = access.next_entry::<String, Value>()? {
                    if object.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom("duplicate response field"));
                    }
                }
                Ok(UniqueObject(object))
            }
        }
        deserializer.deserialize_map(ObjectVisitor)
    }
}

/// Decoded caller-visible challenge inputs, never an automatically solved CAPTCHA.
pub struct RegistrationChallenge {
    pub image: Option<Vec<u8>>,
    pub audio: Option<Vec<u8>>,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct RegistrationWaitHints(BTreeMap<String, i64>);

impl fmt::Debug for RegistrationWaitHints {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistrationWaitHints")
            .field("count", &self.0.len())
            .finish_non_exhaustive()
    }
}

impl RegistrationWaitHints {
    pub fn seconds_for(&self, method: VerificationMethod) -> Option<u64> {
        self.0
            .get(&format!("{}_wait", method.as_str()))
            .and_then(|v| u64::try_from(*v).ok())
    }
    pub fn maximum_seconds(&self) -> Option<u64> {
        self.0.values().filter_map(|v| u64::try_from(*v).ok()).max()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, i64)> {
        self.0.iter().map(|(k, v)| (k.as_str(), *v))
    }
}

/// Known verification-route hints. No entered OTP/PIN or challenge bytes are held.
/// Strings may include telephone prefixes and deliberately have no Debug output.
#[derive(Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "DeliveryHintsWire")]
pub struct RegistrationDeliveryHints {
    code_length: Option<u8>,
    flash_filter: Option<String>,
    flash_prefix: Option<String>,
    flash_country_code: Option<String>,
    flash_timeout_seconds: Option<u32>,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct DeliveryHintsWire {
    code_length: Option<u8>,
    flash_filter: Option<String>,
    flash_prefix: Option<String>,
    flash_country_code: Option<String>,
    flash_timeout_seconds: Option<u32>,
}
impl TryFrom<DeliveryHintsWire> for RegistrationDeliveryHints {
    type Error = RegistrationWireError;
    fn try_from(value: DeliveryHintsWire) -> Result<Self, Self::Error> {
        Self::new(
            value.code_length,
            value.flash_filter,
            value.flash_prefix,
            value.flash_country_code,
            value.flash_timeout_seconds,
        )
    }
}
impl Drop for RegistrationDeliveryHints {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.flash_filter.zeroize();
        self.flash_prefix.zeroize();
        self.flash_country_code.zeroize();
    }
}
impl RegistrationDeliveryHints {
    pub fn new(
        code_length: Option<u8>,
        flash_filter: Option<String>,
        flash_prefix: Option<String>,
        flash_country_code: Option<String>,
        flash_timeout_seconds: Option<u32>,
    ) -> Result<Self, RegistrationWireError> {
        let value = Self {
            code_length,
            flash_filter,
            flash_prefix,
            flash_country_code,
            flash_timeout_seconds,
        };
        if value.code_length.is_some_and(|v| !(1..=32).contains(&v))
            || value.flash_timeout_seconds == Some(0)
            || [
                &value.flash_filter,
                &value.flash_prefix,
                &value.flash_country_code,
            ]
            .iter()
            .zip([128, 64, 64])
            .any(|(value, limit)| {
                value.as_ref().is_some_and(|v| {
                    v.is_empty() || v.len() > limit || v.chars().any(char::is_control)
                })
            })
        {
            return Err(RegistrationWireError::InvalidResponseField(
                "delivery hint".into(),
            ));
        }
        Ok(value)
    }
    pub fn code_length(&self) -> Option<u8> {
        self.code_length
    }
    pub fn flash_filter(&self) -> Option<&str> {
        self.flash_filter.as_deref()
    }
    pub fn flash_prefix(&self) -> Option<&str> {
        self.flash_prefix.as_deref()
    }
    pub fn flash_country_code(&self) -> Option<&str> {
        self.flash_country_code.as_deref()
    }
    pub fn flash_timeout_seconds(&self) -> Option<u32> {
        self.flash_timeout_seconds
    }
}

fn delivery_hints(
    object: &mut Map<String, Value>,
) -> Result<Option<RegistrationDeliveryHints>, RegistrationWireError> {
    // whalibmob 8f50db4 Registration.js2473-2479: only positive length/timeout
    // are retained; empty optional strings are absent. Unknown data is not saved.
    let code_length = object
        .remove("length")
        .map(wait_seconds)
        .transpose()?
        .flatten()
        .filter(|v| *v > 0)
        .map(u8::try_from)
        .transpose()
        .map_err(|_| RegistrationWireError::InvalidResponseField("delivery hint".into()))?;
    let timeout = object
        .remove("flash_timeout")
        .map(wait_seconds)
        .transpose()?
        .flatten()
        .filter(|v| *v > 0)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| RegistrationWireError::InvalidResponseField("delivery hint".into()))?;
    let filter = string(object, "cli_filter")?.filter(|v| !v.is_empty());
    let prefix = string(object, "cli_prefix")?.filter(|v| !v.is_empty());
    let country = string(object, "cli_cc")?.filter(|v| !v.is_empty());
    if code_length.is_none()
        && timeout.is_none()
        && filter.is_none()
        && prefix.is_none()
        && country.is_none()
    {
        return Ok(None);
    }
    RegistrationDeliveryHints::new(code_length, filter, prefix, country, timeout).map(Some)
}

/// Parsed response. Login, challenge and unknown data may be sensitive; no serde/Debug dump.
pub struct RegistrationResponse {
    status: Option<RegistrationResponseStatus>,
    reason: Option<String>,
    login: Option<String>,
    challenge: Option<RegistrationChallenge>,
    waits: RegistrationWaitHints,
    retry_after: Option<i64>,
    delivery_hints: Option<RegistrationDeliveryHints>,
    pending: Option<String>,
    blocked: bool,
    unknown: Map<String, Value>,
}

impl fmt::Debug for RegistrationResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegistrationResponse")
            .field("status_present", &self.status.is_some())
            .field("reason_present", &self.reason.is_some())
            .field("challenge_present", &self.challenge.is_some())
            .field("wait_hint_count", &self.waits.0.len())
            .field("unknown_field_count", &self.unknown.len())
            .finish_non_exhaustive()
    }
}

/// Endpoint-aware semantic decisions derived from the inspected reference branches.
/// Unknown status/reasons remain unknown rather than success-shaped defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RegistrationResponseDecision {
    AccountKeysDiffer,
    AccountRecognized,
    CodeSent,
    Registered,
    ChallengeRequired,
    PinRequired,
    RateLimited,
    NoRoute,
    RequiredInput,
    Blocked,
    Rejected,
    Unknown,
}

fn string(
    object: &mut Map<String, Value>,
    name: &str,
) -> Result<Option<String>, RegistrationWireError> {
    match object.remove(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        _ => Err(RegistrationWireError::InvalidResponseField(name.into())),
    }
}

fn wait_seconds(value: Value) -> Result<Option<i64>, RegistrationWireError> {
    let seconds = match value {
        Value::Null => return Ok(None),
        Value::Number(value) => value.as_i64(),
        Value::String(value)
            if !value.is_empty()
                && value
                    .strip_prefix('-')
                    .unwrap_or(&value)
                    .bytes()
                    .all(|b| b.is_ascii_digit()) =>
        {
            value.parse().ok()
        }
        _ => None,
    }
    .ok_or_else(|| RegistrationWireError::InvalidResponseField("wait hint".into()))?;
    Ok(Some(seconds))
}

fn challenge(value: Option<String>) -> Result<Option<Vec<u8>>, RegistrationWireError> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    if value.len() > RESPONSE_LIMIT {
        return Err(RegistrationWireError::SizeLimit);
    }
    for engine in [
        &base64::engine::general_purpose::STANDARD,
        &base64::engine::general_purpose::STANDARD_NO_PAD,
        &base64::engine::general_purpose::URL_SAFE,
        &base64::engine::general_purpose::URL_SAFE_NO_PAD,
    ] {
        if let Ok(decoded) = engine.decode(&value) {
            return Ok(Some(decoded));
        }
    }
    Err(RegistrationWireError::InvalidChallenge)
}

impl RegistrationResponse {
    pub fn parse(bytes: &[u8]) -> Result<Self, RegistrationWireError> {
        if bytes.is_empty() || bytes.len() > RESPONSE_LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        let UniqueObject(mut object) =
            serde_json::from_slice(bytes).map_err(|_| RegistrationWireError::InvalidJson)?;
        let status = string(&mut object, "status")?
            .map(|value| RegistrationResponseStatus::from(value.as_str()));
        let reason = string(&mut object, "reason")?;
        let pending = string(&mut object, "pending")?;
        let login = match object.remove("login") {
            None | Some(Value::Null) => None,
            Some(Value::String(value))
                if !value.is_empty()
                    && value.len() <= 20
                    && value.bytes().all(|b| b.is_ascii_digit()) =>
            {
                Some(value)
            }
            Some(Value::Number(value)) if value.as_u64().is_some() => Some(value.to_string()),
            _ => return Err(RegistrationWireError::InvalidResponseField("login".into())),
        };
        let image = challenge(string(&mut object, "image_blob")?)?;
        let audio = challenge(string(&mut object, "audio_blob")?)?;
        let challenge =
            (image.is_some() || audio.is_some()).then_some(RegistrationChallenge { image, audio });
        let mut waits = RegistrationWaitHints::default();
        let wait_keys: Vec<_> = object
            .keys()
            .filter(|key| key.ends_with("_wait"))
            .cloned()
            .collect();
        for key in wait_keys {
            let value = object
                .remove(&key)
                .ok_or(RegistrationWireError::InvalidJson)?;
            if let Some(seconds) = wait_seconds(value)? {
                waits.0.insert(key, seconds);
            }
        }
        let retry_after = object
            .remove("retry_after")
            .map(wait_seconds)
            .transpose()?
            .flatten();
        let delivery_hints = delivery_hints(&mut object)?;
        let blocked = match object.get("custom_block_screen") {
            None | Some(Value::Null) | Some(Value::Bool(false)) => false,
            Some(Value::Object(_)) | Some(Value::Bool(true)) => true,
            _ => {
                return Err(RegistrationWireError::InvalidResponseField(
                    "custom_block_screen".into(),
                ));
            }
        };
        Ok(Self {
            status,
            reason,
            login,
            challenge,
            waits,
            retry_after,
            delivery_hints,
            pending,
            blocked,
            unknown: object,
        })
    }

    pub fn status(&self) -> Option<&RegistrationResponseStatus> {
        self.status.as_ref()
    }
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
    pub fn canonical_login(&self) -> Option<&str> {
        self.login.as_deref()
    }
    pub fn challenge(&self) -> Option<&RegistrationChallenge> {
        self.challenge.as_ref()
    }
    pub fn waits(&self) -> &RegistrationWaitHints {
        &self.waits
    }
    pub fn delivery_hints(&self) -> Option<&RegistrationDeliveryHints> {
        self.delivery_hints.as_ref()
    }

    /// Signed server value; negative sentinels are retained, never clamped to zero.
    pub fn retry_after(&self) -> Option<i64> {
        self.retry_after
    }

    /// Source-aligned hint selection (whalibmob 8f50db4, Registration.js1347-1364):
    /// positive preferred-method wait, maximum positive known method wait, then
    /// positive retry_after. Merely inspecting this never schedules a retry.
    pub fn wait_hint_seconds(&self, preferred: Option<VerificationMethod>) -> Option<u64> {
        if let Some(seconds) = preferred
            .and_then(|method| self.waits.seconds_for(method))
            .filter(|v| *v > 0)
        {
            return Some(seconds);
        }
        let known = [
            "sms_wait",
            "voice_wait",
            "wa_old_wait",
            "flash_wait",
            "email_otp_wait",
            "send_sms_wait",
            "silent_auth_wait",
        ];
        known
            .iter()
            .filter_map(|key| self.waits.0.get(*key).copied())
            .filter_map(|v| u64::try_from(v).ok())
            .filter(|v| *v > 0)
            .max()
            .or_else(|| {
                self.retry_after
                    .and_then(|v| u64::try_from(v).ok())
                    .filter(|v| *v > 0)
            })
    }
    pub fn pending(&self) -> Option<&str> {
        self.pending.as_deref()
    }
    pub fn unknown_fields(&self) -> &Map<String, Value> {
        &self.unknown
    }

    pub fn decision(&self, endpoint: RegistrationEndpointPath) -> RegistrationResponseDecision {
        use RegistrationResponseDecision as D;
        if self.blocked {
            return D::Blocked;
        }
        // A finalisation input gate cannot be promoted by a success-like status.
        if self.pending.is_some() || self.reason.as_deref() == Some("consent") {
            return D::RequiredInput;
        }
        if self.challenge.is_some() {
            return D::ChallengeRequired;
        }
        let reason = self.reason.as_deref().unwrap_or("");
        if ["2fa_required", "security_code", "two_factor_required"]
            .iter()
            .any(|v| reason.eq_ignore_ascii_case(v))
        {
            return D::PinRequired;
        }
        if [
            "too_recent",
            "too_many",
            "too_many_guesses",
            "too_many_all_methods",
        ]
        .iter()
        .any(|v| {
            reason.eq_ignore_ascii_case(v)
                || self
                    .status
                    .as_ref()
                    .is_some_and(|status| status.as_str().eq_ignore_ascii_case(v))
        }) {
            return D::RateLimited;
        }
        if reason.eq_ignore_ascii_case("no_routes")
            || self
                .status
                .as_ref()
                .is_some_and(|status| status.as_str().eq_ignore_ascii_case("no_routes"))
        {
            return D::NoRoute;
        }
        let success = matches!(
            self.status,
            Some(
                RegistrationResponseStatus::Ok
                    | RegistrationResponseStatus::Sent
                    | RegistrationResponseStatus::Verified
            )
        );
        if success && !reason.is_empty() {
            // Contradictory or newly introduced server conditions require a
            // caller decision rather than silently accepting a status label.
            return D::Unknown;
        }
        if success {
            return match endpoint {
                RegistrationEndpointPath::Code => D::CodeSent,
                RegistrationEndpointPath::Register
                | RegistrationEndpointPath::Challenge
                | RegistrationEndpointPath::Security => D::Registered,
                RegistrationEndpointPath::Exist => {
                    if matches!(self.status, Some(RegistrationResponseStatus::Ok)) {
                        D::AccountRecognized
                    } else {
                        D::Unknown
                    }
                }
            };
        }
        if endpoint == RegistrationEndpointPath::Exist && reason == "incorrect" {
            return D::AccountKeysDiffer;
        }
        if matches!(self.status, Some(RegistrationResponseStatus::Fail)) {
            D::Rejected
        } else {
            D::Unknown
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn response_decisions_are_endpoint_scoped_and_unknowns_preserved() {
        let r = RegistrationResponse::parse(
            br#"{"status":"sent","login":"99910000000","future":{"flag":true}}"#,
        )
        .unwrap();
        assert_eq!(
            r.decision(RegistrationEndpointPath::Code),
            RegistrationResponseDecision::CodeSent
        );
        assert_eq!(
            r.decision(RegistrationEndpointPath::Exist),
            RegistrationResponseDecision::Unknown
        );
        assert!(r.unknown_fields().contains_key("future"));
        assert!(!format!("{r:?}").contains("99910000000"));
        let r = RegistrationResponse::parse(br#"{"status":"future","reason":"new_rule"}"#).unwrap();
        assert_eq!(
            r.decision(RegistrationEndpointPath::Register),
            RegistrationResponseDecision::Unknown
        );
        assert_eq!(r.reason(), Some("new_rule"));
    }
    #[test]
    fn finalization_status_aliases_match_the_pinned_references() {
        // Cobalt isSuccessful and whalibmob isSuccessful are shared by their
        // register, challenge and security handlers. This is reference
        // compatibility, not an official-server acceptance claim.
        for status in ["ok", "sent", "verified"] {
            let json = format!(r#"{{"status":"{status}"}}"#);
            let response = RegistrationResponse::parse(json.as_bytes()).unwrap();
            for endpoint in [
                RegistrationEndpointPath::Register,
                RegistrationEndpointPath::Challenge,
                RegistrationEndpointPath::Security,
            ] {
                assert_eq!(
                    response.decision(endpoint),
                    RegistrationResponseDecision::Registered
                );
            }
        }
    }

    #[test]
    fn challenges_pins_waits_and_required_input_stay_distinct() {
        let r = RegistrationResponse::parse(br#"{"status":"fail","image_blob":"aW1hZ2U=","audio_blob":"YXVkaW8","sms_wait":"9","future_wait":12}"#).unwrap();
        assert_eq!(
            r.decision(RegistrationEndpointPath::Register),
            RegistrationResponseDecision::ChallengeRequired
        );
        assert_eq!(r.challenge().unwrap().image.as_deref(), Some(&b"image"[..]));
        assert_eq!(r.waits().seconds_for(VerificationMethod::Sms), Some(9));
        assert_eq!(r.waits().maximum_seconds(), Some(12));
        let r = RegistrationResponse::parse(br#"{"status":"fail","reason":"two_factor_required"}"#)
            .unwrap();
        assert_eq!(
            r.decision(RegistrationEndpointPath::Register),
            RegistrationResponseDecision::PinRequired
        );
        let r = RegistrationResponse::parse(
            br#"{"status":"ok","pending":"app_store_age","reason":"consent"}"#,
        )
        .unwrap();
        assert_eq!(
            r.decision(RegistrationEndpointPath::Register),
            RegistrationResponseDecision::RequiredInput
        );
    }
    #[test]
    fn negative_wait_sentinels_are_preserved_without_becoming_delays() {
        let response =
            RegistrationResponse::parse(br#"{"sms_wait":-1,"voice_wait":"-2"}"#).unwrap();
        assert_eq!(response.waits().seconds_for(VerificationMethod::Sms), None);
        assert_eq!(response.waits().maximum_seconds(), None);
        assert_eq!(
            response.waits().iter().collect::<Vec<_>>(),
            vec![("sms_wait", -1), ("voice_wait", -2)]
        );
        let response = RegistrationResponse::parse(br#"{"status":"ok"}"#).unwrap();
        assert_eq!(
            response.decision(RegistrationEndpointPath::Exist),
            RegistrationResponseDecision::AccountRecognized
        );
    }
    #[test]
    fn malformed_types_and_duplicate_status_are_rejected() {
        for bytes in [
            &br#"{"status":true}"#[..],
            &br#"{"status":"ok","status":"fail"}"#[..],
            &br#"{"login":1.2}"#[..],
            &br#"{"image_blob":"!!!"}"#[..],
            &b"[]"[..],
        ] {
            assert!(RegistrationResponse::parse(bytes).is_err());
        }
    }
    #[test]
    fn retry_after_is_typed_signed_and_selected_after_known_method_hints() {
        let response = RegistrationResponse::parse(
            br#"{"status":"fail","reason":"too_recent","retry_after":"12"}"#,
        )
        .unwrap();
        assert_eq!(response.retry_after(), Some(12));
        assert_eq!(
            response.wait_hint_seconds(Some(VerificationMethod::Sms)),
            Some(12)
        );
        assert_eq!(
            response.decision(RegistrationEndpointPath::Code),
            RegistrationResponseDecision::RateLimited
        );
        let response = RegistrationResponse::parse(
            br#"{"sms_wait":2,"voice_wait":9,"future_wait":999,"retry_after":12}"#,
        )
        .unwrap();
        assert_eq!(
            response.wait_hint_seconds(Some(VerificationMethod::Sms)),
            Some(2)
        );
        assert_eq!(response.wait_hint_seconds(None), Some(9));
        let response =
            RegistrationResponse::parse(br#"{"sms_wait":0,"voice_wait":-1,"retry_after":-1}"#)
                .unwrap();
        assert_eq!(response.retry_after(), Some(-1));
        assert_eq!(response.wait_hint_seconds(None), None);
        for malformed in [
            br#"{"retry_after":true}"#.as_slice(),
            br#"{"retry_after":"1.5"}"#,
            br#"{"retry_after":9223372036854775808}"#,
        ] {
            assert!(RegistrationResponse::parse(malformed).is_err());
        }
    }
    #[test]
    fn typed_delivery_hints_preserve_flash_metadata_and_reject_malformed_values() {
        let response = RegistrationResponse::parse(br#"{"status":"sent","length":6,"cli_filter":"(.*)373(.*)","cli_prefix":"373","cli_cc":"39","flash_timeout":30}"#).unwrap();
        let hints = response.delivery_hints().unwrap();
        assert_eq!(hints.code_length(), Some(6));
        assert_eq!(hints.flash_filter(), Some("(.*)373(.*)"));
        assert_eq!(hints.flash_prefix(), Some("373"));
        assert_eq!(hints.flash_country_code(), Some("39"));
        assert_eq!(hints.flash_timeout_seconds(), Some(30));
        assert!(RegistrationResponse::parse(br#"{"length":true}"#).is_err());
        assert!(RegistrationResponse::parse(br#"{"length":33}"#).is_err());
        assert!(RegistrationResponse::parse(br#"{"cli_filter":false}"#).is_err());
        assert!(RegistrationResponse::parse(br#"{"flash_timeout":4294967296}"#).is_err());
        let oversized = serde_json::json!({"cli_filter": "x".repeat(129)});
        assert!(RegistrationResponse::parse(&serde_json::to_vec(&oversized).unwrap()).is_err());
        assert!(
            RegistrationResponse::parse(br#"{"length":0,"flash_timeout":-1}"#)
                .unwrap()
                .delivery_hints()
                .is_none()
        );
    }
}
