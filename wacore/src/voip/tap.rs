//! Optional packet-capture tap over the [`RelayTransport`](super::transport::RelayTransport) seam,
//! for debugging a live call. It decorates a transport/factory to record every packet crossing the
//! seam -- both directions -- into a pluggable [`PacketTap`] sink, WITHOUT touching the engine, the
//! driver, or the codec (the seam is the only interposition point). The sink is any `PacketTap`
//! impl: a file dump, a pcap writer, an in-memory buffer, a logger, a network forwarder. This is the
//! "transport-decorator dump" the design notes left as a follow-up -- modular and consumer-driven,
//! and zero cost when not wired (you simply don't wrap the transport).

use std::sync::Arc;

use anyhow::Result;
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
pub(crate) enum PacketDir {
    /// Sent by us to the relay (an engine `Output::Transmit`).
    Outbound,
    /// Received from the relay (a `RelayTransportEvent::PacketReceived`).
    Inbound,
}

/// A sink for relay packets crossing the seam. Implement it to dump to a file, a pcap, a log, an
/// in-memory buffer, or to forward elsewhere. Called once per packet in both directions -- inline on
/// the send path and on the inbound forwarding hop -- so keep it cheap and non-blocking. It sees
/// every packet, including ones the driver later drops under backpressure (recording happens first).
pub(crate) trait PacketTap: crate::sync_marker::MaybeSendSync {
    fn on_packet(&self, dir: PacketDir, data: &[u8]);
}

/// Decorates a [`RelayTransport`], recording every outbound packet before delegating to the inner
/// transport. Pure: no runtime and no I/O of its own (any I/O is the sink's).
pub(crate) struct TappedTransport {
    inner: Arc<dyn RelayTransport>,
    tap: Arc<dyn PacketTap>,
}

impl TappedTransport {
    pub fn new(inner: Arc<dyn RelayTransport>, tap: Arc<dyn PacketTap>) -> Self {
        Self { inner, tap }
    }
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
}

/// Forward inbound events from `inner_rx` to `out_tx`, recording each `PacketReceived` to `tap`
/// first. Drops on a full `out_tx` (loss tolerant, like the relay pump) and stops when the source
/// closes (relay gone) or the driver drops its receiver. Spawned by [`TappedFactory::connect`]; a
/// free fn so the forwarding/recording logic is testable without a runtime.
async fn tap_forward(
    inner_rx: async_channel::Receiver<RelayTransportEvent>,
    out_tx: async_channel::Sender<RelayTransportEvent>,
    tap: Arc<dyn PacketTap>,
) {
    while let Ok(ev) = inner_rx.recv().await {
        if let RelayTransportEvent::PacketReceived(data) = &ev {
            tap.on_packet(PacketDir::Inbound, data);
        }
        match out_tx.try_send(ev) {
            Ok(()) => {}
            // Mirror the native relay pump: media is loss tolerant, but a dropped STUN Binding
            // Request means the engine never replies Binding Success and relay consent expires.
            // This forwarder sits AFTER the pump, so it must preserve STUN too (a Request that
            // survived the pump can't be silently dropped here). Drop only media; block on STUN.
            Err(async_channel::TrySendError::Full(ev)) => {
                let is_stun = matches!(&ev, RelayTransportEvent::PacketReceived(d)
                    if classify_relay_packet(d) == RelayPacketKind::Stun);
                if is_stun && out_tx.send(ev).await.is_err() {
                    break;
                }
            }
            Err(async_channel::TrySendError::Closed(_)) => break,
        }
    }
}

/// Decorates a [`RelayTransportFactory`] so BOTH directions are tapped: outbound via
/// [`TappedTransport`], inbound via a forwarding task spawned on the injected runtime that records
/// each packet before handing it to the driver. Construct it only when capture is wanted -- the
/// un-tapped path pays nothing. Runtime-agnostic, so native and the WASM bridge use the same tap.
pub(crate) struct TappedFactory {
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
        let transport: Arc<dyn RelayTransport> =
            Arc::new(TappedTransport::new(inner_transport, self.tap.clone()));
        let (out_tx, out_rx) = async_channel::bounded(TAP_FORWARD_CAP);
        // Fire-and-forget: the forwarder self-terminates when the inner stream closes (relay gone) or
        // the driver drops `out_rx` (call ended), so the abort handle is detached rather than stored.
        self.runtime
            .spawn(Box::pin(tap_forward(inner_rx, out_tx, self.tap.clone())))
            .detach();
        Ok((transport, out_rx))
    }
}

/// A [`PacketTap`] that records every packet into an in-memory buffer. For tests and in-process
/// inspection; a file/pcap sink lives in the consumer (e.g. the example's `VOIP_DUMP`).
#[derive(Default)]
pub(crate) struct InMemoryTap {
    captured: std::sync::Mutex<Vec<(PacketDir, Vec<u8>)>>,
}

impl InMemoryTap {
    /// A snapshot of every captured packet, in capture order, as `(direction, bytes)`.
    pub fn captured(&self) -> Vec<(PacketDir, Vec<u8>)> {
        self.captured.lock().unwrap().clone()
    }
}

impl PacketTap for InMemoryTap {
    fn on_packet(&self, dir: PacketDir, data: &[u8]) {
        self.captured.lock().unwrap().push((dir, data.to_vec()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::RelayDisconnectReason;
    use std::sync::Mutex;

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
}
