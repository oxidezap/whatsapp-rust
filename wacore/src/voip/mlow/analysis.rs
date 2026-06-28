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
    vuv: super::smpl_signal_mode::VuvMode,
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
    let pcm_i16: Vec<i16> = pcm[..need]
        .iter()
        .map(|&s| (s * 32768.0).round().clamp(-32768.0, 32767.0) as i16)
        .collect();
    let vad = es
        .vad
        .get_or_insert_with(super::smpl_vad::SmplVadState::new)
        .process_packet(&pcm_i16, SMPL_INTF_LEN);
    let sp_act_prob = vad.vad_results;
    let coded_as_active_voice = vad.coded_as_active_voice;

    // Encoder input high-pass (ARMA2, fcorner 35 Hz), matching the real encoder. Removes the
    // low-frequency content the decoder's de-emphasis would otherwise over-amplify; the residual the
    // analysis codes is then in the same band the real codec quantizes.
    let (hp_ma, hp_ar) = *es
        .hp_coefs
        .get_or_insert_with(|| smpl_get_hp_coefs(SMPL_ENC_HP_FCORNER_HZ));
    let pcm_in: Vec<f32> = pcm[..need].to_vec();
    let mut hp = vec![0f32; need];
    smpl_filt_arma2(&pcm_in, need, hp_ma, hp_ar, &mut es.hp_state, &mut hp);

    // int16-scaled input with smplOrder lead samples of history.
    let mut x = vec![0f64; SMPL_ORDER + need];
    if es.hist.len() >= SMPL_ORDER {
        x[..SMPL_ORDER].copy_from_slice(&es.hist[es.hist.len() - SMPL_ORDER..]);
    }
    for i in 0..need {
        x[SMPL_ORDER + i] = hp[i] as f64 * 32768.0;
    }

    let mut shadow = SmplFrameSynth::default();
    let mut prev_nlsf: Vec<f32> = Vec::new();
    // Predictor mirror, fresh per 60 ms frame (mirrors encode_smpl_frame's fresh SmplLsfState),
    // threaded across the 3 internal frames so the voiced abs-vs-delta lag choice matches the
    // entropy encoder.
    let mut lstate = super::smpl_decode::SmplLsfState::default();

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
    let mut xn = vec![0f32; res_lead + need];
    if es.hist.len() >= res_lead {
        for i in 0..res_lead {
            xn[i] = (es.hist[es.hist.len() - res_lead + i] / 32768.0) as f32;
        }
    }
    xn[res_lead..res_lead + need].copy_from_slice(&hp[..need]);

    // Full HP-domain buffer that `lpcbuf` indexes: [history(144)] ++ [current 960 HP] ++ [32 zeros].
    // The 32-sample lookahead tail is zero at 16 kHz (no band split), per the buffer layout.
    let mut hp_full = vec![0f32; SMPL_LPC_HIST_LEN + need + SMPL_WINNEXT_WB_LEN];
    if es.lpc_hist.len() == SMPL_LPC_HIST_LEN {
        hp_full[..SMPL_LPC_HIST_LEN].copy_from_slice(&es.lpc_hist);
    }
    hp_full[SMPL_LPC_HIST_LEN..SMPL_LPC_HIST_LEN + need].copy_from_slice(&hp[..need]);

    // Snapshot the previous packet's HP tail (pitch history for this packet's intf=0), then refresh it
    // from this packet's tail for the next call.
    let mut hp_pitch_hist = vec![0f32; SMPL_PITCH_LAG_MAX];
    if es.hp_pitch_hist.len() == SMPL_PITCH_LAG_MAX {
        hp_pitch_hist.copy_from_slice(&es.hp_pitch_hist);
    }
    es.hp_pitch_hist = hp[need - SMPL_PITCH_LAG_MAX..need].to_vec();

    // Lazily size the persistent perceptually-weighted speech buffer (`ltp_buf`).
    if es.ltp_buf.len() != super::smpl_pitch_enc::MAX_LTP_BUF_LEN {
        es.ltp_buf = vec![0.0f32; super::smpl_pitch_enc::MAX_LTP_BUF_LEN];
    }

    let celp = es.celp.as_mut().expect("celp built above");
    let perc = es.perc.as_mut().expect("perc built above");
    let bitrate = es.bitrate.as_mut().expect("bitrate built above");
    let ltp_buf = &mut es.ltp_buf;
    let pitch_est = &mut es.pitch_est;

    let mut prev_lsfq = es.prev_lsfq.clone();
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
            hp_n: &hp,
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
    es.hist = x[x.len() - (SMPL_ORDER + SMPL_WINNEXT_WB_LEN)..].to_vec();
    // Carry the last 144 HP samples as next packet's LPC window history (mirrors `lpc_buf_mem`).
    es.lpc_hist = hp[need - SMPL_LPC_HIST_LEN..need].to_vec();
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
        let mut positions: Vec<(usize, i32)> = Vec::new();
        for n in base_pos..base_pos + pos_per {
            if pulse[n] != 0 {
                positions.push((n, pulse[n]));
            }
        }
        let mut run_pos = base_pos as i32;
        let mut first = true;
        for &(p, magv) in &positions {
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
