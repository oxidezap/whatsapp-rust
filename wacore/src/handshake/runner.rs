use log::{debug, info, warn};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::net::{Transport, TransportEvent};
use crate::runtime::{Runtime, timeout as rt_timeout};
use crate::store::Device;
use wacore_binary::consts::WA_CONN_HEADER;
use wacore_noise::{
    HandshakeError as CoreHandshakeError, IkHandshakeState, IkServerHelloOutcome, NoiseCertPolicy,
    NoiseCipher, NoiseError, VerifiedServerCertChain, XxFallbackHandshakeState, XxHandshakeState,
    build_handshake_header,
};

pub const NOISE_HANDSHAKE_RESPONSE_TIMEOUT: Duration = Duration::from_secs(20);

/// One IK failure per process before falling back to XX (matches WA Web).
pub const IK_FAILURE_THRESHOLD: u32 = 1;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HandshakeError {
    #[error("Transport error: {0}")]
    Transport(#[from] anyhow::Error),
    #[error("Core handshake error: {0}")]
    Core(#[from] CoreHandshakeError),
    #[error("Timed out waiting for handshake response")]
    Timeout,
    /// Producer side of `transport_events` was dropped; distinct from a
    /// timeout because nothing more will ever arrive on the channel,
    /// regardless of how long we wait. Surfaced separately so callers can
    /// log it accurately and so retry policies that pace themselves on
    /// timeout don't silently swallow a teardown.
    #[error("Transport event stream closed before handshake completed")]
    StreamClosed,
    #[error("Disconnected during handshake")]
    Disconnected,
    #[error("Unexpected event during handshake: {0}")]
    UnexpectedEvent(String),
}

impl HandshakeError {
    /// The handshake ran out of time, as opposed to being torn down.
    pub fn is_timeout(&self) -> bool {
        match self {
            HandshakeError::Timeout => true,
            HandshakeError::Transport(_)
            | HandshakeError::Core(_)
            | HandshakeError::StreamClosed
            | HandshakeError::Disconnected
            | HandshakeError::UnexpectedEvent(_) => false,
        }
    }

    /// Transient errors that are expected during reconnect and will resolve
    /// on retry. These never invalidate the cached server static.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            Self::Transport(_) | Self::Timeout | Self::Disconnected | Self::StreamClosed
        )
    }

    /// Crypto-fatal: a cached server static or cert chain is no longer
    /// trustworthy. The orchestration layer must clear the IK cache and
    /// fall back to XX on the next attempt.
    pub fn is_crypto_fatal(&self) -> bool {
        let Self::Core(inner) = self else {
            return false;
        };
        match inner {
            CoreHandshakeError::Noise(NoiseError::Decrypt(_))
            | CoreHandshakeError::Noise(NoiseError::CiphertextTooShort)
            | CoreHandshakeError::Noise(NoiseError::InvalidKeyLength { .. }) => true,
            CoreHandshakeError::CertVerification(_) => true,
            CoreHandshakeError::IncompleteResponse
            | CoreHandshakeError::InvalidLength { .. }
            | CoreHandshakeError::InvalidKeyLength
            | CoreHandshakeError::ProtoDecode(_) => true,
            CoreHandshakeError::Crypto(_)
            | CoreHandshakeError::Noise(NoiseError::Encrypt(_))
            | CoreHandshakeError::Noise(NoiseError::HkdfExpandFailed)
            | CoreHandshakeError::Noise(NoiseError::InvalidPatternLength { .. })
            | CoreHandshakeError::Noise(NoiseError::CounterExhausted) => false,
        }
    }
}

pub type Result<T> = std::result::Result<T, HandshakeError>;

/// Pattern picked at the start of a handshake based on cached state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakePattern {
    /// Cold start / pairing / forced fallback after an earlier IK failure.
    Xx,
    /// Cached server static + valid cert chain available; attempt IK.
    Ik([u8; 32]),
}

pub fn select_pattern(
    device: &Device,
    ik_failures: u32,
    now_secs: i64,
    cert_policy: NoiseCertPolicy,
) -> HandshakePattern {
    if !cert_policy.reuse_cached_chain() {
        return HandshakePattern::Xx;
    }
    if !device.is_registered() {
        return HandshakePattern::Xx;
    }
    if ik_failures >= IK_FAILURE_THRESHOLD {
        return HandshakePattern::Xx;
    }
    let Some(chain) = device.server_cert_chain.as_ref() else {
        return HandshakePattern::Xx;
    };
    if !chain.signature_verified {
        return HandshakePattern::Xx;
    }
    if now_secs < chain.leaf.not_before
        || now_secs < chain.intermediate.not_before
        || now_secs >= chain.leaf.not_after
        || now_secs >= chain.intermediate.not_after
    {
        return HandshakePattern::Xx;
    }
    HandshakePattern::Ik(chain.leaf.key)
}

/// `server_cert_chain` is `Some` for an XX / XX-fallback chain whose
/// signatures were actually checked, and `None` for IK Continue (on-disk
/// cache stays authoritative) or a bypass-accepted chain (nothing trusted
/// to persist).
pub struct HandshakeSuccess {
    pub write_cipher: NoiseCipher,
    pub read_cipher: NoiseCipher,
    pub server_cert_chain: Option<VerifiedServerCertChain>,
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.conn.handshake.xx", level = "debug", skip_all, err(Debug))
)]
pub async fn run_xx_handshake(
    runtime: &Arc<dyn Runtime>,
    device: &Device,
    transport: Arc<dyn Transport>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    cert_policy: NoiseCertPolicy,
) -> Result<HandshakeSuccess> {
    let client_payload = waproto::codec::client_payload_to_vec(&device.get_client_payload());
    let mut handshake_state = XxHandshakeState::new_with_cert_policy(
        device.noise_key.clone(),
        client_payload,
        &WA_CONN_HEADER,
        cert_policy,
    )?;
    let mut frame_decoder = crate::framing::FrameDecoder::new();

    let client_hello_bytes = handshake_state.build_client_hello()?;
    send_first_handshake_message(&transport, device, &client_hello_bytes).await?;

    let resp_frame = recv_frame(runtime, transport_events, &mut frame_decoder).await?;
    debug!("[socket] openChatSocket rcv hello");

    let client_finish_bytes =
        handshake_state.read_server_hello_and_build_client_finish(&resp_frame)?;

    debug!("[socket] continueFullHandshakeCore client finish and deriving secrets");
    let framed = crate::framing::encode_frame(&client_finish_bytes, None)
        .map_err(HandshakeError::Transport)?;
    transport.send(bytes::Bytes::from(framed)).await?;

    let outcome = handshake_state.finish()?;
    info!("Handshake complete (XX), switching to encrypted communication");

    Ok(HandshakeSuccess {
        write_cipher: outcome.write_cipher,
        read_cipher: outcome.read_cipher,
        server_cert_chain: outcome.server_cert_chain,
    })
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.conn.handshake.ik", level = "debug", skip_all, err(Debug))
)]
pub async fn run_ik_handshake(
    runtime: &Arc<dyn Runtime>,
    device: &Device,
    server_static_pub: [u8; 32],
    transport: Arc<dyn Transport>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    fallback_taken: &mut bool,
    cert_policy: NoiseCertPolicy,
) -> Result<HandshakeSuccess> {
    let client_payload = waproto::codec::client_payload_to_vec(&device.get_client_payload());
    let mut ik = IkHandshakeState::new(
        device.noise_key.clone(),
        server_static_pub,
        client_payload,
        &WA_CONN_HEADER,
    )?;
    let mut frame_decoder = crate::framing::FrameDecoder::new();

    debug!("[socket] resumeNoiseHandshake send hello");
    let client_hello_bytes = ik.build_client_hello()?;
    send_first_handshake_message(&transport, device, &client_hello_bytes).await?;

    let resp_frame = recv_frame(runtime, transport_events, &mut frame_decoder).await?;
    debug!("[socket] resumeNoiseHandshake rcv hello");

    match ik.read_server_hello(&resp_frame)? {
        IkServerHelloOutcome::Continue(out) => {
            debug!("[socket] resumeNoiseHandshake deriving secrets");
            info!("Handshake complete (IK), switching to encrypted communication");
            Ok(HandshakeSuccess {
                write_cipher: out.write_cipher,
                read_cipher: out.read_cipher,
                server_cert_chain: None,
            })
        }
        IkServerHelloOutcome::Fallback(inputs) => {
            *fallback_taken = true;
            debug!(
                "[socket] resumeNoiseHandshake failed: serverStaticCiphertext not null: \
                 doFallbackHandshake continuing handshake with given server hello"
            );
            let mut fb = XxFallbackHandshakeState::from_ik_failure_with_cert_policy(
                *inputs,
                &WA_CONN_HEADER,
                cert_policy,
            )?;
            let client_finish_bytes = fb.build_client_finish()?;
            debug!(
                "[socket] continueFullHandshakeCore client finish and deriving secrets (XXfallback)"
            );
            let framed = crate::framing::encode_frame(&client_finish_bytes, None)
                .map_err(HandshakeError::Transport)?;
            transport.send(bytes::Bytes::from(framed)).await?;
            let outcome = fb.finish()?;
            info!("Handshake complete (XXfallback), switching to encrypted communication");
            Ok(HandshakeSuccess {
                write_cipher: outcome.write_cipher,
                read_cipher: outcome.read_cipher,
                server_cert_chain: outcome.server_cert_chain,
            })
        }
    }
}

pub async fn send_first_handshake_message(
    transport: &Arc<dyn Transport>,
    device: &Device,
    payload_bytes: &[u8],
) -> Result<()> {
    let (header, used_edge_routing) = build_handshake_header(device.edge_routing_info.as_deref());
    if used_edge_routing {
        debug!("Sending edge routing pre-intro for optimized reconnection");
    } else if device.edge_routing_info.is_some() {
        warn!("Edge routing info provided but not used (possibly too large)");
    }
    let framed = crate::framing::encode_frame(payload_bytes, Some(&header))
        .map_err(HandshakeError::Transport)?;
    transport.send(bytes::Bytes::from(framed)).await?;
    Ok(())
}

pub async fn recv_frame(
    runtime: &Arc<dyn Runtime>,
    transport_events: &mut async_channel::Receiver<TransportEvent>,
    frame_decoder: &mut crate::framing::FrameDecoder,
) -> Result<bytes::BytesMut> {
    let assemble = async {
        loop {
            match transport_events.recv().await {
                Ok(TransportEvent::DataReceived(data)) => {
                    frame_decoder.feed_owned(data);
                    if let Some(frame) = frame_decoder.decode_frame() {
                        return Ok(frame);
                    }
                }
                Ok(TransportEvent::Connected) => continue,
                Ok(TransportEvent::Disconnected(reason)) => {
                    debug!("Transport disconnected during handshake: {reason}");
                    return Err(HandshakeError::Disconnected);
                }
                Err(_) => return Err(HandshakeError::StreamClosed),
            }
        }
    };
    match rt_timeout(&**runtime, NOISE_HANDSHAKE_RESPONSE_TIMEOUT, assemble).await {
        Ok(result) => result,
        Err(_) => Err(HandshakeError::Timeout),
    }
}
