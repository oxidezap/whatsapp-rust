use wacore::types::group_call::{
    CallLink, CallLinkJoin, CallLinkMedia, CallLinkPreview, GroupCallDevice, GroupCallEncRekey,
    GroupCallParticipant, GroupCallRelay, GroupCallRelayEndpoint, GroupCallUpdate, ScreenShare,
    ScreenShareState, WaitingRoom, WaitingRoomUser,
};
use wacore_binary::{Jid, Server};

#[test]
fn public_group_call_payloads_are_constructible_with_builders() {
    let creator = Jid::new("111111111111111", Server::Lid);
    let device = GroupCallDevice::builder()
        .jid(creator.clone().with_device(1))
        .build();
    assert!(device.platform.is_none());
    assert!(device.pid.is_none());
    assert!(device.capability_version.is_none());
    let participant = GroupCallParticipant::builder()
        .jid(creator.clone())
        .pn(Jid::new("12025550123", Server::Pn))
        .state("connected".to_string())
        .devices(vec![device])
        .build();
    let serialized = serde_json::to_value(&participant).expect("serialize participant");
    assert!(
        serialized.get("pn").is_none(),
        "phone-number aliases are internal routing data"
    );
    let update = GroupCallUpdate::builder()
        .call_id("GROUP-CALL".to_string())
        .call_creator(creator)
        .transaction_id(1)
        .media("audio".to_string())
        .connected_limit(32)
        .joinable(true)
        .av_upgradable(true)
        .rekey_requested(false)
        .participants(vec![participant])
        .build();
    assert_eq!(update.call_id, "GROUP-CALL");

    let endpoint = GroupCallRelayEndpoint::builder()
        .relay_id(1)
        .token_id(0)
        .auth_token_id(0)
        .relay_name("gru1c02".to_string())
        .is_fna(false)
        .build();
    assert_eq!(endpoint.relay_name, "gru1c02");
    let relay = GroupCallRelay::builder()
        .uuid("TEST-RELAY".to_string())
        .participant_uuid("TEST-PARTICIPANT".to_string())
        .attribute_padding(false)
        .endpoints(vec![endpoint])
        .build();
    assert_eq!(relay.endpoints.len(), 1);

    let link = CallLink::builder()
        .token("TEST-CALL-LINK".to_string())
        .media(CallLinkMedia::Audio)
        .build();
    assert_eq!(link.token, "TEST-CALL-LINK");

    let creator = Jid::new("111111111111111", Server::Lid);
    let preview = CallLinkPreview::builder()
        .token("TEST-CALL-LINK".to_string())
        .media(CallLinkMedia::Audio)
        .creator(creator.clone())
        .waiting_room_enabled(true)
        .is_admin(false)
        .build();
    assert!(preview.waiting_room_enabled);

    let user = WaitingRoomUser::builder()
        .jid(Jid::new("222222222222222", Server::Lid))
        .state("pending".to_string())
        .build();
    let room = WaitingRoom::builder()
        .call_id("GROUP-CALL".to_string())
        .call_creator(creator.clone())
        .link_token("TEST-CALL-LINK".to_string())
        .media(CallLinkMedia::Audio)
        .enabled(true)
        .is_admin(false)
        .users(vec![user])
        .build();
    assert_eq!(room.users.len(), 1);

    let join = CallLinkJoin::builder()
        .token("TEST-CALL-LINK".to_string())
        .media(CallLinkMedia::Audio)
        .call_id("GROUP-CALL".to_string())
        .call_creator(creator)
        .waiting_room_enabled(false)
        .in_waiting_room(false)
        .is_admin(false)
        .build();
    assert!(!join.in_waiting_room);

    let rekey = GroupCallEncRekey::builder()
        .call_id("GROUP-CALL".to_string())
        .call_creator(Jid::new("111111111111111", Server::Lid))
        .transaction_id(2)
        .key_generation(3)
        .encryption_type("msg".to_string())
        .encryption_version(4)
        .ciphertext(vec![1, 2, 3])
        .build();
    assert_eq!(rekey.transaction_id, 2);
    assert_eq!(rekey.key_generation, 3);
    assert_eq!(rekey.encryption_type, "msg");
    assert_eq!(rekey.encryption_version, 4);
    assert_eq!(rekey.ciphertext, [1, 2, 3]);

    let share = ScreenShare::builder()
        .state(ScreenShareState::Started)
        .version(2)
        .build();
    assert_eq!(share.version, 2);
    assert!(share.screen_share_id.is_none());
}
