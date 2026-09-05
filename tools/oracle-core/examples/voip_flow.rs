//! Drives a full incoming-call flow and reports everything the engine emits.
//!
//! This is the one example kept: it exercises a sequence the CLI cannot express
//! — bring the stack up, hand it an offer, then accept — and prints the engine's
//! own log alongside the host calls it made. Everything else that used to live
//! here is now either a subcommand (`oracle run`, `oracle call --threads --log`)
//! or a test.
//!
//! ```sh
//! cargo run --release --example voip_flow -- <path to JgwtTQVeWPm.wasm>
//! ```
use base64::Engine as _;
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Server;
use wacore_binary::{Jid, Node, marshal};

const CALL_ID: &str = "0102030405060708";
const SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false"}}"#;

/// The shape the engine's parser accepts: call-id and call-creator on `<call>`,
/// and `<voip_settings>` as a sibling of `<offer>` rather than a child.
fn offer_stanza(caller: &Jid, now: u64) -> Node {
    // `call-creator` is a *device* JID. WhatsApp Web's own parser reads it with
    // `attrDeviceJid`, which decodes the AD_JID form (`user:device@server`);
    // a plain JID encodes as JID_PAIR and the engine reads a user of `0`.
    let creator = caller.with_device(1);

    NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", CALL_ID)
        .attr("call-creator", creator)
        .attr("t", now.to_string())
        .children([
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
            // `uncompressed="1"` is what tells the engine the blob is plain.
            // Without it, it reads the bytes as compressed and reports
            // "unexpected compressed voip params". whatsapp-rust sets the same
            // attribute when it builds an accept.
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(SETTINGS.to_vec())
                .build(),
        ])
        .build()
}

// A `compressed_settings()` helper used to sit here, on the reading that the
// engine "expects the compressed form" because it rejected a plain blob. It
// does not: it rejects an *unmarked* one. `uncompressed="1"` above is the whole
// answer, `an_unmarked_settings_blob_is_rejected` in `tests/signaling.rs` pins
// it, and nothing ever called the helper. It took `flate2` out of the
// dependency list with it.

/// How the caller is identified.
///
/// The engine's own identity is a LID — `init_local_state begins ... is_lid: 1`
/// — so an offer arriving under phone-number identity may simply not match it.
/// This sweeps that, one axis at a time.
#[derive(Clone, Copy)]
struct Identity {
    label: &'static str,
    /// `from` and `call-creator` in the stanza use the LID namespace.
    stanza_lid: bool,
    /// Argument 8, the peer JID, is the LID form.
    arg_lid: bool,
}

/// The caller's LID. Fictitious, like every identity in these examples.
const CALLER_LID: &str = "11223344556677";
const CALLER_PN: &str = "15550001111";

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve("JgwtTQVeWPm")?;
    let bytes = std::fs::read(&entry.path)?;

    let shapes = [
        Identity {
            label: "pn / pn",
            stanza_lid: false,
            arg_lid: false,
        },
        Identity {
            label: "lid / pn",
            stanza_lid: true,
            arg_lid: false,
        },
        Identity {
            label: "pn / lid",
            stanza_lid: false,
            arg_lid: true,
        },
        Identity {
            label: "lid / lid",
            stanza_lid: true,
            arg_lid: true,
        },
    ];

    for shape in shapes {
        let mut r = Runtime::instantiate(&bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.run_ctors()?;
        r.attach_log_ring(1 << 20)?;

        // Three legacy-form JIDs, as `WAWeb/Voip/Init.js` passes them: the
        // user, the user's device, and the user's *device LID*.
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
            println!("{:12} media stack did not come up: {init:?}", shape.label);
            continue;
        }

        let caller = if shape.stanza_lid {
            Jid::new(CALLER_LID, Server::Lid)
        } else {
            Jid::new(CALLER_PN, Server::Pn)
        };
        let arg_caller = if shape.arg_lid {
            Jid::new(CALLER_LID, Server::Lid)
        } else {
            Jid::new(CALLER_PN, Server::Pn)
        };

        let now = r.virtual_unix_time();
        let payload = base64::engine::general_purpose::STANDARD
            .encode(marshal::marshal(&offer_stanza(&caller, now))?);

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
                Value::Str(arg_caller.to_string()),
                Value::Bytes(Vec::new()),
            ],
        );
        r.refuel();
        r.settle(std::time::Duration::from_secs(5));

        // Accept the call after the offer. If the engine takes it, it *must*
        // tell the peer — so this is the incoming path's chance to show the
        // outbound channel working.
        let accept_mark = r.engine_log().len();
        let accepted = r.call_embind("acceptCall", &[Value::Bool(false), Value::Bool(false)]);
        r.refuel();
        r.settle(std::time::Duration::from_secs(5));
        let accept_lines = r.engine_log_from(accept_mark);
        println!(
            "   acceptCall -> {} ({} lines, {} sent)",
            match &accepted {
                Ok(value) => format!("{value:?}"),
                Err(_) => "trap".to_owned(),
            },
            accept_lines.len(),
            r.shared()
                .hot_calls()
                .iter()
                .find(|(s, _)| s == "env::sendSignalingXMPP_js_sync")
                .map(|(_, n)| *n)
                .unwrap_or(0)
        );
        for line in accept_lines.iter().take(8) {
            println!("        {}", line.trim());
        }

        let lines = r.engine_log_from(mark);
        let missed = lines.iter().any(|line| line.contains("Call missed"));
        // Counters, not the recorded-call list: that list stops growing at
        // `MAX_TRACE` (8192) and startup alone makes tens of millions of host
        // calls, so `all_calls_to` answers zero for everything by the time this
        // runs. This example reported `sent=0` for a long time for that reason.
        let counts = r.shared().hot_calls();
        let count = |symbol: &str| -> u64 {
            counts
                .iter()
                .find(|(s, _)| s == symbol)
                .map(|(_, n)| *n)
                .unwrap_or(0)
        };
        let sent = count("env::sendSignalingXMPP_js_sync");
        let events = count("env::on_call_event_js_sync");
        println!(
            "=== {:12} missed={missed} sent={sent} events={events} lines={} {}",
            shape.label,
            lines.len(),
            if outcome.is_err() { "(trapped)" } else { "" }
        );
        for line in lines.iter().take(10) {
            println!("      {}", line.trim());
        }
    }

    Ok(())
}
