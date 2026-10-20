//! Evidence requirements shared by capture-backed scenarios.

use oracle_core::{Runtime, SignalingCall, Value};
use sha2::{Digest, Sha256};
use std::fmt;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Failure {
    Mismatch,
    MissingCapture,
    Overflow,
    Infrastructure,
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "oracle {self:?}")
    }
}

pub fn fail(kind: Failure, reason: impl fmt::Display) -> ! {
    panic!("{kind}: {reason}")
}

/// A quiet log alone does not prove a particular protocol outcome. These are
/// prerequisites for judging it; the scenario must still assert its milestone.
pub fn check_log(overflowed: bool, coherent: Option<bool>, settled: bool) -> Result<(), Failure> {
    if overflowed {
        Err(Failure::Overflow)
    } else if coherent != Some(true) || !settled {
        Err(Failure::Infrastructure)
    } else {
        Ok(())
    }
}

/// Record only synthetic test inputs, never account/session state or credentials.
pub fn call(runtime: &mut Runtime, name: &str, args: &[Value]) -> anyhow::Result<Value> {
    let input = format!("{args:?}");
    eprintln!(
        "oracle operation={name} input_sha256={} arguments={input}",
        hex::encode(Sha256::digest(input.as_bytes()))
    );
    let result = runtime.call_embind(name, args);
    eprintln!("oracle operation={name} result={result:?}");
    result
}

pub fn record(runtime: &Runtime) {
    eprintln!(
        "oracle engine_log={:?} host_log={:?} stubs={:?}",
        runtime.engine_log(),
        runtime.logs(),
        runtime.stubs_called()
    );
}

/// A signaling callback is the completion milestone. Worker-pool quiescence
/// would time out even after a complete offer has been captured.
pub fn wait_for_signaling(
    runtime: &mut Runtime,
    timeout: Duration,
    call_id: &str,
) -> anyhow::Result<Vec<SignalingCall>> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Err(error) = runtime.try_process_queued_calls() {
            record(runtime);
            return Err(error);
        }
        let sent: Vec<_> = runtime
            .signaling()?
            .into_iter()
            .filter(|call| call.call_id == call_id)
            .collect();
        if !sent.is_empty() {
            record(runtime);
            for call in &sent {
                eprintln!(
                    "oracle signaling_sha256={} stanza_hex={}",
                    hex::encode(Sha256::digest(&call.stanza)),
                    hex::encode(&call.stanza)
                );
            }
            return Ok(sent);
        }
        if Instant::now() >= deadline {
            record(runtime);
            anyhow::bail!("oracle Infrastructure: no signaling callback before deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}
