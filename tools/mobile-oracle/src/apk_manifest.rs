//! Android binary XML literal metadata. Resource references are never guessed.
//! Format definitions: AOSP androidfw/ResourceTypes.h, blob
//! e330410ed1a0890be266da0e62a3e6d1a25a3cdc (ResStringPool_header,
//! ResXMLTree_node, ResXMLTree_attrExt, ResXMLTree_attribute).
//! This deliberately supports a bounded literal subset, not Android resource resolution.
use anyhow::{Context, Result, ensure};
use serde::Serialize;
const NONE: u32 = u32::MAX;
const ANDROID: &str = "http://schemas.android.com/apk/res/android";
#[derive(Debug, Serialize, PartialEq, Eq)]
pub(crate) struct Manifest {
    pub package: String,
    pub version_name: String,
    pub version_code: u64,
    pub min_sdk: Option<u32>,
    pub target_sdk: Option<u32>,
    pub permissions: Vec<String>,
}
fn u16le(b: &[u8], n: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(
        b.get(n..n + 2).context("truncated XML u16")?.try_into()?,
    ))
}
fn u32le(b: &[u8], n: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(
        b.get(n..n + 4).context("truncated XML u32")?.try_into()?,
    ))
}
fn length8(b: &[u8], p: &mut usize) -> Result<usize> {
    let a = *b.get(*p).context("truncated UTF8 length")?;
    *p += 1;
    if a & 128 == 0 {
        Ok(usize::from(a))
    } else {
        let c = *b.get(*p).context("truncated UTF8 length")?;
        *p += 1;
        Ok((usize::from(a & 127) << 8) | usize::from(c))
    }
}
fn length16(b: &[u8], p: &mut usize) -> Result<usize> {
    let a = u16le(b, *p)?;
    *p += 2;
    if a & 32768 == 0 {
        Ok(usize::from(a))
    } else {
        let c = u16le(b, *p)?;
        *p += 2;
        Ok((usize::from(a & 32767) << 16) | usize::from(c))
    }
}
fn pool(b: &[u8], header: usize) -> Result<Vec<String>> {
    ensure!(header >= 28, "short string pool header");
    let count = u32le(b, 8)? as usize;
    let styles = u32le(b, 12)? as usize;
    ensure!(
        count <= 65_536 && styles <= 65_536,
        "string pool budget exceeded"
    );
    let start = u32le(b, 20)? as usize;
    let style_start = u32le(b, 24)? as usize;
    ensure!(
        start >= header + (count + styles) * 4 && start <= b.len(),
        "invalid string pool offsets"
    );
    let end = if style_start == 0 {
        b.len()
    } else {
        style_start
    };
    ensure!(end >= start && end <= b.len(), "invalid string pool end");
    let utf8 = u32le(b, 16)? & 256 != 0;
    let mut result = Vec::with_capacity(count);
    let mut decoded_total = 0usize;
    for i in 0..count {
        let offset = u32le(b, header + i * 4)? as usize;
        ensure!(offset < end - start, "string outside pool");
        let mut p = start + offset;
        let string = if utf8 {
            let units = length8(&b[..end], &mut p)?;
            let len = length8(&b[..end], &mut p)?;
            ensure!(len <= 1024 * 1024, "XML string budget exceeded");
            let value = std::str::from_utf8(
                b.get(p..p + len)
                    .filter(|_| p + len < end)
                    .context("truncated UTF8")?,
            )?;
            ensure!(
                b[p + len] == 0 && value.encode_utf16().count() == units,
                "invalid UTF8 terminator or length"
            );
            value.to_owned()
        } else {
            let len = length16(&b[..end], &mut p)?;
            ensure!(len <= 512 * 1024, "XML string budget exceeded");
            ensure!(
                len <= (end - p) / 2 && p + len * 2 + 2 <= end,
                "truncated UTF16"
            );
            let units = (0..len)
                .map(|i| u16le(b, p + i * 2))
                .collect::<Result<Vec<_>>>()?;
            ensure!(u16le(b, p + len * 2)? == 0, "invalid UTF16 terminator");
            String::from_utf16(&units)?
        };
        decoded_total += string.len();
        ensure!(
            decoded_total <= 16 * 1024 * 1024,
            "decoded string pool budget exceeded"
        );
        result.push(string);
    }
    Ok(result)
}
fn string(strings: &[String], index: u32) -> Result<&str> {
    Ok(strings
        .get(index as usize)
        .context("invalid XML string index")?)
}
fn set<T>(slot: &mut Option<T>, value: T) -> Result<()> {
    ensure!(slot.is_none(), "duplicate manifest metadata");
    *slot = Some(value);
    Ok(())
}
/// Parse bounded literal manifest fields; no resource table evaluation or platform fallback.
pub(crate) fn parse(bytes: &[u8]) -> Result<Manifest> {
    ensure!(
        bytes.len() >= 8 && bytes.len() <= 8 * 1024 * 1024,
        "manifest size budget exceeded"
    );
    ensure!(
        u16le(bytes, 0)? == 3 && u16le(bytes, 2)? == 8 && u32le(bytes, 4)? as usize == bytes.len(),
        "invalid binary XML root"
    );
    let mut strings = None;
    let mut stack = Vec::new();
    let mut seen_root = false;
    let (mut package, mut version_name, mut version_code, mut major) = (None, None, None, None);
    let (mut min_sdk, mut target_sdk) = (None, None);
    let mut permissions = Vec::new();
    let mut p = 8;
    let mut nodes = 0;
    while p < bytes.len() {
        nodes += 1;
        ensure!(nodes <= 65_536, "XML node budget exceeded");
        let kind = u16le(bytes, p)?;
        let header = u16le(bytes, p + 2)? as usize;
        let size = u32le(bytes, p + 4)? as usize;
        ensure!(
            header >= 8 && size >= header && size.is_multiple_of(4) && p + size <= bytes.len(),
            "invalid XML chunk"
        );
        let b = &bytes[p..p + size];
        if kind == 1 {
            ensure!(
                strings.is_none() && !seen_root,
                "duplicate or late string pool"
            );
            strings = Some(pool(b, header)?);
        } else if kind == 0x102 {
            ensure!(header == 16 && size >= 36, "invalid start element");
            let strings = strings.as_ref().context("missing string pool")?;
            let name = string(strings, u32le(b, 20)?)?;
            if stack.is_empty() {
                ensure!(
                    !seen_root && name == "manifest" && u32le(b, 16)? == NONE,
                    "invalid manifest root"
                );
                seen_root = true;
            }
            ensure!(stack.len() < 64, "XML depth budget exceeded");
            let start = u16le(b, 24)? as usize;
            let stride = u16le(b, 26)? as usize;
            let count = u16le(b, 28)? as usize;
            ensure!(
                start >= 20 && stride == 20 && count <= 4096 && 16 + start + stride * count <= size,
                "invalid XML attributes"
            );
            let mut names = std::collections::BTreeSet::new();
            for i in 0..count {
                let a = 16 + start + i * stride;
                let ns = u32le(b, a)?;
                let key = string(strings, u32le(b, a + 4)?)?;
                ensure!(names.insert((ns, key)), "duplicate XML attribute");
                ensure!(
                    u16le(b, a + 12)? == 8 && b[a + 14] == 0,
                    "invalid XML typed value"
                );
                let android = ns != NONE && string(strings, ns)? == ANDROID;
                let typ = b[a + 15];
                let data = u32le(b, a + 16)?;
                let literal_string = || -> Result<String> {
                    ensure!(typ == 3, "metadata requires literal string");
                    Ok(string(strings, data)?.to_owned())
                };
                let literal_int = || -> Result<u32> {
                    ensure!(
                        matches!(typ, 0x10 | 0x11),
                        "metadata requires literal integer"
                    );
                    Ok(data)
                };
                if name == "manifest" && stack.is_empty() {
                    match key {
                        "package" if ns == NONE => set(&mut package, literal_string()?)?,
                        "versionName" if android => set(&mut version_name, literal_string()?)?,
                        "versionCode" if android => set(&mut version_code, literal_int()?)?,
                        "versionCodeMajor" if android => set(&mut major, literal_int()?)?,
                        _ => {}
                    }
                } else if stack.len() == 1 && name == "uses-sdk" && android {
                    match key {
                        "minSdkVersion" => set(&mut min_sdk, literal_int()?)?,
                        "targetSdkVersion" => set(&mut target_sdk, literal_int()?)?,
                        _ => {}
                    }
                } else if stack.len() == 1
                    && matches!(name, "uses-permission" | "uses-permission-sdk-23")
                    && android
                    && key == "name"
                {
                    ensure!(permissions.len() < 4096, "permission budget exceeded");
                    permissions.push(literal_string()?);
                }
            }
            stack.push((u32le(b, 16)?, name.to_owned()));
        } else if kind == 0x103 {
            ensure!(header == 16 && size == 24, "invalid end element");
            let strings = strings.as_ref().context("missing string pool")?;
            ensure!(
                stack.pop() == Some((u32le(b, 16)?, string(strings, u32le(b, 20)?)?.to_owned())),
                "unbalanced XML elements"
            );
        } else {
            ensure!(
                matches!(kind, 0x100 | 0x101 | 0x180),
                "unsupported XML chunk"
            );
        }
        p += size;
    }
    ensure!(seen_root && stack.is_empty(), "incomplete manifest");
    Ok(Manifest {
        package: package.context("missing package")?,
        version_name: version_name.context("missing versionName")?,
        version_code: u64::from(version_code.context("missing versionCode")?)
            | (u64::from(major.unwrap_or(0)) << 32),
        min_sdk,
        target_sdk,
        permissions,
    })
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn fixture() -> Vec<u8> {
        fn word(b: &mut Vec<u8>, v: u32) {
            b.extend(v.to_le_bytes());
        }
        fn half(b: &mut Vec<u8>, v: u16) {
            b.extend(v.to_le_bytes());
        }
        let values = [
            "manifest",
            "package",
            "com.whatsapp",
            "http://schemas.android.com/apk/res/android",
            "versionName",
            "2.0",
            "versionCode",
        ];
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for value in values {
            offsets.push(data.len() as u32);
            data.push(value.len() as u8);
            data.push(value.len() as u8);
            data.extend(value.as_bytes());
            data.push(0);
        }
        while data.len() % 4 != 0 {
            data.push(0);
        }
        let mut pool = Vec::new();
        half(&mut pool, 1);
        half(&mut pool, 28);
        word(&mut pool, (28 + offsets.len() * 4 + data.len()) as u32);
        word(&mut pool, offsets.len() as u32);
        word(&mut pool, 0);
        word(&mut pool, 256);
        word(&mut pool, (28 + offsets.len() * 4) as u32);
        word(&mut pool, 0);
        for offset in offsets {
            word(&mut pool, offset);
        }
        pool.extend(data);
        let mut start = Vec::new();
        half(&mut start, 0x102);
        half(&mut start, 16);
        word(&mut start, 96);
        word(&mut start, 1);
        word(&mut start, NONE);
        word(&mut start, NONE);
        word(&mut start, 0);
        half(&mut start, 20);
        half(&mut start, 20);
        half(&mut start, 3);
        half(&mut start, 0);
        half(&mut start, 0);
        half(&mut start, 0);
        for (ns, key, kind, value) in [(NONE, 1, 3, 2), (3, 4, 3, 5), (3, 6, 0x10, 42)] {
            word(&mut start, ns);
            word(&mut start, key);
            word(&mut start, NONE);
            half(&mut start, 8);
            start.push(0);
            start.push(kind);
            word(&mut start, value);
        }
        let mut end = Vec::new();
        half(&mut end, 0x103);
        half(&mut end, 16);
        word(&mut end, 24);
        word(&mut end, 1);
        word(&mut end, NONE);
        word(&mut end, NONE);
        word(&mut end, 0);
        let mut root = Vec::new();
        half(&mut root, 3);
        half(&mut root, 8);
        word(&mut root, (8 + pool.len() + start.len() + end.len()) as u32);
        root.extend(pool);
        root.extend(start);
        root.extend(end);
        root
    }
    #[test]
    fn synthetic_literal_manifest_and_all_truncations() {
        let bytes = fixture();
        let manifest = parse(&bytes).unwrap();
        assert_eq!(manifest.package, "com.whatsapp");
        assert_eq!(manifest.version_name, "2.0");
        assert_eq!(manifest.version_code, 42);
        for end in 0..bytes.len() {
            assert!(parse(&bytes[..end]).is_err());
        }
        let mut bad = bytes.clone();
        let end = bad.len();
        bad[end - 4] = 1;
        assert!(parse(&bad).is_err());
    }
    #[test]
    fn bounded_invalid_inputs() {
        for size in 0..128 {
            assert!(parse(&vec![0; size]).is_err());
        }
        assert!(parse(&[3, 0, 8, 0, 8, 0, 0, 0]).is_err());
    }
}
