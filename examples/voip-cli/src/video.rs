//! Cross-platform video source/sink for the demo, with ffmpeg/ffplay as external codec
//! subprocesses (no native video deps in the Rust closure — the library only transports
//! pre-encoded H.264 Annex-B access units).
//!
//!   source: webcam (per-OS ffmpeg input) | any file/URL | `testsrc2` synthetic pattern
//!           -> libx264 (baseline, 640x480@15, ~500kbps, AUD-delimited) -> stdout pipe
//!           -> `AnnexBAuSplitter` -> one AU per channel item
//!   sink:   received AUs -> ffplay window (low-latency flags) | raw `.h264` file | discard
//!
//! Env knobs: `WA_VIDEO_INPUT` = `testsrc` | existing path/URL | webcam device override
//! (absent = default webcam); `WA_VIDEO_SINK` = `window` (default) | `file` | `none`.

use std::process::Stdio;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use log::{info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use wacore::voip::h264::{AnnexBAuSplitter, au_has_idr};
use whatsapp_rust::voip::VideoFrame;

/// Reference encode parameters (H.264 Constrained Baseline, WebCodecs `avc1.42E01F` equivalent).
const VIDEO_SIZE: &str = "640x480";
const VIDEO_FPS: u32 = 15;
const VIDEO_BITRATE: &str = "500k";
/// Keyframe cadence in frames (2 s at 15 fps): the recovery bound after any dropped AU.
const GOP_FRAMES: u32 = 30;
/// Outbound AU backlog before the keyframe-aware dropper kicks in (~2 s of video).
const SOURCE_CHANNEL_CAP: usize = 30;
/// stdout pipe read granularity.
const READ_CHUNK: usize = 32 * 1024;
/// A sink write slower than this sheds frames instead of stalling the call's forwarder.
const SINK_WRITE_TIMEOUT: Duration = Duration::from_millis(200);

pub struct VideoOpts {
    pub input: VideoInput,
    pub sink: VideoSinkMode,
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
    pub fn from_env() -> Self {
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
        Self { input, sink }
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
fn input_args(input: &VideoInput) -> Result<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let fps = VIDEO_FPS.to_string();
    Ok(match input {
        // `-re` paces the synthetic source at realtime; unthrottled, lavfi encodes as fast as the
        // CPU allows and floods the call's bounded channel with dropped GOPs.
        VideoInput::TestSrc => s(&[
            "-re",
            "-f",
            "lavfi",
            "-i",
            &format!("testsrc2=size={VIDEO_SIZE}:rate={VIDEO_FPS}"),
        ]),
        VideoInput::Media(path) => {
            let mut v = s(&["-re", "-stream_loop", "-1", "-i", path]);
            // Normalize any input to the call's fixed geometry/rate.
            v.extend(s(&[
                "-vf",
                &format!(
                    "scale={W}:{H}:force_original_aspect_ratio=decrease,\
                     pad={W}:{H}:(ow-iw)/2:(oh-ih)/2,fps={VIDEO_FPS}",
                    W = 640,
                    H = 480
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
                    VIDEO_SIZE,
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
                    VIDEO_SIZE,
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
                    VIDEO_SIZE,
                    "-i",
                    &format!("video={dev}"),
                ])
            } else {
                bail!("no webcam input mapping for this OS — use WA_VIDEO_INPUT=testsrc or a file");
            }
        }
    })
}

fn spawn_ffmpeg_encoder(input: &VideoInput) -> Result<Child> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-hide_banner", "-loglevel", "error"]);
    cmd.args(input_args(input)?);
    // repeat-headers=1: SPS/PPS ride every IDR, so a peer joining mid-stream (upgrade) or
    // recovering from a drop can always re-sync. aud=insert: the AU delimiter the splitter cuts on.
    cmd.args([
        "-c:v",
        "libx264",
        "-profile:v",
        "baseline",
        "-level",
        "3.1",
        "-pix_fmt",
        "yuv420p",
        "-preset",
        "ultrafast",
        "-tune",
        "zerolatency",
        "-g",
        &GOP_FRAMES.to_string(),
        "-keyint_min",
        &GOP_FRAMES.to_string(),
        "-sc_threshold",
        "0",
        "-b:v",
        VIDEO_BITRATE,
        "-maxrate",
        VIDEO_BITRATE,
        "-bufsize",
        "250k",
        "-x264-params",
        "repeat-headers=1",
        "-bsf:v",
        "h264_metadata=aud=insert",
        "-an",
        "-f",
        "h264",
        "pipe:1",
    ]);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        // stderr inherited: encoder errors (missing camera, busy device) reach the user directly.
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    cmd.spawn().context("spawn ffmpeg")
}

/// Spawn the ffmpeg encoder and return the channel of complete H.264 AUs. The receiver plugs
/// straight into the library (`VideoSource` blanket impl on `Receiver<Vec<u8>>`).
pub async fn spawn_video_source(opts: &VideoOpts) -> Result<async_channel::Receiver<Vec<u8>>> {
    ensure_tool("ffmpeg").await?;
    let mut child = spawn_ffmpeg_encoder(&opts.input)?;
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
        let (mut made, mut dropped) = (0u64, 0u64);
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break, // encoder EOF (camera unplugged, file without loop)
                Ok(n) => {
                    splitter.push(&buf[..n], &mut aus);
                    for au in aus.drain(..) {
                        made += 1;
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
                        match tx.try_send(au) {
                            Ok(()) => {}
                            Err(async_channel::TrySendError::Full(_)) => {
                                dropped += 1;
                                dropping = true;
                            }
                            // Call ended: stop the encoder.
                            Err(async_channel::TrySendError::Closed(_)) => return,
                        }
                    }
                    if made.is_multiple_of(300) && dropped > 0 {
                        info!("🎥 video source: {made} AUs, {dropped} dropped (backpressure)");
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
    Ok(rx)
}

fn spawn_ffplay(tag: &str) -> Result<Child> {
    let mut cmd = Command::new("ffplay");
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-fflags",
        "nobuffer",
        "-flags",
        "low_delay",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        "-framedrop",
        "-window_title",
        &format!("WA Video {tag}"),
        "-f",
        "h264",
        "-i",
        "pipe:0",
    ]);
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
    let (tx, rx) = async_channel::bounded::<VideoFrame>(SOURCE_CHANNEL_CAP);
    let tag = tag.to_string();

    tokio::spawn(async move {
        let mut target: Option<SinkTarget> = None;
        let mut dropping = false;
        let mut last_orientation = 0u8;
        while let Ok(frame) = rx.recv().await {
            if frame.orientation != last_orientation {
                last_orientation = frame.orientation;
                // ffplay can't rotate a live pipe; surface it so the user can tilt their head :)
                info!(
                    "🎥 peer rotated its camera: {}°",
                    frame.orientation as u32 * 90
                );
            }
            // Lazy-open on the first frame.
            if target.is_none() {
                target = Some(if use_window {
                    match spawn_ffplay(&tag) {
                        Ok(child) => SinkTarget::Window(child),
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
                if n.is_multiple_of(150) {
                    info!("🎥 {n} AUs piped through the video plumbing ({}s)", n / 15);
                }
            }
        }
    }
}
