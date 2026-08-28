//! Optional packet-capture tap over the [`RelayTransport`] seam,
//! for debugging a live call. It decorates a transport/factory to record every packet crossing the
//! seam -- both directions -- into a pluggable `PacketTap` sink, WITHOUT touching the engine, the
//! driver, or the codec (the seam is the only interposition point). The sink is any `PacketTap`
//! impl: a file dump, a pcap writer, an in-memory buffer, a logger, a network forwarder. This is the
//! "transport-decorator dump" the design notes left as a follow-up -- modular and consumer-driven,
//! and zero cost when not wired (you simply don't wrap the transport).

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Result, bail};
use async_trait::async_trait;
use bytes::Bytes;

use super::demux::{RelayPacketKind, classify_relay_packet};
use super::transport::{RelayTransport, RelayTransportEvent, RelayTransportFactory};
use crate::runtime::Runtime;

/// Inbound-forwarding channel depth for [`TappedFactory`]. VoIP is loss tolerant, so the forwarder
/// drops a packet (after recording it) rather than block when the driver falls behind -- matching
/// the relay read pump.
const TAP_FORWARD_CAP: usize = 256;

/// Which way a captured packet was crossing the seam.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketDir {
    /// Sent by us to the relay (an engine `Output::Transmit`).
    Outbound,
    /// Received from the relay (a `RelayTransportEvent::PacketReceived`).
    Inbound,
}

/// A sink for relay packets crossing the seam. Implement it to dump to a file, a pcap, a log, an
/// in-memory buffer, or to forward elsewhere. Called once per packet in both directions -- inline on
/// the send path and on the inbound forwarding hop -- so keep it cheap and non-blocking.
///
/// What it sees is everything that reaches the seam, which on the inbound side means everything the
/// platform transport handed up: the tap decorates that transport, it does not replace it. Media
/// the transport's own read pump discards before producing an event (see `inbound_pipe_dropped`)
/// was never at the seam and is not recorded -- a capture taken during an overload is a capture of
/// what survived it. Past that point nothing is lost: a packet the DRIVER later drops under
/// backpressure is recorded first, so the tap and the call disagree only in the transport's
/// favour.
pub trait PacketTap: crate::sync_marker::MaybeSendSync {
    fn on_packet(&self, dir: PacketDir, data: &[u8]);
}

/// Decorates a [`RelayTransport`], recording every outbound packet before delegating to the inner
/// transport. No I/O of its own (any I/O is the sink's).
pub struct TappedTransport {
    inner: Arc<dyn RelayTransport>,
    tap: Arc<dyn PacketTap>,
    /// Needed only by [`RelayTransport::reconnect`], which has to spawn a forwarding hop for the
    /// replacement channel's inbound stream. `None` for a transport built by [`Self::new`].
    runtime: Option<Arc<dyn Runtime>>,
}

impl TappedTransport {
    /// Tap a transport that will never be asked to reconnect.
    ///
    /// A relay migration through this one fails the call: wrapping the replacement channel means
    /// spawning a forwarder for its inbound stream, and that needs a runtime. Prefer
    /// [`with_runtime`](Self::with_runtime), which [`TappedFactory`] uses for exactly this reason.
    pub fn new(inner: Arc<dyn RelayTransport>, tap: Arc<dyn PacketTap>) -> Self {
        Self {
            inner,
            tap,
            runtime: None,
        }
    }

    /// Tap a transport that can follow the call to a new relay, keeping the tap attached.
    pub fn with_runtime(
        inner: Arc<dyn RelayTransport>,
        tap: Arc<dyn PacketTap>,
        runtime: Arc<dyn Runtime>,
    ) -> Self {
        Self {
            inner,
            tap,
            runtime: Some(runtime),
        }
    }
}

/// Wrap one connected inner channel: tap the send side, and put the inbound stream behind a
/// forwarding hop that records it. Shared by the factory's first connect and by every later
/// reconnect, so a migrated call is tapped exactly like the original.
fn tap_channel(
    inner_transport: Arc<dyn RelayTransport>,
    inner_rx: async_channel::Receiver<RelayTransportEvent>,
    tap: Arc<dyn PacketTap>,
    runtime: Arc<dyn Runtime>,
) -> (
    Arc<dyn RelayTransport>,
    async_channel::Receiver<RelayTransportEvent>,
) {
    let transport: Arc<dyn RelayTransport> = Arc::new(TappedTransport::with_runtime(
        inner_transport,
        tap.clone(),
        runtime.clone(),
    ));
    let (out_tx, out_rx) = async_channel::bounded(TAP_FORWARD_CAP);
    // Fire-and-forget: the forwarder self-terminates when the inner stream closes (relay gone) or
    // the driver drops `out_rx` (call ended), so the abort handle is detached rather than stored.
    runtime
        .spawn(Box::pin(tap_forward(inner_rx, out_tx, tap)))
        .detach();
    (transport, out_rx)
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RelayTransport for TappedTransport {
    async fn send(&self, data: Bytes) -> Result<()> {
        self.tap.on_packet(PacketDir::Outbound, &data);
        self.inner.send(data).await
    }
    async fn disconnect(&self) {
        self.inner.disconnect().await;
    }
    /// Follow the call to the new relay rather than inheriting the trait's "cannot redial" default,
    /// which would end every tapped call that migrates -- a group update moving the relay is a
    /// routine event, and tapping a call must not change whether it survives one.
    async fn reconnect(
        &self,
        endpoint: SocketAddr,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )> {
        let Some(runtime) = self.runtime.clone() else {
            bail!("tapped transport was built without a runtime and cannot rewrap {endpoint}")
        };
        let (inner_transport, inner_rx) = self.inner.reconnect(endpoint).await?;
        Ok(tap_channel(
            inner_transport,
            inner_rx,
            self.tap.clone(),
            runtime,
        ))
    }
}

/// Forward inbound events from `inner_rx` to `out_tx`, recording each `PacketReceived` to `tap`
/// first. Drops media on a full `out_tx` (loss tolerant, like the relay pump), accumulating what it
/// dropped and reporting it as `InboundDropped` as soon as there is room again, and stops when the
/// source closes (relay gone) or the driver drops its receiver. Spawned by [`TappedFactory::connect`]; a
/// free fn so the forwarding/recording logic is testable without a runtime.
async fn tap_forward(
    inner_rx: async_channel::Receiver<RelayTransportEvent>,
    out_tx: async_channel::Sender<RelayTransportEvent>,
    tap: Arc<dyn PacketTap>,
) {
    // Media this forwarder itself discarded, waiting for room to report it. Kept as a running total
    // rather than sent per packet: the moment there is something to report is exactly the moment the
    // channel is full, so a blocking send here would stall the tap behind the driver it is feeding.
    let mut local_drops: u32 = 0;
    while let Ok(ev) = inner_rx.recv().await {
        if let RelayTransportEvent::PacketReceived(data) = &ev {
            tap.on_packet(PacketDir::Inbound, data);
        }
        // Handed over BEFORE the packet, for the same reason as the native pump's `deliver`: under
        // the backpressure this report describes the driver frees one slot per packet, so a report
        // sent after the packet that just refilled it would stay pending for the life of the call.
        // The engine folds it into `inbound_pipe_dropped`; without it a tapped call under
        // backpressure loses audio with no counter moving and the watchdog blames the codec.
        if local_drops > 0
            && out_tx
                .try_send(RelayTransportEvent::InboundDropped(local_drops))
                .is_ok()
        {
            local_drops = 0;
        }
        match out_tx.try_send(ev) {
            Ok(()) => {}
            // Mirror the native relay pump: media is loss tolerant, but a dropped STUN Binding
            // Request means the engine never replies Binding Success and relay consent expires.
            // This forwarder sits AFTER the pump, so it must preserve STUN too (a Request that
            // survived the pump can't be silently dropped here). Drop only media; block on STUN.
            Err(async_channel::TrySendError::Full(ev)) => {
                // Exhaustive on purpose, no wildcard: media is the ONLY loss-tolerant thing on this
                // channel, and a wildcard quietly enrolls every variant added later into being
                // dropped as media. `Disconnected` fell into one and was discarded -- and counted
                // as lost media besides -- so a transport that reports the loss before closing its
                // senders left the driver attached with nothing to tell it otherwise.
                // `InboundDropped` is likewise not media but the pump's report OF media, and
                // dropping it would leave the call with no record of the loss at exactly the moment
                // it is losing packets.
                let is_control = match &ev {
                    RelayTransportEvent::PacketReceived(d) => {
                        classify_relay_packet(d) == RelayPacketKind::Stun
                    }
                    RelayTransportEvent::Connected
                    | RelayTransportEvent::InboundDropped(_)
                    | RelayTransportEvent::Disconnected(_) => true,
                };
                if is_control {
                    // A terminal event is the last thing the driver will ever read, so the report
                    // has to precede it rather than trail it -- the same ordering the native pump's
                    // `finish` keeps, and for the same reason: after this, nothing arrives to carry
                    // the count and the call's closing stats would omit the burst that preceded the
                    // failure. Awaited, not best-effort: the driver is draining, so a full queue is
                    // a moment to wait out, and only a closed one ends this.
                    if matches!(ev, RelayTransportEvent::Disconnected(_)) && local_drops > 0 {
                        let report = RelayTransportEvent::InboundDropped(local_drops);
                        if out_tx.send(report).await.is_err() {
                            break;
                        }
                        local_drops = 0;
                    }
                    if out_tx.send(ev).await.is_err() {
                        break;
                    }
                } else {
                    local_drops = local_drops.saturating_add(1);
                }
            }
            Err(async_channel::TrySendError::Closed(_)) => break,
        }
    }
    // Awaited rather than best-effort: the loop can also end because the inner relay simply stopped,
    // with the driver still draining a full queue. `try_send` failed there and the count went with
    // it -- an overload immediately before a failure being exactly the one worth attributing. A
    // closed channel still ends it, which is the only case where nothing is left to diagnose.
    if local_drops > 0 {
        let _ = out_tx
            .send(RelayTransportEvent::InboundDropped(local_drops))
            .await;
    }
}

/// Decorates a [`RelayTransportFactory`] so BOTH directions are tapped: outbound via
/// [`TappedTransport`], inbound via a forwarding task spawned on the injected runtime that records
/// each packet before handing it to the driver. Construct it only when capture is wanted -- the
/// un-tapped path pays nothing. Runtime-agnostic, so native and the WASM bridge use the same tap.
pub struct TappedFactory {
    inner: Arc<dyn RelayTransportFactory>,
    tap: Arc<dyn PacketTap>,
    runtime: Arc<dyn Runtime>,
}

impl TappedFactory {
    pub fn new(
        inner: Arc<dyn RelayTransportFactory>,
        tap: Arc<dyn PacketTap>,
        runtime: Arc<dyn Runtime>,
    ) -> Self {
        Self {
            inner,
            tap,
            runtime,
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl RelayTransportFactory for TappedFactory {
    async fn connect(
        &self,
    ) -> Result<(
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    )> {
        let (inner_transport, inner_rx) = self.inner.connect().await?;
        Ok(tap_channel(
            inner_transport,
            inner_rx,
            self.tap.clone(),
            self.runtime.clone(),
        ))
    }
}

/// A [`PacketTap`] that records every packet into an in-memory buffer. For tests and in-process
/// inspection; a file/pcap sink lives in the consumer (e.g. the example's `VOIP_DUMP`).
#[derive(Default)]
pub struct InMemoryTap {
    captured: std::sync::Mutex<Vec<(PacketDir, Vec<u8>)>>,
}

impl InMemoryTap {
    /// A snapshot of every captured packet, in capture order, as `(direction, bytes)`.
    ///
    /// A poisoned lock yields whatever was captured before the panic rather than propagating it:
    /// this is a diagnostic decorator on the relay's forwarding path, and losing capture is the
    /// worst it may ever cost a live call.
    pub fn captured(&self) -> Vec<(PacketDir, Vec<u8>)> {
        self.captured.lock().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |guard| guard.clone(),
        )
    }
}

impl PacketTap for InMemoryTap {
    fn on_packet(&self, dir: PacketDir, data: &[u8]) {
        // Same reasoning as `captured`: a tap that panics here would take the call down with it, on
        // the forwarding task, for a buffer nothing on the media path reads.
        let mut captured = match self.captured.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        captured.push((dir, data.to_vec()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::AbortHandle;
    use crate::voip::RelayDisconnectReason;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::time::Duration;

    #[derive(Default)]
    struct RecordingTransport {
        sent: Mutex<Vec<Bytes>>,
    }
    #[async_trait]
    impl RelayTransport for RecordingTransport {
        async fn send(&self, data: Bytes) -> Result<()> {
            self.sent.lock().unwrap().push(data);
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    /// An inner transport that can redial, handing back a fresh channel like a real one does.
    #[derive(Default)]
    struct RedialingTransport {
        sent: Mutex<Vec<Bytes>>,
        inbound: Mutex<Option<async_channel::Sender<RelayTransportEvent>>>,
    }
    #[async_trait]
    impl RelayTransport for RedialingTransport {
        async fn send(&self, data: Bytes) -> Result<()> {
            self.sent.lock().unwrap().push(data);
            Ok(())
        }
        async fn disconnect(&self) {}
        async fn reconnect(
            &self,
            _endpoint: SocketAddr,
        ) -> Result<(
            Arc<dyn RelayTransport>,
            async_channel::Receiver<RelayTransportEvent>,
        )> {
            let replacement = Arc::new(RedialingTransport::default());
            let (tx, rx) = async_channel::unbounded();
            *replacement.inbound.lock().unwrap() = Some(tx);
            Ok((replacement, rx))
        }
    }

    /// Runs spawned futures on a thread of their own, which is all `tap_forward` needs.
    struct BlockingRuntime;
    #[async_trait]
    impl Runtime for BlockingRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            std::thread::spawn(move || futures::executor::block_on(future));
            AbortHandle::new(|| {})
        }
        fn sleep(&self, _duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(std::future::pending())
        }
        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            f();
            Box::pin(std::future::ready(()))
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // A group update can move a call to another relay, and the driver ends the call when its
    // transport cannot redial. Inheriting the trait's "cannot redial" default meant tapping a call
    // decided whether it survived a routine migration -- and the tap is a debugging aid that must
    // not change what it observes.
    #[test]
    fn a_tapped_call_survives_a_relay_migration_and_stays_tapped() {
        let inner = Arc::new(RedialingTransport::default());
        let tap = Arc::new(InMemoryTap::default());
        let tapped = TappedTransport::with_runtime(
            inner.clone(),
            tap.clone(),
            Arc::new(BlockingRuntime) as Arc<dyn Runtime>,
        );

        let endpoint: SocketAddr = "203.0.113.7:3478".parse().expect("endpoint");
        let (replacement, rx) = futures::executor::block_on(tapped.reconnect(endpoint))
            .expect("a tapped transport must be able to follow the call to the new relay");

        // The replacement still records what it sends...
        futures::executor::block_on(replacement.send(Bytes::from_static(b"\x01\x02"))).unwrap();
        assert_eq!(tap.captured(), vec![(PacketDir::Outbound, vec![1, 2])]);
        // ...and can itself migrate again, rather than being a one-shot.
        assert!(futures::executor::block_on(replacement.reconnect(endpoint)).is_ok());
        drop(rx);
    }

    // Built without a runtime there is no way to wrap the replacement's inbound stream, so the
    // failure is explicit rather than a silently untapped channel.
    #[test]
    fn a_runtimeless_tapped_transport_refuses_to_reconnect() {
        let tapped = TappedTransport::new(
            Arc::new(RedialingTransport::default()),
            Arc::new(InMemoryTap::default()),
        );
        let endpoint: SocketAddr = "203.0.113.7:3478".parse().expect("endpoint");
        assert!(futures::executor::block_on(tapped.reconnect(endpoint)).is_err());
    }

    #[test]
    fn tapped_transport_records_outbound_then_delegates() {
        let inner = Arc::new(RecordingTransport::default());
        let tap = Arc::new(InMemoryTap::default());
        let tapped = TappedTransport::new(inner.clone(), tap.clone());
        futures::executor::block_on(async {
            tapped.send(Bytes::from_static(b"\x01\x02")).await.unwrap();
            tapped.send(Bytes::from_static(b"\x03")).await.unwrap();
        });
        // Recorded both, in order, as Outbound...
        assert_eq!(
            tap.captured(),
            vec![
                (PacketDir::Outbound, vec![1, 2]),
                (PacketDir::Outbound, vec![3]),
            ]
        );
        // ...and still delegated to the inner transport unchanged.
        assert_eq!(inner.sent.lock().unwrap().len(), 2);
    }

    #[test]
    fn tap_forward_records_inbound_and_forwards_every_event() {
        let (inner_tx, inner_rx) = async_channel::unbounded();
        let (out_tx, out_rx) = async_channel::unbounded();
        inner_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from_static(
                b"\xaa",
            )))
            .unwrap();
        inner_tx.try_send(RelayTransportEvent::Connected).unwrap();
        inner_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from_static(
                b"\xbb\xcc",
            )))
            .unwrap();
        inner_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        inner_tx.close();

        let tap = Arc::new(InMemoryTap::default());
        futures::executor::block_on(tap_forward(inner_rx, out_tx, tap.clone()));

        // Only PacketReceived is captured (both, in order, as Inbound) -- not Connected/Disconnected.
        assert_eq!(
            tap.captured(),
            vec![
                (PacketDir::Inbound, vec![0xaa]),
                (PacketDir::Inbound, vec![0xbb, 0xcc]),
            ]
        );
        // Every event is forwarded to the driver unchanged.
        let forwarded: Vec<_> = std::iter::from_fn(|| out_rx.try_recv().ok()).collect();
        assert_eq!(forwarded.len(), 4);
        assert!(matches!(forwarded[1], RelayTransportEvent::Connected));
        assert!(matches!(forwarded[3], RelayTransportEvent::Disconnected(_)));
    }

    // Backpressure: this forwarder sits after the relay pump, so it must also preserve STUN control
    // while dropping media. A cap-1 out channel fills on the first media; the media behind it is
    // dropped while the STUN is held by a blocking send -- so the second event the driver sees is the
    // STUN, proving the media in between was dropped and the STUN survived. Recording happens before
    // the drop decision, so the tap still captures all three.
    #[test]
    fn tap_forward_preserves_stun_but_drops_media_under_backpressure() {
        let (inner_tx, inner_rx) = async_channel::unbounded();
        for pkt in [
            &b"\x90\x78\x01\x02"[..], // RTP media: fills the cap-1 channel
            &b"\x90\x78\x03\x04"[..], // RTP media: dropped while the channel is full
            &b"\x00\x01\x05\x06"[..], // STUN binding request: must survive the backpressure
        ] {
            inner_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::copy_from_slice(
                    pkt,
                )))
                .unwrap();
        }
        inner_tx.close();
        let (out_tx, out_rx) = async_channel::bounded(1);
        let tap = Arc::new(InMemoryTap::default());

        futures::executor::block_on(async {
            let fwd = tap_forward(inner_rx, out_tx, tap.clone());
            let drain = async {
                let a = out_rx.recv().await.unwrap();
                let b = out_rx.recv().await.unwrap();
                (a, b)
            };
            let (_, (a, b)) = futures::join!(fwd, drain);
            assert!(
                matches!(&a, RelayTransportEvent::PacketReceived(d) if d[0] == 0x90),
                "first delivered is the media that filled the channel, got {a:?}"
            );
            assert!(
                matches!(&b, RelayTransportEvent::PacketReceived(d)
                    if classify_relay_packet(d) == RelayPacketKind::Stun),
                "STUN must survive while the media behind the first was dropped, got {b:?}"
            );
        });
        // Recording happens before the drop decision, so all three are still captured.
        assert_eq!(
            tap.captured().len(),
            3,
            "the tap records every packet, even ones later dropped"
        );
    }

    // What the forwarder discards has to reach the same counter the relay pump's own drops do,
    // otherwise a tapped call under backpressure loses audio while `inbound_pipe_dropped` stays at
    // zero and the watchdog blames the codec for a transport problem.
    #[test]
    fn tap_forward_reports_the_media_it_dropped_as_inbound_dropped() {
        let media = || RelayTransportEvent::PacketReceived(Bytes::from_static(b"\x90\x78\x01\x02"));
        let (inner_tx, inner_rx) = async_channel::unbounded();
        // Two fill the cap-2 channel; the two behind them are dropped by this forwarder.
        for _ in 0..4 {
            inner_tx.try_send(media()).unwrap();
        }
        let (out_tx, out_rx) = async_channel::bounded(2);
        let tap = Arc::new(InMemoryTap::default());

        let seen = futures::executor::block_on(async {
            let fwd = tap_forward(inner_rx, out_tx, tap);
            let drive = async {
                let mut seen = vec![out_rx.recv().await.unwrap(), out_rx.recv().await.unwrap()];
                // Room again: the next packet the driver takes carries the backlog with it.
                inner_tx.try_send(media()).unwrap();
                inner_tx.close();
                while let Ok(ev) = out_rx.recv().await {
                    seen.push(ev);
                }
                seen
            };
            let (_, seen) = futures::join!(fwd, drive);
            seen
        });

        let reported: u32 = seen
            .iter()
            .filter_map(|ev| match ev {
                RelayTransportEvent::InboundDropped(n) => Some(*n),
                _ => None,
            })
            .sum();
        assert_eq!(
            reported, 2,
            "both dropped media packets are accounted for, got {seen:?}"
        );
    }

    // Lifecycle is not media. Dropped through the old wildcard, a `Disconnected` that arrived
    // while the channel was full left the driver attached to a transport that had already reported
    // itself gone -- and inflated the media-loss counter on the way out.
    #[test]
    fn tap_forward_never_drops_the_disconnect() {
        let media = || RelayTransportEvent::PacketReceived(Bytes::from_static(b"\x90\x78\x01\x02"));
        let (inner_tx, inner_rx) = async_channel::unbounded();
        // Fills the cap-1 channel, so everything behind it meets a full queue.
        inner_tx.try_send(media()).unwrap();
        inner_tx.try_send(media()).unwrap();
        inner_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        inner_tx.close();
        let (out_tx, out_rx) = async_channel::bounded(1);
        let tap = Arc::new(InMemoryTap::default());

        let seen = futures::executor::block_on(async {
            let fwd = tap_forward(inner_rx, out_tx, tap);
            let drive = async {
                let mut seen = Vec::new();
                while let Ok(ev) = out_rx.recv().await {
                    seen.push(ev);
                }
                seen
            };
            let (_, seen) = futures::join!(fwd, drive);
            seen
        });

        assert!(
            seen.iter()
                .any(|ev| matches!(ev, RelayTransportEvent::Disconnected(_))),
            "the disconnect has to survive a full queue, got {seen:?}"
        );
        assert!(
            matches!(seen.last(), Some(RelayTransportEvent::Disconnected(_))),
            "and arrives after the media ahead of it, not instead of it, got {seen:?}"
        );
        // The report is no longer best effort at teardown: a terminal event is the last thing the
        // driver reads, so a count that trails it is a count nobody sees.
        let report = seen
            .iter()
            .position(|ev| matches!(ev, RelayTransportEvent::InboundDropped(_)));
        let disconnect = seen
            .iter()
            .position(|ev| matches!(ev, RelayTransportEvent::Disconnected(_)));
        assert!(
            report.is_some() && report < disconnect,
            "the drop report has to precede the disconnect, got {seen:?}"
        );
    }

    // The other way the forwarder ends: the inner relay simply stops, with no `Disconnected` to
    // hang the flush on and the driver still draining a full queue. `try_send` failed there and
    // took the count with it -- and an overload immediately before a failure is exactly the one
    // worth attributing.
    #[test]
    fn tap_forward_flushes_its_last_drops_when_the_relay_just_stops() {
        let media = || RelayTransportEvent::PacketReceived(Bytes::from_static(b"\x90\x78\x01\x02"));
        let (inner_tx, inner_rx) = async_channel::unbounded();
        for _ in 0..3 {
            inner_tx.try_send(media()).unwrap();
        }
        inner_tx.close();
        let (out_tx, out_rx) = async_channel::bounded(1);
        let tap = Arc::new(InMemoryTap::default());

        let seen = futures::executor::block_on(async {
            let fwd = tap_forward(inner_rx, out_tx, tap);
            let drive = async {
                let mut seen = Vec::new();
                while let Ok(ev) = out_rx.recv().await {
                    seen.push(ev);
                }
                seen
            };
            let (_, seen) = futures::join!(fwd, drive);
            seen
        });

        assert!(
            seen.iter()
                .any(|ev| matches!(ev, RelayTransportEvent::InboundDropped(n) if *n > 0)),
            "the media dropped before the relay stopped has to be reported, got {seen:?}"
        );
    }

    // The steady state of a tapped call that is behind: the driver frees exactly one slot before
    // each arriving packet. Reported after the packet, the report loses that slot to the packet
    // every time and never leaves the forwarder -- so the tap would hide the very losses the
    // counter exists to expose.
    #[test]
    fn tap_forward_flushes_its_drop_report_under_sustained_backpressure() {
        let media = || RelayTransportEvent::PacketReceived(Bytes::from_static(b"\x90\x78\x01\x02"));
        let (inner_tx, inner_rx) = async_channel::unbounded();
        // One fills the cap-1 channel, the second is dropped and becomes the pending report.
        for _ in 0..2 {
            inner_tx.try_send(media()).unwrap();
        }
        let (out_tx, out_rx) = async_channel::bounded(1);
        let tap = Arc::new(InMemoryTap::default());

        let seen = futures::executor::block_on(async {
            let fwd = tap_forward(inner_rx, out_tx, tap);
            let drive = async {
                let mut seen = vec![out_rx.recv().await.unwrap()];
                // One slot freed, one packet arriving: the slot must go to the report.
                inner_tx.try_send(media()).unwrap();
                inner_tx.close();
                while let Ok(ev) = out_rx.recv().await {
                    seen.push(ev);
                }
                seen
            };
            let (_, seen) = futures::join!(fwd, drive);
            seen
        });

        assert!(
            seen.iter()
                .any(|ev| matches!(ev, RelayTransportEvent::InboundDropped(1))),
            "the drop report must reach the driver, got {seen:?}"
        );
    }
}
