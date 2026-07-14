//! cpal audio bridge for calls: mic -> 16 kHz mono i16 frames -> engine, engine -> speaker.
//! Lifted from examples/voip-cli (the reference consumer of the voip facade); candidate for a
//! shared crate if a third consumer appears.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use anyhow::{Result, anyhow};
use log::{error, info, warn};
use portable_atomic::AtomicU64;

const FRAME_SAMPLES: usize = 960; // 60 ms @ 16 kHz
const WA_RATE: u32 = 16_000;

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
pub fn spawn_mic() -> Result<async_channel::Receiver<Vec<i16>>> {
    let host = cpal::default_host();
    // Honor `WA_INPUT_DEVICE` (a name substring) to override a wrong/silent default endpoint, else use
    // the OS default. We only enumerate `input_devices()` when a name is requested: enumerating on
    // Linux/ALSA probes the legacy OSS plugin and prints harmless "Cannot open /dev/dsp" noise to
    // stderr (from libasound, not our logger), so the default path -- `default_input_device()`, which
    // does not enumerate -- avoids it entirely. A no-match error stays generic so device names do
    // not leak through logs.
    let want = std::env::var("WA_INPUT_DEVICE")
        .ok()
        .filter(|s| !s.is_empty());
    let device = match &want {
        Some(want) => {
            let devices: Vec<cpal::Device> = host
                .input_devices()
                .map(|i| i.collect())
                .unwrap_or_default();
            devices
                .into_iter()
                .find(|d| {
                    d.description()
                        .is_ok_and(|x| x.name().contains(want.as_str()))
                })
                .ok_or_else(|| anyhow!("WA_INPUT_DEVICE did not match an input device"))?
        }
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("no default input device"))?,
    };
    let (config, format) = default_config(&device, true)?;
    let src_rate = config.sample_rate;
    let channels = config.channels as usize;
    info!("🎤 cpal input: {src_rate} Hz, {channels} ch, {format:?}");

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
                    match tx_drain.try_send(frame) {
                        Ok(()) => {}
                        Err(async_channel::TrySendError::Full(_)) => {
                            dropped.fetch_add(1, Ordering::Relaxed);
                        }
                        // Receiver gone = call ended: teardown, not backpressure.
                        Err(async_channel::TrySendError::Closed(_)) => return,
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
    // Startup barrier: the caller must not get a "working" mic whose stream never started —
    // the call would establish and transmit silence forever.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
    // Build on a dedicated thread: the !Send stream must be created, played, and dropped there.
    std::thread::spawn(move || {
        let started =
            build_input_stream(&device, &config, format, channels, prod).and_then(|stream| {
                match stream.play() {
                    Ok(()) => Ok(stream),
                    Err(e) => Err(anyhow!("play input stream: {e}")),
                }
            });
        match started {
            // The binding keeps the stream alive while parked.
            Ok(_stream) => {
                let _ = ready_tx.send(Ok(()));
                // Hold the stream until the call drops its receiver (or the call ends), polling in
                // short intervals (park alone never wakes on a channel close), then fall out of
                // scope so the stream drops and frees the device.
                while !teardown.is_closed() && alive_thread.load(Ordering::Relaxed) {
                    std::thread::park_timeout(std::time::Duration::from_millis(250));
                }
            }
            Err(e) => {
                error!("mic stream startup failed: {e}");
                let _ = ready_tx.send(Err(e));
            }
        }
        // Signal the drain to stop (single exit, any reason) so the channel closes and the
        // consumer observes the mic ending instead of blocking forever on frames that never arrive.
        alive_thread.store(false, Ordering::Relaxed);
    });
    // Build/play are near-instant; a dead sender means the thread panicked.
    ready_rx
        .recv()
        .map_err(|_| anyhow!("mic stream thread exited before signaling readiness"))??;
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
pub fn spawn_speaker() -> Result<async_channel::Sender<Vec<i16>>> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no default output device"))?;
    let (config, format) = default_config(&device, false)?;
    let dst_rate = config.sample_rate;
    let channels = config.channels as usize;
    info!("🔈 cpal output: {dst_rate} Hz, {channels} ch, {format:?}");

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

    // Held by the stream thread so a build/play failure can close the channel.
    let rx_close = rx.clone();

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

    // Startup barrier mirroring spawn_mic: a speaker whose stream never started must fail call
    // setup, not leave the engine writing remote audio into a ring nobody drains (silent call).
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
    // Build/play the !Send output stream on a dedicated parked thread.
    std::thread::spawn(move || {
        let started =
            build_output_stream(&device, &config, format, channels, cons, pops, underruns)
                .and_then(|stream| match stream.play() {
                    Ok(()) => Ok(stream),
                    Err(e) => Err(anyhow!("play output stream: {e}")),
                });
        match started {
            // The binding keeps the stream alive while parked.
            Ok(_stream) => {
                let _ = ready_tx.send(Ok(()));
                // Hold the stream until the drain signals teardown, then fall out of scope so
                // the stream drops and frees the output device.
                while !teardown.load(Ordering::Relaxed) {
                    std::thread::park_timeout(std::time::Duration::from_millis(250));
                }
            }
            Err(e) => {
                error!("speaker stream startup failed: {e}");
                let _ = ready_tx.send(Err(e));
                // Signal teardown and close the channel so the drain and diag tasks stop and
                // senders see the dead speaker instead of feeding a stream that never started.
                teardown.store(true, Ordering::Relaxed);
                rx_close.close();
            }
        }
    });
    // Build/play are near-instant; a dead sender means the thread panicked.
    ready_rx
        .recv()
        .map_err(|_| anyhow!("speaker stream thread exited before signaling readiness"))??;
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
