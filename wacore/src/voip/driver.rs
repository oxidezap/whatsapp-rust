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
use crate::voip::h264::VideoFrame;
use crate::voip::rtp::{RTP_PAYLOAD_TYPE_H264, parse_rtp_header};
use crate::voip::transport::{RelayTransport, RelayTransportEvent};

/// Mid-call video-plane commands from the shell (upgrade / downgrade / peer orientation). Kept out
/// of the engine so it stays sans-IO; the drive loop translates each into an engine method call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VideoControl {
    /// RTP clock increment for each access unit. Sent before attaching a source whose cadence is
    /// different from the 15 fps compatibility default.
    SetTimestampStride(u32),
    /// Bring the video plane up with outbound video ALLOWED (a from-start call, an accept, or the
    /// initiator once the peer accepted the upgrade).
    Enable,
    /// Bring the video plane up but hold outbound video off the wire until the peer accepts (the
    /// initiator of an upgrade). Inbound still decodes. A later `Enable` ungates it.
    EnableAwaitingAccept,
    /// Tear the video plane down (downgrade to audio).
    Disable,
    /// The peer's device orientation (0..3, ×90°) from a `<video>` stanza.
    SetOrientation(u8),
}

/// The audio + video + event channels the driver bridges to the platform. Mic frames in, playout
/// frames out (dropped on speaker overflow since VoIP is loss tolerant), and engine events out
/// (RelayAllocated, ForeignAudio) for the shell to act on (e.g. decode a non-MLow frame with a
/// platform codec). The video channels are always present — a call that never negotiates video
/// simply leaves them idle (the engine drops AUs while its video plane is off).
pub struct CallChannels {
    pub mic: async_channel::Receiver<Vec<i16>>,
    pub speaker: async_channel::Sender<Vec<i16>>,
    pub events: async_channel::Sender<CallEvent>,
    /// Caller-only: the answering device's LID, delivered once the callee's `<accept>` is received so
    /// the drive loop can rekey the recv path before media flows. `None` on the callee side and esp32.
    pub rekey: Option<async_channel::Receiver<String>>,
    /// Outbound video: one pre-encoded H.264 Annex-B access unit per item.
    pub video_in: async_channel::Receiver<Vec<u8>>,
    /// Inbound video: reassembled peer access units (dropped on sink overflow, like the speaker).
    pub video_out: async_channel::Sender<VideoFrame>,
    /// Mid-call video-plane control (upgrade/downgrade/orientation).
    pub video_ctl: async_channel::Receiver<VideoControl>,
}

/// Bound slow relay writes without truncating a complete video access unit.
const SEND_QUEUE_BATCH_CAP: usize = 64;
const SEND_QUEUE_BYTE_CAP: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendBatchKind {
    Control,
    Media,
    Video,
}

struct SendBatch {
    packets: VecDeque<Bytes>,
    bytes: usize,
    kind: SendBatchKind,
    started: bool,
}

impl SendBatch {
    fn packet(data: Bytes) -> Self {
        let kind = if classify_relay_packet(&data) == RelayPacketKind::Stun {
            SendBatchKind::Control
        } else {
            SendBatchKind::Media
        };
        Self {
            bytes: data.len(),
            packets: VecDeque::from([data]),
            kind,
            started: false,
        }
    }

    fn video(packets: Vec<Bytes>) -> Self {
        Self {
            bytes: packets.iter().map(Bytes::len).sum(),
            packets: packets.into(),
            kind: SendBatchKind::Video,
            started: false,
        }
    }
}

#[derive(Default)]
struct DroppedMedia {
    video_access_units: u32,
    packets: u32,
}

fn shed_to_cap(queue: &mut VecDeque<SendBatch>) -> DroppedMedia {
    let mut dropped = DroppedMedia::default();
    loop {
        let bytes: usize = queue.iter().map(|batch| batch.bytes).sum();
        if queue.len() <= SEND_QUEUE_BATCH_CAP && bytes <= SEND_QUEUE_BYTE_CAP {
            break;
        }
        // Never cut an AU after one of its fragments has entered the transport.
        let victim = queue
            .iter()
            .position(|batch| !batch.started && batch.kind == SendBatchKind::Video)
            .or_else(|| {
                queue
                    .iter()
                    .position(|batch| !batch.started && batch.kind == SendBatchKind::Media)
            })
            .or_else(|| queue.iter().position(|batch| !batch.started));
        let Some(victim) = victim else {
            break;
        };
        let Some(batch) = queue.remove(victim) else {
            break;
        };
        dropped.packets = dropped
            .packets
            .saturating_add(batch.packets.len().try_into().unwrap_or(u32::MAX));
        if batch.kind == SendBatchKind::Video {
            dropped.video_access_units = dropped.video_access_units.saturating_add(1);
        }
    }
    dropped
}

fn enqueue_batch(queue: &mut VecDeque<SendBatch>, batch: SendBatch) -> DroppedMedia {
    queue.push_back(batch);
    shed_to_cap(queue)
}

/// Coalesce every PT-97 packet through the marker into one queue unit, so backpressure keeps or
/// drops a complete H.264 access unit instead of silently truncating an IDR.
fn queue_transmit(
    queue: &mut VecDeque<SendBatch>,
    pending_video: &mut Vec<Bytes>,
    data: Bytes,
) -> DroppedMedia {
    if let Some(header) = parse_rtp_header(&data)
        && header.payload_type == RTP_PAYLOAD_TYPE_H264
    {
        pending_video.push(data);
        if header.marker {
            return enqueue_batch(queue, SendBatch::video(std::mem::take(pending_video)));
        }
        return DroppedMedia::default();
    }
    let mut dropped = DroppedMedia::default();
    if !pending_video.is_empty() {
        dropped = enqueue_batch(queue, SendBatch::video(std::mem::take(pending_video)));
    }
    let more = enqueue_batch(queue, SendBatch::packet(data));
    dropped.video_access_units = dropped
        .video_access_units
        .saturating_add(more.video_access_units);
    dropped.packets = dropped.packets.saturating_add(more.packets);
    dropped
}

fn pop_next_packet(queue: &mut VecDeque<SendBatch>) -> Option<Bytes> {
    let index = queue
        .iter()
        .position(|batch| batch.kind == SendBatchKind::Control)
        .unwrap_or(0);
    let batch = queue.get_mut(index)?;
    batch.started = true;
    let packet = batch.packets.pop_front()?;
    batch.bytes = batch.bytes.saturating_sub(packet.len());
    if batch.packets.is_empty() {
        queue.remove(index);
    }
    Some(packet)
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
    let wallclock_ms = crate::time::now_millis().max(0) as u64;
    run_call_with_clock_and_wallclock(
        rt,
        transport,
        relay_events,
        channels,
        eng,
        move || epoch.elapsed().as_millis() as u64,
        wallclock_ms,
    )
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
#[cfg(test)]
async fn run_call_with_clock(
    rt: Arc<dyn Runtime>,
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    eng: CallEngine,
    now_ms: impl Fn() -> engine::Millis,
) {
    run_call_with_clock_and_wallclock(
        rt,
        transport,
        relay_events,
        channels,
        eng,
        now_ms,
        1_700_000_000_000,
    )
    .await;
}

async fn run_call_with_clock_and_wallclock(
    rt: Arc<dyn Runtime>,
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    mut eng: CallEngine,
    now_ms: impl Fn() -> engine::Millis,
    wallclock_ms: u64,
) {
    eng.start(now_ms(), wallclock_ms);

    #[cfg(feature = "tracing")]
    let call_id = eng.call_id().to_string();

    // The mic feeds outgoing audio only; its liveness must not gate the call. Flipped false when the
    // mic channel closes so we stop polling it without tearing the call down (see the mic arm).
    let mut mic_open = true;

    // The recv-rekey is one-shot (the first callee `<accept>` picks the answering device). Flipped
    // false after the first event -- a rekey or the sender closing -- so the closed channel's
    // always-ready `Err` doesn't busy-spin the select (the mic arm has the same guard).
    let mut rekey_open = true;

    // Same closed-channel guards for the video arms: a call that never wires a video source/control
    // sender must not busy-spin on their always-ready `Err`.
    let mut video_in_open = true;
    let mut video_ctl_open = true;
    // Set by a `Disable` to drain the video input queue after the select block (it can't be drained
    // inside, where the arm futures borrow the channel).
    let mut drain_video_in = false;

    // Decouple sending from receive/playout. Outbound packets are queued and the in-flight send is
    // polled as one arm of the select, CONCURRENTLY with the relay/mic/timer arms -- so a slow or
    // stalled relay write (SCTP cwnd, a congested link, a loaded host) is simply pending here and never
    // freezes the inbound jitter buffer or the playout tick. Awaiting `transport.send()` inline coupled
    // the two: a slow send parked the loop, inbound packets queued then arrived in a burst, and the
    // playout tick fired late -- so the jitter buffer overflowed then underran (audible glitching,
    // worst whatsapp-rust<->whatsapp-rust where both ends stalled; the official client decouples them).
    // Video is queued per access unit so overload never leaves half an IDR on the wire.
    let mut send_queue: VecDeque<SendBatch> = VecDeque::new();
    // Idle sentinel: a terminated `Fuse` is safe to re-select every iteration and never fires until a
    // real send replaces it; on completion it terminates itself, so no manual reset / re-poll hazard.
    // `BoxFuture` is `Send` natively but `?Send` on wasm (the transport is single-threaded there).
    let mut sending: Fuse<BoxFuture<'static, anyhow::Result<()>>> = Fuse::terminated();

    'drive: loop {
        // Drain every intent the last mutation produced; stop at the terminal Timeout.
        let mut pending_video = Vec::new();
        loop {
            match eng.poll_output() {
                // Queue for the in-flight send arm; never await the write in this loop.
                Output::Transmit(data) => {
                    let dropped = queue_transmit(&mut send_queue, &mut pending_video, data);
                    if dropped.packets != 0 {
                        let _ = channels.events.try_send(CallEvent::OutboundMediaDropped {
                            video_access_units: dropped.video_access_units,
                            packets: dropped.packets,
                        });
                    }
                }
                // Loss tolerant: drop the frame if the speaker can't keep up.
                Output::Playout(pcm) => {
                    let _ = channels.speaker.try_send(pcm);
                }
                // Same policy for video: a stalled sink sheds frames, never the drive loop.
                Output::VideoPlayout(frame) => {
                    let _ = channels.video_out.try_send(frame);
                }
                Output::Event(ev) => {
                    let _ = channels.events.try_send(ev);
                }
                Output::Timeout(_) => {
                    if !pending_video.is_empty() {
                        let dropped = enqueue_batch(
                            &mut send_queue,
                            SendBatch::video(std::mem::take(&mut pending_video)),
                        );
                        if dropped.packets != 0 {
                            let _ = channels.events.try_send(CallEvent::OutboundMediaDropped {
                                video_access_units: dropped.video_access_units,
                                packets: dropped.packets,
                            });
                        }
                    }
                    break;
                }
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
            && let Some(data) = pop_next_packet(&mut send_queue)
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

        // Video source and control arms, guarded like the mic: a closed channel disables the arm
        // (a video source EOF must not end the call — audio keeps running after a downgrade).
        let video_in = &channels.video_in;
        let video_in_fut = async move {
            if video_in_open {
                video_in.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(video_in_fut);

        let video_ctl = &channels.video_ctl;
        let video_ctl_fut = async move {
            if video_ctl_open {
                video_ctl.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(video_ctl_fut);

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
            // Video-plane control before the media arms, so an Enable lands before queued AUs and
            // a Disable stops decoding queued inbound PT-97 right away.
            ctl = video_ctl_fut => {
                match ctl {
                    Ok(VideoControl::SetTimestampStride(ts_stride)) => {
                        let _ = eng.set_video_timestamp_stride(ts_stride);
                    }
                    Ok(VideoControl::Enable) => {
                        // False only for a control-only engine or a malformed stored callKey; the
                        // audio plane already validated the key, so treat it as a no-op not fatal.
                        let _ = eng.enable_video();
                    }
                    Ok(VideoControl::EnableAwaitingAccept) => {
                        let _ = eng.enable_video_gated();
                    }
                    Ok(VideoControl::Disable) => {
                        eng.disable_video();
                        // Discard any AUs still queued from the (now-detached) source, so a quick
                        // re-Enable can't transmit stale frames from the previous session under the
                        // new negotiation. Drained after the select block (the futures borrow the
                        // channel).
                        drain_video_in = true;
                    }
                    Ok(VideoControl::SetOrientation(o)) => eng.set_peer_video_orientation(o),
                    Err(_) => video_ctl_open = false,
                }
                // Fire an overdue timer like the other ready arms so a stream of control messages
                // cannot keep this arm hot and defer the keepalive.
                let now = now_ms();
                if let Some(at) = eng.poll_timeout()
                    && at != engine::NEVER
                    && now >= at
                {
                    eng.handle_input(now, Input::Timeout);
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
            au = video_in_fut => match au {
                Ok(au) => {
                    eng.handle_input(now_ms(), Input::VideoFrame(&au));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                // Video source gone (encoder EOF / downgrade released it): disable the arm but keep
                // the call alive, exactly like the mic.
                Err(_) => video_in_open = false,
            },
            _ = timer => eng.handle_input(now_ms(), Input::Timeout),
        }

        // Post-select: the arm futures (which borrow the channels) have been dropped, so it is now
        // safe to drain the video input queue requested by a Disable above.
        if drain_video_in {
            drain_video_in = false;
            while channels.video_in.try_recv().is_ok() {}
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

    /// CallChannels with idle video plumbing (senders/receivers dropped immediately), for the
    /// audio-only driver tests: the closed-channel guards must keep those arms inert.
    fn test_channels(
        mic: async_channel::Receiver<Vec<i16>>,
        speaker: async_channel::Sender<Vec<i16>>,
        events: async_channel::Sender<CallEvent>,
    ) -> CallChannels {
        let (_vin_tx, vin_rx) = async_channel::unbounded::<Vec<u8>>();
        let (vout_tx, _vout_rx) = async_channel::unbounded::<VideoFrame>();
        let (_vctl_tx, vctl_rx) = async_channel::unbounded::<VideoControl>();
        CallChannels {
            mic,
            speaker,
            events,
            rekey: None,
            video_in: vin_rx,
            video_out: vout_tx,
            video_ctl: vctl_rx,
        }
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
            enable_video: false,
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
            test_channels(mic_rx, spk_tx, ev_tx),
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
            test_channels(mic_rx, spk_tx, ev_tx),
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
            test_channels(mic_rx, spk_tx, ev_tx),
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
            test_channels(mic_rx, spk_tx, ev_tx),
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
            test_channels(mic_rx, spk_tx, ev_tx),
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
            test_channels(mic_rx, spk_tx, ev_tx),
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
                test_channels(mic_rx, spk_tx, ev_tx),
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

    // Video through the real drive loop: Enable + orientation via video_ctl (biased ahead of the
    // media arms), mirrored peer video packets via the relay (reassemble to video_out with the
    // orientation stamped), one of our own AUs via video_in (fans out to PT-97 FU-A transmits).
    // Deterministic: queues drain by bias order, then the first timer arm closes the relay.
    #[test]
    fn drive_loop_routes_video_both_ways() {
        use crate::voip::rtp::{RTP_PAYLOAD_TYPE_H264, VIDEO_TS_STRIDE_15FPS, parse_rtp_header};
        use crate::voip::session::{VideoPipeline, VideoPipelineParams};
        use crate::voip::ssrc;

        let sleeps = Arc::new(AtomicUsize::new(0));
        let (relay_tx, relay_rx) = async_channel::unbounded();
        // The timer arm is only polled once every queue is idle; its first sleep closes the relay,
        // ending the loop deterministically after all the video work is done.
        let rt: Arc<dyn Runtime> = Arc::new(CloseRelayOnSleepRuntime {
            sleeps,
            relay_tx: relay_tx.clone(),
            close_after: 1,
        });
        let transport = Arc::new(RecordingTransport::default());
        let cfg = config();

        // Mirrored peer video pipe (its self LID = our peer LID).
        let mut peer_video = VideoPipeline::new(&VideoPipelineParams {
            call_key: &cfg.call_key,
            self_lid: &cfg.peer_lid,
            peer_lid: &cfg.self_lid,
            ssrc: ssrc::derive_video_participant_ssrc(
                &cfg.call_id,
                &ssrc::format_e2e_srtp_participant_id(&cfg.peer_lid),
            ),
            ts_stride: VIDEO_TS_STRIDE_15FPS,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .unwrap();
        let make_au = |len: usize| -> Vec<u8> {
            let mut au = vec![0, 0, 0, 1, 0x65];
            au.extend((0..len).map(|i| (i % 251) as u8));
            au
        };
        let peer_au = make_au(2000);
        for p in peer_video.protect_video(&peer_au) {
            relay_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from(p)))
                .unwrap();
        }

        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();
        let (vin_tx, vin_rx) = async_channel::unbounded::<Vec<u8>>();
        let (vout_tx, vout_rx) = async_channel::unbounded::<VideoFrame>();
        let (vctl_tx, vctl_rx) = async_channel::unbounded::<VideoControl>();

        // Control drains first (bias), so cadence, Enable, and orientation land before any AU.
        vctl_tx
            .try_send(VideoControl::SetTimestampStride(4500))
            .unwrap();
        vctl_tx.try_send(VideoControl::Enable).unwrap();
        vctl_tx.try_send(VideoControl::SetOrientation(1)).unwrap();
        let our_au = make_au(3000);
        vin_tx.try_send(our_au.clone()).unwrap();
        vin_tx.try_send(our_au).unwrap();

        let eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                events: ev_tx,
                rekey: None,
                video_in: vin_rx,
                video_out: vout_tx,
                video_ctl: vctl_rx,
            },
            eng,
        ));

        // Inbound: the peer AU reassembled to the sink, orientation stamped from the control arm.
        let frames: Vec<VideoFrame> = std::iter::from_fn(|| vout_rx.try_recv().ok()).collect();
        assert_eq!(frames.len(), 1, "peer AU must reach video_out exactly once");
        assert_eq!(frames[0].data, peer_au);
        assert!(frames[0].keyframe);
        assert_eq!(
            frames[0].orientation, 1,
            "SetOrientation must apply before the inbound AU reassembles"
        );

        // Outbound: each 3KB AU fans out to four PT-97 packets and the 20 fps stride applies.
        let sent = transport.sent.lock().unwrap();
        let video_headers = sent
            .iter()
            .filter_map(|packet| {
                parse_rtp_header(packet).filter(|h| h.payload_type == RTP_PAYLOAD_TYPE_H264)
            })
            .collect::<Vec<_>>();
        assert_eq!(video_headers.len(), 8);
        assert_eq!(
            video_headers
                .iter()
                .filter(|header| header.marker)
                .map(|header| header.timestamp)
                .collect::<Vec<_>>(),
            [0, 4500]
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

        let mut q: VecDeque<SendBatch> = VecDeque::new();
        q.push_back(SendBatch::packet(control())); // oldest, must survive
        for n in 0..SEND_QUEUE_BATCH_CAP as u8 {
            q.push_back(SendBatch::packet(media(n)));
            let _ = shed_to_cap(&mut q);
        }

        assert_eq!(q.len(), SEND_QUEUE_BATCH_CAP);
        assert_eq!(
            q[0].kind,
            SendBatchKind::Control,
            "the queued control packet must not be evicted by media backpressure"
        );
        // The oldest media (seq 0) is the one shed, not the control at the front.
        assert_eq!(&q[1].packets[0][..], &[0x90, 1]);
    }

    // Pathological: an all-control queue still has to honor the bound, so it falls back to dropping
    // the oldest.
    #[test]
    fn overflow_all_control_drops_oldest() {
        let mut q: VecDeque<SendBatch> = (0..=SEND_QUEUE_BATCH_CAP as u8)
            .map(|n| SendBatch::packet(Bytes::from(vec![0x00, n])))
            .collect();
        let _ = shed_to_cap(&mut q);
        assert_eq!(q.len(), SEND_QUEUE_BATCH_CAP);
        assert_eq!(
            &q[0].packets[0][..],
            &[0x00, 1],
            "oldest control dropped to keep bound"
        );
    }

    #[test]
    fn large_video_au_is_queued_atomically() {
        fn video_packet(seq: u16, marker: bool) -> Bytes {
            let mut packet = vec![0u8; 16];
            packet[0] = 0x90; // V=2, X=1
            packet[1] = ((marker as u8) << 7) | RTP_PAYLOAD_TYPE_H264;
            packet[2..4].copy_from_slice(&seq.to_be_bytes());
            packet[12..14].copy_from_slice(&0xdebeu16.to_be_bytes());
            Bytes::from(packet)
        }

        // The old 32-datagram queue truncated this AU before its marker.
        let mut queue = VecDeque::new();
        let mut pending = Vec::new();
        for seq in 0..40u16 {
            let dropped = queue_transmit(&mut queue, &mut pending, video_packet(seq, seq == 39));
            assert_eq!(dropped.packets, 0);
        }
        assert!(pending.is_empty());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].kind, SendBatchKind::Video);
        assert_eq!(queue[0].packets.len(), 40);

        let sent: Vec<u16> = std::iter::from_fn(|| pop_next_packet(&mut queue))
            .map(|packet| parse_rtp_header(&packet).unwrap().sequence_number)
            .collect();
        assert_eq!(sent, (0..40u16).collect::<Vec<_>>());
    }

    #[test]
    fn oversized_video_is_dropped_as_a_whole_au() {
        let packets = (0..40)
            .map(|_| Bytes::from(vec![0x90; 64 * 1024]))
            .collect();
        let mut queue = VecDeque::new();
        let dropped = enqueue_batch(&mut queue, SendBatch::video(packets));
        assert_eq!(dropped.video_access_units, 1);
        assert_eq!(dropped.packets, 40);
        assert!(queue.is_empty(), "no partial AU may remain queued");
    }
}
