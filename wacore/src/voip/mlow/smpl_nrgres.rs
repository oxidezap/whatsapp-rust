//! MLow unvoiced residual-energy quantizer. The unvoiced excitation LEVEL is carried entirely by the
//! per-subframe quantized residual-energy floor (`nrgres_dbq_Q14`): a frame-mean scalar quant plus a
//! shape VQ. Our decoder reads this floor back as `gain_q` (validated bit-exact in
//! `param_decode_match`), so the encoder must produce the same `nrgres_dbq_Q14` for the round-trip
//! level to be right.
//!
//! The reconstruction is validated against the reference dump (the `nrgres_dbq_Q14` test). It is
//! wired live in `analysis.rs`'s unvoiced path: the wire gain block IS the nrgres layout
//! (`gain_main`==`nrgres_frame_qi`, `gain_delta`==`nrgres_shape_qi`, the gain table == the shape
//! codebook, `cb1` == the frame step), so `gain_q[sf]` decodes back as `nrgres_dbq_Q14`.
#![allow(dead_code)] // serde-only fields + test-only reconstruction helpers
#![allow(clippy::needless_range_loop)]

const SMPL_RES_NRG_BIAS: f32 = 3.1622776e-9;
const SMPL_RES_NRG_MIN_DB: f32 = -85.0;
const SMPL_RES_NRG_MAX_DB: f32 = 0.0;
/// `smpl_nrg_step_db_Q14[2]` (the 4-subframe table index).
const SMPL_NRG_STEP_DB_Q14_4: i32 = 16686;
const SMPL_RES_NRG_SHAPE_CB_N_4: usize = 98;

/// `nrgres_shape_CB_4_Q10` (98 vectors x 4 subframes), stored verbatim.
#[rustfmt::skip]
const NRGRES_SHAPE_CB_4_Q10: [i16; SMPL_RES_NRG_SHAPE_CB_N_4 * 4] = [
    -2515, -2238, 2632, 2121, 790, 3973, -2872, -1891, -533, 2847, 1453, -3767, -6174, -402, 2668, 3908,
    -1623, -1458, 153, 2928, -1254, 3197, -476, -1467, 1803, -1086, 270, -987, 1952, -66, -1257, -629,
    161, 19, -85, -96, 4833, 3147, -105, -7875, -1320, 1377, -1156, 1099, 3398, -2247, 1485, -2637,
    -3031, 2756, 1841, -1566, -1487, 2202, -2668, 1954, 5518, -5344, 522, -696, 8400, -3123, -6235, 958,
    5152, -2444, -2811, 102, 2513, -82, 1181, -3612, -561, -197, -1074, 1832, -294, -1250, -1839, 3383,
    5126, 522, -782, -4866, -7760, -5178, -1840, 14779, -1119, 6007, -1489, -3399, -4567, -2543, 1855, 5255,
    53, -1626, 67, 1506, -12256, -7706, -1982, 21943, 3549, -969, -1096, -1484, -10824, 2981, 2204, 5639,
    -229, 1106, 945, -1821, -9237, 10157, 1616, -2537, 4916, -199, -2177, -2540, 6673, 984, -3355, -4302,
    -7130, -4677, 8925, 2882, 445, 2762, -348, -2859, -196, -1859, 1761, 294, 2725, -2093, -966, 334,
    -3908, -308, 3675, 541, 735, 890, -2516, 891, 504, 1631, -1157, -977, -17817, 2119, 7104, 8594,
    -2056, 1897, -198, 356, 292, -4544, -287, 4538, -1455, -304, 603, 1156, -18259, -12643, 15247, 15655,
    4177, 1778, -1815, -4140, 1425, 576, -294, -1707, -1301, 5132, 2838, -6669, -4727, -3148, -905, 8781,
    -650, 152, -4654, 5152, 13746, 2320, -6259, -9807, -1356, 396, 3789, -2829, 2337, 1947, -29, -4256,
    6033, 820, -5730, -1123, -1795, 1091, 1080, -377, 2208, -1921, -3314, 3027, 9688, 5218, -3754, -11152,
    3814, -3941, -6183, 6310, -1017, -2391, 4393, -984, 10944, -1182, -5011, -4751, -4640, 7201, -218, -2343,
    -1278, 4720, -4212, 770, 2777, 1333, -5944, 1833, -16066, 8107, 5165, 2795, 2530, -5020, 6073, -3582,
    -2111, -7534, 4575, 5070, -8702, -3762, 4050, 8414, 1335, -997, -1567, 1229, 9348, 1534, -3959, -6922,
    2440, 1153, -2175, -1418, -2715, -4538, -4478, 11730, 569, -885, 2032, -1716, 3529, -91, -3218, -219,
    2157, -4121, 191, 1772, -2123, -1968, -1355, 5446, 1475, -354, 3651, -4772, 1654, -3521, 2726, -859,
    2393, 6820, -2958, -6255, -3861, 1365, 1177, 1319, 7614, -1638, -2789, -3187, -3628, -2635, 6902, -639,
    1925, 2295, -1451, -2769, -3683, 4517, -981, 147, -1260, -529, 2339, -550, 3013, 639, -1050, -2602,
    3651, 1959, -3218, -2391, 6267, 3124, -2926, -6464, -8180, 3900, 4191, 89, -3372, -611, 1042, 2941,
    -2510, 856, -925, 2579, -11667, -8436, 10605, 9498, 6427, -2733, 1887, -5581, 1581, -1722, -328, 469,
    2011, 1989, -3606, -394, -1014, 2197, -1200, 17, 1544, -2555, 765, 247, 1188, -183, 1966, -2972,
    -6057, 3480, -2284, 4860, -25659, 8466, 8891, 8303,
];

/// Result of `smpl_quant_nrg_res` for the 4-subframe path.
pub(crate) struct NrgResQuant {
    pub frame_qi: i32,
    pub shape_qi: i32,
    /// Per-subframe quantized residual-energy floor (Q14); the decoder reads this as `gain_q`.
    pub dbq_q14: [i32; 4],
}

/// Residual-energy quantizer for num_subfr == 4. `nrgres[sf]` is the per-subframe residual energy
/// `nrg(reslpc_sf)/subfrlen` in the SAME (int16-scaled) domain `reslpc` lives in.
pub(crate) fn quant_nrg_res_4(nrgres: &[f32; 4]) -> NrgResQuant {
    let mut nrgres_db = [0.0f32; 4];
    let mut frame_db = 0.0f32;
    for i in 0..4 {
        nrgres_db[i] = (10.0 * (nrgres[i] + SMPL_RES_NRG_BIAS).log10()).min(SMPL_RES_NRG_MAX_DB);
        frame_db += nrgres_db[i];
    }
    frame_db /= 4.0;
    let sc_q14 = 1.0f32 / (1i32 << 14) as f32;
    let frame_qi = ((frame_db - SMPL_RES_NRG_MIN_DB) / (sc_q14 * SMPL_NRG_STEP_DB_Q14_4 as f32))
        .round() as i32;
    let mut frame_dbq_q14 = frame_qi * SMPL_NRG_STEP_DB_Q14_4;
    frame_dbq_q14 += (SMPL_RES_NRG_MIN_DB as i32) * (1 << 14);
    for i in 0..4 {
        nrgres_db[i] -= frame_dbq_q14 as f32 * sc_q14;
    }
    // Shape VQ (min RD) over the 98 codebook vectors.
    let sc_q10 = 1.0f32 / (1i32 << 10) as f32;
    let mut best_rd = 1e30f32;
    let mut qi = 0usize;
    for n in 0..SMPL_RES_NRG_SHAPE_CB_N_4 {
        let mut rd = 0.0f32;
        for i in 0..4 {
            let d = nrgres_db[i] - NRGRES_SHAPE_CB_4_Q10[n * 4 + i] as f32 * sc_q10;
            rd += d * d;
        }
        if rd < best_rd {
            qi = n;
            best_rd = rd;
        }
    }
    let mut dbq_q14 = [0i32; 4];
    for i in 0..4 {
        dbq_q14[i] = frame_dbq_q14 + (NRGRES_SHAPE_CB_4_Q10[qi * 4 + i] as i32) * 16;
    }
    NrgResQuant {
        frame_qi,
        shape_qi: qi as i32,
        dbq_q14,
    }
}

/// Reconstruct the per-subframe `nrgres_dbq_Q14` from a frame/shape index pair, matching the unvoiced
/// decode reconstruction. Used to validate the quantizer against the reference dump.
fn dbq_from_indices(frame_qi: i32, shape_qi: usize) -> [i32; 4] {
    let frame_dbq = frame_qi * SMPL_NRG_STEP_DB_Q14_4 + (SMPL_RES_NRG_MIN_DB as i32) * (1 << 14);
    std::array::from_fn(|sf| frame_dbq + (NRGRES_SHAPE_CB_4_Q10[shape_qi * 4 + sf] as i32) * 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The reconstruction must match the reference dump: for the first internal frame the committed
    // nrgres_frame_qi=0, nrgres_shape_qi=8 yield these exact per-subframe nrgres_dbq_Q14 (which the
    // decoder reads back as gain_q).
    #[test]
    fn dbq_reconstruction_matches_c_dump() {
        assert_eq!(
            dbq_from_indices(0, 8),
            [-1390064, -1392336, -1394000, -1394176]
        );
    }

    // Round-trip: quantizing a residual energy and reconstructing from the resulting indices must agree
    // with the direct per-subframe dbq the quantizer reports.
    #[test]
    fn quant_then_reconstruct_consistent() {
        let nrgres = [0.01f32, 0.02, 0.005, 0.03];
        let q = quant_nrg_res_4(&nrgres);
        assert_eq!(q.dbq_q14, dbq_from_indices(q.frame_qi, q.shape_qi as usize));
    }
}
