//! Tiny ART entry class generated directly from Rust. No Java source/compiler.
use anyhow::{Result, ensure};
use sha1::{Digest, Sha1};
use std::collections::BTreeMap;

fn word(out: &mut Vec<u8>, n: u32) {
    out.extend(n.to_le_bytes());
}
fn half(out: &mut Vec<u8>, n: u16) {
    out.extend(n.to_le_bytes());
}
fn leb(out: &mut Vec<u8>, mut n: u32) {
    loop {
        let b = (n & 127) as u8;
        n >>= 7;
        out.push(b | if n != 0 { 128 } else { 0 });
        if n == 0 {
            break;
        }
    }
}
fn align(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}
fn patch(out: &mut [u8], p: usize, n: u32) {
    out[p..p + 4].copy_from_slice(&n.to_le_bytes());
}

/// Emit one class whose initializer loads the explicit native library and whose
/// native `main(String[])` transfers control to Rust. Original APK classes are
/// neither rewritten nor generated here. The supplied path is literal DEX data.
pub fn android_bootstrap(library: &str) -> Result<Vec<u8>> {
    ensure!(
        library.starts_with("/data/local/tmp/mobile-oracle/")
            && library.len() <= 512
            && std::path::Path::new(library)
                .components()
                .all(|component| { !matches!(component, std::path::Component::ParentDir) })
            && library.bytes().all(|b| (0x21..=0x7e).contains(&b)),
        "invalid private library path"
    );
    let mut strings = vec![
        "<clinit>",
        "Ljava/lang/Object;",
        "Ljava/lang/String;",
        "Ljava/lang/System;",
        "Lorg/oxidezap/oracle/RustRunner;",
        "V",
        "VL",
        "[Ljava/lang/String;",
        "load",
        "main",
        library,
    ];
    strings.sort_unstable();
    strings.dedup();
    let ids: BTreeMap<_, _> = strings
        .iter()
        .enumerate()
        .map(|(i, s)| (*s, i as u32))
        .collect();
    let mut types = vec![
        "Ljava/lang/Object;",
        "Ljava/lang/String;",
        "Ljava/lang/System;",
        "Lorg/oxidezap/oracle/RustRunner;",
        "V",
        "[Ljava/lang/String;",
    ];
    types.sort_by_key(|s| ids[s]);
    let tids: BTreeMap<_, _> = types
        .iter()
        .enumerate()
        .map(|(i, s)| (*s, i as u16))
        .collect();
    let mut out = vec![0u8; 112];
    let sid_off = out.len();
    out.resize(out.len() + strings.len() * 4, 0);
    let type_off = out.len();
    for t in &types {
        word(&mut out, ids[t]);
    }
    let proto_off = out.len();
    out.resize(out.len() + 36, 0);
    let method_off = out.len();
    // method_ids sorted by class, name, prototype: System.load, <clinit>, main.
    half(&mut out, tids["Ljava/lang/System;"]);
    half(&mut out, 1);
    word(&mut out, ids["load"]);
    half(&mut out, tids["Lorg/oxidezap/oracle/RustRunner;"]);
    half(&mut out, 0);
    word(&mut out, ids["<clinit>"]);
    half(&mut out, tids["Lorg/oxidezap/oracle/RustRunner;"]);
    half(&mut out, 2);
    word(&mut out, ids["main"]);
    let class_off = out.len();
    out.resize(out.len() + 32, 0);
    align(&mut out);
    let data_off = out.len();
    let strings_off = out.len();
    for (i, s) in strings.iter().enumerate() {
        let p = out.len() as u32;
        patch(&mut out, sid_off + i * 4, p);
        leb(&mut out, s.len() as u32);
        out.extend(s.as_bytes());
        out.push(0);
    }
    align(&mut out);
    let lists_off = out.len();
    word(&mut out, 1);
    half(&mut out, tids["Ljava/lang/String;"]);
    half(&mut out, 0);
    let array_list = out.len();
    word(&mut out, 1);
    half(&mut out, tids["[Ljava/lang/String;"]);
    half(&mut out, 0);
    for (i, (shorty, params)) in [("V", 0), ("VL", lists_off), ("VL", array_list)]
        .into_iter()
        .enumerate()
    {
        patch(&mut out, proto_off + i * 12, ids[shorty]);
        patch(&mut out, proto_off + i * 12 + 4, u32::from(tids["V"]));
        patch(&mut out, proto_off + i * 12 + 8, params as u32);
    }
    let code_off = out.len();
    half(&mut out, 1);
    half(&mut out, 0);
    half(&mut out, 1);
    half(&mut out, 0);
    word(&mut out, 0);
    word(&mut out, 6);
    for unit in [0x001a, ids[library] as u16, 0x1071, 0, 0, 0x000e] {
        half(&mut out, unit);
    }
    let class_data = out.len();
    for n in [0, 0, 2, 0, 1, 0x10008, code_off as u32, 1, 0x109, 0] {
        leb(&mut out, n);
    }
    patch(
        &mut out,
        class_off,
        u32::from(tids["Lorg/oxidezap/oracle/RustRunner;"]),
    );
    patch(&mut out, class_off + 4, 0x11);
    patch(
        &mut out,
        class_off + 8,
        u32::from(tids["Ljava/lang/Object;"]),
    );
    patch(&mut out, class_off + 16, u32::MAX);
    patch(&mut out, class_off + 24, class_data as u32);
    align(&mut out);
    let map_off = out.len();
    let map = [
        (0u16, 1, 0),
        (1, strings.len(), sid_off),
        (2, types.len(), type_off),
        (3, 3, proto_off),
        (5, 3, method_off),
        (6, 1, class_off),
        (0x2002, strings.len(), strings_off),
        (0x1001, 2, lists_off),
        (0x2001, 1, code_off),
        (0x2000, 1, class_data),
        (0x1000, 1, map_off),
    ];
    word(&mut out, map.len() as u32);
    for (ty, count, offset) in map {
        half(&mut out, ty);
        half(&mut out, 0);
        word(&mut out, count as u32);
        word(&mut out, offset as u32);
    }
    out[..8].copy_from_slice(b"dex\n035\0");
    let len = out.len();
    for (p, n) in [
        (32, len),
        (36, 112),
        (40, 0x12345678),
        (52, map_off),
        (56, strings.len()),
        (60, sid_off),
        (64, types.len()),
        (68, type_off),
        (72, 3),
        (76, proto_off),
        (88, 3),
        (92, method_off),
        (96, 1),
        (100, class_off),
        (104, len - data_off),
        (108, data_off),
    ] {
        patch(&mut out, p, n as u32);
    }
    let digest = Sha1::digest(&out[32..]);
    out[12..32].copy_from_slice(&digest);
    let (mut a, mut b) = (1u32, 0u32);
    for byte in &out[12..] {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    patch(&mut out, 8, (b << 16) | a);
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_is_deterministic_and_rejects_nonprivate_paths() {
        let path = "/data/local/tmp/mobile-oracle/libmobile_oracle_android.so";
        let dex = android_bootstrap(path).unwrap();
        assert_eq!(dex, android_bootstrap(path).unwrap());
        assert_eq!(&dex[..8], b"dex\n035\0");
        assert_eq!(
            u32::from_le_bytes(dex[32..36].try_into().unwrap()) as usize,
            dex.len()
        );
        assert_eq!(&dex[12..32], Sha1::digest(&dex[32..]).as_slice());
        assert!(android_bootstrap("/tmp/not-private.so").is_err());
        assert!(android_bootstrap("/data/local/tmp/mobile-oracle/../other.so").is_err());
        assert!(android_bootstrap("/data/local/tmp/mobile-oracle/\n.so").is_err());
    }
}
