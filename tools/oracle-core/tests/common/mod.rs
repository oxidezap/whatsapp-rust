//! Serialising the tests that bring up a VoIP engine.
//!
//! Each test binary already serialises its own with a `Mutex`, which is not
//! enough: cargo runs the binaries themselves in parallel, so `threading` and
//! `host_environment` can each have a PJSIP worker pool running at the same
//! time. Several pools competing for cores miss their own deadlines, and that
//! surfaces as an unrelated-looking failure — `initVoipStack` trapping in a
//! test that is not about startup at all.
//!
//! The lock is a TCP bind on a fixed loopback port. A file would need cleaning
//! up after a killed process; a port is released by the OS when the process
//! ends, whatever killed it, so a crashed test cannot wedge every later run.

// Each integration-test binary compiles this module independently. Some need
// only capture loading while the engine suites also need the cross-process lock.
#![allow(dead_code)]

use std::net::TcpListener;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use oracle_core::Catalog;

pub mod evidence;

/// Chosen from the IANA dynamic range and otherwise arbitrary. Collides only
/// with something else that picked the same number, which would show up as
/// these tests serialising against it rather than as a wrong result.
const LOCK_PORT: u16 = 47_921;

/// How long to wait for another test binary to finish with its engine. Long
/// enough for the slowest engine test, so a real queue is not mistaken for a
/// deadlock.
const WAIT: Duration = Duration::from_secs(600);

/// Held for as long as a test needs exclusive use of a VoIP engine.
pub struct EngineLock(#[allow(dead_code)] TcpListener);

/// Blocks until no other test process is running an engine.
///
/// Failure to acquire exclusivity is an infrastructure failure, never permission
/// to run another engine concurrently.
pub fn engine_lock() -> EngineLock {
    acquire_engine_lock(LOCK_PORT, WAIT).unwrap_or_else(|error| {
        panic!("oracle infrastructure failure: acquiring engine lock: {error}")
    })
}

/// Acquire an engine port, failing closed when its deadline expires.
pub fn acquire_engine_lock(port: u16, wait: Duration) -> std::io::Result<EngineLock> {
    let deadline = Instant::now() + wait;

    loop {
        match TcpListener::bind(("127.0.0.1", port)) {
            Ok(listener) => return Ok(EngineLock(listener)),
            Err(error)
                if error.kind() == std::io::ErrorKind::AddrInUse && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => {
                return Err(error);
            }
        }
    }
}

/// Discover captures while treating only an absent implicit cache as skippable.
pub fn catalog() -> Result<Option<Catalog>> {
    match Catalog::discover() {
        Ok(catalog) => Ok(Some(catalog)),
        Err(error)
            if std::env::var_os(oracle_core::catalog::DIR_ENV).is_none()
                && error
                    .to_string()
                    .starts_with("no directory holding captured .wasm files") =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}

/// Read one verified capture, or `None` only when no implicit capture cache exists.
pub fn capture(id: &str) -> Result<Option<Vec<u8>>> {
    let Some(catalog) = catalog()? else {
        return Ok(None);
    };
    let entry = catalog.resolve(id)?;
    oracle_core::catalog::read_pinned_capture(id, &entry.path)
        .with_context(|| format!("reading capture {}", entry.path.display()))
        .map(Some)
}

/// Holds both locks. The local mutex protects no state; recovering it after a
/// test panic is safe because each scenario constructs and drops its own engine.
pub fn threaded_guard() -> (std::sync::MutexGuard<'static, ()>, EngineLock) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let local = LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    (local, engine_lock())
}
