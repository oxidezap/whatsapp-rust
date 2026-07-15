//! VoIP example for testing the call stack.
//!
//! Audio is bridged to the system through `cpal` (cross-platform; ALSA on Linux,
//! CoreAudio on macOS, WASAPI on Windows). `cpal` is a dev-dependency, so it is
//! linked only for this example and never reaches consumers of the library.
//! VIDEO is bridged through `ffmpeg`/`ffplay` subprocesses (must be on PATH when
//! `--video` is used): ffmpeg encodes the webcam / a file / a test pattern to H.264
//! and ffplay renders the peer's stream — the library only transports encoded AUs.
//!
//! Subcommands (all accept a trailing `--video`):
//!   loopback [--video]  Mic → Opus → E2E-SRTP protect → unprotect → Opus → speaker.
//!                       Exercises the whole media stack locally; NO WhatsApp connection.
//!                       With --video: ffmpeg source → AU splitter → ffplay window instead.
//!   listen [accept] [--video]  Connect, print incoming calls; reject (default) or accept.
//!                       With --video an accepted call answers with video media too.
//!   call <jid> [--video]  Place a call; with --video it is a video call from the start.
//!
//!   cargo run -p whatsapp-rust-voip-cli --release -- loopback
//!
//! During a live call, single-key commands on stdin (terminal only): `v` toggles video
//! (upgrade / accept a pending peer request / downgrade), `q` hangs up.
//! Env: `WA_VIDEO_INPUT` = `testsrc` | file/URL | webcam device (default: OS webcam);
//! `WA_VIDEO_SINK` = `window` (default) | `file` | `none`; optional quality overrides:
//! `WA_VIDEO_SIZE`, `WA_VIDEO_FPS`, `WA_VIDEO_BITRATE_KBPS`, `WA_VIDEO_SINK_FPS`.
//!
//! The inbound MEDIA path is the library facade: `client.voip().accept(&call).audio(mic,
//! speaker).start()` returns a `CallHandle` and the library owns the callKey decrypt, the relay
//! socket, the sans-IO engine, and the task lifetime. This example only supplies the cpal audio
//! device / ffmpeg pipes and reacts to engine events.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use log::{debug, error, info, warn};
use portable_atomic::AtomicU64;
use wacore::stanza::call::{self as stanza, CAPABILITY_OFFER};
use wacore::types::call::{CallAction, IncomingCall};
use wacore::types::events::{Event, EventHandler};
use wacore::voip::CallEvent;
use whatsapp_rust::prelude::*;
use whatsapp_rust::voip::audio::{WaOpusDecoder, WaOpusEncoder};
use whatsapp_rust::voip::session::{MediaPipeline, MediaPipelineParams};
use whatsapp_rust::voip::{CallHandle, VideoState};

mod video;
use video::VideoOpts;

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
    let command = parse_cli(&args);
    if video_source_is_ignored(&command, std::env::var_os("WA_VIDEO_INPUT").is_some()) {
        warn!(
            "WA_VIDEO_INPUT is set, but --video is missing; outbound video is disabled. Keep \
             --video on the same shell command line."
        );
    }
    match command {
        CliCommand::Loopback { video: true } => {
            video::run_video_loopback(&VideoOpts::from_env()?).await
        }
        CliCommand::Loopback { video: false } => run_loopback().await,
        CliCommand::Listen { accept, video } => run_bot(Mode::Listen { accept, video }).await,
        CliCommand::Call { jid, video } => {
            let jid = jid.parse::<Jid>().map_err(|e| anyhow!("bad jid: {e}"))?;
            run_bot(Mode::Call { jid, video }).await
        }
        CliCommand::Usage => {
            eprintln!("usage: voip <loopback | listen [accept] | call <jid>> [--video]");
            Ok(())
        }
    }
}

/// A parsed CLI invocation. Kept separate from `Mode` (and pure) so the argument classification —
/// including the `--video`-implies-accept rule that bit a real test run — is unit-testable.
#[derive(Debug, PartialEq, Eq)]
enum CliCommand {
    Loopback { video: bool },
    Listen { accept: bool, video: bool },
    Call { jid: String, video: bool },
    Usage,
}

/// Classify `argv` (including `argv[0]`). `--video` may appear anywhere. On `listen`, `--video`
/// IMPLIES `accept`: there is no reason to request video while rejecting every call, so
/// `listen --video` means "accept video calls" rather than silently rejecting them (the footgun a
/// user hit: the phone showed "no answer" because the reject went out).
fn parse_cli(argv: &[String]) -> CliCommand {
    let video = argv.iter().any(|a| a == "--video");
    let pos: Vec<&str> = argv
        .iter()
        .skip(1)
        .map(String::as_str)
        .filter(|a| *a != "--video")
        .collect();
    match pos.first().copied() {
        Some("loopback") => CliCommand::Loopback { video },
        Some("listen") => CliCommand::Listen {
            accept: pos.get(1).copied() == Some("accept") || video,
            video,
        },
        Some("call") => match pos.get(1) {
            Some(jid) => CliCommand::Call {
                jid: (*jid).to_string(),
                video,
            },
            None => CliCommand::Usage,
        },
        _ => CliCommand::Usage,
    }
}

fn video_source_is_ignored(command: &CliCommand, video_input_is_set: bool) -> bool {
    video_input_is_set
        && matches!(
            command,
            CliCommand::Loopback { video: false }
                | CliCommand::Listen { video: false, .. }
                | CliCommand::Call { video: false, .. }
        )
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
use cpal::{I24, Sample, SampleFormat, U24};
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
            let names: Vec<String> = devices
                .iter()
                .filter_map(|d| d.description().ok().map(|x| x.name().to_string()))
                .collect();
            devices
                .into_iter()
                .find(|d| d.description().is_ok_and(|x| x.name().contains(want.as_str())))
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
    let src_rate = config.sample_rate;
    let channels = config.channels as usize;
    info!(
        "🎤 cpal input: {} @ {src_rate} Hz, {channels} ch, {format:?}",
        device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "?".into())
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
                    config.clone(),
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
        SampleFormat::I24 => build!(I24),
        SampleFormat::U24 => build!(U24),
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
    let dst_rate = config.sample_rate;
    let channels = config.channels as usize;
    info!(
        "🔈 cpal output: {} @ {dst_rate} Hz, {channels} ch, {format:?}",
        device
            .description()
            .map(|d| d.name().to_string())
            .unwrap_or_else(|_| "?".into())
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
                    config.clone(),
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
        SampleFormat::I24 => build!(I24),
        SampleFormat::U24 => build!(U24),
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
    Listen { accept: bool, video: bool },
    Call { jid: Jid, video: bool },
}

/// Drives calls off the typed `Event::IncomingCall` (no raw-node forwarding needed): on an offer it
/// answers signaling then hands the MEDIA plane to the library facade
/// (`client.voip().accept(..).audio(..).start()`). The facade owns the relay socket, callKey decrypt,
/// engine, and termination; this example supplies the mic/speaker and keeps handles for its UI.
struct CallObserver {
    client: Arc<Client>,
    accept: bool,
    /// `--video`: start/answer calls with video media and auto-accept peer upgrade requests.
    video: bool,
    /// Per-call UI bookkeeping. The client's `CallRegistry` owns media termination.
    state: Arc<Mutex<CallState>>,
}

#[derive(Default)]
struct CallState {
    /// Live calls' handles by call-id.
    handles: HashMap<String, Arc<CallHandle>>,
    /// Registration order; the last live entry is the stdin UI target.
    call_order: Vec<String>,
    /// The stdin `v` toggle's view of each call's video: pending peer request vs our video live.
    video_ui: HashMap<String, VideoUi>,
    starting: HashSet<String>,
    /// Prevents a late startup from resurrecting media after the peer ended the call.
    terminated_during_startup: HashSet<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum VideoUi {
    /// The peer asked for video (`UpgradeRequestV2`); `v` accepts it.
    PendingPeerRequest,
    /// Our video plane is up; `v` downgrades.
    Active,
}

impl CallObserver {
    fn new(client: Arc<Client>, accept: bool, video: bool) -> Self {
        Self {
            client,
            accept,
            video,
            state: Arc::new(Mutex::new(CallState::default())),
        }
    }
}

fn begin_call_startup(state: &Arc<Mutex<CallState>>, call_id: &str) {
    let mut st = state.lock().unwrap();
    st.starting.insert(call_id.to_string());
    st.terminated_during_startup.remove(call_id);
}

fn complete_call_startup(state: &Arc<Mutex<CallState>>, call_id: &str) -> bool {
    let mut st = state.lock().unwrap();
    st.starting.remove(call_id);
    st.terminated_during_startup.remove(call_id)
}

fn record_peer_terminate(state: &Arc<Mutex<CallState>>, call_id: &str) {
    let mut st = state.lock().unwrap();
    if st.starting.contains(call_id) {
        st.terminated_during_startup.insert(call_id.to_string());
    }
    st.handles.remove(call_id);
    st.video_ui.remove(call_id);
    st.call_order.retain(|id| id != call_id);
}

fn mark_call_most_recent(order: &mut Vec<String>, call_id: &str) {
    order.retain(|id| id != call_id);
    order.push(call_id.to_string());
}

/// Register a handle for UI control and remove it when the library-owned call ends.
async fn register_handle(state: &Arc<Mutex<CallState>>, cid: String, handle: Arc<CallHandle>) {
    if complete_call_startup(state, &cid) {
        // The library also sees the terminate, but this closes a handle created after that teardown.
        handle.hangup().await;
        info!("◾ discarded media startup for already-ended call {cid}");
        return;
    }
    {
        let mut st = state.lock().unwrap();
        st.handles.insert(cid.clone(), handle.clone());
        mark_call_most_recent(&mut st.call_order, &cid);
    }
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
                st.video_ui.remove(&cid);
                st.call_order.retain(|id| id != &cid);
            }
        }
        info!("◾ call {cid} media ended");
    });
}

impl EventHandler for CallObserver {
    fn handle_event(&self, event: Arc<Event>) {
        if let Event::IncomingCall(call) = &*event {
            match &call.action {
                CallAction::Offer {
                    call_id, is_video, ..
                } => {
                    let client = self.client.clone();
                    let call = call.clone();
                    let accept = self.accept;
                    // Only answer with video when we're allowed to AND the offer is actually a video
                    // call; advertising `<video>` on an audio offer would be wrong.
                    let video = self.video && *is_video;
                    let state = self.state.clone();
                    let cid = call_id.clone();
                    begin_call_startup(&state, &cid);
                    tokio::spawn(async move {
                        if let Err(e) = respond_to_offer(&client, &call, accept, video).await {
                            error!("call signaling failed: {e}");
                            complete_call_startup(&state, &cid);
                            return;
                        }
                        if !accept {
                            complete_call_startup(&state, &cid);
                            return;
                        }
                        match start_media(&client, &call, video, &state).await {
                            Ok(handle) => register_handle(&state, cid.clone(), handle).await,
                            Err(e) => {
                                complete_call_startup(&state, &cid);
                                warn!("inbound media failed: {e}");
                            }
                        }
                    });
                }
                CallAction::Terminate { call_id, .. } => {
                    info!("◀ peer ended call {call_id}");
                    record_peer_terminate(&self.state, call_id);
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

/// Drive the inbound MEDIA plane through the library facade: cpal mic in, cpal speaker out,
/// the engine/relay/decrypt all internal. With `video`, ffmpeg/ffplay ride along as the codec.
async fn start_media(
    client: &Arc<Client>,
    call: &IncomingCall,
    video: bool,
    state: &Arc<Mutex<CallState>>,
) -> Result<Arc<CallHandle>> {
    let mic = spawn_mic()?;
    let speaker = spawn_speaker()?;
    info!("🔌 connecting media via client.voip().accept(..)…");
    let voip = client.voip();
    let mut builder = voip.accept(call).audio(mic, speaker.clone());
    if video {
        let opts = VideoOpts::from_env()?;
        let cid = call.action.call_id();
        builder = builder.video(
            video::spawn_video_source(&opts).await?,
            video::spawn_video_sink(&opts, cid).await?,
        );
    }
    let handle = builder
        .start()
        .await
        .map_err(|e| anyhow!("accept media: {e}"))?;
    info!(
        "🎙  media flow live for call {} — speak into the mic.{}",
        handle.call_id(),
        if video { " 🎥 video on." } else { "" }
    );
    let handle = Arc::new(handle);
    if video {
        mark_video(state, handle.call_id(), Some(VideoUi::Active));
    }
    spawn_call_event_listener(handle.clone(), speaker, video, state.clone());
    Ok(handle)
}

/// Place an outgoing 1:1 call through the library facade: cpal mic/speaker, the device discovery,
/// callKey encrypt, offer send, ack-driven relay connect, engine, and task lifetime all internal. The
/// returned handle is dormant until the server hands back the relay (live); the facade attaches the
/// engine then. Mirrors `start_media` for the outbound direction.
async fn place_outgoing_call(
    client: &Arc<Client>,
    peer: &Jid,
    video: bool,
    state: &Arc<Mutex<CallState>>,
) -> Result<Arc<CallHandle>> {
    let mic = spawn_mic()?;
    let speaker = spawn_speaker()?;
    info!("📞 placing call to {peer} via client.voip().call(..)…");
    let voip = client.voip();
    let mut builder = voip.call(peer).audio(mic, speaker.clone());
    if video {
        let opts = VideoOpts::from_env()?;
        builder = builder.video(
            video::spawn_video_source(&opts).await?,
            video::spawn_video_sink(&opts, "outgoing").await?,
        );
    }
    let handle = builder
        .start()
        .await
        .map_err(|e| anyhow!("place call: {e}"))?;
    info!(
        "☎  offer sent for call {} — waiting for the peer's relay to connect media.{}",
        handle.call_id(),
        if video { " 🎥 video call." } else { "" }
    );
    let handle = Arc::new(handle);
    if video {
        mark_video(state, handle.call_id(), Some(VideoUi::Active));
    }
    spawn_call_event_listener(handle.clone(), speaker, video, state.clone());
    Ok(handle)
}

/// Update (or clear) the stdin toggle's view of a call's video state.
fn mark_video(state: &Arc<Mutex<CallState>>, call_id: &str, ui: Option<VideoUi>) {
    let mut st = state.lock().unwrap();
    match ui {
        Some(ui) => {
            st.video_ui.insert(call_id.to_string(), ui);
        }
        None => {
            st.video_ui.remove(call_id);
        }
    }
}

/// Surface the call's engine events: log relay-allocate outcomes, decode any non-MLow (standard
/// Opus) frame the core hands back and play it, and drive the video upgrade handshake (auto-accept
/// under `--video`, otherwise park it for the stdin `v` toggle). MLow audio is decoded inside the
/// engine and arrives as playout on the speaker channel directly.
fn spawn_call_event_listener(
    handle: Arc<CallHandle>,
    speaker: async_channel::Sender<Vec<i16>>,
    auto_video: bool,
    state: Arc<Mutex<CallState>>,
) {
    let events = handle.events();
    tokio::spawn(async move {
        let mut opus = WaOpusDecoder::new().ok();
        let mut peer_video_receiver_confirmed = false;
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
                CallEvent::RtcpReceived {
                    packet_types,
                    sender_ssrc,
                    referenced_ssrcs,
                    reports_audio,
                    reports_video,
                    report_blocks,
                    feedback,
                } => {
                    let requests_keyframe = feedback
                        .iter()
                        .any(|item| item.packet_type == 206 && matches!(item.fmt, 1 | 4));
                    let blocks = report_blocks
                        .iter()
                        .map(|report| {
                            format!(
                                "{:#010x}:loss={}/{} highest={} jitter={} lsr={:#010x} dlsr={} ext={}B",
                                report.ssrc,
                                report.fraction_lost,
                                report.cumulative_lost,
                                report.extended_highest_sequence,
                                report.jitter,
                                report.last_sender_report,
                                report.delay_since_last_sender_report,
                                report.profile_extension.len(),
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    let feedback = feedback
                        .iter()
                        .map(|item| {
                            let kind = match (item.packet_type, item.fmt) {
                                (205, 1) => "NACK",
                                (206, 1) => "PLI",
                                (206, 4) => "FIR",
                                (206, 15) => "AFB",
                                _ => "unknown",
                            };
                            format!(
                                "{kind}:sender={:#010x}/media={:#010x}/fci={}B:{}",
                                item.sender_ssrc,
                                item.media_ssrc,
                                item.fci.len(),
                                hex::encode(&item.fci),
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                    if reports_video && !peer_video_receiver_confirmed {
                        info!(
                            "🎥 peer RTCP confirms receipt of our outbound video stream: blocks=[{blocks}] feedback=[{feedback}]"
                        );
                        peer_video_receiver_confirmed = true;
                    }
                    if requests_keyframe {
                        info!("🎥 peer requests an outbound keyframe: [{feedback}]");
                    }
                    debug!(
                        "📊 peer RTCP PT={packet_types:?} sender={sender_ssrc:#010x} refs={referenced_ssrcs:x?} reports_audio={reports_audio} reports_video={reports_video} blocks=[{blocks}] feedback=[{feedback}]"
                    );
                }
                CallEvent::OutboundMediaDropped {
                    video_access_units,
                    packets,
                } => {
                    warn!(
                        "🎥 relay-send backpressure: dropped {video_access_units} complete video AUs / {packets} packets"
                    );
                }
                CallEvent::VideoStateChanged { state: vs, .. } => match vs {
                    VideoState::UpgradeRequest | VideoState::UpgradeRequestV2 => {
                        if auto_video {
                            info!("🎥 peer asked for video — auto-accepting (--video)");
                            if let Err(e) = accept_peer_video(&handle).await {
                                warn!("accepting peer video failed: {e}");
                            } else {
                                mark_video(&state, handle.call_id(), Some(VideoUi::Active));
                            }
                        } else {
                            info!("🎥 peer asked for video — press `v` + Enter to accept");
                            mark_video(&state, handle.call_id(), Some(VideoUi::PendingPeerRequest));
                        }
                    }
                    VideoState::UpgradeAccept | VideoState::Enabled => {
                        info!("🎥 peer video {vs:?} — inbound video should start flowing");
                    }
                    VideoState::Stopped | VideoState::Disabled => {
                        info!("🎥 peer stopped its video");
                    }
                    other => info!("🎥 peer video state: {other:?}"),
                },
                // CallEvent is #[non_exhaustive]: ignore variants newer core versions may add.
                _ => {}
            }
        }
    });
}

/// Fresh ffmpeg/ffplay endpoints for a mid-call upgrade/accept on `handle`.
async fn accept_peer_video(handle: &CallHandle) -> Result<()> {
    let opts = VideoOpts::from_env()?;
    let src = video::spawn_video_source(&opts).await?;
    let sink = video::spawn_video_sink(&opts, handle.call_id()).await?;
    handle
        .accept_video(src, sink)
        .await
        .map_err(|e| anyhow!("accept_video: {e}"))
}
async fn respond_to_offer(
    client: &Arc<Client>,
    call: &IncomingCall,
    accept: bool,
    video: bool,
) -> Result<()> {
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
    // Callee flow: preaccept immediately, then accept (signaling).
    let audio_rates = &["16000"];
    let pre_id = hex::encode(rand::random::<[u8; 8]>());
    client
        .send_node(stanza::build_preaccept(
            call_id,
            &call.from,
            call_creator,
            &pre_id,
            audio_rates,
            // Byte-matched to a real from-start video callee: <video dec=H264 screen 0x0> in the
            // preaccept, with the 0xbb capability (the caller offers 0xfa; the callee preaccepts 0xbb).
            video,
        ))
        .await
        .map_err(|e| anyhow!("send preaccept: {e}"))?;
    let accept_id = hex::encode(rand::random::<[u8; 8]>());
    let metadata = if video { call.media.as_deref() } else { None };
    let accept_node = stanza::build_accept(&stanza::AcceptParams {
        call_id,
        to: &call.from,
        id: &accept_id,
        call_creator,
        audio_rates,
        relay_te: None,
        rte: None,
        voip_settings: None,
        // A real from-start video accept carries NO <capability> (just audio/video/net/encopt); the
        // audio path keeps advertising it.
        capability: if video { None } else { Some(&CAPABILITY_OFFER) },
        video,
        peer_abtest_bucket: metadata.and_then(|media| media.peer_abtest_bucket.as_deref()),
        peer_abtest_bucket_id_list: metadata
            .and_then(|media| media.peer_abtest_bucket_id_list.as_deref()),
    });
    client
        .send_node(accept_node)
        .await
        .map_err(|e| anyhow!("send accept: {e}"))?;
    Ok(())
}

/// Single-key commands on stdin during a live call: `v` toggles video (start an upgrade / accept a
/// pending peer request / downgrade), `q` hangs up. Skipped when stdin is not a terminal (CI /
/// piped input would EOF-spin).
fn spawn_stdin_ui(client: Arc<Client>, state: Arc<Mutex<CallState>>) {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return;
    }
    info!("⌨  during a call: `v` + Enter toggles video, `q` + Enter hangs up");
    // Read stdin on a DETACHED std thread, not `tokio::io::stdin()`: the latter reads via a runtime
    // blocking thread parked in `read()`, and the runtime drop on Ctrl+C waits for it forever — the
    // process would never exit. A plain std thread is killed when the process exits, so shutdown
    // stays clean; lines are forwarded over an async channel the cancellable UI task consumes.
    let (line_tx, line_rx) = async_channel::unbounded::<String>();
    std::thread::spawn(move || {
        use std::io::BufRead;
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line_tx.send_blocking(line).is_err() {
                break;
            }
        }
    });
    tokio::spawn(async move {
        while let Ok(line) = line_rx.recv().await {
            // The most recent live call is the toggle's target (this demo runs one call at a time).
            let picked = {
                let st = state.lock().unwrap();
                st.call_order.iter().rev().find_map(|cid| {
                    st.handles
                        .get(cid)
                        .map(|h| (cid.clone(), h.clone(), st.video_ui.get(cid).copied()))
                })
            };
            let Some((cid, handle, ui)) = picked else {
                info!("⌨  no live call");
                continue;
            };
            match line.trim() {
                "v" => match ui {
                    Some(VideoUi::Active) => {
                        info!("🎥 stopping video (downgrade to audio)");
                        if let Err(e) = handle.stop_video().await {
                            warn!("stop_video failed: {e}");
                        } else {
                            mark_video(&state, &cid, None);
                        }
                    }
                    Some(VideoUi::PendingPeerRequest) => {
                        info!("🎥 accepting the peer's video request");
                        if let Err(e) = accept_peer_video(&handle).await {
                            warn!("accept_video failed: {e}");
                        } else {
                            mark_video(&state, &cid, Some(VideoUi::Active));
                        }
                    }
                    None => {
                        info!("🎥 requesting video upgrade");
                        let started = async {
                            let opts = VideoOpts::from_env()?;
                            let src = video::spawn_video_source(&opts).await?;
                            let sink = video::spawn_video_sink(&opts, &cid).await?;
                            handle
                                .start_video(src, sink)
                                .await
                                .map_err(|e| anyhow!("start_video: {e}"))
                        }
                        .await;
                        match started {
                            Ok(()) => mark_video(&state, &cid, Some(VideoUi::Active)),
                            Err(e) => warn!("video upgrade failed: {e}"),
                        }
                    }
                },
                "q" => {
                    info!("⌨  hanging up {cid}");
                    // Signal the peer with a <terminate> (which also tears our media down), so it
                    // sees a normal hangup instead of waiting for the transport to time out. Fall
                    // back to a local-only hangup if the send fails.
                    if let Err(e) = client
                        .voip()
                        .terminate(&cid, &handle.peer_jid(), handle.call_creator())
                        .await
                    {
                        warn!("⌨  terminate failed ({e}); tearing down locally");
                        handle.hangup().await;
                    }
                }
                "" => {}
                other => info!("⌨  unknown command {other:?} — `v` toggles video, `q` hangs up"),
            }
        }
    });
}

async fn run_bot(mode: Mode) -> Result<()> {
    let store = SqliteStore::new("whatsapp.db")
        .await
        .map_err(|e| anyhow!("sqlite: {e}"))?;
    let (accept, target, video) = match mode {
        Mode::Listen { accept, video } => (accept, None, video),
        Mode::Call { jid, video } => (false, Some(jid), video),
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
    let manages_media = accept || target.is_some();
    let observer = Arc::new(CallObserver::new(client.clone(), accept, video));
    client.register_handler(observer.clone());

    if let Some(peer) = target {
        let client2 = client.clone();
        // Share UI state with the inbound path.
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
            match place_outgoing_call(&client2, &peer, video, &state).await {
                Ok(handle) => {
                    let cid = handle.call_id().to_string();
                    register_handle(&state, cid, handle).await;
                }
                Err(e) => error!("call failed: {e}"),
            }
        });
    } else {
        warn!(
            "listening for calls ({}{}). Ctrl+C to exit.",
            if accept { "auto-accept" } else { "auto-reject" },
            if video { ", video" } else { "" }
        );
    }

    if manages_media {
        spawn_stdin_ui(client.clone(), observer.state.clone());
    }

    tokio::select! {
        _ = bot.run() => {}
        // SIGINT or SIGTERM: react to `docker stop`/k8s, not just Ctrl+C.
        _ = whatsapp_rust::shutdown_signal() => { info!("shutting down"); }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        CallState, CliCommand, begin_call_startup, complete_call_startup, mark_call_most_recent,
        parse_cli, record_peer_terminate, video_source_is_ignored,
    };

    fn argv(parts: &[&str]) -> Vec<String> {
        std::iter::once("voip")
            .chain(parts.iter().copied())
            .map(String::from)
            .collect()
    }

    #[test]
    fn listen_video_implies_accept() {
        // The footgun that showed "no answer" on the phone: `listen --video` must ACCEPT (video is
        // meaningless while rejecting), not silently reject.
        assert_eq!(
            parse_cli(&argv(&["listen", "--video"])),
            CliCommand::Listen {
                accept: true,
                video: true
            }
        );
    }

    #[test]
    fn listen_modes() {
        assert_eq!(
            parse_cli(&argv(&["listen"])),
            CliCommand::Listen {
                accept: false,
                video: false
            }
        );
        assert_eq!(
            parse_cli(&argv(&["listen", "accept"])),
            CliCommand::Listen {
                accept: true,
                video: false
            }
        );
        assert_eq!(
            parse_cli(&argv(&["listen", "accept", "--video"])),
            CliCommand::Listen {
                accept: true,
                video: true
            }
        );
        // `--video` may appear before the subcommand.
        assert_eq!(
            parse_cli(&argv(&["--video", "listen", "accept"])),
            CliCommand::Listen {
                accept: true,
                video: true
            }
        );
    }

    #[test]
    fn loopback_and_call_and_usage() {
        assert_eq!(
            parse_cli(&argv(&["loopback"])),
            CliCommand::Loopback { video: false }
        );
        assert_eq!(
            parse_cli(&argv(&["loopback", "--video"])),
            CliCommand::Loopback { video: true }
        );
        assert_eq!(
            parse_cli(&argv(&["call", "123@lid", "--video"])),
            CliCommand::Call {
                jid: "123@lid".into(),
                video: true
            }
        );
        // `call` with no jid, and unknown/empty commands, are usage errors.
        assert_eq!(parse_cli(&argv(&["call"])), CliCommand::Usage);
        assert_eq!(parse_cli(&argv(&["bogus"])), CliCommand::Usage);
        assert_eq!(parse_cli(&argv(&[])), CliCommand::Usage);
    }

    #[test]
    fn warns_when_video_input_cannot_be_used() {
        let audio_only = parse_cli(&argv(&["listen", "accept"]));
        let video = parse_cli(&argv(&["listen", "accept", "--video"]));

        assert!(video_source_is_ignored(&audio_only, true));
        assert!(!video_source_is_ignored(&audio_only, false));
        assert!(!video_source_is_ignored(&video, true));
        assert!(!video_source_is_ignored(&CliCommand::Usage, true));
    }

    #[test]
    fn peer_terminate_tombstones_in_progress_media_startup() {
        let state = Arc::new(Mutex::new(CallState::default()));
        begin_call_startup(&state, "CALL-ID-STARTING");
        record_peer_terminate(&state, "CALL-ID-STARTING");
        assert!(complete_call_startup(&state, "CALL-ID-STARTING"));

        let st = state.lock().unwrap();
        assert!(st.starting.is_empty());
        assert!(st.terminated_during_startup.is_empty());
    }

    #[test]
    fn stdin_ui_order_tracks_the_most_recent_call() {
        let mut order = Vec::new();
        mark_call_most_recent(&mut order, "CALL-A");
        mark_call_most_recent(&mut order, "CALL-B");
        mark_call_most_recent(&mut order, "CALL-A");
        assert_eq!(order, ["CALL-B", "CALL-A"]);
    }
}
