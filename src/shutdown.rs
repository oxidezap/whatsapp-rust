//! Process shutdown-signal handling for the bundled binaries.
//!
//! Container/service supervisors stop a process by sending **SIGTERM** and only
//! escalate to an unblockable SIGKILL after a grace period (`docker stop`
//! defaults to 10s). Watching only SIGINT (`Ctrl+C`) means SIGTERM goes
//! unhandled, so the process burns the whole grace period and is hard-killed —
//! skipping the flush/disconnect that graceful shutdown performs.
//!
//! This bites hardest under this project's Docker image: the static binary is
//! the `scratch` entrypoint, so it runs as **PID 1**, and the kernel drops any
//! signal for which PID 1 has no handler installed. An unhandled SIGTERM is
//! therefore silently ignored rather than terminating the process, guaranteeing
//! the 10s timeout on every `docker stop`.
//!
//! [`shutdown_signal`] resolves on the first of SIGINT or SIGTERM (Ctrl+C only
//! on non-Unix, the sole portable stop signal Tokio exposes there), so wiring it
//! into a `tokio::select!` is enough to make `docker stop` graceful.

/// Resolves when the process is asked to stop (SIGINT or SIGTERM on Unix,
/// Ctrl+C elsewhere).
///
/// Both signal handlers are installed synchronously before the returned future
/// first suspends, so a signal that arrives afterwards is delivered rather than
/// lost. Compose it with the run loop:
///
/// ```no_run
/// # async fn f(mut handle: whatsapp_rust::bot::BotHandle) {
/// tokio::select! {
///     _ = &mut handle => {}
///     _ = whatsapp_rust::shutdown_signal() => handle.shutdown().await,
/// }
/// # }
/// ```
#[cfg(unix)]
pub async fn shutdown_signal() {
    use tokio::signal::unix::{Signal, SignalKind, signal};

    fn install(kind: SignalKind, name: &str) -> Option<Signal> {
        match signal(kind) {
            Ok(sig) => Some(sig),
            // Realistically unreachable for SIGINT/SIGTERM (only uncatchable
            // signals fail to register). Degrade to whichever handler did
            // install instead of panicking in a top-level shutdown path.
            Err(e) => {
                log::error!("shutdown_signal: failed to install {name} handler: {e}");
                None
            }
        }
    }

    // Registered up front (before the first await) so both are armed by the
    // time either can fire.
    let mut sigint = install(SignalKind::interrupt(), "SIGINT");
    let mut sigterm = install(SignalKind::terminate(), "SIGTERM");

    async fn wait(sig: &mut Option<Signal>) {
        match sig {
            Some(sig) => {
                sig.recv().await;
            }
            None => std::future::pending::<()>().await,
        }
    }

    tokio::select! {
        _ = wait(&mut sigint) => {}
        _ = wait(&mut sigterm) => {}
    }
}

/// Non-Unix fallback: Ctrl+C is the only portable stop signal Tokio exposes.
#[cfg(not(unix))]
pub async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
