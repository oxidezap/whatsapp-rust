//! MLow heap window for the pitch lag/contour reads (`smpl_pitch.rs` Group D), the only consumer
//! still addressing the WASM heap by absolute pointer. Groups A/B/C/E moved to the logical `CcTables`
//! (`cc_seed.bin`), as did the p6!=0 LR gain/filter/weight tables. The contour window proper (pcfg
//! header + per-contour records, lag/frac/delta CDFs, contour_map, delta-lag bounds) is BUILT from
//! `pitch_seed.bin` at load (`build_smpl_mem`), so nothing of it is stored. Everything sits at the
//! original absolute addresses so the existing pointer arithmetic still lands.

use std::sync::OnceLock;

/// `tables.proto` Region.
#[derive(Clone, PartialEq, prost::Message)]
struct SmplMemRegion {
    #[prost(uint32, tag = "1")]
    base: u32,
    #[prost(bytes = "vec", tag = "2")]
    data: Vec<u8>,
}

/// `tables.proto` HeapWindow; the runtime window built by `build_smpl_mem`.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct SmplMem {
    #[prost(message, repeated, tag = "1")]
    regions: Vec<SmplMemRegion>,
    #[prost(uint32, tag = "2")]
    pub(crate) g_cc: u32,
    #[prost(uint32, tag = "3")]
    pub(crate) g_nrg: u32,
    #[prost(uint32, tag = "4")]
    pub(crate) g_pitch: u32,
    #[prost(uint32, tag = "5")]
    pub(crate) g_clk: u32,
}

static SMPL_MEM: OnceLock<SmplMem> = OnceLock::new();

/// Parse the full heap-window JSON dump into a `SmplMem` (the generator's carve source and the
/// byte-identical oracle while Group D still reads the heap).
#[cfg(test)]
pub(crate) fn parse_smpl_mem_json(s: &str) -> SmplMem {
    #[derive(serde::Deserialize)]
    struct RawRegion {
        base: u32,
        b64: String,
    }
    #[derive(serde::Deserialize)]
    struct Raw {
        regions: Vec<RawRegion>,
        g_cc: u32,
        g_nrg: u32,
        g_pitch: u32,
        clk: u32,
    }
    use base64::Engine;
    let raw: Raw = serde_json::from_str(s).expect("smpl_cc_blob.json must parse");
    let engine = base64::engine::general_purpose::STANDARD;
    let regions = raw
        .regions
        .into_iter()
        .map(|r| SmplMemRegion {
            base: r.base,
            data: engine.decode(r.b64).expect("smpl_cc_blob region b64"),
        })
        .collect();
    SmplMem {
        regions,
        g_cc: raw.g_cc,
        g_nrg: raw.g_nrg,
        g_pitch: raw.g_pitch,
        g_clk: raw.clk,
    }
}

// Fixed WASM-build globals for the Group-D heap layout. The window is built at these absolute
// addresses so `smpl_pitch.rs`'s pointer arithmetic lands unchanged.
const G_CLK: u32 = 0xb9f9a8;
const G_PITCH: u32 = 0xb9d378;
const PCFG: u32 = G_CLK.wrapping_add(0x5704);
// pcfg header pointers (@pcfg+0x56e0): num_contours, contour_map, lag_cdf, frac_base, then three the
// consumer never reads, then delta_cdf. Fixed addresses for this build.
const HDR_CONTOUR_MAP: u32 = 0xe7c10;
const HDR_LAG_CDF: u32 = 0xbaa7b0;
const HDR_FRAC_BASE: u32 = 0xbaa9be;
const HDR_DELTA_CDF: u32 = 0xbab13e;
const HDR_UNUSED: [u32; 3] = [0xe7d20, 0xe7ef0, 0xe8096];
const DELTA_BOUNDS_ADDR: u32 = 0xe7ef0;
const NUM_CONTOURS: usize = 217;

/// Build the pitch lag/contour window from the seed. Reproduces the carved window byte-for-byte at
/// every address the consumer reads.
fn build_smpl_mem() -> SmplMem {
    let seed: super::smpl_pitch_seed::PitchSeed =
        super::smpl_tables_blob::load_blob_prost(include_bytes!("testdata/pitch_seed.bin"));
    let w = seed.build_contour_window();

    let mut regions = Vec::with_capacity(6);
    let push = |regions: &mut Vec<SmplMemRegion>, base: u32, data: Vec<u8>| {
        regions.push(SmplMemRegion { base, data });
    };

    // region 0 @pcfg+0x1d38: 217 records (lags[8] | seglens[8] | seg_count, 0x44 each), a 4-byte
    // gap (=NUM_BLOCKTRACKS), then the 8-pointer header.
    let mut r0 = Vec::with_capacity(NUM_CONTOURS * 0x44 + 4 + 32);
    for (lags, seglens) in &w.records {
        let sc = lags.len();
        for i in 0..8 {
            r0.extend_from_slice(&(*lags.get(i).unwrap_or(&0) as i32).to_le_bytes());
        }
        for i in 0..8 {
            r0.extend_from_slice(&(*seglens.get(i).unwrap_or(&0) as i32).to_le_bytes());
        }
        r0.extend_from_slice(&(sc as i32).to_le_bytes());
    }
    r0.extend_from_slice(&187u32.to_le_bytes());
    for h in [
        NUM_CONTOURS as u32,
        HDR_CONTOUR_MAP,
        HDR_LAG_CDF,
        HDR_FRAC_BASE,
        HDR_UNUSED[0],
        HDR_UNUSED[1],
        HDR_UNUSED[2],
        HDR_DELTA_CDF,
    ] {
        r0.extend_from_slice(&h.to_le_bytes());
    }
    push(&mut regions, PCFG.wrapping_add(0x1d38), r0);

    // CDF tables (each its own region; the dead inter-table gaps the carve included are unread).
    let u16_bytes =
        |v: &[u32]| -> Vec<u8> { v.iter().flat_map(|&x| (x as u16).to_le_bytes()).collect() };
    push(&mut regions, HDR_LAG_CDF, u16_bytes(&w.lag_cdf));
    let frac: Vec<u32> = w.frac_cmfs.iter().flatten().copied().collect();
    push(&mut regions, HDR_FRAC_BASE, u16_bytes(&frac));
    let delta: Vec<u32> = w.delta_cmfs.iter().flatten().copied().collect();
    push(&mut regions, HDR_DELTA_CDF, u16_bytes(&delta));

    // contour_map[217].
    push(&mut regions, HDR_CONTOUR_MAP, w.contour_map.clone());

    // delta-lag bounds: firstblock_range u8 pairs, then the carve's 2 trailing pad bytes.
    let mut bounds: Vec<u8> = w
        .firstblock_range
        .iter()
        .flat_map(|&[lo, hi]| [lo as u8, hi as u8])
        .collect();
    bounds.extend_from_slice(&[0, 0]);
    push(&mut regions, DELTA_BOUNDS_ADDR, bounds);

    SmplMem {
        regions,
        g_cc: 0,
        g_nrg: 0,
        g_pitch: G_PITCH,
        g_clk: G_CLK,
    }
}

pub(crate) fn load_smpl_mem() -> &'static SmplMem {
    SMPL_MEM.get_or_init(build_smpl_mem)
}

impl SmplMem {
    /// Region containing `[addr, addr+n)` and the byte offset of `addr` within it, or `None`.
    fn region_for(&self, addr: u32, n: usize) -> Option<(&[u8], usize)> {
        for r in &self.regions {
            if addr >= r.base && (addr - r.base) as usize + n <= r.data.len() {
                return Some((&r.data, (addr - r.base) as usize));
            }
        }
        None
    }

    pub(crate) fn u8(&self, addr: u32) -> u8 {
        self.region_for(addr, 1).map_or(0, |(d, off)| d[off])
    }

    pub(crate) fn u16(&self, addr: u32) -> u16 {
        self.region_for(addr, 2)
            .map_or(0, |(d, off)| u16::from_le_bytes([d[off], d[off + 1]]))
    }

    pub(crate) fn i16(&self, addr: u32) -> i16 {
        self.u16(addr) as i16
    }

    pub(crate) fn u32(&self, addr: u32) -> u32 {
        self.region_for(addr, 4).map_or(0, |(d, off)| {
            u32::from_le_bytes([d[off], d[off + 1], d[off + 2], d[off + 3]])
        })
    }

    pub(crate) fn i32(&self, addr: u32) -> i32 {
        self.u32(addr) as i32
    }

    /// Materialize the n-entry cumulative u16 CDF at WASM address `addr` (for `decode_cdf`). Entries
    /// outside the window read as 0, matching the WASM's out-of-region fallback.
    pub(crate) fn cdf_at(&self, addr: u32, n: usize) -> Vec<u16> {
        (0..n)
            .map(|i| self.u16(addr.wrapping_add((i as u32) * 2)))
            .collect()
    }

    /// Raw `[addr, addr+2n)` byte slice when the whole CDF window sits inside one region (the common
    /// case), so callers can read it in place instead of allocating a `Vec<u16>`. `None` at a region
    /// boundary or out of range, where the zero-fill semantics of `cdf_at` must apply; callers fall
    /// back to `cdf_at` there.
    pub(crate) fn cdf_bytes(&self, addr: u32, n: usize) -> Option<&[u8]> {
        let (data, off) = self.region_for(addr, n * 2)?;
        Some(&data[off..off + n * 2])
    }
}

/// Load the full heap dump from the (gitignored) `smpl_cc_blob.json` oracle, or `None` if absent
/// (CI has only the carved `.bin`; the byte-identical gates run where the JSON lives).
#[cfg(test)]
pub(crate) fn try_load_full_heap() -> Option<SmplMem> {
    let s = std::fs::read_to_string("src/voip/mlow/testdata/smpl_cc_blob.json").ok()?;
    Some(parse_smpl_mem_json(&s))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Byte-identical gate: the window BUILT FROM THE SEED (`build_smpl_mem`) must equal the full
    /// heap dump at every Group-D contour read (the pcfg pointer-chase). A read that the build gets
    /// wrong, or one that falls outside the built regions (reads 0 where the blob had data), would
    /// silently corrupt the bitstream. The p6!=0 LR gain/filter/weight tables moved to `CcTables`
    /// and are gated there. Skipped when the JSON oracle is absent (CI).
    ///
    /// DOES NOT RUN IN CI (oracle gitignored); the CI guard for these tables is
    /// `lsf_seed_build_golden_checksums` + `golden_roundtrip`. A green CI does not imply this ran.
    #[test]
    fn pitch_contour_window_complete_vs_heap() {
        let Some(full) = try_load_full_heap() else {
            println!("smpl_cc_blob.json absent; skipping completeness gate");
            return;
        };
        let window = load_smpl_mem();
        let gclk = full.g_clk;

        // Every concrete Group-D read lands inside a built region and equals the full heap there.
        let pcfg = gclk.wrapping_add(0x5704);
        let chk_u8 = |a: u32| assert_eq!(window.u8(a), full.u8(a), "u8 miss @ {a:#x}");
        let chk_u32 = |a: u32| assert_eq!(window.u32(a), full.u32(a), "u32 miss @ {a:#x}");
        let chk_cdf = |a: u32, n: usize| {
            assert_eq!(window.cdf_at(a, n), full.cdf_at(a, n), "cdf miss @ {a:#x}")
        };

        // pcfg header pointers.
        for off in [22240u32, 22244, 22248, 22252, 22268] {
            chk_u32(pcfg.wrapping_add(off));
        }
        let num_contours = full.u32(pcfg.wrapping_add(22240)) as i32;
        let contour_map = full.u32(pcfg.wrapping_add(22244));
        let lag_cdf = full.u32(pcfg.wrapping_add(22248));
        let frac_base = full.u32(pcfg.wrapping_add(22252));
        let delta_cdf = full.u32(pcfg.wrapping_add(22268));

        // primary-lag CDFs: absolute (n=num_contours+1) and all delta sub-slices.
        chk_cdf(lag_cdf, (num_contours + 1).max(0) as usize);
        for di in 0..10u32 {
            let lo = full.u8(0xe7ef0u32.wrapping_add(di * 2)) as i32;
            let hi = full.u8(0xe7ef0u32.wrapping_add(di * 2 + 1)) as i32;
            chk_u8(0xe7ef0u32.wrapping_add(di * 2));
            chk_u8(0xe7ef0u32.wrapping_add(di * 2 + 1));
            let r_n = (hi - lo) + 2;
            if r_n >= 2 {
                chk_cdf(lag_cdf.wrapping_add((lo as u32) * 2), r_n as usize);
            }
        }

        // delta_cdf rows for every prev_lag the contour base-lags can produce.
        let mut max_prev = 0i32;
        for c in 0..num_contours {
            let ctr_base = pcfg.wrapping_add((c as u32) * 0x44);
            let sc = full.i32(ctr_base.wrapping_add(0x1d78)).max(1);
            for s in 0..sc {
                max_prev = max_prev.max(full.i32(ctr_base.wrapping_add(0x1d38) + (s as u32) * 4));
            }
        }
        for prev in 0..=max_prev {
            chk_cdf(delta_cdf.wrapping_add((prev as u32) * 20), 10);
        }

        // contour_map[0..217].
        for i in 0..217u32 {
            chk_u8(contour_map.wrapping_add(i));
        }

        // per-contour records + frac CDFs.
        for c in 0..num_contours {
            let ctr_base = pcfg.wrapping_add((c as u32) * 0x44);
            let sc = full.i32(ctr_base.wrapping_add(0x1d78));
            chk_u32(ctr_base.wrapping_add(0x1d78));
            chk_u32(ctr_base.wrapping_add(0x1d38)); // base lag
            chk_u32(ctr_base.wrapping_add(0x1d58)); // seg-len 0
            for s in 0..sc.max(0) {
                chk_u32(ctr_base.wrapping_add(0x1d38) + (s as u32) * 4);
                chk_u32(ctr_base.wrapping_add(0x1d58) + (s as u32) * 4);
            }
            // frac CDFs: every seg_sel * every reachable nl2 offset within the 0x280 table.
            for seg_sel in 0..3u32 {
                let frac_seg_base = frac_base.wrapping_add(seg_sel * 0x280);
                for nl2 in 0..0x40i32 {
                    let off = frac_seg_base
                        .wrapping_add((nl2 * 2) as u32)
                        .wrapping_add(0xfe);
                    chk_cdf(off, 65);
                }
            }
        }
    }
}
