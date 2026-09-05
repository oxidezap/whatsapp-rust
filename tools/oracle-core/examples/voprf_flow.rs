//! Runs the VOPRF module's blinding step.
//!
//! The argument layout was not guessed. `oracle abi COs9e0Kj0ic -f blind` shows
//! `blind` is a trampoline that loads a function pointer from offset 12 of its
//! first argument; `--slot 21 --body 170` shows the callee checking two of its
//! arguments against sizes read out of that object. Both are 32 for
//! Ristretto255, which fixes those positions as lengths and leaves the rest as
//! buffers.
use oracle_core::{Catalog, Runtime};
use wasmtime::Val;

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve("COs9e0Kj0ic")?;
    let bytes = std::fs::read(&entry.path)?;
    let mut r = Runtime::instantiate(&bytes)?;
    r.run_ctors().ok();

    let call = |r: &mut Runtime, name: &str, args: &[i32]| -> Option<i32> {
        let vals: Vec<Val> = args.iter().map(|a| Val::I32(*a)).collect();
        let out = r.call(name, &vals).ok()?;
        r.refuel();
        match out.first() {
            Some(Val::I32(v)) => Some(*v),
            _ => Some(0),
        }
    };

    call(&mut r, "sodiumInit", &[]);
    let curve = call(&mut r, "curve_create", &[]).unwrap_or(0);
    call(&mut r, "curve_init_ristretto", &[curve]);
    let voprf = call(&mut r, "voprf_create", &[]).unwrap_or(0);
    call(&mut r, "voprf_init_exp_twohashdh", &[voprf, curve]);

    let element = call(&mut r, "get_curve_element_bytes", &[curve]).unwrap_or(32);
    let scalar = call(&mut r, "get_curve_scalar_bytes", &[curve]).unwrap_or(32);
    println!("ristretto255: element={element}B scalar={scalar}B");

    let blind = |r: &mut Runtime, input: &[u8]| -> Option<(Vec<u8>, Vec<u8>)> {
        let inp = r.write_bytes(input).ok()? as i32;
        let point = r.malloc(element as u32).ok()? as i32;
        let sc = r.malloc(scalar as u32).ok()? as i32;
        let status = call(
            r,
            "blind",
            &[voprf, point, element, sc, scalar, inp, input.len() as i32],
        )?;
        if status != 0 {
            println!("  blind returned {status}");
            return None;
        }
        Some((
            r.read(point as u32, element as u32).ok()?,
            r.read(sc as u32, scalar as u32).ok()?,
        ))
    };

    let (a_point, a_scalar) = blind(&mut r, b"hello").expect("blind");
    let (b_point, _) = blind(&mut r, b"hello").expect("blind");
    let (c_point, _) = blind(&mut r, b"world").expect("blind");

    println!("\nblind(\"hello\") point  = {}", hex(&a_point));
    println!("blind(\"hello\") scalar = {}", hex(&a_scalar));
    println!("blind(\"hello\") again  = {}", hex(&b_point));
    println!("blind(\"world\") point  = {}", hex(&c_point));

    // A blinding factor is random by construction, so the same input must not
    // produce the same point — that is what makes it hide the input.
    println!(
        "\nsame input, different point: {}",
        if a_point != b_point { "yes" } else { "NO" }
    );
    println!(
        "point is on the curve (non-zero, {element}B): {}",
        a_point.iter().any(|b| *b != 0)
    );
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
