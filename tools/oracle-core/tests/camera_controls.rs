//! Captured camera controls while ringing, not an established media-call oracle.

use anyhow::{Context, Result};
use oracle_core::{Runtime, ThreadPolicy, Value, abi, derive::function_body_sha256, patch};
use serde::Deserialize;
use sha2::{Digest, Sha256};

mod common;

const CAPTURE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

fn capture() -> Result<Vec<u8>> {
    let bytes = common::capture("JgwtTQVeWPm")?
        .context("this test requires JgwtTQVeWPm.wasm; set WA_WASM_DIR")?;
    anyhow::ensure!(hex::encode(Sha256::digest(&bytes)) == CAPTURE_SHA);
    Ok(bytes)
}

#[test]
#[ignore = "requires captured JgwtTQVeWPm and S_ivh1PriOA; missing captures fail"]
fn captured_date_now_returns_wall_time_not_a_boolean() -> Result<()> {
    for (id, capture_sha, function, slot, body_sha) in [
        (
            "JgwtTQVeWPm",
            CAPTURE_SHA,
            10363,
            7708,
            "5563de8f88fefc7c6f88a08ec53d9d1ff918c62e88b1b33a79d00e07f6122742",
        ),
        (
            "S_ivh1PriOA",
            "e55e43babf85e2c0fc76ec65dcb8d47beba0f58b03b135b37a5aaaff7fe70e2f",
            10650,
            7923,
            "3172c2c8addc54ce5ac6f738f98381e86cd8366f4fe4abdc04ed00e8f39a0f7b",
        ),
    ] {
        let bytes =
            common::capture(id)?.with_context(|| format!("missing {id}; set WA_WASM_DIR"))?;
        assert_eq!(hex::encode(Sha256::digest(&bytes)), capture_sha);
        assert_eq!(function_body_sha256(&bytes, function)?, body_sha);
        assert!(abi::table_slots_of(&bytes, function)?.contains(&slot));
        let mut runtime = Runtime::instantiate(&bytes)?;
        runtime.run_ctors()?;
        let mut previous = oracle_core::emscripten::EPOCH_MS;
        for _ in 0..3 {
            let result = runtime.call_table(slot, &[])?;
            let now = result[0].f64().context("Date.now must return an f64")?;
            assert!(now > previous, "wall time must advance, got {now}");
            assert!(now < oracle_core::emscripten::EPOCH_MS + 1000.0);
            previous = now;
        }
        assert!(runtime.stubs_called().is_empty());
        eprintln!("executed {id} Date.now callback three times: {previous}");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct CallInfo {
    call_state: u32,
    video_enabled: bool,
    participants: Vec<Participant>,
}

#[derive(Debug, Deserialize)]
struct Participant {
    is_self: bool,
    video_state: u32,
    video_decode_started: bool,
    video_render_started: bool,
}

fn info(runtime: &mut Runtime) -> Result<CallInfo> {
    runtime.refuel();
    let value = runtime.call_embind("getCallInfo", &[])?;
    serde_json::from_str(value.as_str().context("getCallInfo returned no string")?)
        .context("parsing call info")
}

#[test]
#[ignore = "real guest threads and captured JgwtTQVeWPm.wasm; missing captures fail"]
fn ringing_camera_controls_reach_send_only_pause_and_resume() -> Result<()> {
    let _serial = common::threaded_guard();
    let bytes = capture()?;
    for (index, hash) in [
        (
            12431,
            "cdf5093b715e17e58623cc86b1f3444f3d239710d890ea115ac3596eb6430574",
        ),
        (
            6814,
            "b0226a3e5240b74a54bc9aa0379457540a21539b422de7cac40a7916800c5b26",
        ),
        (
            12416,
            "3cc4bb73ee5076ac2a900ac351af414c5e093276eba99f9263d33aaa14d05714",
        ),
        (
            10365,
            "85dc769226de44834ac972e7fcea1f4a8e47ab66ffbe72f4aecdb2eaac3570df",
        ),
    ] {
        assert_eq!(function_body_sha256(&bytes, index)?, hash);
    }
    for (index, anchor) in [
        (6817, "call_resume_video"),
        (12433, "call_video_turn_camera_on"),
    ] {
        assert!(
            abi::find_string_refs(&bytes, anchor)?
                .iter()
                .any(|s| s.referenced_by.contains(&index))
        );
    }
    let (bytes, _) = patch::instrument(
        &bytes,
        &patch::Plan {
            entry: vec![12431, 12433, 12416],
            value_entry: vec![
                (6814, 1),
                (6814, 2),
                (6814, 3),
                (6817, 1),
                (6817, 2),
                (6817, 3),
            ],
            ..Default::default()
        },
    )?;
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.set_thread_policy(ThreadPolicy::Spawn);
    runtime.set_main_thread_registration(true);
    runtime.run_ctors()?;
    runtime.attach_log_ring(4 << 20)?;
    runtime.shared().watch_markers("env::on_call_event_js_sync");
    let initialized = runtime.call_embind(
        "initVoipStack",
        &[
            Value::Str("15550002222@c.us".into()),
            Value::Str("15550002222:0@c.us".into()),
            Value::Str("99887766554433:0@lid".into()),
        ],
    )?;
    assert_eq!(initialized.as_int(), Some(0));
    runtime.refuel();
    let started = runtime.call_embind(
        "startVoipCall",
        &[
            Value::Str("11223344556677@lid".into()),
            Value::StringList(vec!["11223344556677:0@lid".into()]),
            Value::Str("0011223344556677".into()),
            Value::Bool(true),
            Value::Str("11223344556677@lid".into()),
            Value::Bool(false),
            Value::Bytes(vec![0xa5; 32]),
        ],
    )?;
    assert_eq!(started.as_int(), Some(0));
    assert!(
        runtime
            .logs()
            .iter()
            .any(|line| line.contains("WasmTimestampCalibration"))
    );

    for (muted, local_state, expected_status, expected_markers) in [
        (
            true,
            6,
            70020,
            [(200000, 0), (200003, 1), (200004, 1), (200005, 6)],
        ),
        (
            false,
            1,
            0,
            [(200001, 0), (200006, 1), (200007, 1), (200008, 6)],
        ),
    ] {
        let before = runtime.shared().markers().len();
        runtime.refuel();
        let result = runtime.call_embind("setCallVideoMute", &[Value::Bool(muted)])?;
        assert_eq!(result.as_int(), Some(expected_status));
        assert_eq!(&runtime.shared().markers()[before..], &expected_markers);
        let info = info(&mut runtime)?;
        assert_eq!(info.call_state, 1, "this oracle covers Calling, not Active");
        assert!(info.video_enabled);
        assert_eq!(info.participants.len(), 2);
        let local = info
            .participants
            .iter()
            .find(|p| p.is_self)
            .context("no self participant")?;
        let peer = info
            .participants
            .iter()
            .find(|p| !p.is_self)
            .context("no peer participant")?;
        assert_eq!(local.video_state, local_state);
        assert_eq!(peer.video_state, 1);
        assert!(!peer.video_decode_started && !peer.video_render_started);
    }
    assert!(
        runtime
            .engine_log()
            .iter()
            .any(|line| line.contains("skip sending video msg in lonely state"))
    );
    assert!(!runtime.engine_log_overflowed()?);
    let signaling = runtime.signaling()?;
    assert!(!signaling.is_empty(), "origination must emit an offer");
    for sent in signaling {
        let node = wacore_binary::marshal::unmarshal_ref(&sent.stanza[1..])?;
        assert_eq!(
            node.tag, "offer",
            "no camera state is signaled while lonely"
        );
    }
    let stubs = runtime.stubs_called();
    for (symbol, _) in &stubs {
        assert!(
            [
                "env::on_call_event_js_sync",
                "env::query_browser_audio_processing_status_js_sync",
                "env::call_start_video_capture_js_sync",
                "env::call_stop_video_capture_js_sync",
                "env::emscripten_check_blocking_allowed",
                "env::get_persistent_directory_path_js",
            ]
            .contains(&symbol.as_str()),
            "unexpected host stub: {symbol}"
        );
    }
    assert!(
        stubs
            .iter()
            .any(|(name, count)| name == "env::call_stop_video_capture_js_sync" && *count == 1)
    );
    assert!(
        stubs
            .iter()
            .any(|(name, count)| name == "env::call_start_video_capture_js_sync" && *count == 2)
    );
    eprintln!(
        "executed JgwtTQVeWPm ringing camera stop/resume: direction=1, self=6->1, peer=1; remote decoder never started; stubs={stubs:?}"
    );
    Ok(())
}
