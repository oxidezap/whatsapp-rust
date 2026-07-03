use crate::libsignal::protocol::PreKeyBundle;
use crate::types::message::AddressingMode;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use wacore_binary::CompactString;
use wacore_binary::Jid;

fn build_pn_to_lid_map(
    lid_to_pn_map: &HashMap<CompactString, Jid>,
) -> HashMap<CompactString, CompactString> {
    lid_to_pn_map
        .iter()
        .map(|(lid_user, phone_jid)| (phone_jid.user.clone(), lid_user.clone()))
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(from = "GroupInfoDe")]
pub struct GroupInfo {
    pub participants: Vec<Jid>,
    pub addressing_mode: AddressingMode,
    /// Whether this group is a Community Announcement Group (WA Web `isCag`,
    /// derived from `default_sub_group`). `None` means the persisted blob
    /// predates the field, so the answer is unknown and callers must re-query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_community_announce: Option<bool>,
    /// Maps a LID user identifier (the `user` part of the LID JID) to the
    /// corresponding phone-number JID. This is used for device queries since
    /// LID usync requests may not work reliably.
    lid_to_pn_map: HashMap<CompactString, Jid>,
    /// Reverse index: phone user → LID user. Derived from `lid_to_pn_map`
    /// (the LID jid is reconstructed on demand), so it is neither persisted
    /// nor stores Jids — for a large LID group that halves the map bytes in
    /// memory and in the serialized `group_metadata` blob.
    #[serde(skip)]
    pn_to_lid_map: HashMap<CompactString, CompactString>,
}

/// Deserialization shadow: rebuilds the derived reverse index. Old blobs
/// carrying the previously-persisted `pn_to_lid_map` field still decode —
/// serde_json ignores unknown fields.
#[derive(serde::Deserialize)]
struct GroupInfoDe {
    participants: Vec<Jid>,
    addressing_mode: AddressingMode,
    #[serde(default)]
    is_community_announce: Option<bool>,
    #[serde(default)]
    lid_to_pn_map: HashMap<CompactString, Jid>,
}

impl From<GroupInfoDe> for GroupInfo {
    fn from(d: GroupInfoDe) -> Self {
        let mut info = Self::with_lid_to_pn_map(d.participants, d.addressing_mode, d.lid_to_pn_map);
        info.is_community_announce = d.is_community_announce;
        info
    }
}

impl GroupInfo {
    /// Create a [`GroupInfo`] with the provided participants and addressing mode.
    ///
    /// The LID-to-phone mapping defaults to empty. Call
    /// [`GroupInfo::set_lid_to_pn_map`] or [`GroupInfo::with_lid_to_pn_map`] to
    /// populate it when a mapping is available.
    pub fn new(participants: Vec<Jid>, addressing_mode: AddressingMode) -> Self {
        Self {
            participants,
            addressing_mode,
            is_community_announce: None,
            lid_to_pn_map: HashMap::new(),
            pn_to_lid_map: HashMap::new(),
        }
    }

    /// Create a [`GroupInfo`] and populate the LID-to-phone mapping.
    pub fn with_lid_to_pn_map(
        participants: Vec<Jid>,
        addressing_mode: AddressingMode,
        lid_to_pn_map: HashMap<CompactString, Jid>,
    ) -> Self {
        let pn_to_lid_map = build_pn_to_lid_map(&lid_to_pn_map);

        Self {
            participants,
            addressing_mode,
            is_community_announce: None,
            lid_to_pn_map,
            pn_to_lid_map,
        }
    }

    /// Replace the current LID-to-phone mapping.
    pub fn set_lid_to_pn_map(&mut self, lid_to_pn_map: HashMap<CompactString, Jid>) {
        self.pn_to_lid_map = build_pn_to_lid_map(&lid_to_pn_map);
        self.lid_to_pn_map = lid_to_pn_map;
    }

    /// Look up the mapped phone-number JID for a given LID user identifier.
    pub fn phone_jid_for_lid_user(&self, lid_user: &str) -> Option<&Jid> {
        self.lid_to_pn_map.get(lid_user)
    }

    /// Look up the mapped LID user for a given phone number (user part).
    pub fn lid_user_for_phone_user(&self, phone_user: &str) -> Option<&CompactString> {
        self.pn_to_lid_map.get(phone_user)
    }

    /// Append participants that are not already present.
    ///
    /// For LID-addressed groups, also updates the LID-to-PN and PN-to-LID maps
    /// using the `phone_number` field from each participant.  Maps are updated
    /// even for already-present participants so that a later call with
    /// `Some(phone_number)` backfills a previous `None` entry.
    pub fn add_participants<'a, I>(&mut self, new: I)
    where
        I: IntoIterator<Item = (&'a Jid, Option<&'a Jid>)>,
    {
        for (jid, phone_number) in new {
            // Always backfill LID maps — a re-add with phone_number fills a
            // previous None (e.g., client-initiated add followed by server
            // notification that carries the phone number).
            if self.addressing_mode == AddressingMode::Lid
                && let Some(pn) = phone_number
            {
                self.pn_to_lid_map.insert(pn.user.clone(), jid.user.clone());
                self.lid_to_pn_map.insert(jid.user.clone(), pn.clone());
            }

            if self.participants.iter().any(|p| p.user == jid.user) {
                continue;
            }
            self.participants.push(jid.clone());
        }
    }

    /// Remove participants whose user part is in `users_to_remove`.
    ///
    /// Also cleans up the LID-to-PN and PN-to-LID maps.
    pub fn remove_participants(&mut self, users_to_remove: &[&str]) {
        self.participants
            .retain(|p| !users_to_remove.iter().any(|u| *u == p.user));
        for user in users_to_remove {
            if let Some(pn_jid) = self.lid_to_pn_map.remove(*user) {
                self.pn_to_lid_map.remove(&pn_jid.user);
            }
            // Also try reverse: user might be a PN
            if let Some(lid_user) = self.pn_to_lid_map.remove(*user) {
                self.lid_to_pn_map.remove(&lid_user);
            }
        }
    }

    /// Convert a phone-based device JID to a LID-based device JID using the mapping,
    /// consuming the JID. If no mapping exists, returns it unchanged.
    pub fn phone_device_jid_into_lid(&self, phone_device_jid: Jid) -> Jid {
        if phone_device_jid.is_pn()
            && let Some(lid_user) = self.lid_user_for_phone_user(&phone_device_jid.user)
        {
            return Jid::lid_device(lid_user.clone(), phone_device_jid.device);
        }
        phone_device_jid
    }
}

impl crate::stats::HeapSize for GroupInfo {
    fn heap_bytes(&self) -> usize {
        let participants = self.participants.capacity() * size_of::<Jid>()
            + self
                .participants
                .iter()
                .map(|j| j.heap_bytes())
                .sum::<usize>();
        let lid_to_pn = self.lid_to_pn_map.capacity() * size_of::<(CompactString, Jid)>()
            + self
                .lid_to_pn_map
                .iter()
                .map(|(k, v)| k.heap_bytes() + v.heap_bytes())
                .sum::<usize>();
        let pn_to_lid = self.pn_to_lid_map.capacity() * size_of::<(CompactString, CompactString)>()
            + self
                .pn_to_lid_map
                .iter()
                .map(|(k, v)| k.heap_bytes() + v.heap_bytes())
                .sum::<usize>();
        participants + lid_to_pn + pn_to_lid
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SendContextResolver: crate::sync_marker::MaybeSendSync {
    async fn resolve_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error>;

    async fn fetch_prekeys(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error>;

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error>;

    async fn resolve_group_info(&self, jid: &Jid) -> Result<Arc<GroupInfo>, anyhow::Error>;

    /// Get the LID (Linked ID) for a phone number, if known.
    /// This is used to find existing sessions that were established under a LID address
    /// when sending to a phone number address.
    ///
    /// Returns None if no LID mapping is known for this phone number.
    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<wacore_binary::CompactString> {
        // Default implementation returns None - subclasses can override
        let _ = phone_user;
        None
    }

    /// Notify that establishing a session for `jid` replaced a previously-stored
    /// identity key (local detection of a peer identity change on the send path).
    ///
    /// Default is a no-op; the high-level client reacts off-path (mirrors WA Web
    /// `saveIdentity` -> `handleNewIdentity`). The resolver is the only handle
    /// back to the client available inside `encrypt_for_devices`.
    fn on_local_identity_change(&self, jid: &Jid) {
        let _ = jid;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pn(user: &str) -> Jid {
        Jid::pn(user)
    }
    fn lid(user: &str) -> Jid {
        Jid::lid(user)
    }

    #[test]
    fn add_participants_pn_mode() {
        let mut info = GroupInfo::new(vec![pn("alice")], AddressingMode::Pn);
        let bob = pn("bob");
        let carol = pn("carol");
        info.add_participants([(&bob, None), (&carol, None)]);
        assert_eq!(info.participants.len(), 3);
        assert!(info.participants.iter().any(|p| p.user == "bob"));
    }

    #[test]
    fn add_participants_deduplicates() {
        let mut info = GroupInfo::new(vec![pn("alice"), pn("bob")], AddressingMode::Pn);
        let bob = pn("bob");
        let carol = pn("carol");
        info.add_participants([(&bob, None), (&carol, None)]);
        assert_eq!(info.participants.len(), 3); // bob not duplicated
    }

    #[test]
    fn add_participants_lid_mode_updates_maps() {
        let mut info = GroupInfo::new(vec![lid("lid_alice")], AddressingMode::Lid);
        let bob_lid = lid("lid_bob");
        let bob_pn = pn("bob_pn");
        info.add_participants([(&bob_lid, Some(&bob_pn))]);

        assert_eq!(info.participants.len(), 2);
        assert_eq!(
            info.phone_jid_for_lid_user("lid_bob")
                .map(|j| j.user.as_str()),
            Some("bob_pn")
        );
        assert_eq!(
            info.lid_user_for_phone_user("bob_pn").map(|u| u.as_str()),
            Some("lid_bob")
        );
    }

    #[test]
    fn remove_participants_basic() {
        let mut info = GroupInfo::new(
            vec![pn("alice"), pn("bob"), pn("carol")],
            AddressingMode::Pn,
        );
        info.remove_participants(&["bob"]);
        assert_eq!(info.participants.len(), 2);
        assert!(!info.participants.iter().any(|p| p.user == "bob"));
    }

    #[test]
    fn remove_participants_cleans_lid_maps() {
        let lid_to_pn = HashMap::from([
            (CompactString::from("lid_alice"), pn("alice_pn")),
            (CompactString::from("lid_bob"), pn("bob_pn")),
        ]);
        let mut info = GroupInfo::with_lid_to_pn_map(
            vec![lid("lid_alice"), lid("lid_bob")],
            AddressingMode::Lid,
            lid_to_pn,
        );

        assert!(info.phone_jid_for_lid_user("lid_bob").is_some());
        assert!(info.lid_user_for_phone_user("bob_pn").is_some());

        info.remove_participants(&["lid_bob"]);

        assert_eq!(info.participants.len(), 1);
        assert!(info.phone_jid_for_lid_user("lid_bob").is_none());
        assert!(info.lid_user_for_phone_user("bob_pn").is_none());
        assert!(info.phone_jid_for_lid_user("lid_alice").is_some());
    }

    #[test]
    fn remove_nonexistent_is_noop() {
        let mut info = GroupInfo::new(vec![pn("alice")], AddressingMode::Pn);
        info.remove_participants(&["nobody"]);
        assert_eq!(info.participants.len(), 1);
    }

    #[test]
    fn add_participants_backfills_lid_map_for_existing() {
        let mut info = GroupInfo::new(vec![lid("lid_bob")], AddressingMode::Lid);
        // First add without phone_number (simulates client-initiated add)
        let bob_lid = lid("lid_bob");
        let bob_pn = pn("bob_pn");
        info.add_participants([(&bob_lid, None)]);
        assert!(info.phone_jid_for_lid_user("lid_bob").is_none());

        // Second add with phone_number (simulates server notification backfill)
        info.add_participants([(&bob_lid, Some(&bob_pn))]);
        assert_eq!(info.participants.len(), 1); // not duplicated
        assert_eq!(
            info.phone_jid_for_lid_user("lid_bob")
                .map(|j| j.user.as_str()),
            Some("bob_pn")
        );
        assert_eq!(
            info.lid_user_for_phone_user("bob_pn").map(|u| u.as_str()),
            Some("lid_bob")
        );
    }

    /// Old persisted blobs carried a `pn_to_lid_map` field; the reverse index
    /// is now derived and skipped during serialization. Both directions must
    /// hold: old-format JSON still decodes (unknown field ignored) with the
    /// index rebuilt, and the new format omits the field entirely.
    #[test]
    fn serde_reverse_index_is_derived_not_persisted() {
        let mut map = HashMap::new();
        map.insert(CompactString::from("lid_bob"), pn("bob_pn"));
        let info = GroupInfo::with_lid_to_pn_map(vec![lid("lid_bob")], AddressingMode::Lid, map);

        let json = serde_json::to_string(&info).expect("serialize");
        assert!(
            !json.contains("pn_to_lid_map"),
            "derived index must not be persisted: {json}"
        );

        let round: GroupInfo = serde_json::from_str(&json).expect("deserialize new format");
        assert_eq!(
            round.lid_user_for_phone_user("bob_pn").map(|u| u.as_str()),
            Some("lid_bob")
        );

        // Old format: same payload plus the previously-persisted reverse map
        // (its contents are irrelevant — unknown fields are ignored).
        let mut legacy_json: serde_json::Value = serde_json::from_str(&json).expect("value");
        legacy_json["pn_to_lid_map"] = serde_json::json!({ "bob_pn": "lid_bob@lid" });
        // Jid's Deserialize borrows from the input, so go through a string.
        let legacy_str = serde_json::to_string(&legacy_json).expect("legacy json");
        let legacy: GroupInfo = serde_json::from_str(&legacy_str).expect("deserialize old format");
        assert_eq!(
            legacy.lid_user_for_phone_user("bob_pn").map(|u| u.as_str()),
            Some("lid_bob")
        );
        assert_eq!(
            legacy
                .phone_jid_for_lid_user("lid_bob")
                .map(|j| j.user.as_str()),
            Some("bob_pn")
        );
    }
}
