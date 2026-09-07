//! Encodes raw pixels with the mozjpeg module, using the ABI its bytecode gave up.
//!
//! Nothing in this module is named: one export, `z`, takes six integers. What
//! they mean came out of `oracle abi php8T1oSIZM --index 273 --body 4000`:
//!
//! - the scanline loop compares `cinfo.next_scanline` against arg3 and indexes
//!   `arg0 + row * stride`, bounds-checked against arg1 — so arg0/arg1 are the
//!   pixel buffer and its length, and arg3 is the height;
//! - the stride is `frame[180] * frame[172]`, and arg2 is what lands in
//!   `frame[172]` — the width, in a three-component image;
//! - arg4 goes through `f32.convert_i32_u` and a clamp, which is what a quality
//!   setting looks like;
//! - arg5 is only ever `i32.eqz`-tested, so it is a flag.
//!
//! It returns a pointer to a `(ptr, len)` pair — the encoded JPEG.
use oracle_core::{Catalog, Runtime};
use wasmtime::Val;

/// Encoder input. The module writes `0xC_00000004` into the compress struct at
/// `cinfo+36`, which is `input_components = 4` followed by `in_color_space = 12`
/// — libjpeg-turbo's `JCS_EXT_RGBA`. So the buffer is RGBA, four bytes a pixel,
/// and passing three-byte pixels is what made every earlier attempt abort.
const WIDTH: u32 = 16;
const HEIGHT: u32 = 16;
const COMPONENTS: u32 = 4;

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve("php8T1oSIZM")?;
    let bytes = std::fs::read(&entry.path)?;

    let mut r = Runtime::instantiate(&bytes)?;
    r.call("y", &[])?;
    r.refuel();
    // The module starts with five pages; its allocator returns 0 for anything
    // that does not fit, so give it room before asking for buffers.
    r.grow_memory(64).ok();

    // A gradient rather than a flat colour: a solid image survives almost any
    // wrong stride, and would not tell us the encoder read the buffer the way
    // we think it does.
    let pixels: Vec<u8> = (0..WIDTH * HEIGHT)
        .flat_map(|i| {
            let (x, y) = (i % WIDTH, i / WIDTH);
            [(x * 16) as u8, (y * 16) as u8, 0x80, 0xff]
        })
        .collect();
    // The stride the comment above `COMPONENTS` derives from the compress
    // struct, asserted rather than described: getting it wrong is the mistake
    // this example exists to have already made.
    assert_eq!(pixels.len() as u32, WIDTH * HEIGHT * COMPONENTS);

    let alloc = |r: &mut Runtime, n: u32| -> u32 {
        let out = r.call("B", &[Val::I32(n as i32)]).ok();
        r.refuel();
        match out.as_ref().and_then(|v| v.first()) {
            Some(Val::I32(p)) => *p as u32,
            _ => 0,
        }
    };

    // `B` is this module's allocator; there is no exported `malloc`.
    let input = alloc(&mut r, pixels.len() as u32);
    r.write_bytes_at(input, &pixels)?;
    println!(
        "input: {} bytes of {WIDTH}x{HEIGHT} RGBA at {input:#x}",
        pixels.len()
    );

    for quality in [50i32, 90] {
        let args = [
            Val::I32(input as i32),
            Val::I32(pixels.len() as i32),
            Val::I32(WIDTH as i32),
            Val::I32(HEIGHT as i32),
            Val::I32(quality),
            Val::I32(0),
        ];

        let outcome = r.call("z", &args);
        r.refuel();

        let Some(Val::I32(pair)) = outcome.as_ref().ok().and_then(|out| out.first()) else {
            println!("quality {quality}: {:?}", outcome.err());
            continue;
        };

        // The pair is two words and the code does not say which is which, so
        // take the one that addresses a JPEG.
        let (first, second) = (
            r.read_u32_at(*pair as u32)?,
            r.read_u32_at(*pair as u32 + 4)?,
        );
        let (ptr, len) = match r.read(first, 2).as_deref() {
            Ok([0xff, 0xd8]) => (first, second),
            _ => (second, first),
        };
        let encoded = r.read(ptr, len)?;
        println!(
            "quality {quality}: {} bytes, magic {:02x?}, tail {:02x?}",
            encoded.len(),
            &encoded[..4.min(encoded.len())],
            &encoded[encoded.len().saturating_sub(2)..]
        );
    }

    Ok(())
}
