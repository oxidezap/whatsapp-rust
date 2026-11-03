use super::{app_state::*, history::*};
use crate::{appstate::patch_decode::WAPatchName, time::Instant};
use std::time::Duration;

#[test]
fn scope_rejects_retired_before_reading_clock() {
    let scope = SyncScope {
        generation: 7,
        deadline: Some(Instant::now()),
    };
    assert_eq!(
        scope.admits(8, || panic!("retired work must not read time")),
        Err(ScopeLost::Retired)
    );
}

#[test]
fn unbounded_scope_never_reads_clock() {
    let scope = SyncScope {
        generation: 7,
        deadline: None,
    };
    assert_eq!(
        scope.admits(7, || panic!("unbounded work must not read time")),
        Ok(())
    );
    assert_eq!(scope.remaining_with(|| panic!("no deadline")), None);
}

#[test]
fn scope_expires_exactly_at_deadline_and_saturates_remaining() {
    let now = Instant::now();
    let deadline = now + Duration::from_secs(5);
    let scope = SyncScope {
        generation: 7,
        deadline: Some(deadline),
    };
    assert_eq!(scope.admits(7, || now), Ok(()));
    assert_eq!(scope.admits(7, || deadline), Err(ScopeLost::Expired));
    assert_eq!(
        scope.remaining_with(|| deadline + Duration::from_secs(1)),
        Some(Duration::ZERO)
    );
}

#[test]
fn retry_budget_bounds_unproductive_rounds_without_charging_attempts() {
    let mut budget = RetryBudget::default();
    for _ in 0..32 {
        assert_eq!(budget.next_delay(), Some(Duration::from_secs(1)));
    }
    assert_eq!(budget.next_delay(), None);
    assert_eq!(budget.attempts(), 0);
}

#[test]
fn retry_budget_exhausts_eight_actual_attempts() {
    let mut budget = RetryBudget::default();
    for attempt in 0..8 {
        assert_eq!(budget.next_delay(), Some(Duration::from_secs(1 << attempt)));
        budget.record_attempt();
    }
    assert_eq!(budget.next_delay(), None);
    assert_eq!(budget.attempts(), 8);
    assert_eq!(retry_backoff(u32::MAX), Duration::from_secs(3600));
}

#[test]
fn generated_retry_schedules_match_original_round_and_attempt_limits() {
    for schedule in 0..(1u32 << 12) {
        let mut state = RetryBudget::default();
        let mut attempts = 0;
        for round in 0..33 {
            let expected = (attempts < 8 && round < 32).then(|| retry_backoff(attempts));
            assert_eq!(state.next_delay(), expected);
            if expected.is_none() {
                break;
            }
            if schedule & (1 << (round % 12)) != 0 {
                state.record_attempt();
                attempts += 1;
            }
            assert_eq!(state.attempts(), attempts);
        }
    }
}

#[test]
fn reservation_equivalence_never_covers_a_patch_writer() {
    for wait in [
        ReservationWait::SkipBehindSync,
        ReservationWait::Always,
        ReservationWait::TryOnce,
    ] {
        for holder in [SyncHolder::Sync, SyncHolder::PatchSend] {
            let expected = match (wait, holder) {
                (ReservationWait::TryOnce, _) => Some(ReservationSkip::WaitTimedOut),
                (ReservationWait::SkipBehindSync, SyncHolder::Sync) => {
                    Some(ReservationSkip::EquivalentSyncInFlight)
                }
                _ => None,
            };
            assert_eq!(wait.blocked_by(holder), expected);
        }
    }
}

#[test]
fn generated_batch_partitions_preserve_all_outstanding_collections() {
    let names = [
        WAPatchName::CriticalBlock,
        WAPatchName::CriticalUnblockLow,
        WAPatchName::RegularHigh,
        WAPatchName::RegularLow,
        WAPatchName::Regular,
    ];
    for assignment in 0..4usize.pow(names.len() as u32) {
        for reached_server in [false, true] {
            let mut outcome = BatchedSyncOutcome {
                reached_server,
                ..Default::default()
            };
            let mut expected_retry = Vec::new();
            let mut expected_stranded = false;
            let mut digits = assignment;
            for name in names {
                match digits % 4 {
                    0 => outcome.synced.push(name),
                    1 => {
                        outcome.fatal.push(name);
                        expected_stranded = true;
                    }
                    2 => {
                        outcome.retryable.push(name);
                        expected_retry.push(name);
                    }
                    _ => outcome.skipped.push(name),
                }
                digits /= 4;
            }
            expected_retry.extend(outcome.skipped.iter().copied());
            let plan = CriticalSyncPlan::from_outcome(&outcome);
            assert_eq!(plan.retry, expected_retry);
            assert_eq!(plan.stranded, expected_stranded);
            assert_eq!(plan.outstanding(), !outcome.all_synced());
            assert_eq!(
                outcome.unsynced().count() + outcome.synced.len(),
                names.len()
            );
        }
    }
}

#[test]
fn retry_success_does_not_erase_an_earlier_fatal_or_skipped_collection() {
    for skip in [false, true] {
        let mut state = CollectionRetryState::new(SyncSettles::InitialSync, false);
        let mut partial = BatchedSyncOutcome {
            retryable: vec![WAPatchName::Regular],
            ..Default::default()
        };
        if skip {
            partial.skipped.push(WAPatchName::RegularLow);
        } else {
            partial.fatal.push(WAPatchName::RegularLow);
        }
        assert_eq!(state.observe(&partial), RetryProgress::Pending);
        assert_eq!(
            state.observe(&BatchedSyncOutcome::default()),
            RetryProgress::Complete {
                bootstrap_outstanding: Some(true)
            }
        );
    }
}

#[test]
fn reconnect_keeps_retry_work_but_revokes_bootstrap_authority() {
    let mut scope = SyncScope {
        generation: 1,
        deadline: None,
    };
    let mut state = CollectionRetryState::new(SyncSettles::InitialSync, false);
    assert!(!state.rebind(&mut scope, 1));
    assert!(state.rebind(&mut scope, 2));
    assert_eq!(scope.generation(), 2);
    assert_eq!(
        state.observe(&BatchedSyncOutcome::default()),
        RetryProgress::Complete {
            bootstrap_outstanding: None
        }
    );
}

#[test]
fn only_initial_sync_may_settle_bootstrap_and_stranded_is_sticky() {
    for settles in [SyncSettles::InitialSync, SyncSettles::JustTheCollections] {
        for stranded in [false, true] {
            let mut state = CollectionRetryState::new(settles, stranded);
            assert_eq!(
                state.observe(&BatchedSyncOutcome::default()),
                RetryProgress::Complete {
                    bootstrap_outstanding: (settles == SyncSettles::InitialSync)
                        .then_some(stranded),
                }
            );
        }
    }
}

#[test]
fn bootstrap_pairing_rejects_every_preexisting_writer() {
    for tagged in 0..16 {
        for sampled in 0..16 {
            let state = BootstrapState::from_word(tagged << 1).arm_for_pairing(sampled);
            assert!(state.is_armed());
            assert_eq!(state.settle(tagged.max(sampled), false), None);
            let next = state.settle(tagged.max(sampled) + 1, false).unwrap();
            assert!(!next.is_armed());
        }
    }
}

#[test]
fn bootstrap_compare_and_swap_retry_preserves_the_newer_owner() {
    let initial = BootstrapState::new(true);
    let sampled = initial.settle(2, false).unwrap();
    let replacement = initial.settle(3, true).unwrap();
    assert_ne!(sampled.word(), replacement.word());
    assert_eq!(replacement.settle(2, false), None);
    assert_eq!(replacement.settle(3, false).unwrap().word(), 6);
}

#[test]
fn response_omission_is_distinct_from_failed_apply() {
    assert_eq!(unaccounted_for(false, false), Some(Unaccounted::Omitted));
    assert_eq!(unaccounted_for(true, false), Some(Unaccounted::NotApplied));
    assert_eq!(unaccounted_for(false, true), None);
    assert_eq!(unaccounted_for(true, true), None);
}

#[test]
fn key_retry_bounds_tiny_and_unbounded_waits() {
    assert_eq!(initial_key_retry(Duration::ZERO), Duration::from_millis(1));
    assert_eq!(
        initial_key_retry(Duration::from_secs(10)),
        Duration::from_secs(5)
    );
    assert_eq!(initial_key_retry(Duration::MAX), Duration::from_secs(10));
    assert_eq!(next_key_retry(Duration::MAX), Duration::from_secs(60));
}

#[test]
fn skip_does_not_invoke_history_admission() {
    assert_eq!(
        HistoryAdmission::decide(true, || panic!("skip bypasses policy")),
        HistoryAdmission::RejectAndAcknowledge
    );
    assert_eq!(
        HistoryAdmission::decide(false, || true),
        HistoryAdmission::RejectAndAcknowledge
    );
    assert_eq!(
        HistoryAdmission::decide(false, || false),
        HistoryAdmission::Process
    );
}

#[test]
fn history_receipt_has_exactly_one_publication_boundary() {
    for recovery in [HistoryRecovery::Session, HistoryRecovery::Durable] {
        for capture in [false, true] {
            let publication = HistoryPublication::new(recovery, capture);
            let points = [
                HistoryReceiptPoint::BeforePayload,
                HistoryReceiptPoint::Captured,
                HistoryReceiptPoint::Parsed,
            ];
            assert_eq!(
                points
                    .into_iter()
                    .filter(|point| publication.receipt_at(*point))
                    .count(),
                1
            );
            assert_eq!(
                publication.receipt_at(HistoryReceiptPoint::Parsed),
                recovery == HistoryRecovery::Durable
            );
            assert_eq!(
                publication.receipt_at(HistoryReceiptPoint::BeforePayload),
                recovery == HistoryRecovery::Session && !capture
            );
        }
    }
}

#[test]
fn durable_history_survives_reconnect_but_not_permanent_shutdown() {
    for shutting_down in [false, true] {
        for permanent in [false, true] {
            assert_eq!(
                HistoryRecovery::Session.stopped(|| shutting_down, || permanent),
                shutting_down
            );
            assert_eq!(
                HistoryRecovery::Durable.stopped(|| shutting_down, || permanent),
                permanent
            );
        }
    }
}

#[test]
fn stale_history_resize_and_release_cannot_touch_new_work() {
    let mut activity = HistoryActivity::default();
    let stale = activity.begin(100);
    activity.reset();
    let live = activity.begin(200);
    activity.resize(stale, 100, usize::MAX);
    assert!(!activity.release(stale, usize::MAX));
    assert_eq!(
        activity.snapshot(),
        HistoryActivitySnapshot {
            tasks: 1,
            tasks_peak: 1,
            payload_bytes: 200,
            payload_bytes_peak: 200
        }
    );
    activity.resize(live, 200, 300);
    assert!(activity.release(live, 300));
    assert_eq!(activity.snapshot().payload_bytes, 0);
    assert_eq!(activity.snapshot().payload_bytes_peak, 300);
}

#[test]
fn activity_saturates_overflow_and_preserves_peaks_across_reset() {
    let mut activity = HistoryActivity::default();
    activity.begin(usize::MAX);
    activity.begin(1);
    assert_eq!(activity.snapshot().payload_bytes, usize::MAX);
    activity.reset();
    assert_eq!(
        activity.snapshot(),
        HistoryActivitySnapshot {
            tasks: 0,
            tasks_peak: 2,
            payload_bytes: 0,
            payload_bytes_peak: usize::MAX
        }
    );
}

#[test]
fn history_shutdown_reads_only_the_selected_recovery_domain() {
    assert!(HistoryRecovery::Session.stopped(|| true, || panic!("not a durable replay")));
    assert!(HistoryRecovery::Durable.stopped(|| panic!("reconnect must not stop replay"), || true));
}

#[test]
fn extracted_control_state_keeps_fixed_storage_bounded() {
    use std::mem::size_of;
    assert_eq!(size_of::<RetryBudget>(), 2 * size_of::<u32>());
    assert_eq!(size_of::<BootstrapState>(), size_of::<u64>());
    assert_eq!(size_of::<HistoryActivity>(), 5 * size_of::<usize>());
    assert_eq!(size_of::<HistoryActivitySnapshot>(), 4 * size_of::<usize>());
    assert!(size_of::<HistoryPublication>() <= 2);
    assert!(size_of::<CollectionRetryState>() <= 2);
}

#[test]
fn conflict_without_more_requires_a_persisted_baseline() {
    use crate::appstate::patch_decode::CollectionSyncError;
    for baseline in [false, true] {
        assert_eq!(
            CollectionDisposition::from_error(
                &CollectionSyncError::Conflict { has_more: true },
                baseline
            ),
            CollectionDisposition::Refetch
        );
        assert_eq!(
            CollectionDisposition::from_error(
                &CollectionSyncError::Conflict { has_more: false },
                baseline
            ),
            if baseline {
                CollectionDisposition::Synced
            } else {
                CollectionDisposition::Retryable
            }
        );
    }
}

#[test]
fn server_refusal_is_never_an_immediate_refetch() {
    use crate::appstate::patch_decode::CollectionSyncError;
    let retry = CollectionSyncError::Retry {
        code: 503,
        text: "unavailable".into(),
    };
    let fatal = CollectionSyncError::Fatal {
        code: 404,
        text: "unknown".into(),
    };
    for baseline in [false, true] {
        assert_eq!(
            CollectionDisposition::from_error(&retry, baseline),
            CollectionDisposition::Retryable
        );
        assert_eq!(
            CollectionDisposition::from_error(&fatal, baseline),
            CollectionDisposition::Fatal
        );
    }
}

#[test]
fn snapshot_recovery_refuses_unknown_names_and_block_list() {
    assert_eq!(
        RecoveryCollection::admit(WAPatchName::Unknown),
        Err(SnapshotRecoveryRefusal::UnknownCollection)
    );
    assert_eq!(
        RecoveryCollection::admit(WAPatchName::CriticalBlock),
        Err(SnapshotRecoveryRefusal::BlockList)
    );
    for name in [
        WAPatchName::CriticalUnblockLow,
        WAPatchName::Regular,
        WAPatchName::RegularHigh,
        WAPatchName::RegularLow,
    ] {
        assert_eq!(RecoveryCollection::admit(name).unwrap().name(), name);
    }
}

#[test]
fn recovery_record_limit_is_inclusive_and_nonpositive_means_unbounded() {
    let collection = RecoveryCollection::admit(WAPatchName::Regular).unwrap();
    assert!(collection.admit_records(0, 2000).is_ok());
    assert!(collection.admit_records(2000, 2000).is_ok());
    assert_eq!(
        collection.admit_records(2001, 2000),
        Err(SnapshotRecoveryRefusal::TooManyRecords)
    );
    for allowed in [0, -1, i64::MIN] {
        assert!(collection.admit_records(usize::MAX, allowed).is_ok());
    }
}

#[test]
fn pagination_allows_exactly_five_hundred_pages() {
    let mut pagination = Pagination::default();
    for expected in 1..=500 {
        assert_eq!(pagination.next_page(), Some(expected));
    }
    assert_eq!(pagination.next_page(), None);
    assert_eq!(pagination.next_page(), None);
}

#[test]
fn snapshot_recovery_escalates_only_snapshot_authentication_failures() {
    use crate::appstate::AppStateError;
    for name in [WAPatchName::Regular, WAPatchName::CriticalBlock] {
        assert_eq!(
            RecoveryRequest::for_error(name, None),
            RecoveryRequest::NotNeeded
        );
        for error in [
            AppStateError::SnapshotMACMissing,
            AppStateError::SnapshotMACMismatch,
        ] {
            assert_eq!(
                RecoveryRequest::for_error(name, Some(&error)),
                if name == WAPatchName::CriticalBlock {
                    RecoveryRequest::BlockList
                } else {
                    RecoveryRequest::Eligible
                }
            );
        }
    }
}

#[test]
fn absent_rollout_is_not_a_refusal_and_false_is_case_insensitive() {
    assert!(RecoveryRequest::allowed_by_rollout(None));
    for value in ["1", "true", "", " false ", "00"] {
        assert!(RecoveryRequest::allowed_by_rollout(Some(value)));
    }
    for value in ["0", "false", "FALSE", "FaLsE"] {
        assert!(!RecoveryRequest::allowed_by_rollout(Some(value)));
    }
}
