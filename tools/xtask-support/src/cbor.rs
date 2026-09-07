//! Lossless canonical fixture values, independent of zstd representation.
use anyhow::Result;
use serde_json::Value;

fn head(out: &mut Vec<u8>, major: u8, n: u64) {
    let prefix = major << 5;
    if n < 24 {
        out.push(prefix | n as u8);
    } else if n <= u8::MAX as u64 {
        out.extend([prefix | 24, n as u8]);
    } else if n <= u16::MAX as u64 {
        out.push(prefix | 25);
        out.extend((n as u16).to_be_bytes());
    } else if n <= u32::MAX as u64 {
        out.push(prefix | 26);
        out.extend((n as u32).to_be_bytes());
    } else {
        out.push(prefix | 27);
        out.extend(n.to_be_bytes());
    }
}
fn encode_into(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.push(0xf6),
        Value::Bool(b) => out.push(if *b { 0xf5 } else { 0xf4 }),
        Value::Number(n) => {
            if let Some(v) = n.as_u64() {
                head(out, 0, v);
            } else if let Some(v) = n.as_i64() {
                head(out, 1, v.unsigned_abs() - 1);
            } else if let Some(v) = n.as_f64() {
                if f64::from(v as f32) == v {
                    out.push(0xfa);
                    out.extend((v as f32).to_be_bytes());
                } else {
                    out.push(0xfb);
                    out.extend(v.to_be_bytes());
                }
            }
        }
        Value::String(s) => {
            head(out, 3, s.len() as u64);
            out.extend(s.as_bytes());
        }
        Value::Array(a) => {
            head(out, 4, a.len() as u64);
            for v in a {
                encode_into(v, out);
            }
        }
        Value::Object(m) => {
            head(out, 5, m.len() as u64);
            let sorted: std::collections::BTreeMap<_, _> = m.iter().collect();
            for (k, v) in sorted {
                encode_into(&Value::String(k.clone()), out);
                encode_into(v, out);
            }
        }
    }
}
/// Encode JSON values without rounding. Exactly representable floats use f32; others use f64.
pub fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(value, &mut out);
    out
}
/// Compress bytes using the pinned zstd library, without shelling out.
pub fn compress(bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(zstd::stream::encode_all(bytes, 19)?)
}
/// Inflate an archive for content comparison.
pub fn decompress(bytes: &[u8]) -> Result<Vec<u8>> {
    Ok(zstd::stream::decode_all(bytes)?)
}
/// Pack fixture JSON and return hashes and lengths used by the audit manifest.
pub fn pack(source: &[u8]) -> Result<(Vec<u8>, Value)> {
    let value: Value = serde_json::from_slice(source)?;
    let cbor = encode(&value);
    let packed = compress(&cbor)?;
    let records = match &value {
        Value::Array(v) => Some(v.len()),
        Value::Object(v) => Some(v.len()),
        _ => None,
    };
    let meta = serde_json::json!({"json_sha256":crate::sha256(source),"cbor_sha256":crate::sha256(&cbor),"zstd_sha256":crate::sha256(&packed),"json_bytes":source.len(),"cbor_bytes":cbor.len(),"packed_bytes":packed.len(),"records":records});
    Ok((packed, meta))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserves_float_integer_and_negative_zero_distinctions() {
        assert_eq!(encode(&serde_json::json!(1)), [1]);
        assert_eq!(encode(&serde_json::json!(1.0)), [0xfa, 0x3f, 0x80, 0, 0]);
        assert_eq!(encode(&serde_json::json!(-0.0)), [0xfa, 0x80, 0, 0, 0]);
        assert_eq!(
            encode(&serde_json::json!(i64::MIN)),
            [0x3b, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]
        );
        let v = serde_json::json!({"z":[1.25,0.1],"a":false});
        let b = encode(&v);
        assert_eq!(decompress(&compress(&b).unwrap()).unwrap(), b);
    }
}
