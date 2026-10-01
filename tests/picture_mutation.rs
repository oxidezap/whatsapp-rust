//! Public imports and boxed futures for explicit set/remove operations.
use std::{future::Future, pin::Pin};
use whatsapp_rust::features::{
    GroupError as FeatureGroupError, NewsletterError as FeatureNewsletterError,
    ProfileError as FeatureProfileError,
};
use whatsapp_rust::wacore_binary::Jid;
use whatsapp_rust::{Client, GroupError, NewsletterError, ProfileError};

fn mutation_calls<'a>(
    client: &'a Client,
    group: Jid,
    channel: &'a Jid,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
    Box::pin(async move {
        client.profile().set_profile_picture(vec![1]).await?;
        client.profile().remove_profile_picture().await?;
        client
            .groups()
            .set_profile_picture(group.clone(), vec![1])
            .await?;
        client.groups().remove_profile_picture(group).await?;
        client.newsletter().set_picture(channel, &[1]).await?;
        client.newsletter().remove_picture(channel).await?;
        Ok(())
    })
}

#[test]
fn public_picture_mutations_and_typed_empty_errors_compile() {
    let _ = mutation_calls;
    let _: FeatureProfileError = ProfileError::EmptyPicture;
    let _: FeatureGroupError = GroupError::EmptyPicture;
    let _: FeatureNewsletterError = NewsletterError::EmptyPicture;
}
