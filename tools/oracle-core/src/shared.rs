//! State shared by every thread of one module.
//!
//! A wasm thread is a separate instance of the same module over the same linear
//! memory, so each one has its own `Store` and its own `HostState`. Anything
//! that must be coherent across them — the trace and the clock — lives here
//! behind a lock.
//!
//! Which state is shared and which is per-thread is a correctness question, not
//! a convenience one. An in-flight C++ exception belongs to the thread
//! unwinding. A monotonic clock that ran backwards between threads would be
//! worse than no clock. And the PRNG is deliberately *not* shared — see
//! `seed_for`.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Condvar, Mutex};

use crate::state::HostCall;

/// Milliseconds the *monotonic* clock advances per observation.
///
/// The step has to be non-zero — a clock that never moves makes the engine
/// busy-wait forever — and it is charged per observation. The VoIP engine polls
/// this millions of times while waiting on a futex, so the total is large by
/// design: that is what makes a spin terminate.
const CLOCK_STEP_MS: f64 = 0.01;

/// Milliseconds the *wall* clock advances per reading.
///
/// A browser has two clocks and the guest uses them for different things:
/// `performance.now()` to measure how long something took, `Date.now()` to
/// stamp a protocol message. Deriving both from one counter here conflated
/// them, and the busy-wait's millions of monotonic observations dragged the
/// wall clock forward with them — 345 seconds during a single call. The engine
/// then compared an offer's timestamp against `wa_call_is_offer_expired`'s
/// 45-second threshold and dropped it as `Missed`, which looked like the engine
/// rejecting the offer and was the harness ageing it.
///
/// Wall time therefore advances only when it is *read as wall time*, which the
/// guest does a handful of times per call rather than millions.
const WALL_STEP_MS: f64 = 1.0;

/// Seed for the host PRNG. Any constant works; it only has to be stable.
const RNG_SEED: u64 = 0x5741_5F4F_5241_434C;

/// How many individual calls are kept with their arguments.
const MAX_TRACE: usize = 8192;

/// How many markers to keep. Generous: the tail is what is read, and a round
/// that fires more than this has more to say than any single reading of it.
const MAX_MARKERS: usize = 4096;

/// How many log lines are kept, and how long each one may be.
///
/// Both bounds exist because an unbounded log is a memory leak waiting for a
/// hot failing path, and this host found one: a mailbox drain that fails is
/// polled thousands of times, and each failure was recorded with its full wasm
/// backtrace. One test reached 43192 lines totalling 8.5 GB, the longest line
/// 1.1 MB, and the suite was killed by the OOM killer. A truncated line still
/// says which call failed and where, which is what the log is for.
const MAX_LOG_LINES: usize = 8192;
const MAX_LOG_LINE: usize = 2048;

/// One line of host-visible output, tagged with where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Ordinal in a single global sequence. Wall-clock ordering between threads
    /// is not reproducible, but this is: it is what makes a trace comparable
    /// across runs even when the OS interleaves differently.
    pub seq: u64,
    /// 0 is the thread that instantiated the module.
    pub thread: u64,
    /// The line itself, truncated to `MAX_LOG_LINE`.
    pub text: String,
}

#[derive(Debug, Default)]
struct Trace {
    calls: Vec<HostCall>,
    counts: BTreeMap<String, u64>,
    logs: Vec<LogLine>,
    dropped_logs: u64,
}

/// One `sendSignalingXMPP_js_sync` call, with the stanza copied out of guest
/// memory while it was still there.
///
/// The argument order is the engine's: peer JID, call id, stanza, length. Both
/// JIDs are NUL-terminated C strings; the stanza is *not* text — it is
/// WhatsApp's binary XMPP encoding, which is why it is kept as bytes and left
/// for a caller to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalingCall {
    /// Who the stanza is addressed to, as the engine spelled it.
    pub peer_jid: String,
    /// The call this belongs to.
    pub call_id: String,
    /// The stanza itself, copied before the guest freed it. Empty if the
    /// length the guest passed did not name a readable range.
    pub stanza: Vec<u8>,
}

/// The cross-thread half of the host state.
#[derive(Debug)]
pub struct SharedHost {
    trace: Mutex<Trace>,
    /// The import `patch.rs` markers call, once somebody asks to watch it.
    marker_sink: Mutex<Option<String>>,
    pub(crate) snapshots: std::sync::OnceLock<crate::snapshot::Recorder>,
    /// Markers seen, as a **ring**: the last [`MAX_MARKERS`], not the first.
    ///
    /// Separate from `trace` on purpose. The trace answers "what did the module
    /// ask the host for" and `host_environment.rs` asserts it verbatim, so it
    /// truncates at the front and must keep doing so. A marker trace answers
    /// "how far did it get", where the only interesting end is the last one
    /// before a trap — and the engine makes far more than 8192 host calls
    /// before failing, so a front-truncating buffer records the startup and
    /// drops the answer.
    markers: Mutex<std::collections::VecDeque<(i32, i64)>>,
    /// Signaling the guest handed the host, with the bytes copied out.
    ///
    /// Its own store rather than part of `trace` for one reason: the trampoline
    /// that calls `sendSignalingXMPP_js_sync` **frees all three pointers as soon
    /// as the import returns**, and the allocator hands the memory straight back
    /// out. A caller that reads the arguments afterwards gets whatever landed
    /// there next. The bytes have to be copied inside the host call, so the
    /// host call is where this is filled.
    signaling: Mutex<Vec<SignalingCall>>,
    /// Virtual monotonic clock, in milliseconds, advanced under a lock so no
    /// thread ever observes it going backwards.
    clock_ms: Mutex<f64>,
    /// Virtual wall clock, in milliseconds. Separate from `clock_ms` so that
    /// measuring a duration does not age a timestamp — see `WALL_STEP_MS`.
    wall_ms: Mutex<f64>,

    next_seq: AtomicU64,
    next_thread_id: AtomicU64,
    /// Threads started and not yet finished.
    live: AtomicUsize,
    /// Signalled whenever a thread finishes, so a waiter can be woken.
    idle: Condvar,
    idle_lock: Mutex<()>,
    /// Set when any thread calls `exit` or `proc_exit`.
    exit_code: Mutex<Option<i32>>,
    /// Raised when the owning `Runtime` goes away.
    shutting_down: std::sync::atomic::AtomicBool,
    /// Decides which guest thread may execute. See `schedule.rs`.
    pub scheduler: crate::schedule::Scheduler,
    /// Imports that dispatch through the function table, by name.
    ///
    /// A fact about the module rather than about a thread, so it is settled once
    /// at instantiation and every spawned thread links against the same set.
    /// See `abi::find_invoke_imports` for why the name alone is not enough.
    pub invoke_imports: std::sync::OnceLock<std::collections::BTreeSet<String>>,
    /// Export name of the guest's function table, when it is not the
    /// conventional `__indirect_function_table`.
    pub table_export: std::sync::OnceLock<String>,
    /// Threads that have been told to check their mailbox and have not yet.
    ///
    /// Emscripten wakes a thread by posting it a `checkMailbox` message; there
    /// are no messages here, so the notification is recorded and the thread
    /// acts on it when it next waits. Without this the proxying queue is filled
    /// and never drained, which is why the engine's callbacks never fired.
    pub mailboxes: Mutex<std::collections::BTreeSet<u64>>,
    /// Imports that got a zero-returning stub rather than an implementation.
    ///
    /// A stub that is never called is harmless. One the guest *does* call is a
    /// lie waiting to surface frames away — `emscripten_resize_heap` was one,
    /// and it cost days. Recording the set lets a run report which lies it
    /// actually told.
    pub stubbed: std::sync::OnceLock<std::collections::BTreeSet<String>>,
    /// Every name the module exports.
    ///
    /// Kept so that a failed lookup can say what the module *does* have. See
    /// `exports.rs` for the bug that motivated it.
    pub exports: std::sync::OnceLock<std::collections::BTreeSet<String>>,
    /// A span of guest memory that must never change. See `MemoryWatch`.
    pub watch: std::sync::OnceLock<MemoryWatch>,
    /// Guest memory size the last time anyone looked, so growth can be noticed.
    last_size: AtomicUsize,
    /// Every growth seen, with the guest stack that was running.
    ///
    /// The heap ends at exactly `0x10e0000` on a corrupt round and `0xf10000`
    /// on a healthy one — two values, not a distribution — and the growth
    /// happens *before* the memory is destroyed. So one allocation takes a
    /// different path, and this is what names it: most growth never reaches
    /// `emscripten_resize_heap`, the guest running `memory.grow` itself, but
    /// every crossing of the host boundary can still see the size change.
    growths: Mutex<Vec<String>>,
    /// How many threads are inside guest code right now.
    in_wasm: AtomicUsize,
    /// The most that has ever been, which is the number that matters.
    ///
    /// It must be 1. `schedule.rs` exists to make it 1, every guest thread
    /// starts from the same initial stack pointer — the stack pointer is a
    /// per-instance global initialised from the module — and two threads
    /// executing at once therefore push frames over each other's live frames.
    /// Anything above 1 here is unbounded memory corruption, not a slowdown.
    max_in_wasm: AtomicUsize,
}

/// A span of guest memory the host expects to stay constant, checked on entry
/// to **every** host call.
///
/// Knowing that memory was destroyed some time during a call is most of a
/// diagnosis short of the part that matters. This narrows it: host calls are
/// frequent — a guest worker reaches one every few thousand instructions — so
/// the first call that sees the span changed is close in time to whatever
/// changed it, and it knows which thread it is on and what the guest was
/// executing. See `Runtime::watch_memory`.
#[derive(Debug)]
pub struct MemoryWatch {
    /// Where the span starts in linear memory.
    pub at: u32,
    /// What it held when the watch was set.
    pub expected: Vec<u8>,
    /// Set once anyone sees the span changed, so the example can say whether
    /// the watch fired at all.
    pub broken: std::sync::atomic::AtomicBool,
    /// One sighting per thread, in the order they arrived.
    ///
    /// Per thread rather than once overall, because the first sighting names
    /// the thread that *noticed* and that is rarely the one that wrote: the
    /// first catch here was a media worker sitting in `pj_thread_sleep`. The
    /// damage is progressive and the writer yields its turn while it runs, so
    /// it enters host code too — a few sightings later.
    ///
    /// Held here rather than only in the host log, because the log *refuses*
    /// new lines once full rather than evicting old ones, and a run that
    /// corrupts its own memory is a chatty run. The report was being written at
    /// line 8193 and discarded, which looked exactly like a watch that never
    /// fired.
    pub sightings: Mutex<Vec<(u64, bool, String)>>,
}

/// How many threads to catch after the span changes.
///
/// Small: the engine runs a handful of workers, and once each has been seen
/// once there is nothing further to learn from repeating them.
pub const MAX_SIGHTINGS: usize = 12;

impl Default for SharedHost {
    fn default() -> Self {
        Self {
            trace: Mutex::new(Trace::default()),
            marker_sink: Mutex::new(None),
            snapshots: std::sync::OnceLock::new(),
            markers: Mutex::new(std::collections::VecDeque::new()),
            signaling: Mutex::new(Vec::new()),
            clock_ms: Mutex::new(0.0),
            wall_ms: Mutex::new(0.0),
            next_seq: AtomicU64::new(0),
            next_thread_id: AtomicU64::new(1),
            live: AtomicUsize::new(0),
            idle: Condvar::new(),
            idle_lock: Mutex::new(()),
            invoke_imports: std::sync::OnceLock::new(),
            table_export: std::sync::OnceLock::new(),
            exports: std::sync::OnceLock::new(),
            watch: std::sync::OnceLock::new(),
            last_size: AtomicUsize::new(0),
            growths: Mutex::new(Vec::new()),
            in_wasm: AtomicUsize::new(0),
            max_in_wasm: AtomicUsize::new(0),
            mailboxes: Mutex::new(std::collections::BTreeSet::new()),
            stubbed: std::sync::OnceLock::new(),
            exit_code: Mutex::new(None),
            shutting_down: std::sync::atomic::AtomicBool::new(false),
            scheduler: crate::schedule::Scheduler::default(),
        }
    }
}

impl SharedHost {
    /// Advances and returns the virtual clock.
    ///
    /// Every thread reads the same clock, so time cannot appear to move
    /// backwards when execution crosses threads — which is what a busy-wait
    /// loop with a deadline would notice first.
    pub fn tick_clock(&self) -> f64 {
        let mut clock = self.clock_ms.lock().unwrap_or_else(|e| e.into_inner());
        *clock += CLOCK_STEP_MS;
        *clock
    }

    /// Advances and reads the wall clock, in milliseconds since the epoch
    /// origin. Independent of the monotonic clock; see `WALL_STEP_MS`.
    pub fn tick_wall_clock(&self) -> f64 {
        let mut wall = self.wall_ms.lock().unwrap_or_else(|e| e.into_inner());
        *wall += WALL_STEP_MS;
        *wall
    }

    /// Reads the wall clock without advancing it.
    pub fn wall_clock(&self) -> f64 {
        *self.wall_ms.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The monotonic clock, advanced by one step per observation.
    #[must_use]
    pub fn clock(&self) -> f64 {
        *self.clock_ms.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The PRNG seed for a given thread.
    ///
    /// Deliberately *not* one shared stream. A single stream is reproducible
    /// only if it is consumed in a reproducible order, and under threads it is
    /// not: two runs can interleave their `random_get` calls differently and
    /// hand the same caller different bytes. Per-thread streams, seeded from the
    /// thread id, make each thread's sequence depend only on how many bytes that
    /// thread has taken — which *is* reproducible.
    pub fn seed_for(thread: u64) -> u64 {
        RNG_SEED ^ thread.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    /// Records a host log line, tagged with the thread that wrote it.
    pub fn log(&self, thread: u64, mut text: String) {
        if text.len() > MAX_LOG_LINE {
            // On a char boundary, so this cannot split a multi-byte sequence.
            let cut = (0..=MAX_LOG_LINE)
                .rev()
                .find(|at| text.is_char_boundary(*at))
                .unwrap_or(0);
            text.truncate(cut);
            text.push_str(" […truncated]");
        }
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let mut trace = self.trace.lock().unwrap_or_else(|e| e.into_inner());
        if trace.logs.len() >= MAX_LOG_LINES {
            // Counted rather than dropped silently: a reader who sees a log
            // stop dead needs to know whether that is the end of the run or the
            // end of the buffer.
            trace.dropped_logs += 1;
            return;
        }
        trace.logs.push(LogLine { seq, thread, text });
    }

    /// Turns on strict turn-taking. See `strict_turns`.
    pub fn demand_strict_turns(&self) {
        self.scheduler.demand_strict();
    }

    /// Whether the scheduler is holding turns across guest execution.
    #[must_use]
    pub fn strict_turns(&self) -> bool {
        self.scheduler.is_strict()
    }

    /// Notes the current guest memory size, returning the previous one when it
    /// has changed.
    ///
    /// A single atomic swap in the common case, which matters: this runs on
    /// every crossing of the host boundary and a guest worker makes tens of
    /// millions of them.
    pub fn note_memory_size(&self, size: usize) -> Option<usize> {
        let previous = self.last_size.swap(size, Ordering::SeqCst);
        (previous != size && previous != 0).then_some(previous)
    }

    /// Records one growth, with whatever the guest was doing at the time.
    pub fn record_growth(&self, line: String) {
        /// Enough to see the whole sequence of a round; a corrupt round and a
        /// healthy one diverge well before this.
        const MAX: usize = 64;

        let mut growths = self.growths.lock().unwrap_or_else(|e| e.into_inner());
        if growths.len() < MAX {
            growths.push(line);
        }
    }

    /// Every memory growth seen, with the guest stack that asked for it.
    #[must_use]
    pub fn growths(&self) -> Vec<String> {
        self.growths
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Records a thread entering guest code, and returns nothing.
    pub fn entered_wasm(&self) {
        let now = self.in_wasm.fetch_add(1, Ordering::SeqCst) + 1;
        self.max_in_wasm.fetch_max(now, Ordering::SeqCst);
    }

    /// Records a thread leaving guest code.
    pub fn left_wasm(&self) {
        // Saturating: a thread that traps out of guest code can leave without a
        // matching entry, and an underflow here would read as a huge count.
        let _ = self
            .in_wasm
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| {
                Some(now.saturating_sub(1))
            });
    }

    /// The most guest threads that have ever been executing at once.
    ///
    /// See `max_in_wasm`: anything above 1 means threads were running over each
    /// other's stack frames.
    pub fn max_threads_in_wasm(&self) -> usize {
        self.max_in_wasm.load(Ordering::SeqCst)
    }

    /// How many log lines were discarded because the buffer was full.
    pub fn dropped_logs(&self) -> u64 {
        self.trace
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .dropped_logs
    }

    /// Names the import that carries instrumentation markers.
    ///
    /// Until this is set nothing is mirrored, so an uninstrumented run pays a
    /// string comparison and nothing else.
    pub fn watch_markers(&self, symbol: &str) {
        *self.marker_sink.lock().unwrap_or_else(|e| e.into_inner()) = Some(symbol.to_owned());
    }

    /// The markers seen, oldest first, up to the last [`MAX_MARKERS`].
    pub fn markers(&self) -> Vec<(i32, i64)> {
        self.markers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .copied()
            .collect()
    }

    /// Records one host call, and mirrors it into the marker ring when it is
    /// the sink `watch_markers` named.
    pub fn record(&self, module: &str, name: &str, args: Vec<i64>) {
        let symbol = format!("{module}::{name}");

        if self
            .marker_sink
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_deref()
            == Some(symbol.as_str())
            && let (Some(id), value) = (args.first().copied(), args.get(1).copied().unwrap_or(0))
            && let Ok(id) = i32::try_from(id)
        {
            let mut markers = self.markers.lock().unwrap_or_else(|e| e.into_inner());
            if markers.len() == MAX_MARKERS {
                markers.pop_front();
            }
            markers.push_back((id, value));
        }

        let mut trace = self.trace.lock().unwrap_or_else(|e| e.into_inner());
        *trace.counts.entry(symbol).or_default() += 1;
        if trace.calls.len() < MAX_TRACE {
            trace.calls.push(HostCall {
                module: module.to_owned(),
                name: name.to_owned(),
                args,
            });
        }
    }

    /// Every host log line, in global sequence order.
    #[must_use]
    pub fn logs(&self) -> Vec<LogLine> {
        let mut logs = self
            .trace
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .logs
            .clone();
        // Sorting by sequence rather than by arrival makes the transcript
        // reproducible even though the interleaving is not.
        logs.sort_by_key(|line| line.seq);
        logs
    }

    /// The same lines, without their sequence and thread tags.
    #[must_use]
    pub fn log_texts(&self) -> Vec<String> {
        self.logs().into_iter().map(|line| line.text).collect()
    }

    /// Records that `thread` has mail waiting.
    pub fn notify_mailbox(&self, thread: u64) {
        self.mailboxes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(thread);
    }

    /// Takes the notification for `thread`, if there is one.
    pub fn take_mailbox(&self, thread: u64) -> bool {
        self.mailboxes
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&thread)
    }

    /// The first `MAX_TRACE` host calls, with their arguments.
    ///
    /// **The bound bites far earlier than it looks.** Bringing the VoIP engine
    /// up makes ~39 million host calls, so this list is full within the first
    /// moments of a run and nothing later ever appears in it. Searching it for
    /// a call that happened afterwards finds nothing, which reads as "it never
    /// happened" and is not. Use [`Self::hot_calls`] for whether, and
    /// [`Self::clear_trace`] right before the stretch being measured to get the
    /// arguments.
    #[must_use]
    pub fn calls(&self) -> Vec<HostCall> {
        self.trace
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .calls
            .clone()
    }

    /// Records one outbound signaling call, bytes and all.
    ///
    /// Called from the `sendSignalingXMPP_js_sync` host function, which is the
    /// only moment the stanza exists: its trampoline frees the buffer as soon
    /// as this returns.
    pub fn record_signaling(&self, call: SignalingCall) {
        self.signaling
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(call);
    }

    /// Every stanza the guest asked the host to send, oldest first.
    #[must_use]
    pub fn signaling(&self) -> Vec<SignalingCall> {
        self.signaling
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Host symbols by call count, most-called first.
    pub fn hot_calls(&self) -> Vec<(String, u64)> {
        let trace = self.trace.lock().unwrap_or_else(|e| e.into_inner());
        let mut counts: Vec<(String, u64)> = trace
            .counts
            .iter()
            .map(|(symbol, count)| (symbol.clone(), *count))
            .collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    }

    /// How many host calls were made, including those past the trace bound.
    #[must_use]
    pub fn total_calls(&self) -> u64 {
        self.trace
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .counts
            .values()
            .sum()
    }

    /// Forgets the recorded calls, so a later stretch can be measured alone.
    pub fn clear_trace(&self) {
        let mut trace = self.trace.lock().unwrap_or_else(|e| e.into_inner());
        trace.calls.clear();
        trace.counts.clear();
        trace.logs.clear();
    }

    /// Records that a thread called `exit` or `proc_exit`.
    pub fn set_exit_code(&self, code: i32) {
        *self.exit_code.lock().unwrap_or_else(|e| e.into_inner()) = Some(code);
    }

    /// The exit code, if any thread asked to exit.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        *self.exit_code.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Asks every guest thread to stop at its next host call.
    ///
    /// A guest worker is usually an unbounded loop — it only ends when its fuel
    /// runs out, which with a realistic budget is a long time. Without this a
    /// finished test leaves threads burning CPU behind it, and the next test
    /// competes with every engine that ran before it.
    pub fn request_shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    /// Whether the owning `Runtime` has gone away and workers should stop.
    #[must_use]
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    /// Hands out the next guest thread id. 0 is the instantiating thread.
    pub fn allocate_thread_id(&self) -> u64 {
        self.next_thread_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Records a guest thread starting.
    pub fn thread_started(&self) {
        self.live.fetch_add(1, Ordering::SeqCst);
    }

    /// Marks a thread finished and wakes anything waiting for quiescence.
    pub fn thread_finished(&self) {
        self.live.fetch_sub(1, Ordering::SeqCst);
        let _guard = self.idle_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.idle.notify_all();
    }

    /// Guest threads started and not yet finished.
    #[must_use]
    pub fn live_threads(&self) -> usize {
        self.live.load(Ordering::SeqCst)
    }

    /// Waits until every spawned thread has finished, or the deadline passes.
    ///
    /// This is the determinism mitigation that matters: the *interleaving* of
    /// threads is not reproducible, but the state observed after they have all
    /// finished usually is. Anything read before quiescing is a race with the
    /// module's own workers.
    ///
    /// Returns whether the wait completed rather than timed out.
    pub fn wait_until_idle(&self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        let mut guard = self.idle_lock.lock().unwrap_or_else(|e| e.into_inner());

        while self.live.load(Ordering::SeqCst) > 0 {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let (next, timed_out) = self
                .idle
                .wait_timeout(guard, remaining)
                .unwrap_or_else(|e| e.into_inner());
            guard = next;
            if timed_out.timed_out() && self.live.load(Ordering::SeqCst) > 0 {
                return false;
            }
        }
        true
    }
}
