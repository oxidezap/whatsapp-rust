//! Graceful-shutdown signal for the bundled binaries.
//!
//! Watches SIGINT *and* SIGTERM (Ctrl+C only on non-Unix): supervisors like
//! `docker stop` send SIGTERM, and as PID 1 in a `scratch` image the kernel
//! silently drops any signal without an installed handler — so a `ctrl_c()`-only
//! wait never runs cleanup and is SIGKILLed after the stop timeout.

/// Resolves on the first stop signal: SIGINT or SIGTERM on Unix, Ctrl+C
/// elsewhere. Both handlers are armed before the future first suspends, so a
/// signal arriving afterwards is delivered, not missed.
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

    // Realistically unreachable for SIGINT/SIGTERM (only uncatchable signals
    // fail to register); degrade to whichever installs instead of panicking in
    // a top-level shutdown path.
    fn install(kind: SignalKind, name: &str) -> Option<Signal> {
        signal(kind)
            .inspect_err(|e| log::error!("shutdown_signal: cannot watch {name}: {e}"))
            .ok()
    }

    // Registered up front (before the first await) so both are armed by the
    // time either can fire.
    let mut sigint = install(SignalKind::interrupt(), "SIGINT");
    let mut sigterm = install(SignalKind::terminate(), "SIGTERM");

    if sigint.is_none() && sigterm.is_none() {
        // Neither installed: the future below can never resolve, so graceful
        // stop is off. Say so loudly rather than parking silently forever.
        log::error!(
            "shutdown_signal: no stop signal could be installed; graceful shutdown disabled"
        );
    }

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
