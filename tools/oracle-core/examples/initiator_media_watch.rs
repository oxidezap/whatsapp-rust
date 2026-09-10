//! Initiator-side media watch: does the pinned engine emit any transport
//! bytes (`call_sendto`) or decoded frames (`renderVideoFrame_js`) for an
//! outgoing video call that has no active peer?
//!
//! All identities are fictitious. Execute in release: debug Cranelift
//! compilation is too slow for meaningful engine deadlines.
//!
//! ```sh
//! cargo run --release -p oracle-core --example initiator_media_watch
//! ```
//!
//! `ENGINE` overrides the pinned module (default `JgwtTQVeWPm`).

use anyhow::{Result, bail};
use oracle_core::{Catalog, MediaStream, MediaWatch, Runtime, ThreadPolicy, Value};
use sha2::{Digest, Sha256};

const ENGINE: &str = "JgwtTQVeWPm";
const ENGINE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

const SELF: &str = "15550002222@c.us";
const SELF_DEVICE: &str = "15550002222:0@c.us";
const SELF_LID: &str = "99887766554433:0@lid";
const PEER_LID: &str = "11223344556677@lid";
const PEER_DEVICE: &str = "11223344556677:0@lid";
const CALL_ID: &str = "0011223344556677";
const TC_TOKEN: [u8; 32] = [0xA5; 32];

fn main() -> Result<()> {
    let which = std::env::var("ENGINE").unwrap_or_else(|_| ENGINE.into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    let sha = hex::encode(Sha256::digest(&bytes));
    if which == ENGINE {
        anyhow::ensure!(sha == ENGINE_SHA);
    }
    println!("engine: {which} sha256={sha}");

    let mut r = Runtime::instantiate(&bytes)?;
    // Transport egress is `call_sendto(fd, buf, len, addr)`: watch the payload.
    r.watch_media([MediaWatch::new(
        "env",
        "call_sendto",
        MediaStream::Video,
        1,
        2,
    )?])?;
    r.set_thread_policy(ThreadPolicy::Spawn);
    r.set_main_thread_registration(true);
    r.run_ctors()?;
    r.attach_log_ring(4 << 20)?;
    let args = [SELF, SELF_DEVICE, SELF_LID].map(|v| Value::Str(v.to_owned()));
    let init = r.call_embind("initVoipStack", &args);
    r.refuel();
    if init.as_ref().ok().and_then(|v| v.as_int()) != Some(0) {
        bail!("initVoipStack failed: {init:?}");
    }

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut started = false;
    while std::time::Instant::now() < deadline {
        if r.engine_log()
            .iter()
            .any(|l| l.contains("call_event_proc resumed"))
        {
            started = true;
            break;
        }
        r.process_queued_calls();
        r.refuel();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    if !started {
        bail!("event thread never announced itself");
    }

    let outcome = r.call_embind(
        "startVoipCall",
        &[
            Value::Str(PEER_LID.into()),
            Value::StringList(vec![PEER_DEVICE.into()]),
            Value::Str(CALL_ID.into()),
            Value::Bool(true),
            Value::Str(PEER_LID.into()),
            Value::Bool(false),
            Value::Bytes(TC_TOKEN.to_vec()),
        ],
    );
    r.refuel();
    println!("startVoipCall -> {outcome:?}");
    r.settle(std::time::Duration::from_secs(8));
    r.refuel();

    let obs = r.take_media_observations()?;
    println!("media observations on env::call_sendto: {}", obs.len());
    for (i, o) in obs.iter().take(8).enumerate() {
        println!("  [{i}] {o:?}");
    }
    println!("signaling stanzas: {}", r.signaling()?.len());
    for (name, count) in r.stubs_called() {
        if name.contains("capture")
            || name.contains("sendto")
            || name.contains("render")
            || name.contains("playback")
        {
            println!("stub {name}: {count}");
        }
    }
    Ok(())
}
