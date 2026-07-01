//! `PasskeyAuthenticator` — the single pluggable point of the SHORTCAKE_PASSKEY
//! login flow (see `wacore::shortcake` for the deterministic protocol core).
//!
//! WhatsApp's passkey linking gate requires ONE thing an unofficial client cannot
//! reproduce on its own: a WebAuthn assertion (`navigator.credentials.get`) signed
//! by a passkey ALREADY REGISTERED to the account. Everything else in the protocol
//! is deterministic and lives in `wacore::shortcake`. This module abstracts that
//! single step so the rest of the linking flow is platform-agnostic.
//!
//! ## Why a real authenticator (not a software forgery)
//! The assertion's private key lives in a platform authenticator (Google Password
//! Manager / iCloud Keychain), non-extractable. So we do what WhatsApp Web does:
//! delegate to a real authenticator. The assertion signs only the SERVER
//! challenge and origin/rpId (NOT the Shortcake payload), so producing it is a
//! standard WebAuthn `get`. Using the real authenticator = a legitimate,
//! user-verified assertion = low ban risk (forging without the key is
//! impossible anyway).
//!
//! ## Strategies
//! - **Android Credential Manager (recommended for GPM passkeys):** the Android
//!   host app calls `CredentialManager.getCredential(...)` with the server's
//!   `raw_options_json` (a `GetCredentialRequest` containing a
//!   `GetPublicKeyCredentialOption(requestJson = raw_options_json)`); GPM signs
//!   with biometric; the app maps the returned
//!   `PublicKeyCredential.authenticationResponseJson` into an [`Assertion`] and
//!   returns it via a [`CallbackAuthenticator`]. No private key ever touches Rust.
//! - **hybrid/caBLE:** a desktop client tunnels CTAP2 to the phone's authenticator.
//! - **software (`passkey-rs`):** only when the passkey lives in an exportable vault.
//!
//! All three implement [`PasskeyAuthenticator`]; the default build ships only the
//! generic [`CallbackAuthenticator`] (host provides the assertion) so no platform
//! dependency leaks into headless/library builds.

pub mod flow;

use async_trait::async_trait;
use base64::prelude::*;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// WebAuthn user-verification requirement from the server's request options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserVerification {
    Required,
    Preferred,
    Discouraged,
}

impl UserVerification {
    /// Fail closed: a present-but-unrecognized value is rejected rather than
    /// silently downgraded to `Preferred` (absence is handled by the caller).
    fn parse(s: &str) -> Result<Self, PasskeyError> {
        match s {
            "required" => Ok(Self::Required),
            "preferred" => Ok(Self::Preferred),
            "discouraged" => Ok(Self::Discouraged),
            other => Err(PasskeyError::InvalidOptions(format!(
                "unsupported userVerification: {other}"
            ))),
        }
    }
}

/// A WebAuthn assertion request, parsed from the server's
/// `<passkey_request_options>` (a standard `PublicKeyCredentialRequestOptions`
/// JSON). `challenge` and `allow_credentials` are already base64url-decoded.
#[derive(Debug, Clone)]
pub struct AssertionRequest {
    /// Server challenge (raw bytes).
    pub challenge: Vec<u8>,
    /// Relying-party id (e.g. "web.whatsapp.com") the authenticator must sign for.
    pub rp_id: Option<String>,
    /// Allowed credential ids (raw bytes); empty = discoverable.
    pub allow_credentials: Vec<Vec<u8>>,
    pub user_verification: UserVerification,
    pub timeout_ms: Option<u64>,
    /// The verbatim server JSON — pass straight to Android Credential Manager's
    /// `GetPublicKeyCredentialOption(requestJson = ...)` (it wants the original).
    pub raw_options_json: String,
}

/// The result of a WebAuthn assertion, packaged for the `<passkey_prologue>` IQ.
#[derive(Debug, Clone)]
pub struct Assertion {
    /// UTF-8 JSON for `<webauthn_assertion>`:
    /// `{id, rawId(b64url), type:"public-key", response:{clientDataJSON, authenticatorData, signature, userHandle}}`.
    pub assertion_json: Vec<u8>,
    /// Raw credential rawId bytes for `<credential_id>`.
    pub credential_id: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum PasskeyError {
    #[error("no passkey registered for this account on the authenticator")]
    NoCredential,
    #[error("user cancelled or the ceremony timed out")]
    Cancelled,
    #[error("invalid request options: {0}")]
    InvalidOptions(String),
    #[error("authenticator backend error: {0}")]
    Backend(String),
    #[error("passkey linking flow error: {0}")]
    Flow(String),
}

/// Produces a WebAuthn assertion for a SHORTCAKE_PASSKEY link. Implemented by a
/// real authenticator (Android Credential Manager / hybrid / software vault).
///
/// The `MaybeSendSync` supertrait keeps this `Send + Sync` on native (the client
/// stores it as `Arc<dyn PasskeyAuthenticator>` and drives it across threads) but
/// drops the bound on wasm32, where a browser authenticator may hold `!Send` JS
/// handles, matching the sibling extension points (`Transport`, `EventHandler`).
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait PasskeyAuthenticator: wacore::sync_marker::MaybeSendSync {
    async fn get_assertion(&self, request: &AssertionRequest) -> Result<Assertion, PasskeyError>;
}

// Mirror the trait's `async_trait(?Send)` on wasm: a browser authenticator's
// future (e.g. awaiting `navigator.credentials.get`) is `!Send`. `cb`/`new`
// reference this alias, so they pick up the right bound per-target automatically.
#[cfg(not(target_arch = "wasm32"))]
type AssertionFuture = Pin<Box<dyn Future<Output = Result<Assertion, PasskeyError>> + Send>>;
#[cfg(target_arch = "wasm32")]
type AssertionFuture = Pin<Box<dyn Future<Output = Result<Assertion, PasskeyError>>>>;

// The stored closure: `Send + Sync` on native, relaxed on wasm to mirror
// `AssertionFuture` (a browser closure may capture `!Send` JS handles).
#[cfg(not(target_arch = "wasm32"))]
type AssertionCallback = dyn Fn(AssertionRequest) -> AssertionFuture + Send + Sync;
#[cfg(target_arch = "wasm32")]
type AssertionCallback = dyn Fn(AssertionRequest) -> AssertionFuture;

/// Generic [`PasskeyAuthenticator`] that defers to a host-provided async closure.
///
/// This is the integration seam for the Android Credential Manager strategy: the
/// Kotlin/JNI layer performs `CredentialManager.getCredential(...)` and resolves
/// the future with the mapped [`Assertion`]. Keeps all platform code out of the lib.
#[derive(Clone)]
pub struct CallbackAuthenticator {
    cb: Arc<AssertionCallback>,
}

impl CallbackAuthenticator {
    pub fn new<F>(f: F) -> Self
    where
        F: Fn(AssertionRequest) -> AssertionFuture + wacore::sync_marker::MaybeSendSync + 'static,
    {
        Self { cb: Arc::new(f) }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PasskeyAuthenticator for CallbackAuthenticator {
    async fn get_assertion(&self, request: &AssertionRequest) -> Result<Assertion, PasskeyError> {
        (self.cb)(request.clone()).await
    }
}

/// Parse the server's `PublicKeyCredentialRequestOptions` JSON into an
/// [`AssertionRequest`], base64url-decoding `challenge` and `allowCredentials[].id`.
pub fn parse_request_options(json: &str) -> Result<AssertionRequest, PasskeyError> {
    let v: serde_json::Value =
        serde_json::from_str(json).map_err(|e| PasskeyError::InvalidOptions(e.to_string()))?;

    let challenge_b64 = v
        .get("challenge")
        .and_then(|c| c.as_str())
        .ok_or_else(|| PasskeyError::InvalidOptions("missing challenge".into()))?;
    let challenge = BASE64_URL_SAFE_NO_PAD
        .decode(challenge_b64.trim_end_matches('='))
        .map_err(|e| PasskeyError::InvalidOptions(format!("challenge b64url: {e}")))?;
    if challenge.is_empty() {
        return Err(PasskeyError::InvalidOptions("empty challenge".into()));
    }

    // Absent rpId is fine (the authenticator defaults it); a present-but-non-string
    // value is malformed and must fail closed, not silently drop the RP binding.
    let rp_id = match v.get("rpId") {
        None => None,
        Some(r) => Some(
            r.as_str()
                .ok_or_else(|| PasskeyError::InvalidOptions("rpId must be a string".into()))?
                .to_string(),
        ),
    };

    // Reject malformed descriptors instead of dropping them: silently skipping
    // entries can collapse a populated allowCredentials into an empty list, which
    // this API treats as "discoverable" — a confusing, weaker outcome than failing.
    let mut allow_credentials = Vec::new();
    if let Some(allow_credentials_value) = v.get("allowCredentials") {
        let arr = allow_credentials_value.as_array().ok_or_else(|| {
            PasskeyError::InvalidOptions("allowCredentials must be an array".into())
        })?;
        for cred in arr {
            let id = cred.get("id").and_then(|i| i.as_str()).ok_or_else(|| {
                PasskeyError::InvalidOptions("allowCredentials[].id must be a string".into())
            })?;
            let bytes = BASE64_URL_SAFE_NO_PAD
                .decode(id.trim_end_matches('='))
                .map_err(|e| PasskeyError::InvalidOptions(format!("credential id b64url: {e}")))?;
            if bytes.is_empty() {
                return Err(PasskeyError::InvalidOptions(
                    "allowCredentials[].id is empty".into(),
                ));
            }
            allow_credentials.push(bytes);
        }
    }

    let user_verification = match v.get("userVerification") {
        None => UserVerification::Preferred,
        Some(u) => UserVerification::parse(u.as_str().ok_or_else(|| {
            PasskeyError::InvalidOptions("userVerification must be a string".into())
        })?)?,
    };

    let timeout_ms = v.get("timeout").and_then(|t| t.as_u64());

    Ok(AssertionRequest {
        challenge,
        rp_id,
        allow_credentials,
        user_verification,
        timeout_ms,
        raw_options_json: json.to_string(),
    })
}

/// Assemble the `<webauthn_assertion>` JSON (WhatsApp Web's exact shape) from raw
/// WebAuthn assertion components. For authenticator backends that return raw bytes
/// rather than WA-shaped JSON (e.g. a software/hybrid authenticator). `user_handle`
/// is optional. All binary fields are base64url-encoded (no padding).
pub fn build_webauthn_assertion_json(
    credential_id: &[u8],
    client_data_json: &[u8],
    authenticator_data: &[u8],
    signature: &[u8],
    user_handle: Option<&[u8]>,
) -> Vec<u8> {
    let id = BASE64_URL_SAFE_NO_PAD.encode(credential_id);
    let assertion = serde_json::json!({
        "id": id,
        "rawId": id,
        "type": "public-key",
        "response": {
            "clientDataJSON": BASE64_URL_SAFE_NO_PAD.encode(client_data_json),
            "authenticatorData": BASE64_URL_SAFE_NO_PAD.encode(authenticator_data),
            "signature": BASE64_URL_SAFE_NO_PAD.encode(signature),
            "userHandle": user_handle.map(|u| BASE64_URL_SAFE_NO_PAD.encode(u)),
        }
    });
    assertion.to_string().into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_request_options() {
        let challenge = b"the-challenge-bytes!";
        let cred = b"credential-id-1";
        let json = serde_json::json!({
            "challenge": BASE64_URL_SAFE_NO_PAD.encode(challenge),
            "rpId": "web.whatsapp.com",
            "userVerification": "required",
            "timeout": 60000u64,
            "allowCredentials": [
                {"type": "public-key", "id": BASE64_URL_SAFE_NO_PAD.encode(cred)}
            ]
        })
        .to_string();

        let req = parse_request_options(&json).unwrap();
        assert_eq!(req.challenge, challenge);
        assert_eq!(req.rp_id.as_deref(), Some("web.whatsapp.com"));
        assert_eq!(req.user_verification, UserVerification::Required);
        assert_eq!(req.timeout_ms, Some(60000));
        assert_eq!(req.allow_credentials, vec![cred.to_vec()]);
        assert_eq!(req.raw_options_json, json); // verbatim for Credential Manager
    }

    #[test]
    fn missing_challenge_is_error() {
        assert!(parse_request_options("{\"rpId\":\"x\"}").is_err());
    }

    #[test]
    fn unknown_user_verification_fails_closed() {
        let json = serde_json::json!({
            "challenge": BASE64_URL_SAFE_NO_PAD.encode(b"c"),
            "userVerification": "sometimes",
        })
        .to_string();
        assert!(matches!(
            parse_request_options(&json),
            Err(PasskeyError::InvalidOptions(_))
        ));
    }

    #[test]
    fn absent_user_verification_defaults_to_preferred() {
        let json =
            serde_json::json!({ "challenge": BASE64_URL_SAFE_NO_PAD.encode(b"c") }).to_string();
        let req = parse_request_options(&json).unwrap();
        assert_eq!(req.user_verification, UserVerification::Preferred);
    }

    #[test]
    fn malformed_allow_credentials_is_rejected() {
        // non-array
        let json = serde_json::json!({
            "challenge": BASE64_URL_SAFE_NO_PAD.encode(b"c"),
            "allowCredentials": "nope",
        })
        .to_string();
        assert!(parse_request_options(&json).is_err());

        // entry without a string id must error, not be silently dropped
        let json = serde_json::json!({
            "challenge": BASE64_URL_SAFE_NO_PAD.encode(b"c"),
            "allowCredentials": [{"type": "public-key"}],
        })
        .to_string();
        assert!(parse_request_options(&json).is_err());

        // present-but-empty id (all padding / empty string) decodes to zero bytes,
        // which is never a real credential id, so it must error, not push an empty entry.
        let json = serde_json::json!({
            "challenge": BASE64_URL_SAFE_NO_PAD.encode(b"c"),
            "allowCredentials": [{"type": "public-key", "id": ""}],
        })
        .to_string();
        assert!(parse_request_options(&json).is_err());
    }

    #[test]
    fn empty_challenge_is_rejected() {
        // a present-but-empty challenge provides zero replay protection; reject it
        // rather than handing a degenerate request to the authenticator.
        let json = serde_json::json!({ "challenge": "" }).to_string();
        assert!(matches!(
            parse_request_options(&json),
            Err(PasskeyError::InvalidOptions(_))
        ));
    }

    #[test]
    fn non_string_rp_id_is_rejected() {
        // present-but-malformed rpId must fail closed, not silently drop the RP.
        let json = serde_json::json!({
            "challenge": BASE64_URL_SAFE_NO_PAD.encode(b"c"),
            "rpId": 123,
        })
        .to_string();
        assert!(matches!(
            parse_request_options(&json),
            Err(PasskeyError::InvalidOptions(_))
        ));
    }

    #[test]
    fn builds_wa_assertion_json_shape() {
        let bytes = build_webauthn_assertion_json(b"cid", b"cdj", b"authdata", b"sig", None);
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["type"], "public-key");
        assert_eq!(v["id"], BASE64_URL_SAFE_NO_PAD.encode(b"cid"));
        assert_eq!(v["rawId"], BASE64_URL_SAFE_NO_PAD.encode(b"cid"));
        assert_eq!(
            v["response"]["clientDataJSON"],
            BASE64_URL_SAFE_NO_PAD.encode(b"cdj")
        );
        assert_eq!(
            v["response"]["signature"],
            BASE64_URL_SAFE_NO_PAD.encode(b"sig")
        );
        assert!(v["response"]["userHandle"].is_null());
    }

    #[tokio::test]
    async fn callback_authenticator_invokes_closure() {
        let auth = CallbackAuthenticator::new(|req: AssertionRequest| {
            Box::pin(async move {
                Ok(Assertion {
                    assertion_json: req.raw_options_json.into_bytes(),
                    credential_id: req.challenge,
                })
            })
        });
        let req = AssertionRequest {
            challenge: vec![1, 2, 3],
            rp_id: Some("web.whatsapp.com".into()),
            allow_credentials: vec![],
            user_verification: UserVerification::Preferred,
            timeout_ms: None,
            raw_options_json: "{}".into(),
        };
        let a = auth.get_assertion(&req).await.unwrap();
        assert_eq!(a.credential_id, vec![1, 2, 3]);
        assert_eq!(a.assertion_json, b"{}".to_vec());
    }
}
