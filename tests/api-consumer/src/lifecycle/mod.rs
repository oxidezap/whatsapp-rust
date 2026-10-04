//! Standalone public lifecycle fixture, without workspace features or flags.
pub mod admission;
#[cfg(all(test, feature = "native", not(target_arch = "wasm32")))]
#[path = "../../../lifecycle_outcomes.rs"]
mod contract;

/// The owned waiter is nameable downstream and can move independently of Client.
pub fn incoming_waiter(
    client: &whatsapp_rust::Client,
) -> whatsapp_rust::NodeWaiter<wacore_binary::OwnedNodeRef> {
    client.wait_for_node(whatsapp_rust::NodeFilter::tag("notification"))
}

pub async fn recover_media(
    client: &whatsapp_rust::Client,
    target: whatsapp_rust::MessageRef<'_>,
    key: &[u8; 32],
) -> Result<whatsapp_rust::MediaRetryResult, whatsapp_rust::MediaReuploadError> {
    client
        .media_reupload()
        .request(&whatsapp_rust::MediaReuploadRequest {
            target,
            media_key: key,
        })
        .await
}

#[cfg(not(target_arch = "wasm32"))]
pub fn sendable_recovery<'a>(
    client: &'a whatsapp_rust::Client,
    target: whatsapp_rust::MessageRef<'a>,
    key: &'a [u8; 32],
) -> impl std::future::Future<
    Output = Result<whatsapp_rust::MediaRetryResult, whatsapp_rust::MediaReuploadError>,
> + Send
+ 'a {
    recover_media(client, target, key)
}

#[test]
fn reupload_request_debug_redacts_the_media_key() {
    let chat = whatsapp_rust::Jid::pn("15550000002");
    let target = whatsapp_rust::MessageRef::new(
        &chat,
        whatsapp_rust::MessageId::new("MEDIA").unwrap(),
        None,
        false,
    )
    .unwrap();
    let key = [231; 32];
    let request = whatsapp_rust::MediaReuploadRequest {
        target,
        media_key: &key,
    };
    let debug = format!("{request:?}");
    assert!(debug.contains("MEDIA"));
    assert!(debug.contains("REDACTED"));
    assert!(!debug.contains("231"));
}
