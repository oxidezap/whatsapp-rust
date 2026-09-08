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
    start_with_video(fixture, true)
}

fn start_with_video(
    fixture: &CallFixture,
    video: bool,
) -> tokio::task::JoinHandle<Result<CallHandle, whatsapp_rust::CallError>> {
    let client = fixture.client().clone();
    let peer = fixture.peer().clone();
    tokio::spawn(async move {
        let (_mic, mic) = async_channel::bounded::<Vec<i16>>(1);
        let (speaker, _speaker) = async_channel::bounded::<Vec<i16>>(1);
        let (_video, video_rx) = async_channel::bounded::<Vec<u8>>(1);
        let (sink, _sink) = async_channel::bounded::<wacore::voip::VideoFrame>(1);
        let voip = client.voip();
        let builder = voip.call(&peer).audio(mic, speaker);
        let builder = if video {
            builder.video(video_rx, sink)
        } else {
            builder
        };
        builder.start().await
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

#[tokio::test]
async fn peer_video_queue_keeps_sender_state_and_upgrade_token_in_order() -> Result<()> {
    use wacore::types::call::VideoState;
    use wacore::voip::CallEvent;

    let fixture = CallFixture::new().await?;
    let starting = start_with_video(&fixture, false);
    fixture.next_offer().await?.complete()?;
    let handle = starting.await??;
    let winner = fixture.peer().clone().with_device(2);
    let sibling = fixture.peer().clone().with_device(0);
    fixture
        .inject(action(
            &fixture,
            handle.call_id(),
            winner.clone(),
            "accept",
            None,
        ))
        .await?;
    let creator = fixture.client().lid().unwrap();
    let video_stanza = |source: &Jid, state: VideoState, orientation: u8| {
        NodeBuilder::new("call")
            .attr("from", source.clone())
            .attr("id", format!("VIDEO-{}", state.code()))
            .attr("t", "1788840000")
            .children([NodeBuilder::new("video")
                .attr("call-id", handle.call_id())
                .attr("call-creator", creator.clone())
                .attr("state", state.code().to_string())
                .attr("device_orientation", orientation.to_string())
                .attr("dec", "H264")
                .build()])
            .build()
    };
    let (_source_tx, source) = async_channel::bounded::<Vec<u8>>(1);
    let (sink, _sink_rx) = async_channel::bounded::<wacore::voip::VideoFrame>(1);
    handle.start_video(source.clone(), sink.clone()).await?;
    fixture
        .inject(video_stanza(&winner, VideoState::UpgradeAccept, 1))
        .await?;
    fixture
        .inject(video_stanza(&sibling, VideoState::Stopped, 2))
        .await?;
    handle.stop_video().await?;
    fixture
        .inject(video_stanza(&winner, VideoState::UpgradeRequestV2, 3))
        .await?;
    fixture
        .inject(video_stanza(&sibling, VideoState::Stopped, 0))
        .await?;

    let events = handle.events();
    let queued: Vec<_> = std::iter::from_fn(|| events.try_recv().ok()).collect();
    assert_eq!(
        queued.len(),
        8,
        "one source event and one legacy event per committed direct state"
    );
    let observed: Vec<_> = queued
        .chunks_exact(2)
        .map(|pair| {
            let CallEvent::PeerVideoStateChanged {
                source,
                call_creator,
                state,
                orientation,
                upgrade_token,
                ..
            } = &pair[0]
            else {
                panic!("expected source-bearing event, got {:?}", pair[0]);
            };
            assert_eq!(call_creator, &creator);
            assert_eq!(
                pair[1],
                CallEvent::VideoStateChanged {
                    state: *state,
                    orientation: *orientation,
                    upgrade_token: *upgrade_token,
                },
                "legacy construction remains source-compatible and tokens match exactly"
            );
            (source.clone(), *state, *orientation, *upgrade_token)
        })
        .collect();
    assert_eq!(
        observed.iter().map(|event| event.1).collect::<Vec<_>>(),
        [
            VideoState::UpgradeAccept,
            VideoState::Stopped,
            VideoState::UpgradeRequestV2,
            VideoState::Stopped,
        ]
    );
    assert_eq!(
        observed
            .iter()
            .map(|event| event.0.clone())
            .collect::<Vec<_>>(),
        [
            winner.clone(),
            sibling.clone(),
            winner.clone(),
            sibling.clone(),
        ],
        "the ordered handle queue must identify each actual sender without consulting the global event bus"
    );
    let request = observed[2]
        .3
        .expect("the source-bearing request must retain its token");
    assert_eq!(
        observed.iter().map(|event| event.2).collect::<Vec<_>>(),
        [Some(1), Some(2), Some(3), Some(0)]
    );
    assert!(
        observed
            .iter()
            .enumerate()
            .all(|(index, event)| (index == 2) == event.3.is_some())
    );
    assert!(matches!(
        handle.accept_video(request, source, sink).await,
        Err(whatsapp_rust::CallError::VideoUpgradeExpired)
    ));
    assert_eq!(
        handle.peer_jid(),
        winner,
        "metadata must not change current winner policy"
    );
    fixture.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn peer_video_metadata_reports_routed_sender_and_supplied_creator_without_new_authorization()
-> Result<()> {
    use wacore::types::call::VideoState;
    use wacore::voip::CallEvent;

    let fixture = CallFixture::new().await?;
    let handle = dormant(&fixture).await?;
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
    let source = fixture.peer().clone().with_device(0);
    let supplied_creator = Jid::new("444444444444444", Server::Lid);
    let video = |state: VideoState| {
        NodeBuilder::new("call")
            .attr("from", winner.clone())
            .attr("participant", source.clone())
            .attr("id", "ROUTED-VIDEO")
            .attr("t", "1788840000")
            .children([NodeBuilder::new("video")
                .attr("call-id", handle.call_id())
                .attr("call-creator", supplied_creator.clone())
                .attr("state", state.code().to_string())
                .build()])
            .build()
    };
    fixture.inject(video(VideoState::Stopped)).await?;
    let events = handle.events();
    assert_eq!(
        events.try_recv()?,
        CallEvent::PeerVideoStateChanged {
            source: source.clone(),
            call_creator: supplied_creator.clone(),
            state: VideoState::Stopped,
            orientation: None,
            upgrade_token: None,
        }
    );
    assert_eq!(
        events.try_recv()?,
        CallEvent::VideoStateChanged {
            state: VideoState::Stopped,
            orientation: None,
            upgrade_token: None,
        }
    );
    fixture.inject(video(VideoState::Unknown(99))).await?;
    assert!(
        events.is_empty(),
        "an ignored transition must publish neither variant"
    );
    assert_eq!(handle.peer_jid(), winner);
    fixture.shutdown().await?;
    Ok(())
}
