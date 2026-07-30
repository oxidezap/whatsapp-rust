use wacore::types::call::{CallAction, IncomingCall};
use wacore_binary::{Jid, Server};

#[test]
fn incoming_call_uses_the_public_forward_compatible_builder() {
    let creator = Jid::new("111111111111111", Server::Lid);
    let participant = creator.clone().with_device(2);
    let call = IncomingCall::builder()
        .from(Jid::new("CALL-ID-0001", Server::Call))
        .stanza_id("STANZA-ID-0001".to_string())
        .maybe_participant(Some(participant.clone()))
        .timestamp(wacore::time::from_secs(1_700_000_000).expect("timestamp"))
        .offline(false)
        .action(CallAction::RelayLatency {
            call_id: "CALL-ID-0001".to_string(),
            call_creator: creator,
        })
        .build();

    assert_eq!(call.participant, Some(participant));
    assert_eq!(call.action.call_id(), "CALL-ID-0001");
    assert_eq!(call.ringing_generation(), None);
}
