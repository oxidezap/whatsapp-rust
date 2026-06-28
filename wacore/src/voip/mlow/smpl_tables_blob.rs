//! Shared codec for the MLow runtime constant tables: each table is stored on disk as a
//! zlib-compressed protobuf blob (`testdata/<x>.bin`, schema in `tables.proto`) inflated + decoded
//! once at load. Protobuf throughout so the byte-identical blobs stay portable across consumers.
//!
//! The JSON dumps (`testdata/<x>.json`, gitignored) are the source of truth (see
//! `testdata/PROVENANCE.md`); the `.bin` are generated from them by the `VOIP_GEN_TABLES` test in
//! this module. The load path is byte-identical to deserializing the JSON, so the codec output does
//! not change.

use std::io::Read;

/// Zlib level used by the generator. Fixed so re-running yields identical `.bin` bytes.
#[cfg(test)]
const GEN_ZLIB_LEVEL: u32 = 9;

/// Inflate a zlib blob (the runtime load path for the table `.bin`).
fn inflate(compressed: &[u8]) -> Vec<u8> {
    let mut dec = flate2::read::ZlibDecoder::new(compressed);
    let mut out = Vec::new();
    dec.read_to_end(&mut out)
        .expect("mlow table blob must zlib-inflate");
    out
}

/// Load a protobuf table from its embedded zlib blob.
pub(crate) fn load_blob_prost<T: prost::Message + Default>(compressed: &[u8]) -> T {
    let bytes = inflate(compressed);
    T::decode(bytes.as_slice()).expect("mlow table blob must protobuf-decode")
}

/// Zlib-compress already-encoded bytes (for callers that encode protobuf themselves). Deterministic.
#[cfg(test)]
pub(crate) fn make_blob_raw(bytes: &[u8]) -> Vec<u8> {
    use std::io::Write;
    let mut enc =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(GEN_ZLIB_LEVEL));
    enc.write_all(bytes).expect("zlib write");
    enc.finish().expect("zlib finish")
}

#[cfg(test)]
mod generator {
    use super::*;
    use std::fs;

    /// Read a JSON dump if present (cwd is the wacore crate root under `cargo test`); `None` if
    /// absent, so a partial dump set regenerates only the tables whose JSON is on disk.
    fn try_read_json(name: &str) -> Option<String> {
        match fs::read_to_string(format!("src/voip/mlow/testdata/{name}")) {
            Ok(s) => Some(s),
            Err(_) => {
                println!("skip {name}: JSON absent");
                None
            }
        }
    }

    fn write_bin(name: &str, bytes: &[u8]) {
        let path = format!("src/voip/mlow/testdata/{name}");
        fs::write(&path, bytes).unwrap_or_else(|e| panic!("write {path}: {e}"));
        println!("wrote {path} ({} bytes)", bytes.len());
    }

    /// Regenerate the runtime-table `.bin` from the (gitignored) JSON dumps. Env-gated so a normal
    /// `cargo test` run never touches the committed blobs. Deterministic: re-running yields the same
    /// bytes (fixed zlib level + protobuf's canonical encoding).
    ///
    ///   VOIP_GEN_TABLES=1 cargo test -p wacore --features voip gen_runtime_tables -- --nocapture
    #[test]
    fn gen_runtime_tables() {
        if std::env::var_os("VOIP_GEN_TABLES").is_none() {
            println!("VOIP_GEN_TABLES not set; skipping table generation");
            return;
        }

        // 1. LSF seed ROM: protobuf (tables.proto `LsfSeed`), then zlib. The expanded LSF tables
        //    (synth/lsf-cb/lsf-decode) are derived from this at load.
        if let Some(j) = try_read_json("lsf_seed.json") {
            use prost::Message;
            let seed = super::super::smpl_lsf_seed::seed_from_json(&j);
            write_bin("lsf_seed.bin", &make_blob_raw(&seed.encode_to_vec()));
        }

        // 2. pitch seed ROM: protobuf (tables.proto `PitchSeed`), then zlib.
        if let Some(j) = try_read_json("pitch_seed.json") {
            use prost::Message;
            let seed = super::super::smpl_pitch_seed::seed_from_json(&j);
            write_bin("pitch_seed.bin", &make_blob_raw(&seed.encode_to_vec()));
        }

        // 3. cc seed ROM: protobuf (tables.proto `CcSeed`), then zlib. The nrgres/gains (Group A/E)
        //    and LTP gain (Group C) CDFs are derived from this at load.
        if let Some(j) = try_read_json("cc_seed.json") {
            use prost::Message;
            let seed = super::super::smpl_cc_tables::seed_from_json(&j);
            write_bin("cc_seed.bin", &make_blob_raw(&seed.encode_to_vec()));
        }
    }
}
