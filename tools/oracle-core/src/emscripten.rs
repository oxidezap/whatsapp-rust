//! A deterministic emscripten environment.
//!
//! Zero-returning stubs are enough to instantiate a module but not to run it:
//! the VoIP module busy-waits on `emscripten_get_now`, and spins forever when
//! the clock never advances. Everything here exists because leaving it stubbed
//! produces a hang, a silent wrong answer, or an unusable trace.
//!
//! Every source of non-determinism is replaced by a reproducible one — a
//! virtual clock that advances per call, and a seeded PRNG — so two runs of the
//! same input produce byte-identical output. That is the property the oracle is
//! for.

use anyhow::{Result, anyhow};
use wasmtime::error::Context as _;
use wasmtime::{Caller, Func, Linker, Module, Ref, Store, Val, ValType};

use crate::IntoWasmtime as _;
use crate::state::{HostState, ThreadPolicy};

/// Fixed wall-clock origin: 2021-01-01T00:00:00Z in milliseconds. Arbitrary but
/// stable, so traces and any timestamp the module derives stay comparable
/// across runs.
pub const EPOCH_MS: f64 = 1_609_459_200_000.0;

/// `EAGAIN`: the errno for "no resources to create another thread".
const EAGAIN: i32 = 11;

/// Defines the emscripten runtime imports that need real behaviour.
pub fn define(store: &mut Store<HostState>, linker: &mut Linker<HostState>) -> Result<()> {
    let _ = store;

    // --- time: a virtual monotonic clock

    linker.func_wrap(
        "env",
        "emscripten_get_now",
        |caller: Caller<'_, HostState>| -> f64 {
            let state = caller.data();
            state.record("env", "emscripten_get_now", Vec::new());
            state.tick_clock()
        },
    )?;

    linker.func_wrap(
        "env",
        "emscripten_date_now",
        // Wall time, not the monotonic counter: see `shared::WALL_STEP_MS`.
        |caller: Caller<'_, HostState>| -> f64 {
            EPOCH_MS + caller.data().shared.tick_wall_clock()
        },
    )?;

    linker.func_wrap("env", "_emscripten_get_now_is_monotonic", || -> i32 { 1 })?;

    // --- randomness: seeded, so a run is reproducible

    linker.func_wrap(
        "env",
        "getentropy",
        |mut caller: Caller<'_, HostState>, buf: i32, len: i32| -> i32 {
            crate::state::sync_memory(&mut caller);
            match fill_random(&mut caller, buf as u32, len as u32) {
                Ok(()) => 0,
                Err(_) => 28, // EINVAL
            }
        },
    )?;

    // **Length first, buffer second** — the opposite of `getentropy` above, and
    // the single most expensive mistake in this host's history.
    //
    // This is not the standard emscripten import, it is WhatsApp's own, and its
    // declared type `(i32, i32)` says nothing about which is which. Written by
    // analogy with `getentropy(buf, len)`, it was backwards, and the bytecode
    // says so plainly — from the only caller, the crypto callback in table slot
    // 298 that `generate_raw_e2e_keys` dispatches through:
    //
    // ```
    //     i32.const 32     ; the length: this callback rejects any other
    //     local.get 0      ; the destination
    //     call 8           ; env::get_random_bytes_js
    // ```
    //
    // With the arguments swapped the host read the length as the destination
    // and the destination as the length, so a request for 32 bytes at
    // `0xf00000` became **fifteen megabytes of PRNG output written from address
    // 32**. That is the "ring full of key material" this repository has been
    // chasing: it was key material, and the host was writing it.
    //
    // It also explains why the failure looked like a coin flip with an exact
    // discriminator. `HostState::write` refuses an out-of-bounds range, so the
    // bogus write only lands when `32 + destination` still fits inside the
    // memory — which is why a corrupt round always ended with the heap at
    // `0x10e0000` and a healthy one at `0xf10000`. The heap size was not
    // correlated with the corruption, it *decided* it.
    linker.func_wrap(
        "env",
        "get_random_bytes_js",
        |mut caller: Caller<'_, HostState>, len: i32, buf: i32| {
            crate::state::sync_memory(&mut caller);
            let _ = fill_random(&mut caller, buf as u32, len as u32);
        },
    )?;

    // The engine handing the host a stanza to put on the wire.
    //
    // Implemented rather than stubbed for one reason: **the bytes only exist
    // during this call.** The trampoline that reaches this import — function
    // #855, at table slot 464 — frees all three pointers the moment it returns,
    // and the allocator hands the memory straight back out, so a caller reading
    // the recorded arguments afterwards gets whatever landed there next. This
    // is the only place the stanza can be copied.
    //
    // `record` is called by hand because defining the import removes the
    // automatic stub that would otherwise have done it, and the counters are
    // what `hot_calls` reports.
    linker.func_wrap(
        "env",
        "sendSignalingXMPP_js_sync",
        |mut caller: Caller<'_, HostState>, peer: i32, call_id: i32, stanza: i32, len: i32| {
            crate::state::sync_memory(&mut caller);
            let state = caller.data();
            state.shared.record(
                "env",
                "sendSignalingXMPP_js_sync",
                vec![
                    i64::from(peer),
                    i64::from(call_id),
                    i64::from(stanza),
                    i64::from(len),
                ],
            );
            // A length the guest gives is not to be trusted onto a read: a
            // negative or absurd one would be a panic here rather than a
            // diagnosis. Out-of-range reads come back empty, and an empty
            // stanza in the record is itself the finding.
            let bytes = u32::try_from(len)
                .ok()
                .and_then(|len| state.read(stanza as u32, len).ok())
                .unwrap_or_default();
            state.shared.record_signaling(crate::shared::SignalingCall {
                peer_jid: state.read_cstr(peer as u32).unwrap_or_default(),
                call_id: state.read_cstr(call_id as u32).unwrap_or_default(),
                stanza: bytes,
            });
        },
    )?;

    // --- threading
    //
    // The core count steers how many workers the module's pools want. It is
    // reported as one under the single-threaded policies so those pools stay
    // empty, and as a small fixed number when threads really run — fixed rather
    // than the host's actual count, so a run does not depend on the machine.
    linker.func_wrap(
        "env",
        "emscripten_num_logical_cores",
        |caller: Caller<'_, HostState>| -> i32 {
            match caller.data().threads {
                ThreadPolicy::Spawn => 4,
                _ => 1,
            }
        },
    )?;

    // --- diagnostics: keep the message instead of discarding the pointer

    linker.func_wrap(
        "env",
        "emscripten_console_error",
        |mut caller: Caller<'_, HostState>, ptr: i32| {
            crate::state::sync_memory(&mut caller);
            let state = caller.data();
            let message = state.read_cstr(ptr as u32).unwrap_or_default();
            state.log(format!("console.error: {message}"));
        },
    )?;

    linker.func_wrap(
        "env",
        "loggingCallback_js_sync",
        |mut caller: Caller<'_, HostState>, ptr: i32, len: i32| {
            crate::state::sync_memory(&mut caller);
            let text = read_sized(&caller, ptr as u32, len as u32);
            let state = caller.data();
            state.record(
                "env",
                "loggingCallback_js_sync",
                vec![ptr as i64, len as i64],
            );
            state.log(text);
        },
    )?;

    linker.func_wrap(
        "env",
        "__assert_fail",
        |caller: Caller<'_, HostState>,
         condition: i32,
         file: i32,
         line: i32,
         function: i32|
         -> Result<(), wasmtime::Error> {
            let state = caller.data();
            let condition = state.read_cstr(condition as u32).unwrap_or_default();
            let file = state.read_cstr(file as u32).unwrap_or_default();
            let function = state.read_cstr(function as u32).unwrap_or_default();
            Err(wasmtime::Error::msg(format!(
                "assertion failed: {condition} at {file}:{line} in {function}"
            )))
        },
    )?;

    linker.func_wrap(
        "env",
        "abort",
        |caller: Caller<'_, HostState>| -> Result<(), wasmtime::Error> {
            caller.data().log("module called abort()");
            Err(wasmtime::Error::msg("module called abort()"))
        },
    )?;

    // Growing the heap. Stubbed, this reports failure for every request, and
    // the guest is left with whatever it started with — 160 pages, 10 MiB, for
    // the VoIP engine. Every allocation past that fails, and the failure
    // surfaces a long way from here: musl's `mmap` serves an anonymous mapping
    // from `emscripten_builtin_memalign` rather than from `_mmap_js`, so a
    // refusal becomes `ENOMEM`, then `map_pages_internal: mmap failed`, then
    // `abort()` inside whatever the engine happened to be doing. Starting a
    // call was one of those.
    //
    // The declared maximum is the ceiling, and the guest asks in bytes.
    linker.func_wrap(
        "env",
        "emscripten_resize_heap",
        |mut caller: Caller<'_, HostState>, requested: i32| -> i32 {
            /// wasm's page size.
            const PAGE: u64 = 65_536;

            let Some(memory) = caller.data().memory.clone() else {
                caller
                    .data()
                    .log("emscripten_resize_heap: module has no shared memory");
                return 0;
            };

            // A negative request is a size past 2 GiB, which this cannot serve.
            let Ok(requested) = u64::try_from(requested) else {
                return 0;
            };
            let wanted = requested.div_ceil(PAGE);
            let current = memory.size();
            if wanted <= current {
                return 1;
            }

            match memory.grow(wanted - current) {
                Ok(_) => {
                    crate::state::sync_memory(&mut caller);
                    1
                }
                Err(error) => {
                    caller.data().log(format!(
                        "emscripten_resize_heap: {current} -> {wanted} pages refused: {error}"
                    ));
                    0
                }
            }
        },
    )?;

    // Waking a thread to look at its mailbox.
    //
    // This is how one thread hands work to another: it queues the call and then
    // posts the target a `checkMailbox` message. There are no messages here, so
    // the notification is recorded and acted on when the target next waits —
    // and with the notification stubbed out, the queue filled up and nothing
    // ever drained it. `sendSignalingXMPP_js_sync` is reached through that
    // queue, which is why it never fired.
    linker.func_wrap(
        "env",
        "_emscripten_notify_mailbox_postmessage",
        |mut caller: Caller<'_, HostState>, target: i32, current: i32, _mem: i32| {
            let target = target as u64;
            caller.data().shared.notify_mailbox(target);

            // Emscripten posts itself a message rather than recursing; with no
            // event loop the equivalent is to look now.
            if target == current as u64 {
                check_mailbox(&mut caller);
            }
        },
    )?;

    // The other half: a thread waiting on its mailbox.
    //
    // Emscripten blocks here until something posts. Blocking is what this host
    // must not do — see `HostState::register_main_thread` — so a pending
    // notification is consumed instead, which is the same thing minus the wait.
    //
    // The reference drains unconditionally: its `checkMailbox` re-registers the
    // wait and calls `_emscripten_check_mailbox()` every time, letting the
    // notification decide only when to wake. Doing that here was measured and is
    // worse — one run in three dies at 26 engine-log lines against 167 — because
    // that pattern leans on an event loop and on `canBlock=0` to guarantee the
    // dispatch cannot block, and neither holds on this side.
    linker.func_wrap(
        "env",
        "_emscripten_thread_mailbox_await",
        |mut caller: Caller<'_, HostState>, _thread_ptr: i32| {
            let thread = caller.data().thread_id;
            if caller.data().shared.take_mailbox(thread) {
                check_mailbox(&mut caller);
            }
        },
    )?;

    // Anonymous mmap, which the guest's own page allocator runs on.
    //
    // WhatsApp's `boxedalloc` asks for pages through `mmap`, and PJSIP's pool
    // allocator is built on top of it: `pj_pool_create_block` →
    // `pj_pool_alloc_no_trace` → `map_pages_internal`. Stubbed, the allocation
    // fails, `map_pages_internal` logs `mmap failed, size - %zu` and calls
    // `abort()` — and the trap surfaces several frames away, in whatever the
    // engine was doing at the time. Starting a call was one of those.
    //
    // Emscripten's `mmapAlloc` is the model: round the length up to a page,
    // take it from `emscripten_builtin_memalign`, and zero it, because
    // anonymous mmap is defined to return zeroed pages.
    linker.func_wrap(
        "env",
        "_mmap_js",
        |mut caller: Caller<'_, HostState>,
         len: i32,
         _prot: i32,
         _flags: i32,
         _fd: i32,
         _offset: i64,
         allocated: i32,
         addr: i32|
         -> Result<i32, wasmtime::Error> {
            /// wasm's page size, and the alignment emscripten uses here.
            const PAGE: i32 = 65_536;
            /// `ENOMEM`, the errno musl's mmap expects for a refusal.
            const ENOMEM: i32 = -48;

            crate::state::sync_memory(&mut caller);

            let size = (len + PAGE - 1) & !(PAGE - 1);
            let memalign = crate::exports::func(
                &mut caller,
                &["emscripten_builtin_memalign", "memalign", "aligned_alloc"],
            )
            .into_wasmtime()?;

            let mut out = vec![Val::I32(0)];
            memalign.call(&mut caller, &[Val::I32(PAGE), Val::I32(size)], &mut out)?;
            crate::state::sync_memory(&mut caller);

            let Some(Val::I32(ptr)) = out.first() else {
                return Ok(ENOMEM);
            };
            if *ptr == 0 {
                caller
                    .data()
                    .log(format!("_mmap_js: no memory for {size} bytes"));
                return Ok(ENOMEM);
            }

            // Anonymous pages are zero-filled; the guest relies on it.
            let state = caller.data();
            state
                .write(*ptr as u32, &vec![0u8; size as usize])
                .into_wasmtime()?;
            state
                .write(addr as u32, &(*ptr as u32).to_le_bytes())
                .into_wasmtime()?;
            state
                .write(allocated as u32, &1u32.to_le_bytes())
                .into_wasmtime()?;
            Ok(0)
        },
    )?;

    // The matching release, which does *not* free anything.
    //
    // Emscripten's `_munmap_js` only syncs a file-backed mapping; musl frees
    // the memory itself, and it frees an anonymous mapping without coming
    // through here at all. Freeing here as well hands the allocator a pointer
    // it already owns — and a heap corrupted that way surfaces later, as some
    // unrelated destructor trapping in `free`.
    linker.func_wrap(
        "env",
        "_munmap_js",
        |_caller: Caller<'_, HostState>,
         _addr: i32,
         _len: i32,
         _prot: i32,
         _flags: i32,
         _fd: i32,
         _offset: i64|
         -> i32 { 0 },
    )?;

    // How a worker thread gets something run on the main thread.
    //
    // Emscripten proxies such a call by queueing it and having the main thread
    // dispatch it here, through `__indirect_function_table`. Stubbed, the
    // dispatch simply does not happen, so implementing it is necessary for any
    // work the guest hands to the main thread.
    //
    // The claim that used to sit here — that outbound VoIP signaling goes
    // through this queue, from slot 436, and that a stub is why an offer is
    // "never sent" — is wrong in all three parts. The sender's trampoline is
    // at slot **464**, not 436 (`examples/table_slot_of.rs`); this import is
    // called zero times during an origination; and the offer *is* delivered
    // regardless. See `examples/outbound_setup_matrix.rs`.
    //
    // Arguments, read off the call site (`oracle abi JgwtTQVeWPm --index
    // 12155`): the table index, two words this does not need, and a pointer to
    // the argument block. The block holds one f64 per argument, which is how
    // emscripten passes them regardless of the callee's own types.
    linker.func_wrap(
        "env",
        "emscripten_receive_on_main_thread_js",
        |mut caller: Caller<'_, HostState>,
         index: i32,
         _em_asm: i32,
         _count: i32,
         args: i32|
         -> Result<f64, wasmtime::Error> {
            crate::state::sync_memory(&mut caller);
            let callee = table_entry(&mut caller, index as u64).into_wasmtime()?;

            // The callee's own signature decides how many arguments to read and
            // what to widen them to. Trusting the count argument instead would
            // mean reading a length the caller and callee could disagree on.
            let ty = callee.ty(&caller);
            let params: Vec<ValType> = ty.params().collect();
            let results: Vec<ValType> = ty.results().collect();

            let mut call_args = Vec::with_capacity(params.len());
            for (position, param) in params.iter().enumerate() {
                let at = args as u32 + (position * 8) as u32;
                let bytes = caller
                    .data()
                    .read(at, 8)
                    .map_err(|error| anyhow!("reading proxied argument {position}: {error}"))
                    .into_wasmtime()?;
                let raw = f64::from_le_bytes(
                    bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| anyhow!("short read for proxied argument {position}"))
                        .into_wasmtime()?,
                );
                call_args.push(match param {
                    ValType::I32 => Val::I32(raw as i32),
                    ValType::I64 => Val::I64(raw as i64),
                    ValType::F32 => Val::F32((raw as f32).to_bits()),
                    ValType::F64 => Val::F64(raw.to_bits()),
                    other => {
                        return Err(wasmtime::Error::msg(format!(
                            "proxied call takes an unsupported {other:?}"
                        )));
                    }
                });
            }

            // One slot per result, each of the callee's own type. `Val::I32`
            // throughout was wrong for any callee returning `i64`, `f32` or
            // `f64`: wasmtime validates the result buffer against the signature
            // and refuses the call, so the conversion below never got the
            // chance to do what it says it does.
            let mut out: Vec<Val> = results
                .iter()
                .cloned()
                .map(crate::host::zero_value_of)
                .collect();
            callee.call(&mut caller, &call_args, &mut out)?;

            // The result comes back as a double whatever the callee returned,
            // matching how emscripten types this import.
            Ok(match out.first() {
                Some(Val::I32(value)) => *value as f64,
                Some(Val::I64(value)) => *value as f64,
                Some(Val::F32(bits)) => f32::from_bits(*bits) as f64,
                Some(Val::F64(bits)) => f64::from_bits(*bits),
                _ => 0.0,
            })
        },
    )?;

    // How a thread request is answered depends on the policy; see `ThreadPolicy`
    // for why none of the three answers is simply "correct".
    for symbol in ["__pthread_create_js", "pthread_create"] {
        linker.func_wrap(
            "env",
            symbol,
            |caller: Caller<'_, HostState>,
             thread_ptr: i32,
             _attr: i32,
             routine: i32,
             arg: i32|
             -> i32 {
                let state = caller.data();
                let (code, note) = match state.threads {
                    ThreadPolicy::Refuse => (EAGAIN, "refused, host is single-threaded"),
                    ThreadPolicy::PretendSuccess => (0, "reported as started, but never runs"),
                    ThreadPolicy::Spawn => match state.spawner.as_ref() {
                        // Arguments follow emscripten's __pthread_create_js:
                        // (pthread_ptr, attr, start_routine, arg).
                        Some(spawner) => {
                            let code = spawner.spawn(thread_ptr as u32, routine as u32, arg as u32);
                            (code, "started")
                        }
                        None => (EAGAIN, "no spawner on this instance"),
                    },
                };
                caller.data().log(format!("pthread_create: {note}"));
                code
            },
        )?;
    }

    // Emscripten's glue initialises the *main* thread through this callback:
    //
    //   __emscripten_thread_init(tb, /*is_main_browser*/1, /*is_main_runtime*/1,
    //                            /*can_block*/1, 0, 0)
    //
    // Leaving it stubbed leaves the runtime believing no thread is the main
    // one. Nothing fails immediately, but any code that then coordinates with a
    // worker spins forever waiting for a main thread that never identifies
    // itself — which is only visible once threads actually run.
    // EM_ASM: inline JavaScript compiled into the module. The first argument
    // points at the snippet's source in a data segment, so the host can read
    // what is being asked of it instead of guessing.
    //
    // Returning a fixed zero is not neutral: libsodium polls one of these and
    // spins forever on a falsy answer — 161 million calls in one `blind`. The
    // snippet text is what says which answer is right.
    // The two variants differ only in return type, and the declared type has to
    // match: linking a module that imports the `_double` form against an
    // i32-returning host function fails outright.
    linker.func_wrap(
        "env",
        "emscripten_asm_const_int",
        |mut caller: Caller<'_, HostState>, code: i32, _sig: i32, _args: i32| -> i32 {
            answer_asm_const(&mut caller, "emscripten_asm_const_int", code)
        },
    )?;
    linker.func_wrap(
        "env",
        "emscripten_asm_const_double",
        |mut caller: Caller<'_, HostState>, code: i32, _sig: i32, _args: i32| -> f64 {
            answer_asm_const(&mut caller, "emscripten_asm_const_double", code) as f64
        },
    )?;

    linker.func_wrap(
        "env",
        "__emscripten_init_main_thread_js",
        |mut caller: Caller<'_, HostState>, thread_ptr: i32| {
            // Emscripten has shipped this export under both spellings; a
            // module built with one is not obliged to offer the other. Trying
            // only `__emscripten_thread_init` silently did nothing on a module
            // that exports `_emscripten_thread_init`, which leaves the main
            // thread unregistered — and `emscripten_main_thread_process_queued_calls`
            // then asserts `emscripten_is_main_runtime_thread()` and traps, so
            // nothing a worker queues for the main thread ever runs.
            // Both spellings ship. Asking for only one is how the main thread
            // came to never register itself; `exports.rs` has the full story.
            // Both spellings are tried only when main-thread registration is
            // asked for; see `HostState::register_main_thread` for why it is
            // not the default.
            let names: &[&str] = if caller.data().register_main_thread {
                &["__emscripten_thread_init", "_emscripten_thread_init"]
            } else {
                &["__emscripten_thread_init"]
            };
            let Ok(init) = crate::exports::func(&mut caller, names) else {
                return;
            };
            let arity = init.ty(&caller).params().len();
            let args = [
                Val::I32(thread_ptr),
                // (thread_ptr, is_main, is_runtime, can_block, stack, profiling)
                Val::I32(1),
                Val::I32(1),
                // `can_block = 0`, the value emscripten passes on the web
                // (`canBlock: !ENVIRONMENT_IS_WEB`). It decides which futex a
                // wait uses, and that decides whether this host can make
                // progress at all:
                //
                // With 1, the main thread takes `memory.atomic.wait32` — a wait
                // that happens *inside* wasm, with no host call. It therefore
                // holds its scheduler turn while blocked, and the thread that
                // would notify it never gets one. Deadlock, until the turn
                // times out five seconds later.
                //
                // With 0, it takes emscripten's own busy-wait
                // (`futex_wait_main_browser_thread`), which calls
                // `_emscripten_yield` each time round — draining the proxying
                // queue and, because that is a host call, yielding the turn.
                // Progress on both counts, and it is what a browser does.
                Val::I32(0),
                Val::I32(0),
                Val::I32(0),
            ];
            let outcome = init.call(&mut caller, &args[..arity.min(args.len())], &mut []);
            if let Err(error) = outcome {
                caller
                    .data()
                    .log(format!("__emscripten_init_main_thread_js: {error}"));
            }

            // Then thread-local storage, which the main thread needs as much as
            // a worker does. WhatsApp Web's own sequence is `_emscripten_thread_init`,
            // `_emscripten_tls_init`, `__wasm_call_ctors`,
            // `_embind_initialize_bindings`; only the first was being done here,
            // and a main thread with uninitialised TLS reads a wild pointer the
            // moment anything looks one up.
            if let Some(wasmtime::Extern::Func(tls)) = caller.get_export("_emscripten_tls_init") {
                let results = tls.ty(&caller).results().len();
                let mut out = vec![Val::I32(0); results];
                if let Err(error) = tls.call(&mut caller, &[], &mut out) {
                    caller
                        .data()
                        .log(format!("_emscripten_tls_init (main): {error}"));
                }
            }
        },
    )?;

    linker.func_wrap(
        "env",
        "exit",
        |caller: Caller<'_, HostState>, code: i32| -> Result<(), wasmtime::Error> {
            let state = caller.data();
            state.set_exit_code(code);
            state.log(format!("module called exit({code})"));
            Err(wasmtime::Error::msg(format!("module called exit({code})")))
        },
    )?;

    Ok(())
}

/// Defines a trampoline for every `invoke_*` import.
///
/// Emscripten routes calls that may throw through these: the first argument is
/// an index into `__indirect_function_table` and the rest are the callee's own
/// arguments. Stubbing them out silently skips real work — the module's static
/// initialisers, including every embind registration, run through here — so
/// they are dispatched for real. A trap in the callee is reported to the module
/// through `setThrew`, which is how emscripten unwinds a C++ exception.
pub fn define_invokes(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
) -> Result<usize> {
    let mut defined = 0;

    // A minified module has no `invoke_` prefix left; the shape analysis found
    // those, and the union covers both kinds.
    let by_shape = store.data().shared.invoke_imports.get().cloned();

    for import in module.imports() {
        let is_invoke = import.name().starts_with("invoke_")
            || by_shape
                .as_ref()
                .is_some_and(|set| set.contains(import.name()));
        if !is_invoke {
            continue;
        }
        let wasmtime::ExternType::Func(ty) = import.ty() else {
            continue;
        };

        let name = import.name().to_owned();
        let result_types: Vec<ValType> = ty.results().collect();

        let func =
            crate::host::host_func(&mut *store, ty.clone(), move |caller, params, results| {
                let (index, args) = params
                    .split_first()
                    .ok_or_else(|| anyhow!("{name} called with no table index"))
                    .into_wasmtime()?;
                let index = match index {
                    Val::I32(index) => *index as u64,
                    other => {
                        return Err(wasmtime::Error::msg(format!(
                            "{name} index is not i32: {other:?}"
                        )));
                    }
                };

                let callee = table_entry(caller, index).into_wasmtime()?;
                let args = args.to_vec();

                match callee.call(&mut *caller, &args, results) {
                    Ok(()) => Ok(()),
                    Err(error) => {
                        // Emscripten's trampoline reports a *C++* exception
                        // through `setThrew` and rethrows anything else —
                        // `if (e !== e+0) throw e`. This flags both alike.
                        //
                        // Making the same cut was tried and reverted. It is
                        // faithful, and it does surface a real fault: with host
                        // traps propagated, an offer into the VoIP engine fails
                        // in `free` rather than looping in `std::terminate`. But
                        // the engine then reaches different states run to run,
                        // and `offer_handling_is_deterministic` fails two runs
                        // in three. That test is sensitive enough that even
                        // lengthening this log line reorders the threads, so the
                        // faithful version needs the unwind handled at the top
                        // of `Runtime::call` — not a rethrow from here.
                        set_threw(caller);
                        for (slot, ty) in results.iter_mut().zip(&result_types) {
                            *slot = zero_of(ty.clone());
                        }
                        // The root cause, not the top of the chain. A wasmtime
                        // trap prints as "error while executing at wasm
                        // backtrace:" followed by thousands of frames, and the
                        // one thing worth reading — "out of bounds memory
                        // access", "unreachable" — is the last link. Logging
                        // only the head made every failure here look alike.
                        let cause = error
                            .chain()
                            .last()
                            .map_or_else(|| error.to_string(), ToString::to_string);
                        caller.data().log(format!("caught in {name}: {cause}"));
                        Ok(())
                    }
                }
            });

        linker
            .define(&*store, import.module(), import.name(), func)
            .with_context(|| format!("defining {}", import.name()))?;
        defined += 1;
    }

    Ok(defined)
}

/// Resolves a function-table index to a callable function.
fn table_entry(caller: &mut Caller<'_, HostState>, index: u64) -> Result<Func> {
    // Minified modules export the table under whatever letter the optimiser
    // picked, so the conventional name is a fallback rather than the lookup.
    let name = caller
        .data()
        .shared
        .table_export
        .get()
        .cloned()
        .unwrap_or_else(|| "__indirect_function_table".to_owned());

    let table = crate::exports::table(caller, &[&name, "__indirect_function_table"])?;

    match table.get(&mut *caller, index) {
        Some(Ref::Func(Some(func))) => Ok(func),
        Some(Ref::Func(None)) => Err(anyhow!("function table entry {index} is null")),
        Some(other) => Err(anyhow!("table entry {index} is not a function: {other:?}")),
        None => Err(anyhow!("function table index {index} is out of bounds")),
    }
}

/// Tells the module that the call it made through a trampoline threw.
fn set_threw(caller: &mut Caller<'_, HostState>) {
    let Some(func) = crate::exports::optional_func(caller, &["setThrew", "_setThrew"]) else {
        return;
    };
    let _ = func.call(caller, &[Val::I32(1), Val::I32(0)], &mut []);
}

fn zero_of(ty: ValType) -> Val {
    match ty {
        ValType::I32 => Val::I32(0),
        ValType::I64 => Val::I64(0),
        ValType::F32 => Val::F32(0),
        ValType::F64 => Val::F64(0),
        ValType::V128 => Val::V128(0u128.into()),
        ValType::Ref(ty) => Val::null_ref(ty.heap_type()),
    }
}

fn read_sized(caller: &Caller<'_, HostState>, ptr: u32, len: u32) -> String {
    match caller.data().read(ptr, len) {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(_) => String::new(),
    }
}

/// Reads the EM_ASM snippet a call points at and answers it.
///
/// Shared by both variants so the two differ only where they have to: in the
/// return type the module declared.
fn answer_asm_const(caller: &mut Caller<'_, HostState>, symbol: &str, code: i32) -> i32 {
    crate::state::sync_memory(caller);
    let state = caller.data();
    let snippet = state.read_cstr(code as u32).unwrap_or_default();
    state.record("env", symbol, vec![code as i64]);
    answer_em_asm(&snippet)
}

/// What to answer an EM_ASM snippet.
///
/// The snippets these modules use are environment probes — "is there a crypto
/// API", "how many cores" — and the only wrong answer is one that makes the
/// caller retry. Anything unrecognised gets a truthy answer for that reason,
/// and is logged so an unhandled snippet is visible rather than silent.
fn answer_em_asm(snippet: &str) -> i32 {
    // A capability check: claiming the feature exists stops the polling.
    if snippet.contains("crypto") || snippet.contains("getRandomValues") {
        return 1;
    }
    if snippet.contains("navigator") || snippet.contains("hardwareConcurrency") {
        return 1;
    }
    // Default truthy: a zero is what the spin loops are waiting on.
    1
}

/// Fills guest memory with bytes from the deterministic PRNG.
fn fill_random(caller: &mut Caller<'_, HostState>, ptr: u32, len: u32) -> Result<()> {
    // Recorded, because this is the host's only source of high-entropy bytes
    // and the corruption in `examples/ring_corruption.rs` looks exactly like
    // them. An earlier pass excluded this by instrumenting writes of 64 KiB or
    // more — which says nothing about the same total arriving four kilobytes at
    // a time.
    caller
        .data()
        .record("env", "fill_random", vec![ptr as i64, len as i64]);

    let mut bytes = Vec::with_capacity(len as usize);
    {
        let state = caller.data();
        for _ in 0..len {
            bytes.push(state.next_random_byte());
        }
    }
    caller.data().write(ptr, &bytes)
}

/// Runs the guest's mailbox handler, which dispatches whatever was queued.
fn check_mailbox(caller: &mut Caller<'_, HostState>) {
    let Ok(check) = crate::exports::func(
        caller,
        &["__emscripten_check_mailbox", "_emscripten_check_mailbox"],
    ) else {
        return;
    };
    if let Err(error) = check.call(&mut *caller, &[], &mut []) {
        // Best-effort: the mailbox is drained opportunistically, and a failure
        // here belongs to the guest's handler rather than to the notification.
        caller.data().log(format!("check_mailbox failed: {error}"));
    }
}

/// Defines the time imports, deriving each from the signature the module
/// declares.
///
/// `_localtime_js` has shipped as both `(i64, i32) -> i32` and `(i64, i32)`
/// across emscripten versions, so a fixed `func_wrap` links against one and
/// fails outright on the other.
pub fn define_time(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
) -> Result<usize> {
    let mut defined = 0;

    for import in module.imports() {
        let wasmtime::ExternType::Func(ty) = import.ty() else {
            continue;
        };

        let func = match import.name() {
            // Broken-down local time. Stubbed, every field of `struct tm` reads
            // zero — year 1900, January 0th — and the engine formats a
            // timestamp with it 21 times during a single call. UTC, because
            // determinism is the point and the virtual clock has no timezone.
            "_localtime_js" => crate::host::host_func(
                &mut *store,
                ty.clone(),
                |caller: &mut Caller<'_, HostState>, params, results| {
                    crate::state::sync_memory(caller);
                    let time = match params.first() {
                        Some(Val::I64(value)) => *value,
                        Some(Val::I32(value)) => i64::from(*value),
                        _ => 0,
                    };
                    let tm = match params.get(1) {
                        Some(Val::I32(ptr)) => *ptr as u32,
                        _ => 0,
                    };

                    let failed = match BrokenDownTime::from_unix(time) {
                        Some(broken) => broken.write(caller.data(), tm).is_err(),
                        None => true,
                    };
                    // Only the newer signature reports failure; on the older
                    // one there is nowhere to put it.
                    if let Some(slot) = results.first_mut() {
                        *slot = Val::I32(i32::from(failed));
                    }
                    Ok(())
                },
            ),

            // The timezone globals: seconds west of UTC, whether DST ever
            // applies, and the two names `%Z` formats to.
            "_tzset_js" => crate::host::host_func(
                &mut *store,
                ty.clone(),
                |caller: &mut Caller<'_, HostState>, params, _results| {
                    crate::state::sync_memory(caller);
                    let at = |index: usize| match params.get(index) {
                        Some(Val::I32(ptr)) => *ptr as u32,
                        _ => 0,
                    };
                    let state = caller.data();
                    state.write(at(0), &0i32.to_le_bytes()).into_wasmtime()?;
                    state.write(at(1), &0i32.to_le_bytes()).into_wasmtime()?;
                    state.write(at(2), b"UTC\0").into_wasmtime()?;
                    state.write(at(3), b"UTC\0").into_wasmtime()?;
                    Ok(())
                },
            ),

            _ => continue,
        };

        linker
            .define(&*store, import.module(), import.name(), func)
            .with_context(|| format!("defining {}", import.name()))?;
        defined += 1;
    }

    Ok(defined)
}

/// A `struct tm`, as musl lays it out for wasm32.
///
/// Built here rather than taken from a date library: the fields have to be
/// written at the offsets the guest reads, and a dependency would not know
/// them. UTC only — the virtual clock has no timezone, and inventing one would
/// make the oracle's output depend on where it ran.
struct BrokenDownTime {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
}

impl BrokenDownTime {
    /// Splits a Unix timestamp into calendar fields.
    ///
    /// Howard Hinnant's `civil_from_days`, which is exact for the whole range
    /// and needs no table.
    fn from_unix(time: i64) -> Option<Self> {
        const SECONDS_PER_DAY: i64 = 86_400;

        let days = time.div_euclid(SECONDS_PER_DAY);
        let secs_of_day = time.rem_euclid(SECONDS_PER_DAY);

        // Shift the era so that March 1st starts the year, which is what makes
        // the leap day fall at the end and the arithmetic close.
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);

        // 1970-01-01 was a Thursday, and `days` counts from there.
        let wday = (days + 4).rem_euclid(7);

        let jan1 = Self::days_from_civil(year, 1, 1);
        let yday = days - jan1;

        Some(Self {
            sec: i32::try_from(secs_of_day % 60).ok()?,
            min: i32::try_from((secs_of_day / 60) % 60).ok()?,
            hour: i32::try_from(secs_of_day / 3600).ok()?,
            mday: i32::try_from(day).ok()?,
            mon: i32::try_from(month - 1).ok()?,
            year: i32::try_from(year - 1900).ok()?,
            wday: i32::try_from(wday).ok()?,
            yday: i32::try_from(yday).ok()?,
        })
    }

    /// The inverse: days since the epoch for a calendar date.
    fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
        let year = year - i64::from(month <= 2);
        let era = year.div_euclid(400);
        let yoe = year - era * 400;
        let mp = if month > 2 { month - 3 } else { month + 9 };
        let doy = (153 * mp + 2) / 5 + day - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// Writes the fields at the offsets musl's `struct tm` uses.
    fn write(&self, state: &HostState, ptr: u32) -> Result<()> {
        // tm_sec, tm_min, tm_hour, tm_mday, tm_mon, tm_year, tm_wday, tm_yday,
        // tm_isdst, tm_gmtoff, tm_zone — four bytes each on wasm32.
        let fields = [
            self.sec, self.min, self.hour, self.mday, self.mon, self.year, self.wday, self.yday,
            0, // tm_isdst: UTC never has it
            0, // tm_gmtoff
            0, // tm_zone: a null pointer rather than a dangling one
        ];
        for (index, value) in fields.iter().enumerate() {
            state.write(ptr + (index * 4) as u32, &value.to_le_bytes())?;
        }
        Ok(())
    }
}
