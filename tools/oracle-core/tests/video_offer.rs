//! A 1:1 video offer from the captured engine, byte-checked against our builder.
//!
//! Drives `startVoipCall` with the video flag set and reads the emitted
//! signaling stanza. The leading byte is the stream flag, so the node starts
//! at +1. All identities are fictitious.

mod common;

use anyhow::{Context, Result};
use oracle_core::{Runtime, ThreadPolicy, Value};
use sha2::{Digest, Sha256};
use wacore::stanza::call::{CAPABILITY_VIDEO_OFFER, OfferDeviceKey, OfferParams, build_offer};
use wacore_binary::jid::Server;
use wacore_binary::node::NodeContent;
use wacore_binary::{Jid, marshal};

const CAPTURE: &str = "JgwtTQVeWPm";
const CAPTURE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

// J's reader takes a wrapper around a native element. Names and string values
// are pj_str_t pairs in its intrusive attribute list, not serialized XML bytes.
fn read_video(runtime: &mut Runtime, attributes: &[(&str, &str)]) -> Result<(i32, Vec<u8>)> {
    let element = runtime.write_bytes(&[0; 96])?;
    let wrapper = runtime.write_bytes(&element.to_le_bytes())?;
    let result = runtime.write_bytes(&[0; 96])?;
    let sentinel = element + 24;
    let mut previous = sentinel;
    for (name, value) in attributes {
        let attribute = runtime.write_bytes(&[0; 48])?;
        let mut name_bytes = name.as_bytes().to_vec();
        name_bytes.push(0);
        let name_ptr = runtime.write_bytes(&name_bytes)?;
        runtime.write_bytes_at(attribute, &previous.to_le_bytes())?;
        runtime.write_bytes_at(previous + 4, &attribute.to_le_bytes())?;
        runtime.write_bytes_at(attribute + 8, &name_ptr.to_le_bytes())?;
        runtime.write_bytes_at(attribute + 16, &(name.len() as u64).to_le_bytes())?;
        let mut value_bytes = value.as_bytes().to_vec();
        value_bytes.push(0);
        let value_ptr = runtime.write_bytes(&value_bytes)?;
        runtime.write_bytes_at(attribute + 24, &value_ptr.to_le_bytes())?;
        runtime.write_bytes_at(attribute + 32, &(value.len() as u64).to_le_bytes())?;
        previous = attribute;
    }
    runtime.write_bytes_at(previous + 4, &sentinel.to_le_bytes())?;
    runtime.write_bytes_at(sentinel, &previous.to_le_bytes())?;
    let returned = runtime
        .call_table(
            9050,
            &[
                wasmtime::Val::I32(wrapper as i32),
                wasmtime::Val::I32(result as i32),
                wasmtime::Val::I32(1),
            ],
        )
        .with_context(|| {
            format!(
                "video attributes={attributes:?}; stubs={:?}",
                runtime.stubs_called()
            )
        })?;
    Ok((
        returned[0].i32().context("reader result")?,
        runtime.read(result, 96)?,
    ))
}

#[test]
fn group_video_codec_spellings_match_the_pinned_reader() -> Result<()> {
    let Some(bytes) = common::capture(CAPTURE)? else {
        eprintln!("skipping: {CAPTURE} unavailable (set WA_WASM_DIR)");
        return Ok(());
    };
    assert_eq!(hex::encode(Sha256::digest(&bytes)), CAPTURE_SHA);
    assert_eq!(
        oracle_core::derive::function_body_sha256(&bytes, 12707)?,
        "f1e11df4d2b0fe28c0c2132e2a9f9e4b946f2665c8acc93cfc2a07fe5cd452a4"
    );
    assert!(oracle_core::abi::table_slots_of(&bytes, 12707)?.contains(&9050));
    assert!(
        oracle_core::abi::find_string_refs(&bytes, "fill_video: unknown video encoding %s")?
            .iter()
            .any(|entry| entry.referenced_by.contains(&12707))
    );
    let _serial = common::threaded_guard();
    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.run_ctors()?;
    // Unknown encodings log through an engine thread that this reader-only test does not start.
    runtime.set_engine_log_level(0)?;
    for (encoder, code) in [("h.264", 1), ("vp8", 2), ("vp8/h.264", 3)] {
        let (accepted, video) = read_video(&mut runtime, &[("enc", encoder), ("dec", "H264")])?;
        assert_eq!(accepted, 1, "enc={encoder}");
        assert_eq!(u32::from_le_bytes(video[4..8].try_into()?), code);
    }
    for decoder in ["h264", "H264"] {
        let attributes = [
            ("enc", "h.264"),
            ("dec", decoder),
            ("screen_width", "0"),
            ("screen_height", "0"),
            ("device_orientation", "1"),
        ];
        let (accepted, baseline) = read_video(&mut runtime, &attributes)?;
        assert_eq!(accepted, 1, "dec={decoder}");
        assert_eq!(u32::from_le_bytes(baseline[4..8].try_into()?), 1);
        assert_eq!(u32::from_le_bytes(baseline[12..16].try_into()?), 1);
        assert_eq!(&baseline[28..28 + decoder.len()], decoder.as_bytes());
        let mut with_orientation = attributes.to_vec();
        with_orientation.push(("orientation", "3"));
        assert_eq!(
            read_video(&mut runtime, &with_orientation)?,
            (accepted, baseline)
        );
    }
    for encoder in ["h264", "H264"] {
        let (accepted, _) = read_video(&mut runtime, &[("enc", encoder), ("dec", "H264")])?;
        assert_eq!(accepted, 0, "enc={encoder}");
    }
    assert!(
        runtime.stubs_called().is_empty(),
        "reader used a stub: {:?}",
        runtime.stubs_called()
    );
    eprintln!("executed {CAPTURE}: enc spelling, both dec spellings and ignored orientation");
    Ok(())
}

#[test]
#[ignore = "real guest threads and captured JgwtTQVeWPm.wasm; missing captures fail"]
fn video_offer_matches_the_vendor_engine() -> Result<()> {
    let _serial = common::threaded_guard();
    let bytes = common::capture(CAPTURE)?.with_context(|| format!("missing {CAPTURE}"))?;
    assert_eq!(hex::encode(Sha256::digest(&bytes)), CAPTURE_SHA);
    eprintln!(
        "oracle capture={CAPTURE} capture_sha256={CAPTURE_SHA} bytes={}",
        bytes.len()
    );

    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.set_thread_policy(ThreadPolicy::Spawn);
    runtime.demand_strict_turns();
    runtime.set_main_thread_registration(true);
    runtime.run_ctors()?;
    runtime.attach_log_ring(4 << 20)?;
    let init = common::evidence::call(
        &mut runtime,
        "initVoipStack",
        &[
            Value::Str("15550002222@c.us".into()),
            Value::Str("15550002222:0@c.us".into()),
            Value::Str("99887766554433:0@lid".into()),
        ],
    );
    runtime.refuel();
    assert_eq!(
        init.as_ref().ok().and_then(|value| value.as_int()),
        Some(0),
        "oracle infrastructure failure: init={init:?} log={:?}",
        runtime.engine_log()
    );

    let outcome = common::evidence::call(
        &mut runtime,
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
    );
    runtime.refuel();
    assert_eq!(
        outcome.as_ref().ok().and_then(|value| value.as_int()),
        Some(0),
        "startVoipCall: {outcome:?}"
    );
    let sent = common::evidence::wait_for_signaling(
        &mut runtime,
        std::time::Duration::from_secs(8),
        "0011223344556677",
    )?;
    // The signaling callback precedes delivery of the last call event. Wait
    // for that event before comparing exact stub counters.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let call_events = |runtime: &Runtime| {
        runtime
            .stubs_called()
            .iter()
            .find(|(name, _)| name == "env::on_call_event_js_sync")
            .map_or(0, |(_, count)| *count)
    };
    while call_events(&runtime) < 3 {
        if let Err(error) = runtime.try_process_queued_calls() {
            common::evidence::record(&runtime);
            return Err(error);
        }
        if call_events(&runtime) >= 3 {
            break;
        }
        if std::time::Instant::now() >= deadline {
            common::evidence::record(&runtime);
            anyhow::bail!(
                "oracle Infrastructure: final call event deadline expired after {} call events",
                call_events(&runtime)
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    common::evidence::record(&runtime);
    common::evidence::check_log(
        runtime.engine_log_overflowed()?,
        runtime.memory_view_is_coherent(),
        !sent.is_empty(),
    )
    .unwrap_or_else(|kind| common::evidence::fail(kind, "video offer evidence"));

    let mut stubs = runtime.stubs_called();
    stubs.sort();
    // `get_persistent_directory_path_js` and `__syscall_stat64` are
    // implemented, not stubbed (the engine reads file-backed application
    // settings through them), so neither appears here. A changed
    // stub-dependent path must fail here, not bless new geometry.
    assert_eq!(
        stubs,
        [
            ("env::call_start_video_capture_js_sync".to_owned(), 1),
            ("env::emscripten_check_blocking_allowed".to_owned(), 1),
            ("env::on_call_event_js_sync".to_owned(), 3),
            (
                "env::query_browser_audio_processing_status_js_sync".to_owned(),
                4
            ),
        ],
        "a changed stub-dependent path must fail here, not bless new geometry"
    );
    let node = marshal::unmarshal_ref(&sent[0].stanza[1..])?;
    assert_eq!(node.tag, "offer");
    let children = node.children().context("offer children")?;
    let tags: Vec<String> = children.iter().map(|child| child.tag.to_string()).collect();
    assert_eq!(
        tags,
        [
            "privacy",
            "audio",
            "audio",
            "video",
            "net",
            "capability",
            "enc",
            "encopt"
        ]
    );
    let video = children
        .iter()
        .find(|child| child.tag == "video")
        .context("video child")?;
    let attr = |name: &str| {
        video
            .attrs()
            .optional_string(name)
            .as_deref()
            .map(str::to_owned)
    };
    assert_eq!(attr("enc").as_deref(), Some("h.264"));
    assert_eq!(attr("dec").as_deref(), Some("H264"));
    assert_eq!(attr("device_orientation").as_deref(), Some("0"));
    assert_eq!(attr("screen_width").as_deref(), Some("0"));
    assert_eq!(attr("screen_height").as_deref(), Some("0"));
    assert_eq!(attr("orientation"), None);

    let peer = Jid::new("11223344556677", Server::Lid);
    let device = peer.clone().with_device(0);
    let creator = Jid::new("99887766554433", Server::Lid);
    let rust_call = build_offer(&OfferParams {
        call_id: "0011223344556677",
        to: &peer,
        call_creator: &creator,
        device_keys: &[OfferDeviceKey {
            device_jid: device,
            ciphertext: vec![0x42; 32],
            enc_type: "pkmsg".to_string(),
        }],
        privacy_token: Some(&[0xa5; 32]),
        capability: Some(&CAPABILITY_VIDEO_OFFER),
        device_identity: None,
        id: Some("1"),
        multi_device: false,
        video: true,
        audio_rates: &["8000", "16000"],
    });
    let Some(NodeContent::Nodes(nodes)) = &rust_call.content else {
        anyhow::bail!("build_offer produced no children");
    };
    let rust_offer = nodes
        .iter()
        .find(|child| child.tag == "offer")
        .context("rust offer")?;
    let rust_video = match &rust_offer.content {
        Some(NodeContent::Nodes(inner)) => inner
            .iter()
            .find(|child| child.tag == "video")
            .context("rust video child")?,
        _ => anyhow::bail!("rust offer has no children"),
    };
    for name in [
        "enc",
        "dec",
        "device_orientation",
        "screen_width",
        "screen_height",
        "orientation",
    ] {
        assert_eq!(
            rust_video.attrs().optional_string(name).as_deref(),
            video.attrs().optional_string(name).as_deref(),
            "builder drift on video {name}"
        );
    }
    let engine_cap = children
        .iter()
        .find(|child| child.tag == "capability")
        .context("engine capability")?;
    let rust_cap = match &rust_offer.content {
        Some(NodeContent::Nodes(inner)) => inner
            .iter()
            .find(|child| child.tag == "capability")
            .context("rust capability child")?,
        _ => anyhow::bail!("rust offer has no children"),
    };
    let rust_cap_bytes = match &rust_cap.content {
        Some(NodeContent::Bytes(bytes)) => bytes.clone(),
        _ => anyhow::bail!("rust capability has no bytes"),
    };
    assert_eq!(
        Some(rust_cap_bytes.as_slice()),
        engine_cap.content_bytes(),
        "builder drift on video offer capability"
    );
    eprintln!("executed JgwtTQVeWPm.wasm video offer: engine and builder agree on zero screens");
    Ok(())
}
