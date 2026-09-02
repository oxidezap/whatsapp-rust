use crate::libsignal::protocol::PreKeyBundle;
use crate::types::message::AddressingMode;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use wacore_binary::CompactString;
use wacore_binary::Jid;

/// One LID→PN mapping. Kept as `(lid, phone jid)` rather than `(lid, phone
/// user + server)` because `Jid` is 32 bytes and a `CompactString` plus a
/// `Server` pads to the same 32: storing the whole JID costs nothing and lets
/// [`GroupInfo::phone_jid_for_lid_user`] keep handing out a borrow. The server
/// is not assumed to be `Pn` — a malformed response can map a LID to another
/// LID, and callers such as `collect_stale_device_users` check for that.
type LidPnPair = (CompactString, Jid);

/// Which side of a pair an ordering keys on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OrderBy {
    Lid,
    Phone,
}

/// Order `order` in place, `le` deciding whether the first index sorts at or
/// before the second.
///
/// A hand-written bottom-up merge sort, not `sort_unstable_by`. pdqsort
/// monomorphizes into thirteen specialized routines per (element type,
/// comparator type) — `sort13_optimal`, `sort9_optimal`, `ipnsort`, two
/// partition variants, a heapsort fallback and more — which a symbol-level
/// diff of `wacore`'s release object measured at **15.6 KiB of `.text` for
/// this single call site**, half of the crate's whole growth. Nothing here is
/// hot enough to buy that: it runs once per group-metadata fetch and once per
/// membership notification, over at most a few thousand `u32`s.
///
/// The comparator is a trait object rather than a generic parameter so this
/// routine exists exactly once however many orderings are added later — the
/// property the previous spelling had to get by funnelling every caller
/// through one closure, now a guarantee of the signature.
fn merge_sort_indices(order: &mut Vec<u32>, le: &dyn Fn(u32, u32) -> bool) {
    let n = order.len();
    if n < 2 {
        return;
    }
    let mut src = std::mem::take(order);
    let mut dst = vec![0u32; n];
    let mut width = 1;
    while width < n {
        let mut start = 0;
        while start < n {
            let mid = (start + width).min(n);
            let end = (start + 2 * width).min(n);
            let (mut left, mut right, mut out) = (start, mid, start);
            while left < mid && right < end {
                if le(src[left], src[right]) {
                    dst[out] = src[left];
                    left += 1;
                } else {
                    dst[out] = src[right];
                    right += 1;
                }
                out += 1;
            }
            // Exactly one run still has elements, and it fills the rest of
            // this block.
            let rest = if left < mid {
                &src[left..mid]
            } else {
                &src[right..end]
            };
            dst[out..end].copy_from_slice(rest);
            start = end;
        }
        std::mem::swap(&mut src, &mut dst);
        width *= 2;
    }
    *order = src;
}

/// Indices `0..pairs.len()` ordered by one side of the pairs.
///
/// Sorts `u32` indices rather than the pairs themselves: the elements moved
/// are 4 bytes instead of 56, and the permutation is what both callers want
/// anyway. Ties keep input order, so a rebuild of the same input yields the
/// same slice.
fn ordered_indices(pairs: &[LidPnPair], by: OrderBy) -> Vec<u32> {
    let mut order: Vec<u32> = (0..pairs.len() as u32).collect();
    merge_sort_indices(&mut order, &|a, b| {
        let (a, b) = (&pairs[a as usize], &pairs[b as usize]);
        match by {
            OrderBy::Lid => a.0 <= b.0,
            OrderBy::Phone => a.1.user.cmp(&b.1.user).then_with(|| a.0.cmp(&b.0)).is_le(),
        }
    });
    order
}

/// Put a pair list in LID order.
///
/// Permutes rather than sorts, for the reason in [`ordered_indices`]: the
/// order is computed over indices and applied here.
fn sort_pairs(pairs: Vec<LidPnPair>) -> Box<[LidPnPair]> {
    let order = ordered_indices(&pairs, OrderBy::Lid);
    // `mem::take` rather than wrapping each slot in an `Option`: a pair is
    // `Default`, every index appears exactly once, and the alternative asks
    // the binary to carry a second set of `Vec` codegen for
    // `Option<LidPnPair>` to say the same thing.
    let mut slots = pairs;
    order
        .into_iter()
        .map(|i| std::mem::take(&mut slots[i as usize]))
        .collect()
}

/// Order `lid_pn` by the PN user part, returning indices into it.
///
/// **One slot per phone user.** The reverse `HashMap` this replaced was keyed
/// by phone user and so could hold only one LID per number; keeping every
/// duplicate here would answer reverse lookups differently from the map and
/// make `remove_participants` drop mappings it used to leave alone. Two LIDs
/// claiming one phone number is a server bug either way; the map resolved it
/// by iteration order, i.e. arbitrarily, and this resolves it to the last LID
/// in sort order — deterministic, and stable across rebuilds.
fn build_pn_order(lid_pn: &[LidPnPair]) -> Box<[u32]> {
    let mut order = ordered_indices(lid_pn, OrderBy::Phone);
    // `dedup_by` keeps the first of each run; the winner is the last, so the
    // comparison hands the later index to the entry that survives.
    order.dedup_by(|later, earlier| {
        let same = lid_pn[*later as usize].1.user == lid_pn[*earlier as usize].1.user;
        if same {
            *earlier = *later;
        }
        same
    });
    order.into_boxed_slice()
}

/// Sort by LID user and drop the `HashMap`'s excess capacity in one step.
fn build_lid_pn(map: HashMap<CompactString, Jid>) -> Box<[LidPnPair]> {
    sort_pairs(map.into_iter().collect())
}

fn serialize_lid_pn<S: serde::Serializer>(
    lid_pn: &[LidPnPair],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(Some(lid_pn.len()))?;
    for (lid_user, phone_jid) in lid_pn.iter() {
        map.serialize_entry(lid_user, phone_jid)?;
    }
    map.end()
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
    /// LID→PN mappings, sorted by the LID user part, looked up by binary
    /// search. Used for device queries, since LID usync requests may not work
    /// reliably.
    ///
    /// A sorted slice rather than a `HashMap`: a 1024-member group needs 2048
    /// hashbrown buckets, so roughly half the map's bytes were empty slots,
    /// and the entries are read far more often than they are written (member
    /// changes arrive on notifications; lookups run once per participant per
    /// send). Serialized as the `lid_to_pn_map` object the previous layout
    /// wrote, so the persisted `group_metadata` blob is unchanged.
    #[serde(rename = "lid_to_pn_map", serialize_with = "serialize_lid_pn")]
    lid_pn: Box<[LidPnPair]>,
    /// Reverse index: indices into `lid_pn` ordered by the PN user part.
    ///
    /// Derived, so it is not persisted. Four bytes per mapping instead of the
    /// 48 the reverse `HashMap` spent re-storing both identifier strings.
    #[serde(skip)]
    pn_order: Box<[u32]>,
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
            lid_pn: Box::default(),
            pn_order: Box::default(),
        }
    }

    /// Create a [`GroupInfo`] and populate the LID-to-phone mapping.
    pub fn with_lid_to_pn_map(
        participants: Vec<Jid>,
        addressing_mode: AddressingMode,
        lid_to_pn_map: HashMap<CompactString, Jid>,
    ) -> Self {
        let lid_pn = build_lid_pn(lid_to_pn_map);
        let pn_order = build_pn_order(&lid_pn);

        Self {
            participants,
            addressing_mode,
            is_community_announce: None,
            lid_pn,
            pn_order,
        }
    }

    /// Replace the current LID-to-phone mapping.
    pub fn set_lid_to_pn_map(&mut self, lid_to_pn_map: HashMap<CompactString, Jid>) {
        self.lid_pn = build_lid_pn(lid_to_pn_map);
        self.pn_order = build_pn_order(&self.lid_pn);
    }

    /// Rebuild both slices from an edited pair list. The reverse index is
    /// derived, so every write goes through here and no caller can leave the
    /// two out of step.
    fn store_pairs(&mut self, pairs: Vec<LidPnPair>) {
        self.lid_pn = sort_pairs(pairs);
        self.pn_order = build_pn_order(&self.lid_pn);
    }

    /// Position of `lid_user` in the LID-sorted pair list.
    fn lid_index(&self, lid_user: &str) -> Option<usize> {
        self.lid_pn
            .binary_search_by(|(lid, _)| lid.as_str().cmp(lid_user))
            .ok()
    }

    /// Position in `lid_pn` of the mapping whose phone user is `phone_user`,
    /// found through the reverse index.
    fn pn_index(&self, phone_user: &str) -> Option<usize> {
        self.pn_order
            .binary_search_by(|slot| self.lid_pn[*slot as usize].1.user.as_str().cmp(phone_user))
            .ok()
            .map(|slot| self.pn_order[slot] as usize)
    }

    /// Look up the mapped phone-number JID for a given LID user identifier.
    pub fn phone_jid_for_lid_user(&self, lid_user: &str) -> Option<&Jid> {
        self.lid_index(lid_user).map(|i| &self.lid_pn[i].1)
    }

    /// Look up the mapped LID user for a given phone number (user part).
    pub fn lid_user_for_phone_user(&self, phone_user: &str) -> Option<&CompactString> {
        self.pn_index(phone_user).map(|i| &self.lid_pn[i].0)
    }

    /// Append participants that are not already present.
    ///
    /// For LID-addressed groups, also updates the LID→PN mappings using the
    /// `phone_number` field from each participant. Mappings are updated even
    /// for already-present participants so that a later call with
    /// `Some(phone_number)` backfills a previous `None` entry.
    pub fn add_participants<'a, I>(&mut self, new: I)
    where
        I: IntoIterator<Item = (&'a Jid, Option<&'a Jid>)>,
    {
        // The pair list is rebuilt once for the whole batch, not once per
        // added member: sorting a 1024-entry slice per notification is
        // cheaper than the n insertions it replaces, and a notification that
        // maps nobody (the common PN-group case) never touches it at all.
        let mut pairs: Option<Vec<LidPnPair>> = None;
        for (jid, phone_number) in new {
            // Always backfill the LID mapping — a re-add with phone_number
            // fills a previous None (e.g., client-initiated add followed by
            // server notification that carries the phone number).
            if self.addressing_mode == AddressingMode::Lid
                && let Some(pn) = phone_number
            {
                let pairs = pairs.get_or_insert_with(|| self.lid_pn.to_vec());
                match pairs.iter_mut().find(|(lid, _)| *lid == jid.user) {
                    Some((_, mapped)) => *mapped = pn.clone(),
                    None => pairs.push((jid.user.clone(), pn.clone())),
                }
            }

            if self.participants.iter().any(|p| p.user == jid.user) {
                continue;
            }
            self.participants.push(jid.clone());
        }
        // Membership changes arrive one notification at a time and the list
        // is read for hours between them, so trade a realloc per change for
        // no doubling slack: a group that grew past its allocation by one
        // member otherwise kept a second, empty copy of itself resident.
        self.participants.shrink_to_fit();
        if let Some(pairs) = pairs {
            self.store_pairs(pairs);
        }
    }

    /// Remove participants whose user part is in `users_to_remove`.
    ///
    /// Also drops the LID→PN mappings naming them, on either side.
    pub fn remove_participants(&mut self, users_to_remove: &[&str]) {
        self.participants
            .retain(|p| !users_to_remove.iter().any(|u| *u == p.user));
        self.participants.shrink_to_fit();
        // Each name can be either side of a mapping. The phone side is
        // resolved through the reverse index first, so a phone number shared
        // by two LIDs drops exactly the one that index names — which is what
        // removing it from the reverse `HashMap` used to do.
        let doomed_lids: Vec<CompactString> = users_to_remove
            .iter()
            .filter_map(|user| self.pn_index(user).map(|i| self.lid_pn[i].0.clone()))
            .collect();
        if !doomed_lids.is_empty() || users_to_remove.iter().any(|u| self.lid_index(u).is_some()) {
            let mut pairs = self.lid_pn.to_vec();
            pairs.retain(|(lid, _)| {
                !users_to_remove.contains(&lid.as_str()) && !doomed_lids.contains(lid)
            });
            self.store_pairs(pairs);
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
        let lid_pn = self.lid_pn.len() * size_of::<LidPnPair>()
            + self
                .lid_pn
                .iter()
                .map(|(k, v)| k.heap_bytes() + v.heap_bytes())
                .sum::<usize>();
        let pn_order = self.pn_order.len() * size_of::<u32>();
        participants + lid_pn + pn_order
    }
}

/// Opaque RAII holder for the per-device pairwise session locks a
/// [`SendContextResolver`] acquires around the group SKDM fan-out. Wacore holds it
/// across the fan-out and drops it to release; the concrete guard type lives in the
/// platform crate, since the per-address lock cache is not part of the portable core.
#[must_use = "the session locks release the moment this guard is dropped"]
pub struct SessionLockGuard(
    // Held purely for its `Drop` (releases the locks); never read.
    #[allow(dead_code)] Option<Box<dyn crate::sync_marker::MaybeSendSync>>,
);

impl SessionLockGuard {
    /// No locks held — the default resolver behavior (tests/benches don't race).
    pub fn none() -> Self {
        Self(None)
    }

    /// Hold `guards` until this value is dropped.
    pub fn hold(guards: Box<dyn crate::sync_marker::MaybeSendSync>) -> Self {
        Self(Some(guards))
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

    /// Returns the bundles alongside the devices the server rejected by name,
    /// so a per-device rejection is not flattened into "no bundle" before the
    /// fan-out can tell the two apart.
    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> Result<crate::prekeys::PreKeyFetchOutcome, anyhow::Error>;

    async fn resolve_group_info(&self, jid: &Jid) -> Result<Arc<GroupInfo>, anyhow::Error>;

    /// Get the LID (Linked ID) for a phone number, if known.
    /// This is used to find existing sessions that were established under a LID address
    /// when sending to a phone number address.
    ///
    /// Returns None if no LID mapping is known for this phone number.
    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<CompactString> {
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

    /// Notify that `count` devices were dropped from this send's recipient set
    /// for `reason`, so the drop lands on a counter instead of only in a log.
    ///
    /// Dropping them is deliberate and unchanged. Default is a no-op; like
    /// [`Self::on_local_identity_change`], the resolver is the only handle back
    /// to the client available inside the encrypt fan-out.
    fn on_unkeyable_devices(&self, reason: crate::stats::UnkeyableDevice, count: u64) {
        let _ = (reason, count);
    }

    /// Acquire the per-device pairwise session locks for the SKDM fan-out targets,
    /// in the same deadlock-free order the DM send path uses, so a group send and a
    /// concurrent DM (or another group send) sharing a device can't advance that
    /// device's pairwise ratchet at once and drop a chain step. The `sender_key_lock`
    /// only serializes the sender-key chain, not these pairwise sessions. Default:
    /// no-op — tests and benches don't race concurrent sends.
    async fn lock_device_sessions(&self, device_jids: &[Jid]) -> SessionLockGuard {
        let _ = device_jids;
        SessionLockGuard::none()
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

    /// A hand-written sort earns a direct test, not only the oracle below:
    /// merge sorts fail at the block boundaries, on odd lengths, and on runs
    /// of equal keys, and an ordering bug there would surface as a lookup
    /// miss far from here.
    #[test]
    fn the_index_sort_orders_every_length_and_keeps_ties_in_input_order() {
        for n in 0..=65usize {
            // Keys deliberately collide in threes, so most lengths carry runs
            // of equal elements across a merge boundary.
            let keys: Vec<u32> = (0..n as u32).map(|i| (i * 7 % 13) / 3).collect();
            let mut order: Vec<u32> = (0..n as u32).collect();
            merge_sort_indices(&mut order, &|a, b| keys[a as usize] <= keys[b as usize]);

            assert_eq!(order.len(), n, "length {n}: the permutation lost entries");
            let mut seen = order.clone();
            seen.sort_unstable();
            assert_eq!(
                seen,
                (0..n as u32).collect::<Vec<_>>(),
                "length {n}: not a permutation of the input indices"
            );
            for w in order.windows(2) {
                let (a, b) = (w[0] as usize, w[1] as usize);
                assert!(keys[a] <= keys[b], "length {n}: out of order at {a},{b}");
                if keys[a] == keys[b] {
                    assert!(a < b, "length {n}: a tie was reordered ({a} after {b})");
                }
            }
        }
    }

    /// The sorted slices replace two `HashMap`s, so every edit path has to
    /// leave both of them ordered and in step. This drives adds and removes
    /// against a `HashMap` oracle and checks the invariants after each step.
    #[test]
    fn pair_slices_stay_consistent_with_a_map_oracle() {
        let mut info = GroupInfo::new(Vec::new(), AddressingMode::Lid);
        let mut oracle: HashMap<CompactString, Jid> = HashMap::new();

        // A deterministic walk that mixes fresh adds, re-adds that remap an
        // existing LID, removals by the LID side and removals by the PN side.
        for round in 0..40u32 {
            let lid_user = CompactString::from(format!("lid_{}", round % 13));
            // One phone number per LID: the duplicate case has its own test,
            // and mixing it in here would only let the oracle drift.
            let pn_user = CompactString::from(format!("pn_{}", round % 13));
            let lid_jid = Jid::lid(lid_user.clone());
            let pn_jid = Jid::pn(pn_user.clone());

            if round % 5 == 4 {
                let victim = format!("lid_{}", round % 11);
                info.remove_participants(&[victim.as_str()]);
                if let Some(pn) = oracle.remove(victim.as_str()) {
                    let _ = pn;
                }
                oracle.retain(|_, mapped| mapped.user != victim.as_str());
            } else if round % 7 == 6 {
                let victim = format!("pn_{}", round % 11);
                info.remove_participants(&[victim.as_str()]);
                oracle.retain(|lid, mapped| {
                    lid.as_str() != victim.as_str() && mapped.user != victim.as_str()
                });
            } else {
                info.add_participants([(&lid_jid, Some(&pn_jid))]);
                oracle.insert(lid_user.clone(), pn_jid.clone());
            }

            assert!(
                info.lid_pn.windows(2).all(|w| w[0].0 < w[1].0),
                "lid_pn must stay sorted and unique by LID: {:?}",
                info.lid_pn
                    .iter()
                    .map(|(l, _)| l.as_str())
                    .collect::<Vec<_>>()
            );
            assert_eq!(
                info.pn_order.len(),
                info.lid_pn.len(),
                "with one phone number per LID the reverse index loses nothing"
            );
            assert!(
                info.pn_order
                    .windows(2)
                    .all(|w| info.lid_pn[w[0] as usize].1.user < info.lid_pn[w[1] as usize].1.user),
                "pn_order must stay ordered by the PN user"
            );

            assert_eq!(
                info.lid_pn.len(),
                oracle.len(),
                "round {round}: entry count diverged from the oracle"
            );
            for (lid_user, phone_jid) in &oracle {
                assert_eq!(
                    info.phone_jid_for_lid_user(lid_user),
                    Some(phone_jid),
                    "round {round}: forward lookup diverged for {lid_user}"
                );
                // The reverse direction only promises *a* LID for a phone
                // number; two LIDs claiming one PN is a server bug, not a
                // shape this has to preserve. What it must never do is
                // answer with a LID that is not mapped to that PN.
                let reverse = info
                    .lid_user_for_phone_user(&phone_jid.user)
                    .expect("reverse lookup must find some LID");
                assert_eq!(
                    oracle.get(reverse).map(|j| &j.user),
                    Some(&phone_jid.user),
                    "round {round}: reverse lookup pointed at an unmapped LID"
                );
            }
        }
    }

    /// `remove_participants` takes user parts without saying which namespace
    /// they are in, so naming the phone side has to drop the mapping too.
    #[test]
    fn remove_by_phone_side_drops_the_mapping() {
        let lid_to_pn = HashMap::from([(CompactString::from("lid_bob"), pn("bob_pn"))]);
        let mut info =
            GroupInfo::with_lid_to_pn_map(vec![lid("lid_bob")], AddressingMode::Lid, lid_to_pn);

        info.remove_participants(&["bob_pn"]);

        assert!(info.phone_jid_for_lid_user("lid_bob").is_none());
        assert!(info.lid_user_for_phone_user("bob_pn").is_none());
    }

    /// A malformed response can map a LID to something that is not a phone
    /// number. The pair keeps the whole JID rather than a user part plus an
    /// assumed `Pn` server, so callers can still see what it really was.
    #[test]
    fn a_non_pn_mapping_survives_the_round_trip() {
        let lid_to_pn = HashMap::from([(
            CompactString::from("100000000000007"),
            Jid::lid("100000000000099"),
        )]);
        let info = GroupInfo::with_lid_to_pn_map(Vec::new(), AddressingMode::Lid, lid_to_pn);

        let mapped = info
            .phone_jid_for_lid_user("100000000000007")
            .expect("mapping present");
        assert!(!mapped.is_pn());
        assert_eq!(mapped.user.as_str(), "100000000000099");
    }

    /// Two LIDs claiming one phone number is a server bug, but it has to
    /// resolve the same way every time. The reverse `HashMap` this replaced
    /// was keyed by phone user, so it held one LID per number and resolved
    /// collisions by iteration order — arbitrarily. The reverse index keeps
    /// the same one-slot-per-number shape and picks the last LID in sort
    /// order, which survives a rebuild.
    #[test]
    fn two_lids_claiming_one_phone_number_resolve_deterministically() {
        let shared = pn("shared_pn");
        let lid_to_pn = HashMap::from([
            (CompactString::from("lid_aaa"), shared.clone()),
            (CompactString::from("lid_zzz"), shared.clone()),
        ]);
        let mut info = GroupInfo::with_lid_to_pn_map(
            vec![lid("lid_aaa"), lid("lid_zzz")],
            AddressingMode::Lid,
            lid_to_pn,
        );

        assert_eq!(info.pn_order.len(), 1, "one slot per phone number");
        assert_eq!(
            info.lid_user_for_phone_user("shared_pn")
                .map(|u| u.as_str()),
            Some("lid_zzz")
        );

        // Removing by the phone side drops the mapping that slot names and
        // leaves the other alone, which is what removing the entry from the
        // reverse map used to do.
        info.remove_participants(&["shared_pn"]);
        assert!(info.phone_jid_for_lid_user("lid_zzz").is_none());
        assert!(info.phone_jid_for_lid_user("lid_aaa").is_some());
        assert_eq!(
            info.lid_user_for_phone_user("shared_pn")
                .map(|u| u.as_str()),
            Some("lid_aaa"),
            "the survivor takes the slot on the rebuild"
        );
    }

    /// The layout exists to bound what a resident group snapshot costs, so the
    /// bound is asserted rather than left to a comment. A 1024-member LID
    /// group holds, per participant: a 32-byte `Jid`, a 56-byte
    /// `(CompactString, Jid)` pair and a 4-byte reverse index — 92 bytes, with
    /// every identifier short enough to live inline in its `CompactString`.
    /// The two `HashMap`s this replaced needed 2048 buckets for the same 1024
    /// entries and spent 212 bytes per participant.
    #[test]
    fn retained_bytes_per_participant_stay_bounded() {
        use crate::stats::HeapSize;

        const N: usize = 1024;
        let mut participants = Vec::with_capacity(N);
        let mut lid_to_pn = HashMap::with_capacity(N);
        for i in 0..N {
            let lid_user = CompactString::from(format!("1000000{i:08}"));
            let pn_user = CompactString::from(format!("5511{i:09}"));
            participants.push(Jid::lid(lid_user.clone()));
            lid_to_pn.insert(lid_user, Jid::pn(pn_user));
        }
        let info = GroupInfo::with_lid_to_pn_map(participants, AddressingMode::Lid, lid_to_pn);

        let per_participant = (size_of::<GroupInfo>() + info.heap_bytes()) / N;
        assert!(
            per_participant <= 92,
            "a resident LID group must stay within 92 B/participant, got {per_participant}"
        );
    }
}
