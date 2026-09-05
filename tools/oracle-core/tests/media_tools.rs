//! The WASI media modules run as the command-line tools they are.
//!
//! These are WhatsApp's own media validators. What they accept and reject is
//! the specification a Rust implementation has to match, so their verdicts are
//! asserted here rather than described.

use oracle_core::{Catalog, Runtime};

/// A real 16x16 H.264 clip, generated with ffmpeg from a synthetic test
/// pattern. Hand-built MP4s get as far as the H.264 parser and no further —
/// the tools validate the elementary stream, not just the container.
const TINY_MP4: &[u8] = include_bytes!("fixtures/tiny.mp4");

/// What a tool run produced: exit code, streams, and the filesystem it left.
struct ToolRun {
    code: i32,
    stdout: String,
    stderr: String,
    files: std::collections::BTreeMap<String, Vec<u8>>,
}

/// Runs a tool and returns everything it produced.
fn tool_with_files(id: &str, args: &[&str], files: &[(&str, Vec<u8>)]) -> Option<ToolRun> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(id).ok()?;
    let bytes = std::fs::read(&entry.path).ok()?;

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.set_args(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
    for (path, contents) in files {
        runtime.add_file(path, contents.clone());
    }

    let code = runtime.run_main().expect("entry point should run");
    Some(ToolRun {
        code,
        stdout: runtime.wasi().stdout_text(),
        stderr: runtime.wasi().stderr_text(),
        files: runtime.wasi().files.clone(),
    })
}

fn tool(id: &str, args: &[&str], files: &[(&str, Vec<u8>)]) -> Option<(i32, String, String)> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(id).ok()?;
    let bytes = std::fs::read(&entry.path).ok()?;

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.set_args(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>());
    for (path, contents) in files {
        runtime.add_file(path, contents.clone());
    }

    let code = runtime.run_main().expect("entry point should run");
    Some((
        code,
        runtime.wasi().stdout_text(),
        runtime.wasi().stderr_text(),
    ))
}

macro_rules! run_or_skip {
    ($id:expr, $args:expr, $files:expr) => {
        match tool($id, $args, $files) {
            Some(result) => result,
            None => {
                eprintln!("skipping: {} unavailable (set WA_WASM_DIR)", $id);
                return;
            }
        }
    };
}

/// A 1x1 lossy WebP: RIFF header, `VP8 ` chunk, minimal keyframe.
fn webp_1x1() -> Vec<u8> {
    let vp8: [u8; 10] = [0x30, 0x01, 0x00, 0x9d, 0x01, 0x2a, 0x01, 0x00, 0x01, 0x00];
    let mut file = Vec::new();
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&((4 + 8 + vp8.len()) as u32).to_le_bytes());
    file.extend_from_slice(b"WEBP");
    file.extend_from_slice(b"VP8 ");
    file.extend_from_slice(&(vp8.len() as u32).to_le_bytes());
    file.extend_from_slice(&vp8);
    file
}

fn mp4_box(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut out = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
    out.extend_from_slice(kind);
    out.extend_from_slice(payload);
    out
}

/// An `ftyp` box followed by whatever else is supplied.
fn mp4_with(rest: &[u8]) -> Vec<u8> {
    let mut ftyp = b"isom".to_vec();
    ftyp.extend_from_slice(&512u32.to_be_bytes());
    ftyp.extend_from_slice(b"isomiso2avc1mp41");

    let mut file = mp4_box(b"ftyp", &ftyp);
    file.extend_from_slice(rest);
    file
}

#[test]
fn webpcheck_reports_canvas_dimensions() {
    let (code, stdout, stderr) = run_or_skip!(
        "rogm88TRRiw",
        &["input.webp"],
        &[("input.webp", webp_1x1())]
    );

    assert_eq!(code, 0, "webpcheck should accept a valid WebP: {stderr}");
    // The tool prints a WebPFileInfo struct; the dimensions are the payload.
    assert!(
        stdout.contains("canvas_width: 1") && stdout.contains("canvas_height: 1"),
        "unexpected output: {stdout}"
    );
}

#[test]
fn webpcheck_rejects_a_non_webp() {
    let (code, _stdout, _stderr) = run_or_skip!(
        "rogm88TRRiw",
        &["input.webp"],
        &[("input.webp", b"this is not a webp file".to_vec())]
    );
    assert_ne!(code, 0, "garbage should not be accepted as WebP");
}

#[test]
fn mp4check_rejects_an_empty_moov() {
    let file = mp4_with(&mp4_box(b"moov", &[]));
    let (code, stdout, _stderr) =
        run_or_skip!("ayqr5HQtlkb", &["mp4check", "in.mp4"], &[("in.mp4", file)]);

    assert_ne!(code, 0, "an empty moov is not a valid MP4");
    // The offset names where the tool stopped: immediately after the 32-byte
    // ftyp. That precision is what makes it usable as an oracle.
    assert!(
        stdout.contains("offset Some(32)"),
        "expected the failure to be located at the moov: {stdout}"
    );
}

#[test]
fn mp4check_reports_usage_without_a_subcommand() {
    let (code, _stdout, stderr) = run_or_skip!("ayqr5HQtlkb", &[], &[]);

    assert_ne!(code, 0);
    // Guards the argument plumbing itself: the tool has to actually see argv.
    // A host that dropped arguments produced exactly this message too, so the
    // subcommand list is what distinguishes "parsed and rejected" from
    // "received nothing".
    assert!(
        stderr.contains("mp4check") && stderr.contains("mp4repair"),
        "expected the subcommand list: {stderr}"
    );
}

/// Arguments must reach the guest. This is the regression that hid behind an
/// exit code for a long time: the host reported success while writing them into
/// a stale memory window, so every tool behaved as if invoked bare.
#[test]
fn arguments_reach_the_guest() {
    let (code, stdout, _stderr) = run_or_skip!("rogm88TRRiw", &["--help"], &[]);

    assert_eq!(code, 0, "--help should succeed");
    assert!(
        stdout.contains("USAGE") || stdout.contains("Usage"),
        "expected help text: {stdout}"
    );
}

/// The full round trip: repair writes a file, and the checker accepts what it
/// wrote. This is the first test that exercises the in-memory filesystem's
/// *write* path — everything before it only read.
#[test]
fn mp4repair_writes_a_file_the_checker_accepts() {
    let Some(run) = tool_with_files(
        "ayqr5HQtlkb",
        &["mp4repair", "in.mp4", "out.mp4"],
        &[("in.mp4", TINY_MP4.to_vec())],
    ) else {
        eprintln!("skipping: mp4 tools unavailable (set WA_WASM_DIR)");
        return;
    };

    assert_eq!(
        run.code, 0,
        "repair should accept a valid file: {}{}",
        run.stdout, run.stderr
    );

    let files = &run.files;
    let repaired = files
        .get("out.mp4")
        .unwrap_or_else(|| panic!("repair wrote no output; filesystem held {:?}", files.keys()))
        .clone();
    assert!(!repaired.is_empty(), "repaired file is empty");
    assert_eq!(&repaired[4..8], b"ftyp", "output is not an MP4");

    // Feed the output back in. A repair that produced something the tool's own
    // checker rejects would be worse than one that failed outright.
    let checked = tool_with_files(
        "ayqr5HQtlkb",
        &["mp4check", "r.mp4"],
        &[("r.mp4", repaired)],
    )
    .expect("second run");
    assert_eq!(
        checked.code, 0,
        "the checker rejected repair's own output: {}{}",
        checked.stdout, checked.stderr
    );
}

/// Repair rewrites the file rather than copying it: WhatsApp's tool moves the
/// moov box to the front so the clip can start playing before it has fully
/// downloaded, which is what its "needs to be streamified" warning refers to.
#[test]
fn mp4repair_streamifies_its_input() {
    let Some(run) = tool_with_files(
        "ayqr5HQtlkb",
        &["mp4repair", "in.mp4", "out.mp4"],
        &[("in.mp4", TINY_MP4.to_vec())],
    ) else {
        eprintln!("skipping: mp4 tools unavailable (set WA_WASM_DIR)");
        return;
    };
    let repaired = run.files.get("out.mp4").expect("output");

    assert_ne!(
        repaired.as_slice(),
        TINY_MP4,
        "output is byte-identical, so nothing was rewritten"
    );
    assert!(
        position_of(repaired, b"moov") < position_of(repaired, b"mdat"),
        "moov should precede mdat after streamifying"
    );
}

fn position_of(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap_or(usize::MAX)
}

/// The checker reports what it found, not just whether it passed — which is
/// what makes it usable as a specification for a Rust implementation.
#[test]
fn mp4check_reports_stream_properties() {
    let (code, stdout, stderr) = run_or_skip!(
        "ayqr5HQtlkb",
        &["mp4check", "in.mp4"],
        &[("in.mp4", TINY_MP4.to_vec())]
    );

    assert_eq!(code, 0, "fixture should be valid: {stderr}");
    for expected in ["H.264", "16 x 16", "MP4 file consistency: OK"] {
        assert!(
            stdout.contains(expected),
            "expected `{expected}` in the report: {stdout}"
        );
    }
}

/// Elementary streams, extracted from the same clip with ffmpeg. `mp4mux` takes
/// these rather than a container.
const TINY_H264: &[u8] = include_bytes!("fixtures/tiny.h264");
const TINY_AAC: &[u8] = include_bytes!("fixtures/tiny.aac");

/// Muxing video alone, and then checking the result with the sibling tool.
#[test]
fn mp4mux_builds_a_playable_file_from_a_video_stream() {
    let Some(run) = tool_with_files(
        "ayqr5HQtlkb",
        &[
            "mp4mux", "--video", "v.h264", "--output", "out.mp4", "--fps", "1",
        ],
        &[("v.h264", TINY_H264.to_vec())],
    ) else {
        eprintln!("skipping: mp4 tools unavailable (set WA_WASM_DIR)");
        return;
    };

    assert_eq!(run.code, 0, "mux failed: {}{}", run.stdout, run.stderr);
    assert!(
        run.stdout.contains("Video input stream is H.264"),
        "mux did not recognise the stream: {}",
        run.stdout
    );

    let muxed = run
        .files
        .get("out.mp4")
        .expect("mux wrote no output")
        .clone();
    let checked = tool_with_files("ayqr5HQtlkb", &["mp4check", "m.mp4"], &[("m.mp4", muxed)])
        .expect("check run");
    assert_eq!(
        checked.code, 0,
        "the checker rejected mux's own output: {}{}",
        checked.stdout, checked.stderr
    );
    assert!(
        checked.stdout.contains("H.264"),
        "expected a video track in the muxed file: {}",
        checked.stdout
    );
}

/// Muxing audio alongside video. The audio stream dominates the size here, so
/// the result is also evidence that both inputs were actually consumed rather
/// than one being silently ignored.
#[test]
fn mp4mux_combines_audio_and_video() {
    let Some(run) = tool_with_files(
        "ayqr5HQtlkb",
        &[
            "mp4mux", "--video", "v.h264", "--audio", "a.aac", "--output", "out.mp4", "--fps", "1",
        ],
        &[("v.h264", TINY_H264.to_vec()), ("a.aac", TINY_AAC.to_vec())],
    ) else {
        eprintln!("skipping: mp4 tools unavailable (set WA_WASM_DIR)");
        return;
    };

    assert_eq!(run.code, 0, "mux failed: {}{}", run.stdout, run.stderr);
    assert!(
        run.stdout.contains("Audio input stream is AAC"),
        "audio input was not recognised: {}",
        run.stdout
    );

    let with_audio = run.files.get("out.mp4").expect("output").len();

    // The same mux without audio, for comparison.
    let video_only = tool_with_files(
        "ayqr5HQtlkb",
        &[
            "mp4mux", "--video", "v.h264", "--output", "out.mp4", "--fps", "1",
        ],
        &[("v.h264", TINY_H264.to_vec())],
    )
    .expect("video-only run")
    .files
    .get("out.mp4")
    .expect("output")
    .len();

    assert!(
        with_audio > video_only,
        "adding audio did not grow the file ({with_audio} vs {video_only})"
    );
}

/// The MP4 core, which is a different program from the MP4 utils above.
///
/// `ayqr5HQtlkb` is the C++ tool suite; `9Nbh3eMuVjD` is
/// `libmp4operations-rs`, WhatsApp's Rust MP4 implementation, and it ships its
/// own `clap` command line. It was the one module in the lock that nothing
/// exercised, which is also why "`_start` exits 71 before reading argv" stayed
/// on the open-work list after the WASI window bug that caused it was fixed:
/// no test would have noticed either way.
#[test]
fn the_mp4_core_reads_its_arguments() {
    let (code, stdout, stderr) = run_or_skip!("9Nbh3eMuVjD", &["--help"], &[]);

    // Reaching `clap`'s help means `_start` ran, `args_sizes_get` and
    // `args_get` answered consistently, and the guest parsed what it was
    // handed. An entry point that exits 71 from its panic handler — which is
    // what a stale WASI memory window produced — gets to none of this.
    assert_eq!(code, 0, "--help should succeed: {stderr}");
    assert!(
        stdout.contains("check") && stdout.contains("mediautils"),
        "expected the subcommand list, got: {stdout}"
    );
}

#[test]
fn the_mp4_core_accepts_a_real_clip_and_rejects_garbage() {
    let (code, stdout, stderr) = run_or_skip!(
        "9Nbh3eMuVjD",
        &["mediautils", "mp4check", "in.mp4"],
        &[("in.mp4", TINY_MP4.to_vec())]
    );
    assert_eq!(code, 0, "a real clip should check out: {stdout}{stderr}");
    assert!(
        stdout.contains("MP4 file consistency: OK"),
        "unexpected verdict: {stdout}"
    );

    let (code, _stdout, stderr) = run_or_skip!(
        "9Nbh3eMuVjD",
        &["mediautils", "mp4check", "in.mp4"],
        &[("in.mp4", b"not an mp4 at all, just text".to_vec())]
    );
    assert_ne!(code, 0, "garbage should not check out");
    // The tool names the error rather than just failing, and the number is
    // WhatsApp's own: a Rust implementation has to report the same one.
    assert!(
        stderr.contains("239: Unknown MP4 box topology"),
        "expected the wamedia error code, got: {stderr}"
    );
}

/// The mimetype classifier, which is what the client calls before it decides a
/// file is a video at all.
#[test]
fn the_mp4_core_classifies_by_content_not_by_name() {
    let (code, stdout, stderr) = run_or_skip!(
        "9Nbh3eMuVjD",
        &["classify", "anything.bin"],
        &[("anything.bin", TINY_MP4.to_vec())]
    );

    assert_eq!(code, 0, "classify should succeed: {stderr}");
    assert!(
        stdout.contains(r#"Mimetype: Some("video/mp4")"#)
            && stdout.contains(r#"Extension: Some("mp4")"#),
        "a real clip under an unrelated name should still classify: {stdout}"
    );
}
