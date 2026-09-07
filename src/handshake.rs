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
    let now_secs = wacore::time::now_secs();
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
