use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

#[derive(Deserialize)]
struct Tokens {
    single_byte: Vec<String>,
    double_byte: Vec<Vec<String>>,
}

/// Must mirror lookup_bytes in src/token.rs. For len <= 4 the padded word is
/// the key itself (injective because tokens are NUL-free); longer tokens get
/// a first4/last4/len signature that the runtime byte-verifies on match.
fn token_key(b: &[u8]) -> u32 {
    match *b {
        [a] => a as u32,
        [a, b] => u32::from_le_bytes([a, b, 0, 0]),
        [a, b, c] => u32::from_le_bytes([a, b, c, 0]),
        [a, b, c, d] => u32::from_le_bytes([a, b, c, d]),
        _ => {
            let len = b.len();
            let head = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
            let tail = u32::from_le_bytes([b[len - 4], b[len - 3], b[len - 2], b[len - 1]]);
            head.rotate_left(11) ^ tail ^ (len as u32).wrapping_mul(0x9E37_79B1)
        }
    }
}

/// Linear-probing layout: power-of-two size at <= 75% load, multiplier chosen
/// deterministically so the worst probe chain stays short. Misses stop at the
/// first empty (0) slot.
fn build_table(entries: &[(u32, u16, u16)]) -> (u32, Vec<u32>, Vec<u16>, Vec<u16>) {
    let mut k = usize::BITS - (entries.len() * 4 / 3).leading_zeros();
    loop {
        let size = 1usize << k;
        let mut mul: u32 = 0x9E37_79B1;
        for _ in 0..256 {
            let mut keys = vec![0u32; size];
            let mut meta = vec![0u16; size];
            let mut off = vec![0u16; size];
            'insert: {
                for &(x, m, o) in entries {
                    let mut h = (x.wrapping_mul(mul) >> (32 - k)) as usize;
                    let mut disp = 0usize;
                    while keys[h] != 0 {
                        h = (h + 1) & (size - 1);
                        disp += 1;
                        if disp > 8 {
                            break 'insert;
                        }
                    }
                    keys[h] = x;
                    meta[h] = m;
                    off[h] = o;
                }
                return (mul, keys, meta, off);
            }
            mul = mul.wrapping_mul(0x85EB_CA6B) | 1;
        }
        k += 1;
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/tokens.json");
    println!("cargo:rerun-if-changed=build.rs");

    let path = Path::new(&env::var("OUT_DIR")?).join("token_maps.rs");
    let mut file = BufWriter::new(File::create(&path)?);

    let tokens_json = fs::read_to_string("src/tokens.json")?;
    let tokens: Tokens = serde_json::from_str(&tokens_json)?;

    // kind encoding (u16): Single(i) => i; Double(d, i) => (d + 1) * 256 + i.
    let mut values: Vec<(String, u16)> = Vec::new();
    let mut seen: HashMap<String, u16> = HashMap::new();

    for (i, token) in tokens.single_byte.iter().enumerate() {
        if !token.is_empty() {
            let kind = u16::try_from(i)?;
            assert!(kind < 256, "single-byte token index out of range");
            if let Some(existing) = seen.get(token) {
                panic!("duplicate token {:?}: {} vs {}", token, existing, kind);
            }
            seen.insert(token.clone(), kind);
            values.push((token.clone(), kind));
        }
    }

    for (dict_idx, dict) in tokens.double_byte.iter().enumerate() {
        for (token_idx, token) in dict.iter().enumerate() {
            if !token.is_empty() {
                // The packed kind offers no compile-time range check (the old
                // generated TokenKind::Double literals did), so guard here:
                // an overflowing index would silently bleed into the dict bits.
                assert!(token_idx < 256, "double-byte token index out of range");
                assert!(dict_idx + 1 < 256, "double-byte dict index out of range");
                let kind = u16::try_from((dict_idx + 1) * 256 + token_idx)?;
                if let Some(existing) = seen.get(token) {
                    panic!("duplicate token {:?}: {} vs {}", token, existing, kind);
                }
                seen.insert(token.clone(), kind);
                values.push((token.clone(), kind));
            }
        }
    }

    // One open-addressing table over every token, replacing the previous
    // code-generated (hashify::tiny_map) compare chains (~55 KiB of .text)
    // with ~25 KiB of .rodata. The key is a u32: for len <= 4 the padded
    // bytes themselves (exact — tokens are NUL-free, so padding can't alias
    // real bytes, and the meta word's length bits reject "a" vs "a\0"),
    // otherwise a first4/last4/len mix that the runtime byte-verifies via
    // TOK_OFF into TOK_BLOB. Misses stop at the first empty slot, so the
    // encode path's dominant miss traffic costs one multiply and one or two
    // loads — a full-key PHF over the bytes was measured worse precisely
    // because it made misses pay the whole hash.
    let max_len = values.iter().map(|(t, _)| t.len()).max().unwrap_or(0);
    assert!(max_len < 256, "length no longer fits blob prefix byte");

    // meta layout: bits 0..11 kind, bits 11..16 tag. Tags 0-3 are exact short
    // lengths (len - 1); tag 31 marks a long token whose length is the blob
    // prefix byte (48-byte tokens exist, so length can't live in the meta).
    let mut blob: Vec<u8> = Vec::new();
    let mut entries: Vec<(u32, u16, u16)> = Vec::new();
    for (t, k) in &values {
        let b = t.as_bytes();
        assert!(!b.contains(&0), "token {t:?} contains NUL");
        assert!(*k < (1 << 11), "kind overflows meta word");
        let x = token_key(b);
        assert!(x != 0, "token key collides with empty-slot sentinel");
        let (tag, off) = if b.len() <= 4 {
            (b.len() as u16 - 1, 0)
        } else {
            let o = u16::try_from(blob.len())?;
            blob.push(b.len() as u8);
            blob.extend_from_slice(b);
            (31, o)
        };
        entries.push((x, *k | (tag << 11), off));
    }

    let (tok_mul, tok_keys, tok_meta, tok_off) = build_table(&entries);
    writeln!(file, "const MAX_TOKEN_LEN: usize = {max_len};")?;
    writeln!(file, "const TOK_MUL: u32 = {tok_mul};")?;
    writeln!(
        file,
        "const TOK_SHIFT: u32 = {};",
        32 - tok_keys.len().trailing_zeros()
    )?;
    writeln!(file, "const TOK_MASK: usize = {};", tok_keys.len() - 1)?;
    writeln!(
        file,
        "static TOK_KEYS: [u32; {}] = {:?};",
        tok_keys.len(),
        tok_keys
    )?;
    writeln!(
        file,
        "static TOK_META: [u16; {}] = {:?};",
        tok_meta.len(),
        tok_meta
    )?;
    writeln!(
        file,
        "static TOK_OFF: [u16; {}] = {:?};",
        tok_off.len(),
        tok_off
    )?;
    writeln!(file, "static TOK_BLOB: [u8; {}] = {:?};", blob.len(), blob)?;

    // Decode arrays: index → string
    writeln!(file, "\nstatic SINGLE_BYTE_TOKENS: &[&str] = &[")?;
    for token in &tokens.single_byte {
        writeln!(file, "    {:?},", token)?;
    }
    writeln!(file, "];")?;

    writeln!(file, "\nstatic DOUBLE_BYTE_TOKENS: &[&[&str]] = &[")?;
    for dict in &tokens.double_byte {
        writeln!(file, "    &[")?;
        for token in dict {
            writeln!(file, "        {:?},", token)?;
        }
        writeln!(file, "    ],")?;
    }
    writeln!(file, "];")?;

    Ok(())
}
