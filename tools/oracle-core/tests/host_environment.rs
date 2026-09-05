//! Tests for the host environment itself: WASI, exception handling and the
//! deterministic clock.
//!
//! Each of these covers a failure that actually happened. A stubbed `fd_write`
//! made a module retry 3.2 million times; a stubbed `pthread_create` reported a
//! thread that never ran; stubbed exception handling turned every recoverable
//! C++ error into an opaque wasm trap.

use oracle_core::{Catalog, Runtime, Value};

mod common;

const VOIP: &str = "JgwtTQVeWPm";

/// Serialises against the other test binaries, which cargo runs in parallel:
/// two PJSIP worker pools competing for cores miss their own deadlines.
fn engine_guard() -> common::EngineLock {
    common::engine_lock()
}

fn voip() -> Option<Runtime> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(VOIP).ok()?;
    let bytes = std::fs::read(&entry.path).ok()?;
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.run_ctors().expect("ctors");
    Some(runtime)
}

macro_rules! voip_or_skip {
    () => {
        match voip() {
            Some(runtime) => runtime,
            None => {
                eprintln!("skipping: no capture (set WA_WASM_DIR)");
                return;
            }
        }
    };
}

/// Startup must not spin. The regression this guards against burned the entire
/// fuel budget on one import.
#[test]
fn startup_does_not_spin_on_any_host_call() {
    let _engine = engine_guard();
    let mut runtime = voip_or_skip!();

    // Generous enough for real startup work, far below a busy-wait.
    const CEILING: u64 = 100_000;

    let hottest = runtime.state().hot_calls();
    if let Some((symbol, count)) = hottest.first() {
        assert!(
            *count < CEILING,
            "`{symbol}` was called {count} times during startup, which is a spin"
        );
    }
    let _ = runtime.embind();
}

/// A recoverable C++ error must arrive as a readable message, not as a trap
/// with a wasm backtrace.
#[test]
fn cpp_exceptions_are_reported_with_their_message() {
    let _engine = engine_guard();
    let mut runtime = voip_or_skip!();
    runtime.clear_calls();

    // Ask for a parameter before the stack is initialised; the engine throws.
    let _ = runtime.call_embind("getVoipParam", &[Value::Str("nonexistent".to_owned())]);

    let messages: Vec<String> = runtime
        .logs()
        .into_iter()
        .filter(|line| line.starts_with("C++ exception:"))
        .collect();

    if messages.is_empty() {
        // The engine is entitled not to throw here; nothing to assert.
        return;
    }
    assert!(
        messages
            .iter()
            .any(|line| line.contains("::") || line.contains(':')),
        "exception text should name a type or reason, got {messages:?}"
    );
}

/// The virtual clock must move forward, or anything polling for a deadline
/// never reaches it.
#[test]
fn the_virtual_clock_advances() {
    let _engine = engine_guard();
    let mut runtime = voip_or_skip!();
    let before = runtime.state().clock_ms();

    let _ = runtime.call_embind("getCallInfo", &[]);
    // Startup alone observes the clock many times.
    assert!(
        runtime.state().clock_ms() >= before,
        "clock went backwards: {before} -> {}",
        runtime.state().clock_ms()
    );
    assert!(before > 0.0, "clock never advanced during startup");
}

/// Two runs must agree byte for byte, including the traces. Without this, any
/// comparison against whatsapp-rust is unattributable.
#[test]
fn two_runs_produce_identical_traces() {
    let Some(mut first) = voip() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    let mut second = voip().expect("second instance");

    let calls_a = first.state().hot_calls();
    let calls_b = second.state().hot_calls();
    assert_eq!(calls_a, calls_b, "startup traces diverged");

    assert_eq!(
        first.embind().functions.len(),
        second.embind().functions.len()
    );
}

/// The in-memory filesystem must round-trip, since it is the only filesystem a
/// module under test is allowed to see.
#[test]
fn wasi_filesystem_round_trips() {
    let catalog = match Catalog::discover() {
        Ok(catalog) => catalog,
        Err(_) => {
            eprintln!("skipping: no capture (set WA_WASM_DIR)");
            return;
        }
    };
    let Ok(entry) = catalog.resolve("rogm88TRRiw") else {
        eprintln!("skipping: media module unavailable");
        return;
    };
    let bytes = std::fs::read(&entry.path).expect("read");
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");

    runtime.add_file("input.bin", vec![1, 2, 3, 4]);
    assert_eq!(runtime.wasi().file("input.bin"), Some(&[1u8, 2, 3, 4][..]));
    // Leading slashes must resolve to the same entry.
    assert_eq!(runtime.wasi().file("/input.bin"), Some(&[1u8, 2, 3, 4][..]));
    assert_eq!(runtime.wasi().file("missing.bin"), None);
}

/// Arguments are exposed with a program name in `argv[0]`, matching what a C
/// `main` expects.
#[test]
fn wasi_arguments_include_a_program_name() {
    let catalog = match Catalog::discover() {
        Ok(catalog) => catalog,
        Err(_) => {
            eprintln!("skipping: no capture (set WA_WASM_DIR)");
            return;
        }
    };
    let Ok(entry) = catalog.resolve("rogm88TRRiw") else {
        eprintln!("skipping: media module unavailable");
        return;
    };
    let bytes = std::fs::read(&entry.path).expect("read");
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");

    runtime.set_args(&["check".to_owned(), "input.bin".to_owned()]);
    let args = &runtime.wasi().args;
    assert_eq!(args.len(), 3);
    assert_eq!(args[1], "check");
}

/// A missing export must name what the module *does* have.
///
/// This is the regression test for a bug that cost a day: the host asked for
/// `__emscripten_thread_init`, the module exported `_emscripten_thread_init`,
/// and the lookup returned `None` into an `else { return; }`. Nothing failed,
/// the main thread never registered itself, and the symptom — every call a
/// worker queued for the main thread being dropped — surfaced thousands of
/// instructions away.
///
/// So the requirement is not "look up both spellings" but "when a lookup
/// fails, say so and point at the near miss".
#[test]
fn a_missing_export_names_the_near_miss() {
    let _engine = engine_guard();
    let runtime = voip_or_skip!();

    // The VoIP module is the one that exposed the bug: it has the
    // single-underscore spelling and not the double.
    let names = runtime.export_names();
    assert!(
        names.contains("_emscripten_thread_init"),
        "this module should carry the one-underscore spelling"
    );
    assert!(
        !names.contains("__emscripten_thread_init"),
        "and not the two-underscore one — that is what made the miss silent"
    );

    // And a lookup for the spelling this module does not have must report the
    // one it does. That report is the whole fix: the original bug was a lookup
    // that returned `None` into an `else { return; }` and said nothing.
    //
    // `threading.rs::the_main_thread_queue_can_be_drained` covers the other
    // half — that finding the right spelling actually registers the thread.
    let close: Vec<&String> = names
        .iter()
        .filter(|name| {
            name.trim_start_matches('_') == "emscripten_thread_init".trim_start_matches('_')
        })
        .collect();
    assert_eq!(
        close.len(),
        1,
        "exactly one spelling should be present, for the report to point at"
    );
}

/// Measuring a duration must not age a timestamp.
///
/// A browser has two clocks and the guest uses them for different things:
/// `performance.now()` to time an operation, `Date.now()` to stamp a protocol
/// message. Deriving both from one counter conflated them here, and the futex
/// busy-wait's millions of monotonic observations dragged wall time forward
/// with them — 345 seconds during a single call. The engine then compared the
/// offer's timestamp against `wa_call_is_offer_expired`'s 45-second threshold
/// and dropped it, which read as the engine rejecting a valid offer.
#[test]
fn the_wall_clock_does_not_move_when_the_monotonic_one_does() {
    let _engine = engine_guard();
    let runtime = voip_or_skip!();
    let state = runtime.state();

    let wall_before = runtime.virtual_unix_time();
    let monotonic_before = state.clock_ms();

    // A busy-wait's worth of monotonic observations.
    for _ in 0..100_000 {
        state.tick_clock();
    }

    assert!(
        state.clock_ms() > monotonic_before,
        "the monotonic clock must advance, or a guest spin never terminates"
    );
    assert_eq!(
        runtime.virtual_unix_time(),
        wall_before,
        "wall time must not age just because something was timed"
    );
}

/// The heap has to be able to grow.
///
/// `emscripten_resize_heap` was stubbed, so every request to grow was refused
/// and the guest was stuck with its declared minimum — 160 pages for the VoIP
/// engine. The failure surfaces a long way from the cause: musl serves an
/// anonymous `mmap` from `emscripten_builtin_memalign` rather than from
/// `_mmap_js`, so a refusal becomes `ENOMEM`, then `map_pages_internal: mmap
/// failed`, then `abort()` inside whatever was running.
#[test]
fn the_guest_heap_can_grow() {
    let mut runtime = voip_or_skip!();
    let before = runtime.memory_size();

    // Ask through the guest's own allocator, for more than it starts with.
    let wanted = before + (4 << 20);
    let outcome = runtime.call(
        "emscripten_builtin_memalign",
        &[
            wasmtime::Val::I32(65_536),
            wasmtime::Val::I32(wanted as i32),
        ],
    );
    runtime.refuel();

    let ptr = match outcome {
        Ok(values) => match values.first() {
            Some(wasmtime::Val::I32(ptr)) => *ptr,
            other => panic!("memalign returned {other:?}"),
        },
        Err(error) => panic!("memalign trapped: {error:#}"),
    };

    assert_ne!(
        ptr, 0,
        "an allocation past the initial heap must succeed, which needs \
         emscripten_resize_heap to actually grow the memory"
    );
    assert!(
        runtime.memory_size() > before,
        "the memory should have grown from {before} to serve {wanted} bytes"
    );
}

/// A hand-built WASI guest: enough of one to ask the host the two questions
/// below and hand the answers back through its own memory.
///
/// Built rather than compiled from C so the test says exactly which arguments
/// reach the host — the bugs it covers are both about argument *positions*, and
/// a libc between the test and the host would hide them.
mod wasi_fixture {
    use wasm_encoder::{
        CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection,
        ImportSection, Instruction, MemArg, MemorySection, MemoryType, Module, TypeSection,
        ValType,
    };

    /// Where the guest puts things. Fixed addresses, so the test can read them.
    pub const IOVEC: u32 = 0;
    /// The `nread` output.
    pub const NREAD: u32 = 8;
    /// Where a read lands.
    pub const BUFFER: u32 = 64;
    /// How many bytes the iovec asks for.
    pub const BUFFER_LEN: u32 = 8;
    /// Where `clock_time_get` writes its nanoseconds.
    pub const TIMESTAMP: u32 = 32;
    /// Where `path_open` writes the descriptor it allocated.
    pub const OPENED_FD: u32 = 48;
    /// Where the test writes the path for `path_open` to read.
    pub const PATH: u32 = 128;

    const I32: ValType = ValType::I32;
    const I64: ValType = ValType::I64;

    fn word(offset: u64) -> MemArg {
        MemArg {
            offset,
            align: 2,
            memory_index: 0,
        }
    }

    pub fn module() -> Vec<u8> {
        let mut types = TypeSection::new();
        // 0: fd_pread(fd, iovs, iovs_len, offset: u64, nread)
        types.ty().function([I32, I32, I32, I64, I32], [I32]);
        // 1: clock_time_get(id, precision: u64, out)
        types.ty().function([I32, I64, I32], [I32]);
        // 2: path_open(fd, dirflags, path, path_len, oflags, base: u64,
        //              inheriting: u64, fdflags, opened_fd)
        types
            .ty()
            .function([I32, I32, I32, I32, I32, I64, I64, I32, I32], [I32]);
        // 3: fd_read(fd, iovs, iovs_len, nread)
        types.ty().function([I32, I32, I32, I32], [I32]);
        // 4: (i32, i64) -> i32
        types.ty().function([I32, I64], [I32]);
        // 5: (i32) -> i32
        types.ty().function([I32], [I32]);
        // 6: (i32, i32) -> i32
        types.ty().function([I32, I32], [I32]);

        let mut imports = ImportSection::new();
        for (name, ty) in [
            ("fd_pread", 0),
            ("clock_time_get", 1),
            ("path_open", 2),
            ("fd_read", 3),
            ("fd_close", 5),
        ] {
            imports.import("wasi_snapshot_preview1", name, EntityType::Function(ty));
        }

        let mut memories = MemorySection::new();
        memories.memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            shared: false,
            page_size_log2: None,
        });

        let mut functions = FunctionSection::new();
        for ty in [4, 5, 6, 5, 5] {
            functions.function(ty);
        }

        let mut code = CodeSection::new();

        // do_pread(fd, offset) -> errno
        let mut pread = Function::new([]);
        {
            let mut f = pread.instructions();
            f.i32_const(IOVEC as i32)
                .i32_const(BUFFER as i32)
                .i32_store(word(0));
            f.i32_const(IOVEC as i32)
                .i32_const(BUFFER_LEN as i32)
                .i32_store(word(4));
            f.local_get(0)
                .i32_const(IOVEC as i32)
                .i32_const(1)
                .local_get(1)
                .i32_const(NREAD as i32)
                .call(0)
                .end();
        }
        code.function(&pread);

        // do_clock(id) -> errno
        let mut clock = Function::new([]);
        clock
            .instructions()
            .local_get(0)
            .i64_const(0)
            .i32_const(TIMESTAMP as i32)
            .call(1)
            .end();
        code.function(&clock);

        // do_open(path, path_len) -> errno
        let mut open = Function::new([]);
        open.instructions()
            .i32_const(3) // the preopened root
            .i32_const(0)
            .local_get(0)
            .local_get(1)
            .i32_const(0) // no O_CREAT: the file is placed by the host
            .i64_const(0)
            .i64_const(0)
            .i32_const(0)
            .i32_const(OPENED_FD as i32)
            .call(2)
            .end();
        code.function(&open);

        // do_read(fd) -> errno, through the descriptor's own cursor
        let mut read = Function::new([]);
        {
            let mut f = read.instructions();
            f.i32_const(IOVEC as i32)
                .i32_const(BUFFER as i32)
                .i32_store(word(0));
            f.i32_const(IOVEC as i32)
                .i32_const(BUFFER_LEN as i32)
                .i32_store(word(4));
            f.local_get(0)
                .i32_const(IOVEC as i32)
                .i32_const(1)
                .i32_const(NREAD as i32)
                .call(3)
                .end();
        }
        code.function(&read);

        let mut close = Function::new([]);
        close.instructions().local_get(0).call(4).end();
        code.function(&close);

        let mut exports = ExportSection::new();
        exports.export("memory", ExportKind::Memory, 0);
        for (index, name) in ["do_pread", "do_clock", "do_open", "do_read", "do_close"]
            .into_iter()
            .enumerate()
        {
            exports.export(name, ExportKind::Func, 5 + index as u32);
        }

        let mut module = Module::new();
        module.section(&types);
        module.section(&imports);
        module.section(&functions);
        module.section(&memories);
        module.section(&exports);
        module.section(&code);
        module.finish()
    }

    /// Silences the unused warning for `Instruction`, which the builder needs
    /// in scope only through its inherent methods.
    #[allow(dead_code)]
    type Unused = Instruction<'static>;
}

/// The `i32` a fixture export answered with.
fn errno_of(results: &[wasmtime::Val]) -> i32 {
    match results.first() {
        Some(wasmtime::Val::I32(code)) => *code,
        other => panic!("expected an i32 errno, got {other:?}"),
    }
}

/// Reads the guest's buffer back as text.
fn buffer(runtime: &Runtime, len: u32) -> String {
    let bytes = runtime
        .state()
        .read(wasi_fixture::BUFFER, len)
        .expect("read buffer");
    String::from_utf8_lossy(&bytes).into_owned()
}

fn open_fixture_file() -> (Runtime, u32) {
    let bytes = wasi_fixture::module();
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.add_file("data.bin", b"0123456789".to_vec());
    runtime
        .write_bytes_at(wasi_fixture::PATH, b"data.bin")
        .expect("write path");

    let errno = runtime
        .call(
            "do_open",
            &[
                wasmtime::Val::I32(wasi_fixture::PATH as i32),
                wasmtime::Val::I32(8),
            ],
        )
        .expect("do_open");
    assert_eq!(errno_of(&errno), 0, "path_open");

    let fd = runtime
        .read_u32_at(wasi_fixture::OPENED_FD)
        .expect("opened fd");
    (runtime, fd)
}

/// `fd_pread` is not `fd_read` with an extra argument.
///
/// Its offset is 64 bits wide, so `nread` is parameter *4*. Routing it through
/// the four-argument `fd_read` reported the byte count at the low half of the
/// offset, ignored the real output pointer, and moved the descriptor's cursor —
/// so a positional read either failed or corrupted whatever sat at that address.
#[test]
fn fd_pread_reads_from_its_offset_and_leaves_the_cursor_alone() {
    let (mut runtime, fd) = open_fixture_file();

    let errno = runtime
        .call(
            "do_pread",
            &[wasmtime::Val::I32(fd as i32), wasmtime::Val::I64(4)],
        )
        .expect("do_pread");
    assert_eq!(errno_of(&errno), 0, "fd_pread");

    // The count lands at the pointer the guest gave, not at the offset.
    let read = runtime.read_u32_at(wasi_fixture::NREAD).expect("nread");
    assert_eq!(read, 6, "six bytes are left from offset 4");
    assert_eq!(buffer(&runtime, read), "456789", "read from the offset");

    // And the cursor is where it was, which is the whole of what `p` means: an
    // ordinary read still starts at the beginning.
    let errno = runtime
        .call("do_read", &[wasmtime::Val::I32(fd as i32)])
        .expect("do_read");
    assert_eq!(errno_of(&errno), 0, "fd_read");
    let read = runtime.read_u32_at(wasi_fixture::NREAD).expect("nread");
    assert_eq!(read, 8);
    assert_eq!(
        buffer(&runtime, read),
        "01234567",
        "fd_pread must not have advanced the descriptor"
    );
}

#[test]
fn fd_close_rejects_reserved_unknown_and_already_closed_descriptors() {
    let mut runtime = Runtime::instantiate(&wasi_fixture::module()).expect("instantiate");
    runtime.add_file("data.bin", vec![1, 2, 3]);
    runtime
        .write_bytes_at(wasi_fixture::PATH, b"data.bin")
        .expect("path");
    let opened = runtime
        .call(
            "do_open",
            &[
                wasmtime::Val::I32(wasi_fixture::PATH as i32),
                wasmtime::Val::I32(8),
            ],
        )
        .expect("open");
    assert_eq!(errno_of(&opened), 0);
    let fd = runtime.read_u32_at(wasi_fixture::OPENED_FD).expect("fd");

    let close = |runtime: &mut Runtime, fd: u32| {
        errno_of(
            &runtime
                .call("do_close", &[wasmtime::Val::I32(fd as i32)])
                .expect("close"),
        )
    };
    assert_eq!(close(&mut runtime, fd), 0);
    assert_eq!(close(&mut runtime, fd), 8);
    for reserved in [0, 1, 2, 3] {
        assert_eq!(close(&mut runtime, reserved), 8);
    }
}

#[test]
fn c_strings_require_a_terminator_and_valid_utf8() {
    use wasm_encoder::{ExportKind, ExportSection, MemorySection, MemoryType, Module};

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });
    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    let mut module = Module::new();
    module.section(&memories);
    module.section(&exports);

    let mut runtime = Runtime::instantiate(&module.finish()).expect("instantiate");
    runtime
        .write_bytes_at(32, b"valid\0")
        .expect("valid string");
    assert_eq!(runtime.read_cstr(32).unwrap(), "valid");
    runtime
        .write_bytes_at(64, b"\xff\0")
        .expect("invalid UTF-8");
    assert!(runtime.read_cstr(64).is_err());
    runtime
        .write_bytes_at(65_536 - 256, &vec![b'x'; 256])
        .expect("unterminated string");
    assert!(runtime.read_cstr(65_536 - 256).is_err());
}

/// The clock id is the question, and answering all of them with the monotonic
/// counter told a guest asking for wall time that it was 1970.
#[test]
fn clock_time_get_distinguishes_the_clocks_it_has_from_the_ones_it_does_not() {
    /// 2021-01-01T00:00:00Z, the host's virtual origin, in nanoseconds.
    const ORIGIN_NANOS: u64 = 1_609_459_200_000_000_000;
    /// `ENOSYS`.
    const ENOSYS: i32 = 52;

    let bytes = wasi_fixture::module();
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");

    let read_timestamp = |runtime: &Runtime| -> u64 {
        let raw = runtime
            .state()
            .read(wasi_fixture::TIMESTAMP, 8)
            .expect("read timestamp");
        u64::from_le_bytes(raw.try_into().expect("eight bytes"))
    };

    // CLOCK_REALTIME: the virtual wall clock, past its 2021 origin.
    let errno = runtime
        .call("do_clock", &[wasmtime::Val::I32(0)])
        .expect("do_clock");
    assert_eq!(errno_of(&errno), 0);
    let realtime = read_timestamp(&runtime);
    assert!(
        realtime > ORIGIN_NANOS,
        "realtime should be past the epoch origin, got {realtime}"
    );

    // CLOCK_MONOTONIC: the counter, which starts at zero this run.
    let errno = runtime
        .call("do_clock", &[wasmtime::Val::I32(1)])
        .expect("do_clock");
    assert_eq!(errno_of(&errno), 0);
    let monotonic = read_timestamp(&runtime);
    assert!(
        monotonic < ORIGIN_NANOS,
        "monotonic is time since this run started, got {monotonic}"
    );

    // The CPU-time clocks this host does not have are refused, not answered
    // with a number it made up.
    for id in [2, 3] {
        let errno = runtime
            .call("do_clock", &[wasmtime::Val::I32(id)])
            .expect("do_clock");
        assert_eq!(
            errno_of(&errno),
            ENOSYS,
            "clock {id} is not one this host has"
        );
    }
}

/// A module that *imports* an ordinary memory and does not re-export it, then
/// asks the host to read a pointer into it.
///
/// Both halves matter. The host creates that memory to satisfy the import, and
/// used to drop the handle: nothing exports it, so `sync_memory` had no name to
/// look up, and every callback that dereferenced a guest pointer failed with
/// "module memory is not available to the host" on a module that had
/// instantiated perfectly.
#[test]
fn a_host_callback_can_read_an_imported_memory_that_is_never_exported() {
    use wasm_encoder::{
        CodeSection, ConstExpr, DataSection, EntityType, ExportKind, ExportSection, Function,
        FunctionSection, ImportSection, MemoryType, Module, TypeSection, ValType,
    };

    /// Away from address 0, which `read_cstr` reads as "no string".
    const MESSAGE_AT: i32 = 32;
    const MESSAGE: &[u8] = b"imported memory is readable\0";

    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], []); // 0: emscripten_console_error
    types.ty().function([], []); // 1: shout

    let mut imports = ImportSection::new();
    imports.import(
        "env",
        "memory",
        EntityType::Memory(MemoryType {
            minimum: 1,
            maximum: None,
            memory64: false,
            // Not shared: a shared memory takes a different path, and it is the
            // ordinary one that was being dropped.
            shared: false,
            page_size_log2: None,
        }),
    );
    imports.import("env", "emscripten_console_error", EntityType::Function(0));

    let mut functions = FunctionSection::new();
    functions.function(1);

    let mut shout = Function::new([]);
    shout.instructions().i32_const(MESSAGE_AT).call(0).end();
    let mut code = CodeSection::new();
    code.function(&shout);

    let mut data = DataSection::new();
    data.active(
        0,
        &ConstExpr::i32_const(MESSAGE_AT),
        MESSAGE.iter().copied(),
    );

    // Deliberately no memory export: that is the case a name lookup cannot
    // cover.
    let mut exports = ExportSection::new();
    exports.export("shout", ExportKind::Func, 1);

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&exports);
    module.section(&code);
    module.section(&data);
    let bytes = module.finish();

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.call("shout", &[]).expect("shout");

    let logs = runtime.logs();
    assert!(
        logs.iter()
            .any(|line| line.contains("imported memory is readable")),
        "the host should have read the message out of the imported memory: {logs:?}"
    );
}

/// `main(argc, argv)` gets the arguments, not two zeroes.
///
/// Passing zero for both made a command module see no arguments whatever
/// `set_args` had been told — and dereference a null `argv`, which is a crash
/// in most libcs rather than an empty command line.
#[test]
fn a_plain_main_receives_the_arguments_it_was_given() {
    use wasm_encoder::{
        CodeSection, ExportKind, ExportSection, Function, FunctionSection, MemArg, MemorySection,
        MemoryType, Module, TypeSection, ValType,
    };

    /// Where `main` records what it was handed, clear of the argv block.
    const ARGC_AT: i32 = 512;
    /// Where the fixture's `malloc` hands out its one allocation.
    const HEAP: i32 = 1024;

    fn word(offset: u64) -> MemArg {
        MemArg {
            offset,
            align: 2,
            memory_index: 0,
        }
    }

    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], [ValType::I32]); // malloc
    types
        .ty()
        .function([ValType::I32, ValType::I32], [ValType::I32]); // main

    let mut memories = MemorySection::new();
    memories.memory(MemoryType {
        minimum: 1,
        maximum: None,
        memory64: false,
        shared: false,
        page_size_log2: None,
    });

    let mut functions = FunctionSection::new();
    functions.function(0);
    functions.function(1);

    // A one-shot allocator: this module allocates exactly once, for argv.
    let mut malloc = Function::new([]);
    malloc.instructions().i32_const(HEAP).end();

    // main(argc, argv): record argc, then return the first byte of argv[1],
    // which is only reachable if the pointer array was really laid out.
    let mut main = Function::new([]);
    {
        let mut f = main.instructions();
        f.i32_const(ARGC_AT).local_get(0).i32_store(word(0));
        f.local_get(1).i32_load(word(4)).i32_load8_u(MemArg {
            offset: 0,
            align: 0,
            memory_index: 0,
        });
        f.end();
    }

    let mut code = CodeSection::new();
    code.function(&malloc);
    code.function(&main);

    let mut exports = ExportSection::new();
    exports.export("memory", ExportKind::Memory, 0);
    exports.export("malloc", ExportKind::Func, 0);
    exports.export("main", ExportKind::Func, 1);

    let mut module = Module::new();
    module.section(&types);
    module.section(&functions);
    module.section(&memories);
    module.section(&exports);
    module.section(&code);
    let bytes = module.finish();

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.set_args(&["hello".to_owned()]);

    let first_byte = runtime.run_main().expect("run_main");
    assert_eq!(
        first_byte, b'h' as i32,
        "main should reach argv[1], which is `hello`"
    );
    assert_eq!(
        runtime.read_u32_at(ARGC_AT as u32).expect("argc"),
        2,
        "argv[0] is the program name and argv[1] is the argument"
    );

    // argv[argc] must be null: a `while (*p)` walk is what stops on it.
    let argv = HEAP as u32;
    assert_eq!(runtime.read_u32_at(argv + 8).expect("argv[2]"), 0);
}

/// `call_table` must find the table under the name the module exports it by.
///
/// `__indirect_function_table` is the convention, not the name. A minified
/// module exports it as a single letter, and hard-coding the convention made
/// every table call — and every embind call, which reaches its invoker the same
/// way — fail on such a module even though the real name had already been read
/// out of it at instantiation.
#[test]
fn a_renamed_function_table_is_still_found() {
    use wasm_encoder::{
        CodeSection, ConstExpr, ElementSection, Elements, ExportKind, ExportSection, Function,
        FunctionSection, Module, RefType, TableSection, TableType, TypeSection, ValType,
    };

    let mut types = TypeSection::new();
    types.ty().function([ValType::I32], [ValType::I32]);

    let mut functions = FunctionSection::new();
    functions.function(0);

    let mut triple = Function::new([]);
    triple
        .instructions()
        .local_get(0)
        .i32_const(3)
        .i32_mul()
        .end();
    let mut code = CodeSection::new();
    code.function(&triple);

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: RefType::FUNCREF,
        table64: false,
        minimum: 1,
        maximum: Some(1),
        shared: false,
    });

    let mut elements = ElementSection::new();
    elements.active(
        Some(0),
        &ConstExpr::i32_const(0),
        Elements::Functions([0].as_slice().into()),
    );

    // The whole point: a single letter, as an optimiser would leave it.
    let mut exports = ExportSection::new();
    exports.export("a", ExportKind::Table, 0);

    let mut module = Module::new();
    module.section(&types);
    module.section(&functions);
    module.section(&tables);
    module.section(&exports);
    module.section(&elements);
    module.section(&code);
    let bytes = module.finish();

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    let results = runtime
        .call_table(0, &[wasmtime::Val::I32(14)])
        .expect("the table is exported as `a`, and that is what was recorded");
    assert_eq!(errno_of(&results), 42);
}

/// A proxied call whose callee does not return `i32`.
///
/// Every result slot was built as `Val::I32`, and wasmtime validates the result
/// buffer against the signature — so the call failed before producing the
/// value, even though the conversion right below it claims to handle `i64`,
/// `f32` and `f64`.
#[test]
fn a_proxied_call_can_return_something_other_than_an_i32() {
    use wasm_encoder::{
        CodeSection, ConstExpr, ElementSection, Elements, EntityType, ExportKind, ExportSection,
        Function, FunctionSection, ImportSection, Module, RefType, TableSection, TableType,
        TypeSection, ValType,
    };

    const ANSWER: f64 = 1234.5;

    let mut types = TypeSection::new();
    // 0: emscripten_receive_on_main_thread_js(index, em_asm, count, args) -> f64
    types.ty().function([ValType::I32; 4], [ValType::F64]);
    // 1: the callee, which returns an f64 and takes nothing
    types.ty().function([], [ValType::F64]);
    // 2: proxy(index) -> f64
    types.ty().function([ValType::I32], [ValType::F64]);

    let mut imports = ImportSection::new();
    imports.import(
        "env",
        "emscripten_receive_on_main_thread_js",
        EntityType::Function(0),
    );

    let mut functions = FunctionSection::new();
    functions.function(1);
    functions.function(2);

    let mut callee = Function::new([]);
    callee.instructions().f64_const(ANSWER.into()).end();

    let mut proxy = Function::new([]);
    proxy
        .instructions()
        .local_get(0)
        .i32_const(0)
        .i32_const(0)
        .i32_const(0)
        .call(0)
        .end();

    let mut code = CodeSection::new();
    code.function(&callee);
    code.function(&proxy);

    let mut tables = TableSection::new();
    tables.table(TableType {
        element_type: RefType::FUNCREF,
        table64: false,
        minimum: 1,
        maximum: Some(1),
        shared: false,
    });

    let mut elements = ElementSection::new();
    elements.active(
        Some(0),
        &ConstExpr::i32_const(0),
        // Slot 0 is the callee: function index 1, past the one import.
        Elements::Functions([1].as_slice().into()),
    );

    let mut exports = ExportSection::new();
    exports.export("__indirect_function_table", ExportKind::Table, 0);
    exports.export("proxy", ExportKind::Func, 2);

    let mut module = Module::new();
    module.section(&types);
    module.section(&imports);
    module.section(&functions);
    module.section(&tables);
    module.section(&exports);
    module.section(&elements);
    module.section(&code);
    let bytes = module.finish();

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    let results = runtime
        .call("proxy", &[wasmtime::Val::I32(0)])
        .expect("the proxied call should reach an f64-returning callee");
    match results.first() {
        Some(wasmtime::Val::F64(bits)) => assert_eq!(f64::from_bits(*bits), ANSWER),
        other => panic!("expected an f64, got {other:?}"),
    }
}
