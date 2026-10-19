//! Deterministic regressions for failures that do not need a vendor capture.
mod common;

use common::evidence::{Failure, check_log};

#[test]
fn a_busy_engine_port_never_allows_an_unserialized_scenario() {
    use std::net::TcpListener;
    use std::time::Duration;

    let occupied = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = occupied.local_addr().unwrap().port();
    let error = common::acquire_engine_lock(port, Duration::ZERO)
        .err()
        .expect("a busy engine port must prevent the scenario from running");
    assert_eq!(error.kind(), std::io::ErrorKind::AddrInUse);
    drop(occupied);
    let _next = common::acquire_engine_lock(port, Duration::ZERO).unwrap();
}

#[test]
fn a_panicking_scenario_does_not_poison_the_next_engine_guard() {
    let failed = std::panic::catch_unwind(|| {
        let _guard = common::threaded_guard();
        panic!("deliberate scenario mismatch");
    });
    assert!(failed.is_err());
    let _next = common::threaded_guard();
}

#[test]
fn incomplete_evidence_never_reaches_the_verdict() {
    for (overflow, coherent, settled, expected) in [
        (true, Some(true), true, Failure::Overflow),
        (false, Some(false), true, Failure::Infrastructure),
        (false, None, true, Failure::Infrastructure),
        (false, Some(true), false, Failure::Infrastructure),
    ] {
        let mut judged = false;
        let result = check_log(overflow, coherent, settled).map(|()| judged = true);
        assert_eq!(result, Err(expected));
        assert!(!judged);
    }
    assert_eq!(check_log(false, Some(true), true), Ok(()));
}
