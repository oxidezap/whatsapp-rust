//! Cooperative scheduling of guest threads.
//!
//! Turns encourage progress at host calls but may time out. Workers can execute
//! concurrently, so scheduling is not a mutual exclusion or memory-safety contract.
//! Shutdown disables scheduling and wakes blocked acquisitions before joining workers.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

/// How long a thread waits for its turn before taking it anyway.
const TURN_TIMEOUT: Duration = Duration::from_secs(5);

/// The same, while strict turns are demanded.
///
/// Short because under strict turns *every* crossing of the host boundary
/// acquires, and the common case is a worker blocked in `memory.atomic.wait32`
/// holding the turn from inside wasm where nothing can take it back. At five
/// seconds that is a stall per crossing and a round that never finishes; at
/// this it degrades to "mostly serialised", which is enough to attribute a
/// write and cheap enough to reach the write in the first place.
const STRICT_TIMEOUT: Duration = Duration::from_millis(25);

/// Coordinates cooperative turns across guest threads.
#[derive(Debug, Default)]
pub struct Scheduler {
    state: Mutex<State>,
    turn_available: Condvar,
    /// Threads currently blocked waiting for a turn. Read without the lock so
    /// the common case — nobody waiting — costs one atomic load per host call
    /// rather than a lock acquisition.
    waiting: AtomicUsize,
    /// Times a thread gave up waiting and ran anyway.
    forced: AtomicU64,
    enabled: std::sync::atomic::AtomicBool,
    /// Whether the turn must be held across every guest-execution window rather
    /// than only around a thread's routine. See `STRICT_TIMEOUT`.
    strict: std::sync::atomic::AtomicBool,
}

#[derive(Debug, Default)]
struct State {
    /// The thread holding the turn, if any.
    holder: Option<u64>,
    /// Arrival order prevents a yielding poller from overtaking sleeping waiters.
    queue: VecDeque<u64>,
}

impl Scheduler {
    /// Turns scheduling on. Off by default: a single-threaded module pays
    /// nothing, and the cost only makes sense once threads actually run.
    pub fn enable(&self) {
        self.enabled.store(true, Ordering::SeqCst);
    }

    pub(crate) fn shutdown(&self) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        self.enabled.store(false, Ordering::SeqCst);
        state.holder = None;
        self.turn_available.notify_all();
    }

    /// Requests turns at each guest entry. See `Runtime::demand_strict_turns`.
    pub fn demand_strict(&self) {
        self.strict.store(true, Ordering::SeqCst);
    }

    /// Whether turns are being held for the whole of guest execution.
    #[must_use]
    pub fn is_strict(&self) -> bool {
        self.strict.load(Ordering::SeqCst)
    }

    /// Whether the scheduler is handing out turns at all.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// How often a thread had to take its turn without being granted one.
    ///
    /// Non-zero means a waiter exhausted its deadline and ran before the
    /// holder released its turn. Guest waits and host scheduling delays can
    /// both cause this; turns do not guarantee mutual exclusion.
    pub fn forced_turns(&self) -> u64 {
        self.forced.load(Ordering::SeqCst)
    }

    /// Blocks until `thread` may execute guest code.
    pub fn acquire(&self, thread: u64) {
        if !self.is_enabled() {
            return;
        }

        let deadline = Instant::now()
            + if self.is_strict() {
                STRICT_TIMEOUT
            } else {
                TURN_TIMEOUT
            };
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.holder == Some(thread) {
            return;
        }
        state.queue.push_back(thread);
        self.waiting.fetch_add(1, Ordering::SeqCst);
        while self.is_enabled() && (state.holder.is_some() || state.queue.front() != Some(&thread))
        {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.forced.fetch_add(1, Ordering::SeqCst);
                break;
            }
            let (next, _) = self
                .turn_available
                .wait_timeout(state, remaining)
                .unwrap_or_else(|e| e.into_inner());
            state = next;
        }
        // A timed-out waiter can leave from the middle; shutdown also drains
        // each caller's entry before it returns to the host.
        if let Some(index) = state.queue.iter().position(|queued| *queued == thread) {
            state.queue.remove(index);
        }
        if self.is_enabled() {
            state.holder = Some(thread);
        }
        self.waiting.fetch_sub(1, Ordering::SeqCst);
    }

    /// Takes a turn for `thread` and gives it back when the guard is dropped.
    ///
    /// The paired `acquire`/`release` spelling is only correct on a path with
    /// no early return, and `threads.rs` has several: a worker whose
    /// `__emscripten_thread_init` traps used to leave itself recorded as the
    /// holder forever. Every later acquisition then waited out `TURN_TIMEOUT`
    /// and forced its way through, so one initialisation failure turned the
    /// scheduler off for the rest of the run.
    pub fn turn(&self, thread: u64) -> Turn<'_> {
        self.acquire(thread);
        Turn {
            scheduler: self,
            thread,
        }
    }

    /// Gives up the turn held by `thread`.
    pub fn release(&self, thread: u64) {
        if !self.is_enabled() {
            return;
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.holder == Some(thread) {
            state.holder = None;
            self.turn_available.notify_all();
        }
    }

    /// A yield point: hands the turn on if anything is waiting for it.
    ///
    /// Called from every host call. When nothing is waiting this is a single
    /// atomic load, so the guest's own hot paths — the VoIP worker polls the
    /// clock constantly — do not pay for the machinery.
    pub fn yield_point(&self, thread: u64) {
        if !self.is_enabled() || self.waiting.load(Ordering::SeqCst) == 0 {
            return;
        }
        self.release(thread);
        std::thread::yield_now();
        self.acquire(thread);
    }
}

/// A held scheduler turn, released on drop. See [`Scheduler::turn`].
#[derive(Debug)]
pub struct Turn<'a> {
    scheduler: &'a Scheduler,
    thread: u64,
}

impl Drop for Turn<'_> {
    fn drop(&mut self) {
        self.scheduler.release(self.thread);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_released_turn_cannot_overtake_an_existing_waiter() {
        use std::sync::Arc;

        let scheduler = Arc::new(Scheduler::default());
        scheduler.enable();
        scheduler.acquire(0);
        let order = Arc::new(Mutex::new(Vec::new()));
        let waiting_scheduler = Arc::clone(&scheduler);
        let waiting_order = Arc::clone(&order);
        let waiter = std::thread::spawn(move || {
            let _turn = waiting_scheduler.turn(1);
            waiting_order.lock().unwrap().push(1);
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while scheduler.waiting.load(Ordering::SeqCst) != 1 {
            assert!(Instant::now() < deadline, "waiter did not enter scheduler");
            std::thread::yield_now();
        }
        // Model a released turn before the sleeping waiter receives its wakeup.
        // A new arrival must not take that turn while the first waiter sleeps.
        scheduler.state.lock().unwrap().holder = None;
        let waking_scheduler = Arc::clone(&scheduler);
        let observed_order = Arc::clone(&order);
        let wakeup = std::thread::spawn(move || {
            while waking_scheduler.waiting.load(Ordering::SeqCst) != 2
                && observed_order.lock().unwrap().is_empty()
            {
                assert!(
                    Instant::now() < deadline,
                    "second acquisition never arrived"
                );
                std::thread::yield_now();
            }
            waking_scheduler.turn_available.notify_all();
        });
        {
            let _turn = scheduler.turn(0);
            order.lock().unwrap().push(0);
        }
        wakeup.join().unwrap();
        waiter.join().unwrap();
        assert_eq!(*order.lock().unwrap(), [1, 0]);
        assert_eq!(scheduler.forced_turns(), 0);
    }

    #[test]
    fn strict_typed_host_call_does_not_reacquire_the_guest_turn() {
        use wasm_encoder::{
            CodeSection, EntityType, ExportKind, ExportSection, Function, FunctionSection,
            ImportSection, Instruction, Module, TypeSection,
        };

        let mut types = TypeSection::new();
        types.ty().function([], []);
        let mut imports = ImportSection::new();
        imports.import("test", "probe", EntityType::Function(0));
        let mut functions = FunctionSection::new();
        functions.function(0);
        let mut exports = ExportSection::new();
        exports.export("run", ExportKind::Func, 1);
        let mut function = Function::new([]);
        function.instruction(&Instruction::Call(0));
        function.instruction(&Instruction::End);
        let mut code = CodeSection::new();
        code.function(&function);
        let mut module = Module::new();
        module
            .section(&types)
            .section(&imports)
            .section(&functions)
            .section(&exports)
            .section(&code);

        let engine = wasmtime::Engine::default();
        let module = wasmtime::Module::new(&engine, module.finish()).unwrap();
        let mut store = wasmtime::Store::new(&engine, crate::state::HostState::default());
        let shared = std::sync::Arc::clone(&store.data().shared);
        shared.scheduler.enable();
        shared.demand_strict_turns();
        // Keep the waiter signal set across the boundary, without an OS race
        // against the 25 ms strict timeout. We test turn ownership inside the
        // actual typed import; no timeout or scheduling operation is replaced.
        shared.scheduler.waiting.store(1, Ordering::SeqCst);
        crate::host::install_memory_watch(&mut store);
        let mut linker = wasmtime::Linker::new(&engine);
        linker
            .func_wrap(
                "test",
                "probe",
                |caller: wasmtime::Caller<'_, crate::state::HostState>| {
                    let scheduler = &caller.data().shared.scheduler;
                    assert_ne!(
                        scheduler.state.lock().unwrap().holder,
                        Some(caller.data().thread_id),
                        "strict host execution must leave the guest turn released"
                    );
                    scheduler.waiting.store(0, Ordering::SeqCst);
                },
            )
            .unwrap();
        let instance = linker.instantiate(&mut store, &module).unwrap();
        instance
            .get_typed_func::<(), ()>(&mut store, "run")
            .unwrap()
            .call(&mut store, ())
            .unwrap();
        assert_eq!(shared.scheduler.forced_turns(), 0);
        assert_eq!(shared.scheduler.state.lock().unwrap().holder, None);
    }

    #[test]
    fn clock_import_yields_to_a_waiting_thread() {
        use std::sync::Arc;
        use wasm_encoder::{
            BlockType, CodeSection, EntityType, ExportKind, ExportSection, Function,
            FunctionSection, ImportSection, Instruction, MemArg, MemoryType, Module, TypeSection,
            ValType,
        };

        let mut types = TypeSection::new();
        types.ty().function([], [ValType::F64]);
        types.ty().function([], []);
        let mut imports = ImportSection::new();
        imports.import("env", "emscripten_get_now", EntityType::Function(0));
        imports.import(
            "env",
            "memory",
            EntityType::Memory(MemoryType {
                minimum: 1,
                maximum: Some(1),
                memory64: false,
                shared: true,
                page_size_log2: None,
            }),
        );
        let mut functions = FunctionSection::new();
        functions.function(1);
        let mut exports = ExportSection::new();
        exports.export("poll", ExportKind::Func, 1);
        let mut function = Function::new([]);
        // The waiting thread publishes one byte only after obtaining its turn.
        // A clock-only guest loop must let it progress before the timeout escape.
        for instruction in [
            Instruction::Loop(BlockType::Empty),
            Instruction::Call(0),
            Instruction::Drop,
            Instruction::I32Const(0),
            Instruction::I32AtomicLoad8U(MemArg {
                offset: 0,
                align: 0,
                memory_index: 0,
            }),
            Instruction::I32Eqz,
            Instruction::BrIf(0),
            Instruction::End,
            Instruction::End,
        ] {
            function.instruction(&instruction);
        }
        let mut code = CodeSection::new();
        code.function(&function);
        let mut module = Module::new();
        module
            .section(&types)
            .section(&imports)
            .section(&functions)
            .section(&exports)
            .section(&code);
        let mut runtime = crate::Runtime::instantiate(&module.finish()).unwrap();
        let shared = Arc::clone(runtime.shared());
        shared.scheduler.enable();
        shared.scheduler.acquire(0);
        let memory = runtime.state().memory.clone();
        let waiter_shared = Arc::clone(&shared);
        let waiter = std::thread::spawn(move || {
            let _turn = waiter_shared.scheduler.turn(7);
            let mut state = crate::state::HostState::default();
            state.memory = memory;
            state.write(0, &[1]).unwrap();
        });
        let deadline = Instant::now() + Duration::from_secs(3);
        while shared.scheduler.waiting.load(Ordering::SeqCst) == 0 {
            assert!(
                Instant::now() < deadline,
                "waiter did not reach the scheduler"
            );
            std::thread::yield_now();
        }
        let outcome = runtime.call("poll", &[]);
        waiter.join().unwrap();
        outcome.unwrap();
        assert_eq!(shared.scheduler.forced_turns(), 0);
    }

    /// The failure the guard exists for: a worker that returns early while
    /// holding the turn stays the recorded holder, and every later acquisition
    /// then waits out `TURN_TIMEOUT` and forces its way through — one failed
    /// initialisation turning serialisation off for the rest of the run.
    #[test]
    fn runtime_drop_waits_for_a_worker_blocked_in_the_host() {
        use wasm_encoder::{EntityType, ImportSection, MemoryType, Module};
        let mut imports = ImportSection::new();
        imports.import(
            "env",
            "memory",
            EntityType::Memory(MemoryType {
                minimum: 1,
                maximum: Some(1),
                memory64: false,
                shared: true,
                page_size_log2: None,
            }),
        );
        let mut module = Module::new();
        module.section(&imports);
        let mut runtime = crate::Runtime::instantiate(&module.finish()).unwrap();
        runtime.set_thread_policy(crate::ThreadPolicy::Spawn);
        runtime
            .write_bytes_at(128 + 52, &4096_u32.to_le_bytes())
            .unwrap();
        runtime
            .write_bytes_at(128 + 56, &1024_u32.to_le_bytes())
            .unwrap();
        let shared = std::sync::Arc::clone(runtime.shared());
        shared.scheduler.acquire(0);
        assert_eq!(
            runtime.state().spawner.as_ref().unwrap().spawn(128, 0, 0),
            0
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while shared.scheduler.waiting.load(Ordering::SeqCst) == 0 {
            assert!(
                Instant::now() < deadline,
                "worker never reached scheduler acquisition"
            );
            std::thread::yield_now();
        }
        drop(runtime);
        let remaining = shared.live_threads();
        shared.scheduler.release(0);
        assert!(shared.wait_until_idle(Duration::from_secs(5)));
        assert_eq!(remaining, 0, "drop returned with a detached worker");
    }

    #[test]
    fn a_turn_is_given_back_when_its_holder_returns_early() {
        let scheduler = Scheduler::default();
        scheduler.enable();

        fn fallible(scheduler: &Scheduler) -> Result<(), ()> {
            let _turn = scheduler.turn(7);
            Err(())
        }

        assert!(fallible(&scheduler).is_err());
        assert!(
            scheduler
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .holder
                .is_none(),
            "the dead holder must not still own the turn"
        );

        // And another thread takes it without having to force its way in.
        scheduler.acquire(9);
        scheduler.release(9);
        assert_eq!(scheduler.forced_turns(), 0);
    }

    /// Releasing is still keyed on the holder, so a guard cannot take a turn
    /// away from whoever actually has it.
    #[test]
    fn a_guard_releases_only_its_own_turn() {
        let scheduler = Scheduler::default();
        scheduler.enable();

        scheduler.acquire(1);
        drop(scheduler.turn(1));
        assert!(
            scheduler
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .holder
                .is_none(),
            "the same thread's guard gives the turn back"
        );

        scheduler.acquire(1);
        // A guard for a *different* thread would block, so only the release
        // path is exercised here: thread 2 releasing does nothing to thread 1.
        scheduler.release(2);
        assert_eq!(
            scheduler
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .holder,
            Some(1)
        );
    }
}
