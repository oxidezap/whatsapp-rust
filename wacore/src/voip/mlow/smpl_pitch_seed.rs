//! Build-from-seed for the MLow pitch runtime tables. The expanded pitch table is the expansion of a
//! small packed seed (the blocksegs bitstream + the index maps + the DCMF arrays), so we store the
//! seed and rerun the init at load: range-decode of the blocksegs (via `RangeDecoder`), the
//! per-track contour expansion, and the integer CDF expansion. Only the 20 ms / 8-subframe config is
//! built (the active MLow path).

use super::rangecoder::RangeDecoder;
use super::smpl_pitch_enc::{BlockSeg, BlockTrack, NUM_SUBFRAMES, PitchTables};

const NUM_BLOCKSEGS: usize = 217;
const NUM_BLOCKTRACKS: usize = 187;
const PITCH_NUM_BLOCKS: usize = 9;

/// On-disk packed pitch ROM (`tables.proto` `PitchSeed`). `bytes` fields reshape row-major at build.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct PitchSeed {
    #[prost(bytes = "vec", tag = "1")]
    blocksegs_bitstream: Vec<u8>, // 676; range-decoder source
    #[prost(bytes = "vec", tag = "2")]
    blocksegs2idx: Vec<u8>, // [217]
    #[prost(bytes = "vec", tag = "3")]
    blocksegs_ix: Vec<u8>, // [187][2]
    #[prost(bytes = "vec", tag = "4")]
    firstblock_range: Vec<u8>, // [9][2]
    #[prost(bytes = "vec", tag = "5")]
    blockseg_idx_dcmf: Vec<u8>, // [217]
    #[prost(bytes = "vec", tag = "6")]
    delta_lag_dcmfs: Vec<u8>, // [3][319]
    #[prost(bytes = "vec", tag = "7")]
    block_transition_dcmf: Vec<u8>, // [9][9]
}

/// Decode a uniform symbol in `[0, N)`.
fn ec_decode_uniform(dec: &mut RangeDecoder, n: u32) -> u32 {
    let cmf_low = dec.decode(n);
    dec.update(cmf_low, cmf_low + 1, n);
    cmf_low
}

/// Decode one blockseg: `len = uniform(6)+1`, then `len` pairs of `(uniform(9), uniform(4)+1)`.
fn decode_blocksegs(dec: &mut RangeDecoder) -> BlockSeg {
    const N_LEN: u32 = 6;
    const N_BLOCK: u32 = 9;
    const N_SEGLEN: u32 = 4;
    let len = (ec_decode_uniform(dec, N_LEN) + 1) as usize;
    let mut blocks = Vec::with_capacity(len);
    let mut seglens = Vec::with_capacity(len);
    for _ in 0..len {
        blocks.push(ec_decode_uniform(dec, N_BLOCK) as usize);
        seglens.push((ec_decode_uniform(dec, N_SEGLEN) + 1) as usize);
    }
    BlockSeg {
        nblocks: len,
        blocks,
        seglens,
    }
}

/// Expand each track's blockseg into the per-subframe track + mean/deltas.
fn gen_blocktrack(blocksegs: &[BlockSeg], blocksegs_ix: &[[usize; 2]]) -> Vec<BlockTrack> {
    let mut out = Vec::with_capacity(NUM_BLOCKTRACKS);
    for track_idx in 0..NUM_BLOCKTRACKS {
        let seg = &blocksegs[blocksegs_ix[track_idx][0]];
        let mut track = [0usize; NUM_SUBFRAMES];
        let mut seg_idx = 0usize;
        let mut meanblock = 0.0f32;
        let mut trackdeltas = 0.0f32;
        for b in 0..seg.nblocks {
            for _ in 0..seg.seglens[b] {
                track[seg_idx] = seg.blocks[b];
                seg_idx += 1;
            }
            meanblock += (seg.blocks[b] * seg.seglens[b]) as f32;
            if b != 0 {
                trackdeltas += (seg.blocks[b - 1] as i32 - seg.blocks[b] as i32).abs() as f32;
            }
        }
        meanblock /= NUM_SUBFRAMES as f32;
        out.push(BlockTrack {
            track,
            meanblock,
            trackdeltas,
        });
    }
    out
}

/// Integer expansion of a DCMF to a cumulative CDF of length `len+1`.
fn dcmf_to_cmf(dcmf: &[u8]) -> Vec<u32> {
    let dcmf_len = dcmf.len();
    let mut cmf = vec![0u32; dcmf_len + 1];
    let mut sum: i64 = 0;
    for n in 0..dcmf_len {
        let mut tmp = dcmf[n] as i32 + 1;
        tmp *= tmp;
        if tmp > 65535 {
            tmp = 65535;
        }
        cmf[n + 1] = tmp as u32;
        sum += tmp as i64;
    }
    cmf[0] = 0;
    for n in 1..dcmf_len + 1 {
        let prev = cmf[n - 1] as i64;
        let add = (cmf[n] as i64 * (32767 - dcmf_len as i64)) / sum + 1;
        cmf[n] = (prev + add) as u32;
    }
    cmf
}

impl PitchSeed {
    pub(crate) fn build(&self) -> PitchTables {
        let mut dec = RangeDecoder::new(&self.blocksegs_bitstream);
        let mut blocksegs = Vec::with_capacity(NUM_BLOCKSEGS);
        for _ in 0..NUM_BLOCKSEGS {
            blocksegs.push(decode_blocksegs(&mut dec));
        }

        let blocksegs_ix: Vec<[usize; 2]> = self
            .blocksegs_ix
            .chunks_exact(2)
            .map(|p| [p[0] as usize, p[1] as usize])
            .collect();
        let firstblock_range: Vec<[usize; 2]> = self
            .firstblock_range
            .chunks_exact(2)
            .map(|p| [p[0] as usize, p[1] as usize])
            .collect();

        let blocktracks = gen_blocktrack(&blocksegs, &blocksegs_ix);

        let blocksegs2idx: Vec<usize> = self.blocksegs2idx.iter().map(|&x| x as usize).collect();
        let blockseg_idx_cmf = dcmf_to_cmf(&self.blockseg_idx_dcmf);
        let delta_lag_cmfs: Vec<Vec<u32>> = self
            .delta_lag_dcmfs
            .chunks_exact(319)
            .map(dcmf_to_cmf)
            .collect();
        let block_transition_cmf: Vec<Vec<u32>> = self
            .block_transition_dcmf
            .chunks_exact(PITCH_NUM_BLOCKS)
            .map(dcmf_to_cmf)
            .collect();

        debug_assert_eq!(block_transition_cmf.len(), PITCH_NUM_BLOCKS);

        PitchTables::from_parts(
            blocksegs,
            blocktracks,
            blocksegs2idx,
            blockseg_idx_cmf,
            delta_lag_cmfs,
            blocksegs_ix,
            firstblock_range,
            block_transition_cmf,
        )
    }
}

/// The pitch lag/contour heap window built from the same seed: 217 per-contour records, the contour
/// map, the lag/frac/delta CDFs, and the delta-lag bounds; all expansions of the blocksegs bitstream
/// + index maps + DCMFs. `smpl_mem` lays these out at the fixed WASM addresses Group D reads.
pub(crate) struct ContourWindowParts {
    /// Per contour: (base/seg lags = blockseg `blocks`, seg lens = blockseg `seglens`).
    pub(crate) records: Vec<(Vec<usize>, Vec<usize>)>,
    /// `contour_map[217]` (== `blocksegs2idx`).
    pub(crate) contour_map: Vec<u8>,
    /// Delta-lag bounds (== `firstblock_range` flattened).
    pub(crate) firstblock_range: Vec<[usize; 2]>,
    /// Primary-lag CDF (`dcmf_to_cmf(blockseg_idx_dcmf)`), 218 entries.
    pub(crate) lag_cdf: Vec<u32>,
    /// 3 fractional-lag CDFs (`dcmf_to_cmf(delta_lag_dcmfs)`), 320 entries each.
    pub(crate) frac_cmfs: Vec<Vec<u32>>,
    /// 9 delta-lag transition CDFs (`dcmf_to_cmf(block_transition_dcmf)`), 10 entries each.
    pub(crate) delta_cmfs: Vec<Vec<u32>>,
}

impl PitchSeed {
    /// Build the contour-window half: re-decode the blocksegs and expand the index maps + DCMFs into
    /// the exact tables Group D's pointer-chase reads.
    pub(crate) fn build_contour_window(&self) -> ContourWindowParts {
        let mut dec = RangeDecoder::new(&self.blocksegs_bitstream);
        let mut records = Vec::with_capacity(NUM_BLOCKSEGS);
        for _ in 0..NUM_BLOCKSEGS {
            let bs = decode_blocksegs(&mut dec);
            records.push((bs.blocks, bs.seglens));
        }
        let firstblock_range = self
            .firstblock_range
            .chunks_exact(2)
            .map(|p| [p[0] as usize, p[1] as usize])
            .collect();
        ContourWindowParts {
            records,
            contour_map: self.blocksegs2idx.clone(),
            firstblock_range,
            lag_cdf: dcmf_to_cmf(&self.blockseg_idx_dcmf),
            frac_cmfs: self
                .delta_lag_dcmfs
                .chunks_exact(319)
                .map(dcmf_to_cmf)
                .collect(),
            delta_cmfs: self
                .block_transition_dcmf
                .chunks_exact(PITCH_NUM_BLOCKS)
                .map(dcmf_to_cmf)
                .collect(),
        }
    }
}

#[cfg(test)]
pub(crate) fn seed_from_json(s: &str) -> PitchSeed {
    #[derive(serde::Deserialize)]
    struct RawSeed {
        blocksegs_bitstream: Vec<u8>,
        blocksegs2idx: Vec<u8>,
        blocksegs_ix: Vec<Vec<u8>>,
        firstblock_range: Vec<Vec<u8>>,
        blockseg_idx_dcmf: Vec<u8>,
        delta_lag_dcmfs: Vec<Vec<u8>>,
        block_transition_dcmf: Vec<Vec<u8>>,
    }
    let r: RawSeed = serde_json::from_str(s).expect("pitch_seed.json");
    let flat = |v: &[Vec<u8>]| -> Vec<u8> { v.iter().flatten().copied().collect() };
    PitchSeed {
        blocksegs_bitstream: r.blocksegs_bitstream,
        blocksegs2idx: r.blocksegs2idx,
        blocksegs_ix: flat(&r.blocksegs_ix),
        firstblock_range: flat(&r.firstblock_range),
        blockseg_idx_dcmf: r.blockseg_idx_dcmf,
        delta_lag_dcmfs: flat(&r.delta_lag_dcmfs),
        block_transition_dcmf: flat(&r.block_transition_dcmf),
    }
}

#[cfg(test)]
mod tests {
    use super::super::smpl_pitch_enc::load_pitch_tables;

    /// Permanent no-regression tripwire for the pitch init: a checksum over the seed-built tables.
    #[test]
    fn pitch_seed_build_golden_checksum() {
        let built = load_pitch_tables();
        assert_eq!(
            built.debug_checksum(),
            0x21ce_965d_809c_8048,
            "pitch seed-built struct checksum"
        );
    }
}
