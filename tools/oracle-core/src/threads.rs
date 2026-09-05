//! Real wasm threads.
//!
//! A wasm thread is not a host thread running guest code: it is a *separate
//! instance* of the same module, on its own `Store`, sharing one linear memory.
//! That is what emscripten's Web Worker glue does, and doing anything else —
//! sharing a `Store` across threads, or running the entry point on the calling
//! thread — either fails to compile or deadlocks the moment the guest blocks.
//!
//! The function table is *not* shared. Each instance builds its own from the
//! module's element segments, so a function pointer means the same thing in
//! every instance because they were all initialised identically. Passing a table
//! index between threads is therefore sound; passing a `Func` would not be.

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Result, anyhow};
use wasmtime::error::Context as _;
use wasmtime::{Engine, Extern, Module, Ref, SharedMemory, Store, Val};

use crate::host::build_linker;
use crate::shared::SharedHost;
use crate::state::{HostState, ThreadPolicy};

/// How long a spawned thread may run before its fuel is exhausted.
///
/// A worker whose loop never terminates is normal — it is waiting for work that
/// will not arrive here — so this bounds it rather than hanging the process.
const THREAD_FUEL: u64 = 2_000_000_000;

/// Everything needed to bring up another instance of the running module.
pub struct Spawner {
    engine: Engine,
    module: Module,
    memory: SharedMemory,
    shared: Arc<SharedHost>,
    policy: ThreadPolicy,
}

impl std::fmt::Debug for Spawner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spawner")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

impl Spawner {
    /// Everything a new guest thread needs to instantiate the module again.
    ///
    /// The memory is shared; the `Store` and the function table are not.
    #[must_use]
    pub fn new(
        engine: Engine,
        module: Module,
        memory: SharedMemory,
        shared: Arc<SharedHost>,
        policy: ThreadPolicy,
    ) -> Self {
        Self {
            engine,
            module,
            memory,
            shared,
            policy,
        }
    }

    /// The policy new threads are started under.
    #[must_use]
    pub fn policy(&self) -> ThreadPolicy {
        self.policy
    }

    /// A copy carrying a different thread policy.
    pub fn with_policy(&self, policy: ThreadPolicy) -> Self {
        Self {
            engine: self.engine.clone(),
            module: self.module.clone(),
            memory: self.memory.clone(),
            shared: Arc::clone(&self.shared),
            policy,
        }
    }

    /// Starts a guest thread. Returns the errno the guest should see.
    ///
    /// `thread_ptr` is the guest's own pthread control block; the runtime needs
    /// it to bind thread-local storage in the new instance.
    pub fn spawn(&self, thread_ptr: u32, start_routine: u32, arg: u32) -> i32 {
        const EAGAIN: i32 = 11;

        let engine = self.engine.clone();
        let module = self.module.clone();
        let memory = self.memory.clone();
        let shared = Arc::clone(&self.shared);
        let policy = self.policy;
        let id = shared.allocate_thread_id();

        shared.thread_started();
        let launched = std::thread::Builder::new()
            .name(format!("wasm-thread-{id}"))
            .spawn(move || {
                let outcome = run_thread(
                    Context_ {
                        engine,
                        module,
                        memory,
                        shared: Arc::clone(&shared),
                        policy,
                        id,
                    },
                    thread_ptr,
                    start_routine,
                    arg,
                );
                if let Err(error) = outcome {
                    // A worker that ends on a trap is expected here — either
                    // the shutdown interrupt from `Runtime::drop` or a loop
                    // waiting for work that never arrives — so this is
                    // recorded, not propagated.
                    let why = if shared.is_shutting_down() {
                        "host shutting down".to_owned()
                    } else {
                        first_line(&error)
                    };
                    shared.log(id, format!("thread {id} ended: {why}"));
                }
                shared.thread_finished();
            });

        match launched {
            Ok(_handle) => 0,
            Err(error) => {
                self.shared
                    .log(0, format!("pthread_create: host refused: {error}"));
                self.shared.thread_finished();
                EAGAIN
            }
        }
    }
}

/// Offset of `stack` in emscripten's `struct pthread` — the *top* of the
/// thread's stack, since wasm stacks grow down. `stack_size` follows it at
/// `+56`.
///
/// Measured rather than assumed: every worker in this capture reports a
/// distinct value here and `0x10000` at `+56`, which is the 64 KiB the guest's
/// own `pthread_create` allocated for it. The worker waits for both fields and
/// installs those bounds before it can enter guest code.
///
/// Re-measured on the `JgwtTQVeWPm` capture, because `unwasm` reads the same
/// structure at `(stack, stack_size) = (48, 52)` — Emscripten's own
/// `C_STRUCTS.pthread` — and a four-byte disagreement here would put every
/// worker on a stack that is not its own, which is the shape of the fault this
/// host has. It does not: both captures answer `+48=0x12379`, which is no
/// address at all, and `+52`/`+56` hold a distinct top and a flat `0x10000`.
/// So this pair is right for these builds and the offsets are *not* the reason
/// installing per-thread bounds fails. Recorded because it looked like the
/// lead the note in `run_thread` asks for, and it is not.
const STACK_HIGH: u32 = 52;

/// `stack_size` follows it. Together they are what `establishStackSpace` reads.
const STACK_SIZE: u32 = 56;

/// Reads a guest `u32`, or `None` if the address is out of bounds.
///
/// The read is of memory another thread may be writing. That is the point —
/// this is how the host watches for `pthread_create` to finish filling in a
/// control block it does not own.
fn read_u32(store: &Store<HostState>, at: u32) -> Option<u32> {
    let state = store.data();
    // A racy read of shared memory, deliberately — and therefore through
    // `HostState::read`, whose per-byte relaxed atomics make the race defined.
    // The result is only ever treated as a hint that is then range checked, so
    // a torn value cannot become a wild pointer.
    let word = state.read(at, 4).ok()?;
    Some(u32::from_le_bytes([word[0], word[1], word[2], word[3]]))
}

/// Bundle passed into the new thread; keeps `run_thread`'s signature readable.
struct Context_ {
    engine: Engine,
    module: Module,
    memory: SharedMemory,
    shared: Arc<SharedHost>,
    policy: ThreadPolicy,
    id: u64,
}

/// Brings up an instance for one guest thread and runs its entry point.
fn run_thread(ctx: Context_, thread_ptr: u32, start_routine: u32, arg: u32) -> Result<()> {
    let mut state = HostState::for_thread(Arc::clone(&ctx.shared), ctx.id, ctx.policy);
    state.memory = Some(ctx.memory.clone());
    state.spawner = Some(Arc::new(Spawner::new(
        ctx.engine.clone(),
        ctx.module.clone(),
        ctx.memory.clone(),
        Arc::clone(&ctx.shared),
        ctx.policy,
    )));

    let mut store = Store::new(&ctx.engine, state);
    store.set_fuel(THREAD_FUEL).ok();
    // Trap as soon as the epoch moves. Only `Runtime::drop` ever moves it, so
    // this costs nothing during a run and makes shutdown independent of the
    // worker reaching a host call.
    store.set_epoch_deadline(1);
    // Worker threads are where the interesting host calls happen, so a watch
    // installed only on the main store would miss most of them.
    crate::host::install_memory_watch(&mut store);

    let mut linker = build_linker(&mut store, &ctx.module)?;
    linker
        .define(&store, "env", "memory", ctx.memory.clone())
        .context("defining shared memory for thread")?;

    let instance = linker
        .instantiate(&mut store, &ctx.module)
        .context("instantiating module for thread")?;

    // Wait for the creating thread to finish filling in this pthread.
    //
    // `+52` and `+56` — the stack this thread was given — read as zero at the top
    // of the thread and hold values later, and nothing in
    // `__emscripten_thread_init` writes them. So the guest's own `pthread_create`
    // fills them on the creating thread while this one is already running, and
    // anything read before that is a half-built structure.
    const STACK_READY_SPINS: usize = 2000;
    let (stack_high, stack_size) = (0..STACK_READY_SPINS)
        .find_map(|_| {
            let bounds = read_u32(&store, thread_ptr.saturating_add(STACK_HIGH)).and_then(|high| {
                read_u32(&store, thread_ptr.saturating_add(STACK_SIZE))
                    .and_then(|size| (high != 0 && size != 0 && high > size).then_some((high, size)))
            });
            if bounds.is_none() {
                std::thread::yield_now();
            }
            bounds
        })
        .with_context(|| {
            format!(
                "pthread {thread_ptr:#x} stack bounds were not ready after {STACK_READY_SPINS} yields"
            )
        })?;

    // Bind the guest's thread-local storage to this instance before running
    // anything: emscripten's runtime reads the pthread pointer from TLS, and a
    // routine that starts without it corrupts unrelated state rather than
    // failing.
    // A guard rather than a paired release: everything between here and the
    // entry point can fail with `?`, and a worker that died holding the turn
    // stays the recorded holder forever — every later acquisition then waits
    // out `TURN_TIMEOUT` and forces its way through, which turns one failed
    // initialisation into an unserialised run. See `Scheduler::turn`.
    let turn = ctx.shared.scheduler.turn(ctx.id);
    // Both spellings, for the same reason the main thread needs both: a module
    // that exports `_emscripten_thread_init` and is asked for
    // `__emscripten_thread_init` silently falls through to the TLS-only path,
    // and the thread then has no pthread at all. `pthread_self()` returns null,
    // and the first thing that checks — `emscripten_proxy_execute_queue`, the
    // drain for proxied work — asserts and kills the thread. Every engine
    // worker was dying that way. See `exports.rs`.
    let init_names = ["__emscripten_thread_init", "_emscripten_thread_init"];
    let init = init_names
        .into_iter()
        .find_map(|name| instance.get_func(&mut store, name))
        .ok_or_else(|| {
            crate::exports::missing_from(
                ctx.shared.exports.get(),
                "worker initializer",
                &init_names,
            )
        })?;

    // (pthread_ptr, is_main, is_runtime, can_block, is_default_stack, ...)
    //
    // `can_block = 1`: a worker may block, unlike the main thread. Setting
    // it to 0 here was measured and is much worse — seven of twelve
    // threading tests fail — because the busy-wait costs more than the
    // occasional held turn.
    let args = [
        Val::I32(thread_ptr as i32),
        Val::I32(0),
        Val::I32(0),
        Val::I32(1),
        Val::I32(0),
        Val::I32(0),
    ];
    let arity = init.ty(&store).params().len().min(args.len());
    init.call(&mut store, &args[..arity], &mut [])
        .context("__emscripten_thread_init")?;

    // Install the stack the guest allocated for this pthread. Every worker
    // instance starts with the main instance's stack-pointer value, while the
    // workers are measured running concurrently. Emscripten's
    // `establishStackSpace` reads these same bounds and applies them through
    // the two exports below; entering the routine without them lets workers
    // overwrite each other's frames.
    let stack_names = ["emscripten_stack_set_limits"];
    let set_limits = instance
        .get_func(&mut store, stack_names[0])
        .ok_or_else(|| {
            crate::exports::missing_from(ctx.shared.exports.get(), "stack setup", &stack_names)
        })?;
    let restore_names = ["stackRestore"];
    let restore = instance
        .get_func(&mut store, restore_names[0])
        .ok_or_else(|| {
            crate::exports::missing_from(ctx.shared.exports.get(), "stack restore", &restore_names)
        })?;
    let stack_low = stack_high - stack_size;
    // `(base, end)`, in that order: the bytecode stores the first argument
    // into global 8 and the second into global 7.
    set_limits
        .call(
            &mut store,
            &[Val::I32(stack_high as i32), Val::I32(stack_low as i32)],
            &mut [],
        )
        .context("emscripten_stack_set_limits")?;
    restore
        .call(&mut store, &[Val::I32(stack_high as i32)], &mut [])
        .context("stackRestore")?;

    // The stack this thread ended up with, read through `stackSave` rather than
    // through an exported global. Global 0 *is* the stack pointer, but this
    // module exports no globals — only a separately patched capture does — so
    // asking for `__global_0` here logs nothing at all, which reads as "the
    // stack is fine" rather than as "not measured".
    if let Some(save) = instance.get_func(&mut store, "stackSave") {
        let mut sp = [Val::I32(0)];
        let reading = match save.call(&mut store, &[], &mut sp) {
            Ok(()) => match sp.first() {
                Some(Val::I32(value)) => format!("{value:#x}"),
                _ => "not an i32".to_owned(),
            },
            Err(error) => first_line(&error),
        };
        ctx.shared
            .log(ctx.id, format!("thread {} stack pointer {reading}", ctx.id));
    }

    // TLS as well as the pthread, not instead of it. It was an `else if`, so a
    // module that has both got only the first — and thread-local storage left
    // uninitialised reads as a wild pointer later, which is what the
    // out-of-bounds access in the futex path looks like.
    if let Some(tls) = instance.get_func(&mut store, "_emscripten_tls_init") {
        let mut out = vec![Val::I32(0); tls.ty(&store).results().len()];
        tls.call(&mut store, &[], &mut out)
            .context("_emscripten_tls_init")?;
    }

    drop(turn);

    let entry = table_entry(&mut store, &instance, start_routine)?;
    let results = entry.ty(&store).results().len();
    let mut out = vec![Val::I32(0); results];

    ctx.shared
        .log(ctx.id, format!("thread {} entering routine", ctx.id));

    // Same rule as the main thread: hold a turn while inside guest code.
    let turn = ctx.shared.scheduler.turn(ctx.id);
    let outcome = entry
        .call(&mut store, &[Val::I32(arg as i32)], &mut out)
        .context("thread entry point");
    drop(turn);

    // Why a worker stopped is the difference between "it finished its work"
    // and "the host cut it short", and the two need telling apart: the VoIP
    // engine's pool is empty by the time a call is placed.
    // A thread cut short by shutdown is not a fault, and saying so matters:
    // `Runtime::drop` interrupts every worker by bumping the epoch, so without
    // this every clean teardown reports a handful of traps and a run that went
    // perfectly reads as one that broke.
    let stopped_by_shutdown = ctx.shared.is_shutting_down();
    ctx.shared.log(
        ctx.id,
        match &outcome {
            Ok(()) => format!("thread {} routine returned", ctx.id),
            Err(_) if stopped_by_shutdown => {
                format!("thread {} stopped: host shutting down", ctx.id)
            }
            Err(error) => format!(
                "thread {} stopped: {}",
                ctx.id,
                format!("{error:#}").replace('\n', " | ")
            ),
        },
    );

    // The guest's teardown runs even when the routine trapped, and that is
    // deliberate — it was tried the other way.
    //
    // `_emscripten_thread_exit` frees this thread's stack and TLS through the
    // guest allocator and unlinks it from the pthread list, so running it after
    // a trap means running it against whatever state the trap left. Skipping it
    // in that case looks obviously safer and is measurably worse: three rounds
    // of four corrupt in `examples/ring_corruption.rs`, against about one in
    // four with it. A pthread left linked is something the surviving workers
    // trip over, and worker deaths are what the corruption tracks.
    if let Some(exit) = instance.get_func(&mut store, "_emscripten_thread_exit") {
        let _ = exit.call(&mut store, &[Val::I32(0)], &mut []);
    }
    Ok(outcome?)
}

fn table_entry(
    store: &mut Store<HostState>,
    instance: &wasmtime::Instance,
    index: u32,
) -> Result<wasmtime::Func> {
    // The conventional name is a fallback, not the lookup: a minified module
    // calls its table something else, and asking only for the convention is the
    // same class of silent miss as `exports.rs` describes.
    let name = store
        .data()
        .shared
        .table_export
        .get()
        .cloned()
        .unwrap_or_else(|| "__indirect_function_table".to_owned());

    let Some(Extern::Table(table)) = instance.get_export(&mut *store, &name) else {
        let known = store.data().shared.exports.get().map(|set| set.len());
        return Err(anyhow!(
            "module exports no function table named `{name}` (of {} exports)",
            known.unwrap_or(0)
        ));
    };
    match table.get(&mut *store, index as u64) {
        Some(Ref::Func(Some(func))) => Ok(func),
        _ => Err(anyhow!(
            "thread entry {index} is not a callable table entry"
        )),
    }
}

fn first_line(error: &impl std::fmt::Display) -> String {
    format!("{error:#}")
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// Default budget for `Runtime::quiesce`.
pub const DEFAULT_QUIESCE: Duration = Duration::from_secs(5);
