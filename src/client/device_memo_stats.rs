//! Per-term hit/miss accounting for the two device-list memos the group send
//! path depends on.
//!
//! Benchmarks answer *how much* a memo hit and a memo miss each cost
//! (`skdm_target_resolution_warm` vs `skdm_target_resolution_memo_cold`, PR
//! #1283). They cannot answer *which of the two a running client actually
//! gets*, because both of those fixtures force their outcome. That question
//! decides whether the miss-path cost is a real bill or a benchmark artifact,
//! and an aggregate "misses: N" would not answer it either: the group memo has
//! three validity terms and the SKDM memo four, so a miss count says something
//! is wrong without saying what. Every counter here therefore names the term
//! that decided the call.
//!
//! Exactly one counter is bumped per call to each resolver, so
//! [`GroupDevicesMemoStats::calls`] and [`SkdmTargetsMemoStats::calls`] are
//! sums, not separate counters that could drift from their parts.
//!
//! Always on, no feature gate, per `agent_docs/observability.md`: the increment
//! is one indexed relaxed `fetch_add` per resolver call — twice per group send,
//! against a send that signs with Ed25519 and writes a frame — and the
//! reporting types are dropped by LTO in a binary that never calls
//! [`Client::device_memo_stats`]. Measured cost is in the PR that added this.

use portable_atomic::AtomicU64;
use std::sync::atomic::Ordering;

use super::Client;

/// Which term decided one `resolve_group_devices_memoized` call.
///
/// `repr(usize)` and used as an index into [`DeviceMemoCounters`]'s array, so
/// recording one is a single indexed `fetch_add` with no branch on the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum GroupDevicesMemoOutcome {
    /// Entry present, `GroupInfo` identity matched, generation unchanged.
    Hit,
    /// Generation had moved, but every change since provably missed this
    /// group's member set, so the entry was re-stamped instead of recomputed.
    /// Served the same devices a hit would have, at the cost of the
    /// `unchanged_for` scan.
    Restamp,
    /// No entry for this group: first send, or a capacity/TTL eviction.
    MissAbsent,
    /// An entry existed but was built from a different `Arc<GroupInfo>`.
    /// Either the group metadata was genuinely refreshed, or a caller handed
    /// the resolver an `Arc` that is not the cached one.
    MissGroupInfo,
    /// Generation moved and the change log could not prove the change missed
    /// this group (a member was touched, or the log overflowed).
    MissTopology,
    /// The memo was not consulted: store-backed registry/mapping caches make
    /// its freshness contract unenforceable, so it is disabled wholesale.
    Bypassed,
}

/// Which of `skdm_memo_entry_stale_term`'s terms decided one
/// `resolve_skdm_targets_memoized` call. Indexed like its group counterpart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(crate) enum SkdmTargetsMemoOutcome {
    Hit,
    /// No entry at all: first send for this group, or an eviction.
    MissAbsent,
    /// The resolved device-set `Arc` differs from the memoized one. This is
    /// the cascade term: the devices come straight out of the group memo, so
    /// a group-memo recompute forces this miss regardless of the other three.
    MissDevices,
    /// The sender-key device map was rebuilt (a warm-mark write invalidated it).
    MissMap,
    /// Same map `Arc`, advanced generation: an in-place cold flip, i.e. a
    /// retry receipt's `markForgetSenderKey`.
    MissMapGeneration,
    /// A different sending identity (PN↔LID re-addressing, or a re-pair).
    MissSender,
    Bypassed,
    /// The prerequisite device resolution failed, so no memo term was ever
    /// evaluated. Recorded so that one call to `resolve_skdm_targets_memoized`
    /// is always one counter: without it a client whose sends were failing
    /// before the memo lookup would report a healthy hit rate over a
    /// shrinking denominator.
    ResolveFailed,
}

const GROUP_OUTCOMES: usize = 6;
const SKDM_OUTCOMES: usize = 8;

/// Array slot for one group-memo outcome.
///
/// Exhaustive on purpose. `record_group_devices` indexes by `as usize`, which
/// is fast and unchecked-looking; what makes it safe is that adding a variant
/// *anywhere* — including after the current last one — makes this match
/// non-exhaustive and the crate stops compiling. A bounds assertion on the
/// last variant alone would keep passing and let the new variant index off the
/// end of the array on the send path, which has no other symptom.
const fn group_slot(outcome: GroupDevicesMemoOutcome) -> usize {
    let slot = match outcome {
        GroupDevicesMemoOutcome::Hit => 0,
        GroupDevicesMemoOutcome::Restamp => 1,
        GroupDevicesMemoOutcome::MissAbsent => 2,
        GroupDevicesMemoOutcome::MissGroupInfo => 3,
        GroupDevicesMemoOutcome::MissTopology => 4,
        GroupDevicesMemoOutcome::Bypassed => 5,
    };
    assert!(slot < GROUP_OUTCOMES);
    slot
}

/// Array slot for one SKDM-memo outcome. Same contract as [`group_slot`].
const fn skdm_slot(outcome: SkdmTargetsMemoOutcome) -> usize {
    let slot = match outcome {
        SkdmTargetsMemoOutcome::Hit => 0,
        SkdmTargetsMemoOutcome::MissAbsent => 1,
        SkdmTargetsMemoOutcome::MissDevices => 2,
        SkdmTargetsMemoOutcome::MissMap => 3,
        SkdmTargetsMemoOutcome::MissMapGeneration => 4,
        SkdmTargetsMemoOutcome::MissSender => 5,
        SkdmTargetsMemoOutcome::Bypassed => 6,
        SkdmTargetsMemoOutcome::ResolveFailed => 7,
    };
    assert!(slot < SKDM_OUTCOMES);
    slot
}

/// Every variant indexes inside its array, and its slot agrees with the
/// discriminant the recorders use. Const-evaluated, so a mismatch is a build
/// failure rather than a panic on the first send.
const _: () = {
    assert!(group_slot(GroupDevicesMemoOutcome::Hit) == GroupDevicesMemoOutcome::Hit as usize);
    assert!(
        group_slot(GroupDevicesMemoOutcome::Restamp) == GroupDevicesMemoOutcome::Restamp as usize
    );
    assert!(
        group_slot(GroupDevicesMemoOutcome::MissAbsent)
            == GroupDevicesMemoOutcome::MissAbsent as usize
    );
    assert!(
        group_slot(GroupDevicesMemoOutcome::MissGroupInfo)
            == GroupDevicesMemoOutcome::MissGroupInfo as usize
    );
    assert!(
        group_slot(GroupDevicesMemoOutcome::MissTopology)
            == GroupDevicesMemoOutcome::MissTopology as usize
    );
    assert!(
        group_slot(GroupDevicesMemoOutcome::Bypassed) == GroupDevicesMemoOutcome::Bypassed as usize
    );
    assert!(skdm_slot(SkdmTargetsMemoOutcome::Hit) == SkdmTargetsMemoOutcome::Hit as usize);
    assert!(
        skdm_slot(SkdmTargetsMemoOutcome::MissAbsent)
            == SkdmTargetsMemoOutcome::MissAbsent as usize
    );
    assert!(
        skdm_slot(SkdmTargetsMemoOutcome::MissDevices)
            == SkdmTargetsMemoOutcome::MissDevices as usize
    );
    assert!(skdm_slot(SkdmTargetsMemoOutcome::MissMap) == SkdmTargetsMemoOutcome::MissMap as usize);
    assert!(
        skdm_slot(SkdmTargetsMemoOutcome::MissMapGeneration)
            == SkdmTargetsMemoOutcome::MissMapGeneration as usize
    );
    assert!(
        skdm_slot(SkdmTargetsMemoOutcome::MissSender)
            == SkdmTargetsMemoOutcome::MissSender as usize
    );
    assert!(
        skdm_slot(SkdmTargetsMemoOutcome::Bypassed) == SkdmTargetsMemoOutcome::Bypassed as usize
    );
    assert!(
        skdm_slot(SkdmTargetsMemoOutcome::ResolveFailed)
            == SkdmTargetsMemoOutcome::ResolveFailed as usize
    );
};

/// The counters themselves. One per `Client`.
///
/// Arrays rather than named fields so recording an outcome is
/// `slot[outcome as usize].fetch_add(1, Relaxed)` — one indexed atomic add, no
/// branch on which variant it was. [`group_slot`] and [`skdm_slot`] are what
/// keep that indexing in range as the enums grow.
#[derive(Debug, Default)]
pub(crate) struct DeviceMemoCounters {
    group: [AtomicU64; GROUP_OUTCOMES],
    skdm: [AtomicU64; SKDM_OUTCOMES],
    /// Not an outcome — see [`Self::record_skdm_not_stored`] — so it is not in
    /// the array and never counts toward `calls()`.
    skdm_not_stored: AtomicU64,
}

impl DeviceMemoCounters {
    pub(crate) fn record_group_devices(&self, outcome: GroupDevicesMemoOutcome) {
        self.group[outcome as usize].fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn record_skdm_targets(&self, outcome: SkdmTargetsMemoOutcome) {
        self.skdm[outcome as usize].fetch_add(1, Ordering::Relaxed);
    }

    /// A resolved target set that could not be memoized, so the next call
    /// cannot hit. Recorded in addition to that call's own outcome, not
    /// instead of it — it describes the *store*, not the lookup.
    pub(crate) fn record_skdm_not_stored(&self) {
        self.skdm_not_stored.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn snapshot(&self) -> DeviceMemoStats {
        let group =
            |outcome: GroupDevicesMemoOutcome| self.group[outcome as usize].load(Ordering::Relaxed);
        let skdm =
            |outcome: SkdmTargetsMemoOutcome| self.skdm[outcome as usize].load(Ordering::Relaxed);
        DeviceMemoStats {
            group_devices: GroupDevicesMemoStats {
                hits: group(GroupDevicesMemoOutcome::Hit),
                restamps: group(GroupDevicesMemoOutcome::Restamp),
                miss_absent: group(GroupDevicesMemoOutcome::MissAbsent),
                miss_group_info: group(GroupDevicesMemoOutcome::MissGroupInfo),
                miss_topology: group(GroupDevicesMemoOutcome::MissTopology),
                bypassed: group(GroupDevicesMemoOutcome::Bypassed),
            },
            skdm_targets: SkdmTargetsMemoStats {
                hits: skdm(SkdmTargetsMemoOutcome::Hit),
                miss_absent: skdm(SkdmTargetsMemoOutcome::MissAbsent),
                miss_devices: skdm(SkdmTargetsMemoOutcome::MissDevices),
                miss_map: skdm(SkdmTargetsMemoOutcome::MissMap),
                miss_map_generation: skdm(SkdmTargetsMemoOutcome::MissMapGeneration),
                miss_sender: skdm(SkdmTargetsMemoOutcome::MissSender),
                not_stored: self.skdm_not_stored.load(Ordering::Relaxed),
                bypassed: skdm(SkdmTargetsMemoOutcome::Bypassed),
                resolve_failed: skdm(SkdmTargetsMemoOutcome::ResolveFailed),
            },
        }
    }
}

/// Outcomes of `resolve_group_devices_memoized`, one per call.
///
/// A re-stamp serves the same device list a hit would; it is counted apart
/// because it pays the `unchanged_for` scan first, and because a client whose
/// re-stamps dominate its hits is being pushed by unrelated topology writes.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GroupDevicesMemoStats {
    pub hits: u64,
    pub restamps: u64,
    /// No entry for the group: first send, or a capacity/TTL eviction. The
    /// memo holds 64 groups, so a client rotating over more than that reports
    /// its eviction rate here.
    pub miss_absent: u64,
    /// An entry existed, built from a different `Arc<GroupInfo>`.
    pub miss_group_info: u64,
    /// The device topology changed in a way that could have touched this group.
    pub miss_topology: u64,
    /// Calls that did not consult the memo at all (store-backed caches).
    pub bypassed: u64,
}

impl GroupDevicesMemoStats {
    pub fn calls(&self) -> u64 {
        self.hits
            + self.restamps
            + self.miss_absent
            + self.miss_group_info
            + self.miss_topology
            + self.bypassed
    }

    /// Share of calls served without re-resolving the device list — hits and
    /// re-stamps together, since both skip the per-member registry fan-out.
    /// `None` when nothing was resolved yet.
    pub fn served_rate(&self) -> Option<f64> {
        let calls = self.calls();
        (calls > 0).then(|| (self.hits + self.restamps) as f64 / calls as f64)
    }
}

/// Outcomes of `resolve_skdm_targets_memoized`, one per call.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkdmTargetsMemoStats {
    pub hits: u64,
    pub miss_absent: u64,
    /// The device-set `Arc` changed. Read this against
    /// [`GroupDevicesMemoStats`]: the `Arc` is what the group memo returns, so
    /// this counter cannot go to zero while that memo is recomputing.
    pub miss_devices: u64,
    pub miss_map: u64,
    pub miss_map_generation: u64,
    pub miss_sender: u64,
    /// Resolutions whose target set was neither empty nor own-devices-only, so
    /// nothing was memoized. Each of these guarantees the *next* call cannot
    /// hit — but not that it reports [`Self::miss_absent`]: when a stale entry
    /// was already there it is left in place (nothing overwrites it, and it
    /// can never become valid again — the map generation only moves forward
    /// and the `Weak` keeps the old device allocation alive, so no `ptr::eq`
    /// can spuriously match). The next call then reports whichever term is
    /// still failing. A run of these means the group is not settling into the
    /// warm steady state at all, which is why it is reported next to the miss
    /// counts rather than folded into them.
    pub not_stored: u64,
    pub bypassed: u64,
    /// Calls that never reached a memo term because the device resolution they
    /// depend on returned an error. Counted so [`Self::calls`] stays one per
    /// call to `resolve_skdm_targets_memoized`; a rising value here means
    /// group sends are failing upstream of anything this report describes.
    pub resolve_failed: u64,
}

impl SkdmTargetsMemoStats {
    pub fn calls(&self) -> u64 {
        self.hits
            + self.miss_absent
            + self.miss_devices
            + self.miss_map
            + self.miss_map_generation
            + self.miss_sender
            + self.bypassed
            + self.resolve_failed
    }

    /// Memo hits over *every* call to the resolver, [`Self::resolve_failed`]
    /// included. `None` when nothing was resolved yet.
    ///
    /// Deliberately not "share that skipped `filter_skdm_targets`": a call
    /// whose device resolution errored also never reached the filter, and
    /// crediting it would let a client whose group sends are failing report a
    /// rising hit rate. Keeping those in the denominator makes the rate sag
    /// under exactly the condition worth noticing.
    pub fn hit_rate(&self) -> Option<f64> {
        let calls = self.calls();
        (calls > 0).then(|| self.hits as f64 / calls as f64)
    }
}

/// Cumulative per-term outcomes of both device-list memos on the group send
/// path, from [`Client::device_memo_stats`].
///
/// Counts are monotonic for the client's lifetime; take two snapshots and
/// subtract to scope them to a workload. Numbers only, no JIDs — same rule as
/// every other report in `wacore::stats`.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceMemoStats {
    pub group_devices: GroupDevicesMemoStats,
    pub skdm_targets: SkdmTargetsMemoStats,
}

impl DeviceMemoStats {
    /// This snapshot minus an earlier one, so a workload can be scoped without
    /// a reset (a reset would race any other send in flight). Saturating, so a
    /// caller that passes the two the wrong way round reads zeros rather than
    /// wrapping into billions.
    pub fn since(&self, earlier: &Self) -> Self {
        let group = &self.group_devices;
        let was = &earlier.group_devices;
        let skdm = &self.skdm_targets;
        let skdm_was = &earlier.skdm_targets;
        Self {
            group_devices: GroupDevicesMemoStats {
                hits: group.hits.saturating_sub(was.hits),
                restamps: group.restamps.saturating_sub(was.restamps),
                miss_absent: group.miss_absent.saturating_sub(was.miss_absent),
                miss_group_info: group.miss_group_info.saturating_sub(was.miss_group_info),
                miss_topology: group.miss_topology.saturating_sub(was.miss_topology),
                bypassed: group.bypassed.saturating_sub(was.bypassed),
            },
            skdm_targets: SkdmTargetsMemoStats {
                hits: skdm.hits.saturating_sub(skdm_was.hits),
                miss_absent: skdm.miss_absent.saturating_sub(skdm_was.miss_absent),
                miss_devices: skdm.miss_devices.saturating_sub(skdm_was.miss_devices),
                miss_map: skdm.miss_map.saturating_sub(skdm_was.miss_map),
                miss_map_generation: skdm
                    .miss_map_generation
                    .saturating_sub(skdm_was.miss_map_generation),
                miss_sender: skdm.miss_sender.saturating_sub(skdm_was.miss_sender),
                not_stored: skdm.not_stored.saturating_sub(skdm_was.not_stored),
                bypassed: skdm.bypassed.saturating_sub(skdm_was.bypassed),
                resolve_failed: skdm.resolve_failed.saturating_sub(skdm_was.resolve_failed),
            },
        }
    }
}

impl std::fmt::Display for DeviceMemoStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let group = &self.group_devices;
        let skdm = &self.skdm_targets;
        writeln!(f, "=== Device memo stats ===")?;
        writeln!(
            f,
            "  group_devices: {} calls, {} hit, {} restamp, miss: {} absent / {} group_info / {} topology, {} bypassed",
            group.calls(),
            group.hits,
            group.restamps,
            group.miss_absent,
            group.miss_group_info,
            group.miss_topology,
            group.bypassed
        )?;
        write!(
            f,
            "  skdm_targets:  {} calls, {} hit, miss: {} absent / {} devices / {} map / {} map_gen / {} sender, {} not stored, {} bypassed, {} resolve failed",
            skdm.calls(),
            skdm.hits,
            skdm.miss_absent,
            skdm.miss_devices,
            skdm.miss_map,
            skdm.miss_map_generation,
            skdm.miss_sender,
            skdm.not_stored,
            skdm.bypassed,
            skdm.resolve_failed
        )
    }
}

impl Client {
    /// Per-term hit/miss counts for the two device-list memos on the group
    /// send path, cumulative since the client was built.
    ///
    /// The two are chained — `resolve_skdm_targets_memoized` compares the
    /// `Arc` that `resolve_group_devices_memoized` returned — so a group-memo
    /// recompute forces `skdm_targets.miss_devices` no matter what the other
    /// three SKDM terms say. Read the group half first; the SKDM half only
    /// carries independent information once the group half is hitting.
    pub fn device_memo_stats(&self) -> DeviceMemoStats {
        self.device_memo_counters.snapshot()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exactly one counter per call is what makes `calls()` a sum rather than
    /// a fourth thing that can disagree with the other three.
    #[test]
    fn every_outcome_lands_in_calls_exactly_once() {
        let counters = DeviceMemoCounters::default();
        for outcome in [
            GroupDevicesMemoOutcome::Hit,
            GroupDevicesMemoOutcome::Restamp,
            GroupDevicesMemoOutcome::MissAbsent,
            GroupDevicesMemoOutcome::MissGroupInfo,
            GroupDevicesMemoOutcome::MissTopology,
            GroupDevicesMemoOutcome::Bypassed,
        ] {
            counters.record_group_devices(outcome);
        }
        for outcome in [
            SkdmTargetsMemoOutcome::Hit,
            SkdmTargetsMemoOutcome::MissAbsent,
            SkdmTargetsMemoOutcome::MissDevices,
            SkdmTargetsMemoOutcome::MissMap,
            SkdmTargetsMemoOutcome::MissMapGeneration,
            SkdmTargetsMemoOutcome::MissSender,
            SkdmTargetsMemoOutcome::Bypassed,
            SkdmTargetsMemoOutcome::ResolveFailed,
        ] {
            counters.record_skdm_targets(outcome);
        }
        // Not an outcome: it describes the store, so it must not inflate calls.
        counters.record_skdm_not_stored();

        let stats = counters.snapshot();
        assert_eq!(stats.group_devices.calls(), GROUP_OUTCOMES as u64);
        assert_eq!(stats.skdm_targets.calls(), SKDM_OUTCOMES as u64);
        assert_eq!(stats.skdm_targets.not_stored, 1);
    }

    #[test]
    fn rates_are_absent_rather_than_zero_before_the_first_call() {
        let stats = DeviceMemoStats::default();
        assert_eq!(stats.group_devices.served_rate(), None);
        assert_eq!(stats.skdm_targets.hit_rate(), None);
    }
}
