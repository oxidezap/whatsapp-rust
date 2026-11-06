//! Observe the unchanged, hash-pinned upstream Node harness over CDP.
//! No evaluated expression, injected script, source patch or JS adapter is used.
use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read},
    net::TcpStream,
    path::Path,
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::Duration,
};
use tungstenite::{Message, WebSocket, stream::MaybeTlsStream};
const HARNESS: &str = "tests/conformance/groups/versions/2.3000.1052559453/normalization.cjs";
const SHA: &str = "8629e44df910c573a28c5552ecd87c97efa22c004a040b42d214bbda528a70a0";
struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Inspector {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    id: u64,
    pending: Vec<Value>,
}
impl Inspector {
    fn receive(&mut self) -> Result<Value> {
        loop {
            match self.ws.read()? {
                Message::Text(text) => {
                    ensure!(text.len() <= 4 * 1024 * 1024, "CDP message budget");
                    return Ok(serde_json::from_str(&text)?);
                }
                Message::Ping(payload) => self.ws.send(Message::Pong(payload))?,
                Message::Close(_) => bail!("inspector closed before completion"),
                _ => (),
            }
        }
    }
    fn call(&mut self, method: &str, params: Value) -> Result<Value> {
        self.id += 1;
        let id = self.id;
        self.ws.send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))?;
        loop {
            let value = self.receive()?;
            if value["id"] == id {
                ensure!(value.get("error").is_none(), "CDP {method}: {value}");
                return Ok(value["result"].clone());
            }
            ensure!(self.pending.len() < 4096, "CDP event budget");
            self.pending.push(value);
        }
    }
    fn event(&mut self) -> Result<Value> {
        if self.pending.is_empty() {
            self.receive()
        } else {
            Ok(self.pending.remove(0))
        }
    }
    fn properties(&mut self, object: &Value) -> Result<Vec<Value>> {
        let id = object["objectId"]
            .as_str()
            .context("remote object missing ID")?;
        let reply = self.call(
            "Runtime.getProperties",
            json!({"objectId":id,"ownProperties":true,"generatePreview":false}),
        )?;
        Ok(reply["result"]
            .as_array()
            .context("missing descriptors")?
            .clone())
    }
    fn remote_json(&mut self, value: &Value, depth: usize) -> Result<Value> {
        ensure!(depth < 16, "remote JSON depth budget");
        if let Some(v) = value.get("value") {
            return Ok(v.clone());
        }
        ensure!(
            value["type"] == "object",
            "unsupported remote value {value}"
        );
        let props = self.properties(value)?;
        let array = value["subtype"] == "array";
        let mut output = serde_json::Map::new();
        for p in props {
            let name = p["name"].as_str().context("descriptor name")?;
            if array && name == "length" {
                continue;
            }
            if p["enumerable"] == false {
                continue;
            }
            ensure!(
                output.len() < 1024 && p.get("get").is_none(),
                "remote properties budget/accessor"
            );
            if matches!(
                p["value"]["type"].as_str(),
                Some("undefined" | "function" | "symbol")
            ) {
                if array {
                    output.insert(name.into(), Value::Null);
                }
                continue;
            }
            output.insert(name.into(), self.remote_json(&p["value"], depth + 1)?);
        }
        if array {
            let mut entries = vec![];
            for i in 0..output.len() {
                entries.push(
                    output
                        .remove(&i.to_string())
                        .context("sparse remote array")?,
                );
            }
            Ok(Value::Array(entries))
        } else {
            Ok(Value::Object(output))
        }
    }
    fn actual_payload(&mut self, frame: &Value) -> Result<Value> {
        for scope in frame["scopeChain"].as_array().context("scope chain")? {
            for property in self.properties(&scope["object"])? {
                if property["name"] == "actual" {
                    let actual = &property["value"];
                    if actual["subtype"] == "null" {
                        return Ok(Value::Null);
                    }
                    for p in self.properties(actual)? {
                        if p["name"] == "groupDatas" {
                            return self.remote_json(&p["value"], 0);
                        }
                    }
                    bail!("actual result lacks groupDatas");
                }
            }
        }
        bail!("actual not found at capture boundary")
    }
}
fn compact(mut original: Value, payloads: Vec<Value>) -> Result<String> {
    let cases = original["cases"].as_array_mut().context("original cases")?;
    ensure!(
        cases.len() == 204 && cases.len() == payloads.len(),
        "capture count differs"
    );
    let mut outcomes: Vec<Value> = vec![];
    let mut rows = vec![];
    for (case, payload) in cases.iter_mut().zip(payloads) {
        let object = case.as_object_mut().context("case object")?;
        let row: Vec<_> = ["family", "kind", "reason", "hidden"]
            .into_iter()
            .map(|k| object.shift_remove(k).unwrap_or(Value::Null))
            .collect();
        let reads = object.shift_remove("reads").context("reads")?;
        let calls = object.shift_remove("calls").context("calls")?;
        object.insert("groupDatas".into(), payload);
        object.insert("reads".into(), reads);
        object.insert("calls".into(), calls);
        let index = outcomes.iter().position(|v| v == case).unwrap_or_else(|| {
            outcomes.push(case.clone());
            outcomes.len() - 1
        });
        let mut row = row;
        row.push(json!(index));
        rows.push(serde_json::to_string(&row)?);
    }
    let metadata = json!({"waVersion":original["waVersion"],"setHash":original["setHash"],"inputColumns":["family","kind","reason","hidden","outcome"],"outcomes":outcomes});
    let mut rendered = serde_json::to_string_pretty(&metadata)?;
    rendered.truncate(rendered.len() - 2);
    rendered.push_str(",\n  \"cases\": [\n");
    rendered.push_str(
        &rows
            .iter()
            .map(|s| format!("    {s}"))
            .collect::<Vec<_>>()
            .join(",\n"),
    );
    rendered.push_str("\n  ]\n}\n");
    Ok(rendered)
}
pub fn run(root: &Path, check: Option<&Path>) -> Result<()> {
    for (relative, expected) in [
        (
            "tests/conformance/groups/versions/2.3000.1052559453/evidence.json",
            "8f7551630e7ce7a6045c1bb3a5aa2173612a62cbe2ecbc7657acd7182588d2c0",
        ),
        (
            "generated/notif/group-link.json",
            "80f39ae823f54827de3ad7af9e96e103d1212a91a109d0a97888913ec902b641",
        ),
        (
            "generated/notif/group-unlink.json",
            "4ad96a230e7850d6e5407bd8b3e4c073d13ab0a2c123bad54907c09e13405ff2",
        ),
    ] {
        ensure!(
            crate::inputs::sha(&std::fs::read(root.join(relative))?) == expected,
            "upstream group input pin differs: {relative}"
        );
    }
    let script = root.join(HARNESS).canonicalize()?;
    let source = std::fs::read(&script)?;
    ensure!(
        hex::encode(Sha256::digest(&source)) == SHA,
        "review changed upstream harness before capture"
    );
    let text = std::str::from_utf8(&source)?;
    let lines: Vec<_> = text
        .lines()
        .enumerate()
        .filter(|(_, line)| line.trim() == "outcomes.push({")
        .collect();
    ensure!(lines.len() == 1, "capture boundary changed");
    let boundary = lines[0].0;
    let mut child = Process(
        Command::new("node")
            .arg("--inspect-brk=127.0.0.1:0")
            .arg(&script)
            .arg("--json")
            .current_dir(script.parent().context("harness parent")?)
            .env_remove("NODE_OPTIONS")
            .env_remove("NODE_PATH")
            .env_remove("GROUP_LINK_CONTRACT")
            .env_remove("GROUP_UNLINK_CONTRACT")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let stdout = child.0.stdout.take().context("child stdout")?;
    let output = std::thread::spawn(move || {
        let mut bytes = vec![];
        stdout
            .take(2 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let stderr = child.0.stderr.take().context("child stderr")?;
    let (tx, rx) = mpsc::sync_channel(1);
    let errors = std::thread::spawn(move || {
        let mut messages = String::new();
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            if let Some(url) = line.strip_prefix("Debugger listening on ") {
                let _ = tx.try_send(url.to_owned());
            }
            if messages.len() < 65536 {
                messages.push_str(&line);
                messages.push('\n');
            }
        }
        Ok::<_, std::io::Error>(messages)
    });
    let url = rx
        .recv_timeout(Duration::from_secs(15))
        .context("inspector startup timeout")?;
    ensure!(
        url.starts_with("ws://127.0.0.1:"),
        "inspector must be loopback"
    );
    let (ws, _) = tungstenite::connect(&url)?;
    if let MaybeTlsStream::Plain(stream) = ws.get_ref() {
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
    }
    let mut inspector = Inspector {
        ws,
        id: 0,
        pending: vec![],
    };
    inspector.call("Debugger.enable", json!({}))?;
    let bp = inspector.call(
        "Debugger.setBreakpointByUrl",
        json!({"lineNumber":boundary,"urlRegex":".*/normalization\\.cjs$"}),
    )?;
    let breakpoint = bp["breakpointId"]
        .as_str()
        .context("breakpoint id")?
        .to_owned();
    inspector.call("Runtime.runIfWaitingForDebugger", json!({}))?;
    let mut payloads = vec![];
    loop {
        let event = inspector.event()?;
        match event["method"].as_str() {
            Some("Debugger.paused") => {
                let params = &event["params"];
                if params["hitBreakpoints"]
                    .as_array()
                    .is_some_and(|v| v.iter().any(|x| x == &breakpoint))
                {
                    let frame = &params["callFrames"][0];
                    ensure!(
                        frame["location"]["lineNumber"] == boundary && payloads.len() < 204,
                        "unexpected capture pause"
                    );
                    payloads.push(inspector.actual_payload(frame)?);
                }
                inspector.call("Debugger.resume", json!({}))?;
                if payloads.len() == 204 {
                    inspector.call("Debugger.disable", json!({}))?;
                    inspector.ws.close(None)?;
                    break;
                }
            }
            Some("Runtime.executionContextDestroyed") | Some("Inspector.detached") => break,
            _ => (),
        }
    }
    drop(inspector);
    let status = child.0.wait()?;
    let diagnostics = errors
        .join()
        .map_err(|_| anyhow::anyhow!("stderr reader panic"))??;
    ensure!(status.success(), "original harness failed: {diagnostics}");
    let bytes = output
        .join()
        .map_err(|_| anyhow::anyhow!("stdout reader panic"))??;
    ensure!(bytes.len() <= 2 * 1024 * 1024, "harness output budget");
    let rendered = compact(serde_json::from_slice(&bytes)?, payloads)?;
    if let Some(path) = check {
        ensure!(
            std::fs::read_to_string(path)? == rendered,
            "compact group fixture differs"
        );
        println!("204 complete original normalization cases match");
    } else {
        print!("{rendered}");
    }
    Ok(())
}
