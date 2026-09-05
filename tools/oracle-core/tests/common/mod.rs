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

use std::net::TcpListener;
use std::time::{Duration, Instant};

/// Chosen from the IANA dynamic range and otherwise arbitrary. Collides only
/// with something else that picked the same number, which would show up as
/// these tests serialising against it rather than as a wrong result.
const LOCK_PORT: u16 = 47_921;

/// How long to wait for another test binary to finish with its engine. Long
/// enough for the slowest engine test, so a real queue is not mistaken for a
/// deadlock.
const WAIT: Duration = Duration::from_secs(600);

/// Held for as long as a test needs exclusive use of a VoIP engine.
pub struct EngineLock(#[allow(dead_code)] Option<TcpListener>);

/// Blocks until no other test process is running an engine.
///
/// Falls through rather than failing if the wait runs out: a test that then
/// contends is a flaky test, but a test suite that refuses to run is worse, and
/// the timeout is far longer than any legitimate hold.
pub fn engine_lock() -> EngineLock {
    let deadline = Instant::now() + WAIT;

    loop {
        match TcpListener::bind(("127.0.0.1", LOCK_PORT)) {
            Ok(listener) => return EngineLock(Some(listener)),
            Err(_) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(error) => {
                eprintln!("engine lock timed out ({error}); running unserialised");
                return EngineLock(None);
            }
        }
    }
}
