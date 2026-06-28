//! The sans-IO call engine: a str0m-shaped state machine that owns the relay control plane (STUN
//! allocate, 1s keepalive, consent-freshness replies) and, optionally, the media plane (MLow
//! encode/decode + E2E-SRTP + SFrame + a 20ms playout jitter buffer). It owns no socket, no clock,
//! and no thread. The shell performs a single mutation (`handle_input`), drains `poll_output()`
//! until it yields `Output::Timeout`, executes each intent, and arms one timer for that deadline.
//!
//! Time is monotonic milliseconds supplied by the shell; the engine never reads a clock. The only
//! non-deterministic input is the STUN transaction id, injected via [`TxIdSource`], so the whole
//! engine is deterministically testable.
//!
//! This is the portable orchestration the example's `run_media` task did by hand, lifted into pure
//! logic so the Tokio driver, the WASM bridge, and (for the control plane) embedded consumers all
//! drive one implementation.

use std::collections::VecDeque;

use bytes::Bytes;

use super::demux::{RelayPacketKind, classify_relay_packet};
use super::session::{CallDirection, MediaPipeline, MediaPipelineParams};
use super::sframe::{SframeIn, SframeSession};
use super::{is_standard_opus_frame, mlow, stun};

/// Monotonic milliseconds. The shell supplies it; the engine never reads a clock.
pub type Millis = u64;

/// Sentinel deadline meaning "no timer pending"; the shell waits only on I/O until the next input.
pub const NEVER: Millis = u64::MAX;

/// Relay consent-freshness cadence: re-send the STUN allocate + a WA ping every second. The relay
/// drops the client after ~4s without traffic, which is what makes the peer reconnect/terminate.
const KEEPALIVE_MS: Millis = 1000;
/// Deadline for the relay to ack the allocate. Past this with no success the relay is wedged
/// (silently dropping the allocate), so surface a terminal timeout instead of keepaliving forever.
const ALLOCATE_TIMEOUT_MS: Millis = 10_000;
/// Playout drain cadence: hand the speaker a fixed slice every 20ms so it stays fed at 16kHz.
const PLAYOUT_MS: Millis = 20;
/// 20ms @ 16kHz: samples drained to the speaker per playout tick.
const PLAYOUT_DRAIN: usize = 320;
/// ~150ms latency ceiling; a burst past this resyncs (drops oldest) instead of lagging.
const PLAYOUT_CAP: usize = 2400;
/// Prebuffer target: prime playout until the jitter buffer holds two 60ms peer frames, so the
/// steady-state buffer never drains below one frame (a 60ms cushion that absorbs the relay's
/// inter-arrival jitter). Priming to a single frame is a zero cushion: that one frame drains away
/// over its own 60ms cycle, so the buffer returns to empty before the next packet and any late
/// arrival underruns. The cushion has to be one frame above what the per-cycle drain consumes.
const PLAYOUT_TARGET: usize = 1920;
/// Bound on how long playout primes before flushing a partial buffer: if the peer sends one frame
/// then goes DTX the jitter buffer never reaches `PLAYOUT_TARGET`, so after this many 20ms ticks
/// (~200ms) drain whatever is queued instead of holding it (silent) forever. Comfortably above the
/// few ticks a normal jittered second-frame arrival takes, so it never trips in steady operation.
const MAX_PRIME_TICKS: u32 = 10;
/// One-byte mlow DTX comfort-noise token sent on a muted (exact-zero) mic frame so the media stream
/// never gaps; protect_audio frames it with the DTX RTP header and the peer decodes it to silence.
const MLOW_DTX_CNG: [u8; 1] = [0x90];
/// One 60ms MLow frame at 16kHz. `Input::MicFrame` must carry exactly this; a wrong-length buffer is
/// dropped (the encoder requires it), never sent.
const MIC_FRAME_SAMPLES: usize = 960;

/// Supplies STUN transaction ids. Injected so the core stays RNG-free and deterministically
/// testable. Production shells MUST back this with a real RNG (the ids gate consent freshness);
/// [`SequentialTxIds`] is for tests and deterministic drives only.
pub trait TxIdSource: crate::sync_marker::MaybeSendSync {
    fn next_tx_id(&mut self) -> [u8; 12];
}

/// Deterministic counter ids for tests / deterministic drives. NOT for production: predictable
/// transaction ids weaken consent freshness. Doc-hidden and kept off the `voip` facade so a
/// consumer never reaches for it; production shells default to an OS-RNG `TxIdSource` (`RandTxIds`).
#[doc(hidden)]
#[derive(Default)]
pub struct SequentialTxIds(u64);

impl SequentialTxIds {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TxIdSource for SequentialTxIds {
    fn next_tx_id(&mut self) -> [u8; 12] {
        self.0 = self.0.wrapping_add(1);
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&self.0.to_be_bytes());
        id
    }
}

/// Everything the engine needs to be self-contained for one call. The relay fields come from the
/// parsed `<relay>` stanza; the crypto fields from the decrypted callKey and our/our-peer LIDs.
/// Build it via [`for_incoming`](Self::for_incoming) / [`for_outgoing`](Self::for_outgoing), which
/// validate the relay block.
#[derive(Clone)]
pub struct CallConfig {
    pub call_id: String,
    pub direction: CallDirection,
    /// Our own participant LID (the E2E-SRTP send keys are derived from this).
    pub self_lid: String,
    /// The peer's participant LID (the E2E-SRTP recv keys are derived from this).
    pub peer_lid: String,
    /// The 32-byte callKey.
    pub call_key: Vec<u8>,
    pub ssrc: u32,
    /// RTP timestamp stride per packet. NOTE: the MLow encoder requires exactly 960-sample (60ms @
    /// 16kHz) frames, so `Input::MicFrame` must carry 960 samples regardless of this value (a
    /// wrong-length frame is dropped, no RTP sent). Set to 960 unless the codec changes.
    pub samples_per_packet: u32,
    /// Relay endpoint allocate inputs.
    pub relay_token: Vec<u8>,
    pub relay_ip: String,
    pub relay_port: u16,
    /// The relay `<key>` (ASCII) used as the STUN MESSAGE-INTEGRITY key.
    pub integrity_key: Vec<u8>,
    /// The relay `<warp_mi_tag_len>` (default 4); a non-4 length must not desync the WARP MI tag.
    pub warp_mi_tag_len: usize,
    /// Run the media plane (MLow + playout). Off for the esp32 control plane.
    pub enable_media: bool,
    /// Decrypt inbound SFrame, with a plaintext fallback (the Android peer may GCM-wrap its
    /// Opus/MLow). Recv-side only by design: outbound stays plain codec inside WAHKDF SRTP, which
    /// the peer accepts, matching the pre-refactor pipeline (`MediaPipeline`: "SFrame is omitted,
    /// default-off on send"). Send-side SFrame is intentionally not wired.
    pub enable_sframe: bool,
}

// Manual Debug so a stray `{:?}` can't leak the SRTP callKey, the STUN integrity key, or the relay
// token (all live call credentials), matching the redaction the sibling key structs already apply
// (E2eSrtpKeys, SrtpKeyingMaterial).
impl core::fmt::Debug for CallConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CallConfig")
            .field("call_id", &self.call_id)
            .field("direction", &self.direction)
            .field("self_lid", &self.self_lid)
            .field("peer_lid", &self.peer_lid)
            .field("call_key", &"[redacted]")
            .field("ssrc", &self.ssrc)
            .field("samples_per_packet", &self.samples_per_packet)
            .field("relay_token", &"[redacted]")
            .field("relay_ip", &self.relay_ip)
            .field("relay_port", &self.relay_port)
            .field("integrity_key", &"[redacted]")
            .field("warp_mi_tag_len", &self.warp_mi_tag_len)
            .field("enable_media", &self.enable_media)
            .field("enable_sframe", &self.enable_sframe)
            .finish()
    }
}

/// Why the engine could not be constructed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    #[error("callKey too short for E2E keys (need 32 bytes)")]
    BadCallKey,
    #[error("relay endpoint is not a valid IPv4 address")]
    BadEndpoint,
}

/// Why an incoming call's [`CallConfig`] could not be assembled from the offer's relay block.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SetupError {
    #[error("relay has no endpoints")]
    NoRelayEndpoint,
    #[error("relay endpoint has no IPv4 address")]
    NoRelayIpv4,
    #[error("relay has no token #{0}")]
    NoRelayToken(u32),
    #[error("relay has no <key> (STUN integrity key)")]
    NoIntegrityKey,
    /// The relay advertised a WARP MI tag length the SRTP layer can't honor (the tag is sliced from a
    /// 20-byte HMAC-SHA1 digest, so 1..=20 is the only valid range).
    #[error("relay advertised an unsupported WARP MI tag length: {0}")]
    BadWarpMiTagLen(usize),
}

impl CallConfig {
    /// Assemble the engine config from the callKey and the parsed `<relay>`. Pure: derives our
    /// participant SSRC (E2E HKDF over our self LID) and pulls the relay endpoint / token /
    /// integrity-key out of `relay`, so the whole media-config build is offline testable. The only
    /// thing that differs by direction is the `direction` field; everything else is identical.
    /// `enable_sframe` is on (the Android peer may GCM-wrap its codec; recv-decrypt only).
    fn from_relay(
        direction: CallDirection,
        call_id: &str,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &super::relay_parse::RelayData,
    ) -> Result<Self, SetupError> {
        use super::{relay_parse, ssrc};

        let ep = relay_parse::get_media_relay_endpoint(relay).ok_or(SetupError::NoRelayEndpoint)?;
        let (relay_ip, relay_port) =
            relay_parse::get_primary_ipv4_address(ep).ok_or(SetupError::NoRelayIpv4)?;
        // A padded-empty slot (a sparse token block) is a missing token, not a zero-length one: reject
        // it here so the nothing-usable fallback surfaces a precise NoRelayToken instead of dialing the
        // relay with an empty token and failing at the allocate.
        let relay_token = relay
            .relay_tokens
            .get(ep.token_id as usize)
            .filter(|t| !t.is_empty())
            .cloned()
            .ok_or(SetupError::NoRelayToken(ep.token_id))?;
        // The relay <key> is the STUN MESSAGE-INTEGRITY key; without it the allocate/binding-success
        // we sign can't authenticate, so fail here rather than dial with an empty key. Sign with the
        // base64 TEXT of <key> (relay_key_ascii), NOT its decoded bytes (relay.relay_key): the relay
        // HMACs against the ASCII key material, so decoding first fails the allocate (verified against
        // the WhatsApp client; raw decoded bytes were the original bug).
        let integrity_key = relay
            .relay_key_ascii
            .clone()
            .ok_or(SetupError::NoIntegrityKey)?;

        let our_ssrc = ssrc::derive_wasm_participant_ssrc(
            call_id,
            &ssrc::format_e2e_srtp_participant_id(self_lid),
            0,
        );

        // Default to 4 when absent; reject an out-of-range relay value here (a distinct relay-protocol
        // error) rather than letting it collapse into BadCallKey when the SRTP layer rejects it.
        let warp_mi_tag_len = relay
            .warp_mi_tag_len
            .map(|n| n as usize)
            .unwrap_or(super::warp::WARP_MI_TAG_LEN);
        if !(1..=20).contains(&warp_mi_tag_len) {
            return Err(SetupError::BadWarpMiTagLen(warp_mi_tag_len));
        }

        Ok(CallConfig {
            call_id: call_id.to_string(),
            direction,
            self_lid: self_lid.to_string(),
            peer_lid: peer_lid.to_string(),
            call_key,
            ssrc: our_ssrc,
            // The MLow encoder requires exactly 960-sample frames.
            samples_per_packet: 960,
            relay_token,
            relay_ip,
            relay_port,
            integrity_key,
            warp_mi_tag_len,
            enable_media: true,
            enable_sframe: true,
        })
    }

    /// Engine config for an INCOMING call: the callKey was decrypted from the peer's offer.
    pub fn for_incoming(
        call_id: &str,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &super::relay_parse::RelayData,
    ) -> Result<Self, SetupError> {
        Self::from_relay(
            CallDirection::Incoming,
            call_id,
            self_lid,
            peer_lid,
            call_key,
            relay,
        )
    }

    /// Engine config for an OUTGOING call: the callKey is the one WE generated, and the relay block is
    /// the one the server hands back after the offer.
    pub fn for_outgoing(
        call_id: &str,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &super::relay_parse::RelayData,
    ) -> Result<Self, SetupError> {
        Self::from_relay(
            CallDirection::Outgoing,
            call_id,
            self_lid,
            peer_lid,
            call_key,
            relay,
        )
    }
}

/// One input to the engine, applied with the current monotonic timestamp.
pub enum Input<'a> {
    /// A packet arrived on the relay channel (one DataChannel/datagram message).
    RelayPacket(&'a [u8]),
    /// A 60ms PCM frame captured from the local mic (16kHz mono). Must be exactly 960 samples (the
    /// MLow frame size); a wrong-length frame is silently dropped by the encoder (no RTP sent).
    MicFrame(&'a [i16]),
    /// The deadline that `poll_output`/`poll_timeout` last reported has fired.
    Timeout,
}

/// One intent emitted by the engine, drained via `poll_output` until `Timeout`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Output {
    /// Send these bytes over the relay channel.
    Transmit(Bytes),
    /// Decoded PCM for the speaker (16kHz mono).
    Playout(Vec<i16>),
    /// A call lifecycle / diagnostic event.
    Event(CallEvent),
    /// Drained: arm a timer for this monotonic-ms deadline ([`NEVER`] = no timer).
    Timeout(Millis),
}

/// Lifecycle / diagnostic events the shell may act on or surface.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallEvent {
    /// The relay accepted our allocate (an allocate/binding success arrived); media path is live.
    RelayAllocated,
    /// An inbound audio payload the core won't decode itself (a standard-Opus frame, not MLow).
    /// The shell may decode it with a platform codec (libopus). Carries the decrypted payload.
    ForeignAudio(Bytes),
    /// The relay rejected our allocate. Terminal; carries the STUN error code (class*100 + number).
    RelayAllocateFailed(u16),
    /// The relay never acked the allocate within the deadline (wedged relay). Terminal.
    RelayAllocateTimedOut,
}

/// The optional media plane: the SRTP pipeline, the MLow codec, an optional SFrame session, and
/// the playout jitter buffer. One `MediaPipeline` serves both directions: protect uses its send
/// keys/ROC/RTP state, unprotect its recv keys/ROC, and those fields are disjoint.
struct MediaState {
    pipe: MediaPipeline,
    /// Retained so the caller can re-derive the recv keys once the answering device is known (the
    /// callee's `<accept>` carries its device LID). See [`CallEngine::rekey_recv`].
    call_key: Vec<u8>,
    sframe: Option<SframeSession>,
    encoder: mlow::MlowEncoder,
    decoder: mlow::MlowDecoder,
    /// Reused per outbound frame to hold the i16->f32 conversion, so the encode hot path doesn't
    /// allocate a fresh Vec each frame.
    scratch: Vec<f32>,
    jitter: VecDeque<i16>,
    playout_deadline: Millis,
    /// Playout emits silence (without draining) while the jitter buffer fills to `PLAYOUT_TARGET`, so
    /// a late packet costs one re-prime instead of a silence gap every 20ms tick. Re-armed on underrun.
    priming: bool,
    /// Consecutive playout ticks spent priming; bounds the wait so a partial buffer (the peer sent one
    /// frame then went DTX) is flushed after `MAX_PRIME_TICKS` instead of being held silent forever.
    priming_ticks: u32,
}

/// The sans-IO call engine. See the module docs for the drive contract.
pub struct CallEngine {
    call_id: String,
    direction: CallDirection,
    // Control plane.
    relay_token: Vec<u8>,
    endpoint_xor: [u8; 6],
    integrity_key: Vec<u8>,
    allocate: Bytes,
    tx_ids: Box<dyn TxIdSource>,
    keepalive_deadline: Millis,
    /// Deadline by which the allocate must be acked; NEVER once it is (or after firing the timeout).
    allocate_deadline: Millis,
    allocated: bool,
    started: bool,
    /// A terminal relay-allocate failure was surfaced; the engine goes inert (no keepalive, no
    /// timer, no further transmits) so the driver tears the call down instead of keepaliving a
    /// dead relay forever.
    terminated: bool,
    // Media plane (None = control plane only, e.g. esp32).
    media: Option<MediaState>,
    outbox: VecDeque<Output>,
}

impl CallEngine {
    /// Build the engine. Derives the E2E-SRTP keys and the XOR relay endpoint up front so a
    /// malformed callKey or relay address fails here rather than mid-call. Does not touch the
    /// timestamp or the tx-id source; call [`start`](Self::start) once the relay channel is open.
    // Lifecycle span only. The LID and callKey fields are PII/secret, so the config is skipped and
    // only the non-sensitive call_id/direction/media-flag are recorded.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.voip.engine_new",
            level = "debug",
            skip_all,
            fields(call_id = %config.call_id, dir = ?config.direction, media = config.enable_media),
            err(Debug)
        )
    )]
    pub fn new(config: CallConfig, tx_ids: Box<dyn TxIdSource>) -> Result<Self, EngineError> {
        let endpoint_xor = stun::encode_xor_relay_endpoint(&config.relay_ip, config.relay_port)
            .ok_or(EngineError::BadEndpoint)?;

        let media = if config.enable_media {
            let pipe = MediaPipeline::new(&MediaPipelineParams {
                call_key: &config.call_key,
                self_lid: &config.self_lid,
                peer_lid: &config.peer_lid,
                ssrc: config.ssrc,
                samples_per_packet: config.samples_per_packet,
                warp_mi_tag_len: config.warp_mi_tag_len,
            })
            .ok_or(EngineError::BadCallKey)?;
            let sframe = if config.enable_sframe {
                SframeSession::new(&config.call_key, &config.self_lid, &config.peer_lid)
            } else {
                None
            };
            Some(MediaState {
                pipe,
                call_key: config.call_key.clone(),
                sframe,
                encoder: mlow::MlowEncoder::new(),
                decoder: mlow::MlowDecoder::new(),
                scratch: Vec::with_capacity(config.samples_per_packet as usize),
                jitter: VecDeque::new(),
                playout_deadline: 0,
                priming: true,
                priming_ticks: 0,
            })
        } else {
            None
        };

        Ok(Self {
            call_id: config.call_id,
            direction: config.direction,
            relay_token: config.relay_token,
            endpoint_xor,
            integrity_key: config.integrity_key,
            allocate: Bytes::new(),
            tx_ids,
            keepalive_deadline: 0,
            allocate_deadline: 0,
            allocated: false,
            started: false,
            terminated: false,
            media,
            outbox: VecDeque::new(),
        })
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    pub fn direction(&self) -> CallDirection {
        self.direction
    }

    /// Whether the relay has acknowledged our allocate.
    pub fn is_allocated(&self) -> bool {
        self.allocated
    }

    /// Whether a terminal relay-allocate failure has been surfaced. Once true the engine is inert
    /// (emits nothing further); the driver breaks its loop and tears the call down.
    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    /// Caller-side: rekey the recv path to the device that ANSWERED (its LID arrives in the callee's
    /// `<accept>`). The dialed base callee LID is wrong once a companion device answers — without this
    /// every inbound frame decrypts to garbage. No-op (`true`) for a control-only engine (no media).
    /// `false` means the stored call_key is malformed (a setup invariant), so the driver ends the call.
    pub fn rekey_recv(&mut self, answering_peer_lid: &str) -> bool {
        let Some(m) = self.media.as_mut() else {
            return true;
        };
        m.pipe.rekey_recv(&m.call_key, answering_peer_lid)
    }

    /// Begin the media session: build and emit the initial STUN allocate and arm the keepalive
    /// (and playout, if media is enabled) timers. Idempotent.
    pub fn start(&mut self, now: Millis) {
        if self.started {
            return;
        }
        self.started = true;
        let tx = self.tx_ids.next_tx_id();
        // Built once here; the 1s keepalive re-sends it, so store it as Bytes and refcount-clone
        // rather than re-allocating the buffer every tick.
        self.allocate = Bytes::from(stun::build_wasm_stun_allocate_request(
            &tx,
            &self.relay_token,
            &self.endpoint_xor,
            &self.integrity_key,
        ));
        self.outbox
            .push_back(Output::Transmit(self.allocate.clone()));
        self.keepalive_deadline = now + KEEPALIVE_MS;
        self.allocate_deadline = now + ALLOCATE_TIMEOUT_MS;
        if let Some(m) = &mut self.media {
            m.playout_deadline = now + PLAYOUT_MS;
        }
    }

    /// Apply one input at time `now`.
    pub fn handle_input(&mut self, now: Millis, input: Input<'_>) {
        // Inert after a terminal failure: emit no further intents (the driver is tearing down).
        if self.terminated {
            return;
        }
        match input {
            Input::Timeout => self.on_timeout(now),
            Input::RelayPacket(pkt) => self.on_packet(pkt),
            Input::MicFrame(pcm) => self.on_mic(pcm),
        }
    }

    /// Drain one intent. Returns `Output::Timeout(deadline)` once the queue is empty; the shell
    /// stops draining there and arms a timer for `deadline` ([`NEVER`] = none).
    pub fn poll_output(&mut self) -> Output {
        self.outbox
            .pop_front()
            .unwrap_or(Output::Timeout(self.poll_timeout().unwrap_or(NEVER)))
    }

    /// The next deadline (the nearer of the keepalive and, if media is on, the playout tick), or
    /// `None` before `start`. Computed on demand from the two deadline fields.
    pub fn poll_timeout(&self) -> Option<Millis> {
        if !self.started || self.terminated {
            return None;
        }
        let mut next = self.keepalive_deadline;
        // The allocate timeout only matters while the allocate is still outstanding.
        if !self.allocated && self.allocate_deadline != NEVER {
            next = next.min(self.allocate_deadline);
        }
        if let Some(m) = &self.media {
            next = next.min(m.playout_deadline);
        }
        Some(next)
    }

    /// Current playout jitter-buffer depth in samples. Test-only: lets coverage assert the
    /// feed-side bound without exposing the media plane.
    #[cfg(test)]
    pub(crate) fn jitter_len(&self) -> usize {
        self.media.as_ref().map_or(0, |m| m.jitter.len())
    }

    fn on_timeout(&mut self, now: Millis) {
        // The relay never acked the allocate: surface a terminal timeout exactly once, then go inert.
        if !self.allocated
            && self.started
            && self.allocate_deadline != NEVER
            && now >= self.allocate_deadline
        {
            self.allocate_deadline = NEVER;
            self.terminated = true;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, "voip relay allocate timed out");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocateTimedOut));
            return;
        }
        if self.started && now >= self.keepalive_deadline {
            // Re-send the same allocate (consent freshness) plus a fresh-id WA ping.
            self.outbox
                .push_back(Output::Transmit(self.allocate.clone()));
            let tx = self.tx_ids.next_tx_id();
            let ping = stun::build_whatsapp_ping(&tx);
            self.outbox
                .push_back(Output::Transmit(Bytes::copy_from_slice(&ping)));
            self.keepalive_deadline = next_tick(self.keepalive_deadline, now, KEEPALIVE_MS);
        }
        if let Some(m) = self.media.as_mut()
            && self.started
            && now >= m.playout_deadline
        {
            let frame = drain_playout(&mut m.jitter, &mut m.priming, &mut m.priming_ticks);
            m.playout_deadline = next_tick(m.playout_deadline, now, PLAYOUT_MS);
            self.outbox.push_back(Output::Playout(frame));
        }
    }

    fn on_packet(&mut self, pkt: &[u8]) {
        match classify_relay_packet(pkt) {
            RelayPacketKind::Stun => self.on_stun(pkt),
            RelayPacketKind::Rtp => self.on_rtp(pkt),
            // RTCP / other: no behavior yet (stats are a later concern).
            RelayPacketKind::Rtcp | RelayPacketKind::Other => {}
        }
    }

    fn on_stun(&mut self, pkt: &[u8]) {
        // Consent freshness (RFC 7675): answer a binding request with a binding success.
        if stun::stun_message_type(pkt) == Some(stun::MSG_BINDING_REQUEST)
            && let Some(req_tx) = stun::stun_transaction_id(pkt)
            && req_tx.len() == 12
        {
            let mut tx12 = [0u8; 12];
            tx12.copy_from_slice(req_tx);
            let resp = stun::encode_stun_request(
                stun::MSG_BINDING_SUCCESS,
                &tx12,
                &[],
                Some(&self.integrity_key),
                true,
            );
            self.outbox.push_back(Output::Transmit(Bytes::from(resp)));
        }
        // The relay acknowledged our allocate; surface it once and stop the allocate timer.
        if !self.allocated && stun::is_allocate_or_binding_success(pkt) {
            self.allocated = true;
            self.allocate_deadline = NEVER;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, "voip relay allocated");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocated));
            return;
        }
        // A complete allocate-error (a parsed ERROR-CODE) terminates the call; STUN-typed garbage
        // whose error code cannot be parsed is ignored rather than hanging up.
        if !self.allocated
            && self.allocate_deadline != NEVER
            && stun::is_allocate_error(pkt)
            && let Some(code) = stun::parse_stun_error_code(pkt)
        {
            self.allocate_deadline = NEVER;
            self.terminated = true;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, code, "voip relay allocate failed");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocateFailed(code)));
        }
    }

    fn on_rtp(&mut self, pkt: &[u8]) {
        let Some(m) = self.media.as_mut() else {
            return;
        };
        let Some((_, payload)) = m.pipe.unprotect_audio(pkt) else {
            return;
        };
        // SFrame on: use the GCM-decrypted bytes; otherwise the SRTP payload is already plain Opus.
        let opus = match m.sframe.as_ref().map(|s| s.decrypt(&payload)) {
            Some(SframeIn::Decrypted(plain)) => plain,
            _ => payload,
        };
        let first = opus.first().copied().unwrap_or(0);
        if is_standard_opus_frame(first) {
            // Not portably decodable (libopus is FFI); hand it to the shell.
            self.outbox
                .push_back(Output::Event(CallEvent::ForeignAudio(Bytes::from(opus))));
            return;
        }
        // MLow decode (f32 [-1,1]) -> i16, appended to the playout buffer.
        for s in m.decoder.decode(&opus) {
            m.jitter
                .push_back((s * 32767.0).clamp(-32768.0, 32767.0) as i16);
        }
        // Bound the buffer on the feed side too: a burst of inbound packets arriving between two 20ms
        // playout ticks must not grow `jitter` without limit (drain_playout's cap only runs on a
        // tick). Drop oldest past the same ceiling the drain path uses.
        if m.jitter.len() > PLAYOUT_CAP {
            let drop_n = m.jitter.len() - PLAYOUT_CAP;
            m.jitter.drain(..drop_n);
        }
    }

    fn on_mic(&mut self, pcm: &[i16]) {
        let Some(m) = self.media.as_mut() else {
            return;
        };
        // Drop a wrong-length frame before any send: the encoder needs exactly one 60ms frame, and a
        // mis-sized buffer must not reach the DTX fast-path (which would emit an off-cadence packet).
        if pcm.len() != MIC_FRAME_SAMPLES {
            return;
        }
        // OS mic-mute delivers an exactly all-zero frame; genuine quiet speech carries LSB noise.
        // Don't gap the wire on mute: send a cheap cached DTX comfort-noise frame so the peer's
        // media-liveness timer stays fed (no codec CPU) and it doesn't re-negotiate the transport.
        if pcm.iter().all(|&s| s == 0) {
            let packet = m.pipe.protect_audio(&MLOW_DTX_CNG);
            self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
            return;
        }
        m.scratch.clear();
        m.scratch.extend(pcm.iter().map(|&s| s as f32 / 32768.0));
        // A transient encode failure drops just this frame; the next one resyncs.
        let Ok(coded) = m.encoder.encode(&m.scratch) else {
            return;
        };
        // No SFrame on send by design: the encoded frame goes plain into WAHKDF SRTP, which the peer
        // accepts. `enable_sframe` is recv-decrypt-only (see CallConfig). This matches the
        // pre-refactor send path; send-side SFrame is intentionally not wired.
        let packet = m.pipe.protect_audio(&coded);
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
    }
}

/// Advance a periodic deadline past `now`. Normally one interval; if the shell fell far behind
/// (more than one interval late) resync to `now + interval` so we emit one tick, not a backlog.
fn next_tick(deadline: Millis, now: Millis, interval: Millis) -> Millis {
    let stepped = deadline + interval;
    if stepped <= now {
        now + interval
    } else {
        stepped
    }
}

/// Drain one 20ms playout slice. Caps the buffer at the latency ceiling, then while priming holds
/// silence WITHOUT draining until the cushion reaches `PLAYOUT_TARGET`; once primed it takes up to
/// `PLAYOUT_DRAIN` samples and re-arms priming on an underrun, so a late packet costs one clean
/// re-prime rather than a silence pad every tick. Priming also gives up after `MAX_PRIME_TICKS` if
/// the buffer holds some audio but never reaches the target (the peer sent one frame then went DTX),
/// flushing it instead of stalling silent forever.
fn drain_playout(
    jitter: &mut VecDeque<i16>,
    priming: &mut bool,
    priming_ticks: &mut u32,
) -> Vec<i16> {
    if jitter.len() > PLAYOUT_CAP {
        let drop_n = jitter.len() - PLAYOUT_CAP;
        jitter.drain(..drop_n);
    }
    if *priming {
        let reached_target = jitter.len() >= PLAYOUT_TARGET;
        // Bounded wait: a partial buffer that never reaches the target (peer DTX after one frame) is
        // flushed rather than held silent forever / replayed stale when a much later packet arrives.
        let timed_out = *priming_ticks >= MAX_PRIME_TICKS && !jitter.is_empty();
        if reached_target || timed_out {
            *priming = false;
            *priming_ticks = 0;
        } else {
            // Age the timeout only while a partial buffer is actually waiting to fill. An empty
            // buffer (call start, or a DTX gap) doesn't count, so the first real frame still gets the
            // full prebuffer cushion instead of flushing instantly on a counter left high by silence.
            *priming_ticks = if jitter.is_empty() {
                0
            } else {
                *priming_ticks + 1
            };
            return vec![0; PLAYOUT_DRAIN];
        }
    }
    let take = jitter.len().min(PLAYOUT_DRAIN);
    let mut frame: Vec<i16> = jitter.drain(..take).collect();
    if frame.len() < PLAYOUT_DRAIN {
        *priming = true;
        *priming_ticks = 0;
        frame.resize(PLAYOUT_DRAIN, 0);
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::mlow::MlowEncoder;
    use crate::voip::warp::WARP_MI_TAG_LEN;

    const SELF_LID: &str = "111111111111111:0@lid";
    const PEER_LID: &str = "222222222222222:0@lid";
    const SSRC: u32 = 0x5741_0001;
    const SAMPLES: u32 = 960;

    fn config(enable_media: bool) -> CallConfig {
        CallConfig {
            call_id: "CID".into(),
            direction: CallDirection::Incoming,
            self_lid: SELF_LID.into(),
            peer_lid: PEER_LID.into(),
            call_key: (0u8..32).collect(),
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            relay_token: vec![0xAB; 16],
            relay_ip: "203.0.113.7".into(),
            relay_port: 3478,
            integrity_key: b"relay-key".to_vec(),
            warp_mi_tag_len: 4,
            enable_media,
            enable_sframe: false,
        }
    }

    // The SRTP callKey and the STUN integrity key must never reach a `{:?}` dump, matching the
    // redaction on the sibling key structs. Pins the manual Debug against a `#[derive(Debug)]` regression.
    #[test]
    fn call_config_debug_redacts_key_material() {
        let dbg = format!("{:?}", config(true));
        assert!(
            dbg.contains("call_key: \"[redacted]\""),
            "callKey not redacted"
        );
        assert!(
            dbg.contains("integrity_key: \"[redacted]\""),
            "integrity_key not redacted"
        );
        assert!(
            dbg.contains("relay_token: \"[redacted]\""),
            "relay_token not redacted"
        );
        // The 0..32 callKey bytes, the b"relay-key" integrity key, and the 0xAB relay-token bytes
        // must not appear.
        assert!(!dbg.contains("[0, 1, 2, 3"), "callKey bytes leaked");
        assert!(!dbg.contains("114, 101, 108"), "integrity_key bytes leaked");
        assert!(!dbg.contains("[171, 171"), "relay_token bytes leaked");
        // Non-secret fields stay visible for diagnostics.
        assert!(dbg.contains("call_id: \"CID\""));
    }

    fn engine(enable_media: bool) -> CallEngine {
        CallEngine::new(config(enable_media), Box::new(SequentialTxIds::new())).unwrap()
    }

    // CallConfig::for_incoming pulls the relay endpoint/token/integrity-key out of a parsed RelayData
    // and derives our participant SSRC, so the media-config build is offline testable end to end.
    #[test]
    fn for_incoming_builds_config_from_relay() {
        use crate::voip::relay_parse::{RelayAddress, RelayData, RelayEndpoint};
        let relay = RelayData {
            relay_key_ascii: Some(b"relay-key".to_vec()),
            warp_mi_tag_len: Some(4),
            relay_tokens: vec![vec![0xAB; 16]],
            endpoints: vec![RelayEndpoint {
                relay_id: 1,
                relay_name: "gru1c02".into(),
                token_id: 0,
                auth_token_id: 1,
                addresses: vec![RelayAddress {
                    protocol: 0,
                    ipv4: Some("203.0.113.7".into()),
                    ipv6: None,
                    port: 3478,
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let cfg = CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &relay)
            .expect("config builds from a complete relay");
        assert_eq!(cfg.relay_ip, "203.0.113.7");
        assert_eq!(cfg.relay_port, 3478);
        assert_eq!(cfg.relay_token, vec![0xAB; 16]);
        assert_eq!(cfg.integrity_key, b"relay-key");
        assert_eq!(cfg.direction, CallDirection::Incoming);
        assert!(cfg.enable_media && cfg.enable_sframe);
        // SSRC is the deterministic E2E derivation over our self LID.
        assert_eq!(
            cfg.ssrc,
            crate::voip::ssrc::derive_wasm_participant_ssrc(
                "CID",
                &crate::voip::ssrc::format_e2e_srtp_participant_id(SELF_LID),
                0
            )
        );
        // A relay with no <key> is rejected (no STUN integrity key to sign the allocate).
        let mut no_key = relay.clone();
        no_key.relay_key_ascii = None;
        assert!(matches!(
            CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &no_key),
            Err(SetupError::NoIntegrityKey)
        ));
        // No endpoints -> NoRelayEndpoint.
        let mut no_ep = relay.clone();
        no_ep.endpoints.clear();
        assert!(matches!(
            CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &no_ep),
            Err(SetupError::NoRelayEndpoint)
        ));
        // A padded-empty token slot (sparse token block) is a missing token, not a zero-length one.
        let mut empty_token = relay.clone();
        empty_token.relay_tokens = vec![Vec::new()];
        assert!(matches!(
            CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &empty_token),
            Err(SetupError::NoRelayToken(0))
        ));
    }

    // for_outgoing mirrors for_incoming (same relay parse + SSRC derivation) but sets Outgoing and
    // takes the locally-generated callKey.
    #[test]
    fn for_outgoing_builds_config_from_relay() {
        use crate::voip::relay_parse::{RelayAddress, RelayData, RelayEndpoint};
        let relay = RelayData {
            relay_key_ascii: Some(b"relay-key".to_vec()),
            warp_mi_tag_len: Some(4),
            relay_tokens: vec![vec![0xAB; 16]],
            endpoints: vec![RelayEndpoint {
                relay_id: 1,
                relay_name: "gru1c02".into(),
                token_id: 0,
                auth_token_id: 1,
                addresses: vec![RelayAddress {
                    protocol: 0,
                    ipv4: Some("203.0.113.7".into()),
                    ipv6: None,
                    port: 3478,
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let cfg = CallConfig::for_outgoing("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &relay)
            .expect("config builds from a complete relay");
        assert_eq!(cfg.direction, CallDirection::Outgoing);
        assert_eq!(cfg.relay_ip, "203.0.113.7");
        assert_eq!(cfg.relay_port, 3478);
        assert_eq!(cfg.relay_token, vec![0xAB; 16]);
        assert_eq!(cfg.integrity_key, b"relay-key");
        assert_eq!(cfg.call_key, (0u8..32).collect::<Vec<u8>>());
        assert!(cfg.enable_media && cfg.enable_sframe);
        assert_eq!(
            cfg.ssrc,
            crate::voip::ssrc::derive_wasm_participant_ssrc(
                "CID",
                &crate::voip::ssrc::format_e2e_srtp_participant_id(SELF_LID),
                0
            )
        );
        // The same relay-completeness errors apply.
        let mut no_key = relay.clone();
        no_key.relay_key_ascii = None;
        assert!(matches!(
            CallConfig::for_outgoing("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &no_key),
            Err(SetupError::NoIntegrityKey)
        ));
    }

    /// Drain the engine fully, returning every intent up to (and excluding) the terminal Timeout,
    /// plus that deadline.
    fn drain(eng: &mut CallEngine) -> (Vec<Output>, Millis) {
        let mut out = Vec::new();
        loop {
            match eng.poll_output() {
                Output::Timeout(t) => return (out, t),
                other => out.push(other),
            }
        }
    }

    /// Build an Allocate-error STUN packet carrying an ERROR-CODE attr for `code` (class*100+num).
    fn allocate_error(code: u16) -> Vec<u8> {
        let class = (code / 100) as u8;
        let number = (code % 100) as u8;
        // Raw ERROR-CODE (0x0009) TLV: type, len=4, value (2 reserved bytes, class, number).
        let err_attr = [0x00, 0x09, 0x00, 0x04, 0x00, 0x00, class, number];
        stun::encode_stun_request(stun::MSG_ALLOCATE_ERROR, &[3u8; 12], &err_attr, None, false)
    }

    fn count_transmits(outs: &[Output]) -> usize {
        outs.iter()
            .filter(|o| matches!(o, Output::Transmit(_)))
            .count()
    }

    fn feed_frame(b: &mut VecDeque<i16>) {
        // One 60ms peer frame of nonzero samples, so a real slice is distinguishable from a pad.
        b.extend((0..960i32).map(|i| (i % 200) as i16 - 99));
    }

    #[test]
    fn playout_primes_to_target_before_audio() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        // One frame is below PLAYOUT_TARGET (two frames): playout holds silence without draining.
        feed_frame(&mut buf);
        assert!(
            drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                .iter()
                .all(|&s| s == 0),
            "below the prebuffer target playout primes with silence"
        );
        assert_eq!(buf.len(), 960, "priming must not consume the buffer");
        // The second frame reaches the target; playout now produces real audio.
        feed_frame(&mut buf);
        assert!(
            drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                .iter()
                .any(|&s| s != 0),
            "at the prebuffer target playout starts real audio"
        );
    }

    #[test]
    fn playout_prebuffer_absorbs_inter_arrival_jitter() {
        // Packets (one 60ms peer frame) arrive at a jittered cadence around every 3rd 20ms tick, with
        // gaps up to 4 ticks that stay within the 60ms cushion. The primed buffer must emit no
        // mid-stream silence; the old floor-riding drain (no prebuffer) underruns on the same schedule.
        let arrivals = [0usize, 3, 7, 9, 12, 16, 18, 21, 25, 27, 30];
        let ticks = 34;
        let feed = |buf: &mut VecDeque<i16>, t: usize| {
            if arrivals.contains(&t) {
                feed_frame(buf);
            }
        };
        let midstream_silence = |frames: &[bool]| -> usize {
            match (
                frames.iter().position(|&r| r),
                frames.iter().rposition(|&r| r),
            ) {
                (Some(a), Some(b)) => (a..=b).filter(|&t| !frames[t]).count(),
                _ => 0,
            }
        };

        // The pre-fix drain: drains the floor immediately and silence-pads underruns.
        fn floor_drain(jitter: &mut VecDeque<i16>) -> Vec<i16> {
            let take = jitter.len().min(PLAYOUT_DRAIN);
            let mut f: Vec<i16> = jitter.drain(..take).collect();
            f.resize(PLAYOUT_DRAIN, 0);
            f
        }
        let mut old_buf = VecDeque::new();
        let old_real: Vec<bool> = (0..ticks)
            .map(|t| {
                feed(&mut old_buf, t);
                floor_drain(&mut old_buf).iter().any(|&s| s != 0)
            })
            .collect();
        assert!(
            midstream_silence(&old_real) > 0,
            "schedule must stress the buffer: the floor-riding drain should underrun"
        );

        let mut buf = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        let real: Vec<bool> = (0..ticks)
            .map(|t| {
                feed(&mut buf, t);
                drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                    .iter()
                    .any(|&s| s != 0)
            })
            .collect();
        assert_eq!(
            midstream_silence(&real),
            0,
            "prebuffer must absorb the inter-arrival jitter with no mid-stream silence"
        );
    }

    #[test]
    fn bad_endpoint_rejected() {
        let mut cfg = config(true);
        cfg.relay_ip = "not-an-ip".into();
        assert!(matches!(
            CallEngine::new(cfg, Box::new(SequentialTxIds::new())),
            Err(EngineError::BadEndpoint)
        ));
    }

    #[test]
    fn short_call_key_rejected() {
        let mut cfg = config(true);
        cfg.call_key = vec![0u8; 16];
        assert!(matches!(
            CallEngine::new(cfg, Box::new(SequentialTxIds::new())),
            Err(EngineError::BadCallKey)
        ));
    }

    #[test]
    fn start_emits_allocate_and_arms_playout_first() {
        let mut eng = engine(true);
        assert_eq!(eng.poll_timeout(), None);
        eng.start(0);
        let (outs, deadline) = drain(&mut eng);
        // The initial allocate is the only transmit; playout (20ms) is the nearer deadline.
        assert_eq!(count_transmits(&outs), 1);
        assert!(matches!(outs[0], Output::Transmit(_)));
        assert_eq!(deadline, PLAYOUT_MS);
        assert_eq!(eng.poll_timeout(), Some(PLAYOUT_MS));
    }

    #[test]
    fn control_plane_only_arms_keepalive_no_playout() {
        // esp32-style: no media. The only deadline is the 1s keepalive, and mic frames are ignored.
        let mut eng = engine(false);
        eng.start(0);
        let (outs, deadline) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 1); // allocate
        assert_eq!(deadline, KEEPALIVE_MS);
        // A mic frame produces nothing without a media plane.
        eng.handle_input(5, Input::MicFrame(&[1234i16; SAMPLES as usize]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 0);
    }

    #[test]
    fn keepalive_fires_allocate_and_ping() {
        let mut eng = engine(false);
        eng.start(0);
        let _ = drain(&mut eng);
        eng.handle_input(KEEPALIVE_MS, Input::Timeout);
        let (outs, deadline) = drain(&mut eng);
        // allocate + ping.
        assert_eq!(count_transmits(&outs), 2);
        assert_eq!(deadline, 2 * KEEPALIVE_MS);
    }

    #[test]
    fn playout_emits_silence_every_tick() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        eng.handle_input(PLAYOUT_MS, Input::Timeout);
        let (outs, deadline) = drain(&mut eng);
        match outs.as_slice() {
            [Output::Playout(frame)] => {
                assert_eq!(frame.len(), PLAYOUT_DRAIN);
                assert!(frame.iter().all(|&s| s == 0), "no audio fed yet -> silence");
            }
            other => panic!("expected one Playout, got {other:?}"),
        }
        assert_eq!(deadline, 2 * PLAYOUT_MS);
    }

    #[test]
    fn binding_request_gets_binding_success() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let req =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[7u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&req));
        let (outs, _) = drain(&mut eng);
        let transmits: Vec<&Output> = outs
            .iter()
            .filter(|o| matches!(o, Output::Transmit(_)))
            .collect();
        assert_eq!(transmits.len(), 1, "exactly one binding-success reply");
        if let Output::Transmit(b) = transmits[0] {
            assert_eq!(stun::stun_message_type(b), Some(stun::MSG_BINDING_SUCCESS));
        }
    }

    #[test]
    fn allocate_success_emits_event_once() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let ok =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&ok));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocated)))
                .count(),
            1
        );
        assert!(eng.is_allocated());
        // A second success does not re-emit.
        eng.handle_input(2, Input::RelayPacket(&ok));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(_)))
                .count(),
            0
        );
    }

    #[test]
    fn mic_drops_wrong_length_frames() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        // A wrong-length all-zero buffer must not reach the DTX fast-path: it is dropped, not sent.
        eng.handle_input(1, Input::MicFrame(&[0i16; 480]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a short muted frame must be dropped"
        );
        eng.handle_input(2, Input::MicFrame(&[]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 0, "an empty frame must be dropped");
        // A correctly-sized all-zero frame still emits one DTX packet.
        eng.handle_input(3, Input::MicFrame(&[0i16; SAMPLES as usize]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            1,
            "a 960-sample muted frame transmits DTX"
        );
    }

    #[test]
    fn mic_mute_emits_dtx_keepalive_not_a_gap() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        // OS mute delivers exact-zero frames; each must still transmit a DTX comfort-noise frame so
        // the peer's media-liveness timer stays fed (a gap makes the peer re-negotiate the transport).
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        for k in 1..=5u64 {
            eng.handle_input(k, Input::MicFrame(&[0i16; SAMPLES as usize]));
            let (outs, _) = drain(&mut eng);
            assert_eq!(
                count_transmits(&outs),
                1,
                "muted tick {k} must transmit DTX, not skip"
            );
            let pkt = outs
                .iter()
                .find_map(|o| match o {
                    Output::Transmit(b) => Some(b.clone()),
                    _ => None,
                })
                .expect("a transmit");
            let (_, payload) = peer
                .unprotect_audio(&pkt)
                .expect("muted DTX packet must decrypt");
            assert_eq!(payload.len(), 1, "DTX is one byte");
            assert_eq!(
                payload[0], 0x90,
                "muted frame payload is the mlow DTX token"
            );
        }
        // A real tone still encodes + protects to one RTP transmit.
        let tone: Vec<i16> = (0..SAMPLES as usize)
            .map(|i| (8000.0 * (i as f32 * 0.1).sin()) as i16)
            .collect();
        eng.handle_input(6, Input::MicFrame(&tone));
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 1);
    }

    #[test]
    fn inbound_rtp_decodes_into_playout() {
        // A mirrored peer (its self LID is our peer LID) encrypts real MLow tone frames; the engine
        // must SRTP-decrypt, MLow-decode, and drain them to the speaker as non-silent audio. Two
        // frames are sent so the playout prebuffer reaches PLAYOUT_TARGET and starts draining.
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);

        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut peer_enc = MlowEncoder::new();
        for n in 0..2u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let packet = peer_tx.protect_audio(&frame);
            eng.handle_input(1, Input::RelayPacket(&packet));
        }
        // Drain enough playout ticks to pass priming and pull the decoded ~1920 samples (320/tick).
        let mut peak = 0i16;
        for k in 1..=8 {
            eng.handle_input(k * PLAYOUT_MS, Input::Timeout);
            let (outs, _) = drain(&mut eng);
            for o in outs {
                if let Output::Playout(frame) = o {
                    peak = peak.max(frame.iter().map(|s| s.abs()).max().unwrap_or(0));
                }
            }
        }
        assert!(peak > 0, "decoded peer audio must reach the playout buffer");
    }

    // End-to-end of the device-mismatch fix: an engine built for the dialed base callee LID receives
    // garbage from the device that actually answered (a companion `:2`), until `rekey_recv` re-keys
    // the recv path to that device — after which its audio decodes and reaches playout.
    #[test]
    fn rekey_recv_switches_inbound_to_answering_device() {
        let mut eng = engine(true); // recv keyed to PEER_LID = "222...:0@lid" (the dialed base)
        eng.start(0);
        let _ = drain(&mut eng);

        let call_key: Vec<u8> = (0u8..32).collect();
        let answering = "222222222222222:2@lid"; // a companion, NOT the dialed base device
        let mut answerer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: answering,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut enc = MlowEncoder::new();
        let tone = |n: u32| -> Vec<f32> {
            (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
                .collect()
        };

        // Before rekey: recv keyed to the base, so the companion's frames don't decode (garbage).
        for n in 0..2u32 {
            let packet = answerer_tx.protect_audio(&enc.encode(&tone(n)).unwrap());
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }

        assert!(eng.rekey_recv(answering));

        // After rekey: the companion's frames decode to real audio that reaches playout.
        for n in 2..4u32 {
            let packet = answerer_tx.protect_audio(&enc.encode(&tone(n)).unwrap());
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        let mut peak = 0i16;
        for k in 1..=8 {
            eng.handle_input(k * PLAYOUT_MS, Input::Timeout);
            let (outs, _) = drain(&mut eng);
            for o in outs {
                if let Output::Playout(frame) = o {
                    peak = peak.max(frame.iter().map(|s| s.abs()).max().unwrap_or(0));
                }
            }
        }
        assert!(
            peak > 0,
            "after rekey the answering device's audio must reach playout"
        );
    }

    #[test]
    fn merged_deadline_is_the_nearer_of_keepalive_and_playout() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        // Playout (20) is nearer than keepalive (1000) right after start.
        assert_eq!(eng.poll_timeout(), Some(PLAYOUT_MS));
    }

    // A burst of inbound frames arriving between two playout ticks must not grow the jitter buffer
    // without bound: on_rtp caps it at the same ceiling drain_playout uses. Regression for the
    // feed-side unbounded-growth path (no Timeout is interleaved, so the drain-time cap never runs).
    #[test]
    fn inbound_burst_keeps_jitter_bounded() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut peer_enc = MlowEncoder::new();
        for n in 0..200u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.05).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let packet = peer_tx.protect_audio(&frame);
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        assert!(
            eng.jitter_len() <= PLAYOUT_CAP,
            "feed-side jitter must stay <= PLAYOUT_CAP, got {}",
            eng.jitter_len()
        );
    }

    // A decrypted payload whose first byte marks standard Opus/CELT ((b & 0xC0) == 0xC0) is surfaced
    // as CallEvent::ForeignAudio for the shell to decode, NOT pushed into the MLow playout buffer.
    #[test]
    fn standard_opus_payload_routes_to_foreign_audio() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let packet = peer_tx.protect_audio(&[0xC8u8, 1, 2, 3, 4, 5]); // 0xC8 & 0xC0 == 0xC0
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::ForeignAudio(_))))
                .count(),
            1,
            "standard-Opus payload must surface exactly one ForeignAudio event"
        );
        assert_eq!(
            eng.jitter_len(),
            0,
            "foreign audio must not enter the MLow playout buffer"
        );
    }

    // The inbound SFrame decrypt branch end-to-end: a mirrored peer GCM-wraps an MLow frame (its
    // encrypt key == our decrypt key), SRTP-protects it, and the engine must SRTP-decrypt, SFrame-
    // decrypt, MLow-decode, and play it. All other engine tests run enable_sframe = false.
    #[test]
    fn sframe_wrapped_inbound_decrypts_and_plays() {
        let mut cfg = config(true);
        cfg.enable_sframe = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0);
        let _ = drain(&mut eng);
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        // Mirror: peer's self = our peer, peer's peer = our self -> peer.encrypt key == our decrypt key.
        let mut peer_sframe = SframeSession::new(&call_key, PEER_LID, SELF_LID).unwrap();
        let mut peer_enc = MlowEncoder::new();
        for n in 0..2u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let wrapped = peer_sframe.encrypt(&frame);
            let packet = peer_tx.protect_audio(&wrapped);
            eng.handle_input(1, Input::RelayPacket(&packet));
        }
        let mut peak = 0i16;
        for k in 1..=8 {
            eng.handle_input(k * PLAYOUT_MS, Input::Timeout);
            let (outs, _) = drain(&mut eng);
            for o in outs {
                if let Output::Playout(frame) = o {
                    peak = peak.max(frame.iter().map(|s| s.abs()).max().unwrap_or(0));
                }
            }
        }
        assert!(
            peak > 0,
            "SFrame-wrapped peer audio must decrypt, MLow-decode, and reach playout"
        );
    }

    // At t = 1000 the 1s keepalive and the 20ms playout deadlines coincide; one on_timeout must fire
    // BOTH: the keepalive transmits (allocate + ping) and exactly one playout frame.
    #[test]
    fn coincident_keepalive_and_playout_both_fire() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        eng.handle_input(KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 2, "keepalive allocate + ping");
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Playout(_)))
                .count(),
            1,
            "exactly one playout frame on the coincident tick"
        );
    }

    // The MLow encoder requires exactly 960 samples; a wrong-length mic frame is dropped (no RTP,
    // no panic), not partially sent. Pins the samples_per_packet contract (see CallConfig doc).
    #[test]
    fn wrong_length_mic_frame_is_dropped() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let short: Vec<i16> = (0..480i32).map(|i| (i % 50) as i16 + 1).collect();
        eng.handle_input(1, Input::MicFrame(&short));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a non-960 mic frame must be dropped"
        );
    }

    // A Timeout fired before any deadline (the shell woke early) emits nothing and leaves the next
    // deadline unchanged -- no spurious keepalive/playout, no deadline drift, no busy-spin.
    #[test]
    fn early_timeout_is_a_noop() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        assert_eq!(eng.poll_timeout(), Some(PLAYOUT_MS));
        eng.handle_input(5, Input::Timeout); // before the 20ms playout deadline
        let (outs, deadline) = drain(&mut eng);
        assert!(outs.is_empty(), "early timeout must emit nothing");
        assert_eq!(deadline, PLAYOUT_MS, "deadline must be unchanged");
    }

    // Characterizes playout under HARSH inter-arrival jitter (gaps beyond the cushion, which the
    // prebuffer test deliberately avoids). It pins two invariants the re-prime path must keep: buffer
    // occupancy (latency) never exceeds PLAYOUT_CAP, and the stream recovers to real audio once
    // arrivals stabilize. (The ~120ms re-prime pause per underrun is a known tuning cost of the
    // 2-frame prebuffer target -- a separate, deliberate audio trade-off, not asserted here.)
    #[test]
    fn playout_under_harsh_jitter_stays_bounded_and_recovers() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        let mut max_occupancy = 0usize;
        // Phase 1: sparse arrivals with gaps up to ~6 ticks (well beyond the cushion) -> underruns.
        let arrivals = [0usize, 6, 13, 20, 27];
        for t in 0..32 {
            if arrivals.contains(&t) {
                feed_frame(&mut buf);
            }
            let _ = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
            max_occupancy = max_occupancy.max(buf.len());
        }
        assert!(
            max_occupancy <= PLAYOUT_CAP,
            "latency must stay bounded by the cap; peaked at {max_occupancy}"
        );
        // Phase 2: steady arrivals every 3rd tick -> playout must recover to real (non-silent) audio.
        let mut recovered = false;
        for t in 0..30 {
            if t % 3 == 0 {
                feed_frame(&mut buf);
            }
            if drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                .iter()
                .any(|&s| s != 0)
            {
                recovered = true;
            }
        }
        assert!(
            recovered,
            "playout must recover to real audio once arrivals stabilize"
        );
    }

    // Bounded re-prime flush: after priming re-arms, if the peer sends a single 60ms frame and then
    // goes DTX, the buffer stalls below PLAYOUT_TARGET. Playout must flush that frame after
    // MAX_PRIME_TICKS instead of holding it (silent) forever or replaying it stale much later.
    #[test]
    fn priming_flushes_partial_buffer_after_bounded_wait() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        feed_frame(&mut buf); // one 60ms frame (960) < PLAYOUT_TARGET (1920), then nothing (DTX)
        // Up to MAX_PRIME_TICKS the partial buffer is held: silence, no drain.
        for _ in 0..MAX_PRIME_TICKS {
            let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
            assert!(f.iter().all(|&s| s == 0), "still priming -> silence");
            assert_eq!(
                buf.len(),
                960,
                "the partial frame is held while priming, not drained"
            );
        }
        // The next tick hits the bound and flushes the held frame as real audio.
        let flushed = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
        assert!(
            flushed.iter().any(|&s| s != 0),
            "the partial buffer must flush to real audio after the bounded wait"
        );
        assert!(
            buf.len() < 960,
            "the held frame was drained, not stalled forever"
        );
    }

    // The priming timeout must NOT age during initial silence / a DTX gap (empty buffer), or the
    // first frame after a long silence would flush instantly with no cushion. After 2*MAX ticks of
    // empty-buffer priming, one frame must still wait for the cushion instead of flushing.
    #[test]
    fn priming_timeout_does_not_age_on_an_empty_buffer() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        for _ in 0..(MAX_PRIME_TICKS * 2) {
            let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
            assert!(f.iter().all(|&s| s == 0), "empty buffer -> silence");
        }
        // First frame arrives: must NOT flush instantly -- the counter didn't age while empty.
        feed_frame(&mut buf);
        let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
        assert!(
            f.iter().all(|&s| s == 0),
            "one frame is below the target -> still priming, no instant flush"
        );
        assert_eq!(buf.len(), 960, "the first frame is held for the cushion");
        // The second frame reaches the target -> real audio drains.
        feed_frame(&mut buf);
        let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
        assert!(
            f.iter().any(|&s| s != 0),
            "at the target playout starts real audio"
        );
    }

    // An inbound Allocate-error must surface exactly one terminal RelayAllocateFailed carrying the
    // STUN error code, and not mark the call allocated.
    #[test]
    fn allocate_error_emits_failed_event_with_code() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let err = allocate_error(486); // class 4, number 86
        eng.handle_input(1, Input::RelayPacket(&err));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateFailed(486))))
                .count(),
            1,
            "one RelayAllocateFailed carrying the error code"
        );
        assert!(!eng.is_allocated(), "a rejected allocate is not allocated");
    }

    // An allocate-error is terminal: the engine goes inert, so a subsequent Timeout far past the
    // keepalive deadline produces ZERO further transmits (the keepalive stopped, not a dead-relay
    // keepalive forever) and poll_timeout reports no timer.
    #[test]
    fn malformed_stun_success_does_not_cancel_the_allocate_timeout() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        // A success-typed packet without the STUN magic cookie must not mark us allocated or cancel
        // the allocate timeout, else a garbage packet keeps a wedged relay in indefinite keepalive.
        let mut garbage =
            stun::encode_stun_request(stun::MSG_BINDING_SUCCESS, &[3u8; 12], &[], None, false);
        garbage[4] ^= 0xff; // corrupt the magic cookie
        eng.handle_input(1, Input::RelayPacket(&garbage));
        let (outs, _) = drain(&mut eng);
        assert!(
            !eng.is_allocated(),
            "a malformed success must not mark allocated"
        );
        assert!(
            !outs
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::RelayAllocated))),
            "a malformed success must not emit RelayAllocated"
        );
        // The allocate timeout safety net is intact.
        eng.handle_input(ALLOCATE_TIMEOUT_MS + 1, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert!(
            outs.iter()
                .any(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "the allocate timeout must still fire after a malformed success"
        );
    }

    #[test]
    fn garbage_stun_does_not_terminate_the_call() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        // Dropping the ERROR-CODE TLV leaves the body-length header still claiming the full body, so
        // the packet is rejected as INCOMPLETE (fails is_complete_stun), not as a parseable error. A
        // garbage relay packet must not be treated as a terminal failure that hangs up the call.
        let full = allocate_error(486);
        let garbage = &full[..full.len() - 8]; // drop the ERROR-CODE TLV, keep the message type
        eng.handle_input(1, Input::RelayPacket(garbage));
        let (outs, _) = drain(&mut eng);
        assert!(
            !eng.is_terminated(),
            "garbage STUN must not terminate the call"
        );
        assert!(
            !outs
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::RelayAllocateFailed(_)))),
            "garbage STUN must not emit RelayAllocateFailed"
        );
    }

    #[test]
    fn allocate_error_terminates_and_stops_keepalive() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        eng.handle_input(1, Input::RelayPacket(&allocate_error(486)));
        let _ = drain(&mut eng);
        assert!(eng.is_terminated(), "an allocate-error is terminal");
        assert_eq!(eng.poll_timeout(), None, "no timer once terminated");
        // Far past every deadline: the keepalive must not fire.
        eng.handle_input(100 * KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a terminated engine must emit no further transmits"
        );
    }

    // Same for the allocate-timeout path: once it fires the engine is terminal, so a later Timeout
    // emits no keepalive.
    #[test]
    fn allocate_timeout_terminates_and_stops_keepalive() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        eng.handle_input(ALLOCATE_TIMEOUT_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            1,
            "the terminal timeout event is delivered before going inert"
        );
        assert!(eng.is_terminated(), "the allocate-timeout is terminal");
        assert_eq!(eng.poll_timeout(), None, "no timer once terminated");
        eng.handle_input(ALLOCATE_TIMEOUT_MS + 100 * KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a terminated engine must emit no further transmits"
        );
    }

    // With no allocate ack, driving Timeouts past ALLOCATE_TIMEOUT_MS must emit exactly ONE
    // RelayAllocateTimedOut and none after (the deadline fires once, then is cleared).
    #[test]
    fn allocate_timeout_fires_exactly_once() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        eng.handle_input(ALLOCATE_TIMEOUT_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            1,
            "one terminal timeout at the deadline"
        );
        // Drive well past the deadline again: no second timeout event.
        eng.handle_input(ALLOCATE_TIMEOUT_MS + 5 * KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            0,
            "the timeout must not re-fire"
        );
    }

    // A successful allocate before the deadline emits RelayAllocated and stops the timer, so driving
    // Timeouts past ALLOCATE_TIMEOUT_MS yields no RelayAllocateTimedOut.
    #[test]
    fn allocate_success_cancels_the_timeout() {
        let mut eng = engine(true);
        eng.start(0);
        let _ = drain(&mut eng);
        let ok =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&ok));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocated)))
                .count(),
            1
        );
        assert!(eng.is_allocated());
        // Past the deadline: no timeout, the success already stopped the timer.
        eng.handle_input(ALLOCATE_TIMEOUT_MS + KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            0,
            "a successful allocate must cancel the timeout"
        );
    }
}
