//! Drive the original WAM writer from native Rust, preserving the legacy corpus.
use crate::{engine::*, inputs};
use anyhow::{Result, ensure};
use boa_engine::{JsResult, JsValue, object::builtins::JsUint8Array};
use serde_json::{Value, json};
use std::path::Path;
pub const MODULES: &[&str] = &[
    "WABinary",
    "WAHex",
    "WAWamTypes",
    "WAWebWamLibProtocol",
    "WAWamBuffer",
];
/// Globals, timestamp, event code, sampling weight and ordered fields.
type Event = (Vec<(u32, Value)>, i64, u32, i32, Vec<(u32, Value)>);
fn peek(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    use boa_engine::JsArgs;
    method(args.get_or_undefined(0), "readByteArrayView", &[], cx)
}
fn event(b: &JsValue, proto: &JsValue, input: &Event, cx: &mut Cx) -> JsResult<()> {
    let (globals, ts, code, weight, fields) = input;
    let mut globals = globals.to_vec();
    globals.push((47, json!(ts)));
    globals.sort_by_key(|(id, _)| *id);
    for (id, value) in globals {
        let value = JsValue::from_json(&value, cx)?;
        method(
            proto,
            "writeGlobalAttribute",
            &[b.clone(), id.into(), value],
            cx,
        )?;
    }
    method(
        proto,
        "writeEvent",
        &[
            b.clone(),
            (*code).into(),
            (-*weight).into(),
            (!fields.is_empty()).into(),
        ],
        cx,
    )?;
    for (i, (id, value)) in fields.iter().enumerate() {
        let value = JsValue::from_json(value, cx)?;
        method(
            proto,
            "writeField",
            &[
                b.clone(),
                (*id).into(),
                value,
                (i + 1 < fields.len()).into(),
            ],
            cx,
        )?;
    }
    Ok(())
}
struct Corpus {
    cx: Cx,
    binary: JsValue,
    proto: JsValue,
    vectors: Vec<Value>,
}
impl Corpus {
    fn emit(&mut self, name: &str, channel: u8, sequence: u16, events: &[Event]) -> JsResult<()> {
        let b: JsValue = obj(&self.binary)?
            .construct(&[JsValue::undefined(), true.into()], None, &mut self.cx)?
            .into();
        method(&b, "writeString", &[string("WAM")], &mut self.cx)?;
        for byte in [5_u8, 1] {
            method(&b, "writeUint8", &[u32::from(byte).into()], &mut self.cx)?;
        }
        method(
            &b,
            "writeUint16",
            &[u32::from(sequence).into()],
            &mut self.cx,
        )?;
        method(&b, "writeUint8", &[u32::from(channel).into()], &mut self.cx)?;
        for input in events {
            event(&b, &self.proto, input, &mut self.cx)?;
        }
        let reader = function(&mut self.cx, "peekBytes", peek);
        let bytes = method(&b, "peek", &[reader], &mut self.cx)?;
        let bytes = JsUint8Array::from_object(obj(&bytes)?)?.to_vec(&mut self.cx)?;
        self.vectors
            .push(json!({"name":name,"hex":hex::encode(bytes)}));
        Ok(())
    }
    fn one(&mut self, name: &str, code: u32, fields: Vec<(u32, Value)>) -> JsResult<()> {
        self.emit(name, 0, 1, &[(vec![], 1755000000, code, 1, fields)])
    }
}
pub fn run(root: &Path, lock: &Path, check: Option<&Path>, dump: Option<&Path>) -> Result<()> {
    let paths = inputs::verify(root, lock)?;
    let sources = inputs::modules(&paths, MODULES)?;
    if let Some(path) = dump {
        let mut bytes = vec![];
        for source in sources.values() {
            bytes.extend(source);
            bytes.extend(b";\n");
        }
        std::fs::write(path, bytes)?;
    }
    let mut cx = context().map_err(|e| anyhow::anyhow!("{e}"))?;
    for source in sources.values() {
        evaluate(source, &mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let binary_module = load("WABinary", &mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    let binary = get(&binary_module, "Binary", &mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    let proto = load("WAWebWamLibProtocol", &mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut c = Corpus {
        cx,
        binary,
        proto,
        vectors: vec![],
    };
    (|| -> JsResult<()> {
        for v in [
            0_i64,
            1,
            -1,
            2,
            127,
            -128,
            128,
            -129,
            32767,
            -32768,
            32768,
            -32769,
            2147483647,
            -2147483648,
            2147483648,
            -2147483649,
        ] {
            c.one(&format!("int_{v}"), 3448, vec![(3, json!(v))])?;
        }
        for len in [0, 1, 255, 256, 65535, 65536] {
            c.one(
                &format!("str_{len}"),
                3448,
                vec![(1, json!("a".repeat(len)))],
            )?;
        }
        c.one("str_utf8", 3448, vec![(1, json!("héllo·世界"))])?;
        c.one("float_field", 1336, vec![(3, json!(2.5))])?;
        c.one("float_integral", 1336, vec![(3, json!(4))])?;
        c.one("wide_field_id", 462, vec![(1016, json!(12))])?;
        c.emit("no_fields", 0, 7, &[(vec![], 1755000000, 3264, 1, vec![])])?;
        c.emit(
            "weighted_event",
            0,
            258,
            &[(
                vec![],
                1755000000,
                2496,
                2000,
                vec![
                    (14, json!(1)),
                    (1, json!(42)),
                    (3, json!(0)),
                    (9, json!(1)),
                    (7, json!(3)),
                    (4, json!("read")),
                ],
            )],
        )?;
        c.emit(
            "two_events_dedup",
            0,
            3,
            &[
                (
                    vec![
                        (17, json!("2.3000.1045368834")),
                        (11, json!(8)),
                        (4473, json!("")),
                    ],
                    1755000000,
                    4358,
                    1,
                    vec![(1, json!(3448)), (2, json!(1))],
                ),
                (vec![], 1755000061, 1144, 1, vec![(28, json!(2))]),
            ],
        )?;
        c.emit(
            "cleared_global",
            0,
            4,
            &[
                (vec![(779, json!("x"))], 1755000000, 3264, 1, vec![]),
                (vec![(779, Value::Null)], 1755000001, 3264, 1, vec![]),
            ],
        )?;
        c.emit(
            "realtime_channel",
            1,
            9,
            &[(vec![], 1755000000, 5504, 1, vec![(6, json!(1))])],
        )?;
        Ok(())
    })()
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    let result = json!(c.vectors);
    if let Some(path) = check {
        let expected: Value = serde_json::from_slice(&std::fs::read(path)?)?;
        if expected["format"] == "sha256-v1" {
            let lock: Value = serde_json::from_slice(&std::fs::read(lock)?)?;
            ensure!(
                expected["setHash"] == lock["setHash"]
                    && expected["waVersion"] == lock["waVersion"],
                "WAM fixture snapshot differs"
            );
            let observed = c
                .vectors
                .iter()
                .map(|v| {
                    let bytes = hex::decode(
                        v["hex"]
                            .as_str()
                            .ok_or_else(|| anyhow::anyhow!("vector hex"))?,
                    )
                    .map_err(anyhow::Error::msg)?;
                    Ok(json!({"name":v["name"],"bytes":bytes.len(),"sha256":inputs::sha(&bytes)}))
                })
                .collect::<Result<Vec<Value>>>()?;
            ensure!(
                json!(observed) == expected["vectors"],
                "WAM vector hashes differ"
            );
        } else {
            ensure!(result == expected, "WAM vectors differ");
        }
        println!("{} complete original WAM vectors match", c.vectors.len());
    } else {
        println!("{}", serde_json::to_string_pretty(&result)?);
    }
    Ok(())
}
