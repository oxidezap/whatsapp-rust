//! VoIP media-plane hot paths: the per-packet work a live 1:1 call pays 50x/second each way.
//! Two layers are benched so a regression can be localized: the raw primitives (MLow encode/decode,
//! E2E-SRTP protect/unprotect, the AES-CTR payload cipher, SFrame) and the full engine inputs that
//! compose them (`on_mic`, `on_rtp`). `divan::AllocProfiler` is wired as the global allocator so each
//! row reports allocation count + bytes alongside wall time -- the codec dominates CPU, but the
//! crypto/framing seam is where the avoidable per-packet allocations live.

use bytes::Bytes;
use divan::{Bencher, black_box};
use wacore::voip::{
    CallConfig, CallDirection, CallEngine, Input, MediaPipeline, MediaPipelineParams, MlowDecoder,
    MlowEncoder, Output,
};
// Internal crypto/framing primitives the bench drives directly. They are intentionally off the
// `voip` facade and `#[doc(hidden)]` (not part of the consumer API); the bench reaches them via
// their source-module paths.
use wacore::voip::e2e_srtp::{crypt_payload, derive_e2e_keys};
use wacore::voip::engine::SequentialTxIds;
use wacore::voip::sframe::SframeSession;

/// Allocation profiler: makes every bench row also report allocs/frees and bytes, which is the
/// whole point of the seam-level rows (the per-packet `Vec` count is the optimizable signal).
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

const SELF_LID: &str = "111111111111111:0@lid";
const PEER_LID: &str = "222222222222222:0@lid";
const SSRC: u32 = 0x5741_0001;
/// The MLow frame size: 60ms @ 16kHz. The encoder accepts nothing else.
const SAMPLES: usize = 960;
/// Distinct frames cycled through the codec benches. A *single repeated* frame hits a cheaper,
/// lower-allocation code path (fewer pulses/survivors) and under-reports the real per-frame cost, so
/// the codec benches stream several frames instead.
const STREAM: usize = 8;

fn call_key() -> Vec<u8> {
    (0u8..32).collect()
}

/// A 60ms voiced tone (the encoder's LTP path, the realistic steady-state case). `phase` shifts it so
/// a primed encoder sees a fresh frame rather than a repeat.
fn tone_f32(phase: usize) -> Vec<f32> {
    (0..SAMPLES)
        .map(|i| 0.3 * (((i + phase) as f32) * 0.07).sin())
        .collect()
}

/// A 60ms i16 mic frame (what `Input::MicFrame` carries), non-silent so the engine doesn't take the
/// silence-skip shortcut.
fn tone_i16() -> Vec<i16> {
    (0..SAMPLES)
        .map(|i| (8000.0 * (i as f32 * 0.1).sin()) as i16)
        .collect()
}

/// An encoder past its first frame, so timed encodes measure the steady-state (inter-frame state
/// warmed) path rather than the colder first-frame one.
fn primed_encoder() -> MlowEncoder {
    let mut enc = MlowEncoder::new();
    let _ = enc.encode(&tone_f32(0));
    enc
}

/// One realistic encoded MLow frame, for the crypto/framing benches that need a payload.
fn encoded_frame() -> Vec<u8> {
    primed_encoder().encode(&tone_f32(SAMPLES)).unwrap()
}

/// The local send/recv pipeline (self=ours, peer=theirs).
fn pipeline() -> MediaPipeline {
    MediaPipeline::new(&MediaPipelineParams {
        call_key: &call_key(),
        self_lid: SELF_LID,
        peer_lid: PEER_LID,
        ssrc: SSRC,
        samples_per_packet: SAMPLES as u32,
        warp_mi_tag_len: 4,
    })
    .unwrap()
}

/// The mirror peer's send pipeline: its self LID is our peer LID, so what it protects, our
/// `unprotect_audio` recovers (it encrypts under the key our recv side reads).
fn peer_pipeline() -> MediaPipeline {
    MediaPipeline::new(&MediaPipelineParams {
        call_key: &call_key(),
        self_lid: PEER_LID,
        peer_lid: SELF_LID,
        ssrc: SSRC,
        samples_per_packet: SAMPLES as u32,
        warp_mi_tag_len: 4,
    })
    .unwrap()
}

fn config_for(self_lid: &str, peer_lid: &str, ssrc: u32, direction: CallDirection) -> CallConfig {
    CallConfig {
        call_id: "BENCH".into(),
        direction,
        self_lid: self_lid.into(),
        peer_lid: peer_lid.into(),
        call_key: call_key(),
        ssrc,
        samples_per_packet: SAMPLES as u32,
        relay_token: vec![0xAB; 16],
        relay_ip: "203.0.113.7".into(),
        relay_port: 3478,
        integrity_key: b"relay-key".to_vec(),
        warp_mi_tag_len: 4,
        enable_media: true,
        enable_sframe: false,
    }
}

fn config() -> CallConfig {
    config_for(SELF_LID, PEER_LID, SSRC, CallDirection::Incoming)
}

/// A started engine with the initial allocate already drained, so a benched input measures only the
/// steady-state per-packet work.
fn started_engine() -> CallEngine {
    started_engine_from(config())
}

fn started_engine_from(cfg: CallConfig) -> CallEngine {
    let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
    eng.start(0);
    drain(&mut eng);
    eng
}

/// Drain every queued intent up to the terminal Timeout (what the shell does after each input),
/// observing each so the work that produced it isn't optimized away.
fn drain(eng: &mut CallEngine) {
    loop {
        match eng.poll_output() {
            Output::Timeout(_) => break,
            other => {
                black_box(other);
            }
        }
    }
}

// --- Codec: the CPU-dominant primitives ---

/// MLow encode over a varied stream -- the outbound CPU floor (runs ~16.7x/s while the mic is open).
#[divan::bench]
fn mlow_encode(bencher: Bencher) {
    bencher
        .with_inputs(|| {
            let frames: Vec<Vec<f32>> = (0..STREAM).map(|i| tone_f32(i * SAMPLES)).collect();
            (primed_encoder(), frames, 0usize)
        })
        .bench_refs(|(enc, frames, i)| {
            let f = &frames[*i % frames.len()];
            *i += 1;
            black_box(enc.encode(black_box(f.as_slice())).unwrap())
        });
}

/// MLow decode over a varied stream -- the inbound CPU floor (runs once per received audio packet).
#[divan::bench]
fn mlow_decode(bencher: Bencher) {
    let packets: Vec<Vec<u8>> = {
        let mut enc = primed_encoder();
        (0..STREAM)
            .map(|i| enc.encode(&tone_f32(i * SAMPLES)).unwrap())
            .collect()
    };
    bencher
        .with_inputs(|| {
            let mut dec = MlowDecoder::new();
            let _ = dec.decode(&packets[0]); // prime past the first-frame path
            (dec, packets.clone(), 0usize)
        })
        .bench_refs(|(dec, pkts, i)| {
            let p = &pkts[*i % pkts.len()];
            *i += 1;
            black_box(dec.decode(black_box(p.as_slice())))
        });
}

// --- Crypto + framing: the seam where avoidable per-packet allocations live ---

/// `protect_audio`: RTP header + AES-CTR encrypt + WARP MI tag. The outbound framing path; its alloc
/// count is the headline number for the seam-level optimization work.
#[divan::bench]
fn e2e_srtp_protect(bencher: Bencher) {
    let frame = encoded_frame();
    bencher
        .with_inputs(|| (pipeline(), frame.clone()))
        .bench_refs(|(pipe, f)| black_box(pipe.protect_audio(black_box(f.as_slice()))));
}

/// `unprotect_audio`: strip tag, parse header, AES-CTR decrypt. The inbound framing path.
#[divan::bench]
fn e2e_srtp_unprotect(bencher: Bencher) {
    let frame = encoded_frame();
    bencher
        .with_inputs(|| {
            let packet = peer_pipeline().protect_audio(&frame);
            (pipeline(), packet)
        })
        .bench_refs(|(rx, packet)| black_box(rx.unprotect_audio(black_box(packet.as_slice()))));
}

/// The bare AES-128-CTR payload cipher under `protect`/`unprotect`. Isolated so its allocation
/// (`payload.to_vec()`) is attributable separately from the header/tag framing around it.
#[divan::bench]
fn crypt_payload_one(bencher: Bencher) {
    let keys = derive_e2e_keys(&call_key(), SELF_LID).unwrap();
    let frame = encoded_frame();
    bencher
        .with_inputs(|| frame.clone())
        .bench_refs(|f| black_box(crypt_payload(black_box(&keys), SSRC, 1, 0, f.as_slice())));
}

/// SFrame recv decrypt (the live SFrame direction: inbound GCM-unwrap with a plaintext fallback).
#[divan::bench]
fn sframe_decrypt(bencher: Bencher) {
    let frame = {
        let mut peer = SframeSession::new(&call_key(), PEER_LID, SELF_LID).unwrap();
        peer.encrypt(&encoded_frame())
    };
    let rx = SframeSession::new(&call_key(), SELF_LID, PEER_LID).unwrap();
    bencher
        .with_inputs(|| frame.clone())
        .bench_refs(|f| black_box(rx.decrypt(black_box(f.as_slice()))));
}

// --- Full engine inputs: the seam-level cost the shell actually drives ---

/// `handle_input(MicFrame)` + drain: the whole outbound packet path (silence check, i16->f32,
/// MLow encode, SRTP protect, enqueue). One Transmit per non-silent frame.
#[divan::bench]
fn engine_outbound_frame(bencher: Bencher) {
    let tone = tone_i16();
    bencher.with_inputs(started_engine).bench_refs(|eng| {
        eng.handle_input(1, Input::MicFrame(black_box(&tone)));
        drain(eng);
    });
}

/// `handle_input(RelayPacket)` + drain: the whole inbound packet path (classify, SRTP unprotect,
/// SFrame fallthrough, MLow decode, jitter feed).
#[divan::bench]
fn engine_inbound_packet(bencher: Bencher) {
    bencher
        .with_inputs(|| {
            let packet = peer_pipeline().protect_audio(&encoded_frame());
            (started_engine(), packet)
        })
        .bench_refs(|(eng, packet)| {
            eng.handle_input(1, Input::RelayPacket(black_box(packet)));
            drain(eng);
        });
}

// --- End-to-end: two peers talking through a relay seam ---

const SSRC_B: u32 = 0x5741_0002;

/// Which peer a packet came from; the relay delivers it to the other one.
#[derive(Clone, Copy)]
enum Origin {
    A,
    B,
}

/// The relay hop both peers' media crosses. A trait so the simulation routes every packet through one
/// explicit seam -- like the real `RelayTransport` -- instead of handing engine A's output straight to
/// engine B, and so a lossy/jittery relay model could drop in here without touching the call loop.
trait SimRelay {
    fn carry(&mut self, from: Origin, pkt: Bytes);
    /// Take everything queued for A and for B, respectively.
    fn deliver(&mut self) -> (Vec<Bytes>, Vec<Bytes>);
}

/// Lossless, in-order, zero-latency relay: the floor-cost seam (no jitter/loss), so the bench isolates
/// the codec + crypto work of a call rather than buffer dynamics.
#[derive(Default)]
struct LosslessRelay {
    to_a: Vec<Bytes>,
    to_b: Vec<Bytes>,
}

impl SimRelay for LosslessRelay {
    fn carry(&mut self, from: Origin, pkt: Bytes) {
        match from {
            Origin::A => self.to_b.push(pkt),
            Origin::B => self.to_a.push(pkt),
        }
    }
    fn deliver(&mut self) -> (Vec<Bytes>, Vec<Bytes>) {
        (
            std::mem::take(&mut self.to_a),
            std::mem::take(&mut self.to_b),
        )
    }
}

/// Drain one engine, routing its `Transmit`s into the relay and observing every other intent
/// (`Playout`/`Event`) so the work that produced them isn't optimized away.
fn pump(eng: &mut CallEngine, origin: Origin, relay: &mut dyn SimRelay) {
    loop {
        match eng.poll_output() {
            Output::Timeout(_) => break,
            Output::Transmit(b) => relay.carry(origin, b),
            other => {
                black_box(other);
            }
        }
    }
}

/// One second of a live 1:1 call: 50 x 20ms ticks. Each peer captures a 60ms mic frame every third
/// tick (the MLow frame cadence) and runs a playout tick every tick; their `Transmit`s cross the relay
/// and arrive as `RelayPacket`s on the other side, which decrypt + decode + feed the jitter buffer.
/// This is the realistic full-duplex flow -- encode, protect, relay, unprotect, decode, play, both
/// ways -- so the profile shows what actually dominates a call (spoiler: the two encoders).
fn simulate_call_second(a: &mut CallEngine, b: &mut CallEngine, relay: &mut dyn SimRelay) {
    let mic_a = tone_i16();
    let mic_b: Vec<i16> = (0..SAMPLES)
        .map(|i| (6000.0 * (i as f32 * 0.13).cos()) as i16)
        .collect();
    for tick in 0..50u64 {
        let now = tick * 20;
        if tick.is_multiple_of(3) {
            a.handle_input(now, Input::MicFrame(black_box(&mic_a)));
            b.handle_input(now, Input::MicFrame(black_box(&mic_b)));
        }
        a.handle_input(now, Input::Timeout);
        b.handle_input(now, Input::Timeout);
        pump(a, Origin::A, relay);
        pump(b, Origin::B, relay);
        let (to_a, to_b) = relay.deliver();
        for pkt in to_a {
            a.handle_input(now, Input::RelayPacket(black_box(&pkt)));
        }
        for pkt in to_b {
            b.handle_input(now, Input::RelayPacket(black_box(&pkt)));
        }
        // Inbound packets feed the jitter buffer (no Transmit), but still drain Playout/Events.
        pump(a, Origin::A, relay);
        pump(b, Origin::B, relay);
    }
}

/// The headline real-life bench: two peers in a 1-second full-duplex call over the relay seam. The
/// aggregate time/allocs here are the per-call-second cost; comparing it against `mlow_encode` x ~34
/// (two peers x ~16.7 frames/s) shows the encoder is essentially the entire bill.
#[divan::bench]
fn call_second_two_peers(bencher: Bencher) {
    bencher
        .with_inputs(|| {
            let a = started_engine_from(config_for(
                SELF_LID,
                PEER_LID,
                SSRC,
                CallDirection::Outgoing,
            ));
            let b = started_engine_from(config_for(
                PEER_LID,
                SELF_LID,
                SSRC_B,
                CallDirection::Incoming,
            ));
            (a, b, LosslessRelay::default())
        })
        .bench_refs(|(a, b, relay)| simulate_call_second(a, b, relay));
}
