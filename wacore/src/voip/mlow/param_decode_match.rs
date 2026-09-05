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
    let cdump: Value = crate::voip::mlow::fixture::decode(include_bytes!(
        "testdata/gennoise_params_dump.cbor.zst"
    ))
    .unwrap();
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

#[test]
fn all_wire_parameters_match_shipped_wasm() {
    use super::rangecoder::RangeDecoder;
    use super::smpl_cc_tables::load_cc_tables;
    use super::smpl_decode::{SmplLsfState, decode_smpl_lsf, load_smpl_tables};
    use super::smpl_gains::decode_smpl_gains;
    use super::smpl_mem::load_smpl_mem;
    use super::smpl_pitch::decode_smpl_pitch;
    use super::smpl_pulse::decode_smpl_pulses;
    use super::toc::parse_mlow_toc;

    let frames: Vec<String> =
        serde_json::from_str(include_str!("testdata/wasm_derived_frames.json")).unwrap();
    let records: Value =
        crate::voip::mlow::fixture::decode(include_bytes!("testdata/wasm_params.cbor.zst"))
            .expect("wasm wire parameters");
    let records = records.as_array().unwrap();
    assert_eq!(records.len(), frames.len() * 3);
    let ints = |v: &Value| {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap() as i32)
            .collect::<Vec<_>>()
    };
    let tables = load_smpl_tables();
    let cc = load_cc_tables();
    let mem = load_smpl_mem();
    let mut state = SmplLsfState::default();
    let mut voiced_count = 0;
    let mut unvoiced_count = 0;
    for (packet, hex) in frames.iter().enumerate() {
        let frame = hex::decode(hex).unwrap();
        let toc = parse_mlow_toc(frame[0]);
        let mut decoder = RangeDecoder::new(&frame[1..]);
        for internal in 0..3 {
            let r = &records[packet * 3 + internal];
            assert_eq!(r["len"].as_i64().unwrap(), 320);
            assert_eq!(r["subframes"].as_i64().unwrap(), 4);
            assert_eq!(r["frame"].as_u64().unwrap() as usize, internal);
            assert_eq!(r["cav"].as_i64().unwrap() != 0, toc.active);
            let lsf = decode_smpl_lsf(&mut decoder, tables, &mut state, 0, internal, toc.active);
            assert_eq!(
                lsf.stage1,
                r["voiced"].as_i64().unwrap() as i32,
                "packet {packet}/{internal}: voiced"
            );
            let mut indices = vec![lsf.grid];
            indices.extend(lsf.stage2);
            assert_eq!(indices, ints(&r["lsf"]), "packet {packet}/{internal}: LSF");
            assert_eq!(
                lsf.extra,
                r["interp"].as_i64().unwrap() as i32,
                "packet {packet}/{internal}: interpolation"
            );
            let pulses = decode_smpl_pulses(
                &mut decoder,
                cc,
                320,
                4,
                i32::from(toc.active),
                0,
                lsf.stage1,
            );
            assert_eq!(
                pulses.pulses,
                ints(&r["pulses"]),
                "packet {packet}/{internal}: pulse positions/magnitudes"
            );
            assert_eq!(
                pulses.subfr.to_vec(),
                ints(&r["sf_pulses"]),
                "packet {packet}/{internal}: pulse counts"
            );
            let fcbg = if lsf.stage1 == 1 {
                voiced_count += 1;
                let pitch =
                    decode_smpl_pitch(&mut decoder, mem, cc, &mut state, 320, 4, 0, pulses.subfr);
                assert_eq!(
                    pitch.gain_idx.to_vec(),
                    ints(&r["acbg"]),
                    "packet {packet}/{internal}: ACB gain"
                );
                assert_eq!(
                    pitch.block_lags.to_vec(),
                    ints(&r["lags"]),
                    "packet {packet}/{internal}: pitch lags"
                );
                assert_eq!(
                    pitch.contour,
                    r["contour"].as_i64().unwrap() as i32,
                    "packet {packet}/{internal}: contour"
                );
                pitch.filt_idx
            } else {
                unvoiced_count += 1;
                let gains = decode_smpl_gains(&mut decoder, cc, 4, pulses.subfr);
                assert_eq!(
                    gains.gain_q.to_vec(),
                    ints(&r["energy_q14"]),
                    "packet {packet}/{internal}: gain Q14"
                );
                gains.nrg_res
            };
            for (sf, expected) in ints(&r["fcbg"]).iter().enumerate() {
                if pulses.subfr[sf] > 0 {
                    assert_eq!(
                        fcbg[sf], *expected,
                        "packet {packet}/{internal}/{sf}: FCB gain"
                    );
                }
            }
            let range = r["range"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap() as u32)
                .collect::<Vec<_>>();
            assert_eq!(
                decoder.oracle_state().to_vec(),
                range,
                "packet {packet}/{internal}: entropy context"
            );
        }
    }
    assert!(voiced_count > 0 && unvoiced_count > 0);
}
