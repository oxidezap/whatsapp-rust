//! Ownership checks against a real facade handle, outside the library package boundary.
#![cfg(all(feature = "test-support", not(target_arch = "wasm32")))]

use std::{sync::Arc, time::Duration};
use whatsapp_rust::{test_support::CallFixture, voip::CallHandle};

async fn dormant(fixture: &CallFixture) -> anyhow::Result<CallHandle> {
    let client = fixture.client().clone();
    let peer = fixture.peer().clone();
    let start = tokio::spawn(async move {
        let (_microphone, microphone) = async_channel::bounded::<Vec<i16>>(1);
        let (speaker, _speaker) = async_channel::bounded::<Vec<i16>>(1);
        client
            .voip()
            .call(&peer)
            .audio(microphone, speaker)
            .start()
            .await
    });
    fixture.next_offer().await?.complete()?;
    Ok(start.await??)
}

#[tokio::test]
async fn live_clones_share_acquisition_and_receiver_does_not_retain_client() -> anyhow::Result<()> {
    let fixture = CallFixture::new().await?;
    let client = Arc::downgrade(fixture.client());
    let call = dormant(&fixture).await?;
    let clone = call.clone();
    let events = call.take_events().expect("first owner");
    assert!(clone.take_events().is_none());
    drop(call);
    assert!(
        clone.take_events().is_none(),
        "dropping the acquiring handle does not restore ownership"
    );
    assert!(
        fixture.call_snapshot(clone.call_id()).is_some(),
        "handle Drop does not hang up"
    );
    clone.hangup_local().await;
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(1), clone.wait_ended()).await?;
    }
    fixture.shutdown().await?;
    drop(fixture);
    tokio::time::timeout(Duration::from_secs(1), async {
        while client.upgrade().is_some() {
            tokio::task::yield_now().await;
        }
    })
    .await?;
    // Neither a remaining control handle nor the independently owned receiver keeps Client alive.
    assert!(clone.take_events().is_none());
    drop(events);
    assert!(clone.take_events().is_none());
    Ok(())
}

#[tokio::test]
async fn shutdown_before_first_acquisition_preserves_sticky_completion() -> anyhow::Result<()> {
    let fixture = CallFixture::new().await?;
    let call = dormant(&fixture).await?;
    fixture.shutdown().await?;
    let events = call
        .take_events()
        .expect("ending does not consume acquisition");
    for _ in 0..2 {
        tokio::time::timeout(Duration::from_secs(1), call.wait_ended()).await?;
    }
    assert!(call.clone().take_events().is_none());
    drop(events);
    assert!(call.take_events().is_none());
    Ok(())
}
