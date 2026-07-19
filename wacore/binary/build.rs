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

/// Pick the pair of byte positions that best splits the same-length tokens
/// (smallest worst-case bucket). Two positions instead of one because the
/// fat length groups (3-4 bytes, ~280 tokens each, mostly numeric) collide
/// heavily on any single byte.
fn best_disc_pair(tokens: &[&[u8]], len: usize) -> (u8, u8) {
    let mut best = (usize::MAX, (0u8, 0u8));
    for p1 in 0..len {
        // p2 == p1 is deliberate: for groups a single byte already splits
        // best, the pair degenerates to that byte repeated.
        for p2 in p1..len {
            let mut buckets: HashMap<(u8, u8), usize> = HashMap::new();
            let mut worst = 0;
            for t in tokens {
                let c = buckets.entry((t[p1], t[p2])).or_insert(0);
                *c += 1;
                worst = worst.max(*c);
            }
            if worst < best.0 {
                best = (worst, (p1 as u8, p2 as u8));
            }
        }
    }
    best.1
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
                let kind = u16::try_from((dict_idx + 1) * 256 + token_idx)?;
                if let Some(existing) = seen.get(token) {
                    panic!("duplicate token {:?}: {} vs {}", token, existing, kind);
                }
                seen.insert(token.clone(), kind);
                values.push((token.clone(), kind));
            }
        }
    }

    // Length-bucketed, data-driven lookup. Same probe shape as the previous
    // code-generated (hashify::tiny_map) lookup — reject by length, then by a
    // discriminator, full compare only on near-hits — but as ~16 KiB of
    // static tables instead of ~55 KiB of generated compare chains, and still
    // with no full-key hash (the encode path probes every stanza string and
    // most miss, so misses must stay nearly free; a PTHash map was measured
    // worse here for exactly that reason).
    //
    // Layout: entries sorted by (len, disc_key, bytes). Per length L the
    // entry index range is LEN_START[L]..LEN_START[L+1] and the token bytes
    // live at LEN_BLOB_START[L] + (i - LEN_START[L]) * L, so no per-entry
    // offset table is needed. disc_key = key[p1] << 8 | key[p2] with (p1, p2)
    // chosen per length at build time for the smallest worst-case bucket.
    let max_len = values.iter().map(|(t, _)| t.len()).max().unwrap_or(0);

    let mut by_len: Vec<Vec<(&[u8], u16)>> = vec![Vec::new(); max_len + 1];
    for (t, k) in &values {
        by_len[t.len()].push((t.as_bytes(), *k));
    }

    let mut disc_pos: Vec<(u8, u8)> = vec![(0, 0); max_len + 1];
    for (len, group) in by_len.iter().enumerate().skip(1) {
        if !group.is_empty() {
            let toks: Vec<&[u8]> = group.iter().map(|(t, _)| *t).collect();
            disc_pos[len] = best_disc_pair(&toks, len);
        }
    }

    let mut len_start: Vec<u16> = Vec::with_capacity(max_len + 2);
    let mut len_blob_start: Vec<u16> = Vec::with_capacity(max_len + 2);
    let mut disc_keys: Vec<u16> = Vec::new();
    let mut kinds: Vec<u16> = Vec::new();
    let mut blob: Vec<u8> = Vec::new();
    // Direct-indexed p1-byte range tables, so a probe whose p1 byte matches no
    // token of that length dies in a couple of loads (the dominant miss case)
    // instead of a binary search. Per length: the occurring p1-byte span
    // [min, max] plus span+1 cumulative entry offsets, all concatenated.
    let mut d1_min: Vec<u8> = vec![0; max_len + 1];
    let mut d1_table_start: Vec<u16> = Vec::with_capacity(max_len + 2);
    let mut d1_off: Vec<u16> = Vec::new();

    for (len, group) in by_len.iter_mut().enumerate() {
        len_start.push(u16::try_from(disc_keys.len())?);
        len_blob_start.push(u16::try_from(blob.len())?);
        d1_table_start.push(u16::try_from(d1_off.len())?);
        let (p1, p2) = (disc_pos[len].0 as usize, disc_pos[len].1 as usize);
        group.sort_by_key(|(t, _)| (((t[p1] as u16) << 8) | t[p2] as u16, *t));

        if !group.is_empty() {
            let min = group.iter().map(|(t, _)| t[p1]).min().unwrap();
            let max = group.iter().map(|(t, _)| t[p1]).max().unwrap();
            d1_min[len] = min;
            // Cumulative offsets, relative to this length's first entry.
            let mut cum = 0u16;
            for b in min..=max {
                d1_off.push(cum);
                cum += group.iter().filter(|(t, _)| t[p1] == b).count() as u16;
            }
            d1_off.push(cum);
        }

        for (t, k) in group.iter() {
            disc_keys.push(((t[p1] as u16) << 8) | t[p2] as u16);
            kinds.push(*k);
            blob.extend_from_slice(t);
        }
    }
    len_start.push(u16::try_from(disc_keys.len())?);
    len_blob_start.push(u16::try_from(blob.len())?);
    d1_table_start.push(u16::try_from(d1_off.len())?);

    // One 12-byte header per length so a probe's dependent-load chain is one
    // cache line for all per-length metadata, then the offset pair, then the
    // candidate bytes.
    writeln!(file, "const MAX_TOKEN_LEN: usize = {max_len};")?;
    writeln!(file, "static LEN_HDR: [LenHdr; {}] = [", max_len + 1)?;
    for len in 0..=max_len {
        let span_plus_1 = d1_table_start[len + 1] - d1_table_start[len];
        writeln!(
            file,
            "    LenHdr {{ p1: {}, p2: {}, d1_min: {}, span_plus_1: {}, d1_table: {}, entry_start: {}, blob_start: {} }},",
            disc_pos[len].0,
            disc_pos[len].1,
            d1_min[len],
            span_plus_1,
            d1_table_start[len],
            len_start[len],
            len_blob_start[len],
        )?;
    }
    writeln!(file, "];")?;
    writeln!(
        file,
        "static DISC_KEYS: [u16; {}] = {:?};",
        disc_keys.len(),
        disc_keys
    )?;
    writeln!(file, "static KINDS: [u16; {}] = {:?};", kinds.len(), kinds)?;
    writeln!(file, "static BLOB: [u8; {}] = {:?};", blob.len(), blob)?;
    writeln!(
        file,
        "static D1_OFF: [u16; {}] = {:?};",
        d1_off.len(),
        d1_off
    )?;

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
