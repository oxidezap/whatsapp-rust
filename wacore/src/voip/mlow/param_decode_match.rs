//! Invariant: the Rust unvoiced/voiced parameter decode produces the SAME per-subframe
//! `nrgres_dbq_Q14` and `fcbg_idx` as the reference (`gennoise_params_dump.json`).
//!
//! The Rust gains decode (`decode_smpl_gains`) reads the same bits as the reference unvoiced decode,
//! just under different field names: its `gain_q` IS the `nrgres_dbq_Q14` and its per-subframe
//! `nrg_res` symbol IS the `fcbg_idx`. The voiced FCB gain index is the pitch block's `filt_idx`.
//! This test pins that correspondence exactly so the excitation/gen_noise inputs stay faithful.
#![cfg(test)]

use super::decoder::diag_decode_params;
use serde_json::Value;
use std::collections::HashMap;

#[test]
fn nrgres_fcbg_match_c_reference() {
    let cdump: Value =
        serde_json::from_str(include_str!("testdata/gennoise_params_dump.json")).unwrap();
    let carr = cdump.as_array().unwrap();
    let mut cmap: HashMap<(i64, i64, i64), &Value> = HashMap::new();
    for c in carr {
        cmap.insert(
            (
                c["packet"].as_i64().unwrap(),
                c["frame"].as_i64().unwrap(),
                c["sf"].as_i64().unwrap(),
            ),
            c,
        );
    }
    assert_eq!(
        cmap.len(),
        carr.len(),
        "duplicate (packet, frame, sf) keys in the C param dump would hide coverage"
    );

    let rust = diag_decode_params();
    let (mut uv_nrgres, mut uv_fcbg, mut v_fcbg, mut voiced_class) = (0, 0, 0, 0);
    for r in &rust {
        let Some(c) = cmap.get(&(r.packet as i64, r.frame as i64, r.sf as i64)) else {
            continue;
        };
        let cv = c["voiced"].as_i64().unwrap() == 1;
        let cnrg = c["nrgres_dbq_Q14"].as_i64().unwrap() as i32;
        let cfcbg = c["fcbg_idx"].as_i64().unwrap() as i32;
        let cnp = c["sf_pulses"].as_i64().unwrap() as i32;

        assert_eq!(
            r.voiced,
            cv,
            "voiced flag at {:?}",
            (r.packet, r.frame, r.sf)
        );
        voiced_class += 1;
        if cv {
            // Voiced: the FCB gain index (filt_idx) must match the reference fcbg_idx where pulses exist.
            if cnp > 0 {
                assert_eq!(
                    r.fcbg_idx,
                    cfcbg,
                    "voiced fcbg_idx at {:?}",
                    (r.packet, r.frame, r.sf)
                );
                v_fcbg += 1;
            }
        } else {
            assert_eq!(
                r.nrgres_dbq_q14,
                cnrg,
                "unvoiced nrgres_dbq_Q14 at {:?}",
                (r.packet, r.frame, r.sf)
            );
            uv_nrgres += 1;
            if cnp > 0 {
                assert_eq!(
                    r.fcbg_idx,
                    cfcbg,
                    "unvoiced fcbg_idx at {:?}",
                    (r.packet, r.frame, r.sf)
                );
                uv_fcbg += 1;
            }
        }
    }
    assert!(
        voiced_class > 0 && uv_nrgres > 0 && uv_fcbg > 0 && v_fcbg > 0,
        "coverage too thin: class={voiced_class} uv_nrgres={uv_nrgres} uv_fcbg={uv_fcbg} v_fcbg={v_fcbg}"
    );
}
