use std::sync::Mutex;
use whatsapp_rust::buffa::{
    DecodeContext, DecodeError, MessageView, OwnedView, bytes::Bytes, encoding::Tag,
};

static LEAK: Mutex<Option<&'static [u8]>> = Mutex::new(None);
struct LeakingView(&'static [u8]);
impl MessageView<'static> for LeakingView {
    type Owned = whatsapp_rust::waproto::whatsapp::MessageKey;
    fn decode_view(input: &'static [u8]) -> Result<Self, DecodeError> {
        whatsapp_rust::waproto::codec::message_key_decode(input)?;
        *LEAK.lock().unwrap() = Some(input);
        Ok(Self(input))
    }
    fn merge_view_field(
        &mut self,
        _: Tag,
        input: &'static [u8],
        _: &'static [u8],
        _: DecodeContext<'_>,
    ) -> Result<&'static [u8], DecodeError> {
        Ok(&input[input.len()..])
    }
    fn to_owned_message(&self) -> Result<Self::Owned, DecodeError> {
        whatsapp_rust::waproto::codec::message_key_decode(self.0)
    }
}

pub fn forbidden_lifetime_escape() {
    let owner = OwnedView::<LeakingView>::decode(Bytes::from(vec![10, 1, b'x'])).unwrap();
    drop(owner);
    let leaked = LEAK.lock().unwrap().unwrap();
    assert_eq!(leaked, [10, 1, b'x']);
}
