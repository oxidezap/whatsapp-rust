//! External-crate compile contract and intentional lossy conversion.
use std::{future::Future, pin::Pin, time::Duration};
use whatsapp_rust::features::ProfilePictureRequest as FeatureRequest;
use whatsapp_rust::wacore_binary::Jid;
use whatsapp_rust::{
    Client, ContactError, ErrorChainExt, ProfilePictureLookup, ProfilePictureRequest,
    ProfilePictureTarget, ProfilePictureType,
};

fn boxed_lookup<'a>(
    client: &'a Client,
    jid: &'a Jid,
) -> Pin<Box<dyn Future<Output = Result<ProfilePictureLookup, ContactError>> + Send + 'a>> {
    Box::pin(async move {
        client
            .contacts()
            .lookup_picture(
                ProfilePictureRequest::new(
                    ProfilePictureTarget::Group(jid),
                    ProfilePictureType::Full,
                )
                .existing_id(Some("known-photo"))
                .timeout(Some(Duration::from_secs(3))),
            )
            .await
    })
}

#[test]
fn picture_lookup_public_imports_and_boxed_future_compile() {
    let _ = boxed_lookup;
    // `server_rejection` is a trait method, including for downstream callers.
    let rejection_code = |error: &ContactError| error.server_rejection().map(|r| r.code);
    let _ = rejection_code;
    let jid = Jid::pn("15550000001");
    let request: FeatureRequest<'_> = ProfilePictureRequest::new(
        ProfilePictureTarget::Contact(&jid),
        ProfilePictureType::Preview,
    );
    assert_eq!(request.target().jid(), &jid);
}

#[test]
fn picture_lookup_into_found_explicitly_discards_non_found_states() {
    for outcome in [
        ProfilePictureLookup::Unchanged,
        ProfilePictureLookup::NotFound,
        ProfilePictureLookup::NotAuthorized,
    ] {
        assert!(outcome.into_found().is_none());
    }
}
