//! Differential tests: the captured module against a Rust implementation.
//!
//! This is the shape a whatsapp-rust comparison takes. The oracle supplies the
//! ground truth, the Rust side supplies the candidate, and the test asserts they
//! agree over a range of inputs rather than at a single point.

use oracle_core::{Catalog, Runtime, Value};
use wasmtime::Val;

/// The capture these comparisons treat as ground truth. Named once: see the
/// note on the same constant in `abi_inference.rs`.
const VOIP: &str = "JgwtTQVeWPm";

/// `wasmtime::Val` has no `PartialEq`, so results are compared through their
/// scalar value.
fn as_i32(results: &[Val]) -> Option<i32> {
    match results.first() {
        Some(Val::I32(value)) => Some(*value),
        _ => None,
    }
}

fn module(id: &str) -> Option<Runtime> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(id).ok()?;
    let bytes = std::fs::read(&entry.path).ok()?;
    Runtime::instantiate(&bytes).ok()
}

macro_rules! module_or_skip {
    ($id:expr) => {
        match module($id) {
            Some(runtime) => runtime,
            None => {
                eprintln!("skipping: {} unavailable (set WA_WASM_DIR)", $id);
                return;
            }
        }
    };
}

/// The Rust candidate.
///
/// Not `value as f32 / 2f32.powi(n)`: the module splits the value into an
/// integer part and a fraction and adds them **in `f32`**, which is observably
/// different. At `n >= 25` a negative value's fraction rounds to exactly `1.0`
/// and cancels the integer part, so the module returns `0` where a plain
/// division returns a small negative number. The oracle is what revealed this;
/// the naive formula passed every case below `n = 25`.
fn fixed_to_float(value: i32, frac_bits: u32) -> f32 {
    let whole = value >> frac_bits;
    let divisor = 1u32 << frac_bits;
    let fraction = (value as u32) & (divisor - 1);
    whole as f32 + fraction as f32 / divisor as f32
}

/// Shift counts of 32 and above are out of range for a 32-bit shift. The module
/// returns 2^32 for any input there, which is not a meaningful conversion, so
/// the comparison stays inside the defined range.
const MAX_FRAC_BITS: u32 = 31;

#[test]
fn fixed_point_conversion_matches_the_module() {
    let mut runtime = module_or_skip!("rogm88TRRiw");

    let values = [
        0i32,
        1,
        -1,
        2,
        -2,
        100,
        -100,
        0x8000,
        0x10000,
        -0x10000,
        0xffff,
        i32::MAX,
        i32::MIN + 1,
    ];

    for value in values {
        for frac in 0..=MAX_FRAC_BITS {
            let results = runtime
                .call(
                    "convertFixed32BitToFloat",
                    &[Val::I32(value), Val::I32(frac as i32)],
                )
                .unwrap_or_else(|error| panic!("({value}, {frac}): {error:#}"));

            let Some(Val::F32(bits)) = results.first() else {
                panic!("({value}, {frac}) returned {results:?}");
            };
            let from_module = f32::from_bits(*bits);
            let from_rust = fixed_to_float(value, frac);

            assert_eq!(
                from_module.to_bits(),
                from_rust.to_bits(),
                "({value}, {frac}): module gave {from_module:e}, Rust gave {from_rust:e}"
            );
            runtime.refuel();
        }
    }
}

/// The rounding case the naive formula gets wrong, pinned on its own so a
/// regression names the actual problem.
#[test]
fn fixed_point_flushes_small_negatives_to_zero() {
    let mut runtime = module_or_skip!("rogm88TRRiw");

    for frac in 25..=MAX_FRAC_BITS {
        let results = runtime
            .call(
                "convertFixed32BitToFloat",
                &[Val::I32(-1), Val::I32(frac as i32)],
            )
            .expect("call");
        let Some(Val::F32(bits)) = results.first() else {
            panic!("unexpected {results:?}");
        };
        assert_eq!(
            f32::from_bits(*bits),
            0.0,
            "f(-1, {frac}) should collapse to zero"
        );
        runtime.refuel();
    }
}

/// Guards the interpretation itself: were the module treating its input as
/// unsigned, this case would come back as a large positive number.
#[test]
fn fixed_point_input_is_signed() {
    let mut runtime = module_or_skip!("rogm88TRRiw");

    let results = runtime
        .call(
            "convertFixed32BitToFloat",
            &[Val::I32(0xffff_0000u32 as i32), Val::I32(16)],
        )
        .expect("call");
    let Some(Val::F32(bits)) = results.first() else {
        panic!("unexpected result {results:?}");
    };
    assert_eq!(f32::from_bits(*bits), -1.0);
}

/// Vector marshalling is verified by asking the module for the length rather
/// than by trusting that the pushes were accepted.
#[test]
fn embind_vectors_round_trip() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    for (class_name, numbers, strings) in [
        ("Uint8List", vec![0i64, 127, 255], vec![]),
        ("IntList", vec![-5i64, 0, 5, 1000], vec![]),
        (
            "StringList",
            vec![],
            vec!["one".to_owned(), "two".to_owned(), "three".to_owned()],
        ),
    ] {
        let class = runtime
            .class_type_by_name(class_name)
            .unwrap_or_else(|| panic!("{class_name} not registered"));
        let handle = runtime
            .build_vector(class, &numbers, &strings)
            .unwrap_or_else(|error| panic!("{class_name}: {error:#}"));

        let expected = numbers.len().max(strings.len()) as i64;
        assert_eq!(
            runtime.vector_len(handle).expect("size"),
            expected,
            "{class_name} length"
        );
        runtime.release(handle).expect("release");
    }
}

/// A vector holding strings must reject numbers, and vice versa: a silent
/// coercion here would produce a plausible but wrong argument.
#[test]
fn embind_vectors_reject_the_wrong_element_type() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    let strings = runtime
        .class_type_by_name("StringList")
        .expect("StringList");
    assert!(runtime.build_vector(strings, &[1, 2, 3], &[]).is_err());

    let bytes = runtime.class_type_by_name("Uint8List").expect("Uint8List");
    assert!(
        runtime
            .build_vector(bytes, &[], &["not a byte".to_owned()])
            .is_err()
    );
}

/// The signaling entry point has to accept a call offer without raising.
/// Before exception handling and WASI were implemented, this trapped.
#[test]
fn signaling_offer_is_accepted() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");
    runtime.clear_calls();

    let result = runtime.call_embind(
        "handleIncomingSignalingOffer",
        &[
            Value::Str("1234567890123456".to_owned()),
            // Fictitious numbers only.
            Value::Str("15550001111@s.whatsapp.net".to_owned()),
            Value::Str("15550002222@s.whatsapp.net".to_owned()),
            Value::Str("offer".to_owned()),
            Value::Str("1735689600".to_owned()),
            Value::Bool(false),
            Value::Bool(false),
            Value::Str(String::new()),
            Value::Bytes(vec![0x08, 0x01, 0x10, 0x02]),
        ],
    );

    assert_eq!(result.expect("offer should be accepted"), Value::Void);

    let raised: Vec<String> = runtime
        .logs()
        .into_iter()
        .filter(|line| line.starts_with("C++ exception:"))
        .collect();
    assert!(raised.is_empty(), "offer raised exceptions: {raised:?}");
}

#[test]
fn voip_stack_initialises() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    let result = runtime
        .call_embind(
            "initVoipStack",
            &[
                Value::Str("15550002222@s.whatsapp.net".to_owned()),
                Value::Str("0".to_owned()),
                Value::Str("{}".to_owned()),
            ],
        )
        .expect("initVoipStack should return");

    // Pinning the value makes a change in the engine's startup contract visible
    // rather than silent; it is not asserted to mean success.
    assert_eq!(result, Value::Int(120011));
}

/// The VOPRF module exposes plain C exports rather than an embind API, so an
/// empty registry is the correct result there and not a recovery failure.
#[test]
fn voprf_module_is_callable_without_embind() {
    let mut runtime = module_or_skip!("COs9e0Kj0ic");
    runtime.run_ctors().expect("ctors");

    assert!(
        runtime.embind().functions.is_empty(),
        "the VOPRF module is not expected to register an embind API"
    );

    assert_eq!(
        as_i32(&runtime.call("sodiumInit", &[]).expect("sodiumInit")),
        Some(0),
        "libsodium should initialise"
    );

    let curve = runtime.call("curve_create", &[]).expect("curve_create");
    let handle = as_i32(&curve).unwrap_or_else(|| panic!("curve_create returned {curve:?}"));
    assert_ne!(handle, 0, "curve_create should return a handle");

    runtime
        .call("curve_init_ristretto", &[Val::I32(handle)])
        .expect("curve_init_ristretto");

    // Ristretto255 group elements and scalars are both 32 bytes. Reading these
    // back confirms the curve was really initialised, not merely allocated.
    for accessor in ["get_curve_element_bytes", "get_curve_scalar_bytes"] {
        let size = runtime
            .call(accessor, &[Val::I32(handle)])
            .unwrap_or_else(|error| panic!("{accessor}: {error:#}"));
        assert_eq!(as_i32(&size), Some(32), "{accessor}");
    }
}

/// The VOPRF construction the module implements, read from the module itself.
///
/// Every object initialises and reports its sizes: Ristretto255 for the group,
/// exponential two-hash-DH for the OPRF, Naor-Reingold for the KDF. The full
/// blind -> evaluate -> unblind exchange is not driven here — those entry points
/// take up to sixteen undocumented arguments, and calling them with a guessed
/// layout would prove nothing.
#[test]
fn voprf_primitives_initialise_with_their_declared_sizes() {
    let mut runtime = module_or_skip!("COs9e0Kj0ic");
    runtime.run_ctors().expect("ctors");

    assert_eq!(
        as_i32(&runtime.call("sodiumInit", &[]).expect("sodiumInit")),
        Some(0)
    );

    let curve = as_i32(&runtime.call("curve_create", &[]).expect("curve_create")).expect("handle");
    runtime
        .call("curve_init_ristretto", &[Val::I32(curve)])
        .expect("ristretto");

    let voprf = as_i32(&runtime.call("voprf_create", &[]).expect("voprf_create")).expect("handle");
    runtime
        .call(
            "voprf_init_exp_twohashdh",
            &[Val::I32(voprf), Val::I32(curve)],
        )
        .expect("twohashdh");

    // A VOPRF evaluation is two group elements wide, twice the 32-byte element.
    let final_bytes = runtime
        .call("get_voprf_final_evaluation_bytes", &[Val::I32(voprf)])
        .expect("final evaluation size");
    assert_eq!(as_i32(&final_bytes), Some(64));

    let kdf = as_i32(&runtime.call("kdf_create", &[]).expect("kdf_create")).expect("handle");
    runtime
        .call("kdf_init_naor_reingold", &[Val::I32(kdf), Val::I32(curve)])
        .expect("naor-reingold");

    for (name, handle) in [
        ("curve_free", curve),
        ("voprf_free", voprf),
        ("kdf_free", kdf),
    ] {
        runtime
            .call(name, &[Val::I32(handle)])
            .unwrap_or_else(|error| panic!("{name}: {error:#}"));
    }
}

/// Reading a vector back out of the module.
///
/// This is the direction that was missing for a long time: a vector could be
/// built and handed over, but `get` returns `emscripten::val`, so whatever the
/// module put in one was unreadable. Values are compared after a full
/// round trip, not just counted.
#[test]
fn vectors_round_trip_through_the_module() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    /// One vector class, what to put in it, and what should come back.
    struct Case {
        class: &'static str,
        numbers: Vec<i64>,
        strings: Vec<String>,
        expected: Vec<Value>,
    }

    let cases = vec![
        Case {
            class: "Uint8List",
            numbers: vec![0, 7, 42, 255],
            strings: vec![],
            expected: vec![
                Value::Int(0),
                Value::Int(7),
                Value::Int(42),
                Value::Int(255),
            ],
        },
        Case {
            // Signedness is the interesting case: a byte-wise reader would turn
            // -5 into 251.
            class: "IntList",
            numbers: vec![-5, 0, 12345],
            strings: vec![],
            expected: vec![Value::Int(-5), Value::Int(0), Value::Int(12345)],
        },
        Case {
            class: "StringList",
            numbers: vec![],
            strings: vec!["alpha".to_owned(), "beta".to_owned()],
            expected: vec![
                Value::Str("alpha".to_owned()),
                Value::Str("beta".to_owned()),
            ],
        },
    ];

    for case in cases {
        let class_name = case.class;
        let class = runtime
            .class_type_by_name(class_name)
            .unwrap_or_else(|| panic!("{class_name} not registered"));
        let handle = runtime
            .build_vector(class, &case.numbers, &case.strings)
            .unwrap_or_else(|error| panic!("{class_name}: {error:#}"));

        let read_back = runtime
            .read_vector(handle)
            .unwrap_or_else(|error| panic!("{class_name} read back: {error:#}"));
        assert_eq!(read_back, case.expected, "{class_name} did not round trip");

        runtime.release(handle).expect("release");
        runtime.refuel();
    }
}

/// emval handles must not accumulate. Each `get` mints one, and a harness that
/// never released them would grow the table without bound across a sweep.
#[test]
fn emval_handles_do_not_leak() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    let class = runtime.class_type_by_name("IntList").expect("IntList");
    let handle = runtime.build_vector(class, &[1, 2, 3], &[]).expect("build");

    for _ in 0..5 {
        runtime.read_vector(handle).expect("read");
        runtime.refuel();
    }
    runtime.release(handle).expect("release");

    assert_eq!(
        runtime.emval_live(),
        0,
        "emval handles were left behind after reading"
    );
}

/// Calling a method directly, rather than through the vector helpers.
#[test]
fn class_methods_are_callable() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    let class = runtime.class_type_by_name("IntList").expect("IntList");
    let handle = runtime.build_vector(class, &[10, 20], &[]).expect("build");

    // A `bool` return comes back as one, rather than as 0/1.
    assert_eq!(
        runtime
            .call_method(handle, "set", &[Value::Int(0), Value::Int(99)])
            .expect("set"),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .call_method(handle, "get", &[Value::Int(0)])
            .expect("get"),
        Value::Int(99),
        "set should be visible through get"
    );

    // NOTE: embind's `set` does not bounds-check — it writes through
    // `v[index]` and returns true regardless. An out-of-range index is
    // undefined behaviour in the guest, not a rejected call, so it is
    // deliberately not exercised here. The oracle is what established that;
    // the signature (`-> bool`) reads like a validation that does not exist.

    runtime.release(handle).expect("release");
}

/// An unknown method is an error rather than a guess.
#[test]
fn unknown_methods_are_rejected() {
    let mut runtime = module_or_skip!(VOIP);
    runtime.run_ctors().expect("ctors");

    let class = runtime.class_type_by_name("IntList").expect("IntList");
    let handle = runtime.build_vector(class, &[1], &[]).expect("build");

    assert!(runtime.call_method(handle, "no_such_method", &[]).is_err());
    runtime.release(handle).expect("release");
}

/// The VOPRF blinding step, driven through the ABI its own bytecode revealed.
///
/// `blind` is a trampoline: it loads a function pointer from offset 12 of its
/// first argument, and the callee checks two arguments against sizes held in
/// that object — the curve's element and scalar lengths. That fixes the layout
/// as `(object, out_point, element_len, out_scalar, scalar_len, input, len)`.
/// Nothing here was guessed; see `oracle abi COs9e0Kj0ic --slot 21 --body 170`.
#[test]
fn voprf_blinding_produces_a_curve_point() {
    let mut runtime = module_or_skip!("COs9e0Kj0ic");
    runtime.run_ctors().ok();

    let call = |runtime: &mut Runtime, name: &str, args: &[i32]| -> i32 {
        let vals: Vec<Val> = args.iter().map(|a| Val::I32(*a)).collect();
        let out = runtime
            .call(name, &vals)
            .unwrap_or_else(|error| panic!("{name}: {error:#}"));
        runtime.refuel();
        as_i32(&out).unwrap_or(0)
    };

    call(&mut runtime, "sodiumInit", &[]);
    let curve = call(&mut runtime, "curve_create", &[]);
    call(&mut runtime, "curve_init_ristretto", &[curve]);
    let voprf = call(&mut runtime, "voprf_create", &[]);
    call(&mut runtime, "voprf_init_exp_twohashdh", &[voprf, curve]);

    let element = call(&mut runtime, "get_curve_element_bytes", &[curve]);
    let scalar = call(&mut runtime, "get_curve_scalar_bytes", &[curve]);
    assert_eq!((element, scalar), (32, 32), "Ristretto255 sizes");

    let blind = |runtime: &mut Runtime, input: &[u8]| -> Vec<u8> {
        let inp = runtime.write_bytes(input).expect("input") as i32;
        let point = runtime.malloc(element as u32).expect("out") as i32;
        let out_scalar = runtime.malloc(scalar as u32).expect("out") as i32;

        let status = call(
            runtime,
            "blind",
            &[
                voprf,
                point,
                element,
                out_scalar,
                scalar,
                inp,
                input.len() as i32,
            ],
        );
        assert_eq!(status, 0, "blind should succeed");
        runtime.read(point as u32, element as u32).expect("read")
    };

    let hello = blind(&mut runtime, b"hello");
    let world = blind(&mut runtime, b"world");

    assert_eq!(hello.len(), 32);
    assert!(
        hello.iter().any(|byte| *byte != 0),
        "a blinded point should not be zero"
    );
    // The point has to depend on the input, or nothing was computed.
    assert_ne!(
        hello, world,
        "different inputs must blind to different points"
    );
    // And be reproducible, which is what makes it usable as an oracle.
    assert_eq!(hello, blind(&mut runtime, b"hello"));
}

/// The mozjpeg module encodes RGBA pixels to a JPEG.
///
/// Nothing in this module is named — one export, `z`, taking six integers — so
/// the whole call comes from `oracle abi php8T1oSIZM --index 273`:
///
/// - the scanline loop bounds `arg0 + row * stride` by arg1 and compares
///   `cinfo.next_scanline` against arg3, making arg0/arg1 the pixel buffer and
///   arg3 the height;
/// - `i64.store offset=180` writes `0xC_00000004` into the compress struct,
///   which is `input_components = 4` and `in_color_space = 12`
///   (libjpeg-turbo's `JCS_EXT_RGBA`) — so the pixels are RGBA, and arg2 is the
///   width of a four-byte-per-pixel image;
/// - arg4 goes through `f32.convert_i32_u` and a clamp: a quality setting.
#[test]
fn mozjpeg_encodes_rgba_pixels() {
    const WIDTH: u32 = 16;
    const HEIGHT: u32 = 16;

    let mut runtime = module_or_skip!("php8T1oSIZM");
    runtime.call("y", &[]).expect("run constructors");
    runtime.refuel();
    runtime.grow_memory(64).ok();

    // A gradient, not a flat colour: a solid image survives almost any wrong
    // stride and would not show that the encoder read the buffer as we think.
    let pixels: Vec<u8> = (0..WIDTH * HEIGHT)
        .flat_map(|i| {
            [
                ((i % WIDTH) * 16) as u8,
                ((i / WIDTH) * 16) as u8,
                0x80,
                0xff,
            ]
        })
        .collect();

    // `B` is this module's allocator; it exports no `malloc`.
    let input = match runtime.call("B", &[Val::I32(pixels.len() as i32)]) {
        Ok(out) => as_i32(&out).expect("allocation") as u32,
        Err(error) => panic!("allocating input: {error:#}"),
    };
    runtime.refuel();
    runtime
        .write_bytes_at(input, &pixels)
        .expect("write pixels");

    let encode = |runtime: &mut Runtime, quality: i32| -> Vec<u8> {
        let out = runtime
            .call(
                "z",
                &[
                    Val::I32(input as i32),
                    Val::I32(pixels.len() as i32),
                    Val::I32(WIDTH as i32),
                    Val::I32(HEIGHT as i32),
                    Val::I32(quality),
                    Val::I32(0),
                ],
            )
            .unwrap_or_else(|error| panic!("encoding at quality {quality}: {error:#}"));
        runtime.refuel();

        // The return is a pointer to a two-word pair, and the code does not say
        // which word is which; take the one that addresses a JPEG.
        let pair = as_i32(&out).expect("pair pointer") as u32;
        let (first, second) = (
            runtime.read_u32_at(pair).expect("word 0"),
            runtime.read_u32_at(pair + 4).expect("word 1"),
        );
        let (ptr, len) = match runtime.read(first, 2).as_deref() {
            Ok([0xff, 0xd8]) => (first, second),
            _ => (second, first),
        };
        runtime.read(ptr, len).expect("encoded bytes")
    };

    let low = encode(&mut runtime, 50);
    let high = encode(&mut runtime, 90);

    assert_eq!(&low[..2], [0xff, 0xd8], "JPEG start-of-image marker");
    assert_eq!(&low[low.len() - 2..], [0xff, 0xd9], "end-of-image marker");
    assert_eq!(&low[..2], &high[..2]);

    // A quality knob that changes nothing was not the quality knob.
    assert!(
        high.len() > low.len(),
        "quality 90 should encode to more bytes than quality 50, got {} vs {}",
        high.len(),
        low.len()
    );

    // Deterministic, which is what makes it usable as an oracle.
    assert_eq!(low, encode(&mut runtime, 50));
}
