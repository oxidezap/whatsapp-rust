//! Does an offer's `<voip_settings>` load the settings the engine says are missing?
//!
//! `message_bu Application settings not loaded` appears on every incoming offer
//! this harness has driven, and `getVoipParam("options.*")` answers empty. The
//! standing hypothesis was that both are one fact: the engine reads its settings
//! from the `voip_settings` blob that rides on call stanzas, and a run that never
//! receives a real one never has them.
//!
//! This asks it directly, rather than inferring it from a log line. Read
//! `getVoipParam` before the offer, hand the engine an offer that carries a
//! settings blob, read it again, and print both.
//!
//! ```sh
//! cargo run --release --example settings_probe                # the current capture
//! cargo run --release --example settings_probe -- JgwtTQVeWPm  # any id the catalog carries
//! ```
use base64::Engine as _;
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Server;
use wacore_binary::{Jid, Node, marshal};

const CALL_ID: &str = "0102030405060708";

/// The blob `whatsapp-rust` builds for a standard-Opus answer. The keys are the
/// ones `getVoipParam` is asked for elsewhere in WhatsApp Web, which is what
/// made the hypothesis worth testing: `options.*` on both sides.
const SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false","caller_timeout":"45"}}"#;

/// Paths WhatsApp Web's own modules ask for, read out of the bundle.
///
/// `caller_timeout` is in the blob above and the other two are not, on purpose:
/// if the blob is the source, the first should change and the rest should not.
const PARAMS: &[&str] = &[
    "options.caller_timeout",
    "options.video_brightness_setting",
    "options.video_sharpening_setting",
];

/// The identity shape that reaches furthest, measured by `voip_flow`: LID in the
/// stanza and LID in the peer argument. The other three either fail to convert
/// or have `call-creator` stripped.
const CALLER_LID: &str = "11223344556677";

fn offer_stanza(caller: &Jid, now: u64, with_settings: bool) -> Node {
    let mut children = vec![
        NodeBuilder::new("offer")
            .children([
                NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "16000")
                    .build(),
                NodeBuilder::new("net").attr("medium", "3").build(),
                NodeBuilder::new("encopt").attr("keygen", "2").build(),
            ])
            .build(),
    ];
    if with_settings {
        // `uncompressed="1"` marks the blob as plain; without it the engine
        // reports "unexpected compressed voip params".
        children.push(
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(SETTINGS.to_vec())
                .build(),
        );
    }

    NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", CALL_ID)
        .attr("call-creator", caller.with_device(1))
        .attr("t", now.to_string())
        .children(children)
        .build()
}

/// Every `PARAMS` entry, as the engine answers it right now.
fn read_params(r: &mut Runtime) -> Vec<(String, String)> {
    PARAMS
        .iter()
        .map(|path| {
            let answer = r.call_embind("getVoipParam", &[Value::Str((*path).into())]);
            r.refuel();
            let shown = match answer {
                Ok(Value::Str(s)) if s.is_empty() => "\"\" (empty)".to_string(),
                Ok(Value::Str(s)) => format!("{s:?}"),
                Ok(other) => format!("{other:?}"),
                Err(_) => "trap".to_string(),
            };
            ((*path).to_string(), shown)
        })
        .collect()
}

fn print_params(when: &str, params: &[(String, String)]) {
    println!("  getVoipParam {when}:");
    for (path, value) in params {
        println!("    {path:<38} {value}");
    }
}

fn main() -> anyhow::Result<()> {
    let which = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "S_ivh1PriOA".into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    println!(
        "{} ({} bytes)\n",
        entry.path.file_name().unwrap_or_default().to_string_lossy(),
        bytes.len()
    );

    // Run it twice: once handing the engine a settings blob and once not. A
    // difference between the two is the hypothesis; no difference in either
    // direction rules the blob out as the source, which a single run cannot do.
    for with_settings in [false, true] {
        println!(
            "=== offer {} a <voip_settings> blob",
            if with_settings { "carrying" } else { "without" }
        );

        let mut r = Runtime::instantiate(&bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.run_ctors()?;
        r.attach_log_ring(1 << 20)?;

        let init = r.call_embind(
            "initVoipStack",
            &[
                Value::Str("15550002222@c.us".into()),
                Value::Str("15550002222:0@c.us".into()),
                Value::Str("99887766554433:0@lid".into()),
            ],
        );
        r.refuel();
        if init.as_ref().ok().and_then(|value| value.as_int()) != Some(0) {
            println!("  media stack did not come up: {init:?}\n");
            continue;
        }
        println!("  initVoipStack -> 0");

        let before = read_params(&mut r);

        let caller = Jid::new(CALLER_LID, Server::Lid);
        let now = r.virtual_unix_time();
        let payload = base64::engine::general_purpose::STANDARD.encode(marshal::marshal(
            &offer_stanza(&caller, now, with_settings),
        )?);

        let mark = r.engine_log().len();
        let outcome = r.call_embind(
            "handleIncomingSignalingOffer",
            &[
                Value::Str(payload),
                Value::Str("web".into()),
                Value::Str("2.3000.0".into()),
                Value::Str(now.to_string()),
                Value::Str(now.to_string()),
                Value::Bool(false),
                Value::Bool(true),
                Value::Str(caller.to_string()),
                Value::Bytes(Vec::new()),
            ],
        );
        r.refuel();
        r.settle(std::time::Duration::from_secs(5));

        let lines = r.engine_log_from(mark);
        let after = read_params(&mut r);

        println!(
            "  handleIncomingSignalingOffer -> {}, {} log lines",
            match &outcome {
                Ok(value) => format!("{value:?}"),
                Err(_) => "trap".to_string(),
            },
            lines.len()
        );
        print_params("before the offer", &before);
        print_params("after the offer", &after);

        let changed: Vec<&(String, String)> = before
            .iter()
            .zip(&after)
            .filter(|(b, a)| b.1 != a.1)
            .map(|(_, a)| a)
            .collect();
        println!(
            "  -> {} of {} parameters changed",
            changed.len(),
            PARAMS.len()
        );

        // The line the hypothesis is about, quoted rather than summarised.
        for line in lines.iter().filter(|l| {
            l.contains("settings") || l.contains("Settings") || l.contains("voip param")
        }) {
            println!("  engine: {}", line.trim());
        }
        println!();
    }

    Ok(())
}
