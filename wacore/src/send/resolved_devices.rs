//! Resolved group device set with its lazily memoized phash.

use crate::messages::MessageUtils;
use std::sync::OnceLock;
use wacore_binary::CompactString;
use wacore_binary::jid::Jid;

/// The full resolved device set of a group, bundled with a memo of the
/// `phash` derived from it.
///
/// The phash is a pure function of `(devices, sending jid)`, and the holder
/// of this struct is the per-group device-list memo: any topology change
/// produces a NEW memo entry (and so a new, cold instance), which means the
/// phash inherits the device memo's invalidation rules with zero extra
/// bookkeeping. The memo cell sits behind the entry's `Arc`, so every
/// per-send clone of the entry shares one warm value.
///
/// A phash is 10 bytes ("2:" + 8 base64 chars), inline in `CompactString`:
/// serving a warm send costs a pointer-free copy, no allocation.
pub struct ResolvedGroupDevices {
    devices: Vec<Jid>,
    /// `(sending jid, phash)`. The jid pins the only other input, so a
    /// change of sending identity (PN/LID mode flip, re-pair) can never be
    /// served a stale hash; it recomputes without overwriting.
    phash: OnceLock<(Jid, CompactString)>,
}

impl crate::stats::HeapSize for ResolvedGroupDevices {
    fn heap_bytes(&self) -> usize {
        self.devices.capacity() * size_of::<Jid>()
            + self.devices.iter().map(|j| j.heap_bytes()).sum::<usize>()
            + self
                .phash
                .get()
                .map_or(0, |(jid, p)| jid.heap_bytes() + p.heap_bytes())
    }
}

impl ResolvedGroupDevices {
    pub fn new(devices: Vec<Jid>) -> Self {
        Self {
            devices,
            phash: OnceLock::new(),
        }
    }

    pub fn devices(&self) -> &[Jid] {
        &self.devices
    }

    /// The group phash for a send from `own_sending_jid`: memo hit is an
    /// inline copy; first use (or a different sender identity) computes it
    /// over the hosted-filtered device set plus the sending device.
    pub fn phash(&self, own_sending_jid: &Jid) -> Option<CompactString> {
        if let Some((jid, hash)) = self.phash.get() {
            if jid == own_sending_jid {
                return Some(hash.clone());
            }
            return Self::compute(&self.devices, own_sending_jid);
        }
        let hash = Self::compute(&self.devices, own_sending_jid)?;
        // Benign race: whichever first wins the cell. Every caller returns
        // the value computed for its OWN jid; a racer with a different jid
        // that loses the set is served by the bypass branch above afterwards.
        let _ = self.phash.set((own_sending_jid.clone(), hash.clone()));
        Some(hash)
    }

    fn compute(devices: &[Jid], own_sending_jid: &Jid) -> Option<CompactString> {
        let set = super::group::build_group_phash_set(devices, own_sending_jid);
        match MessageUtils::participant_list_hash(&set) {
            Ok(phash) => Some(CompactString::from(phash)),
            Err(e) => {
                log::warn!("Failed to compute group phash: {e:?}");
                None
            }
        }
    }
}

impl std::fmt::Debug for ResolvedGroupDevices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedGroupDevices")
            .field("devices", &self.devices.len())
            .field("phash_warm", &self.phash.get().is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jid(user: &str, device: u16) -> Jid {
        let mut j = Jid::lid(user);
        j.device = device;
        j
    }

    /// The memo must serve exactly what a direct computation produces, stay
    /// warm across calls, and bypass (without poisoning) on a different
    /// sending identity.
    #[test]
    fn phash_memo_matches_direct_compute_and_pins_sender() {
        let devices = vec![jid("100000000000001", 0), jid("100000000000002", 3)];
        let own = jid("100000000000009", 0);
        let other = jid("100000000000008", 0);

        let resolved = ResolvedGroupDevices::new(devices.clone());
        let direct = {
            let set = crate::send::group::build_group_phash_set(&devices, &own);
            MessageUtils::participant_list_hash(&set).unwrap()
        };

        let first = resolved.phash(&own).expect("phash");
        assert_eq!(first.as_str(), direct);
        assert!(resolved.phash.get().is_some(), "memo warmed on first use");
        assert_eq!(resolved.phash(&own).expect("hit"), first);

        // Different sender: correct value, memo not overwritten.
        let other_direct = {
            let set = crate::send::group::build_group_phash_set(&devices, &other);
            MessageUtils::participant_list_hash(&set).unwrap()
        };
        assert_eq!(
            resolved.phash(&other).expect("bypass").as_str(),
            other_direct
        );
        assert_eq!(
            resolved.phash.get().expect("still pinned").0,
            own,
            "memo stays pinned to the first sender"
        );
        assert_ne!(
            first.as_str(),
            other_direct,
            "senders must differ for this test"
        );
    }
}
