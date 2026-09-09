//! Demo logger with idle-shrinking scratch buffers.
//!
//! `env_logger` formats each record into a thread-local buffer that is cleared
//! but never shrunk, so every thread that logs keeps capacity for the largest
//! record it ever formatted (its `Logger::log` source comment says exactly
//! this). The demo's QR-code burst grows that buffer to tens of KiB per logging
//! thread and the capacity then sits at length zero for the rest of the
//! process; heap profiling showed ~108 KiB held across two such buffers with
//! nothing in them, and `env_logger` offers no hook to release it.
//!
//! This logger keeps `RUST_LOG` filtering and caller-supplied format closures,
//! but a thread's scratch buffer shrinks back to the current record's size once
//! the thread has been quiet longer than [`set_idle_shrink_after`]. The only
//! cost is one realloc on the first log after the quiet period; a hot burst
//! reuses its buffer without reallocating.

use std::cell::RefCell;
use std::io::{self, Write};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use log::{LevelFilter, Metadata, Record, SetLoggerError};
use portable_atomic::{AtomicU64, Ordering};

/// Quiet period after which the next log on a thread releases that thread's
/// scratch capacity first. A minute covers the demo's shape: a large QR burst
/// at startup, then small records on an otherwise idle connection.
const DEFAULT_IDLE_SHRINK_AFTER: Duration = Duration::from_secs(60);

/// Idle threshold in nanoseconds; [`set_idle_shrink_after`] retunes it.
static IDLE_SHRINK_AFTER_NANOS: AtomicU64 =
    AtomicU64::new(DEFAULT_IDLE_SHRINK_AFTER.as_nanos() as u64);

/// Retune how long a thread must be quiet before its next log shrinks the
/// scratch buffer. The test suite sets this to zero to simulate the quiet
/// period deterministically.
pub fn set_idle_shrink_after(after: Duration) -> Duration {
    Duration::from_nanos(IDLE_SHRINK_AFTER_NANOS.swap(
        after.as_nanos().min(u64::MAX as u128) as u64,
        Ordering::Relaxed,
    ))
}

/// Current-thread scratch capacity in bytes. Zero when nothing was ever logged
/// here. Exposed so the footprint test can assert the buffers shrink.
pub fn buffer_capacity_bytes() -> usize {
    SCRATCH
        .try_with(|cell| {
            cell.try_borrow()
                .map(|scratch| scratch.as_ref().map_or(0, |s| s.buf.capacity()))
                .unwrap_or(0)
        })
        .unwrap_or(0)
}

struct Scratch {
    buf: Vec<u8>,
    last_use: wacore::time::Instant,
}

thread_local! {
    static SCRATCH: RefCell<Option<Scratch>> = const { RefCell::new(None) };
}

/// Sink for a formatted record. `Fn` rather than `Write` so the default path
/// locks stderr per record (no handle retained) while tests capture bytes.
type EmitFn = dyn Fn(&[u8]) + Send + Sync;

/// `std::io::Write` over the scratch buffer, handed to format closures. Shaped
/// like `env_logger::fmt::Formatter` so existing `.format(...)` closures move
/// over unchanged.
pub struct Formatter<'a> {
    buf: &'a mut Vec<u8>,
}

impl Write for Formatter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

type FormatFn = dyn Fn(&mut Formatter, &Record) -> io::Result<()> + Send + Sync;

fn default_format(buf: &mut Formatter, record: &Record) -> io::Result<()> {
    write!(buf, "[{:<5} {}] ", record.level(), record.target())?;
    // Continuation lines indent by four spaces, matching env_logger's default
    // format exactly. Only this default path does it; custom closures receive
    // the raw record, exactly as under env_logger.
    let args = record.args().to_string();
    let mut first = true;
    for chunk in args.split('\n') {
        if !first {
            write!(buf, "\n    ")?;
        }
        buf.write_all(chunk.as_bytes())?;
        first = false;
    }
    writeln!(buf)
}

#[derive(Debug, Clone)]
struct Directive {
    name: Option<String>,
    level: LevelFilter,
}

/// Parsed filter: target directives plus the optional `/substring` message
/// filter. Semantics mirror `env_filter` (the crate `env_logger` parses
/// through here): a bare word that is not a level means `name=trace`, matching
/// is a plain prefix so `Client` covers the crate's `Client/...` targets, the
/// longest name wins, and an unmatched target is disabled rather than falling
/// back to a global level.
#[derive(Debug, Clone)]
struct Filter {
    directives: Vec<Directive>,
    message: Option<String>,
}

impl Filter {
    fn max_level(&self) -> LevelFilter {
        self.directives
            .iter()
            .map(|d| d.level)
            .max()
            .unwrap_or(LevelFilter::Error)
    }

    fn enabled(&self, metadata: &Metadata) -> bool {
        let target = metadata.target();
        // Directives sort ascending by name length at build, so the reverse
        // scan meets the longest name first, with bare levels as the fallback.
        for directive in self.directives.iter().rev() {
            match &directive.name {
                Some(name) if !target.starts_with(name) => {}
                _ => return metadata.level() <= directive.level,
            }
        }
        false
    }

    fn matches(&self, record: &Record) -> bool {
        if !self.enabled(record.metadata()) {
            return false;
        }
        if let Some(needle) = &self.message
            && !record.args().to_string().contains(needle)
        {
            return false;
        }
        true
    }
}

fn parse_filter(spec: &str) -> Filter {
    let (mods, message) = match spec.split_once('/') {
        Some((mods, message)) => (mods, Some(message.to_string())),
        None => (spec, None),
    };
    let mut directives: Vec<Directive> = mods
        .split(',')
        .filter_map(|part| {
            let part = part.trim();
            if part.is_empty() {
                return None;
            }
            if let Some((name, level)) = part.split_once('=') {
                let name = name.trim();
                let level = level.trim();
                if level.is_empty() {
                    (!name.is_empty()).then(|| Directive {
                        name: Some(name.to_string()),
                        level: LevelFilter::max(),
                    })
                } else {
                    level.parse().ok().map(|level| Directive {
                        name: (!name.is_empty()).then(|| name.to_string()),
                        level,
                    })
                }
            } else {
                match part.parse() {
                    Ok(level) => Some(Directive { name: None, level }),
                    Err(_) => Some(Directive {
                        name: Some(part.to_string()),
                        level: LevelFilter::max(),
                    }),
                }
            }
        })
        .collect();
    // Ascending by name length (bare levels first); `enabled` scans in
    // reverse so the longest name wins. Stable, so equal lengths keep spec
    // order and the last one in the spec decides ties.
    directives.sort_by_key(|d| d.name.as_ref().map_or(0, String::len));
    Filter {
        directives,
        message,
    }
}

/// Which environment variable carries the filter, and what applies when it is
/// unset. Shaped like `env_logger::Env` so call sites read unchanged.
#[derive(Debug, Clone)]
pub struct Env<'a> {
    filter_var: &'a str,
    default_filter: Option<&'a str>,
}

impl<'a> Env<'a> {
    pub fn new() -> Self {
        Self {
            filter_var: "RUST_LOG",
            default_filter: None,
        }
    }

    pub fn filter(mut self, var: &'a str) -> Self {
        self.filter_var = var;
        self
    }

    pub fn filter_or(mut self, var: &'a str, default: &'a str) -> Self {
        self.filter_var = var;
        self.default_filter = Some(default);
        self
    }

    pub fn default_filter_or(mut self, default: &'a str) -> Self {
        self.default_filter = Some(default);
        self
    }

    fn resolve(&self) -> Filter {
        std::env::var(self.filter_var)
            .ok()
            .or_else(|| self.default_filter.map(str::to_string))
            .map(|spec| parse_filter(&spec))
            .unwrap_or(Filter {
                directives: Vec::new(),
                message: None,
            })
    }
}

impl Default for Env<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder shaped like `env_logger::Builder` for the calls this repo makes:
/// `from_env`, `format`, `filter`, `try_init`, `init`.
pub struct Builder {
    filter: Filter,
    format: Option<Box<FormatFn>>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            filter: Filter {
                directives: Vec::new(),
                message: None,
            },
            format: None,
        }
    }

    pub fn from_env<'a>(env: Env<'a>) -> Self {
        Self {
            filter: env.resolve(),
            format: None,
        }
    }

    pub fn format<F>(&mut self, format: F) -> &mut Self
    where
        F: Fn(&mut Formatter, &Record) -> io::Result<()> + Send + Sync + 'static,
    {
        self.format = Some(Box::new(format));
        self
    }

    pub fn filter(&mut self, module: Option<&str>, level: LevelFilter) -> &mut Self {
        self.filter.directives.push(Directive {
            name: module.map(str::to_string),
            level,
        });
        self
    }

    pub fn build(&mut self) -> Logger {
        let format: Arc<FormatFn> = match self.format.take() {
            Some(custom) => Arc::from(custom),
            None => Arc::new(default_format),
        };
        let mut filter = std::mem::replace(
            &mut self.filter,
            Filter {
                directives: Vec::new(),
                message: None,
            },
        );
        if filter.directives.is_empty() {
            // Matches `env_filter`: no directives at all means Error.
            filter.directives.push(Directive {
                name: None,
                level: LevelFilter::Error,
            });
        } else {
            filter
                .directives
                .sort_by_key(|d| d.name.as_ref().map_or(0, String::len));
        }
        Logger {
            filter,
            format,
            emit: Arc::new(emit_to_stderr),
        }
    }

    pub fn try_init(&mut self) -> Result<(), SetLoggerError> {
        let logger = self.build();
        let max = logger.filter.max_level();
        // `log` without its `alloc` feature (this workspace's case) has no
        // `set_boxed_logger`, so the installed logger lives in a `static`
        // slot. A rejected logger drops here instead of leaking, which a
        // `Box::leak`-before-`set_logger` sequence cannot do.
        match INSTALLED.set(logger) {
            Ok(()) => {
                let installed = INSTALLED.get().unwrap_or_else(|| {
                    panic!("logging::INSTALLED was just set, so it must read back")
                });
                log::set_logger(installed)?;
                log::set_max_level(max);
                Ok(())
            }
            Err(rejected) => {
                drop(rejected);
                let installed = INSTALLED
                    .get()
                    .unwrap_or_else(|| panic!("logging::INSTALLED is full, so it must read back"));
                // Complete the winner's install when it hasn't happened yet,
                // so a racy handoff can never park the process with no logger.
                // Then still report failure: this caller's own configuration
                // was discarded, mirroring env_logger's losing racer. The
                // second registration always fails — a global logger is
                // installed by then — and yields the error value.
                if log::set_logger(installed).is_ok() {
                    log::set_max_level(installed.filter.max_level());
                }
                let Err(e) = log::set_logger(installed) else {
                    unreachable!("a global logger is installed by now, so registration must fail")
                };
                Err(e)
            }
        }
    }

    pub fn init(&mut self) {
        if self.try_init().is_err() {
            panic!("logging::init called after another logger was initialized");
        }
    }
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

fn emit_to_stderr(bytes: &[u8]) {
    let stderr = io::stderr();
    let mut handle = stderr.lock();
    let _ = handle.write_all(bytes);
    let _ = handle.flush();
}

/// Slot for the installed logger. `OnceLock::set` hands a rejected logger
/// back for dropping, so failed `try_init` calls leak nothing, and a racy
/// loser installs the slot value rather than failing spuriously.
static INSTALLED: OnceLock<Logger> = OnceLock::new();

/// Logger installed by [`Builder`]. Stateless across records except for the
/// calling thread's scratch buffer, which shrinks after the idle threshold.
#[derive(Clone)]
pub struct Logger {
    filter: Filter,
    format: Arc<FormatFn>,
    emit: Arc<EmitFn>,
}

impl Logger {
    fn log_inner(&self, record: &Record) {
        let idle_after = Duration::from_nanos(IDLE_SHRINK_AFTER_NANOS.load(Ordering::Relaxed));
        let printed = SCRATCH
            .try_with(|cell| {
                if let Ok(mut guard) = cell.try_borrow_mut() {
                    let scratch = guard.get_or_insert_with(|| Scratch {
                        buf: Vec::new(),
                        last_use: wacore::time::Instant::now(),
                    });
                    // Establish the empty-buffer invariant on entry: a panic
                    // inside a format closure or sink would otherwise leave
                    // stale bytes for the next record to append to.
                    scratch.buf.clear();
                    if scratch.last_use.elapsed() >= idle_after {
                        scratch.buf.shrink_to_fit();
                    }
                    let mut formatter = Formatter {
                        buf: &mut scratch.buf,
                    };
                    let _ = (self.format)(&mut formatter, record);
                    (self.emit)(formatter.buf);
                    scratch.buf.clear();
                    scratch.last_use = wacore::time::Instant::now();
                } else {
                    // Re-entrant log from inside a format closure: the scratch
                    // buffer is already borrowed, so format on the stack.
                    let mut owned = Vec::new();
                    let mut formatter = Formatter { buf: &mut owned };
                    let _ = (self.format)(&mut formatter, record);
                    (self.emit)(formatter.buf);
                }
            })
            .is_ok();
        if !printed {
            // Thread-local storage is gone (thread shutdown): single-use stack
            // buffer, mirroring env_logger's fallback.
            let mut owned = Vec::new();
            let mut formatter = Formatter { buf: &mut owned };
            let _ = (self.format)(&mut formatter, record);
            (self.emit)(formatter.buf);
        }
    }
}

impl log::Log for Logger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        self.filter.enabled(metadata)
    }

    fn log(&self, record: &Record) {
        if self.filter.matches(record) {
            self.log_inner(record);
        }
    }

    fn flush(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use log::Log as _;
    use std::fmt::Arguments;
    use std::sync::Mutex;

    struct IdleGuard {
        previous: Duration,
    }

    impl IdleGuard {
        fn set(after: Duration) -> Self {
            Self {
                previous: set_idle_shrink_after(after),
            }
        }
    }

    impl Drop for IdleGuard {
        fn drop(&mut self) {
            set_idle_shrink_after(self.previous);
        }
    }

    fn capture_logger(spec: &str) -> (Logger, Arc<Mutex<Vec<u8>>>) {
        let captured: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&captured);
        // Parse the spec directly instead of routing through `Env`: an
        // ambient `RUST_LOG` on the test machine must not override the case
        // under test. `Env` precedence has its own test below.
        let mut builder = Builder::new();
        builder.filter = parse_filter(spec);
        let mut logger = builder.build();
        logger.emit = Arc::new(move |bytes: &[u8]| {
            sink.lock()
                .expect("capture lock is never held across a log call")
                .extend_from_slice(bytes);
        });
        (logger, captured)
    }

    fn record(args: Arguments<'_>) -> Record<'_> {
        Record::builder()
            .args(args)
            .level(log::Level::Info)
            .target("wr_r3_probe")
            .build()
    }

    #[test]
    fn scratch_buffer_shrinks_after_idle() {
        let _idle = IdleGuard::set(Duration::from_secs(3600));
        let (logger, _) = capture_logger("info");

        let big = "x".repeat(64 * 1024);
        logger.log(&record(format_args!("{}", big)));
        let grown = buffer_capacity_bytes();
        assert!(
            grown >= big.len(),
            "scratch must hold the large record: capacity {grown}, record {}",
            big.len()
        );

        // Simulate the quiet period expiring, then log small: the historic
        // capacity is released first, so the buffer only regrows to this
        // record's size. The realloc for that regrow is the whole cost.
        set_idle_shrink_after(Duration::ZERO);
        logger.log(&record(format_args!("ping")));
        let shrunk = buffer_capacity_bytes();
        assert!(
            shrunk * 16 < grown,
            "capacity must collapse after idle: {grown} -> {shrunk}"
        );
    }

    #[test]
    fn log_output_survives_the_shrink() {
        // No idle-guard here on purpose: `scratch_buffer_shrinks_after_idle`
        // is the single test that retunes the process-global threshold, so
        // parallel tests cannot race its restore against a probe log.
        let (logger, captured) = capture_logger("info");

        logger.log(&record(format_args!("hello-after-idle")));
        let bytes = captured.lock().expect("test holds no other lock").clone();
        let text = String::from_utf8(bytes).expect("logger emits UTF-8");
        assert!(
            text.contains("hello-after-idle") && text.contains("wr_r3_probe"),
            "record must still be emitted, got: {text:?}"
        );
    }

    #[test]
    fn filter_directives_match_env_logger_shape() {
        let (logger, captured) = capture_logger("info,webrtc_sctp=error,webrtc_dtls=error");

        fn probe(target: &str, level: log::Level) -> Metadata<'_> {
            Metadata::builder().level(level).target(target).build()
        }
        assert!(logger.enabled(&probe("app", log::Level::Info)));
        assert!(!logger.enabled(&probe("app", log::Level::Debug)));
        assert!(!logger.enabled(&probe("webrtc_sctp", log::Level::Warn)));
        assert!(logger.enabled(&probe("webrtc_sctp", log::Level::Error)));
        assert!(logger.enabled(&probe("webrtc_sctp::conn", log::Level::Error)));

        // A bare word is a target directive at maximum level, not a level:
        // `RUST_LOG=webrtc_sctp` enables everything under that target.
        let (bare, _) = capture_logger("webrtc_sctp");
        assert!(bare.enabled(&probe("webrtc_sctp", log::Level::Trace)));
        assert!(bare.enabled(&probe("webrtc_sctp::conn", log::Level::Debug)));
        assert!(!bare.enabled(&probe("app", log::Level::Error)));

        // Prefix matching is plain, so `Client` covers the crate's
        // `Client/...` custom targets.
        let (slash, _) = capture_logger("Client=debug");
        assert!(slash.enabled(&probe("Client/AppState", log::Level::Debug)));
        assert!(!slash.enabled(&probe("Client/AppState", log::Level::Trace)));
        assert!(!slash.enabled(&probe("app", log::Level::Error)));

        // Longest name wins over the bare fallback.
        let (longest, _) = capture_logger("info,whatsapp_rust=debug");
        assert!(longest.enabled(&probe("whatsapp_rust::x", log::Level::Debug)));
        assert!(!longest.enabled(&probe("app", log::Level::Debug)));

        // Disabled records never reach the sink.
        logger.log(
            &Record::builder()
                .args(format_args!("m"))
                .level(log::Level::Debug)
                .target("app")
                .build(),
        );
        assert!(
            captured
                .lock()
                .expect("test holds no other lock")
                .is_empty()
        );
    }

    #[test]
    fn default_format_matches_env_logger_lines() {
        let (logger, captured) = capture_logger("info");
        logger.log(&record(format_args!("one line")));
        let bytes = captured.lock().expect("test holds no other lock").clone();
        let text = String::from_utf8(bytes).expect("logger emits UTF-8");
        assert!(
            text.ends_with('\n'),
            "record must end the line, got: {text:?}"
        );
        assert!(text.contains("INFO") && text.contains("one line"));

        // Continuation lines indent by four spaces, as in env_logger's
        // default format.
        captured.lock().expect("test holds no other lock").clear();
        logger.log(&record(format_args!("first\nsecond")));
        let bytes = captured.lock().expect("test holds no other lock").clone();
        let text = String::from_utf8(bytes).expect("logger emits UTF-8");
        assert!(
            text.ends_with("first\n    second\n"),
            "continuation must indent, got: {text:?}"
        );
    }

    #[test]
    fn slash_message_filter_screens_records() {
        let (logger, captured) = capture_logger("info/needle");
        logger.log(&record(format_args!("find the needle here")));
        logger.log(&record(format_args!("nothing relevant")));
        let bytes = captured.lock().expect("test holds no other lock").clone();
        let text = String::from_utf8(bytes).expect("logger emits UTF-8");
        assert!(
            text.contains("needle here") && !text.contains("nothing relevant"),
            "only matching records pass, got: {text:?}"
        );
    }

    #[test]
    fn max_level_covers_configured_directives() {
        let mut named = Builder::new();
        named.filter = parse_filter("foo=off");
        assert_eq!(named.build().filter.max_level(), LevelFilter::Off);
        let mut global = Builder::new();
        global.filter = parse_filter("info,foo=off");
        assert_eq!(global.build().filter.max_level(), LevelFilter::Info);
    }

    #[test]
    fn env_variable_overrides_the_default() {
        // Unique variable name so no other test or ambient environment can
        // observe it; the set/remove pair is scoped to this test.
        const VAR: &str = "WR_R3_LOGGING_TEST_FILTER";
        unsafe {
            std::env::remove_var(VAR);
        }
        let unset = Env::default().filter_or(VAR, "info").resolve();
        assert!(
            unset.enabled(
                &Metadata::builder()
                    .level(log::Level::Info)
                    .target("app")
                    .build()
            )
        );
        unsafe {
            std::env::set_var(VAR, "off");
        }
        let set = Env::default().filter_or(VAR, "info").resolve();
        assert!(
            !set.enabled(
                &Metadata::builder()
                    .level(log::Level::Error)
                    .target("app")
                    .build()
            )
        );
        unsafe {
            std::env::remove_var(VAR);
        }
    }

    #[test]
    fn repeated_try_init_fails_without_leaking() {
        let previous = log::max_level();
        let mut first = Builder::from_env(Env::default().default_filter_or("info"));
        let _ = first.try_init();
        let mut second = Builder::from_env(Env::default().default_filter_or("info"));
        // The slot is full after the first call (whether or not a foreign
        // logger beat it to the global install), so the second call always
        // fails — and the rejected logger drops instead of leaking.
        assert!(second.try_init().is_err());
        log::set_max_level(previous);
    }
}
