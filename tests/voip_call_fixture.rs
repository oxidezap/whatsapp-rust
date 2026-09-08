//! External-consumer coverage of the opt-in fixture and current handler policy.
#![cfg(all(feature = "test-support", not(target_arch = "wasm32")))]

use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use wacore::types::events::Event;
use wacore_binary::{Jid, Node, Server, builder::NodeBuilder};
use whatsapp_rust::{test_support::CallFixture, voip::CallHandle};

fn start(
    fixture: &CallFixture,
) -> tokio::task::JoinHandle<Result<CallHandle, whatsapp_rust::CallError>> {
    let client = fixture.client().clone();
    let peer = fixture.peer().clone();
    tokio::spawn(async move {
        let (_mic, mic) = async_channel::bounded::<Vec<i16>>(1);
        let (speaker, _speaker) = async_channel::bounded::<Vec<i16>>(1);
        let (_video, video) = async_channel::bounded::<Vec<u8>>(1);
        let (sink, _sink) = async_channel::bounded::<wacore::voip::VideoFrame>(1);
        client
            .voip()
            .call(&peer)
            .audio(mic, speaker)
            .video(video, sink)
            .start()
            .await
    })
}

fn action(
    fixture: &CallFixture,
    call_id: &str,
    from: Jid,
    tag: &'static str,
    reason: Option<&str>,
) -> Node {
    let mut action = NodeBuilder::new(tag)
        .attr("call-id", call_id)
        .attr("call-creator", fixture.client().lid().unwrap());
    if let Some(reason) = reason {
        action = action.attr("reason", reason);
    }
    if tag == "accept" {
        action = action.children([
            NodeBuilder::new("audio")
                .attr("enc", "opus")
                .attr("rate", "16000")
                .build(),
            NodeBuilder::new("video")
                .attr("dec", "H264")
                .attr("device_orientation", "0")
                .build(),
        ]);
    }
    NodeBuilder::new("call")
        .attr("from", from)
        .attr("id", "SYNTHETIC-ACTION")
        .attr("t", "1788840000")
        .children([action.build()])
        .build()
}

async fn dormant(fixture: &CallFixture) -> Result<CallHandle> {
    let start = start(fixture);
    fixture.next_offer().await?.complete()?;
    Ok(start.await??)
}

#[tokio::test]
async fn builder_returns_real_dormant_handle_only_after_offer_completion() -> Result<()> {
    let fixture = CallFixture::new().await?;
    assert!(fixture.client().is_connected());
    assert!(fixture.client().is_logged_in());
    fixture
        .client()
        .wait_for_connected(Duration::from_secs(1))
        .await?;
    let start = start(&fixture);
    let offer = fixture.next_offer().await?;
    assert!(!start.is_finished());
    let node = offer.stanza().as_node_ref();
    let offer_node = node.get_optional_child("offer").unwrap();
    let id = offer_node
        .attrs()
        .optional_string("call-id")
        .unwrap()
        .into_owned();
    let video = offer_node.get_optional_child("video").unwrap();
    assert_eq!(
        video.attrs().optional_string("dec").as_deref(),
        Some("H264")
    );
    assert_eq!(
        video.attrs().optional_string("enc").as_deref(),
        Some("h.264")
    );
    assert!(offer_node.get_optional_child("destination").is_some());
    offer.complete()?;
    let handle = start.await??;
    assert_eq!(handle.call_id(), id);
    assert_eq!(handle.peer_jid(), *fixture.peer());
    assert!(
        handle.events().try_recv().is_err(),
        "no relay or media readiness was invented"
    );
    fixture.shutdown().await?;
    tokio::time::timeout(Duration::from_secs(1), handle.wait_ended()).await?;
    Ok(())
}

#[tokio::test]
async fn accept_before_builder_completion_selects_winner_through_handler() -> Result<()> {
    let fixture = Arc::new(CallFixture::new().await?);
    let _raw = fixture.client().acquire_raw_node_forwarding();
    let start = start(&fixture);
    let offer = fixture.next_offer().await?;
    let id = offer
        .stanza()
        .as_node_ref()
        .get_optional_child("offer")
        .unwrap()
        .attrs()
        .optional_string("call-id")
        .unwrap()
        .into_owned();
    let winner = fixture.peer().clone().with_device(2);
    let accept = action(&fixture, &id, winner.clone(), "accept", None);
    let injection = tokio::spawn({
        let fixture = fixture.clone();
        async move { fixture.inject(accept).await }
    });
    tokio::time::timeout(Duration::from_secs(1), async {
        while fixture
            .call_snapshot(&id)
            .and_then(|session| session.answering_device)
            .as_ref()
            != Some(&winner)
        {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert!(
        !start.is_finished(),
        "the handler chose the winner while the offer send remained blocked"
    );
    assert!(
        !injection.is_finished(),
        "sibling dismissal awaits the blocked transport"
    );
    offer.complete()?;
    let handle = start.await??;
    injection.await??;
    assert_eq!(handle.peer_jid(), winner);
    assert!(
        fixture.events()?.iter().any(|event| match &**event {
            Event::RawNode(node) => node
                .get()
                .get_optional_child("accept")
                .and_then(|accept| accept.get_optional_child("video"))
                .is_some_and(
                    |video| video.attrs().optional_string("dec").as_deref() == Some("H264")
                ),
            _ => false,
        }),
        "the real incoming advertisement is available through the standard raw-node lease"
    );
    assert!(
        fixture
            .events()?
            .iter()
            .any(|event| matches!(&**event, Event::IncomingCall(call)
        if call.action.call_id() == id && call.from == winner))
    );
    let dismissals: Vec<_> = fixture
        .outgoing_stanzas()?
        .into_iter()
        .filter(|node| node.as_node_ref().get_optional_child("terminate").is_some())
        .collect();
    assert_eq!(dismissals.len(), 1);
    assert_eq!(
        dismissals[0].as_node_ref().attrs().jid("to"),
        fixture.peer().clone().with_device(0)
    );
    assert_eq!(
        dismissals[0]
            .as_node_ref()
            .get_optional_child("terminate")
            .unwrap()
            .attrs()
            .optional_string("reason")
            .as_deref(),
        Some("accepted_elsewhere")
    );
    fixture
        .inject(action(
            &fixture,
            &id,
            fixture.peer().clone().with_device(0),
            "accept",
            None,
        ))
        .await?;
    assert_eq!(
        handle.peer_jid(),
        winner,
        "late accept must not replace first winner"
    );
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn refused_offer_cleans_up_without_returning_a_handle() -> Result<()> {
    let fixture = CallFixture::new().await?;
    let start = start(&fixture);
    let offer = fixture.next_offer().await?;
    let id = offer
        .stanza()
        .as_node_ref()
        .get_optional_child("offer")
        .unwrap()
        .attrs()
        .optional_string("call-id")
        .unwrap()
        .into_owned();
    offer.fail()?;
    assert!(start.await?.is_err());
    assert!(fixture.call_snapshot(&id).is_none());
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn dropped_offer_refuses_send_and_dropped_fixture_reaps_a_real_handle() -> Result<()> {
    let fixture = CallFixture::new().await?;
    let first = start(&fixture);
    drop(fixture.next_offer().await?);
    assert!(first.await?.is_err());
    fixture.shutdown().await?;

    let fixture = CallFixture::new().await?;
    let handle = dormant(&fixture).await?;
    let client = fixture.client().clone();
    drop(fixture);
    tokio::time::timeout(Duration::from_secs(5), handle.wait_ended()).await?;
    assert!(!client.is_connected());
    Ok(())
}

#[tokio::test]
async fn busy_sibling_keeps_ringing_but_late_nonbusy_reject_currently_ends_winner() -> Result<()> {
    let fixture = CallFixture::new().await?;
    let handle = dormant(&fixture).await?;
    let sibling = fixture.peer().clone().with_device(0);
    fixture
        .inject(action(
            &fixture,
            handle.call_id(),
            sibling.clone(),
            "reject",
            Some("busy"),
        ))
        .await?;
    assert_eq!(handle.peer_jid(), *fixture.peer());
    assert!(
        !fixture
            .outgoing_stanzas()?
            .iter()
            .any(|node| node.as_node_ref().get_optional_child("terminate").is_some())
    );
    let winner = fixture.peer().clone().with_device(2);
    fixture
        .inject(action(
            &fixture,
            handle.call_id(),
            winner.clone(),
            "accept",
            None,
        ))
        .await?;
    assert_eq!(handle.peer_jid(), winner);
    fixture
        .inject(action(
            &fixture,
            handle.call_id(),
            sibling,
            "reject",
            Some("declined"),
        ))
        .await?;
    tokio::time::timeout(Duration::from_secs(1), handle.wait_ended()).await?;
    assert!(
        handle.announce_video_enabled().await.is_err(),
        "characterization of current weak rejection policy, not desired security"
    );
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn current_handler_accepts_an_unrung_device_as_first_winner() -> Result<()> {
    let fixture = CallFixture::new().await?;
    let handle = dormant(&fixture).await?;
    let uninvited = Jid::new("444444444444444", Server::Lid).with_device(7);
    fixture
        .inject(action(
            &fixture,
            handle.call_id(),
            uninvited.clone(),
            "accept",
            None,
        ))
        .await?;
    assert_eq!(
        handle.peer_jid(),
        uninvited,
        "characterize the handler, do not invent authorization in the fixture"
    );
    fixture.shutdown().await?;
    Ok(())
}
