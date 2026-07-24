//! Regression guard for graceful shutdown under `docker stop` / k8s / systemd.
//!
//! Those supervisors stop a process with **SIGTERM** and only escalate to
//! SIGKILL after a grace period. The bundled binaries once watched only SIGINT
//! (`Ctrl+C`), so SIGTERM went unhandled and every `docker stop` timed out and
//! hard-killed the container (as PID 1 in a `scratch` image the kernel drops an
//! unhandled SIGTERM entirely). [`whatsapp_rust::shutdown_signal`] must resolve
//! on SIGTERM as well as SIGINT.
//!
//! All scenarios live in one `#[tokio::test]` on purpose: signals are
//! process-global, so running signal-raising cases in parallel (the default
//! across test fns) would let one case's signal wake another's future. Keeping
//! them sequential makes delivery deterministic.
#![cfg(all(feature = "signal", unix))]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::signal::unix::{SignalKind, signal};

/// Polls a pinned future once with a no-op waker and asserts it has not resolved.
fn assert_pending<F: Future + ?Sized>(mut fut: Pin<&mut F>, msg: &str) {
    let waker = futures::task::noop_waker();
    let mut cx = Context::from_waker(&waker);
    assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending), "{msg}");
}

/// Post `sig` to the current process. A Tokio handler for it is always installed
/// first (see the safety nets in the test), so the signal becomes an async
/// notification instead of the default process-terminating disposition.
fn raise(sig: libc::c_int) {
    // SAFETY: `raise` only posts a signal to the calling process; it has no
    // memory-safety preconditions.
    let rc = unsafe { libc::raise(sig) };
    assert_eq!(rc, 0, "raise({sig}) failed");
}

#[tokio::test]
async fn shutdown_signal_resolves_on_sigint_and_sigterm() {
    // Safety nets: install handlers for both signals up front. Two purposes:
    //  1. A raised signal can never fall through to the default disposition
    //     (which would terminate this test process) even if the code under test
    //     regresses and stops watching one of them — a regression then surfaces
    //     as a clean timeout, not a killed test binary.
    //  2. `recv()` on a net confirms a signal was actually *delivered* before we
    //     assert anything about it.
    let _sigint_net = signal(SignalKind::interrupt()).expect("install SIGINT net");
    let mut sigterm_net = signal(SignalKind::terminate()).expect("install SIGTERM net");

    // --- Reproduction of the original bug: a Ctrl+C-only future (the pre-fix
    //     demo) ignores SIGTERM, which is exactly what `docker stop` sends. ---
    {
        let mut ctrlc = Box::pin(tokio::signal::ctrl_c());
        // First poll registers ctrl_c's SIGINT handler; it must be pending.
        assert_pending(ctrlc.as_mut(), "ctrl_c() resolved before any signal");
        raise(libc::SIGTERM);
        // Block until the SIGTERM is delivered, so the assertion below reflects a
        // *delivered* signal rather than one still in flight.
        sigterm_net.recv().await;
        assert_pending(
            ctrlc.as_mut(),
            "ctrl_c() resolved on SIGTERM — it must only react to SIGINT (this is the bug)",
        );
    }

    // --- The fix: shutdown_signal() resolves on SIGTERM. ---
    {
        let mut fut = Box::pin(whatsapp_rust::shutdown_signal());
        // First poll installs both SIGINT and SIGTERM handlers.
        assert_pending(fut.as_mut(), "shutdown_signal() resolved before any signal");
        raise(libc::SIGTERM);
        tokio::time::timeout(Duration::from_secs(5), fut)
            .await
            .expect("shutdown_signal() must resolve on SIGTERM (docker stop / k8s / systemd)");
    }

    // --- The fix keeps the original behaviour: it still resolves on SIGINT. ---
    {
        let mut fut = Box::pin(whatsapp_rust::shutdown_signal());
        assert_pending(fut.as_mut(), "shutdown_signal() resolved before any signal");
        raise(libc::SIGINT);
        tokio::time::timeout(Duration::from_secs(5), fut)
            .await
            .expect("shutdown_signal() must resolve on SIGINT (Ctrl+C)");
    }
}
