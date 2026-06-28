//! The portable async driver: one loop that drives a [`CallEngine`] over an injected
//! [`Runtime`](crate::runtime::Runtime) and [`RelayTransport`](super::transport::RelayTransport).
//! It owns no concrete socket, clock, or executor; native injects the Tokio runtime + the webrtc-rs
//! DataChannel, the WASM bridge injects its single-threaded runtime + a `node:dgram` transport. The
//! esp32 control plane does not use this (it has no relay/media).
//!
//! The loop is the str0m drive contract: wait for one input (a relay packet, a mic frame, or the
//! timer), apply it with `handle_input(now, ..)`, then drain `poll_output()` running each intent,
//! and arm the next timer from `poll_timeout()`. The monotonic clock is `crate::time::Instant`
//! (native `std::time::Instant`; wasm `performance.now`), so no wall clock leaks into the engine.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::FutureExt;
use futures::future::{Fuse, FusedFuture};

use crate::runtime::{BoxFuture, Runtime};
use crate::time::Instant;
use crate::voip::demux::{RelayPacketKind, classify_relay_packet};
use crate::voip::engine::{self, CallEngine, CallEvent, Input, Output};
use crate::voip::transport::{RelayTransport, RelayTransportEvent};

/// The audio + event channels the driver bridges to the platform. Mic frames in, playout frames out
/// (dropped on speaker overflow since VoIP is loss tolerant), and engine events out (RelayAllocated,
/// ForeignAudio) for the shell to act on (e.g. decode a non-MLow frame with a platform codec).
pub struct CallChannels {
    pub mic: async_channel::Receiver<Vec<i16>>,
    pub speaker: async_channel::Sender<Vec<i16>>,
    pub events: async_channel::Sender<CallEvent>,
    /// Caller-only: the answering device's LID, delivered once the callee's `<accept>` is received so
    /// the drive loop can rekey the recv path before media flows. `None` on the callee side and esp32.
    pub rekey: Option<async_channel::Receiver<String>>,
}

/// Depth of the outbound send queue between the drive loop and the concurrent send task. ~2s of RTP
/// at one 60ms frame per slot; on overflow the oldest is dropped (loss-tolerant), bounding latency.
const SEND_QUEUE_CAP: usize = 32;

/// Enforce [`SEND_QUEUE_CAP`] on the outbound queue when a relay write stalls. Media is loss
/// tolerant, but relay control is not: STUN keepalives and the consent Binding Success replies
/// share this queue, and shedding one under media backpressure fails relay consent (RFC 7675) and
/// tears down a still recoverable call. So evict the oldest *media* packet, sparing control; only
/// if the queue is somehow all control do we drop the oldest to keep the bound.
fn shed_to_cap(queue: &mut VecDeque<Bytes>) {
    if queue.len() <= SEND_QUEUE_CAP {
        return;
    }
    let victim = queue
        .iter()
        .position(|p| classify_relay_packet(p) != RelayPacketKind::Stun)
        .unwrap_or(0);
    queue.remove(victim);
}

/// Drive one call to completion: returns when the relay channel disconnects, a send fails, or the
/// relay-event stream closes. On exit it calls `transport.disconnect()` so the platform's relay read
/// pump is released rather than left parked in `recv()`. The caller spawns this on its runtime and
/// stores the [`AbortHandle`](crate::runtime::AbortHandle) (e.g. in a
/// [`CallRegistry`](super::registry::CallRegistry)) to tear the call down; aborting the task drops
/// this future, which drops the transport `Arc` and closes the channel as well.
pub async fn run_call(
    rt: Arc<dyn Runtime>,
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    eng: CallEngine,
) {
    let epoch = Instant::now();
    run_call_with_clock(rt, transport, relay_events, channels, eng, move || {
        epoch.elapsed().as_millis() as u64
    })
    .await;
}

/// [`run_call`] with an injectable monotonic clock, so tests can drive the keepalive/playout timers
/// deterministically without real-time sleeps. Native calls use [`crate::time::Instant`].
// Lifecycle span over the whole drive. The large channels/transport/clock args are skipped; only the
// non-PII call_id is recorded.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(
        name = "wa.voip.run_call",
        level = "debug",
        skip_all,
        fields(call_id = %eng.call_id())
    )
)]
async fn run_call_with_clock(
    rt: Arc<dyn Runtime>,
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    mut eng: CallEngine,
    now_ms: impl Fn() -> engine::Millis,
) {
    eng.start(now_ms());

    #[cfg(feature = "tracing")]
    let call_id = eng.call_id().to_string();

    // The mic feeds outgoing audio only; its liveness must not gate the call. Flipped false when the
    // mic channel closes so we stop polling it without tearing the call down (see the mic arm).
    let mut mic_open = true;

    // The recv-rekey is one-shot (the first callee `<accept>` picks the answering device). Flipped
    // false after the first event -- a rekey or the sender closing -- so the closed channel's
    // always-ready `Err` doesn't busy-spin the select (the mic arm has the same guard).
    let mut rekey_open = true;

    // Decouple sending from receive/playout. Outbound packets are queued and the in-flight send is
    // polled as one arm of the select, CONCURRENTLY with the relay/mic/timer arms -- so a slow or
    // stalled relay write (SCTP cwnd, a congested link, a loaded host) is simply pending here and never
    // freezes the inbound jitter buffer or the playout tick. Awaiting `transport.send()` inline coupled
    // the two: a slow send parked the loop, inbound packets queued then arrived in a burst, and the
    // playout tick fired late -- so the jitter buffer overflowed then underran (audible glitching,
    // worst whatsapp-rust<->whatsapp-rust where both ends stalled; the official client decouples them).
    // The queue is bounded and drops the OLDEST packet on overflow: outbound media is loss tolerant, so
    // when the link can't keep up, shedding the stalest frame (lowest latency) is correct.
    let mut send_queue: VecDeque<Bytes> = VecDeque::new();
    // Idle sentinel: a terminated `Fuse` is safe to re-select every iteration and never fires until a
    // real send replaces it; on completion it terminates itself, so no manual reset / re-poll hazard.
    // `BoxFuture` is `Send` natively but `?Send` on wasm (the transport is single-threaded there).
    let mut sending: Fuse<BoxFuture<'static, anyhow::Result<()>>> = Fuse::terminated();

    'drive: loop {
        // Drain every intent the last mutation produced; stop at the terminal Timeout.
        loop {
            match eng.poll_output() {
                // Queue for the in-flight send arm; never await the write in this loop.
                Output::Transmit(data) => {
                    send_queue.push_back(data);
                    shed_to_cap(&mut send_queue);
                }
                // Loss tolerant: drop the frame if the speaker can't keep up.
                Output::Playout(pcm) => {
                    let _ = channels.speaker.try_send(pcm);
                }
                Output::Event(ev) => {
                    let _ = channels.events.try_send(ev);
                }
                Output::Timeout(_) => break,
            }
        }

        // A terminal relay-allocate failure went out via the drain above; tear the call down rather
        // than keepalive a dead relay (transport.disconnect runs on the exit path below).
        if eng.is_terminated() {
            break 'drive;
        }

        // Start the next queued send when none is in flight. The future owns an Arc clone, so it is
        // `'static` (no borrow of the loop's `transport`).
        if sending.is_terminated()
            && let Some(data) = send_queue.pop_front()
        {
            let t = transport.clone();
            let fut: BoxFuture<'static, anyhow::Result<()>> =
                Box::pin(async move { t.send(data).await });
            sending = fut.fuse();
        }

        // Arm the timer for the engine's next deadline (or never, if it has none).
        let deadline = eng.poll_timeout();
        let now = now_ms();
        let timer = async {
            match deadline {
                Some(at) if at != engine::NEVER => {
                    rt.sleep(Duration::from_millis(at.saturating_sub(now)))
                        .await;
                }
                _ => futures::future::pending::<()>().await,
            }
        }
        .fuse();
        futures::pin_mut!(timer);

        // Poll the mic only while its channel is open. A closed mic must NOT end the call: OS mute can
        // make the mic source (e.g. `pw-record`) EOF, closing this channel, and the keepalive + playout
        // have to keep running or the relay drops us after ~4s of no traffic and the peer reconnects.
        // On close we disable the arm (a closed async_channel is always-ready `Err`, which would
        // otherwise busy-spin the select and starve the timer) and keep driving with a pending mic.
        let mic = &channels.mic;
        let mic_fut = async move {
            if mic_open {
                mic.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(mic_fut);

        // Caller-only recv-rekey: the answering device's LID. Parked (pending) when there is no rekey
        // channel (callee/esp32) or once consumed, mirroring the mic arm so a closed channel can't spin.
        let rekey_live = rekey_open && channels.rekey.is_some();
        let rekey_ch = channels.rekey.as_ref();
        let rekey_fut = async move {
            if rekey_live {
                rekey_ch.expect("rekey_live implies Some").recv().await.ok()
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(rekey_fut);

        // Wait for exactly one input, then apply it. A dropped (unready) recv future loses nothing:
        // async_channel only dequeues on a ready poll.
        // Biased: the in-flight send first so it always makes progress (drain the queue, surface a send
        // failure) -- a slow send is pending here and yields to the arms below, so it can't stall them.
        // Then the recv-rekey BEFORE relay: if the answering device's LID and its first media packet are
        // both ready, apply the rekey first so that packet decrypts under the right keys (no startup
        // garbage frame). Then relay before the timer: drain a ready inbound packet into the jitter buffer
        // BEFORE the playout tick (no phase-slip underrun), firing an overdue timer in-line so a relay/mic
        // flood can't starve the keepalive/playout.
        futures::select_biased! {
            // The in-flight send completed. A failure tears the call down (the old inline behavior).
            res = sending => {
                if res.is_err() {
                    break 'drive;
                }
            },
            // Rekey recv to the device that answered, before its media reaches the relay arm below.
            lid = rekey_fut => {
                rekey_open = false; // one-shot: a LID or the sender closing both disable the arm
                if let Some(lid) = lid
                    && !eng.rekey_recv(&lid)
                {
                    break 'drive; // malformed stored call_key (a setup invariant violated)
                }
            },
            ev = relay_events.recv().fuse() => match ev {
                Ok(RelayTransportEvent::PacketReceived(data)) => {
                    eng.handle_input(now_ms(), Input::RelayPacket(&data));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                // The channel is already open by the time we run; Connected is a redundant confirm.
                Ok(RelayTransportEvent::Connected) => {}
                Ok(RelayTransportEvent::Disconnected(_)) | Err(_) => break 'drive,
            },
            frame = mic_fut => match frame {
                Ok(pcm) => {
                    eng.handle_input(now_ms(), Input::MicFrame(&pcm));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                // Mic source gone (e.g. muted -> pw-record EOF). Stop polling it but keep the call
                // alive: muting the mic must not hang up the call (see the comment above).
                Err(_) => mic_open = false,
            },
            _ = timer => eng.handle_input(now_ms(), Input::Timeout),
        }
    }

    // Any local exit (relay disconnect or send failure -- not a closed mic, which only disables its
    // arm) tears down the transport so the platform's relay read pump -- which may be parked in recv()
    // with no packet coming -- sees the channel close, returns, and releases its task and socket.
    #[cfg(feature = "tracing")]
    tracing::debug!(call_id = %call_id, "voip call drive ended");
    transport.disconnect().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::AbortHandle;
    use crate::voip::demux::{RelayPacketKind, classify_relay_packet};
    use crate::voip::engine::{CallConfig, SequentialTxIds};
    use crate::voip::mlow::MlowEncoder;
    use crate::voip::session::{CallDirection, MediaPipeline, MediaPipelineParams};
    use crate::voip::{RelayDisconnectReason, stun};
    use async_trait::async_trait;
    use bytes::Bytes;
    use portable_atomic::AtomicU64;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Runtime whose `sleep` never resolves, so the driver is exercised purely by the relay-event
    /// stream (the timer arm stays pending). `spawn` is unused: the shell spawns `run_call`, not the
    /// loop itself.
    struct PendingSleepRuntime;
    #[async_trait]
    impl Runtime for PendingSleepRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, _d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(futures::future::pending())
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    #[derive(Default)]
    struct RecordingTransport {
        sent: Mutex<Vec<Bytes>>,
    }
    #[async_trait]
    impl RelayTransport for RecordingTransport {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data);
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    fn config() -> CallConfig {
        CallConfig {
            call_id: "CID".into(),
            direction: CallDirection::Incoming,
            self_lid: "111111111111111:0@lid".into(),
            peer_lid: "222222222222222:0@lid".into(),
            call_key: (0u8..32).collect(),
            ssrc: 0x5741_0001,
            samples_per_packet: 960,
            relay_token: vec![0xAB; 16],
            relay_ip: "203.0.113.7".into(),
            relay_port: 3478,
            integrity_key: b"relay-key".to_vec(),
            warp_mi_tag_len: 4,
            enable_media: true,
            enable_sframe: false,
        }
    }

    // The driver wiring: start sends the allocate, an inbound binding request gets a binding-success
    // reply, and a Disconnected event ends the loop. Timer-pending so this is deterministic.
    #[test]
    fn run_call_emits_allocate_and_answers_binding_request() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        let req =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[9u8; 12], &[], None, false);
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(req)))
            .unwrap();
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();

        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
            },
            eng,
        ));

        let sent = transport.sent.lock().unwrap();
        assert!(
            sent.iter()
                .any(|b| stun::stun_message_type(b) == Some(stun::MSG_ALLOCATE_REQUEST)),
            "start must emit the STUN allocate"
        );
        assert!(
            sent.iter()
                .any(|b| stun::stun_message_type(b) == Some(stun::MSG_BINDING_SUCCESS)),
            "a binding request must be answered with a binding success"
        );
    }

    /// Runtime with an instant `sleep` that closes `relay_tx` once it has been armed `close_after`
    /// times, so a driver loop that survives a closed mic still terminates deterministically -- via the
    /// relay path, never the mic. `sleeps` counts the arms so a test can assert the loop kept arming
    /// the keepalive/playout timer after the mic went away.
    struct CloseRelayOnSleepRuntime {
        sleeps: Arc<AtomicUsize>,
        relay_tx: async_channel::Sender<RelayTransportEvent>,
        close_after: usize,
    }
    #[async_trait]
    impl Runtime for CloseRelayOnSleepRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, _d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            if self.sleeps.fetch_add(1, Ordering::Relaxed) + 1 >= self.close_after {
                self.relay_tx.close();
            }
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // Regression: a closed mic must NOT end the call. OS mute can make the mic source (pw-record) EOF,
    // closing the mic channel; if that tore the loop down, the relay would lose its 1s keepalive, drop
    // us after ~4s, and the peer would reconnect -- the mute/unmute reconnect loop. The loop must
    // instead disable the mic and keep arming the keepalive/playout timer. We pre-close the mic, then
    // let the runtime end the call via the relay after a couple of timer arms; had the closed mic
    // ended the call, the loop would break before arming any timer and `sleeps` would stay 0.
    #[test]
    fn closed_mic_does_not_end_the_call() {
        let sleeps = Arc::new(AtomicUsize::new(0));
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let rt: Arc<dyn Runtime> = Arc::new(CloseRelayOnSleepRuntime {
            sleeps: sleeps.clone(),
            relay_tx,
            close_after: 2,
        });
        let transport = Arc::new(RecordingTransport::default());
        let (mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        mic_tx.close(); // mic source gone before the call even starts
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
            },
            eng,
        ));

        assert!(
            sleeps.load(Ordering::Relaxed) >= 1,
            "a closed mic must not end the call: the loop has to keep arming the keepalive timer"
        );
        assert!(
            transport
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|b| stun::stun_message_type(b) == Some(stun::MSG_ALLOCATE_REQUEST)),
            "the call must still start (emit the allocate) with a dead mic"
        );
    }

    // A continuously-ready relay must not starve the keepalive/playout timers. `select_biased!`
    // always prefers the relay arm, so without the in-line overdue-timer fire a relay flood would
    // defer Timeout forever (no keepalive -> relay drops us -> reconnect). The clock races forward so
    // every deadline is overdue; the relay is pre-filled (the flood) then closed. With the fix, each
    // drained packet fires the overdue timer, so playout frames keep flowing during the flood.
    #[test]
    fn relay_flood_does_not_starve_the_timer() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime); // the timer arm never resolves itself
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        for _ in 0..40 {
            relay_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from_static(
                    b"\0\0\0\0",
                )))
                .unwrap();
        }
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        // Each clock read jumps a full second, so the 1s keepalive and 20ms playout are always overdue.
        let clock = Arc::new(AtomicU64::new(0));
        let clk = clock.clone();
        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call_with_clock(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
            },
            eng,
            move || clk.fetch_add(1000, Ordering::Relaxed),
        ));

        let playouts = std::iter::from_fn(|| spk_rx.try_recv().ok()).count();
        assert!(
            playouts > 0,
            "the playout timer must keep firing during a relay flood (not starved); got {playouts}"
        );
    }

    /// Records sends and fails after `fail_after` of them, so a drive terminates deterministically via
    /// the send-failure `break 'drive`. A mic-starvation test needs this: a relay event would be
    /// biased ahead of the mic arm and pre-empt the very backlog under test, so the terminator must
    /// not come from the relay.
    struct FailAfterTransport {
        sent: Mutex<Vec<Bytes>>,
        fail_after: usize,
    }
    #[async_trait]
    impl RelayTransport for FailAfterTransport {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            let mut s = self.sent.lock().unwrap();
            s.push(data);
            if s.len() > self.fail_after {
                anyhow::bail!("send failure (test terminator)");
            }
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    // The mic side of the same starvation hazard as `relay_flood_does_not_starve_the_timer`:
    // `select_biased!` prefers the mic arm over the timer, so without the in-line overdue-timer fire
    // (mirroring the relay arm) a continuously-ready mic -- a custom producer on an unbounded channel,
    // or a built-up backlog -- would defer the keepalive/playout `Timeout` indefinitely. The relay
    // stays empty+open so the mic arm wins every iteration; the clock races forward so every deadline
    // is overdue; a send-failure terminator ends the drive (a relay event would win the bias). With
    // the fix, each drained mic frame fires the overdue timer, so playout frames keep flowing.
    #[test]
    fn mic_flood_does_not_starve_the_timer() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime); // the timer arm never resolves itself
        let transport = Arc::new(FailAfterTransport {
            sent: Mutex::new(Vec::new()),
            fail_after: 60,
        });
        // Relay empty but OPEN: `_relay_tx` keeps the channel from closing (a closed/Err relay would
        // win the bias and break immediately), so `recv()` just pends and the mic arm always wins.
        let (_relay_tx, relay_rx) = async_channel::unbounded::<RelayTransportEvent>();
        // A deep backlog of non-silent frames, so the mic arm is continuously ready (never drains
        // before the send-failure terminator fires).
        let (mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let tone: Vec<i16> = (0..960i32).map(|i| (i % 200) as i16 - 99).collect();
        for _ in 0..300 {
            mic_tx.try_send(tone.clone()).unwrap();
        }
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        // Each clock read jumps a full second, so the 1s keepalive and 20ms playout are always overdue.
        let clock = Arc::new(AtomicU64::new(0));
        let clk = clock.clone();
        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call_with_clock(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
            },
            eng,
            move || clk.fetch_add(1000, Ordering::Relaxed),
        ));

        let playouts = std::iter::from_fn(|| spk_rx.try_recv().ok()).count();
        assert!(
            playouts > 0,
            "the playout timer must keep firing during a mic flood (not starved); got {playouts}"
        );
    }

    // A terminal allocate-error must end run_call: the driver breaks after forwarding the terminal
    // event, so the call tears down instead of keepaliving a dead relay forever. The relay event
    // stream is never closed (only the allocate error is pushed); if the engine did not terminate
    // the loop, block_on would deadlock on the pending timer/relay arms.
    #[test]
    fn allocate_error_ends_run_call() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();

        // Raw Allocate-error STUN packet carrying ERROR-CODE 486 (class 4, number 86).
        let err_attr = [0x00, 0x09, 0x00, 0x04, 0x00, 0x00, 4u8, 86u8];
        let err =
            stun::encode_stun_request(stun::MSG_ALLOCATE_ERROR, &[3u8; 12], &err_attr, None, false);
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(err)))
            .unwrap();
        // Note: the relay stream is intentionally NOT closed; the engine termination must end the loop.

        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
            },
            eng,
        ));

        // The terminal event reached the consumer before the loop broke.
        let events: Vec<CallEvent> = std::iter::from_fn(|| ev_rx.try_recv().ok()).collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CallEvent::RelayAllocateFailed(486))),
            "the terminal RelayAllocateFailed must be delivered before teardown"
        );
    }

    /// A runtime with virtual time: `sleep(d)` advances the shared clock by `d` and resolves at once,
    /// so a `block_on` drive steps deterministically through the keepalive/playout deadlines with no
    /// real waiting. Pair with a `now_ms` closure that reads the same clock.
    struct VirtualTimeRuntime {
        clock: Arc<AtomicU64>,
    }
    #[async_trait]
    impl Runtime for VirtualTimeRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            self.clock
                .fetch_add(d.as_millis() as u64, Ordering::Relaxed);
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    /// An in-memory relay that closes the media loop end-to-end without a real UDP/DTLS/SCTP socket.
    /// It records what the engine transmits and reacts like a real relay+peer: it accepts the first
    /// STUN allocate (so the engine's media path goes live) and then streams two MLow tone frames
    /// back as a mirrored peer. After `stop_after_allocates` allocates (one keepalive cycle) it pushes
    /// Disconnected to end the call.
    struct FakeRelay {
        events: async_channel::Sender<RelayTransportEvent>,
        sent: Mutex<Vec<Bytes>>,
        peer: Mutex<PeerSim>,
    }
    struct PeerSim {
        pipe: MediaPipeline,
        enc: MlowEncoder,
        allocates: usize,
        stop_after_allocates: usize,
    }
    #[async_trait]
    impl RelayTransport for FakeRelay {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data.clone());
            if stun::stun_message_type(&data) != Some(stun::MSG_ALLOCATE_REQUEST) {
                return Ok(());
            }
            let mut peer = self.peer.lock().unwrap();
            peer.allocates += 1;
            if peer.allocates == 1 {
                // The relay accepts the allocate, then the mirrored peer streams two MLow tone frames.
                let ok = stun::encode_stun_request(
                    stun::MSG_ALLOCATE_SUCCESS,
                    &[1u8; 12],
                    &[],
                    None,
                    false,
                );
                let _ = self
                    .events
                    .try_send(RelayTransportEvent::PacketReceived(Bytes::from(ok)));
                for n in 0..2u32 {
                    let tone: Vec<f32> = (0..960usize)
                        .map(|i| 0.3 * ((i as f32 + (n * 960) as f32) * 0.07).sin())
                        .collect();
                    let frame = peer.enc.encode(&tone).expect("mlow encode");
                    let pkt = peer.pipe.protect_audio(&frame);
                    let _ = self
                        .events
                        .try_send(RelayTransportEvent::PacketReceived(Bytes::from(pkt)));
                }
            } else if peer.allocates >= peer.stop_after_allocates {
                let _ = self.events.try_send(RelayTransportEvent::Disconnected(
                    RelayDisconnectReason::Closed,
                ));
            }
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    // End-to-end over the in-memory FakeRelay: allocate handshake -> RelayAllocated, mic tone ->
    // outbound RTP, peer RTP -> audible playout, a keepalive over (virtual) time, then teardown. This
    // drives the real run_call + engine + RelayTransport seam; only the webrtc-rs socket is mocked,
    // closing the "media path not exercised end-to-end" gap.
    #[test]
    fn full_media_path_over_fake_relay() {
        let clock = Arc::new(AtomicU64::new(0));
        let rt: Arc<dyn Runtime> = Arc::new(VirtualTimeRuntime {
            clock: clock.clone(),
        });

        let (relay_tx, relay_rx) = async_channel::unbounded();
        let cfg = config();
        // Mirror: the peer's self LID is our peer LID, so its protect keys match the engine's unprotect.
        let peer_pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: &cfg.peer_lid,
            peer_lid: &cfg.self_lid,
            ssrc: cfg.ssrc,
            samples_per_packet: cfg.samples_per_packet,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .unwrap();
        let relay = Arc::new(FakeRelay {
            events: relay_tx,
            sent: Mutex::new(Vec::new()),
            peer: Mutex::new(PeerSim {
                pipe: peer_pipe,
                enc: MlowEncoder::new(),
                allocates: 0,
                stop_after_allocates: 2,
            }),
        });

        let (mic_tx, mic_rx) = async_channel::unbounded();
        let tone: Vec<i16> = (0..cfg.samples_per_packet as usize)
            .map(|i| (8000.0 * (i as f32 * 0.1).sin()) as i16)
            .collect();
        mic_tx.try_send(tone).unwrap();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();

        let eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        let clk = clock.clone();
        futures::executor::block_on(run_call_with_clock(
            rt,
            relay.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
            },
            eng,
            move || clk.load(Ordering::Relaxed),
        ));
        // mic_tx stays alive until here so the mic channel never closes during the drive.
        drop(mic_tx);

        // 1. The allocate handshake surfaced RelayAllocated to the shell.
        let events: Vec<CallEvent> = std::iter::from_fn(|| ev_rx.try_recv().ok()).collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CallEvent::RelayAllocated)),
            "the allocate handshake must surface RelayAllocated"
        );

        // 2. Outbound: an initial allocate, the mic tone's RTP, and a keepalive re-allocate.
        let sent = relay.sent.lock().unwrap();
        let allocates = sent
            .iter()
            .filter(|b| stun::stun_message_type(b) == Some(stun::MSG_ALLOCATE_REQUEST))
            .count();
        assert!(
            allocates >= 2,
            "initial allocate + at least one keepalive re-allocate; got {allocates}"
        );
        let rtp = sent
            .iter()
            .filter(|b| matches!(classify_relay_packet(b), RelayPacketKind::Rtp))
            .count();
        assert!(
            rtp >= 1,
            "the mic tone must produce at least one outbound RTP packet"
        );

        // 3. Inbound: the peer's RTP decoded to non-silent playout at the speaker.
        let peak = std::iter::from_fn(|| spk_rx.try_recv().ok())
            .flatten()
            .map(|s| s.abs())
            .max()
            .unwrap_or(0);
        assert!(
            peak > 0,
            "peer RTP must decode to audible playout end-to-end"
        );
    }

    /// Runtime for the decoupling test: each `sleep` advances a shared virtual clock (so the playout
    /// timer actually fires) and, once the clock passes `close_at_ms`, closes the relay channel to end
    /// the call deterministically -- independent of `transport.send()`, which is wedged in the test.
    struct DrivingRuntime {
        clock: Arc<AtomicU64>,
        relay_tx: async_channel::Sender<RelayTransportEvent>,
        close_at_ms: u64,
    }
    #[async_trait]
    impl Runtime for DrivingRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let now = self
                .clock
                .fetch_add(d.as_millis() as u64, Ordering::Relaxed)
                + d.as_millis() as u64;
            if now >= self.close_at_ms {
                self.relay_tx.close();
            }
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // Regression: a wedged relay write must NOT freeze the receive/playout path. This reproduces the
    // root cause of the whatsapp-rust<->whatsapp-rust glitching: with the old inline
    // `transport.send().await`, the first send (the STUN allocate) blocked the whole loop, so inbound
    // packets never decoded and the speaker starved (silent/choppy audio). Now the send is decoupled,
    // so injected peer RTP still decodes to audible playout while the send is stuck forever.
    #[test]
    fn wedged_send_does_not_freeze_inbound_playout() {
        struct WedgedSend;
        #[async_trait]
        impl RelayTransport for WedgedSend {
            async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
                // A congested SCTP / dead link: this write never completes.
                futures::future::pending().await
            }
            async fn disconnect(&self) {}
        }

        let cfg = config();
        // Mirror the peer's pipeline (its self LID is our peer LID) so the RTP it "sends" decrypts and
        // decodes on our side.
        let mut peer_pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: &cfg.peer_lid,
            peer_lid: &cfg.self_lid,
            ssrc: cfg.ssrc,
            samples_per_packet: cfg.samples_per_packet,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .unwrap();
        let mut enc = MlowEncoder::new();

        let (relay_tx, relay_rx) = async_channel::unbounded();
        // Several non-silent peer frames (enough to clear the playout prebuffer), fed directly so the
        // inbound path does NOT depend on our send reaching the relay.
        for n in 0..6u32 {
            let tone: Vec<f32> = (0..960usize)
                .map(|i| 0.3 * ((i as f32 + (n * 960) as f32) * 0.05).sin())
                .collect();
            let frame = enc.encode(&tone).expect("mlow encode");
            let pkt = peer_pipe.protect_audio(&frame);
            relay_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from(pkt)))
                .unwrap();
        }

        let clock = Arc::new(AtomicU64::new(0));
        let rt: Arc<dyn Runtime> = Arc::new(DrivingRuntime {
            clock: clock.clone(),
            relay_tx: relay_tx.clone(),
            // ~25 playout ticks: long enough to drain the injected frames before the relay closes.
            close_at_ms: 500,
        });
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        let clk = clock.clone();
        let eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        // Run on a worker thread with a wall-clock bound: the OLD inline-send loop DEADLOCKS here (the
        // wedged allocate send freezes the loop forever), so without a bound a regression would hang the
        // whole test binary instead of failing. The fixed loop terminates in microseconds.
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            futures::executor::block_on(run_call_with_clock(
                rt,
                Arc::new(WedgedSend) as Arc<dyn RelayTransport>,
                relay_rx,
                CallChannels {
                    mic: mic_rx,
                    speaker: spk_tx,
                    events: ev_tx,
                    rekey: None,
                },
                eng,
                move || clk.load(Ordering::Relaxed),
            ));
            // Despite `send()` being wedged forever, the injected peer RTP must have decoded to audible
            // playout; report its peak amplitude back to the test.
            let peak = std::iter::from_fn(|| spk_rx.try_recv().ok())
                .flatten()
                .map(|s| s.abs())
                .max()
                .unwrap_or(0);
            let _ = done_tx.send(peak);
        });

        let peak = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("run_call must terminate: a wedged transport.send() must not freeze the loop");
        assert!(
            peak > 0,
            "inbound audio must play out while transport.send() is wedged (send/receive decoupled)"
        );
    }

    // A relay stall backs the queue up past cap: the overflow policy must shed media, never the STUN
    // control (keepalive / consent Binding Success) sharing the queue, or relay consent fails.
    #[test]
    fn overflow_sheds_media_and_spares_control() {
        // version 2 + extension bit -> 0x90, classified as Rtp media.
        let media = |seq: u8| Bytes::from(vec![0x90, seq]);
        // Top two bits zero -> STUN control.
        let control = || Bytes::from(vec![0x00, 0x01]);

        let mut q: VecDeque<Bytes> = VecDeque::new();
        q.push_back(control()); // oldest, must survive
        for n in 0..SEND_QUEUE_CAP as u8 {
            q.push_back(media(n));
            shed_to_cap(&mut q);
        }

        assert_eq!(q.len(), SEND_QUEUE_CAP);
        assert_eq!(
            classify_relay_packet(&q[0]),
            RelayPacketKind::Stun,
            "the queued control packet must not be evicted by media backpressure"
        );
        // The oldest media (seq 0) is the one shed, not the control at the front.
        assert_eq!(&q[1][..], &[0x90, 1]);
    }

    // Pathological: an all-control queue still has to honor the bound, so it falls back to dropping
    // the oldest.
    #[test]
    fn overflow_all_control_drops_oldest() {
        let mut q: VecDeque<Bytes> = (0..=SEND_QUEUE_CAP as u8)
            .map(|n| Bytes::from(vec![0x00, n]))
            .collect();
        shed_to_cap(&mut q);
        assert_eq!(q.len(), SEND_QUEUE_CAP);
        assert_eq!(
            &q[0][..],
            &[0x00, 1],
            "oldest control dropped to keep bound"
        );
    }
}
