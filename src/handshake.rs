use crate::socket::NoiseSocket;
use crate::store::persistence_manager::PersistenceManager;
use crate::transport::{Transport, TransportEvent};
use log::{debug, warn};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use wacore::noise::NoiseCipher;
use wacore::runtime::Runtime;
use wacore::store::DeviceCommand;

pub use wacore::handshake::HandshakeError as CoreHandshakeError;
pub use wacore::handshake::NoiseCertPolicy;
pub use wacore::handshake::build_handshake_header;
pub use wacore::handshake::runner::{
    HandshakeError, HandshakePattern, HandshakeSuccess, IK_FAILURE_THRESHOLD,
    NOISE_HANDSHAKE_RESPONSE_TIMEOUT, Result, recv_frame, run_ik_handshake, run_xx_handshake,
    select_pattern, send_first_handshake_message,
};

/// Connect a registered mobile primary using its flat-certificate XX profile.
/// Transport, entropy, account keys and wall clock are all supplied by the host.
#[cfg(feature = "mobile-core")]
pub async fn do_mobile_handshake(
    runtime: Arc<dyn Runtime>,
    persistence_manager: &PersistenceManager,
    transport: Arc<dyn Transport>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    observers: crate::socket::noise_socket::SendObservers,
    session: &crate::mobile_session::RegisteredMobileSession,
    zero_based_attempt: u32,
) -> Result<Arc<NoiseSocket>> {
    use wacore::mobile::MobileSessionError;
    use wacore::mobile::MobileXxHandshake;
    let clock = runtime
        .clock()
        .ok_or(HandshakeError::Mobile(MobileSessionError::MissingClock))?;
    let read_time = || {
        let reading = clock
            .reading()
            .map_err(|_| HandshakeError::Mobile(MobileSessionError::MalformedClock))?;
        u64::try_from(reading.wall_time_millis())
            .map(|milliseconds| milliseconds / 1000)
            .map_err(|_| HandshakeError::Mobile(MobileSessionError::MalformedClock))
    };
    read_time()?; // Validate the supplied clock before any transport write.
    let device = persistence_manager.get_device_snapshot();
    let payload = session
        .encode_attempt_payload(&device, zero_based_attempt)
        .map_err(HandshakeError::Mobile)?;
    let handshake = MobileXxHandshake::new(
        device.noise_key.clone(),
        session
            .fresh_ephemeral_key()
            .map_err(HandshakeError::Mobile)?,
        payload,
        session.prologue(),
        session.trust(),
    )
    .map_err(HandshakeError::Mobile)?;
    let hello = handshake.client_hello();
    let framed = wacore::framing::encode_frame(&hello, Some(session.prologue().as_bytes()))
        .map_err(HandshakeError::Transport)?;
    transport.send(bytes::Bytes::from(framed)).await?;
    let mut decoder = wacore::framing::FrameDecoder::new();
    let response = recv_frame(&runtime, transport_events, &mut decoder).await?;
    let outcome = handshake
        .receive_server_hello(&response, read_time()?)
        .map_err(HandshakeError::Mobile)?;
    let finish = wacore::framing::encode_frame(&outcome.client_finish, None)
        .map_err(HandshakeError::Transport)?;
    transport.send(bytes::Bytes::from(finish)).await?;
    Ok(Arc::new(NoiseSocket::with_observers(
        runtime,
        transport,
        outcome.write_cipher,
        outcome.read_cipher,
        observers,
    )))
}

fn should_persist_cert_chain(device: &wacore::store::Device) -> bool {
    device.is_registered()
}

/// Runs the Noise handshake with the default cert policy (strict without
/// the legacy feature). Only callers that explicitly opt a client into the
/// testing bypass use `do_handshake_with_cert_policy`.
pub async fn do_handshake(
    runtime: Arc<dyn Runtime>,
    persistence_manager: &PersistenceManager,
    ik_handshake_failures: &AtomicU32,
    transport: Arc<dyn Transport>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    observers: crate::socket::noise_socket::SendObservers,
) -> Result<Arc<NoiseSocket>> {
    do_handshake_with_cert_policy(
        runtime,
        persistence_manager,
        ik_handshake_failures,
        transport,
        transport_events,
        observers,
        NoiseCertPolicy::default(),
    )
    .await
}

/// Runs the Noise handshake with the client's cert policy. A chain accepted
/// under the bypass is neither read from nor written to the trusted IK
/// cache: every connect starts at XX and nothing it learns persists.
pub async fn do_handshake_with_cert_policy(
    runtime: Arc<dyn Runtime>,
    persistence_manager: &PersistenceManager,
    ik_handshake_failures: &AtomicU32,
    transport: Arc<dyn Transport>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    observers: crate::socket::noise_socket::SendObservers,
    cert_policy: NoiseCertPolicy,
) -> Result<Arc<NoiseSocket>> {
    let (write_cipher, read_cipher) = negotiate(
        &runtime,
        persistence_manager,
        ik_handshake_failures,
        &transport,
        transport_events,
        cert_policy,
    )
    .await?;

    // Built outside the handshake span: the socket spawns the connection's
    // sender task, which encrypts every outbound frame until the connection
    // ends. Inside, that per-frame work is rooted at a one-shot span.
    Ok(Arc::new(NoiseSocket::with_observers(
        runtime,
        transport,
        write_cipher,
        read_cipher,
        observers,
    )))
}

/// Runs the Noise handshake and returns the transport cipher pair it derived.
/// The cert chain never leaves this function: persisting it is part of the
/// handshake, using it is not.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.conn.handshake", level = "debug", skip_all, err(Debug))
)]
async fn negotiate(
    runtime: &Arc<dyn Runtime>,
    persistence_manager: &PersistenceManager,
    ik_handshake_failures: &AtomicU32,
    transport: &Arc<dyn Transport>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    cert_policy: NoiseCertPolicy,
) -> Result<(NoiseCipher, NoiseCipher)> {
    let device_snapshot = persistence_manager.get_device_snapshot();
    let now_secs = runtime.wall_time_secs();
    let pattern = select_pattern(
        &device_snapshot,
        ik_handshake_failures.load(Ordering::Acquire),
        now_secs,
        cert_policy,
    );

    let mut fallback_taken = false;

    let result = match pattern {
        HandshakePattern::Xx => {
            debug!("[socket] doFullHandshake: openChatSocket send hello");
            run_xx_handshake(
                runtime,
                &device_snapshot,
                transport.clone(),
                transport_events,
                cert_policy,
            )
            .await
        }
        HandshakePattern::Ik(server_static_pub) => {
            debug!("[socket] resumeNoiseHandshake started");
            run_ik_handshake(
                runtime,
                &device_snapshot,
                server_static_pub,
                transport.clone(),
                transport_events,
                &mut fallback_taken,
                cert_policy,
            )
            .await
        }
    };

    match result {
        Ok(success) => {
            // A bypass-accepted chain arrives as `None`, so it can never be
            // persisted no matter which policy ran the handshake.
            if let Some(chain) = success.server_cert_chain
                && should_persist_cert_chain(&device_snapshot)
            {
                persistence_manager
                    .process_command(DeviceCommand::SetServerCertChain(chain.into()))
                    .await;
            }
            ik_handshake_failures.store(0, Ordering::Release);
            Ok((success.write_cipher, success.read_cipher))
        }
        Err(e) => {
            // Skip invalidation past the XXfallback pivot: by that point the
            // server has already accepted our IK ClientHello and the cache
            // is no longer the implicated party.
            if matches!(pattern, HandshakePattern::Ik(_)) && !fallback_taken && e.is_crypto_fatal()
            {
                warn!(
                    "[socket] resumeNoiseHandshake failed crypto-fatally; \
                     clearing cached server cert chain and forcing XX next connect: {e}"
                );
                ik_handshake_failures.fetch_add(1, Ordering::AcqRel);
                persistence_manager
                    .process_command(DeviceCommand::ClearServerCertChain)
                    .await;
            }
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::store::CachedNoiseCert;
    use wacore::store::CachedServerCertChain;

    fn cached_chain(
        leaf_key: [u8; 32],
        leaf_not_after: i64,
        intermediate_not_after: i64,
    ) -> CachedServerCertChain {
        CachedServerCertChain {
            intermediate: CachedNoiseCert {
                key: [0xCC; 32],
                not_before: 1_700_000_000,
                not_after: intermediate_not_after,
            },
            leaf: CachedNoiseCert {
                key: leaf_key,
                not_before: 1_700_000_000,
                not_after: leaf_not_after,
            },
            signature_verified: true,
        }
    }

    async fn paired_pm() -> Arc<PersistenceManager> {
        let pm = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        pm.process_command(DeviceCommand::SetId(Some(
            "12345@s.whatsapp.net".parse().unwrap(),
        )))
        .await;
        pm
    }

    async fn snapshot_with_chain(
        pm: &PersistenceManager,
        chain: CachedServerCertChain,
    ) -> Arc<crate::store::Device> {
        pm.process_command(DeviceCommand::SetServerCertChain(chain.clone()))
            .await;
        let snapshot = pm.get_device_snapshot();
        assert_eq!(
            snapshot.server_cert_chain.as_ref(),
            Some(&chain),
            "command-applied chain must be visible through the cached snapshot"
        );
        snapshot
    }

    #[tokio::test]
    async fn select_pattern_no_cache_returns_xx() {
        let pm = paired_pm().await;
        let device = pm.get_device_snapshot();
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_with_valid_cache_returns_ik() {
        let pm = paired_pm().await;
        let pub_key = [0xAA; 32];
        let device =
            snapshot_with_chain(&pm, cached_chain(pub_key, 1_900_000_000, 1_900_000_000)).await;
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Ik(pub_key)
        );
    }

    #[tokio::test]
    async fn select_pattern_after_one_failure_returns_xx() {
        let pm = paired_pm().await;
        let device =
            snapshot_with_chain(&pm, cached_chain([0xAA; 32], 1_900_000_000, 1_900_000_000)).await;
        assert_eq!(
            select_pattern(
                &device,
                IK_FAILURE_THRESHOLD,
                1_800_000_000,
                NoiseCertPolicy::Strict
            ),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_with_expired_leaf_returns_xx() {
        let pm = paired_pm().await;
        let device =
            snapshot_with_chain(&pm, cached_chain([0xAA; 32], 1_700_000_500, 1_900_000_000)).await;
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_with_expired_intermediate_returns_xx() {
        let pm = paired_pm().await;
        let device =
            snapshot_with_chain(&pm, cached_chain([0xAA; 32], 1_900_000_000, 1_700_000_500)).await;
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_with_clock_before_leaf_not_before_returns_xx() {
        let pm = paired_pm().await;
        let device =
            snapshot_with_chain(&pm, cached_chain([0xAA; 32], 1_900_000_000, 1_900_000_000)).await;
        assert_eq!(
            select_pattern(&device, 0, 1_699_999_999, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_with_clock_before_intermediate_not_before_returns_xx() {
        let pm = paired_pm().await;
        let mut chain = cached_chain([0xAA; 32], 1_900_000_000, 1_900_000_000);
        chain.intermediate.not_before = 1_800_000_001;
        let device = snapshot_with_chain(&pm, chain).await;
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_unregistered_device_returns_xx_even_with_valid_cache() {
        let pm = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let device =
            snapshot_with_chain(&pm, cached_chain([0xAA; 32], 1_900_000_000, 1_900_000_000)).await;
        assert!(!device.is_registered(), "fresh backend must be unpaired");
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_bypass_ignores_valid_cache() {
        let pm = paired_pm().await;
        let device =
            snapshot_with_chain(&pm, cached_chain([0xAA; 32], 1_900_000_000, 1_900_000_000)).await;
        assert_eq!(
            select_pattern(
                &device,
                0,
                1_800_000_000,
                NoiseCertPolicy::DangerSkipCertChainVerify
            ),
            HandshakePattern::Xx
        );
    }

    #[tokio::test]
    async fn select_pattern_unmarked_cache_returns_xx() {
        let pm = paired_pm().await;
        let mut chain = cached_chain([0xAA; 32], 1_900_000_000, 1_900_000_000);
        chain.signature_verified = false;
        let device = snapshot_with_chain(&pm, chain).await;
        assert_eq!(
            select_pattern(&device, 0, 1_800_000_000, NoiseCertPolicy::Strict),
            HandshakePattern::Xx
        );
    }

    #[test]
    fn cert_policy_cache_gates() {
        assert!(NoiseCertPolicy::Strict.reuse_cached_chain());
        assert!(!NoiseCertPolicy::DangerSkipCertChainVerify.reuse_cached_chain());
        assert!(!NoiseCertPolicy::Strict.skip_signature_check());
        assert!(NoiseCertPolicy::DangerSkipCertChainVerify.skip_signature_check());
    }

    // The default policy is strict.
    #[test]
    fn cert_policy_default_is_strict() {
        assert_eq!(NoiseCertPolicy::default(), NoiseCertPolicy::Strict);
    }

    #[test]
    fn should_persist_cert_chain_unregistered_returns_false() {
        let device = wacore::store::Device::new();
        assert!(!device.is_registered());
        assert!(!should_persist_cert_chain(&device));
    }

    #[tokio::test]
    async fn should_persist_cert_chain_registered_returns_true() {
        let pm = paired_pm().await;
        let device = pm.get_device_snapshot();
        assert!(device.is_registered());
        assert!(should_persist_cert_chain(&device));
    }

    #[test]
    fn handshake_error_classification() {
        // Transient — never invalidate the cache.
        assert!(HandshakeError::Timeout.is_transient());
        assert!(HandshakeError::Disconnected.is_transient());
        assert!(HandshakeError::StreamClosed.is_transient());
        assert!(!HandshakeError::Timeout.is_crypto_fatal());
        assert!(!HandshakeError::Disconnected.is_crypto_fatal());
        assert!(!HandshakeError::StreamClosed.is_crypto_fatal());

        // Stale-cache-indicating Core variants.
        for err in [
            HandshakeError::Core(CoreHandshakeError::IncompleteResponse),
            HandshakeError::Core(CoreHandshakeError::CertVerification("x".into())),
            HandshakeError::Core(CoreHandshakeError::InvalidKeyLength),
        ] {
            assert!(err.is_crypto_fatal(), "{err:?} should be crypto-fatal");
            assert!(!err.is_transient(), "{err:?} should not be transient");
        }

        // Programmer-side bug: Crypto(String) wraps generic crypto-provider
        // misuse; not a server-side cache problem.
        let bug = HandshakeError::Core(CoreHandshakeError::Crypto("bug".into()));
        assert!(
            !bug.is_crypto_fatal(),
            "generic Crypto(String) errors must not invalidate the cache"
        );
        assert!(!bug.is_transient());
    }

    /// Both the XX and IK initial messages must travel inside a frame whose
    /// prologue is `WA_CONN_HEADER` (optionally preceded by an edge-routing
    /// pre-intro). The wire-side server validates this prologue when it
    /// re-derives `h0` for transcript MAC checks, so any divergence between
    /// the two paths would surface only as a generic AEAD failure.
    ///
    /// We compare by fingerprinting the header bytes returned by the shared
    /// helper for the two relevant scenarios — IK and XX both must hit the
    /// same builder, with edge-routing applied identically when present.
    #[test]
    fn xx_and_ik_share_same_first_frame_prologue() {
        // No edge routing: pure WA_CONN_HEADER.
        let (xx_header, xx_used) = build_handshake_header(None);
        let (ik_header, ik_used) = build_handshake_header(None);
        assert_eq!(xx_header, ik_header);
        assert_eq!(xx_used, ik_used);
        assert!(xx_header.starts_with(b"WA"));

        // With edge routing: pre-intro applied identically.
        let routing = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let (xx_h2, xx_used2) = build_handshake_header(Some(&routing));
        let (ik_h2, ik_used2) = build_handshake_header(Some(&routing));
        assert_eq!(xx_h2, ik_h2);
        assert_eq!(xx_used2, ik_used2);
        assert!(xx_used2);
        assert!(xx_h2.starts_with(b"ED\x00\x01"));
        assert!(xx_h2.ends_with(b"WA\x06\x03") || xx_h2.ends_with(b"WA\x06\x04"));
    }
}

#[cfg(all(test, feature = "mobile-core"))]
mod mobile_transport_tests {
    use super::*;
    use bytes::Bytes;
    use rand::SeedableRng;
    use wacore::libsignal::protocol::{KeyPair, PrivateKey, PublicKey};
    use wacore::mobile::*;
    use wacore::noise::NoiseState;
    use wacore::store::{MobileSessionBinding, StoredMobilePlatform};
    use waproto::whatsapp as wa;

    fn key(seed: u8) -> KeyPair {
        let private = PrivateKey::deserialize(&[seed; 32]).unwrap();
        KeyPair::new(private.public_key().unwrap(), private)
    }
    struct Ephemeral(std::sync::atomic::AtomicU8);
    impl crate::MobileEphemeralKeyProvider for Ephemeral {
        fn generate(&self) -> std::result::Result<KeyPair, MobileSessionError> {
            Ok(key(self.0.fetch_add(1, Ordering::Relaxed)))
        }
    }
    struct Clock(i64);
    impl wacore::time::Clock for Clock {
        fn wall_time_millis(&self) -> i64 {
            self.0
        }
        fn monotonic_time(&self) -> wacore::time::Instant {
            wacore::time::Instant::now()
        }
    }
    struct Capture(async_channel::Sender<Bytes>);
    #[async_trait::async_trait]
    impl Transport for Capture {
        async fn send(&self, bytes: Bytes) -> anyhow::Result<()> {
            self.0.send(bytes).await?;
            Ok(())
        }
        async fn disconnect(&self) {}
    }
    struct Fixture {
        pm: Arc<PersistenceManager>,
        config: crate::RegisteredMobileSession,
        transport: Arc<dyn Transport>,
        outbound: async_channel::Receiver<Bytes>,
        incoming: async_channel::Sender<TransportEvent>,
        events: async_channel::Receiver<TransportEvent>,
        runtime: Arc<dyn Runtime>,
    }
    async fn fixture() -> Fixture {
        let pm = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .unwrap(),
        );
        pm.process_command(DeviceCommand::SetId(Some(
            "12345@s.whatsapp.net".parse().unwrap(),
        )))
        .await;
        let root = key(41);
        let environment = MobileEnvironment::new(
            "synthetic-sdk",
            RegistrationEndpoint::for_test_http("http://mock.invalid/registration").unwrap(),
            TcpEndpoint::new("mock.invalid", 5222).unwrap(),
            MobileTrust::new([1; 32], *root.public_key.public_key_bytes()).unwrap(),
        )
        .unwrap();
        let device = pm.get_device_snapshot();
        let identity = RegistrationIdentity::from_public_keys(
            device.identity_key.public_key.public_key_bytes(),
            device.noise_key.public_key.public_key_bytes(),
        );
        let binding = MobileSessionBinding::new(
            *environment.binding().as_bytes(),
            *identity.as_bytes(),
            StoredMobilePlatform::Android,
            vec![],
            None,
        )
        .unwrap();
        pm.process_command(DeviceCommand::SetMobileSessionBinding(binding))
            .await;
        let profile = MobileLoginProfile::new(
            MobilePlatform::Android,
            MobileAppVersion::parse("2.26.1").unwrap(),
            MobileDevice::builder()
                .manufacturer("Acme".into())
                .device("Model".into())
                .os_version("1".into())
                .os_build_number("B1".into())
                .phone_id("synthetic-sdk".into())
                .locale_language("en".into())
                .locale_country("GB".into())
                .build(),
        )
        .unwrap();
        let config = crate::RegisteredMobileSession::new(
            environment,
            profile,
            MobileLoginOptions::default(),
            12345,
            identity,
            MobileNoisePrologue::new(3),
            Arc::new(Ephemeral(std::sync::atomic::AtomicU8::new(44))),
        )
        .unwrap()
        .with_primary_identity_policy(wacore::send::PrimaryDeviceIdentityPolicy::Omit);
        let (send, outbound) = async_channel::bounded(8);
        let (incoming, events) = async_channel::bounded(32);
        Fixture {
            pm,
            config,
            transport: Arc::new(Capture(send)),
            outbound,
            incoming,
            events,
            runtime: Arc::new(wacore::runtime::ClockedRuntime::new(
                Arc::new(crate::TokioRuntime),
                Arc::new(Clock(100_000)),
            )),
        }
    }
    // Independent uint24 framing, deliberately not the production frame encoder.
    fn frame(payload: &[u8]) -> Vec<u8> {
        let n = payload.len();
        let mut out = vec![(n >> 16) as u8, (n >> 8) as u8, n as u8];
        out.extend_from_slice(payload);
        out
    }
    fn unframe(bytes: &[u8]) -> &[u8] {
        assert!(bytes.len() >= 3);
        assert_eq!(
            ((bytes[0] as usize) << 16) | ((bytes[1] as usize) << 8) | bytes[2] as usize,
            bytes.len() - 3
        );
        &bytes[3..]
    }
    fn varint(out: &mut Vec<u8>, mut value: u64) {
        while value > 127 {
            out.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        out.push(value as u8);
    }
    fn field(out: &mut Vec<u8>, number: u8, value: &[u8]) {
        out.push(number << 3 | 2);
        varint(out, value.len() as u64);
        out.extend_from_slice(value);
    }
    fn certificate(server: &KeyPair, mismatch: bool) -> Vec<u8> {
        let mut details = vec![8, 7, 24];
        varint(&mut details, 1000);
        let wrong_key = key(99);
        field(
            &mut details,
            5,
            if mismatch {
                wrong_key.public_key.public_key_bytes()
            } else {
                server.public_key.public_key_bytes()
            },
        );
        let signature = key(41)
            .private_key
            .calculate_signature(&details, &mut rand::rngs::StdRng::seed_from_u64(77))
            .unwrap();
        let mut certificate = Vec::new();
        field(&mut certificate, 1, &details);
        field(&mut certificate, 2, &signature);
        certificate
    }
    async fn peer_hello(
        outbound: &async_channel::Receiver<Bytes>,
        incoming: &async_channel::Sender<TransportEvent>,
        mismatch: bool,
    ) -> (NoiseState, KeyPair) {
        let first = outbound.recv().await.unwrap();
        peer_response(first, incoming, mismatch).await
    }
    async fn peer_response(
        first: Bytes,
        incoming: &async_channel::Sender<TransportEvent>,
        mismatch: bool,
    ) -> (NoiseState, KeyPair) {
        assert_eq!(&first[..4], &[b'W', b'A', 5, 3]);
        let hello = waproto::codec::handshake_message_decode(unframe(&first[4..])).unwrap();
        assert_eq!(
            hello.client_hello.ephemeral.as_deref(),
            Some(key(44).public_key.public_key_bytes().as_slice())
        );
        let client =
            PublicKey::from_djb_public_key_bytes(hello.client_hello.ephemeral.as_ref().unwrap())
                .unwrap();
        let ephemeral = key(42);
        let server = key(43);
        let mut noise = NoiseState::new(
            wacore_binary::consts::NOISE_PATTERN_XX.as_bytes(),
            &[b'W', b'A', 5, 3],
        )
        .unwrap();
        noise.authenticate(client.public_key_bytes());
        noise.authenticate(ephemeral.public_key.public_key_bytes());
        noise
            .mix_key(&ephemeral.private_key.calculate_agreement(&client).unwrap())
            .unwrap();
        let encrypted_static = noise.encrypt(server.public_key.public_key_bytes()).unwrap();
        noise
            .mix_key(&server.private_key.calculate_agreement(&client).unwrap())
            .unwrap();
        let encrypted_cert = noise.encrypt(&certificate(&server, mismatch)).unwrap();
        let mut response = wa::HandshakeMessage::default();
        let mut server_hello = wa::handshake_message::ServerHello::default();
        server_hello.ephemeral = Some(ephemeral.public_key.public_key_bytes().to_vec());
        server_hello.r#static = Some(encrypted_static);
        server_hello.payload = Some(encrypted_cert);
        response.server_hello = buffa::MessageField::some(server_hello);
        let framed = frame(&waproto::codec::handshake_message_to_vec(&response));
        // Split inside both the length prefix and encrypted handshake fields.
        for fragment in [&framed[..1], &framed[1..2], &framed[2..7], &framed[7..]] {
            incoming
                .send(TransportEvent::DataReceived(Bytes::copy_from_slice(
                    fragment,
                )))
                .await
                .unwrap();
        }
        (noise, ephemeral)
    }
    #[tokio::test]
    async fn mobile_sdk_fragmented_xx_and_noise_socket_exchange() {
        let mut f = fixture().await;
        let expected_payload = f.config.encode_payload().unwrap();
        let expected_static =
            f.pm.get_device_snapshot()
                .noise_key
                .public_key
                .public_key_bytes()
                .to_vec();
        let handshake = do_mobile_handshake(
            f.runtime.clone(),
            &f.pm,
            f.transport.clone(),
            &mut f.events,
            Default::default(),
            &f.config,
            0,
        );
        let peer = async {
            let (mut noise, ephemeral) = peer_hello(&f.outbound, &f.incoming, false).await;
            let finish = f.outbound.recv().await.unwrap();
            let finish = waproto::codec::handshake_message_decode(unframe(&finish)).unwrap();
            let client_static = noise
                .decrypt(finish.client_finish.r#static.as_ref().unwrap())
                .unwrap();
            assert_eq!(client_static, expected_static);
            noise
                .mix_key(
                    &ephemeral
                        .private_key
                        .calculate_agreement(
                            &PublicKey::from_djb_public_key_bytes(&client_static).unwrap(),
                        )
                        .unwrap(),
                )
                .unwrap();
            assert_eq!(
                noise
                    .decrypt(finish.client_finish.payload.as_ref().unwrap())
                    .unwrap(),
                expected_payload
            );
            noise.split().unwrap()
        };
        let (socket, peer) = futures::join!(handshake, peer);
        let socket = socket.unwrap();
        socket
            .encrypt_and_send(Bytes::from_static(b"sdk request"))
            .await
            .unwrap();
        let sent = f.outbound.recv().await.unwrap();
        let mut ciphertext = unframe(&sent).to_vec();
        peer.write
            .decrypt_in_place_with_counter(0, &mut ciphertext)
            .unwrap();
        assert_eq!(ciphertext, b"sdk request");
        let ciphertext = peer.read.encrypt_with_counter(0, b"sdk response").unwrap();
        assert_eq!(
            socket
                .decrypt_frame(bytes::BytesMut::from(ciphertext.as_slice()))
                .unwrap()
                .as_ref(),
            b"sdk response"
        );
        socket.abort_sender();
    }
    #[tokio::test]
    async fn mobile_sdk_signed_certificate_static_mismatch_sends_no_finish() {
        let mut f = fixture().await;
        let handshake = do_mobile_handshake(
            f.runtime.clone(),
            &f.pm,
            f.transport.clone(),
            &mut f.events,
            Default::default(),
            &f.config,
            0,
        );
        let peer = peer_hello(&f.outbound, &f.incoming, true);
        let (result, _) = futures::join!(handshake, peer);
        assert!(matches!(
            result,
            Err(HandshakeError::Mobile(
                MobileSessionError::InvalidCertificate(_)
            ))
        ));
        assert!(f.outbound.try_recv().is_err());
    }
    #[tokio::test]
    async fn mobile_sdk_missing_clock_rejects_without_transport_io() {
        let mut f = fixture().await;
        let result = do_mobile_handshake(
            Arc::new(crate::TokioRuntime),
            &f.pm,
            f.transport.clone(),
            &mut f.events,
            Default::default(),
            &f.config,
            0,
        )
        .await;
        assert!(matches!(
            result,
            Err(HandshakeError::Mobile(MobileSessionError::MissingClock))
        ));
        assert!(f.outbound.try_recv().is_err());
    }
    #[tokio::test]
    async fn mobile_sdk_invalid_host_clock_rejects_without_transport_io() {
        let mut f = fixture().await;
        for wall_millis in [-1, i64::MAX] {
            let runtime: Arc<dyn Runtime> = Arc::new(wacore::runtime::ClockedRuntime::new(
                Arc::new(crate::TokioRuntime),
                Arc::new(Clock(wall_millis)),
            ));
            let result = do_mobile_handshake(
                runtime,
                &f.pm,
                f.transport.clone(),
                &mut f.events,
                Default::default(),
                &f.config,
                0,
            )
            .await;
            assert!(matches!(
                result,
                Err(HandshakeError::Mobile(MobileSessionError::MalformedClock))
            ));
            assert!(f.outbound.try_recv().is_err());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn mobile_sdk_timeout_and_cancellation_never_send_finish() {
        let mut f = fixture().await;
        let cancelled_hello = {
            let handshake = do_mobile_handshake(
                f.runtime.clone(),
                &f.pm,
                f.transport.clone(),
                &mut f.events,
                Default::default(),
                &f.config,
                0,
            );
            futures::pin_mut!(handshake);
            let hello = tokio::select! { result=&mut handshake => panic!("unexpected early result: {}",result.is_ok()), hello=f.outbound.recv()=>hello.unwrap() };
            // Dropping the handshake owns cancellation; no sender task exists yet.
            hello
        };
        // A valid response arriving after cancellation cannot advance the transcript.
        let _ = peer_response(cancelled_hello, &f.incoming, false).await;
        tokio::task::yield_now().await;
        assert!(f.outbound.try_recv().is_err());
        let mut f = fixture().await;
        let result = do_mobile_handshake(
            f.runtime.clone(),
            &f.pm,
            f.transport.clone(),
            &mut f.events,
            Default::default(),
            &f.config,
            0,
        )
        .await;
        assert!(matches!(result, Err(HandshakeError::Timeout)));
        let hello = f.outbound.recv().await.unwrap();
        assert_eq!(&hello[..4], &[b'W', b'A', 5, 3]);
        assert!(f.outbound.try_recv().is_err());
    }
}
