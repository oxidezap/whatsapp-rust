//! SFrame varint-decode shootout, in the VoIP context it actually runs in.
//!
//! A reviewer flagged the byte-at-a-time `decode_varint` in `voip::sframe` as
//! "very inefficient" (it runs per received media packet during a call),
//! pointing at protobuf's unrolled `CodedInputStream.readRawVarint64` and the
//! `varint-simd` crate. This bench answers, with numbers, whether either helps
//! *the SFrame path* — not a synthetic varint stream.
//!
//! What SFrame actually decodes (see `voip::sframe`): each inbound packet's
//! trailing header is `[counter_varint || key_id_varint || total_len_byte]`, and
//! `parse_sframe_header` decodes the two varints from a body that is only **2–4
//! bytes** long (counter sweeps 1→3 bytes over a call, key_id stays 1 byte).
//! Two consequences this bench measures:
//!   1. `varint-simd`'s 8-byte bulk load can't apply to a 2–4 byte body without
//!      reading OOB, so the SIMD path falls back to scalar here — it is not the
//!      free win the crate is on multi-MB arrays.
//!   2. The header parse is a sliver of the per-packet AES-128-GCM decrypt, so
//!      even deleting it entirely barely moves the per-packet cost.
//!
//! Strategies (identical contract to the production `decode_varint`:
//! `(data, offset) -> (value, new_offset)`):
//!   * `loop`: the current production byte-at-a-time loop (the baseline).
//!   * `fastpath`: a 1-byte fast path, then the loop.
//!   * `unrolled`: protobuf `CodedInputStream`-style (no loop counter / shift var).
//!   * `pext`: `varint-simd`-style 8-byte bulk load + BMI2 `pext`; falls back to scalar for the <8-byte bodies SFrame always has. Real `pext` only with `-C target-cpu=native`.
//!
//! Run:  cargo bench -p wacore --features voip --bench sframe_varint_benchmark

use divan::counter::ItemsCount;
use divan::{Bencher, black_box};
// `SframeSession` is off the public `voip` facade (doc-hidden); the in-tree bench reaches it via its
// source module, like voip_benchmark does.
use wacore::voip::sframe::SframeSession;

fn main() {
    verify_candidates();
    divan::main();
}

// --- Call fixtures (mirror voip_benchmark) ---

const SELF_LID: &str = "111111111111111:0@lid";
const PEER_LID: &str = "222222222222222:0@lid";
/// Representative encrypted-audio payload size (a 60ms MLow/Opus frame is ~tens
/// to ~150 bytes). GCM cost scales with this; the header parse does not.
const PAYLOAD_LEN: usize = 96;

fn call_key() -> Vec<u8> {
    (0u8..32).collect()
}

// ---------------------------------------------------------------------------
// Candidate decoders. Contract matches production `voip::sframe::decode_varint`
// exactly: decode the varint starting at `offset`, return (value, new_offset).
// ---------------------------------------------------------------------------

/// Baseline: the current production loop, copied verbatim from `voip::sframe`.
#[inline]
fn decode_loop(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut i = offset;
    while i < data.len() {
        let b = data[i];
        i += 1;
        value |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((value, i));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

/// Single-byte fast path, then the loop.
#[inline]
fn decode_fastpath(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    let &first = data.get(offset)?;
    if first < 0x80 {
        return Some((first as u64, offset + 1));
    }
    let mut value = (first & 0x7f) as u64;
    let mut shift = 7u32;
    let mut i = offset + 1;
    while i < data.len() {
        let b = data[i];
        i += 1;
        value |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((value, i));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

/// protobuf `CodedInputStream`-style: fully unrolled over a slice, no loop
/// counter or running shift variable. Wrapped to the (data, offset) contract.
#[inline]
fn decode_unrolled(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    let s = data.get(offset..)?;
    let (v, n) = unrolled_core(s)?;
    Some((v, offset + n))
}

#[inline]
fn unrolled_core(data: &[u8]) -> Option<(u64, usize)> {
    let b0 = *data.first()?;
    if b0 < 0x80 {
        return Some((b0 as u64, 1));
    }
    let mut value = (b0 & 0x7f) as u64;
    let b1 = *data.get(1)?;
    value |= ((b1 & 0x7f) as u64) << 7;
    if b1 < 0x80 {
        return Some((value, 2));
    }
    let b2 = *data.get(2)?;
    value |= ((b2 & 0x7f) as u64) << 14;
    if b2 < 0x80 {
        return Some((value, 3));
    }
    let b3 = *data.get(3)?;
    value |= ((b3 & 0x7f) as u64) << 21;
    if b3 < 0x80 {
        return Some((value, 4));
    }
    let b4 = *data.get(4)?;
    value |= ((b4 & 0x7f) as u64) << 28;
    if b4 < 0x80 {
        return Some((value, 5));
    }
    let b5 = *data.get(5)?;
    value |= ((b5 & 0x7f) as u64) << 35;
    if b5 < 0x80 {
        return Some((value, 6));
    }
    let b6 = *data.get(6)?;
    value |= ((b6 & 0x7f) as u64) << 42;
    if b6 < 0x80 {
        return Some((value, 7));
    }
    let b7 = *data.get(7)?;
    value |= ((b7 & 0x7f) as u64) << 49;
    if b7 < 0x80 {
        return Some((value, 8));
    }
    let b8 = *data.get(8)?;
    value |= ((b8 & 0x7f) as u64) << 56;
    if b8 < 0x80 {
        return Some((value, 9));
    }
    // 10th byte: only bit 63 is valid (64 - 9*7 = 1); reject overflow.
    let b9 = *data.get(9)?;
    if b9 > 1 {
        return None;
    }
    value |= (b9 as u64) << 63;
    Some((value, 10))
}

/// `varint-simd`-style bulk decode. For SFrame this almost always hits the
/// `< 8` fallback because the header body is 2–4 bytes; that *is* the finding.
#[inline]
fn decode_pext(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    let s = data.get(offset..)?;
    let (v, n) = pext_core(s)?;
    Some((v, offset + n))
}

#[inline]
fn pext_core(data: &[u8]) -> Option<(u64, usize)> {
    #[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
    {
        if data.len() >= 8 {
            // SAFETY: bmi2 statically enabled; length checked.
            return unsafe { pext_bmi2(data) };
        }
    }
    unrolled_core(data)
}

#[cfg(all(target_arch = "x86_64", target_feature = "bmi2"))]
#[target_feature(enable = "bmi2")]
unsafe fn pext_bmi2(data: &[u8]) -> Option<(u64, usize)> {
    use core::arch::x86_64::_pext_u64;
    const CONT: u64 = 0x8080_8080_8080_8080;
    const LOW7: u64 = 0x7f7f_7f7f_7f7f_7f7f;

    let word = u64::from_le_bytes(data[..8].try_into().unwrap());
    let terminators = !word & CONT;
    if terminators == 0 {
        return unrolled_core(data); // 9/10-byte varint
    }
    let nbytes = (terminators.trailing_zeros() / 8) as usize + 1;
    let keep = if nbytes == 8 {
        u64::MAX
    } else {
        (1u64 << (nbytes * 8)) - 1
    };
    let value = _pext_u64(word & keep & LOW7, LOW7);
    Some((value, nbytes))
}

// ---------------------------------------------------------------------------
// SFrame header reproduction (the production functions are private). Verified
// against round-trips in `verify_candidates`.
// ---------------------------------------------------------------------------

fn enc_varint(out: &mut Vec<u8>, mut v: u64) {
    while v > 0x7f {
        out.push(((v & 0x7f) | 0x80) as u8);
        v >>= 7;
    }
    out.push((v & 0xff) as u8);
}

fn build_sframe_header(counter: u64, key_id: u64) -> Vec<u8> {
    let mut header = Vec::with_capacity(6);
    enc_varint(&mut header, counter);
    enc_varint(&mut header, key_id);
    header.push((header.len() + 1) as u8);
    header
}

/// Exactly `voip::sframe::parse_sframe_header`, but with a pluggable decoder.
#[inline]
fn parse_header_with(
    header: &[u8],
    decode: impl Fn(&[u8], usize) -> Option<(u64, usize)>,
) -> Option<(u64, u64)> {
    if header.len() < 2 {
        return None;
    }
    let total_len = *header.last().unwrap() as usize;
    if total_len != header.len() {
        return None;
    }
    let body = &header[..header.len() - 1];
    let (counter, next) = decode(body, 0)?;
    let (key_id, _) = decode(body, next)?;
    Some((counter, key_id))
}

// ---------------------------------------------------------------------------
// Corpus: the SFrame headers of one long call. Counter sweeps 0..N (so the body
// is the real 1→2→3-byte mix a call produces at 50 packets/s), key_id = 0.
// ---------------------------------------------------------------------------

/// ~16 minutes of one call's packets at 50/s. Counter 0..N spans 1-, 2- and
/// 3-byte bodies in the same proportion a real call hits them.
const N_PACKETS: usize = 48_000;

fn build_call_headers() -> Vec<Vec<u8>> {
    (0..N_PACKETS as u64)
        .map(|counter| build_sframe_header(counter, 0))
        .collect()
}

// ---------------------------------------------------------------------------
// Benches. `header_parse_*` isolate the varint/header work; `sframe_full_decrypt`
// is the real per-packet cost (GCM + the same parse) for the ratio that matters.
// ---------------------------------------------------------------------------

macro_rules! header_parse_bench {
    ($name:ident, $decode:path) => {
        #[divan::bench]
        fn $name(bencher: Bencher) {
            bencher
                .counter(ItemsCount::new(N_PACKETS))
                .with_inputs(build_call_headers)
                .bench_refs(|headers| {
                    let mut acc = 0u64;
                    for h in headers.iter() {
                        let (counter, key_id) = parse_header_with(h, $decode).unwrap();
                        acc = acc.wrapping_add(counter ^ key_id);
                    }
                    black_box(acc)
                });
        }
    };
}

header_parse_bench!(header_parse_loop, decode_loop);
header_parse_bench!(header_parse_fastpath, decode_fastpath);
header_parse_bench!(header_parse_unrolled, decode_unrolled);
header_parse_bench!(header_parse_pext, decode_pext);

/// The real per-packet inbound cost: `SframeSession::decrypt` = strip trailing
/// header + `parse_sframe_header` (the varints above) + AES-128-GCM decrypt.
/// Reported per packet, so it's directly comparable to `header_parse_*`: the gap
/// between the two is the ceiling on any varint optimization for VoIP.
const DECRYPT_BATCH: usize = 2_000;

#[divan::bench]
fn sframe_full_decrypt(bencher: Bencher) {
    bencher
        .counter(ItemsCount::new(DECRYPT_BATCH))
        .with_inputs(|| {
            let mut peer = SframeSession::new(&call_key(), PEER_LID, SELF_LID).unwrap();
            let payload = vec![0xA5u8; PAYLOAD_LEN];
            // Counters 0..BATCH: the real small-counter regime of a call.
            let frames: Vec<Vec<u8>> = (0..DECRYPT_BATCH).map(|_| peer.encrypt(&payload)).collect();
            let rx = SframeSession::new(&call_key(), SELF_LID, PEER_LID).unwrap();
            (rx, frames)
        })
        .bench_refs(|(rx, frames)| {
            for f in frames.iter() {
                black_box(rx.decrypt(black_box(f)));
            }
        });
}

// ---------------------------------------------------------------------------
// Correctness gate: every strategy must round-trip real SFrame headers, or the
// bench aborts rather than report a fast-but-wrong decoder.
// ---------------------------------------------------------------------------

fn verify_candidates() {
    let mut counter: u64 = 1;
    for _ in 0..60_000 {
        // Sweep counters across 1..6 byte encodings; key_id small like real calls.
        counter = counter.wrapping_mul(2_654_435_761).wrapping_add(1) & ((1 << 42) - 1);
        let key_id = counter & 0x7f;
        let header = build_sframe_header(counter, key_id);
        let want = Some((counter, key_id));
        for (label, got) in [
            ("loop", parse_header_with(&header, decode_loop)),
            ("fastpath", parse_header_with(&header, decode_fastpath)),
            ("unrolled", parse_header_with(&header, decode_unrolled)),
            ("pext", parse_header_with(&header, decode_pext)),
        ] {
            assert_eq!(
                got, want,
                "decoder `{label}` mis-parsed header for {want:?}"
            );
        }
    }
}
