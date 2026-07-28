//! Runtime state machine for group-call membership and participant controls.
//!
//! The server owns the roster. Updates are transaction ordered and membership never merges
//! incrementally: an accepted roster replaces the previous one atomically. Optional relay omission
//! means no allocation refresh, so the last usable relay is carried across roster-only updates.

use std::collections::{HashMap, HashSet};

use wacore_binary::Jid;

use crate::types::group_call::{
    GROUP_CALL_MAX_PARTICIPANTS, GroupCallUpdate, ScreenShare, ScreenShareState, WaitingRoom,
};

/// Result of applying an authoritative group or waiting-room update.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupStateApply {
    Applied,
    Stale,
    UnknownCall,
    IdentityMismatch,
    InvalidSnapshot,
}

/// Portable per-call state for authoritative snapshots and participant controls.
#[derive(Debug, Clone)]
pub struct GroupCallState {
    call_id: String,
    call_creator: Jid,
    snapshot: Option<GroupCallUpdate>,
    waiting_room: Option<WaitingRoom>,
    waiting_room_transaction: Option<u32>,
    raised_hands: HashSet<Jid>,
    screen_shares: HashMap<Jid, ScreenShare>,
}

impl GroupCallState {
    pub fn new(call_id: impl Into<String>, call_creator: Jid) -> Self {
        Self {
            call_id: call_id.into(),
            call_creator,
            snapshot: None,
            waiting_room: None,
            waiting_room_transaction: None,
            raised_hands: HashSet::new(),
            screen_shares: HashMap::new(),
        }
    }

    pub fn snapshot(&self) -> Option<&GroupCallUpdate> {
        self.snapshot.as_ref()
    }

    pub fn waiting_room(&self) -> Option<&WaitingRoom> {
        self.waiting_room.as_ref()
    }

    pub fn raised_hands(&self) -> &HashSet<Jid> {
        &self.raised_hands
    }

    pub fn screen_shares(&self) -> &HashMap<Jid, ScreenShare> {
        &self.screen_shares
    }

    /// Replace the current authoritative snapshot when its transaction is newer.
    pub fn apply_update(&mut self, mut update: GroupCallUpdate) -> GroupStateApply {
        if !self.matches_identity(&update.call_id, &update.call_creator) {
            return GroupStateApply::IdentityMismatch;
        }
        if !valid_group_snapshot(&update) {
            return GroupStateApply::InvalidSnapshot;
        }
        if self
            .snapshot
            .as_ref()
            .is_some_and(|current| update.transaction_id <= current.transaction_id)
        {
            return GroupStateApply::Stale;
        }
        if let Some(current_group_jid) = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.group_jid.as_ref())
        {
            match update.group_jid.as_ref() {
                Some(group_jid) if group_jid != current_group_jid => {
                    return GroupStateApply::IdentityMismatch;
                }
                None => update.group_jid = Some(current_group_jid.clone()),
                Some(_) => {}
            }
        }

        let connected = update
            .participants
            .iter()
            .filter(|participant| participant.is_connected())
            .flat_map(|participant| {
                std::iter::once(participant.jid.to_non_ad())
                    .chain(participant.pn.as_ref().map(Jid::to_non_ad))
            })
            .collect::<HashSet<_>>();
        self.raised_hands
            .retain(|participant| connected.contains(participant));
        self.screen_shares
            .retain(|participant, _| connected.contains(participant));
        if update.relay.is_none() {
            // Roster-only updates do not revoke the relay allocation. The media engine follows the
            // same absent-means-no-refresh rule, so the durable snapshot must retain the last usable
            // relay for builders that attach after this transaction commits.
            update.relay = self
                .snapshot
                .as_ref()
                .and_then(|snapshot| snapshot.relay.clone());
        }
        self.snapshot = Some(update);
        GroupStateApply::Applied
    }

    /// Replace waiting-room state when its optional transaction is not stale.
    pub fn apply_waiting_room(&mut self, room: WaitingRoom) -> GroupStateApply {
        if !self.matches_identity(&room.call_id, &room.call_creator) {
            return GroupStateApply::IdentityMismatch;
        }
        if self.waiting_room.as_ref().is_some_and(|current| {
            current.link_token != room.link_token || current.media != room.media
        }) {
            return GroupStateApply::IdentityMismatch;
        }
        if let (Some(current), Some(next)) = (self.waiting_room_transaction, room.transaction_id)
            && next <= current
        {
            return GroupStateApply::Stale;
        }
        if let Some(transaction_id) = room.transaction_id {
            self.waiting_room_transaction = Some(
                self.waiting_room_transaction
                    .map_or(transaction_id, |current| current.max(transaction_id)),
            );
        }
        self.waiting_room = Some(room);
        GroupStateApply::Applied
    }

    pub fn set_waiting_room_enabled(&mut self, enabled: bool) -> bool {
        self.waiting_room.as_mut().is_some_and(|room| {
            room.enabled = enabled;
            true
        })
    }

    pub fn set_raised_hand(&mut self, participant: &Jid, raised: bool) {
        let participant = participant.to_non_ad();
        if raised {
            self.raised_hands.insert(participant);
        } else {
            self.raised_hands.remove(&participant);
        }
    }

    pub fn set_screen_share(&mut self, participant: &Jid, screen_share: ScreenShare) {
        let participant = participant.to_non_ad();
        if screen_share.state == ScreenShareState::Stopped {
            self.screen_shares.remove(&participant);
        } else {
            self.screen_shares.insert(participant, screen_share);
        }
    }

    fn matches_identity(&self, call_id: &str, creator: &Jid) -> bool {
        self.call_id == call_id && self.call_creator == *creator
    }
}

impl crate::stats::HeapSize for GroupCallState {
    fn heap_bytes(&self) -> usize {
        use core::mem::size_of;

        use crate::stats::HeapSize;

        self.call_id.heap_bytes()
            + self.call_creator.heap_bytes()
            + self.snapshot.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.waiting_room.as_ref().map_or(0, HeapSize::heap_bytes)
            + self.raised_hands.capacity() * size_of::<Jid>()
            + self
                .raised_hands
                .iter()
                .map(HeapSize::heap_bytes)
                .sum::<usize>()
            + self.screen_shares.capacity() * size_of::<(Jid, ScreenShare)>()
            + self
                .screen_shares
                .keys()
                .map(HeapSize::heap_bytes)
                .sum::<usize>()
    }
}

fn valid_group_snapshot(update: &GroupCallUpdate) -> bool {
    if update.transaction_id == 0
        || update.connected_limit == 0
        || update.connected_limit as usize > GROUP_CALL_MAX_PARTICIPANTS
        || update.participants.len() > GROUP_CALL_MAX_PARTICIPANTS
        || !matches!(update.media.as_str(), "audio" | "video")
    {
        return false;
    }
    if update
        .relay
        .as_ref()
        .and_then(|relay| relay.transaction_id)
        .is_some_and(|transaction_id| transaction_id != update.transaction_id)
    {
        return false;
    }

    let mut users = HashSet::with_capacity(update.participants.len());
    let mut pids = HashSet::new();
    let mut devices = HashSet::new();
    update.participants.iter().all(|participant| {
        users.insert(participant.jid.to_non_ad())
            && participant.devices.iter().all(|device| {
                devices.insert(device.jid.clone())
                    && device.pid.is_none_or(|pid| pid != 0 && pids.insert(pid))
            })
    }) && super::group_media::validate_group_media_snapshot(update).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::group_call::{GroupCallDevice, GroupCallParticipant, GroupCallRelay};
    use crate::voip::ssrc::{derive_wasm_participant_ssrc, format_e2e_srtp_participant_id};
    use wacore_binary::Server;

    fn creator() -> Jid {
        Jid::new("100001", Server::Lid).with_device(1)
    }

    fn participant(user: &str, state: &str, pid: u32) -> GroupCallParticipant {
        GroupCallParticipant {
            jid: Jid::new(user, Server::Lid),
            pn: None,
            state: Some(state.to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new(user, Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(pid),
                capability_version: None,
                capability: Vec::new(),
            }],
        }
    }

    fn update(transaction_id: u32, users: Vec<GroupCallParticipant>) -> GroupCallUpdate {
        GroupCallUpdate {
            call_id: "CALL".to_string(),
            call_creator: creator(),
            group_jid: None,
            transaction_id,
            media: "video".to_string(),
            connected_limit: 32,
            joinable: true,
            av_upgradable: true,
            rekey_requested: false,
            participants: users,
            relay: None,
        }
    }

    fn waiting_room(transaction_id: Option<u32>) -> WaitingRoom {
        WaitingRoom {
            call_id: "CALL".to_string(),
            call_creator: creator(),
            link_token: "TEST-CALL-LINK".to_string(),
            media: crate::types::group_call::CallLinkMedia::Audio,
            enabled: true,
            is_admin: false,
            transaction_id,
            users: Vec::new(),
        }
    }

    #[test]
    fn snapshots_are_transaction_ordered_and_clear_departed_controls() {
        let mut state = GroupCallState::new("CALL", creator());
        let alice = Jid::new("200002", Server::Lid);
        assert_eq!(
            state.apply_update(update(
                3,
                vec![
                    participant("100001", "connected", 1),
                    participant("200002", "connected", 2),
                ],
            )),
            GroupStateApply::Applied
        );
        state.set_raised_hand(&alice, true);
        state.set_screen_share(
            &alice,
            ScreenShare {
                state: ScreenShareState::Started,
                version: 2,
                screen_share_id: Some(7),
            },
        );

        assert_eq!(
            state.apply_update(update(3, vec![participant("100001", "connected", 1)],)),
            GroupStateApply::Stale
        );
        assert!(state.raised_hands().contains(&alice));
        assert_eq!(
            state.apply_update(update(4, vec![participant("100001", "connected", 1)],)),
            GroupStateApply::Applied
        );
        assert!(state.raised_hands().is_empty());
        assert!(state.screen_shares().is_empty());
    }

    #[test]
    fn roster_only_updates_retain_the_last_usable_relay() {
        let mut state = GroupCallState::new("CALL", creator());
        let relay = GroupCallRelay::builder()
            .transaction_id(1)
            .self_pid(7)
            .uuid("RELAY-UUID".to_string())
            .participant_uuid("PARTICIPANT-UUID".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(16)
            .endpoints(Vec::new())
            .build();
        let mut admitted = update(1, vec![participant("100001", "connected", 1)]);
        admitted.relay = Some(relay.clone());
        assert_eq!(state.apply_update(admitted), GroupStateApply::Applied);

        assert_eq!(
            state.apply_update(update(2, vec![participant("100001", "connected", 1)],)),
            GroupStateApply::Applied
        );
        assert_eq!(
            state
                .snapshot()
                .and_then(|snapshot| snapshot.relay.as_ref()),
            Some(&relay),
            "an omitted relay is a roster-only update, not allocation revocation"
        );
    }

    #[test]
    fn roster_only_updates_retain_group_identity_and_reject_retagging() {
        let mut state = GroupCallState::new("CALL", creator());
        let group_jid = Jid::new("1234567890-1111111111", Server::Group);
        let mut admitted = update(1, vec![participant("100001", "connected", 1)]);
        admitted.group_jid = Some(group_jid.clone());
        assert_eq!(state.apply_update(admitted), GroupStateApply::Applied);

        assert_eq!(
            state.apply_update(update(2, vec![participant("100001", "connected", 1)],)),
            GroupStateApply::Applied
        );
        assert_eq!(
            state
                .snapshot()
                .and_then(|snapshot| snapshot.group_jid.as_ref()),
            Some(&group_jid),
            "an omitted group_jid is a roster-only update, not an identity change"
        );

        let mut conflicting = update(3, vec![participant("100001", "connected", 1)]);
        conflicting.group_jid = Some(Jid::new("9876543210-2222222222", Server::Group));
        assert_eq!(
            state.apply_update(conflicting),
            GroupStateApply::IdentityMismatch
        );
        assert_eq!(
            state
                .snapshot()
                .map(|snapshot| (snapshot.transaction_id, snapshot.group_jid.as_ref())),
            Some((2, Some(&group_jid))),
            "a conflicting group_jid must not consume the transaction or retag the call"
        );
    }

    #[test]
    fn invalid_identity_duplicate_pid_device_and_oversized_limit_are_rejected() {
        let mut state = GroupCallState::new("CALL", creator());
        let mut wrong = update(1, vec![participant("100001", "connected", 1)]);
        wrong.call_id = "OTHER".to_string();
        assert_eq!(state.apply_update(wrong), GroupStateApply::IdentityMismatch);

        let duplicate_pid = update(
            1,
            vec![
                participant("100001", "connected", 1),
                participant("200002", "connected", 1),
            ],
        );
        assert_eq!(
            state.apply_update(duplicate_pid),
            GroupStateApply::InvalidSnapshot
        );

        let mut second = participant("200002", "connected", 2);
        second.devices[0].jid = Jid::new("100001", Server::Lid).with_device(1);
        let duplicate_device = update(1, vec![participant("100001", "connected", 1), second]);
        assert_eq!(
            state.apply_update(duplicate_device),
            GroupStateApply::InvalidSnapshot,
            "one device identity cannot belong to two roster participants"
        );

        let mut oversized = update(1, vec![]);
        oversized.connected_limit = 33;
        assert_eq!(
            state.apply_update(oversized),
            GroupStateApply::InvalidSnapshot
        );
    }

    #[test]
    fn route_collisions_do_not_consume_the_roster_transaction() {
        let mut state = GroupCallState::new("CALL", creator());
        let first = "37774";
        let second = "53838";
        let first_id = format_e2e_srtp_participant_id(
            &Jid::new(first, Server::Lid).with_device(1).to_string(),
        );
        let second_id = format_e2e_srtp_participant_id(
            &Jid::new(second, Server::Lid).with_device(1).to_string(),
        );
        assert_eq!(
            derive_wasm_participant_ssrc("CALL", &first_id, 0),
            derive_wasm_participant_ssrc("CALL", &second_id, 0),
            "fixture must collide in the audio route"
        );

        assert_eq!(
            state.apply_update(update(
                1,
                vec![
                    participant(first, "connected", 1),
                    participant(second, "connected", 2),
                ],
            )),
            GroupStateApply::InvalidSnapshot
        );
        assert_eq!(
            state.apply_update(update(1, vec![participant(first, "connected", 1)])),
            GroupStateApply::Applied,
            "a corrected redelivery must retain the rejected transaction ID"
        );
    }

    #[test]
    fn stopped_screen_share_and_lowered_hand_remove_state() {
        let mut state = GroupCallState::new("CALL", creator());
        let alice = Jid::new("200002", Server::Lid).with_device(3);
        state.set_raised_hand(&alice, true);
        state.set_screen_share(
            &alice,
            ScreenShare {
                state: ScreenShareState::Started,
                version: 2,
                screen_share_id: Some(9),
            },
        );
        assert_eq!(state.raised_hands().len(), 1);
        assert_eq!(state.screen_shares().len(), 1);

        state.set_raised_hand(&alice, false);
        state.set_screen_share(
            &alice,
            ScreenShare {
                state: ScreenShareState::Stopped,
                version: 2,
                screen_share_id: None,
            },
        );
        assert!(state.raised_hands().is_empty());
        assert!(state.screen_shares().is_empty());
    }

    #[test]
    fn waiting_room_identity_and_transaction_order_are_enforced() {
        let mut state = GroupCallState::new("CALL", creator());
        assert_eq!(
            state.apply_waiting_room(waiting_room(Some(4))),
            GroupStateApply::Applied
        );
        assert_eq!(
            state.apply_waiting_room(waiting_room(Some(3))),
            GroupStateApply::Stale
        );
        assert_eq!(
            state.apply_waiting_room(waiting_room(None)),
            GroupStateApply::Applied,
            "transaction-less service updates always apply"
        );
        assert_eq!(
            state.apply_waiting_room(waiting_room(Some(3))),
            GroupStateApply::Stale,
            "transaction-less updates must not erase the numbered watermark"
        );

        let mut wrong_call = waiting_room(Some(5));
        wrong_call.call_id = "OTHER".to_string();
        assert_eq!(
            state.apply_waiting_room(wrong_call),
            GroupStateApply::IdentityMismatch
        );
        let mut wrong_creator = waiting_room(Some(5));
        wrong_creator.call_creator = Jid::new("999999", Server::Lid);
        assert_eq!(
            state.apply_waiting_room(wrong_creator),
            GroupStateApply::IdentityMismatch
        );
        let mut wrong_link = waiting_room(Some(5));
        wrong_link.link_token = "OTHER-CALL-LINK".to_string();
        assert_eq!(
            state.apply_waiting_room(wrong_link),
            GroupStateApply::IdentityMismatch
        );
        let mut wrong_media = waiting_room(Some(5));
        wrong_media.media = crate::types::group_call::CallLinkMedia::Video;
        assert_eq!(
            state.apply_waiting_room(wrong_media),
            GroupStateApply::IdentityMismatch
        );
        assert_eq!(
            state.apply_waiting_room(waiting_room(Some(5))),
            GroupStateApply::Applied,
            "conflicting link identities must not consume the transaction watermark"
        );
    }

    #[test]
    fn pn_alias_controls_survive_unrelated_roster_updates() {
        let mut state = GroupCallState::new("CALL", creator());
        let mut alice = participant("200002", "connected", 2);
        let alice_pn = Jid::new("12025550111", Server::Pn);
        alice.pn = Some(alice_pn.clone());
        assert_eq!(
            state.apply_update(update(
                1,
                vec![participant("100001", "connected", 1), alice.clone()],
            )),
            GroupStateApply::Applied
        );
        state.set_raised_hand(&alice_pn, true);
        state.set_screen_share(
            &alice_pn,
            ScreenShare {
                state: ScreenShareState::Started,
                version: 2,
                screen_share_id: Some(7),
            },
        );

        assert_eq!(
            state.apply_update(update(
                2,
                vec![participant("100001", "connected", 1), alice],
            )),
            GroupStateApply::Applied
        );
        assert!(state.raised_hands().contains(&alice_pn));
        assert!(state.screen_shares().contains_key(&alice_pn));
    }
}
