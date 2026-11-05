use fluent_uri::{Uri, component::Host};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Configuration errors intentionally omit endpoint and credential contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum MobileConfigurationError {
    #[error("invalid mobile environment identifier")]
    InvalidEnvironment,
    #[error("invalid mobile registration endpoint")]
    InvalidRegistrationEndpoint,
    #[error("invalid mobile TCP endpoint")]
    InvalidTcpEndpoint,
    #[error("mobile trust keys must not be all zero")]
    InvalidTrust,
    #[error("mobile session belongs to another environment or trust configuration")]
    EnvironmentMismatch,
}

/// Validated registration base URL. The constructor performs no I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistrationEndpoint(String);

impl RegistrationEndpoint {
    /// Requires HTTPS; credentials, query strings and fragments are rejected.
    pub fn new(url: impl Into<String>) -> Result<Self, MobileConfigurationError> {
        Self::parse(url.into(), false)
    }

    /// Explicit plaintext test-service configuration, for independently keyed mocks.
    /// This never modifies production trust or allows fallback to another URL.
    pub fn for_test_http(url: impl Into<String>) -> Result<Self, MobileConfigurationError> {
        Self::parse(url.into(), true)
    }

    fn parse(url: String, allow_http: bool) -> Result<Self, MobileConfigurationError> {
        let invalid = MobileConfigurationError::InvalidRegistrationEndpoint;
        let parsed = Uri::parse(url.as_str()).map_err(|_| invalid)?;
        let authority = parsed.authority().ok_or(invalid)?;
        let scheme = parsed.scheme().as_str();
        if !(scheme.eq_ignore_ascii_case("https")
            || allow_http && scheme.eq_ignore_ascii_case("http"))
            || !portable_host(authority.host_parsed())
            || authority.userinfo().is_some()
            || authority.port().is_some_and(|port| port.is_empty())
            || authority.port_to_u16().map_err(|_| invalid)? == Some(0)
            || parsed.query().is_some()
            || parsed.fragment().is_some()
        {
            return Err(invalid);
        }
        Ok(Self(url))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Destination data only. WASM or embedded hosts may supply their own transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TcpEndpoint {
    host: String,
    port: u16,
}

impl TcpEndpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Result<Self, MobileConfigurationError> {
        let host = host.into();
        let invalid = MobileConfigurationError::InvalidTcpEndpoint;
        if port == 0
            || host.is_empty()
            || host
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
            || host.contains(['/', '\\', '@', '?', '#'])
        {
            return Err(invalid);
        }
        // URI parsing also validates IPv6 brackets and rejects port/URL syntax.
        let authority_text = format!("tcp://{host}");
        let uri = Uri::parse(authority_text.as_str()).map_err(|_| invalid)?;
        let authority = uri.authority().ok_or(invalid)?;
        if !portable_host(authority.host_parsed()) || authority.port().is_some() {
            return Err(invalid);
        }
        Ok(Self { host, port })
    }

    /// An IPv6 literal retains its URI brackets; adapters must handle that form.
    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

// RFC URI reg-names also permit percent escapes and sub-delimiters, and
// IPvFuture has no portable socket address interpretation. Exclude URI-only
// syntax before passing the host to HTTP, DNS or external host adapters.
// Resolution and DNS label policy remain the adapter's responsibility.
fn portable_host(host: Host<'_>) -> bool {
    match host {
        Host::RegName(name) => {
            !name.is_empty()
                && name
                    .as_str()
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
        }
        Host::IpvFuture { .. } => false,
        Host::Ipv4 { .. } | Host::Ipv6 { .. } => true,
    }
}

/// Public trust anchors, scoped to one environment. No private key is held here.
/// Construction checks structure only; the protocol crypto layer must validate
/// the key for its algorithm and the evidence source must qualify its origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MobileTrust {
    registration_key: [u8; 32],
    noise_root: [u8; 32],
}

impl MobileTrust {
    pub fn new(
        registration_key: [u8; 32],
        noise_root: [u8; 32],
    ) -> Result<Self, MobileConfigurationError> {
        if registration_key == [0; 32] || noise_root == [0; 32] {
            return Err(MobileConfigurationError::InvalidTrust);
        }
        Ok(Self {
            registration_key,
            noise_root,
        })
    }

    pub fn registration_key(&self) -> &[u8; 32] {
        &self.registration_key
    }

    pub fn noise_root(&self) -> &[u8; 32] {
        &self.noise_root
    }
}

/// Configuration identity to persist alongside credentials and cached routing.
///
/// The digest prevents accidental cross-environment reuse; it is not a MAC and
/// does not authenticate untrusted storage. Changing any endpoint or trust key
/// requires an explicit credential migration rather than implicit fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EnvironmentBinding([u8; 32]);

impl EnvironmentBinding {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Immutable per-session configuration with no built-in production destination.
#[derive(Debug, Clone)]
pub struct MobileEnvironment {
    id: String,
    registration: RegistrationEndpoint,
    connection: TcpEndpoint,
    trust: MobileTrust,
    binding: EnvironmentBinding,
}

impl MobileEnvironment {
    pub fn new(
        id: impl Into<String>,
        registration: RegistrationEndpoint,
        connection: TcpEndpoint,
        trust: MobileTrust,
    ) -> Result<Self, MobileConfigurationError> {
        let id = id.into();
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        {
            return Err(MobileConfigurationError::InvalidEnvironment);
        }
        let mut hash = Sha256::new();
        hash.update(b"whatsapp-rust/mobile-environment/v1");
        // Length prefixes prevent ambiguous concatenation of variable fields.
        for field in [
            id.as_bytes(),
            registration.as_str().as_bytes(),
            connection.host.as_bytes(),
        ] {
            hash.update((field.len() as u64).to_be_bytes());
            hash.update(field);
        }
        hash.update(connection.port.to_be_bytes());
        hash.update(trust.registration_key);
        hash.update(trust.noise_root);
        let binding = EnvironmentBinding(hash.finalize().into());
        Ok(Self {
            id,
            registration,
            connection,
            trust,
            binding,
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn registration(&self) -> &RegistrationEndpoint {
        &self.registration
    }
    pub fn connection(&self) -> &TcpEndpoint {
        &self.connection
    }
    pub fn trust(&self) -> &MobileTrust {
        &self.trust
    }
    pub fn binding(&self) -> EnvironmentBinding {
        self.binding
    }

    pub fn bind<T>(&self, value: T) -> EnvironmentBound<T> {
        EnvironmentBound {
            binding: self.binding,
            value,
        }
    }
}

/// A persisted value that must be checked against the selected environment.
/// No Debug implementation is provided because the value may contain secrets.
#[derive(serde::Serialize, serde::Deserialize)]
pub struct EnvironmentBound<T> {
    binding: EnvironmentBinding,
    value: T,
}

impl<T> EnvironmentBound<T> {
    pub fn get(&self, environment: &MobileEnvironment) -> Result<&T, MobileConfigurationError> {
        self.check(environment)?;
        Ok(&self.value)
    }

    pub fn into_inner(
        self,
        environment: &MobileEnvironment,
    ) -> Result<T, MobileConfigurationError> {
        self.check(environment)?;
        Ok(self.value)
    }

    fn check(&self, environment: &MobileEnvironment) -> Result<(), MobileConfigurationError> {
        if self.binding != environment.binding {
            return Err(MobileConfigurationError::EnvironmentMismatch);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(port: u16, key: u8) -> MobileEnvironment {
        MobileEnvironment::new(
            "fixture",
            RegistrationEndpoint::for_test_http("http://mock.test/v2").unwrap(),
            TcpEndpoint::new("mock.test", port).unwrap(),
            MobileTrust::new([key; 32], [2; 32]).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn bound_state_rejects_endpoint_and_trust_changes_after_serialization() {
        let original = environment(4443, 1);
        let stored = serde_json::to_vec(&original.bind(vec![1, 2, 3])).unwrap();
        let restored: EnvironmentBound<Vec<u8>> = serde_json::from_slice(&stored).unwrap();
        assert_eq!(restored.get(&environment(4443, 1)).unwrap(), &[1, 2, 3]);
        for changed in [environment(4444, 1), environment(4443, 3)] {
            assert_eq!(
                restored.get(&changed),
                Err(MobileConfigurationError::EnvironmentMismatch)
            );
        }
    }

    #[test]
    fn registration_requires_secure_url_or_explicit_test_configuration() {
        for invalid in [
            "http://mock.test/v2",
            "https://user:secret@host/v2",
            "https://host/v2?token=secret",
            "https://host/v2#fragment",
            "https://host:0/v2",
            "relative/path",
        ] {
            assert!(RegistrationEndpoint::new(invalid).is_err(), "{invalid}");
        }
        assert!(RegistrationEndpoint::new("https://registration.test/v2").is_ok());
        assert!(RegistrationEndpoint::for_test_http("http://barback:8080/v2").is_ok());
    }

    #[test]
    fn rejects_uri_only_host_syntax_before_adapter_selection() {
        for host in ["[v1.fixture]", "fixture%2etest", "fixture!name"] {
            assert!(TcpEndpoint::new(host, 443).is_err(), "TCP host: {host}");
            assert!(
                RegistrationEndpoint::new(format!("https://{host}/v2")).is_err(),
                "HTTP host: {host}"
            );
        }
        for host in [
            "barback",
            "bartender_mobile",
            "fixture.test.",
            "127.0.0.1",
            "[::1]",
        ] {
            assert!(TcpEndpoint::new(host, 443).is_ok(), "TCP host: {host}");
            assert!(
                RegistrationEndpoint::new(format!("https://{host}/v2")).is_ok(),
                "HTTP host: {host}"
            );
        }
    }

    #[test]
    fn tcp_destination_is_independent_of_registration() {
        let env = environment(4443, 1);
        assert_eq!(env.registration().as_str(), "http://mock.test/v2");
        assert_eq!(env.connection().host(), "mock.test");
        assert_eq!(env.connection().port(), 4443);
        for host in ["", "host:443", "https://host", "host/path", "host\n"] {
            assert!(TcpEndpoint::new(host, 443).is_err(), "{host}");
        }
        assert!(TcpEndpoint::new("[::1]", 443).is_ok());
    }
}
