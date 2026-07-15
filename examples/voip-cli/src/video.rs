//! Cross-platform video source/sink for the demo, with ffmpeg/ffplay as external codec
//! subprocesses (no native video deps in the Rust closure — the library only transports
//! pre-encoded H.264 Annex-B access units).
//!
//!   source: webcam (per-OS ffmpeg input) | any file/URL | `testsrc2` synthetic pattern
//!           -> libx264 (baseline, 1280x720@20, ~1980kbps, AUD-delimited) -> stdout pipe
//!           -> `AnnexBAuSplitter` -> one AU per channel item
//!   sink:   received AUs -> ffplay window (low-latency flags) | raw `.h264` file | discard
//!
//! Env knobs: `WA_VIDEO_INPUT` = `testsrc` | existing path/URL | webcam device override;
//! `WA_VIDEO_SINK` = `window` (default) | `file` | `none`; `WA_VIDEO_SIZE`, `WA_VIDEO_FPS`,
//! `WA_VIDEO_BITRATE_KBPS`, and `WA_VIDEO_SINK_FPS` override the captured WhatsApp defaults.

use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use log::{debug, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use wacore::voip::h264::{AnnexBAuSplitter, au_has_idr, nal_unit_type, split_annexb};
use wacore::voip::rtp::VIDEO_CLOCK_RATE;
use whatsapp_rust::voip::{VideoFrame, VideoSource};

/// The NAL-unit type of each NAL in an Annex-B access unit (5=IDR, 7=SPS, 8=PPS, 1=non-IDR, ...),
/// for diagnostics comparing our outbound AUs against the decodable inbound ones.
fn au_nal_types(au: &[u8]) -> Vec<u8> {
    split_annexb(au).map(nal_unit_type).collect()
}

fn parameter_set_summary(au: &[u8]) -> Option<String> {
    let mut sps = None;
    let mut pps = None;
    for nal in split_annexb(au) {
        match nal_unit_type(nal) {
            7 => sps.get_or_insert(nal),
            8 => pps.get_or_insert(nal),
            _ => continue,
        };
    }
    let sps = sps?;
    let codec = sps.get(1..4).map(hex::encode).unwrap_or_default();
    Some(format!(
        "codec=avc1.{codec} SPS={} PPS={}",
        hex::encode(sps),
        pps.map(hex::encode).unwrap_or_default(),
    ))
}

/// Captured WhatsApp Web high-quality mode (H.264 Constrained Baseline, Level 3.1).
const DEFAULT_VIDEO_WIDTH: u32 = 1280;
const DEFAULT_VIDEO_HEIGHT: u32 = 720;
const DEFAULT_VIDEO_FPS: u32 = 20;
const DEFAULT_VIDEO_BITRATE_KBPS: u32 = 1980;
/// Raw-H.264 fallback when the bitstream carries no timing; preview timestamps follow arrival time.
const DEFAULT_SINK_FPS: u32 = 15;
const GOP_SECONDS: u32 = 3;
const H264_LEVEL_31_MAX_FRAME_MBS: u32 = 3600;
const H264_LEVEL_31_MAX_MBS_PER_SECOND: u32 = 108_000;
const H264_BASELINE_LEVEL_31_MAX_KBPS: u32 = 14_000;
/// Keep codec plumbing queues short; stale video is worse than a dropped frame in a call.
const SOURCE_CHANNEL_CAP: usize = 4;
const SINK_CHANNEL_CAP: usize = 2;
/// stdout pipe read granularity.
const READ_CHUNK: usize = 32 * 1024;
/// A sink write slower than this sheds frames instead of stalling the call's forwarder.
const SINK_WRITE_TIMEOUT: Duration = Duration::from_millis(75);

pub struct VideoOpts {
    pub input: VideoInput,
    pub sink: VideoSinkMode,
    quality: VideoQuality,
    sink_fps: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct VideoQuality {
    width: u32,
    height: u32,
    fps: u32,
    bitrate_kbps: u32,
}

impl VideoQuality {
    fn new(width: u32, height: u32, fps: u32, bitrate_kbps: u32) -> Result<Self> {
        if width == 0 || height == 0 || !width.is_multiple_of(2) || !height.is_multiple_of(2) {
            bail!("WA_VIDEO_SIZE must contain positive even dimensions (for yuv420p)");
        }
        if fps == 0 || fps > 60 || !VIDEO_CLOCK_RATE.is_multiple_of(fps) {
            bail!("WA_VIDEO_FPS must be a divisor of 90000 in the range 1..=60");
        }
        if !(25..=H264_BASELINE_LEVEL_31_MAX_KBPS).contains(&bitrate_kbps) {
            bail!("WA_VIDEO_BITRATE_KBPS must be in the range 25..=14000");
        }
        let frame_mbs = width.div_ceil(16).saturating_mul(height.div_ceil(16));
        if frame_mbs > H264_LEVEL_31_MAX_FRAME_MBS
            || frame_mbs.saturating_mul(fps) > H264_LEVEL_31_MAX_MBS_PER_SECOND
        {
            bail!("WA_VIDEO_SIZE/FPS exceeds H.264 Level 3.1 (up to 1280x720@30)");
        }
        Ok(Self {
            width,
            height,
            fps,
            bitrate_kbps,
        })
    }

    fn whatsapp_hd() -> Self {
        Self {
            width: DEFAULT_VIDEO_WIDTH,
            height: DEFAULT_VIDEO_HEIGHT,
            fps: DEFAULT_VIDEO_FPS,
            bitrate_kbps: DEFAULT_VIDEO_BITRATE_KBPS,
        }
    }

    fn size(self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    fn timestamp_stride(self) -> u32 {
        VIDEO_CLOCK_RATE / self.fps
    }
}

fn parse_video_size(value: &str) -> Result<(u32, u32)> {
    let (width, height) = value
        .split_once('x')
        .or_else(|| value.split_once('X'))
        .ok_or_else(|| anyhow!("WA_VIDEO_SIZE must use WIDTHxHEIGHT, for example 1280x720"))?;
    Ok((
        width
            .parse()
            .context("WA_VIDEO_SIZE width is not an integer")?,
        height
            .parse()
            .context("WA_VIDEO_SIZE height is not an integer")?,
    ))
}

fn env_u32(name: &str, default: u32) -> Result<u32> {
    match std::env::var(name) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("{name} must be an integer")),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(e) => Err(e).with_context(|| format!("reading {name}")),
    }
}

pub enum VideoInput {
    /// OS default webcam, or an override device (path on Linux, index/name on macOS/Windows).
    Webcam(Option<String>),
    /// Any file or URL ffmpeg can open; looped and rate-limited to realtime.
    Media(String),
    /// `lavfi testsrc2` synthetic pattern — no camera needed (CI / second instance).
    TestSrc,
}

pub enum VideoSinkMode {
    /// ffplay window (falls back to `File` when ffplay is missing).
    Window,
    /// Raw `.h264` capture, replayable with `ffplay -f h264 <file>`.
    File,
    /// Count and discard.
    None,
}

impl VideoOpts {
    pub fn from_env() -> Result<Self> {
        let input = match std::env::var("WA_VIDEO_INPUT") {
            Ok(v) if v == "testsrc" => VideoInput::TestSrc,
            // A `/dev/*` node is a webcam override (v4l2 on Linux), NOT a media file — it exists on
            // disk but must not be opened with `-stream_loop`.
            Ok(v) if v.starts_with("/dev/") => VideoInput::Webcam(Some(v)),
            Ok(v) if std::path::Path::new(&v).exists() || v.contains("://") => VideoInput::Media(v),
            Ok(v) => VideoInput::Webcam(Some(v)),
            Err(_) => VideoInput::Webcam(None),
        };
        let sink = match std::env::var("WA_VIDEO_SINK").as_deref() {
            Ok("file") => VideoSinkMode::File,
            Ok("none") => VideoSinkMode::None,
            _ => VideoSinkMode::Window,
        };
        let defaults = VideoQuality::whatsapp_hd();
        let (width, height) = match std::env::var("WA_VIDEO_SIZE") {
            Ok(value) => parse_video_size(&value)?,
            Err(std::env::VarError::NotPresent) => (defaults.width, defaults.height),
            Err(e) => return Err(e).context("reading WA_VIDEO_SIZE"),
        };
        let quality = VideoQuality::new(
            width,
            height,
            env_u32("WA_VIDEO_FPS", defaults.fps)?,
            env_u32("WA_VIDEO_BITRATE_KBPS", defaults.bitrate_kbps)?,
        )?;
        let sink_fps = env_u32("WA_VIDEO_SINK_FPS", DEFAULT_SINK_FPS)?;
        if sink_fps == 0 || sink_fps > 60 {
            bail!("WA_VIDEO_SINK_FPS must be in the range 1..=60");
        }
        Ok(Self {
            input,
            sink,
            quality,
            sink_fps,
        })
    }
}

/// Fail fast with an actionable message when a required tool is not on PATH. Async so the
/// spawn+wait probe doesn't block a runtime thread (it runs from call-setup and event-task paths).
async fn ensure_tool(tool: &str) -> Result<()> {
    match Command::new(tool)
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
    {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => bail!(
            "`{tool}` not found on PATH — install ffmpeg (pacman -S ffmpeg | apt install ffmpeg | \
             brew install ffmpeg | winget install Gyan.FFmpeg) or run without --video"
        ),
        Err(e) => Err(e).context(format!("probing `{tool}`")),
    }
}

/// The per-input head of the ffmpeg command line.
fn input_args(input: &VideoInput, quality: VideoQuality) -> Result<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let fps = quality.fps.to_string();
    let size = quality.size();
    Ok(match input {
        // `-re` paces the synthetic source at realtime; unthrottled, lavfi encodes as fast as the
        // CPU allows and floods the call's bounded channel with dropped GOPs.
        VideoInput::TestSrc => s(&[
            "-re",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size={size}:rate={fps}"),
        ]),
        VideoInput::Media(path) => {
            let mut v = s(&["-re", "-stream_loop", "-1", "-i", path]);
            // Normalize any input to the call's fixed geometry/rate.
            v.extend(s(&[
                "-vf",
                &format!(
                    "scale={W}:{H}:force_original_aspect_ratio=decrease,\
                     pad={W}:{H}:(ow-iw)/2:(oh-ih)/2,fps={FPS}",
                    W = quality.width,
                    H = quality.height,
                    FPS = quality.fps,
                ),
            ]));
            v
        }
        VideoInput::Webcam(dev) => {
            if cfg!(target_os = "linux") {
                let dev = dev.clone().unwrap_or_else(|| "/dev/video0".into());
                s(&[
                    "-f",
                    "v4l2",
                    "-framerate",
                    &fps,
                    "-video_size",
                    &size,
                    "-i",
                    &dev,
                ])
            } else if cfg!(target_os = "macos") {
                let dev = dev.clone().unwrap_or_else(|| "0".into());
                s(&[
                    "-f",
                    "avfoundation",
                    "-framerate",
                    &fps,
                    "-video_size",
                    &size,
                    "-i",
                    &dev,
                ])
            } else if cfg!(target_os = "windows") {
                let Some(dev) = dev else {
                    bail!(
                        "no default webcam name on Windows — list devices with `ffmpeg \
                         -list_devices true -f dshow -i dummy` and set WA_VIDEO_INPUT=<name>"
                    );
                };
                s(&[
                    "-f",
                    "dshow",
                    "-rtbufsize",
                    "64M",
                    "-framerate",
                    &fps,
                    "-video_size",
                    &size,
                    "-i",
                    &format!("video={dev}"),
                ])
            } else {
                bail!("no webcam input mapping for this OS — use WA_VIDEO_INPUT=testsrc or a file");
            }
        }
    })
}

fn encoder_args(quality: VideoQuality) -> Vec<String> {
    let gop = quality.fps.saturating_mul(GOP_SECONDS).to_string();
    let fps = quality.fps.to_string();
    let bitrate = format!("{}k", quality.bitrate_kbps);
    [
        "-r",
        &fps,
        "-fps_mode",
        "cfr",
        "-c:v",
        "libx264",
        "-profile:v",
        "baseline",
        "-level:v",
        "3.1",
        "-pix_fmt",
        "yuv420p",
        "-preset",
        "veryfast",
        "-tune",
        "zerolatency",
        "-g",
        &gop,
        "-keyint_min",
        &gop,
        "-sc_threshold",
        "0",
        "-b:v",
        &bitrate,
        "-x264-params",
        "repeat-headers=1",
        "-bsf:v",
        "h264_metadata=aud=insert",
        "-an",
        "-f",
        "h264",
        "pipe:1",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn spawn_ffmpeg_encoder(input: &VideoInput, quality: VideoQuality) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"]);
    cmd.args(input_args(input, quality)?);
    // repeat-headers=1: SPS/PPS ride every IDR, so a peer joining mid-stream (upgrade) or
    // recovering from a drop can always re-sync. aud=insert: the AU delimiter the splitter cuts on.
    cmd.args(encoder_args(quality));
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        // stderr inherited: encoder errors (missing camera, busy device) reach the user directly.
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    cmd.spawn().context("spawn ffmpeg")
}

pub struct FfmpegVideoSource {
    frames: async_channel::Receiver<Vec<u8>>,
    timestamp_stride: u32,
}

impl FfmpegVideoSource {
    async fn recv(&self) -> std::result::Result<Vec<u8>, async_channel::RecvError> {
        self.frames.recv().await
    }
}

impl VideoSource for FfmpegVideoSource {
    fn frames(&self) -> async_channel::Receiver<Vec<u8>> {
        self.frames.clone()
    }

    fn rtp_timestamp_stride(&self) -> u32 {
        self.timestamp_stride
    }
}

/// Spawn the ffmpeg encoder and return complete H.264 AUs with their RTP cadence.
pub async fn spawn_video_source(opts: &VideoOpts) -> Result<FfmpegVideoSource> {
    ensure_tool("ffmpeg").await?;
    info!(
        "🎥 encoder: {}x{} @ {} fps, {} kbps average target, H.264 baseline level 3.1",
        opts.quality.width, opts.quality.height, opts.quality.fps, opts.quality.bitrate_kbps,
    );
    let mut child = spawn_ffmpeg_encoder(&opts.input, opts.quality)?;
    let mut stdout = child.stdout.take().context("ffmpeg stdout")?;
    let (tx, rx) = async_channel::bounded::<Vec<u8>>(SOURCE_CHANNEL_CAP);

    tokio::spawn(async move {
        // Owning the child keeps kill_on_drop armed for the whole read loop.
        let _child = &mut child;
        let mut splitter = AnnexBAuSplitter::default();
        let mut buf = vec![0u8; READ_CHUNK];
        let mut aus: Vec<Vec<u8>> = Vec::new();
        // When the channel backs up we drop AUs — but only from a keyframe boundary, because an
        // arbitrary dropped AU corrupts the peer's decode until the next IDR anyway. Dropping
        // *deliberately until* the next IDR turns backpressure into one clean skip.
        let mut dropping = false;
        let (mut made, mut dropped, mut sent) = (0u64, 0u64, 0u64);
        let mut logged_parameter_sets = false;
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break, // encoder EOF (camera unplugged, file without loop)
                Ok(n) => {
                    splitter.push(&buf[..n], &mut aus);
                    for au in aus.drain(..) {
                        made += 1;
                        if !logged_parameter_sets && let Some(summary) = parameter_set_summary(&au)
                        {
                            info!("🎥 OUT H.264: {summary} NALs={:?}", au_nal_types(&au),);
                            logged_parameter_sets = true;
                        }
                        if dropping {
                            // Resume only on an IDR (a self-contained restart): resuming on a
                            // parameter-set-only AU would forward dependent frames the peer can't
                            // decode until the real keyframe.
                            if !au_has_idr(&au) {
                                dropped += 1;
                                continue;
                            }
                            dropping = false;
                        }
                        // Periodic outbound telemetry so a live test shows whether we're actually
                        // producing (and sending) video AUs, and whether they carry IDR keyframes.
                        if sent.is_multiple_of(30) {
                            debug!(
                                "🎥 OUT video: {sent} AUs queued ({dropped} source-dropped) | {}B, NALs {:?}, keyframe={}",
                                au.len(),
                                au_nal_types(&au),
                                au_has_idr(&au)
                            );
                        }
                        match tx.try_send(au) {
                            Ok(()) => sent += 1,
                            Err(async_channel::TrySendError::Full(_)) => {
                                dropped += 1;
                                dropping = true;
                            }
                            // Call ended: stop the encoder.
                            Err(async_channel::TrySendError::Closed(_)) => return,
                        }
                    }
                }
                Err(e) => {
                    warn!("🎥 video source read failed: {e}");
                    break;
                }
            }
        }
        if let Some(last) = splitter.finish() {
            let _ = tx.try_send(last);
        }
        // The call survives a dead source (audio keeps running), same as a muted mic.
        warn!("🎥 video source ended ({made} AUs, {dropped} dropped)");
    });
    Ok(FfmpegVideoSource {
        frames: rx,
        timestamp_stride: opts.quality.timestamp_stride(),
    })
}

fn orientation_filter(orientation: u8) -> Option<&'static str> {
    match orientation & 0x03 {
        0 => None,
        1 => Some("transpose=cclock"),
        2 => Some("hflip,vflip"),
        3 => Some("transpose=clock"),
        _ => unreachable!(),
    }
}

fn ffplay_args(tag: &str, frame_rate: u32, orientation: u8) -> Vec<String> {
    let mut args: Vec<String> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-avioflags",
        "direct",
        "-fflags",
        "nobuffer",
        "-flags",
        "low_delay",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        "-fpsprobesize",
        "0",
        "-max_delay",
        "0",
        "-framedrop",
        "-use_wallclock_as_timestamps",
        "1",
        "-window_title",
        &format!("WA Video {tag}"),
        "-f",
        "h264",
        "-framerate",
        &frame_rate.to_string(),
    ]
    .into_iter()
    .map(String::from)
    .collect();
    if let Some(filter) = orientation_filter(orientation) {
        args.extend(["-vf".into(), filter.into()]);
    }
    args.extend(["-i".into(), "pipe:0".into()]);
    args
}

fn spawn_ffplay(tag: &str, frame_rate: u32, orientation: u8) -> Result<Child> {
    let mut cmd = Command::new("ffplay");
    cmd.args(ffplay_args(tag, frame_rate, orientation));
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    cmd.spawn().context("spawn ffplay")
}

/// Where the sink writer is currently sending frames.
enum SinkTarget {
    Window(Child),
    File(tokio::fs::File, String),
    None,
}

/// Spawn the display/record sink and return the channel the library pushes received frames into
/// (`VideoSink` blanket impl on `Sender<VideoFrame>`). Lazy: the window/file only appears when the
/// first frame arrives, so a call that never activates video opens nothing.
pub async fn spawn_video_sink(
    opts: &VideoOpts,
    tag: &str,
) -> Result<async_channel::Sender<VideoFrame>> {
    let want_window = matches!(opts.sink, VideoSinkMode::Window);
    let want_file = matches!(opts.sink, VideoSinkMode::File);
    let use_window = want_window && ensure_tool("ffplay").await.is_ok();
    if want_window && !use_window {
        warn!("ffplay not found — recording received video to a .h264 file instead");
    }
    let (tx, rx) = async_channel::bounded::<VideoFrame>(SINK_CHANNEL_CAP);
    let tag = tag.to_string();
    let sink_fps = opts.sink_fps;

    tokio::spawn(async move {
        let mut target: Option<SinkTarget> = None;
        let mut dropping = false;
        let mut last_orientation = 0u8;
        let mut player_orientation = None;
        let mut pending_orientation = None;
        let mut recv = 0u64;
        let mut logged_parameter_sets = false;
        while let Ok(frame) = rx.recv().await {
            if !logged_parameter_sets && let Some(summary) = parameter_set_summary(&frame.data) {
                info!(
                    "🎥 IN  H.264: {summary} nals={:?}",
                    au_nal_types(&frame.data),
                );
                logged_parameter_sets = true;
            }
            // Inbound telemetry: the peer's AUs DO decode (ffplay shows them), so their shape is the
            // reference to compare our outbound AUs against.
            if recv.is_multiple_of(30) {
                debug!(
                    "🎥 IN  video: {recv} AUs recv | {}B, NALs {:?}, keyframe={}",
                    frame.data.len(),
                    au_nal_types(&frame.data),
                    au_has_idr(&frame.data)
                );
            }
            recv += 1;
            let orientation = frame.orientation & 0x03;
            if orientation != last_orientation {
                last_orientation = orientation;
                info!(
                    "🎥 peer camera orientation changed to {}° — correcting preview",
                    orientation as u32 * 90
                );
                if matches!(target.as_ref(), Some(SinkTarget::Window(_))) {
                    pending_orientation =
                        (player_orientation != Some(orientation)).then_some(orientation);
                }
            }
            // Lazy-open on the first frame.
            if target.is_none() {
                target = Some(if use_window {
                    match spawn_ffplay(&tag, sink_fps, orientation) {
                        Ok(child) => {
                            player_orientation = Some(orientation);
                            SinkTarget::Window(child)
                        }
                        Err(e) => {
                            warn!("🎥 ffplay spawn failed ({e}); discarding video");
                            SinkTarget::None
                        }
                    }
                } else if want_file || want_window {
                    let path = format!("wa-video-{tag}.h264");
                    match tokio::fs::File::create(&path).await {
                        Ok(f) => {
                            info!(
                                "🎥 recording received video to {path} (replay: ffplay -f h264 {path})"
                            );
                            SinkTarget::File(f, path)
                        }
                        Err(e) => {
                            warn!("🎥 cannot create {path} ({e}); discarding video");
                            SinkTarget::None
                        }
                    }
                } else {
                    SinkTarget::None
                });
            }
            // A new filter graph needs a fresh decoder. Switch only when this AU can seed it.
            if let Some(next_orientation) = pending_orientation
                && matches!(target.as_ref(), Some(SinkTarget::Window(_)))
                && au_has_idr(&frame.data)
            {
                match spawn_ffplay(&tag, sink_fps, next_orientation) {
                    Ok(child) => {
                        target = Some(SinkTarget::Window(child));
                        player_orientation = Some(next_orientation);
                        pending_orientation = None;
                        dropping = false;
                    }
                    Err(e) => {
                        warn!("🎥 cannot rotate ffplay preview ({e}); keeping current window");
                        pending_orientation = None;
                    }
                }
            }
            // Recover from a skip only on an IDR (a self-contained restart): a mid-GOP resume, or
            // one on a parameter-set-only AU, shows garbage until the real keyframe.
            if dropping {
                if !au_has_idr(&frame.data) {
                    continue;
                }
                dropping = false;
            }
            match target.as_mut() {
                Some(SinkTarget::Window(child)) => {
                    let Some(stdin) = child.stdin.as_mut() else {
                        continue;
                    };
                    match tokio::time::timeout(SINK_WRITE_TIMEOUT, stdin.write_all(&frame.data))
                        .await
                    {
                        Ok(Ok(())) => {}
                        // Slow consumer: shed until the next keyframe instead of stalling.
                        Err(_) => dropping = true,
                        // Window closed / player died: degrade to discard, keep the call alive.
                        Ok(Err(e)) => {
                            warn!("🎥 ffplay pipe failed ({e}); discarding further video");
                            target = Some(SinkTarget::None);
                            player_orientation = None;
                            pending_orientation = None;
                        }
                    }
                }
                Some(SinkTarget::File(f, path)) => {
                    if let Err(e) = f.write_all(&frame.data).await {
                        warn!("🎥 write to {path} failed ({e}); discarding further video");
                        target = Some(SinkTarget::None);
                    }
                }
                Some(SinkTarget::None) | None => {}
            }
        }
        // Channel closed (call ended): dropping `target` kills ffplay via kill_on_drop.
    });
    Ok(tx)
}

/// Local smoke test: ffmpeg source -> AU channel -> sink (ffplay window). Exercises the whole
/// external-codec plumbing without any WhatsApp connection.
///
/// Handles Ctrl+C itself and returns cleanly: without it, the default SIGINT terminates the process
/// abruptly, so the ffmpeg/ffplay `Child`s never drop and their `kill_on_drop` never fires — the
/// subprocesses are orphaned and keep spamming broken-pipe errors. Returning from here drops `src`
/// and `sink`, which ends their reader/writer tasks and kills the subprocesses on drop.
pub async fn run_video_loopback(opts: &VideoOpts) -> Result<()> {
    let src = spawn_video_source(opts).await?;
    let sink = spawn_video_sink(opts, "loopback").await?;
    info!("🎥 video loopback running — Ctrl+C to stop");
    let mut n = 0u64;
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("🎥 stopping video loopback (Ctrl+C)");
                return Ok(());
            }
            au = src.recv() => {
                let Ok(au) = au else {
                    return Err(anyhow!("video source ended"));
                };
                let frame = VideoFrame::new(au);
                if sink.send(frame).await.is_err() {
                    return Err(anyhow!("video sink closed"));
                }
                n += 1;
                if n.is_multiple_of((opts.quality.fps * 10) as u64) {
                    info!(
                        "🎥 {n} AUs piped through the video plumbing ({}s)",
                        n / opts.quality.fps as u64
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_after<'a>(args: &'a [String], option: &str) -> &'a str {
        let index = args.iter().position(|arg| arg == option).expect("option");
        &args[index + 1]
    }

    #[test]
    fn nal_diagnostics_report_parameter_sets() {
        let au = [
            &[0, 0, 0, 1, 0x69, 0xf0][..],
            &[0, 0, 0, 1, 0x67, 0x42, 0xc0, 0x1f][..],
            &[0, 0, 0, 1, 0x68, 0xce, 0x06, 0xe2][..],
            &[0, 0, 0, 1, 0x65, 1, 2, 3][..],
        ]
        .concat();

        assert_eq!(au_nal_types(&au), [9, 7, 8, 5]);
        assert_eq!(
            parameter_set_summary(&au).as_deref(),
            Some("codec=avc1.42c01f SPS=6742c01f PPS=68ce06e2")
        );
    }

    #[test]
    fn whatsapp_hd_quality_matches_reference_capture() {
        let quality = VideoQuality::whatsapp_hd();
        assert_eq!(
            quality,
            VideoQuality {
                width: 1280,
                height: 720,
                fps: 20,
                bitrate_kbps: 1980,
            }
        );
        assert_eq!(quality.timestamp_stride(), 4500);
        assert!(VideoQuality::new(1280, 720, 30, 2000).is_ok());
        assert!(VideoQuality::new(1920, 1080, 20, 2000).is_err());
        assert!(VideoQuality::new(1280, 720, 29, 2000).is_err());
        assert!(VideoQuality::new(1279, 720, 20, 2000).is_err());
    }

    #[test]
    fn encoder_uses_unbuffered_average_bitrate_with_whatsapp_hd_cadence() {
        let args = encoder_args(VideoQuality::whatsapp_hd());
        assert_eq!(value_after(&args, "-profile:v"), "baseline");
        assert_eq!(value_after(&args, "-level:v"), "3.1");
        assert_eq!(value_after(&args, "-preset"), "veryfast");
        assert_eq!(value_after(&args, "-tune"), "zerolatency");
        assert_eq!(value_after(&args, "-r"), "20");
        assert_eq!(value_after(&args, "-g"), "60");
        assert_eq!(value_after(&args, "-b:v"), "1980k");
        assert!(!args.iter().any(|arg| arg == "-maxrate"));
        assert!(!args.iter().any(|arg| arg == "-bufsize"));
    }

    #[test]
    fn input_geometry_follows_quality() {
        let quality = VideoQuality::new(640, 360, 20, 750).unwrap();
        let testsrc = input_args(&VideoInput::TestSrc, quality).unwrap();
        assert_eq!(value_after(&testsrc, "-i"), "testsrc2=size=640x360:rate=20");

        let media = input_args(&VideoInput::Media("clip.mp4".into()), quality).unwrap();
        let filter = value_after(&media, "-vf");
        assert!(filter.contains("scale=640:360"));
        assert!(filter.contains("fps=20"));
    }

    #[test]
    fn ffplay_uses_arrival_timestamps_and_bounded_low_latency_flags() {
        let args = ffplay_args("test", 15, 0);
        assert_eq!(value_after(&args, "-framerate"), "15");
        assert_eq!(value_after(&args, "-use_wallclock_as_timestamps"), "1");
        assert_eq!(value_after(&args, "-fflags"), "nobuffer");
        assert_eq!(value_after(&args, "-flags"), "low_delay");
        assert_eq!(value_after(&args, "-avioflags"), "direct");
        assert_eq!(value_after(&args, "-max_delay"), "0");
        assert_eq!(value_after(&args, "-fpsprobesize"), "0");
        assert!(args.iter().any(|arg| arg == "-framedrop"));
        assert!(!args.iter().any(|arg| arg == "-infbuf"));
        assert!(
            args.iter().position(|arg| arg == "-framerate")
                < args.iter().position(|arg| arg == "-i")
        );
        assert!(!args.iter().any(|arg| arg == "-vf"));
    }

    #[test]
    fn ffplay_undoes_whatsapp_device_orientation() {
        assert_eq!(orientation_filter(0), None);
        assert_eq!(orientation_filter(1), Some("transpose=cclock"));
        assert_eq!(orientation_filter(2), Some("hflip,vflip"));
        assert_eq!(orientation_filter(3), Some("transpose=clock"));
        assert_eq!(orientation_filter(5), orientation_filter(1));

        let args = ffplay_args("test", 15, 1);
        assert_eq!(value_after(&args, "-vf"), "transpose=cclock");
        assert!(args.iter().position(|arg| arg == "-vf") < args.iter().position(|arg| arg == "-i"));
    }

    #[test]
    fn parses_case_insensitive_video_size() {
        assert_eq!(parse_video_size("1280x720").unwrap(), (1280, 720));
        assert_eq!(parse_video_size("640X480").unwrap(), (640, 480));
        assert!(parse_video_size("1280:720").is_err());
    }
}
