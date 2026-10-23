use super::*;
use anyhow::bail;
use portable_atomic::AtomicU64;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

#[derive(Default)]
struct Fake {
    epoch: AtomicU64,
    ready: AtomicBool,
    allowed: AtomicBool,
    fail_available: AtomicBool,
    fail_unavailable: AtomicBool,
    hang_available: AtomicBool,
    fail_load: AtomicBool,
    fail_save: AtomicBool,
    recovered: AtomicBool,
    saved: Mutex<Option<u64>>,
    writes: Mutex<Vec<&'static str>>,
    notices: Mutex<Vec<Notice>>,
}
impl Fake {
    fn connected() -> Arc<Self> {
        Arc::new(Self {
            epoch: AtomicU64::new(1),
            ready: AtomicBool::new(true),
            allowed: AtomicBool::new(true),
            ..Self::default()
        })
    }
}
impl Host for Arc<Fake> {
    fn generation(&self) -> Option<u64> {
        if !self.ready.load(Ordering::SeqCst) {
            return None;
        }
        match self.epoch.load(Ordering::SeqCst) {
            0 => None,
            n => Some(n),
        }
    }
    async fn can_pulse(&self) -> bool {
        self.allowed.load(Ordering::SeqCst)
    }
    async fn set_available(&self) -> Result<()> {
        self.writes.lock().unwrap().push("available");
        if self.hang_available.load(Ordering::SeqCst) {
            std::future::pending::<()>().await;
        }
        if self.fail_available.load(Ordering::SeqCst) {
            bail!("synthetic failure");
        }
        Ok(())
    }
    async fn set_unavailable(&self) -> Result<()> {
        self.writes.lock().unwrap().push("unavailable");
        if self.fail_unavailable.load(Ordering::SeqCst) {
            bail!("synthetic failure");
        }
        Ok(())
    }
    async fn load_success(&self) -> Result<Option<u64>> {
        if self.fail_load.load(Ordering::SeqCst) {
            bail!("synthetic failure");
        }
        Ok(*self.saved.lock().unwrap())
    }
    async fn save_success(&self, value: u64) -> Result<()> {
        if self.fail_save.load(Ordering::SeqCst) {
            bail!("synthetic failure");
        }
        *self.saved.lock().unwrap() = Some(value);
        Ok(())
    }
    fn notice(&self, value: Notice) {
        self.notices.lock().unwrap().push(value);
    }
    fn recover_transport(&self) {
        self.recovered.store(true, Ordering::SeqCst);
    }
}
fn settings() -> Settings {
    Settings {
        write_timeout: Duration::from_millis(20),
        poll_every: Duration::from_millis(1),
        ..Settings::default()
    }
}
async fn started(host: &Arc<Fake>) -> Maintenance {
    let mut state = Maintenance::new(settings());
    state.tick(host, 100).await;
    state.tick(host, 100).await;
    assert!(state.pulse.is_some());
    state
}
fn finish_interval(state: &mut Maintenance) {
    state.pulse.as_mut().unwrap().1 = Instant::now();
}

#[tokio::test]
async fn restores_first_then_pulses_once_and_saves_only_after_unavailable() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    for _ in 0..10 {
        state.tick(&host, 101).await;
    }
    assert_eq!(*host.writes.lock().unwrap(), ["unavailable", "available"]);
    assert_eq!(*host.saved.lock().unwrap(), None);
    finish_interval(&mut state);
    state.tick(&host, 200).await;
    assert_eq!(
        *host.writes.lock().unwrap(),
        ["unavailable", "available", "unavailable"]
    );
    assert_eq!(*host.saved.lock().unwrap(), Some(200));
}
#[tokio::test]
async fn successful_checkpoint_survives_restart_and_exactly_24h_becomes_due() {
    let host = Fake::connected();
    *host.saved.lock().unwrap() = Some(100);
    let mut state = Maintenance::new(settings());
    state.tick(&host, 200).await;
    state.tick(&host, 100 + 86_400_000 - 1).await;
    assert_eq!(*host.writes.lock().unwrap(), ["unavailable"]);
    state.tick(&host, 100 + 86_400_000).await;
    assert!(state.pulse.is_some());
}
#[tokio::test]
async fn disconnected_or_unowned_session_never_announces_available() {
    let host = Fake::connected();
    host.epoch.store(0, Ordering::SeqCst);
    let mut state = Maintenance::new(settings());
    state.tick(&host, 100).await;
    assert!(host.writes.lock().unwrap().is_empty());
    host.epoch.store(1, Ordering::SeqCst);
    host.allowed.store(false, Ordering::SeqCst);
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    assert_eq!(*host.writes.lock().unwrap(), ["unavailable"]);
}
#[tokio::test]
async fn reconnect_between_ticks_invalidates_pulse_even_when_socket_is_live() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    finish_interval(&mut state);
    host.epoch.store(2, Ordering::SeqCst);
    state.tick(&host, 200).await;
    assert_eq!(*host.saved.lock().unwrap(), None);
    assert!(host.notices.lock().unwrap().contains(&Notice::Interrupted));
    assert_eq!(host.writes.lock().unwrap().last(), Some(&"unavailable"));
}
#[tokio::test]
async fn disconnect_during_pulse_is_not_recorded_and_reconnect_can_retry() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    host.epoch.store(0, Ordering::SeqCst);
    state.tick(&host, 200).await;
    assert!(state.pulse.is_none());
    assert_eq!(*host.saved.lock().unwrap(), None);
    host.epoch.store(2, Ordering::SeqCst);
    state.tick(&host, 300).await;
    state.retry_at = Instant::now();
    state.tick(&host, 300).await;
    assert!(state.pulse.is_some());
}
#[tokio::test]
async fn failed_available_is_compensated_without_recording_success_or_immediate_retry() {
    let host = Fake::connected();
    host.fail_available.store(true, Ordering::SeqCst);
    let mut state = Maintenance::new(settings());
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    assert_eq!(
        host.writes
            .lock()
            .unwrap()
            .iter()
            .filter(|v| **v == "available")
            .count(),
        1
    );
    assert_eq!(host.writes.lock().unwrap().last(), Some(&"unavailable"));
    assert_eq!(*host.saved.lock().unwrap(), None);
}
#[tokio::test]
async fn uncertain_available_timeout_is_also_compensated() {
    let host = Fake::connected();
    host.hang_available.store(true, Ordering::SeqCst);
    let mut state = Maintenance::new(settings());
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    assert_eq!(
        *host.writes.lock().unwrap(),
        ["unavailable", "available", "unavailable"]
    );
    assert_eq!(*host.saved.lock().unwrap(), None);
}
#[tokio::test]
async fn failed_restoration_retries_offline_and_requests_recovery_after_three_failures() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    finish_interval(&mut state);
    host.fail_unavailable.store(true, Ordering::SeqCst);
    for _ in 0..3 {
        state.tick(&host, 200).await;
    }
    assert_eq!(*host.saved.lock().unwrap(), None);
    assert!(host.recovered.load(Ordering::SeqCst));
    host.fail_unavailable.store(false, Ordering::SeqCst);
    state.tick(&host, 300).await;
    assert_eq!(
        *host.saved.lock().unwrap(),
        None,
        "recovery is not a completed pulse"
    );
    assert_eq!(state.restore_failures, 0);
}
#[tokio::test]
async fn empty_pushname_during_initial_sync_does_not_force_reconnect_loop() {
    let host = Fake::connected();
    host.fail_unavailable.store(true, Ordering::SeqCst);
    let mut state = Maintenance::new(settings());
    for _ in 0..5 {
        state.tick(&host, 100).await;
    }
    assert!(!host.recovered.load(Ordering::SeqCst));
    assert!(state.pulse.is_none());
}
#[tokio::test]
async fn corrupt_checkpoint_fails_closed_but_still_restores_offline() {
    let host = Fake::connected();
    host.fail_load.store(true, Ordering::SeqCst);
    let mut state = Maintenance::new(settings());
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    assert_eq!(*host.writes.lock().unwrap(), ["unavailable"]);
}
#[tokio::test]
async fn failed_checkpoint_write_retries_only_storage_without_another_pulse() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    finish_interval(&mut state);
    host.fail_save.store(true, Ordering::SeqCst);
    state.tick(&host, 200).await;
    assert_eq!(*host.saved.lock().unwrap(), None);
    host.fail_save.store(false, Ordering::SeqCst);
    state.tick(&host, 300).await;
    assert_eq!(*host.saved.lock().unwrap(), Some(200));
    assert_eq!(
        host.writes
            .lock()
            .unwrap()
            .iter()
            .filter(|v| **v == "available")
            .count(),
        1
    );
}
#[tokio::test]
async fn reconnect_login_without_session_readiness_cannot_complete_old_pulse() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    finish_interval(&mut state);
    // Socket/login are back, but the new Connected event has not advanced epoch.
    host.ready.store(false, Ordering::SeqCst);
    state.tick(&host, 200).await;
    assert_eq!(*host.saved.lock().unwrap(), None);
    assert!(state.pulse.is_none());
    host.epoch.store(2, Ordering::SeqCst);
    host.ready.store(true, Ordering::SeqCst);
    state.tick(&host, 300).await;
    assert_eq!(host.writes.lock().unwrap().last(), Some(&"unavailable"));
    assert_eq!(*host.saved.lock().unwrap(), None);
}

#[tokio::test]
async fn completed_checkpoint_retries_during_network_outage() {
    let host = Fake::connected();
    let mut state = started(&host).await;
    finish_interval(&mut state);
    host.fail_save.store(true, Ordering::SeqCst);
    state.tick(&host, 200).await;
    host.epoch.store(0, Ordering::SeqCst);
    host.fail_save.store(false, Ordering::SeqCst);
    state.tick(&host, 300).await;
    assert_eq!(*host.saved.lock().unwrap(), Some(200));
    assert!(host.notices.lock().unwrap().contains(&Notice::Completed));
}

#[tokio::test]
async fn graceful_stop_drains_completed_checkpoint_while_offline() {
    let host = Fake::connected();
    host.fail_save.store(true, Ordering::SeqCst);
    let (tx, rx) = watch::channel(false);
    let task = tokio::spawn(run(
        host.clone(),
        Settings {
            available_for: Duration::ZERO,
            poll_every: Duration::from_millis(100),
            ..settings()
        },
        rx,
    ));
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if host
                .notices
                .lock()
                .unwrap()
                .contains(&Notice::CheckpointFailed)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    host.epoch.store(0, Ordering::SeqCst);
    host.fail_save.store(false, Ordering::SeqCst);
    tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert!(host.saved.lock().unwrap().is_some());
    assert!(host.notices.lock().unwrap().contains(&Notice::Completed));
}

#[tokio::test]
async fn disabled_pulses_still_restore_unavailable_on_reconnect() {
    let host = Fake::connected();
    let mut state = Maintenance::new(Settings {
        enabled: false,
        ..settings()
    });
    state.tick(&host, 100).await;
    state.tick(&host, 100).await;
    host.epoch.store(2, Ordering::SeqCst);
    state.tick(&host, 200).await;
    assert_eq!(*host.writes.lock().unwrap(), ["unavailable", "unavailable"]);
}
#[tokio::test]
async fn clock_rollback_does_not_admit_extra_pulse() {
    let host = Fake::connected();
    *host.saved.lock().unwrap() = Some(10_000);
    let mut state = Maintenance::new(settings());
    state.tick(&host, 10).await;
    state.tick(&host, 10).await;
    assert!(state.pulse.is_none());
}
#[tokio::test]
async fn graceful_stop_cleans_up_without_waiting_45_seconds() {
    let host = Fake::connected();
    let (tx, rx) = watch::channel(false);
    let task = tokio::spawn(run(host.clone(), settings(), rx));
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if host.notices.lock().unwrap().contains(&Notice::Available) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(host.writes.lock().unwrap().last(), Some(&"unavailable"));
    assert_eq!(*host.saved.lock().unwrap(), None);
}
#[tokio::test]
async fn dropped_stop_sender_and_preexisting_stop_both_cleanup() {
    for drop_sender in [true, false] {
        let host = Fake::connected();
        let (tx, rx) = watch::channel(!drop_sender);
        if drop_sender {
            drop(tx);
        }
        tokio::time::timeout(Duration::from_secs(1), run(host.clone(), settings(), rx))
            .await
            .unwrap();
        assert_eq!(*host.writes.lock().unwrap(), ["unavailable"]);
    }
}
#[tokio::test]
async fn failed_shutdown_restore_has_one_bounded_retry() {
    let host = Fake::connected();
    host.fail_unavailable.store(true, Ordering::SeqCst);
    let (_tx, rx) = watch::channel(true);
    run(host.clone(), settings(), rx).await;
    assert_eq!(*host.writes.lock().unwrap(), ["unavailable", "unavailable"]);
    assert_eq!(*host.saved.lock().unwrap(), None);
}
