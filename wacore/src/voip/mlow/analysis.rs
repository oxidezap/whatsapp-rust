//! MLow ENCODER ANALYSIS: PCM -> `SmplFrameParams`. Per internal frame the LPC front-end windows the
//! 20 ms `lpcbuf`, FFT-autocorrelates it, derives the bandwidth-expanded `A` and its NLSF, and feeds
//! the bit-exact LSF quantizer (with the conditional-coding path); the resulting `grid`/`stage2` map
//! directly onto the wire and the decoder reconstructs the same envelope. The excitation comes from
//! the CELP encoder over the per-subframe interpolated LPC residual. The UNVOICED level is the
//! bit-exact `nrg_res` floor (the wire gain block IS the nrgres layout), with the per-subframe FCB
//! gain index as `nrg_res`. The VOICED (LTP, stage1=1) path runs the real CELP ACB/LTP encode: pitch
//! comes from a perceptually-weighted (`w_speech`) search and the `smpl_get_signal_mode` classifier;
//! the CELP's `acb_idx`/`fcb_idx`/pulses drive the wire pitch block (decoder-reconstructed lags feed
//! the ACB basis so encode/decode LTP agree). Closed-loop: decode(encode(analyze(pcm))) tracks the
//! input.
#![allow(clippy::needless_range_loop)]

use super::params::{
    SmplGainParams, SmplInternalParams, SmplLsfParams, SmplPitchParams, SmplPulseParams, SmplRawSym,
};
use super::smpl_celp::{CelpEncoder, smpl_distribute_fcb_surv};
use super::smpl_decode::{SmplLsfState, smpl_advance_lsf_state};
use super::smpl_harmcomb::{smpl_filt_arma2, smpl_get_hp_coefs};
use super::smpl_lpc::{
    SMPL_F_LEN, SMPL_LPC_BUF_LEN, smpl_a2nlsf_16, smpl_lpc_analyze_with_f2, smpl_window_lpc20,
};
use super::smpl_lsf_quant::{lsf_quant, lsf_quant_cond};
use super::smpl_mem::{SmplMem, load_smpl_mem};
use super::smpl_perc::{
    BitrateController, BitrateControllerInputs, PercModelState, SMPL_PERC_EMPH_UV,
    SMPL_PERC_EMPH_V, SMPL_PERC_REG, smpl_perc_ac2a, smpl_perc_model,
};
use super::smpl_signal_mode::{VuvMode, smpl_get_signal_mode};
use super::smpl_synth::{
    SMPL_INTF_LEN, SMPL_ORDER, SMPL_SUBFR_COUNT, SMPL_SUBFR_LEN, SMPL_VOICED_NORM_GAIN,
    SmplFrameSynth, SmplPitchSynth, SmplSynthTables, load_smpl_synth_tables, smpl_gain_lin,
    smpl_nlsf2a, smpl_reconstruct_nlsf, synth_internal_frame,
};

/// HP-history samples to carry for the LPC window buffer. The `lpcbuf` for internal frame 0 reaches
/// 96 samples before the current packet; carrying the full `lpc_buf_mem` (144) is safe and exact.
const SMPL_LPC_HIST_LEN: usize = 144;
/// `lpcbuf` starts 96 samples before each internal frame (`-WINNEXT_WB_LEN + framelen + WINNEXT_WB_LONG_LEN - lpcbuf_len`).
const SMPL_LPC_PRE: usize = 96;
/// `surv = lsf_surv` for complexity 8 (`update_complexity_setting`).
const SMPL_LSF_SURV: usize = 6;
/// 2 ms analysis lookahead (`SMPL_WINNEXT_WB_LEN`); zero at 16 kHz (no band split).
const SMPL_WINNEXT_WB_LEN: usize = 32;
/// `RDw_adj = sqrt(mainBitRate / 14000)` for the HIGH-rate (lowRate=0) path at 20 kbps.
const SMPL_LSF_RDW_ADJ: f32 = 1.1952286;

/// Cross-frame analysis state: only the LPC-analysis input history persists (the decoder rebuilds
/// synthesis state per 60 ms frame).
#[derive(Default)]
pub(crate) struct SmplEncoderState {
    hist: Vec<f64>,
    /// Reused because VAD runs for every packet on the realtime encode path.
    vad_pcm: Vec<i16>,
    /// Reused to avoid four packet-sized allocations on the realtime encode path.
    hp: Vec<f32>,
    x: Vec<f64>,
    xn: Vec<f32>,
    hp_full: Vec<f32>,
    /// Input high-pass (ARMA2, fcorner 35 Hz) coefficients + carried state, matching the real encoder.
    hp_coefs: Option<([f32; 3], [f32; 3])>,
    hp_state: [f32; 4],
    /// Persistent CELP excitation encoder (acb/zir/prev-idx state carries across subframes & frames).
    celp: Option<CelpEncoder>,
    /// Perceptual-weighting model state (FFT history) for the per-subframe `perc_wght_resp`.
    perc: Option<PercModelState>,
    /// Previous-pair perceptual autocorrelation, for the WB even-subframe interpolation.
    perc_prev: Vec<f32>,
    /// Bitrate controller (per-subframe pulse budget + importance), carried across frames.
    bitrate: Option<BitrateController>,
    /// HP-filtered input history (normalized, [-1,1]) for the LPC window buffer, mirroring the C
    /// `lpc_buf_mem`: the last `SMPL_LPC_HIST_LEN` HP samples of the previous packet.
    lpc_hist: Vec<f32>,
    /// Previous internal frame's committed (reconstructed) NLSF, for conditional LSF coding.
    prev_lsfq: Vec<f32>,
    /// Whether the previous internal frame was voiced (for the cond-coding condition).
    prev_voiced: bool,
    /// SILK VAD: per-internal-frame speech-activity probability + the coded_as_active_voice flag the
    /// bitrate controller and the voiced/unvoiced classifier read.
    vad: Option<super::smpl_vad::SmplVadState>,
    /// Voicing-classifier hysteresis + spectral-tilt background tracker (`VUV_Mode`), per stream.
    vuv: VuvMode,
    /// Last `SMPL_PITCH_LAG_MAX` HP samples of the previous packet, so the first internal frame's
    /// pitch search has real history instead of zeros.
    hp_pitch_hist: Vec<f32>,
    /// Persistent perceptually-weighted speech buffer (`ltp_buf`, length `MAX_LTP_BUF_LEN`), shifted
    /// left by one internal-frame each call. The full pitch estimator reads its tail.
    ltp_buf: Vec<f32>,
    /// Cross-frame pitch-estimator predictor (`PitchEstimator` non-scratch fields).
    pitch_est: super::smpl_pitch_enc::PitchEstState,
    /// Reusable scratch for the LPC-analysis FFT: the fixed 512-pt twiddle tables are built once per
    /// stream instead of every internal frame. Lazily initialized on first analyze.
    lpc_fft: Option<super::smpl_perc::FftScratch>,
}

/// Assumed encoder bitrate for the active MLow 1:1 config (the recorded capture's main rate is not
/// known a priori; this drives the per-subframe pulse budget via the bitrate controller).
const SMPL_MAIN_BIT_RATE: i32 = 20000;
const SMPL_COMPLEXITY: i32 = 8;

const SMPL_CELP_LOW_RATE: bool = false;
const SMPL_CELP_PERC_RESP_LEN: usize = 32;
const SMPL_CELP_FCB_SUBFRLEN: usize = 80;
/// 12 subframes per 60 ms packet (4 subframes/internal frame x 3 internal frames).
const SMPL_CELP_SUBFR_PER_PACKET: usize = 12;
/// `perc_resp_len + SMPL_PERC_EMPH_V_LEN - 1` (= 33 = SMPL_MAX_L_RESP): the perceptual autocorrelation
/// length the perc model returns and `smpl_perc_ac2a` consumes.
const SMPL_PERC_R_LEN: usize = SMPL_CELP_PERC_RESP_LEN + 1;
/// `smpl_fcb_tot_surv_20ms_max` for complexity 5-8 (the perc_resp_len=32 path). Drives `tot_surv`.
const SMPL_FCB_TOT_SURV_20MS_MAX: i32 = 100;

/// Encoder input high-pass 3 dB corner (`SMPL_ENC_HP_FCORNER_3DB_HZ`).
const SMPL_ENC_HP_FCORNER_HZ: f32 = 35.0;

fn unvoiced_pitch() -> SmplPitchSynth {
    SmplPitchSynth {
        voiced: false,
        lag_subfr: [0.0; 4],
        norm_gain: 0.0,
    }
}

struct Candidate {
    ip: SmplInternalParams,
    stage1: i32,
    grid: i32,
    qsym: [i32; 16],
    pulse_vec: Vec<i32>,
    /// Per-subframe excitation gainQ used by the synthesis (rate-control gain for unvoiced, 0 for
    /// voiced). Must match what `commit_candidate` feeds the shadow synth (warm history).
    gain_q: [i32; 4],
    /// LTP parameters for the synthesis (`voiced=false` for unvoiced).
    pitch: SmplPitchSynth,
    silent: bool,
}

/// Borrowed CELP/perceptual state for one internal frame's excitation analysis.
struct CelpFrameCtx<'a> {
    celp: &'a mut CelpEncoder,
    perc: &'a mut PercModelState,
    perc_prev: &'a mut Vec<f32>,
    bitrate: &'a mut BitrateController,
    /// Full normalized HP frame (960 samples, [-1,1]); the perc model windows slices of it.
    hp_n: &'a [f32],
    /// Internal-frame index (0..3) within the 60 ms packet.
    intf: usize,
    /// SILK VAD speech-activity probability for this internal frame (bitrate controller input).
    sp_act_prob: f32,
    /// Packet-level coded_as_active_voice (BACKGROUND_NOISE frame_type + voiced gating).
    coded_as_active_voice: bool,
    /// LPC power spectrum `F2[0..256]` for the voicing classifier's spectral tilt.
    f2: [f32; SMPL_F_LEN],
    /// This frame's classifier voicing_strength, fed to the bitrate controller's importance/pulse-
    /// budget computation.
    voicing_strength: f32,
    /// Voicing-classifier hysteresis state, threaded across the whole stream.
    vuv: &'a mut VuvMode,
    /// Previous packet's HP tail (`SMPL_PITCH_LAG_MAX` samples) for the intf=0 pitch history.
    hp_pitch_hist: &'a [f32],
    /// Persistent perceptually-weighted speech buffer (`ltp_buf`), carried across frames; the full
    /// pitch estimator reads its tail.
    ltp_buf: &'a mut Vec<f32>,
    /// Cross-frame pitch-estimator predictor.
    pitch_est: &'a mut super::smpl_pitch_enc::PitchEstState,
    /// Per-subframe perceptual autocorrelation (shared CELP + pitch input), computed once per frame.
    perc_corrs: Vec<Vec<f32>>,
    /// Decoder-reconstructed per-block pitch lags (2 per subframe) for the voiced CELP ACB. The CELP
    /// builds its ACB basis from these so the encoder/decoder LTP contributions agree on the wire.
    block_lags: [[f32; 2]; SMPL_SUBFR_COUNT],
}

/// Turn one 60 ms PCM frame (960 f32 @16 kHz, ~[-1,1]) into params, advancing `es`.
pub(crate) fn smpl_analyze_frame_st(
    es: &mut SmplEncoderState,
    pcm: &[f32],
) -> super::params::SmplFrameParams {
    let need = SMPL_INTF_LEN * 3;
    let mut owned;
    let pcm: &[f32] = if pcm.len() < need {
        owned = vec![0f32; need];
        owned[..pcm.len()].copy_from_slice(pcm);
        &owned
    } else {
        pcm
    };
    let synth_t = load_smpl_synth_tables();

    // SILK VAD on the int16 input PCM (runs on the raw API samples, before the encoder HP). Produces
    // the per-internal-frame speech-activity probability + the packet coded_as_active_voice.
    es.vad_pcm.clear();
    es.vad_pcm.extend(
        pcm[..need]
            .iter()
            .map(|&s| (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16),
    );
    let vad = es
        .vad
        .get_or_insert_with(super::smpl_vad::SmplVadState::new)
        .process_packet(&es.vad_pcm, SMPL_INTF_LEN);
    let sp_act_prob = vad.vad_results;
    let coded_as_active_voice = vad.coded_as_active_voice;

    // Encoder input high-pass (ARMA2, fcorner 35 Hz), matching the real encoder. Removes the
    // low-frequency content the decoder's de-emphasis would otherwise over-amplify; the residual the
    // analysis codes is then in the same band the real codec quantizes.
    let (hp_ma, hp_ar) = *es
        .hp_coefs
        .get_or_insert_with(|| smpl_get_hp_coefs(SMPL_ENC_HP_FCORNER_HZ));
    es.hp.resize(need, 0.0);
    es.hp.fill(0.0);
    smpl_filt_arma2(
        &pcm[..need],
        need,
        hp_ma,
        hp_ar,
        &mut es.hp_state,
        &mut es.hp,
    );

    // int16-scaled input with smplOrder lead samples of history.
    es.x.resize(SMPL_ORDER + need, 0.0);
    es.x.fill(0.0);
    if es.hist.len() >= SMPL_ORDER {
        es.x[..SMPL_ORDER].copy_from_slice(&es.hist[es.hist.len() - SMPL_ORDER..]);
    }
    for i in 0..need {
        es.x[SMPL_ORDER + i] = es.hp[i] as f64 * 32768.0;
    }

    let mut shadow = SmplFrameSynth::default();
    let mut prev_nlsf: Vec<f32> = Vec::new();
    // Predictor mirror, fresh per 60 ms frame (mirrors encode_smpl_frame's fresh SmplLsfState),
    // threaded across the 3 internal frames so the voiced abs-vs-delta lag choice matches the
    // entropy encoder.
    let mut lstate = SmplLsfState::default();

    // Lazily build the persistent CELP encoder + perceptual model (their state carries across frames).
    es.celp.get_or_insert_with(|| {
        CelpEncoder::new(
            SMPL_CELP_LOW_RATE,
            SMPL_CELP_PERC_RESP_LEN,
            SMPL_CELP_FCB_SUBFRLEN,
            SMPL_CELP_SUBFR_PER_PACKET,
        )
    });
    es.perc.get_or_insert_with(PercModelState::new);
    es.bitrate.get_or_insert_with(BitrateController::new);
    if es.perc_prev.len() != SMPL_PERC_R_LEN {
        es.perc_prev = vec![0.0; SMPL_PERC_R_LEN];
    }

    // Normalized HP input for the CELP residual (the real encoder works in [-1,1], not int16).
    // `xhp_frame` for internal frame 0 starts `SMPL_WINNEXT_WB_LEN` (32) samples BEFORE the packet's
    // first sample (xhp_frame = xhp_packet_buf + SMPL_LPC_BUF_MEM_LEN, while x_in16k =
    // xhp_packet_buf + SMPL_LPC_BUF_MEM_LEN + SMPL_WINNEXT_WB_LEN), so the excitation it codes leads
    // the input by 32 samples. Carry SMPL_ORDER + 32 lead so the residual can read that far back.
    let res_lead: usize = SMPL_ORDER + SMPL_WINNEXT_WB_LEN;
    es.xn.resize(res_lead + need, 0.0);
    es.xn.fill(0.0);
    if es.hist.len() >= res_lead {
        for i in 0..res_lead {
            es.xn[i] = (es.hist[es.hist.len() - res_lead + i] / 32768.0) as f32;
        }
    }
    es.xn[res_lead..res_lead + need].copy_from_slice(&es.hp[..need]);

    // Full HP-domain buffer that `lpcbuf` indexes: [history(144)] ++ [current 960 HP] ++ [32 zeros].
    // The 32-sample lookahead tail is zero at 16 kHz (no band split), per the buffer layout.
    es.hp_full
        .resize(SMPL_LPC_HIST_LEN + need + SMPL_WINNEXT_WB_LEN, 0.0);
    es.hp_full.fill(0.0);
    if es.lpc_hist.len() == SMPL_LPC_HIST_LEN {
        es.hp_full[..SMPL_LPC_HIST_LEN].copy_from_slice(&es.lpc_hist);
    }
    es.hp_full[SMPL_LPC_HIST_LEN..SMPL_LPC_HIST_LEN + need].copy_from_slice(&es.hp[..need]);

    let hp = es.hp.as_slice();
    let x = es.x.as_slice();
    let xn = es.xn.as_slice();
    let hp_full = es.hp_full.as_slice();

    // Snapshot the previous packet's HP tail (pitch history for this packet's intf=0), then refresh it
    // from this packet's tail for the next call.
    let mut hp_pitch_hist = std::mem::take(&mut es.hp_pitch_hist);
    if hp_pitch_hist.len() != SMPL_PITCH_LAG_MAX {
        hp_pitch_hist.resize(SMPL_PITCH_LAG_MAX, 0.0);
    }

    // Lazily size the persistent perceptually-weighted speech buffer (`ltp_buf`).
    if es.ltp_buf.len() != super::smpl_pitch_enc::MAX_LTP_BUF_LEN {
        es.ltp_buf = vec![0.0f32; super::smpl_pitch_enc::MAX_LTP_BUF_LEN];
    }

    let celp = es.celp.as_mut().expect("celp built above");
    let perc = es.perc.as_mut().expect("perc built above");
    let bitrate = es.bitrate.as_mut().expect("bitrate built above");
    let ltp_buf = &mut es.ltp_buf;
    let pitch_est = &mut es.pitch_est;

    let mut prev_lsfq = std::mem::take(&mut es.prev_lsfq);
    let mut prev_voiced = es.prev_voiced;

    let mut internal: [SmplInternalParams; 3] = Default::default();
    for f in 0..3 {
        let base = SMPL_ORDER + f * SMPL_INTF_LEN;
        let win = &x[base - SMPL_ORDER..base + SMPL_INTF_LEN];
        // `win_n` carries res_lead (SMPL_ORDER + res_pre) samples before the internal frame so the
        // residual can start res_pre samples early (matching the `xhp_frame` vs `x_in16k` offset).
        let nbase = res_lead + f * SMPL_INTF_LEN;
        let win_n = &xn[nbase - res_lead..nbase + SMPL_INTF_LEN];

        // Front-end LPC analysis: window `lpcbuf` (448 samples starting 96 before this frame),
        // FFT-autocorrelate it, and derive `A`/NLSF. `use_long_win` is true except the last frame.
        let lpc_start = SMPL_LPC_HIST_LEN - SMPL_LPC_PRE + f * SMPL_INTF_LEN;
        let mut lpcbuf = [0f32; SMPL_LPC_BUF_LEN];
        lpcbuf.copy_from_slice(&hp_full[lpc_start..lpc_start + SMPL_LPC_BUF_LEN]);
        let windowed = smpl_window_lpc20(&lpcbuf, f < 2);
        let lpc_fft = es
            .lpc_fft
            .get_or_insert_with(super::smpl_lpc::new_lpc_fft_scratch);
        let (a, f2) = smpl_lpc_analyze_with_f2(&windowed, lpc_fft);
        let nlsf = smpl_a2nlsf_16(&a);

        let mut cs = CelpFrameCtx {
            celp,
            perc,
            perc_prev: &mut es.perc_prev,
            bitrate,
            hp_n: hp,
            intf: f,
            sp_act_prob: sp_act_prob[f],
            coded_as_active_voice,
            f2,
            voicing_strength: 0.0,
            vuv: &mut es.vuv,
            hp_pitch_hist: &hp_pitch_hist,
            ltp_buf: &mut *ltp_buf,
            pitch_est: &mut *pitch_est,
            perc_corrs: Vec::new(),
            block_lags: [[0.0; 2]; SMPL_SUBFR_COUNT],
        };
        let fe = FrontEndLsf {
            a,
            nlsf,
            prev_lsfq: &prev_lsfq,
            prev_voiced,
            intf: f,
        };
        let (ip, nlsf_out, voiced_out) = smpl_analyze_internal(
            synth_t,
            &mut shadow,
            &mut lstate,
            f,
            win,
            win_n,
            &prev_nlsf,
            &fe,
            &mut cs,
        );
        prev_nlsf = nlsf_out.clone();
        prev_lsfq = nlsf_out;
        prev_voiced = voiced_out;
        internal[f] = ip;
        // The C resets the lag-block predictor after the last internal frame of each packet (and after
        // any unvoiced frame, handled in smpl_analyze_internal), so cond-coding restarts per packet.
        if f == 2 {
            pitch_est.reset_cond();
        }
    }

    // Carry SMPL_ORDER + SMPL_WINNEXT_WB_LEN history so the next packet's residual lead is filled.
    es.hist.clear();
    es.hist
        .extend_from_slice(&x[x.len() - (SMPL_ORDER + SMPL_WINNEXT_WB_LEN)..]);
    // Carry the last 144 HP samples as next packet's LPC window history (mirrors `lpc_buf_mem`).
    es.lpc_hist.clear();
    es.lpc_hist
        .extend_from_slice(&hp[need - SMPL_LPC_HIST_LEN..need]);
    hp_pitch_hist.copy_from_slice(&hp[need - SMPL_PITCH_LAG_MAX..need]);
    es.hp_pitch_hist = hp_pitch_hist;
    es.prev_lsfq = prev_lsfq;
    es.prev_voiced = prev_voiced;
    super::params::SmplFrameParams {
        toc: 0x50,
        config: 0,
        internal,
    }
}

/// Front-end LPC/NLSF analysis result for one internal frame, plus the conditional-coding context.
struct FrontEndLsf<'a> {
    /// Post-BWE monic LPC `A[0..16]` (A[0]=1).
    a: [f32; SMPL_LPC_ORDER + 1],
    /// Analysis NLSF (`smpl_A2NLSF_16(A)`), radians 0..pi.
    nlsf: [f32; SMPL_LPC_ORDER],
    /// Previous internal frame's committed NLSF (for conditional coding).
    prev_lsfq: &'a [f32],
    prev_voiced: bool,
    intf: usize,
}

const SMPL_LPC_ORDER: usize = 16;

impl FrontEndLsf<'_> {
    /// Run the bit-exact LSF quantizer for `voiced` and the cond-coding condition, returning the wire
    /// grid + stage2 + the committed (decoder-reconstructed) NLSF + the quantized predcoef.
    fn quantize(
        &self,
        synth_t: &SmplSynthTables,
        voiced: usize,
        prev_nlsf: &[f32],
    ) -> (i32, [i32; 16], Vec<f32>, [f32; 17]) {
        let cond = (self.prev_voiced == (voiced != 0)) && self.intf > 0;
        let res = if cond && self.prev_lsfq.len() == SMPL_LPC_ORDER {
            lsf_quant_cond(
                &self.a,
                &self.nlsf,
                self.prev_lsfq,
                voiced,
                0,
                SMPL_LSF_RDW_ADJ,
                SMPL_LSF_SURV,
            )
        } else {
            lsf_quant(
                &self.a,
                &self.nlsf,
                voiced,
                0,
                SMPL_LSF_RDW_ADJ,
                SMPL_LSF_SURV,
            )
        };
        let grid = res.qi[0];
        let mut stage2 = [0i32; 16];
        stage2.copy_from_slice(&res.qi[1..=SMPL_LPC_ORDER]);
        // Committed NLSF = the envelope the decoder rebuilds from the wire (proven == C qlsf).
        let committed =
            smpl_reconstruct_nlsf(synth_t, voiced, 0, grid as usize, &stage2, prev_nlsf);
        let a_vq = smpl_nlsf2a(&committed);
        let mut predcoef = [0.0f32; 17];
        for (i, &c) in a_vq.iter().enumerate().take(17) {
            predcoef[i] = c;
        }
        predcoef[0] = 1.0;
        (grid, stage2, committed, predcoef)
    }
}

fn commit_candidate(
    synth_t: &SmplSynthTables,
    st: &mut SmplFrameSynth,
    cand: &Candidate,
    prev_nlsf: &[f32],
) -> Vec<f32> {
    if cand.silent {
        let nlsf = smpl_reconstruct_nlsf(
            synth_t,
            0,
            0,
            cand.ip.lsf.grid as usize,
            &cand.ip.lsf.stage2,
            prev_nlsf,
        );
        let pulse_vec = vec![0i32; SMPL_INTF_LEN];
        synth_internal_frame(
            synth_t,
            st,
            0,
            0,
            cand.ip.lsf.grid as usize,
            &cand.ip.lsf.stage2,
            prev_nlsf,
            &pulse_vec,
            &cand.gain_q,
            &cand.pitch,
        );
        return nlsf;
    }
    let (_, nlsf) = synth_internal_frame(
        synth_t,
        st,
        cand.stage1 as usize,
        0,
        cand.grid as usize,
        &cand.qsym,
        prev_nlsf,
        &cand.pulse_vec,
        &cand.gain_q,
        &cand.pitch,
    );
    nlsf
}

fn smpl_unvoiced_candidate(
    synth_t: &SmplSynthTables,
    _st: &SmplFrameSynth,
    win: &[f64],
    win_n: &[f32],
    prev_nlsf: &[f32],
    fe: &FrontEndLsf,
    cs: &mut CelpFrameCtx,
) -> Candidate {
    let frame = &win[SMPL_ORDER..];

    let r0 = smpl_autocorr(frame, 0)[0];
    if r0 <= 0.0 {
        // Silent frame: still advance the CELP excitation state (zeros) so it stays in sync.
        let mut flat = [[0.0f32; 17]; SMPL_SUBFR_COUNT];
        for p in &mut flat {
            p[0] = 1.0;
        }
        // `run_celp_subframes` reads `perc_corrs` but never via `cs`, so lend it without a deep clone.
        let perc_corrs = std::mem::take(&mut cs.perc_corrs);
        run_celp_subframes(
            cs,
            &flat,
            &[0.0f32; SMPL_INTF_LEN],
            &[[0.0; 2]; SMPL_SUBFR_COUNT],
            &perc_corrs,
            SMPL_PERC_EMPH_UV,
            0,
        );
        cs.perc_corrs = perc_corrs;
        return smpl_silent_internal(synth_t);
    }

    // LSF: bit-exact C quantizer fed the faithful front-end NLSF. `grid`/`stage2` map directly onto the
    // wire (grid==16 = the cond centroid); `brec` is the decoder-reconstructed envelope (== C qlsf).
    let (bgrid, bsym, brec, _predcoef) = fe.quantize(synth_t, 0, prev_nlsf);

    // Per-subframe interpolated LPC (smpl_lpc_interpol): early subframes blend the previous frame's
    // committed NLSF with this frame's, smoothing the spectral transition the residual is whitened by.
    // The interpolation search tries idx 1 too and keeps it when it lowers the residual energy.
    let (predcoefs, res_lpc, interpol_idx) = smpl_lsf_interpol_search(&brec, fe.prev_lsfq, win_n);

    // Run the CELP excitation encoder per subframe (each with its interpolated predcoef). Lend
    // `perc_corrs` via mem::take (it is not reached through `cs`) instead of a deep clone.
    let perc_corrs = std::mem::take(&mut cs.perc_corrs);
    let celp_out = run_celp_subframes(
        cs,
        &predcoefs,
        &res_lpc,
        &[[0.0; 2]; SMPL_SUBFR_COUNT],
        &perc_corrs,
        SMPL_PERC_EMPH_UV,
        0,
    );
    cs.perc_corrs = perc_corrs;

    // Map CELP pulses -> per-position pulse train; collect the per-subframe FCB gain index (= the
    // wire `nrg_res` symbol, which the decoder reads back as `fcbg_idx`).
    let mut pulse_vec = vec![0i32; SMPL_INTF_LEN];
    let mut fcbg_idx = [0i32; 4];
    const MAIN: usize = 1;
    for sf in 0..SMPL_SUBFR_COUNT {
        let out = &celp_out[sf];
        for &v in &out.pulses[MAIN] {
            // Same unpacking as the C: sign = 1 + 2*(v>>15); pos = v*sign - 1; pPulses[pos] += sign.
            let sign = 1 + 2 * ((v as i32) >> 15);
            let pos = (v as i32 * sign) - 1;
            if (0..SMPL_SUBFR_LEN as i32).contains(&pos) {
                pulse_vec[sf * SMPL_SUBFR_LEN + pos as usize] += sign;
            }
        }
        fcbg_idx[sf] = out.gain_idx[MAIN] as i32;
    }

    // Unvoiced LEVEL (`nrgres`): bit-exact `smpl_quant_nrg_res` on the per-subframe residual energy.
    // The wire gain block IS the nrgres layout (gain_main=nrgres_frame_qi, gain_delta=nrgres_shape_qi,
    // gain_tab==nrgres_shape_CB, cb1==step) so the decoder reads `gain_q[sf]` back as `nrgres_dbq_Q14`.
    let mut nrgres = [0f32; 4];
    for (sf, n) in nrgres.iter_mut().enumerate() {
        let res = &res_lpc[sf * SMPL_SUBFR_LEN..(sf + 1) * SMPL_SUBFR_LEN];
        // `reslpc` (hence `nrgres`) is in the normalized [-1,1] domain (the encoder works in [-1,1]).
        let e: f32 = res.iter().map(|&v| v * v).sum();
        *n = e / SMPL_SUBFR_LEN as f32;
    }
    let nq = super::smpl_nrgres::quant_nrg_res_4(&nrgres);
    let gm = nq.frame_qi;
    let gd = nq.shape_qi;
    // Synthesis `gain_q[sf]` = the reconstructed per-subframe nrgres floor.
    let gain_q = nq.dbq_q14;

    let pp = smpl_build_pulse_params(&pulse_vec);
    let mut gains = SmplGainParams {
        gain_main: gm,
        gain_delta: gd,
        nrg_res: [-1; 4],
    };
    for sf in 0..4 {
        // The wire writes a per-subframe nrg_res (= fcbg_idx) only where pulses exist.
        gains.nrg_res[sf] = if pp.subfr[sf] > 0 { fcbg_idx[sf] } else { -1 };
    }

    Candidate {
        ip: SmplInternalParams {
            lsf: SmplLsfParams {
                stage1: 0,
                grid: bgrid,
                stage2: bsym,
                // lsf_interpol_idx: the decoder interpolates the per-subframe envelope with this, so it
                // must match the index the residual was whitened under.
                extra: interpol_idx,
            },
            pulses: pp,
            pitch: Default::default(),
            gains,
        },
        stage1: 0,
        grid: bgrid,
        qsym: bsym,
        pulse_vec,
        gain_q,
        pitch: unvoiced_pitch(),
        silent: false,
    }
}

/// Per-subframe perceptual weighting + CELP excitation for one internal frame (4 subframes of 80).
/// Returns the per-subframe CELP outputs; mutates the CELP/perc state so it stays in sync. `lags_subfr`
/// is the per-80-sample-subframe pitch lag in samples (0 = unvoiced); `emph` selects the perceptual
/// emphasis (UV vs V) and `voiced` drives the bitrate controller.
fn run_celp_subframes(
    cs: &mut CelpFrameCtx,
    predcoefs: &[[f32; 17]; SMPL_SUBFR_COUNT],
    res_lpc: &[f32],
    block_lags: &[[f32; 2]; SMPL_SUBFR_COUNT],
    perc_corrs: &[Vec<f32>],
    emph: [f32; 2],
    voiced: i32,
) -> Vec<super::smpl_celp::CelpSubframeOut> {
    let perc_wght = perc_corrs_to_wght(perc_corrs, emph, SMPL_CELP_PERC_RESP_LEN);
    let mut outs = Vec::with_capacity(SMPL_SUBFR_COUNT);

    // Per-subframe weighted target energy (the bitrate controller's `wnrg`). The C uses the
    // perceptually-weighted speech energy; the residual energy in the int16 domain is a faithful proxy
    // for the relative magnitudes the smoothing + importance ratios consume.
    let wnrgs: Vec<f32> = (0..SMPL_SUBFR_COUNT)
        .map(|sf| {
            let res = &res_lpc[sf * SMPL_SUBFR_LEN..(sf + 1) * SMPL_SUBFR_LEN];
            let scale = 32768.0f32;
            res.iter().map(|&v| (v * scale) * (v * scale)).sum::<f32>()
        })
        .collect();

    let enc = BitrateControllerInputs {
        internal_sample_rate: 16000,
        payload_size_ms: 60,
        fec_bit_rate: 0,
        main_bit_rate: SMPL_MAIN_BIT_RATE,
        complexity: SMPL_COMPLEXITY,
        use_fec_rate_compensation: 0,
        use_dtx: 0,
        sub_frame_importance_factor: 1.0,
    };

    for sf in 0..SMPL_SUBFR_COUNT {
        let wnrg = wnrgs[sf];
        let wnrg_next = if sf + 1 < SMPL_SUBFR_COUNT {
            wnrgs[sf + 1]
        } else {
            wnrgs[sf]
        };
        let nonflatness = if voiced != 0 { 0.0 } else { 2.0 };
        // Real classifier voicing_strength (`voicing_strength_buf`), negative for unvoiced.
        let voicing_strength = cs.voicing_strength;
        let (max_pulses, importance) = cs.bitrate.control(
            &enc,
            0,
            cs.coded_as_active_voice as i32,
            cs.sp_act_prob,
            nonflatness,
            voicing_strength,
            voiced,
            wnrg,
            wnrg_next,
            0,
            320,
            80,
        );
        let mut numsurv = [1i16; SMPL_MAX_PULSES_PER_SF as usize];
        let tot_surv =
            1000 * (SMPL_FCB_TOT_SURV_20MS_MAX * SMPL_CELP_FCB_SUBFRLEN as i32) / (20 * 16000);
        smpl_distribute_fcb_surv(&mut numsurv, max_pulses[1] as i32, tot_surv);

        // The two 40-sample sub-blocks of this subframe carry their own (decoder-reconstructed) lags;
        // index 2 is read by the encoder as the trailing lag (`lags[n_lags-1]`).
        let lags = [block_lags[sf][0], block_lags[sf][1], block_lags[sf][1]];

        let res = &res_lpc[sf * SMPL_SUBFR_LEN..(sf + 1) * SMPL_SUBFR_LEN];
        let out = cs.celp.encode_subframe(
            res,
            &predcoefs[sf],
            &perc_wght[sf],
            &lags,
            importance,
            max_pulses,
            &numsurv,
        );
        outs.push(out);
    }
    outs
}

const SMPL_MAX_PULSES_PER_SF: i32 = 40;

/// Per-subframe perceptual autocorrelation (`perc_corrs_buf`, length `SMPL_PERC_R_LEN`), the shared
/// input to BOTH the CELP weighting and the pitch-perceptual weighting. The WB path computes
/// the autocorrelation for odd subframes over a subframe-pair window and interpolates the even ones.
/// Advances the perc-model state, so it must run EXACTLY ONCE per internal frame.
fn compute_perc_corrs(cs: &mut CelpFrameCtx) -> [Vec<f32>; SMPL_SUBFR_COUNT] {
    let frame_ms = 20i32;
    let shorter = 32usize; // SMPL_WINNEXT_WB_LONG_LEN - SMPL_WINNEXT_WB_LEN
    let mut corrs: [Vec<f32>; SMPL_SUBFR_COUNT] = Default::default();
    let mut sf = 1;
    while sf < SMPL_SUBFR_COUNT {
        let start = cs.intf * SMPL_INTF_LEN + (sf - 1) * SMPL_SUBFR_LEN;
        let xlen = 2 * SMPL_SUBFR_LEN + shorter;
        let mut xsubfr = vec![0.0f32; xlen];
        for i in 0..xlen {
            let idx = start + i;
            xsubfr[i] = if idx < cs.hp_n.len() {
                cs.hp_n[idx]
            } else {
                0.0
            };
        }
        let is_last = (cs.intf == 2 && sf == SMPL_SUBFR_COUNT - 1) as i32;
        let r = smpl_perc_model(cs.perc, &xsubfr, xlen, frame_ms, is_last, SMPL_PERC_R_LEN);
        let mut even = vec![0.0f32; SMPL_PERC_R_LEN];
        for i in 0..SMPL_PERC_R_LEN {
            let prev = cs.perc_prev.get(i).copied().unwrap_or(0.0);
            even[i] = 0.5 * (r[i] + prev);
        }
        corrs[sf - 1] = even;
        // Refresh the persistent prev-pair buffer in place (reuse its allocation), then move `r`
        // into corrs, no fresh clone. `perc_prev` and `corrs[sf]` hold identical values.
        cs.perc_prev.clear();
        cs.perc_prev.extend_from_slice(&r);
        corrs[sf] = r;
        sf += 2;
    }
    corrs
}

/// Derive the per-subframe `perc_wght_resp` (length perc_resp_len) from precomputed `perc_corrs` for
/// the given emphasis (`smpl_perc_ac2a`, voiced vs unvoiced). Pure (no state).
fn perc_corrs_to_wght(corrs: &[Vec<f32>], emph: [f32; 2], resp_len: usize) -> Vec<Vec<f32>> {
    corrs
        .iter()
        .map(|c| {
            smpl_perc_ac2a(
                c,
                SMPL_PERC_R_LEN,
                emph[if SMPL_CELP_LOW_RATE { 1 } else { 0 }],
                resp_len,
                SMPL_PERC_REG,
            )
        })
        .collect()
}

/// The per-subframe residual + interpolated predcoef for `lsf_interpol_idx` 0, and the alternative
/// idx 1 when it lowers the summed per-subframe residual RMS by the 0.998 margin. Returns (predcoefs,
/// residual, chosen idx). At complexity 5-8 this search runs for every active frame.
fn smpl_lsf_interpol_search(
    brec: &[f32],
    prev_lsfq: &[f32],
    win_n: &[f32],
) -> ([[f32; 17]; SMPL_SUBFR_COUNT], Vec<f32>, i32) {
    let residual_for = |idx: usize| -> ([[f32; 17]; SMPL_SUBFR_COUNT], Vec<f32>, f32) {
        let (predcoefs, _ilsf) =
            super::smpl_lpc::smpl_lpc_interpol_idx(brec, prev_lsfq, idx, smpl_nlsf2a);
        let mut res = vec![0f32; SMPL_INTF_LEN];
        let mut sum_rms = 0.0f32;
        for sf in 0..SMPL_SUBFR_COUNT {
            let r = smpl_analysis_residual_subfr(&predcoefs[sf], win_n, sf);
            let nrg: f32 = r.iter().map(|&v| v * v).sum();
            sum_rms += (nrg + 1e-30).sqrt();
            res[sf * SMPL_SUBFR_LEN..(sf + 1) * SMPL_SUBFR_LEN].copy_from_slice(&r);
        }
        (predcoefs, res, sum_rms)
    };

    let (pc0, res0, rms0) = residual_for(0);
    // The alt interpolation runs whenever lsf_interpol_search && active && numsubfrs>1.
    let (pc1, res1, rms1) = residual_for(1);
    if rms1 < rms0 * 0.998 {
        (pc1, res1, 1)
    } else {
        (pc0, res0, 0)
    }
}

/// One-subframe residual under that subframe's interpolated predcoef (`smpl_filt_ma16_monic` over the
/// `sf`-th 80-sample block of `win_n`, which carries SMPL_ORDER lead history before the frame).
fn smpl_analysis_residual_subfr(
    a_syn: &[f32; 17],
    win_n: &[f32],
    sf: usize,
) -> [f32; SMPL_SUBFR_LEN] {
    let mut res = [0f32; SMPL_SUBFR_LEN];
    for (n, rn) in res.iter_mut().enumerate() {
        let idx = SMPL_ORDER + sf * SMPL_SUBFR_LEN + n;
        let mut acc = win_n[idx];
        for j in 1..=SMPL_ORDER {
            acc += a_syn[j] * win_n[idx - j];
        }
        *rn = acc;
    }
    res
}

fn smpl_silent_internal(synth_t: &SmplSynthTables) -> Candidate {
    let mut sym = [0i32; 16];
    for (k, s) in sym.iter_mut().enumerate() {
        *s = (synth_t.valtables[0][0][0][k].len() / 2) as i32;
    }
    // Silent frame: lowest encodable gain (no pulses, so the exact value is immaterial).
    let (gm, gd, _) = smpl_rate_control_gains(0.0);
    Candidate {
        ip: SmplInternalParams {
            lsf: SmplLsfParams {
                stage1: 0,
                grid: 0,
                stage2: sym,
                extra: 0,
            },
            pulses: SmplPulseParams::default(),
            pitch: Default::default(),
            gains: SmplGainParams {
                gain_main: gm,
                gain_delta: gd,
                nrg_res: [-1; 4],
            },
        },
        stage1: 0,
        grid: 0,
        qsym: sym,
        pulse_vec: vec![0i32; SMPL_INTF_LEN],
        gain_q: [0; 4],
        pitch: unvoiced_pitch(),
        silent: true,
    }
}

fn smpl_autocorr(x: &[f64], order: usize) -> Vec<f64> {
    let n = x.len();
    let mut r = vec![0f64; order + 1];
    for (lag, rl) in r.iter_mut().enumerate() {
        let mut s = 0f64;
        for i in lag..n {
            s += x[i] * x[i - lag];
        }
        *rl = s;
    }
    r
}

fn smpl_build_pulse_params(pulse: &[i32]) -> SmplPulseParams {
    const P3: usize = 4;
    let pos_per = SMPL_INTF_LEN / P3; // 80
    let mut pp = SmplPulseParams::default();
    for sf in 0..P3 {
        let mut s = 0i32;
        for n in sf * pos_per..(sf + 1) * pos_per {
            s += pulse[n].abs();
        }
        pp.subfr[sf] = s;
    }
    pp.total = pp.subfr.iter().sum();

    let mut mag_runs: Vec<i32> = Vec::new();
    let mut signs: Vec<i32> = Vec::new();
    for sf in 0..P3 {
        if pp.subfr[sf] <= 0 {
            continue;
        }
        let base_pos = pos_per * sf;
        let mut run_pos = base_pos as i32;
        let mut first = true;
        // Collected and consumed in the same ascending order, so the two passes fuse; the `Vec` this
        // replaces grew by push and was rebuilt for every subframe of every frame.
        for (p, magv) in (base_pos..base_pos + pos_per)
            .map(|n| (n, pulse[n]))
            .filter(|&(_, v)| v != 0)
        {
            let mag = magv.abs();
            let m = if first {
                p as i32 - base_pos as i32
            } else {
                p as i32 - run_pos
            };
            mag_runs.push(m);
            run_pos = p as i32;
            if mag > 1 {
                mag_runs.resize(mag_runs.len() + (mag - 1) as usize, 0);
            }
            signs.push(if magv < 0 { -1 } else { 1 });
            first = false;
        }
    }
    pp.mag_runs = mag_runs;

    // SIGN block: batch signs into raw symbols (<=15 bits each, MSB-first).
    let num_pos = signs.len();
    let mut sign_syms: Vec<SmplRawSym> = Vec::new();
    let mut p = 0;
    while p < num_pos {
        let nbits = (num_pos - p).min(15);
        let mut sym = 0u32;
        for q in 0..nbits {
            let bit = if signs[p + q] > 0 { 1u32 } else { 0 };
            sym |= bit << (nbits - 1 - q) as u32;
        }
        sign_syms.push(SmplRawSym {
            sym,
            nbits: nbits as u32,
        });
        p += nbits;
    }
    pp.sign_syms = sign_syms;
    pp
}

/// Find the (gainMain, gainDelta, reconstructed gainQ) whose linear gain is closest to `target_linear`.
fn smpl_rate_control_gains(target_linear: f64) -> (i32, i32, i32) {
    let cc = super::smpl_cc_tables::load_cc_tables();
    let cfg_sel = 2i32;
    let cb1 = cc.nrg_step(cfg_sel);
    let mut best_d = f64::INFINITY;
    let (mut bgm, mut bgd, mut bgq) = (0i32, 0i32, 0i32);
    for gm in 0..84 {
        let base7 = gm * cb1 - 0x154000;
        for gd in 0..98 {
            let cbv = cc.gain_recon(true, 4 * gd);
            let gq = base7 + (cbv << 4);
            let d = (smpl_gain_lin(gq) - target_linear).abs();
            if d < best_d {
                best_d = d;
                bgm = gm;
                bgd = gd;
                bgq = gq;
            }
        }
    }
    (bgm, bgd, bgq)
}

// voiced (LTP) encode path

/// The perceptual emphasis the pitch weighting uses.
const SMPL_PERC_EMPH_PITCH: f32 = -0.82;
/// `pitch_perc_resp_len` for complexity 5-8 (the 17-tap monic MA weighting).
const SMPL_PITCH_PERC_RESP_LEN: usize = 17;
/// Pitch search history span in samples (`SMPL_MAXPITCH_LEN`), carried for the intf=0 estimator.
const SMPL_PITCH_LAG_MAX: usize = 320;
/// Pitch estimator lookahead (`SMPL_PITCH_LOOKAHEAD_LEN`).
const SMPL_PITCH_LOOKAHEAD_LEN: usize = 7;

/// Roll the persistent perceptually-weighted speech buffer (`ltp_buf`) and write this internal frame's
/// weighted speech into its tail: shift left by `framelen`, then per CELP subframe `i` apply the
/// 17-tap monic perceptual MA of the HP frame under `resp_pitch[i]`, plus the
/// `PITCH_LOOKAHEAD_LEN`-sample lookahead under `resp_pitch[3]`.
/// The HP frame (`xhp_frame`) starts `SMPL_WINNEXT_WB_LEN` samples before the internal frame; the MA
/// reads up to `SMPL_LPC_ORDER` samples of history before that. Built in the normalized HP domain,
/// which is scale-invariant for the estimator's pitchcorr/lag outputs.
fn build_ltp_buf(cs: &mut CelpFrameCtx, perc_corrs: &[Vec<f32>]) {
    let resp_pitch = perc_corrs_to_wght(
        perc_corrs,
        [SMPL_PERC_EMPH_PITCH, SMPL_PERC_EMPH_PITCH],
        SMPL_PITCH_PERC_RESP_LEN,
    );
    let max_len = super::smpl_pitch_enc::MAX_LTP_BUF_LEN; // 659
    let look = SMPL_PITCH_LOOKAHEAD_LEN; // 7
    let framelen = SMPL_INTF_LEN; // 320
    // Shift existing weighted speech left by one internal frame.
    let keep = max_len - framelen - look;
    cs.ltp_buf.copy_within(framelen..framelen + keep, 0);
    // HP sample at internal-frame-relative index `idx` (xhp_frame origin), reaching into the previous
    // packet's tail (`hp_pitch_hist`, entry `k` at relative index `k - SMPL_PITCH_LAG_MAX`) for idx<0.
    let frame_start = cs.intf as isize * SMPL_INTF_LEN as isize - SMPL_WINNEXT_WB_LEN as isize;
    let hist = SMPL_PITCH_LAG_MAX as isize;
    let sample = |rel: isize| -> f32 {
        let idx = frame_start + rel;
        if idx >= 0 {
            let u = idx as usize;
            if u < cs.hp_n.len() { cs.hp_n[u] } else { 0.0 }
        } else if cs.hp_pitch_hist.len() == hist as usize {
            let k = idx + hist;
            if k >= 0 {
                cs.hp_pitch_hist[k as usize]
            } else {
                0.0
            }
        } else {
            0.0
        }
    };
    // w_speech write origin in ltp_buf (MAX_LTP_BUF_LEN - numsubfrs*subfrlen - lookahead).
    let w_origin = max_len - SMPL_SUBFR_COUNT * SMPL_SUBFR_LEN - look; // 332
    for i in 0..SMPL_SUBFR_COUNT {
        let coef = &resp_pitch[i];
        for n in 0..SMPL_SUBFR_LEN {
            let pos = (i * SMPL_SUBFR_LEN + n) as isize;
            let mut res = sample(pos); // monic coef[0]==1
            for (j, &c) in coef
                .iter()
                .enumerate()
                .take(SMPL_PITCH_PERC_RESP_LEN)
                .skip(1)
            {
                res += c * sample(pos - j as isize);
            }
            cs.ltp_buf[w_origin + i * SMPL_SUBFR_LEN + n] = res;
        }
    }
    // Lookahead tail under the last subframe's response.
    let coef = &resp_pitch[SMPL_SUBFR_COUNT - 1];
    for n in 0..look {
        let pos = (framelen + n) as isize;
        let mut res = sample(pos);
        for (j, &c) in coef
            .iter()
            .enumerate()
            .take(SMPL_PITCH_PERC_RESP_LEN)
            .skip(1)
        {
            res += c * sample(pos - j as isize);
        }
        cs.ltp_buf[max_len - look + n] = res;
    }
}

/// Analyze one internal frame: compute the shared perceptual autocorrelation, build the perceptually-
/// weighted `ltp_buf`, run the faithful multi-stage pitch estimator + the `smpl_get_signal_mode`
/// voicing classifier, then build the voiced (LTP) or unvoiced candidate, commit it to the shadow synth
/// `st`, and advance the entropy predictor mirror.
#[allow(clippy::too_many_arguments)]
fn smpl_analyze_internal(
    synth_t: &SmplSynthTables,
    st: &mut SmplFrameSynth,
    lstate: &mut SmplLsfState,
    intf: usize,
    win: &[f64],
    win_n: &[f32],
    prev_nlsf: &[f32],
    fe: &FrontEndLsf,
    cs: &mut CelpFrameCtx,
) -> (SmplInternalParams, Vec<f32>, bool) {
    let mem = load_smpl_mem();

    // Shared perceptual autocorrelation (advances perc state EXACTLY ONCE per frame); both the pitch
    // weighting and the CELP weighting derive from it (matching `perc_corrs_buf`). Move the
    // per-subframe Vecs out of the array instead of cloning each.
    cs.perc_corrs = compute_perc_corrs(cs).into();

    // Roll the persistent perceptually-weighted speech buffer (`ltp_buf`) and write this frame's
    // weighted speech + lookahead into its tail, then run the faithful multi-stage pitch estimator.
    // `build_ltp_buf` reads `perc_corrs` but never touches it through `cs`, so lend it via mem::take
    // (no deep clone of the per-subframe Vecs) and restore it after.
    let perc_corrs = std::mem::take(&mut cs.perc_corrs);
    build_ltp_buf(cs, &perc_corrs);
    cs.perc_corrs = perc_corrs;
    let f2 = cs.f2;
    // `pitch_est` and `ltp_buf` are disjoint `cs` fields, so borrow them directly (no ltp_buf clone).
    let pr =
        super::smpl_pitch_enc::smpl_pitch(cs.pitch_est, cs.ltp_buf, &f2, cs.coded_as_active_voice);
    let pitchcorr = pr.pitchcorr;
    let avg_lag = pr.avg_lag;
    let harm = pr.harm_strength;
    let mut lags8 = pr.lags;
    // The single representative lag the voiced encode path uses; the C's wire contour is anchored on
    // the first-subframe lag, so use that as the encode target (the per-block CELP basis is rebuilt
    // from the wire pitch params downstream).
    let lag_samples = pr.lags[0];
    let sp = cs.sp_act_prob;
    let vstr = smpl_get_signal_mode(pitchcorr, &lags8, avg_lag, harm, &f2, sp, cs.vuv);
    cs.voicing_strength = vstr;
    let is_voiced_decision = vstr > 0.0 && cs.coded_as_active_voice;
    lstate.prev_lag_samples = if is_voiced_decision { lag_samples } else { 0.0 };
    // The C resets the lag-block predictor after an unvoiced frame (and after each packet's last frame,
    // handled at the call site); mirror the unvoiced reset here so cond-coding restarts correctly.
    if !is_voiced_decision {
        cs.pitch_est.reset_cond();
        lags8 = [0.0; 8];
    }

    // The CELP excitation encoder advances its per-subframe acb/zir/prev-idx state, so it must run
    // EXACTLY ONCE per internal frame with the lags of the committed decision.
    let mut voiced_lstate = lstate.clone();
    smpl_advance_lsf_state(&mut voiced_lstate, intf, 1);
    let voiced = if is_voiced_decision {
        smpl_voiced_decision_for_lag(pr.blockseg_idx, &pr.laginds, cs, &mut lags8)
    } else {
        None
    };

    let (chosen, chosen_lstate, is_voiced) = match voiced {
        Some(vd) => {
            let cand = smpl_voiced_candidate(synth_t, win_n, prev_nlsf, fe, cs, &vd);
            (cand, Some(voiced_lstate), true)
        }
        None => (
            smpl_unvoiced_candidate(synth_t, st, win, win_n, prev_nlsf, fe, cs),
            None,
            false,
        ),
    };
    let committed_nlsf = commit_candidate(synth_t, st, &chosen, prev_nlsf);
    if chosen.stage1 == 1 {
        *lstate = chosen_lstate.expect("voiced candidate set its lstate");
        let subfr = chosen.ip.pulses.subfr;
        smpl_replay_pitch_state(mem, lstate, 4, subfr, &chosen.ip.pitch);
    } else {
        smpl_advance_lsf_state(lstate, intf, chosen.stage1);
    }
    (chosen.ip, committed_nlsf, is_voiced)
}

/// Advance the predictor mirror exactly as `encode_smpl_pitch` does, without entropy coding, so the
/// analysis predicts the lag/gain predictor for the next internal frame. Threads the lag predictor
/// (`prev_lagblk`/`prev_lagidx`) from the chosen contour + per-block laginds.
fn smpl_replay_pitch_state(
    _mem: &SmplMem,
    st: &mut SmplLsfState,
    p3: i32,
    subfr_counts: [i32; 4],
    pp: &SmplPitchParams,
) {
    for sf in 0..(p3 as usize).min(4) {
        st.prev_gain_idx = pp.gain_idx[sf];
        if subfr_counts[sf] > 0 {
            st.prev_filt_idx = pp.filt_idx[sf];
        }
    }
    let tab = super::smpl_pitch_enc::load_pitch_tables();
    let (nblk, nidx) =
        super::smpl_pitch_enc::smpl_lags_predictor_after(tab, pp.blockseg_idx, &pp.laginds);
    st.prev_lagblk = nblk;
    st.prev_lagidx = nidx;
}

/// The committed voiced decision for one internal frame: the encodable pitch params and the
/// per-subframe synthesis lag carried in `pitch`. The LSF comes from the shared front-end.
struct VoicedDecision {
    pp: SmplPitchParams,
    pitch: SmplPitchSynth,
}

/// Carry the estimator's full per-block contour (`blockseg_idx` + `laginds`) into the voiced decision:
/// the wire pitch encode writes them straight through `smpl_encode_lags`, and the CELP ACB basis uses
/// the SAME per-block lags (`lag = laginds*0.5 + 32`) so the encoder/decoder LTP contributions agree.
/// The gain/filter indices here are placeholders; the voiced candidate overwrites them with the real
/// CELP `acb_idx`/`fcb_idx` per subframe.
fn smpl_voiced_decision_for_lag(
    blockseg_idx: usize,
    laginds: &[i32; 8],
    cs: &mut CelpFrameCtx,
    lags8: &mut [f32; 8],
) -> Option<VoicedDecision> {
    // The decoder maps each 40-block lag index `lag = laginds*0.5 + SMPL_MIN_PITCH_LAG`, clamped ≤320.
    let mut block_lags8 = [0.0f32; 8];
    for b in 0..8 {
        block_lags8[b] = (laginds[b] as f32 * 0.5 + 32.0).min(320.0);
    }
    *lags8 = block_lags8;
    for sf in 0..SMPL_SUBFR_COUNT {
        cs.block_lags[sf] = [block_lags8[2 * sf], block_lags8[2 * sf + 1]];
    }
    let mean_lag = block_lags8.iter().sum::<f32>() / 8.0;

    let pp = SmplPitchParams {
        gain_idx: [5i32; 4],
        filt_idx: [0i32; 4],
        blockseg_idx,
        laginds: *laginds,
    };

    let pitch = SmplPitchSynth {
        voiced: true,
        lag_subfr: [mean_lag as f64; 4],
        norm_gain: SMPL_VOICED_NORM_GAIN,
    };
    Some(VoicedDecision { pp, pitch })
}

/// Build the voiced (stage1=1 + LTP) candidate for one internal frame. The real CELP voiced encoder
/// runs with the decoder-reconstructed per-block lags (so its ACB basis matches the decoder's), and
/// its outputs drive the wire: `pulses[MAIN]` → the pulse train, `acb_idx[MAIN]` → the wire `gain_idx`
/// (ACB/LTP gain), `gain_idx[MAIN]` → the wire `filt_idx` (voiced FCB gain). The decoder then adds the
/// ACB contribution and scales the FCB pulses by the voiced gain table, reproducing the encoder's
/// excitation instead of the prior gainless greedy approximation.
fn smpl_voiced_candidate(
    synth_t: &SmplSynthTables,
    // Use the caller's window WITH the 32-sample CELP pre-lead (same as the unvoiced path), matching
    // the C: both voiced and unvoiced take the LPC residual from the same pre-lead window.
    win_n: &[f32],
    prev_nlsf: &[f32],
    fe: &FrontEndLsf,
    cs: &mut CelpFrameCtx,
    vd: &VoicedDecision,
) -> Candidate {
    let gain_q = [0i32; 4]; // voiced synthesis uses the ACB+FCB excitation, not a gains block

    // Voiced-grid LSF: bit-exact C quantizer fed the faithful front-end NLSF (voiced codebook).
    let (bgrid, bsym, brec, _predcoef) = fe.quantize(synth_t, 1, prev_nlsf);

    // Per-subframe interpolated LPC (same as the unvoiced path).
    let (predcoefs, _ilsf) = super::smpl_lpc::smpl_lpc_interpol(&brec, fe.prev_lsfq, smpl_nlsf2a);
    let mut res_lpc = vec![0f32; SMPL_INTF_LEN];
    for sf in 0..SMPL_SUBFR_COUNT {
        let r = smpl_analysis_residual_subfr(&predcoefs[sf], win_n, sf);
        res_lpc[sf * SMPL_SUBFR_LEN..(sf + 1) * SMPL_SUBFR_LEN].copy_from_slice(&r);
    }

    // Real voiced CELP: with nonzero lags the encoder runs the ACB/LTP path (calc_acb_gain → d_ltp →
    // FCB deldec on the post-LTP residual → calc_gains_v), producing the pulse set + acb/fcb indices.
    let block_lags = cs.block_lags;
    // Lend `perc_corrs` via mem::take (not reached through `cs`) instead of a deep clone.
    let perc_corrs = std::mem::take(&mut cs.perc_corrs);
    let celp_out = run_celp_subframes(
        cs,
        &predcoefs,
        &res_lpc,
        &block_lags,
        &perc_corrs,
        SMPL_PERC_EMPH_V,
        1,
    );
    cs.perc_corrs = perc_corrs;

    // Unpack the MAIN-rate pulses into a per-position train; collect acb/fcb indices per subframe.
    const MAIN: usize = 1;
    let mut pulse_vec = vec![0i32; SMPL_INTF_LEN];
    let mut acbg = [0i32; 4];
    let mut fcbg = [0i32; 4];
    for sf in 0..SMPL_SUBFR_COUNT {
        let out = &celp_out[sf];
        for &v in &out.pulses[MAIN] {
            let sign = 1 + 2 * ((v as i32) >> 15);
            let pos = (v as i32 * sign) - 1;
            if (0..SMPL_SUBFR_LEN as i32).contains(&pos) {
                pulse_vec[sf * SMPL_SUBFR_LEN + pos as usize] += sign;
            }
        }
        // acb_idx is always coded; fcb (filt) only where pulses exist. Clamp to the wire ranges.
        acbg[sf] = (out.acb_idx[MAIN] as i32).clamp(0, 15);
        fcbg[sf] = (out.gain_idx[MAIN] as i32).max(0);
    }
    let pp_pulses = smpl_build_pulse_params(&pulse_vec);
    let subfr = pp_pulses.subfr;
    let mut pp = vd.pp.clone();
    pp.gain_idx = acbg;
    for sf in 0..4 {
        pp.filt_idx[sf] = if subfr[sf] > 0 { fcbg[sf] } else { -1 };
    }

    Candidate {
        ip: SmplInternalParams {
            lsf: SmplLsfParams {
                stage1: 1,
                grid: bgrid,
                stage2: bsym,
                extra: 0,
            },
            pulses: pp_pulses,
            pitch: pp,
            gains: SmplGainParams::default(),
        },
        stage1: 1,
        grid: bgrid,
        qsym: bsym,
        pulse_vec,
        gain_q,
        pitch: vd.pitch.clone(),
        silent: false,
    }
}

/// Per-stage bench surface for `wacore/benches/voip_benchmark.rs`.
///
/// `mlow_encode` is one row, so a change inside the codec moves it without saying WHICH stage moved.
/// These harnesses call the SAME production functions the analyzer calls, one stage per row, so a
/// stage-local change has a stage-local number.
///
/// Setup vs. body is the load-bearing distinction: `Stages::new` runs real frames through
/// `smpl_analyze_frame_st`, which builds every `OnceLock` table, both FFT twiddle sets and every
/// pooled buffer, and leaves the CELP/perc/pitch/VAD state in its steady-state regime. What the
/// `run_*` bodies then execute is per-frame work only -- what production pays every 60 ms, never what
/// it resolves once and holds. The per-frame multiplicity of each stage is on its method.
///
/// KNOWN LIMITATION, measured rather than assumed. The stateful rows advance the same cross-frame
/// state production advances (perc history, pitch predictor, CELP ACB/ZIR, bitrate controller) while
/// replaying a fixed three-frame input cycle, so a row's regime is its own history rather than a live
/// call's. `stage_rows_are_three_periodic` pins the result: `pitch_search` and `perc_model_frame` are
/// EXACTLY 3-periodic from the first iteration, so for them the question does not arise.
/// `celp_subframes_frame` is not -- its pulse counts wander (28,19,31,... early vs 26,16,33,... after
/// 20 iterations) because the excitation history keeps evolving against a repeating residual. Its
/// timing spread stays inside 1%, so the cost is dominated by the fixed-size beam search rather than
/// the exact pulse count, but that row is a steady-state estimate and not a reproduction of one
/// production frame. `analyze_frame` has no such caveat -- it drives the real encoder over eight
/// cycling frames -- which is why the stage rows are reported against it rather than summed alone.
///
/// Not a consumer API: it exists so the benchmark can attribute codec CPU. Unlike the SRTP
/// primitives -- which are production code that merely stays `#[doc(hidden)]` -- this module is pure
/// bench scaffolding, so it is behind the `bench-internals` feature and compiles into nothing for a
/// normal consumer. `#[doc(hidden)]` alone would only hide it from rustdoc, not from the build.
#[cfg(feature = "bench-internals")]
#[doc(hidden)]
pub mod stage_bench {
    use super::*;
    use crate::voip::mlow::smpl_lpc::{SMPL_LPC_NFFT, new_lpc_fft_scratch};
    use crate::voip::mlow::smpl_perc::{
        FftScratch, PERCW_NFFT, rfft_backward_ordered_sc, rfft_forward_ordered_sc,
    };

    /// Frames pushed through the encoder before any input is captured. Three leave every cross-frame
    /// buffer (`hist`, `lpc_hist`, `hp_pitch_hist`, `ltp_buf`, ACB state) filled with real signal
    /// rather than the zeros a first frame sees.
    const WARMUP_FRAMES: usize = 3;

    /// Distinct frames the `analyze_frame` row cycles, matching `mlow_encode`'s own `STREAM`. A
    /// single repeated frame lets the adaptive encoder settle onto a cheaper low-pulse/low-survivor
    /// path, which would under-report the stage and make the row incomparable to `mlow_encode`.
    const STREAM: usize = 8;

    /// A 60 ms voiced tone, matching the benchmark's own steady-state input.
    fn tone(phase: usize) -> Vec<f32> {
        (0..SMPL_INTF_LEN * 3)
            .map(|i| 0.3 * (((i + phase) as f32) * 0.07).sin())
            .collect()
    }

    /// One internal frame's LSF-quantizer inputs. Production derives `a`/NLSF anew per internal
    /// frame and threads the previous frame's committed envelope; `lsf_quant_core` has an
    /// input-dependent early exit (`abs_qerr < 0.25`), so frozen values would fix how many
    /// refinement iterations run.
    struct LsfInputs {
        a: [f32; SMPL_LPC_ORDER + 1],
        nlsf: [f32; SMPL_LPC_ORDER],
        prev_nlsf: Vec<f32>,
    }

    /// One internal frame's pitch-estimator inputs, captured from the live encoder state after
    /// `build_ltp_buf` has rolled that frame's perceptually weighted speech in -- which production
    /// does before every `smpl_pitch` call. A frozen pair would fix the estimator's data-dependent
    /// survivor blocks, so resetting the predictor alone would still measure one workload.
    struct PitchInputs {
        ltp_buf: Vec<f32>,
        f2: [f32; SMPL_F_LEN],
    }

    /// One internal frame's CELP inputs, captured from the live encoder state.
    #[derive(Default)]
    struct CelpInputs {
        predcoefs: [[f32; SMPL_LPC_ORDER + 1]; SMPL_SUBFR_COUNT],
        res_lpc: Vec<f32>,
        block_lags: [[f32; 2]; SMPL_SUBFR_COUNT],
        perc_corrs: Vec<Vec<f32>>,
    }

    /// A primed encoder plus the captured stage inputs.
    pub struct Stages {
        es: SmplEncoderState,
        /// Distinct frames the `analyze_frame` row cycles through, and the cursor into them.
        pcm: Vec<Vec<f32>>,
        pcm_at: usize,
        /// Snapshot of the last analyzed frame's normalized HP signal (`hp_n`), which the perc model
        /// and the residual read.
        hp: Vec<f32>,
        hp_pitch_hist: Vec<f32>,
        /// LPC front-end inputs, captured for internal frame 0.
        /// The `lpcbuf` window each internal frame analyzes. Production advances the start by
        /// `f * SMPL_INTF_LEN`, and the coefficients that fall out drive the data-dependent root
        /// search in `smpl_a2nlsf_16`, so cycling only the long/short window flag would still be
        /// three passes over one signal.
        lpcbufs: Vec<[f32; SMPL_LPC_BUF_LEN]>,
        /// Zero-padded windowed buffer + output, for the bare FFT row.
        fft_in: Vec<f32>,
        fft_out: Vec<f32>,
        fft_scratch: FftScratch,
        /// Same, at the perceptual model's size (N=576 = 2^6 * 3^2), for the bare mixed-radix row.
        fft576_time: Vec<f32>,
        fft576_spec: Vec<f32>,
        /// Separate destination for the inverse: writing back into `fft576_time` would scale the
        /// signal by N on every iteration (the transform pair is unnormalized) and walk the row into
        /// inf, changing what it measures.
        fft576_out: Vec<f32>,
        fft576_scratch: FftScratch,
        prev_nlsf: Vec<f32>,
        /// Per-row cursors over the internal-frame index. Every stage that production runs once per
        /// internal frame behaves DIFFERENTLY at `intf` 0, 1 and 2 -- the LPC window length, the
        /// perceptual model's last-subframe window, the conditional LSF quantizer and the pitch
        /// predictor's packet-boundary reset all key off it. Pinning a row to one index and
        /// multiplying by three would report three copies of one path instead of the frame's real
        /// mix, so each such row cycles 0, 1, 2 and its x3 is the actual frame. One cursor per row,
        /// because divan runs the rows independently.
        intf_at: [usize; Self::INTF_ROWS],
        f2: [f32; SMPL_F_LEN],
        /// CELP inputs, one set per internal frame. Cycling `intf` alone would still hand
        /// `run_celp_subframes` the same predcoefs, residual, lags and perceptual weights every
        /// iteration, and the fixed-codebook search inside it is data-dependent -- it would settle
        /// on one pulse/survivor path and misreport the largest stage of the encoder.
        celp: Vec<CelpInputs>,
        /// Pitch inputs, one set per internal frame. See [`PitchInputs`].
        pitch: Vec<PitchInputs>,
        /// LSF inputs, one set per internal frame. See [`LsfInputs`].
        lsf: Vec<LsfInputs>,
        /// Entropy-coder inputs: analyzed params for a real frame, plus the encoder's own buffers.
        /// Analyzed parameters for each of the `STREAM` frames, and the cursor over them. One set
        /// would make the row re-serialize a single frame forever, and an entropy encode's cost
        /// tracks the parameters it writes (pulse counts, voiced vs. unvoiced block).
        fps: Vec<super::super::params::SmplFrameParams>,
        fp_at: usize,
        range: super::super::rangecoder::RangeEncoder,
        out: Vec<u8>,
    }

    impl Default for Stages {
        fn default() -> Self {
            Self::new()
        }
    }

    impl Stages {
        /// SETUP ONLY. Everything here is what production resolves once per stream and holds:
        /// table loads, twiddle construction, scratch pools, warmed cross-frame state.
        pub fn new() -> Self {
            let mut es = SmplEncoderState::default();
            for k in 0..WARMUP_FRAMES {
                let _ = smpl_analyze_frame_st(&mut es, &tone(k * SMPL_INTF_LEN * 3));
            }
            let pcm: Vec<Vec<f32>> = (0..STREAM)
                .map(|k| tone((WARMUP_FRAMES + k) * SMPL_INTF_LEN * 3))
                .collect();
            // Analyze the whole stream up front so `entropy_encode` has one parameter set per frame
            // to cycle. This runs before the buffer snapshots below so those stay consistent with
            // the state `es` is left in.
            let fps: Vec<_> = pcm
                .iter()
                .map(|f| smpl_analyze_frame_st(&mut es, f))
                .collect();

            let hp = es.hp.clone();
            let hp_pitch_hist = es.hp_pitch_hist.clone();

            // LPC front-end inputs for internal frame 0, built exactly as `smpl_analyze_frame_st`
            // builds them.
            let mut lpcbuf = [0f32; SMPL_LPC_BUF_LEN];
            let lpc_start = SMPL_LPC_HIST_LEN - SMPL_LPC_PRE;
            lpcbuf.copy_from_slice(&es.hp_full[lpc_start..lpc_start + SMPL_LPC_BUF_LEN]);
            let windowed = smpl_window_lpc20(&lpcbuf, true);
            let mut fft_scratch = new_lpc_fft_scratch();
            let mut fft_in = vec![0.0f32; SMPL_LPC_NFFT];
            fft_in[..SMPL_LPC_BUF_LEN].copy_from_slice(&windowed);
            let fft_out = vec![0.0f32; SMPL_LPC_NFFT];
            // Seed the N=576 row from real windowed signal (zero input would still cost the same
            // instructions, but a realistic magnitude keeps any future denormal effect honest).
            let mut fft576_scratch = FftScratch::new(PERCW_NFFT);
            let mut fft576_time = vec![0.0f32; PERCW_NFFT];
            fft576_time[..SMPL_LPC_BUF_LEN].copy_from_slice(&windowed);
            let mut fft576_spec = vec![0.0f32; PERCW_NFFT];
            let fft576_out = vec![0.0f32; PERCW_NFFT];
            rfft_forward_ordered_sc(&fft576_time, &mut fft576_spec, &mut fft576_scratch);
            // `f2` seeds the ctx's voicing-classifier input; the per-frame `a`/NLSF the LSF and
            // CELP rows need are captured in the per-frame loop below.
            let (_, f2) = smpl_lpc_analyze_with_f2(&windowed, &mut fft_scratch);

            let prev_lsfq = es.prev_lsfq.clone();
            let prev_nlsf = prev_lsfq.clone();

            let mut s = Stages {
                es,
                pcm,
                pcm_at: 0,
                hp,
                hp_pitch_hist,
                lpcbufs: Vec::new(),
                fft_in,
                fft_out,
                fft_scratch,
                fft576_time,
                fft576_spec,
                fft576_out,
                fft576_scratch,
                prev_nlsf,
                intf_at: [0; Self::INTF_ROWS],
                f2,
                celp: Vec::new(),
                pitch: Vec::new(),
                lsf: Vec::new(),
                fps,
                fp_at: 0,
                range: super::super::rangecoder::RangeEncoder::new(
                    1 + super::super::encode::SMPL_ENCODE_BUF_BYTES,
                ),
                out: Vec::with_capacity(512),
            };
            // Capture one CELP input set per internal frame, each from the live state at that
            // frame's own offset, so the row's inputs vary with the index it cycles.
            let synth_t = load_smpl_synth_tables();
            let res_lead = SMPL_ORDER + SMPL_WINNEXT_WB_LEN;
            // The committed envelope threads forward: frame f's `brec` is frame f+1's `prev_nlsf`.
            let mut prev_committed = s.prev_nlsf.clone();
            for f in 0..3 {
                // This frame's own LPC power spectrum: production recomputes it per internal frame,
                // and `smpl_pitch` reads it for harmonic strength.
                let mut lpcbuf_f = [0f32; SMPL_LPC_BUF_LEN];
                let start = SMPL_LPC_HIST_LEN - SMPL_LPC_PRE + f * SMPL_INTF_LEN;
                lpcbuf_f.copy_from_slice(&s.es.hp_full[start..start + SMPL_LPC_BUF_LEN]);
                let windowed_f = smpl_window_lpc20(&lpcbuf_f, f < 2);
                let (a_f, f2_f) = smpl_lpc_analyze_with_f2(&windowed_f, &mut s.fft_scratch);
                let nlsf_f = smpl_a2nlsf_16(&a_f);
                s.lpcbufs.push(lpcbuf_f);

                let perc_corrs: Vec<Vec<f32>> = s.with_ctx(f, [[0.0; 2]; SMPL_SUBFR_COUNT], |cs| {
                    compute_perc_corrs(cs).into()
                });
                // Roll this frame's weighted speech into `ltp_buf`, exactly as the analyzer does
                // before each estimator call, then snapshot the pair the row replays.
                s.with_ctx(f, [[0.0; 2]; SMPL_SUBFR_COUNT], |cs| {
                    build_ltp_buf(cs, &perc_corrs);
                });
                s.pitch.push(PitchInputs {
                    ltp_buf: s.es.ltp_buf.clone(),
                    f2: f2_f,
                });
                let pr = super::super::smpl_pitch_enc::smpl_pitch(
                    &mut s.es.pitch_est,
                    &s.es.ltp_buf,
                    &f2_f,
                    true,
                );
                let mut block_lags = [[0.0f32; 2]; SMPL_SUBFR_COUNT];
                for sf in 0..SMPL_SUBFR_COUNT {
                    block_lags[sf] = [pr.lags[2 * sf], pr.lags[2 * sf + 1]];
                }
                // Production sets BOTH `prev_lsfq` and `prev_nlsf` to the frame's committed NLSF
                // after every internal frame, and the LSF quantizer, the envelope reconstruction and
                // the interpolation that builds the CELP residual all read it. Thread one value
                // through all three rather than pinning them to the packet-start snapshot.
                let prev_for_frame = prev_committed.clone();
                let fe = FrontEndLsf {
                    a: a_f,
                    nlsf: nlsf_f,
                    prev_lsfq: &prev_for_frame,
                    prev_voiced: s.es.prev_voiced,
                    intf: f,
                };
                let (_, _, brec, _) = fe.quantize(synth_t, 1, &prev_for_frame);
                let (predcoefs, _) =
                    super::super::smpl_lpc::smpl_lpc_interpol(&brec, &prev_for_frame, smpl_nlsf2a);
                s.lsf.push(LsfInputs {
                    a: a_f,
                    nlsf: nlsf_f,
                    prev_nlsf: prev_for_frame,
                });
                prev_committed = brec;
                // The residual window for frame `f`, with its 32-sample CELP pre-lead.
                let nbase = res_lead + f * SMPL_INTF_LEN;
                let win_n = s.es.xn[nbase - res_lead..nbase + SMPL_INTF_LEN].to_vec();
                let mut res_lpc = vec![0f32; SMPL_INTF_LEN];
                for sf in 0..SMPL_SUBFR_COUNT {
                    let r = smpl_analysis_residual_subfr(&predcoefs[sf], &win_n, sf);
                    res_lpc[sf * SMPL_SUBFR_LEN..(sf + 1) * SMPL_SUBFR_LEN].copy_from_slice(&r);
                }
                s.celp.push(CelpInputs {
                    predcoefs,
                    res_lpc,
                    block_lags,
                    perc_corrs,
                });
            }
            // Warm the entropy coder's own `OnceLock` tables. The analysis path above never calls
            // `encode_smpl_frame_into`, so the range-coder CC tables are still cold here: leaving
            // them to the first timed body charged one `entropy_encode` sample 3.5 ms and ~1000
            // allocations for a stage whose real per-frame cost is 1.9 us and zero allocations --
            // a table build a live stream pays once, reported as if it were per-frame work.
            s.entropy_encode();
            s
        }

        /// Row slots in `intf_at`.
        const ROW_LPC: usize = 0;
        const ROW_PERC: usize = 1;
        const ROW_PITCH: usize = 2;
        const ROW_LSF: usize = 3;
        const ROW_CELP: usize = 4;
        const INTF_ROWS: usize = 5;

        /// Advance this row's cursor and return the internal-frame index to run, 0 -> 1 -> 2 -> 0.
        fn next_intf(&mut self, row: usize) -> usize {
            let intf = self.intf_at[row] % 3;
            self.intf_at[row] += 1;
            intf
        }

        /// Borrow the live per-stream state as the `CelpFrameCtx` the analyzer builds each internal
        /// frame. Pure borrows (no allocation), so calling it inside a timed body costs nothing the
        /// analyzer does not also pay.
        fn with_ctx<R>(
            &mut self,
            intf: usize,
            block_lags: [[f32; 2]; SMPL_SUBFR_COUNT],
            f: impl FnOnce(&mut CelpFrameCtx<'_>) -> R,
        ) -> R {
            let mut cs = CelpFrameCtx {
                celp: self.es.celp.as_mut().expect("primed by warmup"),
                perc: self.es.perc.as_mut().expect("primed by warmup"),
                perc_prev: &mut self.es.perc_prev,
                bitrate: self.es.bitrate.as_mut().expect("primed by warmup"),
                hp_n: &self.hp,
                intf,
                sp_act_prob: 1.0,
                coded_as_active_voice: true,
                f2: self.f2,
                voicing_strength: 1.0,
                vuv: &mut self.es.vuv,
                hp_pitch_hist: &self.hp_pitch_hist,
                ltp_buf: &mut self.es.ltp_buf,
                pitch_est: &mut self.es.pitch_est,
                perc_corrs: Vec::new(),
                block_lags,
            };
            f(&mut cs)
        }

        /// One forward real FFT at the LPC size (N=512, pure radix-2). Runs 3x per 60 ms frame, once
        /// per internal frame, inside [`Self::lpc_front_end`].
        pub fn fft512_forward(&mut self) -> f32 {
            rfft_forward_ordered_sc(&self.fft_in, &mut self.fft_out, &mut self.fft_scratch);
            self.fft_out[0]
        }

        /// One forward plus one inverse real FFT at the perceptual-model size (N=576 = 2^6 * 3^2,
        /// mixed radix 2 and 3). `smpl_perc_model` runs exactly this pair, 2x per internal frame, so
        /// 6x per 60 ms frame -- 12 of the frame's 15 FFTs.
        pub fn fft576_roundtrip(&mut self) -> f32 {
            rfft_forward_ordered_sc(
                &self.fft576_time,
                &mut self.fft576_spec,
                &mut self.fft576_scratch,
            );
            rfft_backward_ordered_sc(
                &self.fft576_spec,
                &mut self.fft576_out,
                &mut self.fft576_scratch,
            );
            self.fft576_out[0]
        }

        /// The full LPC front-end for one internal frame: window, forward FFT, power spectrum, DCT
        /// autocorrelation, Levinson, bandwidth expansion, and A->NLSF. Runs 3x per 60 ms frame.
        ///
        /// Cycles the internal-frame index because `smpl_analyze_frame_st` passes `f < 2` as
        /// `use_long_win`: internal frames 0 and 1 take the long trailing window, frame 2 the short
        /// one, and the two differ in window-generation work.
        pub fn lpc_front_end(&mut self) -> f32 {
            let intf = self.next_intf(Self::ROW_LPC);
            let windowed = smpl_window_lpc20(&self.lpcbufs[intf], intf < 2);
            let (a, _f2) = smpl_lpc_analyze_with_f2(&windowed, &mut self.fft_scratch);
            let nlsf = smpl_a2nlsf_16(&a);
            nlsf[0]
        }

        /// The perceptual model for one internal frame: two `smpl_perc_model` calls, each a forward
        /// plus an inverse FFT at N=576 (the mixed-radix 2/3 size). Runs 1x per internal frame, so
        /// 3x per 60 ms frame -- 12 of the frame's 15 FFTs. Advances the perc history, as production
        /// does.
        ///
        /// Cycles the internal-frame index: `compute_perc_corrs` sets `is_last` only on frame 2's
        /// final subframe, which switches `smpl_perc_model` to the short trailing window, so a row
        /// pinned to frame 1 would never reach one of the frame's six invocations.
        pub fn perc_corrs_frame(&mut self) -> f32 {
            let intf = self.next_intf(Self::ROW_PERC);
            self.with_ctx(intf, [[0.0; 2]; SMPL_SUBFR_COUNT], |cs| {
                compute_perc_corrs(cs)[0][0]
            })
        }

        /// The multi-stage pitch estimator for one internal frame. Runs 3x per 60 ms frame and
        /// advances the cross-frame lag predictor, as production does.
        ///
        /// Resets the lag predictor at each packet boundary, because `smpl_analyze_frame_st` calls
        /// `reset_cond` after internal frame 2: a packet is one non-conditional search followed by
        /// two conditional ones, and those take different candidate paths. Without the reset every
        /// timed call would be conditional and the x3 would not be a frame.
        pub fn pitch_search(&mut self) -> f32 {
            let intf = self.next_intf(Self::ROW_PITCH);
            if intf == 0 {
                self.es.pitch_est.reset_cond();
            }
            let inp = &self.pitch[intf];
            let pr = super::super::smpl_pitch_enc::smpl_pitch(
                &mut self.es.pitch_est,
                &inp.ltp_buf,
                &inp.f2,
                true,
            );
            pr.pitchcorr
        }

        /// The bit-exact LSF vector quantizer plus the decoder-side envelope reconstruction and
        /// NLSF->A that follow it (`FrontEndLsf::quantize`). Runs 1x per internal frame, 3x per
        /// 60 ms frame.
        ///
        /// The row cycles `intf` 0, 1, 2 because the two paths cost differently and production runs
        /// both every frame: the conditional quantizer needs `intf > 0`, so internal frame 0 always
        /// takes `lsf_quant` while 1 and 2 take `lsf_quant_cond` (which carries an extra centroid and
        /// rotated weighting matrices). Pinning the row to one path and multiplying by three would
        /// mis-state the frame; cycling makes the x3 the real mix. Stateless, so a full cycle
        /// measures the same work every time.
        pub fn lsf_quantize(&mut self) -> f32 {
            let intf = self.next_intf(Self::ROW_LSF);
            let inp = &self.lsf[intf];
            let fe = FrontEndLsf {
                a: inp.a,
                nlsf: inp.nlsf,
                prev_lsfq: &inp.prev_nlsf,
                prev_voiced: true,
                intf,
            };
            let (grid, _, _, predcoef) = fe.quantize(load_smpl_synth_tables(), 1, &inp.prev_nlsf);
            predcoef[1] + grid as f32
        }

        /// The CELP excitation encoder for one internal frame: the perceptual weighting filters plus
        /// four `encode_subframe` calls (ACB gain search, then the fixed-codebook pulse search that
        /// dominates it). Runs 1x per internal frame, so `encode_subframe` runs 12x per 60 ms frame.
        /// Advances the ACB/ZIR state, as production does.
        pub fn celp_subframes_frame(&mut self) -> f32 {
            let intf = self.next_intf(Self::ROW_CELP);
            // Lend this frame's input set out (leaving the empty `Default`), so the closure can hold
            // it while `with_ctx` borrows the rest of `self`.
            let inputs = std::mem::take(&mut self.celp[intf]);
            let r = self.with_ctx(intf, inputs.block_lags, |cs| {
                let outs = run_celp_subframes(
                    cs,
                    &inputs.predcoefs,
                    &inputs.res_lpc,
                    &inputs.block_lags,
                    &inputs.perc_corrs,
                    SMPL_PERC_EMPH_V,
                    1,
                );
                outs.iter().map(|o| o.n_pulses[1] as i32).sum::<i32>() as f32
            });
            self.celp[intf] = inputs;
            r
        }

        /// The whole analysis half of `encode`: everything above, plus the VAD, the input high-pass
        /// and the voiced/unvoiced decision. 1x per 60 ms frame.
        ///
        /// This plus [`Self::entropy_encode`] is the CODEC work, not quite all of `mlow_encode`:
        /// the public `MlowEncoder::encode_into` wrapper also sanitizes its 960 input samples
        /// (NaN -> 0, clamp, copy), and `mlow_encode` allocates a fresh output `Vec` per call where
        /// this harness reuses one. The `mlow_encode` vs `mlow_encode_reused_output` pair already
        /// isolates that allocation; the sanitize pass is the small remainder between these two rows
        /// and `mlow_encode_reused_output`.
        pub fn analyze_frame(&mut self) -> u8 {
            let frame = &self.pcm[self.pcm_at % self.pcm.len()];
            self.pcm_at += 1;
            let fp = smpl_analyze_frame_st(&mut self.es, frame);
            fp.toc
        }

        /// The range coder writing the analyzed parameters to the wire. 1x per 60 ms frame.
        pub fn entropy_encode(&mut self) -> usize {
            let fp = &self.fps[self.fp_at % self.fps.len()];
            self.fp_at += 1;
            super::super::encode::encode_smpl_frame_into(fp, &mut self.range, &mut self.out)
                .expect("analyzed params encode");
            self.out.len()
        }
    }
}

#[cfg(all(test, feature = "bench-internals"))]
mod stage_bench_tests {
    use super::stage_bench::Stages;

    /// The two rows whose inputs are fully captured must repeat with period 3 -- one packet -- so
    /// their measured cost is a property of the frame index, not of how long divan happened to run.
    /// `celp_subframes_frame` is deliberately excluded: its excitation state keeps evolving against
    /// the replayed residual, which the module doc records as a known limitation.
    #[test]
    fn stage_rows_are_three_periodic() {
        let mut s = Stages::new();
        let pitch: Vec<u32> = (0..12).map(|_| s.pitch_search().to_bits()).collect();
        let perc: Vec<u32> = (0..12).map(|_| s.perc_corrs_frame().to_bits()).collect();
        for i in 3..12 {
            assert_eq!(
                pitch[i],
                pitch[i - 3],
                "pitch_search drifted at iteration {i}"
            );
            assert_eq!(
                perc[i],
                perc[i - 3],
                "perc_model_frame drifted at iteration {i}"
            );
        }
    }
}
