use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use sha2::{Sha256, Sha512};
use std::collections::HashMap;
use wacore_libsignal::crypto::CryptographicMac;
use waproto::whatsapp as wa;

use crate::{AppStateError, WAPATCH_INTEGRITY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashState {
    pub version: u64,
    #[serde(with = "BigArray")]
    pub hash: [u8; 128],
    pub index_value_map: HashMap<String, Vec<u8>>,
    /// The collection's aggregate ltHash no longer agrees with the one its peers
    /// compute, so a patch's `snapshotMac` can never match again and comparing it
    /// only freezes the collection on a base already proven unusable.
    ///
    /// Mirrors WA Web's `isCollectionInMacMismatchFatal`
    /// (`WAWebGetCollectionVersion`): set the first time a *patch* fails the
    /// snapshotMac comparison, after which the comparison is skipped entirely
    /// (`WAWebSyncdAntiTampering`, `if (E && k) return null`). `patchMac` — the
    /// only MAC that proves authorship, since the server cannot compute it —
    /// stays enforced on every patch either way.
    ///
    /// Unlike WA Web, which merges the flag forward forever, a snapshot resets it:
    /// a snapshot rebuilds the ltHash from scratch, so the new baseline deserves
    /// to be trusted again.
    #[serde(default)]
    pub mac_mismatch_fatal: bool,
    /// Whether a bootstrap reached its terminal page.
    ///
    /// Set only when the run that persisted this state was not still paging, so
    /// a partial bootstrap -- page five of sixty, then a deferral -- leaves it
    /// false even though the version has moved. That is what makes it safe to
    /// ask before building an outgoing patch, where a cursor behind the head
    /// earns a 409 that the send has to unwind.
    ///
    /// Presence of a record cannot answer that on its own: a collection that has
    /// never synced and one that synced and is legitimately empty are both
    /// version 0 with an all-zero ltHash, byte for byte. WA Web distinguishes
    /// them by whether a record exists at all, because it writes one on the
    /// "no updates" branch; we carry the fact explicitly so a row written by an
    /// older build -- which wrote version 0 for an interrupted bootstrap too --
    /// decodes to false and bootstraps once more rather than asking for patches
    /// its empty ltHash can never accept.
    #[serde(default)]
    pub bootstrapped: bool,
}

impl Default for HashState {
    fn default() -> Self {
        Self {
            version: 0,
            hash: [0; 128],
            index_value_map: HashMap::new(),
            mac_mismatch_fatal: false,
            bootstrapped: false,
        }
    }
}

/// What a fold actually did, as opposed to what it was handed.
///
/// Returned rather than recomputed by callers: the fold's eligibility rule has
/// already changed once, and a diagnostic that restates it is one that can
/// disagree with the hash it is describing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldReport {
    /// How many value MACs entered the ltHash.
    pub folded: usize,
    /// How many records reached the fold carrying no index MAC -- no index
    /// message, no blob, or a blob that is present and empty, which WA Web's
    /// `hex(index.blob)` key cannot tell apart.
    ///
    /// Not a subset of [`FoldReport::folded`], which is what makes it worth
    /// reporting: they share one key, so however many there are they add at
    /// most one value MAC between them. Any count above one is a snapshot
    /// where something upstream dropped an index and the fold silently kept
    /// only the last of them -- correct, and still worth seeing in a log,
    /// because nothing else in `folded` will ever hint at it.
    pub unkeyed: usize,
    /// How many records carried no value blob at all and so folded nothing.
    ///
    /// Not a curiosity: WA Web reads `.byteLength` off the absent buffer and
    /// the whole fold throws, so a snapshot holding one is a snapshot nobody
    /// can agree about. Counted here rather than re-derived by a caller, and
    /// answered by whoever decides what a malformed snapshot costs.
    pub valueless: usize,
}

impl HashState {
    /// Whether there is a real ltHash here to apply patches on top of.
    ///
    /// This is the question `return_snapshot` answers: a version past zero proves
    /// a baseline was established, since patches only apply over one. Only
    /// version zero is ambiguous -- never synced and synced-but-empty are
    /// byte-identical there -- and that is the case [`HashState::bootstrapped`]
    /// decides. Asking for the flag alone would make every account re-download a
    /// snapshot per collection on upgrade, for something the version already
    /// answers.
    ///
    /// It is deliberately *not* the question to ask before building an outgoing
    /// patch. A run that persisted page five of sixty and then stopped has a
    /// baseline and is still behind the head; [`HashState::bootstrapped`] is what
    /// says a bootstrap actually finished.
    pub fn has_baseline(&self) -> bool {
        self.bootstrapped || self.version > 0
    }
}

/// Result of updating the hash state with mutations.
#[derive(Debug, Clone, Default)]
pub struct HashUpdateResult {
    /// Whether a REMOVE mutation was missing its previous value.
    /// This happens when the server has an entry we don't have locally.
    /// WhatsApp Web tracks this as telemetry for MAC-failure diagnostics;
    /// it must not make MAC validation failures non-fatal.
    pub has_missing_remove: bool,
}

/// Count of *records with a value blob* at or below which
/// [`HashState::update_hash_from_records`] dedups with a linear scan instead of
/// a sort; above it the sort wins despite ordering every entry. Note this counts
/// records, not distinct indices: a snapshot of 200 records over 100 indices
/// takes the sorted branch. Unkeyed records are counted here too, since they
/// go through the same dedup under the empty key rather than around it.
///
/// Mirrors `MAC_DEDUP_SCAN_LIMIT` in `wacore`'s appstate sync, which measured
/// the same trade-off over the same kind of key (#856).
const MAC_DEDUP_SCAN_LIMIT: usize = 64;

/// Where a value blob's MAC starts, by WA Web's arithmetic rather than by ours.
///
/// `valueMacFromIndexAndValueCipherText` is
/// `new Uint8Array(v).slice(t - MAC_LENGTH)` with `t = v.byteLength`
/// (`WAWebSyncdCrypto`), and that subtraction goes negative on a blob shorter
/// than a MAC — where `Array.prototype.slice` counts a negative start *from the
/// end again*, so the length is subtracted twice. A 31-byte blob is therefore
/// not folded whole; `slice(-1)` folds its last byte. The obvious reading —
/// "a negative start means the whole buffer" — is right for exactly the lengths
/// at or below 16, which is why it survived being written down once: 16 and 0
/// agree with it and 31 does not.
///
/// Only reachable on a record the server should never send. Matching it anyway
/// is the whole point: the ltHash is compared against a MAC computed by
/// somebody running that arithmetic.
fn value_mac_start(len: usize) -> usize {
    if len >= 32 {
        len - 32
    } else {
        // JS: `slice(k)` with `k < 0` starts at `max(0, len + k)`, and here
        // `k` is `len - 32`.
        (2 * len).saturating_sub(32)
    }
}

impl HashState {
    pub fn update_hash<F>(
        &mut self,
        mutations: &[wa::SyncdMutation],
        mut get_prev_set_value_mac: F,
    ) -> (HashUpdateResult, anyhow::Result<()>)
    where
        F: FnMut(&[u8], usize) -> anyhow::Result<Option<Vec<u8>>>,
    {
        // WA Web index-mode (WAWebSyncdAntiTampering, gate `d`): when every mutation
        // carries an index, a SET whose index is also REMOVEd in the same patch must
        // NOT subtract its previous value; the REMOVE owns that subtraction. Without
        // the guard the previous value is subtracted twice and the SET's value is
        // orphaned in the MAC store, leaving the ltHash and the store in permanent
        // disagreement. The legacy non-index path keeps the old math, like WA Web.
        // fn item, not a closure: the borrowed return needs HRTB (issue #825 class).
        fn index_mac_of(mutation: &wa::SyncdMutation) -> Option<&[u8]> {
            mutation
                .record
                .as_option()
                .and_then(|r| r.index.as_option())
                .and_then(|idx| idx.blob.as_deref())
        }
        let index_mode = mutations.iter().all(|m| index_mac_of(m).is_some());
        // Membership set over REMOVE index_macs, which are HMAC outputs (uniformly
        // random). A linear-scan Vec beats a SipHash HashSet at the patch sizes seen
        // in practice — the same trade-off as detect_duplicate_index_in_patch and
        // collect_unique_index_macs (#856). Only `.contains()` is queried below, so an
        // unconditional push is membership-equivalent to the set (a malformed duplicate
        // REMOVE is rejected by the duplicate-index guard regardless).
        let mut removed_in_patch: Vec<&[u8]> = Vec::new();
        if index_mode {
            for mutation in mutations {
                if mutation
                    .operation
                    .is_some_and(|op| op == wa::syncd_mutation::SyncdOperation::REMOVE)
                    && let Some(index_mac) = index_mac_of(mutation)
                {
                    removed_in_patch.push(index_mac);
                }
            }
        }

        // Borrow the MAC tails instead of copying; mirrors `update_hash_from_records`.
        let mut added: Vec<&[u8]> = Vec::with_capacity(mutations.len());
        let mut removed: Vec<Vec<u8>> = Vec::with_capacity(mutations.len());
        let mut result = HashUpdateResult::default();

        for (i, mutation) in mutations.iter().enumerate() {
            // SyncdOperation is an open enum; an unknown op can't be mapped to
            // add or subtract, so bail before touching the hash.
            let op = match mutation.operation {
                None => wa::syncd_mutation::SyncdOperation::SET,
                Some(v) => {
                    let Some(op) = v.as_known() else {
                        return (
                            result,
                            Err(anyhow::anyhow!(AppStateError::UnsupportedSyncdOperation(
                                v.to_i32()
                            ))),
                        );
                    };
                    op
                }
            };
            let is_set = op == wa::syncd_mutation::SyncdOperation::SET;
            // The same slice `generate_patch_mac` feeds the MAC, and it has to
            // be: the two describe one patch, so a rule applied to only one of
            // them lets a patch authenticate against its own patchMac and then
            // carry an ltHash the server's snapshotMac disagrees with.
            if is_set
                && mutation.record.is_set()
                && let Some(blob) = &mutation.record.value.blob
            {
                added.push(value_mac_tail(blob));
            }
            if let Some(index_mac) = index_mac_of(mutation) {
                if is_set && removed_in_patch.contains(&index_mac) {
                    continue;
                }
                match get_prev_set_value_mac(index_mac, i) {
                    Ok(Some(prev)) => removed.push(prev),
                    Ok(None) => {
                        if op == wa::syncd_mutation::SyncdOperation::REMOVE {
                            result.has_missing_remove = true;
                            log::trace!(
                                target: "AppState",
                                "REMOVE mutation missing previous value (hasMissingRemove=true)"
                            );
                        }
                    }
                    Err(e) => return (result, Err(anyhow::anyhow!(e))),
                }
            }
        }

        // One call, not one per direction: `subtract_then_add_in_place` already
        // walks the subtract list then the add list, so splitting it in two ran
        // each list past an empty counterpart and set the 128-byte derivation
        // buffer up twice over.
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &removed, &added);
        (result, Ok(()))
    }

    /// Fold a snapshot's records into the ltHash.
    ///
    /// Takes the records directly rather than cloning into `SyncdMutation`:
    /// a snapshot is all SETs with no previous values to look up.
    ///
    /// A snapshot is keyed by index, not a list: WA Web builds
    /// `new Map(records.map(r => [hex(r.index.blob), valueMac(r.value.blob)]))`
    /// and folds `[...map.values()]` (`WAWebSyncdAntiTampering`), so when a
    /// snapshot carries the same index twice only the last record counts.
    /// Folding both instead leaves the ltHash one value MAC heavier than the one
    /// the server signed, and the snapshotMac can then never match — the
    /// collection is unsyncable for good, every recovery path answering
    /// `snapshot MAC mismatch`. The store already keys the same way
    /// (`put_mutation_macs`, `on_conflict((name, index_mac, device_id))`), so
    /// before this the ltHash and the store disagreed about the same snapshot.
    ///
    /// The key is `hex(index.blob)`, and hex of nothing is the empty string —
    /// so a record with no index message, one whose index message carries no
    /// blob, and one whose blob is present but empty are *one* key in that Map,
    /// and only the last of them in list order survives it. Executed against
    /// the bundle: `[unkeyed(aa), unkeyed(bb)]` folds to exactly `[unkeyed(bb)]`,
    /// and reversing the pair folds to exactly `[unkeyed(aa)]`. Keying the
    /// absent index on the empty slice is therefore not a convenience, it is
    /// the oracle's own key spelled in bytes instead of hex; folding each
    /// unkeyed record separately made the ltHash heavier by one value MAC per
    /// extra unkeyed record, with the same permanent consequence as a repeated
    /// index above.
    ///
    /// A value blob shorter than a MAC is folded rather than dropped, for the
    /// same reason: `valueMacFromIndexAndValueCipherText` is
    /// `new Uint8Array(v).slice(len - MAC_LENGTH)`, which neither throws nor
    /// yields 32 bytes on a short buffer — it answers *something*, WA Web folds
    /// it, and the server signed a snapshot that includes it. Dropping it here
    /// is the ltHash disagreeing with the MAC it will be checked against, which
    /// is a judgement about what a MAC ought to look like paid for with the
    /// whole collection. `value_mac_start` is that slice; an empty blob folds
    /// an empty operand, which the ltHash's HKDF derives from perfectly well.
    ///
    /// Answers what it folded rather than how many records it was given: a
    /// repeated index contributes once, and every unkeyed record contributes
    /// through the one key they share. Diagnostics read this rather than
    /// restating the rule, which is how a count and a fold drift apart.
    pub fn update_hash_from_records(&mut self, records: &[wa::SyncdRecord]) -> FoldReport {
        // Borrow the MAC tails; no Vec<u8> allocation per MAC.
        let mut added: Vec<&[u8]> = Vec::with_capacity(records.len());
        let mut indexed: Vec<(&[u8], &[u8])> = Vec::with_capacity(records.len());
        // Counted here rather than recomputed by a caller, for the reason the
        // fold reports its own total: an unkeyed record is what a snapshot
        // looks like when something upstream lost the index, and after this
        // fix it is invisible in `folded` — several of them contribute one
        // value MAC between them, exactly as they do at the far end.
        let mut unkeyed = 0usize;
        let mut valueless = 0usize;

        for record in records {
            // A record with no value blob at all is the one place this still
            // diverges, and deliberately: WA Web reads `.byteLength` off it and
            // the whole fold throws, so there is no ltHash to agree with. The
            // collection fails either way; skipping is the failure that leaves
            // a diagnosable state behind.
            let Some(blob) = record.value.blob.as_ref() else {
                valueless += 1;
                continue;
            };
            let value_mac = &blob[value_mac_start(blob.len())..];

            // An index MAC is a 32-byte HMAC, so the empty slice is a key no
            // real index can take -- which is what lets the absent index share
            // the dedup with the keyed records instead of bypassing it. Absent
            // index message, absent blob and empty blob all land here, because
            // `hex()` of each of the three is the same empty string.
            let index_mac = record
                .index
                .as_option()
                .and_then(|idx| idx.blob.as_deref())
                .unwrap_or_default();
            if index_mac.is_empty() {
                unkeyed += 1;
            }
            // The last record for this key is the one that counts, keyed or not.
            indexed.push((index_mac, value_mac));
        }

        if indexed.len() <= MAC_DEDUP_SCAN_LIMIT {
            // Index MACs are HMAC outputs (uniformly random), so at these sizes a
            // linear scan beats a SipHash set — the same trade-off as
            // collect_unique_index_macs (#856). Walking backwards makes the first
            // hit the last record, which is the one WA Web's Map keeps.
            let mut seen: Vec<&[u8]> = Vec::with_capacity(indexed.len());
            for (index_mac, value_mac) in indexed.iter().rev() {
                if !seen.contains(index_mac) {
                    seen.push(index_mac);
                    added.push(value_mac);
                }
            }
        } else {
            // Above the scan limit the sort wins despite ordering every entry.
            // `sort_by` is stable, so within one index the encounter order — and
            // therefore which record is last — survives.
            indexed.sort_by(|a, b| a.0.cmp(b.0));
            let mut rest = indexed.as_slice();
            while let Some((index_mac, _)) = rest.first() {
                let run = rest.partition_point(|(candidate, _)| candidate == index_mac);
                added.push(rest[run - 1].1);
                rest = &rest[run..];
            }
        }

        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut self.hash, &[] as &[&[u8]], &added);
        FoldReport {
            folded: added.len(),
            unkeyed,
            valueless,
        }
    }

    pub fn generate_snapshot_mac(&self, name: &str, key: &[u8]) -> Vec<u8> {
        let version_be = version_to_64_bit_network_order(self.version);
        let mut mac =
            CryptographicMac::new("HmacSha256", key).expect("HmacSha256 is a valid algorithm");
        mac.update(&self.hash);
        mac.update(&version_be);
        mac.update(name.as_bytes());
        mac.finalize()
    }
}

pub fn generate_patch_mac(patch: &wa::SyncdPatch, name: &str, key: &[u8], version: u64) -> Vec<u8> {
    let mut mac =
        CryptographicMac::new("HmacSha256", key).expect("HmacSha256 is a valid algorithm");

    // Feed directly to HMAC without collecting into Vec<Vec<u8>>
    if let Some(sm) = &patch.snapshot_mac {
        mac.update(sm);
    }
    for m in &patch.mutations {
        // `m.record.is_set()` is a field-presence check (buffa's `MessageField`),
        // not a filter on the SET operation: WA Web folds every mutation's value
        // blob in, REMOVE included, so narrowing this to SET would change the MAC
        // of every patch carrying a removal.
        if m.record.is_set()
            && let Some(blob) = &m.record.value.blob
        {
            mac.update(value_mac_tail(blob));
        }
    }
    mac.update(&version_to_64_bit_network_order(version));
    mac.update(name.as_bytes());

    mac.finalize()
}

/// The trailing bytes WA Web feeds into a MAC for one value blob.
///
/// The arithmetic is [`value_mac_start`]'s, and deliberately not a second copy
/// of it: the snapshot fold and the patch MAC ask the same question about the
/// same blob, and two spellings of one rule are how they come to disagree. The
/// patch path used to require `blob.len() >= 32` and drop the mutation when it
/// was not met, which made a patch carrying a short blob MAC as though that
/// mutation were absent — for a single-mutation patch, the empty patch's MAC.
pub(crate) fn value_mac_tail(blob: &[u8]) -> &[u8] {
    &blob[value_mac_start(blob.len())..]
}

/// Encodes an app-state collection version the way WA Web's
/// `to64BitNetworkOrder` does — which, despite the name, is *not* a 64-bit
/// big-endian integer:
///
/// ```js
/// function m(e) {
///     var t = new ArrayBuffer(8);
///     return new DataView(t).setUint32(4, e, !1), t
/// }
/// ```
///
/// `setUint32` writes four bytes at offset 4 of an eight-byte buffer, and it
/// takes its argument modulo 2^32 (WebIDL `unsigned long` conversion). So the
/// top four bytes are always zero and the version rides in the low four:
/// `version mod 2^32`, not the version. Same shape as the `octetLength` buffer
/// in `generate_content_mac` below — eight zero bytes with only the tail
/// written.
///
/// Verified by executing the bundle's own function: version `2^32` yields
/// `0000000000000000` and `2^48-1` yields `00000000ffffffff`, so `2^32`
/// produces byte-for-byte the same MAC as `0`.
///
/// Truncation is what the oracle does and so is what we do, but it is silently
/// lossy, so a version that could not survive the round trip is logged rather
/// than swallowed. It is not an error: refusing would desynchronize us from a
/// server whose own client accepts the value, and the MAC we produce is still
/// the correct one. Real collection versions are small (57, 88, 234, 253
/// observed), so the branch is never taken in practice — it exists so that if
/// it ever is, there is a line saying why the version in the log stopped
/// agreeing with the version in the MAC.
fn version_to_64_bit_network_order(version: u64) -> [u8; 8] {
    if version > u64::from(u32::MAX) {
        log::warn!(
            "app-state version {version} exceeds 2^32-1; WA Web's to64BitNetworkOrder \
             truncates it to {} for the MAC, making this version indistinguishable \
             from that one",
            version as u32
        );
    }
    let mut out = [0u8; 8];
    out[4..].copy_from_slice(&(version as u32).to_be_bytes());
    out
}

pub fn generate_content_mac(
    operation: wa::syncd_mutation::SyncdOperation,
    data: &[u8],
    key_id: &[u8],
    key: &[u8],
) -> [u8; 32] {
    let op_byte = [operation as u8 + 1];
    // WA Web (WAWebSyncdMutationKeyApi.Crypto) packs the associated-data length as
    // a single u8 at the low byte of an 8-byte zero buffer:
    //   octetLength = new Uint8Array(8); octetLength[7] = ad.length & 0xff
    // We mirror that exactly so the HMAC input is bytewise identical.
    let mut key_data_length = [0u8; 8];
    key_data_length[7] = ((key_id.len() + 1) & 0xff) as u8;
    // Typed HMACs rather than `CryptographicMac::new("...")`: these two run per
    // decoded record, and the string-dispatched enum costs a name compare chain
    // plus a Sha512-sized stack object on every call. Output is byte-identical.
    let mut mac = Hmac::<Sha512>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(&op_byte);
    mac.update(key_id);
    mac.update(data);
    mac.update(&key_data_length);
    let out = mac.finalize().into_bytes();
    let mut result = [0u8; 32];
    result.copy_from_slice(&out[..32]);
    result
}

fn index_mac_array(index_json_bytes: &[u8], key: &[u8; 32]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(index_json_bytes);
    mac.finalize().into_bytes().into()
}

pub fn generate_index_mac(index_json_bytes: &[u8], key: &[u8; 32]) -> Vec<u8> {
    index_mac_array(index_json_bytes, key).to_vec()
}

pub fn validate_index_mac(
    index_json_bytes: &[u8],
    expected_mac: &[u8],
    key: &[u8; 32],
) -> Result<(), AppStateError> {
    // Compare against the stack array: no heap allocation per validated record.
    if index_mac_array(index_json_bytes, key) != expected_mac {
        Err(AppStateError::MismatchingIndexMAC)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_mutation(
        operation: wa::syncd_mutation::SyncdOperation,
        index_mac: Vec<u8>,
        value_mac: Option<Vec<u8>>,
    ) -> wa::SyncdMutation {
        let value_blob = value_mac.map(|mac| {
            let mut blob = vec![0u8; 16];
            blob.extend_from_slice(&mac);
            blob
        });

        let value = if let Some(b) = value_blob {
            buffa::MessageField::some(wa::SyncdValue { blob: Some(b) })
        } else {
            buffa::MessageField::none()
        };

        wa::SyncdMutation {
            operation: Some(operation.into()),
            record: buffa::MessageField::some(wa::SyncdRecord {
                index: buffa::MessageField::some(wa::SyncdIndex {
                    blob: Some(index_mac),
                }),
                value,
                key_id: buffa::MessageField::some(wa::KeyId {
                    id: Some(b"test_key_id".to_vec()),
                }),
            }),
        }
    }

    #[test]
    fn test_update_hash_with_set_overwrite_and_remove() {
        const INDEX_MAC_1: &[u8] = &[1; 32];
        const VALUE_MAC_1: &[u8] = &[10; 32];

        const INDEX_MAC_2: &[u8] = &[2; 32];
        const VALUE_MAC_2: &[u8] = &[20; 32];

        const VALUE_MAC_3_OVERWRITE: &[u8] = &[30; 32];

        let mut prev_macs = HashMap::<Vec<u8>, Vec<u8>>::new();

        let mut state = HashState::default();
        let initial_mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC_1.to_vec(),
                Some(VALUE_MAC_1.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC_2.to_vec(),
                Some(VALUE_MAC_2.to_vec()),
            ),
        ];

        let get_prev_mac_closure = |_: &[u8], _: usize| Ok(None);
        let (hash_result, result) = state.update_hash(&initial_mutations, get_prev_mac_closure);
        assert!(result.is_ok());
        assert!(!hash_result.has_missing_remove);

        const EMPTY: &[Vec<u8>] = &[];
        let expected_hash_after_add = WAPATCH_INTEGRITY.subtract_then_add(
            &[0; 128],
            EMPTY,
            &[VALUE_MAC_1.to_vec(), VALUE_MAC_2.to_vec()],
        );
        assert_eq!(state.hash.as_slice(), expected_hash_after_add.as_slice());

        prev_macs.insert(INDEX_MAC_1.to_vec(), VALUE_MAC_1.to_vec());
        prev_macs.insert(INDEX_MAC_2.to_vec(), VALUE_MAC_2.to_vec());

        let update_and_remove_mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC_1.to_vec(),
                Some(VALUE_MAC_3_OVERWRITE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC_2.to_vec(),
                None,
            ),
        ];

        let get_prev_mac_closure_phase2 =
            |index_mac: &[u8], _: usize| Ok(prev_macs.get(index_mac).cloned());
        let (hash_result, result) =
            state.update_hash(&update_and_remove_mutations, get_prev_mac_closure_phase2);
        assert!(result.is_ok());
        assert!(!hash_result.has_missing_remove);

        let expected_final_hash = WAPATCH_INTEGRITY.subtract_then_add(
            &expected_hash_after_add,
            &[VALUE_MAC_1.to_vec(), VALUE_MAC_2.to_vec()],
            &[VALUE_MAC_3_OVERWRITE.to_vec()],
        );

        assert_eq!(
            state.hash.as_slice(),
            expected_final_hash.as_slice(),
            "The final hash state after overwrite and remove is incorrect."
        );
    }

    /// WA Web index-mode: a SET whose index is also REMOVEd in the same patch must
    /// not subtract its previous value; only the REMOVE subtracts (the store value).
    #[test]
    fn test_update_hash_set_plus_remove_same_index_subtracts_once() {
        const INDEX_MAC: &[u8] = &[1; 32];
        const PREV_VALUE: &[u8] = &[10; 32];
        const NEW_VALUE: &[u8] = &[20; 32];

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC.to_vec(),
                Some(PREV_VALUE.to_vec()),
            ),
        ];

        let mut state = HashState::default();
        let mut lookups = 0usize;
        let (hash_result, result) = state.update_hash(&mutations, |_, _| {
            lookups += 1;
            Ok(Some(PREV_VALUE.to_vec()))
        });
        assert!(result.is_ok());
        assert!(!hash_result.has_missing_remove);
        assert_eq!(lookups, 1, "the suppressed SET must not query the store");

        let expected = WAPATCH_INTEGRITY.subtract_then_add(
            &[0; 128],
            &[PREV_VALUE.to_vec()],
            &[NEW_VALUE.to_vec()],
        );
        assert_eq!(state.hash.as_slice(), expected.as_slice());
    }

    /// Index-mode is gated on every mutation carrying an index (WA Web's `d`):
    /// with one index-less mutation in the patch, the SET subtracts as before.
    #[test]
    fn test_update_hash_suppression_disabled_without_full_index_coverage() {
        const INDEX_MAC: &[u8] = &[1; 32];
        const PREV_VALUE: &[u8] = &[10; 32];
        const NEW_VALUE: &[u8] = &[20; 32];

        let mut index_less = create_mutation(
            wa::syncd_mutation::SyncdOperation::SET,
            vec![],
            Some(vec![30; 32]),
        );
        if let Some(rec) = index_less.record.as_option_mut() {
            rec.index = Default::default();
        }

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC.to_vec(),
                Some(PREV_VALUE.to_vec()),
            ),
            index_less,
        ];

        let mut state = HashState::default();
        let (_, result) = state.update_hash(&mutations, |_, _| Ok(Some(PREV_VALUE.to_vec())));
        assert!(result.is_ok());

        // Legacy math: SET and REMOVE each subtract the previous value.
        let expected = WAPATCH_INTEGRITY.subtract_then_add(
            &[0; 128],
            &[PREV_VALUE.to_vec(), PREV_VALUE.to_vec()],
            &[NEW_VALUE.to_vec(), vec![30; 32]],
        );
        assert_eq!(state.hash.as_slice(), expected.as_slice());
    }

    /// SET+REMOVE same index against an empty store: the SET still adds, the REMOVE
    /// finds nothing and flags has_missing_remove, matching WA Web index-mode which
    /// has no fallback query.
    #[test]
    fn test_update_hash_set_plus_remove_same_index_empty_store() {
        const INDEX_MAC: &[u8] = &[1; 32];
        const NEW_VALUE: &[u8] = &[20; 32];

        let mutations = vec![
            create_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
            create_mutation(
                wa::syncd_mutation::SyncdOperation::REMOVE,
                INDEX_MAC.to_vec(),
                Some(NEW_VALUE.to_vec()),
            ),
        ];

        let mut state = HashState::default();
        let (hash_result, result) = state.update_hash(&mutations, |_, _| Ok(None));
        assert!(result.is_ok());
        assert!(hash_result.has_missing_remove);

        const EMPTY: &[Vec<u8>] = &[];
        let expected = WAPATCH_INTEGRITY.subtract_then_add(&[0; 128], EMPTY, &[NEW_VALUE.to_vec()]);
        assert_eq!(state.hash.as_slice(), expected.as_slice());
    }

    /// Builds the patch shape `generate_patch_mac` is asked about from the
    /// oracle's own hex, so a test reads as the vector it is anchored on.
    /// `index` and `key_id` are set because the oracle's patches carry them; the
    /// MAC does not read either.
    fn patch_from_hex(
        snapshot_mac: &str,
        mutations: &[(wa::syncd_mutation::SyncdOperation, &str)],
    ) -> wa::SyncdPatch {
        wa::SyncdPatch {
            snapshot_mac: Some(hex::decode(snapshot_mac).expect("vector hex")),
            mutations: mutations
                .iter()
                .map(|(operation, blob)| wa::SyncdMutation {
                    operation: Some((*operation).into()),
                    record: buffa::MessageField::some(wa::SyncdRecord {
                        index: buffa::MessageField::some(wa::SyncdIndex {
                            blob: Some(vec![0u8; 32]),
                        }),
                        value: buffa::MessageField::some(wa::SyncdValue {
                            blob: Some(hex::decode(blob).expect("vector hex")),
                        }),
                        key_id: buffa::MessageField::some(wa::KeyId {
                            id: Some(b"test_key_id".to_vec()),
                        }),
                    }),
                })
                .collect(),
            ..Default::default()
        }
    }

    /// The exact table `WAWebSyncdCrypto.valueMacFromIndexAndValueCipherText`
    /// answers with, read off the running bundle. `slice(len - 32)` counts a
    /// negative start from the end, so this is `blob[max(2*len - 32, 0)..]` and
    /// not "the last 32 bytes, or nothing".
    #[test]
    fn value_mac_tail_matches_the_official_slice() {
        let blob: Vec<u8> = (1..=64u8).collect();
        for (len, expected_len) in [
            (0usize, 0usize),
            (1, 1),
            (8, 8),
            (15, 15),
            (16, 16),
            (17, 15),
            (20, 12),
            (31, 1),
            (32, 32),
            (33, 32),
            (64, 32),
        ] {
            let tail = value_mac_tail(&blob[..len]);
            assert_eq!(
                tail.len(),
                expected_len,
                "wrong length for a {len}-byte blob"
            );
            assert_eq!(
                tail,
                &blob[len - expected_len..len],
                "a {len}-byte blob must contribute its own last {expected_len} bytes"
            );
        }
    }

    /// Known-answer test for `generate_patch_mac`, anchored on the oracle rather
    /// than on a manual replay of this function's own steps: the bytes are
    /// vector `repo-known-answer-test-replica` of the 07-patch-mac conformance
    /// suite, produced by executing WA Web's own
    /// `WAWebSyncdEncryptionManager.generatePatchMac` out of the minified bundle.
    #[test]
    fn test_generate_patch_mac_known_answer() {
        use wa::syncd_mutation::SyncdOperation::SET;
        let key = [0xAAu8; 32];
        let patch = patch_from_hex(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &[
                (
                    SET,
                    "000000000000000000000000000000002222222222222222222222222222222222222222222222222222222222222222",
                ),
                (
                    SET,
                    "000000000000000000000000000000003333333333333333333333333333333333333333333333333333333333333333",
                ),
            ],
        );

        assert_eq!(
            hex::encode(generate_patch_mac(&patch, "regular_high", &key, 42)),
            "f0c524a5c5f1e2877788a6a14363816e829b5dc064bd96234bba5cb92727d3ec"
        );
    }

    /// The same shape with a 16-byte blob wedged into the middle. WA Web folds
    /// that mutation in whole (`slice(16 - 32)` is `slice(-16)`, the whole blob);
    /// a `blob.len() >= 32` guard dropped it, and the MAC came out as though the
    /// patch had only two mutations.
    ///
    /// Vector `repo-known-answer-shape-plus-short-blob-16`.
    #[test]
    fn a_short_blob_is_folded_in_rather_than_dropped() {
        use wa::syncd_mutation::SyncdOperation::SET;
        let key = [0xAAu8; 32];
        let patch = patch_from_hex(
            "1111111111111111111111111111111111111111111111111111111111111111",
            &[
                (
                    SET,
                    "000000000000000000000000000000002222222222222222222222222222222222222222222222222222222222222222",
                ),
                (SET, "44444444444444444444444444444444"),
                (
                    SET,
                    "000000000000000000000000000000003333333333333333333333333333333333333333333333333333333333333333",
                ),
            ],
        );

        assert_eq!(
            hex::encode(generate_patch_mac(&patch, "regular_high", &key, 42)),
            "4a9e2c1ab1b4dbac593e4e88222a18d54bc5520946f8917d439a21108771dd04"
        );
    }

    /// A patch whose *only* mutation carries a short blob. This is the shape the
    /// old guard failed hardest on: dropping the one mutation made the answer
    /// the empty patch's MAC
    /// (`73750b1d…`, vector `empty-mutations`), so a tampered patch and an empty
    /// one were indistinguishable.
    ///
    /// Vector `only-a-short-blob-16`.
    #[test]
    fn a_patch_of_one_short_blob_is_not_the_empty_patch() {
        use wa::syncd_mutation::SyncdOperation::SET;
        let key = hex::decode("c18e344692effe59af62f7e5f603feecf669620eada0ce0f40780dbaa044035c")
            .expect("vector hex");
        let snapshot_mac = "5a7fa4c9ee13385d82a7ccf1163b6085aacff4193e6388add2f71c41668bb0d5";

        let patch = patch_from_hex(snapshot_mac, &[(SET, "c8ed12375c81a6cbf0153a5f84a9cef3")]);
        let mac = hex::encode(generate_patch_mac(&patch, "regular_low", &key, 253));
        assert_eq!(
            mac,
            "ecfe951c2f66815234e19c5919258790351f414a638eed6a1a37288829df0a91"
        );

        let empty = patch_from_hex(snapshot_mac, &[]);
        assert_ne!(
            mac,
            hex::encode(generate_patch_mac(&empty, "regular_low", &key, 253)),
            "a mutation must not vanish from the MAC"
        );
    }

    /// A 31-byte blob contributes its last *one* byte, and a 0-byte blob nothing
    /// — the two ends of the negative-slice rule, both between full-length
    /// neighbours so a wrong length shifts the whole HMAC input.
    ///
    /// Vectors `short-blob-31-between-two-normal` and
    /// `short-blob-0-between-two-normal`.
    #[test]
    fn short_blobs_contribute_the_official_number_of_bytes() {
        use wa::syncd_mutation::SyncdOperation::SET;
        let key = hex::decode("c18e344692effe59af62f7e5f603feecf669620eada0ce0f40780dbaa044035c")
            .expect("vector hex");
        let snapshot_mac = "5a7fa4c9ee13385d82a7ccf1163b6085aacff4193e6388add2f71c41668bb0d5";
        let first = "03284d7297bce1062b50759abfe4092e01264b7095badf04294e7398bde2072c51769bc0e50a2f54799ec3e80d32577c";
        let last = "03284d7297bce1062b50759abfe4092e02274c7196bbe0052a4f7499bee3082d52779cc1e60b30557a9fc4e90e33587d";

        // 31 bytes: the oracle's value MAC for it is the single byte `1e`.
        let patch = patch_from_hex(
            snapshot_mac,
            &[
                (SET, first),
                (
                    SET,
                    "c8ed12375c81a6cbf0153a5f84a9cef3183d6287acd1f61b40658aafd4f91e",
                ),
                (SET, last),
            ],
        );
        assert_eq!(
            hex::encode(generate_patch_mac(&patch, "regular_low", &key, 253)),
            "0ca4713495777d8c3241c327a9def39c6e1d706a5796de78e6c43c0961e2d27e"
        );

        // 0 bytes: the one length that really does contribute nothing.
        let patch = patch_from_hex(snapshot_mac, &[(SET, first), (SET, ""), (SET, last)]);
        assert_eq!(
            hex::encode(generate_patch_mac(&patch, "regular_low", &key, 253)),
            "5e8e0a53184e51a82b015b83d53c801cbc31fafe6f5f6e5781e5c37f01cfccd1"
        );
    }

    /// `m.record.is_set()` in `generate_patch_mac` is field presence, not the SET
    /// operation: WA Web's loop reads `m.record.value.blob` off every mutation
    /// and never looks at `m.operation`. Mistaking it for an operation filter
    /// would change the MAC of every patch carrying a removal, which is most of
    /// them.
    ///
    /// Vectors `mixed-set-and-remove` and `all-remove`.
    #[test]
    fn removals_are_folded_into_the_patch_mac_too() {
        use wa::syncd_mutation::SyncdOperation::{REMOVE, SET};
        let key = hex::decode("c18e344692effe59af62f7e5f603feecf669620eada0ce0f40780dbaa044035c")
            .expect("vector hex");
        let snapshot_mac = "5a7fa4c9ee13385d82a7ccf1163b6085aacff4193e6388add2f71c41668bb0d5";
        let a = "03284d7297bce1062b50759abfe4092e01264b7095badf04294e7398bde2072c51769bc0e50a2f54799ec3e80d32577c";
        let b = "03284d7297bce1062b50759abfe4092e02274c7196bbe0052a4f7499bee3082d52779cc1e60b30557a9fc4e90e33587d";
        let c = "03284d7297bce1062b50759abfe4092e03284d7297bce1062b50759abfe4092e53789dc2e70c31567ba0c5ea0f34597e";
        let d = "03284d7297bce1062b50759abfe4092e04294e7398bde2072c51769bc0e50a2f54799ec3e80d32577ca1c6eb10355a7f";

        let mixed = patch_from_hex(
            snapshot_mac,
            &[(SET, a), (REMOVE, b), (REMOVE, c), (SET, d)],
        );
        assert_eq!(
            hex::encode(generate_patch_mac(&mixed, "regular_low", &key, 253)),
            "bc500ec7136ea2a921d6d4345a40378530ba0fb1d3a222365c912df5d893780c"
        );

        // Vector `all-remove`, which carries its own name, version and snapshot MAC.
        let all_remove = patch_from_hex(
            "082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83",
            &[
                (
                    REMOVE,
                    "03284d7297bce1062b50759abfe4092e052a4f7499bee3082d52779cc1e60b30557a9fc4e90e33587da2c7ec11365b80",
                ),
                (
                    REMOVE,
                    "03284d7297bce1062b50759abfe4092e062b50759abfe4092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81",
                ),
            ],
        );
        assert_eq!(
            hex::encode(generate_patch_mac(
                &all_remove,
                "critical_unblock_low",
                &key,
                3
            )),
            "334e8d4731f4b3cec4a6ff70971358f84bfb785f218699e6142048678827e881"
        );
    }

    /// A record with no index message at all, its value blob a 16-byte prefix
    /// and the MAC tail, like `record_with`.
    fn unkeyed_record(value_mac: &[u8]) -> wa::SyncdRecord {
        let mut blob = vec![0u8; 16];
        blob.extend_from_slice(value_mac);
        wa::SyncdRecord {
            value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob) }),
            ..Default::default()
        }
    }

    fn record_with(index_mac: &[u8], value_mac: &[u8]) -> wa::SyncdRecord {
        let mut blob = vec![0u8; 16];
        blob.extend_from_slice(value_mac);
        wa::SyncdRecord {
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: Some(index_mac.to_vec()),
            }),
            value: buffa::MessageField::some(wa::SyncdValue { blob: Some(blob) }),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(b"test_key_id".to_vec()),
            }),
        }
    }

    /// Production symptom: `regular_low` never syncs again, every recovery path
    /// answering `snapshot MAC mismatch`, because a snapshot carrying the same
    /// index twice folds both value MACs into the ltHash.
    ///
    /// WA Web keys the records by index before folding
    /// (`WAWebSyncdAntiTampering`: `new Map(records.map(r => [hex(r.index.blob),
    /// valueMac(r.value.blob)]))`, then `add(EMPTY_LT_HASH, [...d.values()])`),
    /// so the last record for an index is the only one that counts.
    #[test]
    fn a_repeated_index_in_a_snapshot_folds_only_its_last_value() {
        const INDEX_A: &[u8] = &[1u8; 32];
        const INDEX_B: &[u8] = &[2u8; 32];
        const STALE: &[u8] = &[0xAAu8; 32];
        const WINNER: &[u8] = &[0xBBu8; 32];
        const OTHER: &[u8] = &[0xCCu8; 32];

        let records = vec![
            record_with(INDEX_A, STALE),
            record_with(INDEX_A, WINNER),
            record_with(INDEX_B, OTHER),
        ];

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(
            &mut expected,
            &[] as &[&[u8]],
            &[WINNER, OTHER],
        );

        assert_eq!(
            state.hash, expected,
            "a repeated index must contribute only its last value MAC, as WA Web's Map does"
        );
    }

    /// Review of #1365: a bootstrap that persisted page five of sixty and then
    /// stopped has a baseline and is still behind the head. Reading a positive
    /// version as a finished bootstrap let the send guard skip its preflight and
    /// build the user's patch on that cursor.
    #[test]
    fn a_partial_bootstrap_has_a_baseline_but_has_not_bootstrapped() {
        let partial = HashState {
            version: 5,
            ..Default::default()
        };
        assert!(
            partial.has_baseline(),
            "page five established a baseline, so patches apply over it"
        );
        assert!(
            !partial.bootstrapped,
            "but the run never reached its terminal page"
        );
    }

    /// The upgrade case the baseline question exists for: a row written before
    /// the flag existed, sitting on real data.
    #[test]
    fn a_legacy_row_past_zero_still_counts_as_having_a_baseline() {
        let legacy = HashState {
            version: 7,
            ..Default::default()
        };
        assert!(
            legacy.has_baseline(),
            "asking such a row for a snapshot would re-download one per collection \
             on upgrade, for something the version already answers"
        );
    }

    /// Version zero is the ambiguous case, and the only one the flag decides.
    #[test]
    fn at_version_zero_only_the_flag_separates_never_synced_from_empty() {
        let never = HashState::default();
        let empty_but_synced = HashState {
            bootstrapped: true,
            ..Default::default()
        };
        assert_eq!(never.version, empty_but_synced.version);
        assert_eq!(never.hash, empty_but_synced.hash);
        assert!(!never.has_baseline());
        assert!(empty_but_synced.has_baseline());
    }

    /// The branch production actually takes. `MAC_DEDUP_SCAN_LIMIT` counts
    /// records carrying an index, not distinct indices, so any real snapshot --
    /// `regular_low` on an active account is hundreds of records -- sorts rather
    /// than scans. The linear branch the other tests exercise is the one a real
    /// account almost never reaches.
    #[test]
    fn a_repeated_index_is_deduped_above_the_scan_limit_too() {
        const DUPLICATED: &[u8] = &[0xEEu8; 32];
        const STALE: &[u8] = &[0x11u8; 32];
        const WINNER: &[u8] = &[0x22u8; 32];

        // Comfortably past the limit, so this takes the sorted branch.
        let filler = MAC_DEDUP_SCAN_LIMIT * 2;
        let mut records = Vec::with_capacity(filler + 2);
        let mut expected_macs: Vec<[u8; 32]> = Vec::with_capacity(filler + 1);

        // The stale copy first, and the winner last, with the whole filler set
        // between them: the sort has to keep encounter order within the index.
        records.push(record_with(DUPLICATED, STALE));
        for i in 0..filler {
            let mut index_mac = [0u8; 32];
            index_mac[..8].copy_from_slice(&(i as u64).to_le_bytes());
            let mac = [(i % 251) as u8; 32];
            records.push(record_with(&index_mac, &mac));
            expected_macs.push(mac);
        }
        records.push(record_with(DUPLICATED, WINNER));
        expected_macs.push(*<&[u8; 32]>::try_from(WINNER).unwrap());

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let added: Vec<&[u8]> = expected_macs.iter().map(|m| m.as_slice()).collect();
        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut expected, &[] as &[&[u8]], &added);

        assert_eq!(
            state.hash, expected,
            "above the scan limit the sort must still keep the last record for a \
             repeated index, and only that one"
        );
    }

    /// Three records under one index, the winner in last place. Guards
    /// `rest[run - 1]` against picking the middle of a run.
    #[test]
    fn the_last_of_a_run_of_three_is_the_one_that_folds() {
        const INDEX: &[u8] = &[7u8; 32];
        const FIRST: &[u8] = &[0xA1u8; 32];
        const MIDDLE: &[u8] = &[0xA2u8; 32];
        const LAST: &[u8] = &[0xA3u8; 32];

        let records = vec![
            record_with(INDEX, FIRST),
            record_with(INDEX, MIDDLE),
            record_with(INDEX, LAST),
        ];

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut expected, &[] as &[&[u8]], &[LAST]);

        assert_eq!(
            state.hash, expected,
            "a run of three must fold its last record, not its first or middle"
        );
    }

    /// The ordinary snapshot: every index distinct. Guards the dedup from
    /// changing what it must not change.
    #[test]
    fn distinct_indices_in_a_snapshot_all_fold() {
        const MAC_1: &[u8] = &[0x11u8; 32];
        const MAC_2: &[u8] = &[0x22u8; 32];

        let records = vec![
            record_with(&[1u8; 32], MAC_1),
            record_with(&[2u8; 32], MAC_2),
        ];

        let mut state = HashState::default();
        state.update_hash_from_records(&records);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(
            &mut expected,
            &[] as &[&[u8]],
            &[MAC_1, MAC_2],
        );

        assert_eq!(state.hash, expected);
    }

    /// One record with no index folds, like any other. It is *two* that used to
    /// be wrong: see `unkeyed_records_collapse_onto_the_last_of_them` below,
    /// which is anchored on the bundle's own bytes.
    #[test]
    fn a_record_without_an_index_still_folds() {
        const MAC: &[u8] = &[0x44u8; 32];
        let record = unkeyed_record(MAC);

        let mut state = HashState::default();
        let report = state.update_hash_from_records(&[record]);

        let mut expected = [0u8; 128];
        WAPATCH_INTEGRITY.subtract_then_add_in_place(&mut expected, &[] as &[&[u8]], &[MAC]);

        assert_eq!(state.hash, expected);
        assert_eq!(report.folded, 1);
        assert_eq!(report.unkeyed, 1);
    }

    /// The fold says how many records reached it without an index, because
    /// `folded` deliberately cannot: they share the empty key, so three of them
    /// add one value MAC between them and `folded` reads exactly as it would
    /// for one. A snapshot where something upstream dropped an index is
    /// therefore silent in the hash and in the count, and this number is the
    /// only place it shows.
    ///
    /// It used to pin the opposite -- that each unkeyed record folded on its
    /// own, "a known divergence from WA Web" left in place until somebody
    /// proved such records exist. The proof arrived by executing the bundle,
    /// and the vectors below are what it said.
    #[test]
    fn the_fold_reports_records_that_carried_no_index() {
        let mut state = HashState::default();
        let report = state.update_hash_from_records(&[
            record_with(&[0x01; 32], &[0x11; 32]),
            unkeyed_record(&[0x22; 32]),
            unkeyed_record(&[0x33; 32]),
        ]);

        assert_eq!(
            report.folded, 2,
            "the keyed record, and one entry for both unkeyed ones"
        );
        assert_eq!(report.unkeyed, 2, "two of them carried no index MAC");
    }

    // ---- Anchored on WhatsApp Web's own fold ----
    //
    // Every `ORACLE` below is the ltHash read back out of
    // `WAWebSyncdAntiTampering.computeLtHashAndValidateSnapshot`, evaluated
    // against the bundle over the record list beside it (unit 3 of the appstate
    // conformance harness). They are the bundle's bytes, not ours: an
    // expectation computed by this file would only prove it agrees with itself,
    // which is exactly how the two bugs below survived a review.

    fn hex_to_bytes(hexed: &str) -> Vec<u8> {
        (0..hexed.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hexed[i..i + 2], 16).expect("hex"))
            .collect()
    }

    fn to_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// A record spelled the way a vector spells one: `None` is an index message
    /// carrying no blob, `Some("")` is a blob that is present and empty, and
    /// the value is the whole blob rather than its MAC tail.
    fn vector_record(index: Option<&str>, value: &str) -> wa::SyncdRecord {
        wa::SyncdRecord {
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: index.map(hex_to_bytes),
            }),
            value: buffa::MessageField::some(wa::SyncdValue {
                blob: Some(hex_to_bytes(value)),
            }),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(b"unit-3".to_vec()),
            }),
        }
    }

    fn fold_vector(records: &[(Option<&str>, &str)]) -> String {
        let records: Vec<wa::SyncdRecord> = records
            .iter()
            .map(|(index, value)| vector_record(*index, value))
            .collect();
        let mut state = HashState::default();
        state.update_hash_from_records(&records);
        to_hex(&state.hash)
    }

    const VALUE_AA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const VALUE_BB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaabbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const VALUE_CC: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaacccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const VALUE_DD: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaadddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    const VALUE_11: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa1111111111111111111111111111111111111111111111111111111111111111";
    const VALUE_22: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa2222222222222222222222222222222222222222222222222222222222222222";
    const VALUE_SHORT_16: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const INDEX_01: &str = "0101010101010101010101010101010101010101010101010101010101010101";

    /// The bug this fold was fixed for. Two records with no index, and only the
    /// *last* counts -- `hex(undefined)` is the empty string, so both take the
    /// same slot in `new Map(...)` and the second overwrites the first. The
    /// control is that the pair's ltHash is byte-identical to folding `bb`
    /// alone, which is what "only the last one counts" looks like in bytes.
    #[test]
    fn unkeyed_records_collapse_onto_the_last_of_them() {
        const ORACLE: &str = "322369af239d58af2d96e079f16b2e2f34e99857b2fcb79e03f14989dff0c8a1c90f9318729190703d0b13278d8535244a0392039477f452d8aadd722eb9a494e4c5df3b181804fb4a1416c2a50c0d9557520341764dcc422bed529c8dd1f01512d1943e8ea87c70bcfb423eed082a170d7c73b75f26128799bf4d95add79a2c";

        assert_eq!(fold_vector(&[(None, VALUE_AA), (None, VALUE_BB)]), ORACLE);
        assert_eq!(
            fold_vector(&[(None, VALUE_BB)]),
            ORACLE,
            "the pair must fold to exactly the last of them"
        );
    }

    /// Three of them, in case a fix had merely deduped adjacent pairs.
    #[test]
    fn three_unkeyed_records_still_collapse_onto_the_last() {
        const ORACLE: &str = "6d40554f4612a572442973232ae0b7fbaf4fa2dd142ee8c911daebb4b2b58d2379e35e00fc6ddc65de04e585b1f4319f9408c213ed5789a24d909e645f895abda713d137bac36ab1098be79f1f43890ff1dfc71f92e4501b17117781f70daaea0f7bb1901d2395f572a206466c32db0c5f9cc4f247d8609bdb183b50fa61e850";

        assert_eq!(
            fold_vector(&[(None, VALUE_AA), (None, VALUE_BB), (None, VALUE_CC)]),
            ORACLE
        );
    }

    /// The empty key is a key like any other, so a real index standing between
    /// two unkeyed records neither joins them nor separates them: two entries
    /// out of three records, and the unkeyed winner is still the later one.
    #[test]
    fn unkeyed_records_collapse_beside_a_keyed_one() {
        const ORACLE: &str = "23fcaabc814078febe708da2e0da0e5157848f57b7cadaeaafc8e396aa78bee2378e0829de8b9c4ce15ec1e2f926dfc492e5c0bc8bad1236e06365415485140ffe050199e3bd2414947983ac8cb261d8c8c9679bf628a8035c1689ef0451fb2075f3399e0595dafb0638a55b2643861d0577a664681144e9291e65c2e2103e8e";

        assert_eq!(
            fold_vector(&[
                (None, VALUE_AA),
                (Some(INDEX_01), VALUE_DD),
                (None, VALUE_BB),
            ]),
            ORACLE
        );
    }

    /// An index blob that is present and empty hexes to the same empty string
    /// as an absent one, so the two collide. Which is why the fold keys on the
    /// blob's bytes and lets `unwrap_or_default` put the absent case on that
    /// same empty slice, rather than on a branch of its own.
    #[test]
    fn an_empty_index_blob_collides_with_an_absent_one() {
        const ORACLE: &str = "68f4dc5922d09f0977128992998640f811874354483239f723e73f2bc2d77cdae8b01fa438247f7502daaa05eb688ba7bb488eb24496a57784c85195af193a1cd809459c847f1d0e1a4b3caf9985dcd4029a75f17491c2ceecaa912ce73e79465c788d8453b6224435deee6f12398d7e193a7e86dd39135246cc2a5ca8fee0c7";

        assert_eq!(
            fold_vector(&[(Some(""), VALUE_11), (None, VALUE_22)]),
            ORACLE
        );
        assert_eq!(
            fold_vector(&[(None, VALUE_22)]),
            ORACLE,
            "the second record must have overwritten the first, not joined it"
        );
    }

    /// A 16-byte value blob is folded rather than dropped. `slice(16 - 32)` is
    /// `slice(-16)`, which on a 16-byte buffer is the whole of it -- so this is
    /// the length at which "short blobs fold whole" happens to be true, and the
    /// one below is where it stops being.
    #[test]
    fn a_value_blob_shorter_than_a_mac_folds_whole() {
        const ORACLE: &str = "f20d9b6ec1df9853c9ebf7210db4067cd489e8072f730a1eb61627e7ee0d506ee55c7aa9b631337e583f7d6aa17c535c65d7a0d2bbd73bdf9e0b0c3411757177ef470bb0f0530263b160f535bacdd7ca074631a10f139a6f3fbc03197351091bac2268212e94c000b0078d53363f2a496293646d422b329aa7e861d52b49ac84";

        assert_eq!(fold_vector(&[(Some(INDEX_01), VALUE_SHORT_16)]), ORACLE);
    }

    /// 31 bytes, and the answer is one byte. `slice(t - MAC_LENGTH)` subtracts
    /// the length once by arithmetic and once again by JS's negative-start
    /// rule, so the start is `max(0, 2*31 - 32) = 30`. The control is that this
    /// folds to exactly what a one-byte blob folds to -- which is what makes it
    /// a statement about the arithmetic and not about our own `saturating_sub`,
    /// whose answer for 31 bytes is the whole 31 and is a different hash.
    #[test]
    fn a_thirty_one_byte_value_blob_folds_its_last_byte() {
        const ORACLE: &str = "db072222f470dc859876c6234d729f626f0a5f75592f90a562407419b617a7f812e5c5e003271fff4456b1071603d7848a7d8c64984f6652032eac523bd04d4a8e626524fbafad694ba285faba6afe60fcc62871c6933f8e74859e29deb0dccf7f5fa160d5449581616214f061315b5e486e5d136979ccbe9e2d7a55a59eea6e";

        assert_eq!(fold_vector(&[(Some(INDEX_01), &"cc".repeat(31))]), ORACLE);
        assert_eq!(
            fold_vector(&[(Some(INDEX_01), "cc")]),
            ORACLE,
            "31 bytes must fold as its last byte alone"
        );
    }

    /// The whole of the length table the two rules above are two points on.
    #[test]
    fn the_value_mac_start_is_wa_webs_double_subtraction() {
        assert_eq!(value_mac_start(0), 0);
        assert_eq!(value_mac_start(15), 0, "2*15 - 32 is negative, so from 0");
        assert_eq!(value_mac_start(16), 0, "the whole buffer, exactly once");
        assert_eq!(value_mac_start(17), 2);
        assert_eq!(value_mac_start(31), 30);
        assert_eq!(
            value_mac_start(32),
            0,
            "at a MAC's length the tail is all of it"
        );
        assert_eq!(value_mac_start(48), 16, "and above it, the ordinary tail");
    }

    /// And an empty one folds an empty operand, which the ltHash's HKDF derives
    /// from perfectly well -- the answer is not the zero hash.
    #[test]
    fn an_empty_value_blob_folds_an_empty_operand() {
        const ORACLE: &str = "26671b06d4639067479b3e756bae60656d52dce6b2c73700951d88ee11d7fc11a22bd04b32a2de041368056c60c49af58990ff2f1a55e25d4ea05a3c4b867ea633d8b3af026d84324ca1073f6f53f5b48dc4840457119d872d4d4c23d87de2c8ddb51a1e2738ca3748e80ae8c6d71b076c9c66ad0a85308891bda3c7d86f2e0e";

        assert_eq!(fold_vector(&[(Some(INDEX_01), "")]), ORACLE);
    }

    /// Where the two rules meet: a short value blob is still the last record
    /// for its index, so it wins the dedup. Skipping it made the *previous*
    /// record the winner, which is a different value MAC and so a different
    /// ltHash -- a wrong answer where dropping a lone short record was merely
    /// an incomplete one.
    #[test]
    fn a_short_value_still_wins_its_index() {
        const ORACLE: &str = "f20d9b6ec1df9853c9ebf7210db4067cd489e8072f730a1eb61627e7ee0d506ee55c7aa9b631337e583f7d6aa17c535c65d7a0d2bbd73bdf9e0b0c3411757177ef470bb0f0530263b160f535bacdd7ca074631a10f139a6f3fbc03197351091bac2268212e94c000b0078d53363f2a496293646d422b329aa7e861d52b49ac84";

        assert_eq!(
            fold_vector(&[(Some(INDEX_01), VALUE_11), (Some(INDEX_01), VALUE_SHORT_16),]),
            ORACLE
        );
    }

    /// Every byte string below came out of executing WA Web's own
    /// `to64BitNetworkOrder` over the bundle corpus
    /// (`06-snapshot-mac/vectors.json`, `to64BitNetworkOrder_observed`). It is
    /// the four-low-bytes shape, not a u64: everything at or above 2^32 wraps.
    #[test]
    fn version_encoding_matches_the_bundles_to_64_bit_network_order() {
        for (version, expected) in [
            (0u64, "0000000000000000"),
            (1, "0000000000000001"),
            (253, "00000000000000fd"),
            // The last version a u32 can hold, and the last one that survives
            // the round trip.
            (u32::MAX as u64, "00000000ffffffff"),
            // 2^32 wraps to zero — the whole of this bug.
            (1u64 << 32, "0000000000000000"),
            ((1u64 << 32) + 1, "0000000000000001"),
            (1u64 << 40, "0000000000000000"),
            ((1u64 << 48) - 1, "00000000ffffffff"),
        ] {
            assert_eq!(
                hex::encode(version_to_64_bit_network_order(version)),
                expected,
                "version {version}"
            );
        }
    }

    /// The top four bytes are the `ArrayBuffer`'s untouched zeros: `setUint32`
    /// only ever writes at offset 4.
    #[test]
    fn the_high_four_bytes_of_a_version_are_always_zero() {
        for version in [0u64, 1, 253, u32::MAX as u64, 1u64 << 32, u64::MAX] {
            assert_eq!(
                &version_to_64_bit_network_order(version)[..4],
                &[0u8; 4],
                "version {version}"
            );
        }
    }

    /// The ltHash and key both oracle vectors are generated over
    /// (`06-snapshot-mac/vectors.json`, collection `regular_low`).
    const ORACLE_HASH: &str = "0b30557a9fc4e90e33587da2c7ec11365b80a5caef14395e83a8cdf2173c6186abd0f51a3f6489aed3f81d42678cb1d6fb20456a8fb4d9fe23486d92b7dc01264b7095badf04294e7398bde2072c51769bc0e50a2f54799ec3e80d32577ca1c6eb10355a7fa4c9ee13385d82a7ccf1163b6085aacff4193e6388add2f71c4166";
    const ORACLE_KEY: &str = "28582f9112b4da4049e786b9ab6edfb38eda814736ff638e025e0ca7b608b712";

    /// Snapshot MACs produced by WhatsApp Web's own
    /// `WAWebSyncdEncryptionManager.generateSnapshotMac`, copied from
    /// `06-snapshot-mac/vectors.json`. The four cases at or above 2^32 are the
    /// ones this file used to get wrong; the small ones are the controls that
    /// prove the fix did not move anything a real account can reach.
    #[test]
    fn snapshot_mac_matches_whatsapp_webs_own_output() {
        let mut hash = [0u8; 128];
        hash.copy_from_slice(&hex_to_bytes(ORACLE_HASH));
        let key = hex_to_bytes(ORACLE_KEY);

        for (version, expected) in [
            (
                0u64,
                "afd291ad5599afa5e3c0d5b2daf02a4bb101b370a7ab130139578e979c419a2e",
            ),
            (
                1,
                "e4e75567d69a6a38b0adff30f176d8285b269de0f77b67a3d4e0202770ed5fe8",
            ),
            (
                253,
                "9fbec1d8d747d515ba57ac0dae3138185dfdc4aff22e6279336772b62963979e",
            ),
            (
                65536,
                "5673fd3288ecfd7760226795adfc7495b55caf98f67dc8755463f975ef2f0d64",
            ),
            // 2^32-1: the boundary that still round-trips.
            (
                4294967295,
                "b10707136944665bfb0a85f68abd2f13d9a8e240a368cfdc367efd4a60405494",
            ),
            // 2^32 and 2^32+1 answer with version 0's and version 1's MACs.
            (
                4294967296,
                "afd291ad5599afa5e3c0d5b2daf02a4bb101b370a7ab130139578e979c419a2e",
            ),
            (
                4294967297,
                "e4e75567d69a6a38b0adff30f176d8285b269de0f77b67a3d4e0202770ed5fe8",
            ),
            (
                1099511627776,
                "afd291ad5599afa5e3c0d5b2daf02a4bb101b370a7ab130139578e979c419a2e",
            ),
            (
                281474976710655,
                "b10707136944665bfb0a85f68abd2f13d9a8e240a368cfdc367efd4a60405494",
            ),
        ] {
            let state = HashState {
                version,
                hash,
                ..Default::default()
            };
            assert_eq!(
                hex::encode(state.generate_snapshot_mac("regular_low", &key)),
                expected,
                "snapshot MAC for version {version}"
            );
        }
    }

    /// The wrap is not a property of the encoder alone: the oracle answers
    /// version 2^32 with version 0's MAC byte for byte, so a collection that
    /// ever reached 2^32 would be indistinguishable from a fresh one. Stated as
    /// its own test because it is the fact that makes the truncation a
    /// conformance decision rather than an implementation detail.
    #[test]
    fn a_version_and_that_version_plus_2_32_share_a_snapshot_mac() {
        let mut hash = [0u8; 128];
        hash.copy_from_slice(&hex_to_bytes(ORACLE_HASH));
        let key = hex_to_bytes(ORACLE_KEY);

        for version in [0u64, 1, 253] {
            let low = HashState {
                version,
                hash,
                ..Default::default()
            };
            let wrapped = HashState {
                version: version + (1u64 << 32),
                hash,
                ..Default::default()
            };
            assert_eq!(
                low.generate_snapshot_mac("regular_low", &key),
                wrapped.generate_snapshot_mac("regular_low", &key),
                "version {version} and {} must collide, as they do in WA Web",
                version + (1u64 << 32)
            );
        }
    }

    /// Patch MACs from WhatsApp Web's own
    /// `WAWebSyncdEncryptionManager.generatePatchMac`, copied from
    /// `07-patch-mac/vectors.json`. `version-above-2^32` (2^32+5) and
    /// `version-five-control` (5) carry the same expected answer in the oracle's
    /// own output, which is the second function reading the same truncation.
    #[test]
    fn patch_mac_matches_whatsapp_webs_own_output() {
        const PATCH_MAC_KEY: &str =
            "c18e344692effe59af62f7e5f603feecf669620eada0ce0f40780dbaa044035c";
        const SNAPSHOT_MAC: &str =
            "04294e7398bde2072c51769bc0e50a2f54799ec3e80d32577ca1c6eb10355a7f";
        const BLOB: &str = "03284d7297bce1062b50759abfe4092e092e53789dc2e70c31567ba0c5ea0f34597ea3c8ed12375c81a6cbf0153a5f84";

        let key = hex_to_bytes(PATCH_MAC_KEY);
        let patch = wa::SyncdPatch {
            snapshot_mac: Some(hex_to_bytes(SNAPSHOT_MAC)),
            mutations: vec![wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::SET.into()),
                record: buffa::MessageField::some(wa::SyncdRecord {
                    index: buffa::MessageField::some(wa::SyncdIndex {
                        blob: Some(vec![0u8; 32]),
                    }),
                    value: buffa::MessageField::some(wa::SyncdValue {
                        blob: Some(hex_to_bytes(BLOB)),
                    }),
                    key_id: buffa::MessageField::some(wa::KeyId {
                        id: Some(b"conformance".to_vec()),
                    }),
                }),
            }],
            ..Default::default()
        };

        for (version, expected) in [
            (
                0u64,
                "5197d9bd696573fd0de52f2761a907ea6914f5030d319ee5dd118901f556e801",
            ),
            (
                5,
                "cc830a016ecab383f84b03e117007deaa014e7ad5a648b87a44d477f584ce261",
            ),
            // 2^32-1 round-trips; 2^32+5 comes back as 5's answer above.
            (
                4294967295,
                "3cdbde9510e60994236746d734c05a2805500b2b1f74cff86fa5e273705610b9",
            ),
            (
                4294967301,
                "cc830a016ecab383f84b03e117007deaa014e7ad5a648b87a44d477f584ce261",
            ),
        ] {
            assert_eq!(
                hex::encode(generate_patch_mac(&patch, "regular", &key, version)),
                expected,
                "patch MAC for version {version}"
            );
        }
    }
}
