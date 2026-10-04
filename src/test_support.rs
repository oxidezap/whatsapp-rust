//! Native fixtures, enabled with `test-support` on non-WASM targets.
//!
//! [`CallFixture`] connects a real client to a synthetic Noise server, seeds
//! fictitious Signal peers and waits for production readiness. It requires no
//! SQLite, HTTP access or media transport. Certificate signatures are bypassed
//! only for this fixture's client; this does not prove WhatsApp interoperability.
//!
//! [`CallFixture::next_offer`] returns a pending production send. Inspect its
//! stanza, then complete or fail it. Completion does not acknowledge the offer.
//! [`CallFixture::inject`] runs the real stanza parser and handlers; success
//! does not imply that a malformed action was accepted. Observations report
//! overflow rather than silently discarding entries.
//!
//! Hold a pending offer to test acceptance before builder completion. Spawn
//! injection separately, observe the answering device with `call_snapshot`,
//! complete the offer, and await both operations. Use observable state instead
//! of sleeps. [`CallFixture::shutdown`] joins the reader; cancellation and Drop
//! retain ownership of teardown rather than detaching it.
//!
//! Acquire `CallHandle::take_events` once and retain the receiver. Peer video
//! events carry the received source and creator; those fields are observations,
//! not authorization guarantees. The bounded event queue can evict old entries.
//!
//! # Example
//!
//! ```rust
//! use std::sync::Arc;
//! use whatsapp_rust::test_support::CallFixture;
//! use wacore_binary::builder::NodeBuilder;
//!
//! # async fn example() -> anyhow::Result<()> {
//! let fixture = Arc::new(CallFixture::new().await?);
//! let client = fixture.client().clone();
//! let peer = fixture.peer().clone();
//! let (_mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(1);
//! let (speaker_tx, _speaker_rx) = async_channel::bounded::<Vec<i16>>(1);
//! let (_video_tx, video_rx) = async_channel::bounded::<Vec<u8>>(1);
//! let (sink_tx, _sink_rx) = async_channel::bounded::<wacore::voip::VideoFrame>(1);
//! let starting = tokio::spawn(async move {
//!     client.voip().call(&peer)
//!         .audio(mic_rx, speaker_tx)
//!         .video(video_rx, sink_tx)
//!         .start().await
//! });
//!
//! let offer = fixture.next_offer().await?;
//! assert!(!starting.is_finished());
//! // Inspect offer.stanza() here. The production send is still pending.
//! offer.complete()?;
//! let handle = starting.await??;
//! assert_eq!(handle.peer_jid(), *fixture.peer());
//!
//! let winner = fixture.peer().clone().with_device(2);
//! fixture.inject(NodeBuilder::new("call")
//!     .attr("from", winner.clone())
//!     .attr("id", "SYNTHETIC-ACCEPT")
//!     .attr("t", "1788840000")
//!     .children([NodeBuilder::new("accept")
//!         .attr("call-id", handle.call_id())
//!         .attr("call-creator", fixture.client().lid().unwrap())
//!         .children([
//!             NodeBuilder::new("audio").attr("enc", "opus").attr("rate", "16000").build(),
//!             NodeBuilder::new("video").attr("dec", "H264").attr("device_orientation", "0").build(),
//!         ]).build()])
//!     .build()).await?;
//! assert_eq!(handle.peer_jid(), winner);
//! fixture.shutdown().await?;
//! # Ok(())
//! # }
//! ```

mod session;
pub(crate) use session::seed_peer_session;

#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
mod call;
#[cfg(all(feature = "test-support", not(target_arch = "wasm32")))]
pub use call::{CallFixture, PendingOffer};
