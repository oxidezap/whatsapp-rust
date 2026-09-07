//! Setup shared by the outbound experiments; experiment inputs remain at call sites.
use oracle_core::{Runtime, ThreadPolicy, Value};
use wacore_binary::{Jid, Node, builder::NodeBuilder};

pub struct Startup<'a> {
    pub identity: [&'a str; 3],
    pub attempts: usize,
    pub register_main: bool,
    pub log_bytes: u32,
    pub marker_sink: Option<&'a str>,
}

pub fn engine(bytes: &[u8], setup: Startup<'_>) -> anyhow::Result<Runtime> {
    let mut last_failure = String::new();
    for _ in 0..setup.attempts {
        let mut runtime = Runtime::instantiate(bytes)?;
        runtime.set_thread_policy(ThreadPolicy::Spawn);
        runtime.set_main_thread_registration(setup.register_main);
        runtime.run_ctors()?;
        runtime.attach_log_ring(setup.log_bytes)?;
        if let Some(sink) = setup.marker_sink {
            runtime.shared().watch_markers(sink);
        }
        let args = setup.identity.map(|value| Value::Str(value.to_owned()));
        let init = runtime.call_embind("initVoipStack", &args);
        runtime.refuel();
        if init.as_ref().ok().and_then(Value::as_int) == Some(0) {
            return Ok(runtime);
        }
        last_failure = format!("{init:?}");
    }
    anyhow::bail!(
        "initVoipStack failed after {} attempts: {last_failure}",
        setup.attempts
    )
}

pub fn settings_offer(caller: &Jid, now: u64, call_id: &str, settings: &[u8]) -> Node {
    NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", call_id)
        .attr("call-creator", caller.with_device(1))
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
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(settings.to_vec())
                .build(),
        ])
        .build()
}
