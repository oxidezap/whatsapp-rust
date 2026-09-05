//! Building the host environment a module runs in.
//!
//! The engine configuration, the linker carrying every host implementation, and
//! the stubs standing in for whatever is left. A spawned thread rebuilds all of
//! it, so this has to be reusable rather than inlined into startup.

use anyhow::Result;
use wasmtime::error::Context as _;
use wasmtime::{
    Caller, Config, Engine, Extern, ExternType, Func, FuncType, Global, Linker, Memory, Module,
    Ref, SharedMemory, Store, Table, Val, ValType,
};

use crate::state::{HostState, sync_memory};

/// How much a module may execute before being cut off, so a runaway startup
/// cannot hang a test run.
///
/// Generous on purpose: a guest worker spinning on a queue burns fuel while
/// waiting, and the whole budget can go to waiting rather than to work when the
/// machine is loaded. Too small shows up as a call that "failed" for no visible
/// reason.
pub const DEFAULT_FUEL: u64 = 20_000_000_000;

/// Defines every host implementation, then stubs whatever is left.
///
/// Order matters: `define_stubs` skips anything already defined, so the real
/// implementations have to come first.
pub(crate) fn define_hosts(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
    unstubbable: &mut Vec<String>,
) -> Result<()> {
    crate::embind::define(store, linker, module)?;
    crate::emval::define(store, linker, module)?;
    crate::emscripten::define(store, linker)?;
    crate::emscripten::define_time(store, linker, module)?;
    crate::cxa::define(store, linker, module)?;
    crate::wasi::define(store, linker, module)?;
    crate::emscripten::define_invokes(store, linker, module)?;
    define_stubs(store, linker, module, unstubbable)?;
    Ok(())
}

/// Builds a linker carrying the same host environment as the main instance.
///
/// A spawned thread instantiates the module again and must see exactly the same
/// imports; anything missing here would show up as a link error at thread start
/// rather than as a difference in behaviour.
pub fn build_linker(store: &mut Store<HostState>, module: &Module) -> Result<Linker<HostState>> {
    let mut linker = Linker::new(store.engine());
    linker.allow_shadowing(true);
    let mut unstubbable = Vec::new();
    define_hosts(store, &mut linker, module, &mut unstubbable)?;
    Ok(linker)
}

/// Builds a host function that refreshes the memory window before running.
///
/// Every host function must do this, and adding the call by hand at each
/// definition site is how it gets forgotten: WASI was defined without it and
/// silently read a stale window for as long as the guest did not grow memory —
/// which the media modules do during startup, so their arguments landed outside
/// the window the host believed in and were dropped.
pub fn host_func<F>(store: &mut Store<HostState>, ty: FuncType, handler: F) -> Func
where
    F: Fn(&mut Caller<'_, HostState>, &[Val], &mut [Val]) -> Result<(), wasmtime::Error>
        + Send
        + Sync
        + 'static,
{
    Func::new(store, ty, move |mut caller, params, results| {
        // Every host call is both a cancellation point and a yield point. A
        // guest worker loop reaches one constantly — it polls the clock — which
        // is what makes them the right place for each.
        let thread = caller.data().thread_id;
        if thread != 0 && caller.data().shared.is_shutting_down() {
            return Err(wasmtime::Error::msg("host is shutting down"));
        }

        // Keep a spawned thread fuelled while the host still wants it.
        //
        // A worker was given a fixed budget on the assumption that its loop
        // waits for work that never arrives. That is no longer true: the VoIP
        // engine's worker polls constantly and *does* get work, and the budget
        // ran out between bringing the stack up and placing a call — leaving
        // zero live threads at exactly the moment the engine needed one. What
        // bounds a thread is the shutdown check above, not an instruction
        // count, so the budget is topped up rather than spent down.
        if thread != 0 {
            const LOW: u64 = 100_000_000;
            const TOP_UP: u64 = 1_000_000_000;

            if caller.get_fuel().is_ok_and(|fuel| fuel < LOW) {
                let _ = caller.set_fuel(TOP_UP);
            }
        }

        caller.data().shared.scheduler.yield_point(thread);
        // Also checks the memory watch; see `sync_memory`.
        sync_memory(&mut caller);
        handler(&mut caller, params, results)
    })
}

/// Installs the memory watch on a store, so every crossing of the host boundary
/// checks it.
///
/// A `call_hook` rather than a check inside `host_func`, and the difference is
/// the whole reason this works. Host functions arrive by three routes —
/// `host_func`, the stubs, and the forty-odd `Linker::func_wrap` definitions in
/// `emscripten.rs` — and the hottest call a guest worker makes, the clock, takes
/// the third and touches neither of the first two. A watch checked only in
/// `host_func` stayed silent through a round that destroyed ten megabytes of
/// guest memory, and silence reads as "nothing wrote there" when it means
/// "nothing looked". This hook is the one place the VM guarantees every host
/// call passes through, however the function was defined.
///
/// Checking **both** directions is what lets it name a culprit rather than a
/// witness: a span that was intact when a thread entered wasm and is broken
/// when it comes back was broken by that thread's own guest code. Checking only
/// on the way in reports whoever happened to make the next host call — the
/// first catch that way was a media worker asleep in `pj_thread_sleep`, which
/// had written nothing.
///
/// That argument needs one guest thread at a time, which is *not* how this host
/// normally runs — see `Runtime::demand_strict_turns`, which an investigator
/// has to switch on *before* the operation under suspicion. Switching it on
/// when the watch breaks was tried and cannot work: attribution catches the
/// transition from intact to broken, and by the time anything has noticed, the
/// transition is over. Every sighting after it reads "already broken before
/// this thread ran", correctly and uselessly.
pub fn install_memory_watch(store: &mut Store<HostState>) {
    store.call_hook(|mut context, hook| {
        let intact = context.data().watch_intact();
        let strict = context.data().shared.strict_turns();
        let thread = context.data().thread_id;
        if hook.exiting_host() {
            if strict {
                context.data().shared.scheduler.acquire(thread);
            }
            context.data().shared.entered_wasm();
            // Only worth recording under strict turns; see the field's docs.
            context
                .data()
                .watch_intact_entering_wasm
                .set(if strict { intact } else { None });
        } else {
            context.data().shared.left_wasm();
            note_growth(&mut context);
            if intact == Some(false) {
                let entry = context.data().watch_intact_entering_wasm.get();
                report_broken_watch(&mut context, entry == Some(true), entry.is_some());
            }
            if strict {
                context.data().shared.scheduler.release(thread);
            }
        }
        Ok(())
    });
}

/// Records a change in guest memory size, with the guest stack behind it.
///
/// Cheap enough for every crossing: one atomic swap unless the size actually
/// moved. See `SharedHost::growths` for why the size is worth this much
/// attention.
fn note_growth(context: &mut wasmtime::StoreContextMut<'_, HostState>) {
    let Some(size) = context
        .data()
        .memory
        .as_ref()
        .map(|memory| memory.data().len())
    else {
        return;
    };
    let Some(previous) = context.data().shared.note_memory_size(size) else {
        return;
    };

    let thread = context.data().thread_id;
    let frames: Vec<String> = wasmtime::WasmBacktrace::capture(&*context)
        .frames()
        .iter()
        .map(|frame| match frame.func_name() {
            Some(name) => format!("{name} (f{})", frame.func_index()),
            None => format!("f{}", frame.func_index()),
        })
        .collect();

    context.data().shared.record_growth(format!(
        "{previous:#x} -> {size:#x} (+{:#x}) on thread {thread}: {}",
        size - previous,
        if frames.is_empty() {
            "<no guest frames>".to_owned()
        } else {
            frames.join(" <- ")
        }
    ));
}

/// Names the moment a watched span of guest memory stopped holding what it did.
///
/// The interesting part is the backtrace. Entry to host code happens *inside*
/// guest execution, so capturing here says which guest functions were on the
/// stack when the damage first became visible — the difference between "memory
/// was destroyed somewhere in `startVoipCall`" and a call chain to read.
///
/// Reported through the host log rather than returned as a trap: this is a
/// diagnostic about a fault the host did not cause, and failing the call would
/// replace the symptom under investigation with a different one.
fn report_broken_watch(
    context: &mut wasmtime::StoreContextMut<'_, HostState>,
    wrote_it: bool,
    sound: bool,
) {
    let Some(watch) = context.data().shared.watch.get() else {
        return;
    };
    watch
        .broken
        .store(true, std::sync::atomic::Ordering::SeqCst);

    let thread = context.data().thread_id;
    {
        // One sighting per thread. Taken before the expensive part so that a
        // thread already recorded pays nothing but a lock.
        let sightings = watch.sightings.lock().unwrap_or_else(|e| e.into_inner());
        // Keyed on the thread *and* on whether the reading is attributable: a
        // thread that reported before strict turns began must be allowed to
        // report again once its answer means something.
        if sightings.len() >= crate::shared::MAX_SIGHTINGS
            || sightings
                .iter()
                .any(|(id, was_sound, _)| *id == thread && *was_sound == sound)
        {
            return;
        }
    }
    let at = watch.at;
    let frames: Vec<String> = wasmtime::WasmBacktrace::capture(&*context)
        .frames()
        .iter()
        .map(|frame| match frame.func_name() {
            Some(name) => format!("{name} (f{})", frame.func_index()),
            None => format!("f{}", frame.func_index()),
        })
        .collect();

    // How much of memory has gone by the time anyone notices. A healthy image
    // is about 83% zero bytes; the wreck is about 3%. Sampling that here says
    // whether the damage arrived as one event or was still spreading — the
    // difference between looking for a single wild write and looking for a
    // loop.
    let (size, zeros) = sampled_zeroes(context.data());

    let report = format!(
        "watch {at:#x} broken, thread {thread} {}, \
         memory {size:#x} and {zeros}% zero bytes, guest stack: {}",
        match (wrote_it, sound) {
            (true, true) => "WROTE IT",
            (true, false) => "wrote it, unattributable (threads were still concurrent)",
            (false, _) => "only saw it (already broken before this thread ran)",
        },
        if frames.is_empty() {
            "<none>".to_owned()
        } else {
            frames.join(" <- ")
        }
    );
    watch
        .sightings
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((thread, sound, report.clone()));
    context.data().shared.log(thread, report);
}

/// Memory size, and the percentage of zero bytes in a strided sample of it.
///
/// Strided rather than exhaustive because this runs while the guest is stopped
/// mid-call: reading seventeen megabytes here would change what is being
/// measured. Every 64th byte is plenty to tell 83% from 3%.
#[allow(unsafe_code)]
fn sampled_zeroes(state: &HostState) -> (usize, u32) {
    const STRIDE: usize = 64;

    let Some(memory) = state.memory.as_ref() else {
        return (0, 0);
    };
    let data = memory.data();
    let mut seen = 0u64;
    let mut zero = 0u64;
    let mut at = 0;
    while at < data.len() {
        // SAFETY: as in `HostState::read` — one byte read through the cell, no
        // reference into shared memory formed, index bounded by the loop.
        if unsafe { *data[at].get() } == 0 {
            zero += 1;
        }
        seen += 1;
        at += STRIDE;
    }
    // `checked_div` rather than a guard on `seen`: clippy 1.97 reads the guarded
    // form as a manual check and rejects it, and the two say the same thing.
    let percent = (zero * 100)
        .checked_div(seen)
        .and_then(|share| u32::try_from(share).ok())
        .unwrap_or(0);
    (data.len(), percent)
}

pub(crate) fn build_engine() -> Result<Engine> {
    let mut config = Config::new();
    // Threads is the reason wasmtime is here at all: the VoIP module imports a
    // shared memory and will not load without it.
    config.wasm_threads(true);
    // wasmtime gates shared-memory creation behind a second switch, separate
    // from the threads proposal itself.
    config.shared_memory(true);
    config.wasm_simd(true);
    config.wasm_relaxed_simd(true);
    config.wasm_bulk_memory(true);
    config.wasm_multi_memory(true);
    config.wasm_tail_call(true);
    config.consume_fuel(true);
    // Epoch interruption is what makes shutdown reliable. A guest worker only
    // notices `request_shutdown` at a host call, and its fuel is topped up
    // rather than spent down, so a thread that computes for a long time between
    // host calls outlives the `Runtime` that spawned it — along with its
    // `Store` and the module's memory. Twenty-three sequential tests
    // accumulated eighteen gigabytes that way and the suite was killed by the
    // OOM killer. Bumping the epoch interrupts guest code from outside, with no
    // cooperation from the guest.
    config.epoch_interruption(true);

    // Compiled modules are cached on disk, keyed by their bytes and the
    // compiler settings. The captured modules never change, so after the first
    // run every instantiation skips Cranelift entirely — which matters because
    // the harness builds a fresh instance per test, and a second one per guest
    // thread.
    // A machine without a usable cache directory still runs, just slower.
    if let Ok(cache) = wasmtime::Cache::new(wasmtime::CacheConfig::new()) {
        config.cache(Some(cache));
    }
    // These modules are megabytes of code and the oracle compiles them on every
    // run. Optimising the generated code costs far more than it saves for a
    // test harness that runs each function a handful of times.
    config.cranelift_opt_level(wasmtime::OptLevel::None);
    Ok(Engine::new(&config).context("building wasmtime engine")?)
}

/// The memory this host created for a module that imports one.
///
/// Both variants have to reach `HostState`, and the reason is the same for
/// each: an import is not an export, so nothing later can look it up by name.
/// Dropping the ordinary one on the floor left the host with neither `memory`
/// nor `linear`, and every callback that dereferenced a guest pointer failed
/// with "module memory is not available to the host" on a module that had
/// instantiated perfectly.
#[derive(Debug, Clone, Default)]
pub(crate) struct ImportedMemory {
    /// A shared memory, readable without a store context.
    pub shared: Option<SharedMemory>,
    /// An ordinary one, which needs the store to give up its window.
    pub ordinary: Option<Memory>,
}

/// Creates and defines the imported memory, returning it for the host state.
pub(crate) fn define_memory(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
) -> Result<ImportedMemory> {
    for import in module.imports() {
        let ExternType::Memory(ty) = import.ty() else {
            continue;
        };

        if ty.is_shared() {
            let memory = SharedMemory::new(module.engine(), ty.clone())
                .context("allocating shared memory")?;
            linker
                .define(&*store, import.module(), import.name(), memory.clone())
                .context("defining shared memory import")?;
            return Ok(ImportedMemory {
                shared: Some(memory),
                ordinary: None,
            });
        }

        let memory = Memory::new(&mut *store, ty.clone()).context("allocating memory")?;
        linker
            .define(&*store, import.module(), import.name(), memory)
            .context("defining memory import")?;
        return Ok(ImportedMemory {
            shared: None,
            ordinary: Some(memory),
        });
    }
    Ok(ImportedMemory::default())
}

/// Defines a recording stub for every import the linker does not already have.
pub(crate) fn define_stubs(
    store: &mut Store<HostState>,
    linker: &mut Linker<HostState>,
    module: &Module,
    unstubbable: &mut Vec<String>,
) -> Result<()> {
    let mut stubbed = std::collections::BTreeSet::new();

    for import in module.imports() {
        let symbol = format!("{}::{}", import.module(), import.name());
        if linker.get_by_import(&mut *store, &import).is_some() {
            continue;
        }

        let external: Extern = match import.ty() {
            ExternType::Func(ty) => {
                let module_name = import.module().to_owned();
                let name = import.name().to_owned();
                let results: Vec<Val> = ty.results().map(zero_value_of).collect();

                stubbed.insert(symbol.clone());
                host_func(&mut *store, ty.clone(), move |caller, params, outputs| {
                    let args = params.iter().map(scalar_of).collect();
                    caller.data().record(&module_name, &name, args);
                    outputs.clone_from_slice(&results);
                    Ok(())
                })
                .into()
            }
            ExternType::Memory(_) => continue, // handled by define_memory
            ExternType::Table(ty) => {
                let Some(init) = zero_ref_of(ty.element()) else {
                    unstubbable.push(symbol);
                    continue;
                };
                Table::new(&mut *store, ty.clone(), init)
                    .context("allocating stub table")?
                    .into()
            }
            ExternType::Global(ty) => {
                let init = zero_value_of(ty.content().clone());
                Global::new(&mut *store, ty.clone(), init)
                    .context("allocating stub global")?
                    .into()
            }
            _ => {
                unstubbable.push(symbol);
                continue;
            }
        };

        linker
            .define(&*store, import.module(), import.name(), external)
            .with_context(|| format!("defining stub for {symbol}"))?;
    }

    store.data().shared.stubbed.set(stubbed).ok();
    Ok(())
}

/// Widens any numeric wasm value to `i64` so a trace can hold it uniformly.
/// Pointers, lengths and handles are all i32 in these modules, which is what
/// makes this lossless in practice.
fn scalar_of(value: &Val) -> i64 {
    match value {
        Val::I32(value) => *value as i64,
        Val::I64(value) => *value,
        Val::F32(bits) => f32::from_bits(*bits) as i64,
        Val::F64(bits) => f64::from_bits(*bits) as i64,
        _ => 0,
    }
}

/// The zero of a value type, which is what an unimplemented import answers.
#[must_use]
pub fn zero_value_of(ty: ValType) -> Val {
    match ty {
        ValType::I32 => Val::I32(0),
        ValType::I64 => Val::I64(0),
        ValType::F32 => Val::F32(0),
        ValType::F64 => Val::F64(0),
        ValType::V128 => Val::V128(0u128.into()),
        ValType::Ref(ty) => Val::null_ref(ty.heap_type()),
    }
}

/// The null reference of a reference type, used to initialise a stub table.
/// Only the two hierarchies a table can hold in these modules are supported.
fn zero_ref_of(ty: &wasmtime::RefType) -> Option<Ref> {
    let heap = ty.heap_type();
    if heap.is_func() {
        Some(Ref::Func(None))
    } else if heap.is_extern() {
        Some(Ref::Extern(None))
    } else {
        None
    }
}
