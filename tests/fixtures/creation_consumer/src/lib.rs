//! External construction and host-implementable trait proof, with no session.
use wa::prelude::{Client, CreatedEvent, CreatedPoll, Jid, StoredMessageSecret};
use wa::traits::{MsgSecretEntry, MsgSecretStore};
use wa::wacore::store::error::{Result, StoreError};
use wa::{EventCreationParams, EventResponseType, async_trait};

pub struct ReadOnlyStore {
    pub bytes: Vec<u8>,
    pub timestamp: i64,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl MsgSecretStore for ReadOnlyStore {
    async fn put_msg_secrets(&self, _: Vec<MsgSecretEntry>) -> Result<usize> {
        Err(StoreError::Validation("read-only fixture".into()))
    }
    async fn get_stored_msg_secret(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> Result<Option<StoredMessageSecret>> {
        StoredMessageSecret::from_stored_bytes(&self.bytes, self.timestamp).map(Some)
    }
    async fn delete_expired_msg_secrets(&self, _: i64) -> Result<u32> {
        Err(StoreError::Validation("read-only fixture".into()))
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait CreationOperations: Send + Sync {
    async fn create_and_use(&self, chat: &Jid) -> wa::anyhow::Result<(CreatedPoll, CreatedEvent)>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl CreationOperations for Client {
    async fn create_and_use(&self, chat: &Jid) -> wa::anyhow::Result<(CreatedPoll, CreatedEvent)> {
        let poll = self
            .polls()
            .create_quiz(chat, "Quiz", &["A".into(), "B".into()], 0)
            .await?;
        self.polls().vote(&poll.poll_ref()?, &["A".into()]).await?;
        let event = self
            .events()
            .create(
                chat,
                EventCreationParams::builder().name("Launch".into()).build(),
            )
            .await?;
        self.events()
            .respond(&event.event_ref()?, EventResponseType::Going, None)
            .await?;
        Ok((poll, event))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wa::prelude::{EventRef, MessageId, MessageRef, MessageSecret, PollRef};

    #[test]
    fn references_are_externally_constructible_and_borrow_key_material() {
        let chat = Jid::pn("15550000001");
        let creator = Jid::lid("100000000000001");
        let secret = MessageSecret::try_from(vec![177; 32]).unwrap();
        let message = MessageRef::new(
            &chat,
            MessageId::new("CREATION").unwrap(),
            Some(&chat), // PN sender and its externally known LID crypto alias
            false,
        )
        .unwrap();
        let poll = PollRef::new(message.clone(), &creator, &secret).unwrap();
        let event = EventRef::new(message, &creator, &secret).unwrap();
        assert_eq!(poll.message().id().as_str(), "CREATION");
        assert_eq!(event.creator(), &creator);
        assert!(std::ptr::eq(poll.secret(), &secret));
        assert!(!format!("{poll:#?} {event:#?} {secret:#?}").contains("177"));
    }

    #[test]
    fn external_named_read_preserves_metadata_and_typed_invalid_data() {
        for (timestamp, expected) in [(0, None), (123, Some(123))] {
            let store: &dyn MsgSecretStore = &ReadOnlyStore {
                bytes: vec![177; 32],
                timestamp,
            };
            let row = futures::executor::block_on(store.get_stored_msg_secret("c", "s", "m"))
                .unwrap()
                .unwrap();
            assert_eq!(row.message_ts, expected);
            assert_eq!(row.secret.into_bytes(), [177; 32]);
        }
        let invalid = ReadOnlyStore {
            bytes: vec![177; 31],
            timestamp: 123,
        };
        let error =
            futures::executor::block_on(invalid.get_stored_msg_secret("c", "s", "m")).unwrap_err();
        assert!(matches!(error, StoreError::InvalidMessageSecret(_)));
    }
}
