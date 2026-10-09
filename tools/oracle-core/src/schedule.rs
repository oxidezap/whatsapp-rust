//! Cooperative scheduling of guest threads.
//!
//! Turns encourage progress at host calls but may time out. Workers can execute
//! concurrently, so scheduling is not a mutual exclusion or memory-safety contract.
//! Shutdown disables scheduling and wakes blocked acquisitions before joining workers.

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
    /// Non-zero means guest code was waiting on something it never signalled
    /// through a host call, and the serialisation guarantee was broken to keep
    /// going.
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
        self.waiting.fetch_add(1, Ordering::SeqCst);

        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(holder) = state.holder {
            if holder == thread {
                break;
            }
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
