//! Snapshot layouts are decoded once here; both repositories consume these assembled artifacts.
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::path::Path;
use xtask_support::{unpack, write, write_json};

struct Reader<'a>(&'a Path);
impl Reader<'_> {
    fn raw(&self, name: &str, i: usize) -> Result<Vec<u8>> {
        Ok(std::fs::read(self.0.join(format!("{name}_{i:04}.bin")))?)
    }
    fn array(&self, name: &str, i: usize, format: &str) -> Result<Vec<Value>> {
        unpack(&self.raw(name, i)?, format)
    }
    fn scalar(&self, name: &str, i: usize, format: &str) -> Result<Value> {
        self.array(name, i, format)?
            .into_iter()
            .next()
            .context("empty scalar")
    }
    fn int(&self, name: &str, i: usize) -> Result<i64> {
        self.scalar(name, i, "i")?
            .as_i64()
            .context("integer scalar")
    }
}
fn section(bytes: &[u8], at: usize, len: usize, format: &str) -> Result<Vec<Value>> {
    unpack(
        bytes.get(at..at + len).context("truncated snapshot")?,
        format,
    )
}
fn noise_state(bytes: &[u8]) -> Result<Value> {
    let v = unpack(bytes, "11f3i")?;
    Ok(
        json!({"env_smth":v[0],"env_last":v[1],"out_state_uv":v[2..4],"out_state_v":v[4..6],"corr_smth":v[6..9],"shape_state":v[9..11],"prev_voiced":v[11],"since_unvoiced":v[12],"rand_seed":v[13]}),
    )
}
fn records(count: usize, mut build: impl FnMut(usize) -> Result<Value>) -> Result<Value> {
    Ok(Value::Array(
        (0..count).map(&mut build).collect::<Result<_>>()?,
    ))
}
fn fe(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(330, |i| {
        let frame = r.int("fe_frame", i)?;
        ensure!(frame == i as i64 % 3, "FE frame ordering");
        Ok(
            json!({"pkt":i/3,"numframe":frame,"lpcbuf":r.array("fe_input",i,"448f")?,"windowed":r.array("fe_window",i,"448f")?,"A_before_bwe":r.array("fe_a_raw",i,"17f")?,"A":r.array("fe_a",i,"17f")?,"R":r.array("fe_r",i,"17d")?,"F2":r.array("fe_f2",i,"257f")?}),
        )
    })
}
fn signal(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(330, |i| {
        let inputs = r.array("sig_input", i, "3f")?;
        Ok(
            json!({"frame":i,"pitchcorr":inputs[2],"avg_lag":inputs[1],"harm":inputs[0],"lags":r.array("sig_lags",i,"8f")?,"F2":r.array("sig_f2",i,"257f")?,"sp_act_prob":r.scalar("sig_spact",i,"f")?,"cav":r.int("sig_cav",i)?,"vuv_in":r.array("sig_vuv_in",i,"4f")?,"vuv_out":r.array("sig_vuv_out",i,"4f")?,"vstr":r.scalar("sig_result",i,"f")?,"voiced":r.int("sig_voiced",i)?}),
        )
    })
}
fn pitch(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(330, |i| {
        let prev = r.array("pitch_state", i, "2f6i")?;
        let input = r.array("pitch_output", i, "3f")?;
        Ok(
            json!({"frame":i,"prev_lag":prev[0],"prev_pitch_corr":prev[1],"prev_lagblk":prev[2],"prev_lagidx":prev[3],"numstates":prev[5],"low_rate":prev[6],"low_complexity":prev[7],"ltp_buf":r.array("pitch_ltp",i,"659f")?,"F2":r.array("pitch_f2",i,"257f")?,"cav":r.int("pitch_cav",i)?,"pitchcorr":input[2],"avg_lag":input[1],"harm":input[0],"lags":r.array("pitch_lags",i,"8f")?,"laginds":r.array("pitch_indices",i,"8i")?,"blockseg_idx":r.int("pitch_block",i)?}),
        )
    })
}
fn lsf_count(run: &Path, count: usize) -> Result<Value> {
    let r = Reader(run);
    records(count, |i| {
        let conditional = r
            .scalar("lsf_cond_ptr", i, "I")?
            .as_u64()
            .context("condition pointer")?
            != 0;
        let cond = if conditional {
            Value::Array(r.array("lsf_cond", i, "544f")?)
        } else {
            Value::Null
        };
        Ok(
            json!({"A":r.array("lsf_a",i,"17f")?,"qlsf":r.array("lsf_q",i,"16f")?,"qi":r.array("lsf_qi",i,"17b")?,"bits":r.scalar("lsf_bits",i,"f")?,"RDbest":r.scalar("lsf_rd",i,"f")?,"weights":r.array("lsf_weights",i,"16f")?,"surv":r.int("lsf_surv",i)?,"RDw_adj":r.scalar("lsf_rdw",i,"f")?,"voiced":r.int("lsf_voiced",i)?,"lowRate":r.int("lsf_lowrate",i)?,"cond":cond}),
        )
    })
}
fn lsf(run: &Path) -> Result<Value> {
    lsf_count(run, 330)
}

fn hp(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(330, |i| {
        ensure!(r.int("hp_len", i)? == 320, "HP sample count");
        Ok(
            json!({"frame":i,"state_in":r.array("hp_state_in",i,"333f")?,"state_out":r.array("hp_state_out",i,"333f")?,"input":r.array("hp_pre",i,"320f")?,"output":r.array("hp_post",i,"320f")?,"lag":r.scalar("hp_lag",i,"f")?}),
        )
    })
}
fn harmonic(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(110, |i| {
        ensure!(r.int("harm_len", i)? == 960, "harmonic sample count");
        let count = r.int("harm_nframes", i)?;
        ensure!(count > 0, "harmonic frame count");
        let sum = r
            .scalar("harm_nbr_sum", i, "f")?
            .as_f64()
            .context("bitrate sum")?;
        Ok(
            json!({"packet":i,"state_in":r.array("harm_state_in",i,"2313f2i")?,"state_out":r.array("harm_state_out",i,"2313f2i")?,"input":r.array("harm_pre",i,"960f")?,"output":r.array("harm_post",i,"960f")?,"lags":r.array("harm_lags",i,"24f")?,"norm_br":sum/count as f64}),
        )
    })
}
fn params(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(330, |i| {
        let raw = r.raw("params", i)?;
        let count = section(&raw, 1400, 4, "i")?[0]
            .as_i64()
            .context("pulse count")?;
        ensure!((0..=160).contains(&count), "pulse count out of range");
        let positions = section(&raw, 760, 320, "160h")?;
        let magnitudes = section(&raw, 1080, 320, "160h")?;
        let mut pulses = vec![0i32; 320];
        for j in 0..count as usize {
            let pos = positions[j].as_i64().context("pulse position")?;
            ensure!((0..320).contains(&pos), "pulse position out of range");
            pulses[pos as usize] += magnitudes[j].as_i64().context("pulse magnitude")? as i32;
        }
        let mut value = json!({"voiced":section(&raw,0,4,"i")?[0],"fcbg":section(&raw,4,8,"4h")?,"acbg":section(&raw,12,8,"4h")?,"lsf":section(&raw,20,17,"17b")?,"interp":section(&raw,40,4,"i")?[0],"contour":section(&raw,44,4,"i")?[0],"lags":section(&raw,48,32,"8i")?,"energy_q14":section(&raw,104,16,"4i")?,"pulses":pulses,"sf_pulses":section(&raw,1408,8,"4h")?,"range":r.array("range_after",i,"11I")?});
        for name in ["len", "subframes", "cav", "lowrate", "frame", "sid"] {
            value[name] = json!(r.int(&format!("params_{name}"), i)?);
        }
        Ok(value)
    })
}
fn noise(run: &Path) -> Result<Value> {
    let r = Reader(run);
    records(1320, |i| {
        let sf = r.int("noise_sf", i)?;
        let frame = r.int("noise_frame", i)?;
        ensure!(
            sf == i as i64 % 4 && frame == i as i64 / 4 % 3 && r.int("noise_len", i)? == 80,
            "noise frame ordering/length"
        );
        let ngin = noise_state(&r.raw("noise_in", i)?)?;
        let ngout = noise_state(&r.raw("noise_out", i)?)?;
        let exc = r.array("noise_exc", i, "80f")?;
        let nz = exc
            .iter()
            .enumerate()
            .filter(|(_, v)| v.as_f64() != Some(0.0))
            .map(|(j, v)| json!([j, v]))
            .collect::<Vec<_>>();
        let lags = r.array("noise_lags", i, "24f")?;
        let start = (frame * 8 + sf * 2) as usize;
        let lsfs = r.array("noise_lsfs", i, "64f")?;
        Ok(
            json!({"packet":i/12,"frame":frame,"sf":sf,"voiced":r.int("noise_voiced",i)?,"sf_pulses":r.scalar("noise_pulses",i,"h")?,"fcbg_idx":r.scalar("noise_fcbg",i,"h")?,"nrgres":r.scalar("noise_nrg",i,"f")?,"norm_br":r.scalar("noise_nbr",i,"f")?,"seed_in":ngin["rand_seed"],"seed_out":ngout["rand_seed"],"ng_in":ngin,"ng_out":ngout,"lsf":lsfs[sf as usize*16..(sf as usize+1)*16],"lags":lags[start..start+2],"exc_pre":exc,"nz":nz,"noise":r.array("noise_audio",i,"80f")?}),
        )
    })
}
pub fn all(out: &Path) -> Result<()> {
    let dest = out.join("artifacts");
    std::fs::create_dir_all(&dest)?;
    let mut frames = Vec::new();
    let mut pcm = Vec::new();
    let mut vad = String::from("[\n");
    for i in 0..110 {
        let packet = std::fs::read(out.join(format!("mlow_110frames/packet{i:03}.bin")))?;
        ensure!(!packet.is_empty(), "empty packet");
        if i > 0 {
            vad.push_str(",\n");
        }
        vad.push_str(&format!(
            "  {{\n    \"frame\": {i},\n    \"toc\": {},\n    \"cav\": {},\n    \"len\": {}\n  }}",
            packet[0],
            u8::from(packet[0] != 0x10),
            packet.len()
        ));
        frames.push(hex::encode(packet));
        pcm.extend(std::fs::read(
            out.join(format!("mlow_110frames/decoded{i:03}.raw")),
        )?);
    }
    vad.push_str("\n]\n");
    write(&dest.join("wasm_derived_vad.json"), vad.as_bytes())?;
    write_json(&dest.join("wasm_derived_frames.json"), &frames)?;
    write(&dest.join("wasm_derived_ref.raw"), &pcm)?;
    frames.clear();
    pcm.clear();
    for i in 0..8 {
        frames.push(hex::encode(std::fs::read(
            out.join(format!("mlow_120ms/pkt120_{i}.bin")),
        )?));
        pcm.extend(std::fs::read(
            out.join(format!("mlow_120ms/dec120_{i}.raw")),
        )?);
    }
    write_json(&dest.join("wasm_derived_120ms_frames.json"), &frames)?;
    write(&dest.join("wasm_derived_120ms_ref.raw"), &pcm)?;
    type Build = fn(&Path) -> Result<Value>;
    for (leaf, run, build) in [
        ("fe", "fe", fe as Build),
        ("signal_mode", "signal", signal),
        ("pitch", "kernel", pitch),
        ("lsf_quant", "kernel", lsf),
        ("hp_postfilter", "postfilter", hp),
        ("harm_postfilter", "postfilter", harmonic),
        ("params", "params", params),
        ("gennoise", "gennoise", noise),
    ] {
        write_json(
            &dest.join(format!("wasm_{leaf}.json")),
            &build(&out.join(format!("mlow_{run}_trace")))?,
        )?;
    }
    Ok(())
}

pub fn one(
    kind: &str,
    run: &Path,
    out: &Path,
    secondary: Option<&Path>,
    count: usize,
) -> Result<()> {
    let value = match kind {
        "fe" => fe(run)?,
        "signal" => signal(run)?,
        "params" => params(run)?,
        "gennoise" => noise(run)?,
        "kernel" => {
            let secondary = secondary
                .map(Path::to_owned)
                .unwrap_or_else(|| out.with_file_name("wasm_lsf_quant.json"));
            write_json(&secondary, &lsf_count(run, count)?)?;
            pitch(run)?
        }
        "postfilter" => {
            let secondary = secondary
                .map(Path::to_owned)
                .unwrap_or_else(|| out.with_file_name("wasm_harm_postfilter.json"));
            write_json(&secondary, &harmonic(run)?)?;
            hp(run)?
        }
        _ => anyhow::bail!("unknown MLOW trace kind {kind}"),
    };
    write_json(out, &value)
}
