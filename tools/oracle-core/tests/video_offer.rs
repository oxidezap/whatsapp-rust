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

#[test]
#[ignore = "real guest threads and captured JgwtTQVeWPm.wasm; missing captures fail"]
fn video_offer_matches_the_vendor_engine() -> Result<()> {
    let _serial = common::threaded_guard();
    let bytes = common::capture(CAPTURE)?.with_context(|| format!("missing {CAPTURE}"))?;
    assert_eq!(hex::encode(Sha256::digest(&bytes)), CAPTURE_SHA);

    let mut runtime = None;
    for _ in 0..6 {
        let mut candidate = Runtime::instantiate(&bytes)?;
        candidate.set_thread_policy(ThreadPolicy::Spawn);
        candidate.set_main_thread_registration(true);
        candidate.run_ctors()?;
        candidate.attach_log_ring(4 << 20)?;
        let init = candidate.call_embind(
            "initVoipStack",
            &[
                Value::Str("15550002222@c.us".into()),
                Value::Str("15550002222:0@c.us".into()),
                Value::Str("99887766554433:0@lid".into()),
            ],
        );
        candidate.refuel();
        if init.as_ref().ok().and_then(|value| value.as_int()) == Some(0) {
            runtime = Some(candidate);
            break;
        }
    }
    let mut runtime = runtime.context("initVoipStack never returned 0")?;

    let mut started = false;
    for _ in 0..3 {
        let outcome = runtime.call_embind(
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
        runtime.settle(std::time::Duration::from_secs(8));
        runtime.refuel();
        if outcome.as_ref().ok().and_then(|value| value.as_int()) == Some(0)
            && !runtime.signaling()?.is_empty()
        {
            started = true;
            break;
        }
    }
    assert!(started, "the engine should emit a video offer");

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
    let sent = runtime.signaling()?;
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
