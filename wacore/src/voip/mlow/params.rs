//! Encoder-facing per-frame parameters: the structured output of the analysis, consumed by the
//! entropy encoder. The pulse/pitch blocks also carry the raw entropy symbols the encoder replays
//! (the structured counts alone are lossy w.r.t. the exact bitstream).

#[derive(Default, Clone)]
pub(crate) struct SmplLsfParams {
    pub stage1: i32,
    pub grid: i32,
    pub stage2: [i32; 16],
    pub extra: i32,
}

/// One uniform raw-symbol write (`encode(sym, sym+1, 1<<nbits)`).
#[derive(Clone, Copy)]
pub(crate) struct SmplRawSym {
    pub sym: u32,
    pub nbits: u32,
}

#[derive(Default, Clone)]
pub(crate) struct SmplPulseParams {
    pub total: i32,
    pub subfr: [i32; 4],
    /// Per-position run-length symbols (decodeCDF results) in read order across all subframes.
    pub mag_runs: Vec<i32>,
    /// Per-batch raw sign symbols in order.
    pub sign_syms: Vec<SmplRawSym>,
}

#[derive(Default, Clone)]
pub(crate) struct SmplGainParams {
    pub gain_main: i32,
    pub gain_delta: i32,
    pub nrg_res: [i32; 4],
}

#[derive(Default, Clone)]
#[allow(dead_code)] // populated for the voiced path, not read on the unvoiced encode
pub(crate) struct SmplPitchParams {
    pub gain_idx: [i32; 4],
    pub filt_idx: [i32; 4],
    /// The estimator's chosen contour (`blockseg_idx`) and per-40-block lag indices (`laginds`, 8
    /// entries). These ARE the wire pitch encoding: `smpl_encode_lags` writes the blockseg selector +
    /// the per-block (uniform-first / delta) lag indices straight from them, so the decoder rebuilds
    /// the full per-block contour instead of a flattened single lag.
    pub blockseg_idx: usize,
    pub laginds: [i32; 8],
}

#[derive(Default, Clone)]
pub(crate) struct SmplInternalParams {
    pub lsf: SmplLsfParams,
    pub pulses: SmplPulseParams,
    pub pitch: SmplPitchParams,
    pub gains: SmplGainParams,
}

/// Full decoded/analyzed parameter set for one 60 ms MLow frame.
pub(crate) struct SmplFrameParams {
    pub toc: u8,
    pub config: usize,
    pub internal: [SmplInternalParams; 3],
}
