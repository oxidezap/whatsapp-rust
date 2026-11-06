//! Encode/decode synthetic PCM with the separately built, pinned MLow reference.
//! Compile with rustc, linking its libopus.a. No codec implementation lives here.
use std::{
    ffi::c_void,
    fs::File,
    io::{self, Write},
    ptr::NonNull,
};

mod pcm;
use pcm::read_pcm_frame;

unsafe extern "C" {
    fn smpl_CreateCodec() -> i32;
    fn opus_encoder_create(
        rate: i32,
        channels: i32,
        application: i32,
        error: *mut i32,
    ) -> *mut c_void;
    fn opus_decoder_create(rate: i32, channels: i32, error: *mut i32) -> *mut c_void;
    fn opus_encoder_ctl(state: *mut c_void, request: i32, ...) -> i32;
    fn opus_decoder_ctl(state: *mut c_void, request: i32, ...) -> i32;
    fn opus_encode(
        state: *mut c_void,
        pcm: *const i16,
        samples: i32,
        packet: *mut u8,
        capacity: i32,
    ) -> i32;
    fn opus_decode(
        state: *mut c_void,
        packet: *const u8,
        length: i32,
        pcm: *mut i16,
        samples: i32,
        fec: i32,
    ) -> i32;
    fn opus_encoder_destroy(state: *mut c_void);
    fn opus_decoder_destroy(state: *mut c_void);
}
struct Codec {
    pointer: NonNull<c_void>,
    encoder: bool,
}
impl Drop for Codec {
    fn drop(&mut self) {
        // SAFETY: creation yielded this unique nonnull state; it is destroyed once.
        unsafe {
            if self.encoder {
                opus_encoder_destroy(self.pointer.as_ptr())
            } else {
                opus_decoder_destroy(self.pointer.as_ptr())
            }
        }
    }
}
fn check(code: i32, what: &str) -> Result<(), Box<dyn std::error::Error>> {
    if code != 0 {
        return Err(format!("{what}: reference error {code}").into());
    }
    Ok(())
}
fn create(encoder: bool) -> Result<Codec, Box<dyn std::error::Error>> {
    let mut error = 0;
    // SAFETY: error is writable; fixed parameters describe mono 16 kHz VOIP.
    let pointer = unsafe {
        if encoder {
            opus_encoder_create(16000, 1, 2048, &mut error)
        } else {
            opus_decoder_create(16000, 1, &mut error)
        }
    };
    let codec = Codec {
        pointer: NonNull::new(pointer).ok_or("null codec")?,
        encoder,
    };
    check(error, "create")?;
    Ok(codec)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(3..=5).contains(&args.len()) {
        return Err("usage: mlow_frames <input.raw> <frame_ms> [packets] [bitrate]".into());
    }
    let ms: i32 = args[2].parse()?;
    if ![10, 20, 60, 120].contains(&ms) {
        return Err("frame_ms must be 10, 20, 60 or 120".into());
    }
    let want: i32 = args.get(3).map_or(Ok(-1), |s| s.parse())?;
    let bitrate: i32 = args.get(4).map_or(Ok(20000), |s| s.parse())?;
    if bitrate <= 0 {
        return Err("bitrate must be positive".into());
    }
    // SAFETY: the reference requires one global initialization before creating codecs.
    check(unsafe { smpl_CreateCodec() }, "smpl_CreateCodec")?;
    let encoder = create(true)?;
    let decoder = create(false)?;
    // SAFETY: live unique codec states; these pinned-reference requests take one C int.
    unsafe {
        check(
            opus_encoder_ctl(encoder.pointer.as_ptr(), 4050, 1_i32),
            "encoder SMPL",
        )?;
        check(
            opus_encoder_ctl(encoder.pointer.as_ptr(), 4002, bitrate),
            "bitrate",
        )?;
        check(
            opus_encoder_ctl(encoder.pointer.as_ptr(), 4022, 1_i32),
            "channels",
        )?;
        check(
            opus_decoder_ctl(decoder.pointer.as_ptr(), 4050, 1_i32),
            "decoder SMPL",
        )?;
    }
    let samples = (16 * ms) as usize;
    let mut input = File::open(&args[1])?;
    let mut raw = vec![0; samples * 2];
    let mut pcm = vec![0_i16; samples];
    let mut decoded = vec![0_i16; samples];
    let mut packet = [0_u8; 400];
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    let mut emitted = 0;
    while want < 0 || emitted < want {
        if !read_pcm_frame(&mut input, &mut raw)? {
            break;
        }
        for (sample, bytes) in pcm.iter_mut().zip(raw.chunks_exact(2)) {
            *sample = i16::from_le_bytes([bytes[0], bytes[1]]);
        }
        // SAFETY: buffers have samples and 400 bytes respectively, matching passed bounds.
        let length = unsafe {
            opus_encode(
                encoder.pointer.as_ptr(),
                pcm.as_ptr(),
                samples as i32,
                packet.as_mut_ptr(),
                packet.len() as i32,
            )
        };
        if length <= 0 || length as usize > packet.len() {
            return Err(format!("encode packet {emitted}: {length}").into());
        }
        // SAFETY: encoded length is bounded above; decoder output holds samples i16s.
        let count = unsafe {
            opus_decode(
                decoder.pointer.as_ptr(),
                packet.as_ptr(),
                length,
                decoded.as_mut_ptr(),
                samples as i32,
                0,
            )
        };
        if count <= 0 || count as usize > decoded.len() {
            return Err(format!("decode packet {emitted}: {count}").into());
        }
        for b in &packet[..length as usize] {
            write!(stdout, "{b:02x}")?;
        }
        write!(stdout, " ")?;
        for sample in &decoded[..count as usize] {
            for b in sample.to_le_bytes() {
                write!(stdout, "{b:02x}")?;
            }
        }
        writeln!(stdout)?;
        eprintln!(
            "packet {emitted}: {length} bytes, TOC 0x{:02x}, {count} samples",
            packet[0]
        );
        emitted += 1;
    }
    stdout.flush()?;
    eprintln!("emitted {emitted} packet(s) of {ms} ms");
    if emitted == 0 || (want >= 0 && emitted != want) {
        return Err("input yielded fewer packets than requested".into());
    }
    Ok(())
}
fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
