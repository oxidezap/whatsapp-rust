//! VoIP example for testing the call stack.
//!
//! Audio is bridged to the system through `cpal` (cross-platform; ALSA on Linux,
//! CoreAudio on macOS, WASAPI on Windows). `cpal` is a dev-dependency, so it is
//! linked only for this example and never reaches consumers of the library.
//!
//! Subcommands:
//!   loopback         Mic → Opus → E2E-SRTP protect → unprotect → Opus → speaker.
//!                    Exercises the whole media stack locally; NO WhatsApp connection.
//!                    Run it and you should hear yourself, processed by the voip pipeline.
//!   listen [accept]  Connect to WhatsApp, print incoming calls; reject (default) or accept.
//!   call <jid>       Connect, discover the peer's devices, encrypt the callKey per device,
//!                    and send a `<call><offer>`; logs the signaling flow via raw nodes.
//!
//!   cargo run --example voip --features "voip sqlite-storage tokio-transport ureq-client tokio-native" -- loopback
//!
//! The inbound MEDIA path is the library facade: `client.voip().accept(&call).audio(mic,
//! speaker).start()` returns a `CallHandle` and the library owns the callKey decrypt, the relay
//! socket, the sans-IO engine, and the task lifetime. This example only supplies the cpal audio
//! device and reacts to engine events.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, anyhow};
use log::{error, info, warn};
use portable_atomic::AtomicU64;
use wacore::stanza::call::{self as stanza, CAPABILITY_OFFER};
use wacore::types::call::{CallAction, IncomingCall};
use wacore::types::events::{Event, EventHandler};
use wacore::voip::CallEvent;
use whatsapp_rust::prelude::*;
use whatsapp_rust::voip::CallHandle;
use whatsapp_rust::voip::audio::{WaOpusDecoder, WaOpusEncoder};
use whatsapp_rust::voip::session::{MediaPipeline, MediaPipelineParams};

const FRAME_SAMPLES: usize = 960; // 60 ms @ 16 kHz
const WA_RATE: u32 = 16_000;
const SSRC: u32 = 0x5741_0001;

#[tokio::main]
async fn main() -> Result<()> {
    let pid = std::process::id();
    // webrtc-sctp/-dtls log the DTLS Close Notify at the normal end of a call ("failed to read
    // packets on net_conn: Alert is Fatal or Close Notify"), which is benign teardown noise: the
    // call already ended via <terminate> and the disconnect is surfaced through CallEvent. Quiet
    // those crates to error so the demo output stays clean; RUST_LOG still overrides this.
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,webrtc_sctp=error,webrtc_dtls=error"),
    )
    .format(move |buf, record| {
        use std::io::Write;
        writeln!(
            buf,
            "{} pid={pid} [{:<5}] {}",
            wacore::time::now_utc().format("%M:%S%.3f"),
            record.level(),
            record.args()
        )
    })
    .init();
    info!("🦀 voip run pid={pid}");

    // Defensive Windows audio-timing hygiene: ask for a 1 ms system timer so the mic drain's sleep poll
    // is fine-grained rather than the default ~15.6 ms quantum (some environments still hand out the
    // coarse timer). Standard practice for low-latency audio on Windows (Chrome/Firefox/Skype); a no-op
    // everywhere else.
    #[cfg(target_os = "windows")]
    {
        #[link(name = "winmm")]
        unsafe extern "system" {
            fn timeBeginPeriod(period: u32) -> u32;
        }
        unsafe {
            timeBeginPeriod(1);
        }
    }

    // The realtime media path encodes one mlow frame every 60 ms; a debug build makes that encode
    // ~7-10x slower (measured ~19 ms avg / ~49 ms peak vs ~2.5 ms release), and on a slower machine it
    // overruns the 60 ms budget so frames back up and the far end breaks up. Run calls in release.
    #[cfg(debug_assertions)]
    warn!(
        "⚠ debug build: mlow encode is much slower than release and may not keep up with realtime \
         audio (choppy/inaudible far-end voice). Run VoIP with `cargo run --release` for clean audio."
    );

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("loopback") => run_loopback().await,
        Some("listen") => {
            run_bot(Mode::Listen {
                accept: args.get(2).map(String::as_str) == Some("accept"),
            })
            .await
        }
        Some("call") => {
            let jid = args
                .get(2)
                .context("usage: voip call <jid>")?
                .parse::<Jid>()
                .map_err(|e| anyhow!("bad jid: {e}"))?;
            run_bot(Mode::Call(jid)).await
        }
        _ => {
            eprintln!("usage: voip <loopback | listen [accept] | call <jid>>");
            Ok(())
        }
    }
}

// ===================== cpal audio bridge =====================
//
// The engine speaks 16 kHz mono i16 in 960-sample (60 ms) frames; the OS audio device speaks its own
// native rate/channels/format. Two cpal streams bridge the two:
//
//   mic  : device input  -> downmix to mono -> decimate native_rate -> 16 kHz -> chunk to 960 frames
//   spkr : 16 kHz frames -> ring -> interpolate 16 kHz -> native_rate -> device format -> device out
//
// cpal's data callbacks run on a realtime audio thread and must not allocate. They use pre-sized
// reusable buffers (the mic accumulator, the speaker ring). The only per-call alloc is the 960-sample
// Vec handed to async_channel (one per 60 ms), which the trait API requires. cpal's `Stream` is
// `!Send` on some hosts, so each stream is owned by a dedicated std thread that parks until the call's
// channel is dropped (teardown), then drops the stream to stop the device.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer as _, Producer as _, Split as _};

/// Linear resample `src` (at `src_rate`) into the 16 kHz `out`, carrying a fractional read cursor
/// across calls so block boundaries don't click. Allocation-free: writes into the caller's `out`.
fn resample_into(src: &[i16], src_rate: u32, dst_rate: u32, pos: &mut f64, out: &mut Vec<i16>) {
    if src.is_empty() {
        return;
    }
    let step = src_rate as f64 / dst_rate as f64;
    // `pos` is the fractional index into a virtual stream; it started somewhere in the previous
    // block's tail, so rebase it into this block.
    let mut p = *pos;
    while p < src.len() as f64 {
        let i = p as usize;
        let frac = p - i as f64;
        let a = src[i] as f64;
        let b = if i + 1 < src.len() {
            src[i + 1] as f64
        } else {
            a
        };
        out.push((a + (b - a) * frac).round() as i16);
        p += step;
    }
    // Carry the leftover fraction (relative to the next block's start) so the next call continues
    // smoothly instead of restarting at 0.
    *pos = p - src.len() as f64;
}

/// Pick the device's default config and an i16-friendly stream config.
fn default_config(
    device: &cpal::Device,
    input: bool,
) -> Result<(cpal::StreamConfig, SampleFormat)> {
    let supported = if input {
        device.default_input_config()
    } else {
        device.default_output_config()
    }
    .map_err(|e| anyhow!("default config: {e}"))?;
    let format = supported.sample_format();
    Ok((supported.into(), format))
}

/// Open the default input device and stream 960-sample 16 kHz mono i16 frames over a channel.
///
/// Mirrors the speaker path: the realtime cpal callback only downmixes to mono and PUSHES into a
/// lock-free SPSC ring -- it never touches the async runtime, because a cross-thread send inside a
/// WASAPI input callback can silently wedge capture (cpal #970). An async drain task pops the ring,
/// decimates the native rate to 16 kHz, chunks into 960-sample frames, and feeds the channel. The
/// stream lives on a parked std thread that exits when the receiver is dropped (call ended).
fn spawn_mic() -> Result<async_channel::Receiver<Vec<i16>>> {
    let host = cpal::default_host();
    // Honor `WA_INPUT_DEVICE` (a name substring) to override a wrong/silent default endpoint, else use
    // the OS default. We only enumerate `input_devices()` when a name is requested: enumerating on
    // Linux/ALSA probes the legacy OSS plugin and prints harmless "Cannot open /dev/dsp" noise to
    // stderr (from libasound, not our logger), so the default path -- `default_input_device()`, which
    // does not enumerate -- avoids it entirely. When a name IS requested, a no-match error lists the
    // available inputs so the right substring can be found.
    let want = std::env::var("WA_INPUT_DEVICE")
        .ok()
        .filter(|s| !s.is_empty());
    let device = match &want {
        Some(want) => {
            let devices: Vec<cpal::Device> = host
                .input_devices()
                .map(|i| i.collect())
                .unwrap_or_default();
            let names: Vec<String> = devices.iter().filter_map(|d| d.name().ok()).collect();
            devices
                .into_iter()
                .find(|d| d.name().is_ok_and(|n| n.contains(want)))
                .ok_or_else(|| {
                    anyhow!(
                        "no input device name contains WA_INPUT_DEVICE={want:?}; available inputs: {names:?}"
                    )
                })?
        }
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?,
    };
    let (config, format) = default_config(&device, true)?;
    let src_rate = config.sample_rate.0;
    let channels = config.channels as usize;
    info!(
        "🎤 cpal input: {} @ {src_rate} Hz, {channels} ch, {format:?}",
        device.name().unwrap_or_else(|_| "?".into())
    );

    let (tx, rx) = async_channel::bounded::<Vec<i16>>(8);

    // ~1 s of native-rate mono headroom: absorbs the callback's bursty deliveries so the drain (and
    // the channel) see a steady 60 ms cadence.
    let ring = HeapRb::<i16>::new(src_rate as usize);
    let (prod, mut cons) = ring.split();

    // Peak amplitude of the captured audio, for the one-shot silence check below.
    let peak = Arc::new(AtomicI32::new(0));
    // Cleared by the stream thread on ANY exit (build/play failure or teardown), so the drain stops
    // and the channel closes -- a stream that never starts must not leave the consumer waiting for
    // frames that will never come.
    let alive = Arc::new(AtomicBool::new(true));
    // Frames produced vs dropped because the downstream channel was full -- i.e. the engine (and behind
    // it the relay send) couldn't keep up, so captured audio was lost before transmit. `WA_AUDIO_DIAG=1`
    // logs the rate, surfacing send-side backpressure (the SEND counterpart to the playout underruns).
    let made = Arc::new(AtomicU64::new(0));
    let dropped = Arc::new(AtomicU64::new(0));
    let tx_drain = tx.clone();

    if std::env::var_os("WA_AUDIO_DIAG").is_some() {
        let (made, dropped, alive) = (made.clone(), dropped.clone(), alive.clone());
        tokio::spawn(async move {
            let (mut lm, mut ld) = (0u64, 0u64);
            while alive.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let (m, d) = (
                    made.load(Ordering::Relaxed),
                    dropped.load(Ordering::Relaxed),
                );
                let (dm, dd) = (m - lm, d - ld);
                lm = m;
                ld = d;
                // Only surface real send backpressure; steady state (0 dropped) is silent.
                if dd > 0 {
                    warn!(
                        "🎤 mic->engine: {dd}/{dm} frames dropped (send backpressure) in the last 3s"
                    );
                }
            }
        });
    }

    // Async drain: ring -> resample(native -> 16 kHz) -> 960-frame chunks -> channel.
    {
        let (peak, alive, made, dropped) =
            (peak.clone(), alive.clone(), made.clone(), dropped.clone());
        tokio::spawn(async move {
            let mut pop_buf = vec![0i16; (src_rate as usize / 4).max(FRAME_SAMPLES)];
            let mut acc: Vec<i16> = Vec::with_capacity(FRAME_SAMPLES * 2);
            let mut pos = 0.0f64;
            loop {
                if !alive.load(Ordering::Relaxed) || tx_drain.is_closed() {
                    break;
                }
                let n = cons.pop_slice(&mut pop_buf);
                if n == 0 {
                    // Ring momentarily empty: yield briefly. The poll interval bounds the mic->engine
                    // forwarding latency/jitter, so keep it short (with the 1ms Windows timer above this
                    // sleep is accurate; on Linux it always was). 5ms keeps frame delivery near the
                    // audio clock without busy-spinning.
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    continue;
                }
                resample_into(&pop_buf[..n], src_rate, WA_RATE, &mut pos, &mut acc);
                while acc.len() >= FRAME_SAMPLES {
                    let rest = acc.split_off(FRAME_SAMPLES);
                    let frame = std::mem::replace(&mut acc, rest);
                    let p = frame.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
                    peak.fetch_max(p, Ordering::Relaxed);
                    made.fetch_add(1, Ordering::Relaxed);
                    // Drop on full: loss tolerant. Steady state keeps the channel near-empty.
                    if tx_drain.try_send(frame).is_err() {
                        dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
    }

    // One-shot silence check: if a few seconds in the callback is firing but every captured sample is
    // zero, the OS is handing us silence -- typically a Windows mic-privacy block on a desktop app, or
    // the wrong input endpoint. Warn ONCE (not a recurring diagnostic) with what to try.
    {
        let (peak, alive) = (peak.clone(), alive.clone());
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            if alive.load(Ordering::Relaxed) && peak.load(Ordering::Relaxed) == 0 {
                warn!(
                    "🎤 microphone is capturing only silence. On Windows, allow desktop apps to use \
                     the mic (Settings > Privacy & security > Microphone), or select another device \
                     with WA_INPUT_DEVICE=<name substring>."
                );
            }
        });
    }

    // Teardown probe: once the call drops the receiver the channel closes, so the stream thread can
    // wake and free the input device instead of holding it until process exit.
    let teardown = tx.clone();
    let alive_thread = alive;
    // Build on a dedicated thread: the !Send stream must be created, played, and dropped there.
    std::thread::spawn(move || {
        match build_input_stream(&device, &config, format, channels, prod) {
            Ok(stream) => {
                if let Err(e) = stream.play() {
                    error!("mic stream play failed: {e}");
                } else {
                    // Hold the stream until the call drops its receiver (or the call ends), polling in
                    // short intervals (park alone never wakes on a channel close), then fall out of
                    // scope so the stream drops and frees the device.
                    while !teardown.is_closed() && alive_thread.load(Ordering::Relaxed) {
                        std::thread::park_timeout(std::time::Duration::from_millis(250));
                    }
                }
            }
            Err(e) => error!("mic stream build failed: {e}"),
        }
        // Signal the drain to stop (single exit, any reason) so the channel closes and the
        // consumer observes the mic ending instead of blocking forever on frames that never arrive.
        alive_thread.store(false, Ordering::Relaxed);
    });
    Ok(rx)
}

fn build_input_stream<P>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    channels: usize,
    mut prod: P,
) -> Result<cpal::Stream>
where
    P: ringbuf::traits::Producer<Item = i16> + Send + 'static,
{
    macro_rules! build {
        ($t:ty) => {{
            // Reusable mono scratch; reallocates only past the warmup size. The realtime callback does
            // NO async work and NO logging -- only a downmix and a lock-free push into the ring.
            let mut mono: Vec<i16> = Vec::with_capacity(2048);
            let err = |e| error!("mic stream error: {e}");
            device
                .build_input_stream(
                    config,
                    move |data: &[$t], _| {
                        mono.clear();
                        for frame in data.chunks(channels) {
                            // Downmix: average the channels into one i16.
                            let mut sum = 0i32;
                            for &s in frame {
                                sum += i16::from_sample(s) as i32;
                            }
                            mono.push((sum / channels as i32) as i16);
                        }
                        // Drop overflow: loss tolerant; the ring is full only if the drain fell behind.
                        let _ = prod.push_slice(&mono);
                    },
                    err,
                    None,
                )
                .map_err(|e| anyhow!("build input stream: {e}"))
        }};
    }
    match format {
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        other => Err(anyhow!("unsupported input sample format {other:?}")),
    }
}

/// Open the default output device and play 16 kHz mono i16 frames pushed onto the returned channel.
///
/// An async drain task moves frames from the channel into a lock-free SPSC ring (resampling 16 kHz ->
/// native rate as it goes, allocation-free). cpal's realtime output callback pops mono i16 from the
/// ring and writes the device format, duplicating mono across channels. Underrun = silence. The
/// stream lives on a parked std thread that exits when the sender is dropped (call ended).
fn spawn_speaker() -> Result<async_channel::Sender<Vec<i16>>> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no default output device"))?;
    let (config, format) = default_config(&device, false)?;
    let dst_rate = config.sample_rate.0;
    let channels = config.channels as usize;
    info!(
        "🔈 cpal output: {} @ {dst_rate} Hz, {channels} ch, {format:?}",
        device.name().unwrap_or_else(|_| "?".into())
    );

    // ~1 s of native-rate mono headroom: rides out DTX gaps and scheduling jitter.
    let ring = HeapRb::<i16>::new(dst_rate as usize);
    let (mut prod, cons) = ring.split();

    // Windows/WASAPI only: pre-roll ~100 ms of silence so the ring is never empty at the strict ~10 ms
    // WASAPI pull period. cpal opens the WASAPI shared-mode output with the minimal buffer
    // (hnsBufferDuration=0), and our drain refills in 60 ms bursts, so without headroom any Windows
    // scheduler jitter (default timer ~15.6 ms) drains the ring and the callback emits silence ->
    // choppy. ALSA/PipeWire already buffer ~100 ms, so this is gated to Windows to add ZERO latency on
    // Linux (where playout is already smooth). The cost is a one-time ~100 ms of output latency.
    #[cfg(target_os = "windows")]
    {
        let _ = prod.push_slice(&vec![0i16; dst_rate as usize / 10]);
    }

    let (tx, rx) = async_channel::bounded::<Vec<i16>>(64);
    // Set once the call drops the playout sender (the drain loop below ends), so the output thread
    // frees the device instead of holding it until process exit.
    let teardown = Arc::new(AtomicBool::new(false));
    let teardown_drain = teardown.clone();

    // Playout health counters: samples the WASAPI/ALSA output callback pulled, and how many of those
    // were underruns (ring empty -> silence -> choppy). `WA_AUDIO_DIAG=1` logs the rate periodically so
    // a choppy run shows whether the OUTPUT path is starving (the classic Windows/WASAPI failure) vs a
    // clean playout (then the gap is upstream: network/decode).
    let pops = Arc::new(AtomicU64::new(0));
    let underruns = Arc::new(AtomicU64::new(0));
    if std::env::var_os("WA_AUDIO_DIAG").is_some() {
        let (pops, underruns, teardown) = (pops.clone(), underruns.clone(), teardown.clone());
        tokio::spawn(async move {
            let (mut last_pops, mut last_under) = (0u64, 0u64);
            while !teardown.load(Ordering::Relaxed) {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let (p, u) = (
                    pops.load(Ordering::Relaxed),
                    underruns.load(Ordering::Relaxed),
                );
                let (dp, du) = (p - last_pops, u - last_under);
                last_pops = p;
                last_under = u;
                let pct = if dp > 0 {
                    du as f64 * 100.0 / dp as f64
                } else {
                    0.0
                };
                // Only surface a genuinely choppy interval; a clean playout (~0% underruns) is silent.
                if pct >= 2.0 {
                    info!(
                        "🔈 playout: {du}/{dp} samples were underruns ({pct:.1}%) in the last 3s (high % = choppy output)"
                    );
                }
            }
        });
    }

    // Async drain: channel -> resample -> ring. Owns the ring producer.
    tokio::spawn(async move {
        let mut resampled: Vec<i16> = Vec::with_capacity(4096);
        let mut pos = 0.0f64;
        while let Ok(frame) = rx.recv().await {
            resampled.clear();
            resample_into(&frame, WA_RATE, dst_rate, &mut pos, &mut resampled);
            // Drop overflow: the ring is full only if playout fell badly behind (loss tolerant).
            let _ = prod.push_slice(&resampled);
        }
        teardown_drain.store(true, Ordering::Relaxed);
    });

    // Build/play the !Send output stream on a dedicated parked thread.
    std::thread::spawn(move || {
        let stream =
            match build_output_stream(&device, &config, format, channels, cons, pops, underruns) {
                Ok(s) => s,
                Err(e) => {
                    error!("speaker stream build failed: {e}");
                    return;
                }
            };
        if let Err(e) = stream.play() {
            error!("speaker stream play failed: {e}");
            return;
        }
        // Hold the stream until the drain signals teardown, then fall out of scope so the stream
        // drops and frees the output device.
        while !teardown.load(Ordering::Relaxed) {
            std::thread::park_timeout(std::time::Duration::from_millis(250));
        }
    });
    Ok(tx)
}

fn build_output_stream<C>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    channels: usize,
    mut cons: C,
    pops: Arc<AtomicU64>,
    underruns: Arc<AtomicU64>,
) -> Result<cpal::Stream>
where
    C: ringbuf::traits::Consumer<Item = i16> + Send + 'static,
{
    macro_rules! build {
        ($t:ty) => {{
            let err = |e| error!("speaker stream error: {e}");
            device
                .build_output_stream(
                    config,
                    move |data: &mut [$t], _| {
                        // Accumulate the underrun count locally (single-threaded RT callback) and
                        // publish once per callback, not per sample, to keep the hot path lock-free.
                        let mut under = 0u64;
                        for frame in data.chunks_mut(channels) {
                            // Pop one mono sample (silence on underrun) and fan it across channels.
                            let s: i16 = match cons.try_pop() {
                                Some(s) => s,
                                None => {
                                    under += 1;
                                    0
                                }
                            };
                            let v = <$t>::from_sample(s);
                            for out in frame.iter_mut() {
                                *out = v;
                            }
                        }
                        pops.fetch_add((data.len() / channels) as u64, Ordering::Relaxed);
                        if under > 0 {
                            underruns.fetch_add(under, Ordering::Relaxed);
                        }
                    },
                    err,
                    None,
                )
                .map_err(|e| anyhow!("build output stream: {e}"))
        }};
    }
    match format {
        SampleFormat::I8 => build!(i8),
        SampleFormat::I16 => build!(i16),
        SampleFormat::I32 => build!(i32),
        SampleFormat::U8 => build!(u8),
        SampleFormat::U16 => build!(u16),
        SampleFormat::U32 => build!(u32),
        SampleFormat::F32 => build!(f32),
        SampleFormat::F64 => build!(f64),
        other => Err(anyhow!("unsupported output sample format {other:?}")),
    }
}

// ===================== loopback (no WhatsApp) =====================

async fn run_loopback() -> Result<()> {
    info!(
        "voip loopback: mic → Opus → E2E-SRTP protect/unprotect → Opus → speaker. Ctrl+C to stop."
    );
    let mic = spawn_mic()?;
    let speaker = spawn_speaker()?;
    let mut enc = WaOpusEncoder::new()?;
    let mut dec = WaOpusDecoder::new()?;

    // A throwaway callKey; same LID both ways so the loopback round-trips.
    let call_key: [u8; 32] = rand::random();
    let lid = "10000000000000:0@lid";
    let params = MediaPipelineParams {
        call_key: &call_key,
        self_lid: lid,
        peer_lid: lid,
        ssrc: SSRC,
        samples_per_packet: FRAME_SAMPLES as u32,
        warp_mi_tag_len: 4,
    };
    let mut send =
        MediaPipeline::new(&params).ok_or_else(|| anyhow!("callKey too short for E2E keys"))?;
    let mut recv =
        MediaPipeline::new(&params).ok_or_else(|| anyhow!("callKey too short for E2E keys"))?;

    let mut frames = 0u64;
    while let Ok(pcm) = mic.recv().await {
        let opus = enc.encode(&pcm)?;
        let packet = send.protect_audio(&opus);
        // The recv tracker derives the ROC per packet, so this stays correct past a 16-bit wrap.
        if let Some((_, opus_out)) = recv.unprotect_audio(&packet) {
            let out = dec.decode(&opus_out)?;
            let _ = speaker.send(out).await;
        }
        frames += 1;
        if frames.is_multiple_of(100) {
            info!(
                "{frames} frames piped through the voip stack ({}s)",
                frames * 60 / 1000
            );
        }
    }
    Ok(())
}

// ===================== live call / listen =====================

enum Mode {
    Listen { accept: bool },
    Call(Jid),
}

/// Drives calls off the typed `Event::IncomingCall` (no raw-node forwarding needed): on an offer it
/// answers signaling then hands the MEDIA plane to the library facade
/// (`client.voip().accept(..).audio(..).start()`), on a terminate it hangs the matching call up. The
/// facade owns the relay socket, the callKey decrypt, the engine, and the task lifetime; this only
/// supplies the PipeWire mic/speaker and remembers the `CallHandle` so a `<terminate>` can stop it.
struct CallObserver {
    client: Arc<Client>,
    accept: bool,
    /// Whether this run ever starts a media call (auto-accept inbound OR an outbound `call`), so a
    /// `<terminate>` racing media startup is worth recording. A pure-reject run starts no media, so it
    /// must NOT record (the set would grow unbounded with nothing to consume it).
    manages_media: bool,
    /// Per-call bookkeeping for the example's terminate-driven hangup. The client's own
    /// `CallRegistry` already tears every call down on disconnect; this is only so a `<terminate>`
    /// can stop a specific live call (and so a terminate that races media startup isn't lost).
    state: Arc<Mutex<CallState>>,
}

#[derive(Default)]
struct CallState {
    /// Live calls' handles by call-id.
    handles: HashMap<String, Arc<CallHandle>>,
    /// Call-ids that were terminated BEFORE their media handle finished starting, so a late
    /// `start_media()` hangs the call up instead of leaving an orphaned live call.
    terminated: HashSet<String>,
}

impl CallObserver {
    fn new(client: Arc<Client>, accept: bool, manages_media: bool) -> Self {
        Self {
            client,
            accept,
            manages_media,
            state: Arc::new(Mutex::new(CallState::default())),
        }
    }
}

/// Register a freshly-started `CallHandle` for terminate-driven hangup. Returns false (and the caller
/// should hang up) if a `<terminate>` for this call-id already arrived while media was starting. On
/// success, spawns the wait_ended cleanup that drops the map entry when the call ends on its own.
/// Shared by the inbound (accept) and outbound (call) paths.
fn register_handle(state: &Arc<Mutex<CallState>>, cid: String, handle: Arc<CallHandle>) -> bool {
    let registered = {
        let mut st = state.lock().unwrap();
        if st.terminated.remove(&cid) {
            false
        } else {
            st.handles.insert(cid.clone(), handle.clone());
            true
        }
    };
    if !registered {
        return false;
    }
    // Drop our map entry once the call ends on its own (no terminate).
    let state = state.clone();
    tokio::spawn(async move {
        handle.wait_ended().await;
        {
            let mut st = state.lock().unwrap();
            // Remove only if it is still OUR handle: a same-call-id replacement may now own this slot
            // (its own cleanup will remove it), so we must not delete the live call.
            if st
                .handles
                .get(&cid)
                .is_some_and(|h| Arc::ptr_eq(h, &handle))
            {
                st.handles.remove(&cid);
            }
        }
        info!("◾ call {cid} media ended");
    });
    true
}

impl EventHandler for CallObserver {
    fn handle_event(&self, event: Arc<Event>) {
        if let Event::IncomingCall(call) = &*event {
            match &call.action {
                CallAction::Offer { call_id, .. } => {
                    let client = self.client.clone();
                    let call = call.clone();
                    let accept = self.accept;
                    let state = self.state.clone();
                    let cid = call_id.clone();
                    tokio::spawn(async move {
                        if let Err(e) = respond_to_offer(&client, &call, accept).await {
                            error!("call signaling failed: {e}");
                            return;
                        }
                        if !accept {
                            return;
                        }
                        match start_media(&client, &call).await {
                            Ok(handle) => {
                                let handle = Arc::new(handle);
                                // A <terminate> may have arrived while media was starting; if so, hang
                                // up now instead of registering an orphaned live call.
                                if !register_handle(&state, cid.clone(), handle.clone()) {
                                    handle.hangup().await;
                                    info!("◾ call {cid} terminated during media startup");
                                }
                            }
                            Err(e) => warn!("inbound media failed: {e}"),
                        }
                    });
                }
                CallAction::Terminate { call_id, .. } => {
                    info!("◀ terminate for {call_id} — hanging up");
                    // If the handle is registered, hang it up. If not, the offer is still starting
                    // media: record the call-id so that path hangs up on completion (no orphan).
                    let handle = {
                        let mut st = self.state.lock().unwrap();
                        match st.handles.remove(call_id) {
                            Some(h) => Some(h),
                            // Only track a pre-registration terminate when this run starts media
                            // (inbound accept OR an outbound call); otherwise there is no call to
                            // orphan and the set would grow unbounded.
                            None => {
                                if self.manages_media {
                                    st.terminated.insert(call_id.clone());
                                }
                                None
                            }
                        }
                    };
                    if let Some(handle) = handle {
                        tokio::spawn(async move { handle.hangup().await });
                    }
                }
                _ => {}
            }
        } else if let Event::MissedCall(mc) = &*event {
            // An offer replayed from the offline queue on reconnect: a dead call, never ring/accept.
            info!(
                "☎ missed call {} from {} (offline-delivered)",
                mc.call_id, mc.from
            );
        }
    }
}

/// Drive the inbound MEDIA plane through the library facade: PipeWire mic in, PipeWire speaker out,
/// the engine/relay/decrypt all internal. Replaces the ~180-line hand-rolled `run_inbound_media`.
async fn start_media(client: &Arc<Client>, call: &IncomingCall) -> Result<CallHandle> {
    let mic = spawn_mic()?;
    let speaker = spawn_speaker()?;
    info!("🔌 connecting media via client.voip().accept(..)…");
    let handle = client
        .voip()
        .accept(call)
        .audio(mic, speaker.clone())
        .start()
        .await
        .map_err(|e| anyhow!("accept media: {e}"))?;
    info!(
        "🎙  media flow live for call {} — speak into the mic.",
        handle.call_id()
    );
    spawn_call_event_listener(&handle, speaker);
    Ok(handle)
}

/// Place an outgoing 1:1 call through the library facade: PipeWire mic/speaker, the device discovery,
/// callKey encrypt, offer send, ack-driven relay connect, engine, and task lifetime all internal. The
/// returned handle is dormant until the server hands back the relay (live); the facade attaches the
/// engine then. Mirrors `start_media` for the outbound direction.
async fn place_outgoing_call(client: &Arc<Client>, peer: &Jid) -> Result<CallHandle> {
    let mic = spawn_mic()?;
    let speaker = spawn_speaker()?;
    info!("📞 placing call to {peer} via client.voip().call(..)…");
    let handle = client
        .voip()
        .call(peer)
        .audio(mic, speaker.clone())
        .start()
        .await
        .map_err(|e| anyhow!("place call: {e}"))?;
    info!(
        "☎  offer sent for call {} — waiting for the peer's relay to connect media.",
        handle.call_id()
    );
    spawn_call_event_listener(&handle, speaker);
    Ok(handle)
}

/// Surface the call's engine events: log relay-allocate outcomes, and decode any non-MLow (standard
/// Opus) frame the core hands back and play it. The real peer is MLow-only, so ForeignAudio is a
/// rare/diagnostic path; MLow audio is decoded inside the engine and arrives as playout on the
/// speaker channel directly.
fn spawn_call_event_listener(handle: &CallHandle, speaker: async_channel::Sender<Vec<i16>>) {
    let events = handle.events();
    tokio::spawn(async move {
        let mut opus = WaOpusDecoder::new().ok();
        while let Ok(ev) = events.recv().await {
            match ev {
                CallEvent::RelayAllocated => {
                    info!("🛰  relay allocate acknowledged — media path live")
                }
                CallEvent::RelayAllocateFailed(code) => {
                    warn!("relay rejected the allocate (STUN error {code}) — media never came up")
                }
                CallEvent::RelayAllocateTimedOut => {
                    warn!("relay never acked the allocate (wedged relay) — media never came up")
                }
                CallEvent::ForeignAudio(payload) => {
                    if let Some(dec) = opus.as_mut()
                        && let Ok(pcm) = dec.decode(&payload)
                    {
                        let _ = speaker.try_send(pcm);
                    }
                }
                // CallEvent is #[non_exhaustive]: ignore variants newer core versions may add.
                _ => {}
            }
        }
    });
}
async fn respond_to_offer(client: &Arc<Client>, call: &IncomingCall, accept: bool) -> Result<()> {
    let CallAction::Offer {
        call_id,
        call_creator,
        ..
    } = &call.action
    else {
        return Ok(());
    };
    info!(
        "📞 incoming call {call_id} from {} ({})",
        call.from,
        if accept { "accepting" } else { "rejecting" }
    );
    if !accept {
        client
            .voip()
            .reject(call)
            .await
            .map_err(|e| anyhow!("send reject: {e}"))?;
        return Ok(());
    }
    // Callee flow: preaccept immediately, then accept (signaling). Relay/media follow once
    // the live transport orchestration lands.
    // Codec-steering experiment: VOIP_FORCE_RFC_8K=1 advertises only 8 kHz, trying to push the
    // caller off Meta's 16 kHz mlow onto plain RFC Opus NB (which our stock libopus can decode).
    // Default keeps 16 kHz (mlow, garbled inbound until a real mlow decoder lands).
    let force_rfc_8k = std::env::var_os("VOIP_FORCE_RFC_8K").is_some();
    let audio_rates: &[&str] = if force_rfc_8k { &["8000"] } else { &["16000"] };
    if force_rfc_8k {
        info!("🧪 VOIP_FORCE_RFC_8K set — advertising only <audio rate=8000> to dodge mlow");
    }
    let pre_id = hex::encode(rand::random::<[u8; 8]>());
    client
        .send_node(stanza::build_preaccept(
            call_id,
            &call.from,
            call_creator,
            &pre_id,
            audio_rates,
        ))
        .await
        .map_err(|e| anyhow!("send preaccept: {e}"))?;
    let accept_id = hex::encode(rand::random::<[u8; 8]>());
    let accept_node = stanza::build_accept(&stanza::AcceptParams {
        call_id,
        to: &call.from,
        id: &accept_id,
        call_creator,
        audio_rates,
        relay_te: None,
        rte: None,
        voip_settings: None,
        capability: Some(&CAPABILITY_OFFER),
    });
    client
        .send_node(accept_node)
        .await
        .map_err(|e| anyhow!("send accept: {e}"))?;
    Ok(())
}

async fn run_bot(mode: Mode) -> Result<()> {
    let store = SqliteStore::new("whatsapp.db")
        .await
        .map_err(|e| anyhow!("sqlite: {e}"))?;
    let (accept, target) = match mode {
        Mode::Listen { accept } => (accept, None),
        Mode::Call(jid) => (false, Some(jid)),
    };

    let builder = Bot::builder()
        .with_backend(store)
        .on_qr_code(|code, timeout| async move {
            info!("Scan this QR (valid {}s):\n{code}", timeout.as_secs());
        })
        .on_connected(|_client| async {
            info!("✅ connected");
        });

    let bot = builder
        .build()
        .await
        .map_err(|e| anyhow!("build bot: {e}"))?;
    let client = bot.client();
    // The structured `Event::IncomingCall` (which now carries the offer's <enc>/<relay>) drives the
    // accept flow, so the raw-node-forwarding crutch the old hand-rolled inbound path needed is gone.
    let observer = Arc::new(CallObserver::new(
        client.clone(),
        accept,
        accept || target.is_some(),
    ));
    client.register_handler(observer.clone());

    if let Some(peer) = target {
        let client2 = client.clone();
        // Share the observer's call state so a peer `<terminate>` for our outgoing call can hang it up
        // (the same bookkeeping the inbound path uses).
        let state = observer.state.clone();
        tokio::spawn(async move {
            // Wait until the socket is up before placing the call; if it never connects, don't dial.
            if let Err(e) = client2
                .wait_for_connected(std::time::Duration::from_secs(60))
                .await
            {
                warn!("not connected within 60s, skipping outgoing call: {e}");
                return;
            }
            match place_outgoing_call(&client2, &peer).await {
                Ok(handle) => {
                    let cid = handle.call_id().to_string();
                    let handle = Arc::new(handle);
                    if !register_handle(&state, cid.clone(), handle.clone()) {
                        handle.hangup().await;
                        info!("◾ call {cid} terminated during startup");
                    }
                }
                Err(e) => error!("call failed: {e}"),
            }
        });
    } else {
        warn!(
            "listening for calls ({}). Ctrl+C to exit.",
            if accept { "auto-accept" } else { "auto-reject" }
        );
    }

    tokio::select! {
        _ = bot.run() => {}
        _ = tokio::signal::ctrl_c() => { info!("shutting down"); }
    }
    Ok(())
}
